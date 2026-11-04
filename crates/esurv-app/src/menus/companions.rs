//! The companions in the menus (docs/game/screens.md, "The Shop" and "The Collection tab"): the
//! Achievements window's Shop page (the companion upgrade, the companions for sale and those only
//! achievements give, a picked tile's details with the companion turning and Buy, asked first),
//! and the hero sheet's Companions (the Collection's Companion box on the left, its picker in the
//! details). The turning companion is benilla's creature pane ([`benilla_app::CreatureModel`]),
//! driven here from what the menus show.

use bevy::prelude::*;

use benilla_app::{CreatureModel, CreatureModelView};
use esurv_protocol::Achievements;

use super::look::{palette, ButtonStyle, Face, MenuLook};
use super::scroll::{scrolling, ScrollingList};
use super::{Ctx, MenuAction, Menus, Screen, Sheet, SheetMode, SheetTab};
use crate::achievements::shop::{self, Shelf, ShopTile, TileState};
use crate::achievements::AchievementsPage;
use crate::stock::{MenuLookTip, Tip, TipLine};
use crate::ui::GameContent;

/// Sizes, in units.
const TILE_WIDTH: f32 = 128.0;
const TILE_ICON: f32 = 30.0;
const DETAILS_WIDTH: f32 = 250.0;
const PREVIEW: f32 = 210.0;
const PICK_ICON: f32 = 34.0;
/// How fast the turning companion turns, radians a second.
const TURN_SPEED: f32 = 0.7;

/// A tile's tooltip: its name, its text, how it stands.
fn tile_tip(t: &ShopTile) -> Vec<TipLine> {
    let mut lines = vec![
        TipLine::new(t.name.clone(), palette::VALUE),
        TipLine::new(t.text.clone(), palette::HEADING),
    ];
    let state = shop::state_words(t);
    if !state.is_empty() {
        let ink = match t.state {
            TileState::Owned => palette::GREEN,
            TileState::TooDear | TileState::NeedsRank(_) | TileState::Needs(_) => palette::RED,
            _ => palette::LABEL,
        };
        lines.push(TipLine::new(state, ink));
    }
    lines
}

/// The Shop's tabs, left to right.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ShopTab {
    /// The companion upgrade (and, later, the banishes and skips).
    #[default]
    Upgrades,
    /// The companions.
    Pets,
    Tabards,
    Enchants,
    Decorations,
}

impl ShopTab {
    pub const ALL: [ShopTab; 5] = [
        ShopTab::Upgrades,
        ShopTab::Pets,
        ShopTab::Tabards,
        ShopTab::Enchants,
        ShopTab::Decorations,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ShopTab::Upgrades => "Upgrades",
            ShopTab::Pets => "Pets",
            ShopTab::Tabards => "Tabards",
            ShopTab::Enchants => "Enchants",
            ShopTab::Decorations => "Decorations",
        }
    }

    /// The tabs the Shop shows: Decorations only while camp decorations are on
    /// ([`crate::achievements::cosmetics::CAMP_DECORATIONS`]; parked for the hub) or the walkable
    /// camp is, whose spots take them.
    pub fn shown() -> impl Iterator<Item = ShopTab> {
        ShopTab::ALL.into_iter().filter(|t| {
            *t != ShopTab::Decorations || crate::achievements::cosmetics::CAMP_DECORATIONS || crate::camp::hub_on()
        })
    }

    /// The tab a dev script names (`shop-tab:pets`).
    pub fn named(word: &str) -> Option<ShopTab> {
        ShopTab::ALL
            .into_iter()
            .find(|t| t.label().eq_ignore_ascii_case(word))
    }

    /// Whether it shows shop section `s`: the upgrades', or the companions'.
    fn holds(self, s: &shop::ShopSection) -> bool {
        match self {
            ShopTab::Upgrades => s.shelf == Shelf::Upgrades,
            ShopTab::Pets => s.shelf == Shelf::Pets,
            ShopTab::Tabards => s.shelf == Shelf::Tabards,
            ShopTab::Enchants => s.shelf == Shelf::Enchants,
            ShopTab::Decorations => s.shelf == Shelf::Decorations,
        }
    }
}

/// The Shop: its tabs, the tab's tiles scrolling, and the details of the one picked.
pub(super) fn shop(
    body: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    told: &Achievements,
    base: ScrollingList,
) {
    let sections = shop::sections(ctx.content, ctx.facts.zones_discovered(), told);
    let tab = ctx.shop_tab;
    // each tab keeps its own place
    let list = match base {
        ScrollingList::Achievements(at) => ScrollingList::Achievements(at + tab as u8),
        other => other,
    };
    body.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        flex_basis: Val::ZERO,
        row_gap: look.px(8.0),
        min_height: Val::ZERO,
        ..default()
    })
    .with_children(|col| {
        col.spawn(Node {
            column_gap: look.px(6.0),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|tabs| {
            for t in ShopTab::shown() {
                let style = if t == tab {
                    ButtonStyle::Active
                } else {
                    ButtonStyle::Text
                };
                look.button(tabs, t.label(), style, true, Some(100.0))
                    .insert(MenuAction::ShopTab(t));
            }
        });
        col.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: Val::ZERO,
                row_gap: look.px(6.0),
                min_height: Val::ZERO,
                overflow: Overflow::scroll_y(),
                padding: UiRect::right(look.px(8.0)),
                ..default()
            },
            scrolling(list),
        ))
        .with_children(|l| shop_tab(l, look, ctx, tab, &sections));
    });
    shop_details(body, look, ctx, told);
}

/// A tab's sections, or "Coming soon" for one still to come.
fn shop_tab(
    l: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    tab: ShopTab,
    sections: &[shop::ShopSection],
) {
    {
        let _ = ctx;
        let hint = match tab {
            ShopTab::Pets => "Everything bought is the whole warband's, whatever timeline each hero came from. Each hero picks its companion on its hero sheet (Equipment).",
            ShopTab::Tabards => "Everything bought is the whole warband's; each hero picks its tabard on its hero sheet (Equipment). It shows over any armour.",
            ShopTab::Enchants => "Everything bought is the whole warband's; each hero picks one glow on its hero sheet (Equipment), shown on all its weapons.",
            // in the walkable camp, set out on its spots
            ShopTab::Decorations if crate::camp::hub_on() => "Everything bought or earned is the whole warband's; set it out on a gold ring in the camp (stand on it, Decorate, F): the camp's pieces by the fire, a tent's at any hero's tent.",
            ShopTab::Decorations => "Everything bought or earned is the whole warband's; set it out with Decorate camp, beside the camp switch. Each camp is decorated apart.",
            ShopTab::Upgrades => "Everything bought is the whole warband's, a rank at a time.",
        };
        l.spawn((
            look.hint(hint),
            Node {
                flex_shrink: 0.0,
                ..default()
            },
        ));
        for s in sections.iter().filter(|s| tab.holds(s)) {
            l.spawn((
                look.label(&s.heading),
                Node {
                    flex_shrink: 0.0,
                    margin: UiRect::top(look.px(4.0)),
                    ..default()
                },
            ));
            l.spawn(Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: look.px(5.0),
                row_gap: look.px(5.0),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|grid| {
                for t in &s.tiles {
                    shop_tile(grid, look, t, ctx.shop_pick == Some(t.item.as_str()));
                }
            });
        }
    }
}

/// One tile: its icon, name and price or state; picking it shows its details.
fn shop_tile(grid: &mut ChildSpawnerCommands, look: &MenuLook, t: &ShopTile, picked: bool) {
    let style = if picked {
        ButtonStyle::RowPicked
    } else {
        ButtonStyle::Row
    };
    let mut b = look.frame_button(grid, style, true, Some(TILE_WIDTH));
    b.insert(Node {
        width: look.px(TILE_WIDTH),
        column_gap: look.px(6.0),
        align_items: AlignItems::Center,
        padding: UiRect::all(look.px(4.0)),
        border: UiRect::all(look.px(super::look::BORDER)),
        border_radius: BorderRadius::all(look.px(4.0)),
        ..default()
    });
    b.insert((
        MenuAction::ShopPick(t.item.clone()),
        Tip(tile_tip(t)),
        MenuLookTip,
    ));
    let dark = matches!(
        t.state,
        TileState::FromAchievement(_) | TileState::NeedsRank(_) | TileState::EarnedBy(_) | TileState::Needs(_)
    );
    b.with_children(|b| {
        let tint = if dark { palette::LOCKED } else { Color::WHITE };
        b.spawn((
            ImageNode::new(look.icon_art(&t.icon)).with_color(tint),
            Node {
                width: look.px(TILE_ICON),
                height: look.px(TILE_ICON),
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(look.px(2.0)),
                ..default()
            },
            Pickable::IGNORE,
        ));
        b.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                min_width: Val::ZERO,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|c| {
            let ink = if dark { palette::LABEL } else { palette::VALUE };
            c.spawn((
                look.text(Face::Plain, 9.5, ink, t.name.clone()),
                Pickable::IGNORE,
            ));
            let (words, ink) = match &t.state {
                TileState::Owned => ("Owned".to_owned(), palette::GREEN),
                TileState::TooDear => (shop::state_words(t), palette::RED),
                TileState::ForSale => (shop::state_words(t), palette::HEADING),
                TileState::FromAchievement(_) => ("Achievement".to_owned(), palette::HINT),
                TileState::NeedsRank(_) => (shop::state_words(t), palette::HINT),
                TileState::EarnedBy(_) => ("Earned".to_owned(), palette::HINT),
                TileState::Needs(_) => (shop::state_words(t), palette::HINT),
            };
            c.spawn((look.text(Face::Narrow, 9.0, ink, words), Pickable::IGNORE));
        });
    });
}

/// The details of the tile picked: the companion turning (the upgrade's icon), its name, text and
/// how it stands, and Buy, asked first.
fn shop_details(body: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, told: &Achievements) {
    body.spawn(Node {
        flex_direction: FlexDirection::Column,
        width: look.px(DETAILS_WIDTH),
        flex_shrink: 0.0,
        row_gap: look.px(6.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|d| {
        look.heading(d, "Details", 15.0);
        let Some(t) = ctx.shop_pick.and_then(|i| shop::tile(ctx.content, ctx.facts.zones_discovered(), told, i)) else {
            d.spawn(look.hint(match ctx.shop_tab {
                ShopTab::Tabards => "Pick a tabard to see what it is and who may wear it.",
                ShopTab::Enchants => "Pick a weapon glow to see it. A hero shows one glow on all its weapons.",
                ShopTab::Decorations => {
                    "Pick a piece for the camp. Trophies are never sold: they come with achievements and clears."
                }
                _ => {
                    "Pick a companion to see it, or a rank of the companion upgrade. Companions follow \
                     their hero in runs and bring back the gold out of reach."
                }
            }));
            return;
        };
        if t.display.is_some() {
            preview(d, look, ctx, PREVIEW);
        } else {
            d.spawn((
                ImageNode::new(look.icon_art(&t.icon)),
                Node {
                    width: look.px(64.0),
                    height: look.px(64.0),
                    margin: UiRect::vertical(look.px(12.0)),
                    ..default()
                },
                Pickable::IGNORE,
            ));
        }
        d.spawn((
            look.text(Face::Plain, 13.0, palette::HEADING, t.name.clone()),
            Pickable::IGNORE,
        ));
        d.spawn((
            look.text(Face::Plain, 10.0, palette::VALUE, t.text.clone()),
            TextLayout::new_with_justify(Justify::Center),
            Pickable::IGNORE,
        ));
        let state = shop::state_words(&t);
        if !state.is_empty() {
            let ink = match t.state {
                TileState::Owned => palette::GREEN,
                TileState::TooDear | TileState::NeedsRank(_) | TileState::Needs(_) => palette::RED,
                _ => palette::LABEL,
            };
            d.spawn((look.text(Face::Plain, 10.5, ink, state), Pickable::IGNORE));
        }
        if let TileState::FromAchievement(_) | TileState::EarnedBy(_) = t.state {
            d.spawn(look.hint("Never sold: earned as it says."));
        }
        if t.state == TileState::TooDear {
            d.spawn(look.hint(format!("You have {} Moments to spend.", told.balance)));
        }
        if ctx.shop_confirm && t.buyable() {
            d.spawn((
                look.text(
                    Face::Plain,
                    11.0,
                    palette::VALUE,
                    format!("Spend {} Moments on {}?", t.price.unwrap_or(0), t.name),
                ),
                TextLayout::new_with_justify(Justify::Center),
                Pickable::IGNORE,
            ));
            d.spawn(Node {
                column_gap: look.px(8.0),
                ..default()
            })
            .with_children(|row| {
                look.button(row, "Yes", ButtonStyle::Primary, true, Some(80.0))
                    .insert(MenuAction::ShopAnswer(true));
                look.button(row, "No", ButtonStyle::Text, true, Some(80.0))
                    .insert(MenuAction::ShopAnswer(false));
            });
        } else if t.state != TileState::Owned && !matches!(t.state, TileState::FromAchievement(_) | TileState::EarnedBy(_)) {
            let label = match t.price {
                Some(p) => format!("Buy for {p} Moments"),
                None => "Buy".to_owned(),
            };
            look.button(d, &label, ButtonStyle::Primary, t.buyable(), Some(180.0))
                .insert(MenuAction::ShopBuy);
        }
    });
}

/// The turning companion, `size` units square (a framed space while its model loads).
pub(super) fn preview(p: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, size: f32) {
    let mut node = Node {
        width: look.px(size),
        height: look.px(size),
        flex_shrink: 0.0,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(look.px(super::look::BORDER)),
        border_radius: BorderRadius::all(look.px(4.0)),
        ..default()
    };
    match ctx.creature {
        Some(image) => {
            node.border = UiRect::ZERO;
            p.spawn((ImageNode::new(image.clone()), node, Pickable::IGNORE));
        }
        None => {
            p.spawn((node, BorderColor::all(palette::LOCKED), Pickable::IGNORE))
                .with_children(|b| {
                    b.spawn(look.hint("..."));
                });
        }
    }
}

/// The Collection's Companion box under the Equipment: the hero's companion (its icon and name,
/// or None); a click opens the Companions in the details.
pub(super) fn sheet_box(p: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, sheet: &Sheet) {
    look.heading(p, "Companion", 16.0);
    let current = match sheet.mode {
        SheetMode::Modify(g) => ctx
            .facts
            .hero(g)
            .map(|h| h.companion.clone())
            .unwrap_or_default(),
        SheetMode::Create => String::new(),
    };
    let companion = ctx.content.companion(&current);
    let style = if sheet.companions {
        ButtonStyle::RowPicked
    } else {
        ButtonStyle::Row
    };
    let mut row = look.frame_button(p, style, true, None);
    row.insert(MenuAction::OpenCompanions);
    row.with_children(|b| {
        b.spawn((
            Node {
                width: Val::Percent(100.0),
                column_gap: look.px(8.0),
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|line| {
            let icon = companion.map_or("INV_Misc_QuestionMark", |c| c.icon.as_str());
            let tint = if companion.is_some() {
                Color::WHITE
            } else {
                palette::LOCKED
            };
            line.spawn((
                ImageNode::new(look.icon_art(icon)).with_color(tint),
                Node {
                    width: look.px(26.0),
                    height: look.px(26.0),
                    border_radius: BorderRadius::all(look.px(2.0)),
                    ..default()
                },
                Pickable::IGNORE,
            ));
            line.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|t| {
                t.spawn(look.label("Fetches gold in runs"));
                match companion {
                    Some(c) => {
                        t.spawn(look.text(Face::Plain, 10.5, palette::VALUE, c.name.clone()))
                    }
                    None => t.spawn(look.text(Face::Plain, 10.5, palette::HINT, "None")),
                };
            });
        });
    });
}

/// The Companions in the details: the one picked turning, its name and text, Choose or None; then
/// every companion as an icon, the warband's bright, the others dark with where they come from.
pub(super) fn sheet_details(
    p: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
) {
    let empty = Achievements::default();
    let told = ctx.facts.achievements().unwrap_or(&empty);
    let SheetMode::Modify(guid) = sheet.mode else {
        p.spawn(look.hint("Make the hero first: its companion is picked on its hero sheet once it stands in the camp."));
        return;
    };
    let current = ctx
        .facts
        .hero(guid)
        .map(|h| h.companion.clone())
        .unwrap_or_default();
    let owned = shop::owned_companions(ctx.content, told);
    if let Some(c) = sheet
        .companion
        .as_deref()
        .and_then(|id| ctx.content.companion(id))
    {
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: look.px(4.0),
            ..default()
        })
        .with_children(|d| {
            preview(d, look, ctx, 170.0);
            d.spawn((
                look.text(Face::Plain, 13.0, palette::HEADING, c.name.clone()),
                Pickable::IGNORE,
            ));
            d.spawn((
                look.text(Face::Plain, 10.0, palette::VALUE, c.text.clone()),
                TextLayout::new_with_justify(Justify::Center),
                Pickable::IGNORE,
            ));
            let mine = owned.iter().any(|o| o.id == c.id);
            if current == c.id {
                d.spawn((
                    look.text(Face::Plain, 10.5, palette::GREEN, "With this hero"),
                    Pickable::IGNORE,
                ));
            } else if !mine {
                let tile = shop::tile(ctx.content, ctx.facts.zones_discovered(), told, &esurv_content::companion_item(&c.id));
                let words = tile.map(|t| match t.state {
                    TileState::FromAchievement(a) => format!("From the achievement {a}"),
                    _ => format!("In the Shop for {} Moments", c.price.unwrap_or(0)),
                });
                d.spawn((
                    look.text(Face::Plain, 10.0, palette::LABEL, words.unwrap_or_default()),
                    Pickable::IGNORE,
                ));
            }
            d.spawn(Node {
                column_gap: look.px(8.0),
                ..default()
            })
            .with_children(|row| {
                let can = mine && current != c.id;
                look.button(row, "Choose", ButtonStyle::Primary, can, Some(100.0))
                    .insert(MenuAction::ChooseCompanion(c.id.clone()));
                look.button(
                    row,
                    "None",
                    ButtonStyle::Text,
                    !current.is_empty(),
                    Some(80.0),
                )
                .insert(MenuAction::ChooseCompanion(String::new()));
            });
        });
    } else {
        p.spawn(look.hint("Pick a companion below to see it. It follows this hero in runs and brings back the gold out of reach."));
        if !current.is_empty() {
            look.button(p, "None", ButtonStyle::Text, true, Some(80.0))
                .insert(MenuAction::ChooseCompanion(String::new()));
        }
    }
    p.spawn(look.label(&format!(
        "{} of {} companions owned by the warband",
        owned.len(),
        ctx.content.companions.len()
    )));
    p.spawn(Node {
        flex_wrap: FlexWrap::Wrap,
        column_gap: look.px(3.0),
        row_gap: look.px(3.0),
        ..default()
    })
    .with_children(|grid| {
        // the warband's first, then the rest in the content's order
        let rest = ctx
            .content
            .companions
            .iter()
            .filter(|c| !owned.iter().any(|o| o.id == c.id));
        for c in owned.iter().copied().chain(rest) {
            let mine = owned.iter().any(|o| o.id == c.id);
            let picked = sheet.companion.as_deref() == Some(c.id.as_str());
            let tile = shop::tile(ctx.content, ctx.facts.zones_discovered(), told, &esurv_content::companion_item(&c.id));
            let mut tip = vec![TipLine::new(c.name.clone(), palette::VALUE)];
            if current == c.id {
                tip.push(TipLine::new("With this hero", palette::GREEN));
            } else if let Some(t) = tile.filter(|_| !mine) {
                tip.push(TipLine::new(shop::state_words(&t), palette::LABEL));
            }
            // one the warband does not have yet hides its icon: a grey tile with a "?"
            let image = if mine {
                look.icon_art(&c.icon)
            } else {
                Handle::default()
            };
            look.icon(grid, image, None, PICK_ICON, picked, !mine, false)
                .insert((
                    MenuAction::CompanionPick(c.id.clone()),
                    Tip(tip),
                    MenuLookTip,
                ));
        }
    });
}

/// The companion the menus show turning, if any: the Shop's companion picked, or the hero sheet's
/// on its Collection's Companions.
pub(super) fn shown_companion(menus: &Menus, content: &esurv_content::Content) -> Option<u32> {
    match &menus.screen {
        Screen::Warband if menus.achievements == Some(AchievementsPage::Shop) => {
            let item = menus.shop_pick.as_deref()?;
            Some(
                content
                    .companion(item.strip_prefix("companion:")?)?
                    .display(),
            )
        }
        Screen::Sheet(s) if s.tab == SheetTab::Collection && s.companions => {
            Some(content.companion(s.companion.as_deref()?)?.display())
        }
        _ => None,
    }
}

/// Drives benilla's creature pane: the companion shown, turning; none when the menus show none.
pub(super) fn drive_creature_model(
    menus: Res<Menus>,
    content: Option<Res<GameContent>>,
    time: Res<Time>,
    mut model: ResMut<CreatureModel>,
) {
    let want = content
        .and_then(|c| shown_companion(&menus, &c.0))
        .map(|display_id| CreatureModelView {
            display_id,
            yaw: (time.elapsed_secs() * TURN_SPEED) % std::f32::consts::TAU,
        });
    if model.0 != want {
        model.0 = want;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_decorations_tab_is_parked_while_camp_decorations_and_the_hub_are_off() {
        let shown: Vec<ShopTab> = ShopTab::shown().collect();
        assert_eq!(
            shown.contains(&ShopTab::Decorations),
            crate::achievements::cosmetics::CAMP_DECORATIONS || crate::camp::hub_on()
        );
        assert_eq!(shown[0], ShopTab::Upgrades);
    }
}
