//! The milestone kinds the legendary amulets brought (one legendary per
//! spell, docs/game/gear.md "Legendary amulets"): every n-th cast or hit, a buff's own rules while
//! it is up, a spree, a strike on one more enemy, bursts and lasting areas left behind, effects over
//! time that burst or pass on, barrages, casts again, heals that echo, poisons, combo points and
//! blows struck back. They are generic: any talent or amulet may use them.
//!
//! What they leave to happen later (a delayed burst, a barrage, a second cast) waits here and goes
//! off from the run tick; its damage is a milestone's, so it sets off further milestones only as
//! deep as any milestone damage does (spells.md 5b).

use std::collections::HashMap;

use esurv_content::{Ability, BurstBase, Counted, Mechanic, Milestone, Pick, Resource, Spot};

use super::abilities::{burn_ticks, hit_damage, ticks, BUFF_ENEMY_RANGE};
use super::casting::{Area, Dot, HitSource};
use super::enemy_ai::Vec2;
use super::milestones::{CombatEvent, MilestoneState, SPREAD_REACH};
use super::run::{Aim, Event, Run, Unit};
use crate::game::kit::{option_of, MAX_COMBO_POINTS};

/// The milestones an `Every` or a `WhileBuff` holds keep their state from this slot on, eight a
/// holder (`inner_slot`).
const INNER_SLOTS: usize = 64;
const INNER_PER_HOLDER: usize = 8;
/// A lasting area a milestone leaves strikes once a second, the first time this soon.
const LEFT_AREA_INTERVAL: f32 = 1.0;
const LEFT_AREA_FIRST_TICK: f32 = 0.25;
/// A lasting area a milestone leaves slows for this long at each strike.
const LEFT_AREA_SLOW_SECONDS: f32 = 1.5;
/// The reach of a strike on one more enemy when neither it nor the spell gives one.
const STRIKE_ONE_REACH: f32 = 10.0;

/// The state slot of milestone `j` held by milestone `k`.
fn inner_slot(k: usize, j: usize) -> usize {
    INNER_SLOTS + k * INNER_PER_HOLDER + j.min(INNER_PER_HOLDER - 1)
}

/// A burst still to go off.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingBurst {
    pub hero: usize,
    pub at: f32,
    /// The enemy it follows (it goes off where that enemy then stands), if any.
    pub follows: Option<u32>,
    pub spot: Vec2,
    pub radius: f32,
    pub hit: f32,
    pub effects: Vec<esurv_content::AfflictEffect>,
    pub look: u32,
    pub shown: HitSource,
}

/// A barrage under way.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingBarrage {
    pub hero: usize,
    pub until: f32,
    pub next: f32,
    pub interval: f32,
    pub reach: f32,
    pub hit: f32,
    pub look: u32,
    pub shown: HitSource,
}

/// A free cast still to come (`CastAgain`).
#[derive(Debug, Clone, PartialEq)]
pub struct PendingCast {
    pub hero: usize,
    pub spell: String,
    pub at: f32,
    pub opposite: bool,
    pub elsewhere: bool,
}

/// What the milestones of this module keep during a run.
#[derive(Debug, Clone, Default)]
pub struct ProcEffects {
    pub bursts: Vec<PendingBurst>,
    pub barrages: Vec<PendingBarrage>,
    pub casts: Vec<PendingCast>,
    /// `Every` counting on each enemy apart: (hero, milestone, enemy) to the count so far.
    pub per_enemy: HashMap<(usize, usize, u32), u32>,
    /// The cast under way is a `CastAgain` with `opposite`: a returning spell flies the other way.
    pub reverse_return: bool,
    /// The cast under way is a `CastAgain` with `elsewhere`: a ground area keeps off these spots
    /// (where its first fell, and how far it reached).
    pub avoid: Vec<(Vec2, f32)>,
    /// The pulsing looks on enemies (`legendary_looks.ron` `pulsing`).
    pub pulsing: Vec<PulsingLook>,
}

/// A legendary look pulsing on an enemy: hero `hero`'s look `look` on enemy `enemy`, played again at
/// `next`, until `until`.
#[derive(Debug, Clone, PartialEq)]
pub struct PulsingLook {
    pub hero: usize,
    pub enemy: u32,
    pub look: u32,
    pub until: f32,
    pub next: f32,
}

impl Run {
    /// Until when the buff `spell` left on hero `h` lasts, if it is up.
    pub(crate) fn buff_until(&self, h: usize, spell: &str) -> Option<f32> {
        let look = self.content.ability(spell)?.look_spell;
        let now = self.now;
        self.heroes[h].buffs.iter().filter(|b| b.visual == look && b.until > now).map(|b| b.until).reduce(f32::max)
    }

    /// Before hero `h` casts `a` on its own: each `Every` that counts its casts counts this one;
    /// the counted one arms it for the cast's hits. What it was before, for `uncount_cast`.
    pub(crate) fn count_own_cast(&mut self, h: usize, a: &Ability) -> Vec<(usize, MilestoneState)> {
        let milestones = self.heroes[h].milestones.clone();
        let mut before = Vec::new();
        for (k, m) in milestones.iter().enumerate() {
            let Milestone::Every { count, per: Counted::Casts, chance, filter, .. } = m else { continue };
            if !filter.matches(a) {
                continue;
            }
            before.push((k, *self.state(h, k)));
            let roll = self.rng.unit();
            let st = self.state(h, k);
            st.counter += 1;
            st.armed = st.counter >= (*count).max(1) && roll < *chance;
            if st.counter >= (*count).max(1) {
                st.counter = 0;
            }
        }
        before
    }

    /// A cast that found nothing to do counts for nothing.
    pub(crate) fn uncount_cast(&mut self, h: usize, before: Vec<(usize, MilestoneState)>) {
        for (k, st) in before {
            *self.state(h, k) = st;
        }
    }

    /// `Every` milestone `k` of hero `h` on event `e`.
    pub(crate) fn every_event(&mut self, h: usize, k: usize, m: &Milestone, e: &CombatEvent, source: &HitSource) {
        let Milestone::Every { count, per, per_enemy, chance, first_only, filter, then } = m else { return };
        match per {
            Counted::Casts => {
                if !self.state(h, k).armed {
                    return;
                }
                if self.fire_held(h, k, then, e, source) && *first_only {
                    self.state(h, k).armed = false;
                }
            }
            Counted::Hits => {
                let target = match e {
                    CombatEvent::Hit { target, .. } | CombatEvent::Tick { target, .. } => *target,
                    _ => return,
                };
                if !self.filter_passes(filter, source) {
                    return;
                }
                let n = (*count).max(1);
                let reached = if *per_enemy {
                    let c = self.procs.per_enemy.entry((h, k, target)).or_insert(0);
                    *c += 1;
                    let reached = *c >= n;
                    if reached {
                        *c = 0;
                    }
                    reached
                } else {
                    let st = self.state(h, k);
                    st.counter += 1;
                    let reached = st.counter >= n;
                    if reached {
                        st.counter = 0;
                    }
                    reached
                };
                if reached && (*chance >= 1.0 || self.rng.unit() < *chance) {
                    self.fire_held(h, k, then, e, source);
                }
            }
        }
    }

    /// `WhileBuff` milestone `k` of hero `h` on event `e`: its milestones while the buff is up.
    pub(crate) fn while_buff_event(&mut self, h: usize, k: usize, m: &Milestone, e: &CombatEvent, source: &HitSource) {
        let Milestone::WhileBuff { spell, then, once, .. } = m else { return };
        let Some(until) = self.buff_until(h, spell) else { return };
        // `once`: the buff that ends then has had its go
        if *once && (self.state(h, k).until - until).abs() < 1e-4 {
            return;
        }
        if self.fire_held(h, k, then, e, source) && *once {
            self.state(h, k).until = until;
        }
    }

    /// The milestones `then` held by milestone `k` go off on event `e` as their triggers say;
    /// whether any did.
    fn fire_held(&mut self, h: usize, k: usize, then: &[Milestone], e: &CombatEvent, source: &HitSource) -> bool {
        let mut any = false;
        for (j, inner) in then.iter().enumerate() {
            if self.goes_off(h, inner, e, source) {
                self.milestone_fires(h, inner_slot(k, j), inner, e, source);
                any = true;
            }
        }
        any
    }

    /// What `share` of `base` comes to on event `e`, set off by `source`.
    fn base_amount(&self, h: usize, base: BurstBase, e: &CombatEvent, source: &HitSource) -> f32 {
        let stats = self.heroes[h].stats;
        let spell_hit = || match source {
            HitSource::Spell(id) | HitSource::Tick(id) => self.content.ability(id).map_or(0.0, |a| hit_damage(a, &self.known_instance(h, &a.id), stats.damage)),
            _ => 0.0,
        };
        match base {
            BurstBase::Hit => match e {
                CombatEvent::Hit { dealt, .. } | CombatEvent::Tick { dealt, .. } => *dealt,
                CombatEvent::ShieldBroken { absorbed } => absorbed * stats.damage,
                _ => spell_hit(),
            },
            BurstBase::SpellHit => spell_hit(),
            BurstBase::MaxHealth => stats.max_health * stats.damage,
        }
    }

    /// Where event `e` happened, and the enemy it struck, for hero `h`.
    fn event_spot(&self, h: usize, e: &CombatEvent) -> (Vec2, Option<u32>) {
        match e {
            CombatEvent::Hit { target, at, .. } | CombatEvent::Tick { target, at, .. } | CombatEvent::DotEnded { target, at, .. } => (*at, Some(*target)),
            CombatEvent::Kill { at, .. } => (*at, None),
            _ => (self.heroes[h].pos, None),
        }
    }

    /// `n` spots around `at`, the first on it, the others up to `scatter` yd from it.
    fn scattered(&mut self, at: Vec2, n: u32, scatter: f32) -> Vec<Vec2> {
        (0..n.max(1))
            .map(|i| {
                if i == 0 || scatter <= 0.0 {
                    return at;
                }
                let (angle, r) = (self.rng.unit() * std::f32::consts::TAU, scatter * self.rng.unit().sqrt());
                (at.0 + r * angle.cos(), at.1 + r * angle.sin())
            })
            .collect()
    }

    /// How a milestone's own damage shows: `look` when it has one, else as the hit that set it off.
    fn proc_shown(&self, look: Option<u32>, source: &HitSource) -> (u32, HitSource) {
        let (spell, school) = self.shown_spell(source);
        let spell = look.unwrap_or(spell);
        (spell, self.proc_source(spell, school))
    }

    /// The milestones of this module going off (`milestone_fires` hands them on).
    pub(crate) fn proc_effect_fires(&mut self, h: usize, k: usize, m: &Milestone, e: &CombatEvent, source: &HitSource) {
        let now = self.now;
        let (spot, target) = self.event_spot(h, e);
        let dealt = match e {
            CombatEvent::Hit { dealt, .. } | CombatEvent::Tick { dealt, .. } => *dealt,
            _ => 0.0,
        };
        match m {
            Milestone::ResetOnTrigger { chance, min_interval, .. } => {
                let HitSource::Spell(spell) = source else { return };
                if now < self.state(h, k).ready_at || self.rng.unit() >= *chance {
                    return;
                }
                self.state(h, k).ready_at = now + min_interval;
                if let Some(slot) = self.heroes[h].spells.known.iter_mut().find(|s| s.spell == *spell) {
                    slot.timer = 0.0;
                }
            }
            Milestone::StrikeOne { pick, reach, share, effects, look_spell, reflection, .. } => {
                let Some(target) = target else { return };
                let reach = match (*reach, source) {
                    (r, _) if r > 0.0 => r,
                    (_, HitSource::Spell(id) | HitSource::Tick(id)) => self.content.ability(id).map_or(STRIKE_ONE_REACH, |a| {
                        if a.range > 0.0 {
                            a.range
                        } else if a.radius > 0.0 {
                            super::abilities::radius(a, &self.known_instance(h, &a.id), self.heroes[h].stats.area)
                        } else {
                            STRIKE_ONE_REACH
                        }
                    }),
                    _ => STRIKE_ONE_REACH,
                };
                let found: Vec<u32> = self.enemies_within(self.heroes[h].pos, reach).into_iter().filter(|&t| t != target).collect();
                let chosen = match pick {
                    Pick::Nearest => found.first().copied(),
                    Pick::Farthest => found.last().copied(),
                    Pick::Healthiest => found.iter().copied().max_by(|a, b| {
                        let health = |id: &u32| self.enemy(*id).map_or(0.0, |e| e.health);
                        health(a).total_cmp(&health(b))
                    }),
                };
                let Some(t) = chosen else { return };
                let (look, shown) = match look_spell {
                    Some(l) => self.proc_shown(Some(*l), source),
                    None => {
                        let shown = self.repeat_source(h, source);
                        let look = if let HitSource::Proc { spell, .. } = shown { spell } else { 0 };
                        (look, shown)
                    }
                };
                if *reflection {
                    // the hero's reflection strikes it, not the hero
                    self.push(Event::ReflectionStrikes { hero: self.heroes[h].id, target: t, spell: look });
                } else {
                    self.show_proc(h, look, &[t], Aim::Unit(Unit::Enemy(t)), 0.0);
                }
                if let Some(d) = self.hit_enemy(Some(h), t, share * dealt, false, shown) {
                    for x in effects {
                        self.afflict(h, Some(t), x, d);
                    }
                }
            }
            Milestone::Burst { share, of, radius, at, delay, count, scatter, effects, look_spell, mark_spell, .. } => {
                let hit = share * self.base_amount(h, *of, e, source);
                let (look, shown) = self.proc_shown(*look_spell, source);
                let (centre, follows) = match at {
                    Spot::Target => (spot, target),
                    Spot::Hero => (self.heroes[h].pos, None),
                };
                for p in self.scattered(centre, *count, *scatter) {
                    let follows = if p == centre { follows } else { None };
                    let b = PendingBurst { hero: h, at: now + delay, follows, spot: p, radius: *radius, hit, effects: effects.clone(), look, shown: shown.clone() };
                    if *delay > 0.0 {
                        // the enemy it follows wears the legendary's mark until it goes off
                        if let (Some(mark), Some(t)) = (*mark_spell, follows) {
                            self.enemy_wears_look(t, mark, b.at);
                        }
                        self.procs.bursts.push(b);
                    } else {
                        self.burst_now(&b);
                    }
                }
            }
            Milestone::DotBurst { share, radius, spreads, look_spell, filter, .. } => {
                let dots: Vec<Dot> = match e {
                    CombatEvent::DotEnded { dot, .. } => vec![dot.clone()],
                    CombatEvent::Kill { dots, .. } => dots.iter().filter(|d| self.filter_passes(filter, &HitSource::Tick(d.spell.clone()))).cloned().collect(),
                    _ => return,
                };
                for d in dots {
                    let ability = self.content.ability(&d.spell).cloned();
                    // its whole damage: a tick × the spell's ticks
                    let whole = d.per_tick * ability.as_ref().map_or(d.ticks_left.max(1), |a| a.ticks.max(1)) as f32;
                    let (look, shown) = self.proc_shown(*look_spell, &HitSource::Tick(d.spell.clone()));
                    self.show_proc(h, look, &[], Aim::Ground(spot), *radius);
                    for t in self.enemies_within(spot, *radius) {
                        if self.hit_enemy(Some(h), t, share * whole, false, shown.clone()).is_some() && *spreads {
                            let lacks = self.enemy(t).is_some_and(|x| !x.dots.iter().any(|y| y.spell == d.spell && y.ticks_left > 0));
                            if lacks {
                                let left = ability.as_ref().map_or(d.ticks_left, |a| a.ticks.max(1));
                                self.add_dot(t, Dot { ticks_left: left, next: now + d.interval, from_proc: false, ..d.clone() });
                            }
                        }
                    }
                }
            }
            Milestone::DotSpread { count, reach, max_carriers, every, moves, growth, look_spell, .. } => {
                let Some(target) = target else { return };
                if *every > 0.0 && now < self.state(h, k).ready_at {
                    return;
                }
                // the effect that passes: the ticking one, or the spell's own burn after a hit
                let dot = match (e, source) {
                    (CombatEvent::Tick { spell, .. }, _) => self.enemy(target).and_then(|x| x.dots.iter().find(|d| d.spell == *spell && d.ticks_left > 0).cloned()),
                    (_, HitSource::Spell(id)) => self.content.ability(id).and_then(|a| {
                        let b = burn_ticks(a, &self.known_instance(h, &a.id), self.heroes[h].stats.damage)?;
                        Some(Dot { hero: h, spell: a.id.clone(), per_tick: b.per_tick, ticks_left: b.ticks, interval: b.interval, next: now + b.interval, from_proc: false, leech: 0.0 })
                    }),
                    _ => None,
                };
                let Some(dot) = dot else { return };
                let carriers = self.enemies.iter().filter(|x| x.dots.iter().any(|d| d.spell == dot.spell && d.ticks_left > 0)).count() as u32;
                let room = if *max_carriers > 0 { max_carriers.saturating_sub(carriers) } else { *count };
                let reach = if *reach > 0.0 { *reach } else { SPREAD_REACH };
                let lacking: Vec<u32> = self
                    .nearest_to(spot, reach)
                    .into_iter()
                    .filter(|&t| t != target && self.enemy(t).is_some_and(|x| !x.dots.iter().any(|d| d.spell == dot.spell && d.ticks_left > 0)))
                    .take((*count).min(room) as usize)
                    .collect();
                if lacking.is_empty() {
                    return;
                }
                self.state(h, k).ready_at = now + every;
                let (look, _) = self.shown_spell(&HitSource::Tick(dot.spell.clone()));
                let look = look_spell.unwrap_or(look);
                for t in &lacking {
                    self.add_dot(*t, Dot { per_tick: dot.per_tick * (1.0 + growth), next: now + dot.interval, from_proc: false, ..dot.clone() });
                    // a pulsing look plays while the burn lasts there, any other once
                    if self.content.is_pulsing_look(look) {
                        let until = now + dot.interval * dot.ticks_left as f32;
                        self.enemy_wears_or_pulses_look(h, *t, look, until);
                    } else {
                        self.show_proc(h, look, &[*t], Aim::Unit(Unit::Enemy(*t)), 0.0);
                    }
                }
                if *moves {
                    if let Some(x) = self.enemies.iter_mut().find(|x| x.id == target) {
                        x.dots.retain(|d| d.spell != dot.spell);
                    }
                }
            }
            Milestone::LeaveArea { share, of, seconds, radius, at, count, scatter, slow, poisons, follows_hero, rides_target, look_spell, spirits, .. } => {
                let per_tick = share * self.base_amount(h, *of, e, source);
                let (look, shown) = self.proc_shown(*look_spell, source);
                let centre = match at {
                    Spot::Target => spot,
                    Spot::Hero => self.heroes[h].pos,
                };
                let left = super::casting::LeftArea { slow: *slow, slow_seconds: LEFT_AREA_SLOW_SECONDS, poisons: *poisons, rides: if *rides_target { target } else { None }, spirits: spirits.clone() };
                for p in self.scattered(centre, *count, *scatter) {
                    self.leave_area(h, p, *radius, per_tick, *seconds, *follows_hero, left.clone(), look, shown.clone());
                }
            }
            Milestone::Barrage { seconds, interval, reach, share, of, look_spell, .. } => {
                let hit = share * self.base_amount(h, *of, e, source);
                let (look, shown) = self.proc_shown(*look_spell, source);
                self.procs.barrages.push(PendingBarrage { hero: h, until: now + seconds, next: now, interval: interval.max(0.05), reach: *reach, hit, look, shown });
            }
            Milestone::CastAgain { delay, opposite, elsewhere, .. } => {
                let HitSource::Spell(spell) = source else { return };
                self.procs.casts.push(PendingCast { hero: h, spell: spell.clone(), at: now + delay, opposite: *opposite, elsewhere: *elsewhere });
            }
            Milestone::ApplyPoison { .. } => {
                if let Some(t) = target {
                    self.put_option_poison(h, t);
                }
            }
            Milestone::ComboPoints { points, .. } => {
                let content = self.content.clone();
                let Some(Resource::Energy { combo_points, .. }) = option_of(&content, &self.heroes[h]).and_then(|o| o.resource) else { return };
                let k = &mut self.heroes[h].kit;
                k.combo = (k.combo + points).min(MAX_COMBO_POINTS).min(combo_points.max(1));
                let c = k.combo as f32;
                self.show_resource(h, esurv_content::ResourceKind::ComboPoints, c, combo_points as f32);
            }
            Milestone::Retaliate { share, look_spell } => {
                let CombatEvent::StruckInMelee { by, taken } = e else { return };
                let shown = self.proc_source(look_spell.unwrap_or(0), esurv_content::School::Holy);
                self.hit_enemy(Some(h), *by, share * taken, false, shown);
            }
            _ => {}
        }
    }

    /// A burst going off now: its look at its spot, then every enemy within its radius struck.
    fn burst_now(&mut self, b: &PendingBurst) {
        let spot = b.follows.and_then(|t| self.enemy_pos(t)).unwrap_or(b.spot);
        self.show_proc(b.hero, b.look, &[], Aim::Ground(spot), b.radius);
        for t in self.enemies_within(spot, b.radius) {
            if let Some(d) = self.hit_enemy(Some(b.hero), t, b.hit, false, b.shown.clone()) {
                for x in &b.effects {
                    self.afflict(b.hero, Some(t), x, d);
                }
            }
        }
    }

    /// Leaves a lasting area of hero `h` at `at`: every enemy inside struck for `per_tick` once a
    /// second for `seconds`, doing what `left` says beside; moving with the hero with `follows`;
    /// shown as `look`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn leave_area(&mut self, h: usize, at: Vec2, radius: f32, per_tick: f32, seconds: f32, follows: bool, left: super::casting::LeftArea, look: u32, shown: HitSource) {
        let proc_hit = match shown {
            HitSource::Proc { spell, school, origin } => Some((spell, school, origin)),
            _ => None,
        };
        let id = self.new_id();
        let until = self.now + seconds + super::casting::AREA_LOOK_AFTER;
        self.push(Event::AreaShown { id, caster: Unit::Hero(self.heroes[h].id), spell: look, at, radius, until: Some(until) });
        if let Some(spirits) = &left.spirits {
            self.push(Event::SpiritsAppeared { area: id, hero: self.heroes[h].id, until: self.now + seconds, spirits: spirits.clone() });
        }
        let ticks_left = (seconds / LEFT_AREA_INTERVAL).round().max(1.0) as u32;
        let next = self.now + LEFT_AREA_FIRST_TICK;
        self.areas.push(Area { id, hero: h, spell: String::new(), at, radius, per_tick, ticks_left, interval: LEFT_AREA_INTERVAL, next, from_proc: true, base_radius: radius, growth: None, follows, proc_hit, shown_at: at, shown_radius: radius, left: Some(left) });
    }

    /// Puts the poison of hero `h`'s starting option (a known spell that `Poisons`) on enemy `t`,
    /// as that spell's hits do; nothing without one.
    pub(crate) fn put_option_poison(&mut self, h: usize, t: u32) {
        let content = self.content.clone();
        let found = self.heroes[h].spells.known.iter().find_map(|s| match content.ability(&s.spell).and_then(|a| a.kit.mechanic.as_ref()) {
            Some(Mechanic::Poisons { poison }) => Some((s.spell.clone(), poison.clone())),
            _ => None,
        });
        let Some((by, poison)) = found else { return };
        let Some(p) = content.ability(&poison) else { return };
        let inst = super::abilities::Instance { extra_targets_from_cards: 0, extra_targets_from_talents: 0, ..self.known_instance(h, &by) };
        let (count, per_tick) = ticks(p, &inst, hit_damage(p, &inst, self.heroes[h].stats.damage));
        let interval = p.duration.max(0.01) / p.ticks.max(1) as f32;
        let now = self.now;
        if let Some(e) = self.enemies.iter_mut().find(|e| e.id == t) {
            e.dots.retain(|d| !(d.hero == h && d.spell == p.id));
        }
        let from_proc = self.casting_proc > 0;
        self.add_dot(t, Dot { hero: h, spell: p.id.clone(), per_tick, ticks_left: count, interval, next: now + interval, from_proc, leech: 0.0 });
    }

    /// A heal of `a` for `amount` echoes as its `HealAfter` milestones say: a share more over time.
    pub(crate) fn heal_after(&mut self, h: usize, a: &Ability, amount: f32) {
        let now = self.now;
        let echoes: Vec<(f32, f32)> = self.heroes[h]
            .milestones
            .iter()
            .filter_map(|m| match m {
                Milestone::HealAfter { share, seconds, filter } if filter.matches(a) => Some((*share, *seconds)),
                _ => None,
            })
            .collect();
        for (share, seconds) in echoes {
            let seconds = seconds.max(0.1);
            let value = share * amount / seconds;
            self.heroes[h].buffs.push(super::run::Buff { kind: super::run::BuffKind::Regen, value, visual: a.look_spell, until: now + seconds });
        }
        self.dot_from_heal(h, a, amount);
    }

    /// A cast of heal `a` for `amount` in all puts its `DotFromHeal` milestones' effects over
    /// time on the hero's nearest enemies (or those first in its order): each deals the share of
    /// `amount` over its seconds, ticking once a second. One such effect per enemy: a new one
    /// takes over what the old one had still to deal, and starts its ticks afresh.
    pub(crate) fn dot_from_heal(&mut self, h: usize, a: &Ability, amount: f32) {
        let now = self.now;
        let dots: Vec<(f32, f32, u32, f32, Option<esurv_content::TargetOrder>, u32)> = self.heroes[h]
            .milestones
            .iter()
            .filter_map(|m| match m {
                Milestone::DotFromHeal { share, seconds, targets, reach, order, look_spell, filter } if filter.matches(a) => {
                    Some((*share, *seconds, *targets, *reach, *order, look_spell.unwrap_or(0)))
                }
                _ => None,
            })
            .collect();
        let from_proc = self.casting_proc > 0;
        for (share, seconds, targets, reach, order, look) in dots {
            let ticks = (seconds / super::milestones::DOT_ON_HIT_TICK).round().max(1.0) as u32;
            let spell = format!("{}{look}", super::milestones::AURA_PREFIX);
            let at = self.heroes[h].pos;
            let near = match order {
                Some(order) => self.enemies_in_order(order, at, reach),
                None => self.nearest_to(at, reach),
            };
            let near: Vec<u32> = near.into_iter().take(targets as usize).collect();
            for t in near {
                let Some(e) = self.enemies.iter_mut().find(|x| x.id == t) else { continue };
                let left: f32 = e.dots.iter().filter(|d| d.hero == h && d.spell == spell).map(|d| d.per_tick * d.ticks_left as f32).sum();
                e.dots.retain(|d| !(d.hero == h && d.spell == spell));
                e.dots.push(Dot {
                    hero: h,
                    spell: spell.clone(),
                    per_tick: (share * amount + left) / ticks as f32,
                    ticks_left: ticks,
                    interval: super::milestones::DOT_ON_HIT_TICK,
                    next: now + super::milestones::DOT_ON_HIT_TICK,
                    from_proc,
                    leech: 0.0,
                });
            }
        }
    }

    /// Spree milestone `k` of hero `h`: every `every` seconds (once an enemy is in reach), for
    /// `seconds`, its spell cast for free every `interval`, its stats on.
    pub(crate) fn spree_tick(&mut self, h: usize, k: usize, m: &Milestone) {
        let Milestone::Spree { every, seconds, interval, spell, look_spell, .. } = m else { return };
        let now = self.now;
        let st = *self.state(h, k);
        if st.stacks == 0 && now >= st.next {
            if self.enemies_within(self.heroes[h].pos, BUFF_ENEMY_RANGE).is_empty() {
                return;
            }
            let st = self.state(h, k);
            (st.stacks, st.until, st.next, st.ready_at) = (1, now + seconds, now + every, now);
            self.refresh_stats(h);
            if let Some(look) = *look_spell {
                self.hero_wears_look(h, look, now + seconds);
            }
        }
        let st = *self.state(h, k);
        if st.stacks > 0 && now < st.until && now >= st.ready_at {
            self.state(h, k).ready_at = now + interval.max(0.05);
            self.free_cast(h, spell);
        }
    }

    /// What the milestones left to happen later: delayed bursts, barrages, second casts.
    pub(crate) fn proc_effects_tick(&mut self) {
        self.pulse_looks_tick();
        let now = self.now;
        let (due, later): (Vec<PendingBurst>, Vec<PendingBurst>) = std::mem::take(&mut self.procs.bursts).into_iter().partition(|b| b.at <= now);
        self.procs.bursts = later;
        self.proc_depth += 1;
        for b in due.iter() {
            if !self.heroes[b.hero].alive {
                continue;
            }
            self.burst_now(b);
        }
        // barrages: one enemy within reach of the hero struck at each interval
        let mut strikes = Vec::new();
        for b in &mut self.procs.barrages {
            while b.next <= now && b.next < b.until {
                b.next += b.interval;
                strikes.push((b.hero, b.reach, b.hit, b.look, b.shown.clone()));
            }
        }
        self.procs.barrages.retain(|b| b.next < b.until);
        for (h, reach, hit, look, shown) in strikes {
            if !self.heroes[h].alive {
                continue;
            }
            let found = self.enemies_within(self.heroes[h].pos, reach);
            if found.is_empty() {
                continue;
            }
            let t = found[(self.rng.next_u64() % found.len() as u64) as usize];
            self.show_proc(h, look, &[t], Aim::Unit(Unit::Enemy(t)), 0.0);
            self.hit_enemy(Some(h), t, hit, false, shown);
        }
        self.proc_depth -= 1;
        // casts again: free, as a milestone's are
        let (due, later): (Vec<PendingCast>, Vec<PendingCast>) = std::mem::take(&mut self.procs.casts).into_iter().partition(|c| c.at <= now);
        self.procs.casts = later;
        for c in due {
            if !self.heroes[c.hero].alive {
                continue;
            }
            self.procs.reverse_return = c.opposite;
            self.procs.avoid = if c.elsewhere { self.areas.iter().filter(|a| a.hero == c.hero && a.spell == c.spell).map(|a| (a.at, a.radius)).collect() } else { Vec::new() };
            self.free_cast(c.hero, &c.spell);
            self.procs.reverse_return = false;
            self.procs.avoid.clear();
        }
    }

    /// Hero `h`'s buffs that ended: their spells' `BuffEnds` milestones go off.
    pub(crate) fn buffs_ended(&mut self, h: usize, visuals: &[u32]) {
        let content = self.content.clone();
        for v in visuals {
            let spell = self.heroes[h].spells.known.iter().chain(self.heroes[h].spells.racial.iter()).find(|s| content.ability(&s.spell).is_some_and(|a| a.look_spell == *v)).map(|s| s.spell.clone());
            if let Some(spell) = spell {
                self.milestone_event(h, CombatEvent::BuffEnded { spell });
            }
        }
    }

    /// Whether a ground area of hero `h` may fall on `spot` (a second one keeps off the first).
    pub(crate) fn spot_free(&self, spot: Vec2) -> bool {
        self.procs.avoid.iter().all(|(at, r)| ((spot.0 - at.0).powi(2) + (spot.1 - at.1).powi(2)).sqrt() > *r)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::{AfflictEffect, Content, SpellFilter, Stat, Trigger};

    use super::*;
    use crate::game::enemy::RunOptions;
    use crate::game::run::{Buff, BuffKind, Flat, HeroSetup};

    fn content() -> Arc<Content> {
        Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).expect("content loads"))
    }

    fn run_as(class: &str, milestones: Vec<Milestone>) -> Run {
        let setup = HeroSetup { class: class.into(), race: "orc".into(), milestones, ..Default::default() };
        let mut r = Run::new(content(), "elwynn", RunOptions::default(), &[setup], Box::new(Flat), 23).unwrap();
        let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
        r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        r.enemies.clear();
        r.pickups.clear();
        r.drain_events();
        r
    }

    fn run(milestones: Vec<Milestone>) -> Run {
        run_as("mage", milestones)
    }

    fn enemy(r: &mut Run, d: f32) -> u32 {
        let p = r.heroes[0].pos;
        assert!(r.spawn_roster_enemy(0, Some((p.0 + d, p.1))));
        let e = r.enemies.last_mut().unwrap();
        e.health = 1e6;
        e.max_health = 1e6;
        e.id
    }

    fn lost(r: &Run, id: u32) -> f32 {
        r.enemies.iter().find(|e| e.id == id).map_or(f32::MAX, |e| e.max_health - e.health)
    }

    fn spell(id: &str) -> HitSource {
        HitSource::Spell(id.into())
    }

    fn fb() -> SpellFilter {
        SpellFilter::Spell("fireball".into())
    }

    fn splash(share: f32) -> Milestone {
        Milestone::Splash { share, radius: 8.0, look_spell: None, requires_aura: None, trigger: Trigger::Hit, filter: fb(), effects: vec![] }
    }

    #[test]
    fn every_nth_cast_arms_that_casts_hits_only() {
        let m = Milestone::Every { count: 2, per: Counted::Casts, per_enemy: false, chance: 1.0, first_only: false, filter: fb(), then: vec![splash(0.5)] };
        let mut r = run(vec![m]);
        let (a, b) = (enemy(&mut r, 10.0), enemy(&mut r, 12.0));
        let fireball = r.content.ability("fireball").unwrap().clone();
        r.count_own_cast(0, &fireball);
        r.hit_enemy(Some(0), a, 100.0, false, spell("fireball"));
        assert_eq!(lost(&r, b), 0.0, "the first cast is not the counted one");
        r.count_own_cast(0, &fireball);
        r.hit_enemy(Some(0), a, 100.0, false, spell("fireball"));
        assert!((lost(&r, b) - 50.0).abs() < 0.01, "the second cast's hit splashes");
        // a cast that found nothing counts for nothing
        let before = r.count_own_cast(0, &fireball);
        r.uncount_cast(0, before);
        r.hit_enemy(Some(0), a, 100.0, false, spell("fireball"));
        assert!((lost(&r, b) - 100.0).abs() < 0.01, "still armed from the counted cast");
    }

    #[test]
    fn every_nth_hit_on_each_enemy_counts_apart() {
        let then = vec![Milestone::Afflict { look_spell: None, chance: 1.0, effect: AfflictEffect::Vulnerable { amount: 0.5, seconds: 10.0 }, trigger: Trigger::Hit, filter: fb() }];
        let m = Milestone::Every { count: 2, per: Counted::Hits, per_enemy: true, chance: 1.0, first_only: false, filter: fb(), then };
        let mut r = run(vec![m]);
        let (a, b) = (enemy(&mut r, 5.0), enemy(&mut r, 6.0));
        let vulnerable = |r: &Run, id: u32| r.enemies.iter().find(|e| e.id == id).unwrap().vulnerable.is_some();
        r.hit_enemy(Some(0), a, 1.0, false, spell("fireball"));
        r.hit_enemy(Some(0), b, 1.0, false, spell("fireball"));
        assert!(!vulnerable(&r, a) && !vulnerable(&r, b), "one hit each");
        r.hit_enemy(Some(0), a, 1.0, false, spell("fireball"));
        assert!(vulnerable(&r, a) && !vulnerable(&r, b), "a's second hit");
    }

    #[test]
    fn a_delayed_burst_goes_off_later_where_its_enemy_stands() {
        let m = Milestone::Burst { mark_spell: None, share: 1.0, of: BurstBase::Hit, radius: 5.0, at: Spot::Target, delay: 1.0, count: 1, scatter: 0.0, effects: vec![], look_spell: None, trigger: Trigger::Hit, filter: fb() };
        let mut r = run(vec![m]);
        let (a, b) = (enemy(&mut r, 10.0), enemy(&mut r, 12.0));
        r.hit_enemy(Some(0), a, 100.0, false, spell("fireball"));
        assert_eq!(lost(&r, b), 0.0);
        r.now += 1.0;
        r.proc_effects_tick();
        assert!((lost(&r, b) - 100.0).abs() < 0.01);
    }

    /// The legendary looks: a delayed burst's mark is worn by its
    /// enemy until it goes off, and the burst shows its own look.
    #[test]
    fn a_delayed_burst_s_mark_is_worn_until_it_goes_off() {
        let m = Milestone::Burst { mark_spell: Some(20475), share: 1.0, of: BurstBase::Hit, radius: 5.0, at: Spot::Target, delay: 3.0, count: 1, scatter: 0.0, effects: vec![], look_spell: Some(20476), trigger: Trigger::Hit, filter: fb() };
        let mut r = run(vec![m]);
        let a = enemy(&mut r, 10.0);
        let now = r.now;
        r.hit_enemy(Some(0), a, 100.0, false, spell("fireball"));
        assert_eq!(r.enemy(a).unwrap().kit.worn_looks, [(20475, now + 3.0)]);
        r.drain_events();
        r.now += 3.0;
        r.proc_effects_tick();
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::SpellGo { spell: 20476, aim: Aim::Ground(_), .. })));
    }

    /// The legendary looks: a stun's look is worn while the stun lasts,
    /// a sweep shows its own look on each enemy it sweeps to, and a spree's look is worn while it
    /// lasts.
    #[test]
    fn legendary_moments_wear_and_show_their_looks() {
        let stun = Milestone::Afflict { chance: 1.0, effect: AfflictEffect::Stun { seconds: 1.0 }, trigger: Trigger::Hit, filter: fb(), look_spell: Some(11020) };
        let sweep = Milestone::ExtraTargets { count: 1, share: 0.5, trigger: Trigger::Hit, filter: fb(), per_swing_second: false, growth: 0.0, reach: 0.0, effects: vec![], look_spell: Some(8398) };
        let mut r = run(vec![stun, sweep]);
        let (a, b) = (enemy(&mut r, 10.0), enemy(&mut r, 12.0));
        let now = r.now;
        r.hit_enemy(Some(0), a, 100.0, false, spell("fireball"));
        assert!(r.enemy(a).unwrap().kit.worn_looks.contains(&(11020, now + 1.0)));
        let events = r.drain_events();
        assert!(events.iter().any(|e| matches!(e, Event::SpellGo { spell: 8398, hits, .. } if hits == &[Unit::Enemy(b)])));

        let spree = Milestone::Spree { every: 15.0, seconds: 3.0, interval: 0.5, spell: "whirlwind".into(), stats: vec![], look_spell: Some(9632) };
        let mut r = run_as("warrior", vec![spree.clone()]);
        enemy(&mut r, 3.0);
        let now = r.now;
        r.spree_tick(0, 0, &spree);
        assert_eq!(r.heroes[0].kit.worn_looks, [(9632, now + 3.0)]);
    }

    #[test]
    fn a_dot_bursts_on_death_and_passes_to_those_it_strikes() {
        let m = Milestone::DotBurst { share: 1.0, radius: 6.0, on_end: false, on_death: true, spreads: true, look_spell: None, filter: SpellFilter::Spell("sw_pain".into()) };
        let mut r = run(vec![m]);
        let (a, b) = (enemy(&mut r, 5.0), enemy(&mut r, 7.0));
        let ticks = r.content.ability("sw_pain").unwrap().ticks as f32;
        let dot = Dot { hero: 0, spell: "sw_pain".into(), per_tick: 10.0, ticks_left: 3, interval: 1.0, next: r.now + 1.0, from_proc: false, leech: 0.0 };
        r.enemies.iter_mut().find(|e| e.id == a).unwrap().dots.push(dot);
        r.hit_enemy(Some(0), a, f32::MAX, false, spell("fireball"));
        assert!((lost(&r, b) - 10.0 * ticks).abs() < 0.01, "its whole damage");
        assert!(r.enemies.iter().find(|e| e.id == b).unwrap().dots.iter().any(|d| d.spell == "sw_pain"));
    }

    #[test]
    fn a_dot_that_runs_out_bursts() {
        let m = Milestone::DotBurst { share: 1.0, radius: 6.0, on_end: true, on_death: false, spreads: false, look_spell: None, filter: SpellFilter::Spell("devouring_plague".into()) };
        let mut r = run(vec![m]);
        let (a, b) = (enemy(&mut r, 5.0), enemy(&mut r, 7.0));
        let dot = Dot { hero: 0, spell: "devouring_plague".into(), per_tick: 10.0, ticks_left: 1, interval: 1.0, next: r.now, from_proc: false, leech: 0.0 };
        r.enemies.iter_mut().find(|e| e.id == a).unwrap().dots.push(dot);
        r.effects_tick();
        assert!(lost(&r, b) > 0.0, "the plague detonated as it ended");
    }

    #[test]
    fn a_spreading_dot_stops_at_its_carrier_limit_and_a_moving_one_leaves() {
        let m = Milestone::DotSpread { look_spell: None, count: 1, reach: 0.0, max_carriers: 2, every: 0.0, moves: false, growth: 0.0, trigger: Trigger::DotTick, filter: SpellFilter::Spell("rend".into()) };
        let mut r = run(vec![m]);
        let (a, b, c) = (enemy(&mut r, 5.0), enemy(&mut r, 6.0), enemy(&mut r, 7.0));
        let dot = Dot { hero: 0, spell: "rend".into(), per_tick: 5.0, ticks_left: 5, interval: 1.0, next: r.now + 1.0, from_proc: false, leech: 0.0 };
        r.enemies.iter_mut().find(|e| e.id == a).unwrap().dots.push(dot);
        let has = |r: &Run, id: u32| r.enemies.iter().find(|e| e.id == id).unwrap().dots.iter().any(|d| d.spell == "rend");
        r.hit_enemy(Some(0), a, 5.0, false, HitSource::Tick("rend".into()));
        assert!(has(&r, b));
        r.hit_enemy(Some(0), a, 5.0, false, HitSource::Tick("rend".into()));
        assert!(!has(&r, c), "two carry it already");
        let m = Milestone::DotSpread { look_spell: None, count: 1, reach: 0.0, max_carriers: 0, every: 3.0, moves: true, growth: 0.2, trigger: Trigger::DotTick, filter: SpellFilter::Spell("rend".into()) };
        r.heroes[0].milestones = vec![m];
        r.hit_enemy(Some(0), b, 5.0, false, HitSource::Tick("rend".into()));
        assert!(!has(&r, b) && has(&r, c), "it moved on");
        let per_tick = r.enemies.iter().find(|e| e.id == c).unwrap().dots.iter().find(|d| d.spell == "rend").unwrap().per_tick;
        assert!((per_tick - 6.0).abs() < 1e-4, "20 % stronger");
    }

    #[test]
    fn a_left_area_strikes_and_slows_once_a_second() {
        let m = Milestone::LeaveArea { share: 1.0, of: BurstBase::Hit, seconds: 2.0, radius: 4.0, at: Spot::Target, count: 1, scatter: 0.0, slow: 0.5, poisons: false, follows_hero: false, rides_target: false, look_spell: None, spirits: None, trigger: Trigger::Hit, filter: fb() };
        let mut r = run(vec![m]);
        let a = enemy(&mut r, 10.0);
        r.hit_enemy(Some(0), a, 10.0, false, spell("fireball"));
        assert_eq!(r.areas.len(), 1);
        r.now += 0.3;
        r.effects_tick();
        assert!(r.enemies.iter().find(|e| e.id == a).unwrap().slow.is_some(), "slowed");
        assert!(lost(&r, a) >= 20.0 - 0.01, "the hit and the area's first strike");
    }

    /// A pulsing look (Sunder Armor's strike on an exposed enemy) plays at once and
    /// again every 2 sec while the effect lasts, is never worn as an aura, and stops with it.
    #[test]
    fn a_pulsing_look_plays_again_while_the_effect_lasts() {
        let look = 7405;
        let m = Milestone::Afflict { look_spell: Some(look), chance: 1.0, effect: AfflictEffect::Vulnerable { amount: 0.5, seconds: 5.0 }, trigger: Trigger::Hit, filter: fb() };
        let mut r = run(vec![m]);
        assert!(r.content.is_pulsing_look(look));
        let a = enemy(&mut r, 5.0);
        let plays = |r: &mut Run| r.drain_events().iter().filter(|e| matches!(e, Event::SpellGo { spell, hits, .. } if *spell == look && hits == &vec![Unit::Enemy(a)])).count();
        r.hit_enemy(Some(0), a, 1.0, false, spell("fireball"));
        assert_eq!(plays(&mut r), 1, "at once");
        assert!(r.enemies.iter().find(|e| e.id == a).unwrap().kit.worn_looks.iter().all(|(l, _)| *l != look), "not worn");
        let mut later = 0;
        for _ in 0..12 {
            r.now += 0.5;
            r.proc_effects_tick();
            later += plays(&mut r);
        }
        assert_eq!(later, 2, "at 2 and 4 sec, then the effect is over");
    }

    /// Blink Strike: with `reflection`, the hero's reflection strikes the further
    /// enemy, the hero shows nothing there, and the damage is the milestone's.
    #[test]
    fn a_reflection_strikes_the_further_enemy_in_the_hero_s_place() {
        let one = Milestone::StrikeOne { pick: Pick::Nearest, reach: 15.0, share: 1.5, effects: vec![], look_spell: Some(14278), reflection: true, trigger: Trigger::Hit, filter: fb() };
        let mut r = run(vec![one]);
        let (a, b) = (enemy(&mut r, 3.0), enemy(&mut r, 6.0));
        let hero_at = r.heroes[0].pos;
        r.hit_enemy(Some(0), a, 10.0, false, spell("fireball"));
        let ev = r.drain_events();
        let hero = r.heroes[0].id;
        assert!(ev.iter().any(|e| matches!(e, Event::ReflectionStrikes { hero: h, target, spell: 14278 } if *h == hero && *target == b)), "{ev:?}");
        assert!(!ev.iter().any(|e| matches!(e, Event::SpellGo { spell: 14278, .. })), "the hero strikes nothing there");
        assert!((lost(&r, b) - 15.0).abs() < 0.01);
        assert_eq!(r.heroes[0].pos, hero_at, "the hero stays put");
    }

    /// An area with spirits (Sprint of the Wild) says they set off with it, and as it strikes,
    /// whom each strikes; the damage is the area's.
    #[test]
    fn spirits_set_off_with_their_area_and_strike_what_it_strikes() {
        let spirits = esurv_content::AreaSpirits { count: 2, form: esurv_content::PlayerForm::Cat, scale: 0.6, strike_look: 6800 };
        let m = Milestone::LeaveArea { share: 1.0, of: BurstBase::Hit, seconds: 3.0, radius: 5.0, at: Spot::Hero, count: 1, scatter: 0.0, slow: 0.0, poisons: false, follows_hero: true, rides_target: false, look_spell: None, spirits: Some(spirits.clone()), trigger: Trigger::Hit, filter: fb() };
        let mut r = run(vec![m]);
        let (a, b, c) = (enemy(&mut r, 2.0), enemy(&mut r, 3.0), enemy(&mut r, 4.0));
        r.hit_enemy(Some(0), a, 10.0, false, spell("fireball"));
        let ev = r.drain_events();
        let area = ev.iter().find_map(|e| if let Event::SpiritsAppeared { area, spirits: s, .. } = e { Some((*area, s.clone())) } else { None });
        let Some((area, told)) = area else { panic!("{ev:?}") };
        assert_eq!(told, spirits);
        r.now += 0.3;
        r.effects_tick();
        let strikes: Vec<Vec<u32>> = r.drain_events().into_iter().filter_map(|e| if let Event::SpiritsStrike { area: x, targets } = e { (x == area).then_some(targets) } else { None }).collect();
        let [targets] = &strikes[..] else { panic!("{strikes:?}") };
        assert_eq!(targets.len(), 2, "one enemy each");
        assert!(targets.iter().all(|t| [a, b, c].contains(t)));
        assert!(lost(&r, c) > 0.0, "the area struck all three");
    }

    #[test]
    fn a_buff_brings_its_stats_and_its_milestones_once() {
        let then = vec![Milestone::StrikeAgain { look_spell: None, chance: 1.0, times: 1, share: 1.0, trigger: Trigger::Hit, filter: SpellFilter::WeaponSwing }];
        let m = Milestone::WhileBuff { look_spell: None, spell: "battle_shout".into(), stats: vec![(Stat::MoveSpeed, 0.3)], then, once: true };
        let mut r = run(vec![m]);
        let a = enemy(&mut r, 3.0);
        let speed = r.heroes[0].stats.move_speed;
        r.hit_enemy(Some(0), a, 10.0, false, HitSource::WeaponSwing);
        assert!((lost(&r, a) - 10.0).abs() < 0.01, "no buff, no more");
        let look = r.content.ability("battle_shout").unwrap().look_spell;
        let until = r.now + 5.0;
        r.heroes[0].buffs.push(Buff { kind: BuffKind::Damage, value: 0.0, visual: look, until });
        r.refresh_stats(0);
        assert!((r.heroes[0].stats.move_speed - speed - 0.3).abs() < 1e-4);
        r.hit_enemy(Some(0), a, 10.0, false, HitSource::WeaponSwing);
        r.hit_enemy(Some(0), a, 10.0, false, HitSource::WeaponSwing);
        assert!((lost(&r, a) - 40.0).abs() < 0.01, "one extra blow, once a buff");
    }

    #[test]
    fn small_rules_reset_strike_recast_strike_back_and_echo() {
        let reset = Milestone::ResetOnTrigger { chance: 1.0, min_interval: 0.0, trigger: Trigger::Crit, filter: SpellFilter::Spell("fire_blast".into()) };
        let one = Milestone::StrikeOne { pick: Pick::Healthiest, reach: 20.0, share: 2.0, effects: vec![], look_spell: None, reflection: false, trigger: Trigger::Hit, filter: fb() };
        let again = Milestone::CastAgain { delay: 0.5, opposite: false, elsewhere: false, filter: SpellFilter::Spell("frost_nova".into()) };
        let back = Milestone::Retaliate { share: 0.25, look_spell: None };
        let echo = Milestone::HealAfter { share: 0.5, seconds: 6.0, filter: SpellFilter::Spell("healing_wave".into()) };
        let mut r = run(vec![reset, one, again, back, echo]);
        let (a, b, c) = (enemy(&mut r, 3.0), enemy(&mut r, 6.0), enemy(&mut r, 9.0));
        r.enemies.iter_mut().find(|e| e.id == b).unwrap().health = 100.0;
        // a crit readies Fire Blast
        let blast = r.content.ability("fire_blast").unwrap().clone();
        r.heroes[0].spells.learn(&blast, &[]);
        let k = r.heroes[0].spells.known.iter().position(|s| s.spell == "fire_blast").unwrap();
        r.heroes[0].spells.known[k].timer = 5.0;
        r.hit_enemy(Some(0), a, 1.0, true, spell("fire_blast"));
        assert_eq!(r.heroes[0].spells.known[k].timer, 0.0);
        // one more enemy, the healthiest
        let b_lost = lost(&r, b);
        r.hit_enemy(Some(0), a, 10.0, false, spell("fireball"));
        assert!((lost(&r, c) - 20.0).abs() < 0.01 && lost(&r, b) == b_lost);
        // a second cast waits half a second
        r.milestone_event(0, CombatEvent::Cast { spell: "frost_nova".into() });
        assert_eq!(r.procs.casts.len(), 1);
        // a blow struck back
        let before = lost(&r, a);
        r.milestone_event(0, CombatEvent::StruckInMelee { by: a, taken: 40.0 });
        assert!((lost(&r, a) - before - 10.0).abs() < 0.01);
        // a heal that echoes
        let wave = r.content.ability("healing_wave").unwrap().clone();
        r.heal_after(0, &wave, 60.0);
        assert!(r.heroes[0].buffs.iter().any(|b| b.kind == BuffKind::Regen && (b.value - 5.0).abs() < 1e-4));
    }

    #[test]
    fn silence_and_knockback_land_on_the_enemy() {
        let mut r = run(vec![]);
        let a = enemy(&mut r, 5.0);
        r.afflict(0, Some(a), &AfflictEffect::Silence { seconds: 3.0 }, 0.0);
        let at = r.enemy_pos(a).unwrap();
        r.afflict(0, Some(a), &AfflictEffect::Knockback { yards: 4.0 }, 0.0);
        assert!(r.enemies.iter().find(|e| e.id == a).unwrap().silenced_until > r.now);
        let to = r.enemy_pos(a).unwrap();
        assert!((to.0 - at.0 - 4.0).abs() < 0.01, "thrown straight away from the hero: {at:?} -> {to:?}");
    }

    #[test]
    fn combo_points_come_only_with_a_combo_resource() {
        let m = Milestone::ComboPoints { points: 2, trigger: Trigger::Hit, filter: fb() };
        let mut r = run_as("rogue", vec![m.clone()]);
        let a = enemy(&mut r, 3.0);
        r.hit_enemy(Some(0), a, 1.0, false, spell("fireball"));
        assert_eq!(r.heroes[0].kit.combo, 2);
        let mut r = run(vec![m]);
        let a = enemy(&mut r, 3.0);
        r.hit_enemy(Some(0), a, 1.0, false, spell("fireball"));
        assert_eq!(r.heroes[0].kit.combo, 0);
    }

    /// Every legendary amulet, worn by a hero of its class who knows the spell it remakes, plays
    /// half a minute of a run.
    #[test]
    fn every_legendary_plays() {
        let content = content();
        for l in &content.legendary_amulets {
            let main = l.main_spell();
            let (class, option) = content
                .classes
                .iter()
                .find_map(|c| {
                    let o = c
                        .starting_options
                        .iter()
                        .find(|o| o.spells.iter().any(|s| s == main))
                        .or_else(|| c.starting_options.iter().find(|o| c.card_pool(Some(&o.id)).iter().any(|s| s == main)))?;
                    Some((c.id.clone(), o.id.clone()))
                })
                .unwrap_or_else(|| panic!("no hero knows {main}"));
            let race = match class.as_str() {
                "druid" => "night_elf",
                "shaman" => "orc",
                _ => "human",
            };
            let mut setup = HeroSetup { class: class.clone(), race: race.into(), start: Some(option), ..Default::default() };
            setup.add_effects(l.effects.iter().map(|e| (e, 1)));
            setup.milestones.push(Milestone::StartWith { spell: main.to_string() });
            setup.milestones.push(Milestone::CheatDeath { cooldown: 0.0, health: 1.0, immune_seconds: 0.0 });
            let mut r = Run::new(content.clone(), "elwynn", RunOptions::default(), &[setup], Box::new(Flat), 31).unwrap();
            let (id, p) = (r.heroes[0].id, r.heroes[0].pos);
            r.hero_moved(id, (p.0 + 1.0, p.1), 0.0, 0.0);
            while r.now < 30.0 {
                r.tick(0.05);
                r.take_first_offers();
                r.drain_events();
            }
            assert!(r.heroes[0].spells.knows(main), "{}: knows {main}", l.name);
        }
    }

    #[test]
    fn a_heal_cast_burns_the_three_nearest_for_a_share_of_its_healing() {
        let m = Milestone::DotFromHeal { share: 0.5, seconds: 10.0, targets: 3, reach: 30.0, order: None, look_spell: Some(15286), filter: SpellFilter::Spell("renew".into()) };
        let renew = content().ability("renew").unwrap().clone();
        // no enemies: nothing happens
        let mut r = run(vec![m.clone()]);
        r.heal_after(0, &renew, 60.0);
        assert!(r.enemies.is_empty());
        // four enemies: the three nearest each get 30 over 10 ticks, the farthest none
        let mut r = run(vec![m]);
        let ids: Vec<u32> = [12.0, 3.0, 9.0, 6.0].iter().map(|&d| enemy(&mut r, d)).collect();
        r.heal_after(0, &renew, 60.0);
        for (k, id) in ids.iter().enumerate() {
            let dots: Vec<&Dot> = r.enemies.iter().find(|e| e.id == *id).unwrap().dots.iter().filter(|d| d.spell == "aura:15286").collect();
            if k == 0 {
                assert!(dots.is_empty(), "the farthest is spared");
            } else {
                assert_eq!(dots.len(), 1);
                assert_eq!(dots[0].ticks_left, 10);
                assert!((dots[0].per_tick * dots[0].ticks_left as f32 - 30.0).abs() < 1e-3);
            }
        }
        // another spell's heal sets off nothing
        let mut r = run(vec![Milestone::DotFromHeal { share: 0.5, seconds: 10.0, targets: 3, reach: 30.0, order: None, look_spell: None, filter: SpellFilter::Spell("renew".into()) }]);
        let a = enemy(&mut r, 3.0);
        let wave = r.content.ability("healing_wave").unwrap().clone();
        r.heal_after(0, &wave, 60.0);
        assert!(r.enemies.iter().find(|e| e.id == a).unwrap().dots.is_empty());
    }

    #[test]
    fn a_heal_burns_bosses_elites_and_rares_first_when_asked() {
        let order = Some(esurv_content::TargetOrder::BossesFirst);
        let m = Milestone::DotFromHeal { share: 0.5, seconds: 10.0, targets: 3, reach: 30.0, order, look_spell: Some(15286), filter: SpellFilter::Spell("renew".into()) };
        let renew = content().ability("renew").unwrap().clone();
        let mut r = run(vec![m]);
        let ids: Vec<u32> = [3.0, 6.0, 9.0, 20.0].iter().map(|&d| enemy(&mut r, d)).collect();
        r.enemies.iter_mut().find(|e| e.id == ids[3]).unwrap().elite = true;
        r.heal_after(0, &renew, 60.0);
        let burnt = |id: u32| r.enemies.iter().find(|e| e.id == id).unwrap().dots.iter().any(|d| d.spell == "aura:15286");
        assert!(burnt(ids[3]) && burnt(ids[0]) && burnt(ids[1]));
        assert!(!burnt(ids[2]), "the elite takes the third-nearest's place");
    }

    #[test]
    fn cast_on_cooldown_casts_a_heal_at_full_health_with_an_enemy_near() {
        let on_cooldown = Milestone::CastOnCooldown { filter: SpellFilter::Spell("renew".into()) };
        // without it, Renew waits for low health
        let mut r = run(vec![]);
        enemy(&mut r, 5.0);
        assert!(!r.free_cast(0, "renew"));
        // with it, Renew goes whenever an enemy is near...
        let mut r = run(vec![on_cooldown.clone()]);
        enemy(&mut r, 5.0);
        assert!(r.free_cast(0, "renew"));
        // ...and not with none near
        let mut r = run(vec![on_cooldown]);
        enemy(&mut r, 50.0);
        assert!(!r.free_cast(0, "renew"));
    }
}
