//! The Options panel over a paused run (menus.md, "In a run: the Escape
//! menu"). The run's Escape menu has an Options button (`hud::escape_menu`); it opens the very
//! panel of the warband screen ([`super::options`]) over the run, which stays paused under it,
//! and the Escape menu hides until it closes, as the stock game menu does under its options
//! windows. Okay, Cancel and Escape close it, back to the Escape menu; the settings apply as on
//! the warband screen, through the same calls on the world session's Lua VM. What Okay leaves for
//! later ([`Menus::run_notice`]) the Escape menu says under its buttons until the run goes on.
//!
//! What differs over a run:
//!
//! * the Saves tab exports only ([`super::save_files::SavesPage::in_run`]);
//! * a setting that takes hold when the world loads says it waits for the next time in the world;
//! * a Key Bindings button opens the stock key binding window (`KeyBindingFrame`, loaded on
//!   demand as the stock game menu loads it). The panel waits hidden under it and comes back when
//!   it closes (its Okay, its Cancel or Escape); the window's own closing would open the stock
//!   game menu, which a run does not have, so that one step is left out while it was opened from
//!   here. The run's frames (`run_frames.lua`) do not block it.
//!
//! The same panel opens from the menu before the first step and over the end panel
//! (Escape never opens the stock game menu in a run).
//!
//! The panel closes by itself, its changes dropped as Cancel drops them, if the run goes on under
//! it (another hero resumes it, a step starts it) or the hero leaves the world.

use benilla_app::Objects;
use benilla_ui::script::UiScript;
use bevy::prelude::*;
use esurv_protocol::Phase;

use super::{options, Menus};
use crate::hud::escape_menu::menu_kind;
use crate::hud::{MenuHere, RunHud};
use crate::ui::in_world;

/// Opens the stock key binding window the stock game menu's way (`KeyBindingFrame_LoadUI`, then
/// `ShowUIPanel`), its closing kept from opening the stock game menu while it was opened from the
/// panel (its `OnHide` clears its line, shows `GameMenuFrame` and updates the micro buttons; the
/// middle step is left out). Returns whether the window is up.
const OPEN_KEY_BINDINGS: &str = r#"
if not KeyBindingFrame and KeyBindingFrame_LoadUI then
    KeyBindingFrame_LoadUI()
end
if not KeyBindingFrame then
    return false
end
if not ESURV_KEY_BINDINGS_WRAPPED then
    ESURV_KEY_BINDINGS_WRAPPED = 1
    local stock = KeyBindingFrame:GetScript("OnHide")
    KeyBindingFrame:SetScript("OnHide", function()
        if ESURV_KEY_BINDINGS_FROM_OPTIONS then
            ESURV_KEY_BINDINGS_FROM_OPTIONS = nil
            KeyBindingFrameOutputText:SetText("")
            if UpdateMicroButtons then
                UpdateMicroButtons()
            end
            return
        end
        if stock then
            stock()
        end
    end)
end
ESURV_KEY_BINDINGS_FROM_OPTIONS = 1
ShowUIPanel(KeyBindingFrame)
if KeyBindingFrame:IsVisible() then
    return true
end
ESURV_KEY_BINDINGS_FROM_OPTIONS = nil
return false
"#;

/// Whether the stock key binding window is up.
const KEY_BINDINGS_SHOWN: &str =
    "if KeyBindingFrame and KeyBindingFrame:IsVisible() then return true end return false";

/// Opens the stock key binding window; whether it came up.
pub(super) fn open_key_bindings(script: &UiScript) -> bool {
    match script.eval::<bool>(OPEN_KEY_BINDINGS) {
        Ok(up) => {
            info!(
                "menus: the key binding window {}",
                if up { "opened" } else { "did not open" }
            );
            up
        }
        Err(e) => {
            warn!("menus: the key binding window did not open: {e}");
            false
        }
    }
}

/// Whether the panel may stay over the run: the hero in the world, the run's Escape menu under it
/// (the run paused, or the menu opened before the first step or over the end panel).
pub(super) fn may_stay(in_world: bool, phase: Option<Phase>, here: MenuHere) -> bool {
    in_world && menu_kind(phase, here).is_some()
}

/// Follows what is under the panel over a run: it closes (as Cancel) once the run goes on or the
/// hero leaves the world, and comes back from under the key binding window once that closes.
pub(super) fn follow_the_run(
    mut menus: ResMut<Menus>,
    run: Option<Res<RunHud>>,
    here: Option<Res<MenuHere>>,
    objects: Objects,
    script: Option<NonSend<UiScript>>,
) {
    let phase = run.as_deref().and_then(RunHud::phase);
    let here = here.as_deref().copied().unwrap_or_default();
    let stays = may_stay(in_world(&objects), phase, here);
    if !stays && menus.run_notice.is_some() {
        menus.run_notice = None;
    }
    let Some(d) = menus.game_options.as_ref().filter(|d| d.over_run) else {
        return;
    };
    if !stays {
        info!("menus: the run went on under the Options panel; it closes");
        if let Some(d) = menus.game_options.take() {
            options::cancel(script.as_deref(), &d);
        }
        return;
    }
    if d.key_bindings {
        let shown = script
            .as_deref()
            .and_then(|s| s.eval::<bool>(KEY_BINDINGS_SHOWN).ok())
            .unwrap_or(false);
        if !shown {
            info!("menus: the key binding window closed; back to the Options panel");
            if let Some(d) = menus.game_options.as_mut() {
                d.key_bindings = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_stays_only_over_the_escape_menu() {
        let none = MenuHere::default();
        assert!(may_stay(true, Some(Phase::Paused), none));
        assert!(!may_stay(true, Some(Phase::Running), none), "the run went on");
        assert!(!may_stay(true, Some(Phase::Victory), none));
        assert!(!may_stay(false, Some(Phase::Paused), none), "out of the world");
        assert!(!may_stay(true, None, none));
        for p in [Phase::Waiting, Phase::Victory, Phase::Defeat] {
            assert!(may_stay(true, Some(p), MenuHere(Some(p))), "{p:?}");
        }
        assert!(
            !may_stay(true, Some(Phase::Running), MenuHere(Some(Phase::Waiting))),
            "a step taken under it"
        );
    }
}
