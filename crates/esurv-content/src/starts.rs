//! Starting options: the choice a hero makes before a run (the warband screen's Starting spell
//! spot). Each class has three, the Druid four; each is a level-1 spell (or a strike and its
//! finisher), a passive, and for some an own card pool, a form, a resource or a weapon swing.
//! Docs/game/AUTHORING.md "Starting options".
//!
//! Also here: the kinds the run's events name for the interface (the mana-bar counters, the buff
//! stacks, the summons), so every crate shares one list.

use serde::{Deserialize, Serialize};

use crate::names::NameRef;
use crate::schema::AutoAttack;

/// One starting option of a class.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartingOption {
    /// Unique among all classes' options (`slam`, `moonkin_form` …); the save and `HeroSetup` name it.
    pub id: String,
    /// The name as the content file writes it: a pointer where the client holds it, else our text.
    #[serde(rename = "name")]
    pub name_ref: NameRef,
    /// The name as the game shows it, filled from `name_ref` as the content loads.
    #[serde(skip)]
    pub name: String,
    pub icon: String,
    /// What the option plays like, in our own words (`{Spell(id)}` pointers allowed).
    pub description: String,
    /// The level-1 spells (ability ids), the main one first: the one its upgrade cards improve and
    /// whose casts feed the passive. A second is the strike's finisher (Envenom, Rip …).
    pub spells: Vec<String>,
    pub passive: Passive,
    /// The cards a level-up offers with this option, in place of the class's pool (the Druid's
    /// forms). Missing: the class's pool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_pool: Option<Vec<String>>,
    /// Cards of its pool a level-up offers more often than the others (the forms' Swipe).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub common_cards: Vec<String>,
    /// A form kept the whole run: a model swap and its stats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<Form>,
    /// A resource the level-1 spells spend (rage, energy and combo points).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<Resource>,
    /// A weapon swing in place of the class's (the Bear's and the Cat's); missing: the class's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swing: Option<AutoAttack>,
    /// A pet that fights beside the hero the whole run (Kill Command's cat).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pet: Option<SummonSpec>,
}

impl StartingOption {
    /// The main level-1 spell's id.
    pub fn main_spell(&self) -> &str {
        self.spells.first().map_or("", String::as_str)
    }

    /// Every ability the option names: its level-1 spells, then what its passive, its pet and its
    /// summons cast.
    pub fn all_spells(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.spells.iter().map(String::as_str).collect();
        v.extend(self.passive_spells());
        v.extend(self.pet.iter().flat_map(|s| s.spells.iter().map(String::as_str)));
        v
    }

    /// What the passive itself casts or summons, whose damage is the passive's.
    pub fn passive_spells(&self) -> Vec<&str> {
        match &self.passive.kind {
            PassiveKind::Counter(c) => match &c.release {
                Release::Spell(s) => vec![s.as_str()],
                Release::Summon(list) => list.iter().flat_map(|s| s.spells.iter().map(String::as_str)).collect(),
            },
            PassiveKind::Orbs { spell, .. } | PassiveKind::Marks { spell, .. } | PassiveKind::BlockStrikes { spell } => vec![spell.as_str()],
            PassiveKind::Empower { burst: Some(s), .. } | PassiveKind::SwingSpeedStacks { spell: Some(s), .. } => vec![s.as_str()],
            _ => Vec::new(),
        }
    }
}

/// An option's passive: its name, look and what it does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Passive {
    /// Its id, which `PassiveTriggered` events carry (`holy_power`, `zeal` …).
    pub id: String,
    #[serde(rename = "name")]
    pub name_ref: NameRef,
    #[serde(skip)]
    pub name: String,
    pub icon: String,
    pub description: String,
    pub kind: PassiveKind,
}

/// What a passive does, with its numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PassiveKind {
    /// Nothing of its own: the resource (rage, combo points), the spell itself (Holy Shield's
    /// block) or the form's stats are the passive.
    None,
    /// A counter shown in the mana bar: it fills by `fill`, and at `max` it is released.
    Counter(Counter),
    /// Each proc of the main spell adds a stack of `amount` weapon swing speed for `seconds` on its
    /// own (a new stack does not renew the others), up to `max` (Zeal). With `spell`, each swing
    /// that hits also strikes its target with `spell` once per stack.
    SwingSpeedStacks {
        amount: f32,
        seconds: f32,
        max: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spell: Option<String>,
    },
    /// Each hit of the main spell has `chance` to give `amount` more weapon swing speed for
    /// `seconds`; a new proc renews it (Quick Shots, on the Hunter's ranged swing). While it is up
    /// the swing also deals `damage_bonus` more.
    SwingSpeedOnHit {
        chance: f32,
        amount: f32,
        seconds: f32,
        #[serde(default)]
        damage_bonus: f32,
    },
    /// Every blocked blow strikes the attacker with `spell` (Holy Shield's Block).
    BlockStrikes { spell: String },
    /// The pets' critical hits give them `amount` more attack speed for `seconds` (Frenzy).
    PetFrenzy { amount: f32, seconds: f32 },
    /// Each tick of the main spell has `chance` to make its next `charges` casts free and instant
    /// (Lock and Load).
    FreeCasts { chance: f32, charges: u32 },
    /// Each cast of the main spell has `chance` to drop an orb somewhere `distance` yards from the
    /// hero (min, max, spread evenly over that ring, kept inside the arena), lasting `lasts` seconds;
    /// the hero collects it by walking within `touch` yards of it (pickup range does not reach it),
    /// which casts `spell` (Holy Orbs).
    Orbs { chance: f32, distance: (f32, f32), lasts: f32, touch: f32, spell: String },
    /// Each hit of the main spell has `chance` to mark the enemy for `seconds`; the main spell
    /// takes marked enemies first, and a hit on a marked enemy deals `share` of it as `spell` to
    /// every other enemy within `radius` yd (Lightning Rod).
    Marks { chance: f32, seconds: f32, share: f32, radius: f32, spell: String },
    /// After `max` casts of the main spell, the next damaging spell that is not the main spell
    /// deals `bonus` more on all its damage; the stacks wait at `max` until then (Arcane
    /// Knowledge).
    Empower {
        max: u32,
        bonus: f32,
        /// A look on the hero as the stacks fill (a `Spell.dbc` id).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look: Option<u32>,
        /// Cast as the stacks fill (the passive deals damage of its own).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        burst: Option<String>,
    },
}

/// A counter in the mana bar (Holy Power, Insanity, Soul Shards …).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Counter {
    /// What the mana bar shows (its name and colour).
    pub meter: ResourceKind,
    /// What fills it, each source adding up.
    pub fill: Vec<Fill>,
    pub max: f32,
    /// What happens when it is full; then it starts over at 0.
    pub release: Release,
    /// The first hit of a frost spell on each enemy the release froze deals this many times its
    /// damage (Shatter); 0: none.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub shatter: f32,
    /// A look on the hero when it is released (a `Spell.dbc` id); missing: none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<u32>,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

/// What fills a counter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Fill {
    /// Per cast of the main spell.
    Cast(f32),
    /// Per critical hit of the main spell.
    Crit(f32),
    /// One point per `per` damage the hero deals (its summons from this counter not counted);
    /// healing counts `heal_factor` times, overhealing not at all. With `main_only`, only the
    /// option's level-1 spells and what they summon count, not the other spells.
    Damage {
        per: f32,
        heal_factor: f32,
        #[serde(default)]
        main_only: bool,
    },
    /// Per second.
    Time(f32),
    /// Per hit of the main spell on a frozen enemy (not one the release froze).
    FrozenHit(f32),
    /// Per enemy that dies under the main spell, at most once per cast.
    KillUnderSpell(f32),
    /// Per summon of the main spell whose time runs out.
    SummonExpired(f32),
    /// A chance of +1 per melee hit (weapon swings and extra hits): `fast` with a swing of 2 s
    /// or less, `slow` with a slower one.
    MeleeHit { fast: f32, slow: f32 },
}

/// What a full counter does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Release {
    /// Casts this spell (an ability no card offers; it grows with the main spell's Power cards).
    Spell(String),
    /// Summons these.
    Summon(Vec<SummonSpec>),
}

/// Units a spell or passive summons.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummonSpec {
    pub kind: SummonKind,
    /// The pets (`pets.ron` names) whose models it shows, one after another for each summoned;
    /// none for the hero's own clone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pets: Vec<String>,
    pub count: u32,
    /// How long it stays; 0: the whole run.
    pub seconds: f32,
    /// What it does to enemies: an ability whose numbers it hits with and whose name the damage
    /// meter shows (several abilities: one at random each time).
    pub spells: Vec<String>,
}

/// The kinds of summon, for the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SummonKind {
    /// A ghostly copy of the hero that stands still and casts (Call of the Ancestors).
    Clone,
    EyeTentacle,
    ClawTentacle,
    Infernal,
    Imp,
    /// A small creature that runs at a random enemy (Friend of the Forest).
    Critter,
    /// A plant that runs into an enemy and bursts (Emerald Slumber).
    Plant,
    WaterElemental,
    /// A pet that fights the whole run (Kill Command's).
    HunterPet,
    /// A see-through copy of the hero that runs ahead, then spins after enemies (Blade Dance).
    Reflection,
}

/// A form kept the whole run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Form {
    /// The 1.12 shapeshift spell whose model and look the hero takes.
    pub look_spell: u32,
    /// Flat max health added.
    #[serde(default)]
    pub bonus_health: f32,
    /// Damage reduction added (0.05 = 5 %).
    #[serde(default)]
    pub bonus_damage_reduction: f32,
    /// Spells cast out of the form (Regrowth, Rejuvenation): the hero leaves it for the cast and
    /// shifts straight back, as a powershift.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caster_spells: Vec<String>,
}

/// A resource the level-1 spells spend.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Resource {
    /// `per_swing` per weapon swing that hits, `per_health_lost` per whole max health lost (50:
    /// 1 per 2 %), up to `cap`; no decay. A spell casts by itself once the hero has its cost.
    Rage { per_swing: f32, per_health_lost: f32, cap: f32 },
    /// `per_tick` comes every `tick_every` seconds (1.12's energy
    /// ticks), up to `cap`; strikes build combo points up to `combo_points`, and finishers spend
    /// them. Energy-gain cards make each tick bigger; faster cooldowns make the ticks come sooner.
    Energy { per_tick: f32, tick_every: f32, cap: f32, combo_points: u32 },
}

impl Resource {
    /// The bar it shows in.
    pub fn kind(self) -> ResourceKind {
        match self {
            Resource::Rage { .. } => ResourceKind::Rage,
            Resource::Energy { .. } => ResourceKind::Energy,
        }
    }

    pub fn cap(self) -> f32 {
        match self {
            Resource::Rage { cap, .. } | Resource::Energy { cap, .. } => cap,
        }
    }
}

/// What a hero's mana bar (or the combo point pips) shows. The interface picks its colour and
/// name from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceKind {
    Rage,
    Energy,
    /// The pips on the player frame.
    ComboPoints,
    HolyPower,
    HolyShieldStacks,
    Zeal,
    InnerFire,
    Insanity,
    MaelstromWeapon,
    CallOfTheAncestors,
    ArcaneKnowledge,
    HotStreak,
    WaterElemental,
    SoulShards,
    DemonicSouls,
    BurningEmbers,
    LunarPower,
    EmeraldSlumber,
}

/// Stacks a buff on the hero (or its pet) shows, for the buff bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StackKind {
    Zeal,
    QuickShots,
    /// On the pet.
    Frenzy,
    LockAndLoad,
    HolyShield,
    InnerFire,
    ArcaneKnowledge,
    HotStreak,
    MaelstromWeapon,
    BurningEmbers,
    ThickHide,
}

impl crate::Content {
    /// Every ability starting option `o` brings into play: `all_spells`, and what its spells' own
    /// rules name (a poison, the summons' spells).
    pub fn starting_spells<'a>(&'a self, o: &'a StartingOption) -> Vec<&'a str> {
        let mut v = o.all_spells();
        for s in &o.spells {
            match self.ability(s).and_then(|a| a.kit.mechanic.as_ref()) {
                Some(Mechanic::Poisons { poison }) => v.push(poison),
                Some(Mechanic::Summons(spec)) => v.extend(spec.spells.iter().map(String::as_str)),
                _ => {}
            }
        }
        v
    }

    /// The starting option `id` of any class.
    pub fn starting_option(&self, id: &str) -> Option<&StartingOption> {
        self.classes.iter().flat_map(|c| &c.starting_options).find(|o| o.id == id)
    }

    /// Adds a problem for every starting option of `class` that names what is not there.
    pub(crate) fn check_starting_options(&self, class: &crate::Class, problems: &mut Vec<String>) {
        if class.starting_options.is_empty() {
            problems.push(format!("class {} has no starting option", class.id));
        }
        for o in &class.starting_options {
            let what = format!("class {} starting option {}", class.id, o.id);
            if self.classes.iter().flat_map(|c| &c.starting_options).filter(|x| x.id == o.id).count() > 1 {
                problems.push(format!("{what}: its id is used twice"));
            }
            if o.spells.is_empty() {
                problems.push(format!("{what} has no level-1 spell"));
            }
            let named: Vec<&str> = self.starting_spells(o).into_iter().chain(o.card_pool.iter().flatten().map(String::as_str)).collect();
            let mut summons: Vec<&SummonSpec> = o.pet.iter().collect();
            if let PassiveKind::Counter(Counter { release: Release::Summon(list), .. }) = &o.passive.kind {
                summons.extend(list);
            }
            for s in &summons {
                for p in s.pets.iter().filter(|p| !self.pets.iter().any(|x| &x.name == *p)) {
                    problems.push(format!("{what} summons unknown pet {p}"));
                }
            }
            for a in named.into_iter().filter(|a| self.ability(a).is_none()) {
                problems.push(format!("{what} names unknown ability {a}"));
            }
            // a starting option's spells are never cards
            let pool = class.card_pool(Some(&o.id));
            for s in o.spells.iter().filter(|s| pool.contains(s) || class.abilities.contains(s)) {
                problems.push(format!("{what}: its level-1 spell {s} is also a card"));
            }
            for s in o.common_cards.iter().filter(|s| !pool.contains(s)) {
                problems.push(format!("{what}: its common card {s} is not in its card pool"));
            }
        }
    }
}

/// What a spell does beyond its delivery, for the starting options' spells and the cards that
/// came with them (`Ability::kit`; all defaults: a plain spell).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SpellKit {
    /// Rage or energy a cast spends: the spell casts by itself as soon as the hero has it and an
    /// enemy is in reach (its cooldown, if any, still applies). 0: none.
    #[serde(skip_serializing_if = "is_zero")]
    pub cost: f32,
    /// Combo points a cast gives (a strike).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub combo_points: u32,
    /// An area of the spell strikes at most this many enemies, the
    /// nearest first. 0: every enemy in it.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub max_hits: u32,
    /// A finisher: casts when the hero has this many combo points, and spends them (points over
    /// it carry over). 0: not a finisher.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub finisher_at: u32,
    /// Which enemies it takes first.
    #[serde(skip_serializing_if = "TargetOrder::is_usual")]
    pub target_order: TargetOrder,
    /// A cone's full width in degrees; 0: the usual 120°.
    #[serde(skip_serializing_if = "is_zero")]
    pub cone_degrees: f32,
    /// Critical strike chance it has on top of the hero's (0.1 = 10 %).
    #[serde(skip_serializing_if = "is_zero")]
    pub crit_bonus: f32,
    /// Its hits deal double to enemies above this share of their health (Ravage); 0: never.
    #[serde(skip_serializing_if = "is_zero")]
    pub double_above: f32,
    /// While the buff the spell puts on the hero is up, weapon swings this much faster (0.2 = 20 %;
    /// Slice and Dice). 0: none.
    #[serde(skip_serializing_if = "is_zero")]
    pub swing_speed: f32,
    /// Its cooldown starts only once the effect it put on the hero has ended (run out or taken
    /// away), not at the cast, so it is never up all the time (Divine Shield).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cooldown_after_effect: bool,
    /// What else it does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mechanic: Option<Mechanic>,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

/// The order a spell takes enemies in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetOrder {
    /// Bosses, then elites, then the bunch around the nearest by health, then the nearest.
    #[default]
    Usual,
    /// Bosses, elites and rares first, then the most health left.
    Healthiest,
    /// The least health left first (as a share of its max).
    MostWounded,
    /// The least health left first (flat health points).
    LowestHealth,
    /// At random.
    Random,
    /// Bosses, elites and rares first, then the nearest (bouncing chains' first target).
    BossesFirst,
}

impl TargetOrder {
    fn is_usual(&self) -> bool {
        *self == TargetOrder::Usual
    }
}

/// A spell's own rule, beyond what its delivery does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Mechanic {
    /// A buff of the spell's `duration` on the hero: `block_chance` more block chance and
    /// `stacks` stacks; each blocked blow uses one and deals the spell's damage to the attacker.
    /// It ends with its time or its last stack.
    BlockShield { block_chance: f32, stacks: u32 },
    /// Always on: each weapon swing that hits has a chance to strike the swing's target (and
    /// `targets − 1` more nearby) for the spell's damage. The chance and damage follow the swing:
    /// `fast_chance` with a swing of 2 s or less, `slow_chance` with a slower one, and damage ×
    /// swing ÷ (chance × the swing the spell's damage is written for, `base_swing`), so the
    /// damage a second stays the same.
    SwingProc { fast_chance: f32, slow_chance: f32, base_swing: f32 },
    /// Always on: each weapon swing that hits has `chance` to hit `hits` more times (they cannot
    /// set it off again) for the spell's damage × swing ÷ `base_swing`.
    ExtraSwingHits { chance: f32, hits: u32, base_swing: f32 },
    /// An effect over time on the spell's first target; if that enemy dies, it goes on to the
    /// next in the spell's order for the time left (Drain Soul).
    Drain,
    /// Each enemy struck is also poisoned by `poison` (an effect over time), enemies not yet
    /// poisoned first.
    Poisons { poison: String },
    /// Strikes every enemy in range carrying `spell` (an effect over time), and only those.
    StrikesAfflicted { spell: String },
    /// Takes enemies marked by `spell` (its `Vulnerable` effect) first.
    PrefersMarked { spell: String },
    /// Its effect over time goes on an enemy not yet carrying it first, so cast faster than it
    /// lasts it runs on several at once; cast on one that already carries it, the two share out
    /// what they had left (Rip).
    FreshTargetFirst,
    /// Its effect over time stacks up to `max` times on each enemy (the others renew instead);
    /// one more renews the stack that runs out first, the two sharing out what they had left, as
    /// renewals do. Each cast still puts at most one stack on an enemy (Lacerate).
    StacksUpTo { max: u32 },
    /// A buff of the spell's `duration` on the hero: while it is up, each of the hero's melee
    /// strikes, finishers, weapon swings and effects over time also strikes the spell's `targets`
    /// (and its Targets cards) enemies nearest the one it struck first, within 8 yd of it, for its
    /// full damage, listed under the attack that made them (Berserk).
    AttacksStrikeMore,
    /// Flies `range` yards toward the first target and back, striking every enemy within the
    /// spell's `radius` of its path each way.
    Returns,
    /// Gives `amount` energy, cast by itself when the hero has `below` energy or less.
    GainEnergy { amount: f32, below: f32 },
    /// Orders the hero's pets to strike their targets for the spell's damage each.
    PetsStrike,
    /// Summons these where the spell lands (Hand of Gul'dan's imps, Friend of the Forest's
    /// critters); upgrade cards of the summon kind add to `count`.
    Summons(SummonSpec),
    /// Each cast gives a stack of `amount` damage reduction for `seconds`, each on its own
    /// timer, up to `base` stacks; its Stacks cards raise the most by `per_card` each.
    ReductionStacks {
        amount: f32,
        seconds: f32,
        per_card: u32,
        #[serde(default)]
        base: u32,
    },
    /// A lasting area that strikes once, `seconds` after it lands (Seed of Corruption).
    Delayed { seconds: f32 },
}
