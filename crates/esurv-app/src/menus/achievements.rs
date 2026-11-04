//! The Achievements window on the warband screen (menus.md, "The flow", step 4; screens.md,
//! "The Achievements window"): the warband's achievements as WoW lays them out, in the menus'
//! look. Its left column the pages: Summary, General, Classes (a page per class) and Feats; its
//! right the page: on Summary the score, the points to spend, the last five earned and a bar per
//! category; on the others a row per achievement (its icon, name and text, a shield with its
//! points, a progress bar or a checklist, the day it was earned, its rewards, a series' earlier
//! steps, and a box to track it in runs), earned rows bright and the others grey, scrolling with
//! the wheel. What it shows is built in [`crate::achievements`]; what is earned comes from the
//! save (`ToClient::Achievements`, with a class's facts before a run).

use bevy::prelude::*;

use esurv_protocol::{Achievements, MAX_TRACKED};

use super::look::{
    mark, palette, plain_icon, points_shield, progress_bar, solid, ButtonStyle, Face, MenuLook,
};
use super::scroll::{scrolling, ScrollingList};
use super::{Ctx, MenuAction};
use crate::achievements::{self as shown, AchievementRow, AchievementsPage, RowProgress};
use crate::ui::class_colour;

/// Sizes, in units.
const VIEW_WIDTH: f32 = 900.0;
const BODY_HEIGHT: f32 = 470.0;
const PAGES_WIDTH: f32 = 170.0;
const ICON: f32 = 38.0;

/// Which scrolling list a page is, so each keeps its own place.
pub(super) fn page_list(ctx: &Ctx, page: &AchievementsPage) -> ScrollingList {
    let at = match page {
        AchievementsPage::Summary => 0,
        AchievementsPage::General => 1,
        AchievementsPage::Feats => 2,
        AchievementsPage::Shop => 3 + ctx.content.classes.len(),
        AchievementsPage::Bounties => 4 + ctx.content.classes.len(),
        AchievementsPage::Class(c) => {
            3 + ctx
                .content
                .classes
                .iter()
                .position(|k| k.id == *c)
                .unwrap_or(0)
        }
    };
    ScrollingList::Achievements(at as u8)
}

/// The window open on `page`.
pub(super) fn draw(
    commands: &mut Commands,
    root: Entity,
    look: &MenuLook,
    ctx: &Ctx,
    page: &AchievementsPage,
) {
    let empty = Achievements::default();
    let told = ctx.facts.achievements().unwrap_or(&empty);
    commands.entity(root).with_children(|r| {
        r.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                // every window's title at one height near the top, growing down
                align_items: AlignItems::FlexStart,
                padding: UiRect::top(look.px(super::look::WINDOW_TOP)),
                ..default()
            },
            BackgroundColor(palette::DIM),
            solid(),
            GlobalZIndex(super::MENU_LAYER + 10),
        ))
        .with_children(|veil| {
            let mut node = look.panel_node();
            node.width = look.px(VIEW_WIDTH);
            node.row_gap = look.px(10.0);
            node.padding = UiRect::all(look.px(18.0));
            veil.spawn(look.panel(node)).with_children(|p| {
                // the Shop opens alone, from its own button on the warband screen
                let shop = *page == AchievementsPage::Shop;
                look.heading(p, if shop { "Shop" } else { "Achievements" }, 24.0);
                let line = if shop {
                    format!(
                        "{} Moments to spend, earned with achievements. Your achievement score ({}) never drops.",
                        shown::thousands(u64::from(told.balance)),
                        shown::thousands(u64::from(told.score)),
                    )
                } else {
                    format!(
                        "Earned by the whole warband. Achievement score {}, {} Moments to spend.",
                        shown::thousands(u64::from(told.score)),
                        shown::thousands(u64::from(told.balance)),
                    )
                };
                p.spawn((
                    look.hint(line),
                    TextLayout::new_with_justify(Justify::Center),
                ));
                p.spawn(Node {
                    column_gap: look.px(14.0),
                    height: look.px(BODY_HEIGHT),
                    ..default()
                })
                .with_children(|body| {
                    if shop {
                        super::companions::shop(body, look, ctx, told, page_list(ctx, page));
                        return;
                    }
                    pages(body, look, ctx, told, page);
                    body.spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            flex_basis: Val::ZERO,
                            row_gap: look.px(8.0),
                            min_height: Val::ZERO,
                            overflow: Overflow::scroll_y(),
                            padding: UiRect::right(look.px(8.0)),
                            ..default()
                        },
                        scrolling(page_list(ctx, page)),
                    ))
                    .with_children(|list| match page {
                        AchievementsPage::Summary => summary(list, look, ctx, told),
                        AchievementsPage::Bounties => bounty_rows(list, look, ctx, told),
                        _ => achievement_rows(list, look, ctx, told, page),
                    });
                });
                p.spawn(Node {
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|row| {
                    look.button(row, "Close", ButtonStyle::Primary, true, Some(140.0))
                        .insert(MenuAction::CloseAchievements);
                });
            });
        });
    });
}

/// The left column: a button per page, the classes indented under "Classes", each with its count.
fn pages(
    body: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    told: &Achievements,
    page: &AchievementsPage,
) {
    body.spawn(Node {
        flex_direction: FlexDirection::Column,
        width: look.px(PAGES_WIDTH),
        flex_shrink: 0.0,
        row_gap: look.px(4.0),
        ..default()
    })
    .with_children(|col| {
        let entry = |col: &mut ChildSpawnerCommands,
                     p: AchievementsPage,
                     label: &str,
                     indent: bool,
                     colour: Option<Color>| {
            let on = *page == p;
            let style = if on {
                ButtonStyle::RowPicked
            } else {
                ButtonStyle::Row
            };
            let count = match p {
                AchievementsPage::Summary => None,
                AchievementsPage::Shop => Some((told.balance as usize, usize::MAX)),
                AchievementsPage::Bounties => None,
                _ => Some(shown::page_count(ctx.content, told, &p)),
            };
            let mut b = look.frame_button(col, style, true, None);
            b.insert(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                margin: UiRect::left(look.px(if indent { 14.0 } else { 0.0 })),
                padding: UiRect::axes(look.px(8.0), look.px(if indent { 3.0 } else { 5.0 })),
                border: UiRect::all(look.px(super::look::BORDER)),
                border_radius: BorderRadius::all(look.px(4.0)),
                ..default()
            });
            b.insert(MenuAction::AchievementsPage(p));
            b.with_children(|b| {
                let ink = colour.unwrap_or(if on { palette::HEADING } else { palette::VALUE });
                let size = if indent { 10.0 } else { 11.5 };
                b.spawn((look.text(Face::Plain, size, ink, label), Pickable::IGNORE));
                if let Some((have, all)) = count {
                    // the Shop's count is the points to spend
                    let words = if all == usize::MAX { format!("{have} pts") } else { format!("{have}/{all}") };
                    b.spawn((look.text(Face::Narrow, 9.5, palette::LABEL, words), Pickable::IGNORE));
                }
            });
        };
        entry(col, AchievementsPage::Summary, "Summary", false, None);
        entry(col, AchievementsPage::General, "General", false, None);
        // the bounties, earned again and again, apart from the list one can finish
        if !ctx.content.bounties.is_empty() {
            entry(col, AchievementsPage::Bounties, "Bounties", false, None);
        }
        col.spawn((
            Node {
                padding: UiRect::new(look.px(8.0), Val::ZERO, look.px(4.0), Val::ZERO),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|l| {
            l.spawn(look.label("Classes"));
        });
        for c in &ctx.content.classes {
            let p = AchievementsPage::Class(c.id.clone());
            entry(col, p, &c.name, true, class_colour(&c.id));
        }
        entry(col, AchievementsPage::Feats, "Feats", false, None);
    });
}

/// The Summary: the two numbers, the last five earned, a bar per category.
fn summary(list: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, told: &Achievements) {
    let s = shown::summary(ctx.content, told);
    list.spawn(Node {
        column_gap: look.px(8.0),
        flex_shrink: 0.0,
        ..default()
    })
    .with_children(|row| {
        look.value_box(
            row,
            "Achievement score",
            &shown::thousands(u64::from(s.score)),
            palette::HEADING,
        );
        look.value_box(
            row,
            "Moments to spend",
            &shown::thousands(u64::from(s.balance)),
            palette::GREEN,
        );
    });
    look.heading(list, "Recently earned", 15.0);
    if s.recent.is_empty() {
        list.spawn((
            look.hint("Nothing earned yet: win a run, beat a boss, or try a feat."),
            Node {
                flex_shrink: 0.0,
                ..default()
            },
        ));
    }
    for r in &s.recent {
        list.spawn((
            Node {
                column_gap: look.px(8.0),
                align_items: AlignItems::Center,
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|line| {
            plain_icon(line, look, look.icon_art(&r.icon), 26.0);
            line.spawn((
                look.text(Face::Plain, 11.5, palette::HEADING, r.name.clone()),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                Pickable::IGNORE,
            ));
            if let Some(at) = r.earned_at {
                line.spawn((
                    look.text(Face::Narrow, 10.0, palette::LABEL, shown::date(at)),
                    Pickable::IGNORE,
                ));
            }
            line.spawn((
                look.text(Face::Display, 13.0, palette::HEADING, r.points.to_string()),
                Node {
                    width: look.px(30.0),
                    justify_content: JustifyContent::End,
                    ..default()
                },
                Pickable::IGNORE,
            ));
        });
    }
    look.heading(list, "Progress", 15.0);
    for c in &s.categories {
        list.spawn((
            Node {
                column_gap: look.px(10.0),
                align_items: AlignItems::Center,
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|line| {
            line.spawn((
                look.text(Face::Plain, 11.0, palette::VALUE, c.name),
                Node {
                    width: look.px(80.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                Pickable::IGNORE,
            ));
            line.spawn(Node {
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|bar| {
                let share = c.earned as f32 / c.all.max(1) as f32;
                progress_bar(bar, look, share, &format!("{} / {}", c.earned, c.all), 15.0);
            });
        });
    }
}

/// The Bounties page: each bounty, how often earned and how far to the next time; their points
/// go to spend, never the score.
fn bounty_rows(list: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, told: &Achievements) {
    look.heading(list, "Bounties", 17.0);
    list.spawn((look.hint("Earned again each time: their Moments go to spend, never the score."), Pickable::IGNORE));
    for b in &ctx.content.bounties {
        let r = told.bounties.iter().find(|x| x.id == b.id).cloned().unwrap_or_default();
        let earned = r.times > 0;
        let (edge, back) = if earned { (palette::GOLD_DIM, palette::INSET) } else { (palette::LOCKED, Color::srgba(0.05, 0.045, 0.04, 0.9)) };
        let node = Node {
            column_gap: look.px(10.0),
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            padding: UiRect::axes(look.px(8.0), look.px(7.0)),
            border: UiRect::all(look.px(super::look::BORDER)),
            border_radius: BorderRadius::all(look.px(4.0)),
            ..default()
        };
        list.spawn((node, BackgroundColor(back), BorderColor::all(edge))).with_children(|row| {
            plain_icon(row, look, look.icon_art(&b.icon), ICON);
            let words = Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: Val::ZERO, row_gap: look.px(3.0), ..default() };
            row.spawn((words, Pickable::IGNORE)).with_children(|t| {
                t.spawn((look.text(Face::Plain, 12.0, palette::HEADING, b.name.clone()), Pickable::IGNORE));
                t.spawn((look.text(Face::Plain, 9.5, palette::VALUE, b.text.clone()), Pickable::IGNORE));
                if r.goal > 0 {
                    t.spawn((Node { width: look.px(260.0), ..default() }, Pickable::IGNORE)).with_children(|bar| {
                        progress_bar(bar, look, r.current as f32 / r.goal as f32, &format!("{} / {}", r.current, r.goal), 12.0);
                    });
                }
            });
            row.spawn((look.text(Face::Display, 14.0, if earned { palette::HEADING } else { palette::LABEL }, format!("x{}", r.times)), Pickable::IGNORE));
            points_shield(row, look, b.points, earned);
        });
    }
}

/// A page's rows.
fn achievement_rows(
    list: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    told: &Achievements,
    page: &AchievementsPage,
) {
    let title = match page {
        AchievementsPage::Class(c) => ctx
            .content
            .class(c)
            .map_or_else(|| c.clone(), |k| k.name.clone()),
        AchievementsPage::General => "General".into(),
        AchievementsPage::Feats => "Feats".into(),
        AchievementsPage::Summary => "Summary".into(),
        AchievementsPage::Shop => "Shop".into(),
        AchievementsPage::Bounties => "Bounties".into(),
    };
    look.heading(list, &title, 17.0);
    let rows = shown::rows(ctx.content, told, page);
    let tracked = told.tracked.len();
    for r in &rows {
        achievement_row(list, look, r, tracked < MAX_TRACKED);
    }
}

/// One row: icon; name, text, progress, rewards, earlier steps; the shield, the day, the box.
fn achievement_row(
    list: &mut ChildSpawnerCommands,
    look: &MenuLook,
    r: &AchievementRow,
    room_to_track: bool,
) {
    let earned = r.earned_at.is_some();
    let (edge, back) = if earned {
        (palette::GOLD_DIM, palette::INSET)
    } else {
        (palette::LOCKED, Color::srgba(0.05, 0.045, 0.04, 0.9))
    };
    list.spawn((
        Node {
            column_gap: look.px(10.0),
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            padding: UiRect::axes(look.px(8.0), look.px(7.0)),
            border: UiRect::all(look.px(super::look::BORDER)),
            border_radius: BorderRadius::all(look.px(4.0)),
            ..default()
        },
        BackgroundColor(back),
        BorderColor::all(edge),
    ))
    .with_children(|row| {
        if r.secret {
            look.icon(row, look.icon_art(&r.icon), None, ICON, false, true, false);
        } else {
            let image = look.icon_art(&r.icon);
            if earned {
                plain_icon(row, look, image, ICON);
            } else {
                look.icon(row, image, None, ICON, false, false, true);
            }
        }
        row.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: Val::ZERO,
                row_gap: look.px(3.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|t| {
            let (name_ink, text_ink) = if earned {
                (palette::HEADING, palette::VALUE)
            } else {
                (palette::LABEL, palette::HINT)
            };
            t.spawn((
                look.text(Face::Plain, 12.0, name_ink, r.name.clone()),
                Pickable::IGNORE,
            ));
            t.spawn((
                look.text(Face::Plain, 9.5, text_ink, r.text.clone()),
                Pickable::IGNORE,
            ));
            match &r.progress {
                RowProgress::Bar { current, goal } if !earned => {
                    let words = shown::progress_words(&r.progress).unwrap_or_default();
                    t.spawn((
                        Node {
                            width: look.px(260.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|b| {
                        progress_bar(
                            b,
                            look,
                            *current as f32 / (*goal).max(1) as f32,
                            &words,
                            13.0,
                        )
                    });
                }
                RowProgress::Checklist(items) => checklist(t, look, items, earned),
                _ => {}
            }
            for reward in &r.rewards {
                let ink = if earned {
                    palette::HEADING
                } else {
                    palette::LABEL
                };
                t.spawn((
                    look.text(Face::Plain, 9.5, ink, reward.clone()),
                    Pickable::IGNORE,
                ));
            }
            if !r.earlier.is_empty() {
                let steps: Vec<String> = r
                    .earlier
                    .iter()
                    .map(|(name, at)| match at {
                        Some(at) => format!("{name} ({})", shown::date(*at)),
                        None => name.clone(),
                    })
                    .collect();
                t.spawn((
                    look.text(
                        Face::Plain,
                        9.0,
                        palette::LABEL,
                        format!("Earlier steps: {}", steps.join(", ")),
                    ),
                    Pickable::IGNORE,
                ));
            }
        });
        row.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: look.px(4.0),
                width: look.px(78.0),
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|side| {
            points_shield(side, look, r.points, earned);
            if let Some(at) = r.earned_at {
                side.spawn((
                    look.text(Face::Narrow, 9.5, palette::LABEL, shown::date(at)),
                    Pickable::IGNORE,
                ));
            } else if r.trackable() {
                track_box(side, look, r, room_to_track);
            }
        });
    });
}

/// A checklist: each item with its mark, in a wrapping row.
fn checklist(
    t: &mut ChildSpawnerCommands,
    look: &MenuLook,
    items: &[(String, bool)],
    earned: bool,
) {
    t.spawn((
        Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: look.px(12.0),
            row_gap: look.px(3.0),
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|w| {
        for (name, done) in items {
            w.spawn((
                Node {
                    column_gap: look.px(4.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|item| {
                mark(item, look, *done, 8.0);
                let ink = match (*done, earned) {
                    (true, true) => palette::VALUE,
                    (true, false) => palette::HEADING,
                    _ => palette::HINT,
                };
                item.spawn((
                    look.text(Face::Plain, 9.5, ink, name.clone()),
                    Pickable::IGNORE,
                ));
            });
        }
    });
}

/// The box that tracks a row in runs: ticked when it is; with three tracked, the others' boxes
/// are dimmed.
fn track_box(side: &mut ChildSpawnerCommands, look: &MenuLook, r: &AchievementRow, room: bool) {
    let enabled = r.tracked || room;
    let mut b = look.frame_button(side, ButtonStyle::Text, enabled, None);
    b.insert(Node {
        column_gap: look.px(4.0),
        align_items: AlignItems::Center,
        padding: UiRect::axes(look.px(5.0), look.px(2.0)),
        border: UiRect::all(look.px(super::look::BORDER)),
        border_radius: BorderRadius::all(look.px(3.0)),
        ..default()
    });
    if enabled {
        b.insert(MenuAction::TrackAchievement {
            id: r.id.clone(),
            on: !r.tracked,
        });
    }
    b.with_children(|b| {
        mark(b, look, r.tracked, 8.0);
        let ink = super::look::label_colour(ButtonStyle::Text, enabled);
        b.spawn((look.text(Face::Plain, 9.0, ink, "Track"), Pickable::IGNORE));
    });
}
