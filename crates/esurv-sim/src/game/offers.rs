//! Offers on the 3-choice screens: rarity, reroll cost, the gold card.

use esurv_content::Stat;

use crate::rng::Rng;

/// Offer rarities, in ascending order (white, green, blue, purple, orange).
pub use esurv_content::Rarity;

pub const RARITIES: [Rarity; 5] = Rarity::ALL;

/// Extra rarity factor of an amulet offer over a ring or trinket offer of the same screen.
pub const AMULET_RARITY_BONUS: f32 = 0.5;
/// Extra rarity factor of every offer from a rare cache (free to open) over a cache bought with gold.
pub const RARE_CACHE_RARITY_BONUS: f32 = 0.25;
/// Offers on one screen.
pub const OFFERS_PER_SCREEN: usize = 3;
/// Smallest gold card.
pub const MIN_GOLD_CARD: u32 = 60;

/// The rarity factor `a` that tilts the roll toward higher tiers.
pub fn rarity_factor(luck: f32, level: u32, bonus: f32) -> f32 {
    1.0 + luck + (level as f32 * 0.01).min(0.3) + bonus
}

/// Relative weights of the five rarities for factor `a`.
pub fn rarity_weights(a: f32) -> [f32; 5] {
    let a2 = a * a;
    [60.0, 25.0 * a, 10.0 * a2, 4.0 * a * a2, a2 * a2]
}

pub fn roll_rarity(rng: &mut Rng, a: f32) -> Rarity {
    RARITIES[rng.weighted(&rarity_weights(a)).unwrap_or(0)]
}

/// Gold for the next reroll: free while free rerolls remain, then `100 × 3ⁿ` for the n-th paid
/// one (saturating).
pub fn reroll_cost(free_left: u32, paid_so_far: u32) -> u32 {
    if free_left > 0 {
        return 0;
    }
    3u32.checked_pow(paid_so_far).and_then(|p| p.checked_mul(100)).unwrap_or(u32::MAX)
}

/// The gold card that fills a screen short of real offers.
///
/// Worth half the current cache price, and at least `MIN_GOLD_CARD`.
pub fn gold_card(cache_price: u32) -> u32 {
    (cache_price / 2).max(MIN_GOLD_CARD)
}

/// Spell-card power bonus by rarity value (index `r − 1`; r = 0 has none). The sixth entry is in
/// the table but normal rolls never reach it.
pub const SPELL_POWER_BONUS: [f32; 6] = [0.15, 0.25, 0.35, 0.5, 0.65, 0.0];
/// Spell-card cooldown reduction by rarity value, same indexing.
pub const SPELL_COOLDOWN_BONUS: [f32; 6] = [0.07, 0.10, 0.13, 0.17, 0.20, 0.0];
/// Jewelry value multiplier by rarity.
pub const JEWELRY_RARITY_MULT: [f32; 5] = [1.0, 1.3, 1.7, 2.5, 3.2];
/// A brand-new jewelry item starts this much stronger than an upgrade step.
pub const NEW_JEWELRY_MULT: f32 = 1.5;
/// Each rank of an uncapped jewelry stat shrinks the next step by this much.
pub const JEWELRY_RANK_FALLOFF: f32 = 0.08;

/// What a spell card does to the spell it grants or improves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpellCardEffect {
    None,
    /// Multiply the spell's power by `1 + bonus`.
    Power(f32),
    /// Multiply the spell's cooldown by `1 − bonus`.
    Cooldown(f32),
}

/// The effect of a spell card of `rarity` on a spell that does (`damaging`) or does not deal
/// damage or heal.
pub fn spell_card_effect(rarity: Rarity, damaging: bool) -> SpellCardEffect {
    let r = rarity as usize;
    if r == 0 {
        return SpellCardEffect::None;
    }
    if damaging {
        SpellCardEffect::Power(SPELL_POWER_BONUS[r - 1])
    } else {
        SpellCardEffect::Cooldown(SPELL_COOLDOWN_BONUS[r - 1])
    }
}

/// Double strike's ceiling on a ring or trinket.
pub const DOUBLE_CAST_CAP: f32 = 0.6;

/// Where a jewelry stat stops: `(cap as a multiple of the piece's base value, hard cap)`; both
/// `None` for the stats with no ceiling.
pub fn jewelry_cap(stat: Stat) -> (Option<f32>, Option<f32>) {
    match stat {
        Stat::Damage | Stat::CooldownRecovery | Stat::SpellCooldownRecovery | Stat::SwingCooldownRecovery | Stat::CritChance | Stat::CritDamage | Stat::Area => (None, None),
        Stat::DoubleCast => (None, Some(DOUBLE_CAST_CAP)),
        Stat::MaxHealth | Stat::DamageReduction | Stat::HealthPerSecond | Stat::Luck | Stat::HealPerKill | Stat::Lifesteal => {
            (Some(3.0), None)
        }
        _ => (Some(5.0), None),
    }
}

/// Value of a newly offered jewelry item with `base` value at `rarity`, never past its stat's
/// ceiling (`jewelry_cap`).
pub fn new_jewelry_value(base: f32, rarity: Rarity, cap_mult: Option<f32>, hard_cap: Option<f32>) -> f32 {
    let value = NEW_JEWELRY_MULT * JEWELRY_RARITY_MULT[rarity as usize] * base;
    let cap = cap_mult.map(|c| c * base).or(hard_cap).unwrap_or(f32::INFINITY);
    value.min(cap)
}

/// Extra lives a Soulstone card of `rarity` adds to a hero with `lives` already: 1, or 2 above
/// rare, never past `max`.
pub fn soulstone_lives(rarity: Rarity, lives: u32, max: u32) -> u32 {
    (1 + u32::from(rarity > Rarity::Rare)).min(max.saturating_sub(lives))
}

/// How much an upgrade adds to a worn jewelry item.
///
/// `cap_mult` is `None` for uncapped stats (then `hard_cap`, if any, still applies) and `Some(3.0)`
/// or `Some(5.0)` for capped ones.
pub fn jewelry_upgrade_step(base: f32, current: f32, rank: u32, rarity: Rarity, cap_mult: Option<f32>, hard_cap: Option<f32>) -> f32 {
    let m = JEWELRY_RARITY_MULT[rarity as usize];
    match cap_mult {
        None => {
            let step = m * base / (1.0 + JEWELRY_RANK_FALLOFF * rank as f32);
            match hard_cap {
                Some(cap) => step.min((cap - current).max(0.0)),
                None => step,
            }
        }
        Some(c) => {
            let cap = c * base;
            let room = (cap - current).max(0.0);
            (m * base * room / cap).min(room)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_rarity_odds_are_60_25_10_4_1() {
        let w = rarity_weights(1.0);
        assert_eq!(w, [60.0, 25.0, 10.0, 4.0, 1.0]);
        assert_eq!(w.iter().sum::<f32>(), 100.0);
    }

    #[test]
    fn luck_and_level_raise_the_factor_with_level_capped() {
        assert!((rarity_factor(0.2, 10, 0.0) - 1.3).abs() < 1e-6);
        assert!((rarity_factor(0.0, 80, AMULET_RARITY_BONUS + RARE_CACHE_RARITY_BONUS) - 2.05).abs() < 1e-6);
    }

    #[test]
    fn reroll_costs_triple() {
        assert_eq!(reroll_cost(3, 0), 0);
        assert_eq!([reroll_cost(0, 0), reroll_cost(0, 1), reroll_cost(0, 2), reroll_cost(0, 3)], [100, 300, 900, 2700]);
        assert_eq!(reroll_cost(0, 40), u32::MAX);
    }

    #[test]
    fn spell_cards_by_rarity() {
        assert_eq!(spell_card_effect(Rarity::Common, true), SpellCardEffect::None);
        assert_eq!(spell_card_effect(Rarity::Uncommon, true), SpellCardEffect::Power(0.15));
        assert_eq!(spell_card_effect(Rarity::Legendary, true), SpellCardEffect::Power(0.5));
        assert_eq!(spell_card_effect(Rarity::Rare, false), SpellCardEffect::Cooldown(0.10));
    }

    #[test]
    fn jewelry_values() {
        assert!((new_jewelry_value(0.17, Rarity::Legendary, None, None) - 1.5 * 3.2 * 0.17).abs() < 1e-6);
        // uncapped: step shrinks with rank
        assert!((jewelry_upgrade_step(0.1, 0.3, 5, Rarity::Common, None, None) - 0.1 / 1.4).abs() < 1e-6);
        // double-strike capped at 0.6
        assert!((jewelry_upgrade_step(0.06, 0.58, 0, Rarity::Legendary, None, Some(0.6)) - 0.02).abs() < 1e-6);
        // capped kind: step proportional to room left
        let s = jewelry_upgrade_step(0.2, 0.3, 0, Rarity::Common, Some(3.0), None);
        assert!((s - 0.2 * 0.3 / 0.6).abs() < 1e-6);
    }

    #[test]
    fn new_jewelry_stops_at_its_ceiling() {
        // max health (3x base): an epic or legendary roll would pass 3x, so it starts at 3x
        let (mult, hard) = jewelry_cap(Stat::MaxHealth);
        assert!((new_jewelry_value(0.2, Rarity::Rare, mult, hard) - 1.5 * 1.7 * 0.2).abs() < 1e-6);
        assert!((new_jewelry_value(0.2, Rarity::Epic, mult, hard) - 0.6).abs() < 1e-6);
        assert!((new_jewelry_value(0.2, Rarity::Legendary, mult, hard) - 0.6).abs() < 1e-6);
        // 5x stats never reach theirs (1.5 x 3.2 = 4.8); damage has none
        let (mult, hard) = jewelry_cap(Stat::Gold);
        assert!((new_jewelry_value(0.25, Rarity::Legendary, mult, hard) - 1.5 * 3.2 * 0.25).abs() < 1e-6);
        assert_eq!(jewelry_cap(Stat::Damage), (None, None));
        assert_eq!(jewelry_cap(Stat::DoubleCast), (None, Some(DOUBLE_CAST_CAP)));
    }

    #[test]
    fn soulstones_fill_up_to_the_most_lives() {
        assert_eq!(soulstone_lives(Rarity::Rare, 0, 2), 1);
        assert_eq!(soulstone_lives(Rarity::Epic, 0, 2), 2);
        assert_eq!(soulstone_lives(Rarity::Legendary, 1, 2), 1);
        assert_eq!(soulstone_lives(Rarity::Legendary, 2, 2), 0);
    }

    #[test]
    fn gold_card_minimum() {
        assert_eq!((gold_card(0), gold_card(500)), (60, 250));
    }

    #[test]
    fn rolls_follow_weights() {
        let mut rng = Rng::new(42);
        let mut counts = [0u32; 5];
        for _ in 0..100_000 {
            counts[roll_rarity(&mut rng, 1.0) as usize] += 1;
        }
        // within half a percent of 60/25/10/4/1
        for (c, p) in counts.iter().zip([0.60, 0.25, 0.10, 0.04, 0.01]) {
            assert!((*c as f64 / 100_000.0 - p).abs() < 0.005, "{counts:?}");
        }
    }

    /// Hand of Justice's weapon double hit, new and upgraded, from
    /// its double cast as the offers roll it: 2 / 3.05 / 4.25 / 5.3 % new, each upgrade 1/5 of
    /// its double-cast step, and no more once its double cast reaches its cap.
    #[test]
    fn hand_of_justice_double_hit_follows_its_new_and_upgrade_double_cast() {
        let content = esurv_content::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let hoj = content.jewelry.iter().find(|j| j.id == "hoj").unwrap();
        let (mult, hard) = jewelry_cap(hoj.stat);
        for (rarity, new, first_up) in
            [(Rarity::Common, 0.02, None), (Rarity::Rare, 0.0305, Some(0.016)), (Rarity::Epic, 0.0425, Some(0.023)), (Rarity::Legendary, 0.053, Some(0.030))]
        {
            let v = new_jewelry_value(hoj.value, rarity, mult, hard);
            assert!((hoj.double_hit_at(v) - new).abs() < 1e-4, "{rarity:?} new: {}", hoj.double_hit_at(v));
            if let Some(up) = first_up {
                let step = jewelry_upgrade_step(hoj.value, v, 1, rarity, mult, hard);
                let gain = hoj.double_hit_at(v + step) - hoj.double_hit_at(v);
                assert!((gain - up).abs() < 1e-3, "{rarity:?} first upgrade: {gain}");
            }
        }
        // at the cap an upgrade adds nothing: 12.5 %
        let step = jewelry_upgrade_step(hoj.value, DOUBLE_CAST_CAP, 9, Rarity::Legendary, mult, hard);
        assert_eq!(step, 0.0);
        assert!((hoj.double_hit_at(DOUBLE_CAST_CAP + step) - 0.125).abs() < 1e-5);
    }
}
