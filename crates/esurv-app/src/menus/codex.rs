//! The codex on the warband screen (menus.md, "The flow", step 4): every
//! ring, trinket and amulet any hero of the save has taken in play, kept for good. A panel beside
//! the zone column says how much of each list is found; "Open the codex" shows the lists in a panel
//! over the screen: a tab per list with its count, the pieces a page at a time (the legendary
//! amulets a class a page, the picked hero's class first), a found piece with its icon, name and
//! what it does, one not found dark with a "?" and where it comes from. Built on
//! [`crate::hero_sheet::codex`]; what is found comes from the save through the app's way to the
//! server before a run (its `Codex` answer).

use std::collections::BTreeSet;

use bevy::prelude::*;

use super::look::{palette, solid, Arrow, ButtonStyle, Face, MenuLook};
use super::{Ctx, MenuAction};
use crate::hero_sheet::codex::{self, CodexList};
use crate::ui::rarity_colour;

/// Sizes, in units.
const VIEW_WIDTH: f32 = 780.0;
const ICON: f32 = 34.0;

/// What is found, or nothing when the save has not said.
fn found<'a>(ctx: &'a Ctx) -> Option<&'a BTreeSet<String>> {
    ctx.facts.codex()
}

/// Each list's count, a line each, and the total.
pub(super) fn counts(p: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx) {
    let Some(found) = found(ctx) else {
        p.spawn(look.hint("What the save has found is not known here."));
        return;
    };
    for list in CodexList::ALL {
        let (have, all) = codex::found_of(ctx.content, list, found);
        p.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|row| {
            row.spawn(look.label(list.label()));
            row.spawn(look.text(
                Face::Plain,
                11.0,
                if have > 0 {
                    palette::HEADING
                } else {
                    palette::LABEL
                },
                format!("{have} / {all}"),
            ));
        });
    }
    p.spawn(look.hint(codex::summary(ctx.content, found)));
}

/// The codex open: the lists, a page of one, the pieces.
pub(super) fn draw(
    commands: &mut Commands,
    root: Entity,
    look: &MenuLook,
    ctx: &Ctx,
    list: CodexList,
    page: usize,
) {
    let empty = BTreeSet::new();
    let found = found(ctx).unwrap_or(&empty);
    let pages = codex::pages(ctx.content, list);
    let page = page.min(pages - 1);
    let (entries, class) = codex::page_entries(ctx.content, list, page);
    commands.entity(root).with_children(|r| {
        r.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            solid(),
            GlobalZIndex(super::MENU_LAYER + 10),
        ))
        .with_children(|veil| {
            let mut node = look.panel_node();
            node.width = look.px(VIEW_WIDTH);
            node.row_gap = look.px(10.0);
            node.padding = UiRect::all(look.px(18.0));
            veil.spawn(look.panel(node)).with_children(|p| {
                look.heading(p, "Codex", 24.0);
                p.spawn((
                    look.hint(format!(
                        "Every ring, trinket and amulet any of your heroes has taken in a run, \
                         kept for good. {}",
                        codex::summary(ctx.content, found)
                    )),
                    TextLayout::new_with_justify(Justify::Center),
                ));
                // a tab per list, with its count
                p.spawn(Node {
                    justify_content: JustifyContent::Center,
                    column_gap: look.px(6.0),
                    ..default()
                })
                .with_children(|tabs| {
                    for l in CodexList::ALL {
                        let (have, all) = codex::found_of(ctx.content, l, found);
                        let style = if l == list {
                            ButtonStyle::Active
                        } else {
                            ButtonStyle::Text
                        };
                        look.button(
                            tabs,
                            &format!("{} {have}/{all}", l.label()),
                            style,
                            true,
                            Some(140.0),
                        )
                        .insert(MenuAction::CodexList(l));
                    }
                });
                // the page: its name, and the arrows through the pages
                let title = match class.as_deref().and_then(|c| ctx.content.class(c)) {
                    Some(c) => format!("{} Legendary Amulets", c.name),
                    None => list.label().to_owned(),
                };
                p.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|head| {
                    look.arrow(head, Arrow::Left, 24.0, page > 0)
                        .insert(MenuAction::CodexPage(false));
                    head.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|t| {
                        t.spawn(look.text(Face::Display, 18.0, palette::HEADING, title));
                        t.spawn(look.label(&format!("Page {} of {pages}", page + 1)));
                    });
                    look.arrow(head, Arrow::Right, 24.0, page + 1 < pages)
                        .insert(MenuAction::CodexPage(true));
                });
                // the pieces, in two columns
                p.spawn(Node {
                    display: Display::Grid,
                    grid_template_columns: vec![GridTrack::flex(1.0), GridTrack::flex(1.0)],
                    column_gap: look.px(14.0),
                    row_gap: look.px(8.0),
                    ..default()
                })
                .with_children(|grid| {
                    for e in &entries {
                        entry(grid, look, e, found.contains(&e.id), list);
                    }
                });
                p.spawn(Node {
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|row| {
                    look.button(row, "Close", ButtonStyle::Primary, true, Some(140.0))
                        .insert(MenuAction::CloseCodex);
                });
            });
        });
    });
}

/// One piece: found, its icon, name and text; not found, dark with "?", its name grey and where
/// it comes from.
fn entry(
    grid: &mut ChildSpawnerCommands,
    look: &MenuLook,
    e: &codex::CodexEntry,
    is_found: bool,
    list: CodexList,
) {
    grid.spawn(Node {
        column_gap: look.px(8.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|row| {
        look.icon(
            row,
            look.icon_art(&e.icon),
            None,
            ICON,
            false,
            !is_found,
            false,
        );
        row.spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_shrink: 1.0,
            ..default()
        })
        .with_children(|t| {
            let colour = if !is_found {
                palette::LABEL
            } else {
                e.rarity.map_or(palette::VALUE, rarity_colour)
            };
            t.spawn(look.text(Face::Plain, 11.0, colour, e.name.clone()));
            let line = if is_found {
                e.text.clone()
            } else {
                list.found_in().to_owned()
            };
            t.spawn(look.text(
                Face::Plain,
                9.0,
                if is_found {
                    palette::VALUE
                } else {
                    palette::HINT
                },
                line,
            ));
        });
    });
}
