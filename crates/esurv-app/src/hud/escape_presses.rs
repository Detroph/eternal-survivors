//! `ESURV_ESCAPE_AT=<seconds>[,<seconds>...]`, a development aid: presses Escape inside the
//! game (a key event of the game's own, no key of the operating system), for a live check of
//! pausing with nobody at the keyboard. The first press comes when the run clock reaches the first
//! time, each later one that much later in real seconds (the clock stands while the run is
//! paused). A press goes the whole stock way: the interface, the key binding, the game menu and
//! its stand-in.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::RunHud;

/// The switch's name.
const SWITCH: &str = "ESURV_ESCAPE_AT";

/// The presses still to come, as seconds after the first; when the first came (real seconds).
#[derive(Resource, Default)]
struct EscapePresses {
    /// The run clock the first press waits for.
    first: f32,
    due: Vec<f32>,
    first_at: Option<f32>,
    /// A press went down last frame: it comes up now.
    held: bool,
}

/// Reads the switch's times: seconds, comma-separated, in order; anything else is left out.
pub(super) fn parse_times(value: &str) -> Vec<f32> {
    let mut times: Vec<f32> = value
        .split(',')
        .filter_map(|t| t.trim().parse().ok())
        .filter(|t: &f32| t.is_finite() && *t >= 0.0)
        .collect();
    times.sort_by(f32::total_cmp);
    times
}

/// An Escape key event of the game's own, for `window`: down or up.
pub(crate) fn escape_key(window: Entity, state: ButtonState) -> KeyboardInput {
    KeyboardInput {
        key_code: KeyCode::Escape,
        logical_key: Key::Escape,
        state,
        text: None,
        repeat: false,
        window,
    }
}
/// Adds the presses when the switch is set.
pub(super) fn add(app: &mut App) {
    let Ok(value) = std::env::var(SWITCH) else {
        return;
    };
    let due = parse_times(&value);
    let Some(&first) = due.first() else {
        return;
    };
    info!("hud: Escape will be pressed at {due:?} s of the run ({SWITCH})");
    app.insert_resource(EscapePresses {
        first,
        due,
        ..default()
    })
    // before the input systems turn key events into pressed keys
    .add_systems(First, press_escape);
}

fn press_escape(
    mut presses: ResMut<EscapePresses>,
    run: Res<RunHud>,
    time: Res<Time<Real>>,
    window: Option<Single<Entity, With<PrimaryWindow>>>,
    mut keys: MessageWriter<KeyboardInput>,
) {
    let Some(window) = window else {
        return;
    };
    let now = time.elapsed_secs();
    let escape = |state| escape_key(*window, state);
    if presses.held {
        presses.held = false;
        keys.write(escape(ButtonState::Released));
        return;
    }
    let Some(&next) = presses.due.first() else {
        return;
    };
    let due = match presses.first_at {
        Some(first_at) => now - first_at >= next - presses.first,
        None => run
            .numbers
            .as_ref()
            .is_some_and(|h| h.clock as f32 >= presses.first),
    };
    if due {
        presses.first_at.get_or_insert(now);
        presses.due.remove(0);
        presses.held = true;
        info!("hud: Escape pressed ({SWITCH})");
        keys.write(escape(ButtonState::Pressed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_times_read_in_order() {
        assert_eq!(parse_times("14, 6,x,-1"), vec![6.0, 14.0]);
        assert!(parse_times("").is_empty());
    }
}
