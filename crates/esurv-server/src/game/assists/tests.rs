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
