//! The models some looks on the ground are drawn as, handed to
//! benilla's `GroundLookModels`: the Holy Orb's dynamic object as a glowing ball of holy light from
//! the player's own client ([`esurv_content::option_looks::HOLY_ORB_MODELS`]), and Slam's
//! burst at each enemy's feet as one of [`esurv_content::option_looks::SLAM_BURSTS`], played
//! once. Dev aids, for comparing looks: `ESURV_HOLY_ORB_MODEL=<M2 path>[,<scale>[,<lift>]]`
//! draws another model for the Holy Orb, `ESURV_SLAM_BURST=<name or number>[,<scale>]` draws
//! another of Slam's bursts (`quake`, or `2`: numbered from 1; `quake,0.5` at another size), and
//! `ESURV_ORB_SHOTS=<png>` writes close shots of the first Holy Orb lying on the ground
//! ([`OrbShots`]).

use benilla_app::{GroundLookModel, GroundLookModels};
use bevy::prelude::*;
use esurv_content::option_looks::{LookModel, HOLY_ORB_LOOK, HOLY_ORB_MODELS, MARK_MODELS, SLAM_BURSTS, SLAM_BURST_LOOK};

/// The switch that draws another model for the Holy Orb.
const HOLY_ORB_MODEL_SWITCH: &str = "ESURV_HOLY_ORB_MODEL";
/// The switch that draws another of Slam's bursts.
const SLAM_BURST_SWITCH: &str = "ESURV_SLAM_BURST";

pub(crate) fn register(app: &mut App) {
    let holy = std::env::var(HOLY_ORB_MODEL_SWITCH).ok();
    let slam = std::env::var(SLAM_BURST_SWITCH).ok();
    app.insert_resource(ground_models(holy.as_deref(), slam.as_deref()));
    if let Some(shots) = OrbShots::from_env() {
        app.insert_resource(shots)
            .add_systems(Update, shoot_an_orb)
            .add_systems(
                PostUpdate,
                frame_the_orb.before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

/// The shots' camera: this far from the orb across the ground, this high over the ground, looking
/// at this height (yards), turned this far off the player camera's own bearing.
const SHOT_DISTANCE: f32 = 3.0;
const SHOT_EYE_HEIGHT: f32 = 1.6;
const SHOT_LOOK_HEIGHT: f32 = 0.5;
const SHOT_BEARING_TURN_DEGREES: f32 = 60.0;

/// `ESURV_ORB_SHOTS=<png>`: once the hero has been in the world `ESURV_ORB_SHOTS_AFTER`
/// seconds (default 12) and a Holy Orb lies on the ground, the world camera is held still close to
/// it and `ESURV_ORB_SHOTS_COUNT` shots (default 2) `<stem>-000.png` ... are written,
/// `ESURV_ORB_SHOTS_EVERY` seconds apart (default 0.5), with nobody at the mouse.
/// `ESURV_ORB_SHOTS_LOOK=<spell id>` frames the first look of that spell on the ground
/// instead (`26093`: Slam's burst).
#[derive(Resource)]
struct OrbShots {
    out: String,
    /// The look framed: its dynamic object's spell.
    look: u32,
    count: u32,
    after: f32,
    every: f32,
    taken: u32,
    /// When the hero entered the world, on the app clock.
    entered_at: Option<f32>,
    /// When the camera was first held on an orb, and the held pose (eye, target).
    framed: Option<(f32, Vec3, Vec3)>,
}

impl OrbShots {
    fn from_env() -> Option<Self> {
        let out = std::env::var("ESURV_ORB_SHOTS").ok()?;
        let num = |key: &str, default: f32| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        Some(Self {
            out,
            look: num("ESURV_ORB_SHOTS_LOOK", HOLY_ORB_LOOK as f32) as u32,
            count: num("ESURV_ORB_SHOTS_COUNT", 2.0).max(1.0) as u32,
            after: num("ESURV_ORB_SHOTS_AFTER", 12.0),
            every: num("ESURV_ORB_SHOTS_EVERY", 0.5),
            taken: 0,
            entered_at: None,
            framed: None,
        })
    }

    /// Shot `i`'s file: `<stem>-00i.<ext>`.
    fn path(&self, i: u32) -> String {
        match self.out.rsplit_once('.') {
            Some((stem, ext)) => format!("{stem}-{i:03}.{ext}"),
            None => format!("{}-{i:03}", self.out),
        }
    }
}

/// The look of spell `look` nearest the hero, if one lies on the ground (its object's entry is
/// its look's spell).
fn nearest_orb(
    look: u32,
    objects: &benilla_app::Objects,
    places: &Query<&GlobalTransform, Without<benilla_world::view::WorldCamera>>,
) -> Option<Vec3> {
    let hero = places.get(objects.player()?).ok()?.translation();
    objects
        .iter()
        .filter(|(_, _, f)| f.object_entry() == Some(look))
        .filter_map(|(_, e, _)| places.get(e).ok().map(|t| t.translation()))
        // one just made is not placed yet (it stands at the origin for a frame)
        .filter(|at| *at != Vec3::ZERO)
        .min_by(|a, b| a.distance(hero).total_cmp(&b.distance(hero)))
}

fn shoot_an_orb(
    mut commands: Commands,
    mut shots: ResMut<OrbShots>,
    objects: benilla_app::Objects,
    time: Res<Time>,
) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    let now = time.elapsed_secs();
    if shots.entered_at.is_none() && objects.player().is_some() {
        shots.entered_at = Some(now);
    }
    // a shot is due a second after the camera was held on an orb, then every `every` seconds
    let Some((held_at, _, _)) = shots.framed else {
        return;
    };
    if shots.taken >= shots.count || now < held_at + 1.0 + shots.every * shots.taken as f32 {
        return;
    }
    let path = shots.path(shots.taken);
    info!("orb shots: writing {path}");
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
    shots.taken += 1;
}

/// Holds the world camera on the first Holy Orb lying once the wait is over.
fn frame_the_orb(
    mut shots: ResMut<OrbShots>,
    objects: benilla_app::Objects,
    places: Query<&GlobalTransform, Without<benilla_world::view::WorldCamera>>,
    mut camera: Query<&mut Transform, With<benilla_world::view::WorldCamera>>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    let Some(entered) = shots.entered_at else {
        return;
    };
    if now < entered + shots.after {
        return;
    }
    let Ok(mut cam) = camera.single_mut() else {
        return;
    };
    if shots.framed.is_none() {
        let Some(orb) = nearest_orb(shots.look, &objects, &places) else {
            return; // none lies yet: hold off
        };
        let away = (cam.translation - orb).with_y(0.0).normalize_or(Vec3::Z);
        let bearing = Quat::from_rotation_y(SHOT_BEARING_TURN_DEGREES.to_radians()) * away;
        let eye = orb + bearing * SHOT_DISTANCE + Vec3::Y * SHOT_EYE_HEIGHT;
        let target = orb + Vec3::Y * SHOT_LOOK_HEIGHT;
        info!("orb shots: camera held on the orb at {orb:?}");
        shots.framed = Some((now, eye, target));
    }
    if let Some((_, eye, target)) = shots.framed {
        *cam = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}

/// The models: the Holy Orb's from `holy` when it names one, Slam's burst the one `slam` names.
fn ground_models(holy: Option<&str>, slam: Option<&str>) -> GroundLookModels {
    let holy = match holy.and_then(model_of_switch) {
        Some(m) => vec![m],
        None => HOLY_ORB_MODELS.iter().map(|m| model(m, false)).collect(),
    };
    let slam = vec![slam_burst(slam)];
    // the boss marks: Kel'Thuzad's Shadow Fissure over a purple glow on each disc
    let marks = MARK_MODELS.iter().map(|(spell, looks)| {
        (*spell, looks.iter().map(|m| GroundLookModel { recolor: m.recolor, ..model(&m.look, false) }).collect())
    });
    GroundLookModels([(HOLY_ORB_LOOK, holy), (SLAM_BURST_LOOK, slam)].into_iter().chain(marks).collect())
}

/// The Slam burst `switch` names, `<name or number from 1>[,<scale>]`; else the first at its own
/// size.
fn slam_burst(switch: Option<&str>) -> GroundLookModel {
    let mut parts = switch.unwrap_or("").split(',').map(str::trim);
    let name = parts.next().unwrap_or("");
    let by_number = name.parse::<usize>().ok().and_then(|n| n.checked_sub(1));
    let pick = by_number.or_else(|| SLAM_BURSTS.iter().position(|(n, _)| n.eq_ignore_ascii_case(name)));
    let mut burst = model(&SLAM_BURSTS.get(pick.unwrap_or(0)).unwrap_or(&SLAM_BURSTS[0]).1, true);
    if let Some(scale) = parts.next().and_then(|s| s.parse().ok()) {
        burst.scale = scale;
    }
    burst
}

fn model(look: &LookModel, once: bool) -> GroundLookModel {
    GroundLookModel {
        path: look.path.into(),
        scale: look.scale,
        lift: look.lift,
        tint: look.tint,
        once,
        recolor: false,
    }
}

/// Reads `<path>[,<scale>[,<lift>]]`; the size and lift left out are the Holy Orb glow's own.
fn model_of_switch(text: &str) -> Option<GroundLookModel> {
    let mut parts = text.split(',').map(str::trim);
    let path = parts.next().filter(|p| !p.is_empty())?;
    let glow = &HOLY_ORB_MODELS[0];
    let scale = parts.next().and_then(|s| s.parse().ok()).unwrap_or(glow.scale);
    let lift = parts.next().and_then(|s| s.parse().ok()).unwrap_or(glow.lift);
    Some(GroundLookModel {
        path: path.into(),
        scale,
        lift,
        tint: None,
        once: false,
        recolor: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The boss marks: the warning a glow in the Shadow Fissure's own purple under the fissure's
    /// sparks (in their own colours), the fill a brighter violet glow, each at 70 % of its disc (so
    /// every shape's discs still trace it, smaller than the Consecration marks were); the paladin's
    /// Consecration and Sanctified Ground's look are drawn as their spells' own.
    #[test]
    fn the_boss_marks_are_drawn_as_shadow_fissures_over_a_purple_glow() {
        use esurv_content::option_looks::*;
        let plain = ground_models(None, None);
        let warning = &plain.0[&MARK_WARNING_LOOK];
        let fill = &plain.0[&MARK_FILL_LOOK];
        assert_eq!((warning.len(), fill.len()), (2, 1));
        assert_eq!((warning[0].path.as_str(), warning[0].tint, warning[0].recolor), (MARK_GROUND_MODEL, Some(MARK_PURPLE), true));
        assert_eq!((warning[1].path.as_str(), warning[1].tint, warning[1].recolor), (SHADOW_FISSURE_MODEL, None, false), "the fissure in its own purple");
        assert_eq!((fill[0].path.as_str(), fill[0].tint, fill[0].recolor), (MARK_GROUND_MODEL, Some(MARK_FILL_VIOLET), true));
        // the glow (11.5 yd round) at 70 % of the disc, the fissure at its own size, scaled by r / 8
        assert!((warning[0].scale * 11.5 / 8.0 - 0.7).abs() < 0.01 && warning[1].scale == 1.0);
        assert!(warning.iter().chain(fill).all(|m| !m.once), "lasting while the mark winds up");
        assert!(!plain.0.contains_key(&26_573) && !plain.0.contains_key(&11_433), "the paladin's and the legendary looks unchanged");
    }

    #[test]
    fn the_holy_orb_is_drawn_as_its_models_unless_the_switch_names_another() {
        let plain = ground_models(None, None);
        let holy = &plain.0[&HOLY_ORB_LOOK];
        assert_eq!(holy.len(), HOLY_ORB_MODELS.len());
        assert_eq!(holy[0].path, HOLY_ORB_MODELS[0].path);
        // the holy bolt's ball inside the glow: half its size, dimmed
        assert_eq!((holy[1].scale, holy[1].tint), (0.5, Some([0.55; 3])));
        assert!(holy.iter().all(|m| !m.once), "the orb lasts");
        let other = ground_models(Some("Spells\\Arcane_Missile.mdx, 0.5"), None);
        let m = &other.0[&HOLY_ORB_LOOK];
        assert_eq!(m.len(), 1);
        assert_eq!((m[0].path.as_str(), m[0].scale, m[0].lift), ("Spells\\Arcane_Missile.mdx", 0.5, HOLY_ORB_MODELS[0].lift));
        assert_eq!(ground_models(Some(""), None).0[&HOLY_ORB_LOOK].len(), HOLY_ORB_MODELS.len());
    }

    #[test]
    fn slams_burst_is_the_first_look_unless_the_switch_names_another() {
        let burst = |s| ground_models(None, s).0[&SLAM_BURST_LOOK][0].clone();
        assert_eq!(burst(None).path, SLAM_BURSTS[0].1.path);
        assert!(burst(None).once, "a burst plays once");
        assert_eq!((SLAM_BURSTS[0].0, burst(None).scale), ("quake", 0.3), "Quake's burst at 0.3 by default");
        assert_eq!(burst(Some("dust_field")).path, SLAM_BURSTS[1].1.path);
        assert_eq!(burst(Some("3")).path, SLAM_BURSTS[2].1.path);
        assert_eq!(burst(Some("nothing")).path, SLAM_BURSTS[0].1.path);
        assert_eq!(burst(Some("9")).path, SLAM_BURSTS[0].1.path);
        let grown = burst(Some("dust_field, 1"));
        assert_eq!((grown.path.as_str(), grown.scale), (SLAM_BURSTS[1].1.path, 1.0));
    }
}
