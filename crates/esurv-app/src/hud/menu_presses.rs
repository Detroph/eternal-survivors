//! `ESURV_MENU_PRESS=<button>@<seconds>[,...]`, a development aid: opens the Escape menu
//! and presses one of its buttons inside the game, with nobody at the keyboard or the mouse. For
//! each `<button>@<seconds>` in turn (`continue`, `reset`, `options`, `end` or `quit`), once the
//! run clock of a running run reaches the time, Escape is pressed (a key event of the game's own,
//! no key of the operating system, the whole stock way as `ESURV_ESCAPE_AT`'s), and once the
//! menu has shown a moment, the button is clicked (the kit's own `StockClick`, as the mouse would
//! send it). The next press waits for what the last led to: the run going on, the fresh run, the
//! hero out of the world, the Options panel open over the paused run (which then stays paused:
//! the menus' script, `ESURV_MENUS_SCRIPT`, can go on from there). `reset@20,end@15` resets
//! the run at 0:20, then ends the fresh one at 0:15. If the menu does not come (a choice screen
//! came between), Escape is pressed again once the run runs.
//!
//! `ESURV_MENU_SHOTS=<png>` adds a shot of each step: `<stem>-<n>-menu.png` (the menu, just
//! before the click) and `<stem>-<n>-<button>.png` (what the click led to: the run going on, the
//! fresh run, character select), `<n>` counting the presses from 1. The shots are the game's own
//! frames; the window is never raised or pinned.

use std::collections::VecDeque;

use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::PrimaryWindow;

use benilla_app::Objects;
use esurv_protocol::{Phase, ToClient};

use super::escape_menu::{menu_shown, MenuButton};
use super::escape_presses::escape_key;
use super::RunHud;
use crate::menus::Menus;
use crate::protocol::ServerEvent;
use crate::stock::StockClick;
use crate::ui::in_world;

/// The switches' names.
const SWITCH: &str = "ESURV_MENU_PRESS";
const SHOTS: &str = "ESURV_MENU_SHOTS";

/// Real seconds the menu shows before its button is clicked: drawn, and shot, first.
const MENU_SETTLE: f32 = 1.5;
/// Real seconds a press waits for the menu before it presses Escape again.
const MENU_PATIENCE: f32 = 2.0;
/// Real seconds from what a click led to until its shot: the run going on, the fresh run's
/// first frames, character select after the way out of the world.
const SHOT_AFTER_CONTINUE: f32 = 1.5;
const SHOT_AFTER_RESET: f32 = 4.0;
const SHOT_AFTER_END: f32 = 8.0;

/// Where a press has got to.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    /// Waits for a running run whose clock reaches the time.
    Clock,
    /// Escape went down last frame: it comes up now.
    EscapeUp { at: f32 },
    /// Waits for the menu, Escape pressed at `at`; the menu showing since `shown`.
    Menu { at: f32, shown: Option<f32> },
    /// The button was clicked; what it led to came at `since`.
    Outcome { since: Option<f32> },
}

/// The presses still to come and how far the first has got.
#[derive(Resource)]
struct MenuPresses {
    due: VecDeque<(MenuButton, f32)>,
    step: Step,
    /// The shots' path, when they are asked for.
    shots: Option<String>,
    /// The presses done, counting the one under way.
    count: usize,
}

/// Reads the switch: `<button>@<seconds>` items, comma-separated, in order; anything else is
/// left out.
pub(super) fn parse_presses(value: &str) -> Vec<(MenuButton, f32)> {
    value
        .split(',')
        .filter_map(|item| {
            let (word, at) = item.trim().split_once('@')?;
            let button = MenuButton::from_word(word.trim())?;
            let at: f32 = at.trim().parse().ok()?;
            (at.is_finite() && at >= 0.0).then_some((button, at))
        })
        .collect()
}

/// `<stem>-<n>-<what>.png` from the shots' path.
fn shot_path(out: &str, n: usize, what: &str) -> String {
    match out.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}-{n}-{what}.{ext}"),
        None => format!("{out}-{n}-{what}.png"),
    }
}

/// Adds the presses when the switch is set.
pub(super) fn add(app: &mut App) {
    let Ok(value) = std::env::var(SWITCH) else {
        return;
    };
    let due: VecDeque<_> = parse_presses(&value).into();
    if due.is_empty() {
        return;
    }
    let shots = std::env::var(SHOTS).ok().filter(|v| !v.is_empty());
    info!("hud: the Escape menu will be pressed: {due:?} ({SWITCH})");
    app.insert_resource(MenuPresses {
        due,
        step: Step::Clock,
        shots,
        count: 0,
    })
    // before the input systems turn key events into pressed keys
    .add_systems(First, press_the_menu);
}

#[allow(clippy::too_many_arguments)] // one Bevy system's inputs
fn press_the_menu(
    mut presses: ResMut<MenuPresses>,
    mut events: MessageReader<ServerEvent>,
    run: Res<RunHud>,
    objects: Objects,
    time: Res<Time<Real>>,
    window: Option<Single<Entity, With<PrimaryWindow>>>,
    buttons: Query<(Entity, &MenuButton)>,
    mut keys: MessageWriter<KeyboardInput>,
    mut clicks: MessageWriter<StockClick>,
    mut commands: Commands,
    menus: Option<Res<Menus>>,
) {
    let fresh_run = events
        .read()
        .any(|ServerEvent(m)| matches!(m, ToClient::Ready(_)));
    let Some(window) = window else {
        return;
    };
    let Some(&(button, at_clock)) = presses.due.front() else {
        return;
    };
    let now = time.elapsed_secs();
    let in_world = in_world(&objects);
    let phase = run.phase();
    let shoot = |commands: &mut Commands, n: usize, what: &str, out: &Option<String>| {
        if let Some(out) = out {
            let path = shot_path(out, n, what);
            info!("hud: shot {path} ({SHOTS})");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
    };
    match presses.step {
        Step::Clock => {
            let due = in_world
                && phase == Some(Phase::Running)
                && run
                    .numbers
                    .as_ref()
                    .is_some_and(|h| h.clock as f32 >= at_clock);
            if due {
                presses.count += 1;
                info!("hud: Escape pressed for {} ({SWITCH})", button.word());
                keys.write(escape_key(*window, ButtonState::Pressed));
                presses.step = Step::EscapeUp { at: now };
            }
        }
        Step::EscapeUp { at } => {
            keys.write(escape_key(*window, ButtonState::Released));
            presses.step = Step::Menu { at, shown: None };
        }
        Step::Menu { at, shown } => {
            let up = in_world && run.ready.is_some() && menu_shown(phase);
            match (up, shown) {
                (true, None) => {
                    presses.step = Step::Menu {
                        at,
                        shown: Some(now),
                    }
                }
                (true, Some(since)) if now - since >= MENU_SETTLE => {
                    let n = presses.count;
                    shoot(&mut commands, n, "menu", &presses.shots);
                    let Some((entity, _)) = buttons.iter().find(|(_, b)| **b == button) else {
                        return;
                    };
                    info!("hud: {} clicked ({SWITCH})", button.word());
                    clicks.write(StockClick(entity));
                    presses.step = Step::Outcome { since: None };
                }
                (false, _) if now - at >= MENU_PATIENCE && phase == Some(Phase::Running) => {
                    // the menu never came: press again once the run runs
                    presses.count -= 1;
                    presses.step = Step::Clock;
                }
                _ => {}
            }
        }
        Step::Outcome { since: None } => {
            let came = match button {
                MenuButton::Continue => phase == Some(Phase::Running),
                MenuButton::Reset => fresh_run,
                MenuButton::Options => menus.as_deref().is_some_and(Menus::options_over_the_run),
                MenuButton::End => !in_world,
                // the game closes: nothing more comes
                MenuButton::Quit => false,
            };
            if came {
                info!("hud: {} done ({SWITCH})", button.word());
                presses.step = Step::Outcome { since: Some(now) };
            }
        }
        Step::Outcome { since: Some(since) } => {
            let wait = match button {
                MenuButton::Continue | MenuButton::Options => SHOT_AFTER_CONTINUE,
                MenuButton::Reset => SHOT_AFTER_RESET,
                MenuButton::End | MenuButton::Quit => SHOT_AFTER_END,
            };
            if now - since >= wait {
                let n = presses.count;
                shoot(&mut commands, n, button.word(), &presses.shots);
                presses.due.pop_front();
                presses.step = Step::Clock;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presses_read_in_their_order() {
        assert_eq!(
            parse_presses("reset@20, end@15,armory@3,quit@x,continue@9.5"),
            vec![
                (MenuButton::Reset, 20.0),
                (MenuButton::End, 15.0),
                (MenuButton::Continue, 9.5),
            ]
        );
        assert!(parse_presses("").is_empty());
        assert!(parse_presses("reset").is_empty(), "no time");
    }

    #[test]
    fn the_shots_are_named_by_press_and_step() {
        assert_eq!(
            shot_path("C:/s/menu.png", 2, "reset"),
            "C:/s/menu-2-reset.png"
        );
        assert_eq!(shot_path("menu", 1, "menu"), "menu-1-menu.png");
    }
}
