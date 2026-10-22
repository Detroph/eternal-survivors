//! The way to the server before a run, on the character screen (the menus before a run):
//! what a class has earned, the run options, the codex and a hero's records come
//! from it, and the changes made there go through it (learn, unlearn, wear, take off, look, run
//! options, rename, a race or look change, the zone). Asks and answers are
//! [`esurv_protocol`]'s ([`PreRunAsk`], [`ToClient`]), so a carrier to a remote host can
//! bring them as lines; a local game's launcher hands the app one that reads and writes its
//! embedded server's save directly. Creating, deleting and entering the world go over the world
//! connection as the stock screens' do.

use std::sync::Arc;

use bevy::prelude::*;
use esurv_protocol::{PreRunAsk, Progress, RunOptions, ToClient};

/// Carries an ask to the server and brings back what it is told.
pub trait PreRunCarrier: Send + Sync + 'static {
    /// The answer to `ask`: what the screens are told (`Progress`, `RunOptions`, `Codex`, `Hero`),
    /// or `Refused` naming the verb that could not be done.
    fn ask(&self, ask: &PreRunAsk) -> Vec<ToClient>;
}

/// The app's way to the server before a run, when there is one (a local game's own save; a
/// remote host has no carrier yet, and its screens show only what the content says).
#[derive(Resource, Clone)]
pub struct PreRunLink(pub Arc<dyn PreRunCarrier>);

impl PreRunLink {
    pub fn ask(&self, ask: &PreRunAsk) -> Vec<ToClient> {
        self.0.ask(ask)
    }

    /// What `class` has earned.
    pub fn progress(&self, class: &str) -> Option<Progress> {
        self.ask(&PreRunAsk::Facts {
            class: class.into(),
        })
        .into_iter()
        .find_map(|m| match m {
            ToClient::Progress(p) => Some(p),
            _ => None,
        })
    }

    /// The run options as saved, and the reward bonus they give.
    pub fn run_options(&self, class: &str) -> Option<(RunOptions, f32)> {
        self.ask(&PreRunAsk::Facts {
            class: class.into(),
        })
        .into_iter()
        .find_map(|m| match m {
            ToClient::RunOptions { options, bonus } => Some((options, bonus)),
            _ => None,
        })
    }
}
