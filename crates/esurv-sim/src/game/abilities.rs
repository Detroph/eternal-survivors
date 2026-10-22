//! Casting abilities: the numbers every delivery kind shares.
//!
//! The per-kind targeting (nearest, random order, area placement, cones) lives with the world
//! model; this module holds the formulas so they can be tested on their own.

use esurv_content::{Ability, Delivery, SpellEffect};

// Every spell's damage, durations, heal amounts and cooldown are its listed numbers (see
// docs/game/spells.md). One knob per class, `Class::spell_damage_scale`, multiplies only the
// damage (and heals, shields and aura damage) of that class's spells. Race spells never take
// it. The spell tag coefficients (`balance.ron` `spell_tags`) are already in the spells' listed
// numbers when content loads
// (`esurv_content::apply_spell_tags`), so every formula here, and every card, sees them; the
// class knob multiplies on top.

/// Delay between successive hits of a chain ability.
pub const CHAIN_STEP: f32 = 0.12;
/// First tick of a lasting area.
pub const AREA_FIRST_TICK: f32 = 0.25;
/// Most extra DoT stacks per target.
pub const MAX_DOT_STACKS: u32 = 3;
/// Heals are only cast at or below this share of max health.
pub const HEAL_BELOW: f32 = 0.85;
/// Immunities are only cast at or below this share of max health.
pub const IMMUNITY_BELOW: f32 = 0.35;
/// A spell on the hero (a heal over time, a shield, a buff, an immunity) lasts at least this long.
pub const SELF_EFFECT_MIN_SECONDS: f32 = 0.1;
/// Buffs wait for an enemy within this many yards.
pub const BUFF_ENEMY_RANGE: f32 = 30.0;
/// Totems wait for an enemy within this many yards.
pub const TOTEM_ENEMY_RANGE: f32 = 25.0;
/// A finisher strikes an enemy below this share of its health for this many times its damage.
pub const FINISHER_BELOW: f32 = 0.3;
pub const FINISHER_MULT: f32 = 2.0;

/// A spell's multipliers during the run, each number in two parts: what the hero's upgrade cards
/// gave it (`*_from_cards`) and what talents and the amulet give it (`*_from_talents`); see
/// `spells::SpellSlot`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Instance {
    /// The class's damage knob (`Class::spell_damage_scale`; 1 for race spells): damage, heals,
    /// shields and aura damage only.
    pub damage_scale: f32,
    pub power_from_cards: f32,
    pub power_from_talents: f32,
    pub cooldown_from_cards: f32,
    pub cooldown_from_talents: f32,
    pub radius_from_cards: f32,
    pub radius_from_talents: f32,
    pub duration_from_cards: f32,
    pub duration_from_talents: f32,
    pub extra_targets_from_cards: u32,
    pub extra_targets_from_talents: u32,
    /// Stacks cards taken for the spell (what a stacking rule of its own counts: Maul's thick
    /// hide, `Mechanic::ReductionStacks`), for its card's numbers.
    pub stacks_from_cards: u32,
    /// What those Stacks cards added to `extra_targets_from_cards`.
    pub stack_targets_from_cards: u32,
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            damage_scale: 1.0,
            power_from_cards: 1.0,
            power_from_talents: 1.0,
            cooldown_from_cards: 1.0,
            cooldown_from_talents: 1.0,
            radius_from_cards: 1.0,
            radius_from_talents: 1.0,
            duration_from_cards: 1.0,
            duration_from_talents: 1.0,
            extra_targets_from_cards: 0,
            extra_targets_from_talents: 0,
            stacks_from_cards: 0,
            stack_targets_from_cards: 0,
        }
    }
}

/// Damage of one hit before crit and per-target bonuses. Every delivery kind hits for its listed
/// `damage` (no hidden factor for novas, areas or cones).
pub fn hit_damage(ability: &Ability, inst: &Instance, hero_damage: f32) -> f32 {
    hero_damage * ability.damage * inst.damage_scale * inst.power_from_cards * inst.power_from_talents
}

/// What a ground area's impact (`Ability::impact`) deals to each enemy inside it as it lands,
/// before crit: scaled as its hits are (`hit_damage`).
pub fn impact_damage(ability: &Ability, inst: &Instance, hero_damage: f32) -> f32 {
    hero_damage * ability.impact * inst.damage_scale * inst.power_from_cards * inst.power_from_talents
}

/// How many targets one cast hits. The hero's own extra targets (the off-hand loadouts) reach only
/// spells that hit several ("+1 target on multi-target spells").
pub fn target_count(ability: &Ability, inst: &Instance, hero_extra_targets: u32) -> u32 {
    if ability.targets == 0 {
        return 1;
    }
    let hero_extra_targets = if ability.targets > 1 { hero_extra_targets } else { 0 };
    // a spell whose stacks are its own (Maul's thick hide): its Stacks cards add no targets
    let own_stacks = matches!(ability.kit.mechanic, Some(esurv_content::Mechanic::ReductionStacks { .. }));
    let from_cards = if own_stacks {
        inst.extra_targets_from_cards.saturating_sub(inst.stack_targets_from_cards)
    } else {
        inst.extra_targets_from_cards
    };
    (ability.targets + hero_extra_targets + from_cards + inst.extra_targets_from_talents).max(1)
}

/// Radius of novas and areas (cones use `range` in place of `radius`).
pub fn radius(ability: &Ability, inst: &Instance, hero_area: f32) -> f32 {
    let base = if ability.delivery == Delivery::Cone { ability.range } else { ability.radius };
    base * hero_area * inst.radius_from_cards * inst.radius_from_talents
}

/// Duration of areas, DoTs and self effects.
pub fn duration(ability: &Ability, inst: &Instance) -> f32 {
    ability.duration * inst.duration_from_cards * inst.duration_from_talents
}

/// Tick count and per-tick damage of a lasting area or DoT: ticks keep their size and rate, and
/// extra duration adds only the whole ticks that fit in it (10 ticks
/// over 10 s made 15 % longer is 11 ticks, not 11.5 rounded up).
pub fn ticks(ability: &Ability, inst: &Instance, hit: f32) -> (u32, f32) {
    let base_ticks = ability.ticks.max(1) as f32;
    let base_duration = ability.duration.max(0.01);
    // the small margin keeps an exact fit (6 s × 1.5 = 9 ticks) from losing a tick to rounding
    let n = ((duration(ability, inst) / base_duration) * base_ticks + 1e-3).floor().max(1.0) as u32;
    (n, hit / base_ticks)
}

/// How often one cast of a nova goes off and how many seconds apart: a nova with 2 or more
/// `ticks` repeats around the hero, `duration ÷ ticks` seconds apart, the first at once (3 ticks
/// over 3 s: at 0, 1 and 2 s), each for the full hit; extra duration adds only the whole novas
/// that fit, at the same spacing (as `ticks`). Any other nova goes off once.
pub fn nova_repeats(ability: &Ability, inst: &Instance) -> (u32, f32) {
    if ability.ticks < 2 {
        return (1, 0.0);
    }
    let (count, _) = ticks(ability, inst, 0.0);
    (count, ability.duration.max(0.01) / ability.ticks as f32)
}

/// The burn a spell's hit leaves on each enemy it lands on (`Ability::burn`), before crit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnTicks {
    pub per_tick: f32,
    pub ticks: u32,
    /// Seconds between ticks; the first comes this long after the hit lands.
    pub interval: f32,
}

impl BurnTicks {
    /// All its ticks together.
    pub fn total(&self) -> f32 {
        self.per_tick * self.ticks as f32
    }

    /// Seconds from the hit to its last tick.
    pub fn seconds(&self) -> f32 {
        self.interval * self.ticks as f32
    }
}

/// The burn of `ability` cast as `inst` by a hero with `hero_damage`, if it has one: its listed
/// total scaled as the hit is (`hit_damage`), split evenly over its ticks; a longer duration adds
/// only the whole ticks that fit, at the same size and rate (as `ticks`).
pub fn burn_ticks(ability: &Ability, inst: &Instance, hero_damage: f32) -> Option<BurnTicks> {
    let burn = ability.burn?;
    let base_ticks = burn.ticks.max(1);
    let total = hero_damage * burn.damage * inst.damage_scale * inst.power_from_cards * inst.power_from_talents;
    let stretch = inst.duration_from_cards * inst.duration_from_talents;
    // the small margin keeps an exact fit from losing a tick to rounding
    let ticks = ((stretch * base_ticks as f32) + 1e-3).floor().max(1.0) as u32;
    Some(BurnTicks { per_tick: total / base_ticks as f32, ticks, interval: burn.duration.max(0.01) / base_ticks as f32 })
}

/// Whether a self-effect ability may be cast now.
pub fn self_effect_ready(ability: &Ability, health: f32, max_health: f32, shield_up: bool, enemy_within_30: bool) -> bool {
    match ability.effect {
        SpellEffect::Heal { .. } => health <= max_health * ability.heal_below.unwrap_or(HEAL_BELOW),
        SpellEffect::Shield { .. } => !shield_up,
        SpellEffect::Immunity { .. } => health <= max_health * IMMUNITY_BELOW,
        _ => enemy_within_30,
    }
}

/// Health restored by a heal (instant or over its duration).
pub fn heal_amount(share: f32, inst: &Instance, max_health: f32) -> f32 {
    (inst.power_from_cards * inst.power_from_talents).sqrt() * inst.damage_scale * max_health * share
}

/// Damage a shield absorbs.
pub fn shield_amount(share: f32, inst: &Instance, max_health: f32, buff_power: f32) -> f32 {
    (1.0 + buff_power) * (inst.power_from_cards * inst.power_from_talents).sqrt() * inst.damage_scale * share * max_health
}

/// A finisher's damage against a target at `health` of `max_health`.
pub fn finisher_damage(hit: f32, health: f32, max_health: f32) -> f32 {
    if health / max_health < FINISHER_BELOW {
        hit * FINISHER_MULT
    } else {
        hit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_content::School;

    fn ability(delivery: Delivery, damage: f32, targets: u32) -> Ability {
        Ability {
            id: "t".into(),
            name_ref: Default::default(),
            name: String::new(),
            description: String::new(),
            icon: String::new(),
            look_spell: 0,
            school: School::Fire,
            delivery,
            categories: vec![],
            tags: vec![],
            upgrades: vec![],
            free_casts_can_proc: false,
            damage,
            cooldown: 1.0,
            range: 30.0,
            radius: 8.0,
            targets,
            duration: 6.0,
            ticks: 6,
            pulse_spell: None,
            effect: SpellEffect::None,
            heal_below: None,
            double_on_low_health: false,
            burn: None,
            impact: 0.0,
            kit: Default::default(),
        }
    }

    #[test]
    fn a_self_heal_waits_for_its_own_health_share() {
        let mut heal = Ability { effect: SpellEffect::Heal { share: 0.35 }, ..ability(Delivery::SelfEffect, 0.0, 0) };
        assert!(!self_effect_ready(&heal, 86.0, 100.0, false, true));
        assert!(self_effect_ready(&heal, 85.0, 100.0, false, true));
        heal.heal_below = Some(0.65);
        assert!(!self_effect_ready(&heal, 66.0, 100.0, false, true));
        assert!(self_effect_ready(&heal, 65.0, 100.0, false, true));
    }

    #[test]
    fn hit_damage_is_the_listed_damage_for_every_kind() {
        let i = Instance::default();
        assert!((hit_damage(&ability(Delivery::Projectile, 50.5, 1), &i, 1.0) - 50.5).abs() < 1e-3);
        assert!((hit_damage(&ability(Delivery::Nova, 25.0, 0), &i, 1.0) - 25.0).abs() < 1e-3);
        // the class damage knob multiplies the hit
        let k = Instance { damage_scale: 1.4, ..Default::default() };
        assert!((hit_damage(&ability(Delivery::Projectile, 50.5, 1), &k, 1.0) - 70.7).abs() < 1e-3);
        assert!((hit_damage(&ability(Delivery::Nova, 25.0, 0), &k, 1.0) - 35.0).abs() < 1e-3);
    }

    #[test]
    fn a_burn_scales_with_power_and_a_longer_duration_adds_whole_ticks() {
        let burn = Some(esurv_content::Burn { damage: 160.0, duration: 5.0, ticks: 5 });
        let a = Ability { burn, ..ability(Delivery::Projectile, 280.0, 1) };
        assert_eq!(burn_ticks(&ability(Delivery::Projectile, 280.0, 1), &Instance::default(), 1.0), None);
        let base = burn_ticks(&a, &Instance::default(), 1.0).unwrap();
        assert_eq!(base, BurnTicks { per_tick: 32.0, ticks: 5, interval: 1.0 });
        assert_eq!((base.total(), base.seconds()), (160.0, 5.0));
        // power, the class knob and hero damage scale every tick as they scale the hit
        let strong = Instance { power_from_cards: 1.5, damage_scale: 2.0, ..Default::default() };
        assert_eq!(burn_ticks(&a, &strong, 1.0).unwrap().per_tick, 96.0);
        assert_eq!(burn_ticks(&a, &Instance::default(), 2.0).unwrap().per_tick, 64.0);
        // 30 % longer holds 6 whole ticks of the same size and rate, not 6.5
        let long = Instance { duration_from_cards: 1.3, ..Default::default() };
        assert_eq!(burn_ticks(&a, &long, 1.0).unwrap(), BurnTicks { per_tick: 32.0, ticks: 6, interval: 1.0 });
        let double = Instance { duration_from_cards: 2.0, ..Default::default() };
        assert_eq!(burn_ticks(&a, &double, 1.0).unwrap().ticks, 10);
    }

    #[test]
    fn a_repeating_nova_goes_off_ticks_times_duration_over_ticks_apart_and_longer_adds_whole_novas() {
        let a = Ability { duration: 3.0, ticks: 3, ..ability(Delivery::Nova, 13.5, 0) };
        assert_eq!(nova_repeats(&a, &Instance::default()), (3, 1.0));
        // 30 % longer holds 3.9 novas: still 3; 50 % longer holds 4.5: 4, at the same spacing
        assert_eq!(nova_repeats(&a, &Instance { duration_from_cards: 1.3, ..Default::default() }), (3, 1.0));
        assert_eq!(nova_repeats(&a, &Instance { duration_from_cards: 1.5, ..Default::default() }), (4, 1.0));
        // an ordinary nova goes off once
        assert_eq!(nova_repeats(&Ability { duration: 0.0, ticks: 0, ..ability(Delivery::Nova, 13.5, 0) }, &Instance::default()).0, 1);
    }

    #[test]
    fn targets() {
        let i = Instance { extra_targets_from_cards: 1, ..Default::default() };
        assert_eq!(target_count(&ability(Delivery::Projectile, 1.0, 0), &i, 2), 1);
        assert_eq!(target_count(&ability(Delivery::Projectile, 1.0, 3), &i, 2), 6);
    }

    #[test]
    fn longer_dots_tick_more_for_the_same_per_tick_damage() {
        let a = ability(Delivery::DamageOverTime, 72.0, 2);
        let base = Instance::default();
        assert_eq!(ticks(&a, &base, 60.0), (6, 10.0));
        // the class damage knob never stretches durations
        let k = Instance { damage_scale: 1.4, ..Default::default() };
        assert_eq!((ticks(&a, &k, 60.0).0, duration(&a, &k)), (6, 6.0));
        let long = Instance { duration_from_cards: 2.0, ..Default::default() };
        assert_eq!(ticks(&a, &long, 60.0).0, 12);
        // only whole ticks: 7.5 s holds 7 ticks of 1 s, not 8
        let a_bit_longer = Instance { duration_from_cards: 1.25, ..Default::default() };
        assert_eq!(ticks(&a, &a_bit_longer, 60.0), (7, 10.0));
        let half_again = Instance { duration_from_cards: 1.5, ..Default::default() };
        assert_eq!(ticks(&a, &half_again, 60.0).0, 9);
    }

    #[test]
    fn finisher_doubles_below_30_percent() {
        assert_eq!(finisher_damage(10.0, 29.0, 100.0), 20.0);
        assert_eq!(finisher_damage(10.0, 30.0, 100.0), 10.0);
    }

    #[test]
    fn heals_and_shields() {
        let i = Instance { power_from_cards: 4.0, ..Default::default() };
        assert!((heal_amount(0.25, &i, 200.0) - 2.0 * 200.0 * 0.25).abs() < 1e-3);
        assert!((shield_amount(0.25, &Instance::default(), 200.0, 0.5) - 1.5 * 0.25 * 200.0).abs() < 1e-3);
        let k = Instance { damage_scale: 1.4, ..Default::default() };
        assert!((shield_amount(0.25, &k, 200.0, 0.5) - 1.5 * 1.4 * 0.25 * 200.0).abs() < 1e-3);
        assert!((heal_amount(0.25, &k, 200.0) - 1.4 * 200.0 * 0.25).abs() < 1e-3);
    }
}
