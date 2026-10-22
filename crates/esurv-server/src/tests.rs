//! The whole server against benilla's own client code: logon, the zone list, the character
//! screen, entering the run, enemies coming, logging out.

use std::sync::Arc;
use std::time::{Duration, Instant};

use benilla_protocol::{
    decode, logon, CharCreateReq, EntityKind, ServerPacket, SessionEvent, WorldSession,
};
use esurv_content::Content;

use super::*;

fn server() -> (ServerHandle, Arc<Content>) {
    server_with(Arc::new(Saves::in_memory()))
}

/// [`server`] keeping its heroes in `saves`.
fn server_with(saves: Arc<Saves>) -> (ServerHandle, Arc<Content>) {
    let content =
        Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
    let config = ServerConfig {
        content: Arc::new(esurv_content::LiveContent::fixed(content.clone())),
        saves,
        data: None,
        password: "local".into(),
        bind_host: "127.0.0.1".into(),
        public_host: "127.0.0.1".into(),
        realmd_port: 0,
        autostart: false,
        zone: None,
        granted: Vec::new(),
        move_log: None,
    };
    (start(config).unwrap(), content)
}

/// Reads packets until `want` returns something, or fails after `limit`.
fn wait_for<T>(
    s: &mut WorldSession,
    limit: Duration,
    mut want: impl FnMut(&ServerPacket) -> Option<T>,
) -> T {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if let Some(t) = s.recv().ok().and_then(|p| want(&p)) {
            return t;
        }
    }
    panic!("nothing wanted arrived within {limit:?}");
}

/// [`wait_for`] on the events the client makes of each packet.
fn wait_for_event<T>(
    s: &mut WorldSession,
    limit: Duration,
    mut want: impl FnMut(&[SessionEvent]) -> Option<T>,
) -> T {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if let Some(t) = s.recv().ok().and_then(|p| want(&decode(p))) {
            return t;
        }
    }
    panic!("nothing wanted arrived within {limit:?}");
}

#[test]
fn a_benilla_client_makes_a_hero_enters_elwynn_and_enemies_come() {
    let (server, content) = server();
    let zone = &content.zones[0];
    let logon = logon(&server.realmd.to_string(), "anyone", "local").expect("logon");
    let names: Vec<&str> = logon.realms.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        [REALM_NAME],
        "one realm: the zone is chosen with the hero"
    );

    let mut s = WorldSession::connect(
        logon.realms[0].address.as_str(),
        "anyone",
        logon.session_key,
    )
    .expect("world session");
    assert!(s.char_enum().unwrap().is_empty());
    let req = CharCreateReq {
        name: "ana".into(),
        race: 1,
        class: 8,
        gender: 1,
        skin: 1,
        face: 2,
        hair_style: 3,
        hair_color: 4,
        facial_hair: 0,
    };
    assert_eq!(
        s.create_character(&req).unwrap(),
        packets::code::CHAR_CREATE_SUCCESS
    );
    assert_eq!(
        s.create_character(&req).unwrap(),
        packets::code::CHAR_CREATE_NAME_IN_USE
    );
    let heroes = s.char_enum().unwrap();
    assert_eq!(
        (
            heroes[0].name.as_str(),
            heroes[0].race,
            heroes[0].class,
            heroes[0].hair_color
        ),
        ("Ana", 1, 8, 4)
    );
    let guid = heroes[0].guid;

    s.player_login(guid).unwrap();
    let start = wait_for(&mut s, Duration::from_secs(5), |p| match p {
        ServerPacket::LoginVerifyWorld { map, position, .. } => {
            Some((*map, [position.x, position.y, position.z]))
        }
        _ => None,
    });
    assert_eq!(
        start,
        (zone.map_id, [zone.spawn.x, zone.spawn.y, zone.spawn.z]),
        "at the zone's spawn"
    );
    wait_for_event(&mut s, Duration::from_secs(5), |events| {
        events.iter().any(|e| matches!(e, SessionEvent::ObjectCreate { guid: g, kind: EntityKind::Player, .. } if *g == guid)).then_some(())
    });

    // the first step starts the run; the opening enemies appear and walk at the hero
    let step = [start.1[0] + 1.0, start.1[1], start.1[2]];
    s.start_forward(step, 0.0).unwrap();
    s.heartbeat(step, 0.0).unwrap();
    let enemy = wait_for_event(&mut s, Duration::from_secs(5), |events| {
        events.iter().find_map(|e| match e {
            SessionEvent::ObjectCreate {
                guid,
                kind: EntityKind::Unit,
                ..
            } => Some(*guid),
            _ => None,
        })
    });
    let roster_entries: Vec<u32> = zone.enemies.iter().map(|e| e.entry).collect();
    assert!(
        roster_entries.contains(&(((enemy >> 24) & 0xFF_FFFF) as u32)),
        "a zone enemy"
    );
    let towards = wait_for(&mut s, Duration::from_secs(10), |p| match p {
        ServerPacket::MonsterMove {
            path, stop: false, ..
        } if !path.is_empty() => {
            let (first, last) = (path[0], path[path.len() - 1]);
            let d = |v: &benilla_protocol::wire::Vector3d| {
                ((v.x - step[0]).powi(2) + (v.y - step[1]).powi(2)).sqrt()
            };
            Some(d(&last) < d(&first))
        }
        _ => None,
    });
    assert!(towards, "an enemy walks at the hero");

    s.logout(Duration::from_secs(5))
        .expect("back to the character screen");
    assert_eq!(s.char_enum().unwrap().len(), 1);
}

#[test]
fn a_session_without_a_logon_is_refused() {
    let (server, _) = server();
    let result = WorldSession::connect(server.world, "nobody", [7; 40]);
    assert!(result.is_err());
}

/// A class enters only the zones it has opened: a new mage may play Elwynn but not `duskwood`.
#[test]
fn a_zone_the_class_has_not_opened_is_refused() {
    let saves = Arc::new(Saves::in_memory());
    let (server, content) = server_with(saves.clone());
    let logon = logon(&server.realmd.to_string(), "wanderer", "local").expect("logon");
    let mut s = WorldSession::connect(
        logon.realms[0].address.as_str(),
        "wanderer",
        logon.session_key,
    )
    .expect("world session");
    let req = CharCreateReq {
        name: "Vel".into(),
        race: 1,
        class: 8,
        gender: 0,
        skin: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        facial_hair: 0,
    };
    assert_eq!(
        s.create_character(&req).unwrap(),
        packets::code::CHAR_CREATE_SUCCESS
    );
    let me = s.char_enum().unwrap()[0].guid;
    assert!(saves.choose_zone(me as u32, &content.zones[1].id));
    s.player_login(me).unwrap();
    let reason = wait_for(&mut s, Duration::from_secs(5), |p| match p {
        ServerPacket::CharacterLoginFailed { result } => Some(*result),
        _ => None,
    });
    assert_eq!(reason, packets::code::CHAR_LOGIN_DISABLED);
}

/// The zone chosen for a hero is where its row stands and where it enters, once its class has
/// opened it.
#[test]
fn a_hero_enters_the_zone_chosen_for_it() {
    let saves = Arc::new(Saves::in_memory());
    let (server, content) = server_with(saves.clone());
    let logon = logon(&server.realmd.to_string(), "chooser", "local").expect("logon");
    let mut s = WorldSession::connect(
        logon.realms[0].address.as_str(),
        "chooser",
        logon.session_key,
    )
    .expect("world session");
    let req = CharCreateReq {
        name: "Mira".into(),
        race: 1,
        class: 8,
        gender: 1,
        skin: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        facial_hair: 0,
    };
    assert_eq!(
        s.create_character(&req).unwrap(),
        packets::code::CHAR_CREATE_SUCCESS
    );
    let me = s.char_enum().unwrap()[0].guid;
    // a win in Elwynn opens `duskwood` to every mage
    let won = crate::save::FinishedRun {
        won: true,
        first: true,
        opens_zone: true,
        ..Default::default()
    };
    saves.finish_run(&content, me as u32, &won).unwrap();
    let duskwood = &content.zones[1];
    assert!(saves.choose_zone(me as u32, &duskwood.id));
    let row = &s.char_enum().unwrap()[0];
    assert_eq!(
        (row.map, row.position.x, row.position.y),
        (duskwood.map_id, duskwood.spawn.x, duskwood.spawn.y),
        "the row stands in the chosen zone"
    );
    s.player_login(me).unwrap();
    let start = wait_for(&mut s, Duration::from_secs(5), |p| match p {
        ServerPacket::LoginVerifyWorld { map, position, .. } => Some((*map, position.x)),
        _ => None,
    });
    assert_eq!(start, (duskwood.map_id, duskwood.spawn.x));
}

/// What a client sees of a fight: the action bar, the hero's spells going off with their numbers
/// and cooldowns, enemies swinging back, the opening line, the stats line, and the dead lying
/// down before they are removed.
#[test]
fn the_client_sees_the_fight() {
    let (server, content) = server();
    let logon = logon(&server.realmd.to_string(), "fighter", "local").expect("logon");
    let mut s = WorldSession::connect(
        logon.realms[0].address.as_str(),
        "fighter",
        logon.session_key,
    )
    .expect("world session");
    let req = CharCreateReq {
        name: "Bo".into(),
        race: 1,
        class: 8,
        gender: 0,
        skin: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        facial_hair: 0,
    };
    assert_eq!(
        s.create_character(&req).unwrap(),
        packets::code::CHAR_CREATE_SUCCESS
    );
    let me = s.char_enum().unwrap()[0].guid;
    s.player_login(me).unwrap();
    let mage = content.classes.iter().find(|c| c.id == "mage").unwrap();
    let first_spell = content
        .ability(mage.default_start_spell())
        .unwrap()
        .look_spell;
    let human = content.races.iter().find(|r| r.id == "human").unwrap();
    let race_spell = content.ability(&human.spell).unwrap().look_spell;
    let bar = wait_for(&mut s, Duration::from_secs(5), |p| match p {
        ServerPacket::ActionButtons { buttons } => Some(
            buttons
                .iter()
                .map(|b| (b.slot, b.action))
                .collect::<Vec<_>>(),
        ),
        _ => None,
    });
    assert_eq!(
        bar,
        [(0, first_spell), (11, race_spell)],
        "the starting spell first, the race spell on the last button"
    );

    let spawn = &content.zones[0].spawn;
    s.start_forward([spawn.x + 1.0, spawn.y, spawn.z], 0.0)
        .unwrap();
    let (mut cast, mut number, mut cooldown, mut swing, mut opening, mut stats) =
        (false, false, false, false, false, false);
    let mut dead: std::collections::HashMap<u64, Instant> = Default::default();
    let mut removed_after = None;
    let (mut cache, mut orbs, mut orb_flew) = (false, std::collections::HashSet::new(), false);
    let deadline = Instant::now() + Duration::from_secs(40);
    while Instant::now() < deadline
        && !(cast
            && number
            && cooldown
            && swing
            && opening
            && stats
            && removed_after.is_some()
            && cache
            && orb_flew)
    {
        let Ok(p) = s.recv() else { continue };
        match &p {
            ServerPacket::SpellGo(go) if go.caster == me && go.spell_id == first_spell => {
                cast = true
            }
            ServerPacket::SpellDamageLog(d)
                if d.attacker == me && d.spell_id == first_spell && d.damage > 0 =>
            {
                number = true
            }
            ServerPacket::SpellCooldownList { caster, cooldowns }
                if *caster == me && cooldowns.iter().any(|c| c.0 == first_spell && c.1 > 0) =>
            {
                cooldown = true
            }
            ServerPacket::AttackerState(a) if a.victim == me && a.damage > 0 => swing = true,
            ServerPacket::MessageChat(m)
                if m.chat_type == 0x5A && m.text.contains("hold out as long as you can") =>
            {
                opening = true
            }
            ServerPacket::MessageChat(m) if m.is_addon() && m.text.starts_with("ESURV\thealth=") => {
                stats = true
            }
            ServerPacket::DestroyObject { guid } => {
                if let Some(at) = dead.get(guid) {
                    removed_after = Some(at.elapsed());
                }
            }
            ServerPacket::MonsterMove { guid, path, .. }
                if orbs.contains(guid) && !path.is_empty() =>
            {
                orb_flew = true
            }
            _ => {}
        }
        for e in decode(p) {
            if let SessionEvent::ObjectCreate {
                guid,
                kind,
                display_id,
                ..
            } = &e
            {
                cache |= *kind == EntityKind::GameObject && *display_id == Some(259);
                if *kind == EntityKind::Unit
                    && (guid >> 24) & 0xFF_FFFF == u64::from(game::ORB_ENTRY)
                {
                    orbs.insert(*guid);
                }
            }
            if let SessionEvent::ObjectValues { guid, fields } = e {
                if fields
                    .raw_fields()
                    .any(|(i, v)| i == update::field::UNIT_HEALTH && v == 0)
                {
                    dead.entry(guid).or_insert_with(Instant::now);
                }
            }
        }
    }
    assert!(cast, "the hero's spell goes off");
    assert!(number, "with its damage number");
    assert!(cooldown, "and its cooldown on the bar");
    assert!(swing, "an enemy swings at the hero");
    assert!(opening, "the run opens with its line");
    assert!(stats, "the stats line reaches the character pane");
    assert!(cache, "the run's caches stand on the field");
    assert!(!orbs.is_empty(), "the dead drop experience wisps");
    assert!(orb_flew, "which fly to the hero");
    let lay = removed_after.expect("a dead enemy is removed");
    assert!(
        lay >= Duration::from_millis(1300),
        "after lying down a while ({lay:?})"
    );
}

/// The game's own screens over addon lines, with benilla's client code: after `hello` the client
/// is told its HUD and its class's progress; it pauses the run, and leaving ends it with the
/// tally.
#[test]
fn a_client_with_screens_follows_the_run_and_leaves_it() {
    use benilla_protocol::messages::CHAT_TYPE_PARTY;
    use esurv_protocol::{Ending, Phase, Reader, Screens, ToClient, ToServer, Wire};

    let (server, content) = server();
    let logon = logon(&server.realmd.to_string(), "screens", "local").expect("logon");
    let mut s = WorldSession::connect(
        logon.realms[0].address.as_str(),
        "screens",
        logon.session_key,
    )
    .expect("world session");
    let req = CharCreateReq {
        name: "Cy".into(),
        race: 1,
        class: 8,
        gender: 0,
        skin: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        facial_hair: 0,
    };
    assert_eq!(
        s.create_character(&req).unwrap(),
        packets::code::CHAR_CREATE_SUCCESS
    );
    let me = s.char_enum().unwrap()[0].guid;
    s.player_login(me).unwrap();
    let ask = |s: &mut WorldSession, request: ToServer| {
        for line in request.lines() {
            s.send_addon_message(CHAT_TYPE_PARTY, None, &line.to_string())
                .unwrap();
        }
    };
    ask(
        &mut s,
        ToServer::Hello {
            version: esurv_protocol::VERSION,
            screens: Screens {
                cards: true,
                run_end: true,
                hud: true,
            },
        },
    );
    /// What the client sees that this test follows.
    #[derive(Debug)]
    enum Seen {
        Told(ToClient),
        Rooted(bool),
        LoggedOut,
    }
    let mut reader = Reader::<ToClient>::default();
    let mut next = |s: &mut WorldSession| {
        wait_for(s, Duration::from_secs(5), |p| match p {
            ServerPacket::MessageChat(m) if m.is_addon() => reader.read(&m.text).map(Seen::Told),
            ServerPacket::MoveMode {
                mode: benilla_protocol::MoveMode::Root,
                apply,
                ..
            } => Some(Seen::Rooted(*apply)),
            ServerPacket::LogoutComplete => Some(Seen::LoggedOut),
            _ => None,
        })
    };
    let (mut ready, mut progress) = (false, false);
    loop {
        match next(&mut s) {
            Seen::Told(ToClient::Ready(r)) => ready = r.hero == me && r.name == "Cy",
            Seen::Told(ToClient::Progress(p)) => progress = p.class == "mage",
            Seen::Told(ToClient::Hud(h)) => {
                assert_eq!((h.hero, h.phase), (me, Phase::Waiting));
                break;
            }
            _ => {}
        }
    }
    assert!(ready && progress, "who it is and its armory come first");

    let spawn = &content.zones[0].spawn;
    s.start_forward([spawn.x + 1.0, spawn.y, spawn.z], 0.0)
        .unwrap();
    while !matches!(next(&mut s), Seen::Told(ToClient::Hud(h)) if h.phase == Phase::Running) {}
    // a pause roots the hero and stops the clock
    ask(&mut s, ToServer::Pause);
    let (mut rooted, mut paused) = (false, false);
    while !(rooted && paused) {
        match next(&mut s) {
            Seen::Rooted(true) => rooted = true,
            Seen::Told(ToClient::Hud(h)) if h.phase == Phase::Paused => paused = true,
            _ => {}
        }
    }
    // leaving tells the end of the run, then takes the hero out of the world
    ask(&mut s, ToServer::Leave);
    let end = loop {
        if let Seen::Told(ToClient::RunEnd(e)) = next(&mut s) {
            break e;
        }
    };
    assert_eq!((end.hero, end.ending, end.won), (me, Ending::Left, false));
    while !matches!(next(&mut s), Seen::LoggedOut) {}
    assert_eq!(
        s.char_enum().unwrap().len(),
        1,
        "back at the character screen"
    );
}

/// Creates one hero of each pair `races.ron` adds through a benilla
/// client and reads them back off the character list.
fn create_the_added_pairs(server: &ServerHandle, content: &Content) -> Vec<u8> {
    let logon = logon(&server.realmd.to_string(), "anyone", "local").expect("logon");
    let mut s = WorldSession::connect(
        logon.realms[0].address.as_str(),
        "anyone",
        logon.session_key,
    )
    .expect("world session");
    let pairs = content.added_pairs();
    let names = ["ana", "bea", "cid", "dax", "eli", "fay"];
    let mut codes = Vec::new();
    for (p, name) in pairs.iter().zip(names) {
        codes.push(
            s.create_character(&CharCreateReq {
                name: name.into(),
                race: p.race,
                class: p.class,
                gender: 0,
                skin: 0,
                face: 0,
                hair_style: 0,
                hair_color: 0,
                facial_hair: 0,
            })
            .unwrap(),
        );
    }
    let listed: Vec<(u8, u8)> = s
        .char_enum()
        .unwrap()
        .iter()
        .map(|h| (h.race, h.class))
        .collect();
    let made: Vec<(u8, u8)> = pairs
        .iter()
        .zip(&codes)
        .filter(|(_, &c)| c == packets::code::CHAR_CREATE_SUCCESS)
        .map(|(p, _)| (p.race, p.class))
        .collect();
    assert_eq!(listed, made, "the character list holds what was made");
    codes
}

#[test]
fn the_added_race_and_class_pairs_are_made_and_listed() {
    let (server, content) = server();
    let codes = create_the_added_pairs(&server, &content);
    assert_eq!(codes, [packets::code::CHAR_CREATE_SUCCESS; 6]);
}

/// With the player's data open, the stock pairs come from it: the added pairs pass, a pair the
/// create screen never offers (`tauren` `mage`) is refused.
#[test]
fn a_pair_the_create_screen_does_not_offer_is_refused() {
    let dir = benilla_formats::wow_data_or_skip!();
    let content =
        Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
    let server = start(ServerConfig {
        content: Arc::new(esurv_content::LiveContent::fixed(content.clone())),
        saves: Arc::new(Saves::in_memory()),
        data: Some(crate::ground::GameData::open(&dir, &content).unwrap()),
        password: "local".into(),
        bind_host: "127.0.0.1".into(),
        public_host: "127.0.0.1".into(),
        realmd_port: 0,
        autostart: false,
        zone: None,
        granted: Vec::new(),
        move_log: None,
    })
    .unwrap();
    let codes = create_the_added_pairs(&server, &content);
    assert_eq!(codes, [packets::code::CHAR_CREATE_SUCCESS; 6]);
    let logon = logon(&server.realmd.to_string(), "anyone", "local").expect("logon");
    let mut s = WorldSession::connect(
        logon.realms[0].address.as_str(),
        "anyone",
        logon.session_key,
    )
    .expect("world session");
    let tauren_mage = CharCreateReq {
        name: "moo".into(),
        race: 6,
        class: 8,
        gender: 0,
        skin: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        facial_hair: 0,
    };
    assert_eq!(
        s.create_character(&tauren_mage).unwrap(),
        packets::code::CHAR_CREATE_DISABLED
    );
    let tauren_shaman = CharCreateReq {
        class: 7,
        ..tauren_mage
    };
    assert_eq!(
        s.create_character(&tauren_shaman).unwrap(),
        packets::code::CHAR_CREATE_SUCCESS
    );
}

/// A faction's camp holds nine heroes: the tenth of one faction is
/// refused with the stock realm-limit reply, while the other faction may still make heroes.
#[test]
fn a_faction_holds_nine_heroes() {
    let (server, _) = server();
    let logon = logon(&server.realmd.to_string(), "anyone", "local").expect("logon");
    let mut s = WorldSession::connect(
        logon.realms[0].address.as_str(),
        "anyone",
        logon.session_key,
    )
    .expect("world session");
    let hero = |name: &str, race: u8| CharCreateReq {
        name: name.into(),
        race,
        class: 1,
        gender: 0,
        skin: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        facial_hair: 0,
    };
    for name in ["Ga", "Gb", "Gc", "Gd", "Ge", "Gf", "Gg", "Gh", "Gi"] {
        assert_eq!(
            s.create_character(&hero(name, 2)).unwrap(),
            packets::code::CHAR_CREATE_SUCCESS,
            "orc {name}"
        );
    }
    assert_eq!(
        s.create_character(&hero("Gj", 2)).unwrap(),
        packets::code::CHAR_CREATE_SERVER_LIMIT,
        "a tenth Horde hero"
    );
    assert_eq!(
        s.create_character(&hero("Ha", 1)).unwrap(),
        packets::code::CHAR_CREATE_SUCCESS,
        "the Alliance still has room"
    );
    assert_eq!(s.char_enum().unwrap().len(), 10);
}

/// A hero enters its run wearing its class's gear: its player's visible-item fields hold the worn
/// pieces' entries (a look chosen for a slot as an entry of its own), it holds its weapon in hand,
/// and the item query answers each entry as the character list draws it, by the gear's own names.
#[test]
fn a_hero_wears_its_class_gear_in_the_run() {
    use esurv_sim::game::gear::ARMOR_SLOTS;
    use esurv_sim::game::rewards::gear_item_entry;

    let saves = Arc::new(Saves::in_memory());
    let (server, content) = server_with(saves.clone());
    let logon = logon(&server.realmd.to_string(), "wearer", "local").expect("logon");
    let mut s = WorldSession::connect(
        logon.realms[0].address.as_str(),
        "wearer",
        logon.session_key,
    )
    .expect("world session");
    let req = CharCreateReq {
        name: "Bram".into(),
        race: 1,
        class: 1,
        gender: 0,
        skin: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        facial_hair: 0,
    };
    assert_eq!(
        s.create_character(&req).unwrap(),
        packets::code::CHAR_CREATE_SUCCESS
    );
    let me = s.char_enum().unwrap()[0].guid;
    // a Duskwood helm shown in the Gurubashi look, a Sunken Temple weapon in its own
    let warrior = content.class("warrior").unwrap().wow_class_id;
    let helm = gear_item_entry(warrior, 0, 1);
    let helm_look = gear_item_entry(warrior, 0, 6); // Gurubashi
    let blade = gear_item_entry(warrior, ARMOR_SLOTS, 4); // Sunken Temple
    for e in [helm, helm_look, blade] {
        saves.add_to_collection(&content, e);
    }
    assert!(saves.wear(&content, "warrior", helm));
    assert!(saves.wear(&content, "warrior", blade));
    let arena = content.gear_set("warrior").unwrap().look_of("arena", 0).unwrap() as u32;
    assert!(saves.set_look(&content, "warrior", 0, Some(arena)));
    let progress = saves.class_progress(&content, "warrior");
    // the weapon shown is the weapon loadout's (the first, the two-hander), in the worn blade's tier
    let worn = crate::looks::armed(&content, "warrior", &progress, crate::looks::worn_looks(&content, "warrior", &progress));
    let two_hander = content.loadout("warrior", None).unwrap().weapon(esurv_content::Hand::MainHand).unwrap();
    assert_eq!(worn[1].display_id, two_hander.zone_display("sunken_temple", 0));

    s.player_login(me).unwrap();
    let (fields, items) = wait_for_event(&mut s, Duration::from_secs(5), |events| {
        let fields = events.iter().find_map(|e| match e {
            SessionEvent::ObjectCreate { guid, fields, .. } if *guid == me => Some(fields.clone()),
            _ => None,
        })?;
        // the item objects come along with the hero
        let items: Vec<(u64, Option<u32>)> = events
            .iter()
            .filter_map(|e| match e {
                SessionEvent::ItemCreate { guid, fields, .. } => Some((
                    *guid,
                    fields.raw_fields().find(|&(i, _)| i == 3).map(|(_, v)| v),
                )),
                _ => None,
            })
            .collect();
        Some((fields, items))
    });
    let mut shown = Vec::new();
    for w in &worn {
        let entry = fields.player_visible_item_entry(w.equip_slot);
        assert_eq!(entry, Some(w.entry), "slot {}", w.equip_slot);
        shown.push(w.entry);
    }
    assert_eq!(shown.len(), 2);
    assert_ne!(shown[0], helm, "the helm shows its chosen look");
    assert_eq!(
        (0..19)
            .filter(|&i| fields.player_visible_item_entry(i).is_some())
            .count(),
        2,
        "without the player's data, no starting outfit under it"
    );
    assert_eq!(fields.unit_sheath_state(), Some(1), "weapons in hand");
    // the character pane's slots: an item per worn piece, held in its equipment slot
    assert_eq!(items.len(), worn.len());
    for w in &worn {
        let held = fields
            .player_inv_slot(w.equip_slot)
            .expect("an item in the slot");
        assert!(benilla_protocol::guid::is_item(held));
        assert!(items.contains(&(held, Some(w.entry))), "{items:?}");
    }

    let set = content.gear_set("warrior").unwrap();
    for (w, name) in worn.iter().zip([
        esurv_sim::game::gear::zone_piece_name(&content, set, 0, 1),
        // a loadout weapon's look tells only its kind and look
        Some(String::new()),
    ]) {
        s.item_query(w.entry, 0).unwrap();
        let info = wait_for(&mut s, Duration::from_secs(5), |p| match p {
            ServerPacket::ItemQueryResponse { entry, info } if *entry == w.entry => {
                Some(info.clone())
            }
            _ => None,
        })
        .expect("a known entry");
        assert_eq!(
            (info.display_info_id, info.inventory_type),
            (w.display_id, w.inventory_type)
        );
        assert_eq!(Some(info.name), name);
    }
    s.item_query(25, 0).unwrap();
    let stock = wait_for(&mut s, Duration::from_secs(5), |p| match p {
        ServerPacket::ItemQueryResponse { entry: 25, info } => Some(info.is_none()),
        _ => None,
    });
    assert!(
        stock,
        "no starting outfit to answer from without the player's data"
    );
}

/// The character list arms each hero with its class's chosen weapon loadout, so the camp's
/// figure holds it (roadmap R3): the warrior's two-hander alone in the
/// main hand, then his sword and shield once chosen.
#[test]
fn the_character_list_holds_the_chosen_loadout() {
    let saves = Arc::new(Saves::in_memory());
    let (server, content) = server_with(saves.clone());
    let logon = logon(&server.realmd.to_string(), "armed", "local").expect("logon");
    let mut s = WorldSession::connect(logon.realms[0].address.as_str(), "armed", logon.session_key)
        .expect("world session");
    let req = CharCreateReq {
        name: "Bram".into(),
        race: 1,
        class: 1,
        gender: 0,
        skin: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        facial_hair: 0,
    };
    assert_eq!(s.create_character(&req).unwrap(), packets::code::CHAR_CREATE_SUCCESS);
    let hands = |s: &mut WorldSession| {
        let c = &s.char_enum().unwrap()[0];
        (c.equipment[15].display_id, c.equipment[16].display_id)
    };
    let look = |id: &str, hand| content.loadout("warrior", Some(id)).unwrap().weapon(hand).unwrap().zone_display("elwynn", 0);
    use esurv_content::Hand;
    assert_eq!(hands(&mut s), (look("two_hander", Hand::MainHand), 0));
    assert!(saves.choose_loadout(&content, "warrior", "shield"));
    assert_eq!(hands(&mut s), (look("shield", Hand::MainHand), look("shield", Hand::OffHand)));
}
