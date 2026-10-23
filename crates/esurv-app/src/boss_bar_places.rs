//! Where the boss timer bars' two groups sit (DBM-style bars the player
//! places): the "soon" bars and the later bars each move as a group, dragged by a small handle
//! over the group. The handles always show unless "Show bar handles" is off in Options (Interface,
//! Eternal Survivors), then only while Shift is held. Each group's place is kept in `boss_bars.ron` in benilla's state folder, as the meter
//! keeps its window's, and is back after a restart; "Reset bar positions" in Options
//! (Interface, Eternal Survivors, beside the bars' toggles) puts both back where they start.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde::{Deserialize, Serialize};

/// The file's name in [`benilla_app::config_dir`].
const FILE_NAME: &str = "boss_bars.ron";

/// The Options box that puts both groups back (a CVar of our own, "0" at rest: ticking it and
/// pressing Okay resets the places, and it unticks itself).
pub const RESET_CVAR: &str = "esurvBossBarsReset";

/// One of the two groups of bars.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarGroup {
    /// The bars under 5 s, bigger and flashing.
    Soon,
    /// The bars counting further off.
    Later,
}

/// A group's drag handle (its little title strip, shown while Shift is held).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarHandle(pub BarGroup);

/// The groups' kept places: each group's top-left in logical pixels from the game window's
/// top-left; `None` where it starts (the left side, under the boss's health bar).
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BarPlaces {
    pub soon: Option<(f32, f32)>,
    pub later: Option<(f32, f32)>,
}

impl BarPlaces {
    /// Group `g`'s kept place.
    pub fn of(&self, g: BarGroup) -> Option<(f32, f32)> {
        match g {
            BarGroup::Soon => self.soon,
            BarGroup::Later => self.later,
        }
    }

    pub fn set(&mut self, g: BarGroup, at: (f32, f32)) {
        match g {
            BarGroup::Soon => self.soon = Some(at),
            BarGroup::Later => self.later = Some(at),
        }
    }

    /// Reads places written by [`Self::to_text`]; `None` for text that is not.
    pub fn from_text(text: &str) -> Option<Self> {
        ron::from_str(text).ok()
    }

    pub fn to_text(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::new().depth_limit(1)).unwrap_or_default()
    }

    /// Reads the file at `path`: where they start when it is missing or unreadable.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|t| Self::from_text(&t)).unwrap_or_default()
    }

    /// Writes the file at `path` whole: to a side file first, then renamed over the old one.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let side = path.with_extension("ron.new");
        std::fs::write(&side, self.to_text())?;
        std::fs::rename(&side, path)
    }
}

/// Where the file lives (`None`: no state kept this run, or in a test).
#[derive(Resource, Clone, Debug, Default)]
pub struct BarPlacesFile(pub Option<PathBuf>);

impl BarPlacesFile {
    fn keep(&self, places: &BarPlaces) {
        if let Some(path) = &self.0 {
            if let Err(e) = places.save(path) {
                warn!("boss bars: could not keep their places in {}: {e}", path.display());
            }
        }
    }
}

/// A group being dragged: which, and where the cursor took it from its top-left.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct BarDrag(pub Option<(BarGroup, Vec2)>);

pub(crate) fn register(app: &mut App) {
    let file = BarPlacesFile(benilla_app::config_dir().map(|d| d.join(FILE_NAME)));
    let places = file.0.as_deref().map(BarPlaces::load).unwrap_or_default();
    app.insert_resource(places).insert_resource(file).init_resource::<BarDrag>().add_systems(Update, (show_handles, start_drag, follow_drag).chain());
}

/// Whether the handles show: always when the option is on (`always`), else while Shift is held,
/// and while a group is being dragged.
pub fn handles_shown(keys: &ButtonInput<KeyCode>, drag: &BarDrag, always: bool) -> bool {
    always || keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) || drag.0.is_some()
}

fn show_handles(
    keys: Res<ButtonInput<KeyCode>>,
    drag: Res<BarDrag>,
    options: Option<Res<crate::boss_alerts::AlertOptions>>,
    mut handles: Query<&mut Node, With<BarHandle>>,
) {
    let always = options.is_none_or(|o| o.handles);
    let display = if handles_shown(&keys, &drag, always) { Display::Flex } else { Display::None };
    for mut node in &mut handles {
        if node.display != display {
            node.display = display;
        }
    }
}

/// A press on a handle starts its group's drag.
fn start_drag(
    window: Single<&Window, With<PrimaryWindow>>,
    presses: Query<(&Interaction, &BarHandle), Changed<Interaction>>,
    groups: Query<(&BarGroup, &ComputedNode, &UiGlobalTransform)>,
    mut drag: ResMut<BarDrag>,
) {
    let Some(cursor) = window.cursor_position() else { return };
    for (interaction, handle) in &presses {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some((_, node, at)) = groups.iter().find(|(g, ..)| **g == handle.0) else { continue };
        // the group's top-left on screen, in logical pixels
        let scale = node.inverse_scale_factor();
        let top_left = (at.translation - node.size() / 2.0) * scale;
        drag.0 = Some((handle.0, cursor - top_left));
    }
}

/// Follows the cursor while the left button is held; the release keeps the group's new place.
pub fn follow_drag(
    window: Single<&Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut places: ResMut<BarPlaces>,
    mut drag: ResMut<BarDrag>,
    file: Res<BarPlacesFile>,
) {
    let Some((group, grab)) = drag.0 else { return };
    if !mouse.pressed(MouseButton::Left) {
        drag.0 = None;
        file.keep(&places);
        return;
    }
    let Some(cursor) = window.cursor_position() else { return };
    let screen = window.size();
    let at = (cursor - grab).clamp(Vec2::ZERO, (screen - Vec2::splat(40.0)).max(Vec2::ZERO));
    if places.of(group) != Some((at.x, at.y)) {
        places.set(group, (at.x, at.y));
    }
}

/// Both groups back where they start, kept so.
pub fn reset_places(places: &mut BarPlaces, file: &BarPlacesFile) {
    *places = BarPlaces::default();
    file.keep(places);
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::window::WindowResolution;

    use super::*;

    fn moved() -> BarPlaces {
        BarPlaces { soon: Some((600.0, 220.5)), later: Some((30.0, 400.0)) }
    }

    #[test]
    fn the_places_read_back_from_their_file_and_reset_puts_them_back() {
        let dir = std::env::temp_dir().join(format!("esurv-boss-bars-test-{}", std::process::id()));
        let path = dir.join(FILE_NAME);
        assert_eq!(BarPlaces::load(&path), BarPlaces::default(), "no file yet");
        moved().save(&path).unwrap();
        assert_eq!(BarPlaces::load(&path), moved());
        assert_eq!(BarPlaces::from_text("(later: Some((1.0, 2.0)))"), Some(BarPlaces { soon: None, later: Some((1.0, 2.0)) }), "a partial file");
        let file = BarPlacesFile(Some(path.clone()));
        let mut places = moved();
        reset_places(&mut places, &file);
        assert_eq!(places, BarPlaces::default());
        assert_eq!(BarPlaces::load(&path), BarPlaces::default(), "kept reset");
        std::fs::write(&path, "not places").unwrap();
        assert_eq!(BarPlaces::load(&path), BarPlaces::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_drag_moves_its_group_and_the_release_keeps_it() {
        let dir = std::env::temp_dir().join(format!("esurv-boss-bars-drag-{}", std::process::id()));
        let path = dir.join(FILE_NAME);
        let mut world = World::new();
        world.insert_resource(BarPlaces::default());
        world.insert_resource(BarPlacesFile(Some(path.clone())));
        world.insert_resource(BarDrag(Some((BarGroup::Later, Vec2::new(10.0, 5.0)))));
        world.init_resource::<ButtonInput<MouseButton>>();
        let mut window = Window { resolution: WindowResolution::new(1600, 900), ..default() };
        window.set_cursor_position(Some(Vec2::new(500.0, 300.0)));
        world.spawn((window, PrimaryWindow));
        world.resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        world.run_system_once(follow_drag).unwrap();
        assert_eq!(world.resource::<BarPlaces>().later, Some((490.0, 295.0)));
        assert_eq!(world.resource::<BarPlaces>().soon, None, "the other group stays");
        world.resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Left);
        world.run_system_once(follow_drag).unwrap();
        assert_eq!(world.resource::<BarDrag>().0, None);
        assert_eq!(BarPlaces::load(&path).later, Some((490.0, 295.0)), "kept");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_handles_always_show_unless_turned_off_then_with_shift_held_or_while_dragging() {
        let mut keys = ButtonInput::<KeyCode>::default();
        assert!(handles_shown(&keys, &BarDrag(None), true), "on by default: always shown");
        assert!(!handles_shown(&keys, &BarDrag(None), false), "off: hidden in play");
        keys.press(KeyCode::ShiftLeft);
        assert!(handles_shown(&keys, &BarDrag(None), false));
        keys.release(KeyCode::ShiftLeft);
        assert!(handles_shown(&keys, &BarDrag(Some((BarGroup::Soon, Vec2::ZERO))), false));
    }
}
