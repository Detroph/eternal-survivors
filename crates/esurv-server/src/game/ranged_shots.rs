//! A hero with a ranged swing (the Hunter's Auto Shot) shooting: its Auto Shot and its ranged
//! spells play the shooting of the ranged weapon it holds, a bow's draw for a bow, a rifle's for a
//! gun, a crossbow's (the rifle's clips with the bow's sounds) for a crossbow
//! (docs/game/server.md, "The starting options").
//!
//! The client does most of it itself, the 1.12 way: a ranged-slot spell (`Spell.dbc`
//! `Attributes & 0x2`: Auto Shot, Multi-Shot, the stings, Aimed and Steady Shot ...) fills its
//! look's empty stages from the held weapon look's own shooting look (`ItemDisplayInfo` column
//! 10: visual 5 on bows, 743 on crossbows, 224 on guns), its sounds and arrow flight included. A
//! second look on top would ring a second, maybe wrong, weapon (a crossbow sounding like a gun),
//! so the server adds one only where the client draws none: a ranged spell that is no ranged-slot
//! spell (Explosive Shot looks like Explosive Trap's blast) gets the held weapon's basic shot
//! (Shoot Bow, Shoot Gun, Shoot Crossbow: no look of their own, the weapon's filled in whole);
//! a weapon look with no shooting look of its own gets a stock shot that brings one (a bow's
//! 3018, a rifle's 8682), whatever the spell. The weapon is read from the item in the ranged
//! slot, so a weapon changed later (a loadout) shoots its own way.

use esurv_content::SpellCategory;
use esurv_sim::game::kit::swing_of;

use super::Game;
use crate::ground::GameData;
use crate::looks;

/// The ranged equipment slot.
const RANGED_SLOT: usize = 17;
/// 1.12's item class of weapons, and the ranged subclasses that shoot.
const ITEM_CLASS_WEAPON: u32 = 2;
const SUBCLASS_BOW: u32 = 2;
const SUBCLASS_GUN: u32 = 3;
const SUBCLASS_CROSSBOW: u32 = 18;
/// `Spell.dbc` looks whose own cast plays a shot, for a weapon look with none of its own: a
/// bow's (3018, its visual 5: the bow drawn and loosed, the arrow flying and striking), a
/// rifle's (8682, its visual 224: the rifle shouldered and fired). No stock spell carries a
/// crossbow's visual 743, so a crossbow without one shoulders as a rifle.
pub const BOW_SHOT_LOOK: u32 = 3_018;
pub const RIFLE_SHOT_LOOK: u32 = 8_682;
/// The basic shots, ranged-slot spells with no look of their own: the client shows the held
/// weapon look's own shooting look whole.
pub const SHOOT_BOW: u32 = 2_480;
pub const SHOOT_GUN: u32 = 7_918;
pub const SHOOT_CROSSBOW: u32 = 7_919;

/// The kinds of ranged weapon that shoot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotWeapon {
    Bow,
    Gun,
    Crossbow,
}

impl ShotWeapon {
    /// The kind of a ranged weapon of item `class` and `subclass`; `None` for anything that does
    /// not shoot (a wand, a thrown weapon, no weapon).
    pub fn of(class: u32, subclass: u32) -> Option<Self> {
        match (class, subclass) {
            (ITEM_CLASS_WEAPON, SUBCLASS_BOW) => Some(Self::Bow),
            (ITEM_CLASS_WEAPON, SUBCLASS_GUN) => Some(Self::Gun),
            (ITEM_CLASS_WEAPON, SUBCLASS_CROSSBOW) => Some(Self::Crossbow),
            _ => None,
        }
    }

    /// Its basic shot, which the client shows as the held weapon look's own shooting look.
    pub fn basic_shot(self) -> u32 {
        match self {
            Self::Bow => SHOOT_BOW,
            Self::Gun => SHOOT_GUN,
            Self::Crossbow => SHOOT_CROSSBOW,
        }
    }

    /// The stock shot that brings a look of its own, for a weapon look with none.
    pub fn stock_shot(self) -> u32 {
        match self {
            Self::Bow => BOW_SHOT_LOOK,
            Self::Gun | Self::Crossbow => RIFLE_SHOT_LOOK,
        }
    }
}

/// The shooting look sent beside a shot of a weapon of kind `weapon`: none when the client fills
/// the shot's look from the weapon look itself (`client_fills`, a ranged-slot spell, and the
/// weapon look has a shooting look, `weapon_look`), the basic shot when only the spell does not,
/// the stock shot when the weapon look has none (or the game data is unread).
pub fn shot_look_for(weapon: ShotWeapon, client_fills: bool, weapon_look: bool) -> Option<u32> {
    match (weapon_look, client_fills) {
        (true, true) => None,
        (true, false) => Some(weapon.basic_shot()),
        (false, _) => Some(weapon.stock_shot()),
    }
}

/// [`shot_look_for`] for a shot of look `spell` from a weapon of item `class`, `subclass` and
/// look `display`, read from the game data; `None` for a weapon that does not shoot.
pub fn shot_look_of(data: Option<&GameData>, spell: u32, class: u32, subclass: u32, display: u32) -> Option<u32> {
    let weapon = ShotWeapon::of(class, subclass)?;
    let Some(data) = data else {
        return Some(weapon.stock_shot());
    };
    shot_look_for(weapon, data.ranged_slot_spell(spell), data.display_shot_visual(display).is_some())
}

impl Game {
    /// The shot look of hero `hero`'s swing when it is a ranged one (the Hunter's Auto Shot):
    /// its hits are logged as that shot, as 1.12 logs an Auto Shot, not as a melee swing, which
    /// would make the client swing the gun.
    pub(super) fn ranged_swing_look(&self, hero: u32) -> Option<u32> {
        let h = self.run.heroes.iter().find(|h| h.id == hero)?;
        swing_of(&self.run.content, h)?.look_spell
    }

    /// The shooting look sent beside hero `hero`'s cast of look `spell` ([`shot_look_of`]), when
    /// `spell` is its ranged swing's shot or one of its known ranged spells; `None` where the
    /// client draws the shot from the weapon look itself, for anything else, a hero with a melee
    /// swing or none, or one holding no bow, gun or crossbow.
    pub(super) fn shot_look(&self, hero: u32, spell: u32) -> Option<u32> {
        let h = self.run.heroes.iter().find(|h| h.id == hero)?;
        let content = &self.run.content;
        let swing_shot = swing_of(content, h)?.look_spell?;
        let ranged_spell = || {
            h.spells.known.iter().filter_map(|s| content.ability(&s.spell)).any(|a| {
                a.look_spell == spell && a.categories.contains(&SpellCategory::Ranged)
            })
        };
        if spell != swing_shot && !ranged_spell() {
            return None;
        }
        let entry = *self.dressed.get(&hero)?.shown.get(RANGED_SLOT)?;
        let item = looks::item_info(content, self.data.as_deref(), entry)?;
        shot_look_of(self.data.as_deref(), spell, item.class, item.subclass, item.display_id)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bows_guns_and_crossbows_shoot_and_nothing_else_does() {
        assert_eq!(ShotWeapon::of(2, 2), Some(ShotWeapon::Bow));
        assert_eq!(ShotWeapon::of(2, 3), Some(ShotWeapon::Gun));
        assert_eq!(ShotWeapon::of(2, 18), Some(ShotWeapon::Crossbow));
        // a wand, a thrown weapon, a sword, a shield
        for (class, subclass) in [(2, 19), (2, 16), (2, 7), (4, 6)] {
            assert_eq!(ShotWeapon::of(class, subclass), None);
            assert_eq!(shot_look_of(None, 75, class, subclass, 0), None);
        }
    }

    /// Each weapon kind's pick: nothing beside a shot the client fills from the weapon look, the
    /// kind's own basic shot beside one it does not, a stock shot when the weapon look has none.
    #[test]
    fn a_shot_adds_a_look_only_where_the_client_draws_none() {
        use ShotWeapon::*;
        for weapon in [Bow, Gun, Crossbow] {
            assert_eq!(shot_look_for(weapon, true, true), None, "{weapon:?}");
        }
        assert_eq!(shot_look_for(Bow, false, true), Some(SHOOT_BOW));
        assert_eq!(shot_look_for(Gun, false, true), Some(SHOOT_GUN));
        assert_eq!(shot_look_for(Crossbow, false, true), Some(SHOOT_CROSSBOW));
        for fills in [true, false] {
            assert_eq!(shot_look_for(Bow, fills, false), Some(BOW_SHOT_LOOK));
            assert_eq!(shot_look_for(Gun, fills, false), Some(RIFLE_SHOT_LOOK));
            assert_eq!(shot_look_for(Crossbow, fills, false), Some(RIFLE_SHOT_LOOK));
        }
    }
}

/// Over the game data: every hunter shot, Auto Shot and each ranged spell, from every look of
/// the three loadouts' weapons.
#[cfg(test)]
mod data_tests {
    use super::*;

    /// Auto Shot, the hunter's swing look.
    const AUTO_SHOT: u32 = 75;

    #[test]
    fn every_hunter_shot_from_every_loadout_look_rings_its_own_weapon_once() {
        let dir = benilla_formats::wow_data_or_skip!();
        let names = esurv_content::ClientNames::read(&dir).unwrap();
        let content_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = esurv_content::Content::load_with_names(content_dir, &names).unwrap();
        let data = GameData::open(&dir, &content).unwrap();
        // the basic shots have no look of their own and are filled from the weapon look whole
        for shot in [SHOOT_BOW, SHOOT_GUN, SHOOT_CROSSBOW] {
            assert!(data.ranged_slot_spell(shot), "{shot}");
        }
        // the hunter's shots: the client fills all but Explosive Shot's from the weapon look
        let filled = ["arcane_shot", "aimed_shot", "concussive_shot", "multi_shot", "serpent_sting", "steady_shot", "volley"];
        let looks: Vec<(&str, u32)> = std::iter::once(("auto_shot", AUTO_SHOT))
            .chain(filled.iter().map(|id| (*id, content.ability(id).unwrap().look_spell)))
            .collect();
        let explosive = content.ability("explosive_shot").unwrap().look_spell;
        for (loadout, kind, shooting_look, basic) in [
            ("bow", ShotWeapon::Bow, 5, SHOOT_BOW),
            ("crossbow", ShotWeapon::Crossbow, 743, SHOOT_CROSSBOW),
            ("gun", ShotWeapon::Gun, 224, SHOOT_GUN),
        ] {
            let weapon = content.loadout("hunter", Some(loadout)).unwrap().weapon(esurv_content::Hand::Ranged).unwrap();
            assert!(!weapon.looks.is_empty(), "{loadout}");
            for look in &weapon.looks {
                let display = look.display;
                assert_eq!(data.display_shot_visual(display), Some(shooting_look), "{loadout} {display}");
                let pick = |spell| shot_look_of(Some(&data), spell, ITEM_CLASS_WEAPON, weapon.kind.subclass(), display);
                for &(id, spell) in &looks {
                    assert_eq!(pick(spell), None, "{loadout} {display} {id}: the client draws it, no second look");
                }
                assert_eq!(pick(explosive), Some(basic), "{loadout} {display} explosive_shot");
                assert_eq!(ShotWeapon::of(ITEM_CLASS_WEAPON, weapon.kind.subclass()), Some(kind));
            }
        }
    }
}
