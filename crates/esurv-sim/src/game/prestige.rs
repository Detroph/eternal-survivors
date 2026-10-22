//! Prestige: once a class has all its talent points, its talent
//! experience buys prestige points (`rewards::prestige_points`), each spent on one rank of a stat
//! of `prestige.ron`. A rank can be bought when a point is free, the stat is its class's and short
//! of its cap; any rank can be taken back. A talent reset gives them all back.

use std::collections::BTreeMap;

use esurv_content::{Content, PrestigeStat, TalentEffect};

use super::rewards::prestige_points;

/// A class's prestige ranks, by prestige stat id.
pub type PrestigeRanks = BTreeMap<String, u32>;

/// The prestige stats class `class` can buy, in `prestige.ron`'s order.
pub fn stats_of<'c>(content: &'c Content, class: &str) -> Vec<&'c PrestigeStat> {
    content.prestige.iter().filter(|p| p.for_class(class)).collect()
}

/// Prestige stat `id`, when class `class` can buy it.
fn find<'c>(content: &'c Content, class: &str, id: &str) -> Option<&'c PrestigeStat> {
    content.prestige.iter().find(|p| p.id == id && p.for_class(class))
}

/// Points a class has to spend from its talent experience.
pub fn points_earned(talent_experience: u32) -> u32 {
    prestige_points(talent_experience).0
}

/// Every rank bought.
pub fn points_spent(ranks: &PrestigeRanks) -> u32 {
    ranks.values().sum()
}

/// Whether one more rank of prestige stat `id` can be bought with `points` earned in all.
pub fn can_learn(content: &Content, class: &str, ranks: &PrestigeRanks, id: &str, points: u32) -> bool {
    let Some(p) = find(content, class, id) else { return false };
    let rank = ranks.get(id).copied().unwrap_or(0);
    points_spent(ranks) < points && p.max_ranks.is_none_or(|max| rank < max)
}

/// Buys one rank of prestige stat `id`; `false` when it cannot be bought.
pub fn learn(content: &Content, class: &str, ranks: &mut PrestigeRanks, id: &str, points: u32) -> bool {
    if !can_learn(content, class, ranks, id, points) {
        return false;
    }
    *ranks.entry(id.into()).or_default() += 1;
    true
}

/// Takes back one rank of prestige stat `id`; `false` when it has none.
pub fn unlearn(ranks: &mut PrestigeRanks, id: &str) -> bool {
    match ranks.get(id).copied() {
        None | Some(0) => false,
        Some(1) => ranks.remove(id).is_some(),
        Some(r) => {
            ranks.insert(id.into(), r - 1);
            true
        }
    }
}

/// What the ranks give a hero of class `class` in a run: each stat's effect with its rank (a
/// rank past its cap counts as its cap; an unknown stat, or another class's, gives nothing).
pub fn effects<'c>(content: &'c Content, class: &str, ranks: &PrestigeRanks) -> Vec<(&'c TalentEffect, u32)> {
    ranks
        .iter()
        .filter(|&(_, &r)| r > 0)
        .filter_map(|(id, &r)| find(content, class, id).map(|p| (&p.effect, p.max_ranks.map_or(r, |max| r.min(max)))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::rewards::{prestige_point_cost, talent_point_cost, MAX_TALENT_POINTS};
    use crate::game::run::HeroSetup;
    use crate::game::stats;
    use esurv_content::Stat;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    #[test]
    fn points_come_after_every_talent_point() {
        let all: u32 = (1..=MAX_TALENT_POINTS).map(talent_point_cost).sum();
        assert_eq!(points_earned(all), 0);
        assert_eq!(points_earned(all + prestige_point_cost(1)), 1);
        assert_eq!(points_earned(all + prestige_point_cost(1) + prestige_point_cost(2)), 2);
    }

    #[test]
    fn one_point_buys_one_rank_up_to_the_cap() {
        let c = content();
        let mut ranks = PrestigeRanks::new();
        assert!(learn(&c, "mage", &mut ranks, "damage", 2));
        assert!(learn(&c, "mage", &mut ranks, "damage", 2));
        assert!(!learn(&c, "mage", &mut ranks, "max_health", 2), "no point left");
        assert_eq!(ranks["damage"], 2);
        // a capped stat stops at its cap
        assert!(learn(&c, "mage", &mut ranks, "extra_lives", 10));
        assert!(!learn(&c, "mage", &mut ranks, "extra_lives", 10));
        // another class's stat, or an unknown one, is never bought
        assert!(!learn(&c, "mage", &mut ranks, "block_chance", 10));
        assert!(learn(&c, "warrior", &mut ranks, "block_chance", 10));
        assert!(!learn(&c, "mage", &mut ranks, "targets", 10));
        // taken back a rank at a time
        assert!(unlearn(&mut ranks, "damage"));
        assert!(unlearn(&mut ranks, "damage"));
        assert!(!unlearn(&mut ranks, "damage"));
        assert!(!ranks.contains_key("damage"));
    }

    #[test]
    fn the_table_holds_the_defences_and_never_targets() {
        let c = content();
        assert!(c.prestige.iter().all(|p| !p.adds_targets()));
        let stat_of = |id: &str| match &c.prestige.iter().find(|p| p.id == id).unwrap().effect {
            TalentEffect::Stat { stat, per_rank } => (*stat, *per_rank),
            other => panic!("{other:?}"),
        };
        assert_eq!(stat_of("max_health"), (Stat::MaxHealth, 0.01));
        assert_eq!(stat_of("damage_reduction"), (Stat::DamageReduction, 0.01));
    }

    #[test]
    fn ranks_apply_in_a_run_like_talents() {
        let c = content();
        let class = c.class("mage").unwrap();
        let ranks: PrestigeRanks = [("damage".to_owned(), 5), ("max_health".to_owned(), 10), ("extra_lives".to_owned(), 4), ("block_chance".to_owned(), 3)].into();
        let mut setup = HeroSetup { class: "mage".into(), ..Default::default() };
        setup.add_prestige(&c, &ranks);
        let base = stats::compute(class, 1, &[]);
        let with = stats::compute(class, 1, &setup.modifiers);
        assert!((with.damage - base.damage - 0.05).abs() < 1e-5);
        assert!((with.max_health / base.max_health - 1.10).abs() < 1e-4);
        // the life is capped at one rank; the Warrior's block gives a mage nothing
        let lives: f32 = setup.modifiers.iter().filter(|m| m.stat == Stat::ExtraLives).map(|m| m.value).sum();
        assert_eq!(lives, 1.0);
        assert_eq!(with.block_chance, 0.0);
    }
}
