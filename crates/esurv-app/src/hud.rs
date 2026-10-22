//! The run's HUD (design step 6; docs/game/screens.md, "The HUD"): the
//! run clock, kills, gold and the next cache's price, when the next boss comes (or Endless), the
//! first living boss's health bar, big banners as a boss arrives and as a boss or a rare
//! falls, the server's small HUD messages (toasts), a hint before the first step, and the Escape
//! menu ([`escape_menu`]). The server's `Ready`, `Hud`, `BossBar`, `Banner` and `Toast` messages
//! feed it; it asks `pause`, `resume`, `reset` and `leave`. The server sends the HUD when it
//! changes, at most every 0.2 s; the HUD shows the newest.
//!
//! Every text is ours; the look is the stock 1.12 interface's, from the stock look kit
//! ([`crate::stock`]): the stock fonts and colours and a status bar for the boss; the Escape menu
//! wears the menus' own look ([`crate::menus::look`], `escape_menu_look`).

use bevy::prelude::*;

use benilla_app::{AppExit, BubbleKeepClear, GameMenuAsked, GameMenuStandIn, Objects};
use esurv_protocol::{Phase, ToClient, ToServer};

use crate::menus::{MenuAction, MenuPress, Menus};
use crate::protocol::{ClientScreens, Request, ServerEvent};
use crate::stock::{StatusBar, Stock, StockClick, StockSystems};
use crate::ui::{in_world, UiFonts};

pub mod escape_menu;
mod escape_menu_look;
mod escape_presses;
mod lines;
mod menu_presses;
pub mod state;
mod view;

pub(crate) use escape_presses::escape_key;
pub use lines::{strength, LineLook, TimedLine, TimedLines};
pub use escape_menu::MenuHere;
pub use state::RunHud;

use view::{HudParts, HudRoot};

/// Banners hold this long, then fade over `BANNER_FADE`: the stock `RaidWarningFrame`'s timing
/// (a message frame's defaults, 10 s and 3 s).
const BANNER_HOLD: f32 = 10.0;
const BANNER_FADE: f32 = 3.0;
/// Toasts likewise: the stock `UIErrorsFrame`'s timing (`displayDuration` 5, the default fade).
const TOAST_HOLD: f32 = 5.0;
const TOAST_FADE: f32 = 3.0;
/// The start hint's pulse: its strength is |sin(t × 4.5)| × 0.45 + 0.55.
const HINT_PULSE_RATE: f32 = 4.5;
const HINT_PULSE_DEPTH: f32 = 0.45;

/// The HUD: its state, its parts, and the Escape menu standing in for the game menu.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientScreens>();
        app.world_mut().resource_mut::<ClientScreens>().0.hud = true;
        app.init_resource::<RunHud>()
            .init_resource::<HudLines>()
            .init_resource::<MenuState>()
            .init_resource::<MenuHere>()
            .init_resource::<GameMenuUp>()
            .add_systems(
                Update,
                (
                    build_for_the_window,
                    read_server,
                    follow_the_world,
                    stand_in_for_the_game_menu,
                    answer_the_menu,
                    press_menu_buttons,
                    quit_once_out,
                    refresh_readouts,
                    refresh_lines,
                    show_parts,
                    say_what_options_left_for_later,
                    keep_bubbles_off_the_readouts,
                )
                    .chain()
                    .in_set(HudSystems)
                    .before(StockSystems),
            );
        escape_presses::add(app);
        menu_presses::add(app);
    }
}

/// Whether the run's Escape menu, or the Options panel opened from it, is up: the end panel
/// hides under them.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameMenuUp(pub bool);

/// The HUD's systems, in order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HudSystems;

/// The banners and toasts up, on the HUD's own clock (seconds since the game started).
#[derive(Resource)]
pub struct HudLines {
    pub banners: TimedLines,
    pub toasts: TimedLines,
}

impl Default for HudLines {
    fn default() -> Self {
        Self {
            banners: TimedLines::new(view::BANNER_SLOTS, BANNER_HOLD, BANNER_FADE),
            toasts: TimedLines::new(view::TOAST_SLOTS, TOAST_HOLD, TOAST_FADE),
        }
    }
}

impl HudLines {
    fn clear(&mut self) {
        self.banners.clear();
        self.toasts.clear();
    }
}

/// The Escape menu's ask on its way, and whether the game closes once the hero is out.
#[derive(Resource, Default)]
struct MenuState {
    pending: escape_menu::PendingAsk,
    quitting: bool,
}

/// Builds the HUD at the kit's scale, and again when the scale changes (the window's height).
fn build_for_the_window(
    mut commands: Commands,
    stock: Stock,
    fonts: Res<UiFonts>,
    assets: Res<AssetServer>,
    parts: Option<Res<HudParts>>,
    mut run: ResMut<RunHud>,
    look: crate::menus::look::MenuLook,
) {
    if let Some(parts) = &parts {
        if parts.scale == stock.scale() {
            return;
        }
        for &root in &parts.roots {
            commands.entity(root).despawn();
        }
    }
    let parts = view::build(&mut commands, &stock, &fonts, &assets, &look);
    commands.insert_resource(parts);
    // the new parts are filled at once
    run.set_changed();
}

/// The server's messages: the run's state, its banners and toasts, and the answers to the menu.
fn read_server(
    mut events: MessageReader<ServerEvent>,
    mut run: ResMut<RunHud>,
    mut lines: ResMut<HudLines>,
    mut menu: ResMut<MenuState>,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time>,
    mut here: ResMut<MenuHere>,
) {
    let now = time.elapsed_secs();
    let mut refused = None;
    for ServerEvent(m) in events.read() {
        if run.bypass_change_detection().apply(m) {
            run.set_changed();
        }
        match m {
            ToClient::Ready(_) => {
                lines.clear();
                menu.pending.clear();
                here.set_if_neq(MenuHere(None));
            }
            ToClient::Banner(b) => {
                let (text, look) = state::banner_line(b);
                lines.banners.push(text, look, now);
            }
            ToClient::Toast(text) => lines.toasts.push(text.clone(), LineLook::Toast, now),
            ToClient::Refused(verb) => {
                // leaving was refused (the hero was out already): quitting goes on regardless
                if menu.quitting && verb == ToServer::Leave.verb() {
                    exit.write(AppExit::Success);
                }
                if verb == ToServer::Reset.verb() {
                    let text = state::RESET_REFUSED.to_owned();
                    lines.toasts.push(text, LineLook::Toast, now);
                }
                refused = Some(verb.clone());
            }
            _ => {}
        }
    }
    let phase = run.phase();
    menu.pending.answered(phase, refused.as_deref());
    // a menu opened here goes once the phase moves on (a step taken under it, Endless)
    if here.0.is_some() && here.0 != phase {
        here.set_if_neq(MenuHere(None));
    }
}

/// The hero left the world: the run is forgotten.
fn follow_the_world(
    objects: Objects,
    mut was_in: Local<bool>,
    mut run: ResMut<RunHud>,
    mut lines: ResMut<HudLines>,
    mut menu: ResMut<MenuState>,
    mut here: ResMut<MenuHere>,
) {
    let now_in = in_world(&objects);
    if *was_in && !now_in {
        run.clear();
        lines.clear();
        menu.pending.clear();
        here.set_if_neq(MenuHere(None));
    }
    *was_in = now_in;
}

/// The Escape menu is the game menu whenever a run is up, but over a choice screen.
fn stand_in_for_the_game_menu(
    objects: Objects,
    run: Res<RunHud>,
    mut stand_in: ResMut<GameMenuStandIn>,
) {
    let on = in_world(&objects) && escape_menu::stands_in_for_the_game_menu(run.phase());
    stand_in.set_if_neq(GameMenuStandIn(on));
}

/// The game menu was asked for (Escape, the menu button): pause or resume a run under way; before
/// the first step and on the end panel, open or close the menu here; with the Options panel open
/// over the menu, it closes the panel instead (its Cancel), back to the menu, as the stock options
/// windows go back to the game menu ([`escape_menu::answer_escape`]).
fn answer_the_menu(
    mut asked: MessageReader<GameMenuAsked>,
    run: Res<RunHud>,
    mut menu: ResMut<MenuState>,
    mut here: ResMut<MenuHere>,
    mut requests: MessageWriter<Request>,
    (menus, mut presses): (Option<Res<Menus>>, MessageWriter<MenuPress>),
) {
    for _ in asked.read() {
        let options = menus.as_deref().is_some_and(Menus::options_over_the_run);
        let phase = run.phase();
        match escape_menu::answer_escape(phase, *here, options) {
            escape_menu::EscapeAnswer::CloseOptions => {
                info!("hud: the game menu was asked for over the Options panel; it closes");
                presses.write(MenuPress(MenuAction::OptionsCancel));
            }
            escape_menu::EscapeAnswer::Ask(r) => {
                if let Some(r) = menu.pending.ask(phase, Some(r)) {
                    info!(
                        "hud: the game menu was asked for; asking the server to {}",
                        r.verb()
                    );
                    requests.write(Request(r));
                }
            }
            escape_menu::EscapeAnswer::OpenHere(p) => {
                info!("hud: the game menu opens ({p:?}: no run to pause)");
                here.0 = Some(p);
            }
            escape_menu::EscapeAnswer::CloseHere => {
                info!("hud: the game menu closes");
                here.0 = None;
            }
            escape_menu::EscapeAnswer::Nothing => {}
        }
    }
}

/// The Escape menu's buttons, one ask at a time, each as the menu showing has it.
fn press_menu_buttons(
    mut clicks: MessageReader<StockClick>,
    buttons: Query<&escape_menu::MenuButton>,
    run: Res<RunHud>,
    mut menu: ResMut<MenuState>,
    mut here: ResMut<MenuHere>,
    mut requests: MessageWriter<Request>,
) {
    for StockClick(clicked) in clicks.read() {
        let Ok(&button) = buttons.get(*clicked) else {
            continue;
        };
        let Some(kind) = escape_menu::menu_kind(run.phase(), *here) else {
            continue;
        };
        // Options asks nothing of the server: the menus open their panel (its `MenuAction`)
        let request = match button.press(kind) {
            escape_menu::Press::Ask(r) => Some(r),
            escape_menu::Press::CloseHere => {
                info!("hud: {} pressed; the game menu closes", button.word());
                here.0 = None;
                None
            }
            escape_menu::Press::Nothing => None,
        };
        if let Some(r) = menu.pending.ask(run.phase(), request) {
            info!(
                "hud: {} pressed; asking the server to {}",
                button.word(),
                r.verb()
            );
            menu.quitting |= button.closes_the_game();
            requests.write(Request(r));
        }
    }
}

/// Quit: the game closes once the run has let the hero go (paid out and saved).
fn quit_once_out(objects: Objects, menu: Res<MenuState>, mut exit: MessageWriter<AppExit>) {
    if menu.quitting && !in_world(&objects) {
        exit.write(AppExit::Success);
    }
}

/// The readouts and the boss bar, when the run's state changed.
fn refresh_readouts(
    run: Res<RunHud>,
    parts: Option<Res<HudParts>>,
    mut texts: Query<&mut Text>,
    mut bars: Query<&mut StatusBar>,
) {
    let Some(parts) = parts else {
        return;
    };
    if !run.is_changed() && !parts.is_changed() {
        return;
    }
    if let Some(h) = &run.numbers {
        set_text(&mut texts, parts.clock, &state::clock_text(h.clock));
        let next = state::next_boss_text(h.endless, h.next_boss).unwrap_or_default();
        set_text(&mut texts, parts.next_boss, &next);
        set_text(&mut texts, parts.kills, &h.kills.to_string());
        set_text(&mut texts, parts.gold, &h.gold.to_string());
        set_text(&mut texts, parts.cache, &state::cache_text(h.cache_price));
    }
    if let Some(b) = run.boss() {
        set_text(&mut texts, parts.boss_name, &b.name);
        set_text(
            &mut texts,
            parts.boss_health,
            &state::health_text(b.health, b.max_health),
        );
        if let Ok(mut bar) = bars.get_mut(parts.boss_bar) {
            let value = state::health_fraction(b.health, b.max_health);
            if bar.value != value {
                bar.value = value;
            }
        }
    }
}

/// The banners, toasts and start hint, every frame (they fade).
fn refresh_lines(
    time: Res<Time>,
    run: Res<RunHud>,
    mut lines: ResMut<HudLines>,
    parts: Option<Res<HudParts>>,
    mut texts: Query<(&mut Text, &mut TextColor, &mut Visibility)>,
) {
    let Some(parts) = parts else {
        return;
    };
    let now = time.elapsed_secs();
    if !lines.banners.is_empty() || !lines.toasts.is_empty() {
        lines.banners.prune(now);
        lines.toasts.prune(now);
    }
    for (slots, shown) in [
        (&parts.banners[..], lines.banners.shown(now)),
        (&parts.toasts[..], lines.toasts.shown(now)),
    ] {
        for (i, &slot) in slots.iter().enumerate() {
            let Ok((mut text, mut colour, mut visible)) = texts.get_mut(slot) else {
                continue;
            };
            match shown.get(i) {
                Some((line, strength)) => {
                    if text.0 != line.text {
                        text.0.clone_from(&line.text);
                    }
                    colour.0 = view::line_colour(line.look).with_alpha(*strength);
                    visible.set_if_neq(Visibility::Inherited);
                }
                None => {
                    visible.set_if_neq(Visibility::Hidden);
                }
            }
        }
    }
    if let Ok((mut text, mut colour, mut visible)) = texts.get_mut(parts.hint) {
        if run.phase() == Some(Phase::Waiting) {
            if text.0 != state::START_HINT {
                text.0 = state::START_HINT.into();
            }
            let pulse =
                (now * HINT_PULSE_RATE).sin().abs() * HINT_PULSE_DEPTH + (1.0 - HINT_PULSE_DEPTH);
            colour.0 = colour.0.with_alpha(pulse);
            visible.set_if_neq(Visibility::Inherited);
        } else {
            visible.set_if_neq(Visibility::Hidden);
        }
    }
}

/// Which parts show: everything only while the hero is in the world and the server speaks to
/// the HUD; the boss bar while a boss is up; the Escape menu while the run is paused or the menu
/// was opened here (with the face and buttons of its kind), but not under the Options panel (or
/// the key bindings it opened), as the stock game menu hides under its options windows.
#[allow(clippy::too_many_arguments)] // one Bevy system's inputs
fn show_parts(
    objects: Objects,
    run: Res<RunHud>,
    here: Res<MenuHere>,
    parts: Option<Res<HudParts>>,
    menus: Option<Res<Menus>>,
    mut roots: Query<&mut Visibility, With<HudRoot>>,
    mut nodes: Query<&mut Node>,
    (children, mut texts, mut up): (Query<&Children>, Query<&mut Text>, ResMut<GameMenuUp>),
) {
    let Some(parts) = parts else {
        return;
    };
    let live = in_world(&objects) && run.ready.is_some();
    let numbers = live && run.numbers.is_some();
    let options = menus.as_deref().is_some_and(Menus::options_over_the_run);
    let kind = escape_menu::menu_kind(run.phase(), *here).filter(|_| live);
    up.set_if_neq(GameMenuUp(kind.is_some() || (live && options)));
    if let Some(kind) = kind {
        let mut display = |entity: Entity, on: bool| {
            if let Ok(mut node) = nodes.get_mut(entity) {
                let want = if on { Display::Flex } else { Display::None };
                if node.display != want {
                    node.display = want;
                }
            }
        };
        for &(face, entity) in &parts.menu_faces {
            display(entity, face == kind);
        }
        for &(button, entity) in &parts.menu_buttons {
            display(entity, kind.buttons().contains(&button));
        }
        let label = escape_menu::MenuButton::End.label_on(kind);
        for &child in children.get(parts.menu_end_button).into_iter().flatten() {
            if let Ok(mut text) = texts.get_mut(child) {
                if text.0 != label {
                    text.0 = label.to_owned();
                }
            }
        }
    }
    for (root, on) in [
        (parts.readouts, numbers),
        (parts.boss, numbers && run.boss().is_some()),
        (parts.menu, kind.is_some() && !options),
    ] {
        if let Ok(mut v) = roots.get_mut(root) {
            v.set_if_neq(if on {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
    }
    for &root in &parts.roots {
        if [parts.readouts, parts.boss, parts.menu].contains(&root) {
            continue;
        }
        if let Ok(mut v) = roots.get_mut(root) {
            v.set_if_neq(if live {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
    }
}

/// The Escape menu's line under its buttons: what the Options panel's Okay left for later (a
/// restart, the next time in the world), while the menus keep it.
fn say_what_options_left_for_later(
    parts: Option<Res<HudParts>>,
    menus: Option<Res<Menus>>,
    mut texts: Query<(&mut Text, &mut Visibility)>,
) {
    let Some(parts) = parts else {
        return;
    };
    let notice = menus.as_deref().and_then(|m| m.run_notice.as_deref());
    let Ok((mut text, mut visible)) = texts.get_mut(parts.menu_notice) else {
        return;
    };
    match notice {
        Some(n) => {
            if text.0 != n {
                text.0 = n.to_owned();
            }
            visible.set_if_neq(Visibility::Inherited);
        }
        None => {
            visible.set_if_neq(Visibility::Hidden);
        }
    }
}

/// Room left round a readout a chat bubble keeps out of, in logical pixels.
const BUBBLE_ROOM: f32 = 2.0;

/// Keeps chat bubbles off the readouts at the top centre (the clock, kills, gold, next boss) and
/// the boss bar under them while they show: a boss's shout over its head can rise there, and the
/// clock is to stay readable. benilla's [`BubbleKeepClear`] moves such a
/// bubble down under them.
fn keep_bubbles_off_the_readouts(
    parts: Option<Res<HudParts>>,
    roots: Query<(&Visibility, &Children), With<HudRoot>>,
    nodes: Query<(&bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform)>,
    keep_clear: Option<ResMut<BubbleKeepClear>>,
) {
    let Some(mut keep_clear) = keep_clear else {
        return;
    };
    let mut areas = Vec::new();
    if let Some(parts) = parts {
        // the readouts first, then the boss bar under them: a bubble moved under the one goes on
        // under the other
        for root in [parts.readouts, parts.boss] {
            let Ok((shown, children)) = roots.get(root) else {
                continue;
            };
            if *shown == Visibility::Hidden {
                continue;
            }
            let mut area: Option<Rect> = None;
            for child in children {
                let Ok((node, at)) = nodes.get(*child) else {
                    continue;
                };
                let scale = node.inverse_scale_factor();
                let size = node.size() * scale;
                if size.x <= 0.0 || size.y <= 0.0 {
                    continue;
                }
                let rect = Rect::from_center_size(at.translation * scale, size);
                area = Some(area.map_or(rect, |a| a.union(rect)));
            }
            if let Some(a) = area {
                areas.push(a.inflate(BUBBLE_ROOM));
            }
        }
    }
    if keep_clear.0 != areas {
        keep_clear.0 = areas;
    }
}

fn set_text(texts: &mut Query<&mut Text>, entity: Entity, value: &str) {
    if let Ok(mut text) = texts.get_mut(entity) {
        if text.0 != value {
            text.0 = value.to_owned();
        }
    }
}
