//! Companions (`companions.ron`) and the shop (`shop.ron`): small pets that follow a hero in runs
//! and fetch the gold piles in reach, and stand beside it at the camp fire; bought with the
//! achievements' points to spend, or given by an achievement. docs/game/achievements.md has the
//! rules; docs/game/AUTHORING.md "A companion" the how-to (template
//! `docs/game/templates/companion.ron`).

use std::collections::HashSet;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{Content, Reward};

/// One companion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Companion {
    /// Unique, lowercase; the save keeps what was bought and picked by it, so it never changes
    /// once shipped.
    pub id: String,
    pub name: String,
    /// A texture name from `Interface\Icons` in the WoW client.
    pub icon: String,
    pub look: CompanionLook,
    /// Its size against the look's own (1.0: as in WoW).
    #[serde(default = "one")]
    pub scale: f32,
    /// Points to spend in the shop; `None` for one only an achievement gives.
    #[serde(
        default,
        deserialize_with = "plain_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub price: Option<u32>,
    /// Its shop tooltip, our own words.
    pub text: String,
    /// Given by this achievement (its `rewards: [Companion(id)]`) and never sold.
    #[serde(
        default,
        deserialize_with = "plain_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub unlock: Option<CompanionUnlock>,
    /// Yards it glides above the ground, bobbing, for a look with no walk (the Eye of C'Thun);
    /// `None`: it walks.
    #[serde(
        default,
        deserialize_with = "plain_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub hover: Option<f32>,
    /// The spell looks it plays on itself, at its own size (1.12 `SpellVisualKit.dbc` ids): as it
    /// appears in a run, and as it hands gold over.
    #[serde(default, skip_serializing_if = "CompanionKits::is_empty")]
    pub kits: CompanionKits,
}

/// A companion's spell looks ([`Companion::kits`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompanionKits {
    #[serde(deserialize_with = "plain_some", skip_serializing_if = "Option::is_none")]
    pub appear: Option<u32>,
    #[serde(deserialize_with = "plain_some", skip_serializing_if = "Option::is_none")]
    pub hand_over: Option<u32>,
}

impl CompanionKits {
    pub fn is_empty(&self) -> bool {
        self.appear.is_none() && self.hand_over.is_none()
    }
}

fn one() -> f32 {
    1.0
}

/// A value written plainly (`price: 25`) read as `Some`.
fn plain_some<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

/// What a companion looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompanionLook {
    /// A 1.12 companion pet, by the creature its item summons ([`PET_NPCS`]).
    Pet { npc: u32 },
    /// Any creature model, by its `CreatureDisplayInfo.dbc` id.
    Creature { display: u32 },
}

/// How a companion that is never sold comes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompanionUnlock {
    Achievement(String),
}

/// The 1.12 companion pets' creatures and their looks: (npc, display id), from the vmangos world
/// database (`creature_template.display_id1` of each creature a companion item's spell summons,
/// Summon Critter), every display in the 1.12.1 client's `CreatureDisplayInfo.dbc`.
pub const PET_NPCS: &[(u32, u32)] = &[
    (2620, 1072),   // Prairie Dog
    (2671, 7937),   // Mechanical Squirrel
    (7380, 5585),   // Siamese
    (7381, 5555),   // Silver Tabby
    (7382, 5554),   // Orange Tabby
    (7383, 5448),   // Maine Coon (Black Tabby)
    (7384, 5586),   // Cornish Rex
    (7385, 5556),   // Bombay
    (7386, 9989),   // White Kitten
    (7387, 5207),   // Green Wing Macaw
    (7389, 6190),   // Senegal
    (7390, 6191),   // Cockatiel
    (7391, 6192),   // Hyacinth Macaw
    (7392, 304),    // Prairie Chicken
    (7394, 5369),   // Ancona
    (7395, 2177),   // Cockroach
    (7543, 6288),   // Dark Whelpling
    (7544, 6290),   // Crimson Whelpling
    (7545, 6291),   // Emerald Whelpling
    (7549, 6295),   // Tree Frog
    (7550, 901),    // Wood Frog
    (7553, 4615),   // Great Horned Owl
    (7555, 6299),   // Hawk Owl
    (7560, 328),    // Snowshoe Rabbit
    (7562, 2957),   // Brown Snake
    (7565, 1206),   // Black Kingsnake
    (7567, 6303),   // Crimson Snake
    (8376, 7920),   // Mechanical Chicken
    (9656, 8909),   // Tiny Walking Bomb
    (9657, 8910),   // Lil' Smoky
    (9662, 6294),   // Sprite Darter Hatchling
    (9936, 9209),   // Corrupted Kitten
    (10259, 9563),  // Worg Pup
    (10598, 9997),  // Smolderweb Hatchling
    (11236, 9554),  // Blood Parrot
    (11325, 10990), // Panda Cub
    (11326, 10992), // Mini Diablo
    (11327, 10993), // Zergling
    (12419, 901),   // Lifelike Toad
    (14629, 14657), // Loggerhead Snapjaw
    (14630, 14658), // Leatherback Snapjaw
    (14631, 14659), // Olive Snapjaw
    (14632, 14660), // Hawksbill Snapjaw
    (14633, 14661), // Albino Snapjaw
    (14755, 14778), // Tiny Green Dragon
    (14756, 14779), // Tiny Red Dragon
    (14878, 14938), // Jubling
    (15186, 15369), // Murky
    (15361, 15395), // Murki
    (15429, 15436), // Disgusting Oozeling
    (15661, 15595), // Baby Shark
    (15699, 10269), // Tranquil Mechanical Yeti
    (16069, 15984), // Gurky
    (16085, 15992), // Peddlefeet
    (16445, 15398), // Terky
    (16456, 16189), // Poley
    (16547, 16259), // Speedy
    (16548, 16257), // Mr. Wiggles
    (16549, 2176),  // Whiskers the Rat
    (16701, 16587), // Spirit of Summer
    (17254, 16942), // White Tiger Cub
    (17255, 16943), // Hippogryph Hatchling
];

impl CompanionLook {
    /// Its `CreatureDisplayInfo.dbc` id; `None` for a pet npc [`PET_NPCS`] lacks.
    pub fn display(self) -> Option<u32> {
        match self {
            CompanionLook::Creature { display } => Some(display),
            CompanionLook::Pet { npc } => PET_NPCS.iter().find(|(n, _)| *n == npc).map(|(_, d)| *d),
        }
    }
}

impl Companion {
    /// Its display id (0 for a pet npc the table lacks, which the checks refuse).
    pub fn display(&self) -> u32 {
        self.look.display().unwrap_or(0)
    }

    /// The achievement that gives it, if any.
    pub fn achievement(&self) -> Option<&str> {
        match &self.unlock {
            Some(CompanionUnlock::Achievement(a)) => Some(a),
            None => None,
        }
    }
}

/// What the shop sells besides the companions (`shop.ron`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Shop {
    /// The companion upgrade, one for every companion of the warband: its ranks in order, the
    /// first owned from the start (price 0).
    pub companion_ranks: Vec<CompanionRank>,
    /// The banish upgrade: its ranks in order, none owned from the start; rank
    /// n gives its `charges` banishes a run.
    #[serde(default)]
    pub banish_ranks: Vec<ChargeRank>,
    /// The skip upgrade, as the banish's.
    #[serde(default)]
    pub skip_ranks: Vec<ChargeRank>,
    /// The buffs for the hub in Tanaris, each bought once.
    #[serde(default)]
    pub hub_buffs: Vec<HubBuff>,
}

/// A buff for the hub in Tanaris (the neutral camp), bought once for the whole warband.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HubBuff {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub text: String,
    /// Points to spend it costs.
    pub price: u32,
    /// How much faster the hero runs in the hub (0.6: 60%).
    pub run_faster: f32,
}

/// The shop item hub buff `id` is bought as.
pub fn hub_buff_item(id: &str) -> String {
    format!("hub_buff:{id}")
}

/// One rank of an upgrade that gives charges each run (banishes, skips).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChargeRank {
    /// Points to spend it costs.
    pub price: u32,
    /// How many a run it gives (not added to the ranks before: rank 2's is the total).
    pub charges: u32,
}

/// The shop item rank `rank` (1, 2 ...) of the banish upgrade is bought as.
pub fn banish_rank_item(rank: usize) -> String {
    format!("banish_rank:{rank}")
}

/// The shop item rank `rank` (1, 2 ...) of the skip upgrade is bought as.
pub fn skip_rank_item(rank: usize) -> String {
    format!("skip_rank:{rank}")
}

/// One rank of the companion upgrade.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CompanionRank {
    /// Points to spend it costs (the first rank: 0).
    pub price: u32,
    /// Yards past the hero's own pickup range it fetches gold from.
    pub reach: f32,
    /// Its run speed, yards a second.
    pub speed: f32,
    /// Gold piles it carries before it runs back to its hero.
    pub carry: u32,
}

/// The shop item a companion is bought as (the save keeps the bought by these keys).
pub fn companion_item(id: &str) -> String {
    format!("companion:{id}")
}

/// The shop item rank `rank` (2, 3 ...) of the companion upgrade is bought as.
pub fn companion_rank_item(rank: usize) -> String {
    format!("companion_rank:{rank}")
}

impl Content {
    pub fn companion(&self, id: &str) -> Option<&Companion> {
        self.companions.iter().find(|c| c.id == id)
    }

    /// The companion upgrade's rank `rank` (1 the first); the last for one past it, `None` when
    /// the shop has none.
    pub fn companion_rank(&self, rank: usize) -> Option<&CompanionRank> {
        let ranks = &self.shop.companion_ranks;
        ranks.get(rank.max(1) - 1).or(ranks.last())
    }

    /// The price of shop item `item` (`companion:<id>`, `companion_rank:<n>`); `None` for one the
    /// shop does not sell.
    pub fn shop_price(&self, item: &str) -> Option<u32> {
        let (kind, id) = item.split_once(':')?;
        match kind {
            "companion" => self.companion(id)?.price,
            "companion_rank" => {
                let n: usize = id.parse().ok()?;
                if n < 2 {
                    return None;
                }
                self.shop.companion_ranks.get(n - 1).map(|r| r.price)
            }
            "banish_rank" | "skip_rank" => {
                let n: usize = id.parse().ok()?;
                let ranks = if kind == "banish_rank" { &self.shop.banish_ranks } else { &self.shop.skip_ranks };
                ranks.get(n.checked_sub(1)?).map(|r| r.price)
            }
            "hub_buff" => self.shop.hub_buffs.iter().find(|b| b.id == id).map(|b| b.price),
            // `tabard:`, `glow:`, `decoration:`, `camp_spot:` (crate::cosmetics)
            _ => self.cosmetic_price(kind, id),
        }
    }

    /// `companions.ron` and `shop.ron`'s rules: ids once each, a name, text and icon, a look the
    /// 1.12 tables have and a size above 0; a price or an achievement, never both, the
    /// achievement known and giving it; every companion an achievement gives listed with no price;
    /// the upgrade's first rank free and every rank reaching, running and carrying more than the
    /// one before.
    pub(crate) fn check_companions(&self, problems: &mut Vec<String>) {
        let mut seen = HashSet::new();
        for c in &self.companions {
            let id = &c.id;
            if !seen.insert(id.as_str()) {
                problems.push(format!("companion {id} is listed twice"));
            }
            if c.name.trim().is_empty() || c.text.trim().is_empty() || c.icon.trim().is_empty() {
                problems.push(format!("companion {id} needs a name, a text and an icon"));
            }
            if c.look.display().is_none() {
                problems.push(format!(
                    "companion {id} is pet npc {:?}, which is no 1.12 companion pet",
                    c.look
                ));
            }
            if !(c.scale.is_finite() && c.scale > 0.0) {
                problems.push(format!("companion {id} has a size of {}", c.scale));
            }
            match (c.price, c.achievement()) {
                (Some(_), Some(_)) => problems.push(format!(
                    "companion {id} has a price and an achievement: one only"
                )),
                (None, None) => problems.push(format!(
                    "companion {id} has neither a price nor an achievement"
                )),
                (Some(0), None) => problems.push(format!("companion {id} costs 0")),
                (None, Some(a)) => {
                    let gives = self
                        .achievement(a)
                        .is_some_and(|x| x.rewards.contains(&Reward::Companion(id.clone())));
                    if !gives {
                        problems.push(format!(
                            "companion {id} comes from achievement {a}, which does not give it"
                        ));
                    }
                }
                _ => {}
            }
        }
        for a in &self.achievements {
            for r in &a.rewards {
                if let Reward::Companion(id) = r {
                    match self.companion(id) {
                        None => problems.push(format!("achievement {} gives companion {id}, which companions.ron lacks", a.id)),
                        Some(c) if c.price.is_some() => {
                            problems.push(format!("achievement {} gives companion {id}, which the shop sells: give it no price", a.id))
                        }
                        Some(c) if c.achievement() != Some(a.id.as_str()) => {
                            problems.push(format!("achievement {} gives companion {id}, whose unlock names another", a.id))
                        }
                        _ => {}
                    }
                }
            }
        }
        let ranks = &self.shop.companion_ranks;
        if ranks.is_empty() && !self.companions.is_empty() {
            problems.push("shop.ron has no companion ranks".into());
        }
        if ranks.first().is_some_and(|r| r.price != 0) {
            problems
                .push("shop.ron: the first companion rank is owned from the start, price 0".into());
        }
        for (k, w) in ranks.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            if !(b.reach > a.reach && b.speed >= a.speed && b.carry >= a.carry && b.price > 0) {
                problems.push(format!("shop.ron: companion rank {} must cost something and reach farther than rank {}", k + 2, k + 1));
            }
        }
        for r in ranks {
            if !(r.reach.is_finite()
                && r.reach >= 0.0
                && r.speed.is_finite()
                && r.speed > 0.0
                && r.carry > 0)
            {
                problems.push(
                    "shop.ron: a companion rank needs a reach, a speed above 0 and room to carry"
                        .into(),
                );
            }
        }
        // the banish and skip ranks: each costs something and gives more than the one before
        for (name, ranks) in [("banish", &self.shop.banish_ranks), ("skip", &self.shop.skip_ranks)] {
            let mut last = 0;
            for (k, r) in ranks.iter().enumerate() {
                if r.price == 0 || r.charges <= last {
                    problems.push(format!("shop.ron: {name} rank {} must cost something and give more than the one before", k + 1));
                }
                last = r.charges;
            }
        }
    }
}
