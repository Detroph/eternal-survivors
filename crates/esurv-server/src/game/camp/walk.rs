//! The heroes' walks about the camp (plans/hub-hero-life.md, section 4; two or
//! three away from their tents at a time): the places they go, what each does there, and the way
//! there round what stands in the camp.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::{CampPlace, Game, Keeper, Piece, Spot};
use crate::game::dist;
use crate::walk_graph::{CLIMB, GRID, HEADROOM, KNEE};
use crate::wmo_floor::{WmoFloors, FLOOR_STEP};
use esurv_content::camp_looks::{CAMP_GOL_BOLAR_PLATFORM_DISPLAY, CAMP_NEUTRAL_STAIRS_DISPLAY, CAMP_TROLL_SHOP_DISPLAY, CAMP_UNDEAD_PLATFORM_DISPLAY, CAMP_VENDOR_TENT_LOOK, CAMP_WAR_PAVILION_LOOK};

/// What a hero does at a place it walks to (who may, [`Doing::suits`]: the advisor's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Doing {
    /// Sits on a fire log, facing the flames (sitting only on a seat or a log),
    /// eating now and then, a weary one dozing off, a rogue or warrior with its blade out.
    Sit,
    /// Kneels in prayer at the fire's edge.
    Pray,
    /// Hammers at the anvil (the hammering only there).
    Hammer,
    /// Handles the weapons on the rack, its own drawn, now and then trying a blade's weight.
    LookOver,
    /// Looks the armour stand over, now and then saluting it.
    Inspect,
    /// Reads at the lectern, now and then wondering or struck by something.
    Read,
    /// Peers into the Transmutation Orb.
    Peer,
    /// Drills at a training dummy (the attacks only there): a fighter swings its
    /// own weapon, a caster goes through a cast, a hunter readies its bow.
    Spar,
    /// Keeps watch at the camp's edge, facing out, now and then pointing.
    Watch,
    /// Talks with another hero, the two standing face to face somewhere in the camp: gestures mostly, now and then a line.
    Talk,
}

/// The places that go together: a hero out stays with one a while, moving now and then to
/// another place of it free (grouped, five to seven minutes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Fire,
    Armoury,
    Study,
    Training,
    Watch,
    Talk,
}

/// The classes that fight with weapons in hand (the advisor's).
const FIGHTERS: [&str; 5] = ["warrior", "paladin", "rogue", "hunter", "shaman"];

impl Doing {
    pub fn group(self) -> Group {
        match self {
            Doing::Sit | Doing::Pray => Group::Fire,
            Doing::Hammer | Doing::LookOver | Doing::Inspect => Group::Armoury,
            Doing::Read | Doing::Peer => Group::Study,
            Doing::Spar => Group::Training,
            Doing::Watch => Group::Watch,
            Doing::Talk => Group::Talk,
        }
    }

    /// Whether a hero of `class` and `race`, of temperaments `temperaments`, does this (the
    /// advisor's): smiths are fighters, and dwarves and gnomes whatever their
    /// class; casters read and look into the Orb; a priest prays and never spars.
    pub fn suits(self, class: &str, race: &str, temperaments: &[String]) -> bool {
        let is = |t: &str| temperaments.iter().any(|h| h == t);
        let of = |classes: &[&str]| classes.contains(&class);
        match self {
            Doing::Sit | Doing::Talk => true,
            Doing::Pray => of(&["priest", "paladin"]) || is("devout"),
            Doing::Hammer => of(&FIGHTERS) || matches!(race, "dwarf" | "gnome"),
            Doing::LookOver => of(&FIGHTERS),
            Doing::Inspect => of(&["paladin", "warrior"]),
            Doing::Read => of(&["mage", "priest", "warlock", "druid", "paladin", "shaman"]) || is("curious"),
            Doing::Peer => of(&["mage", "warlock"]) || is("curious"),
            Doing::Spar => class != "priest",
            Doing::Watch => of(&["warrior", "paladin", "hunter"]),
        }
    }
}

/// A place a hero walks to: where it stands, the way it faces there (a world angle), and what it
/// does there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Haunt {
    pub at: (f32, f32),
    pub facing: f32,
    pub doing: Doing,
    /// How high above the ground it is there, yards (on a log, sitting).
    pub lift: f32,
}

/// The decoration a hero spars with.
const TRAINING_DUMMY: &str = "training_dummy";
/// How far before a training dummy, toward the fire, a hero spars with it, yards.
const SPAR_FROM: f32 = 1.8;

/// Ground a hero walks round: from `x.0` to `x.1` yards ahead of `at` (its facing, a world angle)
/// and from `y.0` to `y.1` to its left.
#[derive(Clone, Copy, Debug)]
pub(super) struct Block {
    at: (f32, f32),
    facing: f32,
    x: (f32, f32),
    y: (f32, f32),
}

impl Block {
    /// A square `half` yards each way of `at`.
    fn square(at: (f32, f32), half: f32) -> Self {
        Block { at, facing: 0.0, x: (-half, half), y: (-half, half) }
    }

    /// Whether `p` is on it, or within `margin` yards of it.
    fn holds(&self, p: (f32, f32), margin: f32) -> bool {
        let (dx, dy) = (p.0 - self.at.0, p.1 - self.at.1);
        let (s, c) = self.facing.sin_cos();
        let ahead = dx * c + dy * s;
        let left = -dx * s + dy * c;
        ahead >= self.x.0 - margin && ahead <= self.x.1 + margin && left >= self.y.0 - margin && left <= self.y.1 + margin
    }

    /// How far from `at` it reaches at most, with `margin`, yards.
    fn radius(&self, margin: f32) -> f32 {
        let a = self.x.0.abs().max(self.x.1.abs()) + margin;
        let l = self.y.0.abs().max(self.y.1.abs()) + margin;
        a.hypot(l)
    }
}

/// What a piece of the camp takes up on the ground at size 1, yards ahead and to its left
/// (model_sizes), where that is not its footprint: the tents' and the wagon's bodies, the forge
/// and the anvil, the platform's deck. `None`: nothing in the way (a rug, a piece set on another).
fn taken(p: &Piece) -> Option<((f32, f32), (f32, f32))> {
    let prop = p.prop;
    if prop.is_some_and(|q| q.set_on.is_some() && q.footprint.is_none()) && !matches!(p.display, CAMP_GOL_BOLAR_PLATFORM_DISPLAY | CAMP_UNDEAD_PLATFORM_DISPLAY) {
        return None;
    }
    let sized = match (p.look, p.display) {
        (Some(CAMP_WAR_PAVILION_LOOK), _) => Some(((-4.35, 4.40), (-4.32, 4.35))),
        (Some(CAMP_VENDOR_TENT_LOOK), _) => Some(((-2.2, 4.93), (-3.16, 3.33))),
        // the gypsy wagon without its awning (2.6 to 4.5 to its left), which its trainer and anvil
        // stand under
        (_, 5392) => Some(((-4.57, 6.91), (-2.61, 2.6))),
        (_, 233) => Some(((-0.16, 2.17), (-1.18, 2.34))), // Tradeskill_Forge_01
        (_, 273) => Some(((-1.23, 0.65), (-0.56, 0.56))), // Tradeskill_Anvil_01
        (_, 197) => Some(((-0.92, 0.92), (-0.92, 0.92))), // DwarvenBrazier02
        (_, 289) => Some(((-0.63, 0.99), (-1.08, 0.57))), // FirewoodPile03
        (_, 159) => None,                                 // StormwindRug01: walked on
        // Gol'Bolar's platform: its deck to the left of its stairs, which run down its right side
        // to its origin
        (_, CAMP_GOL_BOLAR_PLATFORM_DISPLAY | CAMP_UNDEAD_PLATFORM_DISPLAY) => Some(((-7.4, 13.3), (1.0, 17.3))),
        _ => None,
    };
    sized
        .or_else(|| prop.and_then(|q| q.footprint).map(|f| (f.x, f.y)))
        .or_else(|| prop.is_some().then(|| (SMALL, SMALL)).map(|(x, y)| ((x.0 / p.scale, x.1 / p.scale), (y.0 / p.scale, y.1 / p.scale))))
}

/// The troll shop's walls at size 1, yards ahead and to its left, round its round room (-10.5 to
/// 4.0 ahead, -7.0 to 6.5 left), open at its door ahead, -2.5 to 4.5 left, where its stairs run
/// down (its WMO), and its stairs' rails, open at their foot: the heroes walk in and up to the Mask
///.
const TROLL_SHOP_WALLS: [((f32, f32), (f32, f32)); 7] = [
    ((-10.5, -9.0), (-7.0, 6.5)),
    ((-10.5, 4.0), (5.0, 6.5)),
    ((-10.5, 4.0), (-7.0, -5.5)),
    ((2.5, 4.0), (4.5, 6.5)),
    ((2.5, 4.0), (-7.0, -2.5)),
    ((4.0, 9.5), (-3.0, -2.5)),
    ((4.0, 9.5), (4.5, 5.0)),
];
/// The troll shop's floor over its origin at size 1, and its stairs: from its door, 4.0 ahead, down
/// 0.53 a yard to their foot, 10.5 ahead, -2.5 to 4.5 left (its WMO).
const TROLL_SHOP_FLOOR: f32 = 4.3;
const TROLL_SHOP_STAIRS: ((f32, f32), (f32, f32)) = ((4.0, 10.5), (-2.5, 4.5));
const TROLL_SHOP_STAIRS_FALL: f32 = 0.53;

/// Ground a hero walks up onto, above the world's: the troll shop's floor and its stairs.
#[derive(Clone, Copy, Debug)]
pub(super) struct Rise {
    at: (f32, f32),
    facing: f32,
    scale: f32,
    /// The height of its origin.
    base: f32,
}

impl Rise {
    /// How far it reaches from `at` at most, yards.
    fn reach(&self) -> f32 {
        10.5 * self.scale + 1.0
    }

    /// Its height at `p`, if `p` is on it.
    pub(super) fn height(&self, p: (f32, f32)) -> Option<f32> {
        let (dx, dy) = (p.0 - self.at.0, p.1 - self.at.1);
        let (s, c) = self.facing.sin_cos();
        let ahead = (dx * c + dy * s) / self.scale;
        let left = (-dx * s + dy * c) / self.scale;
        let ((x0, x1), (y0, y1)) = TROLL_SHOP_STAIRS;
        let over = if (-10.5..=x0).contains(&ahead) && (-7.0..=6.5).contains(&left) {
            TROLL_SHOP_FLOOR
        } else if (x0..=x1).contains(&ahead) && (y0..=y1).contains(&left) {
            TROLL_SHOP_FLOOR - (ahead - x0) * TROLL_SHOP_STAIRS_FALL
        } else {
            return None;
        };
        Some(self.base + over * self.scale)
    }

    /// Whether a walk from `a` to `b` passes over it.
    pub(super) fn crossed(&self, a: (f32, f32), b: (f32, f32)) -> bool {
        let (ab, ap) = ((b.0 - a.0, b.1 - a.1), (self.at.0 - a.0, self.at.1 - a.1));
        let t = ((ap.0 * ab.0 + ap.1 * ab.1) / (ab.0 * ab.0 + ab.1 * ab.1).max(f32::EPSILON)).clamp(0.0, 1.0);
        dist((a.0 + ab.0 * t, a.1 + ab.1 * t), self.at) <= self.reach()
    }
}

/// The height a hero walks at, at `p`: on a rise if it is on one, else the world's ground.
pub(super) fn walked_height(rises: &[Rise], p: (f32, f32), ground: impl Fn((f32, f32)) -> f32) -> f32 {
    let g = ground(p);
    rises.iter().filter_map(|r| r.height(p)).fold(g, f32::max)
}

/// A small thing's ground, yards each way of it.
const SMALL: (f32, f32) = (-0.5, 0.5);
/// A person's, yards each way.
const PERSON: f32 = 0.4;

/// The camp's ground as a grid of cells, each free to walk or in the way.
#[derive(Clone)]
pub(super) struct Walls {
    /// The south-east corner of the grid's first cell, world yards.
    origin: (f32, f32),
    side: usize,
    blocked: Vec<bool>,
    /// The ground above the world's the heroes walk up onto.
    pub(super) rises: Vec<Rise>,
    /// The stairs from the ledge down into the basins.
    pub(super) stairs: Vec<Stair>,
}

/// A cell's side, yards.
const CELL: f32 = 0.5;
/// How near a walking hero comes to anything, yards: half its width and a little.
const CLEAR: f32 = 0.5;
/// A hero leaves from and comes to its place even when that stands close by something: this near
/// it, yards, the ground counts as free.
pub(super) const NEAR_ENDS: f32 = 1.5;

impl Walls {
    /// Nothing in the way.
    pub(super) fn open() -> Self {
        Walls { origin: (0.0, 0.0), side: 0, blocked: Vec::new(), rises: Vec::new(), stairs: Vec::new() }
    }

    /// The ground within `reach` of `middle`, `blocks` in the way.
    fn new(blocks: &[Block], middle: (f32, f32), reach: f32) -> Self {
        let side = (2.0 * reach / CELL).ceil() as usize;
        let origin = (middle.0 - reach, middle.1 - reach);
        let mut walls = Walls { origin, side, blocked: vec![false; side * side], rises: Vec::new(), stairs: Vec::new() };
        for b in blocks {
            let r = b.radius(CLEAR);
            let (lo, hi) = (walls.clamped((b.at.0 - r, b.at.1 - r)), walls.clamped((b.at.0 + r, b.at.1 + r)));
            for i in lo.0..=hi.0 {
                for j in lo.1..=hi.1 {
                    if b.holds(walls.middle((i, j)), CLEAR) {
                        walls.blocked[i * side + j] = true;
                    }
                }
            }
        }
        walls
    }

    /// The cell nearest `p`, on the grid.
    fn clamped(&self, p: (f32, f32)) -> (usize, usize) {
        let at = |v: f32| ((v / CELL).max(0.0) as usize).min(self.side.saturating_sub(1));
        (at(p.0 - self.origin.0), at(p.1 - self.origin.1))
    }

    /// The cell `p` is in, if inside.
    fn cell(&self, p: (f32, f32)) -> Option<(usize, usize)> {
        let (i, j) = ((p.0 - self.origin.0) / CELL, (p.1 - self.origin.1) / CELL);
        (i >= 0.0 && j >= 0.0 && (i as usize) < self.side && (j as usize) < self.side).then_some((i as usize, j as usize))
    }

    fn middle(&self, (i, j): (usize, usize)) -> (f32, f32) {
        (self.origin.0 + (i as f32 + 0.5) * CELL, self.origin.1 + (j as f32 + 0.5) * CELL)
    }

    /// Every cell the straight stretch from `a` to `b` passes through, even at a corner, in order;
    /// `None` for ground off the grid.
    fn crossed(&self, a: (f32, f32), b: (f32, f32)) -> Vec<Option<(usize, usize)>> {
        let grid = |p: (f32, f32)| ((p.0 - self.origin.0) / CELL, (p.1 - self.origin.1) / CELL);
        let ((x, y), (to_x, to_y)) = (grid(a), grid(b));
        let (mut i, mut j) = (x.floor() as i64, y.floor() as i64);
        let (end_i, end_j) = (to_x.floor() as i64, to_y.floor() as i64);
        let (dx, dy) = (to_x - x, to_y - y);
        let (step_i, step_j) = (dx.signum() as i64, dy.signum() as i64);
        // how far along the stretch (0 to 1) the next line across each way is, and between lines
        let across = |v: f32, d: f32| if d > 0.0 { (v.floor() + 1.0 - v) / d } else if d < 0.0 { (v - v.floor()) / -d } else { f32::INFINITY };
        let (mut next_i, mut next_j) = (across(x, dx), across(y, dy));
        let (every_i, every_j) = (if dx != 0.0 { 1.0 / dx.abs() } else { f32::INFINITY }, if dy != 0.0 { 1.0 / dy.abs() } else { f32::INFINITY });
        let on_grid = |i: i64, j: i64| (i >= 0 && j >= 0 && (i as usize) < self.side && (j as usize) < self.side).then_some((i as usize, j as usize));
        let mut cells = vec![on_grid(i, j)];
        while (i, j) != (end_i, end_j) && next_i.min(next_j) <= 1.0 {
            if (next_i - next_j).abs() < 1e-6 {
                // through a corner: both cells beside it too
                cells.push(on_grid(i + step_i, j));
                cells.push(on_grid(i, j + step_j));
                i += step_i;
                j += step_j;
                next_i += every_i;
                next_j += every_j;
            } else if next_i < next_j {
                i += step_i;
                next_i += every_i;
            } else {
                j += step_j;
                next_j += every_j;
            }
            cells.push(on_grid(i, j));
        }
        cells
    }

    /// Whether `p` is in the way.
    pub(super) fn in_the_way(&self, p: (f32, f32)) -> bool {
        self.cell(p).is_none_or(|(i, j)| self.blocked[i * self.side + j])
    }

    /// The way from `from` to `to` round what is in the way: its corners, `from` first and `to`
    /// last. `None` when there is none.
    pub(super) fn route(&self, from: (f32, f32), to: (f32, f32)) -> Option<Vec<(f32, f32)>> {
        if self.side == 0 {
            return Some(vec![from, to]);
        }
        let n = self.side;
        let index = |(i, j): (usize, usize)| i * n + j;
        let free_cell = |c: (usize, usize)| {
            let m = self.middle(c);
            dist(m, from) <= NEAR_ENDS || dist(m, to) <= NEAR_ENDS || !self.blocked[index(c)]
        };
        let free = |p: (f32, f32)| self.cell(p).is_some_and(free_cell);
        let (start, goal) = (self.cell(from)?, self.cell(to)?);
        let guess = |(i, j): (usize, usize)| {
            let (di, dj) = (i.abs_diff(goal.0) as u32, j.abs_diff(goal.1) as u32);
            10 * di.max(dj) + 4 * di.min(dj)
        };
        let mut cost = vec![u32::MAX; n * n];
        let mut came = vec![usize::MAX; n * n];
        let mut open = BinaryHeap::new();
        cost[index(start)] = 0;
        open.push(Reverse((guess(start), index(start))));
        while let Some(Reverse((_, k))) = open.pop() {
            let here = (k / n, k % n);
            if here == goal {
                break;
            }
            for (di, dj) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)] {
                let (Some(i), Some(j)) = (here.0.checked_add_signed(di), here.1.checked_add_signed(dj)) else { continue };
                if i >= n || j >= n || !free(self.middle((i, j))) {
                    continue;
                }
                // never across a corner
                if di != 0 && dj != 0 && !(free(self.middle((i, here.1))) && free(self.middle((here.0, j)))) {
                    continue;
                }
                let step = if di != 0 && dj != 0 { 14 } else { 10 };
                let c = cost[k] + step;
                if c < cost[index((i, j))] {
                    cost[index((i, j))] = c;
                    came[index((i, j))] = k;
                    open.push(Reverse((c + guess((i, j)), index((i, j)))));
                }
            }
        }
        if cost[index(goal)] == u32::MAX {
            return None;
        }
        let mut cells = vec![to];
        let mut k = came[index(goal)];
        while k != usize::MAX && k != index(start) {
            cells.push(self.middle((k / n, k % n)));
            k = came[k];
        }
        cells.push(from);
        cells.reverse();
        // pulled straight: from each corner, on to the farthest point it sees
        let clear = |a: (f32, f32), b: (f32, f32)| self.crossed(a, b).into_iter().all(|c| c.is_some_and(free_cell));
        let mut corners = vec![from];
        let mut at = 0;
        while at + 1 < cells.len() {
            let mut next = at + 1;
            while next + 1 < cells.len() && clear(cells[at], cells[next + 1]) {
                next += 1;
            }
            corners.push(cells[next]);
            at = next;
        }
        Some(corners)
    }
}

/// How finely a guide's straight stretch is walked to see it clear, yards: the step of the client's
/// move points (a quarter yard).
const MARCH: f32 = 0.25;
/// The cells walked out from a guide's goal to see it is not walled in: the camps' walled-in
/// places hold at most about 1,300.
const POCKET: usize = 5_000;
/// How far the line the client draws between a guide's points may stray from the ground walked,
/// yards: the move's own step.
pub(super) const SAG: f32 = 0.25;
/// A stair's tread, yards up it from its origin along its facing (StrangleRuinStair's WMO: from 4
/// below to 7 above): where a guide sets foot on it near its top, and near its foot, where its
/// tread is clear of the south basin's sand.
const STAIR_TOP: f32 = 5.5;
const STAIR_FOOT: f32 = -2.0;

/// A stair from the ledge down into a basin: where it stands and its facing, up it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Stair {
    pub(super) at: (f32, f32),
    pub(super) facing: f32,
}

impl Stair {
    /// The point `ahead` yards up its tread's middle.
    pub(super) fn up(&self, ahead: f32) -> (f32, f32) {
        (self.at.0 + ahead * self.facing.cos(), self.at.1 + ahead * self.facing.sin())
    }
}

/// The floors at `p` a body stands on, highest first: each with room for it above.
pub(super) fn standing(floors: &WmoFloors, p: (f32, f32)) -> Vec<f32> {
    let mut heights = floors.walk_heights(p.0, p.1);
    heights.sort_by(|a, b| b.total_cmp(a));
    heights.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    heights.iter().enumerate().filter(|&(k, &z)| k == 0 || heights[k - 1] - z >= HEADROOM).map(|(_, &z)| z).collect()
}

/// Where a body stands at `p` on `floors`, come from height `from` close by: the floor nearest it
/// with room above, a step up or down from it at most (a climb over a yard, as the runs' walking
/// takes a stair's risers), so it keeps to the ground it walks on rather than climb onto all it
/// passes.
pub(super) fn stand(floors: &WmoFloors, p: (f32, f32), from: f32) -> Option<f32> {
    standing(floors, p).into_iter().filter(|&z| (z - from).abs() <= CLIMB * GRID).min_by(|a, b| (a - from).abs().total_cmp(&(b - from).abs()))
}

/// Whether a body climbs from `a` to `b`, a yard or more apart or one step: no steeper than
/// [`CLIMB`] over it.
fn climbs(a: [f32; 3], b: [f32; 3]) -> bool {
    (b[2] - a[2]).abs() <= CLIMB * (b[0] - a[0]).hypot(b[1] - a[1]).max(GRID)
}

/// The highest floor at `p` with room above, nearest below `near` (a stair step above counts).
pub(super) fn floor_near(floors: &WmoFloors, p: (f32, f32), near: f32) -> Option<f32> {
    standing(floors, p).into_iter().find(|&z| z <= near + FLOOR_STEP)
}

impl Walls {
    /// The way a guide walks from `from` to `to` over the camp's own ground `floors`: round what stands in the
    /// camp, up and down no steeper than a body climbs, never through a wall, a rail or a rock nor
    /// under a floor, and between the ledge and a basin by the stair (`to_z`, the height `to` is
    /// walked at, tells which). The points the client draws its line through, each with its
    /// height, `from` first; `None` when there is no way.
    pub(super) fn guide_route(&self, floors: &WmoFloors, from: [f32; 3], to: (f32, f32), to_z: f32) -> Option<Vec<[f32; 3]>> {
        let mut ends = vec![to];
        // the stair by the lower end, if the two ends are either side of its middle height
        let low = if from[2] < to_z { (from[0], from[1]) } else { to };
        let stair = self.stairs.iter().min_by(|a, b| dist(a.up(STAIR_FOOT), low).total_cmp(&dist(b.up(STAIR_FOOT), low)));
        if let Some(s) = stair {
            let (top, foot) = (s.up(STAIR_TOP), s.up(STAIR_FOOT));
            if let (Some(zt), Some(zf)) = (standing(floors, top).first().copied(), standing(floors, foot).first().copied()) {
                let middle = (zt + zf) / 2.0;
                if (from[2] > middle) != (to_z > middle) {
                    ends = if from[2] > middle { vec![top, foot, to] } else { vec![foot, top, to] };
                }
            }
        }
        // a place the ground walls in is given up at once, not after the whole camp is searched
        // for a way into it
        let mut walled_in = false;
        let by_to = |p: (f32, f32)| dist(p, to) <= NEAR_ENDS;
        let _ = self.climb(floors, [to.0, to.1, to_z], (from[0], from[1]), by_to, POCKET, &mut walled_in);
        if walled_in {
            return None;
        }
        let mut way = vec![from];
        for (k, &end) in ends.iter().enumerate() {
            let here = *way.last()?;
            // only the walk's own two ends may stand close by something
            let last = k + 1 == ends.len();
            let near = |p: (f32, f32)| dist(p, (from[0], from[1])) <= NEAR_ENDS || (last && dist(p, to) <= NEAR_ENDS);
            way.extend(self.climb(floors, here, end, near, usize::MAX, &mut walled_in)?.into_iter().skip(1));
        }
        Some(way)
    }

    /// One stretch of [`Walls::guide_route`]: from `from` to `to`, the ground `near` counts as free
    /// of what stands in the camp, looked for from `looks` cells at most; `walled_in` set when
    /// there is none for all the ground about `from` was walked.
    fn climb(&self, floors: &WmoFloors, from: [f32; 3], to: (f32, f32), near: impl Fn((f32, f32)) -> bool, looks: usize, walled_in: &mut bool) -> Option<Vec<[f32; 3]>> {
        let n = self.side;
        if n == 0 {
            return None;
        }
        let index = |(i, j): (usize, usize)| i * n + j;
        let free_cell = |c: (usize, usize)| near(self.middle(c)) || !self.blocked[index(c)];
        let knee = |p: [f32; 3]| [p[0], p[1], p[2] + KNEE];
        // the steps to a yard back
        let yard = (GRID / MARCH).ceil() as usize;
        // walked from `a` to `b` a quarter yard at a time (`before`, where it was a step before
        // `a`): the ground under each step, if each is clear and none steeper than a climb
        let walk = |before: Option<[f32; 3]>, a: [f32; 3], b: (f32, f32)| -> Option<Vec<[f32; 3]>> {
            let steps = (dist((a[0], a[1]), b) / MARCH).ceil().max(1.0) as usize;
            let mut out = vec![a];
            for s in 1..=steps {
                let t = s as f32 / steps as f32;
                let p = (a[0] + (b.0 - a[0]) * t, a[1] + (b.1 - a[1]) * t);
                let last = *out.last()?;
                let z = stand(floors, p, last[2])?;
                let back = if out.len() > yard { out[out.len() - yard] } else { before.unwrap_or(a) };
                if !self.cell(p).is_some_and(free_cell) || !climbs(back, [p.0, p.1, z]) || floors.walk_blocked(knee(last), knee([p.0, p.1, z])) {
                    return None;
                }
                out.push([p.0, p.1, z]);
            }
            Some(out)
        };
        let (start, goal) = (self.cell((from[0], from[1]))?, self.cell(to)?);
        let guess = |(i, j): (usize, usize)| {
            let (di, dj) = (i.abs_diff(goal.0) as u32, j.abs_diff(goal.1) as u32);
            10 * di.max(dj) + 4 * di.min(dj)
        };
        let mut cost = vec![u32::MAX; n * n];
        let mut came = vec![usize::MAX; n * n];
        // where in each cell it stands: its middle, and `from` in the first
        let mut stood = vec![[f32::NAN; 3]; n * n];
        let mut open = BinaryHeap::new();
        cost[index(start)] = 0;
        stood[index(start)] = from;
        open.push(Reverse((guess(start), index(start))));
        let mut looked = 0;
        while let Some(Reverse((_, k))) = open.pop() {
            let here = (k / n, k % n);
            looked += 1;
            if here == goal || looked > looks {
                break;
            }
            let before = (came[k] != usize::MAX).then(|| stood[came[k]]);
            for (di, dj) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)] {
                let (Some(i), Some(j)) = (here.0.checked_add_signed(di), here.1.checked_add_signed(dj)) else { continue };
                if i >= n || j >= n || !free_cell((i, j)) {
                    continue;
                }
                // never across a corner
                if di != 0 && dj != 0 && !(free_cell((i, here.1)) && free_cell((here.0, j))) {
                    continue;
                }
                let step = if di != 0 && dj != 0 { 14 } else { 10 };
                let c = cost[k] + step;
                if c >= cost[index((i, j))] {
                    continue;
                }
                let Some(walked) = walk(before, stood[k], self.middle((i, j))) else { continue };
                cost[index((i, j))] = c;
                came[index((i, j))] = k;
                stood[index((i, j))] = *walked.last()?;
                open.push(Reverse((c + guess((i, j)), index((i, j)))));
            }
        }
        if cost[index(goal)] == u32::MAX {
            *walled_in = open.is_empty();
            return None;
        }
        let mut cells = Vec::new();
        let mut k = index(goal);
        while k != usize::MAX {
            cells.push(stood[k]);
            k = came[k];
        }
        cells.reverse();
        // on from the last cell's middle to `to` itself
        let last = *cells.last()?;
        let end = walk(None, last, to).or_else(|| (dist((last[0], last[1]), to) <= MARCH).then(|| vec![last, [to.0, to.1, last[2]]]))?;
        cells.push(*end.last()?);
        // pulled straight: from each corner on to the farthest point it walks to clear, on the
        // same floor the search did; a stretch to the next cell is the search's own
        let reach = |a: [f32; 3], b: [f32; 3]| -> Option<Vec<[f32; 3]>> {
            if !self.crossed((a[0], a[1]), (b[0], b[1])).into_iter().all(|c| c.is_some_and(free_cell)) {
                return None;
            }
            let out = walk(None, a, (b[0], b[1]))?;
            ((out.last()?[2] - b[2]).abs() < 0.05).then_some(out)
        };
        let mut way = vec![from];
        let mut at = 0;
        while at + 1 < cells.len() {
            let here = cells[at];
            let mut next = at + 1;
            let mut walked = reach(here, cells[next]).or_else(|| walk(None, here, (cells[next][0], cells[next][1])))?;
            while next + 1 < cells.len() {
                let Some(further) = reach(here, cells[next + 1]) else { break };
                walked = further;
                next += 1;
            }
            way.extend(fewest(&walked).into_iter().skip(1));
            at = next;
        }
        Some(way)
    }
}

/// The fewest of the points of a straight stretch walked, its ends kept, the line between each two
/// straying from the ground walked no more than [`SAG`].
fn fewest(walked: &[[f32; 3]]) -> Vec<[f32; 3]> {
    fn keep(walked: &[[f32; 3]], out: &mut Vec<[f32; 3]>) {
        let (a, b) = (walked[0], walked[walked.len() - 1]);
        let line = |p: [f32; 3]| {
            let (d, l) = ((p[0] - a[0]).hypot(p[1] - a[1]), (b[0] - a[0]).hypot(b[1] - a[1]));
            a[2] + (b[2] - a[2]) * if l > 0.0 { d / l } else { 0.0 }
        };
        let off = |p: [f32; 3]| (p[2] - line(p)).abs();
        let worst = (1..walked.len().saturating_sub(1)).max_by(|&i, &j| off(walked[i]).total_cmp(&off(walked[j])));
        match worst.filter(|&i| off(walked[i]) > SAG) {
            Some(i) => {
                keep(&walked[..=i], out);
                keep(&walked[i..], out);
            }
            None => out.push(b),
        }
    }
    let mut out = vec![walked[0]];
    if walked.len() > 1 {
        keep(walked, &mut out);
    }
    out
}

impl CampPlace {
    /// Where on the ground a spot of the layout is, and the way it faces, as a place to walk to.
    fn haunt(&self, doing: Doing, s: Spot, lift: f32) -> Haunt {
        Haunt { at: self.at(s), facing: self.facing(s), doing, lift }
    }
}

impl Game {
    /// The places of the camp the heroes walk to (the layout's, and before each training dummy set
    /// out), and the ground they walk on, every piece, person and thing in the way.
    pub(super) fn camp_walks(&self) -> (Vec<Haunt>, Walls) {
        let Some(camp) = &self.camp else {
            return (Vec::new(), Walls::open());
        };
        let place = camp.place;
        let layout = place.layout;
        let fire = place.fire();
        let mut haunts: Vec<Haunt> = layout.haunts.iter().map(|&(doing, s, lift)| place.haunt(doing, s, lift)).collect();
        let mut blocks: Vec<Block> = Vec::new();
        let mut rises: Vec<Rise> = Vec::new();
        let mut stairs: Vec<Stair> = Vec::new();
        for p in &camp.pieces {
            let s = p.scale;
            if p.display == CAMP_NEUTRAL_STAIRS_DISPLAY {
                stairs.push(Stair { at: p.at, facing: p.facing });
            }
            // the troll shop: its walls, its door open, and its stairs and floor walked up onto
            if p.display == CAMP_TROLL_SHOP_DISPLAY {
                for (x, y) in TROLL_SHOP_WALLS {
                    blocks.push(Block { at: p.at, facing: p.facing, x: (x.0 * s, x.1 * s), y: (y.0 * s, y.1 * s) });
                }
                rises.push(Rise { at: p.at, facing: p.facing, scale: s, base: self.camp_ground(place, p.at) });
                continue;
            }
            let (x, y) = if let Some(t) = taken(p) {
                t
            } else if p.prop.is_none() && p.look.is_none() {
                // a banner, a trophy, a decoration shown in full
                ((SMALL.0 / p.scale, SMALL.1 / p.scale), (SMALL.0 / p.scale, SMALL.1 / p.scale))
            } else {
                continue;
            };
            blocks.push(Block { at: p.at, facing: p.facing, x: (x.0 * s, x.1 * s), y: (y.0 * s, y.1 * s) });
        }
        for (_, s) in layout.services {
            blocks.push(Block::square(place.at(*s), SMALL.1));
        }
        // the people of the camp where they stand (Wren walks: she is not in the way)
        for (k, s) in layout.keepers.iter().filter(|(k, _)| *k != Keeper::Wren) {
            if !layout.seats.iter().chain(layout.stands_on).any(|(w, _)| w == k) {
                blocks.push(Block::square(place.at(*s), PERSON));
            }
        }
        // the decorations set out, a training dummy with a place to spar before it
        let on = self.hub_decorations(place);
        for (id, s) in super::decoration_spots(place) {
            let Some(piece) = on.get(&id) else { continue };
            let at = place.at(s);
            blocks.push(Block::square(at, SMALL.1));
            if piece == TRAINING_DUMMY {
                let d = dist(at, fire).max(f32::EPSILON);
                let from = (at.0 + (fire.0 - at.0) / d * SPAR_FROM, at.1 + (fire.1 - at.1) / d * SPAR_FROM);
                haunts.push(Haunt { at: from, facing: (at.1 - from.1).atan2(at.0 - from.0), doing: Doing::Spar, lift: 0.0 });
            }
        }
        // the heroes at their tents
        for i in 0..layout.plots.len() {
            if let Some((at, _)) = place.hero_stand(i) {
                blocks.push(Block::square(at, PERSON));
            }
        }
        let mut walls = Walls::new(&blocks, place.middle(), layout.reach);
        walls.rises = rises;
        walls.stairs = stairs;
        (haunts, walls)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::Content;

    use super::*;
    use crate::game::camp::{camp_content, CampPlace, CAMP_HOUR, NEUTRAL_ALLIANCE_CAMP, NEUTRAL_HORDE_CAMP};

    /// The camp at `place` with a hero at every tent.
    fn a_full_camp(place: CampPlace) -> Game {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).unwrap();
        let saves = Arc::new(crate::save::Saves::in_memory());
        let names = ["Aria", "Brom", "Cael", "Dorn", "Elra", "Fenn", "Gilda", "Hale", "Ivo"];
        let heroes: Vec<_> = names.iter().map(|n| saves.create(n, "warrior", "dwarf", 0, [0; 5]).unwrap()).collect();
        let (own, zone) = camp_content(&content, place, CAMP_HOUR).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(own, None, zone, &heroes[0], saves, inputs).unwrap();
        game.make_camp(place, heroes[0].guid, &heroes[1..], Default::default());
        game
    }

    #[test]
    fn every_tent_has_a_clear_way_to_every_place_and_back() {
        clear_ways(NEUTRAL_ALLIANCE_CAMP);
        clear_ways(NEUTRAL_HORDE_CAMP);
    }

    fn clear_ways(place: CampPlace) {
        let game = a_full_camp(place);
        let (haunts, walls) = game.camp_walks();
        assert_eq!(haunts.len(), place.layout.haunts.len(), "no dummy set out: the layout's places only");
        for (k, h) in haunts.iter().enumerate() {
            assert!(!walls.in_the_way(h.at) || walls.route(h.at, h.at).is_some(), "place {k} stands clear");
        }
        for plot in 1..place.layout.plots.len() {
            let (home, _) = place.hero_stand(plot).unwrap();
            for (k, h) in haunts.iter().enumerate() {
                for (from, to) in [(home, h.at), (h.at, home)] {
                    let route = walls.route(from, to).unwrap_or_else(|| panic!("plot {plot} and place {k}: no way"));
                    let length: f32 = route.windows(2).map(|p| dist(p[0], p[1])).sum();
                    assert!(length <= 2.5 * dist(from, to) + 10.0, "plot {plot} and place {k}: a long way round, {length} yd: {route:?}");
                    for pair in route.windows(2) {
                        for s in 0..=1000 {
                            let t = s as f32 / 1000.0;
                            let p = (pair[0].0 + (pair[1].0 - pair[0].0) * t, pair[0].1 + (pair[1].1 - pair[0].1) * t);
                            // a cell by an end counts as free, though part of it is a little farther
                            let by_an_end = walls.cell(p).is_some_and(|c| dist(walls.middle(c), from) <= NEAR_ENDS || dist(walls.middle(c), to) <= NEAR_ENDS);
                            assert!(by_an_end || !walls.in_the_way(p), "plot {plot} and place {k}: through something at {p:?} on {route:?}, from {from:?} to {to:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_block_holds_what_is_on_it_turned_its_way() {
        // facing west (the world's y): two yards ahead is two yards west
        let b = Block { at: (10.0, 10.0), facing: std::f32::consts::FRAC_PI_2, x: (0.0, 3.0), y: (-0.5, 0.5) };
        assert!(b.holds((10.0, 12.0), 0.0));
        assert!(!b.holds((12.0, 10.0), 0.0), "two yards north is to its right, off it");
        assert!(b.holds((10.8, 12.0), 0.5), "within the margin");
    }

    #[test]
    fn a_route_goes_round_what_is_in_the_way_and_never_through_it() {
        let wall = Block { at: (0.0, 0.0), facing: 0.0, x: (-0.5, 0.5), y: (-6.0, 6.0) };
        let walls = Walls::new(&[wall], (0.0, 0.0), 20.0);
        let route = walls.route((-8.0, 0.0), (8.0, 0.0)).expect("a way round");
        assert_eq!(route.first(), Some(&(-8.0, 0.0)));
        assert_eq!(route.last(), Some(&(8.0, 0.0)));
        assert!(route.len() > 2, "it turns at the wall's end: {route:?}");
        for pair in route.windows(2) {
            for s in 0..=40 {
                let t = s as f32 / 40.0;
                let p = (pair[0].0 + (pair[1].0 - pair[0].0) * t, pair[0].1 + (pair[1].1 - pair[0].1) * t);
                assert!(!wall.holds(p, 0.0), "{p:?} is on the wall");
            }
        }
        let open = walls.route((-8.0, 10.0), (8.0, 10.0)).expect("a way");
        assert_eq!(open.len(), 2, "nothing between: straight there");
    }
}

