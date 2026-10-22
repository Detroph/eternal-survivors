//! Objects on the ground a hero walks over (docs/game/bosses.md,
//! "Building blocks"): egg sacs, cocoons, totems, altars, fires, crystals and relics are broken or
//! set off by standing on them for a short contact (half a second in one go, so brushing past
//! does not count), never by damage; hero attacks never strike them. One left alone may hatch
//! into adds; one taken runs its script (a relic making its boss mortal), and may come back later.

use esurv_content::{GroundObject, ObjectNear};

use super::enemy_ai::Vec2;
use super::run::{Event, Run};

/// The contact's progress is told in steps of this share (the glow under it filling).
pub const PROGRESS_STEP: f32 = 0.25;
/// Hatchlings come this far round their object.
pub const HATCH_RING: f32 = 1.5;
/// Objects laid together fan out this far apart (radians) round their boss.
pub const SPREAD_TURN: f32 = 1.0;

/// An object lying on the ground.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldObject {
    pub id: u32,
    pub spec: GroundObject,
    pub at: Vec2,
    /// The boss it belongs to (its script runs for it), if any.
    pub source: Option<u32>,
    /// Seconds a hero has stood on it in one go.
    pub contact: f32,
    /// The progress last told (in `PROGRESS_STEP`s).
    pub told: u32,
    pub hatch_at: Option<f32>,
    /// The boss's attack damage when it was laid.
    pub attack_damage: f32,
    /// When its pulse runs next, and when it goes if left alone.
    pub pulse_at: Option<f32>,
    pub left_at: Option<f32>,
}

/// An object to put down again later (a relic that drops again).
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectDue {
    pub at: f32,
    pub spec: GroundObject,
    pub near: ObjectNear,
    pub source: Option<u32>,
    pub attack_damage: f32,
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Run {
    /// Puts `spec` down `near` boss `source` or the nearest hero.
    pub(crate) fn place_object(&mut self, spec: GroundObject, near: ObjectNear, source: Option<u32>, attack_damage: f32) {
        self.place_object_spread(spec, near, source, attack_damage, (0, 1));
    }

    /// [`Run::place_object`], the `nth.0`-th of `nth.1` laid together: they fan out round the boss
    /// (or the hero), apart enough that a hero on one stands on no other.
    pub(crate) fn place_object_spread(&mut self, spec: GroundObject, near: ObjectNear, source: Option<u32>, attack_damage: f32, nth: (u32, u32)) {
        let fan = (nth.0 as f32 - (nth.1 as f32 - 1.0) / 2.0) * SPREAD_TURN;
        let now = self.now;
        let boss = source.and_then(|s| self.enemies.iter().find(|e| e.id == s)).map(|e| e.position(now));
        let Some(h) = self.anchor_hero() else { return };
        let hero = self.heroes[h].pos;
        let at = match (near, boss) {
            (ObjectNear::Boss(d), Some(b)) => {
                // towards the hero, a little to a side
                let face = super::telegraphs::facing(b, hero) + fan + self.rng.range(-0.2, 0.2);
                (b.0 + face.cos() * d, b.1 + face.sin() * d)
            }
            (ObjectNear::Hero(d), _) | (ObjectNear::Boss(d), None) => {
                let a = self.rng.range(0.0, std::f32::consts::TAU) * f32::from(nth.0 == 0) + fan * 2.0;
                (hero.0 + a.cos() * d, hero.1 + a.sin() * d)
            }
        };
        self.place_object_at(spec, at, source, attack_damage);
    }

    /// Puts `spec` down at `at`.
    pub(crate) fn place_object_at(&mut self, spec: GroundObject, at: Vec2, source: Option<u32>, attack_damage: f32) {
        let id = self.new_id();
        let hatch_at = spec.hatch.as_ref().map(|h| self.now + h.after);
        let pulse_at = spec.pulse.as_ref().map(|p| self.now + p.seconds);
        let left_at = spec.left_alone.as_ref().map(|p| self.now + p.seconds);
        if let Some(w) = spec.warn.clone() {
            self.announce_warning(&w, spec.glow_spell);
        }
        self.push(Event::ObjectPlaced { id, name: spec.name.clone(), display_id: spec.display_id, scale: spec.scale, at, glow_spell: spec.glow_spell, radius: spec.radius });
        self.ground_objects.push(FieldObject { id, spec, at, source, contact: 0.0, told: 0, hatch_at, attack_damage, pulse_at, left_at });
    }

    /// Heroes standing on objects fill their contact (one who steps off starts again); a full
    /// contact breaks or sets one off; one left long enough hatches; objects due come back.
    pub(crate) fn ground_objects_tick(&mut self, dt: f32) {
        let now = self.now;
        let due: Vec<ObjectDue> = {
            let (due, later) = std::mem::take(&mut self.objects_due).into_iter().partition(|d| now >= d.at);
            self.objects_due = later;
            due
        };
        for d in due {
            // a relic of a boss that is gone does not come back
            if d.source.is_none_or(|s| self.enemies.iter().any(|e| e.id == s)) {
                self.place_object(d.spec, d.near, d.source, d.attack_damage);
            }
        }
        let mut k = 0;
        while k < self.ground_objects.len() {
            let o = &self.ground_objects[k];
            let on = self.heroes.iter().any(|h| h.alive && dist(h.pos, o.at) <= o.spec.radius);
            let o = &mut self.ground_objects[k];
            o.contact = if on { o.contact + dt } else { 0.0 };
            let progress = (o.contact / o.spec.contact.max(0.01)).min(1.0);
            let step = (progress / PROGRESS_STEP).floor() as u32;
            if step != o.told {
                o.told = step;
                let id = o.id;
                self.push(Event::ObjectContact { id, progress: step as f32 * PROGRESS_STEP });
            }
            let o = &self.ground_objects[k];
            if o.contact >= o.spec.contact {
                let o = self.ground_objects.remove(k);
                self.push(Event::ObjectGone { id: o.id, taken: true });
                if let Some(again) = o.spec.again_after {
                    let near = ObjectNear::Boss(dist(o.at, o.source.and_then(|s| self.enemies.iter().find(|e| e.id == s)).map_or(o.at, |e| e.position(now))).max(3.0));
                    self.objects_due.push(ObjectDue { at: now + again, spec: o.spec.clone(), near, source: o.source, attack_damage: o.attack_damage });
                }
                let actions = o.spec.on_contact.clone();
                self.run_actions(o.source, o.at, &actions);
                // the last of its name its boss had down (the fourth fire put out)
                let last = !self.ground_objects.iter().any(|x| x.source == o.source && x.spec.name == o.spec.name);
                if last && !o.spec.on_all_taken.is_empty() {
                    let actions = o.spec.on_all_taken.clone();
                    self.run_actions(o.source, o.at, &actions);
                }
                continue;
            }
            // a totem's pulse runs from its spot
            if o.pulse_at.is_some_and(|t| now >= t) {
                let (source, at, every) = (o.source, o.at, o.spec.pulse.as_ref().map_or(1.0, |p| p.seconds.max(0.1)));
                let actions = o.spec.pulse.as_ref().map(|p| p.run.clone()).unwrap_or_default();
                self.ground_objects[k].pulse_at = Some(now + every);
                let id = self.ground_objects[k].id;
                self.run_actions_at(source, at, &actions, true);
                // its actions may have put objects down or taken them: find it again
                let Some(again) = self.ground_objects.iter().position(|x| x.id == id) else { continue };
                k = again;
            }
            let o = &self.ground_objects[k];
            // left alone too long, it goes and does its harm (a sacrifice made)
            if o.left_at.is_some_and(|t| now >= t) {
                let o = self.ground_objects.remove(k);
                self.push(Event::ObjectGone { id: o.id, taken: false });
                let actions = o.spec.left_alone.as_ref().map(|p| p.run.clone()).unwrap_or_default();
                self.run_actions_at(o.source, o.at, &actions, false);
                continue;
            }
            if o.hatch_at.is_some_and(|t| now >= t) {
                let o = self.ground_objects.remove(k);
                self.push(Event::ObjectGone { id: o.id, taken: false });
                if let Some(h) = &o.spec.hatch {
                    self.wave_at_spot(&h.enemy, h.count, o.at, HATCH_RING);
                }
                continue;
            }
            k += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use esurv_content::Hatch;

    use super::*;

    fn egg(hatch: Option<Hatch>) -> GroundObject {
        GroundObject {
            name: "Egg".into(),
            display_id: 6171,
            scale: 1.0,
            glow_spell: 26573,
            contact: esurv_content::STANDARD_CONTACT,
            radius: esurv_content::STANDARD_TOUCH,
            hatch,
            on_contact: vec![],
            again_after: None,
            warn: None,
            pulse: None,
            left_alone: None,
            on_all_taken: vec![],
            creature_display: None,
        }
    }

    #[test]
    fn a_short_contact_breaks_an_object_and_brushing_past_does_not() {
        let mut r = crate::game::ground_zones::tests::quiet_run();
        let (id, p) = (r.heroes[0].id, r.heroes[0].pos);
        r.place_object_at(egg(None), (p.0 + 1.0, p.1), None, 10.0);
        // brushing past: 0.3 s on it, then off
        for _ in 0..3 {
            r.ground_objects_tick(0.1);
        }
        r.hero_moved(id, (p.0 + 6.0, p.1), 0.0, 0.0);
        r.ground_objects_tick(0.1);
        assert_eq!(r.ground_objects.len(), 1, "brushing past does not break it");
        assert_eq!(r.ground_objects[0].contact, 0.0, "stepping off starts again");
        r.hero_moved(id, p, 0.0, 0.0);
        let mut told = Vec::new();
        for _ in 0..6 {
            r.ground_objects_tick(0.1);
            told.extend(r.drain_events());
        }
        assert!(r.ground_objects.is_empty(), "half a second in one go breaks it");
        assert!(told.iter().any(|e| matches!(e, Event::ObjectContact { progress, .. } if *progress == 0.5)), "its glow fills");
        assert!(told.iter().any(|e| matches!(e, Event::ObjectGone { taken: true, .. })));
    }

    #[test]
    fn an_object_left_alone_hatches_and_hero_attacks_never_strike_it() {
        let mut r = crate::game::ground_zones::tests::quiet_run();
        let p = r.heroes[0].pos;
        let hatch = Hatch { after: 2.0, enemy: "Kobold Vermin".into(), count: 3 };
        r.place_object_at(egg(Some(hatch)), (p.0 + 15.0, p.1), None, 10.0);
        // it is no enemy: nothing a hero casts can pick it
        assert!(r.enemies.is_empty());
        r.now += 2.1;
        r.ground_objects_tick(0.1);
        assert!(r.ground_objects.is_empty());
        assert_eq!(r.enemies.len(), 3, "it hatched");
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::ObjectGone { taken: false, .. })));
    }

    #[test]
    fn a_relic_taken_comes_back_later() {
        let mut r = crate::game::ground_zones::tests::quiet_run();
        let p = r.heroes[0].pos;
        let mut relic = egg(None);
        relic.again_after = Some(10.0);
        r.place_object_at(relic, p, None, 10.0);
        for _ in 0..6 {
            r.ground_objects_tick(0.1);
        }
        assert!(r.ground_objects.is_empty() && r.objects_due.len() == 1);
        r.now += 10.0;
        r.ground_objects_tick(0.0);
        assert_eq!(r.ground_objects.len(), 1, "it dropped again");
    }
}
