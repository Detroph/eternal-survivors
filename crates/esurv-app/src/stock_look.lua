-- The ui rework's look on the stock frames the game keeps (esurv-app's `stock_look`): the talk
-- window (`GossipFrame`) loses its parchment for the menus' night panel in a bronze edge, its
-- words parchment-light, at the bottom left; the player frame's portrait ring is bronze. The
-- colours come from the app's palette (`ESURV_LOOK.colours`, set before this runs).
--
-- Returns true once installed (and on every later call), false while the stock frames are not
-- loaded yet.

if ESURV_LOOK_ON then
    return true
end
if not (GossipFrame and GossipFrameUpdate and GossipGreetingText and PlayerFrameTexture
        and ESURV_LOOK and ESURV_LOOK.colours) then
    return false
end
ESURV_LOOK_ON = true

local C = ESURV_LOOK.colours

-- ── The talk window ──────────────────────────────────────────────────────────────────────────
-- Its parchment art hidden: every texture of the frame and its greeting panel but the portrait.
local function hide_art(frame)
    if not frame then
        return
    end
    local regions = { frame:GetRegions() }
    for _, r in ipairs(regions) do
        if r:GetObjectType() == "Texture" and r ~= GossipFramePortrait then
            r:SetAlpha(0)
        end
    end
end
hide_art(GossipFrame)
hide_art(GossipFrameGreetingPanel)

-- the night panel in its bronze edge, over the art's visible part
local back = CreateFrame("Frame", "ESurvGossipBack", GossipFrame)
back:SetPoint("TOPLEFT", GossipFrame, "TOPLEFT", 12, -14)
back:SetPoint("BOTTOMRIGHT", GossipFrame, "BOTTOMRIGHT", -32, 66)
back:SetFrameLevel(math.max(GossipFrame:GetFrameLevel() - 1, 0))
back:SetBackdrop({
    edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border",
    edgeSize = 16,
    insets = { left = 4, right = 4, top = 4, bottom = 4 },
})
back:SetBackdropBorderColor(C.bronze[1], C.bronze[2], C.bronze[3], 1)
-- volume, as the menus' panels have: the fill lit
-- at the top and shaded at its foot, a lit line under the top edge and a dark one over the foot
local fill = back:CreateTexture(nil, "BACKGROUND")
fill:SetTexture(C.panel[1], C.panel[2], C.panel[3], C.panel[4])
fill:SetPoint("TOPLEFT", back, "TOPLEFT", 4, -4)
fill:SetPoint("BOTTOMRIGHT", back, "BOTTOMRIGHT", -4, 4)
-- the light and the shade as stacked bands, each a little stronger nearer its edge
local function band(r, g, b, a, height, top)
    local t = back:CreateTexture(nil, "BACKGROUND")
    t:SetTexture(r, g, b, a)
    t:SetHeight(height)
    local edge = top and "TOP" or "BOTTOM"
    t:SetPoint(edge .. "LEFT", fill, edge .. "LEFT", 0, 0)
    t:SetPoint(edge .. "RIGHT", fill, edge .. "RIGHT", 0, 0)
end
for _, h in ipairs({ 90, 55, 30, 12 }) do
    band(0.55, 0.75, 1.0, 0.035, h, true)
end
for _, h in ipairs({ 220, 140, 80, 35 }) do
    band(0, 0, 0, 0.09, h, false)
end
local lit = back:CreateTexture(nil, "BORDER")
lit:SetTexture(C.bronze_light[1], C.bronze_light[2], C.bronze_light[3], 0.55)
lit:SetHeight(1)
lit:SetPoint("TOPLEFT", fill, "TOPLEFT", 0, 0)
lit:SetPoint("TOPRIGHT", fill, "TOPRIGHT", 0, 0)
local shade = back:CreateTexture(nil, "BORDER")
shade:SetTexture(0, 0, 0, 0.6)
shade:SetHeight(2)
shade:SetPoint("BOTTOMLEFT", fill, "BOTTOMLEFT", 0, 0)
shade:SetPoint("BOTTOMRIGHT", fill, "BOTTOMRIGHT", 0, 0)

-- the portrait's ring, drawn as a bronze disc behind the stock round portrait
local ring = back:CreateTexture(nil, "BACKGROUND")
ring:SetTexture("Interface\\CharacterFrame\\TempPortraitAlphaMask")
ring:SetVertexColor(C.bronze[1], C.bronze[2], C.bronze[3], 1)
ring:SetWidth(66)
ring:SetHeight(66)
ring:SetPoint("CENTER", GossipFramePortrait, "CENTER", 0, 0)

local function ink()
    if GossipFrameNpcNameText then
        GossipFrameNpcNameText:SetTextColor(C.heading[1], C.heading[2], C.heading[3])
    end
    GossipGreetingText:SetTextColor(C.value[1], C.value[2], C.value[3])
    for i = 1, (NUMGOSSIPBUTTONS or 32) do
        local b = getglobal("GossipTitleButton" .. i)
        local t = b and b:GetFontString()
        if t then
            t:SetTextColor(C.value[1], C.value[2], C.value[3])
        end
    end
end

-- at the left, low (bottom left), clear of the chat frame's buttons
local function place()
    GossipFrame:ClearAllPoints()
    GossipFrame:SetPoint("BOTTOMLEFT", UIParent, "BOTTOMLEFT", 0, 120)
end

local stock_update = GossipFrameUpdate
GossipFrameUpdate = function()
    stock_update()
    ink()
    place()
end
ink()

-- ── The run's portrait ring ─────────────────────────────────────────────────────────────────
PlayerFrameTexture:SetVertexColor(C.bronze_light[1], C.bronze_light[2], C.bronze_light[3], 1)

-- ── The action bar ───────────────────────────────────────────────────────────────────────────
-- Bronze-golden: out of a run its stone
-- strips and the experience bar's frame tinted (in a run our own art stands in for them,
-- action_bar.lua, which calls this after each layout, so the strips it moves follow).
local BAR_PIECES = {
    "MainMenuBarTexture0", "MainMenuBarTexture1", "MainMenuBarTexture2", "MainMenuBarTexture3",
    "MainMenuXPBarTexture0", "MainMenuXPBarTexture1", "MainMenuXPBarTexture2",
    "MainMenuXPBarTexture3",
    "MainMenuMaxLevelBar0", "MainMenuMaxLevelBar1", "MainMenuMaxLevelBar2", "MainMenuMaxLevelBar3",
}
-- A tint only darkens the grey stone, so each piece has a twin over it, its own art added back in
-- gold, which lifts it to bronze; the twin follows the piece's place, crop and showing.
local twins = {}
function ESURV_LOOK.tint_bar()
    for _, name in ipairs(BAR_PIECES) do
        local t = getglobal(name)
        if t then
            t:SetVertexColor(C.bar[1], C.bar[2], C.bar[3], 1)
            local twin = twins[name]
            if not twin then
                twin = t:GetParent():CreateTexture(nil, t:GetDrawLayer())
                twin:SetTexture(t:GetTexture())
                twin:SetBlendMode("ADD")
                twin:SetVertexColor(C.gold[1], C.gold[2], C.gold[3], 1)
                twins[name] = twin
            end
            twin:ClearAllPoints()
            twin:SetAllPoints(t)
            twin:SetTexCoord(t:GetTexCoord())
            if t:IsShown() then
                twin:Show()
            else
                twin:Hide()
            end
        end
    end
end
ESURV_LOOK.tint_bar()
-- the gryphons give way to the dragon,
-- drawn over the bar's buttons: the stock caps keep their place
-- but show nothing, and a frame above the buttons draws a dragon on each, the right one mirrored;
-- placed as the mock-up has them:
-- smaller than a cap, broad, their feet at the screen's foot, their inner side over the bar's end
local DRAGON_WIDTH, DRAGON_HEIGHT = 0.8, 0.66
-- how far in from the cap's inner edge the dragon's inner edge sits, and how far under its foot
local DRAGON_IN, DRAGON_DOWN = 8, 6
if MainMenuBarLeftEndCap and MainMenuBarRightEndCap then
    local over = CreateFrame("Frame", "EsurvDragonCaps", MainMenuBar)
    over:SetAllPoints(MainMenuBar)
    over:SetFrameLevel(MainMenuBar:GetFrameLevel() + 10)
    local function dragon(stock_cap, mirrored)
        stock_cap:SetAlpha(0)
        local t = over:CreateTexture(nil, "OVERLAY")
        t:SetTexture(ESURV_LOOK.dragon_cap)
        t:SetWidth(stock_cap:GetWidth() * DRAGON_WIDTH)
        t:SetHeight(stock_cap:GetHeight() * DRAGON_HEIGHT)
        if mirrored then
            t:SetPoint("BOTTOMLEFT", stock_cap, "BOTTOMLEFT", DRAGON_IN, -DRAGON_DOWN)
        else
            t:SetPoint("BOTTOMRIGHT", stock_cap, "BOTTOMRIGHT", -DRAGON_IN, -DRAGON_DOWN)
        end
        if mirrored then
            t:SetTexCoord(1, 0, 0, 1)
        end
    end
    dragon(MainMenuBarLeftEndCap, false)
    dragon(MainMenuBarRightEndCap, true)
end
-- the experience bar's fill, the key art's red and blue swirl for the stock purple, unsqueezed and flowing. The stock
-- fill crops one picture to the bar, so it is hidden (the stock code tints it by rest state: it
-- stays clear) and a texture of our own draws the fill: the art at its own shape, a strip and its
-- mirror twice over (`xp_swirl.tga`, 256 times as wide as tall), a window of it the fill's width
-- sliding along one strip and back to its start, where the next strip looks the same.
local XP_ASPECT, XP_TILES = 256, 2
-- how fast it flows, in units a second
local XP_FLOW = 18
if MainMenuExpBar then
    local bar = MainMenuExpBar
    local set_colour = bar.SetStatusBarColor
    set_colour(bar, 1, 1, 1, 0)
    bar.SetStatusBarColor = function(self)
        set_colour(self, 1, 1, 1, 0)
    end
    local fill = bar:CreateTexture("EsurvExpFlow", "ARTWORK")
    fill:SetTexture(ESURV_LOOK.xp_swirl)
    fill:SetPoint("TOPLEFT", bar, "TOPLEFT", 0, 0)
    fill:SetPoint("BOTTOMLEFT", bar, "BOTTOMLEFT", 0, 0)
    -- a glowing upright line at the fill's tip, the cast bar's spark
    -- (on a frame of its own over the bar's frame, `EsurvXpFrame`, so the frame never covers it)
    local fmod = math.fmod or math.mod
    local flow = CreateFrame("Frame", nil, bar)
    flow:SetAllPoints(bar)
    flow:SetFrameLevel(bar:GetFrameLevel() + 3)
    local spark = flow:CreateTexture("EsurvExpSpark", "OVERLAY")
    spark:SetTexture("Interface\\CastingBar\\UI-CastingBar-Spark")
    spark:SetBlendMode("ADD")
    spark:SetWidth(16)
    spark:SetPoint("CENTER", fill, "RIGHT", 0, 0)
    flow:SetScript("OnUpdate", function()
        local lo, hi = bar:GetMinMaxValues()
        local share = 0
        if hi and lo and hi > lo then
            share = math.min(math.max((bar:GetValue() - lo) / (hi - lo), 0), 1)
        end
        local width = bar:GetWidth() * share
        if width < 1 then
            fill:Hide()
            spark:Hide()
            return
        end
        fill:Show()
        -- the spark only while the bar is filling, not at its very end
        if share < 1 then
            spark:Show()
        else
            spark:Hide()
        end
        fill:SetWidth(width)
        -- the bar's height follows the run's layout
        spark:SetHeight(bar:GetHeight() * 2.4)
        -- the whole texture's width in units at the bar's height; the window never runs past
        -- its end (a strip's slide is left over)
        local span = bar:GetHeight() * XP_ASPECT
        local shown = math.min(width / span, 1 - 1 / XP_TILES)
        local start = fmod(GetTime() * XP_FLOW, span / XP_TILES) / span
        fill:SetTexCoord(start, start + shown, 0, 1)
    end)
end

return true
