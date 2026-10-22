//! The channel beams the run shows (docs/game/screens.md, "Looks"): a
//! caster's channel fields (`UNIT_CHANNEL_SPELL`, `UNIT_CHANNEL_OBJECT`) set while it drains or
//! flays an enemy, so the client draws the 1.12 spell's own beam between them (its channel kit's
//! chain) and the caster holds its channel pose. The hero's Drain Soul on the enemy it drains,
//! and each Insanity tentacle's Mind Flay on the enemy it flays.
//!
//! The client draws a beam on the channel spell's rising edge and keeps its ends, so a beam that
//! moves to another enemy is cleared on one mirror and set again on the next.

use std::collections::HashMap;

use esurv_content::SummonKind;
use esurv_sim::game::run::Unit;

use super::Game;
use crate::update::{field, Fields, ObjectUpdate};

/// The spells whose effect over time the hero channels, by ability id: their looks' channel kits
/// draw the beam.
pub const CHANNELLED_DRAINS: [&str; 1] = ["drain_soul"];

/// A channel: its spell (`Spell.dbc` id) and the guid it is aimed at.
pub type Channel = (u32, u64);

/// What to tell the clients so each caster shows `wanted` (absent: no channel), given what they
/// were `shown`: the casters' new channels, `(0, 0)` to clear one. A channel moving to another
/// spell or target is only cleared now; it is set at the next call.
pub fn channel_changes(
    shown: &mut HashMap<u64, Channel>,
    wanted: &HashMap<u64, Channel>,
) -> Vec<(u64, Channel)> {
    let mut out = Vec::new();
    shown.retain(|caster, now| {
        if wanted.get(caster) == Some(now) {
            return true;
        }
        out.push((*caster, (0, 0)));
        false
    });
    for (caster, channel) in wanted {
        if !shown.contains_key(caster) && !out.iter().any(|(c, _)| c == caster) {
            out.push((*caster, *channel));
            shown.insert(*caster, *channel);
        }
    }
    out.sort_by_key(|(c, _)| *c);
    out
}

impl Game {
    /// The channels every caster should show now, by caster guid.
    fn channels_wanted(&self) -> HashMap<u64, Channel> {
        let content = &self.run.content;
        let mut wanted = HashMap::new();
        // the heroes' drains: the enemy each one's drain is on
        for (i, h) in self.run.heroes.iter().enumerate() {
            let Some(me) = self.guid_of(Unit::Hero(h.id)) else {
                continue;
            };
            for drain in CHANNELLED_DRAINS {
                let Some(look) = content.ability(drain).map(|a| a.look_spell) else {
                    continue;
                };
                let target = self
                    .run
                    .enemies
                    .iter()
                    .find(|e| e.dots.iter().any(|d| d.hero == i && d.spell == drain && d.ticks_left > 0))
                    .and_then(|e| self.guid_of(Unit::Enemy(e.id)));
                if let Some(t) = target {
                    wanted.insert(me, (look, t));
                    break;
                }
            }
        }
        // the eye tentacles' mind flays: the enemy each one flays (the claw claws, no beam)
        for p in &self.run.pets {
            let Some(s) = p.summon.as_ref().filter(|s| s.kind == SummonKind::EyeTentacle) else {
                continue;
            };
            let Some(look) = s.spells.first().and_then(|id| content.ability(id)).map(|a| a.look_spell) else {
                continue;
            };
            let target = p.target.filter(|t| self.run.enemies.iter().any(|e| e.id == *t));
            if let (Some(me), Some(t)) = (self.guid_of(Unit::Pet(p.id)), target.and_then(|t| self.guid_of(Unit::Enemy(t)))) {
                wanted.insert(me, (look, t));
            }
        }
        // the bosses' and their adds' beams (Fairbanks's heals,
        // Hakkar's Blood Siphon)
        for b in &self.run.extras.beams {
            if let (Some(me), Some(t)) = (self.guid_of(Unit::Enemy(b.caster)), self.guid_of(b.target)) {
                wanted.insert(me, (b.spell, t));
            }
        }
        wanted
    }

    /// The casters' channel fields that changed, for every client.
    pub(super) fn mirror_channels(&mut self, objects: &mut Vec<ObjectUpdate>) {
        let wanted = self.channels_wanted();
        for (guid, (spell, target)) in channel_changes(&mut self.channels_shown, &wanted) {
            let mut f = Fields::new();
            f.u64(field::UNIT_CHANNEL_OBJECT, target).u32(field::UNIT_CHANNEL_SPELL, spell);
            objects.push(ObjectUpdate::Values { guid, fields: f });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_beam_that_moves_is_cleared_first_then_set_again() {
        let mut shown = HashMap::new();
        let mut wanted = HashMap::from([(7, (1120, 100))]);
        assert_eq!(channel_changes(&mut shown, &wanted), [(7, (1120, 100))]);
        assert!(channel_changes(&mut shown, &wanted).is_empty(), "nothing new to tell");
        // the drain goes on to another enemy: cleared now, set at the next mirror
        wanted.insert(7, (1120, 101));
        assert_eq!(channel_changes(&mut shown, &wanted), [(7, (0, 0))]);
        assert_eq!(channel_changes(&mut shown, &wanted), [(7, (1120, 101))]);
        // it ends
        wanted.clear();
        assert_eq!(channel_changes(&mut shown, &wanted), [(7, (0, 0))]);
        assert!(shown.is_empty());
    }
}
