//! Totems: a small unit on the hero's side, made by the hero,
//! standing where it was placed; when its time is up it just goes (no death). A race with a
//! totem look of its own (`races.ron` `totem_look`) shows them in a
//! composed look the client builds from the stock totem and the model planted in it.

use std::collections::HashSet;

use esurv_sim::game::casting::Totem;
use esurv_sim::game::run::Unit;

use super::{unit_guid, Game};
use crate::packets::{self, CreatureInfo};
use crate::update::{
    field, Fields, Living, ObjectTypeId, ObjectUpdate, Speeds, TYPE_OBJECT, TYPE_UNIT,
};
use crate::wire::Packet;
use esurv_content::looks::{totem_look, MAGMA_TOTEM_DISPLAY, TOTEM_DISPLAY};

/// Every totem is this creature.
pub const TOTEM_ENTRY: u32 = crate::entries::TOTEM;
const MAGMA_TOTEM: &str = "magma_totem";
const TOTEM_HEALTH: u32 = 5;
/// It cannot be attacked (`UNIT_FLAG_NON_ATTACKABLE`).
const TOTEM_UNIT_FLAGS: u32 = 0x2;
const TOTEM_BOUNDING_RADIUS: f32 = 0.4;
const TOTEM_COMBAT_REACH: f32 = 1.0;
/// `CreatureType.dbc`: totem.
const CREATURE_TYPE_TOTEM: u32 = 11;

/// What the creature query tells about a totem.
pub fn totem_info() -> CreatureInfo {
    CreatureInfo {
        name: "Totem".into(),
        subname: String::new(),
        creature_type: CREATURE_TYPE_TOTEM,
        rank: crate::packets::RANK_NORMAL,
        display_id: MAGMA_TOTEM_DISPLAY,
    }
}

/// The stock look of a totem placed by ability `spell`: every totem's, `magma_totem`'s its own.
pub fn totem_display(spell: &str) -> u32 {
    if spell == MAGMA_TOTEM {
        MAGMA_TOTEM_DISPLAY
    } else {
        TOTEM_DISPLAY
    }
}

impl Game {
    /// The create of totem `t`, and its guid.
    fn totem_object(&self, t: &Totem) -> Option<(ObjectUpdate, u64)> {
        let owner = self.run.heroes.get(t.hero)?;
        let made_by = self.guid_of(Unit::Hero(owner.id)).unwrap_or(0);
        let guid = unit_guid(TOTEM_ENTRY, t.id);
        let display = totem_look(&self.run.content.races[owner.race], totem_display(&t.spell));
        let mut f = Fields::new();
        f.u64(field::OBJECT_GUID, guid)
            .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_UNIT)
            .u32(field::OBJECT_ENTRY, TOTEM_ENTRY)
            .f32(field::OBJECT_SCALE_X, 1.0);
        f.u64(field::UNIT_SUMMONEDBY, made_by)
            .u64(field::UNIT_CREATEDBY, made_by);
        f.u32(field::UNIT_HEALTH, TOTEM_HEALTH)
            .u32(field::UNIT_MAXHEALTH, TOTEM_HEALTH)
            .u32(field::UNIT_LEVEL, owner.progress.level)
            .u32(
                field::UNIT_FACTIONTEMPLATE,
                self.run.content.races[owner.race].faction_template,
            )
            .u32(field::UNIT_FLAGS, TOTEM_UNIT_FLAGS);
        f.f32(field::UNIT_BOUNDINGRADIUS, TOTEM_BOUNDING_RADIUS)
            .f32(field::UNIT_COMBATREACH, TOTEM_COMBAT_REACH)
            .u32(field::UNIT_DISPLAYID, display)
            .u32(field::UNIT_NATIVEDISPLAYID, display);
        let living = Living {
            position: [t.at.0, t.at.1, self.ground_z(t.at)],
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
        Some((create, guid))
    }

    /// Every totem the clients already know of, as creates (for a player who joins).
    pub(super) fn totems_now(&self) -> Vec<ObjectUpdate> {
        self.run
            .totems
            .iter()
            .filter(|t| self.totems_shown.contains_key(&t.id))
            .filter_map(|t| Some(self.totem_object(t)?.0))
            .collect()
    }

    /// New totems appear (their creates go to `creates`); those whose time is up go.
    pub(super) fn mirror_totems(
        &mut self,
        creates: &mut Vec<ObjectUpdate>,
        gone: &mut Vec<Packet>,
    ) {
        let live: HashSet<u32> = self.run.totems.iter().map(|t| t.id).collect();
        let ended: Vec<u32> = self
            .totems_shown
            .keys()
            .copied()
            .filter(|id| !live.contains(id))
            .collect();
        for id in ended {
            if let Some(guid) = self.totems_shown.remove(&id) {
                gone.push(packets::destroy_object(guid));
            }
        }
        for k in 0..self.run.totems.len() {
            let id = self.run.totems[k].id;
            if self.totems_shown.contains_key(&id) {
                continue;
            }
            if let Some((create, guid)) = self.totem_object(&self.run.totems[k]) {
                creates.push(create);
                self.totems_shown.insert(id, guid);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magma_totem_has_its_own_look() {
        assert_eq!(totem_display("magma_totem"), 4589);
        assert_eq!(totem_display("searing_totem"), 4587);
        assert_eq!(totem_info().display_id, 4589);
    }
}
