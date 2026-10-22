//! The hero's stat sheet: every source of power (race, class gear, items, talents, buffs, and
//! later loadouts and class mechanics) contributes `Modifier`s, and one function folds them into
//! the final stats.

use esurv_content::{Class, Race, Stat};

/// One contribution to a stat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Modifier {
    pub stat: Stat,
    pub value: f32,
}

impl Modifier {
    pub fn new(stat: Stat, value: f32) -> Self {
        Self { stat, value }
    }
}

/// The race's lasting bonuses as stat modifiers.
pub fn race_modifiers(race: &Race) -> Vec<Modifier> {
    race.passives.iter().map(|p| Modifier::new(p.stat, p.value)).collect()
}

/// Base values and caps.
pub const HEALTH_PER_LEVEL: f32 = 9.0;
pub const DAMAGE_PER_LEVEL: f32 = 0.03;
pub const BASE_REGEN: f32 = 1.0;
/// Health per second gained per level.
pub const REGEN_PER_LEVEL: f32 = 0.15;
/// The most double cast a hero can have from every source together: 2 = always 3 casts.
/// One ring or trinket stops at 0.6 on its own (`offers::DOUBLE_CAST_CAP`).
pub const DOUBLE_CAST_MAX: f32 = 2.0;
pub const BASE_CRIT_CHANCE: f32 = 0.05;
pub const BASE_CRIT_DAMAGE: f32 = 2.0;
pub const BASE_PICKUP_RANGE: f32 = 10.0;
pub const MAX_MOVE_SPEED: f32 = 2.2;
pub const MAX_DAMAGE_REDUCTION: f32 = 0.6;
/// Yards per second at a move-speed multiplier of 1.
pub const HERO_RUN_SPEED: f32 = 7.0;

/// The hero's final stats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatSheet {
    pub max_health: f32,
    pub damage: f32,
    /// Multiply cooldown durations by this (`1 / (1 + recovery)`).
    pub cooldown_factor: f32,
    /// Multiply the weapon swing pace by the square root of this: the cooldown factor with the
    /// cooldown recovery of talents, rings and trinkets counted at full strength
    /// (their recovery counts half for spell cooldowns, attack speed is unchanged).
    pub swing_factor: f32,
    pub crit_chance: f32,
    pub crit_damage: f32,
    pub move_speed: f32,
    pub pickup_range: f32,
    pub experience: f32,
    pub damage_reduction: f32,
    pub health_regen: f32,
    pub area: f32,
    pub luck: f32,
    pub gold: f32,
    pub extra_targets: u32,
    pub double_cast: f32,
    pub heal_per_kill: f32,
    pub buff_power: f32,
    /// Strength of the heals of the hero's spells and of the shields it casts (0 = none added).
    pub heal_and_shield_power: f32,
    pub dot_stacks: u32,
    /// Share of each hit's damage that heals the hero (class base + bonuses).
    pub lifesteal: f32,
    /// Chance that an enemy's melee blow is blocked (0..1).
    pub block_chance: f32,
    /// Weapon swings this much faster (0.05 = 5 %).
    pub swing_speed: f32,
    /// This much more rage from damage taken (1.0 = twice as much).
    pub rage_when_hurt: f32,
    /// This much more rage from each weapon swing that hits (0.2 = 20 %).
    pub rage_per_swing: f32,
    /// Chance that a weapon swing hits twice (0..1; Hand of Justice).
    pub double_hit: f32,
}

impl StatSheet {
    /// Hero run speed in yards per second.
    pub fn run_speed(&self) -> f32 {
        HERO_RUN_SPEED * self.move_speed
    }
}

/// Folds `modifiers` into the stat sheet of `class` at `level` (1-based).
///
/// Stats handled outside this sheet (weapon damage, pet damage, thorns, the
/// "damage vs ..." bonuses, extra lives) are ignored here and read by the systems that use them.
pub fn compute(class: &Class, level: u32, modifiers: &[Modifier]) -> StatSheet {
    let l = level.saturating_sub(1) as f32;
    let mut flat_health = 0.0;
    let mut health_pct = 0.0;
    let mut damage = 1.0 + DAMAGE_PER_LEVEL * l;
    let mut recovery = 0.0;
    // cooldown recovery that only speeds weapon swings (the swing half of `SpellCooldownRecovery`)
    let mut swing_only_recovery = 0.0;
    let mut crit = BASE_CRIT_CHANCE;
    let mut crit_damage = BASE_CRIT_DAMAGE;
    let mut move_speed = 1.0;
    let mut pickup = BASE_PICKUP_RANGE;
    let mut experience = 1.0;
    let mut reduction = class.base_damage_reduction;
    let mut lifesteal = class.base_lifesteal;
    let mut regen = BASE_REGEN + REGEN_PER_LEVEL * l;
    let mut area = 1.0;
    let mut luck = 0.0;
    let mut gold = 1.0;
    let mut extra_targets = 0.0;
    let mut double_cast = 0.0;
    let mut heal_per_kill = 0.0;
    let mut buff_power = 0.0;
    let mut heal_and_shield_power = 0.0;
    let mut dot_stacks = 0.0;
    let mut block_chance = 0.0f32;
    let mut swing_speed = 0.0f32;
    let mut rage_when_hurt = 0.0f32;
    let mut rage_per_swing = 0.0f32;
    let mut double_hit = 0.0f32;
    let mut damage_from_reduction = 0.0f32;
    for m in modifiers {
        let v = m.value;
        match m.stat {
            Stat::Damage => damage += v,
            Stat::CooldownRecovery => recovery += v,
            Stat::SpellCooldownRecovery => {
                recovery += v;
                swing_only_recovery += v;
            }
            Stat::SwingCooldownRecovery => swing_only_recovery += v,
            Stat::MaxHealth => health_pct += v,
            Stat::FlatMaxHealth => flat_health += v,
            Stat::CritChance => crit += v,
            Stat::CritDamage => crit_damage += v,
            Stat::DamageReduction => reduction += v,
            Stat::HealthPerSecond => regen += v,
            Stat::MoveSpeed => move_speed += v,
            Stat::Area => area += v,
            Stat::PickupRange => pickup *= 1.0 + v,
            Stat::Experience => experience += v,
            Stat::Gold => gold += v,
            Stat::Luck => luck += v,
            Stat::DoubleCast => double_cast += v,
            Stat::HealPerKill => heal_per_kill += v,
            Stat::BuffPower => buff_power += v,
            Stat::HealAndShieldPower => heal_and_shield_power += v,
            Stat::DotStacks => dot_stacks += v,
            Stat::ExtraTargets => extra_targets += v,
            Stat::Lifesteal => lifesteal += v,
            Stat::WeaponDamage
            | Stat::PetDamage
            | Stat::DamageVsCrowdControlled
            | Stat::DamageVsElites
            | Stat::DamageBelow30Pct
            | Stat::DamageVsAfflicted
            | Stat::Thorns
            | Stat::ExtraLives => {}
            Stat::BlockChance => block_chance += v,
            Stat::SwingSpeed => swing_speed += v,
            Stat::RageWhenHurt => rage_when_hurt += v,
            Stat::RagePerSwing => rage_per_swing += v,
            Stat::WeaponDoubleHit => double_hit += v,
            Stat::DamageFromReduction => damage_from_reduction += v,
        }
    }
    // crit chance beyond 100 % turns into crit damage
    if crit > 1.0 {
        crit_damage += crit - 1.0;
        crit = 1.0;
    }
    // damage reduction turned into damage (Bulwark)
    let damage = damage + reduction.min(MAX_DAMAGE_REDUCTION) * damage_from_reduction;
    StatSheet {
        max_health: (class.base_health + HEALTH_PER_LEVEL * l + flat_health) * (1.0 + health_pct),
        damage,
        cooldown_factor: 1.0 / (1.0 + recovery),
        swing_factor: 1.0 / (1.0 + recovery + swing_only_recovery),
        crit_chance: crit,
        crit_damage,
        move_speed: move_speed.min(MAX_MOVE_SPEED),
        pickup_range: pickup,
        experience,
        damage_reduction: reduction.min(MAX_DAMAGE_REDUCTION),
        health_regen: regen,
        area,
        luck,
        gold,
        extra_targets: extra_targets.max(0.0) as u32,
        double_cast: double_cast.min(DOUBLE_CAST_MAX),
        heal_per_kill,
        buff_power,
        heal_and_shield_power,
        dot_stacks: dot_stacks.round().max(0.0) as u32,
        lifesteal,
        block_chance: block_chance.clamp(0.0, 1.0),
        swing_speed,
        rage_when_hurt,
        rage_per_swing,
        double_hit: double_hit.clamp(0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_content::PowerType;

    fn warrior() -> Class {
        Class {
            id: "warrior".into(),
            name_ref: "Class Placeholder".into(),
            name: "Class Placeholder".into(),
            wow_class_id: 1,
            power: PowerType::Rage,
            color: String::new(),
            icon: String::new(),
            description: String::new(),
            base_health: 230.0,
            base_damage_reduction: 0.22,
            base_lifesteal: 0.015,
            auto_attack: None,
            abilities: vec![],
            starting_options: Vec::new(),
            spell_damage_scale: 1.0,
        }
    }

    #[test]
    fn double_cast_stops_at_two_hundred_percent() {
        let m: Vec<Modifier> = (0..5).map(|_| Modifier::new(Stat::DoubleCast, 0.6)).collect();
        assert_eq!(compute(&warrior(), 1, &m).double_cast, DOUBLE_CAST_MAX);
        assert!((compute(&warrior(), 1, &m[..2]).double_cast - 1.2).abs() < 1e-6);
    }

    #[test]
    fn level_one_baseline() {
        let s = compute(&warrior(), 1, &[]);
        assert_eq!(s.max_health, 230.0);
        assert_eq!((s.damage, s.cooldown_factor, s.crit_chance, s.crit_damage), (1.0, 1.0, 0.05, 2.0));
        assert_eq!((s.damage_reduction, s.health_regen, s.pickup_range), (0.22, 1.0, 10.0));
        assert_eq!(s.lifesteal, 0.015);
        assert_eq!(s.run_speed(), 7.0);
    }

    #[test]
    fn levels_add_health_damage_and_regen() {
        let s = compute(&warrior(), 11, &[]);
        assert_eq!(s.max_health, 230.0 + 90.0);
        assert!((s.damage - 1.3).abs() < 1e-6);
        assert!((s.health_regen - (1.0 + 1.5)).abs() < 1e-5, "+0.15 per level above 1");
    }

    #[test]
    fn flat_health_before_percent_and_caps() {
        let m = [
            Modifier::new(Stat::FlatMaxHealth, 70.0),
            Modifier::new(Stat::MaxHealth, 0.1),
            Modifier::new(Stat::MoveSpeed, 5.0),
            Modifier::new(Stat::DamageReduction, 1.0),
            Modifier::new(Stat::CooldownRecovery, 1.0),
        ];
        let s = compute(&warrior(), 1, &m);
        assert!((s.max_health - 330.0).abs() < 1e-3);
        assert_eq!((s.move_speed, s.damage_reduction, s.cooldown_factor), (2.2, 0.6, 0.5));
    }

    #[test]
    fn excess_crit_becomes_crit_damage_and_pickup_multiplies() {
        let m = [
            Modifier::new(Stat::CritChance, 1.15),
            Modifier::new(Stat::PickupRange, 0.3),
            Modifier::new(Stat::PickupRange, 0.5),
        ];
        let s = compute(&warrior(), 1, &m);
        assert_eq!(s.crit_chance, 1.0);
        assert!((s.crit_damage - 2.2).abs() < 1e-5);
        assert!((s.pickup_range - 10.0 * 1.3 * 1.5).abs() < 1e-4);
    }
}
