//! The stock 1.12 look kit: the pieces every game screen but the damage meter is built from,
//! drawn with `bevy_ui` in the stock interface's own art and fonts, read at runtime from the
//! player's data.
//!
//! Spawn the pieces through [`Stock`], a system parameter; each is a bundle:
//! - [`Stock::backdrop`]: `SetBackdrop`'s tiled background and edge, in the [`Backdrop`] presets
//!   (the dialog box, the tooltip); its colours are the root's [`BackdropColours`].
//! - [`Stock::dialog_header`]: the dialog header with its title (as `GameMenuFrame`'s).
//! - [`Stock::panel_button`]: the red panel button (`UIPanelButtonTemplate2`), a [`StockButton`].
//! - [`Stock::icon`]: a spell or item icon in its action-button frame, with the square highlight.
//! - [`Stock::status_bar`]: the stock status bar, its fill a [`StatusBar`].
//! - [`Stock::text`]: a string in a stock font object ([`StockFont`]).
//! - [`Stock::centred`]: a full-screen layer that centres a screen.
//! - [`Stock::tab`]: a window's tab (`CharacterFrameTabButtonTemplate`), a [`StockTab`].
//! - [`Stock::art_button`]: a button drawn in its own art: the close button and the page
//!   arrows ([`ArtButton`]).
//! - [`Tip`]: put on a node, it shows a tooltip by the pointer while hovered ([`tip`]).
//!
//! Sizes are in the stock interface's units ([`StockScale`]), so a screen is as big as a stock
//! frame. Add a screen's content with `.with_children` on what a piece spawned; a screen's root
//! carries `benilla_app::OverlayWindow`. A [`StockButton`] lights and presses as the stock ones
//! and sends a [`StockClick`] when the mouse is let go over it. `ESURV_UI_DEMO=1` shows a
//! sample dialog ([`demo`]).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::sprite::{BorderRect, SliceScaleMode, TextureSlicer};
use bevy::text::LineBreak;
use bevy::ui_render::prelude::MaterialNode;
use bevy::window::PrimaryWindow;

use benilla_assets::{BlpLoaderSettings, BlpVariant};

use crate::ui::{art, UiFonts, GRAY_TEXT, GREEN_TEXT, RED_TEXT, TEXT_SHADOW};

mod add;
pub mod demo;
mod frame;
pub mod tip;

pub use add::AddImage;
pub use tip::{MenuLookTip, ShowTipNow, Tip, TipLine};

/// The stock interface's root is this many units tall, whatever the window's size.
pub const ROOT_HEIGHT: f32 = 768.0;
/// The scale benilla gives the stock interface (its `uiScale`).
pub const UI_SCALE: f32 = 0.9;

/// The dialog header's size, and how far it rises over its dialog and its title sits under its top.
pub const HEADER_WIDTH: f32 = 256.0;
pub const HEADER_HEIGHT: f32 = 64.0;
const HEADER_RISE: f32 = 12.0;
const HEADER_TITLE_DROP: f32 = 14.0;
/// A panel button's height (`UIPanelButtonTemplate2`).
pub const BUTTON_HEIGHT: f32 = 22.0;
/// An action button's icon, and its frame (`UI-Quickslot2`) around it.
pub const ICON_SIZE: f32 = 36.0;
const ICON_FRAME: f32 = 66.0;
/// A tab's height, and its ends kept as they are while its middle stretches (the art is 128 by
/// 32 texels: 20 at each end).
pub const TAB_HEIGHT: f32 = 32.0;
const TAB_END: f32 = 20.0;
/// The tab's glow stays this far in from each end (`UI-Character-Tab-Highlight`'s anchors).
const TAB_GLOW_INSET: f32 = 10.0;
/// What a big window lays under its dialog background, so the world does not show through.
pub const WINDOW_FILL: Color = Color::srgba(0.0, 0.0, 0.0, 0.75);
/// The close button and the page arrows are 32 units a side in the stock frames.
pub const ART_BUTTON_SIZE: f32 = 32.0;

/// How the stock interface's units become pixels: the stock root is [`ROOT_HEIGHT`] units tall,
/// scaled by [`UI_SCALE`].
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct StockScale {
    /// Logical pixels per unit.
    pub unit: f32,
    /// Physical pixels per logical pixel (the window's scale factor).
    pub device: f32,
}

impl Default for StockScale {
    fn default() -> Self {
        Self {
            unit: 1.0,
            device: 1.0,
        }
    }
}

impl StockScale {
    /// The scale for a window `height` logical pixels tall, at `device` physical pixels each.
    pub fn for_window(height: f32, device: f32) -> Self {
        let unit = if height > 0.0 {
            height / ROOT_HEIGHT * UI_SCALE
        } else {
            1.0
        };
        Self {
            unit,
            device: if device > 0.0 { device } else { 1.0 },
        }
    }

    /// `units` as a length.
    pub fn px(self, units: f32) -> Val {
        Val::Px(self.of(units))
    }

    /// `units` in logical pixels.
    pub fn of(self, units: f32) -> f32 {
        units * self.unit
    }

    /// Physical pixels per unit: how much bigger than its texels stock art shows.
    fn texel(self) -> f32 {
        self.unit * self.device
    }
}

/// The stock font objects (`Fonts.xml`): Friz Quadrata for every `GameFont*`, Arial Narrow for
/// the `NumberFont*`s, all with a black shadow one unit right and down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StockFont {
    /// 12, gold: labels, titles of dialogs.
    GameFontNormal,
    /// 12, white: hovered labels, body text.
    GameFontHighlight,
    /// 12, grey: what cannot be used.
    GameFontDisable,
    GameFontGreen,
    GameFontRed,
    /// 10, gold, white and grey.
    GameFontNormalSmall,
    GameFontHighlightSmall,
    GameFontDisableSmall,
    /// 16, gold and white.
    GameFontNormalLarge,
    GameFontHighlightLarge,
    /// 14 and 12 white, for numbers (outlined in the stock UI; a shadow here).
    NumberFontNormal,
    NumberFontNormalSmall,
    /// 14, white: a tooltip's first line (`GameTooltipHeaderText`).
    TooltipHeader,
    /// 12, white: a tooltip's lines (`GameTooltipText`).
    TooltipText,
}

impl StockFont {
    /// Its height, in units.
    pub fn height(self) -> f32 {
        match self {
            StockFont::GameFontNormalSmall
            | StockFont::GameFontHighlightSmall
            | StockFont::GameFontDisableSmall => 10.0,
            StockFont::GameFontNormalLarge | StockFont::GameFontHighlightLarge => 16.0,
            StockFont::NumberFontNormal | StockFont::TooltipHeader => 14.0,
            _ => 12.0,
        }
    }

    /// Its colour.
    pub fn colour(self) -> Color {
        match self {
            StockFont::GameFontNormal
            | StockFont::GameFontNormalSmall
            | StockFont::GameFontNormalLarge => crate::ui::GOLD_TEXT,
            StockFont::GameFontDisable | StockFont::GameFontDisableSmall => GRAY_TEXT,
            StockFont::GameFontGreen => GREEN_TEXT,
            StockFont::GameFontRed => RED_TEXT,
            _ => crate::ui::WHITE_TEXT,
        }
    }

    /// Whether it is a number font (Arial Narrow).
    pub fn is_number(self) -> bool {
        matches!(
            self,
            StockFont::NumberFontNormal | StockFont::NumberFontNormalSmall
        )
    }
}

/// The stock backdrops (`<Backdrop>` / `SetBackdrop`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backdrop {
    /// The dialog box (`GameMenuFrame`, `StaticPopup`): edge 32, tile 32, insets 11/12/12/11.
    Dialog,
    /// The tooltip (`GameTooltip`): edge 16, tile 16, insets 5, a dark blue-black background.
    Tooltip,
}

impl Backdrop {
    /// The edge's width, in units.
    pub fn edge_size(self) -> f32 {
        match self {
            Backdrop::Dialog => 32.0,
            Backdrop::Tooltip => 16.0,
        }
    }

    /// One background tile's size, in units.
    pub fn tile_size(self) -> f32 {
        self.edge_size()
    }

    /// How far the background stays in from each side: left, right, top, bottom.
    pub fn insets(self) -> [f32; 4] {
        match self {
            Backdrop::Dialog => [11.0, 12.0, 12.0, 11.0],
            Backdrop::Tooltip => [5.0; 4],
        }
    }

    /// Its stock colours (`SetBackdropColor`, `SetBackdropBorderColor`).
    pub fn colours(self) -> BackdropColours {
        match self {
            Backdrop::Dialog => BackdropColours {
                background: Color::WHITE,
                edge: Color::WHITE,
            },
            // TOOLTIP_DEFAULT_BACKGROUND_COLOR and TOOLTIP_DEFAULT_COLOR
            Backdrop::Tooltip => BackdropColours {
                background: Color::srgb(0.09, 0.09, 0.19),
                edge: Color::WHITE,
            },
        }
    }

    fn background_file(self) -> &'static str {
        match self {
            Backdrop::Dialog => art::DIALOG_BACKGROUND,
            Backdrop::Tooltip => art::TOOLTIP_BACKGROUND,
        }
    }

    fn edge_file(self) -> &'static str {
        match self {
            Backdrop::Dialog => art::DIALOG_BORDER,
            Backdrop::Tooltip => art::TOOLTIP_BORDER,
        }
    }

    /// One square of its edge file, in texels.
    fn edge_texels(self) -> u32 {
        match self {
            Backdrop::Dialog => 32,
            Backdrop::Tooltip => 16,
        }
    }
}

/// The stock background files are 64 texels a side.
const BACKGROUND_TEXELS: f32 = 64.0;

/// A backdrop's colours, on its root: change them and its background and edge follow.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct BackdropColours {
    /// Multiplies the background tile.
    pub background: Color,
    /// Multiplies the edge.
    pub edge: Color,
}

/// A backdrop's background or edge node.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum BackdropPart {
    Background,
    Edge,
}

/// A button with the stock behaviour: it lights under the pointer, presses, and sends a
/// [`StockClick`] when the mouse is let go over it; a disabled one does none of that.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[require(Button)]
pub struct StockButton {
    pub enabled: bool,
}

impl Default for StockButton {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// A [`StockButton`] was clicked (the mouse let go over it).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct StockClick(pub Entity);

/// A panel button's art and label, and the glow over a button or icon while it is hovered.
#[derive(Component)]
struct PanelButtonArt;
#[derive(Component)]
struct ButtonLabel;
#[derive(Component)]
struct HoverGlow;

/// A status bar's fill: `value` from 0 (empty) to 1 (full), in `colour`.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct StatusBar {
    pub value: f32,
    pub colour: Color,
}

#[derive(Component)]
struct StatusBarFill;

/// A window's tab. The chosen one (`selected`) shows the active art and a white label and takes
/// no click, as `PanelTemplates_SelectTab` leaves it; the others the inactive art, a gold label
/// (white under the pointer) and the tab glow.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(StockButton)]
pub struct StockTab {
    pub selected: bool,
}

#[derive(Component)]
struct TabLabel;
#[derive(Component)]
struct TabArt;

/// A button drawn in its own stock art (up, down, disabled), with an added glow under the
/// pointer.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[require(StockButton)]
pub enum ArtButton {
    /// A window's close button (`UIPanelCloseButton`).
    Close,
    /// The spellbook's page arrows.
    PreviousPage,
    NextPage,
}

impl ArtButton {
    const ALL: [ArtButton; 3] = [
        ArtButton::Close,
        ArtButton::PreviousPage,
        ArtButton::NextPage,
    ];

    /// Its art up, down and disabled, and its glow.
    fn files(self) -> [&'static str; 4] {
        match self {
            ArtButton::Close => [
                art::CLOSE_UP,
                art::CLOSE_DOWN,
                art::CLOSE_DISABLED,
                art::CLOSE_HIGHLIGHT,
            ],
            ArtButton::PreviousPage => [
                art::PREVIOUS_PAGE_UP,
                art::PREVIOUS_PAGE_DOWN,
                art::PREVIOUS_PAGE_DISABLED,
                art::PAGE_HIGHLIGHT,
            ],
            ArtButton::NextPage => [
                art::NEXT_PAGE_UP,
                art::NEXT_PAGE_DOWN,
                art::NEXT_PAGE_DISABLED,
                art::PAGE_HIGHLIGHT,
            ],
        }
    }

    fn index(self) -> usize {
        match self {
            ArtButton::Close => 0,
            ArtButton::PreviousPage => 1,
            ArtButton::NextPage => 2,
        }
    }
}

/// One [`ArtButton`]'s art, loaded once.
struct ArtButtonArt {
    up: Handle<Image>,
    down: Handle<Image>,
    disabled: Handle<Image>,
    glow: Handle<AddImage>,
}

/// The panel button's art fills this much of its 128x32 file.
const BUTTON_ART: Vec2 = Vec2::new(80.0, 22.0);
/// Its rounded ends, kept as they are while the middle stretches.
const BUTTON_END: f32 = 12.0;

/// The kit's art, loaded once.
#[derive(Resource)]
pub struct StockArt {
    dialog_background: Handle<Image>,
    tooltip_background: Handle<Image>,
    header: Handle<Image>,
    button_up: Handle<Image>,
    button_down: Handle<Image>,
    button_disabled: Handle<Image>,
    icon_frame: Handle<Image>,
    status_bar: Handle<Image>,
    button_glow: Handle<AddImage>,
    square_glow: Handle<AddImage>,
    dialog_edge: frame::EdgeFrame,
    tooltip_edge: frame::EdgeFrame,
    tab_active: Handle<Image>,
    tab_inactive: Handle<Image>,
    tab_glow: Handle<AddImage>,
    /// By [`ArtButton::index`].
    art_buttons: Vec<ArtButtonArt>,
}

/// The URL a stock texture is drawn from. The kit loads its art in benilla's sprite form (sRGB,
/// one level, clamped), so it draws in the stock colours; the marked path keeps it apart from the
/// same file loaded as world art.
pub fn stock_art_url(path: &str) -> String {
    let internal = format!("{}.blp", path.replace('\\', "/").to_ascii_lowercase());
    benilla_assets::texture_url(&internal, (false, false))
}

/// Loads a stock texture by its interface path (`Interface\Buttons\UI-Quickslot2`) to draw.
pub fn load_stock_art(assets: &AssetServer, path: &str) -> Handle<Image> {
    assets.load_with_settings::<Image, BlpLoaderSettings>(
        stock_art_url(path),
        |s: &mut BlpLoaderSettings| s.variant = BlpVariant::Sprite,
    )
}

/// Loads a spell's or item's icon by its name in the content (`Spell_Fire_Fireball`).
pub fn load_icon(assets: &AssetServer, icon: &str) -> Handle<Image> {
    load_stock_art(assets, &format!("Interface\\Icons\\{icon}"))
}

/// An edge file, kept on the CPU for the frame it is laid out into, under a path of its own.
fn load_edge_file(assets: &AssetServer, path: &str) -> Handle<Image> {
    let internal = format!("{}.blp", path.replace('\\', "/").to_ascii_lowercase());
    assets.load_with_settings::<Image, BlpLoaderSettings>(
        benilla_assets::texture_url(&internal, (false, true)),
        |s: &mut BlpLoaderSettings| s.variant = BlpVariant::Cursor,
    )
}

/// The kit: its art, scale, buttons, bars and the demo.
pub struct StockPlugin;

impl Plugin for StockPlugin {
    fn build(&self, app: &mut App) {
        add::register(app);
        let assets = app.world().resource::<AssetServer>().clone();
        let load = |path| load_stock_art(&assets, path);
        let highlight = load(art::BUTTON_HIGHLIGHT);
        let square = load(art::ICON_HIGHLIGHT);
        let button_glow = app
            .world_mut()
            .resource_mut::<Assets<AddImage>>()
            .add(AddImage::new(highlight).part(Vec2::ZERO, BUTTON_ART / Vec2::new(128.0, 32.0)));
        let square_glow = app
            .world_mut()
            .resource_mut::<Assets<AddImage>>()
            .add(AddImage::new(square));
        let tab_glow = app
            .world_mut()
            .resource_mut::<Assets<AddImage>>()
            .add(AddImage::new(load(art::TAB_HIGHLIGHT)));
        let art_buttons: Vec<ArtButtonArt> = ArtButton::ALL
            .iter()
            .map(|b| {
                let [up, down, disabled, glow] = b.files();
                let glow = app
                    .world_mut()
                    .resource_mut::<Assets<AddImage>>()
                    .add(AddImage::new(load(glow)));
                ArtButtonArt {
                    up: load(up),
                    down: load(down),
                    disabled: load(disabled),
                    glow,
                }
            })
            .collect();
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        let dialog_edge = frame::EdgeFrame::new(
            load_edge_file(&assets, Backdrop::Dialog.edge_file()),
            Backdrop::Dialog.edge_texels(),
            &mut images,
        );
        let tooltip_edge = frame::EdgeFrame::new(
            load_edge_file(&assets, Backdrop::Tooltip.edge_file()),
            Backdrop::Tooltip.edge_texels(),
            &mut images,
        );
        app.insert_resource(StockArt {
            dialog_background: load(Backdrop::Dialog.background_file()),
            tooltip_background: load(Backdrop::Tooltip.background_file()),
            header: load(art::DIALOG_HEADER),
            button_up: load(art::BUTTON_UP),
            button_down: load(art::BUTTON_DOWN),
            button_disabled: load(art::BUTTON_DISABLED),
            icon_frame: load(art::ICON_BORDER),
            status_bar: load(art::STATUS_BAR),
            button_glow,
            square_glow,
            dialog_edge,
            tooltip_edge,
            tab_active: load(art::TAB_ACTIVE),
            tab_inactive: load(art::TAB_INACTIVE),
            tab_glow,
            art_buttons,
        })
        .init_resource::<StockScale>()
        .add_message::<StockClick>()
        .add_systems(
            Update,
            (
                follow_window_scale,
                compose_edges,
                tint_backdrops,
                click_buttons,
                draw_panel_buttons,
                draw_tabs,
                draw_art_buttons,
                light_glows,
                fill_status_bars,
            )
                .chain()
                .in_set(StockSystems),
        );
        tip::register(app);
        demo::register(app);
    }
}

/// The kit's systems; a screen that spawns or changes pieces runs before them.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StockSystems;

/// What a screen spawns the kit's pieces with.
#[derive(SystemParam)]
pub struct Stock<'w> {
    fonts: Res<'w, UiFonts>,
    art: Res<'w, StockArt>,
    scale: Res<'w, StockScale>,
    assets: Res<'w, AssetServer>,
}

impl Stock<'_> {
    /// The scale the pieces are spawned at.
    pub fn scale(&self) -> StockScale {
        *self.scale
    }

    /// `units` as a length.
    pub fn px(&self, units: f32) -> Val {
        self.scale.px(units)
    }

    /// `value` in `font`.
    pub fn text(&self, font: StockFont, value: impl Into<String>) -> impl Bundle {
        self.coloured_text(font, font.colour(), value)
    }

    /// `value` in `font`, in `colour` instead of the font's own.
    pub fn coloured_text(
        &self,
        font: StockFont,
        colour: Color,
        value: impl Into<String>,
    ) -> impl Bundle {
        let unit = self.scale.unit;
        (
            Text::new(value),
            TextFont {
                font: if font.is_number() {
                    self.fonts.body.clone()
                } else {
                    self.fonts.title.clone()
                },
                font_size: font.height() * unit,
                ..default()
            },
            TextColor(colour),
            TextShadow {
                offset: Vec2::splat(unit),
                color: TEXT_SHADOW,
            },
        )
    }

    /// A stock texture by its interface path, to draw in an `ImageNode`.
    pub fn art(&self, path: &str) -> Handle<Image> {
        load_stock_art(&self.assets, path)
    }

    /// The square glow (`ButtonHilight-Square`, added) over the node it goes in, shown while the
    /// closest button around it is under the pointer.
    pub fn hover_glow(&self) -> impl Bundle {
        (
            HoverGlow,
            full_size(),
            MaterialNode(self.art.square_glow.clone()),
            Visibility::Hidden,
            Pickable::IGNORE,
        )
    }

    /// A backdrop's background and edge, as children of the node it goes on; the node's content
    /// goes in after them (`.with_children`), so it draws over the edge.
    pub fn backdrop(&self, kind: Backdrop) -> impl Bundle {
        self.tinted_backdrop(kind, kind.colours())
    }

    /// A backdrop in other colours.
    pub fn tinted_backdrop(&self, kind: Backdrop, colours: BackdropColours) -> impl Bundle {
        (
            colours,
            children![
                self.backdrop_background(kind, colours),
                self.backdrop_edge(kind, colours)
            ],
        )
    }

    /// A backdrop over a solid `fill` inside its edge: for a big window that should hide the
    /// world behind it, where the stock background alone lets it show through.
    pub fn filled_backdrop(&self, kind: Backdrop, fill: Color) -> impl Bundle {
        let colours = kind.colours();
        (
            colours,
            children![
                (self.backdrop_inside(kind), BackgroundColor(fill)),
                self.backdrop_background(kind, colours),
                self.backdrop_edge(kind, colours)
            ],
        )
    }

    /// The room inside a backdrop's edge.
    fn backdrop_inside(&self, kind: Backdrop) -> Node {
        let s = *self.scale;
        let [left, right, top, bottom] = kind.insets();
        Node {
            position_type: PositionType::Absolute,
            left: s.px(left),
            right: s.px(right),
            top: s.px(top),
            bottom: s.px(bottom),
            ..default()
        }
    }

    fn backdrop_background(&self, kind: Backdrop, colours: BackdropColours) -> impl Bundle {
        let s = *self.scale;
        let background = match kind {
            Backdrop::Dialog => &self.art.dialog_background,
            Backdrop::Tooltip => &self.art.tooltip_background,
        };
        (
            BackdropPart::Background,
            self.backdrop_inside(kind),
            ImageNode {
                image: background.clone(),
                color: colours.background,
                image_mode: NodeImageMode::Tiled {
                    tile_x: true,
                    tile_y: true,
                    stretch_value: kind.tile_size() / BACKGROUND_TEXELS * s.texel(),
                },
                ..default()
            },
        )
    }

    fn backdrop_edge(&self, kind: Backdrop, colours: BackdropColours) -> impl Bundle {
        let s = *self.scale;
        let edge = match kind {
            Backdrop::Dialog => &self.art.dialog_edge,
            Backdrop::Tooltip => &self.art.tooltip_edge,
        };
        // the edge's texels to physical pixels
        let edge_scale = kind.edge_size() / edge.square as f32 * s.texel();
        (
            BackdropPart::Edge,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            ImageNode {
                image: edge.image.clone(),
                color: colours.edge,
                image_mode: NodeImageMode::Sliced(TextureSlicer {
                    border: BorderRect::all(edge.square as f32),
                    center_scale_mode: SliceScaleMode::Stretch,
                    sides_scale_mode: SliceScaleMode::Tile {
                        stretch_value: edge_scale,
                    },
                    max_corner_scale: edge_scale,
                }),
                ..default()
            },
        )
    }

    /// The dialog header with `title` (GameFontNormal), `width` units wide ([`HEADER_WIDTH`] in
    /// the stock dialogs), centred on its dialog's top edge; spawn it as the dialog's child.
    pub fn dialog_header(&self, title: impl Into<String>, width: f32) -> impl Bundle {
        let s = *self.scale;
        (
            Node {
                position_type: PositionType::Absolute,
                top: s.px(-HEADER_RISE),
                left: Val::Percent(50.0),
                margin: UiRect::left(s.px(-width / 2.0)),
                width: s.px(width),
                height: s.px(HEADER_HEIGHT),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexStart,
                padding: UiRect::top(s.px(HEADER_TITLE_DROP)),
                ..default()
            },
            ImageNode {
                image: self.art.header.clone(),
                image_mode: NodeImageMode::Stretch,
                ..default()
            },
            children![(
                self.text(StockFont::GameFontNormal, title),
                TextLayout::new(Justify::Center, LineBreak::NoWrap),
            )],
        )
    }

    /// A red panel button, `width` units wide and [`BUTTON_HEIGHT`] tall, with `label`
    /// (GameFontNormal, white under the pointer, grey when disabled).
    pub fn panel_button(&self, label: impl Into<String>, width: f32) -> impl Bundle {
        let s = *self.scale;
        (
            StockButton::default(),
            PanelButtonArt,
            Node {
                width: s.px(width),
                height: s.px(BUTTON_HEIGHT),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ImageNode {
                image: self.art.button_up.clone(),
                rect: Some(Rect::from_corners(Vec2::ZERO, BUTTON_ART)),
                image_mode: NodeImageMode::Sliced(TextureSlicer {
                    border: BorderRect {
                        min_inset: Vec2::new(BUTTON_END, 0.0),
                        max_inset: Vec2::new(BUTTON_END, 0.0),
                    },
                    center_scale_mode: SliceScaleMode::Stretch,
                    sides_scale_mode: SliceScaleMode::Stretch,
                    max_corner_scale: s.texel(),
                }),
                ..default()
            },
            children![
                (
                    HoverGlow,
                    full_size(),
                    MaterialNode(self.art.button_glow.clone()),
                    Visibility::Hidden,
                ),
                (
                    ButtonLabel,
                    self.text(StockFont::GameFontNormal, label),
                    TextLayout::new(Justify::Center, LineBreak::NoWrap),
                ),
            ],
        )
    }

    /// The icon `icon` (its name in the content, `Spell_Fire_Fireball`) `size` units a side
    /// ([`ICON_SIZE`] on the action bars), in its frame; the square highlight shows while the
    /// closest button around it is hovered.
    pub fn icon(&self, icon: &str, size: f32) -> impl Bundle {
        self.icon_art(load_icon(&self.assets, icon), size, Color::WHITE)
    }

    /// [`Stock::icon`] for a texture named by its whole interface path
    /// (`Interface\Icons\INV_Helmet_01`, `Interface\PaperDoll\UI-PaperDoll-Slot-Head`), drawn
    /// times `tint` (white keeps it as it is; the stock interface greys what cannot be had yet).
    pub fn icon_at(&self, path: &str, size: f32, tint: Color) -> impl Bundle {
        self.icon_art(load_stock_art(&self.assets, path), size, tint)
    }

    fn icon_art(&self, image: Handle<Image>, size: f32, tint: Color) -> impl Bundle {
        let s = *self.scale;
        let frame = size * ICON_FRAME / ICON_SIZE;
        // the frame sits centred, one unit low, as `ActionButtonTemplate`'s
        let offset = (frame - size) / 2.0;
        (
            Node {
                width: s.px(size),
                height: s.px(size),
                flex_shrink: 0.0,
                ..default()
            },
            ImageNode {
                image,
                color: tint,
                ..default()
            },
            children![
                (
                    Node {
                        position_type: PositionType::Absolute,
                        left: s.px(-offset),
                        top: s.px(1.0 - offset),
                        width: s.px(frame),
                        height: s.px(frame),
                        ..default()
                    },
                    ImageNode::new(self.art.icon_frame.clone()),
                ),
                (
                    HoverGlow,
                    full_size(),
                    MaterialNode(self.art.square_glow.clone()),
                    Visibility::Hidden,
                ),
            ],
        )
    }

    /// A window's tab `width` units wide and [`TAB_HEIGHT`] tall with `label`
    /// (GameFontNormalSmall); its [`StockTab`] says whether it is the chosen one. A click on
    /// another sends a [`StockClick`].
    pub fn tab(&self, label: impl Into<String>, width: f32, selected: bool) -> impl Bundle {
        let s = *self.scale;
        (
            StockTab { selected },
            StockButton { enabled: !selected },
            Node {
                width: s.px(width),
                height: s.px(TAB_HEIGHT),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                // the label sits two units over the middle, as the template's
                padding: UiRect::bottom(s.px(4.0)),
                ..default()
            },
            children![
                (
                    TabArt,
                    Node {
                        position_type: PositionType::Absolute,
                        left: s.px(0.0),
                        right: s.px(0.0),
                        top: s.px(tab_art_top(selected)),
                        height: s.px(TAB_HEIGHT),
                        ..default()
                    },
                    ImageNode {
                        image: if selected {
                            self.art.tab_active.clone()
                        } else {
                            self.art.tab_inactive.clone()
                        },
                        image_mode: NodeImageMode::Sliced(TextureSlicer {
                            border: BorderRect {
                                min_inset: Vec2::new(TAB_END, 0.0),
                                max_inset: Vec2::new(TAB_END, 0.0),
                            },
                            center_scale_mode: SliceScaleMode::Stretch,
                            sides_scale_mode: SliceScaleMode::Stretch,
                            max_corner_scale: s.texel(),
                        }),
                        ..default()
                    },
                ),
                (
                    HoverGlow,
                    Node {
                        position_type: PositionType::Absolute,
                        left: s.px(TAB_GLOW_INSET),
                        right: s.px(TAB_GLOW_INSET),
                        top: s.px(0.0),
                        bottom: s.px(4.0),
                        ..default()
                    },
                    MaterialNode(self.art.tab_glow.clone()),
                    Visibility::Hidden,
                ),
                (
                    TabLabel,
                    self.text(tab_font(Interaction::None, selected), label),
                    TextLayout::new(Justify::Center, LineBreak::NoWrap),
                ),
            ],
        )
    }

    /// A button `size` units a side drawn in `kind`'s own art.
    pub fn art_button(&self, kind: ArtButton, size: f32) -> impl Bundle {
        let s = *self.scale;
        let art = &self.art.art_buttons[kind.index()];
        (
            kind,
            Node {
                width: s.px(size),
                height: s.px(size),
                flex_shrink: 0.0,
                ..default()
            },
            ImageNode::new(art.up.clone()),
            children![(
                HoverGlow,
                full_size(),
                MaterialNode(art.glow.clone()),
                Visibility::Hidden,
            )],
        )
    }

    /// A status bar `width` by `height` units, filled to `value` (0 to 1) in `colour` over a dark
    /// background, as the casting bar's; change its [`StatusBar`] to move it.
    pub fn status_bar(&self, colour: Color, value: f32, width: f32, height: f32) -> impl Bundle {
        let s = *self.scale;
        (
            StatusBar { value, colour },
            Node {
                width: s.px(width),
                height: s.px(height),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            children![(
                StatusBarFill,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Percent(fill_percent(value)),
                    ..default()
                },
                ImageNode {
                    image: self.art.status_bar.clone(),
                    color: colour,
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
            )],
        )
    }

    /// A layer over the whole window at `z` (a [`crate::ui::layer`]) that centres what it holds;
    /// it takes no clicks itself, so the screen it holds carries the `OverlayWindow`.
    pub fn centred(&self, z: i32) -> impl Bundle {
        (
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(z),
            Pickable::IGNORE,
        )
    }
}

/// A node filling its parent.
fn full_size() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        right: Val::Px(0.0),
        top: Val::Px(0.0),
        bottom: Val::Px(0.0),
        ..default()
    }
}

/// A fill's width, in percent of its bar.
fn fill_percent(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0) * 100.0
    } else {
        0.0
    }
}

/// Keeps the scale on the window's height.
fn follow_window_scale(window: Query<&Window, With<PrimaryWindow>>, mut scale: ResMut<StockScale>) {
    let Ok(window) = window.single() else {
        return;
    };
    let want = StockScale::for_window(window.height(), window.scale_factor());
    if *scale != want {
        *scale = want;
    }
}

/// Lays each edge file out into its frame once it has loaded.
fn compose_edges(mut art: ResMut<StockArt>, mut images: ResMut<Assets<Image>>) {
    if art.dialog_edge.ready && art.tooltip_edge.ready {
        return;
    }
    let art = &mut *art;
    for edge in [&mut art.dialog_edge, &mut art.tooltip_edge] {
        edge.compose(&mut images);
    }
}

/// A backdrop's background and edge take its colours.
fn tint_backdrops(
    roots: Query<(&BackdropColours, &Children), Changed<BackdropColours>>,
    mut parts: Query<(&mut ImageNode, &BackdropPart)>,
) {
    for (colours, children) in &roots {
        for child in children.iter() {
            if let Ok((mut image, part)) = parts.get_mut(child) {
                let want = if *part == BackdropPart::Edge {
                    colours.edge
                } else {
                    colours.background
                };
                if image.color != want {
                    image.color = want;
                }
            }
        }
    }
}

/// A [`StockClick`] for each enabled button the mouse was let go over, after pressing it.
fn click_buttons(
    buttons: Query<(Entity, &Interaction, &StockButton), Changed<Interaction>>,
    mut pressed: Local<Vec<Entity>>,
    mut clicks: MessageWriter<StockClick>,
) {
    for (entity, interaction, button) in &buttons {
        let was_pressed = pressed.contains(&entity);
        match interaction {
            Interaction::Pressed if button.enabled => {
                if !was_pressed {
                    pressed.push(entity);
                }
            }
            Interaction::Hovered if was_pressed && button.enabled => {
                clicks.write(StockClick(entity));
                pressed.retain(|e| *e != entity);
            }
            _ => pressed.retain(|e| *e != entity),
        }
    }
}

/// A panel button's look: its art up, down or disabled, its label gold, white under the pointer,
/// or grey.
fn draw_panel_buttons(
    art: Res<StockArt>,
    mut buttons: Query<(&Interaction, &StockButton, &mut ImageNode), With<PanelButtonArt>>,
    mut labels: Query<(&ChildOf, &mut TextColor), With<ButtonLabel>>,
) {
    for (interaction, button, mut image) in &mut buttons {
        let want = button_art(&art, *interaction, button.enabled);
        if image.image != *want {
            image.image = want.clone();
        }
    }
    for (parent, mut colour) in &mut labels {
        let Ok((interaction, button, _)) = buttons.get(parent.parent()) else {
            continue;
        };
        let want = label_font(*interaction, button.enabled).colour();
        if colour.0 != want {
            colour.0 = want;
        }
    }
}

fn button_art(art: &StockArt, interaction: Interaction, enabled: bool) -> &Handle<Image> {
    match (enabled, interaction) {
        (false, _) => &art.button_disabled,
        (true, Interaction::Pressed) => &art.button_down,
        _ => &art.button_up,
    }
}

/// A panel button's label font (`NormalFont`, `HighlightFont`, `DisabledFont`).
pub(crate) fn label_font(interaction: Interaction, enabled: bool) -> StockFont {
    match (enabled, interaction) {
        (false, _) => StockFont::GameFontDisable,
        (true, Interaction::None) => StockFont::GameFontNormal,
        (true, _) => StockFont::GameFontHighlight,
    }
}

/// A tab's label font: white when chosen (the template's `DisabledFont`) or under the pointer,
/// else gold; small.
pub(crate) fn tab_font(interaction: Interaction, selected: bool) -> StockFont {
    if selected || interaction != Interaction::None {
        StockFont::GameFontHighlightSmall
    } else {
        StockFont::GameFontNormalSmall
    }
}

/// The chosen tab's art rises this far over the others (the template's `LeftDisabled` anchor).
fn tab_art_top(selected: bool) -> f32 {
    if selected {
        -5.0
    } else {
        0.0
    }
}

/// A tab's look: the active art, raised, and no click when chosen, else the inactive art.
fn draw_tabs(
    art: Res<StockArt>,
    scale: Res<StockScale>,
    mut tabs: Query<(&StockTab, &mut StockButton), Changed<StockTab>>,
    all: Query<(&Interaction, &StockTab)>,
    mut arts: Query<(&ChildOf, &mut ImageNode, &mut Node), With<TabArt>>,
    mut labels: Query<(&ChildOf, &mut TextColor), With<TabLabel>>,
) {
    for (tab, mut button) in &mut tabs {
        if button.enabled == tab.selected {
            button.enabled = !tab.selected;
        }
    }
    for (parent, mut image, mut node) in &mut arts {
        let Ok((_, tab)) = all.get(parent.parent()) else {
            continue;
        };
        let want = if tab.selected {
            &art.tab_active
        } else {
            &art.tab_inactive
        };
        if image.image != *want {
            image.image = want.clone();
        }
        let top = scale.px(tab_art_top(tab.selected));
        if node.top != top {
            node.top = top;
        }
    }
    for (parent, mut colour) in &mut labels {
        let Ok((interaction, tab)) = all.get(parent.parent()) else {
            continue;
        };
        let want = tab_font(*interaction, tab.selected).colour();
        if colour.0 != want {
            colour.0 = want;
        }
    }
}

/// An art button's look: up, down while pressed, or disabled.
fn draw_art_buttons(
    art: Res<StockArt>,
    mut buttons: Query<(&Interaction, &StockButton, &ArtButton, &mut ImageNode)>,
) {
    for (interaction, button, kind, mut image) in &mut buttons {
        let a = &art.art_buttons[kind.index()];
        let want = match (button.enabled, interaction) {
            (false, _) => &a.disabled,
            (true, Interaction::Pressed) => &a.down,
            _ => &a.up,
        };
        if image.image != *want {
            image.image = want.clone();
        }
    }
}

/// A glow shows while the closest button around it is hovered or pressed (and enabled).
fn light_glows(
    mut glows: Query<(Entity, &mut Visibility), With<HoverGlow>>,
    parents: Query<&ChildOf>,
    states: Query<(&Interaction, Option<&StockButton>)>,
) {
    for (glow, mut shown) in &mut glows {
        let mut lit = false;
        for above in parents.iter_ancestors(glow) {
            if let Ok((interaction, button)) = states.get(above) {
                lit = *interaction != Interaction::None && button.is_none_or(|b| b.enabled);
                break;
            }
        }
        shown.set_if_neq(if lit {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// Status bar fills follow their bar.
fn fill_status_bars(
    bars: Query<(&StatusBar, &Children), Changed<StatusBar>>,
    mut fills: Query<(&mut Node, &mut ImageNode), With<StatusBarFill>>,
) {
    for (bar, children) in &bars {
        for child in children.iter() {
            if let Ok((mut node, mut image)) = fills.get_mut(child) {
                let width = Val::Percent(fill_percent(bar.value));
                if node.width != width {
                    node.width = width;
                }
                if image.color != bar.colour {
                    image.color = bar.colour;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_scale_with_the_window_as_the_stock_root() {
        let s = StockScale::for_window(768.0, 1.0);
        assert!(
            (s.unit - UI_SCALE).abs() < 1e-6,
            "a 768-tall window: uiScale"
        );
        let s = StockScale::for_window(1600.0 / 16.0 * 9.0, 1.0);
        assert!(
            (s.of(256.0) - 270.0).abs() < 1e-3,
            "900 tall: 256 units, 270 px"
        );
        assert_eq!(s.px(0.0), Val::Px(0.0));
        assert_eq!(StockScale::for_window(0.0, 0.0), StockScale::default());
        let hi = StockScale::for_window(900.0, 2.0);
        assert!(
            (hi.texel() - 2.0 * hi.unit).abs() < 1e-6,
            "texels on a 2x display"
        );
    }

    #[test]
    fn stock_fonts_keep_their_sizes_and_colours() {
        assert_eq!(StockFont::GameFontNormal.height(), 12.0);
        assert_eq!(StockFont::GameFontNormalSmall.height(), 10.0);
        assert_eq!(StockFont::GameFontNormalLarge.height(), 16.0);
        assert_eq!(StockFont::NumberFontNormal.height(), 14.0);
        assert_eq!(StockFont::GameFontNormal.colour(), crate::ui::GOLD_TEXT);
        assert_eq!(StockFont::GameFontHighlight.colour(), Color::WHITE);
        assert_eq!(StockFont::GameFontDisableSmall.colour(), GRAY_TEXT);
        assert!(StockFont::NumberFontNormal.is_number() && !StockFont::TooltipText.is_number());
    }

    #[test]
    fn backdrop_presets_are_the_stock_ones() {
        assert_eq!(Backdrop::Dialog.edge_size(), 32.0);
        assert_eq!(Backdrop::Dialog.insets(), [11.0, 12.0, 12.0, 11.0]);
        assert_eq!(Backdrop::Tooltip.tile_size(), 16.0);
        assert_eq!(Backdrop::Tooltip.insets(), [5.0; 4]);
        assert_eq!(
            Backdrop::Tooltip.colours().background,
            Color::srgb(0.09, 0.09, 0.19)
        );
    }

    #[test]
    fn a_button_label_is_gold_white_or_grey() {
        assert_eq!(
            label_font(Interaction::None, true),
            StockFont::GameFontNormal
        );
        assert_eq!(
            label_font(Interaction::Hovered, true),
            StockFont::GameFontHighlight
        );
        assert_eq!(
            label_font(Interaction::Hovered, false),
            StockFont::GameFontDisable
        );
    }

    #[test]
    fn a_tab_is_gold_white_under_the_pointer_and_raised_white_when_chosen() {
        assert_eq!(
            tab_font(Interaction::None, false),
            StockFont::GameFontNormalSmall
        );
        assert_eq!(
            tab_font(Interaction::Hovered, false),
            StockFont::GameFontHighlightSmall
        );
        assert_eq!(
            tab_font(Interaction::None, true),
            StockFont::GameFontHighlightSmall
        );
        assert_eq!((tab_art_top(true), tab_art_top(false)), (-5.0, 0.0));
        assert_eq!(ArtButton::ALL.map(ArtButton::index), [0, 1, 2]);
    }

    #[test]
    fn a_fill_stays_inside_its_bar() {
        assert_eq!(fill_percent(0.25), 25.0);
        assert_eq!(fill_percent(-1.0), 0.0);
        assert_eq!(fill_percent(3.0), 100.0);
        assert_eq!(fill_percent(f32::NAN), 0.0);
    }

    #[test]
    fn stock_art_has_paths_of_its_own() {
        assert_eq!(
            stock_art_url(art::BUTTON_UP),
            "mpq://interface/buttons/ui-panel-button-up@cc.blp"
        );
    }

    #[test]
    fn a_click_is_a_press_let_go_over_the_button() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<Messages<StockClick>>();
        let button = world
            .spawn((StockButton::default(), Interaction::Pressed))
            .id();
        world.run_system_once(click_buttons).unwrap();
        *world.get_mut::<Interaction>(button).unwrap() = Interaction::Hovered;
        world.run_system_once(click_buttons).unwrap();
        // a fresh system has no memory of the press: a hover alone is no click
        assert_eq!(world.resource::<Messages<StockClick>>().len(), 0);

        let mut schedule = Schedule::default();
        schedule.add_systems(click_buttons);
        *world.get_mut::<Interaction>(button).unwrap() = Interaction::Pressed;
        schedule.run(&mut world);
        *world.get_mut::<Interaction>(button).unwrap() = Interaction::Hovered;
        schedule.run(&mut world);
        let clicks: Vec<_> = world
            .resource_mut::<Messages<StockClick>>()
            .drain()
            .collect();
        assert_eq!(clicks, vec![StockClick(button)]);

        // dragged off and let go elsewhere: no click
        *world.get_mut::<Interaction>(button).unwrap() = Interaction::Pressed;
        schedule.run(&mut world);
        *world.get_mut::<Interaction>(button).unwrap() = Interaction::None;
        schedule.run(&mut world);
        *world.get_mut::<Interaction>(button).unwrap() = Interaction::Hovered;
        schedule.run(&mut world);
        assert_eq!(world.resource::<Messages<StockClick>>().len(), 0);
    }
}
