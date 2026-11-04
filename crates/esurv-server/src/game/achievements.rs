//! The run's side of the achievements (docs/game/achievements.md): a tracker per run fed by the
//! simulation's events and the heroes' health after every tick, kept per hero. It earns what it
//! can the moment it is met (a boss fight as the boss falls, a kill or gold counter's step as the kill
//! lands or the gold comes in, Endless minutes as they pass) and the won-run achievements as the
//! run is won; while a tracked achievement counts kills or gold, the list is told again (at most
//! once a second) with what the save does not hold yet, so the tracker moves live. The facts
//! go to the save ([`Saves::add_achievement_facts`]), which counts the counters, checklists and
//! metas. Each one earned is told to every client (`ToClient::AchievementEarned`, its banner) and
//! in every chat; the list follows (`ToClient::Achievements`). Challenge Mode counts.

use std::collections::{HashMap, HashSet};

use esurv_content::{boss_key, BossFight, BountyGoal, Content, Criteria, RunCondition, Source, WarbandCounter};
use esurv_protocol::{self as protocol, ToClient};
use esurv_sim::game::casting::HitSource;
use esurv_sim::game::loot::Pickup;
use esurv_sim::game::run::{Event, Hero, Run};

use super::Game;
use crate::packets;
use crate::save::{loadout_key, AchievementFacts};
use crate::wire::Packet;

/// What a run has seen of one hero for the achievements.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct HeroTrack {
    /// Kills not yet added to the save.
    kills_pending: u64,
    /// Killing blows by source: `spell:<id>` (its ticks too), `pets`, `swing`.
    killing_blows: HashMap<String, u32>,
    /// Its lowest health this run, as a share of its most (1 until hurt).
    lowest: Option<f32>,
    /// The times its health fell under each share a `DroppedBelow` asks about:
    /// (share, back to, under now, times).
    drops: Vec<(f32, f32, bool, u32)>,
    /// Health lost this run.
    health_lost: f64,
    /// It took a battle standard.
    took_standard: bool,
    /// The boss fights it stood through to the boss's fall.
    fights: Vec<FightRecord>,
    /// Whole minutes of Endless told to the save.
    endless_minutes: u32,
    /// The run's gold collected already told to the save.
    gold_banked: u32,
}

/// How a boss fight went for a hero, from the boss's arrival to its fall.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FightRecord {
    zone: String,
    boss: usize,
    seconds: f32,
    /// Health lost in the fight, as a share of its most.
    lost: f32,
    /// Its lowest health in the fight, as a share of its most.
    lowest: f32,
}

/// A boss on the field: which, when it came, and each hero's fight so far (health lost, lowest).
#[derive(Debug, Clone, PartialEq)]
struct BossOnField {
    boss: usize,
    arrived: f32,
    heroes: HashMap<u32, (f64, f32)>,
}

/// The run's achievements tracker.
#[derive(Debug, Default)]
pub(super) struct RunTracker {
    heroes: HashMap<u32, HeroTrack>,
    /// The last hit each enemy took, by enemy id: whose and its source key.
    last_hit: HashMap<u32, (u32, String)>,
    bosses: HashMap<u32, BossOnField>,
    /// The battle standards lying on the field, by pickup id.
    standards: HashSet<u32>,
    /// How many kills earn the next kill counter's step, as the save last said (`None`: ask it).
    kills_to_step: Option<Option<u64>>,
    /// How much gold collected earns the next gold counter's step, as the save last said.
    gold_to_step: Option<Option<u64>>,
    /// The won run was counted.
    win_counted: bool,
    /// The kills and gold not yet in the save when the tracker was last told them, and when
    /// (run seconds): the live counters.
    live_told: (u64, u64, f32),
    /// The bounties earned once a run that this run has earned.
    bounties_paid: HashSet<String>,
}

impl RunTracker {
    /// The bounties earned once a run that facts `f` earn and this run has not yet: a boss
    /// fallen, a win, Endless lasted to the mark.
    fn run_bounties(&mut self, content: &Content, f: &AchievementFacts) -> Vec<String> {
        let mut out = Vec::new();
        for b in &content.bounties {
            let met = match b.goal {
                BountyGoal::BossFalls => !f.bosses_defeated.is_empty(),
                BountyGoal::ZoneWon => f.zone_cleared.is_some(),
                BountyGoal::EndlessMinutes(m) => f.endless_seconds >= m * 60,
                BountyGoal::EveryKills(_) | BountyGoal::EveryGold(_) => false,
            };
            if met && self.bounties_paid.insert(b.id.clone()) {
                out.push(b.id.clone());
            }
        }
        out
    }
}

/// `n` as "12th time".
fn nth(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix} time")
}

/// A hero's health as a share of its most.
fn health_share(h: &Hero) -> f32 {
    if h.stats.max_health > 0.0 {
        (h.health / h.stats.max_health).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// Whether hero `h` is still standing (not fallen for good).
fn standing(h: &Hero) -> bool {
    h.alive && h.fell_at.is_none()
}

/// The key a hit's source counts its killing blows under; `None` for what no achievement names.
fn blow_key(run: &Run, source: &HitSource) -> Option<String> {
    match source {
        HitSource::Spell(s) | HitSource::Tick(s) => Some(format!("spell:{s}")),
        HitSource::WeaponSwing => Some("swing".into()),
        // a pet a battle standard's ally summoned is that ally's
        HitSource::Pet(p) if run.pets.iter().any(|x| x.id == *p && x.assist.is_some()) => None,
        HitSource::Pet(_) => Some("pets".into()),
        _ => None,
    }
}

/// Whether a killing-blow key is of `source`.
fn of_source(key: &str, source: &Source) -> bool {
    match source {
        Source::Spells(spells) => key
            .strip_prefix("spell:")
            .is_some_and(|s| spells.iter().any(|x| x == s)),
        Source::Pets => key == "pets",
        Source::Swing => key == "swing",
    }
}

/// Whether a damage meter line is of `source`.
fn on_the_line(line: &protocol::Source, source: &Source) -> bool {
    match (line, source) {
        (protocol::Source::Spell(s), Source::Spells(spells)) => spells.contains(s),
        (protocol::Source::Pet(_), Source::Pets) => true,
        (protocol::Source::Swing, Source::Swing) => true,
        _ => false,
    }
}

/// Whether `source`'s lines together are the top of a damage meter (`done`): above 0 and at least
/// every other line.
pub(super) fn tops_the_meter(done: &[(protocol::Source, f64)], source: &Source) -> bool {
    let mine: f64 = done
        .iter()
        .filter(|(l, _)| on_the_line(l, source))
        .map(|(_, d)| d)
        .sum();
    mine > 0.0
        && done
            .iter()
            .filter(|(l, _)| !on_the_line(l, source))
            .all(|(_, d)| *d <= mine)
}

impl FightRecord {
    /// Whether this fight went the way `f` asks.
    fn meets(&self, f: &BossFight) -> bool {
        self.zone == f.boss.zone
            && self.boss == f.boss.boss
            && f.within_seconds.is_none_or(|s| self.seconds <= s)
            && f.lost_at_most.is_none_or(|x| self.lost <= x)
            && f.never_below.is_none_or(|x| self.lowest >= x)
    }
}

impl RunTracker {
    /// Takes in a tick: its events, then every hero's health after it. The bosses that fell in
    /// it, with each standing hero's fight.
    pub(super) fn observe(
        &mut self,
        run: &Run,
        events: &[Event],
    ) -> Vec<(usize, Vec<(u32, FightRecord)>)> {
        let content = &run.content;
        let mut fell = Vec::new();
        for e in events {
            match e {
                Event::EnemyHit {
                    id,
                    by: Some(hero),
                    source,
                    ..
                } => match blow_key(run, source) {
                    Some(key) => {
                        self.last_hit.insert(*id, (*hero, key));
                    }
                    None => {
                        self.last_hit.remove(id);
                    }
                },
                Event::EnemyDied { id, by } => {
                    if let Some((hero, key)) = self.last_hit.remove(id) {
                        *self
                            .heroes
                            .entry(hero)
                            .or_default()
                            .killing_blows
                            .entry(key)
                            .or_default() += 1;
                    }
                    if let Some(hero) = by {
                        self.heroes.entry(*hero).or_default().kills_pending += 1;
                    }
                    if let Some(b) = self.bosses.remove(id) {
                        let zone = content.zones[run.zone].id.clone();
                        let fights = run
                            .heroes
                            .iter()
                            .filter(|h| standing(h))
                            .map(|h| {
                                let (lost, lowest) = b
                                    .heroes
                                    .get(&h.id)
                                    .copied()
                                    .unwrap_or((0.0, health_share(h)));
                                let most = f64::from(h.stats.max_health.max(1.0));
                                let record = FightRecord {
                                    zone: zone.clone(),
                                    boss: b.boss,
                                    seconds: run.now - b.arrived,
                                    lost: (lost / most) as f32,
                                    lowest,
                                };
                                self.heroes
                                    .entry(h.id)
                                    .or_default()
                                    .fights
                                    .push(record.clone());
                                (h.id, record)
                            })
                            .collect();
                        fell.push((b.boss, fights));
                    }
                }
                Event::BossSpawned { id, boss, .. } => {
                    let heroes = run
                        .heroes
                        .iter()
                        .map(|h| (h.id, (0.0, health_share(h))))
                        .collect();
                    self.bosses.insert(
                        *id,
                        BossOnField {
                            boss: *boss,
                            arrived: run.now,
                            heroes,
                        },
                    );
                }
                Event::HeroHit { hero, lost, .. } => {
                    self.heroes.entry(*hero).or_default().health_lost += f64::from(*lost);
                    for b in self.bosses.values_mut() {
                        b.heroes.entry(*hero).or_insert((0.0, 1.0)).0 += f64::from(*lost);
                    }
                }
                Event::PickupDropped {
                    id,
                    kind: Pickup::BattleStandard(_),
                    ..
                } => {
                    self.standards.insert(*id);
                }
                Event::PickupTaken { id, hero } => {
                    if self.standards.remove(id) {
                        self.heroes.entry(*hero).or_default().took_standard = true;
                    }
                }
                _ => {}
            }
        }
        let shares = drop_shares(content);
        for h in &run.heroes {
            if !standing(h) {
                continue;
            }
            let share = health_share(h);
            let t = self.heroes.entry(h.id).or_default();
            t.lowest = Some(t.lowest.map_or(share, |l| l.min(share)));
            if t.drops.is_empty() {
                t.drops = shares.iter().map(|&(s, back)| (s, back, false, 0)).collect();
            }
            for (at, back_to, under, times) in &mut t.drops {
                if !*under && share < *at {
                    *under = true;
                    *times += 1;
                } else if *under && share >= *back_to {
                    *under = false;
                }
            }
            for b in self.bosses.values_mut() {
                let fight = b.heroes.entry(h.id).or_insert((0.0, share));
                fight.1 = fight.1.min(share);
            }
        }
        fell
    }

    /// Whether every condition holds for hero `hero` of the won run `run` (`meter`: its damage
    /// meter's lines).
    pub(super) fn win_holds(
        &self,
        run: &Run,
        hero: &Hero,
        meter: &[(protocol::Source, f64)],
        conditions: &[RunCondition],
    ) -> bool {
        let content = &run.content;
        let track = self.heroes.get(&hero.id).cloned().unwrap_or_default();
        let class = &content.classes[hero.class].id;
        conditions.iter().all(|c| match c {
            RunCondition::Zone(z) => content.zones[run.zone].id == *z,
            RunCondition::Class(x) => class == x,
            RunCondition::Start(s) => hero.start == *s,
            RunCondition::Loadout { class: c, id } => class == c && hero.loadout == *id,
            RunCondition::ChallengeAtLeast(pct) => run
                .options
                .challenge
                .is_some_and(|ch| ch.slider_pct >= *pct),
            RunCondition::RunOptionsAtMax => content.run_options.iter().all(|s| {
                let o = &run.options;
                let value = match s.id.as_str() {
                    "density" => o.density_pct,
                    "health" => o.health_pct,
                    "damage" => o.damage_pct,
                    "speed" => o.speed_pct,
                    _ => return false,
                };
                value >= s.max_pct
            }),
            RunCondition::NoGoldSpent { collected_at_least } => {
                hero.gold >= hero.gold_earned && hero.gold_earned >= *collected_at_least
            }
            RunCondition::NoBattleStandard => !track.took_standard,
            RunCondition::HealthNeverBelow(x) => track.lowest.unwrap_or(1.0) >= *x,
            RunCondition::DroppedBelow {
                share,
                back_to,
                times,
            } => track
                .drops
                .iter()
                .any(|(s, b, _, n)| s == share && b == back_to && n >= times),
            RunCondition::HealthLostAtLeast(x) => track.health_lost >= *x,
            RunCondition::KillingBlows { by, at_least } => {
                track
                    .killing_blows
                    .iter()
                    .filter(|(k, _)| of_source(k, by))
                    .map(|(_, n)| n)
                    .sum::<u32>()
                    >= *at_least
            }
            RunCondition::TopDamage(by) => tops_the_meter(meter, by),
            RunCondition::Boss(f) => track.fights.iter().any(|r| r.meets(f)),
        })
    }

    /// Hero `hero`'s kills not yet in the save, taken.
    pub(super) fn take_kills(&mut self, hero: u32) -> u64 {
        self.heroes
            .get_mut(&hero)
            .map_or(0, |t| std::mem::take(&mut t.kills_pending))
    }

    /// Every hero's kills not yet in the save.
    pub(super) fn kills_pending(&self) -> u64 {
        self.heroes.values().map(|t| t.kills_pending).sum()
    }
}

/// Every (share, back to) the content's `DroppedBelow` conditions ask about.
fn drop_shares(content: &Content) -> Vec<(f32, f32)> {
    let mut shares: Vec<(f32, f32)> = Vec::new();
    for a in &content.achievements {
        if let Criteria::Win(conditions) = &a.criteria {
            for c in conditions {
                if let RunCondition::DroppedBelow { share, back_to, .. } = c {
                    if !shares.contains(&(*share, *back_to)) {
                        shares.push((*share, *back_to));
                    }
                }
            }
        }
    }
    shares
}

/// How often the tracker's live counters are told at most, run seconds.
const LIVE_EVERY: f32 = 1.0;

/// The chat line telling an achievement earned.
pub fn earned_line(hero: &str, achievement: &str) -> String {
    format!("[{hero}] has earned the achievement [{achievement}]!")
}

impl Game {
    /// After a tick: the tracker takes it in, and whatever it earns now is told. Lines for each
    /// hero's client.
    pub(super) fn track_achievements(&mut self, events: &[Event]) -> Vec<(u32, Packet)> {
        let content = self.run.content.clone();
        let fell = self.achievements.observe(&self.run, events);
        let mut facts: Vec<(u32, AchievementFacts)> = Vec::new();
        for (boss, fights) in fell {
            let key = boss_key(&content.zones[self.run.zone].id, boss);
            for (hero, record) in fights {
                let met = content
                    .achievements
                    .iter()
                    .filter(|a| matches!(&a.criteria, Criteria::Fight(f) if record.meets(f)))
                    .map(|a| a.id.clone())
                    .collect();
                facts.push((
                    hero,
                    AchievementFacts {
                        bosses_defeated: vec![key.clone()],
                        met,
                        ..Default::default()
                    },
                ));
            }
        }
        // a kill counter's step is earned as the kill that reaches it lands
        let pending = self.achievements.kills_pending();
        let to_step = match self.achievements.kills_to_step {
            Some(left) => left,
            None => *self
                .achievements
                .kills_to_step
                .insert(self.saves.kills_to_next_step(&content)),
        };
        if pending > 0 && to_step.is_some_and(|left| pending >= left) {
            for h in self.run.heroes.iter().map(|h| h.id).collect::<Vec<_>>() {
                let kills = self.achievements.take_kills(h);
                if kills > 0 {
                    facts.push((
                        h,
                        AchievementFacts {
                            kills,
                            ..Default::default()
                        },
                    ));
                }
            }
        }
        // a gold counter's step is earned as the gold that reaches it comes in: picked up, from a
        // card, or handed over by a companion; what goes in now is
        // marked banked, so the payout adds only the rest
        let gold_pending = self.gold_pending();
        let to_step = match self.achievements.gold_to_step {
            Some(left) => left,
            None => *self
                .achievements
                .gold_to_step
                .insert(self.saves.to_next_step(&content, WarbandCounter::GoldCollected)),
        };
        if gold_pending > 0 && to_step.is_some_and(|left| gold_pending >= left) {
            for h in self.run.heroes.iter().map(|h| (h.id, h.gold_earned)).collect::<Vec<_>>() {
                let t = self.achievements.heroes.entry(h.0).or_default();
                let gold = h.1.saturating_sub(t.gold_banked);
                t.gold_banked = t.gold_banked.max(h.1);
                if gold > 0 {
                    facts.push((
                        h.0,
                        AchievementFacts {
                            gold: u64::from(gold),
                            ..Default::default()
                        },
                    ));
                }
            }
        }
        // a level asked for is earned the moment a hero reaches it (Peak of Power)
        for e in events {
            if let Event::LevelUp { hero, level } = e {
                let met: Vec<String> = content
                    .achievements
                    .iter()
                    .filter(|a| matches!(a.criteria, Criteria::Level(n) if *level >= n))
                    .map(|a| a.id.clone())
                    .collect();
                if !met.is_empty() {
                    facts.push((*hero, AchievementFacts { met, ..Default::default() }));
                }
            }
        }
        // a legendary amulet shown on a card is in the codex now (the screens put it there)
        for e in events {
            if let Event::ScreenOpened { hero, offers, .. } = e {
                let legendary = offers
                    .iter()
                    .filter_map(|o| o.codex_entry())
                    .any(|id| id.starts_with("legendary:"));
                if legendary {
                    facts.push((*hero, AchievementFacts::default()));
                }
            }
        }
        // Endless, a whole minute at a time
        if let Some(since) = self.run.endless_since {
            let seconds = (self.run.now - since).max(0.0) as u32;
            for h in self
                .run
                .heroes
                .iter()
                .filter(|h| standing(h) && !self.left.contains(&h.id))
            {
                let t = self.achievements.heroes.entry(h.id).or_default();
                if seconds / 60 > t.endless_minutes {
                    t.endless_minutes = seconds / 60;
                    facts.push((
                        h.id,
                        AchievementFacts {
                            endless_seconds: seconds,
                            ..Default::default()
                        },
                    ));
                }
            }
        }
        if self.run.won_at.is_some() && !self.achievements.win_counted {
            self.achievements.win_counted = true;
            facts.extend(self.win_facts());
        }
        let mut told = self.add_facts(facts, false);
        told.extend(self.tell_live_counters());
        told
    }

    /// Every hero's gold collected this run not yet in the save.
    fn gold_pending(&self) -> u64 {
        self.run
            .heroes
            .iter()
            .map(|h| {
                let banked = self.achievements.heroes.get(&h.id).map_or(0, |t| t.gold_banked);
                u64::from(h.gold_earned.saturating_sub(banked))
            })
            .sum()
    }

    /// The warband's achievements as the screens are told them in a run: the save's, the kills
    /// and gold collected not yet in it added to their counters, so the tracker moves live.
    fn live_achievements(&self) -> ToClient {
        let content = &self.run.content;
        let mut told = self.saves.achievements_told(content);
        let (kills, gold) = (self.achievements.kills_pending(), self.gold_pending());
        for p in &mut told.progress {
            let pending = match content.achievement(&p.id).map(|a| &a.criteria) {
                Some(Criteria::Counter { counter: WarbandCounter::Kills, .. }) => kills,
                Some(Criteria::Counter { counter: WarbandCounter::GoldCollected, .. }) => gold,
                _ => continue,
            };
            p.current = (p.current + pending).min(p.goal);
        }
        ToClient::Achievements(told)
    }

    /// The tracker's live counters: while a tracked achievement counts kills
    /// or gold collected, the list again once a second at most when either moved.
    fn tell_live_counters(&mut self) -> Vec<(u32, Packet)> {
        let pending = (self.achievements.kills_pending(), self.gold_pending());
        let (k, g, at) = self.achievements.live_told;
        if (k, g) == pending || self.run.now - at < LIVE_EVERY {
            return Vec::new();
        }
        let content = self.run.content.clone();
        let live = self.saves.achievements(&content).tracked.iter().any(|id| {
            matches!(
                content.achievement(id).map(|a| &a.criteria),
                Some(Criteria::Counter { counter: WarbandCounter::Kills | WarbandCounter::GoldCollected, .. })
            )
        });
        if !live {
            return Vec::new();
        }
        self.achievements.live_told = (pending.0, pending.1, self.run.now);
        let messages = self.for_everyone(&self.live_achievements());
        self.lines(messages)
    }

    /// What the won run brings each hero standing in it.
    fn win_facts(&self) -> Vec<(u32, AchievementFacts)> {
        let content = &self.run.content;
        let zone = &content.zones[self.run.zone].id;
        let mut out = Vec::new();
        for h in self
            .run
            .heroes
            .iter()
            .filter(|h| standing(h) && !self.left.contains(&h.id))
        {
            let class = &content.classes[h.class].id;
            let meter = self.meter_done(h.id);
            let met = content
                .achievements
                .iter()
                .filter(|a| matches!(&a.criteria, Criteria::Win(c) if self.achievements.win_holds(&self.run, h, &meter, c)))
                .map(|a| a.id.clone())
                .collect();
            out.push((
                h.id,
                AchievementFacts {
                    zone_cleared: Some(zone.clone()),
                    class_won: Some(class.clone()),
                    start_won: Some(h.start.clone()),
                    loadout_won: (!h.loadout.is_empty()).then(|| loadout_key(class, &h.loadout)),
                    met,
                    ..Default::default()
                },
            ));
        }
        out
    }

    /// Hero `hero`'s kills and gold collected not yet in the save go there (as the run pays it
    /// out).
    pub(super) fn bank_kills(&mut self, hero: u32) -> Vec<(u32, Packet)> {
        let kills = self.achievements.take_kills(hero);
        let earned = self
            .run
            .heroes
            .iter()
            .find(|h| h.id == hero)
            .map_or(0, |h| h.gold_earned);
        let t = self.achievements.heroes.entry(hero).or_default();
        let gold = earned.saturating_sub(t.gold_banked);
        t.gold_banked = t.gold_banked.max(earned);
        if kills == 0 && gold == 0 {
            return Vec::new();
        }
        self.add_facts(
            vec![(
                hero,
                AchievementFacts {
                    kills,
                    gold: u64::from(gold),
                    ..Default::default()
                },
            )],
            true,
        )
    }

    /// Adds each hero's facts to the save and tells what they earned: a line in every chat, the
    /// banner message to every client with screens, then the list (always with `tell_list`).
    pub(super) fn add_facts(
        &mut self,
        facts: Vec<(u32, AchievementFacts)>,
        tell_list: bool,
    ) -> Vec<(u32, Packet)> {
        let content = self.run.content.clone();
        let mut told = Vec::new();
        let mut messages = Vec::new();
        if !facts.is_empty() {
            // the counters moved: the next step is asked again
            self.achievements.kills_to_step = None;
            self.achievements.gold_to_step = None;
        }
        for (hero, mut f) in facts {
            f.bounties = self.achievements.run_bounties(&content, &f);
            for id in self.saves.add_achievement_facts(&content, &f) {
                // a bounty is named with how often it was earned
                let name = match content.bounty(&id) {
                    Some(b) => {
                        let times = self.saves.achievements(&content).bounties.get(&id).map_or(1, |r| r.times);
                        format!("{} ({})", b.name, nth(times))
                    }
                    None => content.achievement(&id).map_or(id.clone(), |a| a.name.clone()),
                };
                let name = name.as_str();
                let who = self
                    .saved(hero)
                    .map_or_else(String::new, |s| s.name.clone());
                let line = packets::system_message(&earned_line(&who, name));
                told.extend(self.players.iter().map(|p| (p.hero, line.clone())));
                messages.extend(self.for_everyone(&ToClient::AchievementEarned {
                    hero: self.hero_guid(hero),
                    id,
                }));
            }
        }
        if tell_list || !messages.is_empty() {
            messages.extend(self.for_everyone(&self.live_achievements()));
        }
        told.extend(self.lines(messages));
        told
    }

    /// The run's tracker starts over with a fresh run.
    pub(super) fn achievements_start_over(&mut self) {
        self.achievements = RunTracker::default();
    }
}

#[cfg(test)]
mod tests;
