-- The run's two minimap buttons (docs/game/screens.md, "The minimap
-- buttons"). esurv-app's `minimap_buttons` runs this once in each interface VM, once the stock
-- Minimap is loaded. Two round buttons on the minimap's rim, built the way an addon's minimap
-- icon is (an icon under the stock tracking border, the zoom buttons' highlight):
--
--   * the boss timer bars: flips `esurvBossBarHandles` (the same setting as Options -> Timers ->
--     "Show bar handles"), so the bars' drag handles show, the bars can be dragged, and the
--     handles hide again;
--   * the damage meter: shows or hides the meter window; the game keeps that in meter.ron and
--     tells this file back (`set_meter`).
--
-- A click is also queued for the game (`take`), which reads it at once.
--
-- Returns true once installed (and on every later call), false while the Minimap is not loaded.

if ESURV_MINIMAP_BUTTONS then
    return true
end
if not (Minimap and CreateFrame and GameTooltip and GetCVar and SetCVar) then
    return false
end

local M = { pending = {}, meter_shown = true, buttons = {} }
ESURV_MINIMAP_BUTTONS = M

local HANDLES_CVAR = "esurvBossBarHandles"
if RegisterCVar then
    RegisterCVar(HANDLES_CVAR, "1")
end

-- How far from the minimap's middle the buttons sit, and where round the rim (degrees, 0 at the
-- right, counting up anticlockwise): the lower left, clear of the stock tracking, mail, clock and
-- zoom buttons.
local RADIUS = 80
M.angles = { bars = 200, meter = 225 }

local function handles_on()
    local v = GetCVar(HANDLES_CVAR)
    return v == nil or tonumber(v) ~= 0
end

local function place(button, degrees)
    local a = math.rad(degrees)
    button:ClearAllPoints()
    -- the button's middle is its icon's (the ring is drawn from its top-left, as the stock one)
    button:SetPoint("CENTER", Minimap, "CENTER", RADIUS * math.cos(a), RADIUS * math.sin(a))
end

local function tip(button)
    local d = M.buttons[button]
    GameTooltip:SetOwner(button, "ANCHOR_LEFT")
    GameTooltip:SetText(d.title, 1, 0.82, 0)
    GameTooltip:AddLine(d.state(), 0.8, 0.8, 0.8)
    GameTooltip:Show()
end

local function make(key, name, icon, title, state, click)
    local b = CreateFrame("Button", name, Minimap)
    b:SetWidth(33)
    b:SetHeight(33)
    b:SetFrameStrata("LOW")
    if b.SetFrameLevel and Minimap.GetFrameLevel then
        b:SetFrameLevel(Minimap:GetFrameLevel() + 8)
    end
    local face = b:CreateTexture(name .. "Icon", "BACKGROUND")
    face:SetTexture(icon)
    face:SetWidth(20)
    face:SetHeight(20)
    face:SetTexCoord(0.075, 0.925, 0.075, 0.925)
    face:SetPoint("TOPLEFT", b, "TOPLEFT", 7, -6)
    local ring = b:CreateTexture(name .. "Border", "OVERLAY")
    ring:SetTexture("Interface\\Minimap\\MiniMap-TrackingBorder")
    ring:SetWidth(52)
    ring:SetHeight(52)
    ring:SetPoint("TOPLEFT", b, "TOPLEFT", 0, 0)
    b:SetHighlightTexture("Interface\\Minimap\\UI-Minimap-ZoomButton-Highlight")
    b:RegisterForClicks("LeftButtonUp", "RightButtonUp")
    M.buttons[b] = { key = key, title = title, state = state }
    b:SetScript("OnClick", function()
        click()
        table.insert(M.pending, key)
        if GameTooltip:IsOwned(this) then
            tip(this)
        end
    end)
    b:SetScript("OnEnter", function() tip(this) end)
    b:SetScript("OnLeave", function() GameTooltip:Hide() end)
    place(b, M.angles[key])
    return b
end

M.bars = make("bars", "EsurvMinimapBarsButton", "Interface\\Icons\\INV_Misc_PocketWatch_01",
    "Boss timer bars: show/hide drag handles",
    function()
        if handles_on() then
            return "Drag handles shown"
        end
        return "Drag handles hidden (hold Shift to drag)"
    end,
    function()
        SetCVar(HANDLES_CVAR, handles_on() and "0" or "1")
    end)

M.meter = make("meter", "EsurvMinimapMeterButton", "Interface\\Icons\\INV_Sword_04",
    "Damage meter: show/hide",
    function()
        if M.meter_shown then
            return "Shown"
        end
        return "Hidden"
    end,
    function()
        -- the game flips it and tells back; shown at once so the tooltip agrees
        M.meter_shown = not M.meter_shown
    end)

-- The meter's state, from the game (meter.ron).
function M.set_meter(shown)
    M.meter_shown = shown and true or false
    if GameTooltip:IsOwned(M.meter) then
        tip(M.meter)
    end
end

-- The clicks since the last call, oldest first, comma-separated ("bars,meter"); "" for none.
function M.take()
    if not M.pending[1] then
        return ""
    end
    local t = table.concat(M.pending, ",")
    M.pending = {}
    return t
end

-- Where a button's middle is, in the root's units (y up, as `GetCursorPosition` answers), for
-- the game's unattended check, which points at it the way the mouse does.
function M.where(key)
    local b = M[key]
    local x, y = b:GetCenter()
    if not x then
        return nil
    end
    local s = b:GetEffectiveScale()
    return x * s, y * s
end

return true
