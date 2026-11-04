//! The title screen, the first thing the game shows: its art, `title_splash_screen.jpg`, fills the
//! window, the emblem and the game's name at its top left, with Play and Quit under the emblem in
//! the menus' button look. Play (the button, Enter or Return) goes on into the camp, the art held
//! until the loading screen is up under it (with the hub off, the warband screen; a save with no hero, the
//! class picker): the menus' launch rule waits for it ([`passed`]); Escape does nothing here.
//! Under it plays the game's login theme, benilla's glue music. It shows once, at launch; a
//! run ending does not bring it back.
//!
//! The art and the emblem are carried inside the exe ([`TITLE_ART`], [`TITLE_LOGO`]) and decoded
//! the first frame the title is drawn. Dev aids that drive the game unattended skip the title
//! ([`skip_wanted`]), and `ESURV_SKIP_TITLE=1` skips it by hand; `ESURV_TITLE_SHOT=<png>` writes a
//! shot of it.


use bevy::app::AppExit;
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::PrimaryWindow;

use crate::menus::look::{self, palette, ButtonStyle, Face, MenuLook};
use benilla_app::SelectScreen;

use crate::menus::{MenuAction, MenuPress, Menus};
use crate::update::{NoticeView, UpdateNotice, UpdateProgressText};
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use crate::stock::{StockClick, StockScale, StockSystems};

/// The key art, `splash_screen.jpg`, carried inside the exe: the shipped game has no loose files
/// beside it but its content. The menus' cuts are taken from it ([`crate::splash_art`]); the
/// title shows its own art ([`TITLE_ART`]).
pub const SPLASH_SCREEN: &[u8] = include_bytes!("../assets/splash_screen.jpg");

/// The key art's size in pixels: the screen's crop is worked out from it before it is decoded.
pub const SPLASH_SIZE: UVec2 = UVec2::new(4096, 2288);

/// The title's art, `title_splash_screen.jpg`: the bronze dragon before the Caverns
/// of Time under the night sky, carried inside the exe, and its size.
pub const TITLE_ART: &[u8] = include_bytes!("../assets/title_splash_screen.jpg");
pub const TITLE_ART_SIZE: UVec2 = UVec2::new(2048, 1150);

/// The game's emblem, `title_logo.png`, at the art's top left behind the game's name: the banner
/// on its rune ring, the black round it keyed out and the teal inside the ring painted over
/// with the art's starry night blue.
pub const TITLE_LOGO: &[u8] = include_bytes!("../assets/title_logo.png");

/// How far in from the frame's left the emblem and name stand, as a share of the art's width
///.
const INSET: f32 = 4.0;

/// Where the emblem stands on the art, large behind the name, as shares of its
/// size: its left (the name's widest line about as far in as the panels), top and height (it is
/// square), and its width as a share of the art's width. It is dimmed under the words.
const LOGO_LEFT: f32 = INSET + 2.0;
const LOGO_TOP: f32 = 3.5;
const LOGO_HEIGHT: f32 = 24.0;
const LOGO_WIDTH: f32 = LOGO_HEIGHT * TITLE_ART_SIZE.y as f32 / TITLE_ART_SIZE.x as f32;
const LOGO_DIM: f32 = 0.6;

/// The game's name over the emblem, "Eternal" over "Survivors", in Morpheus in gold in a dark
/// outline: its size, as a share of the art's height.
const NAME_SIZE: f32 = 0.08;

/// The game's name, and its outline copies under it: sized with the art ([`fit_the_art`]).
#[derive(Component)]
struct TitleName;

/// The title's layer: over the loading cover (1000), the login and select screens (1100), the
/// menus and the select screen's dialogs (to 1220).
const TITLE_LAYER: i32 = 1500;

/// Where the buttons' night panel stands on the art, as shares of its size: under the emblem,
/// centred on it.
const BUTTONS_WIDTH: f32 = 13.5;
const BUTTONS_LEFT: f32 = LOGO_LEFT + (LOGO_WIDTH - BUTTONS_WIDTH) / 2.0;
const BUTTONS_TOP: f32 = 31.0;
const BUTTONS_HEIGHT: f32 = 19.0;

/// The buttons' width, their padding above and below the word and the word's size, in units:
/// narrower, taller and with a larger word than the menus', the word in gold.
const BUTTON_WIDTH: f32 = 118.0;
const BUTTON_PAD: f32 = 12.0;
const BUTTON_WORD: f32 = 17.0;

/// The lighter bordeaux laid over the buttons' wine.
const BUTTON_TINT: Color = Color::srgba(0.62, 0.19, 0.26, 0.55);

/// The gap between the buttons, in units.
const BUTTON_GAP: f32 = 18.0;

/// After Play, the frames the select screen is down before the art goes: the loading screen rises
/// on the world's entry, the same edge, and is drawn by then.
const COVER_FRAMES: u32 = 3;

/// After Play, the longest the art waits for the world.
const HOLD_AT_MOST_SECS: f32 = 15.0;

/// A shot (`ESURV_TITLE_SHOT`) waits this long after the art shows, for it to reach the screen.
const SHOT_AFTER_SECS: f32 = 2.0;

/// The dev aids that drive the game on their own (scripted shots, presses and runs, the stock
/// screens' smoke tests): with any set, the title screen is skipped, so they run as before.
const UNATTENDED_AIDS: &[&str] = &[
    "ESURV_SKIP_TITLE",
    "ESURV_AUTOSTART",
    "ESURV_CAMP_START",
    "ESURV_ACHIEVEMENT_DEMO",
    "ESURV_BAR_DRAG",
    "ESURV_BAR_DRAG_SHOTS",
    "ESURV_CARDS_SCRIPT",
    "ESURV_CARDS_SHOTS",
    "ESURV_END_PRESS",
    "ESURV_ESCAPE_AT",
    "ESURV_GLOW_CHECK",
    "ESURV_HERO_SHOTS",
    "ESURV_INDICATOR_SHOTS",
    "ESURV_KEY_PRESS",
    "ESURV_KEY_SHOTS",
    "ESURV_MENU_PRESS",
    "ESURV_MENU_SHOTS",
    "ESURV_MENUS_DEMO",
    "ESURV_MENUS_SCRIPT",
    "ESURV_MENUS_SHOTS",
    "ESURV_METER_DEMO",
    "ESURV_MINIMAP_SHOTS",
    "ESURV_ORB_SHOTS",
    "ESURV_PICKER_CLICK",
    "ESURV_PICKER_CLICK_ROW",
    "ESURV_PICKER_SHOT",
    "ESURV_TOOLTIP_SHOT",
    "ESURV_TOOLTIP_SHOTS",
    "ESURV_TOTEM_SHOTS",
    "ESURV_UI_DEMO",
    "WOW_CAPTURE",
    "WOW_CHAR",
    "WOW_CHARCREATE_SHOT",
    "WOW_CHARSELECT_SHOT_AT",
    "WOW_LIVE_SHOT",
    "WOW_LOGIN_SMOKE",
    "WOW_LOGOUT_SMOKE",
    "WOW_PROBE_EXIT_AT",
    "WOW_REALM_SMOKE",
    "WOW_UNATTENDED",
];

/// Whether the title screen is skipped: one of [`UNATTENDED_AIDS`] is set (`ESURV_SKIP_TITLE`
/// only as `1`), by `var`, which reads the environment.
pub fn skip_wanted(var: impl Fn(&str) -> Option<String>) -> bool {
    UNATTENDED_AIDS.iter().any(|name| match var(name) {
        Some(v) if *name == "ESURV_SKIP_TITLE" => v == "1",
        Some(_) => true,
        None => false,
    })
}

/// Whether the title screen still shows. Without it (skipped, or the menus off) the game goes on
/// as if it were passed.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleScreen {
    Showing,
    Passed,
}

/// A run condition: the title screen is passed, or there is none.
pub fn passed(title: Option<Res<TitleScreen>>) -> bool {
    title.is_none_or(|t| *t == TitleScreen::Passed)
}

/// What a title button does.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum TitleAction {
    Enter,
    Quit,
    /// The update pop-up's Update, or the corner's: as the warband screen's Update.
    Update,
    /// The update pop-up's Later: it closes, the corner keeps the offer.
    Later,
}

/// The title's pieces and its art on the way.
#[derive(Resource, Default)]
struct TitleView {
    root: Option<Entity>,
    art: Option<Entity>,
    /// The emblem and the game's name right of it.
    logo: Option<Entity>,
    buttons: Option<Entity>,
    /// The cracks over each button's sandstone, made once.
    cracks: Vec<Handle<Image>>,
    /// When the art showed, for the shot.
    shown_at: Option<f32>,
    /// `ESURV_TITLE_SHOT`'s path, until the shot is taken.
    shot: Option<String>,
    /// The version corner and the update pop-up, and the notice they were drawn for.
    news: Option<Entity>,
    /// The version corner, on the art inside its frame.
    corner: Option<Entity>,
    news_for: Option<(Option<NoticeView>, bool)>,
    /// Later was pressed: the pop-up stays closed.
    later: bool,
    /// When Play was pressed, the select screen seen up since, and the frames it has been down:
    /// the art's hold over the way in ([`leave_the_title`]).
    passed_at: Option<f32>,
    saw_select: bool,
    closed_frames: u32,
}

pub(crate) fn register(app: &mut App) {
    if !crate::menus::menus_on() || skip_wanted(|name| std::env::var(name).ok()) {
        return;
    }
    info!("title screen: shows at launch");
    app.insert_resource(TitleScreen::Showing)
        .insert_resource(TitleView {
            shot: std::env::var("ESURV_TITLE_SHOT").ok(),
            ..default()
        })
        .add_systems(PreUpdate, take_the_keys.after(InputSystems))
        .add_systems(
            Update,
            (
                draw_the_title,
                show_the_art,
                fit_the_art,
                draw_the_news,
                take_the_clicks,
                leave_the_title,
            )
                .chain()
                .after(StockSystems),
        )
        .add_systems(Update, shoot_the_title.after(show_the_art));
}

/// The size the title's art is drawn at in `window`: the window itself when the art's shape is
/// within [`STRETCH`] of it, so the art's painted frame shows whole and as thick on every side
///; otherwise [`cover_size`].
pub fn fill_size(window: Vec2, art: Vec2) -> Vec2 {
    let off = (window.x / window.y) / (art.x / art.y);
    if (off - 1.0).abs() <= STRETCH {
        window
    } else {
        cover_size(window, art)
    }
}

/// How far the title's art may be stretched out of its shape to fill the window: 3%, unseen.
const STRETCH: f32 = 0.03;

/// The size `art` is drawn at to cover `window` whole, its aspect kept: the side that fits
/// exactly, the other cropped.
pub fn cover_size(window: Vec2, art: Vec2) -> Vec2 {
    let scale = (window.x / art.x).max(window.y / art.y);
    art * scale
}


/// Enter or Return enters; neither they nor Escape reach the screens under the title.
fn take_the_keys(mut keys: ResMut<ButtonInput<KeyCode>>, mut title: ResMut<TitleScreen>) {
    if *title != TitleScreen::Showing {
        return;
    }
    let enter =
        keys.clear_just_pressed(KeyCode::Enter) | keys.clear_just_pressed(KeyCode::NumpadEnter);
    keys.clear_just_pressed(KeyCode::Escape);
    if enter {
        *title = TitleScreen::Passed;
    }
}

/// Spawns the title once (black until the art is decoded), and its buttons again whenever the
/// interface's scale changes.
fn draw_the_title(
    mut commands: Commands,
    look: MenuLook,
    scale: Res<StockScale>,
    title: Res<TitleScreen>,
    fonts: Res<crate::ui::UiFonts>,
    mut images: ResMut<Assets<Image>>,
    mut view: ResMut<TitleView>,
) {
    if *title != TitleScreen::Showing {
        return;
    }
    let root = *view.root.get_or_insert_with(|| {
        commands
            .spawn((
                Name::new("title screen"),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::BLACK),
                GlobalZIndex(TITLE_LAYER),
                look::solid(),
                // the stock interface and the select scene under it see no pointer
                benilla_app::OverlayWindow,
            ))
            .id()
    });
    let art = *view.art.get_or_insert_with(|| {
        commands
            .spawn((
                Name::new("title art"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(root),
            ))
            .id()
    });
    if view.logo.is_none() {
        let emblem = commands
            .spawn((
                Name::new("title emblem"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(LOGO_LEFT),
                    top: Val::Percent(LOGO_TOP),
                    height: Val::Percent(LOGO_HEIGHT),
                    aspect_ratio: Some(1.0),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(art),
            ))
            .id();
        view.logo = Some(emblem);
        // the name centred over the emblem, wider than it
        let place = Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(-50.0),
            right: Val::Percent(-50.0),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        };
        let at = commands.spawn((place, Pickable::IGNORE, ChildOf(emblem))).id();
        spawn_the_name(&mut commands, &fonts, at);
    }
    if view.buttons.is_some() && !scale.is_changed() {
        return;
    }
    if let Some(old) = view.buttons.take() {
        commands.entity(old).despawn();
    }
    // a night panel as the menus' are, the buttons in it bordeaux
    let mut node = look.panel_node();
    node.position_type = PositionType::Absolute;
    node.left = Val::Percent(BUTTONS_LEFT);
    node.top = Val::Percent(BUTTONS_TOP);
    node.width = Val::Percent(BUTTONS_WIDTH);
    node.height = Val::Percent(BUTTONS_HEIGHT);
    node.justify_content = JustifyContent::Center;
    node.align_items = AlignItems::Center;
    let panel = commands.spawn((Name::new("title buttons"), look.panel(node), ChildOf(art))).id();
    if view.cracks.is_empty() {
        view.cracks = [3, 11].map(|seed| images.add(crack_texture(seed))).into();
    }
    let cracks = view.cracks.clone();
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: look.px(BUTTON_GAP),
                ..default()
            },
            ChildOf(panel),
        ))
        .with_children(|c| {
            for ((label, action), cracks) in [("Play", TitleAction::Enter), ("Quit", TitleAction::Quit)].into_iter().zip(cracks) {
                let mut b = look.frame_button(c, ButtonStyle::Active, true, Some(BUTTON_WIDTH));
                b.insert(action).entry::<Node>().and_modify({
                    let pad = look.px(BUTTON_PAD);
                    move |mut n| {
                        n.padding.top = pad;
                        n.padding.bottom = pad;
                    }
                });
                b.with_children(|b| {
                    crack_the_stone(b, &cracks);
                    b.spawn((
                        look.text(Face::Display, BUTTON_WORD, palette::GOLD, label),
                        Pickable::IGNORE,
                    ));
                });
            }
        });
    view.buttons = Some(panel);
}

/// The game's name, "Eternal" over "Survivors", centred in `at`: gold in a dark outline (copies
/// of it in black a step off all round, under it) so it reads clearly on the emblem and the night.
fn spawn_the_name(commands: &mut Commands, fonts: &crate::ui::UiFonts, at: Entity) {
    let text = crate::GAME_TITLE.replacen(' ', "\n", 1);
    let stack = commands.spawn((Node::default(), Pickable::IGNORE, ChildOf(at))).id();
    const STEP: f32 = 2.5;
    for (dx, dy) in [(-1.0, -1.0), (0.0, -1.0), (1.0, -1.0), (-1.0, 0.0), (1.0, 0.0), (-1.0, 1.0), (0.0, 1.0), (1.0, 1.0)] {
        commands.spawn((
            Text::new(text.clone()),
            TextFont {
                font: fonts.morpheus.clone(),
                font_size: 40.0,
                ..default()
            },
            TextLayout::new_with_justify(Justify::Center),
            TextColor(Color::srgba(0.04, 0.02, 0.01, 0.95)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(dx * STEP),
                top: Val::Px(dy * STEP),
                ..default()
            },
            TitleName,
            Pickable::IGNORE,
            ChildOf(stack),
        ));
    }
    commands.spawn((
        Text::new(text),
        TextFont {
            font: fonts.morpheus.clone(),
            font_size: 40.0,
            ..default()
        },
        TextLayout::new_with_justify(Justify::Center),
        TextColor(palette::GOLD),
        TextShadow {
            offset: Vec2::splat(5.0),
            color: Color::srgba(0.0, 0.0, 0.0, 0.7),
        },
        TitleName,
        Pickable::IGNORE,
        ChildOf(stack),
    ));
}

/// The cracks over a button's fill and under its word, so the fill still lights on hover
///.
fn crack_the_stone(b: &mut ChildSpawnerCommands, cracks: &Handle<Image>) {
    b.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            ..default()
        },
        // a lighter bordeaux over the menus', the hover still
        // lighting it from under
        BackgroundColor(BUTTON_TINT),
        ImageNode::new(cracks.clone()),
        ZIndex(-1),
        Pickable::IGNORE,
    ));
}

/// A few cracks in the sandstone: thin dark lines wandering in from the edges, a branch now and then, each with a
/// pale lip under it as a cut in stone has; clear elsewhere. Made from a hash on `seed`, so each
/// button keeps its own cracks every launch.
fn crack_texture(seed: u32) -> Image {
    const W: u32 = 384;
    const H: u32 = 96;
    let mut state = seed.wrapping_mul(2_654_435_761) | 1;
    let mut roll = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state & 0xffff) as f32 / 65535.0
    };
    // the dark of the cut and the light of its lip, as coverage
    let mut dark = vec![0.0f32; (W * H) as usize];
    let mut lip = vec![0.0f32; (W * H) as usize];
    let stamp = |field: &mut Vec<f32>, x: f32, y: f32, r: f32, a: f32| {
        let (x0, x1) = ((x - r - 1.0).max(0.0) as u32, ((x + r + 1.0) as u32).min(W - 1));
        let (y0, y1) = ((y - r - 1.0).max(0.0) as u32, ((y + r + 1.0) as u32).min(H - 1));
        for py in y0..=y1 {
            for px in x0..=x1 {
                let d = ((px as f32 - x).powi(2) + (py as f32 - y).powi(2)).sqrt();
                let c = (r + 0.5 - d).clamp(0.0, 1.0) * a;
                let i = (py * W + px) as usize;
                field[i] = field[i].max(c);
            }
        }
    };
    // (x, y, heading, length left, width)
    let mut cracks = Vec::new();
    for _ in 0..4 {
        let (x, y, heading) = match (roll() * 4.0) as u32 {
            0 => (0.0, roll() * H as f32, 0.0),
            1 => (W as f32, roll() * H as f32, std::f32::consts::PI),
            2 => (roll() * W as f32, 0.0, std::f32::consts::FRAC_PI_2),
            _ => (roll() * W as f32, H as f32, -std::f32::consts::FRAC_PI_2),
        };
        cracks.push((x, y, heading + (roll() - 0.5) * 1.2, 40.0 + roll() * 110.0, 2.4 + roll() * 1.0));
    }
    while let Some((mut x, mut y, mut heading, mut left, width)) = cracks.pop() {
        while left > 0.0 {
            heading += (roll() - 0.5) * 0.9;
            x += heading.cos() * 1.5;
            y += heading.sin() * 1.5;
            left -= 1.5;
            let r = width * (0.35 + 0.65 * (left / 150.0).min(1.0));
            stamp(&mut lip, x + 0.6, y + r + 0.9, r * 0.7, 0.3);
            stamp(&mut dark, x, y, r, 0.45);
            if roll() < 0.025 && width > 1.6 {
                cracks.push((x, y, heading + (roll() - 0.5) * 2.0, left * 0.5, width * 0.6));
            }
        }
    }
    let mut pixels = Vec::with_capacity((W * H * 4) as usize);
    for i in 0..(W * H) as usize {
        let (d, l) = (dark[i], lip[i] * (1.0 - dark[i]));
        if d >= l {
            pixels.extend_from_slice(&[38, 22, 8, (d * 255.0) as u8]);
        } else {
            pixels.extend_from_slice(&[255, 236, 196, (l * 255.0) as u8]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image
}

/// The updater's notice as the title shows it ([`crate::update`]); `ESURV_TITLE_OFFER=<version>`,
/// a development aid, shows an offer of that version without asking the feed, to see its look.
fn title_news(notice: Option<&UpdateNotice>) -> Option<NoticeView> {
    if let Ok(v) = std::env::var("ESURV_TITLE_OFFER") {
        return Some(NoticeView {
            line: format!("v{v} is available"),
            failed: false,
            button: Some(true),
            progress: false,
            offer: true,
        });
    }
    notice.and_then(UpdateNotice::view)
}

/// The version and the updater's notice in a box at the bottom left, and while a newer version
/// is offered a pop-up in the middle asking to update;
/// drawn again when the notice changes, Later closes the pop-up, or the scale changes.
fn draw_the_news(
    mut commands: Commands,
    look: MenuLook,
    scale: Res<StockScale>,
    title: Res<TitleScreen>,
    notice: Option<Res<UpdateNotice>>,
    mut view: ResMut<TitleView>,
) {
    let Some(root) = view.root.filter(|_| *title == TitleScreen::Showing) else {
        return;
    };
    let news = title_news(notice.as_deref());
    let want = (news.clone(), view.later);
    if view.news_for.as_ref() == Some(&want) && !scale.is_changed() {
        return;
    }
    view.news_for = Some(want);
    for old in [view.news.take(), view.corner.take()].into_iter().flatten() {
        commands.entity(old).despawn();
    }
    let layer = commands
        .spawn((
            Name::new("title news"),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    view.news = Some(layer);
    // the corner: the version, and under it what the updater has to say, on the art inside its
    // painted frame
    let mut node = look.panel_node();
    node.position_type = PositionType::Absolute;
    node.left = Val::Percent(NEWS_LEFT);
    node.bottom = Val::Percent(NEWS_BOTTOM);
    node.width = Val::Percent(BUTTONS_WIDTH);
    node.padding = UiRect::all(look.px(8.0));
    node.row_gap = look.px(5.0);
    // its lines centred
    node.align_items = AlignItems::Center;
    let offer = news.as_ref().is_some_and(|n| n.offer);
    let on = view.art.unwrap_or(layer);
    let corner = if offer {
        commands.spawn((look.news_box(node), ChildOf(on))).id()
    } else {
        commands.spawn((look.panel(node), ChildOf(on))).id()
    };
    view.corner = Some(corner);
    commands.entity(corner).with_children(|p| {
        p.spawn(look.text(Face::Narrow, 13.0, palette::GOLD, format!("Version v{}", env!("CARGO_PKG_VERSION"))));
        match &news {
            Some(n) => {
                let colour = if n.failed { palette::RED } else { palette::GOLD };
                p.spawn(look.text(Face::Plain, 14.0, colour, n.line.clone()));
                if n.progress {
                    p.spawn((look.text(Face::Narrow, 13.0, palette::VALUE, ""), UpdateProgressText));
                }
                if let Some(enabled) = n.button {
                    look.button(p, "Update", ButtonStyle::Text, enabled, None)
                        .insert(TitleAction::Update);
                }
            }
            None => {
                p.spawn(look.text(Face::Plain, 13.0, palette::GOLD, "Up to date"));
            }
        }
    });
    // the pop-up: a newer version, until Update or Later
    let Some(n) = news.filter(|n| n.offer && !view.later) else {
        return;
    };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(palette::DIM),
            look::solid(),
            ChildOf(layer),
        ))
        .with_children(|dim| {
            let mut node = look.panel_node();
            node.width = look.px(POPUP_WIDTH);
            node.align_items = AlignItems::Center;
            node.padding = UiRect::all(look.px(16.0));
            node.row_gap = look.px(10.0);
            dim.spawn(look.panel(node)).with_children(|p| {
                look.heading(p, "Update available", 20.0);
                p.spawn(look.text(Face::Plain, 12.0, palette::VALUE, n.line.clone()));
                p.spawn(look.text(
                    Face::Plain,
                    10.0,
                    palette::LABEL,
                    "The game downloads it, closes, updates and starts again. Your heroes and settings are kept.",
                ));
                p.spawn(Node {
                    column_gap: look.px(10.0),
                    margin: UiRect::top(look.px(4.0)),
                    ..default()
                })
                .with_children(|row| {
                    look.button(row, "Update", ButtonStyle::Primary, true, Some(120.0))
                        .insert(TitleAction::Update);
                    look.button(row, "Later", ButtonStyle::Text, true, Some(120.0))
                        .insert(TitleAction::Later);
                });
            });
        });
}

/// The version corner's place on the art, inside its frame, as shares of its size: its left edge
/// under the buttons' panel's, and as wide.
const NEWS_LEFT: f32 = BUTTONS_LEFT;
const NEWS_BOTTOM: f32 = 5.0;

/// The update pop-up's width, in units.
const POPUP_WIDTH: f32 = 340.0;

/// Decodes `bytes` (a JPEG or a PNG carried in the exe) as a texture.
fn texture(bytes: &[u8]) -> Option<Image> {
    let pixels = image::load_from_memory(bytes).ok()?.to_rgba8();
    let mut texture = Image::new(
        Extent3d {
            width: pixels.width(),
            height: pixels.height(),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels.into_raw(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    texture.sampler = ImageSampler::linear();
    Some(texture)
}

/// Puts the art and the emblem on, decoded the first frame they are drawn: both are small.
fn show_the_art(
    mut commands: Commands,
    mut view: ResMut<TitleView>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time>,
) {
    let (Some(art), Some(logo), None) = (view.art, view.logo, view.shown_at) else {
        return;
    };
    for (entity, bytes, what, alpha) in [(art, TITLE_ART, "art", 1.0), (logo, TITLE_LOGO, "emblem", LOGO_DIM)] {
        match texture(bytes) {
            Some(t) => {
                commands
                    .entity(entity)
                    .insert(ImageNode::new(images.add(t)).with_color(Color::srgba(1.0, 1.0, 1.0, alpha)));
            }
            None => warn!("title screen: the {what} did not decode"),
        }
    }
    view.shown_at = Some(time.elapsed_secs());
}

/// Sizes the art to the window ([`fill_size`]), from its top left: filling it when the shapes all
/// but agree, else covering it with the overflow right or below cropped, so the logo stays in
/// view; and the name with it.
fn fit_the_art(
    window: Query<&Window, With<PrimaryWindow>>,
    view: Res<TitleView>,
    mut nodes: Query<&mut Node>,
    mut fonts: Query<&mut TextFont, With<TitleName>>,
) {
    let (Ok(window), Some(art)) = (window.single(), view.art) else {
        return;
    };
    let Ok(mut node) = nodes.get_mut(art) else {
        return;
    };
    let size = fill_size(
        Vec2::new(window.width(), window.height()),
        TITLE_ART_SIZE.as_vec2(),
    );
    let (width, height) = (Val::Px(size.x), Val::Px(size.y));
    if node.width != width || node.height != height {
        node.width = width;
        node.height = height;
    }
    // the name grows with the art, as the emblem does
    let font_size = (size.y * NAME_SIZE).round();
    for mut font in &mut fonts {
        if font.font_size != font_size {
            font.font_size = font_size;
        }
    }
}

/// Enter goes on; Quit closes the game.
fn take_the_clicks(
    mut clicks: MessageReader<StockClick>,
    actions: Query<&TitleAction>,
    mut title: ResMut<TitleScreen>,
    mut exit: MessageWriter<AppExit>,
    mut presses: MessageWriter<MenuPress>,
    mut view: ResMut<TitleView>,
) {
    for StockClick(e) in clicks.read() {
        match actions.get(*e) {
            Ok(TitleAction::Enter) => *title = TitleScreen::Passed,
            Ok(TitleAction::Quit) => {
                exit.write(AppExit::Success);
            }
            Ok(TitleAction::Update) => {
                presses.write(MenuPress(MenuAction::UpdateGame));
                view.later = true;
            }
            Ok(TitleAction::Later) => view.later = true,
            Err(_) => {}
        }
    }
}

/// Once passed, the buttons and the corner go at once; the art stays while the hero is on the way
/// into the camp, until the loading screen is up under it, so the select scene's heroes never show
/// between the two. A screen of the menus (the class picker of a save with no hero) has
/// the art go at once; [`HOLD_AT_MOST_SECS`] ends a wait that never sees the world.
fn leave_the_title(
    mut commands: Commands,
    title: Res<TitleScreen>,
    mut view: ResMut<TitleView>,
    screen: Option<Res<SelectScreen>>,
    menus: Option<Res<Menus>>,
    time: Res<Time>,
) {
    if *title != TitleScreen::Passed {
        return;
    }
    let Some(root) = view.root else {
        return;
    };
    for piece in [view.buttons.take(), view.news.take()].into_iter().flatten() {
        commands.entity(piece).despawn();
    }
    let now = time.elapsed_secs();
    let since = *view.passed_at.get_or_insert(now);
    let open = screen.is_some_and(|s| s.open);
    view.saw_select |= open;
    if open {
        view.closed_frames = 0;
    } else if view.saw_select {
        view.closed_frames += 1;
    }
    // the menus have a screen of their own to show over the select scene
    let a_screen = open
        && menus.is_some_and(|m| m.screen != crate::menus::Screen::Unset && !m.on_the_way_in());
    let in_world = view.closed_frames >= COVER_FRAMES;
    if !(a_screen || in_world || now - since > HOLD_AT_MOST_SECS) {
        return;
    }
    info!("title screen: entered");
    commands.entity(root).despawn();
    view.root = None;
    view.art = None;
}

/// `ESURV_TITLE_SHOT=<png>`: a shot of the title a moment after its art shows. With
/// `ESURV_TITLE_PLAY=1` too, a dev aid, Play is pressed a second after it, to see the way into the
/// camp.
fn shoot_the_title(
    mut commands: Commands,
    mut view: ResMut<TitleView>,
    mut title: ResMut<TitleScreen>,
    time: Res<Time>,
) {
    let Some(shown_at) = view.shown_at else {
        return;
    };
    let now = time.elapsed_secs();
    if view.shot.is_none() {
        if *title == TitleScreen::Showing
            && now > shown_at + SHOT_AFTER_SECS + 1.0
            && std::env::var("ESURV_TITLE_PLAY").is_ok_and(|v| v == "1")
        {
            info!("title screen: ESURV_TITLE_PLAY presses Play");
            *title = TitleScreen::Passed;
        }
        return;
    }
    if now < shown_at + SHOT_AFTER_SECS {
        return;
    }
    let Some(path) = view.shot.take() else {
        return;
    };
    info!("title screen: writing {path}");
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The art carried in the exe decodes (the JPEG decoder is built in), at its size.
    #[test]
    fn the_splash_screen_decodes() {
        let art =
            image::load_from_memory_with_format(SPLASH_SCREEN, image::ImageFormat::Jpeg).unwrap();
        assert_eq!(UVec2::new(art.width(), art.height()), SPLASH_SIZE);
        let small = crate::splash_art::decode(UVec2::new(640, 358)).unwrap();
        assert_eq!(small.size(), UVec2::new(640, 358));
    }

    /// The title's art and its emblem decode at their sizes; the emblem's corners are keyed out
    /// and its middle, once teal, is the night's blue.
    #[test]
    fn the_title_art_and_emblem_decode() {
        let art = image::load_from_memory(TITLE_ART).unwrap();
        assert_eq!(UVec2::new(art.width(), art.height()), TITLE_ART_SIZE);
        let logo = image::load_from_memory(TITLE_LOGO).unwrap().to_rgba8();
        assert_eq!(logo.width(), logo.height(), "the emblem is square");
        assert_eq!(logo.get_pixel(4, 4)[3], 0, "its corner is keyed out");
        let teal = logo.get_pixel(logo.width() * 175 / 1024, logo.height() / 2);
        assert!(teal[3] == 255 && teal[2] > teal[1] + 15 && teal[1] > teal[0], "{teal:?} is night blue");
        assert!(texture(TITLE_LOGO).is_some());
    }

    /// The art covers the window whole: the side that fits exactly, the other cropped.
    #[test]
    fn the_art_covers_the_window() {
        let art = SPLASH_SIZE.as_vec2();
        let wide = cover_size(Vec2::new(2560.0, 1080.0), art);
        assert_eq!(wide.x, 2560.0);
        assert!(wide.y > 1080.0);
        let square = cover_size(Vec2::new(1024.0, 1024.0), art);
        assert_eq!(square.y, 1024.0);
        assert!(square.x > 1024.0);
        assert!(
            (wide.x / wide.y - art.x / art.y).abs() < 1e-4,
            "its aspect is kept"
        );
    }

    /// A scripted dev aid skips the title, `ESURV_SKIP_TITLE=1` does too, and a plain launch
    /// (a save of one's own, `ESURV_SAVE`, the shot of the title itself) does not.
    #[test]
    fn unattended_aids_skip_the_title() {
        let with = |set: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                set.iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert!(!skip_wanted(with(&[])));
        assert!(!skip_wanted(with(&[
            ("ESURV_SAVE", "x.ron"),
            ("ESURV_TITLE_SHOT", "t.png"),
            ("ESURV_HUB", "1")
        ])));
        assert!(skip_wanted(with(&[("ESURV_SKIP_TITLE", "1")])));
        assert!(!skip_wanted(with(&[("ESURV_SKIP_TITLE", "0")])));
        assert!(skip_wanted(with(&[("ESURV_HERO_SHOTS", "shots")])));
        assert!(skip_wanted(with(&[("ESURV_MENUS_SCRIPT", "a")])));
        assert!(skip_wanted(with(&[("ESURV_CAMP_START", "1,2,3")])));
    }

    /// The title shows first: what comes after it (the menus' launch rule) waits; Enter leads on,
    /// and is not seen by anything after the title.
    #[test]
    fn the_title_shows_first_and_enter_leads_on() {
        #[derive(Resource, Default)]
        struct LaunchRuleRan(u32, bool);
        fn launch_rule(mut ran: ResMut<LaunchRuleRan>, keys: Res<ButtonInput<KeyCode>>) {
            ran.0 += 1;
            ran.1 |= keys.just_pressed(KeyCode::Enter);
        }
        let mut app = App::new();
        app.insert_resource(TitleScreen::Showing)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<LaunchRuleRan>()
            .add_systems(PreUpdate, take_the_keys)
            .add_systems(Update, launch_rule.run_if(passed));
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<LaunchRuleRan>().0,
            0,
            "the title shows first"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert_eq!(
            *app.world().resource::<TitleScreen>(),
            TitleScreen::Showing,
            "Escape does nothing"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert_eq!(*app.world().resource::<TitleScreen>(), TitleScreen::Passed);
        let ran = app.world().resource::<LaunchRuleRan>();
        assert_eq!(ran.0, 1, "Enter leads on to the launch rule");
        assert!(
            !ran.1,
            "the Enter that passed the title is not seen past it"
        );
    }

    /// Without a title screen (skipped, or the menus off), the launch goes on at once.
    #[test]
    fn no_title_is_passed() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        let world = app.world_mut();
        assert!(world.run_system_cached(passed).unwrap());
    }
}
