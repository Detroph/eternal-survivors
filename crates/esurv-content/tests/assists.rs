//! The assists: every hand-made character has one (its own or its class's), both factions have
//! characters to summon, a character without a script takes its class's default, broken scripts
//! are refused, and the battle standard's rules load.

use esurv_content::{AssistScript, AssistStep, Content, Faction, PlayerCharacter, PlayerForm, Targets};

fn content() -> Content {
    Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).expect("content loads and validates")
}

#[test]
fn every_character_has_an_assist_that_validates() {
    let c = content();
    assert!(!c.player_characters.is_empty());
    for p in &c.player_characters {
        let script = c.assist_of(p).unwrap_or_else(|| panic!("{} has no assist", p.id));
        assert!(!script.steps.is_empty(), "{}", p.id);
        assert!(!c.assist_text(p).is_empty(), "{} has no assist text", p.id);
    }
    // the defaults load and validate with the content; each class has one
    for class in &c.classes {
        assert!(c.class_assist(&class.id, None).is_some(), "{}", class.id);
    }
    for form in [PlayerForm::Cat, PlayerForm::Bear, PlayerForm::Moonkin] {
        assert_eq!(c.class_assist("druid", Some(form)).and_then(|a| a.form), Some(form));
    }
    // every hand-made character carries a script of its own
    assert_eq!(c.characters_on_default_assist(), Vec::<(String, String)>::new());
}

#[test]
fn every_faction_has_a_character_to_summon() {
    let c = content();
    for f in [Faction::Alliance, Faction::Horde] {
        assert!(c.player_characters_of(f).count() > 0, "{f:?}");
    }
    assert!(c.player_characters.iter().all(|p| c.character_faction(p).is_some()));
}

/// Character `id` with its own script taken away.
fn without_script(c: &Content, id: &str) -> PlayerCharacter {
    let mut p = c.player_character(id).unwrap().clone();
    p.id = format!("{id}_plain");
    p.assist_script = None;
    p
}

#[test]
fn a_character_without_a_script_takes_its_class_default() {
    let mut c = content();
    let rogue = without_script(&c, "mir");
    let cat = without_script(&c, "pawsz");
    let mut caster = without_script(&c, "regiss");
    caster.form = None;
    caster.assist = String::new();
    c.player_characters.extend([rogue.clone(), cat.clone(), caster.clone()]);
    c.validate().expect("characters on their defaults validate");
    assert_eq!(c.assist_of(&rogue), Some(&c.class_assist("rogue", None).unwrap().script));
    assert_eq!(c.assist_of(&cat), Some(&c.class_assist("druid", Some(PlayerForm::Cat)).unwrap().script));
    assert_eq!(c.assist_of(&caster), Some(&c.class_assist("druid", None).unwrap().script));
    // its own text stays; with none, the default's shows
    assert_eq!(c.assist_text(&rogue), rogue.assist);
    assert_eq!(c.assist_text(&caster), c.class_assist("druid", None).unwrap().text);
    let named = c.characters_on_default_assist();
    let want = [("mir_plain", "rogue"), ("pawsz_plain", "druid (Cat)"), ("regiss_plain", "druid")];
    assert_eq!(named, want.map(|(a, b)| (a.to_string(), b.to_string())).to_vec());
}

#[test]
fn broken_scripts_are_refused() {
    let base = content();
    let broken: [(&str, AssistScript); 5] = [
        ("no steps", AssistScript { arrive: Default::default(), steps: vec![], leave: Default::default(), seconds: None }),
        ("the same before any", script(vec![AssistStep::Taunt { targets: Targets::Same, seconds: 5.0, look_spell: None }])),
        ("unknown pet", script(vec![AssistStep::Summon { pet: "Nobody".into(), count: 1, seconds: 5.0, damage: None }])),
        ("too long", script(vec![AssistStep::Wait { seconds: 11.0 }])),
        (
            "weakens the hero",
            script(vec![AssistStep::Buff {
                stats: vec![(esurv_content::Stat::Damage, -0.1)],
                seconds: 5.0,
                heal: 0.0,
                every: 0.0,
                immune: false,
                look_spell: None,
                totem: None,
            }]),
        ),
    ];
    for (why, s) in broken {
        let mut c = base.clone();
        let mut p = without_script(&c, "mir");
        p.assist_script = Some(s);
        c.player_characters.push(p);
        assert!(c.validate().is_err(), "{why} should be refused");
    }
    // a longer time of its own lets a longer script through
    let mut c = base.clone();
    let mut p = without_script(&c, "mir");
    p.assist_script = Some(AssistScript { seconds: Some(12.0), ..script(vec![AssistStep::Wait { seconds: 11.0 }]) });
    c.player_characters.push(p);
    c.validate().expect("a script within its own time validates");
}

fn script(steps: Vec<AssistStep>) -> AssistScript {
    AssistScript { arrive: Default::default(), steps, leave: Default::default(), seconds: None }
}

#[test]
fn the_battle_standard_rules_load() {
    let c = content();
    let b = &c.balance.battle_standard;
    assert_eq!((b.per_run, b.endless_every, b.ally_scale, b.assist_seconds), ((1, 2), 600.0, 2.0, 10.0));
    assert_eq!((b.display.of(Faction::Alliance), b.display.of(Faction::Horde)), (14541, 14542));
    assert_eq!((b.planted_look.alliance, b.planted_look.horde), (23034, 23035));
    assert!(!c.battle_standards_in("wsg"));
    assert!(c.battle_standards_in(&c.zones[0].id));
}

/// The player character template, added to the list, is a valid character with its own assist.
#[test]
fn the_player_character_template_is_a_valid_character() {
    let mut c = content();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/game/templates/player_character.ron");
    let text = std::fs::read_to_string(path).unwrap();
    let p: PlayerCharacter = ron::from_str(&text).expect("docs/game/templates/player_character.ron parses");
    assert_eq!(p.id, "my_character");
    assert!(p.assist_script.is_some());
    c.player_characters.push(p);
    c.validate().expect("the player character template validates");
    let p = c.player_character("my_character").unwrap();
    assert_eq!(c.character_faction(p), Some(Faction::Horde));
}
