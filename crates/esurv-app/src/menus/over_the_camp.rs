//! The game's own windows over the camp (`crate::camp`): a thing used in the
//! camp, or a camp person's talk option, opens the window the warband screen has for it, over the
//! world, about the hero walking the camp; closing it leaves the hero where it stood. The war
//! table opens the zone selector: the zone column of the warband screen ([`super::warband`]), its
//! zones, run options and Start run, the run then setting off from the camp
//! ([`ToServer::CampRun`]). The lectern, the Chronicler and the merchant open the
//! warband screen's own windows: the codex, the achievements and the shop (the
//! achievements window on its Shop page), as its buttons open them. The weapon rack, the trainer
//! and the Transmutation Orb open the hero sheet ([`super::sheet`]) on its Equipment, Talents and
//! Character views. F1 opens the warband screen's own menu over the camp as a backup: the hero's panel, its row of buttons and the zones, without the camp of heroes.
//! Chromie's "recruit" leaves the world for the class picker: a new hero, as the warband screen's
//! New character, who then walks into the camp.
//!
//! While a window is open over the camp the menus are on the warband screen without its select
//! screen ([`Menus::over_the_camp`]); they forget it when the hero leaves the camp.
//!
//! [`ToServer::CampRun`]: esurv_protocol::ToServer::CampRun

use bevy::prelude::*;

use super::look::{ButtonStyle, MenuLook};
use super::{achievements, codex, guide, warband, Ctx, MenuAction, MenuPress, Menus, Screen, SheetTab};
use crate::camp::CampVisit;
use benilla_app::{OverlayKeyPressed, OverlayKeys};
use crate::protocol::Request;
use esurv_protocol::ToServer;

/// A window open over the camp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CampWindow {
    /// The war table's: the zones, the run options and Start run.
    Zones,
    /// One of the warband screen's own windows (the codex, the guide, the achievements or the
    /// shop), open while it is.
    Own,
    /// The hero sheet about the hero walking the camp, on one of its views, open while it is.
    Sheet,
    /// The warband screen's own menu as a backup, opened by F1: the hero's
    /// panel, the row of buttons and the zones, without the camp of heroes and its New, Modify and
    /// Delete character.
    Menu,
}

/// The holder of F1 while the hero walks the camp ([`OverlayKeys`]).
const KEYS_OWNER: &str = "camp menu";

/// F1 in the camp opens the warband screen's menu over it ([`CampWindow::Menu`]), or closes the
/// window open; it then targets no one. Out of the camp F1 is the game's again.
pub(super) fn f1_opens_the_menu(
    visit: Res<CampVisit>,
    mut menus: ResMut<Menus>,
    mut presses: MessageReader<OverlayKeyPressed>,
    mut keys: ResMut<OverlayKeys>,
) {
    let wanted: &[KeyCode] = if visit.in_camp { &[KeyCode::F1] } else { &[] };
    if keys.of(KEYS_OWNER) != wanted {
        keys.set(KEYS_OWNER, wanted);
    }
    for OverlayKeyPressed(key) in presses.read() {
        if *key != KeyCode::F1 || !visit.in_camp {
            continue;
        }
        if menus.over_the_camp.is_some() {
            menus.over_the_camp = None;
        } else {
            menus.screen = Screen::Warband;
            menus.over_the_camp = Some(CampWindow::Menu);
        }
    }
}

/// The window the server names ([`esurv_protocol::ToClient::CampOpen`]): the zones, or the press
/// that opens one of the warband screen's own.
fn named(name: &str) -> Option<(CampWindow, Option<MenuAction>)> {
    let own = |a| Some((CampWindow::Own, Some(a)));
    match name {
        "zones" => Some((CampWindow::Zones, None)),
        // F1's menu
        "menu" => Some((CampWindow::Menu, None)),
        "codex" => own(MenuAction::OpenCodex),
        "guide" => own(MenuAction::OpenGuide),
        "achievements" => own(MenuAction::OpenAchievements),
        "shop" => own(MenuAction::OpenShop),
        // the weapon rack, the trainer and the orb: the hero sheet's Equipment, Talents and
        // Character (appearance) views
        "equipment" => Some((CampWindow::Sheet, Some(MenuAction::OpenHeroTab(SheetTab::Collection)))),
        "talents" => Some((CampWindow::Sheet, Some(MenuAction::OpenHeroTab(SheetTab::Talents)))),
        "appearance" => Some((CampWindow::Sheet, Some(MenuAction::OpenHeroTab(SheetTab::Character)))),
        _ => None,
    }
}

/// Opens the window the server asked for; out of the camp, or once closed, none stays open.
pub(super) fn open_camp_windows(
    mut visit: ResMut<CampVisit>,
    mut menus: ResMut<Menus>,
    mut presses: MessageWriter<MenuPress>,
    mut requests: MessageWriter<Request>,
    (mut was_open, mut server_opened): (Local<bool>, Local<bool>),
) {
    // a window the server opened over the camp closed: the server is told (Wren waits for it);
    // F1's menu, and what it opens, are the player's own
    let open = visit.in_camp && menus.over_the_camp.is_some();
    if *was_open && !open && visit.in_camp && *server_opened {
        requests.write(Request(ToServer::CampClosed));
    }
    if !open {
        *server_opened = false;
    }
    *was_open = open;
    if !visit.in_camp {
        if menus.over_the_camp.is_some() {
            menus.over_the_camp = None;
        }
        return;
    }
    // closed, by whatever way: its Difficulty modifiers panel with it
    if menus.over_the_camp.is_none() && menus.run_options_open {
        menus.run_options_open = false;
    }
    let sheet_open = matches!(menus.screen, Screen::Sheet(_));
    let own_open = menus.codex.is_some() || menus.guide.is_some() || menus.achievements.is_some();
    // F1's menu opens a window of its own as the warband screen does: that window, open over the
    // camp in its stead
    if menus.over_the_camp == Some(CampWindow::Menu) {
        if sheet_open {
            menus.over_the_camp = Some(CampWindow::Sheet);
        } else if own_open {
            menus.over_the_camp = Some(CampWindow::Own);
        }
    }
    if menus.over_the_camp == Some(CampWindow::Sheet) {
        // the sheet closed (Back, Save changes): back to walking
        if !sheet_open {
            menus.over_the_camp = None;
            requests.write(Request(ToServer::CampDress));
        }
    } else if sheet_open {
        // closed over the camp (Escape, or another window opened): its looks dropped, its gear
        // (kept as it is picked) shown on the hero
        menus.screen = Screen::Warband;
        requests.write(Request(ToServer::CampDress));
    }
    if menus.over_the_camp == Some(CampWindow::Own) {
        // the warband screen's own window closed (its Close): back to walking
        if !own_open {
            menus.over_the_camp = None;
        }
    } else if own_open {
        // closed over the camp (Escape, or another window opened): none of them stays open
        menus.codex = None;
        menus.guide = None;
        menus.achievements = None;
    }
    let Some(name) = visit.open.take() else {
        return;
    };
    if name == "recruit" {
        // a new hero: out of the world to the class picker, as the warband screen's Create
        // (`choose_the_first_screen`); Back there brings the hero who left back in
        info!("camp: leaving the camp to recruit a new hero");
        visit.recruit = true;
        visit.enter_next = visit.hero;
        visit.in_camp = false;
        menus.over_the_camp = None;
        requests.write(Request(ToServer::Leave));
        return;
    }
    match named(&name) {
        Some((window, press)) => {
            menus.screen = Screen::Warband;
            menus.over_the_camp = Some(window);
            *server_opened = true;
            if let Some(press) = press {
                presses.write(MenuPress(press));
            }
        }
        None => info!("camp: the {name} window does not open over the camp yet"),
    }
}

/// The hero walking the camp, as the roster had it on the way in.
pub(super) fn camp_hero(visit: &CampVisit) -> Option<&benilla_protocol::Character> {
    let guid = visit.hero?;
    visit.roster.iter().find(|c| c.guid == guid)
}

/// Draws `window` over the camp, about `hero`.
pub(super) fn draw(
    commands: &mut Commands,
    root: Entity,
    look: &MenuLook,
    ctx: &Ctx,
    menus: &Menus,
    window: CampWindow,
    hero: Option<&benilla_protocol::Character>,
) {
    match window {
        CampWindow::Zones => {
            commands.entity(root).with_children(|r| {
                warband::zone_column(r, look, ctx, hero);
                // the way back to the camp, at the column's foot left of it
                let mut node = look.panel_node();
                node.position_type = PositionType::Absolute;
                node.right = look.px(warband::ZONE_WIDTH + 2.0 * warband::MARGIN);
                node.bottom = look.px(warband::MARGIN);
                r.spawn(look.panel(node)).with_children(|p| {
                    look.button(p, "Close", ButtonStyle::Text, true, None).insert(MenuAction::CloseCampWindow);
                });
            });
            if menus.run_options_open {
                warband::draw_run_options(commands, root, look, ctx);
            }
        }
        CampWindow::Own => {
            if let Some((list, page)) = menus.codex {
                codex::draw(commands, root, look, ctx, list, page);
            }
            if let Some(tab) = menus.guide {
                guide::draw(commands, root, look, ctx, tab);
            }
            if let Some(page) = &menus.achievements {
                achievements::draw(commands, root, look, ctx, page);
            }
        }
        CampWindow::Menu => {
            warband::draw_over_the_camp(commands, root, look, ctx, hero);
            if menus.run_options_open {
                warband::draw_run_options(commands, root, look, ctx);
            }
        }
        // drawn by the menus as the warband screen's sheet ([`super::sheet`])
        CampWindow::Sheet => {}
    }
}
