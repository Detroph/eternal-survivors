//! Hits on enemies: the per-target step of the damage pipeline.

/// The hero's per-target damage bonuses, as fractions (0.1 = +10 %).
#[derive(Debug, Clone, Copy, Default)]
pub struct TargetBonuses {
    pub vs_crowd_controlled: f32,
    pub below_30_pct: f32,
    pub vs_afflicted: f32,
}

/// What the hit needs to know about the enemy it lands on.
#[derive(Debug, Clone, Copy)]
pub struct TargetState {
    pub health: f32,
    pub max_health: f32,
    /// Any stun, root, slow or fear timer running.
    pub crowd_controlled: bool,
    /// Any of the hero's damage-over-time effects ticking on it.
    pub afflicted: bool,
}

/// Damage actually dealt by a hit of `incoming` (already including the ability's damage and
/// the hero's general multipliers): the per-target bonuses add up, then the result is at least 1.
pub fn dealt(incoming: f32, target: &TargetState, bonus: &TargetBonuses) -> f32 {
    let mut mult = 1.0;
    if target.crowd_controlled {
        mult += bonus.vs_crowd_controlled;
    }
    if target.health <= target.max_health * 0.3 {
        mult += bonus.below_30_pct;
    }
    if target.afflicted {
        mult += bonus.vs_afflicted;
    }
    (mult * incoming).max(1.0)
}

/// Health the hero gets back from a hit that dealt `dealt`: `dealt × lifesteal` (nothing when
/// lifesteal is not above 0). The caller caps health at max health.
pub fn lifesteal_heal(dealt: f32, lifesteal: f32) -> f32 {
    if lifesteal > 0.0 {
        dealt * lifesteal
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifesteal_heals_a_share_of_the_damage_dealt() {
        assert!((lifesteal_heal(200.0, 0.015 + 0.05) - 13.0).abs() < 1e-4);
        assert_eq!(lifesteal_heal(200.0, 0.0), 0.0);
        assert_eq!(lifesteal_heal(200.0, -0.1), 0.0);
    }

    const FULL: TargetState = TargetState { health: 100.0, max_health: 100.0, crowd_controlled: false, afflicted: false };

    #[test]
    fn bonuses_add_not_multiply() {
        let b = TargetBonuses { vs_crowd_controlled: 0.2, below_30_pct: 0.3, vs_afflicted: 0.1 };
        let t = TargetState { health: 30.0, crowd_controlled: true, afflicted: true, ..FULL };
        assert!((dealt(100.0, &t, &b) - 160.0).abs() < 1e-4);
    }

    #[test]
    fn exactly_30_percent_counts_as_below() {
        let b = TargetBonuses { below_30_pct: 0.5, ..Default::default() };
        assert_eq!(dealt(10.0, &TargetState { health: 30.0, ..FULL }, &b), 15.0);
        assert_eq!(dealt(10.0, &TargetState { health: 30.1, ..FULL }, &b), 10.0);
    }

    #[test]
    fn a_hit_deals_at_least_one() {
        assert_eq!(dealt(0.2, &FULL, &TargetBonuses::default()), 1.0);
    }
}
