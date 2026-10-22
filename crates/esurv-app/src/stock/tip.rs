//! Tooltips in the stock look (`GameTooltip`): a node that carries a [`Tip`] shows its lines in a
//! tooltip box by the pointer while the pointer is over it, as the stock interface shows an
//! item's or a talent's tooltip. The first line is the header (`GameTooltipHeaderText`, 14), the
//! others `GameTooltipText` (12), each in its own colour; a long line wraps at the tooltip's
//! width. The box sits right of and below the pointer, kept on screen, over every window
//! ([`crate::ui::layer::TOOLTIP`]); it takes no clicks.

use bevy::prelude::*;
use bevy::text::LineBreak;
use bevy::ui::ComputedNode;
use bevy::window::PrimaryWindow;

use super::{Backdrop, Stock, StockFont, StockScale, StockSystems};
use crate::menus::look::{palette, RADIUS};
use crate::ui::layer;

/// A tooltip line wraps past this many units.
const MAX_WIDTH: f32 = 260.0;
/// A line longer than this many characters wraps in a tooltip.
const WRAPS_PAST: usize = 38;
/// The box's room inside its edge, and its distance from the pointer.
const PAD: f32 = 9.0;
const FROM_POINTER: Vec2 = Vec2::new(18.0, 14.0);

/// One line of a tooltip, in its colour.
#[derive(Clone, Debug, PartialEq)]
pub struct TipLine {
    pub text: String,
    pub colour: Color,
}

impl TipLine {
    pub fn new(text: impl Into<String>, colour: Color) -> Self {
        Self {
            text: text.into(),
            colour,
        }
    }
}

/// The tooltip a node shows while the pointer is over it; the first line is the header. An empty
/// one shows nothing.
#[derive(Component, Clone, Debug, Default, PartialEq)]
#[require(Interaction)]
pub struct Tip(pub Vec<TipLine>);

/// Put beside a [`Tip`]: its box is drawn in the menus' look (a near-black panel with a thin gold
/// border, [`crate::menus::look`]) instead of the stock tooltip's.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct MenuLookTip;

/// How far over the menus' own layer a menu's tooltip is drawn.
const TIP_OVER_MENUS: i32 = 100;

/// Put on a node with a [`Tip`] to show its tooltip as if the pointer were over it (a dev
/// script's shot of a tooltip, with nobody at the mouse).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ShowTipNow;

/// The tooltip as shown: the node it belongs to, and its root.
#[derive(Resource, Default)]
struct ShownTip {
    owner: Option<Entity>,
    root: Option<Entity>,
    lines: Vec<TipLine>,
    scale: Option<StockScale>,
}

#[derive(Component)]
struct TipRoot;

pub(super) fn register(app: &mut App) {
    app.init_resource::<ShownTip>()
        .add_systems(Update, (show_tip, place_tip).chain().after(StockSystems));
}

/// Shows the tooltip of the node under the pointer, and takes it away when there is none.
fn show_tip(
    mut commands: Commands,
    stock: Stock,
    tips: Query<(
        Entity,
        &Interaction,
        &Tip,
        &InheritedVisibility,
        Has<MenuLookTip>,
        Has<ShowTipNow>,
    )>,
    mut shown: ResMut<ShownTip>,
) {
    let hovered = tips
        .iter()
        .filter(|(_, i, tip, v, _, now)| (**i != Interaction::None || *now) && !tip.0.is_empty() && v.get())
        .map(|(e, _, tip, _, menu, _)| (e, tip, menu))
        .last();
    let menu_look = hovered.is_some_and(|(_, _, menu)| menu);
    let want = hovered.map(|(e, tip, _)| (e, tip.0.clone()));
    let same = match (&want, shown.owner) {
        (Some((e, lines)), Some(owner)) => {
            *e == owner && *lines == shown.lines && shown.scale == Some(stock.scale())
        }
        (None, None) => true,
        _ => false,
    };
    if same {
        return;
    }
    if let Some(root) = shown.root.take() {
        commands.entity(root).despawn();
    }
    shown.owner = None;
    shown.lines.clear();
    let Some((owner, lines)) = want else {
        return;
    };
    let px = |u| stock.px(u);
    let base = (
        TipRoot,
        Name::new("tooltip"),
        Node {
            position_type: PositionType::Absolute,
            max_width: px(MAX_WIDTH + 2.0 * PAD),
            // a tooltip whose lines wrap takes its full width, so its box is laid out for the
            // wrapped lines' height
            width: if lines.iter().any(|l| l.text.chars().count() > WRAPS_PAST) { px(MAX_WIDTH + 2.0 * PAD) } else { Val::Auto },
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(PAD)),
            row_gap: px(2.0),
            border: UiRect::all(px(if menu_look { 1.0 } else { 0.0 })),
            border_radius: BorderRadius::all(px(if menu_look { RADIUS } else { 0.0 })),
            ..default()
        },
        // a menu's tooltip over the menus' panels
        GlobalZIndex(if menu_look { crate::menus::MENU_LAYER + TIP_OVER_MENUS } else { layer::TOOLTIP }),
        Pickable::IGNORE,
        // placed once laid out, so it never shows a frame in the wrong place
        Visibility::Hidden,
    );
    let mut root = if menu_look {
        commands.spawn((
            base,
            BackgroundColor(palette::PANEL),
            BorderColor::all(palette::GOLD),
        ))
    } else {
        commands.spawn((base, stock.backdrop(Backdrop::Tooltip)))
    };
    let root = root
        .with_children(|tip| {
            for (i, line) in lines.iter().enumerate() {
                let font = if i == 0 {
                    StockFont::TooltipHeader
                } else {
                    StockFont::TooltipText
                };
                tip.spawn((
                    stock.coloured_text(font, line.colour, line.text.clone()),
                    TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                    Pickable::IGNORE,
                ));
            }
        })
        .id();
    shown.owner = Some(owner);
    shown.root = Some(root);
    shown.lines = lines;
    shown.scale = Some(stock.scale());
}

/// Keeps the tooltip by the pointer and on screen.
fn place_tip(
    window: Single<&Window, With<PrimaryWindow>>,
    scale: Res<StockScale>,
    mut roots: Query<(&mut Node, &ComputedNode, &mut Visibility), With<TipRoot>>,
    shown: Res<ShownTip>,
    owners: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<ShowTipNow>>,
) {
    // a tooltip shown with nobody at the mouse sits by its node's lower right corner
    let by_its_node = shown.owner.and_then(|o| owners.get(o).ok()).map(|(n, at)| {
        let scale = n.inverse_scale_factor();
        at.translation * scale + n.size() * scale / 2.0
    });
    let Some(cursor) = by_its_node.or_else(|| window.cursor_position()) else {
        return;
    };
    for (mut node, laid_out, mut visible) in &mut roots {
        let size = laid_out.size() * laid_out.inverse_scale_factor();
        if size.x <= 0.0 {
            continue;
        }
        let at = tip_place(cursor, size, window.size(), *scale);
        let (left, top) = (Val::Px(at.x), Val::Px(at.y));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        visible.set_if_neq(Visibility::Inherited);
    }
}

/// Where a tooltip `size` big goes for the pointer at `cursor`: right of and below it, or left of
/// or above it where the window ends.
pub(crate) fn tip_place(cursor: Vec2, size: Vec2, window: Vec2, scale: StockScale) -> Vec2 {
    let gap = FROM_POINTER * scale.unit;
    let mut at = cursor + gap;
    if at.x + size.x > window.x {
        at.x = cursor.x - gap.x - size.x;
    }
    if at.y + size.y > window.y {
        at.y = window.y - size.y;
    }
    at.max(Vec2::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tooltip_stays_by_the_pointer_and_on_screen() {
        let s = StockScale::default();
        let window = Vec2::new(800.0, 600.0);
        let size = Vec2::new(200.0, 100.0);
        assert_eq!(
            tip_place(Vec2::new(100.0, 100.0), size, window, s),
            Vec2::new(118.0, 114.0)
        );
        // past the right edge: left of the pointer
        assert_eq!(
            tip_place(Vec2::new(700.0, 100.0), size, window, s).x,
            700.0 - 18.0 - 200.0
        );
        // past the bottom: up against it
        assert_eq!(tip_place(Vec2::new(100.0, 590.0), size, window, s).y, 500.0);
    }
}
