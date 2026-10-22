//! `ESURV_BAR_DRAG=<soon|later>@<seconds>:<dx>,<dy>`, a development aid: drags a group of boss
//! timer bars by its handle inside the game, with nobody at the mouse. That many real seconds
//! after the hero entered the world, the game's own cursor is put on the handle's middle, the
//! left button goes down (a mouse event of the game's own, as a click's), the cursor walks
//! `dx`,`dy` logical pixels over half a second, and the button comes up: the whole way a hand's
//! drag goes (the world's mouse, the camera, the stock interface, the handle's `Interaction`).
//! The group's place before and after is logged. `ESURV_BAR_DRAG_SHOTS=<png>` adds the game's own
//! shot a second after the release, `<stem>-moved.png`, and one at the start, `<stem>-before.png`.
//! Putting the game's cursor somewhere moves the system's cursor while the game has it; nothing
//! else of the system is touched, and the window is never raised or pinned.

use bevy::input::mouse::MouseButtonInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::{CursorMoved, CursorOptions, PrimaryWindow};

use benilla_app::Objects;

use crate::boss_bar_places::{BarGroup, BarHandle, BarPlaces};
use crate::ui::in_world;

const SWITCH: &str = "ESURV_BAR_DRAG";
const SHOTS: &str = "ESURV_BAR_DRAG_SHOTS";
/// Frames the cursor rests on the handle before the press, and after the walk before the release.
const SETTLE_FRAMES: u32 = 3;
/// Real seconds the walk takes.
const WALK_SECS: f32 = 0.5;

/// One drag the switch asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragAsked {
    pub group: BarGroup,
    pub at: f32,
    pub by: Vec2,
}

/// Reads the switch's value; anything else is `None`.
pub fn parse(value: &str) -> Option<DragAsked> {
    let (group, rest) = value.trim().split_once('@')?;
    let group = match group.trim() {
        "soon" => BarGroup::Soon,
        "later" => BarGroup::Later,
        _ => return None,
    };
    let (at, by) = rest.split_once(':')?;
    let (dx, dy) = by.split_once(',')?;
    let at: f32 = at.trim().parse().ok()?;
    let by = Vec2::new(dx.trim().parse().ok()?, dy.trim().parse().ok()?);
    (at.is_finite() && at >= 0.0 && by.is_finite()).then_some(DragAsked { group, at, by })
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Waiting,
    /// The cursor on the handle's middle, frames left before the press.
    Resting { from: Vec2, frames: u32 },
    /// The button down, the walk started then.
    Walking { from: Vec2, since: f32 },
    /// At the end, frames left before the release.
    Ending { frames: u32 },
    /// Released; the shot is due then.
    Done { shot_at: f32 },
    Over,
}

#[derive(Resource)]
struct BarDragCheck {
    asked: DragAsked,
    shots: Option<String>,
    entered: Option<f32>,
    phase: Phase,
    before: Option<(f32, f32)>,
}

pub(crate) fn register(app: &mut App) {
    let Ok(value) = std::env::var(SWITCH) else { return };
    let Some(asked) = parse(&value) else {
        warn!("boss bars: {SWITCH}={value:?} means nothing");
        return;
    };
    info!("boss bars: a drag will be made: {asked:?} ({SWITCH})");
    app.insert_resource(BarDragCheck {
        asked,
        shots: std::env::var(SHOTS).ok().filter(|v| !v.is_empty()),
        entered: None,
        phase: Phase::Waiting,
        before: None,
    })
    // before the input systems turn the button's events into a pressed button
    .add_systems(First, drag_the_bars);
}

fn shot(commands: &mut Commands, out: &Option<String>, name: &str) {
    let Some(out) = out else { return };
    let path = match out.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}-{name}.{ext}"),
        None => format!("{out}-{name}.png"),
    };
    info!("boss bars: shot {path} ({SHOTS})");
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
}

#[allow(clippy::too_many_arguments)]
fn drag_the_bars(
    mut check: ResMut<BarDragCheck>,
    objects: Objects,
    time: Res<Time<Real>>,
    window: Option<Single<(Entity, &mut Window, &CursorOptions), With<PrimaryWindow>>>,
    handles: Query<(&BarHandle, &ComputedNode, &UiGlobalTransform)>,
    places: Res<BarPlaces>,
    mut buttons: MessageWriter<MouseButtonInput>,
    mut moved: MessageWriter<CursorMoved>,
    mut commands: Commands,
) {
    let Some(window) = window else { return };
    let (entity, mut window, cursor) = window.into_inner();
    let now = time.elapsed_secs();
    let mut put = |window: &mut Window, at: Vec2| {
        window.set_cursor_position(Some(at));
        moved.write(CursorMoved { window: entity, position: at, delta: None });
    };
    let button = |state| MouseButtonInput { button: MouseButton::Left, state, window: entity };
    let check = &mut *check;
    match check.phase {
        Phase::Waiting => {
            if !in_world(&objects) {
                check.entered = None;
                return;
            }
            let entered = *check.entered.get_or_insert(now);
            if now - entered < check.asked.at {
                return;
            }
            let Some((_, node, place)) = handles.iter().find(|(h, n, _)| h.0 == check.asked.group && n.size().x > 0.0) else {
                return; // not shown yet
            };
            let from = place.translation * node.inverse_scale_factor();
            check.before = places.of(check.asked.group);
            info!("boss bars: the drag starts on the handle at {from:?}, the group at {:?}", check.before);
            shot(&mut commands, &check.shots, "before");
            put(&mut window, from);
            check.phase = Phase::Resting { from, frames: SETTLE_FRAMES };
        }
        Phase::Resting { from, frames } => {
            put(&mut window, from);
            if frames > 0 {
                check.phase = Phase::Resting { from, frames: frames - 1 };
            } else {
                buttons.write(button(ButtonState::Pressed));
                check.phase = Phase::Walking { from, since: now };
            }
        }
        Phase::Walking { from, since } => {
            let t = ((now - since) / WALK_SECS).clamp(0.0, 1.0);
            put(&mut window, from + check.asked.by * t);
            if t >= 1.0 {
                // the world took the press when the camera hid and locked the cursor for a look
                info!("boss bars: at the walk's end the cursor is {:?}, {}", cursor.grab_mode, if cursor.visible { "shown" } else { "hidden" });
                check.phase = Phase::Ending { frames: SETTLE_FRAMES };
            }
        }
        Phase::Ending { frames } => {
            if frames > 0 {
                check.phase = Phase::Ending { frames: frames - 1 };
            } else {
                buttons.write(button(ButtonState::Released));
                check.phase = Phase::Done { shot_at: now + 1.0 };
            }
        }
        Phase::Done { shot_at } => {
            if now >= shot_at {
                info!(
                    "boss bars: the drag is over: the group was at {:?}, is at {:?}",
                    check.before,
                    places.of(check.asked.group)
                );
                shot(&mut commands, &check.shots, "moved");
                check.phase = Phase::Over;
            }
        }
        Phase::Over => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_reads_a_group_a_time_and_a_way() {
        assert_eq!(
            parse("later@30:400,-120"),
            Some(DragAsked { group: BarGroup::Later, at: 30.0, by: Vec2::new(400.0, -120.0) })
        );
        assert_eq!(parse(" soon @ 5 : 1 , 2 ").map(|d| d.group), Some(BarGroup::Soon));
        assert_eq!(parse("both@5:1,2"), None);
        assert_eq!(parse("soon@5"), None);
        assert_eq!(parse("soon@-1:1,2"), None);
    }
}
