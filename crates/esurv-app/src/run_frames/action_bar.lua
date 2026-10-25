-- The run's action bar (docs/game/screens.md, "The action bar"). esurv-app's
-- `run_frames::action_bar` runs this once in each interface VM, right after the run frames'
-- Lua, which calls `ESURV_ACTION_BAR.apply()` whenever the run turns on or off.
--
-- In a run the stock main bar (MainMenuBar.xml, 1024 wide) is laid out compact and centred, from
-- its own art: eight buttons (seven spells, the race spell on the eighth), the page arrows and the
-- page number, then four micro buttons (character, spellbook, talents, main menu: its tooltip
-- "Options"), a gryphon at each end. The bar's slots past the eighth and the bag slots' strip are
-- left out (ActionButton9 to 12, the bag buttons and the latency bar the run frames hide). The
-- experience bar above it takes the new width in ten segments: the stock strip's first and last
-- quarters, five segments each.
--
-- Out of a run every piece is put back as the stock file laid it out.
--
-- Returns true once installed, false while the stock bar is not loaded yet.

if ESURV_ACTION_BAR then
    return true
end
if not (MainMenuBar and MainMenuBarArtFrame and MainMenuExpBar and MainMenuBarTexture0
        and ActionButton1 and CharacterMicroButton and MainMenuMicroButton) then
    return false
end

local M = {}
ESURV_ACTION_BAR = M

-- ── The layout, in the stock bar's units ─────────────────────────────────────────────────────
-- Button `i` sits at 8 + (i - 1) × 42 as in the stock bar; the eighth ends at 338. The stock art
-- strips are 256 wide: strip 0 holds the bar's left end and buttons 1 to 6, strip 1 buttons 7 to
-- 12 and, at its last 6, the pillar before the arrows, strip 2 the arrows, the page number and
-- the micro buttons, strip 3 the latency bar and the bag slots.
M.BUTTONS = 8
local SLOTS_END = 338
local PILLAR = 6
-- where strip 2 starts: the arrows' centre 10 in, the page number 30, the first micro button 40
local ARROWS_AT = SLOTS_END + PILLAR
local MICRO_STEP = 26
local MICRO_WIDTH = 29
local MICRO_BUTTONS = 4
local MICRO_END = ARROWS_AT + 40 + (MICRO_BUTTONS - 1) * MICRO_STEP + MICRO_WIDTH
-- the bar ends just past the last micro button, where the right gryphon's body starts
M.WIDTH = MICRO_END + 3
-- the gryphons overlap each end by 32, as the stock caps do
local CAP_OVERLAP = 32
-- the experience bar's segments: ten, of a tenth each
M.XP_SEGMENTS = 10

-- the art rows of strips 1 and 2 (MainMenuBar.xml's TexCoords)
local STRIP1_TOP, STRIP1_BOTTOM = 0.58203125, 0.75
local STRIP2_TOP, STRIP2_BOTTOM = 0.33203125, 0.5

-- ── Keeping the stock layout ─────────────────────────────────────────────────────────────────
-- Each piece the run moves, as the stock file left it: its anchors, size, art rows and whether
-- it showed, read once before the first change.
local stock = {}
-- the main menu micro button's stock tooltip while the run's "Options" stands in for it (false:
-- it had none)
local stock_menu_tooltip = nil

local function remember(region)
    if stock[region] then
        return
    end
    local s = { points = {}, width = region:GetWidth(), height = region:GetHeight(),
        shown = region:IsShown() }
    for n = 1, (region:GetNumPoints() or 0) do
        local point, to, to_point, x, y = region:GetPoint(n)
        table.insert(s.points, { point, to, to_point, x, y })
    end
    if region.GetTexCoord then
        s.tex = { region:GetTexCoord() }
    end
    stock[region] = s
end

local function restore(region, s)
    region:ClearAllPoints()
    for _, p in ipairs(s.points) do
        region:SetPoint(p[1], p[2], p[3], p[4], p[5])
    end
    region:SetWidth(s.width)
    region:SetHeight(s.height)
    if s.tex then
        region:SetTexCoord(s.tex[1], s.tex[2], s.tex[3], s.tex[4], s.tex[5], s.tex[6], s.tex[7],
            s.tex[8])
    end
    if s.shown then
        region:Show()
    else
        region:Hide()
    end
end

-- Puts `region` at `point` of `to`, `x` and `y` off, `width` wide (nil keeps it).
local function place(region, point, to, to_point, x, y, width)
    remember(region)
    region:ClearAllPoints()
    region:SetPoint(point, to, to_point, x, y)
    if width then
        region:SetWidth(width)
    end
end

local function hide(region)
    if region then
        remember(region)
        region:Hide()
    end
end

-- The pillar before the arrows, cut from strip 1's last 6 (made once, shown only in a run).
local function pillar()
    local t = EsurvActionBarPillar
    if not t then
        t = MainMenuBarArtFrame:CreateTexture("EsurvActionBarPillar", "ARTWORK")
        t:SetTexture("Interface\\MainMenuBar\\UI-MainMenuBar-Dwarf")
        t:SetTexCoord(1 - PILLAR / 256, 1, STRIP1_TOP, STRIP1_BOTTOM)
        t:SetHeight(43)
        t:SetWidth(PILLAR)
        t:SetPoint("BOTTOMLEFT", MainMenuBarArtFrame, "BOTTOMLEFT", SLOTS_END, 0)
    end
    return t
end

-- ── The compact bar ──────────────────────────────────────────────────────────────────────────
local function lay_out_compact()
    local art = MainMenuBarArtFrame
    remember(MainMenuBar)
    MainMenuBar:SetWidth(M.WIDTH)

    -- strip 0 whole, strip 1 up to the eighth button's frame, the pillar, then strip 2
    place(MainMenuBarTexture0, "BOTTOMLEFT", art, "BOTTOMLEFT", 0, 0)
    place(MainMenuBarTexture1, "BOTTOMLEFT", art, "BOTTOMLEFT", 256, 0, SLOTS_END - 256)
    MainMenuBarTexture1:SetTexCoord(0, (SLOTS_END - 256) / 256, STRIP1_TOP, STRIP1_BOTTOM)
    pillar():Show()
    local rest = M.WIDTH - ARROWS_AT
    place(MainMenuBarTexture2, "BOTTOMLEFT", art, "BOTTOMLEFT", ARROWS_AT, 0, rest)
    MainMenuBarTexture2:SetTexCoord(0, rest / 256, STRIP2_TOP, STRIP2_BOTTOM)
    hide(MainMenuBarTexture3)

    place(MainMenuBarLeftEndCap, "BOTTOM", art, "BOTTOMLEFT", -CAP_OVERLAP, 0)
    place(MainMenuBarRightEndCap, "BOTTOM", art, "BOTTOMRIGHT", CAP_OVERLAP, 0)

    place(ActionBarUpButton, "CENTER", art, "TOPLEFT", ARROWS_AT + 10, -22)
    place(ActionBarDownButton, "CENTER", art, "TOPLEFT", ARROWS_AT + 10, -42)
    place(MainMenuBarPageNumber, "CENTER", art, "TOPLEFT", ARROWS_AT + 30, -31.5)

    place(CharacterMicroButton, "BOTTOMLEFT", art, "BOTTOMLEFT", ARROWS_AT + 40, 2)
    place(MainMenuMicroButton, "BOTTOMLEFT", TalentMicroButton, "BOTTOMRIGHT", -3, 0)
    -- the last micro button opens the run's Escape menu, so its tooltip reads "Options" (with its
    -- key binding as the stock tooltip shows it)
    if stock_menu_tooltip == nil then
        stock_menu_tooltip = MainMenuMicroButton.tooltipText or false
    end
    MainMenuMicroButton.tooltipText = MicroButtonTooltipText and MicroButtonTooltipText("Options", "TOGGLEGAMEMENU") or "Options"

    -- the experience bar: strip 0's first five segments, then strip 3's last five
    local half = M.WIDTH / 2
    remember(MainMenuExpBar)
    MainMenuExpBar:SetWidth(M.WIDTH)
    place(MainMenuXPBarTexture0, "BOTTOMLEFT", MainMenuExpBar, "BOTTOMLEFT", 0, 3, half)
    place(MainMenuXPBarTexture3, "BOTTOMRIGHT", MainMenuExpBar, "BOTTOMRIGHT", 0, 3, half)
    hide(MainMenuXPBarTexture1)
    hide(MainMenuXPBarTexture2)
    -- and the rail that stands in for it at the level cap, likewise
    if MainMenuBarMaxLevelBar then
        remember(MainMenuBarMaxLevelBar)
        MainMenuBarMaxLevelBar:SetWidth(M.WIDTH)
        place(MainMenuMaxLevelBar0, "BOTTOMLEFT", MainMenuBarMaxLevelBar, "TOPLEFT", 0, 0, half)
        place(MainMenuMaxLevelBar3, "LEFT", MainMenuMaxLevelBar0, "RIGHT", 0, 0, half)
        hide(MainMenuMaxLevelBar1)
        hide(MainMenuMaxLevelBar2)
    end
    if ExhaustionTick_Update then
        ExhaustionTick_Update()
    end
end

local function lay_out_stock()
    for region, s in pairs(stock) do
        restore(region, s)
    end
    stock = {}
    if stock_menu_tooltip ~= nil then
        MainMenuMicroButton.tooltipText = stock_menu_tooltip or nil
        stock_menu_tooltip = nil
    end
    if EsurvActionBarPillar then
        EsurvActionBarPillar:Hide()
    end
    if ExhaustionTick_Update then
        ExhaustionTick_Update()
    end
end

local compact = false

-- The run turned on or off (ESURV_RUN_FRAMES_ON).
function M.apply()
    local on = ESURV_RUN_FRAMES_ON and true or false
    if on == compact then
        return
    end
    compact = on
    if on then
        lay_out_compact()
    else
        lay_out_stock()
    end
    if UIParent_ManageFramePositions then
        UIParent_ManageFramePositions()
    end
end

return true
