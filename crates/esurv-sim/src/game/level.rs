//! Experience and levels.

use esurv_content::ExperienceRules;

pub const MAX_LEVEL: u32 = 99;
/// Fraction of max health healed on every level-up.
pub const LEVEL_UP_HEAL: f32 = 0.2;

/// Experience needed to go from `level` to `level + 1` (`balance.ron`).
pub fn xp_to_next(level: u32, rules: &ExperienceRules) -> u32 {
    rules.to_next(level)
}

/// The hero's level and banked experience.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub level: u32,
    pub xp: u32,
}

impl Default for Progress {
    fn default() -> Self {
        Self { level: 1, xp: 0 }
    }
}

impl Progress {
    /// Adds experience and returns how many levels were gained (each one queues a level-up
    /// choice). Leftover experience carries over; nothing is gained past the cap.
    pub fn gain(&mut self, xp: u32, rules: &ExperienceRules) -> u32 {
        self.xp = self.xp.saturating_add(xp);
        let mut gained = 0;
        while self.level < MAX_LEVEL && self.xp >= xp_to_next(self.level, rules) {
            self.xp -= xp_to_next(self.level, rules);
            self.level += 1;
            gained += 1;
        }
        gained
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> ExperienceRules {
        esurv_content::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap().balance.experience
    }

    #[test]
    fn level_costs_follow_their_curve() {
        let r = rules();
        // round(min(200 + 65 × (L − 1)^1.5, 20 000))
        let at = |l| xp_to_next(l, &r);
        assert_eq!([at(1), at(2), at(10), at(40), at(60)], [200, 265, 1955, 16031, 20000]);
    }

    #[test]
    fn several_levels_at_once_with_carry() {
        let r = rules();
        let mut p = Progress::default();
        assert_eq!(p.gain(200 + 265 + 7, &r), 2);
        assert_eq!(p, Progress { level: 3, xp: 7 });
    }

    #[test]
    fn capped_at_99() {
        let r = rules();
        let mut p = Progress { level: 98, xp: 0 };
        assert_eq!(p.gain(10_000_000, &r), 1);
        assert_eq!(p.level, 99);
    }
}
