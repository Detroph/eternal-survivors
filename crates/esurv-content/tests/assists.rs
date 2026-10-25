//! The assists: every hand-made character has one (its own or its class's), both factions have
//! characters to summon, a character without a script takes its class's default, broken scripts
//! are refused, and the battle standard's rules load.

use esurv_content::{AssistBehaviour, AssistScript, AssistStep, CastOn, Content, Faction, MoveStyle, PlayerCharacter, PlayerForm, Targets};

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
        ("unknown pet", script(vec![AssistStep::Summon { pet: "Nobody".into(), count: 1, seconds: 5.0 }])),
        ("too long", script(vec![AssistStep::Wait { seconds: 16.0 }])),
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
    p.assist_script = Some(AssistScript { seconds: Some(17.0), ..script(vec![AssistStep::Wait { seconds: 16.0 }]) });
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
    assert_eq!((b.per_run, b.endless_every, b.ally_scale, b.assist_seconds), ((1, 2), 600.0, 2.0, 15.0));
    let rates = b.assist_damage_per_second;
    assert_eq!((rates.normal, rates.elite_rare_boss), (0.05, 0.01));
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

/// Every character has a behaviour (its own or its class's), each of its own, and every rotation
/// spell has its times in assist_spells.ron; only the `LeavesAfterScript` ones cast nothing after
/// their script.
#[test]
fn every_character_has_a_rotation_of_known_spells() {
    let c = content();
    for p in &c.player_characters {
        let b = c.behaviour_of(p).unwrap_or_else(|| panic!("{} has no behaviour", p.id));
        let casts_nothing = ["vodu", "stormwall", "edgar", "phigaro", "eggmike"].contains(&p.id.as_str());
        assert_eq!(b.rotation.is_empty(), casts_nothing, "{}", p.id);
        for id in &b.rotation {
            assert!(c.assist_spell(*id).is_some(), "{}: {id} has no row", p.id);
        }
    }
    assert_eq!(c.characters_on_default_behaviour(), Vec::<(String, String)>::new());
    // Xaryu novas, jumps away and Blizzards (Spell.dbc: Frost Nova 25 sec, Blizzard a channel of 8
    // sec, a tick a second, 8 yd)
    let xaryu = c.behaviour_of(c.player_character("xaryu").unwrap()).unwrap();
    assert_eq!(xaryu.style, MoveStyle::NovaKiteCaster);
    assert_eq!(&xaryu.rotation[..2], &[10230, 10187]);
    assert_eq!(c.assist_spell(10230).map(|s| (s.cast, s.cooldown)), Some((0.0, 25.0)));
    assert_eq!(c.assist_spell(10187).map(|s| (s.on, s.channel, s.tick_every, s.radius)), Some((CastOn::Ground, 8.0, 1.0, 8.0)));
    // Vodu and Eggmike do nothing after their scripts; Stormwall, Edgar and Phigaro only auto attack
    let style = |id: &str| c.behaviour_of(c.player_character(id).unwrap()).unwrap().style;
    assert_eq!(style("vodu"), MoveStyle::LeavesAfterScript);
    assert_eq!(style("eggmike"), MoveStyle::LeavesAfterScript);
    for id in ["stormwall", "edgar", "phigaro"] {
        assert_eq!(style(id), MoveStyle::Melee, "{id}");
    }
}

/// A character without a behaviour takes its class's (and form's) default, and is named for it.
#[test]
fn a_character_without_a_behaviour_takes_its_class_default() {
    let mut c = content();
    let mut cat = c.player_character("pawsz").unwrap().clone();
    cat.id = "pawsz_plain".into();
    cat.behaviour = None;
    c.player_characters.push(cat.clone());
    c.validate().expect("a character on its default behaviour validates");
    assert_eq!(c.behaviour_of(&cat), Some(&c.class_assist("druid", Some(PlayerForm::Cat)).unwrap().behaviour));
    assert_eq!(c.characters_on_default_behaviour(), vec![("pawsz_plain".to_string(), "druid (Cat)".to_string())]);
}

/// A rotation of a spell the table lacks, or any for an ally that leaves after its script, is
/// refused; an empty one is not (auto attacks only).
#[test]
fn broken_behaviours_are_refused() {
    let base = content();
    let with = |style: MoveStyle, rotation: Vec<u32>| {
        let mut c = base.clone();
        let mut p = c.player_character("mir").unwrap().clone();
        p.id = "mir_broken".into();
        p.behaviour = Some(AssistBehaviour { style, rotation });
        c.player_characters.push(p);
        c.validate()
    };
    assert!(with(MoveStyle::Melee, vec![1]).is_err(), "a spell the table lacks");
    assert!(with(MoveStyle::LeavesAfterScript, vec![11294]).is_err(), "a rotation for one that leaves");
    assert!(with(MoveStyle::Melee, vec![]).is_ok(), "auto attacks only");
    assert!(with(MoveStyle::LeavesAfterScript, vec![]).is_ok(), "nothing after its script");
}

/// A script's damage round: its damage and heals once per pass, effects and looks left out, a
/// pick that deals nothing kept for the `Same` after it; nothing for a script without damage.
#[test]
fn the_damage_round_keeps_the_scripts_damage_and_drops_its_effects() {
    let c = content();
    let round = |id: &str| c.assist_of(c.player_character(id).unwrap()).unwrap().damage_round();
    // Wayze: the stun's pick (no effect), then a damaging hit on the 5 nearest
    let wayze = round("wayze");
    assert_eq!(wayze.len(), 2);
    assert!(matches!(&wayze[0], AssistStep::Hit { targets: Targets::OnScreen, damage: false, effects, .. } if effects.is_empty()));
    assert!(matches!(&wayze[1], AssistStep::Hit { targets: Targets::Nearest(5), damage: true, effects, look_spell: None, .. } if effects.is_empty()));
    // Dikkedop: five passes
    assert_eq!(round("dikkedop").len(), 5);
    // Phigaro exposes armor only, Vodu taunts only: no damage to spread
    assert!(round("phigaro").is_empty());
    assert!(round("vodu").is_empty());
    // Cdew heals 30%; Xaryu's Blizzard area keeps its damage
    assert!(matches!(round("cdew")[..], [AssistStep::Heal { share, .. }] if share == 0.3));
    assert!(round("xaryu").iter().any(|s| matches!(s, AssistStep::Area { damage: true, effects, .. } if effects.is_empty())));
    // Snutz's rain of fire keeps its cap of 5
    assert!(round("snutz").iter().any(|s| matches!(s, AssistStep::Area { most: 5, .. })));
}
