-- The run's action bar (docs/game/screens.md, "The action bar"). esurv-app's
-- `run_frames::action_bar` runs this once in each interface VM, right after the run frames'
-- Lua, which calls `ESURV_ACTION_BAR.apply()` whenever the run turns on or off.
--
-- In a run the stock main bar (MainMenuBar.xml, 1024 wide) is recreated in our own art, after a mock-up; the art is
-- esurv-app's `stock_look::BAR_ART`): eight buttons (seven spells, the race spell on the eighth)
-- in one bronze frame, each in its own dark well and bronze rim, centred, a dragon at each end
-- (`stock_look.lua`); the experience bar over it, seated on the frame's top edge, in a bronze
-- frame ticked in tenths; and apart, at the screen's bottom right, four micro buttons in a frame
-- of their own (character, the hero's portrait in a bronze frame; spellbook, a book; talents, a
-- target; main menu, a computer, its tooltip "Options"). The bar's frame and the micro buttons' sit
-- at one distance from the screen's foot.
-- Left out: the stock strips, the page arrows and number, the
-- slots past the eighth and the bag slots (ActionButton9 to 12, the bag buttons and the latency
-- bar the run frames hide).
--
-- Out of a run every piece is put back as the stock file laid it out, and ours hidden.
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
-- Button `i` sits at 8 + (i - 1) × 42 as in the stock bar; the eighth ends at 338.
M.BUTTONS = 8
local SLOTS_END = 338
-- the frame round the buttons: this far out from them, its edge this thick
local TRAY_PAD = 5
local EDGE = 8
-- the bar ends as far past the eighth button as the first starts in
M.WIDTH = SLOTS_END + 8
-- the frame's foot and the micro buttons' frame's, above the screen's foot
M.BOTTOM_GAP = 6
-- the micro buttons: their size (the art's shape), the step between them, their frame's padding,
-- and how far in from the screen's right edge it sits
local MICRO_WIDTH, MICRO_HEIGHT = 28, 38
local MICRO_GAP = 1
local MICRO_PAD = 5
M.MICRO_RIGHT = 8
-- each button's rim, this much past the button on each side
local RIM_OUT = 2
-- the experience bar: its height, how far in from the frame's ends, how far down over its edge
M.XP_HEIGHT = 10
local XP_IN = 14
local XP_DOWN = 6
-- the experience frame's art (1024 by 32): the bar's window in it
local XP_ART_W, XP_ART_H = 1024, 32
local XP_WIN_L, XP_WIN_R, XP_WIN_T, XP_WIN_B = 14, 1010, 7, 25
-- the experience bar's ticks: ten segments of a tenth each, drawn in the frame's art
M.XP_SEGMENTS = 10
-- the dragons' caps (`stock_look.lua` draws a dragon on each) overlap each end by this much
local CAP_OVERLAP = 38

-- our art (esurv-app's `stock_look::BAR_ART`)
local ART = "Interface\\ESurv\\"
-- the micro buttons' art: the button in the top 172 of its 256 rows
local MICRO_ART_BOTTOM = 172 / 256

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

-- A button's state art as the stock file left it (normal, pushed, disabled, highlight) and its
-- hit insets.
local stock_art = {}
local STATES = { "Normal", "Pushed", "Disabled", "Highlight" }

local function remember_art(button)
    if stock_art[button] then
        return
    end
    local s = {}
    for _, state in ipairs(STATES) do
        local t = button["Get" .. state .. "Texture"](button)
        if t then
            s[state] = { path = t:GetTexture(), tex = { t:GetTexCoord() } }
        end
    end
    s.insets = { button:GetHitRectInsets() }
    stock_art[button] = s
end

local function restore_art(button, s)
    for _, state in ipairs(STATES) do
        local was = s[state]
        if was then
            button["Set" .. state .. "Texture"](button, was.path)
            local t = button["Get" .. state .. "Texture"](button)
            local c = was.tex
            t:SetTexCoord(c[1], c[2], c[3], c[4], c[5], c[6], c[7], c[8])
            t:SetVertexColor(1, 1, 1, 1)
        end
    end
    local i = s.insets
    button:SetHitRectInsets(i[1] or 0, i[2] or 0, i[3] or 0, i[4] or 0)
end

-- ── Our pieces ───────────────────────────────────────────────────────────────────────────────
-- Made on the first run, shown only in a run.
local ours = {}

-- A frame of `art` round `frame`, nine-sliced: its corners a quarter of the art, `edge` wide.
local function nine_slice(frame, art, edge, layer)
    local q = 0.25
    local corners = {
        { "TOPLEFT", 0, q, 0, q }, { "TOPRIGHT", 1 - q, 1, 0, q },
        { "BOTTOMLEFT", 0, q, 1 - q, 1 }, { "BOTTOMRIGHT", 1 - q, 1, 1 - q, 1 },
    }
    for _, c in ipairs(corners) do
        local t = frame:CreateTexture(nil, layer)
        t:SetTexture(art)
        t:SetTexCoord(c[2], c[3], c[4], c[5])
        t:SetWidth(edge)
        t:SetHeight(edge)
        t:SetPoint(c[1], frame, c[1], 0, 0)
    end
    -- the sides, between the corners: from `tl` of the frame to `br`, cut l to r, t to b
    local function side(tl, tl_x, tl_y, br, br_x, br_y, l, r, t, b)
        local s = frame:CreateTexture(nil, layer)
        s:SetTexture(art)
        s:SetTexCoord(l, r, t, b)
        s:SetPoint("TOPLEFT", frame, tl, tl_x, tl_y)
        s:SetPoint("BOTTOMRIGHT", frame, br, br_x, br_y)
    end
    side("TOPLEFT", edge, 0, "TOPRIGHT", -edge, -edge, q, 1 - q, 0, q)
    side("BOTTOMLEFT", edge, edge, "BOTTOMRIGHT", -edge, 0, q, 1 - q, 1 - q, 1)
    side("TOPLEFT", 0, -edge, "BOTTOMLEFT", edge, edge, 0, q, q, 1 - q)
    side("TOPRIGHT", -edge, -edge, "BOTTOMRIGHT", 0, edge, 1 - q, 1, q, 1 - q)
end

-- A small shadow round a frame's outline: `BarShadow.tga` nine-sliced, its corners SHADOW_EDGE wide with the
-- outline half way in, so it stands out that far round the frame, dropped a little.
local SHADOW_EDGE, SHADOW_DROP = 16, 2
local function shadow(frame)
    local s = CreateFrame("Frame", nil, frame)
    -- under the frame and all it holds
    s:SetFrameLevel(math.max(0, frame:GetFrameLevel() - 1))
    local out = SHADOW_EDGE / 2
    s:SetPoint("TOPLEFT", frame, "TOPLEFT", -out, out - SHADOW_DROP)
    s:SetPoint("BOTTOMRIGHT", frame, "BOTTOMRIGHT", out, -out - SHADOW_DROP)
    nine_slice(s, ART .. "BarShadow.tga", SHADOW_EDGE, "BACKGROUND")
end

-- The night-dark fill inside a frame.
local function fill(frame)
    local t = frame:CreateTexture(nil, "BORDER")
    t:SetTexture(0.055, 0.035, 0.02, 0.85)
    t:SetPoint("TOPLEFT", frame, "TOPLEFT", 3, -3)
    t:SetPoint("BOTTOMRIGHT", frame, "BOTTOMRIGHT", -3, 3)
end

local function make_ours()
    if ours.tray then
        return
    end
    local level = MainMenuBar:GetFrameLevel()
    -- the frame round the buttons, under them: its fill and each button's well
    local tray = CreateFrame("Frame", "EsurvActionBarTray", MainMenuBar)
    tray:SetFrameLevel(level)
    tray:SetPoint("TOPLEFT", ActionButton1, "TOPLEFT", -TRAY_PAD, TRAY_PAD)
    tray:SetPoint("BOTTOMRIGHT", getglobal("ActionButton" .. M.BUTTONS), "BOTTOMRIGHT", TRAY_PAD,
        -TRAY_PAD)
    fill(tray)
    shadow(tray)
    -- over the buttons: the frame's edge and each button's rim
    local over = CreateFrame("Frame", "EsurvActionBarRims", MainMenuBar)
    over:SetAllPoints(tray)
    over:SetFrameLevel(ActionButton1:GetFrameLevel() + 2)
    nine_slice(over, ART .. "BarFrame.tga", EDGE, "ARTWORK")
    for i = 1, M.BUTTONS do
        local b = getglobal("ActionButton" .. i)
        local well = tray:CreateTexture("EsurvActionBarWell" .. i, "ARTWORK")
        well:SetTexture(ART .. "BarWell.tga")
        well:SetAllPoints(b)
        local rim = over:CreateTexture("EsurvActionBarRim" .. i, "OVERLAY")
        rim:SetTexture(ART .. "BarRim.tga")
        rim:SetPoint("TOPLEFT", b, "TOPLEFT", -RIM_OUT, RIM_OUT)
        rim:SetPoint("BOTTOMRIGHT", b, "BOTTOMRIGHT", RIM_OUT, -RIM_OUT)
    end
    -- the experience bar's frame, over the bar (its spark over that, `stock_look.lua`)
    local xp = CreateFrame("Frame", "EsurvXpFrame", MainMenuExpBar)
    xp:SetAllPoints(MainMenuExpBar)
    xp:SetFrameLevel(MainMenuExpBar:GetFrameLevel() + 2)
    ours.xp_art = xp:CreateTexture("EsurvXpFrameArt", "ARTWORK")
    ours.xp_art:SetTexture(ART .. "XpFrame.tga")
    -- the micro buttons' frame, at the screen's bottom right
    local micro = CreateFrame("Frame", "EsurvMicroFrame", MainMenuBar)
    micro:SetFrameLevel(level)
    micro:SetWidth(4 * MICRO_WIDTH + 3 * MICRO_GAP + 2 * MICRO_PAD)
    micro:SetHeight(MICRO_HEIGHT + 2 * MICRO_PAD)
    micro:SetPoint("BOTTOMRIGHT", UIParent, "BOTTOMRIGHT", -M.MICRO_RIGHT, M.BOTTOM_GAP)
    fill(micro)
    shadow(micro)
    local micro_edge = CreateFrame("Frame", "EsurvMicroFrameEdge", micro)
    micro_edge:SetAllPoints(micro)
    micro_edge:SetFrameLevel(CharacterMicroButton:GetFrameLevel() + 2)
    nine_slice(micro_edge, ART .. "BarFrame.tga", EDGE - 2, "ARTWORK")
    ours.tray, ours.over, ours.xp, ours.micro = tray, over, xp, micro
end

-- The experience frame's art round the bar, its window on the bar.
local function seat_xp_art()
    local w, h = MainMenuExpBar:GetWidth(), MainMenuExpBar:GetHeight()
    local sx = w / (XP_WIN_R - XP_WIN_L)
    local sy = h / (XP_WIN_B - XP_WIN_T)
    local art = ours.xp_art
    art:ClearAllPoints()
    art:SetPoint("TOPLEFT", MainMenuExpBar, "TOPLEFT", -XP_WIN_L * sx, XP_WIN_T * sy)
    art:SetWidth(XP_ART_W * sx)
    art:SetHeight(XP_ART_H * sy)
end

-- A micro button in our art, the `n`th in its frame: `picture` its art (nil: the character
-- button's frame, the hero's portrait showing through it).
local function micro_button(button, n, picture)
    remember_art(button)
    place(button, "BOTTOMLEFT", ours.micro, "BOTTOMLEFT",
        MICRO_PAD + (n - 1) * (MICRO_WIDTH + MICRO_GAP), MICRO_PAD, MICRO_WIDTH)
    button:SetHeight(MICRO_HEIGHT)
    button:SetHitRectInsets(0, 0, 0, 0)
    local path = ART .. (picture or "MicroPortrait.tga")
    -- held down a shade darker, unusable greyed
    local shades = { Normal = 1, Pushed = 0.7, Disabled = 0.45 }
    for state, shade in pairs(shades) do
        button["Set" .. state .. "Texture"](button, path)
        local t = button["Get" .. state .. "Texture"](button)
        t:SetTexCoord(0, 1, 0, MICRO_ART_BOTTOM)
        t:SetVertexColor(shade, shade, shade, 1)
    end
    button:SetHighlightTexture(ART .. "MicroGlow.tga")
    local glow = button:GetHighlightTexture()
    glow:SetTexCoord(0, 1, 0, MICRO_ART_BOTTOM)
    glow:SetBlendMode("ADD")
end

-- ── The compact bar ──────────────────────────────────────────────────────────────────────────
local function lay_out_compact()
    local art = MainMenuBarArtFrame
    remember(MainMenuBar)
    MainMenuBar:SetWidth(M.WIDTH)
    make_ours()

    -- the stock strips, arrows and page number gone; the buttons up so the frame's foot sits at
    -- the gap
    for i = 0, 3 do
        hide(getglobal("MainMenuBarTexture" .. i))
    end
    hide(ActionBarUpButton)
    hide(ActionBarDownButton)
    hide(MainMenuBarPageNumber)
    place(ActionButton1, "BOTTOMLEFT", art, "BOTTOMLEFT", 8, M.BOTTOM_GAP + TRAY_PAD)
    -- the stock buttons' own frames give way to our rims
    for i = 1, M.BUTTONS do
        local n = getglobal("ActionButton" .. i .. "NormalTexture")
        if n then
            n:SetAlpha(0)
        end
    end
    for _, frame in pairs({ ours.tray, ours.over, ours.micro, ours.xp }) do
        frame:Show()
    end

    place(MainMenuBarLeftEndCap, "BOTTOM", art, "BOTTOMLEFT", -CAP_OVERLAP, 0)
    place(MainMenuBarRightEndCap, "BOTTOM", art, "BOTTOMRIGHT", CAP_OVERLAP, 0)

    micro_button(CharacterMicroButton, 1, nil)
    micro_button(SpellbookMicroButton, 2, "MicroBook.tga")
    micro_button(TalentMicroButton, 3, "MicroTarget.tga")
    micro_button(MainMenuMicroButton, 4, "MicroPc.tga")
    -- the hero's portrait in the character button's frame's window
    if MicroButtonPortrait then
        local inset = 3.5
        remember(MicroButtonPortrait)
        MicroButtonPortrait:ClearAllPoints()
        MicroButtonPortrait:SetPoint("TOPLEFT", CharacterMicroButton, "TOPLEFT", inset, -inset)
        MicroButtonPortrait:SetPoint("BOTTOMRIGHT", CharacterMicroButton, "BOTTOMRIGHT", -inset,
            inset)
    end
    -- the last micro button opens the run's Escape menu, so its tooltip reads "Options" (with its
    -- key binding as the stock tooltip shows it)
    if stock_menu_tooltip == nil then
        stock_menu_tooltip = MainMenuMicroButton.tooltipText or false
    end
    MainMenuMicroButton.tooltipText = MicroButtonTooltipText and MicroButtonTooltipText("Options", "TOGGLEGAMEMENU") or "Options"

    -- the experience bar seated on the frame's top edge, the
    -- frame's width less its ends, in our frame; the stock strips and the level cap's rail gone
    remember(MainMenuExpBar)
    MainMenuExpBar:ClearAllPoints()
    MainMenuExpBar:SetPoint("BOTTOMLEFT", ours.tray, "TOPLEFT", XP_IN, -XP_DOWN)
    MainMenuExpBar:SetPoint("BOTTOMRIGHT", ours.tray, "TOPRIGHT", -XP_IN, -XP_DOWN)
    MainMenuExpBar:SetHeight(M.XP_HEIGHT)
    for i = 0, 3 do
        hide(getglobal("MainMenuXPBarTexture" .. i))
    end
    hide(MainMenuBarMaxLevelBar)
    seat_xp_art()
    if ExhaustionTick_Update then
        ExhaustionTick_Update()
    end
end

local function lay_out_stock()
    for region, s in pairs(stock) do
        restore(region, s)
    end
    stock = {}
    for button, s in pairs(stock_art) do
        restore_art(button, s)
    end
    stock_art = {}
    for i = 1, M.BUTTONS do
        local n = getglobal("ActionButton" .. i .. "NormalTexture")
        if n then
            n:SetAlpha(1)
        end
    end
    for _, frame in pairs({ ours.tray, ours.over, ours.micro, ours.xp }) do
        frame:Hide()
    end
    if stock_menu_tooltip ~= nil then
        MainMenuMicroButton.tooltipText = stock_menu_tooltip or nil
        stock_menu_tooltip = nil
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
    -- the bar's colour follows the new layout once the look is on (stock_look.lua)
    if ESURV_LOOK and ESURV_LOOK.tint_bar then
        ESURV_LOOK.tint_bar()
    end
    if UIParent_ManageFramePositions then
        UIParent_ManageFramePositions()
    end
end

return true
