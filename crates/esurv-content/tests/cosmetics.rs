//! Step 3 of the achievements: titles, tabards, weapon glows and camp decorations as content.

use esurv_content::{
    camp_spot_item, decoration_item, glow_item, glows_on, tabard_item, titled, Content, Faction,
    Holdings, SpotKind, TabardLook,
};

fn content() -> Content {
    Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
}

#[test]
fn the_titles_read_as_the_owner_asked() {
    let c = content();
    let names: Vec<String> = c.titles().map(|(_, t)| titled(t, "Lumi")).collect();
    assert_eq!(
        names,
        [
            "Recruit Lumi",
            "Lumi the Eternal",
            "Lumi, Bossbane",
            "Lumi of Nine Paths",
            "Spellbound Lumi",
            "Lumi the Armsmaster",
            "Lumi, Gnollslayer",
            "Lumi the Fivefold",
            "Lumi, Eye of the Old God",
            "Lumi the Unending",
        ]
    );
    assert_eq!(
        c.titled_name("Lumi", Some("well_armed")),
        "Lumi the Armsmaster"
    );
    assert_eq!(
        c.titled_name("Lumi", Some("slayer_1")),
        "Lumi",
        "no title there"
    );
    assert_eq!(c.titled_name("Lumi", None), "Lumi");
}

#[test]
fn the_tabards_swap_by_faction_and_a_factions_own_stays_with_it() {
    let c = content();
    assert_eq!(c.tabards.len(), 14);
    assert!(
        c.tabard("officers").is_some() && c.tabards.iter().all(|t| !t.name.starts_with("Officer"))
    );
    let ws = c.tabard("warsong_silverwing").unwrap().look;
    assert_eq!(
        (
            ws.display_for(Faction::Horde),
            ws.display_for(Faction::Alliance)
        ),
        (Some(32031), Some(32026))
    );
    let sw = c.tabard("stormwind").unwrap().look;
    assert_eq!(
        (
            sw.display_for(Faction::Horde),
            sw.display_for(Faction::Alliance)
        ),
        (None, Some(21338))
    );
    assert!(matches!(
        c.tabard("darkmoon").unwrap().look,
        TabardLook::Neutral { display: 36469 }
    ));
    assert_eq!(c.shop_price(&tabard_item("guild")), Some(15));
    assert_eq!(c.shop_price(&tabard_item("flame")), Some(30));
}

#[test]
fn the_damage_glows_are_a_ladder_and_glows_skip_shields() {
    let c = content();
    assert_eq!(c.weapon_glows.len(), 18);
    let price = |id: &str| c.shop_price(&glow_item(id)).unwrap();
    assert_eq!(
        (
            price("damage_low"),
            price("damage_medium"),
            price("damage_high")
        ),
        (15, 30, 50)
    );
    assert_eq!(
        (price("crusader"), price("feedback"), price("windfury")),
        (50, 30, 30)
    );
    assert_eq!(
        c.shop_needs(&glow_item("damage_high")),
        [glow_item("damage_medium")]
    );
    assert!(c.shop_needs(&glow_item("crusader")).is_empty());
    assert!(glows_on(13) && glows_on(17) && glows_on(15) && glows_on(26));
    assert!(!glows_on(14) && !glows_on(23) && !glows_on(0));
}

#[test]
fn trophies_are_earned_sets_give_their_signpost_and_the_throne_climbs() {
    let c = content();
    let bought = [
        "decoration:weapon_rack",
        "decoration:training_dummy",
        "decoration:battle_banner",
    ];
    let earned = ["twice_as_hard", "lord_of_nothing", "bossbane"];
    let b = |i: &str| bought.contains(&i);
    let e = |a: &str| earned.contains(&a);
    let z = |z: &str| z == "brm";
    let h = Holdings {
        bought: &b,
        earned: &e,
        cleared: &z,
    };
    assert!(c.owns_decoration("onyxia_head", &h) && c.owns_decoration("nefarian_head", &h));
    assert!(!c.owns_decoration("slime_jar", &h) && !c.owns_decoration("troll_skull_pile", &h));
    assert!(
        !c.owns_decoration("sign_war_camp", &h),
        "the powder kegs are missing"
    );
    let all = |i: &str| bought.contains(&i) || i == decoration_item("powder_kegs");
    let h2 = Holdings {
        bought: &all,
        earned: &e,
        cleared: &z,
    };
    assert!(c.owns_decoration("sign_war_camp", &h2));
    let throne = c.decoration("throne").unwrap();
    assert_eq!(
        c.decoration_display(throne, Faction::Horde, &h),
        6690,
        "the highest throne earned"
    );
    assert_eq!(
        c.decoration_display(c.decoration("onyxia_head").unwrap(), Faction::Alliance, &h),
        5745
    );
    assert!(c
        .decorations
        .pieces
        .iter()
        .filter(|p| p.trophy())
        .all(|p| p.price.is_none()));
    assert_eq!(
        c.shop_price(&decoration_item("onyxia_head")),
        None,
        "never sold"
    );
    assert_eq!(c.shop_price(&decoration_item("great_anvil")), Some(50));
    assert_eq!(c.shop_needs(&decoration_item("great_anvil")).len(), 4);
    assert_eq!(c.shop_price(&camp_spot_item("extra_1")), Some(75));
    assert_eq!(
        c.shop_price(&camp_spot_item("theme")),
        None,
        "open from the start"
    );
    assert!(c.spot_open("theme", &h) && !c.spot_open("extra_2", &h));
    assert!(c.fits("battle_banner", "banner") && !c.fits("battle_banner", "extra_1"));
    assert!(c.fits("midsummer_bonfire", "fire") && c.fits("weapon_rack", "extra_1"));
    assert!(c.fits("war_camp", "theme") && !c.fits("weapon_rack", "theme"));
    assert!(
        !c.fits("throne", "extra_1"),
        "a trophy stands in its corner, on no spot"
    );
    assert!(
        c.may_set_out("war_camp", "theme", &h2),
        "a piece of it owned"
    );
    // every set piece on the ground and every trophy has its place
    assert!(c
        .decorations
        .pieces
        .iter()
        .filter(|p| p.trophy() || (p.spot == SpotKind::Ground && p.set.is_some()))
        .all(|p| p.at.is_some()));
    assert_eq!(
        c.grown_campfire(&h)
            .unwrap()
            .look
            .display_for(Faction::Horde),
        6755,
        "the 5x fire"
    );
    let spots = |k: SpotKind| c.decorations.spots.iter().filter(|s| s.kind == k).count();
    assert_eq!(
        (
            spots(SpotKind::Campfire),
            spots(SpotKind::BannerPost),
            spots(SpotKind::Theme),
            spots(SpotKind::Ground)
        ),
        (1, 1, 1, 2)
    );
    assert_eq!(
        c.decorations.banner_line.banners.len(),
        c.zones.len(),
        "a banner for every zone"
    );
}

#[test]
fn a_trophy_for_sale_or_an_unknown_one_is_refused() {
    let mut c = content();
    c.decorations
        .pieces
        .iter_mut()
        .find(|p| p.id == "slime_jar")
        .unwrap()
        .price = Some(10);
    let e = c.validate().unwrap_err().to_string();
    assert!(e.contains("slime_jar"), "{e}");
    let mut c = content();
    c.decorations.pieces.retain(|p| p.id != "khadgar_statue");
    let e = c.validate().unwrap_err().to_string();
    assert!(e.contains("khadgar_statue"), "{e}");
    let mut c = content();
    c.weapon_glows
        .iter_mut()
        .find(|g| g.id == "damage_low")
        .unwrap()
        .needs = Some("nothing".into());
    assert!(c.validate().is_err());
}
