//! What the warband's achievements show (docs/game/achievements.md, section 8): the rows of the
//! Achievements window's pages ([`rows`]), its Summary ([`summary`]), and the run's tracker and
//! banner ([`in_run`]). Built from the content's achievements and what the save tells
//! (`ToClient::Achievements`): a row's name, text, points, icon and rewards from the content, its
//! progress, date and tracking from the save. A series shows its next step with the earlier ones
//! under it; a hidden achievement is "???" until earned.

pub mod in_run;
pub mod cosmetics;
pub mod shop;

use esurv_content::{Achievement, AchievementCategory, Checklist, Content, Criteria, Goal, Reward};
use esurv_protocol::Achievements;

/// A page of the Achievements window.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum AchievementsPage {
    #[default]
    Summary,
    General,
    /// A class's own page, by its id.
    Class(String),
    Feats,
    /// The bounties: what the warband earns again and again (plans/hub-bounties.md).
    Bounties,
    /// The shop: what the points to spend buy.
    Shop,
}

impl AchievementsPage {
    /// The page a dev script names (`achievements-page:summary`, `...:general`, `...:feats`, or
    /// a class id).
    pub fn named(word: &str) -> AchievementsPage {
        match word {
            "summary" => AchievementsPage::Summary,
            "general" => AchievementsPage::General,
            "feats" => AchievementsPage::Feats,
            "bounties" => AchievementsPage::Bounties,
            "shop" => AchievementsPage::Shop,
            class => AchievementsPage::Class(class.to_owned()),
        }
    }

    /// Whether achievement `a` is on this page.
    pub fn holds(&self, a: &Achievement) -> bool {
        match (self, &a.category) {
            (AchievementsPage::General, AchievementCategory::General) => true,
            (AchievementsPage::Feats, AchievementCategory::Feats) => true,
            (AchievementsPage::Class(c), AchievementCategory::Class(k)) => c == k,
            _ => false,
        }
    }
}

/// How far a row has come, as the window draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowProgress {
    /// Nothing to count (a won run's or a fight's feat).
    None,
    /// A bar: `current` of `goal`.
    Bar { current: u64, goal: u64 },
    /// A checklist: each item's name and whether it is done.
    Checklist(Vec<(String, bool)>),
}

/// One row of a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AchievementRow {
    pub id: String,
    /// "???" for a hidden one not yet earned.
    pub name: String,
    pub text: String,
    pub points: u32,
    pub icon: String,
    /// When it was earned (Unix seconds); `None` not yet.
    pub earned_at: Option<u64>,
    /// A hidden one not yet earned: shown as "???".
    pub secret: bool,
    pub progress: RowProgress,
    /// Its reward lines ("Reward: Title - the Eternal").
    pub rewards: Vec<String>,
    /// A series' earlier steps: name and when earned.
    pub earlier: Vec<(String, Option<u64>)>,
    /// Tracked in runs.
    pub tracked: bool,
}

impl AchievementRow {
    /// Whether it may be ticked to be tracked: not earned, not hidden.
    pub fn trackable(&self) -> bool {
        self.earned_at.is_none() && !self.secret
    }
}

/// When achievement `id` was earned, if it was.
pub fn earned_at(told: &Achievements, id: &str) -> Option<u64> {
    told.earned.iter().find(|(e, _)| e == id).map(|(_, at)| *at)
}

/// A reward as its line says it.
pub fn reward_line(r: &Reward) -> String {
    match r {
        Reward::Title(t) => format!("Reward: Title - {t}"),
        Reward::Companion(id) => format!("Reward: Companion - {}", name_of_id(id)),
        Reward::Trophy(id) => format!("Reward: Trophy - {}", name_of_id(id)),
        Reward::Unlock(id) => format!("Reward: Unlocks the {}", name_of_id(id)),
    }
}

/// A reward as its line says it, a companion by its name in the content.
pub fn reward_line_in(content: &esurv_content::Content, r: &Reward) -> String {
    match r {
        Reward::Companion(id) => match content.companion(id) {
            Some(c) => format!("Reward: Companion - {}", c.name),
            None => reward_line(r),
        },
        _ => reward_line(r),
    }
}

/// A stored id as a name: `tiny_hogger` is "Tiny Hogger".
pub fn name_of_id(id: &str) -> String {
    id.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A number with its thousands apart: 25000 is "25,000".
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(d);
    }
    out
}

/// A day as the rows date it: "27 Oct 2026" (UTC).
pub fn date(unix: u64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    // days since 1970-01-01 to a civil date (Howard Hinnant's days-to-civil)
    let z = (unix / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{day} {} {year}", MONTHS[(month - 1) as usize])
}

/// The items a checklist or a meta marks, by id with their names, in order.
fn items(content: &Content, criteria: &Criteria) -> Option<Vec<(String, String)>> {
    match criteria {
        Criteria::Checklist(list) => {
            let ids = list.items(content);
            let name = |id: &str| -> String {
                match list {
                    Checklist::EveryStartingOption(class) => content
                        .class(class)
                        .and_then(|c| c.starting_options.iter().find(|o| o.id == id))
                        .map(|o| o.name.clone()),
                    Checklist::EveryLoadout(class) => content
                        .loadouts_of(class)
                        .iter()
                        .find(|l| l.id == id)
                        .map(|l| l.name.clone()),
                }
                .unwrap_or_else(|| name_of_id(id))
            };
            Some(ids.iter().map(|id| (id.clone(), name(id))).collect())
        }
        Criteria::AllOf(parts) => Some(
            parts
                .iter()
                .map(|id| {
                    let name = content
                        .achievement(id)
                        .map_or_else(|| name_of_id(id), |a| a.name.clone());
                    (id.clone(), name)
                })
                .collect(),
        ),
        _ => None,
    }
}

/// How far achievement `a` has come.
fn progress_of(content: &Content, told: &Achievements, a: &Achievement) -> RowProgress {
    let earned = earned_at(told, &a.id).is_some();
    let told_progress = told.progress.iter().find(|p| p.id == a.id);
    if let Some(items) = items(content, &a.criteria) {
        let done = |id: &str| {
            earned
                || told_progress.is_some_and(|p| p.done.iter().any(|d| d == id))
                || (matches!(a.criteria, Criteria::AllOf(_)) && earned_at(told, id).is_some())
        };
        return RowProgress::Checklist(
            items
                .into_iter()
                .map(|(id, name)| (name, done(&id)))
                .collect(),
        );
    }
    if let Criteria::Counter { counter, goal } = &a.criteria {
        let goal_number = Goal::number(*goal, *counter, content);
        let current = if earned {
            goal_number
        } else {
            told_progress.map_or(0, |p| p.current)
        };
        let goal = told_progress.map_or(goal_number, |p| p.goal).max(1);
        return RowProgress::Bar { current, goal };
    }
    RowProgress::None
}

/// Achievement `a` as a row.
fn row(content: &Content, told: &Achievements, a: &Achievement) -> AchievementRow {
    let earned = earned_at(told, &a.id);
    let secret = a.hidden && earned.is_none();
    AchievementRow {
        id: a.id.clone(),
        name: if secret { "???".into() } else { a.name.clone() },
        text: if secret {
            "Hidden until it is earned.".into()
        } else {
            a.text.clone()
        },
        points: a.points,
        icon: a.icon.clone(),
        earned_at: earned,
        secret,
        progress: if secret {
            RowProgress::None
        } else {
            progress_of(content, told, a)
        },
        rewards: if secret {
            Vec::new()
        } else {
            a.rewards
                .iter()
                // a trophy stands in a camp decoration, parked for the hub: its line is left out
                .filter(|r| {
                    !matches!(r, Reward::Trophy(_)) || crate::achievements::cosmetics::CAMP_DECORATIONS
                })
                .map(|r| reward_line_in(content, r))
                .collect()
        },
        earlier: Vec::new(),
        tracked: told.tracked.iter().any(|t| *t == a.id),
    }
}

/// A page's rows, in the content's order: a series shows once, at its first step's place, as
/// its next step not earned (the last once all are) with the earlier steps under it.
pub fn rows(
    content: &Content,
    told: &Achievements,
    page: &AchievementsPage,
) -> Vec<AchievementRow> {
    let mut out = Vec::new();
    let mut series_done = Vec::<&str>::new();
    for a in content.achievements.iter().filter(|a| page.holds(a)) {
        let Some(series) = a.series.as_deref() else {
            out.push(row(content, told, a));
            continue;
        };
        if series_done.contains(&series) {
            continue;
        }
        series_done.push(series);
        let steps: Vec<&Achievement> = content
            .achievements
            .iter()
            .filter(|s| s.series.as_deref() == Some(series))
            .collect();
        let next = steps
            .iter()
            .position(|s| earned_at(told, &s.id).is_none())
            .unwrap_or(steps.len() - 1);
        let mut r = row(content, told, steps[next]);
        r.earlier = steps[..next]
            .iter()
            .map(|s| (s.name.clone(), earned_at(told, &s.id)))
            .collect();
        out.push(r);
    }
    out
}

/// One category's line on the Summary: its name, achievements earned and in all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryCount {
    pub name: &'static str,
    pub earned: usize,
    pub all: usize,
}

/// The Summary: the score, the points to spend, the last five earned (newest first), and each
/// category's count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub score: u32,
    pub balance: u32,
    pub recent: Vec<AchievementRow>,
    pub categories: Vec<CategoryCount>,
}

/// How many recent achievements the Summary lists.
pub const RECENT: usize = 5;

pub fn summary(content: &Content, told: &Achievements) -> Summary {
    let recent = told
        .earned
        .iter()
        .rev()
        .filter_map(|(id, _)| content.achievement(id))
        .take(RECENT)
        .map(|a| row(content, told, a))
        .collect();
    let count = |name: &'static str, of: &dyn Fn(&AchievementCategory) -> bool| {
        let all: Vec<&Achievement> = content
            .achievements
            .iter()
            .filter(|a| of(&a.category))
            .collect();
        CategoryCount {
            name,
            earned: all
                .iter()
                .filter(|a| earned_at(told, &a.id).is_some())
                .count(),
            all: all.len(),
        }
    };
    Summary {
        score: told.score,
        balance: told.balance,
        recent,
        categories: vec![
            count("General", &|c| *c == AchievementCategory::General),
            count("Classes", &|c| matches!(c, AchievementCategory::Class(_))),
            count("Feats", &|c| *c == AchievementCategory::Feats),
            count("Total", &|_| true),
        ],
    }
}

/// A page's count: earned and in all.
pub fn page_count(
    content: &Content,
    told: &Achievements,
    page: &AchievementsPage,
) -> (usize, usize) {
    let all: Vec<&Achievement> = content
        .achievements
        .iter()
        .filter(|a| page.holds(a))
        .collect();
    let earned = all
        .iter()
        .filter(|a| earned_at(told, &a.id).is_some())
        .count();
    (earned, all.len())
}

/// A row's progress in a few words, as the run's tracker says it: "312 / 1,000", "2 / 3", or
/// nothing.
pub fn progress_words(p: &RowProgress) -> Option<String> {
    match p {
        RowProgress::None => None,
        RowProgress::Bar { current, goal } => {
            Some(format!("{} / {}", thousands(*current), thousands(*goal)))
        }
        RowProgress::Checklist(items) => {
            let done = items.iter().filter(|(_, d)| *d).count();
            Some(format!("{done} / {}", items.len()))
        }
    }
}

/// The tracked achievements as rows, in the order ticked.
pub fn tracked_rows(content: &Content, told: &Achievements) -> Vec<AchievementRow> {
    told.tracked
        .iter()
        .filter_map(|id| content.achievement(id))
        .map(|a| row(content, told, a))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_protocol::AchievementProgress;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    #[test]
    fn numbers_dates_and_rewards_read_as_the_rows_say_them() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(312), "312");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(250_000), "250,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(date(0), "1 Jan 1970");
        assert_eq!(date(1_793_059_200), "27 Oct 2026");
        assert_eq!(date(951_782_400), "29 Feb 2000");
        assert_eq!(
            reward_line(&Reward::Title("the Eternal".into())),
            "Reward: Title - the Eternal"
        );
        assert_eq!(
            reward_line(&Reward::Companion("tiny_hogger".into())),
            "Reward: Companion - Tiny Hogger"
        );
        assert_eq!(
            reward_line(&Reward::Unlock("death_knight".into())),
            "Reward: Unlocks the Death Knight"
        );
    }

    #[test]
    fn a_series_shows_its_next_step_with_the_earlier_ones_under_it() {
        let content = content();
        let mut told = Achievements::default();
        let general = rows(&content, &told, &AchievementsPage::General);
        let slayer: Vec<_> = general
            .iter()
            .filter(|r| r.name.starts_with("Slayer"))
            .collect();
        assert_eq!(slayer.len(), 1, "one row for the series");
        assert_eq!(
            (slayer[0].name.as_str(), slayer[0].earlier.len()),
            ("Slayer I", 0)
        );
        told.earned = vec![("slayer_1".into(), 1_793_059_200)];
        told.progress = vec![AchievementProgress {
            id: "slayer_2".into(),
            current: 3_120,
            goal: 25_000,
            done: vec![],
        }];
        let general = rows(&content, &told, &AchievementsPage::General);
        let s = general.iter().find(|r| r.id == "slayer_2").unwrap();
        assert_eq!(s.earlier, [("Slayer I".to_string(), Some(1_793_059_200))]);
        assert_eq!(
            s.progress,
            RowProgress::Bar {
                current: 3_120,
                goal: 25_000
            }
        );
        assert_eq!(
            progress_words(&s.progress).as_deref(),
            Some("3,120 / 25,000")
        );
        assert!(general
            .iter()
            .all(|r| r.id != "slayer_1" && r.id != "slayer_3"));
    }

    #[test]
    fn a_class_page_has_its_checklists_and_the_hidden_one_is_secret() {
        let content = content();
        let told = Achievements {
            progress: vec![AchievementProgress {
                id: "warrior_every_start".into(),
                current: 1,
                goal: 3,
                done: vec!["slam".into()],
            }],
            tracked: vec!["warrior_every_start".into()],
            ..Default::default()
        };
        let warrior = rows(&content, &told, &AchievementsPage::Class("warrior".into()));
        assert_eq!(warrior.len(), 3);
        let starts = &warrior[0];
        let RowProgress::Checklist(items) = &starts.progress else {
            panic!("a checklist");
        };
        assert_eq!(items.len(), 3);
        let slam = content.class("warrior").unwrap().starting_options[0]
            .name
            .clone();
        assert_eq!(
            items[0],
            (slam, true),
            "named as the game names the starting option"
        );
        assert!(!items[1].1 && !items[2].1);
        assert!(starts.tracked && starts.trackable());
        let feats = rows(&content, &told, &AchievementsPage::Feats);
        let secret = feats.iter().find(|r| r.id == "death_knight").unwrap();
        assert_eq!(secret.name, "???");
        assert!(secret.secret && !secret.trackable() && secret.rewards.is_empty());
        let told_earned = Achievements {
            earned: vec![("spin_to_win".into(), 1)],
            ..Default::default()
        };
        let spin = rows(
            &content,
            &told_earned,
            &AchievementsPage::Class("warrior".into()),
        );
        let spin = spin.iter().find(|r| r.id == "spin_to_win").unwrap();
        assert_eq!(spin.rewards, ["Reward: Companion - Tiny Gordok"]);
        assert!(!spin.trackable(), "an earned one is not tracked");
    }

    #[test]
    fn the_summary_counts_each_category_and_lists_the_newest_first() {
        let content = content();
        let told = Achievements {
            score: 20,
            balance: 20,
            earned: vec![("first_victory".into(), 1), ("slayer_1".into(), 2)],
            ..Default::default()
        };
        let s = summary(&content, &told);
        assert_eq!((s.score, s.balance), (20, 20));
        assert_eq!(
            s.recent.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["slayer_1", "first_victory"]
        );
        let total = s.categories.iter().find(|c| c.name == "Total").unwrap();
        assert_eq!((total.earned, total.all), (2, content.achievements.len()));
        let general = s.categories.iter().find(|c| c.name == "General").unwrap();
        assert_eq!(general.earned, 2);
        assert_eq!(
            s.categories.iter().take(3).map(|c| c.all).sum::<usize>(),
            content.achievements.len()
        );
    }
}
