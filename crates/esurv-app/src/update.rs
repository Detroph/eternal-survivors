//! The updater (server.md, "Updates"): a player build asks the game's public release feed once
//! at start whether a newer version is out, and the warband screen offers it.
//!
//! * **Check.** Once the interface's settings are in, with the option on ([`CVAR`], "Check for
//!   updates" on the options panel's Interface tab), a thread makes one anonymous GET of the feed
//!   ([`release::FEED`]: GitHub's latest release) and compares its tag with `VERSION.txt`. The game
//!   never waits on it; offline, rate-limited or any failure says nothing. The request carries a
//!   User-Agent of the game's name and version and nothing else. Dev builds never check, unless
//!   `ESURV_UPDATE_FEED` names a test feed (which any build then asks instead, plain HTTP allowed).
//! * **Offer.** A newer release (one with both its archive and its `.sha256`) shows "v0.5.0 is
//!   available" with an Update button over the warband screen's Options ([`notice_view`]).
//! * **Download.** Update downloads the archive (its progress shown) and its checksum, checks the
//!   archive against it, reads the setup out of it, checks the setup carries the release's
//!   version, and writes it into `update-download\` in the game folder ([`fetch_setup`]). Any
//!   failure says so in a line and changes nothing.
//! * **Hand-off.** The setup starts on the WoW folder with `--no-pause --relaunch` ([`handoff`]),
//!   and the game quits; the setup waits for it to close, updates it in place (saves and settings
//!   kept) and starts it again. The next start removes `update-download\`.

mod checksum;
mod fetch;
mod handoff;
mod release;
mod unzip;

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use benilla_ui::script::UiScript;
use bevy::prelude::*;

pub use fetch::Feed;
pub use handoff::{Install, DOWNLOAD_FOLDER};
pub use release::{Release, Version, FEED};

use crate::menus::{MenuAction, MenuPress};
use fetch::Progress;

/// The option's CVar: `"1"` checks for updates at start (the default), `"0"` never asks.
pub const CVAR: &str = "esurvCheckForUpdates";
/// The option's value when nothing was saved: on.
pub const DEFAULT: &str = "1";
/// The environment variable naming a test feed (a URL answering as GitHub's latest release does).
pub const FEED_ENV: &str = "ESURV_UPDATE_FEED";
/// How often the option is read from its CVar (seconds).
const OPTION_EVERY: f32 = 1.0;

/// Registers the option's CVar with its default, as an addon's is, so it reads and is kept.
pub fn register_cvar(script: &UiScript) {
    let _ = script.eval::<()>(&format!("RegisterCVar(\"{CVAR}\", \"{DEFAULT}\")"));
}

/// Whether a CVar value means on: anything but `"0"` (an unknown value is the default).
pub fn on_from_value(v: Option<&str>) -> bool {
    v.is_none_or(|v| v.trim() != "0")
}

/// What the launcher hands the updater.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateConfig {
    pub feed: Feed,
    /// The version installed, as `VERSION.txt` says it (`v0.4.0`).
    pub installed: String,
    /// Where the setup made the install; `None` for a game it did not install, which the
    /// notice then tells to update by hand.
    pub install: Option<Install>,
}

impl UpdateConfig {
    /// The updater for this launch, `None` for none: a player build asks [`FEED`] when its folder
    /// holds `VERSION.txt`; any build asks the test feed `ESURV_UPDATE_FEED` names, if one is
    /// named (its version `VERSION.txt`'s, else `v` and the crate version).
    pub fn for_launch(player_build: bool, exe: &Path, crate_version: &str) -> Option<Self> {
        let installed = exe
            .parent()
            .and_then(|d| std::fs::read_to_string(d.join(esurv_setup::VERSION_FILE)).ok())
            .map(|v| v.trim().to_owned());
        let test_feed = std::env::var(FEED_ENV).ok();
        let (feed, installed) = choose_feed(player_build, test_feed, installed, crate_version)?;
        Some(Self {
            feed,
            installed,
            install: handoff::install_of(exe),
        })
    }
}

/// The feed and the installed version: see [`UpdateConfig::for_launch`].
pub fn choose_feed(
    player_build: bool,
    test_feed: Option<String>,
    installed: Option<String>,
    crate_version: &str,
) -> Option<(Feed, String)> {
    if let Some(url) = test_feed.map(|u| u.trim().to_owned()).filter(|u| !u.is_empty()) {
        let installed = installed.unwrap_or_else(|| format!("v{crate_version}"));
        return Some((Feed { url, https_only: false }, installed));
    }
    if !player_build {
        return None;
    }
    let feed = Feed {
        url: FEED.to_owned(),
        https_only: true,
    };
    installed.map(|v| (feed, v))
}

/// Where the updater is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Nothing to offer: not checked yet, checking, nothing newer, or the check failed.
    #[default]
    Quiet,
    /// This newer version is out.
    Available(Version),
    /// Downloading it.
    Downloading(Version),
    /// The update failed, saying why; nothing was changed, and Update tries again.
    Failed(Version, String),
    /// The setup has started: the game quits for it.
    Restarting(Version),
}

/// The notice as the warband screen shows it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NoticeView {
    /// Its line: what is available, being done, or what went wrong.
    pub line: String,
    /// The line says a failure.
    pub failed: bool,
    /// The Update button, and whether it can be pressed; none once the setup runs.
    pub button: Option<bool>,
    /// A progress line under it, kept up to date in place ([`UpdateProgressText`]).
    pub progress: bool,
    /// It offers a newer version ("vX is available"): news, so the box glows and pulses
    /// (`menus::look::NewsPulse`).
    pub offer: bool,
}

/// The notice for `phase`, the option `on` or off (off hides an offer, not an update under way).
pub fn notice_view(phase: &Phase, on: bool) -> Option<NoticeView> {
    let view = |line: String, failed, button, progress| NoticeView {
        line,
        failed,
        button,
        progress,
        offer: false,
    };
    match phase {
        Phase::Quiet => None,
        Phase::Available(v) => on.then(|| NoticeView {
            offer: true,
            ..view(format!("{} is available", release::version_text(*v)), false, Some(true), false)
        }),
        Phase::Failed(_, why) => on.then(|| view(why.clone(), true, Some(true), false)),
        Phase::Downloading(v) => Some(view(
            format!("Downloading {}", release::version_text(*v)),
            false,
            Some(false),
            true,
        )),
        Phase::Restarting(v) => Some(view(
            format!("Updating to {}: the game restarts", release::version_text(*v)),
            false,
            None,
            false,
        )),
    }
}

/// A download's progress as the notice says it: the share once the size is known, else the
/// megabytes in.
pub fn progress_text(done: u64, total: u64) -> String {
    let mb = |b: u64| b as f64 / (1024.0 * 1024.0);
    if total > 0 {
        let pct = (done.min(total) * 100 / total) as u32;
        format!("{pct}%  ({:.1} of {:.1} MB)", mb(done), mb(total))
    } else {
        format!("{:.1} MB", mb(done))
    }
}

/// The failure line: what went wrong, and that nothing changed.
fn failure(why: &str) -> String {
    format!("The update failed: {why}. Nothing was changed.")
}

/// Downloads `release`, checks the archive against its checksum, reads the setup out of it and
/// checks it carries the release's version, then writes it into the game folder's
/// [`DOWNLOAD_FOLDER`]: the setup's path. `Err` says what went wrong; nothing of the game is
/// touched either way.
pub fn fetch_setup(
    release: &Release,
    feed: &Feed,
    user_agent: &str,
    game: &Path,
    progress: &Progress,
) -> Result<PathBuf, String> {
    let zip = fetch::download(&release.zip.url, feed.https_only, user_agent, progress)
        .map_err(|e| format!("the download failed, {e}"))?;
    let sums = fetch::download(&release.sha256.url, feed.https_only, user_agent, &Progress::default())
        .map_err(|e| format!("the checksum did not download, {e}"))?;
    checksum::verify(&zip, &String::from_utf8_lossy(&sums), &release.zip.name)?;
    let setup = unzip::read_file(&zip, esurv_setup::SETUP_EXE)?;
    let carried = esurv_setup::unpack(&mut Cursor::new(&setup))
        .ok()
        .flatten()
        .and_then(|files| esurv_setup::parse_version(&esurv_setup::carried_version(&files)));
    if carried != Some(release.version) {
        return Err(format!(
            "the setup downloaded is not {}",
            release::version_text(release.version)
        ));
    }
    handoff::stage_setup(game, &setup).map_err(|e| format!("the setup could not be saved ({e})"))
}

/// The updater's state, shared with its threads.
#[derive(Resource)]
pub struct Updater {
    config: Option<UpdateConfig>,
    phase: Arc<Mutex<Phase>>,
    release: Arc<Mutex<Option<Release>>>,
    progress: Arc<Progress>,
    checked: bool,
}

/// What the menus draw: the phase and the option.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateNotice {
    pub phase: Phase,
    pub on: bool,
}

impl UpdateNotice {
    pub fn view(&self) -> Option<NoticeView> {
        notice_view(&self.phase, self.on)
    }
}

/// Marks the notice's progress line, which [`show_progress`] keeps up to date.
#[derive(Component)]
pub struct UpdateProgressText;

/// The updater: its systems, started with the launcher's `config` (none: never checks).
pub fn register(app: &mut App, config: Option<UpdateConfig>) {
    if let Some(install) = config.as_ref().and_then(|c| c.install.clone()) {
        clean_up_later(install.game);
    }
    app.insert_resource(Updater {
        config,
        phase: default(),
        release: default(),
        progress: default(),
        checked: false,
    })
    .init_resource::<UpdateNotice>()
    .add_systems(Update, (read_option, press_update, follow, show_progress).chain());
}

/// Removes the download folder an update left, on a thread, trying again while the setup that
/// started this game may still hold its file.
fn clean_up_later(game: PathBuf) {
    if !game.join(DOWNLOAD_FOLDER).exists() {
        return;
    }
    std::thread::spawn(move || {
        for _ in 0..30 {
            if handoff::clean_up(&game) {
                info!("update: removed {}", game.join(DOWNLOAD_FOLDER).display());
                return;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        warn!("update: {} could not be removed", game.join(DOWNLOAD_FOLDER).display());
    });
}

/// The option, read from its CVar now and then; the check starts the first time it reads on.
fn read_option(
    script: Option<NonSend<UiScript>>,
    mut updater: ResMut<Updater>,
    mut notice: ResMut<UpdateNotice>,
    time: Res<Time>,
    mut next: Local<f32>,
    mut registered: Local<bool>,
) {
    let Some(script) = script else { return };
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + OPTION_EVERY;
    if !*registered {
        register_cvar(&script);
        *registered = true;
    }
    let v = script.eval::<Option<String>>(&format!("return GetCVar(\"{CVAR}\")")).ok().flatten();
    let on = on_from_value(v.as_deref());
    if notice.on != on {
        notice.on = on;
    }
    if on && !updater.checked {
        updater.checked = true;
        if let Some(config) = updater.config.clone() {
            start_check(config, updater.phase.clone(), updater.release.clone());
        }
    }
}

/// The check, on a thread: one GET of the feed; a newer release is offered, anything else says
/// nothing (the log keeps why).
fn start_check(config: UpdateConfig, phase: Arc<Mutex<Phase>>, found: Arc<Mutex<Option<Release>>>) {
    std::thread::spawn(move || {
        let ua = fetch::user_agent(&config.installed);
        match fetch::latest(&config.feed, &ua) {
            Ok(r) if release::is_newer(&config.installed, r.version) => {
                info!("update: {} is available", release::version_text(r.version));
                let v = r.version;
                *found.lock().unwrap() = Some(r);
                *phase.lock().unwrap() = Phase::Available(v);
            }
            Ok(r) => info!(
                "update: {} is the latest release; {} is installed",
                release::version_text(r.version),
                config.installed
            ),
            Err(e) => info!("update: no update offered: {e}"),
        }
    });
}

/// Update pressed: the download, then the hand-off, on a thread.
fn press_update(mut presses: MessageReader<MenuPress>, updater: Res<Updater>) {
    for MenuPress(action) in presses.read() {
        if *action != MenuAction::UpdateGame {
            continue;
        }
        let mut phase = updater.phase.lock().unwrap();
        let v = match &*phase {
            Phase::Available(v) | Phase::Failed(v, _) => *v,
            _ => continue,
        };
        let (Some(config), Some(r)) = (updater.config.clone(), updater.release.lock().unwrap().clone())
        else {
            continue;
        };
        let Some(install) = config.install.clone() else {
            *phase = Phase::Failed(
                v,
                "This game was not installed by its setup: update it by hand.".to_owned(),
            );
            continue;
        };
        *phase = Phase::Downloading(v);
        updater.progress.set(0, r.zip.size);
        let (shared, progress) = (updater.phase.clone(), updater.progress.clone());
        std::thread::spawn(move || {
            let ua = fetch::user_agent(&config.installed);
            let next = match fetch_setup(&r, &config.feed, &ua, &install.game, &progress) {
                Ok(setup) => match handoff::setup_command(&setup, &install.wow).spawn() {
                    Ok(_) => {
                        info!("update: the setup for {} started", release::version_text(v));
                        Phase::Restarting(v)
                    }
                    Err(e) => Phase::Failed(v, failure(&format!("the setup did not start ({e})"))),
                },
                Err(why) => Phase::Failed(v, failure(&why)),
            };
            if let Phase::Failed(_, why) = &next {
                warn!("update: {why}");
            }
            *shared.lock().unwrap() = next;
        });
    }
}

/// The threads' phase into the notice; once the setup runs, the game quits for it.
fn follow(updater: Res<Updater>, mut notice: ResMut<UpdateNotice>, mut quit: MessageWriter<AppExit>, mut quitting: Local<bool>) {
    let phase = updater.phase.lock().unwrap().clone();
    if matches!(phase, Phase::Restarting(_)) && !*quitting {
        *quitting = true;
        info!("update: the game closes for the setup");
        quit.write(AppExit::Success);
    }
    if notice.phase != phase {
        notice.phase = phase;
    }
}

/// The progress line, kept up to date in place (the screen is not drawn anew for it).
fn show_progress(updater: Res<Updater>, notice: Res<UpdateNotice>, mut lines: Query<&mut Text, With<UpdateProgressText>>) {
    if !matches!(notice.phase, Phase::Downloading(_)) {
        return;
    }
    let (done, total) = updater.progress.get();
    let line = progress_text(done, total);
    for mut text in &mut lines {
        if text.0 != line {
            text.0 = line.clone();
        }
    }
}

#[cfg(test)]
mod tests;
