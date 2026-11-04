//! The run's action bar (screens.md, "The action bar"): the stock main bar recreated in our own
//! art while the hero is in a run. Eight buttons (seven spells, the race spell on the eighth: the
//! server's `present::RACE_SPELL_SLOT`) in one bronze frame, each in a dark well and a bronze rim,
//! a dragon at each end; the experience bar seated on the frame's top edge, in a frame ticked in
//! ten segments; and apart, at the screen's bottom right, the character, spellbook, talent and
//! main menu micro buttons in a frame of their own (the last, its tooltip "Options", opens the
//! run's Escape menu, `crate::hud::escape_menu`, as Escape does). Both frames sit at one distance
//! from the screen's foot. All of it is Lua ([`LUA`], `action_bar.lua`), installed by the run
//! frames right after their own and switched with them.

/// The Lua half. Returns true once installed, false while the stock bar is not loaded yet.
pub const LUA: &str = include_str!("action_bar.lua");

/// The buttons the run's bar shows: the server's seven spell buttons and the race spell's.
pub const BUTTONS: u32 = 8;
/// The run's bar's width, in the stock bar's units (the stock bar is 1024).
pub const WIDTH: f64 = 346.0;
/// The bar's frame's foot and the micro buttons' frame's, above the screen's foot.
pub const BOTTOM_GAP: f64 = 6.0;
/// The experience bar's segments: a tenth each.
pub const XP_SEGMENTS: u32 = 10;

#[cfg(test)]
mod tests {
    use benilla_ui::script::{QuadContent, UiScript};

    use super::*;

    #[test]
    fn the_lua_waits_for_the_stock_bar() {
        let lua = mlua::Lua::new();
        assert!(!lua.load(LUA).eval::<bool>().unwrap());
    }

    /// The stock files the bar and its micro buttons need, in the client's own load order.
    const STOCK_FILES: &[&str] = &[
        r"Interface\FrameXML\GlobalStrings.lua",
        r"Interface\FrameXML\Fonts.xml",
        r"Interface\FrameXML\BasicControls.xml",
        r"Interface\FrameXML\UIParent.xml",
        r"Interface\FrameXML\MoneyFrame.lua",
        r"Interface\FrameXML\MoneyFrame.xml",
        r"Interface\FrameXML\GameTooltip.xml",
        r"Interface\FrameXML\UIPanelTemplates.lua",
        r"Interface\FrameXML\UIPanelTemplates.xml",
        r"Interface\FrameXML\OptionsFrameTemplates.xml",
        r"Interface\FrameXML\TextStatusBar.lua",
        r"Interface\FrameXML\TextStatusBar.xml",
        r"Interface\FrameXML\MainMenuBar.xml",
        r"Interface\FrameXML\MainMenuBarMicroButtons.xml",
        r"Interface\FrameXML\Cooldown.xml",
        r"Interface\FrameXML\ActionButtonTemplate.xml",
        r"Interface\FrameXML\ActionBarFrame.xml",
        r"Interface\FrameXML\ReputationFrame.xml",
        r"Interface\FrameXML\BonusActionBarFrame.xml",
    ];

    /// What the loaded files call into files not loaded here: no-ops, and hidden frames seated
    /// on the first call (a frame seated early would shadow the real one).
    const STAND_INS: &str = r#"
        for _, n in ipairs({ "FCF_OnUpdate", "FCF_DockUpdate", "UnitPopup_OnUpdate",
            "BattlefieldFrame_OnUpdate", "MultiActionBar_Update", "LocalizeFrames",
            "updateContainerFrameAnchors", "UpdateNameplates", "CloseAllBags", "OpenBackpack",
            "CloseBackpack", "RaidOptionsFrame_UpdatePartyFrames", "ToggleQuestLog",
            "ToggleFriendsFrame", "ToggleWorldMap", "ToggleHelpFrame", "ToggleBackpack",
            "ToggleBag", "OpenAllBags", "OpenBag", "ToggleKeyRing", "ToggleBattlefieldMinimap",
            "ToggleWorldStateScoreFrame", "ShowMacroFrame", "ToggleCharacter", "ToggleSpellBook",
            "ToggleTalentFrame", "ToggleDropDownMenu" }) do
            _G[n] = _G[n] or function() end
        end
        function ToggleGameMenu(clicked) MENU_ASKED = (MENU_ASKED or 0) + 1; MENU_CLICKED = clicked end
        NUM_CONTAINER_FRAMES = 0; CONTAINER_OFFSET_X = 0; CONTAINER_OFFSET_Y = 70
        BATTLEFIELD_TAB_OFFSET_Y = 210; PETACTIONBAR_YPOS = 98; PETACTIONBAR_XPOS = 36
        local function seat(names)
            for _, name in ipairs(names) do
                if not getglobal(name) then local f = CreateFrame("Frame") f:Hide() setglobal(name, f) end
            end
        end
        local real_manage = UIParent_ManageFramePositions
        function UIParent_ManageFramePositions()
            local named = {}
            for name, row in pairs(UIPARENT_MANAGED_FRAME_POSITIONS) do
                if not row.isVar then table.insert(named, name) end
                if row.anchorTo then table.insert(named, row.anchorTo) end
            end
            seat(named)
            seat({ "MultiBarLeft", "MultiBarRight", "MultiBarBottomLeft", "PetActionBarFrame",
                "ShapeshiftBarFrame", "CastingBarFrame", "QuestTimerFrame", "QuestWatchFrame",
                "DurabilityFrame", "DurabilityShield", "DurabilityOffWeapon", "DurabilityRanged",
                "MinimapCluster", "ChatFrame1", "ChatFrame2",
                "ShapeshiftBarLeft", "ShapeshiftBarMiddle", "ShapeshiftBarRight",
                "BattlefieldMinimapTab" })
            for _, name in ipairs({ "SlidingActionBarTexture0", "SlidingActionBarTexture1" }) do
                if not getglobal(name) then setglobal(name, UIParent:CreateTexture()) end
            end
            return real_manage()
        end
        local real_micro = UpdateMicroButtons
        function UpdateMicroButtons()
            seat({ "CharacterFrame", "SpellBookFrame", "QuestLogFrame", "GameMenuFrame",
                "OptionsFrame", "SoundOptionsFrame", "UIOptionsFrame", "FriendsFrame",
                "WorldMapFrame", "HelpFrame" })
            if not KeyRingButton then KeyRingButton = CreateFrame("Button") KeyRingButton:Hide() end
            KEYRING_CONTAINER = KEYRING_CONTAINER or -2
            IsBagOpen = IsBagOpen or function() return nil end
            return real_micro()
        end
        GetSpellTabInfo = GetSpellTabInfo or function() end
        GetNumSpellTabs = GetNumSpellTabs or function() return 0 end
    "#;

    /// The stock bar from the player's data, then the run frames and the bar's Lua; `None`
    /// without the data.
    fn stock_bar() -> Option<UiScript> {
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
                let report =
                    benilla_ui::loader::load_in(&s, &doc, &entry.replace('\\', "/"), &read);
                assert!(report.errors.is_empty(), "{entry}: {:#?}", report.errors);
            }
            if entry.ends_with("UIParent.xml") {
                // UIParent's own wrappers are in now
                s.run(STAND_INS).unwrap();
            }
        }
        s.run(STAND_INS).unwrap();
        assert!(s.eval::<bool>(crate::run_frames::LUA).unwrap(), "the run frames install");
        assert!(s.eval::<bool>(LUA).unwrap(), "the bar's Lua installs");
        s.fire_event("PLAYER_ENTERING_WORLD", vec![]);
        Some(s)
    }

    fn run_on(s: &mut UiScript, on: bool) {
        let flag = if on { "1" } else { "nil" };
        s.run(&format!("ESURV_RUN_FRAMES_ON = {flag}; ESURV_RUN_FRAMES.apply()"))
            .unwrap();
        s.resolve();
    }

    /// Left and right of a frame or region, as laid out.
    fn span(s: &UiScript, name: &str) -> (f64, f64) {
        s.eval(&format!("return {name}:GetLeft(), {name}:GetRight()"))
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    fn shown(s: &UiScript, name: &str) -> bool {
        s.eval(&format!("return {name}:IsVisible() and true or false"))
            .unwrap()
    }

    /// In a run: the bar is 346 wide and centred, eight buttons in a row in our frame, each in
    /// its well and rim, the arrows and page number gone, the four micro buttons in their own
    /// frame at the screen's bottom right, both frames' feet at one gap over the screen's foot, a
    /// dragon past each end clear of the buttons; the ninth to twelfth buttons, the bag slots'
    /// art and the latency bar gone. Out of a run the stock bar again, ours hidden.
    #[test]
    fn a_run_lays_the_bar_out_compact_and_puts_the_stock_one_back() {
        let Some(mut s) = stock_bar() else {
            benilla_formats::skipped("no WoW install found", &benilla_formats::candidates());
            return;
        };
        s.resolve();
        assert_eq!(s.eval::<f64>("return MainMenuBar:GetWidth()").unwrap(), 1024.0);
        let stock_tooltip: String = s.eval("return MainMenuMicroButton.tooltipText or \"\"").unwrap();

        run_on(&mut s, true);
        let (left, right) = span(&s, "MainMenuBar");
        assert_eq!(right - left, WIDTH);
        assert_eq!((left + right) / 2.0, 512.0, "centred");
        assert_eq!(s.eval::<u32>("return ESURV_ACTION_BAR.BUTTONS").unwrap(), BUTTONS);
        assert_eq!(s.eval::<f64>("return ESURV_ACTION_BAR.WIDTH").unwrap(), WIDTH);
        assert_eq!(s.eval::<f64>("return ESURV_ACTION_BAR.BOTTOM_GAP").unwrap(), BOTTOM_GAP);
        for i in 1..=BUTTONS {
            let name = format!("ActionButton{i}");
            let (l, _) = span(&s, &name);
            assert_eq!(l - left, 8.0 + 42.0 * f64::from(i - 1), "{name} in its place");
            assert!(shown(&s, &format!("EsurvActionBarWell{i}")), "{name}'s well");
            assert!(shown(&s, &format!("EsurvActionBarRim{i}")), "{name}'s rim");
        }
        // an empty button hides itself; the ninth to twelfth stay hidden whatever shows them
        for i in BUTTONS + 1..=12 {
            s.run(&format!("ActionButton{i}:Show()")).unwrap();
            assert!(!shown(&s, &format!("ActionButton{i}")), "button {i} is gone");
        }
        assert!(!shown(&s, "MainMenuBarPerformanceBarFrame"), "no latency bar");
        assert!(!shown(&s, "MainMenuBarTexture3"), "no bag slots' art");
        for name in ["ActionBarUpButton", "ActionBarDownButton", "MainMenuBarPageNumber"] {
            assert!(!shown(&s, name), "{name} is gone");
        }
        // the frame round the buttons, its foot at the gap
        assert!(shown(&s, "EsurvActionBarTray"));
        let (tray_l, tray_r) = span(&s, "EsurvActionBarTray");
        let tray_bottom: f64 = s.eval("return EsurvActionBarTray:GetBottom()").unwrap();
        assert_eq!(tray_bottom, BOTTOM_GAP, "the bar's frame's foot");
        // the micro buttons in a row in their own frame at the bottom right, its foot at the gap
        assert!(shown(&s, "EsurvMicroFrame"));
        let (micro_l, micro_r) = span(&s, "EsurvMicroFrame");
        let micro_bottom: f64 = s.eval("return EsurvMicroFrame:GetBottom()").unwrap();
        assert_eq!(micro_bottom, BOTTOM_GAP, "the micro frame's foot, as the bar's");
        assert!(micro_r <= 1024.0 && micro_r > 1000.0, "at the right: {micro_r}");
        let micro = ["CharacterMicroButton", "SpellbookMicroButton", "TalentMicroButton",
            "MainMenuMicroButton"];
        let mut last = micro_l;
        for name in micro {
            assert!(shown(&s, name), "{name} shows");
            let (l, r) = span(&s, name);
            assert!(l >= last && r <= micro_r, "{name} in a row inside its frame");
            last = r;
        }
        for name in ["QuestLogMicroButton", "SocialsMicroButton", "WorldMapMicroButton",
            "HelpMicroButton"] {
            assert!(!shown(&s, name), "{name} is hidden");
        }
        // a dragon past each end, its middle outside the frame, clear of the first and last
        // buttons' middles
        let (cap_l, cap_lr) = span(&s, "MainMenuBarLeftEndCap");
        let (cap_rl, cap_r) = span(&s, "MainMenuBarRightEndCap");
        assert!((cap_l + cap_lr) / 2.0 < tray_l && (cap_rl + cap_r) / 2.0 > tray_r);
        // the experience bar over the frame, in its own ticked frame
        assert!(shown(&s, "EsurvXpFrame"));
        let xp_bottom: f64 = s.eval("return MainMenuExpBar:GetBottom()").unwrap();
        let tray_top: f64 = s.eval("return EsurvActionBarTray:GetTop()").unwrap();
        assert!(xp_bottom < tray_top && xp_bottom > tray_top - 8.0, "seated on the frame's edge");
        // the last micro button reads "Options"
        let tooltip: String = s.eval("return MainMenuMicroButton.tooltipText or \"\"").unwrap();
        assert!(tooltip.starts_with("Options"), "{tooltip}");

        run_on(&mut s, false);
        let (left, right) = span(&s, "MainMenuBar");
        assert_eq!((left, right), (0.0, 1024.0), "the stock bar again");
        assert!(shown(&s, "MainMenuBarTexture3") && shown(&s, "MainMenuXPBarTexture1"));
        s.run("ActionButton12:Show()").unwrap();
        assert!(shown(&s, "ActionButton12"), "the twelfth button is free again");
        assert_eq!(span(&s, "ActionBarUpButton").0, 522.0 - 16.0);
        assert!(shown(&s, "ActionBarUpButton") && shown(&s, "MainMenuBarPageNumber"));
        assert_eq!(span(&s, "CharacterMicroButton").0, 552.0);
        for name in ["EsurvActionBarTray", "EsurvActionBarRims", "EsurvMicroFrame", "EsurvXpFrame"] {
            assert!(!shown(&s, name), "{name} hidden");
        }
        let normal: String = s.eval("return CharacterMicroButton:GetNormalTexture():GetTexture() or \"\"").unwrap();
        assert!(!normal.contains("ESurv"), "the stock art again: {normal}");
        assert_eq!(s.eval::<String>("return MainMenuMicroButton.tooltipText or \"\"").unwrap(), stock_tooltip, "its stock tooltip again");
        assert!(s.errors().is_empty(), "script errors: {:?}", s.errors());
    }

    /// The experience bar spans the frame less its ends, its frame's art drawn round it and the
    /// stock strips gone.
    #[test]
    fn the_experience_bar_takes_the_width_in_ten_segments() {
        let Some(mut s) = stock_bar() else {
            benilla_formats::skipped("no WoW install found", &benilla_formats::candidates());
            return;
        };
        run_on(&mut s, true);
        let (left, right) = span(&s, "MainMenuExpBar");
        let (tray_l, tray_r) = span(&s, "EsurvActionBarTray");
        assert!(left > tray_l && right < tray_r && right - left > WIDTH - 40.0);
        assert_eq!(s.eval::<u32>("return ESURV_ACTION_BAR.XP_SEGMENTS").unwrap(), XP_SEGMENTS);
        for i in 0..4 {
            assert!(!shown(&s, &format!("MainMenuXPBarTexture{i}")), "stock strip {i} gone");
        }
        let (art_l, art_r) = span(&s, "EsurvXpFrameArt");
        assert!(art_l < left && art_r > right, "the frame's art round the bar");
        let quads = s.extract();
        assert!(
            quads.iter().any(|q| matches!(&q.content,
                QuadContent::Texture { path: Some(p), .. } if p.ends_with("XpFrame.tga"))),
            "the frame drawn"
        );
        assert!(s.errors().is_empty(), "script errors: {:?}", s.errors());
    }

    /// The main menu's micro button stays in a run and asks for the game menu as its stock click
    /// does, which the run's Escape menu answers (`benilla_app::GameMenuStandIn`).
    #[test]
    fn the_main_menu_button_asks_for_the_game_menu() {
        let Some(mut s) = stock_bar() else {
            benilla_formats::skipped("no WoW install found", &benilla_formats::candidates());
            return;
        };
        run_on(&mut s, true);
        assert!(shown(&s, "MainMenuMicroButton"));
        s.run("MainMenuMicroButton:Click()").unwrap();
        assert_eq!(s.eval::<(i64, i64)>("return MENU_ASKED, MENU_CLICKED").unwrap(), (1, 1));
        assert!(s.errors().is_empty(), "script errors: {:?}", s.errors());
    }
}
