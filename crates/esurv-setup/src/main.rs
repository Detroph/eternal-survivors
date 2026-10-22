//! `Eternal Survivors Setup.exe`: put in a World of Warcraft 1.12.1 folder and run, it checks the
//! client in its own folder, then installs `Eternal Survivors\` inside it with the files it
//! carries, or patches the version found there. It says which first:
//! installing, updating from the version found, reinstalling the same one, or, for a newer one
//! found, asks before going back to its own. The player's saves, logs and own files are never
//! touched, and it changes nothing while the game runs from that folder.
//!
//! `--wow <folder>` checks and installs into that folder instead of the setup's own; `--no-pause`
//! closes without waiting for Enter, and asks nothing: a newer game found is left as it is.
//! `--relaunch` is how the game's updater starts it: it waits up to a minute for the game to
//! close, and starts the game again once the update is in; a failure waits for Enter even with
//! `--no-pause`, so the reason can be read in the window the game opened.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use esurv_setup::{
    carried_version, check_not_running, earlier_install, install, installed, own_payload,
    parse_args, realm_files::{carry_realm_files, STATE_DIR}, relaunch, wait_until_closed,
    Plan, GAME_CLOSE_WAIT, GAME_EXE, GAME_FOLDER,
};

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(why) => {
            println!("{why}");
            return ExitCode::from(2);
        }
    };
    if args.relaunch {
        use_own_console();
    }
    let code = run(args.wow, args.pause, args.relaunch);
    if args.pause || (args.relaunch && code != 0) {
        print!("\nPress Enter to close.");
        let _ = std::io::stdout().flush();
        let _ = read_line();
    }
    ExitCode::from(code)
}

/// Started by the game with no console handles of its own: the console window the game opened for
/// the setup takes its output and input.
fn use_own_console() {
    #[cfg(windows)]
    {
        use std::os::windows::io::IntoRawHandle;
        #[link(name = "kernel32")]
        extern "system" {
            fn SetStdHandle(which: u32, handle: *mut std::ffi::c_void) -> i32;
        }
        const STD_INPUT_HANDLE: u32 = -10i32 as u32;
        const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
        const STD_ERROR_HANDLE: u32 = -12i32 as u32;
        let open = |name: &str| std::fs::OpenOptions::new().read(true).write(true).open(name);
        // SAFETY: each handle is an open console file leaked for the life of the process; std
        // reads the standard handles on every read and write.
        unsafe {
            if let Ok(out) = open("CONOUT$") {
                if let Ok(err) = out.try_clone() {
                    SetStdHandle(STD_ERROR_HANDLE, err.into_raw_handle());
                }
                SetStdHandle(STD_OUTPUT_HANDLE, out.into_raw_handle());
            }
            if let Ok(input) = open("CONIN$") {
                SetStdHandle(STD_INPUT_HANDLE, input.into_raw_handle());
            }
        }
    }
}

fn read_line() -> String {
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
    line
}

fn run(wow: Option<PathBuf>, ask: bool, restart: bool) -> u8 {
    let payload = match own_payload() {
        Ok(Some(p)) => p,
        Ok(None) => {
            println!("This setup carries no game files: it was not built for release.");
            return 3;
        }
        Err(e) => {
            println!("This setup could not read the game files it carries: {e}");
            return 3;
        }
    };
    let version = carried_version(&payload);
    println!("Eternal Survivors {version} setup\n");

    let folder = match wow.or_else(|| {
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(PathBuf::from))
    }) {
        Some(f) => f,
        None => {
            println!("The setup could not tell which folder it is in.");
            return 1;
        }
    };
    let report = esurv_client_check::check_folder(&folder);
    if !report.is_usable() {
        println!("{}", report.message());
        println!(
            "Put this setup in your World of Warcraft {} folder (the one with WoW.exe and the \
             Data folder, English client) and run it again.",
            esurv_client_check::CLIENT_VERSION
        );
        return 1;
    }
    println!("{}", report.message());

    // started by the game's updater: the game closes itself once this has started
    if restart {
        println!("\nWaiting for the game to close ...");
        let every = Duration::from_millis(250);
        if let Err(why) = wait_until_closed(&folder.join(GAME_FOLDER), &payload, GAME_CLOSE_WAIT, every) {
            println!("{why}");
            return 5;
        }
    }

    // a game installed under an earlier name of the game moves to its name, saves and all
    if let Some(earlier) = earlier_install(&folder) {
        let name = earlier.file_name().unwrap_or_default().to_string_lossy().into_owned();
        if let Err(e) = std::fs::rename(&earlier, folder.join(GAME_FOLDER)) {
            println!("\nThe game folder \"{name}\" could not be moved to \"{GAME_FOLDER}\": {e}");
            println!("Close the game if it is running and run this setup again. Nothing was changed.");
            return 5;
        }
        println!("\nMoved \"{name}\" to \"{GAME_FOLDER}\" (the game's new name), saves and all.");
    }
    let game = folder.join(GAME_FOLDER);
    let plan = Plan::new(installed(&game), &version);
    println!("\n{}", plan.message());
    println!("  in {}", game.display());
    if let Plan::Downgrade { from, to } = &plan {
        println!(
            "  Going back to {to} keeps your saves folder as it is, but an older game may not \
             read what a newer one saved."
        );
        if !ask {
            println!(
                "Nothing was changed (--no-pause asks nothing). Run this setup without it to be \
                 asked."
            );
            return 4;
        }
        print!("Replace {from} with {to}? Type yes and press Enter to go on: ");
        let _ = std::io::stdout().flush();
        if !read_line().trim().eq_ignore_ascii_case("yes") {
            println!("Nothing was changed.");
            return 4;
        }
    }
    if let Err(why) = check_not_running(&game, &payload) {
        println!("{why}");
        return 5;
    }

    match install(&game, &payload) {
        Ok(done) => {
            let what = match &plan {
                Plan::Install { .. } => "installed".to_owned(),
                Plan::Update { from: Some(from), .. } => format!("updated from {from}"),
                Plan::Update { from: None, .. } => "updated".to_owned(),
                Plan::Reinstall { .. } => "reinstalled".to_owned(),
                Plan::Downgrade { from, .. } => format!("back in place of {from}"),
            };
            println!("Eternal Survivors {version} is {what}.");
            println!("  {} files written, {} old files removed.", done.written, done.removed);
            // the characters' settings under the realm's earlier names move to its current one
            // (the realm carries the game's name)
            match carry_realm_files(&game.join(STATE_DIR), GAME_FOLDER) {
                Ok(c) if c.moved + c.deleted > 0 => {
                    println!("  Your characters' settings were carried over to the game's new name.")
                }
                Ok(_) => {}
                Err(e) => println!("  Your characters' settings could not all be carried over: {e}"),
            }
            println!("  Your saves and logs folders were not touched.");
            if restart {
                match relaunch(&game) {
                    Ok(()) => println!("\nStarting the game again."),
                    Err(e) => println!(
                        "\nThe game could not be started again ({e}): start it with \
                         \"{GAME_FOLDER}\\{GAME_EXE}\"."
                    ),
                }
            } else {
                println!("\nStart the game with \"{GAME_FOLDER}\\{GAME_EXE}\".");
            }
            0
        }
        Err(e) => {
            println!("The install did not finish: {e}");
            println!("Nothing in the saves folder was changed. Run the setup again once fixed.");
            1
        }
    }
}
