//! What enemies throw at heroes besides their melee swing: ranged casters' shots, boss and
//! rare abilities (ground bursts, volleys, stomps, charges, summons), and the hits that land
//! later; plus what a hero's thorns and auras do back.

use esurv_content::BossAbility;

use super::boss_abilities::{cast, next_cooldown, AbilityEffect, GroundHit, BOSS_FIRST_CAST, RARE_FIRST_CAST};
use super::enemy_ai::{attack_damage, Vec2};
use super::run::{Aim, BuffKind, EnemyUnit, Event, Run, Unit};
use super::casting::{HitSource, ProcOrigin};

/// A hit landing later at a spot, with the enemy that made it.
pub type PlacedHit = (GroundHit, u32);

/// A boss or rare waits at least this long between two ability casts, so abilities that
/// came due together (for example during a stun) go off one after another.
pub const ABILITY_GAP: f32 = 1.5;

/// `thorns` strike back under this WoW spell's name in the combat log (Nature).
pub const THORNS_VISUAL: u32 = 467;

/// A ranged enemy's shot on its way: it always hits when it lands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnemyShot {
    pub source: u32,
    pub hero: usize,
    pub damage: f32,
    pub lands_at: f32,
    pub spell: u32,
}

/// One of a boss's or rare's abilities and the time left until its next use.
#[derive(Debug, Clone, PartialEq)]
pub struct AbilitySlot {
    pub ability: BossAbility,
    pub timer: f32,
}

impl AbilitySlot {
    /// A boss's abilities first fire at 60 % of their cooldown, a rare's at 50 %.
    pub fn boss(ability: &BossAbility) -> Self {
        Self { ability: ability.clone(), timer: ability.cooldown * BOSS_FIRST_CAST }
    }

    pub fn rare(ability: &BossAbility) -> Self {
        Self { ability: ability.clone(), timer: ability.cooldown * RARE_FIRST_CAST }
    }
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Run {
    /// A ranged enemy finished casting at hero `h`: its shot flies at the spell's missile speed.
    pub(crate) fn enemy_shoots(&mut self, k: usize, h: usize) {
        let now = self.now;
        let e = &self.enemies[k];
        let Some(spell) = e.ranged_spell else { return };
        let damage = attack_damage(e.damage, EnemyUnit::active(e.weaken, now), e.is_boss() && e.enraged) * e.shot_share;
        let speed = self.world.missile_speed(spell);
        let flight = if speed > 0.0 { dist(e.position(now), self.heroes[h].pos) / speed } else { 0.0 };
        let (id, hero) = (e.id, self.heroes[h].id);
        self.enemy_shots.push(EnemyShot { source: id, hero: h, damage, lands_at: now + flight, spell });
        let target = super::run::Unit::Hero(hero);
        // a shot from a bow, gun or crossbow (a hunter's Auto Shot) flies an arrow
        let ammo = super::run::AMMO_SPELLS.contains(&spell);
        self.push(Event::SpellGo { caster: super::run::Unit::Enemy(id), spell, hits: vec![target], aim: super::run::Aim::Unit(target), ammo, reach: 0.0 });
    }

    /// Counts down a boss's or rare's abilities and uses one that is ready on hero `h`: never
    /// while `held`, and at most one every `ABILITY_GAP` seconds, the longest-waiting first.
    pub(crate) fn enemy_abilities(&mut self, k: usize, h: usize, dt: f32, held: bool) {
        let now = self.now;
        let mut ready = Vec::new();
        {
            let e = &mut self.enemies[k];
            // rage stacked on a boss brings its marked attacks sooner (`game::boss_phases`; its
            // unmarked ones keep their pace: the damage a hero cannot avoid stays)
            let haste = e.mech.phases.haste(now);
            for slot in &mut e.abilities {
                slot.timer -= dt * if super::telegraphs::is_building_block(&slot.ability.kind) { haste } else { 1.0 };
            }
            let due = e.abilities.iter().enumerate().filter(|(_, s)| s.timer <= 0.0).min_by(|a, b| a.1.timer.total_cmp(&b.1.timer)).map(|(i, _)| i);
            if let (Some(i), false, true) = (due, held, now >= e.next_ability_at && now >= e.silenced_until) {
                let enraged = e.enraged;
                let slot = &mut e.abilities[i];
                slot.timer = next_cooldown(&slot.ability, enraged);
                ready.push((i, slot.ability.kind.clone(), slot.timer));
                e.next_ability_at = now + ABILITY_GAP;
            }
        }
        for (slot, kind, next) in ready {
            let e = &self.enemies[k];
            let (caster, id) = (e.position(now), e.id);
            let damage = attack_damage(e.damage, EnemyUnit::active(e.weaken, now), e.is_boss() && e.enraged);
            let (haste, harder) = (e.mech.phases.haste(now), e.mech.phases.harder(now));
            // its timer bar starts again (warnings and bars)
            if let Some(w) = kind.warning() {
                let icon = w.icon_spell.unwrap_or_else(|| super::telegraphs::kind_look(&kind));
                self.push(Event::BarStarted { boss: id, key: slot as u32, label: w.bar_label().into(), icon_spell: icon, until: now + next / haste });
            }
            // the building blocks (marks, rushes, objects, zones) are cast on their own, rage
            // making them hit harder
            if super::telegraphs::is_building_block(&kind) {
                self.cast_building_block(k, h, &kind, damage * harder);
                continue;
            }
            let hero = self.heroes[h].pos;
            let missiles = |spell: u32| self.world.missile_speed(spell);
            let mut rng = std::mem::replace(&mut self.rng, crate::rng::Rng::new(0));
            let effect = cast(&kind, caster, hero, damage, now, &mut rng, missiles);
            self.rng = rng;
            let (boss, target) = (Unit::Enemy(id), Unit::Hero(self.heroes[h].id));
            match effect {
                AbilityEffect::Ground { mut hits, marker } => {
                    // a burst marks the hero's spot until it lands (its caster is the hero)
                    if let Some((visual, at, radius)) = marker {
                        let mark = self.new_id();
                        self.push(Event::AreaShown { id: mark, caster: target, spell: visual, at, radius, until: None });
                        hits.iter_mut().for_each(|g| g.marker = Some(mark));
                    }
                    // a volley's shots show as they leave, each aimed at its spot (at a bunny waiting
                    // there for a look that shows only on a unit it hits)
                    let on_bunny = matches!(kind, esurv_content::BossAbilityKind::Volley { impact_on_bunny: true, .. });
                    for g in hits.iter().filter(|g| !g.look_on_landing && g.impact_spell != 0) {
                        if on_bunny {
                            let linger = (g.lands_at - now).max(0.0) + super::telegraphs::BUNNY_LINGER;
                            self.push(Event::ImpactAtSpot { caster: boss, spell: g.impact_spell, at: g.at, linger });
                        } else {
                            self.push(Event::SpellGo { caster: boss, spell: g.impact_spell, hits: vec![], aim: Aim::Ground(g.at), ammo: false, reach: g.radius });
                        }
                    }
                    self.ground_hits.extend(hits.into_iter().map(|g| (g, id)));
                }
                AbilityEffect::Instant { visual, damage, radius } => {
                    // a stomp: around the boss, listing the hero only when it is in reach
                    let hits = if damage.is_some() { vec![target] } else { vec![] };
                    self.push(Event::SpellGo { caster: boss, spell: visual, hits, aim: Aim::Caster, ammo: false, reach: radius });
                    if let Some(d) = damage {
                        self.damage_hero(h, d, Some(id), false);
                    }
                }
                AbilityEffect::Summon(adds) => {
                    let id = self.enemies[k].id;
                    // rares call theirs silently
                    if !self.enemies[k].is_rare() {
                        self.push(Event::Yell { id, text: super::lines::SUMMON_YELL.into() });
                    }
                    for (roster, off) in adds {
                        self.spawn_roster_enemy(roster, Some((caster.0 + off.0, caster.1 + off.1)));
                    }
                }
                AbilityEffect::Charge { visual, segment, landing, next_decision, swing_in } => {
                    self.push(Event::SpellGo { caster: boss, spell: visual, hits: vec![target], aim: Aim::Unit(target), ammo: false, reach: 0.0 });
                    if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                        e.segment = segment;
                        e.next_decision = next_decision;
                        e.swing_timer = swing_in;
                        e.charging_until = segment.start + segment.duration;
                    }
                    self.ground_hits.push((landing, id));
                }
            }
        }
    }

    /// Shots, ground hits and marks that land now.
    pub(crate) fn enemy_hits_tick(&mut self) {
        self.telegraphs_tick();
        let now = self.now;
        let (due, later): (Vec<EnemyShot>, Vec<EnemyShot>) = std::mem::take(&mut self.enemy_shots).into_iter().partition(|s| s.lands_at <= now);
        self.enemy_shots = later;
        for s in due {
            self.damage_hero(s.hero, s.damage, Some(s.source), false);
        }
        let (due, later): (Vec<PlacedHit>, Vec<PlacedHit>) =
            std::mem::take(&mut self.ground_hits).into_iter().partition(|(g, _)| g.lands_at <= now);
        self.ground_hits = later;
        for (g, source) in due {
            if g.look_on_landing && g.impact_spell != 0 {
                self.push(Event::SpellGo { caster: Unit::Enemy(source), spell: g.impact_spell, hits: vec![], aim: Aim::Ground(g.at), ammo: false, reach: g.radius });
            }
            if let Some(mark) = g.marker {
                self.push(Event::AreaGone { id: mark });
            }
            // a hit on the ground comes from no one (it counts as an area, not the boss)
            for h in 0..self.heroes.len() {
                if self.heroes[h].alive && g.hits(self.heroes[h].pos) {
                    self.damage_hero(h, g.damage, None, false);
                }
            }
        }
    }

    /// Hero `i` was struck in melee by enemy `source` for `taken` (after reduction and shield):
    /// an aura buff hits back for aura × (1 + buff power) × hero damage, thorns for thorns ×
    /// what was taken.
    pub(crate) fn strike_back(&mut self, i: usize, source: u32, taken: f32) {
        let now = self.now;
        let h = &self.heroes[i];
        let aura: f32 = h.buffs.iter().filter(|b| b.kind == BuffKind::Aura && now < b.until).map(|b| b.value).sum();
        if aura > 0.0 {
            let hit = aura * (1.0 + h.stats.buff_power) * h.stats.damage;
            // shown in the combat log as the aura's own spell (`thorns`' when it has none)
            let spell = h.buffs.iter().find(|b| b.kind == BuffKind::Aura && now < b.until && b.visual != 0).map_or(THORNS_VISUAL, |b| b.visual);
            // on the meter under the known spell that wears that look (Lightning Shield)
            let origin = h
                .spells
                .known
                .iter()
                .find(|s| self.content.ability(&s.spell).is_some_and(|a| a.look_spell == spell))
                .map_or(ProcOrigin::Other, |s| ProcOrigin::Spell(s.spell.clone()));
            self.hit_enemy(Some(i), source, hit, false, HitSource::Proc { spell, school: esurv_content::School::Nature, origin });
        }
        let h = &self.heroes[i];
        let thorns: f32 = h.modifiers.iter().filter(|m| m.stat == esurv_content::Stat::Thorns).map(|m| m.value).sum();
        if thorns > 0.0 && taken > 0.0 && self.enemies.iter().any(|e| e.id == source) {
            self.hit_enemy(Some(i), source, thorns * taken, false, HitSource::Proc { spell: THORNS_VISUAL, school: esurv_content::School::Nature, origin: ProcOrigin::Other });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::{BossAbilityKind, Content, Milestone, School, SpellFilter, Stat, Trigger};

    use super::*;
    use crate::game::casting::HitSource;
    use crate::game::enemy::RunOptions;
    use crate::game::run::{Flat, HeroSetup, World};
    use crate::game::stats::Modifier;

    struct Missiles;
    impl World for Missiles {
        fn height(&self, _x: f32, _y: f32) -> Option<f32> {
            Some(0.0)
        }
        fn missile_speed(&self, _spell: u32) -> f32 {
            20.0
        }
    }

    fn run(world: Box<dyn World>) -> Run {
        run_with(world, vec![])
    }

    fn run_with(world: Box<dyn World>, milestones: Vec<Milestone>) -> Run {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Arc::new(Content::load(dir).expect("content loads"));
        let hero = HeroSetup { class: "warrior".into(), race: "orc".into(), milestones, ..Default::default() };
        let mut r = Run::new(content, "elwynn", RunOptions::default(), &[hero], world, 9).unwrap();
        let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
        r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        r.enemies.clear();
        r.pickups.clear();
        r.heroes[0].spells.known.clear();
        r.heroes[0].spells.racial = None;
        r.drain_events();
        r
    }

    #[test]
    fn a_ranged_caster_stops_casts_and_its_shot_flies() {
        let mut r = run(Box::new(Missiles));
        let zone = r.zone;
        let caster = r.content.zones[zone].enemies.iter().position(|e| e.ranged.is_some()).expect("Elwynn has a caster");
        let p = r.heroes[0].pos;
        assert!(r.spawn_roster_enemy(caster, Some((p.0 + 20.0, p.1))));
        let before = r.heroes[0].health;
        let mut hit_at = None;
        for _ in 0..200 {
            r.tick(0.05);
            r.take_first_offers();
            if r.drain_events().iter().any(|e| matches!(e, Event::HeroHit { .. })) {
                hit_at = Some(r.now);
                break;
            }
        }
        assert!(hit_at.is_some(), "the shot landed");
        assert!(r.heroes[0].health < before);
    }

    #[test]
    fn a_ground_burst_hits_only_a_hero_still_inside() {
        let mut r = run(Box::new(Flat));
        r.spawn_boss(0);
        let k = r.enemies.len() - 1;
        let burst = BossAbility {
            kind: BossAbilityKind::GroundBurst { telegraph_spell: 10, impact_spell: 122, radius: 5.0, delay: 1.0, damage_mult: 2.0 },
            cooldown: 6.0,
        };
        r.enemies[k].abilities = vec![AbilitySlot { ability: burst, timer: 0.0 }];
        r.enemy_abilities(k, 0, 0.01, false);
        assert_eq!(r.ground_hits.len(), 1);
        // the hero's spot is marked (with the hero as its caster) and nothing is cast yet
        let id = r.heroes[0].id;
        let boss = r.enemies[k].id;
        let cast = r.drain_events();
        let mark = cast.iter().find_map(|e| match e {
            Event::AreaShown { id: mark, caster: Unit::Hero(h), spell: 10, radius, until: None, .. } if *h == id && *radius == 5.0 => Some(*mark),
            _ => None,
        });
        let mark = mark.expect("the spot is marked");
        assert!(!cast.iter().any(|e| matches!(e, Event::SpellGo { .. })));
        // walk out of the circle
        let p = r.heroes[0].pos;
        r.hero_moved(id, (p.0 + 10.0, p.1), 0.0, 0.0);
        let health = r.heroes[0].health;
        r.now += 1.1;
        r.enemy_hits_tick();
        assert_eq!(r.heroes[0].health, health, "out of the circle: no damage");
        let landed = r.drain_events();
        assert!(landed.iter().any(|e| matches!(e, Event::SpellGo { caster: Unit::Enemy(b), spell: 122, aim: Aim::Ground(at), .. } if *b == boss && *at == p)), "the impact plays on the spot");
        assert!(landed.iter().any(|e| matches!(e, Event::AreaGone { id } if *id == mark)), "and the mark goes");
    }

    #[test]
    fn a_ground_hit_on_the_hero_comes_from_no_one() {
        let mut r = run(Box::new(Flat));
        r.spawn_boss(0);
        let k = r.enemies.len() - 1;
        let burst = BossAbility {
            kind: BossAbilityKind::GroundBurst { telegraph_spell: 10, impact_spell: 0, radius: 5.0, delay: 1.0, damage_mult: 2.0 },
            cooldown: 6.0,
        };
        r.enemies[k].abilities = vec![AbilitySlot { ability: burst, timer: 0.0 }];
        r.enemy_abilities(k, 0, 0.01, false);
        r.drain_events();
        r.now += 1.1;
        r.enemy_hits_tick();
        let landed = r.drain_events();
        assert!(landed.iter().any(|e| matches!(e, Event::HeroHit { by: None, melee: false, .. })));
        assert!(!landed.iter().any(|e| matches!(e, Event::SpellGo { .. })), "no impact look when it has none");
    }

    #[test]
    fn a_volley_shows_every_shot_as_it_leaves() {
        let mut r = run(Box::new(Missiles));
        r.spawn_boss(0);
        let k = r.enemies.len() - 1;
        let boss = r.enemies[k].id;
        let volley = BossAbility { kind: BossAbilityKind::Volley { look_spell: 1510, count: 6, damage_mult: 0.5, impact_on_bunny: false }, cooldown: 6.0 };
        r.enemies[k].abilities = vec![AbilitySlot { ability: volley, timer: 0.0 }];
        r.enemy_abilities(k, 0, 0.01, false);
        let shots: Vec<Vec2> = r
            .drain_events()
            .iter()
            .filter_map(|e| match e {
                Event::SpellGo { caster: Unit::Enemy(b), spell: 1510, hits, aim: Aim::Ground(at), .. } if *b == boss && hits.is_empty() => Some(*at),
                _ => None,
            })
            .collect();
        assert_eq!(shots.len(), r.ground_hits.len());
        assert!(shots.len() > 1);
        r.now += 100.0;
        r.enemy_hits_tick();
        assert!(!r.drain_events().iter().any(|e| matches!(e, Event::SpellGo { .. })), "nothing shows where they land");
    }

    #[test]
    fn a_stomp_lists_the_hero_only_in_reach_and_a_charge_is_aimed_at_the_hero() {
        let mut r = run(Box::new(Flat));
        r.spawn_boss(0);
        let k = r.enemies.len() - 1;
        let (boss, hero) = (r.enemies[k].id, r.heroes[0].id);
        for (radius, listed) in [(50.0, vec![Unit::Hero(hero)]), (0.1, vec![])] {
            let stomp = BossAbility { kind: BossAbilityKind::Stomp { look_spell: 6343, radius, damage_mult: 1.0 }, cooldown: 30.0 };
            r.enemies[k].abilities = vec![AbilitySlot { ability: stomp, timer: 0.0 }];
            r.now += ABILITY_GAP;
            r.enemy_abilities(k, 0, 0.01, false);
            let events = r.drain_events();
            assert!(events.iter().any(|e| matches!(e, Event::SpellGo { caster: Unit::Enemy(b), spell: 6343, hits, aim: Aim::Caster, reach, .. } if *b == boss && *hits == listed && *reach == radius)), "{radius}");
        }
        let charge = BossAbility { kind: BossAbilityKind::Charge { look_spell: 100, damage_mult: 2.0 }, cooldown: 30.0 };
        r.enemies[k].abilities = vec![AbilitySlot { ability: charge, timer: 0.0 }];
        r.now += ABILITY_GAP;
        r.enemy_abilities(k, 0, 0.01, false);
        let target = Unit::Hero(hero);
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::SpellGo { caster: Unit::Enemy(b), spell: 100, hits, aim: Aim::Unit(t), .. } if *b == boss && *hits == vec![target] && *t == target)));
        r.now += 100.0;
        r.enemy_hits_tick();
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::SpellGo { caster: Unit::Enemy(b), spell: 6343, aim: Aim::Ground(_), .. } if *b == boss)), "its landing look");
    }

    #[test]
    fn abilities_due_together_go_off_one_gap_apart() {
        let mut r = run(Box::new(Flat));
        r.spawn_boss(0);
        let k = r.enemies.len() - 1;
        let stomp = BossAbility { kind: BossAbilityKind::Stomp { look_spell: 6343, radius: 50.0, damage_mult: 1.0 }, cooldown: 30.0 };
        let summon = BossAbility { kind: BossAbilityKind::Summon { count: 1 }, cooldown: 30.0 };
        r.enemies[k].abilities = vec![AbilitySlot { ability: stomp, timer: 0.0 }, AbilitySlot { ability: summon, timer: 0.0 }];
        r.enemy_abilities(k, 0, 0.01, false);
        let first: Vec<Event> = r.drain_events();
        assert!(first.iter().any(|e| matches!(e, Event::SpellGo { spell: 6343, .. })));
        assert!(!first.iter().any(|e| matches!(e, Event::Yell { text, .. } if text == crate::game::lines::SUMMON_YELL)), "not both at once");
        r.now += ABILITY_GAP;
        r.enemy_abilities(k, 0, 0.0, false);
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::Yell { text, .. } if text == crate::game::lines::SUMMON_YELL)), "the second after the gap");
    }

    #[test]
    fn a_boss_whose_stomp_kills_an_enemy_ahead_of_it_goes_on_with_its_turn() {
        // the stomp breaks the hero's shield; the shield's burst kills an enemy earlier in the list,
        // so the boss's place in the list moves while it acts
        let burst = Milestone::ShieldBurst {
            school: Some(School::Holy),
            share: 1.0,
            radius: 50.0,
            look_spell: None,
            trigger: Trigger::ShieldBreak,
            filter: SpellFilter::Any,
        };
        let mut r = run_with(Box::new(Flat), vec![burst]);
        r.spawn_boss(0);
        r.spawn_boss(0);
        let (weak, k) = (0, r.enemies.len() - 1);
        r.enemies[weak].abilities.clear();
        r.enemies[weak].health = 1.0;
        let stomp = BossAbility { kind: BossAbilityKind::Stomp { look_spell: 6343, radius: 50.0, damage_mult: 1.0 }, cooldown: 30.0 };
        r.enemies[k].abilities = vec![AbilitySlot { ability: stomp, timer: 0.0 }];
        (r.enemies[k].hold_until, r.enemies[k].next_ability_at) = (0.0, 0.0);
        r.heroes[0].shield = 1.0;
        let (weak_id, boss) = (r.enemies[weak].id, r.enemies[k].id);
        r.enemies_tick(0.01);
        assert!(!r.enemies.iter().any(|e| e.id == weak_id), "the burst killed the enemy ahead");
        assert!(r.enemies.iter().any(|e| e.id == boss), "and the boss is still there");
    }

    #[test]
    fn a_held_boss_keeps_counting_and_casts_when_freed() {
        let mut r = run(Box::new(Flat));
        r.spawn_boss(0);
        let k = r.enemies.len() - 1;
        let stomp = BossAbility { kind: BossAbilityKind::Stomp { look_spell: 6343, radius: 50.0, damage_mult: 1.0 }, cooldown: 2.0 };
        r.enemies[k].abilities = vec![AbilitySlot { ability: stomp, timer: 1.0 }];
        r.enemy_abilities(k, 0, 1.5, true);
        assert!(r.enemies[k].abilities[0].timer < 0.0, "it kept counting");
        assert!(!r.drain_events().iter().any(|e| matches!(e, Event::SpellGo { .. })), "but waited");
        r.enemy_abilities(k, 0, 0.0, false);
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::SpellGo { spell: 6343, .. })), "and cast when freed");
    }

    #[test]
    fn thorns_strike_back_at_melee_attackers() {
        let mut r = run(Box::new(Flat));
        let p = r.heroes[0].pos;
        assert!(r.spawn_roster_enemy(0, Some((p.0 + 1.0, p.1))));
        let id = r.enemies[0].id;
        r.enemies[0].health = 1e6;
        r.heroes[0].modifiers.push(Modifier::new(Stat::Thorns, 0.5));
        r.drain_events();
        r.damage_hero(0, 100.0, Some(id), true);
        let taken = 100.0 * (1.0 - r.heroes[0].stats.damage_reduction);
        assert!((r.enemies[0].health - (1e6 - taken * 0.5)).abs() < 1.0);
        let thorns = HitSource::Proc { spell: THORNS_VISUAL, school: esurv_content::School::Nature, origin: ProcOrigin::Other };
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::EnemyHit { source, .. } if *source == thorns)), "shown as thorns' damage");
        // ranged or ground hits do not trigger it
        r.damage_hero(0, 100.0, Some(id), false);
        assert!((r.enemies[0].health - (1e6 - taken * 0.5)).abs() < 1.0);
    }

    #[test]
    fn a_swing_shows_what_it_was_worth_before_the_shield() {
        let mut r = run(Box::new(Flat));
        let p = r.heroes[0].pos;
        assert!(r.spawn_roster_enemy(0, Some((p.0 + 1.0, p.1))));
        let id = r.enemies[0].id;
        r.heroes[0].shield = 30.0;
        let health = r.heroes[0].health;
        r.drain_events();
        r.damage_hero(0, 100.0, Some(id), true);
        let worth = 100.0 * (1.0 - r.heroes[0].stats.damage_reduction);
        assert!((health - r.heroes[0].health - (worth - 30.0)).abs() < 1e-3, "the shield took its share");
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::HeroHit { amount, melee: true, .. } if (*amount - worth).abs() < 1e-3)));
    }
    /// A boss never stuns, fears, roots or slows a hero. Every boss
    /// ability of every zone (a Warsong boss's its class's, the rares' too), cast on a hero
    /// in reach and landed, takes health and nothing else: the hero runs as fast, keeps its
    /// buffs and modifiers, and stands where it stood.
    #[test]
    fn no_boss_ability_stuns_fears_roots_or_slows_a_hero() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Arc::new(Content::load(dir).expect("content loads"));
        let hero = HeroSetup { class: "warrior".into(), race: "orc".into(), ..Default::default() };
        let mut cast = 0;
        for zone in &content.zones {
            let mut r = Run::new(content.clone(), &zone.id, RunOptions::default(), &[hero.clone()], Box::new(Missiles), 9).unwrap();
            r.enemies.clear();
            r.drain_events();
            let z = r.zone;
            let faction = r.party_faction.other();
            for b in 0..r.content.zones[z].bosses.len() {
                let mut abilities = r.content.boss_abilities(&r.content.zones[z], b, faction, r.seed);
                abilities.extend(crate::game::boss_abilities::rare_abilities());
                for ability in abilities.into_iter().filter(|a| !matches!(a.kind, BossAbilityKind::Summon { .. })) {
                    r.enemies.clear();
                    r.ground_hits.clear();
                    r.spawn_boss(b);
                    let k = r.enemies.len() - 1;
                    let at = r.heroes[0].pos;
                    r.enemies[k].segment = crate::game::enemy_ai::Segment::standing((at.0 + 3.0, at.1), r.now);
                    r.enemies[k].abilities = vec![AbilitySlot { ability: ability.clone(), timer: 0.0 }];
                    r.enemies[k].next_ability_at = 0.0;
                    r.enemies[k].hold_until = 0.0;
                    let hero = &r.heroes[0];
                    let before = (hero.stats.run_speed(), format!("{:?}", hero.buffs), format!("{:?}", hero.modifiers), hero.pos, hero.health);
                    r.enemy_abilities(k, 0, 0.01, false);
                    r.now += 5.0;
                    r.enemy_hits_tick();
                    let hero = &r.heroes[0];
                    let after = (hero.stats.run_speed(), format!("{:?}", hero.buffs), format!("{:?}", hero.modifiers), hero.pos, hero.health);
                    assert_eq!((&before.0, &before.1, &before.2, before.3), (&after.0, &after.1, &after.2, after.3), "zone {} boss {b}: {ability:?}", zone.id);
                    assert!(after.4 <= before.4, "zone {} boss {b}: {ability:?}", zone.id);
                    r.heroes[0].health = r.heroes[0].stats.max_health;
                    r.drain_events();
                    cast += 1;
                }
            }
        }
        assert!(cast > 100, "{cast}");
    }
}
