//! Boss scripts (docs/game/bosses.md, "Building blocks"): what a boss
//! does as it arrives, at a health share, on a timer and as it dies, written in its zone file:
//! yells with the original boss's voice, waves of adds, stopping or swapping the zone's spawns,
//! a shield until a relic is taken, a phase change (a new kit, weapons, look), kneeling, enrage and
//! the scripted pause. Raise Dead is the one script acting on the horde; every other
//! action leaves the normal enemies alone.

use esurv_content::{CreatureType, CreatureWeapons, ScriptAction, ScriptTrigger, WaveAt, SOON_AHEAD};

use super::enemy_ai::{Segment, Vec2};
use super::enemy_attacks::AbilitySlot;
use super::run::{EnemyKind, Event, Run, Speaker, Unit};

/// What a boss's script and building blocks keep on it.
#[derive(Debug, Clone, PartialEq)]
pub struct BossState {
    /// It stands still and does nothing until then (winding up a mark round itself, kneeling).
    pub busy_until: f32,
    /// Kneeling until then (told to the client as it starts and ends).
    pub kneel_until: Option<f32>,
    /// Shielded: the share of each hit it takes, and the look it wears.
    pub shield: Option<(f32, u32)>,
    /// The shield is down until then.
    pub mortal_until: f32,
    /// Each script line: run already (a once-only line), its early warning given, and when a
    /// timed line runs next.
    pub fired: Vec<bool>,
    pub warned: Vec<bool>,
    pub next_at: Vec<f32>,
    /// Weapons and look it took (a phase change), in place of its zone file's.
    pub weapons: Option<CreatureWeapons>,
    pub display_id: Option<u32>,
    /// Its melee swing's multiplier (`Boss::swing_mult`).
    pub swing_mult: f32,
    /// The size it took with a new look.
    pub scale: Option<f32>,
    /// Its phases: waves, immunity, rage (`game::boss_phases`).
    pub phases: super::boss_phases::PhaseState,
}

impl Default for BossState {
    fn default() -> Self {
        Self {
            busy_until: 0.0,
            kneel_until: None,
            shield: None,
            mortal_until: 0.0,
            fired: Vec::new(),
            warned: Vec::new(),
            next_at: Vec::new(),
            weapons: None,
            display_id: None,
            swing_mult: 1.0,
            scale: None,
            phases: Default::default(),
        }
    }
}

impl BossState {
    /// The share of a hit it takes now: its shield's, unless the shield is down; none while it
    /// is immune.
    pub fn damage_share(&self, now: f32) -> f32 {
        if self.phases.immune {
            return 0.0;
        }
        match self.shield {
            Some((share, _)) if now >= self.mortal_until => share,
            _ => 1.0,
        }
    }
}

/// The run's spawn rules a script set.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpawnRules {
    /// The zone's own spawns are stopped (the director, the floor, elites, swarms).
    pub stopped: bool,
    /// Only roster enemies of this type spawn, for the rest of the run.
    pub only: Option<CreatureType>,
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Run {
    /// Boss `k` has just arrived: its script's state, and its arrival lines run.
    pub(crate) fn boss_arrives(&mut self, k: usize) {
        let EnemyKind::Boss(b) = self.enemies[k].kind else { return };
        let now = self.now;
        let boss = &self.content.zones[self.zone].bosses[b];
        let lines = boss.script.len();
        let next_at = boss
            .script
            .iter()
            .map(|l| match l.on {
                ScriptTrigger::Every(s) | ScriptTrigger::After(s) => now + s,
                _ => f32::INFINITY,
            })
            .collect();
        let (swing_mult, id) = (boss.swing_mult, self.enemies[k].id);
        self.enemies[k].mech = BossState { fired: vec![false; lines], warned: vec![false; lines], next_at, swing_mult, ..Default::default() };
        self.boss_bars(k);
        let at = self.enemies[k].position(now);
        let arrival: Vec<usize> = (0..lines).filter(|&i| self.content.zones[self.zone].bosses[b].script[i].on == ScriptTrigger::Arrival).collect();
        for i in arrival {
            self.run_script_line(id, b, i, at);
        }
    }

    /// Boss `k`'s health and timed lines, and the end of its kneel.
    pub(crate) fn boss_script_tick(&mut self, mut k: usize) {
        let EnemyKind::Boss(b) = self.enemies[k].kind else { return };
        let now = self.now;
        let e = &self.enemies[k];
        let id = e.id;
        if e.mech.kneel_until.is_some_and(|t| now >= t) {
            self.enemies[k].mech.kneel_until = None;
            self.push(Event::BossKneels { id, kneeling: false });
        }
        // a run of waves goes on (`game::boss_phases`)
        if self.enemies[k].mech.phases.waves.is_some() {
            self.waves_tick(k);
            let Some(again) = self.enemies.iter().position(|e| e.id == id) else { return };
            k = again;
        }
        let e = &self.enemies[k];
        let pct = e.health / e.max_health.max(1.0) * 100.0;
        let at = e.position(now);
        let content = self.content.clone();
        let script = &content.zones[self.zone].bosses[b].script;
        for (i, line) in script.iter().enumerate() {
            let Some(e) = self.enemies.iter().find(|e| e.id == id) else { return };
            if i >= e.mech.fired.len() {
                return;
            }
            match line.on {
                ScriptTrigger::Health(at_pct) if !e.mech.fired[i] => {
                    if !e.mech.warned[i] && pct <= at_pct + SOON_AHEAD && pct > at_pct {
                        if let Some(text) = &line.soon {
                            let icon = line.warn.as_ref().and_then(|w| w.icon_spell).unwrap_or(0);
                            let colour = line.warn.as_ref().map_or(esurv_content::WarningColour::Purple, |w| w.colour);
                            self.push(Event::Warning { text: text.clone(), colour, icon_spell: icon, soon: true });
                        }
                        if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                            e.mech.warned[i] = true;
                        }
                    }
                    if pct <= at_pct {
                        if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                            e.mech.fired[i] = true;
                            e.mech.warned[i] = true;
                        }
                        self.run_script_line(id, b, i, at);
                    }
                }
                // a boss standing apart behind its waves keeps its timed lines for later
                ScriptTrigger::Every(s) | ScriptTrigger::After(s) if e.mech.phases.aloof() => {
                    if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                        e.mech.next_at[i] = now + s;
                    }
                }
                // a phase's own timer stops below its health share, and its bar runs out
                ScriptTrigger::Every(_) if line.until_health.is_some_and(|u| pct < u) && e.mech.next_at[i].is_finite() => {
                    if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                        e.mech.next_at[i] = f32::INFINITY;
                    }
                    if let Some(w) = &line.warn {
                        self.push(Event::BarStarted { boss: id, key: SCRIPT_BAR_KEYS + i as u32, label: w.bar_label().into(), icon_spell: w.icon_spell.unwrap_or(0), until: now });
                    }
                }
                // a phase's own timer waits for its health share
                ScriptTrigger::Every(_) if line.from_health.is_some_and(|f| pct > f) => {}
                ScriptTrigger::Every(s) | ScriptTrigger::After(s) if now >= e.mech.next_at[i] => {
                    let again = if matches!(line.on, ScriptTrigger::Every(_)) { now + s } else { f32::INFINITY };
                    if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                        e.mech.next_at[i] = again;
                    }
                    self.run_script_line(id, b, i, at);
                    if again.is_finite() {
                        if let Some(w) = &line.warn {
                            self.push(Event::BarStarted { boss: id, key: SCRIPT_BAR_KEYS + i as u32, label: w.bar_label().into(), icon_spell: w.icon_spell.unwrap_or(0), until: again });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Boss `b` (id `id`, attack damage `attack`) fell at `at`: its bars go and its death lines
    /// run (their yells from where it fell, their figures with its attack damage).
    pub(crate) fn boss_dies(&mut self, id: u32, b: usize, at: Vec2, attack: f32) {
        self.push(Event::BarsCleared { boss: id });
        let lines: Vec<usize> = (0..self.content.zones[self.zone].bosses[b].script.len()).filter(|&i| self.content.zones[self.zone].bosses[b].script[i].on == ScriptTrigger::Death).collect();
        self.fallen_attack = attack;
        self.fallen_boss = Some(id);
        for i in lines {
            self.run_script_line(id, b, i, at);
        }
        self.fallen_boss = None;
    }

    /// Whether boss `b`'s death lines yell (its own last words, in place of the standard ones).
    pub(crate) fn dies_yelling(&self, b: usize) -> bool {
        self.content.zones[self.zone].bosses[b].script.iter().any(|l| l.on == ScriptTrigger::Death && l.run.iter().any(|a| matches!(a, ScriptAction::Yell { .. })))
    }

    fn run_script_line(&mut self, id: u32, b: usize, i: usize, at: Vec2) {
        let content = self.content.clone();
        let line = &content.zones[self.zone].bosses[b].script[i];
        if let Some(w) = &line.warn {
            let icon = w.icon_spell.unwrap_or(0);
            self.push(Event::Warning { text: w.text.clone(), colour: w.colour, icon_spell: icon, soon: false });
        }
        self.run_actions(Some(id), at, &line.run);
    }

    /// Runs `actions` for boss `source` (where it stands, or `at` once it is gone).
    pub(crate) fn run_actions(&mut self, source: Option<u32>, at: Vec2, actions: &[ScriptAction]) {
        self.run_actions_at(source, at, actions, false);
    }

    /// [`Run::run_actions`]; with `here`, at `at` even while the boss stands elsewhere (an
    /// object's pulse, an add's death).
    pub(crate) fn run_actions_at(&mut self, source: Option<u32>, at: Vec2, actions: &[ScriptAction], here: bool) {
        let now = self.now;
        let k = source.and_then(|s| self.enemies.iter().position(|e| e.id == s));
        let at = if here { at } else { k.map_or(at, |k| self.enemies[k].position(now)) };
        // a boss gone (its death lines) acts with the attack damage it fell with
        let attack = k.map_or(self.fallen_attack, |k| {
            let e = &self.enemies[k];
            super::enemy_ai::attack_damage(e.damage, super::run::EnemyUnit::active(e.weaken, now), e.is_boss() && e.enraged)
        });
        for action in actions {
            let k = source.and_then(|s| self.enemies.iter().position(|e| e.id == s));
            match action {
                ScriptAction::Yell { text, sound } => {
                    // a boss's last words come from where it fell
                    if let Some(id) = source.filter(|s| k.is_some() || self.fallen_boss == Some(*s)) {
                        self.push(Event::Yell { id, text: text.clone() });
                    }
                    if let Some(s) = sound {
                        self.push(Event::Sound(*s));
                    }
                }
                ScriptAction::Emote { text } => {
                    let from = match source.filter(|_| k.is_some()) {
                        Some(id) => Speaker::Unit(Unit::Enemy(id)),
                        None => Speaker::Named(String::new()),
                    };
                    self.push(Event::Emote { from, text: text.clone() });
                }
                ScriptAction::Wave { enemy, count, at: place } => match place {
                    WaveAt::Ring(r) => {
                        self.wave_at_spot(enemy, *count, at, *r);
                    }
                    WaveAt::Sides(packs) => self.wave_from_sides(source, enemy, *count, *packs),
                    WaveAt::Spot { x, y, spread } => self.wave_at_spot(enemy, *count, (*x, *y), *spread),
                    WaveAt::NearHero(r) => {
                        if let Some(h) = self.anchor_hero() {
                            let p = self.heroes[h].pos;
                            self.wave_at_spot(enemy, *count, p, *r);
                        }
                    }
                    WaveAt::Edges => {
                        let Some(index) = self.roster_index(enemy) else { continue };
                        let Some(h) = self.anchor_hero() else { continue };
                        let ring = self.zone().enemy_spawn_ring;
                        for _ in 0..*count {
                            let p = self.spawn_point(h, ring);
                            self.spawn_summoned(index, p);
                        }
                    }
                },
                ScriptAction::Door { name, open } => {
                    if let Some(d) = self.zone().doors.iter().position(|d| d.name == *name) {
                        self.set_door(d, *open);
                    }
                }
                ScriptAction::StopSpawns => self.spawn_rules.stopped = true,
                ScriptAction::ResumeSpawns => self.spawn_rules.stopped = false,
                ScriptAction::Roster { only } => self.spawn_rules.only = Some(*only),
                ScriptAction::RaiseDead { within, rise_as, look_spell } => self.raise_dead(*within, rise_as, *look_spell),
                ScriptAction::Shield { damage_taken, look_spell } => {
                    if let Some(k) = k {
                        let e = &mut self.enemies[k];
                        e.mech.shield = Some((*damage_taken, *look_spell));
                        e.mech.mortal_until = 0.0;
                        e.kit.worn_looks.retain(|(l, _)| l != look_spell);
                        e.kit.worn_looks.push((*look_spell, f32::INFINITY));
                    }
                }
                ScriptAction::Mortal { seconds } => {
                    if let Some(k) = k {
                        let e = &mut self.enemies[k];
                        e.mech.mortal_until = now + seconds;
                        if let Some((_, look)) = e.mech.shield {
                            // its shield look goes while it is down, and comes back with it
                            e.kit.worn_looks.retain(|(l, _)| *l != look);
                            self.mortal_looks.push((e.id, look, now + seconds));
                        }
                    }
                }
                ScriptAction::Place { object, near, count } => {
                    let n = count.unwrap_or(1).max(1);
                    for i in 0..n {
                        self.place_object_spread(object.clone(), *near, source, attack, (i, n));
                    }
                }
                ScriptAction::LayZone { zone } => {
                    if let Some(id) = source.filter(|_| k.is_some()) {
                        self.lay_zone(zone, at, id, attack);
                    }
                }
                ScriptAction::Kit { resolved, .. } => {
                    if let Some(k) = k {
                        self.enemies[k].abilities = resolved.iter().map(AbilitySlot::boss).collect();
                        let id = self.enemies[k].id;
                        self.push(Event::BarsCleared { boss: id });
                        self.boss_bars(k);
                    }
                }
                ScriptAction::Weapons { weapons } => {
                    if let Some(k) = k {
                        self.enemies[k].mech.weapons = Some(*weapons);
                        let id = self.enemies[k].id;
                        self.push(Event::BossChanged { id });
                    }
                }
                ScriptAction::Look { display_id, scale } => {
                    if let Some(k) = k {
                        self.enemies[k].mech.display_id = Some(*display_id);
                        if scale.is_some() {
                            self.enemies[k].mech.scale = *scale;
                        }
                        let id = self.enemies[k].id;
                        self.push(Event::BossChanged { id });
                    }
                }
                ScriptAction::Kneel { seconds } => {
                    if let Some(k) = k {
                        let until = now + seconds;
                        self.hold_boss_still(k, until);
                        self.enemies[k].mech.kneel_until = Some(until);
                        let id = self.enemies[k].id;
                        self.push(Event::BossKneels { id, kneeling: true });
                    }
                }
                ScriptAction::Enrage => {
                    if let Some(k) = k {
                        self.enemies[k].enraged = true;
                    }
                }
                ScriptAction::Pause { seconds, text } => self.scripted_pause(*seconds, text),
                // the later zones' actions (`boss_script_actions`)
                ScriptAction::Later { .. }
                | ScriptAction::On { .. }
                | ScriptAction::Call { .. }
                | ScriptAction::Revive { .. }
                | ScriptAction::Unshield
                | ScriptAction::Heal { .. }
                | ScriptAction::Cast { .. }
                | ScriptAction::ClearZones
                | ScriptAction::Wear { .. }
                | ScriptAction::Speed { .. }
                | ScriptAction::Bolster { .. }
                | ScriptAction::Siphon { .. } => self.run_more_action(source, at, here, action),
                // waves, immunity, rage, heals, class calls, the fallen, figures (`boss_phases`)
                other => self.run_phase_action(source, k, at, attack, other),
            }
        }
    }

    /// The roster index of the zone's enemy named `name`.
    pub(crate) fn roster_index(&self, name: &str) -> Option<usize> {
        self.zone().enemies.iter().position(|e| e.name == name)
    }

    /// `count` of roster enemy `name` on a ring `radius` yards round `at`.
    pub(crate) fn wave_at_spot(&mut self, name: &str, count: u32, at: Vec2, radius: f32) {
        let Some(index) = self.roster_index(name) else { return };
        let turn = self.rng.range(0.0, std::f32::consts::TAU);
        for i in 0..count {
            let a = turn + i as f32 / count.max(1) as f32 * std::f32::consts::TAU;
            self.spawn_summoned(index, (at.0 + a.cos() * radius, at.1 + a.sin() * radius));
        }
    }

    /// Raise Dead: every living enemy that is not undead within `within`
    /// yards of a hero dies, giving nothing, and rises at its spot as roster enemy `rise_as`.
    pub(crate) fn raise_dead(&mut self, within: f32, rise_as: &str, look_spell: u32) {
        let Some(index) = self.roster_index(rise_as) else { return };
        let now = self.now;
        let content = self.content.clone();
        let zone = &content.zones[self.zone];
        let heroes: Vec<Vec2> = self.heroes.iter().filter(|h| h.alive).map(|h| h.pos).collect();
        let fallen: Vec<(u32, Vec2)> = self
            .enemies
            .iter()
            .filter(|e| match e.kind {
                EnemyKind::Roster(r) | EnemyKind::Rare { roster: r } => zone.enemies[r].creature_type != CreatureType::Undead,
                EnemyKind::Boss(_) => false,
            })
            .map(|e| (e.id, e.position(now)))
            .filter(|(_, p)| heroes.iter().any(|h| dist(*h, *p) <= within))
            .collect();
        for (id, p) in fallen {
            self.enemies.retain(|e| e.id != id);
            self.push(Event::EnemyDied { id, by: None });
            if self.spawn_summoned(index, p) {
                let risen = self.enemies.last().map_or(0, |e| e.id);
                self.push(Event::SpellGo { caster: Unit::Enemy(risen), spell: look_spell, hits: vec![], aim: super::run::Aim::Caster, ammo: false, reach: 0.0 });
            }
        }
    }

    /// Spawns roster enemy `index` at `at` for a script, past the live cap and the spawn rules.
    pub(crate) fn spawn_summoned(&mut self, index: usize, at: Vec2) -> bool {
        self.spawn_roster_enemy_with(index, Some(at), true)
    }

    /// The roster entry that spawns in place of `index` under the spawn rules: itself when it may,
    /// else the allowed one of its rank (normal or elite) already unlocked with the nearest cost;
    /// `None` when none may.
    pub(crate) fn allowed_roster(&self, index: usize) -> Option<usize> {
        let zone = self.zone();
        let e = zone.enemies.get(index)?;
        let Some(only) = self.spawn_rules.only else { return Some(index) };
        if e.creature_type == only {
            return Some(index);
        }
        let minute = self.now / 60.0;
        zone.enemies
            .iter()
            .enumerate()
            .filter(|(_, o)| o.creature_type == only && o.elite == e.elite && !o.only_summoned && o.appears_at_minute <= minute)
            .min_by(|a, b| (a.1.spawn_cost - e.spawn_cost).abs().total_cmp(&(b.1.spawn_cost - e.spawn_cost).abs()))
            .map(|(i, _)| i)
    }

    /// Boss `k`'s timer bars: each warned ability's next use (none while it stands apart behind
    /// its waves).
    pub(crate) fn boss_bars(&mut self, k: usize) {
        let now = self.now;
        let id = self.enemies[k].id;
        if self.enemies[k].mech.phases.aloof() {
            return;
        }
        let bars: Vec<Event> = self.enemies[k]
            .abilities
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let w = s.ability.kind.warning()?;
                let icon = w.icon_spell.unwrap_or_else(|| super::telegraphs::kind_look(&s.ability.kind));
                Some(Event::BarStarted { boss: id, key: i as u32, label: w.bar_label().into(), icon_spell: icon, until: now + s.timer.max(0.0) })
            })
            .collect();
        for b in bars {
            self.push(b);
        }
        let EnemyKind::Boss(b) = self.enemies[k].kind else { return };
        let content = self.content.clone();
        for (i, line) in content.zones[self.zone].bosses[b].script.iter().enumerate() {
            if let (ScriptTrigger::Every(_) | ScriptTrigger::After(_), Some(w)) = (line.on, &line.warn) {
                let until = self.enemies[k].mech.next_at.get(i).copied().unwrap_or(f32::INFINITY);
                // a phase's own timer shows once its phase has come
                let e = &self.enemies[k];
                let waiting = line.from_health.is_some_and(|f| e.health / e.max_health.max(1.0) * 100.0 > f);
                if until.is_finite() && !waiting {
                    self.push(Event::BarStarted { boss: id, key: SCRIPT_BAR_KEYS + i as u32, label: w.bar_label().into(), icon_spell: w.icon_spell.unwrap_or(0), until });
                }
            }
        }
    }

    /// Shield looks come back as the shield does.
    pub(crate) fn mortal_looks_tick(&mut self) {
        let now = self.now;
        let (back, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.mortal_looks).into_iter().partition(|(_, _, t)| now >= *t);
        self.mortal_looks = later;
        for (id, look, _) in back {
            if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                e.kit.worn_looks.push((look, f32::INFINITY));
            }
        }
    }

    /// A boss holding still for its script or a mark: its segment stops.
    pub(crate) fn stand_still(&mut self, k: usize) {
        let now = self.now;
        let e = &mut self.enemies[k];
        if e.segment.moving(now) {
            let p = e.position(now);
            e.segment = Segment::standing(p, now);
        }
    }
}

/// Script lines' timer bars are numbered from here (an ability's by its place in the kit).
pub const SCRIPT_BAR_KEYS: u32 = 100;
