//! The menus before a run (menus.md), in a look of their own ([`look`]):
//! they replace the stock select screen, the stock create screen and the realm list as the zone
//! picker. The flow:
//!
//! * a save with a hero opens on the warband screen (the camps of [`crate::picker`]); a save with
//!   none opens the class picker;
//! * the class picker ([`class_picker`]) opens the hero sheet on a new hero of the class;
//! * the hero sheet makes a hero, or changes one ("Modify selected character"), on its Character
//!   view; the warband screen's Collection and Talents buttons open it on those views (no
//!   tabs);
//! * the warband screen picks the hero, its zone and the run options, and starts the run.
//!
//! Everything a screen changes before a run goes to the server through the app's
//! [`crate::prerun::PreRunLink`]; creating, deleting and entering the world go over the world
//! connection through benilla's select hooks ([`benilla_app::SelectCreate`] ...). The screens are
//! drawn anew whenever what they show changes ([`MenuView`]); a click is a [`MenuPress`], which a
//! dev script can send too ([`dev`]).
//!
//! One piece also shows in a run: the Options panel, from the run's Escape menu, over the paused
//! run ([`over_the_run`]).

pub mod look;

mod achievements;
mod art;
mod class_picker;
mod codex;
mod companions;
mod decorate;
mod dev;
mod facts;
mod guide;
mod options;
pub(crate) mod outline;
mod over_the_camp;
mod over_the_run;
mod preview;
mod save_files;
pub(crate) mod scroll;
mod sheet;
mod warband;
mod wears;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use benilla_app::{
    CreateChoices, OverlayWindow, SelectActionDone, SelectCharacter, SelectCreate, SelectDelete,
    SelectEnterWorld, SelectModelImage, SelectRefresh, SelectScreen, SelectScreenReplaced,
};
use benilla_protocol::CharAction;
use esurv_content::{Content, Faction};
use esurv_protocol::{BuildAction, PreRunAsk, TimePeriod, ToServer};

use crate::achievements::AchievementsPage;
use crate::hero_sheet::codex::CodexList;
use crate::hero_sheet::item_icons::ItemIcons;
use crate::hero_sheet::racials::RacialIcons;
use crate::picker::PickerState;
use crate::prerun::PreRunLink;
use crate::stock::{StockClick, StockScale, StockSystems};
use crate::ui::GameContent;
use look::MenuLook;

pub use facts::PreRunFacts;

/// The menus' layer: over the camp's world and the stock interface, under the loading cover.
pub const MENU_LAYER: i32 = 1150;

/// Whether the menus replace the stock select screen: they do, but with `ESURV_MENUS=0` (a dev
/// aid, to see the stock screens; the run's Escape menu then has no Options panel either).
pub(crate) fn menus_on() -> bool {
    std::env::var("ESURV_MENUS").map_or(true, |v| v != "0")
}

/// The hero sheet's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SheetTab {
    #[default]
    Character,
    Collection,
    Talents,
}

/// The Talents window's pages: the talent trees, or the prestige stats (open once every talent
/// point is earned).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TalentsPage {
    #[default]
    Talents,
    Prestige,
}

/// The build bar's drop-downs on the Talents window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuildDrop {
    Start,
    Loadout,
}

/// What the hero sheet is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SheetMode {
    /// A new hero, made when the sheet is confirmed.
    Create,
    /// The hero of this guid ("Modify selected character").
    Modify(u64),
}

/// The hero as the sheet shapes it: its class (an id in the content), race (1.12 id), sex (0
/// male, 1 female), look (skin, face, hair style, hair colour, the fifth feature) and name.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Draft {
    pub class: String,
    pub race: u8,
    pub sex: u8,
    pub look: [u8; 5],
    pub name: String,
    /// Its temperaments (ids of `camp_chatter.ron`), the leading one first; none: Quiet.
    pub temperament: Vec<String>,
    /// The time period it was pulled in from (flavour: which of the camp's lines it says).
    pub time_period: TimePeriod,
}

/// The hero sheet's state.
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub mode: SheetMode,
    pub tab: SheetTab,
    pub draft: Draft,
    /// The talent picked on the Talents tab, by id.
    pub talent: Option<String>,
    /// The prestige stat picked under the talents, by id (picking one lets go of the other).
    pub prestige: Option<String>,
    /// The piece picked in the collection log: (slot, zone, the row's look; none: the zone's own).
    pub piece: Option<(u32, u32, Option<u32>)>,
    /// The equipment slot picked on the left, for its look choices.
    pub slot: Option<u32>,
    /// The Collection's Companions open in the details (the Companion box on the left), with the
    /// companion picked there (its id), if any.
    pub companions: bool,
    pub companion: Option<String>,
    /// The Collection's Title, Tabard or Weapon glow pick open in the details (its box on the
    /// left), with the choice picked there (its id), if any.
    pub wear: Option<esurv_protocol::WearKind>,
    pub wear_pick: Option<String>,
    /// Which of the temperaments' sample lines shows (a click on it shows the next).
    pub sample: usize,
    /// The Talents window's page: the talent trees or the prestige stats.
    pub talents_page: TalentsPage,
    /// The build bar's drop-down open: the starting spells or the weapon loadouts.
    pub build_drop: Option<BuildDrop>,
    /// The lit build's name as it is typed (Name starts it; Enter or Name again keeps it, Escape
    /// drops it).
    pub build_name: Option<String>,
}

/// Which screen of the menus shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Screen {
    /// None yet: the launch rule picks one once the roster is in.
    #[default]
    Unset,
    /// A sample of the look's pieces (`ESURV_MENUS_DEMO=1`).
    Sample,
    /// The class picker, the class picked.
    ClassPicker { class: String },
    /// The hero sheet.
    Sheet(Box<Sheet>),
    /// Character select: the warband in its camp.
    Warband,
}

/// What a click on a piece of the menus does.
#[derive(Component, Clone, Debug, PartialEq)]
pub enum MenuAction {
    /// Nothing: a piece that only shows (a sample button).
    Nothing,
    /// The class picker: pick a class, open the hero sheet on it, or go back.
    PickClass(String),
    ChooseClass,
    Back,
    /// Create a new hero: the class picker.
    CreateHero,
    /// The warband screen: the other camp; the hero sheet on the picked hero; delete it (after
    /// the question, answered No or Yes); a zone for its next run; the run options and a step of
    /// one of them; the run.
    SwitchCamp,
    ModifyHero,
    /// The hero sheet on the picked hero, straight at its Collection or its Talents (the
    /// warband screen's buttons over the armour); what that view shows is then
    /// seen.
    OpenHeroTab(SheetTab),
    DeleteHero,
    ConfirmNo,
    ConfirmYes,
    PickZone(String),
    /// The starting option (its id) the picked hero's next runs start with.
    PickStart(String),
    /// The weapon loadout (its id) the picked hero's class fights with from its next run
    ///.
    PickLoadout(String),
    OpenRunOptions,
    CloseRunOptions,
    OptionStep {
        slider: usize,
        up: bool,
    },
    /// The zone select's mode tabs, once the class has cleared Ahn'Qiraj: Normal Mode or
    /// Challenge Mode; and a step of the challenge slider.
    PickMode {
        challenge: bool,
    },
    ChallengeStep {
        up: bool,
    },
    /// The box telling that Challenge Mode is open, closed: never shown again for the save.
    CloseChallengeNews,
    StartRun,
    /// The window open over the camp closed: back to walking it ([`over_the_camp`]).
    CloseCampWindow,
    /// Pick the hero of this name (a dev script's press; a click on a hero in the camp picks it
    /// through benilla's scene).
    PickHeroNamed(String),
    /// The codex: open it, a list, the page before or after, close it.
    OpenCodex,
    CodexList(CodexList),
    CodexPage(bool),
    CloseCodex,
    /// The Achievements window (the warband screen's Achievements button): open it (what was
    /// earned is then seen), a page, an achievement tracked in runs or no longer, close it.
    OpenAchievements,
    AchievementsPage(AchievementsPage),
    TrackAchievement {
        id: String,
        on: bool,
    },
    CloseAchievements,
    /// The warband screen's Shop button: the shop alone (the Achievements window on its Shop).
    OpenShop,
    /// The Achievements window's Shop: a tile picked (its shop item), Buy (the question first),
    /// the question answered No or Yes.
    ShopPick(String),
    /// A tab of the Shop.
    ShopTab(companions::ShopTab),
    ShopBuy,
    ShopAnswer(bool),
    /// The hero sheet's Companions (the Collection's Companion box): open them, a companion
    /// picked there (its id), the hero to bring it (empty: none).
    OpenCompanions,
    CompanionPick(String),
    ChooseCompanion(String),
    /// The hero sheet's Title, Tabard and Weapon glow picks: open one, a choice picked there
    /// (its id), the hero to wear it (empty: none).
    OpenWear(esurv_protocol::WearKind),
    WearPick(esurv_protocol::WearKind, String),
    ChooseWear(esurv_protocol::WearKind, String),
    /// The warband screen's Decorate camp (beside the camp switch): open it, a spot picked,
    /// a piece set on it (empty: none), the spot opened for points (the question first, then
    /// its answer), close it.
    OpenDecorate,
    DecorateSpot(String),
    DecoratePiece(String),
    DecorateBuySpot,
    DecorateAnswer(bool),
    CloseDecorate,
    /// The guide (the warband screen's Guide button): open it, a tab, close it.
    OpenGuide,
    GuideTab(guide::GuideTab),
    CloseGuide,
    /// The game's settings ([`options`]), and closing the game.
    OpenGameOptions,
    QuitGame,
    /// The warband screen's update notice: download the newer version and hand over to its
    /// setup ([`crate::update`], which answers it).
    UpdateGame,
    /// The options panel: a tab, a setting flipped or stepped, Okay, Cancel.
    OptionsTab(options::OptionsTab),
    OptionsChange {
        key: &'static str,
        up: bool,
    },
    OptionsOkay,
    OptionsCancel,
    /// Over a paused run: the stock key binding window, opened from the options panel.
    KeyBindings,
    /// The options panel's Saves tab ([`save_files`]): export the save; pick a save in the
    /// folder (by name, or by its place in the list, `0` the newest, as a dev script names it);
    /// import it (the question first); the question answered No or Yes.
    ExportSave,
    PickSaveFile(String),
    ImportSave,
    ImportAnswer(bool),
    /// The hero sheet: a tab; a race, a sex, a step of a feature or every feature at random; a
    /// turn of the hero; Create or Save changes; a slot picked on the left (its looks).
    Tab(SheetTab),
    Race(u8),
    Sex(u8),
    Feature {
        index: usize,
        up: bool,
    },
    Randomize,
    /// The Temperament block: a temperament clicked (the leading one, then the second; a picked
    /// one let go), Quiet, two at random, the next sample line.
    Temperament(String),
    Quiet,
    RandomTemperament,
    /// The Time period block: a time period clicked.
    TimePeriod(TimePeriod),
    SampleLine,
    Turn(bool),
    ConfirmSheet,
    Slot(u32),
    /// The name typed in (a dev script's press; the keyboard types it otherwise).
    TypeName(String),
    /// The Collection tab: a piece picked (in the row of look `look`, a place in the class set's
    /// looks; none: its zone's own); wear it, take off what a slot wears, show a look (or the worn
    /// piece's own).
    Piece {
        slot: u32,
        zone: u32,
        look: Option<u32>,
    },
    Wear {
        slot: u32,
        zone: u32,
    },
    TakeOff(u32),
    ShowLook {
        slot: u32,
        look: Option<u32>,
    },
    /// The Talents tab: a talent clicked (picks it, or learns a rank once picked) or right-clicked
    /// (a rank back); a prestige stat the same; the pick's rank learned or taken back; every
    /// rank, talent and prestige, taken back.
    Talent(String),
    TalentBack(String),
    Prestige(String),
    PrestigeBack(String),
    Learn,
    Unlearn,
    ResetTalents,
    /// The Talents window: a page shown; a build bar drop-down opened (or closed, `None`); a saved
    /// build put back, the build lit saved from what the class holds now, a saved build emptied
    /// (a right click on it).
    TalentsPage(TalentsPage),
    BuildDrop(Option<BuildDrop>),
    UseBuild(u8),
    SaveBuild,
    ClearBuild(u8),
    /// The lit build's Name: starts typing its name, or keeps the name typed; a dev script's
    /// press types a name (the keyboard types it otherwise).
    NameBuild,
    TypeBuildName(String),
}

/// What a left click on a talent or prestige stat does: the first
/// picks it, a click on the one already picked learns a rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickDoes {
    Pick,
    Learn,
}

/// What a left click on `clicked` does with `picked` already picked.
pub fn left_click(picked: Option<&str>, clicked: &str) -> ClickDoes {
    if picked == Some(clicked) {
        ClickDoes::Learn
    } else {
        ClickDoes::Pick
    }
}

/// What a right click on a piece of the menus asks: a talent or prestige stat gives a rank back;
/// a saved build is emptied; anything else, nothing.
pub fn right_click(on: &MenuAction) -> Option<MenuAction> {
    match on {
        MenuAction::Talent(id) => Some(MenuAction::TalentBack(id.clone())),
        MenuAction::Prestige(id) => Some(MenuAction::PrestigeBack(id.clone())),
        MenuAction::UseBuild(slot) => Some(MenuAction::ClearBuild(*slot)),
        _ => None,
    }
}

/// A piece of the menus was clicked, or a dev script pressed it.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct MenuPress(pub MenuAction);

/// The menus' state.
#[derive(Resource, Default)]
pub struct Menus {
    pub screen: Screen,
    /// A line the menus say (a refusal), until the next action.
    pub notice: Option<String>,
    /// The hero the delete question is about, while it is asked.
    pub confirm_delete: Option<u64>,
    /// The run options' panel is open.
    pub run_options_open: bool,
    /// The last piece of the collection log clicked, and when (real seconds): a second click on
    /// it soon after is a double click.
    last_piece: Option<(MenuAction, f64)>,
    /// The codex open: the list and page shown.
    pub codex: Option<(CodexList, usize)>,
    /// The guide open: the tab shown.
    pub guide: Option<guide::GuideTab>,
    /// The Achievements window open: the page shown.
    pub achievements: Option<AchievementsPage>,
    /// The Shop: the tile picked (its shop item) and whether Buy's question is asked.
    pub shop_pick: Option<String>,
    pub shop_confirm: bool,
    /// The Shop's tab shown.
    pub shop_tab: companions::ShopTab,
    /// Decorate camp open over the warband screen: the spot picked and whether opening it for
    /// points is asked.
    pub decorate: Option<DecorateState>,
    /// The options panel open, with what it is changing.
    pub game_options: Option<options::OptionsDraft>,
    /// Over a paused run: what the options panel's Okay left for later, which the run's Escape
    /// menu says under its buttons until the run goes on.
    pub run_notice: Option<String>,
    /// Where in its camp a hero was deleted, so the camp then picks the next one.
    deleted_at: Option<(Faction, usize)>,
    /// A run is being entered: nothing more is asked meanwhile.
    entering: bool,
    /// The window open over the camp, if any ([`over_the_camp`]).
    pub over_the_camp: Option<over_the_camp::CampWindow>,
}

/// Decorate camp's state ([`decorate`]): the spot picked (its id), and whether opening it for
/// points is asked.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DecorateState {
    pub spot: Option<String>,
    pub confirm: bool,
}

impl Menus {
    /// Whether the hero is on the way into the world rather than to a screen of the menus: a run
    /// or the camp being entered, or with the hub on the warband screen, which gives way to the
    /// camp ([`enter_the_camp`]). The title keeps its art over the way in ([`crate::title_screen`]).
    pub(crate) fn on_the_way_in(&self) -> bool {
        self.entering || (self.screen == Screen::Warband && crate::camp::hub_on())
    }

    /// Whether the options panel is open over a paused run (with the key binding window it
    /// opened, if any): the run's Escape menu hides under it, and Escape closes it.
    pub fn options_over_the_run(&self) -> bool {
        self.game_options.as_ref().is_some_and(|d| d.over_run)
    }
}

/// What the menus were last drawn for; a change draws them anew.
#[derive(Resource, Default)]
pub struct MenuView {
    root: Option<Entity>,
    drawn_for: Option<u64>,
}

/// The art sheets the menus cut cells from, loaded once.
#[derive(Resource)]
pub struct MenuArt {
    pub class_icons: Handle<Image>,
    pub race_icons: Handle<Image>,
}

pub struct MenusPlugin;

impl Plugin for MenusPlugin {
    fn build(&self, app: &mut App) {
        // the look's buttons light as the pointer finds them, wherever they are (the run's Escape
        // menu wears the look too), and the news glows beat
        app.add_systems(Update, (look::light_buttons, look::pulse_news_glows).chain().after(StockSystems));
        // the run's Escape menu writes presses too (its Options, Escape over the panel)
        app.add_message::<MenuPress>();
        if !menus_on() {
            return;
        }
        info!("menus: the menus before a run replace the stock select screen");
        app.add_systems(Startup, sheet::talents::draw_the_arrow);
        let assets = app.world().resource::<AssetServer>().clone();
        app.insert_resource(MenuArt {
            class_icons: crate::stock::load_stock_art(&assets, art::CLASS_ICONS),
            race_icons: crate::stock::load_stock_art(&assets, art::RACE_ICONS),
        })
        .init_resource::<Menus>()
        .init_resource::<MenuView>()
        .init_resource::<PreRunFacts>()
        .init_resource::<preview::ModelYaw>()
        .add_systems(
            Update,
            (
                replace_the_stock_screen,
                (clicks_to_presses, right_clicks_to_presses),
                // the launch rule waits until the title screen is passed
                choose_the_first_screen.run_if(crate::title_screen::passed),
                enter_the_camp,
                over_the_camp::f1_opens_the_menu,
                over_the_camp::open_camp_windows,
                (escape_goes_back, sheet::type_the_name, sheet::talents::type_the_build_name),
                act,
                over_the_run::follow_the_run,
                take_answers,
                learn_what_is_shown,
                draw,
            )
                .chain()
                .before(StockSystems),
        )
        .add_systems(
            Update,
            (
                warband::place_name_over_head,
                preview::show_the_hero,
                preview::drag_to_turn,
                companions::drive_creature_model,
                decorate::place_spot_markers,
            )
                .after(StockSystems),
        );
        if !app.world().contains_resource::<ItemIcons>() {
            app.insert_resource(ItemIcons::load());
        }
        if !app.world().contains_resource::<RacialIcons>() {
            app.insert_resource(RacialIcons::load());
        }
        dev::register(app);
        outline::register(app);
        scroll::register(app);
    }
}

/// The stock select screen gives way to the menus while they are on.
fn replace_the_stock_screen(mut replaced: ResMut<SelectScreenReplaced>) {
    if !replaced.0 {
        replaced.0 = true;
    }
}

/// A right click on a menu button under the pointer (a talent, a prestige stat) asks what
/// [`right_click`] says; the stock buttons answer the left button only.
fn right_clicks_to_presses(
    mouse: Res<ButtonInput<MouseButton>>,
    buttons: Query<(&Interaction, &MenuAction)>,
    mut presses: MessageWriter<MenuPress>,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    for (interaction, action) in &buttons {
        if *interaction == Interaction::Hovered {
            if let Some(a) = right_click(action) {
                presses.write(MenuPress(a));
            }
        }
    }
}

/// A click on a menu button is its action.
fn clicks_to_presses(
    mut clicks: MessageReader<StockClick>,
    actions: Query<&MenuAction>,
    mut presses: MessageWriter<MenuPress>,
) {
    for StockClick(e) in clicks.read() {
        if let Ok(a) = actions.get(*e) {
            presses.write(MenuPress(a.clone()));
        }
    }
}

/// The first class of the content, the class picker's first pick.
fn first_class(content: &Content) -> String {
    content
        .classes
        .first()
        .map(|c| c.id.clone())
        .unwrap_or_default()
}

/// The launch rule: once the select screen is up, a save with a hero
/// opens on the warband screen, one with none on the class picker.
fn choose_the_first_screen(
    screen: Res<SelectScreen>,
    content: Option<Res<GameContent>>,
    mut menus: ResMut<Menus>,
    mut facts: ResMut<PreRunFacts>,
    mut visit: ResMut<crate::camp::CampVisit>,
) {
    if !screen.open {
        // walking the camp, the menus keep what its windows show
        if (menus.screen != Screen::Unset || menus.entering) && !visit.in_camp {
            // off to a run, or back to the login: everything starts over on the way back
            *menus = Menus::default();
            facts.forget();
        }
        return;
    }
    let Some(content) = content else {
        return;
    };
    if menus.screen != Screen::Unset {
        return;
    }
    // left the camp to recruit a new hero (`over_the_camp`): the class picker
    let recruit = std::mem::take(&mut visit.recruit);
    menus.screen = if dev::sample_wanted() {
        Screen::Sample
    } else if screen.characters.is_empty() || recruit {
        Screen::ClassPicker {
            class: first_class(&content.0),
        }
    } else {
        Screen::Warband
    };
}

/// With the hub on ([`crate::camp`]), the warband screen gives way to the camp: as soon as it is
/// up with the roster in, the hero taken over in the camp (or sent off to its run from it), else
/// the picked hero, goes into the world, when its faction has a camp. A Horde hero stays at the
/// warband screen while only the Alliance has one.
fn enter_the_camp(
    screen: Res<SelectScreen>,
    content: Option<Res<GameContent>>,
    mut menus: ResMut<Menus>,
    mut visit: ResMut<crate::camp::CampVisit>,
    mut picker: ResMut<PickerState>,
    mut enter: MessageWriter<SelectEnterWorld>,
) {
    if !crate::camp::hub_on() || !screen.open || menus.screen != Screen::Warband || menus.entering {
        return;
    }
    let Some(content) = content else {
        return;
    };
    let next = visit.enter_next.take();
    let hero = next
        .and_then(|g| screen.characters.iter().find(|c| c.guid == g))
        .or_else(|| screen.selected_character());
    let Some(hero) = hero else {
        return;
    };
    let faction = crate::picker::faction_of(&content.0, hero);
    let has_camp = faction.is_some_and(crate::camp::has_camp);
    // a hero of the other faction going in: the warband's camp shown is now its (the list, and
    // what Decorate camp decorates)
    if let Some(f) = faction.filter(|f| *f != picker.faction) {
        picker.faction = f;
    }
    // off to a run is always entered; the camp only for a faction that has one
    if next.is_some() || has_camp {
        info!("camp: {} goes into the world", hero.name);
        visit.roster = screen.characters.clone();
        menus.entering = true;
        enter.write(SelectEnterWorld(hero.guid));
    }
}

/// The ways the menus reach the server: benilla's select requests, and the asks before a run.
#[derive(SystemParam)]
pub(crate) struct Asks<'w> {
    pick: MessageWriter<'w, SelectCharacter>,
    enter: MessageWriter<'w, SelectEnterWorld>,
    /// The teleport a run starts with ([`crate::run_start_sound`]).
    teleport: MessageWriter<'w, benilla_app::SoundThroughLoading>,
    create: MessageWriter<'w, SelectCreate>,
    delete: MessageWriter<'w, SelectDelete>,
    refresh: MessageWriter<'w, SelectRefresh>,
    link: Option<Res<'w, PreRunLink>>,
    facts: ResMut<'w, PreRunFacts>,
}

impl Asks<'_> {
    /// Asks the save before a run; the verbs refused, said in our words.
    fn ask(&mut self, ask: &PreRunAsk) -> Option<String> {
        let link = self.link.as_deref()?;
        let refused = self.facts.ask(link, ask);
        refused.first().map(|v| refusal(v).to_owned())
    }
}

/// What the menus say when the save refuses a change, by its verb.
fn refusal(verb: &str) -> &'static str {
    crate::hero_sheet::progress::refusal_text(verb).unwrap_or(match verb {
        "zone" => "That zone is not open to the class yet.",
        "loadout" => "That weapon loadout is not the class's.",
        "rename" => "That name cannot be taken.",
        "track" => "At most three achievements can be tracked, and only ones not yet earned.",
        "reshape" => "That race cannot be this class, or its camp is full.",
        "temper" => "Those temperaments cannot be kept.",
        "period" => "That time period cannot be kept.",
        "build" => "That build slot holds no build.",
        _ => "That cannot be done now.",
    })
}

/// The hero sheet's hero (its guid; 0 for one not yet made) and class, when the sheet shows.
fn sheet_hero(screen: &Screen) -> Option<(u64, String)> {
    let Screen::Sheet(s) = screen else {
        return None;
    };
    let guid = match s.mode {
        SheetMode::Modify(g) => g,
        SheetMode::Create => 0,
    };
    Some((guid, s.draft.class.clone()))
}

/// Closes the build bar's drop-down, if the sheet shows one, and drops a build name being typed.
fn close_build_drop(screen: &mut Screen) {
    if let Screen::Sheet(s) = screen {
        s.build_drop = None;
        s.build_name = None;
    }
}

/// The hero sheet on hero `c`, at `tab`.
fn hero_sheet(content: &Content, facts: &PreRunFacts, c: &benilla_protocol::Character, tab: SheetTab) -> Option<Sheet> {
    let class = class_of(content, c.class)?;
    Some(Sheet {
        mode: SheetMode::Modify(c.guid),
        tab,
        draft: Draft {
            class: class.id.clone(),
            race: c.race,
            sex: c.gender.min(1),
            look: [c.skin, c.face, c.hair_style, c.hair_color, c.facial_hair],
            name: c.name.clone(),
            temperament: facts.hero(c.guid).map(|h| h.temperament.clone()).unwrap_or_default(),
            time_period: facts.hero(c.guid).map(|h| h.time_period).unwrap_or_default(),
        },
        talent: None,
        prestige: None,
        piece: None,
        slot: None,
        companions: false,
        companion: None,
        wear: None,
        wear_pick: None,
        sample: 0,
        talents_page: TalentsPage::Talents,
        build_drop: None,
        build_name: None,
    })
}

/// The view a sheet's tab is, among those whose news the warband screen's buttons show.
pub(crate) fn seen_view(tab: SheetTab) -> Option<esurv_protocol::SeenView> {
    match tab {
        SheetTab::Character => None,
        SheetTab::Collection => Some(esurv_protocol::SeenView::Collection),
        SheetTab::Talents => Some(esurv_protocol::SeenView::Talents),
    }
}

/// The sheet shows its class's Collection or Talents: what is there is seen,
/// so its button on the warband screen stops glowing.
fn see(asks: &mut Asks, s: &Sheet) {
    let Some(view) = seen_view(s.tab) else {
        return;
    };
    let has_news = asks.facts.progress(&s.draft.class).is_none_or(|p| match view {
        esurv_protocol::SeenView::Collection => p.new_gear,
        esurv_protocol::SeenView::Talents => p.new_points,
        esurv_protocol::SeenView::Challenge => p.challenge_news,
    });
    if has_news {
        asks.ask(&PreRunAsk::Seen {
            class: s.draft.class.clone(),
            view,
        });
    }
}

/// How soon a second click on the same piece of the collection log is a double click, seconds.
const DOUBLE_CLICK: f64 = 0.45;

/// What a press does.
#[allow(clippy::too_many_arguments)] // one Bevy system's inputs
fn act(
    mut presses: MessageReader<MenuPress>,
    mut menus: ResMut<Menus>,
    screen: Res<SelectScreen>,
    content: Option<Res<GameContent>>,
    choices: Res<CreateChoices>,
    mut camp: ResMut<PickerState>,
    mut asks: Asks,
    (mut yaw, mut quit, real): (
        ResMut<preview::ModelYaw>,
        MessageWriter<AppExit>,
        Res<Time<Real>>,
    ),
    (mut visit, mut requests): (ResMut<crate::camp::CampVisit>, MessageWriter<crate::protocol::Request>),
    script: Option<NonSend<benilla_ui::script::UiScript>>,
) {
    let Some(content) = content else {
        presses.clear();
        return;
    };
    let content = &content.0;
    for MenuPress(action) in presses.read() {
        if menus.entering {
            continue;
        }
        menus.notice = None;
        // the hero picked in the camp shown; walking the camp, the hero walking it
        let picked = if visit.in_camp {
            over_the_camp::camp_hero(&visit).cloned()
        } else {
            screen
                .selected_character()
                .filter(|c| crate::picker::faction_of(content, c) == Some(camp.faction))
                .cloned()
        };
        match action {
            MenuAction::Nothing => {}
            MenuAction::SwitchCamp => {
                crate::picker::switch_camp(&mut camp, &screen, Some(content), &mut asks.pick);
            }
            MenuAction::ModifyHero | MenuAction::OpenHeroTab(_) => {
                let tab = match action {
                    MenuAction::OpenHeroTab(t) => *t,
                    _ => SheetTab::Character,
                };
                // the hero's facts told afresh (one not yet given its temperaments is given them)
                if let Some(c) = picked.as_ref() {
                    asks.ask(&PreRunAsk::Hero(c.guid));
                }
                if let Some(s) = picked.as_ref().and_then(|c| hero_sheet(content, &asks.facts, c, tab)) {
                    see(&mut asks, &s);
                    menus.screen = Screen::Sheet(Box::new(s));
                }
            }
            MenuAction::DeleteHero => menus.confirm_delete = picked.as_ref().map(|c| c.guid),
            MenuAction::ConfirmNo => menus.confirm_delete = None,
            MenuAction::ConfirmYes => {
                if let Some(guid) = menus.confirm_delete.take() {
                    let order = crate::picker::camp_heroes(&screen, content, camp.faction);
                    let at = order.iter().position(|g| *g == guid).unwrap_or(0);
                    menus.deleted_at = Some((camp.faction, at));
                    asks.delete.write(SelectDelete(guid));
                }
            }
            MenuAction::PickZone(zone) => {
                if let Some(c) = &picked {
                    menus.notice = asks.ask(&PreRunAsk::Zone {
                        hero: c.guid,
                        zone: zone.clone(),
                    });
                    if menus.notice.is_none() {
                        // the hero's row stands in its zone now: the loading screen is the zone's
                        asks.refresh.write(SelectRefresh);
                    }
                }
            }
            MenuAction::PickStart(option) => {
                // the sheet's hero (the Talents window's build bar), else the hero picked
                let hero = sheet_hero(&menus.screen).map(|(g, _)| g).or(picked.as_ref().map(|c| c.guid));
                close_build_drop(&mut menus.screen);
                if let Some(hero) = hero.filter(|&g| g != 0) {
                    menus.notice = asks.ask(&PreRunAsk::Start {
                        hero,
                        option: option.clone(),
                    });
                }
            }
            MenuAction::PickLoadout(id) => {
                let class = sheet_hero(&menus.screen)
                    .map(|(_, class)| class)
                    .or(picked.as_ref().and_then(|c| class_of(content, c.class)).map(|k| k.id.clone()));
                close_build_drop(&mut menus.screen);
                if let Some(ask) = class.and_then(|k| PreRunAsk::armory(&k, &ToServer::ChooseLoadout(id.clone()))) {
                    menus.notice = asks.ask(&ask);
                    if menus.notice.is_none() {
                        // the camp's figure takes up the new weapons: the character list (its
                        // equipment the loadout's) is read anew
                        asks.refresh.write(SelectRefresh);
                    }
                }
            }
            MenuAction::OpenCodex => menus.codex = Some((CodexList::Rings, 0)),
            MenuAction::CodexList(list) => {
                let class = picked
                    .as_ref()
                    .and_then(|c| class_of(content, c.class))
                    .map(|c| c.name.clone());
                let page = if *list == CodexList::Legendary {
                    crate::hero_sheet::codex::class_page(content, class.as_deref())
                } else {
                    0
                };
                menus.codex = Some((*list, page));
            }
            MenuAction::CodexPage(next) => {
                if let Some((list, page)) = menus.codex {
                    let pages = crate::hero_sheet::codex::pages(content, list);
                    let page = if *next {
                        (page + 1).min(pages - 1)
                    } else {
                        page.saturating_sub(1)
                    };
                    menus.codex = Some((list, page));
                }
            }
            MenuAction::CloseCodex => menus.codex = None,
            MenuAction::OpenAchievements => {
                menus.achievements = Some(AchievementsPage::Summary);
                if asks.facts.achievements_news() {
                    asks.ask(&PreRunAsk::AchievementsSeen);
                }
            }
            MenuAction::OpenShop => {
                menus.achievements = Some(AchievementsPage::Shop);
                menus.shop_confirm = false;
            }
            MenuAction::AchievementsPage(page) => {
                if menus.achievements.is_some() {
                    menus.achievements = Some(page.clone());
                    // the window opened on the Shop, then turned to the achievements: what was
                    // earned is seen now, and the Achievements button stops glowing
                    if *page != AchievementsPage::Shop && asks.facts.achievements_news() {
                        asks.ask(&PreRunAsk::AchievementsSeen);
                    }
                }
            }
            MenuAction::TrackAchievement { id, on } => {
                menus.notice = asks.ask(&PreRunAsk::Track { id: id.clone(), on: *on });
            }
            MenuAction::CloseAchievements => {
                menus.achievements = None;
                menus.shop_confirm = false;
            }
            MenuAction::ShopTab(tab) => {
                menus.shop_tab = *tab;
                menus.shop_pick = None;
                menus.shop_confirm = false;
            }
            MenuAction::ShopPick(item) => {
                menus.shop_pick = Some(item.clone());
                menus.shop_confirm = false;
            }
            MenuAction::ShopBuy => {
                let told = asks.facts.achievements().cloned().unwrap_or_default();
                menus.shop_confirm = menus
                    .shop_pick
                    .as_deref()
                    .and_then(|i| crate::achievements::shop::tile(content, asks.facts.zones_discovered(), &told, i))
                    .is_some_and(|t| t.buyable());
            }
            MenuAction::ShopAnswer(yes) => {
                if let (true, true, Some(item)) = (*yes, menus.shop_confirm, menus.shop_pick.clone()) {
                    menus.notice = asks.ask(&PreRunAsk::Buy { item });
                }
                menus.shop_confirm = false;
            }
            MenuAction::OpenCompanions | MenuAction::CompanionPick(_) => {
                if let Screen::Sheet(s) = &mut menus.screen {
                    let current = match s.mode {
                        SheetMode::Modify(g) => asks.facts.hero(g).map(|h| h.companion.clone()).filter(|c| !c.is_empty()),
                        SheetMode::Create => None,
                    };
                    s.companions = true;
                    s.slot = None;
                    s.piece = None;
                    s.companion = match action {
                        MenuAction::CompanionPick(id) => Some(id.clone()),
                        _ => current,
                    };
                }
            }
            MenuAction::ChooseCompanion(id) => {
                if let Screen::Sheet(s) = &menus.screen {
                    if let SheetMode::Modify(hero) = s.mode {
                        menus.notice = asks.ask(&PreRunAsk::Companion { hero, companion: id.clone() });
                    }
                }
            }
            MenuAction::OpenWear(kind) | MenuAction::WearPick(kind, _) => {
                if let Screen::Sheet(s) = &mut menus.screen {
                    let current = match s.mode {
                        SheetMode::Modify(g) => asks.facts.hero(g).map(|h| wear_of(h, *kind).to_owned()).filter(|c| !c.is_empty()),
                        SheetMode::Create => None,
                    };
                    s.wear = Some(*kind);
                    s.companions = false;
                    s.slot = None;
                    s.piece = None;
                    s.wear_pick = match action {
                        MenuAction::WearPick(_, id) => Some(id.clone()),
                        _ => current,
                    };
                }
            }
            MenuAction::ChooseWear(kind, id) => {
                if let Screen::Sheet(s) = &menus.screen {
                    if let SheetMode::Modify(hero) = s.mode {
                        menus.notice = asks.ask(&PreRunAsk::Dress { hero, kind: *kind, id: id.clone() });
                    }
                }
            }
            MenuAction::OpenDecorate => {
                menus.decorate = Some(DecorateState::default());
                menus.achievements = None;
                menus.codex = None;
                menus.guide = None;
                menus.run_options_open = false;
            }
            MenuAction::DecorateSpot(spot) => {
                if let Some(d) = &mut menus.decorate {
                    d.spot = Some(spot.clone());
                    d.confirm = false;
                }
            }
            MenuAction::DecoratePiece(piece) => {
                if let Some(spot) = menus.decorate.as_ref().and_then(|d| d.spot.clone()) {
                    let camp = crate::achievements::cosmetics::camp_word(camp.faction).to_owned();
                    menus.notice = asks.ask(&PreRunAsk::Decorate { camp, spot, piece: piece.clone() });
                }
            }
            MenuAction::DecorateBuySpot => {
                if let Some(d) = &mut menus.decorate {
                    d.confirm = d.spot.is_some();
                }
            }
            MenuAction::DecorateAnswer(yes) => {
                if let Some(d) = menus.decorate.clone() {
                    if let (true, true, Some(spot)) = (*yes, d.confirm, d.spot) {
                        let item = esurv_content::camp_spot_item(&spot);
                        menus.notice = asks.ask(&PreRunAsk::Buy { item });
                    }
                }
                if let Some(d) = &mut menus.decorate {
                    d.confirm = false;
                }
            }
            MenuAction::CloseDecorate => menus.decorate = None,
            MenuAction::OpenGuide => menus.guide = Some(guide::GuideTab::default()),
            MenuAction::GuideTab(tab) => menus.guide = Some(*tab),
            MenuAction::CloseGuide => menus.guide = None,
            MenuAction::OpenGameOptions => {
                // with the select screen gone, the hero is in a run: the panel opens over it,
                // from its Escape menu
                let over_run = !screen.open;
                menus.run_notice = None;
                menus.game_options = script.as_deref().map(|s| {
                    let mut d = options::read(s, options::OptionsTab::Video);
                    d.over_run = over_run;
                    d.saves.in_run = over_run;
                    d
                });
            }
            MenuAction::KeyBindings => {
                if let (Some(d), Some(s)) = (&mut menus.game_options, script.as_deref()) {
                    if d.over_run {
                        d.key_bindings = over_the_run::open_key_bindings(s);
                    }
                }
            }
            MenuAction::OptionsTab(tab) => {
                if let Some(d) = &mut menus.game_options {
                    d.tab = *tab;
                    if *tab == options::OptionsTab::Saves {
                        save_files::ask(&mut d.saves, asks.link.as_deref(), &PreRunAsk::SaveFiles);
                    }
                }
            }
            MenuAction::ExportSave => {
                if let Some(d) = &mut menus.game_options {
                    save_files::ask(&mut d.saves, asks.link.as_deref(), &PreRunAsk::ExportSave);
                }
            }
            MenuAction::PickSaveFile(name) => {
                if let Some(d) = &mut menus.game_options {
                    d.saves.pick(name);
                }
            }
            MenuAction::ImportSave => {
                if let Some(d) = &mut menus.game_options {
                    d.saves.confirm = d.saves.importable().is_some();
                }
            }
            MenuAction::ImportAnswer(yes) => {
                let mut imported = false;
                if let Some(d) = &mut menus.game_options {
                    let file = d.saves.importable().map(|f| f.name.clone());
                    d.saves.confirm = false;
                    if let (true, Some(file)) = (*yes, file) {
                        let ask = PreRunAsk::ImportSave { file };
                        imported = save_files::ask(&mut d.saves, asks.link.as_deref(), &ask);
                    }
                }
                if imported {
                    // a save of other heroes: everything known of the old one goes, and the
                    // character list is read anew
                    info!("menus: a save was imported");
                    asks.facts.forget();
                    asks.refresh.write(SelectRefresh);
                }
            }
            MenuAction::OptionsChange { key, up } => {
                if let Some(d) = &mut menus.game_options {
                    options::change(script.as_deref(), d, key, *up);
                }
            }
            MenuAction::OptionsOkay => {
                if let (Some(mut d), Some(s)) = (menus.game_options.take(), script.as_deref()) {
                    options::apply(s, &mut d);
                    if d.over_run {
                        // over a run, the Escape menu says what waits for later
                        menus.run_notice = d.notice;
                    } else {
                        menus.notice = d.notice;
                    }
                }
            }
            MenuAction::OptionsCancel => {
                if let Some(d) = menus.game_options.take() {
                    options::cancel(script.as_deref(), &d);
                }
            }
            MenuAction::QuitGame => {
                info!("menus: Quit game");
                quit.write(AppExit::Success);
            }
            // the updater's own systems answer it
            MenuAction::UpdateGame => {}
            MenuAction::OpenRunOptions => menus.run_options_open = true,
            MenuAction::CloseRunOptions => menus.run_options_open = false,
            MenuAction::OptionStep { slider, up } => {
                if let Some((o, _)) = asks.facts.run_options() {
                    let o = warband::options_stepped(content, o, *slider, *up);
                    menus.notice = asks.ask(&PreRunAsk::RunOptions(o));
                }
            }
            MenuAction::PickMode { challenge } => {
                if let Some((mut o, _)) = asks.facts.run_options() {
                    o.challenge = *challenge;
                    menus.notice = asks.ask(&PreRunAsk::RunOptions(o));
                }
                // back in Normal Mode, a zone picked in Challenge Mode that is locked there gives
                // way to the hardest zone the class has open
                let back = picked.as_ref().filter(|_| !*challenge).and_then(|c| {
                    Some((c.guid, warband::normal_mode_zone(content, &asks.facts, c)?))
                });
                if let Some((hero, zone)) = back {
                    menus.notice = asks.ask(&PreRunAsk::Zone { hero, zone });
                    asks.refresh.write(SelectRefresh);
                }
            }
            MenuAction::CloseChallengeNews => {
                let class = picked.as_ref().and_then(|c| class_of(content, c.class)).map(|c| c.id.clone());
                if let Some(class) = class {
                    menus.notice = asks.ask(&PreRunAsk::Seen {
                        class,
                        view: esurv_protocol::SeenView::Challenge,
                    });
                }
            }
            MenuAction::ChallengeStep { up } => {
                if let Some((mut o, _)) = asks.facts.run_options() {
                    o.challenge_pct = esurv_sim::game::challenge::slider_stepped(o.challenge_pct, *up);
                    menus.notice = asks.ask(&PreRunAsk::RunOptions(o));
                }
            }
            MenuAction::Tab(_)
            | MenuAction::Race(_)
            | MenuAction::Sex(_)
            | MenuAction::Feature { .. }
            | MenuAction::Randomize
            | MenuAction::Temperament(_)
            | MenuAction::Quiet
            | MenuAction::RandomTemperament
            | MenuAction::TimePeriod(_)
            | MenuAction::SampleLine
            | MenuAction::Slot(_)
            | MenuAction::TypeName(_) => {
                if let Screen::Sheet(s) = &mut menus.screen {
                    shape(s, action, content, &choices);
                    if matches!(action, MenuAction::Tab(_)) {
                        let s = (**s).clone();
                        see(&mut asks, &s);
                    }
                }
            }
            MenuAction::Piece { slot, zone, look } => {
                // a second click on the same piece soon after (a double
                // click) wears it, as its Wear or Show This Look would
                let now = real.elapsed_secs_f64();
                let double = menus
                    .last_piece
                    .take()
                    .is_some_and(|(a, at)| a == *action && now - at <= DOUBLE_CLICK);
                if !double {
                    menus.last_piece = Some((action.clone(), now));
                }
                let Screen::Sheet(s) = &mut menus.screen else {
                    continue;
                };
                shape(s, action, content, &choices);
                if !double {
                    continue;
                }
                let s = (**s).clone();
                let wears = crate::hero_sheet::gear::ClassGear::of(content, &s.draft.class)
                    .zip(asks.facts.progress(&s.draft.class))
                    .map(|(gear, p)| sheet::collection::double_click(&gear, p, *slot, *zone, *look))
                    .unwrap_or_default();
                for a in wears {
                    menus.notice = armory(&mut asks, content, &s, &a);
                    if menus.notice.is_some() {
                        break;
                    }
                }
            }
            MenuAction::Talent(_)
            | MenuAction::TalentBack(_)
            | MenuAction::Prestige(_)
            | MenuAction::PrestigeBack(_) => {
                let ask = match &mut menus.screen {
                    Screen::Sheet(s) => click_on_a_rank(s, action).map(|a| ((**s).clone(), a)),
                    _ => None,
                };
                if let Some((s, a)) = ask {
                    menus.notice = armory(&mut asks, content, &s, &a);
                }
            }
            MenuAction::Wear { .. }
            | MenuAction::TakeOff(_)
            | MenuAction::ShowLook { .. }
            | MenuAction::Learn
            | MenuAction::Unlearn
            | MenuAction::ResetTalents => {
                if let Screen::Sheet(s) = &menus.screen {
                    let s = s.clone();
                    menus.notice = armory(&mut asks, content, &s, action);
                }
            }
            MenuAction::TalentsPage(page) => {
                if let Screen::Sheet(s) = &mut menus.screen {
                    s.talents_page = *page;
                    s.build_drop = None;
                    s.build_name = None;
                }
            }
            MenuAction::BuildDrop(drop) => {
                if let Screen::Sheet(s) = &mut menus.screen {
                    // a second click on the open one closes it
                    s.build_drop = if s.build_drop == *drop { None } else { *drop };
                }
            }
            MenuAction::UseBuild(_) | MenuAction::SaveBuild | MenuAction::ClearBuild(_) => {
                if let Some((hero, class)) = sheet_hero(&menus.screen) {
                    close_build_drop(&mut menus.screen);
                    let active = asks.facts.progress(&class).map_or(0, |p| p.active_build);
                    let (slot, build) = match action {
                        MenuAction::UseBuild(n) => (*n, BuildAction::Use),
                        MenuAction::ClearBuild(n) => (*n, BuildAction::Clear),
                        _ => (active, BuildAction::Save),
                    };
                    let uses = build == BuildAction::Use;
                    menus.notice = asks.ask(&PreRunAsk::Build { hero, class, slot, action: build });
                    if uses && menus.notice.is_none() {
                        // the talents picked may be gone; the camp's figure takes up the
                        // build's weapons
                        if let Screen::Sheet(s) = &mut menus.screen {
                            s.talent = None;
                            s.prestige = None;
                        }
                        asks.refresh.write(SelectRefresh);
                    }
                }
            }
            MenuAction::NameBuild => {
                let Some((hero, class)) = sheet_hero(&menus.screen) else {
                    continue;
                };
                let progress = asks.facts.progress(&class);
                let slot = progress.map_or(0, |p| p.active_build);
                let held = progress.and_then(|p| p.builds.iter().find(|b| b.slot == slot)).map(|b| b.name.clone());
                let Screen::Sheet(s) = &mut menus.screen else {
                    continue;
                };
                s.build_drop = None;
                match s.build_name.take() {
                    Some(name) => {
                        menus.notice = asks.ask(&PreRunAsk::Build {
                            hero,
                            class,
                            slot,
                            action: BuildAction::Name(name),
                        });
                    }
                    // an empty build has no name to type
                    None => s.build_name = held,
                }
            }
            MenuAction::TypeBuildName(name) => {
                if let Screen::Sheet(s) = &mut menus.screen {
                    s.build_name = Some(sheet::talents::build_name_typed("", name));
                }
            }
            MenuAction::Turn(right) => {
                yaw.0 += if *right {
                    preview::TURN_STEP
                } else {
                    -preview::TURN_STEP
                };
            }
            MenuAction::ConfirmSheet => {
                let Screen::Sheet(s) = &menus.screen else {
                    continue;
                };
                let s = s.clone();
                // not while the Orb's changes are shut (the button is shut; Enter too)
                if sheet::appearance_shut(visit.in_camp, visit.recruited, s.mode) {
                    continue;
                }
                if !sheet::name_ready(&s.draft.name) {
                    menus.notice = Some("A hero's name is two to twelve letters.".into());
                    continue;
                }
                match s.mode {
                    SheetMode::Create => {
                        let d = &s.draft;
                        let class = content.class(&d.class).map_or(0, |c| c.wow_class_id as u8);
                        // the temperaments first, kept for the hero the create makes
                        asks.ask(&PreRunAsk::Temperament {
                            hero: 0,
                            temperament: d.temperament.clone(),
                        });
                        asks.ask(&PreRunAsk::TimePeriod {
                            hero: 0,
                            time_period: d.time_period,
                        });
                        asks.create
                            .write(SelectCreate(benilla_protocol::CharCreateReq {
                                name: d.name.clone(),
                                race: d.race,
                                class,
                                gender: d.sex,
                                skin: d.look[0],
                                face: d.look[1],
                                hair_style: d.look[2],
                                hair_color: d.look[3],
                                facial_hair: d.look[4],
                            }));
                    }
                    SheetMode::Modify(guid) => {
                        // walking the camp, the roster kept on the way in (the world has none)
                        let roster = if visit.in_camp { &visit.roster } else { &screen.characters };
                        menus.notice = save_changes(&mut asks, roster, guid, &s.draft);
                        if menus.notice.is_none() {
                            asks.refresh.write(SelectRefresh);
                            menus.screen = Screen::Warband;
                            if visit.in_camp {
                                // back into the camp as the hero, wearing its new look
                                info!("camp: {} comes back in with its new look", s.draft.name);
                                visit.enter_next = Some(guid);
                                visit.in_camp = false;
                                requests.write(crate::protocol::Request(ToServer::Leave));
                            }
                        }
                    }
                }
            }
            MenuAction::PickHeroNamed(name) => {
                if let Some(c) = screen
                    .characters
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(name))
                {
                    asks.pick.write(SelectCharacter(c.guid));
                }
            }
            MenuAction::StartRun => {
                if let Some(c) = &picked {
                    info!("menus: {} starts a run", c.name);
                    menus.entering = true;
                    if visit.in_camp {
                        // from the camp: the server sends the hero back in as its run
                        menus.over_the_camp = None;
                        requests.write(crate::protocol::Request(ToServer::CampRun));
                    } else {
                        // from the warband screen: the teleport, on into the loading screen (from the
                        // camp it sounds as the server says to leave it, `crate::run_start_sound`)
                        asks.teleport.write(crate::run_start_sound::RUN_START_SOUND);
                        asks.enter.write(SelectEnterWorld(c.guid));
                    }
                }
            }
            MenuAction::CloseCampWindow => menus.over_the_camp = None,
            MenuAction::PickClass(class) => {
                if let Screen::ClassPicker { class: picked } = &mut menus.screen {
                    *picked = class.clone();
                }
            }
            MenuAction::ChooseClass => {
                if let Screen::ClassPicker { class } = &menus.screen {
                    let draft = new_draft(content, &choices, class);
                    menus.screen = Screen::Sheet(Box::new(Sheet {
                        mode: SheetMode::Create,
                        tab: SheetTab::Character,
                        draft,
                        talent: None,
                        prestige: None,
                        piece: None,
                        slot: None,
                        companions: false,
                        companion: None,
                        wear: None,
                        wear_pick: None,
                        sample: 0,
                        talents_page: TalentsPage::Talents,
                        build_drop: None,
                        build_name: None,
                    }));
                }
            }
            MenuAction::CreateHero => {
                menus.screen = Screen::ClassPicker {
                    class: first_class(content),
                };
            }
            MenuAction::Back => {
                menus.screen = match &menus.screen {
                    Screen::Sheet(s) if s.mode == SheetMode::Create => Screen::ClassPicker {
                        class: s.draft.class.clone(),
                    },
                    Screen::ClassPicker { .. } if screen.characters.is_empty() => {
                        menus.screen.clone()
                    }
                    _ => Screen::Warband,
                };
            }
        }
    }
}

/// A change of the sheet's hero: a tab, a race, a sex, a feature, a random look, a slot picked.
fn shape(s: &mut Sheet, action: &MenuAction, content: &Content, choices: &CreateChoices) {
    match action {
        MenuAction::Tab(t) => s.tab = *t,
        MenuAction::Race(r) => {
            s.draft.race = *r;
            sheet::clamp_look(choices, &mut s.draft);
        }
        MenuAction::Sex(x) => {
            s.draft.sex = (*x).min(1);
            sheet::clamp_look(choices, &mut s.draft);
        }
        MenuAction::Feature { index, up } => {
            sheet::step_feature(choices, &mut s.draft, *index, *up)
        }
        MenuAction::Randomize => {
            sheet::randomize(choices, &mut s.draft, sheet::seed_now());
        }
        MenuAction::Temperament(id) => sheet::pick_temperament(&mut s.draft.temperament, id),
        MenuAction::Quiet => s.draft.temperament.clear(),
        MenuAction::TimePeriod(p) => {
            s.draft.time_period = *p;
            s.sample = 0;
        }
        MenuAction::RandomTemperament => {
            s.draft.temperament = sheet::random_temperaments(content, sheet::seed_now());
            s.sample = 0;
        }
        MenuAction::SampleLine => s.sample += 1,
        MenuAction::Slot(slot) => {
            s.slot = Some(*slot);
            s.piece = None;
            s.companions = false;
            s.wear = None;
        }
        MenuAction::TypeName(n) => s.draft.name = n.chars().take(sheet::NAME_LIMIT).collect(),
        MenuAction::Piece { slot, zone, look } => {
            s.piece = Some((*slot, *zone, *look));
            s.slot = None;
            s.companions = false;
            s.wear = None;
        }
        _ => {}
    }
}

/// A click on a talent or a prestige stat: a left click picks it, or
/// once picked asks for a rank more (`Learn`); a right click picks it and asks for a rank back
/// (`Unlearn`). Picking a talent lets go of the prestige stat, and the other way round.
fn click_on_a_rank(s: &mut Sheet, action: &MenuAction) -> Option<MenuAction> {
    let (id, prestige, back) = match action {
        MenuAction::Talent(id) => (id, false, false),
        MenuAction::TalentBack(id) => (id, false, true),
        MenuAction::Prestige(id) => (id, true, false),
        MenuAction::PrestigeBack(id) => (id, true, true),
        _ => return None,
    };
    let picked = if prestige { &s.prestige } else { &s.talent };
    let does = left_click(picked.as_deref(), id);
    if prestige {
        s.prestige = Some(id.clone());
        s.talent = None;
    } else {
        s.talent = Some(id.clone());
        s.prestige = None;
    }
    if back {
        Some(MenuAction::Unlearn)
    } else if does == ClickDoes::Learn {
        Some(MenuAction::Learn)
    } else {
        None
    }
}

/// A change of what the class has earned, asked of the save: wear, take off, a look, a talent's
/// rank, every rank back; what was refused, said.
fn armory(asks: &mut Asks, content: &Content, s: &Sheet, action: &MenuAction) -> Option<String> {
    let class = s.draft.class.as_str();
    let gear = crate::hero_sheet::gear::ClassGear::of(content, class);
    let change = match action {
        MenuAction::Wear { slot, zone } => ToServer::Wear(gear?.entry(*slot, *zone)),
        MenuAction::TakeOff(slot) => ToServer::TakeOff(*slot),
        MenuAction::ShowLook { slot, look } => ToServer::SetLook {
            slot: *slot,
            look: *look,
        },
        MenuAction::Learn => match &s.prestige {
            Some(id) => ToServer::LearnPrestige(id.clone()),
            None => ToServer::LearnTalent(s.talent.clone()?),
        },
        MenuAction::Unlearn => match &s.prestige {
            Some(id) => ToServer::UnlearnPrestige(id.clone()),
            None => ToServer::UnlearnTalent(s.talent.clone()?),
        },
        MenuAction::ResetTalents => {
            let progress = asks.facts.progress(class)?;
            let ranks = crate::hero_sheet::talents::ranks(progress);
            // a talent reset gives the prestige points back too
            let prestige_spent = progress.prestige.spent > 0;
            for id in sheet::talents::reset_order(content, class, &ranks) {
                let ask = PreRunAsk::armory(class, &ToServer::UnlearnTalent(id))?;
                if let Some(n) = asks.ask(&ask) {
                    return Some(n);
                }
            }
            if !prestige_spent {
                return None;
            }
            ToServer::ResetPrestige
        }
        _ => return None,
    };
    asks.ask(&PreRunAsk::armory(class, &change)?)
}

/// The warband's heroes: walking the camp, the roster kept on the way in (the select screen lists
/// none once in the world), else the select screen's.
fn warband<'a>(visit: &'a crate::camp::CampVisit, screen: &'a SelectScreen) -> &'a [benilla_protocol::Character] {
    if visit.in_camp { &visit.roster } else { &screen.characters }
}

/// A modified hero's name, race, sex and look go to the save; what was refused, said.
fn save_changes(asks: &mut Asks, roster: &[benilla_protocol::Character], guid: u64, d: &Draft) -> Option<String> {
    let row = roster.iter().find(|c| c.guid == guid)?;
    if row.name != d.name {
        if let Some(n) = asks.ask(&PreRunAsk::Rename {
            hero: guid,
            name: d.name.clone(),
        }) {
            return Some(n);
        }
    }
    if asks.facts.hero(guid).is_none_or(|h| h.temperament != d.temperament) {
        if let Some(n) = asks.ask(&PreRunAsk::Temperament {
            hero: guid,
            temperament: d.temperament.clone(),
        }) {
            return Some(n);
        }
    }
    if asks.facts.hero(guid).is_none_or(|h| h.time_period != d.time_period) {
        if let Some(n) = asks.ask(&PreRunAsk::TimePeriod {
            hero: guid,
            time_period: d.time_period,
        }) {
            return Some(n);
        }
    }
    let look = [
        row.skin,
        row.face,
        row.hair_style,
        row.hair_color,
        row.facial_hair,
    ];
    if row.race != d.race || row.gender != d.sex || look != d.look {
        return asks.ask(&PreRunAsk::Reshape {
            hero: guid,
            race: d.race,
            gender: d.sex,
            look: d.look,
        });
    }
    None
}

/// Escape closes the question or the panel up, else goes back a screen; on the warband screen it
/// does nothing. In a run Escape is the game menu's: the run's Escape
/// menu closes the options panel over it (`hud::answer_the_menu`).
fn escape_goes_back(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<SelectScreen>,
    mut menus: ResMut<Menus>,
    mut presses: MessageWriter<MenuPress>,
) {
    // walking the camp, Escape over a window is the game menu's ask (`hud`), which closes it
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    // a build name being typed is dropped first, in the camp too
    if let Screen::Sheet(s) = &mut menus.screen {
        if s.build_name.take().is_some() {
            return;
        }
    }
    if !screen.open {
        return;
    }
    if menus.confirm_delete.take().is_some() {
        return;
    }
    if let Some(d) = menus.game_options.as_mut().filter(|d| d.saves.confirm) {
        d.saves.confirm = false;
        return;
    }
    if menus.game_options.is_some() {
        presses.write(MenuPress(MenuAction::OptionsCancel));
        return;
    }
    if menus.codex.take().is_some()
        || menus.guide.take().is_some()
        || menus.achievements.take().is_some()
        || menus.decorate.take().is_some()
    {
        return;
    }
    if menus.run_options_open {
        menus.run_options_open = false;
        return;
    }
    if matches!(menus.screen, Screen::Sheet(_) | Screen::ClassPicker { .. }) {
        presses.write(MenuPress(MenuAction::Back));
    }
}

/// What the server answered to a create or a delete: a deleted hero's camp picks the next one; a
/// hero made lands on the warband screen, picked; a refusal is said.
fn take_answers(
    mut answers: MessageReader<SelectActionDone>,
    mut menus: ResMut<Menus>,
    screen: Res<SelectScreen>,
    content: Option<Res<GameContent>>,
    mut picks: MessageWriter<SelectCharacter>,
    mut facts: ResMut<PreRunFacts>,
    mut visit: ResMut<crate::camp::CampVisit>,
) {
    for a in answers.read() {
        let ok = match a.action {
            CharAction::Create => a.code == benilla_protocol::messages::CHAR_CREATE_SUCCESS,
            CharAction::Delete => a.code == benilla_protocol::messages::CHAR_DELETE_SUCCESS,
        };
        facts.forget_heroes();
        match (a.action, ok) {
            (CharAction::Delete, true) => {
                if let (Some((faction, at)), Some(content)) = (menus.deleted_at.take(), &content) {
                    let order = crate::picker::camp_heroes(&screen, &content.0, faction);
                    if let Some(g) = order.get(at.min(order.len().saturating_sub(1))) {
                        picks.write(SelectCharacter(*g));
                    }
                }
            }
            (CharAction::Delete, false) => {
                menus.deleted_at = None;
                menus.notice = Some("The hero could not be deleted.".into());
            }
            (CharAction::Create, true) => {
                menus.screen = Screen::Warband;
                // recruited from the camp: the new hero (picked) walks in, not the one who left
                visit.enter_next = None;
            }
            (CharAction::Create, false) => menus.notice = Some(create_refusal(a.code).into()),
        }
    }
}

/// Why a hero was not made, in our words, by the server's result byte.
fn create_refusal(code: u8) -> &'static str {
    // the 1.12 create results the server answers with
    const NAME_IN_USE: u8 = 0x31;
    const DISABLED: u8 = 0x32;
    const SERVER_LIMIT: u8 = 0x34;
    const NO_NAME: u8 = 0x43;
    const TOO_SHORT: u8 = 0x44;
    const TOO_LONG: u8 = 0x45;
    const ONLY_LETTERS: u8 = 0x46;
    match code {
        NAME_IN_USE => "Another hero already has that name.",
        SERVER_LIMIT => "That camp is full: nine heroes wait there already.",
        DISABLED => "That race cannot be this class.",
        NO_NAME => "The hero needs a name.",
        TOO_SHORT => "That name is too short: two letters at least.",
        TOO_LONG => "That name is too long: twelve letters at most.",
        ONLY_LETTERS => "A name takes letters only.",
        _ => "The hero could not be made.",
    }
}

/// A new hero of `class`: the first race the class may be (the Alliance's first), a man, the
/// first look, no name yet.
fn new_draft(content: &Content, choices: &CreateChoices, class: &str) -> Draft {
    let wow_class = content.class(class).map_or(0, |c| c.wow_class_id as u8);
    let race = races_in_order()
        .into_iter()
        .find(|&r| race_allowed(content, choices, r, wow_class))
        .unwrap_or(1);
    Draft {
        class: class.into(),
        race,
        sex: 0,
        look: [0; 5],
        name: String::new(),
        // a hero made starts with two at random, so one made in haste lives anyway (the plan)
        temperament: sheet::random_temperaments(content, sheet::seed_now()),
        time_period: TimePeriod::Present,
    }
}

/// The class of 1.12 id `class`.
pub(crate) fn class_of(content: &Content, class: u8) -> Option<&esurv_content::Class> {
    content
        .classes
        .iter()
        .find(|c| c.wow_class_id == u32::from(class))
}

/// The races as the hero sheet shows them: the Alliance's, then the Horde's (1.12 ids).
pub(crate) fn races_in_order() -> [u8; 8] {
    [1, 3, 4, 7, 2, 5, 6, 8]
}

/// Whether race `race` may be the class of 1.12 id `class`: a stock pair or one the game adds;
/// without the player's data, a pair the game adds or any race the content has.
pub(crate) fn race_allowed(
    content: &Content,
    choices: &CreateChoices,
    race: u8,
    class: u8,
) -> bool {
    if choices.loaded() {
        choices.allows(race, class)
    } else {
        content.race_by_wow_id(race).is_some()
    }
}

/// Asks the save for what the screen shown needs and is not known yet.
fn learn_what_is_shown(
    menus: Res<Menus>,
    screen: Res<SelectScreen>,
    content: Option<Res<GameContent>>,
    link: Option<Res<PreRunLink>>,
    mut facts: ResMut<PreRunFacts>,
    visit: Res<crate::camp::CampVisit>,
) {
    let link = link.as_deref();
    match &menus.screen {
        Screen::ClassPicker { class } => facts.need_class(link, class),
        Screen::Sheet(s) => {
            facts.need_class(link, &s.draft.class);
            // the heroes' records (the hero modified's companion, wear and temperament)
            if let SheetMode::Modify(_) = s.mode {
                for c in warband(&visit, &screen) {
                    facts.need_hero(link, c.guid);
                }
            }
        }
        Screen::Warband => {
            // walking the camp, its windows are about the hero walking it
            let hero = match visit.hero.filter(|_| visit.in_camp) {
                Some(_) => over_the_camp::camp_hero(&visit),
                None => screen.selected_character(),
            };
            let (Some(content), Some(c)) = (content, hero) else {
                return;
            };
            if let Some(class) = class_of(&content.0, c.class) {
                facts.need_class(link, &class.id);
            }
            facts.need_hero(link, c.guid);
            // every hero's companion stands by it at the camp fire
            for c in &screen.characters {
                facts.need_hero(link, c.guid);
            }
        }
        Screen::Unset | Screen::Sample => {}
    }
}

/// What a screen is drawn from.
pub(crate) struct Ctx<'a> {
    pub content: &'a Content,
    pub screen: &'a SelectScreen,
    pub facts: &'a PreRunFacts,
    pub images: &'a Assets<Image>,
    pub art: &'a MenuArt,
    pub choices: &'a CreateChoices,
    /// The camp shown.
    pub camp: Faction,
    pub icons: &'a ItemIcons,
    /// The racial passives' icons.
    pub racials: &'a RacialIcons,
    /// The update notice ([`crate::update`]), when the updater runs.
    pub update: Option<&'a crate::update::UpdateNotice>,
    /// The creature pane's picture (a companion turning), while one is shown.
    pub creature: Option<&'a Handle<Image>>,
    /// The Shop's tile picked and whether Buy's question is asked.
    pub shop_pick: Option<&'a str>,
    pub shop_confirm: bool,
    pub shop_tab: companions::ShopTab,
    /// The hero sheet's race portraits and Save changes are shut ([`sheet::appearance_shut`]).
    pub appearance_shut: bool,
}

/// A hash of what the menus show, to draw them anew when it changes.
fn view_key(menus: &Menus, ctx: &Ctx, scale: StockScale) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}", menus.screen).hash(&mut h);
    menus.notice.hash(&mut h);
    menus.entering.hash(&mut h);
    // content loaded again (dev builds) draws the screens anew
    std::ptr::hash(ctx.content, &mut h);
    ctx.screen.open.hash(&mut h);
    ctx.screen.selected.hash(&mut h);
    for c in &ctx.screen.characters {
        (c.guid, &c.name, c.race, c.gender).hash(&mut h);
    }
    ctx.facts.version().hash(&mut h);
    ctx.choices.loaded().hash(&mut h);
    (ctx.camp == Faction::Horde).hash(&mut h);
    ctx.appearance_shut.hash(&mut h);
    (menus.run_options_open, menus.confirm_delete).hash(&mut h);
    format!("{:?}", menus.codex).hash(&mut h);
    menus.guide.hash(&mut h);
    menus.achievements.hash(&mut h);
    (&menus.shop_pick, menus.shop_confirm, menus.shop_tab).hash(&mut h);
    menus.decorate.hash(&mut h);
    menus.over_the_camp.hash(&mut h);
    ctx.creature.is_some().hash(&mut h);
    format!("{:?}", menus.game_options).hash(&mut h);
    // the notice's phase, not its progress (the progress line changes in place)
    ctx.update.and_then(|u| u.view()).hash(&mut h);
    ctx.images.get(&ctx.art.class_icons).is_some().hash(&mut h);
    ctx.images.get(&ctx.art.race_icons).is_some().hash(&mut h);
    scale.unit.to_bits().hash(&mut h);
    h.finish()
}

/// Draws the screen shown, anew when what it shows changed.
#[allow(clippy::too_many_arguments)] // one Bevy system's inputs
fn draw(
    mut commands: Commands,
    menus: Res<Menus>,
    screen: Res<SelectScreen>,
    content: Option<Res<GameContent>>,
    facts: Res<PreRunFacts>,
    images: Res<Assets<Image>>,
    art: Res<MenuArt>,
    choices: Res<CreateChoices>,
    (camp, icons, model, racials, update, creature): (
        Res<PickerState>,
        Res<ItemIcons>,
        Res<SelectModelImage>,
        Res<RacialIcons>,
        Option<Res<crate::update::UpdateNotice>>,
        Res<benilla_app::CreatureModelImage>,
    ),
    look: MenuLook,
    mut view: ResMut<MenuView>,
    visit: Res<crate::camp::CampVisit>,
) {
    let Some(content) = content else {
        return;
    };
    let ctx = Ctx {
        content: &content.0,
        screen: &screen,
        facts: &facts,
        images: &images,
        art: &art,
        choices: &choices,
        camp: camp.faction,
        icons: &icons,
        racials: &racials,
        update: update.as_deref(),
        creature: creature.0.as_ref(),
        shop_pick: menus.shop_pick.as_deref(),
        shop_confirm: menus.shop_confirm,
        shop_tab: menus.shop_tab,
        appearance_shut: match &menus.screen {
            Screen::Sheet(s) => sheet::appearance_shut(visit.in_camp, visit.recruited, s.mode),
            _ => false,
        },
    };
    let key = view_key(&menus, &ctx, look.scale());
    if view.drawn_for == Some(key) {
        return;
    }
    view.drawn_for = Some(key);
    if let Some(root) = view.root.take() {
        commands.entity(root).despawn();
    }
    // over a paused run, the options panel alone (hidden under the key binding window it opened)
    let over_run = menus
        .game_options
        .as_ref()
        .filter(|d| d.over_run && !d.key_bindings && !screen.open);
    // walking the camp, the window one of its things opened, alone
    let over_camp = menus.over_the_camp.filter(|_| visit.in_camp && !screen.open);
    if over_run.is_none() && over_camp.is_none() && (!screen.open || menus.screen == Screen::Unset) {
        return;
    }
    let root = commands
        .spawn((
            Name::new("menus"),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GlobalZIndex(MENU_LAYER),
            Pickable::IGNORE,
            OverlayWindow,
        ))
        .id();
    view.root = Some(root);
    if let Some(d) = over_run {
        options::draw(&mut commands, root, &look, d);
        return;
    }
    if let Some(window) = over_camp {
        // the hero sheet over the camp, as the warband screen opens it
        if let (over_the_camp::CampWindow::Sheet, Screen::Sheet(s)) = (window, &menus.screen) {
            sheet::draw(&mut commands, root, &look, &ctx, s, menus.notice.as_deref(), model.0.clone());
            return;
        }
        let hero = over_the_camp::camp_hero(&visit);
        over_the_camp::draw(&mut commands, root, &look, &ctx, &menus, window, hero);
        if let Some(n) = &menus.notice {
            dev::draw_notice(&mut commands, root, &look, n);
        }
        return;
    }
    match &menus.screen {
        Screen::Sample => dev::draw_sample(&mut commands, root, &look),
        Screen::ClassPicker { class } => class_picker::draw(
            &mut commands,
            root,
            &look,
            &ctx,
            class,
            menus.notice.as_deref(),
        ),
        // with the hub on, the hero goes straight into the camp (`enter_the_camp`): no warband
        // screen on the way
        Screen::Warband if crate::camp::hub_on() && menus.entering => {}
        Screen::Warband => {
            if let Some(d) = &menus.decorate {
                decorate::draw(&mut commands, root, &look, &ctx, d);
                if let Some(n) = &menus.notice {
                    dev::draw_notice(&mut commands, root, &look, n);
                }
                return;
            }
            warband::draw(&mut commands, root, &look, &ctx);
            if let Some(n) = &menus.notice {
                dev::draw_notice(&mut commands, root, &look, n);
            }
            if menus.run_options_open {
                warband::draw_run_options(&mut commands, root, &look, &ctx);
            }
            if let Some((list, page)) = menus.codex {
                codex::draw(&mut commands, root, &look, &ctx, list, page);
            }
            if let Some(tab) = menus.guide {
                guide::draw(&mut commands, root, &look, &ctx, tab);
            }
            if let Some(page) = &menus.achievements {
                achievements::draw(&mut commands, root, &look, &ctx, page);
            }
            if let Some(d) = &menus.game_options {
                options::draw(&mut commands, root, &look, d);
                if let (true, Some(file)) = (d.saves.confirm, d.saves.importable()) {
                    save_files::draw_confirm(&mut commands, root, &look, file);
                }
            }
            if let Some(c) = menus
                .confirm_delete
                .and_then(|g| screen.characters.iter().find(|c| c.guid == g))
            {
                warband::draw_delete_confirm(&mut commands, root, &look, &c.name);
            }
            // back at the warband after the first clear of Ahn'Qiraj: the Challenge Mode box, once
            // per save, over nothing else
            let overlay = menus.run_options_open
                || menus.codex.is_some()
                || menus.achievements.is_some()
                || menus.game_options.is_some()
                || menus.confirm_delete.is_some();
            if ctx.facts.challenge_news() && !overlay {
                warband::draw_challenge_news(&mut commands, root, &look, ctx.content);
            }
        }
        Screen::Sheet(s) => sheet::draw(
            &mut commands,
            root,
            &look,
            &ctx,
            s,
            menus.notice.as_deref(),
            model.0.clone(),
        ),
        Screen::Unset => {}
    }
}

/// A press of a dev script by its words; `None` for words that name no press.
pub(crate) fn script_action(words: &str) -> Option<MenuAction> {
    let (verb, arg) = match words.split_once(':') {
        Some((v, a)) => (v, Some(a.trim())),
        None => (words, None),
    };
    Some(match (verb.trim(), arg) {
        ("nothing", None) => MenuAction::Nothing,
        ("class", Some(c)) => MenuAction::PickClass(c.into()),
        ("choose", None) => MenuAction::ChooseClass,
        ("back", None) => MenuAction::Back,
        ("create-new", None) => MenuAction::CreateHero,
        ("camp", None) => MenuAction::SwitchCamp,
        ("modify", None) => MenuAction::ModifyHero,
        ("open", Some("collection")) => MenuAction::OpenHeroTab(SheetTab::Collection),
        ("open", Some("talents")) => MenuAction::OpenHeroTab(SheetTab::Talents),
        ("delete", None) => MenuAction::DeleteHero,
        ("no", None) => MenuAction::ConfirmNo,
        ("yes", None) => MenuAction::ConfirmYes,
        ("zone", Some(z)) => MenuAction::PickZone(z.into()),
        ("start", Some(o)) => MenuAction::PickStart(o.into()),
        ("loadout", Some(l)) => MenuAction::PickLoadout(l.into()),
        ("build", Some(n)) => MenuAction::UseBuild(n.parse().ok()?),
        ("save-build", None) => MenuAction::SaveBuild,
        ("name-build", None) => MenuAction::NameBuild,
        ("build-name", Some(n)) => MenuAction::TypeBuildName(n.into()),
        ("drop", Some("start")) => MenuAction::BuildDrop(Some(BuildDrop::Start)),
        ("drop", Some("loadout")) => MenuAction::BuildDrop(Some(BuildDrop::Loadout)),
        ("talents-page", Some("talents")) => MenuAction::TalentsPage(TalentsPage::Talents),
        ("talents-page", Some("prestige")) => MenuAction::TalentsPage(TalentsPage::Prestige),
        ("run-options", None) => MenuAction::OpenRunOptions,
        ("done", None) => MenuAction::CloseRunOptions,
        ("mode", Some("normal")) => MenuAction::PickMode { challenge: false },
        ("mode", Some("challenge")) => MenuAction::PickMode { challenge: true },
        ("challenge-up", None) => MenuAction::ChallengeStep { up: true },
        ("challenge-down", None) => MenuAction::ChallengeStep { up: false },
        ("challenge-okay", None) => MenuAction::CloseChallengeNews,
        ("codex", None) => MenuAction::OpenCodex,
        ("codex-list", Some(l)) => MenuAction::CodexList(match l {
            "rings" => CodexList::Rings,
            "trinkets" => CodexList::Trinkets,
            "amulets" => CodexList::Amulets,
            "legendary" => CodexList::Legendary,
            _ => return None,
        }),
        ("codex-page", Some(d)) => MenuAction::CodexPage(d == "next"),
        ("close-codex", None) => MenuAction::CloseCodex,
        ("guide", None) => MenuAction::OpenGuide,
        ("guide-tab", Some(t)) => MenuAction::GuideTab(guide::GuideTab::named(t)?),
        ("close-guide", None) => MenuAction::CloseGuide,
        ("achievements", None) => MenuAction::OpenAchievements,
        ("achievements-page", Some(p)) => MenuAction::AchievementsPage(AchievementsPage::named(p)),
        ("track", Some(id)) => MenuAction::TrackAchievement { id: id.into(), on: true },
        ("untrack", Some(id)) => MenuAction::TrackAchievement { id: id.into(), on: false },
        ("close-achievements", None) => MenuAction::CloseAchievements,
        ("shop", None) => MenuAction::OpenShop,
        ("shop-pick", Some(i)) => MenuAction::ShopPick(i.into()),
        ("shop-tab", Some(t)) => MenuAction::ShopTab(companions::ShopTab::named(t)?),
        ("shop-buy", None) => MenuAction::ShopBuy,
        ("shop-yes", None) => MenuAction::ShopAnswer(true),
        ("shop-no", None) => MenuAction::ShopAnswer(false),
        ("companions", None) => MenuAction::OpenCompanions,
        ("companion-pick", Some(id)) => MenuAction::CompanionPick(id.into()),
        ("companion", Some(id)) => MenuAction::ChooseCompanion(id.into()),
        ("no-companion", None) => MenuAction::ChooseCompanion(String::new()),
        ("wear", Some(k)) => MenuAction::OpenWear(esurv_protocol::WearKind::from_word(k)?),
        ("wear-pick", Some(a)) => {
            let (k, id) = a.split_once('=')?;
            MenuAction::WearPick(esurv_protocol::WearKind::from_word(k)?, id.into())
        }
        ("dress", Some(a)) => {
            let (k, id) = a.split_once('=').unwrap_or((a, ""));
            MenuAction::ChooseWear(esurv_protocol::WearKind::from_word(k)?, id.into())
        }
        ("decorate", None) => MenuAction::OpenDecorate,
        ("decorate-spot", Some(s)) => MenuAction::DecorateSpot(s.into()),
        ("decorate-piece", Some(p)) => MenuAction::DecoratePiece(p.into()),
        ("decorate-clear", None) => MenuAction::DecoratePiece(String::new()),
        ("decorate-buy", None) => MenuAction::DecorateBuySpot,
        ("decorate-yes", None) => MenuAction::DecorateAnswer(true),
        ("decorate-no", None) => MenuAction::DecorateAnswer(false),
        ("close-decorate", None) => MenuAction::CloseDecorate,
        ("game-options", None) => MenuAction::OpenGameOptions,
        ("quit-game", None) => MenuAction::QuitGame,
        ("update", None) => MenuAction::UpdateGame,
        ("options-tab", Some(t)) => MenuAction::OptionsTab(match t {
            "video" => options::OptionsTab::Video,
            "sound" => options::OptionsTab::Sound,
            "interface" => options::OptionsTab::Interface,
            "saves" => options::OptionsTab::Saves,
            _ => return None,
        }),
        ("export-save", None) => MenuAction::ExportSave,
        ("save-file", Some(f)) => MenuAction::PickSaveFile(f.into()),
        ("import-save", None) => MenuAction::ImportSave,
        ("import-yes", None) => MenuAction::ImportAnswer(true),
        ("import-no", None) => MenuAction::ImportAnswer(false),
        ("option", Some(o)) => {
            let (name, dir) = o.rsplit_once(':')?;
            MenuAction::OptionsChange {
                key: options::key_of(name)?,
                up: dir != "-",
            }
        }
        ("okay", None) => MenuAction::OptionsOkay,
        ("cancel", None) => MenuAction::OptionsCancel,
        ("key-bindings", None) => MenuAction::KeyBindings,
        ("more", Some(i)) => MenuAction::OptionStep {
            slider: i.parse().ok()?,
            up: true,
        },
        ("less", Some(i)) => MenuAction::OptionStep {
            slider: i.parse().ok()?,
            up: false,
        },
        ("start", None) => MenuAction::StartRun,
        ("hero", Some(n)) => MenuAction::PickHeroNamed(n.into()),
        ("tab", Some("character")) => MenuAction::Tab(SheetTab::Character),
        ("tab", Some("collection")) => MenuAction::Tab(SheetTab::Collection),
        ("tab", Some("talents")) => MenuAction::Tab(SheetTab::Talents),
        ("race", Some(r)) => MenuAction::Race(r.parse().ok()?),
        ("sex", Some(s)) => MenuAction::Sex(s.parse().ok()?),
        ("feature", Some(f)) => {
            let (i, dir) = f.split_once(':')?;
            MenuAction::Feature {
                index: i.parse().ok()?,
                up: dir == "+",
            }
        }
        ("randomize", None) => MenuAction::Randomize,
        ("temperament", Some(t)) => MenuAction::Temperament(t.into()),
        ("quiet", None) => MenuAction::Quiet,
        ("random-temperament", None) => MenuAction::RandomTemperament,
        ("time-period", Some(p)) => MenuAction::TimePeriod(TimePeriod::from_word(p)?),
        ("sample", None) => MenuAction::SampleLine,
        ("turn", Some(d)) => MenuAction::Turn(d == "right"),
        ("confirm", None) => MenuAction::ConfirmSheet,
        ("slot", Some(s)) => MenuAction::Slot(s.parse().ok()?),
        ("name", Some(n)) => MenuAction::TypeName(n.into()),
        ("piece", Some(p)) => {
            // `piece:<slot>:<zone>`, or `piece:<slot>:<zone>:<look>` in a row of another look
            let mut parts = p.split(':');
            let (slot, zone) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
            let look = match parts.next() {
                Some(l) => Some(l.parse().ok()?),
                None => None,
            };
            MenuAction::Piece { slot, zone, look }
        }
        // `look:<slot>:<look>` shows a look of the collection in a slot, `look:<slot>` its own
        ("look", Some(p)) => {
            let mut parts = p.split(':');
            let slot = parts.next()?.parse().ok()?;
            let look = match parts.next() {
                Some(l) => Some(l.parse().ok()?),
                None => None,
            };
            MenuAction::ShowLook { slot, look }
        }
        ("talent", Some(t)) => MenuAction::Talent(t.into()),
        ("untalent", Some(t)) => MenuAction::TalentBack(t.into()),
        ("prestige", Some(p)) => MenuAction::Prestige(p.into()),
        ("unprestige", Some(p)) => MenuAction::PrestigeBack(p.into()),
        ("learn", None) => MenuAction::Learn,
        ("unlearn", None) => MenuAction::Unlearn,
        ("reset", None) => MenuAction::ResetTalents,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> Sheet {
        Sheet {
            mode: SheetMode::Modify(1),
            tab: SheetTab::Talents,
            draft: Draft {
                class: "warrior".into(),
                race: 2,
                sex: 0,
                look: [0; 5],
                name: "Bo".into(),
                temperament: Vec::new(),
                time_period: TimePeriod::Present,
            },
            talent: None,
            prestige: None,
            piece: None,
            slot: None,
            companions: false,
            companion: None,
            wear: None,
            wear_pick: None,
            sample: 0,
            talents_page: TalentsPage::Talents,
            build_drop: None,
            build_name: None,
        }
    }

    #[test]
    fn a_first_click_picks_a_second_learns_a_right_click_takes_back() {
        assert_eq!(left_click(None, "cleave"), ClickDoes::Pick);
        assert_eq!(left_click(Some("rend"), "cleave"), ClickDoes::Pick);
        assert_eq!(left_click(Some("cleave"), "cleave"), ClickDoes::Learn);
        let mut s = sheet();
        let talent = MenuAction::Talent("cleave".into());
        assert_eq!(click_on_a_rank(&mut s, &talent), None, "picked");
        assert_eq!(s.talent.as_deref(), Some("cleave"));
        assert_eq!(click_on_a_rank(&mut s, &talent), Some(MenuAction::Learn));
        assert_eq!(click_on_a_rank(&mut s, &talent), Some(MenuAction::Learn), "and again");
        assert_eq!(
            click_on_a_rank(&mut s, &MenuAction::TalentBack("rend".into())),
            Some(MenuAction::Unlearn)
        );
        assert_eq!(s.talent.as_deref(), Some("rend"), "a right click picks it too");
        // a prestige stat the same; picking one lets go of the other
        let damage = MenuAction::Prestige("damage".into());
        assert_eq!(click_on_a_rank(&mut s, &damage), None);
        assert_eq!((s.talent.as_deref(), s.prestige.as_deref()), (None, Some("damage")));
        assert_eq!(click_on_a_rank(&mut s, &damage), Some(MenuAction::Learn));
        assert_eq!(
            click_on_a_rank(&mut s, &MenuAction::PrestigeBack("damage".into())),
            Some(MenuAction::Unlearn)
        );
        assert_eq!(click_on_a_rank(&mut s, &talent), None, "back to a talent: picked first");
        assert_eq!(s.prestige, None);
        // what a right click asks
        assert_eq!(right_click(&talent), Some(MenuAction::TalentBack("cleave".into())));
        assert_eq!(right_click(&damage), Some(MenuAction::PrestigeBack("damage".into())));
        assert_eq!(right_click(&MenuAction::Learn), None);
        assert_eq!(script_action("prestige:damage"), Some(damage));
        assert_eq!(script_action("untalent:cleave"), Some(MenuAction::TalentBack("cleave".into())));
    }

    #[test]
    fn a_script_names_its_presses() {
        assert_eq!(
            script_action("class:mage"),
            Some(MenuAction::PickClass("mage".into()))
        );
        assert_eq!(script_action("choose"), Some(MenuAction::ChooseClass));
        assert_eq!(script_action("loadout:shield"), Some(MenuAction::PickLoadout("shield".into())));
        assert_eq!(script_action("loadout"), None);
        assert_eq!(script_action("build:3"), Some(MenuAction::UseBuild(3)));
        assert_eq!(script_action("build:x"), None);
        assert_eq!(script_action("save-build"), Some(MenuAction::SaveBuild));
        assert_eq!(script_action("name-build"), Some(MenuAction::NameBuild));
        assert_eq!(script_action("build-name:Frost"), Some(MenuAction::TypeBuildName("Frost".into())));
        assert_eq!(script_action("drop:loadout"), Some(MenuAction::BuildDrop(Some(BuildDrop::Loadout))));
        assert_eq!(script_action("talents-page:prestige"), Some(MenuAction::TalentsPage(TalentsPage::Prestige)));
        assert_eq!(right_click(&MenuAction::UseBuild(2)), Some(MenuAction::ClearBuild(2)), "a right click empties a build");
        assert_eq!(script_action("guide"), Some(MenuAction::OpenGuide));
        assert_eq!(
            script_action("guide-tab:loot"),
            Some(MenuAction::GuideTab(guide::GuideTab::Loot))
        );
        assert_eq!(script_action("guide-tab:nothing"), None);
        assert_eq!(script_action("close-guide"), Some(MenuAction::CloseGuide));
        assert_eq!(
            script_action("achievements-page:warrior"),
            Some(MenuAction::AchievementsPage(AchievementsPage::Class("warrior".into())))
        );
        assert_eq!(
            script_action("untrack:slayer_1"),
            Some(MenuAction::TrackAchievement { id: "slayer_1".into(), on: false })
        );
        assert_eq!(script_action("fly"), None);
    }

    #[test]
    fn a_new_hero_starts_as_the_first_race_its_class_may_be() {
        let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let choices = CreateChoices::default();
        let d = new_draft(&content, &choices, "warlock");
        assert_eq!((d.class.as_str(), d.sex, d.look), ("warlock", 0, [0; 5]));
        assert_eq!(d.race, 1, "with no data every race the content has passes");
    }

    /// A new hero is from the Present; the sheet's Time period block picks another, and its
    /// sample lines are only those of its own time period or of any time.
    #[test]
    fn a_hero_sheet_picks_a_time_period_and_samples_its_lines() {
        let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let mut d = new_draft(&content, &CreateChoices::default(), "warrior");
        assert_eq!(d.time_period, TimePeriod::Present);
        assert_eq!(script_action("time-period:past"), Some(MenuAction::TimePeriod(TimePeriod::Past)));
        assert_eq!(script_action("time-period:later"), None);
        let all: Vec<String> = content.camp_chatter.temperaments.iter().map(|t| t.id.clone()).collect();
        for period in TimePeriod::ALL {
            d.time_period = period;
            let mut own = 0;
            for k in 0..content.camp_chatter.lines.len() {
                let text = sheet::sample_line(&content, &d, &all, k).unwrap();
                let line = content.camp_chatter.lines.iter().find(|l| l.text == text).unwrap();
                assert!(line.time_period.is_none_or(|p| p == period), "{text} for {}", period.word());
                own += usize::from(line.time_period == Some(period));
            }
            assert!(own > 0, "a {} hero samples lines of its own time", period.word());
        }
    }
}

/// What hero `h` wears of `kind` (its id; empty for none).
fn wear_of(h: &esurv_protocol::HeroFacts, kind: esurv_protocol::WearKind) -> &str {
    match kind {
        esurv_protocol::WearKind::Title => &h.title,
        esurv_protocol::WearKind::Tabard => &h.tabard,
        esurv_protocol::WearKind::Glow => &h.glow,
    }
}
