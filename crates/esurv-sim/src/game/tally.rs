//! What a run was worth when it ends: the end screen's numbers for each hero and the talent
//! experience they earn.

use super::rewards::{talent_experience_parts, ExperienceParts, RunTally};
use super::run::{GearDrop, Phase, Run};

/// One hero's end-of-run summary.
#[derive(Debug, Clone, PartialEq)]
pub struct RunSummary {
    pub hero: u32,
    /// Seconds the run lasted for this hero (to its death, or to the end).
    pub seconds: f32,
    /// The final boss fell.
    pub won: bool,
    pub level: u32,
    pub kills: u32,
    pub elites: u32,
    pub rares: u32,
    /// Bosses slain in the run.
    pub bosses: u32,
    pub damage_dealt: f64,
    pub damage_taken: f64,
    pub gold_earned: u32,
    pub caches: u32,
    /// Gear won from bosses.
    pub loot: Vec<GearDrop>,
    /// Talent experience as the end screen shows it: (base, zone bonus, run-option bonus).
    pub talent_experience: (u32, u32, u32),
    /// The same with the base broken down (time, kills, bosses, the victory).
    pub experience_parts: ExperienceParts,
}

impl Run {
    /// Whether the run is over: every hero has fallen, or the final boss fell and the run was
    /// not carried on into Endless.
    pub fn is_over(&self) -> bool {
        matches!(self.phase, Phase::Defeat | Phase::Victory)
    }

    /// Hero `i`'s summary of the run so far.
    pub fn summary(&self, i: usize) -> RunSummary {
        let h = &self.heroes[i];
        let seconds = h.fell_at.unwrap_or(self.now);
        let won = self.won_at.is_some();
        let tally = RunTally { seconds, kills: h.kills, bosses: self.bosses_slain, won };
        let scaling = &self.content.balance.zone_scaling;
        let parts = talent_experience_parts(&tally, scaling, self.content.zones[self.zone].difficulty, &self.options, &self.content.run_options);
        RunSummary {
            hero: h.id,
            seconds,
            won,
            level: h.progress.level,
            kills: h.kills,
            elites: h.elites_killed,
            rares: h.rares_killed,
            bosses: self.bosses_slain,
            damage_dealt: h.damage_dealt,
            damage_taken: h.damage_taken,
            gold_earned: h.gold_earned,
            caches: h.caches_opened,
            loot: h.loot.clone(),
            talent_experience: (parts.base(), parts.zone, parts.options),
            experience_parts: parts,
        }
    }

    /// Every hero's summary.
    pub fn summaries(&self) -> Vec<RunSummary> {
        (0..self.heroes.len()).map(|i| self.summary(i)).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::Content;

    use super::*;
    use crate::game::casting::HitSource;
    use crate::game::enemy::RunOptions;
    use crate::game::run::{Flat, HeroSetup};

    fn run() -> Run {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Arc::new(Content::load(dir).expect("content loads"));
        let mut r = Run::new(content, "elwynn", RunOptions::default(), &[HeroSetup { class: "warrior".into(), race: "orc".into(), ..Default::default() }], Box::new(Flat), 31).unwrap();
        let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
        r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        r.enemies.clear();
        r
    }

    #[test]
    fn a_won_run_counts_its_kills_bosses_and_talent_experience() {
        let mut r = run();
        r.now = 16.0 * 60.0 + 30.0;
        for b in 0..5 {
            r.spawn_boss(b);
            let id = r.enemies.last().unwrap().id;
            r.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Other);
        }
        r.tick(0.05);
        assert!(r.is_over());
        let s = r.summary(0);
        assert!(s.won);
        assert_eq!((s.kills, s.bosses), (5, 5));
        // 16.5 min x 10 = 165, 5 kills x 0.05 = 0 (0.25 rounded), 5 bosses x 40, +250 for the win
        assert_eq!(s.talent_experience, (165 + 200 + 250, 0, 0));
    }

    #[test]
    fn a_fallen_hero_counts_time_to_its_fall() {
        let mut r = run();
        r.now = 90.0;
        r.damage_hero(0, 1e9, None, false);
        r.tick(0.05);
        assert!(r.is_over());
        let s = r.summary(0);
        assert!(!s.won);
        assert_eq!(s.seconds, 90.0);
        assert!(s.damage_taken > 0.0);
    }
}
