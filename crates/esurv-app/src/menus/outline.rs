//! The picked hero's golden outline in the camp (menus.md, "The flow",
//! step 4): a thin gold edge along the hero's silhouette, its gear and weapons included, with a
//! soft glow fading outward and a slow, gentle pulse. benilla draws the picked hero alone into a
//! window-sized mask (`benilla_app::SceneFigureMask`, [`benilla_app::FigureMaskImage`]); a node of
//! [`OutlineGlow`] over the window reads round each pixel outside the silhouette and adds the gold
//! (`outline_glow.wgsl`). Only the picked hero has it, and it follows the stand-up and the
//! greeting, as the mask is drawn every frame. The hero has mask [`HERO_MASK`] to itself; the
//! enemies' glows in a run (`crate::enemy_glow`) draw the same way, from the other masks, in
//! their own colours.
//!
//! The mask holds the hero alone, so the edge also shows where something stands in front of the
//! hero's edge; the camp's places keep the picked hero clear of the others.

use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor,
};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin};
use bevy::ui_render::ui_material::UiMaterialKey;

use benilla_app::{FigureMaskImage, SceneFigureMask, SelectScreen};

use super::{Menus, Screen};
use crate::picker::PickerState;

/// The edge's width and the glow's reach, in pixels at 1080 rows; how deep the pulse dips.
pub(crate) const EDGE: f32 = 1.5;
pub(crate) const GLOW: f32 = 7.0;
pub(crate) const PULSE: f32 = 0.25;
/// The picked hero's mask (`SceneFigureMask`).
pub(crate) const HERO_MASK: usize = 0;
/// The gold, in gamma bytes; its alpha is the glow's strength at the edge.
const GOLD: Vec4 = Vec4::new(1.0, 0.80, 0.38, 0.55);

/// The outline's material: the mask, the colour and the sizes.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct OutlineGlow {
    /// In gamma bytes; its alpha is the glow's strength at the edge.
    #[uniform(0)]
    pub colour: Vec4,
    /// Edge width, glow reach, pulse depth, time.
    #[uniform(1)]
    pub params: Vec4,
    #[texture(2)]
    pub mask: Handle<Image>,
}

impl UiMaterial for OutlineGlow {
    fn fragment_shader() -> ShaderRef {
        "embedded://esurv_app/menus/outline_glow.wgsl".into()
    }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {
        let add = BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::Zero,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(add);
            }
        }
    }
}

/// The outline's node, once made.
#[derive(Resource)]
struct OutlineNode {
    node: Entity,
    material: Handle<OutlineGlow>,
}

pub(super) fn register(app: &mut App) {
    bevy::asset::embedded_asset!(app, "outline_glow.wgsl");
    app.add_plugins(UiMaterialPlugin::<OutlineGlow>::default())
        .add_systems(Update, (mask_the_picked_hero, draw_the_outline).chain());
}

/// Whether the outline shows: the warband screen up with nothing over it.
fn shows(menus: &Menus, screen: &SelectScreen) -> bool {
    screen.open
        && screen.scene_ready
        && menus.screen == Screen::Warband
        && menus.confirm_delete.is_none()
        && !menus.run_options_open
        && menus.codex.is_none()
        && menus.guide.is_none()
        && menus.game_options.is_none()
}

/// The picked hero's figure carries the mask, and no other.
fn mask_the_picked_hero(
    mut commands: Commands,
    menus: Res<Menus>,
    screen: Res<SelectScreen>,
    camp: Res<PickerState>,
    masked: Query<(Entity, &SceneFigureMask)>,
) {
    let want = if shows(&menus, &screen) {
        screen.selected.and_then(|g| camp.figure_of(g))
    } else {
        None
    };
    for (e, mask) in &masked {
        if mask.0 == HERO_MASK && Some(e) != want {
            commands.entity(e).remove::<SceneFigureMask>();
        }
    }
    if let Some(e) = want {
        if masked.get(e).map(|(_, m)| m.0) != Ok(HERO_MASK) {
            commands.entity(e).insert(SceneFigureMask(HERO_MASK));
        }
    }
}

/// The node over the window that draws the gold from the mask, its pulse kept moving.
fn draw_the_outline(
    mut commands: Commands,
    menus: Res<Menus>,
    screen: Res<SelectScreen>,
    image: Res<FigureMaskImage>,
    time: Res<Time>,
    mut materials: ResMut<Assets<OutlineGlow>>,
    node: Option<Res<OutlineNode>>,
    mut visibility: Query<&mut Visibility>,
) {
    let Some(mask) = image.0[HERO_MASK].clone() else {
        return;
    };
    let Some(node) = node else {
        let material = materials.add(OutlineGlow {
            colour: GOLD,
            params: Vec4::new(EDGE, GLOW, PULSE, 0.0),
            mask,
        });
        let e = commands
            .spawn((
                Name::new("picked hero outline"),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                MaterialNode(material.clone()),
                // over the world, under the menus' panels
                GlobalZIndex(super::MENU_LAYER - 1),
                Pickable::IGNORE,
                Visibility::Hidden,
            ))
            .id();
        commands.insert_resource(OutlineNode { node: e, material });
        return;
    };
    let want = if shows(&menus, &screen) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if let Ok(mut v) = visibility.get_mut(node.node) {
        v.set_if_neq(want);
    }
    if want == Visibility::Inherited {
        if let Some(m) = materials.get_mut(&node.material) {
            m.params.w = time.elapsed_secs();
            if m.mask != mask {
                m.mask = mask;
            }
        }
    }
}
