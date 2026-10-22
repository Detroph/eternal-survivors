//! A run plays to its end with the content it started with: content loaded again meanwhile (dev
//! builds reload the folder as it changes) reaches the next run, not this one.

use std::path::Path;

use esurv_content::testing::{content_folder_with_zones, set_warrior_base_health};
use esurv_content::LiveContent;
use esurv_sim::game::enemy::RunOptions;
use esurv_sim::game::run::{Flat, HeroSetup, Run};

fn warrior_run(live: &LiveContent) -> Run {
    let hero = HeroSetup { class: "warrior".into(), race: "orc".into(), ..Default::default() };
    Run::new(live.current(), "elwynn", RunOptions::default(), &[hero], Box::new(Flat), 7).unwrap()
}

#[test]
fn a_reload_reaches_the_next_run_and_not_the_one_going_on() {
    let from = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content"));
    let dir = content_folder_with_zones(from, "sim-reload", &[]).unwrap();
    let live = LiveContent::load(&dir).unwrap();
    let mut going_on = warrior_run(&live);
    going_on.tick(0.5);
    let old_health = going_on.heroes[0].stats.max_health;

    set_warrior_base_health(&dir, 999.0).unwrap();
    live.reload().unwrap();

    going_on.tick(0.5);
    assert_eq!(going_on.heroes[0].stats.max_health, old_health, "the run going on keeps its numbers");
    let next = warrior_run(&live);
    assert!(next.heroes[0].stats.max_health > old_health, "the next run starts from the new numbers");
    let _ = std::fs::remove_dir_all(&dir);
}
