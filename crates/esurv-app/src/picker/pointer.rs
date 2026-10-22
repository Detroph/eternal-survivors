//! The live check's pointer (`ESURV_PICKER_CLICK=<name>`, a dev aid): once the camp has
//! loaded, it clicks the named hero's figure the way the mouse does, by writing the window's own
//! cursor-move and mouse-button events inside the game at the figure's place on screen. No input
//! of the operating system is touched. With `ESURV_PICKER_SHOT` it also writes a shot just
//! before the click (`<shot>_before.png`); the shot itself follows after the click.

use bevy::input::mouse::MouseButtonInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::{CursorMoved, PrimaryWindow};

use benilla_app::SelectScreen;
use benilla_world::view::WorldCamera;

use super::PickerState;

/// Seconds after the camp shows before the click, so the figures are in and posed.
const CLICK_AFTER: f32 = 4.0;
/// The height on the figure the click aims at, yards over its feet: a kneeling or seated body's
/// chest.
const AIM_HEIGHT: f32 = 0.8;
/// The height aimed at on a figure standing at the back: its chest.
const STANDING_AIM_HEIGHT: f32 = 1.4;

#[derive(Resource)]
struct Pointer {
    name: String,
    /// Click the hero's row in the list instead of its figure.
    row: bool,
    ready_at: Option<f32>,
    step: u8,
    before: Option<u64>,
}

pub(super) fn register(app: &mut App) {
    let figure = std::env::var("ESURV_PICKER_CLICK")
        .ok()
        .map(|n| (n, false));
    let row = std::env::var("ESURV_PICKER_CLICK_ROW")
        .ok()
        .map(|n| (n, true));
    if let Some((name, row)) = figure.or(row) {
        info!("picker: {name} gets clicked (in the list: {row}) by ESURV_PICKER_CLICK*");
        app.insert_resource(Pointer {
            name,
            row,
            ready_at: None,
            step: 0,
            before: None,
        })
        .add_systems(Update, click_the_figure);
    }
}

/// Where a point of the world shows in the window, logical pixels.
fn on_screen(cam: &Camera, at: &GlobalTransform, world: Vec3) -> Option<Vec2> {
    cam.world_to_viewport(at, world).ok()
}

#[allow(clippy::too_many_arguments)] // one Bevy system's inputs
fn click_the_figure(
    mut commands: Commands,
    time: Res<Time>,
    screen: Res<SelectScreen>,
    state: Res<PickerState>,
    mut pointer: ResMut<Pointer>,
    window: Query<Entity, With<PrimaryWindow>>,
    cams: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    figures: Query<&GlobalTransform>,
    texts: Query<(&Text, &bevy::ui::UiGlobalTransform, &bevy::ui::ComputedNode)>,
    mut moves: MessageWriter<CursorMoved>,
    mut buttons: MessageWriter<MouseButtonInput>,
) {
    let now = time.elapsed_secs();
    if !screen.scene_ready {
        pointer.ready_at = None;
        return;
    }
    let ready_at = *pointer.ready_at.get_or_insert(now);
    let since = now - ready_at;
    let Ok(window) = window.single() else {
        return;
    };
    let target = screen
        .characters
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(&pointer.name))
        .map(|c| c.guid);
    let figure = target.and_then(|g| state.heroes.iter().find(|(h, _, _)| *h == g));
    match pointer.step {
        0 if since >= CLICK_AFTER => {
            pointer.before = screen.selected;
            if let Ok(path) = std::env::var("ESURV_PICKER_SHOT") {
                let before = path.strip_suffix(".png").unwrap_or(&path).to_string() + "_before.png";
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(before));
            }
            pointer.step = 1;
        }
        1 if since >= CLICK_AFTER + 0.5 && pointer.row => {
            // the row's name line, by its text (the hero not picked yet, so only its row says it)
            let Some((_, t, node)) = texts.iter().find(|(t, _, _)| t.0 == pointer.name) else {
                warn!("picker: no row says {}", pointer.name);
                pointer.step = 4;
                return;
            };
            let position = t.translation * node.inverse_scale_factor();
            info!(
                "picker: the pointer moves onto the row of {} at {position:?}",
                pointer.name
            );
            moves.write(CursorMoved {
                window,
                position,
                delta: None,
            });
            pointer.step = 2;
        }
        1 if since >= CLICK_AFTER + 0.5 => {
            let Some(&(_, e, place)) = figure else {
                warn!(
                    "picker: no figure of {} in this camp to click",
                    pointer.name
                );
                pointer.step = 4;
                return;
            };
            let (Ok((cam, at)), Ok(feet)) = (cams.single(), figures.get(e)) else {
                return;
            };
            let aim = feet.translation()
                + Vec3::Y
                    * match place {
                        super::scenes::Place::Front => AIM_HEIGHT,
                        super::scenes::Place::Back => STANDING_AIM_HEIGHT,
                    };
            let Some(position) = on_screen(cam, at, aim) else {
                warn!("picker: the figure of {} is off the screen", pointer.name);
                pointer.step = 4;
                return;
            };
            info!(
                "picker: the pointer moves onto {} at {position:?}",
                pointer.name
            );
            moves.write(CursorMoved {
                window,
                position,
                delta: None,
            });
            pointer.step = 2;
        }
        2 if since >= CLICK_AFTER + 0.7 => {
            buttons.write(MouseButtonInput {
                button: MouseButton::Left,
                state: ButtonState::Pressed,
                window,
            });
            pointer.step = 3;
        }
        3 if since >= CLICK_AFTER + 0.8 => {
            buttons.write(MouseButtonInput {
                button: MouseButton::Left,
                state: ButtonState::Released,
                window,
            });
            pointer.step = 4;
        }
        4 if since >= CLICK_AFTER + 1.5 => {
            let name = |g: Option<u64>| {
                g.and_then(|g| screen.characters.iter().find(|c| c.guid == g))
                    .map_or("nobody".to_string(), |c| c.name.clone())
            };
            info!(
                "picker: clicked {}: picked before {}, after {}{}",
                pointer.name,
                name(pointer.before),
                name(screen.selected),
                if screen.selected == target {
                    " (the click took)"
                } else {
                    " (THE CLICK DID NOT TAKE)"
                }
            );
            pointer.step = 5;
        }
        _ => {}
    }
}
