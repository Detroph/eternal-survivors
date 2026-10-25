//! A line's runners (docs/game/screens.md, "Looks"; Stampede's beasts): when a `LineHit` with
//! `runners` strikes, the simulation says so (`Event::LineRunners`), and the server puts a row of
//! creatures across the line's width a little behind the hero, sends each running the line's
//! length at the runners' speed (`SMSG_MONSTER_MOVE`), and takes each away as it reaches the end.
//! A runner is a unit of its own, as a bunny is (`bunnies`): no name, no health bar, cannot be
//! targeted or attacked, takes up no room, and is no enemy of the simulation. The damage stays
//! the line's. Any line may have runners; only Stampede has them so far.

use esurv_content::{spread, LineRunners};
use esurv_sim::game::enemy_ai::{Segment, Vec2};

use super::bunnies::still_unit_create;
use super::{unit_guid, Game};
use crate::packets;
use crate::update::{update_object, ObjectUpdate, Speeds};
use crate::wire::Packet;

/// The row sets off this far behind the hero (yards).
pub const RUNNERS_START_BEHIND: f32 = 3.0;
/// Each runner sets off up to this far before or after the row's line (yards), so the row is
/// ragged as a herd is.
const RUNNER_STAGGER: f32 = 1.0;

/// One runner of a row: its look and the scale it is drawn at, where it sets off and where it goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Runner {
    pub display: u32,
    pub scale: f32,
    pub from: Vec2,
    pub to: Vec2,
}

/// The row of `runners` for a line from `from` to `to`, `width` yards either side of it, picked
/// by `seed`: how many (between the runners' two counts), each one's look, and its lane, the
/// lanes spread evenly across the line's whole width; the row sets off
/// [`RUNNERS_START_BEHIND`] behind `from`, each runner a little before or after it, and each runs
/// straight on to the line's end.
pub fn runner_row(runners: &LineRunners, from: Vec2, to: Vec2, width: f32, seed: u32) -> Vec<Runner> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = (dx * dx + dy * dy).sqrt();
    if length < 0.01 || runners.displays.is_empty() {
        return Vec::new();
    }
    let (along, across) = ((dx / length, dy / length), (-dy / length, dx / length));
    let (least, most) = (runners.count.0.min(runners.count.1), runners.count.0.max(runners.count.1));
    let count = least + spread(seed, 1) % (most - least + 1);
    let looks = row_looks(&runners.displays, count, seed);
    (0..count)
        .map(|i| {
            let lane = -width + 2.0 * width * (i as f32 + 0.5) / count as f32;
            let stagger = RUNNER_STAGGER * ((spread(seed, 100 + i) % 201) as f32 / 100.0 - 1.0);
            let back = RUNNERS_START_BEHIND + stagger;
            let start = (from.0 - along.0 * back + across.0 * lane, from.1 - along.1 * back + across.1 * lane);
            let end = (to.0 + across.0 * lane, to.1 + across.1 * lane);
            let (display, scale) = looks[i as usize];
            Runner { display, scale, from: start, to: end }
        })
        .collect()
}

/// The most runners of one look in a row (a row is a mixed herd).
pub const MOST_OF_ONE_LOOK: usize = 3;

/// `count` looks of `displays` picked at random by `seed`, at most [`MOST_OF_ONE_LOOK`] of any
/// one (past that only when there are too few looks to keep to it).
fn row_looks<T: Copy + PartialEq>(displays: &[T], count: u32, seed: u32) -> Vec<T> {
    let mut picked: Vec<T> = Vec::with_capacity(count as usize);
    let room = displays.len() * MOST_OF_ONE_LOOK >= count as usize;
    for i in 0..count {
        let first = spread(seed, 200 + i) as usize % displays.len();
        let pick = (0..displays.len())
            .map(|k| displays[(first + k) % displays.len()])
            .find(|d| !room || picked.iter().filter(|p| *p == d).count() < MOST_OF_ONE_LOOK)
            .unwrap_or(displays[first]);
        picked.push(pick);
    }
    picked
}

impl Game {
    /// A row of runners sets off along the line from `from` to `to`: each runner's create, then
    /// its run to the end; each goes as it gets there ([`Game::mirror_line_runners`]).
    pub(super) fn send_line_runners(&mut self, runners: &LineRunners, from: Vec2, to: Vec2, width: f32, all: &mut Vec<Packet>) {
        // a fresh pick each row, and each run its own
        self.next_runner_row = self.next_runner_row.wrapping_add(1);
        let seed = spread(self.run.seed as u32 ^ (self.run.seed >> 32) as u32, self.next_runner_row);
        let row = runner_row(runners, from, to, width, seed);
        let now = self.run.now;
        let speed = runners.speed.max(0.1);
        let mut creates = Vec::new();
        let mut moves = Vec::new();
        for r in row {
            self.next_bunny = self.next_bunny.wrapping_add(1) & 0xFF_FFFF;
            let guid = unit_guid(crate::entries::LINE_RUNNER, self.next_bunny);
            let mut create = still_unit_create(guid, crate::entries::LINE_RUNNER, r.display, r.scale, [r.from.0, r.from.1, self.ground_z(r.from)]);
            if let ObjectUpdate::Create { living, .. } = &mut create {
                living.facing = (r.to.1 - r.from.1).atan2(r.to.0 - r.from.0);
                living.speeds = Speeds::running_at(speed);
            }
            creates.push(create);
            let length = ((r.to.0 - r.from.0).powi(2) + (r.to.1 - r.from.1).powi(2)).sqrt();
            let segment = Segment { from: r.from, to: r.to, start: now, duration: length / speed };
            moves.push(self.walk(guid, &segment));
            self.line_runners.push((guid, now + segment.duration));
        }
        if !creates.is_empty() {
            all.push(update_object(&creates));
            all.extend(moves);
        }
    }

    /// Runners that reached the line's end go.
    pub(super) fn mirror_line_runners(&mut self, out: &mut Vec<Packet>) {
        let now = self.run.now;
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.line_runners).into_iter().partition(|(_, until)| now >= *until);
        self.line_runners = kept;
        out.extend(gone.into_iter().map(|(guid, _)| packets::destroy_object(guid)));
    }

    /// Every runner goes (the field was cleared).
    pub(super) fn forget_line_runners(&mut self) -> Vec<Packet> {
        self.line_runners.drain(..).map(|(guid, _)| packets::destroy_object(guid)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stampede() -> LineRunners {
        LineRunners { displays: vec![(604, 0.55), (503, 0.54), (2548, 0.23)], count: (5, 7), speed: 14.0 }
    }

    /// A row is 5 to 7 runners of the listed looks, across the line's whole width, setting off
    /// behind the hero and running parallel to the line to its end; the picks vary with the seed.
    #[test]
    fn a_row_of_runners_spreads_across_the_line_behind_the_hero() {
        let (from, to) = ((10.0, 20.0), (40.0, 20.0));
        let mut counts = std::collections::HashSet::new();
        for seed in 0..60 {
            let row = runner_row(&stampede(), from, to, 2.0, seed);
            counts.insert(row.len());
            assert!((5..=7).contains(&row.len()), "{}", row.len());
            for r in &row {
                assert!(stampede().displays.contains(&(r.display, r.scale)));
                let back = from.0 - r.from.0;
                assert!((RUNNERS_START_BEHIND - RUNNER_STAGGER - 1e-3..=RUNNERS_START_BEHIND + RUNNER_STAGGER + 1e-3).contains(&back), "{back}");
                assert!((r.to.0 - to.0).abs() < 1e-3, "it runs to the line's end");
                assert!((r.from.1 - r.to.1).abs() < 1e-3, "parallel to the line");
                assert!((r.from.1 - from.1).abs() < 2.0, "within the line's width");
            }
            let mut lanes: Vec<f32> = row.iter().map(|r| r.from.1).collect();
            lanes.sort_by(f32::total_cmp);
            assert!(lanes.windows(2).all(|w| w[1] - w[0] > 0.5), "each its own lane");
        }
        assert_eq!(counts.len(), 3, "5, 6 and 7 all come up");
        assert!(runner_row(&stampede(), from, from, 2.0, 1).is_empty(), "no line, no row");
    }
}

#[cfg(test)]
mod cap_tests {
    use super::*;

    /// No row has more than 3 of one look, even from few looks; with too few looks to
    /// keep to it the row still fills; and the rows differ from one to the next.
    #[test]
    fn a_row_has_at_most_three_of_one_look() {
        let mut rows = std::collections::HashSet::new();
        for seed in 0..500 {
            for displays in [vec![604, 503, 2548, 1744, 1973, 1058, 8837], vec![604, 503], vec![604, 503, 2548]] {
                let looks = row_looks(&displays, 7, seed);
                assert_eq!(looks.len(), 7);
                if displays.len() * MOST_OF_ONE_LOOK >= 7 {
                    for d in &displays {
                        assert!(looks.iter().filter(|x| *x == d).count() <= MOST_OF_ONE_LOOK, "{looks:?}");
                    }
                }
                if displays.len() == 7 {
                    rows.insert(looks);
                }
            }
        }
        assert!(rows.len() > 400, "fresh picks: {}", rows.len());
        assert_eq!(row_looks(&[604], 5, 1), vec![604; 5], "one look fills the row");
    }
}
