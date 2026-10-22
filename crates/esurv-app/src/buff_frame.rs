//! The buff frame at the top right in a run shows each buff's time left: the
//! stock option `SHOW_BUFF_DURATIONS` ("Show Buff Durations", off in the stock
//! interface) is turned on as the hero enters the world, so the stock buttons write "10 sec" /
//! "1 min" under their icons and blink near the end. The time itself is the server's
//! (`SMSG_UPDATE_AURA_DURATION` when a buff starts or is renewed), which the stock frame reads
//! through `GetPlayerBuffTimeLeft`.

use bevy::prelude::*;

use benilla_app::Objects;
use benilla_ui::script::UiScript;

use crate::ui::in_world;

/// Turns the durations on and lays the buff rows out for them (the rows sit further apart with
/// a line for the time under each).
pub const LUA: &str = "SHOW_BUFF_DURATIONS = \"1\"; \
if BuffButtons_UpdatePositions then BuffButtons_UpdatePositions() end";

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, show_buff_durations);
}

/// Each time the hero enters the world, in each interface VM.
fn show_buff_durations(
    script: Option<NonSend<UiScript>>,
    objects: Objects,
    mut told: Local<Option<(u64, bool)>>,
) {
    let Some(script) = script else {
        return;
    };
    let now = (script.session(), in_world(&objects));
    if *told == Some(now) {
        return;
    }
    *told = Some(now);
    if now.1 {
        if let Err(e) = script.run(LUA) {
            warn!("buff frame: the durations did not turn on: {e}");
        }
    }
}
