//! The run's two minimap buttons, round on the stock Minimap's rim the
//! way an addon's minimap icons sit: the boss timer bars' button flips their drag handles
//! ([`crate::boss_alerts::HANDLES_CVAR`], the same setting as Options, Timers, "Show bar
//! handles"), and the damage meter's button shows or hides the meter window
//! ([`crate::meter::ToggleMeter`], kept in `meter.ron`). Each has a stock tooltip.
//!
//! The buttons are stock frames, ours, in the interface VM ([`LUA`], `minimap_buttons.lua`), so
//! the stock mouse handling clicks them as it clicks the minimap's own buttons. A click is queued
//! in the Lua and read here every frame: the handles' CVar the Lua has just set is copied into
//! [`AlertOptions`] at once (its own read is only once a second), and a meter click becomes a
//! [`ToggleMeter`]; the meter's state goes back into the Lua for the tooltip.
//!
//! `ESURV_MINIMAP_SHOTS=<stem>`, a development aid: once the hero has been in the world a while, it
//! shows each button's tooltip by running its own hover code, clicks it twice through the
//! interface's own pointer calls, the ones the mouse feed makes (no input of the operating system
//! is touched), and writes the game's own shots of each step: `<stem>-<i>-<step>.png`.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use benilla_app::Objects;
use benilla_ui::script::UiScript;

use crate::boss_alerts::{AlertOptions, HANDLES_CVAR};
use crate::meter::{MeterSettings, MeterSystems, ToggleMeter};
use crate::ui::in_world;

/// The Lua half: the two buttons, their tooltips and the click queue. Returns true once installed.
pub const LUA: &str = include_str!("minimap_buttons.lua");

pub fn register(app: &mut App) {
    app.add_systems(Update, keep_the_buttons.before(MeterSystems));
    shots::register(app);
}

/// What one of the buttons asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    Bars,
    Meter,
}

/// The Lua's queue, read: `"bars,meter"`.
pub fn clicks(queue: &str) -> Vec<Click> {
    queue
        .split(',')
        .filter_map(|k| match k.trim() {
            "bars" => Some(Click::Bars),
            "meter" => Some(Click::Meter),
            _ => None,
        })
        .collect()
}

/// The handles' option from the CVar's value (`None`: not set, on), as [`AlertOptions`] reads it.
pub fn handles_from(value: Option<&str>) -> bool {
    AlertOptions::from_values(None, None, None, value).handles
}

/// What this VM has been told.
#[derive(Default)]
struct Told {
    session: u64,
    installed: bool,
    /// The meter's shown state it last had.
    meter_shown: Option<bool>,
}

/// Installs the Lua in a fresh VM once the hero is in the world, takes the buttons' clicks, and
/// tells the Lua the meter's state.
fn keep_the_buttons(
    script: Option<NonSend<UiScript>>,
    objects: Objects,
    settings: Res<MeterSettings>,
    mut options: Option<ResMut<AlertOptions>>,
    mut toggles: MessageWriter<ToggleMeter>,
    mut told: Local<Told>,
    mut warned: Local<bool>,
) {
    let Some(script) = script else { return };
    if told.session != script.session() {
        *told = Told { session: script.session(), ..default() };
    }
    if !told.installed {
        if !in_world(&objects) {
            return;
        }
        match script.eval::<bool>(LUA) {
            Ok(true) => {
                info!("minimap buttons: installed on the minimap");
                told.installed = true;
            }
            Ok(false) => return,
            Err(e) => {
                if !std::mem::replace(&mut *warned, true) {
                    warn!("minimap buttons: the Lua did not load: {e}");
                }
                return;
            }
        }
    }
    let queue = script
        .eval::<String>("return ESURV_MINIMAP_BUTTONS.take()")
        .unwrap_or_default();
    let mut meter_shown = !settings.hidden;
    for click in clicks(&queue) {
        match click {
            Click::Bars => {
                let value = script
                    .eval::<Option<String>>(&format!("return GetCVar(\"{HANDLES_CVAR}\")"))
                    .ok()
                    .flatten();
                let handles = handles_from(value.as_deref());
                info!("minimap buttons: the boss bars' handles {}", if handles { "show" } else { "hide" });
                if let Some(options) = options.as_mut() {
                    if options.handles != handles {
                        options.handles = handles;
                    }
                }
            }
            Click::Meter => {
                meter_shown = !meter_shown;
                info!("minimap buttons: the meter {}", if meter_shown { "shows" } else { "hides" });
                toggles.write(ToggleMeter);
            }
        }
    }
    if told.meter_shown != Some(meter_shown) {
        let _ = script.run(&format!("ESURV_MINIMAP_BUTTONS.set_meter({meter_shown})"));
        told.meter_shown = Some(meter_shown);
    }
}

/// The unattended check (`ESURV_MINIMAP_SHOTS`).
mod shots {
    use super::*;

    const SWITCH: &str = "ESURV_MINIMAP_SHOTS";
    /// Real seconds in the world before the first step, so the interface has settled.
    const START_AFTER: f32 = 12.0;

    /// What a step does, with the button's key or the shot's name.
    #[derive(Clone, Copy)]
    enum Act {
        Shot(&'static str),
        Point(&'static str),
        Click(&'static str),
    }

    /// The steps, each at real seconds after the start.
    const STEPS: &[(f32, Act)] = &[
        (0.0, Act::Shot("buttons")),
        (1.0, Act::Point("bars")),
        (2.0, Act::Shot("bars-tip")),
        (2.5, Act::Click("bars")),
        (4.0, Act::Shot("bars-handles-flipped")),
        (4.5, Act::Click("bars")),
        (6.0, Act::Shot("bars-handles-back")),
        (6.5, Act::Point("meter")),
        (7.5, Act::Shot("meter-tip")),
        (8.0, Act::Click("meter")),
        (9.5, Act::Shot("meter-hidden")),
        (10.0, Act::Click("meter")),
        (11.5, Act::Shot("meter-shown")),
    ];

    #[derive(Resource)]
    struct Check {
        stem: String,
        start: Option<f32>,
        next: usize,
        shots: u32,
    }

    pub(super) fn register(app: &mut App) {
        if let Ok(stem) = std::env::var(SWITCH) {
            info!("minimap buttons: the unattended check is on ({SWITCH})");
            let stem = stem.strip_suffix(".png").unwrap_or(&stem).to_owned();
            app.insert_resource(Check { stem, start: None, next: 0, shots: 0 })
                .add_systems(Update, run_the_check.after(keep_the_buttons));
        }
    }

    /// Where a button's middle is, in the root's units.
    fn aim(script: &UiScript, key: &str) -> Option<(f32, f32)> {
        let (x, y) = script
            .eval::<(Option<f32>, Option<f32>)>(&format!("return ESURV_MINIMAP_BUTTONS.where('{key}')"))
            .ok()?;
        Some((x?, y?))
    }

    /// Takes the next step when its time has come: the pointer and the button go through the
    /// interface's own pointer calls, as the mouse feed makes them.
    fn run_the_check(
        mut commands: Commands,
        script: Option<NonSendMut<UiScript>>,
        objects: Objects,
        time: Res<Time<Real>>,
        mut check: ResMut<Check>,
    ) {
        let Some(mut script) = script else { return };
        if !in_world(&objects) || check.next >= STEPS.len() {
            return;
        }
        let now = time.elapsed_secs();
        let start = *check.start.get_or_insert(now + START_AFTER);
        let (at, act) = STEPS[check.next];
        if now < start + at {
            return;
        }
        check.next += 1;
        match act {
            Act::Shot(name) => {
                check.shots += 1;
                let path = format!("{}-{}-{name}.png", check.stem, check.shots);
                info!("minimap buttons: shot {path}");
                commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            }
            Act::Point(key) => match aim(&script, key) {
                Some((x, y)) => {
                    // the button's own hover code, as the pointer coming onto it runs it, so a
                    // real pointer resting elsewhere on the window does not take it away again
                    info!("minimap buttons: the pointer comes onto the {key} button at {x:.0},{y:.0}");
                    let chunk = format!(
                        "local b = ESURV_MINIMAP_BUTTONS.{key}; this = b; b:GetScript('OnEnter')(); \
                         if b.LockHighlight then b:LockHighlight() end"
                    );
                    if let Err(e) = script.run(&chunk) {
                        warn!("minimap buttons: the {key} button's hover: {e}");
                    }
                    let tip = script
                        .eval::<Option<String>>("return GameTooltip:IsShown() and GameTooltipTextLeft1 and GameTooltipTextLeft1:GetText()")
                        .ok()
                        .flatten();
                    info!("minimap buttons: the tooltip says {tip:?}");
                }
                None => warn!("minimap buttons: no {key} button to point at"),
            },
            Act::Click(key) => match aim(&script, key) {
                Some((x, y)) => {
                    info!("minimap buttons: the {key} button is clicked");
                    let _ = script.run(
                        "for _, b in ipairs({ ESURV_MINIMAP_BUTTONS.bars, ESURV_MINIMAP_BUTTONS.meter }) do \
                         if b.UnlockHighlight then b:UnlockHighlight() end end",
                    );
                    script.mouse_move(x, y);
                    script.mouse_button(x, y, "LeftButton", true);
                    script.mouse_button(x, y, "LeftButton", false);
                }
                None => warn!("minimap buttons: no {key} button to click"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_queue_reads_back() {
        assert_eq!(clicks(""), vec![]);
        assert_eq!(clicks("bars,meter,bars"), vec![Click::Bars, Click::Meter, Click::Bars]);
        assert_eq!(clicks("bars,what"), vec![Click::Bars]);
        assert!(handles_from(None), "not set: on");
        assert!(handles_from(Some("1")));
        assert!(!handles_from(Some("0")));
    }

    /// Just enough of the stock interface: the Minimap, frames that keep their scripts, the
    /// tooltip and the CVars.
    const STAND_INS: &str = r#"
        CVARS = {}
        function RegisterCVar(n, v) if CVARS[n] == nil then CVARS[n] = v end end
        function GetCVar(n) return CVARS[n] end
        function SetCVar(n, v) CVARS[n] = v end
        local function region()
            local r = {}
            setmetatable(r, { __index = function() return function() end end })
            return r
        end
        local function frame(name)
            local f = region()
            f.scripts = {}
            f.SetScript = function(self, s, fn) self.scripts[s] = fn end
            f.CreateTexture = function() return region() end
            f.GetCenter = function() return 100, 600 end
            f.GetEffectiveScale = function() return 1 end
            f.GetFrameLevel = function() return 1 end
            if name then _G[name] = f end
            return f
        end
        Minimap = frame("Minimap")
        UIParent = frame("UIParent")
        function CreateFrame(kind, name, parent) return frame(name) end
        GameTooltip = {}
        function GameTooltip:SetOwner(o) self.owner = o; TIP = {} end
        function GameTooltip:IsOwned(o) return self.owner == o end
        function GameTooltip:SetText(t) table.insert(TIP, t) end
        function GameTooltip:AddLine(t) table.insert(TIP, t) end
        function GameTooltip:Show() end
        function GameTooltip:Hide() self.owner = nil end
        function click(b) this = b; b.scripts.OnClick() end
        function hover(b) this = b; b.scripts.OnEnter() end
    "#;

    #[test]
    fn the_buttons_flip_the_handles_and_queue_the_meter() {
        let lua = mlua::Lua::new();
        assert!(!lua.load(LUA).eval::<bool>().unwrap(), "no Minimap yet: it waits");
        lua.load(STAND_INS).exec().unwrap();
        assert!(lua.load(LUA).eval::<bool>().unwrap());
        assert!(lua.load(LUA).eval::<bool>().unwrap(), "once is enough");
        let handles = |lua: &mlua::Lua| {
            lua.load(format!("return GetCVar('{HANDLES_CVAR}')")).eval::<String>().unwrap()
        };
        assert_eq!(handles(&lua), "1", "registered on");
        let tip: (String, String) =
            lua.load("hover(EsurvMinimapBarsButton); return TIP[1], TIP[2]").eval().unwrap();
        assert_eq!(tip.0, "Boss timer bars: show/hide drag handles");
        assert_eq!(tip.1, "Drag handles shown");
        // a click flips the setting, says so in the tooltip, and is queued
        let tip: String = lua.load("click(EsurvMinimapBarsButton); return TIP[2]").eval().unwrap();
        assert_eq!(handles(&lua), "0");
        assert!(tip.starts_with("Drag handles hidden"));
        lua.load("click(EsurvMinimapBarsButton)").exec().unwrap();
        assert_eq!(handles(&lua), "1", "and back");
        let tip: (String, String) =
            lua.load("hover(EsurvMinimapMeterButton); return TIP[1], TIP[2]").eval().unwrap();
        assert_eq!(tip, ("Damage meter: show/hide".into(), "Shown".into()));
        let tip: String = lua.load("click(EsurvMinimapMeterButton); return TIP[2]").eval().unwrap();
        assert_eq!(tip, "Hidden");
        let queue: String = lua.load("return ESURV_MINIMAP_BUTTONS.take()").eval().unwrap();
        assert_eq!(clicks(&queue), vec![Click::Bars, Click::Bars, Click::Meter]);
        assert_eq!(lua.load("return ESURV_MINIMAP_BUTTONS.take()").eval::<String>().unwrap(), "");
        // the game's word on the meter wins
        let tip: String =
            lua.load("ESURV_MINIMAP_BUTTONS.set_meter(true); return TIP[2]").eval().unwrap();
        assert_eq!(tip, "Shown");
        let at: (f32, f32) = lua.load("return ESURV_MINIMAP_BUTTONS.where('meter')").eval().unwrap();
        assert_eq!(at, (100.0, 600.0));
    }
}
