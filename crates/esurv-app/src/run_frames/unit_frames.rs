//! The run's player and target frames (screens.md, "The player and target frames"): the stock
//! frames recreated in our own art while the hero is in a run. All of it is Lua ([`LUA`],
//! `unit_frames.lua`), installed by the run frames right after the minimap's and switched with
//! them.

/// The Lua half. Returns true once installed, false while the stock frames are not loaded yet.
pub const LUA: &str = include_str!("unit_frames.lua");

#[cfg(test)]
mod tests {
    use benilla_ui::script::UiScript;

    use super::*;

    #[test]
    fn the_lua_waits_for_the_stock_frames() {
        let lua = mlua::Lua::new();
        assert!(!lua.load(LUA).eval::<bool>().unwrap());
    }

    /// The stock files the two frames need, in the client's own load order.
    const STOCK_FILES: &[&str] = &[
        r"Interface\FrameXML\GlobalStrings.lua",
        r"Interface\FrameXML\Fonts.xml",
        r"Interface\FrameXML\UIParent.xml",
        r"Interface\FrameXML\GameTooltip.xml",
        r"Interface\FrameXML\TextStatusBar.lua",
        r"Interface\FrameXML\TextStatusBar.xml",
        r"Interface\FrameXML\UnitFrame.lua",
        r"Interface\FrameXML\UnitFrame.xml",
        r"Interface\FrameXML\PlayerFrame.xml",
        r"Interface\FrameXML\TargetFrame.xml",
    ];

    /// The stock frames from the player's data, then our Lua; `None` without the data.
    fn stock_frames() -> Option<UiScript> {
        let data = benilla_formats::wow_data()?;
        let chain = benilla_formats::open_chain(&data).ok()?;
        let read = |req: &str| chain.read(req).ok();
        let mut s = UiScript::new().unwrap();
        s.set_screen_size(1024.0, 768.0);
        for entry in STOCK_FILES {
            // the stock wrapper over a localized string, which `UnitFrame.lua` calls at once
            s.run("TEXT = TEXT or function(t) return t end").unwrap();
            let bytes = read(entry).unwrap_or_else(|| panic!("{entry}: not found"));
            if entry.ends_with(".lua") {
                s.run_chunk_named(&bytes, &format!("@{entry}")).unwrap();
            } else {
                let doc =
                    benilla_ui::framexml::parse(&benilla_ui::source::decode(&bytes)).unwrap();
                // other files' templates and frames are not loaded here: what it does load is
                // what is checked
                let _ = benilla_ui::loader::load_in(&s, &doc, &entry.replace('\\', "/"), &read);
            }
        }
        assert!(s.eval::<bool>(LUA).unwrap(), "the unit frames' Lua installs");
        s.resolve();
        Some(s)
    }

    fn run_on(s: &mut UiScript, on: bool) {
        let flag = if on { "1" } else { "nil" };
        s.run(&format!("ESURV_RUN_FRAMES_ON = {flag}; ESURV_UNIT_FRAMES.apply()"))
            .unwrap();
        s.resolve();
    }

    fn shown(s: &UiScript, name: &str) -> bool {
        s.eval(&format!("return {name}:IsVisible() and true or false"))
            .unwrap()
    }

    fn number(s: &UiScript, lua: &str) -> f64 {
        s.eval(&format!("return {lua}")).unwrap_or_else(|e| panic!("{lua}: {e}"))
    }

    fn font_size(s: &UiScript, text: &str) -> f64 {
        s.eval(&format!("local _, h = {text}:GetFont() return h")).unwrap()
    }

    /// In a run: both portraits larger in our ring, the plate and the name's strip shown, the
    /// stock border gone and kept gone, the texts larger, the bars' numbers shown, the target's
    /// portrait at its right. Out of a run the stock frames again, ours hidden.
    #[test]
    fn a_run_shows_ours_and_puts_the_stock_ones_back() {
        let Some(mut s) = stock_frames() else {
            benilla_formats::skipped("no WoW install found", &benilla_formats::candidates());
            return;
        };
        s.run("TargetFrame:Show()").unwrap();
        s.resolve();
        assert_eq!(number(&s, "PlayerPortrait:GetWidth()"), 64.0);
        let stock_name = font_size(&s, "PlayerName");
        let stock_health = font_size(&s, "PlayerFrameHealthBarText");

        run_on(&mut s, true);
        let portrait = number(&s, "ESURV_UNIT_FRAMES.PORTRAIT");
        for frame in ["Player", "Target"] {
            assert_eq!(number(&s, &format!("{frame}Portrait:GetWidth()")), portrait);
            for ours in ["Plate", "Strip", "Ring"] {
                assert!(shown(&s, &format!("EsurvUnitFrame{frame}{ours}")), "{frame}'s {ours}");
            }
        }
        assert!(portrait > 64.0, "a larger portrait");
        assert!(!shown(&s, "PlayerFrameTexture"), "no stock border");
        s.run("PlayerFrameTexture:Show() TargetFrameTexture:Show()").unwrap();
        assert!(!shown(&s, "PlayerFrameTexture"), "kept gone");
        assert!(!shown(&s, "TargetFrameTexture"), "the target's kept gone");
        assert!(font_size(&s, "PlayerName") > stock_name, "a larger name");
        assert!(font_size(&s, "PlayerFrameHealthBarText") > stock_health, "larger numbers");
        assert!(shown(&s, "PlayerFrameHealthBarText"), "the numbers shown");
        let mirrored: bool = s
            .eval("return TargetPortrait:GetLeft() > TargetFrameHealthBar:GetRight()")
            .unwrap();
        assert!(mirrored, "the target's portrait at its right");
        let beside: bool = s
            .eval("return PlayerFrameHealthBar:GetLeft() > PlayerPortrait:GetRight()")
            .unwrap();
        assert!(beside, "the player's bars right of the portrait");
        // shrunk to fit beside the top ribbon: on this 1024 wide screen as far as they go
        let scale = number(&s, "TargetFrame:GetScale()");
        assert_eq!(scale, number(&s, "PlayerFrame:GetScale()"));
        assert!(scale < 1.0, "smaller on a narrow screen");
        assert_eq!(number(&s, "ESURV_UNIT_FRAMES.fit_scale(4000)"), 1.0, "full size on a wide one");

        // the modern layout: smaller, either side of the screen's middle, mirrored about it, low
        s.run("ESURV_UNIT_FRAMES.set_layout('modern')").unwrap();
        s.resolve();
        assert_eq!(number(&s, "ESURV_UNIT_FRAMES.layout() == 'modern' and 1 or 0"), 1.0);
        assert!((number(&s, "PlayerFrame:GetScale()") - 0.7).abs() < 1e-6, "three tenths smaller");
        assert_eq!(number(&s, "TargetFrame:GetScale()"), number(&s, "PlayerFrame:GetScale()"));
        // in the root's units: the player's right edge as far left of the middle as the target's
        // left edge is right of it, the two at one height, in the screen's lower half
        let middle = number(&s, "UIParent:GetWidth()") / 2.0;
        let player_right = number(&s, "PlayerFrame:GetRight() * PlayerFrame:GetScale()");
        let target_left = number(&s, "TargetFrame:GetLeft() * TargetFrame:GetScale()");
        assert!(player_right < middle && target_left > middle, "either side of the middle");
        assert!(((middle - player_right) - (target_left - middle)).abs() < 0.01, "mirrored");
        let player_top = number(&s, "PlayerFrame:GetTop()");
        assert_eq!(player_top, number(&s, "TargetFrame:GetTop()"), "level");
        assert!(player_top * 0.7 < number(&s, "UIParent:GetHeight()") / 2.0, "low");
        s.run("ESURV_UNIT_FRAMES.set_layout('classic')").unwrap();
        s.resolve();
        assert_eq!(number(&s, "TargetFrame:GetScale()"), scale, "back to the classic");

        run_on(&mut s, false);
        assert_eq!(number(&s, "PlayerPortrait:GetWidth()"), 64.0);
        assert_eq!(number(&s, "TargetFrame:GetScale()"), 1.0, "the stock size back");
        assert!(shown(&s, "PlayerFrameTexture"), "the stock border back");
        assert!(!shown(&s, "EsurvUnitFramePlayerPlate"));
        assert!(!shown(&s, "EsurvUnitFrameTargetRing"));
        assert_eq!(font_size(&s, "PlayerName"), stock_name);
    }
}
