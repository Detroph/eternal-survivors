//! An ally's rotation (docs/game/assists.md, "Its rotation"): once its assist script is done, the
//! ally loops its behaviour's 1.12 spells until it leaves, moving as its style says.
//!
//! - **The plan:** at the summon the rotation is laid out from the script's written end to the
//!   ally's stay (`plan_rotation`): the spells in order, one skipped while its cooldown runs, a
//!   1.5 sec global cooldown between casts and a cast's own time (`Spell.dbc`, in
//!   `assist_spells.ron`) before it lands, a channel's whole time before the next. Every planned
//!   cast is made: a script whose moves ran late, or a cast with a cast time (or a channel)
//!   waiting while the ally runs (at most `LATE_MOST` in all), shifts the rest of the plan, and
//!   the ally stays until its last planned cast has landed.
//! - **Its damage:** each cast as it lands strikes again what the script's damaging steps strike
//!   (`AssistScript::damage_round`: the same picks and caps), each enemy once, for the assist's
//!   damage over the cast's cycle (its cast time, at least the global cooldown); an area or an
//!   effect over time of the round deals at its own interval. The script's heals are spread over
//!   the script and the planned casts: the script heals `share` = 1 / (1 + casts) of them, and each
//!   cast one more round of that share. Crowd control, buffs, taunts and summons stay in the
//!   script alone.
//! - **A channel** (Blizzard; not the hero's Blizzard): the ally stands and channels
//!   for the spell's channel time (its 1.12 channel look, `UNIT_CHANNEL_SPELL`), its storm on the
//!   ground under the enemy nearest it at the spell's radius. Each of the channel's ticks strikes
//!   what the round strikes (a ground area of the round: the enemies in the storm), each enemy
//!   once, for the assist's damage over the tick's interval, heals a `1 / ticks` part of the
//!   round's heals, and slows the enemies in the storm by `CHANNEL_SLOW` until the next tick. The
//!   ticks stop when the ally leaves.
//! - **Auto attacks only:** a `Melee` or `ErraticMelee` ally with an empty rotation swings at the
//!   enemy nearest it every `AUTO_ATTACK_SECONDS` while in melee, each swing the assist's damage
//!   over `AUTO_ATTACK_SECONDS` as any of its hits.
//! - **Nothing:** a `LeavesAfterScript` ally leaves once its script is done and no taunt of its
//!   holds.
//! - **Its movement:** `Melee` runs to the enemy nearest it and stays there; `ErraticMelee` also
//!   strafes left and right in melee and jumps every third strafe; `Caster` stands beside the hero
//!   and follows when the hero is too far; `NovaKiteCaster` also jumps away after an instant cast
//!   with an enemy close; `Healer` keeps close to the hero. A cast with a cast time is cast
//!   standing, and a channel channelled standing.

use esurv_content::{AssistSpell, AssistStep, CastOn, MoveStyle, SpellEffect, Targets};

use super::assists::{AllyMove, ALLY_BESIDE_ENEMY, ALLY_BESIDE_HERO, ALLY_RUN_SPEED};
use super::casting::{HitSource, AREA_LOOK_AFTER};
use super::enemy_ai::{Segment, Vec2};
use super::run::{Aim, Event, Run, Unit};

/// The global cooldown between two of the ally's casts (a player's, seconds).
pub const GLOBAL_COOLDOWN: f32 = 1.5;
/// A melee ally closes in again once its enemy is this much farther than where it stops.
const MELEE_SLACK: f32 = 1.5;
/// A strafe of an erratic melee ally lasts this long, at its run speed; every `JUMP_EVERY`th is a
/// jump instead.
pub const STRAFE_SECONDS: f32 = 0.4;
pub const JUMP_EVERY: u32 = 3;
/// A jump's time in the air: a 1.12 player's take-off speed (7.955547 yd/s) up and down again
/// under the client's gravity (19.291105 yd/s²).
pub const JUMP_SECONDS: f32 = 2.0 * 7.955_547 / 19.291_105;
/// A caster follows the hero once it is farther than this from the hero; a healer keeps closer.
pub const CASTER_FOLLOW: f32 = 8.0;
pub const HEALER_FOLLOW: f32 = 4.0;
/// A nova-kiting caster jumps away after an instant cast when an enemy is this close.
pub const KITE_CLOSE: f32 = 8.0;
/// How often a standing ally looks again at where to go (seconds); a run is looked at again as
/// often, so it follows its enemy.
const MOVE_LOOK_EVERY: f32 = 0.25;
/// A cast with a cast time (or a channel) waits while the ally runs (to its enemy, back to the
/// hero), the rest of the plan shifting with it, until the plan runs this late in all (its
/// script's lateness too); then it casts where it stands. An ally this late twice over leaves
/// whatever is left.
const LATE_MOST: f32 = 5.0;
/// A channel's ticks slow the enemies in its storm this much (40 %), each
/// tick's slow lasting until the next.
pub const CHANNEL_SLOW: f32 = 0.4;
/// An ally that only auto attacks swings this often (seconds): a 1.12 unit's base attack time
/// (`UNIT_FIELD_BASEATTACKTIME`'s 2000 ms). Each swing deals the assist's damage for this long.
pub const AUTO_ATTACK_SECONDS: f32 = 2.0;

/// One planned cast: when it begins (seconds after the rotation starts) and which spell of the
/// rotation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlannedCast {
    pub at: f32,
    pub spell: usize,
}

/// The casts a rotation of `spells` makes in `length` seconds: in order, a spell skipped while
/// its cooldown runs (the next ready one cast instead; none ready: the first to be), each cast
/// beginning at least a global cooldown after the one before and once its cast time (and its
/// channel) is done, its cooldown counted from its end. Only casts that end within `length` are
/// planned.
pub fn plan_rotation(spells: &[AssistSpell], length: f32) -> Vec<PlannedCast> {
    let n = spells.len();
    let mut plan = Vec::new();
    if n == 0 {
        return plan;
    }
    let mut ready = vec![0.0f32; n];
    let (mut t, mut next) = (0.0f32, 0usize);
    while t < length - 1e-4 {
        let Some(k) = (0..n).map(|i| (next + i) % n).find(|&i| ready[i] <= t + 1e-4) else {
            t = ready.iter().copied().fold(f32::INFINITY, f32::min);
            continue;
        };
        let end = t + spells[k].cast + spells[k].channel;
        if end > length + 1e-4 {
            break;
        }
        plan.push(PlannedCast { at: t, spell: k });
        ready[k] = end + spells[k].cooldown;
        next = (k + 1) % n;
        t = (t + GLOBAL_COOLDOWN).max(end);
    }
    plan
}

/// A cast being cast: which spell of the rotation, when it lands, and where it is aimed.
#[derive(Debug, Clone, PartialEq)]
pub struct Casting {
    pub spell: usize,
    pub lands: f32,
    pub aim: Aim,
}

/// A channel being channelled: which spell of the rotation, its storm's look (an `AreaShown` id),
/// where and how wide, its ticks' interval, how many ticks it has and has dealt, when the next
/// comes and when it ends.
#[derive(Debug, Clone, PartialEq)]
pub struct Channeling {
    pub spell: usize,
    pub area: u32,
    pub at: Vec2,
    pub radius: f32,
    pub every: f32,
    pub ticks: u32,
    pub ticked: u32,
    pub next_tick: f32,
    pub ends: f32,
}

/// An ally's rotation and its state.
#[derive(Debug, Clone, PartialEq)]
pub struct Rotation {
    pub style: MoveStyle,
    pub spells: Vec<AssistSpell>,
    pub plan: Vec<PlannedCast>,
    /// The next planned cast to begin.
    pub next: usize,
    /// When the rotation begins (run seconds): the script's written end, then, once the script is
    /// done, when it really began.
    pub start: f32,
    /// When it was planned to begin (the script's written end).
    pub planned: f32,
    pub started: bool,
    /// What each cast strikes and heals as it lands (the script's round; its heals at the ally's
    /// share).
    pub round: Vec<AssistStep>,
    pub casting: Option<Casting>,
    pub channeling: Option<Channeling>,
    /// Its next look at where to move.
    pub next_move: f32,
    /// The strafes it made in melee (left, right, left...; every `JUMP_EVERY`th a jump).
    pub strafes: u32,
    /// Its next auto attack (an ally that only auto attacks).
    pub next_swing: f32,
}

impl Rotation {
    /// The rotation of `spells` moving `style`, planned from `start` (run seconds) to `end`, each
    /// cast dealing `round`.
    pub fn planned(style: MoveStyle, spells: Vec<AssistSpell>, round: Vec<AssistStep>, start: f32, end: f32) -> Self {
        let plan = plan_rotation(&spells, end - start);
        Self { style, spells, plan, next: 0, start, planned: start, started: false, round, casting: None, channeling: None, next_move: start, strafes: 0, next_swing: start }
    }

    /// Every planned cast has landed (and its channel ended).
    pub fn done(&self) -> bool {
        self.next >= self.plan.len() && self.casting.is_none() && self.channeling.is_none()
    }

    /// It casts nothing and only auto attacks: a melee style with an empty rotation.
    pub fn auto_attacks_only(&self) -> bool {
        self.spells.is_empty() && matches!(self.style, MoveStyle::Melee | MoveStyle::ErraticMelee)
    }

    /// The share of the script's heals the script and each cast heal: one part for the script and
    /// one for each planned cast (all of it when the round is empty or nothing is planned).
    pub fn share(&self) -> f32 {
        if self.round.is_empty() {
            1.0
        } else {
            1.0 / (1 + self.plan.len()) as f32
        }
    }
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn toward(from: Vec2, to: Vec2) -> f32 {
    (to.1 - from.1).atan2(to.0 - from.0)
}

fn step(from: Vec2, angle: f32, d: f32) -> Vec2 {
    (from.0 + angle.cos() * d, from.1 + angle.sin() * d)
}

impl Run {
    /// Ally `k` (its script done) runs its rotation: a cast landing, a channel's ticks, its
    /// movement and auto attacks, then the next planned cast beginning (one with a cast time or a
    /// channel waits while it runs, up to `LATE_MOST`). Returns whether it is done and may leave
    /// (at its stay, or once its last cast has landed; one that leaves after its script, once no
    /// taunt of its holds).
    pub(crate) fn ally_rotation_tick(&mut self, k: usize) -> bool {
        let now = self.now;
        let a = &mut self.assists.allies[k];
        if a.rotation.style == MoveStyle::LeavesAfterScript {
            return !a.taunting;
        }
        if !a.rotation.started {
            // the plan shifts by however late the script ran
            a.rotation.started = true;
            a.rotation.start = a.rotation.start.max(now);
        }
        if now >= a.until + 2.0 * LATE_MOST {
            return true;
        }
        if a.rotation.casting.as_ref().is_some_and(|c| now >= c.lands) {
            let c = a.rotation.casting.take().unwrap();
            if a.rotation.spells[c.spell].channelled() {
                self.ally_begins_channel(k, c.spell);
            } else {
                self.ally_cast_lands(k, c.spell);
            }
        }
        self.ally_channel_ticks(k);
        self.ally_moves_in_rotation(k);
        self.ally_auto_attacks(k);
        let a = &mut self.assists.allies[k];
        let idle = a.rotation.casting.is_none() && a.rotation.channeling.is_none();
        let due = idle && a.rotation.plan.get(a.rotation.next).is_some_and(|p| now >= a.rotation.start + p.at - 1e-4);
        if due {
            let p = a.rotation.plan[a.rotation.next];
            let row = a.rotation.spells[p.spell];
            let late = now - a.rotation.planned - p.at;
            let standing = row.cast > 0.0 || row.channelled();
            if standing && a.moving == AllyMove::Run && a.segment.moving(now) && late < LATE_MOST {
                // it casts once it is there, the rest of the plan later with it
                return false;
            }
            a.rotation.start = a.rotation.start.max(now - p.at);
            a.rotation.next += 1;
            if row.cast > 0.0 {
                self.ally_begins_cast(k, p.spell);
            } else if row.channelled() {
                self.ally_begins_channel(k, p.spell);
            } else {
                self.ally_cast_lands(k, p.spell);
            }
        }
        let a = &self.assists.allies[k];
        a.rotation.done() && now >= a.until
    }

    /// Ally `k` begins channelling rotation spell `spell` (Blizzard): it stops where it stands,
    /// its storm goes down under the enemy nearest it at the spell's radius for the channel's
    /// time, and the spell goes off from it at the storm (its channel look follows from
    /// `Rotation::channeling`). Its round comes in the channel's ticks.
    fn ally_begins_channel(&mut self, k: usize, spell: usize) {
        let now = self.now;
        let row = self.assists.allies[k].rotation.spells[spell];
        let at = match self.rotation_aim(k, CastOn::Ground) {
            Aim::Ground(p) => p,
            _ => self.assists.allies[k].segment.position(now),
        };
        let area = self.new_id();
        let a = &mut self.assists.allies[k];
        let here = a.segment.position(now);
        if a.segment.moving(now) {
            a.segment = Segment::standing(here, now);
            a.moving = AllyMove::Run;
        }
        if dist(here, at) > 1e-3 {
            a.facing = toward(here, at);
        }
        let id = a.id;
        let every = row.tick_every.max(0.05);
        a.rotation.channeling = Some(Channeling { spell, area, at, radius: row.radius, every, ticks: row.ticks(), ticked: 0, next_tick: now + every, ends: now + row.channel });
        let hits = self.enemies_within(at, row.radius).into_iter().map(Unit::Enemy).collect();
        self.push(Event::SpellGo { caster: Unit::Ally(id), spell: row.spell, hits, aim: Aim::Ground(at), ammo: false, reach: 0.0, triggered: false });
        self.push(Event::AreaShown { id: area, caster: Unit::Ally(id), spell: row.spell, at, radius: row.radius, until: Some(now + row.channel + AREA_LOOK_AFTER) });
    }

    /// Ally `k`'s channel ticks that are due: each strikes what its round strikes (a ground area
    /// of the round: the enemies in the storm), each enemy once, for the assist's damage over the
    /// tick's interval, heals a `1 / ticks` part of the round's heals, and slows the enemies in the
    /// storm by `CHANNEL_SLOW` until the next tick (through the hero's own slow, so a boss or a
    /// rare takes it as from a hero). The channel ends with its last tick.
    fn ally_channel_ticks(&mut self, k: usize) {
        let now = self.now;
        loop {
            let a = &mut self.assists.allies[k];
            let Some(ch) = a.rotation.channeling.as_mut() else { return };
            if ch.ticked >= ch.ticks {
                a.rotation.channeling = None;
                a.rotation.next_move = now;
                return;
            }
            if now < ch.next_tick - 1e-4 {
                return;
            }
            ch.ticked += 1;
            ch.next_tick += ch.every;
            let (ch, h, id, character) = (ch.clone(), a.hero, a.id, a.character.clone());
            let part = 1.0 / ch.ticks as f32;
            let round = a.rotation.round.clone();
            // the tick is one damage event: each enemy takes it once
            self.assists.event_struck = Some(Vec::new());
            for s in &round {
                match s {
                    AssistStep::Area { damage: true, most, .. } => {
                        for e in self.enemies_nearest_within(ch.at, ch.radius, *most) {
                            self.ally_strikes(h, id, &character, e, Some(ch.every), &[], &[], None);
                        }
                    }
                    AssistStep::Heal { share: heal, look_spell } => {
                        self.ally_step(k, &AssistStep::Heal { share: heal * part, look_spell: *look_spell }, ch.every);
                    }
                    other => {
                        self.ally_step(k, other, ch.every);
                    }
                }
            }
            self.assists.event_struck = None;
            let slow = SpellEffect::Slow { amount: CHANNEL_SLOW, seconds: ch.every };
            for e in self.enemies_within(ch.at, ch.radius) {
                self.assist_effect(h, e, &slow, 0.0);
            }
        }
    }

    /// Ally `k` that only auto attacks swings at the enemy nearest it when in melee, every
    /// `AUTO_ATTACK_SECONDS`, facing it; the swing deals the assist's damage over
    /// `AUTO_ATTACK_SECONDS` as any of its hits (no hero bonus, a boss's shield taking its share),
    /// logged as its swing.
    fn ally_auto_attacks(&mut self, k: usize) {
        let now = self.now;
        let a = &self.assists.allies[k];
        if !a.rotation.auto_attacks_only() || now < a.rotation.next_swing {
            return;
        }
        let ring = self.zone().enemy_spawn_ring.0;
        let Some(e) = self.pick_targets(a, &Targets::NearAlly { radius: ring, most: 1 }).first().copied() else { return };
        let Some(p) = self.enemy_pos(e) else { return };
        let here = a.segment.position(now);
        if dist(here, p) > ALLY_BESIDE_ENEMY + MELEE_SLACK {
            return;
        }
        let a = &mut self.assists.allies[k];
        a.rotation.next_swing = now + AUTO_ATTACK_SECONDS;
        a.facing = toward(here, p);
        let (id, h, character) = (a.id, a.hero, a.character.clone());
        self.push(Event::AllySwing { id, target: e });
        self.ally_strikes(h, id, &character, e, Some(AUTO_ATTACK_SECONDS), &[], &[], None);
    }

    /// Where ally `k`'s rotation spell `spell` is aimed now: at the enemy nearest it, the hero,
    /// itself, or the ground under the enemy nearest it (itself when none is there).
    fn rotation_aim(&self, k: usize, on: CastOn) -> Aim {
        let a = &self.assists.allies[k];
        let ring = self.zone().enemy_spawn_ring.0;
        let nearest = self.pick_targets(a, &Targets::NearAlly { radius: ring, most: 1 }).first().copied();
        match on {
            CastOn::Hero => Aim::Unit(Unit::Hero(self.heroes[a.hero].id)),
            CastOn::Caster => Aim::Caster,
            CastOn::Enemy => nearest.map_or(Aim::Caster, |e| Aim::Unit(Unit::Enemy(e))),
            CastOn::Ground => Aim::Ground(nearest.and_then(|e| self.enemy_pos(e)).unwrap_or_else(|| a.segment.position(self.now))),
        }
    }

    /// Ally `k` begins casting rotation spell `spell` (one with a cast time): it stops where it
    /// stands, faces its aim, and the cast lands at its time.
    fn ally_begins_cast(&mut self, k: usize, spell: usize) {
        let now = self.now;
        let row = self.assists.allies[k].rotation.spells[spell];
        let aim = self.rotation_aim(k, row.on);
        let a = &mut self.assists.allies[k];
        let here = a.segment.position(now);
        if a.segment.moving(now) {
            a.segment = Segment::standing(here, now);
            a.moving = AllyMove::Run;
        }
        let id = a.id;
        a.rotation.casting = Some(Casting { spell, lands: now + row.cast, aim: aim.clone() });
        self.push(Event::AllyCastStarted { id, spell: row.spell, seconds: row.cast, aim });
    }

    /// Ally `k`'s rotation spell `spell` lands: one round of the script's damaging steps, each
    /// enemy struck once for the assist's damage over the cast's cycle (its cast time, at least the
    /// global cooldown), and of its heals at the ally's share; the spell going off from it (its
    /// look on what the round struck), and a nova-kiting caster jumps away from a close enemy
    /// after an instant one.
    fn ally_cast_lands(&mut self, k: usize, spell: usize) {
        let now = self.now;
        let (id, row, character) = {
            let a = &self.assists.allies[k];
            (a.id, a.rotation.spells[spell], a.character.clone())
        };
        let aim = self.rotation_aim(k, row.on);
        let before = self.events.len();
        let round = self.assists.allies[k].rotation.round.clone();
        let cycle = row.cast.max(GLOBAL_COOLDOWN);
        self.assists.event_struck = Some(Vec::new());
        for s in &round {
            self.ally_step(k, s, cycle);
        }
        self.assists.event_struck = None;
        // what the round struck, and where its ground area went
        let mut struck: Vec<Unit> = Vec::new();
        let mut area_at = None;
        for ev in &self.events[before..] {
            match ev {
                Event::EnemyHit { id: e, source: HitSource::Assist(c), .. } if *c == character && !struck.contains(&Unit::Enemy(*e)) => struck.push(Unit::Enemy(*e)),
                Event::AreaShown { caster: Unit::Ally(x), at, .. } if *x == id && area_at.is_none() => area_at = Some(*at),
                _ => {}
            }
        }
        let (aim, hits) = match aim {
            Aim::Unit(Unit::Hero(h)) => (Aim::Unit(Unit::Hero(h)), vec![Unit::Hero(h)]),
            Aim::Unit(target) => {
                let first = struck.first().copied().unwrap_or(target);
                let mut hits = struck.clone();
                if !hits.contains(&first) {
                    hits.insert(0, first);
                }
                (Aim::Unit(first), hits)
            }
            Aim::Ground(spot) => (Aim::Ground(area_at.unwrap_or(spot)), struck),
            Aim::Caster => (Aim::Caster, struck),
        };
        if let Aim::Unit(Unit::Enemy(e)) = aim {
            if let Some(p) = self.enemy_pos(e) {
                let a = &mut self.assists.allies[k];
                a.facing = toward(a.segment.position(now), p);
            }
        }
        // the spell goes off before what it dealt, so its hits show under it
        let go = Event::SpellGo { caster: Unit::Ally(id), spell: row.spell, hits, aim, ammo: false, reach: 0.0, triggered: false };
        self.events.insert(before.min(self.events.len()), go);
        let a = &mut self.assists.allies[k];
        a.rotation.next_move = now;
        if a.rotation.style == MoveStyle::NovaKiteCaster && row.cast == 0.0 {
            let here = a.segment.position(now);
            if let Some(&e) = self.enemies_within(here, KITE_CLOSE).first() {
                if let Some(p) = self.enemy_pos(e) {
                    let away = toward(p, here);
                    let to = step(here, away, ALLY_RUN_SPEED * JUMP_SECONDS);
                    self.move_ally(k, to, step(to, away, 1.0), AllyMove::Jump, JUMP_SECONDS, None);
                    // it lands facing the enemies again
                    self.assists.allies[k].facing = toward(to, p);
                }
            }
        }
    }

    /// Ally `k` moves as its style says, while it is not casting and its last move is done (a run
    /// is looked at again every `MOVE_LOOK_EVERY`).
    fn ally_moves_in_rotation(&mut self, k: usize) {
        let now = self.now;
        let a = &self.assists.allies[k];
        if a.rotation.casting.is_some() || a.rotation.channeling.is_some() || now < a.rotation.next_move {
            return;
        }
        if a.segment.moving(now) && a.moving != AllyMove::Run {
            return;
        }
        let (style, here, h) = (a.rotation.style, a.segment.position(now), a.hero);
        let hero = self.heroes[h].pos;
        let ring = self.zone().enemy_spawn_ring.0;
        let nearest = self.pick_targets(a, &Targets::NearAlly { radius: ring, most: 1 }).first().copied();
        self.assists.allies[k].rotation.next_move = now + MOVE_LOOK_EVERY;
        match style {
            MoveStyle::Melee | MoveStyle::ErraticMelee => {
                let Some(e) = nearest else { return };
                let Some(p) = self.enemy_pos(e) else { return };
                let d = dist(here, p);
                if d > ALLY_BESIDE_ENEMY + MELEE_SLACK {
                    let to = step(p, toward(p, here), ALLY_BESIDE_ENEMY);
                    self.move_ally(k, to, p, AllyMove::Run, dist(here, to) / ALLY_RUN_SPEED, None);
                } else if style == MoveStyle::ErraticMelee && !self.assists.allies[k].segment.moving(now) {
                    let a = &mut self.assists.allies[k];
                    a.rotation.strafes += 1;
                    let n = a.rotation.strafes;
                    let left = n % 2 == 1;
                    // sideways round its enemy: left of the facing, then right
                    let side = toward(here, p) + if left { std::f32::consts::FRAC_PI_2 } else { -std::f32::consts::FRAC_PI_2 };
                    if n % JUMP_EVERY == 0 {
                        let to = step(here, side, ALLY_RUN_SPEED * JUMP_SECONDS * 0.5);
                        self.move_ally(k, to, p, AllyMove::Jump, JUMP_SECONDS, None);
                    } else {
                        let to = step(here, side, ALLY_RUN_SPEED * STRAFE_SECONDS);
                        let how = if left { AllyMove::StrafeLeft } else { AllyMove::StrafeRight };
                        self.move_ally(k, to, p, how, STRAFE_SECONDS, None);
                    }
                }
            }
            // it does not move after its script
            MoveStyle::LeavesAfterScript => {}
            MoveStyle::Caster | MoveStyle::NovaKiteCaster | MoveStyle::Healer => {
                let follow = if style == MoveStyle::Healer { HEALER_FOLLOW } else { CASTER_FOLLOW };
                if dist(here, hero) > follow {
                    // back beside the hero, on its left, where it came
                    let side = self.heroes[h].facing + std::f32::consts::FRAC_PI_2;
                    let to = step(hero, side, ALLY_BESIDE_HERO);
                    self.move_ally(k, to, step(to, toward(here, to), 1.0), AllyMove::Run, dist(here, to) / ALLY_RUN_SPEED, None);
                }
            }
        }
    }
}
