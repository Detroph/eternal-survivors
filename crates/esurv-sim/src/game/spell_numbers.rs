//! A spell's numbers as its card describes them: what one cast does for a hero, at the spell's
//! rank and multipliers, worked out by the casting rules themselves (docs/game/screens.md, "What a
//! card says"). Each number has the name a description template asks for; how each reads is in
//! `esurv_content::text::SPELL_NUMBERS`. A spell has only the numbers that fit what it does.

use esurv_content::{Ability, Delivery, SpellEffect, Upgrade};

use super::abilities::{
    burn_ticks, duration, heal_amount, hit_damage, impact_damage, nova_repeats, radius, shield_amount, target_count, ticks,
    Instance, BUFF_ENEMY_RANGE, FINISHER_BELOW, FINISHER_MULT, HEAL_BELOW, IMMUNITY_BELOW, TOTEM_ENEMY_RANGE,
};
use super::casting::{BOUNCE_FALLOFF, HOLD_BOSS_SHARE, HOLD_CHAIN, TOTEM_FIRST_PULSE, TOTEM_PULSE};
use super::choices::{damaging, head_start};
use super::offers::{spell_card_effect, Rarity, SpellCardEffect};
use super::spells::{cooldown, SpellModifier, SpellSlot};
use super::stats::StatSheet;
use super::upgrades::{step, SpellUpgrades};

/// The numbers of `a` cast as `inst` by a hero with `stats`, by name:
///
/// * `damage`: one hit (for a lasting area or an effect over time, all its ticks together);
///   `tick`, `ticks` and `every` (seconds apart) for those; a totem's `damage` is one pulse's,
///   with `pulses` of them `every` seconds once an enemy is within `totem_range`; a nova that
///   repeats around the hero: `damage` is one nova's, with `novas` of them `every` seconds;
/// * `low_damage` on enemies below `low_health` (finishers);
/// * `burn`: what a hit's burn deals in all on each enemy it lands on, over `burn_for` seconds;
/// * `impact`: what a ground area deals at once to each enemy inside it as it lands;
/// * `targets`, `range`, `radius` (a cone's length), `duration`, `cooldown` (seconds between
///   casts), `falloff` (a bouncing chain's loss per jump);
/// * what it does to what it hits: `slow` for `slow_for`, `hold` (a stun or root; `boss_hold` on
///   bosses), `fear`, `leech` (a share of the damage heals the hero), `heal` and `shield` (shares
///   of max health), `weaken` for `weaken_for`;
/// * on the hero: `heal` (below `heal_below` health), `shield`, `buff` (more damage) or `haste`
///   (faster cooldowns) once an enemy is within `buff_range`, immunity below `immune_below`
///   health, `aura` (the damage it strikes back with).
///
/// A totem's pulses carry no on-hit effect in play, so they have none of those numbers. An effect
/// over time carries its spell's: `leech` heals from each tick, any other lands as it is applied.
pub fn spell_numbers(a: &Ability, inst: &Instance, stats: &StatSheet) -> Vec<(&'static str, f32)> {
    use Delivery::*;
    let mut out: Vec<(&'static str, f32)> = Vec::new();
    let kind = a.delivery;
    let mut put = |name: &'static str, value: f32| out.push((name, value));
    put("cooldown", cooldown(a, inst, stats.cooldown_factor));
    if a.kit.swing_speed > 0.0 {
        put("swing_speed", a.kit.swing_speed);
    }
    if a.range > 0.0 {
        put("range", a.range);
    }
    let hit = hit_damage(a, inst, stats.damage);
    if a.damage > 0.0 {
        match kind {
            GroundArea | DamageOverTime => {
                let (count, per_tick) = ticks(a, inst, hit);
                put("damage", per_tick * count as f32);
                put("tick", per_tick);
                put("ticks", count as f32);
                put("every", a.duration.max(0.01) / a.ticks.max(1) as f32);
            }
            _ => put("damage", hit),
        }
        if kind == Finisher || a.double_on_low_health {
            put("low_damage", hit * FINISHER_MULT);
            put("low_health", FINISHER_BELOW);
        }
    }
    if let Some(b) = burn_ticks(a, inst, stats.damage) {
        put("burn", b.total());
        put("burn_for", b.seconds());
    }
    if kind == Nova && a.ticks > 1 {
        let (count, every) = nova_repeats(a, inst);
        put("novas", count as f32);
        put("every", every);
    }
    if kind == GroundArea && a.impact > 0.0 {
        put("impact", impact_damage(a, inst, stats.damage));
    }
    let area = matches!(kind, Nova | GroundArea | Bounce | Cone) || (kind == Totem && a.radius > 0.0);
    if area {
        put("radius", radius(a, inst, stats.area));
    }
    if a.targets > 0 && !matches!(kind, Nova | GroundArea | Cone | SelfEffect) && !(kind == Totem && a.radius > 0.0) {
        put("targets", target_count(a, inst, stats.extra_targets) as f32);
    }
    if kind == Bounce {
        put("falloff", 1.0 - BOUNCE_FALLOFF);
    }
    if a.duration > 0.0 {
        put("duration", duration(a, inst));
    }
    if kind == Totem {
        let length = duration(a, inst);
        let pulses = if length >= TOTEM_FIRST_PULSE { ((length - TOTEM_FIRST_PULSE) / TOTEM_PULSE).floor() as u32 + 1 } else { 0 };
        put("every", TOTEM_PULSE);
        put("pulses", pulses as f32);
        put("totem_range", TOTEM_ENEMY_RANGE);
    }
    let on_self = kind == SelfEffect;
    // what it does to what it hits (totem pulses carry none in play; an effect over time heals from
    // each tick, and lands any other effect as it is applied)
    let strikes = !matches!(kind, Totem | SelfEffect);
    let buff = 1.0 + stats.buff_power;
    // heals and shields of the hero's spells grow with its heal and shield power
    let heals = 1.0 + stats.heal_and_shield_power;
    match a.effect {
        SpellEffect::Slow { amount, seconds } if strikes => {
            put("slow", amount);
            put("slow_for", seconds);
        }
        SpellEffect::Root { seconds } | SpellEffect::Stun { seconds } if strikes => {
            put("hold", seconds.min(HOLD_CHAIN));
            put("boss_hold", (seconds * HOLD_BOSS_SHARE).min(HOLD_CHAIN));
        }
        SpellEffect::Fear { seconds } if strikes => put("fear", seconds),
        SpellEffect::HealFromDamage { share } if strikes => put("leech", share * heals),
        SpellEffect::HealingCharges { charges, share } if strikes => {
            put("charges", charges as f32);
            put("charge_heal", heal_amount(share, inst, 1.0) * heals);
        }
        SpellEffect::Heal { share } if on_self => {
            put("heal", heal_amount(share, inst, 1.0) * heals);
            put("heal_below", a.heal_below.unwrap_or(HEAL_BELOW));
        }
        SpellEffect::Heal { share } if strikes => put("heal", share * heals),
        SpellEffect::Shield { share, .. } if on_self => put("shield", shield_amount(share, inst, 1.0, stats.buff_power) * heals),
        SpellEffect::Shield { share, .. } if strikes => put("shield", share * heals),
        SpellEffect::Weaken { amount, seconds } if strikes => {
            put("weaken", amount);
            put("weaken_for", seconds);
        }
        SpellEffect::DamageBuff { amount, .. } if on_self => {
            put("buff", amount * buff);
            put("buff_range", BUFF_ENEMY_RANGE);
        }
        SpellEffect::HasteBuff { amount, .. } if on_self => {
            put("haste", amount * buff);
            put("buff_range", BUFF_ENEMY_RANGE);
        }
        SpellEffect::Thorns { amount } if on_self => {
            put("aura", inst.damage_scale * inst.power_from_cards * inst.power_from_talents * amount * buff * stats.damage);
            put("buff_range", BUFF_ENEMY_RANGE);
        }
        SpellEffect::SpeedBuff { amount, .. } if on_self => {
            put("speed", amount);
            put("buff_range", BUFF_ENEMY_RANGE);
        }
        SpellEffect::Vulnerable { amount, seconds } if strikes => {
            put("vulnerable", amount);
            put("vulnerable_for", seconds);
        }
        SpellEffect::Immunity { .. } if on_self => put("immune_below", IMMUNITY_BELOW),
        _ => {}
    }
    kit_numbers(a, inst, &mut put);
    out
}

/// The numbers of a spell's `kit` (cost, combo points, its own rule), as `spell_numbers` names them.
fn kit_numbers(a: &Ability, inst: &Instance, put: &mut impl FnMut(&'static str, f32)) {
    use esurv_content::Mechanic;
    let k = &a.kit;
    if k.cost > 0.0 {
        put("cost", k.cost);
    }
    if k.combo_points > 0 {
        put("combo_points", k.combo_points as f32);
    }
    if k.finisher_at > 0 {
        put("finisher_at", k.finisher_at as f32);
    }
    if k.crit_bonus > 0.0 {
        put("crit_bonus", k.crit_bonus);
    }
    let extra = (inst.extra_targets_from_cards + inst.extra_targets_from_talents) as f32;
    match &k.mechanic {
        Some(Mechanic::BlockShield { block_chance, stacks }) => {
            put("buff_range", super::abilities::BUFF_ENEMY_RANGE);
            put("block", *block_chance);
            put("stacks", *stacks as f32 + extra);
        }
        Some(Mechanic::SwingProc { fast_chance, slow_chance, .. }) => {
            put("targets", (a.targets.max(1) as f32 + extra).max(1.0));
            put("chance", *fast_chance);
            put("slow_chance", *slow_chance);
        }
        Some(Mechanic::ExtraSwingHits { chance, hits, .. }) => {
            put("chance", *chance);
            put("hits", *hits as f32 + extra);
        }
        Some(Mechanic::GainEnergy { amount, below }) => {
            put("energy", *amount);
            put("energy_below", *below);
        }
        Some(Mechanic::Summons(s)) => {
            put("count", s.count as f32 + extra);
            // what a spell on the hero calls lasts the spell's own duration, as its cards and
            // talents make it (Friend of the Forest); what an area leaves behind, its own time
            let seconds = if a.delivery == Delivery::SelfEffect { super::abilities::duration(a, inst) } else { s.seconds };
            put("summon_for", seconds);
        }
        Some(Mechanic::ReductionStacks { amount, seconds, per_card, base }) => {
            put("reduction", *amount);
            put("reduction_for", *seconds);
            put("stacks", (*base + *per_card * inst.stacks_from_cards) as f32);
        }
        Some(Mechanic::Delayed { seconds }) => put("delay", *seconds),
        Some(Mechanic::AttacksStrikeMore) => {
            // a buff on the hero: its targets are the enemies each attack strikes more
            put("buff_range", super::abilities::BUFF_ENEMY_RANGE);
            put("targets", a.targets as f32 + extra);
        }
        _ => {}
    }
}

/// The slot a new-spell card of `rarity` gives a hero whose talents and amulet give
/// `modifiers` and whose class has the damage knob `damage_scale`: learned as any spell is, with
/// the card's head start.
pub fn new_spell_slot(a: &Ability, rarity: Rarity, modifiers: &[SpellModifier], damage_scale: f32) -> SpellSlot {
    let mut slot = SpellSlot::new(a, modifiers).with_damage_scale(damage_scale);
    head_start(&mut slot.upgrades, a, rarity);
    slot
}

/// The numbers a new-spell card of `rarity` describes, for a hero with `stats`, `modifiers` and
/// its class's damage knob `damage_scale`.
pub fn new_spell_numbers(
    a: &Ability,
    rarity: Rarity,
    modifiers: &[SpellModifier],
    damage_scale: f32,
    stats: &StatSheet,
) -> Vec<(&'static str, f32)> {
    spell_numbers(a, &new_spell_slot(a, rarity, modifiers, damage_scale).instance(), stats)
}

/// How much more of its main number a spell gets from `by` more power: its damage (or its
/// aura's) grows by `by`; a heal or a shield on the hero grows with the square root of power.
pub fn power_gain(a: &Ability, by: f32) -> f32 {
    let on_self = a.delivery == Delivery::SelfEffect;
    match a.effect {
        SpellEffect::Heal { .. } | SpellEffect::Shield { .. } if on_self => {
            let up = Instance { power_from_cards: 1.0 + by, ..Instance::default() };
            heal_amount(1.0, &up, 1.0) / heal_amount(1.0, &Instance::default(), 1.0) - 1.0
        }
        _ => by,
    }
}

/// What a new-spell card's head start of `rarity` brings, as a card says it: more power (its
/// gain, `power_gain`) or a shorter cooldown (by this share); `None` for a common card.
pub fn head_start_gain(a: &Ability, rarity: Rarity) -> Option<(Upgrade, f32)> {
    match spell_card_effect(rarity, damaging(a)) {
        SpellCardEffect::Power(b) => Some((Upgrade::Power, power_gain(a, b))),
        SpellCardEffect::Cooldown(b) => Some((Upgrade::Cooldown, b)),
        SpellCardEffect::None => None,
    }
}

/// How much one upgrade step of `kind` at `rarity` improves a spell that has taken `u`, as a card
/// says it: power, area and duration grow by this share of what they are, the cooldown shrinks by
/// it, targets rise by it.
pub fn upgrade_gain(a: &Ability, u: &SpellUpgrades, kind: Upgrade, rarity: Rarity) -> f32 {
    let g = step(u, kind, rarity);
    match kind {
        Upgrade::Power => power_gain(a, g),
        Upgrade::Cooldown => g / u.cooldown.max(f32::EPSILON),
        Upgrade::Area => g / u.area.max(f32::EPSILON),
        Upgrade::Duration => g / u.duration.max(f32::EPSILON),
        Upgrade::Targets => g,
        k if k.is_gain() => g / u.gain.max(f32::EPSILON),
        // a stacking rule of the spell's own takes its count per card, whatever the rarity (Maul)
        Upgrade::Stacks => match &a.kit.mechanic {
            Some(esurv_content::Mechanic::ReductionStacks { per_card, .. }) if g > 0.0 => *per_card as f32,
            _ => g,
        },
        _ => g,
    }
}

/// What a known spell's numbers owe to the run's upgrade cards and to the hero, as its tooltip
/// says it. Each total is in a card's terms (`upgrade_gain`'s): power, area and duration as the
/// share their number grew by, the cooldown as the share it shrank by, targets as how many more.
/// A kind is listed only when it moved the spell's numbers (by half a percent, or a target).
#[derive(Debug, Clone, PartialEq)]
pub struct SpellTotals {
    /// What the upgrade cards taken for the spell add up to (the small power each rank brings
    /// counts with its power).
    pub upgrades: Vec<(Upgrade, f32)>,
    /// What the hero's talents, worn gear, jewelry, amulet and race add: the stat sheet's
    /// bonuses (damage, cooldown recovery, area, extra targets, buff power) and the spell's own
    /// modifiers (`SpellMods`); not the level's growth nor a buff of the moment.
    pub bonuses: Vec<(Upgrade, f32)>,
    /// The hero's crit chance and crit damage now, for a spell whose hits can crit.
    pub crit: Option<(f32, f32)>,
}

/// Whether `a`'s hits can crit: every damaging delivery's hits roll for it (`roll_crit`), but an
/// effect over time's ticks and what a spell on the hero does (heals, shields, buffs, an aura's
/// strikes back) never do.
pub fn can_crit(a: &Ability) -> bool {
    a.damage > 0.0 && !matches!(a.delivery, Delivery::DamageOverTime | Delivery::SelfEffect)
}

/// The number a spell's power raises, as `spell_numbers` names it: one tick of what lasts (so a
/// longer duration does not count as power), else its damage, heal, shield or aura.
const POWER_NUMBERS: [&str; 5] = ["tick", "damage", "heal", "shield", "aura"];

/// Every upgrade kind, in a card's order.
const UPGRADE_KINDS: [Upgrade; 5] = [Upgrade::Power, Upgrade::Cooldown, Upgrade::Targets, Upgrade::Area, Upgrade::Duration];

/// How each kind moved the spell from `before` to `after` (both `spell_numbers`), in a card's
/// terms; only the kinds that moved it.
fn moved(before: &[(&'static str, f32)], after: &[(&'static str, f32)]) -> Vec<(Upgrade, f32)> {
    let get = |n: &[(&'static str, f32)], name: &str| n.iter().find(|(k, _)| *k == name).map(|(_, v)| *v);
    let ratio = |name: &str| match (get(before, name), get(after, name)) {
        (Some(b), Some(x)) if b > 0.0 => Some(x / b),
        _ => None,
    };
    let mut out = Vec::new();
    for kind in UPGRADE_KINDS {
        let by = match kind {
            Upgrade::Power => POWER_NUMBERS.iter().find_map(|n| ratio(n)).map(|r| r - 1.0),
            Upgrade::Cooldown => ratio("cooldown").map(|r| 1.0 - r),
            Upgrade::Area => ratio("radius").map(|r| r - 1.0),
            Upgrade::Duration => ratio("duration").map(|r| r - 1.0),
            Upgrade::Targets => match (get(before, "targets"), get(after, "targets")) {
                (Some(b), Some(x)) => Some(x - b),
                _ => None,
            },
            _ => None,
        };
        let shown = match kind {
            Upgrade::Targets => 1.0,
            _ => 0.005,
        };
        if let Some(by) = by.filter(|b| *b >= shown) {
            out.push((kind, by));
        }
    }
    out
}

/// The totals of `slot`'s spell `a` for a hero whose stat sheet is `plain` without any bonus
/// (`stats::compute` with no modifiers, at its class and level), `bonused` with its talents, gear,
/// jewelry, amulet and race (its modifiers, no buff) and `now` as it stands (for the crit), each
/// worked out by `spell_numbers` on the slot with and without what is being counted.
pub fn spell_totals(a: &Ability, slot: &SpellSlot, plain: &StatSheet, bonused: &StatSheet, now: &StatSheet) -> SpellTotals {
    let all = slot.instance();
    // the slot with the cards' upgrades only (no talent or amulet modifier), and with neither
    let cards = Instance {
        power_from_talents: 1.0,
        cooldown_from_talents: 1.0,
        radius_from_talents: 1.0,
        duration_from_talents: 1.0,
        extra_targets_from_talents: 0,
        ..all
    };
    let bare = Instance { damage_scale: all.damage_scale, ..Instance::default() };
    let numbers = |inst: &Instance, stats: &StatSheet| spell_numbers(a, inst, stats);
    SpellTotals {
        upgrades: moved(&numbers(&bare, plain), &numbers(&cards, plain)),
        bonuses: moved(&numbers(&cards, plain), &numbers(&all, bonused)),
        crit: can_crit(a).then_some((now.crit_chance, now.crit_damage)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::stats::compute;
    use esurv_content::Content;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    fn get<'a>(n: &'a [(&'static str, f32)], name: &str) -> Option<f32> {
        n.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
    }

    #[test]
    fn a_projectile_says_its_hit_targets_range_and_hold() {
        let c = content();
        let hammer = c.ability("hammer_of_justice").unwrap();
        let stats = compute(c.class("paladin").unwrap(), 1, &[]);
        let n = spell_numbers(hammer, &SpellSlot::new(hammer, &[]).instance(), &stats);
        assert!((get(&n, "damage").unwrap() - hammer.damage).abs() < 1e-3);
        assert_eq!(get(&n, "targets"), Some(hammer.targets as f32));
        assert_eq!(get(&n, "range"), Some(hammer.range));
        assert_eq!(get(&n, "hold"), Some(3.0));
        assert_eq!(get(&n, "boss_hold"), Some(0.75));
        assert_eq!(get(&n, "radius"), None, "a projectile has no area");
    }

    #[test]
    fn effects_over_time_count_their_ticks_and_heal_from_them() {
        let c = content();
        let drain = c.ability("drain_life").unwrap();
        let stats = compute(c.class("warlock").unwrap(), 1, &[]);
        let inst = SpellSlot::new(drain, &[]).instance();
        let n = spell_numbers(drain, &inst, &stats);
        let (count, per_tick) = ticks(drain, &inst, hit_damage(drain, &inst, 1.0));
        assert_eq!(get(&n, "ticks"), Some(count as f32));
        assert!((get(&n, "damage").unwrap() - per_tick * count as f32).abs() < 1e-3);
        assert_eq!(get(&n, "leech"), Some(0.6), "each tick heals a share of its damage");
    }

    #[test]
    fn a_totem_counts_its_pulses() {
        let c = content();
        let magma = c.ability("magma_totem").unwrap();
        let stats = compute(c.class("shaman").unwrap(), 1, &[]);
        let n = spell_numbers(magma, &SpellSlot::new(magma, &[]).instance(), &stats);
        // 8 s: pulses at 0.5, 2.5, 4.5, 6.5
        assert_eq!(get(&n, "pulses"), Some(4.0));
        assert_eq!(get(&n, "every"), Some(TOTEM_PULSE));
        assert!(get(&n, "radius").is_some() && get(&n, "targets").is_none());
    }

    #[test]
    fn a_head_start_shows_in_the_numbers() {
        let c = content();
        let fireball = c.ability("fireball").unwrap();
        let stats = compute(c.class("mage").unwrap(), 1, &[]);
        let plain = new_spell_numbers(fireball, Rarity::Common, &[], 1.0, &stats);
        let epic = new_spell_numbers(fireball, Rarity::Epic, &[], 1.0, &stats);
        let ratio = get(&epic, "damage").unwrap() / get(&plain, "damage").unwrap();
        assert!((ratio - 1.35).abs() < 1e-4);
        assert_eq!(head_start_gain(fireball, Rarity::Common), None);
        assert_eq!(head_start_gain(fireball, Rarity::Epic), Some((Upgrade::Power, 0.35)));
        // a heal grows with the square root of its power
        let heal = c.ability("holy_light").unwrap();
        let (kind, gain) = head_start_gain(heal, Rarity::Epic).unwrap();
        assert_eq!(kind, Upgrade::Power);
        assert!((gain - (1.35f32.sqrt() - 1.0)).abs() < 1e-4);
    }

    #[test]
    fn upgrade_gains_are_shares_of_what_the_spell_has() {
        let c = content();
        let fireball = c.ability("fireball").unwrap();
        let u = SpellUpgrades { cooldown: 0.6, ..SpellUpgrades::default() };
        let g = step(&u, Upgrade::Cooldown, Rarity::Rare);
        assert!((upgrade_gain(fireball, &u, Upgrade::Cooldown, Rarity::Rare) - g / 0.6).abs() < 1e-6);
        assert_eq!(upgrade_gain(fireball, &u, Upgrade::Power, Rarity::Rare), step(&u, Upgrade::Power, Rarity::Rare));
    }

    fn kind(list: &[(Upgrade, f32)], k: Upgrade) -> Option<f32> {
        list.iter().find(|(x, _)| *x == k).map(|(_, v)| *v)
    }

    #[test]
    fn a_fresh_spell_owes_nothing_and_its_hits_can_crit() {
        let c = content();
        let mage = c.class("mage").unwrap();
        let stats = compute(mage, 4, &[]);
        let fireball = c.ability("fireball").unwrap();
        let t = spell_totals(fireball, &SpellSlot::new(fireball, &[]), &stats, &stats, &stats);
        assert!(t.upgrades.is_empty() && t.bonuses.is_empty(), "{t:?}");
        assert_eq!(t.crit, Some((stats.crit_chance, stats.crit_damage)));
        // what lasts ticks, and what a spell does on the hero, never crits
        assert!(!can_crit(c.ability("drain_life").unwrap()));
        assert!(!can_crit(c.ability("renew").unwrap()));
        assert!(can_crit(c.ability("consecration").unwrap()), "a lasting area's ticks roll");
    }

    #[test]
    fn the_cards_taken_add_up_by_kind() {
        let c = content();
        let stats = compute(c.class("paladin").unwrap(), 1, &[]);
        let area = c.ability("consecration").unwrap();
        let mut slot = SpellSlot::new(area, &[]);
        crate::game::upgrades::apply(&mut slot.upgrades, &[Upgrade::Area], Rarity::Rare);
        crate::game::upgrades::apply(&mut slot.upgrades, &[Upgrade::Power], Rarity::Rare);
        let t = spell_totals(area, &slot, &stats, &stats, &stats);
        let u = slot.upgrades;
        assert!((kind(&t.upgrades, Upgrade::Area).unwrap() - (u.area - 1.0)).abs() < 1e-4, "{t:?}");
        assert!((kind(&t.upgrades, Upgrade::Power).unwrap() - (u.power - 1.0)).abs() < 1e-4, "{t:?}");
        assert_eq!(kind(&t.upgrades, Upgrade::Cooldown), None, "no cooldown card taken");
        assert!(t.bonuses.is_empty(), "the hero adds nothing: {t:?}");
    }

    #[test]
    fn the_heros_bonuses_count_the_stat_sheet_and_the_spells_own_modifiers() {
        use crate::game::stats::Modifier;
        use esurv_content::{SpellFilter, Stat};
        let c = content();
        let mage = c.class("mage").unwrap();
        let fireball = c.ability("fireball").unwrap();
        let modifiers = [Modifier::new(Stat::Damage, 0.2), Modifier::new(Stat::CooldownRecovery, 0.1)];
        let plain = compute(mage, 1, &[]);
        let bonused = compute(mage, 1, &modifiers);
        let spell_mods = [SpellModifier { filter: SpellFilter::Spell("fireball".into()), kind: Upgrade::Power, value: 0.1, seconds: false }];
        let slot = SpellSlot::new(fireball, &spell_mods);
        let t = spell_totals(fireball, &slot, &plain, &bonused, &bonused);
        // damage 1.2 × the talent's 1.1; cooldowns 1 / 1.1 as long
        assert!((kind(&t.bonuses, Upgrade::Power).unwrap() - (1.2 * 1.1 - 1.0)).abs() < 1e-4, "{t:?}");
        assert!((kind(&t.bonuses, Upgrade::Cooldown).unwrap() - (1.0 - 1.0 / 1.1)).abs() < 1e-4, "{t:?}");
        assert_eq!(kind(&t.bonuses, Upgrade::Area), None, "a projectile has no area");
        assert!(t.upgrades.is_empty());
    }

    /// The starting options' timed spells, at level 1, rank 1 and no gear,
    /// deal their budget: about 40 a second (Arcane Blast 30, its passive lifting the others).
    #[test]
    fn the_starting_spells_play_their_numbers() {
        let c = content();
        // (spell, cooldown, targets, damage per hit before crit, damage a second)
        let table = [
            ("steady_shot", 2.0, 1, 80.0, 40.0),
            ("smite", 3.0, 1, 120.0, 40.0),
            ("mind_spike", 2.0, 1, 80.0, 40.0),
            ("fireball", 3.0, 1, 120.0, 40.0),
            ("frostbolt", 3.0, 1, 120.0, 40.0),
            ("arcane_blast", 3.0, 1, 90.0, 30.0),
            ("shadow_bolt", 3.0, 1, 120.0, 40.0),
            ("lightning_bolt", 2.0, 2, 40.0, 40.0),
            ("wellspring", 4.0, 3, 53.0, 39.75),
            ("starfire", 4.0, 1, 160.0, 40.0),
        ];
        let mut seen = 0;
        for class in &c.classes {
            for o in &class.starting_options {
                let Some(&(_, want_cd, targets, _, _)) = table.iter().find(|t| t.0 == o.main_spell()) else { continue };
                seen += 1;
                let a = c.ability(o.main_spell()).unwrap();
                // the hit is the content's (balance passes move it); what is shown must match it
                let hit = a.damage * class.spell_damage_scale;
                let want = hit * targets as f32 / want_cd;
                let stats = compute(class, 1, &[]);
                let n = new_spell_numbers(a, Rarity::Common, &[], class.spell_damage_scale, &stats);
                let slot = new_spell_slot(a, Rarity::Common, &[], class.spell_damage_scale);
                let cd = get(&n, "cooldown").unwrap();
                assert!((cd - cooldown(a, &slot.instance(), stats.cooldown_factor)).abs() < 1e-6);
                let damage = get(&n, "damage").unwrap();
                let shown_targets = get(&n, "targets").unwrap_or(1.0);
                assert!((cd - want_cd).abs() < 1e-5, "{}: cooldown {cd}", a.id);
                assert!((damage - hit).abs() < 1e-3, "{}: hit {damage}", a.id);
                assert_eq!(shown_targets, targets as f32, "{}", a.id);
                let dps = damage * shown_targets / cd;
                assert!((dps - want).abs() < 0.1, "{}: {dps} a second", a.id);
            }
        }
        assert!(seen >= 4, "{seen}");
    }

    /// The content's spells (and pets) with the spell tag
    /// `coefficients` applied on top of the shipped ones (all 1).
    fn retagged(c: &Content, coefficients: esurv_content::SpellTagCoefficients) -> (Vec<Ability>, Vec<esurv_content::Pet>) {
        let (mut spells, mut pets) = (c.abilities.clone(), c.pets.clone());
        esurv_content::apply_spell_tags(&mut spells, &mut pets, &coefficients).unwrap();
        (spells, pets)
    }

    fn by_id<'a>(spells: &'a [Ability], id: &str) -> &'a Ability {
        spells.iter().find(|a| a.id == id).unwrap()
    }

    /// The numbers `id`'s card shows in `c`'s class `class`, before and after re-tagging.
    fn numbers_both(c: &Content, spells: &[Ability], class: &str, id: &str) -> [Vec<(&'static str, f32)>; 2] {
        let stats = compute(c.class(class).unwrap(), 1, &[]);
        [c.ability(id).unwrap(), by_id(spells, id)].map(|a| spell_numbers(a, &SpellSlot::new(a, &[]).instance(), &stats))
    }

    fn ratio(n: &[Vec<(&'static str, f32)>; 2], name: &str) -> f32 {
        get(&n[1], name).unwrap() / get(&n[0], name).unwrap()
    }

    /// Cooldown, duration and tick counts never move with a coefficient.
    fn times_unchanged(n: &[Vec<(&'static str, f32)>; 2]) {
        for name in ["cooldown", "duration", "ticks", "every", "slow_for", "weaken_for", "pulses"] {
            assert_eq!(get(&n[0], name), get(&n[1], name), "{name}");
        }
    }

    #[test]
    fn a_damage_tag_coefficient_scales_only_its_spells_damage() {
        use esurv_content::SpellTagCoefficients as K;
        let c = content();
        let inst = Instance::default();
        for (k, spell, other, class) in [
            (K { nuke: 2.0, ..K::default() }, "pyroblast", "fireball", "mage"),
            (K { level_1: 2.0, ..K::default() }, "fireball", "pyroblast", "mage"),
            (K { cleave: 2.0, ..K::default() }, "multi_shot", "volley", "hunter"),
            (K { dot: 2.0, ..K::default() }, "corruption", "shadow_bolt", "warlock"),
            (K { nova: 2.0, ..K::default() }, "frost_nova", "blizzard", "mage"),
            (K { area: 2.0, ..K::default() }, "blizzard", "frost_nova", "mage"),
            (K { pet: 2.0, ..K::default() }, "magma_totem", "fire_nova", "shaman"),
        ] {
            let (spells, _) = retagged(&c, k);
            let (base, now) = (c.ability(spell).unwrap(), by_id(&spells, spell));
            let hit = hit_damage(now, &inst, 1.0) / hit_damage(base, &inst, 1.0);
            assert!((hit - 2.0).abs() < 1e-5, "{spell}: hit × {hit}");
            let n = numbers_both(&c, &spells, class, spell);
            assert!((ratio(&n, "damage") - 2.0).abs() < 1e-5, "{spell}: the card's damage doubles too");
            times_unchanged(&n);
            assert_eq!(base.effect, now.effect, "{spell}: its effect stays");
            assert_eq!(by_id(&spells, other).damage, c.ability(other).unwrap().damage, "{other} has another damage tag");
        }
        // the class knob multiplies with the coefficient
        let (spells, _) = retagged(&c, K { nuke: 2.0, ..K::default() });
        let k = Instance { damage_scale: 1.5, ..Default::default() };
        let base = hit_damage(c.ability("pyroblast").unwrap(), &inst, 1.0);
        assert!((hit_damage(by_id(&spells, "pyroblast"), &k, 1.0) - 3.0 * base).abs() < 1e-2);
    }

    #[test]
    fn hellfire_shows_one_nova_s_damage_how_many_novas_and_how_far_apart() {
        let c = content();
        let hellfire = c.ability("hellfire").unwrap();
        let stats = compute(c.class("warlock").unwrap(), 1, &[]);
        let n = spell_numbers(hellfire, &Instance::default(), &stats);
        assert!((get(&n, "damage").unwrap() - 13.5 * stats.damage).abs() < 1e-2, "{n:?}");
        assert_eq!((get(&n, "novas"), get(&n, "every"), get(&n, "tick")), (Some(3.0), Some(1.0), None));
        let longer = spell_numbers(hellfire, &Instance { duration_from_cards: 1.5, ..Default::default() }, &stats);
        assert_eq!((get(&longer, "novas"), get(&longer, "every")), (Some(4.0), Some(1.0)));
        // an ordinary nova shows neither
        assert_eq!(get(&spell_numbers(c.ability("frost_nova").unwrap(), &Instance::default(), &stats), "novas"), None);
    }

    #[test]
    fn flamestrike_shows_its_impact_and_a_tag_coefficient_doubles_it_with_the_ticks() {
        let c = content();
        let (spells, _) = retagged(&c, esurv_content::SpellTagCoefficients { area: 2.0, ..Default::default() });
        let n = numbers_both(&c, &spells, "mage", "flamestrike");
        let d = compute(c.class("mage").unwrap(), 1, &[]).damage;
        assert!((get(&n[0], "impact").unwrap() - 46.0 * d).abs() < 1e-2, "{:?}", n[0]);
        assert!((get(&n[0], "damage").unwrap() - 70.0 * d).abs() < 1e-2, "{:?}", n[0]);
        assert_eq!((get(&n[0], "ticks"), get(&n[0], "duration")), (Some(4.0), Some(8.0)));
        assert!((ratio(&n, "impact") - 2.0).abs() < 1e-5 && (ratio(&n, "damage") - 2.0).abs() < 1e-5);
        times_unchanged(&n);
        // a ground area without an impact shows none
        let blizzard = c.ability("blizzard").unwrap();
        let stats = compute(c.class("mage").unwrap(), 1, &[]);
        assert_eq!(get(&spell_numbers(blizzard, &Instance::default(), &stats), "impact"), None);
    }

    #[test]
    fn the_pet_coefficient_also_scales_every_talent_pet() {
        let c = content();
        let (_, pets) = retagged(&c, esurv_content::SpellTagCoefficients { pet: 2.0, ..Default::default() });
        for (before, after) in c.pets.iter().zip(&pets) {
            assert_eq!((after.damage, after.swing), (before.damage * 2.0, before.swing), "{}", before.name);
        }
    }

    #[test]
    fn sustain_scales_every_kind_of_healing_and_nothing_else() {
        let c = content();
        let (spells, _) = retagged(&c, esurv_content::SpellTagCoefficients { sustain: 2.0, ..Default::default() });
        let base = c.ability("holy_light").unwrap();
        let SpellEffect::Heal { share: was } = base.effect else { panic!("holy light heals") };
        let SpellEffect::Heal { share: now } = by_id(&spells, "holy_light").effect else { panic!("holy light heals") };
        let inst = Instance::default();
        assert!((heal_amount(now, &inst, 100.0) / heal_amount(was, &inst, 100.0) - 2.0).abs() < 1e-5);
        for (spell, class, number) in [
            ("holy_light", "paladin", "heal"),
            ("renew", "priest", "heal"),
            ("holy_shock", "paladin", "heal"),
            ("bloodthirst", "warrior", "charge_heal"),
            ("drain_life", "warlock", "leech"),
            ("death_coil", "warlock", "leech"),
        ] {
            let n = numbers_both(&c, &spells, class, spell);
            assert!((ratio(&n, number) - 2.0).abs() < 1e-5, "{spell}: {number}");
            if get(&n[0], "damage").is_some() {
                assert_eq!(ratio(&n, "damage"), 1.0, "{spell}: Sustain leaves damage alone");
            }
            times_unchanged(&n);
        }
    }

    #[test]
    fn buffs_scale_the_buff_amount() {
        let c = content();
        let (spells, _) = retagged(&c, esurv_content::SpellTagCoefficients { buffs: 2.0, ..Default::default() });
        for (spell, class, number) in [("battle_shout", "warrior", "buff"), ("adrenaline_rush", "rogue", "haste")] {
            let n = numbers_both(&c, &spells, class, spell);
            assert!((ratio(&n, number) - 2.0).abs() < 1e-5, "{spell}: {number}");
            times_unchanged(&n);
        }
    }

    #[test]
    fn defensive_scales_shields_weakening_and_reflected_damage() {
        let c = content();
        let (spells, _) = retagged(&c, esurv_content::SpellTagCoefficients { defensive: 2.0, ..Default::default() });
        let SpellEffect::Shield { share: was, seconds } = c.ability("pw_shield").unwrap().effect else { panic!("a shield") };
        let SpellEffect::Shield { share: now, seconds: now_seconds } = by_id(&spells, "pw_shield").effect else { panic!("a shield") };
        let inst = Instance::default();
        assert!((shield_amount(now, &inst, 100.0, 0.0) / shield_amount(was, &inst, 100.0, 0.0) - 2.0).abs() < 1e-5);
        assert_eq!(seconds, now_seconds, "the shield lasts as long");
        for (spell, class, number) in [
            ("pw_shield", "priest", "shield"),
            ("ice_barrier", "mage", "shield"),
            ("ghostly_strike", "rogue", "shield"),
            ("demo_shout", "warrior", "weaken"),
            ("insect_swarm", "druid", "weaken"),
            ("thorns", "druid", "aura"),
        ] {
            let n = numbers_both(&c, &spells, class, spell);
            assert!((ratio(&n, number) - 2.0).abs() < 1e-5, "{spell}: {number}");
            if get(&n[0], "damage").is_some() {
                assert_eq!(ratio(&n, "damage"), 1.0, "{spell}: Defensive leaves damage alone");
            }
            times_unchanged(&n);
        }
        // immunity has no amount to scale
        assert_eq!(by_id(&spells, "divine_shield").effect, c.ability("divine_shield").unwrap().effect);
    }
}
