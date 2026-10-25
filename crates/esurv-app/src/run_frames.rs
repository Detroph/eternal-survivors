//! The stock frames during a run (design step 6's stock interface
//! patch). While the hero is in the world, which in this game is always a run (running, paused,
//! or over on the end panel), the stock interface keeps only three of its windows:
//!
//! * the character pane (C and its micro button), on its Character tab;
//! * the spellbook (P and its micro button), one tab, General, holding exactly what the server
//!   says is known: the race's spell and passives and the spells learned in the run;
//! * the talent frame (N and its micro button, at any level), read-only, showing the class's
//!   bought talents of our own trees (`talents.ron`) on the stock tree art, with our tooltips.
//!
//! Everything else the stock interface would open is blocked, keys and buttons alike, and its
//! micro and bag buttons are hidden ([`LUA`] lists it all); the main menu's micro button stays and
//! opens the run's Escape menu. Besides:
//!
//! * the main action bar is compact ([`action_bar`]): eight buttons, the page arrows, four micro
//!   buttons, a gryphon at each end, the experience bar over it in ten segments;
//! * the character pane's stat rows show the run's numbers (the server's `ESURV` line, read
//!   through the addon message event) and what worn gear adds, each row with a tooltip of ours;
//! * the stock interface's level cap (`MAX_PLAYER_LEVEL`) is the run's
//!   ([`esurv_sim::game::level::MAX_LEVEL`]), so the experience bar behaves at the run's cap;
//! * the stock level-up block in the chat is not printed in a run: the server says what a level
//!   brought in a line of its own, and a fresh run's level going back to 1 is no level-up;
//! * a spell's tooltip on the action bar and in the spellbook, and its name and rank in the
//!   spellbook, are ours ([`spell_tips`]): what the spell does in the run, with the hero's
//!   numbers, not what the stock spell it looks like does.
//!
//! Character select is untouched: the
//! Lua acts only while `ESURV_RUN_FRAMES_ON` is set, which this module keeps to whether the
//! hero is in the world.
//!
//! It is all Lua in the interface VM, ours ([`LUA`], `run_frames.lua`), run once in each VM once
//! the stock FrameXML is loaded; the talent pages are pushed into it from the content and the
//! server's `Progress`.

use std::fmt::Write as _;

use bevy::prelude::*;

use benilla_app::{NetHandlerApp, Objects};
use benilla_protocol::{SessionEvent, SessionEventKind};
use benilla_ui::script::UiScript;
use esurv_content::{Content, Talent};
use esurv_protocol::{Progress, ToClient, NUMBERS_PREFIX};
use esurv_sim::game::level::MAX_LEVEL;
use esurv_sim::game::stats::MAX_DAMAGE_REDUCTION;

use crate::hero_sheet::talents::{self, is_milestone, talent_text, trees};
use crate::protocol::ServerEvent;
use crate::stock::TipLine;
use crate::ui::{in_world, GameContent, GOLD_TEXT, GRAY_TEXT, GREEN_TEXT, WHITE_TEXT};

pub mod action_bar;
pub mod buff_tips;
pub mod class_bar;
mod key_presses;
pub mod spell_tips;
mod tooltip_shots;

/// The Lua half: the wrappers, the hidden buttons, the spellbook's one tab and the talent frame's
/// data source. Returns true once installed.
pub const LUA: &str = include_str!("run_frames.lua");

/// The run's frames: the Lua installed in each VM, the run flag, the talent pages.
pub struct RunFramesPlugin;

impl Plugin for RunFramesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RunProgress>()
            .init_resource::<RunNumbers>()
            .net_handler(SessionEventKind::Chat, keep_the_numbers)
            .add_systems(Update, (keep_progress, keep_the_frames).chain());
        key_presses::add(app);
        spell_tips::add(app);
        buff_tips::add(app);
        class_bar::add(app);
        tooltip_shots::add(app);
    }
}

/// The class's progress as the server last said it (`Progress`), for the talent frame.
#[derive(Resource, Default)]
struct RunProgress(Option<Progress>);

fn keep_progress(
    mut events: MessageReader<ServerEvent>,
    objects: Objects,
    mut was_in: Local<bool>,
    mut progress: ResMut<RunProgress>,
    mut numbers: ResMut<RunNumbers>,
) {
    let now_in = in_world(&objects);
    if *was_in && !now_in {
        progress.0 = None;
        numbers.0 = None;
    }
    *was_in = now_in;
    for ServerEvent(m) in events.read() {
        if let ToClient::Progress(p) = m {
            progress.0 = Some(p.clone());
        }
    }
}

/// The run's numbers line as the server last sent it (its message, after `ESURV` and the TAB): the
/// Lua reads every line through the addon message event, and is handed this one when it is
/// installed, since the first line can come before it.
#[derive(Resource, Default)]
struct RunNumbers(Option<String>);

fn keep_the_numbers(In(ev): In<SessionEvent>, mut numbers: ResMut<RunNumbers>) {
    let SessionEvent::Chat(line) = ev else {
        return;
    };
    if !line.is_addon() {
        return;
    }
    if let Some(message) = numbers_message(&line.text) {
        numbers.0 = Some(message.to_owned());
    }
}

/// The message of a numbers line (`ESURV` TAB `key=value;...`); `None` for any other line.
pub fn numbers_message(line: &str) -> Option<&str> {
    let (prefix, message) = line.split_once('\t')?;
    (prefix == NUMBERS_PREFIX).then_some(message)
}

/// The Lua that hands the run's limits to the frames: its level cap and the most damage
/// reduction there is.
pub fn limits_chunk() -> String {
    format!("ESURV_RUN_FRAMES.set_limits({MAX_LEVEL}, {MAX_DAMAGE_REDUCTION})")
}

/// What this VM has been told.
#[derive(Default)]
struct Told {
    /// The VM (`UiScript::session`) the rest is about.
    session: u64,
    installed: bool,
    on: bool,
    talents: Option<String>,
    /// The numbers line it was handed.
    numbers: Option<String>,
}

/// Installs the Lua in a fresh VM, keeps the run flag to the hero being in the world, and pushes
/// the talent pages when they change.
fn keep_the_frames(
    script: Option<NonSend<UiScript>>,
    objects: Objects,
    progress: Res<RunProgress>,
    numbers: Res<RunNumbers>,
    content: Option<Res<GameContent>>,
    mut told: Local<Told>,
    mut warned: Local<bool>,
) {
    let Some(script) = script else {
        return;
    };
    if told.session != script.session() {
        *told = Told {
            session: script.session(),
            ..default()
        };
    }
    let in_world = in_world(&objects);
    if !told.installed {
        // only once the hero is in the world: the stock frames it wraps are loaded by then
        if !in_world {
            return;
        }
        match script.eval::<bool>(LUA) {
            Ok(true) => {
                info!("run frames: installed in the interface");
                told.installed = true;
                if let Err(e) = script.run(&limits_chunk()) {
                    warn!("run frames: the run's limits: {e}");
                }
                // the compact bar, switched with the frames (their `apply`)
                match script.eval::<bool>(action_bar::LUA) {
                    Ok(true) => {}
                    Ok(false) => warn!("run frames: the action bar: the stock bar is not loaded"),
                    Err(e) => warn!("run frames: the action bar did not load: {e}"),
                }
            }
            Ok(false) => return,
            Err(e) => {
                if !std::mem::replace(&mut *warned, true) {
                    warn!("run frames: the Lua did not load: {e}");
                }
                return;
            }
        }
    }
    if told.on != in_world {
        let flag = if in_world { "1" } else { "nil" };
        let chunk = format!("ESURV_RUN_FRAMES_ON = {flag}; ESURV_RUN_FRAMES.apply()");
        if let Err(e) = script.run(&chunk) {
            warn!("run frames: {e}");
        }
        told.on = in_world;
    }
    // the numbers line the Lua may have missed (one that came before it was installed)
    if in_world && told.numbers != numbers.0 {
        if let Some(line) = &numbers.0 {
            let chunk = format!("ESURV_RUN_FRAMES.take_stats({})", lua_string(line));
            if let Err(e) = script.run(&chunk) {
                warn!("run frames: the run's numbers: {e}");
            }
        }
        told.numbers = numbers.0.clone();
    }
    let pages = match (&progress.0, &content) {
        (Some(p), Some(c)) => lua_pages(&run_talents(&c.0, p)),
        _ => "{}".to_owned(),
    };
    if told.talents.as_deref() != Some(pages.as_str()) {
        if let Err(e) = script.run(&format!("ESURV_RUN_FRAMES.set_talents({pages})")) {
            warn!("run frames: the talent pages: {e}");
        }
        told.talents = Some(pages);
    }
}

/// One tree as the run's talent frame shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct RunTalentTab {
    pub name: String,
    /// The stock art's base (`Interface\TalentFrame\<background>-TopLeft` ...).
    pub background: String,
    pub points: u32,
    pub talents: Vec<RunTalent>,
}

/// One bought talent, at its place in its tree.
#[derive(Clone, Debug, PartialEq)]
pub struct RunTalent {
    pub name: String,
    pub icon: String,
    /// 1-based, as the stock frame counts.
    pub tier: u32,
    pub column: u32,
    pub rank: u32,
    pub max_rank: u32,
    /// Where the talent it needs sits, when that one is shown too (the stock branch arrow).
    pub prereq: Option<(u32, u32)>,
    pub tip: Vec<TipLine>,
}

/// The class's three trees with only the talents bought (rank 1 or more), each in its place.
pub fn run_talents(content: &Content, progress: &Progress) -> Vec<RunTalentTab> {
    let class = progress.class.as_str();
    let ranks = talents::ranks(progress);
    let rank_of = |t: &Talent| ranks.get(&t.id).copied().unwrap_or(0);
    trees(content, class)
        .into_iter()
        .enumerate()
        .map(|(i, tree)| {
            let shown: Vec<&Talent> = tree.talents.iter().filter(|t| rank_of(t) > 0).collect();
            let talents = shown
                .iter()
                .map(|t| {
                    let prereq = t.requires.as_ref().and_then(|id| {
                        shown
                            .iter()
                            .find(|r| &r.id == id)
                            .map(|r| (u32::from(r.row) + 1, u32::from(r.col) + 1))
                    });
                    RunTalent {
                        name: t.name.clone(),
                        icon: format!(r"Interface\Icons\{}", t.icon),
                        tier: u32::from(t.row) + 1,
                        column: u32::from(t.col) + 1,
                        rank: rank_of(t),
                        max_rank: u32::from(t.max_rank),
                        prereq,
                        tip: read_only_tip(t, rank_of(t)),
                    }
                })
                .collect();
            RunTalentTab {
                name: tree.name.clone(),
                background: tree.background.clone(),
                points: talents::points_in(content, class, &ranks, i),
                talents,
            }
        })
        .collect()
}

/// Said under every talent in a run: where talents change.
pub const CHANGED_BEFORE_A_RUN: &str =
    "Talents are learned and taken back on the hero sheet, before a run.";

/// A bought talent's tooltip in a run, as the stock talent tooltip lays it out: its name, its
/// rank, what it does now (gold), what the next rank would do, a milestone's line, and where
/// talents change. Nothing to click.
pub fn read_only_tip(t: &Talent, rank: u32) -> Vec<TipLine> {
    let max_rank = u32::from(t.max_rank);
    let mut lines = vec![
        TipLine::new(t.name.clone(), WHITE_TEXT),
        TipLine::new(format!("Rank {rank}/{max_rank}"), WHITE_TEXT),
        TipLine::new(talent_text(t, rank), GOLD_TEXT),
    ];
    if rank < max_rank {
        lines.push(TipLine::new("Next rank:", WHITE_TEXT));
        lines.push(TipLine::new(talent_text(t, rank + 1), GOLD_TEXT));
    }
    if is_milestone(t) {
        lines.push(TipLine::new(
            "A milestone: it changes how your spells play.",
            GREEN_TEXT,
        ));
    }
    lines.push(TipLine::new(CHANGED_BEFORE_A_RUN, GRAY_TEXT));
    lines
}

/// The pages as the Lua table `ESURV_RUN_FRAMES.set_talents` takes.
pub fn lua_pages(tabs: &[RunTalentTab]) -> String {
    let mut out = String::from("{");
    for tab in tabs {
        let _ = write!(
            out,
            "{{name={},background={},points={},talents={{",
            lua_string(&tab.name),
            lua_string(&tab.background),
            tab.points
        );
        for t in &tab.talents {
            let _ = write!(
                out,
                "{{name={},icon={},tier={},column={},rank={},max={},prereqs={{",
                lua_string(&t.name),
                lua_string(&t.icon),
                t.tier,
                t.column,
                t.rank,
                t.max_rank
            );
            if let Some((tier, column)) = t.prereq {
                let _ = write!(out, "{{{tier},{column}}}");
            }
            out.push_str("},tip={");
            for line in &t.tip {
                let c = line.colour.to_srgba();
                let _ = write!(
                    out,
                    "{{{},{:.3},{:.3},{:.3}}},",
                    lua_string(&line.text),
                    c.red,
                    c.green,
                    c.blue
                );
            }
            out.push_str("}},");
        }
        out.push_str("}},");
    }
    out.push('}');
    out
}

/// `s` as a Lua string literal.
pub fn lua_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\{}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero_sheet::gear::tests::content;

    fn progress(talents: Vec<(&str, u32)>) -> Progress {
        Progress {
            class: "warrior".into(),
            talent_experience: 0,
            points: 20,
            spent: talents.iter().map(|(_, r)| r).sum(),
            into_next: 0,
            next_cost: 80,
            zones_open: 1,
            zones_discovered: Vec::new(),
            talents: talents.into_iter().map(|(i, r)| (i.into(), r)).collect(),
            worn: vec![],
            looks: vec![],
            collection: vec![],
            prestige: Default::default(),
            loadout: String::new(),
            new_gear: false,
            new_points: false,
            challenge_open: false,
            challenge_news: false,
        }
    }

    #[test]
    fn only_bought_talents_show_each_in_its_place() {
        let c = content();
        let tabs = run_talents(c, &progress(vec![("imp_heroic", 3), ("deflection", 5)]));
        assert_eq!(tabs.len(), 3, "the class's three trees");
        assert_eq!(tabs[0].name, "TalentTab 161");
        assert_eq!(tabs[0].background, "WarriorArms");
        assert_eq!(tabs[0].points, 8);
        let names: Vec<&str> = tabs[0].talents.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Spell 12282", "Spell 13713"]);
        let heroic = &tabs[0].talents[0];
        assert_eq!(
            (heroic.tier, heroic.column, heroic.rank, heroic.max_rank),
            (1, 1, 3, 5)
        );
        assert_eq!(heroic.icon, r"Interface\Icons\Ability_Rogue_Ambush");
        assert!(tabs[1].talents.is_empty() && tabs[2].talents.is_empty());
        assert_eq!(tabs[1].points, 0);
    }

    #[test]
    fn a_needed_talent_draws_its_arrow_only_when_it_shows() {
        let c = content();
        let tree = &trees(c, "warrior")[0];
        let needs = tree
            .talents
            .iter()
            .find(|t| t.requires.is_some())
            .expect("a talent that needs another");
        let needed = tree
            .talents
            .iter()
            .find(|t| Some(&t.id) == needs.requires.as_ref())
            .unwrap();
        let both = progress(vec![
            (&needed.id, u32::from(needed.max_rank)),
            (&needs.id, 1),
        ]);
        let tab = &run_talents(c, &both)[0];
        let shown = tab.talents.iter().find(|t| t.name == needs.name).unwrap();
        assert_eq!(
            shown.prereq,
            Some((u32::from(needed.row) + 1, u32::from(needed.col) + 1))
        );
        let alone = progress(vec![(&needs.id, 1)]);
        let tab = &run_talents(c, &alone)[0];
        assert_eq!(
            tab.talents[0].prereq, None,
            "no arrow from a talent not shown"
        );
    }

    #[test]
    fn the_tooltip_has_nothing_to_click() {
        let c = content();
        let t = &trees(c, "warrior")[0].talents[0];
        let tip = read_only_tip(t, 2);
        assert_eq!(tip[0].text, t.name);
        assert_eq!(tip[1].text, format!("Rank 2/{}", t.max_rank));
        assert!(tip.iter().any(|l| l.text == "Next rank:"));
        assert_eq!(tip.last().unwrap().text, CHANGED_BEFORE_A_RUN);
        assert!(
            !tip.iter().any(|l| l.text.contains("lick")),
            "no click line"
        );
        let maxed = read_only_tip(t, u32::from(t.max_rank));
        assert!(!maxed.iter().any(|l| l.text == "Next rank:"));
    }

    #[test]
    fn the_pages_read_as_lua() {
        let c = content();
        let lua = lua_pages(&run_talents(c, &progress(vec![("imp_heroic", 1)])));
        assert!(lua.starts_with("{{name=\"TalentTab 161\",background=\"WarriorArms\",points=1,"));
        assert!(lua.contains("icon=\"Interface\\\\Icons\\\\Ability_Rogue_Ambush\""));
        assert_eq!(lua.matches('{').count(), lua.matches('}').count());
        assert_eq!(lua_string("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
        assert_eq!(lua_pages(&[]), "{}");
    }

    #[test]
    fn the_lua_installs_in_a_vm_with_the_stock_functions() {
        let lua = mlua::Lua::new();
        // without the stock frames loaded it waits
        assert!(!lua.load(LUA).eval::<bool>().unwrap());
        lua.load(STOCK_STAND_INS).exec().unwrap();
        assert!(lua.load(LUA).eval::<bool>().unwrap());
        assert!(lua.load(LUA).eval::<bool>().unwrap(), "once is enough");
        // off: every function is the stock one
        let opened: i64 = lua
            .load("ToggleQuestLog(); ToggleTalentFrame(); return OPENED")
            .eval()
            .unwrap();
        assert_eq!(opened, 2, "the stock talent frame too");
        assert_eq!(
            lua.load("return GetNumSpellTabs()").eval::<i64>().unwrap(),
            3
        );
        // on: blocked keys open nothing; the spellbook has one tab with every spell
        lua.load("ESURV_RUN_FRAMES_ON = 1; ESURV_RUN_FRAMES.apply()")
            .exec()
            .unwrap();
        let opened: i64 = lua
            .load(
                "OPENED = 0; ToggleQuestLog(); ToggleFriendsFrame(); ToggleWorldMap(); \
                 OpenAllBags(); ToggleCharacter('ReputationFrame'); \
                 ToggleSpellBook(BOOKTYPE_PET); return OPENED",
            )
            .eval()
            .unwrap();
        assert_eq!(opened, 0);
        let kept: i64 = lua
            .load(
                "OPENED = 0; ToggleCharacter('PaperDollFrame'); ToggleSpellBook(BOOKTYPE_SPELL); \
                 return OPENED",
            )
            .eval()
            .unwrap();
        assert_eq!(kept, 2, "the character pane and the spellbook");
        let (tabs, name, offset, count): (i64, String, i64, i64) = lua
            .load(
                "local n, _, o, c = GetSpellTabInfo(1); \
                 return GetNumSpellTabs(), n, o, c",
            )
            .eval()
            .unwrap();
        assert_eq!((tabs, name.as_str(), offset, count), (1, "General", 0, 9));
        // the talent frame reads our pages, learns nothing, and its button shows at level 1
        lua.load(format!(
            "ESURV_RUN_FRAMES.set_talents({})",
            lua_pages(&run_talents(content(), &progress(vec![("imp_heroic", 2)])))
        ))
        .exec()
        .unwrap();
        let (tabs, name, rank, points): (i64, String, i64, i64) = lua
            .load(
                "local n, _, _, _, r = GetTalentInfo(1, 1); \
                 return GetNumTalentTabs(), n, r, UnitCharacterPoints('player')",
            )
            .eval()
            .unwrap();
        assert_eq!(
            (tabs, name.as_str(), rank, points),
            (3, "Spell 12282", 2, 0)
        );
        let learned: i64 = lua
            .load("LEARNED = 0; LearnTalent(1, 1); return LEARNED")
            .eval()
            .unwrap();
        assert_eq!(learned, 0, "nothing is learned in a run");
        assert!(lua
            .load("return TALENT_BUTTON_SHOWN")
            .eval::<bool>()
            .unwrap());
        // the hidden buttons, and back
        assert!(!lua
            .load("return QuestLogMicroButton:IsShown()")
            .eval::<bool>()
            .unwrap());
        assert!(
            lua.load("return MainMenuMicroButton:IsShown()").eval::<bool>().unwrap(),
            "the main menu button stays: it opens the run's Escape menu"
        );
        lua.load("ESURV_RUN_FRAMES_ON = nil; ESURV_RUN_FRAMES.apply()")
            .exec()
            .unwrap();
        assert!(lua
            .load("return QuestLogMicroButton:IsShown()")
            .eval::<bool>()
            .unwrap());
    }

    /// The character pane's rows, the addon message listener, the level cap and the chat's
    /// level-up block, in a bare VM with stand-ins for the stock pieces they touch.
    #[test]
    fn the_pane_the_level_cap_and_the_level_up_block_in_a_run() {
        let lua = mlua::Lua::new();
        lua.load(STOCK_STAND_INS).exec().unwrap();
        lua.load(PANE_STAND_INS).exec().unwrap();
        assert!(lua.load(LUA).eval::<bool>().unwrap());
        lua.load(limits_chunk()).exec().unwrap();
        let int = |chunk: &str| lua.load(chunk).eval::<i64>().unwrap();
        let text = |chunk: &str| lua.load(chunk).eval::<String>().unwrap();
        // off: the stock cap and the stock level-up block
        assert_eq!(int("return MAX_PLAYER_LEVEL"), 60);
        assert_eq!(
            int("CHAT_LINES = 0; ChatFrame_OnEvent('PLAYER_LEVEL_UP'); return CHAT_LINES"),
            1
        );
        // on: the run's cap, the bars told, no stock level-up block, other chat as before
        lua.load("ESURV_RUN_FRAMES_ON = 1; ESURV_RUN_FRAMES.apply()")
            .exec()
            .unwrap();
        assert_eq!(int("return MAX_PLAYER_LEVEL"), i64::from(MAX_LEVEL));
        assert!(
            int("return BARS_TOLD") > 0,
            "the experience bar re-reads the cap"
        );
        assert_eq!(
            int("CHAT_LINES = 0; ChatFrame_OnEvent('PLAYER_LEVEL_UP'); \
                 ChatFrame_OnEvent('CHAT_MSG_SYSTEM'); return CHAT_LINES"),
            1,
            "only the server's own line"
        );
        // the numbers line through the addon message event, onto the open pane
        let line = "health=241;damage=1.090;cooldown=0.870;crit=0.050;crit_damage=2.00;reduction=0.250;\
                    regen=1.04;area=1.000;targets=1;double_cast=0.000;speed=1.000;pickup=9.0;experience=1.00;\
                    luck=0.00;gold=1.10;heal_per_kill=0.0;gear_health=11;gear_damage=0.030;gear_cooldown=0.000;\
                    gear_crit=0.000;gear_crit_damage=0.00;gear_reduction=0.030;gear_regen=0.00;gear_area=0.000;\
                    gear_targets=0;gear_double_cast=0.000;gear_speed=0.000;gear_pickup=0.000;gear_gold=0.100";
        lua.load(format!(
            "event = 'CHAT_MSG_ADDON'; arg1 = 'ESURV'; arg2 = {}; \
             EsurvRunNumbers.scripts.OnEvent()",
            lua_string(line)
        ))
        .exec()
        .unwrap();
        assert_eq!(text("return CharacterStatFrame1Label:GetText()"), "Health:");
        assert_eq!(
            text("return CharacterStatFrame1StatText:GetText()"),
            "|cff20ff20241|r",
            "worn gear adds to it: green"
        );
        assert_eq!(
            text("return CharacterStatFrame2StatText:GetText()"),
            "|cff20ff20+9%|r"
        );
        assert_eq!(text("return CharacterStatFrame3StatText:GetText()"), "-13%");
        assert_eq!(
            text("return CharacterArmorFrameStatText:GetText()"),
            "|cff20ff2025%|r"
        );
        assert_eq!(text("return CharacterDamageFrameStatText:GetText()"), "+1");
        assert_eq!(
            text("return CharacterRangedDamageFrameStatText:GetText()"),
            "9.0 yd"
        );
        assert!(!lua
            .load("return CharacterResistanceFrame:IsShown()")
            .eval::<bool>()
            .unwrap());
        // a row's tooltip: its value, our lines, what gear adds
        let tip = text(
            "TIP = {}; this = CharacterArmorFrame; CharacterArmorFrame.scripts.OnEnter(); \
             return table.concat(TIP, '|')",
        );
        assert!(
            tip.starts_with("Reduction 25%|Damage taken is lowered by 25%."),
            "{tip}"
        );
        assert!(tip.contains("It never goes past 60%."), "{tip}");
        assert!(
            tip.ends_with("Worn gear adds +3% damage reduction."),
            "{tip}"
        );
        // the pane refills its rows when it shows: ours come after the stock ones
        lua.load("PaperDollFrame_OnShow()").exec().unwrap();
        assert_eq!(text("return CharacterStatFrame1Label:GetText()"), "Health:");
        // off again: the stock cap, the resistances back, the numbers forgotten
        lua.load("ESURV_RUN_FRAMES_ON = nil; ESURV_RUN_FRAMES.apply()")
            .exec()
            .unwrap();
        assert_eq!(int("return MAX_PLAYER_LEVEL"), 60);
        assert!(lua
            .load(
                "return CharacterResistanceFrame:IsShown() and ESURV_RUN_FRAMES.stats == nil"
            )
            .eval::<bool>()
            .unwrap());
        lua.load("PaperDollFrame_OnShow()").exec().unwrap();
        assert_eq!(
            text("return CharacterStatFrame1Label:GetText()"),
            "Strength:",
            "the stock pane outside a run"
        );
    }

    #[test]
    fn a_numbers_line_is_told_apart() {
        assert_eq!(
            numbers_message("ESURV\thealth=230;damage=1.000"),
            Some("health=230;damage=1.000")
        );
        assert_eq!(numbers_message("ESURVS\tF130 8187 1.250"), None);
        assert_eq!(numbers_message("ESURV health=1"), None);
        assert!(limits_chunk().starts_with(&format!(
            "ESURV_RUN_FRAMES.set_limits({MAX_LEVEL}, 0.6"
        )));
    }

    /// The stock tooltip's spell setters and `GetSpellName`, which our spell tooltips answer.
    const TOOLTIP_STAND_INS: &str = r#"
        GameTooltip = {}
        function GameTooltip:ClearLines() TIP = {} end
        function GameTooltip:AddLine(t) table.insert(TIP, t) end
        function GameTooltip:AddDoubleLine(l, r) table.insert(TIP, l .. " | " .. r) end
        function GameTooltip:Show() end
        function GameTooltip:SetAction(a) TIP = { "stock action " .. a } end
        function GameTooltip:SetSpell(i, b) TIP = { "stock spell " .. i } end
        function GetSpellName(i, b) return "Stock " .. i, "Rank 1" end
        function GetActionTexture(a) return "stock icon " .. a end
        function GetActionCooldown(a) if a == 1 then return 10, 1.5, 1 end return 10, 8, 1 end
        function GetSpellTexture(i, b) return "stock icon " .. i end
        function GameTooltip:SetPlayerBuff(i) TIP = { "stock buff " .. i } end
        function GameTooltip:SetUnitBuff(u, i) TIP = { "stock unit buff " .. i } end
        function GameTooltip:SetUnitDebuff(u, i) TIP = { "stock unit debuff " .. i } end
        function GetPlayerBuffTimeLeft(i) return 7.2 end
        TARGET_IS_PLAYER = false
        function UnitIsUnit(a, b) return TARGET_IS_PLAYER end
    "#;

    /// In a run the buff frame's buffs, and the target frame's when the hero targets itself, show
    /// our buff tooltips with the time left; an enemy's debuff of one of the hero's spells shows
    /// the spell's; anything else, and everything outside a run, is the stock one.
    #[test]
    fn our_buff_tooltips_answer_the_stock_setters_in_a_run() {
        use esurv_protocol::{BuffKind, BuffTip};
        let lua = mlua::Lua::new();
        lua.load(STOCK_STAND_INS).exec().unwrap();
        lua.load(TOOLTIP_STAND_INS).exec().unwrap();
        assert!(lua.load(LUA).eval::<bool>().unwrap());
        let c = content();
        let rune = buff_tips::buff_text(
            c,
            &[],
            &BuffTip {
                look: 23451,
                kind: BuffKind::Speed,
                spell: String::new(),
                value: 0.6,
                stacks: 1,
            },
        )
        .unwrap();
        let debuff = spell_tips::SpellTipText {
            name: "Spell 772".into(),
            sub: String::new(),
            rows: vec![spell_tips::TipRow::one("Spell 772", WHITE_TEXT)],
        };
        lua.load(format!(
            "ESURV_RUN_FRAMES.set_buff_tips({}, {}); \
             ESURV_RUN_FRAMES.set_aura_slots({{[0]=23451}}, {{[1]=23451}}, {{[1]=772}})",
            spell_tips::lua_tips(&[(23451, rune)]),
            spell_tips::lua_tips(&[(772, debuff)])
        ))
        .exec()
        .unwrap();
        let text = |chunk: &str| lua.load(chunk).eval::<String>().unwrap();
        let tip = "GameTooltip:SetPlayerBuff(0); return table.concat(TIP, ' / ')";
        assert_eq!(text(tip), "stock buff 0", "outside a run");
        lua.load("ESURV_RUN_FRAMES_ON = 1; ESURV_RUN_FRAMES.apply()")
            .exec()
            .unwrap();
        assert_eq!(text(tip), "Speed Rune / +60% movement speed / 7 sec left");
        assert_eq!(
            text("GameTooltip:SetPlayerBuff(1); return table.concat(TIP, ' / ')"),
            "stock buff 1",
            "a position with no buff of ours"
        );
        // the target frame: the hero's own buffs while it targets itself; an enemy's debuffs
        assert_eq!(
            text("GameTooltip:SetUnitBuff('target', 1); return TIP[1]"),
            "stock unit buff 1",
            "another unit's buff"
        );
        assert_eq!(
            text("TARGET_IS_PLAYER = true; GameTooltip:SetUnitBuff('target', 1); return TIP[1]"),
            "Speed Rune"
        );
        assert_eq!(
            text("TARGET_IS_PLAYER = false; GameTooltip:SetUnitDebuff('target', 1); return TIP[1]"),
            "Spell 772",
            "the hero's spell on an enemy"
        );
        assert_eq!(
            text("GameTooltip:SetUnitDebuff('target', 2); return TIP[1]"),
            "stock unit debuff 2",
            "another debuff"
        );
    }

    /// In a run the action bar's and the spellbook's spells show our tooltips and names, by the
    /// spell each slot holds; any other slot, and everything outside a run, is the stock one.
    #[test]
    fn our_spell_tooltips_answer_the_stock_setters_in_a_run() {
        use esurv_protocol::{OfferValue, SpellTip, TipKind};
        let lua = mlua::Lua::new();
        lua.load(STOCK_STAND_INS).exec().unwrap();
        lua.load(TOOLTIP_STAND_INS).exec().unwrap();
        assert!(lua.load(LUA).eval::<bool>().unwrap());
        let c = content();
        let fireball = c.ability("fireball").unwrap();
        let tips = spell_tips::lua_spell_tips(
            c,
            &[SpellTip {
                look: fireball.look_spell,
                kind: TipKind::Learned,
                id: "fireball".into(),
                rank: 2,
                numbers: vec![OfferValue {
                    what: "cooldown".into(),
                    value: 2.0,
                }],
                upgrades: vec![],
                bonuses: vec![],
                crit: None,
            }],
        );
        let icons = spell_tips::lua_spell_icons(
            c,
            &[SpellTip {
                look: fireball.look_spell,
                kind: TipKind::Learned,
                id: "fireball".into(),
                rank: 2,
                numbers: vec![],
                upgrades: vec![],
                bonuses: vec![],
                crit: None,
            }],
        );
        let id = fireball.look_spell;
        lua.load(format!(
            "ESURV_RUN_FRAMES.set_spell_tips({tips}); \
             ESURV_RUN_FRAMES.set_spell_icons({icons}); \
             ESURV_RUN_FRAMES.set_spell_slots({{[1]={id}}}, {{[3]={id}}})"
        ))
        .exec()
        .unwrap();
        let text = |chunk: &str| lua.load(chunk).eval::<String>().unwrap();
        let tip = "GameTooltip:SetAction(1); return table.concat(TIP, ' / ')";
        // off: the stock tooltip
        assert_eq!(text(tip), "stock action 1");
        lua.load("ESURV_RUN_FRAMES_ON = 1; ESURV_RUN_FRAMES.apply()")
            .exec()
            .unwrap();
        // the spell's own icon on the bar and in the book, the stock one elsewhere
        let own = format!(r"Interface\Icons\{}", fireball.icon);
        assert_eq!(text("return GetActionTexture(1)"), own);
        assert_eq!(text("return GetSpellTexture(3, 'spell')"), own);
        assert_eq!(text("return GetActionTexture(2)"), "stock icon 2");
        // no global cooldown sweep in a run; a spell's own cooldown still sweeps
        let cooldown = |a: u32| lua.load(format!("local s, d = GetActionCooldown({a}); return d")).eval::<f32>().unwrap();
        assert_eq!(cooldown(1), 0.0);
        assert_eq!(cooldown(2), 8.0);
        let shown = text(tip);
        assert!(
            shown.starts_with("Spell 133 | Rank 2 / Fire spell / Casts itself | 2 sec cooldown"),
            "{shown}"
        );
        assert_eq!(
            text("GameTooltip:SetAction(2); return table.concat(TIP, ' / ')"),
            "stock action 2",
            "a slot without our spell"
        );
        assert!(text("GameTooltip:SetSpell(3, 'spell'); return TIP[1]").starts_with("Spell 133"));
        assert_eq!(
            text("GameTooltip:SetSpell(3, 'pet'); return TIP[1]"),
            "stock spell 3",
            "the pet book is the stock one"
        );
        assert_eq!(
            text("local n, r = GetSpellName(3, 'spell'); return n .. ', ' .. r"),
            "Spell 133, Rank 2"
        );
        assert_eq!(
            text("local n, r = GetSpellName(4, 'spell'); return n .. ', ' .. r"),
            "Stock 4, Rank 1"
        );
        lua.load("ESURV_RUN_FRAMES_ON = nil; ESURV_RUN_FRAMES.apply()")
            .exec()
            .unwrap();
        assert_eq!(
            text(tip),
            "stock action 1",
            "outside a run, the stock one again"
        );
    }

    /// The character pane's rows (a label and a value each, a tooltip script), its resistance
    /// icons and stock row setters, `CreateFrame`, the chat frame's handler, the level bars.
    const PANE_STAND_INS: &str = r#"
        MAX_PLAYER_LEVEL = 60; BARS_TOLD = 0; CHAT_LINES = 0
        NORMAL_FONT_COLOR = { r = 1, g = 0.82, b = 0 }
        function UnitLevel() return 12 end
        function ReputationWatchBar_Update() BARS_TOLD = BARS_TOLD + 1 end
        function MainMenuExpBar_Update() BARS_TOLD = BARS_TOLD + 1 end
        function ChatFrame_OnEvent(ev) CHAT_LINES = CHAT_LINES + 1 end
        local function text_of() local t = {}; function t:SetText(v) self.v = v end
            function t:GetText() return self.v end; return t end
        local function widget(name)
            local f = { shown = true, scripts = {} }
            function f:IsShown() return self.shown end
            function f:IsVisible() return self.shown end
            function f:Show() self.shown = true end
            function f:Hide() self.shown = false end
            function f:GetScript(s) return self.scripts[s] end
            function f:SetScript(s, fn) self.scripts[s] = fn end
            function f:RegisterEvent(e) self.event = e end
            if name then _G[name] = f end
            return f
        end
        function CreateFrame(kind, name) return widget(name) end
        widget("PaperDollFrame"); widget("CharacterResistanceFrame")
        local rows = { "CharacterStatFrame1", "CharacterStatFrame2", "CharacterStatFrame3",
            "CharacterStatFrame4", "CharacterStatFrame5", "CharacterArmorFrame",
            "CharacterAttackFrame", "CharacterAttackPowerFrame", "CharacterDamageFrame",
            "CharacterRangedAttackFrame", "CharacterRangedAttackPowerFrame",
            "CharacterRangedDamageFrame" }
        for _, r in ipairs(rows) do
            local f = widget(r); f.scripts.OnEnter = function() end
            _G[r .. "Label"] = text_of(); _G[r .. "StatText"] = text_of()
        end
        function PaperDollFrame_OnShow() CharacterStatFrame1Label:SetText("Strength:") end
        function PaperDollFrame_OnEvent() end
        GameTooltip = {}
        function GameTooltip:SetOwner() TIP = {} end
        function GameTooltip:SetText(t) table.insert(TIP, t) end
        function GameTooltip:AddLine(t) table.insert(TIP, t) end
        function GameTooltip:Show() end
    "#;

    /// Just enough of the stock interface for the Lua to wrap: toggles that count what they open.
    const STOCK_STAND_INS: &str = r#"
        OPENED = 0; LEARNED = 0; GENERAL = "General"
        BOOKTYPE_SPELL = "spell"; BOOKTYPE_PET = "pet"
        local function opens() OPENED = OPENED + 1 end
        for _, n in ipairs({ "ToggleQuestLog", "ToggleFriendsFrame", "ToggleWorldMap",
            "OpenAllBags", "ToggleCharacter", "ToggleSpellBook", "ShowUIPanel",
            "ToggleDropDownMenu" }) do _G[n] = opens end
        function ToggleTalentFrame() opens() end
        function HideUIPanel(f) f:Hide() end
        local tabs = { { "General", "a", 0, 3 }, { "Fire", "b", 3, 4 }, { "Frost", "c", 7, 2 } }
        function GetNumSpellTabs() return 3 end
        function GetSpellTabInfo(i) local t = tabs[i]; if t then return t[1], t[2], t[3], t[4] end end
        function GetNumTalentTabs() return 3 end
        function LearnTalent() LEARNED = LEARNED + 1 end
        function UnitCharacterPoints() return 5, 0 end
        local function frame(shown)
            local f = { shown = shown, scripts = {} }
            function f:IsShown() return self.shown end
            function f:IsVisible() return self.shown end
            function f:Show() self.shown = true; if self.scripts.OnShow then this = self; self.scripts.OnShow() end end
            function f:Hide() self.shown = false end
            function f:GetScript(s) return self.scripts[s] end
            function f:SetScript(s, fn) self.scripts[s] = fn end
            return f
        end
        QuestLogMicroButton = frame(true)
        MainMenuMicroButton = frame(true)
        TalentMicroButton = frame(false)
        function UpdateTalentButton() TalentMicroButton:Hide() end
        local show = TalentMicroButton.Show
        TalentMicroButton.Show = function(self) TALENT_BUTTON_SHOWN = true; show(self) end
    "#;
}
