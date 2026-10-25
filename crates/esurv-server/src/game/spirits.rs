//! Spirits running with a hero's lasting area (docs/game/screens.md, "Looks"; Sprint of the
//! Wild's wild spirits): when a `LeaveArea` with `spirits` starts, the simulation says
//! so (`Event::SpiritsAppeared`) and the server puts that many see-through creatures in the druid
//! form's look of the hero's faction (Ghost Visual, `starting_looks::GHOST_LOOK`), drawn smaller,
//! beside the hero (left, right, then further out), and keeps them running at its side
//! (`SMSG_MONSTER_MOVE`) while the area lasts. As the area strikes (`Event::SpiritsStrike`),
//! each spirit plays its strike on one of the enemies struck. They go when the area ends. A
//! spirit is a unit as a bunny is (entry `SPIRIT`): no name, unselectable, unattackable, no
//! enemy of the simulation; the damage stays the area's.

use esurv_content::AreaSpirits;
use esurv_sim::game::enemy_ai::{Segment, Vec2};
use esurv_sim::game::run::Unit;

use super::bunnies::still_unit_create;
use super::starting_looks::GHOST_LOOK;
use super::{unit_guid, Game};
use crate::packets::{self, SpellTarget};
use crate::update::{field, update_object, ObjectUpdate, Speeds};
use crate::wire::Packet;

/// How far beside the hero a spirit runs, the first pair (yards); each further pair this much
/// further out.
pub const SPIRIT_SIDE: f32 = 2.0;
/// A spirit runs again once its place by the hero has moved this far (yards).
const SPIRIT_FOLLOW_SLACK: f32 = 0.5;

/// One area's spirits on the field.
#[derive(Debug, Clone)]
pub(super) struct SpiritsShown {
    pub area: u32,
    pub hero: u32,
    pub until: f32,
    pub strike_look: u32,
    /// Each spirit's guid and where it was last sent.
    pub spirits: Vec<(u64, Vec2)>,
}

/// Where spirit `i` of a hero at `at` facing `facing` runs: left, right, then the next pair
/// [`SPIRIT_SIDE`] further out.
pub fn spirit_place(at: Vec2, facing: f32, i: usize) -> Vec2 {
    let side = if i % 2 == 0 { 1.0 } else { -1.0 };
    let reach = SPIRIT_SIDE * (1 + i / 2) as f32;
    let left = (facing + std::f32::consts::FRAC_PI_2, side);
    (at.0 + left.0.cos() * reach * left.1, at.1 + left.0.sin() * reach * left.1)
}

impl Game {
    /// Area `area`'s spirits appear beside hero `hero`, until `until`.
    pub(super) fn send_spirits(&mut self, area: u32, hero: u32, until: f32, spirits: &AreaSpirits, all: &mut Vec<Packet>) {
        let Some(h) = self.run.heroes.iter().find(|h| h.id == hero) else { return };
        let content = &self.run.content;
        let display = super::player_enemies::form_display(spirits.form, content.races[h.race].faction);
        let (at, facing, speed) = (h.pos, h.facing, h.stats.run_speed());
        let mut creates = Vec::new();
        let mut shown = Vec::new();
        for i in 0..spirits.count as usize {
            self.next_bunny = self.next_bunny.wrapping_add(1) & 0xFF_FFFF;
            let guid = unit_guid(crate::entries::SPIRIT, self.next_bunny);
            let spot = spirit_place(at, facing, i);
            let mut create = still_unit_create(guid, crate::entries::SPIRIT, display, spirits.scale, [spot.0, spot.1, self.ground_z(spot)]);
            if let ObjectUpdate::Create { living, fields, .. } = &mut create {
                living.facing = facing;
                living.speeds = Speeds::running_at(speed);
                // see-through, as a hero's ghostly copy is
                fields.u32(field::UNIT_AURA, GHOST_LOOK).u32(field::UNIT_AURAFLAGS, 0x2).u32(field::UNIT_AURALEVELS, 1);
            }
            creates.push(create);
            shown.push((guid, spot));
        }
        if !creates.is_empty() {
            all.push(update_object(&creates));
        }
        self.spirits.push(SpiritsShown { area, hero, until, strike_look: spirits.strike_look, spirits: shown });
    }

    /// Area `area`'s spirits each strike one of `targets`.
    pub(super) fn spirits_strike(&mut self, area: u32, targets: &[u32], all: &mut Vec<Packet>) {
        let Some(s) = self.spirits.iter().find(|s| s.area == area) else { return };
        for (&(guid, _), &t) in s.spirits.iter().zip(targets) {
            if let Some(victim) = self.guid_of(Unit::Enemy(t)) {
                all.push(packets::spell_go(guid, s.strike_look, &[victim], SpellTarget::Unit(victim), None));
            }
        }
    }

    /// The spirits keep at their hero's side; those whose area ended, or whose hero is gone, go.
    pub(super) fn mirror_spirits(&mut self, out: &mut Vec<Packet>) {
        let now = self.run.now;
        let mut kept = Vec::new();
        for mut s in std::mem::take(&mut self.spirits) {
            let hero = self.run.heroes.iter().find(|h| h.id == s.hero && h.alive).map(|h| (h.pos, h.facing, h.stats.run_speed()));
            let Some((at, facing, speed)) = hero.filter(|_| now < s.until) else {
                out.extend(s.spirits.iter().map(|(guid, _)| packets::destroy_object(*guid)));
                continue;
            };
            for (i, (guid, last)) in s.spirits.iter_mut().enumerate() {
                let place = spirit_place(at, facing, i);
                let d = ((place.0 - last.0).powi(2) + (place.1 - last.1).powi(2)).sqrt();
                if d >= SPIRIT_FOLLOW_SLACK {
                    let segment = Segment { from: *last, to: place, start: now, duration: d / speed.max(0.1) };
                    out.push(self.walk(*guid, &segment));
                    *last = place;
                }
            }
            kept.push(s);
        }
        self.spirits = kept;
    }

    /// Every spirit goes (the field was cleared).
    pub(super) fn forget_spirits(&mut self) -> Vec<Packet> {
        self.spirits.drain(..).flat_map(|s| s.spirits.into_iter().map(|(guid, _)| packets::destroy_object(guid))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first spirit runs on the hero's left, the second on its right, 2 yd out; a third and
    /// fourth twice as far.
    #[test]
    fn spirits_run_left_and_right_of_the_hero() {
        let l = spirit_place((0.0, 0.0), 0.0, 0);
        let r = spirit_place((0.0, 0.0), 0.0, 1);
        assert!(l.0.abs() < 1e-4 && (l.1 - SPIRIT_SIDE).abs() < 1e-4, "{l:?}");
        assert!(r.0.abs() < 1e-4 && (r.1 + SPIRIT_SIDE).abs() < 1e-4, "{r:?}");
        let far = spirit_place((0.0, 0.0), 0.0, 2);
        assert!((far.1 - 2.0 * SPIRIT_SIDE).abs() < 1e-4);
        let turned = spirit_place((5.0, 5.0), std::f32::consts::FRAC_PI_2, 0);
        assert!((turned.0 - (5.0 - SPIRIT_SIDE)).abs() < 1e-4 && (turned.1 - 5.0).abs() < 1e-4, "{turned:?}");
    }
}
