//! A crate's own screen standing in for the stock game menu (a run's pause panel, say). While
//! [`GameMenuStandIn`] is on, Escape still closes what the stock ladder closes first (a focused
//! edit box takes it before any binding; then a popup, an options window, a dropdown, a cast, spell
//! targeting, an open window), but where `ToggleGameMenu` would go on to clear the target or open
//! `GameMenuFrame`, the crate gets a [`GameMenuAsked`] instead; the main menu button closes the
//! dropdowns and windows, as its stock arm does, and asks too. A game menu already open is closed
//! by Escape as ever.
//!
//! The Lua half is `ToggleGameMenu`'s wrapper in `GameMenuAdapters.xml`: it reads
//! `BENILLA_GAME_MENU_STAND_IN`, which this module keeps in step with the resource, and counts
//! the asks in `BENILLA_GAME_MENU_ASKED`, which this module takes after the input pass.

use bevy::prelude::*;

use benilla_ui::script::UiScript;

/// While on, Escape and the menu button ask the crate's screen ([`GameMenuAsked`]) where they
/// would clear the target or open the stock game menu. A crate sets it while its screen is the
/// menu, and clears it otherwise.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameMenuStandIn(pub bool);

/// The player asked for the game menu while [`GameMenuStandIn`] was on: the crate's screen answers.
#[derive(Message, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameMenuAsked;

/// Adds the stand-in: the flag reaches Lua before the input pass's clicks and key bindings, and
/// the asks come back after them.
pub(super) fn register(app: &mut App) {
    app.init_resource::<GameMenuStandIn>()
        .add_message::<GameMenuAsked>()
        .add_systems(
            Update,
            (
                tell_lua_the_stand_in
                    .in_set(super::UiInput)
                    .after(super::extract::tick_script)
                    .before(super::input::feed_ui_input),
                take_the_asks
                    .in_set(super::UiInput)
                    .after(crate::bindings::BindingSet)
                    .in_set(crate::char_select::InWorldGated),
            ),
        );
}

/// Keeps `BENILLA_GAME_MENU_STAND_IN` as the resource says, in every new VM too.
fn tell_lua_the_stand_in(
    script: Option<NonSendMut<UiScript>>,
    stand_in: Res<GameMenuStandIn>,
    mut told: Local<super::VmMemo<bool>>,
) {
    let Some(script) = script else {
        return;
    };
    if !told.claim(&script) && !stand_in.is_changed() {
        return;
    }
    let value = if stand_in.0 { "1" } else { "nil" };
    if let Err(e) = script.run(&format!("BENILLA_GAME_MENU_STAND_IN = {value}")) {
        warn!("game menu stand-in: {e}");
    }
}

/// The asks this frame's keys and clicks made, as [`GameMenuAsked`]s. Only a key press or a click
/// asks, so Lua is looked at only then.
fn take_the_asks(
    script: Option<NonSendMut<UiScript>>,
    stand_in: Res<GameMenuStandIn>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut asked: MessageWriter<GameMenuAsked>,
) {
    let Some(script) = script else {
        return;
    };
    let input =
        keys.get_just_pressed().next().is_some() || buttons.get_just_released().next().is_some();
    if !stand_in.0 || !input {
        return;
    }
    let asks = script.eval::<u32>(
        "local n = BENILLA_GAME_MENU_ASKED or 0 BENILLA_GAME_MENU_ASKED = nil return n",
    );
    match asks {
        Ok(n) => {
            for _ in 0..n {
                info!("game menu stand-in: the menu was asked for; the crate's screen answers");
                asked.write(GameMenuAsked);
            }
        }
        Err(e) => warn!("game menu stand-in: {e}"),
    }
}
