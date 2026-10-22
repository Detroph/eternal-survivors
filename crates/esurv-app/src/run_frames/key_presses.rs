//! `ESURV_KEY_PRESS=<key>@<seconds>[,...]`, a development aid: presses keys inside the
//! game (key events of the game's own, no key of the operating system), for a live look at what
//! the stock keys open in a run with nobody at the keyboard. Each `<key>@<seconds>` (a letter, a
//! digit, or `escape`) is pressed that many real seconds after the hero entered the world, and
//! goes the whole stock way (the interface, the key bindings). `P@8,P@11,N@12` opens the
//! spellbook, closes it, opens the talent frame. `<key>@<seconds>:<hold>` keeps the key down that
//! many seconds (`W@8:6` runs forward for six seconds).
//!
//! `ESURV_KEY_SHOTS=<png>` adds a shot 1.5 s after each press, `<stem>-<n>-<key>.png`
//! (`<n>` counting the presses from 1): the game's own frame; the window is never raised or
//! pinned.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::PrimaryWindow;

use benilla_app::Objects;

use crate::ui::in_world;

const SWITCH: &str = "ESURV_KEY_PRESS";
const SHOTS: &str = "ESURV_KEY_SHOTS";
/// Real seconds from a press to its shot: the window it opened drawn.
const SHOT_AFTER: f32 = 1.5;

/// A key the switch can press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PressKey {
    code: KeyCode,
    /// Its name in the switch and the shots, lower case.
    word: char,
}

impl PressKey {
    /// `p`, `N`, `7` or `escape`.
    pub(super) fn from_word(word: &str) -> Option<Self> {
        let word = word.trim().to_ascii_lowercase();
        if word == "escape" {
            return Some(Self {
                code: KeyCode::Escape,
                word: '\u{1b}',
            });
        }
        let mut chars = word.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            return None;
        };
        let code = match c {
            'a' => KeyCode::KeyA,
            'b' => KeyCode::KeyB,
            'c' => KeyCode::KeyC,
            'd' => KeyCode::KeyD,
            'e' => KeyCode::KeyE,
            'f' => KeyCode::KeyF,
            'g' => KeyCode::KeyG,
            'h' => KeyCode::KeyH,
            'i' => KeyCode::KeyI,
            'j' => KeyCode::KeyJ,
            'k' => KeyCode::KeyK,
            'l' => KeyCode::KeyL,
            'm' => KeyCode::KeyM,
            'n' => KeyCode::KeyN,
            'o' => KeyCode::KeyO,
            'p' => KeyCode::KeyP,
            'q' => KeyCode::KeyQ,
            'r' => KeyCode::KeyR,
            's' => KeyCode::KeyS,
            't' => KeyCode::KeyT,
            'u' => KeyCode::KeyU,
            'v' => KeyCode::KeyV,
            'w' => KeyCode::KeyW,
            'x' => KeyCode::KeyX,
            'y' => KeyCode::KeyY,
            'z' => KeyCode::KeyZ,
            '0' => KeyCode::Digit0,
            '1' => KeyCode::Digit1,
            '2' => KeyCode::Digit2,
            '3' => KeyCode::Digit3,
            '4' => KeyCode::Digit4,
            '5' => KeyCode::Digit5,
            '6' => KeyCode::Digit6,
            '7' => KeyCode::Digit7,
            '8' => KeyCode::Digit8,
            '9' => KeyCode::Digit9,
            _ => return None,
        };
        Some(Self { code, word: c })
    }

    /// Its name in a shot's file name.
    fn name(self) -> String {
        if self.code == KeyCode::Escape {
            "escape".into()
        } else {
            self.word.to_string()
        }
    }

    fn event(self, window: Entity, state: ButtonState) -> KeyboardInput {
        let logical_key = if self.code == KeyCode::Escape {
            Key::Escape
        } else {
            Key::Character(self.word.to_string().into())
        };
        KeyboardInput {
            key_code: self.code,
            logical_key,
            state,
            text: None,
            repeat: false,
            window,
        }
    }
}

/// Reads the switch: `<key>@<seconds>[:<hold>]` items, comma-separated, sorted by time, each with
/// the seconds it is held (0: up the next frame); anything else is left out.
pub(super) fn parse_presses(value: &str) -> Vec<(PressKey, f32, f32)> {
    let mut out: Vec<(PressKey, f32, f32)> = value
        .split(',')
        .filter_map(|item| {
            let (key, when) = item.trim().split_once('@')?;
            let key = PressKey::from_word(key)?;
            let (at, hold) = match when.split_once(':') {
                Some((at, hold)) => (at, hold.trim().parse::<f32>().ok()?),
                None => (when, 0.0),
            };
            let at: f32 = at.trim().parse().ok()?;
            (at.is_finite() && at >= 0.0 && hold.is_finite() && hold >= 0.0)
                .then_some((key, at, hold))
        })
        .collect();
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    out
}

/// `<stem>-<n>-<key>.png` from the shots' path.
fn shot_path(out: &str, n: usize, key: &str) -> String {
    match out.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}-{n}-{key}.{ext}"),
        None => format!("{out}-{n}-{key}.png"),
    }
}

#[derive(Resource)]
struct KeyPresses {
    due: Vec<(PressKey, f32, f32)>,
    shots: Option<String>,
    /// When the hero entered the world (real seconds).
    entered: Option<f32>,
    /// A key down, and when it comes up (real seconds).
    held: Option<(PressKey, f32)>,
    /// Shots to take: (when, path).
    shots_due: Vec<(f32, String)>,
    pressed: usize,
}

pub(super) fn add(app: &mut App) {
    let Ok(value) = std::env::var(SWITCH) else {
        return;
    };
    let due = parse_presses(&value);
    if due.is_empty() {
        return;
    }
    info!("run frames: keys will be pressed: {due:?} ({SWITCH})");
    app.insert_resource(KeyPresses {
        due,
        shots: std::env::var(SHOTS).ok().filter(|v| !v.is_empty()),
        entered: None,
        held: None,
        shots_due: Vec::new(),
        pressed: 0,
    })
    // before the input systems turn key events into pressed keys
    .add_systems(First, press_keys);
}

fn press_keys(
    mut presses: ResMut<KeyPresses>,
    objects: Objects,
    time: Res<Time<Real>>,
    window: Option<Single<Entity, With<PrimaryWindow>>>,
    mut keys: MessageWriter<KeyboardInput>,
    mut commands: Commands,
) {
    let Some(window) = window else {
        return;
    };
    let now = time.elapsed_secs();
    if let Some((key, until)) = presses.held {
        if now >= until {
            presses.held = None;
            keys.write(key.event(*window, ButtonState::Released));
        }
    }
    while let Some(i) = presses.shots_due.iter().position(|(at, _)| now >= *at) {
        let (_, path) = presses.shots_due.remove(i);
        info!("run frames: shot {path} ({SHOTS})");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    if !in_world(&objects) {
        presses.entered = None;
        return;
    }
    let entered = *presses.entered.get_or_insert(now);
    let Some(&(key, at, hold)) = presses.due.first() else {
        return;
    };
    // one key down at a time: the next waits for the held one to come up
    if now - entered < at || presses.held.is_some() {
        return;
    }
    presses.due.remove(0);
    presses.pressed += 1;
    info!("run frames: {} pressed ({SWITCH})", key.name());
    keys.write(key.event(*window, ButtonState::Pressed));
    presses.held = Some((key, now + hold));
    if let Some(out) = &presses.shots {
        let path = shot_path(out, presses.pressed, &key.name());
        presses.shots_due.push((now + SHOT_AFTER, path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presses_read_by_time() {
        let presses = parse_presses("N@12, p@8,escape@20,ab@3,q@x,7@1");
        let keys: Vec<(String, f32)> = presses.iter().map(|(k, at, _)| (k.name(), *at)).collect();
        assert_eq!(
            keys,
            [
                ("7".to_owned(), 1.0),
                ("p".into(), 8.0),
                ("n".into(), 12.0),
                ("escape".into(), 20.0)
            ]
        );
        assert_eq!(
            PressKey::from_word("N").map(|k| k.code),
            Some(KeyCode::KeyN)
        );
        assert_eq!(
            parse_presses("w@8:6.5,s@2:x")
                .iter()
                .map(|(k, at, hold)| (k.name(), *at, *hold))
                .collect::<Vec<_>>(),
            [("w".to_owned(), 8.0, 6.5)],
            "a key held, and a hold that is no number left out"
        );
        assert_eq!(shot_path("C:/s/k.png", 2, "p"), "C:/s/k-2-p.png");
    }
}
