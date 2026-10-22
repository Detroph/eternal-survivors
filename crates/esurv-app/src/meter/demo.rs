//! The dev filler: with `ESURV_METER_DEMO=1` the feed fills with made-up totals that grow
//! over time, so the window can be checked without a run.

use bevy::prelude::*;

use super::feed::{MeterEntry, MeterFeed, MeterMode};

/// The switch.
const DEMO_ENV: &str = "ESURV_METER_DEMO";

/// Seconds between two rounds of made-up hits.
const ROUND_SECONDS: f32 = 0.25;

/// Who deals damage in the demo: name, class (`None` for a pet or a spell of no class) and how
/// hard it hits per round at most.
const DEALERS: [(&str, Option<&str>, u64); 12] = [
    ("Probe", Some("mage"), 420),
    ("Brannoc", Some("warrior"), 380),
    ("Sylvara", Some("hunter"), 350),
    ("Ketch", Some("rogue"), 330),
    ("Maelis", Some("warlock"), 300),
    ("Orrin", Some("paladin"), 240),
    ("Tavi", Some("druid"), 220),
    ("Wolf (Sylvara)", None, 160),
    ("Imp (Maelis)", None, 120),
    ("Sorrel", Some("shaman"), 200),
    ("Elune Wisp", None, 90),
    ("Holy Wisp", Some("priest"), 60),
];

/// What hits back in the demo, and how hard per round at most.
const SOURCES: [(&str, u64); 7] = [
    ("Kobold Tunneler", 90),
    ("Defias Footpad", 120),
    ("Riverpaw Brute", 140),
    ("Murloc Raider", 70),
    ("Hogger", 260),
    ("area", 110),
    ("Mangy Wolf", 50),
];

/// Whether this run wants the demo feed.
pub(super) fn wanted() -> bool {
    std::env::var(DEMO_ENV).is_ok_and(|v| v == "1")
}

/// The demo's clock and its dice.
#[derive(Default)]
pub(super) struct DemoState {
    since_round: f32,
    seed: u64,
}

impl DemoState {
    /// A xorshift roll in `0..below`.
    fn roll(&mut self, below: u64) -> u64 {
        if self.seed == 0 {
            self.seed = 0x9E37_79B9_7F4A_7C15;
        }
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed % below.max(1)
    }
}

/// Advances the feed's clock and, every [`ROUND_SECONDS`], lets most dealers and a few sources
/// land a hit.
pub(super) fn fill_demo_feed(
    time: Res<Time>,
    mut feed: ResMut<MeterFeed>,
    mut state: Local<DemoState>,
) {
    feed.elapsed += time.delta_secs();
    state.since_round += time.delta_secs();
    if state.since_round < ROUND_SECONDS {
        return;
    }
    state.since_round = 0.0;
    for (name, class, most) in DEALERS {
        if state.roll(4) == 0 {
            continue;
        }
        let hit = state.roll(most) + 1;
        add_with_class(&mut feed, name, class, hit);
    }
    for (name, most) in SOURCES {
        if state.roll(3) == 0 {
            let hit = state.roll(most) + 1;
            feed.add(MeterMode::DamageTaken, name, hit);
        }
    }
}

fn add_with_class(feed: &mut MeterFeed, name: &str, class: Option<&str>, hit: u64) {
    let entries = feed.entries_mut(MeterMode::DamageDone);
    match entries.iter_mut().find(|e| e.name == name) {
        Some(entry) => entry.amount += hit,
        None => entries.push(MeterEntry {
            class: class.map(str::to_owned),
            ..MeterEntry::new(name, hit)
        }),
    }
}
