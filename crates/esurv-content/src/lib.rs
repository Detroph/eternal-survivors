//! Game content as data: everything a run is built from, loaded from the RON files under
//! `content/` at startup so it can be edited without a rebuild.
//!
//! Units are seconds, yards, per-second rates and fractions (0.4 = 40 %); each field's doc
//! comment says which.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

mod assists;
mod boss_mechanics;
mod categories;
#[cfg(feature = "client")]
mod client_tables;
pub mod entries;
mod kinds;
pub mod legendary_counters;
pub mod legendary_looks;
pub mod live;
mod loadouts;
pub mod looks;
pub mod option_looks;
mod names;
mod schema;
mod starts;
mod tags;
mod zone_files;
pub mod testing;
pub mod text;
#[cfg(feature = "watch")]
pub mod watch;
pub use assists::*;
pub use boss_mechanics::*;
pub use categories::apply_category_tuning;
#[cfg(feature = "client")]
pub use client_tables::{read_spell_times, ClientTables, SpellNameRow, SpellTimes};
pub use kinds::*;
pub use legendary_looks::{LegendaryLook, Moment};
pub use live::{LiveContent, ReloadOutcome};
pub use loadouts::*;
pub use names::{ClientName, ClientNames, NameRef};
pub use schema::*;
pub use starts::*;
pub use tags::apply_spell_tags;
pub use zone_files::{read_zone_entries, ZONE_FOLDER};

/// All content, indexed for lookup by id.
#[derive(Debug, Clone)]
pub struct Content {
    pub zones: Vec<Zone>,
    pub classes: Vec<Class>,
    pub races: Vec<Race>,
    pub abilities: Vec<Ability>,
    pub talent_trees: Vec<TalentTree>,
    pub jewelry: Vec<Jewelry>,
    pub run_options: Vec<RunOption>,
    pub pets: Vec<Pet>,
    pub gear: Gear,
    pub amulets: Vec<Amulet>,
    pub legendary_amulets: Vec<LegendaryAmulet>,
    /// The legendaries' looks (`legendary_looks.ron`), already put into `legendary_amulets`.
    pub legendary_looks: Vec<LegendaryLook>,
    /// The per-category multipliers already applied to `abilities`.
    pub spell_categories: Vec<CategoryTuning>,
    /// The stock starting-outfit weapons and shields, by item entry (`outfit_weapons.ron`).
    pub outfit_weapons: Vec<OutfitWeapon>,
    /// Each class's weapon loadouts, the default first (`loadouts.ron`).
    pub loadouts: Vec<ClassLoadouts>,
    /// The player-looking enemies' gear and names (`pvp_looks.ron`).
    pub pvp_looks: PvpLooks,
    /// The hand-made player characters (`player_characters.ron`).
    pub player_characters: Vec<PlayerCharacter>,
    /// Each class's default assist (and its forms'), for a character without a script of its
    /// own (`class_assists.ron`).
    pub class_assists: Vec<ClassAssist>,
    /// The rotation spells' cast times and cooldowns (`assist_spells.ron`, from `Spell.dbc`).
    pub assist_spells: Vec<AssistSpell>,
    /// What prestige points buy, a rank at a time (`prestige.ron`).
    pub prestige: Vec<PrestigeStat>,
    /// The words cards put around their numbers (`words.ron`).
    pub words: Words,
    /// The enemy stat formula's numbers (`balance.ron`).
    pub balance: Balance,
    /// The roster kinds' and boss abilities' shared numbers (`enemy_kinds.ron`), already resolved
    /// into `zones`; `None` for content built in code.
    pub enemy_kinds: Option<EnemyKinds>,
    ability_index: HashMap<String, usize>,
    class_index: HashMap<String, usize>,
    zone_index: HashMap<String, usize>,
}

/// The on-disk form of each file: a list of one record type.
#[derive(Deserialize, Serialize)]
struct File<T>(Vec<T>);

fn read<T: for<'de> Deserialize<'de>>(dir: &Path, name: &str) -> Result<Vec<T>> {
    let path = dir.join(name);
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let File(v) = ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(v)
}

fn read_one<T: for<'de> Deserialize<'de>>(dir: &Path, name: &str) -> Result<T> {
    let path = dir.join(name);
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

impl Content {
    /// Loads every content file from `dir` and checks cross-references, with no client tables:
    /// a name that points at the client reads as its placeholder (`Spell 12294`). For tests, tools
    /// and builds with no client data; the game loads with [`Content::load_with_names`].
    pub fn load(dir: impl AsRef<Path>) -> Result<Self> {
        Self::load_with_names(dir, &ClientNames::none())
    }

    /// Loads every content file from `dir` and checks cross-references; every name that points at
    /// the client shows as `names` has it. A pointer the read tables lack is a problem.
    pub fn load_with_names(dir: impl AsRef<Path>, names: &ClientNames) -> Result<Self> {
        let dir = dir.as_ref();
        let spell_categories: Vec<CategoryTuning> = read(dir, "spell_categories.ron")?;
        let balance: Balance = read_one(dir, "balance.ron")?;
        let mut abilities = read(dir, "abilities.ron")?;
        let mut pets = read(dir, "pets.ron")?;
        apply_category_tuning(&mut abilities, &spell_categories)?;
        apply_spell_tags(&mut abilities, &mut pets, &balance.spell_tags)?;
        let enemy_kinds: EnemyKinds = read_one(dir, "enemy_kinds.ron")?;
        let zones = resolve_zones(read_zone_entries(dir)?, &enemy_kinds, names)?;
        let mut classes = read(dir, "classes.ron")?;
        let mut talent_trees = read(dir, "talents.ron")?;
        let mut races = read(dir, "races.ron")?;
        let mut words: Words = read_one(dir, "words.ron")?;
        let mut legendary_amulets: Vec<LegendaryAmulet> = read(dir, "legendary_amulets.ron")?;
        let mut player_characters: Vec<PlayerCharacter> = read(dir, "player_characters.ron")?;
        let mut class_assists: Vec<ClassAssist> = read(dir, "class_assists.ron")?;
        show_names(names, &mut classes, &mut abilities, &mut talent_trees, &mut races, &mut words)?;
        let mut loadouts: Vec<ClassLoadouts> = read(dir, "loadouts.ron")?;
        show_prose(names, &mut legendary_amulets, &mut player_characters, &mut class_assists, &mut loadouts)?;
        let mut content = Self::from_parts(
            zones,
            classes,
            abilities,
            talent_trees,
            read(dir, "jewelry.ron")?,
            read(dir, "run_options.ron")?,
            pets,
            read_one(dir, "gear.ron")?,
            read(dir, "amulets.ron")?,
            legendary_amulets,
        );
        content.legendary_looks = read(dir, "legendary_looks.ron")?;
        let looks = content.legendary_looks.clone();
        let unknown = legendary_looks::apply(&mut content.legendary_amulets, &looks);
        if !unknown.is_empty() {
            bail!("content has {} problem(s):\n  {}", unknown.len(), unknown.join("\n  "));
        }
        content.spell_categories = spell_categories;
        content.races = races;
        content.outfit_weapons = read(dir, "outfit_weapons.ron")?;
        content.loadouts = loadouts;
        content.pvp_looks = read_one(dir, "pvp_looks.ron")?;
        content.player_characters = player_characters;
        content.class_assists = class_assists;
        content.assist_spells = read(dir, "assist_spells.ron")?;
        content.prestige = read(dir, "prestige.ron")?;
        content.words = words;
        content.balance = balance;
        content.enemy_kinds = Some(enemy_kinds);
        content.validate()?;
        Ok(content)
    }

    /// `prestige.ron`'s rules: ids once each, known classes, a stat or every-spell number (never a
    /// milestone) with a step above 0, and nothing that adds targets.
    fn check_prestige(&self, problems: &mut Vec<String>) {
        let mut seen = std::collections::HashSet::new();
        for p in &self.prestige {
            if !seen.insert(p.id.as_str()) {
                problems.push(format!("prestige {} is listed twice", p.id));
            }
            if let Some(c) = p.classes.iter().find(|c| self.class(c).is_none()) {
                problems.push(format!("prestige {} names unknown class {c}", p.id));
            }
            if matches!(p.effect, TalentEffect::Milestone(_)) || !(p.per_rank().is_finite() && p.per_rank() > 0.0) {
                problems.push(format!("prestige {} needs a stat or every-spell number with a step above 0", p.id));
            }
            if p.adds_targets() {
                problems.push(format!("prestige {} adds targets, which prestige never may", p.id));
            }
            if p.max_ranks == Some(0) {
                problems.push(format!("prestige {} takes no rank", p.id));
            }
        }
    }

    pub fn from_parts(
        zones: Vec<Zone>,
        classes: Vec<Class>,
        abilities: Vec<Ability>,
        talent_trees: Vec<TalentTree>,
        jewelry: Vec<Jewelry>,
        run_options: Vec<RunOption>,
        pets: Vec<Pet>,
        gear: Gear,
        amulets: Vec<Amulet>,
        legendary_amulets: Vec<LegendaryAmulet>,
    ) -> Self {
        let index = |ids: Vec<&String>| ids.into_iter().enumerate().map(|(i, id)| (id.clone(), i)).collect();
        let mut pets = pets;
        for (i, p) in pets.iter_mut().enumerate() {
            p.entry = entries::pet_entry(i);
        }
        Self {
            ability_index: index(abilities.iter().map(|a| &a.id).collect()),
            class_index: index(classes.iter().map(|c| &c.id).collect()),
            zone_index: index(zones.iter().map(|z| &z.id).collect()),
            zones,
            classes,
            abilities,
            talent_trees,
            jewelry,
            run_options,
            pets,
            gear,
            amulets,
            legendary_amulets,
            legendary_looks: Vec::new(),
            spell_categories: Vec::new(),
            words: Words::default(),
            balance: Balance::default(),
            enemy_kinds: None,
            races: Vec::new(),
            outfit_weapons: Vec::new(),
            loadouts: Vec::new(),
            pvp_looks: PvpLooks::default(),
            player_characters: Vec::new(),
            class_assists: Vec::new(),
            assist_spells: Vec::new(),
            prestige: Vec::new(),
        }
    }

    /// The starting-outfit weapon or shield with item entry `item`.
    pub fn outfit_weapon(&self, item: u32) -> Option<&OutfitWeapon> {
        self.outfit_weapons.iter().find(|w| w.item == item)
    }

    /// Class `class`'s weapon loadouts, the default first (none for a class `loadouts.ron` lacks).
    pub fn loadouts_of(&self, class: &str) -> &[Loadout] {
        self.loadouts.iter().find(|l| l.class == class).map_or(&[], |l| l.loadouts.as_slice())
    }

    /// Class `class`'s loadout `id`, or its first (the default) when `id` is none of its own.
    pub fn loadout(&self, class: &str, id: Option<&str>) -> Option<&Loadout> {
        let all = self.loadouts_of(class);
        id.and_then(|id| all.iter().find(|l| l.id == id)).or(all.first())
    }

    /// `loadouts.ron`'s rules: known classes once each, three loadouts a class with ids once each,
    /// at most one weapon a hand, each with a look, swings above 0 seconds and only for a class
    /// with a weapon swing, and each loadout of a swinging class swings.
    fn check_loadouts(&self, problems: &mut Vec<String>) {
        let mut seen = std::collections::HashSet::new();
        for cl in &self.loadouts {
            if !seen.insert(cl.class.as_str()) {
                problems.push(format!("loadouts for {} are listed twice", cl.class));
            }
            let Some(class) = self.class(&cl.class) else {
                problems.push(format!("loadouts name unknown class {}", cl.class));
                continue;
            };
            if cl.loadouts.len() != 3 {
                problems.push(format!("class {} has {} loadouts, not 3", cl.class, cl.loadouts.len()));
            }
            let mut ids = std::collections::HashSet::new();
            for l in &cl.loadouts {
                if !ids.insert(l.id.as_str()) {
                    problems.push(format!("loadout {}/{} is listed twice", cl.class, l.id));
                }
                for hand in [Hand::MainHand, Hand::OffHand, Hand::Ranged] {
                    if l.weapons.iter().filter(|w| w.hand == hand).count() > 1 {
                        problems.push(format!("loadout {}/{} holds two weapons in one hand", cl.class, l.id));
                    }
                }
                for w in &l.weapons {
                    if w.looks.iter().any(|a| a.display == 0) || w.looks.is_empty() {
                        problems.push(format!("loadout {}/{}: {} needs a look in each of its looks", cl.class, l.id, w.name));
                    }
                    // a look in every zone's armaments, each row the
                    // class set has
                    for look in self.gear_set(&cl.class).map_or(&[][..], |s| s.looks.as_slice()) {
                        if !w.looks.iter().any(|a| a.zone == look.zone && a.row == look.row) {
                            problems.push(format!(
                                "loadout {}/{}: {} has no look in {} row {}",
                                cl.class, l.id, w.name, look.zone, look.row
                            ));
                        }
                    }
                    if let Some(s) = w.swing {
                        if !(s.seconds > 0.0 && s.share >= 0.0) {
                            problems.push(format!("loadout {}/{}: {} needs a swing above 0 seconds", cl.class, l.id, w.name));
                        }
                        if class.auto_attack.is_none() {
                            problems.push(format!("loadout {}/{}: {} swings, but class {} has no weapon swing", cl.class, l.id, w.name, cl.class));
                        }
                    }
                }
                if class.auto_attack.is_some() && l.swings().is_empty() {
                    problems.push(format!("loadout {}/{} has no swing, but class {} swings", cl.class, l.id, cl.class));
                }
            }
        }
        for c in &self.classes {
            if c.auto_attack.is_some_and(|a| a.swing <= 0.0) && self.loadouts_of(&c.id).is_empty() {
                problems.push(format!("class {} swings with no swing time and has no loadouts", c.id));
            }
        }
    }

    pub fn race(&self, id: &str) -> Option<&Race> {
        self.races.iter().find(|r| r.id == id)
    }

    /// The race with WoW race id `wow_race_id`.
    pub fn race_by_wow_id(&self, wow_race_id: u8) -> Option<&Race> {
        self.races.iter().find(|r| r.wow_race_id == wow_race_id)
    }

    pub fn ability(&self, id: &str) -> Option<&Ability> {
        self.ability_index.get(id).map(|&i| &self.abilities[i])
    }

    pub fn class(&self, id: &str) -> Option<&Class> {
        self.class_index.get(id).map(|&i| &self.classes[i])
    }

    pub fn zone(&self, id: &str) -> Option<&Zone> {
        self.zone_index.get(id).map(|&i| &self.zones[i])
    }

    /// The gear set of a class.
    pub fn gear_set(&self, class: &str) -> Option<&GearSet> {
        self.gear.sets.iter().find(|s| s.class == class)
    }

    /// The three talent trees of a class, in tree order.
    pub fn trees_of<'a>(&'a self, class: &'a str) -> impl Iterator<Item = &'a TalentTree> + 'a {
        self.talent_trees.iter().filter(move |t| t.class == class)
    }

    /// Every id a record names must resolve.
    pub fn validate(&self) -> Result<()> {
        let mut problems = Vec::new();
        for (name, clock) in [("health_clock", &self.balance.health_clock), ("damage_clock", &self.balance.damage_clock)] {
            if clock.len() < 2 || clock.windows(2).any(|w| w[1].0 <= w[0].0) {
                problems.push(format!("balance {name} needs two or more points in rising minutes"));
            }
        }
        let s = &self.balance.zone_scaling;
        let exponents = [s.elite_health_power, s.boss_health_power, s.damage_share, s.elite_boss_damage_share, s.gear_exponent, s.talent_exponent];
        if !(s.gear_growth_after.is_finite() && s.gear_growth_after > 0.0) || exponents.iter().any(|v| !(v.is_finite() && *v >= 0.0)) {
            problems.push("balance zone_scaling needs gear_growth_after above 0 and powers and exponents of 0 or more".into());
        }
        let health = &s.normal_health;
        if health.len() < 2 || health.windows(2).any(|w| w[1].0 <= w[0].0) || health.iter().any(|p| !(p.1.is_finite() && p.1 > 0.0)) {
            problems.push("balance zone_scaling normal_health needs two or more points in rising D with multipliers above 0".into());
        }
        let damage = &s.normal_damage;
        if !damage.is_empty() && (damage.len() < 2 || damage.windows(2).any(|w| w[1].0 <= w[0].0) || damage.iter().any(|p| !(p.1.is_finite() && p.1 > 0.0))) {
            problems.push("balance zone_scaling normal_damage needs none, or two or more points in rising D with multipliers above 0".into());
        }
        if [s.elite_damage_power, s.boss_damage_power].iter().any(|v| !(v.is_finite() && *v >= 0.0)) {
            problems.push("balance zone_scaling elite_damage_power and boss_damage_power need to be 0 or more".into());
        }
        if s.gear_tier_starts.windows(2).any(|w| w[1] <= w[0]) {
            problems.push("balance zone_scaling gear_tier_starts need rising difficulties".into());
        }
        let drops = &s.gear_drop_chance;
        if drops.is_empty() || drops.windows(2).any(|w| w[1].0 <= w[0].0) || drops.iter().any(|d| !(d.1 >= 0.0 && d.1 <= 1.0)) {
            problems.push("balance zone_scaling gear_drop_chance needs steps in rising D with chances from 0 to 1".into());
        }
        self.check_prestige(&mut problems);
        self.check_loadouts(&mut problems);
        for c in &self.classes {
            for a in c.abilities.iter() {
                if self.ability(a).is_none() {
                    problems.push(format!("class {} names unknown ability {a}", c.id));
                }
            }
            self.check_starting_options(c, &mut problems);
            if self.trees_of(&c.id).count() != 3 {
                problems.push(format!("class {} has {} talent trees, not 3", c.id, self.trees_of(&c.id).count()));
            }
        }
        for t in &self.talent_trees {
            let ids: Vec<&str> = t.talents.iter().map(|x| x.id.as_str()).collect();
            for tal in &t.talents {
                if let Some(req) = &tal.requires {
                    if !ids.contains(&req.as_str()) {
                        problems.push(format!("talent {} requires {req}, not in tree {}", tal.id, t.name));
                    }
                }
                for e in &tal.effects {
                    if let TalentEffect::Milestone(m) = e {
                        if let Some(name) = m.pet() {
                            if !self.pets.iter().any(|x| &x.name == name) {
                                problems.push(format!("talent {} summons unknown pet {name}", tal.id));
                            }
                        }
                    }
                    for a in e.spells().into_iter().filter(|a| self.ability(a).is_none()) {
                        problems.push(format!("talent {} names unknown ability {a}", tal.id));
                    }
                }
            }
        }
        for (i, r) in self.races.iter().enumerate() {
            if self.ability(&r.spell).is_none() {
                problems.push(format!("race {} has unknown race spell {}", r.id, r.spell));
            }
            if self.races[..i].iter().any(|x| x.id == r.id || x.wow_race_id == r.wow_race_id) {
                problems.push(format!("race {} (WoW id {}) is listed twice", r.id, r.wow_race_id));
            }
            for c in r.added_classes.iter().filter(|c| self.class(c).is_none()) {
                problems.push(format!("race {} adds unknown class {c}", r.id));
            }
            if r.totem_look.as_ref().is_some_and(|t| t.part == crate::TotemPart::Model(String::new()) || t.scale <= 0.0) {
                problems.push(format!("race {} has a totem look without a model or a size", r.id));
            }
        }
        for c in &self.classes {
            if self.gear.sets.iter().filter(|s| s.class == c.id).count() != 1 {
                problems.push(format!("class {} needs exactly one gear set", c.id));
            }
        }
        let g = &self.gear;
        if g.slots.len() != 8 || g.bonus_stats.len() != 8 {
            problems.push("gear needs 8 slots and 8 bonus pairs".into());
        }
        if g.slots.iter().any(|s| s.names.len() != 4) {
            problems.push("every gear slot needs 4 names (cloth, leather, mail, plate)".into());
        }
        for s in &g.sets {
            if self.class(&s.class).is_none() {
                problems.push(format!("gear set names unknown class {}", s.class));
            }
            // every zone its own look of the eight pieces, rows from 0
            // (a run's entries have room for 100 looks)
            if s.looks.iter().any(|l| l.names.len() != 8 || l.displays.len() != 8) {
                problems.push(format!("gear set {} needs 8 names and 8 displays in every look", s.class));
            }
            if s.looks.is_empty() || s.looks.len() > 100 {
                problems.push(format!("gear set {} needs 1 to 100 looks", s.class));
            }
            for (i, l) in s.looks.iter().enumerate() {
                if s.look_of(&l.zone, l.row) != Some(i) {
                    problems.push(format!("gear set {} has look {} row {} twice", s.class, l.zone, l.row));
                }
                if l.row > 0 && s.look_of(&l.zone, l.row - 1).is_none() {
                    problems.push(format!("gear set {}: look {} row {} without the row before it", s.class, l.zone, l.row));
                }
                if l.row_name.as_deref().is_some_and(|n| n.trim().is_empty()) {
                    problems.push(format!("gear set {}: look {} row {} has an empty row name", s.class, l.zone, l.row));
                }
            }
            if s.bonus_pairs.len() != 8 + s.weapons.len() || s.bonus_pairs.iter().any(|&p| p as usize >= g.bonus_stats.len()) {
                problems.push(format!("gear set {} needs a bonus pair of bonus_stats for each slot and weapon", s.class));
            }
            if s.weapons.len() > WEAPON_SLOTS as usize {
                problems.push(format!("gear set {} has room for {WEAPON_SLOTS} weapons", s.class));
            }
        }
        for a in &self.amulets {
            for e in &a.effects {
                for x in e.spells().into_iter().filter(|x| self.ability(x).is_none()) {
                    problems.push(format!("amulet {} names unknown ability {x}", a.id));
                }
            }
        }
        // one legendary amulet per spell a hero can know (every class
        // card, every form's card pool, every level-1 spell; a strike and its finisher share one)
        for l in &self.legendary_amulets {
            if l.spells.is_empty() {
                problems.push(format!("legendary amulet {} remakes no spell", l.name));
            }
            for n in l.spells.iter().chain(l.effects.iter().flat_map(|e| e.spells())).filter(|n| self.ability(n).is_none()) {
                problems.push(format!("legendary amulet {} names unknown ability {n}", l.name));
            }
            // a line's runners: some looks, a row of one or more, a pace
            for e in &l.effects {
                if let TalentEffect::Milestone(Milestone::LineHit { runners: Some(r), .. }) = e {
                    let (least, most) = r.count;
                    if r.displays.is_empty() || r.displays.iter().any(|(_, s)| !(s.is_finite() && *s > 0.0)) || least == 0 || least > most || !(r.speed.is_finite() && r.speed > 0.0) {
                        problems.push(format!("legendary amulet {} needs runners with looks (each scaled above 0), a count of 1 or more (least first) and a speed above 0", l.name));
                    }
                }
            }
        }
        let mut wanted: Vec<&str> = Vec::new();
        for c in &self.classes {
            wanted.extend(c.abilities.iter().map(String::as_str));
            for o in &c.starting_options {
                wanted.extend(o.spells.iter().map(String::as_str));
                wanted.extend(o.card_pool.iter().flatten().map(String::as_str));
            }
        }
        for s in &wanted {
            let n = self.legendary_amulets.iter().filter(|l| l.spells.iter().any(|x| x == s)).count();
            if n != 1 {
                problems.push(format!("spell {s} needs exactly one legendary amulet ({n} remake it)"));
            }
        }
        for l in &self.legendary_amulets {
            if l.spells.iter().any(|s| !wanted.contains(&s.as_str())) {
                problems.push(format!("legendary amulet {} remakes a spell no hero can know", l.name));
            }
        }
        for a in &self.abilities {
            if a.description.trim().is_empty() {
                problems.push(format!("ability {} has no description", a.id));
            } else if let Err(e) = text::check_spell_description(&a.description) {
                problems.push(format!("ability {}: {e}", a.id));
            }
        }
        if !self.words.upgrades.is_empty() || !self.words.stats.is_empty() {
            for u in [Upgrade::Power, Upgrade::Cooldown, Upgrade::Targets, Upgrade::Area, Upgrade::Duration] {
                if self.words.upgrade(u, 2.0).is_none() {
                    problems.push(format!("words.ron has no words for the {u:?} upgrade"));
                }
            }
            for s in self.amulets.iter().flat_map(|a| &a.effects) {
                if let TalentEffect::Stat { stat, .. } = s {
                    if self.words.stat(*stat).is_none() {
                        problems.push(format!("words.ron has no line for the {stat:?} stat an amulet gives"));
                    }
                }
            }
            // a starting option's stacking buff speaks with its passive's own words
            for kind in esurv_protocol::BuffKind::ALL {
                if kind != esurv_protocol::BuffKind::Passive && self.words.buff(kind).is_none() {
                    problems.push(format!("words.ron has no words for the {kind:?} buff"));
                }
            }
        }
        if self.zones.len() > entries::MAX_ZONES || self.pets.len() > entries::MAX_PETS {
            problems.push(format!("the creature entries have room for {} zones and {} pets", entries::MAX_ZONES, entries::MAX_PETS));
        }
        for p in self.pets.iter().filter(|p| p.hover.is_some_and(|h| !(h > 0.0))) {
            problems.push(format!("pet {} hovers {:?} yd above the ground", p.name, p.hover));
        }
        if self.zones.len() > entries::FACTION_ROSTERS_FROM
            || entries::faction_roster_place(self.zones.len().saturating_sub(1), 1) >= entries::MAX_ZONES
        {
            problems.push(format!("the creature entries have room for {} zones", entries::FACTION_ROSTERS_FROM));
        }
        for z in &self.zones {
            let factions = z.by_faction.iter().flat_map(|r| [("its roster fought by the Alliance", &r.vs_alliance), ("its roster fought by the Horde", &r.vs_horde)]);
            let rosters = std::iter::once(("its roster", &z.enemies, &z.bosses)).chain(factions.map(|(what, r)| (what, &r.enemies, &r.bosses)));
            for (what, enemies, bosses) in rosters {
                if enemies.len() > entries::MAX_ROSTER || bosses.len() > entries::MAX_BOSSES {
                    problems.push(format!(
                        "zone {} ({what}): the creature entries have room for {} enemies and {} bosses",
                        z.id,
                        entries::MAX_ROSTER,
                        entries::MAX_BOSSES
                    ));
                }
                if enemies.is_empty() || !bosses.iter().any(|b| b.is_final) {
                    problems.push(format!("zone {} ({what}): needs enemies and a final boss", z.id));
                }
                // a player-looking enemy wears its class's gear; any other shows its own model
                let looks = enemies.iter().map(|e| (&e.name, e.looks_like.as_ref(), e.display_id, e.rank.is_some()));
                for (name, look, display, ranked) in looks.chain(bosses.iter().map(|b| (&b.name, b.looks_like.as_ref(), b.display_id, b.rank.is_some()))) {
                    match look {
                        Some(l) if self.class(&l.class).is_none() || self.pvp_looks.of(&l.class).is_none() => {
                            problems.push(format!("zone {} ({what}): {name} looks like class {}, which classes.ron or pvp_looks.ron lacks", z.id, l.class))
                        }
                        None if display == 0 => problems.push(format!("zone {} ({what}): {name} has no display_id", z.id)),
                        _ => {}
                    }
                    if name.contains(JOKE) && self.pvp_looks.jokes.is_empty() {
                        problems.push(format!("zone {} ({what}): {name} takes a joke, but pvp_looks.ron has no jokes", z.id));
                    }
                    if (ranked || (look.is_some() && name.is_empty())) && self.pvp_looks.player_names.is_empty() {
                        problems.push(format!("zone {} ({what}): {name} has a rank, but pvp_looks.ron has no player_names", z.id));
                    }
                }
                // several final bosses come together
                let finals: Vec<f32> = bosses.iter().filter(|b| b.is_final).map(|b| b.spawn_minute).collect();
                if finals.windows(2).any(|w| w[0] != w[1]) {
                    problems.push(format!("zone {} ({what}): its final bosses come at different minutes", z.id));
                }
            }
        }
        for (k, c) in self.player_characters.iter().enumerate() {
            if self.player_characters[..k].iter().any(|o| o.id == c.id) {
                problems.push(format!("player character {} is written twice", c.id));
            }
            if !self.races.iter().any(|r| r.id == c.race) || self.class(&c.class).is_none() || self.pvp_looks.of(&c.class).is_none() {
                problems.push(format!("player character {}: race {} or class {} is not in races.ron, classes.ron and pvp_looks.ron", c.id, c.race, c.class));
            }
        }
        self.check_assists(&mut problems);
        self.check_battle_standard(&mut problems);
        self.check_class_boss_abilities(&mut problems);
        self.check_boss_mechanics(&mut problems);
        for b in self.zones.iter().flat_map(|z| z.every_boss()) {
            if let Some(CharacterPick::Named(id)) = &b.played_by {
                if self.player_character(id).is_none() {
                    problems.push(format!("boss {} is played by {id}, which player_characters.ron lacks", b.name));
                }
            }
        }
        if !problems.is_empty() {
            bail!("content has {} problem(s):\n  {}", problems.len(), problems.join("\n  "));
        }
        Ok(())
    }

    /// The hand-made character playing boss number `b` of `zone` for heroes fighting `faction`'s
    /// players in a run of seed `seed`: the one named, or one of
    /// `faction`'s dealt at random (the pool shuffled once for the run and dealt to its bosses in
    /// turn); `None` while the pool has none, and the boss shows its stand-in.
    pub fn boss_character(&self, zone: &Zone, b: usize, faction: Faction, seed: u64) -> Option<&PlayerCharacter> {
        match zone.bosses.get(b)?.played_by.as_ref()? {
            CharacterPick::Named(id) => self.player_character(id),
            CharacterPick::Any => {
                let mut all: Vec<_> = self.player_characters_of(faction).enumerate().collect();
                all.sort_by_key(|&(k, _)| spread(seed as u32, 60 + k as u32));
                (!all.is_empty()).then(|| all[b % all.len()].1)
            }
        }
    }

    /// The class boss number `b` of `zone` looks like (its character's, else its `looks_like`).
    pub fn boss_class<'a>(&'a self, zone: &'a Zone, b: usize, faction: Faction, seed: u64) -> Option<&'a str> {
        match self.boss_character(zone, b, faction, seed) {
            Some(c) => Some(&c.class),
            None => zone.bosses.get(b)?.looks_like.as_ref().map(|l| l.class.as_str()),
        }
    }

    /// The spell boss number `b` of `zone` casts from range instead of swinging: its
    /// character's own, else Starfire in Moonkin Form, else none in another
    /// form, else its class's (its character's, else its stand-in's); `None` swings.
    pub fn boss_spell(&self, zone: &Zone, b: usize, faction: Faction, seed: u64) -> Option<u32> {
        let (own, form) = match self.boss_character(zone, b, faction, seed) {
            Some(c) => (c.boss_spell, c.form),
            None => (None, zone.bosses.get(b)?.looks_like.as_ref()?.form),
        };
        match (own, form) {
            (Some(spell), _) => Some(spell),
            (None, Some(PlayerForm::Moonkin)) => Some(MOONKIN_BOSS_SPELL),
            (None, Some(_)) => None,
            (None, None) => self.pvp_looks.of(self.boss_class(zone, b, faction, seed)?)?.boss_spell,
        }
    }

    /// The abilities boss number `b` of `zone` fights with for heroes fighting `faction`'s
    /// players in a run of seed `seed`: the first `class_abilities`
    /// of its class's ([`Self::boss_kit`]), then the zone's own `abilities`.
    pub fn boss_abilities(&self, zone: &Zone, b: usize, faction: Faction, seed: u64) -> Vec<BossAbility> {
        let Some(boss) = zone.bosses.get(b) else { return Vec::new() };
        let n = boss.class_abilities as usize;
        let mut out: Vec<BossAbility> = match (&self.enemy_kinds, n) {
            (Some(kinds), 1..) => {
                let mut problems = Vec::new();
                self.boss_kit(zone, b, faction, seed)
                    .iter()
                    .take(n)
                    .map(|a| kinds::resolve_ability(&zone.id, &boss.name, b, a, kinds, &mut problems))
                    .collect()
            }
            _ => Vec::new(),
        };
        out.extend(boss.abilities.iter().cloned());
        out
    }

    /// The class abilities boss number `b` of `zone` draws its `class_abilities` from: its
    /// character's own, else its class's in its druid form, else its class's
    /// (`pvp_looks.ron`; its character's class, else its stand-in's).
    pub fn boss_kit(&self, zone: &Zone, b: usize, faction: Faction, seed: u64) -> &[BossAbilityEntry] {
        let (own, form) = match self.boss_character(zone, b, faction, seed) {
            Some(c) => (c.boss_abilities.as_slice(), c.form),
            None => (&[][..], zone.bosses.get(b).and_then(|x| x.looks_like.as_ref()).and_then(|l| l.form)),
        };
        if !own.is_empty() {
            return own;
        }
        let Some(looks) = self.boss_class(zone, b, faction, seed).and_then(|c| self.pvp_looks.of(c)) else { return &[] };
        match form.and_then(|f| looks.form_boss_abilities.iter().find(|(g, _)| *g == f)) {
            Some((_, kit)) => kit,
            None => &looks.boss_abilities,
        }
    }

    /// The class abilities' rules: a boss that takes some looks like a
    /// player or is played by one, and every class's (each form's, each character's own) has as
    /// many as the most a boss takes, none a Summon (a boss's adds are its zone's).
    fn check_class_boss_abilities(&self, problems: &mut Vec<String>) {
        let mut need = 0;
        for z in &self.zones {
            for b in z.every_boss().filter(|b| b.class_abilities > 0) {
                need = need.max(b.class_abilities as usize);
                if b.looks_like.is_none() && b.played_by.is_none() {
                    problems.push(format!("zone {}: boss {} takes class abilities but looks like no class", z.id, b.name));
                }
            }
        }
        let mut kits: Vec<(String, &[BossAbilityEntry])> = Vec::new();
        for c in &self.pvp_looks.classes {
            kits.push((format!("class {}", c.class), &c.boss_abilities));
            kits.extend(c.form_boss_abilities.iter().map(|(f, k)| (format!("class {} in {f:?} form", c.class), k.as_slice())));
        }
        kits.extend(self.player_characters.iter().filter(|c| !c.boss_abilities.is_empty()).map(|c| (format!("player character {}", c.id), c.boss_abilities.as_slice())));
        for (what, kit) in kits {
            if kit.iter().any(|a| matches!(a, BossAbilityEntry::Summon { .. })) {
                problems.push(format!("{what}: its boss abilities hold a Summon, which only a zone writes"));
            }
            if kit.len() < need {
                problems.push(format!("{what}: {} boss abilities, but a boss takes {need}", kit.len()));
            }
        }
    }

    /// The building blocks' rules: every roster enemy a boss's script or
    /// ability names (a wave, a hatchling, what the dead rise as) is in its zone's roster.
    fn check_boss_mechanics(&self, problems: &mut Vec<String>) {
        for z in &self.zones {
            for b in &z.bosses {
                let named = b
                    .abilities
                    .iter()
                    .flat_map(|a| a.kind.enemies_named())
                    .chain(b.script.iter().flat_map(|l| l.run.iter().flat_map(|a| a.enemies_named()).chain(l.add.as_deref())));
                for name in named {
                    if !z.enemies.iter().any(|e| e.name == name) {
                        problems.push(format!("zone {}: boss {} names {name}, which its roster lacks", z.id, b.name));
                    }
                }
                // the bosses a script calls, revives or acts as are the zone's own
                for name in b.script.iter().flat_map(|l| l.run.iter().flat_map(|a| a.bosses_named())) {
                    if !z.bosses.iter().any(|o| o.name == name) {
                        problems.push(format!("zone {}: boss {} names boss {name}, which the zone lacks", z.id, b.name));
                    }
                }
                // the doors a script opens or shuts are the zone's own
                for name in b.script.iter().flat_map(|l| l.run.iter().flat_map(|a| a.doors_named())) {
                    if !z.doors.iter().any(|d| d.name == name) {
                        problems.push(format!("zone {}: boss {} names door {name}, which the zone lacks", z.id, b.name));
                    }
                }
                for l in &b.script {
                    if matches!(l.on, crate::ScriptTrigger::AddDied | crate::ScriptTrigger::LastAddDied) != l.add.is_some() {
                        problems.push(format!("zone {}: boss {}'s script line on {:?} names an add only for AddDied and LastAddDied", z.id, b.name, l.on));
                    }
                }
                // a class call names classes there are
                fn calls(a: &ScriptAction, out: &mut Vec<String>) {
                    if let ScriptAction::ClassCall { calls: c, .. } = a {
                        out.extend(c.iter().map(|c| c.class.clone()));
                    }
                    a.nested().into_iter().for_each(|n| calls(n, out));
                }
                let mut called = Vec::new();
                b.script.iter().flat_map(|l| &l.run).for_each(|a| calls(a, &mut called));
                for class in called.iter().filter(|c| !self.classes.iter().any(|k| &k.id == *c)) {
                    problems.push(format!("zone {}: boss {}'s class call names {class}, no class", z.id, b.name));
                }
                // an object is shown one way, and only a unit wears an aura look
                for o in b.ground_objects() {
                    if o.planted_item.is_some() && o.creature_display.is_some() {
                        problems.push(format!("zone {}: boss {}'s {} is both a planted item and a creature model", z.id, b.name, o.name));
                    }
                    if o.aura_look.is_some() && o.planted_item.is_none() && o.creature_display.is_none() {
                        problems.push(format!("zone {}: boss {}'s {} wears an aura look but is no unit", z.id, b.name, o.name));
                    }
                }
            }
        }
        // one composed look per planted item: the same item stands the same way everywhere
        let planted: Vec<&PlantedItem> =
            self.zones.iter().flat_map(|z| &z.bosses).flat_map(|b| b.ground_objects()).filter_map(|o| o.planted_item.as_ref()).collect();
        for (i, p) in planted.iter().enumerate() {
            if planted[..i].iter().any(|q| q.display == p.display && q != p) {
                problems.push(format!("item display {} is planted two ways", p.display));
            }
        }
    }

    /// The hand-made player character `id`.
    pub fn player_character(&self, id: &str) -> Option<&PlayerCharacter> {
        self.player_characters.iter().find(|c| c.id == id)
    }

    /// The hand-made player characters of `faction` (their race's).
    pub fn player_characters_of(&self, faction: Faction) -> impl Iterator<Item = &PlayerCharacter> {
        self.player_characters.iter().filter(move |c| self.races.iter().any(|r| r.id == c.race && r.faction == faction))
    }

    /// The faction of hand-made character `c` (its race's).
    pub fn character_faction(&self, c: &PlayerCharacter) -> Option<Faction> {
        self.races.iter().find(|r| r.id == c.race).map(|r| r.faction)
    }

    /// Class `class`'s default assist in form `form`: the form's own, else the class's.
    pub fn class_assist(&self, class: &str, form: Option<PlayerForm>) -> Option<&ClassAssist> {
        let of = |f: Option<PlayerForm>| self.class_assists.iter().find(|a| a.class == class && a.form == f);
        form.and_then(|f| of(Some(f))).or_else(|| of(None))
    }

    /// The assist hand-made character `c` performs: its own script, else its class's (and
    /// form's) default.
    pub fn assist_of<'a>(&'a self, c: &'a PlayerCharacter) -> Option<&'a AssistScript> {
        c.assist_script.as_ref().or_else(|| self.class_assist(&c.class, c.form).map(|a| &a.script))
    }

    /// What hand-made character `c`'s assist does, as a player reads it: its own `assist` text,
    /// else (with no text and no script of its own) its default's.
    pub fn assist_text<'a>(&'a self, c: &'a PlayerCharacter) -> &'a str {
        match self.class_assist(&c.class, c.form) {
            Some(a) if c.assist.is_empty() && c.assist_script.is_none() => &a.text,
            _ => &c.assist,
        }
    }

    /// The hand-made characters still on their class's default assist (no script of their own),
    /// by id, each with the default it takes ("druid (Cat)"): the ones that still need one.
    pub fn characters_on_default_assist(&self) -> Vec<(String, String)> {
        let on_default = self.player_characters.iter().filter(|c| c.assist_script.is_none());
        on_default.map(|c| (c.id.clone(), self.default_name(c))).collect()
    }

    /// Hand-made character `c`'s rotation and movement after its assist: its own `behaviour`,
    /// else its class's (and form's) default.
    pub fn behaviour_of<'a>(&'a self, c: &'a PlayerCharacter) -> Option<&'a AssistBehaviour> {
        c.behaviour.as_ref().or_else(|| self.class_assist(&c.class, c.form).map(|a| &a.behaviour))
    }

    /// Rotation spell `spell`'s row in `assist_spells.ron`.
    pub fn assist_spell(&self, spell: u32) -> Option<&AssistSpell> {
        self.assist_spells.iter().find(|s| s.spell == spell)
    }

    /// The hand-made characters still on their class's default behaviour (no `behaviour` of their
    /// own), by id, each with the default it takes ("druid (Cat)").
    pub fn characters_on_default_behaviour(&self) -> Vec<(String, String)> {
        self.player_characters.iter().filter(|c| c.behaviour.is_none()).map(|c| (c.id.clone(), self.default_name(c))).collect()
    }

    /// The name of the class default character `c` takes: its class, with its form when the
    /// form has a default of its own ("druid (Cat)").
    fn default_name(&self, c: &PlayerCharacter) -> String {
        match self.class_assist(&c.class, c.form).and_then(|a| a.form) {
            Some(f) => format!("{} ({f:?})", c.class),
            None => c.class.clone(),
        }
    }

    /// Every class has a default assist, every default's class (and form) is written once, and
    /// every script, the characters' own and the defaults, follows the blocks' rules; every
    /// behaviour casts spells `assist_spells.ron` has.
    fn check_assists(&self, problems: &mut Vec<String>) {
        let longest = self.balance.battle_standard.assist_seconds;
        let pet_known = |name: &str| self.pets.iter().any(|p| p.name == name);
        for (k, a) in self.class_assists.iter().enumerate() {
            if self.class(&a.class).is_none() {
                problems.push(format!("class assist {}: classes.ron has no such class", a.class));
            }
            if self.class_assists[..k].iter().any(|o| o.class == a.class && o.form == a.form) {
                problems.push(format!("class assist {} {:?} is written twice", a.class, a.form));
            }
            let what = format!("class assist {} {:?}", a.class, a.form);
            check_script(&what, &a.script, longest, &pet_known, problems);
            check_behaviour(&what, &a.behaviour, &self.assist_spells, problems);
        }
        check_assist_spells(&self.assist_spells, problems);
        if !self.class_assists.is_empty() {
            for c in &self.classes {
                if self.class_assist(&c.id, None).is_none() {
                    problems.push(format!("class {} has no default assist in class_assists.ron", c.id));
                }
            }
        }
        for c in &self.player_characters {
            let what = format!("player character {}", c.id);
            if let Some(b) = &c.behaviour {
                check_behaviour(&what, b, &self.assist_spells, problems);
            }
            match &c.assist_script {
                Some(s) => check_script(&what, s, longest, &pet_known, problems),
                None if !self.class_assists.is_empty() && self.class_assist(&c.class, c.form).is_none() => {
                    problems.push(format!("{what} has no assist script and its class {} no default", c.class))
                }
                None => {}
            }
        }
    }

    /// The battle standard's rules: a count range, positive times and size, known zones and
    /// real looks.
    fn check_battle_standard(&self, problems: &mut Vec<String>) {
        let b = &self.balance.battle_standard;
        if b.per_run.0 > b.per_run.1 || !(b.endless_every > 0.0) || !(b.ally_scale > 0.0) || !(b.assist_seconds > 0.0) {
            problems.push(format!("balance.ron battle_standard: {b:?} needs per_run low to high and positive times and size"));
        }
        let rates = b.assist_damage_per_second;
        if ![rates.normal, rates.elite_rare_boss].iter().all(|r| *r > 0.0 && *r <= 1.0) {
            problems.push(format!("balance.ron battle_standard: assist_damage_per_second {rates:?} needs shares above 0, at most 1"));
        }
        for z in &b.never_in {
            if self.zone(z).is_none() {
                problems.push(format!("balance.ron battle_standard: never_in names zone {z}, which content/zones lacks"));
            }
        }
        if [b.display.alliance, b.display.horde, b.planted_look.alliance, b.planted_look.horde].contains(&0) {
            problems.push("balance.ron battle_standard: a look is 0".into());
        }
    }

    /// Whether a battle standard may lie in zone `zone` (never in Warsong Gulch, whose bosses the
    /// characters it summons are).
    pub fn battle_standards_in(&self, zone: &str) -> bool {
        !self.balance.battle_standard.never_in.iter().any(|z| z == zone)
    }
}

/// Shows names and texts as the game shows them, keeping every pointer the read tables lack.
struct NameShower<'a> {
    names: &'a ClientNames,
    problems: Vec<String>,
}

impl NameShower<'_> {
    fn report(&mut self, what: &str, missing: Vec<ClientName>) {
        let lacked = missing.into_iter().map(|m| format!("{what}: the client has no {} row {}", m.table(), m.id()));
        self.problems.extend(lacked);
    }

    /// A name as shown; our text may hold pointers too (`"{Spell(779)} (Bear)"`: the client's name
    /// with our own words beside it).
    fn name(&mut self, what: &str, name: &NameRef) -> String {
        let mut missing = Vec::new();
        let shown = match name {
            NameRef::Text(text) => self.names.show_text(text, &mut missing),
            pointer => self.names.show(pointer, &mut missing),
        };
        self.report(what, missing);
        shown
    }

    fn text(&mut self, what: &str, text: &mut String) {
        let mut missing = Vec::new();
        *text = self.names.show_text(text, &mut missing);
        self.report(what, missing);
    }
}

/// Shows the pointers written in the legendary amulets' texts, the player characters' and class
/// defaults' assists and
/// the loadouts' strengths, drawbacks and suits (`{Spell(1856)}`), as
/// [`show_names`] does for the other records' texts. Their own names stay
/// our text (items and characters).
fn show_prose(
    names: &ClientNames,
    legendary_amulets: &mut [LegendaryAmulet],
    player_characters: &mut [PlayerCharacter],
    class_assists: &mut [ClassAssist],
    loadouts: &mut [ClassLoadouts],
) -> Result<()> {
    let mut show = NameShower { names, problems: Vec::new() };
    for l in legendary_amulets.iter_mut() {
        show.text(&format!("legendary {}", l.name), &mut l.text);
    }
    for p in player_characters.iter_mut() {
        show.text(&format!("player character {}", p.name), &mut p.assist);
    }
    for a in class_assists.iter_mut() {
        show.text(&format!("class assist {}", a.class), &mut a.text);
    }
    for c in loadouts.iter_mut() {
        for l in c.loadouts.iter_mut() {
            let what = format!("loadout {}/{}", c.class, l.id);
            for text in [&mut l.strength, &mut l.drawback, &mut l.suits] {
                show.text(&what, text);
            }
        }
    }
    if !show.problems.is_empty() {
        bail!("content names {} pointer(s) the client lacks:\n  {}", show.problems.len(), show.problems.join("\n  "));
    }
    Ok(())
}

/// Fills every record's shown name from the name its file writes (our text as it is, a pointer as
/// `names` has it), and shows the pointers written in its texts. Every pointer the read tables
/// lack is reported at once.
fn show_names(
    names: &ClientNames,
    classes: &mut [Class],
    abilities: &mut [Ability],
    talent_trees: &mut [TalentTree],
    races: &mut [Race],
    words: &mut Words,
) -> Result<()> {
    let mut show = NameShower { names, problems: Vec::new() };
    for c in classes.iter_mut() {
        let what = format!("class {}", c.id);
        c.name = show.name(&what, &c.name_ref);
        show.text(&what, &mut c.description);
        for o in c.starting_options.iter_mut() {
            let what = format!("class {} starting option {}", c.id, o.id);
            o.name = show.name(&what, &o.name_ref);
            show.text(&what, &mut o.description);
            o.passive.name = show.name(&what, &o.passive.name_ref);
            show.text(&what, &mut o.passive.description);
        }
    }
    for a in abilities.iter_mut() {
        let what = format!("ability {}", a.id);
        a.name = show.name(&what, &a.name_ref);
        show.text(&what, &mut a.description);
    }
    for tree in talent_trees.iter_mut() {
        tree.name = show.name(&format!("a talent tree of {}", tree.class), &tree.name_ref);
    }
    for t in talent_trees.iter_mut().flat_map(|t| t.talents.iter_mut()) {
        let what = format!("talent {}", t.id);
        t.name = show.name(&what, &t.name_ref);
        show.text(&what, &mut t.text);
    }
    for r in races.iter_mut() {
        let what = format!("race {}", r.id);
        r.name = show.name(&what, &r.name_ref);
        for p in r.passives.iter_mut() {
            p.name = show.name(&what, &p.name_ref);
            p.stock_spell = p.stock_spell_ref.as_ref().map(|s| show.name(&what, s));
            show.text(&what, &mut p.text);
        }
    }
    for b in words.buffs.iter_mut() {
        b.name = show.name(&format!("buff words {:?}", b.kind), &b.name_ref);
    }
    if !show.problems.is_empty() {
        bail!("content names {} pointer(s) the client lacks:\n  {}", show.problems.len(), show.problems.join("\n  "));
    }
    Ok(())
}
