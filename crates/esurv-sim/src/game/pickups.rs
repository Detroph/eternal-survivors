//! What lies on the field and how heroes take it: experience orbs, gold, caches, runes and
//! magnets; when caches, runes and magnets appear; and how a run opens.

use super::enemy_ai::Vec2;
use super::level::LEVEL_UP_HEAL;
use super::loot::{
    magnet_schedule, next_cache_price, random_rune, Pickup, BERSERKER_RUNE, CACHES_AT_START, CACHE_EVERY, CACHE_LIMIT,
    CACHE_MAX_PRESENT, CACHE_MESSAGE_GAP, CACHE_RING, MAX_LOOSE_ORBS, RESTORATION_HEAL, RUNE_EVERY, RUNE_MAX_PRESENT,
    RUNE_RING, SPEED_RUNE,
};
use super::run::{Buff, BuffKind, Event, Run};

/// Experience orbs within pickup range fly to the hero at `ORB_SPEED`; orbs older than
/// `ORB_SELF_COLLECT_AGE` fly in from anywhere at `ORB_OLD_SPEED`. A flight takes at least
/// `ORB_MIN_FLIGHT`.
pub const ORB_SPEED: f32 = 24.0;
pub const ORB_OLD_SPEED: f32 = 32.0;
pub const ORB_SELF_COLLECT_AGE: f32 = 6.0;
pub const ORB_MIN_FLIGHT: f32 = 0.12;
/// A magnet pulls every orb and coin to the hero this fast, taking at least `MAGNET_MIN_FLIGHT`.
pub const MAGNET_SPEED: f32 = 40.0;
pub const MAGNET_MIN_FLIGHT: f32 = 0.15;
/// Caches, runes and magnets are taken within this many yards.
pub const TAKE_RANGE: f32 = 3.0;
/// An elite's cache drops at this offset from where it died.
pub const ELITE_CACHE_OFFSET: Vec2 = (2.0, -1.0);
/// Magnets appear this far from a hero.
pub const MAGNET_RING: (f32, f32) = (25.0, 50.0);
/// A run opens with this many of the roster's first enemy.
pub const OPENING_ENEMIES: usize = 4;
/// The first cache and rune checks.
pub const FIRST_CACHE_CHECK: f32 = super::loot::FIRST_CACHE_CHECK;
pub const FIRST_RUNE_CHECK: f32 = super::loot::FIRST_RUNE_CHECK;
/// The spell visuals of the runes (Spell.dbc ids).
pub const SPEED_RUNE_VISUAL: u32 = 23451;
pub const BERSERKER_RUNE_VISUAL: u32 = 23505;
pub const RESTORATION_RUNE_VISUAL: u32 = 23493;
/// Opening a cache plays this sound; taking a magnet this one (`SoundEntries.dbc` ids).
pub const CACHE_SOUND: u32 = 1204;
pub const MAGNET_SOUND: u32 = 120;

/// Something lying on the field.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldPickup {
    pub id: u32,
    pub kind: Pickup,
    pub value: u32,
    pub at: Vec2,
    pub spawned_at: f32,
    /// On its way to a hero: (hero, arrival).
    pub flying: Option<(usize, f32)>,
}

/// A magnet of the run's schedule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MagnetSlot {
    /// From then on, a boss kill drops it.
    pub due: f32,
    /// Then it appears on its own.
    pub appears: f32,
    pub placed: bool,
}

/// The run-wide state of the field's timers.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldTimers {
    pub cache_price: u32,
    pub next_cache_check: f32,
    pub next_rune_check: f32,
    /// When "This cache needs N gold." was last said.
    pub cache_message_at: f32,
    pub magnets: Vec<MagnetSlot>,
}

impl Default for FieldTimers {
    fn default() -> Self {
        Self {
            cache_price: super::loot::CACHE_PRICE_START,
            next_cache_check: FIRST_CACHE_CHECK,
            next_rune_check: FIRST_RUNE_CHECK,
            cache_message_at: -10.0,
            magnets: Vec::new(),
        }
    }
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Run {
    /// The run has just started: its opening enemies, the first caches, and the magnet schedule.
    pub(crate) fn open_run(&mut self) {
        for _ in 0..OPENING_ENEMIES {
            self.spawn_roster_enemy(0, None);
        }
        self.plan_rares();
        self.plan_standards();
        self.opening_milestones();
        for _ in 0..CACHES_AT_START {
            self.place_pickup(Pickup::BattlegroundCache, CACHE_RING);
        }
        let final_minute = self.content.zones[self.zone].bosses.iter().find(|b| b.is_final).map(|b| b.spawn_minute);
        self.field.magnets = magnet_schedule(final_minute, &mut self.rng)
            .into_iter()
            .map(|(due, appears)| MagnetSlot { due, appears, placed: false })
            .collect();
    }

    /// Puts a pickup at a random spot `ring` yards from a hero.
    pub(crate) fn place_pickup(&mut self, kind: Pickup, ring: (f32, f32)) {
        let Some(h) = self.anchor_hero() else { return };
        let at = self.spawn_point(h, ring);
        self.drop(kind, 0, at);
    }

    /// Lays a rune, cache or magnet at `at`, as the run would (a development aid: a live look at
    /// a rune taken at a known place).
    pub fn lay_pickup(&mut self, kind: Pickup, at: Vec2) {
        self.drop(kind, 0, at);
    }

    pub(crate) fn drop(&mut self, kind: Pickup, value: u32, at: Vec2) {
        if kind == Pickup::BattlegroundCache {
            let caches: Vec<usize> = (0..self.pickups.len()).filter(|&k| self.pickups[k].kind == kind).collect();
            if caches.len() >= CACHE_LIMIT {
                // past the limit, the oldest cache goes
                let oldest = caches.into_iter().min_by(|&x, &y| self.pickups[x].spawned_at.total_cmp(&self.pickups[y].spawned_at)).unwrap();
                self.pickups.remove(oldest);
            }
        }
        let id = self.new_id();
        self.pickups.push(FieldPickup { id, kind, value, at, spawned_at: self.now, flying: None });
        self.push(Event::PickupDropped { id, kind, value, at });
    }

    /// An experience orb; past `MAX_LOOSE_ORBS` loose orbs, the value joins the nearest one.
    pub(crate) fn drop_orb(&mut self, at: Vec2, value: u32) {
        if value == 0 {
            return;
        }
        let loose: Vec<usize> =
            (0..self.pickups.len()).filter(|&k| self.pickups[k].kind == Pickup::Experience && self.pickups[k].flying.is_none()).collect();
        if loose.len() >= MAX_LOOSE_ORBS {
            let nearest = loose.into_iter().min_by(|&x, &y| dist(self.pickups[x].at, at).total_cmp(&dist(self.pickups[y].at, at))).unwrap();
            self.pickups[nearest].value += value;
            return;
        }
        self.drop(Pickup::Experience, value, at);
    }

    /// A boss died at `at`: the first magnet that is due and not yet placed drops there.
    pub(crate) fn drop_due_magnet(&mut self, at: Vec2) {
        let now = self.now;
        if let Some(m) = self.field.magnets.iter_mut().find(|m| !m.placed && m.due <= now) {
            m.placed = true;
            self.drop(Pickup::Magnet, 0, at);
        }
    }

    /// The living hero nearest `at` within `range(hero)`, with the distance.
    fn nearest_hero_within(&self, at: Vec2, range: impl Fn(&super::run::Hero) -> f32) -> Option<(usize, f32)> {
        self.heroes
            .iter()
            .enumerate()
            .filter(|(_, h)| h.alive && dist(h.pos, at) <= range(h))
            .map(|(i, h)| (i, dist(h.pos, at)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Orbs start flying, flying things land, and heroes take what they reach.
    pub(crate) fn pickups_tick(&mut self) {
        let now = self.now;
        // experience orbs set off
        for k in 0..self.pickups.len() {
            let p = &self.pickups[k];
            if p.kind != Pickup::Experience || p.flying.is_some() {
                continue;
            }
            let old = now - p.spawned_at > ORB_SELF_COLLECT_AGE;
            let target = match self.nearest_hero_within(p.at, |h| h.stats.pickup_range) {
                Some(t) => Some((t, if old { ORB_OLD_SPEED } else { ORB_SPEED })),
                None if old => self.nearest_hero_within(p.at, |_| f32::MAX).map(|t| (t, ORB_OLD_SPEED)),
                None => None,
            };
            if let Some(((h, d), speed)) = target {
                self.pickups[k].flying = Some((h, now + (d / speed).max(ORB_MIN_FLIGHT)));
            }
        }
        // what is taken this tick
        let mut taken = Vec::new();
        for k in 0..self.pickups.len() {
            let p = &self.pickups[k];
            let arrived = p.flying.filter(|&(_, at)| now >= at).map(|(h, _)| h);
            let who = match p.kind {
                Pickup::Experience => arrived,
                Pickup::Gold => self.nearest_hero_within(p.at, |h| h.stats.pickup_range).map(|(h, _)| h).or(arrived),
                _ => self.nearest_hero_within(p.at, |_| TAKE_RANGE).map(|(h, _)| h),
            };
            if let Some(h) = who {
                taken.push((k, h));
            }
        }
        let mut gone = Vec::new();
        for &(k, h) in taken.iter().rev() {
            if self.take(k, h) {
                gone.push(k);
            }
        }
        gone.sort_unstable();
        for k in gone.into_iter().rev() {
            self.pickups.remove(k);
        }
    }

    /// Hero `h` takes pickup `k`; false when it stays (a cache the hero cannot pay for).
    fn take(&mut self, k: usize, h: usize) -> bool {
        let p = self.pickups[k].clone();
        let hero = self.heroes[h].id;
        let now = self.now;
        match p.kind {
            Pickup::Experience => self.gain_experience(h, p.value),
            Pickup::Gold => {
                let gold = (p.value as f32 * self.heroes[h].stats.gold).round().max(0.0) as u32;
                self.heroes[h].gold += gold;
                self.heroes[h].gold_earned += gold;
            }
            Pickup::BattlegroundCache => {
                let price = self.field.cache_price;
                if self.heroes[h].gold < price {
                    if now - self.field.cache_message_at > CACHE_MESSAGE_GAP {
                        self.field.cache_message_at = now;
                        self.push(Event::Notice { hero, text: super::lines::cache_price(price) });
                        self.push(Event::Announce(super::lines::cache_price(price)));
                    }
                    return false;
                }
                self.heroes[h].gold -= price;
                self.field.cache_price = next_cache_price(price);
                self.heroes[h].pending_caches += 1;
                self.heroes[h].caches_opened += 1;
                self.push(Event::Sound(CACHE_SOUND));
                self.push(Event::CacheOpened { hero, price });
            }
            // a rare cache costs nothing and leaves the cache price as it is
            Pickup::RareCache => {
                self.heroes[h].pending_rare_caches += 1;
                self.heroes[h].caches_opened += 1;
                self.push(Event::Sound(CACHE_SOUND));
                self.push(Event::CacheOpened { hero, price: 0 });
            }
            Pickup::SpeedRune | Pickup::BerserkerRune => {
                let (kind, (value, seconds), visual, line) = if p.kind == Pickup::SpeedRune {
                    (BuffKind::Speed, SPEED_RUNE, SPEED_RUNE_VISUAL, super::lines::SPEED_RUNE)
                } else {
                    (BuffKind::Berserk, BERSERKER_RUNE, BERSERKER_RUNE_VISUAL, super::lines::BERSERKER_RUNE)
                };
                let me = super::run::Unit::Hero(hero);
                self.push(Event::SpellGo { caster: me, spell: visual, hits: vec![me], aim: super::run::Aim::Caster, ammo: false, reach: 0.0, triggered: false });
                self.push(Event::Announce(line.into()));
                // the same rune's buff starts over
                self.heroes[h].buffs.retain(|b| b.kind != kind);
                self.heroes[h].buffs.push(Buff { kind, value, visual, until: now + seconds });
                self.refresh_stats(h);
            }
            Pickup::RestorationRune => {
                let max = self.heroes[h].stats.max_health;
                self.heal_hero(h, max * RESTORATION_HEAL, 0);
                let me = super::run::Unit::Hero(hero);
                self.push(Event::SpellGo { caster: me, spell: RESTORATION_RUNE_VISUAL, hits: vec![me], aim: super::run::Aim::Caster, ammo: false, reach: 0.0, triggered: false });
                self.push(Event::Announce(super::lines::RESTORATION_RUNE.into()));
            }
            Pickup::Magnet => {
                self.heroes[h].magnets_taken += 1;
                let at = self.heroes[h].pos;
                for q in self.pickups.iter_mut() {
                    if matches!(q.kind, Pickup::Experience | Pickup::Gold) && q.flying.is_none() {
                        q.flying = Some((h, now + (dist(q.at, at) / MAGNET_SPEED).max(MAGNET_MIN_FLIGHT)));
                    }
                }
                self.push(Event::Sound(MAGNET_SOUND));
                self.push(Event::Announce(super::lines::MAGNET_TAKEN.into()));
            }
            Pickup::BattleStandard(_) => {
                // taken first, then the ally comes
                self.push(Event::PickupTaken { id: p.id, hero });
                self.summon_ally(h);
                return true;
            }
        }
        self.push(Event::PickupTaken { id: p.id, hero });
        true
    }

    /// Caches every 70 s (from 60 s) while few lie around, runes every 45 s (from 40 s), and
    /// magnets when their time comes.
    pub(crate) fn field_timers_tick(&mut self) {
        let now = self.now;
        let count = |run: &Run, kinds: &[Pickup]| run.pickups.iter().filter(|p| kinds.contains(&p.kind)).count();
        if now >= self.field.next_cache_check {
            self.field.next_cache_check = now + CACHE_EVERY;
            if count(self, &[Pickup::BattlegroundCache]) < CACHE_MAX_PRESENT {
                self.place_pickup(Pickup::BattlegroundCache, CACHE_RING);
            }
        }
        if now >= self.field.next_rune_check {
            self.field.next_rune_check = now + RUNE_EVERY;
            if count(self, &[Pickup::SpeedRune, Pickup::BerserkerRune, Pickup::RestorationRune]) < RUNE_MAX_PRESENT {
                let rune = random_rune(&mut self.rng);
                self.place_pickup(rune, RUNE_RING);
            }
        }
        let ready: Vec<usize> = (0..self.field.magnets.len()).filter(|&i| !self.field.magnets[i].placed && self.field.magnets[i].appears <= now).collect();
        for i in ready {
            self.field.magnets[i].placed = true;
            self.place_pickup(Pickup::Magnet, MAGNET_RING);
            self.push(Event::Announce(super::lines::MAGNET_APPEARED.into()));
        }
    }

    /// Experience from an orb for hero `i`, times its experience bonus (rounded).
    pub fn gain_experience(&mut self, i: usize, value: u32) {
        let xp = (value as f32 * self.heroes[i].stats.experience).round().max(0.0) as u32;
        self.grant_experience(i, xp);
    }

    /// Experience as it is. Each level gained, in turn: a choice is queued, the stats grow, the
    /// hero is told what the level brought (damage, maximum health, health per second) and heals
    /// 20 %.
    pub fn grant_experience(&mut self, i: usize, xp: u32) {
        let levels = self.heroes[i].progress.gain(xp, &self.content.balance.experience);
        let (hero, top) = (self.heroes[i].id, self.heroes[i].progress.level);
        for level in top + 1 - levels..=top {
            let before = self.heroes[i].stats;
            self.heroes[i].progress.level = level;
            self.heroes[i].pending_choices += 1;
            self.refresh_stats(i);
            let after = self.heroes[i].stats;
            let text = super::lines::level_up_gains((after.damage - before.damage) * 100.0, after.max_health - before.max_health, after.health_regen - before.health_regen);
            self.push(Event::Told { hero, text });
            self.heal_hero(i, after.max_health * LEVEL_UP_HEAL, 0);
        }
        if levels > 0 {
            self.push(Event::LevelUp { hero, level: top });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::Content;

    use super::*;
    use crate::game::enemy::RunOptions;
    use crate::game::run::{Flat, HeroSetup, Phase};

    fn run() -> Run {
        run_with(None)
    }

    /// A warrior run with weapon loadout `loadout` (none: its first).
    fn run_with(loadout: Option<&str>) -> Run {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Arc::new(Content::load(dir).expect("content loads"));
        let mut r = Run::new(
            content,
            "elwynn",
            RunOptions::default(),
            &[HeroSetup { class: "warrior".into(), race: "human".into(), loadout: loadout.map(Into::into), ..Default::default() }],
            Box::new(Flat),
            5,
        )
        .unwrap();
        let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
        r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        r.drain_events();
        r
    }

    #[test]
    fn each_level_tells_the_hero_what_it_brought() {
        // the shield loadout: +10 % max health, no half tenth for the mean of two levels to round away
        let mut r = run_with(Some("shield"));
        let start = r.heroes[0].stats;
        // levels 1 to 3: 200 + 265 experience
        r.grant_experience(0, 465);
        assert_eq!(r.heroes[0].progress.level, 3);
        let told: Vec<String> = r.drain_events().into_iter().filter_map(|e| if let Event::Told { text, .. } = e { Some(text) } else { None }).collect();
        assert_eq!(told.len(), 2, "one line a level: {told:?}");
        let health = r.heroes[0].stats.max_health - start.max_health;
        assert!(health > 0.0, "a level brings health");
        let per_level = super::super::lines::level_up_gains(
            (r.heroes[0].stats.damage - start.damage) * 50.0,
            health / 2.0,
            (r.heroes[0].stats.health_regen - start.health_regen) / 2.0,
        );
        assert_eq!(told[0], per_level, "the same gains each level");
        assert!(!told[0].contains("+0 maximum health"), "{}", told[0]);
    }

    fn near(r: &Run, d: f32) -> Vec2 {
        let p = r.heroes[0].pos;
        (p.0 + d, p.1)
    }

    #[test]
    fn a_run_opens_with_four_enemies_two_caches_and_magnets() {
        let r = run();
        assert_eq!(r.phase, Phase::Running);
        assert_eq!(r.enemies.iter().filter(|e| e.kind == crate::game::run::EnemyKind::Roster(0)).count(), 4);
        assert_eq!(r.pickups.iter().filter(|p| p.kind == Pickup::BattlegroundCache).count(), 2);
        assert!((2..=3).contains(&r.field.magnets.len()));
    }

    #[test]
    fn far_orbs_come_in_by_themselves_after_six_seconds() {
        let mut r = run();
        r.enemies.clear();
        let at = near(&r, 60.0);
        r.drop_orb(at, 100);
        let orb = r.pickups.last().unwrap().id;
        let mut picked = None;
        for _ in 0..200 {
            r.enemies.clear();
            r.tick(0.05);
            if r.drain_events().contains(&Event::PickupTaken { id: orb, hero: r.heroes[0].id }) {
                picked = Some(r.now);
                break;
            }
        }
        let t = picked.expect("the orb came in");
        // leaves after 6 s, flies 60 yd at 32 yd/s
        assert!((7.5..8.2).contains(&t), "collected at {t}");
    }

    #[test]
    fn gold_is_taken_in_pickup_range_times_the_gold_bonus() {
        let mut r = run();
        r.enemies.clear();
        r.heroes[0].stats.gold = 1.5;
        let at = near(&r, 8.0);
        r.drop(Pickup::Gold, 3, at);
        r.pickups_tick();
        assert_eq!(r.heroes[0].gold, 5, "3 x 1.5 = 4.5 rounds to 5"); // round half away from zero
        assert_eq!(r.heroes[0].gold_earned, 5);
    }

    #[test]
    fn a_cache_needs_gold_and_gets_dearer() {
        let mut r = run();
        r.enemies.clear();
        r.pickups.clear();
        let at = near(&r, 2.0);
        r.drop(Pickup::BattlegroundCache, 0, at);
        r.pickups_tick();
        assert_eq!(r.pickups.len(), 1, "no gold: it stays");
        assert!(r.drain_events().contains(&Event::Announce(super::super::lines::cache_price(20))));
        r.heroes[0].gold = 25;
        r.pickups_tick();
        assert!(r.pickups.is_empty());
        assert_eq!((r.heroes[0].gold, r.heroes[0].pending_caches, r.field.cache_price), (5, 1, 30));
    }

    #[test]
    fn runes_buff_and_restore() {
        let mut r = run();
        r.enemies.clear();
        let speed = r.heroes[0].stats.move_speed;
        let at = near(&r, 1.0);
        r.drop(Pickup::SpeedRune, 0, at);
        r.pickups_tick();
        assert!((r.heroes[0].stats.move_speed - speed - 0.6).abs() < 1e-5);
        r.heroes[0].health = 10.0;
        r.drop(Pickup::RestorationRune, 0, at);
        r.pickups_tick();
        assert!((r.heroes[0].health - (10.0 + r.heroes[0].stats.max_health * 0.5)).abs() < 1e-3);
    }

    #[test]
    fn a_magnet_pulls_every_orb_and_coin() {
        let mut r = run();
        r.enemies.clear();
        let far = near(&r, 45.0);
        r.drop_orb(far, 100);
        r.drop(Pickup::Gold, 4, far);
        let at = near(&r, 1.0);
        r.drop(Pickup::Magnet, 0, at);
        r.pickups_tick();
        assert!(r.pickups.iter().filter(|p| matches!(p.kind, Pickup::Experience | Pickup::Gold)).all(|p| p.flying.is_some()));
        for _ in 0..30 {
            r.enemies.clear();
            r.tick(0.05);
        }
        // a human's `diplomacy`: +20 % gold, so 4 x 1.2 = 4.8 rounds to 5
        assert_eq!(r.heroes[0].gold, (4.0 * r.heroes[0].stats.gold).round() as u32);
        assert_eq!(r.heroes[0].gold, 5);
        assert_eq!(r.heroes[0].magnets_taken, 1);
    }

    #[test]
    fn caches_and_runes_appear_on_their_timers() {
        let mut r = run();
        r.pickups.clear();
        r.now = 60.0;
        r.field_timers_tick();
        assert_eq!(r.pickups.iter().filter(|p| p.kind == Pickup::BattlegroundCache).count(), 1);
        assert!(r.pickups.iter().any(|p| matches!(p.kind, Pickup::SpeedRune | Pickup::BerserkerRune | Pickup::RestorationRune)));
        assert_eq!((r.field.next_cache_check, r.field.next_rune_check), (130.0, 105.0));
    }
}
