//! Plays one headless run per zone and class with fixed seeds and prints what it measured, as
//! Markdown tables: how long each boss took to kill against what the hero's spells say they
//! deal, how much damage the hero took per minute, and whether every enemy and boss came in with
//! the health and damage the scaling formulas give.
//!
//! The standard hero, the same for every class:
//!
//! * a race that can play the class, no talents, no gear, default run options;
//! * it never moves after the first step, but turns to face the living boss, else the nearest
//!   enemy (so it takes no cache, rune or magnet; orbs still come to it, by pickup range or by
//!   age);
//! * on every choice screen it takes the first offer;
//! * it cannot die: a cheat death with no cooldown puts it back at full health, so the run
//!   always reaches its bosses and the damage taken keeps counting.
//!
//! "Paper DPS" is what the hero's spells say they deal to one target: each damaging spell's
//! damage (with every tick of what lasts, every pulse of a totem) × the expected crit factor ÷
//! its cooldown, × (1 + double-cast) for a learned spell (an effect over time no more than its stack limit can keep up), plus the weapon
//! swing; pets and milestone procs are left out. It is worked out every tick of a boss fight.
//!
//! `cargo run -p esurv-sim --example measure_runs [-- zone ...]`; with
//! `MEASURE_DETAIL=<zone>/<class>` it also prints that run's boss fights source by source.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use esurv_content::{Balance, Content, Delivery, EnemyType, Milestone, Stat};
use esurv_sim::game::abilities::MAX_DOT_STACKS;
use esurv_sim::game::casting::{HitSource, SWING_SCALE};
use esurv_sim::game::choices::damaging;
use esurv_sim::game::enemy::{time_curve, RunOptions};
use esurv_sim::game::run::{EnemyUnit, Event, Flat, HeroSetup, Phase, Run};
use esurv_sim::game::spell_numbers::{can_crit, spell_numbers};

/// Seconds per tick (the server's loop runs at up to 0.1 s; 0.05 is a steady 20 per second).
const DT: f32 = 0.05;
/// A run stops this long after the final boss arrives if it has not fallen.
const OVERTIME: f32 = 600.0;
/// A difference above this share is flagged.
const FLAG: f64 = 0.05;
/// The run minutes the damage-taken table shows (0-based).
const SHOWN_MINUTES: [usize; 9] = [0, 2, 4, 6, 8, 10, 12, 14, 16];

/// One boss of one run.
#[derive(Default)]
struct BossFight {
    boss: usize,
    arrived: f32,
    first_hit: Option<f32>,
    died: Option<f32>,
    max_health: f32,
    level_at_death: u32,
    /// Damage the boss took by source.
    measured: HashMap<String, f64>,
    /// Paper damage by source over the fight (paper DPS × the run time each tick moved on).
    paper: HashMap<String, f64>,
    /// The boss's distance from the hero, summed over the fight's ticks, and the tick count.
    distance_sum: f64,
    ticks: u32,
    /// Another boss was fought at the same time for part of this fight.
    overlapped: bool,
}

impl BossFight {
    fn length(&self) -> Option<f32> {
        Some(self.died? - self.first_hit?)
    }

    fn paper_total(&self) -> f64 {
        self.paper.values().sum()
    }
}

/// What one run measured.
struct Measured {
    class: String,
    fights: Vec<BossFight>,
    /// Damage taken (after reduction, before shields) per run minute.
    taken: Vec<f64>,
    minutes: usize,
    /// Roster spawns checked against the formulas, and the largest relative difference.
    spawns: u32,
    spawn_worst: f32,
}

/// The race the standard hero of `class` plays (one the class allows in WoW).
fn race_for(class: &str) -> &'static str {
    match class {
        "shaman" => "orc",
        "druid" => "night_elf",
        "hunter" => "dwarf",
        _ => "human",
    }
}

/// The paper DPS of each damaging source the hero has: its spells by id, and "swing".
fn paper_dps(run: &Run) -> Vec<(String, f32)> {
    let mut out = Vec::new();
    let h = &run.heroes[0];
    let stats = h.stats;
    let crit = 1.0 + stats.crit_chance * (stats.crit_damage - 1.0);
    let racial = h.spells.racial.iter().map(|s| (s, 1.0));
    // a learned spell may go off again at once (double-cast); the race spell never does
    for (slot, casts) in h.spells.known.iter().map(|s| (s, 1.0 + stats.double_cast.max(0.0))).chain(racial) {
        let Some(a) = run.content.ability(&slot.spell) else { continue };
        if !damaging(a) {
            continue;
        }
        let n = spell_numbers(a, &slot.instance(), &stats);
        let get = |name: &str| n.iter().find(|(k, _)| *k == name).map(|(_, v)| *v);
        let (Some(damage), Some(cooldown)) = (get("damage"), get("cooldown")) else { continue };
        let per_cast = damage * get("pulses").or(get("novas")).unwrap_or(1.0) * if can_crit(a) { crit } else { 1.0 };
        let mut dps = casts * per_cast / cooldown.max(0.01);
        // an effect over time that lasts longer than its cooldown is renewed, not stacked, past
        // the hero's stack limit
        if a.delivery == Delivery::DamageOverTime {
            let lasts = get("ticks").unwrap_or(1.0) * get("every").unwrap_or(1.0);
            let stacks = 1.0 + stats.dot_stacks.min(MAX_DOT_STACKS) as f32;
            dps = dps.min(stacks * per_cast / lasts.max(0.01));
        }
        out.push((slot.spell.clone(), dps));
    }
    for swing in esurv_sim::game::kit::swings_of(&run.content, h) {
        let weapon = 1.0 + h.modifiers.iter().filter(|m| m.stat == Stat::WeaponDamage).map(|m| m.value).sum::<f32>();
        let hit = weapon * swing.damage * SWING_SCALE * stats.damage;
        out.push(("swing".into(), hit * crit / (stats.swing_factor.sqrt() * swing.swing)));
    }
    out
}

/// A hit's source as the paper DPS names it.
fn source_name(source: &HitSource) -> String {
    match source {
        HitSource::Spell(s) | HitSource::Tick(s) => s.clone(),
        HitSource::WeaponSwing => "swing".into(),
        HitSource::Pet(_) => "pet".into(),
        HitSource::Proc { spell, .. } => format!("proc {spell}"),
        HitSource::Assist(c) => format!("assist {c}"),
        HitSource::Enemy(_) | HitSource::Other => "other".into(),
    }
}

/// How a source reaches its targets, for the tables.
fn delivery_name(content: &Content, source: &str) -> &'static str {
    let Some(a) = content.ability(source) else {
        return if source == "swing" { "swing" } else { "extra" };
    };
    match a.delivery {
        Delivery::Projectile => "projectile",
        Delivery::Chain => "random chain",
        Delivery::Nova => "nova",
        Delivery::GroundArea => "ground area",
        Delivery::Bounce => "bounce",
        Delivery::Melee => "melee",
        Delivery::DamageOverTime => "over time",
        Delivery::Finisher => "finisher",
        Delivery::Cone => "cone",
        _ => "other",
    }
}

/// Turns the hero toward the living boss, else the nearest enemy (cones go where it faces).
fn face_the_fight(run: &mut Run) {
    let (p, now) = (run.heroes[0].pos, run.now);
    let d = |e: &EnemyUnit| {
        let q = e.position(now);
        (q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)
    };
    let boss = run.enemies.iter().filter(|e| e.is_boss()).min_by(|a, b| d(a).total_cmp(&d(b)));
    let target = boss.or_else(|| run.enemies.iter().min_by(|a, b| d(a).total_cmp(&d(b))));
    if let Some(q) = target.map(|e| e.position(now)) {
        run.heroes[0].facing = (q.1 - p.1).atan2(q.0 - p.0);
    }
}
/// An enemy's health and damage at spawn by the one stat formula (run minute `m`, no Endless,
/// default options): base health × kind health × its health growth over D × health clock ×
/// toughness, and base damage × kind damage × damage growth over D × damage clock.
fn formula_stats(b: &Balance, kind: EnemyType, toughness: f32, difficulty: f32, m: f32) -> (f32, f32) {
    let k = b.multipliers(kind);
    let s = &b.zone_scaling;
    (
        b.base_health * k.health * s.health_factor(kind, difficulty) * time_curve(&b.health_clock, m) * toughness,
        b.base_damage * k.damage * s.damage_factor(kind, difficulty) * time_curve(&b.damage_clock, m),
    )
}

/// A roster enemy's formula stats: the elite kind, or the normal kind with its toughness.
fn roster_formula_stats(b: &Balance, e: &esurv_content::Enemy, difficulty: f32, m: f32) -> (f32, f32) {
    if e.elite {
        formula_stats(b, EnemyType::Elite, 1.0, difficulty, m)
    } else {
        formula_stats(b, EnemyType::Normal, e.toughness, difficulty, m)
    }
}

fn play(content: &Arc<Content>, zone: &str, class: &str, seed: u64) -> Measured {
    let cheat = Milestone::CheatDeath { cooldown: 0.0, health: 1.0, immune_seconds: 0.0 };
    let setup = HeroSetup { class: class.into(), race: race_for(class).into(), milestones: vec![cheat], ..Default::default() };
    let mut run = Run::new(content.clone(), zone, RunOptions::default(), &[setup], Box::new(Flat), seed).expect("a run");
    let z = content.zones[run.zone].clone();
    let final_minute = z.bosses.iter().filter(|b| b.is_final).map(|b| b.spawn_minute).fold(0.0, f32::max);
    let hero = run.heroes[0].id;
    let p = run.heroes[0].pos;
    run.hero_moved(hero, (p.0 + 1.0, p.1), 0.0, 0.0);
    let mut fights: HashMap<u32, BossFight> = HashMap::new();
    let mut taken = vec![0.0f64; 64];
    let (mut spawns, mut spawn_worst) = (0u32, 0.0f32);
    let mut last_minute = 0usize;
    let mut won = false;
    while run.now < final_minute * 60.0 + OVERTIME && !won && run.phase != Phase::Victory {
        while let Some(screen) = &run.screen {
            let id = run.heroes[screen.hero].id;
            run.choose(id, 0);
        }
        face_the_fight(&mut run);
        let before = run.now;
        run.tick(DT);
        let step = run.now - before;
        let minute = ((run.now / 60.0) as usize).min(63);
        last_minute = minute;
        for e in run.drain_events() {
            match e {
                Event::BossSpawned { id, boss, .. } => {
                    let unit = run.enemies.iter().find(|e| e.id == id).expect("the boss");
                    fights.insert(id, BossFight { boss, arrived: run.now, max_health: unit.max_health, ..Default::default() });
                }
                Event::EnemySpawned { id, roster, .. } => {
                    if let Some(unit) = run.enemies.iter().find(|e| e.id == id) {
                        let (hp, dmg) = roster_formula_stats(&content.balance, &z.enemies[roster], z.difficulty, run.now / 60.0);
                        spawns += 1;
                        spawn_worst = spawn_worst.max(((unit.max_health - hp) / hp).abs().max(((unit.damage - dmg) / dmg).abs()));
                    }
                }
                Event::EnemyHit { id, amount, source, .. } => {
                    if let Some(f) = fights.get_mut(&id) {
                        f.first_hit.get_or_insert(run.now);
                        *f.measured.entry(source_name(&source)).or_default() += amount as f64;
                    }
                }
                Event::EnemyDied { id, .. } => {
                    if let Some(f) = fights.get_mut(&id) {
                        f.died = Some(run.now);
                        f.level_at_death = run.heroes[0].progress.level;
                        won |= z.bosses[f.boss].is_final;
                    }
                }
                Event::HeroHit { amount, .. } => taken[minute] += amount as f64,
                _ => {}
            }
        }
        let paper = paper_dps(&run);
        let (hp, now) = (run.heroes[0].pos, run.now);
        // a fight shared with another boss has no single expected time
        let fought = fights.values().filter(|f| f.first_hit.is_some() && f.died.is_none()).count();
        for (id, f) in fights.iter_mut() {
            if f.first_hit.is_none() || f.died.is_some() {
                continue;
            }
            f.overlapped |= fought > 1;
            if let Some(q) = run.enemies.iter().find(|e| e.id == *id).map(|e| e.position(now)) {
                f.distance_sum += (((q.0 - hp.0).powi(2) + (q.1 - hp.1).powi(2)) as f64).sqrt();
            }
            f.ticks += 1;
            for (n, dps) in &paper {
                *f.paper.entry(n.clone()).or_default() += (*dps * step) as f64;
            }
        }
    }
    let mut fights: Vec<BossFight> = fights.into_values().collect();
    fights.sort_by(|a, b| a.arrived.total_cmp(&b.arrived));
    if std::env::var("MEASURE_DETAIL").is_ok_and(|v| v == format!("{zone}/{class}")) {
        for f in &fights {
            let Some(len) = f.length() else { continue };
            println!("{zone}/{class} boss {} ({len:.0} s, level {}, mean distance {:.1} yd):", f.boss, f.level_at_death, f.distance_sum / f.ticks.max(1) as f64);
            let names: BTreeSet<&String> = f.measured.keys().chain(f.paper.keys()).collect();
            for n in names {
                let (m, p) = (f.measured.get(n).copied().unwrap_or(0.0), f.paper.get(n).copied().unwrap_or(0.0));
                println!("  {n:24} {:12} measured {:8.0}/s  paper {:8.0}/s", delivery_name(content, n), m / len as f64, p / len as f64);
            }
        }
    }
    Measured { class: class.into(), fights, taken, minutes: last_minute + 1, spawns, spawn_worst }
}

/// "+3 %", with a flag past `FLAG`.
fn pct(measured: f64, expected: f64) -> String {
    let d = measured / expected - 1.0;
    let flag = if d.abs() > FLAG { " **!**" } else { "" };
    format!("{:+.0} %{flag}", d * 100.0)
}

fn main() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
    let content = Arc::new(Content::load(dir).expect("content loads"));
    let wanted: Vec<String> = std::env::args().skip(1).collect();
    let classes: Vec<String> = content.classes.iter().map(|c| c.id.clone()).collect();
    // measured and paper boss damage by class and source, over every zone
    let mut by_source: BTreeMap<(String, String), (f64, f64)> = BTreeMap::new();
    for (zi, zone) in content.zones.iter().enumerate() {
        if !wanted.is_empty() && !wanted.contains(&zone.id) {
            continue;
        }
        let results: Vec<Measured> = classes.iter().enumerate().map(|(ci, c)| play(&content, &zone.id, c, 1000 + 100 * zi as u64 + ci as u64)).collect();
        println!("\n### {} (difficulty {})\n", zone.name, zone.difficulty);
        println!("Boss health at arrival against the formula (base health × boss or final boss kind × difficulty × health clock at the arrival minute).\n");
        println!("| Boss | Arrives | Formula health | Measured |");
        println!("|---|---|---|---|");
        for (bi, b) in zone.bosses.iter().enumerate() {
            let kind = if b.is_final { EnemyType::FinalBoss } else { EnemyType::Boss };
            let expected = formula_stats(&content.balance, kind, 1.0, zone.difficulty, b.spawn_minute).0;
            // each arrival against the formula at its own arrival minute
            let seen: Vec<(f32, f32)> = results
                .iter()
                .flat_map(|r| r.fights.iter().filter(|f| f.boss == bi).map(|f| (f.max_health, formula_stats(&content.balance, kind, 1.0, zone.difficulty, f.arrived / 60.0).0)))
                .collect();
            let worst = seen.iter().map(|(h, want)| (h / want - 1.0).abs()).fold(0.0, f32::max);
            println!("| {} | {:.1} min | {:.0} | {} arrivals, at most {:.3} % off |", b.name, b.spawn_minute, expected, seen.len(), worst * 100.0);
        }
        println!("\nTime to kill each boss (s, first hit to death) / expected (its health ÷ the hero's mean paper DPS over the fight), the hero's level when it fell. A fight shared with another boss for a while shows only its time and level: the spells split between the bosses, so no single expected time fits it.\n");
        print!("| Class |");
        for b in &zone.bosses {
            print!(" {} |", b.name);
        }
        println!("\n|---|{}", "---|".repeat(zone.bosses.len()));
        for r in &results {
            print!("| {} |", r.class);
            for bi in 0..zone.bosses.len() {
                match r.fights.iter().find(|f| f.boss == bi) {
                    Some(f) => match f.length() {
                        Some(len) => {
                            let expected = f.max_health as f64 / (f.paper_total() / len as f64).max(1e-3);
                            if f.overlapped {
                                print!(" {len:.0} shared L{} |", f.level_at_death);
                            } else {
                                print!(" {len:.0} / {expected:.0} ({}) L{} |", pct(len as f64, expected), f.level_at_death);
                            }
                        }
                        None => print!(" alive after {OVERTIME:.0} s |"),
                    },
                    None => print!(" — |"),
                }
                if let Some(f) = r.fights.iter().find(|f| f.boss == bi && f.died.is_some() && !f.overlapped) {
                    for (n, v) in &f.measured {
                        by_source.entry((r.class.clone(), n.clone())).or_default().0 += v;
                    }
                    for (n, v) in &f.paper {
                        by_source.entry((r.class.clone(), n.clone())).or_default().1 += v;
                    }
                }
            }
            println!();
        }
        println!("\nDamage taken per minute (after reduction, before shields), by run minute; and the roster spawns checked against the health and damage formulas.\n");
        print!("| Class |");
        for m in SHOWN_MINUTES {
            print!(" min {} |", m + 1);
        }
        println!(" Spawns checked |");
        println!("|---|{}---|", "---|".repeat(SHOWN_MINUTES.len()));
        for r in &results {
            print!("| {} |", r.class);
            for m in SHOWN_MINUTES {
                if m < r.minutes {
                    print!(" {:.0} |", r.taken[m]);
                } else {
                    print!(" — |");
                }
            }
            println!(" {}, at most {:.3} % off |", r.spawns, r.spawn_worst * 100.0);
        }
    }
    println!("\n### Boss damage by source, every zone\n");
    println!("Damage the killed bosses took from each source against its paper damage over the same fights (fights shared with another boss left out).\n");
    println!("| Class | Source | Reaches | Measured | Paper | Measured / paper |");
    println!("|---|---|---|---|---|---|");
    for ((class, source), (m, p)) in &by_source {
        let ratio = if *p > 0.0 { pct(*m, *p) } else { "not on paper".into() };
        println!("| {class} | {source} | {} | {m:.0} | {p:.0} | {ratio} |", delivery_name(&content, source));
    }
}
