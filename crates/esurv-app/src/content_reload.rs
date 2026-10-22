//! The screens follow content loaded again while the game runs (dev builds watch the content
//! folder; see `esurv_content::watch`). Out of a run, the screens take the new content at once
//! (the warband screen and its zone list, the hero sheet, the codex) and ask the save's facts
//! anew; in a run they keep the content the run started with, as the server's run does, and take
//! the new one as the hero leaves. Each reload is said in a short line: over the menus, or as a
//! HUD toast in a run; a reload that failed says so, and the full error goes to the log.

use std::sync::Arc;

use bevy::prelude::*;

use benilla_app::Objects;
use esurv_content::{Content, LiveContent, ReloadOutcome};

use crate::hud::{HudLines, LineLook};
use crate::menus::{Menus, PreRunFacts};
use crate::ui::{in_world, GameContent};

/// What a reload that went through says.
pub const RELOADED: &str = "Content reloaded";
/// What it says in a run, which keeps the content it started with.
pub const RELOADED_IN_RUN: &str = "Content reloaded: the next run uses it";
/// The longest error a line in game shows; the log has all of it.
const SHOWN_ERROR_CHARS: usize = 160;

/// Where the screens' content comes from, and the last reload they have said.
#[derive(Resource)]
pub struct ContentSource {
    live: Arc<LiveContent>,
    said: u64,
}

pub(crate) fn register(app: &mut App, live: Arc<LiveContent>) {
    let said = live.last_outcome().map_or(0, |o| o.attempt);
    app.insert_resource(ContentSource { live, said });
    app.add_systems(Update, follow_reloads);
}

/// The line a reload's outcome shows in game.
pub fn reload_line(outcome: &ReloadOutcome, in_run: bool) -> String {
    match &outcome.error {
        None if in_run => RELOADED_IN_RUN.into(),
        None => RELOADED.into(),
        Some(e) => {
            let mut line = format!("Content did not reload: {e}");
            if line.chars().count() > SHOWN_ERROR_CHARS {
                line = line.chars().take(SHOWN_ERROR_CHARS - 3).collect::<String>() + "...";
            }
            line
        }
    }
}

/// The content the screens show next: the live content out of a run, else what they have
/// (`None`: keep it).
pub fn content_to_show(shown: &Arc<Content>, live: &LiveContent, in_run: bool) -> Option<Arc<Content>> {
    if in_run {
        return None;
    }
    let now = live.current();
    (!Arc::ptr_eq(shown, &now)).then_some(now)
}

/// Says each reload once, and swaps the screens' content when out of a run.
fn follow_reloads(
    mut source: ResMut<ContentSource>,
    content: Option<ResMut<GameContent>>,
    objects: Objects,
    time: Res<Time>,
    (menus, facts, lines): (Option<ResMut<Menus>>, Option<ResMut<PreRunFacts>>, Option<ResMut<HudLines>>),
) {
    let in_run = in_world(&objects);
    if let Some(outcome) = source.live.last_outcome().filter(|o| o.attempt != source.said) {
        source.said = outcome.attempt;
        match &outcome.error {
            None => info!("content: reloaded"),
            Some(e) => warn!("content: did not reload, keeping what was loaded: {e}"),
        }
        let line = reload_line(&outcome, in_run);
        if in_run {
            if let Some(mut lines) = lines {
                lines.toasts.push(line, LineLook::Toast, time.elapsed_secs());
            }
        } else if let Some(mut menus) = menus {
            menus.notice = Some(line);
        }
    }
    let Some(mut content) = content else {
        return;
    };
    if let Some(new) = content_to_show(&content.0, &source.live, in_run) {
        content.0 = new;
        // the save's facts (zones open, gear) are read against the content: ask them anew
        if let Some(mut facts) = facts {
            facts.forget();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(error: Option<&str>) -> ReloadOutcome {
        ReloadOutcome { attempt: 1, error: error.map(Into::into) }
    }

    #[test]
    fn a_reload_says_what_happened() {
        assert_eq!(reload_line(&outcome(None), false), RELOADED);
        assert_eq!(reload_line(&outcome(None), true), RELOADED_IN_RUN);
        let failed = reload_line(&outcome(Some("parsing classes.ron: 3:5: Expected comma")), false);
        assert!(failed.starts_with("Content did not reload") && failed.contains("classes.ron"));
        let long = reload_line(&outcome(Some(&"x".repeat(500))), false);
        assert_eq!(long.chars().count(), SHOWN_ERROR_CHARS);
    }

    #[test]
    fn the_screens_take_new_content_only_out_of_a_run() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let live = LiveContent::load(dir).unwrap();
        let shown = live.current();
        assert!(content_to_show(&shown, &live, false).is_none(), "nothing new");
        live.reload().unwrap();
        assert!(content_to_show(&shown, &live, true).is_none(), "a run keeps what it started with");
        let new = content_to_show(&shown, &live, false).expect("the menus take the new content");
        assert!(Arc::ptr_eq(&new, &live.current()));
    }
}
