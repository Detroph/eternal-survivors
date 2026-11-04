//! The Horde's side of the camp: its people, and its hubs as they stood in the Horde's own camp
//! on the Merchant Coast south of Ratchet (plans/hub-decisions.md 5, plans/hub-horde-camp.md),
//! which the neutral camp carries over whole (`neutral`).

use esurv_content::camp_looks::{
    CAMP_KARAZHAN_LARGE_SHELF_DISPLAY, CAMP_KARAZHAN_SMALL_SHELF_DISPLAY, CAMP_UNDEAD_PLATFORM_DISPLAY, CAMP_HORDE_KODO_HIDE_DISPLAY, CAMP_HORDE_KODO_RUG_DISPLAY, CAMP_HORDE_LARGE_BASKET_DISPLAY,
    CAMP_HORDE_MARKET_BLANKET_DISPLAY, CAMP_ORC_TENT_DISPLAY, CAMP_TROLL_SHOP_DISPLAY,
};

use super::{spot, Footprint, Keeper, Prop, PropLook, SetOn, Spot, PLANNER_MALLET, PLATFORM_DECK, PLATFORM_SUNK};

/// Its people's names (the advisor's, but Dargash Stonefist and Aldous Greymarch),
/// the job under each, and their looks (creature looks of the 1.12 data, bareheaded, of the race
/// each is: an orc drillmaster, a troll trader, a Forsaken scribe, a troll witch doctor, an orc
/// warlord, a tauren, an orc runner: Rezza, the Warsong Scout's look).
pub(super) fn horde_who(keeper: Keeper) -> Option<(&'static str, &'static str, u32)> {
    Some(match keeper {
        Keeper::Trainer => ("Dargash Stonefist", "Class Trainer", 1311),
        Keeper::Merchant => ("Jun'zabi", "Merchant", 3608),
        Keeper::Chronicler => ("Aldous Greymarch", "Achievements", 1578),
        Keeper::OrbKeeper => ("Zuli'ka", "Mask Keeper", 1882),
        // a rank, as Marshal Corwin Hale's: no subtitle
        Keeper::Marshal => ("Warlord Garnok Ashmaw", "", 1316),
        Keeper::Planner => ("Tohan Highmesa", "Camp Planner", 2083),
        Keeper::Wren => ("Rezza", "Guide", 11869),
    })
}

/// The Warlord's sword, held in hand: the High Warlord's Greatsword (Sword_2H_PVPHorde_A_01, item
/// look 31998), the Horde's twin of the Marshal's.
const WARLORD_SWORD: esurv_content::CreatureWeapons = esurv_content::CreatureWeapons {
    main_hand: Some(esurv_content::CreatureWeapon {
        kind: esurv_content::WeaponKind::TwoHandedSword,
        display: 31998,
        inventory_type: 17,
        sheath: 1,
        material: 1,
    }),
    off_hand: None,
    ranged: None,
};

/// What a person of the Horde camp holds in hand: the Warlord his greatsword, the planner the
/// Carpenter's Mallet as Lorendir does.
pub(super) fn horde_held_by(keeper: Keeper) -> Option<&'static esurv_content::CreatureWeapons> {
    match keeper {
        Keeper::Marshal => Some(&WARLORD_SWORD),
        Keeper::Planner => Some(&PLANNER_MALLET),
        _ => None,
    }
}

/// What a person of the Horde camp says as its talk window opens (the creative advisor's).
pub(super) fn horde_words(keeper: Keeper) -> &'static str {
    match keeper {
        Keeper::Merchant => "Everyt'ing on dis blanket come a long way, mon, and it all be lookin' for a new owner. Have a look.",
        Keeper::Trainer => "You came back with new scars. Good, you'll have learned something. Let's put it to use.",
        Keeper::Chronicler => {
            "I used to write down the Crusade's victories. Yours are better written, and shorter on lies. Your page is open."
        }
        Keeper::OrbKeeper => "Don't stare into da Mask too long, mon. It stare back.",
        Keeper::Marshal => "Choose your ground. I'll see you don't bleed on it for nothing.",
        Keeper::Planner => {
            "Earth Mother keep you, friend. I pitched camps across the Barrens for more summers than I can count. Tell me what you want set down, and I will show you where it fits."
        }
        Keeper::Wren => "",
    }
}

/// Gol'Bolar's platform, the Horde's, its stairs toward the fire in the ring's middle (bearing 201),
/// so it faces 90 less, as the Alliance's: turned 29 degrees about its origin from where it faced
/// the old fire, everything on it and at its foot turned with it. Its colour is the Karazhan bookshelves' frame.
pub(super) const HORDE_PLATFORM: Spot = spot(-3.1, 30.2, 111.13);
/// The appearance shop, trollshop01 at half its size, six yards nearer the fire
/// than the Mask stood: its
/// stairs toward the camp; the Mask, its chest and drum and Zuli'ka
/// on its floor.
pub(super) const HORDE_SHOP: Spot = spot(-21.5, 39.0, 180.0);
const HORDE_SHOP_SIZE: f32 = 0.5;
/// The shop's floor over its origin, 4.3 at size 1 (its WMO).
pub(super) const HORDE_SHOP_FLOOR: f32 = 4.3 * HORDE_SHOP_SIZE;
const ON_THE_SHOP_FLOOR: SetOn = SetOn { lift: HORDE_SHOP_FLOOR, rise_degrees: 0.0, ground_at: Some(HORDE_SHOP) };
/// The Records' table on the deck (DuskwoodTable01 at 0.675): its top, 1.17 at size 1.
/// 10% smaller than its first 0.75.
const HORDE_LIBRARY_TABLE_SIZE: f32 = 0.675;
const HORDE_LIBRARY_TABLE_TOP: f32 = 1.17 * HORDE_LIBRARY_TABLE_SIZE;
/// The War Table (HordeMapTable at 0.45), east between the world's own tents, its map toward the
/// camp, and its map's face: flat at 1.0 at size 1 (model_sizes: 41 square
/// yards there, the figures on it standing higher), so 0.45 at its 0.45.
pub(super) const HORDE_MAP_TABLE: Spot = spot(16.0, -5.3, 295.0);
const HORDE_MAP_TABLE_TOP: f32 = 1.0 * 0.45;

/// The Horde's hubs as they stood in its own camp on the Merchant Coast, its spot there the
/// layout's (0, 0): the neutral camp carries each over whole (`neutral`), its pieces in this
/// order, hub by hub.
pub(super) const HORDE_HUB_PIECES: [Prop; 43] = [
    // the War Table: a kodo-skin rug under it, its length along the way between the world's tents, a brazier behind the table and a drum by its front; it lies tilted on the sand and keeps the walkers off the table
    Prop {
        look: PropLook::Object(CAMP_HORDE_KODO_RUG_DISPLAY), // KotoSkinRug01, ours
        scale: 0.4,
        spot: spot(15.09, -4.88, 298.0),
        footprint: Some(Footprint { x: (-7.28, 6.35), y: (-4.53, 4.45) }),
        set_on: None,
    },
    Prop {
        look: PropLook::Object(655), // TallBrazier01
        scale: 0.6,
        spot: spot(17.92, -6.97, 295.0),
        footprint: None,
        set_on: None,
    },
    Prop {
        look: PropLook::Object(2572), // TaurenDrumMed01
        scale: 1.0,
        spot: spot(15.25, -2.41, 90.0),
        footprint: None,
        set_on: None,
    },
    // on the map, clear of its figures: two skull candles and a scroll
    Prop {
        look: PropLook::Object(6406), // SkullCandle01
        scale: 1.0,
        spot: spot(15.61, -5.61, 180.0),
        footprint: None,
        set_on: Some(SetOn { lift: HORDE_MAP_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(HORDE_MAP_TABLE) }),
    },
    Prop {
        look: PropLook::Object(6406), // SkullCandle01
        scale: 1.0,
        spot: spot(15.99, -4.8, 160.0),
        footprint: None,
        set_on: Some(SetOn { lift: HORDE_MAP_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(HORDE_MAP_TABLE) }),
    },
    Prop {
        look: PropLook::Object(164), // ScrollB01
        scale: 1.0,
        spot: spot(16.13, -5.46, 200.0),
        footprint: None,
        set_on: Some(SetOn { lift: HORDE_MAP_TABLE_TOP + 0.05, rise_degrees: 0.0, ground_at: Some(HORDE_MAP_TABLE) }),
    },
    // the Horde's stores behind the map, between the world's tents: a crate with a smaller one on it (its top 1.44 at size 1), a barrel and a sack
    Prop {
        look: PropLook::Object(5271), // OrcCrate06
        scale: 0.7,
        spot: spot(24.2, -5.8, 25.0),
        footprint: None,
        set_on: None,
    },
    Prop {
        look: PropLook::Object(5531), // OrcCrate07
        scale: 0.7,
        spot: spot(24.2, -5.8, 50.0),
        footprint: None,
        set_on: Some(SetOn { lift: 1.44 * 0.7, rise_degrees: 0.0, ground_at: Some(spot(24.2, -5.8, 25.0)) }),
    },
    Prop {
        look: PropLook::Object(457), // Orcbarrel01
        scale: 1.0,
        spot: spot(24.2, -8.1, 0.0),
        footprint: None,
        set_on: None,
    },
    Prop {
        look: PropLook::Object(323), // TaurenSack02
        scale: 1.0,
        spot: spot(20.5, -5.0, 250.0),
        footprint: None,
        set_on: None,
    },
    // the Market, out of the middle onto the ring's north-east, between the
    // Records and the Warlord, 33 yards from the fire, turned as one so its
    // trader faces the fire: a kodo hide on two poles, its high edge (+x) over the trader
    Prop {
        look: PropLook::Object(CAMP_HORDE_KODO_HIDE_DISPLAY), // TaurenKodoHide, ours
        scale: 0.45,
        spot: spot(11.4, 17.63, 232.0),
        footprint: None,
        set_on: None,
    },
    // the market blanket before him, goods on it: tilted on the sand, raised a tenth over its bumps (0.06 at most here), and keeping the walkers off it
    Prop {
        look: PropLook::Object(CAMP_HORDE_MARKET_BLANKET_DISPLAY), // MarketBlanket01, ours
        scale: 1.0,
        spot: spot(9.58, 16.21, 232.0),
        footprint: Some(Footprint { x: (-1.63, 1.63), y: (-1.75, 1.83) }),
        set_on: Some(SetOn { lift: 0.1, rise_degrees: 0.0, ground_at: None }),
    },
    Prop {
        look: PropLook::Object(5271), // OrcCrate06
        scale: 0.7,
        spot: spot(11.12, 13.92, 242.0),
        footprint: None,
        set_on: None,
    },
    Prop {
        look: PropLook::Object(323), // TaurenSack02
        scale: 1.0,
        spot: spot(9.57, 13.15, 192.0),
        footprint: None,
        set_on: None,
    },
    Prop {
        look: PropLook::Object(457), // Orcbarrel01
        scale: 1.0,
        spot: spot(7.54, 17.85, 232.0),
        footprint: None,
        set_on: None,
    },
    Prop {
        look: PropLook::Object(CAMP_HORDE_LARGE_BASKET_DISPLAY), // LargeBasket01, ours
        scale: 0.6,
        spot: spot(8.95, 20.28, 232.0),
        footprint: None,
        set_on: None,
    },
    Prop {
        look: PropLook::Object(732), // OrcBrazier_LightpostBarrens
        scale: 1.0,
        spot: spot(4.32, 17.43, 52.0),
        footprint: None,
        set_on: None,
    },
    // the appearance shop
    Prop {
        look: PropLook::Object(CAMP_TROLL_SHOP_DISPLAY), // trollshop01, ours
        scale: HORDE_SHOP_SIZE,
        spot: HORDE_SHOP,
        footprint: None,
        set_on: None,
    },
    // the Voodoo Mask's chest and drum, either side of it in the shop
    Prop {
        look: PropLook::Object(2450), // TrollChest
        scale: 1.0,
        spot: spot(-20.3, 41.9, 200.0),
        footprint: None,
        set_on: Some(ON_THE_SHOP_FLOOR),
    },
    Prop {
        look: PropLook::Object(2572), // TaurenDrumMed01
        scale: 1.0,
        spot: spot(-23.2, 41.9, 160.0),
        footprint: None,
        set_on: Some(ON_THE_SHOP_FLOOR),
    },
    // the Armoury: the orc tent over the weapon rack, standing as it is
    Prop {
        look: PropLook::Object(CAMP_ORC_TENT_DISPLAY), // orctent02, ours
        scale: 1.0,
        spot: spot(1.8, -23.6, 333.0),
        footprint: None,
        set_on: None,
    },
    // a water barrel by its front pole and one by its back-left pole, each a yard out from it so it clears the tent (for the front one), keeping the walkers off the poles; and the smith's quenching barrel between the anvil and the forge, keeping them off the canopy's low uphill side
    Prop {
        look: PropLook::Object(457), // Orcbarrel01
        scale: 1.0,
        spot: spot(-0.6, -19.17, 0.0),
        footprint: None,
        set_on: None,
    },
    Prop {
        look: PropLook::Object(457), // Orcbarrel01
        scale: 1.0,
        spot: spot(-1.03, -27.75, 0.0),
        footprint: None,
        set_on: None,
    },
    Prop {
        look: PropLook::Object(457), // Orcbarrel01
        scale: 1.0,
        spot: spot(-4.2, -22.6, 0.0),
        footprint: None,
        set_on: None,
    },
    // the smithy outside its west side, up the slope: the third tradeskill forge in place of the bellows at seven tenths of its size, its middle where they stood, its length (+y, origin at its corner) along the tent's side and its front toward the anvil, lying tilted on the sand
    Prop {
        look: PropLook::Object(305), // Tradeskill_Forge_03
        scale: 0.7,
        spot: spot(-5.14, -25.12, 0.0),
        footprint: Some(Footprint { x: (-2.68, 2.16), y: (-1.68, 5.83) }),
        set_on: None,
    },
    // a stone anvil before it, lying on the ground (the small foundry pit taken away); the tool rack behind the forge, along its back, its face out to the open sand south so its worker has room
    Prop {
        look: PropLook::Object(208), // OrcAnvilStoneBurningSteppes
        scale: 1.0,
        spot: spot(-4.7, -19.85, 63.0),
        footprint: Some(Footprint { x: (-0.73, 1.28), y: (-1.76, 1.38) }),
        set_on: None,
    },
    Prop {
        look: PropLook::Object(1487), // OrcToolRackBurningSteppes
        scale: 0.75,
        spot: spot(-6.82, -27.75, 180.0),
        footprint: Some(Footprint { x: (-0.73, 0.0), y: (-1.67, 2.27) }), // its posts' feet, tilted to the slope
        set_on: None,
    },
    // the Records: Gol'Bolar's platform at three tenths, in the Karazhan bookshelves' frame colour, its stairs toward the fire; a quarter of a yard sunk, as the Alliance's
    Prop {
        look: PropLook::Object(CAMP_UNDEAD_PLATFORM_DISPLAY), // GolBolarPlatform, recoloured
        scale: 0.3,
        spot: HORDE_PLATFORM,
        footprint: None,
        set_on: Some(SetOn { lift: -PLATFORM_SUNK, rise_degrees: 0.0, ground_at: None }),
    },
    // Aldous's library on the deck, the Alliance's turned 20 degrees; the table and what is on it
    // half a yard toward the stairs, away from the large shelf
    Prop {
        look: PropLook::Object(234), // DuskwoodTable01
        scale: HORDE_LIBRARY_TABLE_SIZE,
        spot: spot(-1.84, 31.53, 201.13),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(HORDE_PLATFORM) }),
    },
    // Karazhan's large and small bookshelves, where the Alliance's dwarven
    // ones stand on its deck, at their size: as long along their x, as deep
    Prop {
        look: PropLook::Object(CAMP_KARAZHAN_LARGE_SHELF_DISPLAY),
        scale: 0.72,
        spot: spot(-0.81, 34.04, 111.13),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(HORDE_PLATFORM) }),
    },
    Prop {
        look: PropLook::Object(CAMP_KARAZHAN_SMALL_SHELF_DISPLAY),
        scale: 0.72,
        spot: spot(0.44, 31.36, 201.13),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(HORDE_PLATFORM) }),
    },
    Prop {
        look: PropLook::Object(164), // ScrollB01
        scale: 1.0,
        spot: spot(-1.5, 31.24, 151.13),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK + HORDE_LIBRARY_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(HORDE_PLATFORM) }),
    },
    Prop {
        look: PropLook::Object(2630), // FeatherBigBrown
        scale: 0.6,
        spot: spot(-1.51, 31.3, 166.13),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK + HORDE_LIBRARY_TABLE_TOP + 0.12, rise_degrees: 0.0, ground_at: Some(HORDE_PLATFORM) }),
    },
    Prop {
        look: PropLook::Object(107), // GeneralBookStackShort01
        scale: 1.0,
        spot: spot(-2.34, 31.72, 201.13),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK + HORDE_LIBRARY_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(HORDE_PLATFORM) }),
    },
    Prop {
        look: PropLook::Object(43), // GeneralBookStackTall01
        scale: 1.0,
        spot: spot(-1.68, 31.99, 181.13),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK + HORDE_LIBRARY_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(HORDE_PLATFORM) }),
    },
    Prop {
        look: PropLook::Object(43), // GeneralBookStackTall01
        scale: 1.0,
        spot: spot(-3.12, 33.67, 201.13),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(HORDE_PLATFORM) }),
    },
    Prop {
        look: PropLook::Object(107), // GeneralBookStackShort01
        scale: 1.0,
        spot: spot(-2.37, 33.47, 156.13),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(HORDE_PLATFORM) }),
    },
    Prop {
        look: PropLook::Object(107), // GeneralBookStackShort01
        scale: 1.0,
        spot: spot(-1.18, 30.12, 241.13),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(HORDE_PLATFORM) }),
    },
    Prop {
        look: PropLook::Object(4152), // CandleBlack01
        scale: 1.0,
        spot: spot(-1.07, 31.34, 201.13),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK + HORDE_LIBRARY_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(HORDE_PLATFORM) }),
    },
    Prop {
        look: PropLook::Object(474), // MediumBrazierPurple01 (or 754, LordaeronBrazier01)
        scale: 0.5,
        spot: spot(-4.32, 31.39, 201.13),
        footprint: None,
        set_on: Some(SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(HORDE_PLATFORM) }),
    },
    // Tirisfal at the platform's foot: a Forsaken banner and two weeds
    Prop {
        look: PropLook::Object(6388), // ForsakenBanner01
        scale: 0.6,
        spot: spot(1.61, 29.35, 201.13),
        footprint: None,
        set_on: None,
    },
    Prop {
        look: PropLook::Object(4111), // Gloomweed01
        scale: 1.0,
        spot: spot(2.56, 32.09, 331.13),
        footprint: None,
        set_on: None,
    },
    Prop {
        look: PropLook::Object(4112), // Doomweed01
        scale: 1.0,
        spot: spot(-5.8, 31.25, 331.13),
        footprint: None,
        set_on: None,
    },
];
