//! A run hosted for its players: a thread that ticks the simulation, feeds it the heroes'
//! movement and mirrors it to every client (heroes, enemies and pets created, moved along the
//! ground, their health kept current, killed and removed), tells what happened ([`present`]),
//! and talks with the game's own screens ([`screens`]).

mod again;
mod boss_looks;
mod bunnies;
mod channels;
mod doors;
pub use boss_looks::object_info;
pub use doors::door_info;
mod form_shifts;
mod ground_indicators;
mod ground_looks;
mod jewelry;
mod pickups;
mod player_enemies;
pub use player_enemies::{look_item_info, weapon_entry};
pub use bunnies::BUNNY_DISPLAY;
pub use pickups::{gameobject_look, orb_look, ORB_ENTRY};
mod present;
mod ranged_shots;
mod screens;
mod starting_looks;
mod totems;
pub use totems::{totem_info, TOTEM_ENTRY};
pub use starting_looks::clone_of;

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use esurv_content::{Content, PowerType, WeatherKind};
use esurv_protocol::ToServer;
use esurv_sim::game::enemy_ai::Segment;
use esurv_sim::game::run::{EnemyKind, Event, Flat, Hero, HeroSetup, Phase, Run, Unit, Walk, World};

use crate::ground::GameData;
use crate::walk_graph::FOLLOW_REACH;
use crate::save::{ClassProgress, SavedHero, Saves};
use crate::update::{
    field, update_object, Fields, Living, ObjectTypeId, ObjectUpdate, Speeds, TYPE_ITEM,
    TYPE_OBJECT, TYPE_PLAYER, TYPE_UNIT,
};
use crate::wire::{Packet, PacketWriter};
use crate::{looks, packets};

/// How often the host ticks the run.
pub const TICK: Duration = Duration::from_millis(50);
/// Enemies and pets walk along the ground: a path point every this many yards.
const PATH_STEP: f32 = 4.0;
/// `HIGHGUID_UNIT`: creature guids are this | entry << 24 | a number.
const HIGHGUID_UNIT: u64 = 0xF130 << 48;
/// Enemies are of the "Monster" faction template, hostile to every hero.
const ENEMY_FACTION: u32 = 14;
/// 1.12's player defaults for the bounding radius and melee reach.
const PLAYER_BOUNDING_RADIUS: f32 = 0.389;
const PLAYER_COMBAT_REACH: f32 = 1.5;
/// The hero's gold shows as gold coins (the client counts copper).
const COPPER_PER_GOLD: u32 = 10_000;
/// A pet: 100 health, not attackable, its size from its scale, and
/// running at 9 yd/s (its moves carry their own timing).
const PET_HEALTH: u32 = 100;
const PET_UNIT_FLAGS: u32 = 0x2;
const PET_BOUNDING_SHARE: f32 = 0.5;
const PET_REACH_SHARE: f32 = 1.5;
const PET_RUN_SPEED: f32 = 9.0;
/// `PLAYER_BYTES_2`'s rest state: normal (not rested).
const REST_STATE_NORMAL: u8 = 2;
/// A hero enters a run with its melee weapons in hand, and the client draws and puts them away
/// as the stock 1.12 client does from then on.
const SHEATH_STATE_MELEE: u8 = 1;
/// A caster enters with its weapons put away (staves on the back, one-handers on the hip), and
/// they stay away: its fight is spells.
const SHEATH_STATE_PUT_AWAY: u8 = 0;
/// The classes that keep their weapons put away in a run.
const CASTER_CLASSES: [&str; 3] = ["mage", "warlock", "priest"];

/// The sheath state a hero of `class` enters a run with.
fn sheath_state(class: &str) -> u8 {
    if CASTER_CLASSES.contains(&class) {
        SHEATH_STATE_PUT_AWAY
    } else {
        SHEATH_STATE_MELEE
    }
}

/// A creature's weapons as its virtual items (look, then item info and sheath per slot), held
/// drawn as WoW's creatures hold theirs (vmangos: `SHEATH_STATE_MELEE`), so the client
/// shows them in hand and swings by their kind.
fn weapon_fields(weapons: &esurv_content::CreatureWeapons) -> Fields {
    let mut f = Fields::new();
    for (slot, w) in weapons.slots().iter().enumerate() {
        let Some(w) = w else { continue };
        let slot = slot as u16;
        f.u32(field::UNIT_VIRTUAL_ITEM_SLOT_DISPLAY + slot, w.display)
            .u32(field::UNIT_VIRTUAL_ITEM_INFO + 2 * slot, w.info())
            .u32(field::UNIT_VIRTUAL_ITEM_INFO + 2 * slot + 1, w.sheath & 0xff);
    }
    f.bytes(field::UNIT_BYTES_2, [SHEATH_STATE_MELEE, 0, 0, 0]);
    f
}

/// A player's hero on the wire: its guid is the save's hero number.
pub fn player_guid(hero: &SavedHero) -> u64 {
    u64::from(hero.guid)
}

/// `HIGHGUID_ITEM`: item guids are this | a number.
const HIGHGUID_ITEM: u64 = 0x4000 << 48;

/// The guid of the item a hero (player guid `hero`) wears in equipment slot `slot`.
fn worn_item_guid(hero: u64, slot: u8) -> u64 {
    HIGHGUID_ITEM | (hero & 0xFF_FFFF) << 8 | u64::from(slot)
}

/// An item of entry `entry` held by player `owner` itself (in its equipment or backpack), as its
/// own client is told of it: stack 1.
fn own_item(guid: u64, entry: u32, owner: u64) -> ObjectUpdate {
    let mut f = Fields::new();
    f.u64(field::OBJECT_GUID, guid)
        .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_ITEM)
        .u32(field::OBJECT_ENTRY, entry)
        .f32(field::OBJECT_SCALE_X, 1.0);
    f.u64(field::ITEM_OWNER, owner)
        .u64(field::ITEM_CONTAINED, owner)
        .u32(field::ITEM_STACK_COUNT, 1);
    ObjectUpdate::CreateItem { guid, fields: f }
}

/// What a hero wears through a run, read as it joins: the item entry shown in each equipment
/// slot, and the class gear among them, each piece an item in the hero's own equipment.
struct Dressed {
    shown: [u32; looks::EQUIPMENT_SLOTS],
    worn: Vec<looks::WornLook>,
}

/// The guid of creature `entry` with simulation id `id`.
pub fn unit_guid(entry: u32, id: u32) -> u64 {
    HIGHGUID_UNIT | (u64::from(entry) & 0xFF_FFFF) << 24 | (u64::from(id) & 0xFF_FFFF)
}

/// 1.12's `SMSG_WEATHER` for a zone's weather: type, strength, and the matching ambient sound
/// (light below 0.4, medium below 0.7, heavy above).
pub fn weather_packet(kind: WeatherKind, intensity: f32) -> Packet {
    let (wow_type, sounds) = match kind {
        WeatherKind::None => (0, [0, 0, 0]),
        WeatherKind::Rain => (1, [8533, 8534, 8535]),
        WeatherKind::Snow => (2, [8536, 8537, 8538]),
        WeatherKind::Sandstorm => (3, [8556, 8557, 8558]),
    };
    let sound = sounds[if intensity < 0.4 {
        0
    } else if intensity < 0.7 {
        1
    } else {
        2
    }];
    packets::weather(wow_type, intensity, sound)
}

/// A hero's player in a hosted run.
pub struct Player {
    /// The simulation's hero id.
    pub hero: u32,
    pub saved: SavedHero,
    pub writer: Arc<Mutex<PacketWriter>>,
}

impl Player {
    fn guid(&self) -> u64 {
        player_guid(&self.saved)
    }

    fn send(&self, packets: &[Packet]) -> bool {
        let mut w = self.writer.lock().unwrap();
        packets.iter().all(|p| w.send(p).is_ok())
    }
}

/// What a player's connection tells the host.
pub enum Input {
    /// The hero's client moved it; `relay` is the movement as the other players get it.
    Moved {
        hero: u32,
        position: [f32; 3],
        facing: f32,
        relay: Packet,
    },
    /// The player leaves; the host answers on `done` once it sends nothing more to them.
    Left { hero: u32, done: Sender<()> },
    /// The hero's client asks something of the run or the save (esurv-protocol).
    Asked { hero: u32, request: ToServer },
}

/// An enemy or pet as the clients were last told.
struct Shown {
    guid: u64,
    segment: Segment,
    health: u32,
    /// Its name, for the lines it shouts or is named in.
    name: String,
    /// Dead and lying there until then (run seconds).
    corpse_until: Option<f32>,
    /// Its effects over time as the clients see them (debuffs).
    debuffs: Vec<present::Aura>,
    /// A pet's level as last told (its hero's; a beast's size goes with it); 0 for an enemy.
    level: u32,
}

/// A hero's fields as the clients were last told.
#[derive(Clone, Copy, PartialEq)]
struct HeroShown {
    health: u32,
    max_health: u32,
    level: u32,
    xp: u32,
    /// Experience to the next level.
    next: u32,
    gold: u32,
}

impl HeroShown {
    fn of(h: &Hero, rules: &esurv_content::ExperienceRules) -> Self {
        Self {
            health: whole(h.health),
            max_health: whole(h.stats.max_health),
            level: h.progress.level,
            xp: h.progress.xp,
            next: esurv_sim::game::level::xp_to_next(h.progress.level, rules),
            gold: h.gold,
        }
    }

    /// These fields of the hero with guid `guid`, as an update.
    fn values(self, guid: u64) -> ObjectUpdate {
        let mut f = Fields::new();
        f.u32(field::UNIT_HEALTH, self.health)
            .u32(field::UNIT_MAXHEALTH, self.max_health)
            .u32(field::UNIT_LEVEL, self.level);
        f.u32(field::PLAYER_XP, self.xp)
            .u32(field::PLAYER_NEXT_LEVEL_XP, self.next);
        f.u32(
            field::PLAYER_COINAGE,
            self.gold.saturating_mul(COPPER_PER_GOLD),
        );
        ObjectUpdate::Values { guid, fields: f }
    }
}

/// The run and everyone in it.
pub struct Game {
    run: Run,
    players: Vec<Player>,
    shown: HashMap<u32, Shown>,
    heroes_shown: HashMap<u32, HeroShown>,
    next_spline: u32,
    next_teleport: u32,
    /// The numbers line each hero's client last got.
    stats_sent: HashMap<u32, String>,
    /// Each hero's buffs as its clients last saw them.
    auras_shown: HashMap<u32, Vec<present::Aura>>,
    /// Each hero's starting option as its clients last saw it (bar, stacks), by hero.
    options_shown: HashMap<u32, starting_looks::OptionShown>,
    /// The ghostly copies of heroes on the field, by summon id.
    clones: HashMap<u32, starting_looks::CloneShown>,
    /// Looks and sounds that go out a moment after what brought them, with their run time.
    later: Vec<(f32, Packet)>,
    /// The heroes out of their form for a moment (a powershift), with the run time they shift back.
    out_of_form: HashMap<u32, f32>,
    /// The enemies Starshards' stars fall on, by simulation id, and until when.
    starshards_until: HashMap<u32, f32>,
    /// The dual-wielding enemies whose next swing is their off hand's, by simulation id.
    left_swing_next: HashSet<u32>,
    /// The channel beams the clients see, by caster guid: the spell and what it is aimed at.
    channels_shown: HashMap<u64, (u32, u64)>,
    /// The pickups on the field as the clients were last told.
    pickups_shown: HashMap<u32, pickups::ShownPickup>,
    /// The totems on the field, by simulation id, and their guids.
    totems_shown: HashMap<u32, u64>,
    /// The spell looks on the ground, by simulation id.
    looks_shown: HashMap<u32, ground_looks::ShownLook>,
    /// The bosses' marks and the objects on the ground as the clients were last told, by
    /// simulation id (the building blocks).
    marks_shown: HashMap<u32, boss_looks::ShownMark>,
    objects_shown: HashMap<u32, boss_looks::ShownObject>,
    /// The marks, zones and beams as the clients with a HUD were last told their indicators, the
    /// ones dropped with the field to tell gone, the next indicator's number, and whether the run
    /// stood when they were told (`ground_indicators`).
    indicators_shown: HashMap<ground_indicators::IndicatorOf, ground_indicators::ShownIndicator>,
    indicators_dropped: Vec<u32>,
    next_indicator: u32,
    indicators_still: bool,
    /// The invisible target units boss looks play on, with when each goes (`bunnies`).
    bunnies: Vec<(u64, f32)>,
    next_bunny: u32,
    /// Each hero's stacks a boss left on it, as its debuffs were last shown.
    hero_debuffs_shown: HashMap<u32, Vec<present::Aura>>,
    /// The player's game data, for the spells' own radii.
    data: Option<Arc<GameData>>,
    /// Where the heroes' progress is kept.
    saves: Arc<Saves>,
    /// What the run has paid each hero so far.
    paid: HashMap<u32, screens::Paid>,
    /// The end the run was last paid out at (a victory, then a fall in Endless).
    ended: Option<Phase>,
    /// The heroes who left the run.
    left: HashSet<u32>,
    /// The clients that said `hello`, by hero, and what they were last told.
    told: HashMap<u32, screens::Told>,
    /// The bosses' health bars as the clients were last told, by simulation id.
    bars: HashMap<u32, screens::Bar>,
    /// The gear pieces won, by item entry: their names, and whether they were new to the
    /// collection.
    drops: HashMap<u32, (String, bool)>,
    /// Each hero's damage meter.
    meters: HashMap<u32, screens::MeterTally>,
    /// What each hero wears, read as it joins.
    dressed: HashMap<u32, Dressed>,
    /// Each hero's jewelry items as its own client holds them, the rarity each ring or trinket
    /// was put on at (by hero and id), and the number of the last jewelry item made.
    jewelry_held: HashMap<u32, jewelry::Held>,
    jewelry_rarity: HashMap<(u32, String), u8>,
    next_jewel: u16,
    /// Whether the clients see the run moving (its units walking).
    moving_shown: bool,
    /// Whether the heroes are rooted, and the counter of the last root or unroot.
    rooted: bool,
    next_root: u32,
    /// Each hero's run speed (yards a second) as the clients were last told, and the counter of
    /// the last speed change.
    speeds_shown: HashMap<u32, f32>,
    next_speed_change: u32,
    /// Where the heroes' moves are logged (a dev switch).
    move_log: Option<(std::fs::File, Instant)>,
    /// The run was started without waiting for a first move (a dev switch): a fresh run is too.
    starts_at_once: bool,
    inputs: Receiver<Input>,
}

/// `ESURV_UNHURT=1`, a dev aid for unattended shots of a run: the heroes are healed to full
/// after every tick, so a hero standing still lives long enough for its passives to show, and the
/// first offer of every choice is taken at once.
fn unhurt() -> bool {
    static UNHURT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *UNHURT.get_or_init(|| std::env::var("ESURV_UNHURT").is_ok_and(|v| v == "1"))
}

/// `ESURV_ORBS_STAY=1`, a dev aid for shots of the ground: the orbs a passive drops (Holy
/// Orbs) are never taken and lie until their time is up.
fn orbs_stay() -> bool {
    static ORBS_STAY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ORBS_STAY.get_or_init(|| std::env::var("ESURV_ORBS_STAY").is_ok_and(|v| v == "1"))
}

/// A new run's random seed, from the clock.
fn run_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64)
}

/// The hero a saved hero brings into a run: its class's talents and prestige ranks, and the gear it
/// wears.
pub fn hero_setup(content: &Content, saved: &SavedHero, progress: &ClassProgress) -> HeroSetup {
    let mut setup = HeroSetup::with_progress(
        content,
        &saved.class,
        &saved.race,
        progress
            .talents
            .iter()
            .map(|(id, rank)| (id.as_str(), *rank)),
        &progress.worn,
    );
    // the class's prestige ranks, as its talents
    setup.add_prestige(content, &progress.prestige);
    // the weapon loadout the class fights with (none: its first)
    setup.loadout = content
        .loadout(&saved.class, progress.loadout.as_deref())
        .map(|l| l.id.clone());
    // the starting option picked on the warband screen, when its class still has it
    setup.start = content
        .class(&saved.class)
        .and_then(|c| c.starting_option(saved.start.as_deref()))
        .map(|o| o.id.clone());
    setup
}

impl Game {
    /// A run in zone `zone` for one player, waiting for the first move. `None` if the hero's
    /// class or race is not in the content.
    pub fn new(
        content: Arc<Content>,
        data: Option<&Arc<GameData>>,
        zone: usize,
        saved: &SavedHero,
        saves: Arc<Saves>,
        inputs: Receiver<Input>,
    ) -> Option<Self> {
        let z = content.zones.get(zone)?;
        let ground: Box<dyn World> = match data.map(|d| d.ground(z.map_id)) {
            Some(Ok(g)) => Box::new(g),
            _ => Box::new(Flat),
        };
        let seed = run_seed();
        let run = Run::new(
            content.clone(),
            &z.id,
            saves.run_options().run_options(&content.run_options),
            &[hero_setup(
                &content,
                saved,
                &saves.class_progress(&content, &saved.class),
            )],
            ground,
            seed,
        )?;
        Some(Self {
            run,
            players: Vec::new(),
            shown: HashMap::new(),
            heroes_shown: HashMap::new(),
            next_spline: 1,
            next_teleport: 0,
            stats_sent: HashMap::new(),
            auras_shown: HashMap::new(),
            options_shown: HashMap::new(),
            clones: HashMap::new(),
            later: Vec::new(),
            out_of_form: HashMap::new(),
            starshards_until: HashMap::new(),
            left_swing_next: HashSet::new(),
            channels_shown: HashMap::new(),
            pickups_shown: HashMap::new(),
            totems_shown: HashMap::new(),
            looks_shown: HashMap::new(),
            data: data.cloned(),
            saves,
            paid: HashMap::new(),
            ended: None,
            left: HashSet::new(),
            told: HashMap::new(),
            bars: HashMap::new(),
            drops: HashMap::new(),
            meters: HashMap::new(),
            dressed: HashMap::new(),
            jewelry_held: HashMap::new(),
            jewelry_rarity: HashMap::new(),
            next_jewel: 0,
            marks_shown: HashMap::new(),
            objects_shown: HashMap::new(),
            indicators_shown: HashMap::new(),
            indicators_dropped: Vec::new(),
            next_indicator: 1,
            indicators_still: false,
            bunnies: Vec::new(),
            next_bunny: 0,
            hero_debuffs_shown: HashMap::new(),
            moving_shown: true,
            rooted: false,
            next_root: 0,
            speeds_shown: HashMap::new(),
            next_speed_change: 0,
            move_log: None,
            starts_at_once: false,
            inputs,
        })
    }

    /// The simulation id of the run's `i`-th hero.
    pub fn hero_id(&self, i: usize) -> u32 {
        self.run.heroes[i].id
    }

    /// Brings a player into the world: the zone, its time and weather, their own hero, the other
    /// heroes and everything already there. `false` if their connection failed.
    pub fn join(&mut self, player: Player) -> bool {
        let dressed = self.dress(&player.saved);
        self.dressed.insert(player.hero, dressed);
        let content = self.run.content.clone();
        let zone = &content.zones[self.run.zone];
        let Some(hero) = self.run.heroes.iter().find(|h| h.id == player.hero) else {
            return false;
        };
        let mut welcome = vec![
            packets::login_verify_world(
                zone.map_id,
                [hero.pos.0, hero.pos.1, hero.height],
                hero.facing,
            ),
            packets::login_set_time_speed(zone.hour, 0),
            packets::tips_seen(&self.saves.tips_seen()),
            packets::initial_spells(&self.spellbook(hero)),
            self.action_bar(hero),
        ];
        self.speeds_shown.insert(hero.id, hero.stats.run_speed());
        let mut objects = vec![self.hero_object(hero, &player.saved, true)];
        objects.extend(self.worn_items(hero, &player.saved));
        let me = update_object(&[self.hero_object(hero, &player.saved, false)]);
        for other in &self.players {
            if let Some(h) = self.run.heroes.iter().find(|h| h.id == other.hero) {
                objects.push(self.hero_object(h, &other.saved, false));
            }
        }
        objects.extend(self.units_now());
        objects.extend(self.pickups_now());
        objects.extend(self.doors_now());
        objects.extend(self.totems_now());
        objects.extend(self.ground_objects_now());
        objects.extend(self.clones_now());
        objects.extend(self.ground_looks_now());
        welcome.extend(self.player_enemy_names_now());
        welcome.push(update_object(&objects));
        welcome.extend(self.moves_now());
        welcome.push(packets::init_world_states(zone.map_id, zone.area_id));
        welcome.push(weather_packet(zone.weather.kind, zone.weather.intensity));
        if !player.send(&welcome) {
            return false;
        }
        for other in &self.players {
            other.send(std::slice::from_ref(&me));
        }
        self.players.push(player);
        true
    }

    /// Starts the run now rather than at the first move (and a fresh run after it, too).
    pub fn start(&mut self) {
        self.starts_at_once = true;
        self.run.start();
    }

    /// Ticks the run until every player has left.
    pub fn host(mut self) {
        let mut last = Instant::now();
        loop {
            loop {
                match self.inputs.try_recv() {
                    Ok(input) => self.input(input),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            if self.players.is_empty() {
                return;
            }
            let started = Instant::now();
            self.run.orbs_stay = orbs_stay();
            self.run.tick((started - last).as_secs_f32());
            last = started;
            if unhurt() {
                for h in &mut self.run.heroes {
                    h.health = h.stats.max_health;
                }
            }
            self.stand_in_for_the_card_screen();
            let events = self.run.drain_events();
            self.show(&events);
            std::thread::sleep(TICK.saturating_sub(started.elapsed()));
        }
    }

    fn input(&mut self, input: Input) {
        match input {
            Input::Moved {
                hero,
                position,
                facing,
                relay,
            } => {
                // a hero who left the run moves nothing in it
                if !self.left.contains(&hero) {
                    self.run
                        .hero_moved(hero, (position[0], position[1]), position[2], facing);
                    self.log_move(hero, position);
                }
                for p in self.players.iter().filter(|p| p.hero != hero) {
                    p.send(std::slice::from_ref(&relay));
                }
            }
            Input::Left { hero, done } => {
                // logging out (or the connection closing) pays the run out too
                let paid: Vec<Packet> = self.pay_out(hero).into_iter().map(|(_, p)| p).collect();
                if let Some(i) = self.players.iter().position(|p| p.hero == hero) {
                    self.players[i].send(&paid);
                    let gone = self.players.remove(i);
                    let bye = packets::destroy_object(gone.guid());
                    for p in &self.players {
                        p.send(std::slice::from_ref(&bye));
                    }
                }
                self.told.remove(&hero);
                self.jewelry_held.remove(&hero);
                let _ = done.send(());
            }
            Input::Asked { hero, request } => {
                // most answers are for the asker's client; a fresh run is for everyone's
                let told = self.ask(hero, request);
                for p in &self.players {
                    let mine: Vec<Packet> = told
                        .iter()
                        .filter(|(h, _)| *h == p.hero)
                        .map(|(_, pk)| pk.clone())
                        .collect();
                    if !mine.is_empty() {
                        p.send(&mine);
                    }
                }
            }
        }
    }

    /// For a hero whose client has no choice screen of its own (it never said so with `hello`),
    /// the first offer is taken at once, so the run goes on.
    fn stand_in_for_the_card_screen(&mut self) {
        if let Some(screen) = &self.run.screen {
            let hero = self.run.heroes[screen.hero].id;
            // unattended shots take the first offer too, so no screen covers the run
            if !self.chooses_itself(hero) || unhurt() {
                self.run.choose(hero, 0);
            }
        }
    }

    /// What `saved` wears: in each equipment slot (item entries) its class's worn gear in the
    /// looks chosen for it, over its race and class's starting outfit, as the character list
    /// shows it. Read as the hero joins: a change on the hero sheet shows in the next run.
    fn dress(&self, saved: &SavedHero) -> Dressed {
        let content = &self.run.content;
        let progress = self.saves.class_progress(content, &saved.class);
        // the class gear worn, its weapons those of the class's weapon loadout
        // (roadmap R3)
        let worn = looks::armed(
            content,
            &saved.class,
            &progress,
            looks::worn_looks(content, &saved.class, &progress),
        );
        let armed = !content.loadouts_of(&saved.class).is_empty();
        let race = content.races.iter().find(|r| r.id == saved.race);
        let class = content.class(&saved.class);
        let outfit = match (&self.data, race, class) {
            (Some(d), Some(r), Some(k)) => {
                d.start_outfit(r.wow_race_id, k.wow_class_id as u8, saved.gender.min(1))
            }
            _ => &[],
        };
        let shown = if armed {
            looks::shown_equipment(&worn, &looks::unarmed_outfit(outfit))
        } else {
            looks::shown_equipment(&worn, outfit)
        };
        Dressed { shown, worn }
    }

    /// The items of the class gear `hero` wears, for its own client (the character pane shows
    /// them in its slots): each held by the hero in the equipment slot it is drawn in.
    fn worn_items(&self, hero: &Hero, saved: &SavedHero) -> Vec<ObjectUpdate> {
        let owner = player_guid(saved);
        let Some(dressed) = self.dressed.get(&hero.id) else {
            return Vec::new();
        };
        dressed
            .worn
            .iter()
            .map(|w| own_item(worn_item_guid(owner, w.equip_slot), w.entry, owner))
            .collect()
    }

    /// The player unit of `hero`; `own` for the client that plays it.
    fn hero_object(&self, hero: &Hero, saved: &SavedHero, own: bool) -> ObjectUpdate {
        let content = &self.run.content;
        let class = &content.classes[hero.class];
        let race = &content.races[hero.race];
        // a starting option's rage or energy, else the class's power
        let power = match class.starting_option(Some(&hero.start)).and_then(|o| o.resource) {
            Some(_) => starting_looks::hero_power_type(content, hero),
            None => match class.power {
                PowerType::Mana => 0,
                PowerType::Rage => 1,
                PowerType::Energy => 3,
            },
        };
        // the race's player model from the player's own data (none without it)
        let display = self
            .data
            .as_ref()
            .and_then(|d| d.player_display(race.wow_race_id, saved.gender))
            .unwrap_or(0);
        let guid = player_guid(saved);
        let l = saved.look;
        let mut f = Fields::new();
        f.u64(field::OBJECT_GUID, guid)
            .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_UNIT | TYPE_PLAYER)
            .f32(field::OBJECT_SCALE_X, 1.0);
        f.u32(field::UNIT_HEALTH, whole(hero.health))
            .u32(field::UNIT_MAXHEALTH, whole(hero.stats.max_health));
        f.u32(field::UNIT_LEVEL, hero.progress.level)
            .u32(field::UNIT_FACTIONTEMPLATE, race.faction_template);
        f.bytes(
            field::UNIT_BYTES_0,
            [
                race.wow_race_id,
                class.wow_class_id as u8,
                saved.gender,
                power,
            ],
        );
        f.f32(field::UNIT_BOUNDINGRADIUS, PLAYER_BOUNDING_RADIUS)
            .f32(field::UNIT_COMBATREACH, PLAYER_COMBAT_REACH);
        // a form's model over the race's own
        let shown = starting_looks::form_display(content, hero).unwrap_or(display);
        f.u32(field::UNIT_DISPLAYID, shown)
            .u32(field::UNIT_NATIVEDISPLAYID, display)
            .f32(field::UNIT_MOD_CAST_SPEED, 1.0);
        // a form's weapons: Moonkin's put away, an animal's not shown
        let in_form = starting_looks::form_weapons(content, hero);
        let sheath = match in_form {
            starting_looks::FormWeapons::AsTheClass => sheath_state(&class.id),
            _ => SHEATH_STATE_PUT_AWAY,
        };
        f.bytes(field::UNIT_BYTES_2, [sheath, 0, 0, 0]);
        if let Some(dressed) = self.dressed.get(&hero.id) {
            for (slot, &entry) in dressed.shown.iter().enumerate() {
                let hidden = in_form == starting_looks::FormWeapons::Hidden && (15..=17).contains(&slot);
                if entry != 0 && !hidden {
                    f.u32(
                        field::PLAYER_VISIBLE_ITEM_1_0
                            + slot as u16 * field::PLAYER_VISIBLE_ITEM_STRIDE,
                        entry,
                    );
                }
            }
            // its own client is told which items it holds in which slot ([`Self::worn_items`])
            if own {
                for w in &dressed.worn {
                    f.u64(
                        field::PLAYER_INV_SLOT_HEAD + 2 * u16::from(w.equip_slot),
                        worn_item_guid(guid, w.equip_slot),
                    );
                }
            }
        }
        f.bytes(field::PLAYER_BYTES, [l[0], l[1], l[2], l[3]])
            .bytes(field::PLAYER_BYTES_2, [l[4], 0, 0, REST_STATE_NORMAL])
            .u32(field::PLAYER_BYTES_3, saved.gender.into());
        f.u32(field::PLAYER_XP, hero.progress.xp).u32(
            field::PLAYER_NEXT_LEVEL_XP,
            esurv_sim::game::level::xp_to_next(
                hero.progress.level,
                &content.balance.experience,
            ),
        );
        f.u32(field::PLAYER_WATCHED_FACTION_INDEX, u32::MAX).u32(
            field::PLAYER_COINAGE,
            hero.gold.saturating_mul(COPPER_PER_GOLD),
        );
        let living = Living {
            position: [hero.pos.0, hero.pos.1, hero.height],
            facing: hero.facing,
            speeds: Speeds::running_at(hero.stats.run_speed()),
        };
        ObjectUpdate::Create {
            guid,
            type_id: ObjectTypeId::Player,
            living,
            fields: f,
            own,
        }
    }

    /// The ground height at `at`. Where floors stack (a dungeon; enemies keep no height of their
    /// own) it is the floor there the nearest hero walks to soonest, the one spawns and followers
    /// use, else the floor nearest below that hero's height; that hero's height where there is
    /// none.
    fn ground_z(&self, at: (f32, f32)) -> f32 {
        let hero = self
            .run
            .heroes
            .iter()
            .min_by(|a, b| dist(a.pos, at).total_cmp(&dist(b.pos, at)))
            .map(|h| [h.pos.0, h.pos.1, h.height]);
        if let Some(feet) = hero {
            if let Walk::Reached { z, .. } = self.run.world.walk(feet, at, FOLLOW_REACH) {
                return z;
            }
        }
        let height = hero.map(|f| f[2]);
        self.run
            .world
            .floor(at.0, at.1, height.unwrap_or(f32::MAX))
            .unwrap_or(height.unwrap_or(0.0))
    }

    /// Every enemy and pet, as creates.
    fn units_now(&mut self) -> Vec<ObjectUpdate> {
        let now = self.run.now;
        let mut out = Vec::new();
        for i in 0..self.run.enemies.len() {
            let e = &self.run.enemies[i];
            let (id, segment, health) = (e.id, e.segment, whole(e.health));
            let (create, guid) = self.enemy_object(i, now);
            let name = self.enemy_name(i);
            self.shown.insert(
                id,
                Shown {
                    guid,
                    segment,
                    health,
                    name,
                    corpse_until: None,
                    debuffs: Vec::new(),
                    level: 0,
                },
            );
            out.push(create);
        }
        for i in 0..self.run.pets.len() {
            let p = &self.run.pets[i];
            let (id, segment) = (p.id, p.segment);
            if let Some((create, guid)) = self.pet_object(i, now) {
                let name = p.pet.clone();
                let level = self.pet_level(i);
                self.shown.insert(
                    id,
                    Shown {
                        guid,
                        segment,
                        health: 0,
                        name,
                        corpse_until: None,
                        debuffs: Vec::new(),
                        level,
                    },
                );
                out.push(create);
            }
        }
        out
    }

    /// Logs every hero's moves to `path` from now on (a dev switch, `ESURV_MOVE_LOG`): a line
    /// per move, real seconds since the log began, hero, x, y and the run speed the server holds
    /// for it, tab apart.
    pub fn log_moves_to(&mut self, path: &std::path::Path) {
        self.move_log = std::fs::File::create(path)
            .ok()
            .map(|f| (f, Instant::now()));
    }

    /// One move in the move log, if one is kept.
    fn log_move(&mut self, hero: u32, position: [f32; 3]) {
        use std::io::Write;
        let speed = self
            .run
            .heroes
            .iter()
            .find(|h| h.id == hero)
            .map_or(0.0, |h| h.stats.run_speed());
        if let Some((log, began)) = &mut self.move_log {
            let _ = writeln!(
                log,
                "{:.3}\t{hero}\t{:.3}\t{:.3}\t{speed:.3}",
                began.elapsed().as_secs_f32(),
                position[0],
                position[1]
            );
        }
    }

    /// Each hero whose run speed changed since the clients were last told (a speed rune, its end,
    /// a fresh run): its own client is forced to the new speed (`SMSG_FORCE_RUN_SPEED_CHANGE`,
    /// which it acks), the other clients told it (`MSG_MOVE_SET_RUN_SPEED`). Each packet is for
    /// one hero's client.
    fn mirror_speeds(&mut self) -> Vec<(u32, Packet)> {
        let mut out = Vec::new();
        for h in &self.run.heroes {
            let speed = h.stats.run_speed();
            if self
                .speeds_shown
                .get(&h.id)
                .is_some_and(|&s| (s - speed).abs() < 1e-4)
            {
                continue;
            }
            let Some(mover) = self.players.iter().find(|p| p.hero == h.id) else {
                continue;
            };
            self.speeds_shown.insert(h.id, speed);
            self.next_speed_change = self.next_speed_change.wrapping_add(1);
            let guid = mover.guid();
            out.push((
                h.id,
                packets::force_run_speed_change(guid, self.next_speed_change, speed),
            ));
            let seen =
                packets::move_set_run_speed(guid, [h.pos.0, h.pos.1, h.height], h.facing, speed);
            out.extend(
                self.players
                    .iter()
                    .filter(|p| p.hero != h.id)
                    .map(|p| (p.hero, seen.clone())),
            );
        }
        out
    }

    /// Monster moves for every enemy and pet that is walking right now.
    fn moves_now(&mut self) -> Vec<Packet> {
        let walks: Vec<(u64, Segment)> = self
            .shown
            .values()
            .filter(|s| s.segment.moving(self.run.now))
            .map(|s| (s.guid, s.segment))
            .collect();
        walks
            .into_iter()
            .map(|(guid, segment)| self.walk(guid, &segment))
            .collect()
    }

    fn enemy_object(&self, i: usize, now: f32) -> (ObjectUpdate, u64) {
        let content = &self.run.content;
        let zone = &content.zones[self.run.zone];
        let e = &self.run.enemies[i];
        let (entry, display, scale) = match e.kind {
            EnemyKind::Roster(r) => (
                zone.enemies[r].entry,
                zone.enemies[r].display_id,
                zone.enemies[r].scale,
            ),
            // a boss that changed its look (a phase change) shows its new one
            EnemyKind::Boss(b) => (
                zone.bosses[b].entry,
                e.mech.display_id.unwrap_or(zone.bosses[b].display_id),
                e.mech.scale.unwrap_or(zone.bosses[b].scale),
            ),
            EnemyKind::Rare { roster, .. } => (
                crate::entries::rare(zone.enemies[roster].entry),
                zone.enemies[roster].display_id,
                zone.enemies[roster].scale * esurv_sim::game::rares::SCALE,
            ),
        };
        // one that looks like a player is shown as one
        if let Some(shape) = self.player_shape(i) {
            return self.player_enemy_object(i, now, &shape, scale);
        }
        let guid = unit_guid(entry, e.id);
        let at = e.position(now);
        let mut f = Fields::new();
        f.u64(field::OBJECT_GUID, guid)
            .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_UNIT)
            .u32(field::OBJECT_ENTRY, entry)
            .f32(field::OBJECT_SCALE_X, scale);
        f.u32(field::UNIT_HEALTH, whole(e.health))
            .u32(field::UNIT_MAXHEALTH, whole(e.max_health));
        f.u32(field::UNIT_LEVEL, self.enemy_level())
            .u32(field::UNIT_FACTIONTEMPLATE, ENEMY_FACTION);
        f.u32(field::UNIT_DISPLAYID, display)
            .u32(field::UNIT_NATIVEDISPLAYID, display);
        if let Some(carried) = self.carried_aura(i) {
            f.extend(&carried);
        }
        f.f32(field::UNIT_BOUNDINGRADIUS, e.collision_radius)
            .f32(field::UNIT_COMBATREACH, PLAYER_COMBAT_REACH);
        if let Some(weapons) = self.enemy_weapons(i) {
            f.extend(&weapon_fields(&weapons));
        }
        // a boss kneeling (its script) is seen kneeling
        if e.mech.kneel_until.is_some() {
            f.bytes(field::UNIT_BYTES_1, [8, 0, 0, 0]);
        }
        f.u32(
            field::UNIT_BASEATTACKTIME,
            (e.attack_interval * 1000.0) as u32,
        )
        .f32(field::UNIT_MOD_CAST_SPEED, 1.0);
        let living = Living {
            position: [at.0, at.1, self.ground_z(at)],
            facing: facing_of(&e.segment),
            speeds: Speeds::running_at(e.base_speed),
        };
        (
            ObjectUpdate::Create {
                guid,
                type_id: ObjectTypeId::Unit,
                living,
                fields: f,
                own: false,
            },
            guid,
        )
    }

    /// What creature enemy `i` holds, if anything: its roster entry's or boss's weapons (a
    /// rare its elite's); a player-looking one wears its looks instead.
    pub(super) fn enemy_weapons(&self, i: usize) -> Option<esurv_content::CreatureWeapons> {
        let zone = &self.run.content.zones[self.run.zone];
        let weapons = match self.run.enemies.get(i)?.kind {
            EnemyKind::Roster(r) | EnemyKind::Rare { roster: r, .. } => zone.enemies[r].weapons,
            // a boss holds the weapons its script gave it (a phase change)
            EnemyKind::Boss(b) => self.run.enemies[i].mech.weapons.unwrap_or(zone.bosses[b].weapons),
        };
        (!weapons.is_empty()).then_some(weapons)
    }

    /// Whether enemy `id`'s swing now is struck with its off hand (`HITINFO_LEFTSWING`): one
    /// with a weapon in each hand swings them in turn, main hand first. Only the look: its
    /// swings' timing and damage stay the simulation's.
    pub(super) fn enemy_swings_left(&mut self, id: u32) -> bool {
        let dual = self
            .run
            .enemies
            .iter()
            .position(|e| e.id == id)
            .and_then(|i| self.enemy_weapons(i))
            .is_some_and(|w| w.dual_wields());
        if !dual {
            return false;
        }
        // in the set: its main hand struck last, so the off hand strikes now
        if self.left_swing_next.remove(&id) {
            true
        } else {
            self.left_swing_next.insert(id);
            false
        }
    }

    /// The aura boss `i` carries from its arrival (Warsong's bosses carry
    /// the heroes' own flag), as its first buff slot's fields; enemies' own debuffs take the slots
    /// after the buffs'.
    fn carried_aura(&self, i: usize) -> Option<Fields> {
        let zone = &self.run.content.zones[self.run.zone];
        let EnemyKind::Boss(b) = self.run.enemies[i].kind else { return None };
        let spell = zone.bosses[b].carries?.against(self.run.party_faction);
        Some(present::aura_fields(&[present::Aura { spell, stacks: 1, until: 0.0 }], 0, 0))
    }

    /// The name of enemy `i` (a rare is named after the roster enemy it copies; one that
    /// looks like a player as it is shown, a ranked one with its title and player name).
    fn enemy_name(&self, i: usize) -> String {
        if let Some(shape) = self.player_shape(i) {
            return shape.name;
        }
        let zone = &self.run.content.zones[self.run.zone];
        match self.run.enemies[i].kind {
            EnemyKind::Roster(r) | EnemyKind::Rare { roster: r, .. } => {
                zone.enemies[r].name.clone()
            }
            EnemyKind::Boss(b) => zone.bosses[b].name.clone(),
        }
    }

    /// The level enemies show: the highest hero level.
    fn enemy_level(&self) -> u32 {
        self.run
            .heroes
            .iter()
            .map(|h| h.progress.level)
            .max()
            .unwrap_or(1)
    }

    /// The level pet `i` shows: its hero's.
    fn pet_level(&self, i: usize) -> u32 {
        let p = &self.run.pets[i];
        self.run.heroes.get(p.hero).map_or(1, |h| h.progress.level)
    }

    /// Pet `i`'s size fields at its level: its scale (a beast's grows with its hero's level),
    /// bounding radius and reach.
    fn pet_size_fields(&self, i: usize) -> Option<Fields> {
        let p = &self.run.pets[i];
        let kind = self.run.content.pets.iter().find(|k| k.name == p.pet)?;
        let scale = kind.scale_at(self.pet_level(i));
        let mut f = Fields::new();
        f.f32(field::OBJECT_SCALE_X, scale)
            .f32(field::UNIT_BOUNDINGRADIUS, scale * PET_BOUNDING_SHARE)
            .f32(field::UNIT_COMBATREACH, scale * PET_REACH_SHARE);
        Some(f)
    }

    fn pet_object(&self, i: usize, now: f32) -> Option<(ObjectUpdate, u64)> {
        let content = &self.run.content;
        let p = &self.run.pets[i];
        let kind = content.pets.iter().find(|k| k.name == p.pet)?;
        let owner = self.run.heroes.get(p.hero)?;
        let guid = unit_guid(kind.entry, p.id);
        let made_by = self.guid_of(Unit::Hero(owner.id)).unwrap_or(0);
        let scale = kind.scale_at(owner.progress.level);
        let at = p.segment.position(now);
        let mut f = Fields::new();
        f.u64(field::OBJECT_GUID, guid)
            .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_UNIT)
            .u32(field::OBJECT_ENTRY, kind.entry)
            .f32(field::OBJECT_SCALE_X, scale);
        f.u64(field::UNIT_SUMMONEDBY, made_by)
            .u64(field::UNIT_CREATEDBY, made_by);
        f.u32(field::UNIT_HEALTH, PET_HEALTH)
            .u32(field::UNIT_MAXHEALTH, PET_HEALTH)
            .u32(field::UNIT_LEVEL, owner.progress.level);
        f.u32(
            field::UNIT_FACTIONTEMPLATE,
            content.races[owner.race].faction_template,
        )
        .u32(field::UNIT_FLAGS, PET_UNIT_FLAGS);
        f.u32(field::UNIT_BASEATTACKTIME, (kind.swing * 1000.0) as u32)
            .f32(field::UNIT_BOUNDINGRADIUS, scale * PET_BOUNDING_SHARE)
            .f32(field::UNIT_COMBATREACH, scale * PET_REACH_SHARE);
        f.u32(field::UNIT_DISPLAYID, kind.display_id)
            .u32(field::UNIT_NATIVEDISPLAYID, kind.display_id)
            .f32(field::UNIT_MOD_CAST_SPEED, 1.0);
        // a lasting look it wears from the start (the Phoenix burns)
        if let Some(spell) = kind.aura {
            let worn = [present::Aura { spell, stacks: 1, until: 0.0 }];
            f.extend(&present::aura_fields(&worn, 0, 0));
        }
        // it appears facing the way its hero faces
        let living = Living {
            position: [at.0, at.1, self.ground_z(at)],
            facing: owner.facing,
            speeds: Speeds::running_at(PET_RUN_SPEED),
        };
        Some((
            ObjectUpdate::Create {
                guid,
                type_id: ObjectTypeId::Unit,
                living,
                fields: f,
                own: false,
            },
            guid,
        ))
    }

    /// `SMSG_MONSTER_MOVE` for what is left of `segment`, its path laid on the ground.
    fn walk(&mut self, guid: u64, segment: &Segment) -> Packet {
        let now = self.run.now;
        let from = segment.position(now);
        let start = [from.0, from.1, self.ground_z(from)];
        let left = segment.start + segment.duration - now;
        let spline = self.next_spline;
        self.next_spline = self.next_spline.wrapping_add(1);
        if left <= 0.0 || dist(from, segment.to) < 0.01 {
            return packets::monster_move(guid, start, spline, &[], 0, true);
        }
        let length = dist(from, segment.to);
        let steps = (length / PATH_STEP).ceil().max(1.0) as usize;
        let path: Vec<[f32; 3]> = (1..=steps)
            .map(|k| {
                let t = k as f32 / steps as f32;
                let at = (
                    from.0 + (segment.to.0 - from.0) * t,
                    from.1 + (segment.to.1 - from.1) * t,
                );
                [at.0, at.1, self.ground_z(at)]
            })
            .collect();
        packets::monster_move(
            guid,
            start,
            spline,
            &path,
            (left * 1000.0).round() as u32,
            true,
        )
    }

    /// Tells every client what changed this tick: new units, their moves and health, what
    /// happened (with the totems and looks on the ground it put down), then the dead (who lie
    /// for [`present::CORPSE_TIME`] before they go).
    fn show(&mut self, events: &[Event]) {
        let now = self.run.now;
        let mut objects = Vec::new();
        let mut walks = Vec::new();
        // the names of new player-looking enemies, which reach the clients before they do
        let mut names = Vec::new();
        // new, moved, hurt
        for i in 0..self.run.enemies.len() {
            let e = &self.run.enemies[i];
            let (id, segment, health) = (e.id, e.segment, whole(e.health));
            match self.shown.get_mut(&id) {
                None => {
                    let (create, guid) = self.enemy_object(i, now);
                    if let Some(shape) = self.player_shape(i) {
                        names.push(self.player_enemy_name(i, &shape));
                    }
                    objects.push(create);
                    if segment.moving(now) {
                        walks.push((guid, segment));
                    }
                    let name = self.enemy_name(i);
                    self.shown.insert(
                        id,
                        Shown {
                            guid,
                            segment,
                            health,
                            name,
                            corpse_until: None,
                            debuffs: Vec::new(),
                            level: 0,
                        },
                    );
                }
                Some(s) => {
                    if s.segment != segment {
                        s.segment = segment;
                        walks.push((s.guid, segment));
                    }
                    if s.health != health {
                        s.health = health;
                        let mut f = Fields::new();
                        f.u32(field::UNIT_HEALTH, health);
                        objects.push(ObjectUpdate::Values {
                            guid: s.guid,
                            fields: f,
                        });
                    }
                }
            }
        }
        for i in 0..self.run.pets.len() {
            let p = &self.run.pets[i];
            let (id, segment) = (p.id, p.segment);
            let level = self.pet_level(i);
            let size = self.pet_size_fields(i);
            match self.shown.get_mut(&id) {
                None => {
                    let name = p.pet.clone();
                    if let Some((create, guid)) = self.pet_object(i, now) {
                        objects.push(create);
                        if segment.moving(now) {
                            walks.push((guid, segment));
                        }
                        self.shown.insert(
                            id,
                            Shown {
                                guid,
                                segment,
                                health: 0,
                                name,
                                corpse_until: None,
                                debuffs: Vec::new(),
                                level,
                            },
                        );
                    }
                }
                Some(s) => {
                    if s.segment != segment {
                        s.segment = segment;
                        walks.push((s.guid, segment));
                    }
                    // its hero went up a level: its level, and a beast's size with it
                    if s.level != level {
                        s.level = level;
                        if let Some(mut f) = size {
                            f.u32(field::UNIT_LEVEL, level);
                            objects.push(ObjectUpdate::Values {
                                guid: s.guid,
                                fields: f,
                            });
                        }
                    }
                }
            }
        }
        // heroes
        for h in &self.run.heroes {
            let now_shown = HeroShown::of(h, &self.run.content.balance.experience);
            if self.heroes_shown.insert(h.id, now_shown) != Some(now_shown) {
                let Some(player) = self.players.iter().find(|p| p.hero == h.id) else {
                    continue;
                };
                objects.push(now_shown.values(player.guid()));
            }
        }
        let mut aura_timers = Vec::new();
        // a run going on after a pause (a choice, the Escape menu) counts the buffs down again from
        // where they stood, while the clients went on counting: their timers are told again
        let resumed = self.run.phase == Phase::Running && !self.moving_shown;
        self.note_option_events(events);
        self.auras(&mut objects, &mut aura_timers, resumed);
        self.mirror_channels(&mut objects);
        self.power_fields(&mut objects);
        let form_looks = self.shift_forms(events, &mut objects);
        let (mut pickup_moves, mut pickups_gone) = (Vec::new(), Vec::new());
        self.mirror_pickups(&mut objects, &mut pickup_moves, &mut pickups_gone);
        let (mut totems, mut totems_gone) = (Vec::new(), Vec::new());
        self.mirror_totems(&mut totems, &mut totems_gone);
        self.mirror_clones(&mut totems, &mut totems_gone);
        let mut out = names;
        if !objects.is_empty() {
            out.push(update_object(&objects));
        }
        out.extend(form_looks);
        for (guid, segment) in walks {
            out.push(self.walk(guid, &segment));
        }
        out.append(&mut pickup_moves);
        // what happened
        let (told, mut own) = self.present(events);
        out.extend(told);
        // a totem appears after the cast that placed it
        if !totems.is_empty() {
            out.push(update_object(&totems));
        }
        out.append(&mut totems_gone);
        self.expire_ground_looks(&mut out);
        self.mirror_boss_looks(&mut out);
        own.extend(self.mirror_indicators());
        self.mirror_bunnies(&mut out);
        // the dead lie down where they stand; pets that left vanish
        let died: HashSet<u32> = events
            .iter()
            .filter_map(|e| match e {
                Event::EnemyDied { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        let live: HashSet<u32> = self
            .run
            .enemies
            .iter()
            .map(|e| e.id)
            .chain(self.run.pets.iter().map(|p| p.id))
            .collect();
        let gone: Vec<u32> = self
            .shown
            .iter()
            .filter(|(id, s)| !live.contains(id) && s.corpse_until.is_none())
            .map(|(id, _)| *id)
            .collect();
        let mut fallen = Vec::new();
        for id in gone {
            if died.contains(&id) {
                let segment = self.shown[&id].segment;
                let guid = self.shown[&id].guid;
                if segment.moving(now) {
                    let stop = Segment::standing(segment.position(now), now);
                    out.push(self.walk(guid, &stop));
                }
                // dead: health 0, and its debuffs gone
                let mut f =
                    present::aura_fields(&[], self.shown[&id].debuffs.len(), present::BUFF_SLOTS);
                f.u32(field::UNIT_HEALTH, 0);
                fallen.push(ObjectUpdate::Values { guid, fields: f });
                if let Some(s) = self.shown.get_mut(&id) {
                    s.health = 0;
                    s.corpse_until = Some(now + present::CORPSE_TIME);
                }
            } else if let Some(s) = self.shown.remove(&id) {
                out.push(packets::destroy_object(s.guid));
            }
        }
        if !fallen.is_empty() {
            out.push(update_object(&fallen));
        }
        out.append(&mut pickups_gone);
        let cleared: Vec<u32> = self
            .shown
            .iter()
            .filter(|(_, s)| s.corpse_until.is_some_and(|t| now >= t))
            .map(|(id, _)| *id)
            .collect();
        for id in cleared {
            if let Some(s) = self.shown.remove(&id) {
                out.push(packets::destroy_object(s.guid));
            }
        }
        // while the run stands (a choice, a pause, its end) what walked stands where it is, and it
        // walks on with the run
        // a scripted pause stops everyone
        let moving = self.run.phase == Phase::Running && !self.run.in_cutscene();
        if moving != self.moving_shown {
            self.moving_shown = moving;
            let walking: Vec<(u64, Segment)> = self
                .shown
                .values()
                .filter(|s| s.corpse_until.is_none() && s.segment.moving(now))
                .map(|s| (s.guid, s.segment))
                .collect();
            for (guid, segment) in walking {
                let go = if moving {
                    segment
                } else {
                    Segment::standing(segment.position(now), now)
                };
                out.push(self.walk(guid, &go));
            }
        }
        // the heroes stand rooted while a choice is open, the run is paused or won, and move again
        // with the run; a fall roots nothing
        let rooted = matches!(
            self.run.phase,
            Phase::Choosing | Phase::Paused | Phase::Victory
        ) || self.run.in_cutscene();
        if rooted != self.rooted && (rooted || moving) {
            self.rooted = rooted;
            self.next_root = self.next_root.wrapping_add(1);
            for p in self.players.iter().filter(|p| !self.left.contains(&p.hero)) {
                own.push((
                    p.hero,
                    packets::force_move_root(p.guid(), self.next_root, rooted),
                ));
            }
        }
        // gear won joins the collection at once, with a line in its hero's chat
        // the jewelry taken shows in its hero's own character pane
        own.extend(self.mirror_jewelry(events));
        // a hero whose speed changed (a speed rune, its end) runs that fast on every client
        own.extend(self.mirror_speeds());
        own.extend(self.hand_out_loot(events));
        own.extend(self.settle(events));
        // each cast's bar sweep shows the run's cooldown, not its look's own
        own.extend(self.bar_sweeps_after_casts(&out));
        let mut lost = Vec::new();
        for p in &self.players {
            // a buff's timer goes before the aura fields that name it, as the client expects
            let timers: Vec<Packet> = aura_timers
                .iter()
                .filter(|(h, _)| *h == p.hero)
                .map(|(_, pk)| pk.clone())
                .collect();
            let mine: Vec<Packet> = own
                .iter()
                .filter(|(h, _)| *h == p.hero)
                .map(|(_, pk)| pk.clone())
                .collect();
            if !(p.send(&timers) && p.send(&out) && p.send(&mine)) {
                lost.push(p.hero);
            }
        }
        // a client whose socket failed has gone
        self.players.retain(|p| !lost.contains(&p.hero));
    }
}

fn whole(v: f32) -> u32 {
    v.max(0.0).ceil() as u32
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Which way a unit on `segment` faces: along its move, else east.
fn facing_of(segment: &Segment) -> f32 {
    let (dx, dy) = (segment.to.0 - segment.from.0, segment.to.1 - segment.from.1);
    if dx.abs() + dy.abs() < 1e-4 {
        0.0
    } else {
        dy.atan2(dx)
    }
}

#[cfg(test)]
mod tests {
    use std::net::{TcpListener, TcpStream};

    use benilla_protocol::messages::parse_server;
    use benilla_protocol::{decode, EntityKind, ObjectFields, ServerPacket, SessionEvent};
    use esurv_sim::game::abilities::Instance;
    use esurv_sim::game::casting::Totem;
    use esurv_sim::game::run::Aim;

    use super::*;

    /// The units an `SMSG_SPELL_GO` names as hit (its hit list's guids).
    fn spell_go_hits(p: &Packet) -> Vec<u64> {
        let b = &p.body;
        let mut at = 0usize;
        let mut packed = |at: &mut usize| {
            let mask = b[*at];
            *at += 1 + mask.count_ones() as usize;
        };
        packed(&mut at);
        packed(&mut at);
        at += 4 + 2;
        let n = b[at] as usize;
        at += 1;
        (0..n).map(|k| u64::from_le_bytes(b[at + 8 * k..at + 8 * k + 8].try_into().unwrap())).collect()
    }

    /// The caster an `SMSG_SPELL_GO` names (its first packed guid).
    fn spell_go_caster(p: &Packet) -> u64 {
        let b = &p.body;
        let mask = b[0];
        let mut guid = 0u64;
        let mut at = 1;
        for i in 0..8 {
            if mask & (1 << i) != 0 {
                guid |= (b[at] as u64) << (8 * i);
                at += 1;
            }
        }
        guid
    }

    /// A boss's attacks hit the hero only, and look it: every ability of
    /// every reworked boss, its phases' kits and its figures' too, cast with enemies of the horde all
    /// round the hero and landed, names no enemy as hit in what the clients are told (so no enemy
    /// flinches), and no enemy takes damage from it or dies of it.
    #[test]
    fn a_boss_ability_names_no_enemy_as_hit() {
        let content =
            Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
        let mut checked = 0;
        for zone_id in ["elwynn", "duskwood", "scarlet", "sunken_temple", "feralas", "arena", "strat", "brm", "aq"] {
            let zone = content.zones.iter().position(|z| z.id == zone_id).unwrap();
            let saved = SavedHero {
                guid: 1,
                name: "Thrall".into(),
                class: "warrior".into(),
                race: "orc".into(),
                gender: 0,
                look: [0; 5],
                records: Default::default(),
                zone: None,
                start: None,
            };
            let (_host, inputs) = std::sync::mpsc::channel();
            let mut game = Game::new(content.clone(), None, zone, &saved, Arc::new(Saves::in_memory()), inputs).unwrap();
            let bosses = content.zones[zone].bosses.clone();
            for (b, boss) in bosses.iter().enumerate() {
                // every phase's kit, and the figures' its script raises (Ragnaros)
                let kits: Vec<_> = boss.kits().into_iter().chain(boss.cameo_kits()).collect();
                for ability in kits.into_iter().flatten() {
                    game.run.enemies.clear();
                    game.run.telegraphs.clear();
                    game.run.ground_zones.clear();
                    game.run.ground_objects.clear();
                    // the hero outlasts every boss checked
                    game.run.heroes[0].stats.max_health = 1e12;
                    game.run.heroes[0].health = 1e12;
                    game.run.heroes[0].alive = true;
                    let p = game.run.heroes[0].pos;
                    // the horde all round the hero, struck by nothing but the boss's ability
                    for k in 0..8 {
                        let a = k as f32 * std::f32::consts::TAU / 8.0;
                        game.run.spawn_roster_enemy(0, Some((p.0 + a.cos() * 3.0, p.1 + a.sin() * 3.0)));
                    }
                    for e in &mut game.run.enemies {
                        e.damage = 0.0;
                        e.health = 1e9;
                        e.max_health = 1e9;
                    }
                    // the hero whole and standing again (an earlier ability may have felled it), no
                    // pause left over
                    game.run.heroes[0].alive = true;
                    game.run.heroes[0].stats.max_health = 1e6;
                    game.run.heroes[0].health = 1e6;
                    game.run.cutscene = None;
                    game.run.spawn_boss(b);
                    let k = game.run.enemies.iter().position(|e| e.is_boss()).unwrap();
                    // it fights at once, not behind a run of waves
                    game.run.enemies[k].mech.phases = Default::default();
                    game.run.enemies[k].mech.busy_until = 0.0;
                    game.run.enemies[k].segment = Segment::standing((p.0 + 4.0, p.1), game.run.now);
                    game.run.enemies[k].hold_until = 0.0;
                    game.run.enemies[k].next_ability_at = 0.0;
                    game.run.enemies[k].abilities = vec![esurv_sim::game::enemy_attacks::AbilitySlot { ability: ability.clone(), timer: 0.0 }];
                    game.run.drain_events();
                    let horde: HashSet<u32> = game.run.enemies.iter().filter(|e| !e.is_boss()).map(|e| e.id).collect();
                    game.run.phase = Phase::Running;
                    let mut events = Vec::new();
                    let mut told = Vec::new();
                    for _ in 0..40 {
                        game.run.tick(0.1);
                        let ev = game.run.drain_events();
                        told.extend(game.present(&ev).0);
                        events.extend(ev);
                    }
                    let enemy_guids: HashSet<u64> = horde.iter().filter_map(|&id| game.guid_of(Unit::Enemy(id))).collect();
                    // (a look a unit plays on itself, an add healed or empowered, is no hit)
                    for p in told.iter().filter(|p| p.opcode == benilla_protocol::messages::opcode::SMSG_SPELL_GO) {
                        let caster = spell_go_caster(p);
                        assert!(spell_go_hits(p).iter().all(|g| *g == caster || !enemy_guids.contains(g)), "{zone_id} {}: {:?} names an enemy as hit", boss.name, ability.kind);
                    }
                    for e in &events {
                        match e {
                            Event::SpellGo { caster: caster @ Unit::Enemy(_), hits, .. } => {
                                assert!(hits.iter().all(|u| u == caster || !matches!(u, Unit::Enemy(id) if horde.contains(id))), "{} {:?}", boss.name, ability.kind)
                            }
                            Event::EnemyHit { id, by: None, .. } | Event::EnemyDied { id, by: None } => {
                                assert!(!horde.contains(id), "{zone_id} {}: {:?} struck an enemy of the horde", boss.name, ability.kind)
                            }
                            _ => {}
                        }
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked >= 15, "{checked}");
    }

    /// A boss look that shows only on a unit it hits (Whitemane's Holy
    /// Smite) plays on an invisible target unit put where its mark lands, though the hero dodged
    /// it: the bunny's create, the spell going off naming the bunny (no enemy) as hit, the hero
    /// unhurt; and the bunny goes a moment later.
    #[test]
    fn a_dodged_mark_still_shows_its_impact_on_a_bunny() {
        let content =
            Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
        let zone = content.zones.iter().position(|z| z.id == "scarlet").unwrap();
        let saved = SavedHero {
            guid: 1,
            name: "Thrall".into(),
            class: "warrior".into(),
            race: "orc".into(),
            gender: 0,
            look: [0; 5],
            records: Default::default(),
            zone: None,
            start: None,
        };
        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(content.clone(), None, zone, &saved, Arc::new(Saves::in_memory()), inputs).unwrap();
        let b = content.zones[zone].bosses.iter().position(|b| b.name == "High Inquisitor Whitemane").unwrap();
        let smite = content.zones[zone].bosses[b].abilities[0].clone();
        assert!(matches!(smite.kind, esurv_content::BossAbilityKind::Marked { impact_spell: 9481, impact_on_bunny: true, .. }));
        game.run.enemies.clear();
        let p = game.run.heroes[0].pos;
        game.run.next_boss = content.zones[zone].bosses.len();
        game.run.spawn_boss(b);
        let k = game.run.enemies.len() - 1;
        game.run.enemies[k].segment = Segment::standing((p.0 + 4.0, p.1), game.run.now);
        game.run.enemies[k].hold_until = 0.0;
        game.run.enemies[k].next_ability_at = 0.0;
        game.run.enemies[k].abilities = vec![esurv_sim::game::enemy_attacks::AbilitySlot { ability: smite, timer: 0.0 }];
        game.show(&[]);
        // past her arrival's cutscene
        game.run.cutscene = None;
        game.run.extras.later.clear();
        game.run.phase = Phase::Running;
        game.run.tick(0.1);
        assert!(!game.run.telegraphs.is_empty(), "marked");
        // the hero walks clear of every mark
        game.run.heroes[0].pos = (p.0 - 40.0, p.1);
        game.run.enemies[0].abilities.clear();
        game.run.spawn_rules.stopped = true;
        game.run.drain_events();
        let enemies_before = game.run.enemies.len();
        let mut told = Vec::new();
        let mut hurt = Vec::new();
        for _ in 0..20 {
            game.run.tick(0.1);
            let ev = game.run.drain_events();
            hurt.extend(ev.iter().filter(|e| matches!(e, Event::HeroHit { by: None, .. })).cloned());
            told.extend(game.present(&ev).0);
        }
        assert!(hurt.is_empty(), "dodged: {hurt:?}");
        let go: Vec<&Packet> = told.iter().filter(|p| p.opcode == benilla_protocol::messages::opcode::SMSG_SPELL_GO && u32::from_le_bytes(p.body[spell_go_spell_at(p)..spell_go_spell_at(p) + 4].try_into().unwrap()) == 9481).collect();
        assert!(!go.is_empty(), "the smite's look went off");
        let bunnies: Vec<u64> = game.bunnies.iter().map(|(g, _)| *g).collect();
        assert!(!bunnies.is_empty());
        for p in &go {
            let hits = spell_go_hits(p);
            assert!(!hits.is_empty() && hits.iter().all(|g| bunnies.contains(g)), "it names only bunnies as hit");
        }
        let enemies: Vec<u64> = game.run.enemies.iter().filter_map(|e| game.guid_of(Unit::Enemy(e.id))).collect();
        assert!(bunnies.iter().all(|b| !enemies.contains(b)), "no bunny is an enemy");
        assert_eq!(game.run.enemies.len(), enemies_before, "the simulation has no unit for it");
        // a moment later they go
        game.run.now += esurv_sim::game::telegraphs::BUNNY_LINGER + 0.1;
        let mut out = Vec::new();
        game.mirror_bunnies(&mut out);
        assert!(game.bunnies.is_empty());
        assert_eq!(out.iter().filter(|p| p.opcode == benilla_protocol::messages::opcode::SMSG_DESTROY_OBJECT).count(), bunnies.len());
    }

    /// Where an `SMSG_SPELL_GO`'s spell id sits (after its two packed guids).
    fn spell_go_spell_at(p: &Packet) -> usize {
        let b = &p.body;
        let mut at = 0usize;
        for _ in 0..2 {
            let mask = b[at];
            at += 1 + mask.count_ones() as usize;
        }
        at
    }

    /// A run with one orc shaman in it, its client at the other end of a local socket (unread).
    fn game_with_a_player() -> (Game, TcpStream) {
        game_with_a_shaman_of("orc")
    }

    /// [`game_with_a_player`] with a shaman of `race`.
    fn game_with_a_shaman_of(race: &str) -> (Game, TcpStream) {
        game_with_a_hero_of("shaman", race)
    }

    /// A run with one hero of `class` and `race` in it, its client at the other end of a local
    /// socket (unread).
    fn game_with_a_hero_of(class: &str, race: &str) -> (Game, TcpStream) {
        let content =
            Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
        let saved = SavedHero {
            guid: 1,
            name: "Thrall".into(),
            class: class.into(),
            race: race.into(),
            gender: 0,
            look: [0; 5],
            records: Default::default(),
            zone: None,
            start: None,
        };
        let (_host, inputs) = std::sync::mpsc::channel();
        let saves = Arc::new(Saves::in_memory());
        let mut game = Game::new(content, None, 0, &saved, saves, inputs).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        let hero = game.hero_id(0);
        assert!(game.join(Player {
            hero,
            saved,
            writer: Arc::new(Mutex::new(PacketWriter::new(server))),
        }));
        (game, client)
    }

    fn field_of(fields: &ObjectFields, index: u16) -> Option<u32> {
        fields
            .raw_fields()
            .find(|&(i, _)| i == index)
            .map(|(_, v)| v)
    }

    fn creates(p: &Packet) -> Vec<SessionEvent> {
        decode(parse_server(p.opcode, &p.body).unwrap())
            .into_iter()
            .filter(|e| matches!(e, SessionEvent::ObjectCreate { .. }))
            .collect()
    }

    #[test]
    fn a_totem_is_a_unit_its_hero_made_and_it_just_goes() {
        let (mut game, _client) = game_with_a_player();
        let hero = game.players[0].guid();
        let at = game.run.heroes[0].pos;
        game.run.totems.push(Totem {
            id: 77,
            hero: 0,
            spell: "magma_totem".into(),
            at,
            until: game.run.now + 10.0,
            next_pulse: game.run.now + 0.5,
            interval: 2.0,
            inst: Instance::default(),
            from_proc: false,
        });
        let (mut made, mut gone) = (Vec::new(), Vec::new());
        game.mirror_totems(&mut made, &mut gone);
        assert!(gone.is_empty());
        let [SessionEvent::ObjectCreate {
            guid,
            kind: EntityKind::Unit,
            display_id: Some(4589),
            fields,
            ..
        }] = &creates(&update_object(&made))[..]
        else {
            panic!("one unit")
        };
        assert_eq!(*guid, unit_guid(TOTEM_ENTRY, 77));
        assert_eq!(game.guid_of(Unit::Totem(77)), Some(*guid));
        assert_eq!(field_of(fields, field::OBJECT_ENTRY), Some(TOTEM_ENTRY));
        assert_eq!(field_of(fields, field::UNIT_SUMMONEDBY), Some(hero as u32));
        assert_eq!(field_of(fields, field::UNIT_CREATEDBY), Some(hero as u32));
        assert_eq!(field_of(fields, field::UNIT_HEALTH), Some(5));
        // its time is up: it is destroyed, nothing more
        game.run.totems.clear();
        let (mut made, mut gone) = (Vec::new(), Vec::new());
        game.mirror_totems(&mut made, &mut gone);
        assert!(made.is_empty());
        assert_eq!(gone, [packets::destroy_object(*guid)]);
    }

    /// A dwarf's totems show the composed Ironfoe look of their stock look;
    /// an orc's keep the stock one.
    #[test]
    fn a_dwarf_shamans_totems_wear_their_own_look() {
        for (race, searing, magma) in [("dwarf", 9_304_587, 9_304_589), ("orc", 4587, 4589)] {
            let (mut game, _client) = game_with_a_shaman_of(race);
            let at = game.run.heroes[0].pos;
            for (id, spell) in [(1, "searing_totem"), (2, "magma_totem")] {
                game.run.totems.push(Totem {
                    id,
                    hero: 0,
                    spell: spell.into(),
                    at,
                    until: game.run.now + 10.0,
                    next_pulse: game.run.now + 0.5,
                    interval: 2.0,
                    inst: Instance::default(),
                    from_proc: false,
                });
            }
            let (mut made, mut gone) = (Vec::new(), Vec::new());
            game.mirror_totems(&mut made, &mut gone);
            let looks: Vec<Option<u32>> = creates(&update_object(&made))
                .into_iter()
                .map(|e| match e {
                    SessionEvent::ObjectCreate { display_id, .. } => display_id,
                    _ => None,
                })
                .collect();
            assert_eq!(looks, [Some(searing), Some(magma)], "{race}");
        }
    }

    #[test]
    fn an_area_cast_is_sized_just_before_it_goes_off() {
        let (mut game, _client) = game_with_a_player();
        let hero = Unit::Hero(game.run.heroes[0].id);
        let me = game.players[0].guid();
        let nova = |reach| Event::SpellGo {
            caster: hero,
            spell: 122,
            hits: vec![],
            aim: Aim::Caster,
            ammo: false,
            reach,
            triggered: false,
        };
        // `frost_nova`'s look is drawn 10 yards across: a 5-yard nova shows at half its size
        let (told, _) = game.present(&[nova(5.0)]);
        assert_eq!(told.len(), 2, "its size, then the cast");
        let Ok(ServerPacket::MessageChat(line)) = parse_server(told[0].opcode, &told[0].body)
        else {
            panic!("a size line first")
        };
        assert!(line.is_addon());
        assert_eq!(line.text, format!("ESURVS	{me:X} 122 0.500"));
        assert!(matches!(
            parse_server(told[1].opcode, &told[1].body),
            Ok(ServerPacket::SpellGo(go)) if go.spell_id == 122
        ));
        // at its own size, or not an area: just the cast
        assert_eq!(game.present(&[nova(10.0)]).0.len(), 1);
        assert_eq!(game.present(&[nova(0.0)]).0.len(), 1);
    }

    /// Explosive Shot blasts twice, half a second apart; Fan of Knives spins, then fourteen knives
    /// fly out all around with their sounds.
    #[test]
    fn the_options_extra_sounds_and_knives_come_in_their_order() {
        use benilla_protocol::ServerPacket;
        let (mut game, _client) = game_with_a_player();
        let hero = game.run.heroes[0].id;
        let me = game.players[0].guid();
        let sounds = |told: &[Packet]| -> Vec<u32> {
            told.iter()
                .filter_map(|p| match parse_server(p.opcode, &p.body) {
                    Ok(ServerPacket::PlayObjectSound { sound_id, guid }) if guid == me => Some(sound_id),
                    _ => None,
                })
                .collect()
        };
        let shot = Event::SpellCast { hero, spell: "explosive_shot".into() };
        let (told, _) = game.present(&[shot]);
        assert_eq!(sounds(&told), [starting_looks::extras::DYNAMITE]);
        assert!(sounds(&game.present(&[]).0).is_empty(), "the second waits");
        game.run.now += 0.6;
        assert_eq!(sounds(&game.present(&[]).0), [starting_looks::extras::DYNAMITE]);
        let fan = Event::SpellCast { hero, spell: "fan_of_knives".into() };
        let spin = game.present(&[fan]).0;
        assert_eq!(spin.len(), 1, "the spin first, the knives after it");
        game.run.now += 1.0;
        let told = game.present(&[]).0;
        let knives = told.iter().filter(|p| p.opcode == benilla_protocol::messages::opcode::SMSG_SPELL_GO).count();
        assert_eq!(knives, starting_looks::extras::KNIVES as usize);
        assert_eq!(
            sounds(&told),
            [starting_looks::extras::THROW_ITEM, starting_looks::extras::DAGGER_IN_FLESH_CRITICAL]
        );
    }

    /// Divine Star flies out to where it lands, then back to the priest from a look left there.
    #[test]
    fn divine_star_flies_back_to_the_priest_from_where_it_lands() {
        use benilla_protocol::ServerPacket;
        use esurv_sim::game::run::Aim;
        let (mut game, _client) = game_with_a_player();
        let hero = game.run.heroes[0].id;
        let me = game.players[0].guid();
        let star = starting_looks::extras::DIVINE_STAR;
        let end = (game.run.heroes[0].pos.0 + 24.0, game.run.heroes[0].pos.1);
        let out = Event::SpellGo { caster: Unit::Hero(hero), spell: star, hits: vec![], aim: Aim::Ground(end), ammo: false, reach: 3.0, triggered: false };
        let (told, _) = game.present(&[out]);
        let turn = told
            .iter()
            .flat_map(creates)
            .find_map(|e| match e {
                SessionEvent::ObjectCreate { guid, kind: EntityKind::DynamicObject, .. } => Some(guid),
                _ => None,
            })
            .expect("a look where it lands");
        let back = |told: &[Packet]| {
            told.iter().any(|p| {
                matches!(parse_server(p.opcode, &p.body),
                    Ok(ServerPacket::SpellGo(go)) if go.spell_id == star && go.caster == turn && go.hits == [me])
            })
        };
        assert!(!back(&told), "not before it lands");
        game.run.now += 1.1;
        assert!(back(&game.present(&[]).0), "it flies back to the priest");
    }

    /// Lacerate plays Shred's sound on each unit its bleed's looks reached; Rend alone plays none.
    #[test]
    fn lacerate_sounds_like_shred_on_each_unit_it_reaches() {
        use benilla_protocol::ServerPacket;
        use esurv_sim::game::run::Aim;
        let (mut game, _client) = game_with_a_player();
        let hero = game.run.heroes[0].id;
        let me = game.players[0].guid();
        let sounds = |told: &[Packet]| -> usize {
            told.iter()
                .filter(|p| {
                    matches!(parse_server(p.opcode, &p.body),
                        Ok(ServerPacket::PlayObjectSound { sound_id, guid }) if guid == me && sound_id == starting_looks::extras::CAT_ATTACK)
                })
                .count()
        };
        let rend = || Event::SpellGo {
            caster: Unit::Hero(hero),
            spell: starting_looks::extras::REND,
            hits: vec![Unit::Hero(hero)],
            aim: Aim::Caster,
            ammo: false,
            reach: 0.0,
            triggered: false,
        };
        let cast = Event::SpellCast { hero, spell: "lacerate".into() };
        assert_eq!(sounds(&game.present(&[rend(), rend(), cast]).0), 2);
        let other = Event::SpellCast { hero, spell: "rend".into() };
        assert_eq!(sounds(&game.present(&[rend(), other]).0), 0);
    }

    #[test]
    fn a_look_on_the_ground_is_the_heros_spell_sized_to_the_area_until_it_goes() {
        let (mut game, _client) = game_with_a_player();
        let hero = game.run.heroes[0].id;
        let me = game.players[0].guid();
        let until = game.run.now + 1.0;
        let (told, _) = game.present(&[Event::AreaShown {
            id: 5,
            caster: Unit::Hero(hero),
            spell: 10,
            at: (1.0, 2.0),
            radius: 9.0,
            until: Some(until),
        }]);
        let [SessionEvent::ObjectCreate {
            guid,
            kind: EntityKind::DynamicObject,
            position,
            fields,
            ..
        }] = &creates(&told[0])[..]
        else {
            panic!("one dynamic object")
        };
        assert_eq!(*guid, ground_looks::ground_look_guid(5));
        assert_eq!((position[0], position[1]), (1.0, 2.0));
        assert_eq!(
            field_of(fields, field::OBJECT_SCALE_X),
            Some((9.0f32 / 8.0).to_bits())
        );
        assert_eq!(field_of(fields, field::DYNAMICOBJECT_SPELLID), Some(10));
        assert_eq!(
            field_of(fields, field::DYNAMICOBJECT_CASTER),
            Some(me as u32)
        );
        assert_eq!(
            field_of(fields, field::DYNAMICOBJECT_RADIUS),
            Some(9.0f32.to_bits())
        );
        assert_eq!(
            game.ground_looks_now().len(),
            1,
            "a player who joins sees it"
        );
        let mut out = Vec::new();
        game.expire_ground_looks(&mut out);
        assert!(out.is_empty(), "not yet");
        game.run.now = until;
        game.expire_ground_looks(&mut out);
        assert_eq!(out, [packets::destroy_object(*guid)]);
        // a boss's mark goes when the simulation says
        let (told, _) = game.present(&[Event::AreaShown {
            id: 6,
            caster: Unit::Hero(hero),
            spell: 10,
            at: (1.0, 2.0),
            radius: 5.0,
            until: None,
        }]);
        assert_eq!(told.len(), 1);
        let (told, _) = game.present(&[Event::AreaGone { id: 6 }]);
        assert_eq!(
            told,
            [packets::destroy_object(ground_looks::ground_look_guid(6))]
        );
    }

    /// An area that moves with its hero or grows is put down again where it now is, at its new
    /// size, with its spell and time kept.
    #[test]
    fn a_moved_or_grown_area_is_put_down_again_where_it_now_is() {
        let (mut game, _client) = game_with_a_player();
        let hero = game.run.heroes[0].id;
        let until = Some(game.run.now + 5.0);
        let shown = Event::AreaShown { id: 7, caster: Unit::Hero(hero), spell: 10, at: (1.0, 2.0), radius: 8.0, until };
        game.present(&[shown]);
        let (told, _) = game.present(&[Event::AreaChanged { id: 7, at: (4.0, 2.0), radius: 12.0 }]);
        let guid = ground_looks::ground_look_guid(7);
        assert_eq!(told.len(), 2, "the old one goes, the new one comes");
        assert_eq!(told[0], packets::destroy_object(guid));
        let [SessionEvent::ObjectCreate { position, fields, .. }] = &creates(&told[1])[..] else {
            panic!("one dynamic object")
        };
        assert_eq!((position[0], position[1]), (4.0, 2.0));
        assert_eq!(field_of(fields, field::DYNAMICOBJECT_SPELLID), Some(10));
        assert_eq!(field_of(fields, field::OBJECT_SCALE_X), Some(1.5f32.to_bits()));
        let (told, _) = game.present(&[Event::AreaChanged { id: 99, at: (4.0, 2.0), radius: 12.0 }]);
        assert!(told.is_empty(), "nothing for a look not shown");
    }

    /// An enemy turned on the others swings at its target as an enemy swings at the hero, and a
    /// mark on an enemy shows as its aura.
    #[test]
    fn a_turned_enemy_s_blow_shows_as_its_swing_and_a_mark_as_an_aura() {
        use esurv_sim::game::casting::HitSource;
        let (mut game, _client) = game_with_a_player();
        let p = game.run.heroes[0].pos;
        assert!(game.run.spawn_roster_enemy(0, Some((p.0 + 3.0, p.1))));
        assert!(game.run.spawn_roster_enemy(0, Some((p.0 + 4.0, p.1))));
        let (a, b) = (game.run.enemies[0].id, game.run.enemies[1].id);
        game.show(&[]);
        let (ga, gb) = (game.guid_of(Unit::Enemy(a)).unwrap(), game.guid_of(Unit::Enemy(b)).unwrap());
        let hero = Some(game.run.heroes[0].id);
        let blow = Event::EnemyHit { id: b, amount: 7.0, crit: false, by: hero, source: HitSource::Enemy(a) };
        let (told, _) = game.present(&[blow]);
        assert_eq!(told, [packets::attacker_state(ga, gb, 7, false)]);
        let until = game.run.now + 5.0;
        game.run.enemies[0].kit.attacker_mark = Some((0, until, 6572));
        let (mut objects, mut own) = (Vec::new(), Vec::new());
        game.auras(&mut objects, &mut own, false);
        assert!(objects.iter().any(|o| matches!(o, crate::update::ObjectUpdate::Values { guid, .. } if *guid == ga)));
        assert!(game.shown[&a].debuffs.iter().any(|d| d.spell == 6572), "the mark shows on the marked enemy");
        assert!(game.shown[&b].debuffs.is_empty());
    }

    /// A voiced boss's pull voice line goes to every client as `SMSG_PLAY_SOUND` when it arrives,
    /// beside its yell; an unvoiced boss's arrival sends no sound.
    #[test]
    fn a_voiced_boss_s_pull_line_plays_and_an_unvoiced_one_s_does_not() {
        let (mut game, _client) = game_with_a_player();
        assert_eq!(game.run.content.zones[game.run.zone].id, "elwynn");
        let sounds = |game: &mut Game, boss: usize| {
            game.run.drain_events();
            game.run.spawn_boss(boss);
            let events = game.run.drain_events();
            game.show(&events);
            let (all, _) = game.present(&events);
            all.into_iter().filter(|p| p.opcode == benilla_protocol::messages::opcode::SMSG_PLAY_SOUND).collect::<Vec<_>>()
        };
        assert_eq!(sounds(&mut game, 4), [packets::play_sound(5780)], "Edwin VanCleef's own line");
        assert!(sounds(&mut game, 0).is_empty(), "Hogger has no voice");
    }

    /// Every boss's pull voice line, and every voice its script plays, is a `SoundEntries.dbc`
    /// row of the player's client.
    #[test]
    fn every_boss_pull_line_is_a_client_sound() {
        let dir = benilla_formats::wow_data_or_skip!();
        let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let kits = benilla_formats::load_sound_kit_catalog(&mut benilla_formats::open_chain(&dir).unwrap()).unwrap();
        let mut checked = 0;
        for z in &content.zones {
            for b in &z.bosses {
                if let Some(id) = b.aggro_sound {
                    let kit = kits.get(id).unwrap_or_else(|| panic!("{} {}: sound {id} is not in SoundEntries.dbc", z.id, b.name));
                    assert!(!kit.files.is_empty(), "{} {}: sound {id} has no file", z.id, b.name);
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 19);
        // and every voice a boss script plays with a yell (Ragnaros's "TOO SOON!", Rajaxx's rallies)
        fn voices(a: &esurv_content::ScriptAction, out: &mut Vec<u32>) {
            match a {
                esurv_content::ScriptAction::Yell { sound: Some(s), .. } => out.push(*s),
                esurv_content::ScriptAction::Cameo { run, .. } => run.iter().for_each(|n| voices(n, out)),
                other => other.nested().into_iter().for_each(|n| voices(n, out)),
            }
        }
        let mut played = Vec::new();
        content.zones.iter().flat_map(|z| &z.bosses).flat_map(|b| &b.script).flat_map(|l| &l.run).for_each(|a| voices(a, &mut played));
        for id in &played {
            assert!(kits.get(*id).is_some_and(|k| !k.files.is_empty()), "script voice {id} is not a client sound");
        }
        assert!(played.len() >= 14, "{}", played.len());
    }

    /// A legendary moment's lasting look shows as an aura on the unit
    /// that wears it, the hero's with its time, and goes when its time is up.
    #[test]
    fn a_legendary_moment_s_lasting_look_shows_as_an_aura_while_it_lasts() {
        let (mut game, _client) = game_with_a_player();
        let p = game.run.heroes[0].pos;
        assert!(game.run.spawn_roster_enemy(0, Some((p.0 + 3.0, p.1))));
        let a = game.run.enemies[0].id;
        game.show(&[]);
        let now = game.run.now;
        game.run.enemies[0].kit.worn_looks.push((11020, now + 1.0));
        game.run.heroes[0].kit.worn_looks.push((9632, now + 3.0));
        let (mut objects, mut own) = (Vec::new(), Vec::new());
        game.auras(&mut objects, &mut own, false);
        assert!(game.shown[&a].debuffs.iter().any(|d| d.spell == 11020), "the stone look on the stunned enemy");
        let hero = game.run.heroes[0].id;
        assert!(game.auras_shown[&hero].iter().any(|x| x.spell == 9632 && (x.until - (now + 3.0)).abs() < 1e-4), "the spree's whirl on the hero");
        game.run.now += 2.0;
        game.auras(&mut objects, &mut own, false);
        assert!(!game.shown[&a].debuffs.iter().any(|d| d.spell == 11020), "gone with its time");
    }

    /// With the player's client data present, every look `legendary_looks.ron` names and every
    /// pet's aura is a real `Spell.dbc` row, and every legendary look the styles recolour is one.
    #[test]
    fn every_legendary_look_is_a_spell_of_the_player_s_client() {
        let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        for s in [starting_looks::SANCTIFIED_GROUND] {
            assert!(content.is_legendary_look(s), "style for {s}, which no legendary wears");
        }
        let Ok(Some(names)) = esurv_content::ClientNames::from_install() else {
            return;
        };
        let looks = content.legendary_looks.iter().flat_map(|l| l.moments.iter().map(|(_, s)| *s));
        for spell in looks.chain(content.pets.iter().filter_map(|p| p.aura)) {
            assert!(names.get(esurv_content::ClientName::Spell(spell)).is_some(), "look {spell} is not in the client's Spell.dbc");
        }
    }

    /// The Phoenix wears its fire from the moment it appears (`aura` in
    /// pets.ron, in its first aura slot).
    #[test]
    fn the_phoenix_appears_burning() {
        let (mut game, _client) = game_with_a_player();
        let fire = game.run.content.pets.iter().find(|p| p.name == "Phoenix").and_then(|p| p.aura).expect("the Phoenix has an aura");
        game.run.summon_pet(0, "Phoenix", Some(game.run.now + 8.0));
        let i = game.run.pets.iter().position(|p| p.pet == "Phoenix").unwrap();
        let Some((ObjectUpdate::Create { fields, .. }, _)) = game.pet_object(i, game.run.now) else { panic!("a pet create") };
        assert_eq!(fields.get(field::UNIT_AURA), Some(fire));
        assert_eq!(fields.get(field::UNIT_AURAFLAGS).map(|f| f & 0xF), Some(0x02), "its flags say it is on");
    }

    /// A legendary moment's look aimed at a spot plays from a short look
    /// put down there (its cast at that spot, not at the hero); other ground looks go as before.
    #[test]
    fn a_legendary_look_at_a_spot_plays_from_a_look_put_down_there() {
        let (mut game, _client) = game_with_a_player();
        game.show(&[]);
        let hero = Unit::Hero(game.run.heroes[0].id);
        let me = game.guid_of(hero).unwrap();
        let bomb = game.run.content.legendary_looks.iter().find(|l| l.legendary == "fire_blast").unwrap().moments.iter().find(|(m, _)| *m == esurv_content::Moment::Burst).unwrap().1;
        let go = |spell| Event::SpellGo { caster: hero, spell, hits: vec![], aim: Aim::Ground((4.0, 2.0)), ammo: false, reach: 0.0, triggered: false };
        let (told, _) = game.present(&[go(bomb)]);
        let casts: Vec<u64> = told
            .iter()
            .filter_map(|p| match parse_server(p.opcode, &p.body) {
                Ok(ServerPacket::SpellGo(go)) if go.spell_id == bomb => Some(go.caster),
                _ => None,
            })
            .collect();
        assert_eq!(casts.len(), 1, "{told:?}");
        assert_ne!(casts[0], me, "cast by the look put down, not the hero");
        assert_eq!(creates(&told[0]).len(), 1, "the look put down first");
        let (told, _) = game.present(&[go(10)]);
        assert!(told.iter().all(|p| creates(p).is_empty()), "a card's ground look is cast by the hero as before");
    }

    /// A proc's look goes as a triggered spell (the caster plays no cast for it); the hero's own
    /// cast goes plain, and animates as ever.
    #[test]
    fn a_procs_look_goes_triggered_and_a_cast_plain() {
        let (mut game, _client) = game_with_a_player();
        game.show(&[]);
        let hero = Unit::Hero(game.run.heroes[0].id);
        let go = |triggered| Event::SpellGo { caster: hero, spell: 133, hits: vec![], aim: Aim::Caster, ammo: false, reach: 0.0, triggered };
        for triggered in [false, true] {
            let (told, _) = game.present(&[go(triggered)]);
            let flags: Vec<u16> = told
                .iter()
                .filter_map(|p| match parse_server(p.opcode, &p.body) {
                    Ok(ServerPacket::SpellGo(go)) if go.spell_id == 133 => Some(go.cast_flags),
                    _ => None,
                })
                .collect();
            assert_eq!(flags.len(), 1, "{told:?}");
            assert_eq!(flags[0] & packets::CAST_FLAG_TRIGGERED != 0, triggered, "flags {:#x}", flags[0]);
        }
    }

    /// Two heroes in one run: each player object every client gets shows that hero's own class
    /// gear, in the looks chosen for it, and read as it joined: a change made after shows in the
    /// next run only.
    #[test]
    fn every_client_sees_each_hero_in_its_own_class_gear() {
        use esurv_sim::game::gear::ARMOR_SLOTS;
        use esurv_sim::game::rewards::gear_item_entry;

        let content =
            Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
        let saves = Arc::new(Saves::in_memory());
        let ana = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
        let (mage, warrior) = (
            content.class("mage").unwrap().wow_class_id,
            content.class("warrior").unwrap().wow_class_id,
        );
        let robe = gear_item_entry(mage, 2, 0);
        let staff = gear_item_entry(mage, ARMOR_SLOTS, 1);
        let boots = gear_item_entry(warrior, 5, 4); // Sunken Temple: tier 2
        let boots_look = gear_item_entry(warrior, 5, 8); // Blackrock: both its rows of looks
        for e in [robe, staff, boots, boots_look] {
            saves.add_to_collection(&content, e);
        }
        assert!(saves.wear(&content, "mage", robe) && saves.wear(&content, "mage", staff));
        assert!(saves.wear(&content, "warrior", boots));
        let wrath = content.gear_set("warrior").unwrap().look_of("brm", 1).unwrap() as u32;
        assert!(saves.set_look(&content, "warrior", 5, Some(wrath)), "the second row of Blackrock's looks");

        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(content.clone(), None, 0, &ana, saves.clone(), inputs).unwrap();
        let setups = [&ana, &bo].map(|h| hero_setup(&content, h, &saves.class_progress(&content, &h.class)));
        game.run = Run::new(
            content.clone(),
            &content.zones[0].id,
            saves.run_options().run_options(&content.run_options),
            &setups,
            Box::new(Flat),
            1,
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut clients = Vec::new();
        for (i, saved) in [&ana, &bo].into_iter().enumerate() {
            clients.push(TcpStream::connect(listener.local_addr().unwrap()).unwrap());
            let (server, _) = listener.accept().unwrap();
            assert!(game.join(Player {
                hero: game.hero_id(i),
                saved: saved.clone(),
                writer: Arc::new(Mutex::new(PacketWriter::new(server))),
            }));
        }
        // a change on the hero sheet now waits for the next run
        assert!(saves.take_off("warrior", 5));

        let shown = |i: usize, saved: &SavedHero| {
            let hero = &game.run.heroes[i];
            let SessionEvent::ObjectCreate { fields, .. } =
                creates(&update_object(&[game.hero_object(hero, saved, false)])).remove(0)
            else {
                unreachable!()
            };
            let entries: Vec<(u8, u32)> = (0..19)
                .filter_map(|s| Some((s, fields.player_visible_item_entry(s)?)))
                .collect();
            (entries, fields.unit_sheath_state())
        };
        let slot = |class: &str, s: u32| {
            let set = content.gear_set(class).unwrap();
            if s < ARMOR_SLOTS {
                content.gear.slots[s as usize].equip_slot()
            } else {
                set.weapons[(s - ARMOR_SLOTS) as usize].equip_slot()
            }
        };
        let (ana_shows, ana_sheath) = shown(0, &ana);
        assert_eq!(ana_shows.len(), 2, "{ana_shows:?}");
        assert_eq!(ana_shows[0], (slot("mage", 2), robe));
        // her weapon is her weapon loadout's (the first, the staff), in her worn staff's zone's armaments
        // (Duskwood's)
        assert_eq!(ana_shows[1].0, slot("mage", ARMOR_SLOTS));
        let staff_look = content.loadout("mage", None).unwrap().weapons[0].zone_display("duskwood", 0);
        assert_eq!(looks::item_info(&content, None, ana_shows[1].1).unwrap().display_id, staff_look);
        assert_ne!(ana_shows[1].1, staff);
        assert_eq!(
            ana_sheath,
            Some(SHEATH_STATE_PUT_AWAY),
            "a mage keeps her staff on her back"
        );
        let (bo_shows, bo_sheath) = shown(1, &bo);
        assert_eq!(
            bo_sheath,
            Some(SHEATH_STATE_MELEE),
            "a warrior enters weapons in hand"
        );
        // the boots, and the two-hander of his first weapon loadout in Elwynn's armaments
        assert_eq!(bo_shows.len(), 2, "{bo_shows:?}");
        assert_eq!(bo_shows[0].0, slot("warrior", 5));
        let blade = looks::item_info(&content, None, bo_shows[1].1).unwrap();
        assert_eq!((bo_shows[1].0, blade.display_id), (15, content.loadout("warrior", None).unwrap().weapons[0].zone_display("elwynn", 0)));
        let look = looks::item_info(&content, None, bo_shows[0].1).unwrap();
        assert_eq!(
            look.display_id,
            esurv_sim::game::gear::armor_display(content.gear_set("warrior").unwrap(), 5, wrath),
            "the Sunken Temple boots in Blackwing Lair's look"
        );
        assert_eq!(look.quality, 3, "uncommon, the tier-2 quality");

        // the items behind the character pane's slots: only the hero's own client is told
        let held = |own: bool| {
            let SessionEvent::ObjectCreate { fields, .. } =
                creates(&update_object(&[game.hero_object(
                    &game.run.heroes[0],
                    &ana,
                    own,
                )]))
                .remove(0)
            else {
                unreachable!()
            };
            (0..19)
                .filter_map(|s| Some((s, fields.player_inv_slot(s)?)))
                .collect::<Vec<_>>()
        };
        assert!(held(false).is_empty(), "other clients see no item slots");
        let mine = held(true);
        assert_eq!(mine.len(), 2);
        let items = game.worn_items(&game.run.heroes[0], &ana);
        let made: Vec<(u64, u32)> = decode(parse_server_packet(&update_object(&items)))
            .into_iter()
            .filter_map(|e| match e {
                SessionEvent::ItemCreate { guid, fields, .. } => {
                    assert_eq!(
                        field_of(&fields, field::ITEM_OWNER),
                        Some(player_guid(&ana) as u32),
                        "Ana's"
                    );
                    Some((guid, field_of(&fields, field::OBJECT_ENTRY)?))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            made,
            [(mine[0].1, robe), (mine[1].1, ana_shows[1].1)],
            "an item per worn piece, in its slot (the weapon her loadout's)"
        );
    }

    fn parse_server_packet(p: &Packet) -> benilla_protocol::ServerPacket {
        parse_server(p.opcode, &p.body).unwrap()
    }

    /// What one mirror of the jewelry tells the hero's client: the items made (guid, entry), the
    /// player's item fields set (field, guid), and the guids destroyed.
    #[allow(clippy::type_complexity)]
    fn jewelry_told(
        game: &mut Game,
        events: &[Event],
    ) -> (Vec<(u64, u32)>, Vec<(u16, u64)>, Vec<u64>) {
        let hero = game.hero_id(0);
        let (mut made, mut slots, mut gone) = (Vec::new(), Vec::new(), Vec::new());
        for (to, p) in game.mirror_jewelry(events) {
            assert_eq!(to, hero, "only the hero's own client");
            for e in decode(parse_server_packet(&p)) {
                match e {
                    SessionEvent::ItemCreate { guid, fields, .. } => {
                        made.push((guid, field_of(&fields, field::OBJECT_ENTRY).unwrap()))
                    }
                    SessionEvent::ObjectValues { fields, .. } => {
                        let raw: Vec<(u16, u32)> = fields.raw_fields().collect();
                        for pair in raw.chunks(2) {
                            let guid = u64::from(pair[0].1) | u64::from(pair[1].1) << 32;
                            slots.push((pair[0].0, guid));
                        }
                    }
                    SessionEvent::ObjectDestroyed(guid) => gone.push(guid),
                    _ => {}
                }
            }
        }
        (made, slots, gone)
    }

    /// The run's jewelry in the hero's own equipment: the amulet at the neck, rings on the
    /// fingers, trinkets in the trinket slots, the rest in the backpack; an upgrade replaces its
    /// item; a run without them takes them away.
    #[test]
    fn the_jewelry_taken_is_held_in_the_heros_own_slots() {
        use esurv_sim::game::choices::{Offer, WornAmulet, WornJewelry};
        use esurv_sim::game::offers::Rarity;

        let (mut game, _client) = game_with_a_player();
        let hero = game.hero_id(0);
        let content = game.run.content.clone();
        let rings = ["accuria", "quick_strike", "dark_iron"];
        let trinket = content
            .jewelry
            .iter()
            .find(|j| j.slot == esurv_content::JewelrySlot::Trinket)
            .unwrap()
            .id
            .clone();
        let taken: Vec<Event> = rings
            .iter()
            .map(|id| Event::OfferTaken {
                hero,
                offer: Offer::NewJewelry {
                    id: (*id).into(),
                    value: 0.1,
                    rarity: Rarity::Epic,
                },
            })
            .collect();
        let h = &mut game.run.heroes[0];
        for id in rings.iter().copied().chain([trinket.as_str()]) {
            h.jewelry.push(WornJewelry {
                id: id.into(),
                value: 0.1,
                rank: 1,
            });
        }
        h.amulet = Some(WornAmulet {
            amulet: content.amulets[0].clone(),
            rank: 1,
        });
        let (made, slots, gone) = jewelry_told(&mut game, &taken);
        assert!(gone.is_empty());
        assert_eq!(made.len(), 5);
        let at = |field: u16| {
            let guid = slots.iter().find(|(f, _)| *f == field).unwrap().1;
            made.iter().find(|(g, _)| *g == guid).unwrap().1
        };
        let slot = |s: u16| field::PLAYER_INV_SLOT_HEAD + 2 * s;
        let entry = |id: &str, rarity| {
            looks::jewelry::ring_or_trinket_entry(&content, id, rarity, 0.1).unwrap()
        };
        assert_eq!(at(slot(10)), entry("accuria", 3), "an epic ring");
        assert_eq!(at(slot(11)), entry("quick_strike", 3));
        assert_eq!(
            at(field::PLAYER_PACK_SLOT_1),
            entry("dark_iron", 3),
            "a third ring in the backpack"
        );
        assert_eq!(
            at(slot(12)),
            entry(&trinket, 0),
            "a rarity not told reads common"
        );
        assert_eq!(
            at(slot(1)),
            looks::jewelry::amulet_entry(&content, &content.amulets[0].id, 1).unwrap()
        );
        // nothing changed: nothing told
        assert_eq!(jewelry_told(&mut game, &[]), (vec![], vec![], vec![]));

        // an upgrade and an empowering: new items in the same slots, the old ones gone
        let old_ring = slots.iter().find(|(f, _)| *f == slot(10)).unwrap().1;
        let old_amulet = slots.iter().find(|(f, _)| *f == slot(1)).unwrap().1;
        let h = &mut game.run.heroes[0];
        h.jewelry[0].value = 0.2;
        h.amulet.as_mut().unwrap().rank = 2;
        let (made, slots, mut gone) = jewelry_told(&mut game, &[]);
        assert_eq!(made.len(), 2);
        assert_eq!(slots.len(), 2);
        gone.sort_unstable();
        let mut want = vec![old_ring, old_amulet];
        want.sort_unstable();
        assert_eq!(gone, want);
        assert_eq!(
            made.iter()
                .find(|(g, _)| slots.contains(&(slot(10), *g)))
                .unwrap()
                .1,
            looks::jewelry::ring_or_trinket_entry(&content, "accuria", 3, 0.2).unwrap()
        );

        // a fresh run: every item goes, every slot is emptied
        let h = &mut game.run.heroes[0];
        h.jewelry.clear();
        h.amulet = None;
        let (made, slots, gone) = jewelry_told(&mut game, &[]);
        assert!(made.is_empty());
        assert_eq!(gone.len(), 5);
        assert_eq!(slots.len(), 5);
        assert!(slots.iter().all(|(_, g)| *g == 0));
    }

    /// A speed rune makes the hero's own client run faster for the rune's time (it is forced to
    /// the new speed), then back; nothing is told while the speed stands.
    #[test]
    fn a_speed_rune_forces_the_heros_client_to_its_speed_and_back() {
        use esurv_sim::game::loot::SPEED_RUNE;
        use esurv_sim::game::stats::HERO_RUN_SPEED;

        let (mut game, _client) = game_with_a_player();
        let hero = game.hero_id(0);
        let forced = |told: Vec<(u32, Packet)>| -> Vec<f32> {
            told.iter()
                .map(|(to, p)| {
                    assert_eq!(*to, hero);
                    match parse_server_packet(p) {
                        benilla_protocol::ServerPacket::ForceSpeedChange { speed, .. } => speed,
                        _ => panic!("not a speed change"),
                    }
                })
                .collect()
        };
        assert!(forced(game.mirror_speeds()).is_empty(), "as it joined");
        game.grant(hero, &["speed_rune".into()]);
        game.run.start();
        game.run.tick(0.05);
        let base = HERO_RUN_SPEED * (game.run.heroes[0].stats.move_speed - SPEED_RUNE.0);
        let fast = forced(game.mirror_speeds());
        assert_eq!(fast.len(), 1);
        assert!((fast[0] - HERO_RUN_SPEED * game.run.heroes[0].stats.move_speed).abs() < 1e-4);
        assert!(forced(game.mirror_speeds()).is_empty(), "told once");
        // past the rune's time, back to the hero's own speed
        for _ in 0..((SPEED_RUNE.1 + 1.0) / 0.1) as usize {
            game.run.enemies.clear();
            game.run.tick(0.1);
        }
        let back = forced(game.mirror_speeds());
        assert_eq!(back.len(), 1);
        assert!((back[0] - base).abs() < 1e-3, "{} vs {base}", back[0]);
    }

    /// Every packet a client got so far, by opcode and body, read off its socket.
    fn received(client: &mut TcpStream) -> Vec<(u16, Vec<u8>)> {
        use std::io::Read;
        client
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buf = [0u8; 65536];
        while let Ok(n) = client.read(&mut buf) {
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buf[..n]);
        }
        let mut out = Vec::new();
        let mut at = 0;
        while at + 4 <= bytes.len() {
            let size = u16::from_be_bytes([bytes[at], bytes[at + 1]]) as usize;
            let opcode = u16::from_le_bytes([bytes[at + 2], bytes[at + 3]]);
            out.push((opcode, bytes[at + 4..at + 2 + size].to_vec()));
            at += 2 + size;
        }
        out
    }

    /// A buff's timer reaches the hero's client before the aura fields that name it (the order
    /// the client's buff frame keys its countdown on), with the buff's whole time.
    #[test]
    fn a_buffs_timer_comes_before_the_aura_that_names_it() {
        use benilla_protocol::messages::opcode;
        use esurv_sim::game::loot::SPEED_RUNE;

        let (mut game, mut client) = game_with_a_player();
        let hero = game.hero_id(0);
        received(&mut client);
        game.grant(hero, &["speed_rune".into()]);
        game.run.start();
        game.run.tick(0.05);
        let events = game.run.drain_events();
        game.show(&events);
        let got = received(&mut client);
        let timer = got
            .iter()
            .position(|(op, _)| *op == opcode::SMSG_UPDATE_AURA_DURATION)
            .expect("a timer");
        let (slot, ms) = (
            got[timer].1[0],
            u32::from_le_bytes(got[timer].1[1..5].try_into().unwrap()),
        );
        assert_eq!(slot, 0);
        assert!(
            (ms as f32 - SPEED_RUNE.1 * 1000.0).abs() <= 100.0,
            "{ms} ms"
        );
        let aura = got
            .iter()
            .position(|(op, body)| {
                *op == opcode::SMSG_UPDATE_OBJECT
                    && decode(parse_server(*op, body).unwrap()).iter().any(|e| matches!(e,
                        SessionEvent::ObjectValues { fields, .. } if field_of(fields, field::UNIT_AURA).is_some()))
            })
            .expect("the aura fields");
        assert!(timer < aura, "timer {timer}, aura {aura}");
    }

    /// After a pause the clients' buff timers are told again: they kept counting while the run's
    /// buffs stood still.
    #[test]
    fn a_run_going_on_after_a_pause_retells_the_buff_timers() {
        use benilla_protocol::messages::opcode;
        let (mut game, mut client) = game_with_a_player();
        let hero = game.hero_id(0);
        game.grant(hero, &["speed_rune".into()]);
        game.run.start();
        game.run.tick(0.05);
        let events = game.run.drain_events();
        game.show(&events);
        let timers = |got: &[(u16, Vec<u8>)]| {
            got.iter()
                .filter(|(op, _)| *op == opcode::SMSG_UPDATE_AURA_DURATION)
                .count()
        };
        assert_eq!(timers(&received(&mut client)), 1);
        assert!(game.run.pause());
        game.show(&[]);
        assert_eq!(timers(&received(&mut client)), 0, "nothing while it stands");
        assert!(game.run.resume());
        game.show(&[]);
        assert_eq!(
            timers(&received(&mut client)),
            1,
            "told again as it goes on"
        );
        game.show(&[]);
        assert_eq!(timers(&received(&mut client)), 0, "once");
    }

    #[test]
    fn a_cast_sets_its_bar_sweep_to_the_runs_cooldown_and_not_its_looks() {
        let (mut game, _client) = game_with_a_player();
        let me = game.players[0].guid();
        // Divine Star looks like Holy Shock, whose own cooldown is far longer than the run's
        let a = game.run.content.ability("divine_star").unwrap().clone();
        game.run.heroes[0].spells.learn(&a, &[]);
        let slot = game.run.heroes[0].spells.known.iter().position(|s| s.spell == a.id).unwrap();
        game.run.heroes[0].spells.known[slot].timer = 3.0;
        let cast = packets::spell_go(me, a.look_spell, &[], packets::SpellTarget::Caster, None);
        let told: Vec<ServerPacket> = game
            .bar_sweeps_after_casts(std::slice::from_ref(&cast))
            .iter()
            .map(|(_, p)| parse_server_packet(p))
            .collect();
        assert!(matches!(
            &told[..],
            [ServerPacket::ClearCooldown { spell_id, caster }, ServerPacket::SpellCooldownList { caster: c, cooldowns }]
                if *spell_id == a.look_spell && *caster == me && *c == me && cooldowns[..] == [(a.look_spell, 3000)]
        ));
        // ready again: the look's own cooldown is cleared and no sweep is set
        game.run.heroes[0].spells.known[slot].timer = 0.0;
        let told = game.bar_sweeps_after_casts(std::slice::from_ref(&cast));
        assert!(matches!(&told[..], [(_, p)] if matches!(parse_server_packet(p), ServerPacket::ClearCooldown { .. })));
        // another unit's cast is no hero's
        let theirs = packets::spell_go(me + 1, a.look_spell, &[], packets::SpellTarget::Caster, None);
        assert!(game.bar_sweeps_after_casts(&[theirs]).is_empty());
    }

    /// Roadmap R3: the hunter's weapon loadout puts its bow, crossbow or
    /// gun in the ranged slot, and Auto Shot shoots as that weapon; the loadout chosen reaches
    /// the run's hero.
    #[test]
    fn the_hunters_loadout_sets_its_ranged_weapon_and_its_shot() {
        let (mut game, _client) = game_with_a_hero_of("hunter", "orc");
        let hero = game.players[0].hero;
        let content = game.run.content.clone();
        let saved = game.players[0].saved.clone();
        let auto_shot = esurv_sim::game::kit::swing_of(&content, &game.run.heroes[0])
            .and_then(|s| s.look_spell)
            .unwrap();
        // the first loadout, the bow, as it joined
        assert_eq!(game.run.heroes[0].loadout, "bow");
        assert_eq!(game.shot_look(hero, auto_shot), Some(ranged_shots::BOW_SHOT_LOOK));
        for (loadout, look) in [("gun", ranged_shots::RIFLE_SHOT_LOOK), ("crossbow", ranged_shots::RIFLE_SHOT_LOOK), ("bow", ranged_shots::BOW_SHOT_LOOK)] {
            assert!(game.saves.choose_loadout(&content, "hunter", loadout));
            let progress = game.saves.class_progress(&content, "hunter");
            assert_eq!(hero_setup(&content, &saved, &progress).loadout.as_deref(), Some(loadout));
            let dressed = game.dress(&saved);
            let ranged = content.loadout("hunter", Some(loadout)).unwrap().weapon(esurv_content::Hand::Ranged).unwrap();
            let info = looks::item_info(&content, None, dressed.shown[17]).unwrap();
            assert_eq!((info.subclass, info.display_id), (ranged.kind.subclass(), ranged.zone_display("elwynn", 0)), "{loadout}");
            assert_eq!(dressed.shown[15], 0, "no outfit weapon beside it");
            game.dressed.insert(hero, dressed);
            assert_eq!(game.shot_look(hero, auto_shot), Some(look), "{loadout}");
        }
    }

    #[test]
    fn a_hunter_shoots_as_the_ranged_weapon_it_holds() {
        use esurv_sim::game::gear::ARMOR_SLOTS;
        use esurv_sim::game::rewards::gear_item_entry;

        let (mut game, _client) = game_with_a_hero_of("hunter", "orc");
        let hero = game.players[0].hero;
        let content = game.run.content.clone();
        let auto_shot = esurv_sim::game::kit::swing_of(&content, &game.run.heroes[0])
            .and_then(|s| s.look_spell)
            .expect("the hunter's swing is a shot");
        // nothing in the ranged slot: no shooting look
        game.dressed.insert(hero, Dressed { shown: [0; looks::EQUIPMENT_SLOTS], worn: Vec::new() });
        assert_eq!(game.shot_look(hero, auto_shot), None);
        // its swing's hits log as Auto Shot, not as a melee swing
        assert_eq!(game.ranged_swing_look(hero), Some(auto_shot));
        // the hunter's class bow
        let mut shown = [0; looks::EQUIPMENT_SLOTS];
        shown[17] = gear_item_entry(content.class("hunter").unwrap().wow_class_id, ARMOR_SLOTS, 0);
        game.dressed.insert(hero, Dressed { shown, worn: Vec::new() });
        assert_eq!(game.shot_look(hero, auto_shot), Some(ranged_shots::BOW_SHOT_LOOK));
        // Explosive Shot, a ranged spell whose look is no shot, once known
        let shot = content.ability("explosive_shot").unwrap().clone();
        assert_eq!(game.shot_look(hero, shot.look_spell), None, "not known yet");
        game.run.heroes[0].spells.learn(&shot, &[]);
        assert_eq!(game.shot_look(hero, shot.look_spell), Some(ranged_shots::BOW_SHOT_LOOK));
    }

    /// A class's prestige ranks come into its heroes' runs, as its talents do.
    #[test]
    fn prestige_ranks_come_into_a_run() {
        use esurv_content::Stat;
        let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let saves = Saves::in_memory();
        let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
        let progress = ClassProgress { prestige: [("damage".to_owned(), 4), ("block_chance".to_owned(), 2)].into(), ..Default::default() };
        let setup = hero_setup(&content, &bo, &progress);
        let sum = |stat: Stat| setup.modifiers.iter().filter(|m| m.stat == stat).map(|m| m.value).sum::<f32>();
        assert!((sum(Stat::Damage) - 0.04).abs() < 1e-6);
        assert!((sum(Stat::BlockChance) - 0.02).abs() < 1e-6);
    }

    /// A creature enemy holds what vmangos's creature of its name holds
    /// (docs/game/enemy-weapons.md): the server sends it as its virtual items (look, item info,
    /// sheath), drawn; a rare holds its elite's; a dual wielder swings its hands in turn.
    #[test]
    fn an_armed_enemy_is_sent_its_weapons_as_virtual_items() {
        let content = Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
        let zone = content.zones.iter().position(|z| z.id == "scarlet").unwrap();
        let saves = Arc::new(Saves::in_memory());
        let ana = saves.create("Ana", "warrior", "human", 1, [0; 5]).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(content.clone(), None, zone, &ana, saves, inputs).unwrap();
        let h = game.run.heroes[0].id;
        let p = game.run.heroes[0].pos;
        game.run.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        for _ in 0..400 {
            if !game.run.enemies.is_empty() {
                break;
            }
            game.run.tick(0.05);
        }
        assert!(!game.run.enemies.is_empty(), "an enemy came");
        let roster = &content.zones[zone].enemies;
        let at = |name: &str| roster.iter().position(|e| e.name == name).unwrap();
        // a Scarlet Soldier: a long sword and a kite shield
        game.run.enemies[0].kind = EnemyKind::Roster(at("Scarlet Soldier"));
        let fields = |game: &Game| match game.enemy_object(0, game.run.now).0 {
            ObjectUpdate::Create { fields, .. } => fields,
            _ => unreachable!(),
        };
        let f = fields(&game);
        assert_eq!(f.get(field::UNIT_VIRTUAL_ITEM_SLOT_DISPLAY), Some(7483));
        assert_eq!(f.get(field::UNIT_VIRTUAL_ITEM_SLOT_DISPLAY + 1), Some(1705));
        assert_eq!(f.get(field::UNIT_VIRTUAL_ITEM_SLOT_DISPLAY + 2), None, "no ranged weapon");
        // class 2 subclass 7 (a one-handed sword), material 1, inventory type 13; on the hip
        assert_eq!(f.get(field::UNIT_VIRTUAL_ITEM_INFO), Some(2 | 7 << 8 | 1 << 16 | 13 << 24));
        assert_eq!(f.get(field::UNIT_VIRTUAL_ITEM_INFO + 1), Some(3));
        // class 4 subclass 6 (a shield), inventory type 14; on the back
        assert_eq!(f.get(field::UNIT_VIRTUAL_ITEM_INFO + 2), Some(4 | 6 << 8 | 1 << 16 | 14 << 24));
        assert_eq!(f.get(field::UNIT_VIRTUAL_ITEM_INFO + 3), Some(4));
        assert_eq!(f.get(field::UNIT_BYTES_2).map(|b| b & 0xff), Some(u32::from(SHEATH_STATE_MELEE)));
        // the client reads them back as it reads a creature's
        let p = crate::update::update_object(&[game.enemy_object(0, game.run.now).0]);
        let Some(SessionEvent::ObjectCreate { fields: parsed, .. }) =
            decode(parse_server(p.opcode, &p.body).unwrap()).into_iter().next()
        else {
            panic!("an object create")
        };
        assert_eq!(parsed.unit_virtual_item_display(0), Some(7483));
        assert_eq!(parsed.unit_virtual_item_info(0), Some((2, 7, 1, 13)));
        assert_eq!(parsed.unit_virtual_item_sheath(1), Some(4));
        // one sword and shield: never an off-hand swing
        let id = game.run.enemies[0].id;
        assert!(!(0..4).any(|_| game.enemy_swings_left(id)));
        // a Scarlet Guardsman's rare holds the elite's glaive
        game.run.enemies[0].kind = EnemyKind::Rare { roster: at("Scarlet Guardsman") };
        assert_eq!(fields(&game).get(field::UNIT_VIRTUAL_ITEM_SLOT_DISPLAY), Some(7437));
        // a Scarlet Myrmidon holds a sword in each hand and swings them in turn
        game.run.enemies[0].kind = EnemyKind::Roster(at("Scarlet Myrmidon"));
        assert_eq!(fields(&game).get(field::UNIT_VIRTUAL_ITEM_SLOT_DISPLAY + 1), Some(7488));
        let hands: Vec<bool> = (0..4).map(|_| game.enemy_swings_left(id)).collect();
        assert_eq!(hands, [false, true, false, true]);
        // a Scarlet Tracking Hound holds nothing, and is sent no virtual items
        game.run.enemies[0].kind = EnemyKind::Roster(at("Scarlet Tracking Hound"));
        let f = fields(&game);
        assert_eq!((f.get(field::UNIT_VIRTUAL_ITEM_SLOT_DISPLAY), f.get(field::UNIT_BYTES_2)), (None, None));
    }
}
