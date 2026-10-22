//! `Eternal Survivors Setup.exe`: put in a World of Warcraft 1.12.1 folder and run, it checks the
//! client in its own folder, then installs `Eternal Survivors\` inside it with the files it
//! carries, or patches the version found there. It says which first:
//! installing, updating from the version found, reinstalling the same one, or, for a newer one
//! found, asks before going back to its own. The player's saves, logs and own files are never
//! touched, and it changes nothing while the game runs from that folder.
//!
//! `--wow <folder>` checks and installs into that folder instead of the setup's own; `--no-pause`
//! closes without waiting for Enter, and asks nothing: a newer game found is left as it is.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use esurv_setup::{
    carried_version, check_not_running, earlier_install, install, installed, own_payload,
    realm_files::{carry_realm_files, STATE_DIR},
    Plan, GAME_FOLDER,
};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut wow = None;
    let mut pause = true;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--wow" => wow = args.next().map(PathBuf::from),
            "--no-pause" => pause = false,
            _ => {
                println!("Unknown option {a}. Options: --wow <folder>, --no-pause.");
                return ExitCode::from(2);
            }
        }
    }
    let code = run(wow, pause);
    if pause {
        print!("\nPress Enter to close.");
        let _ = std::io::stdout().flush();
        let _ = read_line();
    }
    code
}

fn read_line() -> String {
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
    line
}

fn run(wow: Option<PathBuf>, ask: bool) -> ExitCode {
    let payload = match own_payload() {
        Ok(Some(p)) => p,
        Ok(None) => {
            println!("This setup carries no game files: it was not built for release.");
            return ExitCode::from(3);
        }
        Err(e) => {
            println!("This setup could not read the game files it carries: {e}");
            return ExitCode::from(3);
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
            return ExitCode::from(1);
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
        return ExitCode::from(1);
    }
    println!("{}", report.message());

    // a game installed under an earlier name of the game moves to its name, saves and all
    if let Some(earlier) = earlier_install(&folder) {
        let name = earlier.file_name().unwrap_or_default().to_string_lossy().into_owned();
        if let Err(e) = std::fs::rename(&earlier, folder.join(GAME_FOLDER)) {
            println!("\nThe game folder \"{name}\" could not be moved to \"{GAME_FOLDER}\": {e}");
            println!("Close the game if it is running and run this setup again. Nothing was changed.");
            return ExitCode::from(5);
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
            return ExitCode::from(4);
        }
        print!("Replace {from} with {to}? Type yes and press Enter to go on: ");
        let _ = std::io::stdout().flush();
        if !read_line().trim().eq_ignore_ascii_case("yes") {
            println!("Nothing was changed.");
            return ExitCode::from(4);
        }
    }
    if let Err(why) = check_not_running(&game, &payload) {
        println!("{why}");
        return ExitCode::from(5);
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
            println!("\nStart the game with \"{GAME_FOLDER}\\Eternal Survivors.exe\".");
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!("The install did not finish: {e}");
            println!("Nothing in the saves folder was changed. Run the setup again once fixed.");
            ExitCode::from(1)
        }
    }
}
