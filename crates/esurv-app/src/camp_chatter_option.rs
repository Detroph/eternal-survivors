//! The camp chatter option (plans/hub-hero-life.md): one choice, "Camp chatter",
//! on the options panel's Interface tab, in its Eternal Survivors section. Bubbles and chat (the
//! default): what the camp's heroes say shows over their heads and in the chat; Bubbles only: over
//! their heads alone; Off: they say nothing, though they still gesture as they talk. Wren and the
//! keepers always speak.
//!
//! The option is the CVar [`CVAR`] (`"2"` bubbles and chat, `"1"` bubbles only, `"0"` off), kept
//! with the game's other settings. The camp ([`crate::camp`]) tells the server the level (it mutes
//! the heroes at Off) and, at Bubbles only, keeps the heroes' lines out of the chat frame.

use benilla_ui::script::UiScript;

/// The option's CVar: `"2"` bubbles and chat, `"1"` bubbles only, `"0"` off.
pub const CVAR: &str = "esurvCampChatter";
/// The option's value when nothing was saved: bubbles and chat.
pub const DEFAULT: &str = "2";

/// Registers the CVar with its default, as an addon's is, so it reads and is kept.
pub fn register_cvar(script: &UiScript) {
    let _ = script.eval::<()>(&format!("RegisterCVar(\"{CVAR}\", \"{DEFAULT}\")"));
}

/// The level a CVar value means: `"0"` off, `"1"` bubbles only, anything else (an unknown or
/// missing value too) the default, bubbles and chat.
pub fn level(value: Option<&str>) -> u32 {
    match value.map(str::trim) {
        Some("0") => 0,
        Some("1") => 1,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anything_unknown_is_bubbles_and_chat() {
        assert_eq!(level(None), 2);
        assert_eq!(level(Some(DEFAULT)), 2);
        assert_eq!(level(Some("")), 2);
        assert_eq!(level(Some("7")), 2);
        assert_eq!(level(Some("1")), 1);
        assert_eq!(level(Some(" 0 ")), 0);
    }
}
