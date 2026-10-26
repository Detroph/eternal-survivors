//! The most talent experience run time can earn: a save's sanity check holds a class's talent
//! experience to what it held before its run time was tracked plus this ceiling of the run time
//! tracked since.
//!
//! The ceiling is an absolute upper limit, not an average: it is the best rate a god-mode hero
//! earned (every enemy, elite, rare and boss killed the tick it appeared, never falling, every
//! zone and density, through the win and on into Endless: `measure_god_mode`), at the hardest
//! zone bonus and the biggest run-option bonus, with the owner's margin on top.

use esurv_content::Content;

use super::challenge::CHALLENGE_ZONE;
use super::rewards::MAX_OPTION_BONUS;

/// The hardest zone planned: 12 zones, D 1-12 (the content may not have it yet). A zone added
/// later pays no more than this unless it is harder still (then the content's own D counts).
pub const PLANNED_TOP_DIFFICULTY: f32 = 12.0;
/// The most base talent experience (time, kills, bosses, the win; before the zone and run-option
/// bonuses) a god-mode hero earned in any minute of run time: 13.49 a second, Scarlet Monastery at
/// 125 % density, measured over three hours (`measure_god_mode`, 2026-10-26). A longer run's
/// average stays under it: 10.6 a second over three hours.
pub const MOST_BASE_EXPERIENCE_PER_SECOND: f64 = 13.5;
/// Margin on top of the measured best (our rule, the owner's: 10 %, since the god-mode hero
/// already kills everything at once).
pub const CEILING_MARGIN: f64 = 1.1;

/// The highest talent experience zone bonus any run can have: × D^`talent_exponent` at the
/// hardest of the planned top zone, the content's hardest zone and Challenge Mode (Ahn'Qiraj's D,
/// × `challenge_factor`).
pub fn top_zone_factor(content: &Content) -> f64 {
    let scaling = &content.balance.zone_scaling;
    let hardest = content.zones.iter().map(|z| z.difficulty).fold(PLANNED_TOP_DIFFICULTY, f32::max);
    let challenge = content
        .zone(CHALLENGE_ZONE)
        .map_or(0.0, |z| scaling.talent_factor(z.difficulty) * content.balance.challenge_factor);
    scaling.talent_factor(hardest).max(challenge) as f64
}

/// The most talent experience any runs totalling `seconds` of run time can earn: a class holding
/// more than it did before its run time was tracked plus this is held to that. No run time,
/// nothing.
pub fn talent_experience_ceiling(content: &Content, seconds: f64) -> u64 {
    let rate = MOST_BASE_EXPERIENCE_PER_SECOND * top_zone_factor(content) * MAX_OPTION_BONUS as f64;
    (CEILING_MARGIN * rate * seconds.max(0.0)).ceil() as u64
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::{Content, Milestone};

    use crate::game::casting::HitSource;
    use crate::game::enemy::RunOptions;
    use crate::game::run::{Flat, HeroSetup, Phase, Run};

    const DT: f32 = 0.05;

    fn content() -> Arc<Content> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        Arc::new(Content::load(dir).expect("content loads"))
    }

    /// A god-mode hero: every enemy on the field dies to it at the end of every tick (bosses
    /// too, at once), and it never falls.
    struct GodRun {
        run: Run,
    }

    impl GodRun {
        fn new(content: Arc<Content>, zone: &str, options: RunOptions, seed: u64) -> Self {
            let setup = HeroSetup { class: "warrior".into(), race: "human".into(), ..Default::default() };
            let mut run = Run::new(content, zone, options, &[setup], Box::new(Flat), seed).unwrap();
            let cheat = Milestone::CheatDeath { cooldown: 0.0, health: 1.0, immune_seconds: 0.0 };
            run.heroes[0].base_milestones.push(cheat.clone());
            run.heroes[0].milestones.push(cheat);
            let (h, p) = (run.heroes[0].id, run.heroes[0].pos);
            run.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
            Self { run }
        }

        fn tick(&mut self) {
            let r = &mut self.run;
            r.tick(DT);
            r.take_first_offers();
            r.heroes[0].health = r.heroes[0].stats.max_health;
            let mut guard = 0;
            while !r.enemies.is_empty() && guard < 10_000 {
                r.kill(r.enemies.len() - 1, Some(0), HitSource::Other);
                guard += 1;
            }
            r.drain_events();
            if r.phase == Phase::Victory {
                r.continue_endless();
            }
        }

        fn total(&self) -> u32 {
            self.run.summary(0).experience_parts.base()
        }
    }

    #[test]
    #[ignore]
    fn measure_god_mode() {
        let c = content();
        let densities = [100u16, 125, 150, 200, 225];
        let only = std::env::var("ESURV_MEASURE_ZONE").ok();
        for z in c.zones.iter().filter(|z| only.as_ref().is_none_or(|o| *o == z.id)) {
            for &d in &densities {
                let options = RunOptions { density_pct: d, ..Default::default() };
                let mut g = GodRun::new(c.clone(), &z.id, options, 7);
                let mut best_prefix = (0.0f64, 0.0f32);
                let mut win_at = None;
                let mut window: std::collections::VecDeque<(f32, u32)> = Default::default();
                let mut best_window = (0.0f64, 0.0f32);
                let mut kills_per_tick_max = 0u32;
                let end = 60.0 * std::env::var("ESURV_MEASURE_MINUTES").ok().and_then(|m| m.parse().ok()).unwrap_or(60.0);
                while g.run.now < end {
                    let k0 = g.run.heroes[0].kills;
                    g.tick();
                    kills_per_tick_max = kills_per_tick_max.max(g.run.heroes[0].kills - k0);
                    let t = g.run.now;
                    let x = g.total();
                    if t >= 30.0 {
                        let rate = x as f64 / t as f64;
                        if rate > best_prefix.0 {
                            best_prefix = (rate, t);
                        }
                    }
                    if win_at.is_none() && g.run.won_at.is_some() {
                        win_at = Some(t);
                    }
                    window.push_back((t, x));
                    while window.front().is_some_and(|(s, _)| t - s > 60.0) {
                        window.pop_front();
                    }
                    if let Some(&(s, x0)) = window.front() {
                        if t - s > 59.0 {
                            let rate = (x - x0) as f64 / (t - s) as f64;
                            if rate > best_window.0 {
                                best_window = (rate, t);
                            }
                        }
                    }
                }
                let s = g.run.summary(0);
                println!(
                    "{:14} d{:3}: prefix {:7.3}/s at {:6.0}s  60s-window {:7.3}/s at {:6.0}s  win {:?}  kills {} bosses {} max kills/tick {}  parts {:?}",
                    z.id, d, best_prefix.0, best_prefix.1, best_window.0, best_window.1, win_at, s.kills, s.bosses, kills_per_tick_max, s.experience_parts
                );
            }
        }
    }

    /// A god-mode hero in the hardest zone, at the density that pays the most and every run option
    /// at its top, earns under the ceiling at every moment of its run, through the win.
    #[test]
    fn a_god_mode_run_stays_under_the_ceiling() {
        let c = content();
        assert_eq!(super::talent_experience_ceiling(&c, 0.0), 0);
        let zone = c.zones.iter().max_by(|a, b| a.difficulty.total_cmp(&b.difficulty)).unwrap();
        let top = |id: &str| c.run_options.iter().find(|o| o.id == id).map_or(100, |o| o.max_pct);
        let options = RunOptions { density_pct: 125, health_pct: top("health"), damage_pct: top("damage"), speed_pct: top("speed"), challenge: None };
        let mut g = GodRun::new(c.clone(), &zone.id, options, 7);
        while g.run.now < 25.0 * 60.0 {
            g.tick();
            let paid = g.run.summary(0).experience_parts.total() as u64;
            let most = super::talent_experience_ceiling(&c, g.run.now as f64);
            assert!(paid <= most, "{paid} past {most} at {:.0} s", g.run.now);
        }
        assert!(g.run.won_at.is_some(), "the run was won");
    }
}
