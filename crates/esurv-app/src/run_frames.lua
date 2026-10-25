-- The stock frames during a run (docs/game/screens.md, "The stock frames
-- in a run"). esurv-app's `run_frames` runs this once in each interface VM, once the stock
-- FrameXML is loaded, and sets ESURV_RUN_FRAMES_ON while the hero is in the world (in this
-- game always in a run, running or over); with it off every function below is the stock one.
--
--   * kept: the character pane (its Character tab only), the spellbook (one tab, General, holding
--     every spell the server says is known: the race's spells and what the run taught), and the
--     talent frame, read-only, showing the class's bought talents of our own trees;
--   * blocked, keys and buttons alike: the quest log, the social window, the world map, the
--     reputation, skills, honor and pet tabs, the bags and the keyring, the pet spellbook, the
--     battlefield minimap and scores, the macro window, the help window, and the right-click
--     menus of the unit frames; the micro buttons and bag buttons that open them are hidden, and
--     so is the minimap's world map button (the main menu's micro button stays: it opens the
--     run's Escape menu, as Escape does);
--   * the main bar is compact (`action_bar.lua`): eight buttons, its ninth to twelfth and its
--     latency bar hidden here;
--   * ours: the tooltips of the hero's spells on the action bar and in the spellbook, and their
--     names and ranks in the spellbook (what the spell does in the run, with the hero's numbers).
--
-- Returns true once installed (and on every later call), false while the stock frames it wraps
-- are not loaded yet.

if ESURV_RUN_FRAMES then
    return true
end
if not (ShowUIPanel and ToggleQuestLog and ToggleCharacter and ToggleSpellBook
        and ToggleTalentFrame and GetSpellTabInfo and GetNumSpellTabs and ToggleDropDownMenu) then
    return false
end

local M = {}
ESURV_RUN_FRAMES = M

local function on()
    return ESURV_RUN_FRAMES_ON
end

-- ── Toggles that open nothing in a run ───────────────────────────────────────────────────────
-- `allow` lets some calls through (the character pane's own tab, the spells page).
local function block(name, allow)
    local stock = _G[name]
    if not stock then
        return
    end
    _G[name] = function(a1, a2, a3, a4, a5, a6, a7, a8)
        if on() and not (allow and allow(a1, a2, a3, a4, a5, a6, a7, a8)) then
            return
        end
        return stock(a1, a2, a3, a4, a5, a6, a7, a8)
    end
end

for _, name in ipairs({
    "ToggleQuestLog", "ToggleFriendsFrame", "ToggleWorldMap", "ToggleHelpFrame",
    "ToggleBackpack", "ToggleBag", "OpenAllBags", "OpenBag", "OpenBackpack", "ToggleKeyRing",
    "ToggleBattlefieldMinimap", "ToggleWorldStateScoreFrame", "ShowMacroFrame",
}) do
    block(name)
end
block("ToggleCharacter", function(tab)
    return tab == "PaperDollFrame"
end)
block("ToggleSpellBook", function(bookType)
    return bookType == BOOKTYPE_SPELL
end)

-- The panels a run never shows, however they are asked for (a slash command, an event).
local BLOCKED_PANELS = {
    QuestLogFrame = 1, FriendsFrame = 1, WorldMapFrame = 1, HelpFrame = 1, MacroFrame = 1,
    WorldStateScoreFrame = 1,
}
block("ShowUIPanel", function(frame)
    return not (frame and frame.GetName and BLOCKED_PANELS[frame:GetName()])
end)

-- The unit frames' right-click menus: party, loot, trade, duel and inspect have no place in a run.
local BLOCKED_MENUS = {
    PlayerFrameDropDown = 1, TargetFrameDropDown = 1, PetFrameDropDown = 1,
    PartyMemberFrame1DropDown = 1, PartyMemberFrame2DropDown = 1,
    PartyMemberFrame3DropDown = 1, PartyMemberFrame4DropDown = 1,
}
block("ToggleDropDownMenu", function(level, value, dropDown)
    return not (dropDown and dropDown.GetName and BLOCKED_MENUS[dropDown:GetName()])
end)

-- ── Buttons hidden in a run ──────────────────────────────────────────────────────────────────
local HIDDEN = {
    "QuestLogMicroButton", "SocialsMicroButton", "WorldMapMicroButton",
    "HelpMicroButton", "MainMenuBarBackpackButton", "CharacterBag0Slot", "CharacterBag1Slot",
    "CharacterBag2Slot", "CharacterBag3Slot", "KeyRingButton", "MiniMapWorldMapButton",
    "MiniMapBattlefieldFrame", "CharacterFrameTab2", "CharacterFrameTab3", "CharacterFrameTab4",
    "CharacterFrameTab5", "SpellBookFrameTabButton2",
    -- the main bar past its eighth button, and its latency bar (`action_bar.lua`)
    "ActionButton9", "ActionButton10", "ActionButton11", "ActionButton12",
    "MainMenuBarPerformanceBarFrame",
}
-- What was shown when a run hid it, to show again after.
local hid = {}

-- Keeps `frame` hidden while a run is on, whatever shows it.
local function keep_hidden(frame)
    local stock = frame:GetScript("OnShow")
    frame:SetScript("OnShow", function()
        if on() then
            this:Hide()
            return
        end
        if stock then
            stock()
        end
    end)
end

for _, name in ipairs(HIDDEN) do
    local frame = _G[name]
    if frame then
        keep_hidden(frame)
    end
end

-- ── The spellbook: one tab, General, with every spell ────────────────────────────────────────
-- Tabs follow one another in the book, so one tab from the first spell over all of them holds
-- the whole book in its order.
local stock_num_tabs, stock_tab_info = GetNumSpellTabs, GetSpellTabInfo
GetNumSpellTabs = function()
    if on() and stock_num_tabs() > 0 then
        return 1
    end
    return stock_num_tabs()
end
GetSpellTabInfo = function(i)
    if not on() then
        return stock_tab_info(i)
    end
    if i ~= 1 then
        return nil
    end
    local name, texture = stock_tab_info(1)
    if name ~= GENERAL then
        name, texture = GENERAL or "General", "Interface\\Icons\\Ability_Kick"
    end
    local total = 0
    for t = 1, stock_num_tabs() do
        local _, _, _, count = stock_tab_info(t)
        total = total + (count or 0)
    end
    return name, texture, 0, total
end
if SpellBookFrame then
    local stock = SpellBookFrame:GetScript("OnShow")
    SpellBookFrame:SetScript("OnShow", function()
        if on() then
            SpellBookFrame.selectedSkillLine = 1
            SpellBookFrame.bookType = BOOKTYPE_SPELL
        end
        if stock then
            stock()
        end
    end)
end

-- ── The talent frame: our trees, the class's bought talents, read-only ───────────────────────
-- ESURV_RUN_FRAMES.talents, from esurv-app: tabs of { name, background, points,
-- talents = { { name, icon, tier, column, rank, max, prereqs = { {tier, column}, ... },
-- tip = { {text, r, g, b}, ... } } } }.
M.talents = {}

local function talent_at(tab, i)
    local t = M.talents[tab]
    return t and t.talents[i]
end

local function wrap(name, run)
    local stock = _G[name]
    _G[name] = function(a1, a2, a3, a4)
        if on() then
            return run(a1, a2, a3, a4)
        end
        if stock then
            return stock(a1, a2, a3, a4)
        end
    end
end

wrap("GetNumTalentTabs", function()
    return table.getn(M.talents)
end)
wrap("GetTalentTabInfo", function(i)
    local t = M.talents[i]
    if not t then
        return nil
    end
    return t.name, nil, t.points, t.background
end)
wrap("GetNumTalents", function(tab)
    local t = M.talents[tab]
    return t and table.getn(t.talents) or 0
end)
wrap("GetTalentInfo", function(tab, i)
    local t = talent_at(tab, i)
    if not t then
        return nil
    end
    return t.name, t.icon, t.tier, t.column, t.rank, t.max, 0, 1
end)
wrap("GetTalentPrereqs", function(tab, i)
    local t = talent_at(tab, i)
    if not t or not t.prereqs[1] then
        return
    end
    return t.prereqs[1][1], t.prereqs[1][2], 1
end)
-- Nothing is learned in a run: the hero sheet before a run learns and takes back.
wrap("LearnTalent", function() end)
wrap("UnitCharacterPoints", function()
    return 0, 0
end)

-- The tooltip: our lines, as the stock talent tooltip lays them out.
local function talent_tip()
    local t = talent_at(PanelTemplates_GetSelectedTab(TalentFrame), this:GetID())
    if not t then
        return
    end
    GameTooltip:SetOwner(this, "ANCHOR_RIGHT")
    for n, line in ipairs(t.tip) do
        if n == 1 then
            GameTooltip:SetText(line[1], line[2], line[3], line[4])
        else
            GameTooltip:AddLine(line[1], line[2], line[3], line[4], 1)
        end
    end
    GameTooltip:Show()
end

local tips_hooked = false
local function hook_talent_tips()
    if tips_hooked or not TalentFrame then
        return
    end
    tips_hooked = true
    for i = 1, (MAX_NUM_TALENTS or 20) do
        local button = _G["TalentFrameTalent" .. i]
        if button then
            local stock = button:GetScript("OnEnter")
            button:SetScript("OnEnter", function()
                if on() then
                    talent_tip()
                elseif stock then
                    stock()
                end
            end)
        end
    end
end

-- Opens at any level in a run (the stock frame waits for level 10).
local stock_toggle_talents = ToggleTalentFrame
ToggleTalentFrame = function()
    if not on() then
        return stock_toggle_talents()
    end
    TalentFrame_LoadUI()
    hook_talent_tips()
    if TalentFrame_Toggle then
        TalentFrame_Toggle()
    end
end

-- Its micro button shows at any level in a run.
local stock_update_talent_button = UpdateTalentButton
UpdateTalentButton = function()
    if on() then
        TalentMicroButton:Show()
        return
    end
    if stock_update_talent_button then
        stock_update_talent_button()
    end
end

-- New talent data: an open talent frame redraws.
function M.set_talents(tabs)
    M.talents = tabs
    if TalentFrame and TalentFrame:IsVisible() and TalentFrame_Update then
        TalentFrame_Update()
    end
end

-- ── Our spell tooltips ───────────────────────────────────────────────────────────────────────
-- The action bar and the spellbook hold each spell as the stock spell it looks like, whose stock
-- tooltip says what that stock spell does. In a run the stock tooltip frame shows ours instead:
-- esurv-app (`run_frames::spell_tips`) hands over each spell's lines by its Spell.dbc id
-- (`M.set_spell_tips`: { name, sub, lines = { {left, r, g, b [, right, r, g, b]}, ... } }) and
-- which spell each action slot and spellbook slot holds (`M.set_spell_slots`), since 1.12's Lua
-- has no call that names an action slot's spell. A spell it has no tooltip for keeps the stock one.
M.spell_tips = {}
M.action_spells = {}
M.book_spells = {}

local function spell_tip(id)
    return on() and id and M.spell_tips[id]
end

-- Fills the tooltip the caller has anchored, as the stock spell tooltip lays it out: a line with a
-- right column (name and rank, school and range, cast and cooldown) stays one line, the others
-- wrap. Returns 1, so the caller keeps refreshing it, as it does a stock spell's.
-- `last`, when given, is one more line ({text, r, g, b}) at the foot (a buff's time left).
local function show_spell_tip(tooltip, tip, last)
    tooltip:ClearLines()
    for _, l in ipairs(tip.lines) do
        if l[5] then
            tooltip:AddDoubleLine(l[1], l[5], l[2], l[3], l[4], l[6], l[7], l[8])
        else
            tooltip:AddLine(l[1], l[2], l[3], l[4], 1)
        end
    end
    if last then
        tooltip:AddLine(last[1], last[2], last[3], last[4])
    end
    tooltip:Show()
    return 1
end

if GameTooltip then
    local stock_set_action = GameTooltip.SetAction
    GameTooltip.SetAction = function(self, action)
        local tip = spell_tip(M.action_spells[action])
        if tip then
            return show_spell_tip(self, tip)
        end
        return stock_set_action(self, action)
    end
    local stock_set_spell = GameTooltip.SetSpell
    GameTooltip.SetSpell = function(self, id, book_type)
        local tip = book_type == BOOKTYPE_SPELL and spell_tip(M.book_spells[id])
        if tip then
            return show_spell_tip(self, tip)
        end
        return stock_set_spell(self, id, book_type)
    end
end

-- The spellbook writes our name and rank under each icon ("Rank 2", "Racial", "Passive").
local stock_spell_name = GetSpellName
if stock_spell_name then
    GetSpellName = function(id, book_type)
        local tip = book_type == BOOKTYPE_SPELL and spell_tip(M.book_spells[id])
        if tip then
            return tip.name, tip.sub
        end
        return stock_spell_name(id, book_type)
    end
end

local function refresh_spellbook()
    if SpellBookFrame and SpellBookFrame:IsVisible() and SpellBookFrame_Update then
        SpellBookFrame_Update()
    end
end

function M.set_spell_tips(tips)
    M.spell_tips = tips
    refresh_spellbook()
end

-- Each spell's own icon (the one its card wears), by its Spell.dbc id: the action bar and the
-- spellbook show it in place of the stock spell's (`M.set_spell_icons`).
M.spell_icons = {}

local function spell_icon(id)
    return on() and id and M.spell_icons[id]
end

local stock_action_texture = GetActionTexture
if stock_action_texture then
    GetActionTexture = function(action)
        return spell_icon(M.action_spells[action]) or stock_action_texture(action)
    end
end
local stock_spell_texture = GetSpellTexture
if stock_spell_texture then
    GetSpellTexture = function(id, book_type)
        return (book_type == BOOKTYPE_SPELL and spell_icon(M.book_spells[id])) or stock_spell_texture(id, book_type)
    end
end

-- No global cooldown sweep in a run: the server casts every spell on
-- its own cooldown and has no global one, so the client's 1.5 sec sweep after each cast shows a
-- wait that is not there. A cooldown of exactly that length reads as none.
local GLOBAL_COOLDOWN = 1.5
local function without_global(start, duration, enable)
    if on() and duration and math.abs(duration - GLOBAL_COOLDOWN) < 0.01 then
        return 0, 0, enable
    end
    return start, duration, enable
end
local stock_action_cooldown = GetActionCooldown
if stock_action_cooldown then
    GetActionCooldown = function(action)
        return without_global(stock_action_cooldown(action))
    end
end
local stock_spell_cooldown = GetSpellCooldown
if stock_spell_cooldown then
    GetSpellCooldown = function(id, book_type)
        return without_global(stock_spell_cooldown(id, book_type))
    end
end

-- The stock bars redraw their buttons, so a button shows the icon handed over since.
local ACTION_BARS = { "ActionButton", "BonusActionButton", "MultiBarBottomLeftButton",
    "MultiBarBottomRightButton", "MultiBarRightButton", "MultiBarLeftButton" }
local function refresh_action_bars()
    if not ActionButton_Update then
        return
    end
    local was = this
    for _, bar in ipairs(ACTION_BARS) do
        for i = 1, 12 do
            local button = getglobal(bar .. i)
            if button then
                this = button
                ActionButton_Update()
            end
        end
    end
    this = was
end

function M.set_spell_icons(icons)
    M.spell_icons = icons
    refresh_action_bars()
    refresh_spellbook()
end

function M.set_spell_slots(actions, book)
    M.action_spells = actions
    M.book_spells = book
    refresh_action_bars()
    refresh_spellbook()
end

-- ── Our buff tooltips ────────────────────────────────────────────────────────────────────────
-- A buff wears the look of a stock spell's aura, whose stock tooltip says what that stock spell
-- does. In a run the stock tooltip frame shows ours instead: esurv-app
-- (`run_frames::buff_tips`) hands over the hero's buffs' lines and its spells' effects on an
-- enemy by the aura's Spell.dbc id (`M.set_buff_tips`), and which aura each of the player's buff
-- positions (0-based, as GetPlayerBuff answers), each of its buffs (1-based, as UnitBuff counts)
-- and each of the target's debuffs is (`M.set_aura_slots`). Anything else keeps the stock one.
M.buff_tips = {}
M.debuff_tips = {}
M.player_auras = {}
M.player_buffs = {}
M.target_debuffs = {}

-- "12 sec left", "2 min left": the time a buff has left, rounded down as the buff button counts
-- it, at the tooltip's foot in gold.
local function time_left(seconds)
    if not seconds or seconds <= 0 then
        return nil
    end
    local text
    if seconds >= 60 then
        text = math.floor(seconds / 60) .. " min left"
    else
        text = math.floor(seconds) .. " sec left"
    end
    return { text, 1, 0.82, 0 }
end

local function is_player(unit)
    return unit == "player" or (UnitIsUnit and UnitIsUnit(unit, "player"))
end

if GameTooltip then
    local stock_player_buff = GameTooltip.SetPlayerBuff
    GameTooltip.SetPlayerBuff = function(self, index)
        local id = on() and M.player_auras[index]
        local tip = id and M.buff_tips[id]
        if tip then
            local left = GetPlayerBuffTimeLeft and GetPlayerBuffTimeLeft(index)
            return show_spell_tip(self, tip, time_left(left))
        end
        return stock_player_buff(self, index)
    end
    local stock_unit_buff = GameTooltip.SetUnitBuff
    GameTooltip.SetUnitBuff = function(self, unit, index, a3)
        local id = on() and is_player(unit) and M.player_buffs[index]
        local tip = id and M.buff_tips[id]
        if tip then
            return show_spell_tip(self, tip)
        end
        return stock_unit_buff(self, unit, index, a3)
    end
    local stock_unit_debuff = GameTooltip.SetUnitDebuff
    GameTooltip.SetUnitDebuff = function(self, unit, index, a3)
        local id = on() and unit == "target" and not is_player(unit) and M.target_debuffs[index]
        local tip = id and M.debuff_tips[id]
        if tip then
            return show_spell_tip(self, tip)
        end
        return stock_unit_debuff(self, unit, index, a3)
    end
end

function M.set_buff_tips(buffs, debuffs)
    M.buff_tips = buffs
    M.debuff_tips = debuffs
end

function M.set_aura_slots(player_auras, player_buffs, target_debuffs)
    M.player_auras = player_auras
    M.player_buffs = player_buffs
    M.target_debuffs = target_debuffs
end

-- A development aid (esurv-app's ESURV_TOOLTIP_SHOT): shows the tooltip of action
-- button `n` of the main bar, of the spellbook's `n`-th spell on its first page (the button
-- whose id is `n`: the left column holds 1 to 6, the right 7 to 12; the book opened first), or of
-- the buff frame's `n`-th buff, the stock way the pointer would, and leaves it up.
function M.show_tip_for(where, n)
    local saved = this
    if where == "buff" then
        this = getglobal("BuffButton" .. (n - 1))
        if this and this.buffIndex and this.buffIndex >= 0 then
            GameTooltip:SetOwner(this, "ANCHOR_BOTTOMLEFT")
            GameTooltip:SetPlayerBuff(this.buffIndex)
        end
    elseif where == "action" then
        this = getglobal("ActionButton" .. n)
        if this and ActionButton_SetTooltip then
            ActionButton_SetTooltip()
        end
    elseif where == "book" then
        if SpellBookFrame and not SpellBookFrame:IsVisible() then
            ToggleSpellBook(BOOKTYPE_SPELL)
        end
        this = nil
        for i = 1, (SPELLS_PER_PAGE or 12) do
            local button = getglobal("SpellButton" .. i)
            if button and button:GetID() == n then
                this = button
            end
        end
        if this and SpellButton_OnEnter then
            SpellButton_OnEnter()
        end
    end
    this = saved
end

-- ── The run's level cap ──────────────────────────────────────────────────────────────────────
-- The stock experience bar and reputation bar read MAX_PLAYER_LEVEL (the stock 60): in a run it
-- is the run's own cap, set from esurv-app (`M.set_limits`), so the bar keeps filling past
-- 60 and gives way only at the run's cap.
local stock_max_level = MAX_PLAYER_LEVEL
M.level_cap = nil

local function refresh_level_bars()
    local level = UnitLevel and UnitLevel("player")
    if ReputationWatchBar_Update and level then
        ReputationWatchBar_Update(level)
    end
    if MainMenuExpBar_Update then
        MainMenuExpBar_Update()
    end
end

local function apply_level_cap()
    local wanted = stock_max_level
    if on() and M.level_cap then
        wanted = M.level_cap
    end
    if MAX_PLAYER_LEVEL ~= wanted then
        MAX_PLAYER_LEVEL = wanted
        refresh_level_bars()
    end
end

-- ── The stock level-up lines ─────────────────────────────────────────────────────────────────
-- The client prints the stock level-up block ("Congratulations, you have reached level N!" and
-- the hit points gained) on every change of the hero's level field. A run's server sends no
-- level-up gains packet, only its own system line with what the level brought, and a fresh run
-- (Again, Reset) puts the level back to 1; so in a run the stock block is not printed at all.
-- Everything else that listens for the level change still hears it.
if ChatFrame_OnEvent then
    local stock_chat_event = ChatFrame_OnEvent
    ChatFrame_OnEvent = function(ev, a2, a3, a4)
        if on() and ev == "PLAYER_LEVEL_UP" then
            return
        end
        return stock_chat_event(ev, a2, a3, a4)
    end
end

-- ── The run's numbers on the character pane ──────────────────────────────────────────────────
-- The server sends the hero's run numbers as an addon line (prefix ESURV, `key=value;` pairs, the
-- same keys with `gear_` in front for what worn gear adds). While a run is on and the numbers have
-- come, the character pane's twelve stat rows show them instead of the stock attributes, each
-- with a tooltip of ours, and the resistance icons give way. The item slots are left alone.
M.stats = nil
M.max_reduction = nil

-- `key=value;...` as a table of numbers.
function M.read_stats(line)
    local t = {}
    local at = 1
    while true do
        local first, last, k, v = string.find(line, "([%w_]+)=([^;]*)", at)
        if not first then
            break
        end
        t[k] = tonumber(v) or 0
        at = last + 1
    end
    return t
end

local GEAR_COLOUR = "|cff20ff20"
local END_COLOUR = "|r"

local function percent(v)
    return string.format("%d%%", math.floor(v * 100 + 0.5))
end

local function signed_percent(v)
    local whole = math.floor(v * 100 + 0.5)
    if whole < 0 then
        return string.format("-%d%%", -whole)
    end
    return string.format("+%d%%", whole)
end

local function num(s, key)
    return s[key] or 0
end

-- The rows in the pane's order, each: the stock row it takes, its label, its value, the gear key
-- that adds to it, what that gear adds in words, and its tooltip lines.
local ROWS = {
    { frame = "CharacterStatFrame1", label = "Health", gear = "gear_health",
      value = function(s) return string.format("%d", num(s, "health")) end,
      gear_text = function(g) return string.format("+%d health", g) end,
      tip = function(s) return { "The most health the hero has in this run." } end },
    { frame = "CharacterStatFrame2", label = "Damage", gear = "gear_damage",
      value = function(s) return signed_percent(num(s, "damage") - 1) end,
      gear_text = function(g) return signed_percent(g) .. " damage" end,
      tip = function(s) return { "How much harder every spell and swing hits than at the start. It grows with each level, and with talents, gear, jewelry and buffs." } end },
    { frame = "CharacterStatFrame3", label = "Cooldowns", gear = "gear_cooldown",
      value = function(s) return signed_percent(num(s, "cooldown") - 1) end,
      gear_text = function(g) return signed_percent(g) .. " cooldown recovery" end,
      tip = function(s) return { string.format("Every spell is ready again after %s of its own cooldown.", percent(num(s, "cooldown"))) } end },
    { frame = "CharacterStatFrame4", label = "Crit", gear = "gear_crit",
      value = function(s) return string.format("%.1f%%", num(s, "crit") * 100) end,
      gear_text = function(g) return signed_percent(g) .. " critical strike chance" end,
      tip = function(s) return { "The chance a hit is a critical strike. What would go past 100% makes critical strikes hit harder instead." } end },
    { frame = "CharacterStatFrame5", label = "Crit Power", gear = "gear_crit_damage",
      value = function(s) return percent(num(s, "crit_damage")) end,
      gear_text = function(g) return signed_percent(g) .. " critical strike damage" end,
      tip = function(s) return { string.format("A critical strike deals %s of the hit's damage.", percent(num(s, "crit_damage"))) } end },
    { frame = "CharacterArmorFrame", label = "Reduction", gear = "gear_reduction",
      value = function(s) return percent(num(s, "reduction")) end,
      gear_text = function(g) return signed_percent(g) .. " damage reduction" end,
      tip = function(s)
          local lines = { string.format("Damage taken is lowered by %s.", percent(num(s, "reduction"))) }
          if M.max_reduction then
              table.insert(lines, string.format("It never goes past %s.", percent(M.max_reduction)))
          end
          return lines
      end },
    { frame = "CharacterAttackFrame", label = "Regen", gear = "gear_regen",
      value = function(s) return string.format("%.1f/s", num(s, "regen")) end,
      gear_text = function(g) return string.format("+%.1f health a second", g) end,
      tip = function(s)
          local lines = { string.format("Health regained every second: %.2f.", num(s, "regen")) }
          if num(s, "heal_per_kill") > 0 then
              table.insert(lines, string.format("Every kill heals %.1f.", num(s, "heal_per_kill")))
          end
          if num(s, "gear_heal_per_kill") ~= 0 then
              table.insert(lines, GEAR_COLOUR .. string.format("Worn gear adds %d health a kill.", num(s, "gear_heal_per_kill"))
                  .. END_COLOUR)
          end
          if num(s, "heal_shield") ~= 0 then
              table.insert(lines, "Heals and shields of your spells: " .. signed_percent(num(s, "heal_shield")) .. ".")
          end
          if num(s, "gear_heal_shield") ~= 0 then
              table.insert(lines, GEAR_COLOUR .. "Worn gear adds " .. signed_percent(num(s, "gear_heal_shield"))
                  .. " heal and shield power." .. END_COLOUR)
          end
          return lines
      end },
    { frame = "CharacterAttackPowerFrame", label = "Area", gear = "gear_area",
      value = function(s) return signed_percent(num(s, "area") - 1) end,
      gear_text = function(g) return signed_percent(g) .. " area" end,
      tip = function(s) return { "How much wider spells reach than at the start: their areas and their ranges." } end },
    { frame = "CharacterDamageFrame", label = "Targets", gear = "gear_targets",
      value = function(s) return string.format("+%d", num(s, "targets")) end,
      gear_text = function(g) return string.format("+%d targets", g) end,
      tip = function(s) return { "Extra targets for every spell that picks its own." } end },
    { frame = "CharacterRangedAttackFrame", label = "Double Cast", gear = "gear_double_cast",
      value = function(s) return percent(num(s, "double_cast")) end,
      gear_text = function(g) return signed_percent(g) .. " double cast" end,
      tip = function(s) return { "The chance a spell goes off a second time. Past 100%, every spell goes twice and the rest is the chance of a third time." } end },
    { frame = "CharacterRangedAttackPowerFrame", label = "Speed", gear = "gear_speed",
      value = function(s) return signed_percent(num(s, "speed") - 1) end,
      gear_text = function(g) return signed_percent(g) .. " movement speed" end,
      tip = function(s) return { "How much faster the hero runs than at the start." } end },
    { frame = "CharacterRangedDamageFrame", label = "Pickup", gear = "gear_pickup",
      value = function(s) return string.format("%.1f yd", num(s, "pickup")) end,
      gear_text = function(g) return signed_percent(g) .. " pickup range" end,
      tip = function(s)
          local lines = { "Experience and gold fly to the hero from this far." }
          table.insert(lines, string.format("Experience %s, gold %s, luck %s.",
              signed_percent(num(s, "experience") - 1), signed_percent(num(s, "gold") - 1),
              signed_percent(num(s, "luck"))))
          if num(s, "gear_gold") ~= 0 then
              table.insert(lines, GEAR_COLOUR .. "Worn gear adds " .. signed_percent(num(s, "gear_gold"))
                  .. " gold." .. END_COLOUR)
          end
          if num(s, "gear_luck") ~= 0 then
              table.insert(lines, GEAR_COLOUR .. "Worn gear adds " .. signed_percent(num(s, "gear_luck"))
                  .. " luck." .. END_COLOUR)
          end
          return lines
      end },
}
M.ROWS = ROWS

-- A row's tooltip in a run; the stock one otherwise.
local function row_tip()
    local row = this.esurv_row
    local s = M.stats
    if not (on() and row and s) then
        if this.esurv_stock_enter then
            this.esurv_stock_enter()
        end
        return
    end
    GameTooltip:SetOwner(this, "ANCHOR_RIGHT")
    GameTooltip:SetText(row.label .. " " .. row.value(s), 1, 1, 1)
    local gold = NORMAL_FONT_COLOR or { r = 1, g = 0.82, b = 0 }
    for _, line in ipairs(row.tip(s)) do
        GameTooltip:AddLine(line, gold.r, gold.g, gold.b, 1)
    end
    local g = num(s, row.gear)
    if g ~= 0 then
        GameTooltip:AddLine("Worn gear adds " .. row.gear_text(g) .. ".", 0.125, 1, 0.125, 1)
    end
    GameTooltip:Show()
end

-- Writes the run's numbers into the pane's rows, when a run is on and they have come. A value worn
-- gear adds to shows green, as the stock pane shows a bonus.
function M.show_stats()
    local s = M.stats
    if not (on() and s) then
        return
    end
    for _, row in ipairs(ROWS) do
        local frame = _G[row.frame]
        if frame then
            if not frame.esurv_row then
                frame.esurv_stock_enter = frame:GetScript("OnEnter")
                frame:SetScript("OnEnter", row_tip)
            end
            frame.esurv_row = row
            local label = _G[row.frame .. "Label"]
            local text = _G[row.frame .. "StatText"]
            if label then
                label:SetText(row.label .. ":")
            end
            if text then
                local value = row.value(s)
                if num(s, row.gear) > 0 then
                    value = GEAR_COLOUR .. value .. END_COLOUR
                end
                text:SetText(value)
            end
        end
    end
    if CharacterResistanceFrame then
        CharacterResistanceFrame:Hide()
    end
end

-- The stock pane fills its rows when it shows and on unit events; ours follow at once.
for _, name in ipairs({ "PaperDollFrame_OnShow", "PaperDollFrame_OnEvent" }) do
    local stock = _G[name]
    if stock then
        _G[name] = function(a1, a2, a3, a4)
            stock(a1, a2, a3, a4)
            M.show_stats()
        end
    end
end

-- A numbers line: kept, and shown at once on an open pane.
function M.take_stats(line)
    M.stats = M.read_stats(line)
    if PaperDollFrame and PaperDollFrame:IsVisible() then
        M.show_stats()
    end
end

-- The numbers line comes through the addon message event.
if CreateFrame then
    local listener = CreateFrame("Frame", "EsurvRunNumbers")
    listener:RegisterEvent("CHAT_MSG_ADDON")
    listener:SetScript("OnEvent", function()
        if event == "CHAT_MSG_ADDON" and arg1 == "ESURV" and arg2 then
            M.take_stats(arg2)
        end
    end)
end

-- The run's limits from esurv-app: its level cap and the most damage reduction there is.
function M.set_limits(level_cap, max_reduction)
    M.level_cap = level_cap
    M.max_reduction = max_reduction
    apply_level_cap()
end

-- ── Turning a run on and off ─────────────────────────────────────────────────────────────────
function M.apply()
    apply_level_cap()
    if not on() then
        -- the next run's numbers come with it; the pane is the stock one again
        M.stats = nil
        if CharacterResistanceFrame then
            CharacterResistanceFrame:Show()
        end
    end
    if on() then
        for _, name in ipairs(HIDDEN) do
            local frame = _G[name]
            if frame and frame:IsShown() then
                hid[name] = 1
                frame:Hide()
            end
        end
        for name in pairs(BLOCKED_PANELS) do
            local frame = _G[name]
            if frame and frame:IsShown() then
                HideUIPanel(frame)
            end
        end
        if CloseAllBags then
            CloseAllBags()
        end
    else
        for name in pairs(hid) do
            local frame = _G[name]
            if frame then
                frame:Show()
            end
        end
        hid = {}
    end
    if UpdateTalentButton and TalentMicroButton then
        UpdateTalentButton()
    end
    -- the main bar, compact in a run (`action_bar.lua`, installed right after this file)
    if ESURV_ACTION_BAR then
        ESURV_ACTION_BAR.apply()
    end
    if SpellBookFrame and SpellBookFrame:IsVisible() and SpellBookFrame_Update then
        SpellBookFrame_Update()
    end
end

return true
