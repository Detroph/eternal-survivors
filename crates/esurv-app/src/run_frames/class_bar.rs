//! The player frame's class bar in a run (screens.md, "The power bar"):
//! what the server says of the hero's starting option (`ESURVY`, [`ToClient::Bar`]) handed to our
//! Lua ([`LUA`], `class_bar.lua`): the mana bar in the resource's or counter's colour with its
//! name and count, the combo points as pips on the player frame, and the icons of the stacking
//! buffs whose aura's own icon is not theirs.

use bevy::prelude::*;

use benilla_app::Objects;
use benilla_ui::script::UiScript;
use esurv_content::option_looks::{bar_colour, icon_overrides, resource_of_word, BarColour};
use esurv_content::ResourceKind;
use esurv_protocol::ToClient;

use super::lua_string;
use crate::protocol::ServerEvent;
use crate::ui::in_world;

/// The Lua half. Returns true once installed.
pub const LUA: &str = include_str!("class_bar.lua");

/// The hero's bar and pips as the server last said them.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct ClassBar {
    /// The bar's kind, name, value and most.
    pub bar: Option<(ResourceKind, String, f32, f32)>,
    /// Combo points: how many, of how many.
    pub pips: Option<(f32, f32)>,
}

pub(super) fn add(app: &mut App) {
    app.init_resource::<ClassBar>()
        .add_systems(Update, (keep_the_bar, hand_the_bar_to_the_lua).chain());
}

/// Keeps the newest bar and pips; a hero out of the world forgets them.
fn keep_the_bar(
    mut events: MessageReader<ServerEvent>,
    objects: Objects,
    mut was_in: Local<bool>,
    mut bar: ResMut<ClassBar>,
) {
    let now_in = in_world(&objects);
    if *was_in && !now_in {
        *bar = ClassBar::default();
    }
    *was_in = now_in;
    for ServerEvent(m) in events.read() {
        if let ToClient::Bar { kind, name, value, max, .. } = m {
            let Some(kind) = resource_of_word(kind) else {
                continue;
            };
            if kind == ResourceKind::ComboPoints {
                bar.pips = Some((*value, *max));
            } else {
                bar.bar = Some((kind, name.clone(), *value, *max));
            }
        }
    }
}

/// The Lua call that shows `bar`.
pub fn bar_chunk(bar: &ClassBar) -> String {
    let mut chunk = String::new();
    match &bar.bar {
        Some((kind, name, value, max)) => {
            let (painted, [r, g, b]) = match bar_colour(*kind) {
                BarColour::Stock => (false, [1.0; 3]),
                BarColour::Painted(rgb) => (true, rgb),
            };
            chunk.push_str(&format!(
                "ESURV_CLASS_BAR.set_bar({}, {}, {painted}, {r:.3}, {g:.3}, {b:.3}, {value}, {max}); ",
                lua_string(&format!("{kind:?}")),
                lua_string(name),
            ));
        }
        None => chunk.push_str("ESURV_CLASS_BAR.clear(); "),
    }
    let (count, max) = bar.pips.unwrap_or((0.0, 0.0));
    chunk.push_str(&format!("ESURV_CLASS_BAR.set_pips({count}, {max})"));
    chunk
}

/// The Lua table of the buff icons shown in place of their auras' own: the stock looks' and,
/// with the content, each legendary counter buff's (the legendary's own icon).
pub fn icons_chunk(content: Option<&esurv_content::Content>) -> String {
    let mut icons: Vec<(u32, String)> = icon_overrides().into_iter().map(|(a, i)| (a, i.to_owned())).collect();
    icons.extend(content.into_iter().flat_map(|c| c.counter_buffs()).map(|b| (b.aura, b.icon)));
    let rows: Vec<String> = icons
        .into_iter()
        .map(|(aura, icon)| format!("[{aura}] = {}", lua_string(&format!(r"Interface\Icons\{icon}"))))
        .collect();
    format!("ESURV_CLASS_BAR.set_icons({{{}}})", rows.join(", "))
}

/// What this VM has been handed.
#[derive(Default)]
struct Handed {
    session: u64,
    installed: bool,
    shown: Option<String>,
}

/// Installs the Lua in a fresh VM once the hero is in the world, and hands it the bar when it
/// changes; out of the world, the stock bar again.
fn hand_the_bar_to_the_lua(
    script: Option<NonSend<UiScript>>,
    objects: Objects,
    bar: Res<ClassBar>,
    content: Option<Res<crate::ui::GameContent>>,
    mut handed: Local<Handed>,
    mut warned: Local<bool>,
) {
    let Some(script) = script else {
        return;
    };
    if handed.session != script.session() {
        *handed = Handed {
            session: script.session(),
            ..default()
        };
    }
    if !handed.installed {
        if !in_world(&objects) {
            return;
        }
        match script.eval::<bool>(LUA) {
            Ok(true) => {
                handed.installed = true;
                if let Err(e) = script.run(&icons_chunk(content.as_ref().map(|c| &*c.0))) {
                    warn!("class bar: the buff icons: {e}");
                }
            }
            Ok(false) => return,
            Err(e) => {
                if !std::mem::replace(&mut *warned, true) {
                    warn!("class bar: the Lua did not load: {e}");
                }
                return;
            }
        }
    }
    let chunk = bar_chunk(&bar);
    if handed.shown.as_deref() != Some(chunk.as_str()) {
        if let Err(e) = script.run(&chunk) {
            warn!("class bar: {e}");
        }
        handed.shown = Some(chunk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_counter_paints_the_bar_and_rage_keeps_its_own() {
        let bar = ClassBar {
            bar: Some((ResourceKind::HolyPower, "Holy Power".into(), 3.0, 5.0)),
            pips: None,
        };
        let chunk = bar_chunk(&bar);
        assert!(chunk.contains("set_bar(\"HolyPower\", \"Holy Power\", true, 0.950"), "{chunk}");
        assert!(chunk.ends_with("set_pips(0, 0)"));
        let rage = ClassBar {
            bar: Some((ResourceKind::Energy, String::new(), 40.0, 100.0)),
            pips: Some((3.0, 5.0)),
        };
        let chunk = bar_chunk(&rage);
        assert!(chunk.contains("set_bar(\"Energy\", \"\", false"), "{chunk}");
        assert!(chunk.ends_with("set_pips(3, 5)"));
        assert!(bar_chunk(&ClassBar::default()).starts_with("ESURV_CLASS_BAR.clear()"));
    }

    #[test]
    fn the_icons_read_as_a_lua_table() {
        let chunk = icons_chunk(None);
        assert!(chunk.contains("INV_Hammer_05"), "{chunk}");
        assert!(chunk.starts_with("ESURV_CLASS_BAR.set_icons({["));
    }

    /// The Lua installs in a VM with the stock frames it changes, paints the mana bar, writes the
    /// count, lights the pips and answers the buff icons.
    #[test]
    fn the_lua_paints_the_bar_and_lights_the_pips() {
        let lua = mlua::Lua::new();
        lua.load(STUB).exec().unwrap();
        assert!(lua.load(LUA).eval::<bool>().unwrap());
        let bar = ClassBar {
            bar: Some((ResourceKind::Insanity, "Insanity".into(), 21.0, 100.0)),
            pips: Some((2.0, 5.0)),
        };
        lua.load(&bar_chunk(&bar)).exec().unwrap();
        lua.load(&icons_chunk(None)).exec().unwrap();
        let painted: (f32, String, bool, bool, String, bool) = lua
            .load(
                "return ManaBarColor[0].r, EsurvClassBarText.text, \
                 EsurvComboPip2Lit.shown, EsurvComboPip3Lit.shown, \
                 EsurvClassBarNameText.text, EsurvClassBarName.shown",
            )
            .eval()
            .unwrap();
        assert!((painted.0 - 0.45).abs() < 0.01);
        assert_eq!(painted.1, "21 / 100");
        assert!(painted.2 && !painted.3);
        assert_eq!(painted.4, "Insanity");
        assert!(painted.5);
        let icon: String = lua
            .load("ESURV_RUN_FRAMES_ON = 1; ESURV_RUN_FRAMES = { player_auras = { [0] = 1467 } }; return GetPlayerBuffTexture(0)")
            .eval()
            .unwrap();
        assert_eq!(icon, r"Interface\Icons\INV_Enchant_EssenceMysticalSmall");
        lua.load("ESURV_CLASS_BAR.clear()").exec().unwrap();
        let back: (f32, bool) = lua.load("return ManaBarColor[0].b, EsurvClassBarName.shown").eval().unwrap();
        assert_eq!(back, (1.0, false));
    }

    /// Just enough of the stock frames for the Lua.
    const STUB: &str = r#"
        local function frame(name)
            local f = { shown = false }
            function f:Show() self.shown = true end
            function f:Hide() self.shown = false end
            function f:SetPoint() end
            function f:ClearAllPoints() end
            function f:SetWidth() end
            function f:SetHeight() end
            function f:SetTexture(t) self.texture = t end
            function f:SetTexCoord() end
            function f:SetText(t) self.text = t end
            function f:SetStatusBarColor(r, g, b) self.colour = { r, g, b } end
            function f:CreateFontString(n) local s = frame(n); if n then _G[n] = s end; return s end
            function f:CreateTexture(n) local s = frame(n); if n then _G[n] = s end; return s end
            return f
        end
        function CreateFrame(_, n) local f = frame(n); if n then _G[n] = f end; return f end
        PlayerFrame = frame()
        PlayerFrameManaBar = frame()
        ManaBarColor = { [0] = { r = 0, g = 0, b = 1 } }
        function UnitPowerType() return 0 end
        function GetPlayerBuffTexture(i) return "stock" end
        function UnitBuff() return "stock", 1 end
    "#;
}
