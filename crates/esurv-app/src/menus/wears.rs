//! The hero sheet's Title, Tabard and Weapon glow picks (docs/game/screens.md, "The Collection
//! tab"): a box for each under the Companion box on the left (what the hero wears, or None; a
//! click opens its picks in the details), and in the details every choice as an icon, the
//! warband's bright and the others dark, the one picked with its name and Choose or None. The
//! hero figure of the Character view wears its tabard ([`super::preview`]).

use bevy::prelude::*;
use esurv_protocol::{Achievements, WearKind};

use super::look::{palette, ButtonStyle, Face, MenuLook};
use super::{Ctx, MenuAction, Sheet, SheetMode};
use crate::achievements::cosmetics::{wear_choices, wear_label, WearChoice};
use crate::stock::{MenuLookTip, Tip, TipLine};

const PICK_ICON: f32 = 34.0;

/// What hero `guid` wears of `kind` (its id; empty for none).
pub(super) fn worn(ctx: &Ctx, sheet: &Sheet, kind: WearKind) -> String {
    match sheet.mode {
        SheetMode::Modify(g) => ctx
            .facts
            .hero(g)
            .map(|h| match kind {
                WearKind::Title => h.title.clone(),
                WearKind::Tabard => h.tabard.clone(),
                WearKind::Glow => h.glow.clone(),
            })
            .unwrap_or_default(),
        SheetMode::Create => String::new(),
    }
}

/// The hero's faction, by the sheet's race.
fn faction(ctx: &Ctx, sheet: &Sheet) -> Option<esurv_content::Faction> {
    ctx.content
        .race_by_wow_id(sheet.draft.race)
        .map(|r| r.faction)
}

fn choices(ctx: &Ctx, sheet: &Sheet, kind: WearKind) -> Vec<WearChoice> {
    let empty = Achievements::default();
    let told = ctx.facts.achievements().unwrap_or(&empty);
    let name = if sheet.draft.name.is_empty() {
        "Name"
    } else {
        sheet.draft.name.as_str()
    };
    wear_choices(ctx.content, told, kind, name, faction(ctx, sheet))
}

/// The three boxes under the Companion box, one row each: the pick's icon, what it does, what
/// the hero wears (or None).
pub(super) fn sheet_boxes(p: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, sheet: &Sheet) {
    look.heading(p, "Title, tabard and glow", 16.0);
    for kind in WearKind::ALL {
        let current = worn(ctx, sheet, kind);
        let list = choices(ctx, sheet, kind);
        let choice = list.iter().find(|c| c.id == current);
        let style = if sheet.wear == Some(kind) {
            ButtonStyle::RowPicked
        } else {
            ButtonStyle::Row
        };
        let (label, does) = wear_label(kind);
        let mut row = look.frame_button(p, style, true, None);
        row.insert(MenuAction::OpenWear(kind));
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
                let icon = choice.map_or("INV_Misc_QuestionMark", |c| c.icon.as_str());
                let tint = if choice.is_some() {
                    Color::WHITE
                } else {
                    palette::LOCKED
                };
                line.spawn((
                    ImageNode::new(look.icon_art(icon)).with_color(tint),
                    Node {
                        width: look.px(22.0),
                        height: look.px(22.0),
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
                    t.spawn(look.label(&format!("{label}: {does}")));
                    match choice {
                        Some(c) => {
                            t.spawn(look.text(Face::Plain, 10.0, palette::VALUE, c.name.clone()))
                        }
                        None => t.spawn(look.text(Face::Plain, 10.0, palette::HINT, "None")),
                    };
                });
            });
        });
    }
}

/// The picks of the box open, in the details.
pub(super) fn sheet_details(
    p: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    kind: WearKind,
) {
    if !matches!(sheet.mode, SheetMode::Modify(_)) {
        p.spawn(look.hint("Make the hero first: what it wears is picked on its hero sheet once it stands in the camp."));
        return;
    }
    let current = worn(ctx, sheet, kind);
    let list = choices(ctx, sheet, kind);
    let owned = list.iter().filter(|c| c.owned).count();
    if let Some(c) = sheet
        .wear_pick
        .as_deref()
        .and_then(|id| list.iter().find(|c| c.id == id))
    {
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: look.px(4.0),
            ..default()
        })
        .with_children(|d| {
            d.spawn((
                ImageNode::new(look.icon_art(&c.icon)),
                Node {
                    width: look.px(48.0),
                    height: look.px(48.0),
                    margin: UiRect::vertical(look.px(6.0)),
                    ..default()
                },
                Pickable::IGNORE,
            ));
            d.spawn((
                look.text(Face::Plain, 13.0, palette::HEADING, c.name.clone()),
                TextLayout::new_with_justify(Justify::Center),
                Pickable::IGNORE,
            ));
            if !c.text.is_empty() {
                d.spawn((
                    look.text(Face::Plain, 10.0, palette::VALUE, c.text.clone()),
                    TextLayout::new_with_justify(Justify::Center),
                    Pickable::IGNORE,
                ));
            }
            if current == c.id {
                d.spawn((
                    look.text(Face::Plain, 10.5, palette::GREEN, "Worn by this hero"),
                    Pickable::IGNORE,
                ));
            } else if !c.owned {
                d.spawn((
                    look.text(Face::Plain, 10.0, palette::LABEL, c.source.clone()),
                    Pickable::IGNORE,
                ));
            } else if !c.fits {
                d.spawn((
                    look.text(
                        Face::Plain,
                        10.0,
                        palette::RED,
                        "Not for this hero's faction",
                    ),
                    Pickable::IGNORE,
                ));
            }
            d.spawn(Node {
                column_gap: look.px(8.0),
                ..default()
            })
            .with_children(|row| {
                let can = c.owned && c.fits && current != c.id;
                look.button(row, "Choose", ButtonStyle::Primary, can, Some(100.0))
                    .insert(MenuAction::ChooseWear(kind, c.id.clone()));
                look.button(
                    row,
                    "None",
                    ButtonStyle::Text,
                    !current.is_empty(),
                    Some(80.0),
                )
                .insert(MenuAction::ChooseWear(kind, String::new()));
            });
        });
    } else {
        let hint = match kind {
            WearKind::Title => {
                "Pick a title below. It shows with the hero's name at the camp, and over its head in runs when names are shown."
            }
            WearKind::Tabard => {
                "Pick a tabard below. It shows over any armour, at the camp and in runs."
            }
            WearKind::Glow => {
                "Pick a glow below. It shows on all the hero's weapons, at the camp and in runs."
            }
        };
        p.spawn(look.hint(hint));
        if !current.is_empty() {
            look.button(p, "None", ButtonStyle::Text, true, Some(80.0))
                .insert(MenuAction::ChooseWear(kind, String::new()));
        }
    }
    let (label, _) = wear_label(kind);
    p.spawn(look.label(&format!(
        "{owned} of {} owned by the warband ({})",
        list.len(),
        label.to_lowercase()
    )));
    p.spawn(Node {
        flex_wrap: FlexWrap::Wrap,
        column_gap: look.px(3.0),
        row_gap: look.px(3.0),
        ..default()
    })
    .with_children(|grid| {
        let (mine, rest): (Vec<&WearChoice>, Vec<&WearChoice>) = list.iter().partition(|c| c.owned);
        for c in mine.into_iter().chain(rest) {
            let picked = sheet.wear_pick.as_deref() == Some(c.id.as_str());
            let mut tip = vec![TipLine::new(c.name.clone(), palette::VALUE)];
            if current == c.id {
                tip.push(TipLine::new("Worn by this hero", palette::GREEN));
            } else if !c.owned {
                tip.push(TipLine::new(c.source.clone(), palette::LABEL));
            } else if !c.fits {
                tip.push(TipLine::new("Not for this hero's faction", palette::RED));
            }
            // one the warband does not have yet hides its icon, as a companion's does
            let image = if c.owned {
                look.icon_art(&c.icon)
            } else {
                Handle::default()
            };
            look.icon(
                grid,
                image,
                None,
                PICK_ICON,
                picked,
                !c.owned,
                c.owned && !c.fits,
            )
            .insert((
                MenuAction::WearPick(kind, c.id.clone()),
                Tip(tip),
                MenuLookTip,
            ));
        }
    });
}
