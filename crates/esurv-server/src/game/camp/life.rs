//! The heroes' life at their tents between runs (plans/hub-hero-life.md): each
//! settles now and then into a pose of its temperaments, gestures, and says a line of
//! `camp_chatter.ron` when the player can see it; two or three at a time walk off to the fire,
//! the anvil, the weapon rack, the lectern or a training dummy a while (walk.rs). Paced so the camp never chatters over the player:
//! one voice at a time, about a line a minute, none twice in a sitting, none while a window is
//! open or Wren is talking. What has just happened to the player's hero (a run won or lost, a zone won
//! for the first time, the hero just made) is said first, once. Flavour only: nothing here
//! changes a run.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use esurv_content::{CampChatter, CampLine, Exchange, Gesture, LineWhen, Pose, Speaker};
use esurv_protocol::ToClient;
use rand::seq::SliceRandom;
use rand::Rng;

use super::tour::Leg;
use super::walk::{floor_near, Doing, Group, Haunt, Walls};
use crate::wmo_floor::WmoFloors;
use super::{CampState, REACH};
use crate::game::{dist, player_guid, Game};
use crate::game::bunnies::{still_unit_create, BUNNY_DISPLAY};
use crate::packets::{self, SpellTarget, EMOTE_TALK};
use benilla_protocol::messages::opcode;
use crate::save::{CampNews, SavedHero};
use crate::update::{field, Fields, ObjectUpdate, Speeds};
use crate::wire::Packet;

// The pacing: the advisor's (plans/hub-hero-life.md, section 7), flavour.
/// About a spoken line a minute camp-wide, seconds.
const LINE_EVERY: f32 = 60.0;
/// A hero speaks at most once every few minutes, seconds.
const HERO_LINE_EVERY: f32 = 180.0;
/// A hero's pose changes every half minute or so, seconds.
const POSE_EVERY: f32 = 30.0;
/// A few gestures a minute across the camp, seconds apart.
const GESTURE_EVERY: f32 = 20.0;
/// Nothing said in the first seconds after the camp is made, so the player's return is not
/// shouted over, seconds.
const QUIET_AT_FIRST: f32 = 10.0;
/// After the player talks to someone, a while before the camp speaks again, seconds.
const QUIET_AFTER_TALK: f32 = 20.0;
/// A hero this near the player can be seen and heard, yards: the client shows a speech bubble
/// only within 20 yd of the player (the 1.12 client's own reach), counted with the height between
/// them, which this leaves out; so a little nearer.
const IN_SIGHT: f32 = 18.0;
/// No hero near enough to speak: the camp tries again this soon, seconds.
const NOBODY_NEAR_RETRY: f32 = 5.0;
/// A hero this near the player speaks to it, not to nobody, yards.
const TO_THE_PLAYER: f32 = REACH * 1.5;
/// A hero draws about two lines in three from its leading temperament (the plan, section 2).
const LEADING_SHARE: f64 = 2.0 / 3.0;
/// A zone won for the first time: the camp's cheers go round the ring a hero this far apart,
/// seconds.
const RIPPLE_EVERY: u64 = 1;
/// Two heroes talk to each other about once every few minutes, seconds (plan, section 7).
const EXCHANGE_EVERY: f32 = 180.0;
/// An exchange's lines come a few seconds apart, seconds.
const EXCHANGE_GAP: f32 = 4.0;
/// Back from a loss, about one time in two two heroes talk of it rather than one saying a line.
const LOSS_EXCHANGE_SHARE: f64 = 0.5;
/// About one held pose in two is standing at ease, the stock idle.
const AT_EASE_SHARE: f64 = 0.5;
/// Two or three heroes away from their tents at a time.
const AWAY_AT_MOST: usize = 3;
/// A hero sets off about this often, seconds.
const OUTING_EVERY: f32 = 45.0;
/// It stays with what it went to do five to seven minutes, seconds.
const STAY: (f32, f32) = (300.0, 420.0);
/// Now and then it moves on to another place of the same kind, about this often, seconds (the
/// fire's log to its edge, the anvil to the rack, the lectern to the Orb): one time in two.
const SHIFT_EVERY: f32 = 120.0;
/// At a training dummy, a swing about this often, seconds.
const SWING_EVERY: f32 = 2.5;
/// About one swing in four a fighter's other move: a rogue's kick, a shield's parry.
const OTHER_MOVE_SHARE: f64 = 0.25;
/// A caster's cast comes this long after its wind-up, seconds.
const CAST_AFTER: f32 = 1.5;
/// Two heroes talking gesture about this often, by turns, seconds.
const TALK_EVERY: f32 = 4.0;
/// How far apart two heroes talking stand, yards.
const TALK_APART: f32 = 1.6;
/// Where two heroes stop to talk: this far from the fire, yards, out of the fire's ring and short
/// of the tents.
const TALK_FROM_FIRE: (f32, f32) = (9.0, 20.0);
/// Tries at finding such a place, and how far it keeps from the places the heroes go, yards.
const TALK_TRIES: usize = 20;
const TALK_CLEAR: f32 = 2.0;
/// A sitting rogue or warrior has its blade out, cleaning it, about one time in two.
const BLADE_SHARE: f64 = 0.5;

// `Emotes.dbc` one-shots and states the advisor chose (the 1.12 ids).
const EMOTE_EXCLAMATION: u32 = 5;
const EMOTE_QUESTION: u32 = 6;
const EMOTE_EAT: u32 = 7;
const EMOTE_CHEER: u32 = 4;
const EMOTE_LAUGH: u32 = 11;
const EMOTE_POINT: u32 = 25;
const EMOTE_ATTACK_1H: u32 = 36;
const EMOTE_ATTACK_2H: u32 = 37;
const EMOTE_PARRY_SHIELD: u32 = 43;
const EMOTE_READY_1H: u32 = 45;
const EMOTE_READY_BOW: u32 = 48;
const EMOTE_SPELL_PRECAST: u32 = 50;
const EMOTE_SPELL_CAST: u32 = 51;
const EMOTE_KICK: u32 = 60;
const EMOTE_SALUTE: u32 = 66;
const EMOTE_YES: u32 = 273;
const EMOTE_NO: u32 = 274;
/// Hands busy before it, the state for handling, reading, looking into.
const STATE_USE_STANDING: u32 = 69;
/// `UNIT_FIELD_BYTES_2`'s first byte: weapons put away, the melee ones drawn, the ranged one.
const SHEATHED: u8 = 0;
const DRAWN: u8 = 1;
const BOW_DRAWN: u8 = 2;

/// `UNIT_STAND_STATE_*`, the first byte of `UNIT_FIELD_BYTES_1`.
const STAND: u8 = 0;
const STAND_SIT: u8 = 1;
const STAND_SLEEP: u8 = 3;
const STAND_KNEEL: u8 = 8;
/// `Emotes.dbc` states, played as the idle (`UNIT_NPC_EMOTESTATE`).
const STATE_DANCE: u32 = 10;
const STATE_READY_UNARMED: u32 = 27;
const STATE_WORK: u32 = 28;

/// The camp's life: each hero's pose and when it last spoke, and the camp's own pacing.
pub(in crate::game) struct Life {
    /// By visitor, as `CampState::visitors`.
    heroes: Vec<HeroLife>,
    next_line: Instant,
    next_gesture: Instant,
    /// The lines said this sitting, by their place in `camp_chatter.ron`.
    said: HashSet<usize>,
    /// A window of the game's own is open (`ToClient::CampOpen` till `CampClosed`).
    pub(super) window_open: bool,
    quiet_until: Instant,
    /// What has just happened to the player's hero, for the camp to react to before any idle line;
    /// gone once it has.
    news: Option<CampNews>,
    next_exchange: Instant,
    /// The exchanges had this sitting, by their place in `camp_chatter.ron`.
    exchanged: HashSet<usize>,
    /// Each hero's place at its tent and the way it faces there, by visitor.
    stands: Vec<Option<([f32; 3], f32)>>,
    /// The places the heroes walk to, and the ground they walk on.
    haunts: Vec<Haunt>,
    walls: Walls,
    next_outing: Instant,
    /// How often a hero sets off, how long it stays and how often it may move on, seconds
    /// (`ESURV_CAMP_WALKS=now`, a development aid, sends them off at once and back soon, to see
    /// them walk).
    outing_every: f32,
    stay: (f32, f32),
    shift_every: f32,
    /// What each hero holds in hand, by visitor.
    kits: Vec<Kit>,
    /// The camp is in good spirits, after a run won or most of the last few: the merry ones toast
    /// at the fire.
    merry: bool,
    /// The camp is low, most of the last few runs lost: nobody dances or cheers at the tents.
    gloomy: bool,
    /// The Camp chatter option is Off: the heroes say nothing, though they still gesture as they
    /// talk (the player's client sets it, `campchatter`).
    pub(super) mute: bool,
    /// The hour the camp was set to as it was made, and when: the client's sky moves on from it at
    /// the world's pace (`packets::TIME_SPEED`, a minute a minute), and the camp's day with it.
    hour: u32,
    made: Instant,
    /// The fire burning low at night, and fed.
    fire: Fire,
    /// The heroes' ways over the camp's own ground.
    ways: HeroWays,
}

/// The equipment slots a hero's weapons are drawn in, and the inventory types that make a kit.
pub(super) const MAIN_HAND: u8 = 15;
pub(super) const OFF_HAND: u8 = 16;
pub(super) const INVTYPE_2HWEAPON: u32 = 17;
pub(super) const INVTYPE_SHIELD: u32 = 14;

/// What a hero holds: a two-handed weapon, a shield.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Kit {
    pub two_hand: bool,
    pub shield: bool,
}

struct HeroLife {
    pose: Pose,
    next_pose: Instant,
    last_said: Option<Instant>,
    /// Away from its tent: where to, and how far along.
    out: Option<Outing>,
}

/// A hero's time away from its tent: at `haunt` (its place in `Life::haunts`, `None` for a spot
/// where it talks with `with`), reached along `leg`, till `until`, moving now and then to another
/// place of the same group; then home along a leg of its own (`home`).
struct Outing {
    haunt: Haunt,
    slot: Option<usize>,
    leg: Leg,
    arrived: bool,
    until: Instant,
    next_shift: Instant,
    home: bool,
    next_beat: Instant,
    /// The hero it talks with, by visitor.
    with: Option<usize>,
    way: Way,
    /// A weary one at the fire, dozed off.
    dozing: bool,
    /// Gone to feed the fire burning low.
    feeds: bool,
}

/// How a hero does things, set as it sets off.
#[derive(Clone, Copy, Debug)]
struct Way {
    drill: Drill,
    /// Sitting, its blade out.
    blade: bool,
    /// Sitting, eats now and then.
    eats: bool,
    /// Sitting, cheers now and then (after a win).
    toasts: bool,
    /// Sitting, dozes off.
    dozes: bool,
}

/// How a hero drills at a training dummy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drill {
    Swing { kit: Kit, kicks: bool },
    Cast,
    Bow,
}

/// What a hero holds while it does a thing: its stand state, its state emote, its weapons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Hold {
    stand: u8,
    state: u32,
    sheath: u8,
}

/// The camp's time of day: by day all goes on; from dusk the anvil and the training dummies rest; at night nobody
/// talks apart, one keeps watch at the camp's edge, those at the fire doze off on the logs (two at
/// most) and about half the lines said are the night's; at dawn the sleepers wake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TimeOfDay {
    Dawn,
    Day,
    Dusk,
    Night,
}

impl TimeOfDay {
    /// The time of day at `hour` (0 to 24): night from 21:00 to 5:00, dawn till 7:00, dusk from
    /// 19:00.
    fn at(hour: f32) -> Self {
        match hour {
            h if !(5.0..21.0).contains(&h) => TimeOfDay::Night,
            h if h < 7.0 => TimeOfDay::Dawn,
            h if h >= 19.0 => TimeOfDay::Dusk,
            _ => TimeOfDay::Day,
        }
    }

    /// Whether the heroes go to `group` now.
    fn goes_to(self, group: Group) -> bool {
        match self {
            TimeOfDay::Night => !matches!(group, Group::Armoury | Group::Training | Group::Talk),
            TimeOfDay::Dusk => !matches!(group, Group::Armoury | Group::Training),
            TimeOfDay::Dawn | TimeOfDay::Day => true,
        }
    }
}

/// The camp's mood from the last few runs come back from (the advisor's rule, flavour, never named
/// or shown): low when three or more of the last five were lost, or the last three; in good
/// spirits when three or more of them were won; else even.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mood {
    Gloomy,
    Even,
    Bright,
}

/// Of the last five, this many lost makes the camp low, this many won lifts it.
const MOOD_TURNS_AT: usize = 3;

/// The mood after runs `recent` (won or not, the latest last).
fn mood(recent: &[bool]) -> Mood {
    let last = &recent[recent.len().saturating_sub(crate::save::RECENT_RUNS)..];
    let lost = last.iter().filter(|w| !**w).count();
    let lost_in_a_row = last.len() >= MOOD_TURNS_AT && last[last.len() - MOOD_TURNS_AT..].iter().all(|w| !*w);
    if lost >= MOOD_TURNS_AT || lost_in_a_row {
        Mood::Gloomy
    } else if last.len() - lost >= MOOD_TURNS_AT {
        Mood::Bright
    } else {
        Mood::Even
    }
}

/// What nobody does at the tents while the camp is low.
fn too_merry_for_a_low_camp(g: Gesture) -> bool {
    matches!(g, Gesture::Cheer | Gesture::Applaud | Gesture::Laugh | Gesture::Flex | Gesture::Chicken)
}

/// At night, about this share of the lines said to nobody are the night's.
const NIGHT_LINE_SHARE: f64 = 0.5;
/// At night, at most this many doze off at the fire (the weary by day too).
const ASLEEP_AT_MOST: usize = 2;
/// A sleeper on a log turns a quarter from the fire, to lie along the log.
const ALONG_THE_LOG: f32 = std::f32::consts::FRAC_PI_2;

// The fire burning low: the advisor's pacing, flavour.
/// Once a night each visit the fire burns low, about this long into the night, seconds.
const FIRE_LOWS_AFTER: f32 = 120.0;
/// About this long after, someone goes to feed it ("within about a minute"), seconds.
const FEED_AFTER: f32 = 30.0;
/// It is at the fire this long, feeding it, before the flames come back, seconds.
const FEED_TAKES: f32 = 6.0;
/// What it does there: hands busy before it.
const FEEDING: Hold = Hold { stand: STAND, state: STATE_USE_STANDING, sheath: SHEATHED };
/// The chat's line as it does, its name for `%s`.
const FEEDS_THE_FIRE: &str = "%s feeds the fire.";

/// A caster of fire or holy light lights it again with a spell of its own, cast into an invisible
/// unit in the flames as a boss's look is: the spell, and whether it is cast at
/// the ground (Flamestrike) rather than at the unit. The ids the game's own abilities show.
fn kindling(class: &str) -> Option<(u32, bool)> {
    match class {
        "warlock" => Some((348, false)),  // Immolate
        "mage" => Some((2120, true)),     // Flamestrike
        "priest" => Some((14914, false)), // Holy Fire
        "paladin" => Some((25912, false)), // Holy Shock (its look, as the ability's)
        _ => None,
    }
}
/// The chat's line as a caster lights it.
const KINDLES_THE_FIRE: &str = "%s rekindles the fire.";
/// The invisible unit stays a while after the spell lands, so its impact plays out, seconds.
const KINDLING_LINGERS: f32 = 3.0;

/// The fire's night (the advisor's, flavour): once a visit, at night, it burns low (only its
/// flames and sparks dimmed, the model left as it is), and a hero walks to its
/// edge and feeds it back.
#[derive(Default)]
struct Fire {
    lows_at: Option<Instant>,
    /// It has burned low this visit.
    burned: bool,
    low: bool,
    /// Someone sets off to feed it from then; a hero is on its way.
    feed_from: Option<Instant>,
    feeding: bool,
    /// The client is to be told the fire is low (true) or fed (false).
    tell: Option<bool>,
    /// How long into the night it burns low, and how long till it is fed, seconds (quick with
    /// `ESURV_CAMP_WALKS=now`).
    pace: (f32, f32),
}

/// `ESURV_CAMP_WALKS=now`: the heroes set off at once and come back soon.
fn walks_now() -> bool {
    std::env::var("ESURV_CAMP_WALKS").is_ok_and(|v| v == "now")
}

/// `secs` give or take half.
fn about(secs: f32) -> Duration {
    Duration::from_secs_f32(secs * rand::thread_rng().gen_range(0.5..1.5))
}

impl Life {
    pub(super) fn new(stands: Vec<Option<([f32; 3], f32)>>, news: Option<CampNews>, recent: &[bool], hour: u32, now: Instant) -> Self {
        Life {
            heroes: (0..stands.len()).map(|_| HeroLife { pose: Pose::Stand, next_pose: now + about(POSE_EVERY), last_said: None, out: None }).collect(),
            next_line: now + Duration::from_secs_f32(QUIET_AT_FIRST),
            next_gesture: now + about(GESTURE_EVERY),
            said: HashSet::new(),
            window_open: false,
            quiet_until: now + Duration::from_secs_f32(QUIET_AT_FIRST),
            news,
            // the first thing said after the camp's first quiet may be two heroes talking
            next_exchange: now,
            exchanged: HashSet::new(),
            stands,
            haunts: Vec::new(),
            walls: Walls::open(),
            next_outing: now + Duration::from_secs_f32(QUIET_AT_FIRST),
            outing_every: if walks_now() { 1.0 } else { OUTING_EVERY },
            stay: if walks_now() { (15.0, 20.0) } else { STAY },
            shift_every: if walks_now() { 6.0 } else { SHIFT_EVERY },
            kits: Vec::new(),
            merry: matches!(news, Some(CampNews::Won { .. })) || mood(recent) == Mood::Bright,
            gloomy: mood(recent) == Mood::Gloomy,
            mute: false,
            hour,
            made: now,
            fire: Fire { pace: if walks_now() { (10.0, 10.0) } else { (FIRE_LOWS_AFTER, FEED_AFTER) }, ..Fire::default() },
            ways: HeroWays::default(),
        }
    }

    /// The places the heroes walk to and the ground they walk on, once the camp stands, and what
    /// each holds.
    pub(super) fn set_ways(&mut self, haunts: Vec<Haunt>, walls: Walls, kits: Vec<Kit>) {
        self.haunts = haunts;
        self.walls = walls;
        self.kits = kits;
        self.ways = HeroWays::default();
    }

    /// The places and the ground again, a piece set out or taken away: each hero out keeps its
    /// place if it is still there (one at a dummy taken away finishes its while where it stands).
    pub(super) fn change_ways(&mut self, haunts: Vec<Haunt>, walls: Walls) {
        for o in self.heroes.iter_mut().filter_map(|h| h.out.as_mut()) {
            o.slot = o.slot.and_then(|_| haunts.iter().position(|h| h.at == o.haunt.at && h.doing == o.haunt.doing));
        }
        self.haunts = haunts;
        self.walls = walls;
        self.ways = HeroWays::default();
    }

    /// The camp's time of day at `now`.
    fn time_of_day(&self, now: Instant) -> TimeOfDay {
        let hour = (self.hour as f32 + now.duration_since(self.made).as_secs_f32() / 3600.0) % 24.0;
        TimeOfDay::at(hour)
    }

    /// Whether hero `i` has dozed off by the fire (it says nothing).
    fn dozing(&self, i: usize) -> bool {
        self.heroes.get(i).and_then(|l| l.out.as_ref()).is_some_and(|o| o.dozing)
    }

    /// Hero `guid` (named `name`) saying `text`, unless the heroes are muted.
    fn line(&self, guid: u64, name: &str, text: &str) -> Option<Packet> {
        (!self.mute).then(|| packets::monster_say(guid, name, text))
    }

    /// The pair talking face to face, if hero `i` is one of them and both are there.
    fn talking_with(&self, i: usize) -> Option<usize> {
        let o = self.heroes.get(i)?.out.as_ref()?;
        let j = o.with?;
        let p = self.heroes.get(j)?.out.as_ref()?;
        (o.arrived && !o.home && p.arrived && !p.home && p.with == Some(i)).then_some(j)
    }

    /// Where hero `i` (by visitor) is at `now`: at its tent, or out walking or where it went.
    pub(super) fn hero_at(&self, i: usize, now: Instant) -> Option<(f32, f32)> {
        match self.heroes.get(i).and_then(|l| l.out.as_ref()) {
            Some(o) => Some(o.leg.at(now)),
            None => self.stands.get(i).copied().flatten().map(|(p, _)| (p[0], p[1])),
        }
    }

    /// Whether hero `i` is at its tent.
    fn at_home(&self, i: usize) -> bool {
        self.heroes.get(i).is_some_and(|l| l.out.is_none())
    }

    /// The player talked to someone: the camp holds its tongue a while.
    pub(super) fn talked(&mut self, now: Instant) {
        self.quiet_until = self.quiet_until.max(now + Duration::from_secs_f32(QUIET_AFTER_TALK));
    }
}

/// A hero's temperaments: its leading one, or (about one time in three) its second.
fn temperament_now(saved: &SavedHero) -> Option<&str> {
    let t = saved.temperament.as_deref()?;
    let second = t.len() > 1 && !rand::thread_rng().gen_bool(LEADING_SHARE);
    t.get(usize::from(second)).map(String::as_str)
}

/// Whether line `l` fits hero `saved` in its temperament `temperament` at moment `when`: its race,
/// class and the time period it was pulled in from, where the line names them.
fn fits(l: &CampLine, saved: &SavedHero, temperament: &str, when: LineWhen) -> bool {
    l.temperament == temperament
        && l.when == when
        && l.race.as_ref().is_none_or(|r| *r == saved.race)
        && l.class.as_ref().is_none_or(|c| *c == saved.class)
        && l.time_period.is_none_or(|p| p == saved.time_period)
}

/// What a pose is on the wire: the stand state and the state emote.
fn pose_fields(pose: Pose) -> (u8, u32) {
    match pose {
        Pose::Stand => (STAND, 0),
        Pose::Sit => (STAND_SIT, 0),
        Pose::Kneel => (STAND_KNEEL, 0),
        Pose::Sleep => (STAND_SLEEP, 0),
        Pose::Work => (STAND, STATE_WORK),
        Pose::Ready => (STAND, STATE_READY_UNARMED),
        Pose::Dance => (STAND, STATE_DANCE),
    }
}

/// What a hero holds while it does `doing` its way (the advisor's): weapons out only to handle
/// the rack's, to drill, or sitting to clean a blade.
fn doing_hold(doing: Doing, way: &Way, dozing: bool) -> Hold {
    let hold = |stand, state, sheath| Hold { stand, state, sheath };
    match doing {
        Doing::Sit if dozing => hold(STAND_SLEEP, 0, SHEATHED),
        Doing::Sit => hold(STAND_SIT, 0, if way.blade { DRAWN } else { SHEATHED }),
        Doing::Pray => hold(STAND_KNEEL, 0, SHEATHED),
        Doing::Hammer => hold(STAND, STATE_WORK, SHEATHED),
        Doing::LookOver => hold(STAND, STATE_USE_STANDING, DRAWN),
        Doing::Inspect | Doing::Read | Doing::Peer => hold(STAND, STATE_USE_STANDING, SHEATHED),
        Doing::Spar => match way.drill {
            Drill::Swing { .. } => hold(STAND, 0, DRAWN),
            Drill::Bow => hold(STAND, 0, BOW_DRAWN),
            Drill::Cast => hold(STAND, 0, SHEATHED),
        },
        Doing::Watch | Doing::Talk => hold(STAND, 0, SHEATHED),
    }
}

/// The update that puts hero `guid` in `hold`.
fn hold_update(guid: u64, hold: Hold) -> ObjectUpdate {
    let mut fields = Fields::new();
    fields
        .bytes(field::UNIT_BYTES_1, [hold.stand, 0, 0, 0])
        .u32(field::UNIT_NPC_EMOTESTATE, hold.state)
        .bytes(field::UNIT_BYTES_2, [hold.sheath, 0, 0, 0]);
    ObjectUpdate::Values { guid, fields }
}

/// The update that puts hero `guid` in `pose`, its weapons put away.
fn pose_update(guid: u64, pose: Pose) -> ObjectUpdate {
    let (stand, state) = pose_fields(pose);
    hold_update(guid, Hold { stand, state, sheath: SHEATHED })
}

/// What a hero doing `doing` its way does now and then, a one-shot `Emotes.dbc` id; `None` for
/// nothing (a weary one at the fire dozes off instead: the caller's).
fn beat(doing: Doing, way: &Way) -> Option<u32> {
    let mut rng = rand::thread_rng();
    match doing {
        Doing::Sit => {
            let mut can = Vec::new();
            if way.eats {
                can.push(EMOTE_EAT);
            }
            if way.toasts {
                can.push(EMOTE_CHEER);
            }
            can.choose(&mut rng).copied()
        }
        Doing::LookOver => Some(EMOTE_READY_1H),
        Doing::Inspect => Some(EMOTE_SALUTE),
        Doing::Read => [EMOTE_QUESTION, EMOTE_EXCLAMATION].choose(&mut rng).copied(),
        Doing::Watch => Some(EMOTE_POINT),
        Doing::Talk => [EMOTE_TALK, EMOTE_TALK, EMOTE_TALK, EMOTE_QUESTION, EMOTE_EXCLAMATION, EMOTE_LAUGH, EMOTE_YES, EMOTE_NO, EMOTE_POINT].choose(&mut rng).copied(),
        Doing::Pray | Doing::Hammer | Doing::Peer | Doing::Spar => None,
    }
}

/// A drill's next move at a training dummy: a one-shot now, and one a little after (a cast after
/// its wind-up).
fn drill_move(drill: Drill) -> (u32, Option<u32>) {
    let other = rand::thread_rng().gen_bool(OTHER_MOVE_SHARE);
    match drill {
        Drill::Swing { kicks: true, .. } if other => (EMOTE_KICK, None),
        Drill::Swing { kit, .. } if other && kit.shield => (EMOTE_PARRY_SHIELD, None),
        Drill::Swing { kit, .. } => (if kit.two_hand { EMOTE_ATTACK_2H } else { EMOTE_ATTACK_1H }, None),
        Drill::Bow => (EMOTE_READY_BOW, None),
        Drill::Cast => (EMOTE_SPELL_PRECAST, Some(EMOTE_SPELL_CAST)),
    }
}

/// How hero `saved`, holding `kit`, does things this time out (the advisor's): a shaman drills
/// with its weapon or its spells; a rogue or warrior at the fire may clean its blade; the merry,
/// the weary and dwarves eat there, the merry toast a win, the weary doze.
fn way_of(saved: &SavedHero, kit: Kit, merry: bool) -> Way {
    let mut rng = rand::thread_rng();
    let is = |t: &str| saved.temperament.iter().flatten().any(|h| h == t);
    let class = saved.class.as_str();
    let drill = match class {
        "hunter" => Drill::Bow,
        "mage" | "warlock" | "druid" | "priest" => Drill::Cast,
        "shaman" if rng.gen_bool(0.5) => Drill::Cast,
        _ => Drill::Swing { kit, kicks: class == "rogue" },
    };
    Way {
        drill,
        blade: matches!(class, "rogue" | "warrior") && rng.gen_bool(BLADE_SHARE),
        eats: is("boisterous") || is("weary") || saved.race == "dwarf",
        toasts: is("boisterous") && merry,
        dozes: is("weary"),
    }
}

/// How often a hero doing `doing` does its thing, seconds.
fn beat_every(doing: Doing) -> f32 {
    match doing {
        Doing::Spar => SWING_EVERY,
        Doing::Talk => TALK_EVERY,
        _ => POSE_EVERY,
    }
}

/// A hero setting off for `haunt` (`slot` in `Life::haunts`) along `leg`, there till `until`.
#[allow(clippy::too_many_arguments)]
fn outing(haunt: Haunt, slot: Option<usize>, leg: Leg, until: Instant, with: Option<usize>, way: Way, now: Instant, shift_every: f32) -> Outing {
    let next_shift = now + Duration::from_secs_f32(leg.takes) + about(shift_every);
    Outing { haunt, slot, leg, arrived: false, until, next_shift, home: false, next_beat: now, with, way, dozing: false, feeds: false }
}

/// Where two heroes from `a` and `b` (their tents) stop to talk: two places a step apart about the
/// camp, a way out of the fire's ring and clear of what stands there and of the places the heroes
/// go, and the way to each; `None` when no such place is found in a few tries.
#[allow(clippy::type_complexity)]
fn talk_spot(life: &Life, fire: (f32, f32), a: (f32, f32), b: (f32, f32), rng: &mut impl Rng) -> Option<(((f32, f32), Vec<(f32, f32)>), ((f32, f32), Vec<(f32, f32)>))> {
    let busy: Vec<(f32, f32)> = life.haunts.iter().map(|h| h.at).chain(life.heroes.iter().filter_map(|h| h.out.as_ref().map(|o| o.haunt.at))).collect();
    let tau = std::f32::consts::TAU;
    for _ in 0..TALK_TRIES {
        let (angle, r, turn) = (rng.gen_range(0.0..tau), rng.gen_range(TALK_FROM_FIRE.0..TALK_FROM_FIRE.1), rng.gen_range(0.0..tau));
        let mid = (fire.0 + r * angle.cos(), fire.1 + r * angle.sin());
        let half = (turn.cos() * TALK_APART / 2.0, turn.sin() * TALK_APART / 2.0);
        let (pa, pb) = ((mid.0 - half.0, mid.1 - half.1), (mid.0 + half.0, mid.1 + half.1));
        if [pa, pb].iter().any(|&p| life.walls.in_the_way(p) || busy.iter().any(|&q| dist(p, q) < TALK_CLEAR)) {
            continue;
        }
        if let (Some(ra), Some(rb)) = (life.walls.route(a, pa), life.walls.route(b, pb)) {
            return Some(((pa, ra), (pb, rb)));
        }
    }
    None
}

/// The way a hero walks from `a` to `b`, the points the client draws its line through with their
/// heights, `a` first: over the
/// camp's own ground round all in the way and by the stairs ([`Walls::guide_route`]), as kept in
/// `ways` or else worked out now and kept; while it is still being worked out, or without the
/// game's data or such a way, round what stands in the camp on `ground`, a point every few yards
/// and a step each half yard up stairs. `None`: no way round.
fn hero_way(walls: &Walls, floors: Option<&WmoFloors>, ways: &mut HeroWays, ground: impl Fn((f32, f32)) -> f32, a: (f32, f32), b: (f32, f32)) -> Option<Vec<[f32; 3]>> {
    ways.take_arrived();
    let key = way_key(a, b);
    if let Some(way) = ways.kept.get(&key) {
        return Some(way.clone());
    }
    if let Some(f) = floors.filter(|_| !ways.coming.contains(&key) && !ways.none.contains(&key)) {
        let stand = |p: (f32, f32)| floor_near(f, p, ground(p)).unwrap_or_else(|| ground(p));
        match walls.guide_route(f, [a.0, a.1, stand(a)], b, stand(b)) {
            Some(way) => {
                ways.kept.insert(key, way.clone());
                return Some(way);
            }
            None => {
                ways.none.insert(key);
            }
        }
    }
    round_the_camp(walls, &ground, a, b)
}

/// A way's ends, a tenth of a yard apart.
fn way_key(a: (f32, f32), b: (f32, f32)) -> [i32; 4] {
    [a.0, a.1, b.0, b.1].map(|v| (v * 10.0).round() as i32)
}

/// A worked-out way, by its ends: over the camp's own ground, or `None` when there is none.
type Arrived = ([i32; 4], Option<Vec<[f32; 3]>>);

/// The heroes' ways over the camp's own ground, by their ends ([`way_key`]): those they walk
/// often worked out on a thread of their own as the camp is made ([`Game::work_out_hero_ways`]),
/// any other the first time it is walked; each kept and walked the same way after.
#[derive(Default)]
struct HeroWays {
    kept: HashMap<[i32; 4], Vec<[f32; 3]>>,
    /// The ways with none over the camp's own ground (walked round the camp's pieces).
    none: HashSet<[i32; 4]>,
    /// The ways still being worked out, and where they arrive.
    coming: HashSet<[i32; 4]>,
    arriving: Option<Receiver<Arrived>>,
}

impl HeroWays {
    /// Keeps the ways worked out since last asked.
    fn take_arrived(&mut self) {
        let Some(arriving) = &self.arriving else { return };
        for (key, way) in arriving.try_iter() {
            self.coming.remove(&key);
            match way {
                Some(way) => {
                    self.kept.insert(key, way);
                }
                None => {
                    self.none.insert(key);
                }
            }
        }
    }
}

impl Game {
    /// Works out the ways the heroes walk often, from each tent to each place they go and back and
    /// from each place on to another of its kind, over the camp's own ground on a thread of its own
    /// (worked out as the camp is made, so none is worked out as a hero sets
    /// off); till each arrives, a hero walks it round the camp's pieces. Nothing without the
    /// game's data.
    pub(super) fn work_out_hero_ways(&mut self) {
        let Some(camp) = &self.camp else { return };
        let Some(floors) = camp.floors.clone() else { return };
        let (place, life) = (camp.place, &camp.life);
        let ground = |p: (f32, f32)| super::walk::walked_height(&life.walls.rises, p, |q| self.camp_ground(place, q));
        let stand = |p: (f32, f32)| floor_near(&floors, p, ground(p)).unwrap_or_else(|| ground(p));
        let tents: Vec<(f32, f32)> = life.stands.iter().flatten().map(|(p, _)| (p[0], p[1])).collect();
        let mut pairs: Vec<((f32, f32), (f32, f32))> = Vec::new();
        for h in &life.haunts {
            pairs.extend(tents.iter().flat_map(|&t| [(t, h.at), (h.at, t)]));
            pairs.extend(life.haunts.iter().filter(|o| o.at != h.at && o.doing.group() == h.doing.group()).map(|o| (h.at, o.at)));
        }
        let jobs: Vec<([i32; 4], [f32; 3], (f32, f32), f32)> = pairs.iter().map(|&(a, b)| (way_key(a, b), [a.0, a.1, stand(a)], b, stand(b))).collect();
        let walls = life.walls.clone();
        let (send, arriving) = std::sync::mpsc::channel();
        let coming = jobs.iter().map(|j| j.0).collect();
        std::thread::spawn(move || {
            for (key, from, to, to_z) in jobs {
                // the camp gone or changed: no more
                if send.send((key, walls.guide_route(&floors, from, to, to_z))).is_err() {
                    return;
                }
            }
        });
        if let Some(camp) = &mut self.camp {
            camp.life.ways.coming = coming;
            camp.life.ways.arriving = Some(arriving);
        }
    }
}

/// The way from `a` to `b` round what stands in the camp on `ground`, as the heroes walked before
/// the camp's own ground: a point every few yards, a step each half yard up stairs.
fn round_the_camp(walls: &Walls, ground: impl Fn((f32, f32)) -> f32, a: (f32, f32), b: (f32, f32)) -> Option<Vec<[f32; 3]>> {
    let corners = walls.route(a, b)?;
    let mut way = vec![[a.0, a.1, ground(a)]];
    for pair in corners.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let step = if walls.rises.iter().any(|r| r.crossed(a, b)) { 0.5 } else { crate::game::PATH_STEP };
        let steps = (dist(a, b) / step).ceil().max(1.0) as usize;
        way.extend((1..=steps).map(|k| {
            let t = k as f32 / steps as f32;
            let at = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            [at.0, at.1, ground(at)]
        }));
    }
    Some(way)
}

/// Whether hero `saved` does `doing`.
fn suits(doing: Doing, saved: &SavedHero) -> bool {
    doing.suits(&saved.class, &saved.race, saved.temperament.as_deref().unwrap_or_default())
}

/// A gesture's one-shot emote (`Emotes.dbc`).
fn gesture_emote(g: Gesture) -> u32 {
    match g {
        Gesture::Wave => 3,
        Gesture::Bow => 2,
        Gesture::Cheer => 4,
        Gesture::Exclamation => 5,
        Gesture::Question => 6,
        Gesture::Eat => 7,
        Gesture::Laugh => 11,
        Gesture::Roar => 15,
        Gesture::Kneel => 16,
        Gesture::Cry => 18,
        Gesture::Chicken => 19,
        Gesture::Applaud => 21,
        Gesture::Flex => 23,
        Gesture::Point => 25,
        Gesture::Salute => 66,
        Gesture::Yes => 273,
        Gesture::No => 274,
    }
}

impl Game {
    /// The heroes' time away from their tents, a camp tick's worth: now and then one sets off to
    /// do a thing it is suited to (or two to talk), stays with it five to seven minutes, moving
    /// now and then to another place of the same kind, and walks home; never more than a few away
    /// at once, nor one by the player or speaking.
    fn outings_tick(&mut self, now: Instant, me: (f32, f32)) -> Vec<Packet> {
        let world = &self.run.world;
        let next_spline = &mut self.next_spline;
        let next_bunny = &mut self.next_bunny;
        let Some(camp) = &mut self.camp else {
            return Vec::new();
        };
        let place = camp.place;
        // the ground where a hero walks: the world's, or a floor of ours above it (the troll shop's
        // room and its stairs)
        let rises = camp.life.walls.rises.clone();
        let ground = |at: (f32, f32)| super::walk::walked_height(&rises, at, |p| world.floor(p.0, p.1, place.spot.2 + 20.0).unwrap_or(place.spot.2));
        let floors = camp.floors.clone();
        // a walk along `way` ([`hero_way`]) at the walking pace, walking, turning to `facing` at
        // the end; from `lifts.0` above the ground to `lifts.1` (off a log, onto one)
        let mut walk = |guid: u64, way: Vec<[f32; 3]>, facing: f32, lifts: (f32, f32)| -> (Leg, Packet) {
            let points: Vec<(f32, f32)> = way.iter().map(|p| (p[0], p[1])).collect();
            let length: f32 = points.windows(2).map(|p| dist(p[0], p[1])).sum();
            let takes = length / Speeds::DEFAULT.walk;
            let mut path = way[1..].to_vec();
            if let Some(last) = path.last_mut() {
                last[2] += lifts.1;
            }
            let from = way[0];
            let spline = *next_spline;
            *next_spline = next_spline.wrapping_add(1);
            let ms = ((takes * 1000.0).round() as u32).max(1);
            let packet = packets::monster_move_facing(guid, [from[0], from[1], from[2] + lifts.0], spline, &path, ms, false, Some(facing));
            (Leg { points, heights: way.iter().map(|p| p[2]).collect(), set_off: now, takes }, packet)
        };
        // a walk's end facing gives way to the path's own the moment it ends, so a hero arriving
        // is turned once more, to its tent's way or the place's, and set at its height there (the
        // client keeps this message's height as it is: on a log's bark)
        let face = |guid: u64, at: [f32; 3], facing: f32| packets::relayed_move(opcode::MSG_MOVE_SET_FACING, guid, 0, 0, at, facing, 0, None);
        let seat = |h: &Haunt| [h.at.0, h.at.1, ground(h.at) + h.lift];
        let visitors = &camp.visitors;
        let life = &mut camp.life;
        let (stay, shift_every) = (life.stay, life.shift_every);
        let time_of_day = life.time_of_day(now);
        let mut asleep = (0..visitors.len()).filter(|&i| life.dozing(i)).count();
        let mut rng = rand::thread_rng();
        let (mut posed, mut moves, mut gestures, mut later) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let talking: Vec<Option<usize>> = (0..visitors.len()).map(|i| life.talking_with(i)).collect();
        for (i, (_, saved)) in visitors.iter().enumerate() {
            let guid = player_guid(saved);
            let home = life.stands.get(i).copied().flatten();
            let taken: Vec<usize> = life.heroes.iter().filter_map(|h| h.out.as_ref().and_then(|o| o.slot)).collect();
            let Some(h) = life.heroes.get_mut(i) else { continue };
            let Some(o) = h.out.as_mut() else { continue };
            let Some((at, facing)) = home else {
                h.out = None;
                continue;
            };
            if !o.leg.ended(now) {
                continue;
            }
            if o.home {
                h.out = None;
                h.next_pose = now + about(POSE_EVERY);
                moves.push(face(guid, at, facing));
                continue;
            }
            // there: it settles to what it does
            if !o.arrived {
                o.arrived = true;
                o.next_beat = now + about(beat_every(o.haunt.doing));
                let mut hold = doing_hold(o.haunt.doing, &o.way, o.dozing);
                if o.feeds {
                    o.next_beat = now + Duration::from_secs_f32(FEED_TAKES);
                    hold = FEEDING;
                    // a caster of fire or light lights it with a spell, into an invisible unit in
                    // the flames, at its own cast's pace at a dummy
                    if let Some((spell, at_ground)) = kindling(&saved.class) {
                        let cast = Duration::from_secs_f32(CAST_AFTER);
                        o.next_beat = now + cast;
                        hold = doing_hold(Doing::Talk, &o.way, false);
                        let fire = place.fire();
                        let at = [fire.0, fire.1, ground(fire)];
                        *next_bunny = next_bunny.wrapping_add(1) & 0xFF_FFFF;
                        let bunny = crate::game::unit_guid(crate::entries::BUNNY, *next_bunny);
                        let target = if at_ground { SpellTarget::Ground(at) } else { SpellTarget::Unit(bunny) };
                        gestures.push(crate::update::update_object(&[still_unit_create(bunny, crate::entries::BUNNY, BUNNY_DISPLAY, 1.0, at)]));
                        gestures.push(packets::spell_start(guid, spell, cast.as_millis() as u32, target));
                        later.push((now + cast, packets::spell_go(guid, spell, &[bunny], target, None)));
                        later.push((now + cast + Duration::from_secs_f32(KINDLING_LINGERS), packets::destroy_object(bunny)));
                    }
                }
                posed.push(hold_update(guid, hold));
                moves.push(face(guid, seat(&o.haunt), o.haunt.facing));
                continue;
            }
            // fed: the flames come back, and it goes home
            if o.feeds {
                if now >= o.next_beat {
                    o.feeds = false;
                    o.until = now;
                    life.fire.low = false;
                    life.fire.feeding = false;
                    life.fire.tell = Some(false);
                    if !life.mute {
                        let line = if kindling(&saved.class).is_some() { KINDLES_THE_FIRE } else { FEEDS_THE_FIRE };
                        gestures.push(packets::monster_emote(&saved.name, line));
                    }
                }
                continue;
            }
            // dawn: a sleeper by night wakes, sitting up on its log (the weary doze on)
            if o.dozing && !o.way.dozes && time_of_day != TimeOfDay::Night {
                o.dozing = false;
                asleep = asleep.saturating_sub(1);
                posed.push(hold_update(guid, doing_hold(Doing::Sit, &o.way, false)));
                moves.push(face(guid, seat(&o.haunt), o.haunt.facing));
                continue;
            }
            // its while over (and not in the middle of a word): home, its weapons put away
            if now >= o.until && h.last_said.is_none_or(|s| s <= now) {
                let to = (at[0], at[1]);
                let from = o.haunt.at;
                let route = hero_way(&life.walls, floors.as_deref(), &mut life.ways, &ground, from, to).unwrap_or_else(|| vec![[from.0, from.1, ground(from)], [to.0, to.1, ground(to)]]);
                let (leg, packet) = walk(guid, route, facing, (o.haunt.lift, 0.0));
                o.leg = leg;
                o.home = true;
                o.slot = None;
                o.dozing = false;
                h.pose = Pose::Stand;
                posed.push(pose_update(guid, Pose::Stand));
                moves.push(packet);
                continue;
            }
            // now and then, on to another place of the same kind free (not from a talk, nor asleep)
            if now >= o.next_shift && o.with.is_none() && !o.dozing {
                o.next_shift = now + about(shift_every);
                let group = o.haunt.doing.group();
                let free: Vec<usize> = (0..life.haunts.len())
                    .filter(|k| !taken.contains(k) && life.haunts[*k].doing.group() == group && suits(life.haunts[*k].doing, saved))
                    .collect();
                let to = free.choose(&mut rng).copied().filter(|_| rng.gen_bool(0.5));
                let route = to.and_then(|k| hero_way(&life.walls, floors.as_deref(), &mut life.ways, &ground, o.haunt.at, life.haunts[k].at).map(|r| (k, r)));
                if let Some((k, route)) = route {
                    let there = life.haunts[k];
                    let (leg, packet) = walk(guid, route, there.facing, (o.haunt.lift, there.lift));
                    o.leg = leg;
                    o.haunt = there;
                    o.slot = Some(k);
                    o.arrived = false;
                    posed.push(pose_update(guid, Pose::Stand));
                    moves.push(packet);
                    continue;
                }
            }
            // what it does there now and then
            if now < o.next_beat {
                continue;
            }
            o.next_beat = now + about(beat_every(o.haunt.doing));
            match o.haunt.doing {
                Doing::Spar => {
                    let (first, then) = drill_move(o.way.drill);
                    gestures.push(packets::emote(guid, first));
                    if let Some(then) = then {
                        later.push((now + Duration::from_secs_f32(CAST_AFTER), packets::emote(guid, then)));
                    }
                }
                // a weary one at the fire dozes off, and at night anyone there
                Doing::Sit if !o.dozing && (o.way.dozes || time_of_day == TimeOfDay::Night) && asleep < ASLEEP_AT_MOST => {
                    o.dozing = true;
                    asleep += 1;
                    posed.push(hold_update(guid, doing_hold(Doing::Sit, &o.way, true)));
                    let along = (o.haunt.facing + ALONG_THE_LOG).rem_euclid(std::f32::consts::TAU);
                    moves.push(face(guid, seat(&o.haunt), along));
                }
                Doing::Sit if o.dozing => {}
                Doing::Talk if talking[i].is_none() => {}
                doing => gestures.extend(beat(doing, &o.way).map(|e| packets::emote(guid, e))),
            }
        }
        // the fire's night: once a visit it burns low, and before long someone feeds it
        if time_of_day == TimeOfDay::Night && !life.fire.burned && life.fire.lows_at.is_none() {
            life.fire.lows_at = Some(now + about(life.fire.pace.0));
        }
        if !life.fire.burned && life.fire.lows_at.is_some_and(|t| now >= t) {
            life.fire.burned = true;
            life.fire.low = true;
            life.fire.tell = Some(true);
            life.fire.feed_from = Some(now + about(life.fire.pace.1));
        }
        if life.fire.low && !life.fire.feeding && life.fire.feed_from.is_some_and(|t| now >= t) {
            life.fire.feed_from = Some(now + about(life.fire.pace.1));
            let taken: Vec<usize> = life.heroes.iter().filter_map(|h| h.out.as_ref().and_then(|o| o.slot)).collect();
            // one awake at the fire, else one resting at its tent away from the player
            let at_fire = |h: &HeroLife| h.out.as_ref().is_some_and(|o| o.arrived && !o.home && !o.dozing && o.with.is_none() && o.haunt.doing.group() == Group::Fire);
            let resting = |i: usize, h: &HeroLife| h.out.is_none() && h.last_said.is_none_or(|s| s <= now) && life.hero_at(i, now).is_some_and(|p| dist(p, me) > TO_THE_PLAYER);
            let by_the_fire: Vec<usize> = (0..visitors.len()).filter(|&i| life.heroes.get(i).is_some_and(at_fire)).collect();
            let at_tents: Vec<usize> = (0..visitors.len()).filter(|&i| life.heroes.get(i).is_some_and(|h| resting(i, h))).collect();
            // a caster who can light it with a spell first, there or at the tents
            let pool = if by_the_fire.is_empty() { at_tents } else { by_the_fire };
            let casters: Vec<usize> = pool.iter().copied().filter(|&i| kindling(&visitors[i].1.class).is_some()).collect();
            let feeder = casters.choose(&mut rng).or_else(|| pool.choose(&mut rng)).copied();
            // to the fire's edge, a place to kneel there free, the nearest
            let edge = feeder.and_then(|i| {
                let from = life.hero_at(i, now)?;
                let k = (0..life.haunts.len())
                    .filter(|k| !taken.contains(k) && life.haunts[*k].doing == Doing::Pray)
                    .min_by(|a, b| dist(life.haunts[*a].at, from).total_cmp(&dist(life.haunts[*b].at, from)))?;
                Some((i, k))
            });
            let edge = edge.and_then(|(i, k)| {
                let from = life.hero_at(i, now)?;
                Some((i, k, hero_way(&life.walls, floors.as_deref(), &mut life.ways, &ground, from, life.haunts[k].at)?))
            });
            if let Some((i, k, route)) = edge {
                let saved = &visitors[i].1;
                let guid = player_guid(saved);
                let lift = life.heroes[i].out.as_ref().map_or(0.0, |o| o.haunt.lift);
                let haunt = life.haunts[k];
                let (leg, packet) = walk(guid, route, haunt.facing, (lift, 0.0));
                let way = way_of(saved, life.kits.get(i).copied().unwrap_or_default(), life.merry);
                let mut o = outing(haunt, Some(k), leg, now + Duration::from_secs(3600), None, way, now, shift_every);
                o.feeds = true;
                let h = &mut life.heroes[i];
                h.pose = Pose::Stand;
                h.out = Some(o);
                posed.push(pose_update(guid, Pose::Stand));
                moves.push(packet);
                life.fire.feeding = true;
            }
        }
        if now >= life.next_outing {
            life.next_outing = now + about(life.outing_every);
            let away = life.heroes.iter().filter(|h| h.out.is_some()).count();
            let taken: Vec<usize> = life.heroes.iter().filter_map(|h| h.out.as_ref().and_then(|o| o.slot)).collect();
            let stand = |i: usize| life.stands.get(i).copied().flatten().map(|(p, _)| (p[0], p[1]));
            let idle: Vec<usize> = (0..visitors.len())
                .filter(|&i| {
                    let resting = life.heroes.get(i).is_some_and(|h| h.out.is_none() && h.last_said.is_none_or(|s| s <= now));
                    resting && stand(i).is_some_and(|p| dist(p, me) > TO_THE_PLAYER)
                })
                .collect();
            let mut set_off: Vec<(usize, Outing, Packet)> = Vec::new();
            let free_for = |i: usize, g: Group| -> Vec<usize> {
                let open = |k: &usize| !taken.contains(k) && life.haunts[*k].doing.group() == g && suits(life.haunts[*k].doing, &visitors[i].1);
                (0..life.haunts.len()).filter(open).collect()
            };
            // at night one keeps watch: while nobody does, the next to set off is one who can
            let watched = life.heroes.iter().any(|h| h.out.as_ref().is_some_and(|o| !o.home && o.haunt.doing.group() == Group::Watch));
            let watchers: Vec<usize> = idle.iter().copied().filter(|&i| !free_for(i, Group::Watch).is_empty()).collect();
            let to_watch = time_of_day == TimeOfDay::Night && !watched && !watchers.is_empty();
            let pick = if to_watch { watchers.choose(&mut rng) } else { idle.choose(&mut rng) };
            if let Some(&i) = pick.filter(|_| away < AWAY_AT_MOST) {
                let saved = &visitors[i].1;
                let from = stand(i).unwrap_or_default();
                let free = |g: Group| free_for(i, g);
                let partners: Vec<usize> = idle.iter().copied().filter(|&j| j != i).collect();
                let places = [Group::Fire, Group::Armoury, Group::Study, Group::Training, Group::Watch];
                let mut groups: Vec<Group> = places.into_iter().filter(|&g| time_of_day.goes_to(g) && !free(g).is_empty()).collect();
                if to_watch {
                    groups = vec![Group::Watch];
                } else if away + 2 <= AWAY_AT_MOST && !partners.is_empty() && time_of_day.goes_to(Group::Talk) {
                    groups.push(Group::Talk);
                }
                let way = |i: usize| way_of(&visitors[i].1, life.kits.get(i).copied().unwrap_or_default(), life.merry);
                let stays = Duration::from_secs_f32(rng.gen_range(stay.0..=stay.1));
                match groups.choose(&mut rng).copied() {
                    // two talk, face to face somewhere about the camp
                    Some(Group::Talk) => {
                        let j = partners.choose(&mut rng).copied().unwrap_or(i);
                        let (fa, fb) = (from, stand(j).unwrap_or_default());
                        let spot = talk_spot(life, place.fire(), fa, fb, &mut rng).map(|((pa, _), (pb, _))| (pa, pb));
                        let mut walked = |a: (f32, f32), b: (f32, f32)| hero_way(&life.walls, floors.as_deref(), &mut life.ways, &ground, a, b);
                        let ways = spot.and_then(|(pa, pb)| Some(((pa, walked(fa, pa)?), (pb, walked(fb, pb)?))));
                        if let Some(((pa, ra), (pb, rb))) = ways {
                            let toward = |a: (f32, f32), b: (f32, f32)| (b.1 - a.1).atan2(b.0 - a.0);
                            let (leg_a, walk_a) = walk(player_guid(saved), ra, toward(pa, pb), (0.0, 0.0));
                            let (leg_b, walk_b) = walk(player_guid(&visitors[j].1), rb, toward(pb, pa), (0.0, 0.0));
                            let until = now + Duration::from_secs_f32(leg_a.takes.max(leg_b.takes)) + stays;
                            for (who, other, here, there, leg, packet) in [(i, j, pa, pb, leg_a, walk_a), (j, i, pb, pa, leg_b, walk_b)] {
                                let haunt = Haunt { at: here, facing: toward(here, there), doing: Doing::Talk, lift: 0.0 };
                                set_off.push((who, outing(haunt, None, leg, until, Some(other), way(who), now, shift_every), packet));
                            }
                        }
                    }
                    Some(g) => {
                        if let Some(&k) = free(g).choose(&mut rng) {
                            let haunt = life.haunts[k];
                            if let Some(route) = hero_way(&life.walls, floors.as_deref(), &mut life.ways, &ground, from, haunt.at) {
                                let (leg, packet) = walk(player_guid(saved), route, haunt.facing, (0.0, haunt.lift));
                                let until = now + Duration::from_secs_f32(leg.takes) + stays;
                                set_off.push((i, outing(haunt, Some(k), leg, until, None, way(i), now, shift_every), packet));
                            }
                        }
                    }
                    None => {}
                }
            }
            for (i, o, packet) in set_off {
                let guid = player_guid(&visitors[i].1);
                let h = &mut life.heroes[i];
                if h.pose != Pose::Stand {
                    posed.push(pose_update(guid, Pose::Stand));
                }
                h.pose = Pose::Stand;
                h.out = Some(o);
                moves.push(packet);
            }
        }
        camp.later.extend(later);
        let mut out = Vec::new();
        if !posed.is_empty() {
            out.push(crate::update::update_object(&posed));
        }
        out.extend(moves);
        out.extend(gestures);
        out
    }

    /// The heroes' life, a camp tick's worth: walks, poses, a gesture, a line.
    pub(super) fn life_tick(&mut self, now: Instant) -> Vec<Packet> {
        let mut out = Vec::new();
        let Some(me) = self.run.heroes.first().map(|h| h.pos) else {
            return out;
        };
        out.extend(self.outings_tick(now, me));
        // the fire burning low, or fed: the client dims its flames and sparks, or lights them again
        let fire = self.camp.as_mut().and_then(|c| Some((c.life.fire.tell.take()?, c.fire_guid()?)));
        if let Some((low, fire)) = fire {
            let told = self.players.iter().map(|p| (p.hero, ToClient::CampFireLow { fire, low })).collect();
            out.extend(self.lines(told).into_iter().map(|(_, p)| p));
        }
        let hero_name = self.run.heroes.first().and_then(|h| self.saved(h.id)).map_or_else(String::new, |s| s.name.clone());
        let Some(camp) = &mut self.camp else {
            return out;
        };
        let content = self.run.content.clone();
        let chatter = &content.camp_chatter;
        // where each hero is now, and whether at its tent (only there does it talk with another)
        let where_now: Vec<Option<(f32, f32)>> = (0..camp.visitors.len()).map(|i| camp.life.hero_at(i, now)).collect();
        let home: Vec<bool> = (0..camp.visitors.len()).map(|i| camp.life.at_home(i)).collect();
        let at = |i: usize| where_now.get(i).copied().flatten();
        let in_sight = |i: usize| at(i).is_some_and(|a| dist(a, me) <= IN_SIGHT);
        let gloomy = camp.life.gloomy;

        // poses: each hero at its tent settles into another now and then, in sight or not (never
        // frozen when the player turns)
        let mut posed = Vec::new();
        for (i, (_, saved)) in camp.visitors.iter().enumerate() {
            let Some(life) = camp.life.heroes.get_mut(i) else { continue };
            if now < life.next_pose || life.out.is_some() {
                continue;
            }
            life.next_pose = now + about(POSE_EVERY);
            let poses = temperament_now(saved).and_then(|t| chatter.temperament(t)).map_or(&[][..], |t| &t.poses[..]);
            let pose = match poses.choose(&mut rand::thread_rng()) {
                Some(p) if !rand::thread_rng().gen_bool(AT_EASE_SHARE) => *p,
                _ => Pose::Stand,
            };
            // a low camp does not dance
            let pose = if gloomy && pose == Pose::Dance { Pose::Stand } else { pose };
            if pose != life.pose {
                life.pose = pose;
                posed.push(pose_update(player_guid(saved), pose));
            }
        }
        if !posed.is_empty() {
            out.push(crate::update::update_object(&posed));
        }

        // a gesture, by a hero standing at its tent in sight
        if now >= camp.life.next_gesture {
            camp.life.next_gesture = now + about(GESTURE_EVERY);
            let standing: Vec<usize> = (0..camp.visitors.len())
                .filter(|&i| in_sight(i) && home[i] && camp.life.heroes.get(i).is_some_and(|l| l.pose == Pose::Stand))
                .collect();
            if let Some(&i) = standing.choose(&mut rand::thread_rng()) {
                let saved = &camp.visitors[i].1;
                let gestures = temperament_now(saved).and_then(|t| chatter.temperament(t)).map_or(&[][..], |t| &t.gestures[..]);
                // a low camp keeps its cheer to itself
                if let Some(g) = gestures.choose(&mut rand::thread_rng()).filter(|g| !(gloomy && too_merry_for_a_low_camp(**g))) {
                    out.push(packets::emote(player_guid(saved), gesture_emote(*g)));
                }
            }
        }

        // a line: one voice at a time, never over a window or Wren
        if now < camp.life.next_line || now < camp.life.quiet_until || camp.life.window_open || camp.tour.is_some() {
            return out;
        }
        if let Some(news) = camp.life.news {
            // what has just happened, first: said once, by a hero in sight with a line for it
            // (the nearest greets a newcomer), or by two in an exchange; nobody in sight yet, it
            // waits for one
            let zone = match news {
                CampNews::Won { zone, .. } | CampNews::Lost { zone } => content.zones.get(zone).map(|z| z.name.as_str()),
                CampNews::Newcomer => None,
            };
            let first_zone_win = matches!(news, CampNews::Won { first_zone_win: true, .. });
            // after a zone's first win always two talk of it if two can; after a loss, now and then
            let talked_of = match news {
                CampNews::Won { first_zone_win: true, .. } => Some(LineWhen::Win),
                CampNews::Lost { .. } if rand::thread_rng().gen_bool(LOSS_EXCHANGE_SHARE) => Some(LineWhen::Loss),
                _ => None,
            };
            let near: Vec<usize> = (0..camp.visitors.len()).filter(|&i| in_sight(i)).collect();
            let near_home: Vec<usize> = near.iter().copied().filter(|&i| home[i]).collect();
            let exchange = talked_of.and_then(|when| pick_exchange(chatter, &camp.visitors, &camp.life.stands, &near_home, when, &camp.life.exchanged));
            let when = match news {
                CampNews::Won { .. } => LineWhen::Win,
                CampNews::Lost { .. } => LineWhen::Loss,
                CampNews::Newcomer => LineWhen::Newcomer,
            };
            let mut speakers: Vec<(usize, usize)> = Vec::new();
            for &i in &near {
                let saved = &camp.visitors[i].1;
                let lines = |t: &str| -> Vec<usize> {
                    let fitting = chatter.lines.iter().enumerate().filter(|(_, l)| fits(l, saved, t, when));
                    fitting.map(|(k, _)| k).collect()
                };
                // its leading temperament's lines, else its second's
                let k = saved.temperament.iter().flatten().map(|t| lines(t)).find(|ks| !ks.is_empty());
                if let Some(&k) = k.as_ref().and_then(|ks| ks.choose(&mut rand::thread_rng())) {
                    speakers.push((i, k));
                }
            }
            let nearest = |&(i, _): &(usize, usize)| at(i).map_or(f32::MAX, |a| dist(a, me));
            let speaker = match news {
                CampNews::Newcomer => speakers.iter().min_by(|a, b| nearest(a).total_cmp(&nearest(b))).copied(),
                _ => speakers.choose(&mut rand::thread_rng()).copied(),
            };
            let talking: Vec<usize> = match (exchange, speaker) {
                (Some((_, a, b)), _) => vec![a, b],
                (None, Some((i, _))) => vec![i],
                (None, None) => Vec::new(),
            };
            if !talking.is_empty() {
                camp.life.news = None;
                // a zone won for the first time: the whole camp cheers and applauds round the
                // ring, one hero a second apart, and what is said comes after
                let mut wait = Duration::ZERO;
                if first_zone_win {
                    for j in (0..camp.visitors.len()).filter(|j| !talking.contains(j)) {
                        let g = [Gesture::Cheer, Gesture::Applaud].choose(&mut rand::thread_rng()).copied().unwrap_or(Gesture::Cheer);
                        camp.later.push((now + wait, packets::emote(player_guid(&camp.visitors[j].1), gesture_emote(g))));
                        wait += Duration::from_secs(RIPPLE_EVERY);
                    }
                }
                let lasts = if let Some((k, a, b)) = exchange {
                    let names = Names { hero: &hero_name, zone: zone.unwrap_or_default() };
                    let cheer = (when == LineWhen::Win).then(|| gesture_emote(Gesture::Cheer));
                    let said = exchange_packets(&chatter.exchanges[k], camp, a, b, &names, cheer, true);
                    camp.life.exchanged.insert(k);
                    wait + play(camp, now + wait, said)
                } else if let Some((i, k)) = speaker {
                    camp.life.said.insert(k);
                    let guid = player_guid(&camp.visitors[i].1);
                    let gesture = if when == LineWhen::Win { gesture_emote(Gesture::Cheer) } else { EMOTE_TALK };
                    camp.later.extend(camp.life.line(guid, &camp.visitors[i].1.name, &chatter.lines[k].text).map(|p| (now + wait, p)));
                    camp.later.push((now + wait, packets::emote(guid, gesture)));
                    wait
                } else {
                    wait
                };
                camp.life.next_line = now + lasts + about(LINE_EVERY);
                for &i in &talking {
                    if let Some(l) = camp.life.heroes.get_mut(i) {
                        l.last_said = Some(now + lasts);
                    }
                }
                return out;
            }
        }
        // now and then, two heroes in sight talk to each other
        if now >= camp.life.next_exchange {
            camp.life.next_exchange = now + Duration::from_secs_f32(NOBODY_NEAR_RETRY);
            // two talking face to face about the camp first (they are there already), else two at
            // their tents
            let pairs: Vec<[usize; 2]> = (0..camp.visitors.len())
                .filter_map(|i| camp.life.talking_with(i).filter(|&j| i < j && in_sight(i) && in_sight(j)).map(|j| [i, j]))
                .collect();
            let pair = pairs.iter().find_map(|near| pick_exchange(chatter, &camp.visitors, &camp.life.stands, near, LineWhen::Idle, &camp.life.exchanged));
            let near: Vec<usize> = (0..camp.visitors.len()).filter(|&i| in_sight(i) && home[i]).collect();
            let at_tents = pair.is_none();
            let found = pair.or_else(|| pick_exchange(chatter, &camp.visitors, &camp.life.stands, &near, LineWhen::Idle, &camp.life.exchanged));
            if let Some((k, a, b)) = found {
                let names = Names { hero: &hero_name, zone: "" };
                let said = exchange_packets(&chatter.exchanges[k], camp, a, b, &names, None, at_tents);
                camp.life.exchanged.insert(k);
                let lasts = play(camp, now, said);
                camp.life.next_exchange = now + lasts + about(EXCHANGE_EVERY);
                camp.life.next_line = now + lasts + about(LINE_EVERY);
                for i in [a, b] {
                    if let Some(l) = camp.life.heroes.get_mut(i) {
                        l.last_said = Some(now + lasts);
                    }
                }
                return out;
            }
        }
        let rested = |l: &HeroLife| l.last_said.is_none_or(|t| now.duration_since(t).as_secs_f32() >= HERO_LINE_EVERY);
        // at night, about one line in two to nobody is the night's
        let night_line = camp.life.time_of_day(now) == TimeOfDay::Night && rand::thread_rng().gen_bool(NIGHT_LINE_SHARE);
        let mut speakers: Vec<(usize, usize)> = Vec::new();
        let mut speakers_near = false;
        for (i, (_, saved)) in camp.visitors.iter().enumerate() {
            if !in_sight(i) || camp.life.dozing(i) || !camp.life.heroes.get(i).is_some_and(rested) {
                continue;
            }
            let Some(temperament) = temperament_now(saved) else { continue };
            speakers_near = true;
            let when = if at(i).is_some_and(|a| dist(a, me) <= TO_THE_PLAYER) {
                LineWhen::ToPlayer
            } else if night_line {
                LineWhen::Night
            } else {
                LineWhen::Idle
            };
            let fitting: Vec<usize> = chatter
                .lines
                .iter()
                .enumerate()
                .filter(|(k, l)| !camp.life.said.contains(k) && fits(l, saved, temperament, when))
                .map(|(k, _)| k)
                .collect();
            if let Some(&k) = fitting.choose(&mut rand::thread_rng()) {
                speakers.push((i, k));
            }
        }
        let Some(&(i, k)) = speakers.choose(&mut rand::thread_rng()) else {
            // nobody near (or every line that fits said: they may come round again)
            camp.life.next_line = now + Duration::from_secs_f32(NOBODY_NEAR_RETRY);
            if speakers_near {
                camp.life.said.clear();
            }
            return out;
        };
        camp.life.next_line = now + about(LINE_EVERY);
        camp.life.said.insert(k);
        if let Some(l) = camp.life.heroes.get_mut(i) {
            l.last_said = Some(now);
        }
        let saved = &camp.visitors[i].1;
        let guid = player_guid(saved);
        out.extend(camp.life.line(guid, &saved.name, &chatter.lines[k].text));
        out.push(packets::emote(guid, EMOTE_TALK));
        out
    }
}

/// What an exchange's `{hero}` and `{zone}` are.
struct Names<'a> {
    hero: &'a str,
    zone: &'a str,
}

/// Whether hero `saved` may speak side `side` of an exchange (a Quiet hero, with no temperament,
/// never joins one).
fn speaks(side: &Speaker, saved: &SavedHero) -> bool {
    saved.temperament.as_ref().is_some_and(|t| !t.is_empty())
        && side.temperament.as_ref().is_none_or(|t| saved.temperament.iter().flatten().any(|h| h == t))
        && side.race.as_ref().is_none_or(|r| *r == saved.race)
        && side.class.as_ref().is_none_or(|c| *c == saved.class)
        && side.time_period.is_none_or(|p| p == saved.time_period)
}

/// An exchange of moment `when` two of heroes `near` (by visitor) can have, and who has it: its
/// place in `camp_chatter.ron`, the first speaker and the second. Tent neighbours first (one the
/// other's nearest, by `stands`); none had this sitting (`had`) before any again.
fn pick_exchange(
    chatter: &CampChatter,
    visitors: &[(u32, SavedHero)],
    stands: &[Option<([f32; 3], f32)>],
    near: &[usize],
    when: LineWhen,
    had: &HashSet<usize>,
) -> Option<(usize, usize, usize)> {
    let mut all = Vec::new();
    for (k, e) in chatter.exchanges.iter().enumerate().filter(|(_, e)| e.when == when) {
        for &a in near {
            for &b in near.iter().filter(|&&b| b != a) {
                if speaks(&e.first, &visitors[a].1) && speaks(&e.second, &visitors[b].1) {
                    all.push((k, a, b));
                }
            }
        }
    }
    let fresh: Vec<_> = all.iter().copied().filter(|(k, _, _)| !had.contains(k)).collect();
    let pool = if fresh.is_empty() { all } else { fresh };
    let apart = |a: usize, b: usize| match (stands.get(a), stands.get(b)) {
        (Some(Some((p, _))), Some(Some((q, _)))) => dist((p[0], p[1]), (q[0], q[1])),
        _ => f32::MAX,
    };
    let nearest = |a: usize| near.iter().copied().filter(|&b| b != a).min_by(|&b, &c| apart(a, b).total_cmp(&apart(a, c)));
    let neighbours: Vec<_> = pool.iter().copied().filter(|&(_, a, b)| nearest(a) == Some(b) || nearest(b) == Some(a)).collect();
    let pool = if neighbours.is_empty() { pool } else { neighbours };
    pool.choose(&mut rand::thread_rng()).copied()
}

/// Exchange `e` between heroes `a` and `b` (by visitor), as packets after its start: both stand
/// and turn to each other (each with `cheer` first, if given), say the lines in turn a few seconds
/// apart with the talk gesture, then turn back to the fire; two already face to face about the
/// camp (not `at_tents`) only talk.
fn exchange_packets(e: &Exchange, camp: &CampState, a: usize, b: usize, names: &Names, cheer: Option<u32>, at_tents: bool) -> Vec<(Duration, Packet)> {
    let mut out = Vec::new();
    let (Some(Some((at_a, back_a))), Some(Some((at_b, back_b)))) = (camp.life.stands.get(a), camp.life.stands.get(b)) else {
        return out;
    };
    let guid = |i: usize| player_guid(&camp.visitors[i].1);
    let face = |i: usize, at: [f32; 3], facing: f32| packets::relayed_move(opcode::MSG_MOVE_SET_FACING, guid(i), 0, 0, at, facing, 0, None);
    let toward = |from: [f32; 3], to: [f32; 3]| (to[1] - from[1]).atan2(to[0] - from[0]);
    let mut stand = Fields::new();
    stand.bytes(field::UNIT_BYTES_1, [STAND, 0, 0, 0]).u32(field::UNIT_NPC_EMOTESTATE, 0);
    let stood = [a, b].map(|i| ObjectUpdate::Values { guid: guid(i), fields: stand.clone() });
    if at_tents {
        out.push((Duration::ZERO, crate::update::update_object(&stood)));
        out.push((Duration::ZERO, face(a, *at_a, toward(*at_a, *at_b))));
        out.push((Duration::ZERO, face(b, *at_b, toward(*at_b, *at_a))));
    }
    let mut t = Duration::from_secs_f32(1.0);
    if let Some(cheer) = cheer {
        out.push((t, packets::emote(guid(a), cheer)));
        out.push((t, packets::emote(guid(b), cheer)));
        t += Duration::from_secs_f32(EXCHANGE_GAP / 2.0);
    }
    for (n, line) in e.lines.iter().enumerate() {
        let i = if n % 2 == 0 { a } else { b };
        let text = line.replace("{hero}", names.hero).replace("{zone}", names.zone);
        out.extend(camp.life.line(guid(i), &camp.visitors[i].1.name, &text).map(|p| (t, p)));
        out.push((t, packets::emote(guid(i), EMOTE_TALK)));
        t += super::line_gap(&text);
    }
    if at_tents {
        out.push((t, face(a, *at_a, *back_a)));
        out.push((t, face(b, *at_b, *back_b)));
    }
    out
}

/// Plays `said` from `start` (onto the camp's later packets); how long it lasts.
fn play(camp: &mut CampState, start: Instant, said: Vec<(Duration, Packet)>) -> Duration {
    let lasts = said.iter().map(|(d, _)| *d).max().unwrap_or_default();
    camp.later.extend(said.into_iter().map(|(d, p)| (start + d, p)));
    lasts
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::Content;

    use super::*;
    use crate::game::camp::{camp_content, CAMP_HOUR, NEUTRAL_ALLIANCE_CAMP};

    /// A camp for a hero just made, Aria, with Brom (stalwart, then grim) at the next tent, and
    /// the player beside him; the camp's first quiet over.
    fn a_newcomers_camp() -> (Game, Content, SavedHero) {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).unwrap();
        let saves = Arc::new(crate::save::Saves::in_memory());
        assert!(saves.temper(&content, 0, &["stalwart".into(), "grim".into()]));
        let brom = saves.create("Brom", "warrior", "dwarf", 0, [0; 5]).unwrap();
        assert_eq!(saves.take_news(brom.guid), Some(CampNews::Newcomer));
        let aria = saves.create("Aria", "paladin", "human", 0, [0; 5]).unwrap();
        let (own, zone) = camp_content(&content, NEUTRAL_ALLIANCE_CAMP, CAMP_HOUR).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(own, None, zone, &aria, saves, inputs).unwrap();
        game.make_camp(NEUTRAL_ALLIANCE_CAMP, aria.guid, std::slice::from_ref(&brom), Default::default());
        let (at, _) = NEUTRAL_ALLIANCE_CAMP.hero_stand(1).unwrap();
        game.run.heroes[0].pos = (at.0 + 6.0, at.1);
        let camp = game.camp.as_mut().unwrap();
        camp.life.next_line = Instant::now();
        camp.life.quiet_until = Instant::now();
        (game, content, brom)
    }

    /// Brom's lines for moment `when`, as said.
    fn says(content: &Content, brom: &SavedHero, when: LineWhen) -> Vec<Packet> {
        let fitting = content.camp_chatter.lines.iter().filter(|l| ["stalwart", "grim"].iter().any(|t| fits(l, brom, t, when)));
        fitting.map(|l| packets::monster_say(player_guid(brom), &brom.name, &l.text)).collect()
    }

    #[test]
    fn the_neighbour_greets_a_hero_just_made_once() {
        let (mut game, content, brom) = a_newcomers_camp();
        let greetings = says(&content, &brom, LineWhen::Newcomer);
        assert!(!greetings.is_empty());
        game.life_tick(Instant::now());
        let later = &game.camp.as_ref().unwrap().later;
        assert!(later.iter().any(|(_, p)| greetings.contains(p)), "Brom greets the newcomer");
        assert!(game.camp.as_ref().unwrap().life.news.is_none(), "once");
    }

    #[test]
    fn a_zone_won_for_the_first_time_goes_round_the_camp_then_is_spoken_of() {
        let (mut game, content, brom) = a_newcomers_camp();
        game.camp.as_mut().unwrap().life.news = Some(CampNews::Won { first_zone_win: true, zone: 0 });
        let wins = says(&content, &brom, LineWhen::Win);
        let now = Instant::now();
        let out = game.life_tick(now);
        assert!(!out.iter().any(|p| wins.contains(p)), "not at once");
        let later = &game.camp.as_ref().unwrap().later;
        assert!(later.iter().any(|(_, p)| wins.contains(p)), "Brom speaks of it");
        assert!(later.contains(&(now, packets::emote(player_guid(&brom), gesture_emote(Gesture::Cheer)))), "and cheers");
    }

    #[test]
    fn two_neighbours_in_sight_talk_to_each_other_in_turn() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).unwrap();
        let saves = Arc::new(crate::save::Saves::in_memory());
        let tempered = |name: &str, class: &str, race: &str| {
            assert!(saves.temper(&content, 0, &["weary".into()]));
            saves.create(name, class, race, 0, [0; 5]).unwrap()
        };
        let brom = tempered("Brom", "warrior", "dwarf");
        let fenn = tempered("Fenn", "hunter", "dwarf");
        let wynn = tempered("Wynn", "hunter", "human");
        let aria = saves.create("Aria", "paladin", "human", 0, [0; 5]).unwrap();
        saves.take_news(aria.guid);
        let (own, zone) = camp_content(&content, NEUTRAL_ALLIANCE_CAMP, CAMP_HOUR).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(own, None, zone, &aria, saves, inputs).unwrap();
        // Fenn and Brom at tents side by side (plots 1 and 2), Wynn at the next (plot 3); the
        // player six yards out from between Fenn and Brom on the side away from Wynn, who is out
        // of sight there
        game.make_camp(NEUTRAL_ALLIANCE_CAMP, aria.guid, &[fenn.clone(), brom.clone(), wynn], Default::default());
        let (a, _) = NEUTRAL_ALLIANCE_CAMP.hero_stand(1).unwrap();
        let (b, _) = NEUTRAL_ALLIANCE_CAMP.hero_stand(2).unwrap();
        let (w, _) = NEUTRAL_ALLIANCE_CAMP.hero_stand(3).unwrap();
        let between = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let away = dist(between, w);
        let me = (between.0 + (between.0 - w.0) * 6.0 / away, between.1 + (between.1 - w.1) * 6.0 / away);
        assert!(dist(me, a) <= IN_SIGHT && dist(me, b) <= IN_SIGHT && dist(me, w) > IN_SIGHT);
        game.run.heroes[0].pos = me;
        let now = Instant::now();
        let camp = game.camp.as_mut().unwrap();
        (camp.life.next_line, camp.life.quiet_until) = (now, now);
        // only the hunter's exchange, to know what is said
        let mut only = (*game.run.content).clone();
        only.camp_chatter.exchanges.retain(|e| e.first.class.as_deref() == Some("hunter") && e.lines[0] == "Have you seen my pet?");
        assert_eq!(only.camp_chatter.exchanges.len(), 1);
        game.run.content = Arc::new(only);
        game.life_tick(now);
        let said: Vec<(&str, String)> = game.camp.as_ref().unwrap().later.iter().filter_map(|(_, p)| {
            [&fenn, &brom].into_iter().find_map(|h| content.camp_chatter.exchanges.iter().flat_map(|e| &e.lines).find(|l| *p == packets::monster_say(player_guid(h), &h.name, l)).map(|l| (h.name.as_str(), l.clone())))
        }).collect();
        let names: Vec<&str> = said.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, ["Fenn", "Brom", "Fenn"], "in turn: {said:?}");
        // a Quiet hero never joins one
        let quiet = SavedHero { temperament: Some(Vec::new()), ..brom.clone() };
        assert!(!speaks(&Speaker::default(), &quiet) && speaks(&Speaker::default(), &brom));
        let camp = game.camp.as_ref().unwrap();
        assert!(camp.life.next_line > now + Duration::from_secs_f32(EXCHANGE_GAP * 2.0), "no line over it");
    }

    #[test]
    fn a_hero_walks_to_the_anvil_hammers_there_and_walks_home() {
        let (mut game, _, _) = a_newcomers_camp();
        let far = (game.run.heroes[0].pos.0 + 100.0, game.run.heroes[0].pos.1);
        let now = Instant::now();
        let camp = game.camp.as_mut().unwrap();
        // by day, the anvil the only place to go
        camp.life.hour = 12;
        camp.life.haunts.retain(|h| h.doing == Doing::Hammer);
        assert_eq!(camp.life.haunts.len(), 1);
        let anvil = camp.life.haunts[0].at;
        camp.life.next_outing = now;
        let set_off = game.outings_tick(now, far);
        assert_eq!(set_off.len(), 1, "a walk: {set_off:?}");
        let life = &game.camp.as_ref().unwrap().life;
        let takes = life.heroes[0].out.as_ref().expect("Brom is out").leg.takes;
        assert!(!life.at_home(0));
        let there = now + Duration::from_secs_f32(takes + 0.1);
        assert!(dist(life.hero_at(0, there).unwrap(), anvil) < 0.01, "at the anvil");
        // there: the hammering, and only there
        game.camp.as_mut().unwrap().life.next_outing = there + Duration::from_secs(3600);
        let settled = game.outings_tick(there, far);
        let brom = player_guid(&game.camp.as_ref().unwrap().visitors[0].1);
        assert_eq!(settled.len(), 2, "hammers, turned to the anvil: {settled:?}");
        let hammers = Hold { stand: STAND, state: STATE_WORK, sheath: SHEATHED };
        assert_eq!(settled[0], crate::update::update_object(&[hold_update(brom, hammers)]));
        assert_eq!(settled[1].opcode, opcode::MSG_MOVE_SET_FACING);
        // it stays five to seven minutes
        let done = game.camp.as_ref().unwrap().life.heroes[0].out.as_ref().unwrap().until;
        let stays = done.duration_since(now).as_secs_f32() - takes;
        assert!((STAY.0..=STAY.1 + 0.01).contains(&stays), "stays {stays}s");
        // its while over: it stands and walks home
        let home = game.outings_tick(done, far);
        assert_eq!(home.len(), 2, "stands, and a walk: {home:?}");
        assert_eq!(home[0], crate::update::update_object(&[pose_update(brom, Pose::Stand)]));
        let back = done + Duration::from_secs_f32(game.camp.as_ref().unwrap().life.heroes[0].out.as_ref().unwrap().leg.takes + 0.1);
        let arrived = game.outings_tick(back, far);
        assert!(game.camp.as_ref().unwrap().life.at_home(0), "home again");
        assert!(arrived.iter().any(|p| p.opcode == opcode::MSG_MOVE_SET_FACING), "turned to its tent's way: {arrived:?}");
    }

    #[test]
    fn a_priest_prays_and_reads_but_never_spars_nor_smiths() {
        let ok = |d: Doing| d.suits("priest", "human", &[]);
        assert!(ok(Doing::Pray) && ok(Doing::Read) && ok(Doing::Sit) && ok(Doing::Talk));
        assert!(!ok(Doing::Spar) && !ok(Doing::Hammer) && !ok(Doing::LookOver) && !ok(Doing::Watch));
        assert!(Doing::Hammer.suits("mage", "dwarf", &[]), "a dwarf smiths whatever its class");
        assert!(Doing::Pray.suits("rogue", "human", &["devout".into()]), "the devout pray");
    }

    #[test]
    fn two_heroes_walk_off_to_talk_face_to_face() {
        let (mut game, _, _) = a_newcomers_camp();
        let far = (game.run.heroes[0].pos.0 + 100.0, game.run.heroes[0].pos.1);
        let now = Instant::now();
        // by day, a second hero, and only talking to go out for
        let mut cael = game.camp.as_ref().unwrap().visitors[0].1.clone();
        cael.name = "Cael".into();
        cael.guid += 1;
        let camp = game.camp.as_mut().unwrap();
        camp.visitors.push((u32::MAX - 9, cael));
        camp.life.heroes.push(HeroLife { pose: Pose::Stand, next_pose: now, last_said: None, out: None });
        let second = NEUTRAL_ALLIANCE_CAMP.hero_stand(2).map(|(at, facing)| ([at.0, at.1, 0.0], facing));
        camp.life.stands.push(second);
        camp.life.haunts.clear();
        camp.life.hour = 12;
        camp.life.next_outing = now;
        game.outings_tick(now, far);
        let life = &game.camp.as_ref().unwrap().life;
        let (a, b) = (life.heroes[0].out.as_ref().expect("Brom out"), life.heroes[1].out.as_ref().expect("Cael out"));
        assert_eq!((a.with, b.with), (Some(1), Some(0)));
        assert_eq!(a.until, b.until, "they part together");
        assert!((dist(a.haunt.at, b.haunt.at) - TALK_APART).abs() < 0.01);
        let toward = |p: (f32, f32), q: (f32, f32)| (q.1 - p.1).atan2(q.0 - p.0);
        assert!((a.haunt.facing - toward(a.haunt.at, b.haunt.at)).abs() < 1e-4, "face to face");
        // both there: they are talking
        let there = now + Duration::from_secs_f32(a.leg.takes.max(b.leg.takes) + 0.1);
        game.camp.as_mut().unwrap().life.next_outing = there + Duration::from_secs(3600);
        game.outings_tick(there, far);
        assert_eq!(game.camp.as_ref().unwrap().life.talking_with(0), Some(1));
    }

    #[test]
    fn the_camps_day_goes_round_a_step_a_run() {
        assert_eq!(TimeOfDay::at(22.0), TimeOfDay::Night);
        assert_eq!(TimeOfDay::at(4.9), TimeOfDay::Night);
        assert_eq!(TimeOfDay::at(5.5), TimeOfDay::Dawn);
        assert_eq!(TimeOfDay::at(12.0), TimeOfDay::Day);
        assert_eq!(TimeOfDay::at(20.0), TimeOfDay::Dusk);
        assert!(!TimeOfDay::Dusk.goes_to(Group::Armoury) && !TimeOfDay::Night.goes_to(Group::Training) && !TimeOfDay::Night.goes_to(Group::Talk));
        assert!(TimeOfDay::Night.goes_to(Group::Fire) && TimeOfDay::Night.goes_to(Group::Watch) && TimeOfDay::Day.goes_to(Group::Armoury));
        if std::env::var_os("ESURV_CAMP_HOUR").is_none() {
            let hours: Vec<u32> = (0..7).map(crate::game::camp_hour).collect();
            assert_eq!(hours, [22, 1, 6, 9, 15, 20, 22], "the first visit at night, then a step a run");
        }
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).unwrap();
        let saves = crate::save::Saves::in_memory();
        let aria = saves.create("Aria", "paladin", "human", 0, [0; 5]).unwrap();
        assert_eq!(saves.runs_back(), 0);
        saves.finish_run(&content, aria.guid, &crate::save::FinishedRun { first: true, ..Default::default() });
        assert_eq!(saves.runs_back(), 1);
    }

    #[test]
    fn the_camps_mood_follows_the_last_five_runs() {
        let (w, l) = (true, false);
        assert_eq!(mood(&[]), Mood::Even);
        assert_eq!(mood(&[w, l, w, l]), Mood::Even);
        assert_eq!(mood(&[w, w, l, l, l]), Mood::Gloomy, "the last three lost");
        assert_eq!(mood(&[l, w, l, w, l]), Mood::Gloomy, "three of five lost");
        assert_eq!(mood(&[w, l, w, l, w]), Mood::Bright);
        assert_eq!(mood(&[l, l, l, w, w, w]), Mood::Bright, "only the last five count");
        assert!(too_merry_for_a_low_camp(Gesture::Cheer) && !too_merry_for_a_low_camp(Gesture::Salute));
    }

    #[test]
    fn a_hero_out_keeps_its_place_when_the_camp_changes() {
        let (mut game, _, _) = a_newcomers_camp();
        let far = (game.run.heroes[0].pos.0 + 100.0, game.run.heroes[0].pos.1);
        let now = Instant::now();
        let camp = game.camp.as_mut().unwrap();
        camp.life.hour = 12;
        camp.life.haunts.retain(|h| h.doing == Doing::Hammer);
        camp.life.next_outing = now;
        game.outings_tick(now, far);
        let (haunts, walls) = game.camp_walks();
        let anvil = haunts.iter().position(|h| h.doing == Doing::Hammer).unwrap();
        let life = &mut game.camp.as_mut().unwrap().life;
        assert_eq!(life.heroes[0].out.as_ref().unwrap().slot, Some(0));
        life.change_ways(haunts.clone(), walls);
        assert_eq!(life.heroes[0].out.as_ref().unwrap().slot, Some(anvil), "the anvil, where it is now listed");
        life.change_ways(haunts.into_iter().filter(|h| h.doing != Doing::Hammer).collect(), Walls::open());
        assert_eq!(life.heroes[0].out.as_ref().unwrap().slot, None, "gone");
    }

    #[test]
    fn at_night_one_keeps_watch() {
        let (mut game, _, _) = a_newcomers_camp();
        let far = (game.run.heroes[0].pos.0 + 100.0, game.run.heroes[0].pos.1);
        let now = Instant::now();
        let camp = game.camp.as_mut().unwrap();
        camp.visitors[0].1.class = "warrior".into();
        camp.life.hour = 23;
        camp.life.made = now;
        camp.life.next_outing = now;
        game.outings_tick(now, far);
        let out = game.camp.as_ref().unwrap().life.heroes[0].out.as_ref().expect("Brom out");
        assert_eq!(out.haunt.doing.group(), Group::Watch);
    }

    /// At night the fire burns low once, a hero walks to its edge and feeds it, and it is lit again.
    #[test]
    fn at_night_the_fire_burns_low_once_and_is_fed() {
        let (mut game, _, _) = a_newcomers_camp();
        let far = (game.run.heroes[0].pos.0 + 500.0, game.run.heroes[0].pos.1);
        let now = Instant::now();
        let camp = game.camp.as_mut().unwrap();
        camp.life.hour = 23;
        camp.life.made = now;
        camp.life.next_outing = now + Duration::from_secs(3600);
        camp.life.fire.lows_at = Some(now);
        game.outings_tick(now, far);
        let fire = &game.camp.as_ref().unwrap().life.fire;
        assert!(fire.low && fire.burned && fire.tell == Some(true));
        let feed_from = fire.feed_from.unwrap();
        game.camp.as_mut().unwrap().life.fire.tell = None;
        game.outings_tick(feed_from, far);
        let life = &game.camp.as_ref().unwrap().life;
        let o = life.heroes[0].out.as_ref().expect("Brom goes to feed it");
        assert!(o.feeds && o.haunt.doing == Doing::Pray, "to the fire's edge");
        let there = o.leg.set_off + Duration::from_secs_f32(o.leg.takes + 0.1);
        game.outings_tick(there, far);
        game.outings_tick(there + Duration::from_secs_f32(FEED_TAKES + 0.1), far);
        let life = &game.camp.as_ref().unwrap().life;
        assert!(!life.fire.low && life.fire.tell == Some(false), "fed");
        assert!(!life.heroes[0].out.as_ref().unwrap().feeds);
        let later = there + Duration::from_secs(600);
        game.camp.as_mut().unwrap().life.fire.tell = None;
        game.outings_tick(later, far);
        assert_eq!(game.camp.as_ref().unwrap().life.fire.tell, None, "once a visit");
    }

    /// The four who light the fire with a spell, and with which; the rest tend it by hand.
    #[test]
    fn casters_of_fire_and_light_relight_it_with_a_spell() {
        assert_eq!(kindling("warlock"), Some((348, false)));
        assert_eq!(kindling("mage"), Some((2120, true)));
        assert_eq!(kindling("priest"), Some((14914, false)));
        assert_eq!(kindling("paladin"), Some((25912, false)));
        assert_eq!(kindling("warrior"), None);
    }

    #[test]
    fn nobody_sets_off_from_beside_the_player() {
        let (mut game, _, _) = a_newcomers_camp();
        let me = game.run.heroes[0].pos;
        let now = Instant::now();
        game.camp.as_mut().unwrap().life.next_outing = now;
        assert!(game.outings_tick(now, me).is_empty(), "Brom stays, the player by him");
    }

    #[test]
    fn a_run_ended_is_the_heros_news() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).unwrap();
        let saves = crate::save::Saves::in_memory();
        let aria = saves.create("Aria", "paladin", "human", 0, [0; 5]).unwrap();
        let run = |won| crate::save::FinishedRun { won, first: true, opens_zone: won, ..Default::default() };
        saves.finish_run(&content, aria.guid, &run(false));
        assert_eq!(saves.take_news(aria.guid), Some(CampNews::Lost { zone: 0 }), "the run's news replaces the newcomer's");
        assert_eq!(saves.take_news(aria.guid), None, "taken once");
        saves.finish_run(&content, aria.guid, &run(true));
        assert_eq!(saves.take_news(aria.guid), Some(CampNews::Won { first_zone_win: true, zone: 0 }));
        saves.finish_run(&content, aria.guid, &run(true));
        assert_eq!(saves.take_news(aria.guid), Some(CampNews::Won { first_zone_win: false, zone: 0 }), "the second time is no first");
        saves.finish_run(&content, aria.guid, &crate::save::FinishedRun { first: false, ..run(false) });
        assert_eq!(saves.take_news(aria.guid), None, "a run's later payment is no news");
    }

    /// A hero says the lines of the time period it was pulled in from and the lines of any time,
    /// never another period's; an exchange's side naming a period is spoken only by a hero of it.
    #[test]
    fn a_hero_speaks_of_its_own_time_period() {
        use esurv_content::TimePeriod;
        let (_, content, brom) = a_newcomers_camp();
        let line = |time_period| CampLine {
            temperament: "stalwart".into(),
            when: LineWhen::Idle,
            race: Some("dwarf".into()),
            class: None,
            time_period,
            text: "Aye.".into(),
        };
        let from = |time_period| SavedHero { time_period, ..brom.clone() };
        for hero in TimePeriod::ALL {
            assert!(fits(&line(None), &from(hero), "stalwart", LineWhen::Idle), "a line of any time fits {hero:?}");
            for said in TimePeriod::ALL {
                assert_eq!(fits(&line(Some(said)), &from(hero), "stalwart", LineWhen::Idle), said == hero, "{said:?} line, {hero:?} hero");
                let side = Speaker { time_period: Some(said), ..Default::default() };
                assert_eq!(speaks(&side, &from(hero)), said == hero);
            }
        }
        // the shipped lines: a hero of the Present never says a Past line of its race
        let present = from(TimePeriod::Present);
        let said: Vec<&CampLine> = content.camp_chatter.lines.iter().filter(|l| fits(l, &present, "stalwart", LineWhen::Idle)).collect();
        assert!(!said.is_empty());
        assert!(said.iter().all(|l| l.time_period.is_none_or(|p| p == TimePeriod::Present)));
        let past = from(TimePeriod::Past);
        assert!(content.camp_chatter.lines.iter().any(|l| l.time_period == Some(TimePeriod::Past) && ["stalwart", "grim"].iter().any(|t| fits(l, &past, t, l.when))), "a dwarf of the Past has its own lines");
    }

    /// Every way a hero walks, from each tent to each place it goes and back and from each place
    /// on to another of its kind, is found and kept, the same way each time. Between the places,
    /// always over the camp's own ground (round all in the way and by the stairs, as the guides
    /// walk); a tent's place the world's ground walls in keeps the way round the camp's pieces
    /// (named in the output). As the camp is made, the ways between its places are worked out on a
    /// thread of their own, a hero walking round the camp's pieces till each arrives, never kept
    /// waiting for one. Reads the player's own WoW data (`WOW_DATA`).
    #[test]
    fn each_heros_way_is_found_and_kept() {
        use super::super::walk::floor_near;
        use crate::game::camp::NEUTRAL_HORDE_CAMP;
        let Some(dir) = benilla_formats::wow_data() else {
            eprintln!("no WoW data: the heroes' ways not walked");
            return;
        };
        let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let data = Arc::new(crate::ground::GameData::open(&dir, &content).unwrap());
        for place in [NEUTRAL_ALLIANCE_CAMP, NEUTRAL_HORDE_CAMP] {
            let (own, zone) = camp_content(&content, place, CAMP_HOUR).unwrap();
            let (_host, inputs) = std::sync::mpsc::channel();
            let saves = Arc::new(crate::save::Saves::in_memory());
            let me = saves.create("Aria", "paladin", "human", 0, [0; 5]).unwrap();
            let mut game = Game::new(own, Some(&data), zone, &me, saves, inputs).unwrap();
            game.make_camp(place, me.guid, &[], Default::default());
            let floors = game.camp.as_ref().unwrap().floors.clone().expect("the camp's own ground");
            let (haunts, walls) = game.camp_walks();
            let made = Instant::now();
            let mut life = std::mem::replace(&mut game.camp.as_mut().unwrap().life, Life::new(Vec::new(), None, &[], 0, made));
            let ground = |p: (f32, f32)| super::super::walk::walked_height(&walls.rises, p, |q| game.camp_ground(place, q));
            // the ways between the places come on their own thread, a hero not kept waiting meanwhile
            let between = haunts.iter().flat_map(|h| haunts.iter().filter(|o| o.at != h.at && o.doing.group() == h.doing.group()).map(|o| (h.at, o.at)));
            let between: Vec<((f32, f32), (f32, f32))> = between.collect();
            assert!(!life.ways.coming.is_empty(), "{}: the ways between the places being worked out", place.camp);
            while !life.ways.coming.is_empty() {
                let (a, b) = between[made.elapsed().subsec_millis() as usize % between.len()];
                let t = Instant::now();
                assert!(hero_way(&walls, Some(&floors), &mut life.ways, ground, a, b).is_some());
                assert!(t.elapsed().as_secs_f32() < 0.05, "{}: kept waiting {:.2} s", place.camp, t.elapsed().as_secs_f32());
                assert!(made.elapsed().as_secs() < 120, "{}: the ways still coming", place.camp);
            }
            let worked_out = made.elapsed().as_secs_f32();
            assert!(between.iter().all(|&(a, b)| life.ways.kept.contains_key(&way_key(a, b))), "{}: a way between places not over the camp's own ground", place.camp);
            // and from each tent there is
            let stand = |p: (f32, f32)| floor_near(&floors, p, ground(p)).unwrap_or_else(|| ground(p));
            let tents: Vec<(f32, f32)> = (0..place.layout.plots.len()).filter_map(|i| place.hero_stand(i)).map(|(at, _)| at).collect();
            let pairs = haunts.iter().flat_map(|h| tents.iter().flat_map(|&t| [(t, h.at), (h.at, t)]));
            let pairs: Vec<((f32, f32), (f32, f32))> = between.iter().copied().chain(pairs).collect();
            let mut walled_in = Vec::new();
            for &(a, b) in &pairs {
                let way = hero_way(&walls, Some(&floors), &mut life.ways, ground, a, b).unwrap_or_else(|| panic!("{}: no way from {a:?} to {b:?}", place.camp));
                let end = *way.last().unwrap();
                assert_eq!((end[0], end[1]), b, "{}: it ends where it was going", place.camp);
                let key = way_key(a, b);
                assert!(life.ways.kept.contains_key(&key) || life.ways.none.contains(&key), "{}: kept, not worked out again", place.camp);
                assert_eq!(hero_way(&walls, Some(&floors), &mut life.ways, ground, a, b), Some(way), "{}: the same way again", place.camp);
                if walls.guide_route(&floors, [a.0, a.1, stand(a)], b, stand(b)).is_none() {
                    let tent = tents.iter().position(|&t| t == a || t == b).unwrap_or_else(|| panic!("{}: from {a:?} to {b:?} not over the camp's own ground", place.camp));
                    if !walled_in.contains(&tent) {
                        walled_in.push(tent);
                    }
                }
            }
            walled_in.sort();
            println!("{}: {} ways between its places worked out in {worked_out:.1} s; {} ways in all; tents walled in, walked round the camp's pieces: {walled_in:?}", place.camp, between.len(), pairs.len());
        }
    }
}
