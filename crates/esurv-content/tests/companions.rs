//! The shipped companions and shop load as the plan gave them, the template is a valid companion,
//! and the checks catch every kind of broken companion.

use esurv_content::{Companion, CompanionLook, Content, Reward, PET_NPCS};

fn content() -> Content {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
    Content::load(dir).expect("content loads and validates")
}

#[test]
fn every_companion_an_achievement_gives_is_listed_with_no_price() {
    let c = content();
    let given: Vec<&str> = c
        .achievements
        .iter()
        .flat_map(|a| a.rewards.iter())
        .filter_map(|r| match r {
            Reward::Companion(id) => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(given.len(), 17);
    for id in given {
        let k = c.companion(id).unwrap_or_else(|| panic!("{id} listed"));
        assert_eq!(k.price, None, "{id}");
        assert!(k.achievement().is_some(), "{id}");
    }
    let sold: Vec<&Companion> = c.companions.iter().filter(|k| k.price.is_some()).collect();
    assert_eq!(sold.iter().filter(|k| k.price == Some(15)).count(), 26);
    assert_eq!(sold.iter().filter(|k| k.price == Some(30)).count(), 20);
    assert!(c.companions.iter().all(|k| k.display() != 0));
    // the Eye of C'Thun has no walk: it glides
    assert_eq!(
        c.companion("baby_cthun_eye").and_then(|k| k.hover),
        Some(1.2)
    );
}

#[test]
fn the_companion_upgrade_has_the_owners_three_ranks() {
    let c = content();
    let ranks: Vec<_> = c
        .shop
        .companion_ranks
        .iter()
        .map(|r| (r.price, r.reach, r.speed, r.carry))
        .collect();
    assert_eq!(
        ranks,
        [(0, 8.0, 9.0, 3), (50, 12.0, 15.0, 6), (100, 16.0, 21.0, 9)]
    );
    assert_eq!(c.shop_price("companion_rank:2"), Some(50));
    assert_eq!(c.shop_price("companion_rank:3"), Some(100));
    assert_eq!(
        c.shop_price("companion_rank:1"),
        None,
        "owned from the start"
    );
    assert_eq!(c.shop_price("companion:bombay_cat"), Some(15));
    assert_eq!(c.shop_price("companion:tiny_hogger"), None, "never sold");
    assert_eq!(c.shop_price("tabard:stormwind"), Some(15), "step 3 sells tabards");
    assert_eq!(c.shop_price("dragon:stormwind"), None);
}

#[test]
fn a_pet_look_is_its_npcs_display() {
    assert_eq!(CompanionLook::Pet { npc: 7385 }.display(), Some(5556));
    assert_eq!(
        CompanionLook::Creature { display: 384 }.display(),
        Some(384)
    );
    assert_eq!(CompanionLook::Pet { npc: 1 }.display(), None);
    let mut npcs: Vec<u32> = PET_NPCS.iter().map(|p| p.0).collect();
    npcs.dedup();
    assert_eq!(npcs.len(), PET_NPCS.len(), "each npc once, in order");
}

#[test]
fn the_companion_template_is_a_valid_companion() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/game/templates/companion.ron"
    );
    let text = std::fs::read_to_string(path).unwrap();
    let k: Companion =
        ron::from_str(&text).expect("docs/game/templates/companion.ron parses as a companion");
    assert_eq!((k.id.as_str(), k.price), ("tiny_kobold", Some(50)));
    let mut c = content();
    c.companions.push(k);
    c.validate().expect("the companion template validates");
}

/// Each broken companion is told by the checks.
#[test]
fn a_broken_companion_is_refused() {
    let base = content();
    let made = |look: &str, rest: &str| -> Companion {
        let text = format!(
            r#"(id: "x", name: "X", icon: "INV_Misc_Coin_01", look: {look}, text: "Hi." {rest})"#
        );
        ron::from_str(&text).unwrap_or_else(|e| panic!("{text}: {e}"))
    };
    let good = made("Pet(npc: 7385)", ", price: 25");
    let mut c = base.clone();
    c.companions.push(good.clone());
    c.validate().expect("a good companion validates");
    let broken = [
        made("Pet(npc: 1)", ", price: 25"),
        made("Creature(display: 384)", ", scale: 0.0, price: 25"),
        made("Creature(display: 384)", ""),
        made(
            "Creature(display: 384)",
            r#", price: 25, unlock: Achievement("gnoll_means_no")"#,
        ),
        made(
            "Creature(display: 384)",
            r#", unlock: Achievement("first_victory")"#,
        ),
        made("Creature(display: 384)", ", price: 0"),
    ];
    for b in broken {
        let mut c = base.clone();
        c.companions.push(b.clone());
        assert!(c.validate().is_err(), "{b:?} is refused");
    }
    // twice
    let mut c = base.clone();
    c.companions.push(good.clone());
    c.companions.push(good);
    assert!(c.validate().is_err());
    // an achievement's companion missing, or sold
    let mut c = base.clone();
    c.companions.retain(|k| k.id != "tiny_hogger");
    assert!(c
        .validate()
        .unwrap_err()
        .to_string()
        .contains("tiny_hogger"));
    let mut c = base.clone();
    let k = c
        .companions
        .iter_mut()
        .find(|k| k.id == "tiny_hogger")
        .unwrap();
    k.price = Some(25);
    assert!(c.validate().is_err());
    // the upgrade's ranks must grow
    let mut c = base;
    c.shop.companion_ranks[2].reach = 1.0;
    assert!(c.validate().is_err());
}

/// The banish and skip upgrades: three ranks each, 25 / 50 / 75 points, 1 / 2 / 3
/// a run.
#[test]
fn banish_and_skip_have_the_owners_three_ranks() {
    let c = content();
    for ranks in [&c.shop.banish_ranks, &c.shop.skip_ranks] {
        let got: Vec<(u32, u32)> = ranks.iter().map(|r| (r.price, r.charges)).collect();
        assert_eq!(got, [(25, 1), (50, 2), (75, 3)]);
    }
    assert_eq!(c.shop_price("banish_rank:1"), Some(25));
    assert_eq!(c.shop_price("skip_rank:3"), Some(75));
    assert_eq!(c.shop_price("skip_rank:4"), None);
    assert_eq!(c.shop_price("banish_rank:0"), None);
    let mut broken = c.clone();
    broken.shop.skip_ranks[1].charges = 1;
    assert!(broken.validate().is_err(), "a rank must give more than the one before");
}
