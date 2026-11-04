//! The shipped achievements load with the numbers the plan gave them, the template is a valid
//! achievement, and the checks catch every kind of broken reference.

use esurv_content::{Achievement, AchievementCategory, Content, WarbandCounter, Criteria, Goal, Reward, RunCondition, Source};

fn content() -> Content {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
    Content::load(dir).expect("content loads and validates")
}

fn points_in(c: &Content, keep: impl Fn(&Achievement) -> bool) -> u32 {
    c.achievements.iter().filter(|a| keep(a)).map(|a| a.points).sum()
}

#[test]
fn fifty_five_achievements_worth_935_points() {
    let c = content();
    assert_eq!(c.achievements.len(), 55);
    assert_eq!(points_in(&c, |_| true), 935);
    assert_eq!(points_in(&c, |a| matches!(a.category, AchievementCategory::Class(_))), 405);
    assert_eq!(points_in(&c, |a| a.category == AchievementCategory::General), 350);
    assert_eq!(points_in(&c, |a| a.category == AchievementCategory::Feats), 180);
    assert_eq!(c.achievements.iter().filter(|a| a.category == AchievementCategory::General).count(), 20);
    assert_eq!(c.achievements.iter().filter(|a| a.category == AchievementCategory::Feats).count(), 8);
    for class in &c.classes {
        let mine: Vec<&Achievement> = c.achievements.iter().filter(|a| a.category == AchievementCategory::Class(class.id.clone())).collect();
        assert_eq!(mine.len(), 3, "{}", class.id);
        assert_eq!(mine.iter().map(|a| a.points).collect::<Vec<_>>(), [10, 10, 25], "{}", class.id);
        assert!(mine[2].rewards.iter().any(|r| matches!(r, Reward::Companion(_))), "{}'s feat gives a companion", class.id);
    }
}

#[test]
fn the_hidden_feat_of_strength_can_never_be_earned_yet() {
    let c = content();
    let hidden: Vec<&Achievement> = c.achievements.iter().filter(|a| a.hidden).collect();
    assert_eq!(hidden.len(), 1);
    assert_eq!((hidden[0].points, &hidden[0].criteria), (0, &Criteria::Never));
}

#[test]
fn the_shipped_feats_ask_what_was_measured() {
    let c = content();
    let win = |id: &str| match &c.achievement(id).unwrap().criteria {
        Criteria::Win(list) => list.clone(),
        other => panic!("{id} is {other:?}"),
    };
    assert!(win("spin_to_win").contains(&RunCondition::KillingBlows { by: Source::Spells(vec!["whirlwind".into()]), at_least: 100 }));
    assert!(win("let_the_pet_work").contains(&RunCondition::KillingBlows { by: Source::Pets, at_least: 300 }));
    assert!(win("soul_collector").contains(&RunCondition::KillingBlows { by: Source::Spells(vec!["drain_soul".into()]), at_least: 1200 }));
    assert!(win("nine_lives").contains(&RunCondition::DroppedBelow { share: 0.2, back_to: 0.3, times: 9 }));
    assert_eq!(c.achievement("slayer_3").unwrap().criteria, Criteria::Counter { counter: WarbandCounter::Kills, goal: Goal::Count(250_000) });
    assert_eq!(Goal::All.number(WarbandCounter::BossesDefeated, &c), 54);
    assert_eq!(Goal::All.number(WarbandCounter::ZonesCleared, &c), 10);
}

#[test]
fn the_achievement_template_is_a_valid_achievement() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/game/templates/achievement.ron");
    let text = std::fs::read_to_string(path).unwrap();
    let a: Achievement = ron::from_str(&text).expect("docs/game/templates/achievement.ron parses as an achievement");
    assert_eq!(a.id, "my_new_achievement");
    let mut c = content();
    c.achievements.push(a);
    c.validate().expect("the achievement template validates");
}

/// Each broken achievement is told by the checks, by its id.
#[test]
fn a_broken_achievement_is_refused() {
    let base = content();
    let made = |points: &str, category: &str, criteria: &str| -> Achievement {
        let text = format!(r#"(id: "x", name: "X", text: "Do it.", points: {points}, icon: "INV_Misc_Coin_01", category: {category}, criteria: {criteria})"#);
        ron::from_str(&text).unwrap_or_else(|e| panic!("{text}: {e}"))
    };
    let good = made("10", "General", "Win([])");
    let broken = [
        made("15", "General", "Win([])"),
        made("10", r#"Class("bard")"#, "Win([])"),
        made("10", "General", r#"Win([Zone("narnia")])"#),
        made("10", "General", r#"Win([Start("frost_nova")])"#),
        made("10", "General", r#"Win([Loadout(class: "mage", id: "daggers")])"#),
        made("10", "General", r#"Win([KillingBlows(by: Spells(["no_such_spell"]), at_least: 10)])"#),
        made("10", "General", "Win([HealthNeverBelow(1.5)])"),
        made("10", "General", "Win([ChallengeAtLeast(600)])"),
        made("10", "General", r#"Fight((boss: (zone: "elwynn", boss: 9)))"#),
        made("10", "General", "Counter(counter: Kills, goal: All)"),
        made("10", "General", "Counter(counter: Kills, goal: Count(0))"),
        made("10", "General", r#"Checklist(EveryLoadout("bard"))"#),
        made("10", "General", r#"AllOf(["no_such_achievement"])"#),
        made("10", "General", r#"AllOf(["x"])"#),
        made("10", "General", r#"AllOf(["death_knight"])"#),
    ];
    for a in broken {
        let mut c = base.clone();
        c.achievements.push(a.clone());
        let err = c.validate().expect_err(&format!("{a:?} is refused")).to_string();
        assert!(err.contains("achievement x "), "{a:?}: {err}");
    }
    let mut twice = base.clone();
    twice.achievements.push(good.clone());
    twice.achievements.push(good);
    assert!(twice.validate().unwrap_err().to_string().contains("listed twice"));
}

#[test]
fn a_series_of_one_step_or_across_categories_is_refused() {
    let mut c = content();
    let lone: Achievement = ron::from_str(r#"(id: "lone", name: "L", text: "T.", points: 10, icon: "I", category: General, series: Some("alone"), criteria: Win([]))"#).unwrap();
    c.achievements.push(lone);
    assert!(c.validate().unwrap_err().to_string().contains("series alone"));
    let mut c = content();
    let stray: Achievement = ron::from_str(r#"(id: "stray", name: "S", text: "T.", points: 10, icon: "I", category: Feats, series: Some("slayer"), criteria: Win([]))"#).unwrap();
    c.achievements.push(stray);
    assert!(c.validate().unwrap_err().to_string().contains("series slayer spans two categories"));
}
