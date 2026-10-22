//! The card screen's look, in the menus' own look (menus.md, "The
//! look"; [`crate::menus::look`]): over the dimmed world, one near-black panel with the thin
//! warm-gold border and rounded corners; the screen's heading in Morpheus small capitals between
//! gold rules, a grey line under it; the hero's spells (left) and worn jewelry (right) as side
//! columns; between them one card per offer (its key at its top left, its badge at its top
//! right, its icon framed in its rarity colour, the rarity in small capitals, its name in that
//! colour, the line under it, what it brings in pieces of their own colour, and at its foot,
//! quietly, the slot it fills or the amulet it would replace); under them the reroll button, the hero's gold and the keys that work. A card's frame
//! and glow take its rarity colour, stronger under the pointer and strongest once taken; a click
//! anywhere on a card takes it (a card has no Choose button).

use bevy::prelude::*;
use bevy::text::LineBreak;
use bevy::ui::FocusPolicy;

use benilla_app::OverlayWindow;
use esurv_protocol::Choices;
use esurv_sim::game::upgrades::MAX_SPELLS;

use super::card_text::{
    card_text, header, keys_hint, kit_text, reroll_label, Badge, CardText, Line, PieceText, Tone,
    JEWELRY_SLOTS,
};
use super::{answer, can_reroll, CardAction, CardScreen, MAX_CARDS};
use crate::menus::look::{palette, ButtonStyle, Face, MenuLook, BORDER, PAD, RADIUS};
use crate::protocol::Request;
use crate::stock::{MenuLookTip, Stock, StockClick, StockScale, Tip, TipLine};
use crate::ui::{layer, rarity_colour, GameContent, GREEN_TEXT};

/// Sizes, in units.
const CARD_WIDTH: f32 = 158.0;
const CARD_GAP: f32 = 12.0;
const CARD_ICON: f32 = 42.0;
const SIDE_WIDTH: f32 = 140.0;
const SIDE_ICON: f32 = 22.0;
const REROLL_WIDTH: f32 = 150.0;
/// How dark the world goes behind the screen.
const DIM: f32 = 0.35;

/// A click on it takes card `n`: the card, and the amulet it would replace named at its foot.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CardPick(pub usize);

/// A card's frame: which card and its rarity, for its border and glow.
#[derive(Component, Clone, Copy)]
pub(super) struct CardFrame {
    n: usize,
    rarity: u8,
}

#[derive(Component)]
pub(super) struct RerollButton;

#[derive(Component)]
pub(super) struct GoldLabel;

/// A dev aid's word on which card shows as under the pointer (`None`: the pointer decides).
#[derive(Resource, Default)]
pub(super) struct LitByScript(pub Option<usize>);

/// What is on screen: the layer, the cards it shows (by when they came), the scale it was built
/// at, and whether a reroll could be paid then.
#[derive(Default)]
pub(super) struct Shown(Option<(Entity, f32, StockScale, bool)>);

/// Builds the screen when cards come or change, when the scale changes and when a reroll
/// becomes payable or not; takes it down when they close; keeps the gold up to date.
pub(super) fn show_cards(
    mut commands: Commands,
    screen: Res<CardScreen>,
    stock: Stock,
    look: MenuLook,
    content: Option<Res<GameContent>>,
    mut shown: Local<Shown>,
    mut gold: Query<&mut Text, With<GoldLabel>>,
) {
    let want = screen.open.as_ref().map(|open| {
        let usable = can_reroll(
            open.choices.free_rerolls,
            open.choices.reroll_cost,
            screen.gold,
        );
        (open.since, stock.scale(), usable)
    });
    if let Some((layer, since, scale, usable)) = shown.0 {
        if want != Some((since, scale, usable)) {
            commands.entity(layer).despawn();
            shown.0 = None;
        }
    }
    if let (None, Some(open), Some((since, scale, usable))) = (shown.0, screen.open.as_ref(), want)
    {
        let content = content.as_ref().map(|c| &*c.0);
        let layer = spawn_screen(
            &mut commands,
            &stock,
            &look,
            &open.choices,
            content,
            screen.gold,
            usable,
        );
        shown.0 = Some((layer, since, scale, usable));
        return;
    }
    if let Some(g) = screen.gold {
        for mut text in &mut gold {
            let want = g.to_string();
            if text.0 != want {
                text.0 = want;
            }
        }
    }
}

/// Each card's frame and glow: its rarity colour, dim at rest, full under the pointer, and
/// strongest on the card taken.
pub(super) fn light_cards(
    screen: Res<CardScreen>,
    lit: Res<LitByScript>,
    scale: Res<StockScale>,
    mut cards: Query<(&Interaction, &CardFrame, &mut BorderColor, &mut BoxShadow)>,
) {
    let taken = screen.open.as_ref().and_then(|o| o.taken);
    for (interaction, frame, mut edge, mut shadow) in &mut cards {
        let state = if taken == Some(frame.n) {
            CardLight::Taken
        } else if *interaction != Interaction::None || lit.0 == Some(frame.n) {
            CardLight::Lit
        } else {
            CardLight::Rest
        };
        let (e, glow, blur) = card_light(frame.rarity, state);
        let want_edge = BorderColor::all(e);
        if *edge != want_edge {
            *edge = want_edge;
        }
        let want = glow_shadow(glow, scale.px(blur));
        if *shadow != want {
            *shadow = want;
        }
    }
}

/// How lit a card is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CardLight {
    Rest,
    Lit,
    Taken,
}

/// A card's border colour, its glow's colour and the glow's blur (units), by its rarity and how
/// lit it is: the rarity's colour throughout, so the rarity reads at a glance.
fn card_light(rarity: u8, state: CardLight) -> (Color, Color, f32) {
    let colour = rarity_colour(rarity);
    let k = strength(rarity);
    match state {
        CardLight::Rest => (colour.with_alpha(0.65 * k), colour.with_alpha(0.3 * k), 8.0),
        CardLight::Lit => (colour, colour.with_alpha(0.65 * k), 14.0),
        CardLight::Taken => (
            colour.mix(&Color::WHITE, 0.3),
            colour.with_alpha(0.85),
            16.0,
        ),
    }
}

/// How strongly a rarity's colour shows at rest: the common white would read as lit at full
/// strength, so it shows at under half.
fn strength(rarity: u8) -> f32 {
    if rarity == 0 {
        0.45
    } else {
        1.0
    }
}

fn glow_shadow(colour: Color, blur: Val) -> BoxShadow {
    BoxShadow::new(colour, Val::ZERO, Val::ZERO, Val::ZERO, blur)
}

/// A click on a card takes it; the reroll button rerolls.
pub(super) fn answer_clicks(
    time: Res<Time<Real>>,
    mut clicks: MessageReader<StockClick>,
    picks: Query<&CardPick>,
    rerolls: Query<(), With<RerollButton>>,
    mut screen: ResMut<CardScreen>,
    mut requests: MessageWriter<Request>,
) {
    for StockClick(entity) in clicks.read() {
        let action = if let Ok(CardPick(n)) = picks.get(*entity) {
            CardAction::Take(*n)
        } else if rerolls.contains(*entity) {
            CardAction::Reroll
        } else {
            continue;
        };
        answer(&mut screen, action, time.elapsed_secs(), &mut requests);
    }
}

/// A line's colour by its tone.
fn tone_colour(tone: Tone) -> Color {
    match tone {
        Tone::Plain => palette::VALUE,
        Tone::Gain => GREEN_TEXT,
        Tone::Now => palette::LABEL,
    }
}

/// A piece's tooltip: its name (in its rarity colour, else gold), its lines, and `more`.
fn piece_tip(piece: &PieceText, more: &[(&str, Color)]) -> Tip {
    let mut lines = vec![TipLine::new(
        piece.name.clone(),
        piece.rarity.map_or(palette::HEADING, rarity_colour),
    )];
    lines.extend(
        piece
            .lines
            .iter()
            .map(|l| TipLine::new(l.text(), tone_colour(l.tone()))),
    );
    lines.extend(more.iter().map(|(l, c)| TipLine::new(*l, *c)));
    Tip(lines)
}

/// The panel's width for `cards` cards: the cards, the two side columns, the gaps, the padding.
fn panel_width(cards: usize) -> f32 {
    let cards = cards.max(1) as f32;
    cards * CARD_WIDTH + (cards - 1.0) * CARD_GAP + 2.0 * (SIDE_WIDTH + CARD_GAP) + 2.0 * PAD + 8.0
}

fn spawn_screen(
    commands: &mut Commands,
    stock: &Stock,
    look: &MenuLook,
    choices: &Choices,
    content: Option<&esurv_content::Content>,
    gold: Option<u32>,
    usable: bool,
) -> Entity {
    let (title, under) = header(choices.kind, choices.level);
    let shown = choices.offers.len().min(MAX_CARDS);
    let kit = kit_text(&choices.kit, content);
    let centred = TextLayout::new(Justify::Center, LineBreak::WordBoundary);
    commands
        .spawn((
            Name::new("card screen"),
            stock.centred(layer::CARDS),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, DIM)),
        ))
        .with_children(|layer| {
            let mut node = look.panel_node();
            node.width = look.px(panel_width(shown));
            node.align_items = AlignItems::Center;
            node.row_gap = look.px(8.0);
            node.padding = UiRect::new(look.px(PAD), look.px(PAD), look.px(14.0), look.px(14.0));
            layer
                .spawn((OverlayWindow, look.panel(node)))
                .with_children(|panel| {
                    look.heading(panel, &title, 26.0);
                    panel.spawn((look.hint(under), centred));
                    panel
                        .spawn(Node {
                            column_gap: look.px(CARD_GAP),
                            margin: UiRect::vertical(look.px(4.0)),
                            ..default()
                        })
                        .with_children(|row| {
                            spells_column(row, look, &kit.spells);
                            for (n, offer) in choices.offers.iter().take(MAX_CARDS).enumerate() {
                                let card = card_text(offer, &choices.kit, content, choices.level);
                                spawn_card(row, look, n, &card);
                            }
                            jewelry_column(row, look, &kit);
                        });
                    look_footer(panel, look, choices, gold, usable, shown);
                });
        })
        .id()
}

/// Under the cards: the reroll button, the gold, the keys.
fn look_footer(
    panel: &mut ChildSpawnerCommands,
    look: &MenuLook,
    choices: &Choices,
    gold: Option<u32>,
    usable: bool,
    shown: usize,
) {
    panel
        .spawn(Node {
            column_gap: look.px(16.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            look.button(
                row,
                &reroll_label(choices.free_rerolls, choices.reroll_cost),
                ButtonStyle::Text,
                usable,
                Some(REROLL_WIDTH),
            )
            .insert(RerollButton);
            if let Some(g) = gold {
                row.spawn((
                    Node {
                        column_gap: look.px(6.0),
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(look.px(8.0), look.px(4.0)),
                        border: UiRect::all(look.px(BORDER)),
                        border_radius: BorderRadius::all(look.px(4.0)),
                        ..default()
                    },
                    BackgroundColor(palette::INSET),
                    BorderColor::all(palette::GOLD_DIM),
                ))
                .with_children(|b| {
                    b.spawn(look.label("Gold"));
                    b.spawn((
                        GoldLabel,
                        look.text(Face::Plain, 12.0, palette::HEADING, g.to_string()),
                    ));
                });
            }
            row.spawn(look.hint(keys_hint(shown)));
        });
}

/// A side column: a title with a count beside it, a divider, its rows.
fn side_column<'a>(row: &'a mut ChildSpawnerCommands, look: &MenuLook) -> EntityCommands<'a> {
    row.spawn((
        Node {
            width: look.px(SIDE_WIDTH),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            row_gap: look.px(4.0),
            padding: UiRect::all(look.px(8.0)),
            border: UiRect::all(look.px(BORDER)),
            border_radius: BorderRadius::all(look.px(4.0)),
            ..default()
        },
        BackgroundColor(palette::INSET),
        BorderColor::all(palette::GOLD_DIM),
    ))
}

/// A title in Morpheus small capitals with `count` in grey at its right.
fn side_title(col: &mut ChildSpawnerCommands, look: &MenuLook, title: &str, count: Option<String>) {
    col.spawn(Node {
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        width: Val::Percent(100.0),
        ..default()
    })
    .with_children(|r| {
        look.small_caps(r, title, 14.0, palette::HEADING);
        if let Some(count) = count {
            r.spawn(look.text(Face::Plain, 10.0, palette::LABEL, count));
        }
    });
    col.spawn(look.divider());
}

/// A small grey label over a group of rows, with the slots in use beside it.
fn side_label(col: &mut ChildSpawnerCommands, look: &MenuLook, label: &str, count: String) {
    col.spawn(Node {
        justify_content: JustifyContent::SpaceBetween,
        width: Val::Percent(100.0),
        margin: UiRect::top(look.px(2.0)),
        ..default()
    })
    .with_children(|r| {
        r.spawn(look.label(label));
        r.spawn(look.label(&count));
    });
}

/// A row of a side column: the piece's icon in its frame, its name over its rank; its tooltip
/// under the pointer. `None`: an empty slot.
fn side_row(col: &mut ChildSpawnerCommands, look: &MenuLook, piece: Option<&PieceText>) {
    let mut row = col.spawn(Node {
        column_gap: look.px(6.0),
        align_items: AlignItems::Center,
        width: Val::Percent(100.0),
        ..default()
    });
    if let Some(p) = piece {
        row.insert((piece_tip(p, &[]), MenuLookTip));
    }
    row.with_children(|r| {
        let edge = piece.and_then(|p| p.rarity).map_or(
            if piece.is_some() {
                palette::GOLD_DIM
            } else {
                palette::GOLD_DIM.with_alpha(0.2)
            },
            |rarity| rarity_colour(rarity).with_alpha(0.8),
        );
        icon_frame(r, look, piece.map(|p| p.icon.as_str()), SIDE_ICON, edge);
        r.spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            min_width: Val::Px(0.0),
            ..default()
        })
        .with_children(|c| match piece {
            Some(p) => {
                let colour = p.rarity.map_or(palette::VALUE, rarity_colour);
                c.spawn(look.text(Face::Plain, 10.0, colour, p.name.clone()));
                c.spawn(look.text(Face::Plain, 8.0, palette::LABEL, format!("RANK {}", p.rank)));
            }
            None => {
                c.spawn(look.text(Face::Plain, 9.0, palette::LOCKED, "Empty"));
            }
        });
    });
}

/// An icon in a square frame of `edge`, `size` units; `None`: the empty frame.
fn icon_frame(
    parent: &mut ChildSpawnerCommands,
    look: &MenuLook,
    icon: Option<&str>,
    size: f32,
    edge: Color,
) {
    parent
        .spawn((
            Node {
                width: look.px(size),
                height: look.px(size),
                flex_shrink: 0.0,
                padding: UiRect::all(look.px(1.5)),
                border: UiRect::all(look.px(BORDER)),
                border_radius: BorderRadius::all(look.px(3.0)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.6)),
            BorderColor::all(edge),
            Pickable::IGNORE,
        ))
        .with_children(|f| {
            if let Some(icon) = icon {
                f.spawn((
                    ImageNode::new(look.icon_art(icon)),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        border_radius: BorderRadius::all(look.px(2.0)),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
            }
        });
}

/// The left column: the spells known and the slots left.
fn spells_column(row: &mut ChildSpawnerCommands, look: &MenuLook, spells: &[PieceText]) {
    side_column(row, look).with_children(|col| {
        side_title(
            col,
            look,
            "Spells",
            Some(format!("{} / {MAX_SPELLS}", spells.len())),
        );
        for i in 0..MAX_SPELLS.max(spells.len()) {
            side_row(col, look, spells.get(i));
        }
    });
}

/// The right column: the rings, the trinkets and the amulet worn, and the slots left.
fn jewelry_column(
    row: &mut ChildSpawnerCommands,
    look: &MenuLook,
    kit: &super::card_text::KitText,
) {
    side_column(row, look).with_children(|col| {
        side_title(col, look, "Jewelry", None);
        let slots = JEWELRY_SLOTS as usize;
        for (label, pieces) in [("Rings", &kit.rings), ("Trinkets", &kit.trinkets)] {
            side_label(col, look, label, format!("{} / {slots}", pieces.len()));
            for i in 0..slots.max(pieces.len()) {
                side_row(col, look, pieces.get(i));
            }
        }
        side_label(
            col,
            look,
            "Amulet",
            format!("{} / 1", usize::from(kit.amulet.is_some())),
        );
        side_row(col, look, kit.amulet.as_ref());
    });
}

fn spawn_card(row: &mut ChildSpawnerCommands, look: &MenuLook, n: usize, card: &CardText) {
    let px = |u| look.px(u);
    let colour = rarity_colour(card.rarity);
    let centred = TextLayout::new(Justify::Center, LineBreak::WordBoundary);
    let (edge, glow, blur) = card_light(card.rarity, CardLight::Rest);
    row.spawn((
        CardPick(n),
        CardFrame {
            n,
            rarity: card.rarity,
        },
        crate::stock::StockButton::default(),
        Node {
            width: px(CARD_WIDTH),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::new(px(10.0), px(10.0), px(12.0), px(10.0)),
            row_gap: px(4.0),
            border: UiRect::all(px(BORDER)),
            border_radius: BorderRadius::all(px(RADIUS)),
            ..default()
        },
        BackgroundColor(palette::PANEL),
        BorderColor::all(edge),
        glow_shadow(glow, px(blur)),
    ))
    .with_children(|c| {
        // the rarity's colour washes down from the card's top
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::ZERO,
                top: Val::ZERO,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(px(RADIUS)),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
                ColorStop::auto(colour.with_alpha(0.2 * strength(card.rarity))),
                ColorStop::percent(Color::NONE, 45.0),
            ])),
            Pickable::IGNORE,
        ));
        // the key that takes it
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(6.0),
                top: px(6.0),
                width: px(16.0),
                height: px(16.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(BORDER)),
                border_radius: BorderRadius::all(px(3.0)),
                ..default()
            },
            BackgroundColor(palette::INSET),
            BorderColor::all(palette::GOLD_DIM),
            Pickable::IGNORE,
        ))
        .with_children(|k| {
            k.spawn(look.text(Face::Plain, 10.0, palette::HEADING, (n + 1).to_string()));
        });
        // the icon, its tooltip (a spell's school and cooldown) under the pointer
        let mut icon = c.spawn((
            Node {
                margin: UiRect::new(Val::Auto, Val::Auto, px(6.0), px(4.0)),
                ..default()
            },
            BoxShadow::new(
                colour.with_alpha(0.45),
                Val::ZERO,
                Val::ZERO,
                Val::ZERO,
                px(8.0),
            ),
        ));
        if !card.tip.is_empty() {
            let piece = PieceText {
                icon: card.icon.clone(),
                name: card.name.clone(),
                rarity: Some(card.rarity),
                rank: 0,
                lines: card.tip.clone(),
            };
            icon.insert((piece_tip(&piece, &[]), MenuLookTip));
        }
        icon.with_children(|i| {
            icon_frame(i, look, Some(&card.icon), CARD_ICON, colour);
        });
        if let Some(b) = &card.badge {
            badge_box(c, look, b, card.rarity);
        }
        // the rarity in small capitals, the name, the line under it
        c.spawn((
            look.text(
                Face::Plain,
                8.5,
                colour,
                spaced(&card.rarity_name().to_uppercase()),
            ),
            centred,
            Pickable::IGNORE,
        ));
        c.spawn((
            look.text(Face::Display, 15.0, colour, card.name.clone()),
            centred,
            Pickable::IGNORE,
        ));
        c.spawn((
            look.text(Face::Plain, 9.5, SUBTITLE, card.subtitle.clone()),
            centred,
            Pickable::IGNORE,
        ));
        c.spawn((look.divider(), Pickable::IGNORE));
        // the middle: only what it brings
        for line in &card.lines {
            spawn_line(c, look, line, 10.5);
        }
        c.spawn((
            Node {
                flex_grow: 1.0,
                min_height: px(8.0),
                ..default()
            },
            Pickable::IGNORE,
        ));
        footer(c, look, n, card);
    });
}

/// The words under a card's name ("New spell", "Rank 2 » 3"): a muted gold.
const SUBTITLE: Color = Color::srgb(0.70, 0.62, 0.47);

/// Small capitals spaced apart: "R A R E".
fn spaced(word: &str) -> String {
    word.chars()
        .map(String::from)
        .collect::<Vec<_>>()
        .join("\u{200a}")
}

/// The badge at a card's top right: NEW, RING, TRINKET, AMULET filled with the rarity's colour
/// (a light grey for common), dark letters on it; a RANK outlined in that colour instead.
fn badge_box(c: &mut ChildSpawnerCommands, look: &MenuLook, badge: &Badge, rarity: u8) {
    let colour = if rarity == 0 {
        BADGE_COMMON
    } else {
        rarity_colour(rarity)
    };
    let (fill, edge, letters) = if badge.again {
        (palette::PANEL, colour.with_alpha(0.85), colour)
    } else {
        (colour, colour, BADGE_LETTERS)
    };
    c.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: look.px(6.0),
            top: look.px(6.0),
            padding: UiRect::axes(look.px(4.0), look.px(1.0)),
            border: UiRect::all(look.px(BORDER)),
            border_radius: BorderRadius::all(look.px(2.0)),
            ..default()
        },
        BackgroundColor(fill),
        BorderColor::all(edge),
        Pickable::IGNORE,
    ))
    .with_children(|b| {
        b.spawn(look.text(Face::Plain, 8.0, letters, badge.text.clone()));
    });
}

/// A common card's badge: a light grey.
const BADGE_COMMON: Color = Color::srgb(0.86, 0.86, 0.84);
/// Letters on a filled badge.
const BADGE_LETTERS: Color = Color::srgb(0.06, 0.05, 0.04);

/// One line of a card, centred and wrapping, each piece in its tone's colour.
fn spawn_line(c: &mut ChildSpawnerCommands, look: &MenuLook, line: &Line, size: f32) {
    c.spawn((
        Text::default(),
        look.text_font(Face::Plain, size),
        TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        Pickable::IGNORE,
    ))
    .with_children(|t| {
        for (piece, tone) in &line.0 {
            t.spawn((
                TextSpan::new(piece.clone()),
                look.text_font(Face::Plain, size),
                TextColor(tone_colour(*tone)),
            ));
        }
    });
}

/// The card's foot, a strip across it under a thin rule: what taking it does to the hero's
/// slots in small quiet grey ("Takes a spell slot · 4/6"); for an amulet that would replace the
/// worn one, "Replaces" and its name a shade lighter, with its tooltip under the pointer (worn
/// now, lost if the card is taken); a click on it takes the card too.
fn footer(c: &mut ChildSpawnerCommands, look: &MenuLook, n: usize, card: &CardText) {
    let px = |u| look.px(u);
    let mut strip = c.spawn((
        Node {
            align_self: AlignSelf::Stretch,
            justify_content: JustifyContent::Center,
            // across the whole card, past its padding
            margin: UiRect::new(px(-10.0), px(-10.0), px(2.0), px(-4.0)),
            padding: UiRect::new(px(6.0), px(6.0), px(6.0), px(0.0)),
            border: UiRect::top(px(BORDER)),
            ..default()
        },
        BorderColor::all(palette::GOLD_DIM.with_alpha(0.25)),
    ));
    match &card.replaces {
        Some(worn) => {
            strip.insert((
                piece_tip(
                    worn,
                    &[
                        ("Worn now", palette::LABEL),
                        ("Lost if you take this one", palette::RED),
                    ],
                ),
                MenuLookTip,
                CardPick(n),
                crate::stock::StockButton::default(),
                // the card under it stays lit and takes the press as well
                FocusPolicy::Pass,
            ));
            strip.with_children(|s| {
                s.spawn((
                    Text::default(),
                    look.text_font(Face::Plain, 8.5),
                    TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                    Pickable::IGNORE,
                ))
                .with_children(|t| {
                    // in red: taking this card loses the worn amulet
                    t.spawn((
                        TextSpan::new("Replaces "),
                        look.text_font(Face::Plain, 8.5),
                        TextColor(palette::RED),
                    ));
                    t.spawn((
                        TextSpan::new(worn.name.clone()),
                        look.text_font(Face::Plain, 8.5),
                        TextColor(palette::RED),
                    ));
                });
            });
        }
        None => {
            strip.insert(Pickable::IGNORE);
            strip.with_children(|s| {
                s.spawn((
                    look.text(Face::Plain, 8.5, palette::LABEL, card.footer.clone()),
                    TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                    Pickable::IGNORE,
                ));
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_takes_its_rarity_colour_and_brightens_as_it_is_lit_and_taken() {
        let full = rarity_colour(3);
        let (rest, rest_glow, rest_blur) = card_light(3, CardLight::Rest);
        let (lit, lit_glow, lit_blur) = card_light(3, CardLight::Lit);
        let (_, taken_glow, taken_blur) = card_light(3, CardLight::Taken);
        assert_eq!(lit, full);
        assert_eq!(rest, full.with_alpha(0.65));
        // the common white at rest is quieter than a lit card's
        assert!(card_light(0, CardLight::Rest).0.alpha() < 0.5);
        assert!(rest_glow.alpha() < lit_glow.alpha() && lit_glow.alpha() < taken_glow.alpha());
        assert!(rest_blur < lit_blur && lit_blur < taken_blur);
        // every rarity's glow is its own colour
        for r in 0..5 {
            assert_eq!(
                card_light(r, CardLight::Rest).1.with_alpha(1.0),
                rarity_colour(r)
            );
        }
    }

    #[test]
    fn the_panel_fits_its_cards_and_both_columns() {
        assert!(panel_width(3) < panel_width(4));
        assert!(panel_width(0) == panel_width(1));
        // four cards still fit a 4:3 window (1137 units wide)
        assert!(panel_width(4) < 1100.0, "{}", panel_width(4));
    }
}
