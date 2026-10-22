//! Help for tests, in this crate and the others: a copy of a content folder with zones added as
//! data alone, the way an author adds one (a file in `zones/`, placed by its difficulty).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// A zone added for a test: its id (and file name), its name and its difficulty.
#[derive(Debug, Clone, Copy)]
pub struct ExtraZone<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub difficulty: f32,
}

/// The text of a small, complete zone file: a roster of three (one of them a caster) and two
/// bosses, the second final, with abilities of every kind.
pub fn zone_file_text(zone: ExtraZone) -> String {
    let ExtraZone { id, name, difficulty } = zone;
    format!(
        r#"// A zone added with data alone.
(
    id: "{id}",
    name: "{name}",
    subtitle: "A Test of Data",
    description: "A zone written for a test.",
    map_id: 0,
    area_id: 12,
    spawn: (x: -9396.0, y: -633.0, z: 68.0, facing: 1.6),
    arena_radius: 80.0,
    hour: 12,
    weather: (kind: None, intensity: 0.0),
    difficulty: {difficulty:?},
    enemies: [
        (name: "Test Wolf", display_id: 165, scale: 0.7, creature_type: Beast,
         kind: FastFodder, toughness: 0.25, appears_at_minute: 0.0),
        (name: "Test Kobold", display_id: 26, scale: 0.9, creature_type: Humanoid,
         kind: Fodder, toughness: 0.5, appears_at_minute: 0.0),
        (name: "Test Caster", display_id: 163, scale: 1.0, creature_type: Humanoid,
         kind: Ranged, toughness: 0.5, appears_at_minute: 0.5, look_spell: Some(133)),
    ],
    bosses: [
        (name: "Test Brute", display_id: 384, scale: 3.0, creature_type: Humanoid,
         spawn_minute: 3.0, yell: "Test!", abilities: [Charge(look_spell: 100), Summon()],
         is_final: false),
        (name: "Test Lord", display_id: 2029, scale: 6.0, creature_type: Humanoid,
         spawn_minute: 6.0, yell: "The test ends here!",
         abilities: [Stomp(look_spell: 339), Volley(look_spell: 686),
                     GroundBurst(telegraph_spell: 5740, impact_spell: 13812)],
         is_final: true),
    ],
)
"#
    )
}

/// Copies the content folder `from` to a fresh folder under the system's temp folder (named after
/// `test`, so tests running at once keep apart) and writes each of `zones` into its `zones/`.
/// The caller removes the folder when done.
pub fn content_folder_with_zones(from: &Path, test: &str, zones: &[ExtraZone]) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("esurv-content-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_folder(from, &dir)?;
    for z in zones {
        let path = dir.join(crate::ZONE_FOLDER).join(format!("{}.ron", z.id));
        std::fs::write(&path, zone_file_text(*z)).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(dir)
}

/// The text of a small zone file with a roster for each faction besides its own: Alliance
/// heroes fight "Horde Grunt"s and the "Horde Warlord", Horde heroes "Alliance Footman"s and the
/// "Alliance Marshal"; each roster's enemy looks like a warrior player of rank 3.
pub fn faction_zone_file_text(zone: ExtraZone) -> String {
    let text = zone_file_text(zone);
    let end = text.rfind(')').unwrap_or(text.len());
    let roster = |enemy: &str, boss: &str| {
        format!(
            r#"(
            enemies: [(name: "{enemy}", display_id: 26, scale: 1.0, creature_type: Humanoid,
                       kind: Fodder, toughness: 0.5, appears_at_minute: 0.0,
                       looks_like: Some((class: "warrior")), rank: Some(3))],
            bosses: [(name: "{boss}", display_id: 2029, scale: 2.0, creature_type: Humanoid,
                      spawn_minute: 6.0, yell: "For the test!", abilities: [Charge(look_spell: 100)],
                      is_final: true)],
        )"#
        )
    };
    format!(
        "{}    by_faction: Some((\n        vs_alliance: {},\n        vs_horde: {},\n    )),\n{}",
        &text[..end],
        roster("Horde Grunt", "Horde Warlord"),
        roster("Alliance Footman", "Alliance Marshal"),
        &text[end..]
    )
}

/// Copies the content folder `from` as [`content_folder_with_zones`] does, with `zone` written
/// as [`faction_zone_file_text`].
pub fn content_folder_with_faction_zone(from: &Path, test: &str, zone: ExtraZone) -> Result<PathBuf> {
    let dir = content_folder_with_zones(from, test, &[zone])?;
    let path = dir.join(crate::ZONE_FOLDER).join(format!("{}.ron", zone.id));
    std::fs::write(&path, faction_zone_file_text(zone)).with_context(|| format!("writing {}", path.display()))?;
    Ok(dir)
}

fn copy_folder(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to).with_context(|| format!("making {}", to.display()))?;
    for item in std::fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let path = item?.path();
        let target = to.join(path.file_name().unwrap_or_default());
        if path.is_dir() {
            copy_folder(&path, &target)?;
        } else {
            std::fs::copy(&path, &target).with_context(|| format!("copying {}", path.display()))?;
        }
    }
    Ok(())
}

/// Sets the warrior's `base_health` in the content folder `dir` (the first in `classes.ron`): a
/// number edit a reload must pick up.
pub fn set_warrior_base_health(dir: &Path, health: f32) -> Result<()> {
    let path = dir.join("classes.ron");
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let at = text.find("base_health:").context("classes.ron has no base_health")?;
    let end = at + text[at..].find(',').context("base_health has no end")?;
    let text = format!("{}base_health: {health:?}{}", &text[..at], &text[end..]);
    std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
}

/// Stand-in client tables for the content folder `dir`: every pointer its `.ron` files write
/// (`Spell(133)`, `{Area(12)}` …) named `Client Spell 133`, `Client Area 12` …, so a test can tell
/// a name read from the tables from a placeholder without the client's own text.
pub fn stand_in_client_names(dir: &Path) -> Result<crate::ClientNames> {
    let mut tables: [HashMap<u32, String>; 7] = Default::default();
    let mut files = Vec::new();
    ron_files(dir, &mut files)?;
    for path in files {
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        for (table, kind) in ["Spell", "Area", "Class", "Race", "TalentTab", "AllianceRank", "HordeRank"].into_iter().enumerate() {
            for (at, _) in text.match_indices(&format!("{kind}(")) {
                let digits: String = text[at + kind.len() + 1..].chars().take_while(char::is_ascii_digit).collect();
                if let Ok(id) = digits.parse::<u32>() {
                    tables[table].insert(id, format!("Client {kind} {id}"));
                }
            }
        }
    }
    // a zone's ranked enemies name their rank by number (`rank: Some(5)`): every rank is there
    for (table, kind) in [(5, "AllianceRank"), (6, "HordeRank")] {
        for rank in 1..=14 {
            tables[table].entry(rank).or_insert_with(|| format!("Client {kind} {rank}"));
        }
    }
    let [spells, areas, classes, races, talent_tabs, alliance_ranks, horde_ranks] = tables;
    Ok(crate::ClientNames::from_tables(spells, areas, classes, races)
        .with_talent_tabs(talent_tabs)
        .with_pvp_ranks(alliance_ranks, horde_ranks))
}

fn ron_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for item in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = item?.path();
        if path.is_dir() {
            ron_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "ron") {
            out.push(path);
        }
    }
    Ok(())
}
