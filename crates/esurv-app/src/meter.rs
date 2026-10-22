//! The damage meter: a small window over the game listing the run's damage done and damage taken,
//! one bar per entry, largest first. [`MeterFeed`] is its only input; the server's meter lines
//! fill it ([`server`]), and `ESURV_METER_DEMO=1` fills it with made-up totals instead, for a
//! look without a run. The window's
//! place, size, mode and fold are kept in `meter.ron` in benilla's state folder.

use bevy::prelude::*;

mod demo;
mod feed;
pub mod server;
mod settings;
mod window;

pub use feed::{
    format_amount, format_rate, MeterEntry, MeterFeed, MeterMode, MeterReset, MeterRow,
};
pub use settings::MeterSettings;
pub use window::{MeterSystems, ToggleMeter};

/// The meter: its feed, its window, and the demo filler when asked for.
pub struct MeterPlugin;

impl Plugin for MeterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MeterFeed>().add_message::<MeterReset>();
        window::add_window(app);
        if demo::wanted() {
            info!("meter: the demo feed is on (ESURV_METER_DEMO)");
            app.add_systems(Update, demo::fill_demo_feed.before(MeterSystems));
        } else {
            app.init_resource::<server::ServerMeter>()
                .add_systems(Update, server::fill_from_server.before(MeterSystems));
        }
    }
}
