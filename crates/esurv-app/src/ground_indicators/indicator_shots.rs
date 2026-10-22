//! `ESURV_INDICATOR_SHOTS=<png>`, a development aid: a window shot of each kind of ground
//! indicator as it first shows (a circle, ring, cone or line winding up; a lasting zone's
//! outline, still or moving; a sweeping beam), `<stem>-<kind>.png`, the world camera held over it
//! looking down from a second before, with nobody at the mouse. A mark is shot half wound up, an
//! outline a second after it shows. `ESURV_INDICATOR_SHOTS_MAX` stops after that many (default 12).

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use benilla_world::view::WorldCamera;
use esurv_protocol::IndicatorShape;

use super::{indicator_now, paint, Indicators};

/// The camera over an indicator: back from it and up, by these shares of its reach (and at
/// least this far), looking at a point this share of its reach along its facing.
const BACK: f32 = 0.55;
const UP: f32 = 1.05;
const LEAST: f32 = 9.0;
const AHEAD: f32 = 0.45;
/// An outline is shot this long after it is framed, a mark once this far wound up.
const OUTLINE_AFTER: f32 = 1.0;
const MARK_SWEPT: f32 = 0.5;

pub(super) fn register(app: &mut App) {
    if let Ok(out) = std::env::var("ESURV_INDICATOR_SHOTS") {
        let most = std::env::var("ESURV_INDICATOR_SHOTS_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(12);
        app.insert_resource(IndicatorShots { out, most, taken: Vec::new(), framing: None, after_shot: None })
            .add_systems(Update, shoot_indicators.after(super::place_indicators))
            .add_systems(PostUpdate, hold_the_camera.before(bevy::transform::TransformSystems::Propagate));
    }
}

#[derive(Resource)]
struct IndicatorShots {
    out: String,
    most: usize,
    /// The kinds shot so far.
    taken: Vec<String>,
    /// The indicator framed: its number, its kind, when it was framed, and the camera's pose.
    framing: Option<(u32, String, f32, Vec3, Vec3)>,
    /// The pose held a moment after a shot (the shot renders the frame it was asked in), and
    /// until when.
    after_shot: Option<(Vec3, Vec3, f32)>,
}

/// An indicator's kind, as its shot is named.
fn kind(shape: IndicatorShape, edge_only: bool, moving: bool, turning: bool) -> String {
    let shape = match shape {
        IndicatorShape::Circle { .. } => "circle",
        IndicatorShape::Ring { .. } => "ring",
        IndicatorShape::Cone { .. } => "cone",
        IndicatorShape::Line { .. } => "line",
    };
    match (edge_only, moving, turning) {
        (_, _, true) => "beam".into(),
        (true, true, _) => format!("{shape}-zone-moving"),
        (true, false, _) => format!("{shape}-zone"),
        _ => shape.into(),
    }
}

fn shoot_indicators(mut commands: Commands, mut shots: ResMut<IndicatorShots>, shown: Res<Indicators>, camera: Query<&Transform, With<WorldCamera>>, time: Res<Time>) {
    let now = time.elapsed_secs();
    if shots.taken.len() >= shots.most {
        shots.framing = None;
        return;
    }
    if let Some((id, k, from, _, _)) = shots.framing.clone() {
        let Some(s) = shown.0.get(&id) else {
            shots.framing = None; // it went before its shot
            return;
        };
        let at = indicator_now(&s.told, now - s.since);
        let due = now >= from + 1.0 && at.swept.is_none_or(|p| p >= MARK_SWEPT) && (at.swept.is_some() || now >= from + OUTLINE_AFTER.max(1.0));
        if due {
            let path = match shots.out.rsplit_once('.') {
                Some((stem, ext)) => format!("{stem}-{k}.{ext}"),
                None => format!("{}-{k}.png", shots.out),
            };
            info!("indicator shots: writing {path}");
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            if let Some((_, _, _, eye, target)) = shots.framing { shots.after_shot = Some((eye, target, now + 0.5)); }
            shots.taken.push(k);
            shots.framing = None;
        }
        return;
    }
    let Ok(cam) = camera.single() else { return };
    for (id, s) in &shown.0 {
        let moving = s.told.velocity[0].abs() + s.told.velocity[1].abs() > 0.05;
        let k = kind(s.told.shape, s.told.edge_only, moving, s.told.turn.abs() > 1e-4);
        if shots.taken.contains(&k) || s.verts.is_empty() {
            continue;
        }
        let at = indicator_now(&s.told, now - s.since);
        // a mark too far on to be framed and shot half wound up waits for the next
        if at.swept.is_some_and(|p| p > 0.2) {
            continue;
        }
        let reach = paint::reach(s.told.shape);
        let centre = s.centre;
        let (fx, fz) = (-at.facing.sin(), -at.facing.cos());
        let ahead = match s.told.shape {
            IndicatorShape::Cone { .. } | IndicatorShape::Line { .. } => Vec3::new(fx, 0.0, fz) * reach * AHEAD,
            _ => Vec3::ZERO,
        };
        let target = centre + ahead;
        let away = (cam.translation - target).with_y(0.0).normalize_or(Vec3::Z);
        let eye = target + away * (reach * BACK).max(LEAST * BACK) + Vec3::Y * (reach * UP).max(LEAST);
        info!("indicator shots: framing {k} at {target:?}");
        shots.framing = Some((*id, k, now, eye, target));
        break;
    }
}

fn hold_the_camera(shots: Res<IndicatorShots>, mut camera: Query<&mut Transform, With<WorldCamera>>, time: Res<Time>) {
    let (eye, target) = match (&shots.framing, shots.after_shot) {
        (Some((_, _, _, eye, target)), _) => (*eye, *target),
        (None, Some((eye, target, until))) if time.elapsed_secs() < until => (eye, target),
        _ => return,
    };
    if let Ok(mut cam) = camera.single_mut() {
        *cam = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}
