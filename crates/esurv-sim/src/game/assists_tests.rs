//! The assists' rules (`game::assists`): the standards' schedule, the summon pool, each block of
//! a script, and every character's assist run to its end.

use std::sync::Arc;

use esurv_content::{AreaAt, AssistScript, AssistStep, Content, Damage, EnemyType, Faction, Movement, OverTime, SpellEffect, Stat, Targets};

use super::assists::{AllyMove, ALLY_BESIDE_ENEMY, ALLY_RUN_SPEED};
use super::casting::{HitSource, HOLD_CHAIN};
use super::director::FIRST_ELITE;
use super::enemy::{health_and_damage, RunOptions, RUN_FACTOR};
use super::enemy_ai::{Segment, Vec2};
use super::loot::{Pickup, CACHE_RING};
use super::run::{Buff, BuffKind, EnemyKind, Event, Flat, HeroSetup, Run, Unit};

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn content() -> Content {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
    Content::load(dir).expect("content loads")
}

/// A warrior of `race` in `zone`, its run started, with no enemies or pickups about.
fn run_in(content: Arc<Content>, zone: &str, race: &str, seed: u64) -> Run {
    let setup = HeroSetup { class: "warrior".into(), race: race.into(), ..Default::default() };
    let mut r = Run::new(content, zone, RunOptions::default(), &[setup], Box::new(Flat), seed).unwrap();
    let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
    r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
    r.enemies.clear();
    r.pickups.clear();
    r.drain_events();
    r
}

fn immune(r: &mut Run) {
    r.heroes[0].buffs.push(Buff { kind: BuffKind::Immunity, value: 0.0, visual: 0, until: 1e9 });
}

/// A human hero's run whose first Alliance character assists with `script`, and that
/// character's id. Nothing hurts the hero.
fn run_with(script: AssistScript) -> (Run, String) {
    let mut c = content();
    let k = c.player_characters.iter().position(|p| c.character_faction(p) == Some(Faction::Alliance)).unwrap();
    c.player_characters[k].assist_script = Some(script);
    let id = c.player_characters[k].id.clone();
    let mut r = run_in(Arc::new(c), "elwynn", "human", 5);
    immune(&mut r);
    (r, id)
}

fn script(steps: Vec<AssistStep>) -> AssistScript {
    AssistScript { arrive: Default::default(), steps, leave: Default::default(), seconds: None }
}

fn hit(targets: Targets, damage: Option<Damage>) -> AssistStep {
    AssistStep::Hit { targets, damage, look_spell: None, melee: false, effects: vec![], wears: vec![], over_time: None }
}

/// A roster enemy `d` yd east of the hero; its id.
fn enemy_at(r: &mut Run, d: f32) -> u32 {
    let p = r.heroes[0].pos;
    assert!(r.spawn_roster_enemy(0, Some((p.0 + d, p.1))));
    r.enemies.last().unwrap().id
}

/// The zone's first boss, 4 yd from the hero and past its arrival hold; its id.
fn boss(r: &mut Run) -> u32 {
    r.spawn_boss(0);
    let k = r.enemies.iter().position(|e| e.is_boss()).unwrap();
    let p = r.heroes[0].pos;
    r.enemies[k].segment = Segment::standing((p.0 + 4.0, p.1), r.now);
    r.enemies[k].hold_until = 0.0;
    r.enemies[k].hold_immune_until = 0.0;
    r.enemies[k].id
}

fn assist_hits(events: &[Event], on: u32) -> Vec<f32> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::EnemyHit { id, amount, source: HitSource::Assist(_), .. } if *id == on => Some(*amount),
            _ => None,
        })
        .collect()
}

/// As `tick_for`, with only enemy `keep` on the field (nothing else spawns).
fn tick_alone(r: &mut Run, keep: u32, seconds: f32) -> Vec<Event> {
    let mut all = Vec::new();
    let end = r.now + seconds;
    while r.now < end - 1e-4 {
        r.enemies.retain(|e| e.id == keep);
        all.extend(tick_for(r, 0.05));
    }
    all
}

fn tick_for(r: &mut Run, seconds: f32) -> Vec<Event> {
    let mut all = Vec::new();
    let end = r.now + seconds;
    while r.now < end - 1e-4 {
        // a choice screen takes its first offer
        while let Some(screen) = &r.screen {
            let id = r.heroes[screen.hero].id;
            r.choose(id, 0);
        }
        if r.phase != super::run::Phase::Running {
            break;
        }
        r.tick(0.05);
        all.extend(r.drain_events());
    }
    all
}

#[test]
fn a_run_lays_one_or_two_standards_between_the_first_elite_and_the_final_boss() {
    let c = Arc::new(content());
    let zone = c.zones.iter().find(|z| z.id == "elwynn").unwrap();
    let end = zone.bosses.iter().find(|b| b.is_final).unwrap().spawn_minute * 60.0;
    let mut counts = [0; 3];
    for seed in 0..60 {
        let r = run_in(c.clone(), "elwynn", "human", seed);
        let due = r.assists.standards_due.clone();
        assert!((1..=2).contains(&due.len()), "{due:?}");
        counts[due.len()] += 1;
        assert!(due.iter().all(|&t| (FIRST_ELITE..=end).contains(&t)), "{due:?}");
        assert!(due.windows(2).all(|w| w[0] <= w[1]));
        // the same seed, the same standards
        assert_eq!(due, run_in(c.clone(), "elwynn", "human", seed).assists.standards_due);
    }
    assert!(counts[1] > 0 && counts[2] > 0, "both counts come: {counts:?}");
}

#[test]
fn warsong_gulch_never_lays_a_standard() {
    let c = Arc::new(content());
    for seed in 0..10 {
        let mut r = run_in(c.clone(), "wsg", "human", seed);
        assert!(r.assists.standards_due.is_empty());
        r.endless_since = Some(0.0);
        r.now = 1300.0;
        r.standards_tick();
        assert!(!r.pickups.iter().any(|p| matches!(p.kind, Pickup::BattleStandard(_))));
    }
}

#[test]
fn a_standard_lies_on_the_caches_ring_when_due_and_one_more_comes_every_ten_minutes_in_endless() {
    let mut r = run_in(Arc::new(content()), "elwynn", "human", 7);
    let first = r.assists.standards_due[0];
    r.now = first - 0.01;
    r.standards_tick();
    assert!(r.pickups.is_empty());
    r.now = first;
    r.standards_tick();
    let laid: Vec<Vec2> = r.pickups.iter().filter(|p| p.kind == Pickup::BattleStandard(Faction::Alliance)).map(|p| p.at).collect();
    assert_eq!(laid.len(), 1);
    assert!(r.drain_events().contains(&Event::Announce("A Warband Standard was raised somewhere. Follow the beam of light.".into())), "announced");
    let d = dist(laid[0], r.heroes[0].pos);
    assert!((CACHE_RING.0 - 0.01..=CACHE_RING.1 + 0.01).contains(&d), "{d}");
    r.pickups.clear();
    r.assists.standards_due.clear();
    r.endless_since = Some(1000.0);
    for t in [1000.0, 1599.0] {
        r.now = t;
        r.standards_tick();
    }
    assert!(r.pickups.is_empty());
    for (t, laid) in [(1600.0, 1), (2199.0, 1), (2200.0, 2)] {
        r.now = t;
        r.standards_tick();
        assert_eq!(r.pickups.len(), laid, "at {t}");
    }
}

#[test]
fn walking_over_a_standard_summons_an_ally_of_the_heros_faction() {
    for (race, faction) in [("human", Faction::Alliance), ("orc", Faction::Horde)] {
        let c = Arc::new(content());
        let mut r = run_in(c.clone(), "elwynn", race, 3);
        let p = r.heroes[0].pos;
        r.drop(Pickup::BattleStandard(r.party_faction), 0, (p.0 + 1.0, p.1));
        r.pickups_tick();
        let ev = r.drain_events();
        assert!(r.pickups.is_empty());
        let Some(Event::AllySummoned { character, scale, .. }) = ev.iter().find(|e| matches!(e, Event::AllySummoned { .. })) else { panic!("no ally: {ev:?}") };
        let ch = c.player_characters.iter().find(|x| x.id == *character).unwrap();
        assert_eq!(c.character_faction(ch), Some(faction));
        assert_eq!(*scale, 2.0);
        assert_eq!(r.assists.summoned, vec![character.clone()]);
        assert!(ev.contains(&Event::Announce(format!("{} answers your call!", ch.name))), "{} answers", ch.name);
    }
}

#[test]
fn no_character_comes_twice_while_another_of_the_faction_has_not_come() {
    let c = Arc::new(content());
    let mut r = run_in(c.clone(), "elwynn", "human", 11);
    let pool: Vec<String> = c.player_characters_of(Faction::Alliance).map(|x| x.id.clone()).collect();
    assert!(pool.len() > 2);
    let mut seen = Vec::new();
    for _ in 0..pool.len() {
        let id = r.pick_ally_character(0).unwrap();
        assert!(!seen.contains(&id), "{id} came twice");
        assert!(pool.contains(&id));
        seen.push(id.clone());
        r.assists.summoned.push(id);
    }
    // then all of them again
    assert!(pool.contains(&r.pick_ally_character(0).unwrap()));
}

#[test]
fn a_share_of_max_health_hit_on_a_boss_is_capped_at_a_normal_enemys() {
    let (mut r, id) = run_with(script(vec![
        hit(Targets::Nearest(1), Some(Damage::MaxHealthShare(0.06))),
        hit(Targets::Strongest, Some(Damage::MaxHealthShare(0.06))),
    ]));
    // the hero's bonus against elites and bosses does not touch an ally's hit
    r.heroes[0].modifiers.push(super::stats::Modifier::new(Stat::DamageVsElites, 1.0));
    r.now = 300.0;
    let normal = enemy_at(&mut r, 3.0);
    let b = boss(&mut r);
    let (normal_max, boss_max) = (r.enemy(normal).unwrap().max_health, r.enemy(b).unwrap().max_health);
    assert!(r.summon_character(0, &id));
    let ev = r.drain_events();
    let on_normal = assist_hits(&ev, normal);
    assert_eq!(on_normal.len(), 1);
    assert!((on_normal[0] - 0.06 * normal_max).abs() < 1e-3, "{on_normal:?}");
    let (cap, _) = health_and_damage(&r.content.balance, EnemyType::Normal, r.zone(), 5.0, 0.0, &r.options, RUN_FACTOR);
    let on_boss = assist_hits(&ev, b);
    assert_eq!(on_boss.len(), 1);
    assert!(boss_max > cap * 2.0, "the boss is tougher");
    assert!((on_boss[0] - 0.06 * cap).abs() < 1e-3, "{on_boss:?} vs {}", 0.06 * cap);
}

#[test]
fn crowd_control_goes_through_the_heros_rules_a_boss_held_a_quarter_and_never_feared() {
    let stun = AssistStep::Hit { targets: Targets::OnScreen, damage: None, look_spell: None, melee: false, effects: vec![SpellEffect::Stun { seconds: 2.0 }], wears: vec![1833], over_time: None };
    let fear = AssistStep::Hit { targets: Targets::Strongest, damage: None, look_spell: None, melee: false, effects: vec![SpellEffect::Fear { seconds: 8.0 }], wears: vec![5782], over_time: None };
    let (mut r, id) = run_with(script(vec![stun, fear]));
    let normal = enemy_at(&mut r, 3.0);
    let b = boss(&mut r);
    let now = r.now;
    assert!(r.summon_character(0, &id));
    // the boss for a quarter (0.5 s), the other for its 2 s
    assert!((r.enemy(b).unwrap().hold_until - (now + 0.5)).abs() < 1e-4);
    assert!((r.enemy(normal).unwrap().hold_until - (now + 2.0)).abs() < 1e-4);
    // no fear on a boss, and its mark is not worn
    assert_eq!(r.enemy(b).unwrap().fear_until, 0.0);
    let ev = r.drain_events();
    let worn: Vec<(Unit, u32, f32)> = ev.iter().filter_map(|e| if let Event::AuraShown { target, spell, until, .. } = e { Some((*target, *spell, *until)) } else { None }).collect();
    assert!(worn.contains(&(Unit::Enemy(b), 1833, now + 0.5)), "{worn:?}");
    assert!(worn.contains(&(Unit::Enemy(normal), 1833, now + 2.0)));
    assert!(!worn.iter().any(|w| w.1 == 5782));
}

#[test]
fn a_normal_enemy_is_held_the_full_time_and_a_boss_by_the_heros_rules() {
    let stun = AssistStep::Hit { targets: Targets::OnScreen, damage: None, look_spell: None, melee: false, effects: vec![SpellEffect::Stun { seconds: 8.0 }], wears: vec![], over_time: None };
    let (mut r, id) = run_with(script(vec![stun]));
    let e = enemy_at(&mut r, 3.0);
    let b = boss(&mut r);
    let now = r.now;
    r.summon_character(0, &id);
    // past the hero's 3 s chain on a normal enemy; a quarter on the boss, within the chain
    assert!(8.0 > HOLD_CHAIN);
    assert!((r.enemy(e).unwrap().hold_until - (now + 8.0)).abs() < 1e-4);
    assert!((r.enemy(b).unwrap().hold_until - (now + 2.0)).abs() < 1e-4);
    // a second stun on the boss as its hold ends: the hero's immunity
    r.now = now + 2.0;
    r.effects_tick();
    let held = r.enemy(b).unwrap().hold_until;
    r.assist_effect(0, b, &SpellEffect::Stun { seconds: 8.0 }, 0.0);
    assert_eq!(r.enemy(b).unwrap().hold_until, held);
}

#[test]
fn taunted_enemies_go_for_the_ally_and_it_stays_while_the_taunt_holds() {
    let (mut r, id) = run_with(script(vec![AssistStep::Taunt { targets: Targets::OnScreen, seconds: 4.0, look_spell: Some(355) }]));
    let e = enemy_at(&mut r, 8.0);
    let b = boss(&mut r);
    let now = r.now;
    r.summon_character(0, &id);
    let a = r.assists.allies[0].clone();
    assert!(a.taunts.contains(&(e, now + 4.0)));
    assert!(a.taunts.contains(&(b, now + 1.0)), "a boss for a quarter: {:?}", a.taunts);
    let ev = r.drain_events();
    assert!(ev.contains(&Event::AllyTaunting { id: a.id, taunting: true }));
    let ev = tick_for(&mut r, 3.0);
    assert!(ev.iter().any(|x| matches!(x, Event::AllyStruck { id, by } if *id == a.id && *by == e)), "the enemy strikes the ally");
    assert!(!ev.iter().any(|x| matches!(x, Event::HeroHit { by: Some(by), .. } if *by == e)), "not the hero");
    assert_eq!(r.assists.allies.len(), 1, "it stays while its taunt holds");
    let ev = tick_for(&mut r, 1.2);
    assert!(ev.contains(&Event::AllyTaunting { id: a.id, taunting: false }), "the taunt ends");
    assert_eq!(r.assists.allies.len(), 1, "it stays its least 5 s");
    let ev = tick_for(&mut r, 1.0);
    assert!(r.assists.allies.is_empty(), "then goes");
    assert!(ev.iter().any(|x| matches!(x, Event::AllyLeft { id, .. } if *id == a.id)));
}

#[test]
fn a_repeat_strikes_on_its_beat_and_the_ally_leaves_after_its_least_stay() {
    let strike = AssistStep::Hit { targets: Targets::Nearest(1), damage: Some(Damage::MaxHealthShare(0.03)), look_spell: Some(1680), melee: true, effects: vec![], wears: vec![], over_time: None };
    let (mut r, id) = run_with(script(vec![AssistStep::Repeat { every: 1.0, times: 3, steps: vec![strike] }]));
    let e = enemy_at(&mut r, 3.0);
    r.summon_character(0, &id);
    let mut ev = r.drain_events();
    assert_eq!(assist_hits(&ev, e).len(), 1, "the first at once");
    assert!(ev.iter().any(|x| matches!(x, Event::AllySwing { target, .. } if *target == e)));
    assert!(ev.iter().any(|x| matches!(x, Event::AllyMoved { how: AllyMove::Step, .. })));
    ev.extend(tick_for(&mut r, 2.5));
    assert_eq!(assist_hits(&ev, e).len(), 3);
    assert_eq!(assist_hits(&tick_for(&mut r, 2.4), e).len(), 0, "the repeat took 3 s");
    assert_eq!(r.assists.allies.len(), 1, "it stays its least 5 s");
    tick_for(&mut r, 0.2);
    assert!(r.assists.allies.is_empty());
}

#[test]
fn the_ally_leaves_at_its_time_whatever_is_left() {
    let strike = AssistStep::Hit { targets: Targets::Nearest(1), damage: None, look_spell: Some(1680), melee: false, effects: vec![], wears: vec![], over_time: None };
    let mut s = script(vec![AssistStep::Repeat { every: 1.0, times: 8, steps: vec![strike] }]);
    s.seconds = Some(3.0);
    let (mut r, id) = run_with(s);
    enemy_at(&mut r, 3.0);
    r.summon_character(0, &id);
    tick_for(&mut r, 2.9);
    assert_eq!(r.assists.allies.len(), 1);
    tick_for(&mut r, 0.2);
    assert!(r.assists.allies.is_empty());
}

#[test]
fn a_ground_area_strikes_at_once_then_every_second_for_its_time() {
    let area = AssistStep::Area { at: AreaAt::Hero, radius: 9.0, seconds: 4.0, every: 1.0, damage: Some(Damage::MaxHealthShare(0.03)), effects: vec![], look_spell: 10 };
    let (mut r, id) = run_with(script(vec![area]));
    let e = enemy_at(&mut r, 3.0);
    r.summon_character(0, &id);
    let mut ev = r.drain_events();
    assert!(ev.iter().any(|x| matches!(x, Event::AreaShown { caster: Unit::Ally(_), spell: 10, .. })));
    assert_eq!(assist_hits(&ev, e).len(), 1);
    ev.extend(tick_for(&mut r, 6.0));
    assert_eq!(assist_hits(&ev, e).len(), 4);
}

#[test]
fn a_bleed_ticks_for_its_time() {
    let o = OverTime { damage: Damage::MaxHealthShare(0.02), every: 1.0, seconds: 3.0, look_spell: Some(703) };
    let bleed = AssistStep::Hit { targets: Targets::Nearest(1), damage: None, look_spell: None, melee: false, effects: vec![], wears: vec![], over_time: Some(o) };
    let (mut r, id) = run_with(script(vec![bleed]));
    let e = enemy_at(&mut r, 3.0);
    r.summon_character(0, &id);
    let ev = tick_for(&mut r, 5.0);
    assert_eq!(assist_hits(&ev, e).len(), 3);
}

#[test]
fn buffs_heals_totems_and_immunity_on_the_hero() {
    let (mut r, id) = run_with(script(vec![
        AssistStep::Buff { stats: vec![(Stat::Damage, 0.15)], seconds: 5.0, heal: 0.0, every: 0.0, immune: false, look_spell: Some(6673), totem: Some(8512) },
        AssistStep::Buff { stats: vec![], seconds: 6.0, heal: 0.1, every: 2.0, immune: true, look_spell: Some(1022), totem: None },
        AssistStep::Heal { share: 0.3, look_spell: Some(2061) },
    ]));
    r.heroes[0].buffs.clear();
    let damage = r.heroes[0].stats.damage;
    let max = r.heroes[0].stats.max_health;
    r.heroes[0].health = 1.0;
    r.summon_character(0, &id);
    assert!((r.heroes[0].stats.damage - damage - 0.15).abs() < 1e-5);
    assert!((r.heroes[0].health - (1.0 + 0.3 * max)).abs() < 1e-3);
    assert!(r.heroes[0].buffs.iter().any(|b| b.kind == BuffKind::Immunity));
    assert!(r.totems.iter().any(|t| t.spell == "assist_totem:8512"));
    let ev = r.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::AllyDropped { unit: Unit::Totem(_), .. })));
    assert!(ev.iter().any(|e| matches!(e, Event::AuraShown { target: Unit::Hero(_), spell: 1022, .. })));
    let before = r.heroes[0].health;
    let ev = tick_for(&mut r, 2.05);
    let healed: f32 = ev.iter().filter_map(|e| if let Event::HeroHealed { amount, .. } = e { Some(*amount) } else { None }).sum();
    assert!((healed - 0.1 * max).abs() < 1e-2, "a tenth after 2 s: {healed}");
    assert!(r.heroes[0].health > before);
    tick_for(&mut r, 3.5);
    // (the warrior's own shout may be up by now)
    assert!(r.assist_modifiers(0).is_empty() && r.assists.buffs.iter().all(|b| b.stats.is_empty()), "the buff ended");
    assert!(r.totems.is_empty());
}

#[test]
fn summoned_pets_fight_for_the_hero_and_name_the_assist() {
    let (mut r, id) = run_with(script(vec![AssistStep::Summon { pet: "Gargoyle".into(), count: 2, seconds: 10.0, damage: None }]));
    r.summon_character(0, &id);
    let ours: Vec<_> = r.pets.iter().filter(|p| p.assist.as_deref() == Some(id.as_str())).collect();
    assert_eq!(ours.len(), 2);
    assert!(ours.iter().all(|p| p.until == Some(r.now + 10.0)));
    let ev = r.drain_events();
    assert_eq!(ev.iter().filter(|e| matches!(e, Event::AllyDropped { unit: Unit::Pet(_), .. })).count(), 2);
}

#[test]
fn a_move_runs_to_its_enemy_at_the_heros_pace() {
    let (mut r, id) = run_with(script(vec![AssistStep::Move { to: Targets::Farthest, how: Movement::Run, look_spell: None }, AssistStep::Wait { seconds: 1.0 }]));
    let e = enemy_at(&mut r, 16.0);
    r.enemies.iter_mut().find(|x| x.id == e).unwrap().hold_until = 100.0;
    let now = r.now;
    r.summon_character(0, &id);
    let ev = r.drain_events();
    let Some(Event::AllyMoved { to, until, how: AllyMove::Run, .. }) = ev.iter().find(|x| matches!(x, Event::AllyMoved { .. })) else { panic!("{ev:?}") };
    let from = r.assists.allies[0].segment.from;
    assert!((until - now - dist(from, *to) / ALLY_RUN_SPEED).abs() < 1e-4);
    assert!((dist(*to, r.enemy_pos(e).unwrap()) - ALLY_BESIDE_ENEMY).abs() < 1e-3);
}

/// Every character's assist and every class default runs to its end, and the ally is gone.
#[test]
fn every_assist_runs_to_its_end() {
    let base = content();
    let mut cases: Vec<(Content, String)> = base.player_characters.iter().map(|c| (base.clone(), c.id.clone())).collect();
    for d in &base.class_assists {
        let mut c = base.clone();
        let p = &mut c.player_characters[0];
        p.class = d.class.clone();
        p.form = d.form;
        p.assist_script = None;
        let id = p.id.clone();
        assert_eq!(c.assist_of(&c.player_characters[0]), Some(&d.script));
        cases.push((c, id));
    }
    for (c, id) in cases {
        let mut r = run_in(Arc::new(c), "elwynn", "human", 9);
        immune(&mut r);
        for d in [3.0, 5.0, 8.0, 12.0, 15.0] {
            enemy_at(&mut r, d);
        }
        // a boss that outlasts the test (its fall would end the run)
        let b = boss(&mut r);
        let k = r.enemies.iter().position(|e| e.id == b).unwrap();
        (r.enemies[k].health, r.enemies[k].max_health) = (1e9, 1e9);
        assert!(r.summon_character(0, &id), "{id} has an assist");
        let ev = tick_for(&mut r, 25.0);
        assert_eq!(r.phase, super::run::Phase::Running, "{id}");
        assert!(r.assists.allies.is_empty(), "{id} is gone");
        assert!(ev.iter().any(|e| matches!(e, Event::AllyLeft { .. })), "{id} left");
        assert!(r.enemies.iter().all(|e| e.health.is_finite() && (e.kind != EnemyKind::Boss(0) || e.health > 0.0)));
    }
}

#[test]
fn a_forced_character_comes_first_then_the_random_pick() {
    let c = Arc::new(content());
    let mut r = run_in(c.clone(), "elwynn", "human", 4);
    let horde = c.player_characters_of(Faction::Horde).next().unwrap().id.clone();
    r.assists.forced = vec!["nobody".into(), horde.clone()];
    r.summon_ally(0);
    assert_eq!(r.assists.summoned, vec![horde]);
    assert!(r.assists.forced.is_empty());
    r.summon_ally(0);
    assert_eq!(r.assists.summoned.len(), 2);
}

#[test]
fn an_allys_hit_is_its_own_no_lifesteal_or_milestone_but_the_kill_rewards_the_hero() {
    let (mut r, id) = run_with(script(vec![hit(Targets::Nearest(1), Some(Damage::Flat(1e6)))]));
    r.heroes[0].stats.lifesteal = 1.0;
    r.heroes[0].stats.heal_per_kill = 50.0;
    r.heroes[0].health = 10.0;
    let e = enemy_at(&mut r, 3.0);
    let (kills, dealt) = (r.heroes[0].kills, r.heroes[0].damage_dealt);
    r.summon_character(0, &id);
    let ev = r.drain_events();
    assert!(ev.iter().any(|x| matches!(x, Event::EnemyDied { id, by: Some(_) } if *id == e)));
    assert!(ev.iter().any(|x| matches!(x, Event::PickupDropped { kind: Pickup::Experience, .. })), "its experience drops");
    assert!(!ev.iter().any(|x| matches!(x, Event::HeroHealed { .. })), "no lifesteal, no heal per kill: {ev:?}");
    assert_eq!(r.heroes[0].kills, kills + 1);
    assert_eq!(r.heroes[0].damage_dealt, dealt);
}

#[test]
fn an_ally_of_instant_steps_stays_five_seconds() {
    let (mut r, id) = run_with(script(vec![AssistStep::Cast { look_spell: 871 }]));
    r.summon_character(0, &id);
    assert_eq!(r.assists.allies.len(), 1);
    tick_for(&mut r, 4.9);
    assert_eq!(r.assists.allies.len(), 1);
    tick_for(&mut r, 0.2);
    assert!(r.assists.allies.is_empty());
}

/// Stormwall taunts for 10 s: it stays that long whether the taunt holds a boss a quarter of it or
/// the enemies it held die.
#[test]
fn a_taunting_ally_stays_the_taunts_whole_time() {
    for with_boss in [true, false] {
        let mut r = run_in(Arc::new(content()), "elwynn", "human", 5);
        immune(&mut r);
        let e = if with_boss { boss(&mut r) } else { enemy_at(&mut r, 4.0) };
        let now = r.now;
        assert!(r.summon_character(0, "stormwall"));
        // it charges first, then taunts
        tick_for(&mut r, 1.0);
        assert!(r.assists.allies[0].taunts.iter().any(|t| t.0 == e), "it taunts: {:?}", r.assists.allies[0]);
        if !with_boss {
            r.enemies.retain(|x| x.id != e);
        }
        tick_for(&mut r, 8.0);
        assert_eq!(r.assists.allies.len(), 1, "still there 9 s in (boss: {with_boss})");
        tick_for(&mut r, 1.5);
        assert!(r.assists.allies.is_empty());
        assert!(r.now - now >= 10.0);
    }
}

/// The hits of pet `pet` on enemy `on`.
fn pet_hits(events: &[Event], pet: u32, on: u32) -> Vec<(f32, bool)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::EnemyHit { id, amount, crit, source: HitSource::Pet(p), .. } if *id == on && *p == pet => Some((*amount, *crit)),
            _ => None,
        })
        .collect()
}

/// Ghorktheorc: one Gargoyle hovering 2 yd up casts its bolt about every 1.5 s for 3 % of the
/// target's max health, he runs in and swings once a second for 3 %, and after 10 s both are gone.
#[test]
fn ghorktheorc_summons_a_hovering_gargoyle_and_swings_then_both_leave() {
    let c = Arc::new(content());
    let gargoyle = c.pets.iter().find(|p| p.name == "Gargoyle").unwrap().clone();
    assert_eq!(gargoyle.hover, Some(2.0), "it hovers 2 yd up");
    assert_eq!(gargoyle.ranged_spell, Some(5176));
    let mut r = run_in(c, "elwynn", "human", 5);
    immune(&mut r);
    let e = enemy_at(&mut r, 6.0);
    let k = r.enemies.iter().position(|x| x.id == e).unwrap();
    (r.enemies[k].max_health, r.enemies[k].health, r.enemies[k].base_speed) = (1000.0, 1e9, 0.0);
    r.enemies[k].hold_until = 100.0;
    let now = r.now;
    assert!(r.summon_character(0, "ghorktheorc"));
    let pets: Vec<u32> = r.pets.iter().filter(|p| p.assist.as_deref() == Some("ghorktheorc")).map(|p| p.id).collect();
    assert_eq!(pets.len(), 1, "one Gargoyle");
    let pet = pets[0];
    let mut ev = r.drain_events();
    assert!(ev.iter().any(|x| matches!(x, Event::AllyMoved { how: AllyMove::Run, .. })), "he runs in");
    ev.extend(tick_alone(&mut r, e, 10.5));
    let bolts = ev.iter().filter(|x| matches!(x, Event::SpellGo { caster: Unit::Pet(p), spell: 5176, .. } if *p == pet)).count();
    assert!((6..=8).contains(&bolts), "a bolt about every 1.5 s: {bolts}");
    let shots = pet_hits(&ev, pet, e);
    assert_eq!(shots.len(), bolts);
    assert!(shots.iter().all(|&(a, crit)| (a - 30.0).abs() < 1e-3 && !crit), "3 % of 1000 each: {shots:?}");
    let swings = ev.iter().filter(|x| matches!(x, Event::AllySwing { target, .. } if *target == e)).count();
    let hits = assist_hits(&ev, e);
    assert!((8..=10).contains(&swings), "a swing a second: {swings}");
    assert_eq!(hits.len(), swings);
    assert!(hits.iter().all(|&a| (a - 30.0).abs() < 1e-3), "3 % of 1000 each: {hits:?}");
    assert!(r.assists.allies.is_empty(), "he left at 10 s");
    assert!(!r.pets.iter().any(|p| p.id == pet), "the Gargoyle left with him");
    assert!(r.now - now < 10.6);
}

/// An assist's pet deals the assist's damage: not raised by the hero's damage, pet damage or
/// level, never a crit, no lifesteal or heal per kill; its kill is still the hero's.
#[test]
fn an_assists_pets_neither_scale_with_the_hero_nor_set_anything_off() {
    let (mut r, id) = run_with(script(vec![AssistStep::Summon { pet: "Gargoyle".into(), count: 1, seconds: 10.0, damage: None }]));
    let own = r.content.pets.iter().find(|p| p.name == "Gargoyle").unwrap().damage;
    let e = enemy_at(&mut r, 12.0);
    r.heroes[0].spells.known.clear();
    r.heroes[0].spells.racial = None;
    let k = r.enemies.iter().position(|x| x.id == e).unwrap();
    (r.enemies[k].health, r.enemies[k].base_speed) = (1e9, 0.0);
    r.summon_character(0, &id);
    let pet = r.pets.iter().find(|p| p.assist.as_deref() == Some(id.as_str())).unwrap().id;
    let hero = &mut r.heroes[0];
    hero.stats.damage = 5.0;
    hero.stats.crit_chance = 1.0;
    hero.stats.lifesteal = 1.0;
    hero.stats.heal_per_kill = 50.0;
    hero.progress.level = 20;
    hero.modifiers.push(super::stats::Modifier::new(Stat::PetDamage, 2.0));
    let ev = tick_alone(&mut r, e, 3.2);
    let shots = pet_hits(&ev, pet, e);
    assert!(!shots.is_empty());
    assert!(shots.iter().all(|&(a, crit)| (a - own).abs() < 1e-3 && !crit), "its own {own}: {shots:?}");
    // its kill: the hero's, with nothing set off
    let k = r.enemies.iter().position(|x| x.id == e).unwrap();
    r.enemies[k].health = 1.0;
    r.heroes[0].health = 10.0;
    let kills = r.heroes[0].kills;
    let ev = tick_alone(&mut r, e, 1.6);
    assert!(ev.iter().any(|x| matches!(x, Event::EnemyDied { id, by: Some(_) } if *id == e)));
    assert_eq!(r.heroes[0].kills, kills + 1);
    assert!(!ev.iter().any(|x| matches!(x, Event::HeroHealed { .. })), "no lifesteal, no heal per kill");
}
