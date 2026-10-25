//! Challenge Mode (docs/game/zones.md, "Challenge Mode"): once a class has cleared
//! Ahn'Qiraj, every zone is open to it in this mode. A zone keeps its own enemies, bosses,
//! mechanics and clocks, but every enemy's health and damage follow Ahn'Qiraj's difficulty D
//! instead of the zone's, and then the final health and damage of every kind are × `balance.ron`
//! `challenge_factor` × the challenge slider, one flat factor outside the kind and zone powers
//! (`enemy::health_and_damage`). The slider runs 1x to 5x in 0.25 steps and stacks with the run
//! options. A challenge run pays only the class gear its bosses drop (as in Normal Mode) and the
//! talent experience the same run in Ahn'Qiraj would, × `challenge_factor`.

use esurv_content::Content;

/// The zone whose clear opens Challenge Mode and whose difficulty every challenge zone takes.
pub const CHALLENGE_ZONE: &str = "aq";

/// The challenge slider, in whole percentages: 100 % (1x) to 500 % (5x) in 25 % steps.
pub const SLIDER_MIN_PCT: u16 = 100;
pub const SLIDER_MAX_PCT: u16 = 500;
pub const SLIDER_STEP_PCT: u16 = 25;

/// The challenge slider value `pct` kept on the slider: within 1x and 5x, on the nearest 0.25 step
/// (halfway goes up).
pub fn slider_on_its_steps(pct: u16) -> u16 {
    let pct = pct.clamp(SLIDER_MIN_PCT, SLIDER_MAX_PCT);
    let steps = (pct - SLIDER_MIN_PCT + SLIDER_STEP_PCT / 2) / SLIDER_STEP_PCT;
    (SLIDER_MIN_PCT + steps * SLIDER_STEP_PCT).min(SLIDER_MAX_PCT)
}

/// The slider value moved one step up or down, kept on the slider.
pub fn slider_stepped(pct: u16, up: bool) -> u16 {
    let pct = slider_on_its_steps(pct);
    slider_on_its_steps(if up { pct.saturating_add(SLIDER_STEP_PCT) } else { pct.saturating_sub(SLIDER_STEP_PCT) })
}

/// The slider value as the screens show it: "1.25x".
pub fn slider_text(pct: u16) -> String {
    format!("{:.2}x", f32::from(slider_on_its_steps(pct)) / 100.0)
}

/// A challenge run's numbers, worked out from the content when the run starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Challenge {
    /// The difficulty D every enemy's health and damage follow: Ahn'Qiraj's.
    pub difficulty: f32,
    /// Every enemy's final health and damage are × this: `challenge_factor` × the slider.
    pub enemy_factor: f32,
    /// The run's talent experience, worked out at `difficulty`, is × this: `challenge_factor`.
    pub talent_experience_factor: f32,
}

impl Challenge {
    /// The challenge at slider value `slider_pct` (kept on the slider); `None` for content without
    /// Ahn'Qiraj.
    pub fn new(content: &Content, slider_pct: u16) -> Option<Self> {
        let difficulty = content.zone(CHALLENGE_ZONE)?.difficulty;
        let factor = content.balance.challenge_factor;
        let slider = f32::from(slider_on_its_steps(slider_pct)) / 100.0;
        Some(Self { difficulty, enemy_factor: factor * slider, talent_experience_factor: factor })
    }
}

/// Whether a class that has won the zones `zones_won` (ids) has Challenge Mode open: it has cleared
/// Ahn'Qiraj, and the content still has it.
pub fn is_open<'a>(content: &Content, mut zones_won: impl Iterator<Item = &'a str>) -> bool {
    content.zone(CHALLENGE_ZONE).is_some() && zones_won.any(|z| z == CHALLENGE_ZONE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::enemy::{boss_stats, health_and_damage, spawn_stats, RunOptions, RUN_FACTOR};
    use crate::game::rewards::{talent_experience_parts, RunTally};
    use esurv_content::EnemyType;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    fn close(got: f32, want: f32) -> bool {
        (got - want).abs() <= want.abs() * 1e-5
    }

    #[test]
    fn the_slider_snaps_to_quarter_steps_from_1x_to_5x() {
        assert_eq!(slider_on_its_steps(0), 100);
        assert_eq!(slider_on_its_steps(100), 100);
        assert_eq!(slider_on_its_steps(112), 100);
        assert_eq!(slider_on_its_steps(113), 125);
        assert_eq!(slider_on_its_steps(140), 150);
        assert_eq!(slider_on_its_steps(500), 500);
        assert_eq!(slider_on_its_steps(900), 500);
        let every: Vec<u16> = (0..=16).map(|i| 100 + i * 25).collect();
        assert!(every.iter().all(|&v| slider_on_its_steps(v) == v));
        assert_eq!((slider_stepped(100, false), slider_stepped(100, true), slider_stepped(500, true), slider_stepped(130, false)), (100, 125, 500, 100));
        assert_eq!([slider_text(100), slider_text(125), slider_text(500)], ["1.00x", "1.25x", "5.00x"]);
    }

    #[test]
    fn the_challenge_reads_ahn_qiraj_s_difficulty_and_the_named_factor() {
        let c = content();
        let aq = c.zone(CHALLENGE_ZONE).unwrap().difficulty;
        let ch = Challenge::new(&c, 175).unwrap();
        assert_eq!(ch.difficulty, aq);
        assert!(close(ch.enemy_factor, c.balance.challenge_factor * 1.75));
        assert_eq!(ch.talent_experience_factor, c.balance.challenge_factor);
        assert_eq!(c.balance.challenge_factor, 1.1, "balance.ron's named value");
    }

    /// An elite and a boss in Elwynn in Challenge Mode: exactly their values at Ahn'Qiraj's D,
    /// × 1.1 × the slider on both health and damage, with the run options on top as ever.
    #[test]
    fn every_kind_is_its_ahn_qiraj_self_times_the_flat_factor() {
        let c = content();
        let elwynn = c.zone("elwynn").unwrap();
        let aq = c.zone(CHALLENGE_ZONE).unwrap();
        let normal = RunOptions { health_pct: 150, damage_pct: 120, ..RunOptions::default() };
        for slider in [100, 125, 300, 500] {
            let ch = Challenge::new(&c, slider).unwrap();
            let flat = 1.1 * f32::from(slider) / 100.0;
            let challenge = RunOptions { challenge: Some(ch), ..normal };
            for kind in [EnemyType::Normal, EnemyType::Elite, EnemyType::Rare, EnemyType::Boss, EnemyType::FinalBoss] {
                let (h_aq, d_aq) = health_and_damage(&c.balance, kind, aq, 9.0, 0.5, &normal, RUN_FACTOR);
                let (h, d) = health_and_damage(&c.balance, kind, elwynn, 9.0, 0.5, &challenge, RUN_FACTOR);
                assert!(close(h, h_aq * flat) && close(d, d_aq * flat), "{kind:?} at {slider}: ({h}, {d}) vs ({h_aq}, {d_aq}) × {flat}");
            }
            // a roster elite and a boss of the zone itself: their own share and roster kept
            let elite = elwynn.enemies.iter().find(|e| e.elite).unwrap();
            let (e, e_aq) = (spawn_stats(elite, elwynn, &c.balance, 6.0, 0.0, &challenge, RUN_FACTOR), spawn_stats(elite, aq, &c.balance, 6.0, 0.0, &normal, RUN_FACTOR));
            assert!(close(e.health, e_aq.health * flat) && close(e.damage, e_aq.damage * flat));
            let boss = &elwynn.bosses[0];
            let (b, b_aq) = (boss_stats(boss, elwynn, &c.balance, 3.0, 0.0, &challenge, RUN_FACTOR), boss_stats(boss, aq, &c.balance, 3.0, 0.0, &normal, RUN_FACTOR));
            assert!(close(b.health, b_aq.health * flat) && close(b.damage, b_aq.damage * flat));
        }
    }

    /// Talent experience: what the same run in Ahn'Qiraj pays, × 1.1, whatever the slider; the
    /// run options keep their bonus.
    #[test]
    fn talent_experience_is_ahn_qiraj_s_times_the_factor_never_the_slider() {
        let c = content();
        let s = &c.balance.zone_scaling;
        let aq = c.zone(CHALLENGE_ZONE).unwrap().difficulty;
        let t = RunTally { seconds: 16.5 * 60.0, kills: 900, bosses: 5, won: true };
        let options = RunOptions { damage_pct: 150, ..RunOptions::default() };
        let in_aq = talent_experience_parts(&t, s, aq, &options, &c.run_options);
        for slider in [100, 250, 500] {
            let challenge = RunOptions { challenge: Challenge::new(&c, slider), ..options };
            // the zone's own D (Elwynn's 1) is not what counts
            let got = talent_experience_parts(&t, s, 1.0, &challenge, &c.run_options);
            assert_eq!(got.base(), in_aq.base());
            assert!((got.total() as f32 - in_aq.total() as f32 * 1.1).abs() <= 2.0, "{} vs {} × 1.1", got.total(), in_aq.total());
            assert!(got.options > 0, "the run options keep their bonus");
        }
    }

    #[test]
    fn challenge_mode_opens_with_ahn_qiraj_cleared() {
        let c = content();
        assert!(!is_open(&c, std::iter::empty()));
        assert!(!is_open(&c, ["elwynn", "stratholme"].into_iter()));
        assert!(is_open(&c, ["elwynn", CHALLENGE_ZONE].into_iter()));
    }
}
