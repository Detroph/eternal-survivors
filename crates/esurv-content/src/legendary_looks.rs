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
    /// Worn by each reflection while it spins.
    Reflections,
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

impl Moment {
    /// Every moment.
    pub const ALL: [Moment; 34] = [
        Moment::Burst,
        Moment::BurstMark,
        Moment::LeaveArea,
        Moment::StrikeOne,
        Moment::Barrage,
        Moment::LineHit,
        Moment::MarkAttackers,
        Moment::ShieldBurst,
        Moment::Splash,
        Moment::DotBurst,
        Moment::ExplodeOnDeath,
        Moment::PullEveryNth,
        Moment::FireTrail,
        Moment::Retaliate,
        Moment::ExtraTargets,
        Moment::StrikeAgain,
        Moment::Reflections,
        Moment::DotSpread,
        Moment::SpreadOnDeath,
        Moment::TimedBuff,
        Moment::Spree,
        Moment::WhileBuff,
        Moment::HealOnTrigger,
        Moment::PetOnTrigger,
        Moment::FearedTurn,
        Moment::GrowingArea,
        Moment::DotFromHeal,
        Moment::Stun,
        Moment::Fear,
        Moment::Silence,
        Moment::Vulnerable,
        Moment::Root,
        Moment::Slow,
        Moment::Knockback,
    ];
}

/// The moments of legendary `l`'s milestones (those an `Every` or a `WhileBuff` holds too) that
/// still have no look once the table is applied, each once, in [`Moment::ALL`]'s order. A burst
/// with no delay needs no mark (nothing waits to go off), a line with runners and an area with
/// spirits show them.
pub fn moments_without_looks(l: &LegendaryAmulet) -> Vec<Moment> {
    fn walk(ms: &mut [Milestone], moment: Moment) -> bool {
        ms.iter_mut().any(|m| {
            let none = match (&*m, moment) {
                (Milestone::Burst { delay, mark_spell: None, .. }, Moment::BurstMark) => *delay > 0.0,
                (Milestone::LineHit { runners: Some(_), .. }, Moment::LineHit) | (Milestone::LeaveArea { spirits: Some(_), .. }, Moment::LeaveArea) => false,
                _ => slot(m, moment).is_some_and(|s| s.is_none()),
            };
            let held = match m {
                Milestone::Every { then, .. } | Milestone::WhileBuff { then, .. } => walk(then, moment),
                _ => false,
            };
            none || held
        })
    }
    let mut ms: Vec<Milestone> = l
        .effects
        .iter()
        .filter_map(|e| match e {
            crate::TalentEffect::Milestone(m) => Some(m.clone()),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for moment in Moment::ALL {
        if !out.contains(&moment) && walk(&mut ms, moment) {
            out.push(moment);
        }
    }
    out
}

/// One legendary's looks: its main spell (`LegendaryAmulet::main_spell`) and the look of each of
/// its moments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegendaryLook {
    pub legendary: String,
    pub moments: Vec<(Moment, u32)>,
    /// Looks among its moments' drawn smaller, at [`SMALL_LOOK_SCALE`] (a smaller
    /// version of the spell's own look on each further enemy). Each is a rank of the spell no
    /// hero casts, so the spell's own look keeps its size.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub small: Vec<u32>,
    /// Looks among its moments' that pulse: on an enemy, played again every
    /// [`PULSE_EVERY`] seconds while the effect lasts, instead of worn as an aura.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pulsing: Vec<u32>,
}

/// How big a `small` look is drawn.
pub const SMALL_LOOK_SCALE: f32 = 0.6;
/// How often a pulsing look plays again (seconds).
pub const PULSE_EVERY: f32 = 2.0;

impl crate::Content {
    /// Whether `spell` is a look `legendary_looks.ron` gives a legendary's moment.
    pub fn is_legendary_look(&self, spell: u32) -> bool {
        spell != 0 && self.legendary_looks.iter().any(|l| l.moments.iter().any(|(_, s)| *s == spell))
    }

    /// Every look a legendary draws smaller, each once.
    pub fn small_looks(&self) -> Vec<u32> {
        let mut out: Vec<u32> = self.legendary_looks.iter().flat_map(|l| l.small.iter().copied()).collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Whether `spell` is a look a legendary plays as a pulse.
    pub fn is_pulsing_look(&self, spell: u32) -> bool {
        spell != 0 && self.legendary_looks.iter().any(|l| l.pulsing.contains(&spell))
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
        | (Milestone::Reflections { look_spell, .. }, M::Reflections)
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
        let looks = [LegendaryLook { legendary: "slam".into(), moments: vec![(Moment::Stun, 11020)], small: vec![], pulsing: vec![] }, LegendaryLook { legendary: "nothing".into(), moments: vec![], small: vec![], pulsing: vec![] }];
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

#[cfg(test)]
mod every_moment_has_a_look {
    use super::*;

    /// The moments allowed to show no look of their own, by legendary (its main spell): Gruul's
    /// Shatter's burst waits on enemies turned to stone, which is their mark.
    const ALLOWED_WITHOUT: &[(&str, Moment)] = &[("slam", Moment::BurstMark)];

    /// Every moment of every legendary's milestones (those an `Every` or a `WhileBuff` holds
    /// too) has a look once the table is applied, so none goes off unseen; its small and pulsing
    /// looks are looks of its own moments.
    #[test]
    fn every_moment_of_every_legendary_has_a_look() {
        let c = crate::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let mut missing = Vec::new();
        for l in &c.legendary_amulets {
            for m in moments_without_looks(l) {
                if !ALLOWED_WITHOUT.contains(&(l.main_spell(), m)) {
                    missing.push(format!("{} ({}): {m:?}", l.name, l.main_spell()));
                }
            }
        }
        assert!(missing.is_empty(), "moments with no look:\n{}", missing.join("\n"));
        for look in &c.legendary_looks {
            for s in look.small.iter().chain(&look.pulsing) {
                assert!(look.moments.iter().any(|(_, x)| x == s), "{}: {s} is no look of its moments", look.legendary);
            }
        }
    }
}
