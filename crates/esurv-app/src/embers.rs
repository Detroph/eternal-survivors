//! Burning Embers over the hero's head (screens.md, "The power bar"):
//! four embers float above the hero while its bar is Burning Embers, each lit as the counter
//! fills it (a half-filled one glows half), as the world camera sees the hero.

use bevy::prelude::*;

use benilla_app::Objects;
use esurv_content::ResourceKind;

use crate::run_frames::class_bar::ClassBar;

/// How many embers there are.
pub const EMBERS: usize = 4;
/// The ember's texture in the player's client.
const EMBER_ART: &str = r"Creature\FireElemental\Ember";
/// An ember's size and the gap between two, in pixels.
const EMBER_SIZE: f32 = 22.0;
const EMBER_GAP: f32 = 4.0;
/// How far over the hero's feet the embers float, in yards.
const OVER_HEAD: f32 = 2.6;
/// An ember's glow: lit, half lit, out.
const LIT: f32 = 1.0;
const HALF: f32 = 0.55;
const OUT: f32 = 0.2;
/// The least an ember shows, so an ember out still reads as one.
const OUT_ALPHA: f32 = 0.45;

/// The row of embers.
#[derive(Component)]
struct EmberRow;

/// One ember, its place in the row (0 first).
#[derive(Component)]
struct Ember(usize);

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, (spawn_the_row, place_the_embers).chain());
}

/// How lit each ember is for a counter at `value` of `max` (the four share it evenly).
pub fn ember_glow(value: f32, max: f32) -> [f32; EMBERS] {
    let per = if max > 0.0 { max / EMBERS as f32 } else { 1.0 };
    std::array::from_fn(|i| {
        let fill = (value / per - i as f32).clamp(0.0, 1.0);
        if fill >= 0.999 {
            LIT
        } else if fill >= 0.5 {
            HALF
        } else {
            OUT
        }
    })
}

fn spawn_the_row(mut commands: Commands, rows: Query<(), With<EmberRow>>, assets: Res<AssetServer>) {
    if !rows.is_empty() {
        return;
    }
    let art = crate::stock::load_stock_art(&assets, EMBER_ART);
    commands
        .spawn((
            EmberRow,
            Node {
                position_type: PositionType::Absolute,
                column_gap: Val::Px(EMBER_GAP),
                ..default()
            },
            GlobalZIndex(crate::ui::layer::HUD),
            Pickable::IGNORE,
            Visibility::Hidden,
        ))
        .with_children(|row| {
            for i in 0..EMBERS {
                row.spawn((
                    Ember(i),
                    ImageNode::new(art.clone()),
                    Node {
                        width: Val::Px(EMBER_SIZE),
                        height: Val::Px(EMBER_SIZE),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
            }
        });
}

/// Keeps the row over the hero's head, lit as the counter stands, while the bar is Burning
/// Embers; hidden otherwise.
fn place_the_embers(
    bar: Res<ClassBar>,
    objects: Objects,
    places: Query<&GlobalTransform>,
    cams: Query<(&Camera, &GlobalTransform), With<benilla_world::view::WorldCamera>>,
    mut rows: Query<(&mut Node, &mut Visibility), With<EmberRow>>,
    mut embers: Query<(&Ember, &mut ImageNode)>,
    mut said: Local<String>,
) {
    let Ok((mut node, mut vis)) = rows.single_mut() else {
        return;
    };
    let counter = bar.bar.as_ref().filter(|b| b.0 == ResourceKind::BurningEmbers).map(|b| (b.2, b.3));
    let at = objects.player().and_then(|e| places.get(e).ok()).map(|t| t.translation() + Vec3::Y * OVER_HEAD);
    // the world camera that draws (benilla keeps more than one)
    let cam = cams.iter().find(|(c, _)| c.is_active);
    let shown = match (counter, at, cam) {
        (Some(c), Some(at), Some((cam, cam_at))) => cam.world_to_viewport(cam_at, at).ok().map(|p| (c, p)),
        _ => None,
    };
    let why = format!("counter {} hero {} camera {} shown {}", counter.is_some(), at.is_some(), cam.is_some(), shown.is_some());
    if *said != why {
        debug!("embers: {why}");
        *said = why;
    }
    let Some(((value, max), p)) = shown else {
        vis.set_if_neq(Visibility::Hidden);
        return;
    };
    let width = EMBERS as f32 * EMBER_SIZE + (EMBERS - 1) as f32 * EMBER_GAP;
    node.left = Val::Px(p.x - width / 2.0);
    node.top = Val::Px(p.y - EMBER_SIZE);
    vis.set_if_neq(Visibility::Inherited);
    let glow = ember_glow(value, max);
    for (ember, mut image) in &mut embers {
        let a = glow[ember.0];
        if (image.color.alpha() - a).abs() > f32::EPSILON {
            // an ember out is dark as well as faint
            image.color = Color::srgba(a, a, a, a.max(OUT_ALPHA));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embers_fill_in_halves() {
        assert_eq!(ember_glow(0.0, 4.0), [OUT; 4]);
        assert_eq!(ember_glow(1.5, 4.0), [LIT, HALF, OUT, OUT]);
        assert_eq!(ember_glow(4.0, 4.0), [LIT; 4]);
        // a counter kept in halves (8 of them) reads the same
        assert_eq!(ember_glow(3.0, 8.0), [LIT, HALF, OUT, OUT]);
    }
}
