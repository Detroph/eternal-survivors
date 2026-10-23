//! Field pickups as world objects: an experience orb is a small
//! wisp (a unit that cannot be targeted), everything else a game object; a magnet stands in a
//! two-part beam of light. Orbs fly to the hero with one monster move; a taken pickup is
//! destroyed.

use std::collections::HashSet;

use esurv_content::{Faction, FactionDisplay};
use esurv_sim::game::loot::{orb_size, OrbSize, Pickup};
use esurv_sim::game::pickups::FieldPickup;

use super::{unit_guid, Game};
use crate::packets;
use crate::update::{
    field, Fields, Living, ObjectTypeId, ObjectUpdate, Speeds, TYPE_GAMEOBJECT, TYPE_OBJECT,
    TYPE_UNIT,
};
use crate::entries;
use crate::wire::Packet;

/// What a pickup looks like.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub entry: u32,
    pub display: u32,
    pub scale: f32,
    /// The game-object type (`None`: a unit, the experience orb).
    pub go_type: Option<u32>,
    /// `GAMEOBJECT_FLAGS`.
    pub flags: u32,
    pub name: &'static str,
}

/// Game-object types used.
const GO_TYPE_CHEST: u32 = 3;
const GO_TYPE_GENERIC: u32 = 5;
/// A game object the player cannot use (the beam of light).
const GO_FLAG_NO_INTERACT: u32 = 0x10;
/// Every pickup belongs to faction 35 (friendly to all).
pub const PICKUP_FACTION: u32 = 35;
/// The orb: an untargetable unit (`UNIT_FLAG_NON_ATTACKABLE | UNIT_FLAG_NOT_SELECTABLE`).
pub const ORB_UNIT_FLAGS: u32 = 0x0200_0002;
pub const ORB_ENTRY: u32 = entries::ORB;
/// The orb's reach and size for collisions.
pub const ORB_RADIUS: f32 = 0.1;

/// An experience orb's look by its size (`balance.ron` sizes it by value): a white wisp, small or
/// medium; a large orb is red.
pub fn orb_look(size: OrbSize) -> Look {
    let (display, scale) = match size {
        OrbSize::Small => (1825, 0.22),
        OrbSize::Medium => (1825, 0.35),
        OrbSize::Large => (16587, 0.9),
    };
    Look {
        entry: ORB_ENTRY,
        display,
        scale,
        go_type: None,
        flags: 0,
        name: "Experience",
    }
}

/// The look of a pickup of `kind`; `orb` sizes an experience orb and is ignored otherwise;
/// `standards` are the battle standards' looks (`balance.ron` `battle_standard.display`).
pub fn look(kind: Pickup, orb: OrbSize, standards: FactionDisplay) -> Look {
    let go = |entry, display, scale, go_type, name| Look {
        entry,
        display,
        scale,
        go_type: Some(go_type),
        flags: 0,
        name,
    };
    match kind {
        Pickup::Experience => orb_look(orb),
        Pickup::Gold => go(entries::GOLD, 2614, 0.6, GO_TYPE_GENERIC, "Gold"),
        Pickup::BattlegroundCache => go(entries::BATTLEGROUND_CACHE, 259, 1.0, GO_TYPE_CHEST, "Battleground Cache"),
        // free to open, and its name says so
        Pickup::RareCache => go(entries::RARE_CACHE, 259, 1.2, GO_TYPE_CHEST, "Rare Cache (Free)"),
        Pickup::SpeedRune => go(entries::SPEED_RUNE, 5931, 1.0, GO_TYPE_GENERIC, "Speed Rune"),
        Pickup::BerserkerRune => go(entries::BERSERKER_RUNE, 5995, 1.0, GO_TYPE_GENERIC, "Berserker Rune"),
        Pickup::RestorationRune => go(entries::RESTORATION_RUNE, 5991, 1.0, GO_TYPE_GENERIC, "Restoration Rune"),
        Pickup::Magnet => go(entries::MAGNET, 342, 1.3, GO_TYPE_GENERIC, "Magnet"),
        // its faction's Alterac battle standard, a creature model planted in the ground: a unit
        // that cannot be targeted, as the orb
        Pickup::BattleStandard(f) => Look {
            entry: entries::BATTLE_STANDARD,
            display: standards.of(f),
            scale: STANDARD_SCALE,
            go_type: None,
            flags: 0,
            name: STANDARD_NAME,
        },
    }
}

/// The two objects of a magnet's beam of light, standing where the magnet is.
pub const MAGNET_BEAM: [Look; 2] = [
    Look {
        entry: entries::MAGNET_BEAM_LIGHT,
        display: 3271,
        scale: 1.0,
        go_type: Some(GO_TYPE_GENERIC),
        flags: GO_FLAG_NO_INTERACT,
        name: "Magnet",
    },
    Look {
        entry: entries::MAGNET_BEAM_GLOW,
        display: 2232,
        scale: 1.6,
        go_type: Some(GO_TYPE_GENERIC),
        flags: GO_FLAG_NO_INTERACT,
        name: "Magnet",
    },
];

/// A battle standard is drawn this size.
pub const STANDARD_SCALE: f32 = 1.0;
/// The soft glow under a battle standard, in its faction's colour: a short blue aura for the
/// Alliance, a short red one for the Horde (`GameObjectDisplayInfo.dbc` 263 `AuraBlueShort`,
/// 1308 `AuraRedShort`), drawn this size.
pub const STANDARD_GLOW: FactionDisplay = FactionDisplay { alliance: 263, horde: 1308 };
pub const STANDARD_GLOW_SCALE: f32 = 1.0;
pub const STANDARD_NAME: &str = "Warband Standard";

/// The glow under a battle standard, of look `display`: a game object of an entry of its own per
/// look (`entries::BATTLE_STANDARDS_FROM`), so the game-object query answers it.
pub fn standard_look(display: u32, scale: f32, flags: u32) -> Look {
    Look {
        entry: entries::BATTLE_STANDARDS_FROM + display % (entries::BATTLE_STANDARDS_END - entries::BATTLE_STANDARDS_FROM),
        display,
        scale,
        go_type: Some(GO_TYPE_GENERIC),
        flags,
        name: STANDARD_NAME,
    }
}

/// The glow under a battle standard of `faction`.
pub fn standard_glow(faction: Faction) -> Look {
    standard_look(STANDARD_GLOW.of(faction), STANDARD_GLOW_SCALE, GO_FLAG_NO_INTERACT)
}

/// The beam of light over a battle standard, in its faction's colour, each part (look, size): a
/// very tall and a tall aura (`GameObjectDisplayInfo.dbc` 2232 `AuraBlueVeryTall` and 298
/// `AuraBlueTall` for the Alliance, 1311 `AuraRedVeryTall` and 4831 `AuraRedTall` for the Horde),
/// the very tall one as large as the magnet's beam; without the magnet's sparkle.
pub const STANDARD_BEAM: [(FactionDisplay, f32); 2] =
    [(FactionDisplay { alliance: 2232, horde: 1311 }, 1.6), (FactionDisplay { alliance: 298, horde: 4831 }, 1.0)];

/// The beam of light over a battle standard of `faction`.
pub fn standard_beam(faction: Faction) -> [Look; 2] {
    STANDARD_BEAM.map(|(display, scale)| standard_look(display.of(faction), scale, GO_FLAG_NO_INTERACT))
}

/// Every game-object look, for the client's game-object query.
pub fn gameobject_look(entry: u32) -> Option<Look> {
    if (entries::BATTLE_STANDARDS_FROM..entries::BATTLE_STANDARDS_END).contains(&entry) {
        let display = entry - entries::BATTLE_STANDARDS_FROM;
        let beam = STANDARD_BEAM.iter().find(|(d, _)| d.alliance == display || d.horde == display);
        return Some(standard_look(display, beam.map_or(STANDARD_GLOW_SCALE, |b| b.1), GO_FLAG_NO_INTERACT));
    }
    let kinds = [
        Pickup::Gold,
        Pickup::BattlegroundCache,
        Pickup::RareCache,
        Pickup::SpeedRune,
        Pickup::BerserkerRune,
        Pickup::RestorationRune,
        Pickup::Magnet,
    ];
    kinds
        .into_iter()
        .map(|k| look(k, OrbSize::Small, FactionDisplay { alliance: 0, horde: 0 }))
        .chain(MAGNET_BEAM)
        .find(|l| l.entry == entry)
}

/// A facing for an object that has none: spread evenly by its id. Ours (drawing one
/// from the run's random numbers would change the run here).
pub fn facing_for(id: u32) -> f32 {
    (id.wrapping_mul(2_654_435_761) as f32 / u32::MAX as f32) * std::f32::consts::TAU
}

/// `HIGHGUID_GAMEOBJECT`: game-object guids are this | entry << 24 | a number.
const HIGHGUID_GAMEOBJECT: u64 = 0xF110 << 48;
/// A game object stands ready (its idle state).
const GO_STATE_READY: u32 = 1;

/// The guid of game object `entry` with number `id`.
pub fn gameobject_guid(entry: u32, id: u32) -> u64 {
    HIGHGUID_GAMEOBJECT | (u64::from(entry) & 0xFF_FFFF) << 24 | (u64::from(id) & 0xFF_FFFF)
}

/// A pickup as the clients were last told.
pub(super) struct ShownPickup {
    guid: u64,
    /// A magnet's beam of light, a battle standard's glow and beam.
    beam: Vec<u64>,
    flying: bool,
}

impl Game {
    /// The create of a game object `look` numbered `id` at `at`.
    fn placed_object(&self, look: &Look, id: u32, at: (f32, f32)) -> (ObjectUpdate, u64) {
        let guid = gameobject_guid(look.entry, id);
        let facing = facing_for(id ^ look.entry);
        let position = [at.0, at.1, self.ground_z(at)];
        let mut f = Fields::new();
        f.u64(field::OBJECT_GUID, guid)
            .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_GAMEOBJECT)
            .u32(field::OBJECT_ENTRY, look.entry)
            .f32(field::OBJECT_SCALE_X, look.scale);
        f.u32(field::GAMEOBJECT_DISPLAYID, look.display)
            .u32(field::GAMEOBJECT_FLAGS, look.flags);
        // a turn about the vertical axis: the rotation's z and w
        f.f32(field::GAMEOBJECT_ROTATION + 2, (facing / 2.0).sin())
            .f32(field::GAMEOBJECT_ROTATION + 3, (facing / 2.0).cos());
        f.u32(field::GAMEOBJECT_STATE, GO_STATE_READY);
        for (k, v) in position.iter().enumerate() {
            f.f32(field::GAMEOBJECT_POS_X + k as u16, *v);
        }
        f.f32(field::GAMEOBJECT_FACING, facing)
            .u32(field::GAMEOBJECT_FACTION, PICKUP_FACTION)
            .u32(
                field::GAMEOBJECT_TYPE_ID,
                look.go_type.unwrap_or(GO_TYPE_GENERIC),
            );
        let create = ObjectUpdate::CreatePlaced {
            guid,
            type_id: ObjectTypeId::GameObject,
            position,
            facing,
            fields: f,
        };
        (create, guid)
    }

    /// The creates of pickup `p` (a magnet with its beam), and the guids they use.
    fn pickup_objects(&self, p: &FieldPickup) -> (Vec<ObjectUpdate>, u64, Vec<u64>) {
        let look = look(
            p.kind,
            orb_size(p.value, &self.run.content.balance.experience),
            self.run.content.balance.battle_standard.display,
        );
        if look.go_type.is_none() {
            let guid = unit_guid(look.entry, p.id);
            let mut f = Fields::new();
            f.u64(field::OBJECT_GUID, guid)
                .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_UNIT)
                .u32(field::OBJECT_ENTRY, look.entry)
                .f32(field::OBJECT_SCALE_X, look.scale);
            f.u32(field::UNIT_HEALTH, 1)
                .u32(field::UNIT_MAXHEALTH, 1)
                .u32(field::UNIT_LEVEL, 1)
                .u32(field::UNIT_FACTIONTEMPLATE, PICKUP_FACTION)
                .u32(field::UNIT_FLAGS, ORB_UNIT_FLAGS);
            f.f32(field::UNIT_BOUNDINGRADIUS, ORB_RADIUS)
                .f32(field::UNIT_COMBATREACH, ORB_RADIUS)
                .u32(field::UNIT_DISPLAYID, look.display)
                .u32(field::UNIT_NATIVEDISPLAYID, look.display);
            let living = Living {
                position: [p.at.0, p.at.1, self.ground_z(p.at)],
                facing: 0.0,
                speeds: Speeds::DEFAULT,
            };
            let create = ObjectUpdate::Create {
                guid,
                type_id: ObjectTypeId::Unit,
                living,
                fields: f,
                own: false,
            };
            let mut creates = vec![create];
            let mut glow = Vec::new();
            // a battle standard stands in its glow, under its beam of light
            if let Pickup::BattleStandard(f) = p.kind {
                for part in std::iter::once(standard_glow(f)).chain(standard_beam(f)) {
                    let (c, g) = self.placed_object(&part, p.id, p.at);
                    creates.push(c);
                    glow.push(g);
                }
            }
            return (creates, guid, glow);
        }
        let (create, guid) = self.placed_object(&look, p.id, p.at);
        let mut creates = vec![create];
        let mut beam = Vec::new();
        let parts: Vec<Look> = match p.kind {
            Pickup::Magnet => MAGNET_BEAM.to_vec(),
            _ => Vec::new(),
        };
        for part in &parts {
            let (c, g) = self.placed_object(part, p.id, p.at);
            creates.push(c);
            beam.push(g);
        }
        (creates, guid, beam)
    }

    /// Every pickup the clients already know of, as creates (for a player who joins).
    pub(super) fn pickups_now(&self) -> Vec<ObjectUpdate> {
        self.run
            .pickups
            .iter()
            .filter(|p| self.pickups_shown.contains_key(&p.id))
            .flat_map(|p| self.pickup_objects(p).0)
            .collect()
    }

    /// Every pickup the clients know of goes (a magnet with its beam): the field is cleared.
    pub(super) fn clear_pickups(&mut self) -> Vec<Packet> {
        self.pickups_shown
            .drain()
            .flat_map(|(_, s)| std::iter::once(s.guid).chain(s.beam))
            .map(packets::destroy_object)
            .collect()
    }

    /// An orb on its way to the hero: one move from where it lies to where the hero is now.
    fn orb_flight(&mut self, p: &FieldPickup, guid: u64) -> Option<Packet> {
        let (h, arrival) = p.flying?;
        let hero = self.run.heroes.get(h)?;
        let end = [hero.pos.0, hero.pos.1, hero.height];
        let start = [p.at.0, p.at.1, self.ground_z(p.at)];
        let ms = (((arrival - self.run.now) * 1000.0) as u32).max(1);
        let spline = self.next_spline;
        self.next_spline = self.next_spline.wrapping_add(1);
        Some(packets::monster_move(guid, start, spline, &[end], ms, true))
    }

    /// New pickups appear, orbs that start flying move, and taken ones go.
    pub(super) fn mirror_pickups(
        &mut self,
        objects: &mut Vec<ObjectUpdate>,
        moves: &mut Vec<Packet>,
        gone: &mut Vec<Packet>,
    ) {
        let live: HashSet<u32> = self.run.pickups.iter().map(|p| p.id).collect();
        let taken: Vec<u32> = self
            .pickups_shown
            .keys()
            .copied()
            .filter(|id| !live.contains(id))
            .collect();
        for id in taken {
            if let Some(s) = self.pickups_shown.remove(&id) {
                gone.push(packets::destroy_object(s.guid));
                gone.extend(s.beam.into_iter().map(packets::destroy_object));
            }
        }
        for k in 0..self.run.pickups.len() {
            let p = self.run.pickups[k].clone();
            let flying = p.flying.is_some();
            let starts_flying = match self.pickups_shown.get_mut(&p.id) {
                None => {
                    let (creates, guid, beam) = self.pickup_objects(&p);
                    objects.extend(creates);
                    // a battle standard is planted with its faction's Battle Standard look, and
                    // pings the minimap where it lies
                    if let Pickup::BattleStandard(f) = p.kind {
                        let look = self.run.content.balance.battle_standard.planted_look.of(f);
                        if look != 0 {
                            moves.push(packets::spell_go(guid, look, &[guid], packets::SpellTarget::Unit(guid), None));
                        }
                        moves.push(packets::minimap_ping(guid, p.at));
                    }
                    self.pickups_shown
                        .insert(p.id, ShownPickup { guid, beam, flying });
                    flying
                }
                Some(s) => {
                    let starts = flying && !s.flying;
                    s.flying = flying;
                    starts
                }
            };
            // only orbs are seen flying; gold just goes when it arrives
            if starts_flying && p.kind == Pickup::Experience {
                let guid = self.pickups_shown[&p.id].guid;
                moves.extend(self.orb_flight(&p, guid));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbs_grow_with_their_value() {
        let r = esurv_content::ExperienceRules::default();
        let at = |v| {
            let l = orb_look(orb_size(v, &r));
            (l.display, l.scale)
        };
        assert_eq!([at(100), at(199)], [(1825, 0.22); 2]);
        assert_eq!([at(200), at(999)], [(1825, 0.35); 2]);
        assert_eq!([at(1000), at(2400)], [(16587, 0.9); 2]);
    }

    #[test]
    fn every_game_object_answers_its_query() {
        for entry in entries::GOLD..=entries::RARE_CACHE {
            assert!(gameobject_look(entry).is_some(), "{entry}");
        }
        assert_eq!(gameobject_look(ORB_ENTRY), None, "the orb is a unit");
    }
}
