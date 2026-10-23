//! The enemy glow option (screens.md, "Options"): one box, "Glow around elites and rares", on the
//! options panel's Interface tab, in its Eternal Survivors section. Ticked (the default), elites
//! wear a purple outline glow, rares and the Rare Caches they drop a silver one; unticked, all of
//! them hide at once. Bosses and normal enemies never glow, and the warband screen's hero glow
//! does not follow it.
//!
//! The option is the CVar [`CVAR`] (`"1"` on, the default; `"0"` off), kept with the game's
//! other settings.

use benilla_ui::script::UiScript;

/// The option's CVar: `"1"` the glows show, `"0"` they hide.
pub const CVAR: &str = "esurvEnemyGlow";
/// The option's value when nothing was saved: on.
pub const DEFAULT: &str = "1";

/// Registers the CVar with its default, as an addon's is, so it reads and is kept.
pub fn register_cvar(script: &UiScript) {
    let _ = script.eval::<()>(&format!("RegisterCVar(\"{CVAR}\", \"{DEFAULT}\")"));
}

/// Whether a CVar value means the glows show: anything but `"0"` (an unknown or missing value is
/// the default, on).
pub fn glow_on(value: Option<&str>) -> bool {
    value.is_none_or(|v| v.trim() != "0")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_zero_turns_the_glow_off() {
        assert!(glow_on(None));
        assert!(glow_on(Some("1")));
        assert!(glow_on(Some(DEFAULT)));
        assert!(glow_on(Some("")));
        assert!(!glow_on(Some("0")));
        assert!(!glow_on(Some(" 0 ")));
    }
}
