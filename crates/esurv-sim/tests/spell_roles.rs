//! Every damaging class spell hits its role's damage target (see
//! docs/game/spells.md, "Spell roles").
//!
//! The damage is worked out with the sim's own formulas (hit damage, target count, bounce
//! falloff, ticks) for a hero with no upgrades, talents or gear, against enemies above 30 %
//! health.

use esurv_content::{Ability, Content, Delivery};
use esurv_sim::game::abilities::{burn_ticks, duration, hit_damage, nova_repeats, target_count, ticks, Instance};
use esurv_sim::game::casting::{BOUNCE_FALLOFF, TOTEM_FIRST_PULSE, TOTEM_PULSE};

/// How far a spell may sit from its role's target.
const TOLERANCE: f32 = 0.03;

#[derive(Debug, Clone, Copy)]
enum Role {
    /// Single target, cooldown up to 2.5 s: 40 damage per second.
    Filler,
    /// Two or three targets: 45 damage per second in total.
    FewTargets,
    /// A bouncing chain: 50 damage per second in total over the whole chain.
    Chain,
    /// Hits on random enemies: 60 damage per second in total.
    RandomHits,
    /// 40 damage per second in total (twice that on enemies under 30 % health).
    Finisher,
    /// Like `FewTargets`, and doubled on enemies under 30 % health.
    FewTargetsFinisher,
    /// Damage over time: 100 per enemy for every 4 s it lasts.
    OverTime,
    /// Damage over time at a total per enemy set for this spell, whatever it lasts.
    OverTimeTotal(f32),
    /// A totem: this much damage per second to each enemy it hits, all its pulses over its cooldown
    /// (`magma_totem` at half of `consecration`'s 10).
    Totem(f32),
    /// Single target at a damage per second set for this spell, whatever its cooldown
    /// (a hit's burn counts with its hit).
    FixedDps(f32),
    /// Damage over time at a damage per second set over a pack: every target's whole
    /// effect, divided by the cooldown.
    PackDps(f32),
    /// Starting spells, areas, utility, buffs, heals and shields keep their numbers.
    Unchanged,
}

use Role::*;

const ROLES: &[(&str, Role)] = &[
    // `warrior`
    // a card spell now: 30 to each of 3 every 2 s
    ("cleave", FewTargets),
    ("whirlwind", Unchanged),
    ("thunder_clap", Unchanged),
    // the single-target nukes at 50 a second
    ("mortal_strike", FixedDps(50.0)),
    ("execute", Finisher),
    ("demo_shout", Unchanged),
    ("battle_shout", Unchanged),
    // 35 a second (its healing charges are its worth)
    ("bloodthirst", FixedDps(35.0)),
    // these set by their damage a second over a pack (targets x total / cooldown)
    ("rend", OverTimeTotal(150.0)),
    ("shield_slam", FewTargets),
    // `paladin`
    ("consecration", Unchanged),
    // `paladin`: `exorcism` and `judgement` 50 a second
    ("exorcism", FixedDps(50.0)),
    ("hammer_of_wrath", FixedDps(40.0)),
    ("holy_shock", RandomHits),
    ("holy_wrath", Unchanged),
    ("judgement", FixedDps(50.0)),
    ("hammer_of_justice", Unchanged),
    ("holy_light", Unchanged),
    ("divine_shield", Unchanged),
    ("holy_shield", Unchanged),
    // `hunter`
    ("auto_shot", Unchanged),
    // `hunter`: `arcane_shot` at 45 a second, `aimed_shot` 50
    ("arcane_shot", FixedDps(45.0)),
    ("multi_shot", FewTargets),
    ("serpent_sting", OverTimeTotal(150.0)),
    ("volley", Unchanged),
    ("explosive_trap", Unchanged),
    ("concussive_shot", Unchanged),
    ("aimed_shot", FixedDps(50.0)),
    ("immolation_trap", Unchanged),
    ("frost_trap", Unchanged),
    // `rogue`
    ("sinister_strike", Unchanged),
    ("eviscerate", Unchanged),
    ("blade_flurry", Unchanged),
    ("rupture", Unchanged),
    ("fan_of_knives", Unchanged),
    ("garrote", OverTime),
    ("kidney_shot", Unchanged),
    ("gouge", Unchanged),
    // `ghostly_strike` is a defensive strike: 30 a second, its shield kept
    ("ghostly_strike", FixedDps(30.0)),
    ("adrenaline_rush", Unchanged),
    ("backstab", FewTargetsFinisher),
    ("slice_and_dice", Unchanged),
    // `priest`
    ("smite", Unchanged),
    ("sw_pain", OverTime),
    // `priest`: `mind_blast` 50 a second
    ("mind_blast", FixedDps(50.0)),
    ("holy_nova", Unchanged),
    ("pw_shield", Unchanged),
    ("renew", Unchanged),
    ("holy_fire", Unchanged),
    ("divine_star", Unchanged),
    ("power_infusion", Unchanged),
    ("mind_flay", Unchanged),
    ("psychic_scream", Unchanged),
    // over a pack, 66 % of `sw_pain`'s damage a second (2 targets × 210 ÷ 3 s = 140)
    ("devouring_plague", PackDps(0.66 * 140.0)),
    // `shaman`
    ("lightning_bolt", Unchanged),
    ("chain_lightning", Chain),
    // `shaman`: `earth_shock` 50 a second
    ("earth_shock", FixedDps(50.0)),
    ("flame_shock", OverTime),
    ("frost_shock", FewTargets),
    ("lightning_shield", Unchanged),
    ("magma_totem", Totem(5.0)),
    ("searing_totem", Totem(20.0)),
    ("fire_nova", Unchanged),
    ("healing_wave", Unchanged),
    // `mage`: each damaging spell still meets its role's target
    // (`fire_blast` 45 a second, `pyroblast` 50)
    ("fireball", Unchanged),
    ("frostbolt", Unchanged),
    ("arcane_missiles", RandomHits),
    ("frost_nova", Unchanged),
    ("arcane_explosion", Unchanged),
    ("blizzard", Unchanged),
    ("flamestrike", Unchanged),
    ("fire_blast", FixedDps(45.0)),
    ("cone_of_cold", Unchanged),
    ("blast_wave", Unchanged),
    ("pyroblast", FixedDps(50.0)),
    ("ice_barrier", Unchanged),
    // `warlock`: `searing_pain` 45 a second, `curse_of_agony` 100 per enemy for every 4 s,
    // `death_coil` 25 a second
    ("shadow_bolt", Unchanged),
    // these totals per enemy sit above the 100-for-every-4-s rule
    ("corruption", OverTimeTotal(220.0)),
    ("immolate", OverTimeTotal(600.0)),
    // single-target effects over time: `curse_of_agony` 60, `immolate` and `rupture` 50 a second
    ("curse_of_agony", OverTimeTotal(600.0)),
    ("rain_of_fire", Unchanged),
    ("hellfire", Unchanged),
    ("death_coil", FixedDps(25.0)),
    ("searing_pain", FixedDps(45.0)),
    // its total set too; its 60 % heal from damage is kept
    ("drain_life", OverTimeTotal(200.0)),
    ("fear", Unchanged),
    // `druid`
    ("wrath", Unchanged),
    // `moonfire` (150) and `insect_swarm` (33, mainly a weakening) set by their damage a second over a pack
    ("moonfire", OverTimeTotal(115.0)),
    ("starfire", Unchanged),
    ("hurricane", Unchanged),
    ("insect_swarm", OverTimeTotal(33.0)),
    ("entangling_roots", Unchanged),
    ("rejuvenation", Unchanged),
    ("regrowth", Unchanged),
    ("thorns", Unchanged),
    ("swipe", Unchanged),
    ("cat_swipe", Unchanged),
    // the cat's and bear's cards (their own pools)
    ("rake", OverTime),
    ("shred", Finisher),
    ("tigers_fury", Unchanged),
    ("pounce", OverTime),
    ("ravage", FixedDps(40.0)),
    ("faerie_fire_feral", Unchanged),
    ("dash", Unchanged),
    ("ferocious_bite", Unchanged),
    ("powershift", Unchanged),
    ("berserk", Unchanged),
    ("demoralizing_roar", Unchanged),
    ("bash", Unchanged),
    ("enrage", Unchanged),
    ("frenzied_regeneration", Unchanged),
    ("barkskin", Unchanged),
    // 150 a stack, up to 3 stacks on each enemy
    ("lacerate", OverTimeTotal(150.0)),
];

fn content() -> Content {
    Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).expect("content loads")
}

/// Damage one cast deals in total, over every target it reaches (a chain's whole bounce).
fn damage_per_cast(a: &Ability) -> f32 {
    let inst = Instance::default();
    let hit = hit_damage(a, &inst, 1.0);
    let n = target_count(a, &inst, 0);
    let hits = match a.delivery {
        Delivery::Bounce => (0..n).map(|k| hit * BOUNCE_FALLOFF.powi(k as i32)).sum(),
        _ => hit * n as f32,
    };
    // a hit's burn: every tick of it, on each enemy the hit lands on
    let burn = burn_ticks(a, &inst, 1.0).map_or(0.0, |b| b.total() * n as f32);
    hits + burn
}

/// What the spell deals against its role's target, and that target.
fn measured_and_target(a: &Ability, role: Role) -> Option<(f32, f32)> {
    let dps = damage_per_cast(a) / a.cooldown;
    Some(match role {
        Filler => {
            assert!(a.cooldown <= 2.5, "{}: a filler's cooldown is 2.5 s at most", a.id);
            (dps, 40.0)
        }
        // an effect over time's hit damage is all its ticks, so this is every target's whole effect
        FixedDps(target) | PackDps(target) => (dps, target),
        FewTargets | FewTargetsFinisher => {
            assert!((2..=3).contains(&a.targets), "{}: hits 2 or 3 targets", a.id);
            (dps, 45.0)
        }
        Chain => (dps, 50.0),
        RandomHits => (dps, 60.0),
        Finisher => (dps, 40.0),
        OverTime => {
            // total per enemy over the whole effect: every tick of one stack
            let (count, per_tick) = ticks(a, &Instance::default(), hit_damage(a, &Instance::default(), 1.0));
            (count as f32 * per_tick, 100.0 * a.duration / 4.0)
        }
        OverTimeTotal(total) => {
            let (count, per_tick) = ticks(a, &Instance::default(), hit_damage(a, &Instance::default(), 1.0));
            (count as f32 * per_tick, total)
        }
        Totem(target) => {
            // a pulse hits each enemy once: 0.5 s after placing, then every 2 s while it stands
            let inst = Instance::default();
            let pulses = ((duration(a, &inst) - TOTEM_FIRST_PULSE) / TOTEM_PULSE).floor() + 1.0;
            (hit_damage(a, &inst, 1.0) * pulses / a.cooldown, target)
        }
        Unchanged => return None,
    })
}

#[test]
fn every_class_spell_has_a_role() {
    let c = content();
    for class in &c.classes {
        let pools = class.starting_options.iter().filter_map(|o| o.card_pool.as_ref()).flatten();
        for id in class.abilities.iter().chain(pools) {
            assert!(ROLES.iter().any(|(r, _)| r == id), "{} ({}) has no role in this test", id, class.id);
        }
    }
    for (id, _) in ROLES {
        assert!(c.ability(id).is_some(), "{id} is not a spell");
    }
}

#[test]
fn every_damaging_class_spell_hits_its_role_s_damage_target() {
    let c = content();
    for &(id, role) in ROLES {
        let a = c.ability(id).unwrap();
        let Some((measured, target)) = measured_and_target(a, role) else { continue };
        assert!(
            (measured / target - 1.0).abs() <= TOLERANCE,
            "{id} ({role:?}): {measured:.2} against a target of {target:.2}"
        );
    }
}

#[test]
fn finishers_are_the_finisher_delivery() {
    let c = content();
    for &(id, role) in ROLES {
        let finisher = matches!(role, Finisher | FewTargetsFinisher);
        assert_eq!(finisher, c.ability(id).unwrap().delivery == Delivery::Finisher, "{id}");
    }
}

/// `flamestrike`: 40 % of its damage lands as the area does, the rest
/// burns over its 4 ticks, about 11.6 a second to each enemy over its cooldown.
#[test]
fn flamestrike_splits_its_damage_between_its_impact_and_its_ticks() {
    let c = content();
    let a = c.ability("flamestrike").unwrap();
    let inst = Instance::default();
    let impact = esurv_sim::game::abilities::impact_damage(a, &inst, 1.0);
    let (count, per_tick) = ticks(a, &inst, hit_damage(a, &inst, 1.0));
    let total = impact + count as f32 * per_tick;
    assert_eq!((count, impact, per_tick), (4, 46.0, 17.5));
    assert!((impact / total - 0.4).abs() < 0.01, "{impact} of {total}");
    assert!((total / a.cooldown - 11.6).abs() < 0.01);
}

/// `rain_of_fire` and `hellfire` keep their damage a second to each enemy:
/// `rain_of_fire` 102 over 4 ticks every 12 s (8.5 a second), `hellfire` three novas of 13.5 every
/// 6 s (about 6.7 a second).
#[test]
fn rain_of_fire_and_hellfire_keep_their_damage_a_second_to_each_enemy() {
    let c = content();
    let inst = Instance::default();
    let rain = c.ability("rain_of_fire").unwrap();
    let (count, per_tick) = ticks(rain, &inst, hit_damage(rain, &inst, 1.0));
    assert_eq!((count, per_tick, rain.cooldown), (4, 25.5, 12.0));
    assert!((count as f32 * per_tick / rain.cooldown - 8.5).abs() < 0.01);
    let hellfire = c.ability("hellfire").unwrap();
    let (novas, every) = nova_repeats(hellfire, &inst);
    assert_eq!((novas, every, hit_damage(hellfire, &inst, 1.0), hellfire.cooldown), (3, 1.0, 13.5, 6.0));
    assert!((novas as f32 * 13.5 / hellfire.cooldown - 6.75).abs() < 0.01);
}
