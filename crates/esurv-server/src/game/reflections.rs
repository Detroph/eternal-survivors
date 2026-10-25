//! A hero's reflection (docs/game/screens.md, "Looks"; Blink Strike): where a
//! `StrikeOne` with `reflection` strikes another enemy, the simulation says so
//! (`Event::ReflectionStrikes`) and the server shows a see-through copy of the hero, as a clone
//! is shown (the Ghost Visual aura, `starting_looks::GHOST_LOOK`), beside that enemy facing it; the
//! copy plays the strike on it and goes a moment later. The hero stays where it is; the damage
//! stays the milestone's.

use esurv_sim::game::enemy_ai::Vec2;
use esurv_sim::game::run::Unit;

use super::starting_looks::{clone_guid, CloneShown};
use super::Game;
use crate::packets::{self, SpellTarget};
use crate::update::{update_object, ObjectUpdate};
use crate::wire::Packet;

/// How long a reflection stays after it strikes (seconds).
pub const REFLECTION_STAYS: f32 = 1.0;
/// How far from the struck enemy it appears, on the hero's side (yards).
pub const REFLECTION_STANDS_OFF: f32 = 1.5;
/// The reflections' clone numbers start here, past any summon's of a run.
const REFLECTIONS_FROM: u32 = 0x3000;

/// Where a reflection stands to strike an enemy at `target` for a hero at `hero`, and its facing:
/// [`REFLECTION_STANDS_OFF`] from the enemy toward the hero, turned to the enemy.
pub fn reflection_spot(hero: Vec2, target: Vec2) -> (Vec2, f32) {
    let (dx, dy) = (target.0 - hero.0, target.1 - hero.1);
    let d = (dx * dx + dy * dy).sqrt();
    if d < 0.01 {
        return (target, 0.0);
    }
    let back = REFLECTION_STANDS_OFF.min(d);
    ((target.0 - dx / d * back, target.1 - dy / d * back), dy.atan2(dx))
}

impl Game {
    /// Hero `hero`'s reflection appears at enemy `target` and strikes it with `spell`; it goes
    /// [`REFLECTION_STAYS`] later ([`Game::mirror_reflections`]).
    pub(super) fn send_reflection(&mut self, hero: u32, target: u32, spell: u32, all: &mut Vec<Packet>) {
        let now = self.run.now;
        let (Some(h), Some(e)) = (self.run.heroes.iter().find(|h| h.id == hero), self.run.enemies.iter().find(|e| e.id == target)) else {
            return;
        };
        let (at, facing) = reflection_spot(h.pos, e.position(now));
        let Some(victim) = self.guid_of(Unit::Enemy(target)) else { return };
        let save = self.players.iter().find(|p| p.hero == hero).map_or(0, |p| p.saved.guid);
        self.next_reflection = self.next_reflection.wrapping_add(1);
        let guid = clone_guid(save, REFLECTIONS_FROM + (self.next_reflection & 0xFFF));
        let Some(mut create) = self.clone_object(&CloneShown { hero, at, guid, created: true }) else { return };
        if let ObjectUpdate::Create { living, .. } = &mut create {
            living.facing = facing;
        }
        all.push(update_object(&[create]));
        if spell != 0 {
            all.push(packets::spell_go(guid, spell, &[victim], SpellTarget::Unit(victim), None));
        }
        self.reflections.push((guid, now + REFLECTION_STAYS));
    }

    /// Reflections whose time is up go.
    pub(super) fn mirror_reflections(&mut self, out: &mut Vec<Packet>) {
        let now = self.run.now;
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.reflections).into_iter().partition(|(_, until)| now >= *until);
        self.reflections = kept;
        out.extend(gone.into_iter().map(|(guid, _)| packets::destroy_object(guid)));
    }

    /// Every reflection goes (the field was cleared).
    pub(super) fn forget_reflections(&mut self) -> Vec<Packet> {
        self.reflections.drain(..).map(|(guid, _)| packets::destroy_object(guid)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reflection stands between the hero and the enemy, 1.5 yd off the enemy, facing it; one
    /// right beside the hero stands on the hero's side.
    #[test]
    fn a_reflection_stands_beside_the_enemy_facing_it() {
        let (at, facing) = reflection_spot((0.0, 0.0), (10.0, 0.0));
        assert!((at.0 - 8.5).abs() < 1e-4 && at.1.abs() < 1e-4);
        assert!(facing.abs() < 1e-4, "facing the enemy");
        let (at, facing) = reflection_spot((0.0, 0.0), (0.0, 5.0));
        assert!(at.0.abs() < 1e-4 && (at.1 - 3.5).abs() < 1e-4);
        assert!((facing - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        let (at, _) = reflection_spot((0.0, 0.0), (1.0, 0.0));
        assert!(at.0.abs() < 1e-4, "never past the hero");
    }
}
