//! `SMSG_UPDATE_OBJECT`: creating objects on the client (with a movement block and their update
//! fields), changing fields, and the 1.12.1 update-field layout this needs.

use std::collections::BTreeMap;

use crate::wire::{Body, Packet};

/// Update-field indices of 1.12.1 (in 4-byte words).
pub mod field {
    pub const OBJECT_GUID: u16 = 0;
    pub const OBJECT_TYPE: u16 = 2;
    pub const OBJECT_ENTRY: u16 = 3;
    pub const OBJECT_SCALE_X: u16 = 4;

    /// Who summoned and who made the unit (guids, 2 words each).
    pub const UNIT_SUMMONEDBY: u16 = 12;
    pub const UNIT_CREATEDBY: u16 = 14;
    pub const UNIT_TARGET: u16 = 16;
    /// What a channel is aimed at (a guid, 2 words), and the channelled spell.
    pub const UNIT_CHANNEL_OBJECT: u16 = 20;
    pub const UNIT_HEALTH: u16 = 22;
    /// Mana, rage, focus, energy, happiness follow.
    pub const UNIT_POWER1: u16 = 23;
    pub const UNIT_MAXHEALTH: u16 = 28;
    pub const UNIT_MAXPOWER1: u16 = 29;
    pub const UNIT_LEVEL: u16 = 34;
    pub const UNIT_FACTIONTEMPLATE: u16 = 35;
    /// race | class << 8 | gender << 16 | power type << 24.
    pub const UNIT_BYTES_0: u16 = 36;
    /// A creature's weapon looks (`ItemDisplayInfo.dbc`), 37 to 39: main hand, off hand, ranged.
    pub const UNIT_VIRTUAL_ITEM_SLOT_DISPLAY: u16 = 37;
    /// Two words per virtual item slot, 40 to 45: class | subclass << 8 | material << 16 |
    /// inventory type << 24, then its sheath type.
    pub const UNIT_VIRTUAL_ITEM_INFO: u16 = 40;
    pub const UNIT_FLAGS: u16 = 46;
    /// 48 aura slots (spell ids), buffs 0-31.
    pub const UNIT_AURA: u16 = 47;
    /// A 4-bit flags nibble per aura slot, 8 to a word.
    pub const UNIT_AURAFLAGS: u16 = 95;
    /// A level byte per aura slot, 4 to a word.
    pub const UNIT_AURALEVELS: u16 = 101;
    /// A byte per aura slot (stacks - 1), 4 to a word.
    pub const UNIT_AURAAPPLICATIONS: u16 = 113;
    pub const UNIT_BASEATTACKTIME: u16 = 126;
    pub const UNIT_BOUNDINGRADIUS: u16 = 129;
    pub const UNIT_COMBATREACH: u16 = 130;
    pub const UNIT_DISPLAYID: u16 = 131;
    pub const UNIT_NATIVEDISPLAYID: u16 = 132;
    pub const UNIT_MINDAMAGE: u16 = 134;
    pub const UNIT_MAXDAMAGE: u16 = 135;
    pub const UNIT_BYTES_1: u16 = 138;
    pub const UNIT_DYNAMIC_FLAGS: u16 = 143;
    pub const UNIT_CHANNEL_SPELL: u16 = 144;
    pub const UNIT_MOD_CAST_SPEED: u16 = 145;
    pub const UNIT_BASE_HEALTH: u16 = 163;
    /// sheath state (0 put away, 1 melee drawn, 2 ranged drawn) | ...
    pub const UNIT_BYTES_2: u16 = 164;

    pub const GAMEOBJECT_CREATED_BY: u16 = 6;
    pub const GAMEOBJECT_DISPLAYID: u16 = 8;
    pub const GAMEOBJECT_FLAGS: u16 = 9;
    /// The rotation quaternion (x, y, z, w floats), 10 to 13.
    pub const GAMEOBJECT_ROTATION: u16 = 10;
    /// 1 = ready (closed / idle).
    pub const GAMEOBJECT_STATE: u16 = 14;
    pub const GAMEOBJECT_POS_X: u16 = 15;
    pub const GAMEOBJECT_FACING: u16 = 18;
    pub const GAMEOBJECT_DYN_FLAGS: u16 = 19;
    pub const GAMEOBJECT_FACTION: u16 = 20;
    pub const GAMEOBJECT_TYPE_ID: u16 = 21;
    pub const GAMEOBJECT_LEVEL: u16 = 22;

    /// The caster's guid (2 words).
    pub const DYNAMICOBJECT_CASTER: u16 = 6;
    pub const DYNAMICOBJECT_BYTES: u16 = 8;
    pub const DYNAMICOBJECT_SPELLID: u16 = 9;
    pub const DYNAMICOBJECT_RADIUS: u16 = 10;
    pub const DYNAMICOBJECT_POS_X: u16 = 11;
    pub const DYNAMICOBJECT_FACING: u16 = 14;

    /// Whose the item is, and what holds it (guids, 2 words each).
    pub const ITEM_OWNER: u16 = 6;
    pub const ITEM_CONTAINED: u16 = 8;
    pub const ITEM_STACK_COUNT: u16 = 14;

    pub const PLAYER_FLAGS: u16 = 190;
    /// skin | face << 8 | hair style << 16 | hair colour << 24.
    pub const PLAYER_BYTES: u16 = 193;
    /// facial hair | ... | rest state << 24.
    pub const PLAYER_BYTES_2: u16 = 194;
    /// gender.
    pub const PLAYER_BYTES_3: u16 = 195;
    /// The item entry shown in equipment slot 0, which every client draws the player by; slot
    /// `n`'s is `PLAYER_VISIBLE_ITEM_1_0 + n * PLAYER_VISIBLE_ITEM_STRIDE`, its seven enchants
    /// after it.
    pub const PLAYER_VISIBLE_ITEM_1_0: u16 = 260;
    pub const PLAYER_VISIBLE_ITEM_STRIDE: u16 = 12;
    /// The item object (guid, 2 words) in equipment slot 0, the next slot 2 words on; only the
    /// player's own client is told.
    pub const PLAYER_INV_SLOT_HEAD: u16 = 486;
    /// The item (guid, 2 words) in backpack slot 0, the next slot 2 words on (16 slots).
    pub const PLAYER_PACK_SLOT_1: u16 = 532;
    pub const PLAYER_XP: u16 = 716;
    pub const PLAYER_NEXT_LEVEL_XP: u16 = 717;
    /// Copper.
    pub const PLAYER_COINAGE: u16 = 1176;
    pub const PLAYER_WATCHED_FACTION_INDEX: u16 = 1261;
}

/// `OBJECT_FIELD_TYPE` bits.
pub const TYPE_OBJECT: u32 = 0x01;
pub const TYPE_ITEM: u32 = 0x02;
pub const TYPE_UNIT: u32 = 0x08;
pub const TYPE_PLAYER: u32 = 0x10;
pub const TYPE_GAMEOBJECT: u32 = 0x20;
pub const TYPE_DYNAMICOBJECT: u32 = 0x40;

/// The object type id on a create.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectTypeId {
    Item = 1,
    Unit = 3,
    Player = 4,
    GameObject = 5,
    DynamicObject = 6,
}

/// Movement-block flags.
const UPDATE_FLAG_SELF: u8 = 0x01;
const UPDATE_FLAG_ALL: u8 = 0x10;
const UPDATE_FLAG_LIVING: u8 = 0x20;
const UPDATE_FLAG_HAS_POSITION: u8 = 0x40;

/// Update kinds.
const UPDATE_VALUES: u8 = 0;
const CREATE_OBJECT2: u8 = 3;
const OUT_OF_RANGE: u8 = 4;

/// Run, walk and the rest of a unit's speeds, in the order the movement block carries them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Speeds {
    pub walk: f32,
    pub run: f32,
    pub run_back: f32,
    pub swim: f32,
    pub swim_back: f32,
    pub turn_rate: f32,
}

impl Speeds {
    /// 1.12's defaults (walk 2.5, run 7, back 4.5, swim 4.72, swim back 2.5, turn π rad/s).
    pub const DEFAULT: Self = Self {
        walk: 2.5,
        run: 7.0,
        run_back: 4.5,
        swim: 4.722_222,
        swim_back: 2.5,
        turn_rate: std::f32::consts::PI,
    };

    pub fn running_at(run: f32) -> Self {
        Self {
            run,
            ..Self::DEFAULT
        }
    }
}

/// A sparse set of update fields.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fields(BTreeMap<u16, u32>);

impl Fields {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn u32(&mut self, index: u16, v: u32) -> &mut Self {
        self.0.insert(index, v);
        self
    }

    pub fn f32(&mut self, index: u16, v: f32) -> &mut Self {
        self.u32(index, v.to_bits())
    }

    pub fn u64(&mut self, index: u16, v: u64) -> &mut Self {
        self.u32(index, v as u32).u32(index + 1, (v >> 32) as u32)
    }

    pub fn bytes(&mut self, index: u16, b: [u8; 4]) -> &mut Self {
        self.u32(index, u32::from_le_bytes(b))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// These fields with `other`'s set over them.
    pub fn extend(&mut self, other: &Fields) -> &mut Self {
        self.0.extend(other.0.iter().map(|(k, v)| (*k, *v)));
        self
    }

    /// The value of field `index`, if it is set.
    pub fn get(&self, index: u16) -> Option<u32> {
        self.0.get(&index).copied()
    }

    /// The mask (block count, then the blocks) and the set values in index order. A create
    /// leaves out zero fields, as the client reads absent ones as 0.
    fn write(&self, b: &mut Body) {
        let blocks = self
            .0
            .keys()
            .next_back()
            .map_or(0, |&i| i as usize / 32 + 1);
        let mut mask = vec![0u32; blocks];
        for &i in self.0.keys() {
            mask[i as usize / 32] |= 1 << (i % 32);
        }
        b.u8(blocks as u8);
        for m in mask {
            b.u32(m);
        }
        for &v in self.0.values() {
            b.u32(v);
        }
    }
}

/// A moving unit's position block: where it is, which way it faces, its speeds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Living {
    pub position: [f32; 3],
    pub facing: f32,
    pub speeds: Speeds,
}

/// One object of an update packet.
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectUpdate {
    /// A unit or player appears; `own` marks the receiver's own player.
    Create {
        guid: u64,
        type_id: ObjectTypeId,
        living: Living,
        fields: Fields,
        own: bool,
    },
    /// A game object or dynamic object appears at a fixed place.
    CreatePlaced {
        guid: u64,
        type_id: ObjectTypeId,
        position: [f32; 3],
        facing: f32,
        fields: Fields,
    },
    /// An item appears: it has no place of its own (where it is held is in its holder's fields).
    CreateItem {
        guid: u64,
        fields: Fields,
    },
    Values {
        guid: u64,
        fields: Fields,
    },
    /// These objects left (the client forgets them without a death).
    OutOfRange {
        guids: Vec<u64>,
    },
}

impl ObjectUpdate {
    fn write(&self, b: &mut Body) {
        match self {
            Self::Create {
                guid,
                type_id,
                living,
                fields,
                own,
            } => {
                b.u8(CREATE_OBJECT2).packed_guid(*guid).u8(*type_id as u8);
                let mut flags = UPDATE_FLAG_ALL | UPDATE_FLAG_LIVING | UPDATE_FLAG_HAS_POSITION;
                if *own {
                    flags |= UPDATE_FLAG_SELF;
                }
                b.u8(flags);
                // LIVING: movement flags, time, position, facing, fall time, the six speeds
                b.u32(0)
                    .u32(0)
                    .vec3(living.position)
                    .f32(living.facing)
                    .f32(0.0);
                let s = living.speeds;
                b.f32(s.walk)
                    .f32(s.run)
                    .f32(s.run_back)
                    .f32(s.swim)
                    .f32(s.swim_back)
                    .f32(s.turn_rate);
                // ALL: a constant 1 (the high part of the guid in later versions)
                b.u32(1);
                fields.write(b);
            }
            Self::CreatePlaced {
                guid,
                type_id,
                position,
                facing,
                fields,
            } => {
                b.u8(CREATE_OBJECT2).packed_guid(*guid).u8(*type_id as u8);
                // HAS_POSITION: where it stands; ALL: a constant 1
                b.u8(UPDATE_FLAG_ALL | UPDATE_FLAG_HAS_POSITION)
                    .vec3(*position)
                    .f32(*facing)
                    .u32(1);
                fields.write(b);
            }
            Self::CreateItem { guid, fields } => {
                b.u8(CREATE_OBJECT2)
                    .packed_guid(*guid)
                    .u8(ObjectTypeId::Item as u8);
                // ALL alone: a constant 1
                b.u8(UPDATE_FLAG_ALL).u32(1);
                fields.write(b);
            }
            Self::Values { guid, fields } => {
                b.u8(UPDATE_VALUES).packed_guid(*guid);
                fields.write(b);
            }
            Self::OutOfRange { guids } => {
                b.u8(OUT_OF_RANGE).u32(guids.len() as u32);
                for &g in guids {
                    b.packed_guid(g);
                }
            }
        }
    }
}

/// `SMSG_UPDATE_OBJECT` with `objects`.
pub fn update_object(objects: &[ObjectUpdate]) -> Packet {
    let mut b = Body::new();
    b.u32(objects.len() as u32).u8(0); // no transport
    for o in objects {
        o.write(&mut b);
    }
    b.packet(benilla_protocol::messages::opcode::SMSG_UPDATE_OBJECT)
}

#[cfg(test)]
mod tests {
    use benilla_protocol::messages::parse_server;
    use benilla_protocol::{decode, EntityKind, SessionEvent};

    use super::*;

    /// A field's value as the client holds it.
    pub(crate) fn field_of(fields: &benilla_protocol::ObjectFields, index: u16) -> Option<u32> {
        fields
            .raw_fields()
            .find(|&(i, _)| i == index)
            .map(|(_, v)| v)
    }

    #[test]
    fn a_created_unit_reads_back_with_its_place_look_and_fields() {
        let mut f = Fields::new();
        f.u64(field::OBJECT_GUID, 0xF130_0000_0000_0042)
            .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_UNIT)
            .u32(field::OBJECT_ENTRY, 299)
            .f32(field::OBJECT_SCALE_X, 1.5);
        f.u32(field::UNIT_HEALTH, 40)
            .u32(field::UNIT_MAXHEALTH, 55)
            .u32(field::UNIT_DISPLAYID, 2490)
            .u32(field::UNIT_NATIVEDISPLAYID, 2490);
        let living = Living {
            position: [-9000.0, 120.5, 80.25],
            facing: 1.0,
            speeds: Speeds::running_at(5.0),
        };
        let p = update_object(&[ObjectUpdate::Create {
            guid: 0xF130_0000_0000_0042,
            type_id: ObjectTypeId::Unit,
            living,
            fields: f,
            own: false,
        }]);
        let parsed = parse_server(p.opcode, &p.body).expect("parses");
        let events = decode(parsed);
        let Some(SessionEvent::ObjectCreate {
            guid,
            kind,
            display_id,
            position,
            orientation,
            scale,
            speeds,
            fields,
            ..
        }) = events.into_iter().next()
        else {
            panic!("an object create")
        };
        assert_eq!(guid, 0xF130_0000_0000_0042);
        assert_eq!(kind, EntityKind::Unit);
        assert_eq!(display_id, Some(2490));
        assert_eq!(position, [-9000.0, 120.5, 80.25]);
        assert_eq!(orientation, 1.0);
        assert_eq!(scale, 1.5);
        assert_eq!(speeds.map(|s| s.run), Some(5.0));
        assert_eq!(field_of(&fields, field::UNIT_HEALTH), Some(40));
        assert_eq!(field_of(&fields, field::UNIT_MAXHEALTH), Some(55));
    }

    #[test]
    fn a_placed_game_object_reads_back_where_it_stands() {
        let mut f = Fields::new();
        f.u64(field::OBJECT_GUID, 0xF110_0000_0000_0007)
            .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_GAMEOBJECT)
            .u32(field::OBJECT_ENTRY, 180_000)
            .f32(field::OBJECT_SCALE_X, 0.5);
        f.u32(field::GAMEOBJECT_DISPLAYID, 6421)
            .u32(field::GAMEOBJECT_STATE, 1)
            .u32(field::GAMEOBJECT_TYPE_ID, 5);
        let p = update_object(&[ObjectUpdate::CreatePlaced {
            guid: 0xF110_0000_0000_0007,
            type_id: ObjectTypeId::GameObject,
            position: [10.0, 20.0, 30.0],
            facing: 1.5,
            fields: f,
        }]);
        let events = decode(parse_server(p.opcode, &p.body).expect("parses"));
        let Some(SessionEvent::ObjectCreate {
            guid,
            kind,
            display_id,
            position,
            orientation,
            scale,
            ..
        }) = events.into_iter().next()
        else {
            panic!("an object create")
        };
        assert_eq!(
            (guid, kind, display_id),
            (0xF110_0000_0000_0007, EntityKind::GameObject, Some(6421))
        );
        assert_eq!(
            (position, orientation, scale),
            ([10.0, 20.0, 30.0], 1.5, 0.5)
        );
    }

    #[test]
    fn a_values_update_and_an_out_of_range_list_read_back() {
        let mut f = Fields::new();
        f.u32(field::UNIT_HEALTH, 7);
        let p = update_object(&[
            ObjectUpdate::Values { guid: 5, fields: f },
            ObjectUpdate::OutOfRange { guids: vec![9, 10] },
        ]);
        let events = decode(parse_server(p.opcode, &p.body).expect("parses"));
        assert!(events.iter().any(|e| matches!(e, SessionEvent::ObjectValues { guid: 5, fields } if field_of(fields, field::UNIT_HEALTH) == Some(7))), "{events:?}");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SessionEvent::ObjectsRemoved(guids) if guids == &[9, 10])),
            "{events:?}"
        );
    }
}
