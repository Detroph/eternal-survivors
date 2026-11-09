use super::*;
use crate::save::lock;

fn content() -> Content {
    Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
}

/// A save as v0.5.3 writes it (no achievements section), plain as before the lock: a warrior
/// that won three zones, a mage that won Elwynn, a priest hero with a win in its records (its
/// class's zones lost to an older save), and a legendary seen in the codex.
const V0_5_3_SAVE: &str = r#"(
    gear_numbering: 2,
    heroes: [
        (guid: 1, name: "Grom", class: "warrior", race: "orc", gender: 0, look: (0, 0, 0, 0, 0),
         records: (best_level: 30, best_seconds: 1200, runs: 9, wins: 3)),
        (guid: 2, name: "Jaina", class: "mage", race: "human", gender: 1, look: (0, 0, 0, 0, 0),
         records: (best_level: 25, best_seconds: 1200, runs: 2, wins: 1)),
        (guid: 3, name: "Anduin", class: "priest", race: "human", gender: 0, look: (0, 0, 0, 0, 0),
         records: (best_level: 20, best_seconds: 1200, runs: 1, wins: 1)),
    ],
    next_guid: 4,
    classes: {
        "warrior": (talent_experience: 5000, zones_won: ["duskwood", "elwynn", "wsg"]),
        "mage": (talent_experience: 900, zones_won: ["elwynn"]),
    },
    codex: ["accuria", "legendary:fireball"],
)"#;

fn temp_save(test: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir =
        std::env::temp_dir().join(format!("esurv-achievements-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("heroes.ron");
    (dir, path)
}

/// The first load of a save from before achievements grants what it can prove, writes the
/// section sealed, and a second load reads it back as it was, granting nothing again.
#[test]
fn an_older_save_is_granted_what_it_proves_once_and_keeps_it_sealed() {
    let content = content();
    let (dir, path) = temp_save("older");
    std::fs::write(&path, V0_5_3_SAVE).unwrap();
    let saves = Saves::open(&path).unwrap();
    let earned = saves.catch_up_achievements(&content);
    assert_eq!(earned, ["first_victory", "wanderer", "first_legend"]);
    let a = saves.achievements(&content);
    assert_eq!(
        a.zones_cleared
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["duskwood", "elwynn", "wsg"]
    );
    assert_eq!(
        a.classes_won.iter().map(String::as_str).collect::<Vec<_>>(),
        ["mage", "priest", "warrior"],
        "the priest by its hero's records"
    );
    // each won zone's final boss: Elwynn's VanCleef, Duskwood's Morbent Fel, Warsong's last carrier
    let finals: Vec<String> = ["elwynn", "duskwood", "wsg"]
        .iter()
        .map(|z| {
            let zone = content.zone(z).unwrap();
            boss_key(z, zone.bosses.iter().position(|b| b.is_final).unwrap())
        })
        .collect();
    assert_eq!(a.bosses_defeated, finals.into_iter().collect());
    assert_eq!(
        (
            a.kills,
            a.longest_endless,
            a.starts_won.len(),
            a.loadouts_won.len()
        ),
        (0, 0, 0, 0),
        "never kept: from nothing"
    );
    let told = saves.achievements_told(&content);
    assert_eq!((told.score, told.balance), (30, 30));
    let zones = told.progress.iter().find(|p| p.id == "pathfinder").unwrap();
    assert_eq!((zones.current, zones.goal), (3, 6));
    // the save on disk is sealed with its section; read again it is the same, and nothing is new
    assert!(matches!(
        lock::unlock(&std::fs::read(&path).unwrap()),
        lock::Unlocked::Locked(_)
    ));
    let again = Saves::open(&path).unwrap();
    assert_eq!(again.achievements(&content), a);
    assert!(again.catch_up_achievements(&content).is_empty());
    assert_eq!(again.heroes().len(), 3, "the rest of the save as it was");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A new save starts with nothing earned, and what a run adds is kept across a restart.
#[test]
fn a_new_save_earns_from_runs_and_keeps_it_across_a_restart() {
    let content = content();
    let (dir, path) = temp_save("new");
    let saves = Saves::open(&path).unwrap();
    assert!(saves.catch_up_achievements(&content).is_empty());
    let won = AchievementFacts {
        kills: 40,
        zone_cleared: Some("elwynn".into()),
        class_won: Some("warrior".into()),
        start_won: Some("slam".into()),
        loadout_won: Some(loadout_key("warrior", "two_hander")),
        met: vec!["first_victory".into()],
        ..Default::default()
    };
    assert_eq!(
        saves.add_achievement_facts(&content, &won),
        ["first_victory"]
    );
    let reopened = Saves::open(&path).unwrap();
    assert_eq!(
        reopened.achievements(&content),
        saves.achievements(&content)
    );
    let told = reopened.achievements_told(&content);
    assert_eq!(
        told.earned
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        ["first_victory"]
    );
    let starts = told
        .progress
        .iter()
        .find(|p| p.id == "warrior_every_start")
        .unwrap();
    assert_eq!(
        (starts.current, starts.goal, starts.done.clone()),
        (1, 3, vec!["slam".to_string()])
    );
    let loadouts = told
        .progress
        .iter()
        .find(|p| p.id == "warrior_every_loadout")
        .unwrap();
    assert_eq!(loadouts.done, ["two_hander"]);
    assert!(
        told.progress.iter().all(|p| p.id != "death_knight"),
        "a hidden achievement shows no progress"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn add(s: &mut SavedAchievements, content: &Content, facts: AchievementFacts) -> Vec<String> {
    s.add(content, &BTreeSet::new(), &facts, 7)
}

#[test]
fn a_counter_earns_each_step_as_it_reaches_it() {
    let content = content();
    let mut s = SavedAchievements::default();
    assert!(add(
        &mut s,
        &content,
        AchievementFacts {
            kills: 999,
            ..Default::default()
        }
    )
    .is_empty());
    assert_eq!(
        add(
            &mut s,
            &content,
            AchievementFacts {
                kills: 1,
                ..Default::default()
            }
        ),
        ["slayer_1"]
    );
    assert_eq!(
        add(
            &mut s,
            &content,
            AchievementFacts {
                kills: 249_000,
                ..Default::default()
            }
        ),
        ["slayer_2", "slayer_3"]
    );
    assert_eq!(s.earned["slayer_3"], 7, "earned now");
    // Endless: the longest run, never a sum
    let endless = |seconds| AchievementFacts {
        endless_seconds: seconds,
        ..Default::default()
    };
    assert!(add(&mut s, &content, endless(599)).is_empty());
    assert_eq!(add(&mut s, &content, endless(600)), ["no_end_in_sight"]);
    assert!(add(&mut s, &content, endless(1799)).is_empty());
    assert!(add(&mut s, &content, endless(1200)).is_empty());
    assert_eq!(add(&mut s, &content, endless(1800)), ["tortoise_wins"]);
    // different things: zones, bosses, classes; every one of them for `All`
    for z in &content.zones {
        add(
            &mut s,
            &content,
            AchievementFacts {
                zone_cleared: Some(z.id.clone()),
                ..Default::default()
            },
        );
    }
    assert!(["wanderer", "pathfinder", "the_eternal_road"]
        .iter()
        .all(|id| s.earned.contains_key(*id)));
    let mut bosses: Vec<String> = content
        .zones
        .iter()
        .flat_map(|z| (0..z.bosses.len()).map(move |i| boss_key(&z.id, i)))
        .collect();
    let last = bosses.pop().unwrap();
    add(
        &mut s,
        &content,
        AchievementFacts {
            bosses_defeated: bosses.clone(),
            ..Default::default()
        },
    );
    assert!(!s.earned.contains_key("bossbane"), "one boss short");
    add(
        &mut s,
        &content,
        AchievementFacts {
            bosses_defeated: bosses,
            ..Default::default()
        },
    );
    assert!(
        !s.earned.contains_key("bossbane"),
        "the same bosses again are no new ones"
    );
    assert_eq!(
        add(
            &mut s,
            &content,
            AchievementFacts {
                bosses_defeated: vec![last],
                ..Default::default()
            }
        ),
        ["bossbane"]
    );
    for c in &content.classes[..8] {
        add(
            &mut s,
            &content,
            AchievementFacts {
                class_won: Some(c.id.clone()),
                ..Default::default()
            },
        );
    }
    assert!(!s.earned.contains_key("of_nine_paths"));
    add(
        &mut s,
        &content,
        AchievementFacts {
            class_won: Some(content.classes[8].id.clone()),
            ..Default::default()
        },
    );
    assert!(s.earned.contains_key("of_nine_paths"));
}

#[test]
fn a_legendary_in_the_codex_earns_first_legend() {
    let content = content();
    let mut s = SavedAchievements::default();
    let codex: BTreeSet<String> = ["accuria".to_string()].into();
    assert!(
        s.add(&content, &codex, &AchievementFacts::default(), 1)
            .is_empty(),
        "a ring is no legendary"
    );
    let codex: BTreeSet<String> = ["accuria".to_string(), "legendary:whirlwind".into()].into();
    assert_eq!(
        s.add(&content, &codex, &AchievementFacts::default(), 1),
        ["first_legend"]
    );
}

/// A checklist is earned with its last mark, and a meta with its last part, in the same go.
#[test]
fn checklists_and_metas_are_earned_with_their_last_part() {
    let content = content();
    let mut s = SavedAchievements::default();
    let start = |id: &str| AchievementFacts {
        start_won: Some(id.into()),
        ..Default::default()
    };
    for c in &content.classes {
        let options: Vec<&str> = c.starting_options.iter().map(|o| o.id.as_str()).collect();
        let (last, rest) = options.split_last().unwrap();
        for o in rest {
            assert!(add(&mut s, &content, start(o)).is_empty(), "{o}");
        }
        let earned = add(&mut s, &content, start(last));
        assert_eq!(earned[0], format!("{}_every_start", c.id));
        if c.id == content.classes.last().unwrap().id {
            assert_eq!(
                earned,
                [format!("{}_every_start", c.id), "spellbound".to_string()],
                "the meta after its part"
            );
        }
    }
    for c in &content.classes {
        for l in content.loadouts_of(&c.id) {
            add(
                &mut s,
                &content,
                AchievementFacts {
                    loadout_won: Some(loadout_key(&c.id, &l.id)),
                    ..Default::default()
                },
            );
        }
    }
    assert!(s.earned.contains_key("well_armed"));
    // a loadout is counted for its own class only
    let mut other = SavedAchievements::default();
    add(
        &mut other,
        &content,
        AchievementFacts {
            loadout_won: Some(loadout_key("mage", "staff")),
            ..Default::default()
        },
    );
    let warlock = content.achievement("warlock_every_loadout").unwrap();
    assert_eq!(
        other
            .progress_of(&content, &BTreeSet::new(), &warlock.criteria)
            .unwrap()
            .current,
        0
    );
}

#[test]
fn only_what_can_be_earned_is_earned_and_the_score_counts_its_points() {
    let content = content();
    let mut s = SavedAchievements::default();
    let met = |ids: &[&str]| AchievementFacts {
        met: ids.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    assert!(add(
        &mut s,
        &content,
        met(&["death_knight", "no_such_achievement"])
    )
    .is_empty());
    assert_eq!(
        add(&mut s, &content, met(&["lord_of_nothing", "first_victory"])),
        ["lord_of_nothing", "first_victory"]
    );
    assert!(
        add(&mut s, &content, met(&["first_victory"])).is_empty(),
        "once"
    );
    assert_eq!(s.score(&content), 60);
    s.spent = 25;
    let told = s.told(&content, &BTreeSet::new());
    assert_eq!(
        (told.score, told.balance),
        (60, 35),
        "the score never drops; the balance does"
    );
    // an achievement the content no longer has counts for nothing
    s.earned.insert("removed_one".into(), 1);
    assert_eq!(s.score(&content), 60);
}

/// Kill steps left, for the run to earn a step the moment it is reached.
#[test]
fn the_kills_to_the_next_step_are_counted_from_the_save() {
    let content = content();
    let saves = Saves::in_memory();
    assert_eq!(saves.kills_to_next_step(&content), Some(1000));
    saves.add_achievement_facts(
        &content,
        &AchievementFacts {
            kills: 1500,
            ..Default::default()
        },
    );
    assert_eq!(saves.kills_to_next_step(&content), Some(23_500));
    saves.add_achievement_facts(
        &content,
        &AchievementFacts {
            kills: 300_000,
            ..Default::default()
        },
    );
    assert_eq!(saves.kills_to_next_step(&content), None);
}

/// Every achievement's icon is a texture of the player's client.
#[test]
fn every_achievement_icon_is_in_the_players_data() {
    let data = benilla_formats::wow_data_or_skip!();
    let chain = benilla_formats::open_chain(&data).unwrap();
    for a in &content().achievements {
        let path = format!("Interface\\Icons\\{}.blp", a.icon);
        assert!(chain.contains(&path), "{}: {path}", a.id);
    }
}

/// Up to three achievements are tracked, kept across a restart, and one earned drops out; the
/// window's news stays until it is opened.
#[test]
fn tracking_and_the_windows_news_are_kept_in_the_save() {
    let content = content();
    let (dir, path) = temp_save("tracked");
    let saves = Saves::open(&path).unwrap();
    assert!(
        !saves.achievements_told(&content).news,
        "nothing earned, nothing new"
    );
    for id in ["first_victory", "slayer_1", "wanderer"] {
        assert!(saves.track_achievement(&content, id, true), "{id}");
    }
    assert!(
        !saves.track_achievement(&content, "pathfinder", true),
        "a fourth is refused"
    );
    assert!(
        !saves.track_achievement(&content, "death_knight", true),
        "a hidden one is refused"
    );
    assert!(!saves.track_achievement(&content, "nothing_at_all", true));
    assert!(saves.track_achievement(&content, "slayer_1", false));
    assert!(
        saves.track_achievement(&content, "pathfinder", true),
        "room again"
    );
    let won = AchievementFacts {
        met: vec!["first_victory".into()],
        zone_cleared: Some("elwynn".into()),
        class_won: Some("warrior".into()),
        ..Default::default()
    };
    saves.add_achievement_facts(&content, &won);
    let reopened = Saves::open(&path).unwrap();
    let told = reopened.achievements_told(&content);
    assert_eq!(
        told.tracked,
        ["wanderer", "pathfinder"],
        "the earned one drops out"
    );
    assert!(told.news, "earned since the window was opened");
    reopened.see_achievements(&content);
    assert!(!Saves::open(&path).unwrap().achievements_told(&content).news);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The shop spends the points to spend and never the score: a companion once, the upgrade a
/// rank at a time, nothing past the points; an achievement's companion is never sold and is owned
/// once earned; each hero picks one it owns, kept across a restart, and brings it at the
/// warband's rank.
#[test]
fn the_shop_spends_points_and_heroes_pick_companions_they_own() {
    let content = content();
    let (dir, path) = temp_save("shop");
    let saves = Saves::open(&path).unwrap();
    let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
    assert!(!saves.buy(&content, "companion:bombay_cat"), "no points yet");
    assert!(!saves.choose_companion(&content, bo.guid, Some("bombay_cat")), "not owned");
    let facts = AchievementFacts {
        met: ["first_victory", "gnoll_means_no", "lord_of_nothing", "tortoise_wins"]
            .map(String::from)
            .to_vec(),
        ..Default::default()
    };
    saves.add_achievement_facts(&content, &facts);
    let told = saves.achievements_told(&content);
    assert_eq!((told.score, told.balance, told.companion_rank), (120, 120, 1));
    assert!(saves.buy(&content, "companion:bombay_cat"));
    assert!(!saves.buy(&content, "companion:bombay_cat"), "once");
    assert!(!saves.buy(&content, "companion:tiny_hogger"), "never sold");
    assert!(!saves.buy(&content, "companion_rank:3"), "rank 2 first");
    assert!(saves.buy(&content, "companion_rank:2"));
    assert!(!saves.buy(&content, "companion_rank:3"), "55 points do not cover 100");
    assert!(!saves.buy(&content, "decoration:onyxia_head"), "a trophy is never sold");
    let told = saves.achievements_told(&content);
    assert_eq!((told.score, told.balance, told.companion_rank), (120, 55, 2), "the score stays");
    assert_eq!(told.bought, ["companion:bombay_cat", "companion_rank:2"]);
    assert!(saves.choose_companion(&content, bo.guid, Some("tiny_hogger")), "earned");
    assert!(!saves.choose_companion(&content, bo.guid, Some("tiny_gordok")), "not earned");
    assert!(!saves.choose_companion(&content, bo.guid, Some("no_such_pet")));
    let reopened = Saves::open(&path).unwrap();
    assert_eq!(reopened.hero(bo.guid).unwrap().companion.as_deref(), Some("tiny_hogger"));
    assert_eq!(reopened.run_companion(&content, bo.guid), Some(("tiny_hogger".into(), 2)));
    assert!(reopened.choose_companion(&content, bo.guid, None));
    assert_eq!(reopened.run_companion(&content, bo.guid), None);
    assert_eq!(reopened.achievements_told(&content).balance, 55);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every companion's icon is a texture of the player's client.
#[test]
fn every_companion_icon_is_in_the_players_data() {
    let data = benilla_formats::wow_data_or_skip!();
    let chain = benilla_formats::open_chain(&data).unwrap();
    for c in &content().companions {
        let path = format!("Interface\\Icons\\{}.blp", c.icon);
        assert!(chain.contains(&path), "{}: {path}", c.id);
    }
}

/// Banish and skip: bought a rank at a time from rank 1, and a run brings the
/// charges of the last rank bought.
#[test]
fn banish_and_skip_ranks_are_bought_in_turn_and_give_their_charges() {
    let content = content();
    let (dir, path) = temp_save("charges");
    let saves = Saves::open(&path).unwrap();
    let facts = AchievementFacts {
        met: ["lord_of_nothing", "tortoise_wins", "spin_to_win", "first_victory"].map(String::from).to_vec(),
        ..Default::default()
    };
    saves.add_achievement_facts(&content, &facts);
    assert_eq!(saves.achievements_told(&content).balance, 135);
    assert_eq!(saves.run_charges(&content), (0, 0));
    assert!(!saves.buy(&content, "banish_rank:2"), "rank 1 first");
    assert!(saves.buy(&content, "banish_rank:1"));
    assert!(saves.buy(&content, "banish_rank:2"));
    assert!(saves.buy(&content, "skip_rank:1"));
    assert!(!saves.buy(&content, "skip_rank:2"), "35 points do not cover 50");
    let told = saves.achievements_told(&content);
    assert_eq!((told.balance, told.banish_rank, told.skip_rank), (35, 2, 1));
    assert_eq!(saves.run_charges(&content), (2, 1));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Step 3: a title once earned, a tabard or glow once bought (a faction's own tabard only for its
/// heroes, the damage glows a rank at a time), each hero its own; a camp's spots hold only what
/// the warband owns and fits, each camp apart, kept across a restart; what a run shows.
#[test]
fn heroes_wear_what_the_warband_owns_and_each_camp_is_decorated_apart() {
    use esurv_protocol::WearKind;
    let content = content();
    let (dir, path) = temp_save("wears");
    let saves = Saves::open(&path).unwrap();
    let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
    let ann = saves.create("Ann", "mage", "human", 1, [0; 5]).unwrap();
    assert!(!saves.dress(&content, bo.guid, WearKind::Title, Some("the_eternal_road")), "not earned");
    let facts = AchievementFacts {
        met: ["the_eternal_road", "lord_of_nothing", "tortoise_wins", "bossbane", "spellbound", "eye_exam_passed", "three_times_the_trouble", "slayer_3", "of_nine_paths", "well_armed", "twice_as_hard", "first_victory", "gnoll_means_no"]
            .map(String::from)
            .to_vec(),
        ..Default::default()
    };
    saves.add_achievement_facts(&content, &facts);
    assert!(saves.dress(&content, bo.guid, WearKind::Title, Some("the_eternal_road")));
    assert!(!saves.dress(&content, bo.guid, WearKind::Title, Some("first_victory")), "no title there");
    assert!(!saves.dress(&content, bo.guid, WearKind::Tabard, Some("guild")), "not bought");
    assert!(saves.buy(&content, "tabard:stormwind") && saves.buy(&content, "tabard:warsong_silverwing"));
    assert!(!saves.dress(&content, bo.guid, WearKind::Tabard, Some("stormwind")), "the Alliance's own");
    assert!(saves.dress(&content, ann.guid, WearKind::Tabard, Some("stormwind")));
    assert!(saves.dress(&content, bo.guid, WearKind::Tabard, Some("warsong_silverwing")));
    assert!(!saves.buy(&content, "glow:damage_medium"), "the low one first");
    assert!(saves.buy(&content, "glow:damage_low") && saves.buy(&content, "glow:damage_medium"));
    assert!(saves.dress(&content, bo.guid, WearKind::Glow, Some("damage_medium")));
    let reopened = Saves::open(&path).unwrap();
    let bo_now = reopened.hero(bo.guid).unwrap();
    assert_eq!(reopened.wears_told(&content, &bo_now), ["the_eternal_road".to_owned(), "warsong_silverwing".into(), "damage_medium".into()]);
    let run = reopened.run_wears(&content, &bo_now);
    assert_eq!((run.name.as_str(), run.tabard, run.enchant), ("Bo the Eternal", Some(32031), Some(1895)));
    let ann_now = reopened.hero(ann.guid).unwrap();
    assert_eq!(reopened.run_wears(&content, &ann_now).tabard, Some(21338));
    assert!(reopened.dress(&content, bo.guid, WearKind::Title, None));
    assert_eq!(reopened.run_wears(&content, &reopened.hero(bo.guid).unwrap()).name, "Bo");

    // the camps: the theme is a set with a piece owned; trophies stand on no spot
    assert!(!reopened.decorate(&content, "horde", "theme", Some("war_camp")), "no piece of it owned");
    assert!(reopened.buy(&content, "decoration:weapon_rack"));
    assert!(reopened.decorate(&content, "horde", "theme", Some("war_camp")));
    assert!(!reopened.decorate(&content, "horde", "theme", Some("weapon_rack")), "a theme is a set");
    assert!(!reopened.decorate(&content, "horde", "banner", Some("weapon_rack")), "not the banner post's");
    assert!(!reopened.decorate(&content, "horde", "trophy_1", Some("throne")), "no such spot");
    assert!(!reopened.decorate(&content, "goblin", "theme", Some("war_camp")), "no such camp");
    assert!(!reopened.decorate(&content, "alliance", "extra_1", Some("weapon_rack")), "the spot is not open");
    assert!(reopened.buy(&content, "camp_spot:extra_1"));
    assert!(reopened.decorate(&content, "alliance", "extra_1", Some("weapon_rack")));
    let told = Saves::open(&path).unwrap().achievements_told(&content);
    assert_eq!(told.on_spot("horde", "theme"), Some("war_camp"));
    assert_eq!(told.on_spot("alliance", "theme"), None, "each camp its own");
    assert_eq!(told.on_spot("alliance", "extra_1"), Some("weapon_rack"));
    assert!(reopened.decorate(&content, "alliance", "extra_1", None));
    assert_eq!(reopened.achievements_told(&content).on_spot("alliance", "extra_1"), None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The gold counter's next step: what is left to collect.
#[test]
fn the_gold_counter_says_what_is_left_to_its_next_step() {
    let content = content();
    let (dir, path) = temp_save("gold-step");
    let saves = Saves::open(&path).unwrap();
    saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
    assert_eq!(saves.to_next_step(&content, WarbandCounter::GoldCollected), Some(2500));
    saves.add_achievement_facts(&content, &AchievementFacts { gold: 2600, ..Default::default() });
    assert_eq!(saves.to_next_step(&content, WarbandCounter::GoldCollected), Some(47_400));
    assert_eq!(saves.to_next_step(&content, WarbandCounter::ZonesCleared), None, "not a live counter");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every tabard's, weapon glow's and camp decoration's icon is a texture of the player's client.
#[test]
fn every_cosmetic_icon_is_in_the_players_data() {
    let data = benilla_formats::wow_data_or_skip!();
    let chain = benilla_formats::open_chain(&data).unwrap();
    let c = content();
    let icons = c.tabards.iter().map(|t| (&t.id, &t.icon))
        .chain(c.weapon_glows.iter().map(|g| (&g.id, &g.icon)))
        .chain(c.decorations.pieces.iter().map(|d| (&d.id, &d.icon)));
    for (id, icon) in icons {
        let path = format!(r"Interface\Icons\{icon}.blp");
        assert!(chain.contains(&path), "{id}: {path}");
    }
}

/// A bounty is earned again and again: a tally once for each step of the warband's count it
/// passes, a run's bounty each time a run brings it; its points go to spend, never the score.
#[test]
fn bounties_are_earned_again_and_give_points_to_spend_only() {
    let content = content();
    let mut s = SavedAchievements::default();
    let earn = |s: &mut SavedAchievements, facts: AchievementFacts| {
        s.add(&content, &BTreeSet::new(), &facts, 7);
        s.earn_bounties(&content, &facts, 7)
    };
    assert!(earn(&mut s, AchievementFacts { kills: 2_499, ..Default::default() }).is_empty());
    assert_eq!(earn(&mut s, AchievementFacts { kills: 1, ..Default::default() }), ["slayers_tally"]);
    assert_eq!(earn(&mut s, AchievementFacts { kills: 5_000, ..Default::default() }), ["slayers_tally", "slayers_tally"]);
    assert_eq!(s.bounties["slayers_tally"], BountyRecord { times: 3, last: 7 });
    let won = || AchievementFacts { bounties: vec!["zone_victor".into()], ..Default::default() };
    assert_eq!(earn(&mut s, won()), ["zone_victor"]);
    assert_eq!(earn(&mut s, won()), ["zone_victor"], "the next run's win earns it again");
    let points = 3 * content.bounty("slayers_tally").unwrap().points + 2 * content.bounty("zone_victor").unwrap().points;
    assert_eq!(s.bounty_points(&content), points);
    assert_eq!(s.balance(&content), s.score(&content) + points, "to spend");
}
