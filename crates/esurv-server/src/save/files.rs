//! The save's export and import (design step 6, the Options panel).
//! The game has no file dialog, so saves go to and come from one
//! fixed folder, `exports` beside the save (`saves/exports` in benilla's state folder by default):
//!
//! * an export writes the whole save as it stands to a new file there, named after the save and
//!   the moment (`heroes-2026-10-13-142530.ron`, UTC);
//! * the folder's saves are listed newest first, each read and checked, with how many heroes it
//!   holds or that it is no save this game can take;
//! * an import reads one of them, checks it as the list does, keeps the save it replaces in the
//!   same folder (`heroes-before-import-<moment>.ron`, so an import can be undone by importing
//!   that), and only then replaces the save and writes it.
//!
//! A save is checked against the content and the save's rules: every hero of a class and a race the
//! content has, a gender of 0 or 1, a name the stock rules allow, no two heroes with one guid or
//! one name, at most nine heroes of a faction and 18 in all. A next guid at or under a hero's guid
//! is raised past it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use esurv_content::{Content, Faction};

use super::{check_name, SaveFile, Saves, MAX_HEROES, MAX_HEROES_PER_FACTION};

/// The folder's name beside the save.
pub const EXPORTS_FOLDER: &str = "exports";
/// What a backup's name starts with after the save's own name.
const BEFORE_IMPORT: &str = "before-import";
/// The most saves the folder's list holds (the newest).
pub const LISTED: usize = 50;

/// Why a save file cannot be taken, in words for the screens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The save lives only in memory: there is no folder.
    NoFolder,
    /// No file of that name in the folder, or a name that is not a plain file name.
    NotThere,
    /// It is not a save of ours.
    Unreadable(String),
    /// It reads, but breaks the save's rules.
    Broken(String),
    /// The file system said no.
    Io(String),
    /// A hero is in a run: the save is not replaced under it.
    InARun,
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::NoFolder => write!(f, "this save is kept in memory only"),
            ImportError::NotThere => write!(f, "no such save in the folder"),
            ImportError::Unreadable(e) => write!(f, "not a save: {e}"),
            ImportError::Broken(e) => write!(f, "{e}"),
            ImportError::Io(e) => write!(f, "{e}"),
            ImportError::InARun => write!(f, "a hero is in a run"),
        }
    }
}

/// `when` as `YYYY-MM-DD-HHMMSS`, UTC.
pub fn moment(when: SystemTime) -> String {
    let secs = when.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_date(days as i64);
    format!(
        "{y:04}-{m:02}-{d:02}-{:02}{:02}{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The calendar date `days` after 1970-01-01 (the proleptic Gregorian calendar).
fn civil_date(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Checks a save against the content and the save's rules; how many heroes it holds. A next guid
/// at or under a hero's guid is raised past it.
pub(super) fn check(content: &Content, file: &mut SaveFile) -> Result<u32, String> {
    if file.heroes.len() > MAX_HEROES {
        return Err(format!("it holds more than {MAX_HEROES} heroes"));
    }
    let mut guids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let (mut alliance, mut horde) = (0, 0);
    for h in &file.heroes {
        if content.class(&h.class).is_none() {
            return Err(format!("{} is of a class the game does not have", h.name));
        }
        let Some(race) = content.race(&h.race) else {
            return Err(format!("{} is of a race the game does not have", h.name));
        };
        match race.faction {
            Faction::Alliance => alliance += 1,
            Faction::Horde => horde += 1,
        }
        if h.gender > 1 {
            return Err(format!("{} has no gender the game knows", h.name));
        }
        if check_name(&h.name).ok().as_deref() != Some(h.name.as_str()) {
            return Err(format!("{:?} is not a name a hero may have", h.name));
        }
        if !guids.insert(h.guid) {
            return Err(format!("two heroes share the number {}", h.guid));
        }
        if !names.insert(h.name.to_lowercase()) {
            return Err(format!("two heroes are called {}", h.name));
        }
    }
    if alliance > MAX_HEROES_PER_FACTION || horde > MAX_HEROES_PER_FACTION {
        return Err(format!(
            "a camp holds more than {MAX_HEROES_PER_FACTION} heroes"
        ));
    }
    if let Some(top) = guids.last() {
        file.next_guid = file.next_guid.max(top + 1);
    }
    Ok(file.heroes.len() as u32)
}

/// Reads and checks the save in `text`.
fn read(content: &Content, text: &str) -> Result<(SaveFile, u32), ImportError> {
    let mut file =
        SaveFile::parse(text).map_err(|e| ImportError::Unreadable(e.to_string()))?;
    let heroes = check(content, &mut file).map_err(ImportError::Broken)?;
    Ok((file, heroes))
}

/// Whether `name` is a plain `.ron` file name, with no way out of its folder.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['/', '\\', ':'])
        && name != "."
        && name != ".."
        && name.to_ascii_lowercase().ends_with(".ron")
}

/// `<stem>-<moment>.ron` (or `<stem>-<what>-<moment>.ron`) in `folder`, a number added when the
/// name is taken.
fn fresh_name(folder: &Path, stem: &str, what: Option<&str>, when: SystemTime) -> String {
    let base = match what {
        Some(w) => format!("{stem}-{w}-{}", moment(when)),
        None => format!("{stem}-{}", moment(when)),
    };
    let mut name = format!("{base}.ron");
    let mut n = 2;
    while folder.join(&name).exists() {
        name = format!("{base}-{n}.ron");
        n += 1;
    }
    name
}

impl Saves {
    /// The folder saves are exported to and imported from; `None` for a save in memory only.
    pub fn exports_folder(&self) -> Option<PathBuf> {
        Some(self.path.as_ref()?.parent()?.join(EXPORTS_FOLDER))
    }

    /// The save's own name without its extension (`heroes`), which exports are named after.
    fn stem(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map_or_else(|| "heroes".into(), |s| s.to_string_lossy().into_owned())
    }

    /// The saves in the folder, newest first (at most [`LISTED`]): each name, and how many heroes
    /// it holds when it is a save this game can take.
    pub fn save_files(&self, content: &Content) -> Vec<(String, Option<u32>)> {
        let Some(folder) = self.exports_folder() else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(&folder) else {
            return Vec::new();
        };
        let mut found: Vec<(SystemTime, String)> = entries
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                plain_name(&name).then_some(name).map(|n| {
                    let when = e
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(UNIX_EPOCH);
                    (when, n)
                })
            })
            .collect();
        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
        found.truncate(LISTED);
        found
            .into_iter()
            .map(|(_, name)| {
                let heroes = std::fs::read_to_string(folder.join(&name))
                    .ok()
                    .and_then(|t| read(content, &t).ok())
                    .map(|(_, n)| n);
                (name, heroes)
            })
            .collect()
    }

    /// Writes the whole save as it stands to a new file in the folder; its name.
    pub fn export(&self) -> Result<String, ImportError> {
        self.export_at(SystemTime::now())
    }

    fn export_at(&self, when: SystemTime) -> Result<String, ImportError> {
        let folder = self.exports_folder().ok_or(ImportError::NoFolder)?;
        std::fs::create_dir_all(&folder).map_err(|e| ImportError::Io(e.to_string()))?;
        let file = self.file.lock().unwrap();
        let name = fresh_name(&folder, &self.stem(), None, when);
        let text = ron::ser::to_string_pretty(&*file, ron::ser::PrettyConfig::default())
            .map_err(|e| ImportError::Io(e.to_string()))?;
        std::fs::write(folder.join(&name), text).map_err(|e| ImportError::Io(e.to_string()))?;
        Ok(name)
    }

    /// Replaces the save with the file `name` in the folder, once it is read and checked; the
    /// save it replaces is kept in the folder first. The backup's name.
    pub fn import(&self, content: &Content, name: &str) -> Result<String, ImportError> {
        self.import_at(content, name, SystemTime::now())
    }

    fn import_at(
        &self,
        content: &Content,
        name: &str,
        when: SystemTime,
    ) -> Result<String, ImportError> {
        if self.any_in_world() {
            return Err(ImportError::InARun);
        }
        let folder = self.exports_folder().ok_or(ImportError::NoFolder)?;
        if !plain_name(name) {
            return Err(ImportError::NotThere);
        }
        let text = std::fs::read_to_string(folder.join(name)).map_err(|_| ImportError::NotThere)?;
        let (incoming, _) = read(content, &text)?;
        let mut file = self.file.lock().unwrap();
        // the save it replaces, kept where it can be imported back
        let backup = fresh_name(&folder, &self.stem(), Some(BEFORE_IMPORT), when);
        let kept = ron::ser::to_string_pretty(&*file, ron::ser::PrettyConfig::default())
            .map_err(|e| ImportError::Io(e.to_string()))?;
        std::fs::write(folder.join(&backup), kept).map_err(|e| ImportError::Io(e.to_string()))?;
        *file = incoming;
        self.write(&file)
            .map_err(|e| ImportError::Io(format!("{e:#}")))?;
        Ok(backup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    fn folder(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "esurv-save-files-{test}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_moment_reads_as_a_utc_date_and_time() {
        let at = |s| UNIX_EPOCH + Duration::from_secs(s);
        assert_eq!(moment(at(0)), "1970-01-01-000000");
        // 2026-10-13 14:25:30 UTC
        assert_eq!(moment(at(1_791_901_530)), "2026-10-13-142530");
        // a leap day
        assert_eq!(moment(at(951_782_400)), "2000-02-29-000000");
    }

    #[test]
    fn an_export_imports_back_and_the_replaced_save_is_kept() {
        let content = content();
        let dir = folder("round");
        let saves = Saves::open(&dir.join("heroes.ron")).unwrap();
        saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        saves.create("Bors", "warrior", "orc", 0, [0; 5]).unwrap();
        let when = UNIX_EPOCH + Duration::from_secs(1_791_901_530);
        let exported = saves.export_at(when).unwrap();
        assert_eq!(exported, "heroes-2026-10-13-142530.ron");
        assert_eq!(
            saves.export_at(when).unwrap(),
            "heroes-2026-10-13-142530-2.ron",
            "a name taken gets a number"
        );
        assert_eq!(saves.exports_folder().unwrap(), dir.join(EXPORTS_FOLDER));
        // the save changes, then the export comes back
        let ana = saves.heroes()[0].guid;
        assert!(saves.delete(ana));
        saves.create("Cael", "rogue", "human", 0, [0; 5]).unwrap();
        let backup = saves
            .import_at(&content, &exported, when + Duration::from_secs(60))
            .unwrap();
        assert_eq!(backup, "heroes-before-import-2026-10-13-142630.ron");
        let names = |s: &Saves| -> Vec<String> { s.heroes().into_iter().map(|h| h.name).collect() };
        assert_eq!(names(&saves), ["Ana", "Bors"]);
        assert_eq!(
            names(&Saves::open(&dir.join("heroes.ron")).unwrap()),
            ["Ana", "Bors"],
            "written"
        );
        // a new hero never takes a guid the imported save has
        let dana = saves.create("Dana", "mage", "human", 1, [0; 5]).unwrap();
        assert!(
            saves
                .heroes()
                .iter()
                .filter(|h| h.guid == dana.guid)
                .count()
                == 1
        );
        // the list: newest first, each with its heroes
        let files = saves.save_files(&content);
        assert!(files.contains(&(backup.clone(), Some(2))), "{files:?}");
        assert!(files.contains(&(exported.clone(), Some(2))), "{files:?}");
        // the backup undoes the import
        saves.import(&content, &backup).unwrap();
        assert_eq!(names(&saves), ["Bors", "Cael"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_that_is_no_save_or_breaks_the_rules_replaces_nothing() {
        let content = content();
        let dir = folder("refused");
        let saves = Saves::open(&dir.join("heroes.ron")).unwrap();
        saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        let exports = saves.exports_folder().unwrap();
        std::fs::create_dir_all(&exports).unwrap();
        std::fs::write(exports.join("junk.ron"), "not a save").unwrap();
        let twins =
            "(heroes: [(guid: 1, name: \"Ana\", class: \"mage\", race: \"human\", gender: 0, \
                     look: (0, 0, 0, 0, 0)), (guid: 2, name: \"ana\", class: \"mage\", race: \
                     \"human\", gender: 0, look: (0, 0, 0, 0, 0))], next_guid: 3)";
        std::fs::write(exports.join("twins.ron"), twins).unwrap();
        let ogre = twins.replace("\"mage\"", "\"ogre\"");
        std::fs::write(exports.join("ogre.ron"), ogre).unwrap();
        for (name, refused) in [
            ("junk.ron", "unreadable"),
            ("twins.ron", "broken"),
            ("ogre.ron", "broken"),
            ("missing.ron", "not there"),
            ("../heroes.ron", "not there"),
        ] {
            let err = saves.import(&content, name).unwrap_err();
            let kind = match err {
                ImportError::Unreadable(_) => "unreadable",
                ImportError::Broken(_) => "broken",
                ImportError::NotThere => "not there",
                _ => "other",
            };
            assert_eq!(kind, refused, "{name}: {err}");
        }
        assert_eq!(saves.heroes().len(), 1, "nothing replaced");
        assert!(
            std::fs::read_dir(&exports).unwrap().all(|e| !e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(BEFORE_IMPORT)),
            "no backup for a refused import"
        );
        let files = saves.save_files(&content);
        assert!(files.contains(&("junk.ron".into(), None)));
        assert!(files.contains(&("twins.ron".into(), None)));
        // a save in memory has no folder
        assert_eq!(Saves::in_memory().export(), Err(ImportError::NoFolder));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
