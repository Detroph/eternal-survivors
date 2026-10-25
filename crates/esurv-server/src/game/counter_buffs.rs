//! The legendaries' counter buffs (`esurv_content::legendary_counters`; docs/game/screens.md,
//! "The stock frames in a run"): a worn legendary that goes off every n-th cast or hit
//! shows a buff on its hero from the first count, its stacks the count toward the next time, as
//! an aura of its own after the hero's other buffs; the buff frame's tooltip names the legendary
//! (`BuffKind::Passive` of `legendary:<main spell>`). A look only: no number changes.

use esurv_protocol as protocol;

use super::present::Aura;
use super::Game;

impl Game {
    /// Notes the counters that have counted once (their buff shows from then on, through the
    /// run).
    pub(super) fn note_counters(&mut self) {
        for h in 0..self.run.heroes.len() {
            let hero = self.run.heroes[h].id;
            for (id, count, _) in self.run.legendary_counters(h) {
                if count > 0 {
                    self.counters_seen.insert((hero, id));
                }
            }
        }
    }

    /// The counters of hero `hero` that show: their legendary, aura, count and n.
    fn counters_shown(&self, hero: u32) -> Vec<(String, u32, u32, u32)> {
        let Some(h) = self.run.heroes.iter().position(|x| x.id == hero) else { return Vec::new() };
        self.run
            .legendary_counters(h)
            .into_iter()
            .filter(|(id, ..)| self.counters_seen.contains(&(hero, id.clone())))
            .filter_map(|(id, count, every)| Some((id.clone(), self.run.content.counter_buff(&id)?.aura, count, every)))
            .collect()
    }

    /// Hero `hero`'s counter buffs as auras: stacks the count, no end.
    pub(super) fn counter_auras(&self, hero: u32) -> Vec<Aura> {
        self.counters_shown(hero).into_iter().map(|(_, spell, count, _)| Aura { spell, stacks: count, until: 0.0 }).collect()
    }

    /// Hero `hero`'s counter buffs as its buff frame's tooltips tell them: the legendary, the
    /// count and n.
    pub(super) fn counter_tips(&self, hero: u32) -> Vec<protocol::BuffTip> {
        self.counters_shown(hero)
            .into_iter()
            .map(|(id, look, count, every)| protocol::BuffTip { look, kind: protocol::BuffKind::Passive, spell: id, value: every as f32, stacks: count })
            .collect()
    }
}
