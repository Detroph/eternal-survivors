//! A spell's lasting look on the ground: a dynamic object of the
//! spell, cast by a hero and sized to the area. A lasting area shows one while it lasts; a boss
//! marks the hero's spot with one until its hit lands.

use esurv_sim::game::enemy_ai::Vec2;

use super::Game;
use crate::packets;
use crate::update::{
    field, update_object, Fields, ObjectTypeId, ObjectUpdate, TYPE_DYNAMICOBJECT, TYPE_OBJECT,
};
use crate::wire::Packet;

/// `HIGHGUID_DYNAMICOBJECT`: dynamic-object guids are this | a number.
const HIGHGUID_DYNAMICOBJECT: u64 = 0xF100 << 48;
/// The bytes field of every look the game puts down.
const LOOK_BYTES: u32 = 1;
/// The game's own short looks are numbered from here, apart from the simulation's areas.
const BURST_LOOK_IDS: u32 = 0x8000_0000;

/// The radius some spells' looks are given, to size them to an area: these at 8, 10 or
/// 5 yards (their own radii in `Spell.dbc`). Every other look is sized by its `Spell.dbc`
/// radius too ([`Game::look_scale`]).
fn drawn_radius(spell: u32) -> Option<f32> {
    match spell {
        10 | 1510 | 1680 | 5740 | 6343 | 8122 | 16914 | 20549 | 26573 => Some(8.0),
        120 | 122 | 1160 | 1449 | 1949 | 11113 | 13810 | 15237 => Some(10.0),
        779 | 2120 => Some(5.0),
        _ => None,
    }
}

/// How much look `spell`, drawn `drawn` yards across (its own radius), is scaled to cover
/// `radius` yards; a look of unknown size keeps its own.
pub fn look_scale(drawn: Option<f32>, radius: f32) -> f32 {
    drawn
        .filter(|&d| d > 0.0 && radius > 0.0)
        .map_or(1.0, |d| radius / d)
}

/// The guid of the look numbered `id`.
pub fn ground_look_guid(id: u32) -> u64 {
    HIGHGUID_DYNAMICOBJECT | u64::from(id)
}

/// A look on the ground as the clients were last told.
pub(super) struct ShownLook {
    guid: u64,
    /// It goes at this run time, or when the simulation says.
    until: Option<f32>,
    create: ObjectUpdate,
    /// Who put it down and its spell, to put it down again moved or grown.
    caster: u64,
    spell: u32,
}

impl Game {
    /// How much look `spell` is scaled to cover `radius` yards: by the sizes in
    /// `drawn_radius`, else
    /// (every area look shows its true size) the spell's own radius in
    /// the player's `Spell.dbc`.
    pub(super) fn look_scale(&self, spell: u32, radius: f32) -> f32 {
        let drawn = drawn_radius(spell).or_else(|| self.data.as_ref()?.spell_radius(spell));
        look_scale(drawn, radius)
    }

    /// Puts look `spell`, cast by the unit with guid `caster`, on the ground: `radius` yards
    /// round `at`, until `until` (or until [`Game::ground_look_gone`]). Returns its create.
    pub(super) fn ground_look(
        &mut self,
        id: u32,
        caster: u64,
        spell: u32,
        at: Vec2,
        radius: f32,
        until: Option<f32>,
    ) -> Packet {
        let scale = self.look_scale(spell, radius);
        self.ground_look_scaled(id, caster, spell, at, radius, scale, until)
    }

    /// Puts a short look `spell` of the game's own, cast by the unit with guid `caster`, at `at`
    /// on the ground at its own size for `seconds` (a burst the client plays once). Returns its
    /// create.
    pub(super) fn burst_look(&mut self, caster: u64, spell: u32, at: Vec2, seconds: f32) -> Packet {
        self.burst_look_placed(caster, spell, at, seconds).1
    }

    /// [`Game::burst_look`], with the look's guid (for a cast it makes, as a caster on the
    /// ground).
    pub(super) fn burst_look_placed(&mut self, caster: u64, spell: u32, at: Vec2, seconds: f32) -> (u64, Packet) {
        // a number of its own, apart from the simulation's
        let id = (BURST_LOOK_IDS..).find(|id| !self.looks_shown.contains_key(id)).unwrap_or(BURST_LOOK_IDS);
        let until = Some(self.run.now + seconds);
        (ground_look_guid(id), self.ground_look_scaled(id, caster, spell, at, 0.0, 1.0, until))
    }

    /// A legendary moment's look at a spot: a short look of its own
    /// put down at `at` for `seconds`, sized to `radius` yards as an area look is, that casts
    /// `spell` on itself, so its cast and its ground model play there and not at the hero.
    /// Returns its create, its size and its cast.
    pub(super) fn spot_look(&mut self, caster: u64, spell: u32, at: Vec2, radius: f32, seconds: f32) -> Vec<Packet> {
        let id = (BURST_LOOK_IDS..).find(|id| !self.looks_shown.contains_key(id)).unwrap_or(BURST_LOOK_IDS);
        let scale = if radius > 0.0 { self.look_scale(spell, radius) } else { 1.0 };
        let until = Some(self.run.now + seconds);
        let create = self.ground_look_scaled(id, caster, spell, at, radius, scale, until);
        let guid = ground_look_guid(id);
        let mut out = vec![create];
        if (scale - 1.0).abs() > 0.01 {
            if let Some(me) = self.players.first().map(|p| p.guid()) {
                out.push(packets::addon_message(me, super::present::LOOK_SIZE_PREFIX, &format!("{guid:X} {spell} {scale:.3}")));
            }
        }
        out.push(packets::spell_go(guid, spell, &[], packets::SpellTarget::Caster, None));
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn ground_look_scaled(
        &mut self,
        id: u32,
        caster: u64,
        spell: u32,
        at: Vec2,
        radius: f32,
        scale: f32,
        until: Option<f32>,
    ) -> Packet {
        let guid = ground_look_guid(id);
        let position = [at.0, at.1, self.ground_z(at)];
        let mut f = Fields::new();
        f.u64(field::OBJECT_GUID, guid)
            .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_DYNAMICOBJECT)
            .u32(field::OBJECT_ENTRY, spell)
            .f32(field::OBJECT_SCALE_X, scale);
        f.u64(field::DYNAMICOBJECT_CASTER, caster)
            .u32(field::DYNAMICOBJECT_BYTES, LOOK_BYTES)
            .u32(field::DYNAMICOBJECT_SPELLID, spell)
            .f32(field::DYNAMICOBJECT_RADIUS, radius);
        for (k, v) in position.iter().enumerate() {
            f.f32(field::DYNAMICOBJECT_POS_X + k as u16, *v);
        }
        f.f32(field::DYNAMICOBJECT_FACING, 0.0);
        let create = ObjectUpdate::CreatePlaced {
            guid,
            type_id: ObjectTypeId::DynamicObject,
            position,
            facing: 0.0,
            fields: f,
        };
        let packet = update_object(std::slice::from_ref(&create));
        self.looks_shown.insert(
            id,
            ShownLook {
                guid,
                until,
                create,
                caster,
                spell,
            },
        );
        packet
    }

    /// Look `id` (a lasting area that moves with its hero or grows) is put down again at `at`,
    /// `radius` yards round, keeping its spell, caster and time; nothing when it is not shown.
    pub(super) fn ground_look_changed(&mut self, id: u32, at: Vec2, radius: f32) -> Vec<Packet> {
        let Some(old) = self.looks_shown.remove(&id) else {
            return Vec::new();
        };
        let gone = packets::destroy_object(old.guid);
        let again = self.ground_look(id, old.caster, old.spell, at, radius, old.until);
        vec![gone, again]
    }

    /// Takes look `id` off the ground.
    pub(super) fn ground_look_gone(&mut self, id: u32) -> Option<Packet> {
        self.looks_shown
            .remove(&id)
            .map(|l| packets::destroy_object(l.guid))
    }

    /// Looks whose time is up go.
    pub(super) fn expire_ground_looks(&mut self, out: &mut Vec<Packet>) {
        let now = self.run.now;
        let ended: Vec<u32> = self
            .looks_shown
            .iter()
            .filter(|(_, l)| l.until.is_some_and(|t| now >= t))
            .map(|(id, _)| *id)
            .collect();
        for id in ended {
            out.extend(self.ground_look_gone(id));
        }
    }

    /// Every look on the ground goes: the field is cleared.
    pub(super) fn clear_ground_looks(&mut self) -> Vec<Packet> {
        self.looks_shown
            .drain()
            .map(|(_, l)| packets::destroy_object(l.guid))
            .collect()
    }

    /// Every look on the ground, as creates (for a player who joins).
    pub(super) fn ground_looks_now(&self) -> Vec<ObjectUpdate> {
        self.looks_shown
            .values()
            .map(|l| l.create.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_drawn_at_a_known_size_are_scaled_to_the_area() {
        let scale = |spell, radius| look_scale(drawn_radius(spell), radius);
        assert_eq!(scale(10, 9.0), 9.0 / 8.0);
        assert_eq!(scale(122, 5.0), 0.5);
        assert_eq!(scale(2120, 8.0), 1.6);
        assert_eq!(scale(42, 8.0), 1.0, "one of unknown size keeps its own");
        assert_eq!(ground_look_guid(7), 0xF100_0000_0000_0007);
    }
}
