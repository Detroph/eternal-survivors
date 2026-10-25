//! The save on disk: a RON file of our own format: the heroes with their records; what each class
//! has earned (talent experience and ranks, zones won, the class gear won, worn and its look);
//! the codex, shared by the whole save; and the hero and zone played last. Class gear is kept by
//! slot and zone id, never by item entry: entries are worked out for a session from the content
//! (`rewards::gear_item_entry`), so a zone added or moved changes nothing in a save.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use esurv_content::Content;
use esurv_sim::game::challenge::{slider_on_its_steps, Challenge};
use esurv_sim::game::enemy::RunOptions;
use esurv_sim::game::gear::{decode_item_entry, slot_id, slot_of_id, zone_look};
use esurv_sim::game::rewards::{gear_item_entry, open_zone_count, talent_points};
use esurv_sim::game::{prestige, talents};

use crate::packets::code;

/// The save's export and import, through a fixed folder beside it.
pub mod files;

/// The most heroes one save holds, and the most of either faction (a
/// camp of the hero picker holds nine, and its list shows only its own camp's heroes).
pub use esurv_protocol::{MAX_HEROES, MAX_HEROES_PER_FACTION};
/// Name length limits of the stock create screen.
pub const NAME_LENGTH: (usize, usize) = (2, 12);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedHero {
    /// Its player guid on the wire.
    pub guid: u32,
    pub name: String,
    /// Ids from classes.ron and races.ron.
    pub class: String,
    pub race: String,
    /// 0 male, 1 female.
    pub gender: u8,
    /// skin, face, hair style, hair colour, facial hair.
    pub look: [u8; 5],
    /// Its best and its totals across runs.
    #[serde(default)]
    pub records: HeroRecords,
    /// The zone (its id in the content) its next run is in: chosen on the character screen
    /// before a run, and the zone it entered last (the zone is picked
    /// with the hero). `None` until either: the first zone.
    #[serde(default)]
    pub zone: Option<String>,
    /// The starting option (its id in the content) its next runs start with, picked on the warband
    /// screen. `None` until picked, or for an option the content no
    /// longer has: its class's first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
}

/// The run options the next run starts with, in whole percentages, and its mode: one set for the
/// whole save, kept from the last pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedRunOptions {
    pub density: u16,
    pub health: u16,
    pub damage: u16,
    pub speed: u16,
    /// The Challenge Mode tab was picked last: a class that has cleared Ahn'Qiraj runs
    /// Challenge Mode, any other Normal Mode. An older save: Normal Mode.
    pub challenge: bool,
    /// The challenge slider as last set, 100 (1x) to 500 (5x) in steps of 25.
    pub challenge_pct: u16,
}

impl Default for SavedRunOptions {
    fn default() -> Self {
        Self::from(esurv_protocol::RunOptions::default())
    }
}

impl From<esurv_protocol::RunOptions> for SavedRunOptions {
    fn from(o: esurv_protocol::RunOptions) -> Self {
        Self {
            density: o.density,
            health: o.health,
            damage: o.damage,
            speed: o.speed,
            challenge: o.challenge,
            challenge_pct: o.challenge_pct,
        }
    }
}

impl From<SavedRunOptions> for esurv_protocol::RunOptions {
    fn from(o: SavedRunOptions) -> Self {
        Self {
            density: o.density,
            health: o.health,
            damage: o.damage,
            speed: o.speed,
            challenge: o.challenge,
            challenge_pct: o.challenge_pct,
        }
    }
}

impl SavedRunOptions {
    /// The options a run starts with, each on its slider (`run_options.ron`), in Normal Mode.
    pub fn run_options(&self, sliders: &[esurv_content::RunOption]) -> RunOptions {
        RunOptions {
            density_pct: self.density,
            health_pct: self.health,
            damage_pct: self.damage,
            speed_pct: self.speed,
            challenge: None,
        }
        .on_the_sliders(sliders)
    }

    /// The options a run of a class starts with: [`SavedRunOptions::run_options`], in Challenge Mode
    /// when its tab was picked and the class has it open (`challenge_open`).
    pub fn run_options_for(&self, content: &Content, challenge_open: bool) -> RunOptions {
        let mut o = self.run_options(&content.run_options);
        if self.challenge && challenge_open {
            o.challenge = Challenge::new(content, self.challenge_pct);
        }
        o
    }

    /// The options kept on their sliders, the challenge slider on its steps.
    pub fn on_the_sliders(self, content: &Content) -> Self {
        let o = self.run_options(&content.run_options);
        Self {
            density: o.density_pct,
            health: o.health_pct,
            damage: o.damage_pct,
            speed: o.speed_pct,
            challenge: self.challenge,
            challenge_pct: slider_on_its_steps(self.challenge_pct),
        }
    }
}

/// A hero's records across runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroRecords {
    pub best_level: u32,
    /// The longest run survived, in seconds.
    pub best_seconds: u32,
    pub runs: u32,
    pub wins: u32,
}

/// What a class has earned; every hero of the class shares it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassProgress {
    /// All the talent experience earned; talent points are bought from it.
    pub talent_experience: u32,
    /// The talent ranks bought, by talent id.
    pub talents: BTreeMap<String, u32>,
    /// The prestige ranks bought, by prestige stat id.
    pub prestige: BTreeMap<String, u32>,
    /// The zones the class has won, by id. Which zones are open follows from them
    /// ([`ClassProgress::open_zones`]), so a zone added to the content later takes its place.
    pub zones_won: BTreeSet<String>,
    /// How many zones were open, in a save from before the zones won were kept; read once and
    /// moved on to `zones_won` (`SaveFile::migrate`).
    #[serde(rename = "zones_open", skip_serializing)]
    pub(crate) old_zones_open: u32,
    /// The class gear won (the armory's collection), by slot and zone.
    pub collection: BTreeSet<SavedPiece>,
    /// The class gear worn, one piece a slot.
    #[serde(rename = "worn")]
    pub worn_pieces: Vec<SavedPiece>,
    /// The look shown in a worn slot (slot id → a zone's row of looks), where it is not the piece's
    /// own (every zone its own look).
    pub looks: BTreeMap<String, SavedLook>,
    /// The looks a save from before every zone had its own kept, by tier (slot id → tier 0-4);
    /// read once and moved on to `looks` (`SaveFile::migrate`).
    #[serde(rename = "appearance", skip_serializing)]
    pub(crate) old_looks: BTreeMap<String, u32>,
    /// The weapon loadout chosen (its id in `loadouts.ron`); none, or one the class no longer has:
    /// the class's first (roadmap R3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loadout: Option<String>,
    /// What the player had seen of the class when it last opened its Collection and its Talents
    /// before a run (the warband screen's buttons glow on what is new).
    /// A class from a save from before it was kept is taken as having seen everything
    /// (`SaveFile::migrate`); a class new to the save has seen nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seen: Option<Seen>,
    /// What it wears as this session numbers it, by item entry ([`Saves::class_progress`]).
    #[serde(skip)]
    pub worn: Vec<u32>,
    /// The looks chosen, by slot number ([`Saves::class_progress`]).
    #[serde(skip)]
    pub appearance: BTreeMap<u32, u32>,
}

/// A look as a save keeps it: the zone it is of, by id, and its row among the zone's looks (0
/// first; Blackrock has two).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SavedLook {
    pub zone: String,
    #[serde(default, skip_serializing_if = "is_first_row")]
    pub row: u32,
}

fn is_first_row(row: &u32) -> bool {
    *row == 0
}

/// A class gear piece as a save keeps it: its slot (`gear::slot_id`: `head` ... `hands`, then
/// `main_hand`, `off_hand`, `main_hand_2` ...) and the zone it is from, by id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SavedPiece {
    pub slot: String,
    pub zone: String,
}

impl SavedPiece {
    /// Its item entry for class `wow_class_id` in `content`'s zone order; `None` for a slot or a
    /// zone the content does not have.
    pub fn entry(&self, content: &Content, wow_class_id: u32) -> Option<u32> {
        let slot = slot_of_id(&self.slot)?;
        let zone = content.zones.iter().position(|z| z.id == self.zone)?;
        Some(gear_item_entry(wow_class_id, slot, zone as u32))
    }

    /// The piece item entry `entry` names in `content`'s zone order, and the id of its class.
    pub fn of_entry(content: &Content, entry: u32) -> Option<(String, SavedPiece)> {
        let (wow_class, slot, zone) = decode_item_entry(entry)?;
        let class = content.classes.iter().find(|c| c.wow_class_id == wow_class)?;
        let piece = SavedPiece { slot: slot_id(slot)?.into(), zone: content.zones.get(zone as usize)?.id.clone() };
        Some((class.id.clone(), piece))
    }
}

impl ClassProgress {
    /// How many zones are open to the class, from the first in the zones' difficulty order: every
    /// zone up to the hardest won, and the next (`rewards::open_zone_count`). A zone won that the
    /// content no longer has counts for nothing.
    pub fn open_zones(&self, content: &Content) -> usize {
        let won = self
            .zones_won
            .iter()
            .filter_map(|id| content.zones.iter().position(|z| z.id == *id));
        open_zone_count(won, content.zones.len())
    }

    /// Whether the class has Challenge Mode open: it has won Ahn'Qiraj, kept with the
    /// other zones it has won (`challenge::is_open`).
    pub fn challenge_open(&self, content: &Content) -> bool {
        esurv_sim::game::challenge::is_open(content, self.zones_won.iter().map(String::as_str))
    }

    /// A class new to the save: nothing earned, nothing seen yet (so what it earns is news).
    pub fn new_to_the_save() -> Self {
        Self {
            seen: Some(Seen::default()),
            ..Self::default()
        }
    }

    /// Every talent and prestige point its experience has earned.
    pub fn points_earned(&self) -> u32 {
        talent_points(self.talent_experience).0 + prestige::points_earned(self.talent_experience)
    }

    /// What the class holds now, as a view of it would show it.
    fn holding(&self) -> Seen {
        Seen {
            pieces: self.collection.len() as u32,
            points: self.points_earned(),
        }
    }

    /// Whether its Collection and its Talents have news since the player last opened them: gear
    /// won, points earned.
    pub fn unseen(&self) -> (bool, bool) {
        let seen = self.seen.unwrap_or_default();
        let now = self.holding();
        (now.pieces > seen.pieces, now.points > seen.points)
    }
}

/// What the player had seen of a class: its gear pieces won when it
/// last opened the Collection, and its points earned when it last opened the Talents.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Seen {
    pub pieces: u32,
    pub points: u32,
}

/// The hero played last and where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastPlayed {
    pub hero: u32,
    /// The zone's id in the content.
    pub zone: String,
}

/// A run as it stands for one hero, to pay out. A run is paid when it ends and again when it is
/// left (an Endless run pays its extra): each payment brings the experience earned since the last.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FinishedRun {
    pub level: u32,
    pub seconds: f32,
    pub won: bool,
    /// The talent experience earned since the last payment.
    pub experience: u32,
    /// The run's first payment: it counts as a run played (and won, as the run stood then); later
    /// payments of the same run do not count it again.
    pub first: bool,
    /// A won run that has not opened a zone yet tries to (once per run).
    pub opens_zone: bool,
    /// The zone's index in the zones' difficulty order.
    pub zone_index: usize,
    /// The class gear won, by item entry (it joined the collection when it dropped).
    pub loot: Vec<u32>,
    /// A Challenge Mode run: it pays its talent experience and its gear, and nothing
    /// else: no records (best level and time, runs, wins) and no zone won or opened.
    pub challenge: bool,
}

/// What a payment paid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Payout {
    pub experience: u32,
    /// The class's talent experience before and after.
    pub total_before: u32,
    pub total_after: u32,
    /// Talent points the class had earned from its experience before and after.
    pub points_before: u32,
    pub points_after: u32,
    /// The zone (index) the run opened to the class.
    pub zone_opened: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SaveFile {
    /// Which class gear numbering the save uses: [`GEAR_BY_NAME`] keeps pieces by slot and zone
    /// id; an older save kept item entries and is moved on when it is read ([`SaveFile::parse`]).
    #[serde(default)]
    gear_numbering: u32,
    heroes: Vec<SavedHero>,
    /// The guid the next new hero gets.
    next_guid: u32,
    /// What each class has earned, by class id.
    #[serde(default)]
    classes: BTreeMap<String, ClassProgress>,
    /// The codex: every ring, trinket and necklace collected in play, by id.
    #[serde(default)]
    codex: BTreeSet<String>,
    #[serde(default)]
    last_played: Option<LastPlayed>,
    /// The run options the next run starts with.
    #[serde(default)]
    run_options: SavedRunOptions,
    /// The client's tips (its tutorial bits), one bit per tip, set when seen: every tip seen in a
    /// new save, so the tips are off until the player turns them on.
    #[serde(default = "every_tip_seen")]
    tips_seen: [u32; TIP_WORDS],
    /// The zones discovered, by id: every zone any class of the save
    /// has opened, for good. Only what the zone lists show hangs on it; each class still opens a
    /// zone itself to play it. A save from before it is filled from what its classes have open
    /// the first time it is read with the content ([`Saves::zones_discovered`]).
    #[serde(default)]
    zones_discovered: BTreeSet<String>,
    /// The box telling that Challenge Mode is open was shown and closed: it shows once per
    /// save, the first time any class has cleared Ahn'Qiraj.
    #[serde(default)]
    challenge_news_seen: bool,
}

impl SaveFile {
    /// Marks every zone open to any class (and the first zone, always open) discovered; whether
    /// any was new.
    fn discover_open_zones(&mut self, content: &Content) -> bool {
        let mut open = content.zones.len().min(1);
        for p in self.classes.values() {
            open = open.max(p.open_zones(content));
        }
        let before = self.zones_discovered.len();
        for zone in &content.zones[..open] {
            self.zones_discovered.insert(zone.id.clone());
        }
        self.zones_discovered.len() > before
    }
}

/// How many words of tip bits the client keeps.
pub const TIP_WORDS: usize = 8;

/// Every tip marked seen: the tips off.
fn every_tip_seen() -> [u32; TIP_WORDS] {
    [u32::MAX; TIP_WORDS]
}

/// The class gear numbering where a save keeps a piece by its slot and zone id ([`SavedPiece`]).
const GEAR_BY_NAME: u32 = 2;

impl Default for SaveFile {
    fn default() -> Self {
        Self {
            gear_numbering: GEAR_BY_NAME,
            heroes: Vec::new(),
            next_guid: 0,
            classes: BTreeMap::new(),
            codex: BTreeSet::new(),
            last_played: None,
            run_options: SavedRunOptions::default(),
            tips_seen: every_tip_seen(),
            zones_discovered: BTreeSet::new(),
            challenge_news_seen: false,
        }
    }
}

impl SaveFile {
    /// Reads a save from its text, moving an older one on to today's form
    /// (`older::OlderSave`).
    fn parse(text: &str) -> Result<Self, ron::error::SpannedError> {
        /// Only the numbering, to tell which form the rest is in.
        #[derive(Deserialize)]
        struct Numbering {
            #[serde(default)]
            gear_numbering: u32,
        }
        let numbering: Numbering = ron::from_str(text)?;
        let mut file = if numbering.gear_numbering >= GEAR_BY_NAME {
            ron::from_str(text)?
        } else {
            ron::from_str::<older::OlderSave>(text)?.moved_on()
        };
        file.migrate();
        Ok(file)
    }

    /// Moves an older save on: a count of open zones becomes the zone it says was won last, and
    /// a zone that was reworked under a new id (`RENAMED_ZONES`) is named by it.
    fn migrate(&mut self) {
        let renamed = |id: &mut String| {
            if let Some((_, new)) = RENAMED_ZONES.iter().find(|(old, _)| old == id) {
                *id = (*new).into();
            }
        };
        for p in self.classes.values_mut() {
            p.zones_won = std::mem::take(&mut p.zones_won)
                .into_iter()
                .map(|mut id| {
                    renamed(&mut id);
                    id
                })
                .collect();
            p.collection = std::mem::take(&mut p.collection)
                .into_iter()
                .map(|mut piece| {
                    renamed(&mut piece.zone);
                    piece
                })
                .collect();
            p.worn_pieces.iter_mut().for_each(|piece| renamed(&mut piece.zone));
            p.looks.values_mut().for_each(|look| renamed(&mut look.zone));
        }
        for p in self.classes.values_mut() {
            // a look chosen by tier becomes the look of the latest zone
            // of that tier whose piece of the slot the class has won; none: the choice goes
            for (slot, tier) in std::mem::take(&mut p.old_looks) {
                let zones = OLD_TIER_ZONES.get(tier as usize).copied().unwrap_or_default();
                let won = zones.iter().rev().find(|&&z| p.collection.iter().any(|c| c.slot == slot && c.zone == z));
                if let Some(&zone) = won {
                    p.looks.entry(slot).or_insert(SavedLook { zone: zone.into(), row: 0 });
                }
            }
        }
        for hero in &mut self.heroes {
            if let Some(zone) = hero.zone.as_mut() {
                renamed(zone);
            }
        }
        if let Some(last) = self.last_played.as_mut() {
            renamed(&mut last.zone);
        }
        self.zones_discovered = std::mem::take(&mut self.zones_discovered)
            .into_iter()
            .map(|mut id| {
                renamed(&mut id);
                id
            })
            .collect();
        for p in self.classes.values_mut() {
            // n zones open: zone n - 1 (the (n - 2)th from 0) was won last; one open is no win
            let won = (p.old_zones_open as usize).checked_sub(2);
            let last = older::ZONES_BEFORE_ZONE_FILES.len() - 1;
            if let Some(id) = won.map(|i| older::ZONES_BEFORE_ZONE_FILES[i.min(last)]) {
                p.zones_won.insert(id.into());
            }
            p.old_zones_open = 0;
        }
        // a class from before what was seen was kept: nothing of it is news
        for p in self.classes.values_mut() {
            if p.seen.is_none() {
                p.seen = Some(p.holding());
            }
        }
    }
}

/// Zones reworked under a new id, (old id, new id): a save naming the
/// old one names the new. Arathi Basin became the Sunken Temple, Alterac Valley Stratholme.
pub const RENAMED_ZONES: [(&str, &str); 2] = [("ab", "sunken_temple"), ("av", "strat")];

/// The zones that shared each gear tier's look before every zone had its own (tier 0 to 4, by
/// the difficulty starts 2, 4, 7 and 9), in difficulty order: a look a save chose by tier is moved
/// on to one of them ([`SaveFile::migrate`]).
const OLD_TIER_ZONES: [&[&str]; 5] = [
    &["elwynn"],
    &["duskwood", "wsg"],
    &["scarlet", "sunken_temple", "feralas"],
    &["arena", "strat"],
    &["brm", "aq"],
];

/// Saves from before pieces were kept by name: they kept item entries, in two numberings.
mod older {
    use super::*;

    /// The zone order every save was written in before each zone came in a file of its own and
    /// the zones were ordered by difficulty: a save without `gear_zones` numbers its pieces by
    /// it, and a count of open zones (`zones_open`) counts in it. Its fourth zone is the
    /// Sunken Temple's place (it was Arathi Basin, `RENAMED_ZONES`), its seventh Stratholme's
    /// (it was Alterac Valley).
    pub(super) const ZONES_BEFORE_ZONE_FILES: [&str; 9] =
        ["elwynn", "duskwood", "wsg", "sunken_temple", "feralas", "arena", "strat", "brm", "aq"];

    fn zones_before_zone_files() -> Vec<String> {
        ZONES_BEFORE_ZONE_FILES.map(String::from).to_vec()
    }

    /// The classes those saves knew, by `ChrClasses.dbc` id (an entry named its class by it).
    const CLASSES: [(u32, &str); 9] = [
        (1, "warrior"),
        (2, "paladin"),
        (3, "hunter"),
        (4, "rogue"),
        (5, "priest"),
        (7, "shaman"),
        (8, "mage"),
        (9, "warlock"),
        (11, "druid"),
    ];

    /// The first numbering, by tier: `100 000 + wow class id × 1000 + slot × 100 + tier × 10`.
    const BY_TIER: u32 = 0;
    /// For the first numbering, an old piece of tier `t` becomes the
    /// piece of the later zone of the tier (tier 0 Elwynn, 1 Warsong, 2 `feralas`, 3 `strat` (Alterac then), 4
    /// `aq`), by its place in [`ZONES_BEFORE_ZONE_FILES`].
    const ZONE_OF_OLD_TIER: [u32; 5] = [0, 2, 4, 6, 8];

    /// A piece of the first numbering as (wow class id, slot, zone place).
    fn piece_by_tier(entry: u32) -> Option<(u32, u32, u32)> {
        let e = entry.checked_sub(100_000)?;
        let tier = (e / 10) % 10;
        if e >= 12_000 || e % 10 != 0 || tier as usize >= ZONE_OF_OLD_TIER.len() {
            return None;
        }
        Some((e / 1000, (e / 100) % 10, ZONE_OF_OLD_TIER[tier as usize]))
    }

    /// A piece of the second numbering, by zone place: `1 000 000 + wow class id × 10 000 + slot ×
    /// 1000 + zone × 10`, as (wow class id, slot, zone place).
    fn piece_by_zone(entry: u32) -> Option<(u32, u32, u32)> {
        let e = entry.checked_sub(1_000_000)?;
        if e >= 120_000 || e % 10 != 0 {
            return None;
        }
        Some((e / 10_000, (e / 1000) % 10, (e / 10) % 100))
    }

    #[derive(Debug, Clone, Default, Deserialize)]
    #[serde(default)]
    struct OlderClass {
        talent_experience: u32,
        talents: BTreeMap<String, u32>,
        zones_won: BTreeSet<String>,
        zones_open: u32,
        worn: Vec<u32>,
        appearance: BTreeMap<u32, u32>,
    }

    /// A save that kept class gear by item entry: `gear_numbering` 0 (by tier) or 1 (by zone).
    #[derive(Debug, Clone, Deserialize)]
    pub(super) struct OlderSave {
        #[serde(default)]
        gear_numbering: u32,
        /// The zone ids in the order its entries counted zones by.
        #[serde(default = "zones_before_zone_files")]
        gear_zones: Vec<String>,
        heroes: Vec<SavedHero>,
        next_guid: u32,
        #[serde(default)]
        classes: BTreeMap<String, OlderClass>,
        #[serde(default)]
        collection: BTreeSet<u32>,
        #[serde(default)]
        codex: BTreeSet<String>,
        #[serde(default)]
        last_played: Option<LastPlayed>,
        #[serde(default)]
        run_options: SavedRunOptions,
    }

    impl OlderSave {
        /// The piece `entry` named, with its class's id; `None` for an entry that is no piece.
        fn piece(&self, entry: u32) -> Option<(&'static str, SavedPiece)> {
            let (class, slot, zone) =
                if self.gear_numbering == BY_TIER { piece_by_tier(entry) } else { piece_by_zone(entry) }?;
            let class = CLASSES.iter().find(|(id, _)| *id == class)?.1;
            let zone = if self.gear_numbering == BY_TIER {
                ZONES_BEFORE_ZONE_FILES.get(zone as usize)?.to_string()
            } else {
                self.gear_zones.get(zone as usize)?.clone()
            };
            Some((class, SavedPiece { slot: slot_id(slot)?.into(), zone }))
        }

        /// The save in today's form: every piece by slot and zone id, the collection kept by
        /// class, a chosen look by slot id. An entry that names no piece is dropped.
        pub(super) fn moved_on(self) -> SaveFile {
            let mut classes: BTreeMap<String, ClassProgress> = BTreeMap::new();
            for (id, old) in &self.classes {
                let worn: Vec<SavedPiece> = old.worn.iter().filter_map(|&e| self.piece(e)).map(|(_, p)| p).collect();
                let looks = old.appearance.iter().filter_map(|(&s, &t)| Some((slot_id(s)?.to_string(), t))).collect();
                let progress = ClassProgress {
                    talent_experience: old.talent_experience,
                    talents: old.talents.clone(),
                    zones_won: old.zones_won.clone(),
                    old_zones_open: old.zones_open,
                    worn_pieces: worn,
                    old_looks: looks,
                    ..Default::default()
                };
                classes.insert(id.clone(), progress);
            }
            for (class, piece) in self.collection.iter().filter_map(|&e| self.piece(e)) {
                classes.entry(class.into()).or_default().collection.insert(piece);
            }
            for p in classes.values_mut() {
                p.worn_pieces.sort();
            }
            SaveFile {
                gear_numbering: GEAR_BY_NAME,
                heroes: self.heroes,
                next_guid: self.next_guid,
                classes,
                codex: self.codex,
                last_played: self.last_played,
                run_options: self.run_options,
                tips_seen: every_tip_seen(),
                zones_discovered: BTreeSet::new(),
                challenge_news_seen: false,
            }
        }
    }
}

/// The save, shared by every connection; written to disk after each change.
pub struct Saves {
    path: Option<PathBuf>,
    file: Mutex<SaveFile>,
    /// How many heroes are in the world now (each entered run holds a [`InWorldMark`]): the save
    /// is never replaced under a run.
    in_world: std::sync::atomic::AtomicUsize,
}

/// A hero's place in the world, held for as long as it is in a run: the save counts it while the
/// mark lives (a dropped connection lets go of it too).
pub struct InWorldMark(Arc<Saves>);

impl Drop for InWorldMark {
    fn drop(&mut self) {
        self.0
            .in_world
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Saves {
    /// Opens the save at `path`, starting empty if there is none yet.
    pub fn open(path: &Path) -> Result<Self> {
        let file = match std::fs::read_to_string(path) {
            Ok(text) => SaveFile::parse(&text)
                .with_context(|| format!("reading the save {}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => SaveFile::default(),
            Err(e) => {
                return Err(e).with_context(|| format!("opening the save {}", path.display()))
            }
        };
        Ok(Self {
            path: Some(path.to_path_buf()),
            file: Mutex::new(file),
            in_world: Default::default(),
        })
    }

    /// A save that lives only in memory.
    pub fn in_memory() -> Self {
        Self {
            path: None,
            file: Mutex::default(),
            in_world: Default::default(),
        }
    }

    /// Counts a hero into the world until the mark it returns is dropped.
    pub fn enter_world(self: &Arc<Self>) -> InWorldMark {
        self.in_world
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        InWorldMark(self.clone())
    }

    /// Whether any hero is in a run now.
    pub fn any_in_world(&self) -> bool {
        self.in_world.load(std::sync::atomic::Ordering::SeqCst) > 0
    }

    pub fn heroes(&self) -> Vec<SavedHero> {
        self.file.lock().unwrap().heroes.clone()
    }

    pub fn hero(&self, guid: u32) -> Option<SavedHero> {
        self.file
            .lock()
            .unwrap()
            .heroes
            .iter()
            .find(|h| h.guid == guid)
            .cloned()
    }

    /// Adds a hero, or says why not (a character-screen result code). The name is checked as
    /// the stock screen does (letters only, 2 to 12) and stored capitalised.
    pub fn create(
        &self,
        name: &str,
        class: &str,
        race: &str,
        gender: u8,
        look: [u8; 5],
    ) -> Result<SavedHero, u8> {
        let name = check_name(name)?;
        let mut file = self.file.lock().unwrap();
        if file
            .heroes
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case(&name))
        {
            return Err(code::CHAR_CREATE_NAME_IN_USE);
        }
        if file.heroes.len() >= MAX_HEROES {
            return Err(code::CHAR_CREATE_SERVER_LIMIT);
        }
        file.next_guid = file.next_guid.max(1);
        let hero = SavedHero {
            guid: file.next_guid,
            name,
            class: class.into(),
            race: race.into(),
            gender,
            look,
            records: HeroRecords::default(),
            zone: None,
            start: None,
        };
        file.next_guid += 1;
        file.heroes.push(hero.clone());
        self.write(&file).map_err(|_| code::CHAR_CREATE_ERROR)?;
        Ok(hero)
    }

    /// Renames hero `guid`, or says why not (a character-screen result code, as for a new hero):
    /// the stock name rules, and no other hero of the save named so. The name as stored.
    pub fn rename(&self, guid: u32, name: &str) -> Result<String, u8> {
        let name = check_name(name)?;
        let mut file = self.file.lock().unwrap();
        if file
            .heroes
            .iter()
            .any(|h| h.guid != guid && h.name.eq_ignore_ascii_case(&name))
        {
            return Err(code::CHAR_CREATE_NAME_IN_USE);
        }
        let hero = file
            .heroes
            .iter_mut()
            .find(|h| h.guid == guid)
            .ok_or(code::CHAR_CREATE_ERROR)?;
        if hero.name != name {
            hero.name = name.clone();
            self.write(&file).map_err(|_| code::CHAR_CREATE_ERROR)?;
        }
        Ok(name)
    }

    /// Gives hero `guid` another race, sex or look (a hero's race and
    /// look may change before any run). The caller checks the race against the class; `false`
    /// without such a hero or when the save could not be written.
    pub fn reshape(&self, guid: u32, race: &str, gender: u8, look: [u8; 5]) -> bool {
        let mut file = self.file.lock().unwrap();
        let Some(hero) = file.heroes.iter_mut().find(|h| h.guid == guid) else {
            return false;
        };
        let changed = hero.race != race || hero.gender != gender || hero.look != look;
        if changed {
            hero.race = race.into();
            hero.gender = gender;
            hero.look = look;
        }
        !changed || self.write(&file).is_ok()
    }

    /// Hero `guid`'s next run goes to `zone` (a zone id); `false` without such a hero. Whether the
    /// class has opened it is the caller's to check.
    pub fn choose_zone(&self, guid: u32, zone: &str) -> bool {
        let mut file = self.file.lock().unwrap();
        let Some(hero) = file.heroes.iter_mut().find(|h| h.guid == guid) else {
            return false;
        };
        if hero.zone.as_deref() != Some(zone) {
            hero.zone = Some(zone.into());
            let _ = self.write(&file);
        }
        true
    }

    /// Sets the starting option hero `guid`'s next runs start with; `false` if there is no such
    /// hero. The caller checks the option is one of its class's.
    pub fn choose_start(&self, guid: u32, option: &str) -> bool {
        let mut file = self.file.lock().unwrap();
        let Some(hero) = file.heroes.iter_mut().find(|h| h.guid == guid) else {
            return false;
        };
        if hero.start.as_deref() != Some(option) {
            hero.start = Some(option.into());
            let _ = self.write(&file);
        }
        true
    }

    /// How many heroes the save holds of `faction` (by race in the content).
    pub fn heroes_of_faction(
        &self,
        content: &Content,
        faction: esurv_content::Faction,
    ) -> usize {
        let file = self.file.lock().unwrap();
        file.heroes
            .iter()
            .filter(|h| content.race(&h.race).is_some_and(|r| r.faction == faction))
            .count()
    }

    /// Removes a hero; `false` if there was none with that guid or the save could not be written.
    pub fn delete(&self, guid: u32) -> bool {
        let mut file = self.file.lock().unwrap();
        let before = file.heroes.len();
        file.heroes.retain(|h| h.guid != guid);
        file.heroes.len() < before && self.write(&file).is_ok()
    }

    /// What class `class` has earned (nothing yet for a class never played), its worn gear and
    /// chosen looks numbered for `content` (`worn`, `appearance`).
    pub fn class_progress(&self, content: &Content, class: &str) -> ClassProgress {
        let mut p = self.saved_class(class);
        if let Some(wow_class) = content.class(class).map(|k| k.wow_class_id) {
            p.worn = p.worn_pieces.iter().filter_map(|w| w.entry(content, wow_class)).collect();
            p.worn.sort_unstable();
        }
        if let Some(set) = content.gear_set(class) {
            p.appearance = p.looks.iter().filter_map(|(s, l)| Some((slot_of_id(s)?, set.look_of(&l.zone, l.row)? as u32))).collect();
        }
        p
    }

    /// What class `class` has earned, as the save keeps it.
    fn saved_class(&self, class: &str) -> ClassProgress {
        let file = self.file.lock().unwrap();
        file.classes.get(class).cloned().unwrap_or_default()
    }

    /// Whether zone `index` (in the zones' difficulty order) is open to `class`: the first
    /// always is.
    pub fn zone_open(&self, content: &Content, class: &str, index: usize) -> bool {
        index < self.saved_class(class).open_zones(content)
    }

    /// Whether `class` has Challenge Mode open: it has cleared Ahn'Qiraj.
    pub fn challenge_open(&self, content: &Content, class: &str) -> bool {
        self.saved_class(class).challenge_open(content)
    }

    /// Whether the box telling that Challenge Mode is open is due: some class of the save has
    /// cleared Ahn'Qiraj and the box was never closed ([`Saves::see`] with `SeenView::Challenge`).
    pub fn challenge_news(&self, content: &Content) -> bool {
        let file = self.file.lock().unwrap();
        !file.challenge_news_seen && file.classes.values().any(|p| p.challenge_open(content))
    }

    /// Whether `class`'s next run is a Challenge Mode run: its tab is picked in the run options and
    /// the class has it open.
    pub fn runs_challenge(&self, content: &Content, class: &str) -> bool {
        self.run_options().challenge && self.challenge_open(content, class)
    }

    /// Whether `class` may run zone `index` next: an open zone, or any zone in Challenge Mode
    /// (every map is open there).
    pub fn zone_open_for_next_run(&self, content: &Content, class: &str, index: usize) -> bool {
        index < content.zones.len() && (self.runs_challenge(content, class) || self.zone_open(content, class, index))
    }

    /// The options `class`'s next run starts with: the saved run options, in Challenge Mode when
    /// [`Saves::runs_challenge`].
    pub fn run_options_for(&self, content: &Content, class: &str) -> RunOptions {
        self.run_options().run_options_for(content, self.challenge_open(content, class))
    }

    /// The zones the save has discovered, by id in `content`'s order: a
    /// zone any class has opened, the first always. A zone open to a class but not yet kept as
    /// discovered (a save from before discovery was kept) is kept now.
    pub fn zones_discovered(&self, content: &Content) -> Vec<String> {
        let mut file = self.file.lock().unwrap();
        if file.discover_open_zones(content) {
            let _ = self.write(&file);
        }
        content
            .zones
            .iter()
            .filter(|z| file.zones_discovered.contains(&z.id))
            .map(|z| z.id.clone())
            .collect()
    }

    /// Hero `guid` entered `zone` (a zone id).
    pub fn played(&self, guid: u32, zone: &str) {
        let mut file = self.file.lock().unwrap();
        if let Some(h) = file.heroes.iter_mut().find(|h| h.guid == guid) {
            h.zone = Some(zone.into());
        }
        file.last_played = Some(LastPlayed {
            hero: guid,
            zone: zone.into(),
        });
        let _ = self.write(&file);
    }

    /// Pays a run out to hero `guid`: its best level and time, the run counted (on its first
    /// payment), the talent experience earned since the last payment for its class and, once on a
    /// win, the zone as won (which opens the next); the gear it won is in the collection. `None`
    /// without such a hero. A Challenge Mode run pays only its talent experience and gear.
    pub fn finish_run(&self, content: &Content, guid: u32, run: &FinishedRun) -> Option<Payout> {
        let mut file = self.file.lock().unwrap();
        let hero = file.heroes.iter_mut().find(|h| h.guid == guid)?;
        let r = &mut hero.records;
        // a challenge run counts for no record
        if !run.challenge {
            r.best_level = r.best_level.max(run.level);
            r.best_seconds = r.best_seconds.max(run.seconds.max(0.0) as u32);
        }
        if run.first && !run.challenge {
            r.runs += 1;
            r.wins += u32::from(run.won);
        }
        let class = hero.class.clone();
        let progress = file.classes.entry(class).or_insert_with(ClassProgress::new_to_the_save);
        let total_before = progress.talent_experience;
        progress.talent_experience = total_before.saturating_add(run.experience);
        let total_after = progress.talent_experience;
        let mut zone_opened = None;
        let won = content.zones.get(run.zone_index).filter(|_| run.won && run.opens_zone && !run.challenge);
        if let Some(zone) = won {
            let before = progress.open_zones(content);
            progress.zones_won.insert(zone.id.clone());
            let after = progress.open_zones(content);
            if after > before {
                zone_opened = Some(after - 1);
            }
            // a zone opened to one class is discovered for the whole save
            file.discover_open_zones(content);
        }
        for (class, piece) in run.loot.iter().filter_map(|&e| SavedPiece::of_entry(content, e)) {
            file.classes.entry(class).or_insert_with(ClassProgress::new_to_the_save).collection.insert(piece);
        }
        let _ = self.write(&file);
        Some(Payout {
            experience: run.experience,
            total_before,
            total_after,
            points_before: talent_points(total_before).0,
            points_after: talent_points(total_after).0,
            zone_opened,
        })
    }

    /// A class gear piece won (item entry `entry`, numbered for `content`): it joins its class's
    /// collection at once. `true` when it is new there.
    pub fn add_to_collection(&self, content: &Content, entry: u32) -> bool {
        let Some((class, piece)) = SavedPiece::of_entry(content, entry) else {
            return false;
        };
        let mut file = self.file.lock().unwrap();
        let new = file.classes.entry(class).or_insert_with(ClassProgress::new_to_the_save).collection.insert(piece);
        if new {
            let _ = self.write(&file);
        }
        new
    }

    /// A ring, trinket or necklace was collected in play: the codex keeps it. `true` when it is
    /// new to the codex.
    pub fn add_to_codex(&self, id: &str) -> bool {
        let mut file = self.file.lock().unwrap();
        let new = file.codex.insert(id.into());
        if new {
            let _ = self.write(&file);
        }
        new
    }

    /// Every ring, trinket and necklace collected so far, by id.
    pub fn codex(&self) -> Vec<String> {
        self.file.lock().unwrap().codex.iter().cloned().collect()
    }

    /// Buys one rank of talent `id` for `class` with the points its talent experience has earned
    /// (the armory's rules: `esurv_sim::game::talents`). `false` when it cannot be bought.
    pub fn learn_talent(&self, content: &Content, class: &str, id: &str) -> bool {
        self.change_class(class, |p| {
            let points = talents::points_earned(p.talent_experience);
            talents::learn(content, class, &mut p.talents, id, points)
        })
    }

    /// Takes back one rank of talent `id` for `class` (free), when every rank left still meets the
    /// rules.
    pub fn unlearn_talent(&self, content: &Content, class: &str, id: &str) -> bool {
        self.change_class(class, |p| {
            talents::unlearn(content, class, &mut p.talents, id)
        })
    }

    /// Buys one prestige rank of stat `id` for `class` with the prestige points its talent
    /// experience has earned (`esurv_sim::game::prestige`).
    pub fn learn_prestige(&self, content: &Content, class: &str, id: &str) -> bool {
        self.change_class(class, |p| {
            let points = prestige::points_earned(p.talent_experience);
            prestige::learn(content, class, &mut p.prestige, id, points)
        })
    }

    /// Takes back one prestige rank of stat `id` for `class` (free).
    pub fn unlearn_prestige(&self, class: &str, id: &str) -> bool {
        self.change_class(class, |p| prestige::unlearn(&mut p.prestige, id))
    }

    /// The player opened `class`'s Collection or Talents: what it holds
    /// there now is seen, and its button on the warband screen stops glowing. `true` (kept) when
    /// it had news. The box telling that Challenge Mode is open, closed: never shown again for
    /// this save.
    pub fn see(&self, class: &str, view: esurv_protocol::SeenView) -> bool {
        if view == esurv_protocol::SeenView::Challenge {
            let mut file = self.file.lock().unwrap();
            let changed = !file.challenge_news_seen;
            file.challenge_news_seen = true;
            return !changed || self.write(&file).is_ok();
        }
        self.change_class(class, |p| {
            let now = p.holding();
            let mut seen = p.seen.unwrap_or_default();
            match view {
                esurv_protocol::SeenView::Collection => seen.pieces = now.pieces,
                esurv_protocol::SeenView::Talents => seen.points = now.points,
                esurv_protocol::SeenView::Challenge => {}
            }
            let changed = p.seen != Some(seen);
            p.seen = Some(seen);
            changed
        })
    }

    /// Gives back every prestige point `class` has spent (a talent reset does too); `false` when
    /// none was spent.
    pub fn reset_prestige(&self, class: &str) -> bool {
        self.change_class(class, |p| {
            let had = !p.prestige.is_empty();
            p.prestige.clear();
            had
        })
    }

    /// Puts class gear piece `entry` (numbered for `content`) on `class`, in place of the piece in
    /// its slot. Only a piece in the collection and made for the class.
    pub fn wear(&self, content: &Content, class: &str, entry: u32) -> bool {
        let Some((owner, piece)) = SavedPiece::of_entry(content, entry) else {
            return false;
        };
        if owner != class {
            return false;
        }
        self.change_class(class, |p| {
            if !p.collection.contains(&piece) {
                return false;
            }
            p.worn_pieces.retain(|w| w.slot != piece.slot);
            p.worn_pieces.push(piece);
            p.worn_pieces.sort();
            true
        })
    }

    /// Takes off what `class` wears in `slot`; `false` when nothing is worn there.
    pub fn take_off(&self, class: &str, slot: u32) -> bool {
        let Some(id) = slot_id(slot) else {
            return false;
        };
        self.change_class(class, |p| {
            let before = p.worn_pieces.len();
            p.worn_pieces.retain(|w| w.slot != id);
            p.worn_pieces.len() < before
        })
    }

    /// Shows look `look` (a place in the class set's `looks`) in `slot` for `class`, or with `None`
    /// the worn piece's own look. Only a look of a zone whose piece of the slot is in the
    /// collection (a piece unlocks every row of its zone's looks); the
    /// worn piece's own look is kept as no choice.
    pub fn set_look(&self, content: &Content, class: &str, slot: u32, look: Option<u32>) -> bool {
        let (Some(id), Some(set)) = (slot_id(slot), content.gear_set(class)) else {
            return false;
        };
        let p = self.saved_class(class);
        let chosen = match look {
            Some(l) => {
                let Some(l) = set.looks.get(l as usize) else {
                    return false;
                };
                if !p.collection.iter().any(|c| c.slot == id && c.zone == l.zone) {
                    return false;
                }
                Some(SavedLook { zone: l.zone.clone(), row: l.row })
            }
            None => None,
        };
        // the worn piece's own look: its zone's first row
        let own = p
            .worn_pieces
            .iter()
            .find(|w| w.slot == id)
            .and_then(|w| content.zones.iter().position(|z| z.id == w.zone))
            .map(|z| zone_look(content, set, z as u32))
            .and_then(|l| set.looks.get(l as usize))
            .map(|l| SavedLook { zone: l.zone.clone(), row: l.row });
        self.change_class(class, |p| {
            let before = p.looks.get(id).cloned();
            match chosen {
                Some(l) if Some(&l) != own.as_ref() => p.looks.insert(id.into(), l),
                _ => p.looks.remove(id),
            };
            p.looks.get(id) != before.as_ref()
        });
        true
    }

    /// Arms `class` with its weapon loadout `id` from its next run; `false` for an id the class's
    /// loadouts lack.
    pub fn choose_loadout(&self, content: &Content, class: &str, id: &str) -> bool {
        if !content.loadouts_of(class).iter().any(|l| l.id == id) {
            return false;
        }
        self.change_class(class, |p| {
            let changed = p.loadout.as_deref() != Some(id);
            p.loadout = Some(id.into());
            changed
        });
        true
    }

    /// Every class-gear piece won, by item entry numbered for `content` (a piece of a zone the
    /// content does not have is left out).
    pub fn collection(&self, content: &Content) -> Vec<u32> {
        let file = self.file.lock().unwrap();
        let mut out: Vec<u32> = file
            .classes
            .iter()
            .filter_map(|(class, p)| Some((content.class(class)?.wow_class_id, p)))
            .flat_map(|(wow_class, p)| p.collection.iter().filter_map(move |c| c.entry(content, wow_class)))
            .collect();
        out.sort_unstable();
        out
    }

    /// The run options the next run starts with.
    pub fn run_options(&self) -> SavedRunOptions {
        self.file.lock().unwrap().run_options
    }

    /// Sets the run options the next run starts with, each kept on its slider; what was kept.
    pub fn set_run_options(&self, content: &Content, options: SavedRunOptions) -> SavedRunOptions {
        let mut file = self.file.lock().unwrap();
        let kept = options.on_the_sliders(content);
        if file.run_options != kept {
            file.run_options = kept;
            let _ = self.write(&file);
        }
        kept
    }

    /// The client's tip bits, set for each tip seen.
    pub fn tips_seen(&self) -> [u32; TIP_WORDS] {
        self.file.lock().unwrap().tips_seen
    }

    /// Marks tip `id` seen (the client's tutorial flag); an id past the bits is left alone.
    pub fn see_tip(&self, id: u32) {
        let (word, bit) = ((id / 32) as usize, id % 32);
        self.change_tips(|tips| {
            if let Some(w) = tips.get_mut(word) {
                *w |= 1 << bit;
            }
        });
    }

    /// Marks every tip seen (`true`: the tips off) or none (`false`: every tip shows again).
    pub fn see_every_tip(&self, seen: bool) {
        self.change_tips(|tips| *tips = [if seen { u32::MAX } else { 0 }; TIP_WORDS]);
    }

    fn change_tips(&self, change: impl FnOnce(&mut [u32; TIP_WORDS])) {
        let mut file = self.file.lock().unwrap();
        let before = file.tips_seen;
        change(&mut file.tips_seen);
        if file.tips_seen != before {
            let _ = self.write(&file);
        }
    }

    /// Changes what `class` has earned; written when `change` says it did.
    fn change_class(&self, class: &str, change: impl FnOnce(&mut ClassProgress) -> bool) -> bool {
        let mut file = self.file.lock().unwrap();
        let changed = change(file.classes.entry(class.into()).or_insert_with(ClassProgress::new_to_the_save));
        if changed {
            let _ = self.write(&file);
        }
        changed
    }

    /// Writes the save next to its place, then moves it there, so a crash never leaves half a file.
    fn write(&self, file: &SaveFile) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = ron::ser::to_string_pretty(file, ron::ser::PrettyConfig::default())?;
        let temp = path.with_extension("ron.new");
        std::fs::write(&temp, text)?;
        std::fs::rename(&temp, path)?;
        Ok(())
    }
}

/// The name as it will be stored ("ana" becomes "Ana"), or why it is refused.
fn check_name(name: &str) -> Result<String, u8> {
    let count = name.chars().count();
    if count == 0 {
        return Err(code::CHAR_NAME_NO_NAME);
    }
    if count < NAME_LENGTH.0 {
        return Err(code::CHAR_NAME_TOO_SHORT);
    }
    if count > NAME_LENGTH.1 {
        return Err(code::CHAR_NAME_TOO_LONG);
    }
    if !name.chars().all(|c| c.is_alphabetic()) {
        return Err(code::CHAR_NAME_ONLY_LETTERS);
    }
    let mut chars = name.chars();
    let first = chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_default();
    Ok(first + &chars.as_str().to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    #[test]
    fn a_new_save_has_the_tips_off_and_keeps_what_the_player_turns_on() {
        let saves = Saves::in_memory();
        assert_eq!(saves.tips_seen(), [u32::MAX; TIP_WORDS], "every tip seen: the tips off");
        saves.see_every_tip(false);
        assert_eq!(saves.tips_seen(), [0; TIP_WORDS]);
        saves.see_tip(33);
        assert_eq!(saves.tips_seen()[1], 1 << 1);
        saves.see_tip(10_000);
        saves.see_every_tip(true);
        assert_eq!(saves.tips_seen(), [u32::MAX; TIP_WORDS]);
        // a save written before the tips were kept reads with them off
        let older = SaveFile::parse("(heroes: [], next_guid: 1, gear_numbering: 2)").unwrap();
        assert_eq!(older.tips_seen, [u32::MAX; TIP_WORDS]);
    }

    /// A zone reworked under a new id (Arathi Basin `ab` became the
    /// Sunken Temple): a save naming the old id names the new one wherever it kept it.
    #[test]
    fn a_save_naming_arathi_basin_names_the_sunken_temple() {
        let old = "(gear_numbering: 2, next_guid: 2, \
            heroes: [(guid: 1, name: \"Ana\", class: \"mage\", race: \"human\", gender: 1, look: (0, 0, 0, 0, 0), zone: Some(\"ab\"))], \
            last_played: Some((hero: 1, zone: \"ab\")), \
            classes: {\"mage\": (zones_won: [\"ab\", \"wsg\"], \
                collection: [(slot: \"head\", zone: \"ab\")], worn: [(slot: \"head\", zone: \"ab\")])})";
        let file = SaveFile::parse(old).unwrap();
        let mage = &file.classes["mage"];
        let temple = |slot: &str| SavedPiece { slot: slot.into(), zone: "sunken_temple".into() };
        assert_eq!(mage.zones_won, ["sunken_temple".to_string(), "wsg".into()].into());
        assert_eq!(mage.collection, [temple("head")].into());
        assert_eq!(mage.worn_pieces, [temple("head")]);
        assert_eq!(file.heroes[0].zone.as_deref(), Some("sunken_temple"));
        assert_eq!(file.last_played.as_ref().map(|l| l.zone.as_str()), Some("sunken_temple"));
    }

    /// Alterac Valley (`av`) became Stratholme (`strat`). A save naming
    /// the old id reads as Stratholme wherever it kept it (a win, gear won and worn, a chosen
    /// look, a hero's zone, the zone played last, the zones discovered), so nobody loses progress;
    /// written back, it names Stratholme.
    #[test]
    fn a_save_naming_alterac_valley_names_stratholme() {
        let old = "(gear_numbering: 2, next_guid: 2, \
            heroes: [(guid: 1, name: \"Ana\", class: \"mage\", race: \"human\", gender: 1, look: (0, 0, 0, 0, 0), zone: Some(\"av\"))], \
            last_played: Some((hero: 1, zone: \"av\")), \
            zones_discovered: [\"elwynn\", \"av\"], \
            classes: {\"mage\": (zones_won: [\"av\", \"wsg\"], \
                collection: [(slot: \"head\", zone: \"av\")], worn: [(slot: \"head\", zone: \"av\")], \
                looks: {\"head\": (zone: \"av\", row: 1)})})";
        let file = SaveFile::parse(old).unwrap();
        let mage = &file.classes["mage"];
        let strat = |slot: &str| SavedPiece { slot: slot.into(), zone: "strat".into() };
        assert_eq!(mage.zones_won, ["strat".to_string(), "wsg".into()].into());
        assert_eq!(mage.collection, [strat("head")].into());
        assert_eq!(mage.worn_pieces, [strat("head")]);
        assert_eq!(mage.looks["head"], SavedLook { zone: "strat".into(), row: 1 });
        assert_eq!(file.heroes[0].zone.as_deref(), Some("strat"));
        assert_eq!(file.last_played.as_ref().map(|l| l.zone.as_str()), Some("strat"));
        assert_eq!(file.zones_discovered, ["elwynn".to_string(), "strat".into()].into());
        let text = ron::to_string(&file).unwrap();
        assert!(!text.contains("\"av\""), "{text}");
        // the zone itself is in the content under its new id, at Alterac Valley's place
        let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let strat_zone = content.zones.iter().find(|z| z.id == "strat").expect("the strat zone");
        assert_eq!(strat_zone.difficulty, 8.0);
        assert!(content.zones.iter().all(|z| z.id != "av"));
    }

    #[test]
    fn heroes_are_kept_across_a_restart() {
        let dir = std::env::temp_dir().join(format!("esurv-save-test-{}", std::process::id()));
        let path = dir.join("heroes.ron");
        let _ = std::fs::remove_file(&path);
        let saves = Saves::open(&path).unwrap();
        let ana = saves
            .create("ana", "mage", "human", 1, [1, 2, 3, 4, 0])
            .unwrap();
        assert_eq!(ana.name, "Ana");
        saves.create("Bors", "warrior", "dwarf", 0, [0; 5]).unwrap();
        assert!(saves.delete(ana.guid));
        let reopened = Saves::open(&path).unwrap();
        let names: Vec<String> = reopened.heroes().into_iter().map(|h| h.name).collect();
        assert_eq!(names, ["Bors"]);
        let third = reopened
            .create("Cael", "rogue", "human", 0, [0; 5])
            .unwrap();
        assert!(third.guid > ana.guid, "guids are never reused");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Design step 5's "done when": a hero wins Elwynn, earns talent points and opens `duskwood`;
    /// the talents bought, the gear worn and its look, and the codex are all there after a
    /// restart.
    #[test]
    fn a_won_run_and_its_spending_are_kept_across_a_restart() {
        let content = esurv_content::Content::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../content"
        ))
        .unwrap();
        let dir =
            std::env::temp_dir().join(format!("esurv-progress-test-{}", std::process::id()));
        let path = dir.join("heroes.ron");
        let _ = std::fs::remove_file(&path);
        let saves = Saves::open(&path).unwrap();
        let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
        assert!(!saves.zone_open(&content, "warrior", 1));
        let (head0, head2) = (gear_item_entry(1, 0, 0), gear_item_entry(1, 0, 4)); // Elwynn, the Sunken Temple (its look: 4)
        let won_elwynn = FinishedRun {
            level: 15,
            seconds: 960.0,
            won: true,
            experience: 300,
            first: true,
            opens_zone: true,
            loot: vec![head0, head2],
            ..Default::default()
        };
        let paid = saves.finish_run(&content, bo.guid, &won_elwynn).unwrap();
        assert_eq!(paid.zone_opened, Some(1), "Area 10 opens");
        assert!(paid.points_after > 0, "talent points earned");
        let first = content
            .talent_trees
            .iter()
            .find(|t| t.class == "warrior")
            .unwrap()
            .talents
            .iter()
            .find(|t| t.row == 0)
            .unwrap();
        assert!(saves.learn_talent(&content, "warrior", &first.id));
        assert!(saves.wear(&content, "warrior", head0));
        assert!(saves.set_look(&content, "warrior", 0, Some(4)));
        assert!(saves.add_to_codex("ring_of_power"));
        drop(saves);

        let reopened = Saves::open(&path).unwrap();
        assert!(reopened.zone_open(&content, "warrior", 1), "Area 10 stays open");
        let progress = reopened.class_progress(&content, "warrior");
        assert_eq!(progress.talents[&first.id], 1);
        assert_eq!(progress.worn, [head0]);
        assert_eq!(progress.appearance[&0], 4);
        assert_eq!(reopened.collection(&content), [head0, head2]);
        assert_eq!(reopened.codex(), ["ring_of_power"]);
        let records = reopened.hero(bo.guid).unwrap().records;
        assert_eq!((records.runs, records.wins), (1, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_finished_run_pays_out_to_the_hero_and_its_class() {
        let content = content();
        let saves = Saves::in_memory();
        let ana = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        let bo = saves.create("Bo", "mage", "gnome", 0, [0; 5]).unwrap();
        assert!(saves.zone_open(&content, "mage", 0) && !saves.zone_open(&content, "mage", 1));
        let lost = FinishedRun {
            level: 12,
            seconds: 300.0,
            experience: 70,
            first: true,
            loot: vec![gear_item_entry(8, 1, 0)],
            ..Default::default()
        };
        let paid = saves.finish_run(&content, ana.guid, &lost).unwrap();
        assert_eq!((paid.points_before, paid.points_after), (0, 0));
        assert_eq!((paid.total_before, paid.total_after), (0, 70));
        assert_eq!(paid.zone_opened, None);
        let won = FinishedRun {
            level: 9,
            seconds: 900.0,
            won: true,
            experience: 300,
            first: true,
            opens_zone: true,
            loot: vec![gear_item_entry(8, 1, 0), gear_item_entry(8, 8, 1)],
            ..Default::default()
        };
        // 370 buys points of 110, 113 and 115 (338)
        let paid = saves.finish_run(&content, bo.guid, &won).unwrap();
        assert_eq!((paid.points_before, paid.points_after), (0, 3));
        assert_eq!(paid.zone_opened, Some(1), "Area 10 opens");
        // the same run paid again (Endless, then left): its new experience only, counted once
        let more = FinishedRun {
            level: 14,
            seconds: 1200.0,
            won: true,
            experience: 25,
            ..Default::default()
        };
        let paid = saves.finish_run(&content, bo.guid, &more).unwrap();
        assert_eq!((paid.total_before, paid.total_after), (370, 395));
        let records = saves.hero(bo.guid).unwrap().records;
        assert_eq!((records.best_level, records.runs, records.wins), (14, 1, 1));
        saves.finish_run(
            &content,
            bo.guid,
            &FinishedRun {
                experience: 0,
                ..more.clone()
            },
        );
        assert!(saves.zone_open(&content, "mage", 1), "for every mage");
        assert!(!saves.zone_open(&content, "warrior", 1), "not for another class");
        let ana = saves.hero(ana.guid).unwrap().records;
        assert_eq!(
            (ana.best_level, ana.best_seconds, ana.runs, ana.wins),
            (12, 300, 1, 0)
        );
        let records = saves.hero(bo.guid).unwrap().records;
        assert_eq!((records.runs, records.wins), (1, 1));
        assert_eq!(saves.class_progress(&content, "mage").talent_experience, 395);
        assert_eq!(saves.collection(&content).len(), 2, "a piece won twice is kept once");
        // winning the same zone again opens nothing more
        assert_eq!(saves.finish_run(&content, bo.guid, &won).unwrap().zone_opened, None);
    }

    #[test]
    fn talents_are_bought_with_earned_points_and_given_back_freely() {
        let content = esurv_content::Content::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../content"
        ))
        .unwrap();
        let saves = Saves::in_memory();
        let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
        let first = content
            .talent_trees
            .iter()
            .find(|t| t.class == "warrior")
            .unwrap()
            .talents
            .iter()
            .find(|t| t.row == 0)
            .unwrap();
        assert!(
            !saves.learn_talent(&content, "warrior", &first.id),
            "no points yet"
        );
        let run = FinishedRun {
            experience: 145,
            ..Default::default()
        };
        saves.finish_run(&content, bo.guid, &run).unwrap();
        assert!(saves.learn_talent(&content, "warrior", &first.id));
        assert!(
            !saves.learn_talent(&content, "warrior", &first.id),
            "one point earned, one spent"
        );
        assert_eq!(saves.class_progress(&content, "warrior").talents[&first.id], 1);
        assert!(saves.unlearn_talent(&content, "warrior", &first.id));
        assert!(saves.class_progress(&content, "warrior").talents.is_empty());
    }

    /// Prestige points come once every talent point is earned; their ranks are kept across a
    /// restart, given back one at a time or all at once (a talent reset).
    #[test]
    fn prestige_ranks_are_bought_kept_and_given_back() {
        use esurv_sim::game::rewards::{prestige_point_cost, talent_point_cost, MAX_TALENT_POINTS};
        let content = content();
        let dir = std::env::temp_dir().join(format!("esurv-prestige-test-{}", std::process::id()));
        let path = dir.join("heroes.ron");
        let _ = std::fs::remove_file(&path);
        let saves = Saves::open(&path).unwrap();
        let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
        let all_talents: u32 = (1..=MAX_TALENT_POINTS).map(talent_point_cost).sum();
        let paid = |experience| FinishedRun { experience, ..Default::default() };
        saves.finish_run(&content, bo.guid, &paid(all_talents)).unwrap();
        assert!(!saves.learn_prestige(&content, "warrior", "damage"), "51 talent points, no prestige point yet");
        saves.finish_run(&content, bo.guid, &paid(prestige_point_cost(1) + prestige_point_cost(2))).unwrap();
        assert!(saves.learn_prestige(&content, "warrior", "damage"));
        assert!(saves.learn_prestige(&content, "warrior", "block_chance"));
        assert!(!saves.learn_prestige(&content, "warrior", "damage"), "two points, two ranks");
        assert!(!saves.learn_prestige(&content, "mage", "damage"), "another class's points");
        drop(saves);

        let saves = Saves::open(&path).unwrap();
        let p = saves.class_progress(&content, "warrior");
        assert_eq!(p.prestige, [("block_chance".to_owned(), 1), ("damage".to_owned(), 1)].into());
        assert!(saves.unlearn_prestige("warrior", "damage"));
        assert!(!saves.unlearn_prestige("warrior", "damage"));
        assert!(saves.learn_prestige(&content, "warrior", "max_health"));
        assert!(saves.reset_prestige("warrior"));
        assert!(saves.class_progress(&content, "warrior").prestige.is_empty());
        assert!(!saves.reset_prestige("warrior"), "nothing left to give back");
        // every point is free again
        assert!(saves.learn_prestige(&content, "warrior", "damage_reduction"));
        assert!(saves.learn_prestige(&content, "warrior", "damage_reduction"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gear_is_worn_from_the_collection_one_piece_a_slot() {
        let content = content();
        let saves = Saves::in_memory();
        let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
        // warrior (1): head tier 0 and tier 1, and a paladin's head
        let (head0, head1, paladin) = (gear_item_entry(1, 0, 0), gear_item_entry(1, 0, 1), gear_item_entry(2, 0, 0));
        assert!(!saves.wear(&content, "warrior", head0), "not won yet");
        let run = FinishedRun {
            loot: vec![head0, head1, paladin],
            ..Default::default()
        };
        saves.finish_run(&content, bo.guid, &run).unwrap();
        assert!(saves.wear(&content, "warrior", head0));
        assert!(saves.wear(&content, "warrior", head1));
        assert_eq!(
            saves.class_progress(&content, "warrior").worn,
            [head1],
            "one piece a slot"
        );
        assert!(!saves.wear(&content, "warrior", paladin), "another class's piece");
        assert!(saves.take_off("warrior", 0));
        assert!(!saves.take_off("warrior", 0));
    }

    #[test]
    fn a_slot_shows_a_look_from_the_collection() {
        let content = content();
        let saves = Saves::in_memory();
        let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
        let (head0, head2) = (gear_item_entry(1, 0, 0), gear_item_entry(1, 0, 4)); // Elwynn, the Sunken Temple (its look: 4)
        assert!(!saves.set_look(&content, "warrior", 0, Some(4)), "not won yet");
        let run = FinishedRun {
            loot: vec![head0, head2],
            ..Default::default()
        };
        saves.finish_run(&content, bo.guid, &run).unwrap();
        assert!(saves.wear(&content, "warrior", head0));
        assert!(saves.set_look(&content, "warrior", 0, Some(4)));
        assert_eq!(saves.class_progress(&content, "warrior").appearance[&0], 4);
        assert!(
            saves.set_look(&content, "warrior", 0, Some(0)),
            "the worn piece's own"
        );
        assert!(saves.class_progress(&content, "warrior").appearance.is_empty());
        assert!(!saves.set_look(&content, "warrior", 0, Some(5)), "not won yet");
        assert!(!saves.set_look(&content, "warrior", 0, Some(99)), "no such look");
        assert!(!saves.set_look(&content, "warrior", 12, Some(0)), "no such slot");
        assert!(saves.set_look(&content, "warrior", 0, None));
    }

    #[test]
    fn run_options_are_kept_for_the_save_on_their_sliders() {
        let saves = Saves::in_memory();
        assert_eq!(saves.run_options(), SavedRunOptions::default());
        let asked = SavedRunOptions {
            density: 260,
            health: 100,
            damage: 900,
            speed: 105,
            challenge: true,
            challenge_pct: 440,
        };
        let content = esurv_content::Content::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../content"
        ))
        .unwrap();
        let kept = saves.set_run_options(&content, asked);
        assert_eq!(
            kept,
            SavedRunOptions {
                density: 250,
                health: 100,
                damage: 300,
                speed: 105,
                challenge: true,
                challenge_pct: 450
            }
        );
        assert_eq!(saves.run_options(), kept);
    }

    #[test]
    fn the_codex_keeps_each_piece_once_for_every_hero() {
        let saves = Saves::in_memory();
        assert!(saves.add_to_codex("ring_of_power"));
        assert!(!saves.add_to_codex("ring_of_power"), "once");
        assert!(saves.add_to_codex("legendary:fireball"));
        assert_eq!(saves.codex(), ["legendary:fireball", "ring_of_power"]);
    }

    #[test]
    fn a_save_from_before_progress_still_opens() {
        let old = "(heroes: [(guid: 1, name: \"Ana\", class: \"mage\", race: \"human\", gender: 1, look: (1, 2, 3, 4, 0))], next_guid: 2)";
        let file = SaveFile::parse(old).unwrap();
        assert_eq!(file.heroes[0].records, HeroRecords::default());
        assert!(file.classes.is_empty());
        assert_eq!(file.gear_numbering, GEAR_BY_NAME);
    }

    /// Gear won and points earned are news for the class until the
    /// player opens its Collection or its Talents; each view clears its own, per class, and what
    /// was seen is kept across a restart.
    #[test]
    fn what_a_class_won_is_news_until_its_view_is_opened() {
        use esurv_protocol::SeenView;
        let content = content();
        let dir = std::env::temp_dir().join(format!("esurv-seen-test-{}", std::process::id()));
        let path = dir.join("heroes.ron");
        let _ = std::fs::remove_file(&path);
        let saves = Saves::open(&path).unwrap();
        let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
        assert_eq!(saves.class_progress(&content, "warrior").unseen(), (false, false), "nothing won yet");
        let run = FinishedRun {
            experience: 145,
            first: true,
            loot: vec![gear_item_entry(1, 0, 0)],
            ..Default::default()
        };
        saves.finish_run(&content, bo.guid, &run).unwrap();
        assert_eq!(saves.class_progress(&content, "warrior").unseen(), (true, true));
        assert_eq!(saves.class_progress(&content, "mage").unseen(), (false, false), "another class has no news");
        // kept across a restart: a class new to the save has seen nothing
        let saves = Saves::open(&path).unwrap();
        assert_eq!(saves.class_progress(&content, "warrior").unseen(), (true, true));
        assert!(saves.see("warrior", SeenView::Collection));
        assert_eq!(saves.class_progress(&content, "warrior").unseen(), (false, true));
        assert!(saves.see("warrior", SeenView::Talents));
        assert!(!saves.see("warrior", SeenView::Talents), "nothing more to see");
        let saves = Saves::open(&path).unwrap();
        assert_eq!(saves.class_progress(&content, "warrior").unseen(), (false, false));
        // the next run's points are news again; the gear seen stays seen
        saves.finish_run(&content, bo.guid, &FinishedRun { experience: 200, ..Default::default() }).unwrap();
        assert_eq!(saves.class_progress(&content, "warrior").unseen(), (false, true));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A save from before what was seen was kept loads with nothing new to show.
    #[test]
    fn an_older_save_has_no_news() {
        let content = content();
        let old = "(gear_numbering: 2, heroes: [], next_guid: 1, classes: {\"mage\": (talent_experience: 900, collection: [(slot: \"head\", zone: \"elwynn\")])})";
        let file = SaveFile::parse(old).unwrap();
        let mage = &file.classes["mage"];
        assert!(mage.points_earned() > 0 && !mage.collection.is_empty());
        assert_eq!(mage.unseen(), (false, false));
        let saves = Saves::in_memory();
        *saves.file.lock().unwrap() = file;
        assert_eq!(saves.class_progress(&content, "mage").unseen(), (false, false));
    }

    /// Roadmap R3: a class keeps the weapon loadout chosen for it; a
    /// save from before loadouts, or one naming a loadout the class lacks, fights with its first.
    #[test]
    fn a_class_keeps_its_weapon_loadout_and_an_older_save_takes_the_first() {
        let content = content();
        let older = "(heroes: [], next_guid: 1, gear_numbering: 2, classes: {\"rogue\": (worn: [])})";
        let file = SaveFile::parse(older).unwrap();
        assert_eq!(file.classes["rogue"].loadout, None);
        let saves = Saves::in_memory();
        let fights_with = |s: &Saves| {
            let p = s.class_progress(&content, "rogue");
            content.loadout("rogue", p.loadout.as_deref()).map(|l| l.id.clone())
        };
        assert_eq!(fights_with(&saves).as_deref(), Some("daggers"));
        assert!(!saves.choose_loadout(&content, "rogue", "two_handed_axe"), "no such loadout");
        assert!(!saves.choose_loadout(&content, "rogue", "staff"), "a mage's, not a rogue's");
        assert!(saves.choose_loadout(&content, "rogue", "swords"));
        assert_eq!(fights_with(&saves).as_deref(), Some("swords"));
        assert_eq!(saves.class_progress(&content, "mage").loadout, None, "each class its own");
        // written and read back
        let text = ron::to_string(&*saves.file.lock().unwrap()).unwrap();
        assert!(text.contains("loadout:Some(\"swords\")"), "{text}");
        let again = SaveFile::parse(&text).unwrap();
        assert_eq!(again.classes["rogue"].loadout.as_deref(), Some("swords"));
    }

    fn piece(slot: &str, zone: &str) -> SavedPiece {
        SavedPiece { slot: slot.into(), zone: zone.into() }
    }

    fn look(zone: &str, row: u32) -> SavedLook {
        SavedLook { zone: zone.into(), row }
    }

    /// A save from before every zone had its own look (pieces by slot
    /// and zone, looks chosen by tier) still loads: its collection and worn gear as they were, a
    /// look chosen by tier moved on to the latest zone of that tier whose piece of the slot was
    /// won, and one of a tier none of whose pieces was won let go. Written back, the looks are
    /// kept by zone and row, and read again nothing moves twice.
    #[test]
    fn a_save_with_looks_by_tier_keeps_its_gear_and_moves_its_looks_to_zones() {
        let content = content();
        let old = "(gear_numbering: 2, heroes: [], next_guid: 1, classes: {\"warrior\": ( \
                   collection: [(slot: \"head\", zone: \"elwynn\"), (slot: \"head\", zone: \"scarlet\"), \
                                (slot: \"head\", zone: \"feralas\"), (slot: \"feet\", zone: \"brm\"), \
                                (slot: \"chest\", zone: \"duskwood\"), (slot: \"main_hand\", zone: \"aq\")], \
                   worn: [(slot: \"head\", zone: \"elwynn\"), (slot: \"feet\", zone: \"brm\"), (slot: \"main_hand\", zone: \"aq\")], \
                   appearance: {\"head\": 2, \"feet\": 4, \"chest\": 3}, loadout: Some(\"shield\"))})";
        let file = SaveFile::parse(old).unwrap();
        let w = &file.classes["warrior"];
        assert_eq!(w.collection.len(), 6);
        assert_eq!(w.worn_pieces, [piece("head", "elwynn"), piece("feet", "brm"), piece("main_hand", "aq")]);
        assert_eq!(w.loadout.as_deref(), Some("shield"));
        // tier 2 was the Scarlet Monastery, the Sunken Temple and Dire Maul: the latest won
        assert_eq!(w.looks["head"], look("feralas", 0));
        assert_eq!(w.looks["feet"], look("brm", 0));
        assert!(!w.looks.contains_key("chest"), "no chest of tier 3 was won");
        assert!(w.old_looks.is_empty());
        let text = ron::to_string(&file).unwrap();
        assert!(!text.contains("appearance"), "{text}");
        let again = SaveFile::parse(&text).unwrap();
        assert_eq!(again.classes, file.classes);

        // the save numbers them for the content: the head in Dire Maul's look
        let saves = Saves::in_memory();
        *saves.file.lock().unwrap() = file;
        let set = content.gear_set("warrior").unwrap();
        let p = saves.class_progress(&content, "warrior");
        assert_eq!(p.appearance[&0], set.look_of("feralas", 0).unwrap() as u32);
        assert_eq!(p.worn.len(), 3);
    }

    /// Blackrock has two rows of looks; a piece of it unlocks the same
    /// piece's look in both rows, and the player picks the row to show. The pick is kept by zone
    /// and row.
    #[test]
    fn a_blackrock_piece_unlocks_both_rows_of_its_look() {
        let content = content();
        let saves = Saves::in_memory();
        let set = content.gear_set("warrior").unwrap();
        let brm = content.zones.iter().position(|z| z.id == "brm").unwrap() as u32;
        let (might, wrath) = (set.look_of("brm", 0).unwrap() as u32, set.look_of("brm", 1).unwrap() as u32);
        let (helm, boots) = (gear_item_entry(1, 0, 1), gear_item_entry(1, 5, brm));
        assert!(!saves.set_look(&content, "warrior", 5, Some(wrath)), "not won yet");
        saves.add_to_collection(&content, helm);
        saves.add_to_collection(&content, boots);
        assert!(saves.wear(&content, "warrior", boots) && saves.wear(&content, "warrior", helm));
        // the boots' own look is the first row; the second is a choice
        assert!(saves.set_look(&content, "warrior", 5, Some(wrath)));
        assert_eq!(saves.class_progress(&content, "warrior").appearance[&5], wrath);
        assert_eq!(saves.saved_class("warrior").looks["feet"], look("brm", 1));
        assert!(saves.set_look(&content, "warrior", 5, Some(might)), "the first row: its own");
        assert!(saves.class_progress(&content, "warrior").appearance.is_empty());
        // the Blackrock boots unlock both rows for the feet only: not for the Duskwood helm
        assert!(!saves.set_look(&content, "warrior", 0, Some(wrath)));
        let text = ron::to_string(&*saves.file.lock().unwrap()).unwrap();
        assert!(text.contains("(slot:\"feet\",zone:\"brm\")"), "{text}");
        assert!(saves.set_look(&content, "warrior", 5, Some(wrath)));
        let text = ron::to_string(&*saves.file.lock().unwrap()).unwrap();
        assert!(text.contains("looks:{\"feet\":(zone:\"brm\",row:1)}"), "{text}");
    }

    /// A save of the first numbering (by tier) keeps its gear, each old
    /// piece of tier t now the piece of the later zone of that tier; a chosen look stays where a
    /// piece of its tier was won.
    #[test]
    fn a_save_numbered_by_tier_keeps_its_pieces_by_name() {
        // warrior (1): head tier 0, chest tier 1 (worn), weapon tier 4 (worn); a mage head tier 3;
        // and an entry that is no gear
        let old = "(heroes: [], next_guid: 1, \
                   classes: {\"warrior\": (worn: [101210, 101840], appearance: {2: 1, 0: 3})}, \
                   collection: [101000, 101210, 101840, 108030, 42])";
        let file = SaveFile::parse(old).unwrap();
        let warrior = &file.classes["warrior"];
        let want = [piece("chest", "wsg"), piece("head", "elwynn"), piece("main_hand", "aq")];
        assert_eq!(warrior.collection, want.into());
        assert_eq!(warrior.worn_pieces, [piece("chest", "wsg"), piece("main_hand", "aq")]);
        // a look chosen by tier: the zone of the tier whose piece was won; one not won goes
        assert_eq!(warrior.looks["chest"], look("wsg", 0), "a chosen look stays");
        assert!(!warrior.looks.contains_key("head"));
        assert_eq!(file.classes["mage"].collection, [piece("head", "strat")].into());
        assert_eq!(file.gear_numbering, GEAR_BY_NAME);
        // written back and read again: nothing moves twice
        let text = ron::to_string(&file).unwrap();
        assert!(text.contains("(slot:\"head\",zone:\"elwynn\")"), "{text}");
        let again = SaveFile::parse(&text).unwrap();
        assert_eq!(again.classes, file.classes);
    }

    /// A save of the second numbering (`1 000 000 + class × 10 000 + slot × 1000 + zone × 10`,
    /// zones counted in its `gear_zones`, else the order before zone files) keeps its pieces
    /// by name, whatever order the content's zones are in now.
    #[test]
    fn a_save_numbered_by_zone_keeps_its_pieces_by_name() {
        let old_entry = |class: u32, slot: u32, zone: u32| 1_000_000 + class * 10_000 + slot * 1000 + zone * 10;
        // no gear_zones: the order before zone files (wsg third, aq last)
        let (head, boots, blade) = (old_entry(1, 0, 0), old_entry(1, 5, 2), old_entry(1, 8, 8));
        let old = format!(
            "(gear_numbering: 1, heroes: [], next_guid: 1, collection: [{head}, {boots}, {blade}, {}, 7], \
             classes: {{\"warrior\": (worn: [{boots}], appearance: {{5: 1}})}})",
            old_entry(8, 9, 3)
        );
        let file = SaveFile::parse(&old).unwrap();
        let warrior = &file.classes["warrior"];
        assert_eq!(warrior.collection, [piece("feet", "wsg"), piece("head", "elwynn"), piece("main_hand", "aq")].into());
        assert_eq!(warrior.worn_pieces, [piece("feet", "wsg")]);
        assert_eq!(warrior.looks["feet"], look("wsg", 0));
        assert_eq!(file.classes["mage"].collection, [piece("off_hand", "sunken_temple")].into());
        // with gear_zones: its zones counted in that order, a zone the content lost kept by name
        let moved = format!(
            "(gear_numbering: 1, gear_zones: [\"elwynn\", \"duskwood\", \"test_middle\", \"wsg\"], \
             heroes: [], next_guid: 1, collection: [{}, {}])",
            old_entry(1, 0, 2),
            old_entry(1, 0, 3)
        );
        let file = SaveFile::parse(&moved).unwrap();
        assert_eq!(file.classes["warrior"].collection, [piece("head", "test_middle"), piece("head", "wsg")].into());
    }

    /// A piece is kept by its zone's id: with a zone added in the middle of the order, every
    /// piece keeps its zone and only its item entry, worked out for the session, moves; a piece
    /// of a zone the content does not have is kept but not shown.
    #[test]
    fn class_gear_keeps_its_zone_when_a_zone_is_added_or_taken_out() {
        let shipped = content();
        let added = content_with_added_zones("save-moved-zone");
        let saves = Saves::in_memory();
        let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
        let index = |c: &Content, id: &str| c.zones.iter().position(|z| z.id == id).unwrap() as u32;
        let (wsg, middle) = (index(&added, "wsg"), index(&added, "test_middle"));
        assert_eq!(index(&shipped, "wsg") + 1, wsg, "the added zone comes before Warsong");
        let run = FinishedRun { loot: vec![gear_item_entry(1, 0, wsg), gear_item_entry(1, 1, middle)], ..Default::default() };
        saves.finish_run(&added, bo.guid, &run).unwrap();
        assert!(saves.wear(&added, "warrior", gear_item_entry(1, 0, wsg)));
        // the same save on the shipped zones: Warsong's piece under Warsong's place there
        assert_eq!(saves.class_progress(&shipped, "warrior").worn, [gear_item_entry(1, 0, index(&shipped, "wsg"))]);
        assert_eq!(saves.collection(&shipped), [gear_item_entry(1, 0, index(&shipped, "wsg"))]);
        // and back on the content with the zone: both pieces as they were
        assert_eq!(saves.collection(&added), [gear_item_entry(1, 0, wsg), gear_item_entry(1, 1, middle)]);
        assert_eq!(saves.class_progress(&added, "warrior").worn, [gear_item_entry(1, 0, wsg)]);
    }

    #[test]
    fn a_hero_is_renamed_reshaped_and_keeps_its_zone() {
        let saves = Saves::in_memory();
        let ana = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        saves.create("Bo", "mage", "gnome", 0, [0; 5]).unwrap();
        assert_eq!(
            saves.rename(ana.guid, "bo"),
            Err(code::CHAR_CREATE_NAME_IN_USE)
        );
        assert_eq!(saves.rename(ana.guid, "A"), Err(code::CHAR_NAME_TOO_SHORT));
        assert_eq!(saves.rename(ana.guid, "anya"), Ok("Anya".into()));
        assert_eq!(
            saves.rename(ana.guid, "ANYA"),
            Ok("Anya".into()),
            "its own name"
        );
        assert!(saves.reshape(ana.guid, "gnome", 0, [1, 2, 3, 4, 5]));
        assert!(!saves.reshape(99, "gnome", 0, [0; 5]));
        let h = saves.hero(ana.guid).unwrap();
        assert_eq!(
            (h.name.as_str(), h.race.as_str(), h.gender, h.look),
            ("Anya", "gnome", 0, [1, 2, 3, 4, 5])
        );
        assert_eq!(h.class, "mage", "the class stays");
        assert_eq!(h.zone, None);
        assert!(saves.choose_zone(ana.guid, "duskwood"));
        assert_eq!(
            saves.hero(ana.guid).unwrap().zone.as_deref(),
            Some("duskwood")
        );
        saves.played(ana.guid, "elwynn");
        assert_eq!(
            saves.hero(ana.guid).unwrap().zone.as_deref(),
            Some("elwynn"),
            "the zone entered last"
        );
        assert!(!saves.choose_zone(99, "elwynn"));
    }

    #[test]
    fn names_follow_the_stock_rules() {
        let saves = Saves::in_memory();
        assert_eq!(
            saves.create("A", "mage", "human", 0, [0; 5]),
            Err(code::CHAR_NAME_TOO_SHORT)
        );
        assert_eq!(
            saves.create("Abcdefghijklm", "mage", "human", 0, [0; 5]),
            Err(code::CHAR_NAME_TOO_LONG)
        );
        assert_eq!(
            saves.create("Ana1", "mage", "human", 0, [0; 5]),
            Err(code::CHAR_NAME_ONLY_LETTERS)
        );
        saves.create("Ana", "mage", "human", 0, [0; 5]).unwrap();
        assert_eq!(
            saves.create("ANA", "rogue", "orc", 0, [0; 5]),
            Err(code::CHAR_CREATE_NAME_IN_USE)
        );
    }

    /// The shipped content with two zones added as files: "test_middle" (D 1.5, between `duskwood`
    /// and Warsong) and "test_end" (D 9, after `aq`).
    fn content_with_added_zones(test: &str) -> Content {
        use esurv_content::testing::{content_folder_with_zones, ExtraZone};
        let shipped = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let zones = [
            ExtraZone { id: "test_middle", name: "Test Middle", difficulty: 2.5 },
            ExtraZone { id: "test_end", name: "Test End", difficulty: 11.0 },
        ];
        let dir = content_folder_with_zones(shipped.as_ref(), test, &zones).unwrap();
        let content = Content::load(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        content.unwrap()
    }

    /// A save from before the zones won were kept counts its open zones in the old order: n open
    /// means zone n - 1 (from 0: n - 2) was won last. It opens exactly what it did.
    #[test]
    fn a_save_counting_open_zones_keeps_them_as_the_zone_won_last() {
        let old = "(gear_numbering: 1, heroes: [], next_guid: 1, classes: {                   \"mage\": (zones_open: 3), \"rogue\": (zones_open: 1),                    \"warrior\": (zones_open: 9), \"druid\": (talent_experience: 5)})";
        let file = SaveFile::parse(old).unwrap();
        let won = |class: &str| file.classes[class].zones_won.iter().cloned().collect::<Vec<_>>();
        assert_eq!(won("mage"), ["duskwood"]);
        assert_eq!(won("warrior"), ["brm"]);
        assert!(won("rogue").is_empty() && won("druid").is_empty());
        let content = content();
        let open = |class: &str| file.classes[class].open_zones(&content);
        // Blackrock won opens up to Ahn'Qiraj, the Scarlet Monastery added among them
        assert_eq!([open("mage"), open("rogue"), open("warrior"), open("druid")], [3, 1, 10, 1]);
        // written back, the count is gone and nothing moves twice
        let text = ron::to_string(&file).unwrap();
        assert!(!text.contains("zones_open"), "{text}");
        assert_eq!(SaveFile::parse(&text).unwrap().classes, file.classes);
    }

    /// Zones open by difficulty from the hardest won, so a zone added
    /// below a class's progress is open at once and nothing it had opened closes.
    #[test]
    fn a_zone_added_by_difficulty_opens_in_its_place() {
        let shipped = content();
        let content = content_with_added_zones("save-added-zones");
        let index = |id: &str| content.zones.iter().position(|z| z.id == id).unwrap();
        let saves = Saves::in_memory();
        let ana = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        let won_in = |id: &str| FinishedRun {
            won: true,
            first: true,
            opens_zone: true,
            zone_index: index(id),
            ..Default::default()
        };
        let paid = saves.finish_run(&content, ana.guid, &won_in("duskwood")).unwrap();
        assert_eq!(paid.zone_opened, Some(index("test_middle")), "after Area 10 comes D 1.5");
        assert!(!saves.zone_open(&content, "mage", index("wsg")));
        saves.finish_run(&content, ana.guid, &won_in("wsg")).unwrap();
        for id in ["elwynn", "duskwood", "test_middle", "wsg", "scarlet"] {
            assert!(saves.zone_open(&content, "mage", index(id)), "{id}");
        }
        assert!(!saves.zone_open(&content, "mage", index("sunken_temple")));
        // the same progress on the shipped zones opens the same zones, less the added one
        assert_eq!(saves.class_progress(&content, "mage").open_zones(&shipped), 4);
        let bo = saves.create("Bo", "rogue", "human", 0, [0; 5]).unwrap();
        let paid = saves.finish_run(&content, bo.guid, &won_in("aq")).unwrap();
        assert_eq!(paid.zone_opened, Some(content.zones.len() - 1), "Test End opens last");
        assert_eq!(content.zones.last().unwrap().id, "test_end");
        assert!(saves.zone_open(&content, "rogue", index("test_end")));
    }

    /// A zone opened to any class is discovered for the whole save,
    /// kept on disk; another class still has it locked.
    #[test]
    fn a_zone_opened_to_one_class_is_discovered_for_all_and_kept() {
        let content = content();
        let dir = std::env::temp_dir().join(format!("esurv-discovery-test-{}", std::process::id()));
        let path = dir.join("save.ron");
        let _ = std::fs::remove_dir_all(&dir);
        let saves = Saves::open(&path).unwrap();
        assert_eq!(saves.zones_discovered(&content), ["elwynn"], "the first zone always is");
        let ana = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        saves.create("Bo", "rogue", "human", 0, [0; 5]).unwrap();
        let won = FinishedRun { won: true, first: true, opens_zone: true, zone_index: 0, ..Default::default() };
        saves.finish_run(&content, ana.guid, &won).unwrap();
        assert_eq!(saves.zones_discovered(&content), ["elwynn", "duskwood"]);
        assert!(!saves.zone_open(&content, "rogue", 1), "discovered is not open");
        // read back from disk, the same
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("zones_discovered"), "{text}");
        assert_eq!(Saves::open(&path).unwrap().zones_discovered(&content), ["elwynn", "duskwood"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A save from before discovery was kept: every zone open to any of its classes counts as
    /// discovered, and is kept once read.
    #[test]
    fn an_older_save_discovers_every_zone_its_classes_have_open() {
        let content = content();
        let old = "(gear_numbering: 2, heroes: [], next_guid: 1, classes: { \"mage\": (zones_won: [\"duskwood\"]), \"rogue\": (zones_won: [\"elwynn\"]) })";
        let mut file = SaveFile::parse(old).unwrap();
        assert!(file.zones_discovered.is_empty());
        assert!(file.discover_open_zones(&content));
        let ids: Vec<&str> = file.zones_discovered.iter().map(String::as_str).collect();
        let mut want = vec!["elwynn", "duskwood", "wsg"];
        want.sort();
        assert_eq!(ids, want);
        assert!(!file.discover_open_zones(&content), "nothing new the second time");
        let text = ron::to_string(&file).unwrap();
        assert_eq!(SaveFile::parse(&text).unwrap().zones_discovered, file.zones_discovered);
    }

    /// A save as the v0.1.0 release wrote it (its `heroes.ron`, pretty-printed, every field it
    /// had): no class's `seen`, looks by zone and row, the hero's start option, a loadout.
    const V0_1_0_SAVE: &str = r#"(
    gear_numbering: 2,
    heroes: [
        (
            guid: 1,
            name: "Patchtest",
            class: "warrior",
            race: "orc",
            gender: 0,
            look: (1, 2, 3, 4, 0),
            records: (
                best_level: 14,
                best_seconds: 612,
                runs: 3,
                wins: 1,
            ),
            zone: Some("duskwood"),
            start: Some("slam"),
        ),
    ],
    next_guid: 2,
    classes: {
        "warrior": (
            talent_experience: 900,
            talents: {
                "imp_heroic": 2,
            },
            prestige: {
                "damage": 1,
            },
            zones_won: [
                "elwynn",
            ],
            collection: [
                (
                    slot: "feet",
                    zone: "brm",
                ),
                (
                    slot: "head",
                    zone: "elwynn",
                ),
            ],
            worn: [
                (
                    slot: "head",
                    zone: "elwynn",
                ),
            ],
            looks: {
                "feet": (
                    zone: "brm",
                    row: 1,
                ),
                "head": (
                    zone: "elwynn",
                ),
            },
            loadout: Some("two_hander"),
        ),
    },
    codex: [
        "accuria",
    ],
    last_played: Some((
        hero: 1,
        zone: "duskwood",
    )),
    run_options: (
        density: 120,
        health: 100,
        damage: 90,
        speed: 100,
    ),
    tips_seen: (4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295),
    zones_discovered: [
        "duskwood",
        "elwynn",
    ],
)"#;

    /// Release v0.2.0: a v0.1.0 save loads in this version with all it
    /// held, the fields added since (a class's `seen`) are filled in without news, and written
    /// back it loads again the same.
    #[test]
    fn a_v0_1_0_save_loads_with_everything_it_held() {
        use esurv_protocol::SeenView;
        let content = content();
        let dir = std::env::temp_dir().join(format!("esurv-v010-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("heroes.ron");
        std::fs::write(&path, V0_1_0_SAVE).unwrap();
        let check = |saves: &Saves| {
            let heroes = saves.heroes();
            assert_eq!(heroes.len(), 1);
            let h = &heroes[0];
            assert_eq!((h.guid, h.name.as_str(), h.class.as_str(), h.race.as_str()), (1, "Patchtest", "warrior", "orc"));
            assert_eq!(h.look, [1, 2, 3, 4, 0]);
            assert_eq!((h.records.best_level, h.records.runs, h.records.wins), (14, 3, 1));
            assert_eq!((h.zone.as_deref(), h.start.as_deref()), (Some("duskwood"), Some("slam")));
            let w = saves.class_progress(&content, "warrior");
            assert_eq!(w.talent_experience, 900);
            assert_eq!(w.talents["imp_heroic"], 2);
            assert_eq!(w.prestige["damage"], 1);
            assert!(w.zones_won.contains("elwynn"));
            assert_eq!(w.collection.len(), 2);
            assert_eq!(w.worn_pieces, [piece("head", "elwynn")]);
            assert_eq!(w.worn.len(), 1, "the worn piece numbers for the content");
            assert_eq!(w.looks["feet"], look("brm", 1), "a look's row is kept");
            assert_eq!(w.looks["head"], look("elwynn", 0));
            assert_eq!(w.loadout.as_deref(), Some("two_hander"));
            assert_eq!(saves.codex(), ["accuria"]);
            let o = saves.run_options();
            assert_eq!((o.density, o.damage), (120, 90));
            let discovered = saves.zones_discovered(&content);
            assert!(["elwynn", "duskwood"].iter().all(|z| discovered.iter().any(|d| d == z)), "{discovered:?}");
        };
        let saves = Saves::open(&path).unwrap();
        check(&saves);
        let w = saves.class_progress(&content, "warrior");
        assert_eq!(w.seen, Some(Seen { pieces: 2, points: w.points_earned() }), "seen added since");
        assert_eq!(w.unseen(), (false, false), "nothing of a v0.1.0 save is news");
        // written back in this version's form (a view opened writes), it loads the same again
        assert!(!saves.see("warrior", SeenView::Collection), "nothing new to see");
        assert!(saves.choose_zone(1, "elwynn"));
        let again = Saves::open(&path).unwrap();
        assert_eq!(again.heroes()[0].zone.as_deref(), Some("elwynn"));
        assert_eq!(again.class_progress(&content, "warrior"), saves.class_progress(&content, "warrior"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Challenge Mode opens once the class has won Ahn'Qiraj (kept with its zones won, as every
    /// zone's clear is), and only then does a picked Challenge Mode tab make the next run a challenge
    /// run in any zone.
    #[test]
    fn challenge_mode_opens_with_ahn_qiraj_won() {
        use esurv_sim::game::challenge::CHALLENGE_ZONE;
        let content = content();
        let saves = Saves::in_memory();
        let hero = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        let last = content.zones.len() - 1;
        let aq = content.zones.iter().position(|z| z.id == CHALLENGE_ZONE).unwrap();
        let challenge = SavedRunOptions { challenge: true, challenge_pct: 200, ..SavedRunOptions::default() };
        saves.set_run_options(&content, challenge);
        // not yet: the tab is picked, but the class has not cleared Ahn'Qiraj
        assert!(!saves.challenge_open(&content, "mage") && !saves.runs_challenge(&content, "mage"));
        assert!(!saves.zone_open_for_next_run(&content, "mage", last));
        assert_eq!(saves.run_options_for(&content, "mage").challenge, None);
        // every zone but Ahn'Qiraj won: still closed
        let mut almost = FinishedRun { won: true, opens_zone: true, first: true, ..FinishedRun::default() };
        for i in (0..content.zones.len()).filter(|&i| i != aq) {
            almost.zone_index = i;
            saves.finish_run(&content, hero.guid, &almost).unwrap();
        }
        assert!(!saves.challenge_open(&content, "mage"));
        // Ahn'Qiraj cleared: open, for this class only
        almost.zone_index = aq;
        saves.finish_run(&content, hero.guid, &almost).unwrap();
        assert!(saves.challenge_open(&content, "mage") && !saves.challenge_open(&content, "warrior"));
        assert!(saves.runs_challenge(&content, "mage"));
        let o = saves.run_options_for(&content, "mage").challenge.expect("a challenge run");
        assert_eq!(o.difficulty, content.zones[aq].difficulty);
        assert!((o.enemy_factor - 1.1 * 2.0).abs() < 1e-6);
        assert!((0..content.zones.len()).all(|i| saves.zone_open_for_next_run(&content, "mage", i)));
        // the Normal Mode tab: a normal run
        saves.set_run_options(&content, SavedRunOptions { challenge: false, ..challenge });
        assert_eq!(saves.run_options_for(&content, "mage").challenge, None);
        assert_eq!(saves.run_options().challenge_pct, 200, "the slider is remembered");
        // the class's progress tells the client the tabs are open
        let esurv_protocol::ToClient::Progress(p) = crate::prerun::progress(&content, &saves, "mage") else { panic!("progress") };
        assert!(p.challenge_open);
    }

    /// The box telling that Challenge Mode is open shows once per save: due once a class
    /// has cleared Ahn'Qiraj, gone for good once closed, even when another class clears it later
    /// and after a restart.
    #[test]
    fn the_challenge_mode_box_shows_once_per_save() {
        use esurv_protocol::SeenView;
        use esurv_sim::game::challenge::CHALLENGE_ZONE;
        let content = content();
        let dir = std::env::temp_dir().join(format!("esurv-challenge-box-{}", std::process::id()));
        let path = dir.join("heroes.ron");
        let _ = std::fs::remove_file(&path);
        let saves = Saves::open(&path).unwrap();
        let ana = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        let bo = saves.create("Bo", "warrior", "human", 0, [0; 5]).unwrap();
        let aq = content.zones.iter().position(|z| z.id == CHALLENGE_ZONE).unwrap();
        let clear = FinishedRun { won: true, opens_zone: true, first: true, zone_index: aq, ..FinishedRun::default() };
        let news = |class: &str| {
            let esurv_protocol::ToClient::Progress(p) = crate::prerun::progress(&content, &saves, class) else { panic!("progress") };
            p.challenge_news
        };
        assert!(!news("mage"), "nothing before Ahn'Qiraj");
        saves.finish_run(&content, ana.guid, &clear).unwrap();
        assert!(news("mage") && news("warrior"), "due, for the whole save");
        assert!(saves.see("mage", SeenView::Challenge));
        assert!(!news("mage") && !news("warrior"), "closed once");
        saves.finish_run(&content, bo.guid, &clear).unwrap();
        assert!(!news("warrior"), "a second class clearing it shows nothing");
        drop(saves);
        let reopened = Saves::open(&path).unwrap();
        assert!(!reopened.challenge_news(&content), "kept across a restart");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
