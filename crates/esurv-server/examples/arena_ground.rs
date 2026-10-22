//! How a zone's arena sits on the real ground: the spawn's height against the floor, how much of
//! each ring around the centre an enemy may appear on (a floor near the hero's height, in the
//! hero's sight) and how far each heading stays open. Works on terrain
//! maps and on WMO-only ones (Dire Maul), where floors stack and walls block sight.
//!
//! `cargo run -p esurv-server --example arena_ground -- elwynn` (needs the client's Data), or
//! a spot no zone has yet: `-- 429 806.63 482.04 37.32 80` (map, x, y, z, radius).

use std::f32::consts::TAU;
use std::time::Instant;

use esurv_content::Content;
use esurv_server::ground::GameData;
use esurv_sim::game::run::{Walk, World, SIGHT_HEIGHT, SPAWN_HEIGHT_TOLERANCE};

/// Points sampled around each ring.
const STEPS: usize = 72;
/// Yards between rings.
const RING_STEP: f32 = 10.0;
/// Yards between the samples along a heading.
const REACH_STEP: f32 = 1.0;
/// A heading's floor may rise or fall this much per sample and still count as walked on.
const REACH_CLIMB: f32 = 2.0;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = benilla_formats::wow_data().ok_or_else(|| anyhow::anyhow!("no WoW install found"))?;
    let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content"))?;
    let data = GameData::open(&dir, &content)?;
    let number = |i: usize| -> anyhow::Result<f32> {
        Ok(args.get(i).ok_or_else(|| anyhow::anyhow!("missing argument {i}"))?.parse()?)
    };
    let (label, map, cx, cy, spawn_z, radius) = match args.first().map(|a| a.parse::<u32>()) {
        Some(Ok(map)) => (format!("map {map}"), map, number(1)?, number(2)?, number(3)?, number(4)?),
        _ => {
            let id = args.first().cloned().unwrap_or_else(|| "elwynn".into());
            let zone = content.zones.iter().find(|z| z.id == id).ok_or_else(|| anyhow::anyhow!("no zone {id}"))?;
            (id, zone.map_id, zone.spawn.x, zone.spawn.y, zone.spawn.z, zone.arena_radius)
        }
    };
    let ground = data.ground(map)?;
    let centre = ground.floor(cx, cy, spawn_z);
    println!("{label}: map {map} centre ({cx}, {cy}) spawn z {spawn_z} floor {centre:?} radius {radius}");
    let hero = centre.unwrap_or(spawn_z);
    let eye = |x: f32, y: f32, z: f32| [x, y, z + SIGHT_HEIGHT];

    // the first walk builds the flow field out to twice the radius; time it
    let feet = [cx, cy, hero];
    let started = Instant::now();
    let first = ground.walk(feet, (cx + 1.0, cy), radius * 2.0);
    println!("flow field out to {:.0} yd: {:.1} ms ({first:?} 1 yd out)", radius * 2.0, started.elapsed().as_secs_f64() * 1e3);

    println!(
        "ring  usable  seen  walked  lowest  highest   (against the centre; usable = a floor within \
         {SPAWN_HEIGHT_TOLERANCE} yd, in sight; walked = reached on foot within twice the radius, \
         the walk's floor within {SPAWN_HEIGHT_TOLERANCE} yd)"
    );
    let mut r = RING_STEP;
    while r <= radius + 0.01 {
        let (mut usable, mut seen, mut walked, mut low, mut high) = (0, 0, 0, f32::MAX, f32::MIN);
        for i in 0..STEPS {
            let a = i as f32 * TAU / STEPS as f32;
            let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
            if let Some(h) = ground.floor(x, y, hero) {
                let d = h - hero;
                (low, high) = (low.min(d), high.max(d));
                let sight = ground.clear_line(eye(cx, cy, hero), eye(x, y, h));
                seen += usize::from(sight);
                usable += usize::from(d.abs() < SPAWN_HEIGHT_TOLERANCE && sight);
            }
            walked += usize::from(match ground.walk(feet, (x, y), radius * 2.0) {
                Walk::Straight => ground.floor(x, y, hero).is_some(),
                Walk::Reached { z, .. } => (z - hero).abs() < SPAWN_HEIGHT_TOLERANCE,
                Walk::Unreachable => false,
            });
        }
        let pct = |n: usize| 100.0 * n as f32 / STEPS as f32;
        println!(
            "{r:>4}  {:>5.0}%  {:>3.0}%  {:>5.0}%  {low:>6.1}  {high:>7.1}",
            pct(usable),
            pct(seen),
            pct(walked)
        );
        r += RING_STEP;
    }

    // A hero walking: each new cell asks a new field, over cells the first one already built.
    let started = Instant::now();
    for k in 1..=20 {
        ground.walk([cx + k as f32, cy, hero], (cx, cy), 40.0);
    }
    println!("a walking hero's flow field out to 40 yd: {:.2} ms each", started.elapsed().as_secs_f64() * 1e3 / 20.0);
    // the followers' field reaches further (100 yd); each cell a hero walks into builds one
    let started = Instant::now();
    for k in 1..=20 {
        ground.toward((cx, cy + 5.0), [cx + k as f32, cy, hero]);
    }
    println!("a walking hero's followers' field: {:.2} ms each", started.elapsed().as_secs_f64() * 1e3 / 20.0);
    // 300 enemies deciding where to go, on the field the hero stands in (built once)
    let feet = [cx + 20.0, cy, ground.floor(cx + 20.0, cy, hero).unwrap_or(hero)];
    ground.toward((cx, cy), feet);
    let started = Instant::now();
    let mut bent = 0;
    for k in 0..300 {
        let a = k as f32 * 2.399_963;
        let r = 40.0 * ((k as f32 + 0.5) / 300.0).sqrt();
        bent += usize::from(ground.toward((cx + r * a.cos(), cy + r * a.sin()), feet).is_some());
    }
    println!(
        "300 enemies' next waypoints: {:.2} ms in all ({bent} round a wall or to another level)",
        started.elapsed().as_secs_f64() * 1e3
    );

    // How far each heading runs on floor, a step at a time, in sight of the centre.
    let mut reach: Vec<f32> = (0..STEPS)
        .map(|i| {
            let a = i as f32 * TAU / STEPS as f32;
            let (mut d, mut z) = (0.0, hero);
            while d < radius * 2.0 {
                let next = d + REACH_STEP;
                let (x, y) = (cx + next * a.cos(), cy + next * a.sin());
                match ground.floor(x, y, z + REACH_CLIMB) {
                    Some(h) if (h - z).abs() <= REACH_CLIMB && ground.clear_line(eye(cx, cy, hero), eye(x, y, h)) => {
                        (d, z) = (next, h)
                    }
                    _ => break,
                }
            }
            d
        })
        .collect();
    reach.sort_by(f32::total_cmp);
    let at = |q: f32| reach[((STEPS - 1) as f32 * q) as usize];
    println!(
        "open reach (yd walked out from the centre, in its sight): min {:.0}, 10% {:.0}, median {:.0}, max {:.0}",
        at(0.0),
        at(0.1),
        at(0.5),
        at(1.0)
    );
    Ok(())
}
