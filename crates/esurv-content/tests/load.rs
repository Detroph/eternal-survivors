//! The shipped content loads, validates, and carries the values it ships with.

use esurv_content::{Content, EnemyType, Faction, SpellCategory, SpellEffect, SpellTag, Stat, TalentEffect, WeatherKind};

fn content() -> Content {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
    Content::load(dir).expect("content loads and validates")
}

#[test]
fn shipped_content_counts() {
    let c = content();
    assert_eq!(c.zones.len(), 10);
    // 124, and the ones only the bosses bring: Elwynn's and Duskwood's
    // four, the Scarlet Trainees, the Nightmare Suppressors, the Eyes of Immol'thar, Jeklik's bat
    // riders, Venoxis's cobras and the Shades of Jin'do, and the 34 Stratholme's, Blackrock's and
    // Ahn'Qiraj's bring (4, 17, 13)
    assert_eq!(c.zones.iter().map(|z| z.enemies.len()).sum::<usize>(), 168);
    assert_eq!(c.zones.iter().map(|z| z.bosses.len()).sum::<usize>(), 54);
    assert_eq!(c.classes.len(), 9);
    assert_eq!(c.abilities.len(), 168);
    assert_eq!(c.talent_trees.len(), 27);
    assert_eq!(c.talent_trees.iter().map(|t| t.talents.len()).sum::<usize>(), 351);
    assert_eq!(c.jewelry.len(), 22);
    assert_eq!(c.jewelry.iter().filter(|j| j.slot == esurv_content::JewelrySlot::Trinket).count(), 13);
    assert_eq!(c.run_options.len(), 4);
    assert_eq!(c.pets.len(), 26, "25 and the assist's Gargoyle");
    assert_eq!(c.races.len(), 8);
}

#[test]
fn the_enemy_stat_formula_loads() {
    let c = content();
    let b = &c.balance;
    assert_eq!((b.base_health, b.base_damage, b.zone_scaling.damage_share, b.zone_scaling.elite_boss_damage_share), (240.0, 8.0, 0.6, 0.45));
    assert_eq!((b.zone_scaling.normal_health.len(), b.zone_scaling.normal_health_at(10.0)), (12, 8.55));
    assert_eq!(b.zone_scaling.gear_drop_chance.first(), Some(&(1.0, 1.0)));
    assert_eq!(b.zone_scaling.gear_tier_starts, [2.0, 4.0, 7.0, 9.0]);
    let kinds = [EnemyType::Normal, EnemyType::Elite, EnemyType::Rare, EnemyType::Boss, EnemyType::FinalBoss]
        .map(|k| b.multipliers(k))
        .map(|m| (m.health, m.damage));
    assert_eq!(kinds, [(1.0, 1.0), (10.0, 3.0), (16.0, 3.0), (24.0, 2.0), (28.0, 1.0)]);
    assert_eq!(b.health_clock, [(0.0, 1.0), (4.0, 2.5), (8.0, 7.0), (12.0, 18.0), (16.0, 36.0), (20.0, 65.0)]);
    assert_eq!(b.damage_clock, [(0.0, 1.0), (4.0, 1.5), (8.0, 2.5), (12.0, 4.5), (16.0, 7.0)]);
    let d: Vec<f32> = c.zones.iter().map(|z| z.difficulty).collect();
    assert_eq!(d, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
}

#[test]
fn every_normal_enemy_s_toughness_is_a_quarter_step() {
    for z in &content().zones {
        for e in z.enemies.iter().filter(|e| !e.elite) {
            assert!(e.toughness >= 0.25 && (e.toughness * 4.0).fract() == 0.0, "{} {}: {}", z.id, e.name, e.toughness);
        }
    }
}

#[test]
fn every_zone_s_final_bosses_come_last_and_together() {
    for z in &content().zones {
        // the Scarlet Monastery and Stratholme have two (Mograine and
        // Whitemane, Ramstein and the Baron); every other zone one
        let finals: Vec<_> = z.bosses.iter().filter(|b| b.is_final).collect();
        assert_eq!(finals.len(), if matches!(z.id.as_str(), "scarlet" | "strat") { 2 } else { 1 }, "{}", z.id);
        assert!(z.bosses.last().unwrap().is_final, "{}", z.id);
        assert!(finals.iter().all(|b| b.spawn_minute == finals[0].spawn_minute), "{}", z.id);
    }
}

/// The original bosses the 1.12 client gives a pull voice line carry it (docs/game/bosses.md,
/// section 5); the unvoiced and Warsong's hand-made characters carry none.
#[test]
fn voiced_bosses_carry_their_pull_voice_line() {
    let c = content();
    let sound = |zone: &str, name: &str| c.zone(zone).unwrap().bosses.iter().find(|b| b.name == name).unwrap().aggro_sound;
    assert_eq!(sound("elwynn", "Edwin VanCleef"), Some(5780));
    assert_eq!(sound("arena", "Hakkar the Soulflayer"), Some(8414));
    assert_eq!(sound("elwynn", "Hogger"), None);
    assert_eq!(sound("feralas", "King Gordok"), None);
    let voiced: usize = c.zones.iter().map(|z| z.bosses.iter().filter(|b| b.aggro_sound.is_some()).count()).sum();
    // Jeklik's only line is her transform line: muted
    assert_eq!(sound("arena", "High Priestess Jeklik"), None);
    // Rajaxx speaks only as he joins after his waves, Nefarian's phase 1 opens with Nefarius's
    // "Let the games begin!" (the boss mechanics' rework)
    assert_eq!(sound("aq", "General Rajaxx"), None);
    assert_eq!(sound("brm", "Nefarian"), Some(8280));
    assert_eq!(voiced, 19);
    assert!(c.zone("wsg").unwrap().bosses.iter().all(|b| b.aggro_sound.is_none()));
}

/// The Scarlet Monastery: its doorway walled, its cathedral door shut
/// until minute 10, its two last bosses at their own spots: Mograine with half a final boss's health
/// (raised at half of it), Whitemane, whom only his fall brings, with 75 %.
#[test]
fn the_scarlet_monastery_walls_its_courtyard_and_opens_its_cathedral() {
    let c = content();
    let sm = c.zone("scarlet").unwrap();
    assert_eq!((sm.map_id, sm.area_id, sm.difficulty), (189, 796, 4.0));
    assert_eq!(sm.walls.len(), 1);
    assert_eq!(sm.doors.len(), 1);
    assert_eq!(sm.doors[0].opens_at_minute, 10.0);
    let finals: Vec<_> = sm.bosses.iter().filter(|b| b.is_final).collect();
    assert!(finals.iter().all(|b| b.at.is_some()));
    assert_eq!(finals.iter().map(|b| (b.health_share, b.called)).collect::<Vec<_>>(), [(0.5, false), (0.75, true)]);
    // every other boss has no spot of its own and its whole health, but those fighting in a pair
    // (Cho'Rush joining King Gordok, Mandokir beside Ohgan)
    let pairs = [("Cho'Rush the Observer", 0.35), ("Bloodlord Mandokir", 0.7), ("Ohgan", 0.3)];
    for b in c.zones.iter().flat_map(|z| &z.bosses).filter(|b| b.at.is_none()) {
        let share = pairs.iter().find(|(n, _)| *n == b.name).map_or(1.0, |p| p.1);
        assert_eq!(b.health_share, share, "{}", b.name);
    }
}

#[test]
fn spot_values() {
    let c = content();
    let elwynn = c.zone("elwynn").unwrap();
    assert_eq!((elwynn.map_id, elwynn.area_id, elwynn.hour), (0, 12, 10));
    assert_eq!(elwynn.difficulty, 1.0);
    let vermin = &elwynn.enemies[0];
    assert_eq!((vermin.name.as_str(), vermin.toughness, vermin.move_speed), ("Kobold Vermin", 0.5, 6.0));
    let geomancer = &elwynn.enemies[3];
    assert_eq!(geomancer.ranged.unwrap().look_spell, 133);
    let minutes: Vec<f32> = elwynn.bosses.iter().map(|b| b.spawn_minute).collect();
    assert_eq!(minutes, [3.0, 6.0, 9.0, 12.0, 16.0]);

    let aq = c.zone("aq").unwrap();
    assert_eq!((aq.weather.kind, aq.difficulty), (WeatherKind::Sandstorm, 10.0));

    let mortal = c.ability("mortal_strike").unwrap();
    assert_eq!((mortal.damage, mortal.cooldown, mortal.range), (300.0, 6.0, 7.0));
    assert_eq!(c.ability("kidney_shot").unwrap().effect, SpellEffect::Stun { seconds: 2.5 });

    let warrior = c.class("warrior").unwrap();
    assert_eq!((warrior.base_health, warrior.default_start_spell()), (240.0, "slam"));
    assert_eq!(warrior.auto_attack.unwrap().damage, 28.0);
    assert!(c.classes.iter().all(|k| k.spell_damage_scale == 1.0));

    let arms = c.trees_of("warrior").next().unwrap();
    assert_eq!(arms.talents[0].effects, [TalentEffect::Stat { stat: Stat::WeaponDamage, per_rank: 0.05 }]);
}

#[test]
fn races_carry_their_passives_and_spell() {
    let c = content();
    let dwarf = c.race("dwarf").unwrap();
    assert_eq!((dwarf.wow_race_id, dwarf.spell.as_str(), dwarf.faction), (3, "stoneform", Faction::Alliance));
    assert_eq!(dwarf.passives.iter().map(|p| (p.stat, p.value)).collect::<Vec<_>>(), [(Stat::Damage, 0.05), (Stat::PickupRange, 0.3)]);
    let undead = c.race_by_wow_id(5).unwrap();
    assert_eq!((undead.faction, undead.faction_template), (Faction::Horde, 5));
    assert_eq!(c.race("gnome").unwrap().faction_template, 115);
}

#[test]
fn run_options_step_and_clamp() {
    let c = content();
    let speed = c.run_options.iter().find(|o| o.id == "speed").unwrap();
    assert_eq!(speed.stepped(100, false), 100);
    assert_eq!(speed.stepped(145, true), 150);
    assert_eq!(speed.stepped(150, true), 150);
    let density = &c.run_options[0];
    // density's reward weight is 0.1
    assert_eq!((density.max_pct, density.step_pct, density.reward_weight), (500, 25, 0.1));
}

#[test]
fn the_spell_template_is_a_valid_spell() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/game/templates/spell.ron");
    let text = std::fs::read_to_string(path).unwrap();
    let spell: esurv_content::Ability = ron::from_str(&text).expect("docs/game/templates/spell.ron parses as a spell");
    assert_eq!(spell.id, "my_new_spell");
    assert_eq!(spell.categories, [SpellCategory::DirectDamage, SpellCategory::Ranged]);
    assert_eq!(spell.name_ref, esurv_content::NameRef::from("My New Spell"), "our own name is text");
    // where the client holds the name, the template's other form: a pointer
    let pointed = text.replacen("name: \"My New Spell\",", "name: Spell(133),", 1);
    let spell: esurv_content::Ability = ron::from_str(&pointed).expect("the template parses with a pointer for its name");
    assert_eq!(spell.name_ref, esurv_content::ClientName::Spell(133).into());
}

#[test]
fn every_spell_has_a_category_and_the_shipped_tuning_changes_nothing() {
    let c = content();
    for a in &c.abilities {
        assert!(!a.categories.is_empty(), "{} has no category", a.id);
    }
    assert_eq!(c.spell_categories.len(), 12);
    assert!(c.spell_categories.iter().all(|t| t.multipliers() == [1.0; 7]));
    let corruption = c.ability("corruption").unwrap();
    assert_eq!(corruption.categories, [SpellCategory::DamageOverTime]);
    assert_eq!((corruption.damage, corruption.duration, corruption.ticks), (220.0, 8.0, 8));
}

#[test]
fn the_boss_template_is_a_valid_boss() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/game/templates/boss.ron");
    let text = std::fs::read_to_string(path).unwrap();
    let boss: esurv_content::BossEntry = ron::from_str(&text).expect("docs/game/templates/boss.ron parses as a boss");
    assert_eq!(boss.abilities.len(), 5);
}

/// Roster entries and boss abilities take their numbers from `enemy_kinds.ron` by kind, and the
/// resolved values are the ones play has always used.
#[test]
fn kinds_resolve_to_the_standard_numbers() {
    use esurv_content::{BossAbility, BossAbilityKind as K};
    let c = content();
    let elwynn = c.zone("elwynn").unwrap();
    let numbers = |name: &str| {
        let e = elwynn.enemies.iter().find(|e| e.name == name).unwrap();
        (e.move_speed, e.attack_interval, e.spawn_cost, e.elite, e.ranged.map(|r| (r.look_spell, r.range, r.cast_time)))
    };
    assert_eq!(numbers("Kobold Vermin"), (6.0, 1.5, 1.0, false, None));
    assert_eq!(numbers("Young Wolf"), (8.0, 1.5, 1.0, false, None));
    assert_eq!(numbers("Riverpaw Brute"), (5.0, 2.0, 3.0, false, None));
    assert_eq!(numbers("Prowler"), (8.0, 2.0, 2.0, false, None));
    assert_eq!(numbers("Kobold Geomancer"), (5.0, 3.0, 2.0, false, Some((133, 18.0, 1.5))));
    // every zone's first fodder takes its kind's speed; the Silithid Swarmer is fast fodder
    let firsts: Vec<f32> = c.zones.iter().map(|z| z.enemies[0].move_speed).collect();
    assert_eq!(firsts, [6.0, 6.0, 6.0, 6.0, 6.0, 6.0, 6.0, 6.0, 6.0, 8.0]);
    // every boss moves at the one boss speed, but slow Stitches (a boss
    // to kite) and the creeping Eye of C'Thun (its beam sweeps
    // round where it stands)
    assert!(c.zones.iter().flat_map(|z| &z.bosses).filter(|b| b.name != "Stitches" && b.name != "C'Thun").all(|b| b.move_speed == 6.0));
    let elites: Vec<f32> = elwynn.enemies.iter().filter(|e| e.elite).map(|e| e.move_speed).collect();
    assert_eq!(elites, [5.5; 3]);

    // the reworked bosses fight with the building blocks; a Summon
    // still takes the standard adds by the boss's place
    let kinds = c.enemy_kinds.as_ref().unwrap();
    assert_eq!(kinds.boss_abilities.summon.adds_by_position, [6, 7, 8, 10, 12]);
    let all: Vec<&BossAbility> = c.zones.iter().flat_map(|z| &z.bosses).flat_map(|b| &b.abilities).collect();
    for a in all {
        match a.kind {
            K::Stomp { radius, damage_mult, .. } => assert_eq!((radius, damage_mult, a.cooldown), (10.0, 2.0, 8.0)),
            K::Volley { count, damage_mult, .. } => assert_eq!((count, damage_mult, a.cooldown), (6, 0.5, 6.0)),
            K::GroundBurst { radius, delay, damage_mult, .. } => {
                assert_eq!((radius, delay, damage_mult, a.cooldown), (10.0, 2.0, 3.0, 6.0))
            }
            K::Charge { damage_mult, .. } => assert_eq!((damage_mult, a.cooldown), (2.0, 8.0)),
            K::Summon { .. } => assert_eq!(a.cooldown, 15.0),
            // the building blocks take their own numbers
            K::Marked { .. } | K::Rush { .. } | K::Lay { .. } | K::Zone { .. } => {}
        }
    }
}

#[test]
fn every_spell_is_tagged_by_its_shape_and_effect() {
    let c = content();
    assert_eq!(c.balance.spell_tags, esurv_content::SpellTagCoefficients::default(), "all coefficients are 1 for now");
    // race spells stay outside the tag system and are tuned one by one
    let race_spells: Vec<&str> = c.races.iter().map(|r| r.spell.as_str()).collect();
    for a in &c.abilities {
        if race_spells.contains(&a.id.as_str()) {
            assert!(a.tags.is_empty(), "{} is a race spell, so it has no tags", a.id);
            continue;
        }
        let damage_tags = a.tags.iter().filter(|t| t.is_damage()).count();
        if a.damage > 0.0 {
            assert_eq!(damage_tags, 1, "{} deals damage, so it has exactly one damage tag: {:?}", a.id, a.tags);
        } else {
            assert_eq!(damage_tags, 0, "{} deals no damage, so it has no damage tag: {:?}", a.id, a.tags);
        }
        let heals = matches!(a.effect, SpellEffect::Heal { .. } | SpellEffect::HealFromDamage { .. } | SpellEffect::HealingCharges { .. });
        let buffs = matches!(a.effect, SpellEffect::DamageBuff { .. } | SpellEffect::HasteBuff { .. });
        let defends = matches!(
            a.effect,
            SpellEffect::Shield { .. } | SpellEffect::Weaken { .. } | SpellEffect::Thorns { .. } | SpellEffect::Immunity { .. }
        );
        // a spell that gives no damage, healing, buff or defence (energy, rage, speed) has none
        assert!(!a.tags.is_empty() || !(a.damage > 0.0 || heals || buffs || defends), "{} has no tags", a.id);
        assert_eq!(a.tags.contains(&SpellTag::Sustain), heals, "{}: Sustain marks the spells that heal", a.id);
        assert_eq!(a.tags.contains(&SpellTag::Buffs), buffs, "{}: Buffs marks the damage and haste buffs", a.id);
        assert_eq!(a.tags.contains(&SpellTag::Defensive), defends, "{}: Defensive marks shields, weakening, auras, immunity", a.id);
    }
    // the starting options' spells (and what their passives cast) are the Level1 spells, and only those are
    let starts: Vec<&str> = c.classes.iter().flat_map(|k| &k.starting_options).flat_map(|o| c.starting_spells(o)).collect();
    for id in &starts {
        let a = c.ability(id).unwrap();
        assert!(a.damage <= 0.0 || a.tags.contains(&SpellTag::Level1), "{id}");
    }
    for a in c.abilities.iter().filter(|a| a.tags.contains(&SpellTag::Level1)) {
        assert!(starts.contains(&a.id.as_str()), "{} is Level1 but no starting option names it", a.id);
    }
}

/// Zones are placed by difficulty alone: a zone file added with D 1.5 lands between `duskwood` and
/// Warsong, one with D 9 comes last, and no code knows of either.
#[test]
fn a_zone_added_as_a_file_takes_its_place_by_difficulty() {
    use esurv_content::testing::{content_folder_with_zones, ExtraZone};
    let shipped = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
    let zones = [
        ExtraZone { id: "test_middle", name: "Test Middle", difficulty: 2.5 },
        ExtraZone { id: "test_end", name: "Test End", difficulty: 11.0 },
    ];
    let dir = content_folder_with_zones(shipped.as_ref(), "added-zones", &zones).unwrap();
    let c = Content::load(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let c = c.expect("content with added zones loads");
    let ids: Vec<&str> = c.zones.iter().map(|z| z.id.as_str()).collect();
    assert_eq!(ids, ["elwynn", "duskwood", "test_middle", "wsg", "scarlet", "sunken_temple", "feralas", "arena", "strat", "brm", "aq", "test_end"]);
    let middle = c.zone("test_middle").unwrap();
    assert_eq!((middle.enemies.len(), middle.bosses.len()), (3, 2));
    assert!(middle.enemies[2].ranged.is_some(), "the caster takes its kind's numbers");
}

/// The zone template, copied into the zones folder under its id, is a zone that loads in its
/// place by difficulty.
#[test]
fn the_zone_template_is_a_zone_that_loads_in_its_place() {
    use esurv_content::testing::content_folder_with_zones;
    let shipped = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
    let template = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/game/templates/zone.ron");
    let dir = content_folder_with_zones(shipped.as_ref(), "zone-template", &[]).unwrap();
    std::fs::copy(template, dir.join("zones").join("my_zone.ron")).unwrap();
    let c = Content::load(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let c = c.expect("docs/game/templates/zone.ron loads as content/zones/my_zone.ron");
    assert_eq!(c.zones.iter().position(|z| z.id == "my_zone"), Some(2), "between Area 10 and Warsong");
}

/// A template file under `docs/game/templates/`, parsed as one record of type `T`.
fn template<T: for<'de> serde::Deserialize<'de>>(name: &str) -> T {
    let path = format!("{}/../../docs/game/templates/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap();
    ron::from_str(&text).unwrap_or_else(|e| panic!("docs/game/templates/{name} parses: {e}"))
}

/// `c` with its lookups rebuilt, after records were added to its lists.
fn rebuilt(c: Content) -> Content {
    let mut out = Content::from_parts(
        c.zones,
        c.classes,
        c.abilities,
        c.talent_trees,
        c.jewelry,
        c.run_options,
        c.pets,
        c.gear,
        c.amulets,
        c.legendary_amulets,
    );
    out.spell_categories = c.spell_categories;
    out.races = c.races;
    out.outfit_weapons = c.outfit_weapons;
    out.loadouts = c.loadouts;
    out.pvp_looks = c.pvp_looks;
    out.words = c.words;
    out.balance = c.balance;
    out.enemy_kinds = c.enemy_kinds;
    out
}

/// The class and gear set templates, with three talent trees, make a class the content accepts.
#[test]
fn the_class_and_gear_set_templates_make_a_valid_class() {
    let mut c = content();
    let class: esurv_content::Class = template("class.ron");
    let set: esurv_content::GearSet = template("gear_set.ron");
    assert_eq!((class.id.as_str(), set.class.as_str()), ("my_class", "my_class"));
    let trees: Vec<_> = c
        .trees_of("mage")
        .cloned()
        .map(|mut t| {
            t.class = class.id.clone();
            t
        })
        .collect();
    c.talent_trees.extend(trees);
    c.classes.push(class);
    c.gear.sets.push(set);
    let c = rebuilt(c);
    c.validate().expect("the class template validates");
    assert!(c.class("my_class").is_some());
}

/// The talent template, added to a tree, validates.
#[test]
fn the_talent_template_is_a_valid_talent() {
    let mut c = content();
    let talent: esurv_content::Talent = template("talent.ron");
    assert_eq!(talent.id, "my_new_talent");
    assert!(talent.text.contains("{v}"));
    c.talent_trees[0].talents.push(talent);
    rebuilt(c).validate().expect("the talent template validates");
}

/// The ring or trinket template parses and names a stat its line can show.
#[test]
fn the_jewelry_template_is_a_valid_ring() {
    let mut c = content();
    let ring: esurv_content::Jewelry = template("jewelry.ron");
    assert_eq!((ring.id.as_str(), ring.slot), ("my_new_ring", esurv_content::JewelrySlot::Ring));
    assert!(ring.text.contains("{v}"));
    c.jewelry.push(ring);
    rebuilt(c).validate().expect("the jewelry template validates");
}

/// The amulet template validates: its spells exist and words.ron has a line for every stat it gives.
#[test]
fn the_amulet_template_is_a_valid_amulet() {
    let mut c = content();
    let amulet: esurv_content::Amulet = template("amulet.ron");
    assert_eq!(amulet.id, "my_new_amulet");
    c.amulets.push(amulet);
    rebuilt(c).validate().expect("the amulet template validates");
}

/// The action bar and the spellbook hold each of the hero's spells as its look (`Spell.dbc` id),
/// and the client keys each button's icon, tooltip and cooldown sweep by it: two spells one hero
/// can hold at once (a starting option's spells, its card pool, the race's spell) wearing the
/// same look would show as one.
#[test]
fn the_spells_one_hero_can_hold_wear_different_looks() {
    let c = content();
    let mut clashes = Vec::new();
    for class in &c.classes {
        for option in &class.starting_options {
            let mut held: Vec<&str> = option.spells.iter().map(String::as_str).collect();
            held.extend(class.card_pool(Some(&option.id)).iter().map(String::as_str));
            held.sort_unstable();
            held.dedup();
            let race_spells: Vec<&str> = c.races.iter().map(|r| r.spell.as_str()).collect();
            for (i, a) in held.iter().enumerate() {
                // a hero has one race: its spell against the class's, never another race's
                for b in held[i + 1..].iter().chain(race_spells.iter().filter(|r| !held.contains(r))) {
                    let (Some(x), Some(y)) = (c.ability(a), c.ability(b)) else {
                        continue;
                    };
                    if x.look_spell != 0 && x.look_spell == y.look_spell {
                        clashes.push(format!("{}/{}: {a} and {b} both look like {}", class.id, option.id, x.look_spell));
                    }
                }
            }
        }
    }
    assert!(clashes.is_empty(), "{}", clashes.join("\n"));
}

/// A zone with a roster for each faction (Warsong Gulch fights the other
/// faction): each roster loads with entries of its own, and the zone as a faction fights it takes
/// that faction's roster; a party fights as the faction most of its heroes share, the first
/// hero's on a tie.
#[test]
fn a_zone_with_a_roster_for_each_faction_loads_both() {
    use esurv_content::testing::{content_folder_with_faction_zone, ExtraZone};
    use esurv_content::{party_faction, Faction};
    let shipped = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
    let zone = ExtraZone { id: "test_factions", name: "Test Factions", difficulty: 11.0 };
    let dir = content_folder_with_faction_zone(shipped.as_ref(), "faction-rosters", zone).unwrap();
    let c = Content::load(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let c = c.expect("a zone with faction rosters loads");
    let z = c.zone("test_factions").unwrap();
    let rosters = z.by_faction.as_ref().expect("its rosters");
    assert_eq!(rosters.vs_alliance.enemies[0].name, "Horde Grunt");
    assert_eq!(rosters.vs_horde.bosses[0].name, "Alliance Marshal");
    // no two creatures share an entry, the rosters' included
    let mut entries: Vec<u32> = c.zones.iter().flat_map(|z| z.every_enemy().map(|e| e.entry).chain(z.every_boss().map(|b| b.entry))).collect();
    let all = entries.len();
    entries.sort_unstable();
    entries.dedup();
    assert_eq!(entries.len(), all);
    assert_eq!(z.for_faction(Faction::Alliance).enemies[0].name, "Horde Grunt");
    assert_eq!(z.for_faction(Faction::Horde).bosses[0].name, "Alliance Marshal");
    // a zone without rosters is the same for both
    let elwynn = c.zone("elwynn").unwrap();
    assert_eq!(elwynn.for_faction(Faction::Horde).enemies[0].name, elwynn.enemies[0].name);
    use Faction::{Alliance, Horde};
    assert_eq!(party_faction(&[Horde]), Horde);
    assert_eq!(party_faction(&[Alliance, Horde, Horde]), Horde);
    assert_eq!(party_faction(&[Horde, Alliance]), Horde, "a tie: the first hero's");
    assert_eq!(party_faction(&[]), Alliance);
}

#[test]
fn finer_prestige_steps_show_their_digits() {
    let c = content();
    let stat = |id: &str| c.prestige.iter().find(|p| p.id == id).unwrap();
    assert_eq!(stat("lifesteal").bonus_text(1), "+0.25%");
    assert_eq!(stat("lifesteal").bonus_text(3), "+0.75%");
    assert_eq!(stat("move_speed").bonus_text(3), "+1.5%");
    assert_eq!(stat("double_cast").bonus_text(2), "+1%");
    assert_eq!(stat("damage").bonus_text(4), "+4%");
    assert!(stat("swing_speed").classes.iter().any(|c| c == "shaman"));
}

/// Every class has its three weapon loadouts (roadmap R3): the swinging
/// classes' each swing, two-handers at 3.6 s for twice the damage, one-handers at 1.8 s, dual
/// wield a slow main hand and a fast off hand at half damage; the casters' never swing; a loadout
/// id the class lacks falls back to its first.
#[test]
fn every_class_has_three_loadouts_with_their_swings() {
    use esurv_content::{Hand, WeaponKind, WeaponSwing};
    let c = content();
    for class in &c.classes {
        let all = c.loadouts_of(&class.id);
        assert_eq!(all.len(), 3, "{}", class.id);
        for l in all {
            assert_eq!(!l.swings().is_empty(), class.auto_attack.is_some(), "{}/{}", class.id, l.id);
            assert!(!l.strength.is_empty() && !l.drawback.is_empty(), "{}/{}", class.id, l.id);
        }
        assert_eq!(c.loadout(&class.id, Some("no_such_loadout")).map(|l| &l.id), all.first().map(|l| &l.id));
    }
    let swing = |class: &str, id: &str| c.loadout(class, Some(id)).unwrap().swings();
    let at = |seconds, share| WeaponSwing { seconds, share };
    assert_eq!(swing("warrior", "two_hander"), [at(3.6, 2.0)]);
    assert_eq!(swing("warrior", "dual_wield"), [at(2.6, 1.0), at(1.6, 0.4)]);
    assert_eq!(swing("warrior", "shield"), [at(1.8, 1.0)]);
    assert_eq!(swing("rogue", "daggers"), [at(1.4, 1.0), at(1.4, 0.5)]);
    assert_eq!(swing("hunter", "bow"), [at(2.2, 0.9)]);
    assert_eq!(swing("hunter", "crossbow"), [at(3.4, 1.6)]);
    let gun = c.loadout("hunter", Some("gun")).unwrap();
    assert_eq!(gun.weapon(Hand::Ranged).map(|w| w.kind), Some(WeaponKind::Gun));
    assert_eq!(c.loadout("paladin", Some("shield")).unwrap().weapon(Hand::OffHand).map(|w| w.kind), Some(WeaponKind::Shield));
}

/// Every zone has its own look for every class, eight armor pieces
/// named and with a look, and its own armaments, a look for each weapon of the class's three
/// loadouts. Blackrock and Stratholme have a second row of both; Naxxramas's looks wait in the content for its
/// zone.
#[test]
fn every_zone_has_each_class_s_armor_and_armament_looks() {
    let c = content();
    let rows = |zone: &str| if zone == "brm" || zone == "strat" { 2 } else { 1 };
    for z in &c.zones {
        for class in &c.classes {
            let set = c.gear_set(&class.id).unwrap();
            for row in 0..rows(&z.id) {
                let look = &set.looks[set.look_of(&z.id, row).unwrap_or_else(|| panic!("{} has no {} row {row}", class.id, z.id))];
                assert_eq!(look.names.len(), 8, "{} {}", class.id, z.id);
                assert!(look.names.iter().all(|n| !n.trim().is_empty()), "{} {}", class.id, z.id);
                // Elwynn keeps the Recruit's starting look, a 0 borrowing the next zone's piece
                let looks = look.displays.iter().filter(|&&d| d != 0).count();
                assert!(looks == 8 || (z.id == "elwynn" && looks >= 7), "{} {}: {looks} looks", class.id, z.id);
                let loadouts = c.loadouts_of(&class.id);
                assert_eq!(loadouts.len(), 3);
                for l in loadouts {
                    for w in &l.weapons {
                        let a = w.looks.iter().find(|a| a.zone == z.id && a.row == row).unwrap();
                        assert!(a.display != 0 && !a.name.is_empty(), "{}/{} {} {}", class.id, l.id, z.id, row);
                    }
                }
            }
            assert!(set.look_of(&z.id, rows(&z.id)).is_none(), "{} {}: no more rows", class.id, z.id);
        }
    }
    let warrior = c.gear_set("warrior").unwrap();
    let names = |zone, row| &warrior.looks[warrior.look_of(zone, row).unwrap()].names[0];
    assert_eq!([names("elwynn", 0), names("scarlet", 0), names("brm", 0), names("brm", 1)], ["Recruit's Helm", "Adventurer's Helm", "Helm of Might", "Helm of Wrath"]);
    assert!(warrior.look_of("naxxramas", 0).is_some(), "Naxxramas's looks are kept for its zone");
    // Stratholme's second row is named "Scholomance sets" for every
    // class; other rows go unnamed
    for set in &c.gear.sets {
        let row_name = |zone, row| set.looks[set.look_of(zone, row).unwrap()].row_name.as_deref();
        assert_eq!(row_name("strat", 1), Some("Scholomance sets"), "{}", set.class);
        assert_eq!([row_name("strat", 0), row_name("brm", 0), row_name("brm", 1)], [None; 3], "{}", set.class);
    }
    // the druid's polearms keep their own grip in a line of two-handed maces
    let staff = c.loadout("druid", Some("two_hander")).unwrap();
    let harpoon = staff.weapons[0].look_in(&c.gear_set("druid").unwrap().looks[1]).unwrap();
    assert_eq!((harpoon.name.as_str(), staff.weapons[0].kind_of(harpoon)), ("Impaling Harpoon", esurv_content::WeaponKind::Polearm));
}

/// Every weapon loadout has its icon for the warband screen's row (an `Interface\Icons` name, no
/// path), and a class's three differ so the row tells them apart.
#[test]
fn every_loadout_has_its_own_icon() {
    let c = content();
    for class in &c.classes {
        let all = c.loadouts_of(&class.id);
        for l in all {
            assert!(!l.icon.trim().is_empty(), "{}/{} has no icon", class.id, l.id);
            assert!(!l.icon.contains(['\\', '/']), "{}/{}: a bare icon name, no path", class.id, l.id);
        }
        let mut icons: Vec<&str> = all.iter().map(|l| l.icon.as_str()).collect();
        icons.sort_unstable();
        icons.dedup();
        assert_eq!(icons.len(), all.len(), "{}: two loadouts share an icon", class.id);
    }
}

/// Every self-buff spell with a duration and a cooldown lasts 40 % of
/// its cooldown at base values, before any card, talent or cooldown recovery. Race spells are
/// separate and keep their own durations.
#[test]
fn every_self_buff_is_up_forty_percent_of_its_cooldown() {
    let c = content();
    let racial = |id: &str| c.races.iter().any(|r| r.spell == id);
    let buffs: Vec<_> = c
        .abilities
        .iter()
        .filter(|a| a.delivery == esurv_content::Delivery::SelfEffect && a.categories.contains(&SpellCategory::Buff))
        .filter(|a| a.cooldown > 0.0 && a.duration > 0.0 && !racial(&a.id))
        .collect();
    let ids: Vec<&str> = buffs.iter().map(|a| a.id.as_str()).collect();
    for id in ["adrenaline_rush", "slice_and_dice", "tigers_fury", "dash", "power_infusion", "battle_shout", "berserk", "arcane_power"] {
        assert!(ids.contains(&id), "{id} is one of them: {ids:?}");
    }
    // the race spells keep their own durations
    for (id, seconds) in [("perception", 10.0), ("blood_fury", 12.0), ("berserking", 10.0)] {
        assert!(racial(id), "{id} is a race spell");
        assert_eq!(c.ability(id).unwrap().duration, seconds, "{id}");
    }
    for a in buffs {
        assert!((a.duration / a.cooldown - 0.4).abs() < 0.002, "{}: {} of {} sec", a.id, a.duration, a.cooldown);
        let seconds = match a.effect {
            SpellEffect::DamageBuff { seconds, .. } | SpellEffect::HasteBuff { seconds, .. } | SpellEffect::SpeedBuff { seconds, .. } => Some(seconds),
            _ => None,
        };
        assert!(seconds.is_none_or(|s| s == a.duration), "{}: its effect lasts as long", a.id);
    }
}

/// Hand of Justice keeps its double cast and adds a chance for a weapon
/// attack to hit twice, 2 % at a new Common piece's 7.5 % plus 1/5 of every point above it.
#[test]
fn hand_of_justice_hits_twice_by_its_double_cast() {
    let c = content();
    let hoj = c.jewelry.iter().find(|j| j.id == "hoj").unwrap();
    assert_eq!((hoj.stat, hoj.value), (Stat::DoubleCast, 0.05), "its double cast as it was");
    // new Common, Rare, Epic, Legendary (1.5 × the rarity's 1, 1.7, 2.5, 3.2 × 5 %)
    for (rarity, want) in [(1.0, 0.02), (1.7, 0.0305), (2.5, 0.0425), (3.2, 0.053)] {
        let got = hoj.double_hit_at(1.5 * rarity * hoj.value);
        assert!((got - want).abs() < 1e-5, "{rarity}: {got}");
    }
    // at the 60 % double-cast cap it stops, at 12.5 %
    assert!((hoj.double_hit_at(0.6) - 0.125).abs() < 1e-5);
    assert_eq!(hoj.text_at(0.075), "7.5% chance for a spell to cast twice, 2% chance for a weapon attack to hit twice");
    assert!(c.jewelry.iter().filter(|j| j.id != "hoj").all(|j| j.double_hit.is_none() && j.double_hit_at(1.0) == 0.0));
}
