//! The save's sanity checks: numbers no play can reach are reined in to the most a player could
//! have, never wiped. Run on every save opened and every save imported (`Saves::rein_in`).
//!
//! * A class's talent experience is at most what it held when play time began to be kept plus the
//!   most its run time can earn (`talent_experience_ceiling`).
//! * Its talents are bought again, row by row, under the armory's rules with the points that
//!   experience earns: ranks past a talent's last, rows not yet open, a missing required talent
//!   or too few points drop the ranks that break them (the deepest first), and the rest stay.
//!   Its prestige ranks the same, with the prestige points.
//! * Gear from a zone the class has not opened leaves the collection; a worn piece not in the
//!   collection comes off; a look of a zone whose piece it lacks goes back to the piece's own.
//! * What it has seen is no more than it holds; a hero's wins no more than its runs.

use esurv_content::Content;
use esurv_sim::game::{prestige, talents};

use super::{ClassProgress, SaveFile};

/// What was reined in, one line each, for the log; empty when the save made sense.
pub(super) fn rein_in(content: &Content, file: &mut SaveFile, ceiling: impl Fn(f64) -> u64) -> Vec<String> {
    let mut notes = Vec::new();
    for (class, p) in &mut file.classes {
        for note in rein_in_class(content, class, p, &ceiling) {
            notes.push(format!("{class}: {note}"));
        }
    }
    for h in &mut file.heroes {
        if h.records.wins > h.records.runs {
            notes.push(format!("{}: {} wins in {} runs", h.name, h.records.wins, h.records.runs));
            h.records.wins = h.records.runs;
        }
    }
    let options = file.run_options.on_the_sliders(content);
    if options != file.run_options {
        notes.push("run options off their sliders".into());
        file.run_options = options;
    }
    notes
}

fn rein_in_class(content: &Content, class: &str, p: &mut ClassProgress, ceiling: &impl Fn(f64) -> u64) -> Vec<String> {
    let mut notes = Vec::new();
    let most = u64::from(p.experience_before_tracking).saturating_add(ceiling(p.run_seconds as f64));
    if u64::from(p.talent_experience) > most {
        notes.push(format!("talent experience {} past the most its play time earns, {most}", p.talent_experience));
        p.talent_experience = most as u32;
    }

    // talents bought again under the rules, shallow rows first
    let points = talents::points_earned(p.talent_experience);
    let mut wanted: Vec<(u8, String, u32)> = content
        .talent_trees
        .iter()
        .filter(|t| t.class == class)
        .flat_map(|t| t.talents.iter())
        .filter_map(|t| p.talents.get(&t.id).map(|&r| (t.row, t.id.clone(), r)))
        .collect();
    wanted.sort();
    let mut bought = talents::Ranks::new();
    loop {
        let mut grew = false;
        for (_, id, want) in &wanted {
            while bought.get(id).copied().unwrap_or(0) < *want && talents::learn(content, class, &mut bought, id, points) {
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    if bought != p.talents {
        notes.push(format!("talents {} ranks kept of {}", talents::points_spent(&bought), talents::points_spent(&p.talents)));
        p.talents = bought;
    }

    let points = prestige::points_earned(p.talent_experience);
    let mut bought = prestige::PrestigeRanks::new();
    for (id, &want) in &p.prestige {
        while bought.get(id).copied().unwrap_or(0) < want && prestige::learn(content, class, &mut bought, id, points) {}
    }
    if bought != p.prestige {
        notes.push(format!("prestige {} ranks kept of {}", prestige::points_spent(&bought), prestige::points_spent(&p.prestige)));
        p.prestige = bought;
    }

    // gear only from the zones open to it; a zone the content no longer has is left alone
    let open = p.open_zones(content);
    let before = p.collection.len();
    p.collection.retain(|piece| content.zones.iter().position(|z| z.id == piece.zone).is_none_or(|z| z < open));
    if p.collection.len() < before {
        notes.push(format!("{} gear pieces from zones it has not opened", before - p.collection.len()));
    }
    let before = p.worn_pieces.len();
    let mut slots = std::collections::BTreeSet::new();
    let collection = &p.collection;
    p.worn_pieces.retain(|w| collection.contains(w) && slots.insert(w.slot.clone()));
    if p.worn_pieces.len() < before {
        notes.push(format!("{} worn pieces it does not own", before - p.worn_pieces.len()));
    }
    let before = p.looks.len();
    p.looks.retain(|slot, look| collection.iter().any(|c| c.slot == *slot && c.zone == look.zone));
    if p.looks.len() < before {
        notes.push(format!("{} looks of gear it does not own", before - p.looks.len()));
    }

    if let Some(seen) = &mut p.seen {
        let holding = (p.collection.len() as u32, talents::points_earned(p.talent_experience) + prestige::points_earned(p.talent_experience));
        seen.pieces = seen.pieces.min(holding.0);
        seen.points = seen.points.min(holding.1);
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::SavedPiece;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    /// Ten experience a second: the most a class here can earn.
    fn ceiling(seconds: f64) -> u64 {
        (seconds * 10.0) as u64
    }

    fn piece(slot: &str, zone: &str) -> SavedPiece {
        SavedPiece { slot: slot.into(), zone: zone.into() }
    }

    #[test]
    fn a_save_that_makes_sense_is_left_as_it_is() {
        let content = content();
        let mut mage = ClassProgress { talent_experience: 600, run_seconds: 60, ..Default::default() };
        let points = talents::points_earned(mage.talent_experience);
        let first = content.talent_trees.iter().find(|t| t.class == "mage").unwrap().talents[0].id.clone();
        assert!(talents::learn(&content, "mage", &mut mage.talents, &first, points));
        mage.collection.insert(piece("head", &content.zones[0].id));
        mage.worn_pieces.push(piece("head", &content.zones[0].id));
        let mut file = SaveFile::default();
        file.classes.insert("mage".into(), mage.clone());
        assert_eq!(rein_in(&content, &mut file, ceiling), Vec::<String>::new());
        assert_eq!(file.classes["mage"], mage);
    }

    #[test]
    fn numbers_no_play_reaches_are_reined_in_never_wiped() {
        let content = content();
        let mut mage = ClassProgress {
            talent_experience: 1_000_000,
            experience_before_tracking: 100,
            run_seconds: 60,
            ..Default::default()
        };
        // every talent at a rank past its last
        for t in content.talent_trees.iter().filter(|t| t.class == "mage").flat_map(|t| t.talents.iter()) {
            mage.talents.insert(t.id.clone(), 9);
        }
        let last = content.zones.last().unwrap().id.clone();
        mage.collection.insert(piece("head", &content.zones[0].id));
        mage.collection.insert(piece("chest", &last));
        mage.worn_pieces = vec![piece("chest", &last), piece("legs", &content.zones[0].id)];
        let mut file = SaveFile::parse(
            "(gear_numbering: 2, next_guid: 2, heroes: [(guid: 1, name: \"Ana\", class: \"mage\", race: \"human\", \n            gender: 1, look: (0, 0, 0, 0, 0), records: (best_level: 1, best_seconds: 0, runs: 2, wins: 7))])",
        )
        .unwrap();
        file.classes.insert("mage".into(), mage);

        let notes = rein_in(&content, &mut file, ceiling);
        let mage = &file.classes["mage"];
        assert_eq!(mage.talent_experience, 100 + 600, "{notes:?}");
        let earned = talents::points_earned(mage.talent_experience);
        assert!(talents::points_spent(&mage.talents) <= earned);
        assert!(!mage.talents.is_empty(), "what the points buy is kept");
        for (id, &rank) in &mage.talents {
            let mut again = mage.talents.clone();
            again.remove(id);
            for _ in 0..rank {
                assert!(talents::learn(&content, "mage", &mut again, id, u32::MAX), "{id} rank {rank}");
            }
        }
        assert_eq!(mage.collection, [piece("head", &content.zones[0].id)].into());
        assert!(mage.worn_pieces.is_empty(), "{:?}", mage.worn_pieces);
        assert_eq!(file.heroes[0].records.wins, 2);
        assert!(notes.len() >= 4, "{notes:?}");
    }
}
