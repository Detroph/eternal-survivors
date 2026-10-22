//! Sweeping beams (docs/game/bosses.md, "Building blocks"): a marked
//! line that, once it lands, stays on its boss and turns round it for a while (C'Thun's Dark
//! Glare), drawn along its length on the ground as it goes. Each tick it hurts the heroes standing
//! in it, never the horde; walking round it ahead of its sweep dodges it.

use esurv_content::{Shape, Sweep};

use super::enemy_ai::Vec2;
use super::ground_zones::LOOK_MOVES_AFTER;
use super::run::{Aim, Event, Run, Unit};
use super::telegraphs::Telegraph;

/// The beam's own look (its spell, from its boss to its far end) plays again this often.
pub const BEAM_LOOK_EVERY: f32 = 1.0;

/// A beam sweeping round its boss.
#[derive(Debug, Clone, PartialEq)]
pub struct Beam {
    pub source: u32,
    pub shape: Shape,
    /// Its facing now (radians), and how fast it turns (radians a second).
    pub facing: f32,
    pub turn: f32,
    pub until: f32,
    pub every: f32,
    pub next_tick: f32,
    /// What one tick does to a hero in it.
    pub damage: f32,
    /// The boss's spell along it (0: none), played again every `BEAM_LOOK_EVERY`.
    pub spell: u32,
    pub next_look: f32,
    /// Its ground looks along it: (id, where each was last put down).
    pub looks: Vec<(u32, Vec2)>,
    pub look_spell: u32,
}

impl Run {
    /// Mark `t` landed as a beam: it stays on its boss and sweeps `sweep.turn` degrees a second.
    pub(crate) fn start_beam(&mut self, t: &Telegraph, sweep: Sweep) {
        let now = self.now;
        let caster = self.first_hero_unit();
        let mut looks = Vec::new();
        for (c, r) in t.shape.patches(t.at, t.facing) {
            let id = self.new_id();
            self.push(Event::AreaShown { id, caster, spell: sweep.look_spell, at: c, radius: r, until: None });
            looks.push((id, c));
        }
        self.beams.push(Beam {
            source: t.source,
            shape: t.shape,
            facing: t.facing,
            turn: sweep.turn.to_radians(),
            until: now + sweep.seconds,
            every: sweep.every.max(0.1),
            next_tick: now,
            damage: sweep.damage_mult * t.attack_damage,
            spell: t.impact_spell,
            next_look: now,
            looks,
            look_spell: sweep.look_spell,
        });
    }

    /// Beams turn with their boss, hurt the heroes in them on each tick, and go with their time
    /// or their boss.
    pub(crate) fn beams_tick(&mut self, dt: f32) {
        let now = self.now;
        let mut k = 0;
        while k < self.beams.len() {
            let origin = self.enemies.iter().find(|e| e.id == self.beams[k].source).map(|e| e.position(now));
            let Some(origin) = origin.filter(|_| now < self.beams[k].until) else {
                let b = self.beams.remove(k);
                for (id, _) in b.looks {
                    self.push(Event::AreaGone { id });
                }
                continue;
            };
            let b = &mut self.beams[k];
            b.facing += b.turn * dt;
            let patches = b.shape.patches(origin, b.facing);
            let mut moved = Vec::new();
            for ((id, shown), (c, r)) in b.looks.iter_mut().zip(&patches) {
                let d = ((c.0 - shown.0).powi(2) + (c.1 - shown.1).powi(2)).sqrt();
                if d >= LOOK_MOVES_AFTER {
                    *shown = *c;
                    moved.push((*id, *c, *r));
                }
            }
            let look = (b.spell != 0 && now >= b.next_look).then(|| {
                b.next_look = now + BEAM_LOOK_EVERY;
                let reach = b.shape.reach();
                (b.source, b.spell, (origin.0 + b.facing.cos() * reach, origin.1 + b.facing.sin() * reach))
            });
            let tick = (now >= b.next_tick).then(|| {
                b.next_tick += b.every;
                (b.shape, b.facing, b.damage)
            });
            for (id, at, radius) in moved {
                self.push(Event::AreaChanged { id, at, radius });
            }
            if let Some((source, spell, end)) = look {
                self.push(Event::SpellGo { caster: Unit::Enemy(source), spell, hits: vec![], aim: Aim::Ground(end), ammo: false, reach: 0.0, triggered: false });
            }
            // it hurts the heroes in it only, as an area
            if let Some((shape, facing, damage)) = tick {
                for h in 0..self.heroes.len() {
                    if self.heroes[h].alive && shape.contains(origin, facing, self.heroes[h].pos) {
                        self.damage_hero(h, damage, None, false);
                    }
                }
            }
            k += 1;
        }
    }
}
