//! What the Shop's Tabards, Enchants and Decorations tabs, the hero sheet's Title, Tabard and
//! Weapon glow picks and the Decorate camp screen show, from the content (`tabards.ron`,
//! `weapon_glows.ron`, `decorations.ron`, the titles of `achievements.ron`) and the warband's
//! achievements as told (what was bought, earned and cleared, each camp's layout).
//! docs/game/screens.md, "The Shop", "The hero sheet" and "Decorate camp".

use esurv_content::{
    camp_spot_item, decoration_item, glow_item, tabard_item, CampSpot, Content, Decoration, Earned,
    Faction, Holdings, SpotKind, TabardLook,
};
use esurv_protocol::{Achievements, WearKind};

use super::shop::{Shelf, ShopSection, ShopTile, TileState};

/// Whether the warband screen shows camp decorations: their pieces round the fire, the Decorate
/// camp button, the Shop's Decorations tab and the trophies among achievement rewards.
/// Parked for a later 3D hub; the content, the save's camps and this code
/// stay, switched off, so the hub can take them up. The camp shows its plain fire.
pub const CAMP_DECORATIONS: bool = false;

/// Runs `f` with the warband's holdings as told.
pub fn with_holdings<R>(told: &Achievements, f: impl FnOnce(&Holdings) -> R) -> R {
    let bought = |i: &str| told.bought.iter().any(|b| b == i);
    let earned = |a: &str| told.earned.iter().any(|(e, _)| e == a);
    let cleared = |z: &str| told.zones_cleared.iter().any(|c| c == z);
    f(&Holdings {
        bought: &bought,
        earned: &earned,
        cleared: &cleared,
    })
}

/// Whether the warband may wear `id` of `kind`: a title earned, a tabard or glow bought.
pub fn owns_wear(content: &Content, told: &Achievements, kind: WearKind, id: &str) -> bool {
    match kind {
        WearKind::Title => {
            content.title_of(id).is_some() && told.earned.iter().any(|(e, _)| e == id)
        }
        WearKind::Tabard => content.tabard(id).is_some() && told.bought.contains(&tabard_item(id)),
        WearKind::Glow => content.weapon_glow(id).is_some() && told.bought.contains(&glow_item(id)),
    }
}

/// The camp's word in the asks.
pub fn camp_word(f: Faction) -> &'static str {
    match f {
        Faction::Horde => "horde",
        Faction::Alliance => "alliance",
    }
}

/// Who may wear a tabard, in words.
pub fn tabard_wearers(look: TabardLook) -> &'static str {
    match look {
        TabardLook::Neutral { .. } => "Any hero may wear it.",
        TabardLook::Faction { .. } => "Each hero shows its own faction's.",
        TabardLook::Only {
            faction: Faction::Alliance,
            ..
        } => "Alliance heroes only.",
        TabardLook::Only {
            faction: Faction::Horde,
            ..
        } => "Horde heroes only.",
    }
}

/// A zone's name as the warband may see it: "???" until discovered.
fn zone_words(content: &Content, discovered: &[String], told: &Achievements, zone: &str) -> String {
    let seen = discovered.iter().any(|z| z == zone) || told.zones_cleared.iter().any(|z| z == zone);
    match content.zones.iter().find(|z| z.id == zone) {
        Some(z) if seen => z.name.clone(),
        _ => "???".into(),
    }
}

/// What earns a piece that is never sold, in words: "Achievement: Bossbane", "Clear Blackrock
/// Mountain", "Own every piece of War Camp".
pub fn earned_words(
    content: &Content,
    discovered: &[String],
    told: &Achievements,
    d: &Decoration,
) -> String {
    let name = |a: &str| {
        content
            .achievement(a)
            .map_or_else(|| a.to_owned(), |x| x.name.clone())
    };
    match &d.earned {
        Some(Earned::Achievement(a)) => format!("Achievement: {}", name(a)),
        Some(Earned::ZoneClear(z)) => format!("Clear {}", zone_words(content, discovered, told, z)),
        Some(Earned::Set(s)) => {
            let set = content
                .decorations
                .sets
                .iter()
                .find(|x| &x.id == s)
                .map_or(s.as_str(), |x| x.name.as_str());
            format!("Own every piece of {set}")
        }
        Some(Earned::Ladder(steps)) => {
            let names: Vec<String> = steps.iter().map(|s| name(&s.achievement)).collect();
            format!("Achievements: {}", names.join(", then "))
        }
        Some(Earned::Class(c)) => {
            let class = content.classes.iter().find(|x| &x.id == c).map_or(c.as_str(), |x| x.name.as_str());
            format!("Free to every {class}")
        }
        None => String::new(),
    }
}

/// The state of a sold tile: owned, waiting for what it needs, too dear, or for sale.
fn sold_state(content: &Content, told: &Achievements, item: &str, price: u32) -> TileState {
    if told.bought.iter().any(|b| b == item) {
        return TileState::Owned;
    }
    let missing = content
        .shop_needs(item)
        .into_iter()
        .find(|n| !told.bought.contains(n));
    if let Some(n) = missing {
        return TileState::Needs(item_name(content, &n));
    }
    if price > told.balance {
        TileState::TooDear
    } else {
        TileState::ForSale
    }
}

/// A cosmetic shop item's name.
pub fn item_name(content: &Content, item: &str) -> String {
    let (kind, id) = item.split_once(':').unwrap_or(("", item));
    match kind {
        "tabard" => content.tabard(id).map(|t| t.name.clone()),
        "glow" => content.weapon_glow(id).map(|g| g.name.clone()),
        "decoration" => content.decoration(id).map(|d| d.name.clone()),
        "camp_spot" => content.camp_spot(id).map(spot_name),
        _ => None,
    }
    .unwrap_or_else(|| id.to_owned())
}

/// A spot's name: "Ground spot 5".
pub fn spot_name(s: &CampSpot) -> String {
    let n =
        s.id.rsplit('_')
            .next()
            .filter(|n| n.chars().all(|c| c.is_ascii_digit()));
    match n {
        Some(n) => format!("{} {n}", s.kind.label()),
        None => s.kind.label().to_owned(),
    }
}

/// A decoration's tile.
pub fn decoration_tile(
    content: &Content,
    discovered: &[String],
    told: &Achievements,
    d: &Decoration,
) -> ShopTile {
    let item = decoration_item(&d.id);
    let owned = with_holdings(told, |h| content.owns_decoration(&d.id, h));
    let state = match (owned, d.price, &d.earned) {
        (true, _, _) => TileState::Owned,
        (false, Some(p), _) => sold_state(content, told, &item, p),
        (false, None, Some(Earned::Achievement(a))) => TileState::FromAchievement(
            content
                .achievement(a)
                .map_or_else(|| a.clone(), |x| x.name.clone()),
        ),
        (false, None, _) => TileState::EarnedBy(earned_words(content, discovered, told, d)),
    };
    let stands = match d.spot {
        SpotKind::Campfire => "Gives the camp fire its look.",
        SpotKind::BannerPost => "Stands on the banner post.",
        SpotKind::Trophy => "Stands in the trophy corner.",
        SpotKind::Sign => "Puts up the camp sign.",
        SpotKind::Ground if d.set.is_some() => {
            "Stands in its place while its set is the camp theme, or on an extra spot."
        }
        SpotKind::Ground | SpotKind::Theme => "Stands on an extra spot.",
        SpotKind::Tent => "Stands at a hero's tent in the camp.",
    };
    ShopTile {
        item,
        name: d.name.clone(),
        icon: d.icon.clone(),
        price: d.price,
        state,
        text: format!("{} {stands}", d.text),
        display: None,
    }
}

/// The Shop's Tabards, Enchants and Decorations sections; `discovered` names the zones a trophy
/// shows by name.
pub fn sections(content: &Content, discovered: &[String], told: &Achievements) -> Vec<ShopSection> {
    let mut out = Vec::new();
    let mut prices: Vec<u32> = content.tabards.iter().map(|t| t.price).collect();
    prices.sort_unstable();
    prices.dedup();
    for p in prices {
        out.push(ShopSection {
            heading: format!("Tabards, {p} Moments"),
            tiles: content
                .tabards
                .iter()
                .filter(|t| t.price == p)
                .map(|t| {
                    let item = tabard_item(&t.id);
                    ShopTile {
                        state: sold_state(content, told, &item, t.price),
                        item,
                        name: t.name.clone(),
                        icon: t.icon.clone(),
                        price: Some(t.price),
                        text: format!("{} {}", t.text, tabard_wearers(t.look)),
                        display: None,
                    }
                })
                .collect(),
            shelf: Shelf::Tabards,
        });
    }
    let mut prices: Vec<u32> = content.weapon_glows.iter().map(|g| g.price).collect();
    prices.sort_unstable();
    prices.dedup();
    for p in prices {
        out.push(ShopSection {
            heading: format!("Weapon glows, {p} Moments"),
            tiles: content
                .weapon_glows
                .iter()
                .filter(|g| g.price == p)
                .map(|g| {
                    let item = glow_item(&g.id);
                    ShopTile {
                        state: sold_state(content, told, &item, g.price),
                        item,
                        name: g.name.clone(),
                        icon: g.icon.clone(),
                        price: Some(g.price),
                        text: format!("{} On all of a hero's weapons, never a shield.", g.text),
                        display: None,
                    }
                })
                .collect(),
            shelf: Shelf::Enchants,
        });
    }
    let deco = &content.decorations;
    for set in &deco.sets {
        out.push(ShopSection {
            // the walkable camp has no camp sign: the set's name alone
            heading: if crate::camp::hub_on() {
                set.name.clone()
            } else {
                format!("{} (one set is a camp's theme; owning every piece puts up the camp sign)", set.name)
            },
            tiles: content
                .set_pieces(&set.id)
                .map(|d| decoration_tile(content, discovered, told, d))
                .collect(),
            shelf: Shelf::Decorations,
        });
    }
    // the walkable camp's tents: everyday things sold, and each class's own piece free to it
    if crate::camp::hub_on() {
        out.push(ShopSection {
            heading: "For a hero's tent (each hero's class has its own piece free)".into(),
            tiles: deco
                .pieces
                .iter()
                .filter(|d| d.spot == SpotKind::Tent)
                .map(|d| decoration_tile(content, discovered, told, d))
                .collect(),
            shelf: Shelf::Decorations,
        });
    }
    if !crate::camp::hub_on() {
        out.push(ShopSection {
            heading: "Signposts (free with a whole set; one camp sign shows)".into(),
            tiles: deco
                .pieces
                .iter()
                .filter(|d| matches!(d.earned, Some(Earned::Set(_))))
                .map(|d| decoration_tile(content, discovered, told, d))
                .collect(),
            shelf: Shelf::Decorations,
        });
    }
    out.push(ShopSection {
        heading: "Trophies (earned, never sold)".into(),
        tiles: deco
            .pieces
            .iter()
            .filter(|d| d.trophy())
            .map(|d| decoration_tile(content, discovered, told, d))
            .collect(),
        shelf: Shelf::Decorations,
    });
    // the warband screen's camp sells two more spots by its fire; the walkable camp's spots are
    // all free, each hero's tent its own
    if crate::camp::hub_on() {
        return out;
    }
    out.push(ShopSection {
        heading: "Camp spots (for both camps)".into(),
        tiles: deco
            .spots
            .iter()
            .filter_map(|s| {
                let price = s.price?;
                let item = camp_spot_item(&s.id);
                Some(ShopTile {
                    state: sold_state(content, told, &item, price),
                    item,
                    name: format!("{} (extra)", spot_name(s)),
                    icon: "INV_Misc_Map_01".into(),
                    price: Some(price),
                    text: "One more spot by the fire, in both camps, for any ground piece the warband owns.".into(),
                    display: None,
                })
            })
            .collect(),
        shelf: Shelf::Decorations,
    });
    out
}

/// One choice of a hero sheet pick: what it is, whether the warband has it, and the words
/// telling where it comes from when not.
#[derive(Clone, Debug, PartialEq)]
pub struct WearChoice {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub owned: bool,
    /// This hero may wear it (a faction's own tabard only for its heroes).
    pub fits: bool,
    pub text: String,
    pub source: String,
}

/// Every title, tabard or weapon glow a hero may pick from, in the content's order: for a hero
/// named `name` of `faction`.
pub fn wear_choices(
    content: &Content,
    told: &Achievements,
    kind: WearKind,
    name: &str,
    faction: Option<Faction>,
) -> Vec<WearChoice> {
    match kind {
        WearKind::Title => content
            .titles()
            .map(|(a, t)| {
                let owned = owns_wear(content, told, kind, a);
                let ach = content.achievement(a);
                WearChoice {
                    id: a.to_owned(),
                    name: esurv_content::titled(t, name),
                    icon: ach.map_or_else(String::new, |x| x.icon.clone()),
                    owned,
                    fits: true,
                    text: String::new(),
                    source: format!("Achievement: {}", ach.map_or(a, |x| x.name.as_str())),
                }
            })
            .collect(),
        WearKind::Tabard => content
            .tabards
            .iter()
            .map(|t| WearChoice {
                id: t.id.clone(),
                name: t.name.clone(),
                icon: t.icon.clone(),
                owned: owns_wear(content, told, kind, &t.id),
                fits: faction.is_some_and(|f| t.look.display_for(f).is_some()),
                text: format!("{} {}", t.text, tabard_wearers(t.look)),
                source: format!("In the Shop for {} Moments", t.price),
            })
            .collect(),
        WearKind::Glow => content
            .weapon_glows
            .iter()
            .map(|g| WearChoice {
                id: g.id.clone(),
                name: g.name.clone(),
                icon: g.icon.clone(),
                owned: owns_wear(content, told, kind, &g.id),
                fits: true,
                text: g.text.clone(),
                source: format!("In the Shop for {} Moments", g.price),
            })
            .collect(),
    }
}

/// A pick's heading and what the box says it does.
pub fn wear_label(kind: WearKind) -> (&'static str, &'static str) {
    match kind {
        WearKind::Title => ("Title", "Shown with the name"),
        WearKind::Tabard => ("Tabard", "Worn over any armour"),
        WearKind::Glow => ("Weapon glow", "On all its weapons"),
    }
}

/// The pieces that may stand on spot `spot` of a camp: every piece of its kind, the warband's
/// first.
pub fn spot_choices<'c>(
    content: &'c Content,
    told: &Achievements,
    spot: &CampSpot,
) -> Vec<(&'c Decoration, bool)> {
    let mut list: Vec<(&Decoration, bool)> = content
        .decorations
        .pieces
        .iter()
        .filter(|d| d.spot == spot.kind)
        .map(|d| {
            (
                d,
                with_holdings(told, |h| content.owns_decoration(&d.id, h)),
            )
        })
        .collect();
    list.sort_by_key(|(_, owned)| !owned);
    list
}

/// The sets a camp may take as its theme: every set, with how many of its pieces the warband
/// owns and has (one owned piece makes it a choice).
pub fn theme_choices<'c>(
    content: &'c Content,
    told: &Achievements,
) -> Vec<(&'c esurv_content::DecorationSet, usize, usize)> {
    content
        .decorations
        .sets
        .iter()
        .map(|s| {
            let pieces: Vec<&Decoration> = content.set_pieces(&s.id).collect();
            let owned = pieces
                .iter()
                .filter(|d| with_holdings(told, |h| content.owns_decoration(&d.id, h)))
                .count();
            (s, owned, pieces.len())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    #[test]
    fn the_new_tabs_list_tabards_glows_and_the_camps_pieces() {
        let c = content();
        let told = Achievements {
            balance: 40,
            bought: vec!["glow:damage_low".into(), "decoration:weapon_rack".into()],
            earned: vec![("bossbane".into(), 1)],
            zones_cleared: vec!["brm".into()],
            ..Default::default()
        };
        let s = sections(&c, &["brm".into()], &told);
        let on = |shelf: Shelf| {
            s.iter()
                .filter(|x| x.shelf == shelf)
                .flat_map(|x| &x.tiles)
                .count()
        };
        assert_eq!((on(Shelf::Tabards), on(Shelf::Enchants)), (14, 18));
        let tile = |item: &str| {
            s.iter()
                .flat_map(|x| &x.tiles)
                .find(|t| t.item == item)
                .unwrap()
                .clone()
        };
        assert_eq!(tile("glow:damage_low").state, TileState::Owned);
        assert_eq!(tile("glow:damage_medium").state, TileState::ForSale);
        assert_eq!(
            tile("glow:damage_high").state,
            TileState::Needs("Impact".into())
        );
        assert_eq!(tile("glow:crusader").state, TileState::TooDear);
        assert_eq!(tile("decoration:onyxia_head").state, TileState::Owned);
        assert_eq!(
            tile("decoration:nefarian_head").state,
            TileState::Owned,
            "Blackrock cleared"
        );
        assert_eq!(
            tile("decoration:slime_jar").state,
            TileState::FromAchievement("Stitched Up".into())
        );
        assert_eq!(
            tile("decoration:troll_skull_pile").state,
            TileState::EarnedBy("Clear ???".into())
        );
        assert_eq!(
            tile("decoration:great_anvil").state,
            TileState::Needs("Anvil".into())
        );
        if crate::camp::hub_on() {
            // the walkable camp: no camp sign, and its spots free
            let listed = |item: &str| s.iter().flat_map(|x| &x.tiles).any(|t| t.item == item);
            assert!(!listed("decoration:sign_war_camp") && !listed("camp_spot:extra_1"));
        } else {
            assert_eq!(
                tile("decoration:sign_war_camp").state,
                TileState::EarnedBy("Own every piece of War Camp".into())
            );
            assert_eq!(tile("camp_spot:extra_1").price, Some(75));
        }
        assert!(tile("tabard:stormwind")
            .text
            .ends_with("Alliance heroes only."));
    }

    #[test]
    fn the_hero_picks_name_the_hero_and_keep_a_factions_own_tabard_with_it() {
        let c = content();
        let told = Achievements {
            earned: vec![("well_armed".into(), 1)],
            bought: vec!["tabard:stormwind".into()],
            ..Default::default()
        };
        let titles = wear_choices(&c, &told, WearKind::Title, "Lumi", Some(Faction::Horde));
        let armsmaster = titles.iter().find(|t| t.id == "well_armed").unwrap();
        assert_eq!(
            (armsmaster.name.as_str(), armsmaster.owned),
            ("Lumi the Armsmaster", true)
        );
        assert!(titles
            .iter()
            .filter(|t| t.id != "well_armed")
            .all(|t| !t.owned));
        let tabards = wear_choices(&c, &told, WearKind::Tabard, "Lumi", Some(Faction::Horde));
        let sw = tabards.iter().find(|t| t.id == "stormwind").unwrap();
        assert!(sw.owned && !sw.fits, "the Alliance's own");
        let spot = c.camp_spot("extra_1").unwrap();
        assert!(spot_choices(&c, &told, spot)
            .iter()
            .all(|(d, _)| d.spot == SpotKind::Ground));
        assert_eq!(spot_name(spot), "Extra spot 1");
        assert_eq!(spot_name(c.camp_spot("theme").unwrap()), "Camp theme");
    }
}

/// 1.12's tabard equipment slot and inventory type, as the character list carries them.
const TABARD_SLOT: usize = 18;
const TABARD_INVENTORY_TYPE: u8 = 19;

/// Dresses a hero figure (a camp's, the hero sheet's) in its tabard and weapon glow, as a run
/// shows them: the tabard (`tabard`, its id; empty for none) over any armour, in its faction's
/// look; the glow (`glow`) as the enchant of each held weapon (never a shield or held item). The
/// enchants of the main hand, off hand and ranged slot.
pub fn dress_figure(
    content: &Content,
    faction: Option<Faction>,
    tabard: &str,
    glow: &str,
    equipment: &mut [benilla_protocol::CharEnumItem; 19],
) -> [u32; 3] {
    let look = content
        .tabard(tabard)
        .zip(faction)
        .and_then(|(t, f)| t.look.display_for(f));
    if let Some(display_id) = look {
        equipment[TABARD_SLOT] = benilla_protocol::CharEnumItem {
            display_id,
            inventory_type: TABARD_INVENTORY_TYPE,
        };
    }
    let enchant = content.weapon_glow(glow).map_or(0, |g| g.enchant);
    [15, 16, 17].map(|slot| {
        let item = equipment[slot];
        if item.display_id != 0 && esurv_content::glows_on(u32::from(item.inventory_type)) {
            enchant
        } else {
            0
        }
    })
}
