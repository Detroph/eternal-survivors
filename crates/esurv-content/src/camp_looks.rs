//! The camp's models that are no game object of the 1.12 data, only a map's own (the experimental
//! walk-around camp, the server's `game::camp`): each shown as a dynamic object of an id of ours,
//! which no spell has, drawn by the client as its model (`esurv-app` `camp`), turned as the
//! object faces.

use crate::option_looks::LookModel;

/// The armor stand beside the Armoury's weapon rack: a stand with a blue mail shirt.
pub const CAMP_ARMOR_STAND_LOOK: u32 = 900_002;
pub const CAMP_ARMOR_STAND: LookModel = LookModel {
    path: "World\\Generic\\Human\\Passive Doodads\\Armor\\ArmorStandMailBlue.mdx",
    scale: 1.0,
    lift: 0.0,
    tint: None,
};

/// The camp's things to use are units, so they wear a name and can be targeted as a person of the
/// camp is: each wears a composed display, its object's model planted in the
/// unseen unit ([`crate::looks::PLANTED_ITEM_BASE`]), numbered from here past the planted items':
/// `9 960 000 + the object's display`.
const CAMP_THING_LOOKS_FROM: u32 = 9_960_000;

/// The composed display a camp thing of object display `display` wears.
pub const fn camp_thing_look(display: u32) -> u32 {
    CAMP_THING_LOOKS_FROM + display
}

/// The camp things' object displays (`GameObjectDisplayInfo.dbc`) and their models.
pub const CAMP_THING_MODELS: [(u32, &str); 11] = [
    (CAMP_WAR_MAP_TABLE_MODEL, "World\\Generic\\Human\\Passive Doodads\\StormWind\\AllianceMapTable.mdx"),
    (130, "World\\Generic\\Human\\Passive Doodads\\WeaponRacks\\GeneralWeaponrack01.mdx"),
    (4891, "World\\Lordaeron\\Scholomance\\PassiveDoodads\\CrystalBall\\ScholomanceCrystalBall01.mdx"),
    (3371, "World\\Generic\\Dwarf\\Passive Doodads\\Tables\\ReadingTable01.mdx"),
    // the Horde camp's (plans/hub-horde-camp.md): Orgrimmar's own map table, the tauren rack with
    // its axes, the troll tiki mask on its pole (the Voodoo Mask), the carved troll tablet (the
    // Lore Tablet)
    (6605, "World\\Kalimdor\\Orgrimmar\\PassiveDoodads\\MapTable\\HordeMapTable.mdx"),
    (CAMP_TAUREN_RACK_MODEL, "World\\Generic\\TAUREN\\PASSIVE DOODADS\\Weapons\\Tauren_WeaponRack01.mdx"),
    (CAMP_TIKI_MASK_MODEL, "World\\Generic\\TROLL\\PASSIVE DOODADS\\Tikimasks\\Troll_tikimask01.mdx"),
    (5, "World\\Generic\\Troll\\Passive Doodads\\TrollTablets\\TrollTablet.mdx"),
    // the camp banner posts: each camp's own banner (Stormwind's griffon, the orc clan banner)
    (624, "World\\Generic\\Human\\Passive Doodads\\Banners\\StormwindGriffonBanner01.mdx"),
    (2552, "World\\Generic\\Orc\\Passive Doodads\\Banners\\ClanBanner.mdx"),
    // the neutral camp's Bronze portal: the Caverns of Time's own
    (4713, "World\\Dungeon\\CavernsOfTime\\PassiveDoodads\\Portal\\COT_portal01.mdx"),
];

/// The Horde's weapon rack and Voodoo Mask: no game objects of the 1.12 data, numbered as the
/// camp's own object displays are.
pub const CAMP_TAUREN_RACK_MODEL: u32 = 90_012;
pub const CAMP_TIKI_MASK_MODEL: u32 = 90_013;

/// The Orb's keeper's look: a gnome woman (display 3106's body) in a Dalaran outfit: the Dalaran Wizard's Robe (item look 12089, the Kirin Tor's tabard on it) and the
/// Magister's Mantle, Boots and Gloves (30211, 29594, 29593); a display of ours, past the camp
/// things' composed looks.
pub const CAMP_ORB_KEEPER_LOOK: u32 = 9_950_001;
pub const CAMP_ORB_KEEPER_BODY: u32 = 3106;
/// Head, shoulder, shirt, chest, belt, legs, feet, wrists, hands, tabard (`ItemDisplayInfo`).
pub const CAMP_ORB_KEEPER_GEAR: [u32; 10] = [0, 30211, 0, 12089, 0, 0, 29594, 0, 29593, 0];

/// The War Table's Marshal, Marshal Corwin Hale: a bald human man with a moustache (display
/// 1423's body: no hair, the second facial hair) in the rank 14 warrior's plate,
/// the Field Marshal's Battlegear (item looks 26658 to 26662, its belt 27226 and bracers 26664),
/// bareheaded so his scalp shows.
pub const CAMP_MARSHAL_LOOK: u32 = 9_950_002;
pub const CAMP_MARSHAL_BODY: u32 = 1423;
pub const CAMP_MARSHAL_GEAR: [u32; 10] = [0, 26662, 0, 26658, 27226, 26659, 26661, 26664, 26660, 0];
/// The camp's runner, Wren Ashcombe: the face and short black hair of one Stormwind woman (display
/// 3448) in another's green leathers (display 1449's) (look 6 with look 1's outfit).
pub const CAMP_WREN_LOOK: u32 = 9_950_003;
pub const CAMP_WREN_BODY: u32 = 3448;
pub const CAMP_WREN_GEAR: [u32; 10] = [0, 0, 13463, 0, 9748, 13464, 9750, 0, 0, 0];

/// The War Table's model: Stormwind's Alliance map table, its map on it; no game
/// object of the 1.12 data, so numbered as the camp's own object displays are.
pub const CAMP_WAR_MAP_TABLE_MODEL: u32 = 90_009;

/// The camp's models that are no game object of the 1.12 data but stand as one (a building, or a
/// thing set on one), each given a game-object display of ours (past the data's own, which end
/// below 7 000) and shown as that object.
pub const CAMP_OBJECT_DISPLAYS: [(u32, &str); 28] = [
    (CAMP_HORDE_TENT_DISPLAY, "World\\Generic\\Orc\\Passive Doodads\\Tents\\DurotarOrcTent02.mdx"),
    (CAMP_HORDE_KODO_RUG_DISPLAY, "World\\Generic\\Orc\\Passive Doodads\\Rugs\\KotoSkinRug01.mdx"),
    (CAMP_HORDE_KODO_HIDE_DISPLAY, "World\\Generic\\TAUREN\\PASSIVE DOODADS\\KodoHide\\TaurenKodoHide.mdx"),
    (CAMP_HORDE_MARKET_BLANKET_DISPLAY, "World\\Generic\\Orc\\Passive Doodads\\Blankets\\MarketBlanket01.mdx"),
    (CAMP_HORDE_LARGE_BASKET_DISPLAY, "World\\Generic\\TAUREN\\PASSIVE DOODADS\\Baskets\\LargeBasket01.mdx"),
    (CAMP_WAR_HELM_DISPLAY, "World\\Generic\\Human\\Passive Doodads\\Armor\\ArmorHelmBlueVisorUp.mdx"),
    (CAMP_TENT_DISPLAY, "World\\Generic\\Buildings\\HumanTentMedium\\HumanTentMedium.mdx"),
    (CAMP_BONFIRE_DISPLAY, "World\\Azeroth\\Karazahn\\PassiveDoodads\\Bonfire\\KarazahnBonFire02.mdx"),
    (CAMP_GOL_BOLAR_PLATFORM_DISPLAY, "World\\wmo\\KhazModan\\Collidable Doodads\\GolBolarPlatform\\GolBolarPlatform.wmo"),
    (CAMP_LIBRARY_LARGE_SHELF_DISPLAY, "World\\Generic\\Dwarf\\Passive Doodads\\Bookshelves\\DwarvenBookshelfLarge.mdx"),
    (CAMP_LIBRARY_TABLE_DISPLAY, "World\\Generic\\Dwarf\\Passive Doodads\\Tables\\DwarvenTableAverage02.mdx"),
    (CAMP_LIBRARY_SMALL_SHELF_DISPLAY, "World\\Generic\\Dwarf\\Passive Doodads\\Bookshelves\\DwarvenBookshelfSmall.mdx"),
    (CAMP_GNOME_TABLE_DISPLAY, "World\\Generic\\Gnome\\Passive Doodads\\Furniture\\GnomeTable01.mdx"),
    (CAMP_UNDEAD_PLATFORM_DISPLAY, "World\\wmo\\KhazModan\\Collidable Doodads\\GolBolarPlatform\\GolBolarPlatform.wmo"),
    (CAMP_KARAZHAN_LARGE_SHELF_DISPLAY, "World\\Azeroth\\Karazahn\\PassiveDoodads\\BookShelves\\KarazahnBookshelfLarge.mdx"),
    (CAMP_KARAZHAN_SMALL_SHELF_DISPLAY, "World\\Azeroth\\Karazahn\\PassiveDoodads\\BookShelves\\KarazahnBookshelfSmall.mdx"),
    (CAMP_ORC_TENT_DISPLAY, "World\\Generic\\ORC\\PASSIVE DOODADS\\Tents\\orctent02.mdx"),
    (CAMP_TROLL_SHOP_DISPLAY, "World\\wmo\\Kalimdor\\Buildings\\TrollShop\\trollshop01.wmo"),
    (CAMP_NEUTRAL_ORC_HUT_DISPLAY, "World\\wmo\\Kalimdor\\Buildings\\OrcHut\\OrcHut.wmo"),
    (CAMP_NEUTRAL_TROLL_BURROW_DISPLAY, "World\\wmo\\Kalimdor\\Buildings\\TrollBurrow\\TrollBurrow.wmo"),
    (CAMP_NEUTRAL_STAIRS_DISPLAY, "World\\wmo\\Azeroth\\Collidable Doodads\\Stranglethorn\\StrangleRuinStair\\StrangleRuinStair.wmo"),
    (CAMP_NEUTRAL_WAR_MAP_TABLE_DISPLAY, "World\\Generic\\Human\\Passive Doodads\\StormWind\\AllianceMapTable.mdx"),
    (CAMP_NEUTRAL_CHROMIE_TABLE_DISPLAY, "World\\KhazModan\\Uldaman\\PassiveDoodads\\Tables\\UldamanTable01.mdx"),
    (CAMP_NEUTRAL_MAGIC_IMPLEMENT_1_DISPLAY, "World\\Generic\\NightElf\\Passive Doodads\\MagicalImplements\\NEMagicImplement01.mdx"),
    (CAMP_NEUTRAL_MAGIC_IMPLEMENT_8_DISPLAY, "World\\Generic\\NightElf\\Passive Doodads\\MagicalImplements\\NEMagicImplement08.mdx"),
    (CAMP_NEUTRAL_HORDE_WAR_MAP_TABLE_DISPLAY, "World\\Kalimdor\\Orgrimmar\\PassiveDoodads\\MapTable\\HordeMapTable.mdx"),
    (CAMP_NEUTRAL_TAUREN_RACK_DISPLAY, "World\\Generic\\TAUREN\\PASSIVE DOODADS\\Weapons\\Tauren_WeaponRack01.mdx"),
    (CAMP_NEUTRAL_TIKI_MASK_DISPLAY, "World\\Generic\\TROLL\\PASSIVE DOODADS\\Tikimasks\\Troll_tikimask01.mdx"),
];
/// The neutral camp's hut where the Caverns building's ruined burrow stood (the
/// ruin, Blizzard's AbandonedTrollBurrow inside the building, read as wrecked): Razor Hill's orc
/// hut or Durotar's troll burrow.
pub const CAMP_NEUTRAL_ORC_HUT_DISPLAY: u32 = 90_023;
pub const CAMP_NEUTRAL_TROLL_BURROW_DISPLAY: u32 = 90_024;
/// The neutral camp's two stairs from the ledge down into the basins: Stranglethorn's ruined stone
/// stairs (placed in Meld).
pub const CAMP_NEUTRAL_STAIRS_DISPLAY: u32 = 90_025;
/// The neutral camp's Alliance War Table, a piece only, as the Bronze portal opens the zones there
///: the Alliance camp's map table as a game object.
pub const CAMP_NEUTRAL_WAR_MAP_TABLE_DISPLAY: u32 = 90_026;
/// The neutral camp's table Chromie sits on, in front of the rock by the portal: Uldaman's stone table
///.
pub const CAMP_NEUTRAL_CHROMIE_TABLE_DISPLAY: u32 = 90_027;
/// The neutral camp's two Darnassus magic implements at the ends of Zazzi's table.
pub const CAMP_NEUTRAL_MAGIC_IMPLEMENT_1_DISPLAY: u32 = 90_028;
pub const CAMP_NEUTRAL_MAGIC_IMPLEMENT_8_DISPLAY: u32 = 90_029;
/// The neutral camp's Horde War Table, a piece only, as the Alliance's; and the Horde's weapon rack and
/// Voodoo Mask as pieces, for the Alliance to look at.
pub const CAMP_NEUTRAL_HORDE_WAR_MAP_TABLE_DISPLAY: u32 = 90_030;
pub const CAMP_NEUTRAL_TAUREN_RACK_DISPLAY: u32 = 90_031;
pub const CAMP_NEUTRAL_TIKI_MASK_DISPLAY: u32 = 90_032;
/// Their hues, recoloured as the undead platform is: the ruin's textures' mean colour, 112.3, 96.8, 84.5 of 255 (weighted by
/// the area each covers), over each hut's own mean brightest channel, 136.0 for the orc hut and
/// 128.0 for the burrow. Measured, not chosen.
pub const CAMP_NEUTRAL_ORC_HUT_HUE: [f32; 3] = [0.826, 0.712, 0.621];
pub const CAMP_NEUTRAL_TROLL_BURROW_HUE: [f32; 3] = [0.877, 0.756, 0.660];
/// Gol'Bolar's wooden platform from Dun Morogh, its stairs down off its right side .
pub const CAMP_GOL_BOLAR_PLATFORM_DISPLAY: u32 = 90_001;
/// The Horde camp's Records stand on the same platform recoloured to the colour of the Karazhan
/// bookshelves' frame beside it.
pub const CAMP_UNDEAD_PLATFORM_DISPLAY: u32 = 90_018;
/// The hue the undead platform's texture is recoloured to, before the light: each texel's brightest
/// channel times it. Its brightest channel, 0.57, is the shelves' frame's texture (54 of 255) over
/// the platform's dock planks' (94); its shade is the frame's texture's, 54, 50, 39, so the hour's
/// light colours both alike (a brown fixed after the light read orange at dusk).
pub const CAMP_UNDEAD_PLATFORM_HUE: [f32; 3] = [0.57, 0.53, 0.41];
/// The Horde Chronicler's bookshelves: Karazhan's large and small.
pub const CAMP_KARAZHAN_LARGE_SHELF_DISPLAY: u32 = 90_019;
pub const CAMP_KARAZHAN_SMALL_SHELF_DISPLAY: u32 = 90_020;
/// The Chronicler's library on the platform: Ironforge's large and small bookshelves and the
/// table he sits on.
pub const CAMP_LIBRARY_LARGE_SHELF_DISPLAY: u32 = 90_002;
pub const CAMP_LIBRARY_TABLE_DISPLAY: u32 = 90_003;
pub const CAMP_LIBRARY_SMALL_SHELF_DISPLAY: u32 = 90_004;
/// The camp's fire: Karazhan's second bonfire.
pub const CAMP_BONFIRE_DISPLAY: u32 = 90_005;
/// The tent at each hero's plot: the Defias' tent of Elwynn Forest, an object so
/// the client makes it solid from its collision hull (heroes cannot walk through
/// the tents).
pub const CAMP_TENT_DISPLAY: u32 = 90_006;
/// The Horde camp's tent at each plot: the Durotar hide tent of Razor Hill's camps
/// (plans/hub-decisions.md 5).
pub const CAMP_HORDE_TENT_DISPLAY: u32 = 90_011;
/// The Horde camp's Armoury tent: the big orc hide tent on its poles (orctent02, no game object of
/// 1.12), over the weapon rack and its trainer (plans/hub-horde-layout-2.md).
pub const CAMP_ORC_TENT_DISPLAY: u32 = 90_021;
/// The Horde camp's appearance shop: the small troll shop of Sen'jin and the Echo Isles (trollshop01,
/// a building of no game object), the Voodoo Mask and its keeper inside.
pub const CAMP_TROLL_SHOP_DISPLAY: u32 = 90_022;
/// The Horde camp's pieces that are no game object (plans/hub-horde-layout.md): the kodo-skin rug
/// under the War Table, the Market's kodo hide on its poles, its blanket of goods and a large
/// tauren basket.
pub const CAMP_HORDE_KODO_RUG_DISPLAY: u32 = 90_014;
pub const CAMP_HORDE_KODO_HIDE_DISPLAY: u32 = 90_015;
pub const CAMP_HORDE_MARKET_BLANKET_DISPLAY: u32 = 90_016;
pub const CAMP_HORDE_LARGE_BASKET_DISPLAY: u32 = 90_017;
/// An officer's helm, visor up, set on the War Table's map.
pub const CAMP_WAR_HELM_DISPLAY: u32 = 90_007;
// 90_008, the logs round the old faction camps' fire, gone with them, is never reused.
/// The gnome table with the purple cloth by the Orb, its keeper standing on it.
pub const CAMP_GNOME_TABLE_DISPLAY: u32 = 90_010;

/// The command tent over the War Table: the dwarven dig pavilion, open on all sides; a look of ours
/// rather than an object so the heroes walk in under it (plans/hub-war-tent.md).
pub const CAMP_WAR_PAVILION_LOOK: u32 = 900_004;
pub const CAMP_WAR_PAVILION: LookModel = LookModel {
    path: "World\\Generic\\Dwarf\\Passive Doodads\\ExcavationTentPavillion\\ExcavationTentPavillion.mdx",
    scale: 1.0,
    lift: 0.0,
    tint: None,
};

/// The Market's tent: Stormwind's vendor tent, its counter on its front.
pub const CAMP_VENDOR_TENT_LOOK: u32 = 900_003;
pub const CAMP_VENDOR_TENT: LookModel = LookModel {
    path: "World\\Generic\\Human\\Passive Doodads\\VendorTents\\StormwindVendorTent01.mdx",
    scale: 1.0,
    // its height is its barrels' (`CampLook::rest_on`)
    lift: 0.0,
    tint: None,
};
/// It leans back a little, its awning's flap clear of the merchant's name, and stays level from
/// side to side.
const VENDOR_TENT_LEAN_BACK: f32 = 4.0;
/// The bottoms of the vendor tent's two counter barrels (model yards: ahead, left, up), which stand
/// on the ground (model_sizes' feet).
const VENDOR_TENT_BARRELS: [[f32; 3]; 2] = [[4.48, -1.0, -0.09], [4.48, 1.0, -0.09]];

/// A camp look: its model, and how it stands on the ground.
#[derive(Clone, Copy, Debug)]
pub struct CampLook {
    pub id: u32,
    pub model: LookModel,
    /// Degrees it leans back (its top toward its back).
    pub lean_back: f32,
    /// Level from side to side, tilted only along its facing.
    pub level_sideways: bool,
    /// Points of it (model yards: ahead, left, up) that stand on the ground: it is set as high as
    /// none of them is above the ground. Empty: its lowest point is on the ground.
    pub rest_on: &'static [[f32; 3]],
}

/// Every camp look.
pub const CAMP_LOOKS: [CampLook; 3] = [
    CampLook { id: CAMP_ARMOR_STAND_LOOK, model: CAMP_ARMOR_STAND, lean_back: 0.0, level_sideways: false, rest_on: &[] },
    CampLook { id: CAMP_WAR_PAVILION_LOOK, model: CAMP_WAR_PAVILION, lean_back: 0.0, level_sideways: false, rest_on: &[] },
    CampLook {
        id: CAMP_VENDOR_TENT_LOOK,
        model: CAMP_VENDOR_TENT,
        lean_back: VENDOR_TENT_LEAN_BACK,
        level_sideways: true,
        rest_on: &VENDOR_TENT_BARRELS,
    },
];
