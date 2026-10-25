//! The battle standard on the field and the ally it summons, as the clients are told them.

use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use benilla_protocol::messages::{opcode, parse_server};
use benilla_protocol::{decode, EntityKind, ObjectFields, SessionEvent};
use esurv_content::{Content, Faction, PlayerForm};
use esurv_sim::game::loot::Pickup;
use esurv_sim::game::run::Aim;

use super::*;
use crate::game::STANDARD_NAME;
use crate::game::{gameobject_look, Player};
use crate::save::{SavedHero, Saves};
use crate::wire::PacketWriter;

/// A run with one warrior of `race`, its client at the other end of a local socket (unread).
fn game_with(race: &str) -> (Game, TcpStream) {
    let content = Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
    let saved = SavedHero {
        guid: 1,
        name: "Ana".into(),
        class: "warrior".into(),
        race: race.into(),
        gender: 0,
        look: [0; 5],
        records: Default::default(),
        zone: None,
        start: None,
    };
    let (_host, inputs) = std::sync::mpsc::channel();
    let mut game = Game::new(content, None, 0, &saved, Arc::new(Saves::in_memory()), inputs).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (server, _) = listener.accept().unwrap();
    let hero = game.hero_id(0);
    assert!(game.join(Player { hero, saved, writer: Arc::new(Mutex::new(PacketWriter::new(server))) }));
    (game, client)
}

fn field_of(fields: &ObjectFields, index: u16) -> Option<u32> {
    fields.raw_fields().find(|&(i, _)| i == index).map(|(_, v)| v)
}

fn creates(objects: &[ObjectUpdate]) -> Vec<SessionEvent> {
    let p = update_object(objects);
    decode(parse_server(p.opcode, &p.body).unwrap()).into_iter().filter(|e| matches!(e, SessionEvent::ObjectCreate { .. })).collect()
}

/// `AllySummoned` of `character` as ally `id` beside the run's hero.
fn summoned(game: &Game, id: u32, character: &str, how: Appear, look_spell: Option<u32>) -> Event {
    Event::AllySummoned {
        id,
        hero: game.hero_id(0),
        character: character.into(),
        at: game.run.heroes[0].pos,
        facing: 0.0,
        how,
        look_spell,
        scale: game.run.content.balance.battle_standard.ally_scale,
    }
}

/// The allies of `events` mirrored: their names, creates and the looks after them.
fn mirrored(game: &mut Game, events: &[Event]) -> (Vec<Packet>, Vec<ObjectUpdate>, Vec<Packet>) {
    let (mut names, mut objects, mut after) = (Vec::new(), Vec::new(), Vec::new());
    game.mirror_allies(events, &mut names, &mut objects, &mut after);
    (names, objects, after)
}

/// A battle standard lies as its faction's Alterac standard, a creature model planted in the ground
/// (a unit that cannot be targeted), in a soft glow of the faction's colour (a game object of an
/// entry of its own that the game-object query answers) under a beam of light of that colour; it
/// is planted with its faction's standard look, pings the minimap where it lies, and all of it goes
/// once it is taken.
#[test]
fn a_battle_standard_is_its_faction_s_standard_in_a_glow_with_a_ping() {
    for (race, faction, model, planted, glow, beam) in
        [("human", Faction::Alliance, 14541, 23034, 263, [2232, 298]), ("orc", Faction::Horde, 14542, 23035, 1308, [1311, 4831])]
    {
        let (mut game, _client) = game_with(race);
        let p = game.run.heroes[0].pos;
        let at = (p.0 + 5.0, p.1);
        game.run.lay_pickup(Pickup::BattleStandard(faction), at);
        let (mut objects, mut moves, mut gone) = (Vec::new(), Vec::new(), Vec::new());
        game.mirror_pickups(&mut objects, &mut moves, &mut gone);
        let shown = creates(&objects);
        let [SessionEvent::ObjectCreate { kind: EntityKind::Unit, guid, fields: standard, .. }, SessionEvent::ObjectCreate { kind: EntityKind::GameObject, fields: lit, .. }, SessionEvent::ObjectCreate { kind: EntityKind::GameObject, fields: tall, .. }, SessionEvent::ObjectCreate { kind: EntityKind::GameObject, fields: taller, .. }] = &shown[..] else {
            panic!("{race}: a unit, its glow and its beam's two parts, {shown:?}")
        };
        // the beam of light in the faction's colour, without the magnet's sparkle
        assert_eq!([field_of(tall, field::GAMEOBJECT_DISPLAYID), field_of(taller, field::GAMEOBJECT_DISPLAYID)], beam.map(Some), "{race}'s beam");
        for part in [tall, taller] {
            let l = gameobject_look(field_of(part, field::OBJECT_ENTRY).unwrap()).unwrap();
            assert_eq!(l.name, STANDARD_NAME);
            assert!(beam.contains(&l.display));
        }
        assert_eq!(field_of(standard, field::UNIT_DISPLAYID), Some(model), "{race}");
        assert_eq!(field_of(standard, field::OBJECT_ENTRY), Some(crate::entries::BATTLE_STANDARD));
        assert_eq!(field_of(standard, field::UNIT_FLAGS), Some(super::super::pickups::ORB_UNIT_FLAGS), "not to be targeted");
        let entry = field_of(lit, field::OBJECT_ENTRY).unwrap();
        assert_eq!((entry, field_of(lit, field::GAMEOBJECT_DISPLAYID)), (crate::entries::BATTLE_STANDARDS_FROM + glow, Some(glow)));
        let l = gameobject_look(entry).unwrap();
        assert_eq!((l.display, l.name), (glow, STANDARD_NAME));
        assert_eq!(moves[0], packets::spell_go(*guid, planted, &[*guid], SpellTarget::Unit(*guid), None), "planted with its look");
        let ping = moves.iter().find(|m| m.opcode == opcode::MSG_MINIMAP_PING).expect("a minimap ping");
        let x = f32::from_le_bytes(ping.body[8..12].try_into().unwrap());
        let y = f32::from_le_bytes(ping.body[12..16].try_into().unwrap());
        assert_eq!((x, y), at);
        game.run.pickups.clear();
        let (mut objects, mut moves, mut gone) = (Vec::new(), Vec::new(), Vec::new());
        game.mirror_pickups(&mut objects, &mut moves, &mut gone);
        assert_eq!(gone.len(), 4, "the standard, its glow and its beam go");
    }
}

/// The summoned ally: its name first, then a player of its character's race, gender, looks and
/// gear, drawn at the summon's scale (twice its size), of its race's faction, not to be attacked
/// or selected, wearing Stealth as it comes out of it; it yells as itself, and once it vanishes it
/// plays the look and goes.
#[test]
fn an_ally_shows_as_its_character_at_twice_its_size_and_leaves() {
    let (mut game, _client) = game_with("orc");
    let content = game.run.content.clone();
    let c = content.player_characters_of(Faction::Horde).find(|c| c.form.is_none()).unwrap().clone();
    let race = content.races.iter().find(|r| r.id == c.race).unwrap();
    let ev = summoned(&game, 900, &c.id, Appear::Stealth, Some(1856));
    let (names, objects, after) = mirrored(&mut game, &[ev]);
    assert_eq!(names.len(), 1, "its name reaches the clients first");
    let [SessionEvent::ObjectCreate { guid, kind: EntityKind::Player, fields, .. }] = &creates(&objects)[..] else {
        panic!("one player")
    };
    assert_eq!(*guid, ally_guid(900));
    assert_eq!(game.guid_of(Unit::Ally(900)), Some(*guid));
    assert_eq!(game.ally_name(900), Some(c.name.clone()));
    assert_eq!(field_of(fields, field::OBJECT_SCALE_X), Some(2.0f32.to_bits()), "twice its size");
    assert_eq!(field_of(fields, field::UNIT_FACTIONTEMPLATE), Some(race.faction_template));
    assert_eq!(field_of(fields, field::UNIT_FLAGS), Some(ALLY_UNIT_FLAGS));
    assert_eq!(field_of(fields, field::UNIT_AURA), Some(STEALTH_LOOK), "out of stealth");
    let bytes0 = field_of(fields, field::UNIT_BYTES_0).unwrap();
    assert_eq!(bytes0 & 0xFF, u32::from(race.wow_race_id));
    assert_eq!((bytes0 >> 16) & 0xFF, u32::from(c.gender.id()));
    if let Some(l) = c.looks {
        assert_eq!(field_of(fields, field::PLAYER_BYTES).map(|b| b & 0xFF), Some(u32::from(l.skin)));
    }
    assert!(after.iter().any(|p| p.opcode == opcode::SMSG_SPELL_GO), "its arrival's look");
    let (all, _) = game.present(&[Event::Yell { id: 900, text: "Lok'tar!".into() }]);
    assert!(all.iter().any(|p| p.opcode == opcode::SMSG_MESSAGECHAT), "it yells");
    let (all, _) = game.present(&[Event::AllyLeft { id: 900, how: Leave::Vanish, look_spell: None, to: None }]);
    assert!(all.iter().any(|p| p.opcode == opcode::SMSG_SPELL_GO), "Vanish's look");
    assert!(game.allies_gone().is_empty(), "the puff plays first");
    game.run.now += 1.0;
    assert_eq!(game.allies_gone(), [packets::destroy_object(ally_guid(900))]);
    assert_eq!(game.guid_of(Unit::Ally(900)), None);
}

/// A druid character shown in a form keeps its form's model over its race's.
#[test]
fn an_ally_in_a_form_shows_its_form() {
    let (mut game, _client) = game_with("human");
    let content = game.run.content.clone();
    let Some(c) = content.player_characters.iter().find(|c| c.form == Some(PlayerForm::Cat)).cloned() else {
        return;
    };
    let faction = content.character_faction(&c).unwrap();
    let ev = summoned(&game, 901, &c.id, Appear::Plain, None);
    let (_, objects, _) = mirrored(&mut game, &[ev]);
    let [SessionEvent::ObjectCreate { fields, .. }] = &creates(&objects)[..] else { panic!("one player") };
    let cat = if faction == Faction::Alliance { 892 } else { 8571 };
    assert_eq!(field_of(fields, field::UNIT_DISPLAYID), Some(cat));
}

/// An ally's swing shows as its own swing at the enemy, its other hits under the look it last
/// cast at enemies; an enemy it taunted strikes it for nothing (absorbed); the looks it puts on
/// an enemy show on it as debuffs while they hold.
#[test]
fn an_ally_s_hits_show_as_its_swings_and_its_looks() {
    let (mut game, _client) = game_with("orc");
    let content = game.run.content.clone();
    let c = content.player_characters_of(Faction::Horde).next().unwrap().clone();
    let ally = ally_guid(902);
    let until = game.run.now + 5.0;
    let events = [
        summoned(&game, 902, &c.id, Appear::Plain, None),
        Event::AllySwing { id: 902, target: 7 },
        Event::SpellGo { caster: Unit::Ally(902), spell: 421, hits: vec![Unit::Enemy(8)], aim: Aim::Unit(Unit::Enemy(8)), ammo: false, reach: 0.0, triggered: false },
        Event::AuraShown { target: Unit::Enemy(8), spell: 118, until, from: Unit::Ally(902) },
    ];
    mirrored(&mut game, &events);
    assert_eq!(game.assist_hit(&c.id, 7, 0xAA, 12, None), Some(packets::attacker_state(ally, 0xAA, 12, false)));
    // the Polymorph look was the last it put on an enemy
    assert_eq!(game.assist_hit(&c.id, 8, 0xBB, 30, None), Some(packets::spell_damage_log(0xBB, ally, 118, 30, 0, false)));
    assert_eq!(game.ally_marks_on(Unit::Enemy(8)).iter().map(|a| a.spell).collect::<Vec<_>>(), [118]);
    game.run.now = until + 0.1;
    assert!(game.ally_marks_on(Unit::Enemy(8)).is_empty(), "gone once its time is up");
    let struck = packets::attacker_state_absorbed(0xCC, ally);
    assert_eq!(struck.opcode, opcode::SMSG_ATTACKERSTATEUPDATE);
    assert!(parse_server(struck.opcode, &struck.body).is_ok(), "the client reads it");
}

/// An ally's leave shows in the tick the run lets it go (how long it stays is the run's alone):
/// it fades at once and goes from the clients when the fade is done.
#[test]
fn an_allys_leave_shows_when_the_run_lets_it_go() {
    let (mut game, _client) = game_with("orc");
    let content = game.run.content.clone();
    let c = content.player_characters_of(Faction::Horde).next().unwrap().clone();
    let ev = summoned(&game, 903, &c.id, Appear::Plain, None);
    mirrored(&mut game, &[ev]);
    let mut out = Vec::new();
    game.present_ally_event(&Event::AllyLeft { id: 903, how: Leave::Fade, look_spell: None, to: None }, &mut out);
    assert!(!out.is_empty(), "its fade shows at once");
    assert!(game.allies_gone().is_empty(), "seen while it fades");
    game.run.now += FADE_OUT;
    assert_eq!(game.allies_gone().len(), 1, "gone once faded");
}

/// The packets the clients get as the run's allies act for `seconds`: each tick's mirror and
/// presentation, enemy `keep` alone on the field and kept alive; and the run's events.
fn ally_packets(game: &mut Game, keep: u32, seconds: f32) -> (Vec<Packet>, Vec<Event>) {
    let (mut out, mut seen) = (Vec::new(), Vec::new());
    let end = game.run.now + seconds;
    for _ in 0..(seconds / 0.05).ceil() as usize + 1 {
        if game.run.now >= end {
            break;
        }
        // a choice screen takes its first offer (the run waits on it)
        while let Some(screen) = &game.run.screen {
            let id = game.run.heroes[screen.hero].id;
            game.run.choose(id, 0);
        }
        assert_eq!(game.run.phase, esurv_sim::game::run::Phase::Running, "at {}", game.run.now);
        game.run.enemies.retain(|e| e.id == keep);
        if let Some(e) = game.run.enemies.iter_mut().find(|e| e.id == keep) {
            (e.health, e.hold_until) = (1e12, 1e9);
        }
        game.run.tick(0.05);
        let events = game.run.drain_events();
        let (_, _, after) = mirrored(game, &events);
        out.extend(after);
        let (all, _) = game.present(&events);
        out.extend(all);
        seen.extend(events);
    }
    (out, seen)
}

/// A rogue ally in melee strafes left and right and jumps as a player's relayed moves the client
/// reads (a strafe's start and stop, a jump's take-off with its flight and its landing); a frost
/// mage's rotation spell with a cast time starts its cast bar before it goes off.
#[test]
fn an_allys_rotation_strafes_jumps_and_casts_as_a_player() {
    let (mut game, _client) = game_with("orc");
    game.run.phase = esurv_sim::game::run::Phase::Running;
    let p = game.run.heroes[0].pos;
    assert!(game.run.spawn_roster_enemy(0, Some((p.0 + 3.0, p.1))));
    let e = game.run.enemies.last().unwrap().id;
    // Mir: Blade Flurry, then his rotation in melee
    assert!(game.run.summon_character(0, "mir"));
    let events = game.run.drain_events();
    mirrored(&mut game, &events);
    game.present(&events);
    let (sent, _) = ally_packets(&mut game, e, 14.0);
    let guid = game.ally_guid_shown(game.run.assists.allies[0].id).unwrap();
    let moves: Vec<(u16, u32, Option<benilla_protocol::JumpInfo>)> = sent
        .iter()
        .filter(|p| [opcode::MSG_MOVE_START_STRAFE_LEFT, opcode::MSG_MOVE_START_STRAFE_RIGHT, opcode::MSG_MOVE_STOP_STRAFE, opcode::MSG_MOVE_JUMP, opcode::MSG_MOVE_FALL_LAND].contains(&p.opcode))
        .map(|p| {
            let ev = decode(parse_server(p.opcode, &p.body).expect("the client reads it"));
            let Some(SessionEvent::UnitMove { guid: g, flags, jump, .. }) = ev.into_iter().next() else { panic!("a relayed move") };
            assert_eq!(g, guid);
            (p.opcode, flags, jump)
        })
        .collect();
    let has = |op: u16| moves.iter().any(|m| m.0 == op);
    assert!(has(opcode::MSG_MOVE_START_STRAFE_LEFT) && has(opcode::MSG_MOVE_START_STRAFE_RIGHT) && has(opcode::MSG_MOVE_STOP_STRAFE), "{moves:?}");
    let jump = moves.iter().find(|m| m.0 == opcode::MSG_MOVE_JUMP).expect("a jump");
    assert_eq!(jump.1, packets::MOVEFLAG_JUMPING);
    assert!(jump.2.is_some_and(|j| j.zspeed == packets::JUMP_TAKE_OFF), "its flight: {jump:?}");
    assert!(has(opcode::MSG_MOVE_FALL_LAND), "it lands");
    // every strafe stops and every jump lands (none left running on the client)
    let count = |op: u16| moves.iter().filter(|m| m.0 == op).count();
    assert_eq!(count(opcode::MSG_MOVE_START_STRAFE_LEFT) + count(opcode::MSG_MOVE_START_STRAFE_RIGHT), count(opcode::MSG_MOVE_STOP_STRAFE));
    assert!(count(opcode::MSG_MOVE_JUMP) - count(opcode::MSG_MOVE_FALL_LAND) <= 1);
    // his casts go off from him
    assert!(sent.iter().any(|p| p.opcode == opcode::SMSG_SPELL_GO));

    // Xaryu: Frostbolt has a cast bar (3 s), then goes off
    let (mut game, _client) = game_with("orc");
    game.run.phase = esurv_sim::game::run::Phase::Running;
    let p = game.run.heroes[0].pos;
    assert!(game.run.spawn_roster_enemy(0, Some((p.0 + 12.0, p.1))));
    let e = game.run.enemies.last().unwrap().id;
    assert!(game.run.summon_character(0, "xaryu"));
    let events = game.run.drain_events();
    mirrored(&mut game, &events);
    game.present(&events);
    let (sent, seen) = ally_packets(&mut game, e, 14.0);
    assert!(sent.iter().any(|p| p.opcode == opcode::SMSG_SPELL_START && p.body.windows(4).any(|w| w == 25304u32.to_le_bytes())), "Frostbolt's cast starts");
    // it goes off once cast (shown as a spell go from it at its aim, an enemy these clients know)
    let started = seen.iter().position(|x| matches!(x, Event::AllyCastStarted { spell: 25304, .. })).unwrap();
    assert!(seen[started..].iter().any(|x| matches!(x, Event::SpellGo { caster: Unit::Ally(_), spell: 25304, .. })), "and goes off");
}

/// An ally channelling (an assist's Blizzard) holds the 1.12 channel look: its channel spell on
/// the storm it channels while the channel runs, cleared when it ends.
#[test]
fn an_ally_s_channel_shows_as_its_channel_spell_on_its_storm() {
    use esurv_sim::game::assist_rotation::Channeling;
    let (mut game, _client) = game_with("human");
    assert!(game.run.summon_character(0, "xaryu"));
    let events = game.run.drain_events();
    mirrored(&mut game, &events);
    let a = &game.run.assists.allies[0];
    let (id, spell) = (a.id, a.rotation.spells.iter().position(|s| s.spell == 10187).unwrap());
    let ally = game.ally_guid_shown(id).unwrap();
    let now = game.run.now;
    game.run.assists.allies[0].rotation.channeling =
        Some(Channeling { spell, area: 77, at: (0.0, 0.0), radius: 8.0, every: 1.0, ticks: 8, ticked: 0, next_tick: now + 1.0, ends: now + 8.0 });
    let mut objects = Vec::new();
    game.mirror_channels(&mut objects);
    let mut f = Fields::new();
    f.u64(field::UNIT_CHANNEL_OBJECT, crate::game::ground_looks::ground_look_guid(77)).u32(field::UNIT_CHANNEL_SPELL, 10187);
    assert_eq!(objects, [ObjectUpdate::Values { guid: ally, fields: f }]);
    game.run.assists.allies[0].rotation.channeling = None;
    let mut objects = Vec::new();
    game.mirror_channels(&mut objects);
    let mut f = Fields::new();
    f.u64(field::UNIT_CHANNEL_OBJECT, 0).u32(field::UNIT_CHANNEL_SPELL, 0);
    assert_eq!(objects, [ObjectUpdate::Values { guid: ally, fields: f }], "the channel ended");
}

/// Noami wields Sulfuras (item 17182: display 29698, a two-handed mace) in both hands: nothing in
/// her off hand, not her class's shield.
#[test]
fn a_two_hander_leaves_the_off_hand_empty() {
    use crate::game::player_enemies::look_item_info;
    let (game, _client) = game_with("human");
    let content = game.run.content.clone();
    let noami = content.player_character("noami").unwrap();
    let shape = game.character_shape(noami, None, 5).unwrap();
    let worn = |slot: u8| shape.items.iter().find(|(s, _)| *s == slot).and_then(|(_, e)| look_item_info(*e));
    let main = worn(esurv_content::Hand::MainHand.equip_slot()).unwrap();
    assert_eq!((main.display_id, main.inventory_type), (29698, 17));
    assert!(worn(esurv_content::Hand::OffHand.equip_slot()).is_none(), "no shield");
}
