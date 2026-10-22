//! The run's Escape menu (menus.md "In a run: the Escape menu"). Once
//! the hero is in a run, the menu is the game menu: it stands in for the stock one
//! (`benilla_app::GameMenuStandIn`), so Escape, once the stock interface has nothing left to close
//! (a focused chat line, a popup, an open window, a cast and a target come first), opens it, and
//! so does the main menu button. The stock game menu never shows in a run;
//! only a choice screen keeps Escape for itself.
//!
//! Which menu Escape opens ([`MenuKind`], [`answer_escape`]):
//!
//! * a running run pauses (`pause`), and the menu shows once the server says it is paused; a
//!   second Escape resumes it. Five buttons ([`MenuButton`]): Continue (`resume`), Reset Run
//!   (`reset`: the run is paid out as a run left and a fresh one starts in the same zone with the
//!   same hero), Options (asks nothing of the server: the menus' Options panel opens over the
//!   paused run, `crate::menus`, and the menu hides under it until it closes), End Run (`leave`:
//!   paid out, back to character select) and Quit Game (`leave`, then the game closes once the
//!   hero is out of the world, the save written);
//! * before the first step there is nothing to pause (the server refuses it): the menu opens on
//!   the client alone ([`MenuHere`]), with Continue (it closes), Options, End Run and Quit Game;
//! * on the end panel (a fall or a victory) likewise, with only Options, Return to main menu (`leave`, as
//!   the panel's own) and Quit Game: nothing that would cross the panel's Run Again or Go Endless.
//!   The end panel hides while it is up.
//!
//! A second Escape closes a menu opened on the client; so does the phase moving on (a step taken
//! under it, a fresh run, the hero out of the world). One ask at a time ([`PendingAsk`]). How it
//! looks is apart, in `hud::escape_menu_look`, so the look can change without these rules.

use bevy::prelude::{Component, Resource};
use esurv_protocol::{Phase, ToServer};

/// Whether the menu stands in for the stock game menu: whenever a run is up, but over a choice
/// screen (which keeps Escape for itself).
pub fn stands_in_for_the_game_menu(phase: Option<Phase>) -> bool {
    matches!(
        phase,
        Some(Phase::Waiting | Phase::Running | Phase::Paused | Phase::Victory | Phase::Defeat)
    )
}

/// Which of the menu's faces shows.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuKind {
    /// The run paused by the server: every button.
    Paused,
    /// Before the first step, opened on the client.
    BeforeStart,
    /// Over the end panel (a fall or a victory), opened on the client.
    RunOver,
}

impl MenuKind {
    pub const ALL: [MenuKind; 3] = [MenuKind::Paused, MenuKind::BeforeStart, MenuKind::RunOver];

    /// Its buttons, top to bottom.
    pub fn buttons(self) -> &'static [MenuButton] {
        match self {
            MenuKind::Paused => &MenuButton::ALL,
            MenuKind::BeforeStart => &[
                MenuButton::Continue,
                MenuButton::Options,
                MenuButton::End,
                MenuButton::Quit,
            ],
            MenuKind::RunOver => &[MenuButton::Options, MenuButton::End, MenuButton::Quit],
        }
    }

    /// Its heading and the hint under it.
    pub fn words(self) -> (&'static str, &'static str) {
        use super::state::{
            BEFORE_START_HINT, GAME_MENU_TITLE, MENU_HINT, MENU_TITLE, RUN_OVER_HINT,
        };
        match self {
            MenuKind::Paused => (MENU_TITLE, MENU_HINT),
            MenuKind::BeforeStart => (GAME_MENU_TITLE, BEFORE_START_HINT),
            MenuKind::RunOver => (GAME_MENU_TITLE, RUN_OVER_HINT),
        }
    }
}

/// The phase a menu was opened in on the client (before the first step, on the end panel); `None`
/// while none is.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuHere(pub Option<Phase>);

/// Whether a phase's menu is opened on the client: there is no paused run for the server to say.
fn opened_here(phase: Phase) -> bool {
    matches!(phase, Phase::Waiting | Phase::Victory | Phase::Defeat)
}

/// The menu showing, if any: the server's paused run, or one opened here in the phase it still is.
pub fn menu_kind(phase: Option<Phase>, here: MenuHere) -> Option<MenuKind> {
    match phase? {
        Phase::Paused => Some(MenuKind::Paused),
        p if here.0 != Some(p) => None,
        Phase::Waiting => Some(MenuKind::BeforeStart),
        Phase::Victory | Phase::Defeat => Some(MenuKind::RunOver),
        _ => None,
    }
}

/// What Escape (or the main menu button) does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscapeAnswer {
    /// The Options panel is open over the menu: it closes, as its Cancel (back to the menu).
    CloseOptions,
    /// The server is asked (pause a running run, resume a paused one).
    Ask(ToServer),
    /// A menu opens on the client, in this phase.
    OpenHere(Phase),
    /// The menu opened on the client closes.
    CloseHere,
    Nothing,
}

/// What Escape does in `phase`, with a menu opened here or not and the Options panel open or not.
pub fn answer_escape(phase: Option<Phase>, here: MenuHere, options_open: bool) -> EscapeAnswer {
    if options_open {
        return EscapeAnswer::CloseOptions;
    }
    match phase {
        Some(Phase::Running | Phase::Paused) => {
            menu_request(phase).map_or(EscapeAnswer::Nothing, EscapeAnswer::Ask)
        }
        Some(p) if opened_here(p) => {
            if here.0 == Some(p) {
                EscapeAnswer::CloseHere
            } else {
                EscapeAnswer::OpenHere(p)
            }
        }
        _ => EscapeAnswer::Nothing,
    }
}

/// What a button pressed does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Press {
    Ask(ToServer),
    /// The menu opened on the client closes (Continue before the first step).
    CloseHere,
    /// Nothing of the HUD's (Options: the menus open their panel).
    Nothing,
}

/// What asking for the game menu does: a running run pauses, a paused one resumes.
pub fn menu_request(phase: Option<Phase>) -> Option<ToServer> {
    match phase {
        Some(Phase::Running) => Some(ToServer::Pause),
        Some(Phase::Paused) => Some(ToServer::Resume),
        _ => None,
    }
}

/// Whether the menu shows.
pub fn menu_shown(phase: Option<Phase>) -> bool {
    phase == Some(Phase::Paused)
}

/// An ask of the menu not answered yet: a second ask waits for the server to answer the first (a
/// new phase, a fresh run, the hero out of the world, or a refusal).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PendingAsk(Option<(ToServer, Option<Phase>)>);

impl PendingAsk {
    /// The request to send for an ask in `phase`, if any: none while one is out.
    pub fn ask(&mut self, phase: Option<Phase>, request: Option<ToServer>) -> Option<ToServer> {
        if self.0.is_some() {
            return None;
        }
        let request = request?;
        self.0 = Some((request.clone(), phase));
        Some(request)
    }

    /// The server answered: the phase moved on, or it refused that verb.
    pub fn answered(&mut self, phase: Option<Phase>, refused: Option<&str>) {
        if let Some((request, asked_in)) = &self.0 {
            if *asked_in != phase || refused == Some(request.verb()) {
                self.0 = None;
            }
        }
    }

    pub fn is_out(&self) -> bool {
        self.0.is_some()
    }

    pub fn clear(&mut self) {
        self.0 = None;
    }
}

/// One of the menu's buttons, top to bottom.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuButton {
    Continue,
    Reset,
    Options,
    End,
    Quit,
}

impl MenuButton {
    pub const ALL: [MenuButton; 5] = [
        MenuButton::Continue,
        MenuButton::Reset,
        MenuButton::Options,
        MenuButton::End,
        MenuButton::Quit,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MenuButton::Continue => super::state::CONTINUE_BUTTON,
            MenuButton::Reset => super::state::RESET_BUTTON,
            MenuButton::Options => super::state::OPTIONS_BUTTON,
            MenuButton::End => super::state::END_BUTTON,
            MenuButton::Quit => super::state::QUIT_BUTTON,
        }
    }

    /// Its name in the development switch (`reset@20`).
    pub fn word(self) -> &'static str {
        match self {
            MenuButton::Continue => "continue",
            MenuButton::Reset => "reset",
            MenuButton::Options => "options",
            MenuButton::End => "end",
            MenuButton::Quit => "quit",
        }
    }

    pub fn from_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.word() == word)
    }

    /// The request it sends; Options sends none (it opens the Options panel over the run).
    pub fn request(self) -> Option<ToServer> {
        match self {
            MenuButton::Continue => Some(ToServer::Resume),
            MenuButton::Reset => Some(ToServer::Reset),
            MenuButton::Options => None,
            MenuButton::End | MenuButton::Quit => Some(ToServer::Leave),
        }
    }

    /// What it does on the menu of `kind`: Continue resumes a paused run and only closes a menu
    /// opened on the client; Reset only resets a paused run.
    pub fn press(self, kind: MenuKind) -> Press {
        match (self, kind) {
            (MenuButton::Continue | MenuButton::Reset, MenuKind::Paused) => {
                self.request().map_or(Press::Nothing, Press::Ask)
            }
            (MenuButton::Continue, _) => Press::CloseHere,
            (MenuButton::Reset | MenuButton::Options, _) => Press::Nothing,
            (MenuButton::End | MenuButton::Quit, _) => Press::Ask(ToServer::Leave),
        }
    }

    /// Its label on the menu of `kind`: over the end panel End Run is the panel's Return to main menu.
    pub fn label_on(self, kind: MenuKind) -> &'static str {
        if self == MenuButton::End && kind == MenuKind::RunOver {
            return super::state::MAIN_MENU_BUTTON;
        }
        self.label()
    }

    /// Whether the game closes once the hero is out of the world.
    pub fn closes_the_game(self) -> bool {
        self == MenuButton::Quit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_stands_in_whenever_a_run_is_up_but_over_a_choice() {
        for (phase, stands_in) in [
            (None, false),
            (Some(Phase::Waiting), true),
            (Some(Phase::Running), true),
            (Some(Phase::Choosing), false),
            (Some(Phase::Paused), true),
            (Some(Phase::Defeat), true),
            (Some(Phase::Victory), true),
        ] {
            assert_eq!(stands_in_for_the_game_menu(phase), stands_in, "{phase:?}");
        }
    }

    #[test]
    fn escape_pauses_a_running_run_and_resumes_a_paused_one() {
        assert_eq!(menu_request(Some(Phase::Running)), Some(ToServer::Pause));
        assert_eq!(menu_request(Some(Phase::Paused)), Some(ToServer::Resume));
        assert_eq!(
            menu_request(Some(Phase::Choosing)),
            None,
            "never over a choice screen"
        );
        assert_eq!(menu_request(Some(Phase::Waiting)), None, "not started");
        assert_eq!(menu_request(Some(Phase::Victory)), None);
        assert!(menu_shown(Some(Phase::Paused)));
        assert!(!menu_shown(Some(Phase::Running)));
    }

    #[test]
    fn one_ask_at_a_time() {
        let mut pending = PendingAsk::default();
        let running = Some(Phase::Running);
        assert_eq!(
            pending.ask(running, menu_request(running)),
            Some(ToServer::Pause)
        );
        assert_eq!(
            pending.ask(running, menu_request(running)),
            None,
            "a second Escape before the answer"
        );
        pending.answered(running, None);
        assert_eq!(
            pending.ask(running, Some(ToServer::Pause)),
            None,
            "still out"
        );
        let paused = Some(Phase::Paused);
        pending.answered(paused, None);
        assert_eq!(
            pending.ask(paused, menu_request(paused)),
            Some(ToServer::Resume),
            "answered by the new phase"
        );
        pending.answered(paused, Some("resume"));
        assert_eq!(
            pending.ask(paused, Some(ToServer::Resume)),
            Some(ToServer::Resume),
            "answered by a refusal"
        );
        assert_eq!(pending.ask(paused, None), None);
    }

    #[test]
    fn a_reset_is_answered_by_the_fresh_run_or_a_refusal() {
        let paused = Some(Phase::Paused);
        let mut pending = PendingAsk::default();
        assert_eq!(
            pending.ask(paused, MenuButton::Reset.request()),
            Some(ToServer::Reset)
        );
        assert_eq!(
            pending.ask(paused, MenuButton::End.request()),
            None,
            "no End while the reset is out"
        );
        pending.answered(paused, Some("resume"));
        assert!(pending.is_out(), "another verb's refusal is not its answer");
        // the fresh run's `Ready` forgets the old run's phase
        pending.answered(None, None);
        assert!(!pending.is_out());
        pending.ask(paused, Some(ToServer::Reset));
        pending.answered(paused, Some("reset"));
        assert!(!pending.is_out());
    }

    #[test]
    fn the_buttons_ask_the_server() {
        assert_eq!(MenuButton::Continue.request(), Some(ToServer::Resume));
        assert_eq!(MenuButton::Reset.request(), Some(ToServer::Reset));
        assert_eq!(MenuButton::End.request(), Some(ToServer::Leave));
        assert_eq!(
            MenuButton::Quit.request(),
            Some(ToServer::Leave),
            "out first"
        );
        assert_eq!(
            MenuButton::Options.request(),
            None,
            "the panel is the client's own"
        );
        assert!(!MenuButton::Options.closes_the_game());
        assert!(MenuButton::Quit.closes_the_game());
        assert!(!MenuButton::End.closes_the_game());
        for b in MenuButton::ALL {
            assert_eq!(MenuButton::from_word(b.word()), Some(b));
        }
        assert_eq!(MenuButton::from_word("armory"), None);
    }

    #[test]
    fn escape_never_leaves_the_stock_game_menu_to_open() {
        use EscapeAnswer::*;
        let none = MenuHere::default();
        assert_eq!(
            answer_escape(Some(Phase::Running), none, false),
            Ask(ToServer::Pause)
        );
        assert_eq!(
            answer_escape(Some(Phase::Paused), none, false),
            Ask(ToServer::Resume)
        );
        for p in [Phase::Waiting, Phase::Victory, Phase::Defeat] {
            assert_eq!(answer_escape(Some(p), none, false), OpenHere(p), "{p:?}");
            assert_eq!(
                answer_escape(Some(p), MenuHere(Some(p)), false),
                CloseHere,
                "a second Escape closes it: {p:?}"
            );
            assert_eq!(
                answer_escape(Some(p), MenuHere(Some(p)), true),
                CloseOptions,
                "the Options panel first: {p:?}"
            );
        }
        assert_eq!(answer_escape(Some(Phase::Paused), none, true), CloseOptions);
        assert_eq!(answer_escape(Some(Phase::Choosing), none, false), Nothing);
        assert_eq!(answer_escape(None, none, false), Nothing);
    }

    #[test]
    fn the_menu_shown_follows_the_phase() {
        let none = MenuHere::default();
        assert_eq!(menu_kind(Some(Phase::Paused), none), Some(MenuKind::Paused));
        assert_eq!(menu_kind(Some(Phase::Waiting), none), None);
        let waiting = MenuHere(Some(Phase::Waiting));
        assert_eq!(
            menu_kind(Some(Phase::Waiting), waiting),
            Some(MenuKind::BeforeStart)
        );
        assert_eq!(
            menu_kind(Some(Phase::Running), waiting),
            None,
            "a step taken under it: the run runs, the menu goes"
        );
        for p in [Phase::Victory, Phase::Defeat] {
            assert_eq!(menu_kind(Some(p), MenuHere(Some(p))), Some(MenuKind::RunOver));
        }
        assert_eq!(
            menu_kind(Some(Phase::Running), MenuHere(Some(Phase::Victory))),
            None,
            "on into Endless"
        );
        assert_eq!(menu_kind(None, waiting), None);
    }

    #[test]
    fn each_menu_has_only_the_buttons_that_fit() {
        assert_eq!(MenuKind::Paused.buttons(), &MenuButton::ALL);
        let before = MenuKind::BeforeStart.buttons();
        assert!(!before.contains(&MenuButton::Reset), "nothing to reset yet");
        assert!(before.contains(&MenuButton::Options));
        assert_eq!(
            MenuKind::RunOver.buttons(),
            &[MenuButton::Options, MenuButton::End, MenuButton::Quit],
            "nothing that crosses the end panel's own buttons"
        );
        assert_eq!(MenuButton::End.label_on(MenuKind::RunOver), "Return to main menu");
        assert_eq!(MenuButton::End.label_on(MenuKind::Paused), "End Run");
        for kind in MenuKind::ALL {
            assert!(kind.buttons().contains(&MenuButton::Quit));
            assert!(!kind.words().0.is_empty());
        }
    }

    #[test]
    fn a_menu_opened_here_asks_only_to_leave() {
        use MenuKind::*;
        assert_eq!(MenuButton::Continue.press(Paused), Press::Ask(ToServer::Resume));
        assert_eq!(MenuButton::Reset.press(Paused), Press::Ask(ToServer::Reset));
        assert_eq!(MenuButton::Continue.press(BeforeStart), Press::CloseHere);
        assert_eq!(MenuButton::Options.press(RunOver), Press::Nothing);
        for kind in MenuKind::ALL {
            assert_eq!(MenuButton::End.press(kind), Press::Ask(ToServer::Leave));
            assert_eq!(MenuButton::Quit.press(kind), Press::Ask(ToServer::Leave));
        }
    }
}
