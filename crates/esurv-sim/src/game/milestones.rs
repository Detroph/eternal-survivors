//! Milestone talents and amulet effects inside a run: combat events (a hit, a crit, a tick, a
//! kill, a cast, a melee blow taken, damage taken, a shield breaking) set them off when their
//! trigger and spell filter match. Each kind also sets how it shows (the spell looks it plays and
//! the combat-log lines of its damage).

use esurv_content::{Ability, AfflictEffect, Mechanic, Milestone, SpellEffect, School, SpellFilter, Trigger};

use super::casting::{Dot, HitSource, ProcOrigin};
use super::enemy_ai::Vec2;
use super::run::{Aim, Event, Run, Unit, AMMO_SPELLS};
use super::spells::{cooldown, MIN_COOLDOWN};
use super::stats::Modifier;

/// Procs may set off further procs only this deep (a guard against endless chains); spells cast
/// by milestones set off none at all.
pub const MAX_PROC_DEPTH: u32 = 3;
/// A damage-over-time milestone ticks once a second.
pub const DOT_ON_HIT_TICK: f32 = 1.0;
/// A spell a milestone casts by itself every few seconds tries again this soon when its cast
/// found nothing to do.
pub const PERIODIC_RETRY: f32 = 0.3;
/// A death explosion with no look of its own still names this spell (`explosive_trap`'s) on its
/// damage lines.
pub const DEATH_EXPLOSION_SPELL: u32 = 13812;
/// Healing strikes go off at most once in this many seconds...
pub const HEALING_STRIKES_PULSE: f32 = 1.0;
/// ...and only once at least this much healing has added up since the last time.
pub const HEALING_STRIKES_MIN_HEALING: f32 = 1.0;
/// Healing strikes' damage lines name this spell (`tranquility`'s), in this school.
pub const HEALING_STRIKES_SPELL: u32 = 740;
pub const HEALING_STRIKES_SCHOOL: School = School::Nature;
/// A sweep reaches enemies this near the struck one; an effect jumps on death this far.
pub const SWEEP_REACH: f32 = 10.0;
pub const SPREAD_REACH: f32 = 10.0;
/// Effects over time that milestones put on enemies are named this, then their aura's spell.
pub(crate) const AURA_PREFIX: &str = "aura:";

/// Something that happened in combat, for milestones to react to.
#[derive(Debug, Clone, PartialEq)]
pub enum CombatEvent {
    /// A hit on an enemy (`at`: where the enemy stood).
    Hit { target: u32, at: Vec2, source: HitSource, crit: bool, dealt: f32 },
    /// A damage-over-time tick.
    Tick { target: u32, at: Vec2, spell: String, dealt: f32 },
    /// An enemy died by `source`, carrying the effects over time `dots`.
    Kill { at: Vec2, source: HitSource, max_health: f32, dots: Vec<Dot> },
    /// A spell's own timed cast.
    Cast { spell: String },
    /// An enemy's melee blow struck the hero for `taken` (after damage reduction, before the
    /// shield took any).
    StruckInMelee { by: u32, taken: f32 },
    /// An enemy's melee blow at the hero was blocked.
    Blocked { by: u32 },
    HeroDamaged { amount: f32 },
    ShieldBroken { absorbed: f32 },
    /// The buff `spell` left on the hero ended (its time ran out, or its last charge went).
    BuffEnded { spell: String },
    /// An effect over time ran out on a living enemy (not one that was renewed or died).
    DotEnded { target: u32, at: Vec2, dot: Dot },
}

/// Puts look `look` into `worn` until `until` (keeping the later end when it is there), the
/// looks that ended dropped.
fn wear(worn: &mut Vec<(u32, f32)>, look: u32, until: f32, now: f32) {
    worn.retain(|(_, u)| *u > now);
    match worn.iter_mut().find(|(l, _)| *l == look) {
        Some(w) => w.1 = w.1.max(until),
        None => worn.push((look, until)),
    }
}

/// What a milestone remembers during a run.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MilestoneState {
    /// `EveryNth`: events counted so far.
    pub counter: u32,
    /// Internal cooldowns: ready again from then.
    pub ready_at: f32,
    /// `TimedBuff`: stacks and when they run out.
    pub stacks: u32,
    pub until: f32,
    /// `Periodic`: the next cast.
    pub next: f32,
    /// `HealingStrikes`: healing added up since the last strike.
    pub healing: f32,
    /// `FasterSwings`: the faster swings left.
    pub swings_left: u32,
    /// `Every` counting casts: the last counted cast was the one whose hits go off.
    pub armed: bool,
}

impl Run {
    /// Whether `filter` lets a hit from `source` through.
    pub(crate) fn filter_passes(&self, filter: &SpellFilter, source: &HitSource) -> bool {
        match (filter, source) {
            (SpellFilter::Any, _) => true,
            (SpellFilter::Pets, HitSource::Pet(_)) => true,
            (SpellFilter::WeaponSwing, HitSource::WeaponSwing) => true,
            (SpellFilter::Look(n), HitSource::Proc { spell, .. }) => spell == n,
            (SpellFilter::Look(n), HitSource::Spell(id) | HitSource::Tick(id)) => match self.content.ability(id) {
                Some(a) => SpellFilter::Look(*n).matches(a),
                None => id.strip_prefix(AURA_PREFIX).and_then(|x| x.parse::<u32>().ok()) == Some(*n),
            },
            (f, HitSource::Spell(id) | HitSource::Tick(id)) => self.content.ability(id).is_some_and(|a| f.matches(a)),
            _ => false,
        }
    }

    /// What event `e` counts as for spell filters (a tick counts as its spell). Inside a proc, a
    /// hit shown as a weapon swing or a pet's swing is a milestone's own blow: it counts as other
    /// damage, like every milestone hit, so it never passes a swing or pet filter.
    fn event_source(&self, e: &CombatEvent) -> HitSource {
        let source = match e {
            CombatEvent::Hit { source, .. } | CombatEvent::Kill { source, .. } => match source {
                HitSource::Tick(s) => HitSource::Spell(s.clone()),
                other => other.clone(),
            },
            CombatEvent::Tick { spell, .. } | CombatEvent::Cast { spell } | CombatEvent::BuffEnded { spell } => HitSource::Spell(spell.clone()),
            CombatEvent::DotEnded { dot, .. } => HitSource::Spell(dot.spell.clone()),
            _ => HitSource::Other,
        };
        match source {
            HitSource::WeaponSwing | HitSource::Pet(_) if self.proc_depth > 0 => HitSource::Other,
            other => other,
        }
    }

    /// The spell a hit from `source` names in the combat log, and its school: a spell's look (a
    /// totem's pulse spell), a ranged pet's spell, a proc's own spell; 0 (nothing) for weapon
    /// swings, melee pets and anything else. Milestones repeat it on the damage they add.
    pub(crate) fn shown_spell(&self, source: &HitSource) -> (u32, School) {
        match source {
            HitSource::Spell(id) | HitSource::Tick(id) => match self.content.ability(id) {
                Some(a) => (a.pulse_spell.unwrap_or(a.look_spell), a.school),
                // an effect over time a milestone added: its aura's spell (its school is not kept)
                None => (id.strip_prefix(AURA_PREFIX).and_then(|n| n.parse().ok()).unwrap_or(0), School::Physical),
            },
            HitSource::Pet(id) => self
                .pets
                .iter()
                .find(|p| p.id == *id)
                .and_then(|p| self.content.pets.iter().find(|x| x.name == p.pet))
                .map_or((0, School::Physical), |p| (p.ranged_spell.unwrap_or(0), p.school)),
            HitSource::Proc { spell, school, .. } => (*spell, *school),
            HitSource::WeaponSwing | HitSource::Enemy(_) | HitSource::Other => (0, School::Physical),
        }
    }

    /// How a sweep or strike-again blow on top of a hit from `source` shows: a damage line with
    /// the hit's spell when it named one; else, after a weapon swing or a pet's melee hit, a melee
    /// swing (from hero `h`'s first pet when a pet's hit set it off and one is there, else from
    /// the hero); anything else stays silent.
    pub(crate) fn repeat_source(&self, h: usize, source: &HitSource) -> HitSource {
        let (spell, school) = self.shown_spell(source);
        match source {
            _ if spell != 0 => self.proc_source(spell, school),
            HitSource::Pet(_) => self.pets.iter().find(|p| p.hero == h).map_or(HitSource::WeaponSwing, |p| HitSource::Pet(p.id)),
            HitSource::WeaponSwing => HitSource::WeaponSwing,
            _ => self.proc_source(0, school),
        }
    }

    /// A milestone's look: `spell` from hero `h`, striking `hits`, aimed at `aim`; nothing when
    /// the milestone has no look (0).
    pub(crate) fn show_proc(&mut self, h: usize, spell: u32, hits: &[u32], aim: Aim, reach: f32) {
        if spell == 0 {
            return;
        }
        let caster = Unit::Hero(self.heroes[h].id);
        let hits = hits.iter().map(|&id| Unit::Enemy(id)).collect();
        self.push(Event::SpellGo { caster, spell, hits, aim, ammo: AMMO_SPELLS.contains(&spell), reach });
    }

    pub(crate) fn trigger_fires(&self, h: usize, trigger: &Trigger, e: &CombatEvent) -> bool {
        match (trigger, e) {
            (Trigger::Hit, CombatEvent::Hit { .. }) => true,
            (Trigger::Crit, CombatEvent::Hit { crit, .. }) => *crit,
            (Trigger::DotTick, CombatEvent::Tick { .. }) => true,
            (Trigger::HitOrTick, CombatEvent::Hit { .. } | CombatEvent::Tick { .. }) => true,
            (Trigger::Kill, CombatEvent::Kill { .. }) => true,
            (Trigger::Cast, CombatEvent::Cast { .. }) => true,
            (Trigger::StruckInMelee, CombatEvent::StruckInMelee { .. }) => true,
            (Trigger::HealthBelow(share), CombatEvent::HeroDamaged { .. }) => {
                let hero = &self.heroes[h];
                hero.health < hero.stats.max_health * share
            }
            (Trigger::ShieldBreak, CombatEvent::ShieldBroken { .. }) => true,
            (Trigger::Block, CombatEvent::Blocked { .. }) => true,
            (Trigger::BuffEnds, CombatEvent::BuffEnded { .. }) => true,
            _ => false,
        }
    }

    /// Runs hero `h`'s milestones against event `e`.
    pub(crate) fn milestone_event(&mut self, h: usize, e: CombatEvent) {
        if self.proc_depth >= MAX_PROC_DEPTH || h >= self.heroes.len() || !self.heroes[h].alive {
            return;
        }
        let source = self.event_source(&e);
        self.proc_depth += 1;
        let milestones = self.heroes[h].milestones.clone();
        // each milestone's proc hits carry what gave it (a proc inside a proc puts it back)
        let outer = self.proc_origin.clone();
        for (k, m) in milestones.iter().enumerate() {
            self.proc_origin = self.origin_of(h, k);
            match m {
                Milestone::Every { .. } => self.every_event(h, k, m, &e, &source),
                Milestone::WhileBuff { .. } => self.while_buff_event(h, k, m, &e, &source),
                _ => {
                    if self.goes_off(h, m, &e, &source) {
                        self.milestone_fires(h, k, m, &e, &source);
                    }
                }
            }
        }
        self.proc_origin = outer;
        self.proc_depth -= 1;
    }

    /// What gave hero `h` its milestone `k` (`ProcOrigin::Other` when nothing is known).
    pub fn origin_of(&self, h: usize, k: usize) -> ProcOrigin {
        self.heroes.get(h).and_then(|x| x.milestone_origins.get(k)).cloned().unwrap_or_default()
    }

    /// A proc hit shown as `spell` in `school`, from the milestone going off now.
    pub(crate) fn proc_source(&self, spell: u32, school: School) -> HitSource {
        HitSource::Proc { spell, school, origin: self.proc_origin.clone() }
    }

    /// What gave hero `h` the effect over time `spell` a milestone put on an enemy
    /// (`aura:<look>`, a `DotOnHit` or `DotFromHeal`): the first such milestone with that look.
    pub fn aura_origin(&self, h: usize, spell: &str) -> Option<(u32, ProcOrigin)> {
        let look: u32 = spell.strip_prefix(AURA_PREFIX)?.parse().ok()?;
        fn has_look(m: &Milestone, look: u32) -> bool {
            match m {
                Milestone::DotOnHit { look_spell, .. } => *look_spell == look,
                Milestone::DotFromHeal { look_spell, .. } => look_spell.unwrap_or(0) == look,
                Milestone::Every { then, .. } | Milestone::WhileBuff { then, .. } => then.iter().any(|x| has_look(x, look)),
                _ => false,
            }
        }
        let k = self.heroes.get(h)?.milestones.iter().position(|m| has_look(m, look))?;
        Some((look, self.origin_of(h, k)))
    }

    /// Whether milestone `m` of hero `h` goes off on event `e` (which counts as `source`).
    pub(crate) fn goes_off(&self, h: usize, m: &Milestone, e: &CombatEvent, source: &HitSource) -> bool {
        {
            let fires = match m {
                // a summon from a dead enemy that carried the effect, whatever killed it
                Milestone::PetOnTrigger { trigger: Trigger::Kill, filter, afflicted: true, .. } => match &e {
                    CombatEvent::Kill { dots, .. } => dots.iter().any(|d| self.filter_passes(filter, &HitSource::Spell(d.spell.clone()))),
                    _ => false,
                },
                Milestone::MarkAttackers { .. } => matches!(e, CombatEvent::StruckInMelee { .. }),
                Milestone::FearedTurn { filter, .. } => matches!(e, CombatEvent::Hit { .. }) && self.filter_passes(filter, &source),
                Milestone::FreeCast { trigger, filter, .. }
                | Milestone::EveryNth { trigger, filter, .. }
                | Milestone::PetOnTrigger { trigger, filter, .. }
                | Milestone::LineHit { trigger, filter, .. }
                | Milestone::ShieldBurst { trigger, filter, .. }
                | Milestone::Splash { trigger, filter, .. }
                | Milestone::ExtraTargets { trigger, filter, .. }
                | Milestone::StrikeAgain { trigger, filter, .. }
                | Milestone::Afflict { trigger, filter, .. }
                | Milestone::DotOnHit { trigger, filter, .. }
                | Milestone::TimedBuff { trigger, filter, .. }
                | Milestone::HealOnTrigger { trigger, filter, .. }
                | Milestone::ResourceOnTrigger { trigger, filter, .. }
                | Milestone::FasterSwings { trigger, filter, .. }
                | Milestone::CooldownOnTrigger { trigger, filter, .. }
                | Milestone::ResetOnTrigger { trigger, filter, .. }
                | Milestone::StrikeOne { trigger, filter, .. }
                | Milestone::Burst { trigger, filter, .. }
                | Milestone::DotSpread { trigger, filter, .. }
                | Milestone::LeaveArea { trigger, filter, .. }
                | Milestone::Barrage { trigger, filter, .. }
                | Milestone::ApplyPoison { trigger, filter, .. }
                | Milestone::ComboPoints { trigger, filter, .. } => {
                    self.trigger_fires(h, trigger, e)
                        && (matches!(trigger, Trigger::StruckInMelee | Trigger::HealthBelow(_) | Trigger::ShieldBreak | Trigger::Block)
                            || self.filter_passes(filter, source))
                }
                Milestone::ResetOnKill { filter, .. } | Milestone::SpreadOnDeath { filter, .. } => {
                    matches!(e, CombatEvent::Kill { .. }) && (matches!(m, Milestone::SpreadOnDeath { .. }) || self.filter_passes(filter, source))
                }
                Milestone::ExplodeOnDeath { afflicted, filter, .. } => match e {
                    CombatEvent::Kill { dots, .. } if *afflicted => dots.iter().any(|d| self.filter_passes(filter, &HitSource::Spell(d.spell.clone()))),
                    CombatEvent::Kill { .. } => self.filter_passes(filter, source),
                    _ => false,
                },
                Milestone::DotBurst { on_end, on_death, filter, .. } => match e {
                    CombatEvent::DotEnded { dot, .. } => *on_end && self.filter_passes(filter, &HitSource::Tick(dot.spell.clone())),
                    CombatEvent::Kill { dots, .. } => *on_death && dots.iter().any(|d| self.filter_passes(filter, &HitSource::Tick(d.spell.clone()))),
                    _ => false,
                },
                Milestone::CastAgain { filter, .. } => matches!(e, CombatEvent::Cast { .. }) && self.filter_passes(filter, source),
                Milestone::Retaliate { .. } => matches!(e, CombatEvent::StruckInMelee { .. }),
                _ => false,
            };
            fires
        }
    }

    pub(crate) fn state(&mut self, h: usize, k: usize) -> &mut MilestoneState {
        let s = &mut self.heroes[h].milestone_state;
        if s.len() <= k {
            s.resize(k + 1, MilestoneState::default());
        }
        &mut s[k]
    }

    /// Milestone `k` of hero `h` goes off on event `e` (which counts as `source`).
    pub(crate) fn milestone_fires(&mut self, h: usize, k: usize, m: &Milestone, e: &CombatEvent, source: &HitSource) {
        let now = self.now;
        let (target, at, dealt) = match e {
            CombatEvent::Hit { target, at, dealt, .. } | CombatEvent::Tick { target, at, dealt, .. } => (Some(*target), *at, *dealt),
            CombatEvent::Kill { at, .. } => (None, *at, 0.0),
            _ => (None, self.heroes[h].pos, 0.0),
        };
        match m {
            Milestone::FreeCast { chance, cooldown, spell, .. } => {
                if now < self.state(h, k).ready_at || self.rng.unit() >= *chance {
                    return;
                }
                self.state(h, k).ready_at = now + cooldown;
                let id = if spell.is_empty() {
                    match source {
                        HitSource::Spell(s) => s.clone(),
                        _ => return,
                    }
                } else {
                    spell.clone()
                };
                self.free_cast(h, &id);
            }
            Milestone::EveryNth { count, spell, .. } => {
                let st = self.state(h, k);
                st.counter += 1;
                if st.counter >= *count {
                    st.counter = 0;
                    let spell = spell.clone();
                    self.free_cast(h, &spell);
                }
            }
            Milestone::PetOnTrigger { chance, seconds, max, pet, count, look_spell, .. } => {
                let alive = self.pets.iter().filter(|p| p.hero == h && &p.pet == pet).count() as u32;
                if alive < *max && self.rng.unit() < *chance {
                    let before = self.pets.len();
                    // as many as it calls, never past `max` at once
                    for _ in 0..(*count).max(1).min(max - alive) {
                        self.summon_pet(h, pet, Some(now + seconds));
                    }
                    // a legendary's look where each one appears
                    let spots: Vec<Vec2> = self.pets[before.min(self.pets.len())..].iter().map(|p| p.segment.position(now)).collect();
                    for at in spots {
                        self.show_proc(h, look_spell.unwrap_or(0), &[], Aim::Ground(at), 0.0);
                    }
                }
            }
            Milestone::LineHit { every, share, length, width, behind, hits_target, look_spell, .. } => {
                let Some(target) = target else { return };
                let st = self.state(h, k);
                st.counter += 1;
                if st.counter < (*every).max(1) {
                    return;
                }
                st.counter = 0;
                let line = super::field_milestones::Line { length: *length, width: *width, behind: *behind, hits_target: *hits_target };
                self.line_hit(h, target, at, share * dealt, line, *look_spell, source);
            }
            Milestone::MarkAttackers { seconds, look_spell, .. } => {
                if let CombatEvent::StruckInMelee { by, .. } = e {
                    if let Some(x) = self.enemies.iter_mut().find(|x| x.id == *by) {
                        x.kit.attacker_mark = Some((h, now + seconds, look_spell.unwrap_or(0)));
                    }
                }
            }
            Milestone::FearedTurn { chance, look_spell, .. } => {
                let Some(target) = target else { return };
                self.maybe_turn(h, target, *chance, source);
                // a turned enemy wears the legendary's look while it fights for the hero
                let turned = self.enemy(target).and_then(|x| x.kit.turned).filter(|(by, until)| *by == h && *until > now);
                if let (Some(look), Some((_, until))) = (*look_spell, turned) {
                    self.enemy_wears_look(target, look, until);
                }
            }
            Milestone::ShieldBurst { school, share, radius, look_spell, .. } => {
                if let CombatEvent::ShieldBroken { absorbed } = e {
                    // the look goes off from the hero onto everything in reach, even when that is
                    // nothing; the damage lines name the same spell in the shield's school
                    let struck = self.enemies_within(self.heroes[h].pos, *radius);
                    self.show_proc(h, look_spell.unwrap_or(0), &struck, Aim::Caster, *radius);
                    let shown = self.proc_source(look_spell.unwrap_or(0), school.unwrap_or(School::Physical));
                    let hit = share * absorbed * self.heroes[h].stats.damage;
                    for t in struck {
                        self.hit_enemy(Some(h), t, hit, false, shown.clone());
                    }
                }
            }
            Milestone::Splash { share, radius, look_spell, requires_aura, effects, .. } => {
                let Some(target) = target else { return };
                if let Some(aura) = *requires_aura {
                    if !self.carries_aura(target, aura) {
                        return;
                    }
                }
                // the look goes off at the struck enemy's spot; the splash hits with the struck
                // hit's own spell and school (silent after a swing)
                self.show_proc(h, look_spell.unwrap_or(0), &[], Aim::Ground(at), *radius);
                let (spell, school) = self.shown_spell(source);
                // each splashed enemy also gets the struck hit's own effect
                let effect = match source {
                    HitSource::Spell(a) | HitSource::Tick(a) => self.content.ability(a).map(|a| a.effect),
                    _ => None,
                };
                for t in self.enemies_within(at, *radius).into_iter().filter(|&t| t != target) {
                    let landed = self.hit_enemy(Some(h), t, share * dealt, false, self.proc_source(spell, school));
                    if let (Some(d), Some(effect)) = (landed, &effect) {
                        self.apply_on_hit(h, t, effect, d);
                    }
                    if let Some(d) = landed {
                        for x in effects {
                            self.afflict(h, Some(t), x, d);
                        }
                    }
                }
            }
            Milestone::ExtraTargets { count, share, per_swing_second, growth, reach, effects, look_spell, .. } => {
                let Some(target) = target else { return };
                // a swing's sweep per second of the weapon's swing time: slow weapons sweep for more
                let share = match (per_swing_second, source) {
                    (true, HitSource::WeaponSwing) => {
                        share * super::kit::swing_of(&self.content, &self.heroes[h]).map_or(1.0, |s| s.swing)
                    }
                    _ => *share,
                };
                let reach = if *reach > 0.0 { *reach } else { SWEEP_REACH };
                let near: Vec<u32> = self.nearest_to(at, reach).into_iter().filter(|&t| t != target).take(*count as usize).collect();
                // after a spell, its look goes onto each swept enemy before the damage line
                let shown = self.repeat_source(h, source);
                for (n, t) in near.into_iter().enumerate() {
                    // a legendary's own look in its place
                    match (look_spell, &shown) {
                        (Some(l), _) => self.show_proc(h, *l, &[t], Aim::Unit(Unit::Enemy(t)), 0.0),
                        (None, HitSource::Proc { spell, .. }) => self.show_proc(h, *spell, &[t], Aim::Unit(Unit::Enemy(t)), 0.0),
                        _ => {}
                    }
                    // each further enemy `growth` stronger than the one before
                    let hit = share * dealt * (1.0 + growth).powi(n as i32);
                    if let Some(d) = self.hit_enemy(Some(h), t, hit, false, shown.clone()) {
                        for x in effects {
                            self.afflict(h, Some(t), x, d);
                        }
                    }
                }
            }
            Milestone::StrikeAgain { chance, times, share, look_spell, .. } => {
                let Some(target) = target else { return };
                if self.rng.unit() < *chance {
                    // no look but a legendary's own: only more damage
                    // lines (or melee swings)
                    self.show_proc(h, look_spell.unwrap_or(0), &[target], Aim::Unit(Unit::Enemy(target)), 0.0);
                    let shown = self.repeat_source(h, source);
                    for _ in 0..*times {
                        self.hit_enemy(Some(h), target, share * dealt, false, shown.clone());
                    }
                }
            }
            Milestone::Afflict { chance, effect, look_spell, .. } => {
                if self.rng.unit() >= *chance {
                    return;
                }
                self.afflict(h, target, effect, dealt);
                if let (Some(look), Some(t)) = (*look_spell, target) {
                    self.afflict_look(h, t, effect, look);
                }
            }
            Milestone::DotOnHit { look_spell, share, seconds, .. } => {
                let Some(target) = target else { return };
                let ticks = (seconds / DOT_ON_HIT_TICK).round().max(1.0) as u32;
                let spell = format!("{AURA_PREFIX}{look_spell}");
                if let Some(t) = self.enemies.iter_mut().find(|x| x.id == target) {
                    // one such effect per enemy: a new one takes over what the old one had
                    // still to deal, and starts its ticks afresh
                    let left: f32 = t.dots.iter().filter(|d| d.spell == spell).map(|d| d.per_tick * d.ticks_left as f32).sum();
                    t.dots.retain(|d| d.spell != spell);
                    t.dots.push(Dot {
                        hero: h,
                        spell,
                        per_tick: (share * dealt + left) / ticks as f32,
                        ticks_left: ticks,
                        interval: DOT_ON_HIT_TICK,
                        next: now + DOT_ON_HIT_TICK,
                        from_proc: false,
                        leech: 0.0,
                    });
                }
            }
            Milestone::ResetOnKill { min_interval, .. } => {
                let HitSource::Spell(spell) = source else { return };
                if now < self.state(h, k).ready_at {
                    return;
                }
                self.state(h, k).ready_at = now + min_interval;
                if let Some(slot) = self.heroes[h].spells.known.iter_mut().find(|s| s.spell == *spell) {
                    slot.timer = 0.0;
                }
            }
            Milestone::ExplodeOnDeath { share, radius, look_spell, .. } => {
                if let CombatEvent::Kill { at, max_health, .. } = e {
                    // the look goes off at the dead enemy's spot; the damage lines name it (or
                    // `explosive_trap`'s spell when there is none) in the killing hit's school
                    self.show_proc(h, look_spell.unwrap_or(0), &[], Aim::Ground(*at), *radius);
                    let spell = look_spell.unwrap_or(DEATH_EXPLOSION_SPELL);
                    let (_, school) = self.shown_spell(source);
                    for t in self.enemies_within(*at, *radius) {
                        self.hit_enemy(Some(h), t, share * max_health, false, self.proc_source(spell, school));
                    }
                }
            }
            Milestone::SpreadOnDeath { count, filter, look_spell } => {
                if let CombatEvent::Kill { at, dots, .. } = e {
                    // a drain goes on to the next enemy by itself (`kit_on_kill`); copied as well,
                    // the copies would merge back into the drain as they die and be copied again,
                    // its time left growing without end
                    let is_drain = |d: &Dot| self.content.ability(&d.spell).is_some_and(|a| matches!(a.kit.mechanic, Some(Mechanic::Drain)));
                    let spread: Vec<Dot> = dots.iter().filter(|d| !is_drain(d) && self.filter_passes(filter, &HitSource::Spell(d.spell.clone()))).cloned().collect();
                    // each effect jumps onto the nearest few close by that lack it, showing its
                    // spell going onto each
                    for d in &spread {
                        let lacking: Vec<u32> = self
                            .nearest_to(*at, SPREAD_REACH)
                            .into_iter()
                            .filter(|&t| self.enemies.iter().find(|e| e.id == t).is_some_and(|e| !e.dots.iter().any(|x| x.spell == d.spell && x.ticks_left > 0)))
                            .take(*count as usize)
                            .collect();
                        for t in lacking {
                            if let Some(enemy) = self.enemies.iter_mut().find(|x| x.id == t) {
                                enemy.dots.push(Dot { next: now + d.interval, ..d.clone() });
                            }
                            let (spell, _) = self.shown_spell(&HitSource::Tick(d.spell.clone()));
                            self.show_proc(h, look_spell.unwrap_or(spell), &[t], Aim::Unit(Unit::Enemy(t)), 0.0);
                        }
                    }
                }
            }
            Milestone::TimedBuff { max_stacks, seconds, look_spell, .. } => {
                let st = self.state(h, k);
                st.stacks = if now < st.until { (st.stacks + 1).min(*max_stacks) } else { 1 };
                st.until = now + seconds;
                self.refresh_stats(h);
                if let Some(look) = *look_spell {
                    self.hero_wears_look(h, look, now + seconds);
                }
            }
            Milestone::HealOnTrigger { share, cooldown, look_spell, .. } => {
                if now < self.state(h, k).ready_at {
                    return;
                }
                self.state(h, k).ready_at = now + cooldown;
                let max = self.heroes[h].stats.max_health;
                self.heal_hero(h, share * max, 0);
                if let Some(look) = *look_spell {
                    self.show_on_hero(h, look);
                }
            }
            Milestone::ResourceOnTrigger { amount, .. } => self.gain_resource(h, *amount),
            Milestone::FasterSwings { swings, .. } => self.state(h, k).swings_left = *swings,
            Milestone::CooldownOnTrigger { seconds, spells, every, .. } => {
                let now = self.now;
                if now < self.state(h, k).ready_at {
                    return;
                }
                self.state(h, k).ready_at = now + every;
                for slot in self.heroes[h].spells.known.iter_mut().filter(|s| spells.contains(&s.spell)) {
                    slot.timer = (slot.timer - seconds).max(0.0);
                }
            }
            other => self.proc_effect_fires(h, k, other, e, source),
        }
    }

    /// A legendary's look for an `Afflict` of `effect` on enemy `t`: worn while the effect lasts,
    /// played on it for one with no time (a knockback).
    fn afflict_look(&mut self, h: usize, t: u32, effect: &AfflictEffect, look: u32) {
        let seconds = match *effect {
            AfflictEffect::Stun { seconds }
            | AfflictEffect::Fear { seconds }
            | AfflictEffect::Silence { seconds }
            | AfflictEffect::Root { seconds }
            | AfflictEffect::Slow { seconds, .. }
            | AfflictEffect::Vulnerable { seconds, .. } => seconds,
            AfflictEffect::Knockback { .. } => 0.0,
            AfflictEffect::HealFromDamage { .. } | AfflictEffect::HealMaxHealth { .. } => return,
        };
        if seconds > 0.0 {
            let until = self.now + seconds;
            self.enemy_wears_look(t, look, until);
        } else {
            self.show_proc(h, look, &[t], Aim::Unit(Unit::Enemy(t)), 0.0);
        }
    }

    /// Enemy `t` wears legendary look `look` until `until` (the later time when it wears it
    /// already).
    pub(crate) fn enemy_wears_look(&mut self, t: u32, look: u32, until: f32) {
        let now = self.now;
        if let Some(x) = self.enemies.iter_mut().find(|x| x.id == t) {
            wear(&mut x.kit.worn_looks, look, until, now);
        }
    }

    /// Hero `h` wears legendary look `look` until `until` (the later time when it wears it
    /// already).
    pub(crate) fn hero_wears_look(&mut self, h: usize, look: u32, until: f32) {
        let now = self.now;
        wear(&mut self.heroes[h].kit.worn_looks, look, until, now);
    }

    /// Legendary look `look` plays on hero `h` (a heal's look on the hero).
    pub(crate) fn show_on_hero(&mut self, h: usize, look: u32) {
        let me = Unit::Hero(self.heroes[h].id);
        self.push(Event::SpellGo { caster: me, spell: look, hits: vec![me], aim: Aim::Caster, ammo: false, reach: 0.0 });
    }

    /// What an `Afflict` effect does to `target` (or for the hero) after a hit of `dealt`.
    pub(crate) fn afflict(&mut self, h: usize, target: Option<u32>, effect: &AfflictEffect, dealt: f32) {
        let now = self.now;
        let effect = match *effect {
            AfflictEffect::HealFromDamage { share } => return self.heal_hero(h, share * dealt, 0),
            AfflictEffect::HealMaxHealth { share } => {
                let max = self.heroes[h].stats.max_health;
                return self.heal_hero(h, share * max, 0);
            }
            AfflictEffect::Silence { seconds } => {
                if let Some(t) = target.and_then(|t| self.enemies.iter_mut().find(|x| x.id == t)) {
                    t.silenced_until = t.silenced_until.max(now + seconds);
                }
                return;
            }
            AfflictEffect::Vulnerable { amount, seconds } => SpellEffect::Vulnerable { amount, seconds },
            AfflictEffect::Slow { amount, seconds } => SpellEffect::Slow { amount, seconds },
            AfflictEffect::Root { seconds } => SpellEffect::Root { seconds },
            AfflictEffect::Stun { seconds } => SpellEffect::Stun { seconds },
            AfflictEffect::Fear { seconds } => SpellEffect::Fear { seconds },
            AfflictEffect::Knockback { yards } => {
                let from = self.heroes[h].pos;
                if let Some(t) = target.and_then(|t| self.enemies.iter_mut().find(|x| x.id == t)) {
                    if !t.is_boss() {
                        let p = t.position(now);
                        let d = ((p.0 - from.0).powi(2) + (p.1 - from.1).powi(2)).sqrt().max(0.01);
                        let to = (p.0 + (p.0 - from.0) / d * yards, p.1 + (p.1 - from.1) / d * yards);
                        t.segment = super::enemy_ai::Segment::standing(to, now);
                        t.next_decision = now;
                    }
                }
                return;
            }
        };
        if let Some(t) = target {
            self.apply_on_hit(h, t, &effect, dealt);
        }
    }

    fn carries_aura(&self, target: u32, aura: u32) -> bool {
        let tag = format!("{AURA_PREFIX}{aura}");
        self.enemies.iter().find(|e| e.id == target).is_some_and(|e| {
            e.dots.iter().any(|d| d.spell == tag || self.content.ability(&d.spell).is_some_and(|a| a.look_spell == aura))
        })
    }

    /// Casts `spell` for hero `h` for free: as its slot if known, else at the average card power;
    /// `false` when the cast found nothing to do.
    /// Nothing the cast does sets off milestones (procs do not proc again),
    /// unless the spell says `free_casts_can_proc`.
    pub(crate) fn free_cast(&mut self, h: usize, spell: &str) -> bool {
        let content = self.content.clone();
        let Some(a) = content.ability(spell) else { return false };
        let mods = self.heroes[h].spell_modifiers.clone();
        let inst = self.heroes[h].spells.free_cast(a, &mods);
        let quiet = u32::from(!a.free_casts_can_proc);
        self.casting_proc += quiet;
        let cast = self.cast(h, a, &inst);
        self.casting_proc -= quiet;
        cast
    }

    /// Nature's Grace: how many more summons this cast of `a` by hero `h` calls, counting the
    /// cast toward each `MoreSummonsEveryNth` that matches it.
    pub(crate) fn more_summons(&mut self, h: usize, a: &Ability) -> u32 {
        let found: Vec<(usize, u32, u32)> = self.heroes[h]
            .milestones
            .iter()
            .enumerate()
            .filter_map(|(k, m)| match m {
                Milestone::MoreSummonsEveryNth { count, extra, filter } if filter.matches(a) => Some((k, *count, *extra)),
                _ => None,
            })
            .collect();
        let mut more = 0;
        for (k, count, extra) in found {
            let s = self.state(h, k);
            s.counter += 1;
            if s.counter >= count.max(1) {
                s.counter = 0;
                more += extra;
            }
        }
        more
    }

    /// Pandemic: the crit bonus of hero `h`'s effects over time (0: they cannot crit).
    pub(crate) fn dot_crit_bonus(&self, h: usize) -> Option<f32> {
        self.heroes.get(h)?.milestones.iter().find_map(|m| if let Milestone::DotCrits { bonus } = m { Some(*bonus) } else { None })
    }

    /// How much faster hero `h`'s next swing comes from its `FasterSwings` milestones; each
    /// one with swings left uses one.
    pub(crate) fn take_faster_swing(&mut self, h: usize) -> f32 {
        let mut faster = 0.0;
        for k in 0..self.heroes[h].milestones.len() {
            let Milestone::FasterSwings { amount, .. } = self.heroes[h].milestones[k] else { continue };
            let st = self.state(h, k);
            if st.swings_left > 0 {
                st.swings_left -= 1;
                faster += amount;
            }
        }
        faster
    }

    /// Stat bonuses of a hero's running timed-buff milestones.
    pub(crate) fn milestone_buff_modifiers(&self, h: usize) -> Vec<Modifier> {
        let now = self.now;
        let hero = &self.heroes[h];
        hero.milestones
            .iter()
            .enumerate()
            .filter_map(|(k, m)| match m {
                Milestone::TimedBuff { stat, amount, .. } => {
                    let st = hero.milestone_state.get(k)?;
                    (now < st.until && st.stacks > 0).then(|| vec![Modifier::new(*stat, amount * st.stacks as f32)])
                }
                // a spree's stats while it lasts
                Milestone::Spree { stats, .. } => {
                    let st = hero.milestone_state.get(k)?;
                    (now < st.until && st.stacks > 0).then(|| stats.iter().map(|(s, v)| Modifier::new(*s, *v)).collect())
                }
                // a buff's stats while it is up
                Milestone::WhileBuff { spell, stats, .. } => {
                    self.buff_until(h, spell).map(|_| stats.iter().map(|(s, v)| Modifier::new(*s, *v)).collect())
                }
                _ => None,
            })
            .flatten()
            .collect()
    }

    /// The milestones the run tick drives: spells cast on their own every few seconds
    /// (`Periodic`), then healing strikes (`HealingStrikes`).
    pub(crate) fn periodic_milestones_tick(&mut self) {
        let now = self.now;
        let content = self.content.clone();
        for h in 0..self.heroes.len() {
            if !self.heroes[h].alive {
                continue;
            }
            let milestones = self.heroes[h].milestones.clone();
            for (k, m) in milestones.iter().enumerate() {
                if let Milestone::Spree { .. } = m {
                    self.spree_tick(h, k, m);
                    continue;
                }
                // a legendary's look worn while its buff is up
                if let Milestone::WhileBuff { spell, look_spell: Some(look), .. } = m {
                    if let Some(until) = self.buff_until(h, spell) {
                        self.hero_wears_look(h, *look, until);
                    }
                    continue;
                }
                let Milestone::Periodic { interval, spell } = m else { continue };
                let Some(a) = content.ability(spell) else { continue };
                if self.state(h, k).next == 0.0 {
                    self.state(h, k).next = now;
                }
                if now < self.state(h, k).next {
                    continue;
                }
                // its own interval, else the spell's cooldown as a cast would set it; either way
                // shortened by the hero's cooldown factor, never under the shortest cooldown. A
                // cast that found nothing to do tries again soon.
                let factor = self.heroes[h].stats.cooldown_factor;
                let inst = self.heroes[h].spells.free_cast(a, &self.heroes[h].spell_modifiers);
                let every = if *interval > 0.0 { (*interval * factor).max(MIN_COOLDOWN) } else { cooldown(a, &inst, factor) };
                let cast = self.free_cast(h, spell);
                self.state(h, k).next = now + if cast { every } else { PERIODIC_RETRY };
            }
        }
        self.healing_strikes_tick();
        self.fire_trails_tick();
        self.proc_effects_tick();
    }

    /// Healing strikes: at most once a second, once at least 1 healing has added up, every enemy
    /// within the radius takes `share` × the healing added up since the last strike, as
    /// `tranquility`'s spell in the Nature school, never a crit. The only healing-strikes record
    /// (`tranquility`) has no look of its own, so no spell goes off.
    fn healing_strikes_tick(&mut self) {
        let now = self.now;
        for h in 0..self.heroes.len() {
            if !self.heroes[h].alive {
                continue;
            }
            let milestones = self.heroes[h].milestones.clone();
            for (k, m) in milestones.iter().enumerate() {
                let Milestone::HealingStrikes { share, radius } = *m else { continue };
                let st = self.state(h, k);
                if now < st.ready_at || st.healing < HEALING_STRIKES_MIN_HEALING {
                    continue;
                }
                st.ready_at = now + HEALING_STRIKES_PULSE;
                let healing = std::mem::take(&mut st.healing);
                // its damage is a proc's: it may set off further milestones, up to the usual depth
                self.proc_depth += 1;
                let origin = self.origin_of(h, k);
                let shown = HitSource::Proc { spell: HEALING_STRIKES_SPELL, school: HEALING_STRIKES_SCHOOL, origin };
                for t in self.enemies_within(self.heroes[h].pos, radius) {
                    self.hit_enemy(Some(h), t, share * healing, false, shown.clone());
                }
                self.proc_depth -= 1;
            }
        }
    }

    /// At run start: spells every run starts with, and pets that stay all run.
    pub(crate) fn opening_milestones(&mut self) {
        let content = self.content.clone();
        for h in 0..self.heroes.len() {
            for m in self.heroes[h].milestones.clone() {
                match m {
                    Milestone::StartWith { spell } => {
                        if let Some(a) = content.ability(&spell) {
                            let mods = self.heroes[h].spell_modifiers.clone();
                            self.heroes[h].spells.learn(a, &mods);
                        }
                    }
                    Milestone::Pet { pet } => self.summon_pet(h, &pet, None),
                    _ => {}
                }
            }
        }
    }

    /// Healing beyond max health becomes a shield (`OverhealShield`), and the whole heal (what it
    /// restored and what spilled over) adds up for healing strikes (`HealingStrikes`, which strike
    /// later from the run tick): called with what a heal tried to restore and what it did.
    pub(crate) fn healing_milestones(&mut self, h: usize, wanted: f32, healed: f32) {
        let overflow = (wanted - healed).max(0.0);
        let milestones = self.heroes[h].milestones.clone();
        for (k, m) in milestones.iter().enumerate() {
            match *m {
                Milestone::OverhealShield { max_share } if overflow > 0.0 => {
                    let hero = &mut self.heroes[h];
                    let cap = max_share * hero.stats.max_health;
                    hero.shield = (hero.shield + overflow).min(cap.max(hero.shield));
                }
                Milestone::HealingStrikes { .. } if wanted > 0.0 => self.state(h, k).healing += wanted,
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::{Content, Stat};

    use super::*;
    use crate::game::enemy::RunOptions;
    use crate::game::run::{Buff, BuffKind, Flat, HeroSetup};

    /// A heal spell of the hero's (its `Spell.dbc` id): only such heals feed the healing milestones.
    const HEAL_SPELL: u32 = 2061;

    fn run(milestones: Vec<Milestone>) -> Run {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Arc::new(Content::load(dir).expect("content loads"));
        let setup = HeroSetup { class: "mage".into(), race: "orc".into(), milestones, ..Default::default() };
        let mut r = Run::new(content, "elwynn", RunOptions::default(), &[setup], Box::new(Flat), 23).unwrap();
        let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
        r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        r.enemies.clear();
        r.pickups.clear();
        r.drain_events();
        r
    }

    fn enemy(r: &mut Run, d: f32) -> u32 {
        let p = r.heroes[0].pos;
        assert!(r.spawn_roster_enemy(0, Some((p.0 + d, p.1))));
        let e = r.enemies.last_mut().unwrap();
        e.health = 1e6;
        e.max_health = 1e6;
        e.id
    }

    #[test]
    fn every_nth_kill_casts_a_free_spell() {
        let m = Milestone::EveryNth { count: 2, spell: "frost_nova".into(), trigger: Trigger::Kill, filter: SpellFilter::Any };
        let mut r = run(vec![m]);
        enemy(&mut r, 3.0);
        for _ in 0..2 {
            let id = enemy(&mut r, 20.0);
            r.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Spell("fireball".into()));
        }
        let casts: Vec<String> = r.drain_events().into_iter().filter_map(|e| if let Event::SpellCast { spell, .. } = e { Some(spell) } else { None }).collect();
        assert_eq!(casts, ["frost_nova"]);
    }

    #[test]
    fn spells_cast_by_milestones_set_off_no_milestones() {
        // every kill casts a free `frost_nova`; the nova's own kills must not cast another
        let m = Milestone::EveryNth { count: 1, spell: "frost_nova".into(), trigger: Trigger::Kill, filter: SpellFilter::Any };
        let mut r = run(vec![m]);
        for _ in 0..4 {
            let id = enemy(&mut r, 3.0);
            r.enemies.iter_mut().filter(|e| e.id == id).for_each(|e| e.health = 1.0);
        }
        let first = r.enemies[0].id;
        r.hit_enemy(Some(0), first, f32::MAX, false, HitSource::Spell("fireball".into()));
        let casts = r.drain_events().iter().filter(|e| matches!(e, Event::SpellCast { .. })).count();
        assert_eq!(casts, 1, "one nova from the fireball kill, none from the nova's own kills");
        assert!(r.enemies.is_empty(), "the nova killed the rest");
    }

    #[test]
    fn a_spell_can_let_its_free_casts_proc() {
        let m = Milestone::EveryNth { count: 1, spell: "frost_nova".into(), trigger: Trigger::Kill, filter: SpellFilter::Any };
        let mut r = run(vec![m]);
        let mut content = (*r.content).clone();
        content.abilities.iter_mut().filter(|a| a.id == "frost_nova").for_each(|a| a.free_casts_can_proc = true);
        r.content = Arc::new(content);
        for _ in 0..3 {
            let id = enemy(&mut r, 3.0);
            r.enemies.iter_mut().filter(|e| e.id == id).for_each(|e| e.health = 1.0);
        }
        let first = r.enemies[0].id;
        r.hit_enemy(Some(0), first, f32::MAX, false, HitSource::Spell("fireball".into()));
        let casts = r.drain_events().iter().filter(|e| matches!(e, Event::SpellCast { .. })).count();
        assert!(casts >= 2, "the nova's own kills cast more novas ({casts})");
    }

    #[test]
    fn a_timed_buff_stacks_and_raises_the_stat() {
        let m = Milestone::TimedBuff { look_spell: None, stat: Stat::Damage, amount: 0.1, max_stacks: 2, seconds: 5.0, trigger: Trigger::Hit, filter: SpellFilter::Any };
        let mut r = run(vec![m]);
        let id = enemy(&mut r, 5.0);
        let base = r.heroes[0].stats.damage;
        for _ in 0..3 {
            r.hit_enemy(Some(0), id, 1.0, false, HitSource::Spell("fireball".into()));
        }
        assert!((r.heroes[0].stats.damage - base - 0.2).abs() < 1e-5, "two stacks at most");
    }

    #[test]
    fn a_swing_crit_makes_the_next_swings_faster_and_a_new_crit_starts_over() {
        let m = Milestone::FasterSwings { amount: 0.3, swings: 2, trigger: Trigger::Crit, filter: SpellFilter::WeaponSwing };
        let mut r = run(vec![m]);
        let id = enemy(&mut r, 5.0);
        r.hit_enemy(Some(0), id, 1.0, false, HitSource::WeaponSwing);
        r.hit_enemy(Some(0), id, 1.0, true, HitSource::Spell("fireball".into()));
        assert_eq!(r.take_faster_swing(0), 0.0, "a plain swing or a spell crit starts nothing");
        r.hit_enemy(Some(0), id, 1.0, true, HitSource::WeaponSwing);
        assert_eq!(r.take_faster_swing(0), 0.3);
        r.hit_enemy(Some(0), id, 1.0, true, HitSource::WeaponSwing);
        let left: Vec<f32> = (0..3).map(|_| r.take_faster_swing(0)).collect();
        assert_eq!(left, [0.3, 0.3, 0.0], "the second crit started the count over, without stacking");
    }

    #[test]
    fn a_blocked_blow_does_less_damage_and_sets_off_block_milestones() {
        let m = Milestone::TimedBuff { look_spell: None, stat: Stat::Damage, amount: 0.1, max_stacks: 1, seconds: 5.0, trigger: Trigger::Block, filter: SpellFilter::Any };
        let mut r = run(vec![m]);
        let id = enemy(&mut r, 3.0);
        let damage = r.heroes[0].stats.damage;
        r.damage_hero(0, 10.0, Some(id), true);
        assert!((r.heroes[0].stats.damage - damage).abs() < 1e-6, "an unblocked blow sets off nothing");
        let health = r.heroes[0].health;
        r.heroes[0].stats.block_chance = 1.0;
        let reduction = r.heroes[0].stats.damage_reduction;
        r.damage_hero(0, 10.0, Some(id), true);
        let lost = health - r.heroes[0].health;
        assert!((lost - 10.0 * (1.0 - crate::game::run::BLOCK_REDUCTION) * (1.0 - reduction)).abs() < 1e-3, "the block took 30 % off ({lost})");
        assert!((r.heroes[0].stats.damage - damage - 0.1).abs() < 1e-5, "the block set off the milestone");
    }

    #[test]
    fn pandemic_lets_effects_over_time_crit_for_their_bonus() {
        let mut r = run(vec![Milestone::DotCrits { bonus: 0.5 }]);
        let id = enemy(&mut r, 5.0);
        r.heroes[0].stats.crit_chance = 1.0;
        let dot = super::super::casting::Dot { hero: 0, spell: "corruption".into(), per_tick: 10.0, ticks_left: 1, interval: 1.0, next: r.now, from_proc: false, leech: 0.0 };
        r.enemies.iter_mut().find(|e| e.id == id).unwrap().dots.push(dot);
        r.effects_tick();
        let ticks: Vec<(f32, bool)> = r.drain_events().iter().filter_map(|e| if let Event::EnemyHit { amount, crit, source: HitSource::Tick(_), .. } = e { Some((*amount, *crit)) } else { None }).collect();
        assert_eq!(ticks.len(), 1);
        assert!(ticks[0].1 && (ticks[0].0 - 15.0).abs() < 1e-3, "a crit tick for 150 %: {ticks:?}");
    }

    #[test]
    fn kindling_takes_time_off_the_listed_cooldowns_on_its_trigger() {
        let m = Milestone::CooldownOnTrigger { seconds: 1.0, spells: vec!["fire_blast".into()], trigger: Trigger::Crit, filter: SpellFilter::Spell("fireball".into()), every: 2.0 };
        let mut r = run(vec![m]);
        let id = enemy(&mut r, 5.0);
        let a = r.content.ability("fire_blast").unwrap().clone();
        r.heroes[0].spells.learn(&a, &[]);
        let k = r.heroes[0].spells.known.iter().position(|s| s.spell == "fire_blast").unwrap();
        r.heroes[0].spells.known[k].timer = 5.0;
        r.hit_enemy(Some(0), id, 1.0, false, HitSource::Spell("fireball".into()));
        assert!((r.heroes[0].spells.known[k].timer - 5.0).abs() < 1e-6, "a plain hit takes nothing");
        r.hit_enemy(Some(0), id, 1.0, true, HitSource::Spell("fireball".into()));
        assert!((r.heroes[0].spells.known[k].timer - 4.0).abs() < 1e-6, "a crit takes a second");
        r.hit_enemy(Some(0), id, 1.0, true, HitSource::Spell("fireball".into()));
        assert!((r.heroes[0].spells.known[k].timer - 4.0).abs() < 1e-6, "not again within 2 sec");
        r.now += 2.0;
        r.hit_enemy(Some(0), id, 1.0, true, HitSource::Spell("fireball".into()));
        assert!((r.heroes[0].spells.known[k].timer - 3.0).abs() < 1e-6, "again after 2 sec");
    }

    #[test]
    fn natures_grace_calls_more_summons_on_every_nth_cast() {
        let m = Milestone::MoreSummonsEveryNth { count: 2, extra: 1, filter: SpellFilter::Spell("friend_of_the_forest".into()) };
        let mut r = run(vec![m]);
        let forest = r.content.ability("friend_of_the_forest").unwrap().clone();
        let fireball = r.content.ability("fireball").unwrap().clone();
        assert_eq!(r.more_summons(0, &forest), 0, "the first cast calls its own");
        assert_eq!(r.more_summons(0, &fireball), 0, "another spell is not counted");
        assert_eq!(r.more_summons(0, &forest), 1, "the second calls one more");
        assert_eq!(r.more_summons(0, &forest), 0, "and the count starts over");
    }

    #[test]
    fn splash_hits_the_others_around_the_target() {
        let filter = SpellFilter::Spell("fireball".into());
        let m = Milestone::Splash { share: 0.5, radius: 8.0, look_spell: None, requires_aura: None, trigger: Trigger::Hit, filter, effects: vec![] };
        let mut r = run(vec![m]);
        let a = enemy(&mut r, 10.0);
        let b = enemy(&mut r, 12.0);
        r.hit_enemy(Some(0), a, 100.0, false, HitSource::Spell("fireball".into()));
        let hb = r.enemies.iter().find(|e| e.id == b).unwrap().health;
        assert!((1e6 - hb - 50.0).abs() < 0.01);
        // another spell does not splash
        r.hit_enemy(Some(0), a, 100.0, false, HitSource::Spell("frostbolt".into()));
        assert_eq!(r.enemies.iter().find(|e| e.id == b).unwrap().health, hb);
    }

    /// The hits enemy `id` took in `events`: how much, and how each shows.
    fn hits_on(events: &[Event], id: u32) -> Vec<(f32, HitSource)> {
        events.iter().filter_map(|e| match e { Event::EnemyHit { id: x, amount, source, .. } if *x == id => Some((*amount, source.clone())), _ => None }).collect()
    }

    fn look(r: &Run, spell: u32, hits: &[u32], aim: Aim, reach: f32) -> Event {
        let hits = hits.iter().map(|&t| Unit::Enemy(t)).collect();
        Event::SpellGo { caster: Unit::Hero(r.heroes[0].id), spell, hits, aim, ammo: false, reach }
    }

    fn spot(r: &Run, id: u32) -> Vec2 {
        r.enemies.iter().find(|e| e.id == id).unwrap().position(r.now)
    }

    #[test]
    fn a_splash_shows_its_look_and_logs_with_the_hits_spell() {
        let filter = SpellFilter::Spell("fireball".into());
        let spell = Milestone::Splash { share: 0.5, radius: 8.0, look_spell: Some(853), requires_aura: None, trigger: Trigger::Hit, filter, effects: vec![] };
        let filter = SpellFilter::WeaponSwing;
        let swing = Milestone::Splash { share: 0.5, radius: 8.0, look_spell: None, requires_aura: None, trigger: Trigger::Hit, filter, effects: vec![] };
        let mut r = run(vec![spell, swing]);
        let a = enemy(&mut r, 10.0);
        let b = enemy(&mut r, 12.0);
        let at = spot(&r, a);
        r.hit_enemy(Some(0), a, 100.0, false, HitSource::Spell("fireball".into()));
        let events = r.drain_events();
        assert!(events.contains(&look(&r, 853, &[], Aim::Ground(at), 8.0)), "the look goes off at the struck enemy");
        assert_eq!(hits_on(&events, b), [(50.0, HitSource::Proc { spell: 133, school: School::Fire, origin: Default::default() })], "logged as the fireball");
        // after a weapon swing: no look of its own, and the splash names no spell
        r.hit_enemy(Some(0), a, 100.0, false, HitSource::WeaponSwing);
        let events = r.drain_events();
        assert!(!events.iter().any(|e| matches!(e, Event::SpellGo { .. })));
        assert_eq!(hits_on(&events, b), [(50.0, HitSource::Proc { spell: 0, school: School::Physical, origin: Default::default() })]);
    }

    #[test]
    fn a_splash_carries_the_struck_hits_own_effect() {
        let filter = SpellFilter::Spell("frostbolt".into());
        let m = Milestone::Splash { share: 0.5, radius: 8.0, look_spell: None, requires_aura: None, trigger: Trigger::Hit, filter, effects: vec![] };
        let mut r = run(vec![m]);
        let a = enemy(&mut r, 10.0);
        let b = enemy(&mut r, 12.0);
        r.hit_enemy(Some(0), a, 100.0, false, HitSource::Spell("frostbolt".into()));
        assert!(r.enemies.iter().find(|e| e.id == b).unwrap().slow.is_some(), "frostbolt's slow comes with the splash");
    }

    #[test]
    fn a_sweep_reaches_only_enemies_close_to_the_struck_one() {
        let m = Milestone::ExtraTargets { look_spell: None, count: 3, share: 0.5, trigger: Trigger::Hit, filter: SpellFilter::WeaponSwing, per_swing_second: false, growth: 0.0, reach: 0.0, effects: vec![] };
        let mut r = run(vec![m]);
        let a = enemy(&mut r, 5.0);
        let near = enemy(&mut r, 14.0);
        let far = enemy(&mut r, 16.0);
        r.hit_enemy(Some(0), a, 100.0, false, HitSource::WeaponSwing);
        let events = r.drain_events();
        assert_eq!(hits_on(&events, near).len(), 1, "9 yd away");
        assert!(hits_on(&events, far).is_empty(), "11 yd away");
    }

    #[test]
    fn a_sweep_after_a_spell_shows_the_spell_onto_each_swept_enemy() {
        let m = Milestone::ExtraTargets { look_spell: None, count: 2, share: 0.5, trigger: Trigger::Hit, filter: SpellFilter::Spell("frostbolt".into()), per_swing_second: false, growth: 0.0, reach: 0.0, effects: vec![] };
        let mut r = run(vec![m]);
        let a = enemy(&mut r, 5.0);
        let b = enemy(&mut r, 6.0);
        let c = enemy(&mut r, 7.0);
        r.hit_enemy(Some(0), a, 100.0, false, HitSource::Spell("frostbolt".into()));
        let events = r.drain_events();
        for t in [b, c] {
            assert!(events.contains(&look(&r, 116, &[t], Aim::Unit(Unit::Enemy(t)), 0.0)));
            assert_eq!(hits_on(&events, t), [(50.0, HitSource::Proc { spell: 116, school: School::Frost, origin: Default::default() })]);
        }
    }

    #[test]
    fn blows_added_to_swings_show_as_swings_and_never_feed_themselves() {
        let swing = Milestone::StrikeAgain { look_spell: None, chance: 1.0, times: 2, share: 1.0, trigger: Trigger::Hit, filter: SpellFilter::WeaponSwing };
        let pets = Milestone::StrikeAgain { look_spell: None, chance: 1.0, times: 2, share: 1.0, trigger: Trigger::Hit, filter: SpellFilter::Pets };
        let mut r = run(vec![swing, pets]);
        let a = enemy(&mut r, 3.0);
        r.hit_enemy(Some(0), a, 10.0, false, HitSource::WeaponSwing);
        assert_eq!(hits_on(&r.drain_events(), a), vec![(10.0, HitSource::WeaponSwing); 3], "the swing and two more, no more after those");
        // a melee pet's bite: the extra blows are the hero's first pet's swings
        r.summon_pet(0, "Wolf", None);
        r.summon_pet(0, "Raptor", None);
        r.summon_pet(0, "Imp", None);
        let (wolf, raptor, imp) = (r.pets[0].id, r.pets[1].id, r.pets[2].id);
        r.drain_events();
        r.hit_enemy(Some(0), a, 10.0, false, HitSource::Pet(raptor));
        assert_eq!(hits_on(&r.drain_events(), a), [(10.0, HitSource::Pet(raptor)), (10.0, HitSource::Pet(wolf)), (10.0, HitSource::Pet(wolf))]);
        // a ranged pet's hit names its spell: the extra blows are damage lines with it
        r.hit_enemy(Some(0), a, 10.0, false, HitSource::Pet(imp));
        let fire = HitSource::Proc { spell: 133, school: School::Fire, origin: Default::default() };
        assert_eq!(hits_on(&r.drain_events(), a), [(10.0, HitSource::Pet(imp)), (10.0, fire.clone()), (10.0, fire)]);
    }

    #[test]
    fn a_death_explosion_logs_with_its_look_or_explosive_traps() {
        let filter = SpellFilter::Spell("fireball".into());
        let plain = Milestone::ExplodeOnDeath { afflicted: false, share: 0.5, radius: 6.0, look_spell: None, filter };
        let filter = SpellFilter::Spell("frostbolt".into());
        let shown = Milestone::ExplodeOnDeath { afflicted: false, share: 0.5, radius: 6.0, look_spell: Some(11113), filter };
        let mut r = run(vec![plain, shown]);
        let a = enemy(&mut r, 5.0);
        let b = enemy(&mut r, 6.0);
        let c = enemy(&mut r, 7.0);
        r.hit_enemy(Some(0), a, f32::MAX, false, HitSource::Spell("fireball".into()));
        let events = r.drain_events();
        assert!(!events.iter().any(|e| matches!(e, Event::SpellGo { .. })), "no look of its own");
        assert_eq!(hits_on(&events, b), [(5e5, HitSource::Proc { spell: DEATH_EXPLOSION_SPELL, school: School::Fire, origin: Default::default() })]);
        let at = spot(&r, b);
        r.hit_enemy(Some(0), b, f32::MAX, false, HitSource::Spell("frostbolt".into()));
        let events = r.drain_events();
        assert!(events.contains(&look(&r, 11113, &[], Aim::Ground(at), 6.0)), "the look goes off where it died");
        assert_eq!(hits_on(&events, c), [(5e5, HitSource::Proc { spell: 11113, school: School::Frost, origin: Default::default() })]);
    }

    #[test]
    fn a_shield_burst_shows_its_look_on_everything_it_strikes() {
        let (look_spell, trigger) = (Some(15237), Trigger::ShieldBreak);
        let m = Milestone::ShieldBurst { school: Some(School::Holy), share: 0.5, radius: 10.0, look_spell, trigger, filter: SpellFilter::Any };
        let mut r = run(vec![m]);
        let near = enemy(&mut r, 5.0);
        let far = enemy(&mut r, 30.0);
        r.milestone_event(0, CombatEvent::ShieldBroken { absorbed: 100.0 });
        let events = r.drain_events();
        assert!(events.contains(&look(&r, 15237, &[near], Aim::Caster, 10.0)));
        // half of what it absorbed, × the hero's damage (the staff loadout's +10 %)
        let burst = 50.0 * r.heroes[0].stats.damage;
        assert_eq!(hits_on(&events, near), [(burst, HitSource::Proc { spell: 15237, school: School::Holy, origin: Default::default() })]);
        assert!(hits_on(&events, far).is_empty());
        // it grows with the hero's damage
        r.heroes[0].stats.damage = 2.0;
        r.milestone_event(0, CombatEvent::ShieldBroken { absorbed: 100.0 });
        assert_eq!(hits_on(&r.drain_events(), near), [(100.0, HitSource::Proc { spell: 15237, school: School::Holy, origin: Default::default() })]);
        // with nothing in reach the look still goes off
        r.enemies.clear();
        r.milestone_event(0, CombatEvent::ShieldBroken { absorbed: 100.0 });
        assert_eq!(r.drain_events(), [look(&r, 15237, &[], Aim::Caster, 10.0)]);
    }

    #[test]
    fn an_effect_that_jumps_on_death_shows_its_spell_going_onto_the_next() {
        let m = Milestone::SpreadOnDeath { look_spell: None, count: 1, filter: SpellFilter::Spell("corruption".into()) };
        let mut r = run(vec![m]);
        let a = enemy(&mut r, 5.0);
        let b = enemy(&mut r, 6.0);
        let dot = Dot { hero: 0, spell: "corruption".into(), per_tick: 1.0, ticks_left: 5, interval: 1.0, next: r.now + 1.0, from_proc: false, leech: 0.0 };
        r.enemies.iter_mut().find(|e| e.id == a).unwrap().dots.push(dot);
        r.hit_enemy(Some(0), a, f32::MAX, false, HitSource::Spell("fireball".into()));
        let events = r.drain_events();
        assert!(events.contains(&look(&r, 172, &[b], Aim::Unit(Unit::Enemy(b)), 0.0)));
        assert_eq!(r.enemies.iter().find(|e| e.id == b).unwrap().dots.len(), 1);
    }

    #[test]
    fn an_effect_jumps_only_to_close_enemies_that_lack_it() {
        let m = Milestone::SpreadOnDeath { look_spell: None, count: 2, filter: SpellFilter::Any };
        let mut r = run(vec![m]);
        let a = enemy(&mut r, 5.0);
        let has_it = enemy(&mut r, 6.0);
        let lacks_it = enemy(&mut r, 7.0);
        let far = enemy(&mut r, 20.0);
        let dot = |spell: &str, now: f32| Dot { hero: 0, spell: spell.into(), per_tick: 1.0, ticks_left: 5, interval: 1.0, next: now + 1.0, from_proc: false, leech: 0.0 };
        let now = r.now;
        r.enemies.iter_mut().find(|e| e.id == a).unwrap().dots.extend([dot("corruption", now), dot("immolate", now)]);
        r.enemies.iter_mut().find(|e| e.id == has_it).unwrap().dots.push(dot("corruption", now));
        r.hit_enemy(Some(0), a, f32::MAX, false, HitSource::Spell("fireball".into()));
        let spells = |r: &Run, id: u32| -> Vec<String> {
            let mut v: Vec<String> = r.enemies.iter().find(|e| e.id == id).unwrap().dots.iter().map(|d| d.spell.clone()).collect();
            v.sort();
            v
        };
        assert_eq!(spells(&r, has_it), ["corruption", "immolate"], "no second Spell 172");
        assert_eq!(spells(&r, lacks_it), ["corruption", "immolate"], "each effect finds its own enemies");
        assert!(spells(&r, far).is_empty(), "15 yd from where it died");
    }

    #[test]
    fn healing_strikes_pulse_at_most_once_a_second() {
        let mut r = run(vec![Milestone::HealingStrikes { share: 3.0, radius: 12.0 }]);
        let a = enemy(&mut r, 5.0);
        let far = enemy(&mut r, 20.0);
        let max = r.heroes[0].stats.max_health;
        r.heroes[0].health = 0.5 * max;
        r.heal_hero(0, 10.0, HEAL_SPELL);
        r.heal_hero(0, 5.0, HEAL_SPELL);
        assert!(hits_on(&r.drain_events(), a).is_empty(), "healing only adds up; the strike comes from the run tick");
        let tranquility = HitSource::Proc { spell: HEALING_STRIKES_SPELL, school: School::Nature, origin: Default::default() };
        r.periodic_milestones_tick();
        let events = r.drain_events();
        assert_eq!(hits_on(&events, a), [(45.0, tranquility.clone())], "3 × the 15 healing added up");
        assert!(hits_on(&events, far).is_empty());
        assert!(!events.iter().any(|e| matches!(e, Event::SpellGo { .. })), "tranquility's record has no look");
        // more healing waits for the second to pass
        r.heal_hero(0, 10.0, HEAL_SPELL);
        r.now += 0.5;
        r.periodic_milestones_tick();
        assert!(hits_on(&r.drain_events(), a).is_empty());
        r.now += 0.5;
        r.periodic_milestones_tick();
        assert_eq!(hits_on(&r.drain_events(), a), [(30.0, tranquility)]);
        // less than 1 healing added up strikes nothing
        r.heal_hero(0, 0.5, HEAL_SPELL);
        r.now += 2.0;
        r.periodic_milestones_tick();
        assert!(hits_on(&r.drain_events(), a).is_empty());
    }

    #[test]
    fn a_crit_free_cast_respects_its_internal_cooldown() {
        let m = Milestone::FreeCast { chance: 1.0, cooldown: 5.0, spell: "frost_nova".into(), trigger: Trigger::Crit, filter: SpellFilter::Any };
        let mut r = run(vec![m]);
        let id = enemy(&mut r, 3.0);
        r.hit_enemy(Some(0), id, 1.0, false, HitSource::Spell("fireball".into()));
        r.hit_enemy(Some(0), id, 1.0, true, HitSource::Spell("fireball".into()));
        r.hit_enemy(Some(0), id, 1.0, true, HitSource::Spell("fireball".into()));
        let casts = r.drain_events().iter().filter(|e| matches!(e, Event::SpellCast { .. })).count();
        assert_eq!(casts, 1, "no cast on a plain hit, one on the first crit, then the cooldown");
    }

    /// A milestone's damage over time is one effect per enemy: a new one takes over what the old
    /// one still had to deal and starts its ticks afresh.
    #[test]
    fn a_milestone_dot_rolls_what_is_left_into_the_new_one() {
        let m = Milestone::DotOnHit { look_spell: 12654, share: 0.8, seconds: 4.0, trigger: Trigger::Crit, filter: SpellFilter::Any };
        let mut r = run(vec![m]);
        let id = enemy(&mut r, 3.0);
        r.hit_enemy(Some(0), id, 100.0, true, HitSource::Spell("fireball".into()));
        let ignite = |r: &Run| r.enemies.iter().find(|e| e.id == id).unwrap().dots.iter().filter(|d| d.spell == format!("{AURA_PREFIX}12654")).cloned().collect::<Vec<_>>();
        let first = ignite(&r);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].ticks_left, 4);
        let one = first[0].per_tick * 4.0;
        // one tick goes by, then another crit lands
        r.enemies.iter_mut().find(|e| e.id == id).unwrap().dots[0].ticks_left = 3;
        r.hit_enemy(Some(0), id, 100.0, true, HitSource::Spell("fireball".into()));
        let second = ignite(&r);
        assert_eq!(second.len(), 1, "still one effect");
        assert_eq!(second[0].ticks_left, 4, "its ticks start afresh");
        assert!((second[0].per_tick * 4.0 - (one + one * 0.75)).abs() < 1e-3, "the new share plus the three ticks left");
    }

    /// A spell cast by itself every few seconds waits its interval shortened by the hero's
    /// cooldown factor after a cast, and tries again 0.3 sec after a cast that found nothing.
    #[test]
    fn a_periodic_spell_waits_its_interval_times_the_cooldown_factor_and_retries_soon() {
        let mut r = run(vec![Milestone::Periodic { interval: 10.0, spell: "frost_nova".into() }]);
        let k = r.heroes[0].milestones.iter().position(|m| matches!(m, Milestone::Periodic { .. })).unwrap();
        r.heroes[0].stats.cooldown_factor = 0.5;
        let now = r.now;
        r.periodic_milestones_tick();
        assert!((r.state(0, k).next - (now + PERIODIC_RETRY)).abs() < 1e-4, "no enemy near: tries again soon");
        enemy(&mut r, 3.0);
        r.now += PERIODIC_RETRY;
        let now = r.now;
        r.periodic_milestones_tick();
        assert!((r.state(0, k).next - (now + 5.0)).abs() < 1e-4, "cast: 10 s × 0.5");
    }

    /// "Hurt while below a share of health" answers only a melee blow from an enemy, while the
    /// hero is under that share; spells and ground effects do not set it off.
    #[test]
    fn hurt_below_a_health_share_answers_melee_blows_only() {
        let m = Milestone::FreeCast { chance: 1.0, cooldown: 0.0, spell: "frost_nova".into(), trigger: Trigger::HealthBelow(0.4), filter: SpellFilter::Any };
        let mut r = run(vec![m]);
        let id = enemy(&mut r, 3.0);
        r.heroes[0].stats.damage_reduction = 0.0;
        let max = r.heroes[0].stats.max_health;
        let casts = |r: &mut Run| r.drain_events().iter().filter(|e| matches!(e, Event::SpellCast { .. })).count();
        r.heroes[0].health = max * 0.5 + 1.0;
        r.damage_hero(0, 1.0, Some(id), true);
        assert_eq!(casts(&mut r), 0, "at half health: not yet below");
        r.heroes[0].health = max * 0.3;
        r.damage_hero(0, 1.0, Some(id), false);
        r.damage_hero(0, 1.0, None, false);
        assert_eq!(casts(&mut r), 0, "a spell or a ground effect sets nothing off");
        r.damage_hero(0, 1.0, Some(id), true);
        assert_eq!(casts(&mut r), 1, "a melee blow below 40 % does");
    }

    #[test]
    fn runs_start_with_spells_and_pets() {
        let r = run(vec![Milestone::StartWith { spell: "frost_nova".into() }, Milestone::Pet { pet: "Imp".into() }]);
        assert!(r.heroes[0].spells.knows("frost_nova"));
        assert_eq!(r.pets.len(), 1);
    }

    #[test]
    fn overhealing_becomes_a_shield_up_to_its_cap() {
        let mut r = run(vec![Milestone::OverhealShield { max_share: 0.2 }]);
        let max = r.heroes[0].stats.max_health;
        r.heal_hero(0, max, HEAL_SPELL);
        assert!((r.heroes[0].shield - 0.2 * max).abs() < 1e-3);
    }

    /// Heal spells and heals over time feed the healing milestones with the whole heal, spilled
    /// over or not; lifesteal, kills, runes, level-ups and milestone heals feed nothing.
    #[test]
    fn only_heal_spells_and_heals_over_time_feed_the_healing_milestones() {
        let mut r = run(vec![Milestone::HealingStrikes { share: 3.0, radius: 12.0 }, Milestone::OverhealShield { max_share: 0.2 }]);
        let k = r.heroes[0].milestones.iter().position(|m| matches!(m, Milestone::HealingStrikes { .. })).unwrap();
        let max = r.heroes[0].stats.max_health;
        r.heroes[0].health = max;
        r.heal_hero(0, 50.0, 0);
        assert_eq!(r.state(0, k).healing, 0.0, "a heal with no spell feeds nothing");
        assert_eq!(r.heroes[0].shield, 0.0);
        r.heal_hero(0, 10.0, HEAL_SPELL);
        assert_eq!(r.state(0, k).healing, 10.0, "at full health the whole heal still adds up");
        assert!((r.heroes[0].shield - 10.0).abs() < 1e-3, "and it spills into the shield");
        r.heroes[0].buffs.push(Buff { kind: BuffKind::Regen, value: 4.0, visual: 774, until: r.now + 10.0 });
        r.heroes[0].stats.health_regen = 0.0;
        r.tick(0.1);
        assert!((r.heroes[0].shield - 10.4).abs() < 1e-3, "a heal over time spills into the shield too: {}", r.heroes[0].shield);
    }

    #[test]
    fn every_full_talent_tree_plays_two_minutes() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Arc::new(Content::load(dir).expect("content loads"));
        for tree in &content.talent_trees {
            let mut setup = HeroSetup { class: tree.class.clone(), race: "orc".into(), ..Default::default() };
            setup.add_effects(tree.talents.iter().flat_map(|t| t.effects.iter().map(move |e| (e, u32::from(t.max_rank)))));
            let mut r = Run::new(content.clone(), "elwynn", RunOptions::default(), &[setup], Box::new(Flat), 29).unwrap();
            let (id, (cx, cy)) = (r.heroes[0].id, r.heroes[0].pos);
            let mut angle = 0.0f32;
            while r.now < 120.0 && matches!(r.phase, crate::game::run::Phase::Running | crate::game::run::Phase::Waiting) {
                let speed = r.heroes[0].stats.run_speed();
                angle += speed * 0.05 / 20.0;
                r.hero_moved(id, (cx + 20.0 * angle.cos(), cy + 20.0 * angle.sin()), 0.0, angle + std::f32::consts::FRAC_PI_2);
                r.tick(0.05);
                r.take_first_offers();
                r.drain_events();
            }
            println!("{:8} {:22} t={:5.1} phase={:?} kills={} level={} pets={}", tree.class, tree.name, r.now, r.phase, r.heroes[0].kills, r.heroes[0].progress.level, r.pets.len());
            assert!(r.heroes[0].kills > 0, "{} / {} killed nothing", tree.class, tree.name);
        }
    }

    /// Each proc hit carries what gave the hero its milestone, for the
    /// meter: a talent bought, an amulet or legendary amulet worn, a ring or trinket.
    #[test]
    fn procs_carry_what_gave_them() {
        use crate::game::casting::ProcOrigin;
        use crate::game::choices::{Offer, WornJewelry};
        let explode = || Milestone::ExplodeOnDeath { afflicted: false, share: 0.5, radius: 6.0, look_spell: None, filter: SpellFilter::Any };
        // the burst of a dead enemy on its neighbour, after a kill by fireball
        let burst_on_neighbour = |r: &mut Run| {
            let a = enemy(r, 5.0);
            let b = enemy(r, 6.0);
            r.drain_events();
            r.hit_enemy(Some(0), a, f32::MAX, false, HitSource::Spell("fireball".into()));
            let hits = hits_on(&r.drain_events(), b);
            r.enemies.clear();
            match hits.as_slice() {
                [(_, HitSource::Proc { origin, .. })] => origin.clone(),
                other => panic!("{other:?}"),
            }
        };

        // a talent: every milestone it gives is the talent's, and so are their hits
        let content = run(vec![]).content.clone();
        let talent = content
            .talent_trees
            .iter()
            .filter(|t| t.class == "mage")
            .flat_map(|t| &t.talents)
            .find(|t| t.effects.iter().any(|e| matches!(e, esurv_content::TalentEffect::Milestone(_))))
            .expect("a mage talent with a milestone");
        let setup = HeroSetup::with_progress(&content, "mage", "orc", [(talent.id.as_str(), 1)], &[]);
        assert!(!setup.milestones.is_empty());
        assert_eq!(setup.milestone_origins, vec![ProcOrigin::Talent(talent.id.clone()); setup.milestones.len()]);
        let setup = HeroSetup { milestones: vec![explode()], milestone_origins: vec![ProcOrigin::Talent(talent.id.clone())], ..setup };
        let mut r = Run::new(content.clone(), "elwynn", RunOptions::default(), &[setup], Box::new(Flat), 23).unwrap();
        r.enemies.clear();
        assert_eq!(burst_on_neighbour(&mut r), ProcOrigin::Talent(talent.id.clone()));

        // an amulet worn, and a legendary amulet (its id `legendary:<spell>`)
        let mut r = run(vec![]);
        let hero = r.heroes[0].id;
        let mut amulet = content.amulets[0].clone();
        amulet.effects = vec![esurv_content::TalentEffect::Milestone(explode())];
        assert!(r.grant(hero, Offer::Amulet { amulet: Box::new(amulet.clone()) }));
        assert_eq!(burst_on_neighbour(&mut r), ProcOrigin::Amulet(amulet.id.clone()));
        let light = content.legendary_amulets.iter().find(|l| l.name == "Light's Blessing").expect("Light's Blessing");
        let mut legendary = super::super::amulets::legendary_for(light);
        assert!(r.grant(hero, Offer::Amulet { amulet: Box::new(legendary.clone()) }));
        let worn = &r.heroes[0];
        assert!(worn.milestone_origins.len() == worn.milestones.len() && !worn.milestones.is_empty());
        assert!(worn.milestone_origins.iter().all(|o| *o == ProcOrigin::Legendary(light.id())), "{:?}", worn.milestone_origins);
        legendary.effects = vec![esurv_content::TalentEffect::Milestone(explode())];
        assert!(r.grant(hero, Offer::Amulet { amulet: Box::new(legendary) }));
        assert_eq!(burst_on_neighbour(&mut r), ProcOrigin::Legendary(light.id()));

        // a trinket: the Soulstone's extra life blasts the enemies near the hero
        let mut r = run(vec![]);
        r.heroes[0].jewelry.push(WornJewelry { id: "soulstone".into(), value: 1.0, rank: 1 });
        r.heroes[0].extra_lives = 1;
        let near = enemy(&mut r, 3.0);
        r.drain_events();
        r.damage_hero(0, 1e9, None, false);
        let hits = hits_on(&r.drain_events(), near);
        assert!(matches!(hits.as_slice(), [(_, HitSource::Proc { spell: 0, origin: ProcOrigin::Jewelry(id), .. })] if id == "soulstone"), "{hits:?}");
    }
}
