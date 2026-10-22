//! The meter window, drawn with `bevy_ui` over the game: a dark see-through panel with a red
//! title bar (the mode's name, the mode arrows, reset and fold), one glossy bar per entry in its
//! class's or its own colour with `rank. name` on the left and `amount (per second, share)` on the
//! right, a thin scroll bar when the rows do not fit, and a grip in the corner to resize it. The
//! title bar moves it. The stock button art comes from the player's own 1.12 data; the bar and
//! title shading are made here.

use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::text::LineBreak;
use bevy::ui::{ComputedNode, RelativeCursorPosition};
use bevy::window::PrimaryWindow;

use benilla_app::{Objects, OverlayWindow};

use crate::stock::load_stock_art;
use crate::ui::{layer, BODY_FONT, CLASS_COLOURS, TITLE_FONT};

use super::feed::{MeterFeed, MeterReset, MeterRow};
use super::settings::{settings_path, MeterSettings, MAX_SIZE, MIN_SIZE};

/// Over the game view and the stock UI, under the loading cover (1000), the screen fade (1050) and
/// the cinematic (900).
const WINDOW_Z: i32 = layer::METER;

const BORDER: f32 = 1.0;
const TITLE_HEIGHT: f32 = 18.0;
const BODY_PAD: f32 = 2.0;
const ROW_HEIGHT: f32 = 17.0;
const ROW_GAP: f32 = 1.0;
const ROW_PITCH: f32 = ROW_HEIGHT + ROW_GAP;
/// Everything but the rows: the borders, the title bar, the body's padding, less the last gap.
const FRAME_HEIGHT: f32 = 2.0 * BORDER + TITLE_HEIGHT + 2.0 * BODY_PAD - ROW_GAP;
const BUTTON_SIZE: f32 = 16.0;
const SCROLL_WIDTH: f32 = 5.0;
const THUMB_MIN: f32 = 8.0;
const GRIP_SIZE: f32 = 10.0;
/// The default spot's gap to the game window's right edge.
const DEFAULT_RIGHT_MARGIN: f32 = 16.0;
/// The default spot's bottom, as a share of the window's height from its bottom: above the stock
/// tooltip's own corner (bottom right, where a spell's tooltip grows upward), so the meter never
/// covers a tooltip.
const DEFAULT_BOTTOM_SHARE: f32 = 0.45;

const TITLE_FONT_SIZE: f32 = 12.0;
const ROW_FONT_SIZE: f32 = 12.5;

const WINDOW_BACKGROUND: Color = Color::srgba(0.02, 0.02, 0.03, 0.62);
const WINDOW_EDGE: Color = Color::srgba(0.42, 0.42, 0.46, 0.85);
const TITLE_TINT: Color = Color::srgb(0.52, 0.07, 0.07);
const TITLE_TEXT: Color = Color::srgb(0.97, 0.95, 0.90);
const ROW_TEXT: Color = Color::WHITE;
const TEXT_SHADOW: Color = Color::srgba(0.0, 0.0, 0.0, 0.95);
const SCROLL_TRACK: Color = Color::srgba(1.0, 1.0, 1.0, 0.10);
const SCROLL_THUMB: Color = Color::srgba(0.80, 0.80, 0.82, 0.75);
const GRIP_DOT: Color = Color::srgba(0.85, 0.85, 0.85, 0.55);
const BUTTON_IDLE: Color = Color::srgb(0.82, 0.82, 0.82);
const BUTTON_HOVER: Color = Color::WHITE;
const BUTTON_DOWN: Color = Color::srgb(0.62, 0.62, 0.62);
/// A drawn button face: gold rim and letter on dark red, like the stock arrows beside it.
const FACE_SIZE: f32 = 12.0;
const FACE_FILL: Color = Color::srgb(0.20, 0.04, 0.04);
const FACE_RIM: Color = Color::srgb(0.72, 0.56, 0.20);
const FACE_RIM_LIT: Color = Color::srgb(1.0, 0.86, 0.38);
const FACE_LETTER: Color = Color::srgb(1.0, 0.82, 0.0);
const FACE_FONT_SIZE: f32 = 9.0;
const TIP_BACKGROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.85);
const TIP_FONT_SIZE: f32 = 11.0;

/// The colours of entries with no class (pets, spells, enemies), picked by name.
const ENTRY_COLOURS: [[f32; 3]; 10] = [
    [0.80, 0.36, 0.30],
    [0.36, 0.60, 0.86],
    [0.56, 0.74, 0.34],
    [0.86, 0.66, 0.30],
    [0.60, 0.46, 0.82],
    [0.30, 0.70, 0.66],
    [0.82, 0.50, 0.66],
    [0.66, 0.60, 0.44],
    [0.46, 0.56, 0.68],
    [0.76, 0.42, 0.22],
];

/// The window's root node.
#[derive(Component)]
pub(super) struct MeterWindow;

/// The fixed parts of the window, found again by entity.
#[derive(Resource)]
struct MeterParts {
    title: Entity,
    body: Entity,
    list: Entity,
    track: Entity,
    thumb: Entity,
    grip: Entity,
    tip: Entity,
    tip_text: Entity,
}

/// One row slot's bar and its two texts.
#[derive(Component)]
struct RowParts {
    bar: Entity,
    label: Entity,
    numbers: Entity,
}

/// What a title-bar button does.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum MeterButton {
    PreviousMode,
    NextMode,
    Reset,
    Fold,
}

impl MeterButton {
    /// Its stock art in the player's 1.12 data; `None` draws a face with its letter.
    fn art(self) -> Option<&'static str> {
        match self {
            MeterButton::PreviousMode => Some("Interface\\Buttons\\UI-SpellbookIcon-PrevPage-Up"),
            MeterButton::NextMode => Some("Interface\\Buttons\\UI-SpellbookIcon-NextPage-Up"),
            MeterButton::Reset | MeterButton::Fold => None,
        }
    }

    /// The letter on its drawn face, shown when it has no art or its art is missing.
    fn glyph(self, folded: bool) -> &'static str {
        match self {
            MeterButton::PreviousMode => "<",
            MeterButton::NextMode => ">",
            MeterButton::Reset => "R",
            MeterButton::Fold if folded => "+",
            MeterButton::Fold => "\u{2013}",
        }
    }

    /// What it does, shown while the pointer is on it.
    fn tip(self, folded: bool) -> &'static str {
        match self {
            MeterButton::PreviousMode => "Previous list",
            MeterButton::NextMode => "Next list",
            MeterButton::Reset => "Clear the meter",
            MeterButton::Fold if folded => "Unfold",
            MeterButton::Fold => "Fold to the title",
        }
    }
}

/// A button's drawn face and the letter on it.
#[derive(Component)]
struct ButtonParts {
    face: Entity,
    glyph: Entity,
}

/// What pressing a part of the window starts dragging.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum DragHandle {
    Title,
    Grip,
    Thumb,
}

/// A drag under way.
#[derive(Clone, Copy, Debug)]
enum Drag {
    /// Moving: the cursor's offset from the window's corner.
    Move { grab: Vec2 },
    /// Resizing from this cursor point and size.
    Resize { from: Vec2, size: Vec2 },
    /// The scroll thumb, from this cursor height and scroll.
    Scroll { from: f32, scroll: usize },
}

/// The window's state this session.
#[derive(Resource, Default)]
struct MeterView {
    /// Where the window is drawn, after keeping it on screen.
    shown_at: Vec2,
    /// Row slots that fit the window's height.
    visible_rows: usize,
    /// The first row shown.
    scroll: usize,
    /// The rows the mode has.
    total_rows: usize,
    /// The wheel's fraction of a row, carried.
    wheel_carry: f32,
    drag: Option<Drag>,
    /// The scroll track's height, as last laid out.
    track_height: f32,
}

/// Where the settings file is; `None` keeps nothing.
#[derive(Resource)]
struct MeterFile(Option<std::path::PathBuf>);

/// The fonts and shading the window draws with.
#[derive(Resource)]
struct MeterArt {
    title_font: Handle<Font>,
    row_font: Handle<Font>,
    bar: Handle<Image>,
}

/// Loads the kept settings and adds the window's systems.
pub(super) fn add_window(app: &mut App) {
    let path = settings_path();
    let settings = path.as_deref().map(MeterSettings::load).unwrap_or_default();
    app.insert_resource(settings)
        .insert_resource(MeterFile(path))
        .init_resource::<MeterView>()
        .add_message::<ToggleMeter>()
        .add_systems(Startup, spawn_window)
        .add_systems(
            Update,
            (
                take_toggles,
                show_in_world,
                press_buttons,
                start_drags,
                follow_drags,
                scroll_with_wheel,
                place_window,
                fit_row_slots,
                refresh_rows,
                light_buttons,
            )
                .chain()
                .in_set(MeterSystems),
        );
}

/// The window's systems, in order; the feed's writers run before them.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeterSystems;

/// A 1-pixel-wide vertical shade, white to grey, for a tinted image to multiply.
fn shade_texture(
    images: &mut Assets<Image>,
    height: u32,
    shade: impl Fn(f32) -> f32,
) -> Handle<Image> {
    let mut data = Vec::with_capacity(height as usize * 4);
    for y in 0..height {
        let t = y as f32 / (height - 1) as f32;
        let v = (shade(t).clamp(0.0, 1.0) * 255.0).round() as u8;
        data.extend_from_slice(&[v, v, v, 255]);
    }
    images.add(Image::new(
        Extent3d {
            width: 1,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

/// The glossy bar: a bright upper half over a darker lower half.
fn bar_shade(t: f32) -> f32 {
    if t < 0.5 {
        1.0 - 0.2 * t
    } else {
        0.78 - 0.3 * (t - 0.5)
    }
}

/// The title bar, darkening downwards.
fn title_shade(t: f32) -> f32 {
    1.0 - 0.45 * t
}

fn spawn_window(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    settings: Res<MeterSettings>,
) {
    let art = MeterArt {
        title_font: assets.load(TITLE_FONT),
        row_font: assets.load(BODY_FONT),
        bar: shade_texture(&mut images, 32, bar_shade),
    };
    let title_shading = shade_texture(&mut images, 16, title_shade);
    let folded = settings.collapsed;

    let mut title = Entity::PLACEHOLDER;
    let mut body = Entity::PLACEHOLDER;
    let mut list = Entity::PLACEHOLDER;
    let mut track = Entity::PLACEHOLDER;
    let mut thumb = Entity::PLACEHOLDER;
    let mut grip = Entity::PLACEHOLDER;
    let mut tip = Entity::PLACEHOLDER;
    let mut tip_text = Entity::PLACEHOLDER;
    commands
        .spawn((
            MeterWindow,
            OverlayWindow,
            Name::new("meter window"),
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(BORDER)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(WINDOW_BACKGROUND),
            BorderColor::all(WINDOW_EDGE),
            GlobalZIndex(WINDOW_Z),
            RelativeCursorPosition::default(),
            Visibility::Hidden,
        ))
        .with_children(|window| {
            window
                .spawn((
                    Button,
                    DragHandle::Title,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(TITLE_HEIGHT),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        padding: UiRect::new(
                            Val::Px(5.0),
                            Val::Px(2.0),
                            Val::Px(0.0),
                            Val::Px(0.0),
                        ),
                        column_gap: Val::Px(1.0),
                        border_radius: BorderRadius::top(Val::Px(2.0)),
                        ..default()
                    },
                    ImageNode {
                        image: title_shading,
                        color: TITLE_TINT,
                        image_mode: NodeImageMode::Stretch,
                        ..default()
                    },
                ))
                .with_children(|bar| {
                    title = bar
                        .spawn((
                            Text::new(settings.mode.title()),
                            TextFont {
                                font: art.title_font.clone(),
                                font_size: TITLE_FONT_SIZE,
                                ..default()
                            },
                            TextColor(TITLE_TEXT),
                            TextShadow {
                                offset: Vec2::splat(1.0),
                                color: TEXT_SHADOW,
                            },
                            TextLayout {
                                linebreak: LineBreak::NoWrap,
                                justify: Justify::Left,
                            },
                            Node {
                                flex_grow: 1.0,
                                min_width: Val::Px(0.0),
                                overflow: Overflow::clip_x(),
                                ..default()
                            },
                        ))
                        .id();
                    for button in [
                        MeterButton::PreviousMode,
                        MeterButton::NextMode,
                        MeterButton::Reset,
                        MeterButton::Fold,
                    ] {
                        spawn_button(bar, &assets, &art, button, folded);
                    }
                });
            body = window
                .spawn(Node {
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Row,
                    padding: UiRect::all(Val::Px(BODY_PAD)),
                    column_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|body| {
                    list = body
                        .spawn(Node {
                            flex_grow: 1.0,
                            min_width: Val::Px(0.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(ROW_GAP),
                            overflow: Overflow::clip(),
                            ..default()
                        })
                        .id();
                    track = body
                        .spawn((
                            Node {
                                width: Val::Px(SCROLL_WIDTH),
                                flex_shrink: 0.0,
                                margin: UiRect::bottom(Val::Px(GRIP_SIZE)),
                                border_radius: BorderRadius::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(SCROLL_TRACK),
                            Visibility::Hidden,
                        ))
                        .with_children(|track| {
                            thumb = track
                                .spawn((
                                    Button,
                                    DragHandle::Thumb,
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Px(0.0),
                                        right: Val::Px(0.0),
                                        border_radius: BorderRadius::all(Val::Px(2.0)),
                                        ..default()
                                    },
                                    BackgroundColor(SCROLL_THUMB),
                                ))
                                .id();
                        })
                        .id();
                })
                .id();
            grip = window
                .spawn((
                    Button,
                    DragHandle::Grip,
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(1.0),
                        bottom: Val::Px(1.0),
                        width: Val::Px(GRIP_SIZE),
                        height: Val::Px(GRIP_SIZE),
                        ..default()
                    },
                ))
                .with_children(|grip| {
                    // a corner of six dots
                    for (x, y) in [
                        (7.0, 1.0),
                        (7.0, 4.0),
                        (4.0, 4.0),
                        (7.0, 7.0),
                        (4.0, 7.0),
                        (1.0, 7.0),
                    ] {
                        grip.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(x),
                                top: Val::Px(y),
                                width: Val::Px(2.0),
                                height: Val::Px(2.0),
                                ..default()
                            },
                            BackgroundColor(GRIP_DOT),
                        ));
                    }
                })
                .id();
            // last, so it draws over the rows: a button's tip, under the title bar's right end
            tip = window
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(TITLE_HEIGHT + 2.0),
                        right: Val::Px(2.0),
                        padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)),
                        border: UiRect::all(Val::Px(BORDER)),
                        border_radius: BorderRadius::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(TIP_BACKGROUND),
                    BorderColor::all(WINDOW_EDGE),
                    Visibility::Hidden,
                ))
                .with_children(|tip| {
                    tip_text = tip
                        .spawn((
                            Text::new(""),
                            TextFont {
                                font: art.row_font.clone(),
                                font_size: TIP_FONT_SIZE,
                                ..default()
                            },
                            TextColor(TITLE_TEXT),
                            TextLayout {
                                linebreak: LineBreak::NoWrap,
                                justify: Justify::Left,
                            },
                        ))
                        .id();
                })
                .id();
        });
    commands.insert_resource(MeterParts {
        title,
        body,
        list,
        track,
        thumb,
        grip,
        tip,
        tip_text,
    });
    commands.insert_resource(art);
}

/// A 16-pixel button: its stock art, over a drawn face with its letter that shows when it has no
/// art or the art did not load.
fn spawn_button(
    bar: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    art: &MeterArt,
    button: MeterButton,
    folded: bool,
) {
    let mut glyph = Entity::PLACEHOLDER;
    let mut face = Entity::PLACEHOLDER;
    let mut slot = bar.spawn((
        Button,
        button,
        Node {
            width: Val::Px(BUTTON_SIZE),
            height: Val::Px(BUTTON_SIZE),
            flex_shrink: 0.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
    ));
    if let Some(path) = button.art() {
        slot.insert(ImageNode {
            // in the stock kit's sprite form (sRGB), so it draws in the stock colours
            image: load_stock_art(assets, path),
            color: BUTTON_IDLE,
            image_mode: NodeImageMode::Stretch,
            ..default()
        });
    }
    slot.with_children(|b| {
        face = b
            .spawn((
                Node {
                    width: Val::Px(FACE_SIZE),
                    height: Val::Px(FACE_SIZE),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(BORDER)),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(FACE_FILL),
                BorderColor::all(FACE_RIM),
                Visibility::Hidden,
            ))
            .with_children(|face| {
                glyph = face
                    .spawn((
                        Text::new(button.glyph(folded)),
                        TextFont {
                            font: art.title_font.clone(),
                            font_size: FACE_FONT_SIZE,
                            ..default()
                        },
                        TextColor(FACE_LETTER),
                    ))
                    .id();
            })
            .id();
    });
    slot.insert(ButtonParts { face, glyph });
}

/// Asks the meter to show if hidden, or hide if shown (the minimap's meter button); the change
/// is kept in `meter.ron`.
#[derive(Message, Clone, Copy, Debug, Default)]
pub struct ToggleMeter;

/// Flips the window shown or hidden for each [`ToggleMeter`], and keeps it.
fn take_toggles(
    mut toggles: MessageReader<ToggleMeter>,
    mut settings: ResMut<MeterSettings>,
    file: Res<MeterFile>,
) {
    let flips = toggles.read().count();
    if flips == 0 {
        return;
    }
    if flips % 2 == 1 {
        settings.hidden = !settings.hidden;
    }
    save(&settings, &file);
}

/// Shows the window while a hero is in the world, unless the minimap's button hid it.
fn show_in_world(
    objects: Objects,
    settings: Res<MeterSettings>,
    mut roots: Query<&mut Visibility, With<MeterWindow>>,
) {
    let want = if objects.player().is_some() && !settings.hidden {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut shown in &mut roots {
        shown.set_if_neq(want);
    }
}

/// The title bar's buttons: the mode arrows, reset and fold.
fn press_buttons(
    presses: Query<(&Interaction, &MeterButton), Changed<Interaction>>,
    mut settings: ResMut<MeterSettings>,
    mut feed: ResMut<MeterFeed>,
    mut resets: MessageWriter<MeterReset>,
    mut view: ResMut<MeterView>,
    file: Res<MeterFile>,
) {
    for (interaction, button) in &presses {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            MeterButton::PreviousMode | MeterButton::NextMode => {
                settings.mode = if *button == MeterButton::NextMode {
                    settings.mode.next()
                } else {
                    settings.mode.previous()
                };
                view.scroll = 0;
                save(&settings, &file);
            }
            MeterButton::Reset => {
                feed.clear();
                resets.write(MeterReset);
                view.scroll = 0;
            }
            MeterButton::Fold => {
                settings.collapsed = !settings.collapsed;
                save(&settings, &file);
            }
        }
    }
}

/// A press on the title bar, the grip or the scroll thumb starts its drag.
fn start_drags(
    window: Single<&Window, With<PrimaryWindow>>,
    presses: Query<(&Interaction, &DragHandle), Changed<Interaction>>,
    mut settings: ResMut<MeterSettings>,
    mut view: ResMut<MeterView>,
) {
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    for (interaction, handle) in &presses {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // the window stays where it is drawn from here on, not at its default spot
        if *handle != DragHandle::Thumb {
            settings.position = Some((view.shown_at.x, view.shown_at.y));
        }
        view.drag = Some(match handle {
            DragHandle::Title => Drag::Move {
                grab: cursor - view.shown_at,
            },
            DragHandle::Grip => Drag::Resize {
                from: cursor,
                size: settings.clamped_size(),
            },
            DragHandle::Thumb => Drag::Scroll {
                from: cursor.y,
                scroll: view.scroll,
            },
        });
    }
}

/// Follows the cursor while the left button is held; the release keeps the window's new place.
fn follow_drags(
    window: Single<&Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut settings: ResMut<MeterSettings>,
    mut view: ResMut<MeterView>,
    file: Res<MeterFile>,
) {
    let Some(drag) = view.drag else {
        return;
    };
    if !mouse.pressed(MouseButton::Left) {
        view.drag = None;
        if !matches!(drag, Drag::Scroll { .. }) {
            save(&settings, &file);
        }
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let screen = window.size();
    match drag {
        Drag::Move { grab } => {
            let at = keep_on_screen(cursor - grab, shown_size(&settings), screen);
            if settings.position != Some((at.x, at.y)) {
                settings.position = Some((at.x, at.y));
            }
        }
        Drag::Resize { from, size } => {
            let room = screen - view.shown_at;
            let want = snap_size(size + (cursor - from), room);
            if settings.size != (want.x, want.y) {
                settings.size = (want.x, want.y);
            }
        }
        Drag::Scroll { from, scroll } => {
            let track = view.track_height;
            let moved = rows_dragged(cursor.y - from, track, view.total_rows);
            let limit = scroll_limit(view.total_rows, view.visible_rows);
            view.scroll = (scroll as i64 + moved).clamp(0, limit as i64) as usize;
        }
    }
}

/// The wheel over the window scrolls its rows.
fn scroll_with_wheel(
    wheel: Res<AccumulatedMouseScroll>,
    roots: Query<(&RelativeCursorPosition, &InheritedVisibility), With<MeterWindow>>,
    mut view: ResMut<MeterView>,
) {
    if wheel.delta.y == 0.0 {
        return;
    }
    if !roots
        .iter()
        .any(|(at, shown)| shown.get() && at.cursor_over())
    {
        return;
    }
    let lines = match wheel.unit {
        MouseScrollUnit::Line => wheel.delta.y,
        MouseScrollUnit::Pixel => wheel.delta.y / ROW_PITCH,
    };
    view.wheel_carry += lines;
    let whole = view.wheel_carry.trunc();
    view.wheel_carry -= whole;
    let limit = scroll_limit(view.total_rows, view.visible_rows) as i64;
    view.scroll = (view.scroll as i64 - whole as i64).clamp(0, limit) as usize;
}

/// Puts the window where the settings say, kept on screen, and folds or unfolds it.
fn place_window(
    window: Single<&Window, With<PrimaryWindow>>,
    settings: Res<MeterSettings>,
    parts: Res<MeterParts>,
    mut view: ResMut<MeterView>,
    mut roots: Query<&mut Node, With<MeterWindow>>,
    mut shown: Query<&mut Visibility, Without<MeterWindow>>,
) {
    let screen = window.size();
    let size = shown_size(&settings);
    let wanted = settings.position.map_or_else(
        || default_position(settings.clamped_size(), screen),
        |(x, y)| Vec2::new(x, y),
    );
    let at = keep_on_screen(wanted, size, screen);
    view.shown_at = at;
    view.visible_rows = if settings.collapsed {
        0
    } else {
        rows_for_height(size.y)
    };
    for mut node in &mut roots {
        let (left, top, width, height) = (
            Val::Px(at.x),
            Val::Px(at.y),
            Val::Px(size.x),
            Val::Px(size.y),
        );
        if node.left != left || node.top != top || node.width != width || node.height != height {
            node.left = left;
            node.top = top;
            node.width = width;
            node.height = height;
        }
    }
    let unfolded = if settings.collapsed {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for part in [parts.body, parts.grip] {
        if let Ok(mut v) = shown.get_mut(part) {
            v.set_if_neq(unfolded);
        }
    }
}

/// Keeps one row slot per row the window's height fits.
fn fit_row_slots(
    mut commands: Commands,
    parts: Res<MeterParts>,
    art: Res<MeterArt>,
    view: Res<MeterView>,
    lists: Query<&Children>,
) {
    let have = lists.get(parts.list).map_or(0, |c| c.len());
    if have == view.visible_rows {
        return;
    }
    commands.entity(parts.list).despawn_related::<Children>();
    for _ in 0..view.visible_rows {
        let slot = spawn_row_slot(&mut commands, &art);
        commands.entity(parts.list).add_child(slot);
    }
}

fn spawn_row_slot(commands: &mut Commands, art: &MeterArt) -> Entity {
    let text = |commands: &mut Commands, node: Node| {
        commands
            .spawn((
                Text::new(""),
                TextFont {
                    font: art.row_font.clone(),
                    font_size: ROW_FONT_SIZE,
                    ..default()
                },
                TextColor(ROW_TEXT),
                TextShadow {
                    offset: Vec2::splat(1.0),
                    color: TEXT_SHADOW,
                },
                TextLayout {
                    linebreak: LineBreak::NoWrap,
                    justify: Justify::Left,
                },
                node,
            ))
            .id()
    };
    let bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                height: Val::Percent(100.0),
                width: Val::Percent(0.0),
                ..default()
            },
            ImageNode {
                image: art.bar.clone(),
                image_mode: NodeImageMode::Stretch,
                ..default()
            },
        ))
        .id();
    let label = text(
        commands,
        Node {
            flex_grow: 1.0,
            flex_shrink: 1.0,
            min_width: Val::Px(0.0),
            overflow: Overflow::clip_x(),
            ..default()
        },
    );
    let numbers = text(
        commands,
        Node {
            flex_shrink: 0.0,
            margin: UiRect::left(Val::Px(4.0)),
            ..default()
        },
    );
    commands
        .spawn((
            RowParts {
                bar,
                label,
                numbers,
            },
            Node {
                height: Val::Px(ROW_HEIGHT),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(3.0)),
                ..default()
            },
            Visibility::Hidden,
        ))
        .add_children(&[bar, label, numbers])
        .id()
}

/// Writes the feed's rows into the slots, from the scroll on, and the title and scroll bar.
fn refresh_rows(
    feed: Res<MeterFeed>,
    settings: Res<MeterSettings>,
    parts: Res<MeterParts>,
    mut view: ResMut<MeterView>,
    lists: Query<&Children>,
    slots: Query<&RowParts>,
    mut texts: Query<&mut Text>,
    mut nodes: Query<&mut Node, Without<MeterWindow>>,
    mut images: Query<&mut ImageNode>,
    mut shown: Query<&mut Visibility, Without<MeterWindow>>,
    laid_out: Query<&ComputedNode>,
) {
    let rows = feed.rows(settings.mode);
    view.total_rows = rows.len();
    let limit = scroll_limit(rows.len(), view.visible_rows);
    view.scroll = view.scroll.min(limit);
    set_text(&mut texts, parts.title, settings.mode.title());

    let slot_list: Vec<Entity> = lists
        .get(parts.list)
        .map(|c| c.iter().collect())
        .unwrap_or_default();
    for (i, slot) in slot_list.into_iter().enumerate() {
        let Ok(row_parts) = slots.get(slot) else {
            continue;
        };
        let row = rows.get(view.scroll + i);
        if let Ok(mut v) = shown.get_mut(slot) {
            v.set_if_neq(if row.is_some() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
        let Some(row) = row else {
            continue;
        };
        set_text(&mut texts, row_parts.label, &row.label());
        set_text(&mut texts, row_parts.numbers, &row.numbers());
        if let Ok(mut node) = nodes.get_mut(row_parts.bar) {
            let width = Val::Percent(row.fill * 100.0);
            if node.width != width {
                node.width = width;
            }
        }
        if let Ok(mut image) = images.get_mut(row_parts.bar) {
            let colour = bar_colour(row);
            if image.color != colour {
                image.color = colour;
            }
        }
    }

    // the scroll bar, when the rows do not all fit
    let scrolls = limit > 0;
    if let Ok(mut v) = shown.get_mut(parts.track) {
        v.set_if_neq(if scrolls {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    // the track's height as last laid out, else as the rows would make it
    view.track_height = laid_out
        .get(parts.track)
        .map(|n| n.size().y * n.inverse_scale_factor())
        .ok()
        .filter(|h| *h > 0.0)
        .unwrap_or(view.visible_rows as f32 * ROW_PITCH - ROW_GAP - GRIP_SIZE);
    if scrolls {
        let (top, height) = thumb_span(
            rows.len(),
            view.visible_rows,
            view.scroll,
            view.track_height,
        );
        if let Ok(mut node) = nodes.get_mut(parts.thumb) {
            let (top, height) = (Val::Px(top), Val::Px(height));
            if node.top != top || node.height != height {
                node.top = top;
                node.height = height;
            }
        }
    }
}

/// Lights the button under the cursor, shows a drawn face where a button has no art or its art
/// did not load, and the tip of the button under the cursor.
fn light_buttons(
    assets: Res<AssetServer>,
    settings: Res<MeterSettings>,
    parts: Res<MeterParts>,
    mut buttons: Query<(
        &Interaction,
        &MeterButton,
        Option<&mut ImageNode>,
        &ButtonParts,
    )>,
    mut faces: Query<(&mut Visibility, &mut BorderColor)>,
    mut texts: Query<&mut Text>,
) {
    let folded = settings.collapsed;
    let mut tip = None;
    for (interaction, button, image, button_parts) in &mut buttons {
        let lit = *interaction != Interaction::None;
        if lit {
            tip = Some(button.tip(folded));
        }
        let mut missing = true;
        if let Some(mut image) = image {
            let tint = match interaction {
                Interaction::Pressed => BUTTON_DOWN,
                Interaction::Hovered => BUTTON_HOVER,
                Interaction::None => BUTTON_IDLE,
            };
            if image.color != tint {
                image.color = tint;
            }
            missing = assets.load_state(&image.image).is_failed();
        }
        if let Ok((mut v, mut rim)) = faces.get_mut(button_parts.face) {
            v.set_if_neq(if missing {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
            let rim_colour = if lit { FACE_RIM_LIT } else { FACE_RIM };
            if rim.top != rim_colour {
                *rim = BorderColor::all(rim_colour);
            }
        }
        set_text(&mut texts, button_parts.glyph, button.glyph(folded));
    }
    if let Ok((mut v, _)) = faces.get_mut(parts.tip) {
        v.set_if_neq(if tip.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    if let Some(tip) = tip {
        set_text(&mut texts, parts.tip_text, tip);
    }
}

fn set_text(texts: &mut Query<&mut Text>, entity: Entity, value: &str) {
    if let Ok(mut text) = texts.get_mut(entity) {
        if text.0 != value {
            text.0 = value.to_owned();
        }
    }
}

fn save(settings: &MeterSettings, file: &MeterFile) {
    if let Some(path) = &file.0 {
        if let Err(e) = settings.save(path) {
            warn!(
                "meter: could not keep the window's place in {}: {e}",
                path.display()
            );
        }
    }
}

/// A row's bar colour: its class's, else one of [`ENTRY_COLOURS`] kept by its name.
fn bar_colour(row: &MeterRow) -> Color {
    let class = row
        .class
        .as_deref()
        .and_then(|c| CLASS_COLOURS.iter().find(|(id, _)| *id == c));
    let [r, g, b] = match class {
        Some((_, rgb)) => *rgb,
        None => ENTRY_COLOURS[name_slot(&row.name, ENTRY_COLOURS.len())],
    };
    Color::srgb(r, g, b)
}

/// A slot in `0..slots` that stays the same for a name (FNV-1a).
fn name_slot(name: &str, slots: usize) -> usize {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in name.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash as usize % slots.max(1)
}

/// The window's drawn size: folded, only the title bar.
fn shown_size(settings: &MeterSettings) -> Vec2 {
    let size = settings.clamped_size();
    if settings.collapsed {
        Vec2::new(size.x, TITLE_HEIGHT + 2.0 * BORDER)
    } else {
        size
    }
}

/// The spot a window that was never moved takes: on the right, above the tooltips' corner.
fn default_position(size: Vec2, screen: Vec2) -> Vec2 {
    Vec2::new(
        screen.x - size.x - DEFAULT_RIGHT_MARGIN,
        screen.y * (1.0 - DEFAULT_BOTTOM_SHARE) - size.y,
    )
}

/// `at`, moved so a window of `size` stays inside the screen (its top-left corner first).
fn keep_on_screen(at: Vec2, size: Vec2, screen: Vec2) -> Vec2 {
    let far = (screen - size).max(Vec2::ZERO);
    at.clamp(Vec2::ZERO, far)
}

/// The row slots a window `height` tall fits.
fn rows_for_height(height: f32) -> usize {
    ((height - FRAME_HEIGHT) / ROW_PITCH).floor().max(0.0) as usize
}

/// The height that fits `rows` exactly.
fn height_for_rows(rows: usize) -> f32 {
    FRAME_HEIGHT + rows as f32 * ROW_PITCH
}

/// A size the grip asked for, inside the bounds and `room`, its height on whole rows.
fn snap_size(want: Vec2, room: Vec2) -> Vec2 {
    let high = MAX_SIZE.min(room).max(MIN_SIZE);
    let size = want.clamp(MIN_SIZE, high);
    let rows = ((size.y - FRAME_HEIGHT) / ROW_PITCH).round().max(0.0) as usize;
    let mut height = height_for_rows(rows);
    while height > high.y && rows > 0 && height > MIN_SIZE.y {
        height -= ROW_PITCH;
    }
    Vec2::new(size.x, height.max(MIN_SIZE.y))
}

/// How far the first row shown can go.
fn scroll_limit(total: usize, visible: usize) -> usize {
    total.saturating_sub(visible)
}

/// The scroll thumb's top and height in a track `track` pixels tall.
fn thumb_span(total: usize, visible: usize, scroll: usize, track: f32) -> (f32, f32) {
    if total == 0 || track <= 0.0 {
        return (0.0, track.max(0.0));
    }
    let height = (track * visible as f32 / total as f32).clamp(THUMB_MIN.min(track), track);
    let limit = scroll_limit(total, visible);
    let top = if limit == 0 {
        0.0
    } else {
        (track - height) * scroll as f32 / limit as f32
    };
    (top, height)
}

/// Rows a thumb drag of `pixels` moves through, in a track `track` pixels tall.
fn rows_dragged(pixels: f32, track: f32, total: usize) -> i64 {
    if track <= 0.0 {
        return 0;
    }
    (pixels * total as f32 / track).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meter::feed::{MeterEntry, MeterMode};
    use bevy::ecs::system::RunSystemOnce;
    use bevy::window::WindowResolution;

    /// A world with the window's state and a 1600x900 game window, the cursor at `cursor`.
    fn world_with_cursor(cursor: Vec2) -> World {
        let mut world = World::new();
        world.insert_resource(MeterSettings::default());
        world.insert_resource(MeterFeed {
            elapsed: 10.0,
            damage_done: (1..=12)
                .map(|i| MeterEntry::new(format!("Hero {i}"), i * 100))
                .collect(),
            ..default()
        });
        world.insert_resource(MeterView {
            visible_rows: 8,
            total_rows: 12,
            track_height: 133.0,
            ..default()
        });
        world.insert_resource(MeterFile(None));
        world.init_resource::<Messages<MeterReset>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        let mut window = Window {
            resolution: WindowResolution::new(1600, 900),
            ..default()
        };
        window.set_cursor_position(Some(cursor));
        world.spawn((window, PrimaryWindow));
        world
    }

    /// One press of `button`, as a fresh system sees it.
    fn press(world: &mut World, button: MeterButton) {
        let pressed = world.spawn((Interaction::Pressed, button)).id();
        world.run_system_once(press_buttons).unwrap();
        world.despawn(pressed);
    }

    #[test]
    fn the_arrows_walk_the_modes_and_reset_clears() {
        let mut world = world_with_cursor(Vec2::ZERO);
        press(&mut world, MeterButton::NextMode);
        assert_eq!(
            world.resource::<MeterSettings>().mode,
            MeterMode::DamageTaken
        );
        press(&mut world, MeterButton::PreviousMode);
        assert_eq!(
            world.resource::<MeterSettings>().mode,
            MeterMode::DamageDone
        );
        press(&mut world, MeterButton::Fold);
        assert!(world.resource::<MeterSettings>().collapsed);
        press(&mut world, MeterButton::Reset);
        assert_eq!(*world.resource::<MeterFeed>(), MeterFeed::default());
        assert_eq!(world.resource::<Messages<MeterReset>>().len(), 1);
    }

    #[test]
    fn the_title_bar_drags_the_window_and_the_release_keeps_it() {
        let dir =
            std::env::temp_dir().join(format!("esurv-meter-drag-test-{}", std::process::id()));
        let path = dir.join("meter.ron");
        let mut world = world_with_cursor(Vec2::new(500.0, 400.0));
        world.insert_resource(MeterFile(Some(path.clone())));
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        world.resource_mut::<MeterView>().drag = Some(Drag::Move {
            grab: Vec2::new(10.0, 5.0),
        });
        world.run_system_once(follow_drags).unwrap();
        assert_eq!(
            world.resource::<MeterSettings>().position,
            Some((490.0, 395.0))
        );
        assert!(!path.exists(), "nothing is written mid-drag");
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        world.run_system_once(follow_drags).unwrap();
        assert!(world.resource::<MeterView>().drag.is_none());
        assert_eq!(
            MeterSettings::load(&path).position,
            Some((490.0, 395.0)),
            "the release keeps the place"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_minimap_toggle_hides_and_shows_and_keeps_it() {
        let dir =
            std::env::temp_dir().join(format!("esurv-meter-toggle-test-{}", std::process::id()));
        let path = dir.join("meter.ron");
        let mut world = world_with_cursor(Vec2::ZERO);
        world.insert_resource(MeterFile(Some(path.clone())));
        world.init_resource::<Messages<ToggleMeter>>();
        world.run_system_once(take_toggles).unwrap();
        assert!(!path.exists(), "no toggle, nothing written");
        world.write_message(ToggleMeter);
        world.run_system_once(take_toggles).unwrap();
        assert!(world.resource::<MeterSettings>().hidden);
        assert!(MeterSettings::load(&path).hidden, "the hide is kept");
        world.resource_mut::<Messages<ToggleMeter>>().clear();
        world.write_message(ToggleMeter);
        world.run_system_once(take_toggles).unwrap();
        assert!(!world.resource::<MeterSettings>().hidden);
        assert!(!MeterSettings::load(&path).hidden, "and the show");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_grip_resizes_on_whole_rows() {
        let mut world = world_with_cursor(Vec2::new(150.0, 140.0));
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        world.resource_mut::<MeterView>().drag = Some(Drag::Resize {
            from: Vec2::new(100.0, 100.0),
            size: Vec2::new(250.0, 167.0),
        });
        world.run_system_once(follow_drags).unwrap();
        assert_eq!(world.resource::<MeterSettings>().size, (300.0, 203.0));
    }

    #[test]
    fn the_wheel_scrolls_only_over_the_window() {
        let mut world = world_with_cursor(Vec2::ZERO);
        world.insert_resource(AccumulatedMouseScroll {
            unit: MouseScrollUnit::Line,
            delta: Vec2::new(0.0, -3.0),
        });
        let root = world
            .spawn((
                MeterWindow,
                RelativeCursorPosition {
                    cursor_over: false,
                    normalized: None,
                },
                InheritedVisibility::VISIBLE,
            ))
            .id();
        world.run_system_once(scroll_with_wheel).unwrap();
        assert_eq!(world.resource::<MeterView>().scroll, 0, "not over it");
        world
            .entity_mut(root)
            .get_mut::<RelativeCursorPosition>()
            .unwrap()
            .cursor_over = true;
        world.run_system_once(scroll_with_wheel).unwrap();
        assert_eq!(world.resource::<MeterView>().scroll, 3);
        world.run_system_once(scroll_with_wheel).unwrap();
        assert_eq!(
            world.resource::<MeterView>().scroll,
            4,
            "12 rows, 8 shown: the last 8 at most"
        );
    }

    #[test]
    fn heights_fit_whole_rows() {
        assert_eq!(rows_for_height(MeterSettings::default().size.1), 8);
        assert_eq!(rows_for_height(MIN_SIZE.y), 2);
        assert_eq!(rows_for_height(MAX_SIZE.y), 40);
        assert_eq!(height_for_rows(8), 167.0);
        assert_eq!(rows_for_height(10.0), 0);
    }

    #[test]
    fn a_resize_snaps_to_rows_and_bounds() {
        let room = Vec2::new(2000.0, 2000.0);
        assert_eq!(
            snap_size(Vec2::new(300.0, 172.0), room),
            Vec2::new(300.0, 167.0)
        );
        assert_eq!(
            snap_size(Vec2::new(300.0, 178.0), room),
            Vec2::new(300.0, 185.0)
        );
        assert_eq!(snap_size(Vec2::new(10.0, 10.0), room), MIN_SIZE);
        let tight = snap_size(Vec2::new(900.0, 900.0), Vec2::new(400.0, 200.0));
        assert_eq!(tight.x, 400.0);
        assert!(tight.y <= 200.0 && rows_for_height(tight.y) == 9);
    }

    #[test]
    fn the_window_stays_on_screen() {
        let screen = Vec2::new(1600.0, 900.0);
        let size = Vec2::new(250.0, 167.0);
        assert_eq!(
            keep_on_screen(Vec2::new(-40.0, 2000.0), size, screen),
            Vec2::new(0.0, 733.0)
        );
        assert_eq!(
            default_position(size, screen),
            Vec2::new(1334.0, 328.0),
            "above the tooltips' corner"
        );
        assert_eq!(
            keep_on_screen(
                Vec2::new(10.0, 10.0),
                Vec2::new(900.0, 900.0),
                Vec2::new(800.0, 600.0)
            ),
            Vec2::ZERO
        );
    }

    #[test]
    fn the_thumb_tracks_the_scroll() {
        assert_eq!(thumb_span(16, 8, 0, 100.0), (0.0, 50.0));
        assert_eq!(thumb_span(16, 8, 8, 100.0), (50.0, 50.0));
        assert_eq!(thumb_span(400, 8, 0, 100.0).1, THUMB_MIN);
        assert_eq!(rows_dragged(25.0, 100.0, 16), 4);
        assert_eq!(scroll_limit(5, 8), 0);
    }

    #[test]
    fn classes_take_their_colour_and_others_keep_theirs() {
        let row = |name: &str, class: Option<&str>| MeterRow {
            rank: 1,
            name: name.into(),
            amount: 1,
            per_second: None,
            percent: 100.0,
            fill: 1.0,
            class: class.map(str::to_owned),
        };
        assert_eq!(
            bar_colour(&row("Probe", Some("mage"))),
            Color::srgb(0.41, 0.8, 0.94)
        );
        let wolf = bar_colour(&row("Mangy Wolf", None));
        assert_eq!(
            wolf,
            bar_colour(&row("Mangy Wolf", None)),
            "same name, same colour"
        );
        assert_eq!(
            bar_colour(&row("Odd", Some("bard"))),
            bar_colour(&row("Odd", None)),
            "an unknown class colours by name"
        );
    }
}
