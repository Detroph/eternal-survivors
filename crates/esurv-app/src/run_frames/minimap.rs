//! The run's minimap (screens.md, "The minimap"): the stock minimap recreated in our own art
//! while the hero is in a run. All of it is Lua ([`LUA`], `minimap.lua`), installed by the run
//! frames right after the action bar's and switched with them.

/// The Lua half. Returns true once installed, false while the stock minimap is not loaded yet.
pub const LUA: &str = include_str!("minimap.lua");

#[cfg(test)]
mod tests {
    use benilla_ui::script::UiScript;

    use super::*;

    #[test]
    fn the_lua_waits_for_the_stock_minimap() {
        let lua = mlua::Lua::new();
        assert!(!lua.load(LUA).eval::<bool>().unwrap());
    }

    /// The stock files the minimap needs, in the client's own load order.
    const STOCK_FILES: &[&str] = &[
        r"Interface\FrameXML\GlobalStrings.lua",
        r"Interface\FrameXML\Fonts.xml",
        r"Interface\FrameXML\UIParent.xml",
        r"Interface\FrameXML\GameTooltip.xml",
        r"Interface\FrameXML\Minimap.xml",
    ];

    /// The stock minimap from the player's data, then our Lua; `None` without the data.
    fn stock_minimap() -> Option<UiScript> {
        let data = benilla_formats::wow_data()?;
        let chain = benilla_formats::open_chain(&data).ok()?;
        let read = |req: &str| chain.read(req).ok();
        let mut s = UiScript::new().unwrap();
        s.set_screen_size(1024.0, 768.0);
        for entry in STOCK_FILES {
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
        assert!(s.eval::<bool>(LUA).unwrap(), "the minimap's Lua installs");
        assert!(
            s.eval::<bool>(crate::minimap_buttons::LUA).unwrap(),
            "our minimap buttons install"
        );
        s.resolve();
        Some(s)
    }

    fn run_on(s: &mut UiScript, on: bool) {
        let flag = if on { "1" } else { "nil" };
        s.run(&format!("ESURV_RUN_FRAMES_ON = {flag}; ESURV_MINIMAP.apply()"))
            .unwrap();
        s.resolve();
    }

    fn shown(s: &UiScript, name: &str) -> bool {
        s.eval(&format!("return {name}:IsVisible() and true or false"))
            .unwrap()
    }

    fn width(s: &UiScript, name: &str) -> f64 {
        s.eval(&format!("return {name}:GetWidth()")).unwrap()
    }

    /// In a run: the map a fifth larger, our ring and ribbon shown, the stock border gone, the
    /// zone's name in the ribbon, our buttons further out, each round button in our ring. Out
    /// of a run the stock minimap again, ours hidden.
    #[test]
    fn a_run_shows_ours_and_puts_the_stock_one_back() {
        let Some(mut s) = stock_minimap() else {
            benilla_formats::skipped("no WoW install found", &benilla_formats::candidates());
            return;
        };
        assert_eq!(width(&s, "Minimap"), 140.0);
        let ring_path = "return MiniMapTrackingBorder:GetTexture()";
        let stock_ring: String = s.eval(ring_path).unwrap();
        let stock_radius = || {
            "local x, y = EsurvMinimapBarsButtonIcon:GetCenter() \
             local cx, cy = Minimap:GetCenter() \
             return math.sqrt((x - cx) ^ 2 + (y - cy) ^ 2)"
        };

        run_on(&mut s, true);
        assert_eq!(width(&s, "Minimap"), 168.0);
        assert!(shown(&s, "EsurvMinimapRing"), "our ring");
        assert!(shown(&s, "EsurvMinimapRibbon"), "our ribbon");
        assert!(!shown(&s, "MinimapBorder"), "no stock border");
        assert!(!shown(&s, "MinimapToggleButton"), "no close button");
        let in_ribbon: bool = s
            .eval(
                "local l, r = EsurvMinimapRibbon:GetLeft(), EsurvMinimapRibbon:GetRight() \
                 local x = MinimapZoneText:GetLeft() return x >= l and x <= r",
            )
            .unwrap();
        assert!(in_ribbon, "the zone's name in the ribbon");
        let ours: String = s.eval(ring_path).unwrap();
        assert!(ours.contains("MapButtonRing"), "our ring round the buttons: {ours}");
        let buttons: String = s.eval("return EsurvMinimapBarsButtonBorder:GetTexture()").unwrap();
        assert!(buttons.contains("MapButtonRing"), "our own buttons too: {buttons}");
        let radius: f64 = s.eval(stock_radius()).unwrap();
        assert!((radius - 98.0).abs() < 1.0, "our buttons on the larger rim: {radius}");

        run_on(&mut s, false);
        assert_eq!(width(&s, "Minimap"), 140.0);
        assert!(!shown(&s, "EsurvMinimapRing"));
        assert!(!shown(&s, "EsurvMinimapRibbon"));
        assert!(shown(&s, "MinimapBorder"), "the stock border back");
        let back: String = s.eval(ring_path).unwrap();
        assert_eq!(back, stock_ring, "the stock ring back");
        let radius: f64 = s.eval(stock_radius()).unwrap();
        assert!((radius - 80.0).abs() < 1.0, "our buttons on the stock rim: {radius}");
    }
}
