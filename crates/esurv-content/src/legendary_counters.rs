//! The legendaries' counter buffs (docs/game/screens.md, "The stock frames in a run"):
//! a legendary amulet whose milestone goes off every n-th cast or hit (Stampede's every 9th Kill
//! Command) shows a buff in the hero's buff frame from its first count, in the legendary's own
//! icon, its stacks the count toward the next time (0 to n - 1), back to 0 as it goes off. A
//! counter kept on each enemy apart (`per_enemy`) has none: no one number tells it.
//!
//! Each such legendary's buff is an aura of its own among [`COUNTER_AURAS`], in the legendaries'
//! order: old `Spell.dbc` entries with no look of their own (the 1.12 client's unused
//! "zzOLD ... Damage Bonus" rows), so only the icon shows, the legendary's in their place.

use crate::{Content, LegendaryAmulet, Milestone, TalentEffect};

/// The auras the counter buffs wear, one a counting legendary.
pub const COUNTER_AURAS: [u32; 20] = [
    2186, 2187, 2188, 2189, 2190, 2191, 2192, 2193, 2194, 2195, 2196, 2197, 2198, 2199, 2200, 2201, 2202, 2203, 2204, 2205,
];

/// How many of its events milestone `m` counts before it goes off, when that takes more than one
/// and one number tells it (not kept on each enemy apart).
pub fn counted_every(m: &Milestone) -> Option<u32> {
    let n = match m {
        Milestone::Every { count, per_enemy: false, .. } => *count,
        Milestone::LineHit { every, .. } => *every,
        Milestone::EveryNth { count, .. } | Milestone::MoreSummonsEveryNth { count, .. } | Milestone::PullEveryNth { count, .. } => *count,
        _ => return None,
    };
    (n > 1).then_some(n)
}

/// How many events legendary `l`'s first counting milestone counts, if it has one.
pub fn legendary_counts(l: &LegendaryAmulet) -> Option<u32> {
    l.effects.iter().find_map(|e| match e {
        TalentEffect::Milestone(m) => counted_every(m),
        _ => None,
    })
}

/// One legendary's counter buff: the legendary (its origin id, `legendary:<main spell>`), the aura
/// it wears and its icon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterBuff {
    pub legendary: String,
    pub aura: u32,
    pub icon: String,
}

impl Content {
    /// Every counting legendary's counter buff, in the legendaries' order (past
    /// [`COUNTER_AURAS`]' count, none).
    pub fn counter_buffs(&self) -> Vec<CounterBuff> {
        self.legendary_amulets
            .iter()
            .filter(|l| legendary_counts(l).is_some())
            .zip(COUNTER_AURAS)
            .map(|(l, aura)| CounterBuff { legendary: format!("legendary:{}", l.main_spell()), aura, icon: l.icon.clone() })
            .collect()
    }

    /// The counter buff of legendary `id` (`legendary:<main spell>`), if it counts.
    pub fn counter_buff(&self, id: &str) -> Option<CounterBuff> {
        self.counter_buffs().into_iter().find(|c| c.legendary == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The counting legendaries, each with an aura of its own (there is room for all) and its own
    /// icon; those that count on each enemy apart have none.
    #[test]
    fn every_counting_legendary_has_a_counter_buff_of_its_own() {
        let c = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let counting = c.legendary_amulets.iter().filter(|l| legendary_counts(l).is_some()).count();
        let buffs = c.counter_buffs();
        assert!(counting <= COUNTER_AURAS.len(), "{counting} counting legendaries, room for {}", COUNTER_AURAS.len());
        assert_eq!(buffs.len(), counting);
        let stampede = c.counter_buff("legendary:kill_command").expect("Stampede counts its commands");
        assert_eq!(stampede.icon, "Ability_Druid_ChallangingRoar");
        let l = c.legendary_amulets.iter().find(|l| l.main_spell() == "kill_command").unwrap();
        assert_eq!(legendary_counts(l), Some(9));
        assert!(c.counter_buff("legendary:frost_trap").is_none(), "Flash Freeze counts on each enemy apart");
        assert!(c.counter_buff("legendary:aimed_shot").is_none(), "every shot: nothing to count");
        let mut auras: Vec<u32> = buffs.iter().map(|b| b.aura).collect();
        auras.dedup();
        assert_eq!(auras.len(), buffs.len());
    }
}
