//! The edge of where the hero may go, drawn as a band of light standing on the ground all round it
//!: gold in a camp (the camp heroes' glow, `menus::outline::GOLD`), red in a
//! run. The server tells it ([`ToClient::ArenaEdge`]); it stands on the terrain under each step of
//! the ring, brightest at the ground and fading as it rises, and pulses gently.

use benilla_assets::coords::wow_to_bevy;
use benilla_assets::AdtTile;
use benilla_world::particles::buffer::{begin_effect_frame, EffectVertex, WorldEffectDraw};
use benilla_world::sky_order::Rung;
use benilla_world::terrain_stream::{terrain_height_under_cached, TerrainStreamer};
use benilla_world::view::WorldCamera;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use esurv_protocol::ToClient;

use crate::protocol::ServerEvent;

/// How tall the band stands, yards.
const HEIGHT: f32 = 3.0;
/// How long each straight piece of the ring is, yards.
const STEP: f32 = 1.0;
/// The colours, in gamma bytes.
const GOLD: [f32; 3] = [1.0, 0.70, 0.15];
const RED: [f32; 3] = [1.0, 0.10, 0.05];
/// The band's strength at the ground by how far the hero is from that piece of it, yards (in a camp and a run alike): 10% from inside, fading in to 30% at 15, 55% at 10, and its whole 70%,
/// see-through, at 5 and closer; straight between. Where the 10% starts (25) is ours.
const FADE: [(f32, f32); 4] = [(25.0, 0.10), (15.0, 0.30), (10.0, 0.55), (5.0, 0.70)];
/// The pulse: how far the strength dips, and its period in seconds.
const PULSE_DIP: f32 = 0.25;
const PULSE_SECONDS: f32 = 2.5;
/// The band's picture, texels: a bright line at the ground, a glow fading up from it.
const PICTURE: u32 = 64;

pub(crate) fn register(app: &mut App) {
    app.init_resource::<Edge>()
        .add_systems(Update, take_edge)
        .add_systems(PostUpdate, draw_edge.after(begin_effect_frame));
}

/// The edge told, and its picture.
#[derive(Resource, Default)]
struct Edge {
    told: Option<([f32; 3], f32, bool)>,
    picture: Option<Handle<Image>>,
}

fn take_edge(mut events: MessageReader<ServerEvent>, mut edge: ResMut<Edge>, mut images: ResMut<Assets<Image>>) {
    for ServerEvent(m) in events.read() {
        match m {
            ToClient::ArenaEdge { at, radius, camp } => {
                edge.told = Some((*at, *radius, *camp));
                if edge.picture.is_none() {
                    edge.picture = Some(images.add(band_picture()));
                }
            }
            // a fresh greeting in a run or a camp tells its own edge again
            ToClient::Ready(_) | ToClient::Camp { .. } => edge.told = None,
            _ => {}
        }
    }
}

/// The band's picture: white, its alpha 1 at the ground (v = 1) fading to nothing at the top
/// (v = 0), a little brighter in its lowest tenth.
fn band_picture() -> Image {
    let mut rgba = Vec::with_capacity((PICTURE * 4) as usize);
    for j in 0..PICTURE {
        let up = 1.0 - (j as f32 + 0.5) / PICTURE as f32;
        let glow = (1.0 - up).powf(2.2);
        let line = if up < 0.1 { 1.0 - up / 0.1 } else { 0.0 };
        let a = (glow * 0.7 + line * 0.3).clamp(0.0, 1.0);
        rgba.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
    }
    let mut image = Image::new(
        Extent3d { width: 1, height: PICTURE, depth_or_array_layers: 1 },
        TextureDimension::D2,
        rgba,
        // gamma bytes, as the effect lane's art
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..Default::default()
    });
    image
}

/// The band's strength `yards` from the hero ([`FADE`]).
fn strength(yards: f32) -> f32 {
    let (far, faint) = FADE[0];
    if yards >= far {
        return faint;
    }
    for w in FADE.windows(2) {
        let ((d0, a0), (d1, a1)) = (w[0], w[1]);
        if yards >= d1 {
            return a0 + (a1 - a0) * (d0 - yards) / (d0 - d1);
        }
    }
    FADE[FADE.len() - 1].1
}

fn draw_edge(
    edge: Res<Edge>,
    time: Res<Time>,
    objects: benilla_app::Objects,
    places: Query<&GlobalTransform, Without<WorldCamera>>,
    cam: Query<Entity, With<WorldCamera>>,
    streamer: Res<TerrainStreamer>,
    tiles: Res<Assets<AdtTile>>,
    mut draw: WorldEffectDraw,
) {
    let (Some((at, radius, camp)), Some(picture)) = (edge.told, &edge.picture) else {
        return;
    };
    let Ok(cam) = cam.single() else { return };
    let [r, g, b] = if camp { GOLD } else { RED };
    let wave = (time.elapsed_secs() * std::f32::consts::TAU / PULSE_SECONDS).sin() * 0.5 + 0.5;
    let pulse = 1.0 - PULSE_DIP * wave;
    let hero = objects.player().and_then(|e| places.get(e).ok()).map(|t| t.translation());
    let steps = ((std::f32::consts::TAU * radius / STEP).ceil() as usize).max(12);
    let mut cache = None;
    // each step's foot on the ground (the told height where no terrain is streamed)
    let feet: Vec<Vec3> = (0..=steps)
        .map(|i| {
            let a = i as f32 / steps as f32 * std::f32::consts::TAU;
            let (x, y) = (at[0] + radius * a.cos(), at[1] + radius * a.sin());
            let probe = wow_to_bevy([x, y, at[2] + 50.0]);
            let z = terrain_height_under_cached(&streamer, &tiles, probe, &mut cache).unwrap_or(at[2]);
            wow_to_bevy([x, y, z])
        })
        .collect();
    let up = Vec3::Y * HEIGHT;
    // each piece as strong as the hero is near it (along the ground); no hero, the faintest
    let vert = |pos: Vec3, u: f32, v: f32| {
        let near = hero.map_or(f32::MAX, |h| (pos - h).with_y(0.0).length());
        EffectVertex { pos: pos.to_array(), uv: [u, v], color: [r, g, b, strength(near) * pulse] }
    };
    // drawn with the ground marks (without a rung of the sky order it never shows)
    let mut batch = draw.batch(cam, picture.id()).additive().anchored(wow_to_bevy(at)).rung(Rung::RETICLE, 0);
    batch.extend(feet.windows(2).flat_map(|w| {
        let (a, b) = (w[0], w[1]);
        [vert(a, 0.0, 1.0), vert(b, 1.0, 1.0), vert(b + up, 1.0, 0.0), vert(a, 0.0, 1.0), vert(b + up, 1.0, 0.0), vert(a + up, 0.0, 0.0)]
    }));
    batch.tris();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The band is brightest at the ground and nothing at its top.
    #[test]
    fn the_band_fades_as_it_rises() {
        let p = band_picture();
        let data = p.data.unwrap();
        let alpha = |j: usize| data[j * 4 + 3];
        assert!(alpha(PICTURE as usize - 1) > 200, "the ground");
        assert!(alpha(0) < 5, "the top");
        assert!((1..PICTURE as usize).all(|j| alpha(j) >= alpha(j - 1)), "rising to the ground");
    }

    /// The band fades in as the hero nears it: stepwise, straight between.
    #[test]
    fn the_band_fades_in_as_the_hero_nears() {
        let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
        for (yards, alpha) in [(60.0, 0.10), (25.0, 0.10), (20.0, 0.20), (15.0, 0.30), (10.0, 0.55), (5.0, 0.70), (0.0, 0.70)] {
            assert!(close(strength(yards), alpha), "{yards}: {}", strength(yards));
        }
    }
}
