//! The camp's own things cast shadows on the ground. 1.12 gives a game object no shadow of its own: the ground's
//! are painted into the map (MCSH) round the map's own models, and a unit casts only its blob. In
//! the camp every tent, every hub's piece and every thing to use casts one as its own: its solid
//! parts' triangles are thrown along the sun the ground's were painted with
//! ([`super::caverns_pieces::BAKED_SUN`]) onto the level of its foot, and the picture of that is
//! laid on the ground and the building's floors (benilla's decal projector) and multiplied in as
//! dark as the painted ones. It is the thing's: made again when its parts or its place change (a
//! tent swapped for another is new parts), gone with it, and none out of the camp, where the runs
//! stay as 1.12 draws them.
//!
//! Where the ground is painted shaded already, the thing's shadow leaves it be, so the two never
//! darken twice. It lands on the camp's other things too, on their faces that look up (a
//! platform's deck under its bookshelves), never on the thing itself. Its edges are soft, each
//! texel shaded as much as the parts cover it, and the small holes between a solid thing's boards
//! are filled, so a bookshelf casts a block, not its frame. Where two things' shadows overlap the
//! ground is shaded once, as under one: the things whose shadows meet are laid as one picture, the
//! darkest of them texel by texel.

use std::hash::{Hash, Hasher};

use benilla_assets::coords::wow_to_bevy;
use benilla_assets::AdtTile;
use benilla_formats::{ModelBlend, RenderSubmesh};
use benilla_world::billboard::BillboardCard;
use benilla_world::decal::{DecalFrame, WorldDecal};
use benilla_world::interact::PickMesh;
use benilla_world::model_render::ModelPart;
use benilla_world::particles::buffer::{begin_effect_frame, EffectVertex, WorldEffectDraw};
use benilla_world::sky_order::Rung;
use benilla_world::terrain_stream::TerrainStreamer;
use benilla_world::view::WorldCamera;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::CampVisit;

/// The ground's painted shadow texel, yards (`CHUNK_SIZE / 64`, MCSH's 64 to a chunk).
const PAINTED_TEXEL: f32 = benilla_formats::CHUNK_SIZE / 64.0;
/// The shadow's texel, yards: an eighth of the painted one, about 6.5 cm, so its edges read as
/// lines, not steps. The game's own
/// options have no shadow detail to tie it to (the stock slider over `shadowLevel` is "Terrain
/// Texture").
const SHADOW_TEXEL: f32 = PAINTED_TEXEL / 8.0;
/// Each texel is sampled on a grid this many to a side and keeps the share of them in the
/// shadow, so its edges fade over a texel instead of stepping.
const SAMPLES: u32 = 4;
/// A shadow wider or deeper than this many texels (some 266 yards) is not drawn (nothing in the
/// camp comes near).
const MOST_TEXELS: u32 = 4096;
/// The ground the shadow lands on lies within this far (yards) below or above the thing's foot,
/// and it fades out over the last [`REACH_FADE`] of it.
const GROUND_REACH: f32 = 4.0;
const REACH_FADE: f32 = 1.5;
/// A face steeper than this (its normal's upward share) is a wall: the shadow is not smeared on it.
const LEAST_UPWARD: f32 = 0.5;
/// What the shadow leaves of the ground's colour, a byte of the effect lane's gamma art: 0.7, as
/// the terrain leaves under its painted shadow. Measured on screen in the Alliance basin: the
/// things' shadows leave 0.71 of the sand (the middle of them), the painted one at the wall's foot
/// 0.65.
const SHADE: u8 = 179;
/// The dynamic objects' high guid, as the server numbers the camp's looks (`ground_looks`).
const HIGH_DYNAMIC_OBJECT: u16 = 0xF100;

pub(super) fn register(app: &mut App) {
    app.init_resource::<LaidShadows>().add_systems(
        PostUpdate,
        (
            cast_object_shadows.after(bevy::transform::TransformSystems::Propagate),
            lay_object_shadows,
            draw_object_shadows.after(begin_effect_frame),
        )
            .chain(),
    );
}

/// A camp thing's shadow, on the thing itself: it goes when the thing does.
#[derive(Component)]
struct ObjectShadow {
    /// What it was cast from: its parts and where they stood.
    key: u64,
    foot: Vec3,
    /// The box round the thing's solid parts, least and most corners.
    body: [Vec3; 2],
    /// Its solid parts' faces that look up, which the other things' shadows lie on.
    faces: Vec<[Vec3; 3]>,
    /// The shadow as cast, which lies on the other things; `None` with nothing to cast.
    mask: Option<ShadowMask>,
    /// The shadow's cover as it lies on the ground: none where the ground is painted shaded.
    ground_cover: Vec<u8>,
}

/// The shadow `tris` (world, Bevy) cast along `sun` (toward the sun, Bevy) onto the level
/// `ground`: how much of each texel of a grid of [`SHADOW_TEXEL`] over it is shaded.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ShadowMask {
    /// The grid's corner (Bevy x, z), on the world's shadow grid: whole texels from its origin.
    pub corner: Vec2,
    /// The corner's texel on the world's shadow grid.
    pub origin: IVec2,
    pub width: u32,
    pub height: u32,
    /// Row by row along z, each along x: the shaded share, 0 to 255.
    pub cover: Vec<u8>,
}

impl ShadowMask {
    /// The middle of texel (`i`, `j`), Bevy x and z.
    pub fn middle(&self, i: u32, j: u32) -> Vec2 {
        self.corner + Vec2::new(i as f32 + 0.5, j as f32 + 0.5) * SHADOW_TEXEL
    }

    /// The shaded share of texel (`i`, `j`), 0 to 1.
    #[cfg(test)]
    pub fn cover_at(&self, i: u32, j: u32) -> f32 {
        f32::from(self.cover[(j * self.width + i) as usize]) / 255.0
    }
}

/// Where a point lands on the level `ground`, thrown along `sun` away from it; a point below the
/// level stays where it stands.
pub(crate) fn thrown(p: Vec3, sun: Vec3, ground: f32) -> Vec2 {
    let q = p - sun * ((p.y - ground).max(0.0) / sun.y);
    Vec2::new(q.x, q.z)
}

/// The shadow of `tris` on the level `ground`, or `None` when there is none to draw.
pub(crate) fn cast_mask(tris: &[[Vec3; 3]], sun: Vec3, ground: f32) -> Option<ShadowMask> {
    let flat: Vec<[Vec2; 3]> = tris.iter().map(|t| t.map(|p| thrown(p, sun, ground))).collect();
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for p in flat.iter().flatten() {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    if lo.x > hi.x {
        return None;
    }
    // on the world's grid, so two shadows' texels meet one to one, and a texel round it, so its
    // edge fades out inside the picture
    let origin = (lo / SHADOW_TEXEL).floor().as_ivec2() - IVec2::ONE;
    let end = (hi / SHADOW_TEXEL).floor().as_ivec2() + IVec2::splat(2);
    let corner = origin.as_vec2() * SHADOW_TEXEL;
    let size = end - origin;
    if size.x > MOST_TEXELS as i32 || size.y > MOST_TEXELS as i32 {
        return None;
    }
    let (width, height) = (size.x as u32, size.y as u32);
    // each texel's samples in the shadow, one bit each: the triangles' union, so two halves of a
    // face that meet inside a texel fill it
    let mut hits = vec![0u16; (width * height) as usize];
    let step = SHADOW_TEXEL / SAMPLES as f32;
    for [a, b, c] in &flat {
        let area = (b - a).perp_dot(c - a);
        if area.abs() < 1e-9 {
            continue;
        }
        let (tlo, thi) = (a.min(*b).min(*c), a.max(*b).max(*c));
        // the texels the triangle's box touches
        let first = ((tlo - corner) / SHADOW_TEXEL).floor().max(Vec2::ZERO).as_uvec2();
        let last = ((thi - corner) / SHADOW_TEXEL).floor();
        if last.x < 0.0 || last.y < 0.0 {
            continue;
        }
        let last = last.as_uvec2().min(UVec2::new(width - 1, height - 1));
        for j in first.y..=last.y {
            for i in first.x..=last.x {
                let at = (j * width + i) as usize;
                if hits[at] == u16::MAX {
                    continue;
                }
                let base = corner + Vec2::new(i as f32, j as f32) * SHADOW_TEXEL;
                for sy in 0..SAMPLES {
                    for sx in 0..SAMPLES {
                        let p = base + (Vec2::new(sx as f32, sy as f32) + 0.5) * step;
                        // inside by all three edges, whichever way round the triangle is wound
                        let e = [(b - a).perp_dot(p - a), (c - b).perp_dot(p - b), (a - c).perp_dot(p - c)];
                        if e.iter().all(|&s| s * area >= 0.0) {
                            hits[at] |= 1 << (sy * SAMPLES + sx);
                        }
                    }
                }
            }
        }
    }
    let all = SAMPLES * SAMPLES;
    let cover: Vec<u8> = hits.iter().map(|h| (h.count_ones() * 255 / all) as u8).collect();
    cover.iter().any(|&c| c > 0).then_some(ShadowMask { corner, origin, width, height, cover })
}

/// The things in the camp that cast a shadow: every game object (the tents, the hubs' pieces, the
/// decorations), every camp look drawn for a dynamic object (the pavilion, the vendor's tent, the
/// armour stand) and every thing to use (a unit wearing its object's model).
fn casts_a_shadow(guid: u64, fields: &benilla_protocol::ObjectFields) -> bool {
    use benilla_protocol::guid;
    use esurv_content::camp_looks::{camp_thing_look, CAMP_LOOKS, CAMP_THING_MODELS};
    if guid::is_gameobject(guid) {
        return true;
    }
    if guid::high(guid) == HIGH_DYNAMIC_OBJECT {
        return fields.dynamicobject_spell_id().is_some_and(|s| CAMP_LOOKS.iter().any(|l| l.id == s));
    }
    guid::is_creature_or_pet(guid)
        && fields
            .unit_displayid()
            .is_some_and(|d| CAMP_THING_MODELS.iter().any(|&(display, _)| camp_thing_look(display) as i32 == d))
}

/// What a thing's shadow is cast from: where it stands, and which parts, of which geometry, stand
/// where. A tent swapped for another, a part loaded late, a model laid on the ground after it came:
/// each changes it.
fn shadow_key<'a>(foot: Vec3, parts: impl Iterator<Item = (Entity, &'a std::sync::Arc<RenderSubmesh>, bevy::math::Affine3A)>) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    foot.to_array().map(f32::to_bits).hash(&mut hasher);
    for (e, mesh, at) in parts {
        e.hash(&mut hasher);
        (std::sync::Arc::as_ptr(mesh) as usize).hash(&mut hasher);
        at.to_cols_array().map(f32::to_bits).hash(&mut hasher);
    }
    hasher.finish()
}

/// A thing's solid parts' triangles in the world.
fn world_tris(parts: &[(&std::sync::Arc<RenderSubmesh>, bevy::math::Affine3A)]) -> Vec<[Vec3; 3]> {
    let mut tris = Vec::new();
    for (mesh, at) in parts {
        let world: Vec<Vec3> = mesh.positions.iter().map(|p| at.transform_point3(Vec3::from(*p))).collect();
        for t in mesh.indices.chunks_exact(3) {
            if let (Some(a), Some(b), Some(c)) = (world.get(t[0] as usize), world.get(t[1] as usize), world.get(t[2] as usize)) {
                tris.push([*a, *b, *c]);
            }
        }
    }
    tris
}

/// Whether a triangle looks up enough to be ground a shadow lies on, not a wall.
fn looks_up(t: &[Vec3; 3]) -> bool {
    (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero().y.abs() >= LEAST_UPWARD
}

/// The longest a frame spends casting shadows, and again laying them; the rest are done in the
/// frames after.
const CAST_BUDGET: std::time::Duration = std::time::Duration::from_millis(4);

/// Each camp thing's shadow cast again when its parts or its place changed; none out of the camp.
#[allow(clippy::too_many_arguments)]
fn cast_object_shadows(
    mut commands: Commands,
    visit: Res<CampVisit>,
    objects: benilla_app::Objects,
    children: Query<&Children>,
    roots: Query<(&GlobalTransform, Option<&ObjectShadow>)>,
    parts: Query<(&PickMesh, &ModelPart, &GlobalTransform, Option<&Visibility>), Without<BillboardCard>>,
    shadowed: Query<Entity, With<ObjectShadow>>,
    streamer: Option<Res<TerrainStreamer>>,
    adt_tiles: Res<Assets<AdtTile>>,
) {
    let start = std::time::Instant::now();
    let roots_cast: Vec<Entity> = if visit.in_camp {
        objects.iter().filter(|(guid, _, fields)| casts_a_shadow(*guid, fields)).map(|(_, e, _)| e).collect()
    } else {
        Vec::new()
    };
    // a thing no longer cast for (out of the camp): its shadow goes
    for e in &shadowed {
        if !roots_cast.contains(&e) {
            commands.entity(e).remove::<ObjectShadow>();
        }
    }
    let sun = wow_to_bevy(super::caverns_pieces::BAKED_SUN);
    // at least one cast a frame, so the gathering never starves the casting
    let mut cast_one = false;
    for &root in &roots_cast {
        let Ok((root_at, had)) = roots.get(root) else {
            continue;
        };
        // its solid parts as drawn: flames, glows and see-through cards cast nothing
        let solid: Vec<_> = children
            .iter_descendants(root)
            .filter_map(|e| parts.get(e).ok().map(|p| (e, p)))
            .filter(|(_, (_, part, _, shown))| *shown != Some(&Visibility::Hidden) && matches!(part.blend, ModelBlend::Opaque | ModelBlend::AlphaTest))
            .collect();
        let foot = root_at.translation();
        let key = shadow_key(foot, solid.iter().map(|(e, (mesh, _, at, _))| (*e, &mesh.0, at.affine())));
        if had.is_some_and(|s| s.key == key) {
            continue;
        }
        if cast_one && start.elapsed() > CAST_BUDGET {
            break;
        }
        cast_one = true;
        let parts: Vec<_> = solid.iter().map(|(_, (mesh, _, at, _))| (&mesh.0, at.affine())).collect();
        let tris = world_tris(&parts);
        let (mut lo, mut hi) = (foot, foot);
        for p in tris.iter().flatten() {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        let faces = tris.iter().filter(|t| looks_up(t)).copied().collect();
        let Some(mut mask) = cast_mask(&tris, sun, foot.y) else {
            // nothing to cast (no solid part shown yet): kept, so it is not cast again till it changes
            commands.entity(root).insert(ObjectShadow { key, foot, body: [lo, hi], faces, mask: None, ground_cover: Vec::new() });
            continue;
        };
        fill_small_holes(&mut mask);
        // where the ground is painted shaded already, it is left as painted; its tile not read
        // yet, tried again next frame
        let Some(painted) = painted_shade(&mask, foot.y, streamer.as_deref(), &adt_tiles) else {
            continue;
        };
        let ground_cover = mask.cover.iter().zip(painted).map(|(&c, p)| (f32::from(c) * (1.0 - p)).round() as u8).collect();
        commands.entity(root).insert(ObjectShadow { key, foot, body: [lo, hi], faces, mask: Some(mask), ground_cover });
    }
}

/// The biggest hole inside a shadow that is filled, square yards: the gaps between a bookshelf's
/// boards and books, a rack's bars, which would read as holes.
/// A tent's open side or an arch reaches the shadow's edge, or is wider, and stays open.
const LARGEST_FILLED_HOLE: f32 = 1.5;

/// Fill the small holes inside `mask`'s shadow: the texels not wholly shaded that cannot be
/// reached from its edge without crossing a wholly shaded one.
fn fill_small_holes(mask: &mut ShadowMask) {
    let (w, h) = (mask.width as usize, mask.height as usize);
    let cover = &mask.cover;
    let open = |at: usize| cover[at] < 255;
    // 0 not yet seen, 1 reached from the edge, 2 and on: a hole's number
    let mut seen = vec![0u32; w * h];
    let mut stack = Vec::new();
    for i in 0..w {
        stack.push(i);
        stack.push((h - 1) * w + i);
    }
    for j in 0..h {
        stack.push(j * w);
        stack.push(j * w + w - 1);
    }
    let flood = |seen: &mut Vec<u32>, stack: &mut Vec<usize>, mark: u32| -> usize {
        let mut count = 0;
        while let Some(at) = stack.pop() {
            if seen[at] != 0 || !open(at) {
                continue;
            }
            seen[at] = mark;
            count += 1;
            let (i, j) = (at % w, at / w);
            if i > 0 {
                stack.push(at - 1);
            }
            if i + 1 < w {
                stack.push(at + 1);
            }
            if j > 0 {
                stack.push(at - w);
            }
            if j + 1 < h {
                stack.push(at + w);
            }
        }
        count
    };
    flood(&mut seen, &mut stack, 1);
    let most = (LARGEST_FILLED_HOLE / (SHADOW_TEXEL * SHADOW_TEXEL)) as usize;
    let mut filled = Vec::new();
    let mut mark = 2;
    for at in 0..w * h {
        if seen[at] != 0 || !open(at) {
            continue;
        }
        stack.push(at);
        if flood(&mut seen, &mut stack, mark) <= most {
            filled.push(mark);
        }
        mark += 1;
    }
    for (s, c) in seen.iter().zip(mask.cover.iter_mut()) {
        if filled.contains(s) {
            *c = 255;
        }
    }
}

/// How much the ground has painted shaded under each of `mask`'s texels, 0 to 1: its painted
/// texels read between their middles, as the terrain's own filter reads them, so the thing's
/// shadow gives way to the painted one smoothly; `None` while a tile under it is still being read.
fn painted_shade(mask: &ShadowMask, ground: f32, streamer: Option<&TerrainStreamer>, adt_tiles: &Assets<AdtTile>) -> Option<Vec<f32>> {
    use benilla_world::terrain_stream::{doodad_ground_shade, ShadeResolve};
    let Some(streamer) = streamer else {
        return Some(vec![0.0; mask.cover.len()]);
    };
    // the painted texels' edges lie on whole multiples of it from the world's origin
    let mut painted: std::collections::HashMap<(i64, i64), f32> = std::collections::HashMap::new();
    let mut out = Vec::with_capacity(mask.cover.len());
    for j in 0..mask.height {
        for i in 0..mask.width {
            if mask.cover[(j * mask.width + i) as usize] == 0 {
                out.push(0.0);
                continue;
            }
            let at = mask.middle(i, j) / PAINTED_TEXEL - 0.5;
            let (cell, part) = (at.floor(), at - at.floor());
            let mut corner = |di: i64, dj: i64| -> Option<f32> {
                let key = (cell.x as i64 + di, cell.y as i64 + dj);
                if let Some(&p) = painted.get(&key) {
                    return Some(p);
                }
                let m = (Vec2::new(key.0 as f32, key.1 as f32) + 0.5) * PAINTED_TEXEL;
                let p = match doodad_ground_shade(streamer, adt_tiles, Vec3::new(m.x, ground, m.y)) {
                    ShadeResolve::Ready(shaded) => f32::from(u8::from(shaded)),
                    ShadeResolve::Pending => return None,
                };
                painted.insert(key, p);
                Some(p)
            };
            let (p00, p10, p01, p11) = (corner(0, 0)?, corner(1, 0)?, corner(0, 1)?, corner(1, 1)?);
            let near = p00 + (p10 - p00) * part.x;
            let far = p01 + (p11 - p01) * part.x;
            out.push(near + (far - near) * part.y);
        }
    }
    Some(out)
}

/// A block of the world's shadow grid, the one every shadow is cast on, so two shadows' texels
/// meet one to one: its least texel and how many texels it is across.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TexelSpan {
    origin: IVec2,
    size: UVec2,
}

impl TexelSpan {
    fn of(mask: &ShadowMask) -> Self {
        TexelSpan { origin: mask.origin, size: UVec2::new(mask.width, mask.height) }
    }

    fn union(self, other: Self) -> Self {
        let origin = self.origin.min(other.origin);
        let end = (self.origin + self.size.as_ivec2()).max(other.origin + other.size.as_ivec2());
        TexelSpan { origin, size: (end - origin).as_uvec2() }
    }

    /// Its square on the ground (Bevy x, z), least and most corners.
    fn reach(self) -> [Vec2; 2] {
        [self.origin.as_vec2() * SHADOW_TEXEL, (self.origin + self.size.as_ivec2()).as_vec2() * SHADOW_TEXEL]
    }
}

/// The darkest of `covers` over `span`, texel by texel: where shadows overlap the ground is shaded
/// once, as under one.
fn darkest<'a>(span: TexelSpan, covers: impl Iterator<Item = (&'a ShadowMask, &'a [u8])>) -> Vec<u8> {
    let mut out = vec![0u8; (span.size.x * span.size.y) as usize];
    for (mask, cover) in covers {
        let off = (mask.origin - span.origin).as_uvec2();
        for j in 0..mask.height {
            for i in 0..mask.width {
                let c = cover[(j * mask.width + i) as usize];
                let at = ((off.y + j) * span.size.x + off.x + i) as usize;
                out[at] = out[at].max(c);
            }
        }
    }
    out
}

/// How much a shadow cast on the level `ground` lies on a face at the height `y`: wholly near it,
/// fading out over the last [`REACH_FADE`] of [`GROUND_REACH`].
fn reach_fade(y: f32, ground: f32) -> f32 {
    ((GROUND_REACH - (y - ground).abs()) / REACH_FADE).clamp(0.0, 1.0)
}

/// How much shadows cast on the levels `feet` lie on a face at the height `y`, laid as one: only
/// as far as every one of them reaches, so a higher thing's level never carries a lower one's
/// shadow up onto a ledge.
fn reach_fade_all(y: f32, feet: &[f32]) -> f32 {
    feet.iter().map(|&f| reach_fade(y, f)).fold(1.0, f32::min)
}

/// Things standing this near one another's level (yards) are laid together where their shadows
/// meet: a thing on a platform's deck with one on the ground by it. One higher (a banner up a
/// wall) is laid apart, so a low one's shadow is not carried up to the ledges in its reach.
const SAME_LEVEL: f32 = 1.5;

/// The things whose shadows or bodies (`boxes`) overlap and that stand on one level (`feet`), in
/// groups: no group's feet lie more than [`SAME_LEVEL`] apart (a chain of things a step higher
/// each is cut where it climbs past it), and no shadow of one group reaches another's shadow or
/// body on its level, so each group is laid on its own.
fn overlapping(boxes: &[[Vec2; 2]], feet: &[f32]) -> Vec<Vec<usize>> {
    let mut group: Vec<usize> = (0..boxes.len()).collect();
    // each group's lowest and highest foot, kept on its first
    let mut levels: Vec<[f32; 2]> = feet.iter().map(|&f| [f, f]).collect();
    fn first(group: &mut [usize], mut k: usize) -> usize {
        while group[k] != k {
            group[k] = group[group[k]];
            k = group[k];
        }
        k
    }
    for a in 0..boxes.len() {
        for b in a + 1..boxes.len() {
            let (p, q) = (boxes[a], boxes[b]);
            if !(p[0].x <= q[1].x && q[0].x <= p[1].x && p[0].y <= q[1].y && q[0].y <= p[1].y) {
                continue;
            }
            let (ra, rb) = (first(&mut group, a), first(&mut group, b));
            if ra == rb {
                continue;
            }
            let both = [levels[ra][0].min(levels[rb][0]), levels[ra][1].max(levels[rb][1])];
            if both[1] - both[0] <= SAME_LEVEL {
                group[ra.max(rb)] = ra.min(rb);
                levels[ra.min(rb)] = both;
            }
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
    for k in 0..boxes.len() {
        let g = first(&mut group, k);
        groups.entry(g).or_default().push(k);
    }
    groups.into_values().collect()
}

/// The camp's shadows as laid, by group (its things and their shadows' keys): each group laid as
/// one, so where two shadows meet the ground is shaded once, not twice.
#[derive(Resource, Default)]
struct LaidShadows {
    groups: std::collections::HashMap<u64, Vec<LaidShadow>>,
}

/// One picture of shadow laid on the ground, or on one thing's faces.
struct LaidShadow {
    owner: Entity,
    anchor: Vec3,
    picture: Handle<Image>,
    verts: Vec<EffectVertex>,
}

/// A thing whose shadow is laid: its root and its shadow, cast.
type Laying<'a> = (Entity, &'a ObjectShadow, &'a ShadowMask);

/// The camp's shadows laid again where a group's things or their shadows changed.
fn lay_object_shadows(shadows: Query<(Entity, &ObjectShadow)>, mut laid: ResMut<LaidShadows>, mut images: ResMut<Assets<Image>>, decals: WorldDecal) {
    let start = std::time::Instant::now();
    let mut cast: Vec<Laying> = shadows.iter().filter_map(|(e, s)| s.mask.as_ref().map(|m| (e, s, m))).collect();
    cast.sort_by_key(|(e, _, _)| *e);
    let boxes: Vec<[Vec2; 2]> = cast
        .iter()
        .map(|(_, s, m)| {
            let reach = TexelSpan::of(m).reach();
            [reach[0].min(s.body[0].xz()), reach[1].max(s.body[1].xz())]
        })
        .collect();
    let mut wanted = std::collections::HashSet::new();
    let (mut all_laid, mut laid_one) = (true, false);
    let feet: Vec<f32> = cast.iter().map(|(_, s, _)| s.foot.y).collect();
    for group in overlapping(&boxes, &feet) {
        let members: Vec<Laying> = group.iter().map(|&k| cast[k]).collect();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for (e, s, _) in &members {
            e.hash(&mut hasher);
            s.key.hash(&mut hasher);
        }
        let key = hasher.finish();
        wanted.insert(key);
        if laid.groups.contains_key(&key) {
            continue;
        }
        if laid_one && start.elapsed() > CAST_BUDGET {
            all_laid = false;
            continue;
        }
        laid_one = true;
        match lay_group(&members, &mut images, &decals) {
            Some(group) => {
                laid.groups.insert(key, group);
            }
            // no ground under it yet: tried again next frame
            None => all_laid = false,
        }
    }
    // the groups that changed go once all the new ones are laid, so none blinks out meanwhile
    if all_laid {
        laid.groups.retain(|key, _| wanted.contains(key));
    }
}

/// A group's shadows laid: on the ground the darkest of them all, and on each of its things' faces
/// that look up the darkest of the others' (never its own); `None` with no ground under it yet.
fn lay_group(members: &[Laying], images: &mut Assets<Image>, decals: &WorldDecal) -> Option<Vec<LaidShadow>> {
    let span = members.iter().map(|(_, _, m)| TexelSpan::of(m)).reduce(TexelSpan::union)?;
    if span.size.x > MOST_TEXELS || span.size.y > MOST_TEXELS {
        // too wide to lay as one (nothing in the camp comes near): each on its own
        if members.len() == 1 {
            return Some(Vec::new());
        }
        let mut out = Vec::new();
        for m in members {
            out.extend(lay_group(std::slice::from_ref(m), images, decals)?);
        }
        return Some(out);
    }
    let feet: Vec<f32> = members.iter().map(|(_, s, _)| s.foot.y).collect();
    let mut out = Vec::new();
    let verts = lay_on_the_ground(span, &feet, decals)?;
    if !verts.is_empty() {
        let cover = darkest(span, members.iter().map(|(_, s, m)| (*m, s.ground_cover.as_slice())));
        let (owner, first, _) = members[0];
        out.push(LaidShadow { owner, anchor: first.foot, picture: images.add(shadow_picture(span, cover)), verts });
    }
    for (me, &(owner, shadow, _)) in members.iter().enumerate() {
        let others: Vec<Laying> = members.iter().enumerate().filter(|&(k, _)| k != me).map(|(_, &o)| o).collect();
        let Some(span) = others.iter().map(|(_, _, m)| TexelSpan::of(m)).reduce(TexelSpan::union) else {
            continue;
        };
        let feet: Vec<f32> = others.iter().map(|(_, s, _)| s.foot.y).collect();
        let verts = lay_on_a_thing(&shadow.faces, span, &feet);
        if verts.is_empty() {
            continue;
        }
        let cover = darkest(span, others.iter().map(|(_, _, m)| (*m, m.cover.as_slice())));
        out.push(LaidShadow { owner, anchor: shadow.foot, picture: images.add(shadow_picture(span, cover)), verts });
    }
    Some(out)
}

/// A picture over `span` laid on a thing's `faces` that lie near the levels `feet` the shadows
/// were cast on, as the ground's triangles are: each corner's place in the square is its
/// picture's place, and past the square the picture's empty edge.
fn lay_on_a_thing(faces: &[[Vec3; 3]], span: TexelSpan, feet: &[f32]) -> Vec<EffectVertex> {
    let reach = span.reach();
    let size = reach[1] - reach[0];
    let fade = |y: f32| reach_fade_all(y, feet);
    let mut out = Vec::new();
    for t in faces {
        let (lo, hi) = (t[0].min(t[1]).min(t[2]), t[0].max(t[1]).max(t[2]));
        if lo.x > reach[1].x || hi.x < reach[0].x || lo.z > reach[1].y || hi.z < reach[0].y {
            continue;
        }
        if t.iter().all(|p| fade(p.y) == 0.0) {
            continue;
        }
        for p in t {
            let uv = (Vec2::new(p.x, p.z) - reach[0]) / size;
            out.push(EffectVertex { pos: p.to_array(), uv: uv.to_array(), color: [1.0, 1.0, 1.0, fade(p.y)] });
        }
    }
    out
}

/// The ground's and the building's triangles under `span` near the levels `feet` the shadows were
/// cast on (walls left out); `None` with no ground there.
fn lay_on_the_ground(span: TexelSpan, feet: &[f32], decals: &WorldDecal) -> Option<Vec<EffectVertex>> {
    let reach = span.reach();
    let size = reach[1] - reach[0];
    // as far as every level reaches (things laid together stand within SAME_LEVEL of one another)
    let low = feet.iter().copied().fold(f32::MIN, f32::max) - GROUND_REACH;
    let high = feet.iter().copied().fold(f32::MAX, f32::min) + GROUND_REACH;
    let middle = (low + high) / 2.0;
    let frame = DecalFrame {
        center: Vec3::new(reach[0].x, middle, reach[0].y),
        sin: 0.0,
        cos: 1.0,
        min_x: 0.0,
        max_x: size.x,
        min_z: 0.0,
        max_z: size.y,
        min_y: low - middle,
        max_y: high - middle,
    };
    let fade = |p: Vec3| reach_fade_all(p.y + middle, feet);
    let mut verts = Vec::new();
    if !decals.project(&mut verts, &frame, fade, |x, z| frame.rect_uv(x, z)) {
        return None;
    }
    let mut kept = Vec::with_capacity(verts.len());
    for tri in verts.chunks_exact(3) {
        if looks_up(&[Vec3::from(tri[0].pos), Vec3::from(tri[1].pos), Vec3::from(tri[2].pos)]) {
            kept.extend_from_slice(tri);
        }
    }
    Some(kept)
}

/// A shadow's picture over `span`: its shade, as much of it as each texel is shaded.
fn shadow_picture(span: TexelSpan, cover: Vec<u8>) -> Image {
    let rgba = cover.iter().flat_map(|&c| [SHADE, SHADE, SHADE, c]).collect();
    let mut image = Image::new(
        Extent3d { width: span.size.x, height: span.size.y, depth_or_array_layers: 1 },
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

/// The camp's shadows onto the effect stream, multiplied in at the blob shadows' rung: over the
/// ground and the building, under the things themselves.
fn draw_object_shadows(laid: Res<LaidShadows>, cam: Query<Entity, With<WorldCamera>>, mut draw: WorldEffectDraw) {
    let Ok(cam) = cam.single() else {
        return;
    };
    for s in laid.groups.values().flatten() {
        let mut batch = draw.batch(cam, s.picture.id()).multiply().anchored(s.anchor).rung(Rung::SHADOW_SORT, Rung::DECAL_RASTER).owner(s.owner);
        batch.vertices(&s.verts);
        batch.tris();
    }
}

#[cfg(test)]
mod tests {
    use benilla_assets::coords::bevy_to_wow;

    use super::*;

    fn sun() -> Vec3 {
        wow_to_bevy(super::super::caverns_pieces::BAKED_SUN)
    }

    /// The middles of the texels mostly shaded.
    fn shaded(mask: &ShadowMask) -> Vec<Vec2> {
        (0..mask.height)
            .flat_map(|j| (0..mask.width).map(move |i| (i, j)))
            .filter(|&(i, j)| mask.cover_at(i, j) > 0.5)
            .map(|(i, j)| mask.middle(i, j))
            .collect()
    }

    /// The shadow's area, square yards, by its texels' shaded shares.
    fn area(mask: &ShadowMask) -> f32 {
        mask.cover.iter().map(|&c| f32::from(c) / 255.0).sum::<f32>() * SHADOW_TEXEL * SHADOW_TEXEL
    }

    #[test]
    fn a_point_above_the_ground_is_thrown_away_from_the_sun() {
        // the sun stands north-west and 46.5° up: a point 10 yards up falls south-east of its
        // foot, a little less than 10 yards off
        let at = thrown(Vec3::new(0.0, 10.0, 0.0), sun(), 0.0);
        let wow = bevy_to_wow(Vec3::new(at.x, 0.0, at.y));
        assert!(wow[0] < -6.0 && wow[1] < -6.0, "{wow:?}");
        let off = 10.0 * (1.0 - 0.7254f32 * 0.7254).sqrt() / 0.7254;
        assert!((at.length() - off).abs() < 0.05, "{at:?}");
        // one on the ground stays put
        assert_eq!(thrown(Vec3::new(3.0, 0.0, 4.0), sun(), 0.0), Vec2::new(3.0, 4.0));
    }

    #[test]
    fn a_pole_casts_a_long_thin_shadow_away_from_the_sun() {
        // a 10-yard pole half a yard thick, as two crossed walls
        let (h, r) = (10.0, 0.25);
        let wall = |a: Vec3, b: Vec3| [[a, b, b + Vec3::Y * h], [a, b + Vec3::Y * h, a + Vec3::Y * h]];
        let mut tris = Vec::new();
        tris.extend(wall(Vec3::new(-r, 0.0, 0.0), Vec3::new(r, 0.0, 0.0)));
        tris.extend(wall(Vec3::new(0.0, 0.0, -r), Vec3::new(0.0, 0.0, r)));
        let mask = cast_mask(&tris, sun(), 0.0).expect("a shadow");
        let shaded = shaded(&mask);
        let tip = shaded.iter().copied().max_by(|a, b| a.length().total_cmp(&b.length())).unwrap();
        let reach = thrown(Vec3::Y * h, sun(), 0.0).length();
        // within the pole's own half-thickness of its top's shadow
        assert!((tip.length() - reach).abs() < r + 2.0 * SHADOW_TEXEL, "reaches {} of {reach}", tip.length());
        let tip_wow = bevy_to_wow(Vec3::new(tip.x, 0.0, tip.y));
        assert!(tip_wow[0] < 0.0 && tip_wow[1] < 0.0, "{tip_wow:?}");
        // thin: under a yard across along its length
        assert!(area(&mask) < reach, "{} square yards for {reach} along", area(&mask));
    }

    #[test]
    fn a_roof_shades_its_own_square_moved_away_from_the_sun() {
        // a flat 4×4 roof 3 yards up: its shadow is the roof's square, moved away from the sun
        let (y, a, b) = (3.0, Vec3::new(-2.0, 3.0, -2.0), Vec3::new(2.0, 3.0, 2.0));
        let roof = [[a, Vec3::new(b.x, y, a.z), b], [a, b, Vec3::new(a.x, y, b.z)]];
        let mask = cast_mask(&roof, sun(), 0.0).expect("a shadow");
        let shift = thrown(Vec3::Y * y, sun(), 0.0);
        let shaded = shaded(&mask);
        assert!((area(&mask) - 16.0).abs() < 0.2, "{}", area(&mask));
        for p in &shaded {
            let d = *p - shift;
            assert!(d.x.abs() <= 2.0 && d.y.abs() <= 2.0, "{p:?} off the moved square");
        }
    }

    #[test]
    fn an_edge_through_a_texel_shades_its_share() {
        // a rectangle on the ground 9.5 texels wide, from the grid's second column: its right
        // edge runs down the middle of the eleventh, half shaded, the one inside whole, the one
        // outside not at all
        let (w, h) = (9.5 * SHADOW_TEXEL, 10.0 * SHADOW_TEXEL);
        let (a, b, c, d) = (Vec3::ZERO, Vec3::new(w, 0.0, 0.0), Vec3::new(w, 0.0, h), Vec3::new(0.0, 0.0, h));
        let mask = cast_mask(&[[a, b, c], [a, c, d]], sun(), 0.0).expect("a shadow");
        assert_eq!(mask.corner, Vec2::splat(-SHADOW_TEXEL));
        let row = 5;
        assert!((mask.cover_at(10, row) - 0.5).abs() < 0.05, "{}", mask.cover_at(10, row));
        assert_eq!(mask.cover_at(9, row), 1.0);
        assert_eq!(mask.cover_at(11, row), 0.0);
    }

    #[test]
    fn nothing_casts_no_shadow() {
        assert_eq!(cast_mask(&[], sun(), 0.0), None);
        // a triangle edge-on to the sun throws a line, no texel's middle
        let flat = [Vec3::ZERO, Vec3::X * 0.01, sun() * 2.0];
        assert_eq!(cast_mask(&[flat], sun(), 0.0), None);
    }

    #[test]
    fn the_camps_things_cast_and_its_people_do_not() {
        use benilla_protocol::guid::{HIGH_GAMEOBJECT, HIGH_PLAYER, HIGH_UNIT};
        let fields = benilla_protocol::ObjectFields::default();
        let guid = |high: u16| (high as u64) << 48 | 5;
        assert!(casts_a_shadow(guid(HIGH_GAMEOBJECT), &fields));
        // a hero and a camp NPC have their blob; a dynamic object that is no camp look, nothing
        assert!(!casts_a_shadow(guid(HIGH_PLAYER), &fields));
        assert!(!casts_a_shadow(guid(HIGH_UNIT), &fields));
        assert!(!casts_a_shadow(guid(HIGH_DYNAMIC_OBJECT), &fields));
    }

    #[test]
    fn a_swapped_part_or_a_moved_thing_casts_anew() {
        let geometry = |n: f32| std::sync::Arc::new(RenderSubmesh { positions: vec![[n, 0.0, 0.0]], ..RenderSubmesh::default() });
        let (tent, other) = (geometry(1.0), geometry(2.0));
        let (part, new_part) = (Entity::from_bits(7 | 1 << 32), Entity::from_bits(8 | 1 << 32));
        let at = bevy::math::Affine3A::IDENTITY;
        let key = |foot: Vec3, e: Entity, m: &std::sync::Arc<RenderSubmesh>| shadow_key(foot, [(e, m, at)].into_iter());
        let standing = key(Vec3::ZERO, part, &tent);
        assert_eq!(standing, key(Vec3::ZERO, part, &tent));
        // another tent's model on the same part, a new part, the thing moved
        assert_ne!(standing, key(Vec3::ZERO, part, &other));
        assert_ne!(standing, key(Vec3::ZERO, new_part, &tent));
        assert_ne!(standing, key(Vec3::X, part, &tent));
        // laid on the ground after it came
        let laid = bevy::math::Affine3A::from_rotation_x(0.1);
        assert_ne!(standing, shadow_key(Vec3::ZERO, [(part, &tent, laid)].into_iter()));
    }

    /// A flat square on the ground from `lo` to `hi` (Bevy x, z), as two triangles.
    fn square(lo: Vec2, hi: Vec2) -> [[Vec3; 3]; 2] {
        let p = |x: f32, z: f32| Vec3::new(x, 0.0, z);
        [[p(lo.x, lo.y), p(hi.x, lo.y), p(hi.x, hi.y)], [p(lo.x, lo.y), p(hi.x, hi.y), p(lo.x, hi.y)]]
    }

    #[test]
    fn shadows_lie_on_one_grid() {
        // two squares a third of a texel apart: their pictures' texels meet one to one
        let a = cast_mask(&square(Vec2::new(1.0, 1.0), Vec2::new(2.0, 2.0)), sun(), 0.0).unwrap();
        let b = cast_mask(&square(Vec2::new(1.5 + SHADOW_TEXEL / 3.0, 1.2), Vec2::new(3.0, 2.5)), sun(), 0.0).unwrap();
        for m in [&a, &b] {
            assert!((m.corner - m.origin.as_vec2() * SHADOW_TEXEL).length() < 1e-5);
        }
    }

    #[test]
    fn overlapping_shadows_shade_once() {
        // two squares overlapping by half a yard: laid together, the overlap is as dark as either
        let a = cast_mask(&square(Vec2::ZERO, Vec2::splat(1.0)), sun(), 0.0).unwrap();
        let b = cast_mask(&square(Vec2::new(0.5, 0.0), Vec2::new(1.5, 1.0)), sun(), 0.0).unwrap();
        let span = TexelSpan::of(&a).union(TexelSpan::of(&b));
        let cover = darkest(span, [(&a, a.cover.as_slice()), (&b, b.cover.as_slice())].into_iter());
        assert_eq!(cover.iter().copied().max(), Some(255));
        // every texel the darker of the two, never more
        let at = |m: &ShadowMask, t: IVec2| {
            let k = t - m.origin;
            (k.x >= 0 && k.y >= 0 && (k.x as u32) < m.width && (k.y as u32) < m.height).then(|| m.cover[(k.y as u32 * m.width + k.x as u32) as usize]).unwrap_or(0)
        };
        for j in 0..span.size.y {
            for i in 0..span.size.x {
                let t = span.origin + IVec2::new(i as i32, j as i32);
                assert_eq!(cover[(j * span.size.x + i) as usize], at(&a, t).max(at(&b, t)));
            }
        }
        // the middle of the overlap wholly shaded, once
        let mid = (Vec2::new(0.75, 0.5) / SHADOW_TEXEL).floor().as_ivec2() - span.origin;
        assert_eq!(cover[(mid.y as u32 * span.size.x + mid.x as u32) as usize], 255);
    }

    #[test]
    fn only_things_whose_shadows_meet_are_laid_together() {
        let boxes = [
            [Vec2::ZERO, Vec2::splat(2.0)],
            [Vec2::splat(1.0), Vec2::splat(3.0)],
            [Vec2::splat(10.0), Vec2::splat(11.0)],
            [Vec2::new(2.5, 2.5), Vec2::new(4.0, 4.0)],
        ];
        assert_eq!(overlapping(&boxes, &[0.0; 4]), vec![vec![0, 1, 3], vec![2]]);
        // one standing high above the rest is laid apart
        assert_eq!(overlapping(&boxes, &[0.0, 1.0, 0.0, 5.0]), vec![vec![0, 1], vec![2], vec![3]]);
        // a chain climbing a yard a step is cut where it climbs past SAME_LEVEL
        let chain: Vec<[Vec2; 2]> = (0..4).map(|k| [Vec2::splat(k as f32), Vec2::splat(k as f32 + 1.5)]).collect();
        let feet = [0.0, 1.0, 2.0, 3.0];
        let groups = overlapping(&chain, &feet);
        assert_eq!(groups, vec![vec![0, 1], vec![2, 3]]);
        for g in &groups {
            let (lo, hi) = g.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &k| (lo.min(feet[k]), hi.max(feet[k])));
            assert!(hi - lo <= SAME_LEVEL);
        }
    }
}
