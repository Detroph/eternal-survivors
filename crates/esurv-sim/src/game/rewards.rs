//! Rewards between runs: the run-option bonus, boss gear drop odds, talent experience and talent
//! points.
//!
//! What follows a zone's difficulty D (drop chance, gear tier, the talent experience zone bonus)
//! reads `balance.ron` `zone_scaling`.

use esurv_content::{Content, RunOption, ZoneScaling};

use super::enemy::RunOptions;

/// The run-option bonus never exceeds this.
pub const MAX_OPTION_BONUS: f32 = 2.0;
/// Most talent points one class can earn.
pub const MAX_TALENT_POINTS: u32 = 51;

/// Talent experience per minute survived, per kill (at 100 % density), per boss killed, and for a
/// win.
pub const TALENT_XP_PER_MINUTE: f32 = 10.0;
pub const TALENT_XP_PER_KILL: f32 = 0.05;
pub const TALENT_XP_PER_BOSS: f32 = 40.0;
pub const TALENT_XP_FOR_A_WIN: f32 = 250.0;
/// The first talent point's cost, and how much more each next one costs
/// (one win in each zone, in order, fills a class's 51 points by the end of the zone at D 9, AUTHORING.md
/// "Power budgets").
pub const FIRST_TALENT_POINT_COST: f32 = 80.0;
pub const TALENT_POINT_COST_GROWTH: f32 = 1.024;
/// How much more each prestige point costs than the one before; the
/// first costs what a 52nd talent point would.
pub const PRESTIGE_POINT_COST_GROWTH: f32 = 1.05;

/// One multiplier for loot odds and talent experience; harder options give more. Each option adds
/// its `reward_weight` (from `run_options.ron`, `defs`) for every 100 % above 100 %.
pub fn option_bonus(o: &RunOptions, defs: &[RunOption]) -> f32 {
    let weight = |id: &str| defs.iter().find(|d| d.id == id).map_or(0.0, |d| d.reward_weight);
    let f = |pct: u16| pct as f32 / 100.0 - 1.0;
    let sum = weight("density") * f(o.density_pct)
        + weight("health") * f(o.health_pct)
        + weight("damage") * f(o.damage_pct)
        + weight("speed") * f(o.speed_pct);
    (1.0 + sum).min(MAX_OPTION_BONUS)
}

/// Chance that one droppable piece drops from a boss in a zone of difficulty `difficulty`, before
/// the run-option bonus.
pub fn zone_drop_chance(scaling: &ZoneScaling, difficulty: f32) -> f32 {
    scaling.drop_chance_at(difficulty)
}

/// Chance that one droppable piece drops from a boss in a zone of difficulty `difficulty`.
pub fn gear_drop_chance(scaling: &ZoneScaling, difficulty: f32, options: &RunOptions, defs: &[RunOption]) -> f32 {
    zone_drop_chance(scaling, difficulty) * option_bonus(options, defs)
}

/// The gear slots a boss can drop, each rolled on its own at `gear_drop_chance`.
///
/// Every zone drops its whole set: its non-final bosses in order, boss
/// number `i` of `n` owns the armor slots `s` (0..8) with `s % n == i`, so four bosses own two
/// slots each. A final boss drops the class's weapons (slots 8 and up). `bosses_final[z][b]`
/// says whether boss `b` of zone `z` is final.
pub fn boss_drop_slots(bosses_final: &[Vec<bool>], zone_index: usize, boss_index: usize, weapons: u32) -> Vec<u32> {
    let Some(&is_final) = bosses_final.get(zone_index).and_then(|z| z.get(boss_index)) else {
        return Vec::new();
    };
    if is_final {
        return (0..weapons).map(|w| super::gear::ARMOR_SLOTS + w).collect();
    }
    let regular: Vec<usize> = bosses_final[zone_index].iter().enumerate().filter(|(_, f)| !**f).map(|(b, _)| b).collect();
    let Some(i) = regular.iter().position(|&b| b == boss_index) else {
        return Vec::new();
    };
    (0..super::gear::ARMOR_SLOTS).filter(|s| *s as usize % regular.len() == i).collect()
}

/// Where class gear item entries start (our numbering): a piece is
/// `GEAR_ENTRY_BASE + wow class id × 100 000 + slot × 1000 + zone × 10`, its last digit left 0 for
/// the look a run shows it in. Entries are worked out for a session and never saved (saves keep
/// a piece's slot and zone ids), so a zone added or moved only renumbers the next session's.
pub const GEAR_ENTRY_BASE: u32 = 2_000_000;

/// The item entry the client is told about for a class gear piece in `slot` (below
/// `gear::MAX_GEAR_SLOTS`) from zone `zone` (its index in the zones' difficulty order, below
/// `gear::MAX_GEAR_ZONES`).
pub fn gear_item_entry(wow_class_id: u32, slot: u32, zone: u32) -> u32 {
    GEAR_ENTRY_BASE + wow_class_id * 100_000 + slot * 1000 + zone * 10
}

/// The gear tier of a zone of difficulty `difficulty`: how many of `zone_scaling`'s `gear_tier_starts`
/// are at or below it, so a zone added between two others takes the tier of the zones around it.
pub fn tier_of_difficulty(scaling: &ZoneScaling, difficulty: f32) -> u32 {
    scaling.gear_tier_starts.iter().filter(|&&start| start <= difficulty).count() as u32
}

/// The gear tier of zone `zone` (its index in the zones' difficulty order): its pieces' quality colour, from
/// its difficulty. A number past the zones counts as the last zone.
pub fn zone_tier(content: &Content, zone: usize) -> u32 {
    let zones = &content.zones;
    zones.get(zone).or(zones.last()).map_or(0, |z| tier_of_difficulty(&content.balance.zone_scaling, z.difficulty))
}

/// How many zones are open to a class, from the first in difficulty order: every
/// zone up to the hardest it has won, and the next one; only the first for a class with no win.
/// `won` holds the indices of the zones won; a zone added below the hardest win opens at once.
pub fn open_zone_count(won: impl IntoIterator<Item = usize>, zone_count: usize) -> usize {
    let hardest = won.into_iter().filter(|&i| i < zone_count).max();
    hardest.map_or(1, |i| i + 2).min(zone_count).max(1)
}

/// What a finished run was worth.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunTally {
    pub seconds: f32,
    pub kills: u32,
    pub bosses: u32,
    pub won: bool,
}

/// Talent experience a run earns, part by part as the end screen breaks it down: the base is time,
/// kills, bosses and the victory; then the zone and run-option bonuses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExperienceParts {
    pub time: u32,
    pub kills: u32,
    pub bosses: u32,
    pub victory: u32,
    pub zone: u32,
    pub options: u32,
}

impl ExperienceParts {
    pub fn base(&self) -> u32 {
        self.time + self.kills + self.bosses + self.victory
    }

    pub fn total(&self) -> u32 {
        self.base() + self.zone + self.options
    }
}

/// Talent experience a run earns, part by part, in a zone of difficulty `difficulty`: the zone
/// bonus is × D^`talent_exponent` (`balance.ron` `zone_scaling`: a zone added
/// anywhere keeps the others' bonus).
pub fn talent_experience_parts(t: &RunTally, scaling: &ZoneScaling, difficulty: f32, options: &RunOptions, defs: &[RunOption]) -> ExperienceParts {
    let time = (t.seconds / 60.0 * TALENT_XP_PER_MINUTE).round();
    let kills = (t.kills as f32 / (options.density_pct as f32 / 100.0) * TALENT_XP_PER_KILL).round();
    let bosses = TALENT_XP_PER_BOSS * t.bosses as f32;
    let victory = if t.won { TALENT_XP_FOR_A_WIN } else { 0.0 };
    let base = time + kills + bosses + victory;
    let zone = (scaling.talent_factor(difficulty) * base).round() - base;
    let opt = ((option_bonus(options, defs) - 1.0) * (base + zone)).round();
    ExperienceParts { time: time as u32, kills: kills as u32, bosses: bosses as u32, victory: victory as u32, zone: zone.max(0.0) as u32, options: opt.max(0.0) as u32 }
}

/// Talent experience a run earns, split as the end screen shows it: (base, zone bonus, option bonus).
pub fn talent_experience(t: &RunTally, scaling: &ZoneScaling, difficulty: f32, options: &RunOptions, defs: &[RunOption]) -> (u32, u32, u32) {
    let p = talent_experience_parts(t, scaling, difficulty, options, defs);
    (p.base(), p.zone, p.options)
}

/// Talent experience the `n`-th point (1-based) costs: 145, 149, 152 ... 498 for the 51st.
pub fn talent_point_cost(n: u32) -> u32 {
    (FIRST_TALENT_POINT_COST * TALENT_POINT_COST_GROWTH.powi(n.saturating_sub(1) as i32)).round() as u32
}

/// Points earned from `xp` total talent experience, and experience into the next point.
pub fn talent_points(mut xp: u32) -> (u32, u32) {
    let mut points = 0;
    while points < MAX_TALENT_POINTS && xp >= talent_point_cost(points + 1) {
        xp -= talent_point_cost(points + 1);
        points += 1;
    }
    (points, xp)
}

/// Talent experience the `n`-th prestige point (1-based) costs: the
/// first what a 52nd talent point would, each next one `PRESTIGE_POINT_COST_GROWTH` times the one
/// before. Never below 1.
pub fn prestige_point_cost(n: u32) -> u32 {
    let first = FIRST_TALENT_POINT_COST * TALENT_POINT_COST_GROWTH.powi(MAX_TALENT_POINTS as i32);
    let steps = n.saturating_sub(1).min(i32::MAX as u32) as i32;
    ((first * PRESTIGE_POINT_COST_GROWTH.powi(steps)).round() as u32).max(1)
}

/// Prestige points earned from `xp` total talent experience, and experience into the next one:
/// what is left once every talent point is paid for buys them; none before that.
pub fn prestige_points(xp: u32) -> (u32, u32) {
    let (talent, mut left) = talent_points(xp);
    if talent < MAX_TALENT_POINTS {
        return (0, 0);
    }
    let mut points = 0;
    while left >= prestige_point_cost(points + 1) {
        left -= prestige_point_cost(points + 1);
        points += 1;
    }
    (points, left)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four sliders with the weights `run_options.ron` ships.
    fn defs() -> Vec<RunOption> {
        [("density", 0.1), ("health", 0.4), ("damage", 0.6), ("speed", 2.0)]
            .map(|(id, w)| RunOption {
                id: id.into(),
                name: String::new(),
                tooltip: String::new(),
                min_pct: 100,
                max_pct: 500,
                step_pct: 5,
                reward_weight: w,
            })
            .to_vec()
    }

    #[test]
    fn option_bonus_weights_and_cap() {
        assert_eq!(option_bonus(&RunOptions::default(), &defs()), 1.0);
        let o = RunOptions { speed_pct: 125, ..Default::default() };
        assert!((option_bonus(&o, &defs()) - 1.5).abs() < 1e-6);
        let max = RunOptions { density_pct: 500, health_pct: 500, damage_pct: 300, speed_pct: 150 };
        assert_eq!(option_bonus(&max, &defs()), 2.0);
        // density counts 0.1 for every 100 %
        let dense = RunOptions { density_pct: 300, ..Default::default() };
        assert!((option_bonus(&dense, &defs()) - 1.2).abs() < 1e-6);
    }

    /// The shipped `zone_scaling` (`balance.ron`).
    fn scaling() -> ZoneScaling {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap().balance.zone_scaling
    }

    #[test]
    fn drop_odds_by_zone() {
        let (s, d) = (scaling(), RunOptions::default());
        let at = |z: f32| gear_drop_chance(&s, z, &d, &defs());
        // steps by D: 100 % at 1, 80 % from 2, 70 % from 4, 60 % from 6, 40 % from 8 on
        assert_eq!([at(1.0), at(2.0), at(3.0), at(4.0), at(5.0), at(6.5), at(7.0), at(8.0), at(12.0)], [1.0, 0.8, 0.8, 0.7, 0.7, 0.6, 0.6, 0.4, 0.4]);
        assert_eq!(at(0.5), 1.0, "below the first step, the first");
        let hard = RunOptions { speed_pct: 150, ..Default::default() };
        assert!((gear_drop_chance(&s, 3.0, &hard, &defs()) - 1.6).abs() < 1e-6);
    }

    #[test]
    fn boss_slots_by_tier_group() {
        // nine zones of four bosses and a final one
        let zones = vec![vec![false, false, false, false, true]; 9];
        assert_eq!(boss_drop_slots(&zones, 0, 0, 1), vec![0, 4]);
        assert_eq!(boss_drop_slots(&zones, 0, 3, 1), vec![3, 7]);
        // every zone drops its whole set, two slots a boss
        assert_eq!(boss_drop_slots(&zones, 1, 2, 1), vec![2, 6]);
        assert_eq!(boss_drop_slots(&zones, 2, 0, 1), vec![0, 4]);
        assert_eq!(boss_drop_slots(&zones, 2, 3, 1), vec![3, 7]);
        // final bosses drop the weapons
        assert_eq!(boss_drop_slots(&zones, 8, 4, 2), vec![8, 9]);
    }

    #[test]
    fn talent_experience_of_a_won_elwynn_run() {
        let s = scaling();
        // 16 min, 2000 kills, 5 bosses, won: 160 + 100 + 200 + 250 = 710
        let t = RunTally { seconds: 960.0, kills: 2000, bosses: 5, won: true };
        assert_eq!(talent_experience(&t, &s, 1.0, &RunOptions::default(), &defs()), (710, 0, 0));
        // same run in `aq` (D 10): x 10^0.17 = x1.48
        assert_eq!(talent_experience(&t, &s, 10.0, &RunOptions::default(), &defs()), (710, 340, 0));
        // part by part, with speed at 125 % (bonus 1.5)
        let fast = RunOptions { speed_pct: 125, ..Default::default() };
        let p = talent_experience_parts(&t, &s, 10.0, &fast, &defs());
        assert_eq!(p, ExperienceParts { time: 160, kills: 100, bosses: 200, victory: 250, zone: 340, options: 525 });
        assert_eq!(p.total(), 710 + 340 + 525);
    }

    #[test]
    fn the_hardest_win_opens_every_zone_up_to_it_and_the_next() {
        assert_eq!(open_zone_count([], 9), 1);
        assert_eq!(open_zone_count([0], 9), 2);
        // an easier win than the hardest changes nothing
        assert_eq!(open_zone_count([2, 0], 9), 4);
        // the last zone has no next; a zone number past the zones counts for nothing
        assert_eq!(open_zone_count([8], 9), 9);
        assert_eq!(open_zone_count([12], 9), 1);
    }

    #[test]
    fn a_zone_s_tier_follows_its_difficulty() {
        let s = scaling();
        let shipped = [1.0, 2.0, 3.0, 4.0, 5.0, 7.0, 8.0, 9.0, 10.0].map(|d| tier_of_difficulty(&s, d));
        assert_eq!(shipped, [0, 1, 1, 2, 2, 3, 3, 4, 4]);
        // a zone between two others takes their tier; past the last start, the last tier
        assert_eq!([tier_of_difficulty(&s, 6.0), tier_of_difficulty(&s, 12.0), tier_of_difficulty(&s, 1.5)], [2, 4, 0]);
    }

    #[test]
    fn talent_point_costs() {
        assert_eq!([talent_point_cost(1), talent_point_cost(2), talent_point_cost(51)], [80, 82, 262]);
        assert_eq!(talent_points(80 + 82 + 10), (2, 10));
        // 11, 21, 31 and all 51 points' total costs
        let total = |n: u32| (1..=n).map(talent_point_cost).sum::<u32>();
        assert_eq!([total(11), total(21), total(31), total(51)], [993, 2153, 3621, 7843]);
        assert_eq!(talent_points(u32::MAX).0, 51);
    }

    #[test]
    fn prestige_points_follow_the_51st_talent_point() {
        // the first costs what a 52nd talent point would, each next one 5 % more
        assert_eq!(prestige_point_cost(1), talent_point_cost(52));
        for n in 1..60 {
            let (a, b) = (prestige_point_cost(n) as f32, prestige_point_cost(n + 1) as f32);
            assert!((b / a - PRESTIGE_POINT_COST_GROWTH).abs() < 0.01, "point {n}: {a} then {b}");
        }
        let all_talents: u32 = (1..=MAX_TALENT_POINTS).map(talent_point_cost).sum();
        // nothing before every talent point is earned, however close
        assert_eq!(prestige_points(all_talents - 1), (0, 0));
        assert_eq!(prestige_points(all_talents), (0, 0));
        assert_eq!(prestige_points(all_talents + prestige_point_cost(1) - 1), (0, prestige_point_cost(1) - 1));
        let three = (1..=3).map(prestige_point_cost).sum::<u32>();
        assert_eq!(prestige_points(all_talents + three + 7), (3, 7));
        // the talent points stay at 51 meanwhile; the curve ends without overflow
        assert_eq!(talent_points(all_talents + three).0, MAX_TALENT_POINTS);
        assert!(prestige_points(u32::MAX).0 > 100);
        assert_eq!(prestige_point_cost(u32::MAX), u32::MAX);
    }
}
