//! The engine-drawn bowstring: bow M2s have no string geometry. The reference's per-frame callback
//! (`0x611ff0`) draws a two-segment line between the `$WTT`/`$WTB` limb-tip markers, its middle
//! vertex at the HandArrow attach while the nock latch (`[+0xd58] & 0x4000`) holds, else the tip
//! midpoint. Drawn here as gizmo lines, light grey ([`STRING_COLOR`]); the width is inferred, as
//! the reference's packed vertex colour is not decoded.
//!
//! The tips are posed: `$BWP` plays BowPull (160) on the prop and `$BWR` returns it to Stand (0),
//! so the markers ride limb bones that bend through the draw.

use bevy::prelude::*;

use crate::creature_anim::NockLatch;
use crate::entities::BoneAttach;

/// The string's colour, a light grey (a bowstring reads white or grey,
/// not black); the reference's packed vertex colour is not decoded.
pub(crate) const STRING_COLOR: Color = Color::srgb(0.80, 0.78, 0.74);

/// The HandArrow attach id (35, `0x6121b8`): the string's middle point while nocked.
const HAND_ARROW: u16 = 0x23;

/// Marks a bow prop root whose model authors the `$WTT`/`$WTB` anchors; despawned with the prop.
#[derive(Component)]
pub(crate) struct Bowstring {
    /// The unit wearing the bow, which carries the HandArrow attach and the nock latch.
    pub(crate) owner: Entity,
    /// The `$WTT` top and `$WTB` bottom anchors as `(bone, model-local offset)`.
    pub(crate) top: (u16, Vec3),
    pub(crate) bottom: (u16, Vec3),
    /// The top and bottom bones' bind pivots: a posed bone frame carries a point as its offset
    /// from the pivot (the prop's palette rows are `model[b] × translate(−pivot)`), so a flexed
    /// tip is `model[b] × (point − pivot)`.
    pub(crate) pivots: [Vec3; 2],
}

/// Bone `bone`'s bind pivot, Bevy model space: the rest translations summed up its parent chain.
pub(crate) fn bind_pivot(skeleton: &benilla_assets::ModelSkeleton, bone: u16) -> Vec3 {
    let mut pivot = Vec3::ZERO;
    let mut at = i32::from(bone);
    // Parent-sorted, so the walk ends; the bound stops a malformed loop.
    for _ in 0..skeleton.joints.len() {
        let Some(joint) = usize::try_from(at).ok().and_then(|i| skeleton.joints.get(i)) else {
            break;
        };
        pivot += joint.local_translation;
        at = i32::from(joint.parent);
    }
    pivot
}

/// Draws every visible bow's string, tip to middle to tip, from this frame's joint frames.
fn draw_bowstrings(
    bows: Query<(
        &Bowstring,
        &GlobalTransform,
        &InheritedVisibility,
        Option<&benilla_world::rig_anim::RigPose>,
    )>,
    owners: Query<(
        &BoneAttach,
        &benilla_world::rig_anim::RigPose,
        Has<NockLatch>,
    )>,
    joints: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
) {
    for (bs, prop, vis, flex) in &bows {
        if !vis.get() {
            continue;
        }
        // The prop's own pose when it flexes (`joints_root` is this entity); its rigid frame
        // otherwise.
        let tip = |(bone, offset): (u16, Vec3), pivot: Vec3| {
            flex.and_then(|p| p.posed_point(prop, bone, offset - pivot))
                .unwrap_or_else(|| prop.transform_point(offset))
        };
        let top = tip(bs.top, bs.pivots[0]);
        let bottom = tip(bs.bottom, bs.pivots[1]);
        let middle = owners
            .get(bs.owner)
            .ok()
            .filter(|(_, _, latched)| *latched)
            .and_then(|(bones, pose, _)| {
                let &(bone, offset) = bones.points.get(&HAND_ARROW)?;
                pose.posed_point(joints.get(pose.joints_root).ok()?, bone, offset)
            })
            .unwrap_or_else(|| (top + bottom) / 2.0);
        gizmos.line(top, middle, STRING_COLOR);
        gizmos.line(middle, bottom, STRING_COLOR);
    }
}

/// Registers the string drawer after the palette pass, beside the other joint-frame readers.
pub(crate) struct BowstringPlugin;

impl Plugin for BowstringPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            draw_bowstrings.in_set(benilla_world::billboard::BillboardPlace),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The string is drawn light grey, never near black.
    #[test]
    fn the_string_is_light_grey() {
        let c = STRING_COLOR.to_srgba();
        assert!(c.red.min(c.green).min(c.blue) >= 0.6, "{c:?}");
        assert!(c.red.max(c.green).max(c.blue) - c.red.min(c.green).min(c.blue) < 0.1, "a grey: {c:?}");
    }

    /// A tip rides its bone as its offset from the bone's bind pivot: the pivot is the rest
    /// translations summed up the parent chain.
    #[test]
    fn a_bones_bind_pivot_sums_its_chain() {
        let joint = |parent: i16, t: Vec3| benilla_assets::ModelJoint {
            parent,
            local_translation: t,
            billboard: None,
            parent_arm: None,
        };
        let skeleton = benilla_assets::ModelSkeleton {
            joints: vec![joint(-1, Vec3::X), joint(0, Vec3::Y), joint(1, Vec3::Z), joint(0, Vec3::NEG_Y)],
            spine_bone: None,
            head_bone: None,
        };
        assert_eq!(bind_pivot(&skeleton, 2), Vec3::new(1.0, 1.0, 1.0));
        assert_eq!(bind_pivot(&skeleton, 3), Vec3::new(1.0, -1.0, 0.0));
        assert_eq!(bind_pivot(&skeleton, 9), Vec3::ZERO, "no such bone");
        // at bind pose a tip taken off its pivot lands on its own model-space point, not a
        // pivot's length past it
        let pose = benilla_world::rig_anim::RigPose::new(Entity::PLACEHOLDER, &skeleton);
        let point = Vec3::new(0.5, 2.0, 1.5);
        let at = pose.posed_point(&GlobalTransform::IDENTITY, 2, point - bind_pivot(&skeleton, 2)).unwrap();
        assert!(at.distance(point) < 1e-5, "{at}");
    }
}
