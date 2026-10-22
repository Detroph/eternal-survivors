//! How enemies move and attack.
//!
//! Enemies move in straight timed segments (from, to, start time, duration), like a WoW monster
//! move, and re-decide on a timer. A chasing enemy heads for a point on a small circle around the
//! hero whose angle creeps forward each decision, so a crowd spreads around the hero instead of
//! stacking. In reach it stops and attacks on its swing timer: melee hits land at once; ranged
//! enemies stand and cast, then a homing projectile flies at the spell's WoW speed and always hits
//! on arrival.

/// Longest single move toward the hero, yards.
pub const MAX_STEP: f32 = 14.0;
/// Each chase decision moves the aim point this far around the hero, radians.
pub const ORBIT_STEP: f32 = 0.15;
/// Smallest aim circle around the hero, yards.
pub const MIN_ORBIT: f32 = 1.5;
/// Melee enemies aim at this share of their collision radius from the hero.
pub const MELEE_ORBIT_SHARE: f32 = 0.8;
/// Ranged enemies start casting once within `range + RANGED_SLACK`.
pub const RANGED_SLACK: f32 = 4.0;
/// A chasing enemy re-decides after its move time, kept within these bounds, seconds.
pub const CHASE_DECISION: (f32, f32) = (0.25, 0.7);
/// Chase speed never drops below this, yards per second.
pub const MIN_CHASE_SPEED: f32 = 0.5;
/// A feared enemy runs this far straight away from the hero per decision, at
/// `FEAR_SPEED_SHARE` of its speed, re-deciding every `FEAR_DECISION` seconds.
pub const FEAR_STEP: f32 = 8.0;
pub const FEAR_SPEED_SHARE: f32 = 0.8;
pub const FEAR_DECISION: f32 = 0.8;
/// An enraged boss (at or below `ENRAGE_HEALTH` of its health, for the rest of its life) moves
/// `ENRAGE_SPEED` times faster, hits `ENRAGE_DAMAGE` times harder and recasts its abilities
/// `ENRAGE_COOLDOWN` times as often.
pub const ENRAGE_HEALTH: f32 = 0.3;
pub const ENRAGE_SPEED: f32 = 1.25;
pub const ENRAGE_DAMAGE: f32 = 1.4;
pub const ENRAGE_COOLDOWN: f32 = 0.7;

pub type Vec2 = (f32, f32);

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// One straight move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub from: Vec2,
    pub to: Vec2,
    pub start: f32,
    pub duration: f32,
}

impl Segment {
    pub fn standing(at: Vec2, now: f32) -> Self {
        Self { from: at, to: at, start: now, duration: 0.0 }
    }

    /// Where the enemy is at `now`.
    pub fn position(&self, now: f32) -> Vec2 {
        if self.duration <= 0.0 {
            return self.to;
        }
        let t = ((now - self.start) / self.duration).clamp(0.0, 1.0);
        (self.from.0 + (self.to.0 - self.from.0) * t, self.from.1 + (self.to.1 - self.from.1) * t)
    }

    pub fn moving(&self, now: f32) -> bool {
        self.duration > 0.0 && now < self.start + self.duration
    }
}

/// Current move speed: base, cut by an active slow, faster for an enraged boss.
pub fn move_speed(base: f32, slow: Option<f32>, enraged_boss: bool) -> f32 {
    let s = base * (1.0 - slow.unwrap_or(0.0));
    if enraged_boss {
        s * ENRAGE_SPEED
    } else {
        s
    }
}

/// Damage of one attack: base, cut by an active weaken, harder for an enraged boss.
pub fn attack_damage(base: f32, weaken: Option<f32>, enraged_boss: bool) -> f32 {
    let d = base * (1.0 - weaken.unwrap_or(0.0));
    if enraged_boss {
        d * ENRAGE_DAMAGE
    } else {
        d
    }
}

/// How close an enemy wants to be: it chases while farther than `.0` and aims at a circle of
/// radius `.1` around the hero.
pub fn reach(collision_radius: f32, ranged_range: Option<f32>) -> (f32, f32) {
    match ranged_range {
        Some(range) => (range + RANGED_SLACK, range.max(MIN_ORBIT)),
        None => (collision_radius, (collision_radius * MELEE_ORBIT_SHARE).max(MIN_ORBIT)),
    }
}

/// A chase decision: the next move and when to decide again. `angle` is the enemy's own aim
/// angle, advanced here.
pub fn chase(pos: Vec2, hero: Vec2, angle: &mut f32, orbit: f32, speed: f32, now: f32) -> (Segment, f32) {
    *angle += ORBIT_STEP;
    let mut aim = (hero.0 + angle.cos() * orbit, hero.1 + angle.sin() * orbit);
    let mut d = dist(pos, aim);
    if d > MAX_STEP {
        let k = MAX_STEP / d;
        aim = (pos.0 + (aim.0 - pos.0) * k, pos.1 + (aim.1 - pos.1) * k);
        d = MAX_STEP;
    }
    let duration = d / speed.max(MIN_CHASE_SPEED);
    let next = now + duration.clamp(CHASE_DECISION.0, CHASE_DECISION.1);
    (Segment { from: pos, to: aim, start: now, duration }, next)
}

/// A fear decision: run straight away from the hero.
pub fn flee(pos: Vec2, hero: Vec2, speed: f32, now: f32) -> (Segment, f32) {
    let away = (pos.0 - hero.0, pos.1 - hero.1);
    let len = dist(pos, hero).max(0.1);
    let to = (pos.0 + away.0 / len * FEAR_STEP, pos.1 + away.1 / len * FEAR_STEP);
    let duration = FEAR_STEP / (speed * FEAR_SPEED_SHARE);
    (Segment { from: pos, to, start: now, duration }, now + FEAR_DECISION)
}

/// Seconds a projectile flies: distance over the spell's `Spell.dbc` Speed; 0 lands at once.
pub fn flight_time(distance: f32, speed: f32) -> f32 {
    if speed > 0.0 {
        distance / speed
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chase_steps_are_capped_and_circle() {
        let mut a = 0.0;
        let (seg, next) = chase((100.0, 0.0), (0.0, 0.0), &mut a, 2.0, 7.0, 10.0);
        assert!((a - 0.15).abs() < 1e-6);
        assert!((dist(seg.from, seg.to) - 14.0).abs() < 1e-4);
        assert!((seg.duration - 2.0).abs() < 1e-4);
        assert!((next - 10.7).abs() < 1e-5);
        // close by: aims at the circle, decides again soon
        let (seg, next) = chase((3.0, 0.0), (0.0, 0.0), &mut a, 2.0, 7.0, 0.0);
        assert!((dist(seg.to, (0.0, 0.0)) - 2.0).abs() < 1e-4);
        assert!((0.25..=0.7).contains(&next));
    }

    #[test]
    fn reach_melee_and_ranged() {
        // scale 1: collision 3.4, aim 2.72
        let (chase_beyond, orbit) = reach(3.4, None);
        assert_eq!(chase_beyond, 3.4);
        assert!((orbit - 2.72).abs() < 1e-5);
        assert_eq!(reach(3.4, Some(18.0)), (22.0, 18.0));
        assert_eq!(reach(1.0, None).1, 1.5);
    }

    #[test]
    fn fear_runs_away() {
        let (seg, next) = flee((5.0, 0.0), (0.0, 0.0), 5.0, 1.0);
        assert_eq!(seg.to, (13.0, 0.0));
        assert!((seg.duration - 2.0).abs() < 1e-6);
        assert!((next - 1.8).abs() < 1e-6);
    }

    #[test]
    fn enrage_and_slow() {
        assert!((move_speed(5.0, Some(0.5), true) - 3.125).abs() < 1e-6);
        assert!((attack_damage(10.0, None, true) - 14.0).abs() < 1e-5);
        assert_eq!(flight_time(24.0, 24.0), 1.0);
        assert_eq!(flight_time(24.0, 0.0), 0.0);
    }
}
