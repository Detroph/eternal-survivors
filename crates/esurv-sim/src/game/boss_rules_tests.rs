//! The Elwynn and Duskwood bosses' mechanics (docs/game/bosses.md,
//! "Elwynn and Duskwood"), played on flat ground: marks wind up long enough to walk out and land
//! on the hero only, the scripts' waves, Raise Dead and the roster swap, the relic's shield, the
//! weapon swaps, the stacks, and the warnings and timer bars.

use std::sync::Arc;

use esurv_content::{BossAbilityKind, Content, CreatureType, Shape};

use super::enemy::RunOptions;
use super::enemy_ai::Segment;
use super::run::{EnemyKind, Event, Flat, HeroSetup, Run};

fn run_in(zone: &str) -> Run {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
    let content = Arc::new(Content::load(dir).expect("content loads"));
    let hero = HeroSetup { class: "warrior".into(), race: "orc".into(), ..Default::default() };
    let mut r = Run::new(content, zone, RunOptions::default(), &[hero], Box::new(Flat), 9).unwrap();
    let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
    r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
    r.enemies.clear();
    r.pickups.clear();
    r.heroes[0].spells.known.clear();
    r.heroes[0].spells.racial = None;
    r.drain_events();
    r
}

/// Boss `name` of the run's zone, brought in 3 yd from the hero, free to act at once; its index.
fn boss_near(r: &mut Run, name: &str) -> usize {
    let b = r.content.zones[r.zone].bosses.iter().position(|b| b.name == name).unwrap();
    r.spawn_boss(b);
    let k = r.enemies.len() - 1;
    let p = r.heroes[0].pos;
    r.enemies[k].segment = Segment::standing((p.0 + 3.0, p.1), r.now);
    r.enemies[k].hold_until = 0.0;
    r.enemies[k].next_ability_at = 0.0;
    k
}

fn boss_index(r: &Run) -> usize {
    r.enemies.iter().position(|e| e.is_boss()).unwrap()
}

/// Casts the boss's ability `slot` now.
fn cast(r: &mut Run, slot: usize) {
    let k = boss_index(r);
    for (i, s) in r.enemies[k].abilities.iter_mut().enumerate() {
        s.timer = if i == slot { 0.0 } else { 1e6 };
    }
    r.enemies[k].next_ability_at = 0.0;
    r.enemies[k].mech.busy_until = 0.0;
    r.enemy_abilities(k, 0, 0.0, false);
}

fn set_health(r: &mut Run, pct: f32) {
    let k = boss_index(r);
    r.enemies[k].health = r.enemies[k].max_health * pct / 100.0;
    r.boss_script_tick(k);
}

#[test]
fn every_mark_winds_up_long_enough_to_walk_out() {
    let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
    for z in &content.zones {
        for b in &z.bosses {
            let mut kits = vec![b.abilities.clone()];
            kits.extend(b.script.iter().flat_map(|l| &l.run).filter_map(|a| match a {
                esurv_content::ScriptAction::Kit { resolved, .. } => Some(resolved.clone()),
                _ => None,
            }));
            for a in kits.iter().flatten() {
                if let BossAbilityKind::Marked { wind_up, .. } | BossAbilityKind::Rush { wind_up, .. } = a.kind {
                    assert!((1.0..=2.5).contains(&wind_up), "{}: {} winds up {wind_up} s", z.id, b.name);
                }
            }
        }
    }
}

/// Rhahk'Zor's slam: marked round him for its wind-up while he stands, then it lands on the hero
/// still inside and never on an enemy of the horde.
#[test]
fn a_slam_is_marked_for_its_wind_up_and_lands_on_the_hero_only() {
    let mut r = run_in("elwynn");
    let p = r.heroes[0].pos;
    assert!(r.spawn_roster_enemy(0, Some((p.0 + 1.0, p.1 + 1.0))));
    r.enemies[0].damage = 0.0;
    let vermin = (r.enemies[0].id, r.enemies[0].health);
    let k = boss_near(&mut r, "Rhahk'Zor");
    r.drain_events();
    cast(&mut r, 0);
    let shown = r.drain_events();
    let wind_up = shown.iter().find_map(|e| match e {
        Event::TelegraphShown { shape: Shape::Circle { radius }, wind_up, .. } if *radius == 10.0 => Some(*wind_up),
        _ => None,
    });
    assert_eq!(wind_up, Some(1.8), "marked for 1.8 s");
    assert!(shown.iter().any(|e| matches!(e, Event::Warning { soon: false, .. })), "and announced");
    assert!(r.now < r.enemies[k].mech.busy_until, "he stands while it winds up");
    let health = r.heroes[0].health;
    for _ in 0..17 {
        r.tick(0.1);
    }
    assert_eq!(r.heroes[0].health, health, "nothing before it lands");
    let mut landed = Vec::new();
    for _ in 0..3 {
        r.tick(0.1);
        landed.extend(r.drain_events());
    }
    assert!(r.heroes[0].health < health, "it landed on the hero inside");
    // the vermin beside the hero took nothing from it (the hero's own swings aside)
    assert!(!landed.iter().any(|e| matches!(e, Event::EnemyHit { id, by: None, .. } if *id == vermin.0)), "and not on the vermin beside it");
    // walked out in time: nothing
    r.heroes[0].health = r.heroes[0].stats.max_health;
    cast(&mut r, 0);
    let (id, p) = (r.heroes[0].id, r.heroes[0].pos);
    r.hero_moved(id, (p.0 - 12.0, p.1), 0.0, 0.0);
    let health = r.heroes[0].health;
    r.now += 2.0;
    r.enemy_hits_tick();
    assert_eq!(r.heroes[0].health, health, "out of the circle: no damage");
}

/// Hogger's head butt is a cone in front of him: a hero behind him is not hit.
#[test]
fn a_cone_hits_in_front_only() {
    let mut r = run_in("elwynn");
    boss_near(&mut r, "Hogger");
    cast(&mut r, 0);
    // the hero steps behind him
    let (id, p) = (r.heroes[0].id, r.heroes[0].pos);
    r.hero_moved(id, (p.0 + 6.0, p.1), 0.0, 0.0);
    let health = r.heroes[0].health;
    r.now += 1.3;
    r.enemy_hits_tick();
    assert_eq!(r.heroes[0].health, health);
}

/// Edwin VanCleef: "Lapdogs, soon" at 55 %, then at 50 % his line with his own voice and six
/// Defias Blackguards round him.
#[test]
fn vancleef_calls_his_lapdogs_at_half_health() {
    let mut r = run_in("elwynn");
    boss_near(&mut r, "Edwin VanCleef");
    r.drain_events();
    set_health(&mut r, 54.0);
    let soon = r.drain_events();
    assert!(soon.iter().any(|e| matches!(e, Event::Warning { text, soon: true, .. } if text == "Lapdogs soon")));
    assert_eq!(r.enemies.len(), 1, "no wave yet");
    set_health(&mut r, 49.0);
    let at_half = r.drain_events();
    assert!(at_half.iter().any(|e| matches!(e, Event::Yell { text, .. } if text == "Lapdogs, all of you!")));
    assert!(at_half.iter().any(|e| matches!(e, Event::Sound(5782))), "his own voice");
    assert!(at_half.iter().any(|e| matches!(e, Event::Warning { soon: false, .. })));
    let guards = r.enemies.iter().filter(|e| matches!(e.kind, EnemyKind::Roster(i) if r.content.zones[r.zone].enemies[i].name == "Defias Blackguard")).count();
    assert_eq!(guards, 6);
    // once only
    set_health(&mut r, 40.0);
    assert_eq!(r.enemies.len(), 7);
}

/// Mr. Smite at 66 %: he kneels 2 s, takes up his two-hander and sweeps a big cone.
#[test]
fn mr_smite_kneels_and_swaps_weapons() {
    let mut r = run_in("elwynn");
    let k = boss_near(&mut r, "Mr. Smite");
    assert!(r.enemies[k].mech.weapons.is_none());
    r.drain_events();
    set_health(&mut r, 65.0);
    let ev = r.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::BossKneels { kneeling: true, .. })));
    assert!(ev.iter().any(|e| matches!(e, Event::Sound(5778))));
    let k = boss_index(&r);
    assert!((r.enemies[k].mech.busy_until - r.now - 2.0).abs() < 1e-4, "kneeling 2 s");
    let w = r.enemies[k].mech.weapons.unwrap();
    assert_eq!(w.main_hand.map(|w| w.display), Some(19766));
    assert!(w.off_hand.is_none());
    assert!(matches!(r.enemies[k].abilities[0].ability.kind, BossAbilityKind::Marked { shape: Shape::Cone { .. }, .. }));
    r.now += 2.1;
    r.boss_script_tick(k);
    assert!(r.drain_events().iter().any(|e| matches!(e, Event::BossKneels { kneeling: false, .. })), "and stands up");
}

/// Mother Fang's eggs hatch unless the hero walks over them in time.
#[test]
fn mother_fang_lays_eggs_that_hatch() {
    let mut r = run_in("elwynn");
    boss_near(&mut r, "Mother Fang");
    cast(&mut r, 1);
    assert_eq!(r.ground_objects.len(), 2);
    // the hero walks over the first
    let (id, at) = (r.heroes[0].id, r.ground_objects[0].at);
    r.hero_moved(id, at, 0.0, 0.0);
    for _ in 0..6 {
        r.ground_objects_tick(0.1);
    }
    assert_eq!(r.ground_objects.len(), 1, "broken by walking over it");
    let before = r.enemies.len();
    r.now += 10.5;
    r.hero_moved(id, (at.0 + 30.0, at.1), 0.0, 0.0);
    r.ground_objects_tick(0.1);
    assert_eq!(r.enemies.len(), before + 3, "the other hatched");
}

/// Mother Fang's webs: standing in one stacks her poison on the hero (damage only).
#[test]
fn a_web_stacks_poison_on_a_hero_standing_in_it() {
    let mut r = run_in("elwynn");
    boss_near(&mut r, "Mother Fang");
    cast(&mut r, 0);
    r.now += 1.6;
    r.enemy_hits_tick();
    assert!(!r.ground_zones.is_empty());
    for _ in 0..3 {
        r.now += 1.0;
        r.ground_zones_tick(0.0);
    }
    assert!(r.heroes[0].afflictions.first().is_some_and(|a| a.stacks >= 2));
}

/// Morbent Fel: as he arrives every living enemy near the hero dies and rises as a skeleton,
/// only the undead spawn from then on, and he is shielded until the hero takes Morbent's Bane.
#[test]
fn morbent_raises_the_dead_swaps_the_roster_and_is_shielded_until_his_bane() {
    let mut r = run_in("duskwood");
    let zone = r.content.zones[r.zone].clone();
    let living = zone.enemies.iter().position(|e| e.creature_type != CreatureType::Undead && !e.only_summoned && !e.elite).unwrap();
    let undead = zone.enemies.iter().position(|e| e.creature_type == CreatureType::Undead && e.name != "Skeletal Raider").unwrap();
    let p = r.heroes[0].pos;
    assert!(r.spawn_roster_enemy(living, Some((p.0 + 5.0, p.1))));
    assert!(r.spawn_roster_enemy(living, Some((p.0, p.1 + 8.0))));
    assert!(r.spawn_roster_enemy(undead, Some((p.0 - 5.0, p.1))));
    boss_near(&mut r, "Morbent Fel");
    let ev = r.drain_events();
    let k = boss_index(&r);
    assert_eq!(ev.iter().filter(|e| matches!(e, Event::EnemyDied { by: None, .. })).count(), 2, "the two living died");
    let raiders = r.enemies.iter().filter(|e| matches!(e.kind, EnemyKind::Roster(i) if zone.enemies[i].name == "Skeletal Raider")).count();
    assert_eq!(raiders, 2, "and rose as skeletons");
    assert!(r.enemies.iter().any(|e| e.kind == EnemyKind::Roster(undead)), "the undead one stays");
    // from now on only the undead spawn
    assert_eq!(r.spawn_rules.only, Some(CreatureType::Undead));
    assert!(r.spawn_roster_enemy(living, None));
    let last = r.enemies.last().unwrap().kind;
    assert!(matches!(last, EnemyKind::Roster(i) if zone.enemies[i].creature_type == CreatureType::Undead), "{last:?}");
    // shielded: a tenth of each hit
    let boss = r.enemies.iter().find(|e| e.is_boss()).unwrap().id;
    let health = r.enemies[k].health;
    r.hit_enemy(None, boss, 1000.0, false, super::casting::HitSource::Proc { spell: 1, school: esurv_content::School::Holy, origin: Default::default() });
    let k = boss_index(&r);
    assert!((health - r.enemies[k].health - 100.0).abs() < 1.0, "shielded");
    assert!(ev.iter().any(|e| matches!(e, Event::Warning { text, .. } if text.contains("relic"))), "the relic is announced");
    // the hero takes Morbent's Bane: mortal for 25 s, the relic back after 15 s
    assert_eq!(r.ground_objects.len(), 1);
    let (id, at) = (r.heroes[0].id, r.ground_objects[0].at);
    r.hero_moved(id, at, 0.0, 0.0);
    for _ in 0..6 {
        r.ground_objects_tick(0.1);
    }
    assert!(r.ground_objects.is_empty());
    let health = r.enemies[k].health;
    r.hit_enemy(None, boss, 1000.0, false, super::casting::HitSource::Proc { spell: 1, school: esurv_content::School::Holy, origin: Default::default() });
    let k = boss_index(&r);
    assert!((health - r.enemies[k].health - 1000.0).abs() < 1.0, "mortal");
    // the relic drops again 15 s after it was taken, while he is still mortal
    r.now += 15.1;
    r.ground_objects_tick(0.0);
    assert_eq!(r.ground_objects.len(), 1);
    r.now += 10.0;
    let health = r.enemies[k].health;
    r.hit_enemy(None, boss, 1000.0, false, super::casting::HitSource::Proc { spell: 1, school: esurv_content::School::Holy, origin: Default::default() });
    let k = boss_index(&r);
    assert!((health - r.enemies[k].health - 100.0).abs() < 1.0, "shielded again");
}

/// Morbent's Touch of Death: a slow bolt at the hero's spot, its mark laid for 2.5 s.
#[test]
fn touch_of_death_is_a_slow_bolt_to_walk_away_from() {
    let mut r = run_in("duskwood");
    boss_near(&mut r, "Morbent Fel");
    r.drain_events();
    cast(&mut r, 0);
    assert!(r.drain_events().iter().any(|e| matches!(e, Event::TelegraphShown { wind_up, .. } if *wind_up == 2.5)));
    let (id, p) = (r.heroes[0].id, r.heroes[0].pos);
    r.hero_moved(id, (p.0 - 6.0, p.1), 0.0, 0.0);
    let health = r.heroes[0].health;
    r.now += 2.6;
    r.enemy_hits_tick();
    assert_eq!(r.heroes[0].health, health, "walked away from it");
}

/// Lupos's bites stack a shadow wound on the hero; Stitches's rot follows him and hurts a hero
/// close to him.
#[test]
fn lupos_bites_stack_and_stitches_rots_whoever_stays_close() {
    let mut r = run_in("duskwood");
    let k = boss_near(&mut r, "Lupos");
    for _ in 0..3 {
        r.enemies[k].swing_timer = 0.0;
        r.enemies[k].mech.busy_until = 0.0;
        for s in &mut r.enemies[k].abilities {
            s.timer = 1e6;
        }
        r.enemies_tick(0.01);
    }
    assert_eq!(r.heroes[0].afflictions.first().map(|a| a.stacks), Some(3));
    let mut r = run_in("duskwood");
    boss_near(&mut r, "Stitches");
    assert_eq!(r.ground_zones.len(), 1, "his rot came with him");
    assert!(r.ground_zones[0].spec.follows);
    let health = r.heroes[0].health;
    r.now += 1.0;
    r.ground_zones_tick(0.0);
    assert!(r.heroes[0].health < health, "close to him: it hurts");
}

/// Hogger's pack at half health, from the arena's edges; Lupos howls on a timer, with a timer
/// bar for his next howl.
#[test]
fn hogger_calls_his_pack_and_lupos_howls_on_his_timer() {
    let mut r = run_in("elwynn");
    boss_near(&mut r, "Hogger");
    set_health(&mut r, 49.0);
    assert_eq!(r.enemies.len(), 7, "six gnolls");
    let mut r = run_in("duskwood");
    boss_near(&mut r, "Lupos");
    let bars = r.drain_events();
    assert!(bars.iter().any(|e| matches!(e, Event::BarStarted { label, until, .. } if label == "Howl" && (*until - r.now - 20.0).abs() < 1e-3)));
    r.now += 20.0;
    let k = boss_index(&r);
    r.boss_script_tick(k);
    assert_eq!(r.enemies.len(), 5, "four wolves");
}

/// A warned ability's timer bar starts as its boss arrives and starts again when it is cast.
#[test]
fn timer_bars_count_down_to_each_warned_ability() {
    let mut r = run_in("elwynn");
    boss_near(&mut r, "Rhahk'Zor");
    let arrived = r.drain_events();
    let first = arrived.iter().find_map(|e| match e {
        Event::BarStarted { key: 0, until, .. } => Some(*until),
        _ => None,
    });
    assert!((first.unwrap() - r.now - 9.0 * super::boss_abilities::BOSS_FIRST_CAST).abs() < 1e-3);
    cast(&mut r, 0);
    assert!(r.drain_events().iter().any(|e| matches!(e, Event::BarStarted { key: 0, until, .. } if (*until - r.now - 9.0).abs() < 1e-3)));
}

/// The slot of the boss's rush in its kit.
fn rush_slot(r: &Run) -> usize {
    let k = boss_index(r);
    r.enemies[k].abilities.iter().position(|s| matches!(s.ability.kind, BossAbilityKind::Rush { .. })).unwrap()
}

/// Hogger casts his rush with the hero 10 yd off; the mark's line, as shown.
fn hogger_rushes(r: &mut Run) -> (Shape, (f32, f32), f32) {
    let k = boss_near(r, "Hogger");
    let p = r.heroes[0].pos;
    r.enemies[k].segment = Segment::standing((p.0 + 10.0, p.1), r.now);
    r.drain_events();
    cast(r, rush_slot(r));
    r.drain_events()
        .iter()
        .find_map(|e| match e {
            Event::TelegraphShown { shape: s @ Shape::Line { .. }, at, facing, .. } => Some((*s, *at, *facing)),
            _ => None,
        })
        .expect("its line is marked")
}

/// Ticks until the boss's run has ended; the hits the hero took from no one (an area), and when.
fn run_out_the_rush(r: &mut Run) -> Vec<(f32, f32)> {
    let mut hits = Vec::new();
    for _ in 0..60 {
        r.tick(0.05);
        for e in r.drain_events() {
            if let Event::HeroHit { amount, by: None, .. } = e {
                hits.push((r.now, amount));
            }
        }
    }
    hits
}

/// Hogger's charge: the line runs from him through the hero's spot to
/// 8 yd past it; once marked, he runs all of it at a charge's speed and the hero on it is hit
/// once, as he passes, not when the mark lands. He ends at the far end; no knock, no stun.
#[test]
fn a_charge_runs_through_its_line_and_hits_the_hero_on_it_once_as_it_passes() {
    let mut r = run_in("elwynn");
    let start = r.heroes[0].pos;
    let p = r.heroes[0].pos;
    assert!(r.spawn_roster_enemy(0, Some((p.0 + 5.0, p.1))));
    r.enemies[0].damage = 0.0;
    let vermin = r.enemies[0].id;
    let (shape, at, facing) = hogger_rushes(&mut r);
    let Shape::Line { length, width } = shape else { unreachable!() };
    assert!((length - 18.0).abs() < 1e-3, "10 yd to the hero and 8 past: {length}");
    assert_eq!(width, 4.0);
    let end = (at.0 + facing.cos() * length, at.1 + facing.sin() * length);
    let mut landed = Vec::new();
    let health = r.heroes[0].health;
    let lands = r.telegraphs[0].lands_at;
    let mut hits = Vec::new();
    for _ in 0..60 {
        r.tick(0.05);
        for e in r.drain_events() {
            if let Event::HeroHit { amount, by: None, .. } = e {
                hits.push((r.now, amount));
            }
            landed.push(e);
        }
    }
    assert_eq!(hits.len(), 1, "hit once: {hits:?}");
    let run = length / super::telegraphs::RUSH_SPEED;
    let (when, _) = hits[0];
    assert!(when > lands + 0.2 && when < lands + run, "mid-run, as he passes: {when} (lands {lands}, runs {run} s)");
    assert!(r.heroes[0].health < health);
    let k = boss_index(&r);
    let at_end = r.enemies[k].position(r.now);
    assert!((at_end.0 - end.0).abs() < 1e-3 && (at_end.1 - end.1).abs() < 1e-3, "he ends at the far end: {at_end:?} vs {end:?}");
    assert!(r.charges.is_empty(), "the run is over");
    // no crowd control: the hero was not moved, and the charge's look lists no one hit
    assert_eq!(r.heroes[0].pos, start);
    assert!(!landed.iter().any(|e| matches!(e, Event::HeroTeleported { .. })));
    assert!(landed.iter().any(|e| matches!(e, Event::SpellGo { spell: 100, hits, .. } if hits.is_empty())), "his charge plays, at no one");
    // the vermin on the line took nothing from it (hero only)
    assert!(!landed.iter().any(|e| matches!(e, Event::EnemyHit { id, by: None, .. } if *id == vermin)), "not on the vermin on the line");
}

/// A hero who side-steps the charge line in its wind-up is not hit as the boss runs by, and the
/// boss still runs to the far end.
#[test]
fn a_hero_beside_the_charge_line_is_not_hit() {
    let mut r = run_in("elwynn");
    let (shape, at, facing) = hogger_rushes(&mut r);
    let Shape::Line { length, .. } = shape else { unreachable!() };
    let (id, p) = (r.heroes[0].id, r.heroes[0].pos);
    r.hero_moved(id, (p.0, p.1 + 3.0), 0.0, 0.0);
    // the boss reaches the far end (it may walk on once it has, so its closest pass is what counts)
    let end = (at.0 + facing.cos() * length, at.1 + facing.sin() * length);
    let k = boss_index(&r);
    let (mut hits, mut closest) = (Vec::new(), f32::MAX);
    for _ in 0..60 {
        r.tick(0.05);
        for e in r.drain_events() {
            if let Event::HeroHit { amount, by: None, .. } = e {
                hits.push((r.now, amount));
            }
        }
        let q = r.enemies[k].position(r.now);
        closest = closest.min(((q.0 - end.0).powi(2) + (q.1 - end.1).powi(2)).sqrt());
    }
    assert!(hits.is_empty(), "{hits:?}");
    assert!(closest < 1e-3, "it ran to the far end: {closest}");
}

/// The mark's line is the very line the run is tested against: its ends and width as told.
#[test]
fn a_charge_marks_the_line_it_runs() {
    let mut r = run_in("elwynn");
    let (shape, at, facing) = hogger_rushes(&mut r);
    let (from, to, width) = r.telegraphs[0].line_ends().unwrap();
    let Shape::Line { length, width: w } = shape else { unreachable!() };
    assert_eq!((from, width), (at, w));
    assert!((to.0 - (at.0 + facing.cos() * length)).abs() < 1e-4 && (to.1 - (at.1 + facing.sin() * length)).abs() < 1e-4);
    // a hero far off: the line stops at the rush's own length
    let mut r = run_in("elwynn");
    let k = boss_near(&mut r, "Hogger");
    let p = r.heroes[0].pos;
    r.enemies[k].segment = Segment::standing((p.0 + 40.0, p.1), r.now);
    let slot = rush_slot(&r);
    cast(&mut r, slot);
    assert_eq!(r.telegraphs[0].shape, Shape::Line { length: 25.0, width: 4.0 });
}

/// A boss's wind-up shows on its boss bar as a cast: Rhahk'Zor's slam, labelled as its timer bar,
/// for its 1.8 s wind-up; a roster add laying the same mark does not.
#[test]
fn a_boss_wind_up_is_told_as_its_cast() {
    let mut r = run_in("elwynn");
    let k = boss_near(&mut r, "Rhahk'Zor");
    let id = r.enemies[k].id;
    r.drain_events();
    cast(&mut r, 0);
    let casts: Vec<_> = r
        .drain_events()
        .into_iter()
        .filter_map(|e| match e {
            Event::BossCasting { boss, label, spell, seconds } => Some((boss, label, spell, seconds)),
            _ => None,
        })
        .collect();
    assert_eq!(casts.len(), 1, "one cast: {casts:?}");
    let (boss, label, spell, seconds) = &casts[0];
    assert_eq!(*boss, id);
    assert_eq!(*seconds, 1.8);
    assert!(!label.is_empty() && *spell != 0, "labelled and with an icon: {label:?} {spell}");
}
