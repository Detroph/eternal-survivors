//! The assists' rules (`game::assists`): the standards' schedule, the summon pool, each block of
//! a script, and every character's assist run to its end.

use std::sync::Arc;

use esurv_content::{AreaAt, AssistBehaviour, AssistScript, AssistStep, Content, Faction, MoveStyle, Movement, OverTime, SpellEffect, Stat, Targets};

use super::assist_rotation::GLOBAL_COOLDOWN;
use super::assists::{AllyMove, ALLY_BESIDE_ENEMY, ALLY_RUN_SPEED};
use super::casting::{HitSource, HOLD_CHAIN};
use super::director::FIRST_ELITE;
use super::enemy::RunOptions;
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

/// A human hero's run whose first Alliance character assists with `script` alone (a rotation of
/// no spells, a caster's: the script deals all its damage and nothing else does), and that
/// character's id. Nothing hurts the hero.
fn run_with(script: AssistScript) -> (Run, String) {
    run_with_rotation(script, MoveStyle::Caster, vec![])
}

/// As `run_with`, the character looping `rotation` (`assist_spells.ron` ids) moving `style` after
/// its script.
fn run_with_rotation(script: AssistScript, style: MoveStyle, rotation: Vec<u32>) -> (Run, String) {
    let mut c = content();
    let k = c.player_characters.iter().position(|p| c.character_faction(p) == Some(Faction::Alliance)).unwrap();
    c.player_characters[k].assist_script = Some(script);
    c.player_characters[k].behaviour = Some(AssistBehaviour { style, rotation });
    let id = c.player_characters[k].id.clone();
    let mut r = run_in(Arc::new(c), "elwynn", "human", 5);
    immune(&mut r);
    (r, id)
}

fn script(steps: Vec<AssistStep>) -> AssistScript {
    AssistScript { arrive: Default::default(), steps, leave: Default::default(), seconds: None }
}

fn hit(targets: Targets, damage: bool) -> AssistStep {
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

/// The assist rates (`balance.ron` `battle_standard.assist_damage_per_second`): on a normal enemy,
/// and on an elite, a rare or a boss.
fn rates(r: &Run) -> (f32, f32) {
    let x = r.content.balance.battle_standard.assist_damage_per_second;
    (x.normal, x.elite_rare_boss)
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

/// The rates load: 5 % of max health a second on a normal enemy, 1 % on an elite, a rare or a boss.
#[test]
fn the_assist_rates_are_five_and_one_percent_a_second() {
    let r = run_in(Arc::new(content()), "elwynn", "human", 5);
    assert_eq!(rates(&r), (0.05, 0.01));
}

/// A one-off hit of the script stands for one global cooldown: the normal enemy's rate × its max
/// health × 1.5 s, the boss's at the boss rate; the hero's bonuses never touch it.
#[test]
fn a_script_hit_deals_its_rate_for_one_global_cooldown_on_a_normal_enemy_and_a_boss() {
    let (mut r, id) = run_with(script(vec![hit(Targets::Nearest(1), true), hit(Targets::Strongest, true)]));
    // the hero's bonus against elites and bosses does not touch an ally's hit
    r.heroes[0].modifiers.push(super::stats::Modifier::new(Stat::DamageVsElites, 1.0));
    r.now = 300.0;
    let normal = enemy_at(&mut r, 3.0);
    let b = boss(&mut r);
    let (normal_max, boss_max) = (r.enemy(normal).unwrap().max_health, r.enemy(b).unwrap().max_health);
    let (on_normal_rate, on_boss_rate) = rates(&r);
    assert!(r.summon_character(0, &id));
    let ev = r.drain_events();
    let on_normal = assist_hits(&ev, normal);
    assert_eq!(on_normal.len(), 1);
    assert!((on_normal[0] - on_normal_rate * normal_max * GLOBAL_COOLDOWN).abs() < 1e-3, "{on_normal:?}");
    let on_boss = assist_hits(&ev, b);
    assert_eq!(on_boss.len(), 1);
    assert!((on_boss[0] - on_boss_rate * boss_max * GLOBAL_COOLDOWN).abs() < 1e-2, "{on_boss:?}");
}

/// Over a fixed window a swinging ally deals 5 % of a normal enemy's max health a second and 1 %
/// of a boss's, and so does a rotation caster (its casts' cycles); an elite takes the boss rate.
#[test]
fn a_swinging_ally_and_a_rotation_caster_deal_their_rate_a_second_over_a_window() {
    use super::assist_rotation::AUTO_ATTACK_SECONDS;
    const WINDOW: f32 = 10.0;
    for kind in ["normal", "elite", "boss"] {
        for caster in [false, true] {
            // a swinging ally: a look, then auto attacks only; a caster: Frostbolt (3 s) on a loop
            let (mut r, id) = if caster {
                run_with_rotation(script(vec![hit(Targets::Nearest(1), true)]), MoveStyle::Caster, vec![25304])
            } else {
                run_with_rotation(script(vec![AssistStep::Cast { look_spell: 871 }]), MoveStyle::Melee, vec![])
            };
            let e = if kind == "boss" { boss(&mut r) } else { enemy_at(&mut r, 3.0) };
            hold_and_keep(&mut r, e);
            let k = r.enemies.iter().position(|x| x.id == e).unwrap();
            if kind == "elite" {
                r.enemies[k].elite = true;
            }
            let max = r.enemies[k].max_health;
            let rate = if kind == "normal" { rates(&r).0 } else { rates(&r).1 };
            r.summon_character(0, &id);
            // the swinging ally is beside it at its first swing; the caster's first cast lands at 3 s
            let mut ev = r.drain_events();
            let lead = if caster { 3.0 } else { 0.0 };
            ev.extend(tick_alone(&mut r, e, WINDOW + lead - 0.1));
            let all = assist_hits(&ev, e);
            // the caster's script hit (one global cooldown) comes first; its casts after
            let hits = if caster { all[1..].to_vec() } else { all };
            let each = if caster { 3.0 } else { AUTO_ATTACK_SECONDS };
            assert!(hits.iter().all(|&h| (h - rate * max * each).abs() < 1e-3 * max), "{kind} {caster}: {hits:?}");
            let total: f32 = hits.iter().sum();
            // the window's seconds at the rate, within one event of it
            assert!((total - rate * max * WINDOW).abs() <= rate * max * each + 1e-3 * max, "{kind} {caster}: {total} vs {}", rate * max * WINDOW);
        }
    }
}

#[test]
fn crowd_control_goes_through_the_heros_rules_a_boss_held_a_quarter_and_never_feared() {
    let stun = AssistStep::Hit { targets: Targets::OnScreen, damage: false, look_spell: None, melee: false, effects: vec![SpellEffect::Stun { seconds: 2.0 }], wears: vec![1833], over_time: None };
    let fear = AssistStep::Hit { targets: Targets::Strongest, damage: false, look_spell: None, melee: false, effects: vec![SpellEffect::Fear { seconds: 8.0 }], wears: vec![5782], over_time: None };
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
    let stun = AssistStep::Hit { targets: Targets::OnScreen, damage: false, look_spell: None, melee: false, effects: vec![SpellEffect::Stun { seconds: 8.0 }], wears: vec![], over_time: None };
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
    assert_eq!(r.assists.allies.len(), 1, "it stays its 15 s");
    tick_for(&mut r, 10.6);
    assert_eq!(r.assists.allies.len(), 1, "still there 14.8 s in");
    let ev = tick_for(&mut r, 0.4);
    assert!(r.assists.allies.is_empty(), "then goes");
    assert!(ev.iter().any(|x| matches!(x, Event::AllyLeft { id, .. } if *id == a.id)));
}

#[test]
fn a_repeat_strikes_on_its_beat_and_the_ally_leaves_after_its_stay() {
    let strike = AssistStep::Hit { targets: Targets::Nearest(1), damage: true, look_spell: Some(1680), melee: true, effects: vec![], wears: vec![], over_time: None };
    let (mut r, id) = run_with(script(vec![AssistStep::Repeat { every: 1.0, times: 3, steps: vec![strike] }]));
    let e = enemy_at(&mut r, 3.0);
    r.summon_character(0, &id);
    let mut ev = r.drain_events();
    assert_eq!(assist_hits(&ev, e).len(), 1, "the first at once");
    assert!(ev.iter().any(|x| matches!(x, Event::AllySwing { target, .. } if *target == e)));
    assert!(ev.iter().any(|x| matches!(x, Event::AllyMoved { how: AllyMove::Step, .. })));
    ev.extend(tick_for(&mut r, 2.5));
    let hits = assist_hits(&ev, e);
    assert_eq!(hits.len(), 3);
    // each stands for the repeat's beat, 1 s
    let max = r.enemy(e).map_or(0.0, |x| x.max_health);
    assert!(hits.iter().all(|&h| (h - rates(&r).0 * max).abs() < 1e-3), "{hits:?}");
    assert_eq!(assist_hits(&tick_for(&mut r, 12.4), e).len(), 0, "the repeat took 3 s");
    assert_eq!(r.assists.allies.len(), 1, "it stays its 15 s");
    tick_for(&mut r, 0.2);
    assert!(r.assists.allies.is_empty());
}

#[test]
fn the_ally_leaves_at_its_time_whatever_is_left() {
    let strike = AssistStep::Hit { targets: Targets::Nearest(1), damage: false, look_spell: Some(1680), melee: false, effects: vec![], wears: vec![], over_time: None };
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
    let area = AssistStep::Area { at: AreaAt::Hero, radius: 9.0, seconds: 4.0, every: 1.0, damage: true, effects: vec![], look_spell: 10, most: 0 };
    let (mut r, id) = run_with(script(vec![area]));
    let e = enemy_at(&mut r, 3.0);
    r.summon_character(0, &id);
    let mut ev = r.drain_events();
    assert!(ev.iter().any(|x| matches!(x, Event::AreaShown { caster: Unit::Ally(_), spell: 10, .. })));
    assert_eq!(assist_hits(&ev, e).len(), 1);
    ev.extend(tick_for(&mut r, 6.0));
    assert_eq!(assist_hits(&ev, e).len(), 4);
}

/// An area's pulse strikes at most its `most` (5) of a crowd, each for its own rate over the
/// pulse's interval: a normal enemy 5 % of its max health a second, an elite 1 %.
#[test]
fn an_area_pulse_strikes_at_most_five_each_at_its_own_rate() {
    let area = AssistStep::Area { at: AreaAt::Hero, radius: 9.0, seconds: 4.0, every: 2.0, damage: true, effects: vec![], look_spell: 10, most: 5 };
    let (mut r, id) = run_with(script(vec![area]));
    let ids: Vec<u32> = (0..8).map(|i| enemy_at(&mut r, 1.0 + i as f32 * 0.5)).collect();
    for (n, &e) in ids.iter().enumerate() {
        hold_and_keep(&mut r, e);
        let k = r.enemies.iter().position(|x| x.id == e).unwrap();
        r.enemies[k].elite = n % 2 == 1;
    }
    let (normal, elite) = rates(&r);
    r.summon_character(0, &id);
    let mut pulses = vec![r.drain_events()];
    pulses.push(tick_for(&mut r, 2.1));
    for ev in &pulses {
        let struck: Vec<u32> = ids.iter().copied().filter(|&e| !assist_hits(ev, e).is_empty()).collect();
        assert_eq!(struck.len(), 5, "at most 5 a pulse");
        for e in struck {
            let x = r.enemy(e).unwrap();
            let rate = if x.elite { elite } else { normal };
            for h in assist_hits(ev, e) {
                assert!((h - rate * x.max_health * 2.0).abs() < 1e-3 * x.max_health, "{h} on {e}");
            }
        }
    }
}

#[test]
fn a_bleed_ticks_for_its_time() {
    let o = OverTime { every: 1.0, seconds: 3.0, look_spell: Some(703) };
    let bleed = AssistStep::Hit { targets: Targets::Nearest(1), damage: false, look_spell: None, melee: false, effects: vec![], wears: vec![], over_time: Some(o) };
    let (mut r, id) = run_with(script(vec![bleed]));
    let e = enemy_at(&mut r, 3.0);
    let max = r.enemy(e).unwrap().max_health;
    r.summon_character(0, &id);
    let ev = tick_for(&mut r, 5.0);
    let hits = assist_hits(&ev, e);
    assert_eq!(hits.len(), 3);
    // each tick its rate for the tick's second
    assert!(hits.iter().all(|&h| (h - rates(&r).0 * max).abs() < 1e-3), "{hits:?}");
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
    let (mut r, id) = run_with(script(vec![AssistStep::Summon { pet: "Gargoyle".into(), count: 2, seconds: 10.0 }]));
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
    let (mut r, id) = run_with(script(vec![hit(Targets::Nearest(1), true)]));
    r.heroes[0].stats.lifesteal = 1.0;
    r.heroes[0].stats.heal_per_kill = 50.0;
    r.heroes[0].health = 10.0;
    let e = enemy_at(&mut r, 3.0);
    let k = r.enemies.iter().position(|x| x.id == e).unwrap();
    r.enemies[k].health = 1.0;
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
fn an_ally_of_instant_steps_stays_fifteen_seconds() {
    let (mut r, id) = run_with(script(vec![AssistStep::Cast { look_spell: 871 }]));
    r.summon_character(0, &id);
    assert_eq!(r.assists.allies.len(), 1);
    tick_for(&mut r, 14.9);
    assert_eq!(r.assists.allies.len(), 1);
    tick_for(&mut r, 0.2);
    assert!(r.assists.allies.is_empty());
}

/// Stormwall taunts for 10 s: it stays past it (its 15 s, its rotation after the taunt) whether
/// the taunt holds a boss a quarter of it or the enemies it held die.
#[test]
fn a_taunting_ally_stays_past_the_taunts_whole_time() {
    for with_boss in [true, false] {
        let mut r = run_in(Arc::new(content()), "elwynn", "human", 5);
        immune(&mut r);
        let e = if with_boss { boss(&mut r) } else { enemy_at(&mut r, 4.0) };
        // it lives through Stormwall's auto attacks
        let k = r.enemies.iter().position(|x| x.id == e).unwrap();
        r.enemies[k].health = 1e9;
        let now = r.now;
        assert!(r.summon_character(0, "stormwall"));
        // it charges first, then taunts
        tick_for(&mut r, 1.0);
        assert!(r.assists.allies[0].taunts.iter().any(|t| t.0 == e), "it taunts: {:?}", r.assists.allies[0]);
        if !with_boss {
            r.enemies.retain(|x| x.id != e);
        }
        tick_for(&mut r, 13.0);
        assert_eq!(r.assists.allies.len(), 1, "still there 14 s in (boss: {with_boss})");
        // it leaves once its last planned cast landed (its charge made it a little late)
        tick_for(&mut r, 3.0);
        assert!(r.assists.allies.is_empty());
        assert!(r.now - now >= 15.0);
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

/// Ghorktheorc: one Gargoyle hovering 2 yd up casts its bolt about every 1.5 s, each the assist's
/// rate for its attack interval, he runs in and swings once a second for 10 s, each the rate for
/// that second, and after his 15 s both are gone.
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
    let normal = rates(&r).0;
    let bolt = normal * 1000.0 * gargoyle.swing;
    assert!(shots.iter().all(|&(a, crit)| (a - bolt).abs() < 1e-3 && !crit), "{bolt} each: {shots:?}");
    let swings = ev.iter().filter(|x| matches!(x, Event::AllySwing { target, .. } if *target == e)).count();
    let hits = assist_hits(&ev, e);
    assert!((8..=11).contains(&swings), "a swing a second: {swings}");
    assert_eq!(hits.len(), swings);
    // his script's swings stand for their 1 s beat; a rotation cast after it for its cycle
    let script_swings = hits.iter().filter(|&&a| (a - normal * 1000.0).abs() < 1e-3).count();
    assert!((8..=10).contains(&script_swings), "{hits:?}");
    assert!(!r.pets.iter().any(|p| p.id == pet), "the Gargoyle left at its 10 s");
    ev.extend(tick_alone(&mut r, e, 6.5));
    assert!(r.assists.allies.is_empty(), "he left after his 15 s");
    assert!(r.now - now > 15.0 && r.now - now < 17.1);
}

/// An assist's pet deals the assist's damage: not raised by the hero's damage, pet damage or
/// level, never a crit, no lifesteal or heal per kill; its kill is still the hero's.
#[test]
fn an_assists_pets_neither_scale_with_the_hero_nor_set_anything_off() {
    let (mut r, id) = run_with(script(vec![AssistStep::Summon { pet: "Gargoyle".into(), count: 1, seconds: 10.0 }]));
    let swing = r.content.pets.iter().find(|p| p.name == "Gargoyle").unwrap().swing;
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
    let own = rates(&r).0 * r.enemy(e).unwrap().max_health * swing;
    assert!(shots.iter().all(|&(a, crit)| (a - own).abs() < 1e-3 && !crit), "the assist's {own}: {shots:?}");
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

fn spell(spell: u32, cast: f32, cooldown: f32) -> esurv_content::AssistSpell {
    esurv_content::AssistSpell { spell, cast, cooldown, channel: 0.0, tick_every: 0.0, radius: 0.0, on: Default::default() }
}

/// A rotation's plan: in order, a global cooldown between instants, a cast's time before the next,
/// a spell on cooldown skipped for the next ready one, and nothing that would land past the end.
#[test]
fn a_rotation_is_planned_in_order_with_its_global_cooldown_casts_and_cooldowns() {
    use super::assist_rotation::{plan_rotation, GLOBAL_COOLDOWN};
    // two instants: one every 1.5 s, in turn
    let plan = plan_rotation(&[spell(1, 0.0, 0.0), spell(2, 0.0, 0.0)], 6.0);
    assert_eq!(plan.iter().map(|p| (p.at, p.spell)).collect::<Vec<_>>(), vec![(0.0, 0), (1.5, 1), (3.0, 0), (4.5, 1)]);
    assert_eq!(GLOBAL_COOLDOWN, 1.5);
    // a 3 s cast holds the next until it lands; one landing past the end is not planned
    let plan = plan_rotation(&[spell(1, 3.0, 0.0)], 7.0);
    assert_eq!(plan.iter().map(|p| p.at).collect::<Vec<_>>(), vec![0.0, 3.0]);
    // Frost Nova (25 s) once, then the next ready ones
    let plan = plan_rotation(&[spell(122, 0.0, 25.0), spell(10, 0.0, 0.0), spell(116, 1.5, 0.0)], 9.0);
    let spells: Vec<usize> = plan.iter().map(|p| p.spell).collect();
    assert_eq!(spells, vec![0, 1, 2, 1, 2, 1]);
    // all on cooldown: it waits for the first ready
    let plan = plan_rotation(&[spell(1, 0.0, 4.0)], 9.0);
    assert_eq!(plan.iter().map(|p| p.at).collect::<Vec<_>>(), vec![0.0, 4.0, 8.0]);
}

/// The default stay is 15 s for every summon; after its script the ally loops its rotation's
/// spells until then, one cast every global cooldown, then goes.
#[test]
fn an_ally_loops_its_rotation_until_its_fifteen_seconds_are_up() {
    // Sinister Strike and Eviscerate, both instant
    let (mut r, id) = run_with_rotation(script(vec![AssistStep::Cast { look_spell: 2983 }]), MoveStyle::Melee, vec![11294, 31016]);
    assert_eq!(r.content.balance.battle_standard.assist_seconds, 15.0);
    enemy_at(&mut r, 3.0);
    let now = r.now;
    r.summon_character(0, &id);
    let mut ev = r.drain_events();
    ev.extend(tick_for(&mut r, 14.9));
    assert_eq!(r.assists.allies.len(), 1);
    let casts: Vec<u32> = ev
        .iter()
        .filter_map(|e| match e {
            Event::SpellGo { caster: Unit::Ally(_), spell, .. } if *spell != 2983 => Some(*spell),
            _ => None,
        })
        .collect();
    // ten casts in 15 s (0, 1.5 ... 13.5), in turn
    assert_eq!(casts.len(), 10, "{casts:?}");
    assert!(casts.iter().enumerate().all(|(k, s)| *s == if k % 2 == 0 { 11294 } else { 31016 }));
    let ev = tick_for(&mut r, 0.2);
    assert!(r.assists.allies.is_empty(), "gone at 15 s");
    assert!(ev.iter().any(|x| matches!(x, Event::AllyLeft { .. })));
    assert!(r.now - now < 15.2);
}

/// A spell with a cast time starts its cast (the client's cast bar) and goes off once it is done;
/// the ally stands while it casts.
#[test]
fn a_rotation_spell_with_a_cast_time_starts_its_cast_and_goes_off_after_it() {
    // Frostbolt, 3 s
    let (mut r, id) = run_with_rotation(script(vec![AssistStep::Cast { look_spell: 12042 }]), MoveStyle::Caster, vec![25304]);
    let e = enemy_at(&mut r, 6.0);
    r.summon_character(0, &id);
    let ev = r.drain_events();
    let started = ev.iter().find_map(|x| if let Event::AllyCastStarted { spell, seconds, aim, .. } = x { Some((*spell, *seconds, aim.clone())) } else { None });
    assert_eq!(started, Some((25304, 3.0, super::run::Aim::Unit(Unit::Enemy(e)))));
    assert!(!r.assists.allies[0].segment.moving(r.now), "it stands to cast");
    let ev = tick_for(&mut r, 2.9);
    assert!(!ev.iter().any(|x| matches!(x, Event::SpellGo { spell: 25304, .. })), "not before its 3 s");
    let ev = tick_for(&mut r, 0.2);
    assert!(ev.iter().any(|x| matches!(x, Event::SpellGo { caster: Unit::Ally(_), spell: 25304, .. })));
    // five casts land in 15 s (0, 3 ... 12)
    assert_eq!(r.assists.allies[0].rotation.plan.len(), 5);
}

/// Each rotation cast strikes what the script's damaging steps strike, each enemy once, for the
/// rate over its cycle (an instant cast: the 1.5 s global cooldown); a boss at the boss rate.
#[test]
fn each_rotation_cast_strikes_the_scripts_picks_once_for_its_cycle() {
    // two hits on the same boss in the script: two one-off hits, then one a cast
    let strike = hit(Targets::Strongest, true);
    let (mut r, id) = run_with_rotation(script(vec![strike.clone(), strike]), MoveStyle::Melee, vec![11294, 31016]);
    r.now = 300.0;
    let b = boss(&mut r);
    hold_and_keep(&mut r, b);
    let max = r.enemy(b).unwrap().max_health;
    r.summon_character(0, &id);
    assert_eq!(r.assists.allies[0].rotation.plan.len(), 10);
    let mut ev = r.drain_events();
    ev.extend(tick_alone(&mut r, b, 15.5));
    let hits = assist_hits(&ev, b);
    assert_eq!(hits.len(), 12, "the script's two and ten casts' one each");
    let each = rates(&r).1 * max * GLOBAL_COOLDOWN;
    assert!(hits.iter().all(|&h| (h - each).abs() < 1e-3 * max), "{hits:?} vs {each}");
}

/// The rotation's casts deal damage only: the script's crowd control is put on once.
#[test]
fn a_rotation_cast_puts_no_crowd_control_on() {
    let stun = AssistStep::Hit { targets: Targets::OnScreen, damage: true, look_spell: None, melee: false, effects: vec![SpellEffect::Stun { seconds: 2.0 }], wears: vec![], over_time: None };
    let (mut r, id) = run_with_rotation(script(vec![stun]), MoveStyle::Melee, vec![11294]);
    let e = enemy_at(&mut r, 3.0);
    let k = r.enemies.iter().position(|x| x.id == e).unwrap();
    r.enemies[k].health = 1e9;
    let now = r.now;
    r.summon_character(0, &id);
    tick_alone(&mut r, e, 6.0);
    assert!(r.enemy(e).unwrap().hold_until <= now + 2.0 + 1e-4, "held only by the script's stun");
}

/// An erratic melee ally strafes left and right round its enemy and jumps now and then; a
/// nova-kiting caster jumps away after Frost Nova with an enemy close; a healer keeps by the hero.
#[test]
fn the_movement_styles_strafe_jump_away_and_follow() {
    // a rogue
    let (mut r, id) = run_with_rotation(script(vec![AssistStep::Cast { look_spell: 2983 }]), MoveStyle::ErraticMelee, vec![11294, 31016]);
    let e = enemy_at(&mut r, 3.0);
    hold_and_keep(&mut r, e);
    r.summon_character(0, &id);
    let ev = tick_alone(&mut r, e, 6.0);
    let ways: Vec<AllyMove> = ev.iter().filter_map(|x| if let Event::AllyMoved { how, .. } = x { Some(*how) } else { None }).collect();
    assert!(ways.contains(&AllyMove::StrafeLeft) && ways.contains(&AllyMove::StrafeRight), "{ways:?}");
    assert!(ways.contains(&AllyMove::Jump), "{ways:?}");
    // it stays in melee
    let at = r.assists.allies[0].segment.to;
    assert!(dist(at, r.enemy_pos(e).unwrap()) < ALLY_BESIDE_ENEMY + 3.0);

    // a frost mage: Frost Nova, a jump back, then Blizzard on the ground
    let (mut r, id) = run_with_rotation(script(vec![AssistStep::Cast { look_spell: 12042 }]), MoveStyle::NovaKiteCaster, vec![10230, 10187]);
    let e = enemy_at(&mut r, 3.0);
    hold_and_keep(&mut r, e);
    r.summon_character(0, &id);
    let ev = r.drain_events();
    let jump = ev.iter().find_map(|x| if let Event::AllyMoved { how: AllyMove::Jump, to, .. } = x { Some(*to) } else { None });
    let to = jump.expect("it jumps away after Frost Nova");
    let from = r.assists.allies[0].segment.from;
    let foe = r.enemy_pos(e).unwrap();
    assert!(dist(to, foe) > dist(from, foe), "away from the enemy");
    let ev = tick_alone(&mut r, e, 1.6);
    assert!(ev.iter().any(|x| matches!(x, Event::SpellGo { spell: 10187, aim: super::run::Aim::Ground(_), .. })), "then Blizzard on the ground");

    // a healer follows the hero
    let (mut r, id) = run_with_rotation(script(vec![AssistStep::Cast { look_spell: 12042 }]), MoveStyle::Healer, vec![10917]);
    r.summon_character(0, &id);
    let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
    r.hero_moved(h, (p.0 + 12.0, p.1), 0.0, 0.0);
    tick_for(&mut r, 3.0);
    let ally = r.assists.allies[0].segment.position(r.now);
    assert!(dist(ally, r.heroes[0].pos) < 4.0, "beside the hero again");
}

/// Enemy `e` stays where it is and does not die (the hero's own swings would kill it).
fn hold_and_keep(r: &mut Run, e: u32) {
    let k = r.enemies.iter().position(|x| x.id == e).unwrap();
    (r.enemies[k].hold_until, r.enemies[k].health, r.enemies[k].max_health) = (100.0, 1e12, 1e12);
}

/// A melee ally with an empty rotation only auto attacks: a swing at the enemy beside it every
/// `AUTO_ATTACK_SECONDS`, each its swing for the assist's rate over `AUTO_ATTACK_SECONDS` as any
/// of its hits, and no rotation spell.
#[test]
fn a_melee_ally_with_no_rotation_only_auto_attacks() {
    use super::assist_rotation::AUTO_ATTACK_SECONDS;
    let (mut r, id) = run_with_rotation(script(vec![AssistStep::Cast { look_spell: 871 }]), MoveStyle::Melee, vec![]);
    let e = enemy_at(&mut r, 3.0);
    let k = r.enemies.iter().position(|x| x.id == e).unwrap();
    r.enemies[k].health = 1e9;
    r.summon_character(0, &id);
    let mut ev = r.drain_events();
    ev.extend(tick_alone(&mut r, e, 10.0));
    let swings = ev.iter().filter(|x| matches!(x, Event::AllySwing { target, .. } if *target == e)).count();
    let most = (10.0 / AUTO_ATTACK_SECONDS) as usize + 1;
    assert!(swings >= 3 && swings <= most, "{swings} swings");
    let hits = assist_hits(&ev, e);
    assert_eq!(hits.len(), swings, "each swing a hit");
    let swing = rates(&r).0 * r.enemies[k].max_health * AUTO_ATTACK_SECONDS;
    assert!(hits.iter().all(|&h| (h - swing).abs() < 1e-3), "{hits:?}");
    assert!(!ev.iter().any(|x| matches!(x, Event::SpellGo { caster: Unit::Ally(_), spell, .. } if *spell != 871)), "no rotation spell");
}

/// An ally that leaves after its script does nothing more: it goes once its taunt no longer
/// holds, well before its 15 s.
#[test]
fn an_ally_that_leaves_after_its_script_goes_when_its_taunt_is_over() {
    let taunt = AssistStep::Taunt { targets: Targets::OnScreen, seconds: 4.0, look_spell: None };
    let (mut r, id) = run_with_rotation(script(vec![taunt]), MoveStyle::LeavesAfterScript, vec![]);
    let e = enemy_at(&mut r, 5.0);
    let k = r.enemies.iter().position(|x| x.id == e).unwrap();
    r.enemies[k].health = 1e9;
    r.summon_character(0, &id);
    let mut ev = r.drain_events();
    ev.extend(tick_alone(&mut r, e, 3.8));
    assert_eq!(r.assists.allies.len(), 1, "its taunt still holds");
    ev.extend(tick_alone(&mut r, e, 0.4));
    assert!(r.assists.allies.is_empty(), "gone with its taunt");
    assert!(!ev.iter().any(|x| matches!(x, Event::AllySwing { .. } | Event::AllyCastStarted { .. })));
    assert!(!ev.iter().any(|x| matches!(x, Event::SpellGo { caster: Unit::Ally(_), .. })));
}

/// An assist's Blizzard is channelled: the ally stands and channels for its 8 s, the cast's
/// round coming in its 8 ticks (each the rate for the tick's second), the enemies in its storm
/// slowed by 40 % at each tick; the next cast waits for the channel's end.
#[test]
fn an_assists_blizzard_is_channelled_its_round_in_ticks_slowing_the_enemies_in_it() {
    use super::assist_rotation::CHANNEL_SLOW;
    let strike = hit(Targets::Strongest, true);
    let (mut r, id) = run_with_rotation(script(vec![strike]), MoveStyle::Caster, vec![10187]);
    let row = *r.content.assist_spell(10187).unwrap();
    assert_eq!((row.channel, row.tick_every, row.radius, row.ticks()), (8.0, 1.0, 8.0, 8));
    let e = enemy_at(&mut r, 5.0);
    let k = r.enemies.iter().position(|x| x.id == e).unwrap();
    r.enemies[k].health = 1e9;
    r.summon_character(0, &id);
    // one Blizzard fits in the stay (the next would end past it)
    assert_eq!(r.assists.allies[0].rotation.plan.len(), 1);
    let mut ev = r.drain_events();
    ev.extend(tick_alone(&mut r, e, 0.5));
    let a = &r.assists.allies[0];
    assert!(a.rotation.channeling.is_some(), "it channels");
    assert!(!a.segment.moving(r.now), "standing");
    assert!(ev.iter().any(|x| matches!(x, Event::SpellGo { caster: Unit::Ally(_), spell: 10187, .. })));
    assert!(ev.iter().any(|x| matches!(x, Event::AreaShown { caster: Unit::Ally(_), spell: 10187, radius, .. } if *radius == 8.0)));
    ev.extend(tick_alone(&mut r, e, 1.0));
    let slow = r.enemies.iter().find(|x| x.id == e).unwrap().slow;
    assert_eq!(slow.map(|s| s.0), Some(CHANNEL_SLOW), "slowed in the storm");
    ev.extend(tick_alone(&mut r, e, 8.0));
    assert!(r.assists.allies[0].rotation.channeling.is_none(), "the channel ended");
    let hits = assist_hits(&ev, e);
    assert_eq!(hits.len(), 9, "the script's and eight ticks: {hits:?}");
    let max = r.enemy(e).unwrap().max_health;
    assert!((hits[0] - rates(&r).0 * max * GLOBAL_COOLDOWN).abs() < 1e-3, "the script's hit: {hits:?}");
    for t in &hits[1..] {
        assert!((t - rates(&r).0 * max * row.tick_every).abs() < 1e-3, "each tick its second: {hits:?}");
    }
}

/// In a crowd, Wayze's Thunderfury, Doogz's and Scarfex's Garrote and Snutz's fire reach at most 5
/// enemies a proc, a cast and a pulse (for performance), each still dealing damage.
#[test]
fn the_crowd_assists_reach_at_most_five_enemies_at_a_time() {
    for who in ["wayze", "doogz", "scarfex", "snutz"] {
        let mut r = run_in(Arc::new(content()), "elwynn", if who == "snutz" { "orc" } else { "dwarf" }, 5);
        immune(&mut r);
        for i in 0..60 {
            let (a, d) = (i as f32 * 0.4, 4.0 + (i % 6) as f32);
            let p = r.heroes[0].pos;
            if !r.spawn_roster_enemy(0, Some((p.0 + a.cos() * d, p.1 + a.sin() * d))) {
                break;
            }
            let k = r.enemies.len() - 1;
            r.enemies[k].health = 1e9;
        }
        let ids: Vec<u32> = r.enemies.iter().map(|e| e.id).collect();
        assert!(ids.len() > 10, "a crowd");
        r.summon_character(0, who);
        let mut ev = r.drain_events();
        let end = r.now + 16.0;
        while r.now < end {
            r.enemies.retain(|e| ids.contains(&e.id));
            r.tick(0.05);
            ev.extend(r.drain_events());
        }
        for e in &ev {
            if let Event::SpellGo { caster: Unit::Ally(_), hits, spell, .. } = e {
                assert!(hits.len() <= 5, "{who}: {spell} reached {}", hits.len());
            }
        }
        // the hits of each tick: at most 5 a proc, a cast or a pulse (a few may share a tick)
        let hits = ev.iter().filter(|e| matches!(e, Event::EnemyHit { source: HitSource::Assist(_), .. })).count();
        assert!(hits > 0, "{who} still hits");
    }
    // the picks stay: Thunderfury's damage on each of the 5
    let c = content();
    let wayze = c.player_character("wayze").unwrap();
    let thunderfury = wayze.assist_script.as_ref().unwrap().steps.iter().find_map(|s| match s {
        AssistStep::Hit { targets, damage, look_spell: Some(21992), .. } => Some((targets.clone(), *damage)),
        _ => None,
    });
    assert_eq!(thunderfury, Some((Targets::Nearest(5), true)));
    // and the Garrote's 10 s on each of the 5
    for who in ["doogz", "scarfex"] {
        let c = c.player_character(who).unwrap();
        let garrote = c.assist_script.as_ref().unwrap().steps.iter().find_map(|s| match s {
            AssistStep::Hit { targets, over_time: Some(o), look_spell: Some(703), .. } => Some((targets.clone(), o.seconds)),
            _ => None,
        });
        assert_eq!(garrote, Some((Targets::Nearest(5), 10.0)), "{who}");
    }
}

/// Our rule (owner): every melee ally strafes round its enemy, while its script strikes too; one
/// that taunts stands while its taunt holds.
#[test]
fn a_melee_ally_strafes_while_it_strikes_but_stands_while_it_taunts() {
    let strike = AssistStep::Repeat { every: 1.0, times: 6, steps: vec![AssistStep::Hit { targets: Targets::Nearest(1), damage: true, look_spell: None, melee: true, effects: vec![], wears: vec![], over_time: None }] };
    let (mut r, id) = run_with_rotation(script(vec![strike]), MoveStyle::Melee, vec![]);
    let e = enemy_at(&mut r, 3.0);
    hold_and_keep(&mut r, e);
    r.summon_character(0, &id);
    let ev = tick_alone(&mut r, e, 5.0);
    let ways: Vec<AllyMove> = ev.iter().filter_map(|x| if let Event::AllyMoved { how, .. } = x { Some(*how) } else { None }).collect();
    assert!(ways.contains(&AllyMove::StrafeLeft) && ways.contains(&AllyMove::StrafeRight), "strafes in its script: {ways:?}");

    let taunt = AssistStep::Taunt { targets: Targets::OnScreen, seconds: 10.0, look_spell: None };
    let (mut r, id) = run_with_rotation(script(vec![taunt, AssistStep::Cast { look_spell: 355 }]), MoveStyle::Melee, vec![]);
    let e = enemy_at(&mut r, 3.0);
    hold_and_keep(&mut r, e);
    r.summon_character(0, &id);
    let ev = tick_alone(&mut r, e, 6.0);
    assert!(!ev.iter().any(|x| matches!(x, Event::AllyMoved { how: AllyMove::StrafeLeft | AllyMove::StrafeRight | AllyMove::Jump, .. })), "it stands while it taunts");
}

/// Moffel's Thunderfury lightning shows on her first swing only (for performance), while all
/// eight swings still strike up to 5 enemies.
#[test]
fn moffels_lightning_shows_once_and_her_swings_still_strike_five() {
    let mut r = run_in(Arc::new(content()), "elwynn", "human", 5);
    immune(&mut r);
    for i in 0..30 {
        let (a, d) = (i as f32 * 0.4, 4.0 + (i % 6) as f32);
        let p = r.heroes[0].pos;
        if !r.spawn_roster_enemy(0, Some((p.0 + a.cos() * d, p.1 + a.sin() * d))) {
            break;
        }
        let k = r.enemies.len() - 1;
        r.enemies[k].health = 1e9;
    }
    let ids: Vec<u32> = r.enemies.iter().map(|e| e.id).collect();
    r.summon_character(0, "moffel");
    let mut ev = r.drain_events();
    let end = r.now + 10.0;
    while r.now < end {
        r.enemies.retain(|e| ids.contains(&e.id));
        r.tick(0.05);
        ev.extend(r.drain_events());
    }
    let lightning = ev.iter().filter(|e| matches!(e, Event::SpellGo { caster: Unit::Ally(_), spell: 21992, .. })).count();
    assert_eq!(lightning, 1);
    let hits = ev.iter().filter(|e| matches!(e, Event::EnemyHit { source: HitSource::Assist(_), .. })).count();
    assert!(hits >= 8 * 6, "eight swings of 1 + 5: {hits}");
}
