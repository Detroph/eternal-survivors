//! The menus' own look (menus.md, "The look"), drawn in code: near-black
//! panels with a thin warm-gold border and rounded corners; headings in Morpheus, gold, in small
//! capitals between two thin gold rules; small all-caps grey labels over larger white or gold
//! values, values in rounded bordered boxes; icons in square frames with a soft glow on the
//! selected one, the locked ones dark with a "?"; gold arrow buttons; framed text buttons, the
//! active one filled dark red. Only the fonts, icons and portraits come from the player's own
//! data; no art file of ours.
//!
//! Sizes are in the stock interface's units ([`StockScale`]), as the stock kit's, so the menus
//! grow with the window. A screen spawns the pieces through [`MenuLook`]; a piece that acts is a
//! [`MenuButton`] (a [`crate::stock::StockButton`] for its clicks) carrying its
//! [`super::MenuAction`].

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::stock::{StockButton, StockScale};
use crate::ui::{UiFonts, TEXT_SHADOW};

/// The menus' colours, and the game's own frames' (the camp's talk window, the run's portrait):
/// the splash screen's (the ui rework, `plans/ui-rework-concept.md`, "The palette"): blue is the
/// room, bronze is the furniture, bordeaux is the cloth on what is chosen, gold is the light.
pub mod palette {
    use bevy::prelude::Color;

    /// The darkest night blue: the cards' bodies, tooltips.
    pub const NIGHT_DEEP: Color = Color::srgb(0.043, 0.082, 0.153);
    /// A level-up card's body and the choice screen's side columns: darker than a panel, so
    /// the cards stand out of it.
    pub const CARD: Color = Color::srgba(0.027, 0.055, 0.110, 0.97);
    /// A panel's fill: the night sky, the world faint behind it.
    pub const PANEL: Color = Color::srgba(0.051, 0.129, 0.212, 0.94);
    /// A value box's and a button's fill, a step lighter (Night Glow).
    pub const INSET: Color = Color::srgba(0.0, 0.169, 0.357, 0.9);
    /// A hovered button's fill (Night Blue).
    pub const NIGHT_BLUE: Color = Color::srgb(0.0, 0.275, 0.455);
    /// What dims the world behind the cards and the windows over it: the night, not black.
    pub const DIM: Color = Color::srgba(0.043, 0.082, 0.153, 0.65);
    /// Every frame's and border's bronze, and its shadow line.
    pub const BRONZE: Color = Color::srgb(0.573, 0.455, 0.380);
    pub const BRONZE_DARK: Color = Color::srgb(0.353, 0.263, 0.212);
    /// The picture's own thin wine edge, outside the bronze.
    pub const FRAME_WINE: Color = Color::srgb(0.267, 0.102, 0.169);
    /// The rules', the bars' and the call to act's gold (the vortex's ring).
    pub const GOLD: Color = Color::srgb(0.910, 0.694, 0.431);
    /// The border of what is not picked: bronze, never gold.
    pub const GOLD_DIM: Color = BRONZE;
    pub const GOLD_CLEAR: Color = Color::srgba(0.910, 0.694, 0.431, 0.0);
    /// A frame's edge under the pointer (Gold Deep).
    pub const GOLD_DEEP: Color = Color::srgb(0.698, 0.478, 0.247);
    /// The deep shadow of gold: the call to act's edge (Ember).
    pub const EMBER: Color = Color::srgb(0.388, 0.212, 0.078);
    /// The text on the gold plate.
    pub const ON_GOLD: Color = Color::srgb(0.102, 0.071, 0.024);
    /// A heading's gold, and a gold value's (Gold Light).
    pub const HEADING: Color = Color::srgb(0.953, 0.827, 0.604);
    /// A label's sand.
    pub const LABEL: Color = Color::srgb(0.788, 0.635, 0.478);
    /// A value's parchment, in place of white.
    pub const VALUE: Color = Color::srgb(0.937, 0.890, 0.800);
    /// A hint's sand, quieter than a label.
    pub const HINT: Color = Color::srgb(0.620, 0.431, 0.290);
    /// The active button's bordeaux cloth, and its edge.
    pub const ACTIVE: Color = Color::srgb(0.263, 0.071, 0.110);
    pub const ACTIVE_EDGE: Color = Color::srgb(0.451, 0.169, 0.220);
    /// A pressed button (Wine Black).
    pub const WINE_BLACK: Color = Color::srgb(0.176, 0.051, 0.098);
    /// The blue of a talk option's `<bracketed action>` (Timeways Blue): the one blue text.
    pub const ACTION_BLUE: Color = Color::srgb(0.290, 0.639, 1.0);
    /// The carved bronze of the title's letters (the logo on the splash): the lit face, its
    /// highlight, and its dark cut edge; the title screen's box and buttons wear them.
    pub const CARVED: Color = Color::srgb(0.651, 0.459, 0.318);
    pub const CARVED_LIGHT: Color = Color::srgb(0.851, 0.659, 0.471);
    pub const CARVED_DARK: Color = Color::srgb(0.231, 0.141, 0.086);
    /// The carved letters' shadow side, the box's fill.
    pub const CARVED_SHADE: Color = Color::srgb(0.380, 0.247, 0.157);
    /// The soft glow round what is picked.
    pub const GLOW: Color = Color::srgba(0.910, 0.694, 0.431, 0.7);
    /// What is not found or not open yet is drawn this dark.
    pub const LOCKED: Color = Color::srgb(0.22, 0.22, 0.22);
    /// Points to spend.
    pub const GREEN: Color = Color::srgb(0.25, 0.92, 0.30);
    pub const RED: Color = Color::srgb(0.95, 0.25, 0.20);
    /// A dimmed choice (a race the class may not be).
    pub const DIMMED: Color = Color::srgba(1.0, 1.0, 1.0, 0.28);
}

/// A panel's border width, its corner radius and its padding, in units.
pub const BORDER: f32 = 1.0;
pub const RADIUS: f32 = 0.0;
pub const PAD: f32 = 12.0;
/// A panel's bronze band, and the studs on its corners, in units.
pub const FRAME: f32 = 2.0;
pub const STUD: f32 = 6.0;

/// Where every window that is not the whole screen puts its top (its title), units from the
/// screen's top; it grows down from there.
pub const WINDOW_TOP: f32 = 64.0;

/// The faces the menus write in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// Cinzel: headings and names.
    Display,
    /// Friz Quadrata: labels, values, buttons.
    Plain,
    /// Arial Narrow: numbers in small places.
    Narrow,
}

/// How a button shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonStyle {
    /// A framed text button.
    Text,
    /// The framed text button that is on (a tab chosen, a sex picked): filled dark red.
    Active,
    /// A framed text button with news behind it (the warband screen's Collection and Talents):
    /// the gold border and the soft glow of what is picked, the glow
    /// pulsing ([`news_pulse`]).
    TextGlow,
    /// A plate of the title's carved bronze (the title screen's Play and Quit).
    Carved,
    /// The call to act on a screen (Create, Start run): gold-framed, a little larger.
    Primary,
    /// An icon in its square frame.
    Icon,
    /// The icon picked: the soft glow round it.
    IconPicked,
    /// A gold arrow, or another bare glyph.
    Arrow,
    /// A row of a list (a zone): a panel-like box.
    Row,
    /// The row picked.
    RowPicked,
}

/// A button of the menus: its look follows the pointer, and a disabled one is dimmed. Its clicks
/// come as the kit's `StockClick` (it is a [`StockButton`]).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[require(StockButton, BackgroundColor, BorderColor)]
pub struct MenuButton {
    pub style: ButtonStyle,
    pub enabled: bool,
}

/// A node that keeps the pointer to itself (a panel): the camp's figures under it are not
/// pointed at.
#[derive(Bundle)]
pub struct Solid {
    interaction: Interaction,
    focus: FocusPolicy,
}

pub fn solid() -> Solid {
    Solid {
        interaction: Interaction::default(),
        focus: FocusPolicy::Block,
    }
}

/// A direction for [`MenuLook::arrow`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrow {
    Left,
    Right,
    Up,
    Down,
}

/// What a screen spawns the menus' pieces with.
#[derive(SystemParam)]
pub struct MenuLook<'w> {
    fonts: Res<'w, UiFonts>,
    scale: Res<'w, StockScale>,
    assets: Res<'w, AssetServer>,
}

impl MenuLook<'_> {
    pub fn scale(&self) -> StockScale {
        *self.scale
    }

    /// `units` as a length.
    pub fn px(&self, units: f32) -> Val {
        self.scale.px(units)
    }

    /// `units` in logical pixels.
    pub fn of(&self, units: f32) -> f32 {
        self.scale.of(units)
    }

    pub fn assets(&self) -> &AssetServer {
        &self.assets
    }

    fn font(&self, face: Face) -> Handle<Font> {
        match face {
            Face::Display => self.fonts.cinzel.clone(),
            Face::Plain => self.fonts.title.clone(),
            Face::Narrow => self.fonts.body.clone(),
        }
    }

    /// `value` in `face` at `size` units, in `colour`, with the shadow every menu text has.
    pub fn text(
        &self,
        face: Face,
        size: f32,
        colour: Color,
        value: impl Into<String>,
    ) -> impl Bundle {
        (
            Text::new(value),
            self.text_font(face, size),
            TextColor(colour),
            TextShadow {
                offset: Vec2::splat(self.of(1.0)),
                color: TEXT_SHADOW,
            },
        )
    }

    pub fn text_font(&self, face: Face, size: f32) -> TextFont {
        TextFont {
            font: self.font(face),
            font_size: self.of(size),
            ..default()
        }
    }

    /// A small all-caps grey label.
    pub fn label(&self, value: &str) -> impl Bundle {
        self.text(Face::Plain, 8.5, palette::LABEL, value.to_uppercase())
    }

    /// A value under a label: white, larger.
    pub fn value(&self, value: impl Into<String>) -> impl Bundle {
        self.text(Face::Plain, 12.0, palette::VALUE, value)
    }

    /// A quiet line at a column's foot.
    pub fn hint(&self, value: impl Into<String>) -> impl Bundle {
        self.text(Face::Plain, 9.0, palette::HINT, value)
    }

    /// The panel's node: a column with the border, the corners and the padding.
    pub fn panel_node(&self) -> Node {
        Node {
            flex_direction: FlexDirection::Column,
            border: UiRect::all(self.px(BORDER)),
            border_radius: BorderRadius::all(self.px(RADIUS)),
            padding: UiRect::all(self.px(PAD)),
            row_gap: self.px(6.0),
            ..default()
        }
    }

    /// A panel: the night fill in a bronze band with the picture's thin wine edge outside it,
    /// square, a bronze stud at each corner; it keeps the pointer.
    pub fn panel(&self, node: Node) -> impl Bundle {
        let mut node = node;
        node.border = UiRect::all(self.px(FRAME));
        node.border_radius = BorderRadius::ZERO;
        (
            node,
            BackgroundColor(palette::PANEL),
            lit_from_above(false),
            bevel(palette::BRONZE, false),
            Outline::new(self.px(1.0), Val::ZERO, palette::FRAME_WINE),
            BoxShadow::new(
                Color::srgba(0.0, 0.0, 0.0, 0.6),
                Val::ZERO,
                self.px(2.0),
                Val::ZERO,
                self.px(8.0),
            ),
            solid(),
            children![
                crate::splash_art::cut(crate::splash_art::Cut::Night, NIGHT_TEXTURE),
                self.stud(true, true),
                self.stud(true, false),
                self.stud(false, true),
                self.stud(false, false),
            ],
        )
    }

    /// A round bronze stud on a panel's corner (`top`, `left`), centred on its band.
    fn stud(&self, top: bool, left: bool) -> impl Bundle {
        let at = self.px(-(FRAME + STUD) / 2.0);
        (
            Node {
                position_type: PositionType::Absolute,
                width: self.px(STUD),
                height: self.px(STUD),
                top: if top { at } else { Val::Auto },
                bottom: if top { Val::Auto } else { at },
                left: if left { at } else { Val::Auto },
                right: if left { Val::Auto } else { at },
                border: UiRect::all(self.px(1.0)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(palette::BRONZE),
            BorderColor::all(palette::BRONZE_DARK),
            Pickable::IGNORE,
        )
    }

    /// A panel with news in it (the update notice's offer): the gold border and glow of a
    /// [`ButtonStyle::TextGlow`] button, pulsing as it does ([`NewsPulse`]).
    pub fn news_box(&self, node: Node) -> impl Bundle {
        (
            node,
            BackgroundColor(palette::PANEL),
            BorderColor::all(palette::HEADING),
            BoxShadow::new(palette::GLOW.with_alpha(0.6), Val::ZERO, Val::ZERO, self.px(1.0), self.px(7.0)),
            NewsPulse,
            solid(),
        )
    }

    /// A thin gold rule that fades out at its far end (`toward` the heading it leads to: its
    /// bright end). It grows to fill its row.
    pub fn rule(&self, bright_end: Arrow) -> impl Bundle {
        let stops = vec![
            ColorStop::auto(palette::GOLD_CLEAR),
            ColorStop::auto(palette::GOLD),
        ];
        let angle = match bright_end {
            Arrow::Right => LinearGradient::TO_RIGHT,
            Arrow::Left => LinearGradient::TO_LEFT,
            Arrow::Down => LinearGradient::TO_BOTTOM,
            Arrow::Up => LinearGradient::TO_TOP,
        };
        (
            Node {
                flex_grow: 1.0,
                height: self.px(1.0),
                min_width: self.px(12.0),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::new(angle, stops)),
        )
    }

    /// A thin gold line across a column, fading out at both ends.
    pub fn divider(&self) -> impl Bundle {
        (
            Node {
                width: Val::Percent(100.0),
                height: self.px(1.0),
                flex_shrink: 0.0,
                margin: UiRect::vertical(self.px(2.0)),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::to_right(vec![
                ColorStop::auto(palette::GOLD_CLEAR),
                ColorStop::auto(palette::GOLD),
                ColorStop::auto(palette::GOLD_CLEAR),
            ])),
        )
    }

    /// A heading: `title` in Morpheus, gold, in small capitals, centred between two rules.
    pub fn heading(&self, parent: &mut ChildSpawnerCommands, title: &str, size: f32) {
        parent
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: self.px(8.0),
                width: Val::Percent(100.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn(self.rule(Arrow::Right));
                self.small_caps(row, title, size, palette::HEADING);
                row.spawn(self.rule(Arrow::Left));
            });
    }

    /// `title` in Morpheus small capitals: each word's first letter at `size`, the rest capitals
    /// at four fifths of it.
    pub fn small_caps(
        &self,
        parent: &mut ChildSpawnerCommands,
        title: &str,
        size: f32,
        colour: Color,
    ) -> Entity {
        let shadow = TextShadow {
            offset: Vec2::splat(self.of(1.0)),
            color: TEXT_SHADOW,
        };
        let mut root = parent.spawn((
            Text::default(),
            self.text_font(Face::Display, size),
            TextColor(colour),
            shadow,
            TextLayout::new_with_no_wrap(),
        ));
        root.with_children(|spans| {
            for (i, word) in title.split(' ').enumerate() {
                let mut chars = word.chars();
                let first = chars.next().map(|c| c.to_uppercase().collect::<String>());
                let rest = chars.as_str().to_uppercase();
                let gap = if i > 0 { " " } else { "" };
                spans.spawn((
                    TextSpan::new(format!("{gap}{}", first.unwrap_or_default())),
                    self.text_font(Face::Display, size),
                    TextColor(colour),
                ));
                if !rest.is_empty() {
                    spans.spawn((
                        TextSpan::new(rest),
                        self.text_font(Face::Display, size * 0.8),
                        TextColor(colour),
                    ));
                }
            }
        });
        root.id()
    }

    /// A value box: a small grey label over a value, in a rounded bordered box.
    pub fn value_box(
        &self,
        parent: &mut ChildSpawnerCommands,
        label: &str,
        value: &str,
        colour: Color,
    ) -> Entity {
        parent
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    padding: UiRect::axes(self.px(6.0), self.px(5.0)),
                    row_gap: self.px(2.0),
                    border: UiRect::all(self.px(BORDER)),
                    border_radius: BorderRadius::ZERO,
                    ..default()
                },
                BackgroundColor(palette::INSET),
                BorderColor::all(palette::GOLD_DIM),
            ))
            .with_children(|b| {
                b.spawn(self.label(label));
                b.spawn(self.text(Face::Plain, 12.0, colour, value));
            })
            .id()
    }

    /// The node of a button of `style`, `width` units wide (`None`: as wide as its content).
    fn button_node(&self, style: ButtonStyle, width: Option<f32>) -> Node {
        let (pad_x, pad_y) = match style {
            ButtonStyle::Primary => (16.0, 7.0),
            ButtonStyle::Carved => (16.0, 9.0),
            ButtonStyle::Icon | ButtonStyle::IconPicked | ButtonStyle::Arrow => (0.0, 0.0),
            ButtonStyle::Row | ButtonStyle::RowPicked => (8.0, 5.0),
            _ => (10.0, 4.0),
        };
        Node {
            width: width.map_or(Val::Auto, |w| self.px(w)),
            padding: UiRect::axes(self.px(pad_x), self.px(pad_y)),
            border: UiRect::all(self.px(BORDER)),
            border_radius: BorderRadius::ZERO,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        }
    }

    /// A framed text button: `label` in it, `width` units wide (`None`: its label's width).
    /// The caller inserts its action.
    pub fn button<'a>(
        &self,
        parent: &'a mut ChildSpawnerCommands,
        label: &str,
        style: ButtonStyle,
        enabled: bool,
        width: Option<f32>,
    ) -> EntityCommands<'a> {
        let size = if matches!(style, ButtonStyle::Primary | ButtonStyle::Carved) {
            13.0
        } else {
            11.0
        };
        let colour = label_colour(style, enabled);
        let mut e = parent.spawn((
            self.button_node(style, width),
            MenuButton { style, enabled },
            StockButton { enabled },
        ));
        e.with_children(|b| {
            let mut text = b.spawn((
                self.text(if matches!(style, ButtonStyle::Primary | ButtonStyle::Carved) { Face::Display } else { Face::Plain }, size, colour, label),
                Pickable::IGNORE,
            ));
            // dark letters on the gold: a dark shadow under them only smears them
            if matches!(style, ButtonStyle::Primary) {
                text.remove::<TextShadow>();
            }
        });
        e
    }

    /// A framed button of `style` with nothing in it yet: the caller puts its content in
    /// (`.with_children`) and its action.
    pub fn frame_button<'a>(
        &self,
        parent: &'a mut ChildSpawnerCommands,
        style: ButtonStyle,
        enabled: bool,
        width: Option<f32>,
    ) -> EntityCommands<'a> {
        parent.spawn((
            self.button_node(style, width),
            MenuButton { style, enabled },
            StockButton { enabled },
        ))
    }

    /// An icon in its square frame, `size` units; `image` drawn whole or by its `rect` (pixels);
    /// a locked one dark with a "?" over it. A button when `button` is set.
    #[allow(clippy::too_many_arguments)]
    pub fn icon<'a>(
        &self,
        parent: &'a mut ChildSpawnerCommands,
        image: Handle<Image>,
        rect: Option<Rect>,
        size: f32,
        picked: bool,
        locked: bool,
        dimmed: bool,
    ) -> EntityCommands<'a> {
        let style = if picked {
            ButtonStyle::IconPicked
        } else {
            ButtonStyle::Icon
        };
        let mut node = self.button_node(style, Some(size));
        node.height = self.px(size);
        node.padding = UiRect::all(self.px(2.0));
        let tint = if locked {
            palette::LOCKED
        } else if dimmed {
            palette::DIMMED
        } else {
            Color::WHITE
        };
        let mut e = parent.spawn((
            node,
            MenuButton {
                style,
                enabled: true,
            },
        ));
        e.with_children(|f| {
            let mut img = ImageNode::new(image).with_color(tint);
            img.rect = rect;
            f.spawn((
                img,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::ZERO,
                    ..default()
                },
                Pickable::IGNORE,
            ));
            if locked {
                f.spawn((
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
                .with_children(|q| {
                    q.spawn(self.text(Face::Display, size * 0.5, palette::LABEL, "?"));
                });
            }
        });
        e
    }

    /// A gold arrow button pointing `dir`, `size` units square: a chevron drawn as two borders of
    /// a turned square.
    pub fn arrow<'a>(
        &self,
        parent: &'a mut ChildSpawnerCommands,
        dir: Arrow,
        size: f32,
        enabled: bool,
    ) -> EntityCommands<'a> {
        let turn = match dir {
            Arrow::Left => -45.0_f32,
            Arrow::Up => 45.0,
            Arrow::Right => 135.0,
            Arrow::Down => -135.0,
        };
        let colour = if enabled {
            palette::HEADING
        } else {
            palette::LOCKED
        };
        let mut e = parent.spawn((
            Node {
                width: self.px(size),
                height: self.px(size),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            MenuButton {
                style: ButtonStyle::Arrow,
                enabled,
            },
            StockButton { enabled },
        ));
        e.with_children(|a| {
            a.spawn((
                Node {
                    width: self.px(size * 0.42),
                    height: self.px(size * 0.42),
                    border: UiRect {
                        left: self.px(2.0),
                        top: self.px(2.0),
                        ..default()
                    },
                    ..default()
                },
                BorderColor::all(colour),
                UiTransform::from_rotation(Rot2::degrees(turn)),
                Pickable::IGNORE,
            ));
        });
        e
    }

    /// A row of `count` dots, the one at `current` filled gold.
    pub fn dots(&self, parent: &mut ChildSpawnerCommands, count: usize, current: usize) {
        // a long row of dots stays readable: past 14 they shrink
        let size = if count > 14 { 3.0 } else { 4.5 };
        parent
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                column_gap: self.px(size * 0.6),
                row_gap: self.px(size * 0.6),
                ..default()
            })
            .with_children(|row| {
                for i in 0..count {
                    let on = i == current;
                    row.spawn((
                        Node {
                            width: self.px(size),
                            height: self.px(size),
                            border: UiRect::all(self.px(0.75)),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BackgroundColor(if on { palette::HEADING } else { Color::NONE }),
                        BorderColor::all(if on {
                            palette::HEADING
                        } else {
                            palette::GOLD_DIM
                        }),
                    ));
                }
            });
    }

    /// A stock texture of the player's own data by its interface path.
    pub fn art(&self, path: &str) -> Handle<Image> {
        crate::stock::load_stock_art(&self.assets, path)
    }

    /// A spell's or item's icon by its name.
    pub fn icon_art(&self, icon: &str) -> Handle<Image> {
        crate::stock::load_icon(&self.assets, icon)
    }
}

/// A framed text button's label colour: sand while disabled, dark on the gold plate, gold light
/// on the active one, parchment on the others.
pub fn label_colour(style: ButtonStyle, enabled: bool) -> Color {
    match (enabled, style) {
        (false, _) => palette::HINT,
        (true, ButtonStyle::Primary) => palette::ON_GOLD,
        (true, ButtonStyle::Carved) => palette::CARVED_DARK,
        (true, ButtonStyle::Active | ButtonStyle::TextGlow) => palette::HEADING,
        (true, _) => palette::VALUE,
    }
}

/// How strongly a panel shows the night sky cut from the key art under its content
/// ([`crate::splash_art`]).
pub const NIGHT_TEXTURE: f32 = 0.3;

/// Volume: a face lit from above, a
/// sheen at its top fading to a shade at its foot, over its fill; `pressed`, the other way up.
pub fn lit_from_above(pressed: bool) -> BackgroundGradient {
    let light = Color::srgba(1.0, 0.95, 0.85, 0.16);
    let shade = Color::srgba(0.0, 0.0, 0.0, 0.32);
    let (top, foot) = if pressed { (shade, light.with_alpha(0.05)) } else { (light, shade) };
    BackgroundGradient::from(LinearGradient::to_bottom(vec![
        ColorStop::new(top, Val::Percent(0.0)),
        ColorStop::new(Color::NONE, Val::Percent(45.0)),
        ColorStop::new(foot, Val::Percent(100.0)),
    ]))
}

/// A cut edge of `edge`: lit on its top and left, shaded on its bottom and right; `pressed`, the
/// other way round.
pub fn bevel(edge: Color, pressed: bool) -> BorderColor {
    let light = edge.mix(&palette::GOLD_CLEAR, 0.45);
    let dark = edge.mix(&Color::BLACK, 0.55);
    let (lit, shaded) = if pressed { (dark, light) } else { (light, dark) };
    BorderColor {
        top: lit,
        left: lit,
        bottom: shaded,
        right: shaded,
    }
}

/// Whether a button of `style` stands out of its panel ([`bevel`], [`lit_from_above`]): all but
/// an icon's frame and a bare arrow.
fn raised(style: ButtonStyle) -> bool {
    !matches!(style, ButtonStyle::Icon | ButtonStyle::IconPicked | ButtonStyle::Arrow)
}

/// Each menu button's fill, border and glow, as the pointer finds it (and again when it is
/// enabled or disabled).
#[allow(clippy::type_complexity)]
pub(super) fn light_buttons(
    mut buttons: Query<
        (
            &MenuButton,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            Option<&mut BoxShadow>,
            Entity,
        ),
        Or<(Changed<Interaction>, Changed<MenuButton>)>,
    >,
    scale: Res<StockScale>,
    mut commands: Commands,
) {
    for (b, interaction, mut fill, mut edge, shadow, entity) in &mut buttons {
        let hovered = b.enabled && *interaction != Interaction::None;
        let pressed = b.enabled && *interaction == Interaction::Pressed;
        let (f, e, glow) = button_colours(b.style, hovered, pressed, b.enabled);
        fill.0 = f;
        // a lit face in a cut edge, pressed in when pressed; not round an icon or an arrow
        if raised(b.style) {
            *edge = bevel(e, pressed);
            commands.entity(entity).insert(lit_from_above(pressed));
        } else {
            *edge = BorderColor::all(e);
        }
        let want = glow.map(|g| {
            BoxShadow::new(
                g,
                Val::ZERO,
                Val::ZERO,
                scale.px(1.0),
                scale.px(if pressed { 4.0 } else { 7.0 }),
            )
        });
        match (shadow, want) {
            (Some(mut s), Some(w)) => *s = w,
            (Some(_), None) => {
                commands.entity(entity).remove::<BoxShadow>();
            }
            (None, Some(w)) => {
                commands.entity(entity).insert(w);
            }
            (None, None) => {}
        }
    }
}

/// A box with news in it (the update notice's "vX is available"): its border gold and its glow
/// pulsing as a [`ButtonStyle::TextGlow`] button's does ([`MenuLook::news_box`]).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NewsPulse;

/// How long one beat of the news glow takes (seconds), and how far its strength dips.
pub const NEWS_PULSE_PERIOD: f32 = 1.6;
pub const NEWS_PULSE_DEPTH: f32 = 0.6;

/// The news glow's strength at `seconds`: 1 at the top of each beat, dipping to
/// `1 - NEWS_PULSE_DEPTH` halfway through it.
pub fn news_pulse(seconds: f32) -> f32 {
    let beat = (seconds / NEWS_PULSE_PERIOD).fract();
    1.0 - NEWS_PULSE_DEPTH * (0.5 - 0.5 * (beat * std::f32::consts::TAU).cos())
}

/// Every news glow beats: a [`ButtonStyle::TextGlow`] button's (on top of what the pointer made
/// it, [`light_buttons`]) and a [`NewsPulse`] box's, their strength and reach together.
#[allow(clippy::type_complexity)]
pub(super) fn pulse_news_glows(
    time: Res<Time>,
    scale: Res<StockScale>,
    mut glows: Query<(&mut BoxShadow, Option<&MenuButton>, Option<&Interaction>, Has<NewsPulse>), Or<(With<MenuButton>, With<NewsPulse>)>>,
) {
    let pulse = news_pulse(time.elapsed_secs());
    for (mut shadow, button, interaction, boxed) in &mut glows {
        let hovered = match button {
            Some(b) if b.style == ButtonStyle::TextGlow => b.enabled && interaction.is_some_and(|i| *i != Interaction::None),
            Some(_) if !boxed => continue,
            _ => false,
        };
        let Some(s) = shadow.0.first_mut() else { continue };
        let strength = if hovered { 0.85 } else { 0.6 };
        let want = palette::GLOW.with_alpha(strength * pulse);
        if s.color != want {
            s.color = want;
            s.blur_radius = scale.px(4.0 + 5.0 * pulse);
        }
    }
}

/// A button's fill, border and glow.
fn button_colours(
    style: ButtonStyle,
    hovered: bool,
    pressed: bool,
    enabled: bool,
) -> (Color, Color, Option<Color>) {
    let bright = |c: Color, by: f32| c.mix(&Color::WHITE, by);
    match style {
        // the bordeaux cloth of what is chosen
        ButtonStyle::Active => (
            if pressed {
                palette::WINE_BLACK
            } else if hovered {
                bright(palette::ACTIVE, 0.06)
            } else {
                palette::ACTIVE
            },
            palette::ACTIVE_EDGE,
            None,
        ),
        // the title's carved bronze: its lit face, lighter under the pointer, its dark cut edge
        ButtonStyle::Carved => (
            if pressed {
                palette::CARVED_SHADE
            } else if hovered {
                palette::CARVED_LIGHT
            } else {
                palette::CARVED
            },
            palette::CARVED_DARK,
            hovered.then_some(palette::GLOW.with_alpha(0.4)),
        ),
        // the gold plate, the one call to act
        ButtonStyle::Primary => (
            if !enabled {
                palette::GOLD_DEEP.with_alpha(0.4)
            } else if pressed {
                palette::GOLD_DEEP
            } else if hovered {
                bright(palette::GOLD, 0.15)
            } else {
                palette::GOLD
            },
            palette::EMBER,
            (enabled && hovered).then_some(palette::GLOW.with_alpha(0.5)),
        ),
        ButtonStyle::Text | ButtonStyle::Row => (
            if pressed {
                palette::WINE_BLACK
            } else if hovered {
                palette::NIGHT_BLUE
            } else {
                palette::INSET
            },
            if hovered {
                palette::GOLD_DEEP
            } else {
                palette::BRONZE
            },
            None,
        ),
        ButtonStyle::TextGlow => (
            if pressed {
                palette::WINE_BLACK
            } else if hovered {
                palette::NIGHT_BLUE
            } else {
                palette::INSET
            },
            palette::HEADING,
            Some(palette::GLOW.with_alpha(if hovered { 0.85 } else { 0.6 })),
        ),
        ButtonStyle::RowPicked => (
            palette::NIGHT_BLUE,
            palette::GOLD,
            Some(palette::GLOW.with_alpha(0.35)),
        ),
        ButtonStyle::Icon => (
            palette::NIGHT_DEEP,
            if hovered {
                palette::GOLD_DEEP
            } else {
                palette::BRONZE
            },
            hovered.then_some(palette::GLOW.with_alpha(0.35)),
        ),
        ButtonStyle::IconPicked => (palette::NIGHT_DEEP, palette::HEADING, Some(palette::GLOW)),
        ButtonStyle::Arrow => (
            Color::NONE,
            Color::NONE,
            hovered.then_some(palette::GLOW.with_alpha(0.3)),
        ),
    }
}

/// An icon in a thin gold frame that only shows (no button): the achievements' rows and banner.
pub fn plain_icon(parent: &mut ChildSpawnerCommands, look: &MenuLook, image: Handle<Image>, size: f32) {
    parent
        .spawn((
            Node {
                width: look.px(size),
                height: look.px(size),
                flex_shrink: 0.0,
                padding: UiRect::all(look.px(2.0)),
                border: UiRect::all(look.px(BORDER)),
                border_radius: BorderRadius::ZERO,
                ..default()
            },
            BackgroundColor(palette::INSET),
            BorderColor::all(palette::GOLD_DIM),
            Pickable::IGNORE,
        ))
        .with_children(|f| {
            f.spawn((
                ImageNode::new(image),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::ZERO,
                    ..default()
                },
                Pickable::IGNORE,
            ));
        });
}

/// An achievement's points on a small shield: a box rounded deeper at its foot, gold when
/// `bright` (earned), grey otherwise.
pub fn points_shield(parent: &mut ChildSpawnerCommands, look: &MenuLook, points: u32, bright: bool) {
    let (edge, ink) = if bright {
        (palette::HEADING, palette::HEADING)
    } else {
        (palette::LOCKED, palette::LABEL)
    };
    parent
        .spawn((
            Node {
                width: look.px(30.0),
                height: look.px(34.0),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(look.px(1.5)),
                border_radius: BorderRadius {
                    top_left: look.px(3.0),
                    top_right: look.px(3.0),
                    bottom_left: look.px(15.0),
                    bottom_right: look.px(15.0),
                },
                ..default()
            },
            BackgroundColor(palette::INSET),
            BorderColor::all(edge),
            Pickable::IGNORE,
        ))
        .with_children(|s| {
            s.spawn((look.text(Face::Display, 13.0, ink, points.to_string()), Pickable::IGNORE));
        });
}

/// A progress bar filled `share` (0 to 1) gold, `words` ("312 / 1,000") over it, `height` units.
pub fn progress_bar(parent: &mut ChildSpawnerCommands, look: &MenuLook, share: f32, words: &str, height: f32) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: look.px(height),
                border: UiRect::all(look.px(BORDER)),
                border_radius: BorderRadius::ZERO,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            BorderColor::all(palette::GOLD_DIM),
            Pickable::IGNORE,
        ))
        .with_children(|bar| {
            bar.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::ZERO,
                    top: Val::ZERO,
                    bottom: Val::ZERO,
                    width: Val::Percent(share.clamp(0.0, 1.0) * 100.0),
                    ..default()
                },
                BackgroundColor(palette::GOLD.with_alpha(0.75)),
                Pickable::IGNORE,
            ));
            bar.spawn((look.text(Face::Plain, (height * 0.7).min(10.0), palette::VALUE, words), Pickable::IGNORE));
        });
}

/// A small square mark: filled gold when `on`, an empty frame otherwise (a checklist's items, the
/// tracking box).
pub fn mark(parent: &mut ChildSpawnerCommands, look: &MenuLook, on: bool, size: f32) {
    parent.spawn((
        Node {
            width: look.px(size),
            height: look.px(size),
            flex_shrink: 0.0,
            border: UiRect::all(look.px(BORDER)),
            border_radius: BorderRadius::ZERO,
            ..default()
        },
        BackgroundColor(if on { palette::HEADING } else { Color::NONE }),
        BorderColor::all(if on { palette::HEADING } else { palette::LABEL }),
        Pickable::IGNORE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The news glow beats: full at the top of each beat, at its dip halfway, the same each beat.
    #[test]
    fn the_news_glow_beats() {
        assert!((news_pulse(0.0) - 1.0).abs() < 1e-5);
        assert!((news_pulse(NEWS_PULSE_PERIOD / 2.0) - (1.0 - NEWS_PULSE_DEPTH)).abs() < 1e-5);
        assert!((news_pulse(0.3) - news_pulse(0.3 + NEWS_PULSE_PERIOD)).abs() < 1e-4);
        for i in 0..100 {
            let p = news_pulse(i as f32 * 0.037);
            assert!((1.0 - NEWS_PULSE_DEPTH - 1e-5..=1.0 + 1e-5).contains(&p), "{p}");
        }
    }

    /// The talk options' blue here is the one the server writes into them.
    #[test]
    fn the_action_blue_is_the_servers() {
        let hex = palette::ACTION_BLUE.to_srgba().to_hex();
        assert_eq!(hex.trim_start_matches('#').to_lowercase(), esurv_protocol::ACTION_BLUE_HEX);
    }

    /// A news button and a news box wear the gold border and a glow at rest.
    #[test]
    fn news_wears_the_gold_border_and_a_glow() {
        let (_, edge, glow) = button_colours(ButtonStyle::TextGlow, false, false, true);
        assert_eq!(edge, palette::HEADING);
        assert!(glow.is_some());
        assert_eq!(button_colours(ButtonStyle::Text, false, false, true).2, None);
    }
}
