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
use benilla_assets::{m2_url, skin_url, M2Model, ModelSubmesh};
use benilla_formats::{CharSkinSlot, ModelBlend, RenderSubmesh};
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
    /// Its turn about the vertical axis, degrees, counter-clockwise seen from above.
    pub turn_degrees: f32,
    /// How deep its foot (its lowest point once stood) goes below the base's ground, the model's
    /// origin; the part is centred on the origin across.
    pub sink: f32,
    /// How far the base's particle effects on its moving bones (any bone with a parent) rise.
    pub raise_effects: f32,
    /// Hide the base's solid body (and the cards laid on it), keeping its effects: glow cards,
    /// particle emitters and their bones' animation.
    pub hide_base_body: bool,
}

/// The displays a game composes; empty in stock benilla. Insert it before the client runs
/// (`benilla_app::run_with`'s extend).
#[derive(Resource, Clone, Debug, Default)]
pub struct ComposedDisplays(pub Vec<ComposedDisplay>);

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
        .init_resource::<ComposeSources>()
        // Before the frame: the aliases once, then any composed model whose sources have landed.
        .add_systems(
            PreUpdate,
            (alias_composed_displays, compose_ready_displays).chain(),
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
        let model = match planted(base, part, p.part_texture.as_ref(), &c.placement) {
            Some(model) => {
                info!(
                    "composed display {}: {:?} planted in display {}",
                    c.id, c.part, c.base
                );
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

/// `base` with `part` planted in it: each part vertex tipped by `pitch`, turned, scaled, centred
/// on the origin across with its lowest point `sink` below the ground (in the base's stand), and
/// skinned whole to the bone the base's body rides ([`part_bone`]); the base's body hidden if
/// asked and its effects on moving bones raised. `None` when the base has no bone to plant on.
fn planted(
    base: &M2Model,
    part: &M2Model,
    part_texture: Option<&Handle<Image>>,
    placement: &PartPlacement,
) -> Option<M2Model> {
    let p = *placement;
    let (root, unpose) = part_bone(base)?;
    // from where it stands in the stand's first frame to the bone's rest pose (WoW axes)
    let to_rest = |v: [f32; 3]| bevy_to_wow(unpose.transform_point3(wow_to_bevy(v)));
    let to_rest_normal = |n: [f32; 3]| {
        bevy_to_wow(unpose.transform_vector3(wow_to_bevy(n)).normalize_or_zero())
    };
    let (psin, pcos) = p.pitch_degrees.to_radians().sin_cos();
    let (tsin, tcos) = p.turn_degrees.to_radians().sin_cos();
    // tip about Y, then turn about Z
    let orient = |v: [f32; 3]| -> [f32; 3] {
        let (x, z) = (v[0] * pcos + v[2] * psin, -v[0] * psin + v[2] * pcos);
        let y = v[1];
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
    // it fades in and out as the base's body does
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
        geo.joints = vec![[root, 0, 0, 0]; n];
        geo.weights = vec![[1.0, 0.0, 0.0, 0.0]; n];
        // A skin section of the base's would read as the same triangles drawn twice.
        geo.section = None;
        geo.billboard = None;
        geo.welded_billboard = false;
        geo.alpha_anim = body_alpha.as_deref().cloned();
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
            alpha_anim: body_alpha.clone(),
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
    // The part widens the authored bounds the cull and the camera read.
    if let Some(b) = out.bounds.as_mut() {
        let top = (hi[2] - lo[2]) * p.scale - p.sink;
        let half = (hi[0] - lo[0]).max(hi[1] - lo[1]) * 0.5 * p.scale;
        b.bbox_max[2] = b.bbox_max[2].max(top);
        let reach = half.hypot(top);
        b.vert_max_from_origin = b.vert_max_from_origin.max(reach);
        b.sphere_radius = b.sphere_radius.max(reach);
    }
    Some(out)
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
            turn_degrees: 0.0,
            sink: 0.5,
            raise_effects: 0.0,
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
        let out = planted(&base(), &weapon, None, &placement()).expect("a still root");
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

    #[test]
    fn a_shown_body_keeps_every_batch() {
        let weapon = model(vec![sub(vec![[0.0, 0.0, 0.0]], 0, ModelBlend::Opaque)]);
        let shown = PartPlacement {
            hide_base_body: false,
            ..placement()
        };
        let out = planted(&base(), &weapon, None, &shown).unwrap();
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
        assert!(planted(&no_root, &weapon, None, &placement()).is_none());
    }

    #[test]
    fn the_part_rides_the_bone_the_body_rides_once_the_base_has_a_stand() {
        assert_eq!(body_bone(&base()), Some(1), "the opaque body's bone, not a card's");
        // without a stand to pose the body bone in, the part falls back to the still root
        assert_eq!(part_bone(&base()).map(|(bone, _)| bone), Some(2));
    }
}
