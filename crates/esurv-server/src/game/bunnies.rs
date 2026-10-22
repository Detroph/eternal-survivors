//! Invisible target units, "bunnies" (docs/game/bosses.md, "Marked
//! attacks"): a boss look that plays its impact only on a unit it hits (a smite, a strike, a bolt)
//! would show nothing where a dodged mark lands. The server puts an invisible unit there, the
//! look's spell goes hitting it, and the unit goes a moment later. It has no name, no health bar,
//! cannot be targeted or attacked, takes up no room, and is no enemy of the simulation: no hero
//! attack ever picks it, and the damage stays the simulation's, the hero's alone.

use esurv_sim::game::enemy_ai::Vec2;
use esurv_sim::game::run::Unit;

use super::pickups::PICKUP_FACTION;
use super::{unit_guid, Game};
use crate::packets::{self, SpellTarget};
use crate::update::{field, update_object, Fields, Living, ObjectTypeId, ObjectUpdate, Speeds, TYPE_OBJECT, TYPE_UNIT};
use crate::wire::Packet;

/// The bunny's model: the client's invisible one (the stand-in of the 1.12 trigger creatures,
/// Dream Fog's and Poison Cloud's).
pub const BUNNY_DISPLAY: u32 = 11686;
/// It cannot be attacked or picked (`UNIT_FLAG_NON_ATTACKABLE | UNIT_FLAG_NOT_SELECTABLE`).
const BUNNY_UNIT_FLAGS: u32 = 0x0200_0002;
/// It takes up no room.
const BUNNY_RADIUS: f32 = 0.01;

/// An unselectable, unattackable, roomless unit `guid` of creature `entry` showing `display` at
/// `position`: a bunny, or a boss's object shown as a creature model (a troll ward).
pub(super) fn still_unit_create(guid: u64, entry: u32, display: u32, scale: f32, position: [f32; 3]) -> ObjectUpdate {
    let mut f = Fields::new();
    f.u64(field::OBJECT_GUID, guid)
        .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_UNIT)
        .u32(field::OBJECT_ENTRY, entry)
        .f32(field::OBJECT_SCALE_X, scale);
    f.u32(field::UNIT_HEALTH, 1)
        .u32(field::UNIT_MAXHEALTH, 1)
        .u32(field::UNIT_LEVEL, 1)
        .u32(field::UNIT_FACTIONTEMPLATE, PICKUP_FACTION)
        .u32(field::UNIT_FLAGS, BUNNY_UNIT_FLAGS);
    f.f32(field::UNIT_BOUNDINGRADIUS, BUNNY_RADIUS)
        .f32(field::UNIT_COMBATREACH, BUNNY_RADIUS)
        .u32(field::UNIT_DISPLAYID, display)
        .u32(field::UNIT_NATIVEDISPLAYID, display);
    ObjectUpdate::Create { guid, type_id: ObjectTypeId::Unit, living: Living { position, facing: 0.0, speeds: Speeds::DEFAULT }, fields: f, own: false }
}

impl Game {
    /// `spell` from `caster` plays at `at` on a bunny put there, which goes `linger` seconds later:
    /// the bunny's create, then the spell going off on it.
    pub(super) fn impact_at_spot(&mut self, caster: Unit, spell: u32, at: Vec2, linger: f32, all: &mut Vec<Packet>) {
        let Some(from) = self.guid_of(caster) else { return };
        self.next_bunny = self.next_bunny.wrapping_add(1) & 0xFF_FFFF;
        let guid = unit_guid(crate::entries::BUNNY, self.next_bunny);
        let position = [at.0, at.1, self.ground_z(at)];
        all.push(update_object(&[still_unit_create(guid, crate::entries::BUNNY, BUNNY_DISPLAY, 1.0, position)]));
        all.push(packets::spell_go(from, spell, &[guid], SpellTarget::Unit(guid), None));
        self.bunnies.push((guid, self.run.now + linger.max(0.0)));
    }

    /// Bunnies whose time is up go.
    pub(super) fn mirror_bunnies(&mut self, out: &mut Vec<Packet>) {
        let now = self.run.now;
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.bunnies).into_iter().partition(|(_, until)| now >= *until);
        self.bunnies = kept;
        out.extend(gone.into_iter().map(|(guid, _)| packets::destroy_object(guid)));
    }

    /// Every bunny goes (the field was cleared).
    pub(super) fn forget_bunnies(&mut self) -> Vec<Packet> {
        self.bunnies.drain(..).map(|(guid, _)| packets::destroy_object(guid)).collect()
    }
}
