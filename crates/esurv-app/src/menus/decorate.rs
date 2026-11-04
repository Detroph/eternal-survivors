//! Decorate camp (docs/game/screens.md, "Decorate camp"): opened by the button beside the round
//! camp switch, it decorates the camp shown, the Horde's and the Alliance's apart (what the
//! warband owns may stand in both). The camp stays in view with a numbered gold marker on each
//! spot (the fire, the banner post, the camp theme, the two extra spots; [`place_spot_markers`]
//! keeps them there); the panel at the right lists the spots and, for the one picked, what fits
//! it: the warband's bright (a click sets one out), the others grey with what earns them or their
//! price; for the camp theme, the sets. A spot not yet open is opened for points (asked first).
//! The panel at the left tells what grows with progress (the fire, the pennant line, the trophy
//! corner, the skull spear, the camp sign), switches camps, and closes.

use bevy::prelude::*;
use esurv_content::{CampSpot, Content, SpotKind};
use esurv_protocol::Achievements;

use super::look::{palette, ButtonStyle, Face, MenuLook};
use super::scroll::{scrolling, ScrollingList};
use super::{Ctx, DecorateState, MenuAction};
use crate::achievements::cosmetics::{
    camp_word, earned_words, spot_choices, spot_name, theme_choices, with_holdings,
};
use crate::stock::{MenuLookTip, Tip, TipLine};

/// Sizes, in units.
const MARGIN: f32 = 16.0;
const SIDE: f32 = 300.0;
const PIECE_ICON: f32 = 26.0;
const MARKER: f32 = 26.0;

/// A spot's marker over the camp, placed on its spot as the camera sees it.
#[derive(Component, Clone, Debug)]
pub(super) struct SpotMarker {
    /// WoW world coordinates.
    at: [f32; 3],
}

/// What stands on spot `s` in words, and its piece's icon.
fn on_spot(
    content: &Content,
    told: &Achievements,
    camp: &str,
    s: &CampSpot,
) -> (String, Option<String>) {
    if s.kind == SpotKind::Theme {
        let set = told
            .on_spot(camp, &s.id)
            .and_then(|id| content.decorations.sets.iter().find(|x| x.id == id));
        return match set {
            Some(x) => {
                let icon = content.set_pieces(&x.id).next().map(|d| d.icon.clone());
                (x.name.clone(), icon)
            }
            None => ("None: no set shows".into(), None),
        };
    }
    let piece = told
        .on_spot(camp, &s.id)
        .and_then(|p| content.decoration(p));
    match (piece, s.kind) {
        (Some(d), _) => (d.name.clone(), Some(d.icon.clone())),
        (None, SpotKind::Campfire) => (
            "The camp fire (grows with your best Challenge clear)".into(),
            None,
        ),
        (None, _) => ("Empty".into(), None),
    }
}

pub(super) fn draw(
    commands: &mut Commands,
    root: Entity,
    look: &MenuLook,
    ctx: &Ctx,
    state: &DecorateState,
) {
    let empty = Achievements::default();
    let told = ctx.facts.achievements().unwrap_or(&empty);
    let camp = camp_word(ctx.camp);
    let content = ctx.content;
    commands.entity(root).with_children(|r| {
        left_panel(r, look, ctx, told);
        right_panel(r, look, ctx, told, camp, state);
        // the markers, one on each spot
        let place = crate::picker::faction_scene(ctx.camp);
        for (i, s) in content.decorations.spots.iter().enumerate() {
            let picked = state.spot.as_deref() == Some(s.id.as_str());
            let open = with_holdings(told, |h| content.spot_open(&s.id, h));
            let style = if picked {
                ButtonStyle::IconPicked
            } else {
                ButtonStyle::Icon
            };
            let at = place.point(s.at.0, s.at.1);
            let (words, _) = on_spot(content, told, camp, s);
            let mut tip = vec![
                TipLine::new(spot_name(s), palette::HEADING),
                TipLine::new(words, palette::VALUE),
            ];
            if !open {
                tip.push(TipLine::new(
                    format!("Opens for {} Moments", s.price.unwrap_or(0)),
                    palette::LABEL,
                ));
            }
            r.spawn((
                SpotMarker { at },
                Node {
                    position_type: PositionType::Absolute,
                    width: look.px(MARKER),
                    height: look.px(MARKER),
                    border: UiRect::all(look.px(2.0)),
                    border_radius: BorderRadius::MAX,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                super::look::MenuButton {
                    style,
                    enabled: true,
                },
                MenuAction::DecorateSpot(s.id.clone()),
                Tip(tip),
                MenuLookTip,
                Visibility::Hidden,
            ))
            .with_children(|m| {
                let ink = if open {
                    palette::HEADING
                } else {
                    palette::HINT
                };
                m.spawn((
                    look.text(Face::Narrow, 12.0, ink, (i + 1).to_string()),
                    Pickable::IGNORE,
                ));
            });
        }
    });
}

/// The left panel: the camp decorated, what grows with progress, the camp switch and Done.
fn left_panel(r: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, told: &Achievements) {
    let content = ctx.content;
    let mut node = look.panel_node();
    node.position_type = PositionType::Absolute;
    node.left = look.px(MARGIN);
    node.top = look.px(MARGIN);
    node.width = look.px(SIDE - 40.0);
    node.row_gap = look.px(6.0);
    r.spawn(look.panel(node)).with_children(|p| {
        let title = match ctx.camp {
            esurv_content::Faction::Horde => "Decorate the Horde camp",
            esurv_content::Faction::Alliance => "Decorate the Alliance camp",
        };
        look.heading(p, title, 17.0);
        p.spawn(look.hint(
            "Pick a spot by its number, then what goes there. The camp theme shows one bought set, \
             each piece in its own place. Each camp is decorated apart; what the warband owns can \
             stand in both.",
        ));
        p.spawn(look.label("Grows with your progress"));
        let deco = &content.decorations;
        let owns = |d: &&&esurv_content::Decoration| {
            with_holdings(told, |h| content.owns_decoration(&d.id, h))
        };
        let trophies: Vec<_> = deco.pieces.iter().filter(|d| d.trophy()).collect();
        let trophies_earned = trophies.iter().filter(owns).count();
        let signs: Vec<_> = deco
            .pieces
            .iter()
            .filter(|d| d.spot == SpotKind::Sign)
            .collect();
        let signs_earned = signs.iter().filter(owns).count();
        let banners = deco
            .banner_line
            .banners
            .iter()
            .filter(|b| told.zones_cleared.iter().any(|z| *z == b.zone))
            .count();
        let earned = |a: &Option<String>| {
            a.as_deref()
                .is_none_or(|a| told.earned.iter().any(|(e, _)| e == a))
        };
        let fires = deco
            .campfire
            .iter()
            .filter(|l| earned(&l.achievement))
            .count();
        let skulls = deco
            .skull_spear
            .skulls
            .iter()
            .filter(|l| earned(&l.achievement))
            .count();
        for line in [
            format!(
                "Camp fire: step {fires} of {} (2x, 3x and 5x Challenge clears)",
                deco.campfire.len()
            ),
            format!(
                "Pennant line: {banners} of {} pennants (one for each zone cleared)",
                deco.banner_line.banners.len()
            ),
            format!(
                "Trophy corner: {trophies_earned} of {} trophies",
                trophies.len()
            ),
            format!(
                "Skull spear: {skulls} of {} skulls (the Slayer steps)",
                deco.skull_spear.skulls.len()
            ),
            format!(
                "Camp sign: {signs_earned} of {} sets owned whole",
                signs.len()
            ),
        ] {
            p.spawn(look.text(Face::Plain, 10.0, palette::VALUE, line));
        }
        p.spawn(Node {
            column_gap: look.px(8.0),
            margin: UiRect::top(look.px(6.0)),
            ..default()
        })
        .with_children(|row| {
            let other = match ctx.camp {
                esurv_content::Faction::Horde => "Alliance camp",
                esurv_content::Faction::Alliance => "Horde camp",
            };
            look.button(row, other, ButtonStyle::Text, true, Some(110.0))
                .insert(MenuAction::SwitchCamp);
            look.button(row, "Done", ButtonStyle::Primary, true, Some(90.0))
                .insert(MenuAction::CloseDecorate);
        });
    });
}

/// The right panel: the spots, then the picked one's pieces.
fn right_panel(
    r: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    told: &Achievements,
    camp: &str,
    state: &DecorateState,
) {
    let content = ctx.content;
    let mut node = look.panel_node();
    node.position_type = PositionType::Absolute;
    node.right = look.px(MARGIN);
    node.top = look.px(MARGIN);
    node.bottom = look.px(MARGIN);
    node.width = look.px(SIDE);
    node.row_gap = look.px(6.0);
    r.spawn(look.panel(node)).with_children(|p| {
        look.heading(p, "Spots", 16.0);
        p.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: Val::ZERO,
                row_gap: look.px(4.0),
                min_height: Val::ZERO,
                overflow: Overflow::scroll_y(),
                padding: UiRect::right(look.px(6.0)),
                ..default()
            },
            scrolling(ScrollingList::Decorate),
        ))
        .with_children(|l| {
            for (i, s) in content.decorations.spots.iter().enumerate() {
                let picked = state.spot.as_deref() == Some(s.id.as_str());
                let open = with_holdings(told, |h| content.spot_open(&s.id, h));
                let (words, icon) = on_spot(content, told, camp, s);
                let words = if open {
                    words
                } else {
                    format!("Opens for {} Moments", s.price.unwrap_or(0))
                };
                let style = if picked {
                    ButtonStyle::RowPicked
                } else {
                    ButtonStyle::Row
                };
                let mut b = look.frame_button(l, style, true, None);
                b.insert((
                    MenuAction::DecorateSpot(s.id.clone()),
                    Node {
                        flex_shrink: 0.0,
                        column_gap: look.px(6.0),
                        align_items: AlignItems::Center,
                        padding: UiRect::all(look.px(4.0)),
                        border: UiRect::all(look.px(super::look::BORDER)),
                        border_radius: BorderRadius::all(look.px(4.0)),
                        ..default()
                    },
                ));
                b.with_children(|b| {
                    b.spawn((
                        look.text(Face::Narrow, 12.0, palette::HEADING, format!("{}", i + 1)),
                        Node {
                            width: look.px(16.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ));
                    // a spot with nothing on it shows no icon (an empty square, not a white one)
                    let tint = match (&icon, open) {
                        (None, _) => Color::NONE,
                        (Some(_), true) => Color::WHITE,
                        (Some(_), false) => palette::LOCKED,
                    };
                    let image = icon.map_or_else(Handle::default, |i| look.icon_art(&i));
                    b.spawn((
                        ImageNode::new(image).with_color(tint),
                        Node {
                            width: look.px(PIECE_ICON - 4.0),
                            height: look.px(PIECE_ICON - 4.0),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        Pickable::IGNORE,
                    ));
                    b.spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|t| {
                        t.spawn((
                            look.text(Face::Plain, 10.0, palette::LABEL, spot_name(s)),
                            Pickable::IGNORE,
                        ));
                        let ink = if open { palette::VALUE } else { palette::HINT };
                        t.spawn((look.text(Face::Plain, 10.5, ink, words), Pickable::IGNORE));
                    });
                });
            }
            if let Some(s) = state.spot.as_deref().and_then(|id| content.camp_spot(id)) {
                spot_pieces(l, look, ctx, told, camp, s, state);
            }
        });
    });
}

/// The picked spot's pieces, or opening it.
fn spot_pieces(
    l: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    told: &Achievements,
    camp: &str,
    s: &CampSpot,
    state: &DecorateState,
) {
    let content = ctx.content;
    l.spawn((
        look.label(&format!("{}: what can stand there", spot_name(s))),
        Node {
            flex_shrink: 0.0,
            margin: UiRect::top(look.px(8.0)),
            ..default()
        },
    ));
    let open = with_holdings(told, |h| content.spot_open(&s.id, h));
    if !open {
        let price = s.price.unwrap_or(0);
        l.spawn(look.hint(format!(
            "This spot opens for {price} Moments, in both camps. You have {} to spend.",
            told.balance
        )));
        if state.confirm {
            l.spawn(look.text(
                Face::Plain,
                11.0,
                palette::VALUE,
                format!("Spend {price} Moments on this spot?"),
            ));
            l.spawn(Node {
                column_gap: look.px(8.0),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|row| {
                look.button(row, "Yes", ButtonStyle::Primary, true, Some(80.0))
                    .insert(MenuAction::DecorateAnswer(true));
                look.button(row, "No", ButtonStyle::Text, true, Some(80.0))
                    .insert(MenuAction::DecorateAnswer(false));
            });
        } else {
            look.button(
                l,
                &format!("Open for {price} Moments"),
                ButtonStyle::Primary,
                price <= told.balance,
                Some(180.0),
            )
            .insert(MenuAction::DecorateBuySpot);
        }
        return;
    }
    let current = told.on_spot(camp, &s.id);
    let none_label = match s.kind {
        SpotKind::Campfire => "The camp fire that grows",
        SpotKind::Theme => "None",
        _ => "Empty",
    };
    let style = if current.is_none() {
        ButtonStyle::RowPicked
    } else {
        ButtonStyle::Row
    };
    look.button(l, none_label, style, current.is_some(), None)
        .insert((
            MenuAction::DecoratePiece(String::new()),
            Node {
                flex_shrink: 0.0,
                padding: UiRect::all(look.px(5.0)),
                border: UiRect::all(look.px(super::look::BORDER)),
                border_radius: BorderRadius::all(look.px(4.0)),
                ..default()
            },
        ));
    if s.kind == SpotKind::Theme {
        theme_rows(l, look, ctx, told, current);
        return;
    }
    let discovered = ctx.facts.zones_discovered();
    for (d, owned) in spot_choices(content, told, s) {
        let here = current == Some(d.id.as_str());
        let elsewhere = told
            .camps
            .iter()
            .find(|c| c.camp == camp)
            .and_then(|c| c.pieces.iter().find(|(sp, p)| *p == d.id && *sp != s.id))
            .and_then(|(sp, _)| content.camp_spot(sp))
            .map(spot_name);
        let source = if owned {
            match elsewhere {
                Some(sp) => format!("Owned, now on {sp}: moves here"),
                None => "Owned".into(),
            }
        } else if let Some(p) = d.price {
            format!("In the Shop for {p} Moments")
        } else {
            earned_words(content, discovered, told, d)
        };
        let style = if here {
            ButtonStyle::RowPicked
        } else {
            ButtonStyle::Row
        };
        let mut b = look.frame_button(l, style, owned && !here, None);
        b.insert((
            MenuAction::DecoratePiece(d.id.clone()),
            Node {
                flex_shrink: 0.0,
                column_gap: look.px(6.0),
                align_items: AlignItems::Center,
                padding: UiRect::all(look.px(4.0)),
                border: UiRect::all(look.px(super::look::BORDER)),
                border_radius: BorderRadius::all(look.px(4.0)),
                ..default()
            },
            Tip(vec![
                TipLine::new(d.name.clone(), palette::VALUE),
                TipLine::new(d.text.clone(), palette::HEADING),
            ]),
            MenuLookTip,
        ));
        b.with_children(|b| {
            // one the warband does not have shows grey (a trophy's outline until earned)
            let tint = if owned { Color::WHITE } else { palette::LOCKED };
            b.spawn((
                ImageNode::new(look.icon_art(&d.icon)).with_color(tint),
                Node {
                    width: look.px(PIECE_ICON),
                    height: look.px(PIECE_ICON),
                    flex_shrink: 0.0,
                    ..default()
                },
                Pickable::IGNORE,
            ));
            b.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|t| {
                let ink = if owned {
                    palette::VALUE
                } else {
                    palette::LABEL
                };
                t.spawn((
                    look.text(Face::Plain, 10.5, ink, d.name.clone()),
                    Pickable::IGNORE,
                ));
                let ink = if here { palette::GREEN } else { palette::HINT };
                let words = if here {
                    "Stands here".to_owned()
                } else {
                    source
                };
                t.spawn((look.text(Face::Narrow, 9.5, ink, words), Pickable::IGNORE));
            });
        });
    }
}

/// The camp theme's choices: each set, with how much of it the warband owns; one with a piece
/// owned can be picked.
fn theme_rows(
    l: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    told: &Achievements,
    current: Option<&str>,
) {
    let content = ctx.content;
    for (set, owned, total) in theme_choices(content, told) {
        let here = current == Some(set.id.as_str());
        let can = owned > 0;
        let style = if here {
            ButtonStyle::RowPicked
        } else {
            ButtonStyle::Row
        };
        let icon = content.set_pieces(&set.id).next().map(|d| d.icon.clone());
        let pieces: Vec<String> = content
            .set_pieces(&set.id)
            .map(|d| d.name.clone())
            .collect();
        let mut b = look.frame_button(l, style, can && !here, None);
        b.insert((
            MenuAction::DecoratePiece(set.id.clone()),
            Node {
                flex_shrink: 0.0,
                column_gap: look.px(6.0),
                align_items: AlignItems::Center,
                padding: UiRect::all(look.px(4.0)),
                border: UiRect::all(look.px(super::look::BORDER)),
                border_radius: BorderRadius::all(look.px(4.0)),
                ..default()
            },
            Tip(vec![
                TipLine::new(set.name.clone(), palette::VALUE),
                TipLine::new(pieces.join(", "), palette::HEADING),
            ]),
            MenuLookTip,
        ));
        b.with_children(|b| {
            let tint = if can { Color::WHITE } else { palette::LOCKED };
            b.spawn((
                ImageNode::new(icon.map_or_else(Handle::default, |i| look.icon_art(&i)))
                    .with_color(tint),
                Node {
                    width: look.px(PIECE_ICON),
                    height: look.px(PIECE_ICON),
                    flex_shrink: 0.0,
                    ..default()
                },
                Pickable::IGNORE,
            ));
            b.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|t| {
                let ink = if can { palette::VALUE } else { palette::LABEL };
                t.spawn((
                    look.text(Face::Plain, 10.5, ink, set.name.clone()),
                    Pickable::IGNORE,
                ));
                let (ink, words) = if here {
                    (
                        palette::GREEN,
                        format!("Shows here: {owned} of {total} pieces owned"),
                    )
                } else if can {
                    (palette::HINT, format!("{owned} of {total} pieces owned"))
                } else {
                    (palette::HINT, "Buy a piece in the Shop".to_owned())
                };
                t.spawn((look.text(Face::Narrow, 9.5, ink, words), Pickable::IGNORE));
            });
        });
    }
}

/// Keeps each spot's marker on its spot as the world camera sees it; hidden off the screen.
pub(super) fn place_spot_markers(
    cams: Query<(&Camera, &GlobalTransform), With<benilla_world::view::WorldCamera>>,
    mut markers: Query<(&SpotMarker, &mut Node, &ComputedNode, &mut Visibility)>,
    screen: Res<benilla_app::SelectScreen>,
) {
    let Ok((cam, at)) = cams.single() else {
        return;
    };
    for (m, mut node, laid, mut vis) in &mut markers {
        let world = benilla_app::SceneFigure::place(m.at, 0.0).translation + Vec3::Y * 0.2;
        let size = laid.size() * laid.inverse_scale_factor();
        let Some(p) = cam
            .world_to_viewport(at, world)
            .ok()
            .filter(|_| screen.scene_ready)
        else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        let (left, top) = (Val::Px(p.x - size.x / 2.0), Val::Px(p.y - size.y / 2.0));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        vis.set_if_neq(Visibility::Inherited);
    }
}
