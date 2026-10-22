//! Milestones that reshape the fight around the hero (the legendary
//! amulets use them): a hit that pierces a line of enemies, enemies marked for striking the hero,
//! ground areas that grow as they strike or move with the hero, a pull before a cast, a trail of
//! fire behind burning enemies, and feared enemies turned on the others.

use esurv_content::{Ability, Milestone, School, SpellEffect};

use super::casting::{Area, HitSource, ProcOrigin, AREA_LOOK_AFTER};
use super::enemy_ai::{attack_damage, chase, flee, reach, Segment, Vec2};
use super::run::{Aim, EnemyUnit, Event, Run, Unit, AMMO_SPELLS};

/// A pulled enemy lands this far from the hero, on its own side.
pub const PULL_STANDOFF: f32 = 2.0;
/// A burning enemy leaves a patch of fire at most this often...
pub const FIRE_TRAIL_EVERY: f32 = 1.0;
/// ...and a hero's trails keep at most this many patches burning at once.
pub const FIRE_TRAIL_MAX: usize = 12;
/// A patch of fire burns once a second.
pub const FIRE_TRAIL_TICK: f32 = 1.0;
/// An area's look is shown again once it has moved this far...
pub const AREA_RESHOW_MOVE: f32 = 2.0;
/// ...or grown by this share of the radius it was last shown at (or to its widest).
pub const AREA_RESHOW_GROWTH: f32 = 0.1;

/// The shape of a `LineHit`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub length: f32,
    pub width: f32,
    pub behind: bool,
    pub hits_target: bool,
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Run {
    /// `LineHit`: every enemy in `line` from hero `h` through `target` (struck at `at`) takes
    /// `hit`, nearest first, shown as `look_spell` (else the hit's spell) flying to the line's end.
    pub(crate) fn line_hit(&mut self, h: usize, target: u32, at: Vec2, hit: f32, line: Line, look_spell: Option<u32>, source: &HitSource) {
        let from = self.heroes[h].pos;
        let d = dist(from, at);
        if d < 0.01 {
            return;
        }
        let dir = ((at.0 - from.0) / d, (at.1 - from.1) / d);
        let end = (from.0 + dir.0 * line.length, from.1 + dir.1 * line.length);
        let now = self.now;
        let mut struck: Vec<(f32, u32)> = self
            .enemies
            .iter()
            .filter_map(|e| {
                let p = e.position(now);
                let (x, y) = (p.0 - from.0, p.1 - from.1);
                let along = x * dir.0 + y * dir.1;
                let across = (x * dir.1 - y * dir.0).abs();
                let inside = (0.0..=line.length).contains(&along) && across <= line.width;
                let wanted = if e.id == target { line.hits_target } else { !line.behind || along > d };
                (inside && wanted).then_some((along, e.id))
            })
            .collect();
        struck.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (spell, school) = self.shown_spell(source);
        let look = look_spell.unwrap_or(spell);
        if look != 0 {
            let caster = Unit::Hero(self.heroes[h].id);
            let hits = struck.iter().map(|&(_, id)| Unit::Enemy(id)).collect();
            self.push(Event::SpellGo { caster, spell: look, hits, aim: Aim::Ground(end), ammo: AMMO_SPELLS.contains(&look), reach: line.width });
        }
        for (_, t) in struck {
            self.hit_enemy(Some(h), t, hit, false, self.proc_source(look, school));
        }
    }

    /// `MarkAttackers`: how much more hero `h`'s hit from `source` deals to the enemy at index `k`
    /// and the share of max health it heals, when the enemy is marked by the hero.
    pub(crate) fn attacker_mark_rule(&self, h: usize, k: usize, source: &HitSource) -> (f32, f32) {
        let Some((by, until, _)) = self.enemies[k].kit.attacker_mark else { return (0.0, 0.0) };
        if by != h || self.now >= until {
            return (0.0, 0.0);
        }
        self.heroes[h].milestones.iter().fold((0.0, 0.0), |(b, x), m| match m {
            Milestone::MarkAttackers { bonus, heal, filter, .. } if self.filter_passes(filter, source) => (b + bonus, x + heal),
            _ => (b, x),
        })
    }

    /// `AreaFollowsHero`: whether hero `i`'s area of `a` moves with the hero.
    pub(crate) fn area_follows(&self, i: usize, a: &Ability) -> bool {
        self.heroes[i].milestones.iter().any(|m| matches!(m, Milestone::AreaFollowsHero { filter } if filter.matches(a)))
    }

    /// `GrowingArea`: how many yards hero `i`'s area of `a`, landing `r` wide, grows per enemy it
    /// damages, and its widest radius; `None` when it does not grow.
    pub(crate) fn area_growth(&self, i: usize, a: &Ability, r: f32) -> Option<(f32, f32)> {
        let (per_hit, max) = self.heroes[i].milestones.iter().fold((0.0, 0.0f32), |(p, x), m| match m {
            Milestone::GrowingArea { per_hit, max, filter, .. } if filter.matches(a) => (p + per_hit, x.max(*max)),
            _ => (p, x),
        });
        (per_hit > 0.0).then_some((per_hit * r, r * (1.0 + max)))
    }

    /// `GrowingArea`: the look hero `i`'s growing area of `a` wears in place of the spell's own,
    /// if a legendary gives one.
    pub(crate) fn area_look(&self, i: usize, a: &Ability) -> Option<u32> {
        self.heroes[i].milestones.iter().find_map(|m| match m {
            Milestone::GrowingArea { filter, look_spell, .. } if filter.matches(a) => *look_spell,
            _ => None,
        })
    }

    /// Area `id` damaged `landed` enemies: a growing one grows, up to its widest.
    pub(crate) fn grow_area(&mut self, id: u32, landed: u32) {
        if let Some(ar) = self.areas.iter_mut().find(|a| a.id == id) {
            if let Some((step, widest)) = ar.growth {
                ar.radius = (ar.radius + step * landed as f32).min(widest).max(ar.radius);
            }
        }
    }

    /// Areas whose look has moved or grown enough since it was shown are shown again.
    pub(crate) fn reshow_changed_areas(&mut self) {
        let mut changed = Vec::new();
        for ar in &mut self.areas {
            let moved = dist(ar.at, ar.shown_at) >= AREA_RESHOW_MOVE;
            let widest = ar.growth.is_some_and(|(_, w)| ar.radius >= w);
            let grew = ar.radius > ar.shown_radius + 1e-3 && (ar.radius >= ar.shown_radius * (1.0 + AREA_RESHOW_GROWTH) || widest);
            if moved || grew {
                ar.shown_at = ar.at;
                ar.shown_radius = ar.radius;
                changed.push(Event::AreaChanged { id: ar.id, at: ar.at, radius: ar.radius });
            }
        }
        for e in changed {
            self.push(e);
        }
    }

    /// `PullEveryNth`: before hero `i` casts `a`, every `count`-th such cast (counting those
    /// with an enemy in reach) pulls the enemies within its radius (not bosses) to the hero's side.
    pub(crate) fn pull_before_cast(&mut self, i: usize, a: &Ability) {
        let pulls: Vec<(usize, u32, f32, Option<u32>)> = self.heroes[i]
            .milestones
            .iter()
            .enumerate()
            .filter_map(|(k, m)| match m {
                Milestone::PullEveryNth { count, radius, look_spell, filter } if filter.matches(a) => Some((k, *count, *radius, *look_spell)),
                _ => None,
            })
            .collect();
        let (now, hero) = (self.now, self.heroes[i].pos);
        for (k, count, radius, look_spell) in pulls {
            let near: Vec<u32> = self.enemies_within(hero, radius).into_iter().filter(|&id| self.enemy(id).is_some_and(|e| !e.is_boss())).collect();
            if near.is_empty() {
                continue;
            }
            let st = self.state(i, k);
            st.counter += 1;
            if st.counter < count.max(1) {
                continue;
            }
            st.counter = 0;
            if let Some(look) = look_spell {
                let hits = near.iter().map(|&id| Unit::Enemy(id)).collect();
                self.push(Event::SpellGo { caster: Unit::Hero(self.heroes[i].id), spell: look, hits, aim: Aim::Caster, ammo: false, reach: radius });
            }
            for id in near {
                let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) else { continue };
                let p = e.position(now);
                let d = dist(p, hero);
                if d <= PULL_STANDOFF {
                    continue;
                }
                let spot = (hero.0 + (p.0 - hero.0) / d * PULL_STANDOFF, hero.1 + (p.1 - hero.1) / d * PULL_STANDOFF);
                e.segment = Segment::standing(spot, now);
                e.next_decision = now;
                e.charging_until = 0.0;
            }
        }
    }

    /// `FireTrail`: enemies burning from a hero's matching effects over time leave patches of fire
    /// behind them as they move.
    pub(crate) fn fire_trails_tick(&mut self) {
        let now = self.now;
        for h in 0..self.heroes.len() {
            if !self.heroes[h].alive {
                continue;
            }
            let trails: Vec<(f32, f32, f32, Option<u32>, esurv_content::SpellFilter, ProcOrigin)> = self.heroes[h]
                .milestones
                .iter()
                .enumerate()
                .filter_map(|(k, m)| match m {
                    Milestone::FireTrail { share, radius, seconds, look_spell, filter } => Some((*share, *radius, *seconds, *look_spell, filter.clone(), self.origin_of(h, k))),
                    _ => None,
                })
                .collect();
            for (share, radius, seconds, look_spell, filter, origin) in trails {
                // each burning enemy: where it is and the burn's damage a second
                let burning: Vec<(u32, Vec2, f32, String)> = self
                    .enemies
                    .iter()
                    .filter_map(|e| {
                        let d = e.dots.iter().find(|d| d.hero == h && d.ticks_left > 0 && self.filter_passes(&filter, &HitSource::Spell(d.spell.clone())))?;
                        Some((e.id, e.position(now), d.per_tick / d.interval.max(0.01), d.spell.clone()))
                    })
                    .collect();
                for e in self.enemies.iter_mut().filter(|e| !burning.iter().any(|b| b.0 == e.id)) {
                    e.kit.trail_from = None;
                }
                for (id, pos, per_second, spell) in burning {
                    let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) else { continue };
                    let Some(from) = e.kit.trail_from else {
                        e.kit.trail_from = Some(pos);
                        e.kit.trail_next = now;
                        continue;
                    };
                    let live = self.areas.iter().filter(|a| a.hero == h && a.proc_hit.is_some()).count();
                    if now < e.kit.trail_next || dist(pos, from) < radius || live >= FIRE_TRAIL_MAX {
                        continue;
                    }
                    e.kit.trail_from = Some(pos);
                    e.kit.trail_next = now + FIRE_TRAIL_EVERY;
                    self.fire_patch(h, from, radius, seconds, share * per_second, look_spell, &spell, origin.clone());
                }
            }
        }
    }

    /// A patch of fire of hero `h` at `at`: `per_tick` once a second for `seconds` on every enemy
    /// within `radius`, its hits shown as `look_spell` (else the burning spell's look).
    #[allow(clippy::too_many_arguments)]
    fn fire_patch(&mut self, h: usize, at: Vec2, radius: f32, seconds: f32, per_tick: f32, look_spell: Option<u32>, spell: &str, origin: ProcOrigin) {
        let (look, school) = match self.content.ability(spell) {
            Some(a) => (look_spell.unwrap_or(a.look_spell), a.school),
            None => (look_spell.unwrap_or(0), School::Fire),
        };
        let id = self.new_id();
        let ticks = (seconds / FIRE_TRAIL_TICK).round().max(1.0) as u32;
        if let Some(l) = look_spell {
            let until = Some(self.now + seconds + AREA_LOOK_AFTER);
            self.push(Event::AreaShown { id, caster: Unit::Hero(self.heroes[h].id), spell: l, at, radius, until });
        }
        self.areas.push(Area {
            id,
            hero: h,
            spell: String::new(),
            at,
            radius,
            per_tick,
            ticks_left: ticks,
            interval: FIRE_TRAIL_TICK,
            next: self.now + FIRE_TRAIL_TICK,
            from_proc: false,
            base_radius: radius,
            growth: None,
            follows: false,
            proc_hit: Some((look, school, origin)),
            left: None,
            shown_at: at,
            shown_radius: radius,
        });
    }

    /// `FearedTurn`: hero `h`'s hit from `source` on `target`, a spell that fears, turns it on the
    /// other enemies for the fear's time, with `chance` (never a boss).
    pub(crate) fn maybe_turn(&mut self, h: usize, target: u32, chance: f32, source: &HitSource) {
        let seconds = match source {
            HitSource::Spell(id) => match self.content.ability(id).map(|a| a.effect) {
                Some(SpellEffect::Fear { seconds }) => seconds,
                _ => return,
            },
            _ => return,
        };
        if self.enemy(target).is_none_or(EnemyUnit::is_boss) || self.rng.unit() >= chance {
            return;
        }
        let now = self.now;
        if let Some(e) = self.enemies.iter_mut().find(|e| e.id == target) {
            e.kit.turned = Some((h, now + seconds));
            e.next_decision = now;
        }
    }

    /// A turned enemy (index `k`, at `pos`, moving at `speed`) chases the nearest other enemy and
    /// strikes it with its own blows; with none left it flees the hero as fear would.
    pub(crate) fn turned_enemy_tick(&mut self, k: usize, pos: Vec2, speed: f32, dt: f32) {
        let now = self.now;
        let Some((h, _)) = self.enemies[k].kit.turned else { return };
        let me = self.enemies[k].id;
        let nearest = self
            .enemies
            .iter()
            .filter(|o| o.id != me)
            .map(|o| (dist(o.position(now), pos), o.id, o.position(now)))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let hero_pos = self.heroes[h].pos;
        let e = &mut self.enemies[k];
        let Some((d, target, at)) = nearest else {
            if now >= e.next_decision {
                let (seg, next) = flee(pos, hero_pos, speed, now);
                e.segment = seg;
                e.next_decision = next;
            }
            return;
        };
        let (chase_beyond, _) = reach(e.collision_radius, None);
        if d > chase_beyond {
            if now >= e.next_decision || !e.segment.moving(now) {
                let (seg, next) = chase(pos, at, &mut e.aim_angle, 0.0, speed, now);
                e.segment = seg;
                e.next_decision = next;
            }
            return;
        }
        if e.segment.moving(now) {
            e.segment = Segment::standing(pos, now);
        }
        e.swing_timer -= dt;
        if e.swing_timer > 0.0 {
            return;
        }
        e.swing_timer = e.attack_interval;
        let amount = attack_damage(e.damage, EnemyUnit::active(e.weaken, now), false);
        // its blows are the hero's doing but set off no milestones
        self.casting_proc += 1;
        self.hit_enemy(Some(h), target, amount, false, HitSource::Enemy(me));
        self.casting_proc -= 1;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::{Content, SpellFilter, Trigger};

    use super::*;
    use crate::game::enemy::RunOptions;
    use crate::game::milestones::CombatEvent;
    use crate::game::run::{Flat, HeroSetup};

    fn run(milestones: Vec<Milestone>) -> Run {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Arc::new(Content::load(dir).expect("content loads"));
        let setup = HeroSetup { class: "mage".into(), race: "orc".into(), milestones, ..Default::default() };
        let mut r = Run::new(content, "elwynn", RunOptions::default(), &[setup], Box::new(Flat), 23).unwrap();
        let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
        r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        r.enemies.clear();
        r.pickups.clear();
        r.drain_events();
        r
    }

    /// An enemy `offset` from the hero, with plenty of health.
    fn enemy(r: &mut Run, offset: Vec2) -> u32 {
        let p = r.heroes[0].pos;
        assert!(r.spawn_roster_enemy(0, Some((p.0 + offset.0, p.1 + offset.1))));
        let e = r.enemies.last_mut().unwrap();
        e.health = 1e6;
        e.max_health = 1e6;
        e.id
    }

    fn lost(r: &Run, id: u32) -> f32 {
        r.enemies.iter().find(|e| e.id == id).map_or(0.0, |e| 1e6 - e.health)
    }

    fn spot(r: &Run, id: u32) -> Vec2 {
        r.enemies.iter().find(|e| e.id == id).unwrap().position(r.now)
    }

    fn line(every: u32, behind: bool, hits_target: bool) -> Milestone {
        let filter = SpellFilter::Spell("fireball".into());
        Milestone::LineHit { every, share: 0.5, length: 30.0, width: 2.0, behind, hits_target, look_spell: Some(19434), trigger: Trigger::Hit, filter }
    }

    fn fireball(r: &mut Run, id: u32) {
        r.hit_enemy(Some(0), id, 100.0, false, HitSource::Spell("fireball".into()));
    }

    #[test]
    fn a_line_hit_pierces_every_enemy_on_the_line_and_none_beside_it() {
        let mut r = run(vec![line(1, false, false)]);
        let near = enemy(&mut r, (5.0, 0.0));
        let target = enemy(&mut r, (10.0, 0.0));
        let past = enemy(&mut r, (20.0, 1.0));
        let beside = enemy(&mut r, (15.0, 6.0));
        let beyond = enemy(&mut r, (35.0, 0.0));
        fireball(&mut r, target);
        assert_eq!(lost(&r, target), 100.0, "the struck enemy only takes its own hit");
        assert_eq!((lost(&r, near), lost(&r, past)), (50.0, 50.0), "the rest of the line takes the share");
        assert_eq!((lost(&r, beside), lost(&r, beyond)), (0.0, 0.0), "off the line, past its end");
        let end = (r.heroes[0].pos.0 + 30.0, r.heroes[0].pos.1);
        let shown = r.drain_events().into_iter().any(|e| matches!(e, Event::SpellGo { spell: 19434, aim: Aim::Ground(at), ammo: true, .. } if dist(at, end) < 1e-3));
        assert!(shown, "the arrow flies to the line's end");
    }

    #[test]
    fn a_line_behind_the_target_spares_those_before_it_and_every_nth_counts() {
        let mut r = run(vec![line(3, true, true)]);
        let near = enemy(&mut r, (5.0, 0.0));
        let target = enemy(&mut r, (10.0, 0.0));
        let past = enemy(&mut r, (20.0, 0.0));
        fireball(&mut r, target);
        fireball(&mut r, target);
        assert_eq!(lost(&r, past), 0.0, "not before the third hit");
        fireball(&mut r, target);
        assert_eq!(lost(&r, past), 50.0);
        assert_eq!(lost(&r, target), 350.0, "the struck enemy takes the line too");
        assert_eq!(lost(&r, near), 0.0, "before the target");
    }

    #[test]
    fn marked_attackers_take_more_from_the_spell_and_heal_the_hero() {
        let filter = SpellFilter::Spell("fireball".into());
        let mut r = run(vec![Milestone::MarkAttackers { seconds: 5.0, bonus: 1.0, heal: 0.01, look_spell: Some(1), filter }]);
        let a = enemy(&mut r, (3.0, 0.0));
        let b = enemy(&mut r, (4.0, 0.0));
        r.damage_hero(0, 10.0, Some(a), true);
        assert!(r.enemies.iter().find(|e| e.id == a).unwrap().kit.attacker_mark.is_some());
        let health = r.heroes[0].health;
        fireball(&mut r, a);
        fireball(&mut r, b);
        assert_eq!((lost(&r, a), lost(&r, b)), (200.0, 100.0), "double on the marked one only");
        let max = r.heroes[0].stats.max_health;
        assert!((r.heroes[0].health - health - 0.01 * max).abs() < 1e-3, "one marked hit healed 1 %");
        r.hit_enemy(Some(0), a, 100.0, false, HitSource::Spell("frostbolt".into()));
        assert_eq!(lost(&r, a), 300.0, "another spell gets no bonus");
        r.now += 5.0;
        fireball(&mut r, a);
        assert_eq!(lost(&r, a), 400.0, "the mark ran out");
    }

    /// A lasting area of `spell` cast by the hero now.
    fn area_run(m: Milestone, spell: &str, offsets: &[Vec2]) -> (Run, Vec<u32>) {
        let mut r = run(vec![m]);
        let ids = offsets.iter().map(|o| enemy(&mut r, *o)).collect();
        let a = r.content.ability(spell).unwrap().clone();
        assert!(r.cast(0, &a, &Default::default()));
        (r, ids)
    }

    #[test]
    fn a_growing_area_widens_with_each_enemy_it_damages_up_to_its_cap() {
        let m = Milestone::GrowingArea { look_spell: None, per_hit: 0.05, max: 1.0, filter: SpellFilter::Spell("consecration".into()) };
        let (mut r, _) = area_run(m, "consecration", &[(1.0, 0.0), (2.0, 0.0), (0.0, 2.0), (-2.0, 0.0)]);
        let base = r.areas[0].radius;
        r.drain_events();
        r.now += 0.6;
        r.effects_tick();
        let grown = r.areas[0].radius;
        assert!((grown - base * 1.2).abs() < 1e-3, "four enemies: 20 % wider ({base} to {grown})");
        let events = r.drain_events();
        assert!(events.iter().any(|e| matches!(e, Event::AreaChanged { radius, .. } if (*radius - grown).abs() < 1e-3)), "shown wider");
        for _ in 0..40 {
            r.now += 0.5;
            r.effects_tick();
        }
        if let Some(a) = r.areas.first() {
            assert!((a.radius - base * 2.0).abs() < 1e-3, "at most twice as wide: {}", a.radius);
        }
    }

    #[test]
    fn a_following_area_lands_on_the_hero_and_moves_with_them() {
        let m = Milestone::AreaFollowsHero { filter: SpellFilter::Spell("blizzard".into()) };
        let (mut r, _) = area_run(m, "blizzard", &[(20.0, 0.0)]);
        let start = r.heroes[0].pos;
        assert!(dist(r.areas[0].at, start) < 1e-3, "on the hero, though the enemy stands 20 yd off");
        let id = r.heroes[0].id;
        r.hero_moved(id, (start.0 + 5.0, start.1), 0.0, 0.0);
        r.drain_events();
        r.now += 0.1;
        r.effects_tick();
        let to = r.heroes[0].pos;
        assert!(dist(r.areas[0].at, to) < 1e-3);
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::AreaChanged { at, .. } if dist(*at, to) < 1e-3)), "its look moves too");
    }

    #[test]
    fn every_nth_cast_first_pulls_the_enemies_near_the_hero_in() {
        let filter = SpellFilter::Spell("arcane_explosion".into());
        let mut r = run(vec![Milestone::PullEveryNth { count: 2, radius: 15.0, look_spell: None, filter }]);
        let near = enemy(&mut r, (12.0, 0.0));
        let far = enemy(&mut r, (0.0, 20.0));
        let a = r.content.ability("arcane_explosion").unwrap().clone();
        let hero = r.heroes[0].pos;
        r.cast(0, &a, &Default::default());
        assert!((dist(spot(&r, near), hero) - 12.0).abs() < 1e-3, "not on the first cast");
        r.cast(0, &a, &Default::default());
        assert!((dist(spot(&r, near), hero) - PULL_STANDOFF).abs() < 1e-3, "pulled in on the second");
        assert!(lost(&r, near) > 0.0, "and the explosion caught it");
        assert!((dist(spot(&r, far), hero) - 20.0).abs() < 1e-3, "out of the pull's reach");
    }

    #[test]
    fn burning_enemies_leave_fire_behind_that_burns_the_others() {
        let filter = SpellFilter::Spell("immolate".into());
        let mut r = run(vec![Milestone::FireTrail { share: 0.3, radius: 3.0, seconds: 4.0, look_spell: Some(2), filter }]);
        let burning = enemy(&mut r, (10.0, 0.0));
        let other = enemy(&mut r, (10.0, 0.5));
        let dot = crate::game::casting::Dot { hero: 0, spell: "immolate".into(), per_tick: 10.0, ticks_left: 100, interval: 3.0, next: 1e9, from_proc: false, leech: 0.0 };
        r.enemies.iter_mut().find(|e| e.id == burning).unwrap().dots.push(dot);
        r.fire_trails_tick();
        assert!(r.areas.is_empty(), "standing still leaves nothing");
        let (now, from) = (r.now, spot(&r, burning));
        r.enemies.iter_mut().find(|e| e.id == burning).unwrap().segment = Segment::standing((from.0 + 5.0, from.1), now);
        r.fire_trails_tick();
        assert_eq!(r.areas.len(), 1, "a patch where it walked from");
        assert!(dist(r.areas[0].at, from) < 1e-3);
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::AreaShown { spell: 2, .. })));
        r.now += 1.0;
        r.effects_tick();
        let hit = lost(&r, other);
        assert!((hit - 0.3 * 10.0 / 3.0).abs() < 1e-3 || r.heroes[0].stats.crit_chance > 0.0, "30 % of the burn's damage a second: {hit}");
        assert!(hit > 0.0);
    }

    #[test]
    fn a_turned_enemy_strikes_the_others_not_the_hero() {
        let filter = SpellFilter::Spell("psychic_scream".into());
        let mut r = run(vec![Milestone::FearedTurn { look_spell: None, chance: 1.0, filter }]);
        let turned = enemy(&mut r, (3.0, 0.0));
        let other = enemy(&mut r, (4.0, 0.0));
        let scream = HitSource::Spell("psychic_scream".into());
        r.milestone_event(0, CombatEvent::Hit { target: turned, at: spot(&r, turned), source: scream, crit: false, dealt: 1.0 });
        assert!(r.enemies.iter().find(|e| e.id == turned).unwrap().kit.turned.is_some(), "a fear spell turns it");
        // the other one stays out of the hero's reach so only the turned one could strike anyone
        let health = r.heroes[0].health;
        let k = r.enemies.iter().position(|e| e.id == turned).unwrap();
        r.enemies[k].swing_timer = 0.0;
        for _ in 0..4 {
            r.now += 0.05;
            let k = r.enemies.iter().position(|e| e.id == turned).unwrap();
            let pos = r.enemies[k].position(r.now);
            r.turned_enemy_tick(k, pos, 7.0, 0.05);
        }
        assert!(lost(&r, other) > 0.0, "it struck the other enemy");
        assert_eq!(r.heroes[0].health, health, "never the hero");
        let blows = r.drain_events().into_iter().any(|e| matches!(e, Event::EnemyHit { id, source: HitSource::Enemy(by), .. } if id == other && by == turned));
        assert!(blows);
    }

    #[test]
    fn a_spell_without_fear_turns_no_one() {
        let filter = SpellFilter::Any;
        let mut r = run(vec![Milestone::FearedTurn { look_spell: None, chance: 1.0, filter }]);
        let a = enemy(&mut r, (3.0, 0.0));
        fireball(&mut r, a);
        assert!(r.enemies[0].kit.turned.is_none());
    }

    #[test]
    fn a_summon_trigger_calls_several_and_an_afflicted_kill_counts_whatever_killed_it() {
        let filter = SpellFilter::Spell("corruption".into());
        let m = Milestone::PetOnTrigger { look_spell: None, chance: 1.0, seconds: 8.0, max: 3, pet: "Imp".into(), trigger: Trigger::Kill, filter, count: 2, afflicted: true };
        let mut r = run(vec![m]);
        for _ in 0..2 {
            let id = enemy(&mut r, (5.0, 0.0));
            let dot = crate::game::casting::Dot { hero: 0, spell: "corruption".into(), per_tick: 1.0, ticks_left: 5, interval: 1.0, next: 1e9, from_proc: false, leech: 0.0 };
            r.enemies.iter_mut().find(|e| e.id == id).unwrap().dots.push(dot);
            r.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Spell("fireball".into()));
        }
        assert_eq!(r.pets.len(), 3, "two from the first kill, one more to the cap");
        let id = enemy(&mut r, (5.0, 0.0));
        r.pets.clear();
        r.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Spell("corruption".into()));
        assert!(r.pets.is_empty(), "a kill without the effect on calls none");
    }
}
