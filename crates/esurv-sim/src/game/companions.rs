//! Companions in a run (docs/game/achievements.md, "Companions"): a small pet that follows its
//! hero and fetches gold. It runs out to the gold piles past its hero's own
//! pickup range, as far as the upgrade's reach goes past it, picks them up one after another and
//! carries them; with as many piles as the upgrade lets it carry, or none left in reach, it runs
//! back and hands them over. The gold counts for its hero only then, with the coin sound. It
//! never fights, never blocks, and no enemy ever looks at it: it is no unit of the fight.
//!
//! The gold it still carries when the run ends (won or lost) is handed over then, so none is lost;
//! while its hero is down it waits beside it, carrying what it has.

use esurv_content::CompanionRank;

use super::enemy_ai::{Segment, Vec2};
use super::loot::Pickup;
use super::pickups::MAGNET_SOUND;
use super::run::{Event, Run};

/// The coin sound of handing the gold over: the stock loot window's coin (`SoundEntries.dbc`
/// 120, LOOTWINDOWCOINSOUND), as a magnet plays.
pub const DEPOSIT_SOUND: u32 = MAGNET_SOUND;
/// It picks a pile up within this many yards of it.
pub const GRAB_RANGE: f32 = 1.0;
/// It hands the gold over within this many yards of its hero.
pub const DEPOSIT_RANGE: f32 = 2.0;
/// With nothing to fetch it keeps to its hero's right, a little behind, this far from it.
pub const FOLLOW_DISTANCE: f32 = 2.5;
pub const FOLLOW_ANGLE: f32 = -0.75 * std::f32::consts::PI;
/// It runs again once its place by the hero is this far from it.
pub const FOLLOW_SLACK: f32 = 1.0;
/// It decides where to go this often (seconds), and at once on arriving.
pub const DECISION: f32 = 0.2;
/// Following or running back it is never slower than its hero's run speed times this, so a hero
/// made faster is not left behind.
pub const KEEP_UP_SHARE: f32 = 1.2;
/// Farther than this from its hero (a teleport), it is put back beside it.
pub const LEASH: f32 = 60.0;

/// What a companion is doing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Errand {
    /// Beside its hero, nothing in reach.
    Follow,
    /// Running to this gold pile (its pickup id).
    Fetch(u32),
    /// Running back to its hero with what it carries.
    Return,
}

/// A companion on the field.
#[derive(Debug, Clone, PartialEq)]
pub struct CompanionUnit {
    pub id: u32,
    /// Its hero's place in the run's heroes.
    pub hero: usize,
    /// Its id in companions.ron.
    pub companion: String,
    /// The warband's upgrade rank it runs at (shop.ron).
    pub rank: CompanionRank,
    pub segment: Segment,
    pub errand: Errand,
    /// The gold piles it carries, by their value.
    pub carried: Vec<u32>,
    pub next_decision: f32,
}

/// A companion a hero brings: its id and the warband's upgrade rank (1 the first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionSetup {
    pub id: String,
    pub rank: usize,
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Its place beside a hero at `at` facing `facing`.
pub fn follow_place(at: Vec2, facing: f32) -> Vec2 {
    let a = facing + FOLLOW_ANGLE;
    (at.0 + a.cos() * FOLLOW_DISTANCE, at.1 + a.sin() * FOLLOW_DISTANCE)
}

impl Run {
    /// Hero `hero` brings companion `setup` into the run: it appears beside it. Nothing for a
    /// companion the content lacks.
    pub(crate) fn add_companion(&mut self, hero: usize, setup: &CompanionSetup) {
        if self.content.companion(&setup.id).is_none() {
            return;
        }
        let Some(&rank) = self.content.companion_rank(setup.rank) else {
            return;
        };
        let h = &self.heroes[hero];
        let at = follow_place(h.pos, h.facing);
        let id = self.new_id();
        let now = self.now;
        self.companions.push(CompanionUnit {
            id,
            hero,
            companion: setup.id.clone(),
            rank,
            segment: Segment::standing(at, now),
            errand: Errand::Follow,
            carried: Vec::new(),
            next_decision: now,
        });
    }

    /// The gold pile companion `c` goes for next: the nearest to it of the piles lying past its
    /// hero's pickup range and within the reach past it, none flying and none another companion
    /// is fetching.
    fn pile_to_fetch(&self, c: usize) -> Option<u32> {
        let k = &self.companions[c];
        let h = &self.heroes[k.hero];
        let at = k.segment.position(self.now);
        let (near, far) = (h.stats.pickup_range, h.stats.pickup_range + k.rank.reach);
        let taken: Vec<u32> = self
            .companions
            .iter()
            .enumerate()
            .filter(|&(o, _)| o != c)
            .filter_map(|(_, o)| match o.errand {
                Errand::Fetch(p) => Some(p),
                _ => None,
            })
            .collect();
        self.pickups
            .iter()
            .filter(|p| p.kind == Pickup::Gold && p.flying.is_none() && !taken.contains(&p.id))
            .filter(|p| {
                let d = dist(h.pos, p.at);
                d > near && d <= far
            })
            .min_by(|a, b| dist(at, a.at).total_cmp(&dist(at, b.at)))
            .map(|p| p.id)
    }

    /// Companion `c` hands what it carries to its hero: the gold counts (times the hero's gold
    /// bonus, as a pile it takes itself) and the coin sound plays.
    fn deposit(&mut self, c: usize) {
        let k = &mut self.companions[c];
        if k.carried.is_empty() {
            return;
        }
        let piles = std::mem::take(&mut k.carried);
        let h = &mut self.heroes[k.hero];
        let gold: u32 = piles.iter().map(|&v| (v as f32 * h.stats.gold).round().max(0.0) as u32).sum();
        h.gold += gold;
        h.gold_earned += gold;
        let hero = h.id;
        self.push(Event::Sound(DEPOSIT_SOUND));
        self.push(Event::CompanionDeposit { companion: self.companions[c].id, hero, gold });
    }

    /// Every companion hands over what it still carries (the run ended).
    pub(crate) fn deposit_all_companions(&mut self) {
        for c in 0..self.companions.len() {
            self.deposit(c);
        }
    }

    /// Companion `c` sets off to `to` at `speed`, unless it is already on its way there.
    fn companion_go(&mut self, c: usize, to: Vec2, speed: f32) {
        let now = self.now;
        let k = &mut self.companions[c];
        let at = k.segment.position(now);
        if dist(k.segment.to, to) < 0.05 && k.segment.moving(now) {
            return;
        }
        let d = dist(at, to);
        k.segment = if d < 0.05 {
            Segment::standing(at, now)
        } else {
            Segment { from: at, to, start: now, duration: d / speed.max(0.1) }
        };
    }

    /// Companions fetch, carry and hand over gold, or keep beside their heroes.
    pub(crate) fn companions_tick(&mut self) {
        let now = self.now;
        for c in 0..self.companions.len() {
            let k = &self.companions[c];
            let h = &self.heroes[k.hero];
            let (hero_at, facing, alive) = (h.pos, h.facing, h.alive);
            let keep_up = k.rank.speed.max(h.stats.run_speed() * KEEP_UP_SHARE);
            let at = k.segment.position(now);
            if dist(at, hero_at) > LEASH {
                let place = follow_place(hero_at, facing);
                let k = &mut self.companions[c];
                k.segment = Segment::standing(place, now);
                k.errand = if k.carried.is_empty() { Errand::Follow } else { Errand::Return };
                continue;
            }
            // its hero is down: it waits beside it with what it carries
            if !alive {
                self.companions[c].errand = Errand::Follow;
                self.companion_go(c, follow_place(hero_at, facing), keep_up);
                continue;
            }
            // what it reached: a pile picked up, or its hero handed the gold
            match self.companions[c].errand {
                Errand::Fetch(p) => {
                    let pile = self.pickups.iter().position(|x| x.id == p);
                    let still = pile.filter(|&x| {
                        let x = &self.pickups[x];
                        x.flying.is_none() && dist(hero_at, x.at) > h_range(self, c)
                    });
                    match still {
                        Some(x) if dist(at, self.pickups[x].at) <= GRAB_RANGE => {
                            let pile = self.pickups.remove(x);
                            self.companions[c].carried.push(pile.value);
                            self.companions[c].errand = Errand::Follow;
                            self.companions[c].next_decision = now;
                        }
                        Some(_) => {}
                        // gone (its hero took it, a magnet pulled it): think again
                        None => {
                            self.companions[c].errand = Errand::Follow;
                            self.companions[c].next_decision = now;
                        }
                    }
                }
                Errand::Return if dist(at, hero_at) <= DEPOSIT_RANGE => {
                    self.deposit(c);
                    self.companions[c].errand = Errand::Follow;
                    self.companions[c].next_decision = now;
                }
                _ => {}
            }
            if now < self.companions[c].next_decision {
                continue;
            }
            self.companions[c].next_decision = now + DECISION;
            let k = &self.companions[c];
            let full = k.carried.len() as u32 >= k.rank.carry;
            let errand = match k.errand {
                Errand::Return => Errand::Return,
                _ if full => Errand::Return,
                Errand::Fetch(p) => Errand::Fetch(p),
                Errand::Follow => match self.pile_to_fetch(c) {
                    Some(p) => Errand::Fetch(p),
                    None if !k.carried.is_empty() => Errand::Return,
                    None => Errand::Follow,
                },
            };
            let speed = self.companions[c].rank.speed;
            self.companions[c].errand = errand;
            match errand {
                Errand::Fetch(p) => {
                    if let Some(x) = self.pickups.iter().find(|x| x.id == p).map(|x| x.at) {
                        self.companion_go(c, x, speed);
                    }
                }
                Errand::Return => self.companion_go(c, hero_at, keep_up),
                Errand::Follow => {
                    let place = follow_place(hero_at, facing);
                    let k = &self.companions[c];
                    if dist(k.segment.to, place) > FOLLOW_SLACK || dist(at, place) > FOLLOW_SLACK {
                        self.companion_go(c, place, keep_up);
                    }
                }
            }
        }
    }
}

/// The pickup range of companion `c`'s hero.
fn h_range(run: &Run, c: usize) -> f32 {
    run.heroes[run.companions[c].hero].stats.pickup_range
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::Content;

    use super::*;
    use crate::game::enemy::RunOptions;
    use crate::game::run::{Flat, HeroSetup, Phase};

    fn run(rank: usize) -> Run {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Arc::new(Content::load(dir).expect("content loads"));
        let setup = HeroSetup {
            class: "warrior".into(),
            race: "human".into(),
            companion: Some(CompanionSetup { id: "bombay_cat".into(), rank }),
            ..Default::default()
        };
        let mut r = Run::new(content, "elwynn", RunOptions::default(), &[setup], Box::new(Flat), 5).unwrap();
        r.phase = Phase::Running;
        r.enemies.clear();
        r.pickups.clear();
        r
    }

    /// Gold piles of `values` as the hero counts them (its gold bonus in).
    fn counted(r: &Run, values: &[u32]) -> u32 {
        values.iter().map(|&v| (v as f32 * r.heroes[0].stats.gold).round() as u32).sum()
    }

    fn ahead(r: &Run, d: f32) -> Vec2 {
        let p = r.heroes[0].pos;
        (p.0 + d, p.1)
    }

    /// Ticks the run, enemies kept away, until `done` or `seconds` pass; the time it took.
    fn run_until(r: &mut Run, seconds: f32, mut done: impl FnMut(&Run) -> bool) -> Option<f32> {
        let start = r.now;
        while r.now - start < seconds {
            r.enemies.clear();
            r.tick(0.05);
            if done(r) {
                return Some(r.now - start);
            }
        }
        None
    }

    #[test]
    fn a_hero_brings_its_companion_beside_it() {
        let r = run(1);
        assert_eq!(r.companions.len(), 1);
        let k = &r.companions[0];
        assert_eq!((k.companion.as_str(), k.rank.reach, k.rank.speed, k.rank.carry), ("bombay_cat", 8.0, 9.0, 3));
        let d = dist(k.segment.position(r.now), r.heroes[0].pos);
        assert!((d - FOLLOW_DISTANCE).abs() < 1e-3);
    }

    /// Three piles past the pickup range: it fetches them one after another and hands them over
    /// together; the gold counts only then, with the coin sound.
    #[test]
    fn it_carries_three_piles_and_the_gold_counts_on_handing_over() {
        let mut r = run(1);
        let range = r.heroes[0].stats.pickup_range;
        for (k, d) in [2.0, 4.0, 6.0].into_iter().enumerate() {
            let mut at = ahead(&r, range + d);
            at.1 += k as f32;
            r.drop(Pickup::Gold, 3, at);
        }
        r.drop(Pickup::Gold, 3, ahead(&r, range + 9.0)); // past the reach of rank 1
        r.drain_events();
        let carried_three = run_until(&mut r, 10.0, |r| r.companions[0].carried.len() == 3).expect("fetched three");
        assert_eq!(r.heroes[0].gold, 0, "nothing counts while carried");
        assert!(r.drain_events().iter().all(|e| *e != Event::Sound(DEPOSIT_SOUND)), "no sound on picking up");
        run_until(&mut r, 10.0, |r| r.companions[0].carried.is_empty()).expect("handed over");
        assert_eq!(r.heroes[0].gold, counted(&r, &[3, 3, 3]));
        assert_eq!(r.heroes[0].gold_earned, counted(&r, &[3, 3, 3]));
        assert!(r.drain_events().contains(&Event::Sound(DEPOSIT_SOUND)));
        assert_eq!(r.pickups.len(), 1, "the far pile lies");
        assert!(carried_three > 0.5);
    }

    /// With fewer piles than it carries, it brings back what it found once none is left in reach.
    #[test]
    fn with_nothing_left_in_reach_it_brings_back_what_it_has() {
        let mut r = run(1);
        let range = r.heroes[0].stats.pickup_range;
        r.drop(Pickup::Gold, 4, ahead(&r, range + 5.0));
        run_until(&mut r, 10.0, |r| r.heroes[0].gold > 0).expect("handed over");
        assert_eq!(r.heroes[0].gold, counted(&r, &[4]));
        assert_eq!(r.companions[0].errand, Errand::Follow);
    }

    /// Rank 3 reaches 16 yd past the pickup range, runs 21 yd/s and carries 9 piles.
    #[test]
    fn rank_three_reaches_farther_and_carries_more() {
        let mut r = run(3);
        let k = &r.companions[0];
        assert_eq!((k.rank.reach, k.rank.speed, k.rank.carry), (16.0, 21.0, 9));
        let range = r.heroes[0].stats.pickup_range;
        r.drop(Pickup::Gold, 2, ahead(&r, range + 15.0));
        run_until(&mut r, 10.0, |r| r.heroes[0].gold > 0).expect("fetched from far");
        assert_eq!(r.heroes[0].gold, counted(&r, &[2]));
    }

    /// Gold inside the pickup range is the hero's own, as without a companion.
    #[test]
    fn gold_in_pickup_range_stays_the_heros_own() {
        let mut r = run(1);
        r.drop(Pickup::Gold, 3, ahead(&r, 3.0));
        r.tick(0.05);
        assert_eq!(r.heroes[0].gold, counted(&r, &[3]));
        assert!(r.companions[0].carried.is_empty());
    }

    /// What it carries when the run ends is handed over then.
    #[test]
    fn carried_gold_is_handed_over_when_the_run_ends() {
        let mut r = run(3);
        let range = r.heroes[0].stats.pickup_range;
        r.drop(Pickup::Gold, 5, ahead(&r, range + 3.0));
        r.drop(Pickup::Gold, 5, ahead(&r, range + 14.0));
        run_until(&mut r, 5.0, |r| !r.companions[0].carried.is_empty()).expect("picked one");
        r.heroes[0].alive = false;
        r.tick(0.05);
        assert_eq!(r.phase, Phase::Defeat);
        assert!(r.companions[0].carried.is_empty());
        assert!(r.heroes[0].gold >= 5);
    }

    /// Without a companion nothing changes.
    #[test]
    fn no_companion_no_errands() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Arc::new(Content::load(dir).unwrap());
        let setup = HeroSetup { class: "warrior".into(), race: "human".into(), ..Default::default() };
        let r = Run::new(content, "elwynn", RunOptions::default(), &[setup], Box::new(Flat), 5).unwrap();
        assert!(r.companions.is_empty());
    }
}
