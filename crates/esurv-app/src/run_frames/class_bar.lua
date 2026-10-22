-- The player frame's class bar in a run (docs/game/screens.md, "The
-- power bar"). esurv-app's `run_frames::class_bar` runs this once in each interface VM, once
-- the stock FrameXML is loaded, and hands it what the server says of the hero's starting option:
--
--   * the mana bar shows the option's resource or counter: rage and energy in the client's own bars
--     (the server sends the power type), every counter in the mana bar painted its own colour
--     (ManaBarColor[0] and SetStatusBarColor), with its count written on it and its name on a
--     small tab under the bar;
--   * combo points are pips on the player frame, under its mana bar;
--   * a stacking buff whose aura's own icon is not its own shows its own in the buff frame.
--
-- Returns true once installed, false while the stock frames it changes are not loaded yet.

if ESURV_CLASS_BAR then
    return true
end
if not (PlayerFrame and PlayerFrameManaBar and ManaBarColor and ManaBarColor[0]) then
    return false
end

local M = {}
ESURV_CLASS_BAR = M

-- the stock mana colour, put back when no counter paints the bar
local stock = { r = ManaBarColor[0].r, g = ManaBarColor[0].g, b = ManaBarColor[0].b }

-- the count on the bar, as the health bar writes its own ("3 / 5")
local text = PlayerFrameManaBar:CreateFontString("EsurvClassBarText", "OVERLAY", "TextStatusBarText")
text:SetPoint("CENTER", PlayerFrameManaBar, "CENTER", 0, 0)
text:Hide()

-- the name on a small tab under the bar: a tooltip's dark ground and
-- thin edge, the name in the small gold font
local tab = CreateFrame("Frame", "EsurvClassBarName", PlayerFrame)
tab:SetHeight(16)
if tab.SetBackdrop then
    tab:SetBackdrop({
        bgFile = "Interface\\Tooltips\\UI-Tooltip-Background",
        edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border",
        tile = true, tileSize = 8, edgeSize = 8,
        insets = { left = 2, right = 2, top = 2, bottom = 2 },
    })
    tab:SetBackdropColor(0, 0, 0, 0.8)
    tab:SetBackdropBorderColor(0.7, 0.7, 0.7, 1)
end
local label = tab:CreateFontString("EsurvClassBarNameText", "OVERLAY", "GameFontNormalSmall")
label:SetPoint("CENTER", tab, "CENTER", 0, 0)
tab:Hide()

-- Under the bar's middle; to its right while combo pips take its left.
local function place_tab(pips)
    tab:ClearAllPoints()
    if pips then
        tab:SetPoint("TOPRIGHT", PlayerFrameManaBar, "BOTTOMRIGHT", 0, -1)
    else
        tab:SetPoint("TOP", PlayerFrameManaBar, "BOTTOM", 0, -1)
    end
end

local function paint(r, g, b)
    ManaBarColor[0].r, ManaBarColor[0].g, ManaBarColor[0].b = r, g, b
    if UnitPowerType and UnitPowerType("player") == 0 then
        PlayerFrameManaBar:SetStatusBarColor(r, g, b)
    end
end

-- `kind` the content's word (Rage, Energy, HolyPower …); `name` the bar's name (empty: the
-- client's own word for rage and energy); `painted` whether the mana bar takes the colour.
function M.set_bar(kind, name, painted, r, g, b, value, max)
    if painted then
        paint(r, g, b)
    else
        paint(stock.r, stock.g, stock.b)
    end
    if name == "" then
        if kind == "Rage" then
            name = RAGE or kind
        elseif kind == "Energy" then
            name = ENERGY or kind
        end
    end
    text:SetText(math.floor(value + 0.001) .. " / " .. math.floor(max + 0.001))
    text:Show()
    label:SetText(name)
    local width = label.GetStringWidth and label:GetStringWidth() or 0
    tab:SetWidth(math.max(width, 30) + 14)
    tab:Show()
end

-- ── Combo points ─────────────────────────────────────────────────────────────────────────────
local PIPS = 5
M.pips = {}
for i = 1, PIPS do
    local back = PlayerFrame:CreateTexture("EsurvComboPip" .. i, "OVERLAY")
    back:SetTexture("Interface\\ComboFrame\\ComboPoint")
    back:SetTexCoord(0, 0.375, 0, 1)
    back:SetWidth(12)
    back:SetHeight(16)
    back:SetPoint("TOPLEFT", PlayerFrameManaBar, "BOTTOMLEFT", (i - 1) * 13, 0)
    back:Hide()
    local lit = PlayerFrame:CreateTexture("EsurvComboPip" .. i .. "Lit", "OVERLAY")
    lit:SetTexture("Interface\\ComboFrame\\ComboPoint")
    lit:SetTexCoord(0.375, 0.5625, 0, 1)
    lit:SetWidth(8)
    lit:SetHeight(16)
    lit:SetPoint("CENTER", back, "CENTER", 0, 0)
    lit:Hide()
    M.pips[i] = { back = back, lit = lit }
end

function M.set_pips(count, max)
    place_tab(max > 0)
    for i = 1, PIPS do
        local pip = M.pips[i]
        if i <= max then
            pip.back:Show()
        else
            pip.back:Hide()
        end
        if i <= count and i <= max then
            pip.lit:Show()
        else
            pip.lit:Hide()
        end
    end
end

-- Out of a run: the stock bar again.
function M.clear()
    paint(stock.r, stock.g, stock.b)
    text:Hide()
    tab:Hide()
    M.set_pips(0, 0)
end

-- ── Buff icons ───────────────────────────────────────────────────────────────────────────────
-- By aura Spell.dbc id: the icon its buff (or an enemy's debuff) shows instead of the aura
-- spell's own. Which aura each buff position is comes from the run frames' own maps
-- (`ESURV_RUN_FRAMES.player_auras`, `.target_debuffs`).
M.icons = {}

function M.set_icons(icons)
    M.icons = icons
end

local function icon_of(map, n)
    local rf = ESURV_RUN_FRAMES
    local id = ESURV_RUN_FRAMES_ON and rf and rf[map] and rf[map][n]
    return id and M.icons[id]
end

if GetPlayerBuffTexture then
    local stock_texture = GetPlayerBuffTexture
    GetPlayerBuffTexture = function(index)
        return icon_of("player_auras", index) or stock_texture(index)
    end
end
if UnitBuff then
    local stock_unit_buff = UnitBuff
    UnitBuff = function(unit, i, a3)
        local texture, count = stock_unit_buff(unit, i, a3)
        if texture and (unit == "player" or (UnitIsUnit and UnitIsUnit(unit, "player"))) then
            texture = icon_of("player_buffs", i) or texture
        end
        return texture, count
    end
end
-- The target's debuffs too: an enemy's debuff can wear its spell's icon (Deadly Poison's cloud).
if UnitDebuff then
    local stock_unit_debuff = UnitDebuff
    UnitDebuff = function(unit, i, a3)
        local texture, count, kind = stock_unit_debuff(unit, i, a3)
        if texture and unit == "target" then
            texture = icon_of("target_debuffs", i) or texture
        end
        return texture, count, kind
    end
end

return true
