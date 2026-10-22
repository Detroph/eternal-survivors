//! Pets fighting beside a hero: talents summon them for a while or keep one for the whole run.

use esurv_content::{Milestone, Pet, Stat};

use super::casting::{HitSource, PendingHit};
use super::enemy_ai::{Segment, Vec2};
use super::run::{Event, Run};

/// A pet appears this far from the hero.
pub const PET_APPEAR_DISTANCE: f32 = 2.5;
/// Pets that stay all run appear behind the hero, spread from its back-left toward its
/// back-right: the i-th of n at hero facing + `PERMANENT_PET_APPEAR_ANGLE` +
/// `PERMANENT_PET_APPEAR_SPREAD` × i / n. A pet summoned for a while appears at a random angle.
pub const PERMANENT_PET_APPEAR_ANGLE: f32 = 0.75 * std::f32::consts::PI;
pub const PERMANENT_PET_APPEAR_SPREAD: f32 = 0.5 * std::f32::consts::PI;
/// When not attacking or running at an enemy, a pet stands 5 yd from its hero, at the side WoW
/// 1.12 puts a following pet (90° to the hero's left).
pub const PET_FOLLOW_DISTANCE: f32 = 5.0;
/// Where the 1st, 2nd, … pet of a hero stands, relative to the hero's facing: left (WoW's spot),
/// then right, behind, and the four diagonals (1.12 had one pet, so the others are ours).
pub const PET_FOLLOW_ANGLES: [f32; 7] = [
    std::f32::consts::FRAC_PI_2,
    -std::f32::consts::FRAC_PI_2,
    std::f32::consts::PI,
    3.0 * std::f32::consts::FRAC_PI_4,
    -3.0 * std::f32::consts::FRAC_PI_4,
    std::f32::consts::FRAC_PI_4,
    -std::f32::consts::FRAC_PI_4,
];
/// Pets decide this often.
pub const PET_DECISION: f32 = 0.25;
/// Farther than this from the hero, a pet rushes back at the hero's run speed ×
/// `PET_LEASH_SPEED_SHARE`.
pub const PET_LEASH: f32 = 40.0;
pub const PET_LEASH_SPEED_SHARE: f32 = 3.0;
/// A ranged pet shoots the nearest enemy within this distance of itself; a melee pet takes a new
/// target only within this distance of the hero.
pub const PET_REACH: f32 = 28.0;
/// A melee pet looks for a new target among the enemies within this distance of itself, and
/// takes the nearest.
pub const PET_SEEK: f32 = 26.0;
/// A melee pet keeps its target while that is within this distance of the hero.
pub const PET_KEEP_TARGET: f32 = 32.0;
/// A melee pet strikes within this distance of its target.
pub const PET_MELEE_RANGE: f32 = 3.5;
/// A melee pet farther than this from its target keeps closing in (even while in reach).
pub const PET_CLOSE_IN: f32 = 2.6;
/// Walking pets base their speed on the hero's run speed, but never on less than this.
pub const PET_MIN_SPEED: f32 = 7.0;
/// A ranged pet moving to its place runs at max(`PET_MIN_SPEED`, hero run speed) × this.
pub const PET_SPEED_SHARE: f32 = 1.15;
/// A melee pet with no target going back to its place runs at max(`PET_MIN_SPEED`, hero run
/// speed) × this.
pub const PET_RETURN_SPEED_SHARE: f32 = 1.2;
/// A melee pet runs at its target at the hero's run speed × `PET_CHASE_SPEED_SHARE`, never
/// slower than `PET_CHASE_MIN_SPEED`, and stops `PET_CHASE_STOP_SHORT` yd short of it.
pub const PET_CHASE_SPEED_SHARE: f32 = 2.2;
pub const PET_CHASE_MIN_SPEED: f32 = 16.0;
pub const PET_CHASE_STOP_SHORT: f32 = 1.6;
/// A move shorter than this is not started: the pet carries on as it was (standing stays put).
pub const PET_MIN_MOVE: f32 = 0.3;
/// Each hero level past the first adds this share to a pet's damage.
pub const PET_DAMAGE_PER_LEVEL: f32 = 0.12;

/// A pet in the run.
#[derive(Debug, Clone, PartialEq)]
pub struct PetUnit {
    pub id: u32,
    pub hero: usize,
    /// Its name in pets.ron.
    pub pet: String,
    /// Where it stands when idle, relative to the hero's facing (one of `PET_FOLLOW_ANGLES`).
    pub slot_angle: f32,
    /// A summoned pet leaves then; `None` for one that stays all run.
    pub until: Option<f32>,
    pub segment: Segment,
    pub next_decision: f32,
    pub next_swing: f32,
    pub target: Option<u32>,
    /// A starting option's summon: what it is and does (`None`: a talent's pet).
    pub summon: Option<super::kit::Summoned>,
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn toward(from: Vec2, to: Vec2, speed: f32, now: f32) -> Segment {
    let d = dist(from, to);
    Segment { from, to, start: now, duration: if speed > 0.0 { d / speed } else { 0.0 } }
}

impl Run {
    /// Hero `h` gets pet `name` (from pets.ron), until `until` or for the whole run.
    ///
    /// A pet for the whole run is the hero's i-th of n (n = its `Pet` milestones) and appears at
    /// its share of the spread behind the hero; a summoned one appears at a random angle.
    pub fn summon_pet(&mut self, h: usize, name: &str, until: Option<f32>) {
        if !self.content.pets.iter().any(|p| p.name == name) {
            return;
        }
        let appear_angle = match until {
            Some(_) => self.rng.range(0.0, std::f32::consts::TAU),
            None => {
                let i = self.pets.iter().filter(|p| p.hero == h && p.until.is_none()).count();
                let n = self.heroes[h].milestones.iter().filter(|m| matches!(m, Milestone::Pet { .. })).count().max(i + 1);
                PERMANENT_PET_APPEAR_ANGLE + PERMANENT_PET_APPEAR_SPREAD * i as f32 / n as f32
            }
        };
        let taken: Vec<f32> = self.pets.iter().filter(|p| p.hero == h).map(|p| p.slot_angle).collect();
        let slot_angle = PET_FOLLOW_ANGLES
            .iter()
            .copied()
            .find(|a| !taken.iter().any(|t| (t - a).abs() < 1e-3))
            .unwrap_or(PET_FOLLOW_ANGLES[taken.len() % PET_FOLLOW_ANGLES.len()]);
        let hero = &self.heroes[h];
        let a = hero.facing + appear_angle;
        let at = (hero.pos.0 + a.cos() * PET_APPEAR_DISTANCE, hero.pos.1 + a.sin() * PET_APPEAR_DISTANCE);
        let id = self.new_id();
        self.pets.push(PetUnit {
            id,
            hero: h,
            pet: name.to_string(),
            slot_angle,
            until,
            segment: Segment::standing(at, self.now),
            next_decision: self.now,
            next_swing: self.now,
            target: None,
            summon: None,
        });
        let hero = self.heroes[h].id;
        self.push(Event::PetSummoned { id, hero, pet: name.to_string(), at });
    }

    fn pet_hit(&mut self, h: usize, pet: &Pet) -> (f32, bool) {
        let hero = &self.heroes[h];
        let bonus = 1.0 + hero.modifiers.iter().filter(|m| m.stat == Stat::PetDamage).map(|m| m.value).sum::<f32>();
        let level = 1.0 + PET_DAMAGE_PER_LEVEL * (hero.progress.level.saturating_sub(1)) as f32;
        let hit = pet.damage * level * hero.stats.damage * bonus;
        if self.rng.unit() < hero.stats.crit_chance {
            (hit * hero.stats.crit_damage, true)
        } else {
            (hit, false)
        }
    }

    /// Summoned pets whose time is up leave; the others follow, pick targets and attack.
    pub(crate) fn pets_tick(&mut self) {
        let now = self.now;
        let gone: Vec<super::pets::PetUnit> = self.pets.iter().filter(|p| p.until.is_some_and(|u| now >= u) || !self.heroes[p.hero].alive).cloned().collect();
        for p in gone {
            self.pets.retain(|x| x.id != p.id);
            if !p.pet.is_empty() {
                self.push(Event::PetGone { id: p.id });
            }
            // a summon whose time ran out (not one that burst or whose hero fell) may feed a counter
            let expired = self.heroes[p.hero].alive && p.until.is_some_and(|u| u > 0.0) && p.summon.as_ref().is_some_and(|s| s.step == 0 || s.kind != esurv_content::SummonKind::Plant);
            self.summon_left(&p, expired);
        }
        let content = self.content.clone();
        for k in 0..self.pets.len() {
            if now < self.pets[k].next_decision {
                continue;
            }
            self.pets[k].next_decision = now + PET_DECISION;
            if self.pets[k].summon.is_some() {
                self.summon_tick(k);
                continue;
            }
            let Some(pet) = content.pets.iter().find(|p| p.name == self.pets[k].pet) else { continue };
            let p = self.pets[k].clone();
            let h = p.hero;
            let hero = &self.heroes[h];
            let pos = p.segment.position(now);
            let run_speed = hero.stats.run_speed();
            let walk_speed = PET_MIN_SPEED.max(run_speed);
            let a = p.slot_angle + hero.facing;
            let place = (hero.pos.0 + a.cos() * PET_FOLLOW_DISTANCE, hero.pos.1 + a.sin() * PET_FOLLOW_DISTANCE);
            let hero_pos = hero.pos;
            if dist(pos, hero_pos) > PET_LEASH {
                self.move_pet(k, pos, place, run_speed * PET_LEASH_SPEED_SHARE);
                continue;
            }
            if let Some(ranged_spell) = pet.ranged_spell {
                // a ranged pet keeps its place and shoots the nearest enemy
                self.move_pet(k, pos, place, walk_speed * PET_SPEED_SHARE);
                if now >= p.next_swing {
                    let Some(&target) = self.nearest_to(pos, PET_REACH).first() else { continue };
                    self.pets[k].next_swing = now + pet.swing;
                    let (damage, crit) = self.pet_hit(h, pet);
                    let speed = self.world.missile_speed(ranged_spell);
                    let flight = match self.enemies.iter().find(|e| e.id == target) {
                        Some(e) if speed > 0.0 => dist(pos, e.position(now)) / speed,
                        _ => 0.0,
                    };
                    let id = self.pets[k].id;
                    self.hits.push(PendingHit { hero: h, target, at: now + flight, damage, crit, spell: String::new(), dot: None, pet: Some(id), from_proc: false, cast: None, burn: None });
                    let enemy = super::run::Unit::Enemy(target);
                    self.push(Event::SpellGo { caster: super::run::Unit::Pet(id), spell: ranged_spell, hits: vec![enemy], aim: super::run::Aim::Unit(enemy), ammo: false, reach: 0.0, triggered: false });
                }
                continue;
            }
            // a melee pet keeps its target while that stays near the hero; else it takes the
            // enemy nearest itself, if that one is near the hero
            let target_pos = |run: &Run, id: u32| run.enemies.iter().find(|e| e.id == id).map(|e| e.position(now));
            let target = p.target.filter(|&t| target_pos(self, t).is_some_and(|tp| dist(tp, hero_pos) < PET_KEEP_TARGET));
            let target = target.or_else(|| self.nearest_to(pos, PET_SEEK).first().copied().filter(|&t| target_pos(self, t).is_some_and(|tp| dist(tp, hero_pos) < PET_REACH)));
            self.pets[k].target = target;
            let Some(t) = target else {
                self.move_pet(k, pos, place, walk_speed * PET_RETURN_SPEED_SHARE);
                continue;
            };
            let tp = target_pos(self, t).unwrap();
            let d = dist(pos, tp);
            // in reach it strikes on its own timer, and it closes in all the same
            if d <= PET_MELEE_RANGE && now >= p.next_swing {
                self.pets[k].next_swing = now + pet.swing;
                let (damage, crit) = self.pet_hit(h, pet);
                let id = self.pets[k].id;
                self.hit_enemy(Some(h), t, damage, crit, HitSource::Pet(id));
            }
            if d > PET_CLOSE_IN {
                let short = PET_CHASE_STOP_SHORT / d;
                let stop = (tp.0 + (pos.0 - tp.0) * short, tp.1 + (pos.1 - tp.1) * short);
                self.move_pet(k, pos, stop, PET_CHASE_MIN_SPEED.max(run_speed * PET_CHASE_SPEED_SHARE));
            }
        }
    }

    /// Sends pet `k` from `from` to `to` at `speed`; a move shorter than `PET_MIN_MOVE` is not
    /// started and the pet carries on as it was.
    pub(crate) fn move_pet(&mut self, k: usize, from: Vec2, to: Vec2, speed: f32) {
        if dist(from, to) >= PET_MIN_MOVE {
            self.pets[k].segment = toward(from, to, speed, self.now);
        }
    }

    /// Living enemies within `r` of `at`, nearest first.
    pub(crate) fn nearest_to(&self, at: Vec2, r: f32) -> Vec<u32> {
        let now = self.now;
        let mut v: Vec<(f32, u32)> = self.enemies.iter().map(|e| (dist(e.position(now), at), e.id)).filter(|(d, _)| *d <= r).collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v.into_iter().map(|(_, id)| id).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::Content;

    use super::*;
    use crate::game::enemy::RunOptions;
    use crate::game::run::{Flat, HeroSetup};

    fn run() -> Run {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Arc::new(Content::load(dir).expect("content loads"));
        let mut r = Run::new(content, "elwynn", RunOptions::default(), &[HeroSetup { class: "hunter".into(), race: "orc".into(), ..Default::default() }], Box::new(Flat), 17).unwrap();
        let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
        r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        r.enemies.clear();
        r.pickups.clear();
        r.heroes[0].spells.known.clear();
        r.heroes[0].spells.racial = None;
        r.drain_events();
        r
    }

    #[test]
    fn a_wolf_runs_to_an_enemy_near_the_hero_and_bites() {
        let mut r = run();
        r.summon_pet(0, "Wolf", None);
        let p = r.heroes[0].pos;
        assert!(r.spawn_roster_enemy(0, Some((p.0 + 10.0, p.1))));
        r.enemies[0].health = 1e9;
        r.enemies[0].base_speed = 0.0;
        r.heroes[0].stats.crit_chance = 0.0;
        let wolf = r.content.pets.iter().find(|x| x.name == "Wolf").unwrap().clone();
        let mut bites = Vec::new();
        for _ in 0..100 {
            r.tick(0.05);
            r.take_first_offers();
            for e in r.drain_events() {
                if let Event::EnemyHit { amount, source: HitSource::Pet(_), .. } = e {
                    bites.push(amount);
                }
            }
        }
        assert!(!bites.is_empty(), "it reached the enemy and bit");
        let want = wolf.damage * r.heroes[0].stats.damage;
        assert!((bites[0] - want).abs() < 1e-3);
    }

    #[test]
    fn an_idle_pet_stands_five_yards_to_the_left() {
        let mut r = run();
        r.summon_pet(0, "Wolf", None);
        r.summon_pet(0, "Raptor", None);
        for _ in 0..40 {
            // no enemies: the pets stay idle
            r.enemies.clear();
            r.tick(0.05);
        }
        let h = &r.heroes[0];
        let left = (h.pos.0 + (h.facing + std::f32::consts::FRAC_PI_2).cos() * PET_FOLLOW_DISTANCE, h.pos.1 + (h.facing + std::f32::consts::FRAC_PI_2).sin() * PET_FOLLOW_DISTANCE);
        let right = (h.pos.0 + (h.facing - std::f32::consts::FRAC_PI_2).cos() * PET_FOLLOW_DISTANCE, h.pos.1 + (h.facing - std::f32::consts::FRAC_PI_2).sin() * PET_FOLLOW_DISTANCE);
        let at = |k: usize| r.pets[k].segment.position(r.now);
        assert!(dist(at(0), left) < 0.01, "the first pet at the left: {:?} vs {:?}", at(0), left);
        assert!(dist(at(1), right) < 0.01, "the second at the right");
    }

    /// How fast a pet's current move goes.
    fn pace(s: &Segment) -> f32 {
        dist(s.from, s.to) / s.duration
    }

    fn summoned_at(r: &mut Run) -> Vec<Vec2> {
        r.drain_events().into_iter().filter_map(|e| if let Event::PetSummoned { at, .. } = e { Some(at) } else { None }).collect()
    }

    #[test]
    fn pets_for_the_whole_run_appear_spread_behind_the_hero() {
        let mut r = run();
        r.heroes[0].milestones = vec![Milestone::Pet { pet: "Treant".into() }, Milestone::Pet { pet: "Treant".into() }];
        r.opening_milestones();
        let at = summoned_at(&mut r);
        assert_eq!(at.len(), 2);
        let h = &r.heroes[0];
        for (i, want) in [0.75 * std::f32::consts::PI, std::f32::consts::PI].into_iter().enumerate() {
            let a = h.facing + want;
            let spot = (h.pos.0 + a.cos() * 2.5, h.pos.1 + a.sin() * 2.5);
            assert!(dist(at[i], spot) < 1e-3, "pet {i} at {:?}, expected {:?}", at[i], spot);
        }
    }

    #[test]
    fn a_summoned_pet_appears_at_a_random_angle() {
        let mut r = run();
        for _ in 0..4 {
            r.summon_pet(0, "Imp", Some(r.now + 10.0));
        }
        let at = summoned_at(&mut r);
        let h = r.heroes[0].pos;
        assert!(at.iter().all(|&p| (dist(p, h) - 2.5).abs() < 1e-3), "all 2.5 yd out");
        assert!(at.windows(2).any(|w| dist(w[0], w[1]) > 0.1), "not all at one spot: {at:?}");
    }

    /// A melee pet takes the enemy nearest itself (not the hero), only within 28 yd of the hero,
    /// and keeps it until it is 32 yd from the hero.
    #[test]
    fn a_melee_pet_takes_the_enemy_nearest_itself_and_keeps_it_while_it_stays_near_the_hero() {
        let mut r = run();
        r.summon_pet(0, "Wolf", None);
        let p = r.heroes[0].pos;
        r.pets[0].segment = Segment::standing((p.0 + 20.0, p.1), r.now);
        assert!(r.spawn_roster_enemy(0, Some((p.0 - 5.0, p.1)))); // near the hero, 25 yd from the pet
        assert!(r.spawn_roster_enemy(0, Some((p.0 + 24.0, p.1)))); // 4 yd from the pet
        let near_pet = r.enemies[1].id;
        for e in r.enemies.iter_mut() {
            e.base_speed = 0.0;
            e.health = 1e9;
        }
        r.tick(0.05);
        assert_eq!(r.pets[0].target, Some(near_pet));
        // carried to 30 yd from the hero, it stays the target; past 32 yd it is let go
        let now = r.now;
        let moved = |r: &mut Run, x: f32| {
            let e = r.enemies.iter_mut().find(|e| e.id == near_pet).unwrap();
            e.segment = super::super::enemy_ai::Segment::standing((p.0 + x, p.1), now);
        };
        moved(&mut r, 30.0);
        r.pets[0].next_decision = r.now;
        r.tick(0.05);
        assert_eq!(r.pets[0].target, Some(near_pet));
        moved(&mut r, 33.0);
        r.pets[0].next_decision = r.now;
        r.tick(0.05);
        assert_ne!(r.pets[0].target, Some(near_pet));
        // an enemy near the pet but past 28 yd of the hero is not taken
        r.enemies.retain(|e| e.id == near_pet);
        moved(&mut r, 29.0);
        r.pets[0].target = None;
        r.pets[0].segment = Segment::standing((p.0 + 27.0, p.1), r.now);
        r.pets[0].next_decision = r.now;
        r.tick(0.05);
        assert_eq!(r.pets[0].target, None);
    }

    #[test]
    fn a_melee_pet_chases_fast_and_stops_short_of_its_target() {
        for hero_speed in [1.0, 10.0 / 7.0] {
            let mut r = run();
            r.heroes[0].stats.move_speed = hero_speed;
            r.summon_pet(0, "Wolf", None);
            let p = r.heroes[0].pos;
            assert!(r.spawn_roster_enemy(0, Some((p.0 + 10.0, p.1))));
            r.enemies[0].health = 1e9;
            r.enemies[0].base_speed = 0.0;
            r.tick(0.05);
            let s = r.pets[0].segment;
            let tp = r.enemies[0].position(r.now);
            assert!((dist(s.to, tp) - 1.6).abs() < 1e-3, "stops 1.6 yd short: {}", dist(s.to, tp));
            let want = (2.2 * r.heroes[0].stats.run_speed()).max(16.0);
            assert!((pace(&s) - want).abs() < 1e-3, "chases at {} not {want}", pace(&s));
        }
    }

    #[test]
    fn an_idle_melee_pet_returns_a_little_faster_than_a_ranged_pet_moves() {
        let mut r = run();
        r.summon_pet(0, "Wolf", None);
        r.summon_pet(0, "Imp", None);
        r.tick(0.05);
        let walk = r.heroes[0].stats.run_speed().max(7.0);
        assert!((pace(&r.pets[0].segment) - 1.2 * walk).abs() < 1e-3, "wolf at {}", pace(&r.pets[0].segment));
        assert!((pace(&r.pets[1].segment) - 1.15 * walk).abs() < 1e-3, "imp at {}", pace(&r.pets[1].segment));
    }

    #[test]
    fn a_pet_far_from_the_hero_rushes_back_at_three_times_its_speed() {
        let mut r = run();
        r.summon_pet(0, "Imp", None);
        let p = r.heroes[0].pos;
        r.pets[0].segment = Segment::standing((p.0 + 50.0, p.1), r.now);
        r.tick(0.05);
        let s = r.pets[0].segment;
        assert!(dist(s.to, r.heroes[0].pos) < PET_FOLLOW_DISTANCE + 0.01, "back to its place");
        assert!((pace(&s) - 3.0 * r.heroes[0].stats.run_speed()).abs() < 1e-3);
    }

    #[test]
    fn a_move_shorter_than_a_third_of_a_yard_is_not_started() {
        let mut r = run();
        r.summon_pet(0, "Wolf", None);
        let idle = |r: &mut Run, ticks: u32| {
            for _ in 0..ticks {
                // no enemies: the pet stays idle
                r.enemies.clear();
                r.tick(0.05);
            }
        };
        idle(&mut r, 40);
        let (id, p, facing) = (r.heroes[0].id, r.heroes[0].pos, r.heroes[0].facing);
        let settled = r.pets[0].segment;
        assert!(!settled.moving(r.now), "it reached its place");
        r.hero_moved(id, (p.0 + 0.2, p.1), 0.0, facing);
        idle(&mut r, 10);
        assert_eq!(r.pets[0].segment, settled, "0.2 yd: the pet keeps standing");
        r.hero_moved(id, (p.0 + 0.6, p.1), 0.0, facing);
        idle(&mut r, 10);
        assert_ne!(r.pets[0].segment, settled, "0.6 yd: it moves");
    }

    #[test]
    fn a_summoned_pet_leaves_when_its_time_is_up() {
        let mut r = run();
        r.summon_pet(0, "Imp", Some(r.now + 1.0));
        assert_eq!(r.pets.len(), 1);
        for _ in 0..25 {
            r.tick(0.05);
        }
        assert!(r.pets.is_empty());
    }
}
