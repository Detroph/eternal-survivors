//! Zones on disk: one file per zone, `content/zones/<id>.ron`, each holding one
//! [`ZoneEntry`]. Zones are ordered by their difficulty `D`, ascending; that order is the unlock
//! path, the order every list shows, and never depends on file names or the folder's listing
//! order. A new zone is added with data alone: write its file and give it a `difficulty` that
//! places it.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::kinds::ZoneEntry;

/// The folder of zone files, under the content folder.
pub const ZONE_FOLDER: &str = "zones";

/// Reads every zone file under `dir/zones/` and returns them in difficulty order. Every problem
/// (a file whose `id` is not its name, an id used twice, two zones of equal difficulty, a
/// difficulty that is not a positive number, no zone at all) is reported at once.
pub fn read_zone_entries(dir: &Path) -> Result<Vec<ZoneEntry>> {
    let folder = dir.join(ZONE_FOLDER);
    let mut files = BTreeMap::new();
    for item in std::fs::read_dir(&folder).with_context(|| format!("reading {}", folder.display()))? {
        let path = item.with_context(|| format!("reading {}", folder.display()))?.path();
        if path.extension().is_some_and(|e| e == "ron") && path.is_file() {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            let entry: ZoneEntry = ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
            files.insert(stem, entry);
        }
    }
    order_by_difficulty(files)
}

/// Checks the zone files (by file name, without `.ron`) and sorts them by difficulty.
fn order_by_difficulty(files: BTreeMap<String, ZoneEntry>) -> Result<Vec<ZoneEntry>> {
    let mut problems = Vec::new();
    if files.is_empty() {
        problems.push(format!("{ZONE_FOLDER}/ holds no zone file"));
    }
    for (name, entry) in &files {
        if entry.id != *name {
            problems.push(format!("{ZONE_FOLDER}/{name}.ron has id {:?}; a zone file's name must be its id", entry.id));
        }
        if !(entry.difficulty.is_finite() && entry.difficulty > 0.0) {
            problems.push(format!("zone {} has difficulty {}; it needs a number above 0", entry.id, entry.difficulty));
        }
    }
    let mut zones: Vec<ZoneEntry> = files.into_values().collect();
    zones.sort_by(|a, b| a.difficulty.total_cmp(&b.difficulty).then_with(|| a.id.cmp(&b.id)));
    for (i, z) in zones.iter().enumerate() {
        if zones[..i].iter().any(|x| x.id == z.id) {
            problems.push(format!("zone id {} is used by more than one zone file", z.id));
        }
    }
    for pair in zones.windows(2) {
        if pair[0].difficulty == pair[1].difficulty {
            problems.push(format!(
                "zones {} and {} both have difficulty {}; zones are ordered by difficulty, so each needs its own",
                pair[0].id, pair[1].id, pair[0].difficulty
            ));
        }
    }
    if !problems.is_empty() {
        bail!("zone files have {} problem(s):\n  {}", problems.len(), problems.join("\n  "));
    }
    Ok(zones)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn zone_text(id: &str, difficulty: f32) -> String {
        format!(
            "(id: \"{id}\", name: \"Z\", subtitle: \"\", description: \"\", map_id: 0, area_id: 0,
              spawn: (x: 0.0, y: 0.0, z: 0.0, facing: 0.0), arena_radius: 80.0, hour: 10,
              weather: (kind: None, intensity: 0.0), difficulty: {difficulty:?}, enemies: [], bosses: [])"
        )
    }

    /// A fresh content folder holding only the given zone files: (file name, id, difficulty).
    fn folder(test: &str, files: &[(&str, &str, f32)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("esurv-zone-files-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(ZONE_FOLDER)).unwrap();
        for (name, id, difficulty) in files {
            std::fs::write(dir.join(ZONE_FOLDER).join(format!("{name}.ron")), zone_text(id, *difficulty)).unwrap();
        }
        dir
    }

    fn load_err(test: &str, files: &[(&str, &str, f32)]) -> String {
        let dir = folder(test, files);
        let err = format!("{:#}", read_zone_entries(&dir).unwrap_err());
        let _ = std::fs::remove_dir_all(&dir);
        err
    }

    #[test]
    fn zones_come_in_difficulty_order_not_file_order() {
        let dir = folder("order", &[("a", "a", 3.0), ("b", "b", 1.0), ("c", "c", 1.5)]);
        std::fs::write(dir.join(ZONE_FOLDER).join("notes.txt"), "not a zone").unwrap();
        let ids: Vec<String> = read_zone_entries(&dir).unwrap().into_iter().map(|z| z.id).collect();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(ids, ["b", "c", "a"]);
    }

    #[test]
    fn a_zone_file_whose_id_is_not_its_name_fails() {
        let err = load_err("mismatch", &[("a", "a", 1.0), ("b", "c", 2.0)]);
        assert!(err.contains(r#"zones/b.ron has id "c"; a zone file's name must be its id"#), "{err}");
    }

    #[test]
    fn an_id_used_by_two_files_fails() {
        let err = load_err("duplicate", &[("a", "a", 1.0), ("b", "a", 2.0)]);
        assert!(err.contains("zone id a is used by more than one zone file"), "{err}");
    }

    #[test]
    fn two_zones_of_equal_difficulty_fail() {
        let err = load_err("tie", &[("a", "a", 2.0), ("b", "b", 2.0)]);
        assert!(err.contains("zones a and b both have difficulty 2; zones are ordered by difficulty"), "{err}");
    }

    #[test]
    fn a_difficulty_that_is_not_a_positive_number_fails() {
        let err = load_err("zero", &[("a", "a", 0.0)]);
        assert!(err.contains("zone a has difficulty 0; it needs a number above 0"), "{err}");
    }

    #[test]
    fn an_empty_zone_folder_fails() {
        let err = load_err("empty", &[]);
        assert!(err.contains("zones/ holds no zone file"), "{err}");
    }
}
