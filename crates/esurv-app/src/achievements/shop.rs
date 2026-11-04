//! What the Achievements window's Shop and the hero sheet's Companions show, from the content
//! (`companions.ron`, `shop.ron`) and the warband's achievements as told (`ToClient::Achievements`:
//! what was bought, the upgrade's rank, what was earned): each companion's tile and its state, and
//! the companion upgrade's ranks. docs/game/screens.md, "The Shop".

use esurv_content::{companion_item, companion_rank_item, Companion, Content};
use esurv_protocol::Achievements;

/// Where a tile stands for the warband.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TileState {
    /// The warband has it.
    Owned,
    /// For sale, and the points to spend cover it.
    ForSale,
    /// For sale, but the points to spend do not cover it.
    TooDear,
    /// Only this achievement gives it (its name), not earned yet.
    FromAchievement(String),
    /// An upgrade rank that waits for the one before it.
    NeedsRank(usize),
    /// Never sold, earned as these words say ("Clear Blackrock Mountain"), not yet.
    EarnedBy(String),
    /// For sale once the warband owns what it needs (its name: "Lesser Impact").
    Needs(String),
}

/// One tile of the shop.
#[derive(Clone, Debug, PartialEq)]
pub struct ShopTile {
    /// The shop item it is (`companion:<id>`, `companion_rank:<n>`).
    pub item: String,
    pub name: String,
    pub icon: String,
    /// Points to spend; `None` for one never sold or owned from the start.
    pub price: Option<u32>,
    pub state: TileState,
    /// Its tooltip and details line.
    pub text: String,
    /// The look a companion shows in the preview (its display id); `None` for an upgrade.
    pub display: Option<u32>,
}

impl ShopTile {
    /// Whether the Buy button may take it.
    pub fn buyable(&self) -> bool {
        self.state == TileState::ForSale
    }
}

/// One section of the shop: its heading and tiles.
#[derive(Clone, Debug, PartialEq)]
pub struct ShopSection {
    pub heading: String,
    pub tiles: Vec<ShopTile>,
    /// The Shop's tab it shows on.
    pub shelf: Shelf,
}

/// The Shop's tabs, left to right: what each section shows on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Shelf {
    /// The companion upgrade, the banishes and skips.
    #[default]
    Upgrades,
    /// The companions.
    Pets,
    Tabards,
    /// The weapon glows.
    Enchants,
    /// The camp decorations and the camp's extra spots.
    Decorations,
}

/// The banish upgrade's and the skip upgrade's icons.
pub const BANISH_ICON: &str = "Spell_Holy_DispelMagic";
pub const SKIP_ICON: &str = "Ability_Rogue_Sprint";

/// A banish or skip upgrade's tiles: rank n gives its charges a run, each rank
/// after the one before, none owned from the start.
fn charge_tiles(
    told: &Achievements,
    ranks: &[esurv_content::ChargeRank],
    have: u32,
    name: &str,
    icon: &str,
    item: fn(usize) -> String,
    text: fn(u32) -> String,
) -> Vec<ShopTile> {
    let have = have as usize;
    ranks
        .iter()
        .enumerate()
        .map(|(k, r)| {
            let n = k + 1;
            let state = if n <= have {
                TileState::Owned
            } else if n > have + 1 {
                TileState::NeedsRank(n - 1)
            } else if r.price > told.balance {
                TileState::TooDear
            } else {
                TileState::ForSale
            };
            ShopTile {
                item: item(n),
                name: format!("{name} Rank {n}"),
                icon: icon.into(),
                price: Some(r.price),
                state,
                text: text(r.charges),
                display: None,
            }
        })
        .collect()
}

fn banish_text(n: u32) -> String {
    let what = if n == 1 { "one card".to_owned() } else { format!("{n} cards") };
    format!(
        "Banish {what} a run, on a level-up or in a cache. Each banished card is cut from the rest of that run's timeline, and a fresh card takes its place."
    )
}

fn skip_text(n: u32) -> String {
    let what = if n == 1 { "once".to_owned() } else { format!("{n} times") };
    format!("Let a level-up or a cache slip by {what} a run, taking nothing from it.")
}

/// Whether the warband has companion `c`: bought, or its achievement earned.
pub fn owns(told: &Achievements, c: &Companion) -> bool {
    match c.achievement() {
        Some(a) => told.earned.iter().any(|(e, _)| e == a),
        None => told.bought.iter().any(|b| *b == companion_item(&c.id)),
    }
}

/// The companions the warband has, in the content's order.
pub fn owned_companions<'c>(content: &'c Content, told: &Achievements) -> Vec<&'c Companion> {
    content
        .companions
        .iter()
        .filter(|c| owns(told, c))
        .collect()
}

/// The icon of the companion upgrade's tiles.
pub const UPGRADE_ICON: &str = "INV_Misc_Coin_02";

/// What rank `n` (1 the first) of the companion upgrade does, in words.
pub fn rank_text(content: &Content, n: usize) -> String {
    let Some(r) = content.shop.companion_ranks.get(n.saturating_sub(1)) else {
        return String::new();
    };
    format!(
        "Fetches gold up to {} yd past your pickup range, runs {} yd/s and carries {} piles before bringing them back.",
        r.reach, r.speed, r.carry
    )
}

/// A companion's tile.
fn companion_tile(content: &Content, told: &Achievements, c: &Companion) -> ShopTile {
    let state = if owns(told, c) {
        TileState::Owned
    } else if let Some(a) = c.achievement() {
        TileState::FromAchievement(
            content
                .achievement(a)
                .map_or_else(|| a.to_owned(), |x| x.name.clone()),
        )
    } else if c.price.unwrap_or(0) > told.balance {
        TileState::TooDear
    } else {
        TileState::ForSale
    };
    ShopTile {
        item: companion_item(&c.id),
        name: c.name.clone(),
        icon: c.icon.clone(),
        price: c.price,
        state,
        text: c.text.clone(),
        display: Some(c.display()),
    }
}

/// The shop's sections: the companion upgrade's ranks, the companions for sale by price, and those
/// only achievements give; the tabards, weapon glows and camp decorations ([`super::cosmetics`]).
pub fn sections(content: &Content, discovered: &[String], told: &Achievements) -> Vec<ShopSection> {
    let rank = told.companion_rank.max(1) as usize;
    let ranks = content
        .shop
        .companion_ranks
        .iter()
        .enumerate()
        .map(|(k, r)| {
            let n = k + 1;
            let state = if n <= rank {
                TileState::Owned
            } else if n > rank + 1 {
                TileState::NeedsRank(n - 1)
            } else if r.price > told.balance {
                TileState::TooDear
            } else {
                TileState::ForSale
            };
            ShopTile {
                item: companion_rank_item(n),
                name: format!("Companion Rank {n}"),
                icon: UPGRADE_ICON.into(),
                price: (r.price > 0).then_some(r.price),
                state,
                text: rank_text(content, n),
                display: None,
            }
        })
        .collect();
    let mut out = vec![
        ShopSection {
            heading: "Companion upgrade (every companion)".into(),
            tiles: ranks,
            shelf: Shelf::Upgrades,
        },
        ShopSection {
            heading: "Banish (every hero, every run)".into(),
            tiles: charge_tiles(told, &content.shop.banish_ranks, told.banish_rank, "Banish", BANISH_ICON, esurv_content::banish_rank_item, banish_text),
            shelf: Shelf::Upgrades,
        },
        ShopSection {
            heading: "Skip (every hero, every run)".into(),
            tiles: charge_tiles(told, &content.shop.skip_ranks, told.skip_rank, "Skip", SKIP_ICON, esurv_content::skip_rank_item, skip_text),
            shelf: Shelf::Upgrades,
        },
    ];
    // the hub's buffs (Time Floes), each bought once
    out.push(ShopSection {
        heading: content.shown("The {Area(1941)} (every hero)"),
        tiles: content
            .shop
            .hub_buffs
            .iter()
            .map(|b| {
                let item = esurv_content::hub_buff_item(&b.id);
                let state = if told.bought.contains(&item) {
                    TileState::Owned
                } else if b.price > told.balance {
                    TileState::TooDear
                } else {
                    TileState::ForSale
                };
                ShopTile { item, name: b.name.clone(), icon: b.icon.clone(), price: Some(b.price), state, text: b.text.clone(), display: None }
            })
            .collect(),
        shelf: Shelf::Upgrades,
    });
    let mut prices: Vec<u32> = content.companions.iter().filter_map(|c| c.price).collect();
    prices.sort_unstable();
    prices.dedup();
    for p in prices {
        out.push(ShopSection {
            heading: format!("Companions, {p} Moments"),
            tiles: content
                .companions
                .iter()
                .filter(|c| c.price == Some(p))
                .map(|c| companion_tile(content, told, c))
                .collect(),
            shelf: Shelf::Pets,
        });
    }
    out.push(ShopSection {
        heading: "Earned from achievements".into(),
        tiles: content
            .companions
            .iter()
            .filter(|c| c.price.is_none())
            .map(|c| companion_tile(content, told, c))
            .collect(),
        shelf: Shelf::Pets,
    });
    // step 3: the tabards, the weapon glows and the camp's pieces
    out.extend(super::cosmetics::sections(content, discovered, told));
    out
}

/// The tile of shop item `item`, if the shop has it.
pub fn tile(content: &Content, discovered: &[String], told: &Achievements, item: &str) -> Option<ShopTile> {
    sections(content, discovered, told)
        .into_iter()
        .flat_map(|s| s.tiles)
        .find(|t| t.item == item)
}

/// What a tile's state says under it.
pub fn state_words(t: &ShopTile) -> String {
    match &t.state {
        TileState::Owned => "Owned".into(),
        TileState::ForSale | TileState::TooDear => match t.price {
            Some(p) => format!("{p} Moments"),
            None => String::new(),
        },
        TileState::FromAchievement(a) => format!("Achievement: {a}"),
        TileState::NeedsRank(n) => format!("Needs Rank {n}"),
        TileState::EarnedBy(w) => w.clone(),
        TileState::Needs(n) => format!("Needs {n}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    #[test]
    fn the_shop_lists_the_upgrade_the_companions_by_price_and_the_earned_ones() {
        let c = content();
        let told = Achievements {
            balance: 25,
            companion_rank: 1,
            ..Default::default()
        };
        let s = sections(&c, &[], &told);
        let caverns = c.shown("The {Area(1941)} (every hero)");
        let heads: Vec<&str> = s.iter().map(|x| x.heading.as_str()).collect();
        assert_eq!(
            heads[..7],
            [
                "Companion upgrade (every companion)",
                "Banish (every hero, every run)",
                "Skip (every hero, every run)",
                caverns.as_str(),
                "Companions, 15 Moments",
                "Companions, 30 Moments",
                "Earned from achievements"
            ]
        );
        let ranks: Vec<&TileState> = s[0].tiles.iter().map(|t| &t.state).collect();
        assert_eq!(
            ranks,
            [
                &TileState::Owned,
                &TileState::TooDear,
                &TileState::NeedsRank(2)
            ]
        );
        let banish: Vec<&TileState> = s[1].tiles.iter().map(|t| &t.state).collect();
        assert_eq!(banish, [&TileState::ForSale, &TileState::NeedsRank(1), &TileState::NeedsRank(2)]);
        // Time Floes, 75 points, beyond the 25 to spend
        assert_eq!(s[3].tiles.iter().map(|t| (t.name.as_str(), &t.state)).collect::<Vec<_>>(), [("Time Floes", &TileState::TooDear)]);
        assert!(s[..4].iter().all(|x| x.shelf == Shelf::Upgrades) && s[4..7].iter().all(|x| x.shelf == Shelf::Pets));
        assert_eq!(
            (s[4].tiles.len(), s[5].tiles.len(), s[6].tiles.len()),
            (26, 20, 17)
        );
        assert!(s[4].tiles.iter().all(|t| t.state == TileState::ForSale));
        assert!(s[5].tiles.iter().all(|t| t.state == TileState::TooDear));
        let hogger = s[6]
            .tiles
            .iter()
            .find(|t| t.item == "companion:tiny_hogger")
            .unwrap();
        assert_eq!(
            hogger.state,
            TileState::FromAchievement("Gnoll Means No".into())
        );
        assert_eq!(state_words(hogger), "Achievement: Gnoll Means No");
    }

    #[test]
    fn what_was_bought_or_earned_is_owned() {
        let c = content();
        let told = Achievements {
            balance: 200,
            companion_rank: 2,
            bought: vec!["companion:bombay_cat".into(), "companion_rank:2".into()],
            earned: vec![("gnoll_means_no".into(), 1)],
            ..Default::default()
        };
        let owned: Vec<&str> = owned_companions(&c, &told)
            .iter()
            .map(|k| k.id.as_str())
            .collect();
        assert_eq!(owned, ["tiny_hogger", "bombay_cat"]);
        let r3 = tile(&c, &[], &told, "companion_rank:3").unwrap();
        assert!(r3.buyable());
        assert_eq!(state_words(&r3), "100 Moments");
        assert_eq!(
            tile(&c, &[], &told, "companion_rank:2").unwrap().state,
            TileState::Owned
        );
        assert!(rank_text(&c, 3).contains("16 yd"));
    }
}
