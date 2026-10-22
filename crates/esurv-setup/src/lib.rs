//! The setup's two halves: the files it carries (a payload appended to its own executable, so the
//! setup is one file) and the install that writes them into `Eternal Survivors\` inside the WoW
//! folder.
//!
//! The payload, after the executable's own bytes: each file as a `u32` path length, the path
//! (UTF-8, `/` apart, relative to the game folder), a `u64` length and the bytes, stored as they
//! are (the release archive compresses them, and the leak scan reads them); then a trailer of
//! the payload's `u64` length, the `u32` file count and [`MAGIC`].

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

pub mod realm_files;

/// The trailer's last eight bytes.
pub const MAGIC: &[u8; 8] = b"ESRVPAY1";
/// The trailer's size: payload length, file count, magic.
const TRAILER: u64 = 8 + 4 + 8;

/// The game folder's name inside the WoW folder.
pub const GAME_FOLDER: &str = "Eternal Survivors";
/// The folders the game keeps the player's own files in: made empty, never written or removed.
pub const PLAYER_FOLDERS: &[&str] = &["saves", "logs"];
/// The list of what the last setup wrote, kept in the game folder, so an update removes the files
/// a newer version no longer has.
pub const MANIFEST: &str = "setup-files.txt";
/// The file the setup writes the version it carries into (`v0.2.0`): what the next setup reads
/// to tell an update from a first install.
pub const VERSION_FILE: &str = "VERSION.txt";
/// The game's executable in its folder: the file a running game holds.
pub const GAME_EXE: &str = "Eternal Survivors.exe";
/// The setup's own file name, as the release archive carries it.
pub const SETUP_EXE: &str = "Eternal Survivors Setup.exe";
/// The flag the game's updater starts the setup with: wait for the game to close
/// ([`wait_until_closed`], up to [`GAME_CLOSE_WAIT`]), then start it again once the update is in
/// ([`relaunch`]).
pub const RELAUNCH_FLAG: &str = "--relaunch";
/// How long `--relaunch` waits for the game to close before it gives up, changing nothing.
pub const GAME_CLOSE_WAIT: std::time::Duration = std::time::Duration::from_secs(60);

/// The setup's options, from its command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupArgs {
    /// `--wow <folder>`: the WoW folder to install into, instead of the setup's own.
    pub wow: Option<PathBuf>,
    /// Without `--no-pause`: wait for Enter before closing, and ask before going back to an
    /// older version.
    pub pause: bool,
    /// [`RELAUNCH_FLAG`]: wait for the game to close first, and start it again after.
    pub relaunch: bool,
}

/// Reads the setup's command line (its arguments after the program's name); `Err` with what to
/// say for an option it does not know.
pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<SetupArgs, String> {
    let mut out = SetupArgs {
        wow: None,
        pause: true,
        relaunch: false,
    };
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--wow" => {
                out.wow = Some(
                    args.next()
                        .map(PathBuf::from)
                        .ok_or("--wow needs the folder after it.")?,
                )
            }
            "--no-pause" => out.pause = false,
            RELAUNCH_FLAG => out.relaunch = true,
            _ => {
                return Err(format!(
                    "Unknown option {a}. Options: --wow <folder>, --no-pause, {RELAUNCH_FLAG}."
                ))
            }
        }
    }
    Ok(out)
}

/// One carried file.
pub struct Entry {
    /// Relative to the game folder, `/` apart.
    pub path: String,
    pub bytes: Vec<u8>,
}

/// Appends `entries` to `stub` (the setup executable's bytes) as a payload.
pub fn pack(stub: &[u8], entries: &[Entry], out: &mut impl Write) -> io::Result<()> {
    out.write_all(stub)?;
    let mut len = 0u64;
    for e in entries {
        check_path(&e.path)?;
        let path = e.path.as_bytes();
        out.write_all(&(path.len() as u32).to_le_bytes())?;
        out.write_all(path)?;
        out.write_all(&(e.bytes.len() as u64).to_le_bytes())?;
        out.write_all(&e.bytes)?;
        len += 4 + path.len() as u64 + 8 + e.bytes.len() as u64;
    }
    out.write_all(&len.to_le_bytes())?;
    out.write_all(&(entries.len() as u32).to_le_bytes())?;
    out.write_all(MAGIC)
}

/// The payload carried at the end of `file`, or `None` when it carries none.
pub fn unpack(file: &mut (impl Read + Seek)) -> io::Result<Option<Vec<Entry>>> {
    let end = file.seek(SeekFrom::End(0))?;
    if end < TRAILER {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(end - TRAILER))?;
    let mut trailer = [0u8; TRAILER as usize];
    file.read_exact(&mut trailer)?;
    if &trailer[12..] != MAGIC {
        return Ok(None);
    }
    let len = u64::from_le_bytes(trailer[..8].try_into().unwrap());
    let count = u32::from_le_bytes(trailer[8..12].try_into().unwrap());
    let start = (end - TRAILER)
        .checked_sub(len)
        .ok_or_else(|| bad("the payload is longer than the file"))?;
    file.seek(SeekFrom::Start(start))?;
    let mut entries = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut n4 = [0u8; 4];
        file.read_exact(&mut n4)?;
        let mut path = vec![0u8; u32::from_le_bytes(n4) as usize];
        file.read_exact(&mut path)?;
        let path = String::from_utf8(path).map_err(|_| bad("a path is not UTF-8"))?;
        check_path(&path)?;
        let mut n8 = [0u8; 8];
        file.read_exact(&mut n8)?;
        let size = u64::from_le_bytes(n8);
        if size > len {
            return Err(bad("a file is longer than the payload"));
        }
        let mut bytes = vec![0u8; size as usize];
        file.read_exact(&mut bytes)?;
        entries.push(Entry { path, bytes });
    }
    Ok(Some(entries))
}

/// A carried path stays inside the game folder and out of the player's own folders.
fn check_path(path: &str) -> io::Result<()> {
    let p = Path::new(path);
    if path.is_empty() || !p.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(bad(&format!("the path {path:?} leaves the game folder")));
    }
    let first = path.split('/').next().unwrap_or_default();
    if PLAYER_FOLDERS
        .iter()
        .any(|f| first.eq_ignore_ascii_case(f))
        || first.eq_ignore_ascii_case(MANIFEST)
    {
        return Err(bad(&format!("the path {path:?} is the player's own")));
    }
    Ok(())
}

fn bad(why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, why.to_owned())
}

/// A version as the setup writes it, `v0.2.0` (the `v` optional): major, minor, patch.
pub fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let t = text.trim();
    let t = t.strip_prefix(['v', 'V']).unwrap_or(t);
    let mut parts = t.split('.').map(|p| p.parse::<u32>().ok());
    let v = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(v)
}

/// The version an install carries: its `VERSION.txt`, trimmed (empty when it has none).
pub fn carried_version(entries: &[Entry]) -> String {
    entries
        .iter()
        .find(|e| e.path == VERSION_FILE)
        .map(|e| String::from_utf8_lossy(&e.bytes).trim().to_owned())
        .unwrap_or_default()
}

/// What is in the game folder before the setup runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// No game there yet.
    Nothing,
    /// A game whose version file says this (as written, trimmed).
    Version(String),
    /// A game (its executable or the setup's list) with no version file that reads as one.
    Unknown,
}

/// What an earlier setup left in `game`.
pub fn installed(game: &Path) -> Found {
    if let Ok(text) = std::fs::read_to_string(game.join(VERSION_FILE)) {
        if parse_version(&text).is_some() {
            return Found::Version(text.trim().to_owned());
        }
    }
    if game.join(MANIFEST).is_file() || game.join(GAME_EXE).is_file() {
        Found::Unknown
    } else {
        Found::Nothing
    }
}

/// What this setup is about to do to the game folder (the setup is
/// also the patcher, and says which before it changes anything).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// No game there: a first install.
    Install { to: String },
    /// An older version (or one whose version is not known) is replaced.
    Update { from: Option<String>, to: String },
    /// The same version is written again (it repairs a damaged install).
    Reinstall { to: String },
    /// A newer version is there: replaced only when the player says so.
    Downgrade { from: String, to: String },
}

impl Plan {
    /// The plan for carrying version `to` into a folder where `found` is.
    pub fn new(found: Found, to: &str) -> Self {
        let to = to.to_owned();
        match found {
            Found::Nothing => Plan::Install { to },
            Found::Unknown => Plan::Update { from: None, to },
            Found::Version(from) => match (parse_version(&from), parse_version(&to)) {
                (Some(a), Some(b)) if a == b => Plan::Reinstall { to },
                (Some(a), Some(b)) if a > b => Plan::Downgrade { from, to },
                _ => Plan::Update { from: Some(from), to },
            },
        }
    }

    /// The line the setup says before it starts.
    pub fn message(&self) -> String {
        match self {
            Plan::Install { to } => format!("Installing Eternal Survivors {to}"),
            Plan::Update { from: Some(from), to } => {
                format!("Updating Eternal Survivors from {from} to {to}")
            }
            Plan::Update { from: None, to } => {
                format!("Updating Eternal Survivors from an earlier version to {to}")
            }
            Plan::Reinstall { to } => format!("Reinstalling Eternal Survivors {to}"),
            Plan::Downgrade { from, to } => {
                format!("Eternal Survivors {from} is installed, newer than this setup's {to}")
            }
        }
    }
}

/// `Err` with what to say when the game is running from `game`: Windows holds a running
/// executable, so it cannot be opened for writing. Checked before anything changes.
pub fn check_not_running(game: &Path, entries: &[Entry]) -> Result<(), String> {
    let exes = entries
        .iter()
        .filter(|e| e.path.to_ascii_lowercase().ends_with(".exe"))
        .map(|e| e.path.as_str());
    for path in exes.chain([GAME_EXE]) {
        let exe = game.join(native(path));
        if !exe.is_file() {
            continue;
        }
        if let Err(err) = std::fs::OpenOptions::new().write(true).open(&exe) {
            return Err(format!(
                "{} is in use ({err}): the game is running from this folder. Close Eternal \
                 Survivors and run the setup again. Nothing was changed.",
                exe.display()
            ));
        }
    }
    Ok(())
}

/// Waits until the game no longer runs from `game` ([`check_not_running`] passes), looking every
/// `every`; `Err` with what to say when it still runs after `timeout`.
pub fn wait_until_closed(
    game: &Path,
    entries: &[Entry],
    timeout: std::time::Duration,
    every: std::time::Duration,
) -> Result<(), String> {
    let start = std::time::Instant::now();
    loop {
        match check_not_running(game, entries) {
            Ok(()) => return Ok(()),
            Err(why) if start.elapsed() >= timeout => {
                return Err(format!(
                    "The game did not close within {} seconds. {why}",
                    timeout.as_secs()
                ))
            }
            Err(_) => std::thread::sleep(every),
        }
    }
}

/// The command that starts the game in `game` again: its executable, in its own folder, with
/// none of the setup's console handed on.
pub fn relaunch_command(game: &Path) -> std::process::Command {
    let mut cmd = std::process::Command::new(game.join(GAME_EXE));
    cmd.current_dir(game)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    cmd
}

/// Starts the game in `game` again ([`relaunch_command`]), not waiting for it.
pub fn relaunch(game: &Path) -> io::Result<()> {
    relaunch_command(game).spawn().map(drop)
}

/// What an install did.
#[derive(Debug, Default)]
pub struct Installed {
    /// The game folder had been installed before.
    pub updated: bool,
    /// Files written.
    pub written: usize,
    /// Files an earlier version wrote that this one no longer has, removed.
    pub removed: usize,
}

/// Writes `entries` into `game` (the game folder): writes every file, removes what an earlier
/// setup wrote (its `setup-files.txt`) and this one does not carry, and makes the player's
/// folders. Nothing in the player's folders (`saves\`, `logs\`) is ever written or removed, and no
/// file outside the setup's lists is (the player's own files stay,
/// `benilla-config\` among them). Refuses before any change while the game runs from `game`.
pub fn install(game: &Path, entries: &[Entry]) -> io::Result<Installed> {
    let mut done = Installed {
        updated: installed(game) != Found::Nothing,
        ..Default::default()
    };
    for e in entries {
        check_path(&e.path)?;
    }
    check_not_running(game, entries)
        .map_err(|why| io::Error::new(io::ErrorKind::PermissionDenied, why))?;
    std::fs::create_dir_all(game)?;
    let old: BTreeSet<String> = std::fs::read_to_string(game.join(MANIFEST))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .filter(|l| check_path(l).is_ok())
        .collect();
    let mut new = BTreeSet::new();
    for e in entries {
        let to = game.join(native(&e.path));
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&to, &e.bytes)?;
        new.insert(e.path.clone());
        done.written += 1;
    }
    // what an older version carried and this one does not, case apart (Windows paths are)
    let carried: BTreeSet<String> = new.iter().map(|p| p.to_ascii_lowercase()).collect();
    for gone in old.iter().filter(|p| !carried.contains(&p.to_ascii_lowercase())) {
        let path = game.join(native(gone));
        if path.is_file() && std::fs::remove_file(&path).is_ok() {
            done.removed += 1;
            remove_empty_parents(game, &path);
        }
    }
    for folder in PLAYER_FOLDERS {
        std::fs::create_dir_all(game.join(folder))?;
    }
    let list: String = new.iter().map(|p| format!("{p}\n")).collect();
    std::fs::write(game.join(MANIFEST), list)?;
    Ok(done)
}

/// A game folder an earlier setup made in `wow` under an earlier name of the game, to be moved to
/// [`GAME_FOLDER`]; none when [`GAME_FOLDER`] is there already. Found by what every setup leaves,
/// not by name: its file list ([`MANIFEST`]) and the player's folders. With several, the newest
/// version's (a folder with no version file, from the first setup, counts as the oldest).
pub fn earlier_install(wow: &Path) -> Option<PathBuf> {
    if wow.join(GAME_FOLDER).exists() {
        return None;
    }
    let mut newest: Option<(Option<(u32, u32, u32)>, PathBuf)> = None;
    for entry in std::fs::read_dir(wow).ok()?.flatten() {
        let path = entry.path();
        let made_by_a_setup = path.join(MANIFEST).is_file()
            && PLAYER_FOLDERS.iter().all(|f| path.join(f).is_dir());
        if !made_by_a_setup {
            continue;
        }
        let version = std::fs::read_to_string(path.join(VERSION_FILE))
            .ok()
            .and_then(|text| parse_version(text.trim()));
        if newest.as_ref().map_or(true, |(v, _)| version > *v) {
            newest = Some((version, path));
        }
    }
    newest.map(|(_, path)| path)
}

/// Removes the folders above a removed file that it left empty, up to (not) the game folder.
fn remove_empty_parents(game: &Path, file: &Path) {
    let mut dir = file.parent();
    while let Some(d) = dir {
        if d == game || !d.starts_with(game) || std::fs::remove_dir(d).is_err() {
            break;
        }
        dir = d.parent();
    }
}

/// A carried path as this system spells it.
fn native(path: &str) -> PathBuf {
    path.split('/').collect()
}

/// The payload this executable carries.
pub fn own_payload() -> io::Result<Option<Vec<Entry>>> {
    let exe = std::env::current_exe()?;
    unpack(&mut File::open(exe)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn entry(path: &str, bytes: &[u8]) -> Entry {
        Entry {
            path: path.into(),
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn a_packed_payload_reads_back_after_the_stub() {
        let mut out = Vec::new();
        let files = [entry("Game.exe", b"MZ game"), entry("content/a.ron", b"(a: 1)")];
        pack(b"MZ setup stub", &files, &mut out).unwrap();
        assert!(out.starts_with(b"MZ setup stub"));
        let back = unpack(&mut Cursor::new(out)).unwrap().unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[1].path, "content/a.ron");
        assert_eq!(back[1].bytes, b"(a: 1)");
        assert!(unpack(&mut Cursor::new(b"MZ no payload".to_vec())).unwrap().is_none());
    }

    #[test]
    fn a_path_out_of_the_game_folder_or_into_the_saves_is_refused() {
        for path in ["../x", "C:/x", "/x", "saves/heroes.ron", "Saves/x", "logs/a", ""] {
            assert!(check_path(path).is_err(), "{path}");
        }
        assert!(check_path("content/zones/a.ron").is_ok());
    }

    #[test]
    fn an_update_replaces_the_game_and_never_touches_the_saves() {
        let game = std::env::temp_dir().join(format!("esurv-setup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&game);
        let v1 = [
            entry("Game.exe", b"v1"),
            entry("content/old.ron", b"old"),
            entry("OLD.txt", b"old"),
        ];
        let first = install(&game, &v1).unwrap();
        assert!(!first.updated);
        assert!(game.join("saves").is_dir() && game.join("logs").is_dir());
        std::fs::write(game.join("saves/heroes.ron"), b"my heroes").unwrap();
        std::fs::write(game.join("logs/esurv.log"), b"a log").unwrap();
        // the player's own files, outside the setup's list
        std::fs::create_dir_all(game.join("benilla-config")).unwrap();
        std::fs::write(game.join("benilla-config/keys.ron"), b"my keys").unwrap();
        std::fs::write(game.join("content/mine.ron"), b"my content").unwrap();
        std::fs::write(game.join("notes.txt"), b"my notes").unwrap();

        let v2 = [entry("Game.exe", b"v2"), entry("content/new.ron", b"new")];
        let second = install(&game, &v2).unwrap();
        assert!(second.updated);
        assert_eq!(second.removed, 2, "OLD.txt and old.ron went with the version that had them");
        assert_eq!(std::fs::read(game.join("Game.exe")).unwrap(), b"v2");
        assert!(!game.join("content/old.ron").exists());
        assert!(!game.join("OLD.txt").exists());
        assert_eq!(std::fs::read(game.join("saves/heroes.ron")).unwrap(), b"my heroes");
        assert_eq!(std::fs::read(game.join("logs/esurv.log")).unwrap(), b"a log");
        assert_eq!(std::fs::read(game.join("benilla-config/keys.ron")).unwrap(), b"my keys");
        assert_eq!(std::fs::read(game.join("content/mine.ron")).unwrap(), b"my content");
        assert_eq!(std::fs::read(game.join("notes.txt")).unwrap(), b"my notes");
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn an_earlier_install_is_found_by_what_a_setup_leaves_not_by_name() {
        let wow = std::env::temp_dir().join(format!("esurv-setup-earlier-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&wow);
        let made = |name: &str, version: Option<&str>| {
            let game = wow.join(name);
            for f in PLAYER_FOLDERS {
                std::fs::create_dir_all(game.join(f)).unwrap();
            }
            std::fs::write(game.join(MANIFEST), "Game.exe\n").unwrap();
            if let Some(v) = version {
                std::fs::write(game.join(VERSION_FILE), v).unwrap();
            }
            game
        };
        std::fs::create_dir_all(wow.join("Data")).unwrap();
        std::fs::create_dir_all(wow.join("Some Addon").join("saves")).unwrap();
        assert_eq!(earlier_install(&wow), None, "only a folder a setup made counts");

        made("First Name", None);
        let newest = made("Second Name", Some("v0.3.0"));
        std::fs::write(newest.join("saves").join("heroes.ron"), "hero").unwrap();
        assert_eq!(earlier_install(&wow), Some(newest.clone()), "the newest version wins");

        let game = wow.join(GAME_FOLDER);
        std::fs::rename(&newest, &game).unwrap();
        assert_eq!(std::fs::read_to_string(game.join("saves").join("heroes.ron")).unwrap(), "hero");
        assert_eq!(installed(&game), Found::Version("v0.3.0".into()));
        assert_eq!(earlier_install(&wow), None, "a game already under its name stays");
        let _ = std::fs::remove_dir_all(&wow);
    }

    #[test]
    fn a_folder_left_empty_by_a_dropped_file_goes_with_it() {
        let game = std::env::temp_dir().join(format!("esurv-setup-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&game);
        install(&game, &[entry("Game.exe", b"v1"), entry("content/old/a.ron", b"a")]).unwrap();
        install(&game, &[entry("Game.exe", b"v2")]).unwrap();
        assert!(!game.join("content").exists());
        std::fs::remove_dir_all(&game).ok();
    }

    /// The setup says what it does to the version it finds.
    #[test]
    fn the_plan_names_the_version_found_and_the_one_carried() {
        let say = |found, to| Plan::new(found, to).message();
        assert_eq!(say(Found::Nothing, "v0.2.0"), "Installing Eternal Survivors v0.2.0");
        assert_eq!(
            say(Found::Version("v0.1.0".into()), "v0.2.0"),
            "Updating Eternal Survivors from v0.1.0 to v0.2.0"
        );
        assert_eq!(say(Found::Version("v0.2.0".into()), "v0.2.0"), "Reinstalling Eternal Survivors v0.2.0");
        assert_eq!(
            Plan::new(Found::Version("v0.10.0".into()), "v0.2.0"),
            Plan::Downgrade { from: "v0.10.0".into(), to: "v0.2.0".into() },
            "the parts compare as numbers"
        );
        assert_eq!(
            say(Found::Unknown, "v0.2.0"),
            "Updating Eternal Survivors from an earlier version to v0.2.0"
        );
        assert_eq!(parse_version("v0.2.0\r\n"), Some((0, 2, 0)));
        assert_eq!(parse_version("0.2"), None);
    }

    #[test]
    fn the_version_found_is_read_from_the_game_folder() {
        let game = std::env::temp_dir().join(format!("esurv-setup-found-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&game);
        assert_eq!(installed(&game), Found::Nothing);
        install(&game, &[entry(GAME_EXE, b"v1")]).unwrap();
        assert_eq!(installed(&game), Found::Unknown, "a game with no version file");
        install(&game, &[entry(GAME_EXE, b"v1"), entry(VERSION_FILE, b"v0.1.0")]).unwrap();
        assert_eq!(installed(&game), Found::Version("v0.1.0".into()));
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn the_command_line_reads_the_folder_the_pause_and_the_relaunch() {
        let args = |a: &[&str]| parse_args(a.iter().map(|s| s.to_string()));
        assert_eq!(
            args(&[]).unwrap(),
            SetupArgs { wow: None, pause: true, relaunch: false }
        );
        assert_eq!(
            args(&["--wow", "C:/WoW", "--no-pause", RELAUNCH_FLAG]).unwrap(),
            SetupArgs { wow: Some(PathBuf::from("C:/WoW")), pause: false, relaunch: true }
        );
        assert!(args(&["--wow"]).is_err(), "the folder is needed");
        assert!(args(&["--wait"]).unwrap_err().contains(RELAUNCH_FLAG), "the options are named");
    }

    /// The game started again runs from its own folder, its own executable.
    #[test]
    fn the_relaunch_starts_the_game_exe_in_its_folder() {
        let game = Path::new("C:/WoW").join(GAME_FOLDER);
        let cmd = relaunch_command(&game);
        assert_eq!(Path::new(cmd.get_program()), game.join(GAME_EXE));
        assert_eq!(cmd.get_current_dir(), Some(game.as_path()));
        assert_eq!(cmd.get_args().count(), 0);
    }

    /// `--relaunch` waits for the game to close: it goes on once the executable is let go, and
    /// gives up, saying so, when it is held past the wait.
    #[cfg(windows)]
    #[test]
    fn the_relaunch_waits_for_the_game_to_close_and_gives_up_after_its_wait() {
        use std::os::windows::fs::OpenOptionsExt;
        use std::time::{Duration, Instant};
        let game = std::env::temp_dir().join(format!("esurv-setup-wait-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&game);
        install(&game, &[entry(GAME_EXE, b"v1")]).unwrap();
        let hold = || {
            std::fs::OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(game.join(GAME_EXE))
                .unwrap()
        };
        let v2 = [entry(GAME_EXE, b"v2")];
        let every = Duration::from_millis(20);
        assert!(wait_until_closed(&game, &v2, Duration::from_secs(5), every).is_ok(), "not running");

        let held = hold();
        let start = Instant::now();
        let why = wait_until_closed(&game, &v2, Duration::from_millis(200), every).unwrap_err();
        assert!(start.elapsed() >= Duration::from_millis(200));
        assert!(why.contains("did not close within") && why.contains("Nothing was changed"), "{why}");

        // the game closes while the setup waits
        let closer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            drop(held);
        });
        assert!(wait_until_closed(&game, &v2, Duration::from_secs(5), every).is_ok());
        closer.join().unwrap();
        std::fs::remove_dir_all(&game).ok();
    }

    /// A running game holds its executable: the setup refuses before it changes anything.
    #[cfg(windows)]
    #[test]
    fn a_running_game_is_refused_and_nothing_changes() {
        use std::os::windows::fs::OpenOptionsExt;
        let game = std::env::temp_dir().join(format!("esurv-setup-running-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&game);
        install(&game, &[entry(GAME_EXE, b"v1"), entry("content/a.ron", b"a1")]).unwrap();
        // held as Windows holds a running executable: shared for reading only
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(game.join(GAME_EXE))
            .unwrap();
        let v2 = [entry(GAME_EXE, b"v2"), entry("content/a.ron", b"a2")];
        let why = check_not_running(&game, &v2).unwrap_err();
        assert!(why.contains("running") && why.contains("Nothing was changed"), "{why}");
        assert!(install(&game, &v2).is_err());
        assert_eq!(std::fs::read(game.join("content/a.ron")).unwrap(), b"a1");
        drop(held);
        install(&game, &v2).unwrap();
        assert_eq!(std::fs::read(game.join("content/a.ron")).unwrap(), b"a2");
        std::fs::remove_dir_all(&game).ok();
    }
}
