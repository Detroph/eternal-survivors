//! The end panel, in the menus' own look ([`crate::menus::look`]; it fits
//! the warband screen, the hero sheet and the Escape menu) in the stock interface's units: one
//! near-black panel with the thin gold border, the title ("Triumph" in the headings' gold,
//! "Overrun" in red) in Morpheus small capitals between two gold rules, the subtitle under it, then
//! the tally (left) and the talent experience being revealed (right) under small all-caps grey
//! labels, the zone opened, the gear won and the look's framed buttons (Run Again the gold-framed
//! call to act). The title and subtitle move the panel. It hides while the run's Escape menu (or
//! the Options panel opened from it) is up over it.

use bevy::prelude::*;
use bevy::text::LineBreak;
use bevy::ui::ComputedNode;
use bevy::window::PrimaryWindow;

use benilla_app::{Objects, OverlayWindow};
use benilla_ui::script::UiScript;
use esurv_content::Content;
use esurv_protocol::Ending;

use crate::hud::GameMenuUp;
use crate::menus::look::{self, palette, Arrow, ButtonStyle, Face, MenuButton, MenuLook};
use crate::protocol::{Request, ServerEvent};
use crate::stock::{StockButton, StockClick, StockScale, StockSystems};
use crate::ui::{in_world, layer, rarity_colour, GameContent};

use super::reveal::{Reveal, RevealFrame};
use super::state::{EndButton, EndOfRun, RunEndState};
use super::tally::{self, LootRow};

/// Sizes, in stock units.
const PANEL_WIDTH: f32 = 480.0;
const PANEL_TOP: f32 = 80.0;
/// The panel's room inside its border, left and right.
const PANEL_PAD: f32 = 20.0;
const TALLY_WIDTH: f32 = 196.0;
const COLUMN_GAP: f32 = 22.0;
const ROW_HEIGHT: f32 = 15.0;
const BAR_HEIGHT: f32 = 14.0;
const BUTTON_WIDTH: f32 = 124.0;
/// Return to main menu, the longest label.
const MAIN_MENU_WIDTH: f32 = 150.0;
const SLOT_WIDTH: f32 = 96.0;
const STATUS_WIDTH: f32 = 90.0;
const LOOT_ROWS: usize = 6;
/// The title, in Morpheus small capitals: the menus' headings, larger.
const TITLE_SIZE: f32 = 34.0;

/// A fall's title is red (the menus' red), a victory's the headings' gold.
const FALLEN_TEXT: Color = palette::RED;
/// The experience bar's purple, as the hero sheet's.
const EXPERIENCE_BAR: Color = Color::srgb(0.58, 0.0, 0.55);
/// How long a talent point's line stays up after it is crossed.
const POINT_POP_TIME: f32 = 1.6;

/// The panel's systems, in order; they run before the kit's.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunEndSystems;

/// The panel's root node.
#[derive(Component)]
struct RunEndPanel;

/// What dragging the panel holds on to.
#[derive(Component)]
struct DragHandle;

/// The parts the reveal changes, found again by entity.
struct RevealParts {
    reveal: Reveal,
    part_rows: Vec<Entity>,
    counter: Entity,
    bar: Entity,
    bar_text: Entity,
    points: Entity,
    point_pop: Entity,
}

/// The panel as drawn.
#[derive(Resource, Default)]
struct PanelView {
    /// The panel drawn (its number from [`RunEndState::opened`]) and at what scale (the menus'
    /// look's, the kit's).
    drawn: Option<(u32, StockScale)>,
    root: Option<Entity>,
    reveal: Option<RevealParts>,
    /// When the shown panel opened (the reveal's clock).
    opened_at: f32,
    /// Where the player moved the panel, from its default place (kept between sessions:
    /// [`super::place`]).
    moved: Vec2,
    /// A drag under way: the cursor's offset from the panel's corner.
    grab: Option<Vec2>,
    /// The panel's corner as drawn.
    shown_at: Vec2,
    /// Talent points gained as last shown, and when the last one popped.
    gained: u32,
    popped_at: Option<f32>,
}

/// The dev switch `ESURV_END_PRESS`: the button an unattended run presses on each panel.
#[derive(Resource)]
struct UnattendedPress(Option<EndButton>);

/// Reads `ESURV_END_PRESS` (`again`, `endless` or `leave`).
fn unattended_press(value: Option<&str>) -> Option<EndButton> {
    match value? {
        "again" => Some(EndButton::Again),
        "endless" => Some(EndButton::KeepFighting),
        "leave" => Some(EndButton::MainMenu),
        _ => None,
    }
}

/// An unattended run waits this long after the reveal before it presses.
const UNATTENDED_WAIT: f32 = 2.0;

pub(super) fn add_panel(app: &mut App) {
    let press = unattended_press(std::env::var("ESURV_END_PRESS").ok().as_deref());
    if let Some(b) = press {
        info!("run end: an unattended run presses {b:?} (ESURV_END_PRESS)");
    }
    app.insert_resource(UnattendedPress(press));
    let path = super::place::place_path();
    let kept = path
        .as_deref()
        .map(super::place::PanelPlace::load)
        .unwrap_or_default();
    app.insert_resource(PanelView {
        moved: kept.moved(),
        ..default()
    })
    .insert_resource(PlaceFile(path))
    .add_systems(
        Update,
        (
            read_messages,
            press_buttons,
            press_unattended,
            draw_panel,
            hide_under_the_game_menu,
            hide_the_death_popup,
            drag_panel,
            place_panel,
            run_reveal,
            enable_buttons,
        )
            .chain()
            .in_set(RunEndSystems)
            .before(StockSystems),
    );
}

/// The server's messages into the state; out of the world nothing is kept.
fn read_messages(
    mut events: MessageReader<ServerEvent>,
    mut state: ResMut<RunEndState>,
    objects: Objects,
) {
    for ServerEvent(m) in events.read() {
        state.take(m);
    }
    if !in_world(&objects) && state.shown().is_some() {
        state.forget();
    }
}

/// A button clicked asks the server.
fn press_buttons(
    mut clicks: MessageReader<StockClick>,
    buttons: Query<&EndButton>,
    mut state: ResMut<RunEndState>,
    mut requests: MessageWriter<Request>,
) {
    for click in clicks.read() {
        if let Ok(button) = buttons.get(click.0) {
            if let Some(r) = state.press(*button) {
                requests.write(Request(r));
            }
        }
    }
}

/// With the dev switch on, each panel's button is pressed once its reveal is over: a live check
/// with no hand on the mouse.
fn press_unattended(
    press: Res<UnattendedPress>,
    time: Res<Time>,
    view: Res<PanelView>,
    mut state: ResMut<RunEndState>,
    mut requests: MessageWriter<Request>,
    mut pressed: Local<Option<u32>>,
) {
    let (Some(button), Some((opened, _)), Some(parts)) = (press.0, view.drawn, &view.reveal) else {
        return;
    };
    if *pressed == Some(opened)
        || time.elapsed_secs() - view.opened_at < parts.reveal.length() + UNATTENDED_WAIT
    {
        return;
    }
    *pressed = Some(opened);
    if let Some(r) = state.press(button) {
        info!("run end: pressing {button:?} unattended");
        requests.write(Request(r));
    }
}

/// Hides the stock death popup (`StaticPopup` "DEATH": release to the graveyard), with no answer
/// to the server.
const HIDE_DEATH_POPUP: &str =
    "if StaticPopup_Visible and StaticPopup_Visible(\"DEATH\") then StaticPopup_Hide(\"DEATH\") end";

/// While the panel shows, the stock death popup stays hidden: a run has no graveyard to release
/// to, and the panel takes the middle of the screen.
fn hide_the_death_popup(
    state: Res<RunEndState>,
    script: Option<NonSend<UiScript>>,
    mut warned: Local<bool>,
) {
    let (Some(_), Some(script)) = (state.shown(), script) else {
        return;
    };
    if let Err(e) = script.run(HIDE_DEATH_POPUP) {
        if !std::mem::replace(&mut *warned, true) {
            warn!("run end: could not hide the stock death popup: {e}");
        }
    }
}

/// Draws the panel when one opens (or the stock scale changes), and takes it away when it closes.
fn draw_panel(
    mut commands: Commands,
    state: Res<RunEndState>,
    mut view: ResMut<PanelView>,
    look: MenuLook,
    content: Option<Res<GameContent>>,
    time: Res<Time>,
) {
    let Some(shown) = state.shown() else {
        if let Some(root) = view.root.take() {
            commands.entity(root).despawn();
        }
        view.drawn = None;
        view.reveal = None;
        return;
    };
    let scale = look.scale();
    let fresh = view.drawn.is_none_or(|(n, _)| n != state.opened());
    let rescaled = view.drawn.is_some_and(|(_, s)| s != scale);
    if !fresh && !rescaled {
        return;
    }
    if let Some(root) = view.root.take() {
        commands.entity(root).despawn();
    }
    if fresh {
        view.opened_at = time.elapsed_secs();
        view.gained = 0;
        view.popped_at = None;
    }
    let content = content.as_ref().map(|c| &*c.0);
    let (root, reveal) = spawn_panel(&mut commands, &look, shown, content);
    view.root = Some(root);
    view.reveal = Some(reveal);
    view.drawn = Some((state.opened(), scale));
}

/// While the run's Escape menu or the Options panel opened from it is up, the panel hides under
/// it (and the stock key binding window opened from the Options panel shows clear of it).
fn hide_under_the_game_menu(
    up: Option<Res<GameMenuUp>>,
    mut roots: Query<&mut Visibility, With<RunEndPanel>>,
) {
    let hidden = up.is_some_and(|u| u.0);
    for mut v in &mut roots {
        v.set_if_neq(if hidden {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
}

/// A press on the title or the subtitle starts a drag; the panel follows the cursor while the
/// button is held. Where it is let go is kept for the next session.
fn drag_panel(
    window: Single<&Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    presses: Query<&Interaction, (With<DragHandle>, Changed<Interaction>)>,
    scale: Res<StockScale>,
    mut view: ResMut<PanelView>,
    file: Res<PlaceFile>,
) {
    let cursor = window.cursor_position();
    if presses.iter().any(|i| *i == Interaction::Pressed) {
        if let Some(cursor) = cursor {
            view.grab = Some(cursor - view.shown_at);
        }
    }
    let Some(grab) = view.grab else {
        return;
    };
    if !mouse.pressed(MouseButton::Left) {
        view.grab = None;
        keep_the_place(view.moved, &file);
        return;
    }
    if let Some(cursor) = cursor {
        view.moved = cursor - grab - default_place(window.width(), *scale);
    }
}

/// Where the panel's place is kept (`end_panel.ron`); `None` keeps it for the session only.
#[derive(Resource)]
struct PlaceFile(Option<std::path::PathBuf>);

/// Writes the panel's place to its file.
fn keep_the_place(moved: Vec2, file: &PlaceFile) {
    let Some(path) = &file.0 else {
        return;
    };
    let place = super::place::PanelPlace {
        moved: (moved.x, moved.y),
    };
    if let Err(e) = place.save(path) {
        warn!(
            "run end: could not keep the panel's place in {}: {e}",
            path.display()
        );
    }
}

/// The panel's default corner: centred, near the top.
fn default_place(window_width: f32, scale: StockScale) -> Vec2 {
    Vec2::new(
        ((window_width - scale.of(PANEL_WIDTH)) / 2.0).max(0.0),
        scale.of(PANEL_TOP),
    )
}

/// Puts the panel where it was moved to, kept on screen.
fn place_panel(
    window: Single<&Window, With<PrimaryWindow>>,
    scale: Res<StockScale>,
    mut view: ResMut<PanelView>,
    mut roots: Query<(&mut Node, &ComputedNode), With<RunEndPanel>>,
) {
    let wanted = default_place(window.width(), *scale) + view.moved;
    for (mut node, laid_out) in &mut roots {
        let size = laid_out.size() * laid_out.inverse_scale_factor();
        let room = (window.size() - size).max(Vec2::ZERO);
        let at = wanted.clamp(Vec2::ZERO, room);
        view.shown_at = at;
        let (left, top) = (Val::Px(at.x), Val::Px(at.y));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
    }
}

/// The talent experience's reveal, frame by frame.
#[allow(clippy::too_many_arguments)] // one Bevy system's inputs
fn run_reveal(
    time: Res<Time>,
    mut view: ResMut<PanelView>,
    mut shown: Query<&mut Visibility>,
    mut texts: Query<&mut Text>,
    mut colours: Query<&mut TextColor>,
    mut shadows: Query<&mut TextShadow>,
    mut nodes: Query<&mut Node>,
) {
    let now = time.elapsed_secs();
    let since = now - view.opened_at;
    let Some(parts) = &view.reveal else {
        return;
    };
    let frame: RevealFrame = parts.reveal.at(since);
    for (i, &row) in parts.part_rows.iter().enumerate() {
        if let Ok(mut v) = shown.get_mut(row) {
            v.set_if_neq(if i < frame.parts_shown {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
    }
    set_text(
        &mut texts,
        parts.counter,
        &format!("+{}", tally::whole_number(frame.counted.into())),
    );
    set_text(&mut texts, parts.bar_text, &bar_text(&frame));
    set_text(&mut texts, parts.points, &points_text(&frame));
    if let Ok(mut bar) = nodes.get_mut(parts.bar) {
        let fill = Val::Percent(frame.standing.fill() * 100.0);
        if bar.width != fill {
            bar.width = fill;
        }
    }
    let pop = parts.point_pop;
    if frame.points_gained > view.gained {
        view.gained = frame.points_gained;
        view.popped_at = Some(now);
    }
    let alpha = point_pop_alpha(view.popped_at.map(|at| now - at));
    if let Ok(mut v) = shown.get_mut(pop) {
        v.set_if_neq(if alpha > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    if let Ok(mut c) = colours.get_mut(pop) {
        let want = palette::GREEN.with_alpha(alpha);
        if c.0 != want {
            c.0 = want;
        }
    }
    if let Ok(mut s) = shadows.get_mut(pop) {
        let want = crate::ui::TEXT_SHADOW.with_alpha(crate::ui::TEXT_SHADOW.alpha() * alpha);
        if s.color != want {
            s.color = want;
        }
    }
}

/// How visible a talent point's line is, `since` seconds after it was crossed: up at once, then
/// fading over the last half of its time; not there before any point or once its time is over.
fn point_pop_alpha(since: Option<f32>) -> f32 {
    let Some(since) = since else {
        return 0.0;
    };
    let t = since / POINT_POP_TIME;
    if !(0.0..1.0).contains(&t) {
        0.0
    } else {
        (2.0 * (1.0 - t)).min(1.0)
    }
}

/// The bar's own text: the way to the next point, or that every point is earned.
fn bar_text(frame: &RevealFrame) -> String {
    let s = frame.standing;
    if s.next_cost == 0 {
        "Every talent point earned".into()
    } else {
        format!("{} / {}", s.into_next, s.next_cost)
    }
}

/// The talent points the class has with what is counted, and how many the run brought so far.
fn points_text(frame: &RevealFrame) -> String {
    if frame.points_gained > 0 {
        format!(
            "Talent points: {} (+{})",
            frame.standing.points, frame.points_gained
        )
    } else {
        format!("Talent points: {}", frame.standing.points)
    }
}

/// The buttons work unless an answer is awaited (a disabled one is dimmed, its label grey).
fn enable_buttons(
    state: Res<RunEndState>,
    mut buttons: Query<(&mut StockButton, &mut MenuButton, &Children), With<EndButton>>,
    mut labels: Query<&mut TextColor>,
) {
    let enabled = state.pending().is_none();
    for (mut stock, mut b, children) in &mut buttons {
        if b.enabled == enabled && stock.enabled == enabled {
            continue;
        }
        stock.enabled = enabled;
        b.enabled = enabled;
        let colour = look::label_colour(b.style, enabled);
        for &child in children {
            if let Ok(mut c) = labels.get_mut(child) {
                c.0 = colour;
            }
        }
    }
}

fn set_text(texts: &mut Query<&mut Text>, entity: Entity, value: &str) {
    if let Ok(mut t) = texts.get_mut(entity) {
        if t.0 != value {
            t.0 = value.into();
        }
    }
}

/// A line of text in the menus' look that never wraps.
fn line(
    look: &MenuLook,
    face: Face,
    size: f32,
    colour: Color,
    value: impl Into<String>,
) -> impl Bundle {
    (
        look.text(face, size, colour, value),
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
    )
}

/// A row with a word on the left and a value on the right.
fn row(look: &MenuLook) -> Node {
    Node {
        width: Val::Percent(100.0),
        height: look.px(ROW_HEIGHT),
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        flex_shrink: 0.0,
        ..default()
    }
}

/// A column's small all-caps grey heading, with room under it.
fn column_label(parent: &mut ChildSpawnerCommands, look: &MenuLook, value: &str) {
    parent.spawn((
        look.label(value),
        Node {
            margin: UiRect::bottom(look.px(4.0)),
            ..default()
        },
    ));
}

/// The title's colour: a victory's gold heading, a fall's red.
fn title_colour(ending: Ending) -> Color {
    if ending == Ending::Victory {
        palette::HEADING
    } else {
        FALLEN_TEXT
    }
}

/// Builds the panel for `shown`; returns its root and the parts the reveal changes.
fn spawn_panel(
    commands: &mut Commands,
    look: &MenuLook,
    shown: &EndOfRun,
    content: Option<&Content>,
) -> (Entity, RevealParts) {
    let end = &shown.end;
    let px = |u| look.px(u);
    let reveal = Reveal::new(end.experience_before, &end.experience);
    let start = reveal.at(0.0);
    let mut part_rows = Vec::new();
    let (mut counter, mut bar, mut bar_text_entity, mut points, mut point_pop) = (
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
    );
    let loot = tally::loot_rows(&shown.loot, content);
    let mut node = look.panel_node();
    node.position_type = PositionType::Absolute;
    node.width = px(PANEL_WIDTH);
    node.padding = UiRect::new(px(PANEL_PAD), px(PANEL_PAD), px(14.0), px(16.0));
    let root = commands
        .spawn((
            RunEndPanel,
            OverlayWindow,
            Name::new("run end panel"),
            look.panel(node),
            GlobalZIndex(layer::RUN_END),
        ))
        .with_children(|panel| {
            // the title between its gold rules, and the subtitle; they also move the panel
            panel
                .spawn((
                    Button,
                    DragHandle,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(4.0),
                        ..default()
                    },
                ))
                .with_children(|top| {
                    top.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: px(10.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|heading| {
                        heading.spawn(look.rule(Arrow::Right));
                        look.small_caps(
                            heading,
                            tally::title(end.ending),
                            TITLE_SIZE,
                            title_colour(end.ending),
                        );
                        heading.spawn(look.rule(Arrow::Left));
                    });
                    if let Some(who) = &shown.who {
                        top.spawn((
                            line(look, Face::Plain, 11.0, palette::VALUE, tally::subtitle(who)),
                            Pickable::IGNORE,
                        ));
                    }
                });
            panel.spawn(look.divider());
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(COLUMN_GAP),
                    margin: UiRect::top(px(2.0)),
                    ..default()
                })
                .with_children(|columns| {
                    // the tally
                    columns
                        .spawn(Node {
                            width: px(TALLY_WIDTH),
                            flex_shrink: 0.0,
                            flex_direction: FlexDirection::Column,
                            ..default()
                        })
                        .with_children(|list| {
                            column_label(list, look, tally::WINDOW_TITLE);
                            for r in tally::tally_rows(end) {
                                list.spawn(row(look)).with_children(|l| {
                                    let label = palette::LABEL;
                                    l.spawn(line(look, Face::Plain, 10.5, label, r.label));
                                    let value = palette::VALUE;
                                    l.spawn(line(look, Face::Narrow, 12.5, value, r.value));
                                });
                            }
                        });
                    // the talent experience
                    columns
                        .spawn(Node {
                            flex_grow: 1.0,
                            min_width: Val::Px(0.0),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        })
                        .with_children(|xp| {
                            column_label(xp, look, "Talent Experience");
                            if reveal.parts.is_empty() {
                                xp.spawn(row(look)).with_children(|l| {
                                    l.spawn(look.hint("Nothing earned this time"));
                                });
                            }
                            for &(part, amount) in &reveal.parts {
                                let shown_row = xp
                                    .spawn((row(look), Visibility::Hidden))
                                    .with_children(|l| {
                                        l.spawn(line(
                                            look,
                                            Face::Plain,
                                            10.5,
                                            palette::VALUE,
                                            part.label(),
                                        ));
                                        l.spawn(line(
                                            look,
                                            Face::Narrow,
                                            12.5,
                                            palette::GREEN,
                                            format!("+{}", tally::whole_number(amount.into())),
                                        ));
                                    })
                                    .id();
                                part_rows.push(shown_row);
                            }
                            xp.spawn(look.divider());
                            xp.spawn(row(look)).with_children(|l| {
                                l.spawn(line(look, Face::Plain, 11.0, palette::HEADING, "Earned"));
                                counter = l
                                    .spawn(line(look, Face::Narrow, 13.0, palette::HEADING, "+0"))
                                    .id();
                            });
                            // the class's bar toward its next talent point, as the hero sheet's
                            xp.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    height: px(BAR_HEIGHT),
                                    margin: UiRect::vertical(px(5.0)),
                                    border: UiRect::all(px(1.0)),
                                    border_radius: BorderRadius::all(px(3.0)),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                                BorderColor::all(palette::GOLD_DIM),
                            ))
                            .with_children(|b| {
                                bar = b
                                    .spawn((
                                        Node {
                                            width: Val::Percent(start.standing.fill() * 100.0),
                                            height: Val::Percent(100.0),
                                            border_radius: BorderRadius::all(px(2.0)),
                                            ..default()
                                        },
                                        BackgroundColor(EXPERIENCE_BAR),
                                    ))
                                    .id();
                                b.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(100.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    Pickable::IGNORE,
                                ))
                                .with_children(|over| {
                                    bar_text_entity = over
                                        .spawn(line(
                                            look,
                                            Face::Narrow,
                                            10.0,
                                            palette::VALUE,
                                            bar_text(&start),
                                        ))
                                        .id();
                                });
                            });
                            xp.spawn(row(look)).with_children(|l| {
                                points = l
                                    .spawn(line(
                                        look,
                                        Face::Plain,
                                        11.0,
                                        palette::HEADING,
                                        points_text(&start),
                                    ))
                                    .id();
                                point_pop = l
                                    .spawn((
                                        line(
                                            look,
                                            Face::Plain,
                                            11.0,
                                            palette::GREEN,
                                            "New talent point!",
                                        ),
                                        Visibility::Hidden,
                                    ))
                                    .id();
                            });
                        });
                });
            if let Some(zone) = &end.zone_opened {
                panel.spawn(look.divider());
                panel.spawn(line(
                    look,
                    Face::Plain,
                    11.5,
                    palette::GREEN,
                    tally::zone_opened(zone),
                ));
            }
            if !loot.is_empty() {
                panel.spawn(look.divider());
                spawn_loot(panel, look, &loot);
            }
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    column_gap: px(8.0),
                    margin: UiRect::top(px(10.0)),
                    ..default()
                })
                .with_children(|buttons| {
                    for &b in shown.buttons() {
                        let width = if b == EndButton::MainMenu { MAIN_MENU_WIDTH } else { BUTTON_WIDTH };
                        look.button(buttons, b.label(), button_style(b), true, Some(width))
                            .insert(b);
                    }
                });
        })
        .id();
    (
        root,
        RevealParts {
            reveal,
            part_rows,
            counter,
            bar,
            bar_text: bar_text_entity,
            points,
            point_pop,
        },
    )
}

/// A button's look: Run Again the call to act (gold-framed), the others framed text buttons.
fn button_style(button: EndButton) -> ButtonStyle {
    if button == EndButton::Again {
        ButtonStyle::Primary
    } else {
        ButtonStyle::Text
    }
}

/// The gear won: a heading with how many are new, then a row per piece (its name in its quality
/// colour, its slot, new or already owned), the first few only.
fn spawn_loot(panel: &mut ChildSpawnerCommands, look: &MenuLook, loot: &[LootRow]) {
    panel
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|list| {
            column_label(list, look, &tally::loot_heading(loot));
            for piece in loot.iter().take(LOOT_ROWS) {
                list.spawn(row(look)).with_children(|l| {
                    l.spawn((
                        line(
                            look,
                            Face::Plain,
                            10.5,
                            rarity_colour(piece.rarity),
                            piece.name.clone(),
                        ),
                        Node {
                            flex_grow: 1.0,
                            min_width: Val::Px(0.0),
                            overflow: Overflow::clip_x(),
                            ..default()
                        },
                    ));
                    l.spawn((
                        line(look, Face::Plain, 9.5, palette::LABEL, piece.slot),
                        Node {
                            width: look.px(SLOT_WIDTH),
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ));
                    let colour = if piece.new {
                        palette::GREEN
                    } else {
                        palette::HINT
                    };
                    l.spawn((
                        Node {
                            width: look.px(STATUS_WIDTH),
                            flex_shrink: 0.0,
                            justify_content: JustifyContent::End,
                            ..default()
                        },
                        children![line(
                            look,
                            Face::Plain,
                            10.0,
                            colour,
                            tally::loot_status(piece.new)
                        )],
                    ));
                });
            }
            if loot.len() > LOOT_ROWS {
                list.spawn(row(look)).with_children(|l| {
                    l.spawn(look.hint(format!("and {} more", loot.len() - LOOT_ROWS)));
                });
            }
        });
}

#[cfg(test)]
mod tests {
    use esurv_protocol::Experience;

    use super::*;

    #[test]
    fn the_panel_sits_centred_near_the_top_at_the_stock_scale() {
        let s = StockScale::for_window(768.0, 1.0);
        let at = default_place(1920.0, s);
        assert!((at.x - (1920.0 - PANEL_WIDTH * 0.9) / 2.0).abs() < 1e-3);
        assert!((at.y - PANEL_TOP * 0.9).abs() < 1e-3);
        assert_eq!(default_place(100.0, s).x, 0.0, "never off the left edge");
    }

    #[test]
    fn the_bar_and_points_read_as_the_reveal_goes() {
        let r = Reveal::new(
            70,
            &Experience {
                time: 250,
                ..Default::default()
            },
        );
        let start = r.at(0.0);
        assert_eq!(bar_text(&start), "70 / 80");
        assert_eq!(points_text(&start), "Talent points: 0");
        let end = r.at(r.length());
        assert_eq!(bar_text(&end), "74 / 86");
        assert_eq!(points_text(&end), "Talent points: 3 (+3)");
        let all = Reveal::new(u32::MAX - 10, &Experience::default()).at(0.0);
        assert_eq!(bar_text(&all), "Every talent point earned");
    }

    #[test]
    fn a_talent_points_line_shows_at_once_then_fades() {
        assert_eq!(point_pop_alpha(None), 0.0, "no point crossed yet");
        assert_eq!(point_pop_alpha(Some(0.0)), 1.0);
        assert_eq!(point_pop_alpha(Some(POINT_POP_TIME * 0.4)), 1.0);
        let fading = point_pop_alpha(Some(POINT_POP_TIME * 0.75));
        assert!((fading - 0.5).abs() < 1e-5);
        assert_eq!(point_pop_alpha(Some(POINT_POP_TIME)), 0.0, "gone");
        assert_eq!(point_pop_alpha(Some(-1.0)), 0.0);
    }

    #[test]
    fn the_panel_wears_the_menus_look() {
        assert_eq!(
            title_colour(Ending::Victory),
            palette::HEADING,
            "a victory in the headings' gold"
        );
        assert_eq!(title_colour(Ending::Fell), palette::RED);
        assert_eq!(button_style(EndButton::Again), ButtonStyle::Primary, "the call to act");
        assert_eq!(button_style(EndButton::KeepFighting), ButtonStyle::Text);
        assert_eq!(button_style(EndButton::MainMenu), ButtonStyle::Text);
        assert_eq!(
            look::label_colour(ButtonStyle::Primary, false),
            palette::HINT,
            "greyed while an answer is awaited"
        );
    }

    #[test]
    fn the_unattended_switch_names_a_button() {
        assert_eq!(unattended_press(Some("again")), Some(EndButton::Again));
        assert_eq!(
            unattended_press(Some("endless")),
            Some(EndButton::KeepFighting)
        );
        assert_eq!(
            unattended_press(Some("leave")),
            Some(EndButton::MainMenu)
        );
        assert_eq!(unattended_press(Some("dance")), None);
        assert_eq!(unattended_press(None), None);
    }
}
