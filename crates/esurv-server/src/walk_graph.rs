//! Walking over a WMO-only map's floors: a grid of floor cells a yard across, built as a search
//! first reaches them, joined where a body could step from one to the next, and the walking
//! distances out from a start (a flow field). Spawns use it to stay on floors the hero can walk
//! to; enemies can follow its distances downhill.
//!
//! The limits stand in for the client's mover, whose own step and slope numbers are not known.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::{Arc, Mutex};

use esurv_sim::game::run::crosses;

use crate::wmo_floor::{WmoFloors, FLOOR_STEP};

/// The side of a floor cell, in yards.
pub const GRID: f32 = 1.0;
/// The steepest walk: this much rise per yard across, about 50°.
pub const CLIMB: f32 = 1.2;
/// A floor with another face less than this above it is no room to stand.
pub const HEADROOM: f32 = 2.0;
/// A step between cells is blocked by a face crossing the line this high over both floors, so a
/// stair's risers below it do not count and a wall does.
pub const KNEE: f32 = 0.6;
/// Flow fields kept for reuse, one per start cell (heroes standing still ask the same one).
const FIELDS_KEPT: usize = 8;
/// A field is reused while its start is this many cells or fewer from the one asked, on the
/// same floor: distances then err by under 3 yd, and a walking hero builds a field every few
/// yards, not every yard (for the tick's cost).
const FIELD_REUSE: i32 = 2;
/// How far a follower's field reaches from the hero, yards: past any spawn ring.
pub const FOLLOW_REACH: f32 = 100.0;
/// Cells a follower looks down the field each time it decides where to go.
pub const FOLLOW_CELLS: usize = 8;

/// A floor cell: its column and which of the column's floors, top first.
pub type Cell = (i32, i32, u16);

/// The walking distances from one start cell, out to `reach` yards.
pub struct Field {
    pub start: Cell,
    pub reach: f32,
    pub distance: HashMap<Cell, f32>,
}

impl Field {
    /// The neighbouring cell one step nearer the start, the way a follower goes.
    pub fn downhill(&self, walking: &Walking, floors: &WmoFloors, cell: Cell) -> Option<Cell> {
        walking
            .steps(floors, cell)
            .iter()
            .filter_map(|&(next, _)| Some((next, *self.distance.get(&next)?)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|&(_, d)| d < self.distance.get(&cell).copied().unwrap_or(f32::MAX))
            .map(|(next, _)| next)
    }
}

/// The walk graph's caches over one building.
#[derive(Default)]
pub struct Walking {
    columns: Mutex<HashMap<(i32, i32), Arc<[f32]>>>,
    steps: Mutex<HashMap<Cell, Arc<[(Cell, f32)]>>>,
    fields: Mutex<Vec<Arc<Field>>>,
    /// The zone's walls and shut doors (`World::set_blocked`): no step or straight line crosses
    /// them.
    blocked: Mutex<Vec<((f32, f32), (f32, f32))>>,
    /// Floor laid over holes (`World::set_platforms`): (centre, radius, height) discs.
    platforms: Mutex<Vec<((f32, f32), f32, f32)>>,
}

fn column_of(v: f32) -> i32 {
    (v / GRID).floor() as i32
}

fn centre(i: i32) -> f32 {
    (i as f32 + 0.5) * GRID
}

/// A min-heap entry for the search.
struct Open(f32, Cell);

impl PartialEq for Open {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Open {}
impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Open {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.total_cmp(&self.0).then_with(|| other.1.cmp(&self.1))
    }
}

impl Walking {
    /// The lines nobody crosses from now on; the steps and fields found before are dropped, as a
    /// door that opened joins two halls.
    pub fn set_blocked(&self, lines: &[((f32, f32), (f32, f32))]) {
        let mut blocked = self.blocked.lock().unwrap();
        if blocked.as_slice() == lines {
            return;
        }
        *blocked = lines.to_vec();
        self.steps.lock().unwrap().clear();
        self.fields.lock().unwrap().clear();
    }

    /// The floor laid over holes from now on; the columns, steps and fields found before are
    /// dropped.
    pub fn set_platforms(&self, discs: &[((f32, f32), f32, f32)]) {
        let mut platforms = self.platforms.lock().unwrap();
        if platforms.as_slice() == discs {
            return;
        }
        *platforms = discs.to_vec();
        self.columns.lock().unwrap().clear();
        self.steps.lock().unwrap().clear();
        self.fields.lock().unwrap().clear();
    }

    /// The heights of the floor laid over holes at `(x, y)`.
    pub fn platform_heights(&self, x: f32, y: f32) -> Vec<f32> {
        self.platforms
            .lock()
            .unwrap()
            .iter()
            .filter(|((cx, cy), r, _)| (x - cx).powi(2) + (y - cy).powi(2) <= r * r)
            .map(|&(_, _, z)| z)
            .collect()
    }

    /// Whether a straight walk from `from` to `to` crosses one of the blocked lines.
    pub fn crosses_blocked(&self, from: (f32, f32), to: (f32, f32)) -> bool {
        self.blocked.lock().unwrap().iter().any(|&l| crosses(from, to, l))
    }

    /// Whether a wall of the building or a blocked line stands between `from` and `to` (knee
    /// height).
    fn walled(&self, floors: &WmoFloors, from: [f32; 3], to: [f32; 3]) -> bool {
        floors.walk_blocked(from, to) || self.crosses_blocked((from[0], from[1]), (to[0], to[1]))
    }

    /// The floors a body can stand on in column `(i, j)`, top first.
    pub fn column(&self, floors: &WmoFloors, i: i32, j: i32) -> Arc<[f32]> {
        if let Some(c) = self.columns.lock().unwrap().get(&(i, j)) {
            return c.clone();
        }
        let mut heights = floors.walk_heights(centre(i), centre(j));
        heights.extend(self.platform_heights(centre(i), centre(j)));
        heights.sort_by(|a, b| b.total_cmp(a));
        heights.dedup_by(|a, b| (*a - *b).abs() < 0.05);
        let standing: Arc<[f32]> = heights
            .iter()
            .enumerate()
            .filter(|&(k, &z)| k == 0 || heights[k - 1] - z >= HEADROOM)
            .map(|(_, &z)| z)
            .collect();
        self.columns.lock().unwrap().insert((i, j), standing.clone());
        standing
    }

    /// The floor cell under `(x, y)` nearest below `near_z` (a step above it counts).
    pub fn cell_at(&self, floors: &WmoFloors, x: f32, y: f32, near_z: f32) -> Option<(Cell, f32)> {
        let (i, j) = (column_of(x), column_of(y));
        let col = self.column(floors, i, j);
        let k = col.iter().position(|&z| z <= near_z + FLOOR_STEP)?;
        Some(((i, j, k as u16), col[k]))
    }

    fn height(&self, floors: &WmoFloors, cell: Cell) -> f32 {
        self.column(floors, cell.0, cell.1)[cell.2 as usize]
    }

    /// The cells a body can step to from `cell`, with the yards each step walks.
    pub fn steps(&self, floors: &WmoFloors, cell: Cell) -> Arc<[(Cell, f32)]> {
        if let Some(s) = self.steps.lock().unwrap().get(&cell) {
            return s.clone();
        }
        let (i, j, _) = cell;
        let z = self.height(floors, cell);
        let mut out = Vec::new();
        for (di, dj) in [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)] {
            let (ni, nj) = (i + di, j + dj);
            let run = GRID * ((di * di + dj * dj) as f32).sqrt();
            let col = self.column(floors, ni, nj);
            let Some((k, &nz)) = col
                .iter()
                .enumerate()
                .filter(|(_, &nz)| (nz - z).abs() <= CLIMB * run)
                .min_by(|a, b| (a.1 - z).abs().total_cmp(&(b.1 - z).abs()))
            else {
                continue;
            };
            let from = [centre(i), centre(j), z + KNEE];
            let to = [centre(ni), centre(nj), nz + KNEE];
            if !self.walled(floors, from, to) {
                out.push(((ni, nj, k as u16), (run * run + (nz - z) * (nz - z)).sqrt()));
            }
        }
        let out: Arc<[(Cell, f32)]> = out.into();
        self.steps.lock().unwrap().insert(cell, out.clone());
        out
    }

    /// The walking distances from the floor under `from` (feet), out to `reach` yards; reused
    /// while the start cell holds.
    pub fn field(&self, floors: &WmoFloors, from: [f32; 3], reach: f32) -> Option<Arc<Field>> {
        let (start, z) = self.cell_at(floors, from[0], from[1], from[2])?;
        let near = |f: &Field| {
            f.reach >= reach
                && (f.start.0 - start.0).abs() <= FIELD_REUSE
                && (f.start.1 - start.1).abs() <= FIELD_REUSE
                && (self.height(floors, f.start) - z).abs() <= FLOOR_STEP
        };
        let known = self.fields.lock().unwrap().iter().find(|f| f.start == start && f.reach >= reach).cloned();
        if let Some(f) = known.or_else(|| self.fields.lock().unwrap().iter().find(|f| near(f)).cloned()) {
            return Some(f);
        }
        let mut distance = HashMap::from([(start, 0.0)]);
        let mut open = BinaryHeap::from([Open(0.0, start)]);
        while let Some(Open(d, cell)) = open.pop() {
            if d > distance[&cell] {
                continue;
            }
            for &(next, step) in self.steps(floors, cell).iter() {
                let nd = d + step;
                if nd <= reach && distance.get(&next).is_none_or(|&known| nd < known) {
                    distance.insert(next, nd);
                    open.push(Open(nd, next));
                }
            }
        }
        let field = Arc::new(Field { start, reach, distance });
        let mut fields = self.fields.lock().unwrap();
        if fields.len() >= FIELDS_KEPT {
            fields.remove(0);
        }
        fields.push(field.clone());
        Some(field)
    }

    /// Where a follower at `from` heads next to reach `hero` (feet): [`FOLLOW_CELLS`] cells down
    /// the hero's field, when a wall or a level stands between them (round a wall, up a ramp);
    /// `None` when it can go straight at the hero on its level, or is not on the field at all.
    /// Of the follower's column it takes the floor nearest the hero by walking (followers keep no
    /// height of their own).
    pub fn toward(&self, floors: &WmoFloors, from: (f32, f32), hero: [f32; 3]) -> Option<(f32, f32)> {
        let field = self.field(floors, hero, FOLLOW_REACH)?;
        let (i, j) = (column_of(from.0), column_of(from.1));
        let col = self.column(floors, i, j);
        let (mut cell, _) = (0..col.len())
            .filter_map(|k| Some(((i, j, k as u16), *field.distance.get(&(i, j, k as u16))?)))
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        let feet = self.height(floors, cell);
        let level = (hero[2] - feet).abs() <= FLOOR_STEP;
        if level && !self.walled(floors, [from.0, from.1, feet + KNEE], [hero[0], hero[1], hero[2] + KNEE]) {
            return None;
        }
        // the furthest of the next cells down the field it can head at in a line no wall crosses
        let mut aim = cell;
        for _ in 0..FOLLOW_CELLS {
            let Some(next) = field.downhill(self, floors, cell) else { break };
            cell = next;
            let to = [centre(cell.0), centre(cell.1), self.height(floors, cell) + KNEE];
            if self.walled(floors, [from.0, from.1, feet + KNEE], to) {
                break;
            }
            aim = cell;
        }
        Some((centre(aim.0), centre(aim.1)))
    }

    /// The walking distance from `from` (feet) to the nearest-by-walking floor at `(x, y)`, and
    /// that floor's height, if one is within `reach` yards.
    pub fn walk(&self, floors: &WmoFloors, from: [f32; 3], to: (f32, f32), reach: f32) -> Option<(f32, f32)> {
        let field = self.field(floors, from, reach)?;
        let (i, j) = (column_of(to.0), column_of(to.1));
        let col = self.column(floors, i, j);
        (0..col.len())
            .filter_map(|k| Some((*field.distance.get(&(i, j, k as u16))?, col[k])))
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Tri = [[f32; 3]; 3];

    /// A level floor over `[x0, x1] × [y0, y1]` at height `z`.
    fn floor(x0: f32, x1: f32, y0: f32, y1: f32, z: f32) -> Vec<Tri> {
        vec![[[x0, y0, z], [x1, y0, z], [x1, y1, z]], [[x0, y0, z], [x1, y1, z], [x0, y1, z]]]
    }

    /// A wall along x = `x` over `[y0, y1]`, from `z0` to `z1`.
    fn wall(x: f32, y0: f32, y1: f32, z0: f32, z1: f32) -> Vec<Tri> {
        vec![[[x, y0, z0], [x, y1, z0], [x, y1, z1]], [[x, y0, z0], [x, y1, z1], [x, y0, z1]]]
    }

    #[test]
    fn a_ramp_joins_the_upper_floor_and_a_ledge_with_no_way_down_does_not() {
        // ground 0..40 × 0..10; a ramp rising 5 yd over x 10..20 (y 0..4) to a deck at 5 over
        // x 20..30 (y 0..4); a ledge at 5 over x 0..8, y 6..10, on posts, with no way up
        let mut walk = floor(0.0, 40.0, 0.0, 10.0, 0.0);
        walk.extend(vec![
            [[10.0, 0.0, 0.0], [20.0, 0.0, 5.0], [20.0, 4.0, 5.0]],
            [[10.0, 0.0, 0.0], [20.0, 4.0, 5.0], [10.0, 4.0, 0.0]],
        ]);
        walk.extend(floor(20.0, 30.0, 0.0, 4.0, 5.0));
        walk.extend(floor(0.0, 8.0, 6.0, 10.0, 5.0));
        let floors = WmoFloors::from_triangles(walk, vec![]);
        let w = Walking::default();
        let hero = [35.0, 8.0, 0.0];
        // the deck's column has the deck over the ground; both are walked to, the deck the long
        // way, up the ramp
        let field = w.field(&floors, hero, 100.0).unwrap();
        assert_eq!(&*w.column(&floors, 25, 2), &[5.0, 0.0]);
        let deck = field.distance.get(&(25, 2, 0)).copied().expect("the deck, up the ramp");
        let under = field.distance[&(25, 2, 1)];
        assert!(deck > under + 10.0, "deck {deck}, under it {under}");
        let (_, z) = w.walk(&floors, hero, (4.5, 8.5), 100.0).expect("the ground under the ledge");
        assert!(z.abs() < 0.01, "only the ground under the ledge: {z}");
        let ledge_feet = [4.5, 8.5, 5.0];
        assert_eq!(w.cell_at(&floors, 4.5, 8.5, 5.0).map(|c| c.1), Some(5.0));
        assert!(
            w.walk(&floors, ledge_feet, (35.5, 8.5), 100.0).is_none(),
            "a ledge on posts reaches nothing"
        );
    }

    #[test]
    fn a_wall_is_walked_around_and_its_far_side_is_further_than_the_crow_flies() {
        let mut walk = floor(0.0, 20.0, 0.0, 20.0, 0.0);
        walk.extend(wall(10.0, 0.0, 15.0, 0.0, 4.0));
        let floors = WmoFloors::from_triangles(walk, vec![]);
        let w = Walking::default();
        let hero = [5.5, 2.5, 0.0];
        let (d, _) = w.walk(&floors, hero, (14.5, 2.5), 100.0).expect("round the wall's end");
        assert!(d > 2.0 * 12.0, "around the end at y 15: {d}");
        // follow the field downhill from the far side, a step at a time, back to the hero
        let field = w.field(&floors, hero, 100.0).unwrap();
        let (mut cell, _) = w.cell_at(&floors, 14.5, 2.5, 0.0).unwrap();
        for _ in 0..200 {
            if cell == field.start {
                break;
            }
            let next = field.downhill(&w, &floors, cell).expect("a way on");
            cell = next;
        }
        assert_eq!(cell, field.start, "the follower reaches the hero");
    }

    /// Moves a follower as the sim does: to `toward`'s point, else straight at the hero, at most
    /// `stride` yards a decision; the moves made, until within 2 yd of the hero.
    pub(crate) fn follow(w: &Walking, floors: &WmoFloors, mut at: (f32, f32), hero: [f32; 3], stride: f32) -> Vec<((f32, f32), (f32, f32))> {
        let mut moves = Vec::new();
        for _ in 0..500 {
            let gap = ((at.0 - hero[0]).powi(2) + (at.1 - hero[1]).powi(2)).sqrt();
            if gap < 2.0 {
                return moves;
            }
            let aim = w.toward(floors, at, hero).unwrap_or((hero[0], hero[1]));
            let d = ((aim.0 - at.0).powi(2) + (aim.1 - at.1).powi(2)).sqrt().max(1e-6);
            let k = (stride / d).min(1.0);
            let next = (at.0 + (aim.0 - at.0) * k, at.1 + (aim.1 - at.1) * k);
            moves.push((at, next));
            at = next;
        }
        panic!("no arrival from {at:?}");
    }

    #[test]
    fn a_follower_goes_round_the_wall_end_and_never_through_it() {
        let mut walk = floor(0.0, 20.0, 0.0, 20.0, 0.0);
        walk.extend(wall(10.0, 0.0, 15.0, 0.0, 4.0));
        let floors = WmoFloors::from_triangles(walk, vec![]);
        let w = Walking::default();
        let hero = [5.5, 2.5, 0.0];
        let moves = follow(&w, &floors, (14.5, 2.5), hero, 3.0);
        for (a, b) in &moves {
            assert!(!floors.walk_blocked([a.0, a.1, KNEE], [b.0, b.1, KNEE]), "{a:?} → {b:?} goes through the wall");
        }
        assert!(moves.iter().any(|(a, _)| a.1 > 14.0), "it went round the end at y 15");
    }
}

#[cfg(test)]
mod real_data {
    use super::*;
    use crate::wmo_floor::real_data::building;

    /// A `.gps` spot inside Dire Maul.
    const SPOT: [f32; 3] = [806.627_26, 482.043, 37.318];

    /// Around that spot in Dire Maul: a floor 3+ yd up that is walked to (a ramp or a
    /// stair joins it), a floor 3+ yd up that is never reached (a wall top, a roof), and a cell
    /// that is walked round to, much further than the crow flies, with a follower going downhill
    /// all the way back.
    #[test]
    fn dire_maul_walks_up_its_stairs_not_onto_its_wall_tops_and_round_its_walls() {
        let Some(dm) = building("DireMaul") else { return };
        let w = Walking::default();
        let field = w.field(&dm, SPOT, 120.0).expect("a floor at the spot");
        let (si, sj) = (column_of(SPOT[0]), column_of(SPOT[1]));
        let (mut upper, mut stranded, mut round) = (None, None, None);
        for i in si - 40..=si + 40 {
            for j in sj - 40..=sj + 40 {
                for (k, &z) in w.column(&dm, i, j).iter().enumerate() {
                    let cell = (i, j, k as u16);
                    let crow = (((i - si).pow(2) + (j - sj).pow(2)) as f32).sqrt() * GRID;
                    match field.distance.get(&cell) {
                        Some(_) if z > SPOT[2] + 3.0 => upper = upper.or(Some(cell)),
                        Some(&d) if crow > 10.0 && d > 1.5 * crow + 5.0 => round = round.or(Some(cell)),
                        None if z > SPOT[2] + 3.0 && crow < 30.0 => stranded = stranded.or(Some(cell)),
                        _ => {}
                    }
                }
            }
        }
        let upper = upper.expect("a higher floor walked to");
        let stranded = stranded.expect("a higher floor with no way up");
        let round = round.expect("a cell walked round to");
        // the walk answers the same: the stranded column's top is never what a walk lands on
        let top = w.column(&dm, stranded.0, stranded.1)[stranded.2 as usize];
        let to = (centre(stranded.0), centre(stranded.1));
        assert!(w.walk(&dm, SPOT, to, 120.0).is_none_or(|(_, z)| z != top), "{stranded:?}");
        assert!(field.distance.contains_key(&upper));
        // a follower from the walked-round cell gets home, every step downhill
        let mut cell = round;
        for _ in 0..1000 {
            if cell == field.start {
                break;
            }
            cell = field.downhill(&w, &dm, cell).expect("a way on");
        }
        assert_eq!(cell, field.start, "the follower from {round:?} reaches the hero");
        // and an enemy moved as the sim moves one gets there too, never through a wall
        let from = (centre(round.0), centre(round.1));
        let moves = super::tests::follow(&w, &dm, from, SPOT, 3.0);
        let z = |p: (f32, f32)| w.cell_at(&dm, p.0, p.1, SPOT[2]).map_or(SPOT[2], |c| c.1);
        let through = moves
            .iter()
            .filter(|(a, b)| dm.walk_blocked([a.0, a.1, z(*a) + KNEE], [b.0, b.1, z(*b) + KNEE]))
            .count();
        assert_eq!(through, 0, "{through} of {} moves from {round:?} go through a wall", moves.len());
    }
}
