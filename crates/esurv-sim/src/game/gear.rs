//! Class gear: what an equipped piece adds to the hero, how it is named, and which look it
//! shows.
//!
//! A piece is a slot (eight armor slots, then the class's weapons) from a zone (its index in
//! the zones' difficulty order). Its stats are the slot's (or weapon's) first-zone stats
//! × the gear factor of D (`balance.ron` `zone_scaling`), D the zone's difficulty, rounded up to whole numbers (percentages to whole
//! percents, flat stats to whole points); from the second zone on, the bonus pair its class's set
//! names for the slot is added, scaled and rounded up the same way. Its name and look are its
//! zone's (every zone its own look, `gear.ron` `looks`; a zone may have
//! several rows, Blackrock two, and a piece unlocks every row of its zone); its quality colour
//! follows the zone's tier (`rewards::zone_tier`). A look is a place in the set's `looks`.

use esurv_content::{ArmamentLook, ArmorSlot, ArmorType, Content, Gear, GearSet, GearStat, Hand, LoadoutWeapon, Stat, WeaponKind, ZoneScaling};

use super::offers::DOUBLE_CAST_CAP;
use super::stats::Modifier;

/// Armor slots come first; weapon `i` of a set is slot `ARMOR_SLOTS + i`.
pub const ARMOR_SLOTS: u32 = 8;
/// How many zones a piece's item entry can name (`rewards::gear_item_entry`).
pub const MAX_GEAR_ZONES: u32 = 100;
/// How many slots a piece's item entry can name.
pub const MAX_GEAR_SLOTS: u32 = 100;
/// The weapon slots a class has room for: weapon slot `ARMOR_SLOTS + 2 × set + hand`.
pub use esurv_content::WEAPON_SLOTS;
/// The weapon slots' ids, in slot order.
const WEAPON_SLOT_IDS: [&str; WEAPON_SLOTS as usize] =
    ["main_hand", "off_hand", "main_hand_2", "off_hand_2", "main_hand_3", "off_hand_3"];

/// The id a save keeps slot `slot` by: an armor slot's (`head` ... `hands`, in `gear.ron`'s
/// order), then the weapon slots' (`main_hand`, `off_hand`, `main_hand_2` ...).
pub fn slot_id(slot: u32) -> Option<&'static str> {
    const ARMOR: [ArmorSlot; ARMOR_SLOTS as usize] = [
        ArmorSlot::Head,
        ArmorSlot::Shoulders,
        ArmorSlot::Chest,
        ArmorSlot::Waist,
        ArmorSlot::Legs,
        ArmorSlot::Feet,
        ArmorSlot::Wrists,
        ArmorSlot::Hands,
    ];
    match slot.checked_sub(ARMOR_SLOTS) {
        None => Some(ARMOR[slot as usize].id()),
        Some(w) => WEAPON_SLOT_IDS.get(w as usize).copied(),
    }
}

/// The slot a save's slot id names (see [`slot_id`]).
pub fn slot_of_id(id: &str) -> Option<u32> {
    (0..ARMOR_SLOTS + WEAPON_SLOTS).find(|&s| slot_id(s) == Some(id))
}

/// A whole number of points or percents, rounded up: a hair of float error over a whole number
/// (1.0000001 from 0.01 × 100) does not count.
fn whole_up(v: f32) -> f32 {
    (v - 1e-3).ceil()
}

/// A gear stat's Elwynn value × `factor`, rounded up our way: flat stats
/// to whole points, percentages to whole percents, so no gear stat ever shows a decimal. Double
/// cast stops at a single piece's cap (the hero's own cap is the stat sheet's); extra targets are
/// never scaled.
fn scaled(s: &GearStat, factor: f32) -> Modifier {
    let v = s.value * factor;
    let value = match s.stat {
        Stat::ExtraTargets => s.value,
        Stat::FlatMaxHealth | Stat::HealthPerSecond | Stat::HealPerKill | Stat::ExtraLives | Stat::DotStacks => whole_up(v),
        Stat::DoubleCast => (whole_up(v * 100.0) / 100.0).min(DOUBLE_CAST_CAP),
        _ => whole_up(v * 100.0) / 100.0,
    };
    Modifier::new(s.stat, value)
}

/// How much stronger than the first zone's a piece from a zone of difficulty `difficulty` is
/// (`balance.ron` `zone_scaling`: D^`gear_exponent`, steeper past `gear_steeper_from`).
pub fn zone_gear_factor(scaling: &ZoneScaling, difficulty: f32) -> f32 {
    scaling.gear_factor(difficulty)
}

/// What one piece from zone `zone` (its index in the zones' difficulty order) adds. `None` when the slot does not
/// exist for the set or the zone does not exist.
pub fn piece_modifiers(content: &Content, set: &GearSet, slot: u32, zone: u32) -> Option<Vec<Modifier>> {
    let gear = &content.gear;
    let factor = zone_gear_factor(&content.balance.zone_scaling, content.zones.get(zone as usize)?.difficulty);
    let base = if slot < ARMOR_SLOTS {
        &gear.slots.get(slot as usize)?.stats
    } else {
        &set.weapons.get((slot - ARMOR_SLOTS) as usize)?.stats
    };
    let mut out: Vec<Modifier> = base.iter().map(|s| scaled(s, factor)).collect();
    if zone >= 1 {
        let pair = gear.bonus_stats.get(*set.bonus_pairs.get(slot as usize)? as usize)?;
        out.extend(pair.iter().map(|s| scaled(s, factor)));
    }
    Some(out)
}

/// The looks of zone `zone` (its index in the zones' difficulty order), places in the set's
/// `looks`, its rows in order (every zone its own look; Blackrock two).
pub fn zone_looks(content: &Content, set: &GearSet, zone: u32) -> Vec<u32> {
    let Some(z) = content.zones.get(zone as usize) else {
        return Vec::new();
    };
    let mut rows: Vec<(u32, u32)> =
        set.looks.iter().enumerate().filter(|(_, l)| l.zone == z.id).map(|(i, l)| (l.row, i as u32)).collect();
    rows.sort_unstable();
    rows.into_iter().map(|(_, i)| i).collect()
}

/// The look a piece of zone `zone` shows of its own: the zone's first row; a zone without looks of
/// its own (one added later) shows the nearest zone's before it that has one, else the set's first.
pub fn zone_look(content: &Content, set: &GearSet, zone: u32) -> u32 {
    (0..=zone.min(content.zones.len() as u32))
        .rev()
        .find_map(|z| zone_looks(content, set, z).first().copied())
        .unwrap_or(0)
}

/// The zone (its index in the zones' difficulty order) look `look` is of; `None` for a look of a
/// zone the content does not have.
pub fn look_zone(content: &Content, set: &GearSet, look: u32) -> Option<u32> {
    let l = set.looks.get(look as usize)?;
    content.zones.iter().position(|z| z.id == l.zone).map(|z| z as u32)
}

/// Every look the content's zones show, as (zone, look), in the zones' difficulty order, a zone's
/// rows in order: the Collection Log's rows.
pub fn shown_looks(content: &Content, set: &GearSet) -> Vec<(u32, u32)> {
    (0..content.zones.len() as u32).flat_map(|z| zone_looks(content, set, z).into_iter().map(move |l| (z, l))).collect()
}

/// The item display id an armor piece shows in look `look` (a place in the set's `looks`). A 0 in
/// the table borrows the nearest look that has one: one look before, then one after, then two
/// before ...
pub fn armor_display(set: &GearSet, slot: u32, look: u32) -> u32 {
    let d = |l: i64| -> u32 {
        usize::try_from(l).ok().and_then(|l| set.looks.get(l)).and_then(|l| l.displays.get(slot as usize)).copied().unwrap_or(0)
    };
    let look = i64::from(look);
    (0..set.looks.len() as i64)
        .flat_map(|step| [look - step, look + step])
        .map(d)
        .find(|&x| x != 0)
        .unwrap_or(0)
}

/// The loadout weapon whose look a class gear weapon (slot `slot`) shows in the collection, and
/// that look in the armaments of look `look`: the first of the class's loadouts with a weapon in
/// the piece's hand (a ranged weapon for a ranged piece), else the first loadout's first weapon.
pub fn armament<'c>(content: &'c Content, set: &'c GearSet, slot: u32, look: u32) -> Option<(&'c LoadoutWeapon, &'c ArmamentLook)> {
    let piece = set.weapons.get(slot.checked_sub(ARMOR_SLOTS)? as usize)?;
    let loadouts = content.loadouts_of(&set.class);
    let w = loadouts
        .iter()
        .flat_map(|l| &l.weapons)
        .find(|w| w.hand == piece.hand)
        .or_else(|| loadouts.first()?.weapons.first())?;
    Some((w, w.look_in(set.looks.get(look as usize)?)?))
}

/// The piece's item name in look `look`: the armor piece's own name in it (`gear.ron` `looks`);
/// a weapon is its zone's armaments ("<zone> Armaments", an off-hand weapon "<zone> Off-hand
/// Armaments"). `None` for a slot the set lacks or a look of no zone.
pub fn piece_name(content: &Content, set: &GearSet, slot: u32, look: u32) -> Option<String> {
    let l = set.looks.get(look as usize)?;
    if slot >= ARMOR_SLOTS {
        let w = set.weapons.get((slot - ARMOR_SLOTS) as usize)?;
        let zone = &content.zones.get(look_zone(content, set, look)? as usize)?.name;
        return Some(armaments_name(w, zone));
    }
    l.names.get(slot as usize).cloned()
}

/// A class gear weapon's name as its zone's armaments: "<zone> Armaments", an off-hand weapon's
/// "<zone> Off-hand Armaments".
fn armaments_name(w: &esurv_content::Weapon, zone: &str) -> String {
    let hand = if w.hand == Hand::OffHand { "Off-hand " } else { "" };
    format!("{zone} {hand}Armaments")
}

/// The name of the piece of slot `slot` from zone `zone`: its zone's own look's (a weapon its
/// zone's armaments).
pub fn zone_piece_name(content: &Content, set: &GearSet, slot: u32, zone: u32) -> Option<String> {
    let z = content.zones.get(zone as usize)?;
    match slot.checked_sub(ARMOR_SLOTS) {
        Some(w) => Some(armaments_name(set.weapons.get(w as usize)?, &z.name)),
        None => piece_name(content, set, slot, zone_look(content, set, zone)),
    }
}

//// The look slot `slot` shows over the piece of zone `zone`: the look `chosen` for it (one the set
/// has), else the piece's own.
pub fn shown_look(content: &Content, set: &GearSet, chosen: Option<u32>, zone: u32) -> u32 {
    chosen.filter(|&l| (l as usize) < set.looks.len()).unwrap_or_else(|| zone_look(content, set, zone))
}

/// The look whose armaments a class's weapon loadout shows (a zone's
/// weapon drop unlocks its armaments for all three loadouts): the look of the class gear weapon
/// worn (the latest, where it wears several; the look `chosen` for its slot, else its own), the
/// first zone's with none. `worn` are item entries; another class's are skipped.
pub fn weapon_look(content: &Content, set: &GearSet, wow_class_id: u32, worn: &[u32], chosen: impl Fn(u32) -> Option<u32>) -> u32 {
    worn.iter()
        .filter_map(|&entry| decode_item_entry(entry))
        .filter(|&(owner, slot, _)| owner == wow_class_id && slot >= ARMOR_SLOTS)
        .map(|(_, slot, zone)| shown_look(content, set, chosen(slot), zone))
        .max()
        .unwrap_or_else(|| zone_look(content, set, 0))
}

/// The weapons class `class`'s loadout `loadout` (its id; none or one it lacks: its first) holds
/// in look `look`'s armaments: each weapon's hand, kind (the look's own grip) and display.
pub fn loadout_armaments(content: &Content, class: &str, loadout: Option<&str>, look: u32) -> Vec<(Hand, WeaponKind, u32)> {
    let (Some(l), Some(set)) = (content.loadout(class, loadout), content.gear_set(class)) else {
        return Vec::new();
    };
    let Some(look) = set.looks.get(look as usize) else {
        return Vec::new();
    };
    l.weapons
        .iter()
        .filter_map(|w| {
            let a = w.look_in(look).filter(|a| a.display != 0)?;
            Some((w.hand, w.kind_of(a), a.display))
        })
        .collect()
}

// WoW inventory type of an armor piece (a cloth chest is a robe).
pub fn armor_inventory_type(gear: &Gear, set: &GearSet, slot: u32) -> u32 {
    if slot == 2 && set.armor == ArmorType::Cloth {
        esurv_content::ROBE_INVENTORY_TYPE
    } else {
        gear.slots[slot as usize].inventory_type()
    }
}

/// A class gear item entry split back into (wow class id, slot, zone); see `rewards::gear_item_entry`.
pub fn decode_item_entry(entry: u32) -> Option<(u32, u32, u32)> {
    let e = entry.checked_sub(super::rewards::GEAR_ENTRY_BASE)?;
    if e >= 1_200_000 || e % 10 != 0 {
        return None;
    }
    Some((e / 100_000, (e / 1000) % MAX_GEAR_SLOTS, (e / 10) % MAX_GEAR_ZONES))
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_content::Content;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    #[test]
    fn a_hero_brings_its_classs_talents_and_worn_gear() {
        use esurv_content::TalentEffect;
        let c = content();
        let warrior = c.classes.iter().find(|k| k.id == "warrior").unwrap();
        let talent = c
            .talent_trees
            .iter()
            .filter(|t| t.class == "warrior")
            .flat_map(|t| &t.talents)
            .find(|t| !t.effects.is_empty() && t.effects.iter().all(|e| matches!(e, TalentEffect::Stat { .. })))
            .expect("a warrior talent that only raises stats");
        let mine = super::super::rewards::gear_item_entry(warrior.wow_class_id, 0, 1);
        let theirs = super::super::rewards::gear_item_entry(warrior.wow_class_id + 1, 0, 1);
        let setup = crate::game::run::HeroSetup::with_progress(&c, "warrior", "human", [(talent.id.as_str(), 99)], &[mine, theirs, 42]);
        let mut want: Vec<(Stat, f32)> = talent
            .effects
            .iter()
            .filter_map(|e| match e {
                TalentEffect::Stat { stat, per_rank } => Some((*stat, per_rank * f32::from(talent.max_rank))),
                _ => None,
            })
            .collect();
        let piece = piece_modifiers(&c, c.gear_set("warrior").unwrap(), 0, 1).unwrap();
        want.extend(piece.iter().map(|m| (m.stat, m.value)));
        // the rank past the talent's last counts as its last; another class's piece and an unknown
        // entry bring nothing
        approx(&setup.modifiers, &want);
        assert_eq!((setup.class.as_str(), setup.race.as_str()), ("warrior", "human"));
    }

    fn approx(m: &[Modifier], want: &[(Stat, f32)]) {
        assert_eq!(m.len(), want.len(), "{m:?}");
        for (a, (s, v)) in m.iter().zip(want) {
            assert_eq!(a.stat, *s);
            assert!((a.value - v).abs() < 1e-4, "{a:?} vs {v}");
        }
    }

    /// A stat of a piece, zone by zone (Elwynn to `aq`).
    fn by_zone(c: &Content, class: &str, slot: u32, stat: Stat) -> Vec<f32> {
        let set = c.gear_set(class).unwrap();
        (0..c.zones.len() as u32)
            .map(|z| piece_modifiers(c, set, slot, z).unwrap().iter().filter(|m| m.stat == stat).map(|m| m.value).next().unwrap())
            .collect()
    }

    fn close(got: &[f32], want: &[f32]) {
        assert_eq!(got.len(), want.len(), "{got:?}");
        assert!(got.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-4), "{got:?} vs {want:?}");
    }

    #[test]
    fn each_zone_scales_the_first_zone_stats_by_its_gear_factor() {
        let c = content();
        let zones: Vec<f32> = c.zones.iter().map(|z| z.difficulty).collect();
        assert_eq!(zones, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
        let s = &c.balance.zone_scaling;
        let w = "warrior";
        // (slot, stat, first-zone value, whole points or whole percents)
        let cases = [
            (0, Stat::CritChance, 0.01, false),
            (0, Stat::CritDamage, 0.04, false),
            (2, Stat::FlatMaxHealth, 15.0, true),
            (4, Stat::DoubleCast, 0.02, false),
            (5, Stat::FlatMaxHealth, 10.0, true),
            (7, Stat::Luck, 0.1, false),
            (ARMOR_SLOTS, Stat::Damage, 0.08, false),
        ];
        for (slot, stat, first, whole) in cases {
            let want: Vec<f32> = zones
                .iter()
                .map(|&d| if whole { whole_up(first * s.gear_factor(d)) } else { whole_up(first * s.gear_factor(d) * 100.0) / 100.0 })
                .collect();
            close(&by_zone(&c, w, slot, stat), &want);
        }
        // the rogue's off hand: 4 % damage and 6 health in Elwynn
        let rogue = c.gear_set("rogue").unwrap();
        approx(&piece_modifiers(&c, rogue, ARMOR_SLOTS + 1, 0).unwrap(), &[(Stat::Damage, 0.04), (Stat::FlatMaxHealth, 6.0)]);
        assert!(piece_modifiers(&c, rogue, ARMOR_SLOTS + 1, c.zones.len() as u32).is_none(), "no such zone");
    }

    #[test]
    fn the_gear_factor_is_a_power_of_d_then_steeper() {
        let s = ZoneScaling { gear_exponent: 0.5, gear_steeper_from: 9.0, gear_growth_after: 1.25, ..ZoneScaling::default() };
        for (d, want) in [(1.0, 1.0), (4.0, 2.0), (9.0, 3.0), (10.0, 3.75), (11.0, 4.6875)] {
            assert!((zone_gear_factor(&s, d) - want).abs() < 1e-5, "D {d}");
        }
    }

    #[test]
    fn every_gear_stat_is_a_whole_number_after_rounding() {
        let c = content();
        for class in &c.classes {
            let set = c.gear_set(&class.id).unwrap();
            for slot in 0..ARMOR_SLOTS + set.weapons.len() as u32 {
                for zone in 0..c.zones.len() as u32 {
                    for m in piece_modifiers(&c, set, slot, zone).unwrap() {
                        let whole = if m.stat == Stat::FlatMaxHealth || m.stat == Stat::HealPerKill { m.value } else { m.value * 100.0 };
                        assert!((whole - whole.round()).abs() < 1e-3, "{} slot {slot} zone {zone}: {m:?}", class.id);
                    }
                }
            }
        }
    }

    #[test]
    fn a_piece_of_double_cast_stops_at_a_single_pieces_cap() {
        let s = GearStat { stat: Stat::DoubleCast, value: 0.5 };
        assert_eq!(scaled(&s, 4.0).value, DOUBLE_CAST_CAP);
        assert_eq!(scaled(&GearStat { stat: Stat::HealthPerSecond, value: 0.2 }, 1.0).value, 1.0, "whole points, rounded up");
        // a whole number stays itself; anything above it goes up
        assert_eq!(scaled(&GearStat { stat: Stat::CritChance, value: 0.01 }, 1.0).value, 0.01);
        assert_eq!(scaled(&GearStat { stat: Stat::CritChance, value: 0.01 }, 1.05).value, 0.02);
        assert_eq!(scaled(&GearStat { stat: Stat::FlatMaxHealth, value: 15.0 }, 1.46).value, 22.0);
    }

    #[test]
    fn elwynn_pieces_are_the_base_stats_and_bring_no_bonus_pair() {
        let c = content();
        let set = c.gear_set("warrior").unwrap();
        approx(&piece_modifiers(&c, set, 0, 0).unwrap(), &[(Stat::CritChance, 0.01), (Stat::CritDamage, 0.04)]);
        approx(&piece_modifiers(&c, set, 7, 0).unwrap(), &[(Stat::Luck, 0.1)]);
    }

    #[test]
    fn pieces_after_elwynn_add_the_class_bonus_pair_scaled_and_rounded_up() {
        let c = content();
        let set = c.gear_set("warrior").unwrap();
        // warrior chest (slot 2): 15 health and 1 % damage taken less, then bonus pair 5 = 10 health
        // + 1 health per kill; each × the zone's gear factor, rounded up
        for zone in [1, 8] {
            let f = c.balance.zone_scaling.gear_factor(c.zones[zone].difficulty);
            approx(&piece_modifiers(&c, set, 2, zone as u32).unwrap(), &[
                (Stat::FlatMaxHealth, whole_up(15.0 * f)),
                (Stat::DamageReduction, whole_up(f) / 100.0),
                (Stat::FlatMaxHealth, whole_up(10.0 * f)),
                (Stat::HealPerKill, whole_up(f)),
            ]);
        }
    }

    #[test]
    fn a_new_zone_gets_its_own_gear_level_from_its_difficulty() {
        let mut c = content();
        let mut extra = c.zones[1].clone();
        extra.difficulty = 1.5;
        c.zones.push(extra);
        let z = c.zones.len() as u32 - 1;
        let f = 1.5f32.powf(c.balance.zone_scaling.gear_exponent);
        assert!((zone_gear_factor(&c.balance.zone_scaling, 1.5) - f).abs() < 1e-6);
        let set = c.gear_set("warrior").unwrap();
        let m = piece_modifiers(&c, set, 2, z).unwrap();
        approx(&m[..2], &[(Stat::FlatMaxHealth, whole_up(15.0 * f)), (Stat::DamageReduction, whole_up(f) / 100.0)]);
    }

    #[test]
    fn weapons_names_and_looks() {
        let c = content();
        let w = c.gear_set("warrior").unwrap();
        approx(&piece_modifiers(&c, w, 8, 0).unwrap(), &[(Stat::Damage, 0.08)]);
        assert!(piece_modifiers(&c, w, 9, 0).is_none());
        // every zone its own names and looks
        assert_eq!(zone_piece_name(&c, w, 0, 0).unwrap(), "Recruit's Helm");
        assert_eq!(zone_piece_name(&c, w, 0, 1).unwrap(), "Sparkmetal Coif");
        assert_eq!(zone_piece_name(&c, w, 0, 3).unwrap(), "Adventurer's Helm");
        assert_eq!(zone_piece_name(&c, w, 8, 9).unwrap(), format!("{} Armaments", c.zones[9].name));
        let rogue = c.gear_set("rogue").unwrap();
        assert_eq!(zone_piece_name(&c, rogue, 9, 1).unwrap(), format!("{} Off-hand Armaments", c.zones[1].name));
        // Elwynn has no helm look: it borrows the next zone's
        assert_eq!(armor_display(w, 0, 0), w.looks[1].displays[0]);
        assert_eq!(armor_display(w, 1, 0), w.looks[0].displays[1]);
        let mage = c.gear_set("mage").unwrap();
        assert_eq!(armor_inventory_type(&c.gear, mage, 2), 20);
        assert_eq!(zone_piece_name(&c, mage, 2, 0).unwrap(), "Apprentice's Robes");
        // a weapon piece shows the class's first loadout's weapon in its hand, in the zone's look
        let (sword, look) = armament(&c, w, 8, zone_look(&c, w, 1)).unwrap();
        assert_eq!((sword.hand, look.name.as_str()), (esurv_content::Hand::MainHand, "Duskbringer"));
        let shaman = c.gear_set("shaman").unwrap();
        let (shield, _) = armament(&c, shaman, 9, 0).unwrap();
        assert_eq!(shield.kind, esurv_content::WeaponKind::Shield);
    }

    /// Blackrock has two rows of looks, Molten Core's then Blackwing
    /// Lair's; a zone without looks of its own (one added later) shows the zone's before it.
    #[test]
    fn a_zone_s_looks_are_its_rows_in_order() {
        let mut c = content();
        let brm = c.zones.iter().position(|z| z.id == "brm").unwrap() as u32;
        for class in &c.classes {
            let set = c.gear_set(&class.id).unwrap();
            let rows = zone_looks(&c, set, brm);
            assert_eq!(rows.len(), 2, "{}", class.id);
            assert_eq!(rows.iter().map(|&l| set.looks[l as usize].row).collect::<Vec<_>>(), [0, 1]);
            assert_eq!(zone_look(&c, set, brm), rows[0]);
            assert_eq!(rows.iter().map(|&l| look_zone(&c, set, l)).collect::<Vec<_>>(), [Some(brm), Some(brm)]);
            // a row per zone, Blackrock's and Stratholme's two
            assert_eq!(shown_looks(&c, set).len(), c.zones.len() + 2);
            for z in 0..c.zones.len() as u32 {
                assert!(zone_looks(&c, set, z).iter().all(|&l| (0..8).all(|s| armor_display(set, s, l) != 0)), "{} zone {z}", class.id);
            }
        }
        let w = c.gear_set("warrior").unwrap();
        let rows = zone_looks(&c, w, brm);
        assert_eq!(rows.iter().map(|&l| piece_name(&c, w, 0, l).unwrap()).collect::<Vec<_>>(), ["Helm of Might", "Helm of Wrath"]);
        // a look of a zone the content does not have waits for it
        let naxx = w.look_of("naxxramas", 0).unwrap() as u32;
        assert_eq!(look_zone(&c, w, naxx), None);
        assert!(!shown_looks(&c, w).iter().any(|&(_, l)| l == naxx));
        let mut extra = c.zones[1].clone();
        extra.id = "no_looks_yet".into();
        c.zones.insert(2, extra);
        let w = c.gear_set("warrior").unwrap();
        assert_eq!(zone_look(&c, w, 2), zone_look(&c, w, 1));
    }

    #[test]
    fn item_entries_name_class_slot_and_zone() {
        let e = super::super::rewards::gear_item_entry(8, 9, 3);
        assert_eq!(e, 2_809_030);
        assert_eq!(decode_item_entry(e), Some((8, 9, 3)));
        // a warrior's off hand of the second weapon set from zone 8
        let set_2 = super::super::rewards::gear_item_entry(1, ARMOR_SLOTS + 3, 8);
        assert_eq!(set_2, 2_111_080);
        assert_eq!(decode_item_entry(set_2), Some((1, 11, 8)));
        let last = ARMOR_SLOTS + WEAPON_SLOTS - 1;
        let far = super::super::rewards::gear_item_entry(11, last, MAX_GEAR_ZONES - 1);
        assert_eq!(decode_item_entry(far), Some((11, last, MAX_GEAR_ZONES - 1)));
        assert_eq!(decode_item_entry(99_999), None);
        assert_eq!(decode_item_entry(101_000), None, "the first numbering is no piece");
        assert_eq!(decode_item_entry(1_089_030), None, "nor is the second");
        assert_eq!(decode_item_entry(2_010_051), None, "a look's entry is no piece");
    }

    #[test]
    fn every_slot_has_an_id_a_save_keeps() {
        assert_eq!((slot_id(0), slot_id(7)), (Some("head"), Some("hands")));
        assert_eq!((slot_id(ARMOR_SLOTS), slot_id(ARMOR_SLOTS + 5)), (Some("main_hand"), Some("off_hand_3")));
        assert_eq!(slot_id(ARMOR_SLOTS + WEAPON_SLOTS), None);
        for s in 0..ARMOR_SLOTS + WEAPON_SLOTS {
            assert_eq!(slot_of_id(slot_id(s).unwrap()), Some(s));
        }
        assert_eq!(slot_of_id("tabard"), None);
        // the armor ids are gear.ron's slots', in its order
        let c = content();
        let ids: Vec<&str> = c.gear.slots.iter().map(|s| s.slot.id()).collect();
        assert_eq!(ids, (0..ARMOR_SLOTS).map(|s| slot_id(s).unwrap()).collect::<Vec<_>>());
    }
}
