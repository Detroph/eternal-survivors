//! Buying talents in the armory: a class's talent
//! ranks by talent id, bought one at a time with the points its talent experience has earned. A
//! rank can be bought when a point is free, the talent is short of its last rank, its tree has
//! 5 points per row in the rows above it, and the talent it requires is at its last rank. A rank
//! can be taken back (respecs are free) when every rank left still meets those rules.

use std::collections::BTreeMap;

use esurv_content::{Content, Talent};

use super::rewards::talent_points;

/// A row opens at this many points spent in its tree, per row above it.
pub const POINTS_PER_ROW: u32 = 5;

/// A class's talent ranks, by talent id.
pub type Ranks = BTreeMap<String, u32>;

/// The talent `id` of `class`, with its tree's index (0 to 2).
fn find<'c>(content: &'c Content, class: &str, id: &str) -> Option<(usize, &'c Talent)> {
    content.talent_trees.iter().filter(|t| t.class == class).enumerate().find_map(|(tree, t)| t.talents.iter().find(|x| x.id == id).map(|x| (tree, x)))
}

/// Ranks bought in tree `tree` of `class`, in the rows above `row`.
pub fn points_in_tree(content: &Content, class: &str, ranks: &Ranks, tree: usize, row: u8) -> u32 {
    let Some(t) = content.talent_trees.iter().filter(|t| t.class == class).nth(tree) else { return 0 };
    t.talents.iter().filter(|x| x.row < row).map(|x| ranks.get(&x.id).copied().unwrap_or(0)).sum()
}

/// Every rank bought.
pub fn points_spent(ranks: &Ranks) -> u32 {
    ranks.values().sum()
}

/// Points a class has to spend from its talent experience.
pub fn points_earned(talent_experience: u32) -> u32 {
    talent_points(talent_experience).0
}

/// Whether talent `t` (in tree `tree`) has what it needs at its current ranks: its rows above
/// filled, and the talent it requires at its last rank.
fn opened(content: &Content, class: &str, ranks: &Ranks, tree: usize, t: &Talent) -> bool {
    if points_in_tree(content, class, ranks, tree, t.row) < POINTS_PER_ROW * u32::from(t.row) {
        return false;
    }
    match &t.requires {
        None => true,
        Some(req) => find(content, class, req).is_some_and(|(_, r)| ranks.get(&r.id).copied().unwrap_or(0) >= u32::from(r.max_rank)),
    }
}

/// Whether one more rank of talent `id` can be bought with `points` earned in all.
pub fn can_learn(content: &Content, class: &str, ranks: &Ranks, id: &str, points: u32) -> bool {
    let Some((tree, t)) = find(content, class, id) else { return false };
    points_spent(ranks) < points && ranks.get(id).copied().unwrap_or(0) < u32::from(t.max_rank) && opened(content, class, ranks, tree, t)
}

/// Buys one rank of talent `id`; `false` when it cannot be bought.
pub fn learn(content: &Content, class: &str, ranks: &mut Ranks, id: &str, points: u32) -> bool {
    if !can_learn(content, class, ranks, id, points) {
        return false;
    }
    *ranks.entry(id.into()).or_default() += 1;
    true
}

/// Whether every rank bought still meets the rules.
pub fn valid(content: &Content, class: &str, ranks: &Ranks) -> bool {
    ranks.iter().filter(|(_, &r)| r > 0).all(|(id, &r)| find(content, class, id).is_some_and(|(tree, t)| r <= u32::from(t.max_rank) && opened(content, class, ranks, tree, t)))
}

/// Takes back one rank of talent `id`, when every rank left still meets the rules.
pub fn unlearn(content: &Content, class: &str, ranks: &mut Ranks, id: &str) -> bool {
    let Some(&rank) = ranks.get(id) else { return false };
    if rank == 0 {
        return false;
    }
    let mut after = ranks.clone();
    if rank == 1 {
        after.remove(id);
    } else {
        after.insert(id.into(), rank - 1);
    }
    if !valid(content, class, &after) {
        return false;
    }
    *ranks = after;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    /// The warrior's first tree, its talents by row.
    fn arms(c: &Content) -> &esurv_content::TalentTree {
        c.talent_trees.iter().find(|t| t.class == "warrior").unwrap()
    }

    #[test]
    fn a_row_opens_at_five_points_per_row_above_it() {
        let c = content();
        let tree = arms(&c);
        let top = tree.talents.iter().find(|t| t.row == 0 && t.max_rank >= 5).unwrap();
        let second = tree.talents.iter().find(|t| t.row == 1 && t.requires.is_none()).unwrap();
        let mut ranks = Ranks::new();
        assert!(!can_learn(&c, "warrior", &ranks, &second.id, 51), "row 1 needs 5 points above");
        for _ in 0..4 {
            assert!(learn(&c, "warrior", &mut ranks, &top.id, 51));
        }
        assert!(!can_learn(&c, "warrior", &ranks, &second.id, 51));
        assert!(learn(&c, "warrior", &mut ranks, &top.id, 51));
        assert!(!learn(&c, "warrior", &mut ranks, &top.id, 51), "past its last rank");
        assert!(learn(&c, "warrior", &mut ranks, &second.id, 51), "five points opened row 1");
        // no point left
        assert!(!can_learn(&c, "warrior", &ranks, &second.id, 6));
        // the top talent can't give back a point that holds row 1 open...
        assert!(!unlearn(&c, "warrior", &mut ranks, &top.id));
        // ...until the row-1 rank goes first
        assert!(unlearn(&c, "warrior", &mut ranks, &second.id));
        assert!(unlearn(&c, "warrior", &mut ranks, &top.id));
        assert_eq!(points_spent(&ranks), 4);
    }

    #[test]
    fn a_talent_waits_for_the_one_it_requires_at_its_last_rank() {
        let c = content();
        let tree = arms(&c);
        let needy = tree.talents.iter().find(|t| t.requires.is_some()).unwrap();
        let req = tree.talents.iter().find(|t| Some(&t.id) == needy.requires.as_ref()).unwrap();
        let mut ranks = Ranks::new();
        // fill the rows above with the first rows' talents (no requirement), up to what it needs
        let filler: Vec<&Talent> = tree.talents.iter().filter(|t| t.row < needy.row && t.requires.is_none() && t.id != req.id).collect();
        for t in &filler {
            while points_in_tree(&c, "warrior", &ranks, 0, needy.row) < POINTS_PER_ROW * u32::from(needy.row) && learn(&c, "warrior", &mut ranks, &t.id, 51) {}
        }
        assert!(!can_learn(&c, "warrior", &ranks, &needy.id, 51), "{} first", req.id);
        while learn(&c, "warrior", &mut ranks, &req.id, 51) {}
        assert_eq!(ranks[&req.id], u32::from(req.max_rank));
        assert!(learn(&c, "warrior", &mut ranks, &needy.id, 51));
        assert!(!unlearn(&c, "warrior", &mut ranks, &req.id), "it holds the other open");
        assert!(valid(&c, "warrior", &ranks));
    }

    #[test]
    fn points_come_from_talent_experience() {
        assert_eq!(points_earned(109), 0);
        assert_eq!(points_earned(110), 1);
        assert_eq!(points_earned(110 + 113 + 115), 3);
    }
}
