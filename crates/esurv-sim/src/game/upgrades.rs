//! Level-up cards for spells: which cards a level-up can show, and what an upgrade card does to a
//! spell you know.
//!
//! There is no maximum rank. Each upgrade kind has its own cap or falloff, and a kind is only
//! offered while one more step would still add at least `MIN_GAIN`.

use esurv_content::{Ability, Delivery, SpellEffect, Upgrade};

use super::offers::Rarity;

/// A hero can hold at most this many spells; past it, no new-spell cards.
pub const MAX_SPELLS: usize = 6;
/// Weight of a new-spell card while the hero knows fewer than 3 spells, and after.
pub const NEW_SPELL_WEIGHT: (f32, f32) = (1.6, 0.9);
/// A starting option's common card (the forms' Swipe) is offered as a
/// new spell this many times as often as the other cards of its pool.
pub const COMMON_CARD_WEIGHT_FACTOR: f32 = 2.0;
/// Weight of an upgrade card for a known spell.
pub const UPGRADE_WEIGHT: f32 = 1.3;
/// An upgrade kind is offered only while its next step adds at least this much
/// (checked at legendary strength).
pub const MIN_GAIN: f32 = 0.005;
/// A legendary upgrade card improves this many kinds at once; others one.
pub const LEGENDARY_KINDS: usize = 2;
/// Every upgrade step also multiplies the spell's power by
/// `1 + RANK_POWER / (1 + RANK_POWER_FALLOFF × max(0, rank − 2))`.
pub const RANK_POWER: f32 = 0.04;
pub const RANK_POWER_FALLOFF: f32 = 0.125;
/// When nothing at all can be offered, the screen shows a gold card worth this much.
pub const EMPTY_SCREEN_GOLD: u32 = 25;

/// Step size per rarity (common .. legendary) of each upgrade kind (our numbers).
pub const POWER_STEP: [f32; 5] = [0.15, 0.25, 0.35, 0.5, 0.65];
pub const COOLDOWN_STEP: [f32; 5] = [0.07, 0.10, 0.13, 0.17, 0.20];
/// Extra targets a card adds: none below rare, so a Targets card is never common or uncommon.
pub const TARGETS_STEP: [u32; 5] = [0, 0, 1, 2, 2];
pub const AREA_STEP: [f32; 5] = [0.10, 0.15, 0.25, 0.35, 0.5];
pub const DURATION_STEP: [f32; 5] = [0.15, 0.20, 0.30, 0.40, 0.60];
/// Power steps shrink with each power upgrade taken: `step / (1 + 0.5 × taken)`.
pub const POWER_FALLOFF: f32 = 0.5;
/// Cooldown multiplier floor, extra-target cap, area and duration multiplier caps.
pub const COOLDOWN_FLOOR: f32 = 0.2;
pub const MAX_EXTRA_TARGETS: u32 = 12;
pub const MAX_AREA: f32 = 5.0;
pub const MAX_DURATION: f32 = 6.0;
/// Rage %, Energy % and Chance cards: half a Cooldown card's strength, up to +200 % (× 3).
pub const GAIN_SHARE: f32 = 0.5;
pub const MAX_GAIN: f32 = 3.0;
/// Pet cards: at most 2 more pets (3 in all).
pub const MAX_EXTRA_PETS: u32 = 2;

/// What upgrades a known spell has taken. A new spell is rank 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpellUpgrades {
    pub rank: u32,
    pub power: f32,
    pub cooldown: f32,
    pub extra_targets: u32,
    /// Of `extra_targets`, what Stacks cards added: a spell whose stacks are its own (Maul's thick
    /// hide) leaves them out of its targets.
    pub extra_stacks: u32,
    pub area: f32,
    pub duration: f32,
    /// Rage gained, energy refilled or proc chance, × this (Rage %, Energy %, Chance cards).
    pub gain: f32,
    /// Upgrades taken per kind, in `Upgrade` order.
    pub taken: [u32; Upgrade::ALL.len()],
}

impl Default for SpellUpgrades {
    fn default() -> Self {
        Self { rank: 1, power: 1.0, cooldown: 1.0, extra_targets: 0, extra_stacks: 0, area: 1.0, duration: 1.0, gain: 1.0, taken: [0; Upgrade::ALL.len()] }
    }
}

/// How much one step of `kind` at `rarity` would add.
pub fn step(u: &SpellUpgrades, kind: Upgrade, rarity: Rarity) -> f32 {
    let r = rarity as usize;
    match kind {
        Upgrade::Power => POWER_STEP[r] / (1.0 + POWER_FALLOFF * u.taken[0] as f32),
        Upgrade::Cooldown => ((u.cooldown - COOLDOWN_FLOOR) / (1.0 - COOLDOWN_FLOOR)).max(0.0) * COOLDOWN_STEP[r],
        Upgrade::Targets => MAX_EXTRA_TARGETS.saturating_sub(u.extra_targets).min(TARGETS_STEP[r]) as f32,
        Upgrade::Area => ((MAX_AREA - u.area) * 0.25).max(0.0) * AREA_STEP[r],
        Upgrade::Duration => ((MAX_DURATION - u.duration) / 5.0).max(0.0) * DURATION_STEP[r],
        k if k.is_gain() => {
            // half a Cooldown card: the casting rate a cooldown cut of that step would give, halved
            let cut = ((MAX_GAIN - u.gain) / (MAX_GAIN - 1.0)).max(0.0) * COOLDOWN_STEP[r];
            GAIN_SHARE * (1.0 / (1.0 - cut) - 1.0)
        }
        Upgrade::Pets => MAX_EXTRA_PETS.saturating_sub(u.extra_targets).min(1) as f32,
        _ => MAX_EXTRA_TARGETS.saturating_sub(u.extra_targets).min(TARGETS_STEP[r]) as f32,
    }
}

fn is_lasting_buff(a: &Ability) -> bool {
    matches!(a.effect, SpellEffect::DamageBuff { .. } | SpellEffect::HasteBuff { .. } | SpellEffect::Thorns { .. })
}

/// Whether `kind` makes sense for `spell` at all (before caps).
///
/// `cooldown_still_matters` is false for a buff that, with the hero's cooldown speed, already
/// lasts as long as its cooldown; `extra_targets_on` is the hero-wide switch checked
/// before offering targets.
pub fn applies(spell: &Ability, kind: Upgrade, cooldown_still_matters: bool, extra_targets_on: bool) -> bool {
    use Delivery::*;
    let d = spell.delivery;
    let self_effect = d == SelfEffect;
    match kind {
        // a spell with a rule of its own (a starting spell): what it lists
        _ if spell.kit != esurv_content::SpellKit::default() => kind != Upgrade::Cooldown || cooldown_still_matters,
        Upgrade::Power => {
            if self_effect {
                matches!(spell.effect, SpellEffect::Heal { .. } | SpellEffect::Shield { .. } | SpellEffect::Thorns { .. })
            } else {
                spell.damage > 0.0
            }
        }
        Upgrade::Cooldown => {
            let always_up = self_effect && is_lasting_buff(spell) && spell.duration >= spell.cooldown;
            !always_up && cooldown_still_matters
        }
        Upgrade::Targets => {
            spell.targets != 0
                && (matches!(d, Projectile | Chain | Bounce | Melee | DamageOverTime | Finisher)
                    || (!matches!(d, Nova | GroundArea | Cone | SelfEffect) && spell.radius <= 0.0))
                && extra_targets_on
        }
        Upgrade::Area => matches!(d, Nova | GroundArea | Bounce | Cone) || (d == Totem && spell.radius > 0.0),
        Upgrade::Duration => match d {
            GroundArea | DamageOverTime | Totem => cooldown_still_matters,
            // a nova that repeats around the hero goes off more times
            Nova if spell.ticks > 1 => cooldown_still_matters,
            // healing charges (`bloodthirst`) last longer
            _ if matches!(spell.effect, SpellEffect::HealingCharges { .. }) => cooldown_still_matters,
            SelfEffect => {
                spell.duration > 0.0
                    && !matches!(spell.effect, SpellEffect::Heal { .. } | SpellEffect::Shield { .. })
                    && !(is_lasting_buff(spell) && spell.duration >= spell.cooldown)
                    && cooldown_still_matters
            }
            _ => false,
        },
        // the kinds that stand in for Cooldown or Targets on a starting spell: wherever it lists them
        _ => true,
    }
}

/// The upgrade kinds a card for `spell` may pick from, in the spell's `upgrades` order.
pub fn offerable(spell: &Ability, u: &SpellUpgrades, cooldown_still_matters: bool, extra_targets_on: bool) -> Vec<Upgrade> {
    spell
        .upgrades
        .iter()
        .copied()
        .filter(|&k| applies(spell, k, cooldown_still_matters, extra_targets_on))
        .filter(|&k| step(u, k, Rarity::Legendary) >= MIN_GAIN)
        .collect()
}

/// Applies one upgrade card: the rank goes up once per kind taken, each kind steps once, and
/// every step adds the small rank power bonus.
pub fn apply(u: &mut SpellUpgrades, kinds: &[Upgrade], rarity: Rarity) {
    for &k in kinds {
        u.rank += 1;
        let g = step(u, k, rarity);
        match k {
            Upgrade::Power => u.power *= 1.0 + g,
            Upgrade::Cooldown => u.cooldown -= g,
            Upgrade::Targets => u.extra_targets += g as u32,
            Upgrade::Area => u.area += g,
            Upgrade::Duration => u.duration += g,
            k if k.is_gain() => u.gain += g,
            _ => {
                if k == Upgrade::Stacks {
                    u.extra_stacks += g as u32;
                }
                u.extra_targets += g as u32
            }
        }
        u.taken[k as usize] += 1;
        u.power *= 1.0 + RANK_POWER / (1.0 + RANK_POWER_FALLOFF * u.rank.saturating_sub(2) as f32);
    }
}

/// How many kinds an upgrade card of `rarity` improves.
pub fn kinds_per_card(rarity: Rarity) -> usize {
    if rarity == Rarity::Legendary {
        LEGENDARY_KINDS
    } else {
        1
    }
}

/// Weight of a new-spell card, or `None` when the hero already holds `MAX_SPELLS`.
pub fn new_spell_weight(known: usize) -> Option<f32> {
    match known {
        k if k >= MAX_SPELLS => None,
        k if k < 3 => Some(NEW_SPELL_WEIGHT.0),
        _ => Some(NEW_SPELL_WEIGHT.1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_steps_shrink() {
        let mut u = SpellUpgrades::default();
        assert!((step(&u, Upgrade::Power, Rarity::Common) - 0.15).abs() < 1e-6);
        u.taken[0] = 2;
        assert!((step(&u, Upgrade::Power, Rarity::Common) - 0.075).abs() < 1e-6);
    }

    /// A Targets card adds none below rare, one at rare, two at epic
    /// and legendary.
    #[test]
    fn targets_cards_start_at_rare() {
        let u = SpellUpgrades::default();
        let adds: Vec<f32> = Rarity::ALL.iter().map(|&r| step(&u, Upgrade::Targets, r)).collect();
        assert_eq!(adds, [0.0, 0.0, 1.0, 2.0, 2.0]);
    }

    #[test]
    fn caps_stop_offers() {
        let u = SpellUpgrades { cooldown: 0.2, extra_targets: 12, area: 5.0, duration: 6.0, ..Default::default() };
        for k in [Upgrade::Cooldown, Upgrade::Targets, Upgrade::Area, Upgrade::Duration] {
            assert_eq!(step(&u, k, Rarity::Legendary), 0.0, "{k:?}");
        }
    }

    #[test]
    fn apply_steps_and_rank_bonus() {
        let mut u = SpellUpgrades::default();
        apply(&mut u, &[Upgrade::Cooldown], Rarity::Common);
        // a new spell is rank 1, so its first upgrade makes it rank 2 (full +4 % bonus)
        assert_eq!(u.rank, 2);
        assert!((u.cooldown - 0.93).abs() < 1e-6);
        assert!((u.power - 1.04).abs() < 1e-6);
        apply(&mut u, &[Upgrade::Power, Upgrade::Area], Rarity::Legendary);
        assert_eq!(u.rank, 4);
        assert_eq!(u.taken[..5], [1, 1, 0, 1, 0]);
        // power: 1.04, then x 1.65 and the rank-3 bonus, then the rank-4 bonus from the area step
        let want = 1.04 * 1.65 * (1.0 + 0.04 / 1.125) * (1.0 + 0.04 / 1.25);
        assert!((u.power - want).abs() < 1e-4);
        // area: (5 - 1) x 0.25 x 0.5 = +0.5
        assert!((u.area - 1.5).abs() < 1e-6);
    }

    #[test]
    fn new_spell_weights() {
        assert_eq!(new_spell_weight(2), Some(1.6));
        assert_eq!(new_spell_weight(3), Some(0.9));
        assert_eq!(new_spell_weight(6), None);
        assert_eq!(kinds_per_card(Rarity::Legendary), 2);
        assert_eq!(kinds_per_card(Rarity::Epic), 1);
    }
}
