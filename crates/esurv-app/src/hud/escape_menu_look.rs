//! How the Escape menu looks: in the menus' own look ([`crate::menus::look`];
//! menus.md, "In a run: the Escape menu"), in the stock interface's units: centred on the
//! screen, a near-black panel with the thin gold border, 230 units wide, the heading "Run Paused"
//! in Morpheus small capitals between its gold rules ("Game Menu" before the first step and over
//! the end panel), a hint, and its buttons ([`MenuKind::buttons`]) as the look's framed buttons,
//! Continue the call to act, the ways out (End Run, Quit Game) set apart from the ways on
//! (Continue, Reset Run) and Options. The menu sits over the end panel (which hides under it).
//! Only this file holds the look; the buttons keep their [`MenuButton`] component, which the HUD
//! answers, and Options also the menus' [`MenuAction::OpenGameOptions`], which the menus answer.
//! With the menus off (`ESURV_MENUS=0`, a dev aid) there is no Options panel, and no Options button.

use bevy::prelude::*;
use bevy::text::LineBreak;

use benilla_app::OverlayWindow;

use crate::menus::look::{palette, ButtonStyle, Face, MenuLook};
use crate::menus::MenuAction;
use crate::stock::Stock;
use crate::ui::layer;

use super::escape_menu::{MenuButton, MenuKind};
use super::view::HudRoot;

/// The panel's width, and its buttons'.
const MENU_WIDTH: f32 = 230.0;
const MENU_BUTTON_WIDTH: f32 = 170.0;
/// Between the run's way on (Continue, Reset) and its ways out (End, Quit).
const GROUP_GAP: f32 = 10.0;

/// The menu's parts the HUD changes with the menu showing ([`MenuKind`]).
pub(super) struct MenuParts {
    pub root: Entity,
    /// The line under the buttons, where the Options panel's Okay says what waits for later.
    pub notice: Entity,
    /// Each face's heading and hint, shown with its menu.
    pub faces: Vec<(MenuKind, Entity)>,
    /// Each button's room, shown when its menu has it.
    pub buttons: Vec<(MenuButton, Entity)>,
    /// The End button, whose label is End Run, or Return to main menu over the end panel.
    pub end_button: Entity,
}

/// Builds the menu, hidden: its root, a layer over the HUD and the end panel that centres it, a
/// heading and hint for each of its faces and every button, each shown with the menu that has it
/// (`hud::show_parts`), and its notice line under the buttons (hidden while empty).
pub(super) fn build(commands: &mut Commands, stock: &Stock, look: &MenuLook) -> MenuParts {
    let mut notice = Entity::PLACEHOLDER;
    let mut faces = Vec::new();
    let mut buttons = Vec::new();
    let mut end_button = Entity::PLACEHOLDER;
    let root = commands
        .spawn((
            HudRoot,
            Name::new("hud escape menu"),
            stock.centred(layer::GAME_MENU),
            Visibility::Hidden,
        ))
        .with_children(|layer| {
            let mut node = look.panel_node();
            node.width = look.px(MENU_WIDTH);
            node.align_items = AlignItems::Center;
            node.row_gap = look.px(6.0);
            node.padding = UiRect::new(look.px(16.0), look.px(16.0), look.px(14.0), look.px(18.0));
            layer
                .spawn((OverlayWindow, look.panel(node)))
                .with_children(|panel| {
                    for kind in MenuKind::ALL {
                        let (title, hint) = kind.words();
                        let face = panel
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                row_gap: look.px(6.0),
                                margin: UiRect::bottom(look.px(4.0)),
                                display: Display::None,
                                ..default()
                            })
                            .with_children(|face| {
                                look.heading(face, title, 20.0);
                                face.spawn((
                                    look.hint(hint),
                                    TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                                ));
                            })
                            .id();
                        faces.push((kind, face));
                    }
                    for button in MenuButton::ALL {
                        if button == MenuButton::Options && !crate::menus::menus_on() {
                            continue;
                        }
                        let above = if button == MenuButton::End {
                            GROUP_GAP
                        } else {
                            0.0
                        };
                        let style = if button == MenuButton::Continue {
                            ButtonStyle::Primary
                        } else {
                            ButtonStyle::Text
                        };
                        let room = panel
                            .spawn(Node {
                                margin: UiRect::top(look.px(above)),
                                display: Display::None,
                                ..default()
                            })
                            .with_children(|room| {
                                let mut b = look.button(
                                    room,
                                    button.label(),
                                    style,
                                    true,
                                    Some(MENU_BUTTON_WIDTH),
                                );
                                b.insert(button).insert_if(MenuAction::OpenGameOptions, || {
                                    button == MenuButton::Options
                                });
                                if button == MenuButton::End {
                                    end_button = b.id();
                                }
                            })
                            .id();
                        buttons.push((button, room));
                    }
                    notice = panel
                        .spawn((
                            look.text(Face::Plain, 10.5, palette::HEADING, ""),
                            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                            Node {
                                margin: UiRect::top(look.px(6.0)),
                                max_width: look.px(MENU_BUTTON_WIDTH + 20.0),
                                ..default()
                            },
                            Visibility::Hidden,
                        ))
                        .id();
                });
        })
        .id();
    MenuParts {
        root,
        notice,
        faces,
        buttons,
        end_button,
    }
}
