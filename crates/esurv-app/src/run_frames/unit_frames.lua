-- The run's player and target frames (docs/game/screens.md, "The player and target frames").
-- esurv-app's `run_frames::unit_frames` runs this once in each interface VM, right after the
-- minimap's Lua; the run frames' Lua calls `ESURV_UNIT_FRAMES.apply()` whenever the run turns on
-- or off.
--
-- In a run both frames are recreated in our own art, after a choice of mock-up: the
-- portrait larger, round in a bronze ring, the level in a small bronze diamond hung on the ring,
-- and beside it a slim dark plate in a bronze edge holding the name on a strip (night blue, red
-- for a foe), the health bar and the power bar, their numbers always shown and every text
-- larger. The target's frame is the player's mirrored. Left out: the stock border, the rest and
-- combat icons, the status glow and the PvP icon. The art is esurv-app's `stock_look::BAR_ART`.
--
-- Out of a run every piece is put back as the stock files laid it out, and ours hidden.
--
-- Returns true once installed, false while the stock frames are not loaded yet.

if ESURV_UNIT_FRAMES then
    return true
end
if not (PlayerFrame and PlayerPortrait and PlayerFrameHealthBar and PlayerFrameManaBar
        and PlayerName and PlayerLevelText and TargetFrame and TargetPortrait
        and TargetFrameHealthBar and TargetFrameManaBar and TargetName and TargetLevelText
        and TargetFrameNameBackground) then
    return false
end

local M = {}
ESURV_UNIT_FRAMES = M

-- ── The layout, in the screen's units, from the frame's outer top corner ────────────────────
-- the frame: its size, and how far from the screen's top left (the target's beside it)
M.WIDTH, M.HEIGHT = 300, 96
local PLAYER_X, TARGET_X, TOP = 8, 320, 8
-- Both frames shrink together as far as the screen's width needs to keep the target clear of
-- the HUD's top ribbon, this far from its end, but never below
-- MIN_SCALE; `ESURV_TOP_RIBBON_HALF` the ribbon's half width, from esurv-app's `hud::top_box`.
local RIBBON_GAP, MIN_SCALE = 12, 0.75
-- The classic layout a fifth smaller, or smaller still where
-- the screen is too narrow for it beside the ribbon.
local CLASSIC_SCALE = 0.8
-- The modern layout: either side of the
-- screen's middle, low over the hero, mirrored. The layout chosen is kept in LAYOUT_CVAR, which
-- the minimap's layout button flips (`minimap_buttons.lua`).
local MODERN_SCALE, MODERN_GAP, MODERN_UP = 0.7, 0.08, 0.31
local LAYOUT_CVAR = "esurvFramesLayout"
if RegisterCVar then
    RegisterCVar(LAYOUT_CVAR, "classic")
end
-- the portrait, larger than the stock 64, and its ring:
-- the ring's art 128 square with the portrait inside its radius 50
M.PORTRAIT = 80
local PORTRAIT_IN = 8
local RING_ART, RING_HOLE = 128, 100
-- the plate: from the ring's middle out, its art (256 by 64) with its body from (8, 8) to
-- (248, 56)
local PLATE_IN, PLATE_TOP, PLATE_W, PLATE_H = 66, 18, 228, 60
-- the plate's edge left out this far into its art, under the ring
local EDGE_CUT = 28
-- the rows on the plate, from its body's top: name, health, power, filling it, their inner
-- ends under the ring and their outer ends at the edge's inside, which is drawn over them
local ROW_IN, ROW_END = 26, 4
-- the name this far into its strip, clear of the ring
local NAME_IN = 20
-- the power bar's name tab and pips (`class_bar.lua`) this far in from the bar and down from it
M.CLASS_BAR_X, M.CLASS_BAR_Y = 4, -6
M.NAME_TOP, M.NAME_H = 5, 17
M.HEALTH_TOP, M.HEALTH_H = 24, 17
M.POWER_TOP, M.POWER_H = 43, 12
-- the texts' sizes, past the stock 10 and 14
M.NAME_SIZE, M.HEALTH_SIZE, M.POWER_SIZE, M.LEVEL_SIZE = 14, 15, 12, 17
-- the level's diamond, on the ring's lower outer side; it and its number 30% larger than at first
local LEVEL = 39
-- the name strip's colours: night blue for a friend, red for a foe
local NIGHT = { 0.13, 0.2, 0.42 }
local FOE = { 0.5, 0.09, 0.07 }

local ART = "Interface\\ESurv\\"

-- ── Keeping the stock layout (as `minimap.lua`) ───────────────────────────────────────────────
local stock = {}

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
    stock[region] = s
end

local function restore(region, s)
    region:ClearAllPoints()
    for _, p in ipairs(s.points) do
        region:SetPoint(p[1], p[2], p[3], p[4], p[5])
    end
    region:SetWidth(s.width)
    region:SetHeight(s.height)
    if s.shown then
        region:Show()
    else
        region:Hide()
    end
end

-- Hidden in a run whatever shows it, shown again (if the stock files show it) out of one.
local kept_hidden = {}

local function keep_hidden(region)
    if not region or kept_hidden[region] then
        return
    end
    remember(region)
    kept_hidden[region] = true
    region.Show = function() end
    region:Hide()
end

local function let_show()
    for region in pairs(kept_hidden) do
        region.Show = nil
    end
    kept_hidden = {}
end

-- A text's font as the stock file left it, and the text in a size of ours.
local stock_font = {}

local function size_text(text, size)
    if not text then
        return
    end
    local file, height, flags = text:GetFont()
    if not file then
        return
    end
    if not stock_font[text] then
        stock_font[text] = { file, height, flags }
    end
    text:SetFont(file, size, flags)
end

-- ── The two frames ───────────────────────────────────────────────────────────────────────────
-- Each frame's parts, the target's mirrored (`side` -1: the portrait at the right).
local FRAMES = {
    {
        key = "player", frame = PlayerFrame, x = PLAYER_X, side = 1,
        portrait = PlayerPortrait, health = PlayerFrameHealthBar, power = PlayerFrameManaBar,
        name = PlayerName, level = PlayerLevelText,
        health_text = PlayerFrameHealthBarText, power_text = PlayerFrameManaBarText,
        hidden = { PlayerFrameTexture, PlayerFrameBackground, PlayerStatusTexture,
            PlayerAttackBackground, PlayerPVPIcon, PlayerRestIcon, PlayerAttackIcon,
            PlayerStatusGlow, PlayerRestGlow, PlayerAttackGlow, PlayerLeaderIcon,
            PlayerMasterIcon },
    },
    {
        key = "target", frame = TargetFrame, x = TARGET_X, side = -1,
        portrait = TargetPortrait, health = TargetFrameHealthBar, power = TargetFrameManaBar,
        name = TargetName, level = TargetLevelText,
        health_text = TargetFrameHealthBarText, power_text = TargetFrameManaBarText,
        hidden = { TargetFrameTexture, TargetFrameBackground, TargetFrameNameBackground,
            TargetPVPIcon, TargetLeaderIcon },
    },
}

-- `region` at (x, y) from the frame's outer top corner, `w` wide: for the target, mirrored.
local function at(region, f, point, x, y)
    region:ClearAllPoints()
    if f.side > 0 then
        region:SetPoint("TOP" .. point, f.frame, "TOPLEFT", x, -y)
    else
        local mirrored = (point == "LEFT" and "RIGHT") or (point == "RIGHT" and "LEFT") or point
        region:SetPoint("TOP" .. mirrored, f.frame, "TOPRIGHT", -x, -y)
    end
end

-- A texture's art the right way round for the frame's side.
local function face(texture, f)
    if f.side > 0 then
        texture:SetTexCoord(0, 1, 0, 1)
    else
        texture:SetTexCoord(1, 0, 0, 1)
    end
end

local function make_ours(f)
    if f.ours then
        return
    end
    local name = "EsurvUnitFrame" .. (f.key == "player" and "Player" or "Target")
    local o = {}
    -- the plate under the bars, the name's strip on it
    o.plate = f.frame:CreateTexture(name .. "Plate", "BACKGROUND")
    o.plate:SetTexture(ART .. "UfPlate.tga")
    face(o.plate, f)
    o.strip = f.frame:CreateTexture(name .. "Strip", "BORDER")
    o.strip:SetTexture(ART .. "UfName.tga")
    -- the plate's edge over the bars and the strip, the same art hollow
    local bars = math.max(f.health:GetFrameLevel(), f.power:GetFrameLevel())
    o.edge_frame = CreateFrame("Frame", name .. "Edge", f.frame)
    o.edge_frame:SetFrameLevel(bars + 1)
    o.edge_frame:SetAllPoints(f.frame)
    o.edge = o.edge_frame:CreateTexture(name .. "EdgeArt", "ARTWORK")
    o.edge:SetTexture(ART .. "UfPlateEdge.tga")
    -- the ring over the portrait and the plate's end, the level's diamond over the ring, under
    -- the stock texts
    o.over = CreateFrame("Frame", name .. "Ring", f.frame)
    o.over:SetFrameLevel(bars + 2)
    o.over:SetAllPoints(f.frame)
    o.ring = o.over:CreateTexture(name .. "RingArt", "ARTWORK")
    o.ring:SetTexture(ART .. "UfRing.tga")
    o.diamond = o.over:CreateTexture(name .. "Level", "OVERLAY")
    o.diamond:SetTexture(ART .. "UfLevel.tga")
    -- the stock frame has no number on the target's health: ours, as the mock-up drew it ("72%",
    -- or "240 / 240" where the game knows the unit's own)
    if not f.health_text then
        o.health_text = o.over:CreateFontString(name .. "HealthText", "OVERLAY", "TextStatusBarText")
        o.health_text:SetPoint("CENTER", f.health, "CENTER", 0, 0)
        local bar = f.health
        o.over:SetScript("OnUpdate", function()
            local low, high = bar:GetMinMaxValues()
            local value = bar:GetValue()
            if not (UnitExists and UnitExists(f.key)) or not high or high <= 0 or value <= 0 then
                o.health_text:SetText("")
            elseif high == 100 then
                o.health_text:SetText(string.format("%d%%", value))
            else
                o.health_text:SetText(string.format("%d / %d", value, high))
            end
        end)
    end
    f.ours = o
end

local function lay_out_ours(f)
    make_ours(f)
    local o = f.ours
    local frame = f.frame
    remember(frame)
    frame:ClearAllPoints()
    frame:SetPoint("TOPLEFT", UIParent, "TOPLEFT", f.x, -TOP)
    frame:SetWidth(M.WIDTH)
    -- the target's buffs hang from its foot (`TargetFrame.lua`): under the plate
    frame:SetHeight(f.side > 0 and M.HEIGHT or PLATE_TOP + PLATE_H + 36)
    for _, region in ipairs(f.hidden) do
        keep_hidden(region)
    end

    -- the portrait and its ring
    local middle = PORTRAIT_IN + M.PORTRAIT / 2
    remember(f.portrait)
    at(f.portrait, f, "LEFT", PORTRAIT_IN, PORTRAIT_IN)
    f.portrait:SetWidth(M.PORTRAIT)
    f.portrait:SetHeight(M.PORTRAIT)
    local ring = M.PORTRAIT * RING_ART / RING_HOLE
    o.ring:SetWidth(ring)
    o.ring:SetHeight(ring)
    o.ring:ClearAllPoints()
    o.ring:SetPoint("CENTER", f.portrait, "CENTER", 0, 0)

    -- the plate, its art's margins past its body
    local sx, sy = PLATE_W / 240, PLATE_H / 48
    o.plate:SetWidth(256 * sx)
    o.plate:SetHeight(64 * sy)
    at(o.plate, f, "LEFT", PLATE_IN - 8 * sx, PLATE_TOP - 8 * sy)
    -- the edge from where the ring covers its ends on: its square end would cross the portrait
    local cut = EDGE_CUT / 256
    o.edge:SetWidth((256 - EDGE_CUT) * sx)
    o.edge:SetHeight(64 * sy)
    at(o.edge, f, "LEFT", PLATE_IN + (EDGE_CUT - 8) * sx, PLATE_TOP - 8 * sy)
    if f.side > 0 then
        o.edge:SetTexCoord(cut, 1, 0, 1)
    else
        o.edge:SetTexCoord(1, cut, 0, 1)
    end

    -- the rows
    local row_x = PLATE_IN + ROW_IN
    local row_w = PLATE_W - ROW_IN - ROW_END
    o.strip:SetWidth(row_w)
    o.strip:SetHeight(M.NAME_H)
    at(o.strip, f, "LEFT", row_x, PLATE_TOP + M.NAME_TOP)
    for _, row in ipairs({ { f.health, M.HEALTH_TOP, M.HEALTH_H }, { f.power, M.POWER_TOP, M.POWER_H } }) do
        remember(row[1])
        row[1]:SetWidth(row_w)
        row[1]:SetHeight(row[3])
        at(row[1], f, "LEFT", row_x, PLATE_TOP + row[2])
    end
    remember(f.name)
    f.name:ClearAllPoints()
    f.name:SetPoint(f.side > 0 and "LEFT" or "RIGHT", o.strip, f.side > 0 and "LEFT" or "RIGHT",
        NAME_IN * f.side, 0)
    f.name:SetWidth(row_w - NAME_IN - 8)
    if f.name.SetJustifyH then
        f.name:SetJustifyH(f.side > 0 and "LEFT" or "RIGHT")
    end
    for _, t in ipairs({ { f.health_text, f.health }, { f.power_text, f.power } }) do
        if t[1] then
            remember(t[1])
            t[1]:ClearAllPoints()
            t[1]:SetPoint("CENTER", t[2], "CENTER", 0, 0)
        end
    end

    -- the level's diamond on the ring's lower outer side, the level on it
    o.diamond:SetWidth(LEVEL)
    o.diamond:SetHeight(LEVEL)
    o.diamond:ClearAllPoints()
    local off = M.PORTRAIT / 2 * 0.72
    o.diamond:SetPoint("CENTER", f.portrait, "CENTER", -off * f.side, -off)
    remember(f.level)
    f.level:ClearAllPoints()
    f.level:SetPoint("CENTER", o.diamond, "CENTER", 0, 0)

    -- the larger texts
    size_text(f.name, M.NAME_SIZE)
    size_text(f.health_text, M.HEALTH_SIZE)
    size_text(f.power_text, M.POWER_SIZE)
    size_text(f.level, M.LEVEL_SIZE)
    size_text(o.health_text, M.HEALTH_SIZE)
    size_text(ESURV_CLASS_BAR and f.key == "player" and EsurvClassBarText, M.POWER_SIZE)

    -- the stock texts' frame over our ring and diamond, so the level shows on the diamond
    local texts = f.name:GetParent()
    if texts and texts.SetFrameLevel then
        if not o.texts_level then
            o.texts_level = texts:GetFrameLevel()
        end
        texts:SetFrameLevel(o.over:GetFrameLevel() + 1)
    end

    -- the bars' numbers always shown, as the mock-up drew them, without the stock "Health" and
    -- "Mana" before them
    if ShowTextStatusBarText and not o.numbers then
        ShowTextStatusBarText(f.health)
        ShowTextStatusBarText(f.power)
        o.numbers = true
    end
    o.prefixes = o.prefixes or {}
    for _, bar in ipairs({ f.health, f.power }) do
        if bar.prefix then
            o.prefixes[bar] = bar.prefix
            bar.prefix = nil
        end
        if TextStatusBar_UpdateTextString then
            TextStatusBar_UpdateTextString(bar)
        end
    end
    o.plate:Show()
    o.edge_frame:Show()
    o.strip:Show()
    o.over:Show()
end

-- The target's own pieces seated on ours: the skull of a far higher level on the diamond, the
-- raid mark over the portrait, the dead text on the health bar.
local function seat_target_bits(f)
    local o = f.ours
    if TargetHighLevelTexture then
        remember(TargetHighLevelTexture)
        TargetHighLevelTexture:ClearAllPoints()
        TargetHighLevelTexture:SetPoint("CENTER", o.diamond, "CENTER", 0, 0)
    end
    if TargetRaidTargetIcon then
        remember(TargetRaidTargetIcon)
        TargetRaidTargetIcon:ClearAllPoints()
        TargetRaidTargetIcon:SetPoint("CENTER", f.portrait, "TOP", 0, 0)
    end
    if TargetDeadText then
        remember(TargetDeadText)
        TargetDeadText:ClearAllPoints()
        TargetDeadText:SetPoint("CENTER", f.health, "CENTER", 0, 0)
    end
end

local function hide_ours(f)
    local o = f.ours
    if not o then
        return
    end
    o.plate:Hide()
    o.edge_frame:Hide()
    o.strip:Hide()
    o.over:Hide()
    if o.texts_level then
        f.name:GetParent():SetFrameLevel(o.texts_level)
    end
    for bar, prefix in pairs(o.prefixes or {}) do
        bar.prefix = prefix
        if TextStatusBar_UpdateTextString then
            TextStatusBar_UpdateTextString(bar)
        end
    end
    o.prefixes = nil
    if o.numbers and HideTextStatusBarText then
        -- the stock function reads `this` as the bar
        local was = this
        for _, bar in ipairs({ f.health, f.power }) do
            this = bar
            HideTextStatusBarText(bar)
        end
        this = was
        o.numbers = nil
    end
end

local on = false

-- The scale both frames take on this screen.
function M.fit_scale(screen_width)
    local room = screen_width / 2 - (ESURV_TOP_RIBBON_HALF or 170) - RIBBON_GAP
    return math.max(MIN_SCALE, math.min(1, room / (TARGET_X + M.WIDTH)))
end

-- Which layout the frames take, kept in the CVar the minimap's layout button flips.
function M.layout()
    return GetCVar and GetCVar(LAYOUT_CVAR) == "modern" and "modern" or "classic"
end

-- Each frame placed and scaled for the layout: the classic at the screen's top left, shrunk to
-- fit beside the ribbon; the modern smaller, either side of the screen's middle, the player's
-- plate ending and the target's beginning MODERN_GAP of the screen's width from it, both
-- portraits' middles MODERN_UP of its height over its foot.
local arranged
local function arrange()
    local width, height, layout = UIParent:GetWidth(), UIParent:GetHeight(), M.layout()
    local key = width .. "x" .. height .. layout
    if key == arranged then
        return
    end
    arranged = key
    local modern = layout == "modern"
    local scale = modern and MODERN_SCALE or math.min(CLASSIC_SCALE, M.fit_scale(width))
    for _, f in ipairs(FRAMES) do
        f.stock_scale = f.stock_scale or f.frame:GetScale()
        f.frame:SetScale(scale)
        f.frame:ClearAllPoints()
        if modern then
            -- the offsets in the frame's own scaled units
            local x = MODERN_GAP * width / scale
            local y = MODERN_UP * height / scale + PORTRAIT_IN + M.PORTRAIT / 2
            if f.side > 0 then
                f.frame:SetPoint("TOPRIGHT", UIParent, "BOTTOM", -x, y)
            else
                f.frame:SetPoint("TOPLEFT", UIParent, "BOTTOM", x, y)
            end
        else
            f.frame:SetPoint("TOPLEFT", UIParent, "TOPLEFT", f.x, -TOP)
        end
    end
end

-- The layout chosen ("classic" or "modern"), kept, and taken at once in a run.
function M.set_layout(layout)
    if SetCVar then
        SetCVar(LAYOUT_CVAR, layout == "modern" and "modern" or "classic")
    end
    if on then
        arrange()
    end
end

local function unfit()
    arranged = nil
    for _, f in ipairs(FRAMES) do
        if f.stock_scale then
            f.frame:SetScale(f.stock_scale)
            f.stock_scale = nil
        end
    end
end

-- arranged again as the screen's size changes
local fitter = CreateFrame("Frame", "EsurvUnitFramesFit", UIParent)
fitter:SetScript("OnUpdate", function()
    if on then
        arrange()
    end
end)

-- The name strip's colour: red for a unit that would fight the hero, night blue for the rest.
local function tint_strips()
    if not on then
        return
    end
    for _, f in ipairs(FRAMES) do
        if f.ours then
            local c = NIGHT
            if f.key == "target" and UnitExists and UnitExists("target") and UnitCanAttack
                    and UnitCanAttack("player", "target") then
                c = FOE
            end
            f.ours.strip:SetVertexColor(c[1], c[2], c[3], 1)
        end
    end
end

-- The stock files set a bar's "Mana" or "Rage" anew when the power changes: kept for the stock
-- frames while ours show.
if SetTextStatusBarTextPrefix then
    local stock_prefix = SetTextStatusBarTextPrefix
    SetTextStatusBarTextPrefix = function(bar, prefix)
        for _, f in ipairs(FRAMES) do
            local o = f.ours
            if on and o and o.prefixes and (bar == f.health or bar == f.power) then
                o.prefixes[bar] = prefix
                return
            end
        end
        stock_prefix(bar, prefix)
    end
end

-- The stock file tints its own name strip at every new target: ours after it.
if TargetFrame_CheckFaction then
    local stock_check = TargetFrame_CheckFaction
    TargetFrame_CheckFaction = function(a, b, c)
        stock_check(a, b, c)
        tint_strips()
    end
end

local function lay_out_stock()
    let_show()
    for region, s in pairs(stock) do
        restore(region, s)
    end
    stock = {}
    for text, s in pairs(stock_font) do
        text:SetFont(s[1], s[2], s[3])
    end
    stock_font = {}
    for _, f in ipairs(FRAMES) do
        hide_ours(f)
        if f.name.SetJustifyH then
            f.name:SetJustifyH("CENTER")
        end
    end
    -- the stock files decide again what shows on the target's frame
    if UnitExists and UnitExists("target") and TargetFrame_Update then
        local was = this
        this = TargetFrame
        TargetFrame_Update()
        this = was
    end
end

-- Whether ours shows.
function M.on()
    return on
end

-- The run turned on or off (ESURV_RUN_FRAMES_ON).
function M.apply()
    local want = ESURV_RUN_FRAMES_ON and true or false
    if want == on then
        return
    end
    on = want
    if on then
        for _, f in ipairs(FRAMES) do
            lay_out_ours(f)
        end
        seat_target_bits(FRAMES[2])
        tint_strips()
        arrange()
    else
        lay_out_stock()
        unfit()
    end
    -- the power bar's name tab and pips (`class_bar.lua`) under the plate's edge, clear of the ring
    if ESURV_CLASS_BAR and ESURV_CLASS_BAR.place then
        if on then
            ESURV_CLASS_BAR.place(M.CLASS_BAR_X, M.CLASS_BAR_Y)
        else
            ESURV_CLASS_BAR.place(0, 0)
        end
    end
end

return true
