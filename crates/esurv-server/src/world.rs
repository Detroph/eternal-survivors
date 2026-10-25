//! The world server: one listener for every zone, and per connection the session handshake, the
//! character screen (heroes from the save) and the hero in the run, in the zone chosen for it.

use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use benilla_protocol::messages::opcode;
use benilla_srp::HeaderCrypto;
use rand::RngCore;
use sha1::{Digest, Sha1};
use esurv_content::{Content, LiveContent};
use esurv_protocol::{Reader, ToServer};

use crate::game::{player_guid, Game, Input, Player};
use crate::ground::GameData;
use crate::packets::{self, code, CreatureInfo, RosterEntry, RANK_BOSS, RANK_ELITE, RANK_NORMAL, RANK_RARE};
use crate::realmd::SessionKeys;
use crate::save::{SavedHero, Saves, MAX_HEROES_PER_FACTION};
use crate::wire::{Body, PacketReader, PacketWriter};
use crate::{client, game};

/// What every world connection shares.
pub struct Shared {
    /// The content: a run takes it as it is when the run starts and keeps it to its end; the
    /// character screen reads it as it is now (dev builds load it again as its files change).
    pub content: Arc<LiveContent>,
    pub saves: Arc<Saves>,
    /// The player's 1.12.1 data, for ground heights and missile speeds; without it the ground is
    /// flat and every spell hits at once (tests).
    pub data: Option<Arc<GameData>>,
    pub keys: SessionKeys,
    /// Runs start as the hero enters, without waiting for a first move (a dev switch).
    pub autostart: bool,
    /// Every hero enters this zone, whatever was chosen for it (a dev switch).
    pub zone: Option<usize>,
    /// What every hero is given from the start of its run (a dev switch; `Game::grant`).
    pub granted: Vec<String>,
    /// Where every hero's moves are logged (a dev switch; `Game::log_moves_to`).
    pub move_log: Option<std::path::PathBuf>,
}

impl Shared {
    /// The content as it is now.
    pub fn content(&self) -> Arc<Content> {
        self.content.current()
    }

    /// The zone (index in `content`) hero `h`'s next run is in: the dev switch's, else the one
    /// chosen for it (or entered last), else the first.
    pub fn zone_of(&self, content: &Content, h: &SavedHero) -> usize {
        self.zone
            .filter(|&z| z < content.zones.len())
            .or_else(|| {
                let id = h.zone.as_deref()?;
                content.zones.iter().position(|z| z.id == id)
            })
            .unwrap_or(0)
    }
}

/// The content a connection answers from: its run's while its hero is in one (a run keeps the
/// content it started with), else the content as it is now.
fn content_in_use(world: &Option<InWorld>, shared: &Shared) -> Arc<Content> {
    world.as_ref().map_or_else(|| shared.content(), |w| w.content.clone())
}

/// Accepts the clients, one thread each.
pub fn serve(listener: TcpListener, shared: Arc<Shared>) {
    for stream in listener.incoming().flatten() {
        let shared = shared.clone();
        std::thread::spawn(move || {
            let _ = session(stream, &shared);
        });
    }
}

/// The client moves its own hero with these.
fn is_movement(op: u16) -> bool {
    matches!(
        op,
        opcode::MSG_MOVE_START_FORWARD..=opcode::MSG_MOVE_SET_WALK_MODE
    ) || matches!(
        op,
        opcode::MSG_MOVE_FALL_LAND
            | opcode::MSG_MOVE_START_SWIM
            | opcode::MSG_MOVE_STOP_SWIM
            | opcode::MSG_MOVE_SET_FACING
            | opcode::MSG_MOVE_SET_PITCH
            | opcode::MSG_MOVE_HEARTBEAT
    )
}

/// A hero in the run: how to reach the host.
struct InWorld {
    hero: u32,
    guid: u64,
    host: Sender<Input>,
    /// The content the run started with, which it keeps to its end.
    content: Arc<Content>,
    /// Counts the hero into the world for the save while it is in the run.
    _mark: crate::save::InWorldMark,
}

impl InWorld {
    /// Leaves the run; returns once the host sends this client nothing more.
    fn leave(self) {
        let (done, wait) = mpsc::channel();
        if self
            .host
            .send(Input::Left {
                hero: self.hero,
                done,
            })
            .is_ok()
        {
            let _ = wait.recv();
        }
    }
}

fn session(stream: TcpStream, shared: &Shared) -> Result<()> {
    stream.set_nodelay(true)?;
    let writer = Arc::new(Mutex::new(PacketWriter::new(stream.try_clone()?)));
    let mut reader = PacketReader::new(stream);
    let server_seed = rand::thread_rng().next_u32();
    writer
        .lock()
        .unwrap()
        .send(&packets::auth_challenge(server_seed))?;

    // the client proves it holds the session key realmd gave this account
    let (op, body) = reader.recv()?;
    if op != opcode::CMSG_AUTH_SESSION {
        bail!("expected CMSG_AUTH_SESSION, got {op:#x}");
    }
    let auth = client::auth_session(&body)?;
    let account = auth.account.to_uppercase();
    let Some(key) = shared.keys.lock().unwrap().get(&account).copied() else {
        writer.lock().unwrap().send(&packets::auth_refused(
            benilla_protocol::messages::AUTH_UNKNOWN_ACCOUNT,
        ))?;
        return Ok(());
    };
    let expected: [u8; 20] = Sha1::new()
        .chain_update(account.as_bytes())
        .chain_update(0u32.to_le_bytes())
        .chain_update(auth.client_seed.to_le_bytes())
        .chain_update(server_seed.to_le_bytes())
        .chain_update(key)
        .finalize()
        .into();
    if expected != auth.proof {
        writer.lock().unwrap().send(&packets::auth_refused(
            benilla_protocol::messages::AUTH_FAILED,
        ))?;
        return Ok(());
    }
    let (encrypt, decrypt) = HeaderCrypto::from_session_key(key).split();
    writer.lock().unwrap().encrypt_with(encrypt);
    reader.decrypt_with(decrypt);
    writer.lock().unwrap().send(&packets::auth_ok())?;

    let mut world: Option<InWorld> = None;
    // the game's own screens ask through addon lines (esurv-protocol)
    let mut requests = Reader::<ToServer>::default();
    let result = (|| -> Result<()> {
        loop {
            let (op, body) = reader.recv()?;
            let reply = |p: crate::wire::Packet| writer.lock().unwrap().send(&p).context("sending");
            match op {
                opcode::CMSG_CHAR_ENUM => reply(packets::char_enum(&roster(shared)))?,
                opcode::CMSG_CHAR_CREATE => reply(packets::char_create_result(create_hero(
                    shared,
                    &client::char_create(&body)?,
                )))?,
                opcode::CMSG_CHAR_DELETE => {
                    let ok =
                        u32::try_from(client::guid(&body)?).is_ok_and(|g| shared.saves.delete(g));
                    reply(packets::char_delete_result(if ok {
                        code::CHAR_DELETE_SUCCESS
                    } else {
                        code::CHAR_DELETE_FAILED
                    }))?;
                }
                opcode::CMSG_PLAYER_LOGIN if world.is_none() => {
                    let guid = client::guid(&body)?;
                    match enter_world(shared, guid, writer.clone()) {
                        Ok(w) => {
                            requests.clear();
                            world = Some(w);
                        }
                        Err(code) => reply(packets::character_login_failed(code))?,
                    }
                }
                opcode::CMSG_MESSAGECHAT => {
                    let asked = client::message_chat(&body)
                        .ok()
                        .filter(|line| line.is_addon())
                        .and_then(|line| requests.read(&line.text));
                    if let (Some(w), Some(request)) = (&world, asked) {
                        let leaving = request == ToServer::Leave;
                        let _ = w.host.send(Input::Asked {
                            hero: w.hero,
                            request,
                        });
                        // leaving the run takes the hero out of the world, once the host has paid
                        // it out and told it
                        if leaving {
                            if let Some(w) = world.take() {
                                w.leave();
                            }
                            reply(packets::logout_complete())?;
                        }
                    }
                }
                opcode::CMSG_LOGOUT_REQUEST => {
                    if let Some(w) = world.take() {
                        w.leave();
                    }
                    for p in packets::logout() {
                        reply(p)?;
                    }
                }
                opcode::CMSG_PING => reply(packets::pong(client::ping(&body)?))?,
                // the tips the player has seen, or turned off or on in the options, kept in the save
                opcode::CMSG_TUTORIAL_FLAG => shared.saves.see_tip(client::tip_seen(&body)?),
                opcode::CMSG_TUTORIAL_CLEAR => shared.saves.see_every_tip(true),
                opcode::CMSG_TUTORIAL_RESET => shared.saves.see_every_tip(false),
                opcode::CMSG_QUERY_TIME => {
                    let unix = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs() as u32);
                    reply(packets::query_time_response(unix))?;
                }
                opcode::CMSG_NAME_QUERY => {
                    // a hero, or a ghostly copy of one (named as its hero)
                    let asked = client::guid(&body)?;
                    let save = game::clone_of(asked).or_else(|| u32::try_from(asked).ok());
                    if let Some(h) = save.and_then(|g| shared.saves.hero(g)) {
                        let (race, class) = wow_ids(&content_in_use(&world, shared), &h);
                        reply(packets::name_query_response(
                            asked,
                            &h.name,
                            race,
                            h.gender,
                            class,
                        ))?;
                    }
                }
                opcode::CMSG_GAMEOBJECT_QUERY => {
                    let entry = client::creature_query(&body)?;
                    let info = game::gameobject_look(entry)
                        .map(|l| packets::GameObjectInfo {
                            type_id: l.go_type.unwrap_or(0),
                            display_id: l.display,
                            name: l.name.into(),
                            data: [0; 24],
                        })
                        .or_else(|| game::door_info(&content_in_use(&world, shared), entry))
                        .or_else(|| game::object_info(&content_in_use(&world, shared), entry));
                    reply(packets::gameobject_query_response(entry, info.as_ref()))?;
                }
                opcode::CMSG_ITEM_QUERY_SINGLE => {
                    // the entry, then an item guid the answer does not need
                    let entry = client::creature_query(&body)?;
                    let info =
                        crate::looks::item_info(&content_in_use(&world, shared), shared.data.as_deref(), entry);
                    reply(packets::item_query_response(entry, info.as_ref()))?;
                }
                opcode::CMSG_CREATURE_QUERY => {
                    let entry = client::creature_query(&body)?;
                    reply(packets::creature_query_response(
                        entry,
                        creature_info(&content_in_use(&world, shared), entry).as_ref(),
                    ))?;
                }
                op if is_movement(op) => {
                    if let Some(w) = &world {
                        let moved = client::movement(&body)?;
                        let mut relay = Body::new();
                        relay.packed_guid(w.guid).bytes(&body);
                        let relay = relay.packet(op);
                        let _ = w.host.send(Input::Moved {
                            hero: w.hero,
                            position: moved.position,
                            facing: moved.facing,
                            relay,
                        });
                    }
                }
                _ => {}
            }
        }
    })();
    if let Some(w) = world.take() {
        w.leave();
    }
    writer.lock().unwrap().close();
    result
}

/// 1.12's race and class ids of a saved hero.
fn wow_ids(content: &Content, h: &SavedHero) -> (u8, u8) {
    let race = content
        .races
        .iter()
        .find(|r| r.id == h.race)
        .map_or(0, |r| r.wow_race_id);
    let class = content
        .classes
        .iter()
        .find(|c| c.id == h.class)
        .map_or(0, |c| c.wow_class_id as u8);
    (race, class)
}

/// The character list: every hero, standing at the spawn of the zone its next run is in, wearing
/// its class's worn gear.
fn roster(shared: &Shared) -> Vec<RosterEntry> {
    let content = shared.content();
    shared
        .saves
        .heroes()
        .iter()
        .map(|h| {
            let z = &content.zones[shared.zone_of(&content, h)];
            let (race, class) = wow_ids(&content, h);
            RosterEntry {
                guid: player_guid(h),
                name: h.name.clone(),
                race,
                class,
                gender: h.gender,
                look: h.look,
                level: 1,
                area: z.area_id,
                map: z.map_id,
                position: [z.spawn.x, z.spawn.y, z.spawn.z],
                equipment: {
                    // its class gear, its weapons its weapon loadout's
                    let progress = shared.saves.class_progress(&content, &h.class);
                    let worn = crate::looks::worn_looks(&content, &h.class, &progress);
                    crate::looks::char_enum_equipment(&crate::looks::armed(
                        &content, &h.class, &progress, worn,
                    ))
                },
            }
        })
        .collect()
}

/// A new hero from the create screen; a class or race the game does not have is refused, and so
/// is a pair the create screen does not offer: the stock ones (read from the player's data, when
/// it is open) and the ones `races.ron` adds.
fn create_hero(shared: &Shared, req: &client::CharCreate) -> u8 {
    let content = shared.content();
    if !crate::prerun::pair_offered(&content, shared.data.as_deref(), req.race, req.class) {
        return code::CHAR_CREATE_DISABLED;
    }
    let class = content
        .classes
        .iter()
        .find(|c| c.wow_class_id == u32::from(req.class));
    let race = content
        .races
        .iter()
        .find(|r| r.wow_race_id == req.race);
    let (Some(class), Some(race)) = (class, race) else {
        return code::CHAR_CREATE_DISABLED;
    };
    // a faction's camp in the hero picker is full at nine
    if shared
        .saves
        .heroes_of_faction(&content, race.faction)
        >= MAX_HEROES_PER_FACTION
    {
        return code::CHAR_CREATE_SERVER_LIMIT;
    }
    match shared
        .saves
        .create(&req.name, &class.id, &race.id, req.gender.min(1), req.look)
    {
        Ok(_) => code::CHAR_CREATE_SUCCESS,
        Err(code) => code,
    }
}

/// What a pet's creature query answers under its name.
const PET_SUBNAME: &str = "Companion";

/// What the creature query tells about `entry`: a zone enemy, a boss, a totem, an experience orb
/// or a pet. A rare answers under its own entry as the elite it copies, ranked rare: the
/// silver dragon on its portrait where the elite wears the gold one.
fn creature_info(content: &Content, entry: u32) -> Option<CreatureInfo> {
    if let Some(elite) = crate::entries::rare_of(entry) {
        let e = content.zones.iter().flat_map(|z| z.every_enemy()).find(|e| e.entry == elite && e.elite)?;
        return Some(CreatureInfo {
            name: e.name.clone(),
            subname: String::new(),
            creature_type: e.creature_type.dbc_id(),
            rank: RANK_RARE,
            display_id: e.display_id,
        });
    }
    for z in &content.zones {
        if let Some(e) = z.every_enemy().find(|e| e.entry == entry) {
            return Some(CreatureInfo {
                name: e.name.clone(),
                subname: String::new(),
                creature_type: e.creature_type.dbc_id(),
                rank: if e.elite || e.focus_as_elite { RANK_ELITE } else { RANK_NORMAL },
                display_id: e.display_id,
            });
        }
        if let Some(b) = z.every_boss().find(|b| b.entry == entry) {
            return Some(CreatureInfo {
                name: b.name.clone(),
                subname: String::new(),
                creature_type: b.creature_type.dbc_id(),
                rank: RANK_BOSS,
                display_id: b.display_id,
            });
        }
    }
    if entry == game::TOTEM_ENTRY {
        return Some(game::totem_info());
    }
    // the invisible target unit boss looks play on, a boss's object shown as a unit and a line's
    // runner (a stampede's beast) and a spirit by the hero: no name to show, the model their own
    if [crate::entries::BUNNY, crate::entries::GROUND_UNIT, crate::entries::LINE_RUNNER, crate::entries::SPIRIT].contains(&entry) {
        return Some(CreatureInfo {
            name: String::new(),
            subname: String::new(),
            creature_type: 0,
            rank: RANK_NORMAL,
            display_id: game::BUNNY_DISPLAY,
        });
    }
    if entry == crate::entries::BATTLE_STANDARD {
        return Some(CreatureInfo {
            name: game::STANDARD_NAME.into(),
            subname: String::new(),
            creature_type: 0,
            rank: RANK_NORMAL,
            display_id: content.balance.battle_standard.display.alliance,
        });
    }
    if entry == game::ORB_ENTRY {
        let look = game::orb_look(esurv_sim::game::loot::OrbSize::Small);
        return Some(CreatureInfo {
            name: look.name.into(),
            subname: String::new(),
            creature_type: 0,
            rank: RANK_NORMAL,
            display_id: look.display,
        });
    }
    content
        .pets
        .iter()
        .find(|p| p.entry == entry)
        .map(|p| CreatureInfo {
            name: p.name.clone(),
            subname: PET_SUBNAME.into(),
            creature_type: 1,
            rank: RANK_NORMAL,
            display_id: p.display_id,
        })
}

/// Starts a run for hero `guid` in the zone chosen for it and brings the client into it.
fn enter_world(
    shared: &Shared,
    guid: u64,
    writer: Arc<Mutex<PacketWriter>>,
) -> Result<InWorld, u8> {
    let saved = u32::try_from(guid)
        .ok()
        .and_then(|g| shared.saves.hero(g))
        .ok_or(code::CHAR_LOGIN_NO_CHARACTER)?;
    // the run takes the content as it is now and keeps it to its end, whatever is loaded meanwhile
    let content = shared.content();
    let zone = shared.zone_of(&content, &saved);
    // a class enters only the zones it has opened (the first is always open), or any in Challenge Mode
    if !shared.saves.zone_open_for_next_run(&content, &saved.class, zone) {
        return Err(code::CHAR_LOGIN_DISABLED);
    }
    let (host, inputs) = mpsc::channel();
    let mut game = Game::new(
        content.clone(),
        shared.data.as_ref(),
        zone,
        &saved,
        shared.saves.clone(),
        inputs,
    )
    .ok_or(code::CHAR_LOGIN_NO_WORLD)?;
    shared
        .saves
        .played(saved.guid, &content.zones[zone].id);
    let hero = game.hero_id(0);
    if !game.join(Player {
        hero,
        saved: saved.clone(),
        writer,
    }) {
        return Err(code::CHAR_LOGIN_NO_WORLD);
    }
    game.grant(hero, &shared.granted);
    if let Some(path) = &shared.move_log {
        game.log_moves_to(path);
    }
    if shared.autostart {
        game.start();
    }
    std::thread::spawn(move || game.host());
    Ok(InWorld {
        hero,
        guid: game::player_guid(&saved),
        host,
        content,
        _mark: shared.saves.enter_world(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pet_is_answered_as_a_companion_beast() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).expect("content loads");
        let wolf = content.pets.iter().find(|p| p.name == "Wolf").unwrap();
        let info = creature_info(&content, wolf.entry).unwrap();
        assert_eq!(info.name, "Wolf");
        assert_eq!(info.subname, "Companion");
        assert_eq!(info.creature_type, 1);
        assert_eq!(info.rank, 0);
        assert_eq!(info.display_id, wolf.display_id);
    }

    /// An elite wears the gold dragon (elite); its rare copy, under its own entry, the silver
    /// one (rare), with the same name and look.
    #[test]
    fn a_rare_is_answered_rare_and_its_elite_elite() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).expect("content loads");
        let elite = content.zones[0].enemies.iter().find(|e| e.elite).unwrap();
        let own = creature_info(&content, elite.entry).unwrap();
        assert_eq!(own.rank, RANK_ELITE);
        let rare = creature_info(&content, crate::entries::rare(elite.entry)).unwrap();
        assert_eq!(rare.rank, RANK_RARE);
        assert_eq!((rare.name.as_str(), rare.display_id), (elite.name.as_str(), elite.display_id));
        // a normal enemy has no rare copy
        let normal = content.zones[0].enemies.iter().find(|e| !e.elite).unwrap();
        assert_eq!(creature_info(&content, crate::entries::rare(normal.entry)), None);
    }

    /// The ones to focus (the Nightmare Suppressors, the Black Guard Sentries) are answered elite,
    /// so they glow purple.
    #[test]
    fn ones_to_focus_are_answered_elite() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).expect("content loads");
        let focus: Vec<_> = content.zones.iter().flat_map(|z| z.enemies.iter()).filter(|e| e.focus_as_elite).collect();
        let names: Vec<&str> = focus.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Nightmare Suppressor") && names.contains(&"Black Guard Sentry"), "{names:?}");
        for s in focus {
            assert!(!s.elite);
            assert_eq!(creature_info(&content, s.entry).unwrap().rank, RANK_ELITE, "{}", s.name);
        }
    }

    /// A writer to a connection nobody reads, for a run's packets (few enough to buffer).
    fn quiet_writer() -> (Arc<Mutex<PacketWriter>>, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let ours = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (theirs, _) = listener.accept().unwrap();
        (Arc::new(Mutex::new(PacketWriter::new(ours))), theirs)
    }

    fn warrior_health(content: &Content) -> f32 {
        content.classes.iter().find(|c| c.id == "warrior").unwrap().base_health
    }

    #[test]
    fn a_new_run_takes_reloaded_content_and_a_run_going_on_keeps_its_own() {
        use esurv_content::testing::{content_folder_with_zones, set_warrior_base_health, zone_file_text, ExtraZone};
        let from = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content"));
        let dir = content_folder_with_zones(from, "server-reload", &[]).unwrap();
        let live = Arc::new(LiveContent::load(&dir).unwrap());
        let saves = Arc::new(Saves::in_memory());
        let first = saves.create("Before", "warrior", "orc", 0, [0; 5]).unwrap();
        let second = saves.create("After", "warrior", "orc", 0, [0; 5]).unwrap();
        let shared = Shared {
            content: live.clone(),
            saves: saves.clone(),
            data: None,
            keys: SessionKeys::default(),
            autostart: false,
            zone: None,
            granted: Vec::new(),
            move_log: None,
        };
        let old_health = warrior_health(&shared.content());
        let (writer, _peer) = quiet_writer();
        let going_on = enter_world(&shared, player_guid(&first), writer).ok().unwrap();

        // the author raises a number and adds a zone, and the folder loads again
        set_warrior_base_health(&dir, old_health + 100.0).unwrap();
        let added = ExtraZone { id: "test_marsh", name: "Test Marsh", difficulty: 99.0 };
        let zone_file = dir.join(esurv_content::ZONE_FOLDER).join("test_marsh.ron");
        std::fs::write(zone_file, zone_file_text(added)).unwrap();
        live.reload().unwrap();

        assert_eq!(warrior_health(&going_on.content), old_health, "the run going on keeps its numbers");
        let (writer, _peer2) = quiet_writer();
        let next = enter_world(&shared, player_guid(&second), writer).ok().unwrap();
        assert_eq!(warrior_health(&next.content), old_health + 100.0, "a new run sees the new numbers");
        assert!(shared.content().zones.iter().any(|z| z.id == "test_marsh"), "the added zone is offered");
        assert_eq!(roster(&shared).len(), 2);
        going_on.leave();
        next.leave();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
