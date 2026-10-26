//! The game's own screens (design step 6), through esurv-protocol's addon lines. Once a
//! hero's client says `hello` it is told who and where it is, its HUD, the boss bars and banners,
//! its choice screens, the gear it wins, the end of its run with what that paid out, its damage
//! meter, its class's progress and the run options; what it asks (an offer, a reroll, pause,
//! Endless, a fresh run after the end or a reset in the middle, leaving, run options, the armory)
//! is done through the run and the save. A client that never said `hello` sees the run as before.
//!
//! The run pays out when it ends (each end once: a victory,
//! then a fall in Endless), when a hero leaves it and when a hero's client logs out or drops; each
//! payment brings the talent experience earned since the last, and the zone a win opens comes
//! once per run.

use std::time::{Duration, Instant};

use esurv_content::Upgrade;
use esurv_protocol::{self as protocol, Ending, Screens, ToClient, ToServer, Wire};
use esurv_sim::game::amulets::empower_factor;
use esurv_sim::game::casting::{HitSource, ProcOrigin};
use esurv_sim::game::choices::{Offer, ScreenKind};
use esurv_sim::game::gear::{decode_item_entry, zone_piece_name};
use esurv_sim::game::offers::reroll_cost;
use esurv_sim::game::rewards::{talent_points, ExperienceParts};
use esurv_sim::game::run::{EnemyKind, Event, Phase, Unit};
use esurv_sim::game::spell_numbers::{
    head_start_gain, new_spell_numbers, spell_numbers, spell_totals, upgrade_gain,
};
use esurv_sim::game::spells::SpellSlot;
use esurv_sim::game::stats::compute;

use super::{whole, Game};
use crate::packets;
use crate::save::{FinishedRun, Payout, SavedRunOptions};
use crate::wire::Packet;

/// While only its numbers change, a HUD, a boss bar or a meter goes out at most this often.
const SCREENS_EVERY: Duration = Duration::from_millis(200);

/// A client that said `hello`: the screens it has, and what it was last told.
#[derive(Default)]
pub(super) struct Told {
    pub screens: Screens,
    hud: Option<(protocol::Hud, Instant)>,
    meter: Option<(protocol::Meter, Instant)>,
    /// The spellbook's tooltips as last told, and when.
    spell_tips: Option<(Vec<protocol::SpellTip>, Instant)>,
    /// The buffs as last told, and when.
    buffs: Option<(Vec<protocol::BuffTip>, Instant)>,
    /// It heard every ground indicator standing (`ground_indicators`).
    pub indicators: bool,
}

/// What the run has paid a hero so far.
#[derive(Default)]
pub(super) struct Paid {
    /// The talent experience paid, part by part.
    parts: ExperienceParts,
    /// The run's seconds paid for (its play time).
    seconds: f32,
    /// The run was counted as a run played (and won).
    counted: bool,
    /// A win tried to open the next zone (once per run).
    zone_tried: bool,
    /// The last payment: its parts and what it paid out.
    last: Option<(ExperienceParts, Payout)>,
}

/// A boss's bar as the clients were last told it.
pub(super) struct Bar {
    guid: u64,
    name: String,
    health: u32,
    max_health: u32,
    phases: Vec<protocol::BossPhase>,
    at: Instant,
}

/// Where boss `boss`'s script changes something by health, highest first: each `Health(..)`
/// line, labelled by its "soon" line without its " soon" ("Gnoll pack soon": "Gnoll pack"), else
/// its warning's bar label, else nothing. Two lines at one share make one mark.
pub(super) fn boss_phases(boss: &esurv_content::Boss) -> Vec<protocol::BossPhase> {
    let mut phases: Vec<protocol::BossPhase> = Vec::new();
    for line in &boss.script {
        let esurv_content::ScriptTrigger::Health(at) = line.on else { continue };
        let label = match (&line.soon, &line.warn) {
            (Some(soon), _) => soon.strip_suffix(" soon").unwrap_or(soon).to_string(),
            (None, Some(w)) => w.bar.clone().unwrap_or_default(),
            (None, None) => String::new(),
        };
        match phases.iter_mut().find(|p| p.at == at) {
            Some(p) if p.label.is_empty() => p.label = label,
            Some(_) => {}
            None => phases.push(protocol::BossPhase { at, label }),
        }
    }
    phases.sort_by(|a, b| b.at.total_cmp(&a.at));
    phases
}

/// One hero's damage by source, as its meter counts it.
#[derive(Default)]
pub(super) struct MeterTally {
    done: Vec<(protocol::Source, f64)>,
    taken: Vec<(protocol::Source, f64)>,
}

fn count(list: &mut Vec<(protocol::Source, f64)>, source: protocol::Source, amount: f32) {
    match list.iter_mut().find(|(s, _)| *s == source) {
        Some((_, total)) => *total += f64::from(amount),
        None => list.push((source, f64::from(amount))),
    }
}

/// What gave the hero a proc, as the meter's line tells it.
fn meter_origin(origin: &ProcOrigin) -> protocol::ProcOrigin {
    match origin {
        ProcOrigin::Talent(id) => protocol::ProcOrigin::Talent(id.clone()),
        ProcOrigin::Jewelry(id) => protocol::ProcOrigin::Jewelry(id.clone()),
        ProcOrigin::Amulet(id) => protocol::ProcOrigin::Amulet(id.clone()),
        ProcOrigin::Legendary(id) => protocol::ProcOrigin::Legendary(id.clone()),
        ProcOrigin::Loadout(id) => protocol::ProcOrigin::Loadout(id.clone()),
        ProcOrigin::Spell(id) => protocol::ProcOrigin::Spell(id.clone()),
        ProcOrigin::Other => protocol::ProcOrigin::Other,
    }
}

/// The meter's lines, biggest first.
fn meter_entries(list: &[(protocol::Source, f64)]) -> Vec<protocol::MeterEntry> {
    let mut out: Vec<protocol::MeterEntry> = list
        .iter()
        .map(|(source, total)| protocol::MeterEntry {
            source: source.clone(),
            amount: total.max(0.0).round() as u64,
        })
        .collect();
    out.sort_by_key(|e| std::cmp::Reverse(e.amount));
    out
}

/// Whether something told at `at` may be told again now.
fn due(at: Instant) -> bool {
    at.elapsed() >= SCREENS_EVERY
}

fn phase_shown(p: Phase) -> protocol::Phase {
    match p {
        Phase::Waiting => protocol::Phase::Waiting,
        Phase::Running => protocol::Phase::Running,
        Phase::Choosing => protocol::Phase::Choosing,
        Phase::Paused => protocol::Phase::Paused,
        Phase::Defeat => protocol::Phase::Defeat,
        Phase::Victory => protocol::Phase::Victory,
    }
}

/// `hero`'s buffs as its buff frame shows them, in the aura slots' order (`present::hero_auras`:
/// one slot per look, stacks counted): what each does, the spell of ours that cast it (the race's
/// spell or a learned one wearing that look; none for a rune or a milestone's), and its value now,
/// as the run's rules apply it: a damage or haste buff grown by the hero's buff power and the
/// rune's as it is (`Run::refresh_stats`), the shield's absorb left (`Hero::shield`), what an
/// aura strikes back with (`strike_back`: grown by buff power and the hero's damage), health a
/// second; a slot's buffs of one look add up.
pub(super) fn buff_tips(
    content: &esurv_content::Content,
    h: &esurv_sim::game::run::Hero,
    now: f32,
) -> Vec<protocol::BuffTip> {
    use esurv_sim::game::run::BuffKind as K;
    let buff_power = 1.0 + h.stats.buff_power;
    let casts = |look: u32| {
        h.spells
            .racial
            .iter()
            .chain(&h.spells.known)
            .filter_map(|s| content.ability(&s.spell))
            .find(|a| a.look_spell == look)
            .map(|a| a.id.clone())
            .unwrap_or_default()
    };
    super::present::hero_auras(h, now)
        .into_iter()
        .filter_map(|aura| {
            let mine: Vec<_> = h
                .buffs
                .iter()
                .filter(|b| b.visual == aura.spell && b.until > now)
                .collect();
            let first = mine.first()?;
            let kind = match first.kind {
                K::Shield => protocol::BuffKind::Shield,
                K::Damage => protocol::BuffKind::Damage,
                K::Haste => protocol::BuffKind::Haste,
                K::Immunity => protocol::BuffKind::Immunity,
                K::Aura => protocol::BuffKind::Aura,
                K::Regen => protocol::BuffKind::Regen,
                K::Speed => protocol::BuffKind::Speed,
                K::Berserk => protocol::BuffKind::Berserk,
                K::HealingCharges => protocol::BuffKind::HealingCharges,
                K::ExtraStrikes => protocol::BuffKind::ExtraStrikes,
                K::SwingSpeed => protocol::BuffKind::Haste,
            };
            let value = match first.kind {
                K::Shield => h.shield,
                K::Immunity => 0.0,
                _ => mine
                    .iter()
                    .map(|b| match b.kind {
                        K::Damage | K::Haste => b.value * buff_power,
                        K::Aura => b.value * buff_power * h.stats.damage,
                        K::HealingCharges => b.value * h.stats.max_health,
                        // Slice and Dice's swing speed rides along its damage buff: not part of its number
                        K::SwingSpeed => 0.0,
                        _ => b.value,
                    })
                    .sum(),
            };
            let spell = match first.kind {
                K::Speed | K::Berserk => String::new(),
                _ => casts(aura.spell),
            };
            Some(protocol::BuffTip {
                look: aura.spell,
                kind,
                spell,
                value,
                stacks: aura.stacks,
            })
        })
        .collect()
}

fn upgrade_word(u: Upgrade) -> &'static str {
    u.word()
}

fn value(what: &str, value: f32) -> protocol::OfferValue {
    protocol::OfferValue {
        what: what.into(),
        value,
    }
}

/// Each part of `now` past what `paid` holds.
fn parts_since(now: ExperienceParts, paid: ExperienceParts) -> ExperienceParts {
    ExperienceParts {
        time: now.time.saturating_sub(paid.time),
        kills: now.kills.saturating_sub(paid.kills),
        bosses: now.bosses.saturating_sub(paid.bosses),
        victory: now.victory.saturating_sub(paid.victory),
        zone: now.zone.saturating_sub(paid.zone),
        options: now.options.saturating_sub(paid.options),
    }
}

/// Each part at the larger of the two.
fn parts_max(a: ExperienceParts, b: ExperienceParts) -> ExperienceParts {
    ExperienceParts {
        time: a.time.max(b.time),
        kills: a.kills.max(b.kills),
        bosses: a.bosses.max(b.bosses),
        victory: a.victory.max(b.victory),
        zone: a.zone.max(b.zone),
        options: a.options.max(b.options),
    }
}

impl Game {
    /// `messages` as addon lines, each to its hero's own client, sent as from that hero.
    pub(super) fn lines(&self, messages: Vec<(u32, ToClient)>) -> Vec<(u32, Packet)> {
        let mut out = Vec::new();
        for (hero, m) in messages {
            let Some(p) = self.players.iter().find(|p| p.hero == hero) else {
                continue;
            };
            let guid = p.guid();
            out.extend(
                m.lines()
                    .into_iter()
                    .map(|l| (hero, packets::addon_message(guid, &l.prefix, &l.message))),
            );
        }
        out
    }

    /// `message` for every client that said `hello`.
    fn for_everyone(&self, message: &ToClient) -> Vec<(u32, ToClient)> {
        self.told
            .keys()
            .map(|&hero| (hero, message.clone()))
            .collect()
    }

    fn hero_guid(&self, hero: u32) -> u64 {
        self.guid_of(Unit::Hero(hero)).unwrap_or(0)
    }

    /// The saved hero `hero` plays.
    fn saved(&self, hero: u32) -> Option<&crate::save::SavedHero> {
        self.players
            .iter()
            .find(|p| p.hero == hero)
            .map(|p| &p.saved)
    }

    /// A HUD message: a toast for a client with a HUD of its own, else a line in the chat.
    pub(super) fn announce(&self, text: &str) -> Vec<(u32, Packet)> {
        let mut out = Vec::new();
        for p in &self.players {
            if self.told.get(&p.hero).is_some_and(|t| t.screens.hud) {
                out.extend(self.lines(vec![(p.hero, ToClient::Toast(text.into()))]));
            } else {
                out.push((p.hero, packets::system_message(text)));
            }
        }
        out
    }

    /// Whether `hero`'s client answers its own choice screens.
    pub(super) fn chooses_itself(&self, hero: u32) -> bool {
        self.players.iter().any(|p| p.hero == hero)
            && self.told.get(&hero).is_some_and(|t| t.screens.cards)
    }

    /// Pays `hero` what the run earned since it last paid (nothing for a run that never began):
    /// its records, its class's talent experience and, once on a win, the next zone for its class.
    /// The chat lines that tell it, for its client.
    pub(super) fn pay_out(&mut self, hero: u32) -> Vec<(u32, Packet)> {
        if self.run.now <= 0.0 {
            return Vec::new();
        }
        let content = self.run.content.clone();
        let Some(saved) = self.saved(hero).cloned() else {
            return Vec::new();
        };
        let Some(i) = self.run.heroes.iter().position(|h| h.id == hero) else {
            return Vec::new();
        };
        let summary = self.run.summary(i);
        let paid = self.paid.entry(hero).or_default();
        let parts = parts_since(summary.experience_parts, paid.parts);
        let run = FinishedRun {
            level: summary.level,
            seconds: summary.seconds,
            won: summary.won,
            experience: parts.total(),
            played: (summary.seconds - paid.seconds).max(0.0),
            first: !paid.counted,
            opens_zone: summary.won && !paid.zone_tried,
            zone_index: self.run.zone,
            loot: summary.loot.iter().map(|g| g.entry).collect(),
            challenge: self.run.options.challenge.is_some(),
        };
        let Some(payout) = self.saves.finish_run(&content, saved.guid, &run) else {
            return Vec::new();
        };
        paid.parts = parts_max(summary.experience_parts, paid.parts);
        paid.seconds = paid.seconds.max(summary.seconds);
        paid.counted = true;
        paid.zone_tried |= summary.won;
        paid.last = Some((parts, payout));
        let classes = content
            .classes
            .iter()
            .find(|c| c.id == saved.class)
            .map_or_else(String::new, |c| format!("{}s", c.name));
        let mut told = Vec::new();
        if payout.experience > 0 {
            let earned = esurv_sim::game::lines::talent_experience_earned(
                &classes,
                payout.experience,
                payout.points_after - payout.points_before,
            );
            told.push((hero, packets::system_message(&earned)));
        }
        if let Some(zone) = payout.zone_opened.and_then(|z| content.zones.get(z)) {
            let opened = esurv_sim::game::lines::zone_opened(&zone.name, &classes);
            told.push((hero, packets::system_message(&opened)));
        }
        told
    }

    /// After a tick: when the run has just ended (each end once), every hero still in it is paid
    /// out; then the screens are told what they follow, the end of the run last. Lines for one
    /// hero's client each.
    pub(super) fn settle(&mut self, events: &[Event]) -> Vec<(u32, Packet)> {
        let mut told = Vec::new();
        let mut ends = Vec::new();
        if self.run.is_over() && self.ended != Some(self.run.phase) {
            self.ended = Some(self.run.phase);
            let ending = if self.run.phase == Phase::Victory {
                Ending::Victory
            } else {
                Ending::Fell
            };
            let heroes: Vec<u32> = self
                .players
                .iter()
                .map(|p| p.hero)
                .filter(|h| !self.left.contains(h))
                .collect();
            for hero in heroes {
                told.extend(self.pay_out(hero));
                ends.push((hero, ending));
            }
        }
        let mut messages = self.tell_events(events);
        messages.extend(self.boss_bars());
        messages.extend(self.enemy_ranks());
        messages.extend(self.huds());
        messages.extend(self.meters());
        messages.extend(self.tell_spell_tips());
        messages.extend(self.tell_buffs());
        for (hero, ending) in ends {
            if self.told.contains_key(&hero) {
                messages.extend(self.run_end(hero, ending).map(|e| (hero, e)));
            }
        }
        told.extend(self.lines(messages));
        told
    }

    /// A request from `hero`'s client; what it is told at once.
    pub(super) fn ask(&mut self, hero: u32, request: ToServer) -> Vec<(u32, Packet)> {
        let mut chat = Vec::new();
        let mut told = Vec::new();
        let done = match &request {
            ToServer::Hello { screens, .. } => {
                told = self.hello(hero, *screens);
                true
            }
            ToServer::Choose(n) => {
                self.run.phase == Phase::Choosing && self.run.choose(hero, *n as usize)
            }
            ToServer::Reroll => self.run.phase == Phase::Choosing && self.run.reroll(hero),
            ToServer::Pause => self.run.pause(),
            ToServer::Resume => self.run.resume(),
            ToServer::Endless => self.run.continue_endless(),
            ToServer::Again => {
                let fresh = self.run_again(hero);
                let done = fresh.is_some();
                chat.extend(fresh.into_iter().flatten());
                done
            }
            ToServer::Reset => {
                // what the run paid goes in the chat; no end panel, the fresh run is up at once
                let fresh = self.reset_run(hero);
                let done = fresh.is_some();
                chat.extend(fresh.into_iter().flatten());
                done
            }
            ToServer::Leave => {
                // the hero is out and paid out at once: its client leaves the world next
                let left = self.left.insert(hero);
                if left {
                    self.run.leave(hero);
                    chat = self.pay_out(hero);
                    if self.told.contains_key(&hero) {
                        told.extend(self.run_end(hero, Ending::Left).map(|e| (hero, e)));
                    }
                }
                left
            }
            ToServer::RunOptions(o) => {
                let asked = SavedRunOptions::from(*o);
                let kept = self.saves.set_run_options(&self.run.content, asked);
                told.push((hero, self.run_options(kept)));
                true
            }
            ToServer::LearnTalent(_)
            | ToServer::UnlearnTalent(_)
            | ToServer::LearnPrestige(_)
            | ToServer::UnlearnPrestige(_)
            | ToServer::ResetPrestige
            | ToServer::Wear(_)
            | ToServer::TakeOff(_)
            | ToServer::SetLook { .. }
            | ToServer::ChooseLoadout(_) => self.in_the_armory(hero, &mut told, &request),
        };
        if !done {
            told.push((hero, ToClient::Refused(request.verb().into())));
        }
        chat.extend(self.lines(told));
        chat
    }

    /// The screens' side of a fresh run: the boss bars go, what the old run paid, dropped and
    /// metered is forgotten, and every told client still in the run hears who and where it is
    /// again (its screens start over, its HUD and meter are told anew) and its class's progress.
    pub(super) fn screens_start_over(&mut self) -> Vec<(u32, Packet)> {
        let mut told = Vec::new();
        let bars: Vec<Bar> = self.bars.drain().map(|(_, bar)| bar).collect();
        for bar in bars {
            let gone = ToClient::BossBar(protocol::BossBar {
                boss: bar.guid,
                name: bar.name,
                health: 0,
                max_health: bar.max_health,
                phases: bar.phases,
            });
            told.extend(self.for_everyone(&gone));
        }
        // the clients forget the told ranks with the `Ready` below
        self.ranks_told.clear();
        self.paid.clear();
        self.ended = None;
        self.drops.clear();
        self.meters.clear();
        let heroes: Vec<u32> = self
            .told
            .keys()
            .copied()
            .filter(|h| !self.left.contains(h))
            .collect();
        for hero in heroes {
            if let Some(t) = self.told.get_mut(&hero) {
                t.hud = None;
                t.meter = None;
            }
            told.extend(self.ready(hero).map(|r| (hero, r)));
            told.extend(self.progress(hero).map(|p| (hero, p)));
        }
        self.lines(told)
    }

    /// A change in the armory for `hero`'s class (it counts from the next run), by the same rules
    /// as before a run ([`crate::prerun::armory_change`]); its progress is told when it changed.
    fn in_the_armory(&self, hero: u32, told: &mut Vec<(u32, ToClient)>, change: &ToServer) -> bool {
        let Some(class) = self.saved(hero).map(|s| s.class.clone()) else {
            return false;
        };
        let done = crate::prerun::armory_change(&self.run.content, &self.saves, &class, change)
            .unwrap_or(false);
        if done {
            told.extend(self.progress(hero).map(|p| (hero, p)));
        }
        done
    }

    /// A client says it speaks the protocol: it is told who and where it is, its class's
    /// progress, the run options, the codex, the boss bars up and the choice it has open; its HUD
    /// follows with the next tick.
    fn hello(&mut self, hero: u32, screens: Screens) -> Vec<(u32, ToClient)> {
        self.told.insert(
            hero,
            Told {
                screens,
                ..Default::default()
            },
        );
        let mut told: Vec<(u32, ToClient)> = Vec::new();
        told.extend(self.ready(hero).map(|r| (hero, r)));
        told.extend(self.progress(hero).map(|p| (hero, p)));
        told.push((hero, self.run_options(self.saves.run_options())));
        told.push((hero, self.codex()));
        told.extend(self.option_hello(hero).into_iter().map(|m| (hero, m)));
        for bar in self.bars.values() {
            told.push((
                hero,
                ToClient::BossBar(protocol::BossBar {
                    boss: bar.guid,
                    name: bar.name.clone(),
                    health: bar.health,
                    max_health: bar.max_health,
                    phases: bar.phases.clone(),
                }),
            ));
        }
        let ranks = self.ranked_player_enemies();
        if !ranks.is_empty() {
            told.push((hero, ToClient::EnemyRanks(ranks.into_iter().map(|(_, unit, rank)| (unit, rank)).collect())));
        }
        if let Some(s) = self.run.screen.clone() {
            if self.run.heroes[s.hero].id == hero {
                told.push((hero, self.choices(hero, s.kind, &s.offers)));
            }
        }
        told
    }

    /// Who `hero` is and where its run is.
    fn ready(&self, hero: u32) -> Option<ToClient> {
        let saved = self.saved(hero)?;
        let content = &self.run.content;
        let zone = &content.zones[self.run.zone];
        let class = content.classes.iter().find(|c| c.id == saved.class)?;
        let race = content.races.iter().find(|r| r.id == saved.race)?;
        Some(ToClient::Ready(protocol::Ready {
            hero: self.hero_guid(hero),
            name: saved.name.clone(),
            class: class.name.clone(),
            race: race.name.clone(),
            zone: zone.name.clone(),
            subtitle: zone.subtitle.clone(),
        }))
    }

    fn run_options(&self, o: SavedRunOptions) -> ToClient {
        crate::prerun::run_options(&self.run.content, o)
    }

    /// The codex as the save keeps it: every ring, trinket and amulet taken in play by any hero.
    fn codex(&self) -> ToClient {
        crate::prerun::codex(&self.saves)
    }

    /// What `hero`'s class has earned, for the armory.
    fn progress(&self, hero: u32) -> Option<ToClient> {
        let saved = self.saved(hero)?;
        Some(crate::prerun::progress(
            &self.run.content,
            &self.saves,
            &saved.class,
        ))
    }

    /// A choice screen as its hero's client is told it: each offer with what it would bring.
    fn choices(&self, hero: u32, kind: ScreenKind, offers: &[Offer]) -> ToClient {
        let h = self.run.heroes.iter().find(|h| h.id == hero);
        let (level, reroll, free) = h.map_or((0, 0, 0), |h| {
            (
                h.progress.level,
                reroll_cost(h.rerolls_free, h.rerolls_paid),
                h.rerolls_free,
            )
        });
        ToClient::Choices(protocol::Choices {
            hero: self.hero_guid(hero),
            kind: match kind {
                ScreenKind::LevelUp => protocol::ScreenKind::LevelUp,
                ScreenKind::Cache => protocol::ScreenKind::Cache,
                ScreenKind::RareCache => protocol::ScreenKind::RareCache,
            },
            level,
            reroll_cost: reroll,
            free_rerolls: free,
            offers: offers.iter().map(|o| self.offer(hero, o)).collect(),
            kit: self.kit(hero),
        })
    }

    /// What `hero` has as a choice opens: its spells with their ranks, its rings and trinkets
    /// with their ranks and values, its amulet with its rank and empower factor.
    fn kit(&self, hero: u32) -> protocol::Kit {
        let Some(h) = self.run.heroes.iter().find(|h| h.id == hero) else {
            return protocol::Kit::default();
        };
        protocol::Kit {
            spells: h
                .spells
                .known
                .iter()
                .map(|s| protocol::KnownSpell {
                    id: s.spell.clone(),
                    rank: s.upgrades.rank,
                })
                .collect(),
            jewelry: h
                .jewelry
                .iter()
                .map(|w| protocol::Worn {
                    id: w.id.clone(),
                    name: String::new(),
                    rank: w.rank,
                    value: w.value,
                })
                .collect(),
            amulet: h.amulet.as_ref().map(|a| protocol::Worn {
                id: a.amulet.id.clone(),
                name: a.amulet.name.clone(),
                rank: a.rank,
                value: empower_factor(a.amulet.growth, a.rank),
            }),
        }
    }

    /// One offer with what it brings: a spell card's head start (as the gain it gives) and the
    /// spell's numbers for this hero, an upgrade's gains and the rank it reaches, jewelry's value
    /// (and how many of its slot are worn), an amulet's name, the rank and factor an empowered
    /// amulet reaches, gold.
    fn offer(&self, hero: u32, offer: &Offer) -> protocol::Offer {
        use protocol::OfferKind as K;
        let content = &self.run.content;
        let h = self.run.heroes.iter().find(|h| h.id == hero);
        let card = |kind, id: &str, rarity, values| protocol::Offer {
            kind,
            id: id.into(),
            name: String::new(),
            rarity,
            values,
            numbers: Vec::new(),
        };
        match offer {
            Offer::NewSpell { spell, rarity } => {
                let ability = content.ability(spell);
                let values = ability
                    .and_then(|a| head_start_gain(a, *rarity))
                    .map(|(k, by)| vec![value(upgrade_word(k), by)])
                    .unwrap_or_default();
                // the spell as this hero would cast it: its talents, amulet and stats count
                let numbers = ability.zip(h).map_or_else(Vec::new, |(a, h)| {
                    new_spell_numbers(a, *rarity, &h.spell_modifiers, h.spells.damage_scale, &h.stats)
                        .into_iter()
                        .map(|(what, v)| value(what, v))
                        .collect()
                });
                protocol::Offer {
                    numbers,
                    ..card(K::NewSpell, spell, *rarity as u8, values)
                }
            }
            Offer::UpgradeSpell {
                spell,
                kinds,
                rarity,
            } => {
                // the rank goes up once for each kind the card steps
                let slot = h.and_then(|h| h.spells.known.iter().find(|s| &s.spell == spell));
                let mut values = vec![value(
                    "rank",
                    slot.map_or(0.0, |s| (s.upgrades.rank as usize + kinds.len()) as f32),
                )];
                // each kind's gain on what the spell has now (a cooldown 13% shorter ...)
                let ability = content.ability(spell);
                values.extend(kinds.iter().map(|&k| {
                    let by = slot
                        .zip(ability)
                        .map_or(0.0, |(s, a)| upgrade_gain(a, &s.upgrades, k, *rarity));
                    value(upgrade_word(k), by)
                }));
                card(K::UpgradeSpell, spell, *rarity as u8, values)
            }
            Offer::NewJewelry {
                id,
                value: v,
                rarity,
            } => {
                let slot = content.jewelry.iter().find(|j| &j.id == id).map(|j| j.slot);
                let worn = h.map_or(0, |h| {
                    h.jewelry
                        .iter()
                        .filter(|w| {
                            content
                                .jewelry
                                .iter()
                                .any(|j| j.id == w.id && Some(j.slot) == slot)
                        })
                        .count()
                });
                card(
                    K::NewJewelry,
                    id,
                    *rarity as u8,
                    vec![value("value", *v), value("worn", worn as f32)],
                )
            }
            Offer::UpgradeJewelry {
                id,
                step: s,
                rarity,
            } => {
                let worn = h.and_then(|h| h.jewelry.iter().find(|w| &w.id == id));
                card(
                    K::UpgradeJewelry,
                    id,
                    *rarity as u8,
                    vec![
                        value("rank", worn.map_or(0.0, |w| (w.rank + 1) as f32)),
                        value("step", *s),
                        value("value", worn.map_or(0.0, |w| w.value)),
                    ],
                )
            }
            Offer::Amulet { amulet } => protocol::Offer {
                name: amulet.name.clone(),
                ..card(K::Amulet, &amulet.id, amulet.rarity.index(), vec![])
            },
            Offer::Empower => match h.and_then(|h| h.amulet.as_ref()) {
                Some(worn) => {
                    let rank = worn.rank + 1;
                    protocol::Offer {
                        name: worn.amulet.name.clone(),
                        ..card(
                            K::Empower,
                            &worn.amulet.id,
                            worn.amulet.rarity.index(),
                            vec![
                                value("rank", rank as f32),
                                value("factor", empower_factor(worn.amulet.growth, rank)),
                            ],
                        )
                    }
                }
                None => card(K::Empower, "", 0, vec![]),
            },
            Offer::Gold(g) => card(K::Gold, "", 0, vec![value("gold", *g as f32)]),
        }
    }

    /// The lines this tick's events bring: choice screens, drops, banners, the codex when it grew;
    /// the meter counts. A ring, trinket or amulet taken joins the codex here
    /// (the codex keeps what was taken, for every hero of the save).
    fn tell_events(&mut self, events: &[Event]) -> Vec<(u32, ToClient)> {
        use protocol::Banner;
        let mut told = Vec::new();
        let mut codex_grew = false;
        for e in events {
            self.count_for_the_meter(e);
            if let Event::OfferTaken { offer, .. } = e {
                // a Challenge Mode run adds nothing to the codex
                if let Some(id) = offer.codex_entry().filter(|_| self.run.options.challenge.is_none()) {
                    codex_grew |= self.saves.add_to_codex(id);
                }
            }
            match e {
                Event::ResourceChanged { hero, kind, value, max } if self.told.contains_key(hero) => {
                    told.extend(self.bar_message(*hero, *kind, *value, *max).map(|m| (*hero, m)));
                }
                Event::ScreenOpened { hero, kind, offers } if self.told.contains_key(hero) => {
                    told.push((*hero, self.choices(*hero, *kind, offers)));
                }
                Event::OfferTaken { hero, .. } if self.told.contains_key(hero) => {
                    let hero_guid = self.hero_guid(*hero);
                    told.push((*hero, ToClient::ChoicesClosed { hero: hero_guid }));
                }
                Event::GearDropped {
                    hero,
                    slot,
                    tier,
                    entry,
                } => {
                    if let Some((name, new)) = self.drops.get(entry).cloned() {
                        if self.told.contains_key(hero) {
                            told.push((
                                *hero,
                                ToClient::GearDropped(protocol::GearDrop {
                                    hero: self.hero_guid(*hero),
                                    entry: *entry,
                                    slot: *slot,
                                    tier: *tier,
                                    name,
                                    new,
                                }),
                            ));
                        }
                    }
                }
                Event::BossSpawned { boss, .. } => {
                    let b = &self.run.content.zones[self.run.zone].bosses[*boss];
                    let banner = Banner::BossComes {
                        name: b.name.clone(),
                        final_boss: b.is_final,
                    };
                    told.extend(self.for_everyone(&ToClient::Banner(banner)));
                }
                // a rare falls silently, as an elite does
                Event::EnemyDied { id, .. } => {
                    if let Some(bar) = self.bars.get(id) {
                        let banner = Banner::BossDown {
                            name: bar.name.clone(),
                        };
                        told.extend(self.for_everyone(&ToClient::Banner(banner)));
                    }
                }
                _ => {}
            }
        }
        if codex_grew {
            told.extend(self.for_everyone(&self.codex()));
        }
        told
    }

    /// A gear piece won joins the collection at once, and its hero's chat says so; the drop is
    /// kept (name, new) for the screens.
    pub(super) fn hand_out_loot(&mut self, events: &[Event]) -> Vec<(u32, Packet)> {
        let mut told = Vec::new();
        for e in events {
            let Event::GearDropped {
                hero,
                slot,
                tier: _,
                entry,
            } = e
            else {
                continue;
            };
            let content = &self.run.content;
            let class = self
                .run
                .heroes
                .iter()
                .find(|h| h.id == *hero)
                .map(|h| content.classes[h.class].id.clone());
            let name = class
                .and_then(|c| content.gear_set(&c))
                .and_then(|set| zone_piece_name(content, set, *slot, decode_item_entry(*entry)?.2))
                .unwrap_or_default();
            let new = self.saves.add_to_collection(content, *entry);
            told.push((
                *hero,
                packets::system_message(&esurv_sim::game::lines::gear_won(&name, new)),
            ));
            self.drops.insert(*entry, (name, new));
        }
        told
    }

    /// The meter counts what each hero (its pets and procs too) did and took.
    fn count_for_the_meter(&mut self, e: &Event) {
        use protocol::Source;
        match e {
            Event::EnemyHit {
                amount,
                by: Some(hero),
                source,
                ..
            } => {
                let index = self.run.heroes.iter().position(|h| h.id == *hero);
                let source = match source {
                    // an effect over time a milestone put on (`aura:<look>`): that milestone's proc
                    HitSource::Tick(a) => match index.and_then(|i| self.run.aura_origin(i, a)) {
                        Some((spell, origin)) => Source::Proc {
                            spell,
                            origin: meter_origin(&origin),
                        },
                        None => Source::Spell(a.clone()),
                    },
                    HitSource::Spell(a) => Source::Spell(a.clone()),
                    HitSource::WeaponSwing => Source::Swing,
                    // a pet an ally summoned counts as that ally's
                    HitSource::Pet(p) if self.run.pets.iter().any(|x| x.id == *p && x.assist.is_some()) => {
                        let character = self.run.pets.iter().find(|x| x.id == *p).and_then(|x| x.assist.clone()).unwrap_or_default();
                        Source::Assist(self.character_name(&character))
                    }
                    HitSource::Pet(p) => {
                        let name = match self.run.pets.iter().find(|x| x.id == *p) {
                            Some(x) => x.pet.clone(),
                            None => self
                                .shown
                                .get(p)
                                .map_or_else(String::new, |s| s.name.clone()),
                        };
                        Source::Pet(name)
                    }
                    HitSource::Proc { spell, origin, .. } => Source::Proc {
                        spell: *spell,
                        origin: meter_origin(origin),
                    },
                    // an enemy turned on the others (Dominate Mind)
                    // a battle standard's ally: its own row, by its character's name
                    HitSource::Assist(character) => Source::Assist(self.character_name(character)),
                    HitSource::Enemy(_) | HitSource::Other => Source::Other,
                };
                count(
                    &mut self.meters.entry(*hero).or_default().done,
                    source,
                    *amount,
                );
            }
            Event::HeroHit { hero, lost, by, .. } => {
                let source = match by.and_then(|id| self.shown.get(&id)) {
                    Some(s) => Source::Enemy(s.name.clone()),
                    None => Source::Area,
                };
                count(
                    &mut self.meters.entry(*hero).or_default().taken,
                    source,
                    *lost,
                );
            }
            _ => {}
        }
    }

    /// Each told hero's HUD, when it changed: at once for a new phase, else at most every 0.2 s.
    fn huds(&mut self) -> Vec<(u32, ToClient)> {
        let now = self.run.now;
        let zone = &self.run.content.zones[self.run.zone];
        let next_boss = zone
            .bosses
            .get(self.run.next_boss)
            .map(|b| (b.spawn_minute * 60.0 - now).ceil())
            .filter(|&s| s > 0.0)
            .map(|s| s as u32);
        let (phase, endless) = (
            phase_shown(self.run.phase),
            self.run.endless_since.is_some(),
        );
        let cache_price = self.run.field.cache_price;
        let mut told = Vec::new();
        for h in &self.run.heroes {
            let hero = self.guid_of(Unit::Hero(h.id));
            let (Some(t), Some(hero)) = (self.told.get_mut(&h.id), hero) else {
                continue;
            };
            let hud = protocol::Hud {
                hero,
                phase,
                endless,
                clock: now.max(0.0) as u32,
                kills: h.kills,
                gold: h.gold,
                cache_price,
                next_boss,
            };
            let tell = match &t.hud {
                None => true,
                Some((last, at)) => {
                    *last != hud
                        && (last.phase != hud.phase || last.endless != hud.endless || due(*at))
                }
            };
            if tell {
                t.hud = Some((hud.clone(), Instant::now()));
                told.push((h.id, ToClient::Hud(hud)));
            }
        }
        told
    }

    /// `hero`'s spellbook as its tooltips show it, in the spellbook's order (`spellbook`): the
    /// race's spell, the race's passives under their stock racial spells (none without the game
    /// data), then the spells learned; each spell with its rank and its numbers for the hero now,
    /// worked out by the casting rules as a card's are (its talents, amulet, upgrades and stats
    /// count), and its totals (`spell_totals`): what its upgrade cards add up to, what the hero's
    /// talents, gear, jewelry, amulet and race add (its modifiers against the bare stat sheet of
    /// its class and level), and its crit when its hits can crit.
    pub(super) fn spell_tips(&self, hero: u32) -> Vec<protocol::SpellTip> {
        use protocol::{SpellTip, TipKind};
        let Some(h) = self.run.heroes.iter().find(|h| h.id == hero) else {
            return Vec::new();
        };
        let content = &self.run.content;
        let class = &content.classes[h.class];
        let plain = compute(class, h.progress.level, &[]);
        let bonused = compute(class, h.progress.level, &h.modifiers);
        let by_kind = |list: Vec<(Upgrade, f32)>| -> Vec<protocol::OfferValue> {
            list.into_iter()
                .map(|(k, v)| value(upgrade_word(k), v))
                .collect()
        };
        let spell = |slot: &SpellSlot, kind: TipKind, rank: u32| {
            let a = content.ability(&slot.spell)?;
            let totals = spell_totals(a, slot, &plain, &bonused, &h.stats);
            Some(SpellTip {
                look: a.look_spell,
                kind,
                id: a.id.clone(),
                rank,
                numbers: spell_numbers(a, &slot.instance(), &h.stats)
                    .into_iter()
                    .map(|(what, v)| value(what, v))
                    .collect(),
                upgrades: by_kind(totals.upgrades),
                bonuses: by_kind(totals.bonuses),
                crit: totals.crit,
            })
        };
        let mut tips: Vec<SpellTip> = Vec::new();
        tips.extend(
            h.spells
                .racial
                .as_ref()
                .and_then(|s| spell(s, TipKind::Racial, 0)),
        );
        if let Some(race) = content.races.get(h.race) {
            for (i, p) in race.passives.iter().enumerate() {
                let look = self
                    .data
                    .as_ref()
                    .and_then(|d| d.racial_spell(p.stock_spell_name()));
                if let Some(look) = look {
                    tips.push(SpellTip {
                        look,
                        kind: TipKind::Passive(i as u32),
                        id: race.id.clone(),
                        rank: 0,
                        numbers: Vec::new(),
                        upgrades: Vec::new(),
                        bonuses: Vec::new(),
                        crit: None,
                    });
                }
            }
        }
        for s in &h.spells.known {
            tips.extend(spell(s, TipKind::Learned, s.upgrades.rank));
        }
        tips
    }

    /// Each told hero's spellbook tooltips, when they changed, at most every 0.2 s (a buff that
    /// comes and goes changes the numbers as the HUD's change).
    fn tell_spell_tips(&mut self) -> Vec<(u32, ToClient)> {
        let heroes: Vec<u32> = self.told.keys().copied().collect();
        let mut told = Vec::new();
        for hero in heroes {
            let spells = self.spell_tips(hero);
            let guid = self.hero_guid(hero);
            let Some(t) = self.told.get_mut(&hero) else {
                continue;
            };
            let tell = match &t.spell_tips {
                None => true,
                Some((last, at)) => *last != spells && due(*at),
            };
            if guid == 0 || !tell {
                continue;
            }
            t.spell_tips = Some((spells.clone(), Instant::now()));
            told.push((hero, ToClient::SpellTips { hero: guid, spells }));
        }
        told
    }

    /// Each told hero's buffs, when they changed (one started or ended, a value moved), at most
    /// every 0.2 s.
    fn tell_buffs(&mut self) -> Vec<(u32, ToClient)> {
        let heroes: Vec<u32> = self.told.keys().copied().collect();
        let mut told = Vec::new();
        for hero in heroes {
            let Some(h) = self.run.heroes.iter().find(|h| h.id == hero) else {
                continue;
            };
            let mut buffs = buff_tips(&self.run.content, h, self.run.now);
            buffs.extend(self.stack_tips(h));
            buffs.extend(self.counter_tips(hero));
            let guid = self.hero_guid(hero);
            let Some(t) = self.told.get_mut(&hero) else {
                continue;
            };
            let tell = match &t.buffs {
                None => true,
                Some((last, at)) => *last != buffs && due(*at),
            };
            if guid == 0 || !tell {
                continue;
            }
            t.buffs = Some((buffs.clone(), Instant::now()));
            told.push((hero, ToClient::Buffs { hero: guid, buffs }));
        }
        told
    }

    /// Every player-looking elite, rare and boss in sight: its simulation id, guid and rank.
    fn ranked_player_enemies(&self) -> Vec<(u32, u64, u32)> {
        (0..self.run.enemies.len())
            .filter_map(|i| {
                let id = self.run.enemies[i].id;
                let guid = self.shown.get(&id).map(|s| s.guid).filter(|&g| super::player_enemies::is_player_enemy_guid(g))?;
                Some((id, guid, self.enemy_rank(i)?))
            })
            .collect()
    }

    /// The ranks of the player-looking elites, rares and bosses as they appear, to every told
    /// client (a player guid has no creature query to tell its client the portrait's dragon or
    /// the glow); those gone are forgotten.
    fn enemy_ranks(&mut self) -> Vec<(u32, ToClient)> {
        let ranked = self.ranked_player_enemies();
        self.ranks_told.retain(|id| ranked.iter().any(|&(x, ..)| x == *id));
        let new: Vec<(u64, u32)> = ranked
            .into_iter()
            .filter(|&(id, ..)| self.ranks_told.insert(id))
            .map(|(_, unit, rank)| (unit, rank))
            .collect();
        if new.is_empty() {
            return Vec::new();
        }
        self.for_everyone(&ToClient::EnemyRanks(new))
    }

    /// The bosses' health bars, to every told client: when a boss's health changes (at most every
    /// 0.2 s), and at 0 once it is gone.
    fn boss_bars(&mut self) -> Vec<(u32, ToClient)> {
        // a new bar's name as the boss shows it (a Warsong boss is played by a player character,
        // its name its own)
        let names: Vec<(u32, String)> = (0..self.run.enemies.len())
            .filter(|&i| self.run.enemies[i].is_boss() && !self.bars.contains_key(&self.run.enemies[i].id))
            .map(|i| (self.run.enemies[i].id, self.enemy_name(i)))
            .collect();
        let zone = &self.run.content.zones[self.run.zone];
        let mut changed = Vec::new();
        let mut alive = Vec::new();
        for e in &self.run.enemies {
            let EnemyKind::Boss(b) = e.kind else { continue };
            alive.push(e.id);
            let (health, max_health) = (whole(e.health), whole(e.max_health));
            match self.bars.get_mut(&e.id) {
                Some(bar) if bar.health != health && due(bar.at) => {
                    bar.health = health;
                    bar.max_health = max_health;
                    bar.at = Instant::now();
                    changed.push(e.id);
                }
                Some(_) => {}
                None => {
                    let Some(guid) = self.shown.get(&e.id).map(|s| s.guid) else {
                        continue;
                    };
                    self.bars.insert(
                        e.id,
                        Bar {
                            guid,
                            name: names.iter().find(|(id, _)| *id == e.id).map_or_else(|| zone.bosses[b].name.clone(), |(_, n)| n.clone()),
                            health,
                            max_health,
                            phases: boss_phases(&zone.bosses[b]),
                            at: Instant::now(),
                        },
                    );
                    changed.push(e.id);
                }
            }
        }
        let bar_of = |bar: &Bar, health| {
            ToClient::BossBar(protocol::BossBar {
                boss: bar.guid,
                name: bar.name.clone(),
                health,
                max_health: bar.max_health,
                phases: bar.phases.clone(),
            })
        };
        let mut told = Vec::new();
        for id in changed {
            let bar = &self.bars[&id];
            told.extend(self.for_everyone(&bar_of(bar, bar.health)));
        }
        let gone: Vec<u32> = self
            .bars
            .keys()
            .copied()
            .filter(|id| !alive.contains(id))
            .collect();
        for id in gone {
            if let Some(bar) = self.bars.remove(&id) {
                told.extend(self.for_everyone(&bar_of(&bar, 0)));
            }
        }
        told
    }

    /// Each told hero's meter, when it changed: at most every 0.2 s while the run goes on.
    fn meters(&mut self) -> Vec<(u32, ToClient)> {
        let going = self.run.phase == Phase::Running;
        let mut told = Vec::new();
        for (&hero, t) in self.told.iter_mut() {
            let Some(tally) = self.meters.get(&hero) else {
                continue;
            };
            let Some(guid) = self
                .players
                .iter()
                .find(|p| p.hero == hero)
                .map(|p| p.guid())
            else {
                continue;
            };
            let meter = protocol::Meter {
                hero: guid,
                done: meter_entries(&tally.done),
                taken: meter_entries(&tally.taken),
            };
            let tell = match &t.meter {
                None => true,
                Some((last, at)) => *last != meter && (!going || due(*at)),
            };
            if tell {
                t.meter = Some((meter.clone(), Instant::now()));
                told.push((hero, ToClient::Meter(meter)));
            }
        }
        told
    }

    /// `hero`'s run as it ended: the tally, and what its last payment brought.
    fn run_end(&self, hero: u32, ending: Ending) -> Option<ToClient> {
        let i = self.run.heroes.iter().position(|h| h.id == hero)?;
        let saved = self.saved(hero)?;
        let s = self.run.summary(i);
        let total = self.saves.class_progress(&self.run.content, &saved.class).talent_experience;
        let (points, into_next) = talent_points(total);
        let last = self.paid.get(&hero).and_then(|p| p.last);
        let (parts, payout) = match last {
            Some((parts, payout)) => (parts, payout),
            None => (
                ExperienceParts::default(),
                Payout {
                    experience: 0,
                    total_before: total,
                    total_after: total,
                    points_before: points,
                    points_after: points,
                    zone_opened: None,
                },
            ),
        };
        Some(ToClient::RunEnd(protocol::RunEnd {
            hero: self.hero_guid(hero),
            ending,
            won: s.won,
            seconds: s.seconds,
            level: s.level,
            kills: s.kills,
            elites: s.elites,
            rares: s.rares,
            bosses: s.bosses,
            damage_dealt: s.damage_dealt.max(0.0).round() as u64,
            damage_taken: s.damage_taken.max(0.0).round() as u64,
            gold: s.gold_earned,
            caches: s.caches,
            loot: s.loot.iter().map(|g| g.entry).collect(),
            experience: protocol::Experience {
                time: parts.time,
                kills: parts.kills,
                bosses: parts.bosses,
                victory: parts.victory,
                zone: parts.zone,
                options: parts.options,
            },
            experience_before: payout.total_before,
            experience_after: payout.total_after,
            points_before: payout.points_before,
            points_after: payout.points_after,
            into_next,
            next_cost: crate::prerun::next_point_cost(points),
            zone_opened: payout
                .zone_opened
                .and_then(|z| self.run.content.zones.get(z))
                .map(|z| z.name.clone()),
        }))
    }
}

#[cfg(test)]
mod tests;
