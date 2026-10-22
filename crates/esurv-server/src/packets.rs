//! The server packets the world server sends, one builder each.

use benilla_protocol::messages::opcode;

use crate::wire::{Body, Packet};

/// Result codes of the character screen (1.12).
pub mod code {
    pub const CHAR_CREATE_SUCCESS: u8 = 0x2E;
    pub const CHAR_CREATE_ERROR: u8 = 0x2F;
    pub const CHAR_CREATE_NAME_IN_USE: u8 = 0x31;
    pub const CHAR_CREATE_DISABLED: u8 = 0x32;
    pub const CHAR_CREATE_SERVER_LIMIT: u8 = 0x34;
    pub const CHAR_DELETE_SUCCESS: u8 = 0x39;
    pub const CHAR_DELETE_FAILED: u8 = 0x3A;
    /// `SMSG_CHARACTER_LOGIN_FAILED` takes a reason (1 to 6), not a status code: the client
    /// shows "World server is down", "Login for that race, class, or character is currently
    /// disabled." and "Character not found" for these; anything else reads "Login failed".
    pub const CHAR_LOGIN_NO_WORLD: u8 = 1;
    pub const CHAR_LOGIN_DISABLED: u8 = 4;
    pub const CHAR_LOGIN_NO_CHARACTER: u8 = 5;
    pub const CHAR_NAME_NO_NAME: u8 = 0x43;
    pub const CHAR_NAME_TOO_SHORT: u8 = 0x44;
    pub const CHAR_NAME_TOO_LONG: u8 = 0x45;
    pub const CHAR_NAME_ONLY_LETTERS: u8 = 0x46;
}

pub fn auth_challenge(server_seed: u32) -> Packet {
    Body::new()
        .u32(server_seed)
        .packet(opcode::SMSG_AUTH_CHALLENGE)
}

/// `SMSG_AUTH_RESPONSE` admitting the session (no billing time).
pub fn auth_ok() -> Packet {
    Body::new()
        .u8(benilla_protocol::messages::AUTH_OK)
        .u32(0)
        .u8(0)
        .u32(0)
        .packet(opcode::SMSG_AUTH_RESPONSE)
}

pub fn auth_refused(code: u8) -> Packet {
    Body::new().u8(code).packet(opcode::SMSG_AUTH_RESPONSE)
}

/// One hero on the character list.
#[derive(Debug, Clone, PartialEq)]
pub struct RosterEntry {
    pub guid: u64,
    pub name: String,
    pub race: u8,
    pub class: u8,
    pub gender: u8,
    /// skin, face, hair style, hair colour, facial hair.
    pub look: [u8; 5],
    pub level: u8,
    pub area: u32,
    pub map: u32,
    pub position: [f32; 3],
    /// What it shows in each of the 19 equipment slots: `(ItemDisplayInfo id, inventory type)`,
    /// `(0, 0)` for nothing (its class's worn gear, see [`crate::looks`]).
    pub equipment: [(u32, u8); 19],
}

/// `SMSG_CHAR_ENUM`: the heroes, each with what it wears.
pub fn char_enum(heroes: &[RosterEntry]) -> Packet {
    let mut b = Body::new();
    b.u8(heroes.len().min(255) as u8);
    for h in heroes.iter().take(255) {
        b.u64(h.guid)
            .cstr(&h.name)
            .u8(h.race)
            .u8(h.class)
            .u8(h.gender);
        for v in h.look {
            b.u8(v);
        }
        b.u8(h.level).u32(h.area).u32(h.map).vec3(h.position);
        b.u32(0).u32(0).u8(0); // guild, flags, first login
        b.u32(0).u32(0).u32(0); // pet display, level, family
                                // the 19 equipment slots, then the first bag (none): display, inventory type
        for (display, inventory_type) in h.equipment {
            b.u32(display).u8(inventory_type);
        }
        b.u32(0).u8(0);
    }
    b.packet(opcode::SMSG_CHAR_ENUM)
}

pub fn char_create_result(code: u8) -> Packet {
    Body::new().u8(code).packet(opcode::SMSG_CHAR_CREATE)
}

pub fn char_delete_result(code: u8) -> Packet {
    Body::new().u8(code).packet(opcode::SMSG_CHAR_DELETE)
}

pub fn character_login_failed(code: u8) -> Packet {
    Body::new()
        .u8(code)
        .packet(opcode::SMSG_CHARACTER_LOGIN_FAILED)
}

/// `SMSG_LOGIN_VERIFY_WORLD`: the map and where the hero stands.
pub fn login_verify_world(map: u32, position: [f32; 3], facing: f32) -> Packet {
    Body::new()
        .u32(map)
        .vec3(position)
        .f32(facing)
        .packet(opcode::SMSG_LOGIN_VERIFY_WORLD)
}

/// The game clock runs at real speed: one game minute per minute.
pub const TIME_SPEED: f32 = 1.0 / 60.0;

/// `SMSG_LOGIN_SETTIMESPEED`: the time of day is `hour`:`minute` (the date is fixed and unused).
pub fn login_set_time_speed(hour: u32, minute: u32) -> Packet {
    // minute:6, hour:5, weekday:3, day:6, month:4, year:5 (years since 2000)
    let packed = (minute & 0x3F) | (hour & 0x1F) << 6 | 1 << 11 | 25 << 24;
    Body::new()
        .u32(packed)
        .f32(TIME_SPEED)
        .packet(opcode::SMSG_LOGIN_SETTIMESPEED)
}

/// `SMSG_TUTORIAL_FLAGS`: the tips seen, a bit each (every bit set: the tips off).
pub fn tips_seen(bits: &[u32]) -> Packet {
    let mut b = Body::new();
    for &word in bits {
        b.u32(word);
    }
    b.packet(opcode::SMSG_TUTORIAL_FLAGS)
}

/// `SMSG_INITIAL_SPELLS`: the spellbook at login (`Spell.dbc` ids), no cooldowns.
pub fn initial_spells(spells: &[u32]) -> Packet {
    let mut b = Body::new();
    b.u8(0).u16(spells.len() as u16);
    for &s in spells {
        b.u16(s as u16).u16(0);
    }
    b.u16(0).packet(opcode::SMSG_INITIAL_SPELLS)
}

/// `SMSG_ACTION_BUTTONS`: the 120 action-bar slots, `(slot, spell)` filled, the rest empty.
pub fn action_buttons(spells: &[(u8, u32)]) -> Packet {
    let mut slots = [0u32; 120];
    for &(slot, spell) in spells {
        if let Some(s) = slots.get_mut(usize::from(slot)) {
            // the top byte 0: a spell button
            *s = spell & 0x00FF_FFFF;
        }
    }
    let mut b = Body::new();
    for s in slots {
        b.u32(s);
    }
    b.packet(opcode::SMSG_ACTION_BUTTONS)
}

/// `SMSG_UPDATE_AURA_DURATION`: the hero's aura in `slot` runs out in `ms`.
pub fn aura_duration(slot: u8, ms: u32) -> Packet {
    Body::new()
        .u8(slot)
        .u32(ms)
        .packet(opcode::SMSG_UPDATE_AURA_DURATION)
}

/// `SMSG_LEARNED_SPELL`: one more spell in the book.
pub fn learned_spell(spell: u32) -> Packet {
    Body::new().u32(spell).packet(opcode::SMSG_LEARNED_SPELL)
}

/// `SMSG_SPELL_COOLDOWN`: `caster`'s spells wait this many ms (the action bar's sweep).
pub fn spell_cooldown(caster: u64, cooldowns: &[(u32, u32)]) -> Packet {
    let mut b = Body::new();
    b.u64(caster);
    for &(spell, ms) in cooldowns {
        b.u32(spell).u32(ms);
    }
    b.packet(opcode::SMSG_SPELL_COOLDOWN)
}

/// `SMSG_CLEAR_COOLDOWN`: the client forgets `spell`'s cooldown records (its own timer, its
/// category's and its GCD).
pub fn clear_cooldown(caster: u64, spell: u32) -> Packet {
    Body::new()
        .u32(spell)
        .u64(caster)
        .packet(opcode::SMSG_CLEAR_COOLDOWN)
}

/// `SMSG_COOLDOWN_CHEAT`: every cooldown of `caster`'s spells ends at once.
pub fn cooldowns_cleared(caster: u64) -> Packet {
    Body::new().u64(caster).packet(opcode::SMSG_COOLDOWN_CHEAT)
}

/// Where a spell is aimed on the wire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpellTarget {
    Unit(u64),
    Ground([f32; 3]),
    /// At the caster.
    Caster,
}

fn spell_target(b: &mut Body, target: SpellTarget) {
    const TARGET_FLAG_UNIT: u16 = 0x02;
    const TARGET_FLAG_DEST_LOCATION: u16 = 0x40;
    match target {
        SpellTarget::Unit(guid) => {
            b.u16(TARGET_FLAG_UNIT).packed_guid(guid);
        }
        SpellTarget::Ground(at) => {
            b.u16(TARGET_FLAG_DEST_LOCATION).vec3(at);
        }
        SpellTarget::Caster => {
            b.u16(0);
        }
    }
}

/// The cast flags every spell go carries, and the one that adds an ammo block.
const CAST_FLAGS: u16 = 0x100;
const CAST_FLAG_AMMO: u16 = 0x20;
/// The arrow a ranged shot flies: its display id and inventory type (ammo).
pub const ARROW: (u32, u32) = (5996, 24);

/// `SMSG_SPELL_START`: `caster` begins casting `spell` for `cast_ms`.
pub fn spell_start(caster: u64, spell: u32, cast_ms: u32, target: SpellTarget) -> Packet {
    let mut b = Body::new();
    b.packed_guid(caster)
        .packed_guid(caster)
        .u32(spell)
        .u16(2)
        .u32(cast_ms);
    spell_target(&mut b, target);
    b.packet(opcode::SMSG_SPELL_START)
}

/// `SMSG_SPELL_GO`: `caster`'s `spell` goes off, striking `hits`; `ammo` adds a flying arrow.
pub fn spell_go(
    caster: u64,
    spell: u32,
    hits: &[u64],
    target: SpellTarget,
    ammo: Option<(u32, u32)>,
) -> Packet {
    let mut b = Body::new();
    let flags = CAST_FLAGS | if ammo.is_some() { CAST_FLAG_AMMO } else { 0 };
    b.packed_guid(caster)
        .packed_guid(caster)
        .u32(spell)
        .u16(flags);
    b.u8(hits.len().min(255) as u8);
    for &h in hits.iter().take(255) {
        b.u64(h);
    }
    // no misses
    b.u8(0);
    spell_target(&mut b, target);
    if let Some((display, inventory_type)) = ammo {
        b.u32(display).u32(inventory_type);
    }
    b.packet(opcode::SMSG_SPELL_GO)
}

/// `SMSG_SPELLNONMELEEDAMAGELOG`: `caster`'s `spell` hit `target` for `damage` (the floating
/// number and the combat log line).
pub fn spell_damage_log(
    target: u64,
    caster: u64,
    spell: u32,
    damage: u32,
    school: u8,
    crit: bool,
) -> Packet {
    let mut b = Body::new();
    b.packed_guid(target)
        .packed_guid(caster)
        .u32(spell)
        .u32(damage)
        .u8(school);
    // absorbed, resisted, periodic, unused, blocked
    b.u32(0).u32(0).u8(0).u8(0).u32(0);
    b.u32(if crit { 2 } else { 0 }).u8(0);
    b.packet(opcode::SMSG_SPELLNONMELEEDAMAGELOG)
}

/// `SMSG_PERIODICAURALOG`: one damage-over-time tick of `spell` on `target`.
pub fn periodic_damage_log(
    target: u64,
    caster: u64,
    spell: u32,
    damage: u32,
    school: u8,
) -> Packet {
    const AURA_PERIODIC_DAMAGE: u32 = 3;
    let mut b = Body::new();
    b.packed_guid(target).packed_guid(caster).u32(spell).u32(1);
    b.u32(AURA_PERIODIC_DAMAGE)
        .u32(damage)
        .u32(u32::from(school))
        .u32(0)
        .u32(0);
    b.packet(opcode::SMSG_PERIODICAURALOG)
}

/// `SMSG_SPELLHEALLOG`: `healer`'s `spell` healed `target` for `amount`.
pub fn spell_heal_log(target: u64, healer: u64, spell: u32, amount: u32, crit: bool) -> Packet {
    Body::new()
        .packed_guid(target)
        .packed_guid(healer)
        .u32(spell)
        .u32(amount)
        .u8(u8::from(crit))
        .packet(opcode::SMSG_SPELLHEALLOG)
}

/// `SMSG_ATTACKERSTATEUPDATE`: a melee swing of `attacker` at `victim` for `damage`.
pub fn attacker_state(attacker: u64, victim: u64, damage: u32, crit: bool) -> Packet {
    attacker_state_hand(attacker, victim, damage, crit, false)
}

/// [`attacker_state`] by either hand: `off_hand` marks the swing as the off hand's
/// (`HITINFO_LEFTSWING`), so the client plays the off-hand attack.
pub fn attacker_state_hand(attacker: u64, victim: u64, damage: u32, crit: bool, off_hand: bool) -> Packet {
    const HITINFO_NORMALSWING2: u32 = 0x02;
    const HITINFO_LEFTSWING: u32 = 0x04;
    const HITINFO_CRITICALHIT: u32 = 0x80;
    const VICTIMSTATE_NORMAL: u32 = 1;
    let hand = if off_hand { HITINFO_LEFTSWING } else { 0 };
    let mut b = Body::new();
    b.u32(HITINFO_NORMALSWING2 | hand | if crit { HITINFO_CRITICALHIT } else { 0 });
    b.packed_guid(attacker).packed_guid(victim).u32(damage);
    // one physical part: school, damage as float and whole, absorbed, resisted
    b.u8(1).u32(0).f32(damage as f32).u32(damage).u32(0).u32(0);
    // victim state, attacker state, melee spell, blocked
    b.u32(VICTIMSTATE_NORMAL).u32(0).u32(0).u32(0);
    b.packet(opcode::SMSG_ATTACKERSTATEUPDATE)
}

/// Chat types and languages the server speaks with.
const CHAT_MSG_WHISPER: u8 = 0x06;
const CHAT_MSG_SYSTEM: u8 = 0x0A;
const CHAT_MSG_MONSTER_YELL: u8 = 0x0C;
const CHAT_MSG_RAID_BOSS_EMOTE: u8 = 0x5A;
const CHAT_MSG_RAID_WARNING: u8 = 0x58;
const LANGUAGE_UNIVERSAL: u32 = 0;
const LANGUAGE_ADDON: u32 = 0xFFFF_FFFF;

/// A string with its length (counting the NUL) in front.
fn len_string(b: &mut Body, s: &str) {
    b.u32(s.len() as u32 + 1).cstr(s);
}

/// `SMSG_MESSAGECHAT`: unit `guid` named `name` yells `text`.
pub fn monster_yell(guid: u64, name: &str, text: &str) -> Packet {
    let mut b = Body::new();
    b.u8(CHAT_MSG_MONSTER_YELL)
        .u32(LANGUAGE_UNIVERSAL)
        .u64(guid);
    len_string(&mut b, name);
    b.u64(0);
    len_string(&mut b, text);
    b.u8(0).packet(opcode::SMSG_MESSAGECHAT)
}

/// `SMSG_MESSAGECHAT`: a line across the middle of the screen; `%s` in `text` is `name`.
pub fn raid_boss_emote(name: &str, text: &str) -> Packet {
    let mut b = Body::new();
    b.u8(CHAT_MSG_RAID_BOSS_EMOTE).u32(LANGUAGE_UNIVERSAL);
    len_string(&mut b, name);
    b.u64(0);
    len_string(&mut b, text);
    b.u8(0).packet(opcode::SMSG_MESSAGECHAT)
}

/// `SMSG_MESSAGECHAT`: a raid warning, large across the middle of the screen.
pub fn raid_warning(text: &str) -> Packet {
    let mut b = Body::new();
    b.u8(CHAT_MSG_RAID_WARNING).u32(LANGUAGE_UNIVERSAL).u64(0);
    len_string(&mut b, text);
    b.u8(0).packet(opcode::SMSG_MESSAGECHAT)
}

/// `SMSG_MESSAGECHAT`: a system line in the chat frame.
pub fn system_message(text: &str) -> Packet {
    let mut b = Body::new();
    b.u8(CHAT_MSG_SYSTEM).u32(LANGUAGE_UNIVERSAL).u64(0);
    len_string(&mut b, text);
    b.u8(0).packet(opcode::SMSG_MESSAGECHAT)
}

/// `SMSG_MESSAGECHAT`: an addon message from `sender` (`prefix`, a tab, `message`), which the
/// client hands to its interface as `CHAT_MSG_ADDON` and never prints.
pub fn addon_message(sender: u64, prefix: &str, message: &str) -> Packet {
    let mut b = Body::new();
    b.u8(CHAT_MSG_WHISPER).u32(LANGUAGE_ADDON).u64(sender);
    len_string(&mut b, &format!("{prefix}\t{message}"));
    b.u8(0).packet(opcode::SMSG_MESSAGECHAT)
}

/// `SMSG_INIT_WORLD_STATES`: none.
pub fn init_world_states(map: u32, area: u32) -> Packet {
    Body::new()
        .u32(map)
        .u32(area)
        .u16(0)
        .packet(opcode::SMSG_INIT_WORLD_STATES)
}

/// `SMSG_WEATHER`: 1.12's weather type (0 fine, 1 rain, 2 snow, 3 storm/sandstorm), its strength
/// 0..1 and the ambient sound that goes with it.
pub fn weather(kind: u32, grade: f32, sound: u32) -> Packet {
    Body::new()
        .u32(kind)
        .f32(grade)
        .u32(sound)
        .u8(0)
        .packet(opcode::SMSG_WEATHER)
}

pub fn pong(sequence: u32) -> Packet {
    Body::new().u32(sequence).packet(opcode::SMSG_PONG)
}

/// `SMSG_QUERY_TIME_RESPONSE`: wall-clock unix seconds.
pub fn query_time_response(unix: u32) -> Packet {
    Body::new()
        .u32(unix)
        .packet(opcode::SMSG_QUERY_TIME_RESPONSE)
}

/// `SMSG_NAME_QUERY_RESPONSE` for a hero.
pub fn name_query_response(guid: u64, name: &str, race: u8, gender: u8, class: u8) -> Packet {
    Body::new()
        .u64(guid)
        .cstr(name)
        .cstr("")
        .u32(race.into())
        .u32(gender.into())
        .u32(class.into())
        .packet(opcode::SMSG_NAME_QUERY_RESPONSE)
}

/// A creature's rank as the creature query tells it (the client's numbering): an ordinary one.
pub const RANK_NORMAL: u32 = 0;
/// An elite (the gold dragon on its portrait).
pub const RANK_ELITE: u32 = 1;
/// A rare one (the silver dragon on its portrait; `UnitClassification` "rare"): our rares.
pub const RANK_RARE: u32 = 4;
/// A boss (the skull for its level).
pub const RANK_BOSS: u32 = 3;

/// What the creature query answers for one creature entry.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureInfo {
    pub name: String,
    pub subname: String,
    /// `CreatureType.dbc` id.
    pub creature_type: u32,
    /// `RANK_NORMAL`, `RANK_ELITE`, `RANK_RARE` or `RANK_BOSS`.
    pub rank: u32,
    pub display_id: u32,
}

/// `SMSG_CREATURE_QUERY_RESPONSE`; `None` tells the client the entry is unknown.
pub fn creature_query_response(entry: u32, info: Option<&CreatureInfo>) -> Packet {
    let mut b = Body::new();
    match info {
        None => b.u32(entry | 0x8000_0000),
        Some(i) => b
            .u32(entry)
            .cstr(&i.name)
            .cstr("")
            .cstr("")
            .cstr("")
            .cstr(&i.subname)
            .u32(0) // type flags
            .u32(i.creature_type)
            .u32(0) // pet family
            .u32(i.rank)
            .u32(0)
            .u32(0) // pet spell list
            .u32(i.display_id)
            .u8(0) // civilian
            .u8(0), // racial leader
    };
    b.packet(opcode::SMSG_CREATURE_QUERY_RESPONSE)
}

/// What the item query answers for one item entry: what the client draws and names it by. The
/// rest of the template (stats, prices, requirements, spells) is sent empty.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemInfo {
    /// Item class (2 weapon, 4 armour) and subclass (a weapon's kind, an armour's material).
    pub class: u32,
    pub subclass: u32,
    pub name: String,
    /// `ItemDisplayInfo.dbc` id.
    pub display_id: u32,
    /// 0 poor, 1 common, 2 uncommon, 3 rare, 4 epic, 5 legendary.
    pub quality: u32,
    /// The equipment slot family (1 head, 5 chest, 13 one-hand, 17 two-hand, 21 main hand ...).
    pub inventory_type: u32,
    /// Where a weapon hangs when put away (1 back, 2 lower back, 3 hip, 4 shield on the back).
    pub sheath: u32,
    /// The tooltip's quoted line under the rest; empty for none.
    pub description: String,
}

/// `SMSG_ITEM_QUERY_SINGLE_RESPONSE`, 1.12's full item template; `None` tells the client the
/// entry is unknown.
pub fn item_query_response(entry: u32, info: Option<&ItemInfo>) -> Packet {
    let mut b = Body::new();
    let Some(i) = info else {
        b.u32(entry | 0x8000_0000);
        return b.packet(opcode::SMSG_ITEM_QUERY_SINGLE_RESPONSE);
    };
    b.u32(entry)
        .u32(i.class)
        .u32(i.subclass)
        .cstr(&i.name)
        .cstr("")
        .cstr("")
        .cstr("")
        .u32(i.display_id)
        .u32(i.quality)
        .u32(0) // flags
        .u32(0) // buy price
        .u32(0) // sell price
        .u32(i.inventory_type)
        .i32(-1) // any class
        .i32(-1) // any race
        .u32(0); // item level
                 // the required level, skill and rank, spell, honor rank, city rank, reputation faction and
                 // rank
    for _ in 0..8 {
        b.u32(0);
    }
    b.u32(0) // max count (0: no limit, no "Unique" line)
        .u32(1) // stackable
        .u32(0); // container slots
    for _ in 0..10 {
        b.u32(0).i32(0); // stat type, value
    }
    for _ in 0..5 {
        b.f32(0.0).f32(0.0).u32(0); // damage min, max, school
    }
    // armour, then the holy, fire, nature, frost, shadow and arcane resistances
    for _ in 0..7 {
        b.u32(0);
    }
    b.u32(0) // delay
        .u32(0) // ammo type
        .f32(0.0); // ranged range modifier
    for _ in 0..5 {
        // spell, trigger, charges, cooldown, category, category cooldown
        b.u32(0).u32(0).i32(0).i32(-1).u32(0).i32(-1);
    }
    b.u32(0) // bonding
        .cstr(&i.description)
        .u32(0) // page text
        .u32(0) // language
        .u32(0) // page material
        .u32(0) // start quest
        .u32(0) // lock
        .u32(0) // material
        .u32(i.sheath)
        .u32(0) // random property
        .u32(0) // block
        .u32(0) // item set
        .u32(0) // max durability
        .u32(0) // area
        .u32(0) // map
        .u32(0); // bag family
    b.packet(opcode::SMSG_ITEM_QUERY_SINGLE_RESPONSE)
}

/// What the game-object query tells about one game-object entry.
#[derive(Debug, Clone, PartialEq)]
pub struct GameObjectInfo {
    /// The game-object type (`GameObjectTypes`: 5 generic, 8 spell focus, 3 chest ...).
    pub type_id: u32,
    pub display_id: u32,
    pub name: String,
    /// The type's data slots.
    pub data: [i32; 24],
}

/// `SMSG_GAMEOBJECT_QUERY_RESPONSE`; `None` tells the client the entry is unknown.
pub fn gameobject_query_response(entry: u32, info: Option<&GameObjectInfo>) -> Packet {
    let mut b = Body::new();
    match info {
        None => {
            b.u32(entry | 0x8000_0000);
        }
        Some(i) => {
            b.u32(entry).u32(i.type_id).u32(i.display_id).cstr(&i.name);
            // name2..4 and the icon name
            b.cstr("").cstr("").cstr("").cstr("");
            for d in i.data {
                b.i32(d);
            }
        }
    }
    b.packet(opcode::SMSG_GAMEOBJECT_QUERY_RESPONSE)
}

/// `SMSG_LOGOUT_RESPONSE` (accepted, at once) and `SMSG_LOGOUT_COMPLETE`.
pub fn logout() -> [Packet; 2] {
    [
        Body::new()
            .u32(0)
            .u8(1)
            .packet(opcode::SMSG_LOGOUT_RESPONSE),
        logout_complete(),
    ]
}

/// `SMSG_LOGOUT_COMPLETE`: the hero is out of the world, back to the character screen.
pub fn logout_complete() -> Packet {
    Body::new().packet(opcode::SMSG_LOGOUT_COMPLETE)
}

/// `SMSG_PLAY_SOUND`: a `SoundEntries.dbc` sound.
pub fn play_sound(sound: u32) -> Packet {
    Body::new().u32(sound).packet(opcode::SMSG_PLAY_SOUND)
}

/// `SMSG_PLAY_OBJECT_SOUND`: a `SoundEntries.dbc` sound, played where object `source` stands.
pub fn play_object_sound(sound: u32, source: u64) -> Packet {
    Body::new().u32(sound).u64(source).packet(opcode::SMSG_PLAY_OBJECT_SOUND)
}

/// `SMSG_NOTIFICATION`: red text across the screen.
pub fn notification(text: &str) -> Packet {
    Body::new().cstr(text).packet(opcode::SMSG_NOTIFICATION)
}

/// `SMSG_DESTROY_OBJECT`: the client forgets `guid`.
pub fn destroy_object(guid: u64) -> Packet {
    Body::new().u64(guid).packet(opcode::SMSG_DESTROY_OBJECT)
}

/// `SMSG_MONSTER_MOVE`: a unit walks (or runs, `run`) from `start` through `path` in
/// `duration_ms`; an empty path stops it at `start`.
pub fn monster_move(
    guid: u64,
    start: [f32; 3],
    spline_id: u32,
    path: &[[f32; 3]],
    duration_ms: u32,
    run: bool,
) -> Packet {
    const MOVE_STOP: u8 = 1;
    const SPLINE_FLAG_RUNMODE: u32 = 0x100;
    let mut b = Body::new();
    b.packed_guid(guid).vec3(start).u32(spline_id);
    let Some((&end, before)) = path.split_last() else {
        return b.u8(MOVE_STOP).packet(opcode::SMSG_MONSTER_MOVE);
    };
    b.u8(0)
        .u32(if run { SPLINE_FLAG_RUNMODE } else { 0 })
        .u32(duration_ms);
    b.u32(path.len() as u32).vec3(end);
    // the points before the last, as quarter-yard offsets back from it
    for p in before {
        let q = |d: f32, mask: i32, shift: u32| ((d * 4.0).round() as i32 & mask) << shift;
        b.i32(
            q(end[0] - p[0], 0x7FF, 0) | q(end[1] - p[1], 0x7FF, 11) | q(end[2] - p[2], 0x3FF, 22),
        );
    }
    b.packet(opcode::SMSG_MONSTER_MOVE)
}

/// `MSG_MOVE_TELEPORT_ACK`: the server puts the client's own hero at `position`; the client
/// answers with the same opcode and `counter`.
pub fn teleport(guid: u64, counter: u32, position: [f32; 3], facing: f32) -> Packet {
    let mut b = Body::new();
    b.packed_guid(guid).u32(counter);
    b.u32(0).u32(0).vec3(position).f32(facing).u32(0); // movement flags, time, place, facing, fall time
    b.packet(opcode::MSG_MOVE_TELEPORT_ACK)
}

/// `SMSG_FORCE_MOVE_ROOT` (or `_UNROOT` when `rooted` is false): the client's own hero cannot
/// move until it is let go; the client acks with `counter`.
pub fn force_move_root(guid: u64, counter: u32, rooted: bool) -> Packet {
    Body::new()
        .packed_guid(guid)
        .u32(counter)
        .packet(if rooted {
            opcode::SMSG_FORCE_MOVE_ROOT
        } else {
            opcode::SMSG_FORCE_MOVE_UNROOT
        })
}

/// `SMSG_FORCE_RUN_SPEED_CHANGE`: the player `guid`'s own client runs at `speed` yards a second
/// from now (it acks with `CMSG_FORCE_RUN_SPEED_CHANGE_ACK`); `counter` numbers the change.
pub fn force_run_speed_change(guid: u64, counter: u32, speed: f32) -> Packet {
    Body::new()
        .packed_guid(guid)
        .u32(counter)
        .f32(speed)
        .packet(opcode::SMSG_FORCE_RUN_SPEED_CHANGE)
}

/// `MSG_MOVE_SET_RUN_SPEED`: the other clients learn that `guid`, standing at `position` facing
/// `facing`, runs at `speed` yards a second from now.
pub fn move_set_run_speed(guid: u64, position: [f32; 3], facing: f32, speed: f32) -> Packet {
    Body::new()
        .packed_guid(guid)
        .u32(0) // movement flags
        .u32(0) // time
        .vec3(position)
        .f32(facing)
        .u32(0) // fall time
        .f32(speed)
        .packet(opcode::MSG_MOVE_SET_RUN_SPEED)
}

#[cfg(test)]
mod tests {
    use benilla_protocol::messages::parse_server;
    use benilla_protocol::ServerPacket;

    use super::*;

    fn parse(p: Packet) -> ServerPacket {
        let (packet, tail) =
            benilla_protocol::messages::parse_server_with_tail(p.opcode, &p.body).expect("parses");
        assert_eq!(tail, 0, "nothing left unread");
        packet
    }

    #[test]
    fn the_character_list_reads_back() {
        let hero = RosterEntry {
            guid: 7,
            name: "Ana".into(),
            race: 1,
            class: 8,
            gender: 1,
            look: [1, 2, 3, 4, 5],
            level: 3,
            area: 12,
            map: 0,
            position: [1.0, 2.0, 3.0],
            equipment: {
                let mut e = [(0, 0); 19];
                e[4] = (9001, 20);
                e[15] = (9002, 17);
                e
            },
        };
        let ServerPacket::CharEnum { characters } = parse(char_enum(&[hero])) else {
            panic!()
        };
        let c = &characters[0];
        assert_eq!(
            (
                c.guid,
                c.name.as_str(),
                c.race,
                c.class,
                c.gender,
                c.level,
                c.zone
            ),
            (7, "Ana", 1, 8, 1, 3, 12)
        );
        assert_eq!(
            (c.skin, c.face, c.hair_style, c.hair_color, c.facial_hair),
            (1, 2, 3, 4, 5)
        );
        assert_eq!(
            (c.equipment[4].display_id, c.equipment[4].inventory_type),
            (9001, 20)
        );
        assert_eq!(c.equipment[15].display_id, 9002);
        assert_eq!(c.equipment[0].display_id, 0, "nothing on the head");
    }

    #[test]
    fn world_entry_packets_read_back() {
        assert!(
            matches!(parse(login_verify_world(0, [1.0, 2.0, 3.0], 0.5)), ServerPacket::LoginVerifyWorld { map: 0, orientation, .. } if orientation == 0.5)
        );
        assert!(matches!(
            parse(login_set_time_speed(14, 30)),
            ServerPacket::TimeSpeed {
                hours: 14,
                minutes: 30,
                ..
            }
        ));
        assert!(matches!(
            parse(weather(1, 0.5, 8534)),
            ServerPacket::Weather {
                weather_type: 1,
                sound_id: 8534,
                ..
            }
        ));
        assert!(matches!(
            parse(auth_ok()),
            ServerPacket::AuthResponse { result: 0x0C, .. }
        ));
        for p in [
            tips_seen(&[u32::MAX; 8]),
            initial_spells(&[133, 20554]),
            action_buttons(&[(0, 133), (11, 20554)]),
            init_world_states(0, 12),
        ] {
            let opcode = p.opcode;
            parse_server(opcode, &p.body).unwrap_or_else(|e| panic!("{opcode:#x}: {e}"));
        }
    }

    #[test]
    fn query_answers_read_back() {
        let info = CreatureInfo {
            name: "Kobold Vermin".into(),
            subname: String::new(),
            creature_type: 7,
            rank: 1,
            display_id: 366,
        };
        let ServerPacket::CreatureQueryResponse {
            entry: 6,
            info: Some(i),
        } = parse(creature_query_response(6, Some(&info)))
        else {
            panic!()
        };
        assert_eq!(
            (i.name.as_str(), i.creature_type, i.rank, i.display_id),
            ("Kobold Vermin", 7, 1, 366)
        );
        assert!(matches!(
            parse(creature_query_response(6, None)),
            ServerPacket::CreatureQueryResponse {
                entry: 6,
                info: None
            }
        ));
        assert!(matches!(
            parse(name_query_response(7, "Ana", 1, 1, 8)),
            ServerPacket::NameQueryResponse {
                guid: 7,
                race: 1,
                gender: 1,
                class: 8,
                ..
            }
        ));
    }

    #[test]
    fn an_item_query_answer_reads_back_whole() {
        let info = ItemInfo {
            class: 2,
            subclass: 8,
            name: "Recruit's Greatsword".into(),
            display_id: 563,
            quality: 1,
            inventory_type: 17,
            sheath: 1,
            description: "Heavy in the hand.".into(),
        };
        let ServerPacket::ItemQueryResponse {
            entry: 100_180,
            info: Some(i),
        } = parse(item_query_response(100_180, Some(&info)))
        else {
            panic!()
        };
        assert_eq!(
            (
                i.class,
                i.subclass,
                i.name.as_str(),
                i.display_info_id,
                i.quality,
                i.inventory_type,
                i.sheath
            ),
            (2, 8, "Recruit's Greatsword", 563, 1, 17, 1)
        );
        assert_eq!((i.allowable_class, i.allowable_race), (-1, -1));
        assert_eq!(i.max_count, 0, "no Unique line");
        assert_eq!(i.description, "Heavy in the hand.");
        assert!(i.stats.is_empty() && i.damages.is_empty() && i.spells.is_empty());
        assert!(matches!(
            parse(item_query_response(25, None)),
            ServerPacket::ItemQueryResponse {
                entry: 25,
                info: None
            }
        ));
    }

    #[test]
    fn a_run_speed_change_reads_back_for_the_mover_and_the_others() {
        use benilla_protocol::messages::SpeedKind;
        assert!(matches!(
            parse(force_run_speed_change(7, 3, 11.2)),
            ServerPacket::ForceSpeedChange { guid: 7, kind: SpeedKind::Run, counter: 3, speed }
                if speed == 11.2
        ));
        let ServerPacket::MoveSetSpeed {
            guid: 7,
            kind: SpeedKind::Run,
            position,
            orientation,
            speed,
            ..
        } = parse(move_set_run_speed(7, [1.0, 2.0, 3.0], 0.5, 11.2))
        else {
            panic!()
        };
        assert_eq!(
            ([position.x, position.y, position.z], orientation, speed),
            ([1.0, 2.0, 3.0], 0.5, 11.2)
        );
    }

    #[test]
    fn a_monster_move_reads_back_with_its_path() {
        let path = [[2.0, 0.0, 0.5], [4.0, 1.0, 1.0], [6.0, 2.0, 1.25]];
        let ServerPacket::MonsterMove {
            guid,
            path: read,
            duration_ms,
            run_mode,
            stop,
            ..
        } = parse(monster_move(9, [0.0, 0.0, 0.0], 1, &path, 1500, true))
        else {
            panic!()
        };
        assert_eq!((guid, duration_ms, run_mode, stop), (9, 1500, true, false));
        let read: Vec<[f32; 3]> = read.iter().map(|v| [v.x, v.y, v.z]).collect();
        assert_eq!(read, [[0.0, 0.0, 0.0], path[0], path[1], path[2]]);
        assert!(matches!(
            parse(monster_move(9, [1.0, 1.0, 1.0], 2, &[], 0, true)),
            ServerPacket::MonsterMove { stop: true, .. }
        ));
    }

    #[test]
    fn a_teleport_reads_back() {
        assert!(
            matches!(parse(teleport(7, 3, [5.0, 6.0, 7.0], 1.0)), ServerPacket::Teleport { guid: 7, counter: 3, orientation, .. } if orientation == 1.0)
        );
    }

    #[test]
    fn the_spellbook_and_action_bar_read_back() {
        let ServerPacket::InitialSpells { spell_ids, .. } = parse(initial_spells(&[20554, 133]))
        else {
            panic!()
        };
        assert_eq!(spell_ids, [20554, 133]);
        let ServerPacket::ActionButtons { buttons } =
            parse(action_buttons(&[(0, 133), (11, 20554)]))
        else {
            panic!()
        };
        let slots: Vec<(u8, u32, u8)> =
            buttons.iter().map(|b| (b.slot, b.action, b.kind)).collect();
        assert_eq!(slots, [(0, 133, 0), (11, 20554, 0)]);
        assert!(matches!(
            parse(learned_spell(10)),
            ServerPacket::LearnedSpell { spell_id: 10 }
        ));
        let ServerPacket::SpellCooldownList { caster, cooldowns } =
            parse(spell_cooldown(7, &[(133, 2100)]))
        else {
            panic!()
        };
        assert_eq!((caster, cooldowns), (7, vec![(133, 2100)]));
        assert!(matches!(
            parse(cooldowns_cleared(7)),
            ServerPacket::CooldownCheat { caster: 7 }
        ));
        assert!(matches!(
            parse(clear_cooldown(7, 133)),
            ServerPacket::ClearCooldown {
                spell_id: 133,
                caster: 7
            }
        ));
    }

    #[test]
    fn spell_looks_read_back_with_their_targets() {
        let ServerPacket::SpellGo(go) =
            parse(spell_go(7, 120, &[9, 10], SpellTarget::Caster, None))
        else {
            panic!()
        };
        assert_eq!(
            (
                go.caster,
                go.spell_id,
                go.hits.clone(),
                go.targets.mask,
                go.ammo_display_id
            ),
            (7, 120, vec![9, 10], 0, None)
        );
        let ServerPacket::SpellGo(go) =
            parse(spell_go(7, 75, &[9], SpellTarget::Unit(9), Some(ARROW)))
        else {
            panic!()
        };
        assert_eq!(
            (go.targets.unit_target, go.ammo_display_id),
            (Some(9), Some(5996))
        );
        let ServerPacket::SpellGo(go) = parse(spell_go(
            7,
            2120,
            &[],
            SpellTarget::Ground([1.0, 2.0, 3.0]),
            None,
        )) else {
            panic!()
        };
        assert_eq!(
            go.targets.dest.map(|d| [d.x, d.y, d.z]),
            Some([1.0, 2.0, 3.0])
        );
        let ServerPacket::SpellStart(start) =
            parse(spell_start(9, 9053, 1500, SpellTarget::Unit(7)))
        else {
            panic!()
        };
        assert_eq!(
            (
                start.caster,
                start.spell_id,
                start.cast_time_ms,
                start.targets.unit_target
            ),
            (9, 9053, 1500, Some(7))
        );
    }

    #[test]
    fn numbers_read_back() {
        let ServerPacket::SpellDamageLog(d) = parse(spell_damage_log(9, 7, 133, 41, 2, true))
        else {
            panic!()
        };
        assert_eq!(
            (d.target, d.attacker, d.spell_id, d.damage, d.school, d.hit_info),
            (9, 7, 133, 41, 2, 2)
        );
        let ServerPacket::PeriodicAuraLog(p) = parse(periodic_damage_log(9, 7, 172, 12, 5)) else {
            panic!()
        };
        assert!(matches!(
            p.ticks.as_slice(),
            [benilla_protocol::messages::PeriodicTick::Damage {
                amount: 12,
                school: 5,
                ..
            }]
        ));
        let ServerPacket::SpellHealLog(h) = parse(spell_heal_log(7, 7, 2061, 30, false)) else {
            panic!()
        };
        assert_eq!((h.target, h.healer, h.spell_id, h.amount), (7, 7, 2061, 30));
        let ServerPacket::AttackerState(a) = parse(attacker_state(9, 7, 15, true)) else {
            panic!()
        };
        assert_eq!(
            (a.attacker, a.victim, a.damage, a.hit_info & 0x80),
            (9, 7, 15, 0x80)
        );
        assert_eq!(a.hit_info & 0x04, 0, "the main hand's swing");
        let ServerPacket::AttackerState(a) = parse(attacker_state_hand(9, 7, 15, false, true)) else {
            panic!()
        };
        assert_eq!(a.hit_info & 0x84, 0x04, "the off hand's swing, no crit");
    }

    #[test]
    fn the_game_object_query_reads_back() {
        let info = GameObjectInfo {
            type_id: 5,
            display_id: 6421,
            name: "Orb".into(),
            data: [0; 24],
        };
        let ServerPacket::GameObjectQueryResponse {
            entry,
            info: Some(i),
        } = parse(gameobject_query_response(180_000, Some(&info)))
        else {
            panic!()
        };
        assert_eq!(
            (entry, i.type_id, i.display_id, i.name.as_str()),
            (180_000, 5, 6421, "Orb")
        );
        assert!(matches!(
            parse(gameobject_query_response(9, None)),
            ServerPacket::GameObjectQueryResponse {
                entry: 9,
                info: None
            }
        ));
    }

    #[test]
    fn lines_read_back() {
        let ServerPacket::MessageChat(y) = parse(monster_yell(9, "Hogger", "Grr!")) else {
            panic!()
        };
        assert_eq!(
            (
                y.chat_type,
                y.sender_guid,
                y.sender_name.as_deref(),
                y.text.as_str()
            ),
            (0x0C, 9, Some("Hogger"), "Grr!")
        );
        let ServerPacket::MessageChat(e) = parse(raid_boss_emote("Hogger", "%s approaches!"))
        else {
            panic!()
        };
        assert_eq!(
            (e.chat_type, e.sender_name.as_deref(), e.text.as_str()),
            (0x5A, Some("Hogger"), "%s approaches!")
        );
        let ServerPacket::MessageChat(m) = parse(system_message("Hello")) else {
            panic!()
        };
        assert_eq!((m.chat_type, m.text.as_str()), (0x0A, "Hello"));
        let ServerPacket::MessageChat(a) = parse(addon_message(7, "ESURV", "health=230")) else {
            panic!()
        };
        assert!(a.is_addon());
        assert_eq!((a.sender_guid, a.text.as_str()), (7, "ESURV\thealth=230"));
    }

    #[test]
    fn a_root_and_its_release_read_back() {
        for rooted in [true, false] {
            let ServerPacket::MoveMode {
                guid,
                counter,
                mode,
                apply,
            } = parse(force_move_root(7, 3, rooted))
            else {
                panic!()
            };
            assert_eq!((guid, counter, apply), (7, 3, rooted));
            assert_eq!(mode, benilla_protocol::MoveMode::Root);
        }
    }
}
