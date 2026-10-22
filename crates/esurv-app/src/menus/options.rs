//! Options on the warband screen (menus.md, "The flow", step 4): the
//! game's own settings in a panel of the menus' look. The stock options frames (Video Options,
//! Sound & Voice, Interface Options) exist only in the world session's Lua interface, which
//! benilla builds when a hero enters the world; so this panel stands in for them before a run. It
//! reads and writes the very settings those frames do, through the same calls they make
//! (`GetCVar`, `SetCVar`, `SetWorldDetail`, `SetGamma`, `SetScreenResolution`, `RestartGx` ...)
//! on the Lua VM benilla keeps at the character select, so a change shows in the game and in the
//! stock frames later.
//!
//! Video and Interface apply on Okay, as the stock frames do, and Cancel drops their changes;
//! Sound changes as it is moved and Cancel puts it back. A setting the game applies only after a
//! restart (or after entering the world) says so.
//!
//! The same panel opens over a paused run, from the run's Escape menu
//! (menus.md, "In a run: the Escape menu"; [`super::over_the_run`]): the same settings through the
//! same calls, on the world session's Lua VM. There a setting that takes hold when the world
//! loads says "from the next run", import is off (the save is not replaced mid-run), and a Key
//! Bindings button opens the stock key binding window, which needs the world's Lua interface;
//! before a run a line says where to find it.

use benilla_ui::script::UiScript;
use bevy::prelude::*;

use super::look::{palette, solid, Arrow, ButtonStyle, Face, MenuLook};
use crate::stock::{MenuLookTip, Tip, TipLine};
use super::MenuAction;

/// The panel's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OptionsTab {
    #[default]
    Video,
    Sound,
    Interface,
    /// The boss timers: warnings, timer bars and their handles.
    Timers,
    /// The save's export and import ([`super::save_files`]).
    Saves,
}

/// When a change takes hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Takes {
    /// At once.
    Now,
    /// After the display is rebuilt (`RestartGx`), which Okay does.
    Display,
    /// When the game starts again.
    Restart,
    /// When a hero next enters the world.
    World,
}

/// How a setting is read and written: a CVar; a pair of the stock frames' calls; the anisotropy
/// stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Via {
    Cvar(&'static str),
    Calls(&'static str, &'static str),
    Anisotropy,
}

/// One setting of the panel.
#[derive(Clone, Copy, Debug)]
enum Setting {
    /// A box: its CVar, label, the box it needs ticked, when it takes hold.
    Check {
        cvar: &'static str,
        label: &'static str,
        needs: Option<&'static str>,
        takes: Takes,
    },
    /// A value on a range, stepped by its arrows.
    Range {
        via: Via,
        label: &'static str,
        min: f32,
        max: f32,
        step: f32,
        takes: Takes,
    },
    /// One of a few values, stepped round by its arrows: its CVar, label, the choices in the
    /// panel's order as (value, label), and how the game reads any value as one of them.
    Choice {
        cvar: &'static str,
        label: &'static str,
        choices: &'static [(&'static str, &'static str)],
        reads: fn(&str) -> &'static str,
        takes: Takes,
    },
    /// The window's resolution, among the display's.
    Resolution,
    /// The multisample format, among the device's.
    Multisample,
}

impl Setting {
    /// Its key in a draft.
    fn key(&self) -> &'static str {
        match self {
            Setting::Check { cvar, .. } | Setting::Choice { cvar, .. } => cvar,
            Setting::Range { via, .. } => match via {
                Via::Cvar(c) => c,
                Via::Calls(get, _) => get,
                Via::Anisotropy => "anisotropic",
            },
            Setting::Resolution => "@resolution",
            Setting::Multisample => "@multisample",
        }
    }
}

use Setting::{Check, Choice, Range};

/// Camera Following Style (`cameraSmoothStyle`), the choices the stock 1.12 dropdown writes and
/// benilla's camera does (`player::camera::FollowStyle`: 0 Never, 1 Smart, 2 Always); our labels.
const CAMERA_STYLES: &[(&str, &str)] =
    &[("1", "Only when moving"), ("2", "Always adjust camera"), ("0", "Never adjust camera")];

/// How the camera reads `cameraSmoothStyle` (`FollowStyle::from_cvar`): 0 and 3 Never, 2 Always,
/// anything else Smart.
fn camera_style_read(v: &str) -> &'static str {
    match v.trim().parse::<f32>().map_or(1, |v| v as i32) {
        0 | 3 => "0",
        2 => "2",
        _ => "1",
    }
}

/// Damage numbers ([`crate::smart_numbers`]): Smart, the default, or
/// All.
const DAMAGE_NUMBERS: &[(&str, &str)] = &[("1", "Smart"), ("0", "All")];

/// How the game reads the Damage numbers CVar ([`crate::smart_numbers::smart_from_value`]):
/// `"0"` All, anything else Smart.
fn damage_numbers_read(v: &str) -> &'static str {
    if crate::smart_numbers::smart_from_value(Some(v)) {
        "1"
    } else {
        "0"
    }
}

/// A setting's tooltip, when it has one: the header, then the lines.
fn tip_of(s: &Setting) -> Option<&'static [&'static str]> {
    match s.key() {
        crate::smart_numbers::CVAR => Some(&[
            "Damage numbers",
            "Smart: every 30 seconds the game takes the average of your hits, each counted as much as it deals, and for the next 30 seconds hides the numbers below 60% of it, so only your bigger hits show. In the first 30 seconds every number shows. Crits are held to it too, but left out of the average unless your crit chance is over 50%.",
            "Each number is one hit: each target of a spell that hits many, and each tick of damage over time. Small ticks can't pull the average down, so they hide.",
            "All: every number shows.",
            "The damage, the meter and the combat log are the same either way; numbers over your hero always show.",
        ]),
        crate::update::CVAR => Some(&[
            "Check for updates",
            "When the game starts, it asks the game's release page on GitHub whether a newer version is out, and the character screen offers it with an Update button.",
            "Update downloads the new version, checks it, and restarts the game into it; your heroes and settings stay.",
            "Nothing about you is sent. Unticked, the game never asks.",
        ]),
        _ => None,
    }
}

/// A row's tooltip from its lines: the header in the heading's colour, the rest as values.
fn row_tip(lines: &[&str]) -> Tip {
    Tip(lines
        .iter()
        .enumerate()
        .map(|(i, l)| TipLine::new(*l, if i == 0 { palette::HEADING } else { palette::VALUE }))
        .collect())
}

/// The tips box's key: no CVar but the stock calls Show Tutorials makes (`TutorialsEnabled` to
/// read, `ResetTutorials` and `ClearTutorials` to write). The tips live with the world session,
/// so the box is open only over a run; a new save starts with them off (the server's).
const TIPS: &str = "@tips";

/// The Video Options frame's settings, one for one (its eighteen boxes, nine sliders, resolution
/// and multisampling; its refresh-rate list is always empty in benilla, which owns no exclusive
/// display mode, so it is left out). Two columns.
const VIDEO_DISPLAY: &[Setting] = &[
    Setting::Resolution,
    Setting::Multisample,
    Check {
        cvar: "gxWindow",
        label: "Windowed",
        needs: None,
        takes: Takes::Display,
    },
    Check {
        cvar: "gxMaximize",
        label: "Windowed and maximized",
        needs: Some("gxWindow"),
        takes: Takes::Display,
    },
    Check {
        cvar: "gxVSync",
        label: "Vertical sync",
        needs: None,
        takes: Takes::Display,
    },
    Check {
        cvar: "gxTripleBuffer",
        label: "Triple buffering",
        needs: Some("gxVSync"),
        takes: Takes::Display,
    },
    Check {
        cvar: "gxCursor",
        label: "Hardware cursor",
        needs: None,
        takes: Takes::Display,
    },
    Check {
        cvar: "gxFixLag",
        label: "Smoother input",
        needs: Some("gxCursor"),
        takes: Takes::Display,
    },
    Check {
        cvar: "useUiScale",
        label: "Scale the interface",
        needs: None,
        takes: Takes::Now,
    },
    Range {
        via: Via::Cvar("uiScale"),
        label: "Interface scale",
        min: 0.64,
        max: 1.0,
        step: 0.01,
        takes: Takes::Now,
    },
    Check {
        cvar: "desktopGamma",
        label: "Desktop brightness",
        needs: None,
        takes: Takes::Now,
    },
    Range {
        via: Via::Calls("GetGamma", "SetGamma"),
        label: "Brightness",
        min: -0.5,
        max: 0.5,
        step: 0.1,
        takes: Takes::Now,
    },
    Check {
        cvar: "movieSubtitle",
        label: "Film subtitles",
        needs: None,
        takes: Takes::Now,
    },
];

const VIDEO_WORLD: &[Setting] = &[
    Range {
        via: Via::Cvar("farclip"),
        label: "View distance",
        min: 177.0,
        max: 777.0,
        step: 60.0,
        takes: Takes::Now,
    },
    Range {
        via: Via::Calls("GetWorldDetail", "SetWorldDetail"),
        label: "Ground detail",
        min: 0.0,
        max: 2.0,
        step: 1.0,
        takes: Takes::Now,
    },
    Range {
        via: Via::Calls("GetTerrainMip", "SetTerrainMip"),
        label: "Terrain textures",
        min: 0.0,
        max: 1.0,
        step: 1.0,
        takes: Takes::Restart,
    },
    Range {
        via: Via::Calls("GetBaseMip", "SetBaseMip"),
        label: "Texture detail",
        min: 0.0,
        max: 1.0,
        step: 1.0,
        takes: Takes::Now,
    },
    Range {
        via: Via::Anisotropy,
        label: "Texture sharpness",
        min: 1.0,
        max: 4.0,
        step: 1.0,
        takes: Takes::Restart,
    },
    Range {
        via: Via::Cvar("spellEffectLevel"),
        label: "Spell effects",
        min: 0.0,
        max: 2.0,
        step: 1.0,
        takes: Takes::Now,
    },
    Range {
        via: Via::Cvar("weatherDensity"),
        label: "Weather",
        min: 0.0,
        max: 3.0,
        step: 1.0,
        takes: Takes::Now,
    },
    Check {
        cvar: "trilinear",
        label: "Smooth texture blending",
        needs: None,
        takes: Takes::Restart,
    },
    Check {
        cvar: "lod",
        label: "Far-off land in less detail",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "pixelShaders",
        label: "All shaders",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "specular",
        label: "Shining terrain",
        needs: Some("pixelShaders"),
        takes: Takes::World,
    },
    Check {
        cvar: "ffxGlow",
        label: "Screen glow",
        needs: Some("pixelShaders"),
        takes: Takes::Now,
    },
    Check {
        cvar: "ffxDeath",
        label: "Death effect",
        needs: Some("pixelShaders"),
        takes: Takes::Now,
    },
    Check {
        cvar: "M2UseShaders",
        label: "Animated model shaders",
        needs: None,
        takes: Takes::World,
    },
    Check {
        cvar: "M2UsePixelShaders",
        label: "Smooth model shading",
        needs: Some("M2UseShaders"),
        takes: Takes::Now,
    },
    Check {
        cvar: "useWeatherShaders",
        label: "Weather shaders",
        needs: None,
        takes: Takes::Now,
    },
];

/// The Sound & Voice frame's settings, one for one.
const SOUND: &[Setting] = &[
    Check {
        cvar: "MasterSoundEffects",
        label: "Sound",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "EnableAmbience",
        label: "Ambient sound",
        needs: Some("MasterSoundEffects"),
        takes: Takes::Now,
    },
    Check {
        cvar: "EnableErrorSpeech",
        label: "Spoken errors",
        needs: Some("MasterSoundEffects"),
        takes: Takes::Now,
    },
    Check {
        cvar: "EnableMusic",
        label: "Music",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "SoundZoneMusicNoDelay",
        label: "Music without pauses",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "SoundListenerAtCharacter",
        label: "Hear from the hero, not the camera",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "EmoteSounds",
        label: "Emote sounds",
        needs: None,
        takes: Takes::Now,
    },
    Range {
        via: Via::Cvar("MasterVolume"),
        label: "Master volume",
        min: 0.0,
        max: 1.0,
        step: 0.1,
        takes: Takes::Now,
    },
    Range {
        via: Via::Cvar("SoundVolume"),
        label: "Effects volume",
        min: 0.0,
        max: 1.0,
        step: 0.1,
        takes: Takes::Now,
    },
    Range {
        via: Via::Cvar("MusicVolume"),
        label: "Music volume",
        min: 0.0,
        max: 1.0,
        step: 0.1,
        takes: Takes::Now,
    },
    Range {
        via: Via::Cvar("AmbienceVolume"),
        label: "Ambience volume",
        min: 0.0,
        max: 1.0,
        step: 0.1,
        takes: Takes::Now,
    },
];

/// The Interface Options frame's settings that are plain CVars the game itself reads (not the Lua
/// interface): controls, camera, names over heads, chat and combat numbers. Left out: what only
/// the Lua interface reads (tooltips, bar text, the per-character interface settings such as
/// combat text, party frames and action bars), Show Helm and Show Cloak (the hero's own, sent to
/// the server in the world), the spam filter, and Key Bindings.
const INTERFACE_CONTROLS: &[Setting] = &[
    Check {
        cvar: "mouseInvertPitch",
        label: "Invert the mouse",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "autointeract",
        label: "Click to move",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "deselectOnClick",
        label: "Clicking the ground clears the target",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "assistAttack",
        label: "Attack on assist",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "autoSelfCast",
        label: "Cast on yourself when nothing is targeted",
        needs: None,
        takes: Takes::Now,
    },
    Range {
        via: Via::Cvar("mousespeed"),
        label: "Mouse sensitivity",
        min: 0.5,
        max: 1.5,
        step: 0.05,
        takes: Takes::Now,
    },
    Range {
        via: Via::Cvar("cameraYawMoveSpeed"),
        label: "Mouse look speed",
        min: 90.0,
        max: 270.0,
        step: 10.0,
        takes: Takes::Now,
    },
    Choice {
        cvar: "cameraSmoothStyle",
        label: "Camera following style",
        choices: CAMERA_STYLES,
        reads: camera_style_read,
        takes: Takes::Now,
    },
    Range {
        via: Via::Cvar("cameraYawSmoothSpeed"),
        label: "Camera follow speed",
        min: 90.0,
        max: 270.0,
        step: 10.0,
        takes: Takes::Now,
    },
    Range {
        via: Via::Cvar("cameraDistanceMaxFactor"),
        label: "Camera distance",
        min: 1.0,
        max: 2.0,
        step: 0.1,
        takes: Takes::Now,
    },
    Check {
        cvar: "cameraTerrainTilt",
        label: "Camera follows the ground",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "cameraBobbing",
        label: "Camera bobs with steps",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "cameraWaterCollision",
        label: "Camera stays out of water",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "cameraPivot",
        label: "Camera turns smartly",
        needs: None,
        takes: Takes::Now,
    },
];

const INTERFACE_DISPLAY: &[Setting] = &[
    Check {
        cvar: "UnitNamePlayer",
        label: "Player names",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "UnitNamePlayerGuild",
        label: "Guild names",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "UnitNamePlayerPVPTitle",
        label: "Player titles",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "UnitNameNPC",
        label: "Creature names",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "UnitNameOwn",
        label: "Your own name",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "CombatDamage",
        label: "Show damage numbers",
        needs: None,
        takes: Takes::Now,
    },
    // Smart hides the hero's small hits ([`crate::smart_numbers`])
    Choice {
        cvar: crate::smart_numbers::CVAR,
        label: "Damage numbers",
        choices: DAMAGE_NUMBERS,
        reads: damage_numbers_read,
        takes: Takes::Now,
    },
    Check {
        cvar: "PetMeleeDamage",
        label: "Pet damage numbers",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "ChatBubbles",
        label: "Chat bubbles",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "ChatBubblesParty",
        label: "Party chat bubbles",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "showLootSpam",
        label: "Every roll in the chat",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "guildMemberNotify",
        label: "Guild arrivals in the chat",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "profanityFilter",
        label: "Language filter",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "autoClearAFK",
        label: "Clear away when acting",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "BlockTrades",
        label: "Refuse trades",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: "showGameTips",
        label: "Tips on loading screens",
        needs: None,
        takes: Takes::Now,
    },
    Check {
        cvar: TIPS,
        label: "Tips in the game",
        needs: None,
        takes: Takes::Now,
    },
    // the updater's check at start ([`crate::update`]); on, it checks as soon as it is ticked
    Check {
        cvar: crate::update::CVAR,
        label: "Check for updates",
        needs: None,
        takes: Takes::Now,
    },
];

/// The Timers tab (`crate::boss_alerts`, `crate::boss_bar_places`): the
/// boss warnings and their sound on the left, the timer bars, their drag handles and putting them
/// back on the right.
const TIMERS_WARNINGS: &[Setting] = &[
    Check {
        cvar: crate::boss_alerts::WARNINGS_CVAR,
        label: "Boss warnings",
        needs: None,
        takes: Takes::Now,
    },
    // unticked, the warnings come silent
    Check {
        cvar: crate::boss_alerts::SOUND_CVAR,
        label: "Warning sound",
        needs: Some(crate::boss_alerts::WARNINGS_CVAR),
        takes: Takes::Now,
    },
];

const TIMERS_BARS: &[Setting] = &[
    Check {
        cvar: crate::boss_alerts::BARS_CVAR,
        label: "Boss timer bars",
        needs: None,
        takes: Takes::Now,
    },
    // each group's drag handle always shows; unticked, it shows only while Shift is held
    Check {
        cvar: crate::boss_alerts::HANDLES_CVAR,
        label: "Show bar handles",
        needs: Some(crate::boss_alerts::BARS_CVAR),
        takes: Takes::Now,
    },
    // ticked, both groups of bars go back where they start (and the box unticks itself)
    Check {
        cvar: crate::boss_bar_places::RESET_CVAR,
        label: "Reset bar positions",
        needs: Some(crate::boss_alerts::BARS_CVAR),
        takes: Takes::Now,
    },
];

/// A tab's two columns.
fn columns(tab: OptionsTab) -> [&'static [Setting]; 2] {
    match tab {
        OptionsTab::Video => [VIDEO_DISPLAY, VIDEO_WORLD],
        OptionsTab::Sound => [&SOUND[..7], &SOUND[7..]],
        OptionsTab::Interface => [INTERFACE_CONTROLS, INTERFACE_DISPLAY],
        OptionsTab::Timers => [TIMERS_WARNINGS, TIMERS_BARS],
        // export and import: no setting ([`super::save_files`])
        OptionsTab::Saves => [&[], &[]],
    }
}

fn all(tab: OptionsTab) -> impl Iterator<Item = &'static Setting> {
    columns(tab).into_iter().flatten()
}

/// The stock frame's anisotropy stops (`ANISOTROPIC_VALUES`), the slider's 1 to 4 as the CVar's.
const ANISOTROPY: [&str; 5] = ["1", "2", "4", "8", "16"];

/// What the panel shows and is changing.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionsDraft {
    pub tab: OptionsTab,
    /// Each setting's value as the panel holds it (a box "1" or "0", a range its number, the
    /// resolution and multisample their 1-based index), by key.
    values: Vec<(&'static str, String)>,
    /// The values when the panel opened, for Cancel and to see what changed.
    opened: Vec<(&'static str, String)>,
    resolutions: Vec<String>,
    multisamples: Vec<String>,
    /// What the device has: triple buffering, the anisotropy stops it reaches.
    triple_buffering: bool,
    anisotropy_top: f32,
    /// A line said after Okay: what needs a restart.
    pub notice: Option<String>,
    /// The Saves tab: export and import.
    pub saves: super::save_files::SavesPage,
    /// The panel is open over a paused run (from its Escape menu), not on the warband screen.
    pub over_run: bool,
    /// Over a run: the stock key binding window is up, opened from here; the panel waits under
    /// it, hidden, and comes back when it closes.
    pub key_bindings: bool,
}

impl OptionsDraft {
    fn get(&self, key: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
    }

    fn put(&mut self, key: &'static str, value: String) {
        match self.values.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => *v = value,
            None => self.values.push((key, value)),
        }
    }

    fn ticked(&self, cvar: &str) -> bool {
        self.get(cvar)
            .is_some_and(|v| v.trim() != "0" && !v.is_empty())
    }

    fn number(&self, key: &str) -> f32 {
        self.get(key)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0.0)
    }

    fn changed(&self, key: &str) -> bool {
        let before = self
            .opened
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str());
        before != self.get(key)
    }
}

/// A Lua string literal of `s`.
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Reads every setting from the Lua VM, as the stock frames' `_Load` do.
pub(super) fn read(script: &UiScript, tab: OptionsTab) -> OptionsDraft {
    // our own settings are registered as an addon's are, so they read and keep
    crate::boss_alerts::register_cvars(script);
    crate::smart_numbers::register_cvar(script);
    crate::update::register_cvar(script);
    let text = |chunk: &str| script.eval::<Option<String>>(chunk).ok().flatten();
    let list = |call: &str| -> Vec<String> {
        text(&format!(
            "local t = {{ {call} }} local s = '' for i = 1, getn(t) do s = s .. tostring(t[i]) .. ';' end return s"
        ))
        .unwrap_or_default()
        .split(';')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
    };
    let resolutions = list("GetScreenResolutions()");
    let flat = list("GetMultisampleFormats()");
    let multisamples: Vec<String> = flat
        .chunks(3)
        .filter(|c| c.len() == 3)
        .map(|c| format!("{}-bit colour, {}-bit depth, {}x", c[0], c[1], c[2]))
        .collect();
    let caps = list(
        "(function() local a, b, c, d, e, f, g = GetVideoCaps() return a or 0, b or 0, c or 0, d or 0, e or 0, f or 0, g or 0 end)()",
    );
    let cap = |i: usize| {
        caps.get(i)
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.0)
    };
    let max_aniso = cap(5);
    let anisotropy_top = ANISOTROPY
        .iter()
        .position(|v| v.parse::<f32>().ok() == Some(max_aniso))
        .map_or(4.0, |i| (i + 1) as f32);
    let mut draft = OptionsDraft {
        tab,
        values: Vec::new(),
        opened: Vec::new(),
        resolutions,
        multisamples,
        triple_buffering: cap(4) > 0.0,
        anisotropy_top,
        notice: None,
        saves: Default::default(),
        over_run: false,
        key_bindings: false,
    };
    for tab in [OptionsTab::Video, OptionsTab::Sound, OptionsTab::Interface, OptionsTab::Timers] {
        for s in all(tab) {
            let value = match s {
                Check { cvar: TIPS, .. } => text("return TutorialsEnabled() and '1' or '0'"),
                Check { cvar, .. } => text(&format!("return GetCVar({})", quoted(cvar))),
                Choice { cvar, reads, .. } => text(&format!("return GetCVar({})", quoted(cvar))).map(|v| reads(&v).to_owned()),
                Range { via, .. } => match via {
                    Via::Cvar(c) => text(&format!("return GetCVar({})", quoted(c))),
                    Via::Calls(get, _) => text(&format!("return tostring({get}())")),
                    Via::Anisotropy => text("return GetCVar(\"anisotropic\")").map(|v| {
                        ANISOTROPY
                            .iter()
                            .position(|a| *a == v.trim())
                            .map_or(1, |i| i + 1)
                            .to_string()
                    }),
                },
                Setting::Resolution => text("return tostring(GetCurrentResolution())"),
                Setting::Multisample => text("return tostring(GetCurrentMultisampleFormat())"),
            };
            draft.put(s.key(), value.unwrap_or_else(|| "0".into()));
        }
    }
    draft.opened = draft.values.clone();
    draft
}

/// A setting found by its key.
fn find(key: &str) -> Option<&'static Setting> {
    [OptionsTab::Video, OptionsTab::Sound, OptionsTab::Interface, OptionsTab::Timers]
        .into_iter()
        .flat_map(all)
        .find(|s| s.key() == key)
}

/// A setting's key by its name (a dev script's).
pub(super) fn key_of(name: &str) -> Option<&'static str> {
    find(name).map(|s| s.key())
}

/// Whether a setting may be changed now: its box ticked where it needs one, and the device able.
fn open(draft: &OptionsDraft, s: &Setting) -> bool {
    match s {
        Check { cvar: TIPS, .. } => draft.over_run,
        Check { cvar, needs, .. } => {
            (*cvar != "gxTripleBuffer" || draft.triple_buffering)
                && needs.is_none_or(|n| draft.ticked(n))
        }
        Range { via, .. } => match via {
            Via::Cvar("uiScale") => draft.ticked("useUiScale"),
            Via::Calls("GetGamma", _) => !draft.ticked("desktopGamma") && !draft.ticked("gxWindow"),
            _ => true,
        },
        Choice { .. } => true,
        Setting::Resolution => !draft.resolutions.is_empty(),
        Setting::Multisample => !draft.multisamples.is_empty(),
    }
}

/// A change made in the panel: a box flipped, a value stepped. Sound settings go to the game at
/// once, as the stock Sound frame's do.
pub(super) fn change(script: Option<&UiScript>, draft: &mut OptionsDraft, key: &str, up: bool) {
    let Some(s) = find(key) else {
        return;
    };
    if !open(draft, s) {
        return;
    }
    let key = s.key();
    let value = match s {
        Check { cvar, .. } => {
            if draft.ticked(cvar) {
                "0".to_owned()
            } else {
                "1".to_owned()
            }
        }
        Range {
            via,
            min,
            max,
            step,
            ..
        } => {
            let top = if *via == Via::Anisotropy {
                draft.anisotropy_top
            } else {
                *max
            };
            let v = draft.number(key);
            let next = if up { v + step } else { v - step };
            // kept on the range's own steps
            let snapped = (((next - min) / step).round() * step + min).clamp(*min, top);
            trim(snapped)
        }
        Choice { choices, .. } => {
            let n = choices.len();
            let at = choices.iter().position(|(v, _)| Some(*v) == draft.get(key)).unwrap_or(0);
            let next = if up { (at + 1) % n } else { (at + n - 1) % n };
            choices[next].0.to_owned()
        }
        Setting::Resolution | Setting::Multisample => {
            let n = if *s == Setting::Resolution {
                draft.resolutions.len()
            } else {
                draft.multisamples.len()
            };
            let at = draft.number(key) as usize;
            let next = if up { at % n + 1 } else { (at + n - 2) % n + 1 };
            next.to_string()
        }
    };
    draft.put(key, value.clone());
    if draft.tab == OptionsTab::Sound {
        if let (Some(script), Some(cvar)) = (script, cvar_of(s)) {
            let _ = script.run(&format!("SetCVar({}, {})", quoted(cvar), quoted(&value)));
        }
    }
}

/// A number as the stock sliders hand it to `SetCVar`: no trailing zeros.
fn trim(v: f32) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

impl PartialEq for Setting {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

fn cvar_of(s: &Setting) -> Option<&'static str> {
    match s {
        Check { cvar, .. } | Choice { cvar, .. } => Some(cvar),
        Range {
            via: Via::Cvar(c), ..
        } => Some(c),
        _ => None,
    }
}

/// Okay: every changed setting of every tab written as the stock frames' Okay writes it, the
/// display rebuilt when a display setting moved; what needs a restart is said.
pub(super) fn apply(script: &UiScript, draft: &mut OptionsDraft) {
    let mut chunk = String::new();
    let mut display = false;
    let mut later: Vec<&'static str> = Vec::new();
    for tab in [OptionsTab::Video, OptionsTab::Interface, OptionsTab::Timers] {
        for s in all(tab) {
            let key = s.key();
            if !draft.changed(key) {
                continue;
            }
            let value = draft.get(key).unwrap_or("0").to_owned();
            let takes = match s {
                Check { cvar: TIPS, takes, .. } => {
                    chunk += if value == "1" { "ResetTutorials()\n" } else { "ClearTutorials()\n" };
                    *takes
                }
                Choice { cvar, takes, .. } => {
                    chunk += &format!("SetCVar({}, {})
", quoted(cvar), quoted(&value));
                    *takes
                }
                Check { cvar, takes, .. } => {
                    chunk += &format!("SetCVar({}, {})\n", quoted(cvar), quoted(&value));
                    // the stock Okay copies All Shaders into `ffx`
                    if *cvar == "pixelShaders" {
                        chunk += &format!("SetCVar(\"ffx\", {})\n", quoted(&value));
                    }
                    *takes
                }
                Range { via, takes, .. } => {
                    match via {
                        Via::Cvar(c) => {
                            chunk += &format!("SetCVar({}, {})\n", quoted(c), quoted(&value))
                        }
                        Via::Calls(_, set) => chunk += &format!("{set}({value})\n"),
                        Via::Anisotropy => {
                            let i = (value.parse::<f32>().unwrap_or(1.0) as usize).clamp(1, 5);
                            chunk +=
                                &format!("SetCVar(\"anisotropic\", \"{}\")\n", ANISOTROPY[i - 1]);
                        }
                    }
                    *takes
                }
                Setting::Resolution => {
                    chunk += &format!("SetScreenResolution({value})\n");
                    Takes::Display
                }
                Setting::Multisample => {
                    chunk += &format!("SetMultisampleFormat({value})\n");
                    Takes::Restart
                }
            };
            match takes {
                Takes::Display => display = true,
                Takes::Restart => later.push("when the game starts again"),
                // over a run the world is loaded already: it takes the change the next time
                Takes::World if draft.over_run => {
                    later.push("when a hero next enters the world (End Run, then a new run)")
                }
                Takes::World => later.push("when a hero next enters the world"),
                Takes::Now => {}
            }
        }
    }
    if display {
        chunk += "RestartGx()\n";
    }
    if !chunk.is_empty() {
        if let Err(e) = script.run(&chunk) {
            warn!("menus: the options did not all apply: {e}");
        }
    }
    later.sort_unstable();
    later.dedup();
    draft.notice =
        (!later.is_empty()).then(|| format!("Some changes take hold {}.", later.join(" or ")));
    draft.opened = draft.values.clone();
}

/// Cancel: the sound settings back as they were (the others were never written).
pub(super) fn cancel(script: Option<&UiScript>, draft: &OptionsDraft) {
    let Some(script) = script else {
        return;
    };
    let mut chunk = String::new();
    for s in all(OptionsTab::Sound) {
        if let (Some(cvar), true) = (cvar_of(s), draft.changed(s.key())) {
            let before = draft
                .opened
                .iter()
                .find(|(k, _)| *k == s.key())
                .map_or("0", |(_, v)| v.as_str());
            chunk += &format!("SetCVar({}, {})\n", quoted(cvar), quoted(before));
        }
    }
    if !chunk.is_empty() {
        let _ = script.run(&chunk);
    }
}

/// How a value shows: a stop's word where the stock slider has stops, else the number.
fn shown(s: &Setting, draft: &OptionsDraft) -> String {
    let v = draft.number(s.key());
    match s {
        Range { via, label, .. } => match (via, *label) {
            (Via::Anisotropy, _) => format!("{}x", ANISOTROPY[(v as usize).clamp(1, 5) - 1]),
            (Via::Calls("GetWorldDetail", _), _) | (Via::Cvar("spellEffectLevel"), _) => {
                ["Low", "Medium", "High"][(v as usize).min(2)].to_owned()
            }
            (Via::Cvar("weatherDensity"), _) => {
                ["Off", "Low", "Medium", "High"][(v as usize).min(3)].to_owned()
            }
            (Via::Calls("GetTerrainMip", _), _) | (Via::Calls("GetBaseMip", _), _) => {
                ["Low", "High"][(v as usize).min(1)].to_owned()
            }
            (Via::Cvar(c), _) if c.ends_with("Volume") => format!("{}%", (v * 100.0).round()),
            (Via::Cvar("farclip"), _) => format!("{} yards", v.round()),
            _ => trim(v),
        },
        Setting::Resolution => draft
            .resolutions
            .get((v as usize).saturating_sub(1))
            .cloned()
            .unwrap_or_else(|| "As it is".into()),
        Setting::Multisample => draft
            .multisamples
            .get((v as usize).saturating_sub(1))
            .cloned()
            .unwrap_or_else(|| "As it is".into()),
        Choice { choices, reads, .. } => {
            let v = reads(draft.get(s.key()).unwrap_or_default());
            choices.iter().find(|(c, _)| *c == v).map_or(v, |(_, l)| *l).to_owned()
        }
        Check { .. } => String::new(),
    }
}

/// What a setting's line says about when it takes hold; over a run, the world is loaded already.
fn takes_note(s: &Setting, over_run: bool) -> Option<&'static str> {
    let takes = match s {
        Check { takes, .. } | Range { takes, .. } | Choice { takes, .. } => *takes,
        Setting::Resolution => Takes::Now,
        Setting::Multisample => Takes::Restart,
    };
    match takes {
        Takes::Restart => Some("after a restart"),
        Takes::World if over_run => Some("next time in the world"),
        Takes::World => Some("in the world"),
        _ => None,
    }
}

/// A setting's label.
fn label(s: &Setting) -> &'static str {
    match s {
        Check { label, .. } | Range { label, .. } | Choice { label, .. } => label,
        Setting::Resolution => "Resolution",
        Setting::Multisample => "Multisampling",
    }
}

/// The panel.
pub(super) fn draw(commands: &mut Commands, root: Entity, look: &MenuLook, draft: &OptionsDraft) {
    commands.entity(root).with_children(|r| {
        r.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            solid(),
            GlobalZIndex(super::MENU_LAYER + 10),
        ))
        .with_children(|veil| {
            let mut node = look.panel_node();
            node.width = look.px(860.0);
            node.row_gap = look.px(8.0);
            node.padding = UiRect::all(look.px(16.0));
            veil.spawn(look.panel(node)).with_children(|p| {
                look.heading(p, "Options", 24.0);
                p.spawn(Node {
                    justify_content: JustifyContent::Center,
                    column_gap: look.px(6.0),
                    ..default()
                })
                .with_children(|tabs| {
                    for (tab, name) in [
                        (OptionsTab::Video, "Video"),
                        (OptionsTab::Sound, "Sound"),
                        (OptionsTab::Interface, "Interface"),
                        (OptionsTab::Timers, "Timers"),
                        (OptionsTab::Saves, "Saves"),
                    ] {
                        let style = if tab == draft.tab {
                            ButtonStyle::Active
                        } else {
                            ButtonStyle::Text
                        };
                        look.button(tabs, name, style, true, Some(120.0))
                            .insert(MenuAction::OptionsTab(tab));
                    }
                });
                if draft.tab == OptionsTab::Saves {
                    super::save_files::draw_page(p, look, &draft.saves);
                }
                p.spawn(Node {
                    column_gap: look.px(24.0),
                    align_items: AlignItems::FlexStart,
                    ..default()
                })
                .with_children(|cols| {
                    for column in columns(draft.tab) {
                        cols.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            flex_basis: Val::Px(0.0),
                            row_gap: look.px(3.0),
                            ..default()
                        })
                        .with_children(|col| {
                            for s in column {
                                row(col, look, draft, s);
                            }
                        });
                    }
                });
                let hint = match draft.tab {
                    OptionsTab::Video => "Video changes apply on Okay; display changes rebuild the window.",
                    OptionsTab::Sound => "Sound changes are heard at once; Cancel puts them back.",
                    OptionsTab::Interface => {
                        "Interface changes apply on Okay. The interface's own settings (tooltips, bars, combat text) are set in the game."
                    }
                    OptionsTab::Timers => "Boss warnings and timer bars. Changes apply on Okay; with the handles hidden, hold Shift to move the bars.",
                    OptionsTab::Saves if draft.over_run => {
                        "An export is done at once; Okay and Cancel are for the other tabs."
                    }
                    OptionsTab::Saves => {
                        "An export or an import is done at once; Okay and Cancel are for the other tabs."
                    }
                };
                p.spawn(look.hint(hint));
                p.spawn(look.hint(if draft.over_run {
                    "Key Bindings opens the game's own key binding window; closing it comes back here."
                } else {
                    "Key bindings are set during a run: Escape, then Options, then Key Bindings."
                }));
                if let Some(n) = &draft.notice {
                    p.spawn(look.text(Face::Plain, 11.0, palette::HEADING, n.clone()));
                }
                p.spawn(Node {
                    justify_content: JustifyContent::Center,
                    column_gap: look.px(12.0),
                    ..default()
                })
                .with_children(|b| {
                    if draft.over_run {
                        look.button(b, "Key Bindings", ButtonStyle::Text, true, Some(140.0))
                            .insert(MenuAction::KeyBindings);
                    }
                    look.button(b, "Okay", ButtonStyle::Primary, true, Some(140.0))
                        .insert(MenuAction::OptionsOkay);
                    look.button(b, "Cancel", ButtonStyle::Text, true, Some(140.0))
                        .insert(MenuAction::OptionsCancel);
                });
            });
        });
    });
}

/// One setting's line: a box with its label, or its label, its value and the arrows.
fn row(col: &mut ChildSpawnerCommands, look: &MenuLook, draft: &OptionsDraft, s: &Setting) {
    let can = open(draft, s);
    let text_colour = if can { palette::VALUE } else { palette::HINT };
    let note = takes_note(s, draft.over_run);
    match s {
        Check { cvar, .. } => {
            let on = draft.ticked(cvar);
            let mut line = look.frame_button(col, super::look::ButtonStyle::Row, can, None);
            line.insert(MenuAction::OptionsChange {
                key: s.key(),
                up: true,
            });
            line.with_children(|b| {
                b.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        column_gap: look.px(8.0),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|l| {
                    l.spawn((
                        Node {
                            width: look.px(12.0),
                            height: look.px(12.0),
                            border: UiRect::all(look.px(1.0)),
                            border_radius: BorderRadius::all(look.px(2.0)),
                            padding: UiRect::all(look.px(2.0)),
                            ..default()
                        },
                        BorderColor::all(if can {
                            palette::GOLD
                        } else {
                            palette::GOLD_DIM
                        }),
                    ))
                    .with_children(|box_| {
                        if on {
                            box_.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    border_radius: BorderRadius::all(look.px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(if can {
                                    palette::HEADING
                                } else {
                                    palette::LOCKED
                                }),
                            ));
                        }
                    });
                    l.spawn(look.text(Face::Plain, 10.5, text_colour, label(s)));
                    if let Some(n) = note {
                        l.spawn(look.text(Face::Plain, 8.5, palette::HINT, format!("({n})")));
                    }
                });
            });
        }
        _ => {
            col.spawn((
                Node {
                    align_items: AlignItems::Center,
                    column_gap: look.px(6.0),
                    padding: UiRect::axes(look.px(8.0), look.px(2.0)),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|l| {
                let mut name = l.spawn(Node {
                    flex_grow: 1.0,
                    column_gap: look.px(6.0),
                    align_items: AlignItems::Center,
                    ..default()
                });
                // a row with a tooltip shows it while the pointer is over its label
                if let Some(lines) = tip_of(s) {
                    name.insert((row_tip(lines), MenuLookTip));
                }
                name.with_children(|n| {
                    n.spawn((look.text(Face::Plain, 10.5, text_colour, label(s)), Pickable::IGNORE));
                    if let Some(note) = note {
                        n.spawn((
                            look.text(Face::Plain, 8.5, palette::HINT, format!("({note})")),
                            Pickable::IGNORE,
                        ));
                    }
                });
                look.arrow(l, Arrow::Left, 20.0, can)
                    .insert(MenuAction::OptionsChange {
                        key: s.key(),
                        up: false,
                    });
                l.spawn(Node {
                    min_width: look.px(150.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|v| {
                    v.spawn(look.text(
                        Face::Plain,
                        10.5,
                        if can { palette::HEADING } else { palette::HINT },
                        shown(s, draft),
                    ));
                });
                look.arrow(l, Arrow::Right, 20.0, can)
                    .insert(MenuAction::OptionsChange {
                        key: s.key(),
                        up: true,
                    });
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> OptionsDraft {
        let mut d = OptionsDraft {
            tab: OptionsTab::Video,
            values: Vec::new(),
            opened: Vec::new(),
            resolutions: vec!["1280x720".into(), "1600x900".into()],
            multisamples: vec![],
            triple_buffering: false,
            anisotropy_top: 3.0,
            notice: None,
            saves: Default::default(),
            over_run: false,
            key_bindings: false,
        };
        d.put("farclip", "417".into());
        d.put("gxWindow", "1".into());
        d.put("anisotropic", "3".into());
        d.put("@resolution", "2".into());
        d.opened = d.values.clone();
        d
    }

    #[test]
    fn a_range_steps_on_the_stock_steps_and_stays_in_it() {
        let mut d = draft();
        change(None, &mut d, "farclip", true);
        assert_eq!(d.get("farclip"), Some("477"));
        for _ in 0..20 {
            change(None, &mut d, "farclip", true);
        }
        assert_eq!(d.get("farclip"), Some("777"));
        change(None, &mut d, "anisotropic", true);
        assert_eq!(d.get("anisotropic"), Some("3"), "the device reaches 4x");
        change(None, &mut d, "@resolution", true);
        assert_eq!(d.get("@resolution"), Some("1"), "round the list");
    }

    /// Camera Following Style steps round the three values the stock dropdown writes and the
    /// camera does (1 Smart, 2 Always, 0 Never), and shows a value as the camera reads it: 3 as
    /// Never, anything off the ladder as Smart.
    #[test]
    fn the_camera_following_style_steps_round_the_camera_s_three_styles() {
        let values: Vec<&str> = CAMERA_STYLES.iter().map(|(v, _)| *v).collect();
        assert_eq!(values, ["1", "2", "0"]);
        let reads = ["0", "1", "2", "3", "7", "", "2.0"].map(camera_style_read);
        assert_eq!(reads, ["0", "1", "2", "0", "1", "1", "2"]);
        let s = find("cameraSmoothStyle").unwrap();
        let mut d = draft();
        d.put("cameraSmoothStyle", camera_style_read("3").into());
        d.opened = d.values.clone();
        assert_eq!(shown(s, &d), "Never adjust camera");
        change(None, &mut d, "cameraSmoothStyle", true);
        assert_eq!((d.get("cameraSmoothStyle"), shown(s, &d).as_str()), (Some("1"), "Only when moving"), "round the list");
        change(None, &mut d, "cameraSmoothStyle", true);
        assert_eq!(d.get("cameraSmoothStyle"), Some("2"));
        change(None, &mut d, "cameraSmoothStyle", false);
        change(None, &mut d, "cameraSmoothStyle", false);
        assert_eq!(d.get("cameraSmoothStyle"), Some("0"));
        assert!(!d.changed("cameraSmoothStyle"), "back where it opened: Okay writes nothing");
        assert_eq!(cvar_of(s), Some("cameraSmoothStyle"));
    }

    /// Damage numbers: Smart by default, stepped to All, written on
    /// Okay as the CVar the host keeps, and read back from a saved value; the row has its tooltip.
    #[test]
    fn damage_numbers_default_to_smart_and_are_saved() {
        use crate::smart_numbers::CVAR;
        let mut script = UiScript::new().unwrap();
        let mut d = read(&script, OptionsTab::Interface);
        let s = find(CVAR).unwrap();
        assert_eq!((d.get(CVAR), shown(s, &d).as_str()), (Some("1"), "Smart"), "the default");
        change(None, &mut d, CVAR, true);
        assert_eq!((d.get(CVAR), shown(s, &d).as_str()), (Some("0"), "All"));
        let _ = script.take_cvar_changes();
        apply(&script, &mut d);
        assert_eq!(script.cvar(CVAR).as_deref(), Some("0"));
        assert!(
            script.take_cvar_changes().iter().any(|(k, v)| k.eq_ignore_ascii_case(CVAR) && v == "0"),
            "the change goes to the host, which saves it"
        );
        // the next start: the saved value outranks the default
        let mut again = UiScript::new().unwrap();
        again.set_cvar_saved_base([(CVAR.to_ascii_lowercase(), "0".to_string())]);
        let d = read(&again, OptionsTab::Interface);
        assert_eq!(shown(s, &d), "All");
        assert_eq!(damage_numbers_read("7"), "1", "an unknown value reads as Smart");
        let tip = row_tip(tip_of(s).unwrap());
        assert_eq!(tip.0[0].text, "Damage numbers");
        assert!(tip.0[1].text.contains("60%") && tip.0[1].text.contains("every 30 seconds") && tip.0[1].text.contains("over 50%"));
    }

    /// Check for updates: on by default, a box on the Interface tab, written on Okay as the CVar
    /// the host keeps; the row has its tooltip.
    #[test]
    fn check_for_updates_defaults_to_on_and_is_saved() {
        use crate::update::CVAR;
        let mut script = UiScript::new().unwrap();
        let mut d = read(&script, OptionsTab::Interface);
        let s = find(CVAR).unwrap();
        assert!(all(OptionsTab::Interface).any(|x| x.key() == CVAR));
        assert!(d.ticked(CVAR), "on by default");
        change(None, &mut d, CVAR, true);
        assert!(!d.ticked(CVAR));
        let _ = script.take_cvar_changes();
        apply(&script, &mut d);
        assert_eq!(script.cvar(CVAR).as_deref(), Some("0"));
        assert!(script.take_cvar_changes().iter().any(|(k, v)| k.eq_ignore_ascii_case(CVAR) && v == "0"));
        assert!(!crate::update::on_from_value(script.cvar(CVAR).as_deref()));
        let tip = row_tip(tip_of(s).unwrap());
        assert_eq!(tip.0[0].text, "Check for updates");
        assert!(tip.0.iter().any(|l| l.text.contains("Nothing about you is sent")));
    }

    #[test]
    fn a_box_needs_the_box_it_hangs_on() {
        let mut d = draft();
        change(None, &mut d, "gxMaximize", true);
        assert_eq!(d.get("gxMaximize"), Some("1"), "windowed is ticked");
        change(None, &mut d, "gxTripleBuffer", true);
        assert_eq!(
            d.get("gxTripleBuffer"),
            None,
            "the device has no triple buffering"
        );
        assert!(d.changed("gxMaximize") && !d.changed("farclip"));
    }
}
