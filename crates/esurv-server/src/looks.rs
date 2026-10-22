//! What a hero shows on sight: the class gear its class wears, each piece in the look chosen for
//! its slot, over the race and class's starting outfit. The character list carries it
//! (`SMSG_CHAR_ENUM`'s equipment), so the hero picker dresses each hero by the fire in its class's
//! gear; in a run the hero's player carries it in its visible-item fields, as item entries the
//! item query answers ([`item_info`]).
//!
//! A look is a place in the class set's `looks` (every zone its own look,
//! a zone may have several rows; Blackrock two): a piece shows its zone's first row of its own, and
//! by choice any look of a zone whose piece of the slot is in the collection.

use benilla_formats::StartOutfitItem;
use esurv_content::{ArmorType, Class, Content, GearSet, WeaponLook};
use esurv_sim::game::gear::{
    armament, armor_display, armor_inventory_type, decode_item_entry, loadout_armaments, look_zone,
    piece_name, shown_look, weapon_look, zone_look, zone_piece_name, ARMOR_SLOTS,
};
use esurv_sim::game::rewards::{zone_tier, GEAR_ENTRY_BASE};

use crate::ground::GameData;
use crate::packets::ItemInfo;
use crate::save::ClassProgress;

/// The item look a legendary amulet wears in bags and on the ground when its spell's icon has
/// none (spell icons have no item look): an amulet's, Choker of the Fire Lord's.
pub const LEGENDARY_ITEM_ICON: &str = "INV_Jewelry_Amulet_05";

/// The equipment slots a player has (0 head ... 15 main hand, 16 off hand, 17 ranged, 18 tabard).
pub const EQUIPMENT_SLOTS: usize = 19;

/// 1.12's item class of armour (and of shields).
const ITEM_CLASS_ARMOR: u32 = 4;

/// Where the entries of a piece shown in a look not its own start (our numbering, wire-only):
/// `SHOWN_LOOKS_FROM + (piece entry - GEAR_ENTRY_BASE) / 10 × LOOKS_PER_PIECE + look`, far past
/// the pieces' own entries and below the jewelry's.
const SHOWN_LOOKS_FROM: u32 = 10_000_000;
/// How many looks a piece's shown entries have room for (the content allows as many per set).
const LOOKS_PER_PIECE: u32 = 100;
/// The end of the shown looks' entries: room for every piece entry (`decode_item_entry`).
const SHOWN_LOOKS_END: u32 = SHOWN_LOOKS_FROM + 120_000 * LOOKS_PER_PIECE;

/// One worn piece as the client draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WornLook {
    /// The WoW equipment slot it is drawn in (0 head ... 15 main hand, 16 off hand, 17 ranged).
    pub equip_slot: u8,
    /// `ItemDisplayInfo.dbc` id.
    pub display_id: u32,
    /// WoW inventory type (1 head, 5 chest, 20 robe, 17 two-hand ...).
    pub inventory_type: u32,
    /// The item entry a run shows it as ([`shown_item_entry`]).
    pub entry: u32,
}

/// The item entry a run shows class gear piece `entry` (whose own look is `own`) as, worn in look
/// `look`: the piece's own entry in its own look, else an entry of the look's own. The
/// client keeps an entry's answer for the session, so each look is an entry of its own and every
/// answer stays the same.
pub fn shown_item_entry(entry: u32, own: u32, look: u32) -> u32 {
    match entry.checked_sub(GEAR_ENTRY_BASE) {
        Some(e) if look != own && look < LOOKS_PER_PIECE => SHOWN_LOOKS_FROM + e / 10 * LOOKS_PER_PIECE + look,
        _ => entry,
    }
}

/// The class of wow class id `class_id` and its gear set.
fn class_set(content: &Content, class_id: u32) -> Option<(&Class, &GearSet)> {
    let class = content.classes.iter().find(|c| c.wow_class_id == class_id)?;
    Some((class, content.gear_set(&class.id)?))
}

/// A shown entry split into the piece's (wow class id, slot, zone) and the look it shows; `None`
/// for an entry that is no class gear.
fn split_shown_entry(content: &Content, entry: u32) -> Option<((u32, u32, u32), u32)> {
    if (SHOWN_LOOKS_FROM..SHOWN_LOOKS_END).contains(&entry) {
        let n = entry - SHOWN_LOOKS_FROM;
        let piece = decode_item_entry(GEAR_ENTRY_BASE + n / LOOKS_PER_PIECE * 10)?;
        let (_, set) = class_set(content, piece.0)?;
        let look = n % LOOKS_PER_PIECE;
        return ((look as usize) < set.looks.len()).then_some((piece, look));
    }
    let piece = decode_item_entry(entry)?;
    let (_, set) = class_set(content, piece.0)?;
    Some((piece, zone_look(content, set, piece.2)))
}

/// What piece `slot` of `set` shows in look `look`: its equipment slot, display and inventory
/// type (a weapon its zone's armaments' look, [`armament`]). `None` for a slot the set does not
/// have or a piece with no look.
fn piece_look(content: &Content, set: &GearSet, slot: u32, look: u32) -> Option<(u8, u32, u32)> {
    let gear = &content.gear;
    let (equip_slot, display, inventory_type) = if slot < ARMOR_SLOTS {
        let s = gear.slots.get(slot as usize)?;
        (s.equip_slot(), armor_display(set, slot, look), armor_inventory_type(gear, set, slot))
    } else {
        let w = set.weapons.get((slot - ARMOR_SLOTS) as usize)?;
        let (shown, a) = armament(content, set, slot, look)?;
        (w.equip_slot(), a.display, shown.kind_of(a).inventory_type())
    };
    (display != 0).then_some((equip_slot, display, inventory_type))
}

/// The pieces `class` (an id from `classes.ron`) wears in `progress`, each in its look: the one
/// chosen for its slot (`appearance`), else the piece's own. Pieces of another class and entries
/// that are no class gear are skipped.
pub fn worn_looks(content: &Content, class: &str, progress: &ClassProgress) -> Vec<WornLook> {
    let (Some(k), Some(set)) = (content.class(class), content.gear_set(class)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for &entry in &progress.worn {
        let Some((owner, slot, zone)) = decode_item_entry(entry) else {
            continue;
        };
        if owner != k.wow_class_id {
            continue;
        }
        let look = shown_look(content, set, progress.appearance.get(&slot).copied(), zone);
        if let Some((equip_slot, display_id, inventory_type)) = piece_look(content, set, slot, look) {
            out.push(WornLook {
                equip_slot,
                display_id,
                inventory_type,
                entry: shown_item_entry(entry, zone_look(content, set, zone), look),
            });
        }
    }
    out
}

/// Whether equipment slot `slot` holds a weapon (15 main hand, 16 off hand, 17 ranged).
fn is_weapon_slot(slot: usize) -> bool {
    (15..=17).contains(&slot)
}

/// `worn` armed with the weapon loadout `class` fights with (`progress`'s choice, else its first;
/// roadmap R3): the class gear weapons give way to the loadout's
/// weapons, each in its hand's slot, in the armaments of the class gear weapon's look (a
/// zone's weapon drop unlocks its armaments for all three loadouts;
/// `gear::weapon_look`). A class without loadouts keeps `worn` as it is.
pub fn armed(
    content: &Content,
    class: &str,
    progress: &ClassProgress,
    mut worn: Vec<WornLook>,
) -> Vec<WornLook> {
    let (Some(k), Some(set)) = (content.class(class), content.gear_set(class)) else {
        return worn;
    };
    if content.loadout(class, None).is_none() {
        return worn;
    }
    let look = weapon_look(content, set, k.wow_class_id, &progress.worn, |s| progress.appearance.get(&s).copied());
    worn.retain(|w| !is_weapon_slot(w.equip_slot.into()));
    for (hand, kind, display) in loadout_armaments(content, class, progress.loadout.as_deref(), look) {
        worn.push(WornLook {
            equip_slot: hand.equip_slot(),
            display_id: display,
            inventory_type: kind.inventory_type(),
            entry: crate::game::weapon_entry(WeaponLook { kind, display }),
        });
    }
    worn
}

// The starting outfit without its weapons, for a class whose weapon loadout arms it ([`armed`]):
/// a two-hander leaves the off hand empty, not the outfit's shield.
pub fn unarmed_outfit(outfit: &[StartOutfitItem]) -> Vec<StartOutfitItem> {
    outfit
        .iter()
        .filter(|i| !outfit_slot(i.inv_type).is_some_and(is_weapon_slot))
        .cloned()
        .collect()
}

/// The 19 equipment slots of the character list, `(display id, inventory type)` each, from what
/// is worn; an empty slot is `(0, 0)`.
pub fn char_enum_equipment(worn: &[WornLook]) -> [(u32, u8); EQUIPMENT_SLOTS] {
    let mut slots = [(0, 0); EQUIPMENT_SLOTS];
    for w in worn {
        if let Some(s) = slots.get_mut(w.equip_slot as usize) {
            *s = (w.display_id, w.inventory_type.min(255) as u8);
        }
    }
    slots
}

/// An inventory type's equipment slot, where the character list places a starting-outfit piece;
/// `None` for one that is never drawn (rings, trinkets, bags, ammo ...).
fn outfit_slot(inventory_type: u8) -> Option<usize> {
    Some(match inventory_type {
        1 => 0,             // head
        2 => 1,             // neck
        3 => 2,             // shoulders
        4 => 3,             // shirt
        5 | 20 => 4,        // chest, robe
        6 => 5,             // waist
        7 => 6,             // legs
        8 => 7,             // feet
        9 => 8,             // wrists
        10 => 9,            // hands
        16 => 14,           // back
        13 | 17 | 21 => 15, // one-hand, two-hand, main hand
        14 | 22 | 23 => 16, // shield, off hand, held in the off hand
        15 | 25 | 26 => 17, // bow, thrown, gun / crossbow / wand
        19 => 18,           // tabard
        _ => return None,
    })
}

/// The item entry a run shows in each equipment slot (0 for none): the class gear worn, each in
/// its look, and in every slot it leaves empty the starting-outfit piece the character list
/// shows there (`outfit`; a later piece for the same slot over an earlier one).
pub fn shown_equipment(worn: &[WornLook], outfit: &[StartOutfitItem]) -> [u32; EQUIPMENT_SLOTS] {
    let mut slots = [0; EQUIPMENT_SLOTS];
    for item in outfit.iter().filter(|i| i.item_id != 0) {
        if let Some(slot) = outfit_slot(item.inv_type) {
            slots[slot] = item.item_id;
        }
    }
    for w in worn {
        if let Some(s) = slots.get_mut(w.equip_slot as usize) {
            *s = w.entry;
        }
    }
    slots
}

/// 1.12's armour subclass of an armour type.
fn armor_subclass(armor: ArmorType) -> u32 {
    match armor {
        ArmorType::Cloth => 1,
        ArmorType::Leather => 2,
        ArmorType::Mail => 3,
        ArmorType::Plate => 4,
    }
}

/// The item quality of a tier or rarity, 0 common to 4 legendary: 1.12's qualities from common
/// (1) to legendary (5).
fn quality_of(rank: u32) -> u32 {
    rank.min(4) + 1
}

//// What the item query answers for class gear entry `entry` (a piece's own, or one of its looks,
/// [`shown_item_entry`]): the piece's name (the look's own where the look is of the piece's zone,
/// as Blackrock's second row) and quality (its zone's tier), the look's display, the item class,
/// subclass, inventory type and sheath the client draws and holds it by (a weapon its
/// armaments'), and the zone it came from as its description.
fn gear_item_info(content: &Content, entry: u32) -> Option<ItemInfo> {
    let ((class_id, slot, zone), look) = split_shown_entry(content, entry)?;
    let from = content.zones.get(zone as usize)?;
    let tier = zone_tier(content, zone as usize);
    let (_, set) = class_set(content, class_id)?;
    let (_, display_id, inventory_type) = piece_look(content, set, slot, look)?;
    let name = if look_zone(content, set, look) == Some(zone) {
        piece_name(content, set, slot, look)
    } else {
        zone_piece_name(content, set, slot, zone)
    }?;
    let (item_class, subclass, sheath) = if slot < ARMOR_SLOTS {
        (ITEM_CLASS_ARMOR, armor_subclass(set.armor), 0)
    } else {
        // the look drawn decides the grip
        let (w, a) = armament(content, set, slot, look)?;
        let kind = w.kind_of(a);
        (kind.item_class(), kind.subclass(), kind.sheath())
    };
    Some(ItemInfo {
        class: item_class,
        subclass,
        name,
        display_id,
        quality: quality_of(tier),
        inventory_type,
        sheath,
        description: piece_origin(&from.name),
    })
}

// The line that says which zone a class gear piece came from: "From `duskwood`".
pub fn piece_origin(zone_name: &str) -> String {
    format!("From {zone_name}")
}

/// What the item query answers for a starting-outfit piece: its display and inventory type from
/// the player's own data (the 1.12 client has no item table to name it by), and for a weapon or
/// shield its item class, subclass and sheath from the content (`outfit_weapons.ron`).
fn outfit_item_info(content: &Content, data: &GameData, entry: u32) -> Option<ItemInfo> {
    let item = data.outfit_item(entry)?;
    let weapon = content.outfit_weapon(entry);
    Some(ItemInfo {
        class: weapon.map_or(0, |w| w.item_class()),
        subclass: weapon.map_or(0, |w| w.subclass()),
        name: String::new(),
        display_id: item.display_id,
        quality: 0,
        inventory_type: item.inv_type.into(),
        sheath: weapon.map_or(0, |w| w.sheath()),
        description: String::new(),
    })
}

/// What the item query answers for `entry`: class gear by its content ([`gear_item_info`]), a
/// starting-outfit piece ([`outfit_item_info`]), a piece of the run's jewelry
/// ([`jewelry::item_info`]). `None` for any other entry.
pub fn item_info(content: &Content, data: Option<&GameData>, entry: u32) -> Option<ItemInfo> {
    gear_item_info(content, entry)
        .or_else(|| outfit_item_info(content, data?, entry))
        .or_else(|| jewelry::item_info(content, data, entry))
        .or_else(|| crate::game::look_item_info(entry))
}

/// The run's rings, trinkets and amulet as items of the hero's own: their item entries and what
/// the item query answers for them.
///
/// Entries, far past the class gear's (`100 000 ..`) and every stock item's, each fixed
/// for the session (the client keeps an entry's answer) and so holding all its answer tells:
///
/// * a ring or trinket: `1 000 000 000 + index × 10 000 000 + rarity × 1 000 000 + value`, its
///   index in `jewelry.ron`, the rarity it was put on at (0 common .. 4 legendary) and its value
///   now in ten-thousandths (at most 999 999), so an upgrade is another entry;
/// * an amulet of `amulets.ron`: `1 300 000 000 + index × 1000 + rank` (rank at most 999);
/// * a remade legendary amulet (`legendary:<spell>`): `1 400 000 000 + the spell's index in
///   `abilities.ron` × 1000 + rank`.
///
/// They are wire-only: saves and the codex keep ids, so reordering a file only renumbers the
/// next session's entries.
pub mod jewelry {
    use esurv_content::text::percent;
    use esurv_content::{Content, Growth, JewelrySlot};
    use esurv_sim::game::amulets::empower_factor;

    use super::{quality_of, ItemInfo, ITEM_CLASS_ARMOR};
    use crate::ground::GameData;

    const JEWELRY_FROM: u32 = 1_000_000_000;
    const JEWELRY_SPAN: u32 = 10_000_000;
    const RARITY_SPAN: u32 = 1_000_000;
    const VALUE_STEPS: f32 = 10_000.0;
    const AMULETS_FROM: u32 = 1_300_000_000;
    const LEGENDARY_FROM: u32 = 1_400_000_000;
    const RANK_SPAN: u32 = 1000;
    /// The amulet ranges' end (each holds a thousand pieces).
    const AMULETS_END: u32 = 1_500_000_000;
    /// 1.12's inventory types of a neck piece, a ring and a trinket.
    const INVENTORY_NECK: u32 = 2;
    const INVENTORY_FINGER: u32 = 11;
    const INVENTORY_TRINKET: u32 = 12;
    /// The armour subclass of jewelry ("miscellaneous").
    const SUBCLASS_MISC: u32 = 0;

    /// The entry of ring or trinket `id` put on at `rarity` (0 common .. 4 legendary) and worth
    /// `value` now; `None` for an id `jewelry.ron` lacks.
    pub fn ring_or_trinket_entry(
        content: &Content,
        id: &str,
        rarity: u8,
        value: f32,
    ) -> Option<u32> {
        let index = content.jewelry.iter().position(|j| j.id == id)? as u32;
        let value = ((value.max(0.0) * VALUE_STEPS).round() as u32).min(RARITY_SPAN - 1);
        Some(JEWELRY_FROM + index * JEWELRY_SPAN + u32::from(rarity.min(4)) * RARITY_SPAN + value)
    }

    /// The entry of amulet `id` (of `amulets.ron`, or `legendary:<spell>`) at `rank`; `None` for
    /// an id the content lacks.
    pub fn amulet_entry(content: &Content, id: &str, rank: u32) -> Option<u32> {
        let rank = rank.clamp(1, RANK_SPAN - 1);
        if let Some(spell) = id.strip_prefix("legendary:") {
            let index = content.abilities.iter().position(|a| a.id == spell)? as u32;
            return Some(LEGENDARY_FROM + index * RANK_SPAN + rank);
        }
        let index = content.amulets.iter().position(|a| a.id == id)? as u32;
        Some(AMULETS_FROM + index * RANK_SPAN + rank)
    }

    /// An amulet's tooltip line from rank 2 on: how strong its effects are now.
    fn rank_line(rank: u32, growth: Growth) -> String {
        format!(
            "Rank {rank}: its effects at {}.",
            percent(empower_factor(growth, rank))
        )
    }

    /// What the item query answers for a jewelry entry: the content's name, the rarity as quality,
    /// the look that shows the content's icon (from the player's data; none without it), the
    /// slot, and in the tooltip what it does now, in our words.
    pub(super) fn item_info(
        content: &Content,
        data: Option<&GameData>,
        entry: u32,
    ) -> Option<ItemInfo> {
        let look = |icon: &str| data.and_then(|d| d.icon_display(icon)).unwrap_or(0);
        let item = |name: String, icon: &str, rarity: u32, inventory_type, description| ItemInfo {
            class: ITEM_CLASS_ARMOR,
            subclass: SUBCLASS_MISC,
            name,
            display_id: look(icon),
            quality: quality_of(rarity),
            inventory_type,
            sheath: 0,
            description,
        };
        match entry {
            JEWELRY_FROM..AMULETS_FROM => {
                let e = entry - JEWELRY_FROM;
                let j = content.jewelry.get((e / JEWELRY_SPAN) as usize)?;
                let rarity = (e % JEWELRY_SPAN) / RARITY_SPAN;
                let value = (e % RARITY_SPAN) as f32 / VALUE_STEPS;
                let slot = match j.slot {
                    JewelrySlot::Ring => INVENTORY_FINGER,
                    JewelrySlot::Trinket => INVENTORY_TRINKET,
                };
                (rarity <= 4).then(|| {
                    item(
                        j.name.clone(),
                        &j.icon,
                        rarity,
                        slot,
                        j.text_at(value),
                    )
                })
            }
            AMULETS_FROM..LEGENDARY_FROM => {
                let e = entry - AMULETS_FROM;
                let a = content.amulets.get((e / RANK_SPAN) as usize)?;
                let rank = e % RANK_SPAN;
                let mut text = a.text.clone();
                if rank > 1 {
                    text = format!("{text} {}", rank_line(rank, a.growth));
                }
                (rank >= 1).then(|| {
                    item(
                        a.name.clone(),
                        &a.icon,
                        u32::from(a.rarity.index()),
                        INVENTORY_NECK,
                        text,
                    )
                })
            }
            LEGENDARY_FROM..AMULETS_END => {
                let e = entry - LEGENDARY_FROM;
                let spell = content.abilities.get((e / RANK_SPAN) as usize)?;
                let rank = e % RANK_SPAN;
                let recipe = content.legendary_amulets.iter().find(|l| l.main_spell() == spell.id)?;
                let mut text = recipe.text.clone();
                if rank > 1 {
                    text = format!("{text} {}", rank_line(rank, recipe.growth));
                }
                // the spell's icon has no item look: an amulet's stands in (bags, the ground)
                let icon = if data.and_then(|d| d.icon_display(&recipe.icon)).is_some() {
                    recipe.icon.as_str()
                } else {
                    super::LEGENDARY_ITEM_ICON
                };
                (rank >= 1).then(|| {
                    item(
                        recipe.name.clone(),
                        icon,
                        u32::from(recipe.rarity.index()),
                        INVENTORY_NECK,
                        text,
                    )
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_sim::game::rewards::gear_item_entry;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    #[test]
    fn a_worn_piece_shows_its_own_look_or_the_look_chosen_for_its_slot() {
        let c = content();
        let warrior = c.class("warrior").unwrap();
        let set = c.gear_set("warrior").unwrap();
        let mut progress = ClassProgress {
            worn: vec![
                gear_item_entry(warrior.wow_class_id, 0, 1),
                gear_item_entry(warrior.wow_class_id, 2, 4), // the Sunken Temple's chest
                // another class's piece and an entry that is no gear show nothing
                gear_item_entry(warrior.wow_class_id + 1, 2, 1),
                42,
            ],
            ..Default::default()
        };
        let looks = worn_looks(&c, "warrior", &progress);
        assert_eq!(looks.len(), 2, "{looks:?}");
        assert_eq!(looks[0].equip_slot, c.gear.slots[0].equip_slot());
        assert_eq!(looks[0].display_id, armor_display(set, 0, zone_look(&c, set, 1)));
        assert_eq!(looks[0].entry, gear_item_entry(warrior.wow_class_id, 0, 1));
        let temple = set.look_of("sunken_temple", 0).unwrap();
        assert_eq!(looks[1].display_id, set.looks[temple].displays[2]);

        // Blackrock's second row over the Duskwood helm
        let wrath = set.look_of("brm", 1).unwrap() as u32;
        progress.appearance.insert(0, wrath);
        let looks = worn_looks(&c, "warrior", &progress);
        assert_eq!(looks[0].display_id, armor_display(set, 0, wrath));
        assert_ne!(
            looks[0].entry,
            gear_item_entry(warrior.wow_class_id, 0, 1),
            "another look is another entry"
        );

        let slots = char_enum_equipment(&looks);
        let head = c.gear.slots[0].equip_slot() as usize;
        assert_eq!(slots[head].0, armor_display(set, 0, wrath));
        assert_eq!(
            slots.iter().filter(|s| s.0 != 0).count(),
            2,
            "nothing else is worn"
        );
    }

    #[test]
    fn the_item_query_answers_a_worn_piece_as_the_character_list_draws_it() {
        let c = content();
        for class in &c.classes {
            let set = c.gear_set(&class.id).unwrap();
            let pieces = ARMOR_SLOTS + set.weapons.len() as u32;
            for slot in 0..pieces {
                for zone in 0..c.zones.len() as u32 {
                    let tier = zone_tier(&c, zone as usize);
                    for look in 0..set.looks.len() as u32 {
                        let piece = gear_item_entry(class.wow_class_id, slot, zone);
                        let progress = ClassProgress {
                            worn: vec![piece],
                            appearance: [(slot, look)].into(),
                            ..Default::default()
                        };
                        let Some(&worn) = worn_looks(&c, &class.id, &progress).first() else {
                            continue;
                        };
                        let info = item_info(&c, None, worn.entry).unwrap_or_else(|| {
                            panic!("{} slot {slot} zone {zone} look {look}", class.id)
                        });
                        assert_eq!(info.display_id, worn.display_id);
                        assert_eq!(info.inventory_type, worn.inventory_type);
                        assert_eq!(info.quality, tier + 1, "common to legendary");
                        // the worn piece's own name, whatever its look; a row of its own zone's
                        // names it as that row does
                        let name = if look_zone(&c, set, look) == Some(zone) {
                            piece_name(&c, set, slot, look)
                        } else {
                            zone_piece_name(&c, set, slot, zone)
                        };
                        assert_eq!(Some(info.name), name);
                        assert_eq!(info.description, format!("From {}", c.zones[zone as usize].name));
                    }
                }
            }
        }
    }

    #[test]
    fn the_item_query_tells_armour_from_weapons() {
        let c = content();
        let warrior = c.class("warrior").unwrap();
        let set = c.gear_set("warrior").unwrap();
        let chest = item_info(&c, None, gear_item_entry(warrior.wow_class_id, 2, 0)).unwrap();
        assert_eq!((chest.class, chest.subclass), (4, 4), "plate armour");
        let sword = &c.loadout("warrior", None).unwrap().weapons[0];
        let blade = item_info(
            &c,
            None,
            gear_item_entry(warrior.wow_class_id, ARMOR_SLOTS, 0),
        );
        let blade = blade.unwrap();
        assert_eq!((blade.class, blade.subclass), (sword.kind.item_class(), sword.kind.subclass()));
        assert_eq!(blade.sheath, sword.kind.sheath());
        // the Scarlet Monastery's two-hander is an axe: its grip is an axe's
        let scarlet = c.zones.iter().position(|z| z.id == "scarlet").unwrap() as u32;
        let axe = item_info(&c, None, gear_item_entry(warrior.wow_class_id, ARMOR_SLOTS, scarlet)).unwrap();
        assert_eq!(axe.subclass, esurv_content::WeaponKind::TwoHandedAxe.subclass());
        assert_eq!(axe.name, format!("{} Armaments", c.zones[scarlet as usize].name));
        // no gear, a look past the set's: unknown
        assert!(item_info(&c, None, 42).is_none());
        assert!(item_info(&c, None, gear_item_entry(warrior.wow_class_id, 0, 0) + 9).is_none());
        let past = shown_item_entry(gear_item_entry(warrior.wow_class_id, 0, 0), 0, set.looks.len() as u32);
        assert!(item_info(&c, None, past).is_none());
    }
    /// What the client keys a weapon spell's swing on: the hero's held items, as the item query
    /// tells them. A two-hander in the main hand turns the kits' Special1H into Special2H; a
    /// ranged weapon's display picks the shot (bow: LoadBow/AttackBow, gun and crossbow:
    /// LoadRifle/AttackRifle); a hunter holds no melee weapon, so its melee strikes go unarmed.
    #[test]
    fn each_loadout_tells_the_client_the_weapons_its_spells_swing() {
        let c = content();
        let held = |class: &str, loadout: &str| -> Vec<(u8, u32, u32)> {
            let progress = ClassProgress { loadout: Some(loadout.into()), ..Default::default() };
            let mut out: Vec<(u8, u32, u32)> = armed(&c, class, &progress, Vec::new())
                .iter()
                .map(|w| {
                    let info = crate::game::look_item_info(w.entry).expect("a held weapon answers its query");
                    assert_eq!(info.display_id, w.display_id);
                    (w.equip_slot, info.class, info.subclass)
                })
                .collect();
            out.sort();
            out
        };
        // equipment slots 15 main hand, 16 off hand, 17 ranged; class 2 a weapon
        assert_eq!(held("warrior", "two_hander"), [(15, 2, 8)], "a two-handed sword alone");
        assert_eq!(held("warrior", "dual_wield"), [(15, 2, 7), (16, 2, 7)], "two one-handed swords");
        let shield = held("warrior", "shield");
        assert_eq!((shield[0], shield[1].0), ((15, 2, 7), 16), "a one-hander beside the shield");
        assert_eq!(held("hunter", "bow"), [(17, 2, 2)]);
        assert_eq!(held("hunter", "crossbow"), [(17, 2, 18)]);
        assert_eq!(held("hunter", "gun"), [(17, 2, 3)]);
    }

    #[test]
    fn the_starting_outfit_fills_the_slots_the_class_gear_leaves() {
        let item = |item_id, inv_type| StartOutfitItem {
            item_id,
            display_id: item_id * 10,
            inv_type,
        };
        let worn = [WornLook {
            equip_slot: 7,
            display_id: 5,
            inventory_type: 8,
            entry: 100_170,
        }];
        // a shirt, boots under the worn feet, a sword, a ring that is never drawn
        let outfit = [item(38, 4), item(40, 8), item(25, 21), item(99, 11)];
        let slots = shown_equipment(&worn, &outfit);
        assert_eq!(slots[3], 38);
        assert_eq!(slots[7], 100_170, "the class gear over the outfit");
        assert_eq!(slots[15], 25);
        assert_eq!(slots.iter().filter(|&&e| e != 0).count(), 3);
    }

    /// Roadmap R3: a hero shows the weapons of its class's weapon
    /// loadout (the one chosen, else the first) in their hands' slots, in the armaments of the
    /// class gear weapon worn (its zone's, or the look chosen for its slot; Elwynn's with none);
    /// the class gear's weapon gives way; the item query answers each.
    #[test]
    fn a_hero_shows_its_weapon_loadouts_weapons() {
        let c = content();
        for class in &c.classes {
            let set = c.gear_set(&class.id).unwrap();
            for l in c.loadouts_of(&class.id) {
                let progress = ClassProgress { loadout: Some(l.id.clone()), ..Default::default() };
                let armed = armed(&c, &class.id, &progress, Vec::new());
                assert_eq!(armed.len(), l.weapons.len(), "{}/{}", class.id, l.id);
                for (shown, w) in armed.iter().zip(&l.weapons) {
                    let a = w.look_in(&set.looks[0]).unwrap();
                    assert_eq!(set.looks[0].zone, "elwynn");
                    assert_eq!(shown.equip_slot, w.hand.equip_slot());
                    assert_eq!(shown.display_id, a.display);
                    let info = item_info(&c, None, shown.entry).unwrap();
                    let kind = w.kind_of(a);
                    assert_eq!((info.class, info.subclass), (kind.item_class(), kind.subclass()));
                    assert_eq!((info.display_id, info.inventory_type), (shown.display_id, kind.inventory_type()));
                }
            }
        }
        // the warrior's first: the two-hander, in the armaments of the blade worn (Sunken Temple)
        let warrior = c.class("warrior").unwrap();
        let set = c.gear_set("warrior").unwrap();
        let progress = ClassProgress {
            worn: vec![gear_item_entry(warrior.wow_class_id, 0, 1), gear_item_entry(warrior.wow_class_id, ARMOR_SLOTS, 4)],
            ..Default::default()
        };
        let worn = worn_looks(&c, "warrior", &progress);
        let shown = armed(&c, "warrior", &progress, worn.clone());
        assert_eq!(shown.len(), 2);
        assert_eq!(shown[0], worn[0], "the armour stays");
        let two_hander = &c.loadout("warrior", None).unwrap().weapons[0];
        let look = |zone, row| two_hander.look_in(&set.looks[set.look_of(zone, row).unwrap()]).unwrap().display;
        assert_eq!((shown[1].equip_slot, shown[1].display_id), (15, look("sunken_temple", 0)));
        // a chosen look for the blade's slot picks the armaments: Blackwing Lair's
        let wrath = set.look_of("brm", 1).unwrap() as u32;
        let looked = ClassProgress { appearance: [(ARMOR_SLOTS, wrath)].into(), ..progress };
        assert_eq!(armed(&c, "warrior", &looked, Vec::new())[0].display_id, look("brm", 1));
        // a shield loadout fills the off hand
        let shield = ClassProgress { loadout: Some("shield".into()), ..Default::default() };
        let slots: Vec<u8> = armed(&c, "warrior", &shield, Vec::new()).iter().map(|w| w.equip_slot).collect();
        assert_eq!(slots, [15, 16]);
    }

    #[test]
    fn an_armed_class_drops_the_outfits_weapons() {
        let item = |item_id, inv_type| StartOutfitItem { item_id, display_id: item_id * 10, inv_type };
        // a shirt, a sword, a shield, a bow
        let outfit = [item(38, 4), item(25, 21), item(2362, 14), item(2504, 15)];
        let kept: Vec<u32> = unarmed_outfit(&outfit).iter().map(|i| i.item_id).collect();
        assert_eq!(kept, [38]);
    }

    #[test]
    fn a_ring_or_trinket_answers_with_its_name_rarity_slot_and_value_now() {
        use jewelry::ring_or_trinket_entry;
        let c = content();
        for (index, j) in c.jewelry.iter().enumerate() {
            let entry = ring_or_trinket_entry(&c, &j.id, 2, j.value * 1.5).unwrap();
            assert!(entry >= 1_000_000_000, "far past every other item");
            let info = item_info(&c, None, entry).unwrap();
            assert_eq!(info.name, j.name, "jewelry {index}");
            assert_eq!(info.quality, 3, "a rare card: blue");
            assert_eq!(
                info.inventory_type,
                match j.slot {
                    esurv_content::JewelrySlot::Ring => 11,
                    esurv_content::JewelrySlot::Trinket => 12,
                }
            );
            assert_eq!(
                info.description,
                j.text_at(j.value * 1.5)
            );
            assert_eq!(info.display_id, 0, "no look without the player's data");
        }
        let accuria = ring_or_trinket_entry(&c, "accuria", 0, 0.17).unwrap();
        let raised = ring_or_trinket_entry(&c, "accuria", 0, 0.25).unwrap();
        assert_ne!(accuria, raised, "an upgrade is another entry");
        assert_eq!(
            item_info(&c, None, accuria).unwrap().description,
            "+17% damage"
        );
        assert_eq!(ring_or_trinket_entry(&c, "no_such_ring", 0, 0.1), None);
    }

    #[test]
    fn an_amulet_answers_with_its_text_and_rank_and_a_legendary_with_the_spell_it_remakes() {
        use jewelry::amulet_entry;
        let c = content();
        let a = &c.amulets[0];
        let first = item_info(&c, None, amulet_entry(&c, &a.id, 1).unwrap()).unwrap();
        assert_eq!(
            (first.name.as_str(), first.description.as_str()),
            (a.name.as_str(), a.text.as_str())
        );
        assert_eq!(first.quality, u32::from(a.rarity.index()) + 1);
        assert_eq!(first.inventory_type, 2, "the neck");
        let empowered = item_info(&c, None, amulet_entry(&c, &a.id, 3).unwrap()).unwrap();
        assert!(
            empowered.description.starts_with(&a.text)
                && empowered.description.contains("Rank 3: its effects at"),
            "{}",
            empowered.description
        );

        let recipe = &c.legendary_amulets[0];
        let entry = amulet_entry(&c, &recipe.id(), 1).unwrap();
        let legendary = item_info(&c, None, entry).unwrap();
        assert_eq!(legendary.name, recipe.name);
        assert_eq!(legendary.quality, 5, "legendary: orange");
        assert_eq!(legendary.description, recipe.text);
        assert_eq!(amulet_entry(&c, "legendary:no_such_spell", 1), None);
        assert_eq!(amulet_entry(&c, "no_such_amulet", 1), None);
    }
}
