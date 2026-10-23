//! What falls on the field: experience orbs, gold, caches, runes and magnets, and when they
//! appear.

use esurv_content::{Enemy, ExperienceRules};

use super::enemy::RunOptions;
use crate::rng::Rng;

/// The things that can lie on the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pickup {
    /// Experience; the orb's look grows with its value.
    Experience,
    Gold,
    BattlegroundCache,
    /// The cache a slain rare drops: free to open.
    RareCache,
    SpeedRune,
    BerserkerRune,
    RestorationRune,
    Magnet,
    /// A battle standard (`game::assists`): the banner of this faction (the party's). A hero
    /// walking over it summons an ally of the hero's own faction.
    BattleStandard(esurv_content::Faction),
}

/// Gold per kill scales with `KILL_SCALE / density`. (Experience per kill is `balance.ron`'s.)
pub const KILL_SCALE: f32 = 1.65;
/// A normal kill drops gold with chance `KILL_SCALE / density × NORMAL_GOLD_CHANCE`.
pub const NORMAL_GOLD_CHANCE: f32 = 0.1;
/// Gold of a normal kill: 2, 3 or 4.
pub const NORMAL_GOLD: std::ops::Range<u32> = 2..5;
/// An elite always drops this much gold, and a cache.
pub const ELITE_GOLD: u32 = 15;
/// Boss kill gold: `40 + 20 × bosses slain` (this one included). Its experience is `balance.ron`'s.
pub const BOSS_GOLD: (u32, u32) = (40, 20);
/// Loose experience orbs beyond this many merge into the nearest one.
pub const MAX_LOOSE_ORBS: usize = 44;

/// The look of an experience orb worth `value`, by `balance.ron`'s thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrbSize {
    Small,
    Medium,
    Large,
}

/// An orb's size: below `orb_medium` small, below `orb_large` medium, else large.
pub fn orb_size(value: u32, rules: &ExperienceRules) -> OrbSize {
    if value < rules.orb_medium {
        OrbSize::Small
    } else if value < rules.orb_large {
        OrbSize::Medium
    } else {
        OrbSize::Large
    }
}

/// Every `RUNE_EVERY` seconds, if at most `RUNE_MAX_PRESENT − 1` runes lie on the field, one of
/// the three appears `RUNE_RING` yards from the hero.
pub const RUNE_EVERY: f32 = 45.0;
/// The first rune check comes at this run second.
pub const FIRST_RUNE_CHECK: f32 = 40.0;
pub const RUNE_MAX_PRESENT: usize = 2;
pub const RUNE_RING: (f32, f32) = (15.0, 40.0);
/// Every `CACHE_EVERY` seconds, if fewer than `CACHE_MAX_PRESENT` caches lie on the field, one
/// appears `CACHE_RING` yards from the hero. Two are placed at the start of a run. Over
/// `CACHE_LIMIT` caches, the oldest is removed.
pub const CACHE_EVERY: f32 = 70.0;
/// The first periodic cache check comes at this run second.
pub const FIRST_CACHE_CHECK: f32 = 60.0;
pub const CACHE_MAX_PRESENT: usize = 3;
pub const CACHE_RING: (f32, f32) = (18.0, 50.0);
pub const CACHES_AT_START: usize = 2;
pub const CACHE_LIMIT: usize = 4;

/// Opening a cache costs gold. The first costs `CACHE_PRICE_START`; each opening raises the price
/// to `price × CACHE_PRICE_GROWTH + CACHE_PRICE_STEP`, rounded down: 20, 30, 42, 57, 76, ...
/// (×1.25)
/// Without enough gold, "This cache needs N gold." shows at most every `CACHE_MESSAGE_GAP` s.
pub const CACHE_PRICE_START: u32 = 20;
pub const CACHE_PRICE_GROWTH: f32 = 1.25;
pub const CACHE_PRICE_STEP: f32 = 5.0;
pub const CACHE_MESSAGE_GAP: f32 = 4.0;

/// The price of the next cache after one is opened at `price`.
pub fn next_cache_price(price: u32) -> u32 {
    (price as f32 * CACHE_PRICE_GROWTH + CACHE_PRICE_STEP) as u32
}

/// Speed Rune: +60 % move speed for 10 s. Berserker Rune: +50 % damage for 15 s. Taking a rune
/// whose buff is up restarts it.
pub const SPEED_RUNE: (f32, f32) = (0.6, 10.0);
pub const BERSERKER_RUNE: (f32, f32) = (0.5, 15.0);
/// `restoration` Rune heals this share of max health.
pub const RESTORATION_HEAL: f32 = 0.5;

/// Magnets per run: 2 or 3.
pub const MAGNETS: std::ops::RangeInclusive<u32> = 2..=3;
/// Without a final boss, magnets spread over this many seconds.
pub const MAGNET_SPAN_DEFAULT: f32 = 1200.0;
/// A slain rare drops a magnet that is already due with this chance (a boss always does).
pub const RARE_MAGNET_CHANCE: f32 = 0.5;

/// Experience dropped by a kill: a normal enemy drops
/// `normal_kill × 100 ÷ density %`, rounded at random so the average is exact; an elite drops
/// `elite_kill`.
pub fn kill_experience(enemy: &Enemy, rules: &ExperienceRules, options: &RunOptions, rng: &mut Rng) -> u32 {
    if enemy.elite {
        return rules.elite_kill;
    }
    let v = rules.normal_kill / (options.density_pct as f32 / 100.0);
    let whole = v.trunc();
    whole as u32 + u32::from(rng.unit() < v - whole)
}

/// Gold dropped by a kill, if any.
pub fn kill_gold(enemy: &Enemy, options: &RunOptions, rng: &mut Rng) -> Option<u32> {
    let chance = if enemy.elite { 1.0 } else { KILL_SCALE / (options.density_pct as f32 / 100.0) * NORMAL_GOLD_CHANCE };
    if rng.unit() >= chance {
        return None;
    }
    Some(if enemy.elite {
        ELITE_GOLD
    } else {
        NORMAL_GOLD.start + (rng.next_u64() % (NORMAL_GOLD.end - NORMAL_GOLD.start) as u64) as u32
    })
}

/// Experience and gold dropped by the `slain`-th boss of the run (1-based): experience
/// `boss_kill.0 + boss_kill.1 × bosses already slain`, gold
/// `40 + 20 × slain`.
pub fn boss_drops(slain: u32, rules: &ExperienceRules) -> (u32, u32) {
    let before = slain.saturating_sub(1);
    (rules.boss_kill.0 + rules.boss_kill.1 * before, BOSS_GOLD.0 + BOSS_GOLD.1 * slain)
}

/// When each magnet becomes due and when it appears on its own: the span (seconds until the
/// final boss) is cut into equal windows; magnet `i` is due at the start of window `i` and
/// appears 60–100 % of the way through it.
pub fn magnet_schedule(final_boss_minute: Option<f32>, rng: &mut Rng) -> Vec<(f32, f32)> {
    let span = final_boss_minute.map_or(MAGNET_SPAN_DEFAULT, |m| m * 60.0);
    let n = MAGNETS.start() + (rng.next_u64() % (MAGNETS.end() - MAGNETS.start() + 1) as u64) as u32;
    let w = span / n as f32;
    (0..n).map(|i| (i as f32 * w, i as f32 * w + rng.range(0.6, 1.0) * w)).collect()
}

/// The rune that appears: each of the three equally likely.
pub fn random_rune(rng: &mut Rng) -> Pickup {
    match rng.next_u64() % 3 {
        0 => Pickup::SpeedRune,
        1 => Pickup::BerserkerRune,
        _ => Pickup::RestorationRune,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_content::Content;

    #[test]
    fn kill_rewards_average_out() {
        let c = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let normal = c.zones[0].enemies.iter().find(|e| !e.elite).unwrap();
        let mut rng = Rng::new(7);
        let o = RunOptions::default();
        let n = 20_000;
        let r = &c.balance.experience;
        assert_eq!(kill_experience(normal, r, &o, &mut rng), 100, "100 at 100 % density");
        let golds: Vec<u32> = (0..n).filter_map(|_| kill_gold(normal, &o, &mut rng)).collect();
        assert!((golds.len() as f32 / n as f32 - 0.165).abs() < 0.01);
        assert!(golds.iter().all(|g| (2..=4).contains(g)));
        let elite = c.zones[0].enemies.iter().find(|e| e.elite).unwrap();
        assert_eq!(kill_gold(elite, &o, &mut rng), Some(15));
        assert_eq!(kill_experience(elite, r, &o, &mut rng), 1000);
    }

    #[test]
    fn normal_kill_experience_scales_with_density() {
        let c = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let r = &c.balance.experience;
        let normal = c.zones[0].enemies.iter().find(|e| !e.elite).unwrap();
        let elite = c.zones[0].enemies.iter().find(|e| e.elite).unwrap();
        let mut rng = Rng::new(11);
        let n = 20_000;
        for (density, want) in [(50u16, 200.0f32), (200, 50.0), (300, 100.0 / 3.0)] {
            let o = RunOptions { density_pct: density, ..RunOptions::default() };
            let drops: Vec<u32> = (0..n).map(|_| kill_experience(normal, r, &o, &mut rng)).collect();
            // rounded at random to the two whole numbers around the exact value, averaging it
            assert!(drops.iter().all(|&d| d as f32 >= want.floor() && d as f32 <= want.ceil()), "{density}");
            let mean = drops.iter().sum::<u32>() as f32 / n as f32;
            assert!((mean - want).abs() < 0.02 * want, "{density}: {mean}");
            assert_eq!(kill_experience(elite, r, &o, &mut rng), 1000, "an elite ignores density");
        }
    }

    #[test]
    fn orb_sizes_follow_their_thresholds() {
        let r = esurv_content::ExperienceRules::default();
        let size = |v| orb_size(v, &r);
        assert_eq!([size(1), size(199), size(200), size(999), size(1000), size(4000)], {
            use OrbSize::*;
            [Small, Small, Medium, Medium, Large, Large]
        });
    }

    #[test]
    fn cache_prices_grow() {
        let mut p = CACHE_PRICE_START;
        let mut seen = vec![p];
        for _ in 0..4 {
            p = next_cache_price(p);
            seen.push(p);
        }
        assert_eq!(seen, [20, 30, 42, 57, 76]);
    }

    #[test]
    fn bosses_and_magnets() {
        let r = esurv_content::ExperienceRules::default();
        // experience 2400 + 800 × bosses already slain; gold 40 + 20 × slain, this one included
        assert_eq!([boss_drops(1, &r), boss_drops(2, &r), boss_drops(3, &r)], [(2400, 60), (3200, 80), (4000, 100)]);
        let mut rng = Rng::new(3);
        for _ in 0..50 {
            let s = magnet_schedule(Some(15.0), &mut rng);
            assert!((2..=3).contains(&s.len()));
            let w = 900.0 / s.len() as f32;
            for (i, (due, at)) in s.iter().enumerate() {
                assert!((due - i as f32 * w).abs() < 1e-3);
                assert!(*at >= due + 0.6 * w - 1e-3 && *at <= due + w);
            }
        }
    }
}
