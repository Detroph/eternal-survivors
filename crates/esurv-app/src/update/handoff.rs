//! The hand-off to the setup: the downloaded setup is put in a folder of its own inside the game
//! folder ([`DOWNLOAD_FOLDER`]) and started on the WoW folder with `--no-pause` and the setup's
//! relaunch flag, in a console window of its own; the game then quits, the setup waits for it to
//! close, updates it and starts it again. The next start removes the folder.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use esurv_setup::{GAME_FOLDER, RELAUNCH_FLAG, SETUP_EXE, VERSION_FILE};

/// The folder inside the game folder the setup is downloaded to; the setup never writes it (it is
/// not among the files it carries), and the next start removes it.
pub const DOWNLOAD_FOLDER: &str = "update-download";

/// Where the game is installed: its folder, and the WoW folder holding it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Install {
    pub game: PathBuf,
    pub wow: PathBuf,
}

/// The install the game at `exe` runs from, when the setup made it: the executable sits in
/// [`GAME_FOLDER`] beside the setup's `VERSION.txt`, inside the WoW folder; `None` otherwise (a
/// build run where it was built), which the setup could not update in place.
pub fn install_of(exe: &Path) -> Option<Install> {
    let game = exe.parent()?;
    let named = game
        .file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(GAME_FOLDER));
    if !named || !game.join(VERSION_FILE).is_file() {
        return None;
    }
    Some(Install {
        game: game.to_path_buf(),
        wow: game.parent()?.to_path_buf(),
    })
}

/// Writes the downloaded setup into the game's [`DOWNLOAD_FOLDER`] (emptied first); its path.
pub fn stage_setup(game: &Path, setup: &[u8]) -> io::Result<PathBuf> {
    let folder = game.join(DOWNLOAD_FOLDER);
    if folder.exists() {
        std::fs::remove_dir_all(&folder)?;
    }
    std::fs::create_dir_all(&folder)?;
    let path = folder.join(SETUP_EXE);
    std::fs::write(&path, setup)?;
    Ok(path)
}

/// The command that starts the staged setup on the WoW folder: `--wow <folder> --no-pause` and
/// the relaunch flag, in a console window of its own (the setup takes it for its output), nothing
/// of the game's handed on.
pub fn setup_command(setup: &Path, wow: &Path) -> Command {
    let mut cmd = Command::new(setup);
    cmd.arg("--wow")
        .arg(wow)
        .arg("--no-pause")
        .arg(RELAUNCH_FLAG)
        .current_dir(setup.parent().unwrap_or(wow))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        cmd.creation_flags(CREATE_NEW_CONSOLE);
    }
    cmd
}

/// Removes the [`DOWNLOAD_FOLDER`] an update left in `game`: true once it is gone (or was never
/// there). The setup that just updated the game may still be closing, holding its file, so the
/// caller tries again a little later.
pub fn clean_up(game: &Path) -> bool {
    let folder = game.join(DOWNLOAD_FOLDER);
    !folder.exists() || std::fs::remove_dir_all(&folder).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("esurv-update-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn only_a_game_the_setup_installed_is_updated_in_place() {
        let wow = scratch("install");
        let game = wow.join(GAME_FOLDER);
        std::fs::create_dir_all(&game).unwrap();
        let exe = game.join(esurv_setup::GAME_EXE);
        assert_eq!(install_of(&exe), None, "no VERSION.txt: not the setup's");
        std::fs::write(game.join(VERSION_FILE), "v0.4.0").unwrap();
        assert_eq!(install_of(&exe), Some(Install { game: game.clone(), wow: wow.clone() }));
        // a build run from its target folder
        let elsewhere = wow.join("target").join("release");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(elsewhere.join(VERSION_FILE), "v0.4.0").unwrap();
        assert_eq!(install_of(&elsewhere.join("esurv.exe")), None);
        std::fs::remove_dir_all(&wow).ok();
    }

    #[test]
    fn the_setup_is_staged_in_its_folder_and_cleaned_up() {
        let game = scratch("stage");
        std::fs::create_dir_all(&game).unwrap();
        let first = stage_setup(&game, b"old").unwrap();
        let setup = stage_setup(&game, b"MZ setup").unwrap();
        assert_eq!(first, setup);
        assert_eq!(setup, game.join(DOWNLOAD_FOLDER).join(SETUP_EXE));
        assert_eq!(std::fs::read(&setup).unwrap(), b"MZ setup");
        assert!(clean_up(&game));
        assert!(!game.join(DOWNLOAD_FOLDER).exists());
        assert!(clean_up(&game), "nothing left to clean");
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn the_setup_starts_on_the_wow_folder_asking_nothing_and_relaunching() {
        let wow = Path::new("C:/Games/WoW 1.12");
        let setup = wow.join(GAME_FOLDER).join(DOWNLOAD_FOLDER).join(SETUP_EXE);
        let cmd = setup_command(&setup, wow);
        assert_eq!(Path::new(cmd.get_program()), setup);
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args, ["--wow", "C:/Games/WoW 1.12", "--no-pause", RELAUNCH_FLAG]);
        // the arguments are the ones the setup reads
        let read = esurv_setup::parse_args(args).unwrap();
        assert_eq!(read.wow.as_deref(), Some(wow));
        assert!(!read.pause && read.relaunch);
    }
}
