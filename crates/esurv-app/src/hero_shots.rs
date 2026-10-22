//! `ESURV_HERO_SHOTS=<png>`, a development aid: a close burst of window shots of the player's
//! own hero in the world, `<stem>-000.png`, ... (`ESURV_HERO_SHOTS_COUNT`, default 3), each
//! from another side (a third of a turn on), `ESURV_HERO_SHOTS_EVERY` seconds apart (default
//! 0.6), to see what it wears with nobody at the mouse. It holds until the hero has been in the
//! world `ESURV_HERO_SHOTS_AFTER` seconds (default 12, past the loading screen); from a second
//! before the first shot the world camera is held close to the hero (the `SHOT_*` pose).
//! `ESURV_HERO_SHOTS_DISTANCE`, `_HEIGHT` and `_TURN` (yards, yards, degrees a shot) hold it
//! elsewhere: `DISTANCE=14 HEIGHT=24 TURN=0` looks down on the ground round the hero (the boss
//! marks).

use bevy::prelude::*;

/// The shots' camera: this far from the hero across the ground, this high over its feet, looking
/// at this height on it (yards).
const SHOT_DISTANCE: f32 = 6.0;
const SHOT_EYE_HEIGHT: f32 = 2.4;
const SHOT_LOOK_HEIGHT: f32 = 1.2;
/// Each shot turns the camera this far round the hero from the last.
const SHOT_TURN_DEGREES: f32 = 120.0;

pub(crate) fn register(app: &mut App) {
    if let Some(shots) = HeroShots::from_env() {
        app.insert_resource(shots)
            .add_systems(Update, shoot_the_hero)
            .add_systems(
                PostUpdate,
                frame_the_hero.before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

#[derive(Resource)]
struct HeroShots {
    out: String,
    count: u32,
    after: f32,
    every: f32,
    taken: u32,
    /// When the hero entered the world, on the app clock.
    entered_at: Option<f32>,
    /// The side the first shot is taken from (the player camera's), across the ground.
    first_side: Option<Vec3>,
    /// The frame (app clock) the last shot was taken in: that frame keeps its shot's side.
    shot_at: Option<f32>,
    /// The camera's pose: distance across the ground, eye height, turn between shots.
    distance: f32,
    eye_height: f32,
    turn: f32,
}

impl HeroShots {
    fn from_env() -> Option<Self> {
        let out = std::env::var("ESURV_HERO_SHOTS").ok()?;
        let num = |key: &str, default: f32| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        Some(Self {
            out,
            count: num("ESURV_HERO_SHOTS_COUNT", 3.0).max(1.0) as u32,
            after: num("ESURV_HERO_SHOTS_AFTER", 12.0),
            every: num("ESURV_HERO_SHOTS_EVERY", 0.6),
            taken: 0,
            entered_at: None,
            first_side: None,
            shot_at: None,
            distance: num("ESURV_HERO_SHOTS_DISTANCE", SHOT_DISTANCE),
            eye_height: num("ESURV_HERO_SHOTS_HEIGHT", SHOT_EYE_HEIGHT),
            turn: num("ESURV_HERO_SHOTS_TURN", SHOT_TURN_DEGREES),
        })
    }

    /// Shot `i`'s file: `<stem>-00i.<ext>`.
    fn path(&self, i: u32) -> String {
        match self.out.rsplit_once('.') {
            Some((stem, ext)) => format!("{stem}-{i:03}.{ext}"),
            None => format!("{}-{i:03}", self.out),
        }
    }

    /// When framing begins: a second before the first shot.
    fn framing_from(&self) -> Option<f32> {
        Some(self.entered_at? + self.after - 1.0)
    }
}

fn shoot_the_hero(
    mut commands: Commands,
    mut shots: ResMut<HeroShots>,
    objects: benilla_app::Objects,
    time: Res<Time>,
) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    let now = time.elapsed_secs();
    if shots.entered_at.is_none() && objects.player().is_some() {
        shots.entered_at = Some(now);
        info!("hero shots: the hero is in the world");
    }
    let Some(from) = shots.framing_from() else {
        return;
    };
    if shots.taken >= shots.count
        || shots.first_side.is_none()
        || now < from + 1.0 + shots.every * shots.taken as f32
    {
        return;
    }
    let path = shots.path(shots.taken);
    info!("hero shots: writing {path}");
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
    shots.taken += 1;
    shots.shot_at = Some(now);
}

/// Holds the world camera close to the hero from a second before the first shot, a third of a
/// turn on for each shot taken; the frame a shot is taken in keeps that shot's side.
fn frame_the_hero(
    mut shots: ResMut<HeroShots>,
    objects: benilla_app::Objects,
    places: Query<&GlobalTransform, Without<benilla_world::view::WorldCamera>>,
    mut camera: Query<&mut Transform, With<benilla_world::view::WorldCamera>>,
    time: Res<Time>,
) {
    let Some(from) = shots.framing_from() else {
        return;
    };
    let now = time.elapsed_secs();
    let side_index = if shots.shot_at == Some(now) {
        shots.taken - 1
    } else {
        shots.taken
    };
    if now < from || side_index >= shots.count {
        return;
    }
    let (Ok(mut cam), Some(hero)) = (
        camera.single_mut(),
        objects.player().and_then(|e| places.get(e).ok()),
    ) else {
        return;
    };
    let hero = hero.translation();
    let first = *shots
        .first_side
        .get_or_insert_with(|| (cam.translation - hero).with_y(0.0).normalize_or(Vec3::Z));
    let side = Quat::from_rotation_y((shots.turn * side_index as f32).to_radians()) * first;
    let eye = hero + side * shots.distance + Vec3::Y * shots.eye_height;
    *cam = Transform::from_translation(eye).looking_at(hero + Vec3::Y * SHOT_LOOK_HEIGHT, Vec3::Y);
}
