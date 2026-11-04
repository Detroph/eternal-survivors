//! Composed creature displays: a display id a game built on top of benilla gives a look of its own,
//! made at run time from two of the player's own models. A part model (any M2, or a weapon's
//! `ItemDisplayInfo` model in its own texture) stands planted in the base display's model at its
//! ground and moves as the base's body does (it rides the bone the body rides), while the base
//! keeps its animation and effects; the base's solid body can be hidden so only its effects show
//! around the part. The composed id answers every other lookup
//! (scale, size, sounds) as its base display. 1.12 has no such display; nothing here runs unless a
//! game registers one ([`ComposedDisplays`]).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use benilla_assets::coords::{bevy_to_wow, wow_to_bevy};
use benilla_assets::{
    m2_url, skin_url, GlobalBone, GlobalSeqChannel, M2Model, ModelJoint, ModelSkeleton, ModelSubmesh,
};
use benilla_formats::{CharSkinSlot, ModelBlend, RenderSubmesh};
use bevy::math::Affine3A;
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::prelude::*;

use super::display::{new_creature_display, ModelHandle};
use super::{Creatures, ItemDisplays};
use crate::net::NetEntity;

/// One composed display: what a game on top of benilla registers.
#[derive(Clone, Debug, PartialEq)]
pub struct ComposedDisplay {
    /// The display id the server sends, one `CreatureDisplayInfo` does not have.
    pub id: u32,
    /// The `CreatureDisplayInfo` display it is drawn as: its model, animation and effects.
    pub base: u32,
    /// The model planted in it.
    pub part: PartModel,
    /// How the part stands, and what of the base still shows.
    pub placement: PartPlacement,
    /// A band of the part's colours turned to another hue: its textures, re-read from the
    /// player's own data and shifted in memory, and its particles' colour keys. `None` keeps them.
    pub recolor: Option<HueShift>,
}

/// A hue band turned to another, for a part drawn in other colours than it was made in (the
/// Caverns portal's violet nether turned gold). Grey stays grey: a pixel turns by how saturated
/// it is, and by how deep in the band its hue sits, so the band's edges blend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HueShift {
    /// The hues turned, degrees on the colour wheel (`0` red, `120` green, `240` blue), low to
    /// high, within `0..360`.
    pub from: (f32, f32),
    /// Where the band lands, `from`'s low end on `to.0` and its high end on `to.1`.
    pub to: (f32, f32),
    /// How far past each end of `from` a hue still turns, fading out, degrees.
    pub feather: f32,
    /// Saturation under which a pixel is grey and kept; it turns fully [SATURATION_RAMP] over it.
    pub min_saturation: f32,
    /// The turned colour's brightness, times its own (a dark violet is a dull brown in gold).
    pub brighten: f32,
}

/// How far over [HueShift::min_saturation] a colour turns fully: narrow, so a tinted grey is not
/// half turned into a muddy mix of the two.
const SATURATION_RAMP: f32 = 0.15;

impl HueShift {
    /// One colour turned, any scale (bytes over 255 or linear floats), alpha untouched.
    pub fn shift_rgb(&self, rgb: [f32; 3]) -> [f32; 3] {
        let [r, g, b] = rgb;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        if max <= 0.0 {
            return rgb;
        }
        let chroma = max - min;
        let sat = chroma / max;
        if chroma <= 0.0 || sat < self.min_saturation {
            return rgb;
        }
        let hue = if max == r {
            60.0 * ((g - b) / chroma).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / chroma + 2.0)
        } else {
            60.0 * ((r - g) / chroma + 4.0)
        };
        let (lo, hi) = self.from;
        let feather = self.feather.max(1e-3);
        let inside = if hue < lo {
            1.0 - (lo - hue) / feather
        } else if hue > hi {
            1.0 - (hue - hi) / feather
        } else {
            1.0
        };
        let weight =
            inside.clamp(0.0, 1.0) * ((sat - self.min_saturation) / SATURATION_RAMP).clamp(0.0, 1.0);
        if weight <= 0.0 {
            return rgb;
        }
        let along = ((hue.clamp(lo, hi) - lo) / (hi - lo).max(1e-3)).clamp(0.0, 1.0);
        let to = (self.to.0 + along * (self.to.1 - self.to.0)).rem_euclid(360.0);
        let value = max * self.brighten;
        // back from hue, saturation and value
        let c = value * sat;
        let x = c * (1.0 - ((to / 60.0).rem_euclid(2.0) - 1.0).abs());
        let (r1, g1, b1) = match (to / 60.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = value - c;
        let turned = [r1 + m, g1 + m, b1 + m];
        [0, 1, 2].map(|k| rgb[k] + (turned[k] - rgb[k]) * weight)
    }

    /// RGBA8 pixels turned in place.
    pub fn shift_rgba8(&self, rgba: &mut [u8]) {
        for px in rgba.chunks_exact_mut(4) {
            let rgb = self.shift_rgb([px[0], px[1], px[2]].map(f32::from));
            for k in 0..3 {
                px[k] = rgb[k].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// A texture re-read from the player's data and turned by `shift`, in the world-art upload
/// (authored mips, `wrap` address mode). `None` when the file does not read or decode.
fn shifted_texture(
    chain: &benilla_formats::Chain,
    path: &str,
    wrap: (bool, bool),
    shift: &HueShift,
) -> Option<Image> {
    let bytes = chain.read(path).ok()?;
    let mut mips = benilla_formats::blp_bytes_to_native_chain(&bytes)
        .ok()?
        .into_rgba8();
    for level in &mut mips.mips {
        shift.shift_rgba8(level);
    }
    Some(benilla_assets::repeat_texture_authored(
        benilla_assets::UploadChain {
            chain: mips,
            format: bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        },
        wrap,
    ))
}

/// The part's batches (the last `subs` of `model`'s) and emitters (the last `emitters`) in
/// `shift`'s colours: each texture turned once, the particles' colour keys turned too.
fn recolor_part(
    model: &mut M2Model,
    subs: usize,
    emitters: usize,
    shift: &HueShift,
    chain: &benilla_formats::Chain,
    images: &mut Assets<Image>,
) -> usize {
    let mut turned: HashMap<(String, bool, bool), Option<Handle<Image>>> = HashMap::new();
    let mut texture = |path: &str, wrap: (bool, bool)| {
        turned
            .entry((path.to_ascii_lowercase(), wrap.0, wrap.1))
            .or_insert_with(|| shifted_texture(chain, path, wrap, shift).map(|i| images.add(i)))
            .clone()
    };
    let n = model.submeshes.len();
    for sub in &mut model.submeshes[n.saturating_sub(subs)..] {
        let Some(path) = sub.geometry.texture.clone() else {
            continue;
        };
        if sub.texture.is_none() {
            continue; // a skin slot, filled at spawn
        }
        if let Some(t) = texture(&path, (sub.geometry.wrap_x, sub.geometry.wrap_y)) {
            sub.texture = Some(t);
        }
        // the batch's constant tint turns with its texture
        if !sub.geometry.vertex_colors.is_empty() {
            let mut geo = (*sub.geometry).clone();
            for c in &mut geo.vertex_colors {
                let [r, g, b] = shift.shift_rgb([c[0], c[1], c[2]]);
                *c = [r, g, b, c[3]];
            }
            sub.geometry = Arc::new(geo);
        }
    }
    let n = model.emitters.len();
    for e in &mut model.emitters[n.saturating_sub(emitters)..] {
        if let Some(t) = e.def.texture.clone().and_then(|p| texture(&p, (true, true))) {
            e.texture = Some(t);
        }
        for key in &mut e.def.over_life.color {
            let [r, g, b] = shift.shift_rgb([key[0], key[1], key[2]]);
            *key = [r, g, b, key[3]];
        }
    }
    turned.values().filter(|t| t.is_some()).count()
}

/// The model a composed display plants in its base.
#[derive(Clone, Debug, PartialEq)]
pub enum PartModel {
    /// An M2 path in the player's own data.
    Model(String),
    /// A weapon's `ItemDisplayInfo` display: its `Item\ObjectComponents\Weapon` model, in the
    /// display's own texture.
    Weapon(u32),
}

/// How a part stands in its base model, in the base's model space at model scale 1 (yards).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartPlacement {
    /// The part model's scale.
    pub scale: f32,
    /// A tip about the part's own Y axis, degrees, before the turn: a weapon model lies along +X
    /// from its grip, so -90 stands it head up.
    pub pitch_degrees: f32,
    /// A tip about the part's own X axis, degrees, after the pitch and before the turn: a positive
    /// roll raises its left (+Y), as a thing stood on a slope across it (ours, not the reference's).
    pub roll_degrees: f32,
    /// Its turn about the vertical axis, degrees, counter-clockwise seen from above.
    pub turn_degrees: f32,
    /// How deep its foot (its lowest point once stood) goes below the base's ground, the model's
    /// origin; the part is centred on the origin across.
    pub sink: f32,
    /// How far the base's particle effects on its moving bones (any bone with a parent) rise.
    pub raise_effects: f32,
    /// How far the base's attachment points rise: the looks the unit wears (an aura's state
    /// model) hang on them; negative lowers them.
    pub raise_attachments: f32,
    /// Hide the base's solid body (and the cards laid on it), keeping its effects: glow cards,
    /// particle emitters and their bones' animation.
    pub hide_base_body: bool,
}

/// The displays a game composes; empty in stock benilla. Insert it before the client runs
/// (`benilla_app::run_with`'s extend).
#[derive(Resource, Clone, Debug, Default)]
pub struct ComposedDisplays(pub Vec<ComposedDisplay>);

/// A person a game dresses: display `id`, its base's body wearing `equipment` (`ItemDisplayInfo`
/// ids by body slot: head, shoulder, shirt, chest, belt, legs, feet, wrists, hands, tabard).
#[derive(Clone, Debug, PartialEq)]
pub struct DressedDisplay {
    pub id: u32,
    pub base: u32,
    pub equipment: [u32; 10],
}

/// The displays a game dresses; empty in stock benilla. Insert it before the client runs
/// (`benilla_app::run_with`'s extend).
#[derive(Resource, Clone, Debug, Default)]
pub struct DressedDisplays(pub Vec<DressedDisplay>);

/// The two source models of each composed display, held while they load and after, the part's
/// object texture, and the handle its cache entry draws, which nothing loads: the composed model
/// is put there.
#[derive(Resource, Default)]
struct ComposeSources(HashMap<u32, Pending>);

struct Pending {
    base: Handle<M2Model>,
    part: Handle<M2Model>,
    /// A weapon's own texture, for its batches that take the object skin.
    part_texture: Option<Handle<Image>>,
    composed: Handle<M2Model>,
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<ComposedDisplays>()
        .init_resource::<DressedDisplays>()
        .init_resource::<ComposeSources>()
        // Before the frame: the aliases and the dressed once, then any composed model whose
        // sources have landed.
        .add_systems(
            PreUpdate,
            (alias_composed_displays, dress_displays, compose_ready_displays).chain(),
        )
        // A composed display in use gets its cache entry before the display build makes a plain
        // one for its id, and before the visuals set's equipment resolve reads the same caches.
        .add_systems(
            Update,
            reserve_composed_displays
                .after(benilla_world::schedule::WorldStage::Net)
                .after(super::evict_display_caches)
                .before(super::DisplayBuildSet)
                .before(super::EntityVisualsSet),
        );
}

/// Once the creature catalog lands, each composed id answers as its base display.
fn alias_composed_displays(
    composed: Res<ComposedDisplays>,
    creatures: Option<ResMut<Creatures>>,
    mut done: Local<bool>,
) {
    if *done || composed.0.is_empty() {
        return;
    }
    let Some(mut creatures) = creatures else {
        return;
    };
    for c in &composed.0 {
        if !creatures.catalog.alias_display(c.id, c.base) {
            warn!(
                "composed display {} refused: the id is a stock display or base {} is missing",
                c.id, c.base
            );
        }
    }
    *done = true;
}

/// Once the creature catalog lands, each dressed id answers as its base in its own gear.
fn dress_displays(dressed: Res<DressedDisplays>, creatures: Option<ResMut<Creatures>>, mut done: Local<bool>) {
    if *done || dressed.0.is_empty() {
        return;
    }
    let Some(mut creatures) = creatures else {
        return;
    };
    for d in &dressed.0 {
        if !creatures.catalog.dress_display(d.id, d.base, d.equipment) {
            warn!(
                "dressed display {} refused: the id is a stock display or base {} is no character model",
                d.id, d.base
            );
        }
    }
    *done = true;
}

/// The part's model path and, for a weapon, its texture's URL.
fn part_files(part: &PartModel, items: Option<&ItemDisplays>) -> Option<(String, Option<String>)> {
    match part {
        PartModel::Model(path) => Some((path.clone(), None)),
        PartModel::Weapon(display) => {
            let d = items?.catalog.get(*display)?;
            let dir = "Item\\ObjectComponents\\Weapon";
            let model = format!("{dir}\\{}", d.model[0].as_ref()?);
            Some((model, d.model_texture[0].as_ref().map(|t| skin_url(dir, t))))
        }
    }
}

/// A composed display a unit shows gets its cache entry under a handle nothing loads, so the
/// display build waits on it, and its sources start loading.
fn reserve_composed_displays(
    composed: Res<ComposedDisplays>,
    mut sources: ResMut<ComposeSources>,
    entities: Query<&NetEntity>,
    creatures: Option<ResMut<Creatures>>,
    items: Option<Res<ItemDisplays>>,
    m2s: Res<Assets<M2Model>>,
    asset_server: Res<AssetServer>,
) {
    if composed.0.is_empty() {
        return;
    }
    let Some(mut creatures) = creatures else {
        return;
    };
    for c in &composed.0 {
        // Peek first: a mutable borrow marks the cache changed, which equipment resolution reads.
        if creatures.models.contains_key(&c.id)
            || !entities.iter().any(|e| e.display_id == Some(c.id))
        {
            continue;
        }
        let (Some(base_path), Some((part_path, part_texture)), mut dm) = (
            creatures.catalog.model(c.base).map(|m| m.model_path),
            part_files(&c.part, items.as_deref()),
            new_creature_display(&creatures.catalog, c.id, &asset_server),
        ) else {
            continue; // not aliased, or no part model: nothing to compose
        };
        if !matches!(dm.handle, ModelHandle::M2(_)) {
            continue;
        }
        // after a cache eviction the composed model, which the sources hold, is reused
        let composed = sources
            .0
            .entry(c.id)
            .or_insert_with(|| Pending {
                base: asset_server.load(m2_url(&base_path)),
                part: asset_server.load(m2_url(&part_path)),
                part_texture: part_texture.map(|t| asset_server.load(t)),
                composed: m2s.reserve_handle(),
            })
            .composed
            .clone();
        dm.handle = ModelHandle::M2(composed);
        creatures.models.insert(c.id, dm);
    }
}

/// Puts each composed model whose two sources have loaded under its reserved handle.
fn compose_ready_displays(
    composed: Res<ComposedDisplays>,
    sources: Res<ComposeSources>,
    mut m2s: ResMut<Assets<M2Model>>,
    mut ibps: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut images: ResMut<Assets<Image>>,
    assets: Option<Res<benilla_assets::WorldAssets>>,
) {
    for c in &composed.0 {
        let Some(p) = sources.0.get(&c.id) else {
            continue;
        };
        if m2s.contains(&p.composed) {
            continue;
        }
        let (Some(base), Some(part)) = (m2s.get(&p.base), m2s.get(&p.part)) else {
            continue;
        };
        let base_ibp: Option<Vec<Mat4>> = ibps.get(&base.inverse_bindposes).map(|b| b.to_vec());
        let planted = planted(
            base,
            part,
            p.part_texture.as_ref(),
            &c.placement,
            base_ibp.as_deref(),
        );
        let model = match planted {
            Some((mut model, rig)) => {
                info!(
                    "composed display {}: {:?} planted in display {}{}",
                    c.id,
                    c.part,
                    c.base,
                    if rig.is_some() { ", its bones on the base's" } else { "" }
                );
                if let Some(rig) = rig {
                    model.inverse_bindposes = ibps.add(SkinnedMeshInverseBindposes::from(rig));
                }
                if let (Some(shift), Some(assets)) = (c.recolor.as_ref(), assets.as_deref()) {
                    let emitters = model.emitters.len() - base.emitters.len();
                    let chain = assets.chain.lock().unwrap_or_else(|e| e.into_inner());
                    let n = recolor_part(
                        &mut model,
                        part.submeshes.len(),
                        emitters,
                        shift,
                        &chain,
                        &mut images,
                    );
                    info!("composed display {}: {n} textures recoloured", c.id);
                }
                model
            }
            None => {
                warn!(
                    "composed display {}: no bone to plant on in display {}'s model, drawn bare",
                    c.id, c.base
                );
                base.clone()
            }
        };
        let _ = m2s.insert(p.composed.id(), model);
    }
}

/// A batch that draws solid surface, not a glow card or an effect.
fn is_solid(sub: &ModelSubmesh) -> bool {
    matches!(sub.blend, ModelBlend::Opaque | ModelBlend::AlphaTest)
        && sub.billboard.is_none()
        && !sub.additive
        && !sub.geometry.joints.is_empty()
}

/// The bones a model's animation moves: keyed in a sequence or in a global sequence.
fn animated_bones(model: &M2Model) -> HashSet<u16> {
    let Some(anims) = &model.animations else {
        return HashSet::new();
    };
    anims
        .pose
        .clips
        .iter()
        .flat_map(|c| c.bones.iter().map(|b| b.bone))
        .chain(anims.global_bones.iter().map(|g| g.bone))
        .collect()
}

/// The bone a part planted at the ground rides: a root no animation moves, the one nearest the
/// model's origin.
fn still_root(model: &M2Model) -> Option<u16> {
    let moving = animated_bones(model);
    model
        .skeleton
        .joints
        .iter()
        .enumerate()
        .filter(|&(i, j)| j.parent < 0 && !moving.contains(&(i as u16)))
        .min_by(|a, b| {
            a.1.local_translation
                .length()
                .total_cmp(&b.1.local_translation.length())
        })
        .and_then(|(i, _)| u16::try_from(i).ok())
}

/// The bone most of a model's solid body rides, if any.
fn body_bone(model: &M2Model) -> Option<u16> {
    let mut count: HashMap<u16, usize> = HashMap::new();
    for j in model
        .submeshes
        .iter()
        .filter(|s| is_solid(s))
        .flat_map(|s| s.geometry.joints.iter())
    {
        *count.entry(j[0]).or_default() += 1;
    }
    count
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(bone, _)| bone)
}

/// What `bone` does to a vertex skinned whole to it in the first frame of `model`'s stand
/// (animation 0), Bevy space: its joint chain posed there times its inverse bind pose.
fn stand_skin(model: &M2Model, bone: u16) -> Option<Mat4> {
    let anims = model.animations.as_ref()?;
    let clip = anims.clips.iter().find(|c| c.anim_id == 0)?;
    let pose = anims.pose.clips.get(anims.pose.node(clip.node)?.clip as usize)?;
    let joints = &model.skeleton.joints;
    let (mut posed, mut pivot) = (Mat4::IDENTITY, Vec3::ZERO);
    let mut at = Some(usize::from(bone));
    while let Some(i) = at {
        let j = joints.get(i)?;
        let keyed = pose.bones.iter().find(|b| usize::from(b.bone) == i);
        let t = keyed
            .and_then(|b| b.translation.sample(0.0))
            .unwrap_or(j.local_translation);
        let r = keyed
            .and_then(|b| b.rotation.sample(0.0))
            .unwrap_or(Quat::IDENTITY);
        let s = keyed.and_then(|b| b.scale.sample(0.0)).unwrap_or(Vec3::ONE);
        posed = Mat4::from_scale_rotation_translation(s, r, t) * posed;
        pivot += j.local_translation;
        at = usize::try_from(j.parent).ok();
    }
    Some(posed * Mat4::from_translation(-pivot))
}

/// The bone a planted part rides and where its rest pose is from the stand's: the base's body
/// bone, so the part moves as the body does, with the inverse of that bone's stand skin, so the
/// part stands where it is placed in the stand's first frame; else a still root bone, unmoved.
fn part_bone(base: &M2Model) -> Option<(u16, Mat4)> {
    let on_body = body_bone(base).and_then(|bone| {
        let skin = stand_skin(base, bone)?;
        (skin.determinant().abs() > 1e-6).then(|| (bone, skin.inverse()))
    });
    on_body.or_else(|| still_root(base).map(|root| (root, Mat4::IDENTITY)))
}

/// Each bone's pivot, its bind position: the rest translations summed down its chain.
fn pivots(skeleton: &ModelSkeleton) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = Vec::with_capacity(skeleton.joints.len());
    for (i, j) in skeleton.joints.iter().enumerate() {
        let parent = usize::try_from(j.parent)
            .ok()
            .filter(|&p| p < i)
            .map_or(Vec3::ZERO, |p| out[p]);
        out.push(parent + j.local_translation);
    }
    out
}

/// The part's sequence slot each of the base's slots plays: the slot of the same animation id,
/// else the part's idle. A part planted in a unit runs on the unit's clock, whose slots are the
/// base's (the stalker's Stand is its slot 2, the portal's its slot 0).
fn slot_map(base: &M2Model, part: &M2Model) -> Vec<usize> {
    let idle = part
        .animations
        .as_ref()
        .and_then(|a| a.idle_seq())
        .unwrap_or(0);
    let slots = base
        .sequences
        .iter()
        .map(|s| s.seq_index + 1)
        .max()
        .unwrap_or(1);
    (0..slots)
        .map(|i| {
            base.sequences
                .iter()
                .find(|s| s.seq_index == i)
                .and_then(|s| part.sequences.iter().find(|p| p.anim_id == s.anim_id))
                .map_or(idle, |p| p.seq_index)
        })
        .collect()
}

/// A per-slot loop of the part's keyed on the base's slots.
fn reslotted<T>(
    anim: &Option<Arc<T>>,
    reslot: impl Fn(&T) -> T,
) -> Option<Arc<T>> {
    anim.as_deref().map(|a| Arc::new(reslot(a)))
}

/// `base` with `part` planted in it: each part vertex tipped by `pitch`, turned, scaled, centred
/// on the origin across with its lowest point `sink` below the ground (in the base's stand); the
/// base's body hidden if asked and its effects on moving bones raised. Given the base's inverse
/// bind poses, the part's own bones are added under the bone the base's body rides
/// ([`part_bone`]), placed as its vertices are, with its global-sequence loops (a portal's swirl)
/// and its particle emitters; the merged inverse bind poses come back beside the model. Else the
/// part is skinned whole to that bone. Its material loops play on the base's sequence slots.
/// `None` when the base has no bone to plant on.
fn planted(
    base: &M2Model,
    part: &M2Model,
    part_texture: Option<&Handle<Image>>,
    placement: &PartPlacement,
    base_ibp: Option<&[Mat4]>,
) -> Option<(M2Model, Option<Vec<Mat4>>)> {
    let p = *placement;
    let (root, unpose) = part_bone(base)?;
    // from where it stands in the stand's first frame to the bone's rest pose (WoW axes)
    let to_rest = |v: [f32; 3]| bevy_to_wow(unpose.transform_point3(wow_to_bevy(v)));
    let to_rest_normal = |n: [f32; 3]| {
        bevy_to_wow(unpose.transform_vector3(wow_to_bevy(n)).normalize_or_zero())
    };
    let (psin, pcos) = p.pitch_degrees.to_radians().sin_cos();
    let (rsin, rcos) = p.roll_degrees.to_radians().sin_cos();
    let (tsin, tcos) = p.turn_degrees.to_radians().sin_cos();
    // tip about Y, roll about X, then turn about Z
    let orient = |v: [f32; 3]| -> [f32; 3] {
        let (x, z) = (v[0] * pcos + v[2] * psin, -v[0] * psin + v[2] * pcos);
        let (y, z) = (v[1] * rcos - z * rsin, v[1] * rsin + z * rcos);
        [x * tcos - y * tsin, x * tsin + y * tcos, z]
    };
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in part
        .submeshes
        .iter()
        .flat_map(|s| s.geometry.positions.iter())
    {
        let o = orient(*v);
        for k in 0..3 {
            lo[k] = lo[k].min(o[k]);
            hi[k] = hi[k].max(o[k]);
        }
    }
    if lo[0] > hi[0] {
        return None; // a part without vertices
    }
    let foot = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, lo[2]];
    let place = |v: &[f32; 3]| -> [f32; 3] {
        let o = orient(*v);
        [
            (o[0] - foot[0]) * p.scale,
            (o[1] - foot[1]) * p.scale,
            (o[2] - foot[2]) * p.scale - p.sink,
        ]
    };
    let mut out = base.clone();
    if p.hide_base_body {
        // the solid batches, and the cards laid on the bones they ride
        let body_bones: HashSet<u16> = base
            .submeshes
            .iter()
            .filter(|s| is_solid(s))
            .flat_map(|s| s.geometry.joints.iter().map(|j| j[0]))
            .collect();
        out.submeshes.retain(|s| {
            !is_solid(s)
                && (s.billboard.is_some()
                    || !s.geometry.joints.iter().any(|j| body_bones.contains(&j[0])))
        });
    }
    // The placement as one Bevy-space affine, the part's space to the base's rest: its bones'
    // pivots and loops are carried as its vertices are.
    let placed = |v: Vec3| wow_to_bevy(to_rest(place(&bevy_to_wow(v))));
    let origin = placed(Vec3::ZERO);
    let linear = Mat3::from_cols(
        placed(Vec3::X) - origin,
        placed(Vec3::Y) - origin,
        placed(Vec3::Z) - origin,
    );
    let to_base = Affine3A::from_mat3_translation(linear, origin);
    let (_, turn, _) = to_base.to_scale_rotation_translation();
    let off = base.skeleton.joints.len();
    // The part's bones join the base's when the merged rig can be bound: the base's inverse bind
    // poses loaded, one per joint, and the indices in range.
    let rig = base_ibp.filter(|ibp| {
        ibp.len() == off
            && !part.skeleton.joints.is_empty()
            && off + part.skeleton.joints.len() <= i16::MAX as usize
    });
    let slots = slot_map(base, part);
    let base_pivots = pivots(&base.skeleton);
    let root_pivot = base_pivots
        .get(usize::from(root))
        .copied()
        .unwrap_or(Vec3::ZERO);
    let mut merged_ibp = None;
    if let Some(base_ibp) = rig {
        let new_pivots: Vec<Vec3> = pivots(&part.skeleton)
            .into_iter()
            .map(|p| to_base.transform_point3(p))
            .collect();
        for (i, j) in part.skeleton.joints.iter().enumerate() {
            // a part root hangs on the bone the base's body rides
            let (parent, parent_pivot) = match usize::try_from(j.parent).ok().filter(|&p| p < i) {
                Some(p) => ((p + off) as i16, new_pivots[p]),
                None => (root as i16, root_pivot),
            };
            out.skeleton.joints.push(ModelJoint {
                parent,
                local_translation: new_pivots[i] - parent_pivot,
                ..*j
            });
        }
        let mut ibp = base_ibp.to_vec();
        ibp.extend(new_pivots.iter().map(|p| Mat4::from_translation(-*p)));
        merged_ibp = Some(ibp);
        // Its global-sequence loops, in the placed frame: a translation's offset from rest and a
        // rotation turn with the part; a scale stays (exact for a uniform one).
        if let (Some(anims), Some(part_anims)) = (out.animations.as_mut(), part.animations.as_ref())
        {
            for g in &part_anims.global_bones {
                let i = usize::from(g.bone);
                let (Some(was), Some(now)) = (
                    part.skeleton.joints.get(i),
                    out.skeleton.joints.get(off + i),
                ) else {
                    continue;
                };
                let (was, now) = (was.local_translation, now.local_translation);
                anims.global_bones.push(GlobalBone {
                    bone: (off + i) as u16,
                    translation: g.translation.as_ref().map(|c| GlobalSeqChannel {
                        period: c.period,
                        keys: c
                            .keys
                            .iter()
                            .map(|&(t, v)| (t, now + linear * (v - was)))
                            .collect(),
                    }),
                    rotation: g.rotation.as_ref().map(|c| GlobalSeqChannel {
                        period: c.period,
                        keys: c
                            .keys
                            .iter()
                            .map(|&(t, q)| (t, turn * q * turn.inverse()))
                            .collect(),
                    }),
                    scale: g.scale.clone(),
                });
            }
        }
    }
    // it fades in and out as the base's body does, else by its own loops
    let body_alpha = base
        .submeshes
        .iter()
        .find(|s| is_solid(s))
        .and_then(|s| s.alpha_anim.clone());
    for sub in &part.submeshes {
        let mut geo: RenderSubmesh = (*sub.geometry).clone();
        geo.positions = geo.positions.iter().map(|v| to_rest(place(v))).collect();
        geo.normals = geo
            .normals
            .iter()
            .map(|n| to_rest_normal(orient(*n)))
            .collect();
        let n = geo.positions.len();
        if rig.is_some() && geo.joints.len() == n && geo.weights.len() == n {
            for j in &mut geo.joints {
                for b in j.iter_mut() {
                    *b = if usize::from(*b) < part.skeleton.joints.len() {
                        (usize::from(*b) + off) as u16
                    } else {
                        root
                    };
                }
            }
        } else {
            geo.joints = vec![[root, 0, 0, 0]; n];
            geo.weights = vec![[1.0, 0.0, 0.0, 0.0]; n];
        }
        // A skin section of the base's would read as the same triangles drawn twice.
        geo.section = None;
        geo.billboard = None;
        geo.welded_billboard = false;
        let alpha_anim = body_alpha
            .clone()
            .or_else(|| reslotted(&sub.alpha_anim, |a| a.reslot(&slots)));
        geo.alpha_anim = alpha_anim.as_deref().cloned();
        geo.uv_seq = geo.uv_seq.map(|a| a.reslot(&slots));
        geo.uv_rot_seq = geo.uv_rot_seq.map(|a| a.reslot(&slots));
        geo.uv_scale_seq = geo.uv_scale_seq.map(|a| a.reslot(&slots));
        geo.rgb_seq = geo.rgb_seq.map(|a| a.reslot(&slots));
        // A weapon's object-skin batches wear its own texture.
        let object_skin = sub.char_slot == Some(CharSkinSlot::Object);
        out.submeshes.push(ModelSubmesh {
            geometry: Arc::new(geo),
            texture: if object_skin {
                part_texture.cloned()
            } else {
                sub.texture.clone()
            },
            char_slot: if object_skin { None } else { sub.char_slot },
            billboard: None,
            ground_quad: None,
            alpha_anim,
            uv_seq: reslotted(&sub.uv_seq, |a| a.reslot(&slots)),
            uv_rot_seq: reslotted(&sub.uv_rot_seq, |a| a.reslot(&slots)),
            uv_scale_seq: reslotted(&sub.uv_scale_seq, |a| a.reslot(&slots)),
            rgb_seq: reslotted(&sub.rgb_seq, |a| a.reslot(&slots)),
            ..sub.clone()
        });
    }
    // the effects on the base's moving bones rise
    let joints = out.skeleton.joints.clone();
    for e in &mut out.emitters {
        if joints
            .get(e.def.bone as usize)
            .is_some_and(|j| j.parent >= 0)
        {
            e.def.position[2] += p.raise_effects;
        }
    }
    // the part's own effects ride its bones, placed with them, on the base's clock
    if rig.is_some() {
        let idle = base
            .animations
            .as_ref()
            .and_then(|a| a.idle_seq())
            .unwrap_or(0);
        let at = |v: [f32; 3]| bevy_to_wow(to_base.transform_point3(wow_to_bevy(v)));
        for e in &part.emitters {
            let mut e = e.clone();
            e.def.position = at(e.def.position);
            if usize::from(e.def.bone) < part.skeleton.joints.len() {
                e.def.bone += off as u16;
                e.bone_pivot = at(e.bone_pivot);
            } else {
                e.def.bone = root;
                e.bone_pivot = bevy_to_wow(root_pivot);
            }
            if let Some(b) = e.billboard.as_mut() {
                b.bone += off as u16;
                b.pivot = at(b.pivot);
            }
            e.def.timing = e.def.timing.reslot(&slots);
            e.def.params = e.def.params.reslot(&slots);
            e.idle_seq = idle;
            out.emitters.push(e);
        }
    }
    // the looks the unit wears hang on its attachment points
    for a in &mut out.attachments {
        a.offset.y += p.raise_attachments;
    }
    // The part widens the authored bounds the cull and the camera read.
    if let Some(b) = out.bounds.as_mut() {
        let top = (hi[2] - lo[2]) * p.scale - p.sink;
        let half = (hi[0] - lo[0]).max(hi[1] - lo[1]) * 0.5 * p.scale;
        b.bbox_max[2] = b.bbox_max[2].max(top);
        let reach = half.hypot(top);
        b.vert_max_from_origin = b.vert_max_from_origin.max(reach);
        b.sphere_radius = b.sphere_radius.max(reach);
    }
    Some((out, merged_ibp))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-bone-per-batch test model: each submesh's vertices on `bone`.
    fn sub(positions: Vec<[f32; 3]>, bone: u16, blend: ModelBlend) -> ModelSubmesh {
        let n = positions.len();
        let geo = RenderSubmesh {
            positions,
            joints: vec![[bone, 0, 0, 0]; n],
            weights: vec![[1.0, 0.0, 0.0, 0.0]; n],
            blend,
            ..Default::default()
        };
        ModelSubmesh {
            geometry: Arc::new(geo),
            texture: None,
            skin_slot: None,
            geoset_id: 0,
            char_slot: None,
            blend,
            two_sided: false,
            interior: false,
            emissive: false,
            icon_slot: false,
            sidn: None,
            window: false,
            additive: false,
            env_map: false,
            no_depth_write: false,
            no_depth_test: false,
            fog_policy: Default::default(),
            billboard: None,
            alpha_anim: None,
            uv_anim: None,
            uv_seq: None,
            uv_rot_seq: None,
            uv_scale_seq: None,
            rgb_anim: None,
            rgb_seq: None,
            wmo_batch: None,
            ground_quad: None,
        }
    }

    fn model(submeshes: Vec<ModelSubmesh>) -> M2Model {
        M2Model {
            submeshes,
            ..empty_model()
        }
    }

    fn empty_model() -> M2Model {
        M2Model {
            submeshes: Vec::new(),
            bounds: None,
            collision: None,
            emitters: Vec::new(),
            ribbons: Vec::new(),
            lights: Vec::new(),
            skeleton: Default::default(),
            inverse_bindposes: Handle::default(),
            animations: None,
            first_seq_span: None,
            sequences: Vec::new(),
            attachments: Vec::new(),
            markers: Vec::new(),
            portrait_camera: None,
            camera0: None,
            pane_camera: None,
            cameras: Vec::new(),
            string_anchors: None,
            cch_marker: None,
            global_flags: 0,
        }
    }

    fn joint(parent: i16, at: Vec3) -> benilla_assets::ModelJoint {
        benilla_assets::ModelJoint {
            parent,
            local_translation: at,
            billboard: None,
            parent_arm: None,
        }
    }

    fn placement() -> PartPlacement {
        PartPlacement {
            scale: 2.0,
            pitch_degrees: -90.0,
            roll_degrees: 0.0,
            turn_degrees: 0.0,
            sink: 0.5,
            raise_effects: 0.0,
            raise_attachments: 0.0,
            hide_base_body: true,
        }
    }

    /// A totem-like base: a body on bone 1 (under root 0), a glow card on bone 1, a card laid on
    /// the body's bone, and a still root 2 at the origin.
    fn base() -> M2Model {
        let body = sub(
            vec![[0.0, 0.0, 1.0], [0.0, 0.0, 2.0]],
            1,
            ModelBlend::Opaque,
        );
        let mut glow = sub(vec![[0.3, 0.0, 1.5]], 1, ModelBlend::Blend);
        glow.billboard = Some(benilla_assets::BillboardInfo {
            pivot: Vec3::ZERO,
            bone: 1,
            kind: benilla_formats::BillboardKind::Spherical,
            scale_anim: None,
            seq_translations: Vec::new(),
        });
        let rune = sub(vec![[0.1, 0.0, 1.5]], 1, ModelBlend::Blend);
        let mut m = model(vec![body, glow, rune]);
        m.skeleton.joints = vec![
            joint(-1, Vec3::new(0.0, 1.4, 0.0)),
            joint(0, Vec3::ZERO),
            joint(-1, Vec3::new(0.0, 0.001, 0.0)),
        ];
        m
    }

    #[test]
    fn a_weapon_stands_head_up_on_a_still_root_with_its_grip_in_the_ground() {
        // a weapon along +X: pommel at -0.25, head end at 1.0
        let weapon = model(vec![sub(
            vec![[-0.25, -0.1, 0.0], [1.0, 0.1, 0.0]],
            0,
            ModelBlend::Opaque,
        )]);
        let out = planted(&base(), &weapon, None, &placement(), None).map(|(m, _)| m).expect("a still root");
        let added = &out.submeshes.last().unwrap().geometry;
        let want = [[0.0, -0.2, -0.5], [0.0, 0.2, 2.0]];
        for (got, want) in added.positions.iter().zip(want) {
            assert!(
                (0..3).all(|k| (got[k] - want[k]).abs() < 1e-5),
                "{got:?} vs {want:?}"
            );
        }
        assert!(
            added.joints.iter().all(|j| j[0] == 2),
            "the still root nearest the origin, not the moving one"
        );
        // the body and the card laid on its bone are hidden, the glow card stays
        assert_eq!(out.submeshes.len(), 2);
        assert!(out.submeshes[0].billboard.is_some());
    }

    /// The looks the unit wears hang on its attachment points, which move by the placement's
    /// lift (an aura lowered round a planted weapon).
    #[test]
    fn the_attachment_points_move_by_their_lift() {
        let weapon = model(vec![sub(vec![[0.0, 0.0, 0.0]], 0, ModelBlend::Opaque)]);
        let mut with_point = base();
        with_point.attachments = vec![benilla_assets::ModelAttachment { id: 19, bone: 2, offset: Vec3::new(0.1, 0.2, 0.3) }];
        let lowered = PartPlacement { raise_attachments: -0.8, ..placement() };
        let out = planted(&with_point, &weapon, None, &lowered, None).map(|(m, _)| m).unwrap();
        assert_eq!((out.attachments[0].id, out.attachments[0].bone), (19, 2));
        assert!((out.attachments[0].offset - Vec3::new(0.1, -0.6, 0.3)).length() < 1e-6, "{:?}", out.attachments[0].offset);
    }

    #[test]
    fn a_shown_body_keeps_every_batch() {
        let weapon = model(vec![sub(vec![[0.0, 0.0, 0.0]], 0, ModelBlend::Opaque)]);
        let shown = PartPlacement {
            hide_base_body: false,
            ..placement()
        };
        let out = planted(&base(), &weapon, None, &shown, None).map(|(m, _)| m).unwrap();
        assert_eq!(
            out.submeshes.len(),
            4,
            "three of the base's, one of the part's"
        );
    }

    #[test]
    fn a_base_without_a_still_root_takes_no_part() {
        let mut no_root = base();
        no_root.skeleton.joints.clear();
        let weapon = model(vec![sub(vec![[0.0, 0.0, 0.0]], 0, ModelBlend::Opaque)]);
        assert!(planted(&no_root, &weapon, None, &placement(), None).map(|(m, _)| m).is_none());
    }

    fn seq(anim_id: u16, seq_index: usize) -> benilla_assets::M2SequenceInfo {
        benilla_assets::M2SequenceInfo {
            anim_id,
            seq_index,
            duration_ms: 1000,
            start_ms: 0,
            looping: true,
        }
    }

    fn no_clips() -> benilla_assets::ModelAnimations {
        benilla_assets::ModelAnimations {
            graph: Handle::default(),
            clips: Vec::new(),
            hand_close: [None, None],
            playable_animation_lookup: Vec::new(),
            animation_lookup: Vec::new(),
            global_bones: Vec::new(),
            first_seq: None,
            pose: Arc::new(benilla_assets::PoseSource {
                clips: Vec::new(),
                nodes: Vec::new(),
                bone_masks: Vec::new(),
            }),
        }
    }

    /// A portal-like part: a root and a swirl bone one yard up (Bevy +Y), the swirl's vertices
    /// on it, a global-sequence nudge on it, and an emitter on it.
    fn swirl() -> M2Model {
        let mut m = model(vec![sub(
            vec![[-1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [0.0, 0.0, 2.0]],
            1,
            ModelBlend::Blend,
        )]);
        m.skeleton.joints = vec![joint(-1, Vec3::ZERO), joint(0, Vec3::Y)];
        let mut anims = no_clips();
        anims.global_bones = vec![GlobalBone {
            bone: 1,
            translation: Some(GlobalSeqChannel {
                period: 1.0,
                keys: vec![(0.0, Vec3::new(1.0, 1.0, 0.0))],
            }),
            rotation: None,
            scale: None,
        }];
        m.animations = Some(anims);
        m.emitters = vec![benilla_assets::ModelEmitter {
            def: benilla_formats::ParticleEmitterDef {
                bone: 1,
                position: [0.5, 0.0, 1.0],
                ..benilla_world::testing::plain_particle_def()
            },
            texture: None,
            bone_pivot: [0.0, 0.0, 1.0],
            billboard: None,
            recursion: None,
            geometry: None,
            owner_reach: 0.0,
            water_bound: (Vec3::ZERO, 0.0),
            idle_seq: 0,
        }];
        m.sequences = vec![seq(7, 0), seq(0, 1)];
        m
    }

    fn upright() -> PartPlacement {
        PartPlacement {
            scale: 2.0,
            pitch_degrees: 0.0,
            sink: 0.0,
            ..placement()
        }
    }

    /// The base's rest inverse bind poses, `translate(-pivot)`.
    fn rest_ibp(m: &M2Model) -> Vec<Mat4> {
        pivots(&m.skeleton)
            .into_iter()
            .map(|p| Mat4::from_translation(-p))
            .collect()
    }

    /// A portal planted in an invisible stalker keeps its swirl: its bones join the base's under
    /// the still root, placed as its vertices are, with their loop and their emitter.
    #[test]
    fn a_part_with_bones_brings_them_their_loops_and_its_effects() {
        let mut host = base();
        host.animations = Some(no_clips());
        let (out, ibp) = planted(&host, &swirl(), None, &upright(), Some(&rest_ibp(&host)))
            .expect("a still root");
        let ibp = ibp.expect("the rig is merged");
        assert_eq!(out.skeleton.joints.len(), 5, "three of the base's, two of the part's");
        assert_eq!(ibp.len(), 5);
        // the part's root hangs on the still root 2, its swirl bone two yards up (scale 2)
        assert_eq!(out.skeleton.joints[3].parent, 2);
        assert_eq!(out.skeleton.joints[4].parent, 3);
        let at = pivots(&out.skeleton);
        assert!((at[4] - Vec3::new(0.0, 2.0, 0.0)).length() < 1e-5, "{:?}", at[4]);
        assert!((ibp[4].transform_point3(at[4])).length() < 1e-5, "rest skins to identity");
        // the vertices ride the swirl bone
        let added = &out.submeshes.last().unwrap().geometry;
        assert!(added.joints.iter().all(|j| j[0] == 4), "{:?}", added.joints);
        // the loop's one-yard nudge becomes two, from the placed rest
        let g = out.animations.as_ref().unwrap().global_bones.last().expect("the swirl's loop");
        assert_eq!(g.bone, 4);
        let key = g.translation.as_ref().unwrap().keys[0].1;
        assert!((key - Vec3::new(2.0, 2.0, 0.0)).length() < 1e-5, "{key:?}");
        // the emitter rides the swirl bone at its placed spot
        let e = out.emitters.last().expect("the part's emitter");
        assert_eq!(e.def.bone, 4);
        assert_eq!(e.bone_pivot, [0.0, 0.0, 2.0]);
        assert_eq!(e.def.position, [1.0, 0.0, 2.0]);
    }

    /// Without the base's bind poses (or with a boneless part), the part rides one bone whole.
    #[test]
    fn without_the_bases_bind_poses_the_part_rides_one_bone() {
        let (out, ibp) = planted(&base(), &swirl(), None, &upright(), None).unwrap();
        assert!(ibp.is_none());
        assert_eq!(out.skeleton.joints.len(), 3);
        assert!(out.emitters.is_empty());
        let added = &out.submeshes.last().unwrap().geometry;
        assert!(added.joints.iter().all(|j| j[0] == 2));
    }

    /// Violet turns gold, grey and green stay, and a blue at the band's feathered edge turns part
    /// way.
    #[test]
    fn a_hue_shift_turns_the_band_and_keeps_grey() {
        let gold = HueShift {
            from: (225.0, 330.0),
            to: (28.0, 52.0),
            feather: 20.0,
            min_saturation: 0.4,
            brighten: 1.0,
        };
        let violet = gold.shift_rgb([160.0, 40.0, 220.0]);
        assert!(violet[0] > violet[1] && violet[1] > violet[2], "{violet:?}");
        assert!((violet[0] - 220.0).abs() < 1.0, "keeps its brightness: {violet:?}");
        assert_eq!(gold.shift_rgb([120.0, 118.0, 125.0]), [120.0, 118.0, 125.0], "grey stone");
        assert_eq!(gold.shift_rgb([30.0, 200.0, 40.0]), [30.0, 200.0, 40.0], "green");
        let edge = gold.shift_rgb([0.0, 85.0, 255.0]); // hue 220, half into the feather
        assert!(edge[0] > 0.0 && edge[2] < 255.0, "{edge:?}");
        let mut px = [160u8, 40, 220, 77];
        gold.shift_rgba8(&mut px);
        assert_eq!(px[3], 77, "alpha untouched");
    }

    /// The base's Stand plays the part's Stand whatever slots they sit in; a slot the part lacks
    /// plays the part's idle.
    #[test]
    fn the_parts_loops_play_on_the_bases_slots_by_animation() {
        let mut host = base();
        host.sequences = vec![seq(5, 0), seq(5, 1), seq(0, 2)];
        assert_eq!(slot_map(&host, &swirl()), vec![0, 0, 1]);
        assert_eq!(slot_map(&base(), &swirl()), vec![0], "no sequences: one slot, the idle");
    }

    #[test]
    fn the_part_rides_the_bone_the_body_rides_once_the_base_has_a_stand() {
        assert_eq!(body_bone(&base()), Some(1), "the opaque body's bone, not a card's");
        // without a stand to pose the body bone in, the part falls back to the still root
        assert_eq!(part_bone(&base()).map(|(bone, _)| bone), Some(2));
    }
}
