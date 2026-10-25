//! The spawn director: a budget that fills over time and is spent on the zone's normal
//! enemies, a population floor, timed elites and timed swarms.
//!
//! Normal enemies are the roster entries with `elite: false`. Elites never come from the budget:
//! only from the elite timer.

use esurv_content::{Enemy, Zone};

use crate::rng::Rng;

/// Most enemies the director places in one tick.
pub const MAX_SPAWNS_PER_TICK: usize = 8;
/// Kill pressure decays with this time constant, seconds.
pub const PRESSURE_TAU: f32 = 12.0;
/// The kill-rate catch-up never speeds spawning up more than this.
pub const MAX_BOOST: f32 = 3.0;
/// Rate factor while a non-final boss is alive.
pub const BOSS_ALIVE_FACTOR: f32 = 0.45;
/// Rate factor while the final boss is alive.
pub const FINAL_BOSS_ALIVE_FACTOR: f32 = 0.3;
/// Budget a run starts with.
pub const START_BUDGET: f32 = 3.0;

/// Population floor: at least `(FLOOR_PER_MINUTE × m + FLOOR_BASE) × density × boss factor`
/// normal enemies alive, never more than `FLOOR_CAP × density`; at most
/// `FLOOR_MAX_PER_TICK × ceil(density)` are added in one tick.
pub const FLOOR_PER_MINUTE: f32 = 1.6;
pub const FLOOR_BASE: f32 = 4.0;
pub const FLOOR_CAP: f32 = 36.0;
pub const FLOOR_MAX_PER_TICK: u32 = 3;

/// The first elite arrives at this run second; then every `max(ELITE_MIN_GAP, ELITE_GAP_BASE −
/// ELITE_GAP_PER_MINUTE × m)` seconds, one unlocked elite picked evenly.
pub const FIRST_ELITE: f32 = 240.0;
pub const ELITE_GAP_BASE: f32 = 55.0;
pub const ELITE_GAP_PER_MINUTE: f32 = 2.5;
pub const ELITE_MIN_GAP: f32 = 25.0;

/// The first swarm comes at this run second; then every `max(SWARM_MIN_GAP, SWARM_GAP_BASE −
/// SWARM_GAP_PER_MINUTE × m)` seconds. A swarm is `density × (SWARM_PER_MINUTE × m + SWARM_BASE)
/// × SWARM_SCALE` copies of one cheap unlocked normal enemy (spawn cost ≤ `SWARM_MAX_COST`),
/// placed evenly on a ring of `SWARM_RADIUS` yards around the hero.
pub const FIRST_SWARM: f32 = 80.0;
pub const SWARM_GAP_BASE: f32 = 80.0;
pub const SWARM_GAP_PER_MINUTE: f32 = 2.0;
pub const SWARM_MIN_GAP: f32 = 45.0;
pub const SWARM_PER_MINUTE: f32 = 2.5;
pub const SWARM_BASE: f32 = 12.0;
pub const SWARM_SCALE: f32 = 0.6;
pub const SWARM_MAX_COST: f32 = 1.2;
pub const SWARM_RADIUS: f32 = 26.0;

/// What the director needs to know about the run this tick.
#[derive(Debug, Clone, Copy)]
pub struct DirectorInput {
    /// Seconds since the run started.
    pub elapsed: f32,
    pub dt: f32,
    /// Enemy density run option, percent (100..=500).
    pub density_pct: u16,
    /// The zone's spawn budget per second at 100 % density right now (`balance.ron` `spawn_budget`).
    pub budget_per_second: f32,
    /// `None` when no boss is alive; `Some(true)` when the final boss is among the living.
    pub boss_alive: Option<bool>,
    /// Where Endless's growth counts from (run second: the latest final boss death), while
    /// Endless is on.
    pub endless_since: Option<f32>,
}

/// A timed swarm: which roster enemy, how many, and where around the hero.
#[derive(Debug, Clone, PartialEq)]
pub struct Swarm {
    pub enemy: usize,
    pub count: u32,
    /// Offsets from the hero, yards.
    pub offsets: Vec<(f32, f32)>,
}

#[derive(Debug, Clone)]
pub struct Director {
    budget: f32,
    pressure: f32,
    next_elite: f32,
    next_swarm: f32,
}

impl Default for Director {
    fn default() -> Self {
        Self { budget: START_BUDGET, pressure: 0.0, next_elite: FIRST_ELITE, next_swarm: FIRST_SWARM }
    }
}

fn boss_factor(boss_alive: Option<bool>) -> f32 {
    match boss_alive {
        Some(true) => FINAL_BOSS_ALIVE_FACTOR,
        Some(false) => BOSS_ALIVE_FACTOR,
        None => 1.0,
    }
}

impl Director {
    /// Base spawn rate in cost units per second, before the kill-rate boost.
    pub fn rate(input: &DirectorInput) -> f32 {
        let mut rate = (input.density_pct as f32 / 100.0) * input.budget_per_second * boss_factor(input.boss_alive);
        if let Some(since) = input.endless_since {
            let minutes = ((input.elapsed - since) / 60.0).max(0.0);
            rate *= 1.5 + 0.1 * minutes;
        }
        rate
    }

    /// A normal enemy died (elites, rares and bosses do not count): its spawn cost feeds the
    /// kill-rate catch-up.
    pub fn on_kill(&mut self, spawn_cost: f32) {
        self.pressure += spawn_cost;
    }

    pub fn budget(&self) -> f32 {
        self.budget
    }

    /// The kill pressure: recent normal kills' spawn costs, decaying.
    pub fn pressure(&self) -> f32 {
        self.pressure
    }

    /// When the next swarm comes (run second).
    pub fn next_swarm(&self) -> f32 {
        self.next_swarm
    }

    /// Advances one tick and returns the roster indices to spawn, in order. `try_spawn` places
    /// one enemy and says whether it succeeded; a failure ends this tick's spawning.
    pub fn tick(
        &mut self,
        input: &DirectorInput,
        zone: &Zone,
        rng: &mut Rng,
        mut try_spawn: impl FnMut(usize) -> bool,
    ) -> Vec<usize> {
        self.pressure *= (-input.dt / PRESSURE_TAU).exp();
        let rate = Self::rate(input).max(0.001);
        let kill_rate = self.pressure / PRESSURE_TAU * 1.25;
        let boost = (kill_rate / rate).clamp(1.0, MAX_BOOST);
        self.budget += input.dt * rate * boost;

        let minute = input.elapsed / 60.0;
        let mut spawned = Vec::new();
        let mut last_cost = None;
        while self.budget >= 1.0 && spawned.len() < MAX_SPAWNS_PER_TICK {
            let weights = roster_weights(&zone.enemies, minute);
            let Some(index) = rng.weighted(&weights) else { break };
            let cost = zone.enemies[index].spawn_cost;
            last_cost = Some(cost);
            if cost > self.budget || !try_spawn(index) {
                break;
            }
            self.budget -= cost;
            spawned.push(index);
        }
        if let Some(cost) = last_cost {
            self.budget = self.budget.min(cost);
        }
        spawned
    }

    /// How many normal enemies the population floor adds this tick, given how many are alive.
    pub fn floor_shortfall(input: &DirectorInput, normals_alive: u32) -> u32 {
        let m = input.elapsed / 60.0;
        let d = input.density_pct as f32 / 100.0;
        let target = ((FLOOR_PER_MINUTE * m + FLOOR_BASE) * d * boss_factor(input.boss_alive)).min(FLOOR_CAP * d);
        let missing = (target as u32).saturating_sub(normals_alive);
        missing.min(d.ceil() as u32 * FLOOR_MAX_PER_TICK)
    }

    /// The elite to send in now, if the elite timer has run out (the timer restarts either way).
    pub fn elite_due(&mut self, input: &DirectorInput, zone: &Zone, rng: &mut Rng) -> Option<usize> {
        if input.elapsed < self.next_elite {
            return None;
        }
        let m = input.elapsed / 60.0;
        self.next_elite = input.elapsed + (ELITE_GAP_BASE - ELITE_GAP_PER_MINUTE * m).max(ELITE_MIN_GAP);
        let elites: Vec<usize> = zone
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.elite && !e.only_summoned && e.appears_at_minute <= m)
            .map(|(i, _)| i)
            .collect();
        (!elites.is_empty()).then(|| elites[(rng.next_u64() % elites.len() as u64) as usize])
    }

    /// The swarm to send in now, if the swarm timer has run out.
    pub fn swarm_due(&mut self, input: &DirectorInput, zone: &Zone, rng: &mut Rng) -> Option<Swarm> {
        if input.elapsed < self.next_swarm {
            return None;
        }
        let m = input.elapsed / 60.0;
        self.next_swarm = input.elapsed + (SWARM_GAP_BASE - SWARM_GAP_PER_MINUTE * m).max(SWARM_MIN_GAP);
        let cheap: Vec<usize> = zone
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.elite && !e.only_summoned && e.appears_at_minute <= m && e.spawn_cost <= SWARM_MAX_COST)
            .map(|(i, _)| i)
            .collect();
        if cheap.is_empty() {
            return None;
        }
        let enemy = cheap[(rng.next_u64() % cheap.len() as u64) as usize];
        let count = ((input.density_pct as f32 / 100.0) * (SWARM_PER_MINUTE * m + SWARM_BASE) * SWARM_SCALE) as u32;
        let offsets = (0..count)
            .map(|i| {
                let a = i as f32 / count as f32 * std::f32::consts::TAU;
                (a.cos() * SWARM_RADIUS, a.sin() * SWARM_RADIUS)
            })
            .collect();
        Some(Swarm { enemy, count, offsets })
    }
}

/// How fast an enemy's weight grows per minute since it unlocked.
pub const WEIGHT_GROWTH_PER_MINUTE: f32 = 0.5;
/// An enemy's weight when it has just unlocked.
pub const WEIGHT_AT_UNLOCK: f32 = 1.0;
/// The most an enemy's weight can grow to (before dividing by its cost).
pub const WEIGHT_CAP: f32 = 4.0;

/// Relative chance of each roster entry being picked at `minute`: elites weigh 0 (they only
/// come from the elite timer); an unlocked normal enemy weighs
/// `min(4, 1 + 0.5 × minutes since it unlocked) / spawn cost`; a locked one weighs 0. Newly
/// unlocked enemies start rare and become common over six minutes; costly ones stay rarer.
pub fn roster_weights(roster: &[Enemy], minute: f32) -> Vec<f32> {
    roster
        .iter()
        .map(|e| {
            if e.elite || e.only_summoned || e.appears_at_minute > minute {
                return 0.0;
            }
            let grown = (minute - e.appears_at_minute).max(0.0) * WEIGHT_GROWTH_PER_MINUTE + WEIGHT_AT_UNLOCK;
            grown.min(WEIGHT_CAP) / e.spawn_cost
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(elapsed: f32) -> DirectorInput {
        let budget = crate::game::enemy::time_curve(&[(0.0, 0.5), (4.0, 1.5), (8.0, 3.0), (12.0, 5.0), (16.0, 7.5), (20.0, 10.5)], elapsed / 60.0);
        DirectorInput { elapsed, dt: 0.1, density_pct: 100, budget_per_second: budget, boss_alive: None, endless_since: None }
    }

    #[test]
    fn base_rate_follows_the_curve() {
        assert!((Director::rate(&input(0.0)) - 0.5).abs() < 1e-6);
        // m = 10: halfway from 3 (minute 8) to 5 (minute 12)
        assert!((Director::rate(&input(600.0)) - 4.0).abs() < 1e-5);
    }

    #[test]
    fn bosses_and_density_scale_the_rate() {
        let mut i = input(600.0);
        i.density_pct = 200;
        assert!((Director::rate(&i) - 8.0).abs() < 1e-4);
        i.boss_alive = Some(false);
        assert!((Director::rate(&i) - 8.0 * 0.45).abs() < 1e-4);
        i.boss_alive = Some(true);
        assert!((Director::rate(&i) - 8.0 * 0.3).abs() < 1e-4);
    }

    fn enemy(appears_at_minute: f32, spawn_cost: f32) -> Enemy {
        Enemy {
            name: String::new(),
            entry: 0,
            display_id: 0,
            scale: 1.0,
            creature_type: esurv_content::CreatureType::Beast,
            toughness: 1.0,
            attack_interval: 1.0,
            move_speed: 1.0,
            spawn_cost,
            appears_at_minute,
            elite: false,
            ranged: None,
            looks_like: None,
            rank: None,
            weapons: Default::default(),
            only_summoned: false,
            focus_as_elite: false,
        }
    }

    #[test]
    fn elites_never_come_from_the_budget() {
        let mut e = enemy(0.0, 12.0);
        e.elite = true;
        assert_eq!(roster_weights(&[enemy(0.0, 1.0), e], 10.0), [4.0, 0.0]);
    }

    #[test]
    fn population_floor() {
        // minute 5, density 100: (8 + 4) = 12 wanted
        assert_eq!(Director::floor_shortfall(&input(300.0), 0), 3);
        assert_eq!(Director::floor_shortfall(&input(300.0), 10), 2);
        assert_eq!(Director::floor_shortfall(&input(300.0), 12), 0);
        // minute 30: capped at 36
        assert_eq!(Director::floor_shortfall(&input(1800.0), 34), 2);
        let mut boss = input(300.0);
        boss.boss_alive = Some(false);
        // 12 x 0.45 = 5.4 -> 5
        assert_eq!(Director::floor_shortfall(&boss, 4), 1);
    }

    #[test]
    fn elite_and_swarm_timers() {
        let c = esurv_content::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let zone = &c.zones[0];
        let mut d = Director::default();
        let mut rng = Rng::new(1);
        assert!(d.elite_due(&input(200.0), zone, &mut rng).is_none());
        let e = d.elite_due(&input(240.0), zone, &mut rng).unwrap();
        assert!(zone.enemies[e].elite);
        // next elite at 240 + (55 - 10) = 285
        assert!(d.elite_due(&input(284.0), zone, &mut rng).is_none());
        assert!(d.swarm_due(&input(79.0), zone, &mut rng).is_none());
        let s = d.swarm_due(&input(120.0), zone, &mut rng).unwrap();
        // minute 2: (5 + 12) x 0.6 = 10.2 -> 10
        assert_eq!(s.count, 10);
        assert!(zone.enemies[s.enemy].spawn_cost <= 1.2 && !zone.enemies[s.enemy].elite);
        let (x, y) = s.offsets[0];
        assert!(((x * x + y * y).sqrt() - 26.0).abs() < 1e-3);
    }

    #[test]
    fn roster_weights_grow_after_unlock_and_divide_by_cost() {
        let roster = [enemy(0.0, 1.0), enemy(2.0, 2.0), enemy(9.0, 3.0)];
        // minute 4: first unlocked 4 min ago -> 1+2 = 3; second 2 min ago -> 2/2 = 1; third locked
        assert_eq!(roster_weights(&roster, 4.0), [3.0, 1.0, 0.0]);
        // minute 20: everyone capped at 4, divided by cost
        let w = roster_weights(&roster, 20.0);
        assert_eq!(w, [4.0, 2.0, 4.0 / 3.0]);
    }

    #[test]
    fn endless_grows_by_a_tenth_per_minute() {
        let mut i = input(600.0);
        i.endless_since = Some(480.0);
        assert!((Director::rate(&i) - 4.0 * 1.7).abs() < 1e-4);
    }
}
