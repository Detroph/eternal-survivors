//! Taking one building apart as it loads, for a game built on benilla: the building's triangles
//! fall into islands (triangles joined by shared corners), each island goes to the first
//! [`CarvePiece`] whose area holds its middle and whose textures match, and a piece is moved,
//! turned and tilted, or left out. 1.12 never does this; benilla carves nothing until a crate on
//! top of it sets a carve with [`set_wmo_carve`], and the file in the install is never changed.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, RwLock};

use super::super::RenderSubmesh;
use crate::terrain::{ChunkMesh, CHUNK_SIZE, SHADOW_MAP_SIZE};

/// How a carved building's pieces are found and where each goes, in the building's own frame.
#[derive(Debug, Clone, Default)]
pub struct WmoCarve {
    /// Tried in order; an island no piece takes stays where it is.
    pub pieces: Vec<CarvePiece>,
    /// The direction toward the sun the ground's MCSH shadows round the building were baked
    /// with, WoW world frame; with it [`CarvedPieces::reshade`] moves the pieces' shadows with
    /// them, without it the ground keeps its shadows as they are.
    pub sun: Option<[f32; 3]>,
}

/// One piece: the islands whose middle lies in `area` and that wear one of `textures`.
#[derive(Debug, Clone)]
pub struct CarvePiece {
    /// The x and y ranges, the building's frame, that hold the middle of the piece's islands.
    pub area: ([f32; 2], [f32; 2]),
    /// Any island texture whose path contains one of these (any case) matches; empty matches all.
    pub textures: Vec<String>,
    /// Where the piece goes; `None` leaves it out.
    pub to: Option<CarveMove>,
}

/// A piece's move: each point is turned about `pivot` by `turn` and the pivot set at `to`.
#[derive(Debug, Clone, Copy)]
pub struct CarveMove {
    pub pivot: [f32; 3],
    /// A quaternion `(x, y, z, w)`.
    pub turn: [f32; 4],
    pub to: [f32; 3],
}

impl CarveMove {
    /// Where the point `p` goes.
    pub fn point(&self, p: [f32; 3]) -> [f32; 3] {
        let r = rotate(
            self.turn,
            [
                p[0] - self.pivot[0],
                p[1] - self.pivot[1],
                p[2] - self.pivot[2],
            ],
        );
        [r[0] + self.to[0], r[1] + self.to[1], r[2] + self.to[2]]
    }

    /// Where the direction `n` points after the move.
    pub fn direction(&self, n: [f32; 3]) -> [f32; 3] {
        rotate(self.turn, n)
    }
}

/// What becomes of one triangle.
#[derive(Debug, Clone, Copy)]
pub enum CarveFate {
    Stays,
    Moves(CarveMove),
    Goes,
}

static CARVES: RwLock<Vec<(String, Arc<WmoCarve>)>> = RwLock::new(Vec::new());

/// Carve the building at `root_path` (any case, `\` or `/`) every time it is loaded from now on.
pub fn set_wmo_carve(root_path: &str, carve: WmoCarve) {
    let key = carve_key(root_path);
    let mut carves = CARVES.write().unwrap_or_else(|e| e.into_inner());
    carves.retain(|(k, _)| *k != key);
    carves.push((key, Arc::new(carve)));
}

/// The carve set for the building at `root_path`, if any.
pub fn wmo_carve(root_path: &str) -> Option<Arc<WmoCarve>> {
    let key = carve_key(root_path);
    let carves = CARVES.read().unwrap_or_else(|e| e.into_inner());
    carves
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, c)| c.clone())
}

fn carve_key(path: &str) -> String {
    path.trim_start_matches("mpq://")
        .replace('/', "\\")
        .to_ascii_lowercase()
}

/// A building's carve resolved against its geometry: each corner's piece.
pub struct CarvedPieces {
    carve: Arc<WmoCarve>,
    /// A corner, to a hundredth of a yard, and the piece its island went to (`None`: no piece).
    corners: HashMap<[i64; 3], Option<usize>>,
    /// Every drawn triangle, where the building authors it, and its piece.
    drawn: Vec<([[f32; 3]; 3], Option<usize>)>,
}

fn corner(p: [f32; 3]) -> [i64; 3] {
    p.map(|c| (c * 100.0).round() as i64)
}

impl CarvedPieces {
    /// The pieces of a building drawn by `batches` (every group's), the islands taken from them.
    pub fn new(carve: Arc<WmoCarve>, batches: &[RenderSubmesh]) -> Self {
        let mut tris: Vec<([[f32; 3]; 3], String)> = Vec::new();
        for b in batches {
            let texture = b.texture.clone().unwrap_or_default().to_ascii_uppercase();
            for t in b.indices.chunks_exact(3) {
                let at = |k: usize| b.positions.get(t[k] as usize).copied();
                if let (Some(a), Some(c), Some(d)) = (at(0), at(1), at(2)) {
                    tris.push(([a, c, d], texture.clone()));
                }
            }
        }
        let mut parent: Vec<usize> = (0..tris.len()).collect();
        fn root(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        let mut first: HashMap<[i64; 3], usize> = HashMap::new();
        for (i, (t, _)) in tris.iter().enumerate() {
            for v in t {
                match first.get(&corner(*v)) {
                    Some(&j) => {
                        let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                        parent[a] = b;
                    }
                    None => {
                        first.insert(corner(*v), i);
                    }
                }
            }
        }
        // each island's middle (the mean of its triangles' corners) and textures
        let mut islands: HashMap<usize, ([f64; 2], f64, BTreeSet<&str>)> = HashMap::new();
        for (i, (t, texture)) in tris.iter().enumerate() {
            let island =
                islands
                    .entry(root(&mut parent, i))
                    .or_insert(([0.0; 2], 0.0, BTreeSet::new()));
            for v in t {
                island.0[0] += f64::from(v[0]);
                island.0[1] += f64::from(v[1]);
                island.1 += 1.0;
            }
            island.2.insert(texture);
        }
        let piece_of: HashMap<usize, Option<usize>> = islands
            .iter()
            .map(|(&r, (sum, n, textures))| {
                let middle = [(sum[0] / n) as f32, (sum[1] / n) as f32];
                let piece = carve.pieces.iter().position(|p| {
                    within(p, middle)
                        && (p.textures.is_empty()
                            || textures.iter().any(|t| {
                                p.textures
                                    .iter()
                                    .any(|f| t.contains(&f.to_ascii_uppercase()))
                            }))
                });
                (r, piece)
            })
            .collect();
        let mut corners = HashMap::new();
        let mut drawn = Vec::with_capacity(tris.len());
        for (i, (t, _)) in tris.iter().enumerate() {
            let piece = piece_of[&root(&mut parent, i)];
            for v in t {
                corners.insert(corner(*v), piece);
            }
            drawn.push((*t, piece));
        }
        Self {
            carve,
            corners,
            drawn,
        }
    }

    /// What becomes of the triangle `t`: its first known corner's piece decides; a triangle no
    /// drawn island shares a corner with (a collision-only face) goes by the first piece whose
    /// area holds its middle.
    pub fn fate(&self, t: [[f32; 3]; 3]) -> CarveFate {
        let piece = t
            .iter()
            .find_map(|v| self.corners.get(&corner(*v)).copied())
            .unwrap_or_else(|| {
                let middle = [
                    (t[0][0] + t[1][0] + t[2][0]) / 3.0,
                    (t[0][1] + t[1][1] + t[2][1]) / 3.0,
                ];
                self.carve.pieces.iter().position(|p| within(p, middle))
            });
        match piece.map(|i| self.carve.pieces[i].to) {
            None => CarveFate::Stays,
            Some(Some(to)) => CarveFate::Moves(to),
            Some(None) => CarveFate::Goes,
        }
    }

    /// Carve a triangle list in place: moved corners are moved, a left-out triangle dropped. A
    /// corner shared by triangles of two fates is split so each keeps its own.
    pub fn carve_triangles(&self, positions: &mut Vec<[f32; 3]>, indices: &mut Vec<u32>) {
        let mut out_pos: Vec<[f32; 3]> = Vec::with_capacity(positions.len());
        let mut out_idx: Vec<u32> = Vec::with_capacity(indices.len());
        for t in indices.chunks_exact(3) {
            let at = |k: usize| positions.get(t[k] as usize).copied();
            let (Some(a), Some(b), Some(c)) = (at(0), at(1), at(2)) else {
                continue;
            };
            let placed = match self.fate([a, b, c]) {
                CarveFate::Goes => continue,
                CarveFate::Stays => [a, b, c],
                CarveFate::Moves(m) => [m.point(a), m.point(b), m.point(c)],
            };
            for p in placed {
                out_idx.push(out_pos.len() as u32);
                out_pos.push(p);
            }
        }
        *positions = out_pos;
        *indices = out_idx;
    }

    /// Carve one drawn batch: each corner moves with the first triangle that uses it (an island
    /// keeps all its triangles together, so they agree), its normal turned with it; a left-out
    /// triangle is dropped.
    pub fn carve_batch(&self, batch: &mut RenderSubmesh) {
        let mut moved = vec![false; batch.positions.len()];
        let mut kept = Vec::with_capacity(batch.indices.len());
        for t in batch.indices.chunks_exact(3) {
            let at = |k: usize| batch.positions.get(t[k] as usize).copied();
            let (Some(a), Some(b), Some(c)) = (at(0), at(1), at(2)) else {
                continue;
            };
            match self.fate([a, b, c]) {
                CarveFate::Goes => continue,
                CarveFate::Stays => {}
                CarveFate::Moves(m) => {
                    for &i in t {
                        let i = i as usize;
                        if !moved[i] {
                            moved[i] = true;
                            batch.positions[i] = m.point(batch.positions[i]);
                            if let Some(n) = batch.normals.get_mut(i) {
                                *n = m.direction(*n);
                            }
                        }
                    }
                }
            }
            kept.extend_from_slice(t);
        }
        batch.indices = kept;
    }

    /// Move the pieces' baked shadows on the ground `chunks` (one tile's) with the pieces, by
    /// [`WmoCarve::sun`]; `place` takes the building's frame to the world's. A texel the old spot
    /// of a moved or left-out piece shaded, or shaded within [`BAKED_RIM`] texels of it, and
    /// nothing that stays shades as near, is lit; a texel a moved piece shades at its new spot is
    /// shadowed, a chunk with no MCSH given one.
    pub fn reshade(&self, chunks: &mut [ChunkMesh], place: impl Fn([f32; 3]) -> [f32; 3]) {
        let Some(sun) = self.carve.sun else { return };
        let (mut gone, mut stays, mut came) = (Vec::new(), Vec::new(), Vec::new());
        for (t, piece) in &self.drawn {
            match piece.and_then(|i| self.carve.pieces.get(i)).map(|p| p.to) {
                None => stays.push(t.map(&place)),
                Some(to) => {
                    gone.push(t.map(&place));
                    if let Some(m) = to {
                        came.push(t.map(|p| place(m.point(p))));
                    }
                }
            }
        }
        let (gone, stays, came) = (
            RayGrid::new(gone, sun),
            RayGrid::new(stays, sun),
            RayGrid::new(came, sun),
        );
        let n = SHADOW_MAP_SIZE as usize;
        let texel = CHUNK_SIZE / n as f32;
        let rim = BAKED_RIM as f32 * texel;
        for chunk in chunks.iter_mut() {
            let Some(nw) = chunk.positions.first().copied() else {
                continue;
            };
            let mut map = chunk.shadow.clone().unwrap_or_else(|| vec![0; n * n]);
            let mut changed = false;
            for row in 0..n {
                for col in 0..n {
                    let (x, y) = (
                        nw[0] - (row as f32 + 0.5) * texel,
                        nw[1] - (col as f32 + 0.5) * texel,
                    );
                    if !gone.may_shade(x, y, rim) && !came.may_shade(x, y, rim) {
                        continue;
                    }
                    let Some(z) = chunk.height_at([x, y, 0.0]) else {
                        continue;
                    };
                    let ground = [x, y, z + 0.05];
                    let cell = &mut map[row * n + col];
                    let shade = if came.shades(ground) {
                        true
                    } else if *cell >= 128
                        && gone.shades_near(ground, texel)
                        && !stays.shades_near(ground, texel)
                    {
                        false
                    } else {
                        continue;
                    };
                    let value = if shade { 255 } else { 0 };
                    changed |= *cell != value;
                    *cell = value;
                }
            }
            if changed {
                chunk.shadow = Some(map);
            }
        }
    }
}

fn within(p: &CarvePiece, middle: [f32; 2]) -> bool {
    let (x, y) = p.area;
    middle[0] >= x[0] && middle[0] <= x[1] && middle[1] >= y[0] && middle[1] <= y[1]
}

/// `p` turned by the quaternion `q` `(x, y, z, w)`, normalized first.
fn rotate(q: [f32; 4], p: [f32; 3]) -> [f32; 3] {
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if n < 1.0e-9 {
        return p;
    }
    let (x, y, z, w) = (q[0] / n, q[1] / n, q[2] / n, q[3] / n);
    // p + 2w(u×p) + 2u×(u×p)
    let c = [
        y * p[2] - z * p[1],
        z * p[0] - x * p[2],
        x * p[1] - y * p[0],
    ];
    let cc = [
        y * c[2] - z * c[1],
        z * c[0] - x * c[2],
        x * c[1] - y * c[0],
    ];
    [
        p[0] + 2.0 * (w * c[0] + cc[0]),
        p[1] + 2.0 * (w * c[1] + cc[1]),
        p[2] + 2.0 * (w * c[2] + cc[2]),
    ]
}

/// How many texels past what a ray from a texel's middle finds a baked shadow reaches, here and
/// there along its edges: measured, not chosen. On the Caverns' south basin, cleared by middles
/// alone, 126 texels stayed shaded right beside cleared ones, a dotted rim round the old shadows
/// of the pieces taken away; looked for a texel round, 6 stay; two texels round, none, and three
/// clear no more.
const BAKED_RIM: i32 = 2;

/// Triangles bucketed by where they fall seen down the sun's ray, to ask which ground they shade.
struct RayGrid {
    sun: [f32; 3],
    across: [[f32; 3]; 2],
    cells: HashMap<(i32, i32), Vec<usize>>,
    tris: Vec<[[f32; 3]; 3]>,
    /// The ground the triangles' shadows can reach, x and y.
    reach: ([f32; 2], [f32; 2]),
}

impl RayGrid {
    const CELL: f32 = 2.0;
    /// How far below its lowest corner a shadow is looked for, yards.
    const DROP: f32 = 60.0;

    fn new(tris: Vec<[[f32; 3]; 3]>, sun: [f32; 3]) -> Self {
        let sun = normalize(sun);
        let up = if sun[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let u = normalize(cross(up, sun));
        let across = [u, cross(sun, u)];
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        let mut reach = ([f32::MAX; 2], [f32::MIN; 2]);
        let run = Self::DROP / sun[2].max(0.05);
        for (i, t) in tris.iter().enumerate() {
            let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
            for p in t {
                let q = [dot(*p, across[0]), dot(*p, across[1])];
                for k in 0..2 {
                    lo[k] = lo[k].min(q[k]);
                    hi[k] = hi[k].max(q[k]);
                }
                for (k, foot) in [p[0], p[1]].into_iter().enumerate() {
                    let far = foot - sun[k] * run;
                    reach.0[k] = reach.0[k].min(foot.min(far));
                    reach.1[k] = reach.1[k].max(foot.max(far));
                }
            }
            for a in (lo[0] / Self::CELL).floor() as i32..=(hi[0] / Self::CELL).floor() as i32 {
                for b in (lo[1] / Self::CELL).floor() as i32..=(hi[1] / Self::CELL).floor() as i32 {
                    cells.entry((a, b)).or_default().push(i);
                }
            }
        }
        Self {
            sun,
            across,
            cells,
            tris,
            reach,
        }
    }

    /// Whether the ground within `margin` of `x`, `y` may be shaded.
    fn may_shade(&self, x: f32, y: f32, margin: f32) -> bool {
        let (lo, hi) = self.reach;
        x >= lo[0] - margin && x <= hi[0] + margin && y >= lo[1] - margin && y <= hi[1] + margin
    }

    /// Whether a triangle stands between the sun and `ground` or the ground up to
    /// [`BAKED_RIM`] texels from it, any way round at its height (the sand is flat that near, and
    /// a texel across a chunk's edge needs no other chunk): see [`CarvedPieces::reshade`].
    fn shades_near(&self, ground: [f32; 3], texel: f32) -> bool {
        (-BAKED_RIM..=BAKED_RIM).any(|i| {
            (-BAKED_RIM..=BAKED_RIM).any(|j| {
                self.shades([
                    ground[0] + i as f32 * texel,
                    ground[1] + j as f32 * texel,
                    ground[2],
                ])
            })
        })
    }

    /// Whether a triangle stands between `ground` and the sun.
    fn shades(&self, ground: [f32; 3]) -> bool {
        let key = (
            (dot(ground, self.across[0]) / Self::CELL).floor() as i32,
            (dot(ground, self.across[1]) / Self::CELL).floor() as i32,
        );
        self.cells
            .get(&key)
            .is_some_and(|l| l.iter().any(|&i| ray_hits(ground, self.sun, &self.tris[i])))
    }
}

/// Whether the ray from `o` along `d` passes through the triangle `t` (Möller–Trumbore).
fn ray_hits(o: [f32; 3], d: [f32; 3], t: &[[f32; 3]; 3]) -> bool {
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let p = cross(d, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-8 {
        return false;
    }
    let s = sub(o, t[0]);
    let a = dot(s, p) / det;
    if !(0.0..=1.0).contains(&a) {
        return false;
    }
    let q = cross(s, e1);
    let b = dot(d, q) / det;
    b >= 0.0 && a + b <= 1.0 && dot(e2, q) / det > 0.01
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch(positions: Vec<[f32; 3]>, indices: Vec<u32>, texture: &str) -> RenderSubmesh {
        let mut b = RenderSubmesh::default();
        b.normals = vec![[0.0, 0.0, 1.0]; positions.len()];
        b.positions = positions;
        b.indices = indices;
        b.texture = Some(texture.into());
        b
    }

    /// Two separate triangles: one near the origin in wood, one at x 10 in stone.
    fn two_islands() -> RenderSubmesh {
        batch(
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [10.0, 0.0, 0.0],
                [11.0, 0.0, 0.0],
                [10.0, 1.0, 0.0],
            ],
            vec![0, 1, 2, 3, 4, 5],
            "World\\Wood.blp",
        )
    }

    #[test]
    fn a_moved_piece_carries_its_island_and_leaves_the_other() {
        let to = CarveMove {
            pivot: [0.0; 3],
            turn: [0.0, 0.0, 0.0, 1.0],
            to: [0.0, 0.0, 5.0],
        };
        let carve = Arc::new(WmoCarve {
            sun: None,
            pieces: vec![CarvePiece {
                area: ([-1.0, 2.0], [-1.0, 2.0]),
                textures: vec![],
                to: Some(to),
            }],
        });
        let mut b = two_islands();
        let pieces = CarvedPieces::new(carve, std::slice::from_ref(&b));
        pieces.carve_batch(&mut b);
        assert_eq!(b.positions[0], [0.0, 0.0, 5.0]);
        assert_eq!(b.positions[3], [10.0, 0.0, 0.0]);
        assert_eq!(b.indices.len(), 6);
    }

    #[test]
    fn a_left_out_piece_drops_its_triangles_and_a_texture_must_match() {
        let carve = Arc::new(WmoCarve {
            sun: None,
            pieces: vec![
                CarvePiece {
                    area: ([-100.0, 100.0], [-100.0, 100.0]),
                    textures: vec!["stone".into()],
                    to: None,
                },
                CarvePiece {
                    area: ([9.0, 12.0], [-1.0, 2.0]),
                    textures: vec![],
                    to: None,
                },
            ],
        });
        let mut b = two_islands();
        let pieces = CarvedPieces::new(carve, std::slice::from_ref(&b));
        // collision faces on the same corners follow the drawn islands
        let (mut pos, mut idx) = (b.positions.clone(), b.indices.clone());
        pieces.carve_triangles(&mut pos, &mut idx);
        pieces.carve_batch(&mut b);
        assert_eq!(b.indices, vec![0, 1, 2]);
        assert_eq!(idx.len(), 3);
        assert_eq!(pos[idx[0] as usize], [0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_quarter_turn_about_up_takes_x_to_y_and_turns_the_normal() {
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let m = CarveMove {
            pivot: [1.0, 0.0, 0.0],
            turn: [0.0, 0.0, s, s],
            to: [1.0, 0.0, 0.0],
        };
        let p = m.point([2.0, 0.0, 0.0]);
        assert!(
            (p[0] - 1.0).abs() < 1e-5 && (p[1] - 1.0).abs() < 1e-5,
            "{p:?}"
        );
        let n = m.direction([1.0, 0.0, 0.0]);
        assert!((n[1] - 1.0).abs() < 1e-5, "{n:?}");
    }

    #[test]
    fn a_roof_shades_the_ground_down_the_sun_from_it() {
        let roof = vec![[[0.0, 0.0, 10.0], [4.0, 0.0, 10.0], [0.0, 4.0, 10.0]]];
        // the sun up and to the north (+x): the shadow falls 10 yards south
        let grid = RayGrid::new(roof, [1.0, 0.0, 1.0]);
        assert!(grid.shades([-9.0, 1.0, 0.0]));
        assert!(
            !grid.shades([1.0, 1.0, 0.0]),
            "under the roof but not down the sun from it"
        );
        assert!(grid.may_shade(-9.0, 1.0, 0.0) && !grid.may_shade(-80.0, 1.0, 0.0));
    }

    #[test]
    fn a_left_out_roof_takes_its_whole_baked_shadow_rim_and_all() {
        let n = SHADOW_MAP_SIZE as usize;
        let texel = CHUNK_SIZE / n as f32;
        // flat ground, its north-west corner at the origin, a roof 10 up over x and y -20..-10
        let ground = ChunkMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [-CHUNK_SIZE, 0.0, 0.0],
                [0.0, -CHUNK_SIZE, 0.0],
                [-CHUNK_SIZE, -CHUNK_SIZE, 0.0],
            ],
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: vec![0, 1, 2, 2, 1, 3],
            holes: 0,
            base_texture: None,
            layer_textures: Vec::new(),
            layer_effect_ids: Vec::new(),
            alpha_map: None,
            shadow: None,
            pred_tex: [0; 64],
            no_effect_doodad: [false; 64],
            index_x: 0,
            index_y: 0,
            area_id: 0,
            impassable: false,
            liquids: Vec::new(),
        };
        let roof = batch(
            vec![
                [-10.0, -10.0, 10.0],
                [-20.0, -10.0, 10.0],
                [-10.0, -20.0, 10.0],
                [-20.0, -20.0, 10.0],
            ],
            vec![0, 1, 2, 2, 1, 3],
            "World\\Roof.blp",
        );
        // the sun straight up; baked, its shadow two texels wider all round, and a doodad's shadow
        // a few texels past it
        let middle = |k: usize| -(k as f32 + 0.5) * texel;
        let under = |v: f32| (-20.0 - 2.0 * texel..=-10.0 + 2.0 * texel).contains(&v);
        let mut baked = vec![0u8; n * n];
        for row in 0..n {
            for col in 0..n {
                if under(middle(row)) && under(middle(col)) {
                    baked[row * n + col] = 255;
                }
            }
        }
        let doodad = ((10.0 / texel) as usize - 4) * n + (15.0 / texel) as usize;
        baked[doodad] = 255;
        let mut chunks = vec![ground];
        chunks[0].shadow = Some(baked);
        let carve = Arc::new(WmoCarve {
            sun: Some([0.0, 0.0, 1.0]),
            pieces: vec![CarvePiece {
                area: ([-30.0, 0.0], [-30.0, 0.0]),
                textures: vec![],
                to: None,
            }],
        });
        CarvedPieces::new(carve, std::slice::from_ref(&roof)).reshade(&mut chunks, |p| p);
        let map = chunks[0].shadow.as_ref().unwrap();
        let left: Vec<usize> = (0..n * n).filter(|&i| map[i] >= 128).collect();
        assert_eq!(left, vec![doodad], "only the doodad's shadow stays");
    }

    #[test]
    fn a_carve_is_found_by_its_path_in_any_case_and_slash() {
        set_wmo_carve("World\\Test\\Carved.wmo", WmoCarve::default());
        assert!(wmo_carve("mpq://world/test/carved.wmo").is_some());
        assert!(wmo_carve("world/test/other.wmo").is_none());
    }
}
