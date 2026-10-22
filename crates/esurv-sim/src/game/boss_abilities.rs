//! What boss and rare abilities do when their cooldown runs out.
//!
//! Every ability scales off the caster's attack damage (its damage, cut by weaken, ×1.4 while an
//! enraged boss; see `enemy_ai::attack_damage`). Hits that land later wait as ground hits: they
//! strike if the hero is inside their circle when they land.

use esurv_content::{BossAbility, BossAbilityKind};

use super::enemy_ai::{Segment, Vec2, ENRAGE_COOLDOWN};
use crate::rng::Rng;

/// A boss's abilities start at this share of their cooldown when it spawns.
pub const BOSS_FIRST_CAST: f32 = 0.6;
/// A rare's start at half.
pub const RARE_FIRST_CAST: f32 = 0.5;

/// `volley`: the slowest a missile flies and its hit circle (its share of attack damage is the
/// ability's, from `enemy_kinds.ron`); the missiles after the first land this far from the hero.
pub const VOLLEY_MIN_SPEED: f32 = 10.0;
pub const VOLLEY_RADIUS: f32 = 2.5;
pub const VOLLEY_SPREAD: (f32, f32) = (2.0, 6.0);
/// Summon: the adds' ring around the caster, and the roster entries used (every third add is the
/// second entry).
pub const SUMMON_RING: f32 = 5.0;
/// Charge: rush speed, shortest rush, landing circle, landing visual (WoW spell 6343, Thunder
/// Clap), and the pauses after arriving before it re-decides and swings (its landing damage share
/// is the ability's, from `enemy_kinds.ron`).
pub const CHARGE_SPEED: f32 = 24.0;
pub const CHARGE_MIN_TIME: f32 = 1.1;
pub const CHARGE_RADIUS: f32 = 4.5;
pub const CHARGE_IMPACT_VISUAL: u32 = 6343;
pub const CHARGE_DECISION_PAUSE: f32 = 0.2;
pub const CHARGE_SWING_PAUSE: f32 = 0.3;

/// A hit that lands later at a spot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundHit {
    pub at: Vec2,
    pub radius: f32,
    pub damage: f32,
    pub lands_at: f32,
    pub impact_spell: u32,
    /// The impact's look plays where it lands (a burst, a charge's landing); a volley's shot
    /// shows when it leaves instead.
    pub look_on_landing: bool,
    /// The marked spot shown until it lands (a lasting ground look's id).
    pub marker: Option<u32>,
}

impl GroundHit {
    /// Whether the hero, standing at `hero` when it lands, is hit.
    pub fn hits(&self, hero: Vec2) -> bool {
        ((hero.0 - self.at.0).powi(2) + (hero.1 - self.at.1).powi(2)).sqrt() <= self.radius
    }
}

/// What one ability cast does.
#[derive(Debug, Clone, PartialEq)]
pub enum AbilityEffect {
    /// Hits landing later, with an optional marker shown until they land.
    Ground { hits: Vec<GroundHit>, marker: Option<(u32, Vec2, f32)> },
    /// An immediate hit on the hero (`None` when out of reach), with the visual played around the
    /// caster, `radius` yards.
    Instant { visual: u32, damage: Option<f32>, radius: f32 },
    /// Adds to spawn: roster index and offset from the caster.
    Summon(Vec<(usize, Vec2)>),
    /// The caster rushes to `segment.to`; the landing hit; when it re-decides and next swings.
    Charge { visual: u32, segment: Segment, landing: GroundHit, next_decision: f32, swing_in: f32 },
}

/// The rares' fixed abilities: Charge (landing ×2) every 9 s, a `thunder_clap` stomp (8 yd,
/// ×1.4) every 7 s, and four adds every 16 s.
pub fn rare_abilities() -> [BossAbility; 3] {
    [
        BossAbility { kind: BossAbilityKind::Charge { look_spell: 100, damage_mult: 2.0 }, cooldown: 9.0 },
        BossAbility { kind: BossAbilityKind::Stomp { look_spell: 6343, radius: 8.0, damage_mult: 1.4 }, cooldown: 7.0 },
        BossAbility { kind: BossAbilityKind::Summon { count: 4 }, cooldown: 16.0 },
    ]
}

/// The cooldown restarted after a cast.
pub fn next_cooldown(ability: &BossAbility, enraged: bool) -> f32 {
    if enraged {
        ability.cooldown * ENRAGE_COOLDOWN
    } else {
        ability.cooldown
    }
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Casts `kind`. `missile_speed` gives a WoW spell's `Spell.dbc` Speed (0 when it has none).
pub fn cast(
    kind: &BossAbilityKind,
    caster: Vec2,
    hero: Vec2,
    attack_damage: f32,
    now: f32,
    rng: &mut Rng,
    missile_speed: impl Fn(u32) -> f32,
) -> AbilityEffect {
    match *kind {
        BossAbilityKind::GroundBurst { telegraph_spell, impact_spell, radius, delay, damage_mult } => AbilityEffect::Ground {
            hits: vec![GroundHit { at: hero, radius, damage: damage_mult * attack_damage, lands_at: now + delay, impact_spell, look_on_landing: true, marker: None }],
            marker: Some((telegraph_spell, hero, radius)),
        },
        BossAbilityKind::Volley { look_spell, count, damage_mult, .. } => {
            let speed = missile_speed(look_spell).max(VOLLEY_MIN_SPEED);
            let hits = (0..count)
                .map(|i| {
                    let offset = rng.unit();
                    let at = if i == 0 {
                        hero
                    } else {
                        let angle = i as f32 / count as f32 * std::f32::consts::TAU + offset;
                        let r = rng.range(VOLLEY_SPREAD.0, VOLLEY_SPREAD.1);
                        (hero.0 + angle.cos() * r, hero.1 + angle.sin() * r)
                    };
                    GroundHit {
                        at,
                        radius: VOLLEY_RADIUS,
                        damage: damage_mult * attack_damage,
                        lands_at: now + dist(caster, at) / speed,
                        impact_spell: look_spell,
                        look_on_landing: false,
                        marker: None,
                    }
                })
                .collect();
            AbilityEffect::Ground { hits, marker: None }
        }
        BossAbilityKind::Summon { count } => AbilityEffect::Summon(
            (0..count)
                .map(|i| {
                    let a = i as f32 / count as f32 * std::f32::consts::TAU;
                    (usize::from(i % 3 == 2), (a.cos() * SUMMON_RING, a.sin() * SUMMON_RING))
                })
                .collect(),
        ),
        BossAbilityKind::Stomp { look_spell, radius, damage_mult } => AbilityEffect::Instant {
            visual: look_spell,
            radius,
            damage: (dist(caster, hero) <= radius).then_some(damage_mult * attack_damage),
        },
        BossAbilityKind::Charge { look_spell, damage_mult } => {
            let duration = (dist(caster, hero) / CHARGE_SPEED).max(CHARGE_MIN_TIME);
            let end = now + duration;
            AbilityEffect::Charge {
                visual: look_spell,
                segment: Segment { from: caster, to: hero, start: now, duration },
                landing: GroundHit {
                    at: hero,
                    radius: CHARGE_RADIUS,
                    damage: damage_mult * attack_damage,
                    lands_at: end,
                    impact_spell: CHARGE_IMPACT_VISUAL,
                    look_on_landing: true,
                    marker: None,
                },
                next_decision: end + CHARGE_DECISION_PAUSE,
                swing_in: duration + CHARGE_SWING_PAUSE,
            }
        }
        // the building blocks are cast by `Run::cast_building_block` (game::telegraphs)
        BossAbilityKind::Marked { .. } | BossAbilityKind::Rush { .. } | BossAbilityKind::Lay { .. } | BossAbilityKind::Zone { .. } => AbilityEffect::Summon(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_burst_waits_then_checks_the_circle() {
        let k = BossAbilityKind::GroundBurst { telegraph_spell: 10, impact_spell: 122, radius: 10.0, delay: 1.4, damage_mult: 3.0 };
        let e = cast(&k, (0.0, 0.0), (20.0, 0.0), 10.0, 5.0, &mut Rng::new(1), |_| 0.0);
        let AbilityEffect::Ground { hits, marker } = e else { panic!() };
        assert_eq!(marker, Some((10, (20.0, 0.0), 10.0)));
        assert!((hits[0].lands_at - 6.4).abs() < 1e-5);
        assert_eq!(hits[0].damage, 30.0);
        assert!(hits[0].hits((25.0, 0.0)) && !hits[0].hits((31.0, 0.0)));
    }

    #[test]
    fn volley_first_on_the_hero_rest_around() {
        let k = BossAbilityKind::Volley { look_spell: 133, count: 5, damage_mult: 0.5, impact_on_bunny: false };
        let e = cast(&k, (0.0, 0.0), (24.0, 0.0), 10.0, 0.0, &mut Rng::new(3), |_| 24.0);
        let AbilityEffect::Ground { hits, .. } = e else { panic!() };
        assert_eq!(hits.len(), 5);
        assert_eq!(hits[0].at, (24.0, 0.0));
        assert!((hits[0].lands_at - 1.0).abs() < 1e-5);
        assert!((hits[0].damage - 5.0).abs() < 1e-5);
        for h in &hits[1..] {
            let r = dist(h.at, (24.0, 0.0));
            assert!((2.0..6.0).contains(&r), "{r}");
        }
        // a slow spell is lifted to 10 yd/s
        let e = cast(&k, (0.0, 0.0), (20.0, 0.0), 10.0, 0.0, &mut Rng::new(3), |_| 0.0);
        let AbilityEffect::Ground { hits, .. } = e else { panic!() };
        assert!((hits[0].lands_at - 2.0).abs() < 1e-5);
    }

    #[test]
    fn summon_stomp_charge() {
        let AbilityEffect::Summon(adds) = cast(&BossAbilityKind::Summon { count: 6 }, (0.0, 0.0), (9.0, 9.0), 1.0, 0.0, &mut Rng::new(1), |_| 0.0) else { panic!() };
        let kinds: Vec<usize> = adds.iter().map(|a| a.0).collect();
        assert_eq!(kinds, [0, 0, 1, 0, 0, 1]);
        assert!((dist(adds[0].1, (0.0, 0.0)) - 5.0).abs() < 1e-5);
        let stomp = BossAbilityKind::Stomp { look_spell: 6343, radius: 8.0, damage_mult: 1.4 };
        assert_eq!(cast(&stomp, (0.0, 0.0), (5.0, 0.0), 10.0, 0.0, &mut Rng::new(1), |_| 0.0), AbilityEffect::Instant { visual: 6343, damage: Some(14.0), radius: 8.0 });
        assert_eq!(cast(&stomp, (0.0, 0.0), (9.0, 0.0), 10.0, 0.0, &mut Rng::new(1), |_| 0.0), AbilityEffect::Instant { visual: 6343, damage: None, radius: 8.0 });
        let charge = BossAbilityKind::Charge { look_spell: 100, damage_mult: 2.0 };
        let AbilityEffect::Charge { segment, landing, next_decision, swing_in, .. } =
            cast(&charge, (0.0, 0.0), (12.0, 0.0), 10.0, 0.0, &mut Rng::new(1), |_| 0.0)
        else {
            panic!()
        };
        // 12 yd at 24 yd/s is 0.5 s, raised to 1.1 s
        assert!((segment.duration - 1.1).abs() < 1e-5);
        assert_eq!((landing.radius, landing.damage), (4.5, 20.0));
        assert!((next_decision - 1.3).abs() < 1e-5 && (swing_in - 1.4).abs() < 1e-5);
    }

    #[test]
    fn enrage_shortens_cooldowns() {
        let g = rare_abilities();
        assert!((next_cooldown(&g[0], true) - 6.3).abs() < 1e-5);
        assert_eq!(next_cooldown(&g[2], false), 16.0);
    }
}
