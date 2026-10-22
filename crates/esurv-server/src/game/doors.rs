//! A zone's doors and props (zones.md "Walls and doors"): each door
//! is the map's own door object, shown shut where it stands until the run clock opens it
//! (`Event::DoorOpened`) or a boss's script opens or shuts it (`Event::DoorShut`), swung as it
//! stands; the sim keeps it a wall while it is shut. A prop is a
//! map object of our own (the round table plugging the Sunken Temple's pit), solid to the client where its
//! model is.

use esurv_content::{Content, Door};

use super::pickups::{gameobject_guid, PICKUP_FACTION};
use super::Game;
use crate::packets::GameObjectInfo;
use crate::entries::ZONE_PROPS_FROM;
use crate::update::{field, Fields, ObjectTypeId, ObjectUpdate, TYPE_GAMEOBJECT, TYPE_OBJECT};

/// `GAMEOBJECT_TYPE_GENERIC`.
const GO_TYPE_GENERIC: u32 = 5;
/// A prop stands in its ready state.
const PROP_READY: u32 = 1;

/// `GAMEOBJECT_TYPE_DOOR`.
const GO_TYPE_DOOR: u32 = 0;
/// A door stands shut in its ready state and open in its active one.
const DOOR_SHUT: u32 = 1;
const DOOR_OPEN: u32 = 0;
/// Nobody clicks it open: only the run clock does.
const GO_FLAG_NO_INTERACT: u32 = 0x10;

/// The guid of the zone's door number `d`.
fn door_guid(door: &Door, d: usize) -> u64 {
    gameobject_guid(door.object, d as u32 + 1)
}

/// The game-object entry of a prop that looks like `display_id`.
fn prop_entry(display_id: u32) -> u32 {
    ZONE_PROPS_FROM + display_id
}

/// What the client's game-object query hears of a door or prop entry of any zone.
pub fn door_info(content: &Content, entry: u32) -> Option<GameObjectInfo> {
    if let Some(door) = content.zones.iter().flat_map(|z| &z.doors).find(|d| d.object == entry) {
        return Some(GameObjectInfo { type_id: GO_TYPE_DOOR, display_id: door.display_id, name: door.name.clone(), data: [0; 24] });
    }
    let prop = content.zones.iter().flat_map(|z| &z.props).find(|p| prop_entry(p.display_id) == entry)?;
    Some(GameObjectInfo { type_id: GO_TYPE_GENERIC, display_id: prop.display_id, name: prop.name.clone(), data: [0; 24] })
}

impl Game {
    /// The create of the zone's door number `d`, shut or open as it stands now.
    fn door_object(&self, d: usize) -> Option<ObjectUpdate> {
        let door = self.run.content.zones[self.run.zone].doors.get(d)?;
        let guid = door_guid(door, d);
        let at = door.at;
        let mut f = Fields::new();
        f.u64(field::OBJECT_GUID, guid)
            .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_GAMEOBJECT)
            .u32(field::OBJECT_ENTRY, door.object)
            .f32(field::OBJECT_SCALE_X, 1.0);
        f.u32(field::GAMEOBJECT_DISPLAYID, door.display_id)
            .u32(field::GAMEOBJECT_FLAGS, GO_FLAG_NO_INTERACT);
        // a turn about the vertical axis: the rotation's z and w
        f.f32(field::GAMEOBJECT_ROTATION + 2, (at.facing / 2.0).sin())
            .f32(field::GAMEOBJECT_ROTATION + 3, (at.facing / 2.0).cos());
        let open = self.run.doors_open.get(d).copied().unwrap_or(false);
        f.u32(field::GAMEOBJECT_STATE, if open { DOOR_OPEN } else { DOOR_SHUT });
        for (k, v) in [at.x, at.y, at.z].iter().enumerate() {
            f.f32(field::GAMEOBJECT_POS_X + k as u16, *v);
        }
        f.f32(field::GAMEOBJECT_FACING, at.facing)
            .u32(field::GAMEOBJECT_FACTION, PICKUP_FACTION)
            .u32(field::GAMEOBJECT_TYPE_ID, GO_TYPE_DOOR);
        Some(ObjectUpdate::CreatePlaced {
            guid,
            type_id: ObjectTypeId::GameObject,
            position: [at.x, at.y, at.z],
            facing: at.facing,
            fields: f,
        })
    }

    /// Every door of the zone as it stands now, and its props (for a player who arrives).
    pub(super) fn doors_now(&self) -> Vec<ObjectUpdate> {
        let props = self.run.content.zones[self.run.zone].props.iter().enumerate().map(|(k, p)| prop_object(p, k));
        (0..self.run.doors_open.len()).filter_map(|d| self.door_object(d)).chain(props).collect()
    }

    /// Door `d` swung open, or shut again for a fresh run.
    pub(super) fn door_state(&self, d: usize) -> Option<ObjectUpdate> {
        let door = self.run.content.zones[self.run.zone].doors.get(d)?;
        let open = self.run.doors_open.get(d).copied().unwrap_or(false);
        let mut f = Fields::new();
        f.u32(field::GAMEOBJECT_STATE, if open { DOOR_OPEN } else { DOOR_SHUT });
        Some(ObjectUpdate::Values { guid: door_guid(door, d), fields: f })
    }
}

/// The create of prop `p`, number `k` of its zone.
fn prop_object(p: &esurv_content::Prop, k: usize) -> ObjectUpdate {
    let entry = prop_entry(p.display_id);
    let guid = gameobject_guid(entry, k as u32 + 1);
    let at = p.at;
    let mut f = Fields::new();
    f.u64(field::OBJECT_GUID, guid)
        .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_GAMEOBJECT)
        .u32(field::OBJECT_ENTRY, entry)
        .f32(field::OBJECT_SCALE_X, p.scale);
    f.u32(field::GAMEOBJECT_DISPLAYID, p.display_id)
        .u32(field::GAMEOBJECT_FLAGS, GO_FLAG_NO_INTERACT);
    f.f32(field::GAMEOBJECT_ROTATION + 2, (at.facing / 2.0).sin())
        .f32(field::GAMEOBJECT_ROTATION + 3, (at.facing / 2.0).cos());
    f.u32(field::GAMEOBJECT_STATE, PROP_READY);
    for (k, v) in [at.x, at.y, at.z].iter().enumerate() {
        f.f32(field::GAMEOBJECT_POS_X + k as u16, *v);
    }
    f.f32(field::GAMEOBJECT_FACING, at.facing)
        .u32(field::GAMEOBJECT_FACTION, PICKUP_FACTION)
        .u32(field::GAMEOBJECT_TYPE_ID, GO_TYPE_GENERIC);
    ObjectUpdate::CreatePlaced { guid, type_id: ObjectTypeId::GameObject, position: [at.x, at.y, at.z], facing: at.facing, fields: f }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The client's game-object query hears of a zone's door and its props by their entries.
    #[test]
    fn a_door_and_a_prop_answer_the_game_object_query() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).unwrap();
        let door = door_info(&content, 104_591).expect("the cathedral's door");
        assert_eq!((door.type_id, door.display_id), (GO_TYPE_DOOR, 1207));
        let table = door_info(&content, prop_entry(6508)).expect("the Sunken Temple's table");
        assert_eq!((table.type_id, table.name.as_str()), (GO_TYPE_GENERIC, "Round Table"));
        assert!(door_info(&content, prop_entry(1)).is_none());
    }
}
