//! Marked attacks (docs/game/bosses.md, "Building blocks"): a boss's
//! area attack is laid on the ground as a shape (circle, ring, cone, line) and shown for its whole
//! wind-up, filling as it goes, before it lands; it lands on the heroes still inside it, never on
//! the horde. A rush is a marked line from the boss through the hero's spot that the boss then runs
//! from end to end, hitting a hero on the line as it passes. What a mark leaves where it lands
//! (a lasting zone, an object) is `ground_zones`' and `interactables`'.

use esurv_content::{BossAbilityKind, Brings, GroundObject, LastingZone, MarkAt, Shape, Sweep};

use super::enemy_ai::{Segment, Vec2};
use super::run::{Aim, Event, Run, Unit};

/// A rush's line runs this far past the hero's spot (the charge carries
/// through), never longer than the ability's own `length`.
pub const RUSH_PAST_HERO: f32 = 8.0;
/// How fast a rushing boss runs its line once the mark lands, yards a second (a charge's).
pub const RUSH_SPEED: f32 = super::boss_abilities::CHARGE_SPEED;
/// The most impact looks one mark plays where it lands (one per covering disc, a cone's or a
/// line's spread along it).
pub const MOST_IMPACTS: usize = 6;
/// The most lasting zones one mark leaves (a cone's puddles along it).
pub const MOST_LEFT_ZONES: usize = 4;
/// A bunny (`Event::ImpactAtSpot`) stays this long after its look lands, for the look to play out.
pub const BUNNY_LINGER: f32 = 2.0;
/// Marks laid at the hero from several adds: all but the first this far (yards) round its spot.
pub const ADDS_SPREAD: (f32, f32) = (3.5, 7.0);

/// A mark on the ground, waiting to land.
#[derive(Debug, Clone, PartialEq)]
pub struct Telegraph {
    pub id: u32,
    /// The boss that laid it.
    pub source: u32,
    pub shape: Shape,
    pub at: Vec2,
    pub facing: f32,
    pub lands_at: f32,
    /// What a hero inside it takes when it lands.
    pub damage: f32,
    pub impact_spell: u32,
    pub leaves: Option<LastingZone>,
    pub leaves_object: Option<GroundObject>,
    /// The boss's attack damage when it was cast (a left zone's ticks scale off it).
    pub attack_damage: f32,
    /// When the impact flies from the boss (a missile, sent to land as the wind-up ends).
    pub missile_at: Option<f32>,
    /// The boss runs along it as it lands (a rush).
    pub rush: bool,
    /// Laid round its boss: the boss plays the impact itself (its own spell, round it).
    pub from_boss: bool,
    /// Its impact plays on a bunny at the spot (`ImpactAtSpot`).
    pub on_bunny: bool,
    /// What lands with it: adds bursting from it, a beam that stays and sweeps.
    pub brings: Option<Brings>,
    pub sweep: Option<Sweep>,
}

impl Telegraph {
    /// A line's start, far end and width (the very line its hits are tested against, for a
    /// client drawing it); none for the other shapes.
    pub fn line_ends(&self) -> Option<(Vec2, Vec2, f32)> {
        line_ends(self.shape, self.at, self.facing)
    }
}

/// Line `shape` laid at `at` facing `facing`: its start, far end and width.
pub fn line_ends(shape: Shape, at: Vec2, facing: f32) -> Option<(Vec2, Vec2, f32)> {
    match shape {
        Shape::Line { length, width } => Some((at, (at.0 + facing.cos() * length, at.1 + facing.sin() * length), width)),
        _ => None,
    }
}

/// A boss running its rush line: from `from` along `facing`, over
/// `shape` (the line its mark showed). A hero on the line when the boss passes it takes `damage`,
/// once a run.
#[derive(Debug, Clone, PartialEq)]
pub struct ChargeRun {
    pub source: u32,
    pub from: Vec2,
    pub facing: f32,
    pub shape: Shape,
    pub damage: f32,
    /// When the boss reaches the far end.
    pub until: f32,
    /// How far along the line the boss had come at the last check.
    pub reached: f32,
    /// The heroes it already hit.
    pub hit: Vec<u32>,
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// The facing from `from` to `to` (along +x when they meet).
pub fn facing(from: Vec2, to: Vec2) -> f32 {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    if dx.abs() + dy.abs() < 1e-4 {
        0.0
    } else {
        dy.atan2(dx)
    }
}

/// Where `at` lays its marks for a boss at `boss` and a hero at `hero`: each mark's origin and
/// facing. `spots` draws the extra marks' spots round the hero.
pub fn mark_spots(at: MarkAt, boss: Vec2, hero: Vec2, mut spot: impl FnMut(f32) -> Vec2) -> Vec<(Vec2, f32)> {
    let towards = facing(boss, hero);
    match at {
        MarkAt::Boss => vec![(boss, towards)],
        MarkAt::Hero => vec![(hero, towards)],
        // the spots the lines cross (their facings and origins are drawn by the caster)
        MarkAt::NearHero { count, spread } | MarkAt::Across { count, spread } => {
            let mut out = vec![(hero, towards)];
            for _ in 1..count.max(1) {
                let off = spot(spread);
                out.push(((hero.0 + off.0, hero.1 + off.1), towards));
            }
            out
        }
    }
}

impl Run {
    /// Enemy `k` casts building block `kind` at hero `h` with attack damage `damage`: marks,
    /// rushes, objects and zones (the other kinds are `boss_abilities::cast`'s).
    pub(crate) fn cast_building_block(&mut self, k: usize, h: usize, kind: &BossAbilityKind, damage: f32) {
        self.cast_block(k, h, kind, damage, None);
    }

    /// [`Run::cast_building_block`], laid from `spot` in place of the boss when set (an object's
    /// pulse).
    pub(crate) fn cast_block(&mut self, k: usize, h: usize, kind: &BossAbilityKind, damage: f32, spot: Option<Vec2>) {
        let now = self.now;
        let (own, id) = (self.enemies[k].position(now), self.enemies[k].id);
        let boss = spot.unwrap_or(own);
        let hero = self.heroes[h].pos;
        // a mark laid from adds: from each living one of that name; with none up, nothing
        let from_adds: Option<Vec<(u32, Vec2)>> = match kind {
            BossAbilityKind::Marked { from: Some(name), .. } => {
                let zone = self.zone();
                Some(self.enemies.iter().filter(|e| matches!(e.kind, super::run::EnemyKind::Roster(r) if zone.enemies[r].name == *name)).map(|e| (e.id, e.position(now))).collect())
            }
            _ => None,
        };
        if from_adds.as_ref().is_some_and(|a| a.is_empty()) {
            return;
        }
        if let Some(w) = kind.warning() {
            self.announce_warning(w, kind_look(kind));
        }
        match kind {
            BossAbilityKind::Marked { shape, at, wind_up, damage_mult, impact_spell, warning_spell, fill_spell, leaves, leaves_object, missile, impact_on_bunny, chain, brings, sweep, .. } => {
                let by_adds = from_adds.is_some();
                for (n, (source, from)) in from_adds.unwrap_or_else(|| vec![(id, boss)]).into_iter().enumerate() {
                    let mut rng = std::mem::replace(&mut self.rng, crate::rng::Rng::new(0));
                    // several adds aiming at the hero: the first on its spot, the others round it, so
                    // their marks do not all land on one spot
                    let aim = match *at {
                        MarkAt::Hero if n > 0 => {
                            let a = rng.range(0.0, std::f32::consts::TAU);
                            let r = rng.range(ADDS_SPREAD.0, ADDS_SPREAD.1);
                            (hero.0 + a.cos() * r, hero.1 + a.sin() * r)
                        }
                        _ => hero,
                    };
                    let mut spots = mark_spots(*at, from, aim, |spread| {
                        let a = rng.range(0.0, std::f32::consts::TAU);
                        let r = rng.range(spread * 0.4, spread);
                        (a.cos() * r, a.sin() * r)
                    });
                    // lines across the ground: each through its spot, at a facing of its own
                    if matches!(at, MarkAt::Across { .. }) {
                        let half = shape.reach() / 2.0;
                        for (spot, face) in &mut spots {
                            *face = rng.range(0.0, std::f32::consts::TAU);
                            *spot = (spot.0 - face.cos() * half, spot.1 - face.sin() * half);
                        }
                    }
                    self.rng = rng;
                    for (i, (origin, face)) in spots.into_iter().enumerate() {
                        // a chain: each further mark lands a little after the one before
                        let wind_up = wind_up + chain * i as f32;
                        let lands_at = now + wind_up;
                        let missile_at = missile.then(|| {
                            let speed = self.world.missile_speed(*impact_spell);
                            if speed > 0.0 {
                                (lands_at - dist(from, origin) / speed).max(now)
                            } else {
                                now
                            }
                        });
                        self.lay_mark(Telegraph {
                            id: 0,
                            source,
                            shape: *shape,
                            at: origin,
                            facing: face,
                            lands_at,
                            damage: damage_mult * damage,
                            impact_spell: *impact_spell,
                            leaves: leaves.clone(),
                            leaves_object: leaves_object.clone(),
                            attack_damage: damage,
                            missile_at,
                            rush: false,
                            // round its caster (an add's too, not an object's): it plays the impact
                            from_boss: matches!(at, MarkAt::Boss) && spot.is_none(),
                            on_bunny: *impact_on_bunny,
                            brings: brings.clone(),
                            sweep: *sweep,
                        }, *warning_spell, *fill_spell, wind_up);
                    }
                }
                // a boss laying a mark round itself stands while it winds up (and while its beam
                // sweeps)
                if matches!(at, MarkAt::Boss) && spot.is_none() && !by_adds {
                    self.hold_boss_still(k, now + wind_up + sweep.map_or(0.0, |s| s.seconds));
                }
            }
            BossAbilityKind::Rush { look_spell, length, width, wind_up, damage_mult, warning_spell, fill_spell, fade_spell, .. } => {
                let lands_at = now + wind_up;
                // from the boss through the hero's spot and on past it
                let reach = (dist(boss, hero) + RUSH_PAST_HERO).min(*length);
                self.lay_mark(Telegraph {
                    id: 0,
                    source: id,
                    shape: Shape::Line { length: reach, width: *width },
                    at: boss,
                    facing: facing(boss, hero),
                    lands_at,
                    damage: damage_mult * damage,
                    impact_spell: *look_spell,
                    leaves: None,
                    leaves_object: None,
                    attack_damage: damage,
                    missile_at: None,
                    rush: true,
                    from_boss: true,
                    on_bunny: false,
                    brings: None,
                    sweep: None,
                }, *warning_spell, *fill_spell, *wind_up);
                // it stands while it winds up, then runs the line (no swing, no ability)
                self.hold_boss_still(k, lands_at + reach / RUSH_SPEED);
                if let Some(fade) = fade_spell {
                    self.enemies[k].kit.worn_looks.push((*fade, lands_at));
                }
            }
            BossAbilityKind::Lay { object, count, near, .. } => {
                for i in 0..*count {
                    self.place_object_spread(object.clone(), *near, Some(id), damage, (i, *count));
                }
            }
            BossAbilityKind::Zone { zone, at, .. } => {
                let spot = match at {
                    MarkAt::Boss => boss,
                    _ => hero,
                };
                self.lay_zone(zone, spot, id, damage);
            }
            _ => {}
        }
    }

    /// Puts mark `t` down (numbered now) and tells the client its shape and looks.
    pub(crate) fn lay_mark(&mut self, mut t: Telegraph, warning_spell: u32, fill_spell: u32, wind_up: f32) {
        t.id = self.new_id();
        self.push(Event::TelegraphShown { id: t.id, shape: t.shape, at: t.at, facing: t.facing, warning_spell, fill_spell, wind_up });
        self.telegraphs.push(t);
    }

    /// Enemy `k` stands still (no move, no swing, no ability) until `until`: a boss winding up a
    /// mark round itself. Its own state, not a hold: a hero's damage against held enemies does
    /// not count it.
    pub(crate) fn hold_boss_still(&mut self, k: usize, until: f32) {
        let now = self.now;
        let e = &mut self.enemies[k];
        e.mech.busy_until = e.mech.busy_until.max(until);
        let p = e.position(now);
        e.segment = Segment::standing(p, now);
    }

    /// The marks whose time has come land: their looks play, the heroes inside take their damage,
    /// and what they leave is put down.
    pub(crate) fn telegraphs_tick(&mut self) {
        let now = self.now;
        // missiles leave so they land as the wind-up ends
        for t in self.telegraphs.iter_mut().filter(|t| t.missile_at.is_some_and(|m| now >= m)) {
            t.missile_at = None;
            if self.enemies.iter().any(|e| e.id == t.source) {
                if t.on_bunny {
                    // it flies at a bunny waiting where it lands, so its impact shows there
                    let linger = t.lands_at - now + BUNNY_LINGER;
                    self.events.push(Event::ImpactAtSpot { caster: Unit::Enemy(t.source), spell: t.impact_spell, at: t.at, linger });
                } else {
                    self.events.push(Event::SpellGo { caster: Unit::Enemy(t.source), spell: t.impact_spell, hits: vec![], aim: Aim::Ground(t.at), ammo: false, reach: t.shape.reach() });
                }
            }
            t.impact_spell = 0;
        }
        let (due, later): (Vec<Telegraph>, Vec<Telegraph>) = std::mem::take(&mut self.telegraphs).into_iter().partition(|t| t.lands_at <= now);
        self.telegraphs = later;
        for t in due {
            self.land_mark(t);
        }
        self.charges_tick();
    }

    /// Rushing bosses run on along their lines: a living hero on the line that the boss passed
    /// since the last check takes the rush's damage, once a run (never an enemy:
    /// boss attacks hit the hero only). A run ends at the far end, or when the boss is gone or
    /// knocked off its line.
    pub(crate) fn charges_tick(&mut self) {
        let now = self.now;
        let mut i = 0;
        while i < self.charges.len() {
            let c = self.charges[i].clone();
            let Some(p) = self.enemies.iter().find(|e| e.id == c.source).map(|e| e.position(now)) else {
                self.charges.remove(i);
                continue;
            };
            let Shape::Line { width, .. } = c.shape else {
                self.charges.remove(i);
                continue;
            };
            let (fx, fy) = (c.facing.cos(), c.facing.sin());
            let along_of = |q: Vec2| (q.0 - c.from.0) * fx + (q.1 - c.from.1) * fy;
            let (dx, dy) = (p.0 - c.from.0, p.1 - c.from.1);
            if (-dx * fy + dy * fx).abs() > width / 2.0 {
                self.charges.remove(i);
                continue;
            }
            // the stretch the boss's front swept since the last check (its body half the line's width)
            let reached = along_of(p);
            let swept = (c.reached - width / 2.0)..=(reached + width / 2.0);
            self.extras.blame = Some(c.source);
            for h in 0..self.heroes.len() {
                let (id, pos) = (self.heroes[h].id, self.heroes[h].pos);
                if c.damage > 0.0 && self.heroes[h].alive && !c.hit.contains(&id) && c.shape.contains(c.from, c.facing, pos) && swept.contains(&along_of(pos)) {
                    self.charges[i].hit.push(id);
                    self.damage_hero(h, c.damage, None, false);
                }
            }
            self.extras.blame = None;
            if now >= c.until {
                self.charges.remove(i);
            } else {
                self.charges[i].reached = c.reached.max(reached);
                i += 1;
            }
        }
    }

    fn land_mark(&mut self, t: Telegraph) {
        let now = self.now;
        self.push(Event::TelegraphGone { id: t.id });
        let patches = t.shape.patches(t.at, t.facing);
        let alive = self.enemies.iter().any(|e| e.id == t.source);
        if let (Some(sweep), true) = (t.sweep, alive) {
            // a beam: it stays and sweeps round its boss (`game::beams`)
            self.start_beam(&t, sweep);
        } else if t.impact_spell != 0 && t.on_bunny && !t.rush {
            // a look shown only on a unit it hits: on bunnies over the shape
            let caster = if alive { Unit::Enemy(t.source) } else { self.first_hero_unit() };
            let step = patches.len().div_ceil(MOST_IMPACTS).max(1);
            for (c, _) in patches.iter().step_by(step) {
                self.push(Event::ImpactAtSpot { caster, spell: t.impact_spell, at: *c, linger: BUNNY_LINGER });
            }
        } else if t.impact_spell != 0 && t.from_boss && alive && !t.rush {
            // laid round the boss: it plays its own spell round itself (a slam, a stomp)
            self.push(Event::SpellGo { caster: Unit::Enemy(t.source), spell: t.impact_spell, hits: vec![], aim: Aim::Caster, ammo: false, reach: t.shape.reach() });
        } else if t.impact_spell != 0 && t.rush && alive {
            let end = (t.at.0 + t.facing.cos() * t.shape.reach(), t.at.1 + t.facing.sin() * t.shape.reach());
            self.push(Event::SpellGo { caster: Unit::Enemy(t.source), spell: t.impact_spell, hits: vec![], aim: Aim::Ground(end), ammo: false, reach: 0.0 });
        } else if t.impact_spell != 0 {
            let caster = if alive { Unit::Enemy(t.source) } else { self.first_hero_unit() };
            let step = patches.len().div_ceil(MOST_IMPACTS).max(1);
            for (c, r) in patches.iter().step_by(step) {
                self.push(Event::SpellGo { caster, spell: t.impact_spell, hits: vec![], aim: Aim::Ground(*c), ammo: false, reach: *r });
            }
        }
        if t.rush {
            // the boss runs its whole line, end to end, hitting as it passes (`charges_tick`)
            if let (Some(e), Some((from, to, _))) = (self.enemies.iter_mut().find(|e| e.id == t.source), t.line_ends()) {
                let duration = t.shape.reach() / RUSH_SPEED;
                e.segment = Segment { from, to, start: now, duration };
                e.charging_until = now + duration;
                e.next_decision = now + duration;
                e.mech.busy_until = e.mech.busy_until.max(now + duration);
                self.charges.push(ChargeRun { source: t.source, from, facing: t.facing, shape: t.shape, damage: t.damage, until: now + duration, reached: 0.0, hit: Vec::new() });
            }
        }
        // a mark lands on the heroes inside it only (boss attacks hit the hero
        // only), and comes from no one (an area, not the boss's swing); a hero falling to it falls
        // to its boss
        self.extras.blame = Some(t.source);
        for h in 0..self.heroes.len() {
            // a rush hits as its boss passes instead (`charges_tick`)
            if t.damage > 0.0 && !t.rush && self.heroes[h].alive && t.shape.contains(t.at, t.facing, self.heroes[h].pos) {
                self.damage_hero(h, t.damage, None, false);
            }
        }
        self.extras.blame = None;
        // adds burst from where it landed
        if let Some(b) = &t.brings {
            let before = self.enemies.len();
            self.wave_at_spot(&b.enemy, b.count, t.at, super::interactables::HATCH_RING);
            if b.rooted {
                self.enemies[before..].iter_mut().for_each(|e| e.mech.phases.rooted = true);
            }
        }
        if let Some(zone) = &t.leaves {
            let spots: Vec<Vec2> = match t.shape {
                Shape::Circle { .. } => vec![t.at],
                _ => {
                    let step = patches.len().div_ceil(MOST_LEFT_ZONES).max(1);
                    patches.iter().step_by(step).map(|(c, _)| *c).collect()
                }
            };
            for s in spots {
                self.lay_zone(zone, s, t.source, t.attack_damage);
            }
        }
        if let Some(o) = t.leaves_object {
            self.place_object_at(o, t.at, Some(t.source), t.attack_damage);
        }
    }

    /// The first hero, as the caster of a look whose boss is gone.
    pub(crate) fn first_hero_unit(&self) -> Unit {
        Unit::Hero(self.heroes.first().map_or(0, |h| h.id))
    }
}

/// The look a building block's warning shows as its icon when it names none.
pub fn kind_look(kind: &BossAbilityKind) -> u32 {
    match kind {
        BossAbilityKind::Marked { impact_spell, .. } => *impact_spell,
        BossAbilityKind::Rush { look_spell, .. } => *look_spell,
        BossAbilityKind::Lay { object, .. } => object.glow_spell,
        BossAbilityKind::Zone { zone, .. } => zone.look_spell,
        BossAbilityKind::GroundBurst { impact_spell, .. } => *impact_spell,
        BossAbilityKind::Volley { look_spell, .. } | BossAbilityKind::Stomp { look_spell, .. } | BossAbilityKind::Charge { look_spell, .. } => *look_spell,
        BossAbilityKind::Summon { .. } => 0,
    }
}

/// Whether `kind` is one of the building blocks (cast by [`Run::cast_building_block`]).
pub fn is_building_block(kind: &BossAbilityKind) -> bool {
    matches!(kind, BossAbilityKind::Marked { .. } | BossAbilityKind::Rush { .. } | BossAbilityKind::Lay { .. } | BossAbilityKind::Zone { .. })
}
