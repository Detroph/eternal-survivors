//! The bosses' marks drawn flat on the ground (docs/game/bosses.md,
//! "Building blocks"): Final Fantasy XIV's indicators, see-through orange shapes (circle, ring with
//! its safe middle, cone, line) with a brighter outline, the wind-up's fill sweeping from the
//! origin to the edge as the hit comes, chevrons the way a drifting zone or a sweeping beam goes,
//! and a lasting zone's edge as a thin outline. The server tells each one (`ESURVGI`) and its going
//! (`ESURVGX`). Each is a picture ([`paint`]) projected onto the ground the selection ring is
//! (terrain and building floors, through benilla's decal projector): over the ground, under the
//! units, never fighting the ground's depth, casting no shadow and never picked.

mod indicator_shots;
pub mod paint;

use std::collections::HashMap;

use benilla_assets::coords::wow_to_bevy;
use benilla_world::decal::{DecalFrame, WorldDecal};
use benilla_world::particles::buffer::{begin_effect_frame, EffectVertex, WorldEffectDraw};
use benilla_world::sky_order::Rung;
use benilla_world::view::WorldCamera;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use esurv_protocol::{GroundIndicator, IndicatorShape, ToClient};

use crate::protocol::ServerEvent;
use paint::{Chevron, Layout};

/// The wind-up's fill is painted again in this many steps.
const SWEEP_STEPS: f32 = 40.0;
/// The ground the picture lands on lies within this far (yards) below or above the indicator's
/// point, more for a big one, and fades out over the last [`SLAB_FADE`] of it.
const SLAB_LEAST: f32 = 3.0;
const SLAB_SHARE: f32 = 0.15;
const SLAB_MOST: f32 = 8.0;
const SLAB_FADE: f32 = 1.5;
/// A face steeper than this (its normal's upward share) is a wall: the picture is not smeared on it.
const LEAST_UPWARD: f32 = 0.5;
/// The picture is projected again once its indicator moved or turned this far.
const MOVED: f32 = 0.02;
const TURNED: f32 = 0.002;
/// A mark not told gone this long (seconds) after it was due is dropped.
const OVERDUE: f32 = 3.0;

pub(crate) fn register(app: &mut App) {
    app.init_resource::<Indicators>()
        .add_systems(Update, (take_indicators, place_indicators).chain())
        .add_systems(PostUpdate, draw_indicators.after(begin_effect_frame));
    indicator_shots::register(app);
}

/// One indicator as drawn.
struct Shown {
    told: GroundIndicator,
    /// When it was told, on the app clock.
    since: f32,
    layout: Layout,
    picture: Handle<Image>,
    /// What the picture was painted for: its sweep step and its chevrons' way.
    painted: Option<(i32, Option<(i32, i32)>)>,
    /// The projection, and where, which way and on how many surfaces it was made.
    verts: Vec<EffectVertex>,
    projected: Option<(Vec3, f32, usize)>,
    centre: Vec3,
}

/// The indicators on the ground, by the server's number.
#[derive(Resource, Default)]
struct Indicators(HashMap<u32, Shown>);

/// An indicator `elapsed` seconds after it was told: where it stands, which way it faces (a
/// moving circle faces the way it goes), how far its wind-up has swept (`None`: an outline), and
/// the way its chevrons point in its own frame (`None`: it does not move).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Now {
    pub at: [f32; 3],
    pub facing: f32,
    pub swept: Option<f32>,
    pub towards: Option<(f32, f32)>,
}

/// Where `g` stands `elapsed` seconds after it was told.
pub fn indicator_now(g: &GroundIndicator, elapsed: f32) -> Now {
    let t = if g.still { 0.0 } else { elapsed.max(0.0) };
    let at = [g.at[0] + g.velocity[0] * t, g.at[1] + g.velocity[1] * t, g.at[2]];
    let speed = (g.velocity[0].powi(2) + g.velocity[1].powi(2)).sqrt();
    let round = matches!(g.shape, IndicatorShape::Circle { .. } | IndicatorShape::Ring { .. });
    let (facing, towards) = if g.turn.abs() > 1e-4 {
        // a beam: its chevrons point across it, the way it turns (leftwards turning up)
        (g.facing + g.turn * t, Some((0.0, g.turn.signum())))
    } else if speed > 0.05 && round {
        (g.velocity[1].atan2(g.velocity[0]), Some((1.0, 0.0)))
    } else if speed > 0.05 {
        let way = g.velocity[1].atan2(g.velocity[0]) - g.facing;
        (g.facing, Some((way.cos(), way.sin())))
    } else {
        (g.facing, None)
    };
    let swept = (!g.edge_only).then(|| if g.wind_up > 0.0 { (1.0 - (g.lands_in - t) / g.wind_up).clamp(0.0, 1.0) } else { 1.0 });
    Now { at, facing, swept, towards }
}

/// The decal's frame for an indicator at `centre` (Bevy) facing `facing` (WoW radians): its
/// rotated `x` runs along the facing and its `z` across it rightwards, so the picture's `u` runs
/// along and its `v` rightwards ([`Layout::uv`]).
pub fn decal_frame(layout: &Layout, centre: Vec3, facing: f32, slab: f32) -> DecalFrame {
    DecalFrame {
        center: centre,
        sin: facing.cos(),
        cos: -facing.sin(),
        min_x: layout.along.0,
        max_x: layout.along.1,
        min_z: -layout.across.1,
        max_z: -layout.across.0,
        min_y: -slab,
        max_y: slab,
    }
}

fn slab(shape: IndicatorShape) -> f32 {
    (SLAB_LEAST + paint::reach(shape) * SLAB_SHARE).min(SLAB_MOST)
}

/// `g`'s picture projected onto the ground at `now`: triangles over the surfaces under it, walls
/// left out, fading near the top and bottom of its slab.
pub fn project(decals: &WorldDecal, layout: &Layout, shape: IndicatorShape, centre: Vec3, facing: f32) -> Vec<EffectVertex> {
    let slab = slab(shape);
    let frame = decal_frame(layout, centre, facing, slab);
    let mut verts = Vec::new();
    decals.project(&mut verts, &frame, |p| ((slab - p.y.abs()) / SLAB_FADE).clamp(0.0, 1.0), |x, z| frame.rect_uv(x, z));
    let mut kept = Vec::with_capacity(verts.len());
    for tri in verts.chunks_exact(3) {
        let [a, b, c] = [Vec3::from(tri[0].pos), Vec3::from(tri[1].pos), Vec3::from(tri[2].pos)];
        let n = (b - a).cross(c - a).normalize_or_zero();
        if n.y.abs() >= LEAST_UPWARD {
            kept.extend_from_slice(tri);
        }
    }
    kept
}

fn picture(layout: &Layout, rgba: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d { width: layout.width, height: layout.height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        rgba,
        // gamma bytes, as the effect lane's art: its shader converts after the filter
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

/// The server's indicators: a new one, one told again (moved, turned, held), one gone; all go
/// when the server greets the client anew (a new run, a new world), and it tells the standing
/// ones again.
fn take_indicators(mut events: MessageReader<ServerEvent>, mut shown: ResMut<Indicators>, mut images: ResMut<Assets<Image>>, time: Res<Time>) {
    let now = time.elapsed_secs();
    for ServerEvent(m) in events.read() {
        match m {
            ToClient::GroundIndicator(g) => {
                let layout = Layout::of(g.shape);
                match shown.0.get_mut(&g.id) {
                    Some(s) if s.layout == layout => {
                        s.told = g.clone();
                        s.since = now;
                        s.painted = None;
                    }
                    _ => {
                        if let Some(old) = shown.0.remove(&g.id) {
                            images.remove(&old.picture);
                        }
                        let picture = images.add(picture(&layout, vec![0; (layout.width * layout.height * 4) as usize]));
                        shown.0.insert(g.id, Shown { told: g.clone(), since: now, layout, picture, painted: None, verts: Vec::new(), projected: None, centre: Vec3::ZERO });
                    }
                }
            }
            ToClient::GroundIndicatorGone { id } => {
                if let Some(old) = shown.0.remove(id) {
                    images.remove(&old.picture);
                }
            }
            ToClient::Ready(_) => {
                for (_, old) in shown.0.drain() {
                    images.remove(&old.picture);
                }
            }
            _ => {}
        }
    }
}

/// Each indicator where it stands now: its picture painted again as its wind-up sweeps or its
/// way turns, projected again as it moves or turns or the ground under it streams in.
fn place_indicators(mut shown: ResMut<Indicators>, mut images: ResMut<Assets<Image>>, decals: WorldDecal, time: Res<Time>) {
    let now = time.elapsed_secs();
    let surfaces = decals.receiver_count();
    shown.0.retain(|_, s| {
        let overdue = !s.told.edge_only && s.told.wind_up > 0.0 && !s.told.still && now - s.since > s.told.lands_in + OVERDUE;
        if overdue {
            images.remove(&s.picture);
        }
        !overdue
    });
    for s in shown.0.values_mut() {
        let at = indicator_now(&s.told, now - s.since);
        let step = at.swept.map_or(-1, |p| (p * SWEEP_STEPS).floor() as i32);
        let way = at.towards.map(|(x, y)| ((x * 100.0).round() as i32, (y * 100.0).round() as i32));
        if s.painted != Some((step, way)) {
            let swept = at.swept.map(|_| step as f32 / SWEEP_STEPS);
            let chevrons: Vec<Chevron> = at.towards.map(|t| paint::chevrons(s.told.shape, t)).unwrap_or_default();
            if let Some(image) = images.get_mut(&s.picture) {
                image.data = Some(paint::paint(s.told.shape, &s.layout, swept, &chevrons));
                s.painted = Some((step, way));
            }
        }
        let centre = wow_to_bevy(at.at);
        s.centre = centre;
        let stale = s.projected.is_none_or(|(c, f, n)| c.distance(centre) > MOVED || (f - at.facing).abs() > TURNED || n != surfaces);
        if stale {
            s.verts = project(&decals, &s.layout, s.told.shape, centre, at.facing);
            s.projected = Some((centre, at.facing, surfaces));
        }
    }
}

/// Every indicator onto the effect stream, in the ground decals' band (after the reticle's rung),
/// see-through.
fn draw_indicators(shown: Res<Indicators>, cam: Query<Entity, With<WorldCamera>>, mut draw: WorldEffectDraw) {
    let Ok(cam) = cam.single() else { return };
    for s in shown.0.values().filter(|s| !s.verts.is_empty()) {
        let mut batch = draw.batch(cam, s.picture.id()).alpha().anchored(s.centre).rung(Rung::RETICLE, Rung::DECAL_RASTER);
        batch.vertices(&s.verts);
        batch.tris();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::prelude::Collider;
    use benilla_assets::coords::bevy_to_wow;
    use benilla_world::collision::GroundDecalSurface;
    use bevy::ecs::system::RunSystemOnce;
    use esurv_content::Shape;

    fn told(shape: IndicatorShape, at: [f32; 3], facing: f32) -> GroundIndicator {
        GroundIndicator { id: 1, shape, at, facing, wind_up: 2.0, lands_in: 2.0, velocity: [0.0, 0.0], turn: 0.0, edge_only: false, still: false }
    }

    /// The whole way from the server's mark to the ground: each shape projected onto a flat
    /// ground at its point and facing covers, at every spot of the ground its picture lands on,
    /// exactly what the simulation's hit test takes (away from the edge).
    #[test]
    fn the_picture_on_the_ground_is_the_hit_test() {
        let cases = [
            (IndicatorShape::Circle { radius: 8.0 }, Shape::Circle { radius: 8.0 }),
            (IndicatorShape::Ring { inner: 6.0, outer: 15.0 }, Shape::Ring { inner: 6.0, outer: 15.0 }),
            (IndicatorShape::Cone { length: 20.0, angle: 60.0 }, Shape::Cone { length: 20.0, angle: 60.0 }),
            (IndicatorShape::Line { length: 40.0, width: 6.0 }, Shape::Line { length: 40.0, width: 6.0 }),
        ];
        let origin = [-120.0f32, 75.0, 12.0];
        for (drawn, hit) in cases {
            for facing in [0.0f32, 0.9, -2.3] {
                let mut world = World::new();
                // a flat ground at the mark's height, 200 yd across (Bevy space)
                let c = wow_to_bevy(origin);
                let q = 100.0;
                world.spawn((
                    GroundDecalSurface,
                    Collider::trimesh(
                        vec![c + Vec3::new(-q, 0.0, -q), c + Vec3::new(q, 0.0, -q), c + Vec3::new(q, 0.0, q), c + Vec3::new(-q, 0.0, q)],
                        vec![[0, 1, 2], [0, 2, 3]],
                    ),
                ));
                let layout = Layout::of(drawn);
                let g = told(drawn, origin, facing);
                let verts = world
                    .run_system_once(move |decals: WorldDecal| {
                        let now = indicator_now(&g, 0.0);
                        project(&decals, &layout, g.shape, wow_to_bevy(now.at), now.facing)
                    })
                    .unwrap();
                assert!(!verts.is_empty(), "{drawn:?} lands on the ground");
                let px = paint::paint(drawn, &layout, Some(0.0), &[]);
                let mut checked = 0;
                for tri in verts.chunks_exact(3) {
                    // spots spread over each triangle: position and picture coordinate together
                    let spots = (1..30).flat_map(|a| (1..30 - a).map(move |b| (a as f32 / 30.0, b as f32 / 30.0)));
                    for (wa, wb) in spots {
                        let wc = 1.0 - wa - wb;
                        let pos = Vec3::from(tri[0].pos) * wa + Vec3::from(tri[1].pos) * wb + Vec3::from(tri[2].pos) * wc;
                        let u = tri[0].uv[0] * wa + tri[1].uv[0] * wb + tri[2].uv[0] * wc;
                        let v = tri[0].uv[1] * wa + tri[1].uv[1] * wb + tri[2].uv[1] * wc;
                        let w = bevy_to_wow(pos);
                        let p = (w[0], w[1]);
                        let along = (p.0 - origin[0]) * facing.cos() + (p.1 - origin[1]) * facing.sin();
                        let across = -(p.0 - origin[0]) * facing.sin() + (p.1 - origin[1]) * facing.cos();
                        if paint::edge_distance(drawn, along, across).abs() < 3.0 * layout.texel() {
                            continue;
                        }
                        let i = ((u * layout.width as f32) as u32).min(layout.width - 1);
                        let j = ((v * layout.height as f32) as u32).min(layout.height - 1);
                        let alpha = px[((j * layout.width + i) * 4 + 3) as usize];
                        assert_eq!(alpha > 0, hit.contains((origin[0], origin[1]), facing, p), "{drawn:?} facing {facing} at {p:?} (uv {u},{v})");
                        checked += 1;
                    }
                }
                assert!(checked > 300, "{drawn:?}: {checked}");
            }
        }
    }

    /// The wind-up sweeps from nothing to whole as the hit comes, and holds while the run stands;
    /// a moving zone goes its way and faces it; a beam turns and its chevrons point the way it
    /// turns; a lasting zone has no sweep.
    #[test]
    fn an_indicator_sweeps_moves_and_turns_with_time() {
        let mut g = told(IndicatorShape::Circle { radius: 5.0 }, [0.0, 0.0, 0.0], 0.0);
        g.lands_in = 1.5;
        assert_eq!(indicator_now(&g, 0.0).swept, Some(0.25));
        assert_eq!(indicator_now(&g, 1.5).swept, Some(1.0));
        assert_eq!(indicator_now(&g, 9.0).swept, Some(1.0));
        g.still = true;
        assert_eq!(indicator_now(&g, 1.0).swept, Some(0.25), "held");
        let mut zone = told(IndicatorShape::Circle { radius: 5.0 }, [10.0, 0.0, 3.0], 0.0);
        zone.edge_only = true;
        zone.velocity = [0.0, -2.0];
        let n = indicator_now(&zone, 1.0);
        assert_eq!((n.at, n.swept, n.towards), ([10.0, -2.0, 3.0], None, Some((1.0, 0.0))));
        assert!((n.facing + std::f32::consts::FRAC_PI_2).abs() < 1e-5, "faces the way it drifts");
        let mut beam = told(IndicatorShape::Line { length: 30.0, width: 4.0 }, [0.0, 0.0, 0.0], 1.0);
        beam.edge_only = true;
        beam.turn = -0.5;
        let n = indicator_now(&beam, 2.0);
        assert!((n.facing - 0.0).abs() < 1e-5);
        assert_eq!(n.towards, Some((0.0, -1.0)), "turning rightwards, its chevrons point right");
    }

    /// The server's lines put indicators down, tell them again in place and take them away; a
    /// fresh greeting clears them.
    #[test]
    fn indicators_come_and_go_with_the_servers_lines() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).init_resource::<Assets<Image>>().add_message::<ServerEvent>().init_resource::<Indicators>().add_systems(Update, take_indicators);
        let g = told(IndicatorShape::Cone { length: 12.0, angle: 90.0 }, [1.0, 2.0, 3.0], 0.5);
        app.world_mut().write_message(ServerEvent(ToClient::GroundIndicator(g.clone())));
        app.world_mut().write_message(ServerEvent(ToClient::GroundIndicator(GroundIndicator { id: 2, ..g.clone() })));
        app.update();
        assert_eq!(app.world().resource::<Indicators>().0.len(), 2);
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 2, "a picture each");
        app.world_mut().write_message(ServerEvent(ToClient::GroundIndicator(GroundIndicator { facing: 0.7, ..g.clone() })));
        app.world_mut().write_message(ServerEvent(ToClient::GroundIndicatorGone { id: 2 }));
        app.update();
        let shown = &app.world().resource::<Indicators>().0;
        assert_eq!((shown.len(), shown[&1].told.facing), (1, 0.7));
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 1, "the gone one's picture freed");
        app.world_mut().write_message(ServerEvent(ToClient::Ready(esurv_protocol::Ready { hero: 1, name: String::new(), class: String::new(), race: String::new(), zone: String::new(), subtitle: String::new() })));
        app.update();
        assert!(app.world().resource::<Indicators>().0.is_empty());
    }
}
