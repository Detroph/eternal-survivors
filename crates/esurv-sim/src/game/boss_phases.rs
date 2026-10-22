//! Boss phases beyond a single script line (docs/game/bosses.md,
//! "Building blocks"): runs of waves the boss waits behind (a gate event, a guard of adds), the
//! boss taking no damage or no spell damage for a while, rage that stacks, adds that heal their
//! boss, class calls that twist a hero's class (never crowd control), the fallen rising again, the
//! horde pulled into one clump, and a figure rising for a moment (Ragnaros). Each acts on the
//! heroes only, except the pull and the rising, which move or raise the horde.

use esurv_content::{ClassCall, ScriptAction, WaveAt, WaveStep};

use super::enemy_ai::{Segment, Vec2};
use super::enemy_attacks::AbilitySlot;
use super::ground_zones::HeroAffliction;
use super::run::{Aim, EnemyKind, Event, Run, Unit};

/// A run of waves' timer bar and its adds' counter are keyed from here.
pub const WAVE_BAR_KEY: u32 = 90;
pub const WAVE_COUNTER_KEY: u32 = 91;
/// Enemies pulled into a clump slide there in this long, then stand this long (a burst window).
pub const GATHER_SLIDE: f32 = 0.4;
pub const GATHER_HOLD: f32 = 2.0;
/// A pulled clump is this many yards across.
pub const GATHER_SPREAD: f32 = 2.5;
/// A figure's sinking look plays this long before it is gone.
pub const SINK_LOOK_AHEAD: f32 = 1.5;

/// What a boss's phases keep on it (and on a figure risen for a moment).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PhaseState {
    /// It takes no damage (behind its waves, or a figure).
    pub immune: bool,
    /// It takes no spell damage until then (Reflection).
    pub spell_immune_until: f32,
    pub waves: Option<WaveRun>,
    pub rage: Option<Rage>,
    /// Where its waves' adds fell.
    pub fallen: Vec<Vec2>,
    /// It never moves (a figure, a tentacle); a figure never swings either.
    pub rooted: bool,
    /// A figure sinks then, its look `sink_spell` played a little before.
    pub sinks_at: Option<(f32, u32)>,
}

/// A run of waves going on.
#[derive(Debug, Clone, PartialEq)]
pub struct WaveRun {
    pub waves: Vec<WaveStep>,
    pub gap: f32,
    pub aloof: bool,
    pub look_spell: u32,
    pub bar: Option<String>,
    pub counter: Option<String>,
    pub then: Vec<ScriptAction>,
    /// The wave now (its index).
    pub step: usize,
    /// Its adds still standing, and where they were last.
    pub adds: Vec<(u32, Vec2)>,
    pub total: usize,
    /// The adds left as last told.
    pub told: usize,
    /// The next wave comes then anyway (infinite: only once these are dead).
    pub next_at: f32,
}

/// Rage stacked on a boss.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rage {
    pub stacks: u32,
    pub faster: f32,
    pub harder: f32,
    pub until: f32,
}

impl PhaseState {
    /// How much faster its abilities come and it moves now.
    pub fn haste(&self, now: f32) -> f32 {
        self.rage.filter(|r| now < r.until).map_or(1.0, |r| 1.0 + r.faster * r.stacks as f32)
    }

    /// How much harder its marked attacks and zones hit now.
    pub fn harder(&self, now: f32) -> f32 {
        self.rage.filter(|r| now < r.until).map_or(1.0, |r| 1.0 + r.harder * r.stacks as f32)
    }

    /// Whether it stands apart behind its waves.
    pub fn aloof(&self) -> bool {
        self.waves.as_ref().is_some_and(|w| w.aloof)
    }
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Run {
    /// The actions of the phases (`boss_scripts`' `run_actions` hands them here), for enemy `k`
    /// (the boss, if still here) whose attack damage is `attack`, at `at`.
    pub(crate) fn run_phase_action(&mut self, source: Option<u32>, k: Option<usize>, at: Vec2, attack: f32, action: &ScriptAction) {
        let now = self.now;
        match action {
            ScriptAction::Waves { waves, gap, immune, aloof, look_spell, bar, counter, then } => {
                let Some(k) = k else { return };
                let e = &mut self.enemies[k];
                e.mech.phases.immune = *immune;
                if *look_spell != 0 {
                    e.kit.worn_looks.push((*look_spell, f32::INFINITY));
                }
                if *aloof {
                    e.mech.busy_until = f32::INFINITY;
                    let p = e.position(now);
                    e.segment = Segment::standing(p, now);
                }
                e.mech.phases.waves = Some(WaveRun {
                    waves: waves.clone(),
                    gap: *gap,
                    aloof: *aloof,
                    look_spell: *look_spell,
                    bar: bar.clone(),
                    counter: counter.clone(),
                    then: then.clone(),
                    step: 0,
                    adds: Vec::new(),
                    total: 0,
                    told: usize::MAX,
                    next_at: f32::INFINITY,
                });
                let id = e.id;
                // standing apart, it shows no bars of its own until it fights
                if *aloof {
                    self.push(Event::BarsCleared { boss: id });
                }
                self.start_wave(id, 0);
            }
            ScriptAction::Gather { within, ahead, look_spell } => {
                let Some(k) = k else { return };
                let boss = self.enemies[k].position(now);
                let Some(h) = self.nearest_hero(boss) else { return };
                let face = super::telegraphs::facing(boss, self.heroes[h].pos);
                let clump = (boss.0 + face.cos() * ahead, boss.1 + face.sin() * ahead);
                let id = self.enemies[k].id;
                self.push(Event::SpellGo { caster: Unit::Enemy(id), spell: *look_spell, hits: vec![], aim: Aim::Caster, ammo: false, reach: *within, triggered: false });
                let mut rng = std::mem::replace(&mut self.rng, crate::rng::Rng::new(0));
                for e in self.enemies.iter_mut().filter(|e| !e.is_boss() && !e.mech.phases.rooted) {
                    let p = e.position(now);
                    if dist(p, boss) > *within {
                        continue;
                    }
                    let (a, r) = (rng.range(0.0, std::f32::consts::TAU), rng.range(0.0, GATHER_SPREAD));
                    let to = (clump.0 + a.cos() * r, clump.1 + a.sin() * r);
                    e.segment = Segment { from: p, to, start: now, duration: GATHER_SLIDE };
                    e.charging_until = now + GATHER_SLIDE;
                    e.next_decision = now + GATHER_SLIDE + GATHER_HOLD;
                    e.hold_until = e.hold_until.max(now + GATHER_SLIDE + GATHER_HOLD);
                }
                self.rng = rng;
            }
            ScriptAction::SpellImmune { seconds, look_spell } => {
                let Some(k) = k else { return };
                let e = &mut self.enemies[k];
                e.mech.phases.spell_immune_until = now + seconds;
                e.kit.worn_looks.push((*look_spell, now + seconds));
            }
            ScriptAction::Empower { look_spell, faster, harder, max_stacks, seconds } => {
                let Some(k) = k else { return };
                let until = if *seconds > 0.0 { now + seconds } else { f32::INFINITY };
                let e = &mut self.enemies[k];
                let stacks = e.mech.phases.rage.filter(|r| now < r.until).map_or(0, |r| r.stacks);
                e.mech.phases.rage = Some(Rage { stacks: (stacks + 1).min((*max_stacks).max(1)), faster: *faster, harder: *harder, until });
                e.kit.worn_looks.retain(|(l, _)| l != look_spell);
                e.kit.worn_looks.push((*look_spell, until));
            }
            ScriptAction::Mend { by, share, look_spell } => {
                let (Some(k), Some(index)) = (k, self.roster_index(by)) else { return };
                let boss = self.enemies[k].id;
                let healers: Vec<u32> = self.enemies.iter().filter(|e| e.kind == EnemyKind::Roster(index)).map(|e| e.id).collect();
                if healers.is_empty() {
                    return;
                }
                let e = &mut self.enemies[k];
                e.health = (e.health + share * e.max_health * healers.len() as f32).min(e.max_health);
                for h in healers {
                    self.push(Event::SpellGo { caster: Unit::Enemy(h), spell: *look_spell, hits: vec![Unit::Enemy(boss)], aim: Aim::Unit(Unit::Enemy(boss)), ammo: false, reach: 0.0, triggered: false });
                }
            }
            ScriptAction::ClassCall { calls, seconds } => self.class_call(source, at, calls, *seconds),
            ScriptAction::RaiseFallen { rise_as, look_spell, most } => {
                let (Some(k), Some(index)) = (k, self.roster_index(rise_as)) else { return };
                let spots: Vec<Vec2> = std::mem::take(&mut self.enemies[k].mech.phases.fallen).into_iter().take(*most as usize).collect();
                for p in spots {
                    if self.spawn_summoned(index, p) {
                        let risen = self.enemies.last().map_or(0, |e| e.id);
                        self.push(Event::SpellGo { caster: Unit::Enemy(risen), spell: *look_spell, hits: vec![], aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
                    }
                }
            }
            ScriptAction::Cameo { delay, .. } => {
                if *delay > 0.0 {
                    self.cameos_due.push((now + delay, action.clone(), at));
                } else {
                    self.cameo_rises(action, at, attack);
                }
            }
            _ => {}
        }
    }

    /// Wave `step` of boss `id`'s run comes: its warning, its actions, and its adds counted.
    fn start_wave(&mut self, id: u32, step: usize) {
        let now = self.now;
        let Some(k) = self.enemies.iter().position(|e| e.id == id) else { return };
        let Some(w) = self.enemies[k].mech.phases.waves.clone() else { return };
        let Some(wave) = w.waves.get(step) else { return };
        if let Some(warn) = &wave.warn {
            self.announce_warning(warn, warn.icon_spell.unwrap_or(0));
        }
        let before: std::collections::HashSet<u32> = self.enemies.iter().map(|e| e.id).collect();
        let at = self.enemies[k].position(now);
        self.run_actions(Some(id), at, &wave.run);
        let adds: Vec<(u32, Vec2)> = self.enemies.iter().filter(|e| !before.contains(&e.id) && !e.is_boss()).map(|e| (e.id, e.position(now))).collect();
        let next_at = if w.gap > 0.0 { now + w.gap } else { f32::INFINITY };
        if let Some(bar) = w.bar.as_ref().filter(|_| next_at.is_finite()) {
            let label = if step + 1 < w.waves.len() {
                format!("{bar} {}/{}", step + 2, w.waves.len())
            } else {
                match self.enemies[k].kind {
                    EnemyKind::Boss(b) => self.content.zones[self.zone].bosses[b].name.clone(),
                    _ => bar.clone(),
                }
            };
            self.push(Event::BarStarted { boss: id, key: WAVE_BAR_KEY, label, icon_spell: 0, until: next_at });
        }
        let Some(k) = self.enemies.iter().position(|e| e.id == id) else { return };
        let total = adds.len();
        if let Some(run) = &mut self.enemies[k].mech.phases.waves {
            run.step = step;
            run.total = total;
            run.adds = adds;
            run.told = total;
            run.next_at = next_at;
        }
        if let Some(label) = &w.counter {
            self.push(Event::CounterShown { boss: id, key: WAVE_COUNTER_KEY, label: format!("{label} {total}/{total}") });
        }
    }

    /// Boss `k`'s run of waves goes on: its fallen adds are noted, the counter told, and the
    /// next wave (or its end) comes once they are all dead or the gap ran out.
    pub(crate) fn waves_tick(&mut self, k: usize) {
        let now = self.now;
        let id = self.enemies[k].id;
        let Some(mut w) = self.enemies[k].mech.phases.waves.take() else { return };
        let mut fallen = Vec::new();
        w.adds.retain_mut(|(add, p)| match self.enemies.iter().find(|e| e.id == *add) {
            Some(e) => {
                *p = e.position(now);
                true
            }
            None => {
                fallen.push(*p);
                false
            }
        });
        let left = w.adds.len();
        let counter = (left != w.told).then(|| w.counter.clone()).flatten();
        w.told = left;
        let (done, step, last) = (left == 0 || now >= w.next_at, w.step, w.step + 1 >= w.waves.len());
        let e = &mut self.enemies[k];
        e.mech.phases.fallen.extend(fallen);
        e.mech.phases.waves = Some(w);
        if let Some(label) = counter {
            let total = self.enemies[k].mech.phases.waves.as_ref().map_or(0, |w| w.total);
            self.push(Event::CounterShown { boss: id, key: WAVE_COUNTER_KEY, label: format!("{label} {left}/{total}") });
        }
        if !done {
            return;
        }
        if !last {
            self.start_wave(id, step + 1);
            return;
        }
        // the last wave is down: the boss fights
        let w = self.enemies[k].mech.phases.waves.take().unwrap();
        let e = &mut self.enemies[k];
        e.mech.phases.immune = false;
        if w.look_spell != 0 {
            e.kit.worn_looks.retain(|(l, _)| *l != w.look_spell);
        }
        if w.aloof {
            e.mech.busy_until = now;
            for s in &mut e.abilities {
                s.timer = s.ability.cooldown * super::boss_abilities::BOSS_FIRST_CAST;
            }
        }
        self.push(Event::BarsCleared { boss: id });
        let at = self.enemies[k].position(now);
        self.run_actions(Some(id), at, &w.then);
        if let Some(k) = self.enemies.iter().position(|e| e.id == id) {
            self.boss_bars(k);
        }
    }

    /// A class call: each living hero whose class has a call is announced it and twisted for
    /// `seconds` (it deals less damage), and the call's actions run.
    fn class_call(&mut self, source: Option<u32>, at: Vec2, calls: &[ClassCall], seconds: f32) {
        let now = self.now;
        let content = self.content.clone();
        let mut called: Vec<&ClassCall> = Vec::new();
        for h in 0..self.heroes.len() {
            if !self.heroes[h].alive {
                continue;
            }
            let class = &content.classes[self.heroes[h].class].id;
            let Some(call) = calls.iter().find(|c| &c.class == class) else { continue };
            if call.weakens > 0.0 || call.look_spell != 0 {
                let spec = esurv_content::Affliction { look_spell: call.look_spell, damage_mult: 0.0, every: seconds.max(0.1), seconds, max_stacks: 1 };
                let hero = &mut self.heroes[h];
                hero.afflictions.retain(|a| a.spec.look_spell != call.look_spell);
                hero.afflictions.push(HeroAffliction { spec, stacks: 1, per_stack: 0.0, next_tick: now + seconds + 1.0, until: now + seconds, weakens: call.weakens });
            }
            if !called.contains(&call) {
                called.push(call);
            }
        }
        for call in called {
            self.announce_warning(&call.warn, call.look_spell);
            self.run_actions(source, at, &call.run);
        }
    }

    /// Figure `spec` (a `Cameo`) rises at `at`: it takes no damage, never moves or swings, says
    /// its lines and fights with its kit until it sinks.
    fn cameo_rises(&mut self, spec: &ScriptAction, at: Vec2, attack: f32) {
        let ScriptAction::Cameo { enemy, at: place, seconds, rise_spell, sink_spell, run, resolved, .. } = spec else { return };
        let Some(index) = self.roster_index(enemy) else { return };
        let spot = self.wave_spot(*place, at);
        if !self.spawn_summoned(index, spot) {
            return;
        }
        let now = self.now;
        let e = self.enemies.last_mut().unwrap();
        let id = e.id;
        e.mech.phases.immune = true;
        e.mech.phases.rooted = true;
        e.mech.phases.sinks_at = Some((now + seconds, *sink_spell));
        e.swing_timer = f32::INFINITY;
        if attack > 0.0 {
            e.damage = attack;
        }
        e.abilities = resolved.iter().map(AbilitySlot::boss).collect::<Vec<_>>();
        if *rise_spell != 0 {
            self.push(Event::SpellGo { caster: Unit::Enemy(id), spell: *rise_spell, hits: vec![], aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
        }
        self.run_actions(Some(id), spot, run);
    }

    /// The spot `place` gives round `at` (a ring's first spot, the hero's side), for one figure.
    fn wave_spot(&mut self, place: WaveAt, at: Vec2) -> Vec2 {
        match place {
            WaveAt::Ring(r) if r > 0.0 => {
                let a = self.rng.range(0.0, std::f32::consts::TAU);
                (at.0 + a.cos() * r, at.1 + a.sin() * r)
            }
            WaveAt::Spot { x, y, .. } => (x, y),
            WaveAt::NearHero(r) => {
                let h = self.anchor_hero().map_or(at, |h| self.heroes[h].pos);
                let a = self.rng.range(0.0, std::f32::consts::TAU);
                (h.0 + a.cos() * r, h.1 + a.sin() * r)
            }
            _ => at,
        }
    }

    /// Figures due rise; risen ones whose time is up sink (their look a little before), giving
    /// nothing.
    pub(crate) fn cameos_tick(&mut self) {
        let now = self.now;
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.cameos_due).into_iter().partition(|(t, _, _)| now >= *t);
        self.cameos_due = later;
        for (_, spec, at) in due {
            self.cameo_rises(&spec, at, self.fallen_attack);
        }
        let mut looks = Vec::new();
        let mut gone = Vec::new();
        for e in &mut self.enemies {
            if let Some((t, look)) = e.mech.phases.sinks_at {
                if look != 0 && now >= t - SINK_LOOK_AHEAD {
                    looks.push((e.id, look));
                    e.mech.phases.sinks_at = Some((t, 0));
                }
                if now >= t {
                    gone.push(e.id);
                }
            }
        }
        for (id, look) in looks {
            self.push(Event::SpellGo { caster: Unit::Enemy(id), spell: look, hits: vec![], aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
        }
        self.enemies.retain(|e| !gone.contains(&e.id));
    }

    /// Hero `h`'s twisted share now: how much less damage it deals (a class call).
    pub(crate) fn hero_weakened(&self, h: usize) -> f32 {
        let now = self.now;
        self.heroes[h].afflictions.iter().filter(|a| now < a.until).map(|a| a.weakens).fold(0.0, f32::max).clamp(0.0, 0.95)
    }
}
