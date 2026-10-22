//! Lasting ground zones and the stacking effects they leave
//! (docs/game/bosses.md, "Building blocks"): fire, poison, plague or webs that stay a while, hurt
//! the heroes standing in them on each tick and may add a stack of an effect over time; they can
//! drift towards the nearest hero or follow their boss (an aura). They never touch the horde, and
//! never slow or hold a hero: damage only.

use esurv_content::{Affliction, LastingZone};

use super::enemy_ai::Vec2;
use super::run::{Event, Run};

/// A drifting or following zone's look is put down again once it has moved this far.
pub const LOOK_MOVES_AFTER: f32 = 1.5;

/// A lasting zone on the ground.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundZone {
    pub id: u32,
    pub source: u32,
    pub spec: LastingZone,
    pub at: Vec2,
    /// Where its look was last put down.
    pub shown_at: Vec2,
    /// When it goes (`None`: with its boss).
    pub until: Option<f32>,
    pub next_tick: f32,
    /// What one tick does (its share of the boss's attack damage when it was laid).
    pub damage: f32,
    /// The boss's attack damage when it was laid (a stack's damage scales off it).
    pub attack_damage: f32,
}

/// A stacking effect on a hero (damage only).
#[derive(Debug, Clone, PartialEq)]
pub struct HeroAffliction {
    pub spec: Affliction,
    pub stacks: u32,
    /// What each stack does each tick.
    pub per_stack: f32,
    pub next_tick: f32,
    pub until: f32,
    /// The share of its damage the hero deals less while it lasts (a class call; 0 for a stack).
    pub weakens: f32,
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Run {
    /// Puts zone `spec` down at `at` for boss `source` (attack damage `attack_damage`).
    pub(crate) fn lay_zone(&mut self, spec: &LastingZone, at: Vec2, source: u32, attack_damage: f32) {
        let id = self.new_id();
        let now = self.now;
        let until = (spec.seconds > 0.0).then_some(now + spec.seconds);
        let caster = self.first_hero_unit();
        self.push(Event::AreaShown { id, caster, spell: spec.look_spell, at, radius: spec.radius, until: None });
        self.ground_zones.push(GroundZone {
            id,
            source,
            spec: spec.clone(),
            at,
            shown_at: at,
            until,
            next_tick: now + spec.every,
            damage: spec.damage_mult * attack_damage,
            attack_damage,
        });
    }

    /// Zones move, tick and go; the heroes' stacks tick and wear off.
    pub(crate) fn ground_zones_tick(&mut self, dt: f32) {
        let now = self.now;
        let mut k = 0;
        while k < self.ground_zones.len() {
            let source_at = self.enemies.iter().find(|e| e.id == self.ground_zones[k].source).map(|e| e.position(now));
            let z = &self.ground_zones[k];
            // gone with its time, or (lasting while its boss lives, or following it) with its boss
            let ended = z.until.is_some_and(|u| now >= u) || ((z.until.is_none() || z.spec.follows) && source_at.is_none());
            if ended {
                let id = z.id;
                self.ground_zones.remove(k);
                self.push(Event::AreaGone { id });
                continue;
            }
            let nearest = self.heroes.iter().filter(|h| h.alive).map(|h| h.pos).min_by(|a, b| dist(*a, z.at).total_cmp(&dist(*b, z.at)));
            let z = &mut self.ground_zones[k];
            if z.spec.follows {
                if let Some(p) = source_at {
                    z.at = p;
                }
            } else if let (true, Some(target)) = (z.spec.drift > 0.0, nearest) {
                let d = dist(z.at, target);
                if d > 0.1 {
                    let step = (z.spec.drift * dt).min(d);
                    z.at = (z.at.0 + (target.0 - z.at.0) / d * step, z.at.1 + (target.1 - z.at.1) / d * step);
                }
            }
            if dist(z.at, z.shown_at) >= LOOK_MOVES_AFTER {
                z.shown_at = z.at;
                let (id, at, radius) = (z.id, z.at, z.spec.radius);
                self.push(Event::AreaChanged { id, at, radius });
            }
            let z = &self.ground_zones[k];
            if now >= z.next_tick {
                let (at, radius, damage, affliction, attack) = (z.at, z.spec.radius, z.damage, z.spec.affliction, z.attack_damage);
                self.ground_zones[k].next_tick += self.ground_zones[k].spec.every;
                self.extras.blame = Some(self.ground_zones[k].source);
                for h in 0..self.heroes.len() {
                    if self.heroes[h].alive && dist(self.heroes[h].pos, at) <= radius {
                        if damage > 0.0 {
                            self.damage_hero(h, damage, None, false);
                        }
                        if let Some(a) = affliction {
                            self.stack_on_hero(h, a, attack);
                        }
                    }
                }
                self.extras.blame = None;
            }
            k += 1;
        }
        self.afflictions_tick();
    }

    /// Hero `h` takes a stack of `spec` (each stack `spec.damage_mult` × `attack_damage` a tick);
    /// a new stack renews the others' time.
    pub(crate) fn stack_on_hero(&mut self, h: usize, spec: Affliction, attack_damage: f32) {
        let now = self.now;
        let hero = &mut self.heroes[h];
        match hero.afflictions.iter_mut().find(|a| a.spec.look_spell == spec.look_spell) {
            Some(a) => {
                a.stacks = (a.stacks + 1).min(spec.max_stacks.max(1));
                a.until = now + spec.seconds;
                a.per_stack = a.per_stack.max(spec.damage_mult * attack_damage);
            }
            None => hero.afflictions.push(HeroAffliction { spec, stacks: 1, per_stack: spec.damage_mult * attack_damage, next_tick: now + spec.every, until: now + spec.seconds, weakens: 0.0 }),
        }
    }

    fn afflictions_tick(&mut self) {
        let now = self.now;
        for h in 0..self.heroes.len() {
            if !self.heroes[h].alive {
                self.heroes[h].afflictions.clear();
                continue;
            }
            let mut hurt = 0.0;
            for a in &mut self.heroes[h].afflictions {
                while now >= a.next_tick && a.next_tick <= a.until {
                    hurt += a.per_stack * a.stacks as f32;
                    a.next_tick += a.spec.every;
                }
            }
            self.heroes[h].afflictions.retain(|a| now < a.until);
            if hurt > 0.0 {
                self.damage_hero(h, hurt, None, false);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use esurv_content::Content;

    use super::*;
    use crate::game::enemy::RunOptions;
    use crate::game::run::{Flat, HeroSetup};

    pub(crate) fn quiet_run() -> Run {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Arc::new(Content::load(dir).expect("content loads"));
        let hero = HeroSetup { class: "warrior".into(), race: "orc".into(), ..Default::default() };
        let mut r = Run::new(content, "elwynn", RunOptions::default(), &[hero], Box::new(Flat), 9).unwrap();
        let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
        r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        r.enemies.clear();
        r.pickups.clear();
        r.heroes[0].spells.known.clear();
        r.heroes[0].spells.racial = None;
        r.drain_events();
        r
    }

    fn zone(drift: f32, affliction: Option<Affliction>) -> LastingZone {
        LastingZone { look_spell: 3256, radius: 4.0, seconds: 6.0, damage_mult: 0.5, every: 1.0, drift, follows: false, affliction }
    }

    #[test]
    fn a_zone_ticks_on_a_hero_inside_only_and_goes_with_its_time() {
        let mut r = quiet_run();
        let p = r.heroes[0].pos;
        r.lay_zone(&zone(0.0, None), (p.0 + 10.0, p.1), 1, 10.0);
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::AreaShown { spell: 3256, radius, .. } if *radius == 4.0)));
        let health = r.heroes[0].health;
        r.now += 1.0;
        r.ground_zones_tick(0.0);
        assert_eq!(r.heroes[0].health, health, "outside: nothing");
        r.lay_zone(&zone(0.0, None), p, 1, 10.0);
        r.now += 1.0;
        r.ground_zones_tick(0.0);
        assert!(r.heroes[0].health < health, "inside: a tick");
        r.now += 10.0;
        r.ground_zones_tick(0.0);
        assert!(r.ground_zones.is_empty());
        assert_eq!(r.drain_events().iter().filter(|e| matches!(e, Event::AreaGone { .. })).count(), 2);
    }

    #[test]
    fn a_drifting_zone_moves_towards_the_hero_and_its_look_follows() {
        let mut r = quiet_run();
        let p = r.heroes[0].pos;
        r.lay_zone(&zone(2.0, None), (p.0 + 10.0, p.1), 1, 10.0);
        r.drain_events();
        for _ in 0..10 {
            r.now += 0.1;
            r.ground_zones_tick(0.1);
        }
        assert!((r.ground_zones[0].at.0 - (p.0 + 8.0)).abs() < 0.01, "2 yd in 1 s");
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::AreaChanged { .. })));
    }

    #[test]
    fn a_zone_stacks_its_effect_which_ticks_by_its_stacks_and_wears_off() {
        let aff = Affliction { look_spell: 744, damage_mult: 0.1, every: 1.0, seconds: 3.0, max_stacks: 5 };
        let mut r = quiet_run();
        let p = r.heroes[0].pos;
        let mut z = zone(0.0, Some(aff));
        z.damage_mult = 0.0;
        z.seconds = 2.5;
        r.lay_zone(&z, p, 1, 100.0);
        for _ in 0..2 {
            r.now += 1.0;
            r.ground_zones_tick(0.0);
        }
        assert_eq!(r.heroes[0].afflictions[0].stacks, 2);
        // no slowing, no buff: damage only
        assert!(r.heroes[0].buffs.is_empty());
        r.now += 10.0;
        r.ground_zones_tick(0.0);
        assert!(r.heroes[0].afflictions.is_empty(), "worn off");
    }
}
