//! The added race and class pairs and the races' own totem looks, handed to benilla's hooks:
//! the create screen offers each pair `races.ron` adds, dressed in the outfit
//! its rule borrows, and a race's totems get their composed look, built by the client from the stock
//! totem and the planted model (a weapon, for the dwarf) in the player's own data. The character
//! select's figures put their starting-outfit weapons away where `outfit_weapons.ron` says
//! (staves on the back, one-handers on the hip).

use benilla_app::{
    AddedPair, AddedRaceClassPairs, ComposedDisplay, ComposedDisplays, OutfitWeaponSheaths,
    PartModel, PartPlacement,
};
use bevy::prelude::*;
use esurv_content::{Content, TotemPart};

pub(crate) fn register(app: &mut App, content: &Content) {
    app.insert_resource(added_pairs(content));
    app.insert_resource(composed_totems(content));
    app.insert_resource(outfit_weapon_sheaths(content));
    if let Some(shots) = TotemShots::from_env() {
        app.insert_resource(shots)
            .add_systems(Update, shoot_a_totem)
            .add_systems(
                PostUpdate,
                frame_the_totem.before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

/// The creature every totem is (the server's `entries::TOTEM`, the first past the content's).
const TOTEM_ENTRY: u32 = esurv_content::entries::CREATURES_END + 1;
/// The shots' camera: this far from the totem across the ground, this high over its foot, looking
/// at this height on it (yards), turned this far off the player camera's own bearing so the hero
/// is not in the way.
const SHOT_DISTANCE: f32 = 5.0;
const SHOT_EYE_HEIGHT: f32 = 2.6;
const SHOT_LOOK_HEIGHT: f32 = 1.1;
const SHOT_BEARING_TURN_DEGREES: f32 = 90.0;

/// `ESURV_TOTEM_SHOTS=<png>`: a close burst of window shots of a totem, `<stem>-000.png`, ...
/// (`ESURV_TOTEM_SHOTS_COUNT`, default 2), `ESURV_TOTEM_SHOTS_EVERY` seconds apart
/// (default 0.4): its look and how it moves, with nobody at the mouse. It holds until the hero has
/// been in the world `ESURV_TOTEM_SHOTS_AFTER` seconds (default 12, past the loading screen)
/// and a totem stands; from a second before the first shot the world camera is held still close to
/// the totem (the `SHOT_*` pose), so the frames differ only by what moves.
#[derive(Resource)]
struct TotemShots {
    out: String,
    count: u32,
    after: f32,
    every: f32,
    taken: u32,
    /// When the hero entered the world, on the app clock.
    entered_at: Option<f32>,
    /// The held camera pose (eye, target), once framing has begun.
    pose: Option<(Vec3, Vec3)>,
}

impl TotemShots {
    fn from_env() -> Option<Self> {
        let out = std::env::var("ESURV_TOTEM_SHOTS").ok()?;
        let num = |key: &str, default: f32| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        Some(Self {
            out,
            count: num("ESURV_TOTEM_SHOTS_COUNT", 2.0).max(1.0) as u32,
            after: num("ESURV_TOTEM_SHOTS_AFTER", 12.0),
            every: num("ESURV_TOTEM_SHOTS_EVERY", 0.4),
            taken: 0,
            entered_at: None,
            pose: None,
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

/// The totem nearest the hero, if one stands.
fn nearest_totem(
    objects: &benilla_app::Objects,
    places: &Query<&GlobalTransform, Without<benilla_world::view::WorldCamera>>,
) -> Option<Vec3> {
    let hero = places.get(objects.player()?).ok()?.translation();
    objects
        .iter()
        .filter(|(_, _, f)| f.object_entry() == Some(TOTEM_ENTRY))
        .filter_map(|(_, e, _)| places.get(e).ok().map(|t| t.translation()))
        // one just made is not placed yet (it stands at the origin for a frame)
        .filter(|at| *at != Vec3::ZERO)
        .min_by(|a, b| a.distance(hero).total_cmp(&b.distance(hero)))
}

fn shoot_a_totem(
    mut commands: Commands,
    mut shots: ResMut<TotemShots>,
    objects: benilla_app::Objects,
    time: Res<Time>,
) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    let now = time.elapsed_secs();
    if shots.entered_at.is_none() && objects.player().is_some() {
        shots.entered_at = Some(now);
        info!("totem shots: the hero is in the world");
    }
    let Some(from) = shots.framing_from() else {
        return;
    };
    // a shot is due once the camera has been held, and only with a totem framed
    if shots.taken >= shots.count
        || shots.pose.is_none()
        || now < from + 1.0 + shots.every * shots.taken as f32
    {
        return;
    }
    let path = shots.path(shots.taken);
    info!("totem shots: writing {path}");
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
    shots.taken += 1;
}

/// Holds the world camera on the totem from a second before the first shot.
fn frame_the_totem(
    mut shots: ResMut<TotemShots>,
    objects: benilla_app::Objects,
    places: Query<&GlobalTransform, Without<benilla_world::view::WorldCamera>>,
    mut camera: Query<&mut Transform, With<benilla_world::view::WorldCamera>>,
    time: Res<Time>,
) {
    let Some(from) = shots.framing_from() else {
        return;
    };
    if time.elapsed_secs() < from {
        return;
    }
    let Ok(mut cam) = camera.single_mut() else {
        return;
    };
    if shots.pose.is_none() {
        let Some(totem) = nearest_totem(&objects, &places) else {
            return; // none stands yet: hold off
        };
        // from the player camera's side of the totem, turned off its bearing
        let away = (cam.translation - totem).with_y(0.0).normalize_or(Vec3::Z);
        let bearing = Quat::from_rotation_y(SHOT_BEARING_TURN_DEGREES.to_radians()) * away;
        let eye = totem + bearing * SHOT_DISTANCE + Vec3::Y * SHOT_EYE_HEIGHT;
        let target = totem + Vec3::Y * SHOT_LOOK_HEIGHT;
        info!("totem shots: camera held on the totem at {totem:?}");
        shots.pose = Some((eye, target));
    }
    if let Some((eye, target)) = shots.pose {
        *cam = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}

/// Where each starting-outfit weapon rests when a figure puts it away, by its look.
fn outfit_weapon_sheaths(content: &Content) -> OutfitWeaponSheaths {
    OutfitWeaponSheaths(
        content
            .outfit_weapons
            .iter()
            .map(|w| (w.display, w.sheath().min(255) as u8))
            .collect(),
    )
}

/// The pairs for the create screen, each with the races whose outfit it borrows, in order.
fn added_pairs(content: &Content) -> AddedRaceClassPairs {
    AddedRaceClassPairs(
        content
            .added_pairs()
            .into_iter()
            .map(|p| AddedPair {
                race: p.race,
                class: p.class,
                outfit_races: p.outfit_races,
            })
            .collect(),
    )
}

/// One composed display per stock totem look of each race with a totem look of its own.
fn composed_totems(content: &Content) -> ComposedDisplays {
    ComposedDisplays(
        content
            .composed_totems()
            .into_iter()
            .map(|(id, base, look)| ComposedDisplay {
                id,
                base,
                part: match &look.part {
                    TotemPart::Model(path) => PartModel::Model(path.clone()),
                    TotemPart::Weapon(display) => PartModel::Weapon(*display),
                },
                placement: PartPlacement {
                    scale: look.scale,
                    pitch_degrees: look.pitch_degrees,
                    turn_degrees: look.turn_degrees,
                    sink: look.sink,
                    raise_effects: look.raise_effects,
                    hide_base_body: look.hide_totem_body,
                },
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hooks_get_six_pairs_and_the_dwarfs_two_ironfoe_totems() {
        let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        assert_eq!(added_pairs(&content).0.len(), 6);
        let totems = composed_totems(&content).0;
        assert_eq!(
            totems.iter().map(|t| (t.id, t.base)).collect::<Vec<_>>(),
            [(9_304_587, 4587), (9_304_589, 4589)]
        );
        // Ironfoe's display, its body hidden
        assert!(totems
            .iter()
            .all(|t| t.part == PartModel::Weapon(23618) && t.placement.hide_base_body));
    }

    /// The weapon display the dwarf's totems name is Ironfoe's mace in the player's data.
    #[test]
    fn the_dwarfs_totem_weapon_is_ironfoes_mace_in_the_players_data() {
        let data = benilla_formats::wow_data_or_skip!();
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let displays = benilla_formats::load_item_display_catalog(&mut chain).unwrap();
        let ironfoe = displays.get(23618).expect("display 23618");
        assert_eq!(
            ironfoe.model[0].as_deref(),
            Some("hammer_1h_standard_c_03thaurissan.m2")
        );
        assert_eq!(
            ironfoe.model_texture[0].as_deref(),
            Some("Hammer_1H_Standard_C_03Thaurissan")
        );
    }
}
