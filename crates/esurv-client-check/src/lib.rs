//! Whether a folder holds a client Eternal Survivors plays on: World of Warcraft 1.12.1, build
//! 5875, enUS. The setup asks it of the folder it was dropped into, and the game asks it of the
//! folder its data comes from at start, so a wrong folder is told in words, not by a black screen.
//!
//! What it looks at, all read-only:
//!
//! * `WoW.exe`'s file version (the `VS_FIXEDFILEINFO` record of its version resource) is 1.12.1.5875.
//! * `Data\` holds the ten base archives and `patch.MPQ` and `patch-2.MPQ`, the 1.12.1 patch chain.
//! * The locale is enUS: `WTF\Config.wtf` sets no other `locale` (1.12 has no locale folder; a
//!   client that sets none is enUS), and `Data\` has no locale folder of another language (a later
//!   client's layout).

use std::fmt;
use std::path::{Path, PathBuf};

/// The client the game plays on, as `WoW.exe`'s file version spells it.
pub const CLIENT_VERSION: FileVersion = FileVersion(1, 12, 1, 5875);

/// The archives a 1.12.1 `Data` folder holds: the ten base archives and the two patches.
pub const REQUIRED_ARCHIVES: &[&str] = &[
    "dbc.MPQ",
    "speech.MPQ",
    "fonts.MPQ",
    "interface.MPQ",
    "misc.MPQ",
    "sound.MPQ",
    "wmo.MPQ",
    "terrain.MPQ",
    "texture.MPQ",
    "model.MPQ",
    "patch.MPQ",
    "patch-2.MPQ",
];

/// The one locale the game plays on.
pub const LOCALE: &str = "enUS";

/// A file version, `major.minor.patch.build`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileVersion(pub u16, pub u16, pub u16, pub u16);

impl fmt::Display for FileVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{} (build {})", self.0, self.1, self.2, self.3)
    }
}

/// One thing wrong with a client folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// The folder does not exist.
    NoFolder(PathBuf),
    /// No `WoW.exe` in the folder.
    NoExe,
    /// `WoW.exe` has no file version to read.
    UnreadableVersion,
    /// `WoW.exe` is another version.
    WrongVersion(FileVersion),
    /// No `Data` folder.
    NoData,
    /// An archive of the 1.12.1 chain is missing from `Data`.
    MissingArchive(&'static str),
    /// The client is set to another language.
    WrongLocale(String),
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::NoFolder(p) => write!(f, "the folder {} does not exist", p.display()),
            Problem::NoExe => write!(f, "there is no WoW.exe in it"),
            Problem::UnreadableVersion => write!(f, "WoW.exe has no version to read"),
            Problem::WrongVersion(v) => {
                write!(f, "WoW.exe is version {v}, not {CLIENT_VERSION}")
            }
            Problem::NoData => write!(f, "there is no Data folder in it"),
            Problem::MissingArchive(a) => write!(f, "Data\\{a} is missing"),
            Problem::WrongLocale(l) => {
                write!(f, "the client is in the {l} language, not English ({LOCALE})")
            }
        }
    }
}

/// What the check found in one folder.
#[derive(Clone, Debug)]
pub struct Report {
    /// The folder checked, the one holding `WoW.exe` and `Data`.
    pub folder: PathBuf,
    /// Everything wrong; empty when the client is usable.
    pub problems: Vec<Problem>,
}

impl Report {
    /// The client can be played on.
    pub fn is_usable(&self) -> bool {
        self.problems.is_empty()
    }

    /// The report in words for the player: one line per problem, under the folder's name.
    pub fn message(&self) -> String {
        if self.is_usable() {
            return format!(
                "{} holds World of Warcraft {CLIENT_VERSION}, {LOCALE}.",
                self.folder.display()
            );
        }
        let mut out = format!(
            "{} is not a World of Warcraft {CLIENT_VERSION} {LOCALE} folder:\n",
            self.folder.display()
        );
        for p in &self.problems {
            out.push_str("  - ");
            out.push_str(&p.to_string());
            out.push('\n');
        }
        out
    }
}

/// Checks a client folder: the one holding `WoW.exe`, `Data\` and `WTF\`.
pub fn check_folder(folder: &Path) -> Report {
    let mut problems = Vec::new();
    if !folder.is_dir() {
        problems.push(Problem::NoFolder(folder.to_path_buf()));
        return Report {
            folder: folder.to_path_buf(),
            problems,
        };
    }
    match find_ci(folder, "WoW.exe").filter(|p| p.is_file()) {
        None => problems.push(Problem::NoExe),
        Some(exe) => match std::fs::read(&exe).ok().and_then(|b| file_version(&b)) {
            None => problems.push(Problem::UnreadableVersion),
            Some(v) if v != CLIENT_VERSION => problems.push(Problem::WrongVersion(v)),
            Some(_) => {}
        },
    }
    match find_ci(folder, "Data").filter(|p| p.is_dir()) {
        None => problems.push(Problem::NoData),
        Some(data) => problems.extend(check_data(&data)),
    }
    if let Some(locale) = configured_locale(folder) {
        if !locale.eq_ignore_ascii_case(LOCALE) {
            problems.push(Problem::WrongLocale(locale));
        }
    }
    Report {
        folder: folder.to_path_buf(),
        problems,
    }
}

/// Checks a `Data` folder alone (a data folder named by hand, with no `WoW.exe` beside it): the
/// archives of the chain and no locale folder of another language.
pub fn check_data(data: &Path) -> Vec<Problem> {
    let names = dir_names(data);
    let mut problems: Vec<Problem> = REQUIRED_ARCHIVES
        .iter()
        .filter(|a| {
            !names
                .iter()
                .any(|(n, is_file)| *is_file && n.eq_ignore_ascii_case(a))
        })
        .map(|a| Problem::MissingArchive(a))
        .collect();
    for (name, is_file) in &names {
        if !is_file && is_locale_code(name) && !name.eq_ignore_ascii_case(LOCALE) {
            problems.push(Problem::WrongLocale(name.clone()));
        }
    }
    problems
}

/// The `locale` the client's `WTF\Config.wtf` sets (`SET locale "deDE"`), if any.
pub fn configured_locale(folder: &Path) -> Option<String> {
    let wtf = find_ci(folder, "WTF")?;
    let config = std::fs::read(find_ci(&wtf, "Config.wtf")?).ok()?;
    let text = String::from_utf8_lossy(&config);
    text.lines().find_map(|line| {
        let mut words = line.split_whitespace();
        let set = words.next()?;
        let key = words.next()?;
        if !set.eq_ignore_ascii_case("SET") || !key.eq_ignore_ascii_case("locale") {
            return None;
        }
        let value = words.next()?.trim_matches('"');
        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// The file version in an executable's version resource: the `VS_FIXEDFILEINFO` record found by
/// its signature, `0xFEEF04BD`, followed by the structure version and the version's two halves.
pub fn file_version(exe: &[u8]) -> Option<FileVersion> {
    const SIGNATURE: [u8; 4] = 0xFEEF_04BDu32.to_le_bytes();
    let at = exe.windows(4).position(|w| w == SIGNATURE)?;
    let dword = |off: usize| -> Option<u32> {
        let b = exe.get(at + off..at + off + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let (ms, ls) = (dword(8)?, dword(12)?);
    Some(FileVersion(
        (ms >> 16) as u16,
        ms as u16,
        (ls >> 16) as u16,
        ls as u16,
    ))
}

/// A four-letter locale code, `xxYY`.
fn is_locale_code(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() == 4
        && b[0].is_ascii_lowercase()
        && b[1].is_ascii_lowercase()
        && b[2].is_ascii_uppercase()
        && b[3].is_ascii_uppercase()
}

/// The entries of a folder, by name, each with whether it is a file (links followed).
fn dir_names(dir: &Path) -> Vec<(String, bool)> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.filter_map(|e| {
        let e = e.ok()?;
        let name = e.file_name().into_string().ok()?;
        Some((name, e.path().is_file()))
    })
    .collect()
}

/// `dir\name`, matched without regard to case (a client copied from elsewhere may spell it so).
fn find_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if exact.exists() {
        return Some(exact);
    }
    dir_names(dir)
        .into_iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(n, _)| dir.join(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An executable's bytes holding a version record of `v`, with filler around it.
    fn exe_with(v: FileVersion) -> Vec<u8> {
        let mut b = vec![0x4d, 0x5a, 0, 0, 7, 7, 7];
        b.extend_from_slice(&0xFEEF_04BDu32.to_le_bytes());
        b.extend_from_slice(&0x0001_0000u32.to_le_bytes());
        b.extend_from_slice(&((u32::from(v.0) << 16) | u32::from(v.1)).to_le_bytes());
        b.extend_from_slice(&((u32::from(v.2) << 16) | u32::from(v.3)).to_le_bytes());
        b.extend_from_slice(&[0; 32]);
        b
    }

    /// A scratch client folder: `WoW.exe` of version `v` and a `Data` holding `archives`.
    fn client(tag: &str, v: FileVersion, archives: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("esurv-check-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Data")).unwrap();
        std::fs::write(dir.join("WoW.exe"), exe_with(v)).unwrap();
        for a in archives {
            std::fs::write(dir.join("Data").join(a), b"").unwrap();
        }
        dir
    }

    #[test]
    fn the_version_record_reads_back() {
        assert_eq!(file_version(&exe_with(CLIENT_VERSION)), Some(CLIENT_VERSION));
        assert_eq!(file_version(b"no record here"), None);
    }

    #[test]
    fn a_whole_1_12_1_client_is_usable() {
        let dir = client("whole", CLIENT_VERSION, REQUIRED_ARCHIVES);
        let report = check_folder(&dir);
        assert!(report.is_usable(), "{}", report.message());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn another_version_and_a_missing_patch_are_both_told() {
        let mut archives = REQUIRED_ARCHIVES.to_vec();
        archives.retain(|a| *a != "patch-2.MPQ");
        let dir = client("wrong", FileVersion(2, 4, 3, 8606), &archives);
        let report = check_folder(&dir);
        assert_eq!(
            report.problems,
            vec![
                Problem::WrongVersion(FileVersion(2, 4, 3, 8606)),
                Problem::MissingArchive("patch-2.MPQ"),
            ]
        );
        assert!(report.message().contains("patch-2.MPQ is missing"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn another_language_is_told_from_the_config_or_a_locale_folder() {
        let dir = client("locale", CLIENT_VERSION, REQUIRED_ARCHIVES);
        std::fs::create_dir_all(dir.join("WTF")).unwrap();
        std::fs::write(dir.join("WTF/Config.wtf"), "SET gxApi \"d3d\"\nSET locale \"deDE\"\n")
            .unwrap();
        assert_eq!(
            check_folder(&dir).problems,
            vec![Problem::WrongLocale("deDE".into())]
        );
        std::fs::write(dir.join("WTF/Config.wtf"), "SET locale \"enUS\"\n").unwrap();
        std::fs::create_dir_all(dir.join("Data/frFR")).unwrap();
        assert_eq!(
            check_folder(&dir).problems,
            vec![Problem::WrongLocale("frFR".into())]
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_empty_folder_says_what_is_missing() {
        let dir = std::env::temp_dir().join(format!("esurv-check-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(
            check_folder(&dir).problems,
            vec![Problem::NoExe, Problem::NoData]
        );
        std::fs::remove_dir_all(&dir).ok();
        assert!(matches!(
            check_folder(&dir).problems[..],
            [Problem::NoFolder(_)]
        ));
    }

    /// A real client, where there is one (`ESURV_CLIENT_DIR`), read-only.
    #[test]
    fn the_real_client_is_usable_when_named() {
        let Some(dir) = std::env::var_os("ESURV_CLIENT_DIR") else {
            return;
        };
        let report = check_folder(Path::new(&dir));
        assert!(report.is_usable(), "{}", report.message());
    }
}
