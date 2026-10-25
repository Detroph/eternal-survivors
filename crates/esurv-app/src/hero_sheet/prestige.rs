//! A class's prestige, from the content and the class's progress,
//! under the server's own rules (`esurv_sim::game::prestige`): whether the section shows,
//! each stat's rank and bonus, whether a rank can be learned or taken back, and its tooltip.

use esurv_content::{Content, PrestigeStat};
use esurv_protocol::Progress;
use esurv_sim::game::prestige::{self, PrestigeRanks};
use esurv_sim::game::rewards::MAX_TALENT_POINTS;

use crate::stock::TipLine;
use crate::ui::{GOLD_TEXT, GRAY_TEXT, GREEN_TEXT, WHITE_TEXT};

/// Whether the class has every talent point, so the prestige section shows.
pub fn shown(progress: &Progress) -> bool {
    progress.points >= MAX_TALENT_POINTS
}

/// The class's prestige ranks, by prestige stat id.
pub fn ranks(progress: &Progress) -> PrestigeRanks {
    progress.prestige.ranks.iter().cloned().collect()
}

/// Prestige points earned and not spent.
pub fn points_free(progress: &Progress) -> u32 {
    progress.prestige.points.saturating_sub(progress.prestige.spent)
}

/// One prestige stat as the sheet shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct PrestigeView {
    pub rank: u32,
    /// Its cap, if it has one.
    pub max_rank: Option<u32>,
    /// What its ranks give now ("+3%") and what one more would.
    pub bonus: String,
    pub next_bonus: String,
    pub can_learn: bool,
    pub can_unlearn: bool,
}

/// How the sheet shows prestige stat `p` of `class`.
pub fn view(content: &Content, class: &str, progress: &Progress, p: &PrestigeStat) -> PrestigeView {
    let ranks = ranks(progress);
    let rank = ranks.get(&p.id).copied().unwrap_or(0);
    PrestigeView {
        rank,
        max_rank: p.max_ranks,
        bonus: p.bonus_text(rank),
        next_bonus: p.bonus_text(rank + 1),
        can_learn: prestige::can_learn(content, class, &ranks, &p.id, progress.prestige.points),
        can_unlearn: rank > 0,
    }
}

/// "Rank 3", or "Rank 1/3" for a capped stat.
pub fn rank_text(v: &PrestigeView) -> String {
    match v.max_rank {
        Some(max) => format!("Rank {}/{max}", v.rank),
        None => format!("Rank {}", v.rank),
    }
}

/// The tooltip of prestige stat `p`: its name and rank, its bonus now and at the next rank, and
/// what the clicks do (`picked`: it is the stat picked).
pub fn tip(content: &Content, class: &str, progress: &Progress, p: &PrestigeStat, picked: bool) -> Vec<TipLine> {
    let v = view(content, class, progress, p);
    let mut lines = vec![TipLine::new(p.name.clone(), WHITE_TEXT), TipLine::new(rank_text(&v), WHITE_TEXT), TipLine::new(format!("{} {}", v.bonus, p.name.to_lowercase()), GOLD_TEXT)];
    if v.max_rank.is_none_or(|max| v.rank < max) {
        lines.push(TipLine::new("Next rank:", WHITE_TEXT));
        lines.push(TipLine::new(format!("{} {}", v.next_bonus, p.name.to_lowercase()), GOLD_TEXT));
    }
    lines.extend(click_lines(picked, v.can_learn, v.can_unlearn));
    lines
}

/// What a click does, for a talent or a prestige stat: the first
/// click picks it, the next learns a rank; a right click takes one back.
pub fn click_lines(picked: bool, can_learn: bool, can_unlearn: bool) -> Vec<TipLine> {
    let mut lines = Vec::new();
    if can_learn {
        let text = if picked { "Click to learn a rank." } else { "Click to pick it, click again to learn a rank." };
        lines.push(TipLine::new(text, GREEN_TEXT));
    }
    if can_unlearn {
        lines.push(TipLine::new("Right-click to take a rank back.", GRAY_TEXT));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero_sheet::gear::tests::content;
    use esurv_protocol::PrestigeProgress;

    fn progress(points: u32, prestige_points: u32, ranks: Vec<(&str, u32)>) -> Progress {
        Progress {
            class: "warrior".into(),
            talent_experience: 0,
            points,
            spent: 0,
            into_next: 0,
            next_cost: 0,
            zones_open: 1,
            zones_discovered: Vec::new(),
            talents: vec![],
            worn: vec![],
            looks: vec![],
            collection: vec![],
            prestige: PrestigeProgress {
                points: prestige_points,
                spent: ranks.iter().map(|(_, r)| r).sum(),
                into_next: 0,
                next_cost: 500,
                ranks: ranks.into_iter().map(|(i, r)| (i.into(), r)).collect(),
            },
            loadout: String::new(),
            new_gear: false,
            new_points: false,
            challenge_open: false,
            challenge_news: false,
        }
    }

    #[test]
    fn the_section_shows_once_every_talent_point_is_earned() {
        assert!(!shown(&progress(50, 0, vec![])));
        assert!(shown(&progress(51, 0, vec![])));
    }

    #[test]
    fn a_stat_shows_its_rank_its_bonus_and_what_can_be_done() {
        let c = content();
        let stat = |id: &str| c.prestige.iter().find(|p| p.id == id).unwrap();
        let p = progress(51, 4, vec![("damage", 3)]);
        assert_eq!(points_free(&p), 1);
        let v = view(c, "warrior", &p, stat("damage"));
        assert_eq!((v.rank, v.bonus.as_str(), v.next_bonus.as_str()), (3, "+3%", "+4%"));
        assert!(v.can_learn && v.can_unlearn);
        let v = view(c, "warrior", &p, stat("base_health"));
        assert_eq!((v.bonus.as_str(), v.next_bonus.as_str(), v.can_unlearn), ("+0", "+5", false));
        // capped
        let p = progress(51, 4, vec![("extra_lives", 1)]);
        let v = view(c, "warrior", &p, stat("extra_lives"));
        assert_eq!(rank_text(&v), "Rank 1/1");
        assert!(!v.can_learn);
        // no point left
        let p = progress(51, 3, vec![("damage", 3)]);
        assert!(!view(c, "warrior", &p, stat("max_health")).can_learn);
    }

    #[test]
    fn the_tooltip_says_what_the_clicks_do() {
        let texts = |picked| click_lines(picked, true, true).into_iter().map(|l| l.text).collect::<Vec<_>>();
        assert_eq!(texts(false), ["Click to pick it, click again to learn a rank.", "Right-click to take a rank back."]);
        assert_eq!(texts(true)[0], "Click to learn a rank.");
        assert!(click_lines(false, false, false).is_empty());
    }
}
