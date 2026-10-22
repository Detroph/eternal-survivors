//! The legendary amulets' looks (`legendary_looks.ron`): each
//! legendary's signature moments wear a 1.12 spell look of their own, the boss's or hero's own
//! look where the player's client has it. A legendary names the moments of its milestones by kind
//! (a burst, a lasting area, a stun on an enemy, a spree...) and the `Spell.dbc` id each one shows;
//! as the content loads, each moment's look goes into the matching milestones of that legendary
//! (in an `Every` or a `WhileBuff` too), over any look `legendary_amulets.ron` gives them.
//! docs/game/screens.md "Looks" says how each kind shows.

use serde::{Deserialize, Serialize};

use crate::{AfflictEffect, LegendaryAmulet, Milestone};

/// A legendary's moment: a milestone kind, or for `Afflict` the effect it puts on an enemy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Moment {
    /// A burst's look at its spot.
    Burst,
    /// A delayed burst's mark, worn by the enemy it follows until it goes off.
    BurstMark,
    /// A lasting area's look on the ground.
    LeaveArea,
    StrikeOne,
    Barrage,
    LineHit,
    MarkAttackers,
    ShieldBurst,
    Splash,
    DotBurst,
    ExplodeOnDeath,
    PullEveryNth,
    FireTrail,
    Retaliate,
    /// On each enemy a sweep or a jump strikes.
    ExtraTargets,
    StrikeAgain,
    DotSpread,
    SpreadOnDeath,
    /// Worn by the hero while the stacks last.
    TimedBuff,
    /// Worn by the hero while the spree lasts.
    Spree,
    /// Worn by the hero while the buff is up.
    WhileBuff,
    /// On the hero as it heals.
    HealOnTrigger,
    /// At the spot each summon appears.
    PetOnTrigger,
    /// Worn by a turned enemy.
    FearedTurn,
    /// A growing area's look on the ground.
    GrowingArea,
    /// Worn by each enemy a heal's effect over time is on.
    DotFromHeal,
    /// `Afflict` effects, worn by the enemy while they last (a knockback plays on it).
    Stun,
    Fear,
    Silence,
    Vulnerable,
    Root,
    Slow,
    Knockback,
}

/// One legendary's looks: its main spell (`LegendaryAmulet::main_spell`) and the look of each of
/// its moments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegendaryLook {
    pub legendary: String,
    pub moments: Vec<(Moment, u32)>,
}

impl crate::Content {
    /// Whether `spell` is a look `legendary_looks.ron` gives a legendary's moment.
    pub fn is_legendary_look(&self, spell: u32) -> bool {
        spell != 0 && self.legendary_looks.iter().any(|l| l.moments.iter().any(|(_, s)| *s == spell))
    }
}

/// The look slot milestone `m` has for `moment`, if it is that moment.
fn slot(m: &mut Milestone, moment: Moment) -> Option<&mut Option<u32>> {
    use Moment as M;
    Some(match (m, moment) {
        (Milestone::Burst { look_spell, .. }, M::Burst)
        | (Milestone::Burst { mark_spell: look_spell, .. }, M::BurstMark)
        | (Milestone::LeaveArea { look_spell, .. }, M::LeaveArea)
        | (Milestone::StrikeOne { look_spell, .. }, M::StrikeOne)
        | (Milestone::Barrage { look_spell, .. }, M::Barrage)
        | (Milestone::LineHit { look_spell, .. }, M::LineHit)
        | (Milestone::MarkAttackers { look_spell, .. }, M::MarkAttackers)
        | (Milestone::ShieldBurst { look_spell, .. }, M::ShieldBurst)
        | (Milestone::Splash { look_spell, .. }, M::Splash)
        | (Milestone::DotBurst { look_spell, .. }, M::DotBurst)
        | (Milestone::ExplodeOnDeath { look_spell, .. }, M::ExplodeOnDeath)
        | (Milestone::PullEveryNth { look_spell, .. }, M::PullEveryNth)
        | (Milestone::FireTrail { look_spell, .. }, M::FireTrail)
        | (Milestone::Retaliate { look_spell, .. }, M::Retaliate)
        | (Milestone::ExtraTargets { look_spell, .. }, M::ExtraTargets)
        | (Milestone::StrikeAgain { look_spell, .. }, M::StrikeAgain)
        | (Milestone::DotSpread { look_spell, .. }, M::DotSpread)
        | (Milestone::SpreadOnDeath { look_spell, .. }, M::SpreadOnDeath)
        | (Milestone::TimedBuff { look_spell, .. }, M::TimedBuff)
        | (Milestone::Spree { look_spell, .. }, M::Spree)
        | (Milestone::WhileBuff { look_spell, .. }, M::WhileBuff)
        | (Milestone::HealOnTrigger { look_spell, .. }, M::HealOnTrigger)
        | (Milestone::PetOnTrigger { look_spell, .. }, M::PetOnTrigger)
        | (Milestone::FearedTurn { look_spell, .. }, M::FearedTurn)
        | (Milestone::GrowingArea { look_spell, .. }, M::GrowingArea)
        | (Milestone::DotFromHeal { look_spell, .. }, M::DotFromHeal) => look_spell,
        (Milestone::Afflict { effect, look_spell, .. }, moment) if afflict_moment(effect) == Some(moment) => look_spell,
        _ => return None,
    })
}

/// The moment an `Afflict` of `effect` is.
fn afflict_moment(effect: &AfflictEffect) -> Option<Moment> {
    Some(match effect {
        AfflictEffect::Stun { .. } => Moment::Stun,
        AfflictEffect::Fear { .. } => Moment::Fear,
        AfflictEffect::Silence { .. } => Moment::Silence,
        AfflictEffect::Vulnerable { .. } => Moment::Vulnerable,
        AfflictEffect::Root { .. } => Moment::Root,
        AfflictEffect::Slow { .. } => Moment::Slow,
        AfflictEffect::Knockback { .. } => Moment::Knockback,
        AfflictEffect::HealFromDamage { .. } | AfflictEffect::HealMaxHealth { .. } => return None,
    })
}

/// Puts `look` into every milestone of `ms` (and those they hold) that is `moment`; how many.
fn put(ms: &mut [Milestone], moment: Moment, look: u32) -> usize {
    let mut n = 0;
    for m in ms {
        if let Some(s) = slot(m, moment) {
            *s = Some(look);
            n += 1;
        }
        if let Milestone::Every { then, .. } | Milestone::WhileBuff { then, .. } = m {
            n += put(then, moment, look);
        }
    }
    n
}

/// How many milestones of legendary `l` are `moment` (those an `Every` or a `WhileBuff` holds
/// counted too).
pub fn moment_count(l: &LegendaryAmulet, moment: Moment) -> usize {
    let mut ms: Vec<Milestone> = l
        .effects
        .iter()
        .filter_map(|e| match e {
            crate::TalentEffect::Milestone(m) => Some(m.clone()),
            _ => None,
        })
        .collect();
    put(&mut ms, moment, 0)
}

/// Puts each look of `looks` into its legendary's milestones; the problems found (a legendary
/// that is not there).
pub fn apply(amulets: &mut [LegendaryAmulet], looks: &[LegendaryLook]) -> Vec<String> {
    let mut problems = Vec::new();
    for look in looks {
        let Some(l) = amulets.iter_mut().find(|l| l.main_spell() == look.legendary) else {
            problems.push(format!("legendary_looks.ron names legendary {}, which is not in legendary_amulets.ron", look.legendary));
            continue;
        };
        for &(moment, spell) in &look.moments {
            for e in l.effects.iter_mut() {
                if let crate::TalentEffect::Milestone(m) = e {
                    put(std::slice::from_mut(m), moment, spell);
                }
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Content, SpellFilter, Trigger};

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    #[test]
    fn a_moment_s_look_goes_into_the_milestones_of_its_kind_held_ones_too() {
        let stun = Milestone::Afflict { chance: 1.0, effect: AfflictEffect::Stun { seconds: 1.0 }, trigger: Trigger::Hit, filter: SpellFilter::Any, look_spell: None };
        let slow = Milestone::Afflict { chance: 1.0, effect: AfflictEffect::Slow { amount: 0.5, seconds: 1.0 }, trigger: Trigger::Hit, filter: SpellFilter::Any, look_spell: None };
        let every = Milestone::Every { count: 2, per: crate::Counted::Casts, per_enemy: false, chance: 1.0, first_only: false, filter: SpellFilter::Any, then: vec![stun.clone()] };
        let mut l = LegendaryAmulet {
            spells: vec!["slam".into()],
            name: "x".into(),
            icon: "x".into(),
            text: "x".into(),
            effects: vec![crate::TalentEffect::Milestone(every), crate::TalentEffect::Milestone(slow)],
            rarity: crate::Rarity::Legendary,
            growth: crate::Growth::Slowest,
        };
        assert_eq!((moment_count(&l, Moment::Stun), moment_count(&l, Moment::Slow), moment_count(&l, Moment::Burst)), (1, 1, 0));
        let looks = [LegendaryLook { legendary: "slam".into(), moments: vec![(Moment::Stun, 11020)] }, LegendaryLook { legendary: "nothing".into(), moments: vec![] }];
        let problems = apply(std::slice::from_mut(&mut l), &looks);
        assert_eq!(problems.len(), 1, "an unknown legendary is a problem");
        let crate::TalentEffect::Milestone(Milestone::Every { then, .. }) = &l.effects[0] else { panic!() };
        assert!(matches!(then[0], Milestone::Afflict { look_spell: Some(11020), .. }));
        assert!(matches!(l.effects[1], crate::TalentEffect::Milestone(Milestone::Afflict { look_spell: None, .. })), "only the stun");
    }

    #[test]
    fn every_legendary_the_table_names_is_there_and_has_each_moment_it_names() {
        let c = content();
        assert!(!c.legendary_looks.is_empty());
        for look in &c.legendary_looks {
            let l = c.legendary_amulets.iter().find(|l| l.main_spell() == look.legendary);
            let Some(l) = l else { panic!("legendary_looks.ron names legendary {}, which is not there", look.legendary) };
            for (moment, spell) in &look.moments {
                assert!(*spell > 0, "{}: {moment:?} names no look", look.legendary);
                assert!(moment_count(l, *moment) > 0, "{}: no {moment:?} moment among its milestones", look.legendary);
            }
            let mut seen = Vec::new();
            for (moment, _) in &look.moments {
                assert!(!seen.contains(moment), "{}: {moment:?} named twice", look.legendary);
                seen.push(*moment);
            }
        }
        let mut names: Vec<&str> = c.legendary_looks.iter().map(|l| l.legendary.as_str()).collect();
        names.sort_unstable();
        let n = names.len();
        names.dedup();
        assert_eq!(names.len(), n, "a legendary is named once");
    }

    #[test]
    fn the_loaded_legendaries_wear_their_table_looks() {
        let c = content();
        let gruul = c.legendary_amulets.iter().find(|l| l.main_spell() == "slam").unwrap();
        let look = c.legendary_looks.iter().find(|l| l.legendary == "slam").unwrap();
        let stun = look.moments.iter().find(|(m, _)| *m == Moment::Stun).unwrap().1;
        let held = gruul.effects.iter().filter_map(|e| match e {
            crate::TalentEffect::Milestone(Milestone::Every { then, .. }) => Some(then),
            _ => None,
        });
        assert!(held.flatten().any(|m| matches!(m, Milestone::Afflict { look_spell: Some(s), .. } if *s == stun)));
    }
}
