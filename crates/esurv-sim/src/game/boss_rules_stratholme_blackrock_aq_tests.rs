//! The Stratholme, Blackrock Mountain and Ahn'Qiraj bosses' mechanics
//! (docs/game/bosses.md, "Stratholme, Blackrock Mountain and Ahn'Qiraj"), played on flat
//! ground: rage that stacks, beams and lines, drifting zones, the horde pulled into a clump, the
//! spell reflection, adds that heal, runs of waves the boss waits behind, the guards' counter,
//! Ragnaros rising for one smash, the class calls, the fallen rising, the crystal, the sweeping
//! glare and the tentacles. None of it holds, slows or fears the hero.

use std::sync::Arc;

use esurv_content::{BossAbilityKind, Content, MarkAt, Shape};

use super::casting::HitSource;
use super::enemy::RunOptions;
use super::enemy_ai::Segment;
use super::run::{EnemyKind, Event, Flat, HeroSetup, Run};

fn run_as(zone: &str, class: &str) -> Run {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
    let content = Arc::new(Content::load(dir).expect("content loads"));
    let hero = HeroSetup { class: class.into(), race: "orc".into(), ..Default::default() };
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

fn run_in(zone: &str) -> Run {
    run_as(zone, "warrior")
}

/// Boss `name` of the run's zone, brought in 3 yd from the hero, free to act at once; its index.
fn boss_near(r: &mut Run, name: &str) -> usize {
    let b = r.content.zones[r.zone].bosses.iter().position(|b| b.name == name).unwrap();
    r.spawn_boss(b);
    let k = r.enemies.iter().position(|e| e.kind == EnemyKind::Boss(b)).unwrap();
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

/// Runs the boss's timed script line `i` now.
fn run_timed_line(r: &mut Run, i: usize) {
    let k = boss_index(r);
    r.enemies[k].mech.next_at[i] = r.now;
    r.boss_script_tick(k);
}

fn named(r: &Run, name: &str) -> usize {
    r.enemies.iter().filter(|e| matches!(e.kind, EnemyKind::Roster(i) if r.content.zones[r.zone].enemies[i].name == name)).count()
}

fn hit(r: &mut Run, id: u32, amount: f32, source: HitSource) -> f32 {
    r.hit_enemy(None, id, amount, false, source).unwrap_or(0.0)
}

/// Kills every enemy that is not a boss (as the hero would).
fn kill_adds(r: &mut Run) {
    let ids: Vec<u32> = r.enemies.iter().filter(|e| !e.is_boss()).map(|e| e.id).collect();
    for id in ids {
        hit(r, id, 1e9, HitSource::WeaponSwing);
    }
}

// ---- Stratholme ---------------------------------------------------------------------------------

/// The living boss named `name`'s index.
fn boss_named(r: &Run, name: &str) -> Option<usize> {
    r.boss_unit(name)
}

/// Kills the living boss named `name` (as the hero would).
fn kill_boss(r: &mut Run, name: &str) {
    let id = r.enemies[boss_named(r, name).unwrap()].id;
    hit(r, id, 1e12, HitSource::WeaponSwing);
}

/// Barthilas: his Furious Anger stacks every 20 s up to five, his marks coming sooner and hitting
/// harder, his swing staying as it is; Mighty Blow is a marked cone, Crowd Pummel a 10 yd circle.
#[test]
fn barthilas_stacks_furious_anger_on_his_marks_not_his_swing() {
    let mut r = run_in("strat");
    let k = boss_near(&mut r, "Magistrate Barthilas");
    assert_eq!(r.enemies[k].mech.swing_mult, 1.5);
    assert!(matches!(r.enemies[k].abilities[0].ability.kind, BossAbilityKind::Marked { shape: Shape::Cone { .. }, at: MarkAt::Boss, .. }));
    assert!(matches!(r.enemies[k].abilities[1].ability.kind, BossAbilityKind::Marked { shape: Shape::Circle { radius }, at: MarkAt::Boss, .. } if radius == 10.0));
    for _ in 0..7 {
        run_timed_line(&mut r, 0);
    }
    let k = boss_index(&r);
    assert_eq!(r.enemies[k].mech.phases.rage.unwrap().stacks, 5, "five stacks at most");
    assert_eq!(r.enemies[k].mech.swing_mult, 1.5, "his swing stays");
    r.drain_events();
    cast(&mut r, 1);
    let attack = r.enemies[boss_index(&r)].damage;
    assert!((r.telegraphs[0].damage - attack * 3.0 * 1.5).abs() < 1e-2 * attack, "the pummel lands harder");
}

/// Anastari: Banshee Wail volleys, and her Possess a fade and a dive along a marked line through
/// the hero's spot; nothing takes hold of the hero.
#[test]
fn anastari_wails_in_volleys_and_dives_through_a_marked_line() {
    let mut r = run_in("strat");
    let k = boss_near(&mut r, "Baroness Anastari");
    assert!(matches!(r.enemies[k].abilities[0].ability.kind, BossAbilityKind::Volley { look_spell: 16565, .. }));
    let p = r.heroes[0].pos;
    r.enemies[k].segment = Segment::standing((p.0 + 10.0, p.1), r.now);
    r.drain_events();
    cast(&mut r, 1);
    let line = r.drain_events().iter().find_map(|e| match e {
        Event::TelegraphShown { shape: s @ Shape::Line { .. }, at, facing, .. } => Some((*s, *at, *facing)),
        _ => None,
    });
    let (shape, at, facing) = line.expect("her dive is marked");
    assert!(shape.contains(at, facing, p), "through the hero's spot");
    assert!(r.heroes[0].buffs.is_empty(), "no control of the hero");
}

/// Nerub'enkan: his blows stack Pierce Armor (damage only), his Encasing Webs are three marks round
/// the hero leaving webs, and every 20 s six Crypt Scarabs crawl out round him.
#[test]
fn nerubenkan_pierces_webs_and_raises_crypt_scarabs() {
    let mut r = run_in("strat");
    let k = boss_near(&mut r, "Nerub'enkan");
    for _ in 0..3 {
        r.enemies[k].swing_timer = 0.0;
        for s in &mut r.enemies[k].abilities {
            s.timer = 1e6;
        }
        r.enemies_tick(0.01);
    }
    assert_eq!(r.heroes[0].afflictions.first().map(|a| (a.spec.look_spell, a.stacks)), Some((6016, 3)));
    assert!(r.heroes[0].buffs.is_empty(), "no root, no slow");
    r.drain_events();
    cast(&mut r, 0);
    assert_eq!(r.telegraphs.len(), 3);
    r.now += 1.6;
    r.enemy_hits_tick();
    assert_eq!(r.ground_zones.len(), 3, "webs where they landed");
    run_timed_line(&mut r, 0);
    assert_eq!(named(&r, "Crypt Scarab"), 6);
}

/// Maleki: his Frostbolt flies at the hero's spot, his Ice Tomb is a circle on it (no stun); from
/// 60 % he drains life, healing himself on a timer.
#[test]
fn maleki_bolts_and_tombs_then_drains_life_below_sixty() {
    let mut r = run_in("strat");
    let k = boss_near(&mut r, "Maleki the Pallid");
    assert!(matches!(r.enemies[k].abilities[0].ability.kind, BossAbilityKind::Marked { at: MarkAt::Hero, missile: true, .. }));
    cast(&mut r, 1);
    let p = r.heroes[0].pos;
    assert!(r.telegraphs.iter().any(|t| matches!(t.shape, Shape::Circle { radius } if radius == 5.0) && t.at == p));
    set_health(&mut r, 50.0);
    let k = boss_index(&r);
    let before = r.enemies[k].health;
    run_timed_line(&mut r, 1);
    let k = boss_index(&r);
    assert!((r.enemies[k].health - before - 0.04 * r.enemies[k].max_health).abs() < 1.0, "a twenty-fifth back");
}

/// As Stratholme plays it: Ramstein comes at 20:00 (the Slaughterhouse
/// door opened just before) and the spawns stop; as he falls the Baron comes, apart and immune,
/// while 34 Mindless Undead pour in at the south gate, then his five Black Guard Sentries (60 s
/// later, or once all are dead) with his own door opening; with them dead he fights, his Unholy
/// Aura round him; his Raise Dead skeletons heal him unless killed; the run is won when he falls.
#[test]
fn ramstein_then_the_dead_then_the_black_guard_then_the_baron() {
    let mut r = run_in("strat");
    let zone = &r.content.zones[r.zone];
    let door = |name: &str| zone.doors.iter().position(|d| d.name == name).unwrap();
    let (slaughterhouse, baron_s) = (door("Slaughterhouse Door"), door("Baron's Door"));
    assert_eq!(zone.doors[slaughterhouse].opens_at_minute, 19.95, "open as Ramstein is due");
    assert!(zone.doors[baron_s].opens_at_minute.is_infinite(), "only his script opens it");
    let ramstein = zone.bosses.iter().position(|b| b.name == "Ramstein the Gorger").unwrap();
    assert_eq!(zone.bosses[ramstein].spawn_minute, 20.0);
    r.next_boss = zone.bosses.len();
    r.spawn_boss(ramstein);
    assert!(r.spawn_rules.stopped, "the streets fall quiet");
    let full = r.enemies[boss_named(&r, "Ramstein the Gorger").unwrap()].max_health;
    r.drain_events();
    kill_boss(&mut r, "Ramstein the Gorger");
    assert!(r.won_at.is_none(), "not won: the Baron comes");
    let k = boss_named(&r, "Baron Rivendare").expect("the Baron comes");
    assert!((r.enemies[k].max_health / full - 0.7 / 0.3).abs() < 0.05, "0.7 and 0.3 of the final slot");
    assert!(r.enemies[k].mech.phases.aloof() && r.enemies[k].mech.phases.immune);
    assert_eq!(named(&r, "Mindless Undead"), 34);
    let gate = (3974.0, -3395.0);
    assert!(r.enemies.iter().filter(|e| !e.is_boss()).all(|e| {
        let p = e.position(r.now);
        ((p.0 - gate.0).powi(2) + (p.1 - gate.1).powi(2)).sqrt() <= 5.5
    }), "in at the south gate");
    let id = r.enemies[k].id;
    assert_eq!(hit(&mut r, id, 500.0, HitSource::WeaponSwing), 0.0, "immune behind his waves");
    // 60 s on, whatever the dead do: the Black Guard, and his door opens behind them
    r.now += 60.5;
    let k = boss_named(&r, "Baron Rivendare").unwrap();
    r.waves_tick(k);
    assert_eq!(named(&r, "Black Guard Sentry"), 5);
    assert!(r.doors_open[baron_s], "his door opens");
    assert!(r.drain_events().contains(&Event::DoorOpened { door: baron_s }));
    // all dead: he fights, his aura round him
    kill_adds(&mut r);
    let k = boss_named(&r, "Baron Rivendare").unwrap();
    r.waves_tick(k);
    let k = boss_named(&r, "Baron Rivendare").unwrap();
    assert!(!r.enemies[k].mech.phases.aloof() && !r.enemies[k].mech.phases.immune);
    assert!(r.ground_zones.iter().any(|z| z.spec.follows && z.spec.look_spell == 17467), "his aura round him");
    // Raise Dead: six skeletons; those still up 12 s later heal him a hundredth each
    r.enemies[k].health = r.enemies[k].max_health * 0.5;
    r.enemies[k].mech.next_at[1] = r.now;
    r.boss_script_tick(k);
    assert_eq!(named(&r, "Mindless Skeleton"), 6);
    r.heroes[0].stats.max_health = 1e9;
    r.heroes[0].health = 1e9;
    let before = r.enemies[boss_named(&r, "Baron Rivendare").unwrap()].health;
    let mut pact = false;
    for _ in 0..125 {
        r.tick(0.1);
        r.take_first_offers();
        pact |= r.drain_events().iter().any(|e| matches!(e, Event::Emote { text, .. } if text.contains("give their lives")));
    }
    assert!(pact, "the skeletons give their lives to him");
    let k = boss_named(&r, "Baron Rivendare").unwrap();
    assert!(r.enemies[k].health > before, "the skeletons' pact heals him");
    kill_adds(&mut r);
    kill_boss(&mut r, "Baron Rivendare");
    assert!(r.won_at.is_some(), "won when the Baron falls");
    assert!(!r.spawn_rules.stopped, "the spawns come back");
}

/// Endless brings the final slot back as it came: Ramstein alone (the Baron only his death
/// calls), stopping the spawns again; the Baron's fall starts them again.
#[test]
fn endless_brings_ramstein_back_and_the_spawns_stop_until_the_baron_falls() {
    let mut r = run_in("strat");
    let zone = &r.content.zones[r.zone];
    let finals: Vec<&str> = zone.bosses.iter().filter(|b| b.is_final && !b.called).map(|b| b.name.as_str()).collect();
    assert_eq!(finals, ["Ramstein the Gorger"], "Endless brings only Ramstein");
    let ramstein = zone.bosses.iter().position(|b| b.name == "Ramstein the Gorger").unwrap();
    r.next_boss = zone.bosses.len();
    r.endless_since = Some(r.now);
    for round in 0..2 {
        r.spawn_boss(ramstein);
        assert!(r.spawn_rules.stopped, "round {round}: stopped for Ramstein");
        kill_boss(&mut r, "Ramstein the Gorger");
        assert!(r.spawn_rules.stopped, "round {round}: still stopped behind the Baron's waves");
        kill_adds(&mut r);
        let k = boss_named(&r, "Baron Rivendare").unwrap();
        r.now += 61.0;
        r.waves_tick(k);
        kill_adds(&mut r);
        let k = boss_named(&r, "Baron Rivendare").unwrap();
        r.waves_tick(k);
        kill_boss(&mut r, "Baron Rivendare");
        assert!(!r.spawn_rules.stopped, "round {round}: back after the Baron");
    }
}

// ---- Blackrock Mountain ----------------------------------------------------------------------

/// Angerforge: at 30 % (as the original) eight Reservists and two Medics; the Medics heal him
/// while they live.
#[test]
fn angerforge_calls_reservists_and_medics_who_heal_him() {
    let mut r = run_in("brm");
    boss_near(&mut r, "General Angerforge");
    set_health(&mut r, 29.0);
    assert_eq!((named(&r, "Anvilrage Reservist"), named(&r, "Anvilrage Medic")), (8, 2));
    let k = boss_index(&r);
    let before = r.enemies[k].health;
    r.drain_events();
    run_timed_line(&mut r, 1);
    let k = boss_index(&r);
    assert!((r.enemies[k].health - before - 0.02 * r.enemies[k].max_health).abs() < 1.0, "1 % for each Medic");
    assert_eq!(r.drain_events().iter().filter(|e| matches!(e, Event::SpellGo { spell: 15586, .. })).count(), 2);
    kill_adds(&mut r);
    let before = r.enemies[boss_index(&r)].health;
    run_timed_line(&mut r, 1);
    assert_eq!(r.enemies[boss_index(&r)].health, before, "no Medics, no heals");
}

/// Rend: the Spire's five waves first, Rend standing apart and immune, the next wave once one is
/// dead; then he comes down on Gyth and breathes flame; at 50 % he is knocked off and whirls.
#[test]
fn rend_waits_behind_the_spires_waves_then_rides_gyth_then_fights_on_foot() {
    let mut r = run_in("brm");
    boss_near(&mut r, "Warchief Rend Blackhand");
    let ev = r.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::Warning { text, .. } if text.contains("wave 1"))));
    assert!(ev.iter().any(|e| matches!(e, Event::BarStarted { label, .. } if label == "Next wave 2/5")));
    assert!(ev.iter().any(|e| matches!(e, Event::CounterShown { label, .. } if label == "Defenders left 6/6")));
    let k = boss_index(&r);
    let id = r.enemies[k].id;
    assert!(r.enemies[k].mech.phases.aloof() && r.now < r.enemies[k].mech.busy_until);
    assert_eq!(hit(&mut r, id, 500.0, HitSource::WeaponSwing), 0.0, "immune behind his waves");
    assert_eq!(named(&r, "Chromatic Whelp"), 5);
    for wave in 2..=5 {
        kill_adds(&mut r);
        let k = boss_index(&r);
        r.boss_script_tick(k);
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::Warning { text, .. } if text.contains(&format!("{wave}")) || text.contains("Last"))), "wave {wave}");
    }
    kill_adds(&mut r);
    let k = boss_index(&r);
    r.boss_script_tick(k);
    let k = boss_index(&r);
    assert!(!r.enemies[k].mech.phases.aloof() && r.enemies[k].mech.busy_until <= r.now, "he fights");
    assert_eq!(r.enemies[k].mech.display_id, Some(9723), "on Gyth");
    assert!(hit(&mut r, id, 500.0, HitSource::WeaponSwing) > 0.0);
    cast(&mut r, 0);
    assert!(matches!(r.telegraphs[0].shape, Shape::Cone { .. }) && r.telegraphs[0].leaves.is_some(), "flame breath leaves fire");
    set_health(&mut r, 49.0);
    let k = boss_index(&r);
    assert_eq!(r.enemies[k].mech.display_id, Some(9778), "on foot");
    assert!(matches!(&r.enemies[k].abilities[0].ability.kind, BossAbilityKind::Zone { zone, .. } if zone.follows));
}

/// A run of waves' next wave comes after its gap even with adds still standing.
#[test]
fn the_next_wave_comes_after_its_gap_anyway() {
    let mut r = run_in("aq");
    boss_near(&mut r, "General Rajaxx");
    assert_eq!(named(&r, "Captain Qeez"), 1);
    r.now += 20.1;
    let k = boss_index(&r);
    r.boss_script_tick(k);
    assert_eq!(named(&r, "Captain Tuubid"), 1);
    assert_eq!(named(&r, "Captain Qeez"), 1, "the first wave's still stand");
}

/// Thaurissan: Princess Moira comes with him and heals him every 6 s until she dies; his hammer
/// flies at the hero's spot, and Avatar of Flame is a ring.
#[test]
fn thaurissan_is_healed_by_moira_and_throws_his_hammer() {
    let mut r = run_in("brm");
    boss_near(&mut r, "Emperor Dagran Thaurissan");
    assert_eq!(named(&r, "Princess Moira Bronzebeard"), 1);
    let k = boss_index(&r);
    r.enemies[k].health *= 0.5;
    let before = r.enemies[k].health;
    run_timed_line(&mut r, 1);
    assert!(r.enemies[boss_index(&r)].health > before);
    cast(&mut r, 1);
    assert!(r.telegraphs[0].missile_at.is_some() && r.telegraphs[0].at == r.heroes[0].pos);
    cast(&mut r, 0);
    assert!(matches!(r.telegraphs[1].shape, Shape::Ring { .. }));
    kill_adds(&mut r);
    let before = r.enemies[boss_index(&r)].health;
    run_timed_line(&mut r, 1);
    assert_eq!(r.enemies[boss_index(&r)].health, before);
}

/// Majordomo: immune until all eight Flamewakers are dead (no damage shield on them), a "Guards
/// left" counter counting them down; beaten, he submits with his voice, and Ragnaros rises 4 s
/// later, immune and still, says "TOO SOON!" with his voice, brings down one marked Sulfuras
/// smash and sinks, giving nothing.
#[test]
fn majordomo_hides_behind_his_guards_and_ragnaros_rises_once() {
    let mut r = run_in("brm");
    boss_near(&mut r, "Majordomo Executus");
    let ev = r.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::CounterShown { label, .. } if label == "Guards left 8/8")));
    let id = r.enemies[boss_index(&r)].id;
    assert_eq!(hit(&mut r, id, 500.0, HitSource::WeaponSwing), 0.0, "immune");
    assert_eq!(named(&r, "Flamewaker Elite") + named(&r, "Flamewaker Healer"), 8);
    // a guard takes its hits whole and burns no one
    let guard = r.enemies.iter().find(|e| !e.is_boss()).unwrap().id;
    let health = r.heroes[0].health;
    hit(&mut r, guard, 1e9, HitSource::WeaponSwing);
    assert_eq!(r.heroes[0].health, health, "no damage shield");
    let k = boss_index(&r);
    r.boss_script_tick(k);
    assert!(r.drain_events().iter().any(|e| matches!(e, Event::CounterShown { label, .. } if label == "Guards left 7/8")));
    kill_adds(&mut r);
    let k = boss_index(&r);
    r.boss_script_tick(k);
    assert!(hit(&mut r, id, 500.0, HitSource::WeaponSwing) > 0.0, "vulnerable once they are dead");
    r.drain_events();
    hit(&mut r, id, 1e12, HitSource::WeaponSwing);
    let ev = r.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::Yell { text, .. } if text.contains("I submit"))));
    assert!(ev.iter().any(|e| matches!(e, Event::Sound(8038))));
    assert!(!ev.iter().any(|e| matches!(e, Event::Yell { text, .. } if text == super::lines::BOSS_DEATH_YELL)), "his own last words only");
    assert_eq!(named(&r, "Ragnaros"), 0, "not yet");
    r.now += 4.0;
    r.cameos_tick();
    let ev = r.drain_events();
    let rag = r.enemies.iter().position(|e| matches!(e.kind, EnemyKind::Roster(i) if r.content.zones[r.zone].enemies[i].name == "Ragnaros")).expect("Ragnaros rose");
    assert!(ev.iter().any(|e| matches!(e, Event::Sound(8043))), "TOO SOON, with his voice");
    assert!(ev.iter().any(|e| matches!(e, Event::Yell { text, .. } if text.starts_with("TOO SOON!"))));
    let rid = r.enemies[rag].id;
    assert_eq!(hit(&mut r, rid, 500.0, HitSource::WeaponSwing), 0.0, "he takes nothing");
    assert!(r.enemies[rag].mech.phases.rooted);
    // one marked smash at the hero, then he sinks, giving nothing
    let mut marks = 0;
    let mut died = false;
    for _ in 0..90 {
        r.tick(0.1);
        r.take_first_offers();
        let ev = r.drain_events();
        marks += ev.iter().filter(|e| matches!(e, Event::TelegraphShown { .. })).count();
        died |= ev.iter().any(|e| matches!(e, Event::EnemyDied { id, .. } if *id == rid));
    }
    assert_eq!(marks, 1, "one smash");
    assert!(!r.enemies.iter().any(|e| e.id == rid), "he sank");
    assert!(!died, "giving nothing");
}

/// Nefarian: phase 1 drakonid waves with him apart and immune, no class call meanwhile; he lands
/// with his voice; a class call twists a warrior (half damage for 10 s, no crowd control); at
/// 20 % the fallen drakonids rise as bone constructs.
#[test]
fn nefarian_lands_after_his_drakonids_calls_classes_and_raises_the_fallen() {
    let mut r = run_in("brm");
    boss_near(&mut r, "Nefarian");
    assert!(r.enemies[boss_index(&r)].mech.phases.aloof());
    // no class call while he stands apart
    r.drain_events();
    run_timed_line(&mut r, 1);
    assert!(r.heroes[0].afflictions.is_empty());
    let mut fallen = 0;
    for _ in 0..4 {
        fallen += r.enemies.iter().filter(|e| !e.is_boss()).count();
        kill_adds(&mut r);
        let k = boss_index(&r);
        r.boss_script_tick(k);
    }
    let ev = r.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::Sound(8288))), "he lands with his voice");
    let k = boss_index(&r);
    assert!(!r.enemies[k].mech.phases.aloof());
    assert_eq!(r.enemies[k].mech.phases.fallen.len(), fallen);
    // a class call: a warrior deals half damage for 10 s, and is held by nothing
    run_timed_line(&mut r, 1);
    assert!(r.heroes[0].afflictions.iter().any(|a| a.weakens == 0.5 && a.spec.look_spell == 23397));
    assert!(r.heroes[0].buffs.is_empty());
    assert!(r.drain_events().iter().any(|e| matches!(e, Event::Warning { text, .. } if text.contains("half damage"))));
    let id = r.enemies[boss_index(&r)].id;
    let dealt = r.hit_enemy(Some(0), id, 1000.0, false, HitSource::WeaponSwing).unwrap();
    assert!(dealt < 600.0, "{dealt}: halved");
    r.now += 10.1;
    let dealt = r.hit_enemy(Some(0), id, 1000.0, false, HitSource::WeaponSwing).unwrap();
    assert!(dealt >= 1000.0 * 0.95, "{dealt}: whole again");
    // 20 %: the fallen rise
    set_health(&mut r, 19.0);
    assert_eq!(named(&r, "Bone Construct"), fallen.min(16));
}

/// A warlock's class call brings infernals round it instead.
#[test]
fn nefarians_warlock_call_brings_infernals() {
    let mut r = run_as("brm", "warlock");
    boss_near(&mut r, "Nefarian");
    for _ in 0..4 {
        kill_adds(&mut r);
        let k = boss_index(&r);
        r.boss_script_tick(k);
    }
    run_timed_line(&mut r, 1);
    assert_eq!(named(&r, "Corrupted Infernal"), 3);
}

// ---- Ahn'Qiraj ---------------------------------------------------------------------------------

/// Kurinnaxx: a sand trap marked under the hero that stays a while; Mortal Wound stacks from his
/// blows; at 30 % he enrages.
#[test]
fn kurinnaxx_opens_sand_traps_and_wounds_and_enrages() {
    let mut r = run_in("aq");
    let k = boss_near(&mut r, "Kurinnaxx");
    cast(&mut r, 0);
    let p = r.heroes[0].pos;
    assert_eq!(r.telegraphs[0].at, p);
    r.now += 1.6;
    r.enemy_hits_tick();
    assert_eq!(r.ground_zones.len(), 1);
    let k = r.enemies.iter().position(|e| e.is_boss()).unwrap_or(k);
    r.enemies[k].swing_timer = 0.0;
    r.enemies[k].mech.busy_until = 0.0;
    for s in &mut r.enemies[k].abilities {
        s.timer = 1e6;
    }
    r.enemies_tick(0.01);
    assert!(r.heroes[0].afflictions.iter().any(|a| a.spec.look_spell == 25646));
    set_health(&mut r, 29.0);
    assert!(r.enemies[boss_index(&r)].enraged);
}

/// Rajaxx: seven waves, each led by its captain, a timer bar per wave and his rally lines with
/// his voice; he joins with "Impudent fool!" and his Thundercrash is a big marked circle.
#[test]
fn rajaxx_sends_seven_captains_then_joins() {
    let mut r = run_in("aq");
    boss_near(&mut r, "General Rajaxx");
    let mut sounds = Vec::new();
    let captains = ["Captain Qeez", "Captain Tuubid", "Captain Drenn", "Captain Xurrem", "Major Yeggeth", "Major Pakkon", "Colonel Zerran"];
    let mut bars = 0;
    for (n, captain) in captains.iter().enumerate() {
        assert_eq!(named(&r, captain), 1, "wave {} led by {captain}", n + 1);
        let ev = r.drain_events();
        bars += ev.iter().filter(|e| matches!(e, Event::BarStarted { key: super::boss_phases::WAVE_BAR_KEY, .. })).count();
        sounds.extend(ev.iter().filter_map(|e| if let Event::Sound(s) = e { Some(*s) } else { None }));
        kill_adds(&mut r);
        let k = boss_index(&r);
        r.boss_script_tick(k);
    }
    sounds.extend(r.drain_events().iter().filter_map(|e| if let Event::Sound(s) = e { Some(*s) } else { None }));
    assert_eq!(bars, 7, "a timer bar for each wave");
    assert_eq!(sounds, [8641, 8642, 8643, 8644, 8645, 8638], "his rallies, then he joins");
    let k = boss_index(&r);
    assert!(!r.enemies[k].mech.phases.aloof());
    cast(&mut r, 0);
    assert!(matches!(r.telegraphs[0].shape, Shape::Circle { radius } if radius >= 15.0));
}

/// Huhuran: a frenzy brings her clouds sooner for 8 s, never her volleys (the damage a hero
/// cannot avoid stays); at 30 % she goes berserk.
#[test]
fn huhurans_frenzy_hastens_her_clouds_not_her_volleys() {
    let mut r = run_in("aq");
    let k = boss_near(&mut r, "Princess Huhuran");
    run_timed_line(&mut r, 0);
    let k = r.enemies.iter().position(|e| e.is_boss()).unwrap_or(k);
    assert!((r.enemies[k].mech.phases.haste(r.now) - 1.5).abs() < 1e-4);
    for s in &mut r.enemies[k].abilities {
        s.timer = 100.0;
    }
    r.enemies[k].next_ability_at = 1e9;
    r.enemy_abilities(k, 0, 1.0, false);
    let timers: Vec<f32> = r.enemies[k].abilities.iter().map(|s| s.timer).collect();
    assert_eq!(timers, [99.0, 98.5], "the volley at its pace, the clouds sooner");
    r.now += 8.1;
    assert_eq!(r.enemies[k].mech.phases.haste(r.now), 1.0, "for 8 s");
    set_health(&mut r, 29.0);
    assert!(r.enemies[boss_index(&r)].enraged);
}

/// Ossirian: Supreme, a tenth of each hit, until the hero walks over the crystal: weak 30 s; his
/// tornadoes wander after the hero.
#[test]
fn ossirian_is_supreme_until_the_crystal_and_his_tornadoes_wander() {
    let mut r = run_in("aq");
    boss_near(&mut r, "Ossirian the Unscarred");
    let id = r.enemies[boss_index(&r)].id;
    assert!((hit(&mut r, id, 1000.0, HitSource::WeaponSwing) - 100.0).abs() < 1.0);
    assert_eq!(r.ground_objects.len(), 1);
    let (hero, at) = (r.heroes[0].id, r.ground_objects[0].at);
    r.hero_moved(hero, at, 0.0, 0.0);
    for _ in 0..6 {
        r.ground_objects_tick(0.1);
    }
    assert!((hit(&mut r, id, 1000.0, HitSource::WeaponSwing) - 1000.0).abs() < 1.0, "weak");
    r.now += 30.1;
    assert!((hit(&mut r, id, 1000.0, HitSource::WeaponSwing) - 100.0).abs() < 1.0, "Supreme again");
    cast(&mut r, 1);
    r.now += 1.3;
    r.enemy_hits_tick();
    assert!(r.ground_zones.iter().any(|z| z.spec.drift > 0.0));
}

/// C'Thun: Dark Glare lands as a beam that sweeps round him, hurting a hero in it and not one
/// behind it; Eye Beams chain, each mark landing after the one before; Eye Tentacles burst from
/// marked spots and stay there; at 50 % his body rises with claws and giants.
#[test]
fn cthuns_glare_sweeps_his_beams_chain_and_tentacles_burst() {
    let mut r = run_in("aq");
    boss_near(&mut r, "C'Thun");
    cast(&mut r, 0);
    let health = r.heroes[0].health;
    for _ in 0..26 {
        r.tick(0.1);
    }
    assert_eq!(r.beams.len(), 1, "the glare stays");
    assert!(r.heroes[0].health < health, "it caught the hero standing in it");
    let facing = r.beams[0].facing;
    for _ in 0..10 {
        r.tick(0.1);
    }
    assert!((r.beams[0].facing - facing - 25f32.to_radians() * 1.0).abs() < 0.05, "and turns 25 degrees a second");
    // a hero behind him takes nothing from it
    let k = boss_index(&r);
    let boss = r.enemies[k].position(r.now);
    let back = r.beams[0].facing + std::f32::consts::PI;
    let (id, spot) = (r.heroes[0].id, (boss.0 + back.cos() * 8.0, boss.1 + back.sin() * 8.0));
    r.hero_moved(id, spot, 0.0, 0.0);
    r.heroes[0].health = r.heroes[0].stats.max_health;
    r.beams[0].next_tick = r.now;
    r.beams_tick(0.0);
    assert_eq!(r.heroes[0].health, r.heroes[0].stats.max_health);
    r.now += 9.0;
    r.beams_tick(0.0);
    assert!(r.beams.is_empty(), "gone with its time");
    // Eye Beams chain
    r.telegraphs.clear();
    cast(&mut r, 1);
    let lands: Vec<f32> = r.telegraphs.iter().map(|t| t.lands_at).collect();
    assert_eq!(lands.len(), 4);
    assert!(lands.windows(2).all(|w| (w[1] - w[0] - 0.5).abs() < 1e-4));
    // Eye Tentacles from the marks, staying where they burst
    r.telegraphs.clear();
    cast(&mut r, 2);
    r.now += 2.1;
    r.enemy_hits_tick();
    assert_eq!(named(&r, "Eye Tentacle"), 3);
    assert!(r.enemies.iter().filter(|e| matches!(e.kind, EnemyKind::Roster(i) if r.content.zones[r.zone].enemies[i].name == "Eye Tentacle")).all(|e| e.mech.phases.rooted));
    set_health(&mut r, 49.0);
    let k = boss_index(&r);
    assert_eq!(r.enemies[k].mech.display_id, Some(15787));
    assert!(r.enemies[k].abilities.iter().all(|a| matches!(&a.ability.kind, BossAbilityKind::Marked { brings: Some(_), .. })));
}
