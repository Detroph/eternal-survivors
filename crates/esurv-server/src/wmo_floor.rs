//! The floors and walls of a WMO-only map (Dire Maul, the Stockade): the map is one building and
//! has no terrain, so its ground is the building's own collision faces. The client's face rules
//! (`benilla-world`'s collision): walking skips MOPY `DETAIL` faces, sight skips `NOCAMCOLLIDE`
//! ones; the shown doodads' collision hulls (pillars, statues) count for both.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use benilla_formats::{
    accumulate_wmo_group_camera_collision, accumulate_wmo_group_collision,
    load_m2_collision_hull, parse_wmo_root, triangle_z_at, Chain, CollisionMesh, GlobalWmo,
};

/// How far above the height asked about a floor still counts as "below" it: a stair step.
pub const FLOOR_STEP: f32 = 2.0;
/// The side of the index's square cells, in yards.
const CELL: f32 = 4.0;

/// A triangle in world space.
pub type Tri = [[f32; 3]; 3];

/// Triangles in world space, indexed by the square cells their XY bounds cover.
#[derive(Default)]
struct Faces {
    tris: Vec<Tri>,
    cells: HashMap<(i32, i32), Vec<u32>>,
}

fn cell(v: f32) -> i32 {
    (v / CELL).floor() as i32
}

impl Faces {
    fn new(tris: Vec<Tri>) -> Self {
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, t) in tris.iter().enumerate() {
            let (lo, hi) = bounds(t);
            for cx in cell(lo[0])..=cell(hi[0]) {
                for cy in cell(lo[1])..=cell(hi[1]) {
                    cells.entry((cx, cy)).or_default().push(i as u32);
                }
            }
        }
        Self { tris, cells }
    }

    /// The faces whose XY bounds cover the cell of `(x, y)`.
    fn at(&self, x: f32, y: f32) -> impl Iterator<Item = &Tri> {
        self.cells
            .get(&(cell(x), cell(y)))
            .into_iter()
            .flatten()
            .map(|&i| &self.tris[i as usize])
    }

    /// Whether any face crosses the segment `from`→`to`.
    fn crossed(&self, from: [f32; 3], to: [f32; 3]) -> bool {
        let (x0, x1) = (cell(from[0].min(to[0])), cell(from[0].max(to[0])));
        let (y0, y1) = (cell(from[1].min(to[1])), cell(from[1].max(to[1])));
        if x0 == x1 && y0 == y1 {
            return self.at(from[0], from[1]).any(|t| segment_hits(from, to, t));
        }
        let mut seen: Vec<u32> = Vec::new();
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                seen.extend(self.cells.get(&(cx, cy)).into_iter().flatten().copied());
            }
        }
        seen.sort_unstable();
        seen.dedup();
        seen.into_iter().any(|i| segment_hits(from, to, &self.tris[i as usize]))
    }

    /// Every face's height over column `(x, y)`.
    fn heights(&self, x: f32, y: f32) -> impl Iterator<Item = f32> + '_ {
        self.at(x, y).filter_map(move |t| triangle_z_at(t, x, y))
    }
}

fn bounds(t: &Tri) -> ([f32; 2], [f32; 2]) {
    let lo = [0, 1].map(|k| t[0][k].min(t[1][k]).min(t[2][k]));
    let hi = [0, 1].map(|k| t[0][k].max(t[1][k]).max(t[2][k]));
    (lo, hi)
}

/// Whether segment `a`→`b` crosses triangle `t` strictly between its ends (Möller–Trumbore).
fn segment_hits(a: [f32; 3], b: [f32; 3], t: &Tri) -> bool {
    let sub = |p: [f32; 3], q: [f32; 3]| [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
    let cross = |p: [f32; 3], q: [f32; 3]| {
        [p[1] * q[2] - p[2] * q[1], p[2] * q[0] - p[0] * q[2], p[0] * q[1] - p[1] * q[0]]
    };
    let dot = |p: [f32; 3], q: [f32; 3]| p[0] * q[0] + p[1] * q[1] + p[2] * q[2];
    let d = sub(b, a);
    let (e1, e2) = (sub(t[1], t[0]), sub(t[2], t[0]));
    let p = cross(d, e2);
    let det = dot(e1, p);
    if det.abs() < 1.0e-9 {
        return false;
    }
    let s = sub(a, t[0]);
    let u = dot(s, p) / det;
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = cross(s, e1);
    let v = dot(d, q) / det;
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    let along = dot(e2, q) / det;
    along > 1.0e-4 && along < 1.0 - 1.0e-4
}

/// A WMO-only map's floors (what a body stands on) and walls (what blocks sight).
pub struct WmoFloors {
    walk: Faces,
    sight: Faces,
}

impl WmoFloors {
    /// From world-space walking and sight triangles.
    pub fn from_triangles(walk: Vec<Tri>, sight: Vec<Tri>) -> Self {
        Self {
            walk: Faces::new(walk),
            sight: Faces::new(sight),
        }
    }

    /// Reads the map's WMO, every group file and the shown doodads' hulls, placed as the WDT says.
    pub fn load(chain: &mut Chain, wmo: &GlobalWmo) -> Result<Self> {
        Self::load_all(chain, std::slice::from_ref(wmo))
    }

    /// Several buildings' floors as one: a dungeon whose wings stand on its tiles.
    pub fn load_all(chain: &mut Chain, wmos: &[GlobalWmo]) -> Result<Self> {
        Self::load_all_on(chain, wmos, Vec::new())
    }

    /// Several buildings' floors and the terrain between them (world-space triangles) as one:
    /// a party dungeon with streets and squares between its buildings (Stratholme).
    pub fn load_all_on(chain: &mut Chain, wmos: &[GlobalWmo], terrain: Vec<Tri>) -> Result<Self> {
        let (mut walk, mut sight) = (Vec::new(), Vec::new());
        for wmo in wmos {
            let (w, s) = Self::placed(chain, wmo)?;
            walk.extend(w);
            sight.extend(s);
        }
        sight.extend_from_slice(&terrain);
        walk.extend(terrain);
        Ok(Self::from_triangles(walk, sight))
    }

    /// One building's walking and sight triangles, where it is placed.
    fn placed(chain: &mut Chain, wmo: &GlobalWmo) -> Result<(Vec<Tri>, Vec<Tri>)> {
        let [rx, ry, rz] = wmo.rotation;
        if rx != 0.0 || rz != 0.0 {
            bail!("{} is tilted ({rx}, {rz}); no shipped map is", wmo.model);
        }
        // The client's placement, `Rx(90°)·Ry(ry−180°)·Rz(−rx)·Rx(rz−90°)` in the WoW frame,
        // is a turn of `ry − 180°` about Z when untilted (Dire Maul's 180° is none at all).
        let (sin, cos) = (ry - 180.0).to_radians().sin_cos();
        let place = |p: [f32; 3]| {
            [
                p[0] * cos - p[1] * sin + wmo.position[0],
                p[0] * sin + p[1] * cos + wmo.position[1],
                p[2] + wmo.position[2],
            ]
        };
        let root_path = wmo.model.to_ascii_lowercase();
        let bytes = chain
            .read_file(&root_path)
            .with_context(|| format!("reading WMO {root_path}"))?;
        let root = parse_wmo_root(&bytes).with_context(|| format!("parsing WMO {root_path}"))?;
        let stem = root_path.strip_suffix(".wmo").unwrap_or(&root_path);
        let (mut walk, mut sight) = (CollisionMesh::default(), CollisionMesh::default());
        for gi in 0..root.group_count() {
            let Ok(group) = chain.read_file(&format!("{stem}_{gi:03}.wmo")) else {
                continue;
            };
            accumulate_wmo_group_collision(&group, &mut walk.positions, &mut walk.indices);
            accumulate_wmo_group_camera_collision(&group, &mut sight.positions, &mut sight.indices);
        }
        let mut walk = triangles(&walk, place);
        let mut sight = triangles(&sight, place);

        // Set 0 always shows, and the placement's own set beside it.
        let sets = root.doodad_sets();
        let shown = [0, usize::from(wmo.doodad_set)];
        let mut hulls: HashMap<String, Option<CollisionMesh>> = HashMap::new();
        for (k, &s) in shown.iter().enumerate() {
            let Some(set) = sets.get(s).filter(|_| k == 0 || s != 0) else {
                continue;
            };
            let range = set.start as usize..(set.start + set.count) as usize;
            for d in root.doodads().get(range).unwrap_or_default() {
                let hull = hulls
                    .entry(d.model.to_ascii_lowercase())
                    .or_insert_with(|| load_m2_collision_hull(chain, &d.model).ok());
                let Some(hull) = hull.as_ref().filter(|h| !h.is_empty()) else {
                    continue;
                };
                let at = |p: [f32; 3]| {
                    let r = rotate(d.orientation, p);
                    place([
                        r[0] * d.scale + d.position[0],
                        r[1] * d.scale + d.position[1],
                        r[2] * d.scale + d.position[2],
                    ])
                };
                let placed = triangles(hull, at);
                walk.extend_from_slice(&placed);
                sight.extend(placed);
            }
        }
        Ok((walk, sight))
    }

    /// The floor at `(x, y)` nearest below `near_z` (up to [`FLOOR_STEP`] above it counts).
    pub fn floor(&self, x: f32, y: f32, near_z: f32) -> Option<f32> {
        self.walk
            .heights(x, y)
            .filter(|&z| z <= near_z + FLOOR_STEP)
            .max_by(f32::total_cmp)
    }

    /// The highest floor at `(x, y)`: the roof where there is one.
    pub fn top(&self, x: f32, y: f32) -> Option<f32> {
        self.walk.heights(x, y).max_by(f32::total_cmp)
    }

    /// Whether no wall, floor or prop crosses the line from `from` to `to`.
    pub fn clear_line(&self, from: [f32; 3], to: [f32; 3]) -> bool {
        !self.sight.crossed(from, to)
    }

    /// Whether a walking face (a wall, a step's riser, a prop's hull) crosses the line from
    /// `from` to `to`: what stops a body moving along it.
    pub fn walk_blocked(&self, from: [f32; 3], to: [f32; 3]) -> bool {
        self.walk.crossed(from, to)
    }

    /// Every walking face's height over column `(x, y)`, floors and ceilings alike.
    pub fn walk_heights(&self, x: f32, y: f32) -> Vec<f32> {
        self.walk.heights(x, y).collect()
    }

    /// How many floor and sight faces the map has.
    pub fn face_counts(&self) -> (usize, usize) {
        (self.walk.tris.len(), self.sight.tris.len())
    }
}

/// A mesh's triangles, each vertex through `place`.
fn triangles(mesh: &CollisionMesh, place: impl Fn([f32; 3]) -> [f32; 3]) -> Vec<Tri> {
    mesh.indices
        .chunks_exact(3)
        .filter_map(|t| {
            let v = |k: usize| mesh.positions.get(t[k] as usize).map(|&p| place(p));
            Some([v(0)?, v(1)?, v(2)?])
        })
        .collect()
}

/// `p` turned by quaternion `q` `(x, y, z, w)`, normalized first as the client does.
fn rotate(q: [f32; 4], p: [f32; 3]) -> [f32; 3] {
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if n < 1.0e-9 {
        return p;
    }
    let (x, y, z, w) = (q[0] / n, q[1] / n, q[2] / n, q[3] / n);
    // p + 2w(u×p) + 2u×(u×p)
    let c = [y * p[2] - z * p[1], z * p[0] - x * p[2], x * p[1] - y * p[0]];
    let cc = [y * c[2] - z * c[1], z * c[0] - x * c[2], x * c[1] - y * c[0]];
    [
        p[0] + 2.0 * (w * c[0] + cc[0]),
        p[1] + 2.0 * (w * c[1] + cc[1]),
        p[2] + 2.0 * (w * c[2] + cc[2]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square floor 10 yd across at height `z`, over (0..10, 0..10).
    fn slab(z: f32) -> Vec<Tri> {
        vec![
            [[0.0, 0.0, z], [10.0, 0.0, z], [10.0, 10.0, z]],
            [[0.0, 0.0, z], [10.0, 10.0, z], [0.0, 10.0, z]],
        ]
    }

    #[test]
    fn a_stacked_floor_answers_the_one_nearest_below_the_height_asked() {
        let mut walk = slab(0.0);
        walk.extend(slab(8.0));
        let f = WmoFloors::from_triangles(walk, vec![]);
        assert_eq!(f.floor(5.0, 4.0, 0.0), Some(0.0), "the cellar");
        assert_eq!(f.floor(5.0, 4.0, 7.0), Some(8.0), "a step below the hall counts");
        assert_eq!(f.floor(5.0, 4.0, 5.0), Some(0.0), "too far under the hall");
        assert_eq!(f.floor(5.0, 4.0, 30.0), Some(8.0));
        assert_eq!(f.floor(5.0, 4.0, -5.0), None, "under every floor");
        assert_eq!(f.top(5.0, 4.0), Some(8.0));
        assert_eq!(f.floor(15.0, 4.0, 0.0), None, "off the floor");
    }

    #[test]
    fn a_wall_blocks_the_line_across_it_and_not_one_beside_it() {
        // a wall at x = 5 over y 0..10, z 0..4
        let wall = vec![
            [[5.0, 0.0, 0.0], [5.0, 10.0, 0.0], [5.0, 10.0, 4.0]],
            [[5.0, 0.0, 0.0], [5.0, 10.0, 4.0], [5.0, 0.0, 4.0]],
        ];
        let f = WmoFloors::from_triangles(vec![], wall);
        assert!(!f.clear_line([0.0, 5.0, 1.5], [10.0, 5.0, 1.5]));
        assert!(f.clear_line([0.0, 5.0, 6.0], [10.0, 5.0, 6.0]), "over it");
        assert!(f.clear_line([0.0, 12.0, 1.5], [10.0, 12.0, 1.5]), "past its end");
        assert!(f.clear_line([0.0, 5.0, 1.5], [4.0, 5.0, 1.5]), "short of it");
    }

    #[test]
    fn a_quarter_turn_about_z_takes_x_to_y() {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let r = rotate([0.0, 0.0, h, h], [1.0, 0.0, 0.0]);
        assert!((r[0]).abs() < 1e-6 && (r[1] - 1.0).abs() < 1e-6 && r[2].abs() < 1e-6, "{r:?}");
    }
}

#[cfg(test)]
pub(crate) mod real_data {
    use benilla_formats::MapTiles;

    use super::*;

    pub(crate) fn building(map: &str) -> Option<WmoFloors> {
        let data = benilla_formats::wow_data_or_skip!(None);
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let tiles = MapTiles::load(&mut chain, map).unwrap();
        let wmo = tiles.global_wmo().expect("a WMO-only map").clone();
        Some(WmoFloors::load(&mut chain, &wmo).unwrap())
    }

    /// A `.gps` spot inside Dire Maul (map 429): its floor is the height the client stood
    /// at, under open sky, with stacked halls near it and walls that block sight across it.
    #[test]
    fn dire_maul_has_a_floor_at_its_gps_spot_and_walls_around_it() {
        let Some(dm) = building("DireMaul") else { return };
        let (walk, sight) = dm.face_counts();
        assert!(walk > 10_000 && sight > 10_000, "{walk} floor, {sight} sight faces");
        let (x, y, z) = (806.627_26, 482.043, 37.317_01);
        let floor = dm.floor(x, y, z).expect("a floor at the spot");
        assert!((floor - z).abs() < 0.1, "floor {floor}, the client stood at {z}");
        assert_eq!(dm.top(x, y), Some(floor), "open sky over the spot");
        // somewhere near it the floors stack: a hall over a lower one, each found from its height
        let stacked = (-50..=50)
            .flat_map(|i| (-50..=50).map(move |j| (x + i as f32 * 2.0, y + j as f32 * 2.0)))
            .find_map(|(px, py)| {
                let top = dm.top(px, py)?;
                let low = dm.floor(px, py, top - 6.0)?;
                (top - low > 6.0).then_some((px, py, top, low))
            });
        let (px, py, top, low) = stacked.expect("stacked floors within 100 yd");
        assert_eq!(dm.floor(px, py, top), Some(top));
        assert_eq!(dm.floor(px, py, low + 0.5), Some(low));
        // walls stop a good share of the level lines 60 yd out (23 of 72 headings in 1.12.1)
        let eye = [x, y, floor + 1.5];
        let blocked = (0..72)
            .map(|i| (i as f32 * 5.0).to_radians())
            .filter(|a| !dm.clear_line(eye, [x + 60.0 * a.cos(), y + 60.0 * a.sin(), floor + 1.5]))
            .count();
        assert!((12..60).contains(&blocked), "{blocked} of 72 headings blocked at 60 yd");
        assert!(dm.clear_line(eye, [x + 5.0, y, floor + 1.5]), "open floor 5 yd out");
    }

    /// The Stockade (map 34) is placed at heading 0°, a half turn in the client's placement: its
    /// entrance, where vmangos puts a player coming in, `(54.23, 0.28, -18.34)`, has its floor.
    #[test]
    fn the_stockades_half_turn_puts_its_floor_under_the_entrance() {
        let Some(jail) = building("StormwindJail") else { return };
        let floor = jail.floor(54.23, 0.28, -18.34).expect("a floor at the entrance");
        assert!((floor + 18.34).abs() < 2.0, "floor {floor}");
    }
}
