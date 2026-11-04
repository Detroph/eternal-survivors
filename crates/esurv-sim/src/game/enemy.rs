//! Enemy stats at spawn.

use esurv_content::{Balance, Boss, Enemy, EnemyType, Zone};

pub use super::challenge::Challenge;

/// Per minute of Endless, health and damage multiply by these.
pub const ENDLESS_HEALTH_GROWTH: f32 = 1.22;
pub const ENDLESS_DAMAGE_GROWTH: f32 = 1.12;
/// Fewest and most roster enemies alive at once; between them the cap is `density% × 0.5`.

/// The run-wide difficulty multiplier every run starts with (confirmed).
pub const RUN_FACTOR: f32 = 1.0;

/// The run options, as whole percentages, and the run's Challenge Mode (none in Normal Mode).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunOptions {
    pub density_pct: u16,
    pub health_pct: u16,
    pub damage_pct: u16,
    pub speed_pct: u16,
    /// Challenge Mode (`challenge`): every enemy at Ahn'Qiraj's D, its final health and
    /// damage × the challenge's flat factor.
    pub challenge: Option<Challenge>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self { density_pct: 100, health_pct: 100, damage_pct: 100, speed_pct: 100, challenge: None }
    }
}

impl RunOptions {
    /// The options as the sliders of `run_options.ron` can set them: each kept within its slider
    /// and on one of its steps (a value between two steps goes to the lower). An option with no
    /// slider is kept as it is.
    pub fn on_the_sliders(self, sliders: &[esurv_content::RunOption]) -> Self {
        let set = |id: &str, v: u16| match sliders.iter().find(|s| s.id == id) {
            Some(s) => {
                let step = s.step_pct.max(1);
                let v = v.clamp(s.min_pct, s.max_pct.max(s.min_pct));
                s.min_pct + (v - s.min_pct) / step * step
            }
            None => v,
        };
        Self { density_pct: set("density", self.density_pct), health_pct: set("health", self.health_pct), damage_pct: set("damage", self.damage_pct), speed_pct: set("speed", self.speed_pct), challenge: self.challenge }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpawnStats {
    pub health: f32,
    pub damage: f32,
    pub move_speed: f32,
    pub collision_radius: f32,
    /// Seconds until the first melee swing.
    pub first_swing: f32,
}

/// A run clock's multiplier at `minute`: straight lines between the (minute, multiplier) points,
/// and past the last point the last step's pace goes on.
pub fn time_curve(points: &[(f32, f32)], minute: f32) -> f32 {
    let m = minute.max(0.0);
    let i = points.iter().position(|p| m <= p.0).unwrap_or(points.len() - 1).max(1);
    let (a, b) = (points[i - 1], points[i]);
    a.1 + (m - a.0) * (b.1 - a.1) / (b.0 - a.0)
}

/// The difficulty dial: what a run multiplies every enemy's health and damage by at the very end
/// of the formula, the same on every kind: its health and damage options, Endless's growth and, in
/// Challenge Mode, the challenge's flat `enemy_factor` (`challenge_factor` × the slider).
pub fn difficulty_dial(endless_minutes: f32, options: &RunOptions) -> (f32, f32) {
    let challenge = options.challenge.map_or(1.0, |c| c.enemy_factor);
    (
        (options.health_pct as f32 / 100.0) * ENDLESS_HEALTH_GROWTH.powf(endless_minutes) * challenge,
        (options.damage_pct as f32 / 100.0) * ENDLESS_DAMAGE_GROWTH.powf(endless_minutes) * challenge,
    )
}

/// Health and damage of an enemy of kind `kind` that appears at `minute` of a run in `zone`,
/// before its role: the one stat formula (`balance.ron`). Every kind rides the same zone curve
/// (`zone_scaling`) and the same run clocks; the kind is a plain multiplier, and the difficulty
/// dial (`difficulty_dial`) comes last. In Challenge Mode the zone's D is Ahn'Qiraj's.
///
/// `run_factor` R is a run-wide multiplier on zone difficulty: health × R and damage × √R. Every
/// run starts it at `RUN_FACTOR` and nothing changes it, but it stays a knob a mode can use.
pub fn health_and_damage(
    balance: &Balance,
    kind: EnemyType,
    zone: &Zone,
    minute: f32,
    endless_minutes: f32,
    options: &RunOptions,
    run_factor: f32,
) -> (f32, f32) {
    let scaling = &balance.zone_scaling;
    let d = options.challenge.map_or(zone.difficulty, |c| c.difficulty);
    let (health_dial, damage_dial) = difficulty_dial(endless_minutes, options);
    // Bosses and final bosses read the health clock to `boss_clock_power`
    let clock_power = if matches!(kind, EnemyType::Boss | EnemyType::FinalBoss) { balance.boss_clock_power } else { 1.0 };
    let kind = balance.multipliers(kind);
    let health = balance.base_health * kind.health * scaling.health_factor(d) * time_curve(&balance.health_clock, minute).powf(clock_power) * run_factor * health_dial;
    let damage = balance.base_damage * kind.damage * scaling.damage_factor(d) * time_curve(&balance.damage_clock, minute) * run_factor.sqrt() * damage_dial;
    (health, damage)
}

/// Stats of roster `enemy` spawned at `minute` of a run in `zone`: the normal kind times its role's
/// multiplier (`toughness`), or the elite kind (elites have no role).
pub fn spawn_stats(
    enemy: &Enemy,
    zone: &Zone,
    balance: &Balance,
    minute: f32,
    endless_minutes: f32,
    options: &RunOptions,
    run_factor: f32,
) -> SpawnStats {
    let kind = if enemy.elite { EnemyType::Elite } else { EnemyType::Normal };
    let (health, damage) = health_and_damage(balance, kind, zone, minute, endless_minutes, options, run_factor);
    let health = if enemy.elite { health } else { health * enemy.toughness };
    SpawnStats {
        health,
        damage,
        move_speed: options.speed_pct as f32 / 100.0 * balance.enemy_speed * enemy.move_speed,
        collision_radius: 1.2 * enemy.scale + 2.2,
        first_swing: 0.5 * enemy.attack_interval,
    }
}

/// Seconds between a boss's swings.
pub const BOSS_SWING: f32 = 2.0;

/// Stats of `boss` when it arrives at `minute`: the boss kind (the final boss kind for the
/// final boss) on the run clocks. Its speed is its own times the game's enemy speed; its reach is
/// `1.5 × scale + 3.5`, at most 9.
pub fn boss_stats(boss: &Boss, zone: &Zone, balance: &Balance, minute: f32, endless_minutes: f32, options: &RunOptions, run_factor: f32) -> SpawnStats {
    let kind = if boss.is_final { EnemyType::FinalBoss } else { EnemyType::Boss };
    let (health, damage) = health_and_damage(balance, kind, zone, minute, endless_minutes, options, run_factor);
    SpawnStats {
        health: health * boss.health_share,
        damage,
        move_speed: options.speed_pct as f32 / 100.0 * balance.enemy_speed * boss.move_speed,
        collision_radius: (1.5 * boss.scale + 3.5).min(9.0),
        first_swing: BOSS_SWING,
    }
}

/// Most roster enemies that may be alive at once: `live_cap.0` at 100 % density, rising in a
/// straight line to `live_cap.1` at 500 %.
pub fn live_cap(density_pct: u16, balance: &esurv_content::Balance) -> usize {
    let (low, high) = balance.live_cap;
    let share = ((density_pct as f32 - 100.0) / 400.0).clamp(0.0, 1.0);
    (low + (high - low) * share).round() as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_content::{Content, CreatureType, Spawn, Weather, WeatherKind};

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    fn zone(difficulty: f32) -> Zone {
        Zone {
            id: "t".into(),
            name: String::new(),
            subtitle: String::new(),
            description: String::new(),
            map_id: 0,
            area_id: 0,
            spawn: Spawn { x: 0.0, y: 0.0, z: 0.0, facing: 0.0 },
            arena_radius: 80.0,
            arena_centre: None,
            walls: vec![],
            doors: vec![],
            props: vec![],
            platforms: vec![],
            by_faction: None,
            enemy_spawn_ring: esurv_content::ENEMY_SPAWN_RING,
            boss_spawn_ring: esurv_content::BOSS_SPAWN_RING,
            hour: 0,
            weather: Weather { kind: WeatherKind::None, intensity: 0.0 },
            difficulty,
            enemies: vec![],
            bosses: vec![],
        }
    }

    fn vermin(elite: bool, toughness: f32) -> Enemy {
        Enemy {
            name: String::new(),
            entry: 600,
            display_id: 26,
            scale: 0.9,
            creature_type: CreatureType::Humanoid,
            toughness,
            role: Default::default(),
            attack_interval: 1.6,
            move_speed: 5.0,
            spawn_cost: 1.0,
            appears_at_minute: 0.0,
            elite,
            ranged: None,
            looks_like: None,
            rank: None,
            weapons: Default::default(),
            only_summoned: false,
            focus_as_elite: false,
        }
    }

    fn close(got: f32, want: f32) -> bool {
        (got - want).abs() <= want.abs() * 1e-5
    }

    #[test]
    fn every_kind_follows_the_one_formula() {
        let b = content().balance;
        let s = &b.zone_scaling;
        let o = RunOptions::default();
        // (kind, D, minute, health clock, damage clock)
        let cases = [
            (EnemyType::Normal, 1.0f32, 0.0, 1.0, 1.0),
            (EnemyType::Normal, 4.0, 8.0, 7.0, 2.5),
            (EnemyType::Elite, 2.0, 10.0, 12.5, 3.5),
            (EnemyType::Rare, 3.0, 4.0, 2.5, 1.5),
            (EnemyType::Rare, 8.0, 12.0, 18.0, 4.5),
            (EnemyType::Boss, 1.0, 3.0, 2.125, 1.375),
            (EnemyType::Boss, 9.0, 16.0, 36.0, 7.0),
            (EnemyType::FinalBoss, 10.0, 16.0, 36.0, 7.0),
        ];
        for (kind, d, m, hc, dc) in cases {
            // 240 × kind × D^health_power × health clock, 8 × kind × (D^health_power)^damage_share ×
            // damage clock: the same curve and clocks for every kind
            let k = b.multipliers(kind);
            let hc = if matches!(kind, EnemyType::Boss | EnemyType::FinalBoss) { f32::powf(hc, b.boss_clock_power) } else { hc };
            let h = 240.0 * k.health * d.powf(s.health_power) * hc;
            let dmg = 8.0 * k.damage * d.powf(s.health_power * s.damage_share) * dc;
            let (gh, gd) = health_and_damage(&b, kind, &zone(d), m, 0.0, &o, RUN_FACTOR);
            assert!(close(gh, h) && close(gd, dmg), "{kind:?} D {d} minute {m}: got ({gh}, {gd}), want ({h}, {dmg})");
        }
    }

    #[test]
    fn every_kind_is_a_plain_multiple_of_a_normal_enemy() {
        let b = content().balance;
        let o = RunOptions::default();
        for (d, m) in [(1.0, 3.0), (4.0, 9.5), (10.0, 19.0)] {
            let (nh, nd) = health_and_damage(&b, EnemyType::Normal, &zone(d), m, 0.0, &o, RUN_FACTOR);
            for kind in [EnemyType::Elite, EnemyType::Rare, EnemyType::Boss, EnemyType::FinalBoss] {
                let (h, dmg) = health_and_damage(&b, kind, &zone(d), m, 0.0, &o, RUN_FACTOR);
                let k = b.multipliers(kind);
                // Bosses read the health clock to boss_clock_power
                let clock = if matches!(kind, EnemyType::Boss | EnemyType::FinalBoss) { time_curve(&b.health_clock, m).powf(b.boss_clock_power - 1.0) } else { 1.0 };
                assert!(close(h, nh * k.health * clock) && close(dmg, nd * k.damage), "{kind:?} at D {d}, minute {m}");
            }
        }
    }

    #[test]
    fn the_zone_curve_is_a_power_of_d_and_damage_a_power_of_it() {
        let mut b = content().balance;
        (b.zone_scaling.health_power, b.zone_scaling.damage_share) = (2.0, 0.5);
        let o = RunOptions::default();
        let at = |kind, d: f32| health_and_damage(&b, kind, &zone(d), 0.0, 0.0, &o, RUN_FACTOR);
        // D 3: health × 3² = 9, damage × 9^0.5 = 3, every kind alike
        assert!(close(at(EnemyType::Normal, 3.0).0, 240.0 * 9.0) && close(at(EnemyType::Normal, 3.0).1, 8.0 * 3.0));
        assert!(close(at(EnemyType::Boss, 3.0).0, 240.0 * b.boss.health * 9.0) && close(at(EnemyType::FinalBoss, 3.0).1, 8.0 * b.final_boss.damage * 3.0));
    }

    #[test]
    fn options_endless_and_run_factor_scale_the_formula() {
        let b = content().balance;
        let o = RunOptions { health_pct: 200, damage_pct: 150, ..RunOptions::default() };
        let (h, d) = health_and_damage(&b, EnemyType::Elite, &zone(1.0), 0.0, 2.0, &o, 4.0);
        assert!(close(h, 240.0 * b.elite.health * 2.0 * 1.22 * 1.22 * 4.0));
        assert!(close(d, 8.0 * b.elite.damage * 1.5 * 1.12 * 1.12 * 2.0));
        // the dial is the options and Endless, last
        let (hd, dd) = difficulty_dial(2.0, &o);
        assert!(close(hd, 2.0 * 1.22 * 1.22) && close(dd, 1.5 * 1.12 * 1.12));
    }

    #[test]
    fn toughness_scales_health_only() {
        let b = content().balance;
        let o = RunOptions::default();
        let s = spawn_stats(&vermin(false, 0.5), &zone(1.0), &b, 0.0, 0.0, &o, RUN_FACTOR);
        assert!(close(s.health, 120.0) && close(s.damage, 8.0));
        let t = spawn_stats(&vermin(false, 3.0), &zone(1.0), &b, 0.0, 0.0, &o, RUN_FACTOR);
        assert!(close(t.health, 720.0) && close(t.damage, 8.0));
        // elites have no role
        let e = spawn_stats(&vermin(true, 3.0), &zone(1.0), &b, 0.0, 0.0, &o, RUN_FACTOR);
        assert!(close(e.health, 240.0 * b.elite.health) && close(e.damage, 8.0 * b.elite.damage));
        // the vermin's 5 yd/s times the game's enemy speed
        assert!((s.move_speed - 5.0 * b.enemy_speed).abs() < 1e-4);
        assert!((s.collision_radius - (1.2 * 0.9 + 2.2)).abs() < 1e-5);
        assert!((s.first_swing - 0.8).abs() < 1e-6);
    }

    #[test]
    fn bosses_follow_the_clocks_at_their_arrival() {
        let c = content();
        let z = c.zone("elwynn").unwrap();
        let o = RunOptions::default();
        let hogger = &z.bosses[0];
        let s = boss_stats(hogger, z, &c.balance, hogger.spawn_minute, 0.0, &o, RUN_FACTOR);
        // Elwynn (D 1): the boss kind on the health clock to boss_clock_power
        assert!(close(s.health, 240.0 * c.balance.boss.health * hogger.health_share * 2.125f32.powf(c.balance.boss_clock_power)) && close(s.damage, 8.0 * c.balance.boss.damage * 1.375), "{s:?}");
        assert!((s.collision_radius - (1.5 * hogger.scale + 3.5)).abs() < 1e-5);
        // the same boss later in the run is stronger, by the gentler boss clock
        let late = boss_stats(hogger, z, &c.balance, 8.0, 0.0, &o, RUN_FACTOR);
        assert!(close(late.health / s.health, (7.0f32 / 2.125).powf(c.balance.boss_clock_power)) && close(late.damage / s.damage, 2.5 / 1.375));
        let last = z.bosses.last().unwrap();
        let f = boss_stats(last, z, &c.balance, 16.0, 0.0, &o, RUN_FACTOR);
        assert!(close(f.health, 240.0 * c.balance.final_boss.health * last.health_share * 36f32.powf(c.balance.boss_clock_power)) && close(f.damage, 8.0 * c.balance.final_boss.damage * 7.0));
    }

    #[test]
    fn the_clocks() {
        let b = content().balance;
        // the listed points hold exactly, and past each clock's last point its last step's pace goes on
        for (m, h, d) in [(0.0, 1.0, 1.0), (4.0, 2.5, 1.5), (8.0, 7.0, 2.5), (10.0, 12.5, 3.5), (12.0, 18.0, 4.5), (16.0, 36.0, 7.0), (18.0, 50.5, 8.25), (20.0, 65.0, 9.5), (22.0, 79.5, 10.75)] {
            assert!((time_curve(&b.health_clock, m) - h).abs() < 1e-4 && (time_curve(&b.damage_clock, m) - d).abs() < 1e-4, "minute {m}");
        }
    }

    #[test]
    fn live_cap_follows_density() {
        let b = esurv_content::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap().balance;
        assert_eq!(live_cap(100, &b), 60);
        assert_eq!(live_cap(300, &b), 90);
        assert_eq!(live_cap(500, &b), 120);
    }

    #[test]
    fn options_stay_on_their_sliders() {
        let content = content();
        let sliders = &content.run_options;
        let o = RunOptions { density_pct: 130, health_pct: 900, damage_pct: 55, speed_pct: 149, challenge: None }.on_the_sliders(sliders);
        assert_eq!(o, RunOptions { density_pct: 125, health_pct: 500, damage_pct: 100, speed_pct: 145, challenge: None });
        let max = RunOptions { density_pct: 500, health_pct: 500, damage_pct: 300, speed_pct: 150, challenge: None };
        assert_eq!(max.on_the_sliders(sliders), max);
    }
}
