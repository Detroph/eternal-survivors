use std::sync::Arc;

use esurv_content::{Faction, Goal, WarbandCounter};
use esurv_sim::game::challenge::Challenge;
use esurv_sim::game::enemy::RunOptions;
use esurv_sim::game::run::{Flat, HeroSetup};

use super::*;

fn content() -> Arc<Content> {
    Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap())
}

/// A run in `zone` with one hero of `class` playing starting option `start` (and loadout `loadout`).
fn run_of(zone: &str, class: &str, start: &str, loadout: Option<&str>) -> Run {
    let content = content();
    let mut setup = HeroSetup::with_progress(&content, class, "orc", [], &[]);
    setup.start = Some(start.into());
    setup.loadout = loadout.map(str::to_string);
    Run::new(
        content,
        zone,
        RunOptions::default(),
        &[setup],
        Box::new(Flat),
        1,
    )
    .unwrap()
}

fn hit(id: u32, hero: u32, source: HitSource) -> Event {
    Event::EnemyHit {
        id,
        amount: 10.0,
        crit: false,
        by: Some(hero),
        source,
    }
}

fn died(id: u32, hero: u32) -> Event {
    Event::EnemyDied { id, by: Some(hero) }
}

fn win(conditions: Vec<RunCondition>) -> Vec<RunCondition> {
    conditions
}

/// The hero's health set to `share` of its most, and a tick taken in.
fn at_health(t: &mut RunTracker, run: &mut Run, share: f32) {
    let h = &mut run.heroes[0];
    h.health = h.stats.max_health * share;
    t.observe(run, &[]);
}

#[test]
fn a_killing_blow_is_the_last_hit_before_the_enemy_died() {
    let run = run_of("feralas", "warrior", "whirlwind", None);
    let me = run.heroes[0].id;
    let mut t = RunTracker::default();
    let whirlwind = Source::Spells(vec!["whirlwind".into()]);
    let blows = |t: &RunTracker, n: u32, by: &Source| {
        t.win_holds(
            &run,
            &run.heroes[0],
            &[],
            &[RunCondition::KillingBlows {
                by: by.clone(),
                at_least: n,
            }],
        )
    };
    t.observe(
        &run,
        &[
            hit(1, me, HitSource::Spell("whirlwind".into())),
            hit(1, me, HitSource::WeaponSwing),
            died(1, me),
        ],
    );
    t.observe(
        &run,
        &[
            hit(2, me, HitSource::WeaponSwing),
            hit(2, me, HitSource::Tick("whirlwind".into())),
            died(2, me),
        ],
    );
    t.observe(&run, &[hit(3, me, HitSource::Pet(99)), died(3, me)]);
    assert!(
        blows(&t, 1, &whirlwind) && !blows(&t, 2, &whirlwind),
        "a tick is its spell's; the swing landed the first"
    );
    assert!(blows(&t, 1, &Source::Swing) && !blows(&t, 2, &Source::Swing));
    assert!(
        blows(&t, 1, &Source::Pets),
        "a pet that is no ally's is the hero's own"
    );
    assert_eq!(t.kills_pending(), 3);
    assert_eq!((t.take_kills(me), t.kills_pending()), (3, 0));
}

#[test]
fn a_boss_fight_is_timed_from_its_arrival_and_counts_the_health_lost_in_it() {
    let content = content();
    let mut run = run_of("elwynn", "warrior", "slam", None);
    let me = run.heroes[0].id;
    let most = run.heroes[0].stats.max_health;
    let mut t = RunTracker::default();
    run.now = 100.0;
    t.observe(
        &run,
        &[Event::BossSpawned {
            id: 50,
            boss: 0,
            at: (0.0, 0.0),
        }],
    );
    run.now = 115.0;
    t.observe(
        &run,
        &[Event::HeroHit {
            hero: me,
            amount: most * 0.04,
            lost: most * 0.04,
            by: Some(50),
            melee: true,
        }],
    );
    at_health(&mut t, &mut run, 0.6);
    run.heroes[0].health = most;
    let fell = t.observe(&run, &[died(50, me)]);
    assert_eq!(fell.len(), 1);
    let (boss, fights) = &fell[0];
    let record = &fights[0].1;
    assert_eq!((*boss, fights[0].0, record.seconds), (0, me, 15.0));
    assert!(
        (record.lost - 0.04).abs() < 1e-4 && (record.lowest - 0.6).abs() < 1e-4,
        "{record:?}"
    );
    let fight = |id: &str| match &content.achievement(id).unwrap().criteria {
        Criteria::Fight(f) => f.clone(),
        other => panic!("{other:?}"),
    };
    assert!(record.meets(&fight("gnoll_means_no")), "Hogger within 20 s");
    let mut late = record.clone();
    late.seconds = 21.0;
    assert!(!late.meets(&fight("gnoll_means_no")));
    assert!(!record.meets(&fight("stitched_up")), "another boss");
    let stitches = FightRecord {
        zone: "duskwood".into(),
        boss: 2,
        seconds: 30.0,
        lost: 0.05,
        lowest: 0.95,
    };
    assert!(stitches.meets(&fight("stitched_up")));
    assert!(!FightRecord {
        lost: 0.051,
        ..stitches
    }
    .meets(&fight("stitched_up")));
    let cthun = FightRecord {
        zone: "aq".into(),
        boss: 4,
        seconds: 90.0,
        lost: 2.0,
        lowest: 0.5,
    };
    assert!(cthun.meets(&fight("eye_exam_passed")));
    assert!(!FightRecord {
        lowest: 0.49,
        ..cthun
    }
    .meets(&fight("eye_exam_passed")));
    // won with the fight in it
    let in_the_run = RunCondition::Boss(fight("gnoll_means_no"));
    assert!(t.win_holds(&run, &run.heroes[0], &[], &[in_the_run]));
}

#[test]
fn a_drop_below_counts_once_until_the_hero_is_back_to_the_rearm_share() {
    let mut run = run_of("feralas", "druid", "cat_form", None);
    let mut t = RunTracker::default();
    let nine = RunCondition::DroppedBelow {
        share: 0.2,
        back_to: 0.3,
        times: 9,
    };
    for _ in 0..8 {
        at_health(&mut t, &mut run, 0.15);
        at_health(&mut t, &mut run, 0.25);
        at_health(&mut t, &mut run, 0.18);
        at_health(&mut t, &mut run, 0.35);
    }
    assert!(
        !t.win_holds(&run, &run.heroes[0], &[], &[nine.clone()]),
        "8 times, the dips under 30 % between them not counted"
    );
    at_health(&mut t, &mut run, 0.19);
    assert!(t.win_holds(&run, &run.heroes[0], &[], &[nine]));
    assert!(!t.win_holds(
        &run,
        &run.heroes[0],
        &[],
        &[RunCondition::HealthNeverBelow(0.5)]
    ));
    assert!(t.win_holds(
        &run,
        &run.heroes[0],
        &[],
        &[RunCondition::HealthNeverBelow(0.05)]
    ));
}

#[test]
fn every_won_run_condition_reads_the_run() {
    let content = content();
    let mut run = run_of("brm", "mage", "fireball", Some("staff"));
    let me = run.heroes[0].id;
    let mut t = RunTracker::default();
    t.observe(&run, &[]);
    let holds = |t: &RunTracker, run: &Run, c: RunCondition| {
        t.win_holds(run, &run.heroes[0], &[], &win(vec![c]))
    };
    assert!(
        holds(&t, &run, RunCondition::Zone("brm".into()))
            && !holds(&t, &run, RunCondition::Zone("elwynn".into()))
    );
    assert!(
        holds(&t, &run, RunCondition::Class("mage".into()))
            && !holds(&t, &run, RunCondition::Class("priest".into()))
    );
    assert!(
        holds(&t, &run, RunCondition::Start("fireball".into()))
            && !holds(&t, &run, RunCondition::Start("frostbolt".into()))
    );
    let staff = |class: &str| RunCondition::Loadout {
        class: class.into(),
        id: "staff".into(),
    };
    assert!(
        holds(&t, &run, staff("mage")) && !holds(&t, &run, staff("warlock")),
        "a loadout is its class's"
    );
    // Challenge Mode at its slider
    assert!(
        !holds(&t, &run, RunCondition::ChallengeAtLeast(200)),
        "Normal Mode"
    );
    run.options.challenge = Challenge::new(&content, 300);
    assert!(
        holds(&t, &run, RunCondition::ChallengeAtLeast(200))
            && holds(&t, &run, RunCondition::ChallengeAtLeast(300))
    );
    assert!(!holds(&t, &run, RunCondition::ChallengeAtLeast(500)));
    // every slider at its most
    assert!(!holds(&t, &run, RunCondition::RunOptionsAtMax));
    let max = |id: &str| {
        content
            .run_options
            .iter()
            .find(|s| s.id == id)
            .unwrap()
            .max_pct
    };
    run.options.density_pct = max("density");
    run.options.health_pct = max("health");
    run.options.damage_pct = max("damage");
    assert!(
        !holds(&t, &run, RunCondition::RunOptionsAtMax),
        "speed is not at its most"
    );
    run.options.speed_pct = max("speed");
    assert!(holds(&t, &run, RunCondition::RunOptionsAtMax));
    // gold: none spent, enough picked up
    let pincher = RunCondition::NoGoldSpent {
        collected_at_least: 1000,
    };
    (run.heroes[0].gold, run.heroes[0].gold_earned) = (1200, 1200);
    assert!(holds(&t, &run, pincher.clone()));
    run.heroes[0].gold = 1100;
    assert!(!holds(&t, &run, pincher.clone()), "100 spent");
    (run.heroes[0].gold, run.heroes[0].gold_earned) = (900, 900);
    assert!(!holds(&t, &run, pincher), "too little picked up");
    // a battle standard taken; another pickup taken is no standard
    assert!(holds(&t, &run, RunCondition::NoBattleStandard));
    t.observe(
        &run,
        &[
            Event::PickupDropped {
                id: 8,
                kind: Pickup::Gold,
                value: 5,
                at: (0.0, 0.0),
            },
            Event::PickupTaken { id: 8, hero: me },
        ],
    );
    assert!(holds(&t, &run, RunCondition::NoBattleStandard));
    t.observe(
        &run,
        &[
            Event::PickupDropped {
                id: 9,
                kind: Pickup::BattleStandard(Faction::Horde),
                value: 0,
                at: (0.0, 0.0),
            },
            Event::PickupTaken { id: 9, hero: me },
        ],
    );
    assert!(!holds(&t, &run, RunCondition::NoBattleStandard));
    // health lost over the run
    t.observe(
        &run,
        &[Event::HeroHit {
            hero: me,
            amount: 30_000.0,
            lost: 30_000.0,
            by: None,
            melee: false,
        }],
    );
    assert!(!holds(&t, &run, RunCondition::HealthLostAtLeast(50_000.0)));
    t.observe(
        &run,
        &[Event::HeroHit {
            hero: me,
            amount: 20_000.0,
            lost: 20_000.0,
            by: None,
            melee: false,
        }],
    );
    assert!(holds(&t, &run, RunCondition::HealthLostAtLeast(50_000.0)));
    // no condition: any win
    assert!(t.win_holds(&run, &run.heroes[0], &[], &[]));
}

#[test]
fn a_source_tops_the_meter_with_its_lines_together() {
    use protocol::Source as Line;
    let fireball = Source::Spells(vec!["fireball".into()]);
    let meter = vec![
        (Line::Spell("fireball".into()), 500.0),
        (Line::Pet("Imp".into()), 300.0),
        (Line::Pet("Imp 2".into()), 250.0),
        (Line::Swing, 10.0),
    ];
    assert!(tops_the_meter(&meter, &fireball));
    assert!(
        tops_the_meter(&meter, &Source::Pets),
        "the pets' lines together"
    );
    assert!(!tops_the_meter(&meter, &Source::Swing));
    assert!(
        !tops_the_meter(&[], &fireball),
        "nothing dealt is no top line"
    );
    let behind = vec![
        (Line::Spell("fireball".into()), 500.0),
        (
            Line::Proc {
                spell: 1,
                origin: Default::default(),
            },
            501.0,
        ),
    ];
    assert!(!tops_the_meter(&behind, &fireball));
}

/// The game earns as the run goes: a boss fight as the boss falls, a kill counter's step as its
/// kill lands, Endless minutes as they pass, the won-run achievements once as the run is won; each
/// told in the chat and to the screens, and kept in the save.
#[test]
fn the_game_earns_and_tells_achievements_as_the_run_goes() {
    let (mut game, _client) = super::super::tests::game_with_a_hero_of("warrior", "orc");
    let content = game.run.content.clone();
    let me = game.run.heroes[0].id;
    let said = |out: &[(u32, Packet)], name: &str| {
        out.iter()
            .any(|(h, p)| *h == me && *p == packets::system_message(&earned_line("Thrall", name)))
    };
    // Hogger falls 12 seconds after he came
    game.run.now = 60.0;
    game.track_achievements(&[Event::BossSpawned {
        id: 900,
        boss: 0,
        at: (0.0, 0.0),
    }]);
    game.run.now = 72.0;
    let out =
        game.track_achievements(&[hit(900, me, HitSource::Spell("slam".into())), died(900, me)]);
    assert!(said(&out, "Gnoll Means No"), "earned the moment he fell");
    let kept = game.saves.achievements(&content);
    assert!(
        kept.earned.contains_key("gnoll_means_no") && kept.bosses_defeated.contains("elwynn/0")
    );
    // the 1,000th kill earns Slayer I at once (one of them was Hogger)
    let kills: Vec<Event> = (0..998)
        .flat_map(|i| [hit(i, me, HitSource::Spell("slam".into())), died(i, me)])
        .collect();
    assert!(
        !said(&game.track_achievements(&kills), "Slayer I"),
        "999 kills"
    );
    let out = game.track_achievements(&[died(5000, me)]);
    assert!(said(&out, "Slayer I"));
    assert_eq!(game.saves.achievements(&content).kills, 1000);
    // the won run, counted once
    game.run.won_at = Some(game.run.now);
    let out = game.track_achievements(&[]);
    assert!(said(&out, "First Victory"));
    assert!(game.track_achievements(&[]).is_empty(), "counted once");
    let kept = game.saves.achievements(&content);
    assert!(
        kept.zones_cleared.contains("elwynn")
            && kept.classes_won.contains("warrior")
            && kept.starts_won.contains("slam")
    );
    assert!(
        kept.loadouts_won.contains("warrior/two_hander"),
        "{:?}",
        kept.loadouts_won
    );
    // Endless: ten whole minutes
    game.run.endless_since = Some(game.run.now);
    game.run.now += 599.0;
    assert!(!said(&game.track_achievements(&[]), "No End in Sight"));
    game.run.now += 1.0;
    assert!(said(&game.track_achievements(&[]), "No End in Sight"));
    assert_eq!(game.saves.achievements(&content).longest_endless, 600);
    // kills not yet at a step go in with the pay-out
    let more: Vec<Event> = (0..5).map(|i| died(6000 + i, me)).collect();
    game.track_achievements(&more);
    assert_eq!(game.saves.achievements(&content).kills, 1000);
    game.pay_out(me);
    assert_eq!(game.saves.achievements(&content).kills, 1005);
    let told = game.saves.achievements_told(&content);
    assert_eq!(
        told.score,
        10 + 10 + 10 + 10,
        "Gnoll Means No, Slayer I, First Victory, No End in Sight"
    );
    let slayer = told.progress.iter().find(|p| p.id == "slayer_2").unwrap();
    assert_eq!(
        (slayer.current, slayer.goal),
        (
            1005,
            Goal::Count(25_000).number(WarbandCounter::Kills, &content)
        )
    );
}

/// A legendary amulet on a card earns First Legend as the card is shown (the screens put it in
/// the codex first); a pay-out with kills tells the list again.
#[test]
fn a_legendary_shown_earns_first_legend_at_once() {
    use esurv_sim::game::amulets::{legendary_for, legendary_named};
    use esurv_sim::game::choices::{Offer, ScreenKind};
    let (mut game, _client) = super::super::tests::game_with_a_hero_of("warrior", "orc");
    let content = game.run.content.clone();
    let me = game.run.heroes[0].id;
    let amulet = legendary_for(legendary_named(&content, "legendary:whirlwind").unwrap());
    let shown = Event::ScreenOpened {
        hero: me,
        kind: ScreenKind::Cache,
        offers: vec![Offer::Amulet {
            amulet: Box::new(amulet),
        }],
    };
    assert!(game.saves.add_to_codex("legendary:whirlwind"));
    let out = game.track_achievements(&[shown]);
    let line = packets::system_message(&earned_line("Thrall", "First Legend"));
    assert!(out.iter().any(|(_, p)| *p == line));
    game.run.now = 30.0;
    game.track_achievements(&[died(1, me)]);
    // a client with screens hears the list again after a pay-out with kills
    game.told.insert(me, Default::default());
    assert!(!game.bank_kills(me).is_empty(), "the list told again");
    assert_eq!(game.saves.achievements(&content).kills, 1);
}

/// Gold collected goes into the warband's counter as the run pays out, once (/// Hoarder), and Peak of Power is earned the moment a hero reaches level 99.
#[test]
fn gold_banks_once_per_pay_out_and_level_99_earns_peak_of_power() {
    let (mut game, _client) = super::super::tests::game_with_a_hero_of("warrior", "orc");
    let content = game.run.content.clone();
    let me = game.run.heroes[0].id;
    game.run.heroes[0].gold_earned = 2600;
    game.bank_kills(me);
    game.bank_kills(me);
    let saved = game.saves.achievements(&content);
    assert_eq!(saved.gold_collected, 2600);
    assert!(saved.earned.contains_key("hoarder_1"));
    assert!(!saved.earned.contains_key("hoarder_2"));
    game.run.heroes[0].gold_earned = 2700;
    game.bank_kills(me);
    assert_eq!(game.saves.achievements(&content).gold_collected, 2700);

    game.track_achievements(&[Event::LevelUp { hero: me, level: 98 }]);
    assert!(!game.saves.achievements(&content).earned.contains_key("peak_of_power"));
    let out = game.track_achievements(&[Event::LevelUp { hero: me, level: 99 }]);
    let line = packets::system_message(&earned_line("Thrall", "Peak of Power"));
    assert!(out.iter().any(|(_, p)| *p == line));
}

/// Hoarder counts live: the gold that reaches a step earns it the
/// moment it comes in, the payout adds only the rest, and while a counting achievement is tracked
/// the list is told again (once a second at most) with the kills and gold not yet in the save.
#[test]
fn gold_earns_hoarder_the_moment_it_comes_in_and_the_tracker_moves_live() {
    let (mut game, _client) = super::super::tests::game_with_a_hero_of("warrior", "orc");
    let content = game.run.content.clone();
    let me = game.run.heroes[0].id;
    game.run.now = 10.0;
    game.run.heroes[0].gold_earned = 2499;
    game.track_achievements(&[]);
    assert_eq!(game.saves.achievements(&content).gold_collected, 0, "not at the step yet");
    game.run.heroes[0].gold_earned = 2500;
    let out = game.track_achievements(&[]);
    let line = packets::system_message(&earned_line("Thrall", "Hoarder I"));
    assert!(out.iter().any(|(_, p)| *p == line), "earned mid-run");
    assert_eq!(game.saves.achievements(&content).gold_collected, 2500);
    // the tracker: Hoarder II and Slayer I ticked, a client with screens
    assert!(game.saves.track_achievement(&content, "hoarder_2", true));
    assert!(game.saves.track_achievement(&content, "slayer_1", true));
    game.told.insert(me, Default::default());
    game.run.heroes[0].gold_earned = 2650;
    game.run.now = 12.0;
    let out = game.track_achievements(&[died(1, me), died(2, me)]);
    assert!(!out.is_empty(), "the list told again with the live counters");
    let ToClient::Achievements(live) = game.live_achievements() else { unreachable!() };
    let of = |id: &str| live.progress.iter().find(|p| p.id == id).map(|p| p.current);
    assert_eq!((of("hoarder_2"), of("slayer_1")), (Some(2650), Some(2)));
    game.run.now = 12.5;
    game.run.heroes[0].gold_earned = 2700;
    assert!(game.track_achievements(&[]).is_empty(), "not again within the second");
    // the payout adds only what is not in the save yet
    game.bank_kills(me);
    let kept = game.saves.achievements(&content);
    assert_eq!((kept.gold_collected, kept.kills), (2700, 2));
}

/// A run earns each once-a-run bounty once: the first boss down, the win, Endless reaching its
/// mark; a fresh tracker (a fresh run) earns them again.
#[test]
fn a_run_earns_each_run_bounty_once() {
    let content = content();
    let mut t = RunTracker::default();
    let boss = || AchievementFacts { bosses_defeated: vec!["elwynn/0".into()], ..Default::default() };
    assert_eq!(t.run_bounties(&content, &boss()), ["boss_hunter"]);
    assert!(t.run_bounties(&content, &boss()).is_empty(), "the second boss of the run");
    let endless = |seconds| AchievementFacts { endless_seconds: seconds, ..Default::default() };
    assert!(t.run_bounties(&content, &endless(599)).is_empty());
    assert_eq!(t.run_bounties(&content, &endless(600)), ["endless_hour"]);
    assert!(t.run_bounties(&content, &endless(660)).is_empty());
    let won = AchievementFacts { zone_cleared: Some("elwynn".into()), ..Default::default() };
    assert_eq!(t.run_bounties(&content, &won), ["zone_victor"]);
    assert_eq!(RunTracker::default().run_bounties(&content, &boss()), ["boss_hunter"], "the next run");
}
