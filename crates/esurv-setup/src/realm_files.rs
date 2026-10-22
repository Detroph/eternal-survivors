//! The game's realm carries the game's name, so a renamed game renames its realm, and benilla keeps
//! a character's camera, chat windows, frame layout, key bindings, addon choices and saved
//! variables, and the realm's name cache, in files named after the realm (in the game folder's
//! `benilla-config\`). The game only ever offers one realm, so a file there named after any other
//! realm is the game's own under an earlier name: [`carry_realm_files`] moves each character's
//! newest such file to the current realm's name and deletes the other earlier ones, so a patched
//! game keeps its settings and nothing under an earlier name.
//!
//! The heroes' save itself (`saves\heroes.ron`) carries no realm or game name and needs nothing.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The game folder's settings folder (benilla's state folder beside the game's executable).
pub const STATE_DIR: &str = "benilla-config";
/// The state folders that hold one file per character, `<realm>-<character>.txt`.
const CHARACTER_FILE_DIRS: [&str; 5] = ["camera", "chat", "layout", "bindings", "addons"];
/// The state folder that holds one folder per character, `<realm>-<character>/`.
const CHARACTER_DIR_DIR: &str = "saved";
/// The realm's name cache, `cache/<realm>.tsv`.
const NAME_CACHE_DIR: &str = "cache";

/// What [`carry_realm_files`] did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Carried {
    /// Files moved to the current realm's name.
    pub moved: usize,
    /// Earlier files deleted: older ones, and ones the current realm already had.
    pub deleted: usize,
}

/// A name as benilla writes it into a state file's name (`benilla_app::local_state::file_token`):
/// every character but letters and digits as `_`.
pub fn file_token(name: &str) -> String {
    let token: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    if token.is_empty() {
        "unknown".into()
    } else {
        token
    }
}

/// Moves the files in `state_dir` named after an earlier realm to `realm`'s names: per character
/// the newest, when `realm` has none yet; every other earlier file is deleted. The name cache's
/// header names its realm, so the moved one names `realm`.
pub fn carry_realm_files(state_dir: &Path, realm: &str) -> io::Result<Carried> {
    let current = file_token(realm);
    let mut done = Carried::default();
    for kind in CHARACTER_FILE_DIRS.iter().chain([&CHARACTER_DIR_DIR]) {
        let dir = state_dir.join(kind);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        // each character's files under an earlier realm's name (a realm token has no `-`)
        let mut earlier: BTreeMap<String, Vec<(SystemTime, PathBuf)>> = BTreeMap::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let Some((token, character)) = name.to_str().and_then(|n| n.split_once('-')) else {
                continue;
            };
            if token != current && !character.is_empty() {
                let found = (modified(&entry.path()), entry.path());
                earlier.entry(character.to_owned()).or_default().push(found);
            }
        }
        for (character, mut files) in earlier {
            files.sort();
            let to = dir.join(format!("{current}-{character}"));
            if !to.exists() {
                let (_, newest) = files.pop().expect("a character listed has a file");
                fs::rename(&newest, &to)?;
                done.moved += 1;
            }
            for (_, file) in files {
                delete(&file)?;
                done.deleted += 1;
            }
        }
    }

    let cache = state_dir.join(NAME_CACHE_DIR);
    let Ok(entries) = fs::read_dir(&cache) else {
        return Ok(done);
    };
    let mut earlier: Vec<(SystemTime, PathBuf)> = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let other_realm = path.extension().is_some_and(|e| e == "tsv")
            && path.file_stem().is_some_and(|s| s != current.as_str());
        if other_realm {
            earlier.push((modified(&path), path));
        }
    }
    earlier.sort();
    let to = cache.join(format!("{current}.tsv"));
    if !to.exists() {
        if let Some((_, newest)) = earlier.pop() {
            fs::write(&to, rename_cache_header(&fs::read_to_string(&newest)?, realm))?;
            fs::remove_file(&newest)?;
            done.moved += 1;
        }
    }
    for (_, file) in earlier {
        fs::remove_file(&file)?;
        done.deleted += 1;
    }
    Ok(done)
}

/// When a file last changed; the epoch when the system can't tell.
fn modified(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn delete(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// The name cache's text with its header's realm (the fifth field of the first line) set to `realm`.
fn rename_cache_header(text: &str, realm: &str) -> String {
    let (header, rest) = text.split_once('\n').unwrap_or((text, ""));
    let mut fields: Vec<&str> = header.split('\t').collect();
    if fields.len() > 4 {
        fields[4] = realm;
    }
    let newline = if text.contains('\n') { "\n" } else { "" };
    format!("{}{newline}{rest}", fields.join("\t"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const NEW: &str = "Eternal Survivors";

    fn state_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("esurv-realm-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn write_at(path: &Path, text: &str, seconds: u64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + seconds);
        fs::File::options().write(true).open(path).unwrap().set_modified(at).unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut found: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        found.sort();
        found
    }

    #[test]
    fn the_realm_files_are_named_as_benilla_names_them() {
        assert_eq!(file_token(NEW), "Eternal_Survivors");
        assert_eq!(file_token("An - Earlier Name"), "An___Earlier_Name");
    }

    #[test]
    fn an_earlier_realms_files_move_and_nothing_earlier_stays() {
        let dir = state_dir("move");
        write_at(&dir.join("camera/Earlier_Name-Hero.txt"), "pose", 0);
        write_at(&dir.join("chat/Earlier_Name-Hero.txt"), "old chat", 0);
        write_at(&dir.join("chat/Eternal_Survivors-Hero.txt"), "new chat", 0);
        write_at(&dir.join("saved/Earlier_Name-Hero/Addon.lua"), "x = 1", 0);
        write_at(
            &dir.join("cache/Earlier_Name.tsv"),
            "benilla-namecache\t2\t5875\tenUS\tEarlier Name\nC\t1\tName\n",
            0,
        );

        assert_eq!(carry_realm_files(&dir, NEW).unwrap(), Carried { moved: 3, deleted: 1 });
        assert_eq!(fs::read_to_string(dir.join("camera/Eternal_Survivors-Hero.txt")).unwrap(), "pose");
        assert_eq!(
            fs::read_to_string(dir.join("chat/Eternal_Survivors-Hero.txt")).unwrap(),
            "new chat",
            "a file the current realm already has is kept"
        );
        assert_eq!(fs::read_to_string(dir.join("saved/Eternal_Survivors-Hero/Addon.lua")).unwrap(), "x = 1");
        assert_eq!(
            fs::read_to_string(dir.join("cache/Eternal_Survivors.tsv")).unwrap(),
            "benilla-namecache\t2\t5875\tenUS\tEternal Survivors\nC\t1\tName\n"
        );
        for kind in ["camera", "chat", "saved", "cache"] {
            assert!(names(&dir.join(kind)).iter().all(|n| n.starts_with("Eternal_Survivors")), "{kind}");
        }
        assert_eq!(carry_realm_files(&dir, NEW).unwrap(), Carried::default(), "nothing left the second time");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn of_two_earlier_realms_the_newest_file_wins() {
        let dir = state_dir("newest");
        write_at(&dir.join("layout/First-Hero.txt"), "first layout", 0);
        write_at(&dir.join("layout/Second-Hero.txt"), "second layout", 60);
        write_at(&dir.join("cache/First.tsv"), "benilla-namecache\t2\t5875\tenUS\tFirst\n", 60);
        write_at(&dir.join("cache/Second.tsv"), "benilla-namecache\t2\t5875\tenUS\tSecond\n", 0);

        assert_eq!(carry_realm_files(&dir, NEW).unwrap(), Carried { moved: 2, deleted: 2 });
        assert_eq!(fs::read_to_string(dir.join("layout/Eternal_Survivors-Hero.txt")).unwrap(), "second layout");
        assert_eq!(
            fs::read_to_string(dir.join("cache/Eternal_Survivors.tsv")).unwrap(),
            "benilla-namecache\t2\t5875\tenUS\tEternal Survivors\n",
            "the newest cache, under the current realm's name"
        );
        assert_eq!(names(&dir.join("layout")), ["Eternal_Survivors-Hero.txt"]);
        assert_eq!(names(&dir.join("cache")), ["Eternal_Survivors.tsv"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_state_folder_changes_nothing() {
        assert_eq!(carry_realm_files(&state_dir("none"), NEW).unwrap(), Carried::default());
    }
}
