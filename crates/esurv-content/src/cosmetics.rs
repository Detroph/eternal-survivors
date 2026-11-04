//! The looks the shop sells besides the companions, and what the camp fire is decorated with:
//! tabards (`tabards.ron`), weapon glows (`weapon_glows.ron`) and camp decorations
//! (`decorations.ron`), with the titles achievements give (`achievements.ron`). None of them
//! changes a number of the game. docs/game/achievements.md has the rules; docs/game/AUTHORING.md
//! "A tabard", "A weapon glow" and "A camp decoration" the how-to.

use std::collections::HashSet;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{Content, Faction, Reward};

/// One tabard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tabard {
    /// Unique, lowercase; kept in the save once shipped.
    pub id: String,
    pub name: String,
    /// A texture name from `Interface\Icons`.
    pub icon: String,
    pub look: TabardLook,
    /// Points to spend.
    pub price: u32,
    /// Its shop tooltip, our own words.
    pub text: String,
}

/// What a tabard looks like, and who may wear it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabardLook {
    /// One look (an `ItemDisplayInfo.dbc` id) any hero may wear.
    Neutral { display: u32 },
    /// A pair sold as one: each hero shows its own faction's.
    Faction { horde: u32, alliance: u32 },
    /// Only that faction's heroes may wear it.
    Only { faction: Faction, display: u32 },
}

impl TabardLook {
    /// The look a hero of `faction` shows; `None` when it may not wear it.
    pub fn display_for(self, faction: Faction) -> Option<u32> {
        match self {
            TabardLook::Neutral { display } => Some(display),
            TabardLook::Faction { horde, alliance } => Some(match faction {
                Faction::Horde => horde,
                Faction::Alliance => alliance,
            }),
            TabardLook::Only {
                faction: f,
                display,
            } => (f == faction).then_some(display),
        }
    }

    fn displays(self) -> Vec<u32> {
        match self {
            TabardLook::Neutral { display } | TabardLook::Only { display, .. } => vec![display],
            TabardLook::Faction { horde, alliance } => vec![horde, alliance],
        }
    }
}

/// One weapon glow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaponGlow {
    /// Unique, lowercase; kept in the save once shipped.
    pub id: String,
    pub name: String,
    pub icon: String,
    /// The `SpellItemEnchantment.dbc` id whose item visual is the glow.
    pub enchant: u32,
    pub price: u32,
    /// A glow the warband must own before this one is sold (the damage glows' ladder).
    #[serde(
        default,
        deserialize_with = "plain_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub needs: Option<String>,
    pub text: String,
}

/// 1.12's inventory types a weapon glow shows on: one-hand (13), bow (15),
/// two-hand (17), main hand (21), off-hand weapon (22), thrown (25), gun, crossbow and wand (26);
/// never a shield (14), a held off-hand item (23) or a relic (28).
pub const GLOWING_INVENTORY_TYPES: [u32; 7] = [13, 15, 17, 21, 22, 25, 26];

/// Whether a weapon glow shows on an item of inventory type `t`.
pub fn glows_on(t: u32) -> bool {
    GLOWING_INVENTORY_TYPES.contains(&t)
}

/// A camp decoration's look: one for both camps, or each camp its own (each at its own size
/// against the piece's, when the two models are not alike in size).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DecorLook {
    Same(u32),
    Faction {
        horde: u32,
        alliance: u32,
        #[serde(default = "one", skip_serializing_if = "is_one")]
        horde_scale: f32,
        #[serde(default = "one", skip_serializing_if = "is_one")]
        alliance_scale: f32,
    },
}

impl DecorLook {
    /// The `GameObjectDisplayInfo.dbc` id the camp of `faction` shows.
    pub fn display_for(self, faction: Faction) -> u32 {
        match self {
            DecorLook::Same(d) => d,
            DecorLook::Faction {
                horde, alliance, ..
            } => match faction {
                Faction::Horde => horde,
                Faction::Alliance => alliance,
            },
        }
    }

    /// The size the camp of `faction` shows it at, against the piece's own.
    pub fn scale_for(self, faction: Faction) -> f32 {
        match self {
            DecorLook::Same(_) => 1.0,
            DecorLook::Faction {
                horde_scale,
                alliance_scale,
                ..
            } => match faction {
                Faction::Horde => horde_scale,
                Faction::Alliance => alliance_scale,
            },
        }
    }

    fn displays(self) -> Vec<u32> {
        match self {
            DecorLook::Same(d) => vec![d],
            DecorLook::Faction {
                horde, alliance, ..
            } => vec![horde, alliance],
        }
    }

    fn scales(self) -> [f32; 2] {
        [Faction::Horde, Faction::Alliance].map(|f| self.scale_for(f))
    }
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

/// What a camp spot holds, and where the pieces that stand on no spot go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpotKind {
    /// The camp fire itself (empty: the fire that grows with progress).
    Campfire,
    BannerPost,
    /// The camp's theme: one bought set, whose pieces stand in their own places. What stands on it is a set's id.
    Theme,
    /// An extra spot, for any ground piece the warband owns.
    Ground,
    /// A trophy: no spot, it stands in the trophy corner once earned.
    Trophy,
    /// A set's signpost: no spot, the camp sign stands once one is earned.
    Sign,
    /// A spot at a hero's tent in the walkable camp (`ESURV_HUB`; plans/hub-decoration-spots.md):
    /// the hero's own things, on none of the warband screen's spots.
    Tent,
}

impl SpotKind {
    pub fn label(self) -> &'static str {
        match self {
            SpotKind::Campfire => "Camp fire",
            SpotKind::BannerPost => "Banner post",
            SpotKind::Theme => "Camp theme",
            SpotKind::Ground => "Extra spot",
            SpotKind::Trophy => "Trophy corner",
            SpotKind::Sign => "Camp sign",
            SpotKind::Tent => "Tent spot",
        }
    }

    /// Whether a camp spot of this kind is picked on the Decorate camp screen.
    pub fn is_spot(self) -> bool {
        matches!(
            self,
            SpotKind::Campfire | SpotKind::BannerPost | SpotKind::Theme | SpotKind::Ground
        )
    }
}

/// One spot round the camp fire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampSpot {
    pub id: String,
    pub kind: SpotKind,
    /// Yards from the camp fire as the camera sees it: (to its right, beyond it).
    pub at: (f32, f32),
    /// Points it costs to open, bought once for both camps; `None`: open from the start.
    #[serde(
        default,
        deserialize_with = "plain_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub price: Option<u32>,
}

/// How a piece that is never sold comes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Earned {
    /// With this achievement.
    Achievement(String),
    /// With this zone's first clear (any hero).
    ZoneClear(String),
    /// With owning every piece of this set.
    Set(String),
    /// With the first of these achievements; the look is the highest earned's.
    Ladder(Vec<LadderStep>),
    /// Free to every hero of this class (a tent's class piece in the walkable camp): the hero's,
    /// not the warband's.
    Class(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LadderStep {
    pub achievement: String,
    pub look: DecorLook,
    /// Its size against the model's own.
    #[serde(default = "one")]
    pub scale: f32,
}

/// A value written plainly (`price: 25`) read as `Some`.
fn plain_some<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

fn one() -> f32 {
    1.0
}

/// One camp decoration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decoration {
    /// Unique, lowercase; kept in the save once shipped.
    pub id: String,
    pub name: String,
    pub icon: String,
    /// The spots it may stand on.
    pub spot: SpotKind,
    pub look: DecorLook,
    /// Its size against the model's own.
    #[serde(default = "one")]
    pub scale: f32,
    /// Degrees it turns from facing the camp fire.
    #[serde(default)]
    pub turn: f32,
    /// Where it stands when it stands on no spot, yards from the camp fire as the camera sees
    /// it (to its right, beyond): a set's ground piece in its set's arrangement (while the set is
    /// the camp's theme), a trophy in the trophy corner.
    #[serde(
        default,
        deserialize_with = "plain_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub at: Option<(f32, f32)>,
    /// The set it belongs to.
    #[serde(
        default,
        deserialize_with = "plain_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub set: Option<String>,
    /// Points to spend; `None` for one that is earned.
    #[serde(
        default,
        deserialize_with = "plain_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub price: Option<u32>,
    /// Pieces the warband must own before this one is sold.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<String>,
    #[serde(
        default,
        deserialize_with = "plain_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub earned: Option<Earned>,
    pub text: String,
}

impl Decoration {
    /// Whether it is a trophy (earned, never sold, on a trophy spot).
    pub fn trophy(&self) -> bool {
        self.spot == SpotKind::Trophy
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecorationSet {
    pub id: String,
    pub name: String,
}

/// A look that shows once an achievement is earned (none: from the start).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgressLook {
    #[serde(
        default,
        deserialize_with = "plain_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub achievement: Option<String>,
    pub look: DecorLook,
    /// Yards from its anchor: (to the camera's right, beyond).
    #[serde(default)]
    pub at: (f32, f32),
    /// Yards over the ground.
    #[serde(default)]
    pub lift: f32,
    #[serde(default)]
    pub turn: f32,
    /// Its size against the model's own (a city's great fire shrunk to a camp's).
    #[serde(default = "one")]
    pub scale: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneBanner {
    pub zone: String,
    pub look: DecorLook,
    /// Its size against the model's own (each banner brought to a pennant's height).
    #[serde(default = "one")]
    pub scale: f32,
}

/// The banner line: pennants in a row from `from`, `step` yards apart to the camera's right,
/// along the rope (`rope`: its posts and lengths, from the start, placed from `from`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BannerLine {
    pub from: (f32, f32),
    pub step: f32,
    #[serde(default)]
    pub rope: Vec<ProgressLook>,
    pub banners: Vec<ZoneBanner>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkullSpear {
    pub at: (f32, f32),
    pub spear: ProgressLook,
    pub skulls: Vec<ProgressLook>,
}

/// `decorations.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decorations {
    pub spots: Vec<CampSpot>,
    pub sets: Vec<DecorationSet>,
    pub pieces: Vec<Decoration>,
    /// The fire's looks, the last one earned showing.
    pub campfire: Vec<ProgressLook>,
    pub banner_line: BannerLine,
    pub skull_spear: SkullSpear,
    /// Where the camp sign stands once a set's signpost is earned: one sign however many.
    #[serde(default)]
    pub camp_sign: (f32, f32),
}

impl Default for Decorations {
    fn default() -> Self {
        Decorations {
            spots: Vec::new(),
            sets: Vec::new(),
            pieces: Vec::new(),
            campfire: Vec::new(),
            camp_sign: (0.0, 0.0),
            banner_line: BannerLine {
                from: (0.0, 0.0),
                step: 1.0,
                rope: Vec::new(),
                banners: Vec::new(),
            },
            skull_spear: SkullSpear {
                at: (0.0, 0.0),
                spear: ProgressLook {
                    achievement: None,
                    look: DecorLook::Same(0),
                    at: (0.0, 0.0),
                    lift: 0.0,
                    turn: 0.0,
                    scale: 1.0,
                },
                skulls: Vec::new(),
            },
        }
    }
}

/// The shop item a tabard is bought as.
pub fn tabard_item(id: &str) -> String {
    format!("tabard:{id}")
}

/// The shop item a weapon glow is bought as.
pub fn glow_item(id: &str) -> String {
    format!("glow:{id}")
}

/// The shop item a camp decoration is bought as.
pub fn decoration_item(id: &str) -> String {
    format!("decoration:{id}")
}

/// The shop item a camp spot is opened with.
pub fn camp_spot_item(id: &str) -> String {
    format!("camp_spot:{id}")
}

/// What the warband has, as the owning rules read it: what the shop sold it, the achievements
/// earned and the zones cleared.
pub struct Holdings<'a> {
    pub bought: &'a dyn Fn(&str) -> bool,
    pub earned: &'a dyn Fn(&str) -> bool,
    pub cleared: &'a dyn Fn(&str) -> bool,
}

impl Content {
    pub fn tabard(&self, id: &str) -> Option<&Tabard> {
        self.tabards.iter().find(|t| t.id == id)
    }

    pub fn weapon_glow(&self, id: &str) -> Option<&WeaponGlow> {
        self.weapon_glows.iter().find(|g| g.id == id)
    }

    pub fn decoration(&self, id: &str) -> Option<&Decoration> {
        self.decorations.pieces.iter().find(|d| d.id == id)
    }

    pub fn camp_spot(&self, id: &str) -> Option<&CampSpot> {
        self.decorations.spots.iter().find(|s| s.id == id)
    }

    /// The pieces of set `set`.
    pub fn set_pieces<'c>(&'c self, set: &'c str) -> impl Iterator<Item = &'c Decoration> + 'c {
        self.decorations
            .pieces
            .iter()
            .filter(move |d| d.set.as_deref() == Some(set) && d.price.is_some())
    }

    /// The price of a cosmetic shop item (`tabard:`, `glow:`, `decoration:`, `camp_spot:`);
    /// `None` for one the shop does not sell.
    pub(crate) fn cosmetic_price(&self, kind: &str, id: &str) -> Option<u32> {
        match kind {
            "tabard" => Some(self.tabard(id)?.price),
            "glow" => Some(self.weapon_glow(id)?.price),
            "decoration" => self.decoration(id)?.price,
            "camp_spot" => self.camp_spot(id)?.price,
            _ => None,
        }
    }

    /// What a cosmetic shop item waits for before it is sold (another glow, other pieces); empty
    /// for one that waits for nothing.
    pub fn shop_needs(&self, item: &str) -> Vec<String> {
        match item.split_once(':') {
            Some(("glow", id)) => self
                .weapon_glow(id)
                .and_then(|g| g.needs.as_deref())
                .map(|n| vec![glow_item(n)])
                .unwrap_or_default(),
            Some(("decoration", id)) => self
                .decoration(id)
                .map(|d| d.needs.iter().map(|n| decoration_item(n)).collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// Whether the warband has decoration `id`: bought, or earned as its `earned` says.
    pub fn owns_decoration(&self, id: &str, h: &Holdings) -> bool {
        let Some(d) = self.decoration(id) else {
            return false;
        };
        if d.price.is_some() {
            return (h.bought)(&decoration_item(id));
        }
        match &d.earned {
            Some(Earned::Achievement(a)) => (h.earned)(a),
            Some(Earned::ZoneClear(z)) => (h.cleared)(z),
            Some(Earned::Set(s)) => {
                let mut pieces = self.set_pieces(s).peekable();
                pieces.peek().is_some() && pieces.all(|p| (h.bought)(&decoration_item(&p.id)))
            }
            Some(Earned::Ladder(steps)) => steps.iter().any(|s| (h.earned)(&s.achievement)),
            // a hero's, never the warband's ([`Content::hero_owns_decoration`])
            Some(Earned::Class(_)) | None => false,
        }
    }

    /// Whether a hero of class `class` has decoration `id`: the warband's, or free to its class.
    pub fn hero_owns_decoration(&self, id: &str, class: &str, h: &Holdings) -> bool {
        self.owns_decoration(id, h)
            || self.decoration(id).is_some_and(|d| d.earned == Some(Earned::Class(class.to_owned())))
    }

    /// The look decoration `d` shows in the camp of `faction` (a ladder's highest earned step).
    pub fn decoration_display(&self, d: &Decoration, faction: Faction, h: &Holdings) -> u32 {
        self.decoration_look(d, faction, h).0
    }

    /// The look and size decoration `d` shows in the camp of `faction`.
    pub fn decoration_look(&self, d: &Decoration, faction: Faction, h: &Holdings) -> (u32, f32) {
        if let Some(Earned::Ladder(steps)) = &d.earned {
            if let Some(s) = steps.iter().rev().find(|s| (h.earned)(&s.achievement)) {
                return (
                    s.look.display_for(faction),
                    s.scale * s.look.scale_for(faction),
                );
            }
        }
        (
            d.look.display_for(faction),
            d.scale * d.look.scale_for(faction),
        )
    }

    /// Whether the warband owns a piece of set `set` (so it may be a camp's theme).
    pub fn owns_set_piece(&self, set: &str, h: &Holdings) -> bool {
        self.set_pieces(set)
            .any(|p| (h.bought)(&decoration_item(&p.id)))
    }

    /// Whether `piece` may be set out on spot `spot`: it fits, the warband has it (for the
    /// theme, a piece of the set) and the spot is open.
    pub fn may_set_out(&self, piece: &str, spot: &str, h: &Holdings) -> bool {
        let theme = self
            .camp_spot(spot)
            .is_some_and(|s| s.kind == SpotKind::Theme);
        let has = if theme {
            self.owns_set_piece(piece, h)
        } else {
            self.owns_decoration(piece, h)
        };
        self.fits(piece, spot) && has && self.spot_open(spot, h)
    }

    /// Whether camp spot `id` is open: free from the start, or bought.
    pub fn spot_open(&self, id: &str, h: &Holdings) -> bool {
        self.camp_spot(id)
            .is_some_and(|s| s.price.is_none() || (h.bought)(&camp_spot_item(id)))
    }

    /// The fire's look as progress has it: the last step earned.
    pub fn grown_campfire(&self, h: &Holdings) -> Option<&ProgressLook> {
        self.decorations
            .campfire
            .iter()
            .filter(|l| l.achievement.as_deref().is_none_or(|a| (h.earned)(a)))
            .last()
    }

    /// Whether piece `piece` may stand on spot `spot`; on the theme spot, whether `piece` is a
    /// set.
    pub fn fits(&self, piece: &str, spot: &str) -> bool {
        match self.camp_spot(spot) {
            Some(s) if s.kind == SpotKind::Theme => {
                self.decorations.sets.iter().any(|x| x.id == piece)
            }
            Some(s) => self.decoration(piece).is_some_and(|d| d.spot == s.kind),
            None => false,
        }
    }

    /// `tabards.ron`, `weapon_glows.ron` and `decorations.ron`'s rules: ids once each, a name,
    /// text and icon, a price above 0, looks above 0; a glow's ladder of known glows; spots of
    /// one fire, a known set or need for every piece, a price or an earned rule (never both, a
    /// trophy always earned), known achievements and zones; every trophy an achievement gives
    /// listed and earned by it; the progress looks' achievements known, the banner line's zones
    /// the game's.
    pub(crate) fn check_cosmetics(&self, problems: &mut Vec<String>) {
        let mut seen = HashSet::new();
        for t in &self.tabards {
            if !seen.insert(t.id.as_str()) {
                problems.push(format!("tabard {} is listed twice", t.id));
            }
            if t.name.trim().is_empty() || t.text.trim().is_empty() || t.icon.trim().is_empty() {
                problems.push(format!("tabard {} needs a name, a text and an icon", t.id));
            }
            if t.price == 0 || t.look.displays().contains(&0) {
                problems.push(format!("tabard {} needs a price and looks above 0", t.id));
            }
        }
        let mut seen = HashSet::new();
        for g in &self.weapon_glows {
            if !seen.insert(g.id.as_str()) {
                problems.push(format!("weapon glow {} is listed twice", g.id));
            }
            if g.name.trim().is_empty() || g.text.trim().is_empty() || g.icon.trim().is_empty() {
                problems.push(format!(
                    "weapon glow {} needs a name, a text and an icon",
                    g.id
                ));
            }
            if g.price == 0 || g.enchant == 0 {
                problems.push(format!(
                    "weapon glow {} needs a price and an enchant above 0",
                    g.id
                ));
            }
            if let Some(n) = &g.needs {
                if n == &g.id || self.weapon_glow(n).is_none() {
                    problems.push(format!(
                        "weapon glow {} needs {n}, which is no other glow",
                        g.id
                    ));
                }
            }
        }
        let d = &self.decorations;
        let mut seen = HashSet::new();
        for s in &d.spots {
            if !seen.insert(s.id.as_str()) {
                problems.push(format!("camp spot {} is listed twice", s.id));
            }
            if s.price == Some(0) {
                problems.push(format!("camp spot {} costs 0", s.id));
            }
            if !s.kind.is_spot() {
                problems.push(format!(
                    "camp spot {} is a {:?}: those stand on no spot",
                    s.id, s.kind
                ));
            }
        }
        if d.spots.iter().filter(|s| s.kind == SpotKind::Theme).count() > 1 {
            problems.push("decorations.ron has more than one camp theme spot".into());
        }
        if d.spots
            .iter()
            .filter(|s| s.kind == SpotKind::Campfire)
            .count()
            != 1
            && !d.spots.is_empty()
        {
            problems.push("decorations.ron needs exactly one camp fire spot".into());
        }
        let known_achievement = |a: &str| self.achievement(a).is_some();
        let known_zone = |z: &str| self.zones.iter().any(|x| x.id == z);
        let mut seen = HashSet::new();
        for p in &d.pieces {
            let id = &p.id;
            if !seen.insert(id.as_str()) {
                problems.push(format!("decoration {id} is listed twice"));
            }
            if p.name.trim().is_empty() || p.text.trim().is_empty() || p.icon.trim().is_empty() {
                problems.push(format!("decoration {id} needs a name, a text and an icon"));
            }
            let sized = |s: f32| s.is_finite() && s > 0.0;
            if p.look.displays().contains(&0)
                || !sized(p.scale)
                || !p.look.scales().into_iter().all(sized)
            {
                problems.push(format!(
                    "decoration {id} needs looks above 0 and a size above 0"
                ));
            }
            if p.spot.is_spot() && !d.spots.iter().any(|s| s.kind == p.spot) && !d.spots.is_empty()
            {
                problems.push(format!(
                    "decoration {id} stands on a {:?}, which no spot is",
                    p.spot
                ));
            }
            let placed =
                p.spot == SpotKind::Trophy || (p.spot == SpotKind::Ground && p.set.is_some());
            if placed && p.at.is_none() {
                problems.push(format!(
                    "decoration {id} needs its place (`at`): a trophy's in the corner, a set piece's in its set"
                ));
            }
            if let Some(s) = &p.set {
                if !d.sets.iter().any(|x| &x.id == s) {
                    problems.push(format!(
                        "decoration {id} is in set {s}, which decorations.ron lacks"
                    ));
                }
            }
            for n in &p.needs {
                if n == id || self.decoration(n).is_none_or(|x| x.price.is_none()) {
                    problems.push(format!(
                        "decoration {id} needs {n}, which is no other piece for sale"
                    ));
                }
            }
            match (&p.price, &p.earned) {
                (Some(_), Some(_)) => problems.push(format!(
                    "decoration {id} has a price and an earned rule: one only"
                )),
                (None, None) => problems.push(format!(
                    "decoration {id} has neither a price nor an earned rule"
                )),
                (Some(0), _) => problems.push(format!("decoration {id} costs 0")),
                _ => {}
            }
            if p.trophy() && p.price.is_some() {
                problems.push(format!("trophy {id} is sold: trophies are only earned"));
            }
            match &p.earned {
                Some(Earned::Achievement(a)) => {
                    let gives = self
                        .achievement(a)
                        .is_some_and(|x| x.rewards.contains(&Reward::Trophy(id.clone())));
                    if p.trophy() && !gives {
                        problems.push(format!(
                            "trophy {id} comes from achievement {a}, which does not give it"
                        ));
                    }
                    if !known_achievement(a) {
                        problems.push(format!(
                            "decoration {id} comes from unknown achievement {a}"
                        ));
                    }
                }
                Some(Earned::ZoneClear(z)) if !known_zone(z) => problems.push(format!(
                    "decoration {id} comes from clearing unknown zone {z}"
                )),
                Some(Earned::Set(s)) if self.set_pieces(s).next().is_none() => problems.push(
                    format!("decoration {id} comes from set {s}, which has no pieces for sale"),
                ),
                Some(Earned::Class(c)) if !self.classes.iter().any(|x| &x.id == c) => {
                    problems.push(format!("decoration {id} is free to unknown class {c}"))
                }
                Some(Earned::Ladder(steps)) => {
                    if steps.is_empty() {
                        problems.push(format!("decoration {id} has an empty ladder"));
                    }
                    for s in steps {
                        let gives = self
                            .achievement(&s.achievement)
                            .is_some_and(|x| x.rewards.contains(&Reward::Trophy(id.clone())));
                        if !gives || s.look.displays().contains(&0) {
                            problems.push(format!("decoration {id}'s ladder names {}, which does not give it (or a look of 0)", s.achievement));
                        }
                    }
                }
                _ => {}
            }
        }
        for a in &self.achievements {
            for r in &a.rewards {
                if let Reward::Trophy(t) = r {
                    match self.decoration(t) {
                        Some(p) if p.trophy() => {}
                        _ => problems.push(format!(
                            "achievement {} gives trophy {t}, which decorations.ron lacks",
                            a.id
                        )),
                    }
                }
            }
        }
        // content built in code has no decorations at all
        if d.spots.is_empty() {
            return;
        }
        let progress = d
            .campfire
            .iter()
            .chain([&d.skull_spear.spear])
            .chain(&d.skull_spear.skulls)
            .chain(&d.banner_line.rope);
        for l in progress {
            if let Some(a) = &l.achievement {
                if !known_achievement(a) {
                    problems.push(format!("decorations.ron names unknown achievement {a}"));
                }
            }
            if l.look.displays().contains(&0) {
                problems.push("decorations.ron has a progress look of 0".into());
            }
        }
        if !d.spots.is_empty() && d.campfire.first().is_none_or(|l| l.achievement.is_some()) {
            problems.push("decorations.ron: the first camp fire look is the one from the start (no achievement)".into());
        }
        for b in &d.banner_line.banners {
            if !known_zone(&b.zone) || b.look.displays().contains(&0) || !(b.scale > 0.0) {
                problems.push(format!(
                    "the banner line names unknown zone {} (or a look of 0)",
                    b.zone
                ));
            }
        }
    }
}
