//! The golden run: one headless run per zone and class with a fixed seed, its whole event log
//! hashed, so any change of behaviour, down to one event, shows as a different hash.
//!
//! The hero is the same for every class: a race that can play it, no talents, no gear, default
//! run options; it never moves after its first step, takes the first offer on every choice
//! screen, and cannot die (a cheat death with no cooldown), so every run lasts its full length.
//!
//! `cargo run --release -p esurv-sim --example golden_run [-- minutes] [--log <file>]` prints one line per
//! zone and class and the hash of them all (default 6 minutes a run). Compare the last line
//! before and after a change that must not alter play.
//!
//! `-- --log <file>` also writes every event, one Debug line each, under a `# zone class` line per
//! run, so two runs' logs can be compared line by line when a rename changes how an event prints.
//!
//! The events carry names as the game shows them (a zone's, a class's …), which the content points
//! at in the player's client, so the run reads the client's names: from `$WOW_DATA`, else the
//! project folder's `WoW/Data`. With no client data the names show as placeholders and the hash
//! is not comparable to one taken with it; the run says so.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use esurv_content::{ClientNames, Content, Milestone};
use esurv_sim::game::enemy::RunOptions;
use esurv_sim::game::run::{Flat, HeroSetup, Phase, Run};

/// Seconds per tick, as the server's steady loop.
const DT: f32 = 0.05;
/// The seed every run starts from.
const SEED: u64 = 20261014;

/// FNV-1a, 64 bit: a hash that is the same on every machine and build.
fn fnv1a(bytes: &[u8], mut h: u64) -> u64 {
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

const FNV_START: u64 = 0xcbf2_9ce4_8422_2325;

/// A race that can play `class`.
fn race_for(class: &str) -> &'static str {
    match class {
        "shaman" => "orc",
        "druid" => "night_elf",
        "hunter" => "dwarf",
        _ => "human",
    }
}

/// Plays one run for `seconds` and returns the hash of its event log and how many events it had,
/// writing each event's line to `log` too when there is one.
fn play(content: &Arc<Content>, zone: &str, class: &str, seconds: f32, log: &mut Option<BufWriter<File>>) -> (u64, usize) {
    let cheat = Milestone::CheatDeath { cooldown: 0.0, health: 1.0, immune_seconds: 0.0 };
    let setup = HeroSetup { class: class.into(), race: race_for(class).into(), milestones: vec![cheat], ..Default::default() };
    let mut run = Run::new(content.clone(), zone, RunOptions::default(), &[setup], Box::new(Flat), SEED).expect("a run");
    let (hero, p) = (run.heroes[0].id, run.heroes[0].pos);
    run.hero_moved(hero, (p.0 + 1.0, p.1), 0.0, 0.0);
    let (mut h, mut events) = (FNV_START, 0);
    while run.now < seconds && run.phase != Phase::Victory {
        while let Some(screen) = &run.screen {
            let id = run.heroes[screen.hero].id;
            run.choose(id, 0);
        }
        run.tick(DT);
        for e in run.drain_events() {
            let line = format!("{e:?}\n");
            h = fnv1a(line.as_bytes(), h);
            if let Some(log) = log {
                log.write_all(line.as_bytes()).expect("the log is written");
            }
            events += 1;
        }
    }
    (h, events)
}

/// The client's names: `$WOW_DATA`, else the project folder's `WoW/Data`; none without either.
fn client_names() -> ClientNames {
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../WoW/Data");
    let dir = std::env::var_os("WOW_DATA").filter(|d| !d.is_empty()).map(PathBuf::from).unwrap_or(project);
    if !dir.is_dir() {
        eprintln!("golden run: no client data at {}: names show as placeholders, a hash not comparable", dir.display());
        return ClientNames::none();
    }
    ClientNames::read(&dir).expect("the client's names read")
}

fn main() {
    let (mut minutes, mut log) = (6.0f32, None);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--log" {
            let path = args.next().expect("--log takes a file");
            log = Some(BufWriter::new(File::create(&path).expect("the log file is created")));
        } else if let Ok(m) = arg.parse() {
            minutes = m;
        }
    }
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
    let content = Arc::new(Content::load_with_names(dir, &client_names()).expect("content loads"));
    let mut all = FNV_START;
    for zone in &content.zones {
        for class in &content.classes {
            if let Some(log) = &mut log {
                writeln!(log, "# {} {}", zone.id, class.id).expect("the log is written");
            }
            let (h, events) = play(&content, &zone.id, &class.id, minutes * 60.0, &mut log);
            println!("{:10} {:8} {:7} events  {h:016x}", zone.id, class.id, events);
            all = fnv1a(&h.to_le_bytes(), all);
        }
    }
    if let Some(log) = &mut log {
        log.flush().expect("the log is written");
    }
    println!("golden run ({minutes} min a run): {all:016x}");
}
