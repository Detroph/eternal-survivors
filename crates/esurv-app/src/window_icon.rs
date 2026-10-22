//! The game window's icon: the title bar and the taskbar show the
//! Eternal Survivors emblem, read from the game exe's own icon resource (resource 1, embedded by
//! `crates/esurv/build.rs`). A build without that resource (another platform, or no Windows SDK
//! at build time) keeps the default icon.

use bevy::prelude::*;

pub(crate) fn register(app: &mut App) {
    #[cfg(windows)]
    app.add_systems(Update, set_window_icon);
    #[cfg(not(windows))]
    let _ = app;
}

/// Once the primary window exists: give it the exe's icon, then stop trying.
#[cfg(windows)]
fn set_window_icon(mut done: Local<bool>, _main_thread: bevy::ecs::system::NonSendMarker) {
    use winit::platform::windows::IconExtWindows;
    if *done {
        return;
    }
    bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
        for window in windows.windows.values() {
            // the exe's resource 1, at the sizes the title bar and taskbar ask for; an exe
            // without one keeps the default, and isn't asked again every frame
            if let Ok(icon) = winit::window::Icon::from_resource(1, None) {
                window.set_window_icon(Some(icon));
            }
            *done = true;
        }
    });
}
