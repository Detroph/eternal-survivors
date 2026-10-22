//! A class's talents, from the content and the class's progress, under the same rules the
//! server buys them by (`esurv_sim::game::talents`): each talent's rank, whether a rank can
//! be learned or taken back, how it shows (as the stock talent frame: learnable, maxed, or not
//! yet open), and its tooltip in our words.

use esurv_content::{Content, Talent, TalentEffect, TalentTree};
use esurv_protocol::Progress;
use esurv_sim::game::talents::{self, Ranks, POINTS_PER_ROW};

use crate::cards::card_text::with_value;
use crate::stock::TipLine;
use crate::ui::{GOLD_TEXT, GRAY_TEXT, GREEN_TEXT, RED_TEXT, WHITE_TEXT};

/// How a talent shows, as the stock talent frame colours it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TalentLook {
    /// Open: its slot and rank green.
    Open,
    /// At its last rank: gold.
    Maxed,
    /// Its row or the talent it needs is not there yet: greyed.
    Closed,
}

/// The class's three trees.
pub fn trees<'c>(content: &'c Content, class: &str) -> Vec<&'c TalentTree> {
    content
        .talent_trees
        .iter()
        .filter(|t| t.class == class)
        .collect()
}

/// The class's ranks, by talent id.
pub fn ranks(progress: &Progress) -> Ranks {
    progress.talents.iter().cloned().collect()
}

/// Every rank bought in tree `tree`.
pub fn points_in(content: &Content, class: &str, ranks: &Ranks, tree: usize) -> u32 {
    talents::points_in_tree(content, class, ranks, tree, u8::MAX)
}

/// Points earned and not spent.
pub fn points_free(progress: &Progress) -> u32 {
    progress.points.saturating_sub(progress.spent)
}

/// Talent `t` of tree `tree` as the window shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct TalentView {
    pub id: String,
    pub rank: u32,
    pub max_rank: u32,
    pub look: TalentLook,
    pub can_learn: bool,
    pub can_unlearn: bool,
}

/// How the window shows talent `t` (in tree `tree`) of `class`, with `ranks` bought and `points`
/// earned.
pub fn view(
    content: &Content,
    class: &str,
    ranks: &Ranks,
    points: u32,
    tree: usize,
    t: &Talent,
) -> TalentView {
    let rank = ranks.get(&t.id).copied().unwrap_or(0);
    let max_rank = u32::from(t.max_rank);
    let can_learn = talents::can_learn(content, class, ranks, &t.id, points);
    let mut after = ranks.clone();
    let can_unlearn = rank > 0 && talents::unlearn(content, class, &mut after, &t.id);
    let look = if rank >= max_rank {
        TalentLook::Maxed
    } else if row_open(content, class, ranks, tree, t) && required_met(content, class, ranks, t) {
        TalentLook::Open
    } else {
        TalentLook::Closed
    };
    TalentView {
        id: t.id.clone(),
        rank,
        max_rank,
        look,
        can_learn,
        can_unlearn,
    }
}

fn row_open(content: &Content, class: &str, ranks: &Ranks, tree: usize, t: &Talent) -> bool {
    talents::points_in_tree(content, class, ranks, tree, t.row) >= POINTS_PER_ROW * u32::from(t.row)
}

/// The talent `t` requires, if any, and whether it is at its last rank.
fn required<'c>(content: &'c Content, class: &str, t: &Talent) -> Option<&'c Talent> {
    let id = t.requires.as_ref()?;
    trees(content, class)
        .into_iter()
        .flat_map(|tree| &tree.talents)
        .find(|x| &x.id == id)
}

fn required_met(content: &Content, class: &str, ranks: &Ranks, t: &Talent) -> bool {
    required(content, class, t)
        .is_none_or(|r| ranks.get(&r.id).copied().unwrap_or(0) >= u32::from(r.max_rank))
}

/// The number effect `e` brings at `rank` (rank 1 when none is bought): its number times the
/// rank, as the run counts it; a milestone's count where it has no number that grows.
pub fn effect_number(e: &TalentEffect, rank: u32) -> Option<f32> {
    let r = rank.max(1) as f32;
    match e {
        TalentEffect::Stat { per_rank, .. } | TalentEffect::AbilityMod { per_rank, .. } => {
            Some(per_rank * r)
        }
        TalentEffect::Milestone(m) => {
            let mut m = m.clone();
            if let Some(v) = m.rank_value_mut() {
                return Some(*v * r);
            }
            match m {
                esurv_content::Milestone::EveryNth { count, .. }
                | esurv_content::Milestone::SpreadOnDeath { count, .. } => Some(count as f32),
                _ => None,
            }
        }
    }
}

/// The talent's text at `rank`: each `{v}` in turn shows the number of the effect in its place
/// (the last one past the effects), as a percentage where a `%` follows it.
pub fn talent_text(t: &Talent, rank: u32) -> String {
    let numbers: Vec<Option<f32>> = t.effects.iter().map(|e| effect_number(e, rank)).collect();
    let mut out = String::with_capacity(t.text.len());
    let mut rest = t.text.as_str();
    let mut i = 0;
    while let Some(at) = rest.find("{v}") {
        out.push_str(&rest[..at]);
        rest = &rest[at + 3..];
        let number = numbers.get(i).or(numbers.last()).copied().flatten();
        if let Some(v) = number {
            let template = if rest.starts_with('%') { "{v}%" } else { "{v}" };
            if template == "{v}%" {
                rest = &rest[1..];
            }
            out.push_str(&with_value(template, v));
        }
        i += 1;
    }
    out.push_str(rest);
    out
}

/// Whether the talent is a milestone (`Talent::is_milestone`).
pub fn is_milestone(t: &Talent) -> bool {
    t.is_milestone()
}

/// The tooltip of talent `t` in tree `tree`, as the stock talent tooltip lays it out: its name,
/// its rank, what it still needs (red), what it does now (gold), what the next rank does, and
/// what a click does.
pub fn tip(
    content: &Content,
    class: &str,
    ranks: &Ranks,
    points: u32,
    tree: usize,
    t: &Talent,
) -> Vec<TipLine> {
    let v = view(content, class, ranks, points, tree, t);
    let mut lines = vec![
        TipLine::new(t.name.clone(), WHITE_TEXT),
        TipLine::new(format!("Rank {}/{}", v.rank, v.max_rank), WHITE_TEXT),
    ];
    if v.rank < v.max_rank {
        if !row_open(content, class, ranks, tree, t) {
            let tree_name = trees(content, class)
                .get(tree)
                .map_or(String::new(), |x| x.name.clone());
            lines.push(TipLine::new(
                format!(
                    "Requires {} points in {tree_name} Talents",
                    POINTS_PER_ROW * u32::from(t.row)
                ),
                RED_TEXT,
            ));
        }
        if let Some(r) = required(content, class, t) {
            if !required_met(content, class, ranks, t) {
                lines.push(TipLine::new(
                    format!("Requires {} points in {}", r.max_rank, r.name),
                    RED_TEXT,
                ));
            }
        }
    }
    lines.push(TipLine::new(talent_text(t, v.rank), GOLD_TEXT));
    if v.rank > 0 && v.rank < v.max_rank {
        lines.push(TipLine::new("Next rank:", WHITE_TEXT));
        lines.push(TipLine::new(talent_text(t, v.rank + 1), GOLD_TEXT));
    }
    if is_milestone(t) {
        lines.push(TipLine::new(
            "A milestone: it changes how your spells play.",
            GREEN_TEXT,
        ));
    }
    if v.can_learn {
        lines.push(TipLine::new("Click to learn a rank.", GREEN_TEXT));
    }
    if v.can_unlearn {
        lines.push(TipLine::new("Right-click to take a rank back.", GRAY_TEXT));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero_sheet::gear::tests::content;

    fn progress(talents: Vec<(&str, u32)>, points: u32) -> Progress {
        Progress {
            class: "warrior".into(),
            talent_experience: 0,
            points,
            spent: talents.iter().map(|(_, r)| r).sum(),
            into_next: 0,
            next_cost: 80,
            zones_open: 1,
            zones_discovered: Vec::new(),
            talents: talents.into_iter().map(|(i, r)| (i.into(), r)).collect(),
            worn: vec![],
            looks: vec![],
            collection: vec![],
            prestige: Default::default(),
            loadout: String::new(),
            new_gear: false,
            new_points: false,
        }
    }

    #[test]
    fn every_talent_text_reads_without_its_placeholder() {
        let c = content();
        for tree in &c.talent_trees {
            for t in &tree.talents {
                for rank in 0..=u32::from(t.max_rank) {
                    let text = talent_text(t, rank);
                    assert!(!text.contains("{v}"), "{}: {text}", t.id);
                }
            }
        }
        let arms = trees(c, "warrior")[0];
        let heroic = arms.talents.iter().find(|t| t.id == "imp_heroic").unwrap();
        assert_eq!(talent_text(heroic, 0), "Weapon damage +5%.");
        assert_eq!(talent_text(heroic, 3), "Weapon damage +15%.");
    }

    #[test]
    fn a_talent_shows_open_maxed_or_closed_as_the_rules_say() {
        let c = content();
        let arms = trees(c, "warrior")[0];
        let top = arms.talents.iter().find(|t| t.id == "imp_heroic").unwrap();
        let second = arms
            .talents
            .iter()
            .find(|t| t.row == 1 && t.requires.is_none())
            .unwrap();
        let p = progress(vec![("imp_heroic", 4)], 6);
        let r = ranks(&p);
        let v = view(c, "warrior", &r, p.points, 0, top);
        assert_eq!(
            (v.rank, v.look, v.can_learn, v.can_unlearn),
            (4, TalentLook::Open, true, true)
        );
        let v = view(c, "warrior", &r, p.points, 0, second);
        assert_eq!((v.look, v.can_learn), (TalentLook::Closed, false));
        let tip_lines = tip(c, "warrior", &r, p.points, 0, second);
        assert!(tip_lines
            .iter()
            .any(|l| l.text == "Requires 5 points in TalentTab 161 Talents" && l.colour == RED_TEXT));
        let p = progress(vec![("imp_heroic", 5)], 6);
        let r = ranks(&p);
        let v = view(c, "warrior", &r, p.points, 0, top);
        assert_eq!((v.look, v.can_learn), (TalentLook::Maxed, false));
        assert!(view(c, "warrior", &r, p.points, 0, second).can_learn);
        assert_eq!(points_in(c, "warrior", &r, 0), 5);
        assert_eq!(points_free(&p), 1);
        let p = progress(vec![("imp_heroic", 5)], 5);
        assert!(
            !view(c, "warrior", &ranks(&p), p.points, 0, second).can_learn,
            "no point left"
        );
    }

    #[test]
    fn the_tooltip_says_the_next_rank_and_what_a_click_does() {
        let c = content();
        let arms = trees(c, "warrior")[0];
        let top = arms.talents.iter().find(|t| t.id == "imp_heroic").unwrap();
        let p = progress(vec![("imp_heroic", 2)], 3);
        let lines: Vec<String> = tip(c, "warrior", &ranks(&p), p.points, 0, top)
            .into_iter()
            .map(|l| l.text)
            .collect();
        assert_eq!(
            lines,
            [
                "Spell 12282",
                "Rank 2/5",
                "Weapon damage +10%.",
                "Next rank:",
                "Weapon damage +15%.",
                "Click to learn a rank.",
                "Right-click to take a rank back.",
            ]
        );
    }
}
