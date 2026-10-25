//! The run's action bar (screens.md, "The action bar"): the stock main bar laid out compact and
//! centred while the hero is in a run, from its own art. Eight buttons (seven spells, the race
//! spell on the eighth: the server's `present::RACE_SPELL_SLOT`), the page arrows and number, the
//! character, spellbook, talent and main menu micro buttons (the last, its tooltip "Options",
//! opens the run's Escape menu, `crate::hud::escape_menu`, as Escape does), a gryphon at each
//! end, and the experience bar over it in ten segments. All of it is Lua ([`LUA`],
//! `action_bar.lua`), installed by the run frames right after their own and switched with them.

/// The Lua half. Returns true once installed, false while the stock bar is not loaded yet.
pub const LUA: &str = include_str!("action_bar.lua");

/// The buttons the run's bar shows: the server's seven spell buttons and the race spell's.
pub const BUTTONS: u32 = 8;
/// The run's bar's width, in the stock bar's units (the stock bar is 1024).
pub const WIDTH: f64 = 494.0;
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

    /// In a run: the bar is 494 wide and centred, eight buttons in a row, the arrows and the
    /// four micro buttons after them, a gryphon over each end, the ninth to twelfth buttons,
    /// the bag slots' art and the latency bar gone; out of a run the stock bar again.
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
        assert_eq!(
            s.eval::<u32>("return ESURV_ACTION_BAR.BUTTONS").unwrap(),
            BUTTONS
        );
        assert_eq!(s.eval::<f64>("return ESURV_ACTION_BAR.WIDTH").unwrap(), WIDTH);
        for i in 1..=BUTTONS {
            let name = format!("ActionButton{i}");
            let (l, _) = span(&s, &name);
            assert_eq!(l - left, 8.0 + 42.0 * f64::from(i - 1), "{name} in its place");
        }
        // an empty button hides itself; the ninth to twelfth stay hidden whatever shows them
        for i in BUTTONS + 1..=12 {
            s.run(&format!("ActionButton{i}:Show()")).unwrap();
            assert!(!shown(&s, &format!("ActionButton{i}")), "button {i} is gone");
        }
        assert!(!shown(&s, "MainMenuBarPerformanceBarFrame"), "no latency bar");
        assert!(!shown(&s, "MainMenuBarTexture3"), "no bag slots' art");
        // the arrows and the page number after the eighth button, then the micro buttons
        let (_, eighth) = span(&s, "ActionButton8");
        let (up_left, up_right) = span(&s, "ActionBarUpButton");
        assert!(up_left > eighth - 10.0 && up_right < span(&s, "CharacterMicroButton").0 + 1.0);
        assert!(shown(&s, "MainMenuBarPageNumber"));
        let micro = ["CharacterMicroButton", "SpellbookMicroButton", "TalentMicroButton",
            "MainMenuMicroButton"];
        let mut last = up_right;
        for name in micro {
            assert!(shown(&s, name), "{name} shows");
            let (l, r) = span(&s, name);
            assert!(l >= last - 3.0 && r <= right, "{name} in a row inside the bar");
            last = r;
        }
        for name in ["QuestLogMicroButton", "SocialsMicroButton", "WorldMapMicroButton",
            "HelpMicroButton"] {
            assert!(!shown(&s, name), "{name} is hidden");
        }
        // a gryphon over each end, overlapping it by 32 as the stock caps do
        let (cap_l, _) = span(&s, "MainMenuBarLeftEndCap");
        let (_, cap_r) = span(&s, "MainMenuBarRightEndCap");
        assert_eq!((left - cap_l, cap_r - right), (96.0, 96.0));
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
        assert_eq!(span(&s, "CharacterMicroButton").0, 552.0);
        assert!(!shown(&s, "EsurvActionBarPillar"));
        assert_eq!(s.eval::<String>("return MainMenuMicroButton.tooltipText or \"\"").unwrap(), stock_tooltip, "its stock tooltip again");
        assert!(s.errors().is_empty(), "script errors: {:?}", s.errors());
    }

    /// The experience bar spans the run's bar in ten segments: two of the stock art's four
    /// strips of five, the first and the last (its two ends), half the width each.
    #[test]
    fn the_experience_bar_takes_the_width_in_ten_segments() {
        let Some(mut s) = stock_bar() else {
            benilla_formats::skipped("no WoW install found", &benilla_formats::candidates());
            return;
        };
        run_on(&mut s, true);
        let (left, right) = span(&s, "MainMenuExpBar");
        assert_eq!(right - left, WIDTH);
        assert_eq!(s.eval::<u32>("return ESURV_ACTION_BAR.XP_SEGMENTS").unwrap(), XP_SEGMENTS);
        let strips: Vec<(f64, f64)> = (0..4)
            .filter(|i| shown(&s, &format!("MainMenuXPBarTexture{i}")))
            .map(|i| span(&s, &format!("MainMenuXPBarTexture{i}")))
            .collect();
        // each stock strip draws five segments
        assert_eq!(strips.len() as u32 * 5, XP_SEGMENTS);
        assert_eq!(strips, [(left, left + WIDTH / 2.0), (left + WIDTH / 2.0, right)]);
        let quads = s.extract();
        let rows: Vec<_> = quads
            .iter()
            .filter_map(|q| match &q.content {
                QuadContent::Texture { path: Some(p), tex_coords, .. }
                    if p.ends_with("UI-MainMenuBar-Dwarf") && q.rect.is_some_and(|r| r.top - r.bottom < 11.0) =>
                {
                    *tex_coords
                }
                _ => None,
            })
            .collect();
        assert_eq!(rows.len(), 2, "two strips drawn: {rows:?}");
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
