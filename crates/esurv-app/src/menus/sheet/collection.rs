//! The hero sheet's Collection (menus.md, "The flow", step 3): the
//! Equipment column on the left; a double click on a piece wears it ([`double_click`]); in the middle the "Collection Log", the
//! class's gear in columns by slot and rows by zone (each zone drops its own pieces in its
//! own look, Blackrock in two rows of looks;
//! each row named in its quality colour, with its quality and how many of it are found, a piece
//! not found dark), under it what is still to find in
//! the zones open to the class (piece, boss, zone, chance), and a foot with how to read it and the
//! total found; on the right "Details": the piece picked (what it adds, which bosses drop it, Wear
//! or Take Off, Show This Look or Show Own Look), or the look choices of a slot picked on the left.
//! What changes here is the class's, for every hero of it, from the next run.

use bevy::prelude::*;
use esurv_protocol::Progress;
use super::super::look::{palette, ButtonStyle, Face, MenuLook};
use super::super::scroll::{scrolling, ScrollingList};
use super::super::{Ctx, MenuAction, Sheet, SheetMode};
use super::{MARGIN, SIDE, SLOT_ORDER, TOP};
use crate::hero_sheet::gear::{
    drop_chance, look_tip, minute_text, origin_line, piece_tip, stat_line, tier_quality,
    ClassGear,
};
use crate::stock::{MenuLookTip, Tip};
use esurv_sim::game::gear::ARMOR_SLOTS;
use crate::ui::rarity_colour;
use crate::zone_discovery::{discovered_in, shown_name, LOCK_ICON, UNDISCOVERED};

/// The icon in a slot's place of a zone not discovered.
const UNKNOWN_ICON: &str = "INV_Misc_QuestionMark";

/// Sizes, in units.
const CELL: f32 = 28.0;
const ROW_HEAD: f32 = 150.0;
/// How many pieces still to find the list shows.
const STILL_TO_FIND: usize = 5;

/// The line under a row's zone name: the first row's quality, a second row's own name where it
/// has one (Stratholme's "Scholomance sets") else "Second look", then
/// the pieces found of the row's slots.
pub(super) fn row_head_line(gear: &ClassGear, progress: &Progress, row_look: u32, found: usize, slots: usize) -> String {
    let head = if gear.look_row(row_look) > 0 {
        gear.look_row_name(Some(progress), row_look).unwrap_or("Second look")
    } else {
        quality_name(gear.tier(gear.look_zone(row_look).unwrap_or(0)))
    };
    format!("{head}  {found}/{slots}")
}

/// The qualities' names, a tier each.
pub(super) fn quality_name(tier: u32) -> &'static str {
    match tier {
        0 => "Common",
        1 => "Uncommon",
        2 => "Rare",
        3 => "Epic",
        _ => "Legendary",
    }
}

/// A chance as the sheet says it: `12%`, `2.5%`.
fn chance_text(c: f32) -> String {
    let pct = c * 100.0;
    if (pct - pct.round()).abs() < 0.05 {
        format!("{}%", pct.round() as i64)
    } else {
        format!("{pct:.1}%")
    }
}

/// The hint every tab whose changes are the class's carries while a hero is made.
pub(in crate::menus) fn class_wide_hint(
    look: &MenuLook,
    p: &mut ChildSpawnerCommands,
    sheet: &Sheet,
) {
    if sheet.mode == SheetMode::Create {
        p.spawn(look.hint(
            "Talents and gear are the class's: what changes here stays, even if this hero is not \
             made.",
        ));
    }
}

pub(in crate::menus) fn draw(
    r: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
) {
    let Some(gear) = ClassGear::of(ctx.content, &sheet.draft.class) else {
        return;
    };
    let progress = ctx.facts.progress(&sheet.draft.class);
    let mut node = look.panel_node();
    node.position_type = PositionType::Absolute;
    node.left = look.px(MARGIN * 2.0 + SIDE);
    node.right = look.px(MARGIN * 2.0 + SIDE);
    node.top = look.px(TOP);
    node.bottom = look.px(MARGIN);
    node.row_gap = look.px(8.0);
    r.spawn(look.panel(node)).with_children(|p| {
        look.heading(p, "Collection Log", 18.0);
        let Some(progress) = progress else {
            p.spawn(look.hint("What the class has won is not known here."));
            return;
        };
        log_grid(p, look, ctx, sheet, &gear, progress);
        p.spawn(look.divider());
        still_to_find(p, look, ctx, &gear, progress);
        // the foot: how to read it, and the total
        let (have, all) = gear.collected(progress);
        p.spawn(Node {
            flex_grow: 1.0,
            ..default()
        });
        p.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::FlexEnd,
            ..default()
        })
        .with_children(|foot| {
            foot.spawn(look.hint(
                "E marks the piece worn, L the look its slot shows; a dark piece is not found yet.",
            ));
            foot.spawn(look.text(
                Face::Plain,
                12.0,
                palette::HEADING,
                format!("{have} / {all} found"),
            ));
        });
        class_wide_hint(look, p, sheet);
    });
    details(r, look, ctx, sheet, &gear, progress);
}

/// The log: a header of slots, then a row per zone.
fn log_grid(
    p: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    gear: &ClassGear,
    progress: &Progress,
) {
    let slots: Vec<u32> = SLOT_ORDER.into_iter().chain(gear.weapon_slots()).collect();
    p.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: look.px(4.0),
        min_height: Val::ZERO,
        ..default()
    })
    .with_children(|grid| {
        // the slots' names over their columns
        grid.spawn(Node {
            column_gap: look.px(4.0),
            ..default()
        })
        .with_children(|head| {
            head.spawn(Node {
                width: look.px(ROW_HEAD),
                ..default()
            });
            for &slot in &slots {
                head.spawn(Node {
                    width: look.px(CELL),
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|c| {
                    let short: String = gear.slot_name(slot).chars().take(5).collect();
                    c.spawn(look.text(Face::Plain, 7.0, palette::LABEL, short.to_uppercase()));
                });
            }
        });
        // a row per zone, in difficulty order; more than the panel shows scroll
        grid.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: look.px(4.0),
                min_height: Val::ZERO,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            scrolling(ScrollingList::CollectionLog),
        ))
        .with_children(|rows| zone_rows(rows, look, ctx, sheet, gear, progress, &slots));
    });
}

/// The log's rows: a row per zone's look (every zone its own look;
/// Blackrock two rows, both unlocked by its pieces), its zone's name and quality, then its piece
/// of each slot in that look.
fn zone_rows(
    grid: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    gear: &ClassGear,
    progress: &Progress,
    slots: &[u32],
) {
    let worn = gear.worn(progress);
    for (zone, row_look) in gear.rows() {
        let second = gear.look_row(row_look) > 0;
        if !discovered_in(ctx.content, Some(progress), zone as usize) {
            // a zone not discovered shows one row of "???", whatever rows of looks it has
            if !second {
                undiscovered_row(grid, look, slots.len());
            }
            continue;
        }
        let tier = gear.tier(zone);
        let found = slots
            .iter()
            .filter(|&&s| gear.owned(progress, s, zone))
            .count();
        grid.spawn(Node {
            column_gap: look.px(4.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                width: look.px(ROW_HEAD),
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|h| {
                h.spawn(look.text(
                    Face::Plain,
                    11.0,
                    rarity_colour(tier_quality(tier)),
                    gear.zone_name(Some(progress), zone),
                ));
                let line = row_head_line(gear, progress, row_look, found, slots.len());
                h.spawn(look.text(Face::Plain, 8.0, palette::HINT, line));
            });
            for &slot in slots {
                let owned = gear.owned(progress, slot, zone);
                let picked = sheet
                    .piece
                    .is_some_and(|(s, z, l)| (s, z, l.unwrap_or(gear.own_look(z))) == (slot, zone, row_look));
                let path = ctx.icons.piece_icon(gear, slot, row_look);
                let mut cell =
                    look.icon(row, look.art(&path), None, CELL, picked, !owned, false);
                let mut tip = piece_tip(gear, progress, slot, zone);
                if second {
                    tip[0].text = gear.look_name(slot, row_look);
                }
                cell.insert((
                    MenuAction::Piece { slot, zone, look: second.then_some(row_look) },
                    Tip(tip),
                    MenuLookTip,
                ));
                let worn_here = worn.get(slot as usize).copied().flatten();
                // the worn piece: on its zone's first row
                let is_worn = worn_here == Some(zone) && !second;
                // the look shown: on its row's pieces found, unless the worn piece is its own
                let shows = owned
                    && gear.shown_look(progress, slot) == Some(row_look)
                    && worn_here.map(|z| gear.own_look(z)) != Some(row_look);
                if is_worn || shows {
                    cell.with_children(|c| {
                        c.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                right: look.px(1.0),
                                bottom: look.px(0.0),
                                ..default()
                            },
                            Pickable::IGNORE,
                        ))
                        .with_children(|m| {
                            m.spawn(look.text(
                                Face::Narrow,
                                10.0,
                                palette::HEADING,
                                if is_worn { "E" } else { "L" },
                            ));
                        });
                    });
                }
            }
        });
    }
}

// A zone the save has not discovered: "???" over a lock, and a dark
/// unknown icon in each slot's place, nothing to pick and no tooltip.
fn undiscovered_row(grid: &mut ChildSpawnerCommands, look: &MenuLook, slots: usize) {
    grid.spawn(Node {
        column_gap: look.px(4.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|row| {
        row.spawn(Node {
            width: look.px(ROW_HEAD),
            column_gap: look.px(6.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|h| {
            look.icon(h, look.art(LOCK_ICON), None, 14.0, false, true, false);
            h.spawn(look.text(Face::Plain, 11.0, palette::LABEL, UNDISCOVERED));
        });
        for _ in 0..slots {
            look.icon(row, look.icon_art(UNKNOWN_ICON), None, CELL, false, true, false);
        }
    });
}

/// The pieces the zones open to the class still hold: piece, boss, zone, chance.
fn still_to_find(
    p: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    gear: &ClassGear,
    progress: &Progress,
) {
    let bonus = ctx.facts.run_options().map_or(1.0, |(_, b)| b);
    let open = (progress.zones_open.max(1) as usize).min(ctx.content.zones.len());
    let mut rows: Vec<(u32, u32, String, usize, f32)> = Vec::new();
    for z in 0..open {
        for b in gear.drop_table(z) {
            for &(slot, zone) in &b.pieces {
                if gear.owned(progress, slot, zone)
                    || rows.iter().any(|r| (r.0, r.1) == (slot, zone))
                {
                    continue;
                }
                rows.push((
                    slot,
                    zone,
                    format!("{} ({})", b.boss, minute_text(b.minute)),
                    z,
                    drop_chance(&ctx.content.balance.zone_scaling, ctx.content.zones[z].difficulty, bonus),
                ));
            }
        }
    }
    p.spawn(look.label(&format!("Still to find in your zones: {}", rows.len())));
    if rows.is_empty() {
        p.spawn(look.hint("Your open zones hold nothing more: a win opens the next zone."));
        return;
    }
    for (slot, piece_zone, boss, zone, chance) in rows.into_iter().take(STILL_TO_FIND) {
        p.spawn(Node {
            column_gap: look.px(8.0),
            ..default()
        })
        .with_children(|row| {
            let mut cell = |w: f32, text: String, colour: Color| {
                row.spawn(Node {
                    width: look.px(w),
                    ..default()
                })
                .with_children(|c| {
                    c.spawn(look.text(Face::Plain, 10.0, colour, text));
                });
            };
            cell(
                200.0,
                gear.name(slot, piece_zone),
                rarity_colour(gear.quality(piece_zone)),
            );
            cell(190.0, boss, palette::VALUE);
            cell(150.0, shown_name(ctx.content, Some(progress), zone).to_owned(), palette::LABEL);
            cell(60.0, chance_text(chance), palette::HEADING);
        });
    }
}

//// The right column: the piece picked, or a slot's looks.
fn details(
    r: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    gear: &ClassGear,
    progress: Option<&Progress>,
) {
    let mut node = look.panel_node();
    node.position_type = PositionType::Absolute;
    node.right = look.px(MARGIN);
    node.top = look.px(TOP);
    node.bottom = look.px(MARGIN);
    node.width = look.px(SIDE);
    node.row_gap = look.px(8.0);
    r.spawn(look.panel(node)).with_children(|p| {
        look.heading(p, "Details", 18.0);
        let Some(progress) = progress else {
            return;
        };
        if let Some((slot, zone, row_look)) = sheet.piece {
            let row_look = row_look.unwrap_or(gear.own_look(zone));
            piece_details(p, look, ctx, gear, progress, slot, zone, row_look);
        } else if let Some(slot) = sheet.slot {
            slot_looks(p, look, ctx, gear, progress, slot);
        } else {
            p.spawn(look.hint(
                "Pick a piece of the log to see what it adds and who drops it, or a slot on the \
                 left for the looks it can show.",
            ));
        }
    });
}

/// A piece in the look of its row (`row_look`): its icon and name, the zone it comes from, what
/// it adds (a weapon also the loadouts' weapons in the look), how it stands, who drops it, and
/// what can be done.
#[allow(clippy::too_many_arguments)]
fn piece_details(
    p: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    gear: &ClassGear,
    progress: &Progress,
    slot: u32,
    zone: u32,
    row_look: u32,
) {
    let owned = gear.owned(progress, slot, zone);
    let worn_here = gear.worn(progress).get(slot as usize).copied().flatten();
    let worn = worn_here == Some(zone);
    let shown = gear.shown_look(progress, slot) == Some(row_look);
    p.spawn(Node {
        column_gap: look.px(10.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|row| {
        look.icon(
            row,
            look.art(&ctx.icons.piece_icon(gear, slot, row_look)),
            None,
            40.0,
            false,
            !owned,
            false,
        );
        row.spawn(Node {
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|t| {
            t.spawn(look.text(
                Face::Plain,
                12.0,
                rarity_colour(gear.quality(zone)),
                gear.look_name(slot, row_look),
            ));
            t.spawn(look.label(gear.slot_name(slot)));
            t.spawn(look.label(&origin_line(gear, Some(progress), zone)));
        });
    });
    p.spawn(look.label("It adds"));
    for m in gear.bonuses(slot, zone) {
        p.spawn(look.text(
            Face::Plain,
            10.5,
            palette::GREEN,
            stat_line(m.stat, m.value),
        ));
    }
    if slot >= ARMOR_SLOTS {
        // one weapon of the zone unlocks its armaments for all three
        // weapon loadouts
        p.spawn(look.label("Armaments for each weapon loadout"));
        for w in gear.armaments(row_look) {
            p.spawn(look.text(Face::Plain, 10.0, palette::VALUE, w));
        }
    }
    p.spawn(look.text(
        Face::Plain,
        10.5,
        palette::VALUE,
        if worn {
            "Worn"
        } else if owned {
            "In the collection"
        } else {
            "Not found yet"
        },
    ));
    p.spawn(look.label("Dropped by"));
    let sources = gear.sources(slot, zone);
    if sources.is_empty() {
        p.spawn(look.hint("No boss drops it."));
    }
    for (zone, boss) in sources.into_iter().take(6) {
        // a zone not discovered names neither itself nor its bosses
        let line = if discovered_in(ctx.content, Some(progress), zone) {
            format!("{boss}, {}", ctx.content.zones[zone].name)
        } else {
            UNDISCOVERED.to_owned()
        };
        p.spawn(look.text(Face::Plain, 10.0, palette::VALUE, line));
    }
    // the worn piece's own look is no choice of look
    let own_look = worn_here.map(|z| gear.own_look(z)) == Some(row_look);
    p.spawn(Node {
        flex_wrap: FlexWrap::Wrap,
        column_gap: look.px(6.0),
        row_gap: look.px(6.0),
        margin: UiRect::top(look.px(6.0)),
        ..default()
    })
    .with_children(|b| {
        if worn {
            look.button(b, "Take Off", ButtonStyle::Text, true, None)
                .insert(MenuAction::TakeOff(slot));
        } else {
            look.button(b, "Wear", ButtonStyle::Text, owned, None)
                .insert(MenuAction::Wear { slot, zone });
        }
        if shown && !own_look {
            look.button(b, "Show Own Look", ButtonStyle::Text, true, None)
                .insert(MenuAction::ShowLook { slot, look: None });
        } else {
            look.button(
                b,
                "Show This Look",
                ButtonStyle::Text,
                owned && !shown,
                None,
            )
            .insert(MenuAction::ShowLook {
                slot,
                look: Some(row_look),
            });
        }
    });
}

/// What a double click on a piece of the log asks: the same as picking
/// it and pressing its Wear (a zone's own row: the piece worn) or its Show This Look (another row
/// of looks: the look shown, the piece worn first when the slot wears nothing); nothing for a
/// piece not found, or one already worn and shown.
pub(in crate::menus) fn double_click(
    gear: &ClassGear,
    progress: &Progress,
    slot: u32,
    zone: u32,
    look: Option<u32>,
) -> Vec<MenuAction> {
    if !gear.owned(progress, slot, zone) {
        return Vec::new();
    }
    let worn_here = gear.worn(progress).get(slot as usize).copied().flatten();
    let wear = MenuAction::Wear { slot, zone };
    match look {
        None if worn_here != Some(zone) => vec![wear],
        None => Vec::new(),
        Some(l) if gear.shown_look(progress, slot) == Some(l) => Vec::new(),
        Some(l) => {
            let show = MenuAction::ShowLook { slot, look: Some(l) };
            if worn_here.is_none() {
                vec![wear, show]
            } else {
                vec![show]
            }
        }
    }
}

/// A slot's look choices: every zone's look (and row) of it, a found one lit, the one shown
/// glowing, and its own look.
fn slot_looks(
    p: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    gear: &ClassGear,
    progress: &Progress,
    slot: u32,
) {
    p.spawn(look.text(Face::Display, 18.0, palette::HEADING, gear.slot_name(slot)));
    let worn = gear.worn(progress).get(slot as usize).copied().flatten();
    let shown = gear.shown_look(progress, slot);
    p.spawn(look.label("Looks it can show"));
    p.spawn(Node {
        flex_wrap: FlexWrap::Wrap,
        column_gap: look.px(6.0),
        row_gap: look.px(6.0),
        ..default()
    })
    .with_children(|row| {
        for (_, l) in gear.rows() {
            let owned = gear.owns_look(progress, slot, l);
            let mut cell = look.icon(
                row,
                look.art(&ctx.icons.piece_icon(gear, slot, l)),
                None,
                38.0,
                shown == Some(l),
                !owned,
                false,
            );
            cell.insert((Tip(look_tip(gear, progress, slot, l)), MenuLookTip));
            if owned && worn.is_some() {
                cell.insert(MenuAction::ShowLook { slot, look: Some(l) });
            }
        }
    });
    if worn.is_none() {
        p.spawn(look.hint("Nothing is worn in this slot: a look shows over a piece worn."));
    } else {
        look.button(
            p,
            "Show Own Look",
            ButtonStyle::Text,
            shown != worn.map(|z| gear.own_look(z)),
            None,
        )
        .insert(MenuAction::ShowLook { slot, look: None });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chance_reads_as_a_percentage() {
        assert_eq!(chance_text(0.12), "12%");
        assert_eq!(chance_text(0.025), "2.5%");
        assert_eq!(quality_name(4), "Legendary");
    }

    /// A row's head line: the first row's quality, Blackrock's second "Second look", Stratholme's
    /// second its name, "Scholomance sets".
    #[test]
    fn a_row_head_names_its_quality_or_its_second_row() {
        let content = crate::hero_sheet::gear::tests::content();
        let gear = ClassGear::of(content, "warrior").unwrap();
        let progress = Progress {
            class: "warrior".into(),
            talent_experience: 0,
            points: 0,
            spent: 0,
            into_next: 0,
            next_cost: 80,
            zones_open: 1,
            zones_discovered: content.zones.iter().map(|z| z.id.clone()).collect(),
            talents: vec![],
            worn: vec![],
            looks: vec![],
            collection: vec![],
            prestige: Default::default(),
            loadout: String::new(),
            new_gear: false,
            new_points: false,
        };
        let look = |zone, row| gear.set.look_of(zone, row).unwrap() as u32;
        assert_eq!(row_head_line(&gear, &progress, look("elwynn", 0), 2, 8), "Common  2/8");
        assert_eq!(row_head_line(&gear, &progress, look("brm", 0), 0, 10), "Legendary  0/10");
        assert_eq!(row_head_line(&gear, &progress, look("brm", 1), 0, 10), "Second look  0/10");
        assert_eq!(row_head_line(&gear, &progress, look("strat", 1), 3, 10), "Scholomance sets  3/10");
    }

    /// A double click on a piece found wears it (its Wear), on another
    /// row of looks shows that look (its Show This Look, the piece worn first on a bare slot), and
    /// does nothing on a piece not found or already worn.
    #[test]
    fn a_double_click_wears_the_piece_or_shows_its_look() {
        let content = crate::hero_sheet::gear::tests::content();
        let gear = ClassGear::of(content, "warrior").unwrap();
        let (zone, second) = gear
            .rows()
            .into_iter()
            .find(|&(_, l)| gear.look_row(l) > 0)
            .expect("a zone with a second row of looks");
        let progress = |worn: Vec<u32>, collection: Vec<u32>| Progress {
            class: "warrior".into(),
            talent_experience: 0,
            points: 0,
            spent: 0,
            into_next: 0,
            next_cost: 80,
            zones_open: 1,
            zones_discovered: Vec::new(),
            talents: vec![],
            worn,
            looks: vec![],
            collection,
            prestige: Default::default(),
            loadout: String::new(),
            new_gear: false,
            new_points: false,
        };
        let head = gear.entry(0, zone);
        let found = progress(vec![], vec![head]);
        assert_eq!(double_click(&gear, &found, 0, zone, None), [MenuAction::Wear { slot: 0, zone }]);
        assert_eq!(
            double_click(&gear, &found, 0, zone, Some(second)),
            [MenuAction::Wear { slot: 0, zone }, MenuAction::ShowLook { slot: 0, look: Some(second) }],
            "a bare slot wears the piece, then shows the look"
        );
        let worn = progress(vec![head], vec![head]);
        assert!(double_click(&gear, &worn, 0, zone, None).is_empty(), "already worn");
        assert_eq!(
            double_click(&gear, &worn, 0, zone, Some(second)),
            [MenuAction::ShowLook { slot: 0, look: Some(second) }]
        );
        assert!(double_click(&gear, &progress(vec![], vec![]), 0, zone, None).is_empty(), "not found");
    }
}
