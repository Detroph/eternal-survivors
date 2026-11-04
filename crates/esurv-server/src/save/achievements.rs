//! The warband's achievements as the save keeps them (docs/game/achievements.md): what was
//! earned and when, and the facts the counters and checklists count from. Runs bring their facts
//! ([`AchievementFacts`]); the counters, checklists and metas are counted from what is kept, so an
//! achievement added later counts what was kept before it.
//!
//! A save from before achievements has no section. The first time it is read with the content
//! it is given one from what it can prove ([`SavedAchievements::from_before`]).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use esurv_content::{boss_key, BountyGoal, Checklist, Content, Criteria, RunCondition, WarbandCounter};
use esurv_protocol as protocol;

use super::{SaveFile, Saves};

/// The save's achievements section.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedAchievements {
    /// Every achievement earned, by id, with when (Unix seconds).
    pub earned: BTreeMap<String, u64>,
    /// Enemies killed by every hero.
    pub kills: u64,
    /// Gold collected by every hero (counted from when the save first kept it).
    pub gold_collected: u64,
    /// The zones won (in any mode), by id.
    pub zones_cleared: BTreeSet<String>,
    /// The bosses defeated, as `zone/index` (`esurv_content::boss_key`).
    pub bosses_defeated: BTreeSet<String>,
    /// The classes that have won a run, by id.
    pub classes_won: BTreeSet<String>,
    /// The starting options won with, by id.
    pub starts_won: BTreeSet<String>,
    /// The weapon loadouts won with, as `class/loadout`.
    pub loadouts_won: BTreeSet<String>,
    /// The most seconds of Endless survived in one run.
    pub longest_endless: u32,
    /// The points the shop took.
    pub spent: u32,
    /// The achievements tracked in runs, by id, in the order ticked (at most
    /// `esurv_protocol::MAX_TRACKED`; one earned since drops out as it is told).
    pub tracked: Vec<String>,
    /// How many achievements were earned when the Achievements window was last opened: more
    /// earned since makes its button glow.
    pub seen: u32,
    /// What the shop sold the warband, as its items (`companion:<id>`, `companion_rank:<n>`),
    /// in the order bought.
    pub bought: Vec<String>,
    /// Each camp's decorations (`horde`, `alliance`): the piece on each spot, by id.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub camps: BTreeMap<String, BTreeMap<String, String>>,
    /// The walkable camps' decorations (`ESURV_HUB`; plans/hub-decoration-spots.md), apart from
    /// the warband screen's: by camp, the piece on each spot (`fire_1` ...), by id.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub hub: BTreeMap<String, BTreeMap<String, String>>,
    /// The bounties earned (`bounties.ron`), by id: how many times, and when last.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub bounties: BTreeMap<String, BountyRecord>,
}

/// How often a bounty was earned, and when last (Unix seconds).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BountyRecord {
    pub times: u32,
    pub last: u64,
}

/// What a run brings the warband's achievements for one hero.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AchievementFacts {
    pub kills: u64,
    /// Gold collected since last told.
    pub gold: u64,
    /// A won run: its zone, the hero's class, starting option and loadout (`class/loadout`).
    pub zone_cleared: Option<String>,
    pub class_won: Option<String>,
    pub start_won: Option<String>,
    pub loadout_won: Option<String>,
    /// Bosses defeated, as `zone/index`.
    pub bosses_defeated: Vec<String>,
    /// Seconds of Endless survived in the run so far.
    pub endless_seconds: u32,
    /// Achievements whose won-run or fight conditions held, by id.
    pub met: Vec<String>,
    /// Bounties earned once a run that this run earned, by id (the run counts each once).
    pub bounties: Vec<String>,
}

/// The `class/loadout` key the save counts a loadout won with by.
pub fn loadout_key(class: &str, loadout: &str) -> String {
    format!("{class}/{loadout}")
}

impl SavedAchievements {
    /// The section a save from before achievements gets, from what it can prove: every zone a
    /// class won (Normal Mode keeps no other), each such zone's final bosses as defeated, every
    /// class that won a zone or whose hero has a win in its records as a class won, and every
    /// won-run achievement that asks only for zones and classes a class's won zones show (First
    /// Victory). The counters the save never kept (kills, Endless, starting options and loadouts
    /// won with) start at nothing; the codex is read as it is. What follows from these (Wanderer,
    /// Of Nine Paths ...) is earned with them, at `now`.
    pub(super) fn from_before(content: &Content, file: &SaveFile, now: u64) -> Self {
        let mut s = Self::default();
        for (class, p) in &file.classes {
            for zone in &p.zones_won {
                s.zones_cleared.insert(zone.clone());
                s.classes_won.insert(class.clone());
            }
        }
        for h in file.heroes.iter().filter(|h| h.records.wins > 0) {
            s.classes_won.insert(h.class.clone());
        }
        for z in content
            .zones
            .iter()
            .filter(|z| s.zones_cleared.contains(&z.id))
        {
            for (i, _) in z.bosses.iter().enumerate().filter(|(_, b)| b.is_final) {
                s.bosses_defeated.insert(boss_key(&z.id, i));
            }
        }
        for a in &content.achievements {
            let Criteria::Win(conditions) = &a.criteria else {
                continue;
            };
            let proven = file.classes.iter().any(|(class, p)| {
                p.zones_won.iter().any(|zone| {
                    conditions.iter().all(|c| match c {
                        RunCondition::Zone(z) => z == zone,
                        RunCondition::Class(c) => c == class,
                        _ => false,
                    })
                })
            });
            if proven {
                s.earned.insert(a.id.clone(), now);
            }
        }
        s.earn_what_follows(content, &file.codex, now);
        s
    }

    /// Adds a run's facts; the achievements they earn, by id, in the order earned (a meta after
    /// its parts).
    pub fn add(
        &mut self,
        content: &Content,
        codex: &BTreeSet<String>,
        facts: &AchievementFacts,
        now: u64,
    ) -> Vec<String> {
        self.kills = self.kills.saturating_add(facts.kills);
        self.gold_collected = self.gold_collected.saturating_add(facts.gold);
        self.zones_cleared
            .extend(facts.zone_cleared.iter().cloned());
        self.classes_won.extend(facts.class_won.iter().cloned());
        self.starts_won.extend(facts.start_won.iter().cloned());
        self.loadouts_won.extend(facts.loadout_won.iter().cloned());
        self.bosses_defeated
            .extend(facts.bosses_defeated.iter().cloned());
        self.longest_endless = self.longest_endless.max(facts.endless_seconds);
        let mut new = Vec::new();
        for id in &facts.met {
            let known = content
                .achievement(id)
                .is_some_and(|a| a.criteria != Criteria::Never);
            if known && !self.earned.contains_key(id) {
                self.earned.insert(id.clone(), now);
                new.push(id.clone());
            }
        }
        new.extend(self.earn_what_follows(content, codex, now));
        new
    }

    /// Earns the bounties: those the run earned, and each kills and gold one once for every step
    /// the warband's counts passed; their ids, one for each time earned.
    pub fn earn_bounties(&mut self, content: &Content, facts: &AchievementFacts, now: u64) -> Vec<String> {
        let mut new = Vec::new();
        for b in &content.bounties {
            let record = self.bounties.entry(b.id.clone()).or_default();
            let times = match b.goal {
                BountyGoal::EveryKills(step) => (self.kills / step.max(1)) as u32,
                BountyGoal::EveryGold(step) => (self.gold_collected / step.max(1)) as u32,
                _ => record.times + u32::from(facts.bounties.contains(&b.id)),
            };
            for _ in record.times..times {
                new.push(b.id.clone());
            }
            if times > record.times {
                *record = BountyRecord { times, last: now };
            }
            if record.times == 0 {
                self.bounties.remove(&b.id);
            }
        }
        new
    }

    /// The points the bounties earned, every time each.
    pub fn bounty_points(&self, content: &Content) -> u32 {
        self.bounties.iter().filter_map(|(id, r)| Some(content.bounty(id)?.points.saturating_mul(r.times))).sum()
    }

    /// Earns every counter, checklist and meta now complete, until none is new; their ids.
    fn earn_what_follows(
        &mut self,
        content: &Content,
        codex: &BTreeSet<String>,
        now: u64,
    ) -> Vec<String> {
        let mut new = Vec::new();
        loop {
            let done: Vec<String> = content
                .achievements
                .iter()
                .filter(|a| !self.earned.contains_key(&a.id))
                .filter(|a| {
                    self.progress_of(content, codex, &a.criteria)
                        .is_some_and(|p| p.goal > 0 && p.current >= p.goal)
                })
                .map(|a| a.id.clone())
                .collect();
            if done.is_empty() {
                return new;
            }
            for id in done {
                self.earned.insert(id.clone(), now);
                new.push(id);
            }
        }
    }

    /// How far `criteria` has come, for a counter, a checklist or a meta; `None` for the rest
    /// (won runs and fights are met or not).
    fn progress_of(
        &self,
        content: &Content,
        codex: &BTreeSet<String>,
        criteria: &Criteria,
    ) -> Option<protocol::AchievementProgress> {
        let count = |n: usize| n as u64;
        let (current, goal, done) = match criteria {
            Criteria::Counter { counter, goal } => {
                let current = match counter {
                    WarbandCounter::Kills => self.kills,
                    WarbandCounter::GoldCollected => self.gold_collected,
                    WarbandCounter::ZonesCleared => count(
                        content
                            .zones
                            .iter()
                            .filter(|z| self.zones_cleared.contains(&z.id))
                            .count(),
                    ),
                    WarbandCounter::BossesDefeated => count(
                        content
                            .zones
                            .iter()
                            .flat_map(|z| (0..z.bosses.len()).map(move |i| boss_key(&z.id, i)))
                            .filter(|k| self.bosses_defeated.contains(k))
                            .count(),
                    ),
                    WarbandCounter::LegendaryAmulets => count(
                        content
                            .legendary_amulets
                            .iter()
                            .filter(|l| {
                                l.spells
                                    .first()
                                    .is_some_and(|s| codex.contains(&format!("legendary:{s}")))
                            })
                            .count(),
                    ),
                    WarbandCounter::EndlessMinutes => u64::from(self.longest_endless / 60),
                    WarbandCounter::ClassesWon => count(
                        content
                            .classes
                            .iter()
                            .filter(|c| self.classes_won.contains(&c.id))
                            .count(),
                    ),
                };
                (current, goal.number(*counter, content), Vec::new())
            }
            Criteria::Checklist(list) => {
                let items = list.items(content);
                let done: Vec<String> = items
                    .iter()
                    .filter(|item| match list {
                        Checklist::EveryStartingOption(_) => self.starts_won.contains(*item),
                        Checklist::EveryLoadout(class) => {
                            self.loadouts_won.contains(&loadout_key(class, item))
                        }
                    })
                    .cloned()
                    .collect();
                (count(done.len()), count(items.len()), done)
            }
            Criteria::AllOf(ids) => {
                let done: Vec<String> = ids
                    .iter()
                    .filter(|id| self.earned.contains_key(*id))
                    .cloned()
                    .collect();
                (count(done.len()), count(ids.len()), done)
            }
            Criteria::Win(_) | Criteria::Fight(_) | Criteria::Level(_) | Criteria::CampTour | Criteria::Never => {
                return None
            }
        };
        Some(protocol::AchievementProgress {
            id: String::new(),
            current: current.min(goal),
            goal,
            done,
        })
    }

    /// The achievement score: the points of every earned achievement the content still has.
    pub fn score(&self, content: &Content) -> u32 {
        self.earned
            .keys()
            .filter_map(|id| content.achievement(id))
            .map(|a| a.points)
            .sum()
    }

    /// The warband's achievements as the screens are told them: the score, the points to spend,
    /// what was earned (oldest first), and how far each one not yet earned that counts has come.
    pub fn told(&self, content: &Content, codex: &BTreeSet<String>) -> protocol::Achievements {
        let score = self.score(content);
        let mut earned: Vec<(String, u64)> = self
            .earned
            .iter()
            .map(|(id, &at)| (id.clone(), at))
            .collect();
        earned.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        let progress = content
            .achievements
            .iter()
            .filter(|a| !self.earned.contains_key(&a.id) && !a.hidden)
            .filter_map(|a| {
                let p = self.progress_of(content, codex, &a.criteria)?;
                Some(protocol::AchievementProgress {
                    id: a.id.clone(),
                    ..p
                })
            })
            .collect();
        protocol::Achievements {
            score,
            balance: self.balance(content),
            earned,
            progress,
            tracked: self.still_tracked(content),
            news: self.earned.len() > self.seen as usize,
            bought: self.bought.clone(),
            companion_rank: self.companion_rank(content) as u32,
            banish_rank: self.charge_rank(&content.shop.banish_ranks, esurv_content::banish_rank_item) as u32,
            skip_rank: self.charge_rank(&content.shop.skip_ranks, esurv_content::skip_rank_item) as u32,
            zones_cleared: self.zones_cleared.iter().cloned().collect(),
            camps: self.camps_told(content),
            bounties: content
                .bounties
                .iter()
                .map(|b| {
                    let r = self.bounties.get(&b.id).copied().unwrap_or_default();
                    let (current, goal) = match b.goal {
                        BountyGoal::EveryKills(step) => (self.kills % step.max(1), step),
                        BountyGoal::EveryGold(step) => (self.gold_collected % step.max(1), step),
                        _ => (0, 0),
                    };
                    protocol::BountyTold { id: b.id.clone(), times: r.times, last: r.last, current, goal }
                })
                .collect(),
        }
    }

    /// The points to spend: the score and what the bounties earned, less what the shop took.
    pub fn balance(&self, content: &Content) -> u32 {
        (self.score(content) + self.bounty_points(content)).saturating_sub(self.spent)
    }

    /// The companion upgrade's rank the warband has: 1, and one more for each rank after it bought
    /// in turn (no further than the shop's last).
    pub fn companion_rank(&self, content: &Content) -> usize {
        let ranks = content.shop.companion_ranks.len().max(1);
        let mut rank = 1;
        while rank < ranks && self.bought.contains(&esurv_content::companion_rank_item(rank + 1)) {
            rank += 1;
        }
        rank
    }

    /// A banish or skip upgrade's rank the warband has (0: none), its ranks bought in turn.
    pub fn charge_rank(&self, ranks: &[esurv_content::ChargeRank], item: fn(usize) -> String) -> usize {
        let mut rank = 0;
        while rank < ranks.len() && self.bought.contains(&item(rank + 1)) {
            rank += 1;
        }
        rank
    }

    /// The banishes and skips a run gives: the charges of the ranks bought.
    pub fn charges(&self, content: &Content) -> (u32, u32) {
        let of = |ranks: &[esurv_content::ChargeRank], item: fn(usize) -> String| {
            match self.charge_rank(ranks, item) {
                0 => 0,
                n => ranks[n - 1].charges,
            }
        };
        (
            of(&content.shop.banish_ranks, esurv_content::banish_rank_item),
            of(&content.shop.skip_ranks, esurv_content::skip_rank_item),
        )
    }

    /// Whether the warband has companion `id`: bought in the shop, or given by an achievement
    /// earned.
    pub fn owns_companion(&self, content: &Content, id: &str) -> bool {
        let Some(c) = content.companion(id) else {
            return false;
        };
        match c.achievement() {
            Some(a) => self.earned.contains_key(a),
            None => self.bought.contains(&esurv_content::companion_item(id)),
        }
    }

    /// The shop sells item `item` for points to spend (the score stays): refused (false) for an
    /// item the shop does not sell, one already owned, a rank before the one under it, or one
    /// the points do not cover.
    pub fn buy(&mut self, content: &Content, item: &str) -> bool {
        let Some(price) = content.shop_price(item) else {
            return false;
        };
        if self.bought.iter().any(|b| b == item) {
            return false;
        }
        if let Some(rank) = item.strip_prefix("companion_rank:") {
            // the next rank only
            if rank.parse::<usize>().ok() != Some(self.companion_rank(content) + 1) {
                return false;
            }
        }
        for (prefix, ranks, name) in [
            ("banish_rank:", &content.shop.banish_ranks, esurv_content::banish_rank_item as fn(usize) -> String),
            ("skip_rank:", &content.shop.skip_ranks, esurv_content::skip_rank_item),
        ] {
            if let Some(rank) = item.strip_prefix(prefix) {
                if rank.parse::<usize>().ok() != Some(self.charge_rank(ranks, name) + 1) {
                    return false;
                }
            }
        }
        // a damage glow after the one before it, the Great Anvil after the rest of Forge Row
        if content.shop_needs(item).iter().any(|n| !self.bought.contains(n)) {
            return false;
        }
        if price > self.balance(content) {
            return false;
        }
        self.spent += price;
        self.bought.push(item.to_owned());
        true
    }

    /// The tracked achievements the content still has and that are not earned yet.
    fn still_tracked(&self, content: &Content) -> Vec<String> {
        self.tracked
            .iter()
            .filter(|id| content.achievement(id).is_some() && !self.earned.contains_key(*id))
            .cloned()
            .collect()
    }

    /// Tracks achievement `id` in runs (`on`) or stops: refused (false) for one the content
    /// lacks, a hidden or earned one, or a fourth.
    pub fn track(&mut self, content: &Content, id: &str, on: bool) -> bool {
        self.tracked = self.still_tracked(content);
        if !on {
            self.tracked.retain(|t| t != id);
            return true;
        }
        let Some(a) = content.achievement(id) else {
            return false;
        };
        if a.hidden || self.earned.contains_key(id) {
            return false;
        }
        if self.tracked.iter().any(|t| t == id) {
            return true;
        }
        if self.tracked.len() >= protocol::MAX_TRACKED {
            return false;
        }
        self.tracked.push(id.to_owned());
        true
    }
}

/// Now, in Unix seconds.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl SaveFile {
    /// The achievements section, made from what the save can prove when it has none.
    pub(super) fn achievements_section(&mut self, content: &Content, now: u64) -> &mut SavedAchievements {
        if self.achievements.is_none() {
            let made = SavedAchievements::from_before(content, self, now);
            self.achievements = Some(made);
        }
        self.achievements.as_mut().expect("just made")
    }
}

impl Saves {
    /// Gives a save from before achievements its section (granting what it can prove) and earns
    /// whatever is now complete (an achievement added to the content); written when anything
    /// changed. The achievements earned, by id.
    pub fn catch_up_achievements(&self, content: &Content) -> Vec<String> {
        self.add_achievement_facts(content, &AchievementFacts::default())
    }

    /// Adds a run's facts for one hero to the warband's achievements (written when anything
    /// changed); the achievements they earned, by id, in the order earned.
    pub fn add_achievement_facts(
        &self,
        content: &Content,
        facts: &AchievementFacts,
    ) -> Vec<String> {
        let now = unix_now();
        let mut file = self.file.lock().unwrap();
        let before = file.achievements.clone();
        let codex = file.codex.clone();
        let section = file.achievements_section(content, now);
        // what a save from before achievements was just granted comes first
        let had = |id: &str| before.as_ref().is_some_and(|b| b.earned.contains_key(id));
        let mut new: Vec<String> = content
            .achievements
            .iter()
            .filter(|a| section.earned.contains_key(&a.id) && !had(&a.id))
            .map(|a| a.id.clone())
            .collect();
        new.extend(section.add(content, &codex, facts, now));
        new.extend(section.earn_bounties(content, facts, now));
        if file.achievements != before {
            let _ = self.write(&file);
        }
        new
    }

    /// The warband's achievements as kept (a save from before them: what it can prove).
    pub fn achievements(&self, content: &Content) -> SavedAchievements {
        let file = self.file.lock().unwrap();
        match &file.achievements {
            Some(a) => a.clone(),
            None => SavedAchievements::from_before(content, &file, unix_now()),
        }
    }

    /// The warband's achievements as the screens are told them.
    pub fn achievements_told(&self, content: &Content) -> protocol::Achievements {
        let codex: BTreeSet<String> = self.codex().into_iter().collect();
        self.achievements(content).told(content, &codex)
    }

    /// Tracks achievement `id` in runs or stops ([`SavedAchievements::track`]); written when it
    /// changed. False when refused.
    pub fn track_achievement(&self, content: &Content, id: &str, on: bool) -> bool {
        let mut file = self.file.lock().unwrap();
        let section = file.achievements_section(content, unix_now());
        let before = section.clone();
        let ok = section.track(content, id, on);
        if *section != before {
            let _ = self.write(&file);
        }
        ok
    }

    /// The shop sells item `item` to the warband ([`SavedAchievements::buy`]); written when
    /// sold. False when refused.
    pub fn buy(&self, content: &Content, item: &str) -> bool {
        let mut file = self.file.lock().unwrap();
        let section = file.achievements_section(content, unix_now());
        let ok = section.buy(content, item);
        if ok {
            let _ = self.write(&file);
        }
        ok
    }

    /// Hero `guid` brings companion `companion` (`None`: none), one the warband owns; written
    /// when it changed. False for no such hero or a companion not owned.
    pub fn choose_companion(&self, content: &Content, guid: u32, companion: Option<&str>) -> bool {
        let mut file = self.file.lock().unwrap();
        if let Some(id) = companion {
            let section = file.achievements_section(content, unix_now());
            if !section.owns_companion(content, id) {
                return false;
            }
        }
        let Some(hero) = file.heroes.iter_mut().find(|h| h.guid == guid) else {
            return false;
        };
        if hero.companion.as_deref() != companion {
            hero.companion = companion.map(str::to_owned);
            let _ = self.write(&file);
        }
        true
    }

    /// The banishes and skips every hero's run brings, from the warband's upgrades.
    pub fn run_charges(&self, content: &Content) -> (u32, u32) {
        self.achievements(content).charges(content)
    }

    /// What hero `guid`'s companion brings into a run: the companion picked, while the warband
    /// still owns it and the content has it, at the warband's upgrade rank.
    pub fn run_companion(&self, content: &Content, guid: u32) -> Option<(String, usize)> {
        let a = self.achievements(content);
        let file = self.file.lock().unwrap();
        let id = file.heroes.iter().find(|h| h.guid == guid)?.companion.clone()?;
        a.owns_companion(content, &id).then(|| (id, a.companion_rank(content)))
    }

    /// The Achievements window was opened: everything earned so far is seen (its button stops
    /// glowing); written when it changed.
    pub fn see_achievements(&self, content: &Content) {
        let mut file = self.file.lock().unwrap();
        let section = file.achievements_section(content, unix_now());
        let earned = section.earned.len() as u32;
        if section.seen != earned {
            section.seen = earned;
            let _ = self.write(&file);
        }
    }

    /// How many more kills earn the next kill counter's step; `None` when none is left to earn.
    pub fn kills_to_next_step(&self, content: &Content) -> Option<u64> {
        self.to_next_step(content, WarbandCounter::Kills)
    }

    /// How much more of `counter` (kills, gold collected) earns its next step; `None` when none
    /// is left to earn.
    pub fn to_next_step(&self, content: &Content, counter: WarbandCounter) -> Option<u64> {
        let a = self.achievements(content);
        let have = match counter {
            WarbandCounter::Kills => a.kills,
            WarbandCounter::GoldCollected => a.gold_collected,
            _ => return None,
        };
        content
            .achievements
            .iter()
            .filter(|x| !a.earned.contains_key(&x.id))
            .filter_map(|x| match x.criteria {
                Criteria::Counter { counter: c, goal } if c == counter => Some(goal.number(counter, content)),
                _ => None,
            })
            .min()
            .map(|goal| goal.saturating_sub(have))
    }
}

#[cfg(test)]
mod tests;
