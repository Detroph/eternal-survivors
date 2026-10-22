//! The Scarlet Monastery, Sunken Temple, Dire Maul and Gurubashi Arena bosses' mechanics
//! (docs/game/bosses.md, "Scarlet Monastery to Gurubashi Arena"), played on
//! flat ground: each boss's own mechanic, its scripts' adds, shields, heals, revives and pauses,
//! and the damage a hero cannot avoid.

use std::sync::Arc;

use esurv_content::{BossAbility, BossAbilityEntry, BossAbilityKind, Content, School, Shape};

use super::boss_damage::{avoidable_per_second, unavoidable_per_second};
use super::casting::HitSource;
use super::enemy::RunOptions;
use super::enemy_ai::Segment;
use super::run::{EnemyKind, Event, Flat, HeroSetup, Run, Unit};

fn content() -> Arc<Content> {
    Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).expect("content loads"))
}

fn run_in(zone: &str) -> Run {
    let hero = HeroSetup { class: "warrior".into(), race: "orc".into(), ..Default::default() };
    let mut r = Run::new(content(), zone, RunOptions::default(), &[hero], Box::new(Flat), 9).unwrap();
    let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
    r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
    r.enemies.clear();
    r.pickups.clear();
    r.heroes[0].spells.known.clear();
    r.heroes[0].spells.racial = None;
    // a sturdy hero, so the fights can be watched to their end
    r.heroes[0].stats.max_health = 1e6;
    r.heroes[0].health = 1e6;
    r.drain_events();
    r
}

/// Boss `name` of the run's zone, brought in 3 yd from the hero, free to act at once; its index.
fn boss_near(r: &mut Run, name: &str) -> usize {
    let b = r.content.zones[r.zone].bosses.iter().position(|b| b.name == name).unwrap();
    r.spawn_boss(b);
    let k = r.enemies.iter().rposition(|e| e.kind == EnemyKind::Boss(b)).unwrap();
    let p = r.heroes[0].pos;
    r.enemies[k].segment = Segment::standing((p.0 + 3.0, p.1), r.now);
    r.enemies[k].hold_until = 0.0;
    r.enemies[k].next_ability_at = 0.0;
    k
}

/// The living boss named `name`'s index.
fn boss(r: &Run, name: &str) -> Option<usize> {
    r.boss_unit(name)
}

fn id_of(r: &Run, name: &str) -> u32 {
    r.enemies[boss(r, name).unwrap()].id
}

/// Boss `name` casts its ability `slot` now.
fn cast(r: &mut Run, name: &str, slot: usize) {
    let k = boss(r, name).unwrap();
    for (i, s) in r.enemies[k].abilities.iter_mut().enumerate() {
        s.timer = if i == slot { 0.0 } else { 1e6 };
    }
    r.enemies[k].next_ability_at = 0.0;
    r.enemies[k].mech.busy_until = 0.0;
    r.enemy_abilities(k, 0, 0.0, false);
}

fn set_health(r: &mut Run, name: &str, pct: f32) {
    let k = boss(r, name).unwrap();
    r.enemies[k].health = r.enemies[k].max_health * pct / 100.0;
    r.boss_script_tick(k);
}

/// Boss `name`'s timed script line `line` comes due now.
fn timer_due(r: &mut Run, name: &str, line: usize) {
    let k = boss(r, name).unwrap();
    r.enemies[k].mech.next_at[line] = r.now;
    r.boss_script_tick(k);
}

fn hit(r: &mut Run, id: u32, amount: f32) {
    r.hit_enemy(None, id, amount, false, HitSource::Proc { spell: 1, school: School::Holy, origin: Default::default() });
}

fn kill(r: &mut Run, name: &str) {
    let id = id_of(r, name);
    hit(r, id, 1e12);
}

fn roster_count(r: &Run, name: &str) -> usize {
    let zone = &r.content.zones[r.zone];
    r.enemies.iter().filter(|e| matches!(e.kind, EnemyKind::Roster(i) if zone.enemies[i].name == name)).count()
}

fn ticks(r: &mut Run, n: usize) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..n {
        r.tick(0.1);
        r.take_first_offers();
        out.extend(r.drain_events());
    }
    out
}

/// The zones reworked so far.
const REWORKED: [&str; 4] = ["scarlet", "sunken_temple", "feralas", "arena"];

/// Each reworked boss's kit before the rework (release v0.2.2), as its zone file wrote it.
const BEFORE: [(&str, &str); 21] = [
    ("Houndmaster Loksey", "[Charge(look_spell: 100), Summon()]"),
    ("Arcanist Doan", "[Volley(look_spell: 133), GroundBurst(telegraph_spell: 5740, impact_spell: 2120), Summon()]"),
    ("Herod", "[Stomp(look_spell: 1680), Charge(look_spell: 100), Summon()]"),
    ("High Inquisitor Fairbanks", "[Volley(look_spell: 686), Stomp(look_spell: 8122), Summon()]"),
    ("Scarlet Commander Mograine", "[Charge(look_spell: 100), Stomp(look_spell: 6343), Summon(count: Some(6))]"),
    ("High Inquisitor Whitemane", "[Volley(look_spell: 686), GroundBurst(telegraph_spell: 5740, impact_spell: 2120), Summon(count: Some(6))]"),
    ("Atal'alarion", "[Stomp(look_spell: 20549), Charge(look_spell: 100), Summon()]"),
    ("Jammal'an the Prophet", "[Volley(look_spell: 403), GroundBurst(telegraph_spell: 16914, impact_spell: 339), Summon()]"),
    ("Morphaz", "[Charge(look_spell: 100), Stomp(look_spell: 1680), Summon()]"),
    ("Avatar of Hakkar", "[Volley(look_spell: 686), GroundBurst(telegraph_spell: 16914, impact_spell: 20549), Summon()]"),
    ("Shade of Eranikus", "[Volley(look_spell: 5176), Stomp(look_spell: 6343), GroundBurst(telegraph_spell: 5740, impact_spell: 2120), Summon()]"),
    ("Zevrim Thornhoof", "[Volley(look_spell: 686), Summon()]"),
    ("Alzzin the Wildshaper", "[GroundBurst(telegraph_spell: 16914, impact_spell: 339), Volley(look_spell: 5176), Summon()]"),
    ("Tendris Warpwood", "[Stomp(look_spell: 20549), GroundBurst(telegraph_spell: 16914, impact_spell: 339), Summon()]"),
    ("Immol'thar", "[Volley(look_spell: 686), Stomp(look_spell: 1449), Charge(look_spell: 100), Summon()]"),
    ("King Gordok", "[Charge(look_spell: 100), Stomp(look_spell: 20549), Volley(look_spell: 133), Summon()]"),
    ("High Priestess Jeklik", "[Volley(look_spell: 133), GroundBurst(telegraph_spell: 5740, impact_spell: 2120), Summon()]"),
    ("High Priest Venoxis", "[Volley(look_spell: 403), Stomp(look_spell: 2812), Summon()]"),
    ("Bloodlord Mandokir", "[Charge(look_spell: 100), Stomp(look_spell: 1680), Summon()]"),
    ("Jin'do the Hexxer", "[Volley(look_spell: 686), GroundBurst(telegraph_spell: 26573, impact_spell: 1449), Summon()]"),
    ("Hakkar the Soulflayer", "[Volley(look_spell: 686), GroundBurst(telegraph_spell: 16914, impact_spell: 1449), Stomp(look_spell: 8122), Summon()]"),
];

/// Bosses new with the rework that fight beside one of the old ones: what a hero cannot avoid
/// from both together stays the old boss's (Ohgan beside Mandokir); Cho'Rush, a caster, never
/// swings.
const BESIDE: [(&str, &str); 2] = [("Ohgan", "Bloodlord Mandokir"), ("Cho'Rush the Observer", "King Gordok")];

/// Every kit a boss fights with: its own and each phase's.
fn kits(b: &esurv_content::Boss) -> Vec<Vec<BossAbility>> {
    let mut kits = vec![b.abilities.clone()];
    for line in &b.script {
        for a in &line.run {
            if let esurv_content::ScriptAction::Kit { resolved, .. } = a {
                kits.push(resolved.clone());
            }
        }
    }
    kits
}

/// Every reworked boss of these zones takes from a hero what it cannot
/// avoid as it did before, within 3 %, in every phase of its kit (a boss fighting beside it
/// counted with it); what a hero who avoids nothing takes on top is more, within twice that.
#[test]
fn a_reworked_boss_keeps_the_damage_a_hero_cannot_avoid() {
    let content = content();
    let kinds = content.enemy_kinds.as_ref().unwrap();
    let mut checked = 0;
    for (name, before) in BEFORE {
        let (zone, b) = content.zones.iter().find_map(|z| z.bosses.iter().position(|b| b.name == name).map(|b| (z, b))).unwrap();
        if !REWORKED.contains(&zone.id.as_str()) {
            continue;
        }
        let entries: Vec<BossAbilityEntry> = ron::from_str(before).unwrap();
        let mut problems = Vec::new();
        // its place among the bosses of before (the old Summon took its adds by it)
        let place = zone.bosses[..b].iter().filter(|x| BEFORE.iter().any(|(n, _)| *n == x.name)).count();
        let old: Vec<BossAbility> = entries.iter().map(|e| esurv_content::resolve_ability_for_tests(&zone.id, name, place, e, kinds, &mut problems)).collect();
        assert!(problems.is_empty(), "{problems:?}");
        let then = unavoidable_per_second(&old, 1.0, None);
        let boss = &zone.bosses[b];
        // a boss beside it: its unavoidable part counted with this one's
        let beside: f32 = BESIDE
            .iter()
            .filter(|(_, with)| *with == name)
            .map(|(other, _)| zone.bosses.iter().find(|x| x.name == *other).unwrap())
            .map(|o| kits(o).iter().map(|k| unavoidable_per_second(k, o.swing_mult, o.on_hit)).fold(0.0, f32::max))
            .sum();
        for kit in kits(boss) {
            let now = unavoidable_per_second(&kit, boss.swing_mult, boss.on_hit) + beside;
            assert!((now - then).abs() <= then * 0.03, "{name}: unavoidable {now:.3} a second, was {then:.3}");
            let on_top = avoidable_per_second(&kit);
            assert!(on_top > 0.0 && on_top <= 2.0 * then, "{name}: {on_top:.3} on top of {then:.3}");
        }
        checked += 1;
    }
    assert_eq!(checked, BEFORE.iter().filter(|(n, _)| content.zones.iter().any(|z| REWORKED.contains(&z.id.as_str()) && z.bosses.iter().any(|b| b.name == *n))).count());
    assert!(checked > 0);
}

/// No boss of these zones casts anything a hero must interrupt, and no
/// ability of theirs is one of the old unmarked kinds.
#[test]
fn every_area_attack_of_these_zones_is_marked() {
    let content = content();
    for z in content.zones.iter().filter(|z| REWORKED.contains(&z.id.as_str())) {
        for b in &z.bosses {
            for a in kits(b).iter().flatten() {
                assert!(super::telegraphs::is_building_block(&a.kind), "{} {}: {:?}", z.id, b.name, a.kind);
            }
        }
    }
}

// ---- Scarlet Monastery -------------------------------------------------------------------------

/// Loksey: the hounds' four lanes are marked from far-apart sides, the packs run in from them 2 s
/// later, and then he gives them Bloodlust (faster, harder).
#[test]
fn loksey_s_hounds_come_from_four_marked_sides_and_take_bloodlust() {
    let mut r = run_in("scarlet");
    boss_near(&mut r, "Houndmaster Loksey");
    let ev = r.drain_events();
    let lanes: Vec<f32> = ev
        .iter()
        .filter_map(|e| match e {
            Event::TelegraphShown { shape: Shape::Line { .. }, facing, wind_up, .. } if *wind_up == super::boss_script_actions::LANE_WARNING => Some(*facing),
            _ => None,
        })
        .collect();
    assert_eq!(lanes.len(), 4, "four lanes");
    assert!(ev.iter().any(|e| matches!(e, Event::Warning { colour: esurv_content::WarningColour::Orange, .. })));
    assert_eq!(roster_count(&r, "Scarlet Tracking Hound"), 0, "not yet");
    let health = r.heroes[0].health;
    ticks(&mut r, 21);
    assert_eq!(roster_count(&r, "Scarlet Tracking Hound"), 8, "the packs ran in");
    let zone = r.content.zones[r.zone].clone();
    let hound = r.enemies.iter().find(|e| matches!(e.kind, EnemyKind::Roster(i) if zone.enemies[i].name == "Scarlet Tracking Hound")).unwrap().clone();
    assert!(r.heroes[0].health >= health - 1e3, "the lanes hurt no one");
    ticks(&mut r, 5);
    let after = r.enemies.iter().find(|e| e.id == hound.id).unwrap();
    assert!((after.damage / hound.damage - 1.3).abs() < 1e-3 && (after.base_speed / hound.base_speed - 1.3).abs() < 1e-3, "faster and harder");
    assert!(after.kit.worn_looks.iter().any(|(l, _)| *l == 6742));
}

/// Doan: at half health he bubbles (immune) and Detonation is marked round him, 15 yd for 2.5 s;
/// the bubble goes once it has gone off.
#[test]
fn doan_bubbles_and_detonates_at_half_health() {
    let mut r = run_in("scarlet");
    boss_near(&mut r, "Arcanist Doan");
    r.drain_events();
    set_health(&mut r, "Arcanist Doan", 54.0);
    assert!(r.drain_events().iter().any(|e| matches!(e, Event::Warning { soon: true, .. })));
    set_health(&mut r, "Arcanist Doan", 49.0);
    let ev = r.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::TelegraphShown { shape: Shape::Circle { radius }, wind_up, .. } if *radius == 15.0 && *wind_up == 2.5)));
    assert!(ev.iter().any(|e| matches!(e, Event::Sound(5843))), "his own line");
    let (id, before) = (id_of(&r, "Arcanist Doan"), r.enemies[boss(&r, "Arcanist Doan").unwrap()].health);
    hit(&mut r, id, 1000.0);
    assert!(before - r.enemies[boss(&r, "Arcanist Doan").unwrap()].health <= 1.0, "immune in his bubble");
    // the hero runs clear, and the bubble goes after it went off
    let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
    r.hero_moved(h, (p.0 - 20.0, p.1), 0.0, 0.0);
    let health = r.heroes[0].health;
    ticks(&mut r, 27);
    assert_eq!(r.heroes[0].health, health, "out of it: nothing");
    let before = r.enemies[boss(&r, "Arcanist Doan").unwrap()].health;
    hit(&mut r, id, 1000.0);
    assert!(before - r.enemies[boss(&r, "Arcanist Doan").unwrap()].health > 900.0, "bubble gone");
}

/// Herod: his Whirlwind is a circle that spins with him ("Blades of light!"); at 25 % he
/// enrages with "Light, give me strength!"; as he falls twenty trainees pour in.
#[test]
fn herod_whirls_enrages_and_his_trainees_pour_in() {
    let mut r = run_in("scarlet");
    boss_near(&mut r, "Herod");
    r.drain_events();
    timer_due(&mut r, "Herod", 0);
    let ev = r.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::Sound(5832))));
    assert!(r.ground_zones.iter().any(|z| z.spec.follows && z.spec.radius == 7.0), "the circle goes with him");
    assert!(r.enemies[boss(&r, "Herod").unwrap()].kit.worn_looks.iter().any(|(l, _)| *l == 8989), "spinning");
    set_health(&mut r, "Herod", 24.0);
    assert!(r.drain_events().iter().any(|e| matches!(e, Event::Sound(5833))));
    assert!(r.enemies[boss(&r, "Herod").unwrap()].enraged);
    kill(&mut r, "Herod");
    let ev = r.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::Sound(5834))));
    assert_eq!(roster_count(&r, "Scarlet Trainee"), 20);
}

/// Fairbanks: green beams heal the enemies near him, and he shields himself now and then.
#[test]
fn fairbanks_heals_the_enemies_near_him_with_beams_and_shields_himself() {
    let mut r = run_in("scarlet");
    let k = boss_near(&mut r, "High Inquisitor Fairbanks");
    let at = r.enemies[k].position(r.now);
    assert!(r.spawn_roster_enemy(0, Some((at.0 + 4.0, at.1))));
    let e = r.enemies.len() - 1;
    r.enemies[e].health = r.enemies[e].max_health * 0.5;
    let (add, before) = (r.enemies[e].id, r.enemies[e].health);
    timer_due(&mut r, "High Inquisitor Fairbanks", 1);
    let healed = r.enemies.iter().find(|x| x.id == add).unwrap().health;
    assert!(healed > before, "healed");
    let me = id_of(&r, "High Inquisitor Fairbanks");
    assert!(r.extras.beams.iter().any(|b| b.caster == me && b.target == Unit::Enemy(add) && b.spell == 13540), "with a green beam");
    timer_due(&mut r, "High Inquisitor Fairbanks", 0);
    let k = boss(&r, "High Inquisitor Fairbanks").unwrap();
    assert_eq!(r.enemies[k].mech.shield.map(|s| s.0), Some(0.3));
    r.later_tick(6.1);
    assert!(r.enemies[k].mech.shield.is_none(), "for 6 s");
}

/// Mograine fights alone and falls; Whitemane comes, the fight freezes (the scripted pause) and
/// she raises him with "Arise, my champion!", at half his health; then she shields and heals him.
/// The run is won when both have fallen, and his second fall brings no one.
#[test]
fn mograine_falls_and_whitemane_raises_him_in_the_scripted_pause() {
    let mut r = run_in("scarlet");
    r.next_boss = r.content.zones[r.zone].bosses.len();
    boss_near(&mut r, "Scarlet Commander Mograine");
    assert!(boss(&r, "High Inquisitor Whitemane").is_none(), "he fights alone");
    let full = r.enemies[boss(&r, "Scarlet Commander Mograine").unwrap()].max_health;
    r.drain_events();
    kill(&mut r, "Scarlet Commander Mograine");
    assert!(r.won_at.is_none(), "not won");
    assert!(boss(&r, "High Inquisitor Whitemane").is_some(), "she comes");
    let ev = r.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::Sound(5838))));
    assert!(ev.iter().any(|e| matches!(e, Event::PauseBegan { seconds, .. } if *seconds == 5.0)));
    assert!(r.in_cutscene());
    let ev = ticks(&mut r, 31);
    assert!(r.in_cutscene(), "still frozen as she raises him");
    assert!(ev.iter().any(|e| matches!(e, Event::Yell { text, .. } if text == "Arise, my champion!")));
    assert!(ev.iter().any(|e| matches!(e, Event::Sound(5840))));
    let k = boss(&r, "Scarlet Commander Mograine").expect("raised");
    assert!((r.enemies[k].health - full * 0.5).abs() < 1.0, "at half his health");
    let ev = ticks(&mut r, 30);
    assert!(!r.in_cutscene());
    assert!(ev.iter().any(|e| matches!(e, Event::Sound(5837))), "At your side, milady!");
    // she shields him and heals him with a beam
    timer_due(&mut r, "High Inquisitor Whitemane", 1);
    let k = boss(&r, "Scarlet Commander Mograine").unwrap();
    assert_eq!(r.enemies[k].mech.shield.map(|s| s.0), Some(0.3));
    r.enemies[k].health = full * 0.1;
    timer_due(&mut r, "High Inquisitor Whitemane", 2);
    let k = boss(&r, "Scarlet Commander Mograine").unwrap();
    assert!(r.enemies[k].health > full * 0.1);
    let her = id_of(&r, "High Inquisitor Whitemane");
    assert!(r.extras.beams.iter().any(|b| b.caster == her));
    kill(&mut r, "High Inquisitor Whitemane");
    assert!(r.won_at.is_none());
    kill(&mut r, "Scarlet Commander Mograine");
    assert!(boss(&r, "High Inquisitor Whitemane").is_none(), "his second fall brings no one");
    assert!(r.won_at.is_some(), "won when both have fallen");
}

// ---- Sunken Temple -----------------------------------------------------------------------------

/// The hero walks onto object `k` and stands there half a second.
fn walk_over(r: &mut Run, k: usize) {
    let (id, at) = (r.heroes[0].id, r.ground_objects[k].at);
    r.hero_moved(id, at, 0.0, 0.0);
    for _ in 0..6 {
        r.ground_objects_tick(0.1);
    }
}

/// Atal'alarion: Sweeping Slam is a marked cone before him, Ground Tremor a ring round him.
#[test]
fn atal_alarion_slams_a_cone_and_cracks_a_ring() {
    let mut r = run_in("sunken_temple");
    boss_near(&mut r, "Atal'alarion");
    r.drain_events();
    cast(&mut r, "Atal'alarion", 0);
    assert!(r.drain_events().iter().any(|e| matches!(e, Event::TelegraphShown { shape: Shape::Cone { .. }, .. })));
    cast(&mut r, "Atal'alarion", 1);
    assert!(r.drain_events().iter().any(|e| matches!(e, Event::TelegraphShown { shape: Shape::Ring { inner, .. }, wind_up, .. } if *inner == 5.0 && *wind_up == 1.8)));
}

/// Jammal'an: his healing totem heals the enemies round it until the hero walks over it; his
/// Earthgrab Totem lays a marked ring round itself; Flamestrike leaves fire.
#[test]
fn jammal_an_s_totems_heal_and_ring_until_broken() {
    let mut r = run_in("sunken_temple");
    let k = boss_near(&mut r, "Jammal'an the Prophet");
    let id = r.enemies[k].id;
    cast(&mut r, "Jammal'an the Prophet", 1);
    assert_eq!(r.ground_objects.len(), 1);
    let at = r.ground_objects[0].at;
    assert!(r.spawn_roster_enemy(0, Some((at.0 + 2.0, at.1))));
    let e = r.enemies.len() - 1;
    r.enemies[e].health = r.enemies[e].max_health * 0.5;
    let (add, before) = (r.enemies[e].id, r.enemies[e].health);
    r.now += 3.0;
    r.ground_objects_tick(0.0);
    assert!(r.enemies.iter().find(|x| x.id == add).unwrap().health > before, "the totem heals round it");
    walk_over(&mut r, 0);
    assert!(r.ground_objects.is_empty(), "broken");
    // the Earthgrab Totem rings itself
    cast(&mut r, "Jammal'an the Prophet", 2);
    let at = r.ground_objects[0].at;
    r.drain_events();
    r.now += 4.0;
    r.ground_objects_tick(0.0);
    let ev = r.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::TelegraphShown { shape: Shape::Ring { .. }, at: c, .. } if *c == at)), "a ring round the totem");
    assert!(r.enemies.iter().any(|e| e.id == id));
    // Flamestrike leaves fire where it lands
    r.telegraphs.clear();
    cast(&mut r, "Jammal'an the Prophet", 0);
    r.now += 1.6;
    r.enemy_hits_tick();
    assert!(r.ground_zones.iter().any(|z| z.spec.look_spell == 12468));
}

/// Morphaz: Acid Breath, a marked cone, leaves acid pools along it.
#[test]
fn morphaz_s_acid_breath_leaves_pools() {
    let mut r = run_in("sunken_temple");
    boss_near(&mut r, "Morphaz");
    cast(&mut r, "Morphaz", 0);
    r.now += 1.6;
    r.enemy_hits_tick();
    assert!(r.ground_zones.len() >= 2, "pools along the cone");
}

/// The Avatar of Hakkar is shielded while the four blood fires burn; each walked onto goes out,
/// and with the fourth his shield falls.
#[test]
fn the_avatar_is_shielded_until_the_four_blood_fires_are_out() {
    let mut r = run_in("sunken_temple");
    boss_near(&mut r, "Avatar of Hakkar");
    assert_eq!(r.ground_objects.len(), 4, "four fires");
    let id = id_of(&r, "Avatar of Hakkar");
    let before = r.enemies[boss(&r, "Avatar of Hakkar").unwrap()].health;
    hit(&mut r, id, 1000.0);
    assert!(before - r.enemies[boss(&r, "Avatar of Hakkar").unwrap()].health <= 1.0, "shielded");
    for n in (1..4).rev() {
        walk_over(&mut r, 0);
        assert_eq!(r.ground_objects.len(), n);
        assert!(r.enemies[boss(&r, "Avatar of Hakkar").unwrap()].mech.shield.is_some(), "still shielded");
    }
    walk_over(&mut r, 0);
    assert!(r.ground_objects.is_empty());
    assert!(r.enemies[boss(&r, "Avatar of Hakkar").unwrap()].mech.shield.is_none(), "the shield falls");
    let before = r.enemies[boss(&r, "Avatar of Hakkar").unwrap()].health;
    hit(&mut r, id, 1000.0);
    assert!(before - r.enemies[boss(&r, "Avatar of Hakkar").unwrap()].health > 900.0);
}

/// The Shade of Eranikus: Nightmare Suppressors come and heal him with beams; his Nightmare
/// clouds drift after the hero.
#[test]
fn eranikus_s_suppressors_heal_him_and_his_clouds_drift() {
    let mut r = run_in("sunken_temple");
    boss_near(&mut r, "Shade of Eranikus");
    timer_due(&mut r, "Shade of Eranikus", 0);
    assert_eq!(roster_count(&r, "Nightmare Suppressor"), 3);
    let k = boss(&r, "Shade of Eranikus").unwrap();
    r.enemies[k].health = r.enemies[k].max_health * 0.5;
    let before = r.enemies[k].health;
    timer_due(&mut r, "Shade of Eranikus", 1);
    let k = boss(&r, "Shade of Eranikus").unwrap();
    assert!((r.enemies[k].health - before - 0.03 * r.enemies[k].max_health).abs() < 1.0, "1 % from each of three");
    let me = r.enemies[k].id;
    assert_eq!(r.extras.beams.iter().filter(|b| b.target == Unit::Enemy(me) && b.spell == 12623).count(), 3, "a beam from each");
    // a cloud drifts after the hero
    cast(&mut r, "Shade of Eranikus", 1);
    let z = r.ground_zones.iter().find(|z| z.spec.drift > 0.0).unwrap().clone();
    let hero = r.heroes[0].pos;
    let d = |a: (f32, f32)| ((a.0 - hero.0).powi(2) + (a.1 - hero.1).powi(2)).sqrt();
    r.ground_zones_tick(1.0);
    let moved = r.ground_zones.iter().find(|x| x.id == z.id).unwrap();
    assert!(d(moved.at) < d(z.at) || d(z.at) < 0.2);
}

// ---- Dire Maul ---------------------------------------------------------------------------------

/// Zevrim: an altar laid by him heals him when left alone; walked over in time, it does not.
#[test]
fn zevrim_s_sacrifice_heals_him_unless_the_hero_stands_on_its_altar() {
    let mut r = run_in("feralas");
    boss_near(&mut r, "Zevrim Thornhoof");
    let k = boss(&r, "Zevrim Thornhoof").unwrap();
    r.enemies[k].health = r.enemies[k].max_health * 0.5;
    cast(&mut r, "Zevrim Thornhoof", 1);
    assert_eq!(r.ground_objects.len(), 1);
    walk_over(&mut r, 0);
    assert!(r.ground_objects.is_empty(), "stood on in time");
    r.now += 9.0;
    r.ground_objects_tick(0.0);
    let k = boss(&r, "Zevrim Thornhoof").unwrap();
    assert_eq!(r.enemies[k].health, r.enemies[k].max_health * 0.5, "no sacrifice");
    // left alone: he heals a tenth
    cast(&mut r, "Zevrim Thornhoof", 1);
    let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
    r.hero_moved(h, (p.0 + 30.0, p.1), 0.0, 0.0);
    r.now += 8.1;
    r.ground_objects_tick(0.1);
    let k = boss(&r, "Zevrim Thornhoof").unwrap();
    assert!((r.enemies[k].health - r.enemies[k].max_health * 0.6).abs() < 1.0, "healed by the sacrifice");
}

/// Alzzin: tree form with a withering aura; at 66 % a fast wolf with charges (the aura gone); at
/// 33 % his satyr shape with shadow volleys.
#[test]
fn alzzin_shifts_tree_wolf_satyr() {
    let mut r = run_in("feralas");
    boss_near(&mut r, "Alzzin the Wildshaper");
    let k = boss(&r, "Alzzin the Wildshaper").unwrap();
    assert_eq!(r.enemies[k].mech.display_id, Some(13173), "tree form");
    assert_eq!(r.ground_zones.iter().filter(|z| z.spec.follows).count(), 1, "its aura");
    let speed = r.enemies[k].base_speed;
    set_health(&mut r, "Alzzin the Wildshaper", 65.0);
    let k = boss(&r, "Alzzin the Wildshaper").unwrap();
    assert_eq!(r.enemies[k].mech.display_id, Some(2352), "wolf form");
    assert!(r.ground_zones.is_empty(), "the aura went with the tree");
    assert!((r.enemies[k].base_speed / speed - 1.5).abs() < 1e-3, "fast");
    assert!(matches!(r.enemies[k].abilities[0].ability.kind, BossAbilityKind::Rush { .. }));
    set_health(&mut r, "Alzzin the Wildshaper", 32.0);
    let k = boss(&r, "Alzzin the Wildshaper").unwrap();
    assert_eq!(r.enemies[k].mech.display_id, Some(14416));
    assert!((r.enemies[k].base_speed - speed).abs() < 1e-3);
    assert!(matches!(r.enemies[k].abilities[0].ability.kind, BossAbilityKind::Marked { at: esurv_content::MarkAt::NearHero { count: 4, .. }, .. }));
}

/// Tendris: seeds land at marked spots and sprout Ironbark Protectors unless walked over.
#[test]
fn tendris_s_seeds_sprout_protectors_unless_stomped() {
    let mut r = run_in("feralas");
    boss_near(&mut r, "Tendris Warpwood");
    cast(&mut r, "Tendris Warpwood", 0);
    r.now += 1.6;
    r.enemy_hits_tick();
    assert_eq!(r.ground_objects.len(), 2, "two seeds");
    walk_over(&mut r, 0);
    let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
    r.hero_moved(h, (p.0 + 40.0, p.1), 0.0, 0.0);
    r.now += 8.1;
    r.ground_objects_tick(0.1);
    assert_eq!(roster_count(&r, "Ironbark Protector"), 1, "the other sprouted");
}

/// Immol'thar: portals bring Eyes, and each Eye fires a marked laser line at the hero; with no
/// Eye up, none is fired.
#[test]
fn immol_thar_s_eyes_fire_marked_lines() {
    let mut r = run_in("feralas");
    boss_near(&mut r, "Immol'thar");
    assert_eq!(roster_count(&r, "Eye of Immol'thar"), 2, "two Eyes as he comes");
    r.drain_events();
    cast(&mut r, "Immol'thar", 1);
    let zone = r.content.zones[r.zone].clone();
    let eyes: Vec<u32> = r.enemies.iter().filter(|e| matches!(e.kind, EnemyKind::Roster(i) if zone.enemies[i].name == "Eye of Immol'thar")).map(|e| e.id).collect();
    let lines = r.telegraphs.iter().filter(|t| matches!(t.shape, Shape::Line { .. }) && eyes.contains(&t.source)).count();
    assert_eq!(lines, 2, "one from each Eye");
    r.telegraphs.clear();
    r.enemies.retain(|e| e.is_boss());
    r.drain_events();
    cast(&mut r, "Immol'thar", 1);
    assert!(r.telegraphs.is_empty() && !r.drain_events().iter().any(|e| matches!(e, Event::Warning { .. })), "no Eye: nothing");
    timer_due(&mut r, "Immol'thar", 1);
    assert_eq!(roster_count(&r, "Eye of Immol'thar"), 3);
}

/// King Gordok: at 50 % Cho'Rush the Observer joins (a caster that never swings); at 25 % Gordok
/// enrages; his fall alone wins the run.
#[test]
fn cho_rush_joins_king_gordok_at_half_health() {
    let mut r = run_in("feralas");
    r.next_boss = r.content.zones[r.zone].bosses.len();
    boss_near(&mut r, "King Gordok");
    assert!(boss(&r, "Cho'Rush the Observer").is_none());
    set_health(&mut r, "King Gordok", 49.0);
    let k = boss(&r, "Cho'Rush the Observer").expect("he joins");
    assert_eq!(r.enemies[k].mech.swing_mult, 0.0);
    // he never swings, even standing by the hero
    let p = r.heroes[0].pos;
    r.enemies[k].segment = Segment::standing((p.0 + 1.0, p.1), r.now);
    r.enemies[k].swing_timer = 0.0;
    for s in &mut r.enemies[k].abilities {
        s.timer = 1e6;
    }
    let health = r.heroes[0].health;
    r.enemies_tick(0.1);
    let k = boss(&r, "Cho'Rush the Observer").unwrap();
    let _ = k;
    assert!(r.drain_events().iter().all(|e| !matches!(e, Event::HeroHit { melee: true, .. })) && r.heroes[0].health >= health - 1e3);
    set_health(&mut r, "King Gordok", 24.0);
    assert!(r.enemies[boss(&r, "King Gordok").unwrap()].enraged);
    kill(&mut r, "King Gordok");
    assert!(r.won_at.is_some(), "Gordok's fall wins");
}

// ---- Gurubashi Arena (Zul'Gurub) ---------------------------------------------------------------

/// Jeklik: a bat at first, calling bat swarms from the edges; at 50 % her troll shape, the swarms
/// stop and bat riders come, each dropping fire bombs at the hero's spot (marked, leaving fire).
#[test]
fn jeklik_turns_troll_and_her_bat_riders_drop_fire() {
    let mut r = run_in("arena");
    boss_near(&mut r, "High Priestess Jeklik");
    let k = boss(&r, "High Priestess Jeklik").unwrap();
    assert_eq!(r.enemies[k].mech.display_id, Some(15191), "a bat");
    timer_due(&mut r, "High Priestess Jeklik", 1);
    assert_eq!(roster_count(&r, "Bloodseeker Bat"), 6, "a swarm");
    set_health(&mut r, "High Priestess Jeklik", 49.0);
    let k = boss(&r, "High Priestess Jeklik").unwrap();
    assert_eq!(r.enemies[k].mech.display_id, Some(15219), "her troll shape");
    assert_eq!(roster_count(&r, "Gurubashi Bat Rider"), 2);
    // the swarms stop, the riders' own timer runs
    let lines = r.enemies[k].mech.next_at.clone();
    assert!(lines[1].is_infinite(), "no more swarms");
    timer_due(&mut r, "High Priestess Jeklik", 3);
    assert_eq!(roster_count(&r, "Gurubashi Bat Rider"), 4);
    let zone = r.content.zones[r.zone].clone();
    let riders: Vec<u32> = r.enemies.iter().filter(|e| matches!(e.kind, EnemyKind::Roster(i) if zone.enemies[i].name == "Gurubashi Bat Rider")).map(|e| e.id).collect();
    cast(&mut r, "High Priestess Jeklik", 0);
    assert_eq!(r.telegraphs.iter().filter(|t| riders.contains(&t.source)).count(), 4, "a bomb from each rider");
    let p = r.heroes[0].pos;
    // the first at the hero's spot, the others round it, apart
    assert!(r.telegraphs.iter().any(|t| t.at == p));
    assert!(r.telegraphs.iter().all(|t| ((t.at.0 - p.0).powi(2) + (t.at.1 - p.1).powi(2)).sqrt() <= 7.01));
    assert!(r.telegraphs.iter().filter(|t| t.at == p).count() == 1, "not all on one spot");
    r.now += 2.1;
    r.enemy_hits_tick();
    assert!(r.ground_zones.iter().any(|z| z.spec.look_spell == 12468), "fire left");
}

/// Venoxis: at 50 % he becomes a snake with his line and voice, cobras come, and his poison
/// clouds stay on the ground.
#[test]
fn venoxis_becomes_a_snake_with_cobras_and_poison_clouds() {
    let mut r = run_in("arena");
    boss_near(&mut r, "High Priest Venoxis");
    r.drain_events();
    set_health(&mut r, "High Priest Venoxis", 49.0);
    assert!(r.drain_events().iter().any(|e| matches!(e, Event::Sound(8421))));
    let k = boss(&r, "High Priest Venoxis").unwrap();
    assert_eq!(r.enemies[k].mech.display_id, Some(15102));
    assert_eq!(roster_count(&r, "Razzashi Cobra"), 4);
    cast(&mut r, "High Priest Venoxis", 0);
    r.now += 1.6;
    r.enemy_hits_tick();
    assert_eq!(r.ground_zones.iter().filter(|z| z.spec.look_spell == 23861).count(), 2, "two clouds stay");
}

/// Mandokir comes with Ohgan; killing Ohgan first enrages him; each life the hero loses to him
/// makes him stronger ("DING!").
#[test]
fn mandokir_comes_with_ohgan_and_levels_up_on_the_hero() {
    let mut r = run_in("arena");
    boss_near(&mut r, "Bloodlord Mandokir");
    assert!(boss(&r, "Ohgan").is_some(), "with his raptor");
    let id = id_of(&r, "Bloodlord Mandokir");
    let damage = r.enemies[boss(&r, "Bloodlord Mandokir").unwrap()].damage;
    r.heroes[0].extra_lives = 1;
    r.drain_events();
    r.damage_hero(0, 1e12, Some(id), true);
    let ev = r.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::ExtraLifeUsed { .. })));
    assert!(ev.iter().any(|e| matches!(e, Event::Yell { text, .. } if text == "DING!")));
    let k = boss(&r, "Bloodlord Mandokir").unwrap();
    assert!((r.enemies[k].damage / damage - 1.1).abs() < 1e-4, "stronger");
    assert!(!r.enemies[k].enraged);
    kill(&mut r, "Ohgan");
    assert!(r.enemies[boss(&r, "Bloodlord Mandokir").unwrap()].enraged, "Ohgan's death enrages him");
}

/// Jin'do: his Healing Ward heals him until broken; his Brain Wash Totem marks the hero's spot;
/// Shades come from the edges.
#[test]
fn jin_do_s_totems_heal_him_and_mark_the_hero() {
    let mut r = run_in("arena");
    boss_near(&mut r, "Jin'do the Hexxer");
    let k = boss(&r, "Jin'do the Hexxer").unwrap();
    r.enemies[k].health = r.enemies[k].max_health * 0.5;
    cast(&mut r, "Jin'do the Hexxer", 1);
    r.now += 3.0;
    r.ground_objects_tick(0.0);
    let k = boss(&r, "Jin'do the Hexxer").unwrap();
    assert!(r.enemies[k].health > r.enemies[k].max_health * 0.5, "the ward heals him");
    walk_over(&mut r, 0);
    assert!(r.ground_objects.is_empty());
    cast(&mut r, "Jin'do the Hexxer", 2);
    r.telegraphs.clear();
    r.now += 4.0;
    r.ground_objects_tick(0.0);
    let p = r.heroes[0].pos;
    assert!(r.telegraphs.iter().any(|t| t.at == p), "the totem marks the hero's spot");
    timer_due(&mut r, "Jin'do the Hexxer", 0);
    assert_eq!(roster_count(&r, "Shade of Jin'do"), 2);
}

/// Hakkar: a Son of Hakkar leaves its poisonous blood where it falls, and a hero walking through
/// it is poisoned; Blood Siphon freezes the fight and drains the hero and every enemy, healing
/// him, but a poisoned hero's blood hurts him instead.
#[test]
fn hakkar_s_blood_siphon_drains_all_and_poisoned_blood_hurts_him() {
    let mut r = run_in("arena");
    boss_near(&mut r, "Hakkar the Soulflayer");
    let son = r.roster_index("Son of Hakkar").unwrap();
    let p = r.heroes[0].pos;
    assert!(r.spawn_roster_enemy(son, Some((p.0 + 1.0, p.1))));
    let son_id = r.enemies.last().unwrap().id;
    hit(&mut r, son_id, 1e12);
    assert!(r.ground_zones.iter().any(|z| z.spec.look_spell == 24320), "its blood stays");
    // another enemy, to be drained
    assert!(r.spawn_roster_enemy(0, Some((p.0 + 10.0, p.1))));
    let (add, its_max) = (r.enemies.last().unwrap().id, r.enemies.last().unwrap().max_health);
    // unpoisoned: the siphon hurts the hero and heals him
    let k = boss(&r, "Hakkar the Soulflayer").unwrap();
    let max = r.enemies[k].max_health;
    r.enemies[k].health = max * 0.5;
    let (h, _) = (r.heroes[0].id, ());
    r.hero_moved(h, (p.0 + 30.0, p.1), 0.0, 0.0);
    let health = r.heroes[0].health;
    r.drain_events();
    timer_due(&mut r, "Hakkar the Soulflayer", 2);
    let ev = r.drain_events();
    assert!(ev.iter().any(|e| matches!(e, Event::PauseBegan { .. })), "the scripted pause");
    assert!(r.heroes[0].health < health, "drained");
    let k = boss(&r, "Hakkar the Soulflayer").unwrap();
    assert!(r.enemies[k].health > max * 0.5, "it heals him");
    assert!((r.enemies.iter().find(|e| e.id == add).unwrap().health - its_max * 0.5).abs() < 1.0, "the horde drained too");
    let me = r.enemies[k].id;
    assert!(r.extras.beams.iter().any(|b| b.caster == me && b.spell == 24323));
    // poisoned: walk through the blood first, and the siphon hurts him
    r.cutscene = None;
    r.hero_moved(h, p, 0.0, 0.0);
    r.now += 1.0;
    r.ground_zones_tick(0.0);
    assert!(r.heroes[0].afflictions.iter().any(|a| a.spec.look_spell == 24321), "poisoned");
    let k = boss(&r, "Hakkar the Soulflayer").unwrap();
    let before = r.enemies[k].health;
    let health = r.heroes[0].health;
    timer_due(&mut r, "Hakkar the Soulflayer", 2);
    let k = boss(&r, "Hakkar the Soulflayer").unwrap();
    assert!(r.enemies[k].health < before, "the poisoned blood hurts him");
    assert!(r.heroes[0].health >= health - 1e3, "and the hero is spared");
    assert!(r.extras.beams.iter().any(|b| b.caster == me && b.spell == 24322));
}
