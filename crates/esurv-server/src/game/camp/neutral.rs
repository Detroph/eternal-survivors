//! The neutral camp, the only camp:
//! the Caverns of Time in Tanaris, the circle round the anchor spot out to the one they gave,
//! 148.8 yards west. The anchor spot is on a ledge running east and west between two
//! basins some ten yards lower: the fire and the War Table on the ledge, shared; each side's tents
//! and services in its own basin, the Alliance's north, the Horde's south (the plan,
//! neutral-plan11). Each side's camp is laid out for the hero going in, its own heroes at their tents.
//! Each side's hubs are its old camp's, carried over whole and stood flat on the basin floor
//! ([`Port`]); the other side sees them but does not use them. The War Tables are only to look at: the Bronze portal opens the zones.

use esurv_content::camp_looks::{
    CAMP_NEUTRAL_CHROMIE_TABLE_DISPLAY, CAMP_NEUTRAL_HORDE_WAR_MAP_TABLE_DISPLAY, CAMP_NEUTRAL_MAGIC_IMPLEMENT_1_DISPLAY, CAMP_NEUTRAL_MAGIC_IMPLEMENT_8_DISPLAY, CAMP_NEUTRAL_STAIRS_DISPLAY, CAMP_NEUTRAL_TAUREN_RACK_DISPLAY, CAMP_NEUTRAL_TIKI_MASK_DISPLAY, CAMP_NEUTRAL_TROLL_BURROW_DISPLAY, CAMP_NEUTRAL_WAR_MAP_TABLE_DISPLAY,
};
use esurv_content::Faction;

use super::horde::{HORDE_HUB_PIECES, HORDE_MAP_TABLE, HORDE_PLATFORM, HORDE_SHOP, HORDE_SHOP_FLOOR};
use super::walk::Doing;
use super::{
    spot, CampLayout, DecoKind, Keeper, Prop, PropLook, Service, SetOn, Spot, ALLIANCE_HUB_PIECES, FIRE_ON_THE_GROUND, GNOME_TABLE, GNOME_TABLE_TOP,
    HORDE_FIRE_ON_THE_GROUND, HORDE_TENT_ON_THE_GROUND, LIBRARY_TABLE_TOP, PLATFORM, PLATFORM_DECK, TENT_ON_THE_GROUND, WAR_TABLE, WAR_TABLE_SIZE,
};

/// The camp's edge from the fire, yards: the anchor spots.
const REACH: f32 = 148.8;

/// The fire, shared, on the ledge east of the anchor spot (the plan's middle
/// group moved east).
const FIRE: Spot = spot(28.2, 8.4, 0.0);

/// The middle of the camp's edge: the anchor spot, the circle's middle.
const MIDDLE: Spot = spot(0.0, 0.0, 0.0);

/// The Bronze portal, shared, on the ledge east of the fire on the anchor spot: the zone menu, in
/// the War Table's stead.
const BRONZE_PORTAL: (Service, Spot) = (Service::BronzePortal, spot(81.0, 20.9, 225.0));

/// Chromie, sitting on a small Uldaman table in front of the rock by the portal, facing the way she
/// faced on the rock, centred between the two standards ('center chromie and her table').
const CHROMIE: Spot = spot(73.29, 28.54, 245.0);
/// Anachronos at the cave mouth, where the world has him, facing out over the camp.
const ANACHRONOS: Spot = spot(130.0, 30.0, 257.0);
/// The table, scaled to a seat's height.
const CHROMIE_TABLE_SIZE: f32 = 0.3;
/// Its top, 3.61 at full size (model_sizes).
const ON_THE_TABLE: SetOn = SetOn { lift: 3.61 * CHROMIE_TABLE_SIZE, rise_degrees: 0.0, ground_at: None };

/// The shared decoration spots round the fire, each facing it, moved with it.
const DECORATIONS: [(DecoKind, Spot); 4] = [
    (DecoKind::Any, spot(36.43, 13.15, 240.0)),
    (DecoKind::Any, spot(36.43, 3.65, 300.0)),
    (DecoKind::Any, spot(19.97, 3.65, 60.0)),
    (DecoKind::Any, spot(19.97, 13.15, 120.0)),
];

/// The troll burrow where the Caverns building's ruined one stood, south of the camp, its door toward the fire; recoloured to the ruin's hue (`camp_looks`). Both sides see it.
/// A quarter larger, tipped 10° front down with the slope, and set 3 yards down, picked
/// from the pictures: about 27% of its height above its floor (16.68 yards) under the ground at
/// its deepest, worked out from the model's points on the ground's plane there (falling 3.5°
/// toward the camp).
///
/// Then the two wooden-post standards, the Alliance's and the Horde's, either side of Chromie on
/// the anchor spots.
///
/// Then the two stairs from the ledge down into the basins, Stranglethorn's ruined stone stairs
/// where they were fitted in Meld: each one's spot, its height over the ground
/// there and its turn read from Meld's map (Meld's turn is the map file's less a quarter turn, as
/// the Caverns' orc hut shows; a map file's turn less a half turn is the world's), its tilt of
/// under a degree left out.

const HUT: [Prop; 6] = [
    Prop {
        look: PropLook::Object(CAMP_NEUTRAL_TROLL_BURROW_DISPLAY),
        scale: 1.25,
        spot: spot(18.85, -74.0, 345.7),
        footprint: None,
        set_on: Some(SetOn { lift: -3.0, rise_degrees: -10.0, ground_at: None }),
    },
    Prop { look: PropLook::Object(5651), scale: 1.0, spot: spot(72.7, 31.2, 245.0), footprint: None, set_on: None },
    // the Horde's standard on the rock's side, its foot at the Alliance's height
    Prop {
        look: PropLook::Object(5652),
        scale: 1.0,
        spot: spot(75.3, 26.7, 248.0),
        footprint: None,
        set_on: Some(SetOn { lift: 0.0, rise_degrees: 0.0, ground_at: Some(spot(72.7, 31.2, 0.0)) }),
    },
    Prop { look: PropLook::Object(CAMP_NEUTRAL_CHROMIE_TABLE_DISPLAY), scale: CHROMIE_TABLE_SIZE, spot: CHROMIE, footprint: None, set_on: None },
    // down into the north basin (Meld: x 373.38, z 157.41, 2.30 down, turned -1.166), then 2 yards
    // north at the same height, so it cuts less into the slope
    Prop {
        look: PropLook::Object(CAMP_NEUTRAL_STAIRS_DISPLAY),
        scale: 1.0,
        spot: spot(51.80, 50.68, 156.81),
        footprint: None,
        set_on: Some(SetOn { lift: -2.30, rise_degrees: 0.0, ground_at: Some(spot(51.80, 48.68, 0.0)) }),
    },
    // down into the south basin (Meld: x 390.59, z 224.28, 2.62 down, turned -4.461)
    Prop {
        look: PropLook::Object(CAMP_NEUTRAL_STAIRS_DISPLAY),
        scale: 1.0,
        spot: spot(69.00, -18.19, 345.60),
        footprint: None,
        set_on: Some(SetOn { lift: -2.62, rise_degrees: 0.0, ground_at: None }),
    },
];

/// A hub of a faction's own camp carried over whole: each
/// piece kept where it stood to the hub's anchor, the whole turned about it. `from` is the anchor
/// in its own camp, `to` its spot here (plans/hub-decisions.md); `sin` and `cos` are of the turn,
/// `to.faces` less `from.faces`, clockwise.
#[derive(Clone, Copy)]
struct Port {
    from: Spot,
    to: Spot,
    sin: f32,
    cos: f32,
}

impl Port {
    /// Spot `s` of the hub's own camp, here.
    const fn spot(&self, s: Spot) -> Spot {
        let (de, dn) = (s.east - self.from.east, s.north - self.from.north);
        let faces = s.faces + self.to.faces - self.from.faces;
        let faces = if faces >= 360.0 {
            faces - 360.0
        } else if faces < 0.0 {
            faces + 360.0
        } else {
            faces
        };
        spot(self.to.east + de * self.cos + dn * self.sin, self.to.north - de * self.sin + dn * self.cos, faces)
    }

    /// Piece `p` of the hub's own camp, here: what it stood on carried with it, and flat, one that
    /// lay tilted to the slope under its footprint standing upright on the ground at its spot.
    const fn prop(&self, p: Prop) -> Prop {
        let set_on = match p.set_on {
            Some(on) => Some(SetOn {
                ground_at: match on.ground_at {
                    Some(g) => Some(self.spot(g)),
                    None => None,
                },
                ..on
            }),
            None => None,
        };
        Prop { spot: self.spot(p.spot), footprint: None, set_on, ..p }
    }
}

/// The Alliance's hubs, each anchored on its spot along the north basin's north-east edge: the Muster by its War Table at the edge's east end, moved 5 yards to the left as
/// seen from in front of its pavilion, ...
const MUSTER: Port = Port { from: WAR_TABLE, to: spot(59.62, 73.81, 254.0), sin: 0.96126, cos: 0.27564 };
/// ... the Armoury by its weapon rack, turned so Thurgen faces the way up to it from the south-west
///, ...
const ARMOURY: Port = Port { from: spot(33.1, 23.6, 165.0), to: spot(52.4, 81.3, 140.0), sin: -0.42262, cos: 0.90631 };
/// ... the Orb, turned so Zazzi faces the way up to her from the south, then square to the wall behind her, which runs 15°
/// off east-west, ...
const ORB: Port = Port { from: spot(21.4, 31.4, 190.0), to: spot(37.3, 86.9, 195.0), sin: 0.08716, cos: 0.99619 };
/// ... the Records by the lectern, turned a half turn so Tobbin faces the camp, not the wall
/// behind him, and then set so Tobbin
/// sits on the anchor spot, 27.4, 90.6, clear of the wall, ...
const RECORDS: Port = Port { from: spot(35.6, 14.6, 69.0), to: spot(27.24, 87.2, 22.0), sin: -0.73135, cos: 0.68200 };
/// ... and the Market by its merchant.
const MARKET: Port = Port { from: spot(-0.2, 27.7, 162.0), to: spot(9.9, 92.7, 185.0), sin: 0.39073, cos: 0.92050 };

/// How many pieces the Alliance's hubs have.
const ALLIANCE_HUB_PROPS: usize = ALLIANCE_HUB_PIECES.len();

/// The hub the Alliance's hub piece `k` belongs to.
const fn alliance_hub(k: usize) -> Port {
    match k {
        0..=3 => ARMOURY,
        4 => ORB,
        5 | 25..=37 => RECORDS,
        6..=24 => MARKET,
        _ => MUSTER,
    }
}

/// The Alliance's hubs' pieces here, and its War Table, only to look at.
const ALLIANCE_HUBS: [Prop; ALLIANCE_HUB_PROPS + 1] = {
    let props = ALLIANCE_HUB_PIECES;
    let mut out = [props[0]; ALLIANCE_HUB_PROPS + 1];
    let mut k = 0;
    while k < ALLIANCE_HUB_PROPS {
        out[k] = alliance_hub(k).prop(props[k]);
        k += 1;
    }
    out[ALLIANCE_HUB_PROPS] =
        Prop { look: PropLook::Object(CAMP_NEUTRAL_WAR_MAP_TABLE_DISPLAY), scale: WAR_TABLE_SIZE, spot: MUSTER.to, footprint: None, set_on: None };
    out
};

/// The Alliance's things to use, as the Horde sees them: only to look at.
const ALLIANCE_THINGS: [Prop; 3] = [
    alliance_thing(Service::WeaponRack, ARMOURY.to),
    alliance_thing(Service::TransmutationOrb, ORB.to),
    alliance_thing(Service::Lectern, RECORDS.to),
];

const fn alliance_thing(service: Service, s: Spot) -> Prop {
    thing_as_piece(Faction::Alliance, service, s)
}

/// `faction`'s thing to use `service` as a piece at `s`, its look a game object's.
const fn thing_as_piece(faction: Faction, service: Service, s: Spot) -> Prop {
    let (display, scale) = service.look(faction);
    Prop { look: PropLook::Object(display), scale, spot: s, footprint: None, set_on: None }
}

/// The Horde's hubs, carried over whole as the Alliance's, each anchored on its spot in the column along the
/// wrecked boat on the south basin's east edge, north to south, facing as drawn: the
/// Muster by its War Table, ...
const HORDE_MUSTER: Port = Port { from: HORDE_MAP_TABLE, to: spot(104.0, -30.0, 270.0), sin: -0.42262, cos: 0.90631 };
/// ... the Armoury by its tauren rack, ...
const HORDE_ARMOURY: Port = Port { from: spot(2.12, -24.89, 333.0), to: spot(106.0, -44.0, 289.0), sin: -0.69466, cos: 0.71934 };
/// ... the Mask shop by its Voodoo Mask, ...
const HORDE_MASK: Port = Port { from: spot(-21.75, 42.4, 180.0), to: spot(100.0, -56.0, 300.0), sin: 0.86603, cos: -0.50000 };
/// ... the Records by the Lore Tablet, ...
const HORDE_RECORDS: Port = Port { from: spot(-1.78, 28.53, 200.13), to: spot(92.0, -68.0, 301.0), sin: 0.98205, cos: -0.18858 };
/// ... and the Market by its trader.
const HORDE_MARKET: Port = Port { from: spot(11.0, 17.32, 232.0), to: spot(84.0, -78.0, 333.0), sin: 0.98163, cos: -0.19081 };

/// The Armoury's orc tent turned a quarter about itself, the barrels by its poles with it.
const HORDE_TENT: Spot = HORDE_ARMOURY.spot(spot(1.8, -23.6, 333.0));
const TENT_TURN: Port = Port { from: HORDE_TENT, to: spot(HORDE_TENT.east, HORDE_TENT.north, HORDE_TENT.faces + 90.0), sin: 1.0, cos: 0.0 };
/// The tauren rack on the anchor spot, 103.2, -46.3, facing 310, the hero looking it over with it.
const HORDE_RACK: Port = Port { from: spot(2.12, -24.89, 333.0), to: spot(103.2, -46.3, 310.0), sin: -0.39073, cos: 0.92050 };
/// The smithy with its forge's middle on the anchor spot, 107.3, -39.8, facing 292, and the anvil,
/// quenching barrel and tool rack where they stood to it: the forge's origin at its corner, its middle 0.26 behind and 2.08
/// left of it (its footprint).
const SMITHY: Port = Port { from: spot(-5.14, -25.12, 0.0), to: spot(107.84, -37.77, 292.0), sin: -0.92718, cos: 0.37461 };

/// How many pieces the Horde's hubs have.
const HORDE_HUB_PROPS: usize = HORDE_HUB_PIECES.len();

/// The hub the Horde's hub piece `k` belongs to.
const fn horde_hub(k: usize) -> Port {
    match k {
        0..=9 => HORDE_MUSTER,
        10..=16 => HORDE_MARKET,
        17..=19 => HORDE_MASK,
        20..=26 => HORDE_ARMOURY,
        _ => HORDE_RECORDS,
    }
}

/// The shop sunk so its stairs reach the sand, the basin falling away before its door.
const HORDE_SHOP_SUNK: f32 = 1.2;

/// The Mask on the shop's floor, carried with the shop.
const ON_THE_MASK_SHOP_FLOOR: SetOn = SetOn { lift: HORDE_SHOP_FLOOR - HORDE_SHOP_SUNK, rise_degrees: 0.0, ground_at: Some(HORDE_MASK.spot(HORDE_SHOP)) };

/// The Horde's hubs' pieces here, and its War Table, only to look at.
const HORDE_HUBS: [Prop; HORDE_HUB_PROPS + 1] = {
    let props = HORDE_HUB_PIECES;
    let mut out = [props[0]; HORDE_HUB_PROPS + 1];
    let mut k = 0;
    while k < HORDE_HUB_PROPS {
        out[k] = horde_hub(k).prop(props[k]);
        k += 1;
    }
    // the Market's brazier a small fire pit
    out[16] = Prop { look: PropLook::Object(408), scale: 1.0, ..out[16] }; // SmallFirePit01
    // the shop and what stands on its floor, sunk
    out[17].set_on = Some(SetOn { lift: -HORDE_SHOP_SUNK, rise_degrees: 0.0, ground_at: None });
    out[18].set_on = Some(ON_THE_MASK_SHOP_FLOOR);
    out[19].set_on = Some(ON_THE_MASK_SHOP_FLOOR);
    // the orc tent and the two barrels by its poles, turned a quarter about it
    let mut k = 20;
    while k <= 22 {
        out[k] = TENT_TURN.prop(out[k]);
        k += 1;
    }
    // the smithy, the forge on the anchor spot: the quenching barrel, forge, anvil and tool rack
    let mut k = 23;
    while k <= 26 {
        out[k] = SMITHY.prop(props[k]);
        k += 1;
    }
    let (_, scale) = Service::WarTable.look(Faction::Horde);
    out[HORDE_HUB_PROPS] =
        Prop { look: PropLook::Object(CAMP_NEUTRAL_HORDE_WAR_MAP_TABLE_DISPLAY), scale, spot: HORDE_MUSTER.to, footprint: None, set_on: None };
    out
};

/// The Horde's things to use, as the Alliance sees them: only to look at.
const HORDE_THINGS: [Prop; 3] = [
    Prop { look: PropLook::Object(CAMP_NEUTRAL_TAUREN_RACK_DISPLAY), scale: Service::WeaponRack.look(Faction::Horde).1, spot: HORDE_RACK.to, footprint: None, set_on: None },
    Prop {
        look: PropLook::Object(CAMP_NEUTRAL_TIKI_MASK_DISPLAY),
        scale: Service::TransmutationOrb.look(Faction::Horde).1,
        spot: HORDE_MASK.to,
        footprint: None,
        set_on: Some(ON_THE_MASK_SHOP_FLOOR),
    },
    thing_as_piece(Faction::Horde, Service::Lectern, HORDE_RECORDS.to),
];

/// Two mage banners (1.12's `MagicBanner01`) on the wall behind Zazzi, either side of her, facing
/// out, at under a third of their
/// size and hung flat on the wall at head height; then
/// set along the wall, 15° off east-west, either side of the point behind Zazzi, and raised clear of
/// the sloped ledge at the wall's foot onto its upright face; then a yard higher and 2 yards further apart. The wall does not turn with the Orb, so they stand on the camp's own spots.
const MAGE_BANNERS: [Prop; 2] = [mage_banner(spot(37.12, 93.81, 195.0)), mage_banner(spot(40.90, 92.80, 195.0))];

const fn mage_banner(s: Spot) -> Prop {
    Prop { look: PropLook::Object(634), scale: 0.3, spot: s, footprint: None, set_on: Some(SetOn { lift: 3.0, rise_degrees: 0.0, ground_at: None }) }
}

/// Darnassus' magic implements 1 and 8 on the ground at either end of Zazzi's table, facing as she
/// does. The table, ported
/// with the Orb, stands at 37.90, 89.16, its 3.4-yard length along the wall (285°); each stands
/// clear of its end by its own half width (`model_sizes`: 1.0 and 1.6 yards across).
const MAGIC_IMPLEMENTS: [Prop; 2] = [
    magic_implement(CAMP_NEUTRAL_MAGIC_IMPLEMENT_1_DISPLAY, spot(35.29, 89.86, 195.0)),
    magic_implement(CAMP_NEUTRAL_MAGIC_IMPLEMENT_8_DISPLAY, spot(40.80, 88.38, 195.0)),
];

const fn magic_implement(display: u32, s: Spot) -> Prop {
    Prop { look: PropLook::Object(display), scale: 1.0, spot: s, footprint: None, set_on: None }
}

/// `parts` one after another.
const fn joined<const N: usize>(parts: &[&[Prop]]) -> [Prop; N] {
    let mut out = [parts[0][0]; N];
    let (mut n, mut i) = (0, 0);
    while i < parts.len() {
        let mut j = 0;
        while j < parts[i].len() {
            out[n] = parts[i][j];
            n += 1;
            j += 1;
        }
        i += 1;
    }
    assert!(n == N, "the parts fill the array");
    out
}

/// What each side sees standing: the shared pieces and both sides' hubs; each the other's things to
/// use too, as pieces.
const BOTH_SEE: [Prop; HUT.len() + ALLIANCE_HUBS.len() + MAGE_BANNERS.len() + MAGIC_IMPLEMENTS.len() + HORDE_HUBS.len()] =
    joined(&[&HUT, &ALLIANCE_HUBS, &MAGE_BANNERS, &MAGIC_IMPLEMENTS, &HORDE_HUBS]);
const ALLIANCE_SEES: [Prop; BOTH_SEE.len() + HORDE_THINGS.len()] = joined(&[&BOTH_SEE, &HORDE_THINGS]);
const HORDE_SEES: [Prop; BOTH_SEE.len() + ALLIANCE_THINGS.len()] = joined(&[&BOTH_SEE, &ALLIANCE_THINGS]);

/// The Alliance's half: the north basin, laid out on the plan (neutral-plan11): the tents in a loose ring round the middle of the basin, two out in the west strip,
/// the hubs along the north-east edge facing back into it. The people of a hub stand (or sit) in
/// it as they did in the Alliance camp, each at their spot there carried over with the hub.
pub(super) const NEUTRAL_ALLIANCE_LAYOUT: CampLayout = CampLayout {
    fire: FIRE,
    fire_piece: &FIRE_ON_THE_GROUND,
    tent: &TENT_ON_THE_GROUND,
    tent_turns: &[0.0; 10],
    services: &[BRONZE_PORTAL, (Service::WeaponRack, ARMOURY.to), (Service::TransmutationOrb, ORB.to), (Service::Lectern, RECORDS.to)],
    // their spots in the Alliance camp
    keepers: &[
        (Keeper::Trainer, ARMOURY.spot(spot(30.1, 24.5, 220.0))),
        (Keeper::Merchant, MARKET.spot(spot(-0.2, 27.7, 162.0))),
        (Keeper::Chronicler, RECORDS.spot(spot(38.2, 16.8, 250.0))),
        (Keeper::OrbKeeper, ORB.spot(spot(21.8, 33.7, 190.0))),
        (Keeper::Marshal, MUSTER.spot(spot(5.1, 10.0, 210.0))),
        (Keeper::Planner, MUSTER.spot(spot(2.3, 10.0, 150.0))),
        // the fire's north-west side, facing the way down to her camp
        (Keeper::Wren, spot(25.0, 17.3, 160.0)),
    ],
    seats: &[(Keeper::Chronicler, SetOn { lift: PLATFORM_DECK + LIBRARY_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(RECORDS.spot(PLATFORM)) })],
    stands_on: &[(Keeper::OrbKeeper, SetOn { lift: GNOME_TABLE_TOP, rise_degrees: 0.0, ground_at: Some(ORB.spot(GNOME_TABLE)) })],
    services_on: &[],
    props: &ALLIANCE_SEES,
    // every door toward the ring's middle (2, 57), each a little off it so they don't line up
    plots: &[
        spot(-8.0, 76.0, 157.0),
        spot(10.0, 75.0, 198.0),
        spot(27.0, 72.0, 243.0),
        spot(29.0, 57.0, 267.0),
        spot(14.0, 46.0, 319.0),
        spot(-6.0, 38.0, 18.0),
        spot(-26.0, 40.0, 62.0),
        spot(-24.0, 60.0, 91.0),
        spot(-48.0, 42.0, 78.0),
        spot(-68.0, 36.0, 69.0),
    ],
    banners: [spot(24.0, 10.0, 20.0), spot(-3.0, 44.0, 180.0), spot(40.0, 56.0, 225.0)],
    // on the north-east shelf at the ledge's height, over the camp
    head: spot(50.0, 28.0, 240.0),
    trophies: [
        spot(5.0, 62.0, 185.0),
        spot(-13.0, 57.5, 167.0),
        spot(23.5, 65.5, 200.0),
        spot(-27.0, 47.5, 150.0),
        spot(42.5, 71.5, 211.0),
        spot(7.0, 72.0, 186.0),
        spot(25.5, 75.0, 199.0),
        spot(9.0, 81.5, 186.0),
        spot(44.0, 82.0, 208.0),
    ],
    decorations: &DECORATIONS,
    plot_decorations: [(5.8, -3.0), (5.8, 3.0), (4.9, 5.1)],
    haunts: &[
        (Doing::Pray, spot(31.21, 9.49, 250.0), 0.0),
        (Doing::Pray, spot(25.19, 9.49, 110.0), 0.0),
        // the hubs' own, as in the Alliance camp
        (Doing::Hammer, ARMOURY.spot(spot(28.45, 25.44, 40.0)), 0.0),
        (Doing::LookOver, ARMOURY.spot(spot(31.9, 22.4, 45.0)), 0.0),
        (Doing::Inspect, ARMOURY.spot(spot(32.56, 20.88, 20.0)), 0.0),
        (Doing::Read, RECORDS.spot(spot(34.4, 14.2, 69.0)), 0.0),
        (Doing::Peer, ORB.spot(spot(21.19, 30.22, 10.0)), 0.0),
        (Doing::Watch, spot(56.0, 62.0, 60.0), 0.0),
        (Doing::Watch, spot(-40.0, 28.0, 300.0), 0.0),
    ],
    orb_subject: None,
    chromie: Some((CHROMIE, ON_THE_TABLE)),
    anachronos: Some(ANACHRONOS),
    reach: REACH,
    middle: MIDDLE,
};

/// The Horde's half: the south basin, the bigger and flatter, laid out on the plan
/// (neutral-plan11): the tents round the basin's middle, three on their
/// own spots by the south slope, two in the free west; the services in a column along the
/// wrecked boat on the east edge, facing back into it. The people of a hub stand on it as they
/// did in the Horde camp, each at their spot there carried over with the hub.
pub(super) const NEUTRAL_HORDE_LAYOUT: CampLayout = CampLayout {
    fire: FIRE,
    fire_piece: &HORDE_FIRE_ON_THE_GROUND,
    tent: &HORDE_TENT_ON_THE_GROUND,
    // each turned a quarter, a half or three quarters, at random; the tenth
    // drawn the same way
    tent_turns: &[180.0, 180.0, 270.0, 90.0, 270.0, 180.0, 270.0, 90.0, 180.0, 90.0],
    services: &[
        BRONZE_PORTAL,
        (Service::WeaponRack, HORDE_RACK.to),
        (Service::TransmutationOrb, HORDE_MASK.to),
        (Service::Lectern, HORDE_RECORDS.to),
    ],
    // their spots in the Horde camp
    keepers: &[
        (Keeper::Trainer, HORDE_ARMOURY.spot(spot(3.08, -21.94, 348.0))),
        (Keeper::Merchant, HORDE_MARKET.spot(spot(11.0, 17.32, 232.0))),
        (Keeper::Chronicler, HORDE_RECORDS.spot(spot(-1.37, 32.75, 201.13))),
        (Keeper::OrbKeeper, HORDE_MASK.spot(spot(-22.7, 40.6, 180.0))),
        (Keeper::Marshal, HORDE_MUSTER.spot(spot(15.5, 2.0, 256.0))),
        // the fire's south side, facing the way down to her camp (as Wren on its north side)
        (Keeper::Wren, spot(31.4, -0.4, 125.0)),
        (Keeper::Planner, HORDE_MUSTER.spot(spot(10.6, -11.3, 309.0))),
    ],
    seats: &[],
    stands_on: &[
        (Keeper::OrbKeeper, ON_THE_MASK_SHOP_FLOOR),
        (Keeper::Chronicler, SetOn { lift: PLATFORM_DECK, rise_degrees: 0.0, ground_at: Some(HORDE_RECORDS.spot(HORDE_PLATFORM)) }),
    ],
    services_on: &[(Service::TransmutationOrb, ON_THE_MASK_SHOP_FLOOR)],
    props: &HORDE_SEES,
    // every door toward the basin's middle (55, -50), each a little off it, but the three
    // (4 to 6) as drawn
    plots: &[
        spot(68.0, -33.0, 222.0),
        spot(80.0, -50.0, 264.0),
        spot(76.0, -66.0, 311.0),
        spot(63.0, -81.0, 343.0),
        spot(45.5, -84.0, 7.0),
        spot(-8.4, -67.5, 31.0),
        spot(-4.0, -42.0, 101.0),
        spot(28.0, -38.0, 108.0),
        spot(48.0, -31.0, 165.0),
        spot(-26.0, -48.0, 87.0),
    ],
    banners: [spot(-4.0, -11.0, 180.0), spot(-20.0, -47.0, 20.0), spot(30.0, -42.0, 330.0)],
    // at the south ramp's foot, facing the fire
    head: spot(12.0, -38.0, 342.0),
    trophies: [
        spot(-1.5, -59.0, 1.0),
        spot(-31.0, -53.0, 30.0),
        spot(-15.0, -62.0, 14.0),
        spot(25.0, -62.0, 338.0),
        spot(43.0, -53.0, 321.0),
        spot(10.5, -60.0, 350.0),
        spot(28.0, -70.0, 338.0),
        spot(56.0, -62.0, 318.0),
        spot(19.5, -54.0, 340.0),
    ],
    decorations: &DECORATIONS,
    plot_decorations: [(5.8, -3.0), (5.8, 3.0), (4.9, 5.1)],
    haunts: &[
        (Doing::Pray, spot(29.72, 5.14, 335.0), 0.0),
        (Doing::Pray, spot(26.68, 5.14, 25.0), 0.0),
        // the hubs' own, as in the Horde camp
        (Doing::Hammer, SMITHY.spot(spot(-5.9, -20.69, 63.0)), 0.0),
        (Doing::LookOver, HORDE_RACK.spot(spot(1.35, -23.38, 153.0)), 0.0),
        (Doing::Inspect, SMITHY.spot(spot(-6.6, -29.15, 0.0)), 0.0),
        (Doing::Read, HORDE_RECORDS.spot(spot(-2.57, 26.55, 20.13)), 0.0),
        (Doing::Peer, HORDE_MASK.spot(spot(-21.4, 40.6, 349.0)), 0.0),
        (Doing::Watch, spot(-50.0, -40.0, 250.0), 0.0),
        (Doing::Watch, spot(80.0, -60.0, 110.0), 0.0),
    ],
    // the boar by the shop, facing Zuli'ka, as in the Horde camp
    orb_subject: Some(HORDE_MASK.spot(spot(-26.7, 40.5, 90.0))),
    chromie: Some((CHROMIE, ON_THE_TABLE)),
    anachronos: Some(ANACHRONOS),
    reach: REACH,
    middle: MIDDLE,
};

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_content::camp_looks::{CAMP_HORDE_KODO_HIDE_DISPLAY, CAMP_ORC_TENT_DISPLAY, CAMP_TROLL_SHOP_DISPLAY, CAMP_UNDEAD_PLATFORM_DISPLAY};

    #[test]
    fn each_hub_turns_as_far_as_its_anchor_does() {
        for port in [MUSTER, ARMOURY, ORB, RECORDS, MARKET, HORDE_MUSTER, HORDE_ARMOURY, HORDE_MASK, HORDE_RECORDS, HORDE_MARKET, TENT_TURN, HORDE_RACK, SMITHY] {
            let turn = (port.to.faces - port.from.faces).to_radians();
            assert!((port.sin - turn.sin()).abs() < 1e-4 && (port.cos - turn.cos()).abs() < 1e-4, "{:?}", port.to);
            let anchor = port.spot(port.from);
            assert!((anchor.east - port.to.east).abs() < 1e-4 && (anchor.north - port.to.north).abs() < 1e-4);
        }
    }

    #[test]
    fn each_side_s_hub_pieces_run_hub_by_hub() {
        assert!(matches!(ALLIANCE_HUB_PIECES[ALLIANCE_HUB_PROPS - 1].look, PropLook::Object(197)), "the Muster's brazier last");
        let props = HORDE_HUB_PIECES;
        assert!(matches!(props[HORDE_HUB_PROPS - 1].look, PropLook::Object(4112)), "the Records' Doomweed last");
        // each Horde hub's first piece where `horde_hub` begins it
        let first = [(10, CAMP_HORDE_KODO_HIDE_DISPLAY), (17, CAMP_TROLL_SHOP_DISPLAY), (20, CAMP_ORC_TENT_DISPLAY), (27, CAMP_UNDEAD_PLATFORM_DISPLAY)];
        for (k, display) in first {
            assert!(matches!(props[k].look, PropLook::Object(d) if d == display), "prop {k}");
        }
    }

    #[test]
    fn the_war_tables_are_only_to_look_at() {
        for layout in [&NEUTRAL_ALLIANCE_LAYOUT, &NEUTRAL_HORDE_LAYOUT] {
            assert!(layout.services.iter().all(|(s, _)| *s != Service::WarTable));
            for table in [CAMP_NEUTRAL_WAR_MAP_TABLE_DISPLAY, CAMP_NEUTRAL_HORDE_WAR_MAP_TABLE_DISPLAY] {
                assert!(layout.props.iter().any(|p| matches!(p.look, PropLook::Object(t) if t == table)));
            }
        }
    }
}
