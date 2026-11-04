//! The Bronze Dragonflight's watch over the neutral camp: two Bronze Drakonids
//! guarding the way out of the Caverns of Time's alcove at its western border, looking west; two
//! more walking a round together along the middle path and through both sides' basins; and the
//! bronze dragons Tick and Chronalis flying rounds high over the camp and past its edges. Only
//! looks: none of them is spoken to or fought.

use std::f32::consts::TAU;
use std::time::Instant;

use super::walk::Walls;
use super::{keeper_faction, spot, CampPlace, Spot, KEEPER_UNIT_FLAGS, NEUTRAL_ALLIANCE_CAMP};
use crate::entries::CAMP_KEEPERS_FROM;
use crate::game::{dist, unit_guid, Game, PATH_STEP};
use crate::packets;
use crate::update::{field, Living, ObjectTypeId, ObjectUpdate, Speeds};
use crate::wire::Packet;

/// The Bronze Drakonids' creature entry, the guards' and the walkers' (one for all four, each its
/// own guid), and their look, Blackwing Lair's Bronze Drakonid's (`creature_template` 14263:
/// display 14887, as `brm.ron`'s summoned Bronze Drakonid wears it).
const DRAKONID_ENTRY: u32 = CAMP_KEEPERS_FROM + 12;
const DRAKONID_LOOK: u32 = 14_887;
const DRAKONID_SIZE: f32 = 1.0;
/// The dragons' creature entries, Tick's and Chronalis's (the names of two of the bronze
/// dragonflight the world has at the Caverns), each in Anachronos's bronze dragon look (display
/// 2719; the world's own Tick and Chronalis stand there in mortal guise).
const TICK_ENTRY: u32 = CAMP_KEEPERS_FROM + 13;
const CHRONALIS_ENTRY: u32 = CAMP_KEEPERS_FROM + 14;
const DRAGON_LOOK: u32 = 2719;

/// The two guards at the way out of the alcove at its western border, either side of the middle
/// path where it leaves the circle, facing out west.
const WEST_GUARDS: [Spot; 2] = [spot(-136.0, 14.0, 270.0), spot(-136.0, 0.0, 270.0)];

/// The walkers' round, the corners they walk by in turn and back to the first, each leg routed
/// round what stands in the camp: west along the middle path to the guards and back east, north
/// of Wren by the fire; down the slope east of the north stairs into the Alliance's basin to the
/// middle of its ring of tents and back up; down the slope west of the south stairs into the
/// Horde's basin, round it and back up.
const PATROL: [Spot; 13] = [
    spot(-118.0, 7.0, 0.0),
    spot(10.0, 22.0, 0.0),
    spot(45.0, 28.0, 0.0),
    spot(58.0, 34.0, 0.0),
    spot(58.0, 58.0, 0.0),
    spot(2.0, 54.0, 0.0),
    spot(58.0, 58.0, 0.0),
    spot(58.0, 34.0, 0.0),
    spot(55.0, -2.0, 0.0),
    spot(55.0, -24.0, 0.0),
    spot(52.0, -56.0, 0.0),
    spot(14.0, -50.0, 0.0),
    spot(55.0, -24.0, 0.0),
];
/// How far behind the first walker the second walks, on the same way, yards (our pick).
const WALKER_GAP: f32 = 3.0;
/// The walkers' pace: 1.12's walk, walking.
const WALKER_PACE: f32 = Speeds::DEFAULT.walk;

/// A dragon's round: its middle, how far out and how high over the camp's ground it flies, which
/// way round, and its size.
struct Round {
    entry: u32,
    name: &'static str,
    middle: Spot,
    out: f32,
    up: f32,
    clockwise: bool,
    size: f32,
}

/// Tick and Chronalis flying rounds over the camp, high above it and out past its edge, the
/// camp's reach 148.8 yards, one each way.
const ROUNDS: [Round; 2] = [
    Round { entry: TICK_ENTRY, name: "Tick", middle: spot(40.0, 10.0, 0.0), out: 125.0, up: 55.0, clockwise: true, size: 1.5 },
    Round { entry: CHRONALIS_ENTRY, name: "Chronalis", middle: spot(-20.0, -10.0, 0.0), out: 150.0, up: 75.0, clockwise: false, size: 1.8 },
];
/// The dragons' pace, yards a second (our pick).
const DRAGON_PACE: f32 = 14.0;
/// The points a round is told by: one each tenth of a quarter turn, the client flying a smooth
/// curve through them.
const ROUND_POINTS: usize = 40;

/// The watch going on: the walkers' way and the leg they are on, and the dragons' rounds.
#[derive(Clone, Debug)]
pub(super) struct Watch {
    /// The walkers' round, its corners in turn (back to the first after the last).
    way: Vec<(f32, f32)>,
    /// How far along the way each corner is, and the whole way's length at the end.
    corner_at: Vec<f32>,
    /// How far along the way each stop of [`PATROL`] is.
    stop_at: Vec<f32>,
    /// The leg walked now (from its stop to the next), set off when and how long it takes.
    leg: usize,
    leg_set_off: Instant,
    leg_takes: f32,
    /// When each dragon's round now flown set off, and how long it takes (0: none yet).
    rounds: [(Instant, f32); 2],
}

/// Whether `place` is watched: the neutral camp, either side's half.
fn watched(place: CampPlace) -> bool {
    place.spot == NEUTRAL_ALLIANCE_CAMP.spot
}

impl Watch {
    /// The watch over `place`, the walkers' way routed round `walls`; none outside the neutral
    /// camp.
    pub(super) fn new(place: CampPlace, walls: &Walls, now: Instant) -> Option<Self> {
        if !watched(place) {
            return None;
        }
        let stops: Vec<(f32, f32)> = PATROL.iter().map(|s| place.at(*s)).collect();
        let mut way = Vec::new();
        let mut stop_at = Vec::new();
        let mut corner_at = Vec::new();
        let mut length = 0.0;
        for (k, &from) in stops.iter().enumerate() {
            let to = stops[(k + 1) % stops.len()];
            stop_at.push(length);
            let route = walls.route(from, to).unwrap_or_else(|| vec![from, to]);
            for pair in route.windows(2) {
                way.push(pair[0]);
                corner_at.push(length);
                length += dist(pair[0], pair[1]);
            }
        }
        corner_at.push(length);
        // the last leg ended as the camp starts: the first tick sets them off on the first
        let leg = stops.len() - 1;
        Some(Watch { way, corner_at, stop_at, leg, leg_set_off: now, leg_takes: 0.0, rounds: [(now, 0.0); 2] })
    }

    fn length(&self) -> f32 {
        *self.corner_at.last().expect("a way has an end")
    }

    /// The point `along` yards along the way (round and round).
    fn point(&self, along: f32) -> (f32, f32) {
        let along = along.rem_euclid(self.length());
        let k = self.corner_at.partition_point(|&c| c <= along).saturating_sub(1).min(self.way.len() - 1);
        let (a, b) = (self.way[k], self.way[(k + 1) % self.way.len()]);
        let d = self.corner_at[k + 1] - self.corner_at[k];
        let t = if d > 0.0 { (along - self.corner_at[k]) / d } else { 0.0 };
        (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
    }

    /// The way from `from` to `to` yards along it: its ends and every corner between.
    fn stretch(&self, from: f32, to: f32) -> Vec<(f32, f32)> {
        let length = self.length();
        let lap = from.div_euclid(length);
        let mut points = vec![self.point(from)];
        for l in [lap, lap + 1.0] {
            for (k, &c) in self.corner_at[..self.way.len()].iter().enumerate() {
                let at = c + l * length;
                if at > from && at < to {
                    points.push(self.way[k]);
                }
            }
        }
        points.push(self.point(to));
        points
    }

    /// Where leg `leg` starts and ends along the way.
    fn leg_span(&self, leg: usize) -> (f32, f32) {
        let end = self.stop_at.get(leg + 1).copied().unwrap_or(self.length());
        (self.stop_at[leg], end)
    }

    /// How far along the way the first walker is at `now`.
    fn walked(&self, now: Instant) -> f32 {
        let (from, to) = self.leg_span(self.leg);
        let t = if self.leg_takes <= 0.0 { 1.0 } else { (now.duration_since(self.leg_set_off).as_secs_f32() / self.leg_takes).min(1.0) };
        from + (to - from) * t
    }

    /// Where dragon `k` is at `now` on its round: the turn it has come, 0 to 1.
    fn flown(&self, k: usize, now: Instant) -> f32 {
        let (set_off, takes) = self.rounds[k];
        if takes <= 0.0 {
            0.0
        } else {
            (now.duration_since(set_off).as_secs_f32() / takes).min(1.0)
        }
    }
}

impl Round {
    /// The world point `turn` (0 to 1) of the way round, at its height over the camp.
    fn point(&self, place: CampPlace, turn: f32) -> [f32; 3] {
        let way = if self.clockwise { 1.0 } else { -1.0 };
        let (s, c) = (way * turn * TAU).sin_cos();
        let (x, y) = place.at(spot(self.middle.east + self.out * s, self.middle.north + self.out * c, 0.0));
        [x, y, place.spot.2 + self.up]
    }

    /// The world facing along the round at `turn`.
    fn facing(&self, place: CampPlace, turn: f32) -> f32 {
        let (a, b) = (self.point(place, turn), self.point(place, turn + 0.01));
        (b[1] - a[1]).atan2(b[0] - a[0]).rem_euclid(TAU)
    }

    fn takes(&self) -> f32 {
        TAU * self.out / DRAGON_PACE
    }
}

/// What the client's creature query hears of a watcher's entry.
pub(super) fn creature_info(entry: u32) -> Option<packets::CreatureInfo> {
    let (name, display) = match entry {
        DRAKONID_ENTRY => ("Bronze Drakonid", DRAKONID_LOOK),
        _ => ROUNDS.iter().find(|r| r.entry == entry).map(|r| (r.name, DRAGON_LOOK))?,
    };
    Some(packets::CreatureInfo {
        name: name.into(),
        subname: String::new(),
        // Dragonkin
        creature_type: 2,
        rank: packets::RANK_NORMAL,
        display_id: display,
    })
}

/// The guid of guard `k` (0, 1) or, past them, walker `k - 2`.
fn drakonid_guid(k: usize) -> u64 {
    unit_guid(DRAKONID_ENTRY, k as u32 + 1)
}

fn dragon_guid(k: usize) -> u64 {
    unit_guid(ROUNDS[k].entry, 1)
}

impl Game {
    /// The watch's creates: the guards, the walkers where they are, the dragons where they fly.
    pub(super) fn watch_now(&self) -> Vec<ObjectUpdate> {
        let Some(camp) = &self.camp else {
            return Vec::new();
        };
        let Some(watch) = &camp.watch else {
            return Vec::new();
        };
        let place = camp.place;
        let now = std::time::Instant::now();
        let mut out = Vec::new();
        for (k, s) in WEST_GUARDS.iter().enumerate() {
            let at = place.at(*s);
            out.push(self.watcher(place, drakonid_guid(k), DRAKONID_ENTRY, DRAKONID_LOOK, DRAKONID_SIZE, [at.0, at.1, self.camp_ground(place, at)], place.facing(*s)));
        }
        let walked = watch.walked(now);
        for (k, along) in [walked, walked - WALKER_GAP].into_iter().enumerate() {
            let at = watch.point(along);
            let ahead = watch.point(along + 1.0);
            let facing = (ahead.1 - at.1).atan2(ahead.0 - at.0).rem_euclid(TAU);
            out.push(self.watcher(place, drakonid_guid(k + 2), DRAKONID_ENTRY, DRAKONID_LOOK, DRAKONID_SIZE, [at.0, at.1, self.camp_ground(place, at)], facing));
        }
        for (k, round) in ROUNDS.iter().enumerate() {
            let turn = watch.flown(k, now);
            out.push(self.watcher(place, dragon_guid(k), round.entry, DRAGON_LOOK, round.size, round.point(place, turn), round.facing(place, turn)));
        }
        out
    }

    /// A watcher of the camp at `position` facing `facing`: friendly, never fought, not to talk to.
    #[allow(clippy::too_many_arguments)]
    fn watcher(&self, place: CampPlace, guid: u64, entry: u32, display: u32, size: f32, position: [f32; 3], facing: f32) -> ObjectUpdate {
        let ObjectUpdate::Create { living, mut fields, .. } = super::super::bunnies::still_unit_create(guid, entry, display, size, position) else {
            unreachable!("a still unit is a create")
        };
        fields
            .u32(field::UNIT_HEALTH, 100)
            .u32(field::UNIT_MAXHEALTH, 100)
            .u32(field::UNIT_FACTIONTEMPLATE, keeper_faction(place.faction))
            .u32(field::UNIT_FLAGS, KEEPER_UNIT_FLAGS)
            .f32(field::UNIT_BOUNDINGRADIUS, 0.4)
            .f32(field::UNIT_COMBATREACH, 1.5);
        let living = Living { facing, ..living };
        ObjectUpdate::Create { guid, type_id: ObjectTypeId::Unit, living, fields, own: false }
    }

    /// The walkers set off on their next leg as the last ends, and each dragon on its next round.
    pub(super) fn watch_tick(&mut self, now: Instant) -> Vec<Packet> {
        let Some(place) = self.camp.as_ref().map(|c| c.place) else {
            return Vec::new();
        };
        let Some(watch) = self.camp.as_ref().and_then(|c| c.watch.clone()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut next = watch.clone();
        if now.duration_since(watch.leg_set_off).as_secs_f32() >= watch.leg_takes {
            next.leg = (watch.leg + 1) % watch.stop_at.len();
            let (from, to) = next.leg_span(next.leg);
            next.leg_set_off = now;
            next.leg_takes = (to - from) / WALKER_PACE;
            let ms = ((next.leg_takes * 1000.0).round() as u32).max(1);
            for (k, behind) in [0.0, WALKER_GAP].into_iter().enumerate() {
                let corners = next.stretch(from - behind, to - behind);
                let path = self.on_the_ground(place, &corners);
                let start = [corners[0].0, corners[0].1, self.camp_ground(place, corners[0])];
                let spline = self.next_spline;
                self.next_spline = self.next_spline.wrapping_add(1);
                out.push(packets::monster_move(drakonid_guid(k + 2), start, spline, &path, ms, false));
            }
        }
        for (k, round) in ROUNDS.iter().enumerate() {
            let (set_off, takes) = watch.rounds[k];
            if now.duration_since(set_off).as_secs_f32() < takes {
                continue;
            }
            next.rounds[k] = (now, round.takes());
            let path: Vec<[f32; 3]> = (1..=ROUND_POINTS).map(|i| round.point(place, i as f32 / ROUND_POINTS as f32)).collect();
            let spline = self.next_spline;
            self.next_spline = self.next_spline.wrapping_add(1);
            out.push(packets::monster_move_flying(dragon_guid(k), round.point(place, 0.0), spline, &path, (round.takes() * 1000.0).round() as u32));
        }
        if let Some(camp) = &mut self.camp {
            camp.watch = Some(next);
        }
        out
    }

    /// The walk by `corners` as points on the ground a step apart, the first left out.
    fn on_the_ground(&self, place: CampPlace, corners: &[(f32, f32)]) -> Vec<[f32; 3]> {
        let mut path = Vec::new();
        for pair in corners.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let steps = (dist(a, b) / PATH_STEP).ceil().max(1.0) as usize;
            path.extend((1..=steps).map(|k| {
                let t = k as f32 / steps as f32;
                let at = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                [at.0, at.1, self.camp_ground(place, at)]
            }));
        }
        path
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::Content;

    use super::*;
    use crate::game::camp::{camp_content, CAMP_HOUR, NEUTRAL_HORDE_CAMP};

    /// The camp at `place` with a hero at every tent.
    fn a_full_camp(place: CampPlace) -> Game {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).unwrap();
        let saves = Arc::new(crate::save::Saves::in_memory());
        let names = ["Aria", "Brom", "Cael", "Dorn", "Elra", "Fenn", "Gilda", "Hale", "Ivo", "Jory"];
        let heroes: Vec<_> = names.iter().map(|n| saves.create(n, "warrior", "dwarf", 0, [0; 5]).unwrap()).collect();
        let (own, zone) = camp_content(&content, place, CAMP_HOUR).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(own, None, zone, &heroes[0], saves, inputs).unwrap();
        game.make_camp(place, heroes[0].guid, &heroes[1..], Default::default());
        game
    }

    #[test]
    fn the_guards_stand_at_the_western_border_looking_west() {
        for place in [NEUTRAL_ALLIANCE_CAMP, NEUTRAL_HORDE_CAMP] {
            for s in WEST_GUARDS {
                // the world's y runs west: facing west is a quarter turn from its x
                let facing = place.facing(s);
                assert!((facing - std::f32::consts::FRAC_PI_2).abs() < 1e-4, "{s:?} faces {facing}");
                let from_middle = dist(place.at(s), place.middle());
                assert!(from_middle < place.layout.reach && from_middle > place.layout.reach - 15.0, "{s:?} by the border");
                assert!(s.east < -120.0, "{s:?} on its west side");
            }
        }
    }

    #[test]
    fn the_walkers_way_goes_round_every_tent_and_piece() {
        for place in [NEUTRAL_ALLIANCE_CAMP, NEUTRAL_HORDE_CAMP] {
            let game = a_full_camp(place);
            let (_, walls) = game.camp_walks();
            let watch = game.camp.as_ref().unwrap().watch.clone().expect("the neutral camp is watched");
            for s in PATROL {
                assert!(!walls.in_the_way(place.at(s)), "stop {s:?} stands clear");
            }
            let length = watch.length();
            let mut along = 0.0;
            while along < length {
                let p = watch.point(along);
                assert!(!walls.in_the_way(p), "{}: through something at {p:?}, {along} yd along", place.camp);
                assert!(dist(p, place.middle()) < place.layout.reach, "{p:?} inside the camp");
                along += 0.25;
            }
        }
    }

    #[test]
    fn the_walkers_legs_join_up_round_the_whole_way() {
        let game = a_full_camp(NEUTRAL_ALLIANCE_CAMP);
        let watch = game.camp.as_ref().unwrap().watch.clone().unwrap();
        for leg in 0..PATROL.len() {
            let (from, to) = watch.leg_span(leg);
            let (next, _) = watch.leg_span((leg + 1) % PATROL.len());
            assert!(to == next || (to == watch.length() && next == 0.0), "leg {leg} ends where the next starts");
            let stretch = watch.stretch(from - WALKER_GAP, to - WALKER_GAP);
            let walked: f32 = stretch.windows(2).map(|p| dist(p[0], p[1])).sum();
            assert!((walked - (to - from)).abs() < 0.01, "the second walker walks as far as the first on leg {leg}");
        }
    }

    #[test]
    fn the_dragons_fly_over_the_camp_and_past_its_edge() {
        let place = NEUTRAL_ALLIANCE_CAMP;
        for round in &ROUNDS {
            let points: Vec<[f32; 3]> = (0..ROUND_POINTS).map(|i| round.point(place, i as f32 / ROUND_POINTS as f32)).collect();
            assert!(points.iter().all(|p| p[2] >= place.spot.2 + 50.0), "{} flies high", round.name);
            assert!(points.iter().any(|p| dist((p[0], p[1]), place.middle()) < place.layout.reach), "{} flies over the camp", round.name);
            assert!(points.iter().any(|p| dist((p[0], p[1]), place.middle()) > place.layout.reach), "{} flies past its edge", round.name);
        }
    }

    #[test]
    fn the_watch_is_two_guards_two_walkers_and_two_dragons() {
        let game = a_full_camp(NEUTRAL_HORDE_CAMP);
        let creates = game.watch_now();
        assert_eq!(creates.len(), WEST_GUARDS.len() + 2 + ROUNDS.len(), "two guards, two walkers, two dragons");
        assert_eq!(creature_info(DRAKONID_ENTRY).unwrap().display_id, DRAKONID_LOOK);
        assert_eq!(creature_info(TICK_ENTRY).unwrap().name, "Tick");
        assert_eq!(creature_info(CHRONALIS_ENTRY).unwrap().name, "Chronalis");
    }
}
