//! The hero sheet's Talents window (menus.md, "The flow", step 3). On top the build bar: the
//! hero's Starting spell and the class's Weapon loadout, each opening a drop-down of the choices.
//! Under it the pages Talents and Prestige (open once every talent point is earned), and on their
//! right the five saved builds of the class with Save: a click on one puts it back, a right click
//! empties it, Save keeps the talents, prestige, loadout and starting spell in the one lit, Name
//! types a name for the one lit (Enter keeps it, Escape drops it). The
//! Talents page: the class's three trees side by side, each with its name, icon and points spent, on its stock
//! background art, its talents as icons with `rank / max` and lines between a talent and the one it
//! needs, and under it its milestone talents; on the right "Talent Points": the points to spend in
//! big green, earned and spent, the talent experience bar to the next point with how runs earn it,
//! Reset (talents and prestige), and the talent or prestige stat picked (its icon, name, rank and
//! what it does) with Learn and Unlearn. The Prestige page: its points, the bar to the next and
//! every stat. A click picks a talent or prestige stat, a
//! click on the one picked learns a rank, a right click takes one back.
//! The rules are the server's own (`hero_sheet::talents`); changes are the class's.

use benilla_app::OverlayKeys;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use esurv_content::{Talent, TalentTree};
use esurv_protocol::Progress;
use esurv_sim::game::rewards::MAX_TALENT_POINTS;
use esurv_sim::game::talents::{Ranks, POINTS_PER_ROW};

use super::super::look::{palette, ButtonStyle, Face, MenuLook};
use super::super::{warband, BuildDrop, Ctx, MenuAction, MenuPress, Menus, Screen, Sheet, SheetMode, TalentsPage};
use super::collection::class_wide_hint;
use super::{MARGIN, SIDE, TOP};
use super::super::scroll::{scrolling, ScrollingList};
use crate::hero_sheet::prestige;
use crate::hero_sheet::talents::{self, TalentLook};
use crate::stock::{MenuLookTip, Tip, TipLine};

/// The stock talent frame's shape (320 by 384, its top 332 rows shown) and its grid, in its own
/// units at a 250-wide tree: talents 64 apart across, 36 down, 8 in from the corner, 30 wide.
const ART_WIDTH: f32 = 320.0;
const ART_HEIGHT: f32 = 384.0;
const ART_SHOWN: f32 = 332.0;
const GRID_WIDTH: f32 = 250.0;
const GRID_INSET: f32 = 8.0;
const GRID_COLUMN: f32 = 64.0;
const GRID_ROW: f32 = 36.0;
const GRID_ICON: f32 = 30.0;
/// The experience bar's purple.
const EXPERIENCE: Color = Color::srgb(0.58, 0.0, 0.55);

/// A talent's tooltip with our click lines in place of the stock frame's:
/// a click picks it, a click on the one picked learns a rank, a right click takes one
/// back. `picked`: it is the talent picked.
fn tip(
    ctx: &Ctx,
    class: &str,
    ranks: &Ranks,
    points: u32,
    tree: usize,
    t: &Talent,
    picked: bool,
) -> Vec<TipLine> {
    let v = talents::view(ctx.content, class, ranks, points, tree, t);
    let mut lines: Vec<TipLine> = talents::tip(ctx.content, class, ranks, points, tree, t)
        .into_iter()
        .filter(|l| !l.text.starts_with("Click") && !l.text.starts_with("Right-click"))
        .collect();
    lines.extend(prestige::click_lines(picked, v.can_learn, v.can_unlearn));
    lines
}

pub(in crate::menus) fn draw(
    r: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
) {
    let class = sheet.draft.class.as_str();
    let Some(progress) = ctx.facts.progress(class) else {
        return;
    };
    let ranks = talents::ranks(progress);
    let trees = talents::trees(ctx.content, class);
    // the prestige page opens once every talent point is earned
    let page = match sheet.talents_page {
        TalentsPage::Prestige if prestige::shown(progress) => TalentsPage::Prestige,
        _ => TalentsPage::Talents,
    };
    // from the left margin to the points column: the build bar, the page tabs with the saved
    // builds, then the page (the trees side by side, or the prestige stats); more than shows
    // scrolls
    r.spawn(Node {
        position_type: PositionType::Absolute,
        left: look.px(MARGIN),
        right: look.px(MARGIN * 2.0 + SIDE),
        top: look.px(TOP),
        bottom: look.px(MARGIN),
        flex_direction: FlexDirection::Column,
        row_gap: look.px(8.0),
        min_height: Val::ZERO,
        ..default()
    })
    .with_children(|left| {
        build_bar(left, look, ctx, sheet, progress);
        page_tabs(left, look, progress, page, sheet.build_name.as_deref());
        left.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_height: Val::ZERO,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            scrolling(ScrollingList::Talents),
        ))
        .with_children(|scroll| match page {
            TalentsPage::Talents => {
                scroll
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::SpaceEvenly,
                        align_items: AlignItems::FlexStart,
                        column_gap: look.px(10.0),
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|row| {
                        for (i, tree) in trees.iter().enumerate() {
                            tree_column(row, look, ctx, sheet, progress, &ranks, i, tree);
                        }
                    });
            }
            TalentsPage::Prestige => prestige_section(scroll, look, ctx, sheet, progress),
        });
        // the drop-down open over the page, under its spot on the bar
        if let Some(drop) = sheet.build_drop {
            build_drop(left, look, ctx, sheet, progress, drop);
        }
    });
    points_column(r, look, ctx, sheet, progress, &ranks);
}

/// The build bar's height, its spots' icons and a drop-down's rows, in units.
const BAR_HEIGHT: f32 = 48.0;
const BAR_ICON: f32 = 36.0;
const DROP_ICON: f32 = 30.0;
/// A drop-down row's icon and name, as wide in every row, in units.
const DROP_ROW_WIDTH: f32 = 160.0;
/// A build bar spot's width, in units.
const SPOT_WIDTH: f32 = 230.0;
/// The arrow on a build bar spot, in units.
const ARROW_WIDTH: f32 = 11.0;
const ARROW_HEIGHT: f32 = 7.0;

/// The arrow's picture: a white triangle pointing down, tinted gold where it shows.
const ARROW: Handle<Image> = bevy::asset::uuid_handle!("6f1d2b8e-4c3a-4e57-9a1f-2d8c7b5e3a90");
/// The arrow's picture size, in pixels.
const ARROW_PIXELS: (u32, u32) = (22, 14);

/// Draws the arrow's picture once ([`ARROW`]): a triangle with its edges smoothed, four by four
/// samples a pixel.
pub(in crate::menus) fn draw_the_arrow(mut images: ResMut<Assets<Image>>) {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let (w, h) = ARROW_PIXELS;
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let mut inside = 0u32;
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = x as f32 + (sx as f32 + 0.5) / 4.0;
                    let py = y as f32 + (sy as f32 + 0.5) / 4.0;
                    // the top edge full width, narrowing to the point at the bottom
                    let half = w as f32 / 2.0 * (1.0 - py / h as f32);
                    if (px - w as f32 / 2.0).abs() <= half {
                        inside += 1;
                    }
                }
            }
            rgba.extend_from_slice(&[255, 255, 255, (inside * 255 / 16) as u8]);
        }
    }
    let image = Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let _ = images.insert(&ARROW, image);
}

/// The starting option the sheet's hero starts with (the class's first for a hero not yet made).
fn hero_start<'c>(ctx: &Ctx<'c>, sheet: &Sheet) -> Option<&'c esurv_content::StartingOption> {
    let class = ctx.content.class(&sheet.draft.class)?;
    let told = match sheet.mode {
        SheetMode::Modify(g) => ctx.facts.hero(g).map(|f| f.start.as_str()).filter(|s| !s.is_empty()),
        SheetMode::Create => None,
    };
    class.starting_option(told)
}

/// A starting option's icon: its own, else its main spell's.
fn option_icon<'c>(ctx: &Ctx<'c>, o: &'c esurv_content::StartingOption) -> &'c str {
    match o.icon.as_str() {
        "" => ctx.content.ability(o.main_spell()).map_or("", |a| a.icon.as_str()),
        own => own,
    }
}

/// The build bar over the page: the Starting spell (of a hero already made) and the Weapon
/// loadout, each its icon and name with a gold arrow; a click opens its drop-down of the class's choices.
fn build_bar(left: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, sheet: &Sheet, progress: &Progress) {
    let class = sheet.draft.class.as_str();
    let mut node = look.panel_node();
    node.flex_direction = FlexDirection::Row;
    node.align_items = AlignItems::Center;
    node.column_gap = look.px(14.0);
    node.padding = UiRect::axes(look.px(10.0), look.px(6.0));
    node.height = look.px(BAR_HEIGHT);
    node.flex_shrink = 0.0;
    left.spawn(look.panel(node)).with_children(|bar| {
        if matches!(sheet.mode, SheetMode::Modify(_)) {
            if let (Some(c), Some(o)) = (ctx.content.class(class), hero_start(ctx, sheet)) {
                let tip = warband::option_tip(ctx.content, c, o);
                bar_spot(bar, look, "Starting spell", option_icon(ctx, o), &o.name, tip, BuildDrop::Start, sheet);
            }
        }
        let icons = warband::loadout_icons(ctx.content, class, Some(progress.loadout.as_str()));
        if let Some(i) = icons.iter().find(|i| i.chosen) {
            let l = i.loadout;
            bar_spot(bar, look, "Weapon loadout", &l.icon, &l.name, warband::loadout_tip(l), BuildDrop::Loadout, sheet);
        }
    });
}

/// One spot of the build bar: its label over its icon and name, and a gold arrow pointing down
/// (up, and the spot lit, while its drop-down is open); the choice's tooltip on it.
#[allow(clippy::too_many_arguments)]
fn bar_spot(
    bar: &mut ChildSpawnerCommands,
    look: &MenuLook,
    label: &str,
    icon: &str,
    name: &str,
    tip: Vec<TipLine>,
    drop: BuildDrop,
    sheet: &Sheet,
) {
    let open = sheet.build_drop == Some(drop);
    let style = if open { ButtonStyle::RowPicked } else { ButtonStyle::Row };
    look.frame_button(bar, style, true, Some(SPOT_WIDTH))
        .insert((MenuAction::BuildDrop(Some(drop)), Tip(tip), MenuLookTip))
        .with_children(|b| {
            b.spawn((
                Node {
                    width: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    column_gap: look.px(8.0),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|row| {
                row.spawn((
                    ImageNode::new(look.icon_art(icon)),
                    Node {
                        width: look.px(BAR_ICON - 8.0),
                        height: look.px(BAR_ICON - 8.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
                row.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        flex_grow: 1.0,
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|words| {
                    words.spawn((look.label(label), Pickable::IGNORE));
                    words.spawn((look.value(name.to_string()), Pickable::IGNORE));
                });
                // down while shut, up while its drop-down is open
                row.spawn((
                    ImageNode {
                        image: ARROW,
                        color: palette::GOLD,
                        flip_y: open,
                        ..default()
                    },
                    Node {
                        width: look.px(ARROW_WIDTH),
                        height: look.px(ARROW_HEIGHT),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
            });
        });
}

/// The drop-down open under its spot: every choice of the class as a row (its icon and name, the
/// one taken lit, its tooltip on it); a click takes it and closes the drop-down.
fn build_drop(
    left: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    progress: &Progress,
    drop: BuildDrop,
) {
    let class = sheet.draft.class.as_str();
    // (icon, name, taken, tooltip, action) of each choice
    let rows: Vec<(&str, String, bool, Vec<TipLine>, MenuAction)> = match drop {
        BuildDrop::Start => {
            let (Some(c), Some(taken)) = (ctx.content.class(class), hero_start(ctx, sheet)) else {
                return;
            };
            c.starting_options
                .iter()
                .map(|o| {
                    let tip = warband::option_tip(ctx.content, c, o);
                    (option_icon(ctx, o), o.name.clone(), o.id == taken.id, tip, MenuAction::PickStart(o.id.clone()))
                })
                .collect()
        }
        BuildDrop::Loadout => warband::loadout_icons(ctx.content, class, Some(progress.loadout.as_str()))
            .into_iter()
            .map(|i| {
                let l = i.loadout;
                (l.icon.as_str(), l.name.clone(), i.chosen, warband::loadout_tip(l), MenuAction::PickLoadout(l.id.clone()))
            })
            .collect(),
    };
    // under its spot: the loadout's is the second when the starting spell shows
    let second = drop == BuildDrop::Loadout && matches!(sheet.mode, SheetMode::Modify(_));
    let mut node = look.panel_node();
    node.position_type = PositionType::Absolute;
    node.top = look.px(BAR_HEIGHT + 2.0);
    node.left = look.px(10.0 + if second { SPOT_WIDTH + 14.0 } else { 0.0 });
    node.width = look.px(SPOT_WIDTH);
    node.row_gap = look.px(4.0);
    node.padding = UiRect::all(look.px(6.0));
    // over the menus' own layer (the window's panels would take its clicks), under their
    // windows and the tooltips
    left.spawn((look.panel(node), GlobalZIndex(super::super::MENU_LAYER + 5))).with_children(|list| {
        for (icon, name, taken, tip, action) in rows {
            let style = if taken { ButtonStyle::RowPicked } else { ButtonStyle::Row };
            look.frame_button(list, style, true, Some(SPOT_WIDTH - 12.0))
                .insert((action, Tip(tip), MenuLookTip))
                .with_children(|b| {
                    // one width for every row, centred in it: the icons and names line up on
                    // the left of a column in the middle
                    b.spawn((
                        Node {
                            width: look.px(DROP_ROW_WIDTH),
                            align_items: AlignItems::Center,
                            column_gap: look.px(8.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|row| {
                        row.spawn((
                            ImageNode::new(look.icon_art(icon)),
                            Node {
                                width: look.px(DROP_ICON - 6.0),
                                height: look.px(DROP_ICON - 6.0),
                                flex_shrink: 0.0,
                                ..default()
                            },
                            Pickable::IGNORE,
                        ));
                        let colour = if taken { palette::GOLD } else { palette::VALUE };
                        row.spawn((look.text(Face::Plain, 11.0, colour, name), Pickable::IGNORE));
                    });
                });
        }
    });
}

/// The page tabs, Talents and Prestige (shut until every talent point is earned), and on the
/// right the saved builds: Builds 1 to 5 (the one lit filled; its tooltip what it holds) and
/// Save, which keeps what the class holds now in the build lit.
fn page_tabs(
    left: &mut ChildSpawnerCommands,
    look: &MenuLook,
    progress: &Progress,
    page: TalentsPage,
    naming: Option<&str>,
) {
    left.spawn(Node {
        width: Val::Percent(100.0),
        align_items: AlignItems::Center,
        column_gap: look.px(6.0),
        flex_shrink: 0.0,
        ..default()
    })
    .with_children(|row| {
        let open = prestige::shown(progress);
        for (label, p, enabled) in [("Talents", TalentsPage::Talents, true), ("Prestige", TalentsPage::Prestige, open)] {
            let style = if p == page { ButtonStyle::Active } else { ButtonStyle::Text };
            let mut tab = look.button(row, label, style, enabled, Some(96.0));
            tab.insert(MenuAction::TalentsPage(p));
            if !enabled {
                tab.insert((
                    Tip(vec![
                        TipLine::new("Prestige", palette::VALUE),
                        TipLine::new(format!("Opens once all {MAX_TALENT_POINTS} talent points are earned."), palette::LABEL),
                    ]),
                    MenuLookTip,
                ));
            }
        }
        row.spawn(Node {
            flex_grow: 1.0,
            ..default()
        });
        row.spawn(look.label("Builds"));
        for slot in 0..esurv_protocol::BUILD_SLOTS {
            let build = progress.builds.iter().find(|b| b.slot == slot);
            let lit = progress.active_build == slot;
            let style = if lit { ButtonStyle::Active } else { ButtonStyle::Text };
            look.button(row, &(slot + 1).to_string(), style, true, Some(30.0))
                .insert((MenuAction::UseBuild(slot), Tip(build_tip(slot, build, lit)), MenuLookTip));
        }
        look.button(row, "Save", ButtonStyle::Text, true, Some(56.0)).insert((
            MenuAction::SaveBuild,
            Tip(vec![
                TipLine::new("Save build", palette::VALUE),
                TipLine::new(
                    format!(
                        "Keeps the talents, prestige, weapon loadout and starting spell in build {}.",
                        progress.active_build + 1
                    ),
                    palette::LABEL,
                ),
            ]),
            MenuLookTip,
        ));
        name_button(row, look, progress, naming);
    });
}

/// Name, after Save: a click starts typing the lit build's name (shut while it is empty); while
/// typing it shows the name typed, and a click or Enter keeps it.
fn name_button(row: &mut ChildSpawnerCommands, look: &MenuLook, progress: &Progress, naming: Option<&str>) {
    let lit = progress.active_build + 1;
    let (label, style, enabled, width, how) = match naming {
        Some(typed) => (
            format!("{typed}_"),
            ButtonStyle::Active,
            true,
            150.0,
            "Type the name: Enter or a click keeps it, Escape drops it. Empty gives back the number.".to_owned(),
        ),
        None => {
            let held = progress.builds.iter().any(|b| b.slot == progress.active_build);
            let how = if held {
                format!("Click to type a name for build {lit}.")
            } else {
                format!("Save build {lit} first to name it.")
            };
            ("Name".to_owned(), ButtonStyle::Text, held, 56.0, how)
        }
    };
    look.button(row, &label, style, enabled, Some(width)).insert((
        MenuAction::NameBuild,
        Tip(vec![TipLine::new("Name build", palette::VALUE), TipLine::new(how, palette::LABEL)]),
        MenuLookTip,
    ));
}

/// The holder of the typing keys while a build's name is typed ([`OverlayKeys`]): in the camp a
/// letter typed runs no key binding (W does not walk the hero).
const NAME_KEYS_OWNER: &str = "build name";

/// The keys a build name being typed takes from the key bindings.
const NAME_KEYS: &[KeyCode] = &[
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Space,
    KeyCode::Minus,
    KeyCode::Quote,
    KeyCode::Backspace,
    KeyCode::Enter,
    KeyCode::NumpadEnter,
    KeyCode::Escape,
];

/// `name` with `typed` added: letters, digits, spaces, dashes and apostrophes, up to
/// [`esurv_protocol::BUILD_NAME_LIMIT`] characters.
pub(in crate::menus) fn build_name_typed(name: &str, typed: &str) -> String {
    let mut name = name.to_owned();
    for c in typed.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '\'')) {
        if name.chars().count() < esurv_protocol::BUILD_NAME_LIMIT {
            name.push(c);
        }
    }
    name
}

/// The lit build's name typed while Name is on: Backspace takes a character back, Enter keeps
/// the name (Escape, which drops it, is `escape_goes_back`'s).
pub(in crate::menus) fn type_the_build_name(
    mut keys: MessageReader<KeyboardInput>,
    mut menus: ResMut<Menus>,
    mut presses: MessageWriter<MenuPress>,
    mut taken: ResMut<OverlayKeys>,
) {
    let typed = match &menus.screen {
        Screen::Sheet(s) => s.build_name.clone(),
        _ => None,
    };
    let wanted: &[KeyCode] = if typed.is_some() { NAME_KEYS } else { &[] };
    if taken.of(NAME_KEYS_OWNER) != wanted {
        taken.set(NAME_KEYS_OWNER, wanted);
    }
    let Some(typed) = typed else {
        keys.clear();
        return;
    };
    let mut name = typed.clone();
    for k in keys.read() {
        if !k.state.is_pressed() {
            continue;
        }
        match &k.logical_key {
            Key::Backspace => {
                name.pop();
            }
            Key::Enter => {
                presses.write(MenuPress(MenuAction::NameBuild));
            }
            Key::Space => name = build_name_typed(&name, " "),
            Key::Character(s) => name = build_name_typed(&name, s),
            _ => {}
        }
    }
    if name != typed {
        if let Screen::Sheet(s) = &mut menus.screen {
            s.build_name = Some(name);
        }
    }
}

/// A saved build's tooltip: its name or number, the talent and prestige points it puts back, and
/// what a click and a right click do; an empty one says how to fill it.
fn build_tip(slot: u8, build: Option<&esurv_protocol::SavedBuild>, lit: bool) -> Vec<TipLine> {
    let Some(b) = build else {
        let how = if lit {
            "Save keeps what the class holds now here."
        } else {
            "Click to light it, then Save to keep what the class holds now."
        };
        return vec![
            TipLine::new(format!("Build {}", slot + 1), palette::VALUE),
            TipLine::new("Empty.", palette::LABEL),
            TipLine::new(how, palette::GREEN),
        ];
    };
    let name = if b.name.is_empty() { format!("Build {}", slot + 1) } else { b.name.clone() };
    let talents: u32 = b.talents.iter().map(|(_, r)| r).sum();
    let prestige: u32 = b.prestige.iter().map(|(_, r)| r).sum();
    vec![
        TipLine::new(name, palette::VALUE),
        TipLine::new(format!("{talents} talent points \u{b7} {prestige} prestige points"), palette::HEADING),
        TipLine::new("Click to switch to it.", palette::GREEN),
        TipLine::new("Right-click to empty it.", palette::GREEN),
    ]
}

/// The prestige section: its points to spend, earned and spent, the
/// bar to the next point, and every stat the class can buy with its icon, rank and bonus.
fn prestige_section(
    left: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    progress: &Progress,
) {
    let class = sheet.draft.class.as_str();
    let p = &progress.prestige;
    let mut node = look.panel_node();
    node.padding = UiRect::all(look.px(10.0));
    node.row_gap = look.px(8.0);
    node.flex_shrink = 0.0;
    left.spawn(look.panel(node)).with_children(|panel| {
        // the head: name, points to spend, earned and spent, the bar to the next point
        panel
            .spawn(Node {
                column_gap: look.px(14.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|h| {
                h.spawn(look.text(Face::Display, 17.0, palette::HEADING, "Prestige"));
                let free = prestige::points_free(progress);
                h.spawn(look.text(
                    Face::Display,
                    17.0,
                    if free > 0 {
                        palette::GREEN
                    } else {
                        palette::LABEL
                    },
                    format!("{free} to spend"),
                ));
                h.spawn(look.text(
                    Face::Plain,
                    10.0,
                    palette::LABEL,
                    format!("Earned {} \u{b7} Spent {}", p.points, p.spent),
                ));
                h.spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                });
                experience_bar(h, look, p.into_next, p.next_cost, Some(180.0));
            });
        panel.spawn(look.hint(
            "Every talent point is earned: talent experience now buys prestige points, each a \
             little dearer. A point buys a rank of a stat; a talent reset gives them all back.",
        ));
        // the stats, as many to a row as fit
        panel
            .spawn(Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: look.px(6.0),
                row_gap: look.px(6.0),
                ..default()
            })
            .with_children(|grid| {
                for stat in esurv_sim::game::prestige::stats_of(ctx.content, class) {
                    prestige_cell(grid, look, ctx, sheet, progress, stat);
                }
            });
    });
}

/// One prestige stat: its icon (picked glowing, one that can take no rank dimmed), its name,
/// and its rank with what it gives.
fn prestige_cell(
    grid: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    progress: &Progress,
    stat: &esurv_content::PrestigeStat,
) {
    let class = sheet.draft.class.as_str();
    let v = prestige::view(ctx.content, class, progress, stat);
    let picked = sheet.prestige.as_deref() == Some(stat.id.as_str());
    grid.spawn(Node {
        width: look.px(150.0),
        column_gap: look.px(6.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|cell| {
        look.icon(
            cell,
            look.icon_art(&stat.icon),
            None,
            26.0,
            picked,
            false,
            v.rank == 0 && !v.can_learn,
        )
        .insert((
            MenuAction::Prestige(stat.id.clone()),
            Tip(prestige::tip(ctx.content, class, progress, stat, picked)),
            MenuLookTip,
        ));
        cell.spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_shrink: 1.0,
            ..default()
        })
        .with_children(|t| {
            t.spawn(look.text(Face::Plain, 9.5, palette::VALUE, stat.name.clone()));
            let colour = if v.max_rank.is_some_and(|m| v.rank >= m) {
                palette::HEADING
            } else if v.can_learn {
                palette::GREEN
            } else {
                palette::LABEL
            };
            t.spawn(look.text(
                Face::Narrow,
                10.0,
                colour,
                format!("{} \u{b7} {}", v.rank, v.bonus),
            ));
        });
    });
}

/// A bar of talent experience into the next point (purple), `into / cost` written on it; full,
/// with "Every point earned", when there is no next point. As wide as its column, or `width`.
fn experience_bar(
    p: &mut ChildSpawnerCommands,
    look: &MenuLook,
    into: u32,
    cost: u32,
    width: Option<f32>,
) {
    let share = if cost > 0 {
        (into as f32 / cost as f32).clamp(0.0, 1.0)
    } else {
        1.0
    };
    p.spawn((
        Node {
            width: width.map_or(Val::Auto, |w| look.px(w)),
            height: look.px(14.0),
            border: UiRect::all(look.px(1.0)),
            border_radius: BorderRadius::all(look.px(3.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        BorderColor::all(palette::GOLD_DIM),
    ))
    .with_children(|bar| {
        bar.spawn((
            Node {
                width: Val::Percent(share * 100.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(look.px(2.0)),
                ..default()
            },
            BackgroundColor(EXPERIENCE),
        ));
        bar.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|t| {
            let text = if cost > 0 {
                format!("{into} / {cost}")
            } else {
                "Every point earned".to_owned()
            };
            t.spawn(look.text(Face::Narrow, 10.0, palette::VALUE, text));
        });
    });
}

/// One tree: its head, its art with its talents, its milestones.
#[allow(clippy::too_many_arguments)]
fn tree_column(
    row: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    progress: &Progress,
    ranks: &Ranks,
    index: usize,
    tree: &TalentTree,
) {
    let class = sheet.draft.class.as_str();
    let mut node = look.panel_node();
    node.flex_grow = 1.0;
    node.flex_basis = Val::Px(0.0);
    node.padding = UiRect::all(look.px(8.0));
    node.row_gap = look.px(6.0);
    row.spawn(look.panel(node)).with_children(|col| {
        // the head: icon, name, points spent
        col.spawn(Node {
            column_gap: look.px(8.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|h| {
            h.spawn((
                ImageNode::new(look.icon_art(&tree.icon)),
                Node {
                    width: look.px(26.0),
                    height: look.px(26.0),
                    border_radius: BorderRadius::all(look.px(3.0)),
                    ..default()
                },
            ));
            h.spawn(look.text(Face::Display, 17.0, palette::HEADING, tree.name.clone()));
            h.spawn(Node {
                flex_grow: 1.0,
                ..default()
            });
            let spent = talents::points_in(ctx.content, class, ranks, index);
            h.spawn(look.text(Face::Plain, 12.0, palette::VALUE, format!("{spent}")));
        });
        // the art, the stock frame's shape, its top rows shown, the talents over it
        col.spawn(Node {
            width: Val::Percent(100.0),
            aspect_ratio: Some(ART_WIDTH / ART_SHOWN),
            overflow: Overflow::clip(),
            border_radius: BorderRadius::all(look.px(4.0)),
            ..default()
        })
        .with_children(|frame| {
            frame
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0 * ART_HEIGHT / ART_SHOWN),
                    ..default()
                })
                .with_children(|art| {
                    // the four stock parts: 256 and 64 wide, 256 and 128 tall
                    for (part, left, top, w, h) in [
                        ("TopLeft", 0.0, 0.0, 80.0, 66.667),
                        ("TopRight", 80.0, 0.0, 20.0, 66.667),
                        ("BottomLeft", 0.0, 66.667, 80.0, 33.333),
                        ("BottomRight", 80.0, 66.667, 20.0, 33.333),
                    ] {
                        art.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(left),
                                top: Val::Percent(top),
                                width: Val::Percent(w),
                                height: Val::Percent(h),
                                ..default()
                            },
                            ImageNode::new(look.art(&format!(
                                "Interface\\TalentFrame\\{}-{part}",
                                tree.background
                            )))
                            .with_color(Color::srgb(0.75, 0.75, 0.75)),
                            Pickable::IGNORE,
                        ));
                    }
                });
            lines(frame, look, ranks, tree);
            for t in &tree.talents {
                talent(
                    frame,
                    look,
                    ctx,
                    sheet,
                    progress,
                    ranks,
                    (index, columns(tree)),
                    t,
                );
            }
        });
        // the milestones: what 10, 20 and 30 points open
        let milestones: Vec<&Talent> = tree
            .talents
            .iter()
            .filter(|t| talents::is_milestone(t))
            .collect();
        if !milestones.is_empty() {
            col.spawn(look.label("Milestones"));
            col.spawn(Node {
                column_gap: look.px(8.0),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            })
            .with_children(|m| {
                for t in milestones {
                    let v = talents::view(ctx.content, class, ranks, progress.points, index, t);
                    m.spawn(Node {
                        column_gap: look.px(4.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|one| {
                        look.icon(
                            one,
                            look.icon_art(&t.icon),
                            None,
                            24.0,
                            sheet.talent.as_deref() == Some(t.id.as_str()),
                            false,
                            v.look == TalentLook::Closed,
                        )
                        .insert((
                            MenuAction::Talent(t.id.clone()),
                            Tip(tip(ctx, class, ranks, progress.points, index, t, sheet.talent.as_deref() == Some(t.id.as_str()))),
                            MenuLookTip,
                        ));
                        one.spawn(look.text(
                            Face::Plain,
                            9.0,
                            palette::LABEL,
                            format!("{} points", POINTS_PER_ROW * u32::from(t.row)),
                        ));
                    });
                }
            });
        }
    });
}

/// Where a talent sits in its tree, as a share of the tree's width: its left, its top, its size;
/// the tree's columns centred across it.
fn place(t: &Talent, columns: u8) -> (f32, f32, f32) {
    let used = f32::from(columns.max(1) - 1) * GRID_COLUMN + GRID_ICON;
    let left = (GRID_WIDTH - used) / 2.0;
    let x = (left + f32::from(t.col) * GRID_COLUMN) / GRID_WIDTH;
    let y = (GRID_INSET + f32::from(t.row) * GRID_ROW) / GRID_WIDTH;
    (x, y, GRID_ICON / GRID_WIDTH)
}

/// The frame's height as a share of its width.
const SHOWN_PER_WIDTH: f32 = ART_SHOWN / ART_WIDTH;

/// How many columns a tree's talents stand in.
fn columns(tree: &TalentTree) -> u8 {
    tree.talents.iter().map(|t| t.col + 1).max().unwrap_or(1)
}

/// One talent: its icon (picked glowing, not open yet dimmed) and its rank.
#[allow(clippy::too_many_arguments)]
fn talent(
    frame: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    progress: &Progress,
    ranks: &Ranks,
    (tree, columns_of): (usize, u8),
    t: &Talent,
) {
    let class = sheet.draft.class.as_str();
    let v = talents::view(ctx.content, class, ranks, progress.points, tree, t);
    let (x, y, size) = place(t, columns_of);
    frame
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(x * 100.0),
                top: Val::Percent(y / SHOWN_PER_WIDTH * 100.0),
                width: Val::Percent(size * 100.0),
                aspect_ratio: Some(1.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|slot| {
            let mut icon = look.icon(
                slot,
                look.icon_art(&t.icon),
                None,
                0.0,
                sheet.talent.as_deref() == Some(t.id.as_str()),
                false,
                v.look == TalentLook::Closed,
            );
            icon.insert((
                MenuAction::Talent(t.id.clone()),
                Tip(tip(ctx, class, ranks, progress.points, tree, t, sheet.talent.as_deref() == Some(t.id.as_str()))),
                MenuLookTip,
            ));
            // the icon fills the slot
            icon.insert(Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(look.px(1.5)),
                border: UiRect::all(look.px(1.0)),
                border_radius: BorderRadius::all(look.px(3.0)),
                ..default()
            });
            let colour = match v.look {
                TalentLook::Maxed => palette::HEADING,
                TalentLook::Open => palette::GREEN,
                TalentLook::Closed => palette::LABEL,
            };
            slot.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: look.px(-5.0),
                    bottom: look.px(-5.0),
                    padding: UiRect::axes(look.px(3.0), look.px(0.0)),
                    border: UiRect::all(look.px(1.0)),
                    border_radius: BorderRadius::all(look.px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
                BorderColor::all(colour.with_alpha(0.6)),
                Pickable::IGNORE,
            ))
            .with_children(|b| {
                b.spawn(look.text(
                    Face::Narrow,
                    10.0,
                    colour,
                    format!("{}/{}", v.rank, v.max_rank),
                ));
            });
        });
}

/// The lines from each talent to the one it needs: gold once that one is at its last rank.
fn lines(frame: &mut ChildSpawnerCommands, look: &MenuLook, ranks: &Ranks, tree: &TalentTree) {
    for t in &tree.talents {
        let Some(need) = t
            .requires
            .as_ref()
            .and_then(|id| tree.talents.iter().find(|x| &x.id == id))
        else {
            continue;
        };
        let met = ranks.get(&need.id).copied().unwrap_or(0) >= u32::from(need.max_rank);
        let colour = if met { palette::GOLD } else { palette::LOCKED };
        let (fx, fy, size) = place(need, columns(tree));
        let (tx, ty, _) = place(t, columns(tree));
        let half = size / 2.0;
        let thick = look.px(2.0);
        // a share of the width to a percent of the frame's height
        let down = |y: f32| Val::Percent(y / SHOWN_PER_WIDTH * 100.0);
        let across = |x: f32| Val::Percent(x * 100.0);
        let mut seg = |left: Val, top: Val, width: Val, height: Val| {
            frame.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left,
                    top,
                    width,
                    height,
                    ..default()
                },
                BackgroundColor(colour),
                Pickable::IGNORE,
            ));
        };
        if need.row == t.row {
            // across, between their sides
            let (a, b) = if fx < tx {
                (fx + size, tx)
            } else {
                (tx + size, fx)
            };
            seg(across(a), down(fy + half), across(b - a), thick);
        } else {
            // across at the needed one's row to the talent's column, then down to its top
            if (fx - tx).abs() > f32::EPSILON {
                let (a, b) = if fx < tx {
                    (fx + size, tx + half)
                } else {
                    (tx + half, fx)
                };
                seg(across(a), down(fy + half), across(b - a), thick);
            }
            let start = if (fx - tx).abs() > f32::EPSILON {
                fy + half
            } else {
                fy + size
            };
            seg(across(tx + half), down(start), thick, down(ty - start));
        }
    }
}

/// The right column: the points, the bar, Reset, the talent picked.
fn points_column(
    r: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    progress: &Progress,
    ranks: &Ranks,
) {
    let class = sheet.draft.class.as_str();
    let mut node = look.panel_node();
    node.position_type = PositionType::Absolute;
    node.right = look.px(MARGIN);
    node.top = look.px(TOP);
    node.bottom = look.px(MARGIN);
    node.width = look.px(SIDE);
    node.row_gap = look.px(8.0);
    r.spawn(look.panel(node)).with_children(|p| {
        look.heading(p, "Talent Points", 18.0);
        let free = talents::points_free(progress);
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|c| {
            c.spawn(look.text(
                Face::Display,
                44.0,
                if free > 0 {
                    palette::GREEN
                } else {
                    palette::LABEL
                },
                free.to_string(),
            ));
            c.spawn(look.label("To spend"));
        });
        p.spawn(Node {
            column_gap: look.px(8.0),
            ..default()
        })
        .with_children(|row| {
            look.value_box(
                row,
                "Earned",
                &format!("{} of {MAX_TALENT_POINTS}", progress.points),
                palette::VALUE,
            );
            look.value_box(row, "Spent", &progress.spent.to_string(), palette::VALUE);
        });
        // the bar to the next point
        p.spawn(look.label("Talent experience"));
        experience_bar(p, look, progress.into_next, progress.next_cost, None);
        p.spawn(look.hint(
            "Runs earn talent experience: the time lasted, the foes and bosses felled, a victory, \
             and harder zones and run options. Every point costs a little more.",
        ));
        // a reset gives the prestige points back too
        let anything_spent = progress.spent > 0 || progress.prestige.spent > 0;
        look.button(p, "Reset", ButtonStyle::Text, anything_spent, None)
            .insert(MenuAction::ResetTalents);
        p.spawn(look.divider());
        if sheet.prestige.is_some() {
            picked_prestige(p, look, ctx, sheet, progress, class);
        } else {
            picked(p, look, ctx, sheet, progress, ranks, class);
        }
        p.spawn(Node {
            flex_grow: 1.0,
            ..default()
        });
        class_wide_hint(look, p, sheet);
    });
}

/// The talent picked: its icon, name and rank, what it does now and at its next rank, Learn and
/// Unlearn.
fn picked(
    p: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    progress: &Progress,
    ranks: &Ranks,
    class: &str,
) {
    let found = sheet.talent.as_ref().and_then(|id| {
        talents::trees(ctx.content, class)
            .into_iter()
            .enumerate()
            .find_map(|(i, tree)| tree.talents.iter().find(|t| &t.id == id).map(|t| (i, t)))
    });
    let Some((tree, t)) = found else {
        p.spawn(look.hint(
            "Click a talent to pick it and read it; click it again to learn a rank, right-click \
             it to take one back.",
        ));
        return;
    };
    let v = talents::view(ctx.content, class, ranks, progress.points, tree, t);
    p.spawn(Node {
        column_gap: look.px(10.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|row| {
        look.icon(row, look.icon_art(&t.icon), None, 40.0, true, false, false);
        row.spawn(Node {
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|n| {
            n.spawn(look.text(Face::Plain, 12.5, palette::HEADING, t.name.clone()));
            n.spawn(look.label(&format!("Rank {} of {}", v.rank, v.max_rank)));
        });
    });
    p.spawn(look.text(
        Face::Plain,
        10.5,
        palette::VALUE,
        talents::talent_text(t, v.rank),
    ));
    if v.rank > 0 && v.rank < v.max_rank {
        p.spawn(look.label("Next rank"));
        p.spawn(look.text(
            Face::Plain,
            10.5,
            palette::VALUE,
            talents::talent_text(t, v.rank + 1),
        ));
    }
    for line in tip(ctx, class, ranks, progress.points, tree, t, true)
        .into_iter()
        .filter(|l| l.text.starts_with("Requires"))
    {
        p.spawn(look.text(Face::Plain, 10.0, palette::RED, line.text));
    }
    p.spawn(Node {
        column_gap: look.px(8.0),
        ..default()
    })
    .with_children(|b| {
        look.button(b, "Learn", ButtonStyle::Primary, v.can_learn, Some(120.0))
            .insert(MenuAction::Learn);
        look.button(b, "Unlearn", ButtonStyle::Text, v.can_unlearn, Some(120.0))
            .insert(MenuAction::Unlearn);
    });
}

/// The prestige stat picked: its icon, name and rank, what it gives now and at its next rank,
/// Learn and Unlearn.
fn picked_prestige(
    p: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    progress: &Progress,
    class: &str,
) {
    let Some(stat) = sheet
        .prestige
        .as_ref()
        .and_then(|id| ctx.content.prestige.iter().find(|s| &s.id == id))
    else {
        return;
    };
    let v = prestige::view(ctx.content, class, progress, stat);
    p.spawn(Node {
        column_gap: look.px(10.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|row| {
        look.icon(row, look.icon_art(&stat.icon), None, 40.0, true, false, false);
        row.spawn(Node {
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|n| {
            n.spawn(look.text(Face::Plain, 12.5, palette::HEADING, stat.name.clone()));
            n.spawn(look.label(&format!("Prestige {}", prestige::rank_text(&v))));
        });
    });
    let name = stat.name.to_lowercase();
    p.spawn(look.text(
        Face::Plain,
        10.5,
        palette::VALUE,
        format!("{} {name}", v.bonus),
    ));
    if v.max_rank.is_none_or(|max| v.rank < max) {
        p.spawn(look.label("Next rank"));
        p.spawn(look.text(
            Face::Plain,
            10.5,
            palette::VALUE,
            format!("{} {name}", v.next_bonus),
        ));
    }
    p.spawn(Node {
        column_gap: look.px(8.0),
        ..default()
    })
    .with_children(|b| {
        look.button(b, "Learn", ButtonStyle::Primary, v.can_learn, Some(120.0))
            .insert(MenuAction::Learn);
        look.button(b, "Unlearn", ButtonStyle::Text, v.can_unlearn, Some(120.0))
            .insert(MenuAction::Unlearn);
    });
}

/// Every rank bought, taken back by the rules, the deepest rows first; the asks in order.
pub(in crate::menus) fn reset_order(
    content: &esurv_content::Content,
    class: &str,
    ranks: &Ranks,
) -> Vec<String> {
    let mut left = ranks.clone();
    let mut order = Vec::new();
    let mut all: Vec<&Talent> = talents::trees(content, class)
        .into_iter()
        .flat_map(|t| &t.talents)
        .collect();
    all.sort_by_key(|t| std::cmp::Reverse(t.row));
    // each pass takes back what the rules let go; a pass that takes nothing ends it
    loop {
        let before = order.len();
        for t in &all {
            while left.get(&t.id).copied().unwrap_or(0) > 0 {
                let mut after = left.clone();
                if !esurv_sim::game::talents::unlearn(content, class, &mut after, &t.id) {
                    break;
                }
                left = after;
                order.push(t.id.clone());
            }
        }
        if order.len() == before {
            break;
        }
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reset_takes_back_every_rank_by_the_rules() {
        let content = esurv_content::Content::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../content"
        ))
        .unwrap();
        let tree = content
            .talent_trees
            .iter()
            .find(|t| t.class == "warrior")
            .unwrap();
        let first = tree.talents.iter().find(|t| t.row == 0).unwrap();
        let mut ranks = Ranks::new();
        ranks.insert(first.id.clone(), u32::from(first.max_rank));
        let order = reset_order(&content, "warrior", &ranks);
        assert_eq!(order.len(), first.max_rank as usize);
        assert!(order.iter().all(|id| id == &first.id));
    }

    /// A build name keeps letters, digits, spaces, dashes and apostrophes, up to the limit.
    #[test]
    fn a_build_name_keeps_what_can_be_typed() {
        assert_eq!(build_name_typed("Fire", " & frost!"), "Fire  frost");
        assert_eq!(build_name_typed("", "Kel'Thas-2"), "Kel'Thas-2");
        let long = build_name_typed("", &"a".repeat(40));
        assert_eq!(long.chars().count(), esurv_protocol::BUILD_NAME_LIMIT);
        assert_eq!(build_name_typed(&long, "b"), long, "nothing past the limit");
    }
}
