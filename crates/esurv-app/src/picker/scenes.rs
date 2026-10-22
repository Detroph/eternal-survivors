//! The two places the heroes wait in: the Horde's by a campfire on flat
//! ground east of the road before the front gate of `orgrimmar` at night, the Alliance's by a
//! campfire on the grass beside the main gate of Stormwind. Each is a spot of the world itself,
//! seen from a fixed camera a little over the ground, looking down a little on the heroes with the
//! gate behind them; the campfire is a game object of the player's own data.
//!
//! A camp holds nine heroes, in roster order: five in front, in a semicircle round the campfire on
//! its far side, evenly spaced and facing it, in the faction's front pose (they stand up while
//! hovered or picked); four behind them, two on each side, already standing, facing the campfire
//! and the camera (a picked one greets: a wave or a salute, [`greeting`]).

use benilla_app::SceneView;
use esurv_content::Faction;

/// How a hero waits when it is not the one picked. One constant per faction and place, so a pose
/// can change in one place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Kneel,
    Sit,
    Stand,
}

impl Pose {
    /// The `UNIT_FIELD_BYTES_1` stand state that shows it.
    pub fn stand_state(self) -> u8 {
        match self {
            Pose::Stand => 0,
            Pose::Sit => 1,
            Pose::Kneel => 8,
        }
    }
}

/// How a picked hero at the back greets: an `AnimationData.dbc` emote, played once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Greeting {
    /// `EmoteWave`
    Wave = 67,
    /// `EmoteSalute`
    Salute = 113,
}

/// The Horde's heroes kneel in front and stand at the back.
pub const HORDE_FRONT_POSE: Pose = Pose::Kneel;
pub const HORDE_BACK_POSE: Pose = Pose::Stand;
/// The Alliance's heroes sit in front and stand at the back.
pub const ALLIANCE_FRONT_POSE: Pose = Pose::Sit;
pub const ALLIANCE_BACK_POSE: Pose = Pose::Stand;
/// A front hero stands while hovered or picked.
pub const PICKED_POSE: Pose = Pose::Stand;
/// How a picked back hero greets, by class (a class id from `classes.ron`): the fighting classes
/// salute, the others wave. The same in both camps.
pub fn greeting(class: &str) -> Greeting {
    match class {
        "warrior" | "paladin" | "hunter" | "rogue" => Greeting::Salute,
        _ => Greeting::Wave,
    }
}

/// Where a hero waits: in the front semicircle, or standing at the back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Front,
    Back,
}

/// Where the camera stands and what it looks at, in yards from the campfire along the camera's
/// bearing: `back` behind it and `up` over its ground, looking at a point `beyond` past it and
/// `look_up` high.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    pub back: f32,
    pub up: f32,
    pub beyond: f32,
    pub look_up: f32,
}

/// The front semicircle round the campfire: `radius` yards out, on its far side, its places
/// `step` radians apart and centred on the camera's line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrontArc {
    pub radius: f32,
    pub step: f32,
}

/// One faction's place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FactionScene {
    /// `Map.dbc` id.
    pub map: u32,
    /// Where the campfire burns, WoW world coordinates (the ground under it).
    pub campfire: [f32; 3],
    /// The campfire's `GameObjectDisplayInfo.dbc` id.
    pub campfire_display: u32,
    /// The way the camera looks across the campfire, radians about WoW +Z from north toward west.
    pub bearing: f32,
    pub framing: Framing,
    /// The hour the place is lit by, minutes after midnight.
    pub minute_of_day: u32,
    /// How the heroes not picked wait in front and at the back.
    pub front_pose: Pose,
    pub back_pose: Pose,
    pub front: FrontArc,
    /// The places at the back, filled in this order: left near, right near, left far, right far
    /// (yards to the camera's right of the campfire, negative to its left; yards beyond it). Each
    /// behind the semicircle and between its heroes as the camera sees them, clear of the select
    /// screen's panels.
    pub back_places: [(f32, f32); BACK_ROOM],
}

/// How many heroes a camp holds, in front and at the back.
const FRONT_ROOM: usize = 5;
const BACK_ROOM: usize = 4;
const ROOM: usize = FRONT_ROOM + BACK_ROOM;

/// Both camps' semicircle: 3 yards out, 36 degrees apart (the five span 144 degrees).
const FRONT_ARC: FrontArc = FrontArc {
    radius: 3.0,
    step: 0.628,
};

/// Both camps' places at the back, each standing in a gap of the front row as the camera sees it:
/// the near pair between the middle hero and the next, the far pair between the next and the
/// outermost.
const BACK_PLACES: [(f32, f32); BACK_ROOM] = [(-1.2, 5.2), (1.2, 5.2), (-3.2, 4.6), (3.2, 4.6)];

/// A camp-sized campfire (`ElwynnCampfire.mdx`, `GameObjectDisplayInfo` 192).
const CAMPFIRE: u32 = 192;

/// The Horde's camera: 3 yards over the campfire's ground and 9 behind it, looking down a little
/// on the heroes, aimed low enough that they sit clear of the select screen's name and Enter World
/// button, with the gate rising behind them.
const HORDE_FRAMING: Framing = Framing {
    back: 9.0,
    up: 3.0,
    beyond: 30.0,
    look_up: -0.4,
};

/// The Alliance's camera: the Horde's, 5% closer along its line of sight.
const ALLIANCE_FRAMING: Framing = Framing {
    back: 8.55,
    up: 2.96,
    beyond: 30.0,
    look_up: -0.4,
};

/// The Horde's place: flat ground east of the road before the front gate of `orgrimmar`, the gate
/// square behind the heroes.
pub const HORDE: FactionScene = FactionScene {
    map: 1,
    campfire: [1325.0, -4412.0, 26.9],
    campfire_display: CAMPFIRE,
    // looking north by west (38 degrees), past the heroes to the gate
    bearing: 0.663,
    framing: HORDE_FRAMING,
    // half past eleven at night
    minute_of_day: 23 * 60 + 30,
    front_pose: HORDE_FRONT_POSE,
    back_pose: HORDE_BACK_POSE,
    front: FRONT_ARC,
    back_places: BACK_PLACES,
};

/// The Alliance's place: on the grass beside the main gate of Stormwind, the gate and its banners
/// behind the heroes.
pub const ALLIANCE: FactionScene = FactionScene {
    map: 0,
    campfire: [-9110.0, 418.0, 93.7],
    campfire_display: CAMPFIRE,
    // looking north along the road, at the gate
    bearing: 0.0,
    framing: ALLIANCE_FRAMING,
    // half past eleven at night
    minute_of_day: 23 * 60 + 30,
    front_pose: ALLIANCE_FRONT_POSE,
    back_pose: ALLIANCE_BACK_POSE,
    front: FRONT_ARC,
    back_places: BACK_PLACES,
};

/// How far the warband screen moves the camera's aim: yards to its right and up, at the aim
/// point 30 yards past the campfire ([`FactionScene::warband_view`]).
pub const WARBAND_AIM: (f32, f32) = (9.0, 0.0);

/// A faction's place.
pub fn scene(faction: Faction) -> FactionScene {
    match faction {
        Faction::Horde => HORDE,
        Faction::Alliance => ALLIANCE,
    }
}

/// Numbers from a switch's value, `a,b,c`.
fn numbers(v: &str) -> Vec<f32> {
    v.split(',').filter_map(|p| p.trim().parse().ok()).collect()
}

impl FactionScene {
    /// How a hero not picked waits at `place`.
    pub fn pose(&self, place: Place) -> Pose {
        match place {
            Place::Front => self.front_pose,
            Place::Back => self.back_pose,
        }
    }

    /// The way the camera looks, as a ground direction (WoW x, y).
    fn ahead(&self) -> (f32, f32) {
        (self.bearing.cos(), self.bearing.sin())
    }

    /// How the select screen sees the place.
    pub fn view(&self) -> SceneView {
        let (dx, dy) = self.ahead();
        let [x, y, z] = self.campfire;
        let f = self.framing;
        SceneView {
            map: self.map,
            eye: [x - dx * f.back, y - dy * f.back, z + f.up],
            look_at: [x + dx * f.beyond, y + dy * f.beyond, z + f.look_up],
            minute_of_day: self.minute_of_day,
        }
    }

    /// How the menus' warband screen sees the place (menus.md): the
    /// same camera, its aim moved `aside` yards to the camera's right and `lift` yards up, so the
    /// camp sits in the lower left of the window, clear of the zone column on the right.
    pub fn warband_view(&self, (aside, lift): (f32, f32)) -> SceneView {
        let mut v = self.view();
        let (fx, fy) = self.ahead();
        // the camera's right: its bearing turned a quarter clockwise
        let (rx, ry) = (fy, -fx);
        v.look_at = [
            v.look_at[0] + rx * aside,
            v.look_at[1] + ry * aside,
            v.look_at[2] + lift,
        ];
        v
    }

    /// Where hero `i` of `n` waits, its facing (towards the campfire) and which place it is;
    /// `None` past the last place. The semicircle fills first, from its middle out (so a camp of
    /// fewer than five stays centred), then the back, in roster order.
    pub fn slot(&self, i: usize, n: usize) -> Option<([f32; 3], f32, Place)> {
        let n = n.min(ROOM);
        if i >= n {
            return None;
        }
        let [x, y, z] = self.campfire;
        let (fx, fy) = self.ahead();
        let (at, place) = if i < FRONT_ROOM {
            let m = n.min(FRONT_ROOM);
            // the places centred on the camera's line, filled from the middle out
            let mid = (m - 1) as f32 / 2.0;
            let mut order: Vec<usize> = (0..m).collect();
            order.sort_by(|a, b| {
                ((*a as f32 - mid).abs(), *a)
                    .partial_cmp(&((*b as f32 - mid).abs(), *b))
                    .unwrap()
            });
            let off = (order[i] as f32 - mid) * self.front.step;
            let a = self.bearing + off;
            let r = self.front.radius;
            ([x + r * a.cos(), y + r * a.sin(), z], Place::Front)
        } else {
            let (right, beyond) = self.back_places[i - FRONT_ROOM];
            // the camera's right: its bearing turned a quarter clockwise
            let (rx, ry) = (fy, -fx);
            (
                [
                    x + fx * beyond + rx * right,
                    y + fy * beyond + ry * right,
                    z,
                ],
                Place::Back,
            )
        };
        let facing = (y - at[1]).atan2(x - at[0]);
        Some((at, facing, place))
    }

    /// How many heroes the place has room for.
    pub fn room() -> usize {
        ROOM
    }

    /// The place with the dev switches' corrections, to try a spot out:
    /// `ESURV_PICKER_CAMPFIRE=<x>,<y>,<z>` moves the campfire, `ESURV_PICKER_BEARING=
    /// <degrees>` turns the camera, `ESURV_PICKER_FRAMING=<back>,<up>,<beyond>,<look up>`
    /// moves it.
    pub fn with_overrides(
        mut self,
        campfire: Option<&str>,
        bearing: Option<&str>,
        framing: Option<&str>,
    ) -> Self {
        if let Some([x, y, z]) = campfire
            .map(numbers)
            .and_then(|c| <[f32; 3]>::try_from(c).ok())
        {
            self.campfire = [x, y, z];
        }
        if let Some(deg) = bearing.and_then(|b| b.trim().parse::<f32>().ok()) {
            self.bearing = deg.to_radians();
        }
        if let Some([back, up, beyond, look_up]) = framing
            .map(numbers)
            .and_then(|c| <[f32; 4]>::try_from(c).ok())
        {
            self.framing = Framing {
                back,
                up,
                beyond,
                look_up,
            };
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_heroes_face_the_campfire_inside_the_frame() {
        for s in [HORDE, ALLIANCE] {
            let v = s.view();
            let dist =
                |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
            for n in 1..=FactionScene::room() {
                for i in 0..n {
                    let (at, facing, place) = s.slot(i, n).unwrap();
                    assert_eq!(
                        place == Place::Front,
                        i < FRONT_ROOM,
                        "the front fills first"
                    );
                    assert!(
                        dist(at, v.eye) > dist(s.campfire, v.eye),
                        "slot {i} waits beyond the campfire"
                    );
                    // facing the campfire: a step forward closes on it
                    let step = [
                        at[0] + facing.cos() * 0.1,
                        at[1] + facing.sin() * 0.1,
                        at[2],
                    ];
                    assert!(
                        dist(step, s.campfire) < dist(at, s.campfire),
                        "slot {i} faces the campfire"
                    );
                    // in the frame between the two panels: across the view, as a share of the
                    // half width (the 45 degree view is 72.8 degrees wide at 16:9)
                    let (fx, fy) = (s.bearing.cos(), s.bearing.sin());
                    let (ex, ey) = (at[0] - v.eye[0], at[1] - v.eye[1]);
                    let ahead = ex * fx + ey * fy;
                    let right = ex * fy - ey * fx;
                    let across = right / (ahead * 36.4f32.to_radians().tan());
                    assert!(
                        (-0.5..0.5).contains(&across),
                        "slot {i} of {n} is in the frame ({across:.2})"
                    );
                }
            }
            assert!(s.slot(3, 3).is_none());
            assert!(s
                .slot(FactionScene::room(), FactionScene::room() + 1)
                .is_none());
            // the camera stands a little over the ground and looks down a little
            assert!(v.look_at[2] < v.eye[2] && v.eye[2] > s.campfire[2]);
        }
        assert_eq!(
            (
                HORDE.pose(Place::Front).stand_state(),
                HORDE.pose(Place::Back).stand_state(),
                ALLIANCE.pose(Place::Front).stand_state(),
                ALLIANCE.pose(Place::Back).stand_state(),
                PICKED_POSE.stand_state()
            ),
            (8, 0, 1, 0, 0)
        );
    }

    #[test]
    fn a_camp_holds_five_in_front_evenly_spaced_and_four_behind() {
        let places: Vec<_> = (0..9).map(|i| HORDE.slot(i, 9).unwrap()).collect();
        assert_eq!(places.iter().filter(|p| p.2 == Place::Front).count(), 5);
        assert_eq!(places.iter().filter(|p| p.2 == Place::Back).count(), 4);
        // the five in front: all as far from the campfire, and each next pair as far apart
        let c = HORDE.campfire;
        let d = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
        for p in &places[..5] {
            assert!((d(p.0, c) - HORDE.front.radius).abs() < 1e-3);
        }
        let mut gaps: Vec<f32> = places[..5]
            .iter()
            .flat_map(|a| places[..5].iter().map(move |b| d(a.0, b.0)))
            .filter(|g| *g > 0.01)
            .collect();
        gaps.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!(gaps[0] > 1.8, "room between them: {:.2}", gaps[0]);
        // two at the back on either side
        let left = BACK_PLACES.iter().filter(|p| p.0 < 0.0).count();
        assert_eq!(left, 2);
    }

    #[test]
    fn a_picked_back_hero_greets_by_class() {
        assert_eq!(greeting("warrior"), Greeting::Salute);
        assert_eq!(greeting("mage"), Greeting::Wave);
        assert_eq!((Greeting::Wave as u16, Greeting::Salute as u16), (67, 113));
    }

    #[test]
    fn a_place_can_be_tried_out_by_switch() {
        let s = HORDE.with_overrides(Some("1,2,3"), Some("90"), Some("5,1,10,2"));
        assert_eq!(s.campfire, [1.0, 2.0, 3.0]);
        assert!((s.bearing - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(s.framing.beyond, 10.0);
        assert_eq!(
            HORDE.with_overrides(Some("nonsense"), None, Some("1,2")),
            HORDE
        );
    }
}
