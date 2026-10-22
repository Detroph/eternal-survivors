//! The end of a run: a panel over the game when the hero falls or wins, with the run's tally, the
//! talent experience it paid (revealed part by part, the class's bar filling and each talent point
//! crossed called out), the zone it opened, the gear it won, and what to do next: a fresh run, on
//! into Endless after a victory, or back to the character screen. It is drawn from the server's
//! `RunEnd` (with `Ready` for who and where, and each `GearDropped`) and answers with `again`,
//! `endless` or `leave`. This client says it has the screen, so the server waits at a victory for
//! the player's choice.

use bevy::prelude::*;

mod panel;
pub mod place;
pub mod reveal;
pub mod state;
pub mod tally;

pub use panel::RunEndSystems;
pub use state::{buttons_for, EndButton, EndOfRun, RunEndState, WonPiece};

use crate::protocol::ClientScreens;

/// The end panel: its state, fed by the server's messages, and its window.
pub struct RunEndPlugin;

impl Plugin for RunEndPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientScreens>();
        app.world_mut().resource_mut::<ClientScreens>().0.run_end = true;
        app.init_resource::<RunEndState>();
        panel::add_panel(app);
    }
}
