//! The heroes' golden glow in the camp: every other hero there, at their tents,
//! wears the warband screen's picked-hero outline (`menus::outline`), the same gold and pulse;
//! the hero played and the camp's NPCs (the trainer, the merchant) wear none. benilla draws each
//! hero alone into mask [`CAMP_HERO_MASK`]; a node over the window draws the gold round it.
//! A hero is told by its player's guid: the other heroes come as players too, the NPCs as
//! creatures.

use benilla_app::{FigureMaskImage, Objects, SceneFigureMask};
use bevy::prelude::*;

use super::CampVisit;
use crate::menus::outline::{OutlineGlow, EDGE, GLOW, GOLD, PULSE};

/// The camp heroes' mask (beside the picked hero's and the enemies' glows).
pub const CAMP_HERO_MASK: usize = 4;

/// The gold node, once made.
#[derive(Resource)]
struct HeroGlowNode {
    node: Entity,
    material: Handle<OutlineGlow>,
}

pub(super) fn register(app: &mut App) {
    app.add_systems(Update, (mask_the_heroes, draw_the_glow).chain());
}

/// In the camp every hero but the one played carries the mask; out of it, none.
fn mask_the_heroes(
    mut commands: Commands,
    visit: Res<CampVisit>,
    objects: Objects,
    masked: Query<(Entity, &SceneFigureMask)>,
) {
    let want: Vec<Entity> = if visit.in_camp {
        let me = objects.player();
        objects
            .iter()
            .filter(|(guid, e, _)| benilla_protocol::guid::is_player(*guid) && Some(*e) != me)
            .map(|(_, e, _)| e)
            .collect()
    } else {
        Vec::new()
    };
    for (e, mask) in &masked {
        if mask.0 == CAMP_HERO_MASK && !want.contains(&e) {
            commands.entity(e).remove::<SceneFigureMask>();
        }
    }
    for e in want {
        if masked.get(e).map(|(_, m)| m.0) != Ok(CAMP_HERO_MASK) {
            commands.entity(e).insert(SceneFigureMask(CAMP_HERO_MASK));
        }
    }
}

/// The node over the window that draws the gold, shown in the camp, its pulse kept moving.
fn draw_the_glow(
    mut commands: Commands,
    visit: Res<CampVisit>,
    image: Res<FigureMaskImage>,
    time: Res<Time>,
    mut materials: ResMut<Assets<OutlineGlow>>,
    node: Option<Res<HeroGlowNode>>,
    mut visibility: Query<&mut Visibility>,
) {
    let Some(mask) = image.0[CAMP_HERO_MASK].clone() else {
        return;
    };
    let Some(node) = node else {
        let material = materials.add(OutlineGlow { colour: GOLD, params: Vec4::new(EDGE, GLOW, PULSE, 0.0), mask });
        let e = commands
            .spawn((
                Name::new("camp heroes' glow"),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                MaterialNode(material.clone()),
                // over the world, under the windows, as the enemies' glows
                GlobalZIndex(crate::ui::layer::METER - 10),
                Pickable::IGNORE,
                Visibility::Hidden,
            ))
            .id();
        commands.insert_resource(HeroGlowNode { node: e, material });
        return;
    };
    let want = if visit.in_camp { Visibility::Inherited } else { Visibility::Hidden };
    if let Ok(mut v) = visibility.get_mut(node.node) {
        v.set_if_neq(want);
    }
    if visit.in_camp {
        if let Some(m) = materials.get_mut(&node.material) {
            m.params.w = time.elapsed_secs();
            if m.mask != mask {
                m.mask = mask;
            }
        }
    }
}
