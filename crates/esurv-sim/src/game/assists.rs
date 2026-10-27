//! Assists (docs/game/assists.md): battle standards lie on the field; a hero who walks over one
//! summons a hand-made player character of the hero's own faction (`player_characters.ron`), which
//! runs its assist script (`Content::assist_of`) beside the hero and leaves.
//!
//! - **The standards:** `balance.battle_standard.per_run` of them a run (drawn from the run's seed
//!   on its own, as the rares' count), each at a random second between the first elite and the
//!   final boss's minute; in Endless one more every `endless_every` seconds. They lie on the
//!   caches' ring, never in a zone `never_in` names, until a hero walks over one (`TAKE_RANGE`).
//! - **The ally:** a character of the taker's faction not yet summoned this run (all of them again
//!   once each has come). It runs its steps one after the other, and leaves when they are done and
//!   no taunt of its own holds, or at its time (`seconds`, else `assist_seconds`). Nothing hurts
//!   it; enemies come at it only while it taunts them.
//! - **Its damage** is its own (`assist_hit`, `HitSource::Assist(character)`): no hero bonus, no
//!   crit, nothing of the hero's set off; its kills give the hero their rewards. Every damage event
//!   deals `balance.battle_standard.assist_damage_per_second` for the enemy's kind × its max health
//!   × the seconds the event stands for (`assist_damage`). Its crowd control
//!   on bosses and rares goes through `apply_on_hit`, the hero's own rules; on other enemies a stun
//!   or root holds its full time (`assist_effect`).

use esurv_content::{AreaAt, AssistStep, Leave, Leaving, MoveStyle, Movement, OverTime, SpellEffect, Stat, Targets};

use super::assist_rotation::{Rotation, GLOBAL_COOLDOWN};

use super::boss_abilities::CHARGE_SPEED;
use super::casting::{HitSource, Totem, AREA_LOOK_AFTER, AREA_SPOTS, HOLD_BOSS_SHARE, TOTEM_OFFSET};
use super::director::FIRST_ELITE;
use super::enemy_ai::{chase, reach, Segment, Vec2};
use super::loot::{Pickup, CACHE_RING};
use super::pets::{PET_APPEAR_DISTANCE, PET_CHASE_STOP_SHORT};
use super::rares::FALLBACK_RUN_MINUTES;
use super::run::{Aim, Buff, BuffKind, EnemyUnit, Event, Run, Unit};
use super::stats::{Modifier, HERO_RUN_SPEED};
use crate::rng::Rng;

/// Mixed into the run's seed for the standards' own draws (their count and times), so the run's
/// other draws stay as they were.
const STANDARD_SEED: u64 = 0xBA77_1E57;
/// The ally appears this far beside the hero (where a pet appears) and stands this far short of
/// an enemy it goes to or strikes (where a melee pet stops).
pub const ALLY_BESIDE_HERO: f32 = PET_APPEAR_DISTANCE;
pub const ALLY_BESIDE_ENEMY: f32 = PET_CHASE_STOP_SHORT;
/// It runs at the hero's base run speed and charges at a boss's charge speed.
pub const ALLY_RUN_SPEED: f32 = HERO_RUN_SPEED;
pub const ALLY_CHARGE_SPEED: f32 = CHARGE_SPEED;
/// A charge's look unless the script names one (Charge).
pub const CHARGE_LOOK: u32 = 100;
/// The ways of leaving with a look of their own: Vanish's and the Hearthstone's.
pub const VANISH_LOOK: u32 = 1856;
pub const HEARTH_LOOK: u32 = 8690;
/// At most this many steps of one ally run in a tick (a repeat of instant steps every 0 s ends).
const STEPS_PER_TICK: usize = 100;

/// How the ally gets where it goes (`Event::AllyMoved`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllyMove {
    /// At a run.
    Run,
    /// A charge: a fast dash in a straight line.
    Charge,
    /// A blink: there at once.
    Blink,
    /// A step beside the enemy it strikes in melee: there at once.
    Step,
    /// A strafe to its left or right, facing its enemy (its rotation in melee).
    StrafeLeft,
    StrafeRight,
    /// A jump: through the air to where it lands.
    Jump,
}

/// Steps being run: the script's own, or a repeat's.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub steps: Vec<AssistStep>,
    /// The next step to run.
    pub next: usize,
    /// A repeat's passes, the gap between their starts, when its first began and how many have
    /// begun. The script's own steps are one pass.
    pub times: u32,
    pub every: f32,
    pub started: f32,
    pub passes: u32,
    /// The enemies picked before the repeat (a `Same` at the start of each pass means them).
    pub pick_before: Vec<u32>,
}

/// An ally on the field.
#[derive(Debug, Clone, PartialEq)]
pub struct Ally {
    pub id: u32,
    /// The hero it came for (an index in `Run::heroes`).
    pub hero: usize,
    /// Its character's id in `player_characters.ron`.
    pub character: String,
    pub segment: Segment,
    pub facing: f32,
    /// Its stay ends then (its arrival plus its script's time, else `assist_seconds`): it leaves
    /// then, or once the last cast of its rotation lands when its script ran late.
    pub until: f32,
    /// How it makes the move its segment is (a run, a strafe, a jump).
    pub moving: AllyMove,
    /// Its next step waits until then.
    pub busy_until: f32,
    pub frames: Vec<Frame>,
    /// The enemies the last picking step picked.
    pub picked: Vec<u32>,
    /// The enemies its taunts hold, each until when.
    pub taunts: Vec<(u32, f32)>,
    pub taunting: bool,
    pub leave: Leaving,
    /// What it does once its script is done (`game::assist_rotation`).
    pub rotation: Rotation,
    /// The share of its script's heals the script heals (`Rotation::share`), each of its
    /// rotation's casts healing as much again.
    pub share: f32,
}

/// A ground area an ally left.
#[derive(Debug, Clone, PartialEq)]
pub struct AllyArea {
    pub id: u32,
    pub ally: u32,
    pub hero: usize,
    pub character: String,
    pub at: Vec2,
    pub radius: f32,
    pub every: f32,
    pub next: f32,
    pub until: f32,
    /// Each pulse deals the assist's damage for `every` seconds.
    pub damage: bool,
    pub effects: Vec<SpellEffect>,
    /// Its look (`Spell.dbc`): the same ally laying one of the same look again ends this one.
    pub spell: u32,
    /// It strikes at most this many each time (0: every enemy inside), the nearest its middle first.
    pub most: u32,
}

/// Damage over time an ally's hit left on an enemy: each tick the assist's damage for `every`
/// seconds. The same assist putting it on again renews it.
#[derive(Debug, Clone, PartialEq)]
pub struct AllyDot {
    pub enemy: u32,
    pub hero: usize,
    pub character: String,
    pub every: f32,
    pub next: f32,
    pub until: f32,
}

/// A buff an ally put on a hero.
#[derive(Debug, Clone, PartialEq)]
pub struct AllyBuff {
    pub hero: usize,
    pub stats: Vec<(Stat, f32)>,
    /// Heals this share of max health every `every` seconds, the first after `every`.
    pub heal: f32,
    pub every: f32,
    pub next_heal: f32,
    pub until: f32,
}

/// A bolt an assist's ranged pet cast, landing at `at` on `enemy` for the assist's damage for
/// `seconds` (the pet's attack interval).
#[derive(Debug, Clone, PartialEq)]
pub struct AllyBolt {
    pub hero: usize,
    pub pet: u32,
    pub enemy: u32,
    pub at: f32,
    pub seconds: f32,
}

/// The run-wide state of battle standards and allies.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Assists {
    /// When the battle standards still to come are due (run seconds).
    pub standards_due: Vec<f32>,
    /// In Endless, when the next one comes.
    pub next_endless: Option<f32>,
    /// The allies on the field.
    pub allies: Vec<Ally>,
    /// The characters summoned this run (ids), in order.
    pub summoned: Vec<String>,
    /// A development aid: while not empty, the next standard taken summons its first character
    /// (an id; any faction), taken off the list, in place of the random pick.
    pub forced: Vec<String>,
    pub areas: Vec<AllyArea>,
    pub dots: Vec<AllyDot>,
    pub buffs: Vec<AllyBuff>,
    /// Bolts of assists' pets on their way.
    pub bolts: Vec<AllyBolt>,
    /// While one damage event of several steps runs (a rotation cast landing, a channel's tick):
    /// the enemies it has struck, each taking its damage once.
    pub event_struck: Option<Vec<u32>>,
}

/// When a run's battle standards are due: `per_run.0` to `per_run.1` of them (evenly), each at a
/// random second between the first elite and the final boss's minute (the rares' window), sorted.
pub fn standard_schedule(per_run: (u32, u32), final_minute: Option<f32>, rng: &mut Rng) -> Vec<f32> {
    let (lo, hi) = (per_run.0.min(per_run.1), per_run.0.max(per_run.1));
    let count = lo + (rng.next_u64() % u64::from(hi - lo + 1)) as u32;
    let end = (final_minute.unwrap_or(FALLBACK_RUN_MINUTES) * 60.0).max(FIRST_ELITE);
    let mut times: Vec<f32> = (0..count).map(|_| rng.range(FIRST_ELITE, end)).collect();
    times.sort_by(f32::total_cmp);
    times
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// The facing from `from` to `to`; `otherwise` when they are one spot.
fn facing(from: Vec2, to: Vec2, otherwise: f32) -> f32 {
    if dist(from, to) < 1e-3 {
        otherwise
    } else {
        (to.1 - from.1).atan2(to.0 - from.0)
    }
}

/// `d` yards short of `to` on the way from `from` (where it stands when nearer than that).
fn short_of(from: Vec2, to: Vec2, d: f32) -> Vec2 {
    let len = dist(from, to);
    if len <= d {
        return from;
    }
    let k = (len - d) / len;
    (from.0 + (to.0 - from.0) * k, from.1 + (to.1 - from.1) * k)
}

impl Run {
    /// The run's battle standards, drawn when it opens (none in a zone that never lays them).
    pub(crate) fn plan_standards(&mut self) {
        if !self.content.battle_standards_in(&self.zone().id) {
            return;
        }
        let final_minute = self.zone().bosses.iter().find(|b| b.is_final).map(|b| b.spawn_minute);
        let per_run = self.content.balance.battle_standard.per_run;
        self.assists.standards_due = standard_schedule(per_run, final_minute, &mut Rng::new(self.seed ^ STANDARD_SEED));
    }

    /// The battle standards whose time came are laid; in Endless one more each `endless_every`.
    pub(crate) fn standards_tick(&mut self) {
        let now = self.now;
        let mut due = 0;
        while self.assists.standards_due.first().is_some_and(|&t| t <= now) {
            self.assists.standards_due.remove(0);
            due += 1;
        }
        if let Some(since) = self.endless_since {
            if self.content.battle_standards_in(&self.zone().id) {
                let every = self.content.balance.battle_standard.endless_every.max(1.0);
                let next = *self.assists.next_endless.get_or_insert(since + every);
                if now >= next {
                    self.assists.next_endless = Some(next + every);
                    due += 1;
                }
            }
        }
        for _ in 0..due {
            self.place_pickup(Pickup::BattleStandard(self.party_faction), CACHE_RING);
            self.push(Event::Announce(super::lines::STANDARD_RAISED.into()));
        }
    }

    /// The character the next standard hero `h` takes summons: one of the hero's faction summoned
    /// the fewest times this run (so none comes twice while another has not come), at random.
    pub(crate) fn pick_ally_character(&mut self, h: usize) -> Option<String> {
        let faction = self.content.races[self.heroes[h].race].faction;
        let pool: Vec<String> = self.content.player_characters_of(faction).map(|c| c.id.clone()).collect();
        let times = |id: &String| self.assists.summoned.iter().filter(|s| *s == id).count();
        let fewest = pool.iter().map(times).min()?;
        let left: Vec<String> = pool.iter().filter(|c| times(c) == fewest).cloned().collect();
        let k = (self.rng.next_u64() % left.len() as u64) as usize;
        Some(left[k].clone())
    }

    /// Hero `h` took a battle standard: an ally of the hero's faction comes and starts its assist.
    pub(crate) fn summon_ally(&mut self, h: usize) {
        while !self.assists.forced.is_empty() {
            let id = self.assists.forced.remove(0);
            if self.content.player_characters.iter().any(|c| c.id == id) {
                self.summon_character(h, &id);
                return;
            }
        }
        let Some(character) = self.pick_ally_character(h) else { return };
        self.summon_character(h, &character);
    }

    /// Character `character` comes to assist hero `h` (false when it has no assist).
    pub fn summon_character(&mut self, h: usize, character: &str) -> bool {
        let content = self.content.clone();
        let Some(c) = content.player_characters.iter().find(|c| c.id == character) else { return false };
        let Some(script) = content.assist_of(c) else { return false };
        let rules = &content.balance.battle_standard;
        let now = self.now;
        let hero = &self.heroes[h];
        let (hero_id, hero_pos, hero_facing) = (hero.id, hero.pos, hero.facing);
        let id = self.new_id();
        // beside the hero, on its left; or beside the enemy the arrival picks
        let side = hero_facing + std::f32::consts::FRAC_PI_2;
        let mut at = (hero_pos.0 + side.cos() * ALLY_BESIDE_HERO, hero_pos.1 + side.sin() * ALLY_BESIDE_HERO);
        // its stay: its script, then its rotation, the script's heals spread over both
        let until = now + script.longest(rules.assist_seconds);
        let (style, spells) = match content.behaviour_of(c) {
            Some(b) => (b.style, b.rotation.iter().filter_map(|s| content.assist_spell(*s).copied()).collect()),
            None => (MoveStyle::Melee, Vec::new()),
        };
        let rotation = Rotation::planned(style, spells, script.damage_round(), now + script.written_seconds(), until);
        let share = rotation.share();
        let mut ally = Ally {
            id,
            hero: h,
            character: c.id.clone(),
            segment: Segment::standing(at, now),
            facing: hero_facing,
            until,
            moving: AllyMove::Run,
            busy_until: now,
            frames: vec![Frame { steps: script.steps.clone(), next: 0, times: 1, every: 0.0, started: now, passes: 1, pick_before: Vec::new() }],
            picked: Vec::new(),
            taunts: Vec::new(),
            taunting: false,
            leave: script.leave.clone(),
            rotation,
            share,
        };
        if let Some(t) = &script.arrive.beside {
            let picked = self.pick_targets(&ally, t);
            if let Some(p) = picked.first().and_then(|&e| self.enemy_pos(e)) {
                at = short_of(hero_pos, p, ALLY_BESIDE_ENEMY);
                ally.facing = facing(at, p, hero_facing);
                ally.segment = Segment::standing(at, now);
            }
            ally.picked = picked;
        }
        self.assists.summoned.push(c.id.clone());
        let (how, look_spell, facing, scale) = (script.arrive.how, script.arrive.look_spell, ally.facing, rules.ally_scale);
        self.push(Event::AllySummoned { id, hero: hero_id, character: c.id.clone(), at, facing, how, look_spell, scale });
        self.push(Event::Announce(super::lines::ally_answers(&c.name)));
        if !c.yell.is_empty() {
            self.push(Event::Yell { id, text: c.yell.clone() });
        }
        self.assists.allies.push(ally);
        // its first steps go at once
        let k = self.assists.allies.len() - 1;
        self.ally_tick(k);
        true
    }

    /// The standards' schedule; allies run their steps; then what they left: areas, effects
    /// over time, buffs.
    pub(crate) fn assists_tick(&mut self) {
        self.standards_tick();
        let mut k = 0;
        while k < self.assists.allies.len() {
            let id = self.assists.allies[k].id;
            self.ally_tick(k);
            k = self.assists.allies.iter().position(|a| a.id == id).map_or(k, |p| p + 1);
        }
        self.ally_areas_tick();
        self.ally_dots_tick();
        self.ally_buffs_tick();
        self.ally_bolts_tick();
    }

    /// Assists' pets' bolts land: the assist's damage on their enemy (gone: nothing).
    fn ally_bolts_tick(&mut self) {
        let now = self.now;
        if self.assists.bolts.is_empty() {
            return;
        }
        let (landed, flying): (Vec<AllyBolt>, Vec<AllyBolt>) = std::mem::take(&mut self.assists.bolts).into_iter().partition(|b| b.at <= now);
        self.assists.bolts = flying;
        for b in landed {
            let amount = self.assist_damage(b.enemy, b.seconds);
            self.assist_hit_as(b.hero, b.enemy, amount, HitSource::Pet(b.pet));
        }
    }

    /// Ally `k` runs every step that is due; it leaves when they are done and no taunt holds, or
    /// at its time (or when its hero has fallen).
    fn ally_tick(&mut self, k: usize) {
        let now = self.now;
        // a taunt that ran out, or on an enemy that died, holds no more
        let alive: Vec<u32> = self.enemies.iter().map(|e| e.id).collect();
        let a = &mut self.assists.allies[k];
        a.taunts.retain(|(e, until)| now < *until && alive.contains(e));
        if a.taunting && a.taunts.is_empty() {
            a.taunting = false;
            let id = a.id;
            self.push(Event::AllyTaunting { id, taunting: false });
        }
        if !self.heroes[self.assists.allies[k].hero].alive {
            return self.ally_leaves(k);
        }
        // our rule (owner): a melee ally keeps moving round its enemy while its script strikes,
        // as after it (never during a move of the script)
        let a = &mut self.assists.allies[k];
        if !a.frames.is_empty() && matches!(a.rotation.style, MoveStyle::Melee | MoveStyle::ErraticMelee) && !a.segment.moving(now) {
            // its moves are planned from the script's end; in the script they start at once
            if !a.rotation.started && a.rotation.next_move == a.rotation.start {
                a.rotation.next_move = now;
            }
            self.ally_moves_in_rotation(k);
        }
        for _ in 0..STEPS_PER_TICK {
            let a = &mut self.assists.allies[k];
            // its script still running at the end of its stay: it goes whatever is left
            if now >= a.until && !a.frames.is_empty() {
                return self.ally_leaves(k);
            }
            if now < a.busy_until {
                return;
            }
            let depth = a.frames.len();
            let Some(f) = a.frames.last_mut() else {
                // its script done: its rotation until its stay ends
                if self.ally_rotation_tick(k) {
                    return self.ally_leaves(k);
                }
                return;
            };
            if f.next < f.steps.len() {
                let step = f.steps[f.next].clone();
                f.next += 1;
                // a hit of the script stands for one global cooldown, one of a repeat for its beat
                let seconds = if depth > 1 && f.every > 0.0 { f.every } else { GLOBAL_COOLDOWN };
                let takes = self.ally_step(k, &step, seconds);
                let a = &mut self.assists.allies[k];
                a.busy_until = a.busy_until.max(now + takes);
                continue;
            }
            // a pass is over: the next one at its time, or the repeat is done after its last
            let next_start = f.started + f.every * f.passes as f32;
            if f.passes < f.times {
                if now < next_start {
                    a.busy_until = next_start;
                    return;
                }
                f.passes += 1;
                f.next = 0;
                let before = f.pick_before.clone();
                a.picked = before;
                continue;
            }
            a.frames.pop();
            a.busy_until = a.busy_until.max(next_start);
        }
    }

    /// Ally `k` goes: its taunts end, its death yell, its way of leaving. What it left stays.
    fn ally_leaves(&mut self, k: usize) {
        let a = self.assists.allies.remove(k);
        let now = self.now;
        if a.taunting {
            self.push(Event::AllyTaunting { id: a.id, taunting: false });
        }
        // a channel cut short: its ticks stop and its storm goes
        if let Some(ch) = &a.rotation.channeling {
            if now < ch.ends + super::casting::AREA_LOOK_AFTER {
                self.push(Event::AreaGone { id: ch.area });
            }
        }
        let content = self.content.clone();
        if let Some(c) = content.player_characters.iter().find(|c| c.id == a.character) {
            if !c.death_yell.is_empty() {
                self.push(Event::Yell { id: a.id, text: c.death_yell.clone() });
            }
        }
        let look_spell = a.leave.look_spell.or(match a.leave.how {
            Leave::Vanish => Some(VANISH_LOOK),
            Leave::Hearth => Some(HEARTH_LOOK),
            _ => None,
        });
        let to = (a.leave.how == Leave::RunAway).then(|| {
            // off the screen, straight away from the hero
            let (pos, hero) = (a.segment.position(now), self.heroes[a.hero].pos);
            let r = self.zone().enemy_spawn_ring.0;
            let dir = facing(hero, pos, a.facing);
            (hero.0 + dir.cos() * r, hero.1 + dir.sin() * r)
        });
        self.push(Event::AllyLeft { id: a.id, how: a.leave.how, look_spell, to });
    }

    /// The enemies `t` picks for ally `a`: living ones on screen (within the zone's enemy spawn
    /// ring's inner radius of the hero).
    pub(crate) fn pick_targets(&self, a: &Ally, t: &Targets) -> Vec<u32> {
        let now = self.now;
        let hero = self.heroes[a.hero].pos;
        let on_screen = self.enemies_within(hero, self.zone().enemy_spawn_ring.0);
        let ally = a.segment.position(now);
        let near = |at: Vec2, radius: f32, most: u32| {
            let mut v: Vec<(f32, u32)> = on_screen.iter().filter_map(|&id| self.enemy_pos(id).map(|p| (dist(p, at), id))).filter(|x| x.0 <= radius).collect();
            v.sort_by(|x, y| x.0.total_cmp(&y.0));
            let mut ids: Vec<u32> = v.into_iter().map(|x| x.1).collect();
            if most > 0 {
                ids.truncate(most as usize);
            }
            ids
        };
        // the first (nearest) of the best by `better`
        let best = |keep: &dyn Fn(&EnemyUnit) -> bool, better: &dyn Fn(&EnemyUnit, &EnemyUnit) -> std::cmp::Ordering| {
            on_screen
                .iter()
                .filter_map(|&id| self.enemy(id))
                .filter(|e| keep(e))
                .enumerate()
                .min_by(|(i, x), (j, y)| better(x, y).then(i.cmp(j)))
                .map(|(_, e)| vec![e.id])
                .unwrap_or_default()
        };
        // a boss, else a rare, else an elite, else the most max health
        let rank = |e: &EnemyUnit| if e.is_boss() { 0 } else if e.is_rare() { 1 } else if e.ranks_as_elite() { 2 } else { 3 };
        let strongest = |x: &EnemyUnit, y: &EnemyUnit| rank(x).cmp(&rank(y)).then(y.max_health.total_cmp(&x.max_health));
        match *t {
            Targets::Nearest(n) => on_screen.into_iter().take(n as usize).collect(),
            Targets::NearHero { radius, most } => near(hero, radius, most),
            Targets::NearAlly { radius, most } => near(ally, radius, most),
            Targets::Strongest => best(&|_| true, &strongest),
            Targets::StrongestNotBoss => best(&|e| !e.is_boss(), &strongest),
            Targets::Healthiest => best(&|_| true, &|x, y| y.health.total_cmp(&x.health)),
            Targets::Farthest => on_screen.last().map(|&id| vec![id]).unwrap_or_default(),
            Targets::OnScreen => on_screen,
            Targets::Same => a.picked.iter().copied().filter(|id| self.enemy(*id).is_some()).collect(),
        }
    }

    /// What an assist's damage event standing for `seconds` deals to enemy `id`: the assist rate
    /// for its kind (`balance.battle_standard.assist_damage_per_second`: `normal`, or
    /// `elite_rare_boss` on an elite, a rare or a boss) × its max health × `seconds`.
    pub(crate) fn assist_damage(&self, id: u32, seconds: f32) -> f32 {
        let Some(e) = self.enemy(id) else { return 0.0 };
        let rates = self.content.balance.battle_standard.assist_damage_per_second;
        let rate = if e.is_boss() || e.is_rare() || e.elite { rates.elite_rare_boss } else { rates.normal };
        rate * e.max_health * seconds
    }

    /// Runs one step of ally `k`; returns the seconds it takes. A damaging hit deals the assist's
    /// damage for `seconds` (what the hit stands for); a ground area's pulses and an effect over
    /// time's ticks for their interval. Its heals are dealt at the ally's `share` (the rest comes
    /// with its rotation's casts).
    pub(crate) fn ally_step(&mut self, k: usize, step: &AssistStep, seconds: f32) -> f32 {
        let now = self.now;
        let (id, h, character, share) = {
            let a = &self.assists.allies[k];
            (a.id, a.hero, a.character.clone(), a.share)
        };
        let me = Unit::Ally(id);
        match step {
            AssistStep::Move { to, how, look_spell } => {
                let picked = self.pick_targets(&self.assists.allies[k], to);
                self.assists.allies[k].picked = picked.clone();
                let spots: Vec<Vec2> = picked.iter().filter_map(|&e| self.enemy_pos(e)).collect();
                if spots.is_empty() {
                    return 0.0;
                }
                let from = self.assists.allies[k].segment.position(now);
                let goal = if spots.len() == 1 {
                    short_of(from, spots[0], ALLY_BESIDE_ENEMY)
                } else {
                    let n = spots.len() as f32;
                    (spots.iter().map(|p| p.0).sum::<f32>() / n, spots.iter().map(|p| p.1).sum::<f32>() / n)
                };
                let (way, speed, look) = match how {
                    Movement::Run => (AllyMove::Run, ALLY_RUN_SPEED, *look_spell),
                    Movement::Charge => (AllyMove::Charge, ALLY_CHARGE_SPEED, Some(look_spell.unwrap_or(CHARGE_LOOK))),
                    Movement::Blink => (AllyMove::Blink, 0.0, *look_spell),
                };
                let takes = if speed > 0.0 { dist(from, goal) / speed } else { 0.0 };
                self.move_ally(k, goal, spots[0], way, takes, look);
                takes
            }
            AssistStep::Cast { look_spell } => {
                self.push(Event::SpellGo { caster: me, spell: *look_spell, hits: Vec::new(), aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
                0.0
            }
            AssistStep::Hit { targets, damage, look_spell, melee, effects, wears, over_time } => {
                let picked = self.pick_targets(&self.assists.allies[k], targets);
                self.assists.allies[k].picked = picked.clone();
                let Some(&first) = picked.first() else { return 0.0 };
                if *melee {
                    // one strafing or jumping round its enemy swings on the move
                    let dodging = self.assists.allies[k].segment.moving(now) && self.assists.allies[k].moving != AllyMove::Run;
                    if let Some(p) = self.enemy_pos(first).filter(|_| !dodging) {
                        let from = self.assists.allies[k].segment.position(now);
                        self.move_ally(k, short_of(from, p, ALLY_BESIDE_ENEMY), p, AllyMove::Step, 0.0, None);
                    }
                    self.push(Event::AllySwing { id, target: first });
                }
                if let Some(spell) = look_spell {
                    let hits = picked.iter().map(|&e| Unit::Enemy(e)).collect();
                    self.push(Event::SpellGo { caster: me, spell: *spell, hits, aim: Aim::Unit(Unit::Enemy(first)), ammo: false, reach: 0.0, triggered: false });
                }
                let damage = damage.then_some(seconds);
                for e in picked {
                    self.ally_strikes(h, id, &character, e, damage, effects, wears, over_time.as_ref());
                }
                0.0
            }
            AssistStep::Area { at, radius, seconds: lasts, every, damage, effects, look_spell, most } => {
                let spot = match at {
                    AreaAt::Hero => self.heroes[h].pos,
                    AreaAt::Ally => self.assists.allies[k].segment.position(now),
                    AreaAt::BiggestPack => self.biggest_pack(h, *radius),
                };
                // the same ally's earlier area of this look ends: one at a time
                let old: Vec<u32> = self.assists.areas.iter().filter(|x| x.ally == id && x.spell == *look_spell).map(|x| x.id).collect();
                self.assists.areas.retain(|x| !old.contains(&x.id));
                for gone in old {
                    self.push(Event::AreaGone { id: gone });
                }
                let area = self.new_id();
                self.push(Event::AreaShown { id: area, caster: me, spell: *look_spell, at: spot, radius: *radius, until: Some(now + lasts + AREA_LOOK_AFTER) });
                let every = every.max(0.05);
                self.assists.areas.push(AllyArea {
                    id: area,
                    ally: id,
                    hero: h,
                    character: character.clone(),
                    at: spot,
                    radius: *radius,
                    every,
                    next: now + every,
                    until: now + lasts,
                    damage: *damage,
                    effects: effects.clone(),
                    spell: *look_spell,
                    most: *most,
                });
                // its first pulse at once, on what is inside now
                for e in self.enemies_nearest_within(spot, *radius, *most) {
                    self.ally_strikes(h, id, &character, e, damage.then_some(every), effects, &[], None);
                }
                0.0
            }
            AssistStep::Buff { stats, seconds, heal, every, immune, look_spell, totem } => {
                let until = now + seconds;
                let hero = Unit::Hero(self.heroes[h].id);
                if let Some(spell) = totem {
                    // planted at the hero's side; it never pulses (the buff is the ally's)
                    let f = self.heroes[h].facing + std::f32::consts::FRAC_PI_2;
                    let p = self.heroes[h].pos;
                    let t = self.new_id();
                    self.totems.push(Totem {
                        id: t,
                        hero: h,
                        spell: format!("assist_totem:{spell}"),
                        at: (p.0 + f.cos() * TOTEM_OFFSET, p.1 + f.sin() * TOTEM_OFFSET),
                        until,
                        next_pulse: until + 1.0,
                        interval: super::casting::TOTEM_PULSE,
                        inst: Default::default(),
                        from_proc: false,
                    });
                    self.push(Event::AllyDropped { id, unit: Unit::Totem(t) });
                }
                if let Some(spell) = look_spell {
                    self.push(Event::SpellGo { caster: me, spell: *spell, hits: vec![hero], aim: Aim::Unit(hero), ammo: false, reach: 0.0, triggered: false });
                    self.push(Event::AuraShown { target: hero, spell: *spell, until, from: me });
                }
                if *immune {
                    self.heroes[h].buffs.push(Buff { kind: BuffKind::Immunity, value: 0.0, visual: 0, until });
                }
                if !stats.is_empty() || *heal > 0.0 {
                    let every = if *every > 0.0 { *every } else { 1.0 };
                    self.assists.buffs.push(AllyBuff { hero: h, stats: stats.clone(), heal: *heal, every, next_heal: now + every, until });
                }
                if !stats.is_empty() {
                    self.refresh_stats(h);
                }
                0.0
            }
            AssistStep::Heal { share: heal_share, look_spell } => {
                let hero = Unit::Hero(self.heroes[h].id);
                if let Some(spell) = look_spell {
                    self.push(Event::SpellGo { caster: me, spell: *spell, hits: vec![hero], aim: Aim::Unit(hero), ammo: false, reach: 0.0, triggered: false });
                }
                let max = self.heroes[h].stats.max_health;
                self.heal_hero(h, heal_share * share * max, 0);
                0.0
            }
            AssistStep::Taunt { targets, seconds, look_spell } => {
                let picked = self.pick_targets(&self.assists.allies[k], targets);
                self.assists.allies[k].picked = picked.clone();
                for e in picked {
                    let Some(enemy) = self.enemies.iter_mut().find(|x| x.id == e) else { continue };
                    // a boss is held a quarter of the time
                    let share = if enemy.is_boss() { HOLD_BOSS_SHARE } else { 1.0 };
                    enemy.next_decision = now;
                    let a = &mut self.assists.allies[k];
                    a.taunts.retain(|(x, _)| *x != e);
                    a.taunts.push((e, now + seconds * share));
                    if let Some(spell) = look_spell {
                        let target = Unit::Enemy(e);
                        self.push(Event::SpellGo { caster: me, spell: *spell, hits: vec![target], aim: Aim::Unit(target), ammo: false, reach: 0.0, triggered: false });
                    }
                }
                let a = &mut self.assists.allies[k];
                if !a.taunting && !a.taunts.is_empty() {
                    a.taunting = true;
                    self.push(Event::AllyTaunting { id, taunting: true });
                }
                0.0
            }
            AssistStep::Summon { pet, count, seconds: lasts } => {
                let spot = self.assists.allies[k].segment.position(now);
                // its attacks are the assist's damage, each for the pet's attack interval
                for _ in 0..*count {
                    if let Some(p) = self.summon_pet_at(h, pet, Some(now + lasts), Some(spot)) {
                        if let Some(unit) = self.pets.iter_mut().find(|x| x.id == p) {
                            unit.assist = Some(character.clone());
                        }
                        self.push(Event::AllyDropped { id, unit: Unit::Pet(p) });
                    }
                }
                0.0
            }
            AssistStep::Repeat { every, times, steps } => {
                if *times > 0 && !steps.is_empty() {
                    let a = &mut self.assists.allies[k];
                    let pick_before = a.picked.clone();
                    a.frames.push(Frame { steps: steps.clone(), next: 0, times: *times, every: *every, started: now, passes: 1, pick_before });
                }
                0.0
            }
            AssistStep::Wait { seconds: waits } => *waits,
        }
    }

    /// Ally `k` goes to `to`, then faces `look_at`, taking `takes` seconds.
    pub(crate) fn move_ally(&mut self, k: usize, to: Vec2, look_at: Vec2, how: AllyMove, takes: f32, look_spell: Option<u32>) {
        let now = self.now;
        let a = &mut self.assists.allies[k];
        let from = a.segment.position(now);
        a.facing = facing(to, look_at, facing(from, to, a.facing));
        a.segment = Segment { from, to, start: now, duration: takes };
        a.moving = how;
        let (id, facing) = (a.id, a.facing);
        self.push(Event::AllyMoved { id, to, facing, how, until: now + takes, look_spell });
    }

    /// The spot of the thickest crowd on screen for an area `r` yd round, as the hero's ground
    /// areas pick it (among the nearest enemies' spots, the first with the most within `r`); the
    /// hero's spot when no enemy is there.
    fn biggest_pack(&self, h: usize, r: f32) -> Vec2 {
        let hero = self.heroes[h].pos;
        let spots: Vec<Vec2> = self.enemies_within(hero, self.zone().enemy_spawn_ring.0).into_iter().take(AREA_SPOTS).filter_map(|id| self.enemy_pos(id)).collect();
        let count = |p: Vec2| self.enemies_within(p, r).len();
        let Some(most) = spots.iter().map(|&p| count(p)).max() else { return hero };
        spots.into_iter().find(|&p| count(p) == most).unwrap_or(hero)
    }

    /// The ally of hero `h` strikes enemy `e`: with `damage`, the assist's damage for that many
    /// seconds (once an event when one runs, `Assists::event_struck`), its effects through the
    /// hero's own rules, a mark worn while they hold, and damage over time (renewed, never stacked).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ally_strikes(&mut self, h: usize, ally: u32, character: &str, e: u32, damage: Option<f32>, effects: &[SpellEffect], wears: &[u32], over_time: Option<&OverTime>) {
        let now = self.now;
        let mut dealt = 0.0;
        let damage = match self.assists.event_struck.as_mut() {
            Some(struck) if damage.is_some() => {
                if struck.contains(&e) {
                    None
                } else {
                    struck.push(e);
                    damage
                }
            }
            _ => damage,
        };
        if let Some(seconds) = damage {
            let amount = self.assist_damage(e, seconds);
            match self.assist_hit(h, e, amount, character) {
                Some(x) => dealt = x,
                None => return,
            }
        }
        if self.enemy(e).is_none() {
            return;
        }
        for effect in effects {
            self.assist_effect(h, e, effect, dealt);
        }
        let from = Unit::Ally(ally);
        if !wears.is_empty() {
            let held = self.effects_hold(e, effects);
            if held > now {
                let spell = wears[(self.rng.next_u64() % wears.len() as u64) as usize];
                self.push(Event::AuraShown { target: Unit::Enemy(e), spell, until: held, from });
            }
        }
        if let Some(o) = over_time {
            let every = o.every.max(0.05);
            self.assists.dots.retain(|d| !(d.enemy == e && d.character == character));
            self.assists.dots.push(AllyDot { enemy: e, hero: h, character: character.to_string(), every, next: now + every, until: now + o.seconds });
            if let Some(spell) = o.look_spell {
                self.push(Event::AuraShown { target: Unit::Enemy(e), spell, until: now + o.seconds, from });
            }
        }
    }

    /// An ally's hit of `amount` on enemy `e` for hero `h`: its own damage only (no hero bonus,
    /// no crit, no vulnerability on the enemy, nothing for the hero's lifesteal, resource or
    /// milestones), only a boss's shield taking its share. A kill is the hero's for its rewards.
    /// Returns the damage dealt, `None` when the enemy is gone.
    pub(crate) fn assist_hit(&mut self, h: usize, e: u32, amount: f32, character: &str) -> Option<f32> {
        self.assist_hit_as(h, e, amount, HitSource::Assist(character.to_string()))
    }

    /// As `assist_hit`, logged as `source` (an assist's pet's hit stays its pet's, which the
    /// meter counts as its assist's).
    pub(crate) fn assist_hit_as(&mut self, h: usize, e: u32, amount: f32, source: HitSource) -> Option<f32> {
        let now = self.now;
        let k = self.enemies.iter().position(|x| x.id == e)?;
        let shield = self.enemies[k].mech.damage_share(now);
        if shield <= 0.0 || amount <= 0.0 {
            return Some(0.0);
        }
        let dealt = amount * shield;
        self.enemies[k].health -= dealt;
        let by = Some(self.heroes[h].id);
        self.push(Event::EnemyHit { id: e, amount: dealt, crit: false, by, source: source.clone() });
        if self.enemies[k].health <= 0.0 {
            self.kill_setting_off(k, Some(h), source, false);
        }
        Some(dealt)
    }

    /// An ally's effect on enemy `e`: the hero's rules (`apply_on_hit`), except that a stun or root
    /// holds a normal enemy (not a boss, not a rare) its full written time, outside the hero's
    /// chain of holds and its immunity.
    pub(crate) fn assist_effect(&mut self, h: usize, e: u32, effect: &SpellEffect, dealt: f32) {
        let now = self.now;
        if let SpellEffect::Stun { seconds } | SpellEffect::Root { seconds } = *effect {
            if let Some(x) = self.enemies.iter_mut().find(|x| x.id == e && !x.is_boss() && !x.is_rare()) {
                if now + seconds > x.hold_until {
                    x.hold_until = now + seconds;
                    let p = x.position(now);
                    x.segment = Segment::standing(p, now);
                }
                return;
            }
        }
        self.apply_on_hit(h, e, effect, dealt);
    }

    /// Until when the effects of `effects` hold on enemy `e` (now when none does: a fear on a
    /// boss, a hold on an enemy still immune to holds).
    fn effects_hold(&self, e: u32, effects: &[SpellEffect]) -> f32 {
        let now = self.now;
        let Some(x) = self.enemy(e) else { return now };
        let until = |o: Option<(f32, f32)>| o.map_or(now, |(_, u)| u);
        effects
            .iter()
            .map(|f| match f {
                SpellEffect::Stun { .. } | SpellEffect::Root { .. } => x.hold_until,
                SpellEffect::Fear { .. } => x.fear_until,
                SpellEffect::Slow { .. } => until(x.slow),
                SpellEffect::Weaken { .. } => until(x.weaken),
                SpellEffect::Vulnerable { .. } => until(x.vulnerable),
                _ => now,
            })
            .fold(now, f32::max)
    }

    /// The enemies within `radius` yd of `at`, the nearest first, at most `most` of them (0: all).
    pub(crate) fn enemies_nearest_within(&self, at: Vec2, radius: f32, most: u32) -> Vec<u32> {
        let mut v: Vec<(f32, u32)> = self.enemies_within(at, radius).into_iter().filter_map(|e| self.enemy_pos(e).map(|p| (dist(p, at), e))).collect();
        v.sort_by(|x, y| x.0.total_cmp(&y.0));
        if most > 0 {
            v.truncate(most as usize);
        }
        v.into_iter().map(|x| x.1).collect()
    }

    /// Allies' ground areas strike every enemy inside (or the nearest `most`) at each of their times.
    fn ally_areas_tick(&mut self) {
        let now = self.now;
        let mut strikes = Vec::new();
        for a in &mut self.assists.areas {
            while a.next <= now && a.next < a.until - 1e-4 {
                a.next += a.every;
                strikes.push(a.clone());
            }
        }
        self.assists.areas.retain(|a| a.next < a.until - 1e-4);
        for a in strikes {
            for e in self.enemies_nearest_within(a.at, a.radius, a.most) {
                self.ally_strikes(a.hero, a.ally, &a.character, e, a.damage.then_some(a.every), &a.effects, &[], None);
            }
        }
    }

    /// Allies' damage over time ticks (the last at its end).
    fn ally_dots_tick(&mut self) {
        let now = self.now;
        let mut ticks = Vec::new();
        for d in &mut self.assists.dots {
            while d.next <= now && d.next <= d.until + 1e-4 {
                d.next += d.every;
                ticks.push(d.clone());
            }
        }
        let alive: Vec<u32> = self.enemies.iter().map(|e| e.id).collect();
        self.assists.dots.retain(|d| d.next <= d.until + 1e-4 && alive.contains(&d.enemy));
        for d in ticks {
            let amount = self.assist_damage(d.enemy, d.every);
            if amount > 0.0 {
                self.assist_hit(d.hero, d.enemy, amount, &d.character);
            }
        }
    }

    /// Allies' buffs heal at their times and end.
    fn ally_buffs_tick(&mut self) {
        let now = self.now;
        let mut heals = Vec::new();
        for b in &mut self.assists.buffs {
            while b.heal > 0.0 && b.next_heal <= now && b.next_heal <= b.until + 1e-4 {
                b.next_heal += b.every;
                heals.push((b.hero, b.heal));
            }
        }
        for (h, share) in heals {
            let max = self.heroes[h].stats.max_health;
            self.heal_hero(h, share * max, 0);
        }
        let ended: Vec<usize> = self.assists.buffs.iter().filter(|b| b.until <= now && !b.stats.is_empty()).map(|b| b.hero).collect();
        self.assists.buffs.retain(|b| b.until > now);
        for h in ended {
            self.refresh_stats(h);
        }
    }

    /// The stats allies' buffs add to hero `h` now.
    pub(crate) fn assist_modifiers(&self, h: usize) -> Vec<Modifier> {
        let now = self.now;
        self.assists.buffs.iter().filter(|b| b.hero == h && now < b.until).flat_map(|b| b.stats.iter().map(|&(s, v)| Modifier::new(s, v))).collect()
    }

    /// The ally taunting enemy `enemy`, if one does: its id and where it stands.
    pub(crate) fn taunted_by(&self, enemy: u32) -> Option<(u32, Vec2)> {
        let now = self.now;
        self.assists.allies.iter().find(|a| a.taunts.iter().any(|&(e, until)| e == enemy && now < until)).map(|a| (a.id, a.segment.position(now)))
    }

    /// Enemy `k` at `pos`, taunted by ally `ally` standing at `at`, goes to it and strikes it at
    /// its pace; the ally takes nothing.
    pub(crate) fn taunted_enemy_tick(&mut self, k: usize, pos: Vec2, ally: u32, at: Vec2, speed: f32, dt: f32) {
        let now = self.now;
        let e = &mut self.enemies[k];
        let (chase_beyond, _) = reach(e.collision_radius, None);
        if dist(pos, at) > chase_beyond {
            if now >= e.next_decision || !e.segment.moving(now) {
                let (seg, next) = chase(pos, at, &mut e.aim_angle, 0.0, speed, now);
                e.segment = seg;
                e.next_decision = next;
            }
            return;
        }
        if e.segment.moving(now) {
            e.segment = Segment::standing(pos, now);
        }
        e.swing_timer -= dt;
        if e.swing_timer > 0.0 {
            return;
        }
        e.swing_timer = e.attack_interval;
        let by = e.id;
        self.push(Event::AllyStruck { id: ally, by });
    }
}
