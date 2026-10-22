//! A hero in a form who casts a caster spell (Rejuvenation, Regrowth from Cat or Bear Form) or
//! shifts on purpose (Powershift) leaves the form for a moment and shifts straight back
//! (docs/game/server.md, "The starting options"). The sim says so with
//! `Event::FormLeft`; every client sees the hero drop to its own model for the cast (the update
//! goes before the cast's `SMSG_SPELL_GO`, so the cast plays on the caster's model), then take
//! the form's model again with the form's own shapeshift look; both shifts show the form's puff of
//! smoke (its look's impact, Druid Morph, played on the hero as the look's target).

use std::collections::HashMap;

use esurv_sim::game::run::{Event, Unit};

use super::{starting_looks, Game};
use crate::packets::{self, SpellTarget};
use crate::update::{field, Fields, ObjectUpdate};
use crate::wire::Packet;

impl Game {
    /// The models of hero `hero`: its guid, its own model (its race's) and its form's, and the
    /// form's look (`Spell.dbc` id); `None` for a hero without a form, or without the game data.
    fn form_models(&self, hero: u32) -> Option<(u64, u32, u32, u32)> {
        let h = self.run.heroes.iter().find(|h| h.id == hero)?;
        let content = &self.run.content;
        let form = content.classes.get(h.class)?.starting_option(Some(&h.start))?.form.as_ref()?;
        let saved = &self.players.iter().find(|p| p.hero == hero)?.saved;
        let race = content.races.get(h.race)?;
        let own = self.data.as_ref()?.player_display(race.wow_race_id, saved.gender)?;
        let shaped = starting_looks::form_display(content, h)?;
        Some((self.guid_of(Unit::Hero(hero))?, own, shaped, form.look_spell))
    }

    /// This tick's form shifts: a hero leaving its form shows its own model at once
    /// (`objects`); one whose moment out has run takes the form's model again (`objects`), and
    /// the form's shapeshift look, which goes out after the update (returned). Both shifts
    /// puff: the form's look with the hero as its target plays its impact on the hero.
    pub(super) fn shift_forms(&mut self, events: &[Event], objects: &mut Vec<ObjectUpdate>) -> Vec<Packet> {
        let display = |guid: u64, d: u32| {
            let mut f = Fields::new();
            f.u32(field::UNIT_DISPLAYID, d);
            ObjectUpdate::Values { guid, fields: f }
        };
        let models: HashMap<u32, (u64, u32, u32, u32)> = self
            .run
            .heroes
            .iter()
            .filter_map(|h| Some((h.id, self.form_models(h.id)?)))
            .collect();
        let (left, back) = form_shift_times(&mut self.out_of_form, events, self.run.now, |h| models.contains_key(&h));
        let mut looks = Vec::new();
        let puff = |guid: u64, look: u32| packets::spell_go(guid, look, &[guid], SpellTarget::Unit(guid), None);
        for hero in left {
            let (guid, own, _, look) = models[&hero];
            objects.push(display(guid, own));
            looks.push(puff(guid, look));
        }
        for hero in back {
            if let Some(&(guid, _, shaped, look)) = models.get(&hero) {
                objects.push(display(guid, shaped));
                looks.push(puff(guid, look));
            }
        }
        looks
    }
}

/// The heroes that leave their form this tick (a `FormLeft` while in it, for a hero
/// `can_shift`) and those whose moment out has run by `now` and shift back; `out` keeps when each
/// hero out of its form shifts back (a cast while out keeps it out until the last one's moment
/// has run).
fn form_shift_times(
    out: &mut HashMap<u32, f32>,
    events: &[Event],
    now: f32,
    can_shift: impl Fn(u32) -> bool,
) -> (Vec<u32>, Vec<u32>) {
    let mut left = Vec::new();
    for e in events {
        let Event::FormLeft { hero, seconds, .. } = e else {
            continue;
        };
        let back_at = now + seconds;
        match out.get_mut(hero) {
            Some(at) => *at = at.max(back_at),
            None if can_shift(*hero) => {
                out.insert(*hero, back_at);
                left.push(*hero);
            }
            None => {}
        }
    }
    let mut back: Vec<u32> = out.iter().filter(|(_, at)| **at <= now).map(|(h, _)| *h).collect();
    back.sort_unstable();
    for h in &back {
        out.remove(h);
    }
    (left, back)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn left(hero: u32, seconds: f32) -> Event {
        Event::FormLeft { hero, seconds, look_spell: 768 }
    }

    #[test]
    fn a_hero_leaves_its_form_once_and_shifts_back_when_the_last_cast_s_moment_has_run() {
        let mut out = HashMap::new();
        let yes = |_| true;
        assert_eq!(form_shift_times(&mut out, &[left(1, 1.0)], 10.0, yes), (vec![1], vec![]));
        // a second cast while out: no new shift, and it stays out a moment more
        assert_eq!(form_shift_times(&mut out, &[left(1, 1.0)], 10.5, yes), (vec![], vec![]));
        assert_eq!(form_shift_times(&mut out, &[], 11.2, yes), (vec![], vec![]));
        assert_eq!(form_shift_times(&mut out, &[], 11.5, yes), (vec![], vec![1]));
        assert!(out.is_empty());
        // a hero without a form (or without the game data to draw it) never leaves one
        assert_eq!(form_shift_times(&mut out, &[left(2, 1.0)], 12.0, |_| false), (vec![], vec![]));
        assert!(out.is_empty());
    }
}
