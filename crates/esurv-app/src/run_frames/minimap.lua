-- The run's minimap (docs/game/screens.md, "The minimap"). esurv-app's `run_frames::minimap`
-- runs this once in each interface VM, right after the action bar's Lua; the run frames' Lua
-- calls `ESURV_MINIMAP.apply()` whenever the run turns on or off.
--
-- In a run the stock minimap is recreated in our own art, from a choice of mock-ups: the map a fifth larger than the stock one, in a
-- thin gold ring with a long bronze point at north, south, east and west, and over it a
-- night-blue ribbon in a bronze edge, its ends slanting in, with the zone's name and the game's
-- clock; the zoom buttons small gold marks on the rim. Left out: the stock border, its top and
-- the "N" it draws, the close button and the day and night button. The tracking, mail,
-- battleground and meeting stone buttons keep their stock places on the map's corners, and our
-- own two (`minimap_buttons.lua`) move out to the larger rim. The buffs move left, clear of the
-- ribbon. The art is esurv-app's `stock_look::BAR_ART`.
--
-- Out of a run every piece is put back as the stock file laid it out, and ours hidden.
--
-- Returns true once installed, false while the stock minimap is not loaded yet.

if ESURV_MINIMAP then
    return true
end
if not (Minimap and MinimapCluster and MinimapBorder and MinimapBackdrop and MinimapZoomIn
        and MinimapZoomOut and MinimapZoneTextButton and MinimapZoneText) then
    return false
end

local M = {}
ESURV_MINIMAP = M

-- ── The layout, in the screen's units ────────────────────────────────────────────────────────
-- the map a fifth larger than the stock 140
M.SCALE = 1.2
M.SIZE = 140 * M.SCALE
-- the ring's art: 256 square, the map's edge at its radius 98
local RING_ART, RING_MAP = 256, 196
-- the ribbon, as wide as the ring's points:
-- its size, how far from the screen's top right; its art's body (512 by 64) from row 12 to
-- row 50
M.RIBBON_WIDTH, M.RIBBON_HEIGHT = 200, 36
local RIBBON_RIGHT, RIBBON_TOP = 6, 2
-- the map's top under the ribbon, its middle under the ribbon's
M.MAP_TOP = 52
-- the zoom marks: their size, and where on the rim (degrees, 0 at the right, anticlockwise)
local MARK = 18
local ZOOM_IN_AT, ZOOM_OUT_AT = -28, -52
-- the zoom marks' and our own buttons' distance from the map's middle, past its rim
M.MARK_RADIUS = M.SIZE / 2 + 12
M.BUTTON_RADIUS = M.SIZE / 2 + 14
-- the buffs and the weapon enchants this much further left than the stock file has them
local BUFFS_LEFT = 12

local ART = "Interface\\ESurv\\"

-- ── Keeping the stock layout (as `action_bar.lua`) ────────────────────────────────────────────
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

local function hide(region)
    if region then
        remember(region)
        region:Hide()
    end
end

-- A zoom button's art as the stock file left it.
local stock_art = {}
local STATES = { "Normal", "Pushed", "Disabled" }

local function remember_art(button)
    if stock_art[button] then
        return
    end
    local s = {}
    for _, state in ipairs(STATES) do
        local t = button["Get" .. state .. "Texture"](button)
        if t then
            s[state] = t:GetTexture()
        end
    end
    stock_art[button] = s
end

-- The round buttons on the map: each one's ring and the icon it rings. Their stock ring art.
local BUTTON_RINGS = {
    { "MiniMapTrackingBorder", "MiniMapTrackingIcon" },
    { "MiniMapMailBorder", "MiniMapMailIcon" },
    { "MiniMapBattlefieldBorder", "MiniMapBattlefieldIcon" },
    { "MiniMapMeetingStoneBorder", "MiniMapMeetingStoneIcon" },
    { "EsurvMinimapBarsButtonBorder", "EsurvMinimapBarsButtonIcon" },
    { "EsurvMinimapMeterButtonBorder", "EsurvMinimapMeterButtonIcon" },
    { "EsurvMinimapLayoutButtonBorder", "EsurvMinimapLayoutButtonIcon" },
}
local stock_ring = {}

-- Each round button's icon cut to the circle inside it. Again on every frame while ours shows: the stock files set the
-- tracking and battleground icons anew, and a texture set anew is square again.
local function round_icons()
    for _, b in ipairs(BUTTON_RINGS) do
        local icon = getglobal(b[2])
        local path = icon and icon:GetTexture()
        if path then
            SetPortraitToTexture(b[2], path)
        end
    end
end

local function square_icons()
    for _, b in ipairs(BUTTON_RINGS) do
        local icon = getglobal(b[2])
        local path = icon and icon:GetTexture()
        if path then
            icon:SetTexture(path)
        end
    end
end

-- ── Our pieces ───────────────────────────────────────────────────────────────────────────────
local ours = {}

local function make_ours()
    if ours.ring then
        return
    end
    -- the ring over the map, under the buttons on it
    local ring = CreateFrame("Frame", "EsurvMinimapRing", Minimap)
    ring:SetFrameLevel(Minimap:GetFrameLevel() + 1)
    ring:SetAllPoints(Minimap)
    local art = ring:CreateTexture("EsurvMinimapRingArt", "ARTWORK")
    art:SetTexture(ART .. "MapRing.tga")
    local side = M.SIZE * RING_ART / RING_MAP
    art:SetWidth(side)
    art:SetHeight(side)
    art:SetPoint("CENTER", Minimap, "CENTER", 0, 0)
    -- the ribbon, the zone's name at its left and the clock at its right
    local ribbon = CreateFrame("Frame", "EsurvMinimapRibbon", MinimapCluster)
    ribbon:SetFrameLevel(Minimap:GetFrameLevel() + 2)
    ribbon:SetWidth(M.RIBBON_WIDTH)
    ribbon:SetHeight(M.RIBBON_HEIGHT)
    ribbon:SetPoint("TOPRIGHT", UIParent, "TOPRIGHT", -RIBBON_RIGHT, -RIBBON_TOP)
    local band = ribbon:CreateTexture("EsurvMinimapRibbonArt", "ARTWORK")
    band:SetTexture(ART .. "MapRibbon.tga")
    band:SetAllPoints(ribbon)
    local clock = ribbon:CreateFontString("EsurvMinimapClock", "OVERLAY", "GameFontHighlightSmall")
    clock:SetPoint("RIGHT", ribbon, "RIGHT", -16, 1)
    clock:SetJustifyH("RIGHT")
    local shown_minute = nil
    ribbon:SetScript("OnUpdate", function()
        if SetPortraitToTexture then
            round_icons()
        end
        if not GetGameTime then
            return
        end
        local h, m = GetGameTime()
        if m ~= shown_minute then
            shown_minute = m
            clock:SetText(string.format("%d:%02d", h or 0, m or 0))
        end
    end)
    ours.ring, ours.ribbon, ours.clock = ring, ribbon, clock
end

-- `button` at `degrees` round the map's middle, `radius` out.
local function on_rim(button, degrees, radius)
    local a = math.rad(degrees)
    button:ClearAllPoints()
    button:SetPoint("CENTER", Minimap, "CENTER", radius * math.cos(a), radius * math.sin(a))
end

local function zoom_mark(button, art, degrees)
    remember(button)
    remember_art(button)
    on_rim(button, degrees, M.MARK_RADIUS)
    button:SetWidth(MARK)
    button:SetHeight(MARK)
    local shades = { Normal = 1, Pushed = 0.7, Disabled = 0.4 }
    for state, shade in pairs(shades) do
        button["Set" .. state .. "Texture"](button, ART .. art)
        local t = button["Get" .. state .. "Texture"](button)
        t:SetVertexColor(shade, shade, shade, 1)
    end
end

-- The round buttons on the map in our thin bronze ring, round their icons; the ring art's circle is
-- 46 of its 64 across, just past the icon.
local function ring_buttons()
    for _, b in ipairs(BUTTON_RINGS) do
        local ring, icon = getglobal(b[1]), getglobal(b[2])
        if ring and icon and not stock_ring[ring] then
            remember(ring)
            stock_ring[ring] = ring:GetTexture()
            ring:SetTexture(ART .. "MapButtonRing.tga")
            ring:ClearAllPoints()
            ring:SetPoint("CENTER", icon, "CENTER", 0, 0)
            local side = math.max(icon:GetWidth(), 20) * 64 / 46
            ring:SetWidth(side)
            ring:SetHeight(side)
        end
    end
end

-- ── Ours, and the stock one back ─────────────────────────────────────────────────────────────
local function lay_out_ours()
    make_ours()
    remember(Minimap)
    Minimap:ClearAllPoints()
    Minimap:SetPoint("TOP", UIParent, "TOPRIGHT", -RIBBON_RIGHT - M.RIBBON_WIDTH / 2, -M.MAP_TOP)
    Minimap:SetWidth(M.SIZE)
    Minimap:SetHeight(M.SIZE)
    hide(MinimapBorder)
    hide(MinimapBorderTop)
    hide(MinimapToggleButton)
    hide(GameTimeFrame)
    -- the zone's name in the ribbon, at its left
    remember(MinimapZoneTextButton)
    MinimapZoneTextButton:ClearAllPoints()
    MinimapZoneTextButton:SetPoint("LEFT", ours.ribbon, "LEFT", 16, 1)
    MinimapZoneTextButton:SetWidth(M.RIBBON_WIDTH - 70)
    -- over the ribbon, not under it
    if not ours.zone_level then
        ours.zone_level = MinimapZoneTextButton:GetFrameLevel()
    end
    MinimapZoneTextButton:SetFrameLevel(ours.ribbon:GetFrameLevel() + 1)
    remember(MinimapZoneText)
    MinimapZoneText:ClearAllPoints()
    MinimapZoneText:SetPoint("LEFT", MinimapZoneTextButton, "LEFT", 0, 0)
    MinimapZoneText:SetWidth(M.RIBBON_WIDTH - 70)
    MinimapZoneText:SetJustifyH("LEFT")
    ring_buttons()
    zoom_mark(MinimapZoomIn, "MapPlus.tga", ZOOM_IN_AT)
    zoom_mark(MinimapZoomOut, "MapMinus.tga", ZOOM_OUT_AT)
    -- the buffs clear of the ribbon
    for _, frame in ipairs({ BuffFrame, TemporaryEnchantFrame }) do
        if frame then
            remember(frame)
            local point, to, to_point, x, y = frame:GetPoint(1)
            frame:ClearAllPoints()
            frame:SetPoint(point, to, to_point, x - BUFFS_LEFT, y)
        end
    end
    ours.ring:Show()
    ours.ribbon:Show()
end

local function lay_out_stock()
    for region, s in pairs(stock) do
        restore(region, s)
    end
    stock = {}
    for button, s in pairs(stock_art) do
        for state, path in pairs(s) do
            button["Set" .. state .. "Texture"](button, path)
            button["Get" .. state .. "Texture"](button):SetVertexColor(1, 1, 1, 1)
        end
    end
    stock_art = {}
    for ring, path in pairs(stock_ring) do
        ring:SetTexture(path)
    end
    stock_ring = {}
    square_icons()
    if MinimapZoneText.SetJustifyH then
        MinimapZoneText:SetJustifyH("CENTER")
    end
    if ours.zone_level then
        MinimapZoneTextButton:SetFrameLevel(ours.zone_level)
    end
    if ours.ring then
        ours.ring:Hide()
        ours.ribbon:Hide()
    end
end

local on = false

-- Whether ours shows (our own minimap buttons read it for their radius).
function M.on()
    return on
end

-- Buttons made on the map after ours turned on (`minimap_buttons.lua`) take our ring too.
function M.refresh()
    if on then
        ring_buttons()
    end
end

-- The run turned on or off (ESURV_RUN_FRAMES_ON).
function M.apply()
    local want = ESURV_RUN_FRAMES_ON and true or false
    if want == on then
        return
    end
    on = want
    if on then
        lay_out_ours()
    else
        lay_out_stock()
    end
    if ESURV_MINIMAP_BUTTONS and ESURV_MINIMAP_BUTTONS.place_all then
        ESURV_MINIMAP_BUTTONS.place_all()
    end
end

return true
