//! The kit's look check: with `ESURV_UI_DEMO=1` a sample dialog shows while the hero is in
//! the world, with every piece of the kit: the dialog backdrop and header, text in the stock
//! fonts, icons in their frames, a tooltip box, a status bar and panel buttons (one disabled).

use bevy::prelude::*;
use bevy::text::LineBreak;

use benilla_app::{Objects, OverlayWindow};

use super::{
    Backdrop, StatusBar, Stock, StockButton, StockClick, StockFont, StockScale, StockSystems,
    HEADER_WIDTH, ICON_SIZE,
};
use crate::ui::{in_world, layer, rarity_colour};

/// The switch.
const DEMO_ENV: &str = "ESURV_UI_DEMO";

/// Whether this run wants the sample dialog.
fn wanted() -> bool {
    std::env::var(DEMO_ENV).is_ok_and(|v| v == "1")
}

/// The sample dialog's layer, and the scale it was built at.
#[derive(Resource, Default)]
struct DemoDialog(Option<(Entity, StockScale)>);

/// The bar that fills and empties.
#[derive(Component)]
struct DemoBar;

pub(super) fn register(app: &mut App) {
    if !wanted() {
        return;
    }
    info!("stock: the sample dialog is on ({DEMO_ENV})");
    app.init_resource::<DemoDialog>().add_systems(
        Update,
        (show_demo, move_demo_bar, log_demo_clicks).before(StockSystems),
    );
}

/// Builds the dialog while the hero is in the world, again when the scale changes.
fn show_demo(
    mut commands: Commands,
    objects: Objects,
    stock: Stock,
    mut shown: ResMut<DemoDialog>,
) {
    let want = in_world(&objects);
    let scale = stock.scale();
    match shown.0 {
        Some((layer, at)) if !want || at != scale => {
            commands.entity(layer).despawn();
            shown.0 = None;
        }
        _ => {}
    }
    if want && shown.0.is_none() {
        shown.0 = Some((spawn_demo(&mut commands, &stock), scale));
    }
}

fn spawn_demo(commands: &mut Commands, stock: &Stock) -> Entity {
    let px = |u| stock.px(u);
    let line = |font: StockFont, text: &'static str| {
        (
            stock.text(font, text),
            TextLayout::new(Justify::Left, LineBreak::WordBoundary),
        )
    };
    commands
        // at the left, clear of a card screen in the middle
        .spawn((
            Name::new("stock look check"),
            Node {
                position_type: PositionType::Absolute,
                left: px(24.0),
                top: px(110.0),
                ..default()
            },
            GlobalZIndex(layer::CARDS),
        ))
        .with_children(|layer| {
            layer
                .spawn((
                    OverlayWindow,
                    Node {
                        width: px(380.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::new(px(24.0), px(24.0), px(30.0), px(22.0)),
                        row_gap: px(8.0),
                        ..default()
                    },
                    stock.backdrop(Backdrop::Dialog),
                ))
                .with_children(|dialog| {
                    dialog.spawn(stock.dialog_header("Stock Look", HEADER_WIDTH));
                    dialog.spawn(line(
                        StockFont::GameFontHighlight,
                        "Every screen of the game but the meter is built from these pieces.",
                    ));
                    dialog.spawn(line(StockFont::GameFontNormalLarge, "Normal large"));
                    dialog.spawn(line(StockFont::GameFontNormal, "Normal"));
                    dialog.spawn(line(StockFont::GameFontNormalSmall, "Normal small"));
                    dialog.spawn(line(StockFont::GameFontDisable, "Disabled"));
                    dialog.spawn(line(StockFont::NumberFontNormal, "1234567890"));
                    dialog
                        .spawn(Node {
                            column_gap: px(14.0),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|row| {
                            for icon in ["Spell_Fire_Fireball", "INV_Jewelry_Ring_15"] {
                                row.spawn((StockButton::default(), stock.icon(icon, ICON_SIZE)));
                            }
                            row.spawn((
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    padding: UiRect::all(px(10.0)),
                                    row_gap: px(2.0),
                                    ..default()
                                },
                                stock.backdrop(Backdrop::Tooltip),
                            ))
                            .with_children(|tip| {
                                tip.spawn(stock.coloured_text(
                                    StockFont::TooltipHeader,
                                    rarity_colour(3),
                                    "Band of Accuria",
                                ));
                                tip.spawn(stock.text(StockFont::TooltipText, "+17% damage"));
                            });
                        });
                    dialog.spawn((
                        DemoBar,
                        stock.status_bar(Color::srgb(0.0, 1.0, 0.0), 0.6, 200.0, 12.0),
                    ));
                    dialog
                        .spawn(Node {
                            column_gap: px(6.0),
                            margin: UiRect::top(px(4.0)),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn(stock.panel_button("Accept", 100.0));
                            row.spawn(stock.panel_button("Cancel", 100.0));
                            row.spawn(stock.panel_button("Disabled", 100.0))
                                .insert(StockButton { enabled: false });
                        });
                });
        })
        .id()
}

/// The bar fills and empties, so its fill can be seen moving.
fn move_demo_bar(time: Res<Time>, mut bars: Query<&mut StatusBar, With<DemoBar>>) {
    for mut bar in &mut bars {
        bar.value = 0.5 + 0.5 * (time.elapsed_secs() * 0.7).sin();
    }
}

fn log_demo_clicks(mut clicks: MessageReader<StockClick>, names: Query<&Children>) {
    for click in clicks.read() {
        info!(
            "stock: sample dialog click on {:?} ({} parts)",
            click.0,
            names.get(click.0).map_or(0, |c| c.len())
        );
    }
}
