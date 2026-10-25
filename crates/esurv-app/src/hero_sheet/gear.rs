//! The class gear, from the game's content and the class's progress: the class's pieces by
//! slot and zone (names, slots, icons' display ids, what each adds), what is worn and which look
//! each slot shows, what worn gear adds in all, and which boss drops what in each zone.
//!
//! Each zone drops its own pieces, each with its own name and look (a
//! zone may have several rows of looks: Blackrock two, which its piece unlocks both); its quality
//! follows the zone's tier, and a piece says which zone it came from.

use std::collections::BTreeMap;

use esurv_content::{Class, Content, GearSet, Stat, ZoneScaling};
use esurv_protocol::Progress;
use esurv_sim::game::gear::{
    armament, armor_display, decode_item_entry, look_zone, piece_modifiers, piece_name, shown_looks,
    zone_look, zone_piece_name, ARMOR_SLOTS,
};
use esurv_sim::game::rewards::{boss_drop_slots, gear_item_entry, zone_drop_chance, zone_tier};
use esurv_sim::game::stats::Modifier;

use crate::cards::card_text::{number, percent};
use crate::run_end::tally::slot_name;
use crate::stock::TipLine;
use crate::zone_discovery::{discovered_in, shown_name};
use crate::ui::{rarity_colour, GRAY_TEXT, GREEN_TEXT, WHITE_TEXT};

/// The paper doll's slots, as the stock character pane lays them out: the left column (head,
/// shoulders, chest, wrists), the right (hands, waist, legs, feet); the weapons go below.
pub const LEFT_SLOTS: [u32; 4] = [0, 1, 2, 6];
pub const RIGHT_SLOTS: [u32; 4] = [7, 3, 4, 5];

/// A class's gear: its class and set in the content.
#[derive(Clone, Copy)]
pub struct ClassGear<'c> {
    pub content: &'c Content,
    pub class: &'c Class,
    pub set: &'c GearSet,
}

impl<'c> ClassGear<'c> {
    /// The gear of class `class` (its id), if the content has it.
    pub fn of(content: &'c Content, class: &str) -> Option<Self> {
        let set = content.gear_set(class)?;
        let class = content.classes.iter().find(|c| c.id == class)?;
        Some(Self {
            content,
            class,
            set,
        })
    }

    /// How many slots the class has: the eight armor slots, then its weapons.
    pub fn slots(&self) -> u32 {
        ARMOR_SLOTS + self.set.weapons.len() as u32
    }

    /// The weapon slots, in order.
    pub fn weapon_slots(&self) -> std::ops::Range<u32> {
        ARMOR_SLOTS..self.slots()
    }

    /// How many zones drop gear (every zone of the content).
    pub fn zones(&self) -> u32 {
        self.content.zones.len() as u32
    }

    /// The gear tier zone `zone` drops, from its difficulty: its pieces' quality.
    pub fn tier(&self, zone: u32) -> u32 {
        zone_tier(self.content, zone as usize)
    }

    /// A piece's quality colour index: its zone's tier's.
    pub fn quality(&self, zone: u32) -> u8 {
        tier_quality(self.tier(zone))
    }

    /// A look's quality colour index: its zone's tier's (the first zone's for a look of none).
    pub fn look_quality(&self, look: u32) -> u8 {
        self.quality(self.look_zone(look).unwrap_or(0))
    }

    /// The look a piece of zone `zone` shows of its own: the zone's first row (every
    /// zone its own look).
    pub fn own_look(&self, zone: u32) -> u32 {
        zone_look(self.content, self.set, zone)
    }

    /// The zone look `look` is of; `None` for a look of a zone the content does not have.
    pub fn look_zone(&self, look: u32) -> Option<u32> {
        look_zone(self.content, self.set, look)
    }

    /// The looks the Collection Log shows, (zone, look) a row each: every zone's, a zone's rows in
    /// order (Blackrock's two).
    pub fn rows(&self) -> Vec<(u32, u32)> {
        shown_looks(self.content, self.set)
    }

    /// Look `look`'s row among its zone's looks (0 the first).
    pub fn look_row(&self, look: u32) -> u32 {
        self.set.looks.get(look as usize).map_or(0, |l| l.row)
    }

    /// Look `look`'s row name, where its row has one (Stratholme's
    /// second row, "Scholomance sets"); shown only once its zone is discovered.
    pub fn look_row_name(&self, progress: Option<&Progress>, look: u32) -> Option<&'c str> {
        let zone = self.look_zone(look)?;
        if !discovered_in(self.content, progress, zone as usize) {
            return None;
        }
        self.set.looks.get(look as usize)?.row_name.as_deref()
    }

    /// A zone's name as `progress` may see it: "???" until the save has discovered it
    /// ("" for none).
    pub fn zone_name(&self, progress: Option<&Progress>, zone: u32) -> &'c str {
        shown_name(self.content, progress, zone as usize)
    }

    /// The piece's item entry.
    pub fn entry(&self, slot: u32, zone: u32) -> u32 {
        gear_item_entry(self.class.wow_class_id, slot, zone)
    }

    /// The (slot, zone) of an item entry of this class.
    pub fn piece_of(&self, entry: u32) -> Option<(u32, u32)> {
        let (class, slot, zone) = decode_item_entry(entry)?;
        (class == self.class.wow_class_id && slot < self.slots() && zone < self.zones())
            .then_some((slot, zone))
    }

    /// The piece's name: its zone's own look's ("Sparkmetal Coif" from `duskwood`), a weapon its
    /// zone's armaments.
    pub fn name(&self, slot: u32, zone: u32) -> String {
        zone_piece_name(self.content, self.set, slot, zone)
            .unwrap_or_else(|| format!("Item {}", self.entry(slot, zone)))
    }

    /// The name of the slot's piece in look `look`.
    pub fn look_name(&self, slot: u32, look: u32) -> String {
        piece_name(self.content, self.set, slot, look)
            .unwrap_or_else(|| format!("Item {}", self.entry(slot, 0)))
    }

    /// The weapons a weapon slot's look holds, as the collection names them: the class's three
    /// loadouts' in the look's armaments ("Duskbringer", "Darkwater Talwar and Buzz Saw" ...).
    pub fn armaments(&self, look: u32) -> Vec<String> {
        let Some(l) = self.set.looks.get(look as usize) else {
            return Vec::new();
        };
        self.content
            .loadouts_of(&self.class.id)
            .iter()
            .map(|loadout| {
                let names: Vec<&str> = loadout.weapons.iter().filter_map(|w| Some(w.look_in(l)?.name.as_str())).collect();
                names.join(" and ")
            })
            .collect()
    }

    /// Where it is worn, as the stock interface names it.
    pub fn slot_name(&self, slot: u32) -> &'static str {
        if slot < ARMOR_SLOTS {
            self.content
                .gear
                .slots
                .get(slot as usize)
                .map_or("", |s| slot_name(s.inventory_type()))
        } else {
            match self.weapon(slot).map(|w| w.equip_slot()) {
                Some(15) => "Main Hand",
                Some(16) => "Off Hand",
                Some(17) => "Ranged",
                _ => "",
            }
        }
    }

    fn weapon(&self, slot: u32) -> Option<&'c esurv_content::Weapon> {
        self.set
            .weapons
            .get(slot.checked_sub(ARMOR_SLOTS)? as usize)
    }

    /// The stock paper doll's picture for the slot while nothing is worn there.
    pub fn empty_slot_art(&self, slot: u32) -> &'static str {
        match slot {
            0 => r"Interface\PaperDoll\UI-PaperDoll-Slot-Head",
            1 => r"Interface\PaperDoll\UI-PaperDoll-Slot-Shoulder",
            2 => r"Interface\PaperDoll\UI-PaperDoll-Slot-Chest",
            3 => r"Interface\PaperDoll\UI-PaperDoll-Slot-Waist",
            4 => r"Interface\PaperDoll\UI-PaperDoll-Slot-Legs",
            5 => r"Interface\PaperDoll\UI-PaperDoll-Slot-Feet",
            6 => r"Interface\PaperDoll\UI-PaperDoll-Slot-Wrists",
            7 => r"Interface\PaperDoll\UI-PaperDoll-Slot-Hands",
            _ => match self.weapon(slot).map(|w| w.equip_slot()) {
                Some(16) => r"Interface\PaperDoll\UI-PaperDoll-Slot-SecondaryHand",
                Some(17) => r"Interface\PaperDoll\UI-PaperDoll-Slot-Ranged",
                _ => r"Interface\PaperDoll\UI-PaperDoll-Slot-MainHand",
            },
        }
    }

    /// The item display look `look` shows in the slot (its icon comes from `ItemDisplayInfo.dbc`):
    /// a weapon slot the class's loadout weapon in its hand, in the look's armaments.
    pub fn display(&self, slot: u32, look: u32) -> u32 {
        if slot < ARMOR_SLOTS {
            armor_display(self.set, slot, look)
        } else {
            armament(self.content, self.set, slot, look).map_or(0, |(_, a)| a.display)
        }
    }

    /// What the piece from zone `zone` adds.
    pub fn bonuses(&self, slot: u32, zone: u32) -> Vec<Modifier> {
        piece_modifiers(self.content, self.set, slot, zone)
            .unwrap_or_default()
    }

    /// The zone of the piece each slot wears, by slot (`None`: nothing worn).
    pub fn worn(&self, progress: &Progress) -> Vec<Option<u32>> {
        let mut worn = vec![None; self.slots() as usize];
        for &entry in &progress.worn {
            if let Some((slot, zone)) = self.piece_of(entry) {
                worn[slot as usize] = Some(zone);
            }
        }
        worn
    }

    /// The look chosen for slot `slot`, where one is (one the set has).
    pub fn chosen_look(&self, progress: &Progress, slot: u32) -> Option<u32> {
        progress
            .looks
            .iter()
            .find(|(s, _)| *s == slot)
            .map(|(_, look)| *look)
            .filter(|&l| (l as usize) < self.set.looks.len())
    }

    /// The look slot `slot` shows: its chosen look, else the worn piece's own (`None`: nothing).
    pub fn shown_look(&self, progress: &Progress, slot: u32) -> Option<u32> {
        self.chosen_look(progress, slot).or_else(|| {
            self.worn(progress)
                .get(slot as usize)
                .copied()
                .flatten()
                .map(|z| self.own_look(z))
        })
    }

    /// The look the worn piece of zone `zone` in slot `slot` goes by: a row of its own zone shown
    /// over it (Blackrock's second), else its own.
    pub fn own_zone_look(&self, progress: &Progress, slot: u32, zone: u32) -> u32 {
        self.shown_look(progress, slot).filter(|&l| self.look_zone(l) == Some(zone)).unwrap_or(self.own_look(zone))
    }

    /// Whether the piece is in the collection.
    pub fn owned(&self, progress: &Progress, slot: u32, zone: u32) -> bool {
        progress.collection.contains(&self.entry(slot, zone))
    }

    /// Whether look `look` of the slot is in the collection: the slot's piece of the look's zone
    /// (a piece unlocks every row of its zone's looks).
    pub fn owns_look(&self, progress: &Progress, slot: u32, look: u32) -> bool {
        self.look_zone(look).is_some_and(|z| self.owned(progress, slot, z))
    }

    /// How many of the class's pieces are collected, of how many (a piece per slot and zone).
    pub fn collected(&self, progress: &Progress) -> (u32, u32) {
        let all = self.slots() * self.zones();
        let have = (0..self.slots())
            .flat_map(|s| (0..self.zones()).map(move |z| (s, z)))
            .filter(|&(s, z)| self.owned(progress, s, z))
            .count() as u32;
        (have, all)
    }

    /// What all worn gear adds, stat by stat, in the order the stats first come.
    pub fn worn_totals(&self, progress: &Progress) -> Vec<(Stat, f32)> {
        let mut totals: Vec<(Stat, f32)> = Vec::new();
        for (slot, zone) in self.worn(progress).into_iter().enumerate() {
            let Some(zone) = zone else { continue };
            for m in self.bonuses(slot as u32, zone) {
                match totals.iter_mut().find(|(s, _)| *s == m.stat) {
                    Some((_, v)) => *v += m.value,
                    None => totals.push((m.stat, m.value)),
                }
            }
        }
        totals
    }

    /// Every boss of zone `zone` and the class's pieces it can drop, in the run's order.
    pub fn drop_table(&self, zone: usize) -> Vec<BossDrops> {
        let Some(z) = self.content.zones.get(zone) else {
            return Vec::new();
        };
        let finals = self.finals();
        z.bosses
            .iter()
            .enumerate()
            .map(|(b, boss)| BossDrops {
                boss: boss.name.clone(),
                minute: boss.spawn_minute,
                is_final: boss.is_final,
                pieces: boss_drop_slots(&finals, zone, b, self.set.weapons.len() as u32)
                    .into_iter()
                    .map(|slot| (slot, zone as u32))
                    .collect(),
            })
            .collect()
    }

    /// The bosses (zone, boss name) that can drop a piece: bosses of its own zone.
    pub fn sources(&self, slot: u32, zone: u32) -> Vec<(usize, String)> {
        self.drop_table(zone as usize)
            .into_iter()
            .filter(|b| b.pieces.contains(&(slot, zone)))
            .map(|b| (zone as usize, b.boss))
            .collect()
    }

    /// How many pieces the zones open to the class can still give (each counted once).
    pub fn still_to_find(&self, progress: &Progress) -> u32 {
        let open = (progress.zones_open.max(1) as usize).min(self.content.zones.len());
        let mut missing: Vec<(u32, u32)> = (0..open)
            .flat_map(|z| self.drop_table(z))
            .flat_map(|b| b.pieces)
            .filter(|&(s, z)| !self.owned(progress, s, z))
            .collect();
        missing.sort_unstable();
        missing.dedup();
        missing.len() as u32
    }

    fn finals(&self) -> Vec<Vec<bool>> {
        self.content
            .zones
            .iter()
            .map(|z| z.bosses.iter().map(|b| b.is_final).collect())
            .collect()
    }
}

/// A boss and what it can drop for the class.
#[derive(Clone, Debug, PartialEq)]
pub struct BossDrops {
    pub boss: String,
    /// The run minute it arrives.
    pub minute: f32,
    pub is_final: bool,
    /// (slot, zone) of each piece.
    pub pieces: Vec<(u32, u32)>,
}

/// The chance each piece from a zone of difficulty `difficulty` drops with run options worth `bonus`.
pub fn drop_chance(scaling: &ZoneScaling, difficulty: f32, bonus: f32) -> f32 {
    zone_drop_chance(scaling, difficulty) * bonus
}

/// A tier's quality colour index: its tier, common to legendary.
pub fn tier_quality(tier: u32) -> u8 {
    tier.min(4) as u8
}

/// The line that says which zone a piece came from: "From `duskwood`".
pub fn origin_line(g: &ClassGear, p: Option<&Progress>, zone: u32) -> String {
    format!("From {}", g.zone_name(p, zone))
}

/// A piece's tooltip: its name in its quality colour, its slot, the zone it came from, what it
/// adds, and how it stands (worn, in the collection, not won yet).
pub fn piece_tip(g: &ClassGear, p: &Progress, slot: u32, zone: u32) -> Vec<TipLine> {
    let mut lines = vec![
        TipLine::new(g.name(slot, zone), rarity_colour(g.quality(zone))),
        TipLine::new(g.slot_name(slot), WHITE_TEXT),
        TipLine::new(origin_line(g, Some(p), zone), WHITE_TEXT),
    ];
    for m in g.bonuses(slot, zone) {
        lines.push(TipLine::new(stat_line(m.stat, m.value), GREEN_TEXT));
    }
    let worn = g.worn(p).get(slot as usize).copied().flatten() == Some(zone);
    let status = if worn {
        "Worn"
    } else if g.owned(p, slot, zone) {
        "In the collection"
    } else {
        "Not won yet"
    };
    lines.push(TipLine::new(status, GRAY_TEXT));
    lines
}

/// A look's tooltip: its name in its zone's quality colour, its slot, the zone whose pieces show
/// it ("???" for one not discovered; "the second look" for a zone's second row, after its
/// row name where it has one: "Scholomance sets: the second look of pieces from Stratholme"), a
/// weapon slot's look the three loadouts' weapons in it, and whether its
/// zone's piece is in the collection.
pub fn look_tip(g: &ClassGear, p: &Progress, slot: u32, look: u32) -> Vec<TipLine> {
    let zone = g.look_zone(look).map_or("", |z| g.zone_name(Some(p), z));
    let (which, which_first) = match g.look_row(look) {
        0 => ("the look", "The look"),
        1 => ("the second look", "The second look"),
        _ => ("another look", "Another look"),
    };
    let place = match g.look_row_name(Some(p), look) {
        Some(name) => format!("{name}: {which} of pieces from {zone}"),
        None => format!("{which_first} of pieces from {zone}"),
    };
    let status = if g.owns_look(p, slot, look) {
        "In the collection"
    } else {
        "Not won yet"
    };
    let mut lines = vec![
        TipLine::new(g.look_name(slot, look), rarity_colour(g.look_quality(look))),
        TipLine::new(g.slot_name(slot), WHITE_TEXT),
        TipLine::new(place, WHITE_TEXT),
    ];
    if slot >= ARMOR_SLOTS {
        lines.extend(g.armaments(look).into_iter().map(|w| TipLine::new(w, GREEN_TEXT)));
    }
    lines.push(TipLine::new(status, GRAY_TEXT));
    lines
}

/// What a stat's bonus reads as: `+12 health`, `+1.4% critical strike chance`.
pub fn stat_line(stat: Stat, value: f32) -> String {
    let (label, flat) = stat_words(stat);
    let amount = if flat { number(value) } else { percent(value) };
    format!("+{amount} {label}")
}

/// A stat's name in our words, and whether it is a flat number (else a percentage).
pub fn stat_words(stat: Stat) -> (&'static str, bool) {
    match stat {
        Stat::Damage => ("damage", false),
        Stat::CooldownRecovery => ("cooldown recovery", false),
        Stat::SpellCooldownRecovery => ("spell cooldown recovery", false),
        Stat::SwingCooldownRecovery => ("weapon swing speed", false),
        Stat::WeaponDoubleHit => ("chance for a weapon attack to hit twice", false),
        Stat::MaxHealth => ("maximum health", false),
        Stat::CritChance => ("critical strike chance", false),
        Stat::CritDamage => ("critical strike damage", false),
        Stat::DamageReduction => ("damage reduction", false),
        Stat::HealthPerSecond => ("health per second", true),
        Stat::MoveSpeed => ("movement speed", false),
        Stat::Area => ("spell area", false),
        Stat::PickupRange => ("pickup range", false),
        Stat::Experience => ("experience", false),
        Stat::Gold => ("gold", false),
        Stat::Luck => ("luck", false),
        Stat::DoubleCast => ("chance to cast twice", false),
        Stat::Lifesteal => ("of damage dealt as healing", false),
        Stat::HealPerKill => ("health per kill", true),
        Stat::WeaponDamage => ("weapon damage", false),
        Stat::PetDamage => ("pet damage", false),
        Stat::DamageVsCrowdControlled => ("damage to held enemies", false),
        Stat::DamageVsElites => ("damage to elites and bosses", false),
        Stat::DamageBelow30Pct => ("damage to enemies below 30% health", false),
        Stat::DamageVsAfflicted => ("damage to afflicted enemies", false),
        Stat::Thorns => ("damage struck back", false),
        Stat::BuffPower => ("buff strength", false),
        Stat::ExtraLives => ("extra lives", true),
        Stat::DotStacks => ("stacks of effects over time", true),
        Stat::FlatMaxHealth => ("health", true),
        Stat::ExtraTargets => ("targets", true),
        Stat::HealAndShieldPower => ("heal and shield power", false),
        Stat::BlockChance => ("block chance", false),
        Stat::RageWhenHurt => ("rage from damage taken", false),
        Stat::RagePerSwing => ("rage per swing", false),
        Stat::DamageFromReduction => ("damage from damage reduction", false),
        Stat::SwingSpeed => ("weapon swing speed", false),
    }
}

/// Run minutes as `M:SS`.
pub fn minute_text(minute: f32) -> String {
    let s = (minute.max(0.0) * 60.0).round() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Pieces by slot, then zone, as a map the window walks.
pub fn by_slot(pieces: &[(u32, u32)]) -> BTreeMap<u32, Vec<u32>> {
    let mut out: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &(s, t) in pieces {
        out.entry(s).or_default().push(t);
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::OnceLock;

    use super::*;

    pub(crate) fn content() -> &'static Content {
        static CONTENT: OnceLock<Content> = OnceLock::new();
        CONTENT.get_or_init(|| {
            Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
        })
    }

    fn progress(worn: Vec<u32>, looks: Vec<(u32, u32)>, collection: Vec<u32>) -> Progress {
        Progress {
            class: "warrior".into(),
            talent_experience: 0,
            points: 0,
            spent: 0,
            into_next: 0,
            next_cost: 80,
            zones_open: 1,
            zones_discovered: Vec::new(),
            talents: vec![],
            worn,
            looks,
            collection,
            prestige: Default::default(),
            loadout: String::new(),
            new_gear: false,
            new_points: false,
            challenge_open: false,
            challenge_news: false,
        }
    }

    #[test]
    fn a_class_has_its_armor_and_weapons_named_by_slot() {
        let g = ClassGear::of(content(), "warrior").unwrap();
        assert_eq!(g.slots(), 9, "eight armor slots and one weapon");
        assert_eq!(g.slot_name(0), "Head");
        assert_eq!(g.slot_name(8), "Main Hand");
        // every zone its own names
        assert_eq!(g.name(0, 1), "Sparkmetal Coif");
        assert_eq!(g.name(0, 2), "Brutal Helm");
        assert_eq!(g.name(8, 1), "Area 10 Armaments");
        assert_eq!(g.entry(0, 1), 2_100_010);
        assert_eq!(g.piece_of(2_100_010), Some((0, 1)));
        assert_eq!(g.piece_of(2_200_010), None, "a paladin's piece");
        assert_eq!(g.piece_of(g.entry(0, 10)), None, "no eleventh zone");
        assert!(g.display(0, 0) != 0, "Elwynn borrows a helm look");
        assert!(g.display(8, g.own_look(1)) != 0, "a weapon shows its zone's armaments");
        // a weapon look's armaments: the three loadouts' weapons
        assert_eq!(g.armaments(g.own_look(1)), ["Duskbringer", "Darkwater Talwar and Buzz Saw", "Butcher's Slicer and Commander's Crest"]);
        let rogue = ClassGear::of(content(), "rogue").unwrap();
        assert_eq!(rogue.slots(), 10);
        assert_eq!(rogue.slot_name(9), "Off Hand");
        assert!(rogue.empty_slot_art(9).ends_with("SecondaryHand"));
        let hunter = ClassGear::of(content(), "hunter").unwrap();
        assert_eq!(hunter.slot_name(8), "Ranged");
        // a row per zone, Blackrock's and Stratholme's two
        let rows = g.rows();
        assert_eq!(rows.len(), content().zones.len() + 2);
        let brm: Vec<u32> = rows.iter().filter(|(z, _)| content().zones[*z as usize].id == "brm").map(|r| r.1).collect();
        assert_eq!(brm.iter().map(|&l| g.look_row(l)).collect::<Vec<_>>(), [0, 1]);
        assert_eq!(brm.iter().map(|&l| g.look_name(0, l)).collect::<Vec<_>>(), ["Helm of Might", "Helm of Wrath"]);
    }

    #[test]
    fn a_piece_tip_names_it_says_where_it_came_from_and_how_it_stands() {
        let g = ClassGear::of(content(), "warrior").unwrap();
        let mut p = progress(vec![g.entry(0, 1)], vec![], vec![g.entry(0, 1), g.entry(0, 0)]);
        // nothing past the first zone discovered: "???"
        assert_eq!(piece_tip(&g, &p, 0, 1)[2].text, "From ???");
        assert_eq!(look_tip(&g, &p, 0, g.own_look(1))[2].text, "The look of pieces from ???");
        p.zones_discovered = content().zones.iter().map(|z| z.id.clone()).collect();
        let tip = piece_tip(&g, &p, 0, 1);
        assert_eq!(tip[0].text, "Sparkmetal Coif");
        assert_eq!(tip[1].text, "Head");
        assert_eq!(tip[2].text, "From Area 10");
        assert_eq!(tip.last().unwrap().text, "Worn");
        assert_eq!(piece_tip(&g, &p, 0, 2)[2].text, "From Area 3277");
        assert_eq!(
            piece_tip(&g, &p, 0, 0).last().unwrap().text,
            "In the collection"
        );
        assert_eq!(piece_tip(&g, &p, 0, 8).last().unwrap().text, "Not won yet");
        let look = look_tip(&g, &p, 0, g.own_look(1));
        assert_eq!(look[2].text, "The look of pieces from Area 10");
        assert_eq!(look.last().unwrap().text, "In the collection");
        assert_eq!(look_tip(&g, &p, 0, g.own_look(2)).last().unwrap().text, "Not won yet");
        let wrath = g.set.look_of("brm", 1).unwrap() as u32;
        assert!(look_tip(&g, &p, 0, wrath)[2].text.starts_with("The second look of pieces from"));
        assert_eq!(g.look_row_name(Some(&p), wrath), None, "a row without a name keeps the text");
        // Stratholme's second row goes by its name
        let strat = content().zones.iter().position(|z| z.id == "strat").unwrap() as u32;
        let scholomance = g.set.look_of("strat", 1).unwrap() as u32;
        assert_eq!(g.look_row_name(Some(&p), scholomance), Some("Scholomance sets"));
        assert_eq!(
            look_tip(&g, &p, 0, scholomance)[2].text,
            format!("Scholomance sets: the second look of pieces from {}", g.zone_name(Some(&p), strat))
        );
        // not discovered yet: neither the zone nor the row is named
        p.zones_discovered.retain(|z| z != "strat");
        assert_eq!(g.look_row_name(Some(&p), scholomance), None);
        assert_eq!(look_tip(&g, &p, 0, scholomance)[2].text, "The second look of pieces from ???");
    }

    #[test]
    fn worn_gear_and_its_look_per_slot_and_what_it_adds() {
        let g = ClassGear::of(content(), "warrior").unwrap();
        // head from Elwynn shown in Gurubashi's look, chest from the Sunken Temple
        let arena = g.own_look(6);
        let p = progress(
            vec![g.entry(0, 0), g.entry(2, 4), 99],
            vec![(0, arena)],
            vec![g.entry(0, 0), g.entry(0, 6), g.entry(2, 4)],
        );
        let worn = g.worn(&p);
        assert_eq!((worn[0], worn[2], worn[1]), (Some(0), Some(4), None));
        assert_eq!(g.shown_look(&p, 0), Some(arena), "a look chosen");
        assert_eq!(g.shown_look(&p, 2), Some(g.own_look(4)), "the worn piece's own look");
        assert_eq!(g.shown_look(&p, 1), None);
        assert!(g.owns_look(&p, 0, arena) && !g.owns_look(&p, 0, g.own_look(1)));
        assert_eq!(g.collected(&p), (3, 9 * 10), "a piece per slot and zone");
        let totals = g.worn_totals(&p);
        let health: f32 = totals
            .iter()
            .filter(|(s, _)| *s == Stat::FlatMaxHealth)
            .map(|(_, v)| v)
            .sum();
        let want: f32 = g
            .bonuses(0, 0)
            .iter()
            .chain(&g.bonuses(2, 4))
            .filter(|m| m.stat == Stat::FlatMaxHealth)
            .map(|m| m.value)
            .sum();
        assert_eq!(health, want);
        assert_eq!(
            totals
                .iter()
                .filter(|(s, _)| *s == Stat::FlatMaxHealth)
                .count(),
            1,
            "summed"
        );
    }

    /// A Blackrock piece unlocks the slot's look in both its rows.
    #[test]
    fn a_blackrock_piece_owns_both_rows_of_its_look() {
        let g = ClassGear::of(content(), "warrior").unwrap();
        let brm = content().zones.iter().position(|z| z.id == "brm").unwrap() as u32;
        let rows: Vec<u32> = g.rows().into_iter().filter(|r| r.0 == brm).map(|r| r.1).collect();
        let p = progress(vec![g.entry(5, brm)], vec![], vec![g.entry(5, brm)]);
        assert!(rows.iter().all(|&l| g.owns_look(&p, 5, l)));
        assert!(rows.iter().all(|&l| !g.owns_look(&p, 4, l)), "the feet only");
        assert_eq!(g.shown_look(&p, 5), Some(rows[0]), "its own look is the first row");
        // shown in its second row, the worn piece goes by that row's look and name
        let p = progress(vec![g.entry(5, brm)], vec![(5, rows[1])], vec![g.entry(5, brm)]);
        assert_eq!(g.own_zone_look(&p, 5, brm), rows[1]);
        assert_eq!(g.look_name(5, g.own_zone_look(&p, 5, brm)), "Sabatons of Wrath");
    }

    #[test]
    fn the_drop_tables_follow_the_run_rules() {
        let g = ClassGear::of(content(), "warrior").unwrap();
        let elwynn = g.drop_table(0);
        assert_eq!(elwynn.len(), content().zones[0].bosses.len());
        let armor: Vec<(u32, u32)> = elwynn
            .iter()
            .filter(|b| !b.is_final)
            .flat_map(|b| b.pieces.clone())
            .collect();
        assert_eq!(
            armor.len(),
            8,
            "the four bosses share the eight armor slots"
        );
        assert!(armor.iter().all(|&(_, z)| z == 0));
        let last = elwynn.iter().find(|b| b.is_final).unwrap();
        assert_eq!(last.pieces, [(8, 0)], "the final boss drops the weapon");
        assert_eq!(g.sources(8, 0).len(), 1);
        assert_eq!(g.sources(0, 1).len(), 1, "a boss of Area 10 drops its head");
        assert!(g.drop_table(1).iter().flat_map(|b| &b.pieces).all(|&(_, z)| z == 1), "Area 10 drops its own");
        let p = progress(vec![], vec![], vec![g.entry(0, 0)]);
        assert_eq!(g.still_to_find(&p), 8, "Elwynn's nine pieces, one owned");
        let steps = ZoneScaling { gear_drop_chance: vec![(1.0, 0.6), (4.0, 0.3)], ..ZoneScaling::default() };
        assert!((drop_chance(&steps, 1.0, 1.0) - 0.6).abs() < 1e-6);
        assert!((drop_chance(&steps, 4.0, 1.5) - 0.45).abs() < 1e-6);
    }

    #[test]
    fn stats_read_in_words() {
        assert_eq!(stat_line(Stat::FlatMaxHealth, 8.0), "+8 health");
        assert_eq!(
            stat_line(Stat::CritChance, 0.007),
            "+0.7% critical strike chance"
        );
        assert_eq!(
            stat_line(Stat::HealthPerSecond, 0.28),
            "+0.3 health per second"
        );
        assert_eq!(
            stat_line(Stat::HealAndShieldPower, 0.02),
            "+2% heal and shield power"
        );
        assert_eq!(stat_line(Stat::Luck, 0.1), "+10% luck");
        assert_eq!(minute_text(3.0), "3:00");
        assert_eq!(minute_text(15.5), "15:30");
        assert_eq!(by_slot(&[(1, 2), (0, 1), (1, 3)])[&1], [2, 3]);
    }
}
