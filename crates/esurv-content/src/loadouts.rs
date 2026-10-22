//! Weapon loadouts (roadmap R3): before a run a class picks one of its three loadouts, each a set
//! of weapons with strengths and drawbacks (`content/loadouts.ron`);
//! docs/game/AUTHORING.md "Weapon loadouts".
//!
//! A loadout's weapons set the swing per hand (seconds and damage share of the class's
//! `auto_attack` damage) for the classes that swing; its `effects` use the talents' vocabulary
//! (stats, every-spell numbers, milestones). Each weapon has a look per zone (a
//! zone's "armaments", which one drop of the zone's class weapons unlocks for all three
//! loadouts), a zone's rows as the class set's (`gear.ron` `looks`).

use serde::{Deserialize, Serialize};

use crate::schema::{GearLook, Hand, TalentEffect, WeaponKind};

/// Every loadout of one class, the default first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassLoadouts {
    pub class: String,
    pub loadouts: Vec<Loadout>,
}

/// One weapon loadout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Loadout {
    /// Unique within its class (`two_hander`, `dual_wield` ...); the save names it.
    pub id: String,
    pub name: String,
    /// The icon (`Interface\Icons` name) that stands for it on the warband screen.
    pub icon: String,
    /// The weapons, at most one per hand.
    pub weapons: Vec<LoadoutWeapon>,
    /// What it gives and what it costs, as the cards show them (green and red).
    pub strength: String,
    pub drawback: String,
    /// The spec it suits best (a hint; every loadout works with any spec).
    #[serde(default)]
    pub suits: String,
    /// Its stats, every-spell numbers and procs, in the talents' vocabulary (rank 1).
    #[serde(default)]
    pub effects: Vec<TalentEffect>,
}

/// One weapon of a loadout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadoutWeapon {
    pub hand: Hand,
    pub kind: WeaponKind,
    /// How the cards name it ("Two-handed sword").
    pub name: String,
    /// Its swing, for a class that swings; missing: it swings not (a shield, a caster's weapon).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swing: Option<WeaponSwing>,
    /// Its look in each zone's armaments, in the zones' difficulty order.
    pub looks: Vec<ArmamentLook>,
}

/// A loadout weapon's look in one zone's armaments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmamentLook {
    /// The zone's id, and its row among the zone's looks (as the class set's, `GearLook`).
    pub zone: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub row: u32,
    /// The item it looks like, as the collection names it.
    pub name: String,
    /// Its `ItemDisplayInfo.dbc` look.
    pub display: u32,
    /// Its own kind where it is not the loadout weapon's (an axe in a line of swords): it sets the
    /// grip and where it rests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<WeaponKind>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl LoadoutWeapon {
    /// Its look in the armaments of `look` (a zone's row of the class set): that row's, else the
    /// zone's first row's, else its first look.
    pub fn look_in(&self, look: &GearLook) -> Option<&ArmamentLook> {
        let of = |row| self.looks.iter().find(|l| l.zone == look.zone && l.row == row);
        of(look.row).or_else(|| of(0)).or(self.looks.first())
    }

    /// Its look in zone `zone`'s (an id) armaments of row `row`; 0 for none.
    pub fn zone_display(&self, zone: &str, row: u32) -> u32 {
        self.looks.iter().find(|l| l.zone == zone && l.row == row).map_or(0, |l| l.display)
    }

    /// Its kind in look `armament` (its own where the look has none).
    pub fn kind_of(&self, armament: &ArmamentLook) -> WeaponKind {
        armament.kind.unwrap_or(self.kind)
    }
}

/// A weapon's swing: every `seconds`, for `share` of the class's swing damage.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeaponSwing {
    pub seconds: f32,
    pub share: f32,
}

impl Loadout {
    /// Its weapon in `hand`.
    pub fn weapon(&self, hand: Hand) -> Option<&LoadoutWeapon> {
        self.weapons.iter().find(|w| w.hand == hand)
    }

    /// Its swings, main hand (or ranged) first, then the off hand.
    pub fn swings(&self) -> Vec<WeaponSwing> {
        let order = [Hand::MainHand, Hand::Ranged, Hand::OffHand];
        order.iter().filter_map(|h| self.weapon(*h)?.swing).collect()
    }
}
