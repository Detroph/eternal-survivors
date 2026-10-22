//! The level-up, cache and rare cache card screens: when the server opens a choice for the
//! hero ([`ToClient::Choices`]) a panel in the menus' look shows one card per offer, side by
//! side, between the hero's spells and worn jewelry, and the run waits until one is taken. Keys 1
//! to 4 (and the keypad's) or a click take a card, R or the reroll button rerolls them; input is
//! ignored for [`INPUT_GUARD`] seconds after the screen opens or its cards change, and one answer
//! goes per screen. While the cards are open their keys (and Escape, which does nothing then)
//! reach them and no key binding. [`CardScreen::is_open`] says whether they show.

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use benilla_app::{Objects, OverlayKeyPressed, OverlayKeys};
use esurv_protocol::{Choices, Phase, ToClient, ToServer};

use crate::protocol::{ClientScreens, Request, ServerEvent};
use crate::stock::StockSystems;
use crate::ui::in_world;

pub(crate) mod card_text;
mod lines;
mod dev;
mod screen;

/// Input is ignored this long after the screen opens or its cards change, so a key pressed for
/// the game does not take a card by chance.
pub const INPUT_GUARD: f32 = 0.45;

/// At most this many cards show.
pub const MAX_CARDS: usize = 4;

/// The keys the open screen takes: the four card keys, the keypad's, R, and Escape.
pub const SCREEN_KEYS: [KeyCode; 10] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Numpad1,
    KeyCode::Numpad2,
    KeyCode::Numpad3,
    KeyCode::Numpad4,
    KeyCode::KeyR,
    KeyCode::Escape,
];

/// What a key or a click asks of the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardAction {
    /// Take card `n` (from 0).
    Take(usize),
    Reroll,
}

/// What a key does on the screen, if anything.
pub fn key_action(key: KeyCode) -> Option<CardAction> {
    Some(match key {
        KeyCode::Digit1 | KeyCode::Numpad1 => CardAction::Take(0),
        KeyCode::Digit2 | KeyCode::Numpad2 => CardAction::Take(1),
        KeyCode::Digit3 | KeyCode::Numpad3 => CardAction::Take(2),
        KeyCode::Digit4 | KeyCode::Numpad4 => CardAction::Take(3),
        KeyCode::KeyR => CardAction::Reroll,
        _ => return None,
    })
}

/// Whether a reroll can be paid: a free one is left, or the hero has the gold (when its gold is
/// not known yet, the server decides).
pub fn can_reroll(free: u32, cost: u32, gold: Option<u32>) -> bool {
    free > 0 || gold.is_none_or(|g| g >= cost)
}

/// An open screen: its cards, when they last changed, and whether it was answered.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenCards {
    pub choices: Choices,
    /// Seconds since the app started, when the cards came.
    pub since: f32,
    /// A take or a reroll went out: nothing more until the server answers.
    pub answered: bool,
    /// The card taken, lit until the server answers.
    pub taken: Option<usize>,
    /// A dev aid's made-up screen: its answers go nowhere.
    pub made_up: bool,
}

impl OpenCards {
    /// The request `action` makes at `now`, if the screen takes it: not in the guard time, not
    /// once answered, a card that is there, a reroll that can be paid.
    pub fn answer(&self, action: CardAction, now: f32, gold: Option<u32>) -> Option<ToServer> {
        if self.answered || now - self.since < INPUT_GUARD {
            return None;
        }
        match action {
            CardAction::Take(n) if n < self.choices.offers.len().min(MAX_CARDS) => {
                Some(ToServer::Choose(n as u32))
            }
            CardAction::Reroll
                if can_reroll(self.choices.free_rerolls, self.choices.reroll_cost, gold) =>
            {
                Some(ToServer::Reroll)
            }
            _ => None,
        }
    }
}

/// The card screen's state: the open cards, if any, and the hero's gold as the HUD last said.
#[derive(Resource, Default, Debug)]
pub struct CardScreen {
    pub open: Option<OpenCards>,
    pub gold: Option<u32>,
}

impl CardScreen {
    /// Whether the cards show (the pause key leaves the run alone then).
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
}

/// The card screens, and the client's word to the server that it has them.
pub struct CardsPlugin;

impl Plugin for CardsPlugin {
    fn build(&self, app: &mut App) {
        // the server waits for the player's pick instead of taking the first card
        app.init_resource::<ClientScreens>();
        app.world_mut().resource_mut::<ClientScreens>().0.cards = true;
        app.init_resource::<CardScreen>().add_systems(
            Update,
            (
                read_server,
                close_out_of_world,
                answer_keys,
                screen::answer_clicks,
                take_keys,
                screen::show_cards,
                screen::light_cards,
            )
                .chain()
                .before(StockSystems),
        );
        dev::register(app);
    }
}

/// Opens, changes and closes the screen on the server's word, and keeps the hero's gold.
pub(super) fn read_server(
    time: Res<Time<Real>>,
    mut events: MessageReader<ServerEvent>,
    mut screen: ResMut<CardScreen>,
) {
    for ServerEvent(m) in events.read() {
        match m {
            ToClient::Choices(choices) => {
                screen.open = Some(OpenCards {
                    choices: choices.clone(),
                    since: time.elapsed_secs(),
                    answered: false,
                    taken: None,
                    made_up: false,
                });
            }
            ToClient::ChoicesClosed { .. } | ToClient::RunEnd(_) | ToClient::Ready(_) => {
                screen.open = None;
            }
            // the server said no (a reroll it could not pay): the screen answers again
            ToClient::Refused(verb) if verb == "choose" || verb == "reroll" => {
                if let Some(open) = screen.open.as_mut() {
                    open.answered = false;
                    open.taken = None;
                }
            }
            ToClient::Hud(hud) => {
                screen.gold = Some(hud.gold);
                // a run that stopped, or has not begun, has no choice open
                if matches!(hud.phase, Phase::Waiting | Phase::Defeat | Phase::Victory) {
                    screen.open = None;
                }
            }
            _ => {}
        }
    }
}

/// The screen closes when the hero leaves the world.
fn close_out_of_world(objects: Objects, mut screen: ResMut<CardScreen>) {
    if screen.open.is_some() && !in_world(&objects) {
        screen.open = None;
    }
}

/// The card keys' presses, which come to the screen instead of the key bindings.
fn answer_keys(
    time: Res<Time<Real>>,
    mut presses: MessageReader<OverlayKeyPressed>,
    mut screen: ResMut<CardScreen>,
    mut requests: MessageWriter<Request>,
) {
    for OverlayKeyPressed(key) in presses.read() {
        let Some(action) = key_action(*key) else {
            continue;
        };
        answer(&mut screen, action, time.elapsed_secs(), &mut requests);
    }
}

/// Sends what `action` asks, if the screen takes it.
fn answer(
    screen: &mut CardScreen,
    action: CardAction,
    now: f32,
    requests: &mut MessageWriter<Request>,
) {
    let gold = screen.gold;
    let Some(open) = screen.open.as_mut() else {
        return;
    };
    if let Some(request) = open.answer(action, now, gold) {
        open.answered = true;
        if let ToServer::Choose(n) = request {
            open.taken = Some(n as usize);
        }
        if !open.made_up {
            requests.write(Request(request));
        }
    }
}

/// The card screen's name among the screens that take keys.
pub const KEYS_OWNER: &str = "cards";

/// While the cards are open their keys are theirs.
fn take_keys(screen: Res<CardScreen>, mut keys: ResMut<OverlayKeys>) {
    let want: &[KeyCode] = if screen.is_open() { &SCREEN_KEYS } else { &[] };
    if keys.of(KEYS_OWNER) != want {
        keys.set(KEYS_OWNER, want);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_protocol::{Offer, OfferKind, ScreenKind, Wire};

    fn open(offers: usize, free: u32, cost: u32) -> OpenCards {
        OpenCards {
            choices: Choices {
                hero: 1,
                kind: ScreenKind::LevelUp,
                level: 2,
                reroll_cost: cost,
                free_rerolls: free,
                offers: (0..offers)
                    .map(|_| Offer {
                        kind: OfferKind::Gold,
                        id: String::new(),
                        name: String::new(),
                        rarity: 0,
                        values: vec![],
                        numbers: vec![],
                    })
                    .collect(),
                kit: Default::default(),
            },
            since: 10.0,
            answered: false,
            taken: None,
            made_up: false,
        }
    }

    #[test]
    fn the_number_keys_and_the_keypad_take_cards_and_r_rerolls() {
        assert_eq!(key_action(KeyCode::Digit1), Some(CardAction::Take(0)));
        assert_eq!(key_action(KeyCode::Numpad4), Some(CardAction::Take(3)));
        assert_eq!(key_action(KeyCode::KeyR), Some(CardAction::Reroll));
        assert_eq!(key_action(KeyCode::Digit5), None);
        assert_eq!(key_action(KeyCode::Escape), None, "Escape does nothing");
        for key in SCREEN_KEYS {
            assert!(key == KeyCode::Escape || key_action(key).is_some());
        }
    }

    #[test]
    fn input_waits_out_the_guard_after_the_cards_come() {
        let cards = open(3, 1, 0);
        assert_eq!(cards.answer(CardAction::Take(0), 10.2, None), None);
        assert_eq!(cards.answer(CardAction::Take(0), 10.44, None), None);
        assert_eq!(
            cards.answer(CardAction::Take(0), 10.46, None),
            Some(ToServer::Choose(0))
        );
    }

    #[test]
    fn hello_says_this_client_has_the_cards() {
        let mut app = App::new();
        app.add_plugins(CardsPlugin);
        let screens = app.world().resource::<ClientScreens>().0;
        assert!(screens.cards && !screens.run_end);
        let hello = Request(ToServer::Hello {
            version: esurv_protocol::VERSION,
            screens,
        });
        assert!(hello.0.lines()[0].to_string().ends_with(" cards"));
    }

    #[test]
    fn only_cards_that_are_there_can_be_taken() {
        let cards = open(3, 0, 100);
        assert_eq!(
            cards.answer(CardAction::Take(2), 11.0, None),
            Some(ToServer::Choose(2))
        );
        assert_eq!(cards.answer(CardAction::Take(3), 11.0, None), None);
    }

    #[test]
    fn one_answer_per_screen() {
        let mut cards = open(3, 1, 0);
        cards.answered = true;
        assert_eq!(cards.answer(CardAction::Take(0), 11.0, None), None);
        assert_eq!(cards.answer(CardAction::Reroll, 11.0, None), None);
    }

    #[test]
    fn a_reroll_needs_a_free_one_or_the_gold() {
        assert!(can_reroll(2, 0, Some(0)));
        assert!(!can_reroll(0, 300, Some(299)));
        assert!(can_reroll(0, 300, Some(300)));
        assert!(can_reroll(0, 300, None), "gold unknown: the server decides");
        let cards = open(3, 0, 300);
        assert_eq!(cards.answer(CardAction::Reroll, 11.0, Some(100)), None);
        assert_eq!(
            cards.answer(CardAction::Reroll, 11.0, Some(900)),
            Some(ToServer::Reroll)
        );
    }

    #[test]
    fn the_keys_are_the_screens_only_while_it_is_open() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<OverlayKeys>();
        world.insert_resource(CardScreen {
            open: Some(open(3, 1, 0)),
            gold: None,
        });
        world.run_system_once(take_keys).unwrap();
        assert_eq!(world.resource::<OverlayKeys>().of(KEYS_OWNER), SCREEN_KEYS);
        assert!(world.resource::<OverlayKeys>().takes(KeyCode::Escape));
        world.resource_mut::<CardScreen>().open = None;
        world.run_system_once(take_keys).unwrap();
        assert!(!world.resource::<OverlayKeys>().takes(KeyCode::Digit1));
    }

    #[test]
    fn the_servers_word_opens_changes_and_closes_the_screen() {
        let mut world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(read_server);
        world.init_resource::<Messages<ServerEvent>>();
        world.insert_resource(Time::<Real>::default());
        world.init_resource::<CardScreen>();
        let cards = open(3, 1, 0).choices;
        world.write_message(ServerEvent(ToClient::Choices(cards.clone())));
        schedule.run(&mut world);
        assert_eq!(
            world
                .resource::<CardScreen>()
                .open
                .as_ref()
                .unwrap()
                .choices,
            cards
        );

        // a refused reroll lets the screen answer again
        world
            .resource_mut::<CardScreen>()
            .open
            .as_mut()
            .unwrap()
            .answered = true;
        world.write_message(ServerEvent(ToClient::Refused("reroll".into())));
        schedule.run(&mut world);
        assert!(
            !world
                .resource::<CardScreen>()
                .open
                .as_ref()
                .unwrap()
                .answered
        );

        world.write_message(ServerEvent(ToClient::ChoicesClosed { hero: 1 }));
        schedule.run(&mut world);
        assert!(!world.resource::<CardScreen>().is_open());
    }

    #[test]
    fn a_key_press_sends_the_pick_once() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<Messages<OverlayKeyPressed>>();
        world.init_resource::<Messages<Request>>();
        world.insert_resource(Time::<Real>::default());
        let mut cards = open(3, 1, 0);
        cards.since = -1.0;
        world.insert_resource(CardScreen {
            open: Some(cards),
            gold: None,
        });
        world.write_message(OverlayKeyPressed(KeyCode::Digit2));
        world.write_message(OverlayKeyPressed(KeyCode::Digit3));
        world.run_system_once(answer_keys).unwrap();
        let sent: Vec<_> = world.resource_mut::<Messages<Request>>().drain().collect();
        assert_eq!(sent, vec![Request(ToServer::Choose(1))]);
        let open = world.resource::<CardScreen>().open.clone().unwrap();
        assert!(open.answered);
        assert_eq!(open.taken, Some(1), "the card taken stays lit");
    }

    #[test]
    fn a_made_up_screen_asks_nothing_of_the_server() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<Messages<OverlayKeyPressed>>();
        world.init_resource::<Messages<Request>>();
        world.insert_resource(Time::<Real>::default());
        let mut cards = open(3, 1, 0);
        cards.since = -1.0;
        cards.made_up = true;
        world.insert_resource(CardScreen {
            open: Some(cards),
            gold: None,
        });
        world.write_message(OverlayKeyPressed(KeyCode::Digit3));
        world.run_system_once(answer_keys).unwrap();
        assert_eq!(world.resource_mut::<Messages<Request>>().drain().count(), 0);
        let open = world.resource::<CardScreen>().open.clone().unwrap();
        assert_eq!(open.taken, Some(2));
    }
}
