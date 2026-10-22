//! Shared numbers, defined once (`content/enemy_kinds.ron`): what each
//! kind of roster enemy moves, swings and costs, and what each kind of boss ability does. A
//! zone's roster entries and boss abilities in its `zones/<id>.ron` name a kind and keep only what is their
//! own (looks, toughness, unlock minute); [`resolve_zones`] turns them into the full [`Zone`]
//! records the game plays, at load time.

use std::collections::BTreeMap;
use std::fmt;

use anyhow::{bail, Result};
use serde::{Deserialize, Deserializer, Serialize};

use crate::schema::{Boss, BossAbility, BossAbilityKind, CreatureType, Enemy, RangedAttack, Spawn, Weather, Zone};
use crate::names::{ClientName, ClientNames, NameRef};

/// The name of an enemy kind, written bare in RON (`kind: Fodder`); which names exist is up to
/// `enemy_kinds.ron`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KindName(pub String);

impl<'de> Deserialize<'de> for KindName {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Name;
        impl serde::de::Visitor<'_> for Name {
            type Value = KindName;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an enemy kind name, such as Fodder")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<KindName, E> {
                Ok(KindName(v.to_string()))
            }
        }
        d.deserialize_identifier(Name)
    }
}

impl fmt::Display for KindName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `enemy_kinds.ron`: the roster kinds by name, and the boss abilities' standard numbers.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EnemyKinds {
    pub enemies: BTreeMap<KindName, EnemyKind>,
    /// Yards per second every boss moves at, unless it sets its own `move_speed`.
    pub boss_move_speed: f32,
    pub boss_abilities: BossAbilityNumbers,
}

/// One roster kind's numbers.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EnemyKind {
    /// Yards per second.
    pub move_speed: f32,
    /// Seconds between attacks.
    pub attack_interval: f32,
    /// Cost against the spawn director's budget, and what a kill adds to its kill pressure.
    pub spawn_cost: f32,
    /// Elites come on their own timer and use the elite kind of the enemy formula.
    #[serde(default)]
    pub elite: bool,
    /// Casters: how far they cast from and how long they stand casting. An entry of a ranged kind
    /// names its spell's look (`look_spell`).
    #[serde(default)]
    pub ranged: Option<RangedKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct RangedKind {
    pub range: f32,
    pub cast_time: f32,
}

/// Each boss ability kind's standard numbers.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BossAbilityNumbers {
    pub charge: ChargeNumbers,
    pub summon: SummonNumbers,
    pub stomp: StompNumbers,
    pub volley: VolleyNumbers,
    pub ground_burst: GroundBurstNumbers,
    /// The building blocks' standard numbers.
    pub marked: MarkedNumbers,
    pub rush: RushNumbers,
    pub lay: LayNumbers,
    pub zone: ZoneNumbers,
}

/// A marked attack's: its cooldown, wind-up (long enough to walk out), damage, and the two looks
/// every mark is drawn with unless it names its own: the warning laid over the whole shape and
/// the fill that grows inside it as it winds up.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct MarkedNumbers {
    pub cooldown: f32,
    pub wind_up: f32,
    pub damage_mult: f32,
    pub warning_spell: u32,
    pub fill_spell: u32,
}

/// A marked rush's: as a mark's, and its line's length and width.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct RushNumbers {
    pub cooldown: f32,
    pub wind_up: f32,
    pub damage_mult: f32,
    pub length: f32,
    pub width: f32,
}

/// Laying objects: cooldown and how many.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct LayNumbers {
    pub cooldown: f32,
    pub count: u32,
}

/// Putting down a lasting zone: cooldown.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct ZoneNumbers {
    pub cooldown: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct ChargeNumbers {
    pub cooldown: f32,
    pub damage_mult: f32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SummonNumbers {
    pub cooldown: f32,
    /// Adds per cast for a zone's first, second … boss (its place in the zone's `bosses` list).
    pub adds_by_position: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct StompNumbers {
    pub cooldown: f32,
    pub radius: f32,
    pub damage_mult: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct VolleyNumbers {
    pub cooldown: f32,
    pub count: u32,
    pub damage_mult: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct GroundBurstNumbers {
    pub cooldown: f32,
    pub radius: f32,
    pub delay: f32,
    pub damage_mult: f32,
}

// ---- zone files as written ---------------------------------------------------------------------

/// A zone as its file `zones/<id>.ron` writes it: the roster and bosses name kinds instead of carrying numbers.
#[derive(Debug, Clone, Deserialize)]
pub struct ZoneEntry {
    pub id: String,
    /// Our text, or a pointer to the client's name (`Area(12)`).
    pub name: NameRef,
    pub subtitle: String,
    pub description: String,
    pub map_id: u32,
    pub area_id: u32,
    pub spawn: Spawn,
    pub arena_radius: f32,
    #[serde(default)]
    pub arena_centre: Option<(f32, f32)>,
    #[serde(default)]
    pub walls: Vec<crate::Wall>,
    #[serde(default)]
    pub doors: Vec<crate::Door>,
    #[serde(default)]
    pub props: Vec<crate::Prop>,
    #[serde(default)]
    pub platforms: Vec<crate::Platform>,
    #[serde(default = "crate::schema::enemy_spawn_ring")]
    pub enemy_spawn_ring: (f32, f32),
    #[serde(default = "crate::schema::boss_spawn_ring")]
    pub boss_spawn_ring: (f32, f32),
    pub hour: u32,
    pub weather: Weather,
    pub difficulty: f32,
    pub enemies: Vec<EnemyEntry>,
    pub bosses: Vec<BossEntry>,
    /// A roster for each faction the heroes may be, as written.
    #[serde(default)]
    pub by_faction: Option<FactionRosterEntries>,
}

/// A zone's roster for each faction, as written: `vs_alliance` what Alliance heroes fight,
/// `vs_horde` what Horde heroes fight.
#[derive(Debug, Clone, Deserialize)]
pub struct FactionRosterEntries {
    pub vs_alliance: RosterEntries,
    pub vs_horde: RosterEntries,
}

/// One roster as written.
#[derive(Debug, Clone, Deserialize)]
pub struct RosterEntries {
    pub enemies: Vec<EnemyEntry>,
    pub bosses: Vec<BossEntry>,
}

/// One roster entry as written: its kind, and what is its own. The optional overrides replace
/// the kind's number for this enemy only.
#[derive(Debug, Clone, Deserialize)]
pub struct EnemyEntry {
    pub name: String,
    #[serde(default)]
    pub display_id: u32,
    pub scale: f32,
    pub creature_type: CreatureType,
    pub kind: KindName,
    #[serde(default = "one")]
    pub toughness: f32,
    pub appears_at_minute: f32,
    /// The WoW spell a caster casts; required for a ranged kind, refused for the others.
    #[serde(default)]
    pub look_spell: Option<u32>,
    #[serde(default)]
    pub move_speed: Option<f32>,
    #[serde(default)]
    pub attack_interval: Option<f32>,
    #[serde(default)]
    pub spawn_cost: Option<f32>,
    #[serde(default)]
    pub range: Option<f32>,
    #[serde(default)]
    pub cast_time: Option<f32>,
    /// A player-looking enemy.
    #[serde(default)]
    pub looks_like: Option<crate::PlayerLook>,
    /// The PvP rank (1 to 14) whose title it carries before a made-up player name.
    #[serde(default)]
    pub rank: Option<u32>,
    /// What it holds, as the WoW creature of its name does (vmangos).
    #[serde(default)]
    pub weapons: crate::CreatureWeapons,
    /// Only a boss's script or ability brings it.
    #[serde(default)]
    pub only_summoned: bool,
}

/// A boss as written: its abilities name a kind and its looks.
#[derive(Debug, Clone, Deserialize)]
pub struct BossEntry {
    pub name: String,
    #[serde(default)]
    pub display_id: u32,
    pub scale: f32,
    pub creature_type: CreatureType,
    pub spawn_minute: f32,
    /// Its own speed instead of `boss_move_speed`; none uses one today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_speed: Option<f32>,
    pub yell: String,
    pub abilities: Vec<BossAbilityEntry>,
    pub is_final: bool,
    #[serde(default)]
    pub at: Option<Spawn>,
    #[serde(default)]
    pub health_share: Option<f32>,
    #[serde(default)]
    pub carries: Option<crate::FactionSpell>,
    #[serde(default)]
    pub played_by: Option<crate::CharacterPick>,
    #[serde(default)]
    pub looks_like: Option<crate::PlayerLook>,
    #[serde(default)]
    pub rank: Option<u32>,
    /// How many of its class's boss abilities (`pvp_looks.ron`) it uses before its own
    /// (Warsong's player-looking bosses fight as their class).
    #[serde(default)]
    pub class_abilities: u32,
    /// The original boss's pull voice line (`SoundEntries.dbc` id), played with its yell.
    #[serde(default)]
    pub aggro_sound: Option<u32>,
    /// What it holds, as the WoW creature of its name does (vmangos).
    #[serde(default)]
    pub weapons: crate::CreatureWeapons,
    /// Its script (`crate::ScriptLine`).
    #[serde(default)]
    pub script: Vec<crate::ScriptLine>,
    /// A stack its melee blows leave on the hero.
    #[serde(default)]
    pub on_hit: Option<crate::Affliction>,
    /// Its melee swing hits this many times as hard (a reworked boss
    /// keeps the damage a hero cannot avoid; the swing takes over what its unmarked abilities
    /// did). 1 when left out.
    #[serde(default)]
    pub swing_mult: Option<f32>,
    /// Only a script brings it (`Call`): the schedule passes it by
    /// (Whitemane comes when Mograine falls).
    #[serde(default)]
    pub called: bool,
}

/// One boss ability as written: the kind and its looks; every number comes from
/// `enemy_kinds.ron` unless overridden here (`cooldown: Some(10.0)`, `radius: Some(12.0)` …).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BossAbilityEntry {
    GroundBurst {
        telegraph_spell: u32,
        impact_spell: u32,
        #[serde(default)]
        radius: Option<f32>,
        #[serde(default)]
        delay: Option<f32>,
        #[serde(default)]
        damage_mult: Option<f32>,
        #[serde(default)]
        cooldown: Option<f32>,
    },
    Volley {
        look_spell: u32,
        #[serde(default)]
        count: Option<u32>,
        #[serde(default)]
        damage_mult: Option<f32>,
        #[serde(default)]
        cooldown: Option<f32>,
        /// Each shot plays on an invisible target unit where it lands (`Marked`'s, docs/game/bosses.md).
        #[serde(default)]
        impact_on_bunny: bool,
    },
    Summon {
        #[serde(default)]
        count: Option<u32>,
        #[serde(default)]
        cooldown: Option<f32>,
    },
    Stomp {
        look_spell: u32,
        #[serde(default)]
        radius: Option<f32>,
        #[serde(default)]
        damage_mult: Option<f32>,
        #[serde(default)]
        cooldown: Option<f32>,
    },
    Charge {
        look_spell: u32,
        #[serde(default)]
        damage_mult: Option<f32>,
        #[serde(default)]
        cooldown: Option<f32>,
    },
    /// A marked attack (`crate::BossAbilityKind::Marked`); its looks default to `marked`'s.
    Marked {
        shape: crate::Shape,
        at: crate::MarkAt,
        impact_spell: u32,
        #[serde(default)]
        warning_spell: Option<u32>,
        #[serde(default)]
        fill_spell: Option<u32>,
        #[serde(default)]
        wind_up: Option<f32>,
        #[serde(default)]
        damage_mult: Option<f32>,
        #[serde(default)]
        cooldown: Option<f32>,
        #[serde(default)]
        leaves: Option<crate::LastingZone>,
        #[serde(default)]
        leaves_object: Option<crate::GroundObject>,
        #[serde(default)]
        missile: bool,
        #[serde(default)]
        warn: Option<crate::Warning>,
        #[serde(default)]
        from: Option<String>,
        #[serde(default)]
        impact_on_bunny: bool,
        #[serde(default)]
        chain: Option<f32>,
        #[serde(default)]
        brings: Option<crate::Brings>,
        #[serde(default)]
        sweep: Option<crate::Sweep>,
    },
    /// A marked rush (`crate::BossAbilityKind::Rush`).
    Rush {
        look_spell: u32,
        #[serde(default)]
        fade_spell: Option<u32>,
        #[serde(default)]
        length: Option<f32>,
        #[serde(default)]
        width: Option<f32>,
        #[serde(default)]
        wind_up: Option<f32>,
        #[serde(default)]
        damage_mult: Option<f32>,
        #[serde(default)]
        cooldown: Option<f32>,
        #[serde(default)]
        warn: Option<crate::Warning>,
    },
    /// Objects put down (`crate::BossAbilityKind::Lay`).
    Lay {
        object: crate::GroundObject,
        near: crate::ObjectNear,
        #[serde(default)]
        count: Option<u32>,
        #[serde(default)]
        cooldown: Option<f32>,
        #[serde(default)]
        warn: Option<crate::Warning>,
    },
    /// A lasting zone put down (`crate::BossAbilityKind::Zone`).
    Zone {
        zone: crate::LastingZone,
        at: crate::MarkAt,
        #[serde(default)]
        cooldown: Option<f32>,
        #[serde(default)]
        warn: Option<crate::Warning>,
    },
}

fn one() -> f32 {
    1.0
}

// ---- resolving --------------------------------------------------------------------------------

/// Builds the full zones from the zone files' entries and the kinds' numbers, each zone's name as
/// `names` shows it; every problem found (an unknown kind, a caster without a spell look, a name
/// the client lacks …) is reported at once.
pub fn resolve_zones(entries: Vec<ZoneEntry>, kinds: &EnemyKinds, names: &ClientNames) -> Result<Vec<Zone>> {
    let mut problems = Vec::new();
    let zones = entries.into_iter().enumerate().map(|(i, z)| resolve_zone(i, z, kinds, names, &mut problems)).collect();
    if !problems.is_empty() {
        bail!("zones have {} problem(s):\n  {}", problems.len(), problems.join("\n  "));
    }
    Ok(zones)
}

/// Zone `z`, at place `index` in difficulty order (its creatures' entries count from it).
fn resolve_zone(index: usize, z: ZoneEntry, kinds: &EnemyKinds, names: &ClientNames, problems: &mut Vec<String>) -> Zone {
    let mut missing: Vec<ClientName> = Vec::new();
    let name = names.show(&z.name, &mut missing);
    let subtitle = names.show_text(&z.subtitle, &mut missing);
    let description = names.show_text(&z.description, &mut missing);
    problems.extend(missing.into_iter().map(|m| format!("zone {}: the client has no {} row {}", z.id, m.table(), m.id())));
    let (enemies, bosses) = resolve_roster(index, &z.id, z.enemies, z.bosses, kinds, names, problems);
    let by_faction = z.by_faction.map(|r| {
        let side = |side: usize, roster: RosterEntries, problems: &mut Vec<String>| {
            let place = crate::entries::faction_roster_place(index, side);
            let (enemies, bosses) = resolve_roster(place, &z.id, roster.enemies, roster.bosses, kinds, names, problems);
            crate::schema::Roster { enemies, bosses }
        };
        crate::schema::FactionRosters { vs_alliance: side(0, r.vs_alliance, problems), vs_horde: side(1, r.vs_horde, problems) }
    });
    Zone {
        id: z.id,
        name,
        subtitle,
        description,
        map_id: z.map_id,
        area_id: z.area_id,
        spawn: z.spawn,
        arena_radius: z.arena_radius,
        arena_centre: z.arena_centre,
        walls: z.walls,
        doors: z.doors,
        props: z.props,
        platforms: z.platforms,
        enemy_spawn_ring: z.enemy_spawn_ring,
        boss_spawn_ring: z.boss_spawn_ring,
        hour: z.hour,
        weather: z.weather,
        difficulty: z.difficulty,
        enemies,
        bosses,
        by_faction,
    }
}

/// A roster's enemies and bosses, their entries numbered as the zone at `place`'s.
fn resolve_roster(
    place: usize,
    zone: &str,
    enemies: Vec<EnemyEntry>,
    bosses: Vec<BossEntry>,
    kinds: &EnemyKinds,
    names: &ClientNames,
    problems: &mut Vec<String>,
) -> (Vec<Enemy>, Vec<Boss>) {
    let enemies = enemies
        .into_iter()
        .enumerate()
        .map(|(at, e)| Enemy { entry: crate::entries::enemy_entry(place, at), ..resolve_enemy(zone, e, kinds, names, problems) })
        .collect();
    let bosses = bosses
        .into_iter()
        .enumerate()
        .map(|(position, b)| {
            let mut boss = Boss {
            abilities: b.abilities.iter().map(|a| resolve_ability(zone, &b.name, position, a, kinds, problems)).collect(),
            script: b.script.clone(),
            on_hit: b.on_hit,
            swing_mult: b.swing_mult.unwrap_or(1.0),
            called: b.called,
            name: shown_name(zone, &b.name, names, problems),
            entry: crate::entries::boss_entry(place, position),
            display_id: b.display_id,
            scale: b.scale,
            creature_type: b.creature_type,
            spawn_minute: b.spawn_minute,
            move_speed: b.move_speed.unwrap_or(kinds.boss_move_speed),
            yell: b.yell,
            is_final: b.is_final,
            at: b.at,
            health_share: b.health_share.unwrap_or(1.0),
            carries: b.carries,
            played_by: b.played_by,
            looks_like: b.looks_like,
            rank: b.rank.map(|r| rank_titles(zone, r, names, problems)),
            class_abilities: b.class_abilities,
            aggro_sound: b.aggro_sound,
            weapons: checked_weapons(zone, &b.name, b.weapons, problems),
            };
            let shown = |n: &str, problems: &mut Vec<String>| shown_name(zone, n, names, problems);
            resolve_mechanics(zone, &b.name, position, &mut boss, kinds, &shown, problems);
            boss
        })
        .collect();
    (enemies, bosses)
}

/// A boss's script and its abilities' objects, resolved: each phase's kit takes the standard
/// numbers, each object's name is shown as the client names its pointers, and every line is
/// checked (a health share between 0 and 100, a pause of 5 s at most).
fn resolve_mechanics(
    zone: &str,
    boss: &str,
    position: usize,
    b: &mut Boss,
    kinds: &EnemyKinds,
    shown: &dyn Fn(&str, &mut Vec<String>) -> String,
    problems: &mut Vec<String>,
) {
    for a in &mut b.abilities {
        resolve_kind_objects(zone, boss, position, &mut a.kind, kinds, shown, problems);
    }
    for line in &mut b.script {
        if let crate::ScriptTrigger::Health(pct) = line.on {
            if !(0.0..100.0).contains(&pct) {
                problems.push(format!("zone {zone}: boss {boss}'s script runs at {pct} % health, not between 0 and 100"));
            }
        }
        if let Some(w) = &mut line.warn {
            show_warning(w, shown, problems);
        }
        if let Some(soon) = &mut line.soon {
            *soon = shown(soon, problems);
        }
        resolve_actions(zone, boss, position, &mut line.run, kinds, shown, problems);
    }
}

/// A warning's text and bar label, shown as the client names their pointers (`{Spell(..)}`).
fn show_warning(w: &mut crate::Warning, shown: &dyn Fn(&str, &mut Vec<String>) -> String, problems: &mut Vec<String>) {
    w.text = shown(&w.text, problems);
    if let Some(bar) = &mut w.bar {
        *bar = shown(bar, problems);
    }
}

fn resolve_kind_objects(
    zone: &str,
    boss: &str,
    position: usize,
    kind: &mut BossAbilityKind,
    kinds: &EnemyKinds,
    shown: &dyn Fn(&str, &mut Vec<String>) -> String,
    problems: &mut Vec<String>,
) {
    match kind {
        BossAbilityKind::Marked { leaves_object: Some(o), .. } | BossAbilityKind::Lay { object: o, .. } => {
            resolve_object(zone, boss, position, o, kinds, shown, problems)
        }
        _ => {}
    }
    match kind {
        BossAbilityKind::Marked { warn: Some(w), .. }
        | BossAbilityKind::Rush { warn: Some(w), .. }
        | BossAbilityKind::Lay { warn: Some(w), .. }
        | BossAbilityKind::Zone { warn: Some(w), .. } => show_warning(w, shown, problems),
        _ => {}
    }
}

fn resolve_object(
    zone: &str,
    boss: &str,
    position: usize,
    o: &mut crate::GroundObject,
    kinds: &EnemyKinds,
    shown: &dyn Fn(&str, &mut Vec<String>) -> String,
    problems: &mut Vec<String>,
) {
    o.name = shown(&o.name, problems);
    if let Some(w) = &mut o.warn {
        show_warning(w, shown, problems);
    }
    resolve_actions(zone, boss, position, &mut o.on_contact, kinds, shown, problems);
    resolve_actions(zone, boss, position, &mut o.on_all_taken, kinds, shown, problems);
    for timed in [&mut o.pulse, &mut o.left_alone].into_iter().flatten() {
        resolve_actions(zone, boss, position, &mut timed.run, kinds, shown, problems);
    }
}

fn resolve_actions(
    zone: &str,
    boss: &str,
    position: usize,
    actions: &mut [crate::ScriptAction],
    kinds: &EnemyKinds,
    shown: &dyn Fn(&str, &mut Vec<String>) -> String,
    problems: &mut Vec<String>,
) {
    for action in actions {
        match action {
            crate::ScriptAction::Kit { abilities, resolved } => {
                *resolved = abilities.iter().map(|a| resolve_ability(zone, boss, position, a, kinds, problems)).collect();
                for a in resolved.iter_mut() {
                    resolve_kind_objects(zone, boss, position, &mut a.kind, kinds, shown, problems);
                }
            }
            crate::ScriptAction::Place { object, .. } => resolve_object(zone, boss, position, object, kinds, shown, problems),
            crate::ScriptAction::Emote { text } => *text = shown(text, problems),
            crate::ScriptAction::Pause { seconds, text } if *seconds <= crate::LONGEST_PAUSE && *seconds > 0.0 => *text = shown(text, problems),
            crate::ScriptAction::Later { run, .. } | crate::ScriptAction::On { run, .. } => {
                resolve_actions(zone, boss, position, run, kinds, shown, problems)
            }
            crate::ScriptAction::Cast { ability, resolved } => {
                let mut a = resolve_ability(zone, boss, position, ability, kinds, problems);
                resolve_kind_objects(zone, boss, position, &mut a.kind, kinds, shown, problems);
                *resolved = Some(a);
            }
            crate::ScriptAction::Pause { seconds, .. } if *seconds > crate::LONGEST_PAUSE || *seconds <= 0.0 => {
                problems.push(format!("zone {zone}: boss {boss}'s scripted pause lasts {seconds} s, not up to {} s", crate::LONGEST_PAUSE));
            }
            crate::ScriptAction::Waves { waves, then, bar, counter, .. } => {
                if waves.is_empty() {
                    problems.push(format!("zone {zone}: boss {boss}'s run of waves has no wave"));
                }
                for w in waves.iter_mut() {
                    if let Some(warn) = &mut w.warn {
                        show_warning(warn, shown, problems);
                    }
                    resolve_actions(zone, boss, position, &mut w.run, kinds, shown, problems);
                }
                for label in bar.iter_mut().chain(counter.iter_mut()) {
                    *label = shown(label, problems);
                }
                resolve_actions(zone, boss, position, then, kinds, shown, problems);
            }
            crate::ScriptAction::ClassCall { calls, .. } => {
                for c in calls.iter_mut() {
                    show_warning(&mut c.warn, shown, problems);
                    resolve_actions(zone, boss, position, &mut c.run, kinds, shown, problems);
                }
            }
            crate::ScriptAction::Cameo { abilities, resolved, run, .. } => {
                *resolved = abilities.iter().map(|a| resolve_ability(zone, boss, position, a, kinds, problems)).collect();
                for a in resolved.iter_mut() {
                    resolve_kind_objects(zone, boss, position, &mut a.kind, kinds, shown, problems);
                }
                resolve_actions(zone, boss, position, run, kinds, shown, problems);
            }
            _ => {}
        }
    }
}

/// An enemy's or boss's name as written, its pointers (`{Class(1)}`) shown as the client names
/// them.
fn shown_name(zone: &str, name: &str, names: &ClientNames, problems: &mut Vec<String>) -> String {
    let mut missing = Vec::new();
    let shown = names.show_text(name, &mut missing);
    problems.extend(missing.into_iter().map(|m| format!("zone {zone}: the client has no {} row {}", m.table(), m.id())));
    shown
}

/// PvP rank `rank`'s titles as the client names them for each faction.
fn rank_titles(zone: &str, rank: u32, names: &ClientNames, problems: &mut Vec<String>) -> crate::RankTitles {
    if !(1..=14).contains(&rank) {
        problems.push(format!("zone {zone}: PvP rank {rank} is not one of 1 to 14"));
    }
    let mut missing = Vec::new();
    let alliance = names.show(&NameRef::Client(ClientName::AllianceRank(rank)), &mut missing);
    let horde = names.show(&NameRef::Client(ClientName::HordeRank(rank)), &mut missing);
    problems.extend(missing.into_iter().map(|m| format!("zone {zone}: the client has no {} row {}", m.table(), m.id())));
    crate::RankTitles { rank, alliance, horde }
}

fn resolve_enemy(zone: &str, e: EnemyEntry, kinds: &EnemyKinds, names: &ClientNames, problems: &mut Vec<String>) -> Enemy {
    let known = || kinds.enemies.keys().map(|k| k.0.as_str()).collect::<Vec<_>>().join(", ");
    let kind = kinds.enemies.get(&e.kind);
    if kind.is_none() {
        problems.push(format!("zone {zone}: enemy {} names unknown kind {} (enemy_kinds.ron has {})", e.name, e.kind, known()));
    }
    let ranged = match kind.and_then(|k| k.ranged) {
        Some(r) => match e.look_spell {
            Some(look_spell) => {
                Some(RangedAttack { look_spell, range: e.range.unwrap_or(r.range), cast_time: e.cast_time.unwrap_or(r.cast_time) })
            }
            None => {
                problems.push(format!("zone {zone}: enemy {} is of ranged kind {} but has no look_spell", e.name, e.kind));
                None
            }
        },
        None => {
            if kind.is_some() && (e.look_spell.is_some() || e.range.is_some() || e.cast_time.is_some()) {
                problems
                    .push(format!("zone {zone}: enemy {} has a look_spell, range or cast_time, but kind {} does not cast", e.name, e.kind));
            }
            None
        }
    };
    let (move_speed, attack_interval, spawn_cost, elite) =
        kind.map_or((0.0, 0.0, 0.0, false), |k| (k.move_speed, k.attack_interval, k.spawn_cost, k.elite));
    Enemy {
        name: shown_name(zone, &e.name, names, problems),
        entry: 0,
        display_id: e.display_id,
        scale: e.scale,
        creature_type: e.creature_type,
        toughness: e.toughness,
        attack_interval: e.attack_interval.unwrap_or(attack_interval),
        move_speed: e.move_speed.unwrap_or(move_speed),
        spawn_cost: e.spawn_cost.unwrap_or(spawn_cost),
        appears_at_minute: e.appears_at_minute,
        elite,
        ranged,
        looks_like: e.looks_like,
        rank: e.rank.map(|r| rank_titles(zone, r, names, problems)),
        weapons: checked_weapons(zone, &e.name, e.weapons, problems),
        only_summoned: e.only_summoned,
    }
}

/// `weapons` as written, each in a slot its kind is held in: a melee weapon in the main hand, a
/// melee weapon, shield or held item in the off hand, a bow, gun, crossbow, wand or thrown weapon
/// in the ranged slot.
fn checked_weapons(zone: &str, name: &str, weapons: crate::CreatureWeapons, problems: &mut Vec<String>) -> crate::CreatureWeapons {
    use crate::WeaponKind::*;
    let ranged = |k: crate::WeaponKind| matches!(k, Bow | Gun | Crossbow | Wand | Thrown);
    let fits = [
        weapons.main_hand.is_none_or(|w| w.kind.item_class() == 2 && !ranged(w.kind)),
        weapons.off_hand.is_none_or(|w| !ranged(w.kind)),
        weapons.ranged.is_none_or(|w| ranged(w.kind)),
    ];
    for ((slot, fits), w) in ["main hand", "off hand", "ranged slot"].iter().zip(fits).zip(weapons.slots()) {
        if let (false, Some(w)) = (fits, w) {
            problems.push(format!("zone {zone}: {name} holds a {:?} in its {slot}", w.kind));
        }
    }
    weapons
}

pub(crate) fn resolve_ability(
    zone: &str,
    boss: &str,
    position: usize,
    a: &BossAbilityEntry,
    kinds: &EnemyKinds,
    problems: &mut Vec<String>,
) -> BossAbility {
    let n = &kinds.boss_abilities;
    let (kind, cooldown) = match a.clone() {
        BossAbilityEntry::GroundBurst { telegraph_spell, impact_spell, radius, delay, damage_mult, cooldown } => (
            BossAbilityKind::GroundBurst {
                telegraph_spell,
                impact_spell,
                radius: radius.unwrap_or(n.ground_burst.radius),
                delay: delay.unwrap_or(n.ground_burst.delay),
                damage_mult: damage_mult.unwrap_or(n.ground_burst.damage_mult),
            },
            cooldown.unwrap_or(n.ground_burst.cooldown),
        ),
        BossAbilityEntry::Volley { look_spell, count, damage_mult, cooldown, impact_on_bunny } => (
            BossAbilityKind::Volley {
                look_spell,
                count: count.unwrap_or(n.volley.count),
                damage_mult: damage_mult.unwrap_or(n.volley.damage_mult),
                impact_on_bunny,
            },
            cooldown.unwrap_or(n.volley.cooldown),
        ),
        BossAbilityEntry::Summon { count, cooldown } => {
            let standard = n.summon.adds_by_position.get(position).copied();
            if count.is_none() && standard.is_none() {
                problems.push(format!(
                    "zone {zone}: boss {boss} is boss {} but summon adds_by_position lists only {}; give its Summon a count",
                    position + 1,
                    n.summon.adds_by_position.len()
                ));
            }
            (BossAbilityKind::Summon { count: count.or(standard).unwrap_or(0) }, cooldown.unwrap_or(n.summon.cooldown))
        }
        BossAbilityEntry::Stomp { look_spell, radius, damage_mult, cooldown } => (
            BossAbilityKind::Stomp {
                look_spell,
                radius: radius.unwrap_or(n.stomp.radius),
                damage_mult: damage_mult.unwrap_or(n.stomp.damage_mult),
            },
            cooldown.unwrap_or(n.stomp.cooldown),
        ),
        BossAbilityEntry::Charge { look_spell, damage_mult, cooldown } => (
            BossAbilityKind::Charge { look_spell, damage_mult: damage_mult.unwrap_or(n.charge.damage_mult) },
            cooldown.unwrap_or(n.charge.cooldown),
        ),
        BossAbilityEntry::Marked { shape, at, impact_spell, warning_spell, fill_spell, wind_up, damage_mult, cooldown, leaves, leaves_object, missile, warn, from, impact_on_bunny, chain, brings, sweep } => (
            BossAbilityKind::Marked {
                from,
                impact_on_bunny,
                shape,
                at,
                wind_up: wind_up.unwrap_or(n.marked.wind_up),
                damage_mult: damage_mult.unwrap_or(n.marked.damage_mult),
                impact_spell,
                warning_spell: warning_spell.unwrap_or(n.marked.warning_spell),
                fill_spell: fill_spell.unwrap_or(n.marked.fill_spell),
                leaves,
                leaves_object,
                missile,
                warn,
                chain: chain.unwrap_or(0.0),
                brings,
                sweep,
            },
            cooldown.unwrap_or(n.marked.cooldown),
        ),
        BossAbilityEntry::Rush { look_spell, fade_spell, length, width, wind_up, damage_mult, cooldown, warn } => (
            BossAbilityKind::Rush {
                look_spell,
                length: length.unwrap_or(n.rush.length),
                width: width.unwrap_or(n.rush.width),
                wind_up: wind_up.unwrap_or(n.rush.wind_up),
                damage_mult: damage_mult.unwrap_or(n.rush.damage_mult),
                warning_spell: n.marked.warning_spell,
                fill_spell: n.marked.fill_spell,
                fade_spell,
                warn,
            },
            cooldown.unwrap_or(n.rush.cooldown),
        ),
        BossAbilityEntry::Lay { object, near, count, cooldown, warn } => {
            (BossAbilityKind::Lay { object, count: count.unwrap_or(n.lay.count), near, warn }, cooldown.unwrap_or(n.lay.cooldown))
        }
        BossAbilityEntry::Zone { zone: z, at, cooldown, warn } => (BossAbilityKind::Zone { zone: z, at, warn }, cooldown.unwrap_or(n.zone.cooldown)),
    };
    BossAbility { kind, cooldown }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: &str = "(
        enemies: {
            Fodder: (move_speed: 6.0, attack_interval: 1.5, spawn_cost: 1.0),
            Ranged: (move_speed: 5.0, attack_interval: 3.0, spawn_cost: 2.0, ranged: Some((range: 18.0, cast_time: 1.5))),
            Elite: (move_speed: 5.5, attack_interval: 2.0, spawn_cost: 12.0, elite: true),
        },
        boss_move_speed: 6.0,
        boss_abilities: (
            charge: (cooldown: 8.0, damage_mult: 2.0),
            summon: (cooldown: 15.0, adds_by_position: [6, 7]),
            stomp: (cooldown: 8.0, radius: 10.0, damage_mult: 2.0),
            volley: (cooldown: 6.0, count: 6, damage_mult: 0.5),
            ground_burst: (cooldown: 6.0, radius: 10.0, delay: 2.0, damage_mult: 3.0),
            marked: (cooldown: 8.0, wind_up: 1.5, damage_mult: 3.0, warning_spell: 20924, fill_spell: 20923),
            rush: (cooldown: 10.0, wind_up: 1.5, damage_mult: 3.0, length: 30.0, width: 5.0),
            lay: (cooldown: 15.0, count: 3),
            zone: (cooldown: 12.0),
        ),
    )";

    fn zone(enemies: &str, abilities: &str) -> Result<Vec<Zone>> {
        let text = format!(
            "(id: \"z\", name: \"Z\", subtitle: \"\", description: \"\", map_id: 0, area_id: 0,
              spawn: (x: 0.0, y: 0.0, z: 0.0, facing: 0.0), arena_radius: 80.0, hour: 10,
              weather: (kind: None, intensity: 0.0), difficulty: 1.0, enemies: [{enemies}],
              bosses: [(name: \"B\", display_id: 1, scale: 1.0, creature_type: Humanoid, spawn_minute: 5.0,
                        yell: \"\", abilities: [{abilities}], is_final: true)])"
        );
        let kinds: EnemyKinds = ron::from_str(KINDS).unwrap();
        resolve_zones(vec![ron::from_str(&text).unwrap()], &kinds, &ClientNames::none())
    }

    fn enemy(kind: &str, extra: &str) -> String {
        format!(
            "(name: \"E\", display_id: 1, scale: 1.0, creature_type: Beast, kind: {kind}, \
             appears_at_minute: 0.0, {extra})"
        )
    }

    /// Rares have no lairs. A zone file never listed them, and one
    /// written with lair fields still loads: the loader ignores fields it does not know.
    #[test]
    fn a_zone_file_with_old_lair_fields_still_loads() {
        let text = format!(
            "(id: \"z\", name: \"Z\", subtitle: \"\", description: \"\", map_id: 0, area_id: 0,
              spawn: (x: 0.0, y: 0.0, z: 0.0, facing: 0.0), arena_radius: 80.0, hour: 10,
              weather: (kind: None, intensity: 0.0), difficulty: 1.0, enemies: [{}],
              lairs: [(at: (x: 1.0, y: 2.0), leash: 65.0)], lair_respawn: 180.0,
              bosses: [(name: \"B\", display_id: 1, scale: 1.0, creature_type: Humanoid, spawn_minute: 5.0,
                        yell: \"\", abilities: [], is_final: true)])",
            enemy("Elite", "")
        );
        let kinds: EnemyKinds = ron::from_str(KINDS).unwrap();
        let zones = resolve_zones(vec![ron::from_str(&text).expect("lair fields are ignored")], &kinds, &ClientNames::none()).unwrap();
        assert!(zones[0].enemies[0].elite);
    }

    #[test]
    fn an_entry_takes_its_kinds_numbers_and_its_own_overrides() {
        let z = zone(&[enemy("Fodder", ""), enemy("Ranged", "look_spell: Some(133), range: Some(20.0)"), enemy("Elite", "")].join(","), "")
            .unwrap();
        let e = &z[0].enemies;
        assert_eq!((e[0].move_speed, e[0].attack_interval, e[0].spawn_cost, e[0].elite, e[0].toughness), (6.0, 1.5, 1.0, false, 1.0));
        let r = e[1].ranged.unwrap();
        assert_eq!((r.look_spell, r.range, r.cast_time), (133, 20.0, 1.5));
        assert!(e[2].elite);
        let z = zone(&enemy("Fodder", "move_speed: Some(7.0)"), "").unwrap();
        assert_eq!(z[0].enemies[0].move_speed, 7.0);
    }

    #[test]
    fn an_unknown_kind_fails_with_its_name() {
        let err = zone(&enemy("Fodderr", ""), "").unwrap_err().to_string();
        assert!(err.contains("enemy E names unknown kind Fodderr (enemy_kinds.ron has Elite, Fodder, Ranged)"), "{err}");
    }

    #[test]
    fn a_caster_needs_a_spell_look_and_only_casters_take_one() {
        let err = zone(&enemy("Ranged", ""), "").unwrap_err().to_string();
        assert!(err.contains("enemy E is of ranged kind Ranged but has no look_spell"), "{err}");
        let err = zone(&enemy("Fodder", "look_spell: Some(133)"), "").unwrap_err().to_string();
        assert!(err.contains("kind Fodder does not cast"), "{err}");
    }

    #[test]
    fn boss_abilities_take_the_standard_numbers_unless_overridden() {
        let abilities =
            "Charge(look_spell: 100), Stomp(look_spell: 339, radius: Some(12.0)), Summon(), Volley(look_spell: 686, cooldown: Some(9.0))";
        let z = zone("", abilities).unwrap();
        let a = &z[0].bosses[0].abilities;
        assert_eq!(a[0], BossAbility { kind: BossAbilityKind::Charge { look_spell: 100, damage_mult: 2.0 }, cooldown: 8.0 });
        assert_eq!(a[1], BossAbility { kind: BossAbilityKind::Stomp { look_spell: 339, radius: 12.0, damage_mult: 2.0 }, cooldown: 8.0 });
        assert_eq!(a[2], BossAbility { kind: BossAbilityKind::Summon { count: 6 }, cooldown: 15.0 });
        assert_eq!(a[3], BossAbility { kind: BossAbilityKind::Volley { look_spell: 686, count: 6, damage_mult: 0.5, impact_on_bunny: false }, cooldown: 9.0 });
    }

    #[test]
    fn an_unknown_ability_kind_fails_to_parse_with_its_name() {
        let err = ron::from_str::<BossAbilityEntry>("Meteorfall(look_spell: 133)").unwrap_err().to_string();
        assert!(err.contains("Meteorfall"), "{err}");
    }

    #[test]
    fn a_boss_past_the_adds_list_needs_its_own_summon_count() {
        let kinds: EnemyKinds = ron::from_str(KINDS).unwrap();
        let mut problems = Vec::new();
        let summon = BossAbilityEntry::Summon { count: None, cooldown: None };
        resolve_ability("z", "B", 2, &summon, &kinds, &mut problems);
        assert!(problems[0].contains("boss B is boss 3 but summon adds_by_position lists only 2"), "{problems:?}");
    }

    #[test]
    fn a_creature_holds_its_weapons_each_in_a_slot_of_its_kind() {
        let sword = "(kind: OneHandedSword, display: 7483, inventory_type: 13, sheath: 3, material: 1)";
        let shield = "(kind: Shield, display: 1705, inventory_type: 14, sheath: 4, material: 1)";
        let z = zone(&enemy("Fodder", &format!("weapons: (main_hand: Some({sword}), off_hand: Some({shield}))")), "").unwrap();
        let w = z[0].enemies[0].weapons;
        assert_eq!(w.main_hand.map(|w| (w.display, w.info())), Some((7483, 2 | 7 << 8 | 1 << 16 | 13 << 24)));
        assert!(!w.dual_wields(), "a shield never swings");
        let bow = "(kind: Bow, display: 1, inventory_type: 15, sheath: 0, material: 1)";
        let err = zone(&enemy("Fodder", &format!("weapons: (main_hand: Some({bow}), ranged: Some({shield}))")), "").unwrap_err().to_string();
        assert!(err.contains("E holds a Bow in its main hand") && err.contains("E holds a Shield in its ranged slot"), "{err}");
        // none written: none held
        assert!(zone(&enemy("Fodder", ""), "").unwrap()[0].enemies[0].weapons.is_empty());
    }

    /// The shipped zones' creatures hold vmangos's weapons (docs/game/enemy-weapons.md).
    #[test]
    fn the_shipped_creatures_hold_their_weapons() {
        let content = crate::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let zone = |id: &str| content.zones.iter().find(|z| z.id == id).unwrap();
        let enemy = |z: &str, name: &str| zone(z).enemies.iter().find(|e| e.name == name).unwrap().weapons;
        let boss = |z: &str, name: &str| zone(z).bosses.iter().find(|b| b.name == name).unwrap().weapons;
        assert!(enemy("scarlet", "Scarlet Myrmidon").dual_wields());
        assert_eq!(boss("scarlet", "Herod").main_hand.map(|w| (w.kind, w.display)), Some((crate::WeaponKind::TwoHandedAxe, 15576)));
        assert_eq!(boss("feralas", "King Gordok").main_hand.map(|w| w.display), Some(23969));
        assert_eq!(boss("brm", "Majordomo Executus").main_hand.map(|w| (w.kind, w.sheath)), Some((crate::WeaponKind::Polearm, 2)));
        assert!(enemy("scarlet", "Scarlet Tracking Hound").is_empty() && boss("brm", "Nefarian").is_empty());
    }
}

/// One boss ability as written, resolved with `kinds`' standard numbers (as the content loads
/// it), for tests that compare a kit as written once against the shipped one.
#[doc(hidden)]
pub fn resolve_ability_for_tests(zone: &str, boss: &str, position: usize, a: &BossAbilityEntry, kinds: &EnemyKinds, problems: &mut Vec<String>) -> BossAbility {
    resolve_ability(zone, boss, position, a, kinds, problems)
}
