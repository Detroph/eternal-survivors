//! The stock interface's additive textures (`alphaMode="ADD"`): button highlights and the square
//! glow over icons are added to what is under them, which `bevy_ui`'s blending cannot do, so they
//! draw as a [`MaterialNode`] of [`AddImage`].
//!
//! The lane's first sight is in the world, on hover, so the kit draws one invisible pixel of it
//! from the first frames before the world ([`AddImageWarmPixel`]): its one pipeline compiles
//! behind the glue screens, not live under the pointer.

use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor,
};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin};
use bevy::ui_render::ui_material::UiMaterialKey;

use benilla_app::Objects;

use crate::ui::in_world;

/// A texture added to what is under it, in gamma bytes, as the 1.12 client blends `ADD`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct AddImage {
    /// The tint, in gamma bytes, alpha last.
    #[uniform(0)]
    pub tint: Vec4,
    /// The part of the texture drawn: its least and greatest uv.
    #[uniform(1)]
    pub uv: Vec4,
    /// An sRGB texture (the kit's stock art).
    #[texture(2)]
    #[sampler(3)]
    pub texture: Handle<Image>,
}

impl AddImage {
    /// All of `texture`, untinted.
    pub fn new(texture: Handle<Image>) -> Self {
        Self {
            tint: Vec4::ONE,
            uv: Vec4::new(0.0, 0.0, 1.0, 1.0),
            texture,
        }
    }

    /// Only the part of the texture from `min` to `max` (uv).
    pub fn part(mut self, min: Vec2, max: Vec2) -> Self {
        self.uv = Vec4::new(min.x, min.y, max.x, max.y);
        self
    }
}

impl UiMaterial for AddImage {
    fn fragment_shader() -> ShaderRef {
        "embedded://esurv_app/stock/add_image.wgsl".into()
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

/// The one pixel of [`AddImage`] drawn before the world, so the lane's pipeline compiles behind
/// the glue screens (benilla's `pipe_warm` counts every pre-world frame as covered).
///
/// One warm is all the lane needs: a UI material's pipeline key is the view's `hdr` flag and the
/// material's bind-group data. [`AddImage`] has no bind-group data, and every `bevy_ui` node, the
/// glue screens' and the world's alike, draws on benilla's one player-UI camera, which is never
/// `Hdr`. So the pixel drawn here and a glow lit in the world share the one pipeline.
#[derive(Component)]
struct AddImageWarmPixel;

/// A 1-pixel node in the corner, tinted with alpha 0: the blend adds `texel * 0`, nothing, and
/// keeps the target's alpha. Its texture is the always-present default white image, so it draws
/// from the first frames without waiting on the player's data.
fn spawn_warm_pixel(mut commands: Commands, mut materials: ResMut<Assets<AddImage>>) {
    let material = materials.add(AddImage {
        tint: Vec4::ZERO,
        ..AddImage::new(Handle::default())
    });
    commands.spawn((
        Name::new("additive glow warm pixel"),
        AddImageWarmPixel,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(1.0),
            height: Val::Px(1.0),
            ..default()
        },
        MaterialNode(material),
        Pickable::IGNORE,
    ));
}

/// The pixel's work is done once the hero is in the world: the pipeline is in the cache.
fn drop_warm_pixel_in_world(
    mut commands: Commands,
    objects: Objects,
    pixels: Query<Entity, With<AddImageWarmPixel>>,
) {
    if pixels.is_empty() || !in_world(&objects) {
        return;
    }
    for pixel in &pixels {
        commands.entity(pixel).despawn();
    }
}

/// The material, its shader and its warm pixel.
pub(super) fn register(app: &mut App) {
    bevy::asset::embedded_asset!(app, "add_image.wgsl");
    app.add_plugins(UiMaterialPlugin::<AddImage>::default())
        .add_systems(Startup, spawn_warm_pixel)
        .add_systems(Update, drop_warm_pixel_in_world);
}
