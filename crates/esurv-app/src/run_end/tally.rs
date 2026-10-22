//! The end panel's texts, in our own words: its title and subtitle, the tally's rows, the line for
//! a newly opened zone, and the gear won with its names, slots and colours.

use esurv_content::Content;
use esurv_protocol::{Ending, Ready, RunEnd};
use esurv_sim::game::gear::{decode_item_entry, zone_piece_name, ARMOR_SLOTS};
use esurv_sim::game::rewards::zone_tier;

use super::state::WonPiece;

/// The big title over the panel.
pub fn title(ending: Ending) -> &'static str {
    match ending {
        Ending::Fell => "Overrun",
        Ending::Victory => "Triumph",
        Ending::Left => "",
    }
}

/// Under the title: the hero, its race and class, then the zone.
pub fn subtitle(who: &Ready) -> String {
    format!(
        "{}, {} {} \u{2013} {}",
        who.name, who.race, who.class, who.zone
    )
}

/// The header of the panel's window.
pub const WINDOW_TITLE: &str = "Run Report";

/// One row of the tally: what it counts and how much.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TallyRow {
    pub label: &'static str,
    pub value: String,
}

/// The tally, top to bottom: time, level, what was slain, damage both ways, gold and caches.
pub fn tally_rows(e: &RunEnd) -> Vec<TallyRow> {
    let row = |label, value: String| TallyRow { label, value };
    vec![
        row("Time lasted", run_time(e.seconds)),
        row("Level reached", e.level.to_string()),
        row("Enemies defeated", whole_number(e.kills.into())),
        row("Elites defeated", whole_number(e.elites.into())),
        row("Rares defeated", whole_number(e.rares.into())),
        row("Bosses defeated", whole_number(e.bosses.into())),
        row("Damage done", whole_number(e.damage_dealt)),
        row("Damage suffered", whole_number(e.damage_taken)),
        row("Gold collected", whole_number(e.gold.into())),
        row("Caches looted", whole_number(e.caches.into())),
    ]
}

/// A run's time as minutes and seconds (`16:05`), whole seconds down.
pub fn run_time(seconds: f32) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// A whole number with its thousands marked (`4,812,331`).
pub fn whole_number(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The line for a zone the run opened to the class.
pub fn zone_opened(zone: &str) -> String {
    format!("The way to {zone} lies open")
}

/// A piece of gear as the panel lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootRow {
    pub name: String,
    /// Its quality colour, 0 common to 4 legendary: its tier.
    pub rarity: u8,
    /// Where it is worn, as the stock interface names it.
    pub slot: &'static str,
    pub new: bool,
}

/// The gear won, named by what its drop said, else from the content (else its item entry).
pub fn loot_rows(won: &[WonPiece], content: Option<&Content>) -> Vec<LootRow> {
    won.iter()
        .map(|p| {
            let decoded = decode_item_entry(p.entry);
            let tier = decoded
                .zip(content)
                .map_or(0, |((_, _, zone), c)| zone_tier(&c, zone as usize));
            LootRow {
                name: p
                    .name
                    .clone()
                    .or_else(|| content.and_then(|c| gear_name(c, p.entry)))
                    .unwrap_or_else(|| format!("Item {}", p.entry)),
                rarity: tier.min(4) as u8,
                slot: content.map_or("", |c| gear_slot(c, p.entry)),
                new: p.new,
            }
        })
        .collect()
}

/// The heading over the gear won: how many pieces, how many new to the collection.
pub fn loot_heading(rows: &[LootRow]) -> String {
    let new = rows.iter().filter(|r| r.new).count();
    let pieces = if rows.len() == 1 { "piece" } else { "pieces" };
    format!("Gear won: {} {pieces}, {new} new", rows.len())
}

/// What the panel says of a piece: new to the collection, or one it had.
pub fn loot_status(new: bool) -> &'static str {
    if new {
        "New"
    } else {
        "Already owned"
    }
}

/// A class gear piece's name, from its item entry.
pub fn gear_name(content: &Content, entry: u32) -> Option<String> {
    let (wow_class, slot, zone) = decode_item_entry(entry)?;
    let class = content
        .classes
        .iter()
        .find(|c| c.wow_class_id == wow_class)?;
    let set = content.gear_set(&class.id)?;
    zone_piece_name(content, set, slot, zone)
}

/// Where a class gear piece is worn, by the stock interface's name for its inventory type.
pub fn gear_slot(content: &Content, entry: u32) -> &'static str {
    let Some((wow_class, slot, _)) = decode_item_entry(entry) else {
        return "";
    };
    let inventory_type = if slot < ARMOR_SLOTS {
        content
            .gear
            .slots
            .get(slot as usize)
            .map(|s| s.inventory_type())
    } else {
        content
            .classes
            .iter()
            .find(|c| c.wow_class_id == wow_class)
            .and_then(|c| content.gear_set(&c.id))
            .and_then(|set| set.weapons.get((slot - ARMOR_SLOTS) as usize))
            .map(|w| w.inventory_type())
    };
    inventory_type.map_or("", slot_name)
}

/// The stock interface's name for an inventory type (its `INVTYPE_*` strings).
pub fn slot_name(inventory_type: u32) -> &'static str {
    match inventory_type {
        1 => "Head",
        2 => "Neck",
        3 => "Shoulder",
        4 => "Shirt",
        5 | 20 => "Chest",
        6 => "Waist",
        7 => "Legs",
        8 => "Feet",
        9 => "Wrist",
        10 => "Hands",
        11 => "Finger",
        12 => "Trinket",
        13 => "One-Hand",
        14 => "Off Hand",
        15 | 26 => "Ranged",
        16 => "Back",
        17 => "Two-Hand",
        21 => "Main Hand",
        22 => "Off Hand",
        23 => "Held In Off-hand",
        25 => "Thrown",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::super::state::tests::run_end;
    use super::*;

    fn content() -> &'static Content {
        static CONTENT: OnceLock<Content> = OnceLock::new();
        CONTENT.get_or_init(|| {
            Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
        })
    }

    #[test]
    fn the_tally_reads_in_order_with_its_numbers_marked() {
        let e = RunEnd {
            seconds: 987.9,
            damage_dealt: 4_812_331,
            ..run_end(Ending::Victory)
        };
        let rows = tally_rows(&e);
        let labels: Vec<&str> = rows.iter().map(|r| r.label).collect();
        assert_eq!(
            labels,
            [
                "Time lasted",
                "Level reached",
                "Enemies defeated",
                "Elites defeated",
                "Rares defeated",
                "Bosses defeated",
                "Damage done",
                "Damage suffered",
                "Gold collected",
                "Caches looted",
            ]
        );
        assert_eq!(rows[0].value, "16:27");
        assert_eq!(rows[1].value, "6");
        assert_eq!(rows[6].value, "4,812,331");
        assert_eq!(rows[7].value, "678");
    }

    #[test]
    fn times_and_numbers_read_as_the_stock_interface_writes_them() {
        assert_eq!(run_time(0.0), "0:00");
        assert_eq!(run_time(59.99), "0:59");
        assert_eq!(run_time(61.0), "1:01");
        assert_eq!(run_time(3725.0), "62:05");
        assert_eq!(run_time(-3.0), "0:00");
        assert_eq!(whole_number(0), "0");
        assert_eq!(whole_number(999), "999");
        assert_eq!(whole_number(1000), "1,000");
        assert_eq!(whole_number(20_115), "20,115");
        assert_eq!(whole_number(1_234_567_890), "1,234,567,890");
    }

    #[test]
    fn titles_and_lines_are_ours() {
        assert_eq!(title(Ending::Fell), "Overrun");
        assert_eq!(title(Ending::Victory), "Triumph");
        let who = Ready {
            hero: 1,
            name: "Probe".into(),
            class: "Class 9".into(),
            race: "Race 1".into(),
            zone: "Area 12".into(),
            subtitle: String::new(),
        };
        assert_eq!(
            subtitle(&who),
            "Probe, Race 1 Class 9 \u{2013} Area 12"
        );
        assert_eq!(zone_opened("Area 10"), "The way to Area 10 lies open");
    }

    #[test]
    fn gear_is_named_and_placed_from_the_content() {
        let c = content();
        // a mage's (class 8) head from Elwynn; its staff (slot 8) from `aq`
        assert_eq!(gear_slot(c, 2_800_000), "Head");
        assert_eq!(gear_slot(c, 2_808_080), "Two-Hand");
        assert!(gear_name(c, 2_800_000).is_some_and(|n| !n.is_empty()));
        assert_eq!(gear_name(c, 42), None);
        let won = [
            WonPiece {
                entry: 2_800_000,
                name: Some("Told Name".into()),
                new: true,
            },
            WonPiece {
                entry: 2_808_080,
                name: None,
                new: false,
            },
            WonPiece {
                entry: 7,
                name: None,
                new: false,
            },
        ];
        let rows = loot_rows(&won, Some(c));
        assert_eq!(rows[0].name, "Told Name", "the drop's own name first");
        assert_eq!(
            (rows[0].rarity, rows[0].slot, rows[0].new),
            (0, "Head", true)
        );
        assert_eq!(rows[1].name, gear_name(c, 2_808_080).unwrap());
        assert_eq!(rows[1].rarity, 4, "aq's tier 4 is legendary");
        assert_eq!((rows[2].name.as_str(), rows[2].slot), ("Item 7", ""));
        assert_eq!(loot_heading(&rows), "Gear won: 3 pieces, 1 new");
        assert_eq!(loot_heading(&rows[..1]), "Gear won: 1 piece, 1 new");
        assert_eq!(loot_rows(&won[1..2], None)[0].name, "Item 2808080");
    }
}
