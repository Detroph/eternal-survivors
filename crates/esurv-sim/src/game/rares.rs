//! Rares: five or six strong copies of the zone's elite enemies a run sends in on the elite
//! timer, spread over the run. They come and fight silently, as elites do. A slain rare gives
//! gold, an instant level and a cache free to open. A rare's health and damage are the
//! rare kind of the one enemy stat formula (`enemy::health_and_damage`); slain rares only
//! raise the gold.

use esurv_content::{EnemyType, Zone};

use super::boss_abilities::rare_abilities;
use super::director::FIRST_ELITE;
use super::enemy::{health_and_damage, spawn_stats, RUN_FACTOR};
use super::enemy_ai::{Segment, Vec2};
use super::enemy_attacks::AbilitySlot;
use super::level::xp_to_next;
use super::loot::{Pickup, RARE_MAGNET_CHANCE};
use super::pickups::ELITE_CACHE_OFFSET;
use super::run::{EnemyKind, EnemyUnit, Event, Run};
use crate::rng::Rng;

/// How many rares a run sends: one of these, drawn per run.
pub const RARES_PER_RUN: std::ops::RangeInclusive<u32> = 5..=6;
/// Mixed into the run's seed for the rare count's own draw.
const RARE_SEED: u64 = 0x6A0D_1A75;
/// The run length rares spread over when a zone has no final boss, minutes.
pub const FALLBACK_RUN_MINUTES: f32 = 16.0;
/// Rares are drawn this much larger than the enemy they copy.
pub const SCALE: f32 = 1.35;
/// Gold a slain rare drops: `GOLD_BASE + GOLD_PER_SLAIN × rares slain (this one included)`.
pub const GOLD_BASE: u32 = 25;
pub const GOLD_PER_SLAIN: u32 = 5;

/// How many rares this run sends (5 or 6, evenly).
pub fn rare_count(rng: &mut Rng) -> u32 {
    let (lo, hi) = (*RARES_PER_RUN.start(), *RARES_PER_RUN.end());
    lo + (rng.next_u64() % u64::from(hi - lo + 1)) as u32
}

/// When the rares are due (run seconds): `count` points spread evenly between the first elite
/// (4:00) and the final boss, each in the middle of its share. The first elite the elite timer
/// sends at or after a due point comes as a rare instead.
pub fn rare_schedule(count: u32, final_minute: Option<f32>) -> Vec<f32> {
    let end = final_minute.unwrap_or(FALLBACK_RUN_MINUTES) * 60.0;
    let step = (end - FIRST_ELITE).max(0.0) / count.max(1) as f32;
    (0..count).map(|i| FIRST_ELITE + (i as f32 + 0.5) * step).collect()
}

/// Gold dropped by the `slain`-th rare of the run (1-based).
pub fn gold(slain: u32) -> u32 {
    GOLD_BASE + GOLD_PER_SLAIN * slain
}

/// Whether roster entry `roster` of `zone` is an elite (only elites have rare copies).
pub fn is_elite(zone: &Zone, roster: usize) -> bool {
    zone.enemies.get(roster).is_some_and(|e| e.elite)
}

impl Run {
    /// The run's rare schedule, drawn when the run opens. The count is spread from the run's
    /// seed (as the bosses' characters are), so it leaves the run's own draws as they were.
    pub(crate) fn plan_rares(&mut self) {
        let count = rare_count(&mut Rng::new(self.seed ^ RARE_SEED));
        let final_minute = self.zone().bosses.iter().find(|b| b.is_final).map(|b| b.spawn_minute);
        self.rares_due = rare_schedule(count, final_minute);
    }

    /// The elite timer sent roster elite `roster`: true when a rare was due and it came as one.
    pub(crate) fn elite_as_rare(&mut self, roster: usize) -> bool {
        if !self.rares_due.first().is_some_and(|&t| t <= self.now) || !is_elite(self.zone(), roster) {
            return false;
        }
        if !self.spawn_rare(roster, None) {
            return false;
        }
        self.rares_due.remove(0);
        true
    }

    /// A rare copy of roster elite `roster`, at `at` or on the elite spawn ring around a hero
    /// (a rare is never held back by the live cap). False when no hero lives.
    pub fn spawn_rare(&mut self, roster: usize, at: Option<Vec2>) -> bool {
        let pos = match at {
            Some(p) => p,
            None => match self.anchor_hero() {
                Some(h) => self.spawn_point(h, self.zone().enemy_spawn_ring),
                None => return false,
            },
        };
        let content = self.content.clone();
        let zone = &content.zones[self.zone];
        let minute = self.now / 60.0;
        let endless = self.endless_minutes();
        let mut copy = zone.enemies[roster].clone();
        copy.scale *= SCALE;
        let stats = spawn_stats(&copy, zone, &content.balance, minute, endless, &self.options, RUN_FACTOR);
        let (hp, dmg) = health_and_damage(&content.balance, EnemyType::Rare, zone, minute, endless, &self.options, RUN_FACTOR);
        let id = self.new_id();
        let aim_angle = self.rng.range(0.0, std::f32::consts::TAU);
        self.enemies.push(EnemyUnit {
            id,
            kind: EnemyKind::Rare { roster },
            segment: Segment::standing(pos, self.now),
            next_decision: self.now,
            aim_angle,
            health: hp,
            max_health: hp,
            damage: dmg,
            // a rare runs at its enemy's own speed, times the game's enemy speed
            base_speed: self.options.speed_pct as f32 / 100.0 * self.content.balance.enemy_speed * copy.move_speed,
            attack_interval: copy.attack_interval,
            swing_timer: stats.first_swing,
            collision_radius: stats.collision_radius,
            ranged_range: None,
            ranged_spell: None,
            cast_time: 0.0,
            shot_share: 1.0,
            casting_until: None,
            charging_until: 0.0,
            abilities: rare_abilities().iter().map(AbilitySlot::rare).collect(),
            next_ability_at: 0.0,
            slow: None,
            weaken: None,
            fear_until: 0.0,
            silenced_until: 0.0,
            hold_until: 0.0,
            hold_chain_start: 0.0,
            hold_immune_until: 0.0,
            vulnerable: None,
            dots: Vec::new(),
            kit: Default::default(),
            elite: true,
            enraged: false,
            spawn_cost: copy.spawn_cost,
            mech: Default::default(),
        });
        self.push(Event::RareSpawned { id, roster, at: pos });
        true
    }

    /// A rare died (by hero `hero`): gold drops, a due magnet may drop, a rare cache
    /// (free to open) drops beside it, and the killer gets an instant level.
    pub(crate) fn rare_slain(&mut self, at: Vec2, hero: Option<usize>) {
        self.rares_slain += 1;
        self.drop(Pickup::Gold, gold(self.rares_slain), at);
        if self.rng.unit() < RARE_MAGNET_CHANCE {
            self.drop_due_magnet(at);
        }
        self.drop(Pickup::RareCache, 0, (at.0 + ELITE_CACHE_OFFSET.0, at.1 + ELITE_CACHE_OFFSET.1));
        if let Some(h) = hero {
            let p = self.heroes[h].progress;
            let to_level = xp_to_next(p.level, &self.content.balance.experience).saturating_sub(p.xp).max(1);
            self.grant_experience(h, to_level);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::Content;

    use super::*;
    use crate::game::casting::HitSource;
    use crate::game::choices::ScreenKind;
    use crate::game::enemy::RunOptions;
    use crate::game::run::{Flat, HeroSetup, Phase};

    fn content() -> Arc<Content> {
        Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).expect("content loads"))
    }

    fn run(seed: u64) -> Run {
        let hero = HeroSetup { class: "warrior".into(), race: "orc".into(), ..Default::default() };
        let mut r = Run::new(content(), "elwynn", RunOptions::default(), &[hero], Box::new(Flat), seed).unwrap();
        let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
        r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        r.drain_events();
        r
    }

    fn rares(r: &Run) -> Vec<&EnemyUnit> {
        r.enemies.iter().filter(|e| e.is_rare()).collect()
    }

    fn dist(a: Vec2, b: Vec2) -> f32 {
        ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
    }

    #[test]
    fn five_or_six_rares_a_run_spread_between_the_first_elite_and_the_final_boss() {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..40 {
            let r = run(seed);
            let n = r.rares_due.len() as u32;
            assert!(RARES_PER_RUN.contains(&n), "seed {seed}: {n}");
            seen.insert(n);
            let last = r.rares_due.last().copied().unwrap();
            assert!(r.rares_due[0] > FIRST_ELITE && last < 16.0 * 60.0, "{:?}", r.rares_due);
            assert!(r.rares_due.windows(2).all(|w| w[1] > w[0]));
        }
        assert_eq!(seen.len(), 2, "both counts come up");
        assert_eq!(rare_schedule(6, Some(16.0)), vec![300.0, 420.0, 540.0, 660.0, 780.0, 900.0]);
    }

    #[test]
    fn rares_come_silently_on_the_elite_timer_on_the_elite_ring() {
        // a whole Elwynn run up to the final boss: every rare due arrives on an elite's turn, a
        // copy of one of the zone's elites, around the hero on the elite ring, saying nothing
        let mut r = run(5);
        let planned = r.rares_due.len();
        let ring = r.content.zones[r.zone].enemy_spawn_ring;
        let mut arrivals = 0;
        let mut ids = std::collections::HashSet::new();
        while r.now < 16.0 * 60.0 - 1.0 && r.phase != Phase::Defeat {
            r.heroes[0].health = r.heroes[0].stats.max_health;
            r.tick(0.1);
            r.take_first_offers();
            let hero = r.heroes[0].pos;
            for e in r.drain_events() {
                match e {
                    Event::RareSpawned { id, roster, at } => {
                        arrivals += 1;
                        ids.insert(id);
                        assert!(r.now >= FIRST_ELITE);
                        assert!(r.content.zones[r.zone].enemies[roster].elite);
                        let d = dist(at, hero);
                        assert!(d >= ring.0 - 0.5 && d <= ring.1 + 0.5, "{d} in {ring:?}");
                        // the elite timer's turn went to the rare: no elite came with it
                        assert_eq!(r.enemies.iter().filter(|e| e.elite && !e.is_rare()).count(), 0);
                        assert!(r.enemies.iter().any(|g| g.id == id && g.is_rare()));
                    }
                    Event::Emote { from: crate::game::run::Speaker::Unit(crate::game::run::Unit::Enemy(id)), .. }
                    | Event::Yell { id, .. }
                        if ids.contains(&id) =>
                    {
                        panic!("a rare spoke")
                    }
                    _ => {}
                }
            }
            // an empty field: nothing hurts the hero and the live cap never stands in the way
            r.enemies.clear();
            r.enemy_shots.clear();
            r.ground_hits.clear();
        }
        assert_eq!(arrivals, planned, "{:?} at {}", r.phase, r.now);
        assert!(r.rares_due.is_empty());
    }

    #[test]
    fn a_rare_follows_the_rare_formula() {
        let mut r = run(13);
        r.enemies.clear();
        r.now = 20.0;
        let roster = r.content.zones[r.zone].enemies.iter().position(|e| e.elite).unwrap();
        assert!(r.spawn_rare(roster, None));
        let g = rares(&r)[0].clone();
        assert_eq!(g.base_speed, r.content.zones[r.zone].enemies[roster].move_speed * r.content.balance.enemy_speed);
        // Elwynn (D 1) at minute 1/3: 240 × 16 × health clock, 8 × 3 × damage clock
        let m = 20.0 / 60.0;
        let (h, dmg) = (240.0 * 16.0 * (1.0 + m * 1.5 / 4.0), 24.0 * (1.0 + m * 0.5 / 4.0));
        assert!((g.max_health - h).abs() / h < 1e-5 && (g.damage - dmg).abs() / dmg < 1e-5);
    }

    #[test]
    fn killing_a_rare_gives_a_level_and_a_free_cache() {
        let mut r = run(13);
        r.enemies.clear();
        r.now = 20.0;
        let roster = r.content.zones[r.zone].enemies.iter().position(|e| e.elite).unwrap();
        let near = (r.heroes[0].pos.0 + 10.0, r.heroes[0].pos.1);
        r.spawn_rare(roster, Some(near));
        let id = rares(&r)[0].id;
        let level = r.heroes[0].progress.level;
        r.drain_events();
        r.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Other);
        assert_eq!(r.heroes[0].progress.level, level + 1, "an instant level");
        assert_eq!(r.rares_slain, 1);
        assert!(r.pickups.iter().any(|p| p.kind == Pickup::Gold && p.value == 30));
        assert!(!r.drain_events().iter().any(|e| matches!(e, Event::Emote { .. } | Event::Yell { .. })), "it falls silently");
        let cache = r.pickups.iter().find(|p| p.kind == Pickup::RareCache).unwrap().clone();
        // the level-up card comes first
        r.tick(0.05);
        let hero = r.heroes[0].id;
        assert_eq!(r.screen.as_ref().unwrap().kind, ScreenKind::LevelUp);
        assert!(r.choose(hero, 0));
        // the cache opens with no gold at all, and the cache price stays as it was
        let price = r.field.cache_price;
        r.heroes[0].gold = 0;
        r.heroes[0].pos = cache.at;
        r.drain_events();
        r.tick(0.05);
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::CacheOpened { price: 0, .. })));
        assert_eq!(r.field.cache_price, price);
        assert_eq!(r.screen.as_ref().unwrap().kind, ScreenKind::RareCache);
        assert_eq!(r.screen.as_ref().unwrap().offers.len(), 3);
        assert!(r.choose(hero, 0));
        assert_eq!(r.heroes[0].pending_rare_caches, 0);
    }

    #[test]
    fn rares_do_not_grow_per_kill() {
        let mut r = run(13);
        r.enemies.clear();
        r.now = 20.0;
        let roster = r.content.zones[r.zone].enemies.iter().position(|e| e.elite).unwrap();
        r.spawn_rare(roster, None);
        let first = rares(&r)[0].clone();
        for _ in 0..3 {
            let id = rares(&r)[0].id;
            r.hit_enemy(None, id, f32::MAX, false, HitSource::Other);
            r.spawn_rare(roster, None);
        }
        assert_eq!(r.rares_slain, 3);
        let again = rares(&r)[0];
        assert_eq!((again.max_health, again.damage), (first.max_health, first.damage), "the same minute, the same rare");
    }

    #[test]
    fn gold_grows_with_rares_slain() {
        assert_eq!([gold(1), gold(2)], [30, 35]);
    }
}
