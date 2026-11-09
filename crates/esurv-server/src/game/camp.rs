//! The warband's camp as a place to walk around in: the hub (on unless `ESURV_HUB=0`,
//! plans/hub-concept.md and plans/hub-layout.md). In place of the warband
//! screen's menu, the hero logs into its faction's camp, a spot of the real world, and walks it
//! the WoW way. Things in the camp do what the menu's buttons did: the war table picks the zone
//! and starts the run, the weapon rack opens the hero's equipment, and the other heroes of the
//! faction, each at its own tent, talk in the WoW talk window and can be taken over.
//!
//! The camp is a run that never starts: a zone of its own (no enemies, no bosses) laid at the
//! camp's spot, its hero standing at its own tent, the others standing at theirs. Moves go
//! nowhere near the simulation; the camp keeps the hero inside its edge itself.
//!
//! The camp is the neutral camp at the Caverns of Time in Tanaris (camp/neutral.rs), each faction's
//! heroes in its own half; the hubs there are carried over from the faction camps that stood in
//! Dun Morogh and on the Merchant Coast (camp/horde.rs) before it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

use esurv_content::{Content, Faction, Spawn, Weather, WeatherKind, Zone};
use esurv_protocol::ToClient;

use super::pickups::{gameobject_guid, PICKUP_FACTION};
use super::{player_guid, Game};
use crate::entries::{CAMP_KEEPERS_FROM, CAMP_PIECES_FROM, CAMP_SERVICES_FROM};
use esurv_content::camp_looks::{
    camp_thing_look, CAMP_ARMOR_STAND_LOOK, CAMP_BONFIRE_DISPLAY, CAMP_GOL_BOLAR_PLATFORM_DISPLAY, CAMP_OBJECT_DISPLAYS,
    CAMP_LIBRARY_LARGE_SHELF_DISPLAY, CAMP_LIBRARY_SMALL_SHELF_DISPLAY, CAMP_LIBRARY_TABLE_DISPLAY, CAMP_TENT_DISPLAY,
    CAMP_HORDE_TENT_DISPLAY, CAMP_TAUREN_RACK_MODEL, CAMP_TIKI_MASK_MODEL,
    CAMP_GNOME_TABLE_DISPLAY, CAMP_MARSHAL_LOOK, CAMP_WREN_LOOK, CAMP_ORB_KEEPER_LOOK, CAMP_VENDOR_TENT_LOOK, CAMP_WAR_MAP_TABLE_MODEL, CAMP_WAR_HELM_DISPLAY, CAMP_WAR_PAVILION_LOOK,
};
use crate::packets::{self, GameObjectInfo};
use crate::save::SavedHero;
use crate::update::{field, Fields, ObjectTypeId, ObjectUpdate, TYPE_DYNAMICOBJECT, TYPE_GAMEOBJECT, TYPE_OBJECT};
use crate::wire::Packet;
use crate::wmo_floor::WmoFloors;

mod anachronos;
mod bronze;
mod horde;
mod life;
mod neutral;
mod tour;
mod walk;
use horde::{horde_held_by, horde_who, horde_words};
use neutral::{NEUTRAL_ALLIANCE_LAYOUT, NEUTRAL_HORDE_LAYOUT};
use life::{Kit, Life, INVTYPE_2HWEAPON, INVTYPE_SHIELD, MAIN_HAND, OFF_HAND};
use tour::{before_chromie, walk_from, Did, Leg, Tour, TourChoice};
use walk::Doing;

/// The camp's zone id in the camp's own content (never one of the content's zones).
pub const CAMP_ZONE: &str = "camp";

/// A spot of a camp's layout: yards east and north of the anchor spot, and the compass bearing it
/// faces (0 north, 90 east).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub east: f32,
    pub north: f32,
    pub faces: f32,
}

const fn spot(east: f32, north: f32, faces: f32) -> Spot {
    Spot { east, north, faces }
}

/// A thing of the camp to use, and the window of the game's own it opens (never
/// a talk window). The services sit in small hubs by theme: the Muster (the war table under its
/// command tent; the signpost removed, the guide is Wren's to come), the
/// Armoury (rack, orb, trainer, under a gypsy wagon's awning), the Records (the lectern; the
/// Chronicler's library on its platform), the Market.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    WarTable,
    WeaponRack,
    /// The appearance window: the Scholomance crystal ball.
    TransmutationOrb,
    Lectern,
    // the retired notice board's entry (4), the signpost's (5) and the camp banner post's (6, to
    // the other faction's camp, gone with the faction camps) are never reused: the client
    // remembers a creature entry's name, so a thing never takes another's
    /// The neutral camp's Bronze portal, the Caverns of Time's own, a fifth its size: the zones, in
    /// the War Table's stead.
    BronzePortal = 7,
}

impl Service {
    const ALL: [Service; 5] = [Service::WarTable, Service::WeaponRack, Service::TransmutationOrb, Service::Lectern, Service::BronzePortal];

    /// Its creature entry in `faction`'s camp: each camp's things of their own, as the client
    /// remembers an entry's look.
    fn entry(self, faction: Faction) -> u32 {
        CAMP_SERVICES_FROM + camp_entries_from(faction) + self as u32
    }

    fn of_entry(entry: u32) -> Option<(Self, Faction)> {
        [Faction::Alliance, Faction::Horde]
            .into_iter()
            .find_map(|f| Self::ALL.into_iter().find(|s| s.entry(f) == entry).map(|s| (s, f)))
    }

    /// Its name: the Horde's appearance and codex are the Voodoo Mask and the Lore Tablet
    /// (plans/hub-decisions.md 5).
    fn name(self, faction: Faction) -> &'static str {
        match (self, faction) {
            (Service::WarTable, _) => "War Table",
            (Service::WeaponRack, _) => "Weapon Rack",
            (Service::TransmutationOrb, Faction::Alliance) => "Transmutation Orb",
            (Service::TransmutationOrb, Faction::Horde) => "Voodoo Mask",
            (Service::Lectern, Faction::Alliance) => "Lectern",
            (Service::Lectern, Faction::Horde) => "Lore Tablet",
            (Service::BronzePortal, _) => "Bronze Portal",
        }
    }

    /// Its look (`GameObjectDisplayInfo.dbc`) and size. The 1.12 objects have no lectern: a dwarven
    /// reading table stands in for it.
    const fn look(self, faction: Faction) -> (u32, f32) {
        match (self, faction) {
            (Service::WarTable, Faction::Alliance) => (CAMP_WAR_MAP_TABLE_MODEL, WAR_TABLE_SIZE), // AllianceMapTable, shrunk
            (Service::WeaponRack, Faction::Alliance) => (130, 1.0),   // GeneralWeaponrack01
            (Service::TransmutationOrb, Faction::Alliance) => (4891, 0.85), // ScholomanceCrystalBall01, 15% smaller
            (Service::Lectern, Faction::Alliance) => (3371, 0.5),     // ReadingTable01 (stand-in), half as tall
            // the Horde's, at the sizes of plans/hub-horde-camp.md 3.3 to 3.5
            (Service::WarTable, Faction::Horde) => (6605, 0.45), // HordeMapTable
            (Service::WeaponRack, Faction::Horde) => (CAMP_TAUREN_RACK_MODEL, 0.9), // Tauren_WeaponRack01
            (Service::TransmutationOrb, Faction::Horde) => (CAMP_TIKI_MASK_MODEL, 1.1), // Troll_tikimask01
            (Service::Lectern, Faction::Horde) => (5, 0.4), // TrollTablet, 20% smaller
            (Service::BronzePortal, _) => (4713, 0.2), // COT_portal01, a fifth its size
        }
    }

    /// The window it opens (`ToClient::CampOpen`).
    fn window(self) -> &'static str {
        match self {
            Service::WarTable | Service::BronzePortal => "zones",
            Service::WeaponRack => "equipment",
            Service::TransmutationOrb => "appearance",
            Service::Lectern => "codex",
        }
    }

    /// The line under its name, as a person of the camp's: what it opens.
    fn subtitle(self, faction: Faction) -> &'static str {
        match (self, faction) {
            (Service::WarTable | Service::BronzePortal, _) => "Zones",
            (Service::WeaponRack, _) => "Equipment",
            (Service::TransmutationOrb, _) => "Appearance",
            (Service::Lectern, _) => "Codex",
        }
    }

    /// Its unit's guid: one of each in a camp.
    fn guid(self, faction: Faction) -> u64 {
        super::unit_guid(self.entry(faction), 1)
    }
}

/// How far past the Alliance's a faction's camp things and people are numbered: the Horde's 50
/// on (the people's 50 to 99 never used before).
const fn camp_entries_from(faction: Faction) -> u32 {
    match faction {
        Faction::Alliance => 0,
        Faction::Horde => 50,
    }
}

/// A person of the camp: a WoW talk window whose option opens the game's own window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keeper {
    /// The class trainer, Thurgen Ironbrow: talents.
    Trainer,
    /// The merchant by the cart, Maribel Thorne: the shop.
    Merchant,
    /// The Chronicler, Tobbin Quillsprocket, sat on the table of his library on the platform: the
    /// achievements (in the notice board's stead).
    Chronicler,
    /// The Orb's keeper, a gnome woman standing on her table by it: words only, the Orb keeps its
    /// own menu.
    OrbKeeper,
    /// The War Table's Marshal, Marshal Corwin Hale, behind it: words only, the table keeps its
    /// own menu.
    Marshal,
    /// The camp planner, Lorendir Nightbough, in the Marshal's tent: turns the placing mode on and
    /// off, the decoration spots' gold rings shown only while it is on.
    Planner,
    /// The camp's runner, Wren Ashcombe, by the fire: walks a new hero round the camp, and after
    /// opens the Guide (plans/hub-wren-tour-draft.md).
    Wren,
}

impl Keeper {
    /// Its creature entry. The people were first named by their jobs ("Merchant", "Chronicler",
    /// "Class Trainer" in a look per class: entries 0, 1 and 100 on); named since, each takes an entry of its own, as the client remembers an entry's name.
    /// The Horde camp's people are numbered 50 past the Alliance's ([`camp_entries_from`]).
    fn entry(self, faction: Faction) -> u32 {
        CAMP_KEEPERS_FROM
            + camp_entries_from(faction)
            + match self {
                Keeper::OrbKeeper => 2,
                Keeper::Merchant => 3,
                Keeper::Chronicler => 4,
                Keeper::Trainer => 5,
                Keeper::Marshal => 6,
                Keeper::Planner => 7,
                Keeper::Wren => 8,
            }
    }

    /// Its name, the job under it (their own name, the job as the subtitle), and
    /// its look.
    fn who(self, faction: Faction) -> Option<(&'static str, &'static str, u32)> {
        Some(match faction {
            Faction::Alliance => match self {
                Keeper::Merchant => ("Maribel Thorne", "Merchant", MERCHANT_DISPLAY),
                Keeper::Chronicler => ("Tobbin Quillsprocket", "Achievements", CHRONICLER_DISPLAY),
                Keeper::OrbKeeper => ("Zazzi Glimerspell", "Orb Keeper", CAMP_ORB_KEEPER_LOOK),
                Keeper::Trainer => ("Thurgen Ironbrow", "Class Trainer", TRAINER_DISPLAY),
                // a rank, his name as the town marshals' (Marshal Dughan): no subtitle
                Keeper::Marshal => ("Marshal Corwin Hale", "", CAMP_MARSHAL_LOOK),
                // the creative advisor's name (a male night elf, <Camp Planner>)
                Keeper::Planner => ("Lorendir Nightbough", "Camp Planner", PLANNER_DISPLAY),
                Keeper::Wren => ("Wren Ashcombe", "Guide", CAMP_WREN_LOOK),
            },
            Faction::Horde => horde_who(self)?,
        })
    }
}

const ALL_KEEPERS: [Keeper; 7] = [Keeper::Trainer, Keeper::Merchant, Keeper::Chronicler, Keeper::OrbKeeper, Keeper::Marshal, Keeper::Planner, Keeper::Wren];



/// The camp planner's look: a night elf man of Darnassus, bareheaded, in plain clothes.
const PLANNER_DISPLAY: u32 = 1285;

/// The trainer's one look, whatever the hero's class: Granis Swiftaxe's, the Kharanos warrior trainer.
const TRAINER_DISPLAY: u32 = 3431;

/// The merchant's look: a woman of Stormwind in a vendor's dress (a female
/// human), behind the Market tent's counter.
const MERCHANT_DISPLAY: u32 = 1470;

/// The Chronicler's look: a gnome man, bareheaded, his hair the second of the gnome men's styles
/// (a male gnome with windswept hair).
const CHRONICLER_DISPLAY: u32 = 3104;

/// The Orb's keeper's staff, held in hand: the Magician Staff, the purple
/// jeweled staff (Stave_2H_Jeweled_B_01, item look 28578).
const ORB_KEEPER_STAFF: esurv_content::CreatureWeapons = esurv_content::CreatureWeapons {
    main_hand: Some(esurv_content::CreatureWeapon {
        kind: esurv_content::WeaponKind::Staff,
        display: 28578,
        inventory_type: 17,
        sheath: 2,
        material: 2,
    }),
    off_hand: None,
    ranged: None,
};

/// The Marshal's sword, held in hand: the Grand Marshal's Claymore, the rank 14 warrior's
/// two-handed sword (Sword_2H_PVPAlliance_A_01, item look 31996).
const MARSHAL_SWORD: esurv_content::CreatureWeapons = esurv_content::CreatureWeapons {
    main_hand: Some(esurv_content::CreatureWeapon {
        kind: esurv_content::WeaponKind::TwoHandedSword,
        display: 31996,
        inventory_type: 17,
        sheath: 1,
        material: 1,
    }),
    off_hand: None,
    ranged: None,
};

/// The camp planner's Carpenter's Mallet (item 1415), in his main hand.
const PLANNER_MALLET: esurv_content::CreatureWeapons = esurv_content::CreatureWeapons {
    main_hand: Some(esurv_content::CreatureWeapon {
        kind: esurv_content::WeaponKind::OneHandedMace,
        display: 19613,
        inventory_type: 21,
        sheath: 3,
        material: 2,
    }),
    off_hand: None,
    ranged: None,
};

/// What a camp person says as its talk window opens: the creative advisor's lines
/// (plans/hub-people-and-tutorial.md; the Horde's, plans/hub-horde-layout.md).
fn keeper_words(faction: Faction, keeper: Keeper) -> &'static str {
    match (faction, keeper) {
        (Faction::Horde, k) => horde_words(k),
        (_, Keeper::Trainer) => "Every blow you strike out there teaches you something. Sit, and we'll put it to use.",
        (_, Keeper::Merchant) => "Fresh off the road, and I only carried it this far because I knew you'd want it. Have a look.",
        (_, Keeper::Chronicler) => {
            "Every deed done out there ends up in these books sooner or later. Yours has a page already. Shall we see what's on it?"
        }
        (_, Keeper::Planner) => {
            "Elune light your path, friend. I have laid out more camps than I can count, and I can show you where every piece of this one ought to go."
        }
        (_, Keeper::OrbKeeper) => "Hands off the Orb! Oh, go on, but gently.",
        (_, Keeper::Marshal) => "Pick your ground. I'll see you have what you need to hold it.",
        (_, Keeper::Wren) => "",
    }
}

/// What a camp person holds in hand, if anything.
fn held_by(faction: Faction, keeper: Keeper) -> Option<&'static esurv_content::CreatureWeapons> {
    match (faction, keeper) {
        (Faction::Alliance, Keeper::OrbKeeper) => Some(&ORB_KEEPER_STAFF),
        (Faction::Alliance, Keeper::Marshal) => Some(&MARSHAL_SWORD),
        (Faction::Alliance, Keeper::Planner) => Some(&PLANNER_MALLET),
        (Faction::Horde, k) => horde_held_by(k),
        _ => None,
    }
}

/// What the creature query hears of a camp person's entry.
pub fn camp_creature_info(entry: u32) -> Option<crate::packets::CreatureInfo> {
    // a thing to use: a unit wearing its object's model, named for it, its window under its name
    if let Some((s, faction)) = Service::of_entry(entry) {
        return Some(crate::packets::CreatureInfo {
            name: s.name(faction).into(),
            subname: s.subtitle(faction).into(),
            creature_type: 7,
            rank: crate::packets::RANK_NORMAL,
            display_id: camp_thing_look(s.look(faction).0),
        });
    }
    if entry == CHROMIE_ENTRY {
        return Some(crate::packets::CreatureInfo {
            name: "Chromie".into(),
            subname: String::new(),
            creature_type: 2,
            rank: crate::packets::RANK_NORMAL,
            display_id: CHROMIE_LOOK,
        });
    }
    if entry == ANACHRONOS_ENTRY {
        return Some(crate::packets::CreatureInfo {
            name: "Anachronos".into(),
            subname: String::new(),
            creature_type: 2,
            rank: crate::packets::RANK_NORMAL,
            display_id: ANACHRONOS_LOOK,
        });
    }
    if let Some(info) = bronze::creature_info(entry) {
        return Some(info);
    }
    if entry == ORB_SUBJECT_ENTRY {
        return Some(crate::packets::CreatureInfo {
            name: "Boar".into(),
            subname: String::new(),
            creature_type: 1,
            rank: crate::packets::RANK_NORMAL,
            display_id: ORB_SUBJECT_LOOK,
        });
    }
    let (name, subname, display) = [Faction::Alliance, Faction::Horde]
        .into_iter()
        .find_map(|f| ALL_KEEPERS.into_iter().find(|k| k.entry(f) == entry)?.who(f))?;
    Some(crate::packets::CreatureInfo {
        name: name.into(),
        subname: subname.into(),
        creature_type: 7,
        rank: crate::packets::RANK_NORMAL,
        display_id: display,
    })
}

/// What a decoration spot suits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoKind {
    /// Round the fire: any camp piece.
    Any,
    Seat,
    Supplies,
}

/// Where the things of a camp stand, planned on the overhead photo of its ground
/// (plans/hub-layout-photo.md, the creative advisor's): spread out, an improvised
/// camp, no edge markers, the services in small hubs by theme).
#[derive(Debug)]
pub struct CampLayout {
    pub fire: Spot,
    /// The fire's piece and the tent's (each lying on the ground), at their spots' origin.
    pub fire_piece: &'static Prop,
    pub tent: &'static Prop,
    pub services: &'static [(Service, Spot)],
    pub keepers: &'static [(Keeper, Spot)],
    /// The people of the camp sat on something rather than standing on the ground.
    pub seats: &'static [(Keeper, SetOn)],
    /// The people of the camp standing up on something rather than on the ground.
    pub stands_on: &'static [(Keeper, SetOn)],
    /// The things to use standing up on something rather than on the ground.
    pub services_on: &'static [(Service, SetOn)],
    /// The pieces that belong to a hub but are no thing to use.
    pub props: &'static [Prop],
    /// Each plot's middle and the way its door faces, filled in this order: a fixed patch sized for
    /// the biggest tent it can be upgraded to; the tent stands [`TENT_AHEAD`] before the middle, its
    /// hero [`HERO_OUT`] before it.
    pub plots: &'static [Spot],
    /// Each plot's tent turned this far about its own middle, degrees (the Horde's basic tents,
    /// each by a quarter, a half or three quarters, picked at random once).
    pub tent_turns: &'static [f32],
    /// The three banner posts.
    pub banners: [Spot; 3],
    /// The dragon head (Blackrock Mountain's trophy).
    pub head: Spot,
    /// The other zones' trophies, in the zones' order (Blackrock Mountain's is the head).
    pub trophies: [Spot; 9],
    /// The camp's shared decoration spots.
    pub decorations: &'static [(DecoKind, Spot)],
    /// Each plot's own decoration spots: yards ahead of its middle (the way its door faces) and to
    /// its left ([`CampLayout::plot_decoration_spots`]).
    pub plot_decorations: [(f32, f32); 3],
    /// Where the other heroes walk to from their tents and what they do there, each spot facing
    /// the thing, and how high above the ground it is there (on a log, sitting) (plans/hub-hero-life.md,
    /// section 4).
    pub haunts: &'static [(Doing, Spot, f32)],
    /// The animal the Orb's keeper polymorphs into a person of another race and back, standing
    /// before her (the Horde's boar); none: she polymorphs herself.
    pub orb_subject: Option<Spot>,
    /// Where Chromie sits and what on, the neutral camp's; none in a faction's own
    /// camp.
    pub chromie: Option<(Spot, SetOn)>,
    /// Where Anachronos stands, the neutral camp's at the cave mouth; none in a
    /// faction's own camp.
    pub anachronos: Option<Spot>,
    /// How far from the camp's middle the hero may walk, yards: past the farthest thing of the camp.
    pub reach: f32,
    /// The middle of the camp's edge: the fire's spot, but the neutral camp's the anchor spot,
    /// its circle's, with the fire moved off it.
    pub middle: Spot,
}

/// The camp's decoration spots by id: the shared ones by the fire (`fire_1` ...), then each
/// tent's three (`tent_<plot>_<1..3>`).
fn decoration_spots(place: CampPlace) -> Vec<(String, Spot)> {
    let layout = place.layout;
    let shared = layout.decorations.iter().enumerate().map(|(i, (_, s))| (format!("fire_{}", i + 1), *s));
    let tents = (0..layout.plots.len())
        .flat_map(|p| layout.plot_decoration_spots(p).into_iter().enumerate().map(move |(k, s)| (format!("tent_{}_{}", p + 1, k + 1), s)));
    shared.chain(tents).collect()
}

/// A tent's decoration spot `tent_<plot>_<k>`: its plot (from 0) and its spot (1 to 3).
fn tent_spot(id: &str) -> Option<(usize, usize)> {
    let (p, k) = id.strip_prefix("tent_")?.split_once('_')?;
    Some((p.parse::<usize>().ok()?.checked_sub(1)?, k.parse().ok()?))
}

/// The decorations' objects are numbered past the camp's own pieces.
const DECORATION_PIECES_FROM: usize = 10_000;

impl CampLayout {
    /// Plot `i`'s own decoration spots, each facing as its door does (empty past the last plot).
    pub fn plot_decoration_spots(&self, i: usize) -> Vec<Spot> {
        let Some(plot) = self.plots.get(i) else {
            return Vec::new();
        };
        let (s, c) = plot.faces.to_radians().sin_cos();
        self.plot_decorations
            .iter()
            .map(|&(ahead, left)| Spot { east: plot.east + ahead * s - left * c, north: plot.north + ahead * c + left * s, ..*plot })
            .collect()
    }
}

/// Where a faction's camp is: the anchor spot and the camp laid out round it.
#[derive(Clone, Copy, Debug)]
pub struct CampPlace {
    pub map: u32,
    pub area: u32,
    /// The anchor spot, world (x, y, z): the layout's (0, 0).
    pub spot: (f32, f32, f32),
    pub layout: &'static CampLayout,
    /// Its word in the save's decorations (`save::cosmetics::CAMPS`).
    pub camp: &'static str,
    /// Whose camp it is: its people and things are that faction's.
    pub faction: Faction,
}

/// The neutral camp, the only camp:
/// Tanaris' Caverns of Time, the anchor spot its middle, each side's half laid out for its heroes
/// (`neutral`). Each side's decorations are saved under its own word, as when each had a camp of
/// its own.
pub const NEUTRAL_ALLIANCE_CAMP: CampPlace = CampPlace {
    map: 1,
    area: 440,
    spot: (-8206.09, -4588.25, 8.86),
    layout: &NEUTRAL_ALLIANCE_LAYOUT,
    camp: "alliance",
    faction: Faction::Alliance,
};
pub const NEUTRAL_HORDE_CAMP: CampPlace = CampPlace { layout: &NEUTRAL_HORDE_LAYOUT, camp: "horde", faction: Faction::Horde, ..NEUTRAL_ALLIANCE_CAMP };

/// Gol'Bolar's platform east of the camp (its origin, the foot of its stairs), and how high its
/// deck stands over the ground there: 3.2 yards up at full size (model_sizes), at three tenths
/// 0.96, the platform a quarter of a yard under its stairs' foot.
const PLATFORM: Spot = spot(36.8, 17.0, 160.0);
const PLATFORM_SUNK: f32 = 0.25;
const PLATFORM_DECK: f32 = 0.96 - PLATFORM_SUNK;
/// The library's table's top over its feet (DwarvenTableAverage02, model_sizes).
const LIBRARY_TABLE_TOP: f32 = 0.89;
/// The War Table (the Alliance map table) and its map's top over its feet
/// (model_sizes; plans/hub-war-tent.md), with its whole tent moved west of the fire onto the
/// flatter ground there, the tent centred on the anchor spot (3.7, 11.6).
const WAR_TABLE: Spot = spot(3.7, 8.6, 180.0);
const WAR_TABLE_SIZE: f32 = 0.8;
const WAR_TABLE_TOP: f32 = 1.29 * WAR_TABLE_SIZE;
/// The Market's counter: the middle between its two barrels (the vendor tent's feet, 4.48 ahead
/// and a yard to each side of its origin), and its plank's top over the ground there.
const COUNTER: Spot = spot(1.1, 26.9, 156.0);
const COUNTER_TOP: f32 = 1.0;
/// The Market's crate stack at the tent's left side.
const MARKET_CRATES: Spot = spot(4.0, 30.9, 156.0);
/// The Orb's keeper's table (GnomeTable01, the purple cloth), on the anchor spot, turned a
/// quarter and sunk a little into the snow, and its top (model_sizes).
const GNOME_TABLE: Spot = spot(21.8, 33.7, 280.0);
const GNOME_TABLE_SUNK: f32 = 0.08;
const GNOME_TABLE_TOP: f32 = 1.1 - GNOME_TABLE_SUNK;
/// The Alliance's hubs as they stood in its own camp in Dun Morogh, its spot there the layout's
/// (0, 0): the neutral camp carries each over whole ([`neutral`]), its pieces in this order, hub
/// by hub. Planned to true size on the photo (plans/hub-services.md), in themed hubs.
const ALLIANCE_HUB_PIECES: [Prop; 51] = [Prop {
        // the Armoury's gypsy wagon (StormwindGypsywagon01) behind the trainer, its red awning
        // (the model's left side, 2.6 to 4.5 yards out) over the trainer and the weapon rack, on the even ground south-west of the pines (within a yard over its length)
        look: PropLook::Object(5392),
        scale: 1.0,
        spot: spot(31.4, 28.0, 310.0),
        // its wheels, the shafts' tips and the awning's poles (model_sizes: x -4.57..6.91, y
        // -2.61..4.54)
        footprint: Some(Footprint { x: (-4.4, 6.7), y: (-2.5, 4.4) }),
        set_on: None,
    }, Prop {
        // the armor stand beside the weapon rack, toward the player
        look: PropLook::Camp(CAMP_ARMOR_STAND_LOOK),
        scale: 1.1,
        spot: spot(33.0, 22.1, 255.0),
        footprint: None,
        set_on: None,
    }, Prop {
        // the smith's things (in place of the brazier): a tradeskill anvil beside
        // the trainer under the awning, close by the wagon, along it, toward its
        // front ...
        look: PropLook::Object(273), // Tradeskill_Anvil_01
        scale: 1.0,
        spot: spot(28.97, 26.51, 310.0),
        footprint: None,
        set_on: None,
    }, Prop {
        // ... and a tradeskill forge, its middle on the anchor spot (26.3, 28.9) by the wagon's
        // front, turned about its middle to face the wagon (its origin at its
        // corner: it reaches 2.1 yards ahead and 1.2 right to 2.35 left,
        // model_sizes)
        look: PropLook::Object(233), // Tradeskill_Forge_01
        scale: 1.0,
        spot: spot(26.04, 27.73, 40.0),
        footprint: None,
        set_on: None,
    }, Prop {
        // the Orb's keeper's gnome table, the purple cloth, her standing on it
        look: PropLook::Object(CAMP_GNOME_TABLE_DISPLAY),
        scale: 1.0,
        spot: GNOME_TABLE,
        footprint: None,
        set_on: Some(SetOn { lift: -GNOME_TABLE_SUNK, rise_degrees: 0.0, ground_at: None }),
    }, Prop {
        // an open book on the lectern, BookMediumOpen01: on its desk's middle
        // (the reading table's top at half size: 1.17 yards up, rising 20 degrees away from the
        // reader, model_sizes), facing as the lectern does
        look: PropLook::Object(254),
        scale: 1.0,
        spot: spot(35.6, 14.6, 69.0),
        footprint: None,
        set_on: Some(SetOn { lift: 1.19, rise_degrees: 20.0, ground_at: None }),
    }, Prop {
        // the Market's vendor tent (StormwindVendorTent01), its counter on its front toward the
        // camp; the merchant stands behind the counter
        look: PropLook::Camp(CAMP_VENDOR_TENT_LOOK),
        scale: 1.0,
        spot: spot(-0.7, 31.0, 156.0),
        footprint: None,
        set_on: None,
    }, Prop {
        // the Market dressed (plans/hub-dressing-and-light.md, goods on the
        // counter, crates, sacks and a barrel, no hanging wares, no sign): on the counter's left
        // half, away from the merchant, a bowl of fruit, a loaf, two spools of cloth and a
        // standing lantern; bottles and a jar at its right end by her hand; all on its plank (3.5
        // yards long, 4.0 to 4.9 ahead of the tent's origin, model_sizes' slice) ...
        look: PropLook::Object(381), // FruitBowl_Mixed
        scale: 0.45,
        spot: spot(1.66, 27.18, 156.0),
        footprint: None,
        set_on: Some(SetOn { lift: COUNTER_TOP, rise_degrees: 0.0, ground_at: Some(COUNTER) }),
    }, Prop {
        look: PropLook::Object(229), // bread01
        scale: 0.45,
        spot: spot(1.02, 26.89, 66.0),
        footprint: None,
        set_on: Some(SetOn { lift: COUNTER_TOP, rise_degrees: 0.0, ground_at: Some(COUNTER) }),
    }, Prop {
        look: PropLook::Object(1747), // ClothSpoolYellow
        scale: 2.0,
        spot: spot(2.15, 27.56, 156.0),
        footprint: None,
        set_on: Some(SetOn { lift: COUNTER_TOP, rise_degrees: 0.0, ground_at: Some(COUNTER) }),
    }, Prop {
        look: PropLook::Object(4071), // ClothSpoolRedLow
        scale: 2.0,
        spot: spot(2.31, 27.31, 120.0),
        footprint: None,
        set_on: Some(SetOn { lift: COUNTER_TOP, rise_degrees: 0.0, ground_at: Some(COUNTER) }),
    }, Prop {
        look: PropLook::Object(6038), // GeneralLantern01
        scale: 0.6,
        spot: spot(2.53, 27.57, 156.0),
        footprint: None,
        set_on: Some(SetOn { lift: COUNTER_TOP, rise_degrees: 0.0, ground_at: Some(COUNTER) }),
    }, Prop {
        look: PropLook::Object(225), // Bottle01
        scale: 1.0,
        spot: spot(-0.32, 26.46, 156.0),
        footprint: None,
        set_on: Some(SetOn { lift: COUNTER_TOP, rise_degrees: 0.0, ground_at: Some(COUNTER) }),
    }, Prop {
        look: PropLook::Object(225), // Bottle01
        scale: 1.0,
        spot: spot(-0.25, 26.17, 200.0),
        footprint: None,
        set_on: Some(SetOn { lift: COUNTER_TOP, rise_degrees: 0.0, ground_at: Some(COUNTER) }),
    }, Prop {
        look: PropLook::Object(297), // Jar01
        scale: 1.0,
        spot: spot(0.06, 26.47, 156.0),
        footprint: None,
        set_on: Some(SetOn { lift: COUNTER_TOP, rise_degrees: 0.0, ground_at: Some(COUNTER) }),
    }, Prop {
        // ... and by the tent's left side two Stormwind crates stacked, the top one turned a
        // little, a second lantern on them to light the way in from the fire, a low open crate
        // behind and a tied sack at their front; a barrel at the tent's right front corner
        look: PropLook::Object(31), // StormwindCrate01
        scale: 1.0,
        spot: MARKET_CRATES,
        footprint: None,
        set_on: None,
    }, Prop {
        look: PropLook::Object(31),
        scale: 0.85,
        spot: spot(4.0, 30.9, 170.0),
        footprint: None,
        set_on: Some(SetOn { lift: 1.24, rise_degrees: 0.0, ground_at: Some(MARKET_CRATES) }),
    }, Prop {
        look: PropLook::Object(6038), // GeneralLantern01
        scale: 0.8,
        spot: spot(4.0, 30.9, 156.0),
        footprint: None,
        set_on: Some(SetOn { lift: 1.24 + 1.05, rise_degrees: 0.0, ground_at: Some(MARKET_CRATES) }),
    }, Prop {
        look: PropLook::Object(275), // Crate01
        scale: 1.0,
        spot: spot(3.75, 32.3, 120.0),
        footprint: None,
        set_on: None,
    }, Prop {
        look: PropLook::Object(2890), // Sack02
        scale: 1.0,
        spot: spot(4.6, 29.9, 200.0),
        footprint: None,
        set_on: None,
    }, Prop {
        look: PropLook::Object(32), // Barrel01
        scale: 1.0,
        spot: spot(-3.3, 26.3, 156.0),
        footprint: None,
        set_on: None,
    }, Prop {
        // inside the tent: a blue Stormwind rug on the floor well back from the counter,
        // and supply crates by the walls at its back; the rug just over the snow, which is
        // bumpy there
        look: PropLook::Object(159), // StormwindRug01
        scale: 1.3,
        spot: spot(-0.86, 31.37, 156.0),
        footprint: Some(Footprint { x: (-1.47, 1.63), y: (-1.39, 1.42) }),
        set_on: Some(SetOn { lift: 0.1, rise_degrees: 0.0, ground_at: None }),
    }, Prop {
        look: PropLook::Object(31), // StormwindCrate01
        scale: 0.8,
        spot: spot(0.74, 32.19, 166.0),
        footprint: None,
        set_on: None,
    }, Prop {
        look: PropLook::Object(113), // Crate02
        scale: 1.0,
        spot: spot(-2.55, 30.72, 140.0),
        footprint: None,
        set_on: None,
    }, Prop {
        look: PropLook::Object(275), // Crate01
        scale: 1.0,
        spot: spot(-2.29, 29.42, 66.0),
        footprint: None,
        set_on: None,
    }, Prop {
        // Gol'Bolar's platform at three tenths of its size, level, its deck
        // east of the camp, swapped with tent 8 and brought forward from its old plot (51, 17)
        // onto the flatter ground at (39, 17), its stairs running down toward
        // the middle of the tents' circle (bearing 250): they are off its right side, so it faces
        // 90 degrees less, and the deck's middle (model_sizes: 2.5 ahead, 7 left, scaled) is that
        // point; a quarter of a yard under its stairs' foot on the ground
        look: PropLook::Object(CAMP_GOL_BOLAR_PLATFORM_DISPLAY),
        scale: 0.3,
        spot: PLATFORM,
        footprint: None,
        set_on: Some(SetOn { lift: -PLATFORM_SUNK, rise_degrees: 0.0, ground_at: None }),
    }, Prop {
        // the Chronicler's library on the platform's deck: Ironforge's large
        // bookshelf (DwarvenBookshelfLarge, 5.25 yards long, 1.92 deep, its length along its x)
        // along the deck's back edge, its shelves toward the camp, both shelves made 15% smaller
        // twice, 4.45 yards to the platform's left (moved back)
        // and 0.75 ahead of its origin ...
        look: PropLook::Object(CAMP_LIBRARY_LARGE_SHELF_DISPLAY),
        scale: 0.72,
        spot: spot(41.2, 17.8, 160.0),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(PLATFORM) }),
    }, Prop {
        // ... the small one (DwarvenBookshelfSmall) at a right angle to it, on the anchor spot
        // (40.2, 14.5) by the deck's front edge, then 0.6 yards further onto the deck ...
        look: PropLook::Object(CAMP_LIBRARY_SMALL_SHELF_DISPLAY),
        scale: 0.72,
        spot: spot(40.0, 15.1, 250.0),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(PLATFORM) }),
    }, Prop {
        // ... and the table the Chronicler sits on (DwarvenTableAverage02) before them, 2.2 yards
        // to the platform's left and 0.75 ahead of its origin
        look: PropLook::Object(CAMP_LIBRARY_TABLE_DISPLAY),
        scale: 1.0,
        spot: spot(39.1, 17.1, 250.0),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(PLATFORM) }),
    }, Prop {
        // the library dressed: on the table by the Chronicler's left hand an
        // unrolled scroll (ScrollB01) with a brown feather on it for his quill (FeatherBigBrown,
        // 1.12 has no quill), ...
        look: PropLook::Object(164),
        scale: 1.0,
        spot: spot(39.1, 16.6, 200.0),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK + LIBRARY_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(PLATFORM) }),
    }, Prop {
        look: PropLook::Object(2630),
        scale: 0.6,
        spot: spot(39.15, 16.65, 215.0),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK + LIBRARY_TABLE_TOP + 0.12, rise_degrees: 0.0, ground_at: Some(PLATFORM) }),
    }, Prop {
        // ... a short pile of books at his right (GeneralBookStackShort01) and a tall one behind
        // him (GeneralBookStackTall01), ...
        look: PropLook::Object(107),
        scale: 1.0,
        spot: spot(38.9, 17.65, 250.0),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK + LIBRARY_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(PLATFORM) }),
    }, Prop {
        look: PropLook::Object(43),
        scale: 1.0,
        spot: spot(39.6, 17.3, 230.0),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK + LIBRARY_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(PLATFORM) }),
    }, Prop {
        // ... and piles on the deck: two by the large bookshelf's end, one by the stairs' top
        look: PropLook::Object(43),
        scale: 1.0,
        spot: spot(39.4, 19.3, 250.0),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(PLATFORM) }),
    }, Prop {
        look: PropLook::Object(107),
        scale: 1.0,
        spot: spot(39.75, 18.6, 205.0),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(PLATFORM) }),
    }, Prop {
        look: PropLook::Object(107),
        scale: 1.0,
        spot: spot(38.0, 15.5, 290.0),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(PLATFORM) }),
    }, Prop {
        // the Records' light (plans/hub-dressing-and-light.md): a candle on the
        // table beside the scroll, where his quill hand is, and a standing lantern on the deck by
        // the top of the stairs, away from the book pile
        look: PropLook::Object(100), // Candle01
        scale: 1.0,
        spot: spot(39.5, 16.3, 250.0),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK + LIBRARY_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(PLATFORM) }),
    }, Prop {
        look: PropLook::Object(6038), // GeneralLantern01
        scale: 1.0,
        spot: spot(36.9, 18.7, 250.0),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(PLATFORM) }),
    }, Prop {
        // the Muster's command tent over the War Table (plans/hub-war-tent.md):
        // the dwarven dig pavilion, open all round, the size of the heroes' earned one;
        // its centre pole 3 yards behind the table, so the table stands against it
        look: PropLook::Camp(CAMP_WAR_PAVILION_LOOK),
        scale: 1.0,
        spot: spot(3.7, 11.6, 180.0),
        footprint: None,
        set_on: None,
    }, Prop {
        // a blue Stormwind rug under the table and the chairs, laid on the snow
        look: PropLook::Object(159), // StormwindRug01
        scale: 1.5,
        spot: spot(3.7, 8.9, 180.0),
        footprint: Some(Footprint { x: (-1.47, 1.63), y: (-1.39, 1.42) }),
        set_on: None,
    }, Prop {
        // two plain wooden chairs (GeneralChairLoEnd01) west and east of the
        // table, facing it, their seats a little back from its edges, to sit on, the
        // south side left open for the player; a crate and a crate of swords by the back poles
        look: PropLook::Object(39), // GeneralChairLoEnd01
        scale: 1.0,
        spot: spot(1.43, 8.6, 90.0),
        footprint: None,
        set_on: None,
    }, Prop {
        look: PropLook::Object(39), // GeneralChairLoEnd01
        scale: 1.0,
        spot: spot(5.97, 8.6, 270.0),
        footprint: None,
        set_on: None,
    }, Prop {
        look: PropLook::Object(275), // Crate01
        scale: 1.0,
        spot: spot(1.0, 12.2, 30.0),
        footprint: None,
        set_on: None,
    }, Prop {
        look: PropLook::Object(6708), // WeaponCrateAllianceSword
        scale: 1.0,
        spot: spot(6.6, 13.2, 180.0),
        footprint: None,
        set_on: None,
    }, Prop {
        // on the map table's map, all from the table's own ground: an officer's helm on its north-west corner (its origin at its crown, 0.31 over its
        // rim) ...
        look: PropLook::Object(CAMP_WAR_HELM_DISPLAY),
        scale: 1.0,
        spot: spot(3.14, 9.2, 200.0),
        footprint: None,
        set_on: Some(SetOn { lift: WAR_TABLE_TOP + 0.43, rise_degrees: 0.0, ground_at: Some(WAR_TABLE) }),
    }, Prop {
        // ... a sword planted point-down in the map: its blade along its length
        // (1.2 yards at 0.8), tipped front-down to near upright, its point a tenth into the map ...
        look: PropLook::Object(669), // HumanSword01
        scale: 0.8,
        spot: spot(4.1, 8.44, 145.0),
        footprint: None,
        set_on: Some(SetOn { lift: WAR_TABLE_TOP + 0.5, rise_degrees: -85.0, ground_at: Some(WAR_TABLE) }),
    }, Prop {
        // ... candles, a goblet and a rolled scroll round its edges
        look: PropLook::Object(6363), // Candle02, three candles
        scale: 1.0,
        spot: spot(4.38, 9.24, 180.0),
        footprint: None,
        set_on: Some(SetOn { lift: WAR_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(WAR_TABLE) }),
    }, Prop {
        look: PropLook::Object(100), // Candle01
        scale: 1.0,
        spot: spot(4.42, 7.88, 180.0),
        footprint: None,
        set_on: Some(SetOn { lift: WAR_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(WAR_TABLE) }),
    }, Prop {
        look: PropLook::Object(527), // Goblet02
        scale: 1.0,
        spot: spot(4.5, 8.76, 180.0),
        footprint: None,
        set_on: Some(SetOn { lift: WAR_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(WAR_TABLE) }),
    }, Prop {
        look: PropLook::Object(164), // ScrollB01
        scale: 1.0,
        spot: spot(2.98, 8.36, 160.0),
        footprint: None,
        set_on: Some(SetOn { lift: WAR_TABLE_TOP + 0.05, rise_degrees: 0.0, ground_at: Some(WAR_TABLE) }),
    }, Prop {
        // a small brazier inside, at half size, on the east side between the chair and the sword
        // crate, for light at night
        look: PropLook::Object(197), // DwarvenBrazier02
        scale: 0.5,
        spot: spot(6.4, 10.8, 0.0),
        footprint: None,
        set_on: None,
    }];

/// The camp of `faction`: its half of the neutral camp at the Caverns of Time.
pub fn camp_of(faction: Faction) -> Option<CampPlace> {
    Some(match faction {
        Faction::Alliance => NEUTRAL_ALLIANCE_CAMP,
        Faction::Horde => NEUTRAL_HORDE_CAMP,
    })
}

/// How far before its plot's middle a hero stands, yards: two yards clear of the front stakes of
/// the plot's biggest tent (plans/hub-tents.md).
const HERO_OUT: f32 = 7.0;

/// How far before its plot's middle the basic tent's own middle stands, yards, and its size: the
/// Defias tent's front end at the same place as the biggest tent's (plans/hub-tents.md).
const TENT_AHEAD: f32 = 2.3;
const TENT_SIZE: f32 = 0.7;
/// The basic tent lies on the ground under its guy ropes' stakes (HumanTentMedium's feet at size 1,
/// model_sizes: x -5.04..3.53, y -6.18..6.36), tilted to the slope, none of them in the air; an
/// object, so it is solid.
const TENT_ON_THE_GROUND: Prop = Prop {
    look: PropLook::Object(CAMP_TENT_DISPLAY),
    scale: TENT_SIZE,
    spot: spot(0.0, 0.0, 0.0),
    footprint: Some(Footprint { x: (-5.04, 3.53), y: (-6.18, 6.36) }),
    set_on: None,
};

/// The hour the camp is set to on the first visit: night, as the warband screen's camps.
#[cfg(test)]
const CAMP_HOUR: u32 = CAMP_HOURS[FIRST_HOUR_STEP];

/// The camp's hours: the first visit after the
/// game starts at night, each return from a run one step on, dusk, night, late night, dawn,
/// morning, afternoon and round again. The steps are the advisor's, flavour.
const CAMP_HOURS: [u32; 6] = [20, 22, 1, 6, 9, 15];
/// The step the first visit is at: 22:00.
const FIRST_HOUR_STEP: usize = 1;

/// The camp's hour after `runs_back` runs since the game started (`ESURV_CAMP_HOUR=<hour>`, a
/// development aid, sets it, to see the camp at that hour).
pub fn camp_hour(runs_back: usize) -> u32 {
    if let Some(hour) = std::env::var("ESURV_CAMP_HOUR").ok().and_then(|h| h.trim().parse::<u32>().ok()).filter(|h| *h < 24) {
        return hour;
    }
    CAMP_HOURS[(FIRST_HOUR_STEP + runs_back) % CAMP_HOURS.len()]
}

impl CampPlace {
    /// The world point of a spot of the layout.
    pub fn at(&self, s: Spot) -> (f32, f32) {
        // the world's x runs north, its y west
        (self.spot.0 + s.north, self.spot.1 - s.east)
    }

    /// The world facing of a spot of the layout (a compass bearing turned the world's way).
    pub fn facing(&self, s: Spot) -> f32 {
        (-s.faces.to_radians()).rem_euclid(std::f32::consts::TAU)
    }

    /// The camp fire's world point.
    pub fn fire(&self) -> (f32, f32) {
        self.at(self.layout.fire)
    }

    /// The world point of the middle of the camp's edge.
    pub fn middle(&self) -> (f32, f32) {
        self.at(self.layout.middle)
    }

    /// The world point and facing where the hero of plot `i` stands.
    pub fn hero_stand(&self, i: usize) -> Option<((f32, f32), f32)> {
        let tent = *self.layout.plots.get(i)?;
        let (s, c) = tent.faces.to_radians().sin_cos();
        let hero = Spot { east: tent.east + HERO_OUT * s, north: tent.north + HERO_OUT * c, ..tent };
        Some((self.at(hero), self.facing(hero)))
    }
}

/// The camp fire's look: a bonfire, Karazhan's second, a display of ours.
const FIRE: u32 = CAMP_BONFIRE_DISPLAY; // KarazahnBonFire02
/// The fire lies on the ground under its logs' ends (model_sizes: x -2.14..2.10, y -2.71..2.18),
/// tilted to the slope and none of them floating.
const FIRE_ON_THE_GROUND: Prop = Prop {
    look: PropLook::Object(FIRE),
    scale: 1.0,
    spot: spot(0.0, 0.0, 0.0),
    footprint: Some(Footprint { x: (-2.1, 2.1), y: (-2.7, 2.2) }),
    set_on: None,
};

/// The Horde camp's fire: the Orgrimmar gate bonfire (plans/hub-decisions.md 5), lying on the
/// ground under its hull's foot (model_sizes: x -2.42..2.17, y -2.52..2.50).
const HORDE_FIRE: u32 = 4572; // OrgrimmarBonFire01
const HORDE_FIRE_ON_THE_GROUND: Prop = Prop {
    look: PropLook::Object(HORDE_FIRE),
    scale: 1.0,
    spot: spot(0.0, 0.0, 0.0),
    footprint: Some(Footprint { x: (-2.4, 2.2), y: (-2.5, 2.5) }),
    set_on: None,
};

/// The Horde camp's tent: the Durotar hide tent (plans/hub-decisions.md 5) at its own size, on the ground under its hull (model_sizes: x -3.74..3.59, y
/// -3.50..3.18).
const HORDE_TENT_ON_THE_GROUND: Prop = Prop {
    look: PropLook::Object(CAMP_HORDE_TENT_DISPLAY),
    scale: 1.0,
    spot: spot(0.0, 0.0, 0.0),
    footprint: Some(Footprint { x: (-3.74, 3.59), y: (-3.5, 3.18) }),
    set_on: None,
};

/// Show the camp as a warband that has earned everything would have it: the three banners
/// planted, every trophy and the dragon head in its place, a piece on every decoration spot (the camp fully populated, to walk around in). Off, those spots stay bare until
/// the hub's decorating comes (phase 3).
const SHOW_EVERYTHING: bool = false;

/// The camp is built a round at a time, each planned on the photo first: the
/// tents stand, then the things to use and the people (the skeleton), then the decoration.
const SERVICES_PLACED: bool = true;

/// What the fully shown camp puts on its spots: the banners (the Stormpike-sized Alliance battle
/// banner, a dwarven banner, Stormwind's griffon), the zones' trophies in the zones' order
/// (Elwynn's mine cart, Duskwood's coffin lid, Warsong's sword in a skull, a Scarlet shield,
/// Sunken Temple's tiki head, Feralas' ogre head on a pike, the Arena's troll skull pile,
/// Stratholme's skull lights, Ahn'Qiraj's silithid gong), the dragon head (the Alliance's hanging
/// Nefarian trophy), and what stands on each kind of decoration spot, in turn. Each (display,
/// size).
const SHOWN_BANNERS: [(u32, f32); 3] = [(5771, 0.5), (630, 0.6), (624, 0.6)];
const SHOWN_TROPHIES: [(u32, f32); 9] =
    [(291, 0.8), (352, 1.0), (4174, 1.0), (58, 1.0), (653, 0.4), (3081, 0.6), (293, 0.8), (6417, 0.9), (4675, 0.5)];
const SHOWN_HEAD: (u32, f32) = (5952, 0.13);
const SHOWN_ANY: [(u32, f32); 4] = [(216, 0.65), (166, 0.6), (518, 0.8), (409, 0.35)]; // Cauldron, Anvil, StormWindWoodenDummy01, SmokedMeatRack01
const SHOWN_SEATS: [(u32, f32); 4] = [(622, 0.45), (2087, 0.5), (682, 0.45), (1569, 0.6)]; // StormwindBench01, DwarvenChair03, DuskwoodBench, TaurenLogChair02
const SHOWN_SUPPLIES: [(u32, f32); 8] =
    [(32, 1.0), (275, 1.0), (5511, 1.0), (1727, 0.75), (113, 1.0), (33, 1.0), (2890, 1.0), (289, 1.0)]; // Barrel01, Crate01, Sack01, BeerKeg01, Crate02, PowderKeg01, Sack02, FirewoodPile03

/// `GAMEOBJECT_TYPE_GENERIC`: every camp object only stands there (the things to use are people).
const GO_GENERIC: u32 = 5;
/// Nobody uses a piece that only stands there.
const GO_FLAG_NO_INTERACT: u32 = 0x10;
/// `GAMEOBJECT_TYPE_CHAIR`: a chair the hero sits on, right-clicked.
const GO_CHAIR: u32 = 7;
/// The camp's chairs to sit on: GeneralChairLoEnd01, the War Table's.
const CHAIR: u32 = 39;
/// Where on a chair the hero sits: yards behind its origin (its seat's front edge; model_sizes:
/// the chair runs 1.0 back from it), and its stand state there, `UNIT_STAND_STATE_SIT_MEDIUM_CHAIR`.
const CHAIR_SEAT_BACK: f32 = 0.4;
const STAND_SIT_CHAIR: u8 = 5;
/// `UNIT_NPC_FLAG_GOSSIP`: a hero at its tent and a camp person talk.
const NPC_GOSSIP: u32 = 1;
/// A camp person's faction: Ironforge's, friendly to the Alliance; Orgrimmar's, friendly to the
/// Horde.
const fn keeper_faction(faction: Faction) -> u32 {
    match faction {
        Faction::Alliance => 55,
        Faction::Horde => 29,
    }
}
/// The other side's heroes' faction: friendly to everyone (the pickups'), no foe to the hero.
const OTHER_SIDE_HERO_FACTION: u32 = super::pickups::PICKUP_FACTION;
/// A camp person cannot be attacked (`UNIT_FLAG_NON_ATTACKABLE`), but is picked to talk to.
const KEEPER_UNIT_FLAGS: u32 = 0x2;
/// `UNIT_STAND_STATE_SIT`, the first byte of `UNIT_FIELD_BYTES_1`: a camp person sat on something.
/// (The chair sits, legs over an edge, as asked, do not play on the client yet.)
const STAND_SIT: u8 = 1;

/// How near the hero must stand to use a thing or talk, yards (the client walks up to it first).
const REACH: f32 = 6.0;

/// The camp's own dynamic objects (its tents) are numbered from here.
const CAMP_LOOK_IDS: u32 = 0x7000_0000;
/// The bytes field of a dynamic object of ours.
const LOOK_BYTES: u32 = 1;

/// A piece of the camp: a game object of this entry and display, or (`look`) a model that is no
/// game object, drawn for a dynamic object of that id (`esurv_content::camp_looks`).
#[derive(Clone, Debug)]
struct Piece {
    entry: u32,
    display: u32,
    look: Option<u32>,
    at: (f32, f32),
    facing: f32,
    scale: f32,
    /// The hub's piece it is, which says how it stands ([`Prop`]).
    prop: Option<&'static Prop>,
}

/// Where a model meets the ground, at size 1, yards from its origin: from `x.0` to `x.1` along its
/// facing and from `y.0` to `y.1` to its left (wheels, shafts, poles). A piece with one lies on
/// the ground tilted to its slope ([`Game::lie_on_the_ground`]).
#[derive(Clone, Copy, Debug)]
pub struct Footprint {
    pub x: (f32, f32),
    pub y: (f32, f32),
}

/// How many points along each side of a footprint the ground is read at.
const FOOTPRINT_SAMPLES: usize = 5;

/// A piece of a hub that is no thing to use: its look, size and spot, and where it meets the
/// ground when it lies on it tilted (a game object only; a camp look leans on the client).
#[derive(Clone, Copy, Debug)]
pub struct Prop {
    pub look: PropLook,
    pub scale: f32,
    pub spot: Spot,
    pub footprint: Option<Footprint>,
    /// Set on something rather than the ground (a game object only); with a footprint, only its
    /// lift counts, raising it off the ground it lies on.
    pub set_on: Option<SetOn>,
}

/// How a piece (or a person) sits on another thing: `lift` yards over the ground at its spot, or
/// at `ground_at` (a thing on a platform: the platform's own foot, the deck being level while the
/// ground under it is not), tipped so its front (its facing) rises `rise_degrees`, as a book lies
/// on a slanted desk.
#[derive(Clone, Copy, Debug)]
pub struct SetOn {
    pub lift: f32,
    pub rise_degrees: f32,
    pub ground_at: Option<Spot>,
}

impl SetOn {
    /// Its height over the world's zero, standing at `at` in `place`, given the ground's height.
    fn height(&self, place: CampPlace, at: (f32, f32), ground: impl Fn((f32, f32)) -> f32) -> f32 {
        ground(self.ground_at.map_or(at, |s| place.at(s))) + self.lift
    }
}

/// A prop's look.
#[derive(Clone, Copy, Debug)]
pub enum PropLook {
    /// A game object's (`GameObjectDisplayInfo.dbc`).
    Object(u32),
    /// One of the camp's own (`esurv_content::camp_looks`), a model that is no game object.
    Camp(u32),
}

/// What the camp is doing for one connection: its place, its pieces, the other heroes at their
/// tents, the words of the talk windows open, and where the hero last stood inside the edge.
pub(crate) struct CampState {
    place: CampPlace,
    /// The saved guid of the hero walking the camp (its tent the first plot's).
    own: u32,
    pieces: Vec<Piece>,
    /// The plot of the hero walking the camp: its place in its side's heroes, in the warband's
    /// order.
    own_plot: usize,
    /// The other heroes of the camp: (simulation id, saved hero).
    visitors: Vec<(u32, SavedHero)>,
    /// Each other hero's plot, as `visitors`.
    visitor_plots: Vec<usize>,
    /// The other side's half of the camp and its heroes, each with its simulation id and plot:
    /// standing at their tents, to look at.
    other: CampPlace,
    neighbours: Vec<(u32, SavedHero, usize)>,
    /// The words of each talk window's text, by text id.
    texts: HashMap<u32, String>,
    /// What each open talk window's options do, by speaker.
    menus: HashMap<u64, Vec<CampChoice>>,
    last_inside: HashMap<u32, (f32, f32, f32)>,
    /// The heroes whose next time into the world is a run (shared with the world server).
    runs_asked: Arc<Mutex<HashSet<u32>>>,
    /// The Orb's keeper's polymorph: when she next changes, and the look she wears till then
    /// (none: her own).
    zazzi: (std::time::Instant, Option<u32>),
    /// The placing mode: the decoration spots shown (the camp planner's; only
    /// while it is on).
    placing: bool,
    /// The ground, buildings and pieces the guide walks over and round (none without the game's
    /// data).
    pub(super) floors: Option<Arc<WmoFloors>>,
    /// Where Wren stands or walks, and her walk round the camp, if one is going on.
    wren: Leg,
    tour: Option<Tour>,
    /// Chromie's welcome going on: for which hero, and when she calls the guide over.
    lesson: Option<(u32, std::time::Instant)>,
    /// The hero last told it is being walked round (`ToClient::CampGuided`).
    guided_told: Option<u32>,
    /// The other heroes' life at their tents: poses, gestures, lines (plans/hub-hero-life.md).
    life: Life,
    /// The Bronze Dragonflight's watch over the neutral camp (none elsewhere).
    watch: Option<bronze::Watch>,
    /// Speech bubbles and gold lines to come, and when.
    later: Vec<(std::time::Instant, Packet)>,
    /// The camp's last tick: the time its talk windows and walks go by.
    ticked: std::time::Instant,
}

impl CampState {
    /// The camp fire's guid, while it stands.
    fn fire_guid(&self) -> Option<u64> {
        let PropLook::Object(fire) = self.place.layout.fire_piece.look else {
            return None;
        };
        let (k, p) = self.pieces.iter().enumerate().find(|(_, p)| p.display == fire)?;
        Some(gameobject_guid(p.entry, k as u32 + 1))
    }
}

/// The looks the Orb's keeper polymorphs herself into now and then (another
/// female Alliance character with a random face): women of Stormwind, Ironforge, Darnassus and
/// Gnomeregan, each with a face, hair and clothes of her own (`model_sizes people:<race>:1`).
const ZAZZI_SHAPES: [u32; 13] = [176, 1361, 1445, 1481, 1286, 1839, 3049, 3060, 1714, 1727, 2056, 3125, 5033];
/// How long she stays another woman, and how long herself between, seconds.
const ZAZZI_AWAY: f32 = 5.0;
const ZAZZI_HERSELF: (f32, f32) = (60.0, 120.0);
/// As she changes, either way, a little puff of smoke: the druid's shapeshift
/// puff (`SpellVisualKit` 5729, `DruidMorph_Impact_Base` at her feet, with no sound of its
/// own), and the mage's Teleport sound (`SoundEntries` 3226, `Teleport.wav`).
const ZAZZI_PUFF: u32 = 5729;
const ZAZZI_SOUND: u32 = 3226;
/// The animal the Horde's Mask keeper polymorphs instead of herself: its creature
/// entry (past the Horde's people), a brown boar (`Creature\Boar\Boar`, BoarSkinBrown), and the
/// people it becomes, of the races no hero plays, each at its own size of 1 (`CreatureDisplayInfo.dbc`): forest and
/// jungle trolls, quillboar (plain, warrior, caster), an ogre and an ogre mage, high elves (two men,
/// two women), a gnoll, a kobold, a murloc, a furbolg, a centaur, a harpy, a satyr, a naga, a goblin.
const ORB_SUBJECT_ENTRY: u32 = CAMP_KEEPERS_FROM + camp_entries_from(Faction::Horde) + 9;
const ORB_SUBJECT_LOOK: u32 = 704;
const ORB_SUBJECT_SHAPES: [u32; 21] =
    [194, 167, 298, 924, 4658, 4647, 415, 326, 4494, 4245, 1643, 3293, 175, 163, 346, 145, 172, 969, 467, 4036, 1140];
/// Chromie, sitting in the neutral camp: her creature entry (past the Alliance's
/// people, one for both sides), and her look and size, the world's own Chromie's
/// (`creature_template` 10667: display 10008 at 1.15). No talk window yet.
const CHROMIE_ENTRY: u32 = CAMP_KEEPERS_FROM + 10;
const CHROMIE_LOOK: u32 = 10_008;
const CHROMIE_SIZE: f32 = 1.15;
/// Anachronos, standing at the neutral camp's cave mouth: his creature entry,
/// one for both sides, and his look and size, the world's own (`creature_template` 15192: display
/// 2719 at 1.2).
const ANACHRONOS_ENTRY: u32 = CAMP_KEEPERS_FROM + 11;
const ANACHRONOS_LOOK: u32 = 2719;
const ANACHRONOS_SIZE: f32 = 1.2;
/// She casts Polymorph at it (`Spell.dbc` 118), the change coming as the spell goes off.
const POLYMORPH: u32 = 118;
const POLYMORPH_CAST: std::time::Duration = std::time::Duration::from_millis(1500);
/// How long after her Polymorph goes off the Mask keeper turns back from the boar.
const POLYMORPH_TURN_BACK: std::time::Duration = std::time::Duration::from_secs(1);
/// How long her new body takes to come in before the puff can play on it.
const ZAZZI_PUFF_AFTER: std::time::Duration = std::time::Duration::from_millis(250);

/// The news `ESURV_CAMP_NEWS` names (a development aid).
fn news_to_see() -> Option<crate::save::CampNews> {
    use crate::save::CampNews;
    Some(match std::env::var("ESURV_CAMP_NEWS").ok()?.as_str() {
        "won" => CampNews::Won { first_zone_win: false, zone: 0 },
        "first-win" => CampNews::Won { first_zone_win: true, zone: 0 },
        "lost" => CampNews::Lost { zone: 0 },
        "newcomer" => CampNews::Newcomer,
        _ => return None,
    })
}

/// How long the Orb's keeper stays herself this time, seconds; `ESURV_ZAZZI_SOON=<seconds>`, a
/// development aid, has her change that soon each time, to see it.
fn zazzi_herself() -> f32 {
    if let Some(soon) = std::env::var("ESURV_ZAZZI_SOON").ok().and_then(|v| v.parse().ok()) {
        return soon;
    }
    ZAZZI_HERSELF.0 + chance() * (ZAZZI_HERSELF.1 - ZAZZI_HERSELF.0)
}

/// A number from 0 to 1, new each call.
fn chance() -> f32 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    (h.finish() >> 11) as f32 / (1u64 << 53) as f32
}

/// `words` with each part in <brackets> blue, as 1.12 draws what a talk option does.
fn blue_brackets(words: &str) -> String {
    let mut out = String::new();
    let mut rest = words;
    while let (Some(open), Some(close)) = (rest.find('<'), rest.find('>')) {
        if close < open {
            break;
        }
        out.push_str(&rest[..open]);
        out.push_str("|cff");
        out.push_str(esurv_protocol::ACTION_BLUE_HEX);
        out.push_str(&rest[open..=close]);
        out.push_str("|r");
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

/// What an option of a camp talk window does.
#[derive(Clone, Debug, PartialEq)]
enum CampChoice {
    /// Take over as this hero (its saved guid).
    TakeOver(u32),
    /// Open this window of the game's own (a person's talk window leads into it).
    Open(&'static str),
    /// Turn the placing mode on or off (the camp planner's).
    Placing(bool),
    /// Wren's walk round the camp: start it, skip it, or move it on.
    Tour(TourChoice),
    /// Anachronos's page of lore with this id (none: his greeting).
    Lore(Option<&'static str>),
    /// Chromie's page of words with this id (none: her greeting).
    Chromie(Option<&'static str>),
    Goodbye,
}

/// The camp's own content: the content with the camp's zone added last (no enemies, no bosses,
/// at `hour`), and that zone's index.
pub fn camp_content(content: &Content, place: CampPlace, hour: u32) -> Option<(Arc<Content>, usize)> {
    let mut zone: Zone = content.zones.first()?.clone();
    zone.id = CAMP_ZONE.into();
    zone.name = "The Warband Camp".into();
    zone.subtitle = String::new();
    zone.description = String::new();
    zone.map_id = place.map;
    zone.area_id = place.area;
    let ((x, y), facing) = place.hero_stand(0)?;
    zone.spawn = Spawn { x, y, z: place.spot.2, facing };
    zone.arena_radius = place.layout.reach;
    zone.arena_centre = Some(place.middle());
    zone.walls.clear();
    zone.doors.clear();
    zone.props.clear();
    zone.platforms.clear();
    zone.enemies.clear();
    zone.bosses.clear();
    zone.by_faction = None;
    zone.hour = hour;
    zone.weather = Weather { kind: WeatherKind::None, intensity: 0.0 };
    let mut own = content.clone();
    own.zones.push(zone);
    let index = own.zones.len() - 1;
    Some((Arc::new(own), index))
}

/// What the client's game-object query hears of a camp entry.
pub fn camp_object_info(entry: u32) -> Option<GameObjectInfo> {
    let (type_id, display_id, name) = if (CAMP_PIECES_FROM..CAMP_PIECES_FROM + 100_000).contains(&entry) {
        let display = entry - CAMP_PIECES_FROM;
        match display {
            FIRE | HORDE_FIRE => (GO_GENERIC, display, "Camp Fire"),
            CHAIR => (GO_CHAIR, display, "Chair"),
            _ => (GO_GENERIC, display, ""),
        }
    } else {
        return None;
    };
    let mut data = [0; 24];
    if type_id == GO_CHAIR {
        // one seat, of medium height
        data[0] = 1;
        data[1] = 1;
    }
    Some(GameObjectInfo { type_id, display_id, name: name.into(), data })
}

/// The camp's pieces but its tents ([`tent_pieces`]): the fire, the things to use, and, shown
/// fully ([`SHOW_EVERYTHING`]), the banners, the trophies, the dragon head and the decorations.
fn pieces(place: CampPlace) -> Vec<Piece> {
    let mut out = Vec::new();
    let layout = place.layout;
    // the hubs' pieces, each knowing how it stands
    for prop in layout.props.iter().filter(|_| SERVICES_PLACED) {
        let (entry, display, look) = match prop.look {
            PropLook::Object(display) => (CAMP_PIECES_FROM + display, display, None),
            PropLook::Camp(look) => (0, 0, Some(look)),
        };
        let (at, facing) = (place.at(prop.spot), place.facing(prop.spot));
        out.push(Piece { entry, display, look, at, facing, scale: prop.scale, prop: Some(prop) });
    }
    // both game objects, so they stand solid
    let object = |prop: &Prop| match prop.look {
        PropLook::Object(display) | PropLook::Camp(display) => display,
    };
    let fire = object(layout.fire_piece);
    out.push(Piece {
        entry: CAMP_PIECES_FROM + fire,
        display: fire,
        look: None,
        at: place.at(layout.fire),
        facing: place.facing(layout.fire),
        scale: layout.fire_piece.scale,
        prop: Some(layout.fire_piece),
    });
    let mut put = |entry: u32, display: u32, look: Option<u32>, s: Spot, scale: f32| {
        out.push(Piece { entry, display, look, at: place.at(s), facing: place.facing(s), scale, prop: None });
    };
    let stand = |(display, scale): (u32, f32)| (CAMP_PIECES_FROM + display, display, scale);
    if SHOW_EVERYTHING {
        let shown = layout
            .banners
            .iter()
            .zip(SHOWN_BANNERS)
            .chain(layout.trophies.iter().zip(SHOWN_TROPHIES))
            .chain(std::iter::once((&layout.head, SHOWN_HEAD)));
        for (s, look) in shown {
            let (entry, display, scale) = stand(look);
            put(entry, display, None, *s, scale);
        }
        let (mut any, mut seats, mut supplies) = (SHOWN_ANY.iter().cycle(), SHOWN_SEATS.iter().cycle(), SHOWN_SUPPLIES.iter().cycle());
        // only the fire's own spots: every spot filled at once is a mess
        for (kind, s) in layout.decorations.iter().filter(|(k, _)| *k == DecoKind::Any) {
            let look = match kind {
                DecoKind::Any => any.next(),
                DecoKind::Seat => seats.next(),
                DecoKind::Supplies => supplies.next(),
            };
            if let Some(&look) = look {
                let (entry, display, scale) = stand(look);
                put(entry, display, None, *s, scale);
            }
        }
    }
    out
}

/// The tents of `place`'s first `heroes` plots: a tent only where a hero is.
fn tent_pieces(place: CampPlace, heroes: usize) -> Vec<Piece> {
    let layout = place.layout;
    let (PropLook::Object(tent_look) | PropLook::Camp(tent_look)) = layout.tent.look;
    let plots = layout.plots.iter().zip(layout.tent_turns).take(heroes);
    plots
        .map(|(tent, turn)| {
            let (s, c) = tent.faces.to_radians().sin_cos();
            let faces = tent.faces + layout.tent.spot.faces + turn;
            let at = Spot { east: tent.east + TENT_AHEAD * s, north: tent.north + TENT_AHEAD * c, faces };
            Piece {
                entry: CAMP_PIECES_FROM + tent_look,
                display: tent_look,
                look: None,
                at: place.at(at),
                facing: place.facing(at),
                scale: layout.tent.scale,
                prop: Some(layout.tent),
            }
        })
        .collect()
}

/// Heroes, each with its plot.
type Plotted = Vec<(SavedHero, usize)>;

/// Each hero's plot: each side's heroes on its own plots in the warband's order (`own` first when
/// `warband` leaves it out), as many as the plots hold. The plot of `own`, the other heroes of its
/// side and the other side's heroes.
fn plots_of(content: &Content, place: CampPlace, own: u32, warband: &[SavedHero]) -> (usize, Plotted, Plotted) {
    // a hero of no known race counts as of the camp's side
    let side_of = |s: &SavedHero| content.races.iter().find(|r| r.id == s.race).map_or(place.faction, |r| r.faction);
    let own_first = (!warband.iter().any(|s| s.guid == own)).then_some(None);
    let order = own_first.into_iter().chain(warband.iter().map(Some));
    let (mut own_plot, mut mine, mut theirs) = (0, Vec::new(), Vec::new());
    let (mut next_mine, mut next_theirs) = (0, 0);
    for hero in order {
        match hero {
            Some(s) if s.guid != own && side_of(s) != place.faction => {
                theirs.push((s.clone(), next_theirs));
                next_theirs += 1;
            }
            Some(s) if s.guid != own => {
                mine.push((s.clone(), next_mine));
                next_mine += 1;
            }
            _ => {
                own_plot = next_mine;
                next_mine += 1;
            }
        }
    }
    let plots = place.layout.plots.len();
    own_plot = own_plot.min(plots - 1);
    mine.retain(|(_, p)| *p < plots);
    theirs.retain(|(_, p)| *p < other_side(place).layout.plots.len());
    (own_plot, mine, theirs)
}

/// The other side's half of the neutral camp.
fn other_side(place: CampPlace) -> CampPlace {
    match place.faction {
        Faction::Alliance => NEUTRAL_HORDE_CAMP,
        Faction::Horde => NEUTRAL_ALLIANCE_CAMP,
    }
}

/// The least-squares plane through `points` (x, y, height): its rise per yard along x and y.
fn ground_slope(points: &[(f32, f32, f32)]) -> (f32, f32) {
    let n = points.len().max(1) as f32;
    let mean = points.iter().fold((0.0, 0.0, 0.0), |s, p| (s.0 + p.0 / n, s.1 + p.1 / n, s.2 + p.2 / n));
    let (mut xx, mut xy, mut yy, mut xz, mut yz) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for p in points {
        let (dx, dy, dz) = (p.0 - mean.0, p.1 - mean.1, p.2 - mean.2);
        xx += dx * dx;
        xy += dx * dy;
        yy += dy * dy;
        xz += dx * dz;
        yz += dy * dz;
    }
    let det = xx * yy - xy * xy;
    if det.abs() < f32::EPSILON {
        return (0.0, 0.0);
    }
    ((xz * yy - yz * xy) / det, (yz * xx - xz * xy) / det)
}

/// The rotation `q` after `r` (quaternions as x, y, z, w).
fn quat_mul(q: [f32; 4], r: [f32; 4]) -> [f32; 4] {
    let [x1, y1, z1, w1] = q;
    let [x2, y2, z2, w2] = r;
    [
        w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2,
        w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2,
        w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2,
        w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2,
    ]
}

/// The create of a camp model that is no game object: a dynamic object of look `look`, numbered
/// `id`, standing at `position` turned to `facing`; nobody casts it.
fn look_object(id: u32, look: u32, position: [f32; 3], facing: f32, scale: f32) -> ObjectUpdate {
    let guid = super::ground_looks::ground_look_guid(id);
    let mut f = Fields::new();
    f.u64(field::OBJECT_GUID, guid)
        .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_DYNAMICOBJECT)
        .u32(field::OBJECT_ENTRY, look)
        .f32(field::OBJECT_SCALE_X, scale);
    f.u64(field::DYNAMICOBJECT_CASTER, 0)
        .u32(field::DYNAMICOBJECT_BYTES, LOOK_BYTES)
        .u32(field::DYNAMICOBJECT_SPELLID, look)
        .f32(field::DYNAMICOBJECT_RADIUS, 0.0);
    for (k, v) in position.iter().enumerate() {
        f.f32(field::DYNAMICOBJECT_POS_X + k as u16, *v);
    }
    f.f32(field::DYNAMICOBJECT_FACING, facing);
    ObjectUpdate::CreatePlaced { guid, type_id: ObjectTypeId::DynamicObject, position, facing, fields: f }
}

/// A line of dialogue this long or shorter is followed by the shortest pause, characters.
const SHORT_LINE: f32 = 40.0;
/// A line this long or longer is followed by the longest pause, characters.
const LONG_LINE: f32 = 160.0;
/// The shortest and longest pause after a line of dialogue, seconds.
const LINE_GAPS: (f32, f32) = (4.0, 7.0);

/// The pause after line `words` before the next line: longer for a longer line, from the shortest
/// at [`SHORT_LINE`] characters to the longest at [`LONG_LINE`].
fn line_gap(words: &str) -> std::time::Duration {
    let long = ((words.chars().count() as f32 - SHORT_LINE) / (LONG_LINE - SHORT_LINE)).clamp(0.0, 1.0);
    std::time::Duration::from_secs_f32(LINE_GAPS.0 + long * (LINE_GAPS.1 - LINE_GAPS.0))
}

/// What a hero at its tent says when talked to: its last run, or what it has been up to.
fn greeting(content: &Content, h: &SavedHero) -> String {
    let zone = h.zone.as_deref().and_then(|z| content.zones.iter().find(|zz| zz.id == z)).map(|z| z.name.as_str());
    match (h.records.runs, zone) {
        (0, _) | (_, None) => "Haven't been out there yet. I've been keeping watch and getting my gear in order. \
             Say the word and I'll take the field."
            .into(),
        (_, Some(zone)) if h.records.wins > 0 => {
            format!("Just back from {zone}. We've beaten it before and we'll beat it again. Need me out there?")
        }
        (_, Some(zone)) => {
            format!("Just back from {zone}. It was rough out there, but I'm patched up and ready to try again.")
        }
    }
}

impl Game {
    /// Makes this game the camp of `place` for its hero `own`, with the rest of `warband` (the
    /// warband's heroes in its order, both sides; `own` among them or, left out, first) at their
    /// tents: each side's heroes on its plots in that order, the other side's in its own half.
    /// `runs_asked` is where a hero sent off from the war table is marked for a run.
    pub fn make_camp(&mut self, place: CampPlace, own: u32, warband: &[SavedHero], runs_asked: Arc<Mutex<HashSet<u32>>>) {
        let other = other_side(place);
        let (own_plot, mine, theirs) = plots_of(&self.run.content, place, own, warband);
        let visitor_plots: Vec<usize> = mine.iter().map(|(_, p)| *p).collect();
        let visitors: Vec<(u32, SavedHero)> = mine.into_iter().enumerate().map(|(k, (s, _))| (u32::MAX - 1 - k as u32, s)).collect();
        let neighbours: Vec<(u32, SavedHero, usize)> =
            theirs.into_iter().enumerate().map(|(k, (s, p))| (u32::MAX - 1 - (place.layout.plots.len() + k) as u32, s, p)).collect();
        for (id, saved) in visitors.iter().map(|(id, s)| (id, s)).chain(neighbours.iter().map(|(id, s, _)| (id, s))) {
            let dressed = self.dress(saved);
            self.dressed.insert(*id, dressed);
        }
        // `ESURV_CAMP_NEWS=won|first-win|lost|newcomer`, a development aid, starts the camp with that
        // news, to see the camp react
        let news = self.saves.take_news(own).or_else(news_to_see);
        // a new hero's first arrival in the neutral camp is in front of Chromie, facing her; every
        // later one at its tent
        let arrives = if news == Some(crate::save::CampNews::Newcomer) { before_chromie(place) } else { None };
        // the warband's very first hero is welcomed by Chromie and walked round
        let lesson = arrives.is_some() && self.saves.heroes().len() == 1 && !self.recruited();
        // the hero at its own tent, facing out of it; `ESURV_CAMP_START=<east>,<north>,<facing>`
        // (a dev aid, to look at a part of the camp) starts it there
        let start = std::env::var("ESURV_CAMP_START").ok().and_then(|v| {
            let n: Vec<f32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            let s = spot(*n.first()?, *n.get(1)?, *n.get(2)?);
            Some((place.at(s), place.facing(s)))
        });
        if let Some((at, facing)) = start.or(arrives).or_else(|| place.hero_stand(own_plot)) {
            let z = self.camp_ground(place, at);
            if let Some(h) = self.run.heroes.first_mut() {
                h.pos = at;
                h.height = z;
                h.facing = facing;
            }
        }
        // the hub's buffs the warband bought: in the hub in Tanaris the hero runs that much faster
        // (Time Floes, 60%)
        if place.spot == NEUTRAL_ALLIANCE_CAMP.spot {
            let content = self.run.content.clone();
            let bought = self.saves.achievements(&content).bought;
            let faster: f32 = content
                .shop
                .hub_buffs
                .iter()
                .filter(|b| bought.contains(&esurv_content::hub_buff_item(&b.id)))
                .map(|b| b.run_faster)
                .sum();
            if let Some(h) = self.run.heroes.first_mut() {
                h.stats.move_speed *= 1.0 + faster;
            }
        }
        let walk_from = self.run.heroes.first().and_then(|h| walk_from(h.id, place));
        // each other hero's place at its tent, its height on the ground, and the way it faces there
        let stands = visitor_plots
            .iter()
            .map(|&p| place.hero_stand(p).map(|(at, facing)| ([at.0, at.1, self.camp_ground(place, at)], facing)))
            .collect();
        let pieces: Vec<Piece> = pieces(place).into_iter().chain(tent_pieces(place, visitor_plots.len() + 1)).chain(tent_pieces(other, neighbours.len())).collect();
        let floors = self.camp_floors(place, &pieces);
        self.camp = Some(CampState {
            pieces,
            floors,
            place,
            own,
            own_plot,
            visitors,
            visitor_plots,
            other,
            neighbours,
            texts: HashMap::new(),
            menus: HashMap::new(),
            last_inside: HashMap::new(),
            runs_asked,
            zazzi: (std::time::Instant::now() + std::time::Duration::from_secs_f32(zazzi_herself()), None),
            placing: false,
            wren: walk_from.as_ref().map_or_else(|| Leg::still(place.layout.keepers.iter().find(|(k, _)| *k == Keeper::Wren).map_or(place.fire(), |(_, s)| place.at(*s))), |(leg, _)| leg.clone()),
            // `ESURV_WREN_TOUR=1`, a development aid, starts her walk at once, to see it
            // (only in a camp she is in: the Horde's has no walk yet)
            tour: walk_from
                .map(|(_, t)| t)
                .or_else(|| std::env::var_os("ESURV_WREN_TOUR").and(self.run.heroes.first()).map(|h| Tour::new(h.id)))
                .filter(|_| place.layout.keepers.iter().any(|(k, _)| *k == Keeper::Wren)),
            lesson: None,
            guided_told: None,
            life: Life::new(stands, news, &self.saves.recent_runs(), self.run.content.zones[self.run.zone].hour, std::time::Instant::now()),
            watch: None,
            later: Vec::new(),
            ticked: std::time::Instant::now(),
        });
        if let Some(hero) = self.run.heroes.first().map(|h| h.id).filter(|_| lesson) {
            // her first words are for its race and time period: the saved hero, its player not
            // joined yet
            let saved = self.saves.hero(own);
            self.chromie_welcome(hero, saved.as_ref(), std::time::Instant::now() + std::time::Duration::from_secs_f32(tour::WELCOME_AFTER));
        } else if arrives.is_some() {
            // every later new hero hears only her first words for its race and time period
            let saved = self.saves.hero(own);
            self.chromie_greet(saved.as_ref(), std::time::Instant::now() + std::time::Duration::from_secs_f32(tour::WELCOME_AFTER));
        }
        // where the heroes walk to and the ground they walk on, the camp standing, and what each
        // holds (a two-hander, a shield: how it drills at a training dummy)
        let (haunts, walls) = self.camp_walks();
        let kits = self.camp.iter().flat_map(|c| &c.visitors).map(|(id, _)| {
            let worn = self.dressed.get(id).map_or(&[][..], |d| &d.worn[..]);
            let holds = |slot: u8, kind: u32| worn.iter().any(|w| w.equip_slot == slot && w.inventory_type == kind);
            Kit { two_hand: holds(MAIN_HAND, INVTYPE_2HWEAPON), shield: holds(OFF_HAND, INVTYPE_SHIELD) }
        });
        let kits = kits.collect();
        let watch = bronze::Watch::new(place, &walls, std::time::Instant::now());
        if let Some(camp) = &mut self.camp {
            camp.watch = watch;
            camp.life.set_ways(haunts, walls, kits);
        }
        self.work_out_hero_ways();
    }

    /// The camp's own life between the heroes' doings: Wren's walk round the camp, and the Orb's
    /// keeper now and then polymorphing herself into another woman, and back.
    pub(super) fn camp_tick(&mut self) -> Vec<Packet> {
        let now = std::time::Instant::now();
        let mut out = self.tour_tick(now);
        self.tell_guided();
        out.extend(self.zazzi_tick(now));
        out.extend(self.life_tick(now));
        out.extend(self.watch_tick(now));
        out
    }

    /// The Orb's keeper changes, when her time comes: herself, or (the Horde's) the boar before her,
    /// which she polymorphs into a person and which turns back into a boar on its own.
    fn zazzi_tick(&mut self, now: std::time::Instant) -> Vec<Packet> {
        let Some(camp) = &mut self.camp else {
            return Vec::new();
        };
        if now < camp.zazzi.0 || !camp.place.layout.keepers.iter().any(|(k, _)| *k == Keeper::OrbKeeper) {
            return Vec::new();
        }
        let subject = camp.place.layout.orb_subject.is_some();
        let (shapes, own): (&[u32], u32) =
            if subject { (&ORB_SUBJECT_SHAPES, ORB_SUBJECT_LOOK) } else { (&ZAZZI_SHAPES, CAMP_ORB_KEEPER_LOOK) };
        let (look, wait) = match camp.zazzi.1 {
            Some(_) => (None, zazzi_herself()),
            None => (Some(shapes[(chance() * shapes.len() as f32) as usize % shapes.len()]), ZAZZI_AWAY),
        };
        // the boar changes as her spell goes off, its time as a person from then
        let cast = if subject && look.is_some() { POLYMORPH_CAST } else { std::time::Duration::ZERO };
        camp.zazzi = (now + cast + std::time::Duration::from_secs_f32(wait), look);
        let keeper = self.keeper_guid(Keeper::OrbKeeper);
        let guid = if subject { super::unit_guid(ORB_SUBJECT_ENTRY, 1) } else { keeper };
        let mut fields = Fields::new();
        fields.u32(field::UNIT_DISPLAYID, look.unwrap_or(own));
        let change = [crate::update::update_object(&[ObjectUpdate::Values { guid, fields }]), packets::play_object_sound(ZAZZI_SOUND, guid)];
        let Some(camp) = &mut self.camp else {
            return Vec::new();
        };
        // the puff once the new body is in: a body changed drops the effects on the old one
        camp.later.push((now + cast + ZAZZI_PUFF_AFTER, packets::play_spell_visual(guid, ZAZZI_PUFF)));
        if cast.is_zero() {
            return change.into();
        }
        let target = packets::SpellTarget::Unit(guid);
        camp.later.extend(change.map(|p| (now + cast, p)));
        camp.later.push((now + cast, packets::spell_go(keeper, POLYMORPH, &[guid], target, None)));
        // she turns to the boar to cast, and back once it has gone off
        let place = camp.place;
        let her = place.layout.keepers.iter().find(|(k, _)| *k == Keeper::OrbKeeper).map(|&(_, s)| s);
        let (Some(her), Some(boar)) = (her, place.layout.orb_subject) else {
            return vec![packets::spell_start(keeper, POLYMORPH, cast.as_millis() as u32, target)];
        };
        let (from, to) = (place.at(her), place.at(boar));
        let toward = (to.1 - from.1).atan2(to.0 - from.0).rem_euclid(std::f32::consts::TAU);
        let mut out: Vec<Packet> = self.keeper_turn(Keeper::OrbKeeper, toward).into_iter().collect();
        out.push(packets::spell_start(keeper, POLYMORPH, cast.as_millis() as u32, target));
        if let Some(back) = self.keeper_turn(Keeper::OrbKeeper, place.facing(her)) {
            self.camp.as_mut().expect("a camp").later.push((now + cast + POLYMORPH_TURN_BACK, back));
        }
        out
    }

    /// The boar the Mask keeper polymorphs, standing at `s` as it looks now: friendly, not one to
    /// talk to.
    fn orb_subject_object(&self, place: CampPlace, s: Spot) -> ObjectUpdate {
        let at = place.at(s);
        // the Mask keeper's polymorphs are only in her own side's camp
        let look = self.camp.as_ref().filter(|c| c.place.faction == place.faction).and_then(|c| c.zazzi.1).unwrap_or(ORB_SUBJECT_LOOK);
        // the other side's boar numbered apart, not to be taken for this side's
        let own_side = self.camp.as_ref().is_none_or(|c| c.place.faction == place.faction);
        let guid = super::unit_guid(ORB_SUBJECT_ENTRY, if own_side { 1 } else { 2 });
        let ObjectUpdate::Create { living, mut fields, .. } =
            super::bunnies::still_unit_create(guid, ORB_SUBJECT_ENTRY, look, 1.0, [at.0, at.1, self.camp_ground(place, at)])
        else {
            unreachable!("a still unit is a create")
        };
        fields
            .u32(field::UNIT_NATIVEDISPLAYID, ORB_SUBJECT_LOOK)
            .u32(field::UNIT_HEALTH, 100)
            .u32(field::UNIT_MAXHEALTH, 100)
            .u32(field::UNIT_FACTIONTEMPLATE, keeper_faction(place.faction))
            .u32(field::UNIT_FLAGS, KEEPER_UNIT_FLAGS)
            .f32(field::UNIT_BOUNDINGRADIUS, 0.4)
            .f32(field::UNIT_COMBATREACH, 1.5);
        let living = crate::update::Living { facing: place.facing(s), ..living };
        ObjectUpdate::Create { guid, type_id: ObjectTypeId::Unit, living, fields, own: false }
    }

    /// The Camp chatter option as the player set it (`campchatter`): at 0 the heroes say nothing.
    pub(super) fn camp_chatter(&mut self, level: u32) -> bool {
        let Some(camp) = &mut self.camp else {
            return false;
        };
        camp.life.mute = level == 0;
        true
    }

    /// Whether this game is a camp, not a run.
    pub(super) fn is_camp(&self) -> bool {
        self.camp.is_some()
    }

    /// The ground at `at` near the camp's height.
    fn camp_ground(&self, place: CampPlace, at: (f32, f32)) -> f32 {
        self.run.world.floor(at.0, at.1, place.spot.2 + 20.0).unwrap_or(place.spot.2)
    }

    /// Every piece of the camp and every other hero at its tent, as creates (for a player who
    /// arrives).
    pub(super) fn camp_now(&self) -> Vec<ObjectUpdate> {
        let Some(camp) = &self.camp else {
            return Vec::new();
        };
        let mut out: Vec<ObjectUpdate> = camp
            .pieces
            .iter()
            .enumerate()
            .map(|(k, p)| self.piece_object(camp.place, p, k))
            .collect();
        out.extend(self.decoration_objects(camp.place));
        for ((id, saved), plot) in camp.visitors.iter().zip(&camp.visitor_plots) {
            out.extend(self.visitor_object(camp.place, *id, *plot, saved));
        }
        for (keeper, s) in camp.place.layout.keepers.iter().filter(|_| SERVICES_PLACED) {
            out.push(self.keeper_object(camp.place, *keeper, *s, true));
        }
        // the other side's half: its heroes at their tents, its people and its Mask's boar, to look
        // at, as its hubs and things are
        for (id, saved, plot) in &camp.neighbours {
            out.extend(self.visitor_object(camp.other, *id, *plot, saved));
        }
        for (keeper, s) in camp.other.layout.keepers.iter().filter(|_| SERVICES_PLACED) {
            out.push(self.keeper_object(camp.other, *keeper, *s, false));
        }
        if let Some(s) = camp.other.layout.orb_subject.filter(|_| SERVICES_PLACED) {
            out.push(self.orb_subject_object(camp.other, s));
        }
        for (service, s) in camp.place.layout.services.iter().filter(|_| SERVICES_PLACED) {
            out.push(self.thing_object(camp.place, *service, *s));
        }
        if let Some(s) = camp.place.layout.orb_subject.filter(|_| SERVICES_PLACED) {
            out.push(self.orb_subject_object(camp.place, s));
        }
        if let Some((s, on)) = camp.place.layout.chromie.filter(|_| SERVICES_PLACED) {
            out.push(self.chromie_object(camp.place, s, on));
        }
        if let Some(s) = camp.place.layout.anachronos.filter(|_| SERVICES_PLACED) {
            out.push(self.anachronos_object(camp.place, s));
        }
        out.extend(self.watch_now());
        out
    }

    /// Anachronos standing at `s` on the ground: friendly, one to talk to.
    fn anachronos_object(&self, place: CampPlace, s: Spot) -> ObjectUpdate {
        let at = place.at(s);
        let z = self.camp_ground(place, at);
        let guid = super::unit_guid(ANACHRONOS_ENTRY, 1);
        let ObjectUpdate::Create { living, mut fields, .. } =
            super::bunnies::still_unit_create(guid, ANACHRONOS_ENTRY, ANACHRONOS_LOOK, ANACHRONOS_SIZE, [at.0, at.1, z])
        else {
            unreachable!("a still unit is a create")
        };
        fields
            .u32(field::UNIT_HEALTH, 100)
            .u32(field::UNIT_MAXHEALTH, 100)
            .u32(field::UNIT_FACTIONTEMPLATE, keeper_faction(place.faction))
            .u32(field::UNIT_FLAGS, KEEPER_UNIT_FLAGS)
            .u32(field::UNIT_NPC_FLAGS, NPC_GOSSIP)
            .f32(field::UNIT_BOUNDINGRADIUS, 0.4)
            .f32(field::UNIT_COMBATREACH, 1.5);
        let living = crate::update::Living { facing: place.facing(s), ..living };
        ObjectUpdate::Create { guid, type_id: ObjectTypeId::Unit, living, fields, own: false }
    }

    /// Anachronos's talk window at page `page` (none: a greeting, one at random): its words, an
    /// option on to each page it leads to, Back and Farewell (plans/hub-bronze-lore.md section 2).
    fn anachronos_page(&mut self, speaker: u64, page: Option<&'static str>) -> Vec<Packet> {
        let lore = |label: &str, to: Option<&'static str>| (label.to_string(), CampChoice::Lore(to));
        let (words, mut options) = match page.and_then(anachronos::page) {
            None => {
                use rand::seq::SliceRandom;
                let words = anachronos::GREETINGS.choose(&mut rand::thread_rng()).copied().unwrap_or_default();
                (words, anachronos::ASKS.iter().map(|(label, to)| lore(label, Some(*to))).collect::<Vec<_>>())
            }
            Some(p) => {
                let mut options: Vec<_> = p.more.iter().map(|(label, to)| lore(label, Some(*to))).collect();
                // the moments' page: a page for each zone the save has discovered
                if p.id == anachronos::MOMENTS {
                    let content = self.run.content.clone();
                    for id in self.saves.zones_discovered(&content) {
                        let (Some(zone), Some(to)) = (content.zones.iter().find(|z| z.id == id), anachronos::page(&id)) else {
                            continue;
                        };
                        options.push(lore(&format!("Tell me of {}.", zone.name), Some(to.id)));
                    }
                }
                options.push(lore("Back.", p.back));
                (p.words, options)
            }
        };
        options.push(("Farewell.".to_string(), CampChoice::Goodbye));
        self.camp_window(speaker, words.into(), options)
    }

    /// Chromie sitting at `s` on what `on` says: friendly, one to talk to.
    fn chromie_object(&self, place: CampPlace, s: Spot, on: SetOn) -> ObjectUpdate {
        let at = place.at(s);
        let z = on.height(place, at, |p| self.camp_ground(place, p));
        let guid = super::unit_guid(CHROMIE_ENTRY, 1);
        let ObjectUpdate::Create { living, mut fields, .. } =
            super::bunnies::still_unit_create(guid, CHROMIE_ENTRY, CHROMIE_LOOK, CHROMIE_SIZE, [at.0, at.1, z])
        else {
            unreachable!("a still unit is a create")
        };
        fields
            .u32(field::UNIT_HEALTH, 100)
            .u32(field::UNIT_MAXHEALTH, 100)
            .u32(field::UNIT_FACTIONTEMPLATE, keeper_faction(place.faction))
            .u32(field::UNIT_FLAGS, KEEPER_UNIT_FLAGS)
            .u32(field::UNIT_NPC_FLAGS, NPC_GOSSIP)
            .f32(field::UNIT_BOUNDINGRADIUS, 0.4)
            .f32(field::UNIT_COMBATREACH, 1.5);
        fields.bytes(field::UNIT_BYTES_1, [STAND_SIT, 0, 0, 0]);
        let living = crate::update::Living { facing: place.facing(s), ..living };
        ObjectUpdate::Create { guid, type_id: ObjectTypeId::Unit, living, fields, own: false }
    }

    /// Thing `service` standing at `s`: a unit wearing its object's model, named for it and
    /// targeted as a person of the camp is, standing still where it was put.
    fn thing_object(&self, place: CampPlace, service: Service, s: Spot) -> ObjectUpdate {
        let at = place.at(s);
        // one standing up on something is that high; any other on the ground
        let up_on = place.layout.services_on.iter().find(|(t, _)| *t == service).map(|(_, on)| on);
        let z = up_on.map_or_else(|| self.camp_ground(place, at), |on| on.height(place, at, |p| self.camp_ground(place, p)));
        let position = [at.0, at.1, z];
        let (display, scale) = service.look(place.faction);
        let guid = service.guid(place.faction);
        let ObjectUpdate::Create { living, mut fields, .. } =
            super::bunnies::still_unit_create(guid, service.entry(place.faction), camp_thing_look(display), scale, position)
        else {
            unreachable!("a still unit is a create")
        };
        fields
            .u32(field::UNIT_FACTIONTEMPLATE, keeper_faction(place.faction))
            .u32(field::UNIT_FLAGS, KEEPER_UNIT_FLAGS)
            .u32(field::UNIT_NPC_FLAGS, NPC_GOSSIP)
            .f32(field::UNIT_BOUNDINGRADIUS, 0.4)
            .f32(field::UNIT_COMBATREACH, 1.5);
        let living = crate::update::Living { facing: place.facing(s), ..living };
        ObjectUpdate::Create { guid, type_id: ObjectTypeId::Unit, living, fields, own: false }
    }

    /// The guid of camp person `keeper`.
    fn keeper_guid(&self, keeper: Keeper) -> u64 {
        super::unit_guid(keeper.entry(self.camp_faction()), keeper as u32 + 1)
    }

    /// Whose camp this is (the Alliance's outside one).
    fn camp_faction(&self) -> Faction {
        self.camp.as_ref().map_or(Faction::Alliance, |c| c.place.faction)
    }

    /// Camp person `keeper`'s name in this camp.
    fn keeper_name(&self, keeper: Keeper) -> &'static str {
        keeper.who(self.camp_faction()).map_or("", |(name, _, _)| name)
    }

    /// Where camp person `keeper` put at `s` is: one sat or standing on something that high, anyone
    /// else on the ground.
    fn keeper_position(&self, place: CampPlace, keeper: Keeper, s: Spot) -> [f32; 3] {
        let at = place.at(s);
        let seat = place.layout.seats.iter().find(|(k, _)| *k == keeper).map(|(_, on)| on);
        let up_on = seat.or_else(|| place.layout.stands_on.iter().find(|(k, _)| *k == keeper).map(|(_, on)| on));
        let z = up_on.map_or_else(|| self.camp_ground(place, at), |on| on.height(place, at, |p| self.camp_ground(place, p)));
        [at.0, at.1, z]
    }

    /// Camp person `keeper` turning where it stands to `facing`, a world angle.
    fn keeper_turn(&mut self, keeper: Keeper, facing: f32) -> Option<Packet> {
        let place = self.camp.as_ref()?.place;
        let &(_, s) = place.layout.keepers.iter().find(|(k, _)| *k == keeper)?;
        let at = self.keeper_position(place, keeper, s);
        let spline = self.next_spline;
        self.next_spline = self.next_spline.wrapping_add(1);
        Some(packets::monster_move_facing(self.keeper_guid(keeper), at, spline, &[at], 1, false, Some(facing)))
    }

    /// Camp person `keeper` of `place` standing at `s`: friendly, never to fight, one to talk to if
    /// `talks` (the other side's are only to look at).
    fn keeper_object(&self, place: CampPlace, keeper: Keeper, s: Spot, talks: bool) -> ObjectUpdate {
        let entry = keeper.entry(place.faction);
        let display = camp_creature_info(entry).map_or(MERCHANT_DISPLAY, |i| i.display_id);
        let seat = place.layout.seats.iter().find(|(k, _)| *k == keeper).map(|(_, on)| on);
        let position = self.keeper_position(place, keeper, s);
        let guid = super::unit_guid(entry, keeper as u32 + 1);
        let ObjectUpdate::Create { living, mut fields, .. } = super::bunnies::still_unit_create(guid, entry, display, 1.0, position) else {
            unreachable!("a still unit is a create")
        };
        if seat.is_some() {
            fields.bytes(field::UNIT_BYTES_1, [STAND_SIT, 0, 0, 0]);
        }
        if let Some(held) = held_by(place.faction, keeper) {
            fields.extend(&super::weapon_fields(held));
        }
        fields
            .u32(field::UNIT_HEALTH, 100)
            .u32(field::UNIT_MAXHEALTH, 100)
            .u32(field::UNIT_LEVEL, 30)
            .u32(field::UNIT_FACTIONTEMPLATE, keeper_faction(place.faction))
            .u32(field::UNIT_FLAGS, KEEPER_UNIT_FLAGS)
            .u32(field::UNIT_NPC_FLAGS, if talks { NPC_GOSSIP } else { 0 })
            .f32(field::UNIT_BOUNDINGRADIUS, 0.4)
            .f32(field::UNIT_COMBATREACH, 1.5);
        let living = crate::update::Living { facing: place.facing(s), ..living };
        ObjectUpdate::Create { guid, type_id: ObjectTypeId::Unit, living, fields, own: false }
    }

    /// A piece laid on the ground under its footprint: tilted to the plane that best fits the
    /// ground there, and as high as that plane goes with no point of the footprint above the
    /// ground (an uphill wheel sinks a little rather than a downhill one hanging). Its height and
    /// its rotation (`GAMEOBJECT_ROTATION`: x, y, z, w; its turn, then the tilt).
    fn lie_on_the_ground(&self, place: CampPlace, p: &Piece, foot: Footprint) -> (f32, [f32; 4]) {
        let (sin, cos) = p.facing.sin_cos();
        let last = (FOOTPRINT_SAMPLES - 1) as f32;
        // the ground's points under the footprint, relative to the piece's spot
        let mut points = Vec::with_capacity(FOOTPRINT_SAMPLES * FOOTPRINT_SAMPLES);
        for i in 0..FOOTPRINT_SAMPLES {
            for j in 0..FOOTPRINT_SAMPLES {
                let along = (foot.x.0 + (foot.x.1 - foot.x.0) * i as f32 / last) * p.scale;
                let left = (foot.y.0 + (foot.y.1 - foot.y.0) * j as f32 / last) * p.scale;
                let (dx, dy) = (along * cos - left * sin, along * sin + left * cos);
                points.push((dx, dy, self.camp_ground(place, (p.at.0 + dx, p.at.1 + dy))));
            }
        }
        let (a, b) = ground_slope(&points);
        let base = points.iter().map(|&(dx, dy, z)| z - a * dx - b * dy).fold(f32::MAX, f32::min);
        // the tilt carries straight up onto the plane's up, (-a, -b, 1)
        let up = {
            let n = (a * a + b * b + 1.0).sqrt();
            (-a / n, -b / n, 1.0 / n)
        };
        let half_angle = up.2.clamp(-1.0, 1.0).acos() / 2.0;
        let axis = (-up.1, up.0);
        let axis_len = axis.0.hypot(axis.1);
        let tilt = if axis_len < 1e-6 {
            [0.0, 0.0, 0.0, 1.0]
        } else {
            let s = half_angle.sin() / axis_len;
            [axis.0 * s, axis.1 * s, 0.0, half_angle.cos()]
        };
        let turn = [0.0, 0.0, (p.facing / 2.0).sin(), (p.facing / 2.0).cos()];
        (base, quat_mul(tilt, turn))
    }

    /// Where a piece that is no look stands: its height and its turn (a quaternion), as its object
    /// is sent.
    fn piece_pose(&self, place: CampPlace, p: &Piece) -> (f32, [f32; 4]) {
        let turn = [0.0, 0.0, (p.facing / 2.0).sin(), (p.facing / 2.0).cos()];
        match (p.prop.and_then(|q| q.footprint), p.prop.and_then(|q| q.set_on)) {
            // lying on the ground, raised by its lift if it has one (a rug over bumpy snow)
            (Some(foot), on) => {
                let (z, rotation) = self.lie_on_the_ground(place, p, foot);
                (z + on.map_or(0.0, |o| o.lift), rotation)
            }
            (None, Some(on)) => {
                // tipped about its own left: a turn of minus the rise lifts its front
                let half = -on.rise_degrees.to_radians() / 2.0;
                (on.height(place, p.at, |q| self.camp_ground(place, q)), quat_mul(turn, [0.0, half.sin(), 0.0, half.cos()]))
            }
            (None, None) => (self.camp_ground(place, p.at), turn),
        }
    }

    /// The camp's own ground for its guide to walk: the world's ground, buildings and rocks
    /// about the camp and every piece of it the client collides with, as each is sent. `None`
    /// without the game's data.
    fn camp_floors(&self, place: CampPlace, pieces: &[Piece]) -> Option<Arc<WmoFloors>> {
        let data = self.data.as_ref()?;
        let mut models = Vec::new();
        for p in pieces.iter().filter(|p| p.look.is_none()) {
            let Some((_, model)) = CAMP_OBJECT_DISPLAYS.iter().find(|(d, _)| *d == p.display) else { continue };
            let (z, rotation) = self.piece_pose(place, p);
            models.push((*model, [p.at.0, p.at.1, z], rotation, p.scale));
        }
        let map = self.run.content.zones[self.run.zone].map_id;
        data.ground_near(map, place.middle(), place.layout.reach, &models).ok().map(Arc::new)
    }

    fn piece_object(&self, place: CampPlace, p: &Piece, k: usize) -> ObjectUpdate {
        if let Some(look) = p.look {
            let z = self.camp_ground(place, p.at);
            return look_object(CAMP_LOOK_IDS + k as u32, look, [p.at.0, p.at.1, z], p.facing, p.scale);
        }
        let (z, rotation) = self.piece_pose(place, p);
        let guid = gameobject_guid(p.entry, k as u32 + 1);
        let mut f = Fields::new();
        f.u64(field::OBJECT_GUID, guid)
            .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_GAMEOBJECT)
            .u32(field::OBJECT_ENTRY, p.entry)
            .f32(field::OBJECT_SCALE_X, p.scale);
        let chair = p.display == CHAIR;
        f.u32(field::GAMEOBJECT_DISPLAYID, p.display)
            .u32(field::GAMEOBJECT_FLAGS, if chair { 0 } else { GO_FLAG_NO_INTERACT });
        for (k, v) in rotation.iter().enumerate() {
            f.f32(field::GAMEOBJECT_ROTATION + k as u16, *v);
        }
        f.u32(field::GAMEOBJECT_STATE, 1);
        for (k, v) in [p.at.0, p.at.1, z].iter().enumerate() {
            f.f32(field::GAMEOBJECT_POS_X + k as u16, *v);
        }
        f.f32(field::GAMEOBJECT_FACING, p.facing)
            .u32(field::GAMEOBJECT_FACTION, PICKUP_FACTION)
            .u32(field::GAMEOBJECT_TYPE_ID, if chair { GO_CHAIR } else { GO_GENERIC });
        ObjectUpdate::CreatePlaced { guid, type_id: ObjectTypeId::GameObject, position: [p.at.0, p.at.1, z], facing: p.facing, fields: f }
    }

    /// Hero `saved` (simulation id `id`), standing at plot `plot`'s tent of `place` facing out of
    /// it: its player look, one to talk to.
    fn visitor_object(&self, place: CampPlace, id: u32, plot: usize, saved: &SavedHero) -> Option<ObjectUpdate> {
        let (at, facing) = place.hero_stand(plot)?;
        let mut hero = self.run.heroes.first()?.clone();
        hero.id = id;
        hero.pos = at;
        hero.height = self.camp_ground(place, at);
        hero.facing = facing;
        let content = &self.run.content;
        hero.class = content.classes.iter().position(|c| c.id == saved.class)?;
        hero.race = content.races.iter().position(|r| r.id == saved.race)?;
        let ObjectUpdate::Create { guid, type_id, living, mut fields, .. } = self.hero_object(&hero, saved, false) else {
            return None;
        };
        fields.u32(field::UNIT_NPC_FLAGS, NPC_GOSSIP);
        // the other side's hero is no foe, one to talk to and take over
        if self.camp.as_ref().is_some_and(|c| c.place.faction != place.faction) {
            fields.u32(field::UNIT_FACTIONTEMPLATE, OTHER_SIDE_HERO_FACTION);
        }
        // its weapons put away at its tent (drawn only to drill, handle the rack's or clean a blade)
        fields.bytes(field::UNIT_BYTES_2, [0, 0, 0, 0]);
        Some(ObjectUpdate::Create { guid, type_id, living, fields, own: false })
    }

    /// The hero's client moved it in the camp: it goes where it went, but never past the camp's edge
    /// (put back where it last stood inside). Returns what its own client is told.
    pub(super) fn camp_moved(&mut self, hero: u32, position: [f32; 3], facing: f32) -> Vec<Packet> {
        let Some(camp) = &self.camp else {
            return Vec::new();
        };
        let middle = camp.place.middle();
        let inside = (position[0] - middle.0).hypot(position[1] - middle.1) <= camp.place.layout.reach;
        if inside {
            if let Some(c) = &mut self.camp {
                c.last_inside.insert(hero, (position[0], position[1], position[2]));
            }
            if let Some(h) = self.run.heroes.iter_mut().find(|h| h.id == hero) {
                h.pos = (position[0], position[1]);
                h.height = position[2];
                h.facing = facing;
            }
            return Vec::new();
        }
        let back = camp.last_inside.get(&hero).copied().unwrap_or_else(|| {
            let ((x, y), _) = camp.place.hero_stand(camp.own_plot).unwrap_or((middle, 0.0));
            (x, y, camp.place.spot.2)
        });
        let Some(p) = self.players.iter().find(|p| p.hero == hero) else {
            return Vec::new();
        };
        self.next_teleport += 1;
        vec![packets::teleport(p.guid(), self.next_teleport, [back.0, back.1, back.2], facing)]
    }

    /// Who the hero is, in place of a run's `Ready`.
    pub(super) fn camp_hello(&self, hero: u32) -> Option<ToClient> {
        self.camp.as_ref()?;
        Some(ToClient::Camp { hero: self.hero_guid(hero) })
    }

    /// The hero `hero` talks to `speaker` (a hero at its tent) or uses it (a thing of the camp).
    /// Returns what its client is told.
    pub(super) fn camp_talk(&mut self, hero: u32, speaker: u64) -> Vec<Packet> {
        let Some(camp) = &self.camp else {
            return Vec::new();
        };
        let Some(me) = self.run.heroes.iter().find(|h| h.id == hero) else {
            return Vec::new();
        };
        let near = |at: (f32, f32)| ((at.0 - me.pos.0).powi(2) + (at.1 - me.pos.1).powi(2)).sqrt() <= REACH;
        let content = self.run.content.clone();
        // a hero at its tent, of this side (where it stands now, at its tent or away) or of the
        // other (always at its tent)
        let mine = camp.visitors.iter().enumerate().find(|(_, (_, s))| player_guid(s) == speaker).map(|(i, (_, s))| (s, camp.life.hero_at(i, camp.ticked)));
        let theirs = || camp.neighbours.iter().find(|(_, s, _)| player_guid(s) == speaker).map(|(_, s, p)| (s, camp.other.hero_stand(*p).map(|(at, _)| at)));
        if let Some((saved, at)) = mine.or_else(theirs) {
            if !at.is_some_and(near) {
                return Vec::new();
            }
            let words = greeting(&content, saved);
            let options = vec![
                (format!("Take over as {}. <Switch hero>", saved.name), CampChoice::TakeOver(saved.guid)),
                ("Goodbye.".to_string(), CampChoice::Goodbye),
            ];
            return self.camp_window(speaker, words, options);
        }
        // a person of the camp: a WoW talk window whose option opens the game's own window
        if let Some(&(keeper, s)) = camp.place.layout.keepers.iter().find(|(k, _)| self.keeper_guid(*k) == speaker) {
            // Wren walks: her own window, reached where she is now
            if keeper == Keeper::Wren {
                return self.wren_talk(hero);
            }
            if !near(camp.place.at(s)) {
                return Vec::new();
            }
            let placing = camp.placing;
            let faction = camp.place.faction;
            self.tour_did(hero, Did::Talked(keeper));
            let words = keeper_words(faction, keeper);
            let first = match keeper {
                Keeper::Trainer => Some(("I need to change my specialization. <Open Talents>".to_string(), CampChoice::Open("talents"))),
                Keeper::Merchant => Some(("Let me see your wares. <Open Shop>".to_string(), CampChoice::Open("shop"))),
                Keeper::Chronicler => Some(("Show me what you have chronicled. <Open Achievements>".to_string(), CampChoice::Open("achievements"))),
                // the placing mode, on or off
                Keeper::Planner => {
                    let switch = if placing { "I am done decorating. <Placing mode off>" } else { "Show me where things can go. <Placing mode on>" };
                    Some((switch.to_string(), CampChoice::Placing(!placing)))
                }
                // words only: the Orb and the War Table keep their own menus
                Keeper::OrbKeeper | Keeper::Marshal => None,
                Keeper::Wren => unreachable!("Wren's window is her own, above"),
            };
            let options = first.into_iter().chain([("Goodbye.".to_string(), CampChoice::Goodbye)]).collect();
            return self.camp_window(speaker, words.into(), options);
        }
        // Chromie: a new hero, her words, her lesson again; at its end, the first run
        if let Some((s, _)) = camp.place.layout.chromie.filter(|_| speaker == super::unit_guid(CHROMIE_ENTRY, 1)) {
            if !near(camp.place.at(s)) {
                return Vec::new();
            }
            return self.chromie_talk(hero, None);
        }
        // Anachronos: his pages of lore
        if let Some(s) = camp.place.layout.anachronos.filter(|_| speaker == super::unit_guid(ANACHRONOS_ENTRY, 1)) {
            if !near(camp.place.at(s)) {
                return Vec::new();
            }
            return self.anachronos_page(speaker, None);
        }
        // a chair: the hero sits on it, facing as it does
        let chair = camp.pieces.iter().enumerate().find(|(k, p)| p.display == CHAIR && gameobject_guid(p.entry, *k as u32 + 1) == speaker);
        if let Some((_, p)) = chair {
            if !near(p.at) {
                return Vec::new();
            }
            let (at, facing) = ((p.at.0 - CHAIR_SEAT_BACK * p.facing.cos(), p.at.1 - CHAIR_SEAT_BACK * p.facing.sin()), p.facing);
            let z = self.camp_ground(camp.place, p.at);
            let Some(guid) = self.players.iter().find(|q| q.hero == hero).map(|q| q.guid()) else {
                return Vec::new();
            };
            if let Some(h) = self.run.heroes.iter_mut().find(|h| h.id == hero) {
                h.pos = at;
                h.height = z;
                h.facing = facing;
            }
            self.next_teleport += 1;
            return vec![packets::teleport(guid, self.next_teleport, [at.0, at.1, z], facing), packets::stand_state_update(STAND_SIT_CHAIR)];
        }
        // a thing of the camp
        let Some(&(service, s)) = camp.place.layout.services.iter().find(|(t, _)| t.guid(camp.place.faction) == speaker) else {
            return Vec::new();
        };
        if !near(camp.place.at(s)) {
            return Vec::new();
        }
        // a thing opens its window of the game's own (never a talk window)
        let window = service.window();
        self.tour_did(hero, Did::Opened(window));
        let told = vec![(hero, ToClient::CampOpen(window.into()))];
        self.lines(told).into_iter().map(|(_, p)| p).collect()
    }

    /// Opens a talk window of `speaker` saying `words` (its pointers, `{Area(3428)}`, shown as the
    /// client's names), with `options`.
    /// What an option does, in its <brackets>, is drawn blue.
    fn camp_window(&mut self, speaker: u64, words: String, options: Vec<(String, CampChoice)>) -> Vec<Packet> {
        let words = self.run.content.shown(&words);
        let Some(camp) = &mut self.camp else {
            return Vec::new();
        };
        camp.life.talked(camp.ticked);
        let text_id = camp.texts.len() as u32 + 1;
        camp.texts.insert(text_id, words.clone());
        let labels: Vec<String> = options.iter().map(|(l, _)| blue_brackets(l)).collect();
        camp.menus.insert(speaker, options.into_iter().map(|(_, c)| c).collect());
        vec![packets::npc_text_update(text_id, &words), packets::gossip_message(speaker, text_id, &labels)]
    }

    /// The words of a talk window's text, asked for by the client.
    pub(super) fn camp_text(&self, text_id: u32) -> Vec<Packet> {
        let words = self.camp.as_ref().and_then(|c| c.texts.get(&text_id)).cloned().unwrap_or_default();
        vec![packets::npc_text_update(text_id, &words)]
    }

    /// The hero picked option `option` of `speaker`'s talk window.
    pub(super) fn camp_pick(&mut self, hero: u32, speaker: u64, option: u32) -> Vec<Packet> {
        let choice = self
            .camp
            .as_ref()
            .and_then(|c| c.menus.get(&speaker))
            .and_then(|m| m.get(option as usize))
            .cloned();
        let mut out = vec![packets::gossip_complete()];
        let leave = match choice {
            Some(CampChoice::TakeOver(guid)) => Some(u64::from(guid)),
            Some(CampChoice::Open(window)) => {
                self.tour_did(hero, Did::Opened(window));
                let told = vec![(hero, ToClient::CampOpen(window.into()))];
                out.extend(self.lines(told).into_iter().map(|(_, p)| p));
                None
            }
            Some(CampChoice::Placing(on)) => {
                if let Some(camp) = &mut self.camp {
                    camp.placing = on;
                }
                let words = if on { "There. Every spot that will hold something, ringed in gold." } else { "I suppose you are done for now." };
                out.push(packets::monster_say(speaker, self.keeper_name(Keeper::Planner), words));
                out.push(packets::emote(speaker, packets::EMOTE_TALK));
                let told: Vec<_> = self.camp_decoration_spots().into_iter().map(|m| (hero, m)).collect();
                out.extend(self.lines(told).into_iter().map(|(_, p)| p));
                None
            }
            Some(CampChoice::Tour(choice)) => {
                out.extend(self.tour_pick(hero, choice));
                None
            }
            Some(CampChoice::Lore(page)) => {
                out.extend(self.anachronos_page(speaker, page));
                None
            }
            Some(CampChoice::Chromie(page)) => {
                out.extend(self.chromie_talk(hero, page));
                None
            }
            Some(CampChoice::Goodbye) | None => {
                self.tour_closed(hero);
                None
            }
        };
        if let Some(enter) = leave {
            let told = vec![(hero, ToClient::CampLeave { enter })];
            out.extend(self.lines(told).into_iter().map(|(_, p)| p));
        }
        out
    }

    /// `.gps`: where the hero stands, as a world point (map, x, y, z, facing) and, in a camp, as the
    /// layout's spot (`spot(east, north, faces)`, yards from the anchor spot, degrees from north).
    pub(super) fn gps(&self, hero: u32) -> Vec<Packet> {
        let Some(h) = self.run.heroes.iter().find(|h| h.id == hero) else {
            return Vec::new();
        };
        let map = self.run.content.zones.get(self.run.zone).map_or(0, |z| z.map_id);
        let mut lines = vec![format!(
            "Map {map}  X {:.2}  Y {:.2}  Z {:.2}  Orientation {:.2}",
            h.pos.0, h.pos.1, h.height, h.facing
        )];
        if let Some(camp) = &self.camp {
            let s = camp.place.spot;
            let (east, north) = (s.1 - h.pos.1, h.pos.0 - s.0);
            let faces = (-h.facing.to_degrees()).rem_euclid(360.0);
            lines.push(format!("Camp spot({east:.1}, {north:.1}, {faces:.0})"));
        }
        lines.iter().map(|l| packets::system_message(l)).collect()
    }

    /// The hero's gear changed on the hero sheet in the camp: what it wears is read again from the
    /// save and its client shown it (every visible slot, the emptied ones too). `false` outside a
    /// camp.
    pub(super) fn camp_dress(&mut self, hero: u32) -> bool {
        if self.camp.is_none() {
            return false;
        }
        let Some(saved) = self.saved(hero).cloned() else {
            return false;
        };
        let dressed = self.dress(&saved);
        let mut f = Fields::new();
        for (slot, &entry) in dressed.shown.iter().enumerate() {
            let at = field::PLAYER_VISIBLE_ITEM_1_0 + slot as u16 * field::PLAYER_VISIBLE_ITEM_STRIDE;
            f.u32(at, entry);
            f.u32(at + 1, if (15..=17).contains(&slot) { dressed.enchant.unwrap_or(0) } else { 0 });
        }
        self.dressed.insert(hero, dressed);
        let guid = self.hero_guid(hero);
        self.send_to(hero, &[crate::update::update_object(&[ObjectUpdate::Values { guid, fields: f }])]);
        true
    }

    /// The camp's decoration spots, for the client to ring and offer Decorate (F) at: the shared
    /// ones by the fire (`fire_1` ...), then each tent's three (`tent_<plot>_<1..3>`), on the
    /// ground. `None` outside a camp.
    pub(super) fn camp_decoration_spots(&self) -> Option<ToClient> {
        let camp = self.camp.as_ref()?;
        // none shown out of the placing mode
        if !camp.placing {
            return Some(ToClient::DecorationSpots(Vec::new()));
        }
        let place = camp.place;
        let on = self.hub_decorations(place);
        let content = &self.run.content;
        let fire_choices = self.saves.achievements(content).hub_choices(content);
        let tent_choices: Vec<Vec<String>> = self.plot_heroes(place).iter().map(|h| h.as_ref().map(|h| self.saves.plot_choices(content, h)).unwrap_or_default()).collect();
        let spots = decoration_spots(place)
            .into_iter()
            .map(|(id, s)| {
                let (x, y) = place.at(s);
                let piece = on.get(&id).cloned();
                let choices = match tent_spot(&id) {
                    Some((p, _)) => tent_choices.get(p).cloned().unwrap_or_default(),
                    None => fire_choices.clone(),
                };
                (id, [x, y, self.camp_ground(place, (x, y))], piece, choices)
            })
            .collect();
        Some(ToClient::DecorationSpots(spots))
    }

    /// The hero at each plot's tent, as the save keeps it now: the hero walking the camp at its own,
    /// the others at theirs; `None` for a plot with no hero.
    fn plot_heroes(&self, place: CampPlace) -> Vec<Option<SavedHero>> {
        let Some(camp) = &self.camp else {
            return Vec::new();
        };
        let mut guids = vec![None; place.layout.plots.len()];
        if let Some(g) = guids.get_mut(camp.own_plot) {
            *g = Some(camp.own);
        }
        for ((_, s), &p) in camp.visitors.iter().zip(&camp.visitor_plots) {
            if let Some(g) = guids.get_mut(p) {
                *g = Some(s.guid);
            }
        }
        guids.into_iter().map(|g| g.and_then(|g| self.saves.hero(g))).collect()
    }

    /// What stands on the camp's decoration spots, by spot, as the save keeps it: the fire's
    /// shared ones, and each tent's its hero's.
    fn hub_decorations(&self, place: CampPlace) -> BTreeMap<String, String> {
        let content = &self.run.content;
        let mut on = self.saves.achievements(content).hub_told(content, place.camp);
        for (p, hero) in self.plot_heroes(place).iter().enumerate() {
            for (k, piece) in hero.iter().flat_map(|h| self.saves.plot_told(content, h)) {
                on.insert(format!("tent_{}_{k}", p + 1), piece);
            }
        }
        on
    }

    /// The decorations standing on the camp's spots, as creates.
    fn decoration_objects(&self, place: CampPlace) -> Vec<ObjectUpdate> {
        let on = self.hub_decorations(place);
        decoration_spots(place)
            .into_iter()
            .enumerate()
            .filter_map(|(k, (id, s))| self.decoration_object(place, k, s, on.get(&id)?))
            .collect()
    }

    /// Decoration `piece` standing on the `k`th decoration spot `s`, facing as the spot does, turned
    /// as the piece is.
    fn decoration_object(&self, place: CampPlace, k: usize, s: Spot, piece: &str) -> Option<ObjectUpdate> {
        let content = &self.run.content;
        let d = content.decoration(piece)?;
        let faction = crate::save::camp_faction(place.camp)?;
        let (display, scale) = self.saves.achievements(content).with_holdings(|h| content.decoration_look(d, faction, h));
        let turned = Spot { faces: s.faces + d.turn, ..s };
        let p = Piece { entry: CAMP_PIECES_FROM + display, display, look: None, at: place.at(s), facing: place.facing(turned), scale, prop: None };
        Some(self.piece_object(place, &p, DECORATION_PIECES_FROM + k))
    }

    /// The hero set `piece` out on decoration spot `spot` (`None`: cleared it): kept in the save,
    /// the piece that stood there taken away and the new one put down, and the spots told again.
    /// False when refused (outside a camp, an unknown spot, a piece that may not stand there).
    pub(super) fn camp_decorate(&mut self, hero: u32, spot: &str, piece: Option<&str>) -> bool {
        let Some(place) = self.camp.as_ref().map(|c| c.place) else {
            return false;
        };
        let spots = decoration_spots(place);
        if !spots.iter().any(|(id, _)| id == spot) {
            return false;
        }
        let before = self.hub_decorations(place);
        let done = match tent_spot(spot) {
            // a tent's: its hero's, kept with the hero
            Some((p, k)) => {
                let hero = self.plot_heroes(place).get(p).cloned().flatten();
                hero.is_some_and(|h| self.saves.decorate_plot(&self.run.content, h.guid, &k.to_string(), piece))
            }
            None => self.saves.decorate_hub(&self.run.content, place.camp, spot, piece),
        };
        if !done {
            return false;
        }
        let after = self.hub_decorations(place);
        // every spot whose piece changed (a piece set out elsewhere moves off its old spot)
        let mut packets = Vec::new();
        for (j, (id, s)) in spots.iter().enumerate() {
            let (was, now) = (before.get(id), after.get(id));
            if was == now {
                continue;
            }
            if let Some(ObjectUpdate::CreatePlaced { guid, .. }) = was.and_then(|w| self.decoration_object(place, j, *s, w)) {
                packets.push(crate::packets::destroy_object(guid));
            }
            if let Some(new) = now.and_then(|n| self.decoration_object(place, j, *s, n)) {
                packets.push(crate::update::update_object(&[new]));
            }
        }
        self.send_to(hero, &packets);
        // the heroes walk round what was set out, and to a training dummy set out
        let (haunts, walls) = self.camp_walks();
        if let Some(camp) = &mut self.camp {
            camp.life.change_ways(haunts, walls);
        }
        self.work_out_hero_ways();
        true
    }

    /// The hero sets off from the camp on a run in the zone chosen for it: the world server is
    /// told its next time in is the run, and its client to come back in. `None` outside a camp.
    pub(super) fn camp_run(&mut self, hero: u32) -> Option<ToClient> {
        let seen_off = self.tour_seen_off(hero);
        self.send_to(hero, &seen_off);
        let camp = self.camp.as_ref()?;
        let saved = self.saved(hero)?;
        camp.runs_asked.lock().unwrap().insert(saved.guid);
        Some(ToClient::CampLeave { enter: player_guid(saved) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_plot_has_its_tent_turn() {
        // a plot without a turn would lose its tent: pieces() zips the two
        for place in [NEUTRAL_ALLIANCE_CAMP, NEUTRAL_HORDE_CAMP] {
            assert_eq!(place.layout.plots.len(), place.layout.tent_turns.len(), "{}", place.camp);
        }
    }

    #[test]
    fn a_spot_east_and_north_is_the_world_point_there() {
        let place = NEUTRAL_ALLIANCE_CAMP;
        // the world's x runs north and its y west: 22 east, 16 north of the anchor spot
        let (x, y) = place.at(spot(22.0, 16.0, 0.0));
        assert!((x - -8190.09).abs() < 1e-3 && (y - -4610.25).abs() < 1e-3, "{x} {y}");
        // facing north is the world's 0, facing east a quarter turn the other way
        assert!(place.facing(spot(0.0, 0.0, 0.0)).abs() < 1e-6);
        let east = place.facing(spot(0.0, 0.0, 90.0));
        assert!((east - 1.5 * std::f32::consts::PI).abs() < 1e-5, "{east}");
    }

    #[test]
    fn every_thing_of_the_camp_is_within_its_reach_of_its_middle() {
        for place in [NEUTRAL_ALLIANCE_CAMP, NEUTRAL_HORDE_CAMP] {
            let middle = place.middle();
            let layout = place.layout;
            let spots = layout
                .plots
                .iter()
                .copied()
                .chain(layout.services.iter().map(|(_, s)| *s))
                .chain(layout.keepers.iter().map(|(_, s)| *s))
                .chain(layout.props.iter().map(|p| p.spot))
                .chain(layout.banners)
                .chain(layout.trophies)
                .chain([layout.head])
                .chain(layout.decorations.iter().map(|(_, s)| *s))
                .chain((0..layout.plots.len()).flat_map(|i| layout.plot_decoration_spots(i)));
            for s in spots {
                let (x, y) = place.at(s);
                assert!((x - middle.0).hypot(y - middle.1) < layout.reach, "{s:?}");
            }
        }
    }

    #[test]
    fn the_camp_zone_has_no_enemies_and_stands_at_the_camp() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).unwrap();
        let (own, i) = camp_content(&content, NEUTRAL_ALLIANCE_CAMP, CAMP_HOUR).unwrap();
        let z = &own.zones[i];
        assert_eq!((z.id.as_str(), z.map_id), (CAMP_ZONE, 1));
        assert!(z.enemies.is_empty() && z.bosses.is_empty());
        assert_eq!(own.zones.len(), content.zones.len() + 1);
    }

    #[test]
    fn the_things_to_use_are_people_wearing_their_objects_and_the_rest_objects() {
        let table = camp_creature_info(Service::WarTable.entry(Faction::Alliance)).unwrap();
        assert_eq!((table.name.as_str(), table.subname.as_str()), ("War Table", "Zones"));
        assert_eq!(table.display_id, camp_thing_look(CAMP_WAR_MAP_TABLE_MODEL));
        assert!(camp_object_info(Service::WarTable.entry(Faction::Alliance)).is_none());
        // the people by their own names, their jobs under them; the old entries
        // named by job are never reused
        let maribel = camp_creature_info(Keeper::Merchant.entry(Faction::Alliance)).unwrap();
        assert_eq!((maribel.name.as_str(), maribel.subname.as_str(), maribel.display_id), ("Maribel Thorne", "Merchant", MERCHANT_DISPLAY));
        assert_eq!(camp_creature_info(Keeper::Trainer.entry(Faction::Alliance)).unwrap().name, "Thurgen Ironbrow");
        assert_eq!(camp_creature_info(Keeper::Chronicler.entry(Faction::Alliance)).unwrap().name, "Tobbin Quillsprocket");
        assert_eq!(camp_creature_info(Keeper::Marshal.entry(Faction::Alliance)).unwrap().display_id, CAMP_MARSHAL_LOOK);
        assert!(camp_creature_info(CAMP_KEEPERS_FROM).is_none() && camp_creature_info(CAMP_KEEPERS_FROM + 1).is_none());
        assert!(camp_creature_info(CAMP_KEEPERS_FROM + 100 + 2).is_none());
        assert_eq!(camp_object_info(CAMP_PIECES_FROM + FIRE).unwrap().type_id, GO_GENERIC);
        assert!(camp_object_info(1).is_none());
    }

    fn hero(guid: u32, name: &str, class: &str, race: &str) -> SavedHero {
        SavedHero {
            guid,
            name: name.into(),
            class: class.into(),
            race: race.into(),
            gender: 0,
            look: [0; 5],
            records: Default::default(),
            zone: None,
            start: None,
            companion: None,
            wears: Default::default(),
            plot: None,
            temperament: None,
            time_period: Default::default(),
        }
    }

    /// A hero's tent shows its class's free piece beside it until first decorated; its spots take
    /// only what the hero may have (its class's piece, the tent pieces the warband owns), each kept
    /// with the hero.
    #[test]
    fn a_tent_shows_its_heros_class_piece_and_takes_only_its_things() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let (own, zone) = camp_content(&Content::load(dir).unwrap(), NEUTRAL_ALLIANCE_CAMP, CAMP_HOUR).unwrap();
        let saves = Arc::new(crate::save::Saves::in_memory());
        let me = saves.create("Aria", "paladin", "human", 1, [0; 5]).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(own, None, zone, &me, saves.clone(), inputs).unwrap();
        game.make_camp(NEUTRAL_ALLIANCE_CAMP, me.guid, &[], Default::default());
        let id = game.hero_id(0);
        // the candles (Candle02) as the object on a spot of the first tent
        let spots = decoration_spots(NEUTRAL_ALLIANCE_CAMP);
        let on = |spot: &str| {
            let k = spots.iter().position(|(id, _)| id == spot).unwrap();
            gameobject_guid(CAMP_PIECES_FROM + 6363, (DECORATION_PIECES_FROM + k) as u32 + 1)
        };
        let stands = |game: &Game, spot: &str| game.camp_now().iter().any(|o| matches!(o, ObjectUpdate::CreatePlaced { guid, .. } if *guid == on(spot)));
        assert!(stands(&game, "tent_1_3"), "the paladin's candles beside its tent");
        assert!(!game.camp_decorate(id, "tent_1_1", Some("warrior_crate")), "another class's");
        assert!(!game.camp_decorate(id, "tent_1_1", Some("camp_chair")), "not bought");
        assert!(game.camp_decorate(id, "tent_1_1", Some("paladin_candles")), "moved to the front");
        assert_eq!(saves.hero(me.guid).unwrap().plot.unwrap().into_iter().collect::<Vec<_>>(), [("1".to_string(), "paladin_candles".to_string())]);
        assert!(stands(&game, "tent_1_1") && !stands(&game, "tent_1_3"));
        assert!(game.camp_decorate(id, "tent_1_1", None));
        assert!(!stands(&game, "tent_1_1") && !stands(&game, "tent_1_3"), "taken away");
    }

    /// Time Floes bought, the hero runs 60% faster in the hub in Tanaris.
    #[test]
    fn time_floes_speeds_the_hero_in_the_hub_only() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let mut content = Content::load(dir).unwrap();
        // free here, so the empty save can buy it
        content.shop.hub_buffs[0].price = 0;
        let saves = Arc::new(crate::save::Saves::in_memory());
        let me = saves.create("Aria", "paladin", "human", 1, [0; 5]).unwrap();
        let speed = |place: CampPlace| {
            let (own, zone) = camp_content(&content, place, CAMP_HOUR).unwrap();
            let (_host, inputs) = std::sync::mpsc::channel();
            let mut game = Game::new(own, None, zone, &me, saves.clone(), inputs).unwrap();
            game.make_camp(place, me.guid, &[], Default::default());
            game.run.heroes[0].stats.run_speed()
        };
        let hub = speed(NEUTRAL_ALLIANCE_CAMP);
        assert!(saves.buy(&content, &esurv_content::hub_buff_item("time_floes")));
        assert!((speed(NEUTRAL_ALLIANCE_CAMP) - hub * 1.6).abs() < 1e-4, "60% faster in the hub");
    }

    /// The Alliance's half of the camp, its hero Aria and Brom at his tent.
    pub(super) fn a_camp() -> (Game, u32) {
        camp_at(NEUTRAL_ALLIANCE_CAMP)
    }

    /// A camp at `place`, its hero Aria and Brom at his tent.
    pub(super) fn camp_at(place: CampPlace) -> (Game, u32) {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let (own, zone) = camp_content(&Content::load(dir).unwrap(), place, CAMP_HOUR).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let saves = Arc::new(crate::save::Saves::in_memory());
        let me = hero(1, "Aria", "paladin", "human");
        let mut game = Game::new(own, None, zone, &me, saves, inputs).unwrap();
        game.make_camp(place, 1, &[hero(2, "Brom", "warrior", "dwarf")], Default::default());
        let id = game.hero_id(0);
        (game, id)
    }

    /// Each side's tents stand only for its heroes, in the warband's order, the hero walking the camp
    /// arrives at its own, and the other side's heroes are in the same camp.
    #[test]
    fn tents_stand_for_each_sides_heroes_and_the_hero_arrives_at_its_own() {
        let place = NEUTRAL_HORDE_CAMP;
        let other = other_side(place);
        assert!(place.layout.plots.len() > 4 && other.layout.plots.len() > 2, "plots to stand empty");
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let (own, zone) = camp_content(&Content::load(dir).unwrap(), place, CAMP_HOUR).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let me = hero(4, "Grukk", "warrior", "orc");
        let warband = [
            hero(1, "Zul", "shaman", "troll"),
            hero(2, "Aria", "paladin", "human"),
            hero(3, "Mok", "hunter", "tauren"),
            hero(5, "Brom", "warrior", "dwarf"),
            hero(6, "Tharg", "rogue", "orc"),
            me.clone(),
        ];
        let mut game = Game::new(own, None, zone, &me, Arc::new(crate::save::Saves::in_memory()), inputs).unwrap();
        game.make_camp(place, me.guid, &warband, Default::default());
        let camp = game.camp.as_ref().unwrap();
        let stands = |at: (f32, f32)| camp.pieces.iter().any(|p| p.at == at);
        let tents = |side: CampPlace| tent_pieces(side, side.layout.plots.len()).iter().map(|t| stands(t.at)).collect::<Vec<_>>();
        let mut horde = vec![false; place.layout.plots.len()];
        horde[..4].fill(true);
        let mut alliance = vec![false; other.layout.plots.len()];
        alliance[..2].fill(true);
        assert_eq!((tents(place), tents(other)), (horde, alliance), "the Horde's four tents and the Alliance's two");
        assert_eq!(camp.own_plot, 3);
        assert_eq!(camp.visitor_plots, [0, 1, 2]);
        let names = |hs: Vec<&SavedHero>| hs.iter().map(|h| h.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(camp.visitors.iter().map(|(_, s)| s).collect()), ["Zul", "Mok", "Tharg"]);
        assert_eq!(names(camp.neighbours.iter().map(|(_, s, _)| s).collect()), ["Aria", "Brom"]);
        let (at, _) = place.hero_stand(3).unwrap();
        assert_eq!(game.run.heroes[0].pos, at, "at the fourth tent");
        let created = game.camp_now();
        for (_, saved, _) in &camp.neighbours {
            let guid = crate::game::player_guid(saved);
            assert!(created.iter().any(|o| matches!(o, ObjectUpdate::Create { guid: g, .. } if *g == guid)), "the Alliance's heroes there");
        }
    }

    /// The other side's heroes are no foes: talked to at their tents, one is taken over, and the
    /// hero taken over arrives at its own tent in its own side's camp.
    #[test]
    fn an_other_side_hero_is_talked_to_and_taken_over_at_its_tent() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).unwrap();
        let me = hero(1, "Aria", "paladin", "human");
        let warband = [me.clone(), hero(2, "Zul", "shaman", "troll"), hero(3, "Mok", "hunter", "tauren")];
        let camp_for = |saved: &SavedHero, place: CampPlace| {
            let (own, zone) = camp_content(&content, place, CAMP_HOUR).unwrap();
            let (_host, inputs) = std::sync::mpsc::channel();
            let mut game = Game::new(own, None, zone, saved, Arc::new(crate::save::Saves::in_memory()), inputs).unwrap();
            game.make_camp(place, saved.guid, &warband, Default::default());
            game
        };
        let mut game = camp_for(&me, NEUTRAL_ALLIANCE_CAMP);
        let id = game.hero_id(0);
        let mok = player_guid(&warband[2]);
        let created = game.camp_now();
        let faction = created.iter().find_map(|o| match o {
            ObjectUpdate::Create { guid, fields, .. } if *guid == mok => fields.get(field::UNIT_FACTIONTEMPLATE),
            _ => None,
        });
        assert_eq!(faction, Some(OTHER_SIDE_HERO_FACTION), "no foe");
        let (at, _) = NEUTRAL_HORDE_CAMP.hero_stand(1).unwrap();
        walk_to(&mut game, (at.0 + 1.0, at.1));
        assert!(!game.camp_talk(id, mok).is_empty(), "talked to at its tent");
        assert_eq!(game.camp.as_ref().unwrap().menus[&mok][0], CampChoice::TakeOver(3));
        // taken over, it comes into the Horde's half at its own tent, the Horde's second
        let game = camp_for(&warband[2], NEUTRAL_HORDE_CAMP);
        assert_eq!(game.run.heroes[0].pos, at);
    }

    /// A short line is followed by the shortest pause, a long one by the longest, and those
    /// between by one between.
    #[test]
    fn a_longer_line_is_followed_by_a_longer_pause() {
        let gap = |n: usize| line_gap(&"a".repeat(n)).as_secs_f32();
        assert_eq!((gap(5), gap(400)), LINE_GAPS);
        assert!(LINE_GAPS.0 < gap(100) && gap(100) < LINE_GAPS.1);
    }

    /// The hero walked up to `at`.
    pub(super) fn walk_to(game: &mut Game, at: (f32, f32)) {
        game.run.heroes[0].pos = at;
    }

    #[test]
    fn each_thing_of_the_camp_opens_its_window_where_it_stands() {
        for place in [NEUTRAL_ALLIANCE_CAMP, NEUTRAL_HORDE_CAMP] {
            let (mut game, me) = camp_at(place);
            let things = game.camp_now();
            for &(service, s) in place.layout.services {
                let window = service.window();
                let guid = service.guid(place.faction);
                assert!(
                    things.iter().any(|t| matches!(t, ObjectUpdate::Create { guid: g, type_id: ObjectTypeId::Unit, .. } if *g == guid)),
                    "{}: {service:?} stands in the camp as a person",
                    place.camp
                );
                let at = place.at(s);
                let opens: Vec<Packet> = game.lines(vec![(me, ToClient::CampOpen(window.into()))]).into_iter().map(|(_, p)| p).collect();
                walk_to(&mut game, (at.0 + 40.0, at.1));
                assert!(game.camp_talk(me, guid).is_empty(), "{}: {service:?} is out of reach from afar", place.camp);
                walk_to(&mut game, (at.0 + 1.5, at.1));
                assert_eq!(game.camp_talk(me, guid), opens, "{}: {service:?} opens {window}", place.camp);
            }
        }
    }

    /// A dev aid, to plan the camp on even ground: prints the ground's height every 3 yards over
    /// the camp (yards over the anchor spot), from the player's own data (`WOW_DATA`).
    /// `cargo test -p esurv-server the_camp_ground -- --ignored --nocapture`
    #[test]
    #[ignore = "reads the player's own WoW data"]
    fn the_camp_ground() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Content::load(dir).unwrap();
        let data = crate::ground::GameData::open(&benilla_formats::wow_data().unwrap(), &content).unwrap();
        // from `ESURV_GROUND_FROM=<east>,<north>`
        let place = NEUTRAL_ALLIANCE_CAMP;
        let from: Vec<i32> = std::env::var("ESURV_GROUND_FROM").ok().map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect()).unwrap_or_default();
        let (east_from, north_top) = (from.first().copied().unwrap_or(-15), from.get(1).copied().unwrap_or(49));
        let (own, zone) = camp_content(&content, place, CAMP_HOUR).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let saves = Arc::new(crate::save::Saves::in_memory());
        let game = Game::new(own, Some(&data), zone, &hero(1, "Aria", "paladin", "human"), saves, inputs).unwrap();
        let base = place.spot.2;
        print!("north\\east");
        let easts: Vec<i32> = (east_from..=east_from + 75).step_by(3).collect();
        for e in &easts {
            print!("{e:>6}");
        }
        println!();
        for n in (0..=26).map(|k| north_top - 3 * k) {
            print!("{n:>10}");
            for &e in &easts {
                let at = place.at(spot(e as f32, n as f32, 0.0));
                print!("{:>6.1}", game.camp_ground(place, at) - base);
            }
            println!();
        }
    }

    #[test]
    fn the_ground_slope_is_the_plane_through_the_points() {
        let points: Vec<(f32, f32, f32)> = (0..9)
            .map(|k| {
                let (x, y) = ((k % 3) as f32 - 1.0, (k / 3) as f32 - 1.0);
                (x, y, 0.5 * x - 0.25 * y + 3.0)
            })
            .collect();
        let (a, b) = ground_slope(&points);
        assert!((a - 0.5).abs() < 1e-5 && (b + 0.25).abs() < 1e-5);
    }

    #[test]
    fn a_turn_then_no_tilt_is_the_turn() {
        let turn = [0.0, 0.0, 0.6_f32.sin(), 0.6_f32.cos()];
        assert_eq!(quat_mul([0.0, 0.0, 0.0, 1.0], turn), turn);
    }

    #[test]
    fn each_person_of_the_camp_talks_then_opens_their_window() {
        let (mut game, me) = a_camp();
        for (keeper, window) in [(Keeper::Trainer, "talents"), (Keeper::Merchant, "shop"), (Keeper::Chronicler, "achievements")] {
            let s = NEUTRAL_ALLIANCE_CAMP.layout.keepers.iter().find(|(k, _)| *k == keeper).unwrap().1;
            let at = NEUTRAL_ALLIANCE_CAMP.at(s);
            let guid = game.keeper_guid(keeper);
            walk_to(&mut game, (at.0 + 1.5, at.1));
            // the WoW talk window: its words, then the window with its options
            let talk = game.camp_talk(me, guid);
            assert_eq!(talk.len(), 2, "{keeper:?} opens a talk window");
            let mut opens = vec![packets::gossip_complete()];
            opens.extend(game.lines(vec![(me, ToClient::CampOpen(window.into()))]).into_iter().map(|(_, p)| p));
            assert_eq!(game.camp_pick(me, guid, 0), opens, "{keeper:?}'s first option opens {window}");
        }
    }

    #[test]
    fn the_camp_planner_shows_the_decoration_spots_only_in_placing_mode() {
        let (mut game, me) = a_camp();
        let spots = |game: &Game| match game.camp_decoration_spots() {
            Some(ToClient::DecorationSpots(spots)) => spots.len(),
            _ => panic!("the camp tells its spots"),
        };
        assert_eq!(spots(&game), 0, "no rings out of the placing mode");
        let s = NEUTRAL_ALLIANCE_CAMP.layout.keepers.iter().find(|(k, _)| *k == Keeper::Planner).unwrap().1;
        let at = NEUTRAL_ALLIANCE_CAMP.at(s);
        let guid = game.keeper_guid(Keeper::Planner);
        walk_to(&mut game, (at.0 + 1.5, at.1));
        assert_eq!(game.camp_talk(me, guid).len(), 2, "the planner opens a talk window");
        game.camp_pick(me, guid, 0);
        assert!(spots(&game) > 0, "his first option turns the rings on");
        game.camp_talk(me, guid);
        game.camp_pick(me, guid, 0);
        assert_eq!(spots(&game), 0, "and then off");
    }
}
