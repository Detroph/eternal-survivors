//! The HUD drawn with the stock look kit ([`crate::stock`]), in the stock interface's units so it
//! is as big as the stock frames at every window size. Placed clear of the stock frames: the
//! readouts and the boss bar at the top centre, above the stock error line (`UIErrorsFrame`, 122
//! units down); the banners and toasts below the stock centre lines (`RaidWarningFrame` and
//! `RaidBossEmoteFrame`, where the run's centre-screen lines show); the Escape menu in the middle,
//! where the stock game menu would be, its look in [`super::escape_menu_look`].

use bevy::prelude::*;
use bevy::text::LineBreak;

use crate::stock::{Stock, StockFont, StockScale};
use crate::ui::{layer, UiFonts, GOLD_TEXT, RED_TEXT, WHITE_TEXT};

use super::lines::LineLook;

/// How many banners and toasts show at once.
pub(super) const BANNER_SLOTS: usize = 2;
pub(super) const TOAST_SLOTS: usize = 3;

// Where the parts sit, in units down from the top of the screen. The stock centre lines take
// 122 to 282: `UIErrorsFrame` (60 tall at 122), then 10 below it `RaidWarningFrame` (40 tall),
// then 10 below that `RaidBossEmoteFrame` (40 tall).
const READOUTS_TOP: f32 = 2.0;
const BANNERS_TOP: f32 = 292.0;
const TOASTS_TOP: f32 = 352.0;

/// `GameFontNormalHuge` (20; the kit's fonts go up to 16): the clock and the banners, as the
/// stock raid warning's lines.
const HUGE_FONT: f32 = 20.0;
/// A ribbon side's cell, across: the kills on the left, the gold on the right.
const READOUT_SIDE_WIDTH: f32 = 110.0;
/// The kills' skull and the gold's coin, a side.
const READOUT_ICON: f32 = 14.0;
/// Our own skull and coin, as the mock-up drew them, 64 square.
const KILLS_ICON: &[u8] = include_bytes!("../../assets/hud_skull.png");
const GOLD_ICON: &[u8] = include_bytes!("../../assets/hud_coin.png");

/// An embedded PNG as a texture, or an empty one when it does not decode.
fn icon_image(png: &[u8]) -> Image {
    let Ok(art) = image::load_from_memory_with_format(png, image::ImageFormat::Png) else {
        return Image::default();
    };
    let pixels = art.to_rgba8();
    let mut texture = Image::new(
        bevy::render::render_resource::Extent3d {
            width: pixels.width(),
            height: pixels.height(),
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        pixels.into_raw(),
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    texture.sampler = bevy::image::ImageSampler::linear();
    texture
}
/// How far down the crest the clock starts.
const CLOCK_DROP: f32 = 6.0;
/// The top box's labels, the menus' bronze-light, so its numbers (kills, clock, gold, the cache's
/// price) stand out in gold.
const TOP_LABEL: Color = crate::menus::look::palette::LABEL;
/// The stock centre lines' width.
const LINE_WIDTH: f32 = 512.0;

/// The stock raid warning's colour (`ChatTypeInfo["RAID_WARNING"]`).
const WARNING_ORANGE: Color = Color::srgb(1.0, 0.28, 0.0);
/// The stock info message's yellow (`UI_INFO_MESSAGE` in `UIErrorsFrame`).
const INFO_YELLOW: Color = Color::srgb(1.0, 1.0, 0.0);

/// The colour a line takes by its look.
pub(super) fn line_colour(look: LineLook) -> Color {
    match look {
        LineLook::Toast => INFO_YELLOW,
        LineLook::BossComes => WARNING_ORANGE,
        LineLook::FinalBossComes => RED_TEXT,
        LineLook::BossDown => GOLD_TEXT,
    }
}

/// One of the HUD's root nodes.
#[derive(Component)]
pub(super) struct HudRoot;

/// The HUD's parts, found again by entity, and the scale they were built at.
#[derive(Resource)]
pub(super) struct HudParts {
    pub scale: StockScale,
    pub roots: Vec<Entity>,
    pub readouts: Entity,
    pub clock: Entity,
    pub next_boss: Entity,
    pub kills: Entity,
    pub gold: Entity,
    pub cache: Entity,
    /// The bosses' bars' root, drawn by [`super::boss_stack`].
    pub boss: Entity,
    pub banners: [Entity; BANNER_SLOTS],
    pub toasts: [Entity; TOAST_SLOTS],
    pub hint: Entity,
    pub menu: Entity,
    /// The menu's line under its buttons: what the Options panel's Okay left for later.
    pub menu_notice: Entity,
    /// The menu's faces (heading and hint) and buttons, shown with the menu that has them.
    pub menu_faces: Vec<(super::escape_menu::MenuKind, Entity)>,
    pub menu_buttons: Vec<(super::escape_menu::MenuButton, Entity)>,
    pub menu_end_button: Entity,
}

/// Builds the HUD at the kit's scale, every part hidden.
pub(super) fn build(
    commands: &mut Commands,
    stock: &Stock,
    fonts: &UiFonts,
    look: &crate::menus::look::MenuLook,
    images: &mut Assets<Image>,
) -> HudParts {
    let scale = stock.scale();
    let u = |v: f32| stock.px(v);
    // `GameFontNormalHuge`'s size on a kit text
    let huge = TextFont {
        font: fonts.title.clone(),
        font_size: scale.of(HUGE_FONT),
        ..default()
    };
    let one_line = |justify| TextLayout::new(justify, LineBreak::NoWrap);
    // a root across the screen's width at `top`, its children centred across it: in a column
    // (one under the other, centred) or in a row (side by side, tops in line, `gap` apart)
    let root = |commands: &mut Commands, name: &str, top: f32, row_gap: Option<f32>| {
        let mut node = Node {
            position_type: PositionType::Absolute,
            top: u(top),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        };
        if let Some(gap) = row_gap {
            node.flex_direction = FlexDirection::Row;
            node.align_items = AlignItems::FlexStart;
            node.column_gap = u(gap);
        }
        commands
            .spawn((
                HudRoot,
                Name::new(name.to_owned()),
                node,
                GlobalZIndex(layer::HUD),
                Pickable::IGNORE,
                Visibility::Hidden,
            ))
            .id()
    };

    // ── the readouts: kills on the ribbon's left, the clock and next boss on the crest, gold and
    // the cache's price on the ribbon's right ──
    let readouts = root(commands, "hud readouts", READOUTS_TOP, Some(0.0));
    // a ribbon side's cell: its number beside its icon, its small line under them
    let cell = Node {
        width: u(READOUT_SIDE_WIDTH),
        height: u(super::top_box::RIBBON_HEIGHT - READOUTS_TOP),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    };
    let number_row = Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: u(4.0),
        ..default()
    };
    let skull = images.add(icon_image(KILLS_ICON));
    let coin = images.add(icon_image(GOLD_ICON));
    let icon = |image: Handle<Image>| {
        (
            ImageNode::new(image),
            Node {
                width: u(READOUT_ICON),
                height: u(READOUT_ICON),
                flex_shrink: 0.0,
                ..default()
            },
        )
    };
    let mut kills = Entity::PLACEHOLDER;
    let mut clock = Entity::PLACEHOLDER;
    let mut next_boss = Entity::PLACEHOLDER;
    let mut gold = Entity::PLACEHOLDER;
    let mut cache = Entity::PLACEHOLDER;
    commands.entity(readouts).with_children(|row| {
        // the ribbon and the crest, hung from the top of the screen, under them, their shadow's
        // margin round them
        let size = super::top_box::IMAGE_SIZE;
        row.spawn((
            Name::new("hud top box"),
            Node {
                position_type: PositionType::Absolute,
                top: u(-READOUTS_TOP),
                left: Val::Percent(50.0),
                margin: UiRect::left(u(-size.x / 2.0)),
                width: u(size.x),
                height: u(size.y),
                ..default()
            },
            ImageNode::new(images.add(super::top_box::image(scale))),
            ZIndex(-1),
            Pickable::IGNORE,
        ));
        row.spawn(cell.clone()).with_children(|c| {
            c.spawn(number_row.clone()).with_children(|r| {
                r.spawn(icon(skull));
                kills = r
                    .spawn((stock.text(StockFont::NumberFontNormal, ""), one_line(Justify::Left)))
                    .insert(TextColor(GOLD_TEXT))
                    .id();
            });
            c.spawn((
                stock.text(StockFont::GameFontNormalSmall, super::state::KILLS_LABEL),
                one_line(Justify::Center),
            ))
            .insert(TextColor(TOP_LABEL));
        });
        row.spawn(Node {
            width: u(super::top_box::CREST_HALF * 2.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::top(u(CLOCK_DROP)),
            ..default()
        })
        .with_children(|c| {
            clock = c
                .spawn((
                    stock.text(StockFont::GameFontHighlightLarge, ""),
                    one_line(Justify::Center),
                ))
                .insert((huge.clone(), TextColor(GOLD_TEXT)))
                .id();
            next_boss = c
                .spawn((
                    stock.text(StockFont::GameFontNormalSmall, ""),
                    one_line(Justify::Center),
                ))
                .insert(TextColor(TOP_LABEL))
                .id();
        });
        row.spawn(cell).with_children(|c| {
            c.spawn(number_row).with_children(|r| {
                gold = r
                    .spawn((stock.text(StockFont::NumberFontNormal, ""), one_line(Justify::Right)))
                    .insert(TextColor(GOLD_TEXT))
                    .id();
                r.spawn(icon(coin));
            });
            cache = c
                .spawn((
                    stock.text(StockFont::GameFontNormalSmall, ""),
                    one_line(Justify::Center),
                ))
                .insert(TextColor(GOLD_TEXT))
                .id();
        });
    });

    // ── the bosses' bars: drawn by `boss_stack` as the bosses come and go ──
    let boss = root(commands, "hud boss bars", super::boss_stack::STACK_TOP, None);

    // ── banners, toasts and the start hint: centred lines that fade ──
    // `n` lines in `GameFontNormalLarge` (16: `ErrorFont`, as `UIErrorsFrame`'s), or in
    // `GameFontNormalHuge` (20, as `RaidWarningFrame`'s) when `big`
    let lines = |commands: &mut Commands, name: &str, top: f32, n: usize, big: bool| {
        let parent = root(commands, name, top, None);
        let mut slots = Vec::with_capacity(n);
        commands.entity(parent).with_children(|c| {
            for _ in 0..n {
                let mut line = c.spawn((
                    stock.coloured_text(StockFont::GameFontNormalLarge, WHITE_TEXT, ""),
                    TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                    Node {
                        max_width: u(LINE_WIDTH),
                        ..default()
                    },
                    Visibility::Hidden,
                ));
                if big {
                    line.insert(huge.clone());
                }
                slots.push(line.id());
            }
        });
        (parent, slots)
    };
    let (banner_root, banner_slots) =
        lines(commands, "hud banners", BANNERS_TOP, BANNER_SLOTS, true);
    let (toast_root, toast_slots) = lines(commands, "hud toasts", TOASTS_TOP, TOAST_SLOTS, false);
    let (hint_root, hint_slots) = lines(commands, "hud start hint", BANNERS_TOP, 1, false);
    commands.entity(hint_slots[0]).insert(TextColor(GOLD_TEXT));

    // ── the Escape menu, in a look of its own ──
    let menu_parts = super::escape_menu_look::build(commands, stock, look);
    let menu = menu_parts.root;

    HudParts {
        scale,
        roots: vec![readouts, boss, banner_root, toast_root, hint_root, menu],
        readouts,
        clock,
        next_boss,
        kills,
        gold,
        cache,
        boss,
        banners: banner_slots.try_into().expect("banner slots"),
        toasts: toast_slots.try_into().expect("toast slots"),
        hint: hint_slots[0],
        menu,
        menu_notice: menu_parts.notice,
        menu_faces: menu_parts.faces,
        menu_buttons: menu_parts.buttons,
        menu_end_button: menu_parts.end_button,
    }
}
