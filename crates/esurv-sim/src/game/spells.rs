//! The hero's spell slots: the spells learned from cards, plus the race's spell in a slot of its
//! own. Each slot casts by itself when its timer runs out.
//!
//! A slot carries two sets of multipliers: what upgrade cards gave it (`SpellUpgrades`), and what
//! talents and the amulet give it (`SpellMods`, worked out again whenever the hero's modifiers
//! change).

use esurv_content::{Ability, SpellFilter, TalentEffect, Upgrade};

use super::abilities::Instance;
use super::upgrades::SpellUpgrades;

/// A newly learned spell (and the race's spell at run start) first casts after this many seconds.
pub const FIRST_CAST: f32 = 0.5;
/// No spell recasts faster than this: a hard cap on cooldown cuts.
pub const MIN_COOLDOWN: f32 = 1.0;
/// A learned spell that found nothing to cast on tries again after this many seconds.
pub const RETRY: f32 = 0.2;
/// The race's spell tries again after this many seconds.
pub const RACIAL_RETRY: f32 = 0.5;
/// Talent and amulet cooldown cuts stop here (at most 60 % shorter).
pub const MOD_COOLDOWN_FLOOR: f32 = 0.4;

/// One spell modifier on the hero: `value` added to one number of every spell `filter` lets
/// through (a talent's or amulet's `AbilityMod`, already multiplied by its rank).
#[derive(Debug, Clone, PartialEq)]
pub struct SpellModifier {
    pub filter: SpellFilter,
    pub kind: Upgrade,
    pub value: f32,
}

impl SpellModifier {
    /// The spell modifier a talent effect at `rank` gives (amulet effects: rank 1, after
    /// empowering), if it is one.
    pub fn from_effect(effect: &TalentEffect, rank: u32) -> Option<Self> {
        match effect {
            TalentEffect::AbilityMod { filter, kind, per_rank } => {
                Some(Self { filter: filter.clone(), kind: *kind, value: per_rank * rank as f32 })
            }
            _ => None,
        }
    }
}

/// What talents and the amulet add to one spell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpellMods {
    pub power: f32,
    pub cooldown: f32,
    pub targets: u32,
    pub area: f32,
    pub duration: f32,
    /// Rage gained, energy refilled or proc chance, × this (`RageGain`, `EnergyGain`, `Chance`).
    pub gain: f32,
}

impl Default for SpellMods {
    fn default() -> Self {
        Self { power: 1.0, cooldown: 1.0, targets: 0, area: 1.0, duration: 1.0, gain: 1.0 }
    }
}

/// Sums the modifiers that reach `spell`: power, area and duration `1 + Σ`, cooldown
/// `max(0.4, 1 − Σ)`, targets `Σ` rounded, and the gains `1 + Σ`. The kinds that add whole units
/// (stacks, strikes, imps, pets, critters) add to the targets, as their cards do.
pub fn spell_mods(modifiers: &[SpellModifier], spell: &Ability) -> SpellMods {
    let mut sum = [0.0f32; Upgrade::ALL.len()];
    for m in modifiers.iter().filter(|m| m.filter.matches(spell)) {
        sum[m.kind as usize] += m.value;
    }
    let counts: f32 = Upgrade::ALL.iter().filter(|k| k.is_count()).map(|k| sum[*k as usize]).sum();
    let gains: f32 = Upgrade::ALL.iter().filter(|k| k.is_gain()).map(|k| sum[*k as usize]).sum();
    SpellMods {
        power: 1.0 + sum[Upgrade::Power as usize],
        cooldown: (1.0 - sum[Upgrade::Cooldown as usize]).max(MOD_COOLDOWN_FLOOR),
        targets: counts.round().max(0.0) as u32,
        area: 1.0 + sum[Upgrade::Area as usize],
        duration: 1.0 + sum[Upgrade::Duration as usize],
        gain: (1.0 + gains).max(0.0),
    }
}

/// One spell the hero casts.
#[derive(Debug, Clone, PartialEq)]
pub struct SpellSlot {
    /// The spell's id in the content.
    pub spell: String,
    pub upgrades: SpellUpgrades,
    pub mods: SpellMods,
    /// Seconds until it casts.
    pub timer: f32,
    /// The class's damage knob (`Class::spell_damage_scale`; 1 in the race slot).
    pub damage_scale: f32,
}

impl SpellSlot {
    /// A spell just learned (or the race's spell at run start), at damage scale 1; class spells
    /// take their class's knob through `with_damage_scale`.
    pub fn new(spell: &Ability, modifiers: &[SpellModifier]) -> Self {
        Self {
            spell: spell.id.clone(),
            upgrades: SpellUpgrades::default(),
            mods: spell_mods(modifiers, spell),
            timer: FIRST_CAST,
            damage_scale: 1.0,
        }
    }

    /// The race spell's slot: it plays exactly as its tooltip says (no class damage knob).
    pub fn racial(spell: &Ability, modifiers: &[SpellModifier]) -> Self {
        Self::new(spell, modifiers)
    }

    /// The slot with its class's damage knob.
    pub fn with_damage_scale(self, damage_scale: f32) -> Self {
        Self { damage_scale, ..self }
    }

    /// Both multiplier sets together, as the casting formulas take them.
    pub fn instance(&self) -> Instance {
        Instance {
            damage_scale: self.damage_scale,
            power_from_cards: self.upgrades.power,
            power_from_talents: self.mods.power,
            cooldown_from_cards: self.upgrades.cooldown,
            cooldown_from_talents: self.mods.cooldown,
            radius_from_cards: self.upgrades.area,
            radius_from_talents: self.mods.area,
            duration_from_cards: self.upgrades.duration,
            duration_from_talents: self.mods.duration,
            extra_targets_from_cards: self.upgrades.extra_targets,
            extra_targets_from_talents: self.mods.targets,
            stacks_from_cards: self.upgrades.taken[Upgrade::Stacks as usize],
            stack_targets_from_cards: self.upgrades.extra_stacks,
        }
    }

    /// Counts the timer down; true when the spell should be cast now.
    pub fn ready(&mut self, dt: f32) -> bool {
        self.timer -= dt;
        self.timer <= 0.0
    }

    /// Restarts the timer after a cast attempt: the full cooldown if it was cast, else `retry`.
    pub fn after_cast(&mut self, spell: &Ability, cast: bool, hero_cooldown_factor: f32, retry: f32) {
        self.timer = if cast { cooldown(spell, &self.instance(), hero_cooldown_factor) } else { retry };
    }
}

/// Seconds between casts: listed cooldown × card cut × talent/amulet cut × the hero's cooldown factor, never under [`MIN_COOLDOWN`].
pub fn cooldown(spell: &Ability, inst: &Instance, hero_cooldown_factor: f32) -> f32 {
    (spell.cooldown * inst.cooldown_from_cards * inst.cooldown_from_talents * hero_cooldown_factor).max(MIN_COOLDOWN)
}

/// The hero's spells.
#[derive(Debug, Clone, PartialEq)]
pub struct Spellbook {
    /// Learned from cards (the class's starting spell first), at most `upgrades::MAX_SPELLS`.
    pub known: Vec<SpellSlot>,
    /// The race's own spell, cast like the others but never upgraded by cards.
    pub racial: Option<SpellSlot>,
    /// The class's damage knob (`Class::spell_damage_scale`), given to every class spell slot.
    pub damage_scale: f32,
}

impl Default for Spellbook {
    fn default() -> Self {
        Self { known: Vec::new(), racial: None, damage_scale: 1.0 }
    }
}

impl Spellbook {
    /// A run's opening spells: the class's starting spell, and the race spell in its own slot.
    /// `damage_scale` is the class's damage knob; the race spell never takes it.
    pub fn start(starting: &Ability, race_spell: &Ability, modifiers: &[SpellModifier], damage_scale: f32) -> Self {
        Self {
            known: vec![SpellSlot::new(starting, modifiers).with_damage_scale(damage_scale)],
            racial: Some(SpellSlot::racial(race_spell, modifiers)),
            damage_scale,
        }
    }

    pub fn knows(&self, id: &str) -> bool {
        self.known.iter().any(|s| s.spell == id)
    }

    pub fn learn(&mut self, spell: &Ability, modifiers: &[SpellModifier]) {
        if !self.knows(&spell.id) {
            self.known.push(SpellSlot::new(spell, modifiers).with_damage_scale(self.damage_scale));
        }
    }

    /// Works the talent and amulet multipliers out again for every slot (after the hero's
    /// modifiers change). `spell` looks a slot's spell up by id.
    pub fn refresh_mods<'a>(&mut self, modifiers: &[SpellModifier], spell: impl Fn(&str) -> Option<&'a Ability>) {
        for s in self.known.iter_mut().chain(self.racial.iter_mut()) {
            if let Some(a) = spell(&s.spell) {
                s.mods = spell_mods(modifiers, a);
            }
        }
    }

    /// How `spell` is cast when something casts it for free: as its slot if the hero knows it,
    /// else as a rank-1 spell whose card power is the average of the known spells' (at least 1).
    pub fn free_cast(&self, spell: &Ability, modifiers: &[SpellModifier]) -> Instance {
        if let Some(s) = self.known.iter().find(|s| s.spell == spell.id) {
            return s.instance();
        }
        let mut slot = SpellSlot::new(spell, modifiers).with_damage_scale(self.damage_scale);
        if !self.known.is_empty() {
            let mean = self.known.iter().map(|s| s.upgrades.power).sum::<f32>() / self.known.len() as f32;
            slot.upgrades.power = mean.max(1.0);
        }
        slot.instance()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_content::{Delivery, SpellEffect, School, SpellCategory};

    fn spell(id: &str, school: School, delivery: Delivery, cooldown: f32) -> Ability {
        Ability {
            id: id.into(),
            name_ref: Default::default(),
            name: String::new(),
            description: String::new(),
            icon: String::new(),
            look_spell: 0,
            school,
            delivery,
            categories: vec![SpellCategory::DirectDamage],
            tags: vec![],
            upgrades: vec![],
            free_casts_can_proc: false,
            damage: 10.0,
            cooldown,
            range: 30.0,
            radius: 0.0,
            targets: 1,
            duration: 0.0,
            ticks: 0,
            pulse_spell: None,
            effect: SpellEffect::None,
            heal_below: None,
            double_on_low_health: false,
            burn: None,
            impact: 0.0,
            kit: Default::default(),
        }
    }

    fn m(filter: SpellFilter, kind: Upgrade, value: f32) -> SpellModifier {
        SpellModifier { filter, kind, value }
    }

    #[test]
    fn modifiers_reach_spells_by_filter() {
        let fireball = spell("fireball", School::Fire, Delivery::Projectile, 2.0);
        let mods = [
            m(SpellFilter::School(School::Fire), Upgrade::Power, 0.08),
            m(SpellFilter::Spell("fireball".into()), Upgrade::Power, 0.1),
            m(SpellFilter::School(School::Frost), Upgrade::Power, 5.0),
            m(SpellFilter::Delivery(Delivery::Projectile), Upgrade::Targets, 0.6),
            m(SpellFilter::Category(SpellCategory::DirectDamage), Upgrade::Area, 0.25),
        ];
        let s = spell_mods(&mods, &fireball);
        assert!((s.power - 1.18).abs() < 1e-6);
        assert_eq!(s.targets, 1); // 0.6 rounds to 1
        assert_eq!(s.area, 1.25);
    }

    #[test]
    fn talent_ranks_multiply_the_modifier() {
        let e = TalentEffect::AbilityMod { filter: SpellFilter::Delivery(Delivery::DamageOverTime), kind: Upgrade::Power, per_rank: 0.1 };
        let got = SpellModifier::from_effect(&e, 3).unwrap();
        assert_eq!((got.filter, got.kind), (SpellFilter::Delivery(Delivery::DamageOverTime), Upgrade::Power));
        assert!((got.value - 0.3).abs() < 1e-6);
        let stat = TalentEffect::Stat { stat: esurv_content::Stat::Damage, per_rank: 0.1 };
        assert_eq!(SpellModifier::from_effect(&stat, 1), None);
    }

    #[test]
    fn cooldown_cuts_from_talents_stop_at_60_percent() {
        let a = spell("a", School::Fire, Delivery::Projectile, 10.0);
        let s = spell_mods(&[m(SpellFilter::Any, Upgrade::Cooldown, 0.5), m(SpellFilter::Any, Upgrade::Cooldown, 0.3)], &a);
        assert_eq!(s.cooldown, 0.4);
    }

    #[test]
    fn cooldown_rule() {
        let a = spell("a", School::Fire, Delivery::Projectile, 10.0);
        let mut slot = SpellSlot::new(&a, &[]);
        assert_eq!(slot.timer, FIRST_CAST);
        // 10 s, halved by the hero's cooldown factor
        slot.after_cast(&a, true, 0.5, RETRY);
        assert!((slot.timer - 5.0).abs() < 1e-6);
        slot.upgrades.cooldown = 0.5;
        slot.mods.cooldown = 0.4;
        slot.after_cast(&a, true, 1.0, RETRY);
        assert!((slot.timer - 2.0).abs() < 1e-6);
        slot.after_cast(&a, true, 0.1, RETRY);
        assert_eq!(slot.timer, MIN_COOLDOWN);
        slot.after_cast(&a, false, 1.0, RETRY);
        assert_eq!(slot.timer, RETRY);
    }

    #[test]
    fn free_casts_of_unknown_spells_use_the_average_card_power() {
        let a = spell("a", School::Fire, Delivery::Projectile, 2.0);
        let b = spell("b", School::Fire, Delivery::Projectile, 2.0);
        let c = spell("c", School::Fire, Delivery::Projectile, 2.0);
        let mut book = Spellbook::default();
        assert_eq!(book.free_cast(&c, &[]).power_from_cards, 1.0);
        book.learn(&a, &[]);
        book.learn(&b, &[]);
        book.known[0].upgrades.power = 2.0;
        book.known[1].upgrades.power = 1.5;
        assert_eq!(book.free_cast(&c, &[]).power_from_cards, 1.75);
        assert_eq!(book.free_cast(&a, &[]).power_from_cards, 2.0);
        book.known[0].upgrades.power = 0.5;
        book.known[1].upgrades.power = 0.5;
        assert_eq!(book.free_cast(&c, &[]).power_from_cards, 1.0);
    }

    #[test]
    fn a_run_opens_with_the_starting_spell_and_the_race_spell() {
        let cleave = spell("cleave", School::Physical, Delivery::Melee, 3.0);
        let stomp = spell("war_stomp", School::Physical, Delivery::Nova, 15.0);
        let book = Spellbook::start(&cleave, &stomp, &[], 1.0);
        assert_eq!(book.known.len(), 1);
        assert!(book.knows("cleave") && !book.knows("war_stomp"));
        let racial = book.racial.as_ref().unwrap();
        assert_eq!((racial.spell.as_str(), racial.timer), ("war_stomp", FIRST_CAST));
    }

    #[test]
    fn race_spells_keep_their_tooltip_numbers() {
        let fireball = spell("fireball", School::Fire, Delivery::Projectile, 2.0);
        let stomp = spell("war_stomp", School::Physical, Delivery::Nova, 15.0);
        let mut book = Spellbook::start(&fireball, &stomp, &[], 1.5);
        let racial = book.racial.as_mut().unwrap();
        racial.after_cast(&stomp, true, 1.0, RACIAL_RETRY);
        assert_eq!(racial.timer, 15.0, "every 15 s, as the tooltip says");
        racial.after_cast(&stomp, true, 0.5, RACIAL_RETRY);
        assert_eq!(racial.timer, 7.5, "cooldown recovery still counts");
        assert_eq!(racial.damage_scale, 1.0, "the class's damage knob never reaches the race spell");
        // the class's damage knob reaches its spells' damage only, never their cooldown
        book.known[0].after_cast(&fireball, true, 1.0, RETRY);
        assert_eq!(book.known[0].timer, 2.0);
        assert_eq!(book.known[0].damage_scale, 1.5);
        book.learn(&spell("frostbolt", School::Frost, Delivery::Projectile, 1.0), &[]);
        assert_eq!(book.known[1].damage_scale, 1.5);
        assert_eq!(book.free_cast(&spell("pyroblast", School::Fire, Delivery::Projectile, 1.0), &[]).damage_scale, 1.5);
    }

    #[test]
    fn refresh_updates_every_slot() {
        let a = spell("a", School::Holy, Delivery::Projectile, 2.0);
        let r = spell("stoneform", School::Physical, Delivery::SelfEffect, 45.0);
        let mut book = Spellbook::default();
        book.learn(&a, &[]);
        book.racial = Some(SpellSlot::new(&r, &[]));
        let mods = [m(SpellFilter::Any, Upgrade::Duration, 0.5)];
        book.refresh_mods(&mods, |id| [&a, &r].into_iter().find(|x| x.id == id));
        assert_eq!(book.known[0].mods.duration, 1.5);
        assert_eq!(book.racial.as_ref().unwrap().mods.duration, 1.5);
    }
}
