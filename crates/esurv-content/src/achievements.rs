//! Achievements (`achievements.ron`): what the warband can earn, its points, and what each one
//! asks for, written with a small vocabulary of criteria so a new achievement is data alone.
//! docs/game/achievements.md has the rules; docs/game/AUTHORING.md "An achievement" the how-to.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::Content;

/// The points an achievement may give: Feats of Strength 0, the rest 5 to 50.
pub const ACHIEVEMENT_POINTS: [u32; 5] = [0, 5, 10, 25, 50];

/// One achievement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Achievement {
    /// Unique, lowercase; the save keeps what was earned by it, so it never changes once shipped.
    pub id: String,
    /// Our own name for it.
    pub name: String,
    /// What to do, in our own words.
    pub text: String,
    /// 0, 5, 10, 25 or 50 ([`ACHIEVEMENT_POINTS`]).
    pub points: u32,
    /// A texture name from `Interface\Icons` in the WoW client.
    pub icon: String,
    pub category: AchievementCategory,
    /// What it gives besides its points, by id (titles by their text); the shop, the companions,
    /// the titles and the camp read them in later steps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rewards: Vec<Reward>,
    /// The series it is a step of (`slayer` ...): the steps are the series' achievements in the
    /// file's order, and the window shows the next step with the earlier ones under it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series: Option<String>,
    /// Shown as "???" until earned.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    pub criteria: Criteria,
}

/// A bounty (`bounties.ron`): an achievement earned again each time its goal is met again, its
/// points to spend each time and never the score (plans/hub-bounties.md).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bounty {
    /// Unique, lowercase; kept in the save once shipped.
    pub id: String,
    pub name: String,
    pub text: String,
    /// The points to spend each time it is earned.
    pub points: u32,
    /// A texture name from `Interface\Icons` in the WoW client.
    pub icon: String,
    pub goal: BountyGoal,
}

/// What earns a bounty again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BountyGoal {
    /// Each time the warband's kills pass another step of this many.
    EveryKills(u64),
    /// Each time the warband's gold collected passes another step of this much.
    EveryGold(u64),
    /// Each run in which a zone's boss falls, won or not (once a run).
    BossFalls,
    /// Each run won (once a run).
    ZoneWon,
    /// Each Endless run that lasts this many minutes (once a run).
    EndlessMinutes(u32),
}

impl Content {
    pub fn bounty(&self, id: &str) -> Option<&Bounty> {
        self.bounties.iter().find(|b| b.id == id)
    }

    /// `bounties.ron`'s rules: ids once each and none an achievement's, a name, text and icon,
    /// points and steps above 0.
    pub(crate) fn check_bounties(&self, problems: &mut Vec<String>) {
        let mut seen = HashSet::new();
        for b in &self.bounties {
            let id = &b.id;
            if !seen.insert(id.as_str()) || self.achievement(id).is_some() {
                problems.push(format!("bounty {id} is listed twice, or is an achievement's id"));
            }
            if b.name.trim().is_empty() || b.text.trim().is_empty() || b.icon.trim().is_empty() {
                problems.push(format!("bounty {id} needs a name, a text and an icon"));
            }
            let step = match b.goal {
                BountyGoal::EveryKills(n) | BountyGoal::EveryGold(n) => n,
                BountyGoal::EndlessMinutes(m) => u64::from(m),
                BountyGoal::BossFalls | BountyGoal::ZoneWon => 1,
            };
            if b.points == 0 || step == 0 {
                problems.push(format!("bounty {id} needs points and a step above 0"));
            }
        }
    }
}

/// Where the window lists an achievement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AchievementCategory {
    General,
    /// A class's own page, by class id.
    Class(String),
    /// The creative feats and the Feats of Strength.
    Feats,
}

/// What an achievement gives besides its points, stored by id for the steps that use it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reward {
    /// A title a hero can wear, as its text with [`TITLE_NAME`] where the name goes
    /// ("{name} the Eternal"); a hero wears it by the achievement's id.
    Title(String),
    /// A companion only achievements give, by id.
    Companion(String),
    /// A camp trophy, by id.
    Trophy(String),
    /// Something bigger opened (a class), by id.
    Unlock(String),
}

/// What earns an achievement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Criteria {
    /// A won run where every condition held (none: any win). Challenge Mode counts.
    Win(Vec<RunCondition>),
    /// A boss fight that went this way, won run or not: earned as the boss falls.
    Fight(BossFight),
    /// A warband counter reaching its goal.
    Counter { counter: WarbandCounter, goal: Goal },
    /// Every mark of a checklist.
    Checklist(Checklist),
    /// Every one of these achievements (a meta achievement).
    AllOf(Vec<String>),
    /// A hero reaching this level in one run, Endless too: earned the moment it does.
    Level(u32),
    /// Walking the whole camp with its runner: earned as the tour ends.
    CampTour,
    /// Can never be earned yet (kept for something still to come).
    Never,
}

/// One thing a won run must have held for a hero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RunCondition {
    /// In this zone (id).
    Zone(String),
    /// With a hero of this class (id).
    Class(String),
    /// With this starting option (its id, unique among all classes').
    Start(String),
    /// With this weapon loadout of this class.
    Loadout { class: String, id: String },
    /// In Challenge Mode with its slider at this or more, in whole percentages (200: 2x).
    ChallengeAtLeast(u16),
    /// Every run option slider (`run_options.ron`) at its highest.
    RunOptionsAtMax,
    /// No gold spent all run (caches, rerolls), with at least this much picked up.
    NoGoldSpent { collected_at_least: u32 },
    /// No battle standard taken all run.
    NoBattleStandard,
    /// The hero's health never under this share of its most (0.5: half) all run.
    HealthNeverBelow(f32),
    /// The hero's health fell under this share of its most at least `times` separate times;
    /// a time ends once its health is back to `back_to`.
    DroppedBelow { share: f32, back_to: f32, times: u32 },
    /// The hero lost at least this much health over the run.
    HealthLostAtLeast(f64),
    /// These sources landed at least this many killing blows (the last hit before an enemy died).
    KillingBlows { by: Source, at_least: u32 },
    /// These sources' line on the hero's damage meter is its top line.
    TopDamage(Source),
    /// A boss fight in the run went this way.
    Boss(BossFight),
}

/// A boss fight: the boss, and how it went for a hero still standing when it fell. Every
/// condition given must hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BossFight {
    pub boss: BossRef,
    /// It fell within this many seconds of its arrival.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within_seconds: Option<f32>,
    /// The hero lost at most this share of its most health from its arrival to its death
    /// (0.05: 5 %).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lost_at_most: Option<f32>,
    /// The hero's health never under this share of its most from its arrival to its death.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub never_below: Option<f32>,
}

/// A boss of a zone, by the zone's id and the boss's place in its `bosses` list (0 first).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BossRef {
    pub zone: String,
    pub boss: usize,
}

impl BossRef {
    /// The key the save counts a defeated boss by: `zone/index`.
    pub fn key(&self) -> String {
        boss_key(&self.zone, self.boss)
    }
}

/// The key the save counts a defeated boss by: `zone/index`.
pub fn boss_key(zone: &str, boss: usize) -> String {
    format!("{zone}/{boss}")
}

/// Whose hits count for a killing blow or a damage meter line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// These spells (ability ids), their ticks included.
    Spells(Vec<String>),
    /// The hero's own pets (not a battle standard's ally's).
    Pets,
    /// The hero's weapon swing.
    Swing,
}

/// A number the warband keeps across runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarbandCounter {
    /// Enemies killed, every hero's.
    Kills,
    /// Different zones won.
    ZonesCleared,
    /// Different bosses defeated (each zone's, Warsong's flag carriers too).
    BossesDefeated,
    /// Different legendary amulets in the codex (seen on a card counts).
    LegendaryAmulets,
    /// The most whole minutes of Endless survived in one run.
    EndlessMinutes,
    /// Different classes that have won a run.
    ClassesWon,
    /// Gold collected by every hero across all runs (a companion's when handed over).
    GoldCollected,
}

/// A counter's goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Goal {
    Count(u64),
    /// Every one the content has (zones, bosses, legendaries, classes).
    All,
}

/// A checklist: one mark for each item, made by a won run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Checklist {
    /// A win with each of the class's starting options.
    EveryStartingOption(String),
    /// A win with each of the class's weapon loadouts.
    EveryLoadout(String),
}

impl WarbandCounter {
    /// How many there are of a counter that counts different things; `None` for a plain count.
    pub fn all(self, content: &Content) -> Option<u64> {
        let n = match self {
            WarbandCounter::ZonesCleared => content.zones.len(),
            WarbandCounter::BossesDefeated => content.zones.iter().map(|z| z.bosses.len()).sum(),
            WarbandCounter::LegendaryAmulets => content.legendary_amulets.len(),
            WarbandCounter::ClassesWon => content.classes.len(),
            WarbandCounter::Kills
            | WarbandCounter::EndlessMinutes
            | WarbandCounter::GoldCollected => return None,
        };
        Some(n as u64)
    }
}

impl Goal {
    /// The number to reach (`All`: every one the content has, 0 for a plain count).
    pub fn number(self, counter: WarbandCounter, content: &Content) -> u64 {
        match self {
            Goal::Count(n) => n,
            Goal::All => counter.all(content).unwrap_or(0),
        }
    }
}

impl Checklist {
    /// The items to mark, by id, in the content's order.
    pub fn items(&self, content: &Content) -> Vec<String> {
        match self {
            Checklist::EveryStartingOption(class) => content
                .class(class)
                .map_or_else(Vec::new, |c| c.starting_options.iter().map(|o| o.id.clone()).collect()),
            Checklist::EveryLoadout(class) => content.loadouts_of(class).iter().map(|l| l.id.clone()).collect(),
        }
    }
}

/// Where a hero's name goes in a title's text (`"{name} the Eternal"`, `"{name}, Bossbane"`,
/// `"Spellbound {name}"`: the WoW-like mix).
pub const TITLE_NAME: &str = "{name}";

/// A title's text with `name` in its place: `"Lumi the Eternal"`.
pub fn titled(title: &str, name: &str) -> String {
    title.replacen(TITLE_NAME, name, 1)
}

impl Content {
    pub fn achievement(&self, id: &str) -> Option<&Achievement> {
        self.achievements.iter().find(|a| a.id == id)
    }

    /// The title achievement `id` gives (its text, `{name}` where the hero's name goes); a hero
    /// wears it by the achievement's id, which never changes.
    pub fn title_of(&self, id: &str) -> Option<&str> {
        self.achievement(id)?.rewards.iter().find_map(|r| match r {
            Reward::Title(t) => Some(t.as_str()),
            _ => None,
        })
    }

    /// The achievements that give a title (id, title), in the list's order.
    pub fn titles(&self) -> impl Iterator<Item = (&str, &str)> {
        self.achievements
            .iter()
            .filter_map(|a| Some((a.id.as_str(), self.title_of(&a.id)?)))
    }

    /// A hero's name with the title of achievement `title` (none or an unknown one: the name).
    pub fn titled_name(&self, name: &str, title: Option<&str>) -> String {
        match title.and_then(|t| self.title_of(t)) {
            Some(t) => titled(t, name),
            None => name.to_owned(),
        }
    }

    /// `achievements.ron`'s rules: ids once each, points on the scale, a name, text and icon, known
    /// classes, zones, bosses, starting options, loadouts, spells and meta ids, shares between 0
    /// and 1, goals above 0, an `All` goal only on a counter of different things, series in one
    /// category, and a meta never naming itself or a hidden achievement.
    pub(crate) fn check_achievements(&self, problems: &mut Vec<String>) {
        let mut seen = HashSet::new();
        for a in &self.achievements {
            let id = &a.id;
            if !seen.insert(id.as_str()) {
                problems.push(format!("achievement {id} is listed twice"));
            }
            if !ACHIEVEMENT_POINTS.contains(&a.points) {
                problems.push(format!("achievement {id} gives {} points, not 0, 5, 10, 25 or 50", a.points));
            }
            if a.name.trim().is_empty() || a.text.trim().is_empty() || a.icon.trim().is_empty() {
                problems.push(format!("achievement {id} needs a name, a text and an icon"));
            }
            if let AchievementCategory::Class(c) = &a.category {
                if self.class(c).is_none() {
                    problems.push(format!("achievement {id} is listed under unknown class {c}"));
                }
            }
            for r in &a.rewards {
                let (Reward::Title(x) | Reward::Companion(x) | Reward::Trophy(x) | Reward::Unlock(x)) = r;
                if x.trim().is_empty() {
                    problems.push(format!("achievement {id} has a reward without an id"));
                }
                if let Reward::Title(t) = r {
                    if t.matches(TITLE_NAME).count() != 1 || t.trim() == TITLE_NAME {
                        problems.push(format!(
                            "achievement {id}'s title {t:?} needs {TITLE_NAME} once, with words beside it"
                        ));
                    }
                }
            }
            match &a.criteria {
                Criteria::Win(conditions) => {
                    for c in conditions {
                        self.check_condition(id, c, problems);
                    }
                }
                Criteria::Fight(f) => self.check_fight(id, f, problems),
                Criteria::Counter { counter, goal } => match goal {
                    Goal::Count(0) => problems.push(format!("achievement {id} has a goal of 0")),
                    Goal::All if counter.all(self).is_none() => {
                        problems.push(format!("achievement {id} counts {counter:?}, which has no \"every\", so its goal is a number"))
                    }
                    _ => {}
                },
                Criteria::Checklist(list) => {
                    let (Checklist::EveryStartingOption(c) | Checklist::EveryLoadout(c)) = list;
                    if self.class(c).is_none() || list.items(self).is_empty() {
                        problems.push(format!("achievement {id} names unknown class {c}, or one with nothing to list"));
                    }
                }
                Criteria::AllOf(ids) => {
                    if ids.is_empty() {
                        problems.push(format!("achievement {id} is a meta of nothing"));
                    }
                    for m in ids {
                        match self.achievement(m) {
                            None => problems.push(format!("achievement {id} names unknown achievement {m}")),
                            Some(x) if x.id == *id => problems.push(format!("achievement {id} names itself")),
                            Some(x) if x.hidden || x.criteria == Criteria::Never => {
                                problems.push(format!("achievement {id} names {m}, which can not be earned yet"))
                            }
                            _ => {}
                        }
                    }
                }
                Criteria::Level(level) => {
                    if *level == 0 {
                        problems.push(format!("achievement {id} asks for level 0"));
                    }
                }
                Criteria::CampTour | Criteria::Never => {}
            }
        }
        // a meta of metas must end: no achievement may come back to itself
        for a in &self.achievements {
            if self.meta_loops_back(&a.id, &a.id, &mut HashSet::new()) {
                problems.push(format!("achievement {} needs itself through its metas", a.id));
            }
        }
        let mut series: Vec<(&str, &AchievementCategory, usize)> = Vec::new();
        for a in &self.achievements {
            if let Some(s) = &a.series {
                match series.iter_mut().find(|(x, _, _)| *x == s) {
                    Some((_, category, n)) => {
                        *n += 1;
                        if *category != &a.category {
                            problems.push(format!("series {s} spans two categories (achievement {})", a.id));
                        }
                    }
                    None => series.push((s, &a.category, 1)),
                }
            }
        }
        for (s, _, n) in series.into_iter().filter(|(_, _, n)| *n < 2) {
            problems.push(format!("series {s} has {n} step; a series has two or more"));
        }
    }

    fn meta_loops_back(&self, start: &str, at: &str, visited: &mut HashSet<String>) -> bool {
        let Some(Criteria::AllOf(ids)) = self.achievement(at).map(|a| &a.criteria) else {
            return false;
        };
        for m in ids {
            if m == start {
                return true;
            }
            if visited.insert(m.clone()) && self.meta_loops_back(start, m, visited) {
                return true;
            }
        }
        false
    }

    fn check_condition(&self, id: &str, c: &RunCondition, problems: &mut Vec<String>) {
        let share = |x: f32| x.is_finite() && x > 0.0 && x <= 1.0;
        match c {
            RunCondition::Zone(z) => {
                if self.zone(z).is_none() {
                    problems.push(format!("achievement {id} names unknown zone {z}"));
                }
            }
            RunCondition::Class(c) => {
                if self.class(c).is_none() {
                    problems.push(format!("achievement {id} names unknown class {c}"));
                }
            }
            RunCondition::Start(s) => {
                if !self.classes.iter().any(|c| c.starting_options.iter().any(|o| o.id == *s)) {
                    problems.push(format!("achievement {id} names unknown starting option {s}"));
                }
            }
            RunCondition::Loadout { class, id: l } => {
                if !self.loadouts_of(class).iter().any(|x| x.id == *l) {
                    problems.push(format!("achievement {id} names unknown loadout {class}/{l}"));
                }
            }
            RunCondition::ChallengeAtLeast(pct) => {
                if !(100..=500).contains(pct) {
                    problems.push(format!("achievement {id} asks Challenge at {pct} %, outside 100 to 500"));
                }
            }
            RunCondition::HealthNeverBelow(x) | RunCondition::DroppedBelow { share: x, .. } => {
                if !share(*x) {
                    problems.push(format!("achievement {id} needs a health share above 0 and at most 1"));
                }
                if matches!(c, RunCondition::DroppedBelow { times: 0, .. }) {
                    problems.push(format!("achievement {id} asks to drop below 0 times"));
                }
                if let RunCondition::DroppedBelow { share: s, back_to, .. } = c {
                    if !(*back_to > *s && *back_to <= 1.0) {
                        problems.push(format!("achievement {id} needs back_to above its share and at most 1"));
                    }
                }
            }
            RunCondition::HealthLostAtLeast(x) => {
                if !(x.is_finite() && *x > 0.0) {
                    problems.push(format!("achievement {id} needs health lost above 0"));
                }
            }
            RunCondition::KillingBlows { by, at_least } => {
                self.check_source(id, by, problems);
                if *at_least == 0 {
                    problems.push(format!("achievement {id} asks for 0 killing blows"));
                }
            }
            RunCondition::TopDamage(by) => self.check_source(id, by, problems),
            RunCondition::Boss(f) => self.check_fight(id, f, problems),
            RunCondition::RunOptionsAtMax | RunCondition::NoGoldSpent { .. } | RunCondition::NoBattleStandard => {}
        }
    }

    fn check_source(&self, id: &str, by: &Source, problems: &mut Vec<String>) {
        if let Source::Spells(spells) = by {
            if spells.is_empty() {
                problems.push(format!("achievement {id} names no spell"));
            }
            for s in spells.iter().filter(|s| self.ability(s).is_none()) {
                problems.push(format!("achievement {id} names unknown spell {s}"));
            }
        }
    }

    fn check_fight(&self, id: &str, f: &BossFight, problems: &mut Vec<String>) {
        match self.zone(&f.boss.zone) {
            None => problems.push(format!("achievement {id} names unknown zone {}", f.boss.zone)),
            Some(z) if f.boss.boss >= z.bosses.len() => {
                problems.push(format!("achievement {id} names boss {} of {}, which has {}", f.boss.boss, z.id, z.bosses.len()))
            }
            _ => {}
        }
        let share = |x: Option<f32>| x.is_none_or(|x| x.is_finite() && (0.0..=1.0).contains(&x));
        if !share(f.lost_at_most) || !share(f.never_below) || f.within_seconds.is_some_and(|s| !(s.is_finite() && s > 0.0)) {
            problems.push(format!("achievement {id} needs fight shares from 0 to 1 and seconds above 0"));
        }
    }
}
