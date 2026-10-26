//! The game launcher: starts the embedded server on its own threads, then the benilla client
//! pointed at it. The client logs on by itself with a password made for this launch, and enters
//! the game's one realm; the zone is chosen with the hero.
//!
//! A player build (no `dev`) opens no console: what it prints goes to `logs\esurv.log` beside the
//! game, and a client it cannot play on is said in a message box.

#![cfg_attr(not(feature = "dev"), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use benilla_app::BuildId;
use rand::distributions::{Alphanumeric, DistString};
use esurv_content::LiveContent;

/// The account name the client logs on with (any name is accepted).
const ACCOUNT: &str = "HERO";

fn main() -> benilla_app::AppExit {
    let build = BuildId {
        version: env!("CARGO_PKG_VERSION"),
        describe: env!("BENILLA_GIT_DESCRIBE"),
        sha: env!("BENILLA_GIT_SHA"),
        short: env!("BENILLA_GIT_SHORT"),
        date: env!("BENILLA_GIT_DATE"),
        profile: env!("BENILLA_PROFILE"),
        project_dir: env!("BENILLA_PROJECT_DIR"),
        ..Default::default()
    };
    // the same folder `run_with` records, set first so the server finds the install and saves
    benilla_formats::set_project_folder(build.project_dir);
    #[cfg(not(feature = "dev"))]
    log_beside_the_game();
    if let Err(why) = check_client() {
        eprintln!("esurv: {why}");
        show_error(&why);
        return benilla_app::AppExit::from_code(1);
    }
    // the game's content, for the server's rules and the client's screens alike, its names read
    // from the player's client (a reload reads them the same way)
    let content = match LiveContent::load_with_names(content_dir(), Arc::new(client_names())) {
        Ok(c) => {
            report_default_assists(&c.current());
            Some(Arc::new(c))
        }
        Err(e) => {
            eprintln!("esurv: the game content did not load: {e:#}");
            None
        }
    };
    let mut link = None;
    if let Some(content) = &content {
        match start_server(content.clone()) {
            // the character screen reads and changes the same save, before any run
            Ok((save, data)) => {
                link = Some(esurv_app::prerun::PreRunLink(Arc::new(LocalSave {
                    content: content.clone(),
                    saves: save,
                    data,
                })))
            }
            Err(e) => eprintln!("esurv: the game server did not start: {e:#}"),
        }
    }
    // dev builds load the content again as its files change; kept until the game closes
    #[cfg(feature = "dev")]
    let _watcher = content.clone().and_then(watch_content);
    // the updater: a player build checks the release feed; a dev build only a test feed
    // (`ESURV_UPDATE_FEED`)
    let update = std::env::current_exe().ok().and_then(|exe| {
        esurv_app::update::UpdateConfig::for_launch(cfg!(not(feature = "dev")), &exe, env!("CARGO_PKG_VERSION"))
    });
    benilla_app::run_with(build, move |app| {
        esurv_app::extend(app, content, link);
        esurv_app::update::register(app, update);
    })
}

/// The names the content points at (spells, zones, classes, races), read from the player's own
/// client; with no client data, or one that does not read, the pointers show as placeholders.
fn client_names() -> esurv_content::ClientNames {
    match esurv_content::ClientNames::from_install() {
        Ok(Some(names)) => names,
        Ok(None) => esurv_content::ClientNames::none(),
        Err(e) => {
            eprintln!("esurv: the client's names did not read, showing placeholders: {e:#}");
            esurv_content::ClientNames::none()
        }
    }
}

/// Dev builds: watches the content folder and loads it again after each burst of changes. A run
/// going on keeps the content it started with; new runs and the screens before a run take the new
/// one. A folder that fails to load keeps the old content, and says why.
#[cfg(feature = "dev")]
fn watch_content(live: Arc<LiveContent>) -> Option<esurv_content::watch::ContentWatcher> {
    let dir = live.dir().map(|d| d.display().to_string()).unwrap_or_default();
    let watching = esurv_content::watch::watch(live, move |reloaded| match reloaded {
        // the save keeps class gear by slot and zone id, so it needs nothing from the new content
        Ok(c) => {
            eprintln!("esurv: content reloaded from {dir}");
            report_default_assists(c);
        }
        Err(e) => eprintln!("esurv: the content did not reload, keeping what was loaded: {e:#}"),
    });
    match watching {
        Ok(w) => Some(w),
        Err(e) => {
            eprintln!("esurv: the content folder is not watched: {e:#}");
            None
        }
    }
}

/// The character screen's way to the server in a local game: its asks go straight to the
/// embedded server's save, by the server's own rules; being on the save's own machine, it may also
/// export and import the save.
struct LocalSave {
    content: Arc<LiveContent>,
    saves: Arc<esurv_server::save::Saves>,
    data: Option<Arc<esurv_server::ground::GameData>>,
}

impl esurv_app::prerun::PreRunCarrier for LocalSave {
    fn ask(&self, ask: &esurv_protocol::PreRunAsk) -> Vec<esurv_protocol::ToClient> {
        esurv_server::prerun::answer_local(
            &self.content.current(),
            &self.saves,
            self.data.as_deref(),
            ask,
        )
    }
}

/// The save the heroes are kept in and the player's data, as the server reads them.
type Started = (
    Arc<esurv_server::save::Saves>,
    Option<Arc<esurv_server::ground::GameData>>,
);

/// Starts the server on this machine and points the client at it; the save it keeps the heroes in
/// and the player's data it reads.
fn start_server(live: Arc<LiveContent>) -> Result<Started> {
    let content = live.current();
    let saves = Arc::new(esurv_server::save::Saves::open(
        &save_path().context("no folder to keep the heroes in")?,
    )?);
    saves.rein_in(&content);
    let data = match benilla_formats::wow_data() {
        Some(dir) => match esurv_server::ground::GameData::open(&dir, &content) {
            Ok(d) => Some(d),
            Err(e) => {
                eprintln!("esurv: playing on flat ground, the game data did not open: {e:#}");
                None
            }
        },
        None => None,
    };
    let password = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
    let zone = std::env::var("ESURV_ZONE")
        .ok()
        .and_then(|z| zone_index(&content, &z));
    let server = esurv_server::start(esurv_server::ServerConfig {
        zone,
        content: live,
        saves: saves.clone(),
        data: data.clone(),
        password: password.clone(),
        bind_host: "127.0.0.1".into(),
        public_host: "127.0.0.1".into(),
        realmd_port: 0,
        autostart: std::env::var_os("ESURV_AUTOSTART").is_some(),
        granted: std::env::var("ESURV_GRANT")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default(),
        move_log: std::env::var_os("ESURV_MOVE_LOG").map(Into::into),
    })?;
    // read by the client's login screen, before any of its threads start
    std::env::set_var("WOW_HOST", server.realmd.to_string());
    std::env::set_var("WOW_USER", ACCOUNT);
    std::env::set_var("WOW_PASS", password);
    // the one realm, so the realm list never shows: the zone is chosen with the hero
    std::env::set_var("WOW_REALM", esurv_server::REALM_NAME);
    Ok((saves, data))
}

/// `ESURV_ZONE=<zone>` (a dev switch): a zone by its id or its name, either case.
fn zone_index(content: &esurv_content::Content, zone: &str) -> Option<usize> {
    let zone = zone.trim();
    let found = content
        .zones
        .iter()
        .position(|z| z.id.eq_ignore_ascii_case(zone) || z.name.eq_ignore_ascii_case(zone));
    if found.is_none() {
        eprintln!("esurv: ESURV_ZONE names no zone: {zone}");
    }
    found
}

/// The game content: `$ESURV_CONTENT`, else `content/` in the project folder (dev builds) or beside
/// the executable.
fn content_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ESURV_CONTENT") {
        return dir.into();
    }
    if let Some(project) = benilla_formats::project_folder() {
        return project.join("content");
    }
    exe_dir().join("content")
}

/// The heroes' save: `$ESURV_SAVE`, else `saves/heroes.ron` beside the game:
/// the game's own folder, or the project folder in a dev build (its executable sits in
/// `target/`, which a clean wipes). A save still in benilla's state folder, where it used to live,
/// is copied over the first time.
fn save_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ESURV_SAVE") {
        return Some(path.into());
    }
    let home = benilla_formats::project_folder().map(PathBuf::from).unwrap_or_else(exe_dir);
    let path = home.join("saves").join("heroes.ron");
    if !path.exists() {
        if let Some(old) = benilla_app::config_dir().map(|d| d.join("saves").join("heroes.ron")).filter(|p| p.exists()) {
            let moved = std::fs::create_dir_all(home.join("saves")).and_then(|_| std::fs::copy(&old, &path));
            match moved {
                Ok(_) => eprintln!("esurv: the save moved beside the game: {} (the old copy stays)", path.display()),
                Err(e) => eprintln!("esurv: could not copy the save from {} to {}: {e}", old.display(), path.display()),
            }
        }
    }
    Some(path)
}


/// The client check at start (`esurv_client_check`): the folder the game data comes from holds
/// WoW 1.12.1 build 5875, enUS, with its whole patch chain. A player build stops on a wrong or
/// missing client with the reason; a dev build, or data named by `WOW_DATA`, only says it.
fn check_client() -> Result<(), String> {
    let named = std::env::var_os("WOW_DATA").is_some();
    let strict = cfg!(not(feature = "dev")) && !named;
    let Some(data) = benilla_formats::wow_data() else {
        let why = "Eternal Survivors found no World of Warcraft 1.12.1 client.\n\n\
                   Its folder belongs inside your WoW 1.12.1 folder, beside WoW.exe and Data: \
                   put \"Eternal Survivors Setup.exe\" in your WoW folder and run it there."
            .to_owned();
        return if strict { Err(why) } else { Ok(()) };
    };
    // data named by hand may sit apart from its WoW.exe: only its archives are checked
    let (folder, problems) = match data.parent().filter(|_| !named) {
        Some(folder) => (folder, esurv_client_check::check_folder(folder).problems),
        None => (data.as_path(), esurv_client_check::check_data(&data)),
    };
    if problems.is_empty() {
        return Ok(());
    }
    let mut why = format!(
        "Eternal Survivors plays on World of Warcraft {} enUS, and {} is not that client:\n",
        esurv_client_check::CLIENT_VERSION,
        folder.display()
    );
    for p in &problems {
        why.push_str(&format!("\n  - {p}"));
    }
    if strict {
        Err(why)
    } else {
        eprintln!("esurv: {why}");
        Ok(())
    }
}

/// Says why the game cannot start: a message box on Windows (a player build has no console).
fn show_error(text: &str) {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(
                hwnd: *mut std::ffi::c_void,
                text: *const u16,
                caption: *const u16,
                kind: u32,
            ) -> i32;
        }
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        const MB_ICONERROR: u32 = 0x10;
        // SAFETY: both strings are NUL-terminated UTF-16 that outlive the call; no owner window.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                wide(text).as_ptr(),
                wide("Eternal Survivors").as_ptr(),
                MB_ICONERROR,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = text;
}

/// A player build: what the game prints goes to `logs\esurv.log` beside it (the last launch's kept
/// as `esurv.prev.log`), since it opens no console.
#[cfg(not(feature = "dev"))]
fn log_beside_the_game() {
    let dir = exe_dir().join("logs");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let log = dir.join("esurv.log");
    let _ = std::fs::rename(&log, dir.join("esurv.prev.log"));
    let Ok(file) = std::fs::File::create(&log) else {
        return;
    };
    #[cfg(windows)]
    {
        use std::os::windows::io::IntoRawHandle;
        #[link(name = "kernel32")]
        extern "system" {
            fn SetStdHandle(which: u32, handle: *mut std::ffi::c_void) -> i32;
        }
        const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
        const STD_ERROR_HANDLE: u32 = -12i32 as u32;
        let Ok(second) = file.try_clone() else {
            return;
        };
        // SAFETY: the handles are open files leaked for the life of the process; std reads the
        // standard handles on every write, so both streams go to the file from here.
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, file.into_raw_handle());
            SetStdHandle(STD_ERROR_HANDLE, second.into_raw_handle());
        }
    }
    #[cfg(not(windows))]
    drop(file);
}

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(PathBuf::from))
        .unwrap_or_default()
}

/// Names in the log every hand-made player character still on its class's default assist (no
/// `assist_script` of its own in player_characters.ron), and every one on its class's default rotation
/// (no `behaviour`), so the ones that need one are easy to find.
fn report_default_assists(content: &esurv_content::Content) {
    let on_default = content.characters_on_default_assist();
    if !on_default.is_empty() {
        let list: Vec<String> = on_default.iter().map(|(id, default)| format!("{id} ({default})")).collect();
        eprintln!("esurv: {} player character(s) use their class's default assist: {}", list.len(), list.join(", "));
    }
    let on_default = content.characters_on_default_behaviour();
    if !on_default.is_empty() {
        let list: Vec<String> = on_default.iter().map(|(id, default)| format!("{id} ({default})")).collect();
        eprintln!("esurv: {} player character(s) use their class's default rotation: {}", list.len(), list.join(", "));
    }
}

#[cfg(test)]
mod tests {
    /// The client's glow knows the Rare Cache by the server's entry for it.
    #[test]
    fn the_glow_knows_the_rare_cache_by_the_servers_entry() {
        assert_eq!(esurv_app::enemy_glow::RARE_CACHE_ENTRY, esurv_server::entries::RARE_CACHE);
    }
}
