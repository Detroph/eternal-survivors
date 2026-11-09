//! The guide's walk round the camp (plans/hub-wren-tour-draft.md): she leads a
//! hero past each person and thing of the camp, says her lines at each stop in speech bubbles,
//! then the gold line of what to try there, waits till it is tried (or the hero asks for the next
//! stop), and at the end writes the warband in as a Recruit. Between walks she waits by the fire,
//! and talking to her opens the Guide. In the neutral camp the warband's first hero is welcomed
//! by Chromie first, walked round by its side's guide (Wren or Rezza) and handed back to Chromie,
//! who sends it on its first run (walks.rs).

mod walks;

use std::time::{Duration, Instant};

use super::walk::{floor_near, walked_height, Walls};
use super::{CampChoice, CampPlace, Faction, Keeper, Service, Spot, REACH};
use crate::game::{dist, Game};
use crate::packets::{self, EMOTE_TALK, EMOTE_WAVE};
use crate::save::{AchievementFacts, SavedHero};
use esurv_protocol::ToClient;
use crate::update::Speeds;
use crate::wire::Packet;

/// The achievement the whole walk earns (`achievements.ron`), its title "Recruit".
const RECRUIT: &str = "recruit";
/// The fire stop: she sets off this long after her lines, seconds (plans/hub-people-and-tutorial.md).
const FIRE_WAIT: f32 = 3.0;
/// Her pace (5 yards a second, with the running animation), yards a second.
const WALK_SPEED: f32 = 5.0;
/// A walk shorter than this she takes at the hero's own walking pace, walking; the camp's
/// reach, yards.
const SHORT_WALK: f32 = REACH;
/// At the tent stop she stands this far to the side of where the hero stands before it, yards.
const BESIDE_THE_HERO: f32 = 2.5;
/// She turns to what a line tells of only when it lies more than this off her facing (the sine of
/// the angle, about 6 degrees), so she does not twitch.
const TURN_AT: f32 = 0.1;
/// The hero this far from her has wandered off, yards (the same plan's "about 25 yards").
const WANDERED: f32 = 25.0;
/// Where she stands before a person she tells of: this far in front of them, yards.
const IN_FRONT: f32 = 3.5;
/// A new hero arrives this far in front of Chromie, facing her, yards.
const BEFORE_CHROMIE: f32 = 4.0;
/// Chromie's welcome starts this long after the camp is made, the hero's client in the world by
/// then, seconds.
pub(super) const WELCOME_AFTER: f32 = 6.0;

/// Something a hero did that a stop may wait for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Did {
    /// Talked to this person of the camp.
    Talked(Keeper),
    /// Talked to Chromie.
    Chromie,
    /// Opened this window of the game's own (`ToClient::CampOpen`).
    Opened(&'static str),
}

/// Whom or what she turns to at a stop: what she is telling of.
#[derive(Clone, Copy)]
enum Toward {
    Person(Keeper),
    /// Two people side by side: she stands before the both of them.
    Between(Keeper, Keeper),
    /// Chromie, beside where the hero stands before her.
    Chromie,
}

/// What ends a stop, besides the hero asking "Where to next?".
enum Ends {
    /// This long after her lines, seconds.
    After(f32),
    /// The hero doing one of these.
    By(&'static [Did]),
}

/// Which part of the walk a stop is: meeting the hero at Chromie's and handing it back to her
/// are only for a walk Chromie started.
#[derive(Clone, Copy, PartialEq)]
enum Part {
    Meet,
    Walk,
    Back,
}

/// A stop of the walk: the corners she walks by on her way there (round the benches, the
/// Records' ramp and the Marshal's pavilion, not through them), where she stands (none:
/// before whom she tells of), who says what there (in order, a bubble each), what Chromie says
/// after, who waves as she arrives, the gold line, and what ends it.
struct Stop {
    via: &'static [Spot],
    at: Option<Spot>,
    faces: Toward,
    lines: &'static [(Keeper, &'static str)],
    chromie: &'static [&'static str],
    waves: Option<Keeper>,
    try_it: &'static str,
    ends: Ends,
    part: Part,
}

/// The guide's words in her talk window and between stops.
pub(super) struct Words {
    first: &'static str,
    show_me: &'static str,
    own_way: &'static str,
    skipped: &'static str,
    next: &'static str,
    again: &'static str,
    enough_option: &'static str,
    enough: &'static str,
    wandered_off: &'static str,
    seen_off: &'static str,
    written_in: &'static str,
    after: &'static str,
    read_guide: &'static str,
    walk_again: &'static str,
    farewell: &'static str,
}

/// A camp's walk: its stops and its guide's words.
pub(super) struct Walk {
    stops: &'static [Stop],
    words: Words,
}

/// The walk of the camp at `place`: its side's.
fn walk_of(place: CampPlace) -> &'static Walk {
    match place.faction {
        Faction::Horde => &walks::REZZA_AT_THE_CAVERNS,
        Faction::Alliance => &walks::WREN_AT_THE_CAVERNS,
    }
}

/// Where a new hero arrives in the neutral camp: in front of Chromie, facing her.
pub(super) fn before_chromie(place: CampPlace) -> Option<((f32, f32), f32)> {
    let s = place.layout.chromie?.0;
    let ((x, y), f) = (place.at(s), place.facing(s));
    Some(((x + BEFORE_CHROMIE * f.cos(), y + BEFORE_CHROMIE * f.sin()), f + std::f32::consts::PI))
}

impl Stop {
    /// Where she stands at this stop, and the world facing she turns to there.
    fn stand_at(&self, place: CampPlace) -> Option<((f32, f32), f32)> {
        let layout = place.layout;
        let keeper = |k: Keeper| layout.keepers.iter().find(|(p, _)| *p == k).map(|(_, s)| *s);
        let front = |s: Spot| {
            let ((x, y), f) = (place.at(s), place.facing(s));
            (x + IN_FRONT * f.cos(), y + IN_FRONT * f.sin())
        };
        let middle = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let toward = match self.faces {
            Toward::Person(k) => place.at(keeper(k)?),
            Toward::Between(a, b) => middle(place.at(keeper(a)?), place.at(keeper(b)?)),
            Toward::Chromie => place.at(layout.chromie?.0),
        };
        let at = match (self.at, self.faces) {
            (Some(s), _) => place.at(s),
            // beside where the hero stands before Chromie
            (None, Toward::Chromie) => {
                let ((x, y), facing) = before_chromie(place)?;
                (x - BESIDE_THE_HERO * facing.sin(), y + BESIDE_THE_HERO * facing.cos())
            }
            (None, Toward::Person(k)) => front(keeper(k)?),
            (None, Toward::Between(a, b)) => middle(front(keeper(a)?), front(keeper(b)?)),
        };
        Some((at, (toward.1 - at.1).atan2(toward.0 - at.0)))
    }
}

impl Walk {
    /// The stop after `stop` (none: the first) on a walk Chromie started or not: the meeting and
    /// the hand-back only on hers.
    fn after(&self, stop: Option<usize>, from_chromie: bool) -> Option<usize> {
        let from = stop.map_or(0, |s| s + 1);
        (from..self.stops.len()).find(|&i| match self.stops[i].part {
            Part::Walk => true,
            Part::Meet | Part::Back => from_chromie,
        })
    }

    /// Whether `stop` is the walk's last own stop: the one that writes the warband in.
    fn last_walk(&self, stop: usize) -> bool {
        self.stops.iter().rposition(|s| s.part == Part::Walk) == Some(stop)
    }
}

/// `ESURV_RP_DEMO=1`, a development aid: the walk goes on by itself (`Game::demo_beside`).
fn demo() -> bool {
    std::env::var_os("ESURV_RP_DEMO").is_some()
}

/// In the demo, how long the hero stands at a stop after her lines before it is done, seconds.
const DEMO_WAIT: f32 = 3.0;

/// `ESURV_WREN_TOUR_FROM=<stop>,<seconds>`, a development aid: Wren stands at that stop (0 the
/// first) as the camp is made and sets off for the next `seconds` later, to see one leg of the
/// walk (Chromie's, in the neutral camp).
pub(super) fn walk_from(hero: u32, place: CampPlace) -> Option<(Leg, Tour)> {
    let aid = std::env::var("ESURV_WREN_TOUR_FROM").ok()?;
    let (stop, wait) = aid.split_once(',')?;
    let stops = walk_of(place).stops;
    let stop: usize = stop.trim().parse().ok().filter(|s| s + 1 < stops.len())?;
    let wait: f32 = wait.trim().parse().ok()?;
    let (at, _) = stops[stop].stand_at(place)?;
    let set_off = Instant::now() + Duration::from_secs_f32(wait);
    let from_chromie = place.layout.chromie.is_some();
    Some((Leg::still(at), Tour { stop, lines_end: Some(set_off), done: true, from_chromie, ..Tour::new(hero) }))
}

/// What an option of the guide's or Chromie's talk window does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum TourChoice {
    /// "Show me around the camp." / "Walk me round the camp again."
    Start,
    /// "I know my way around."
    Skip,
    /// "Where to next?"
    Next,
    /// "What should I do here?": the gold line again.
    Again,
    /// "That's enough for now, thank you."
    Enough,
    /// Chromie's "explain everything again": her welcome and the whole walk again.
    FromChromie,
    /// Chromie's "Send me through the portal.": the first run.
    SendMe,
}
/// One walk (Wren's, or a hero's about the camp): from where it stood by the corners in `points`
/// to the last, set off at `set_off`, `takes` seconds.
#[derive(Clone, Debug)]
pub(super) struct Leg {
    pub(super) points: Vec<(f32, f32)>,
    /// The height at each point, where the walk was sent with its own (none: on the ground).
    pub(super) heights: Vec<f32>,
    pub(super) set_off: Instant,
    pub(super) takes: f32,
}

impl Leg {
    /// Standing at `at`.
    pub(super) fn still(at: (f32, f32)) -> Self {
        Leg { points: vec![at], heights: Vec::new(), set_off: Instant::now(), takes: 0.0 }
    }

    /// The stretch it is on at `now` (its first point's index) and how far along it, 0 to 1.
    fn along(&self, now: Instant) -> (usize, f32) {
        let t = if self.takes <= 0.0 { 1.0 } else { (now.duration_since(self.set_off).as_secs_f32() / self.takes).min(1.0) };
        let length: f32 = self.points.windows(2).map(|p| dist(p[0], p[1])).sum();
        let mut left = t * length;
        for (k, pair) in self.points.windows(2).enumerate() {
            let d = dist(pair[0], pair[1]);
            if left <= d && d > 0.0 {
                return (k, left / d);
            }
            left -= d;
        }
        (self.points.len() - 1, 0.0)
    }

    /// Where it is at `now`.
    pub(super) fn at(&self, now: Instant) -> (f32, f32) {
        let (k, f) = self.along(now);
        let (a, b) = (self.points[k], *self.points.get(k + 1).unwrap_or(&self.points[k]));
        (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f)
    }

    /// Its height at `now`, if it was sent with its own.
    pub(super) fn height(&self, now: Instant) -> Option<f32> {
        let (k, f) = self.along(now);
        let a = *self.heights.get(k)?;
        Some(a + (self.heights.get(k + 1).unwrap_or(&a) - a) * f)
    }

    pub(super) fn ended(&self, now: Instant) -> bool {
        now.duration_since(self.set_off).as_secs_f32() >= self.takes
    }
}

/// A walk going on: whose, the stop she is at or walking to (past the last: walking home), and
/// how that stop goes.
#[derive(Clone, Debug)]
pub(super) struct Tour {
    hero: u32,
    stop: usize,
    /// When her lines there end (none: not there yet).
    lines_end: Option<Instant>,
    /// The hero did what the stop asks (its window closed after), or asked for the next.
    done: bool,
    /// The hero opened what the stop asks: done once it is closed.
    waiting: bool,
    /// She called the hero back once already.
    called: bool,
    /// Chromie started it: the guide meets the hero at hers and hands it back to her at the end.
    from_chromie: bool,
}

impl Tour {
    /// Hero `hero`'s walk, from its start.
    pub(super) fn new(hero: u32) -> Self {
        Tour { hero, stop: 0, lines_end: None, done: false, waiting: false, called: false, from_chromie: false }
    }
}

impl Game {
    /// Where Wren stands or walks now.
    fn wren_at(&self, now: Instant) -> Option<(f32, f32)> {
        Some(self.camp.as_ref()?.wren.at(now))
    }

    /// This camp's walk.
    fn camp_walk(&self) -> &'static Walk {
        walk_of(self.camp.as_ref().map_or(super::NEUTRAL_ALLIANCE_CAMP, |c| c.place))
    }

    /// A camp person's speech bubble, with the talking animation (everyone who speaks); the
    /// pointers in `words` (`{Area(1537)}`) shown as the client's names.
    fn say(&self, who: Keeper, words: &str) -> [Packet; 2] {
        let guid = self.keeper_guid(who);
        [packets::monster_say(guid, self.keeper_name(who), &self.run.content.shown(words)), packets::emote(guid, EMOTE_TALK)]
    }

    /// Chromie's speech bubble, with the talking animation, its pointers shown as names.
    fn chromie_say(&self, words: &str) -> [Packet; 2] {
        let guid = super::super::unit_guid(super::CHROMIE_ENTRY, 1);
        [packets::monster_say(guid, "Chromie", &self.run.content.shown(words)), packets::emote(guid, EMOTE_TALK)]
    }

    /// Wren walks from where she is by the corners `via` to `to` (at 5 yd/s, the
    /// running animation always; a short walk at the hero's walking pace, walking), turning to
    /// `facing` there.
    fn wren_walk(&mut self, via: &[Spot], to: (f32, f32), facing: Option<f32>, now: Instant) -> Option<Packet> {
        let place = self.camp.as_ref()?.place;
        let from = self.wren_at(now)?;
        let start = [from.0, from.1, self.wren_height(now)?];
        let ends: Vec<(f32, f32)> = via.iter().map(|s| place.at(*s)).chain([to]).collect();
        let way = self.guide_way(place, start, &ends);
        let points: Vec<(f32, f32)> = way.iter().map(|p| (p[0], p[1])).collect();
        let length: f32 = points.windows(2).map(|p| dist(p[0], p[1])).sum();
        let short = length < SHORT_WALK;
        let takes = length / if short { Speeds::DEFAULT.walk } else { WALK_SPEED };
        let path = way[1..].to_vec();
        let heights = way.iter().map(|p| p[2]).collect();
        self.camp.as_mut()?.wren = Leg { points, heights, set_off: now, takes };
        let spline = self.next_spline;
        self.next_spline = self.next_spline.wrapping_add(1);
        // already there: a step in place, to turn
        let ms = ((takes * 1000.0).round() as u32).max(1);
        Some(packets::monster_move_facing(self.keeper_guid(Keeper::Wren), start, spline, &path, ms, !short, facing))
    }

    /// The height Wren stands or walks at now: on the ground she was walked over, else the camp's.
    fn wren_height(&self, now: Instant) -> Option<f32> {
        let camp = self.camp.as_ref()?;
        if let Some(z) = camp.wren.height(now) {
            return Some(z);
        }
        let (place, at) = (camp.place, camp.wren.at(now));
        let (_, walls) = self.camp_walks();
        Some(self.guide_ground(place, &walls, at))
    }

    /// The height the guide stands at at `at`: on the camp's own ground, the floor nearest the
    /// world's ground or the troll shop's there; without the game's data, that.
    fn guide_ground(&self, place: CampPlace, walls: &Walls, at: (f32, f32)) -> f32 {
        let ground = walked_height(&walls.rises, at, |p| self.camp_ground(place, p));
        let floors = self.camp.as_ref().and_then(|c| c.floors.as_ref());
        floors.and_then(|f| floor_near(f, at, ground)).unwrap_or(ground)
    }

    /// The way the guide walks from `from` by each of `ends` in turn: over the camp's own ground round all in the
    /// way and by the stairs ([`Walls::guide_route`]); without the game's data, round what stands
    /// in the camp on the world's ground. The points the client draws her line through, with
    /// their heights, `from` first.
    fn guide_way(&self, place: CampPlace, from: [f32; 3], ends: &[(f32, f32)]) -> Vec<[f32; 3]> {
        let (_, walls) = self.camp_walks();
        let floors = self.camp.as_ref().and_then(|c| c.floors.clone());
        let ground = |p: (f32, f32)| walked_height(&walls.rises, p, |q| self.camp_ground(place, q));
        let mut way = vec![from];
        for &to in ends {
            let here = *way.last().expect("the way has its start");
            let on_floors = floors.as_ref().and_then(|f| walls.guide_route(f, here, to, self.guide_ground(place, &walls, to)));
            if let Some(r) = on_floors {
                way.extend(r.into_iter().skip(1));
                continue;
            }
            // over the world's ground, a point every few yards, round what stands in the camp if
            // there is a way round
            let corners = walls.route((here[0], here[1]), to).unwrap_or_else(|| vec![(here[0], here[1]), to]);
            for pair in corners.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let steps = (dist(a, b) / super::super::PATH_STEP).ceil().max(1.0) as usize;
                way.extend((1..=steps).map(|k| {
                    let t = k as f32 / steps as f32;
                    let at = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                    [at.0, at.1, ground(at)]
                }));
            }
        }
        way
    }

    /// Wren standing at `at` turning to `facing`, a world angle.
    fn wren_turn(&mut self, at: (f32, f32), facing: f32) -> Option<Packet> {
        let now = self.camp.as_ref()?.ticked;
        let p = [at.0, at.1, self.wren_height(now)?];
        let spline = self.next_spline;
        self.next_spline = self.next_spline.wrapping_add(1);
        Some(packets::monster_move_facing(self.keeper_guid(Keeper::Wren), p, spline, &[p], 1, false, Some(facing)))
    }

    /// Where what the guide's line tells of stands: whoever answers her, else the first person or
    /// thing of the camp the line names.
    fn told_of(&self, place: CampPlace, who: Keeper, words: &str) -> Option<(f32, f32)> {
        let layout = place.layout;
        let keeper_at = |k: Keeper| layout.keepers.iter().find(|(p, _)| *p == k).map(|(_, s)| place.at(*s));
        if who != Keeper::Wren {
            return keeper_at(who);
        }
        let faction = self.camp_faction();
        let mut named: Vec<(usize, (f32, f32))> = Vec::new();
        for k in super::ALL_KEEPERS.into_iter().filter(|&k| k != Keeper::Wren) {
            let first = k.who(faction).and_then(|(name, _, _)| name.split(' ').next());
            if let (Some(i), Some(at)) = (first.and_then(|n| words.find(n)), keeper_at(k)) {
                named.push((i, at));
            }
        }
        for s in Service::ALL {
            let at = layout.services.iter().find(|(t, _)| *t == s).map(|(_, sp)| place.at(*sp));
            if let (Some(i), Some(at)) = (words.find(s.name(faction)), at) {
                named.push((i, at));
            }
        }
        if let (Some(i), Some(c)) = (words.find("Chromie"), layout.chromie) {
            named.push((i, place.at(c.0)));
        }
        named.into_iter().min_by_key(|(i, _)| *i).map(|(_, at)| at)
    }

    /// Whether the warband walked the whole camp with Wren already.
    pub(super) fn recruited(&self) -> bool {
        self.saves.achievements(&self.run.content).earned.contains_key(RECRUIT)
    }

    /// Hero `hero`'s walk sets off, Chromie's or the guide's own: she walks to its first stop.
    fn start_walk(&mut self, hero: u32, from_chromie: bool, now: Instant) -> Option<Packet> {
        let walk = self.camp_walk();
        let first = walk.after(None, from_chromie)?;
        self.camp.as_mut()?.tour = Some(Tour { stop: first, from_chromie, ..Tour::new(hero) });
        let (to, facing) = walk.stops[first].stand_at(self.camp.as_ref()?.place)?;
        self.wren_walk(walk.stops[first].via, to, Some(facing), now)
    }

    /// Chromie welcomes hero `hero`, saved as `saved`, from `from` on (the
    /// warband's first hero, or one who asks her to explain everything again): her first words
    /// for its race and time period, the rest of her words a bubble apart, then she calls its
    /// side's guide over, who meets it at hers and walks it round.
    pub(super) fn chromie_welcome(&mut self, hero: u32, saved: Option<&SavedHero>, from: Instant) {
        let Some(place) = self.camp.as_ref().map(|c| c.place) else {
            return;
        };
        let first = saved.map_or(walks::CHROMIE_FIRST_ANY, |s| walks::chromie_first(&s.race, s.time_period));
        let calls = if place.faction == Faction::Horde { walks::CALLS_REZZA } else { walks::CALLS_WREN };
        let mut at = from;
        let mut said = Vec::new();
        for words in [&first].into_iter().chain(walks::CHROMIE_WELCOME).chain([&calls]) {
            said.extend(self.chromie_say(words).into_iter().map(|p| (at, p)));
            at += super::line_gap(words);
        }
        if let Some(camp) = &mut self.camp {
            camp.later.extend(said);
            camp.lesson = Some((hero, at));
            // the heroes at their tents hold their tongues meanwhile
            camp.life.talked(at);
        }
    }

    /// Chromie greets a later new hero, saved as `saved`, at `from`: only her first words for its
    /// race and time period, with no lesson after.
    pub(super) fn chromie_greet(&mut self, saved: Option<&SavedHero>, from: Instant) {
        let first = saved.map_or(walks::CHROMIE_FIRST_ANY, |s| walks::chromie_first(&s.race, s.time_period));
        let said = self.chromie_say(first).map(|p| (from, p));
        if let Some(camp) = &mut self.camp {
            camp.later.extend(said);
            camp.life.talked(from);
        }
    }

    /// The demo puts hero `hero` beside the guide at `stop`, facing what she faces.
    fn demo_beside(&mut self, hero: u32, stop: &Stop, place: CampPlace) -> Option<Packet> {
        let ((x, y), f) = stop.stand_at(place)?;
        let at = (x + BESIDE_THE_HERO * f.sin(), y - BESIDE_THE_HERO * f.cos());
        let z = self.camp_ground(place, at);
        let guid = self.players.iter().find(|q| q.hero == hero).map(|q| q.guid())?;
        let h = self.run.heroes.iter_mut().find(|h| h.id == hero)?;
        h.pos = at;
        h.height = z;
        h.facing = f;
        self.next_teleport += 1;
        Some(packets::teleport(guid, self.next_teleport, [at.0, at.1, z], f))
    }

    /// The hero talks to the guide: her window before a walk, during one, or after the warband
    /// walked it all.
    pub(super) fn wren_talk(&mut self, hero: u32) -> Vec<Packet> {
        let Some(now) = self.camp.as_ref().map(|c| c.ticked) else {
            return Vec::new();
        };
        let (Some(at), Some(me)) = (self.wren_at(now), self.run.heroes.iter().find(|h| h.id == hero)) else {
            return Vec::new();
        };
        if dist(at, me.pos) > REACH {
            return Vec::new();
        }
        let guid = self.keeper_guid(Keeper::Wren);
        let walk = self.camp_walk();
        let words = &walk.words;
        let tour = self.camp.as_ref().and_then(|c| c.tour.clone()).filter(|t| t.stop < walk.stops.len());
        let option = |label: &str, choice| (label.to_string(), choice);
        if let Some(t) = tour {
            // during a walk: the stop's first words again
            let said = walk.stops[t.stop].lines[0].1;
            let options = vec![
                option(words.next, CampChoice::Tour(TourChoice::Next)),
                option(words.again, CampChoice::Tour(TourChoice::Again)),
                option(words.enough_option, CampChoice::Tour(TourChoice::Enough)),
            ];
            return self.camp_window(guid, said.into(), options);
        }
        if self.recruited() {
            let options = vec![
                option(words.read_guide, CampChoice::Open("guide")),
                option(words.walk_again, CampChoice::Tour(TourChoice::Start)),
                option(words.farewell, CampChoice::Goodbye),
            ];
            return self.camp_window(guid, words.after.into(), options);
        }
        let options = vec![
            option(words.show_me, CampChoice::Tour(TourChoice::Start)),
            option(words.own_way, CampChoice::Tour(TourChoice::Skip)),
        ];
        self.camp_window(guid, words.first.into(), options)
    }

    /// The hero talks to Chromie: at the end of her lesson, she sends it through the portal; else
    /// she offers a new hero, her words on this and that, and the whole lesson again.
    pub(super) fn chromie_talk(&mut self, hero: u32, page: Option<&'static str>) -> Vec<Packet> {
        let guid = super::super::unit_guid(super::CHROMIE_ENTRY, 1);
        let walk = self.camp_walk();
        let at_the_end = self
            .camp
            .as_ref()
            .and_then(|c| c.tour.as_ref())
            .is_some_and(|t| t.hero == hero && walk.stops.get(t.stop).is_some_and(|s| s.part == Part::Back) && t.lines_end.is_some());
        let option = |label: &str, choice| (label.to_string(), choice);
        if at_the_end && page.is_none() {
            self.tour_did(hero, Did::Chromie);
            let options = vec![option(walks::SEND_ME, CampChoice::Tour(TourChoice::SendMe)), option("Not yet.", CampChoice::Goodbye)];
            return self.camp_window(guid, walks::CHROMIE_SEND.into(), options);
        }
        if let Some((_, words)) = page.and_then(|p| walks::CHROMIE_PAGES.iter().find(|(id, _)| *id == p)) {
            let options = vec![option("Back.", CampChoice::Chromie(None)), option("Goodbye.", CampChoice::Goodbye)];
            return self.camp_window(guid, (*words).into(), options);
        }
        let mut options = vec![option(walks::RECRUIT_ONE, CampChoice::Open("recruit"))];
        options.extend(walks::CHROMIE_ASKS.iter().map(|(label, id)| option(label, CampChoice::Chromie(Some(id)))));
        options.push(option(walks::TELL_AGAIN, CampChoice::Tour(TourChoice::FromChromie)));
        options.push(option("Goodbye.", CampChoice::Goodbye));
        self.camp_window(guid, walks::CHROMIE_HELLO.into(), options)
    }

    /// The hero picked `choice` in the guide's or Chromie's window.
    pub(super) fn tour_pick(&mut self, hero: u32, choice: TourChoice) -> Vec<Packet> {
        let Some(camp) = &mut self.camp else {
            return Vec::new();
        };
        let now = camp.ticked;
        let walk = walk_of(camp.place);
        let mut out = Vec::new();
        match choice {
            TourChoice::Start => out.extend(self.start_walk(hero, false, now)),
            TourChoice::Skip => out.extend(self.say(Keeper::Wren, walk.words.skipped)),
            TourChoice::Next => {
                if let Some(t) = camp.tour.as_mut().filter(|t| t.lines_end.is_some()) {
                    t.done = true;
                }
            }
            TourChoice::Again => {
                if let Some(t) = camp.tour.as_ref().filter(|t| t.stop < walk.stops.len()) {
                    out.push(packets::raid_warning(walk.stops[t.stop].try_it));
                }
            }
            TourChoice::Enough => {
                out.extend(self.say(Keeper::Wren, walk.words.enough));
                out.extend(self.wren_home(now));
            }
            // a walk going on stops: the lesson starts over
            TourChoice::FromChromie => {
                if camp.tour.take().is_some() {
                    out.extend(self.wren_home(now));
                }
                let saved = self.saved(hero).cloned();
                self.chromie_welcome(hero, saved.as_ref(), now);
            }
            TourChoice::SendMe => {
                if let Some(leave) = self.camp_run(hero) {
                    out.extend(self.lines(vec![(hero, leave)]).into_iter().map(|(_, p)| p));
                }
            }
        }
        out
    }

    /// The guide walks back to her place by the fire, the walk over.
    fn wren_home(&mut self, now: Instant) -> Option<Packet> {
        let len = self.camp_walk().stops.len();
        let camp = self.camp.as_mut()?;
        let home = camp.place.layout.keepers.iter().find(|(k, _)| *k == Keeper::Wren)?.1;
        let (to, facing) = (camp.place.at(home), camp.place.facing(home));
        if let Some(t) = &mut camp.tour {
            t.stop = len;
        }
        self.wren_walk(&[], to, Some(facing), now)
    }

    /// The hero did `did`: the stop waiting for it is done once its window is closed.
    pub(super) fn tour_did(&mut self, hero: u32, did: Did) {
        // the heroes at their tents hold their tongues over a window (plans/hub-hero-life.md)
        if let (Some(camp), Did::Opened(_)) = (&mut self.camp, did) {
            camp.life.window_open = true;
        }
        let walk = self.camp_walk();
        let Some(t) = self.camp.as_mut().and_then(|c| c.tour.as_mut()) else {
            return;
        };
        if t.hero != hero || t.lines_end.is_none() {
            return;
        }
        if let Some(Ends::By(dids)) = walk.stops.get(t.stop).map(|s| &s.ends) {
            if dids.contains(&did) {
                t.waiting = true;
            }
        }
    }

    /// The hero closed a window (one of the game's own, or a talk window by its Goodbye): a stop
    /// waiting for it is done.
    pub(in crate::game) fn tour_closed(&mut self, hero: u32) {
        if let Some(camp) = &mut self.camp {
            camp.life.window_open = false;
        }
        if let Some(t) = self.camp.as_mut().and_then(|c| c.tour.as_mut()).filter(|t| t.hero == hero && t.waiting) {
            t.done = true;
        }
    }

    /// The hero sets off on a run during a walk: the guide sees it off.
    pub(super) fn tour_seen_off(&mut self, hero: u32) -> Vec<Packet> {
        let walk = self.camp_walk();
        let Some(camp) = &mut self.camp else {
            return Vec::new();
        };
        if camp.lesson.is_some_and(|(h, _)| h == hero) {
            camp.lesson = None;
        }
        if !camp.tour.take().is_some_and(|t| t.hero == hero) {
            return Vec::new();
        }
        self.say(Keeper::Wren, walk.words.seen_off).to_vec()
    }

    /// The hero being walked round (till she turns home) is told so, and told again once not: its
    /// race portraits shut meanwhile.
    pub(super) fn tell_guided(&mut self) {
        let len = self.camp_walk().stops.len();
        let Some(camp) = &mut self.camp else {
            return;
        };
        let now = camp.tour.as_ref().filter(|t| t.stop < len).map(|t| t.hero);
        let was = std::mem::replace(&mut camp.guided_told, now);
        if was == now {
            return;
        }
        let told: Vec<_> = was.map(|h| (h, ToClient::CampGuided(false))).into_iter().chain(now.map(|h| (h, ToClient::CampGuided(true)))).collect();
        for (h, p) in self.lines(told) {
            self.send_to(h, &[p]);
        }
    }

    /// The walk's own time: Chromie's welcome ends with the guide coming over, the guide reaches a
    /// stop and says her lines, moves on once it is done, calls a hero who wandered off back once,
    /// and writes the warband in at the end.
    pub(super) fn tour_tick(&mut self, now: Instant) -> Vec<Packet> {
        let mut out = Vec::new();
        let walk = self.camp_walk();
        let Some(camp) = &mut self.camp else {
            return out;
        };
        camp.ticked = now;
        // the bubbles and gold lines whose time came
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut camp.later).into_iter().partition(|(at, _)| *at <= now);
        camp.later = later;
        out.extend(due.into_iter().map(|(_, p)| p));
        // Chromie's welcome said: the guide comes over
        if let Some((hero, _)) = camp.lesson.filter(|(_, at)| now >= *at) {
            camp.lesson = None;
            camp.tour = None;
            out.extend(self.start_walk(hero, true, now));
            return out;
        }
        let Some(camp) = &mut self.camp else {
            return out;
        };
        let Some(mut t) = camp.tour.clone() else {
            return out;
        };
        let place = camp.place;
        let (wren, walking) = (camp.wren.at(now), !camp.wren.ended(now));
        // home again: she waves and waits by the fire
        if t.stop >= walk.stops.len() {
            if !walking {
                camp.tour = None;
                out.push(packets::emote(self.keeper_guid(Keeper::Wren), EMOTE_WAVE));
            }
            return out;
        }
        let Some(hero) = self.run.heroes.iter().find(|h| h.id == t.hero).map(|h| h.pos) else {
            return out;
        };
        // on her way to meet the hero at Chromie's it is she who is far, not the hero
        // (the demo's hero stays put between stops)
        if dist(hero, wren) > WANDERED && !t.called && walk.stops[t.stop].part != Part::Meet && !demo() {
            t.called = true;
            out.extend(self.say(Keeper::Wren, walk.words.wandered_off));
        }
        let stop = &walk.stops[t.stop];
        match t.lines_end {
            _ if walking => {}
            None => {
                // there: who waves, then each line a bubble apart, Chromie's after, then the gold line
                if let Some(who) = stop.waves {
                    out.push(packets::emote(self.keeper_guid(who), EMOTE_WAVE));
                }
                let mut at = now;
                let mut said = Vec::new();
                let here = self.wren_at(now).unwrap_or(wren);
                let mut facing = stop.stand_at(place).map_or(0.0, |(_, f)| f);
                for (who, words) in stop.lines {
                    // she turns to whom or what the line tells of
                    if let Some(to) = self.told_of(place, *who, words) {
                        let f = (to.1 - here.1).atan2(to.0 - here.0);
                        if (f - facing).sin().abs() > TURN_AT || (f - facing).cos() < 0.0 {
                            facing = f;
                            said.extend(self.wren_turn(here, f).map(|p| (at, p)));
                        }
                    }
                    said.extend(self.say(*who, words).into_iter().map(|p| (at, p)));
                    at += super::line_gap(words);
                }
                for words in stop.chromie {
                    said.extend(self.chromie_say(words).into_iter().map(|p| (at, p)));
                    at += super::line_gap(words);
                }
                said.push((at, packets::raid_warning(stop.try_it)));
                if let Some(camp) = &mut self.camp {
                    camp.later.extend(said);
                }
                t.lines_end = Some(at);
                // `ESURV_RP_DEMO=1`, a development aid: the hero is put beside her at each stop and does
                // what it asks once her lines are said, to film the walk without a hand on the keys
                if demo() {
                    out.extend(self.demo_beside(t.hero, stop, place));
                }
            }
            Some(end) => {
                let done = match stop.ends {
                    Ends::After(wait) => t.done || now >= end + Duration::from_secs_f32(wait),
                    Ends::By(_) if demo() && stop.part == Part::Back => {
                        if !t.waiting {
                            t.waiting = true;
                            let window = self.chromie_talk(t.hero, None);
                            self.send_to(t.hero, &window);
                        }
                        t.done || now >= end + Duration::from_secs_f32(DEMO_WAIT * 2.0)
                    }
                    Ends::By(_) => t.done || (demo() && now >= end + Duration::from_secs_f32(DEMO_WAIT)),
                };
                if done && now >= end {
                    // the walk's own stops all seen: the warband is written in
                    if walk.last_walk(t.stop) {
                        out.extend(self.say(Keeper::Wren, walk.words.written_in));
                        let facts = AchievementFacts { met: vec![RECRUIT.into()], ..Default::default() };
                        for (h, p) in self.add_facts(vec![(t.hero, facts)], false) {
                            self.send_to(h, &[p]);
                        }
                    }
                    match walk.after(Some(t.stop), t.from_chromie) {
                        Some(next) => {
                            t = Tour { stop: next, lines_end: None, done: false, waiting: false, ..t };
                            let there = walk.stops[next].stand_at(place);
                            if let Some(camp) = &mut self.camp {
                                camp.tour = Some(t.clone());
                            }
                            out.extend(there.and_then(|(to, facing)| self.wren_walk(walk.stops[next].via, to, Some(facing), now)));
                        }
                        None => {
                            if let Some(camp) = &mut self.camp {
                                camp.tour = Some(t);
                            }
                            out.extend(self.wren_home(now));
                            return out;
                        }
                    }
                }
            }
        }
        if let Some(camp) = &mut self.camp {
            camp.tour = Some(t);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::camp::tests::{a_camp, camp_at, walk_to};
    use crate::game::camp::{NEUTRAL_ALLIANCE_CAMP, NEUTRAL_HORDE_CAMP};

    /// What Wren's open window's options do.
    fn her_options(game: &Game) -> Vec<CampChoice> {
        let guid = game.keeper_guid(Keeper::Wren);
        game.camp.as_ref().unwrap().menus.get(&guid).cloned().unwrap_or_default()
    }

    fn stop_of(game: &Game) -> Option<usize> {
        game.camp.as_ref().unwrap().tour.as_ref().map(|t| t.stop)
    }

    #[test]
    fn wren_walks_the_whole_camp_and_writes_the_warband_in_as_a_recruit() {
        let (mut game, me) = a_camp();
        let place = game.camp.as_ref().unwrap().place;
        let wren = game.keeper_guid(Keeper::Wren);
        let home = game.wren_at(game.camp.as_ref().unwrap().ticked).unwrap();
        walk_to(&mut game, home);
        assert_eq!(game.camp_talk(me, wren).len(), 2, "Wren opens a talk window");
        assert_eq!(her_options(&game), [CampChoice::Tour(TourChoice::Start), CampChoice::Tour(TourChoice::Skip)]);
        game.camp_pick(me, wren, 0);
        let mut now = game.camp.as_ref().unwrap().ticked;
        // her own walk: the stops of it, not the meeting at Chromie's nor the hand-back
        for (i, stop) in walk_of(place).stops.iter().enumerate().filter(|(_, s)| s.part == Part::Walk) {
            assert_eq!(stop_of(&game), Some(i), "on to stop {i}");
            // she gets there, the hero with her
            now += Duration::from_secs(60);
            let (at, _) = stop.stand_at(place).unwrap();
            walk_to(&mut game, at);
            let mut heard = game.tour_tick(now);
            now += Duration::from_secs(28);
            heard.extend(game.tour_tick(now));
            assert!(heard.contains(&packets::raid_warning(stop.try_it)), "stop {i} says what to try");
            for (who, words) in stop.lines {
                assert!(heard.contains(&game.say(*who, words)[0]), "stop {i}: {words}");
            }
            // talking to her reaches her where she stands
            assert_eq!(game.camp_talk(me, wren).len(), 2, "Wren is reached at stop {i}");
            match stop.ends {
                Ends::After(_) => {}
                Ends::By(dids) => {
                    assert_eq!(stop_of(&game), Some(i), "stop {i} waits for the hero");
                    game.tour_did(me, dids[0]);
                    game.tour_tick(now + Duration::from_secs(30));
                    assert_eq!(stop_of(&game), Some(i), "stop {i} waits till its window is closed");
                    game.tour_closed(me);
                }
            }
            game.tour_tick(now);
        }
        assert!(game.recruited(), "the whole walk earns Recruit");
        assert_eq!(stop_of(&game), Some(walk_of(place).stops.len()), "she walks home");
        now += Duration::from_secs(60);
        assert!(game.tour_tick(now).contains(&packets::emote(wren, EMOTE_WAVE)), "and waves by the fire");
        assert_eq!(stop_of(&game), None);
        let home = game.wren_at(now).unwrap();
        walk_to(&mut game, home);
        game.camp_talk(me, wren);
        assert_eq!(her_options(&game)[..2], [CampChoice::Open("guide"), CampChoice::Tour(TourChoice::Start)], "after it: the Guide, or the walk again");
    }

    #[test]
    fn wren_calls_a_wanderer_back_once_and_stops_when_asked() {
        let (mut game, me) = a_camp();
        let wren = game.keeper_guid(Keeper::Wren);
        let home = game.wren_at(game.camp.as_ref().unwrap().ticked).unwrap();
        walk_to(&mut game, home);
        game.camp_talk(me, wren);
        game.camp_pick(me, wren, 0);
        let mut now = game.camp.as_ref().unwrap().ticked + Duration::from_secs(1);
        game.tour_tick(now);
        let called = game.say(Keeper::Wren, game.camp_walk().words.wandered_off)[0].clone();
        // south, away from her first stop in the Alliance's basin to the north
        walk_to(&mut game, (home.0 - 30.0, home.1));
        now += Duration::from_secs(1);
        assert!(game.tour_tick(now).contains(&called), "wandered off: called back");
        now += Duration::from_secs(1);
        assert!(!game.tour_tick(now).contains(&called), "once");
        // back to her, on her way to her first stop
        let by_her = game.wren_at(game.camp.as_ref().unwrap().ticked).unwrap();
        walk_to(&mut game, by_her);
        game.camp_talk(me, wren);
        assert_eq!(her_options(&game)[2], CampChoice::Tour(TourChoice::Enough));
        game.camp_pick(me, wren, 2);
        now += Duration::from_secs(60);
        game.tour_tick(now);
        assert_eq!(stop_of(&game), None, "the walk is over");
        assert!(!game.recruited(), "and earns nothing");
        let home = game.wren_at(now).unwrap();
        walk_to(&mut game, home);
        game.camp_talk(me, wren);
        assert_eq!(her_options(&game)[0], CampChoice::Tour(TourChoice::Start), "she offers it again");
    }

    #[test]
    fn the_guide_turns_to_whom_her_line_tells_of() {
        let (game, _) = camp_at(NEUTRAL_ALLIANCE_CAMP);
        let place = NEUTRAL_ALLIANCE_CAMP;
        let keeper = |k: Keeper| place.at(place.layout.keepers.iter().find(|(p, _)| *p == k).unwrap().1);
        let stop = &walks::WREN_AT_THE_CAVERNS.stops[1];
        let told = |i: usize| game.told_of(place, stop.lines[i].0, stop.lines[i].1);
        assert_eq!(told(0), Some(keeper(Keeper::Chronicler)), "Tobbin");
        assert_eq!(told(1), Some(keeper(Keeper::Merchant)), "Maribel");
        assert_eq!(told(2), Some(keeper(Keeper::Merchant)), "Maribel answering");
    }

    #[test]
    fn chromie_welcomes_the_guide_walks_the_hero_round_and_hands_it_back() {
        for place in [NEUTRAL_ALLIANCE_CAMP, NEUTRAL_HORDE_CAMP] {
            let (mut game, me) = camp_at(place);
            let walk = walk_of(place);
            for (i, stop) in walk.stops.iter().enumerate() {
                assert!(stop.stand_at(place).is_some(), "{}: stop {i} has a place to stand", place.camp);
            }
            let chromie = crate::game::unit_guid(super::super::CHROMIE_ENTRY, 1);
            let mut now = game.camp.as_ref().unwrap().ticked;
            game.chromie_welcome(me, None, now);
            let mut heard = Vec::new();
            // her lines 4 to 7 seconds apart, by their length, till the guide is called
            let called = game.camp.as_ref().unwrap().lesson.unwrap().1;
            while now <= called {
                now += Duration::from_secs(3);
                heard.extend(game.tour_tick(now));
            }
            for words in [&walks::CHROMIE_FIRST_ANY].into_iter().chain(walks::CHROMIE_WELCOME) {
                assert!(heard.contains(&game.chromie_say(words)[0]), "{}: {words}", place.camp);
            }
            assert_eq!(stop_of(&game), Some(0), "{}: the guide comes over", place.camp);
            loop {
                let i = stop_of(&game).unwrap();
                let stop = &walk.stops[i];
                now += Duration::from_secs(60);
                walk_to(&mut game, stop.stand_at(place).unwrap().0);
                let mut heard = game.tour_tick(now);
                now += Duration::from_secs(30);
                heard.extend(game.tour_tick(now));
                if stop.part == Part::Back {
                    assert!(game.recruited(), "{}: the walk earns Recruit before the hand-back", place.camp);
                    // her last words before the send-off: more heroes, of every class for each side
                    let at = |p: &Packet| heard.iter().position(|h| h == p);
                    let more = at(&game.chromie_say(walks::CHROMIE_MORE_HEROES)[0]).expect("Chromie asks for more heroes");
                    assert_eq!(stop.chromie.last(), Some(&walks::CHROMIE_MORE_HEROES), "{}", place.camp);
                    for words in stop.chromie.iter().filter(|w| **w != walks::CHROMIE_MORE_HEROES) {
                        assert!(at(&game.chromie_say(words)[0]).unwrap() < more, "{}: {words} before the more heroes", place.camp);
                    }
                    assert!(at(&packets::raid_warning(stop.try_it)).unwrap() > more, "{}: then the gold line to talk to her", place.camp);
                    game.chromie_talk(me, None);
                    let options = game.camp.as_ref().unwrap().menus.get(&chromie).cloned().unwrap();
                    assert_eq!(options[0], CampChoice::Tour(TourChoice::SendMe), "{}: Chromie sends it through", place.camp);
                    break;
                }
                if let Ends::By(dids) = stop.ends {
                    game.tour_did(me, dids[0]);
                    game.tour_closed(me);
                }
                game.tour_tick(now);
                assert_ne!(stop_of(&game), Some(i), "{}: on from stop {i}", place.camp);
            }
            // the guide's own walk again: no meeting at Chromie's, no hand-back
            let mut game = camp_at(place).0;
            let now = game.camp.as_ref().unwrap().ticked;
            game.start_walk(me, false, now);
            assert_eq!(stop_of(&game), walk.after(None, false));
            assert!(walk.stops[stop_of(&game).unwrap()].part == Part::Walk);
            assert_eq!(walk.after(Some(walk.stops.iter().rposition(|s| s.part == Part::Walk).unwrap()), false), None);
        }
    }

    /// Chromie's first words to the warband's first hero are its own, for its race and the time
    /// period it was pulled in from, each pair's different from every other's and from the rest of
    /// her welcome; then the guide comes over and meets it at hers.
    #[test]
    fn chromies_first_words_are_for_the_heros_race_and_time_period() {
        use crate::game::camp::{camp_content, CAMP_HOUR};
        use esurv_content::TimePeriod;
        use std::collections::HashSet;
        use std::sync::Arc;
        const RACES: [&str; 8] = ["human", "dwarf", "gnome", "night_elf", "orc", "troll", "tauren", "undead"];
        let firsts: HashSet<&str> = RACES.iter().flat_map(|r| TimePeriod::ALL.map(|p| walks::chromie_first(r, p))).collect();
        assert_eq!(firsts.len(), RACES.len() * TimePeriod::ALL.len(), "a line of its own for each race and time period");
        assert_eq!(walks::CHROMIE_FIRST.len(), firsts.len(), "and no others");
        for words in walks::CHROMIE_WELCOME.iter().chain([&walks::CHROMIE_FIRST_ANY]) {
            assert!(!firsts.contains(words), "{words}");
        }
        assert_eq!(walks::chromie_first("blood_elf", TimePeriod::Past), walks::CHROMIE_FIRST_ANY);

        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = esurv_content::Content::load(dir).unwrap();
        for race in RACES {
            let place = if ["orc", "troll", "tauren", "undead"].contains(&race) { NEUTRAL_HORDE_CAMP } else { NEUTRAL_ALLIANCE_CAMP };
            for time_period in TimePeriod::ALL {
                // a fresh warband's first hero, made with its time period, comes into the camp
                let (own, zone) = camp_content(&content, place, CAMP_HOUR).unwrap();
                let saves = Arc::new(crate::save::Saves::in_memory());
                assert!(saves.set_time_period(0, time_period));
                let me = saves.create("Aria", "warrior", race, 1, [0; 5]).unwrap();
                let (_host, inputs) = std::sync::mpsc::channel();
                let mut game = Game::new(own, None, zone, &me, saves, inputs).unwrap();
                game.make_camp(place, me.guid, &[], Default::default());
                let mut now = game.camp.as_ref().unwrap().ticked;
                let mut heard = Vec::new();
                for _ in 0..21 {
                    now += Duration::from_secs(3);
                    heard.extend(game.tour_tick(now));
                }
                let first = walks::chromie_first(race, time_period);
                let said = |words: &str| heard.iter().position(|p| *p == game.chromie_say(words)[0]);
                let at = said(first).unwrap_or_else(|| panic!("{race} {time_period:?}: {first}"));
                assert_eq!(said(walks::CHROMIE_FIRST_ANY), None, "{race} {time_period:?}");
                assert!(walks::CHROMIE_WELCOME.iter().all(|w| said(w).is_some_and(|i| i > at)), "{race} {time_period:?}: her other words after");
                let stop = stop_of(&game).unwrap_or_else(|| panic!("{race} {time_period:?}: the guide comes over"));
                assert_eq!(stop, 0, "{race} {time_period:?}");
                assert!(walk_of(place).stops[stop].part == Part::Meet, "{race} {time_period:?}: the guide meets it at Chromie's");
            }
        }
    }

    /// A hero made after the warband's first hears only Chromie's first words for its race and
    /// time period as it arrives, with no lesson and no guide coming over.
    #[test]
    fn a_later_new_hero_hears_only_its_own_first_words() {
        use crate::game::camp::{camp_content, CAMP_HOUR};
        use esurv_content::TimePeriod;
        use std::sync::Arc;
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = esurv_content::Content::load(dir).unwrap();
        let place = NEUTRAL_HORDE_CAMP;
        let (own, zone) = camp_content(&content, place, CAMP_HOUR).unwrap();
        let saves = Arc::new(crate::save::Saves::in_memory());
        let first = saves.create("Aria", "warrior", "orc", 1, [0; 5]).unwrap();
        assert!(saves.set_time_period(0, TimePeriod::Future));
        let me = saves.create("Brakka", "shaman", "tauren", 1, [0; 5]).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(own, None, zone, &me, saves, inputs).unwrap();
        game.make_camp(place, me.guid, std::slice::from_ref(&first), Default::default());
        let mut now = game.camp.as_ref().unwrap().ticked;
        let mut heard = Vec::new();
        for _ in 0..15 {
            now += Duration::from_secs(3);
            heard.extend(game.tour_tick(now));
        }
        let said = |words: &str| heard.contains(&game.chromie_say(words)[0]);
        assert!(said(walks::chromie_first("tauren", TimePeriod::Future)), "its own first words");
        assert!(walks::CHROMIE_WELCOME.iter().all(|w| !said(w)), "no lesson after");
        assert_eq!(stop_of(&game), None, "the guide stays put");
    }

    /// Each guide's whole walk, and her way home from every stop, over the camp's own ground ("fix the path of the horde and alliance guide to not walk through collision
    /// and to properly walk up ramps or walk around railings and obstacles"): a way found every
    /// time, the client's line between her points never through a wall, a rail, a rock or a piece
    /// of the camp nor off the ground, between the ledge and a basin over a stair's tread, and
    /// ending where she turns to whom she tells of. Reads the player's own WoW data (`WOW_DATA`).
    #[test]
    fn each_guide_walks_clear_of_all_in_the_way_and_by_the_stairs() {
        use crate::game::camp::walk::{standing, Stair, NEAR_ENDS, SAG};
        use crate::walk_graph::{CLIMB, KNEE};
        let Some(dir) = benilla_formats::wow_data() else {
            eprintln!("no WoW data: the guides' ways not walked");
            return;
        };
        let content_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = esurv_content::Content::load(content_dir).unwrap();
        let data = std::sync::Arc::new(crate::ground::GameData::open(&dir, &content).unwrap());
        // on a stair's tread: within its steps (its WMO: 4 below to 7 above its origin, 4 each side)
        let on_tread = |s: &Stair, p: (f32, f32)| {
            let (dx, dy) = (p.0 - s.at.0, p.1 - s.at.1);
            let (sin, cos) = s.facing.sin_cos();
            let (ahead, left) = (dx * cos + dy * sin, -dx * sin + dy * cos);
            (-4.0..=7.0).contains(&ahead) && (-4.0..=4.0).contains(&left)
        };
        for place in [NEUTRAL_ALLIANCE_CAMP, NEUTRAL_HORDE_CAMP] {
            let (own, zone) = crate::game::camp::camp_content(&content, place, crate::game::camp::CAMP_HOUR).unwrap();
            let (_host, inputs) = std::sync::mpsc::channel();
            let saves = std::sync::Arc::new(crate::save::Saves::in_memory());
            let me = saves.create("Aria", "paladin", "human", 0, [0; 5]).unwrap();
            let made = Instant::now();
            let mut game = Game::new(own, Some(&data), zone, &me, saves, inputs).unwrap();
            game.make_camp(place, me.guid, &[], Default::default());
            let floors = game.camp.as_ref().unwrap().floors.clone().expect("the camp's own ground");
            let (_, walls) = game.camp_walks();
            assert_eq!(walls.stairs.len(), 2, "{}: the two stairs", place.camp);
            println!("{}: camp's ground {:?} faces, made in {:.1} s", place.camp, floors.face_counts(), made.elapsed().as_secs_f32());
            let walk = walk_of(place);
            let home_spot = place.layout.keepers.iter().find(|(k, _)| *k == Keeper::Wren).unwrap().1;
            let home = place.at(home_spot);
            let ground = |p: (f32, f32)| game.guide_ground(place, &walls, p);
            // the stair's middle height: above it the ledge, below it a basin
            let level = |p: (f32, f32), z: f32| {
                let s = walls.stairs.iter().min_by(|a, b| dist(a.at, p).total_cmp(&dist(b.at, p))).unwrap();
                let (top, foot) = (standing(&floors, s.up(5.5))[0], standing(&floors, s.up(-2.0))[0]);
                if z > (top + foot) / 2.0 { "ledge" } else { "basin" }
            };
            // her walk from home by every stop in turn, then home from each
            let mut legs: Vec<(String, (f32, f32), Vec<(f32, f32)>)> = Vec::new();
            let mut from = home;
            for (k, stop) in walk.stops.iter().enumerate() {
                let (to, _) = stop.stand_at(place).unwrap();
                legs.push((format!("to stop {k}"), from, stop.via.iter().map(|s| place.at(*s)).chain([to]).collect()));
                from = to;
            }
            for (k, stop) in walk.stops.iter().enumerate() {
                legs.push((format!("home from stop {k}"), stop.stand_at(place).unwrap().0, vec![home]));
            }
            for (name, start, ends) in legs {
                let mut way = vec![[start.0, start.1, ground(start)]];
                for &end in &ends {
                    let here = *way.last().unwrap();
                    let timed = Instant::now();
                    let r = walls.guide_route(&floors, here, end, ground(end)).unwrap_or_else(|| panic!("{}: {name}: no way from {here:?} to {end:?}", place.camp));
                    assert!(timed.elapsed().as_secs_f32() < 2.0, "{}: {name}: {:.1} s to find", place.camp, timed.elapsed().as_secs_f32());
                    way.extend(r.into_iter().skip(1));
                }
                let (first, last) = (way[0], *way.last().unwrap());
                let to = *ends.last().unwrap();
                let length: f32 = way.windows(2).map(|p| dist((p[0][0], p[0][1]), (p[1][0], p[1][1]))).sum();
                println!("  {name}: {} points, {length:.0} yd for {:.0} straight, {} to {}", way.len(), dist(start, to), level((first[0], first[1]), first[2]), level(to, last[2]));
                assert!(length <= 3.0 * dist(start, to) + 15.0, "{}: {name}: a long way round, {length} yd", place.camp);
                assert_eq!((last[0], last[1]), to, "{}: {name}: she ends where she turns to whom she tells of", place.camp);
                assert!((last[2] - ground(to)).abs() <= SAG + 0.05, "{}: {name}: she ends on the floor there, {} not {}", place.camp, last[2], ground(to));
                // between the ledge and a basin: over a stair's tread
                if level((first[0], first[1]), first[2]) != level(to, last[2]) {
                    assert!(
                        way.iter().any(|p| walls.stairs.iter().any(|s| on_tread(s, (p[0], p[1])))),
                        "{}: {name}: from the {} to the {} by no stair: {way:?}",
                        place.camp,
                        level((first[0], first[1]), first[2]),
                        level(to, last[2])
                    );
                }
                // the client's line between her points, a quarter yard at a time
                let near_ends = |p: (f32, f32)| dist(p, start) <= NEAR_ENDS + 0.5 || dist(p, to) <= NEAR_ENDS + 0.5;
                for pair in way.windows(2) {
                    let (a, b) = (pair[0], pair[1]);
                    let run = (b[0] - a[0]).hypot(b[1] - a[1]);
                    // a stair's riser one step, else no steeper than a climb
                    assert!((b[2] - a[2]).abs() <= CLIMB * run.max(1.0), "{}: {name}: too steep from {a:?} to {b:?}", place.camp);
                    let steps = (run / 0.25).ceil().max(1.0) as usize;
                    let mut last = a;
                    for s in 1..=steps {
                        let t = s as f32 / steps as f32;
                        let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
                        let p2 = (p[0], p[1]);
                        assert!(
                            standing(&floors, p2).iter().any(|z| (z - p[2]).abs() <= SAG + 0.05),
                            "{}: {name}: off the ground at {p:?} between {a:?} and {b:?} (floors there {:?})",
                            place.camp,
                            floors.walk_heights(p[0], p[1])
                        );
                        assert!(!floors.walk_blocked([last[0], last[1], last[2] + KNEE], [p[0], p[1], p[2] + KNEE]), "{}: {name}: through a wall from {last:?} to {p:?}", place.camp);
                        assert!(near_ends(p2) || !walls.in_the_way(p2), "{}: {name}: through a piece of the camp at {p:?}", place.camp);
                        last = p;
                    }
                }
            }
        }
    }
}
