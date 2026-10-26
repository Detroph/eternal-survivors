//! The balance check: for every zone, how long its enemies last against the hero we expect a player
//! to bring there, minute by minute. Run it after a change to spells, gear, talents or enemy
//! scaling to see what the change does to balance.
//!
//! Every zone (in difficulty order) is played with every class, a few seeds each, for 20 minutes
//! or until the final boss falls. The hero arrives as a player would after one won run in each
//! earlier zone:
//!
//! * **talent points:** the talent experience of one won 20-minute run in each earlier zone, its
//!   bosses all slain, default run options, and the mean kill count this tool measured in that
//!   zone (over every class and seed), turned into points by the rewards rules;
//! * **talents:** every point spent. The first tree first, always on the deepest talent it can
//!   learn, so its last row (the capstone) comes at the 31st point; when the tree is full, the
//!   second tree, then the third;
//! * **gear:** the class's whole set from the zone before it (every armor slot and weapon, with
//!   their bonus pairs); none in the first zone.
//!
//! The hero is played as in the golden run: it never moves after its first step, takes the first
//! offer on every choice screen and cannot die (a cheat death with no cooldown). How often that
//! cheat death fires is counted: it is how often the hero would have fallen.
//!
//! Per zone and run minute it prints the mean hero level and DPS over that minute (all damage dealt
//! ÷ 60 s); the mean max health and damage per hit of the normal enemies alive (sampled each
//! second) and their **kill seconds** (that health ÷ that DPS); the elites and rares that
//! appeared that minute, with their health at spawn, kill seconds (health ÷ the minute's DPS) and
//! damage per hit as a share of the hero's max health; and the cheat deaths. Each boss follows,
//! the same way, at the minute it appeared, with the seconds it lived (from appearing to dying, over
//! the runs it died in) and how many of its appearances died. A minute after the final boss fell has no values (`—`).
//!
//! `cargo run --release -p esurv-sim --example balance_check [-- options]`:
//!
//! * `--seeds N`: runs per zone and class (default 2);
//! * `--check-seeds N`: runs per class and case in `--milestones` and `--gear` (default 200: one
//!   run differs from the next by far more than a milestone adds, so these need many; each gain is
//!   printed with its standard error, `±`);
//! * `--threads N`: runs played at once (default half the machine's threads);
//! * `--targets <csv>`: our target kill seconds of normal enemies, columns `zone,minute,kill_seconds`
//!   (zone by id); each measured value is printed ÷ its target and flagged outside the band;
//! * `--band LOW,HIGH`: the band of measured ÷ target that passes (default 0.8,1.25);
//! * `--milestones`: for every class, tree and milestone row (2, 4 and 6), a hero who spends its
//!   points in that tree first, so it reaches the row, plays the first zone whose arrival points
//!   reach it, with that row's milestone and without it (the talent's other effects stay); the DPS
//!   gain is printed against the row's power budget (docs/game/AUTHORING.md). Damage milestones are
//!   played; defensive and utility ones are only listed. `--talents id,...` plays only those talents;
//! * `--gear`: in every zone, the arrival hero with the previous zone's set against the same hero
//!   with the set before it; the DPS gain is printed against the gear budget;
//! * `--csv <file>`: one row per zone and minute:
//!   `zone,D,minute,level,dps,enemy_health,enemy_damage,kill_seconds,elite_health,boss_health,boss_kill_seconds,cheat_deaths`
//!   (elite and boss values empty in a minute none appeared);
//! * `--starts`: every class's starting options instead, one by one, in each zone of `--zones`
//!   (default elwynn,feralas,aq) with that zone's arrival hero (talent points measured as above,
//!   with `--seeds` runs, and the previous zone's set). Each option is played `--seeds` times in
//!   each of these runs:
//!   * its level-1 spell alone for 300 s (no experience, gold or caches, so no other spell), in
//!     the zone's swarm, once with every passive turned off and once with them on: its damage a
//!     second alone and with its passive, what the passive adds, its swing, the enemies its spell
//!     strikes per cast, the cheat deaths a minute, for rage and energy the income a second (spent
//!     plus banked) and the share of time at the cap, and how often the passive fired;
//!   * the same on one enemy that stands in front of the hero and never strikes (damage on one target);
//!   * a whole run as above (first offers): kill seconds ÷ the `--targets` (and `--second-targets`)
//!     at minutes 5, 10 and 15, and cheat deaths a minute;
//!   * with `--kite`, also the swarm run with the hero standing, for comparison.
//!
//! * `--budgets`: every starting option's level-1 spell alone for 300 s at level 1 in the first
//!   zone of `--zones` (`--seeds` runs; `--kite` kites), no gear: its strike (the level-1 spell
//!   and what it brings, its finisher left out) and its passive (the passive's own damage, plus the
//!   finisher for a strike-and-finisher pair) a second, in the swarm and on one enemy, against our
//!   budget: 40 a second for the strike and 50 for the passive; 90 for the
//!   strike where the passive does no damage (Warrior, Bear). `--csv <file>` writes one row each.
//! * `--starter-runs`: every class's starting options, `--seeds` whole runs each (first offers) in
//!   each zone of `--zones`, with the previous zone's set; `--no-talents` plays them without
//!   talent points (else the arrival points, first tree first), `--no-amulets` turns amulet offers
//!   down, `--kite` kites. Per zone: each option's run DPS, against its class's mean and the zone's
//!   median option, its DPS in minutes 5, 10 and 15, level at minute 15, kills and cheat deaths;
//!   `--csv <file>` writes one row per zone and option.
//!
//!   `--own-tree` spends the arrival points in the option's own spec tree first (then the others
//!   in order), as a player who picked it would, instead of the first tree first.
//!   `--csv <file>` writes one row per zone and option instead of the minutes;
//! * `--loadouts`: every class's three weapon loadouts (roadmap R3),
//!   each played with every starting option of the class, `--seeds` whole runs a pair in each zone
//!   of `--zones`, kiting and declining amulets (as `--starter-runs --kite --no-amulets`; arrival
//!   talents unless `--no-talents`, the previous zone's set). Per zone and class, a row per
//!   loadout: its run DPS over the starters, ÷ the class's loadouts' mean, each starter's DPS,
//!   cheat deaths a run and a minute, runs won; `--csv <file>` writes one row per loadout and
//!   starter;
//! * `--start all` and `--loadout <id|all>` (the zone mode above): each class plays every
//!   starting option (else its first) × every weapon loadout (or that one where the class has it;
//!   else its first), `--seeds` runs a pair on the same seeds, the heroes as the zone mode makes
//!   them; `--no-amulets` declines amulet offers there too. The per-zone output keeps its form,
//!   each zone's lines over all the pairs' runs;
//! * `--adaptive` (with `--milestones`): each talent is played in batches of `--adaptive-batch` seeds
//!   (default 50) a side per starting option and stops as soon as its gain is clear: its error at
//!   most a quarter of the ±25 % band, or its 2-error interval wholly inside or outside the band;
//!   never before `--adaptive-min` seeds (default 200); at most `--check-seeds` seeds; the "runs"
//!   column says how many it took;
//! * `--fair-start` (with `--milestones`): in the arm without a milestone that starts the hero with
//!   a spell (`StartWith`), the hero takes that spell the first time a card offers it, as a player
//!   building that tree would (else the first offer as before); the spell still fills one of its
//!   slots, so the milestone is measured as the head start it is, not as a spell the other arm lacks;
//! * `--whole-talent` (with `--milestones`): the hero without the milestone also lacks the talent's
//!   other effects, so the gain is the whole talent's (a capstone with a damage bonus beside it);
//! * `--also-in <zones>` (with `--milestones`): each milestone is also measured in these later zones,
//!   with their arrival points and the previous zone's set; `--no-amulets` turns amulet offers down;
//! * `--block <share>` (with `--milestones`): the hero blocks that share of melee blows, as a
//!   shield loadout would (0.3 for 30 %);
//! * `--start <option>` (with `--milestones`): the classes that have that starting option play it
//!   (a milestone's worth depends on the starting spell); the others their first;
//! * `--kite` (with `--starts`, `--milestones` or the zone mode): the hero kites instead of standing. It moves at its run speed,
//!   circling the pack (the enemies within 30 yd) counter-clockwise around the pack's centre,
//!   stepping out from the nearest enemy when it is closer than the stand-off and in toward it when
//!   it is farther, so the front of the pack stays at about the stand-off: 3.5 yd for an option
//!   that fights in melee (it spends rage or energy, or its level-1 spell reaches under 10 yd),
//!   8 yd for a caster or shooter. Within 20 yd of the arena's edge it turns back in. It faces the
//!   nearest enemy (it strafes), so cones point at the pack; with no enemy within 30 yd it runs at
//!   the nearest one. It takes damage as any hero
//!   does and still cannot die; the cheat deaths count the times it would have;
//! * `--legendaries <ids|all>`: each legendary amulet (by the spell it remakes, comma-separated, or
//!   `all`) (one per spell; its budget +30 % to the hero's total damage,
//!   +100 % to what a heal heals and absorbs or a defensive buff's uptime). Its hero is the class
//!   with the option whose level-1 spell it remakes, else the class's first option that can draw
//!   the card, knowing the spell (rank 1) from the start; the first cache it opens is sure to offer
//!   the legendary, and it is taken (and kept: later amulets are declined). The baseline is the same
//!   hero and seeds without it (it takes that cache's first offer that is no amulet). Each is
//!   played in the first zone and in Ahn'Qiraj, as `--milestones --also-in aq` does (arrival points,
//!   spent in the option's own tree first, and the previous zone's set). One line each: the spells'
//!   own damage a second with and without it (their hits, ticks, procs, areas and the pets it calls)
//!   and its gain, the hero's whole damage gain, the healing and absorbing gain (heals and shields)
//!   or the buff's uptime gain, how many runs wore it, and the budget, flagged (`!`) outside ±20 %
//!   of it: total damage for damage spells (hybrids too), shields and buffs that add power (+30 %),
//!   healing and absorbing for heals, uptime for defensive buffs (+100 %); then the role, the healing and
//!   absorbing a second with and without it, the damage struck at the hero a second and the
//!   hero's whole damage a second without it.
//!   `--check-seeds` (default 100 here), `--kite`, `--no-amulets` (other amulet offers declined),
//!   `--threads` (default the machine's threads less 2);
//! * `--take-amulets <rarities>` (zone mode; `common,uncommon,rare,epic,legendary` or `all`): on
//!   every choice screen an amulet of those rarities is taken (legendary takes the legendary
//!   amulets too), else the worn one's empowering, else the first offer that is no amulet;
//!   `--buy-caches`: every bought cache is brought under the standing hero as soon as it has the
//!   gold (a player walks to it), so offers come at a real run's timing; `--take-standards`: every
//!   battle standard is brought under the hero as soon as it lies on the field (its ally comes);
//! * `--boss-circle` (with `--kite`, zone mode): while a boss lives the hero loops around it, the
//!   boss at the centre (at its stand-off, at least 6 yd), as players do while dodging its attacks;
//! * `--best-cards` (zone mode): on every screen, unless an amulet is taken, the card of the highest
//!   rarity (a spell upgrade before a new spell before a ring or trinket) instead of the first;
//! * `--overtime <s>` (zone mode): the run goes on that many seconds past minute 20, so a final boss
//!   that came late can die (its "lived s" is not cut off); kills, and so talent points, still stop
//!   at minute 20;
//! * `--content <dir>`: loads the content from that folder instead of the repository's (to measure
//!   another version of the numbers side by side);
//! * `--d <zone>=<D>,...`: gives real zones new difficulties, in memory;
//! * `--add-zone <new_id>=<template_id>:<D>`: adds a synthetic zone in memory, the template's
//!   roster and bosses at difficulty D (may be given more than once).
//!
//! Zones changed or added that way take their place in the difficulty order, and their enemies,
//! gear and talent experience follow their new difficulty; no content file is written.

use std::collections::HashMap;
use std::fs;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use esurv_content::{AfflictEffect, Content, Milestone, PassiveKind, Release, Resource, StackKind, Stat, StartingOption, TalentEffect, Upgrade};
use esurv_sim::game::enemy::RunOptions;
use esurv_sim::game::gear::{piece_modifiers, ARMOR_SLOTS};
use esurv_sim::game::rewards::{gear_item_entry, talent_experience, talent_points, RunTally, MAX_TALENT_POINTS};
use esurv_sim::game::casting::HitSource;
use esurv_sim::game::choices::Offer;
use esurv_sim::game::loot::Pickup;
use esurv_sim::game::enemy_ai::Vec2;
use esurv_sim::game::run::{EnemyKind, Event, Flat, HeroSetup, Phase, Run};
use esurv_sim::game::talents::{learn, Ranks, POINTS_PER_ROW};

/// `--take-amulets <rarities>`: the rarities of amulet the hero takes when offered (a bit per
/// `Rarity`; 0: the first offer, as before); its worn amulet's empowering is taken too.
/// `--take-standards`: every battle standard is brought under the hero (it walks over it).
/// `--buy-caches`: every bought cache is brought under the hero once it has the gold, as a player
/// walks to it.
static TAKE_AMULETS: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static TAKE_STANDARDS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static BUY_CACHES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--boss-circle` (with `--kite`): while a boss lives the hero loops around it instead of the pack.
static BOSS_CIRCLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--best-cards`: the hero takes the card of the highest rarity (an upgrade before a new spell).
static BEST_CARDS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--overtime <s>`: the run goes on this many seconds past minute 20 (a final boss that came late can die), its kills counted to minute 20.
/// `--prefer-upgrades`: an upgrade of a `--prefer` spell is taken over any card but an amulet.
static PREFER_UPGRADES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--damage-focus`: with `--prefer`, a damage amulet of a higher rarity, then a Power upgrade of a
/// preferred spell, then damage jewelry, then empowering a damage amulet, then any upgrade of it.
static DAMAGE_FOCUS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static OVERTIME: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

/// Seconds per tick, as the server's steady loop.
const DT: f32 = 0.05;
/// Minutes a run lasts at most.
const MINUTES: usize = 20;
/// The seed every run's seed is made from.
const SEED: u64 = 20261015;
/// The talent rows that hold a milestone, and the DPS gain each row's milestone is budgeted at.
const MILESTONE_BUDGETS: [(u8, f64); 3] = [(2, 0.05), (4, 0.10), (6, 0.25)];
/// A milestone passes within this share of its budget (a band of ±25 %).
const MILESTONE_TOLERANCE: f64 = 0.25;
/// The DPS gain of a zone's whole set over the previous zone's while talents still grow, and once
/// the hero arrives with every talent point.
const GEAR_BUDGET: f64 = 0.10;
const GEAR_BUDGET_AFTER_TALENTS: f64 = 0.25;

/// A race that can play `class`.
fn race_for(class: &str) -> &'static str {
    match class {
        "shaman" => "orc",
        "druid" => "night_elf",
        "hunter" => "dwarf",
        _ => "human",
    }
}

/// Spends `points` talent points for `class`. The talents in `first` are bought first, in that
/// order, whenever they can be; then each point goes on the deepest talent it can learn (leftmost
/// first), tree by tree in `tree_order`.
fn spend_points_in_order(content: &Content, class: &str, points: u32, tree_order: &[usize], first: &[String]) -> Ranks {
    let trees: Vec<_> = content.trees_of(class).collect();
    let mut order: Vec<&str> = first.iter().map(String::as_str).collect();
    for &t in tree_order {
        let Some(tree) = trees.get(t) else { continue };
        let mut talents: Vec<_> = tree.talents.iter().collect();
        talents.sort_by_key(|t| (std::cmp::Reverse(t.row), t.col));
        order.extend(talents.iter().map(|t| t.id.as_str()));
    }
    let mut ranks = Ranks::new();
    while order.iter().any(|id| learn(content, class, &mut ranks, id, points)) {}
    ranks
}

/// Spends `points` the way the expected hero does: the first tree, then the second, then the third.
fn spend_talent_points(content: &Content, class: &str, points: u32) -> Ranks {
    spend_points_in_order(content, class, points, &[0, 1, 2], &[])
}

/// The class's whole gear set from zone `zone` (its index in difficulty order), as item entries.
fn whole_set(content: &Content, class: &str, zone: usize) -> Vec<u32> {
    let (Some(c), Some(set)) = (content.classes.iter().find(|c| c.id == class), content.gear_set(class)) else { return Vec::new() };
    (0..ARMOR_SLOTS + set.weapons.len() as u32).map(|slot| gear_item_entry(c.wow_class_id, slot, zone as u32)).collect()
}

/// The damage stat the class's whole set from zone `zone` adds, summed over its pieces.
fn set_damage(content: &Content, class: &str, zone: usize) -> f32 {
    let Some(set) = content.gear_set(class) else { return 0.0 };
    (0..ARMOR_SLOTS + set.weapons.len() as u32)
        .filter_map(|slot| piece_modifiers(content, set, slot, zone as u32))
        .flatten()
        .filter(|m| m.stat == Stat::Damage)
        .map(|m| m.value)
        .sum()
}

/// Gives real zones new difficulties in memory, then puts the zones back in difficulty order. A
/// zone's enemies, gear and talent experience all follow its difficulty.
fn set_difficulties(content: &mut Content, difficulties: &[(String, f32)]) {
    for (id, d) in difficulties {
        let zone = content.zones.iter_mut().find(|z| z.id == *id).unwrap_or_else(|| panic!("--d: no zone {id}"));
        zone.difficulty = *d;
    }
    content.zones.sort_by(|a, b| a.difficulty.total_cmp(&b.difficulty));
}

/// Adds a synthetic zone `id` in memory: zone `from`'s roster and bosses at difficulty
/// `difficulty`, slotted into the difficulty order, so its enemies and gear follow the rules every
/// zone's do. Only the zone list is changed: the sim finds a zone by its place in it.
fn add_zone(content: &mut Content, id: &str, from: &str, difficulty: f32) {
    let mut zone = content.zones.iter().find(|z| z.id == from).unwrap_or_else(|| panic!("--add-zone: no zone {from}")).clone();
    assert!(content.zones.iter().all(|z| z.id != id), "--add-zone: a zone {id} already exists");
    zone.id = id.into();
    zone.difficulty = difficulty;
    let at = content.zones.iter().position(|z| z.difficulty > difficulty).unwrap_or(content.zones.len());
    content.zones.insert(at, zone);
}

/// The first seed of a zone's runs, from its id, so adding a zone leaves the others' seeds alone.
fn zone_seed(zone: &str) -> u64 {
    let hash = zone.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    SEED + (hash % 1_000_000) * 1000
}

/// What a player brings into a zone.
#[derive(Clone)]
struct Loadout {
    ranks: Ranks,
    /// The zone whose whole set is worn (its index in difficulty order); none in the first zone.
    set_from: Option<usize>,
    /// A talent whose milestones are left off (its other effects stay).
    without_milestone: Option<String>,
    /// With `without_milestone`, its other effects are left off too (`--whole-talent`).
    without_whole_talent: bool,
    /// The weapon loadout the hero fights with (`loadouts.ron`; `--loadout`, `--loadouts`); the
    /// class's first when none.
    weapons: Option<String>,
    /// The starting option the hero plays (`--start`); the class's first when none.
    start: Option<String>,
}

/// The hero `loadout` makes, unable to die.
fn hero_setup(content: &Content, class: &str, loadout: &Loadout) -> HeroSetup {
    let skipped = loadout.without_milestone.as_deref();
    let kept = loadout.ranks.iter().filter(|(id, _)| Some(id.as_str()) != skipped).map(|(id, r)| (id.as_str(), *r));
    let worn = loadout.set_from.map_or_else(Vec::new, |z| whole_set(content, class, z));
    let mut setup = HeroSetup::with_progress(content, class, race_for(class), kept, &worn);
    if let Some(id) = skipped.filter(|_| !loadout.without_whole_talent) {
        let talents = content.trees_of(class).flat_map(|t| &t.talents);
        if let Some(t) = talents.into_iter().find(|t| t.id == id) {
            let rank = loadout.ranks.get(id).copied().unwrap_or(0).min(u32::from(t.max_rank));
            setup.add_effects(t.effects.iter().filter(|e| !matches!(e, TalentEffect::Milestone(_))).map(|e| (e, rank)));
        }
    }
    setup.start = loadout.start.clone();
    setup.loadout = loadout.weapons.clone();
    // first, so a talent's own cheat death on its cooldown never stands in front of it
    setup.milestones.insert(0, Milestone::CheatDeath { cooldown: 0.0, health: 1.0, immune_seconds: 0.0 });
    setup
}

/// What one run measured in one run minute.
#[derive(Clone, Copy, Default)]
struct MinuteSample {
    level: f64,
    damage: f64,
    hero_max_health: f64,
    /// Mean max health and damage per hit of the normal enemies alive, over the seconds that had any.
    health: f64,
    enemy_damage: f64,
    cheat_deaths: u32,
}

/// The kinds of strong enemy the check follows one by one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StrongKind {
    Elite,
    Rare,
    /// A boss of the zone's schedule, by its index there.
    Boss(usize),
}

/// A strong enemy as it appeared.
#[derive(Clone)]
struct Arrival {
    kind: StrongKind,
    /// Run minute it appeared in (index 0 = minute 1).
    minute: usize,
    health: f64,
    damage: f64,
    /// The enemy's id, the run second it appeared and the run second it died (`None`: alive at the end).
    id: u32,
    spawned: f32,
    died: Option<f32>,
    /// Damage it took, and the hero's whole damage when it appeared and when it died.
    took: f64,
    /// Its biggest single hit taken and what dealt it.
    biggest: (f64, String),
    hero_from: f64,
    hero_to: Option<f64>,
}

/// What one run measured.
struct RunResult {
    /// By run minute (index 0 = minute 1); `None` for a minute the run did not finish.
    minutes: Vec<Option<MinuteSample>>,
    arrivals: Vec<Arrival>,
    kills: u32,
    /// The amulet worn at the end (its rarity and rank), and the caches opened.
    amulet: Option<(esurv_content::Rarity, u32)>,
    caches: u32,
    seconds: f32,
    damage: f64,
}

impl RunResult {
    /// The hero's damage per second over the whole run.
    fn dps(&self) -> f64 {
        self.damage / f64::from(self.seconds.max(1.0))
    }
}

/// Plays one run of `setup` in `zone` and samples it minute by minute, the hero standing; with
/// `kite` (a stand-off in yards) the hero kites (`kite_step`).
fn play_moving(content: &Arc<Content>, zone: &str, setup: HeroSetup, seed: u64, kite: Option<f32>) -> RunResult {
    play_picking(content, zone, setup, seed, kite, false)
}

/// Plays one run as `play_moving` does; with `no_amulets` the hero takes the first offer that is
/// not an amulet (or an amulet's empowering) on every choice screen, the first one when all are.
fn play_picking(content: &Arc<Content>, zone: &str, setup: HeroSetup, seed: u64, kite: Option<f32>, no_amulets: bool) -> RunResult {
    play_picking_preferring(content, zone, setup, seed, kite, no_amulets, &[])
}

/// Stats that raise a hero's damage: `--damage-focus` takes jewelry and amulets with them.
fn damage_stat(stat: Stat) -> bool {
    matches!(stat, Stat::Damage | Stat::CritChance | Stat::CritDamage | Stat::DoubleCast | Stat::SpellCooldownRecovery | Stat::CooldownRecovery)
}

fn damage_jewelry(content: &Content, id: &str) -> bool {
    content.jewelry.iter().any(|j| j.id == id && damage_stat(j.stat))
}

/// An amulet whose first effect raises damage (a stat, or a spell's power), or any legendary.
fn damage_amulet(a: &esurv_content::Amulet) -> bool {
    a.rarity == esurv_content::Rarity::Legendary
        || match a.effects.first() {
            Some(TalentEffect::Stat { stat, .. }) => damage_stat(*stat),
            Some(TalentEffect::AbilityMod { kind, .. }) => *kind == Upgrade::Power,
            _ => false,
        }
}

/// As `play_picking`; a card that offers one of the spells in `prefer` is taken first (`--fair-start`).
fn play_picking_preferring(content: &Arc<Content>, zone: &str, setup: HeroSetup, seed: u64, kite: Option<f32>, no_amulets: bool, prefer: &[String]) -> RunResult {
    let mut run = Run::new(content.clone(), zone, RunOptions::default(), &[setup], Box::new(Flat), seed).expect("a run");
    let (hero, p) = (run.heroes[0].id, run.heroes[0].pos);
    run.hero_moved(hero, (p.0 + 1.0, p.1), 0.0, 0.0);
    let mut minutes = vec![None; MINUTES];
    let mut arrivals: Vec<Arrival> = Vec::new();
    let (mut damage_before, mut next_second, mut cheat_deaths) = (0.0, 1.0, 0u32);
    let mut heading = (1.0, 0.0);
    let (mut health_sum, mut damage_sum, mut sampled) = (0.0, 0.0, 0u32);
    let overtime = f32::from(OVERTIME.load(Ordering::Relaxed));
    let mut kills_at_end: Option<u32> = None;
    while run.now < MINUTES as f32 * 60.0 + overtime && run.phase != Phase::Victory {
        // `--overtime`: the kills (talent experience) stop at the run's end, the run goes on for the bosses
        if kills_at_end.is_none() && run.now >= MINUTES as f32 * 60.0 {
            kills_at_end = Some(run.heroes[0].kills);
        }
        while let Some(screen) = &run.screen {
            let id = run.heroes[screen.hero].id;
            let pick = if no_amulets {
                screen.offers.iter().position(|x| !matches!(x, Offer::Amulet { .. } | Offer::Empower)).unwrap_or(0)
            } else if TAKE_AMULETS.load(Ordering::Relaxed) != 0 {
                // a wanted amulet of a higher rarity than the worn one, else the worn one's
                // empowering, else the first offer that is no amulet
                let wanted = TAKE_AMULETS.load(Ordering::Relaxed);
                let worn = run.heroes[screen.hero].amulet.as_ref().map(|w| w.amulet.rarity);
                let better = |x: &Offer| match x {
                    Offer::Amulet { amulet } => wanted & (1 << amulet.rarity as u8) != 0 && worn.is_none_or(|r| amulet.rarity > r),
                    _ => false,
                };
                (screen.offers.iter().position(better))
                    .or_else(|| screen.offers.iter().position(|x| matches!(x, Offer::Empower)))
                    .or_else(|| screen.offers.iter().position(|x| !matches!(x, Offer::Amulet { .. } | Offer::Empower)))
                    .unwrap_or(0)
            } else {
                0
            };
            // `--best-cards`: unless an amulet (or its empowering) was picked, the card of the
            // highest rarity, a spell upgrade before a new spell before jewelry before gold
            let pick = match (BEST_CARDS.load(Ordering::Relaxed), screen.offers.get(pick)) {
                (true, Some(Offer::Amulet { .. } | Offer::Empower)) | (false, _) | (_, None) => pick,
                (true, Some(_)) => {
                    let score = |x: &Offer| match x {
                        Offer::UpgradeSpell { rarity, .. } => *rarity as i32 * 10 + 3,
                        Offer::NewSpell { rarity, .. } => *rarity as i32 * 10 + 2,
                        Offer::NewJewelry { rarity, .. } | Offer::UpgradeJewelry { rarity, .. } => *rarity as i32 * 10 + 1,
                        _ => -1,
                    };
                    (0..screen.offers.len()).max_by_key(|&k| (score(&screen.offers[k]), std::cmp::Reverse(k))).unwrap_or(pick)
                }
            };
            // `--fair-start`: the spell the milestone would start with, when first offered
            let pick = screen.offers.iter().position(|x| matches!(x, Offer::NewSpell { spell, .. } if prefer.contains(spell))).unwrap_or(pick);
            let pick = if DAMAGE_FOCUS.load(Ordering::Relaxed) && !matches!(screen.offers.get(pick), Some(Offer::NewSpell { spell, .. }) if prefer.contains(spell)) {
                let worn = run.heroes[screen.hero].amulet.as_ref();
                let worn_damage = worn.is_some_and(|w| damage_amulet(&w.amulet));
                let offers = &screen.offers;
                let find = |f: &dyn Fn(&Offer) -> bool| offers.iter().position(f);
                find(&|x| matches!(x, Offer::Amulet { amulet } if damage_amulet(amulet) && (!worn_damage || worn.is_none_or(|w| amulet.rarity > w.amulet.rarity))))
                    .or_else(|| find(&|x| matches!(x, Offer::UpgradeSpell { spell, kinds, .. } if prefer.contains(spell) && kinds.contains(&Upgrade::Power))))
                    .or_else(|| find(&|x| matches!(x, Offer::NewJewelry { id, .. } | Offer::UpgradeJewelry { id, .. } if damage_jewelry(content, id))))
                    .or_else(|| find(&|x| matches!(x, Offer::Empower) && worn_damage))
                    .or_else(|| find(&|x| matches!(x, Offer::UpgradeSpell { spell, .. } if prefer.contains(spell))))
                    .unwrap_or(pick)
            } else {
                pick
            };
            let pick = match screen.offers.get(pick) {
                Some(Offer::NewSpell { spell, .. }) if prefer.contains(spell) => pick,
                Some(Offer::Amulet { .. } | Offer::Empower) => pick,
                _ if PREFER_UPGRADES.load(Ordering::Relaxed) => screen
                    .offers
                    .iter()
                    .position(|x| matches!(x, Offer::UpgradeSpell { spell, .. } if prefer.contains(spell)))
                    .unwrap_or(pick),
                _ => pick,
            };
            run.choose(id, pick);
        }
        // `--take-standards` and `--buy-caches`: the pickup is brought under the standing hero (as
        // if it walked over it), a cache once the hero can pay for it
        let (here, gold, price) = (run.heroes[0].pos, run.heroes[0].gold, run.field.cache_price);
        for p in run.pickups.iter_mut() {
            let bring = match p.kind {
                Pickup::BattleStandard(_) => TAKE_STANDARDS.load(Ordering::Relaxed),
                Pickup::BattlegroundCache => BUY_CACHES.load(Ordering::Relaxed) && gold >= price,
                _ => false,
            };
            if bring {
                p.at = here;
            }
        }
        if let Some(stand_off) = kite {
            kite_step(&mut run, stand_off, &mut heading, DT);
        }
        run.tick(DT);
        let minute = ((run.now / 60.0) as usize).min(MINUTES - 1);
        for e in run.drain_events() {
            let (id, kind) = match e {
                Event::CheatedDeath { .. } => {
                    cheat_deaths += 1;
                    continue;
                }
                Event::EnemyHit { id, amount, source, .. } => {
                    if let Some(a) = arrivals.iter_mut().find(|a| a.id == id && a.died.is_none()) {
                        a.took += f64::from(amount);
                        if f64::from(amount) > a.biggest.0 {
                            a.biggest = (f64::from(amount), format!("{source:?}"));
                        }
                    }
                    continue;
                }
                Event::EnemyDied { id, .. } => {
                    if let Some(a) = arrivals.iter_mut().find(|a| a.id == id && a.died.is_none()) {
                        a.died = Some(run.now);
                        a.hero_to = Some(run.heroes[0].damage_dealt);
                    }
                    continue;
                }
                Event::BossSpawned { id, boss, .. } => (id, StrongKind::Boss(boss)),
                Event::RareSpawned { id, .. } => (id, StrongKind::Rare),
                Event::EnemySpawned { id, .. } => (id, StrongKind::Elite),
                _ => continue,
            };
            let Some(unit) = run.enemies.iter().find(|u| u.id == id) else { continue };
            if kind == StrongKind::Elite && !(unit.elite && matches!(unit.kind, EnemyKind::Roster(_))) {
                continue;
            }
            arrivals.push(Arrival { kind, minute, health: f64::from(unit.max_health), damage: f64::from(unit.damage), id, spawned: run.now, died: None, took: 0.0, biggest: (0.0, String::new()), hero_from: run.heroes[0].damage_dealt, hero_to: None });
        }
        if run.now + 1e-3 >= next_second {
            next_second += 1.0;
            let normal: Vec<(f32, f32)> =
                run.enemies.iter().filter(|e| matches!(e.kind, EnemyKind::Roster(_)) && !e.elite).map(|e| (e.max_health, e.damage)).collect();
            if !normal.is_empty() {
                let n = normal.len() as f64;
                health_sum += normal.iter().map(|&(h, _)| f64::from(h)).sum::<f64>() / n;
                damage_sum += normal.iter().map(|&(_, d)| f64::from(d)).sum::<f64>() / n;
                sampled += 1;
            }
            // a minute ends on its 60th second
            if (next_second as usize - 1) % 60 == 0 && (next_second as usize - 1) / 60 <= MINUTES {
                let m = (next_second as usize - 1) / 60 - 1;
                let h = &run.heroes[0];
                let per = |sum: f64| if sampled > 0 { sum / f64::from(sampled) } else { 0.0 };
                minutes[m] = Some(MinuteSample {
                    level: f64::from(h.progress.level),
                    damage: h.damage_dealt - damage_before,
                    hero_max_health: f64::from(h.stats.max_health),
                    health: per(health_sum),
                    enemy_damage: per(damage_sum),
                    cheat_deaths,
                });
                (damage_before, health_sum, damage_sum, sampled, cheat_deaths) = (h.damage_dealt, 0.0, 0.0, 0, 0);
            }
        }
    }
    let h = &run.heroes[0];
    RunResult { minutes, arrivals, kills: kills_at_end.unwrap_or(h.kills), seconds: run.now, damage: h.damage_dealt, amulet: h.amulet.as_ref().map(|w| (w.amulet.rarity, w.rank)), caches: h.caches_opened }
}

/// The talent experience of one won 20-minute run in zone `zone` (its index in difficulty order):
/// every boss slain, default run options, `kills` kills.
fn won_run_experience(content: &Content, zone: usize, kills: f64) -> u32 {
    let z = &content.zones[zone];
    let tally = RunTally { seconds: MINUTES as f32 * 60.0, kills: kills.round() as u32, bosses: z.bosses.len() as u32, won: true };
    let (base, zone_bonus, option_bonus) = talent_experience(&tally, &content.balance.zone_scaling, z.difficulty, &RunOptions::default(), &content.run_options);
    base + zone_bonus + option_bonus
}

/// The talent points each zone's expected hero arrives with (by difficulty order), measured as the
/// main check does: every zone played with every class's default hero, `seeds` runs each, and the
/// mean kills of a zone's runs turned into the next zone's experience.
fn arrival_points(content: &Arc<Content>, seeds: u64, threads: usize) -> Vec<u32> {
    let (mut experience, mut points) = (0u32, Vec::new());
    for zi in 0..content.zones.len() {
        let p = talent_points(experience).0;
        points.push(p);
        let set_from = zi.checked_sub(1);
        let jobs = zone_jobs(content, zi, seeds, |class| Some(Loadout { ranks: spend_talent_points(content, class, p), set_from, without_milestone: None, without_whole_talent: false, weapons: None, start: None }));
        let results = play_all(content, &jobs, threads);
        let kills = mean(results.iter().flatten().map(|r| f64::from(r.kills))).unwrap_or(0.0);
        experience += won_run_experience(content, zi, kills);
    }
    points
}

/// One run to play.
struct Job {
    zone: usize,
    class: usize,
    seed: u64,
    loadout: Loadout,
    /// The hero kites keeping this stand-off (`--kite`), else stands.
    kite: Option<f32>,
    /// The hero blocks this share of melee blows (`--block`), as a shield loadout would.
    block: f32,
    /// The hero turns amulet offers down (`--no-amulets`).
    no_amulets: bool,
    /// Spells the hero learns the first time a card offers them (`--fair-start`: the spell a
    /// milestone would start it with, in the arm without it).
    prefer: Vec<String>,
}

/// Plays `jobs` on `threads` threads; results in job order. A run that panics is reported and left
/// out (`None`), so one failing run does not stop the check.
fn play_all(content: &Arc<Content>, jobs: &[Job], threads: usize) -> Vec<Option<RunResult>> {
    let next = AtomicUsize::new(0);
    let results: Vec<Mutex<Option<RunResult>>> = jobs.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(job) = jobs.get(i) else { break };
                let (zone, class) = (&content.zones[job.zone].id, &content.classes[job.class].id);
                let mut setup = hero_setup(content, class, &job.loadout);
                let shield = TalentEffect::Stat { stat: Stat::BlockChance, per_rank: job.block };
                if job.block > 0.0 {
                    setup.add_effects([(&shield, 1)]);
                }
                match std::panic::catch_unwind(AssertUnwindSafe(|| play_picking_preferring(content, zone, setup, job.seed, job.kite, job.no_amulets, &job.prefer))) {
                    Ok(r) => *results[i].lock().unwrap() = Some(r),
                    Err(_) => eprintln!("balance check: the run of {zone}/{class} seed {} panicked; it is left out", job.seed),
                }
            });
        }
    });
    results.into_iter().map(|r| r.into_inner().unwrap()).collect()
}

/// The mean of `values`, `None` when there are none.
fn mean(values: impl IntoIterator<Item = f64>) -> Option<f64> {
    let (sum, n) = values.into_iter().fold((0.0, 0usize), |(s, n), v| (s + v, n + 1));
    (n > 0).then(|| sum / n as f64)
}

/// The mean of `values` and its standard error, `None` when there are none.
fn mean_and_error(values: impl IntoIterator<Item = f64>) -> Option<(f64, f64)> {
    let values: Vec<f64> = values.into_iter().collect();
    let m = mean(values.iter().copied())?;
    let n = values.len() as f64;
    let variance = if n > 1.0 { values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - 1.0) } else { 0.0 };
    Some((m, (variance / n).sqrt()))
}

/// The gain of `with` over `without` (each a mean and its standard error) and the gain's standard error.
fn gain_and_error(with: (f64, f64), without: (f64, f64)) -> (f64, f64) {
    let ratio = with.0 / without.0.max(1e-9);
    let relative = ((with.1 / with.0.max(1e-9)).powi(2) + (without.1 / without.0.max(1e-9)).powi(2)).sqrt();
    (ratio - 1.0, ratio * relative)
}

/// Case `case`'s gain (with ÷ without − 1) over the runs played so far and its standard error,
/// from the seed-by-seed differences of the two arms (they play the same seeds; the unpaired error
/// when fewer than two pairs finished).
fn case_gain(jobs: &[(usize, bool, Job)], results: &[Option<RunResult>], case: usize) -> Option<(f64, f64)> {
    let by_seed = |without: bool| -> HashMap<u64, f64> {
        jobs.iter()
            .zip(results)
            .filter(|((k, w, _), _)| *k == case && *w == without)
            .filter_map(|((_, _, j), r)| r.as_ref().map(|r| (j.seed, r.dps())))
            .filter(|(_, d)| d.is_finite())
            .collect()
    };
    let (with_runs, without_runs) = (by_seed(false), by_seed(true));
    let with = mean_and_error(with_runs.values().copied())?;
    let without = mean_and_error(without_runs.values().copied())?;
    let (gain, unpaired) = gain_and_error(with, without);
    let diffs: Vec<f64> = with_runs.iter().filter_map(|(s, w)| without_runs.get(s).map(|wo| w - wo)).collect();
    let error = match mean_and_error(diffs.iter().copied()) {
        Some((_, e)) if diffs.len() > 1 => e / without.0.max(1e-9),
        _ => unpaired,
    };
    Some((gain, error))
}

/// `--adaptive`: a talent's measure can stop once its gain is clear against its row's budget:
/// its standard error is at most a quarter of the band (±`MILESTONE_TOLERANCE` of the budget), or
/// its 2-error interval lies wholly inside the band or wholly outside it.
fn milestone_settled(gain: f64, error: f64, budget: f64) -> bool {
    let (low, high) = (budget * (1.0 - MILESTONE_TOLERANCE), budget * (1.0 + MILESTONE_TOLERANCE));
    let precise = error <= budget * MILESTONE_TOLERANCE / 4.0;
    let (lo, hi) = (gain - 2.0 * error, gain + 2.0 * error);
    precise || (lo >= low && hi <= high) || hi < low || lo > high
}

#[cfg(test)]
mod adaptive_tests {
    use super::milestone_settled;

    /// A gain stops once its error is small, or its interval sits wholly inside or outside the band.
    #[test]
    fn a_milestone_measure_stops_once_its_gain_is_clear() {
        // budget +10 %, band +7.5 to +12.5 %
        assert!(milestone_settled(0.10, 0.006, 0.10), "error within a quarter of the band");
        assert!(milestone_settled(0.10, 0.01, 0.10), "interval 8-12 % inside");
        assert!(milestone_settled(0.20, 0.03, 0.10), "interval 14-26 % above");
        assert!(milestone_settled(0.01, 0.02, 0.10), "interval -3 to 5 % below");
        assert!(!milestone_settled(0.10, 0.02, 0.10), "interval 6-14 % straddles the band");
        assert!(!milestone_settled(0.13, 0.02, 0.10), "interval 9-17 % straddles the top");
    }
}

/// A zone's measured values in one minute, over every class and seed.
#[derive(Clone, Copy)]
struct MinuteMeans {
    level: f64,
    dps: f64,
    health: f64,
    enemy_damage: f64,
    kill_seconds: f64,
    cheat_deaths: f64,
    /// Runs that finished this minute, of all.
    runs: usize,
}

fn minute_means(results: &[&RunResult], minute: usize) -> Option<MinuteMeans> {
    let samples: Vec<MinuteSample> = results.iter().filter_map(|r| r.minutes[minute]).collect();
    let level = mean(samples.iter().map(|s| s.level))?;
    let dps = mean(samples.iter().map(|s| s.damage / 60.0))?;
    let health = mean(samples.iter().map(|s| s.health))?;
    Some(MinuteMeans {
        level,
        dps,
        health,
        enemy_damage: mean(samples.iter().map(|s| s.enemy_damage))?,
        kill_seconds: if dps > 0.0 { health / dps } else { f64::INFINITY },
        cheat_deaths: mean(samples.iter().map(|s| f64::from(s.cheat_deaths)))?,
        runs: samples.len(),
    })
}

/// Strong enemies of one kind as they appeared, over every run: mean health and damage per hit at
/// spawn, kill seconds (health ÷ the hero's DPS in the minute it appeared, in its own run) and
/// damage per hit as a share of the hero's max health then.
#[derive(Clone, Copy)]
struct StrongMeans {
    count: usize,
    minute: f64,
    health: f64,
    damage: f64,
    kill_seconds: f64,
    hit_share: f64,
    /// Mean seconds from appearing to dying, over those that died (`None`: none did), and how many died.
    lived: Option<f64>,
    killed: usize,
    /// Of the hero's damage while it lived, the share that struck it (those that died).
    share: Option<f64>,
}

fn strong_means(results: &[&RunResult], wanted: impl Fn(&Arrival) -> bool) -> Option<StrongMeans> {
    let mut rows = Vec::new();
    for r in results {
        for a in r.arrivals.iter().filter(|a| wanted(a)) {
            let Some(s) = r.minutes[a.minute] else { continue };
            let dps = s.damage / 60.0;
            rows.push((a, if dps > 0.0 { a.health / dps } else { f64::INFINITY }, a.damage / s.hero_max_health.max(1.0)));
        }
    }
    Some(StrongMeans {
        count: rows.len(),
        minute: mean(rows.iter().map(|(a, _, _)| a.minute as f64 + 1.0))?,
        health: mean(rows.iter().map(|(a, _, _)| a.health))?,
        damage: mean(rows.iter().map(|(a, _, _)| a.damage))?,
        kill_seconds: mean(rows.iter().map(|(_, k, _)| *k))?,
        hit_share: mean(rows.iter().map(|(_, _, s)| *s))?,
        lived: mean(rows.iter().filter_map(|(a, _, _)| a.died.map(|d| f64::from(d - a.spawned)))),
        killed: rows.iter().filter(|(a, _, _)| a.died.is_some()).count(),
        share: mean(rows.iter().filter_map(|(a, _, _)| a.hero_to.map(|t| a.took / (t - a.hero_from).max(1.0)))),
    })
}

/// Our target kill seconds by (zone id, minute), read from a `zone,minute,kill_seconds` file.
fn read_targets(path: &str) -> HashMap<(String, usize), f64> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("the target file {path} reads: {e}"));
    let mut out = HashMap::new();
    for (n, line) in text.lines().enumerate() {
        let cells: Vec<&str> = line.split(',').map(str::trim).collect();
        if cells.len() < 3 || cells[0].is_empty() || cells[0].starts_with('#') {
            continue;
        }
        let (Ok(minute), Ok(seconds)) = (cells[1].parse::<usize>(), cells[2].parse::<f64>()) else {
            // the header line, or a line that is not a target
            if n > 0 {
                eprintln!("balance check: line {} of {path} is no target: {line}", n + 1);
            }
            continue;
        };
        out.insert((cells[0].to_string(), minute), seconds);
    }
    out
}

/// What a milestone does, for the report: damage milestones are held to their budget.
fn milestone_role(m: &Milestone) -> &'static str {
    match m {
        Milestone::CheatDeath { .. } | Milestone::HealOnTrigger { .. } | Milestone::OverhealShield { .. } | Milestone::TotemHeal { .. } => {
            "defensive"
        }
        Milestone::Afflict { effect: AfflictEffect::Vulnerable { .. }, .. } => "damage",
        Milestone::Afflict { effect: AfflictEffect::HealFromDamage { .. } | AfflictEffect::HealMaxHealth { .. }, .. } => "defensive",
        Milestone::Afflict { .. } => "utility",
        Milestone::TimedBuff { stat, .. } => match stat {
            Stat::Damage | Stat::CritChance | Stat::CritDamage | Stat::CooldownRecovery | Stat::SpellCooldownRecovery | Stat::DoubleCast | Stat::Area | Stat::ExtraTargets => {
                "damage"
            }
            Stat::MoveSpeed => "utility",
            _ => "defensive",
        },
        _ => "damage",
    }
}

/// A short name of a milestone's kind and main number, for the report.
fn milestone_label(m: &Milestone) -> String {
    let mut m = m.clone();
    let main = m.rank_value_mut().map(|v| format!(" {v}")).unwrap_or_default();
    let debug = format!("{m:?}");
    let kind = debug.split([' ', '{', '(']).next().unwrap_or("?").to_string();
    format!("{kind}{main}")
}

/// Options from the command line.
struct Options {
    seeds: u64,
    check_seeds: u64,
    threads: usize,
    targets: Option<String>,
    band: (f64, f64),
    milestones: bool,
    /// Only these talents in `--milestones` (all when empty).
    talents: Vec<String>,
    gear: bool,
    /// Measure every starting option instead (`--starts`).
    starts: bool,
    /// `--starts` with a kiting hero (`--kite`), not a standing one.
    kite: bool,
    /// `--milestones` weighs the whole talent, not only its milestones (`--whole-talent`).
    whole_talent: bool,
    /// `--milestones` with a hero that blocks this share of melee blows (`--block`).
    block: f32,
    /// `--milestones` also in these later zones (`--also-in`).
    also_in: Vec<String>,
    /// `--milestones` with this starting option for the classes that have it (`--start`).
    start: Option<String>,
    /// The zones `--starts` measures in (`--zones`).
    start_zones: Vec<String>,
    /// A second target file `--starts` also holds the kill seconds against (`--second-targets`).
    second_targets: Option<String>,
    /// `--starts` spends the arrival points in the option's own tree first (`--own-tree`).
    own_tree: bool,
    /// Whole runs of every starting option instead (`--starter-runs`).
    starter_runs: bool,
    /// Every class's weapon loadouts with its starting options, kiting, amulets declined (`--loadouts`).
    loadouts: bool,
    /// The zone mode plays every starting option (`--start all`).
    start_all: bool,
    /// The zone mode's weapon loadout: one by id where the class has it (`--loadout <id>`), or
    /// every one (`--loadout all`).
    loadout: Option<String>,
    loadout_all: bool,
    /// Every starting option's level-1 strike and passive against their budgets (`--budgets`).
    budgets: bool,
    /// `--starter-runs` with no talent points (`--no-talents`).
    no_talents: bool,
    /// `--starter-runs` turning amulet offers down (`--no-amulets`).
    no_amulets: bool,
    /// `--adaptive`: each talent's runs come in batches of `adaptive_batch` seeds a side per
    /// starting option, and stop once its gain is clear (`milestone_settled`), at most `--check-seeds`.
    adaptive: bool,
    adaptive_batch: u64,
    adaptive_min: u64,
    /// `--fair-start`: the arm without a start-spell milestone takes that spell when first offered.
    fair_start: bool,
    /// `--prefer a,b`: zone mode, every hero takes these spells when first offered; `--classes a,b`: only these classes.
    prefer: Vec<String>,
    classes: Vec<String>,
    /// The legendary amulets `--legendaries` measures (by their spell, or `all`).
    legendaries: Vec<String>,
    /// `--check-seeds` and `--threads` as given (`--legendaries` has its own defaults).
    check_seeds_given: Option<u64>,
    threads_given: Option<usize>,
    csv: Option<String>,
    /// Load the content from this folder instead of the repository's (`--content <dir>`).
    content_dir: Option<String>,
    /// Synthetic zones: (new id, template zone id, difficulty).
    added_zones: Vec<(String, String, f32)>,
    /// Difficulties that replace real zones' own: (zone id, difficulty).
    difficulties: Vec<(String, f32)>,
}

impl Default for Options {
    /// The options with none given.
    fn default() -> Self {
        let half = std::thread::available_parallelism().map_or(4, |n| n.get() / 2).max(1);
        Options {
            seeds: 2,
            check_seeds: 200,
            threads: half,
            targets: None,
            band: (0.8, 1.25),
            milestones: false,
            talents: Vec::new(),
            gear: false,
            starts: false,
            kite: false,
            whole_talent: false,
            block: 0.0,
            also_in: Vec::new(),
            start: None,
            start_zones: STARTS_ZONES.iter().map(|z| z.to_string()).collect(),
            second_targets: None,
            own_tree: false,
            starter_runs: false,
            loadouts: false,
            start_all: false,
            loadout: None,
            loadout_all: false,
            budgets: false,
            no_talents: false,
            no_amulets: false,
            fair_start: false,
            prefer: Vec::new(),
            classes: Vec::new(),
            adaptive: false,
            adaptive_batch: 50,
            adaptive_min: 200,
            legendaries: Vec::new(),
            check_seeds_given: None,
            threads_given: None,
            csv: None,
            content_dir: None,
            added_zones: Vec::new(),
            difficulties: Vec::new(),
        }
    }
}

fn options() -> Options {
    let mut o = Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{arg} takes a value"));
        match arg.as_str() {
            "--seeds" => o.seeds = value().parse().expect("--seeds takes a number"),
            "--check-seeds" => {
                o.check_seeds = value().parse().expect("--check-seeds takes a number");
                o.check_seeds_given = Some(o.check_seeds);
            }
            "--threads" => {
                o.threads = value().parse().expect("--threads takes a number");
                o.threads_given = Some(o.threads);
            }
            "--legendaries" => o.legendaries = value().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            "--targets" => o.targets = Some(value()),
            "--csv" => o.csv = Some(value()),
            "--content" => o.content_dir = Some(value()),
            "--milestones" => o.milestones = true,
            "--talents" => o.talents = value().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            "--gear" => o.gear = true,
            "--starts" => o.starts = true,
            "--kite" => o.kite = true,
            "--whole-talent" => o.whole_talent = true,
            "--block" => o.block = value().parse().expect("--block takes a share, e.g. 0.3"),
            "--also-in" => o.also_in = value().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            "--start" => match value() {
                all if all == "all" => o.start_all = true,
                id => o.start = Some(id),
            },
            "--loadout" => match value() {
                all if all == "all" => o.loadout_all = true,
                id => o.loadout = Some(id),
            },
            "--loadouts" => o.loadouts = true,
            "--zones" => o.start_zones = value().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            "--second-targets" => o.second_targets = Some(value()),
            "--own-tree" => o.own_tree = true,
            "--starter-runs" => o.starter_runs = true,
            "--budgets" => o.budgets = true,
            "--no-talents" => o.no_talents = true,
            "--no-amulets" => o.no_amulets = true,
            "--fair-start" => o.fair_start = true,
            "--prefer" => o.prefer = value().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            "--prefer-upgrades" => PREFER_UPGRADES.store(true, Ordering::Relaxed),
            "--damage-focus" => DAMAGE_FOCUS.store(true, Ordering::Relaxed),
            "--classes" => o.classes = value().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            "--adaptive" => o.adaptive = true,
            "--adaptive-batch" => o.adaptive_batch = value().parse().expect("--adaptive-batch <seeds>"),
            "--adaptive-min" => o.adaptive_min = value().parse().expect("--adaptive-min <seeds>"),
            "--take-amulets" => {
                let mut bits = 0u8;
                for r in value().split(',').map(str::trim) {
                    bits |= match r {
                        "common" => 1,
                        "uncommon" => 2,
                        "rare" => 4,
                        "epic" => 8,
                        "legendary" => 16,
                        "all" => 31,
                        other => panic!("--take-amulets: unknown rarity {other}"),
                    };
                }
                TAKE_AMULETS.store(bits, Ordering::Relaxed);
            }
            "--buy-caches" => BUY_CACHES.store(true, Ordering::Relaxed),
            "--boss-circle" => BOSS_CIRCLE.store(true, Ordering::Relaxed),
            "--best-cards" => BEST_CARDS.store(true, Ordering::Relaxed),
            "--overtime" => OVERTIME.store(value().parse().expect("--overtime <seconds>"), Ordering::Relaxed),
            "--take-standards" => TAKE_STANDARDS.store(true, Ordering::Relaxed),
            "--d" => {
                for pair in value().split(',').filter(|p| !p.trim().is_empty()) {
                    let parsed = pair.split_once('=').map(|(id, d)| (id.trim(), d.trim().parse::<f32>()));
                    let Some((id, Ok(d))) = parsed else { panic!("--d takes ZONE_ID=DIFFICULTY,..., not {pair}") };
                    o.difficulties.push((id.into(), d));
                }
            }
            "--add-zone" => {
                let v = value();
                let parsed = v.split_once('=').and_then(|(id, rest)| rest.split_once(':').map(|(from, d)| (id, from, d.parse::<f32>())));
                let Some((id, from, Ok(d))) = parsed else { panic!("--add-zone takes NEW_ID=TEMPLATE_ID:DIFFICULTY, not {v}") };
                o.added_zones.push((id.trim().into(), from.trim().into(), d));
            }
            "--band" => {
                let v = value();
                let (low, high) = v.split_once(',').expect("--band takes LOW,HIGH");
                o.band = (low.trim().parse().expect("a low bound"), high.trim().parse().expect("a high bound"));
            }
            other => panic!("unknown option {other}"),
        }
    }
    o
}

/// The jobs of every class and seed in zone `zone`, each class with the loadout `loadout` gives it.
fn zone_jobs(content: &Content, zone: usize, seeds: u64, mut loadout: impl FnMut(&str) -> Option<Loadout>) -> Vec<Job> {
    let mut jobs = Vec::new();
    for (ci, class) in content.classes.iter().enumerate() {
        let Some(l) = loadout(&class.id) else { continue };
        for s in 0..seeds {
            jobs.push(Job { zone, class: ci, seed: zone_seed(&content.zones[zone].id) + ((ci as u64) << 40) + s, loadout: l.clone(), kite: None, block: 0.0, no_amulets: false, prefer: Vec::new() });
        }
    }
    jobs
}

/// The zone mode's jobs in zone `zone` for heroes arriving with `points` talent points (first tree
/// first) and the previous zone's set: every class, `--seeds` runs of each pair of its starting
/// option (`--start all`: every one; else its first) and its weapon loadout (`--loadout all`: every
/// one; `--loadout <id>`: that one where the class has it; else its first), each pair on the same
/// seeds; `--no-amulets` declines amulet offers.
fn zone_mode_jobs(content: &Content, zone: usize, o: &Options, points: u32) -> Vec<Job> {
    let set_from = zone.checked_sub(1);
    let mut jobs = Vec::new();
    for (ci, class) in content.classes.iter().enumerate() {
        if !o.classes.is_empty() && !o.classes.contains(&class.id) {
            continue;
        }
        let starts: Vec<Option<String>> =
            if o.start_all {
                class.starting_options.iter().map(|s| Some(s.id.clone())).collect()
            } else {
                // `--start <id>`: that option where the class has it
                vec![o.start.clone().filter(|id| class.starting_options.iter().any(|s| s.id == *id))]
            };
        let own = content.loadouts_of(&class.id);
        let weapons: Vec<Option<String>> = if o.loadout_all && !own.is_empty() {
            own.iter().map(|l| Some(l.id.clone())).collect()
        } else {
            vec![o.loadout.clone().filter(|id| own.iter().any(|l| l.id == *id))]
        };
        let ranks = spend_talent_points(content, &class.id, points);
        for start in &starts {
            for w in &weapons {
                for s in 0..o.seeds {
                    let loadout = Loadout {
                        ranks: ranks.clone(),
                        set_from,
                        without_milestone: None,
                        without_whole_talent: false,
                        weapons: w.clone(),
                        start: start.clone(),
                    };
                    let seed = zone_seed(&content.zones[zone].id) + ((ci as u64) << 40) + s;
                    // `--kite`: the hero kites as its option would
                    let option = loadout.start.as_deref().and_then(|id| content.starting_option(id)).unwrap_or(&class.starting_options[0]);
                    let kite = o.kite.then(|| kite_stand_off(content, option));
                    jobs.push(Job { zone, class: ci, seed, loadout, kite, block: 0.0, no_amulets: o.no_amulets, prefer: o.prefer.clone() });
                }
            }
        }
    }
    jobs
}

fn main() {
    let o = options();
    let started = Instant::now();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
    let dir = o.content_dir.as_deref().unwrap_or(dir);
    let mut content = Content::load(dir).expect("content loads");
    set_difficulties(&mut content, &o.difficulties);
    for (id, from, difficulty) in &o.added_zones {
        add_zone(&mut content, id, from, *difficulty);
    }
    let content = Arc::new(content);
    if !o.legendaries.is_empty() {
        legendary_check(&content, &o);
        println!("\n(took {:.0} s)", started.elapsed().as_secs_f64());
        return;
    }
    if o.budgets {
        budgets(&content, &o);
        println!("\n(took {:.0} s)", started.elapsed().as_secs_f64());
        return;
    }
    if o.starter_runs {
        starter_runs(&content, &o);
        println!("\n(took {:.0} s)", started.elapsed().as_secs_f64());
        return;
    }
    if o.loadouts {
        loadout_runs(&content, &o);
        println!("\n(took {:.0} s)", started.elapsed().as_secs_f64());
        return;
    }
    if o.starts {
        starts_check(&content, &o);
        println!("
(took {:.0} s)", started.elapsed().as_secs_f64());
        return;
    }
    let synthetic: HashMap<&str, &str> = o.added_zones.iter().map(|(id, from, _)| (id.as_str(), from.as_str())).collect();
    let targets = o.targets.as_deref().map(read_targets);
    let classes: Vec<String> = content.classes.iter().map(|c| c.id.clone()).collect();
    let mut csv =
        String::from("zone,D,minute,level,dps,enemy_health,enemy_damage,kill_seconds,elite_health,boss_health,boss_kill_seconds,cheat_deaths\n");
    // talent experience earned by one won run in each zone so far
    let mut experience = 0u32;
    // the talent points each zone's hero arrives with
    let mut arrival_points = Vec::new();
    // per zone, measured ÷ target by minute
    let mut summary: Vec<(String, Vec<Option<f64>>)> = Vec::new();
    println!("Balance check: {} seeds a zone and class, {} threads.\n", o.seeds, o.threads);
    if o.start_all || o.loadout_all || o.loadout.is_some() {
        println!(
            "Each class plays {} × {}, {} seeds a pair{}.\n",
            if o.start_all { "every starting option" } else { "its first starting option" },
            match (&o.loadout, o.loadout_all) {
                (_, true) => "every weapon loadout".to_string(),
                (Some(id), false) => format!("weapon loadout {id} (where it has it, else its first)"),
                (None, false) => "its first weapon loadout".to_string(),
            },
            o.seeds,
            if o.no_amulets { ", amulets declined" } else { "" }
        );
    }
    for (zi, zone) in content.zones.iter().enumerate() {
        let points = talent_points(experience).0;
        arrival_points.push(points);
        let set_from = zi.checked_sub(1);
        let jobs = zone_mode_jobs(&content, zi, &o, points);
        let results = play_all(&content, &jobs, o.threads);
        let full: Vec<&RunResult> = results.iter().filter_map(Option::as_ref).collect();
        let kills = mean(full.iter().map(|r| f64::from(r.kills))).unwrap_or(0.0);
        let won = full.iter().filter(|r| r.seconds < MINUTES as f32 * 60.0).count();
        let made = synthetic.get(zone.id.as_str()).map_or(String::new(), |from| format!(", synthetic from {from}"));
        let gear: Vec<f32> = classes.iter().map(|c| set_from.map_or(0.0, |z| set_damage(&content, c, z))).collect();
        let gear_mean = gear.iter().sum::<f32>() / gear.len().max(1) as f32;
        let worn = set_from.map_or("no".to_string(), |z| format!("{}'s", content.zones[z].id));
        let cheats: f64 = full.iter().map(|r| r.minutes.iter().flatten().map(|s| f64::from(s.cheat_deaths)).sum::<f64>()).sum::<f64>();
        println!(
            "## {} (D {}{}): {} talent points, {} gear (damage +{:.0} % mean); mean {:.0} kills, {} of {} runs won before minute {}, {:.1} cheat deaths a run",
            zone.id,
            zone.difficulty,
            made,
            points,
            worn,
            gear_mean * 100.0,
            kills,
            won,
            full.len(),
            MINUTES,
            cheats / full.len().max(1) as f64
        );
        {
            let n = full.len().max(1) as f64;
            let caches = full.iter().map(|r| f64::from(r.caches)).sum::<f64>() / n;
            let mut by: Vec<String> = Vec::new();
            for r in [esurv_content::Rarity::Common, esurv_content::Rarity::Uncommon, esurv_content::Rarity::Rare, esurv_content::Rarity::Epic, esurv_content::Rarity::Legendary] {
                let worn: Vec<u32> = full.iter().filter_map(|x| x.amulet).filter(|a| a.0 == r).map(|a| a.1).collect();
                if !worn.is_empty() {
                    by.push(format!("{r:?} {} (rank {:.1})", worn.len(), worn.iter().sum::<u32>() as f64 / worn.len() as f64));
                }
            }
            println!("caches opened {caches:.1} a run; amulet worn at the end: {}", if by.is_empty() { "none".to_string() } else { by.join(", ") });
        }
        print!("{:>3} {:>5} {:>8} {:>8} {:>6} {:>6}", "min", "level", "dps", "health", "dmg", "kill s");
        if targets.is_some() {
            print!(" {:>6} {:>7}", "target", "ratio");
        }
        println!(" {:>8} {:>6} {:>5} {:>8} {:>6} {:>5} {:>5}", "elite hp", "kill s", "hit%", "guard hp", "kill s", "hit%", "cheat");
        let mut ratios = vec![None; MINUTES];
        for m in 0..MINUTES {
            let Some(mm) = minute_means(&full, m) else {
                println!("{:>3} {:>5}", m + 1, "—");
                continue;
            };
            print!("{:>3} {:>5.1} {:>8.0} {:>8.0} {:>6.0} {:>6.2}", m + 1, mm.level, mm.dps, mm.health, mm.enemy_damage, mm.kill_seconds);
            if let Some(t) = &targets {
                match t.get(&(zone.id.clone(), m + 1)) {
                    Some(&target) => {
                        let ratio = mm.kill_seconds / target;
                        let flag = if ratio < o.band.0 || ratio > o.band.1 { "!" } else { " " };
                        print!(" {target:>6.2} {ratio:>6.2}{flag}");
                        ratios[m] = Some(ratio);
                    }
                    None => print!(" {:>6} {:>7}", "—", ""),
                }
            }
            let strong = |kind: StrongKind| strong_means(&full, |a| a.kind == kind && a.minute == m);
            let cells = |s: Option<StrongMeans>| match s {
                Some(s) => format!(" {:>8.0} {:>6.1} {:>5.1}", s.health, s.kill_seconds, s.hit_share * 100.0),
                None => format!(" {:>8} {:>6} {:>5}", "", "", ""),
            };
            let elite = strong(StrongKind::Elite);
            print!("{}{}", cells(elite), cells(strong(StrongKind::Rare)));
            let partial = if mm.runs < full.len() { format!("  ({} of {} runs)", mm.runs, full.len()) } else { String::new() };
            println!(" {:>5.2}{partial}", mm.cheat_deaths);
            let boss = strong_means(&full, |a| matches!(a.kind, StrongKind::Boss(_)) && a.minute == m);
            let opt = |v: Option<f64>, digits: usize| v.map_or(String::new(), |v| format!("{v:.digits$}"));
            csv += &format!(
                "{},{},{},{:.2},{:.1},{:.1},{:.2},{:.3},{},{},{},{:.2}\n",
                zone.id,
                zone.difficulty,
                m + 1,
                mm.level,
                mm.dps,
                mm.health,
                mm.enemy_damage,
                mm.kill_seconds,
                opt(elite.map(|e| e.health), 1),
                opt(boss.map(|b| b.health), 1),
                opt(boss.map(|b| b.kill_seconds), 2),
                mm.cheat_deaths
            );
        }
        println!("\n{:>6} {:>6} {:>7} {:>10} {:>8} {:>8} {:>7} {:>7} {:>8} {:>7}  name", "boss", "final", "minute", "health", "damage", "kill s", "hit%", "lived s", "killed", "on it%");
        for (bi, b) in zone.bosses.iter().enumerate() {
            let Some(s) = strong_means(&full, |a| a.kind == StrongKind::Boss(bi)) else { continue };
            let fin = if b.is_final { "yes" } else { "" };
            println!(
                "{:>6} {:>6} {:>7.1} {:>10.0} {:>8.0} {:>8.1} {:>7.1} {:>7} {:>8} {:>7}  {}",
                bi + 1,
                fin,
                s.minute,
                s.health,
                s.damage,
                s.kill_seconds,
                s.hit_share * 100.0,
                s.lived.map_or("—".to_string(), |l| format!("{l:.1}")),
                format!("{}/{}", s.killed, s.count),
                s.share.map_or("—".to_string(), |x| format!("{:.0}", x * 100.0)),
                b.name
            );
        }
        // each boss's biggest single hit, the largest over the runs, by share of its health
        for (bi, b) in zone.bosses.iter().enumerate() {
            let top = full.iter().flat_map(|r| &r.arrivals).filter(|a| a.kind == StrongKind::Boss(bi)).max_by(|x, y| (x.biggest.0 / x.health).total_cmp(&(y.biggest.0 / y.health)));
            if let Some(a) = top {
                let mut shares: Vec<f64> = full.iter().flat_map(|r| &r.arrivals).filter(|a| a.kind == StrongKind::Boss(bi)).map(|a| a.biggest.0 / a.health).collect();
                shares.sort_by(f64::total_cmp);
                println!("biggest hit on {}: {:.0} of {:.0} health ({:.0} %, {}); median run's biggest {:.0} %", b.name, a.biggest.0, a.health, 100.0 * a.biggest.0 / a.health, a.biggest.1, 100.0 * shares[shares.len() / 2]);
            }
        }
        // `--prefer`: each class's bosses, seconds alive (those killed) and how many died
        if !o.prefer.is_empty() {
            print!("{:10}", "lived s");
            for b in &zone.bosses {
                print!(" {:>16}", b.name.chars().take(16).collect::<String>());
            }
            println!();
            for (ci, class) in content.classes.iter().enumerate() {
                let own: Vec<&RunResult> = jobs.iter().zip(&results).filter(|(j, _)| j.class == ci).filter_map(|(_, r)| r.as_ref()).collect();
                if own.is_empty() {
                    continue;
                }
                print!("{:10}", class.id);
                for bi in 0..zone.bosses.len() {
                    let cell = strong_means(&own, |a| a.kind == StrongKind::Boss(bi))
                        .map_or("—".to_string(), |s| format!("{} {}/{}", s.lived.map_or("—".to_string(), |l| format!("{l:.0}")), s.killed, s.count));
                    print!(" {cell:>16}");
                }
                println!();
            }
        }
        for (name, kind) in [("elites", StrongKind::Elite), ("rares", StrongKind::Rare)] {
            if let Some(s) = strong_means(&full, |a| a.kind == kind && (4..18).contains(&a.minute)) {
                println!(
                    "{name} at minutes 5-18: {} seen, kill {:.1} s, hit {:.1} % of the hero's health",
                    s.count,
                    s.kill_seconds,
                    s.hit_share * 100.0
                );
            }
        }
        println!();
        summary.push((zone.id.clone(), ratios));
        // the next zone's hero has this zone's won run behind it: 20 minutes, every boss, the
        // mean kills measured here
        experience += won_run_experience(&content, zi, kills);
    }
    println!("Talent points after a won run in every zone: {}", talent_points(experience).0);
    let schedule: Vec<String> =
        content.zones.iter().zip(&arrival_points).map(|(z, p)| format!("{} {p}", z.id)).collect();
    println!("Points on arrival: {}\n", schedule.join(", "));
    if targets.is_some() {
        println!("Measured ÷ target kill seconds of normal enemies (mean = geometric mean of minutes 5-18):\n");
        println!("{:12} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}", "zone", "m5", "m10", "m15", "m18", "mean", "low", "high");
        for (zone, r) in &summary {
            let cell = |v: Option<f64>| v.map_or("—".to_string(), |v| format!("{v:.2}{}", if v < o.band.0 || v > o.band.1 { "!" } else { "" }));
            let mid: Vec<f64> = r[4..18].iter().flatten().copied().collect();
            let geo = mean(mid.iter().map(|v| v.ln())).map(f64::exp);
            let low = mid.iter().copied().reduce(f64::min);
            let high = mid.iter().copied().reduce(f64::max);
            let at = |m: usize| r[m - 1];
            println!(
                "{zone:12} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
                cell(at(5)),
                cell(at(10)),
                cell(at(15)),
                cell(at(18)),
                cell(geo),
                cell(low),
                cell(high)
            );
        }
        println!();
    }
    if o.gear {
        gear_check(&content, &o, &arrival_points);
    }
    if o.milestones {
        milestone_check(&content, &o, &arrival_points);
    }
    if let Some(path) = &o.csv {
        fs::write(path, &csv).unwrap_or_else(|e| panic!("the csv file {path} is written: {e}"));
        println!("Wrote {path}.");
    }
    println!("Done in {:.0} s.", started.elapsed().as_secs_f64());
}

/// Every zone's arrival hero with the set it wears against the same hero with the set before it.
fn gear_check(content: &Arc<Content>, o: &Options, arrival_points: &[u32]) {
    // (zone, wears the previous zone's set) per job
    let mut arms = Vec::new();
    let mut jobs = Vec::new();
    for zi in 1..content.zones.len() {
        let points = arrival_points[zi];
        for set_from in [zi.checked_sub(1), zi.checked_sub(2)] {
            let more = zone_jobs(content, zi, o.check_seeds, |class| {
                Some(Loadout { ranks: spend_talent_points(content, class, points), set_from, without_milestone: None, without_whole_talent: false, weapons: None, start: None })
            });
            arms.extend(more.iter().map(|_| (zi, set_from == zi.checked_sub(1))));
            jobs.extend(more);
        }
    }
    let results = play_all(content, &jobs, o.threads);
    println!("Gear: each zone's set against the set before it, worn by the arrival hero of the zone after it (run DPS):\n");
    println!("{:12} {:>12} {:>8} {:>6} {:>8} {:>8} {:>8}  by class", "set", "worn in", "gain", "±", "budget", "low", "high");
    for zi in 1..content.zones.len() {
        let mut gains = Vec::new();
        for ci in 0..content.classes.len() {
            let dps = |new: bool| {
                let runs = jobs.iter().zip(&results).zip(&arms).filter(|((j, _), a)| j.class == ci && **a == (zi, new));
                mean_and_error(runs.filter_map(|((_, r), _)| r.as_ref()).map(RunResult::dps))
            };
            if let (Some(w), Some(b)) = (dps(true), dps(false)) {
                let (gain, error) = gain_and_error(w, b);
                gains.push((content.classes[ci].id.as_str(), gain, error));
            }
        }
        let Some(gain) = mean(gains.iter().map(|g| g.1)) else { continue };
        let error = gains.iter().map(|g| g.2 * g.2).sum::<f64>().sqrt() / gains.len() as f64;
        // a set worn by a hero who already had every talent point stands in for a zone of talents
        let budget = if arrival_points[zi - 1] >= MAX_TALENT_POINTS { GEAR_BUDGET_AFTER_TALENTS } else { GEAR_BUDGET };
        let low = gains.iter().map(|g| g.1).fold(f64::MAX, f64::min);
        let high = gains.iter().map(|g| g.1).fold(f64::MIN, f64::max);
        let classes: Vec<String> = gains.iter().map(|(c, g, _)| format!("{c} {:+.0}", g * 100.0)).collect();
        println!(
            "{:12} {:>12} {:>+7.1}% {:>5.1}% {:>+7.0}% {:>+7.0}% {:>+7.0}%  {}",
            content.zones[zi - 1].id,
            content.zones[zi].id,
            gain * 100.0,
            error * 100.0,
            budget * 100.0,
            low * 100.0,
            high * 100.0,
            classes.join(", ")
        );
    }
    println!();
}

/// For every class, tree and milestone row, the row's milestone talent with its milestones and
/// without them, in the first zone whose arrival points reach the row.
fn milestone_check(content: &Arc<Content>, o: &Options, arrival_points: &[u32]) {
    struct Case {
        class: String,
        tree: usize,
        row: u8,
        talent: String,
        role: &'static str,
        label: String,
        zone: usize,
        budget: f64,
        start: String,
    }
    let mut cases = Vec::new();
    let mut templates: Vec<(usize, bool, Job)> = Vec::new();
    for class in &content.classes {
        for (ti, tree) in content.trees_of(&class.id).enumerate() {
            for &(row, budget) in &MILESTONE_BUDGETS {
                let need = POINTS_PER_ROW * u32::from(row) + 1;
                let Some(first) = arrival_points.iter().position(|&p| p >= need) else {
                    println!("milestones: no zone's arrival hero has {need} points for row {row}");
                    continue;
                };
                // the first zone whose arrival hero reaches the row, then `--also-in`'s later ones
                let zones: Vec<usize> = std::iter::once(first)
                    .chain(o.also_in.iter().filter_map(|id| content.zones.iter().position(|z| z.id == *id)).filter(|&z| z > first))
                    .collect();
                for t in tree.talents.iter().filter(|t| t.row == row && (o.talents.is_empty() || o.talents.contains(&t.id))) {
                    let milestones: Vec<&Milestone> =
                        t.effects.iter().filter_map(|e| if let TalentEffect::Milestone(m) = e { Some(m) } else { None }).collect();
                    if !t.is_milestone() {
                        continue;
                    }
                    // the talents it needs first, then it, then the tree deepest first, then the others
                    let mut wanted = vec![t.id.clone()];
                    let mut needs = t.requires.clone();
                    while let Some(id) = needs {
                        needs = tree.talents.iter().find(|x| x.id == id).and_then(|x| x.requires.clone());
                        wanted.insert(0, id);
                    }
                    let order: Vec<usize> = std::iter::once(ti).chain((0..3).filter(|&x| x != ti)).collect();
                    for &zone in &zones {
                    let ranks = spend_points_in_order(content, &class.id, arrival_points[zone], &order, &wanted);
                    if ranks.get(&t.id).copied().unwrap_or(0) == 0 {
                        println!("milestones: {} {} row {row}: {} not reached with {} points", class.id, ti, t.id, arrival_points[zone]);
                        continue;
                    }
                    // a milestone of plain numbers (no rule change) makes spells stronger: damage
                    // a talent that also raises spell power or a damage stat is a damage milestone
                    let raises_damage = t.effects.iter().any(|e| match e {
                        TalentEffect::AbilityMod { kind, .. } => *kind == Upgrade::Power,
                        TalentEffect::Stat { stat, .. } => matches!(stat, Stat::Damage | Stat::CritChance | Stat::CritDamage | Stat::WeaponDamage | Stat::SwingSpeed | Stat::PetDamage | Stat::DoubleCast | Stat::CooldownRecovery | Stat::SpellCooldownRecovery | Stat::DamageVsCrowdControlled | Stat::DamageFromReduction),
                        TalentEffect::CooldownSeconds { .. } | TalentEffect::Milestone(_) => false,
                    });
                    let role = milestones.iter().map(|m| milestone_role(m)).find(|r| *r == "damage").or(raises_damage.then_some("damage")).or(milestones.first().map(|m| milestone_role(m))).unwrap_or("damage");
                    let label = if milestones.is_empty() { "numbers only".to_string() } else { milestones.iter().map(|m| milestone_label(m)).collect::<Vec<_>>().join(" + ") };
                    // `--start all`: a case for each of the class's starting options; `--start <id>`:
                    // the class plays that option when it has one, else its first
                    let starts: Vec<Option<String>> = if o.start_all {
                        class.starting_options.iter().map(|x| Some(x.id.clone())).collect()
                    } else {
                        vec![o.start.clone().filter(|id| class.starting_options.iter().any(|x| x.id == *id))]
                    };
                    for start in starts {
                    let case = cases.len();
                    let start_id = start.clone().unwrap_or_else(|| class.starting_options[0].id.clone());
                    cases.push(Case { class: class.id.clone(), tree: ti, row, talent: t.id.clone(), role, label: label.clone(), zone, budget, start: start_id });
                    let set_from = zone.checked_sub(1);
                    // only damage milestones are held to a budget; the others are listed, not played
                    let arms: &[bool] = if role == "damage" { &[false, true] } else { &[] };
                    for &without in arms {
                        let start = start.clone();
                        let loadout = Loadout { ranks: ranks.clone(), set_from, without_milestone: without.then(|| t.id.clone()), without_whole_talent: o.whole_talent, weapons: None, start };
                        // `--fair-start`: without the milestone the hero takes the spell it would
                        // start with the first time a card offers it, as a player building that tree
                        let prefer: Vec<String> = if without && o.fair_start {
                            t.effects.iter().filter_map(|e| match e { TalentEffect::Milestone(Milestone::StartWith { spell }) => Some(spell.clone()), _ => None }).collect()
                        } else {
                            Vec::new()
                        };
                        let ci = content.classes.iter().position(|c| c.id == class.id).unwrap_or(0);
                        let seed = zone_seed(&content.zones[zone].id) + ((ci as u64) << 40);
                        // with `--kite` the hero kites as its option would
                        let option = loadout.start.as_deref().and_then(|id| content.starting_option(id)).unwrap_or(&content.classes[ci].starting_options[0]);
                        let kite = o.kite.then(|| kite_stand_off(content, option));
                        // the seeds are added batch by batch below (`--adaptive`)
                        templates.push((case, without, Job { zone, class: ci, seed, loadout: loadout.clone(), kite, block: o.block, no_amulets: o.no_amulets, prefer: prefer.clone() }));
                    }
                    }
                    }
                }
            }
        }
    }
    // the runs, batch by batch: every case plays seeds `played..played + batch` in both arms; with
    // `--adaptive` a talent stops as soon as its gain is clear (`milestone_settled`)
    let mut jobs: Vec<(usize, bool, Job)> = Vec::new();
    let mut results: Vec<Option<RunResult>> = Vec::new();
    let key = |c: &Case| (c.class.clone(), c.tree, c.row, c.talent.clone(), c.zone);
    let mut open: Vec<bool> = vec![true; cases.len()];
    let batch = if o.adaptive { o.adaptive_batch.max(1) } else { o.check_seeds.max(1) };
    let mut played = 0;
    while played < o.check_seeds && open.iter().any(|&x| x) {
        let n = batch.min(o.check_seeds - played);
        let mut fresh = Vec::new();
        for (case, without, j) in &templates {
            if !open[*case] {
                continue;
            }
            for s in played..played + n {
                fresh.push((*case, *without, Job { zone: j.zone, class: j.class, seed: j.seed + s, loadout: j.loadout.clone(), kite: j.kite, block: j.block, no_amulets: j.no_amulets, prefer: j.prefer.clone() }));
            }
        }
        let plain: Vec<Job> = fresh.iter().map(|(_, _, j)| Job { zone: j.zone, class: j.class, seed: j.seed, loadout: j.loadout.clone(), kite: j.kite, block: j.block, no_amulets: j.no_amulets, prefer: j.prefer.clone() }).collect();
        results.extend(play_all(content, &plain, o.threads));
        jobs.extend(fresh);
        played += n;
        // a talent settles only once it has `--adaptive-min` seeds a side (a first batch can mislead)
        if o.adaptive && played >= o.adaptive_min {
            for ci in 0..cases.len() {
                if !open[ci] {
                    continue;
                }
                // the talent's gain over its starting options (the mean) and its error
                let k = key(&cases[ci]);
                let mine: Vec<usize> = (0..cases.len()).filter(|&x| key(&cases[x]) == k).collect();
                let stats: Vec<(f64, f64)> = mine.iter().filter_map(|&x| case_gain(&jobs, &results, x)).collect();
                if stats.len() == mine.len() && !stats.is_empty() {
                    let m = stats.len() as f64;
                    let gain = stats.iter().map(|s| s.0).sum::<f64>() / m;
                    let error = stats.iter().map(|s| s.1 * s.1).sum::<f64>().sqrt() / m;
                    if milestone_settled(gain, error, cases[ci].budget) {
                        for &x in &mine {
                            open[x] = false;
                        }
                    }
                } else if mine.iter().all(|&x| cases[x].role != "damage") {
                    for &x in &mine {
                        open[x] = false;
                    }
                }
            }
        }
    }
    println!("Milestones: run DPS with the row's milestone and without it, in the first zone whose arrival hero reaches the row:\n");
    println!(
        "{:8} {:16} {:>4} {:>3} {:24} {:9} {:>9} {:>10} {:>10} {:>7} {:>6} {:>7} {:>5}  milestone",
        "class", "start", "tree", "row", "talent", "role", "zone", "with", "without", "gain", "±", "budget", "runs"
    );
    for (ci, c) in cases.iter().enumerate() {
        let dps = |without: bool| {
            let runs = jobs.iter().zip(&results).filter(|((k, w, _), _)| *k == ci && *w == without);
            // a run that ended at once has no DPS (left out)
            mean_and_error(runs.filter_map(|(_, r)| r.as_ref()).map(RunResult::dps).filter(|d| d.is_finite()))
        };
        let (Some(with), Some(without)) = (dps(false), dps(true)) else {
            let zone = &content.zones[c.zone].id;
            println!("{:8} {:16} {:>4} {:>3} {:24} {:9} {:>9} {:>46}  {}", c.class, c.start, c.tree, c.row, c.talent, c.role, zone, "not played", c.label);
            continue;
        };
        let Some((gain, error)) = case_gain(&jobs, &results, ci) else { continue };
        let runs = jobs.iter().filter(|(k, w, _)| *k == ci && !*w).count();
        let off = c.role == "damage" && (gain < c.budget * (1.0 - MILESTONE_TOLERANCE) || gain > c.budget * (1.0 + MILESTONE_TOLERANCE));
        println!(
            "{:8} {:16} {:>4} {:>3} {:24} {:9} {:>9} {:>10.0} {:>10.0} {:>+6.1}% {:>5.1}% {:>+6.0}%{} {:>5} {}",
            c.class,
            c.start,
            c.tree,
            c.row,
            c.talent,
            c.role,
            content.zones[c.zone].id,
            with.0,
            without.0,
            gain * 100.0,
            error * 100.0,
            c.budget * 100.0,
            if off { "!" } else { " " },
            runs,
            c.label
        );
    }
    println!();
}


/// Our budget for a starting option's level-1 strike and its passive, a second:
/// 40 and 50; the rage spenders (Warrior, Bear), whose passive deals no damage, put both into the
/// strike: 90; a strike and its finisher (Rogue, Cat) share 90 as 50 and 40.
fn budget_of(option: &StartingOption) -> (f64, f64) {
    if matches!(option.resource, Some(Resource::Rage { .. })) {
        (90.0, 0.0)
    } else if option.spells.len() > 1 {
        (50.0, 40.0)
    } else {
        (40.0, 50.0)
    }
}

/// `--budgets`: every starting option's strike and passive alone, against their budgets. The
/// strike is the level-1 spell with every passive off (a pair's finisher left out); the passive
/// is what turning it on adds to all the option deals (its own damage, or a stronger strike or
/// swing), plus the pair's finisher.
fn budgets(content: &Arc<Content>, o: &Options) {
    let zone = o.start_zones.first().cloned().unwrap_or_else(|| "elwynn".into());
    let mut quiet = (**content).clone();
    for opt in quiet.classes.iter_mut().flat_map(|c| c.starting_options.iter_mut()) {
        opt.passive.kind = PassiveKind::None;
    }
    let quiet = Arc::new(quiet);
    let options: Vec<(String, String)> =
        content.classes.iter().flat_map(|c| c.starting_options.iter().map(move |s| (c.id.clone(), s.id.clone()))).collect();
    let arena = |start: &str, one: bool| {
        let kite = o.kite.then(|| kite_stand_off(content, content.starting_option(start).expect("an option")));
        if one { StartArena::OneEnemy } else { StartArena::Swarm(kite) }
    };
    // (option, one enemy, passive on, seed)
    let jobs: Vec<(usize, bool, bool, u64)> = (0..options.len())
        .flat_map(|oi| [false, true].into_iter().flat_map(move |one| [true, false].into_iter().flat_map(move |on| (0..o.seeds).map(move |s| (oi, one, on, s)))))
        .collect();
    let samples = parallel_map(&jobs, o.threads, |&(oi, one, on, s)| {
        let (class, start) = &options[oi];
        let loadout = Loadout { ranks: Ranks::new(), set_from: None, without_milestone: None, without_whole_talent: false, weapons: None, start: Some(start.clone()) };
        let seed = zone_seed(&zone) + ((oi as u64) << 32) + s;
        measure_start(if on { content } else { &quiet }, &zone, class, start, &loadout, seed, arena(start, one))
    });
    println!(
        "Budgets: each option's level-1 spell alone for {STARTS_SECONDS} s at level 1 in {zone}, no gear, {} seeds, {}; the passive is what turning it on adds, plus a pair's finisher.\n",
        o.seeds,
        if o.kite { "kiting" } else { "standing" }
    );
    println!("| class | option | strike /s | budget | ÷ | passive /s | budget | ÷ | finisher | fires/min | one enemy: strike | one enemy: passive |");
    println!("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    let mut csv = String::from("class,option,strike,strike_budget,passive,passive_budget,finisher,fires_per_minute,one_strike,one_passive\n");
    for (oi, (class, start)) in options.iter().enumerate() {
        let option = content.starting_option(start).expect("an option");
        let (sb, pb) = budget_of(option);
        let mean_of = |one: bool, on: bool, f: &dyn Fn(&StartSample) -> f64| {
            mean(jobs.iter().zip(&samples).filter(|((i, w, p, _), _)| *i == oi && *w == one && *p == on).map(|(_, x)| f(x))).unwrap_or(0.0)
        };
        let all = |x: &StartSample| x.spell + x.passive + x.swing;
        // the option's own pet (Kill Command's) counts as its passive
        let strike = |one: bool| mean_of(one, false, &|x: &StartSample| x.spell - x.finisher - x.pet);
        let passive = |one: bool| mean_of(one, true, &all) - mean_of(one, false, &all) + mean_of(one, false, &|x: &StartSample| x.finisher + x.pet);
        let fires = mean_of(false, true, &|x: &StartSample| x.passive_fires) / (f64::from(STARTS_SECONDS) / 60.0);
        let (st, pa, fi) = (strike(false), passive(false), mean_of(false, false, &|x: &StartSample| x.finisher));
        let (st1, pa1) = (strike(true), passive(true));
        let ratio = |v: f64, b: f64| if b > 0.0 { format!("{:.2}", v / b) } else { "—".into() };
        println!(
            "| {class} | {start} | {st:.1} | {sb:.0} | {} | {pa:.1} | {pb:.0} | {} | {fi:.1} | {fires:.1} | {st1:.1} | {pa1:.1} |",
            ratio(st, sb),
            ratio(pa, pb)
        );
        csv.push_str(&format!("{class},{start},{st:.2},{sb},{pa:.2},{pb},{fi:.2},{fires:.2},{st1:.2},{pa1:.2}\n"));
    }
    if let Some(path) = &o.csv {
        fs::write(path, csv).expect("the csv is written");
        println!("wrote {path}");
    }
}

/// `--starter-runs`: whole runs of every starting option in every zone of `--zones`.
fn starter_runs(content: &Arc<Content>, o: &Options) {
    let zones: Vec<usize> = o
        .start_zones
        .iter()
        .map(|id| content.zones.iter().position(|z| z.id == *id).unwrap_or_else(|| panic!("--zones: no zone {id}")))
        .collect();
    let points = if o.no_talents { vec![0; content.zones.len()] } else { arrival_points(content, o.seeds, o.threads) };
    let options: Vec<(String, String)> =
        content.classes.iter().flat_map(|c| c.starting_options.iter().map(move |s| (c.id.clone(), s.id.clone()))).collect();
    let jobs: Vec<(usize, usize, u64)> =
        zones.iter().flat_map(|&zi| (0..options.len()).flat_map(move |oi| (0..o.seeds).map(move |s| (zi, oi, s)))).collect();
    let results = parallel_map(&jobs, o.threads, |&(zi, oi, s)| {
        let (class, start) = &options[oi];
        let zone = &content.zones[zi].id;
        let loadout = Loadout {
            ranks: spend_talent_points(content, class, points[zi]),
            set_from: zi.checked_sub(1),
            without_milestone: None,
            without_whole_talent: false,
            weapons: None,
            start: Some(start.clone()),
        };
        let setup = hero_setup(content, class, &loadout);
        let kite = o.kite.then(|| kite_stand_off(content, content.starting_option(start).expect("an option")));
        let seed = zone_seed(zone) + ((oi as u64) << 32) + s;
        let r = std::panic::catch_unwind(AssertUnwindSafe(|| play_picking(content, zone, setup, seed, kite, o.no_amulets)));
        if r.is_err() {
            eprintln!("balance check: the run of {zone}/{start} seed {seed} panicked; it is left out");
        }
        r.ok()
    });
    println!(
        "Starting options, whole runs: {} seeds each, {}, {}, {}, the previous zone's set.\n",
        o.seeds,
        if o.kite { "kiting" } else { "standing" },
        if o.no_talents { "no talents" } else { "arrival talents" },
        if o.no_amulets { "no amulets" } else { "first offers" }
    );
    let mut csv = String::from("zone,class,option,runs,dps,error,vs_class,vs_median,dps_m5,dps_m10,dps_m15,level_m15,kills,cheat_deaths_per_minute,won\n");
    for &zi in &zones {
        let zone = &content.zones[zi].id;
        let worn = zi.checked_sub(1).map_or("no".to_string(), |z| format!("{}'s", content.zones[z].id));
        // per option: its runs
        let runs: Vec<Vec<&RunResult>> = (0..options.len())
            .map(|oi| jobs.iter().zip(&results).filter(|((z, i, _), _)| *z == zi && *i == oi).filter_map(|(_, r)| r.as_ref()).collect())
            .collect();
        let dps: Vec<Option<(f64, f64)>> = runs.iter().map(|rs| mean_and_error(rs.iter().map(|r| r.dps()).filter(|d| d.is_finite()))).collect();
        let mut sorted: Vec<f64> = dps.iter().flatten().map(|d| d.0).collect();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let median = sorted.get(sorted.len() / 2).copied().unwrap_or(1.0);
        println!("## {zone} ({} talent points, {worn} gear; zone median {median:.0} DPS)\n", points[zi]);
        println!("| class | option | run DPS | ± | ÷ class mean | ÷ median | DPS m5 | DPS m10 | DPS m15 | level m15 | kills | cheat deaths/min | won |");
        println!("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
        for (oi, (class, start)) in options.iter().enumerate() {
            let Some((d, e)) = dps[oi] else { continue };
            let mine: Vec<usize> = (0..options.len()).filter(|&k| options[k].0 == *class).collect();
            let class_mean = mine.iter().filter_map(|&k| dps[k].map(|x| x.0)).sum::<f64>() / mine.len().max(1) as f64;
            let rs = &runs[oi];
            let minute = |m: usize| mean(rs.iter().filter_map(|r| r.minutes.get(m - 1).copied().flatten()).map(|x| x.damage / 60.0)).unwrap_or(f64::NAN);
            let level = mean(rs.iter().filter_map(|r| r.minutes.get(14).copied().flatten()).map(|x| x.level)).unwrap_or(f64::NAN);
            let kills = mean(rs.iter().map(|r| f64::from(r.kills))).unwrap_or(0.0);
            let minutes_played: f64 = rs.iter().map(|r| f64::from(r.seconds) / 60.0).sum();
            let cheats: f64 = rs.iter().map(|r| r.minutes.iter().flatten().map(|x| f64::from(x.cheat_deaths)).sum::<f64>()).sum();
            let won = rs.iter().filter(|r| r.seconds < MINUTES as f32 * 60.0).count();
            println!(
                "| {class} | {start} | {d:.0} | {e:.0} | {:.2} | {:.2} | {:.0} | {:.0} | {:.0} | {level:.1} | {kills:.0} | {:.2} | {won}/{} |",
                d / class_mean,
                d / median,
                minute(5),
                minute(10),
                minute(15),
                cheats / minutes_played.max(1e-9),
                rs.len()
            );
            csv.push_str(&format!(
                "{zone},{class},{start},{},{d:.0},{e:.0},{:.3},{:.3},{:.0},{:.0},{:.0},{level:.1},{kills:.0},{:.3},{won}\n",
                rs.len(),
                d / class_mean,
                d / median,
                minute(5),
                minute(10),
                minute(15),
                cheats / minutes_played.max(1e-9)
            ));
        }
        println!();
    }
    if let Some(path) = &o.csv {
        fs::write(path, csv).expect("the csv is written");
        println!("wrote {path}");
    }
}

/// `--loadouts`: every class's three weapon loadouts compared (roadmap
/// R3), each played with every starting option of the class, `--seeds` whole runs a pair in each
/// zone of `--zones`, kiting and declining amulets as `--starter-runs --kite --no-amulets` does
/// (arrival talents unless `--no-talents`, the previous zone's set). Per zone and class, a row per
/// loadout: its run DPS over the class's starters, against the mean of the class's loadouts, each
/// starter's DPS, the cheat deaths (the deaths the hero would have had) a run and a minute, and
/// the runs won. `--csv <file>` writes one row per zone, loadout and starter.
fn loadout_runs(content: &Arc<Content>, o: &Options) {
    let zones: Vec<usize> = o
        .start_zones
        .iter()
        .map(|id| content.zones.iter().position(|z| z.id == *id).unwrap_or_else(|| panic!("--zones: no zone {id}")))
        .collect();
    let points = if o.no_talents { vec![0; content.zones.len()] } else { arrival_points(content, o.seeds, o.threads) };
    // (class, starting option, weapon loadout)
    let pairs: Vec<(String, String, String)> = content
        .classes
        .iter()
        .flat_map(|c| {
            let starts = c.starting_options.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
            content.loadouts_of(&c.id).iter().flat_map(move |l| starts.clone().into_iter().map(move |s| (c.id.clone(), s, l.id.clone())))
        })
        .collect();
    let jobs: Vec<(usize, usize, u64)> =
        zones.iter().flat_map(|&zi| (0..pairs.len()).flat_map(move |pi| (0..o.seeds).map(move |s| (zi, pi, s)))).collect();
    let results = parallel_map(&jobs, o.threads, |&(zi, pi, s)| {
        let (class, start, weapons) = &pairs[pi];
        let zone = &content.zones[zi].id;
        let loadout = Loadout {
            ranks: spend_talent_points(content, class, points[zi]),
            set_from: zi.checked_sub(1),
            without_milestone: None,
            without_whole_talent: false,
            weapons: Some(weapons.clone()),
            start: Some(start.clone()),
        };
        let setup = hero_setup(content, class, &loadout);
        let kite = Some(kite_stand_off(content, content.starting_option(start).expect("an option")));
        // a starter's seeds are the same whatever its loadout, so the loadouts meet the same runs
        let option = content.classes.iter().flat_map(|c| &c.starting_options).position(|x| x.id == *start).unwrap_or(0);
        let seed = zone_seed(zone) + ((option as u64) << 32) + s;
        let r = std::panic::catch_unwind(AssertUnwindSafe(|| play_picking(content, zone, setup, seed, kite, true)));
        if r.is_err() {
            eprintln!("balance check: the run of {zone}/{class}/{start} with {weapons} seed {seed} panicked; it is left out");
        }
        r.ok()
    });
    println!(
        "Weapon loadouts: every class's starting options with each of its loadouts, whole runs, {} seeds a pair, kiting, {}, no amulets, the previous zone's set.\n",
        o.seeds,
        if o.no_talents { "no talents" } else { "arrival talents" },
    );
    let mut csv = String::from("zone,class,loadout,option,runs,dps,error,cheat_deaths_per_run,cheat_deaths_per_minute,won\n");
    for &zi in &zones {
        let zone = &content.zones[zi].id;
        let worn = zi.checked_sub(1).map_or("no".to_string(), |z| format!("{}'s", content.zones[z].id));
        println!("## {zone} ({} talent points, {worn} gear)\n", points[zi]);
        println!("| class | loadout | run DPS | ÷ class mean | per starter | cheat deaths/run | cheat deaths/min | won |");
        println!("|---|---|---:|---:|---|---:|---:|---:|");
        let runs_of = |pi: usize| -> Vec<&RunResult> {
            jobs.iter().zip(&results).filter(|((z, p, _), _)| *z == zi && *p == pi).filter_map(|(_, r)| r.as_ref()).collect()
        };
        for class in &content.classes {
            let ids: Vec<&str> = content.loadouts_of(&class.id).iter().map(|l| l.id.as_str()).collect();
            // per loadout: (its id, mean DPS over its starters, the starters' DPS, cheat deaths, minutes, runs, won)
            let mut rows = Vec::new();
            for id in &ids {
                let mine: Vec<usize> = (0..pairs.len()).filter(|&p| pairs[p].0 == class.id && pairs[p].2 == *id).collect();
                let mut per_start = Vec::new();
                let (mut cheats, mut minutes, mut count, mut won) = (0.0, 0.0, 0usize, 0usize);
                for &p in &mine {
                    let rs = runs_of(p);
                    let Some((d, e)) = mean_and_error(rs.iter().map(|r| r.dps()).filter(|d| d.is_finite())) else { continue };
                    let c: f64 = rs.iter().map(|r| r.minutes.iter().flatten().map(|x| f64::from(x.cheat_deaths)).sum::<f64>()).sum();
                    let m: f64 = rs.iter().map(|r| f64::from(r.seconds) / 60.0).sum();
                    let w = rs.iter().filter(|r| r.seconds < MINUTES as f32 * 60.0).count();
                    csv.push_str(&format!(
                        "{zone},{},{id},{},{},{d:.0},{e:.0},{:.2},{:.3},{w}\n",
                        class.id,
                        pairs[p].1,
                        rs.len(),
                        c / rs.len().max(1) as f64,
                        c / m.max(1e-9)
                    ));
                    per_start.push((pairs[p].1.clone(), d));
                    (cheats, minutes, count, won) = (cheats + c, minutes + m, count + rs.len(), won + w);
                }
                let dps = mean(per_start.iter().map(|(_, d)| *d));
                rows.push((*id, dps, per_start, cheats, minutes, count, won));
            }
            let class_mean = mean(rows.iter().filter_map(|r| r.1)).unwrap_or(f64::NAN);
            for (id, dps, per_start, cheats, minutes, count, won) in rows {
                let Some(dps) = dps else { continue };
                let starters: Vec<String> = per_start.iter().map(|(s, d)| format!("{s} {d:.0}")).collect();
                println!(
                    "| {} | {id} | {dps:.0} | {:.2} | {} | {:.2} | {:.2} | {won}/{count} |",
                    class.id,
                    dps / class_mean,
                    starters.join(", "),
                    cheats / count.max(1) as f64,
                    cheats / minutes.max(1e-9)
                );
            }
        }
        println!();
    }
    if let Some(path) = &o.csv {
        fs::write(path, csv).expect("the csv is written");
        println!("wrote {path}");
    }
}

/// Run seconds of each `--starts` run of the level-1 spell alone (in the swarm or on one enemy).
const STARTS_SECONDS: f32 = 300.0;
/// The zones `--starts` measures in unless `--zones` names others.
const STARTS_ZONES: [&str; 3] = ["elwynn", "feralas", "aq"];
/// The talent tree (by its place among the class's three) of the spec each starting option comes
/// from, as the level-1 decisions name them; an option not listed is its place in the class.
const OPTION_TREES: [(&str, usize); 5] = [("kill_command", 0), ("steady_shot", 1), ("cat_form", 1), ("bear_form", 1), ("restoration", 2)];

/// The tree option `start` of `class` belongs to (`OPTION_TREES`).
fn option_tree(content: &Content, class: &str, start: &str) -> usize {
    OPTION_TREES.iter().find(|(id, _)| *id == start).map_or_else(
        || content.classes.iter().find(|c| c.id == class).and_then(|c| c.starting_options.iter().position(|o| o.id == start)).unwrap_or(0).min(2),
        |(_, t)| *t,
    )
}

/// The run minutes whose kill seconds `--starts` holds against the targets.
const STARTS_TARGET_MINUTES: [usize; 3] = [5, 10, 15];

/// `--kite`: the distance a kiting hero keeps from the nearest enemy, for options that fight in
/// melee and for the others (`kite_stand_off`).
const KITE_MELEE_STAND_OFF: f32 = 3.5;
const KITE_RANGED_STAND_OFF: f32 = 8.0;
/// `--boss-circle`: the smallest radius of the loop round a boss (yards).
const BOSS_CIRCLE_RADIUS: f32 = 6.0;
/// The enemies within this many yards of the hero are the pack it circles; with none that close
/// it runs at the nearest enemy.
const KITE_PACK_RADIUS: f32 = 30.0;
/// Within this many yards of the arena's edge the hero turns back in, harder the closer it gets.
const KITE_EDGE_MARGIN: f32 = 20.0;
/// How strongly the hero corrects its distance to the nearest enemy against circling (1: as much
/// as the circling itself, at two yards off).
const KITE_DISTANCE_WEIGHT: f32 = 2.0;
/// The hero picks its step among this many headings around it.
const KITE_HEADINGS: usize = 24;
/// Enemies within this many yards crowd a heading (the more, the closer and the straighter ahead).
const KITE_CROWD_RADIUS: f32 = 12.0;
/// What a heading is worth: its crowd counts against it; how close it is to the wanted heading
/// (circling at the stand-off) and to the last step's heading count for it.
const KITE_CROWD_WEIGHT: f32 = 1.0;
const KITE_WANTED_WEIGHT: f32 = 0.6;
const KITE_KEEP_WEIGHT: f32 = 0.3;

/// The distance a kiting hero keeps from the nearest enemy with option `o`: close for an option
/// that fights in melee (it spends rage or energy, or its level-1 spell reaches under 10 yd: a
/// swing proc, a shield, a spin, a strike), further for a caster or a shooter.
fn kite_stand_off(content: &Content, o: &StartingOption) -> f32 {
    let melee = o.resource.is_some() || content.ability(o.main_spell()).map_or(true, |a| a.range < 10.0);
    if melee {
        KITE_MELEE_STAND_OFF
    } else {
        KITE_RANGED_STAND_OFF
    }
}

/// Moves the hero one tick the way a player kites a swarm (`--kite`), always at its run speed.
///
/// * **Wanted heading:** circle the pack (the enemies within `KITE_PACK_RADIUS`) counter-clockwise
///   around its centre, stepping out from the nearest enemy when it is closer than `stand_off` and
///   in toward it when it is farther, so the front of the pack stays at about that distance; within
///   `KITE_EDGE_MARGIN` of the arena's edge, turn back in. With no enemy within the pack radius,
///   run straight at the nearest one.
/// * **Step:** of `KITE_HEADINGS` headings around the hero, the one with the best worth: minus its
///   crowd (each enemy within `KITE_CROWD_RADIUS` counts by how straight ahead it is, squared, ÷
///   its distance), plus how close it is to the wanted heading and to the last step's (`heading`,
///   kept between calls). A hero closed in so runs out through the thinnest side, as a player would,
///   instead of standing still between enemies on both sides.
/// * **Facing:** the nearest enemy (the hero strafes), so cones point at the pack.
fn kite_step(run: &mut Run, stand_off: f32, heading: &mut Vec2, dt: f32) {
    let dist = |a: Vec2, b: Vec2| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    let unit = |v: Vec2| {
        let l = (v.0 * v.0 + v.1 * v.1).sqrt();
        if l < 1e-4 {
            (0.0, 0.0)
        } else {
            (v.0 / l, v.1 / l)
        }
    };
    let now = run.now;
    let h = &run.heroes[0];
    let (id, pos, speed) = (h.id, h.pos, h.stats.run_speed());
    let foes: Vec<Vec2> = run.enemies.iter().map(|e| e.segment.position(now)).collect();
    let Some(&nearest) = foes.iter().min_by(|a, b| dist(**a, pos).total_cmp(&dist(**b, pos))) else { return };
    let near = dist(nearest, pos);
    let mut wanted = if near > KITE_PACK_RADIUS {
        unit((nearest.0 - pos.0, nearest.1 - pos.1))
    } else {
        let pack: Vec<Vec2> = foes.iter().copied().filter(|f| dist(*f, pos) <= KITE_PACK_RADIUS).collect();
        let n = pack.len() as f32;
        let centre = (pack.iter().map(|f| f.0).sum::<f32>() / n, pack.iter().map(|f| f.1).sum::<f32>() / n);
        let from_nearest = unit((pos.0 - nearest.0, pos.1 - nearest.1));
        let mut out = unit((pos.0 - centre.0, pos.1 - centre.1));
        if out == (0.0, 0.0) {
            out = from_nearest;
        }
        let around = (-out.1, out.0);
        let step_out = ((stand_off - near) / 2.0).clamp(-1.0, 1.0) * KITE_DISTANCE_WEIGHT;
        unit((around.0 + from_nearest.0 * step_out, around.1 + from_nearest.1 * step_out))
    };
    // `--boss-circle`: while a boss lives, loop around it (the boss at the circle's centre) at the
    // stand-off, at least BOSS_CIRCLE_RADIUS, as players do while dodging its attacks
    let boss = run.enemies.iter().filter(|e| e.is_boss() && e.health > 0.0).map(|e| e.segment.position(now)).min_by(|a, b| dist(*a, pos).total_cmp(&dist(*b, pos)));
    let mut face = nearest;
    if let (true, Some(b)) = (BOSS_CIRCLE.load(Ordering::Relaxed), boss) {
        let radius = stand_off.max(BOSS_CIRCLE_RADIUS);
        let d = dist(b, pos);
        let out = if d < 1e-3 { (1.0, 0.0) } else { unit((pos.0 - b.0, pos.1 - b.1)) };
        let around = (-out.1, out.0);
        let step_out = ((radius - d) / 2.0).clamp(-1.0, 1.0) * KITE_DISTANCE_WEIGHT;
        wanted = unit((around.0 + out.0 * step_out, around.1 + out.1 * step_out));
        face = b;
    }
    let zone = &run.content.zones[run.zone];
    let centre = (zone.spawn.x, zone.spawn.y);
    let inner = zone.arena_radius - KITE_EDGE_MARGIN;
    let from_centre = dist(pos, centre);
    if from_centre > inner {
        let pull = (from_centre - inner) / KITE_EDGE_MARGIN * 3.0;
        let inward = unit((centre.0 - pos.0, centre.1 - pos.1));
        wanted = unit((wanted.0 + inward.0 * pull, wanted.1 + inward.1 * pull));
    }
    let crowd: Vec<(Vec2, f32)> =
        foes.iter().map(|f| (unit((f.0 - pos.0, f.1 - pos.1)), dist(*f, pos))).filter(|(_, d)| *d <= KITE_CROWD_RADIUS).collect();
    let worth = |dir: Vec2| {
        let crowded: f32 = crowd.iter().map(|(to, d)| (to.0 * dir.0 + to.1 * dir.1).max(0.0).powi(2) / d.max(1.0)).sum();
        let dot = |v: Vec2| v.0 * dir.0 + v.1 * dir.1;
        -KITE_CROWD_WEIGHT * crowded + KITE_WANTED_WEIGHT * dot(wanted) + KITE_KEEP_WEIGHT * dot(*heading)
    };
    let dir = (0..KITE_HEADINGS)
        .map(|k| {
            let a = k as f32 / KITE_HEADINGS as f32 * std::f32::consts::TAU;
            (a.cos(), a.sin())
        })
        .max_by(|a, b| worth(*a).total_cmp(&worth(*b)))
        .unwrap_or(wanted);
    *heading = dir;
    let to = (pos.0 + dir.0 * speed * dt, pos.1 + dir.1 * speed * dt);
    let facing = (face.1 - to.1).atan2(face.0 - to.0);
    run.hero_moved(id, to, 0.0, facing);
}

/// Where a `--starts` run of the level-1 spell alone is played.
#[derive(Clone, Copy, PartialEq)]
enum StartArena {
    /// The zone's swarm; the hero stands (`None`) or kites keeping this stand-off.
    Swarm(Option<f32>),
    /// One enemy in front of the standing hero that never strikes.
    OneEnemy,
}

/// What one run of a level-1 spell alone measured (damage a second unless it says otherwise).
#[derive(Clone, Copy, Default)]
struct StartSample {
    /// The option's own spells: its strike and finisher, what they bring (a poison, imps,
    /// critters) and its pet.
    spell: f64,
    /// Of `spell`, what its finisher dealt (Envenom, Eviscerate, Rupture, Rip and their ticks).
    finisher: f64,
    /// Of `spell`, what the option's own pet dealt (Kill Command's): the passive's, for the budget.
    pet: f64,
    /// The passive's own spells and summons (Radiance, an Infernal, Soulfire …).
    passive: f64,
    swing: f64,
    /// Everything else the hero dealt (talent milestones and their procs).
    other: f64,
    cheat_deaths_per_minute: f64,
    /// Rage or energy gained and kept a second (spent plus what was banked); 0 for other options.
    resource_income: f64,
    /// Share of the run the rage or energy sat at its cap (more income would be lost).
    resource_capped: f64,
    /// How often the passive fired in the run (`passive_fires_label` says what is counted).
    passive_fires: f64,
    /// Enemies the main spell struck per cast (direct hits, not ticks); 0 with no cast.
    main_hits_per_cast: f64,
    /// The hero's damage stat (1 = a naked level-1 hero's).
    damage_stat: f64,
}

/// What `passive_fires` counts for option `o`.
fn passive_fires_label(o: &StartingOption) -> &'static str {
    match &o.passive.kind {
        PassiveKind::Counter(c) => match c.release {
            Release::Spell(_) => "releases",
            Release::Summon(_) => "summons",
        },
        PassiveKind::Orbs { .. } => "orbs taken",
        PassiveKind::Marks { .. } => "discharges",
        PassiveKind::Empower { .. } => "full stacks",
        PassiveKind::FreeCasts { .. } | PassiveKind::SwingSpeedStacks { .. } | PassiveKind::SwingSpeedOnHit { .. } | PassiveKind::PetFrenzy { .. } => {
            "procs"
        }
        PassiveKind::None if o.spells.len() > 1 => "finishers",
        PassiveKind::None if o.main_spell() == "holy_shield" => "blocks",
        PassiveKind::BlockStrikes { .. } => "blocks",
        PassiveKind::None => "",
    }
}

/// Plays option `start` of `class` with `loadout` for `STARTS_SECONDS` in `zone` with its level-1
/// spell alone: no experience, gold or caches, so it never learns another spell.
fn measure_start(content: &Arc<Content>, zone: &str, class: &str, start: &str, loadout: &Loadout, seed: u64, arena: StartArena) -> StartSample {
    let mut setup = hero_setup(content, class, loadout);
    setup.start = Some(start.into());
    let mut run = Run::new(content.clone(), zone, RunOptions::default(), &[setup], Box::new(Flat), seed).expect("a run");
    let (hero, p) = (run.heroes[0].id, run.heroes[0].pos);
    run.hero_moved(hero, (p.0 + 1.0, p.1), 0.0, 0.0);
    let option = content.starting_option(start).expect("an option");
    // past level 1: the hidden level-1 targeting is off (no level-up: its stats stay level 1's)
    run.heroes[0].progress.level = 2;
    let mut target = None;
    if arena == StartArena::OneEnemy {
        run.enemies.clear();
        let at = (run.heroes[0].pos.0 + 5.0, run.heroes[0].pos.1);
        run.spawn_roster_enemy(0, Some(at));
        let e = run.enemies.last_mut().expect("the enemy");
        (e.health, e.max_health, e.base_speed, e.damage) = (1e12, 1e12, 0.0, 0.0);
        target = Some(e.id);
    }
    let kite = if let StartArena::Swarm(k) = arena { k } else { None };
    // what the passive brings (its spell, its summons' spells) counts as the passive's
    let passive: Vec<&str> = option.passive_spells();
    // the option's own pet strikes with these (Kill Command's)
    let pet_spells: Vec<&str> = option.pet.iter().flat_map(|p| p.spells.iter().map(String::as_str)).collect();
    let own: Vec<&str> = content.starting_spells(option).into_iter().filter(|s| !passive.contains(s)).collect();
    let main = option.main_spell();
    let finisher = option.spells.get(1).map(String::as_str);
    let counts_finishers = passive_fires_label(option) == "finishers";
    let costs: HashMap<&str, f32> = option.spells.iter().filter_map(|s| content.ability(s).map(|a| (s.as_str(), a.kit.cost))).collect();
    let cap = option.resource.map(|r| r.cap());
    let mut s = StartSample { damage_stat: f64::from(run.heroes[0].stats.damage), ..Default::default() };
    let (mut cheat_deaths, mut main_hits, mut main_casts, mut capped_ticks, mut ticks) = (0u32, 0u32, 0u32, 0u32, 0u32);
    let resource_at_start = run.heroes[0].kit.resource;
    let mut spent = 0.0f64;
    let mut heading = (1.0, 0.0);
    while run.now < STARTS_SECONDS && run.phase == Phase::Running || run.phase == Phase::Choosing {
        while let Some(screen) = &run.screen {
            let id = run.heroes[screen.hero].id;
            run.choose(id, 0);
        }
        // no experience, gold or caches: the hero keeps its level-1 spell alone
        run.pickups.clear();
        if let Some(t) = target {
            run.enemies.retain(|e| e.id == t);
        }
        if let Some(stand_off) = kite {
            kite_step(&mut run, stand_off, &mut heading, DT);
        }
        run.tick(DT);
        ticks += 1;
        if cap.is_some_and(|c| run.heroes[0].kit.resource >= c - 1e-3) {
            capped_ticks += 1;
        }
        let mut discharged = false;
        for e in run.drain_events() {
            match e {
                Event::EnemyHit { amount, by: Some(_), source, .. } => {
                    let amount = f64::from(amount);
                    match &source {
                        HitSource::Spell(x) | HitSource::Tick(x) if passive.contains(&x.as_str()) => {
                            s.passive += amount;
                            discharged |= matches!(option.passive.kind, PassiveKind::Marks { .. });
                        }
                        HitSource::Spell(x) | HitSource::Tick(x) if own.contains(&x.as_str()) => {
                            s.spell += amount;
                            if pet_spells.contains(&x.as_str()) {
                                s.pet += amount;
                            }
                            if Some(x.as_str()) == finisher {
                                s.finisher += amount;
                            }
                        }
                        HitSource::WeaponSwing => s.swing += amount,
                        HitSource::Pet(_) if option.pet.is_some() => {
                            s.spell += amount;
                            s.pet += amount;
                        }
                        _ => s.other += amount,
                    }
                    if matches!(&source, HitSource::Spell(x) if x == main) {
                        main_hits += 1;
                    }
                }
                Event::SpellCast { spell, .. } if spell == main => main_casts += 1,
                // one a paid cast (a double cast has none)
                Event::SpellCooldown { spell, .. } => {
                    spent += f64::from(costs.get(spell.as_str()).copied().unwrap_or(0.0));
                    if counts_finishers && Some(spell.as_str()) == finisher {
                        s.passive_fires += 1.0;
                    }
                }
                Event::CheatedDeath { .. } => cheat_deaths += 1,
                Event::PassiveTriggered { .. } => s.passive_fires += 1.0,
                Event::StacksChanged { kind: StackKind::Zeal | StackKind::QuickShots | StackKind::Frenzy, until: Some(_), .. } => s.passive_fires += 1.0,
                Event::Blocked { .. } if main == "holy_shield" => s.passive_fires += 1.0,
                _ => {}
            }
        }
        if discharged {
            s.passive_fires += 1.0;
        }
    }
    let t = f64::from(run.now.max(1.0));
    for v in [&mut s.spell, &mut s.finisher, &mut s.pet, &mut s.passive, &mut s.swing, &mut s.other] {
        *v /= t;
    }
    s.cheat_deaths_per_minute = f64::from(cheat_deaths) / (t / 60.0);
    if cap.is_some() {
        s.resource_income = (spent + f64::from(run.heroes[0].kit.resource - resource_at_start)) / t;
        s.resource_capped = f64::from(capped_ticks) / f64::from(ticks.max(1));
    }
    s.main_hits_per_cast = if main_casts > 0 { f64::from(main_hits) / f64::from(main_casts) } else { 0.0 };
    s
}

/// `items` mapped by `f` on `threads` threads, in order.
fn parallel_map<T: Sync, R: Send>(items: &[T], threads: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let results: Vec<Mutex<Option<R>>> = items.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(item) = items.get(k) else { break };
                *results[k].lock().unwrap() = Some(f(item));
            });
        }
    });
    results.into_iter().map(|r| r.into_inner().unwrap().expect("every item mapped")).collect()
}

/// One `--starts` run.
#[derive(Clone, Copy, PartialEq)]
enum StartJob {
    /// The level-1 spell alone; `quiet`: with every passive turned off.
    Alone { arena: StartArena, quiet: bool },
    /// A whole run as the main check plays it (first offers), the hero standing or kiting.
    Whole { kite: Option<f32> },
}

/// What one `--starts` run gave back.
enum StartOutcome {
    Alone(StartSample),
    Whole(Option<RunResult>),
}

/// The mean of `field` over `samples` (0 with none).
fn mean_of(samples: &[&StartSample], field: impl Fn(&StartSample) -> f64) -> f64 {
    mean(samples.iter().map(|s| field(s))).unwrap_or(0.0)
}

/// Target kill seconds by (zone id, minute), if a file was given.
type Targets = Option<HashMap<(String, usize), f64>>;

/// `--starts`: every starting option, in each zone of `--zones` with that zone's expected hero
/// (`arrival_points`, the previous zone's set). See the doc comment at the top.
fn starts_check(content: &Arc<Content>, o: &Options) {
    let zones: Vec<usize> = o
        .start_zones
        .iter()
        .map(|id| content.zones.iter().position(|z| z.id == *id).unwrap_or_else(|| panic!("--zones: no zone {id}")))
        .collect();
    let movement = if o.kite { "kiting" } else { "standing" };
    let trees = if o.own_tree { "its option's own talent tree first" } else { "talents in the first tree first" };
    println!(
        "Starting options: {} seeds each, the hero {movement}, {trees}: its level-1 spell alone for {} s, then a whole run taking first offers.\n",
        o.seeds, STARTS_SECONDS
    );
    let points = arrival_points(content, o.seeds, o.threads);
    // the same content with every passive turned off
    let mut quiet = (**content).clone();
    for opt in quiet.classes.iter_mut().flat_map(|c| c.starting_options.iter_mut()) {
        opt.passive.kind = PassiveKind::None;
    }
    let quiet = Arc::new(quiet);
    let options: Vec<(String, String)> =
        content.classes.iter().flat_map(|c| c.starting_options.iter().map(move |s| (c.id.clone(), s.id.clone()))).collect();
    let stand_off = |start: &str| o.kite.then(|| kite_stand_off(content, content.starting_option(start).expect("an option")));
    // (zone, option, seed, run)
    let mut jobs = Vec::new();
    for &zi in &zones {
        for (oi, (_, start)) in options.iter().enumerate() {
            let kite = stand_off(start);
            let mut kinds = vec![
                StartJob::Alone { arena: StartArena::Swarm(kite), quiet: true },
                StartJob::Alone { arena: StartArena::Swarm(kite), quiet: false },
                StartJob::Alone { arena: StartArena::OneEnemy, quiet: false },
                StartJob::Whole { kite },
            ];
            if kite.is_some() {
                kinds.push(StartJob::Alone { arena: StartArena::Swarm(None), quiet: false });
            }
            for s in 0..o.seeds {
                for &kind in &kinds {
                    jobs.push((zi, oi, s, kind));
                }
            }
        }
    }
    let outcomes = parallel_map(&jobs, o.threads, |&(zi, oi, s, kind)| {
        let (class, start) = &options[oi];
        let zone = &content.zones[zi].id;
        let ranks = if o.own_tree {
            let own = option_tree(content, class, start);
            let order: Vec<usize> = std::iter::once(own).chain((0..3).filter(|&t| t != own)).collect();
            spend_points_in_order(content, class, points[zi], &order, &[])
        } else {
            spend_talent_points(content, class, points[zi])
        };
        let loadout = Loadout { ranks, set_from: zi.checked_sub(1), without_milestone: None, without_whole_talent: false, weapons: None, start: None };
        let seed = zone_seed(zone) + ((oi as u64) << 32) + s;
        match kind {
            StartJob::Alone { arena, quiet: q } => StartOutcome::Alone(measure_start(if q { &quiet } else { content }, zone, class, start, &loadout, seed, arena)),
            StartJob::Whole { kite } => {
                let mut setup = hero_setup(content, class, &loadout);
                setup.start = Some(start.clone());
                let r = std::panic::catch_unwind(AssertUnwindSafe(|| play_moving(content, zone, setup, seed, kite)));
                if r.is_err() {
                    eprintln!("balance check: the whole run of {zone}/{start} seed {seed} panicked; it is left out");
                }
                StartOutcome::Whole(r.ok())
            }
        }
    });
    let targets: Targets = o.targets.as_deref().map(read_targets);
    let second: Targets = o.second_targets.as_deref().map(read_targets);
    let mut csv = String::from(
        "zone,points,class,option,stand_off,damage_stat,spell_alone,swing_alone,with_passive,passive_adds,passive_own,swing,other,one_enemy,\
standing_with_passive,main_hits_per_cast,cheat_deaths_per_minute,standing_cheat_deaths_per_minute,resource_income,resource_capped,\
passive_fires_per_run,passive_fires_what,whole_dps_m5,whole_dps_m10,whole_dps_m15,ratio_m5,ratio_m10,ratio_m15,\
second_ratio_m5,second_ratio_m10,second_ratio_m15,whole_cheat_deaths_per_minute\n",
    );
    for &zi in &zones {
        let zone = &content.zones[zi];
        let worn = zi.checked_sub(1).map_or("no".to_string(), |z| format!("{}'s", content.zones[z].id));
        println!("## {} ({} talent points, {worn} gear)", zone.id, points[zi]);
        println!(
            "| class | option | dmg stat | alone | with passive | passive adds | one enemy | standing | hits/cast | deaths/min | standing deaths/min | income/s | capped | fires/run | m5 | m10 | m15 | whole deaths/min |"
        );
        println!("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
        for (oi, (class, start)) in options.iter().enumerate() {
            let option = content.starting_option(start).expect("an option");
            let kite = stand_off(start);
            let mine: Vec<(&StartJob, &StartOutcome)> =
                jobs.iter().zip(&outcomes).filter(|((z, i, _, _), _)| *z == zi && *i == oi).map(|((_, _, _, k), r)| (k, r)).collect();
            let samples = |want: StartJob| -> Vec<&StartSample> {
                mine.iter().filter(|(k, _)| **k == want).filter_map(|(_, r)| if let StartOutcome::Alone(s) = r { Some(s) } else { None }).collect()
            };
            let alone = samples(StartJob::Alone { arena: StartArena::Swarm(kite), quiet: true });
            let with = samples(StartJob::Alone { arena: StartArena::Swarm(kite), quiet: false });
            let one = samples(StartJob::Alone { arena: StartArena::OneEnemy, quiet: false });
            let standing = if kite.is_some() { samples(StartJob::Alone { arena: StartArena::Swarm(None), quiet: false }) } else { with.clone() };
            let whole: Vec<&RunResult> =
                mine.iter().filter_map(|(k, r)| if let (StartJob::Whole { .. }, StartOutcome::Whole(Some(r))) = (k, r) { Some(r) } else { None }).collect();
            let alone_spell = mean_of(&alone, |s| s.spell);
            let alone_swing = mean_of(&alone, |s| s.swing);
            let with_spell = mean_of(&with, |s| s.spell + s.passive);
            let adds = mean_of(&with, |s| s.spell + s.passive + s.swing) - mean_of(&alone, |s| s.spell + s.passive + s.swing);
            let one_dps = mean_of(&one, |s| s.spell + s.passive);
            let standing_dps = mean_of(&standing, |s| s.spell + s.passive);
            let minute = |m: usize| minute_means(&whole, m - 1);
            let ratio = |t: &Targets, m: usize| Some(minute(m)?.kill_seconds / t.as_ref()?.get(&(zone.id.clone(), m))?);
            let cell = |v: Option<f64>| v.map_or("—".to_string(), |v| format!("{v:.2}"));
            let opt = |v: Option<f64>, digits: usize| v.map_or(String::new(), |v| format!("{v:.digits$}"));
            let whole_deaths = mean(whole.iter().map(|r| {
                let minutes: Vec<&MinuteSample> = r.minutes.iter().flatten().collect();
                minutes.iter().map(|s| f64::from(s.cheat_deaths)).sum::<f64>() / minutes.len().max(1) as f64
            }));
            let label = passive_fires_label(option);
            let resource = option.resource.is_some();
            println!(
                "| {class} | {start} | {:.2} | {alone_spell:.1} | {with_spell:.1} | {adds:+.1} | {one_dps:.1} | {standing_dps:.1} | {:.2} | {:.1} | {:.1} | {} | {} | {} | {} | {} | {} | {} |",
                mean_of(&with, |s| s.damage_stat),
                mean_of(&with, |s| s.main_hits_per_cast),
                mean_of(&with, |s| s.cheat_deaths_per_minute),
                mean_of(&standing, |s| s.cheat_deaths_per_minute),
                if resource { format!("{:.1}", mean_of(&with, |s| s.resource_income)) } else { "—".into() },
                if resource { format!("{:.0} %", mean_of(&with, |s| s.resource_capped) * 100.0) } else { "—".into() },
                if label.is_empty() { "—".into() } else { format!("{:.1} {label}", mean_of(&with, |s| s.passive_fires)) },
                cell(ratio(&targets, 5)),
                cell(ratio(&targets, 10)),
                cell(ratio(&targets, 15)),
                cell(whole_deaths),
            );
            let [m5, m10, m15] = STARTS_TARGET_MINUTES.map(|m| opt(minute(m).map(|x| x.dps), 1));
            let [r5, r10, r15] = STARTS_TARGET_MINUTES.map(|m| opt(ratio(&targets, m), 3));
            let [q5, q10, q15] = STARTS_TARGET_MINUTES.map(|m| opt(ratio(&second, m), 3));
            csv += &format!(
                "{},{},{class},{start},{},{:.3},{alone_spell:.2},{alone_swing:.2},{with_spell:.2},{adds:.2},{:.2},{:.2},{:.2},{one_dps:.2},{standing_dps:.2},{:.3},{:.3},{:.3},{:.2},{:.3},{:.2},{label},{m5},{m10},{m15},{r5},{r10},{r15},{q5},{q10},{q15},{}\n",
                zone.id,
                points[zi],
                kite.map_or(String::new(), |k| format!("{k}")),
                mean_of(&with, |s| s.damage_stat),
                mean_of(&with, |s| s.passive),
                mean_of(&with, |s| s.swing),
                mean_of(&with, |s| s.other),
                mean_of(&with, |s| s.main_hits_per_cast),
                mean_of(&with, |s| s.cheat_deaths_per_minute),
                mean_of(&standing, |s| s.cheat_deaths_per_minute),
                mean_of(&with, |s| s.resource_income),
                mean_of(&with, |s| s.resource_capped),
                mean_of(&with, |s| s.passive_fires),
                opt(whole_deaths, 3),
            );
        }
        println!();
    }
    if let Some(path) = &o.csv {
        fs::write(path, &csv).unwrap_or_else(|e| panic!("the csv file {path} is written: {e}"));
        println!("Wrote {path}.");
    }
}

// ---- --legendaries: each legendary amulet against its budget ---------------------------------

/// Our budget: a heal, shield or defensive legendary doubles what its spell heals,
/// absorbs or keeps up (+100 %); a gain passes within this share of it (±20 %).
const LEGENDARY_BUDGET: f64 = 1.0;
/// A damage legendary's budget: +30 % to the hero's total damage.
const LEGENDARY_DAMAGE_BUDGET: f64 = 0.3;
const LEGENDARY_TOLERANCE: f64 = 0.2;
/// `--legendaries` measures in the first zone and in this one.
const LEGENDARY_LATE_ZONE: &str = "aq";

/// What a legendary's spell is for, and so which gain its budget holds: its own damage, the
/// healing and absorbing it does, or the time its buff is up.
#[derive(Clone, Copy, PartialEq, Debug)]
enum LegendaryRole {
    Damage,
    Healing,
    /// A defensive buff (immunity): measured by its uptime.
    Buff,
    /// A buff that adds power (damage, haste, speed, rage or energy): measured by total damage.
    Power,
}

/// What a legendary's measure counts as its spells' own.
struct OwnParts {
    /// Spell ids whose hits and ticks are the spells' (they, what they cast, poison or summon).
    spells: Vec<String>,
    /// `Spell.dbc` ids whose procs, auras and lasting areas are the spells' (their looks, the
    /// looks the legendary's milestones name).
    looks: Vec<u32>,
    /// Pets the legendary calls (by name).
    pets: Vec<String>,
    /// The main spell's look: its heals and its buff.
    main_look: u32,
}

/// What one `--legendaries` run measured.
#[derive(Clone, Copy, Default)]
struct LegendarySample {
    total: f64,
    own: f64,
    /// Healing given (heals and heals over time of the spell) and damage shields absorbed.
    healing: f64,
    /// Share of the run the spell's buff was up.
    uptime: f64,
    /// Damage struck at the hero (before shields took any).
    taken: f64,
    /// The legendary was worn (its first cache was opened).
    worn: bool,
}

/// The ids, looks and pets a legendary's measure counts as its spells' own.
fn own_parts(content: &Content, l: &esurv_content::LegendaryAmulet) -> OwnParts {
    let mut spells: Vec<String> = l.spells.clone();
    for s in &l.spells {
        match content.ability(s).and_then(|a| a.kit.mechanic.as_ref()) {
            Some(esurv_content::Mechanic::Poisons { poison }) => spells.push(poison.clone()),
            Some(esurv_content::Mechanic::Summons(spec)) => spells.extend(spec.spells.iter().cloned()),
            _ => {}
        }
    }
    for e in &l.effects {
        spells.extend(e.spells().into_iter().filter(|s| !s.is_empty() && content.ability(s).is_some()).cloned());
    }
    let mut looks: Vec<u32> = spells.iter().filter_map(|s| content.ability(s)).flat_map(|a| [a.look_spell].into_iter().chain(a.pulse_spell)).filter(|&x| x != 0).collect();
    let mut pets = Vec::new();
    fn walk(m: &Milestone, looks: &mut Vec<u32>, pets: &mut Vec<String>) {
        let debug = format!("{m:?}");
        for part in debug.split("look_spell: ").skip(1) {
            // `Some(1234)` or a bare number (DotOnHit)
            let digits: String = part.trim_start_matches("Some(").chars().take_while(char::is_ascii_digit).collect();
            if let Ok(n) = digits.parse() {
                looks.push(n);
            }
        }
        if let Some(p) = m.pet() {
            pets.push(p.clone());
        }
        if let Milestone::Every { then, .. } | Milestone::WhileBuff { then, .. } = m {
            for inner in then {
                walk(inner, looks, pets);
            }
        }
    }
    for e in &l.effects {
        if let TalentEffect::Milestone(m) = e {
            walk(m, &mut looks, &mut pets);
        }
    }
    looks.sort_unstable();
    looks.dedup();
    let main_look = content.ability(l.main_spell()).map_or(0, |a| a.look_spell);
    OwnParts { spells, looks, pets, main_look }
}

/// The role of a legendary's main spell: a spell that deals damage
/// (a hybrid such as Holy Nova or Ghostly Strike too: its heal or shield is the smaller part) and
/// a buff that adds power (damage, haste, speed, rage or energy) are measured by the hero's total
/// damage, +30 %; a heal by what it heals (and what shields absorb), and a defensive buff
/// (immunity) by its uptime, +100 %. A shield is measured as power too: kiting, the hero's own
/// shield already takes most of the damage struck at it (Power Word: Shield about 80 %, Ice
/// Barrier about 85 %), so what a shield absorbs cannot double whatever its legendary does.
fn legendary_role(content: &Content, l: &esurv_content::LegendaryAmulet) -> LegendaryRole {
    let Some(a) = content.ability(l.main_spell()) else { return LegendaryRole::Damage };
    // a heal whose legendary turns its healing into damage (Renew's Deplete) is a damage legendary
    let heal_to_damage = l.effects.iter().any(|e| matches!(e, TalentEffect::Milestone(Milestone::DotFromHeal { .. })));
    if heal_to_damage {
        return LegendaryRole::Damage;
    }
    match a.effect {
        _ if a.damage > 0.0 || a.kit.mechanic.is_some() => LegendaryRole::Damage,
        esurv_content::SpellEffect::Heal { .. } => LegendaryRole::Healing,
        esurv_content::SpellEffect::Shield { .. } => LegendaryRole::Power,
        esurv_content::SpellEffect::Thorns { .. } => LegendaryRole::Damage,
        esurv_content::SpellEffect::Immunity { .. } => LegendaryRole::Buff,
        _ => LegendaryRole::Power,
    }
}

/// The gain a legendary of `role` is held to: +30 % total damage for damage
/// and power, +100 % healing and absorbing (or uptime) for heals, shields and defensive buffs.
fn legendary_budget(role: LegendaryRole) -> f64 {
    match role {
        LegendaryRole::Damage | LegendaryRole::Power => LEGENDARY_DAMAGE_BUDGET,
        LegendaryRole::Healing | LegendaryRole::Buff => LEGENDARY_BUDGET,
    }
}

/// The class and starting option that measure legendary `l`: for a level-1 spell its option, for
/// a card the class's first option that can draw it.
fn legendary_hero(content: &Content, l: &esurv_content::LegendaryAmulet) -> Option<(String, String)> {
    let main = l.main_spell();
    content.classes.iter().find_map(|c| {
        let o = c
            .starting_options
            .iter()
            .find(|o| o.spells.iter().any(|s| s == main))
            .or_else(|| c.starting_options.iter().find(|o| c.card_pool(Some(&o.id)).iter().any(|s| s == main)))?;
        Some((c.id.clone(), o.id.clone()))
    })
}

/// Plays one `--legendaries` run of `setup` in `zone`: with `legendary`, the first cache opened
/// offers it and the hero takes it (and keeps it: other amulets are declined); without, that
/// cache's first offer that is no amulet is taken. Later screens: the first offer, the first that
/// is no amulet with `no_amulets`.
#[allow(clippy::too_many_arguments)]
fn play_legendary(content: &Arc<Content>, zone: &str, setup: HeroSetup, seed: u64, kite: Option<f32>, no_amulets: bool, legendary: Option<&esurv_content::Amulet>, own: &OwnParts, buff_spell: &str) -> LegendarySample {
    let mut run = Run::new(content.clone(), zone, RunOptions::default(), &[setup], Box::new(Flat), seed).expect("a run");
    let (hero, p) = (run.heroes[0].id, run.heroes[0].pos);
    run.hero_moved(hero, (p.0 + 1.0, p.1), 0.0, 0.0);
    run.heroes[0].first_cache_amulet = legendary.cloned();
    let mut s = LegendarySample::default();
    let mut first_cache = true;
    let mut heading = (1.0, 0.0);
    let (mut up_ticks, mut ticks) = (0u32, 0u32);
    let buff_look = content.ability(buff_spell).map_or(0, |a| a.look_spell);
    while run.now < MINUTES as f32 * 60.0 && run.phase != Phase::Victory {
        while let Some(screen) = &run.screen {
            let id = run.heroes[screen.hero].id;
            let is_amulet = |x: &Offer| matches!(x, Offer::Amulet { .. } | Offer::Empower);
            let pick = if screen.kind == esurv_sim::game::choices::ScreenKind::Cache && first_cache {
                first_cache = false;
                match legendary {
                    Some(l) => screen.offers.iter().position(|x| matches!(x, Offer::Amulet { amulet } if amulet.id == l.id)).unwrap_or(0),
                    None => screen.offers.iter().position(|x| !is_amulet(x)).unwrap_or(0),
                }
            } else if no_amulets {
                screen.offers.iter().position(|x| !is_amulet(x)).unwrap_or(0)
            } else if legendary.is_some() {
                // the worn legendary stays: no other amulet replaces it (it may be empowered)
                screen.offers.iter().position(|x| !matches!(x, Offer::Amulet { .. })).unwrap_or(0)
            } else {
                0
            };
            run.choose(id, pick);
        }
        if let Some(stand_off) = kite {
            kite_step(&mut run, stand_off, &mut heading, DT);
        }
        run.tick(DT);
        ticks += 1;
        let h = &run.heroes[0];
        let now = run.now;
        if h.buffs.iter().any(|b| b.visual == buff_look && b.until > now) {
            up_ticks += 1;
        }
        // heals over time of the spell, as they are given
        s.healing += h.buffs.iter().filter(|b| b.kind == esurv_sim::game::run::BuffKind::Regen && b.visual == own.main_look && b.until > now).map(|b| f64::from(b.value * DT)).sum::<f64>();
        for e in run.drain_events() {
            match e {
                Event::EnemyHit { amount, by: Some(_), source, .. } => {
                    let amount = f64::from(amount);
                    s.total += amount;
                    let mine = match &source {
                        HitSource::Spell(x) | HitSource::Tick(x) => {
                            own.spells.contains(x) || x.strip_prefix("aura:").and_then(|n| n.parse::<u32>().ok()).is_some_and(|n| own.looks.contains(&n))
                        }
                        HitSource::Proc { spell, .. } => own.looks.contains(spell),
                        HitSource::Pet(id) => run.pets.iter().find(|p| p.id == *id).is_some_and(|p| own.pets.contains(&p.pet)),
                        _ => false,
                    };
                    if mine {
                        s.own += amount;
                    }
                }
                Event::HeroHealed { amount, spell, .. } if spell != 0 && spell == own.main_look => s.healing += f64::from(amount),
                Event::HeroHit { amount, lost, .. } => {
                    s.healing += f64::from((amount - lost).max(0.0));
                    s.taken += f64::from(amount);
                }
                _ => {}
            }
        }
        if legendary.is_some() && run.heroes[0].amulet.as_ref().is_some_and(|a| Some(&a.amulet.id) == legendary.map(|l| &l.id)) {
            s.worn = true;
        }
    }
    let t = f64::from(run.now.max(1.0));
    s.total /= t;
    s.own /= t;
    s.healing /= t;
    s.taken /= t;
    s.uptime = f64::from(up_ticks) / f64::from(ticks.max(1));
    s
}

/// `--legendaries <ids|all>`: every legendary amulet named (by its spell) played in the first zone
/// and in Ahn'Qiraj with the arrival hero of each (the previous zone's points, spent in the
/// option's own tree first, and set), with it worn from the first cache on and without it, the
/// same seeds; its spells' own damage (or healing and absorbing, or buff uptime) gain against
/// the budget, +100 %.
fn legendary_check(content: &Arc<Content>, o: &Options) {
    let wanted: Vec<&esurv_content::LegendaryAmulet> = content
        .legendary_amulets
        .iter()
        .filter(|l| o.legendaries.iter().any(|w| w == "all" || w.strip_prefix("legendary:").unwrap_or(w) == l.main_spell() || l.spells.contains(w)))
        .collect();
    if wanted.is_empty() {
        println!("--legendaries: no legendary amulet matches {:?}", o.legendaries);
        return;
    }
    let late = content.zones.iter().position(|z| z.id == LEGENDARY_LATE_ZONE);
    // the arrival points are measured only when a later zone needs them
    let points = match late {
        Some(_) if !o.no_talents => arrival_points(content, o.seeds, o.threads_given.unwrap_or(o.threads)),
        _ => vec![0; content.zones.len()],
    };
    let zones: Vec<usize> = std::iter::once(0).chain(late.filter(|&z| z > 0)).collect();
    let seeds = o.check_seeds_given.unwrap_or(100);
    // (legendary, zone, with it, seed)
    let mut jobs = Vec::new();
    for (li, _) in wanted.iter().enumerate() {
        for &z in &zones {
            for with in [true, false] {
                for s in 0..seeds {
                    jobs.push((li, z, with, s));
                }
            }
        }
    }
    let heroes: Vec<Option<(String, String)>> = wanted.iter().map(|l| legendary_hero(content, l)).collect();
    let parts: Vec<OwnParts> = wanted.iter().map(|l| own_parts(content, l)).collect();
    let amulets: Vec<esurv_content::Amulet> = wanted.iter().map(|l| esurv_sim::game::amulets::legendary_for(l)).collect();
    let threads = o.threads_given.unwrap_or_else(|| std::thread::available_parallelism().map_or(2, |n| n.get().saturating_sub(2)).max(1));
    let results = parallel_map(&jobs, threads, |&(li, z, with, s)| {
        let (class, start) = heroes[li].clone()?;
        let order: Vec<usize> = {
            let t = option_tree(content, &class, &start);
            std::iter::once(t).chain((0..3).filter(|&x| x != t)).collect()
        };
        let ranks = spend_points_in_order(content, &class, points[z], &order, &[]);
        let loadout = Loadout { ranks, set_from: z.checked_sub(1), without_milestone: None, without_whole_talent: false, weapons: None, start: Some(start.clone()) };
        let mut setup = hero_setup(content, &class, &loadout);
        let main = wanted[li].main_spell().to_string();
        let option = content.starting_option(&start)?;
        if !option.spells.contains(&main) {
            // a card spell: known from the start, at rank 1
            setup.milestones.push(Milestone::StartWith { spell: main.clone() });
        }
        let kite = o.kite.then(|| kite_stand_off(content, option));
        let ci = content.classes.iter().position(|c| c.id == class).unwrap_or(0);
        let seed = zone_seed(&content.zones[z].id) + ((ci as u64) << 40) + s;
        let zone = content.zones[z].id.clone();
        let legendary = with.then_some(&amulets[li]);
        std::panic::catch_unwind(AssertUnwindSafe(|| play_legendary(content, &zone, setup, seed, kite, o.no_amulets, legendary, &parts[li], &main))).ok()
    });
    println!(
        "Legendaries: {} seeds a case, {} threads{}{}; the legendary from the first cache on against the same hero without it (a second):\n",
        seeds,
        threads,
        if o.kite { ", kiting" } else { "" },
        if o.no_amulets { ", other amulets declined" } else { "" }
    );
    println!(
        "{:22} {:8} {:16} {:9} {:>9} {:>9} {:>8} {:>8} {:>9} {:>8} {:>7}  role     heal with/without, taken, total without (a second)",
        "legendary", "class", "option", "zone", "own with", "without", "own +%", "total +%", "heal/up +%", "worn", "budget"
    );
    for (li, l) in wanted.iter().enumerate() {
        let role = legendary_role(content, l);
        let Some((class, start)) = &heroes[li] else {
            println!("{:22} no hero can know it", l.main_spell());
            continue;
        };
        for &z in &zones {
            let arm = |with: bool| -> Vec<LegendarySample> {
                jobs.iter().zip(&results).filter(|((j, jz, w, _), _)| *j == li && *jz == z && *w == with).filter_map(|(_, r)| *r).collect()
            };
            let (with, without) = (arm(true), arm(false));
            let m = |v: &[LegendarySample], f: fn(&LegendarySample) -> f64| mean_and_error(v.iter().map(f));
            let gain = |f: fn(&LegendarySample) -> f64| match (m(&with, f), m(&without, f)) {
                (Some(a), Some(b)) if b.0 > 1e-9 => Some(gain_and_error(a, b)),
                (Some(a), Some(_)) if a.0 > 1e-9 => Some((f64::INFINITY, 0.0)),
                _ => None,
            };
            let own = gain(|s| s.own);
            let total = gain(|s| s.total);
            let side = match role {
                LegendaryRole::Healing => gain(|s| s.healing),
                _ => gain(|s| s.uptime),
            };
            let measured = match role {
                LegendaryRole::Damage | LegendaryRole::Power => total,
                LegendaryRole::Healing => side,
                LegendaryRole::Buff => side.or(total),
            };
            let budget = legendary_budget(role);
            let flag = match measured {
                Some((g, _)) if (g - budget).abs() <= budget * LEGENDARY_TOLERANCE => " ",
                _ => "!",
            };
            let worn = with.iter().filter(|s| s.worn).count();
            let cell = |g: Option<(f64, f64)>| g.map_or("—".to_string(), |(g, _)| if g.is_finite() { format!("{:+.0}", g * 100.0) } else { "new".to_string() });
            println!(
                "{:22} {:8} {:16} {:9} {:>9.1} {:>9.1} {:>8} {:>8} {:>9} {:>4}/{:<3} {:>+6.0}%{}  {:8} {:.1}/{:.1}, {:.1}, {:.0}",
                l.main_spell(),
                class,
                start,
                content.zones[z].id,
                m(&with, |s| s.own).map_or(0.0, |x| x.0),
                m(&without, |s| s.own).map_or(0.0, |x| x.0),
                cell(own),
                cell(total),
                cell(side),
                worn,
                with.len(),
                budget * 100.0,
                flag,
                format!("{role:?}"),
                m(&with, |s| s.healing).map_or(0.0, |x| x.0),
                m(&without, |s| s.healing).map_or(0.0, |x| x.0),
                m(&without, |s| s.taken).map_or(0.0, |x| x.0),
                m(&without, |s| s.total).map_or(0.0, |x| x.0)
            );
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The zone mode plays each class's first option and loadout `--seeds` times on the seeds it
    /// always had; `--start all --loadout all` plays every pair of option and loadout on those
    /// same seeds, amulets declined with `--no-amulets`; the hero fights with its loadout.
    #[test]
    fn the_zone_mode_plays_every_starting_option_and_loadout_pair() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).expect("content loads");
        let mut o = Options { seeds: 3, ..Options::default() };
        let plain = zone_mode_jobs(&content, 1, &o, 5);
        assert_eq!(plain.len(), content.classes.len() * 3);
        assert!(plain.iter().all(|j| j.loadout.start.is_none() && j.loadout.weapons.is_none() && !j.no_amulets));
        assert_eq!(plain[1].seed, zone_seed(&content.zones[1].id) + 1);
        (o.start_all, o.loadout_all, o.no_amulets) = (true, true, true);
        let all = zone_mode_jobs(&content, 1, &o, 5);
        let pairs: usize = content.classes.iter().map(|c| c.starting_options.len() * content.loadouts_of(&c.id).len()).sum();
        assert_eq!(all.len(), pairs * 3);
        assert!(all.iter().all(|j| j.no_amulets && j.loadout.set_from == Some(0)));
        let rogue = content.classes.iter().position(|c| c.id == "rogue").unwrap();
        let swords: Vec<&Job> =
            all.iter().filter(|j| j.class == rogue && j.loadout.weapons.as_deref() == Some("swords") && j.loadout.start.as_deref() == Some("mutilate")).collect();
        assert_eq!(swords.iter().map(|j| j.seed).collect::<Vec<_>>(), plain.iter().filter(|j| j.class == rogue).map(|j| j.seed).collect::<Vec<_>>());
        let setup = hero_setup(&content, "rogue", &swords[0].loadout);
        assert_eq!(setup.loadout.as_deref(), Some("swords"));
        // one loadout by id, where the class has it
        let one = Options { loadout: Some("shield".into()), ..Options::default() };
        let jobs = zone_mode_jobs(&content, 0, &one, 0);
        let of = |class: &str| jobs.iter().find(|j| content.classes[j.class].id == class).unwrap().loadout.weapons.clone();
        assert_eq!((of("warrior"), of("mage")), (Some("shield".into()), None));
    }

    /// The legendary measure's hero wears the legendary its first cache was sure to offer, and
    /// its spell's own damage is counted; the baseline wears none.
    #[test]
    fn the_first_cache_gives_the_legendary_and_its_damage_is_counted() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Arc::new(Content::load(dir).expect("content loads"));
        let l = content.legendary_amulets.iter().find(|l| l.main_spell() == "fireball").unwrap();
        let (class, start) = legendary_hero(&content, l).unwrap();
        assert_eq!((class.as_str(), start.as_str()), ("mage", "fireball"));
        let card = content.legendary_amulets.iter().find(|l| l.main_spell() == "cleave").unwrap();
        assert_eq!(legendary_hero(&content, card).unwrap(), ("warrior".to_string(), "slam".to_string()), "a card: the class's first option");
        let own = own_parts(&content, l);
        assert!(own.pets.contains(&"Phoenix".to_string()) && own.spells.contains(&"fireball".to_string()));
        let amulet = esurv_sim::game::amulets::legendary_for(l);
        let loadout = Loadout { ranks: Ranks::new(), set_from: None, without_milestone: None, without_whole_talent: false, weapons: None, start: Some(start.clone()) };
        let option = content.starting_option(&start).unwrap();
        let kite = Some(kite_stand_off(&content, option));
        let with = play_legendary(&content, "elwynn", hero_setup(&content, &class, &loadout), SEED, kite, true, Some(&amulet), &own, "fireball");
        let without = play_legendary(&content, "elwynn", hero_setup(&content, &class, &loadout), SEED, kite, true, None, &own, "fireball");
        assert!(with.worn, "the legendary was taken from the first cache");
        assert!(!without.worn);
        assert!(with.own > 0.0 && without.own > 0.0 && with.own <= with.total);
    }
}
