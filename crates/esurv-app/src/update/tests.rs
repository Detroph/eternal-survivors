//! The updater against a fake release served by a local in-process HTTP server (no internet):
//! the feed, a redirect to the file store as GitHub's downloads make, the archive and its
//! checksum; and the menus' notice and the systems' hand-over.

use std::io::{Read, Write};
use std::net::TcpListener;

use super::*;
use esurv_setup::{Entry, GAME_EXE, SETUP_EXE, VERSION_FILE};

/// One answer of the fake server: its status line, extra headers and body.
#[derive(Clone)]
struct Answer {
    status: &'static str,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn ok(body: impl Into<Vec<u8>>) -> Answer {
    Answer {
        status: "200 OK",
        headers: vec![],
        body: body.into(),
    }
}

/// Serves `routes` (path, answer) on 127.0.0.1 from a thread, for the rest of the test run; its
/// base URL. A path not routed answers 404.
fn serve(routes: impl Fn(&str) -> Vec<(String, Answer)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let routes = routes(&base);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = Vec::new();
            let mut buf = [0u8; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => request.extend_from_slice(&buf[..n]),
                }
            }
            let text = String::from_utf8_lossy(&request);
            let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let answer = routes
                .iter()
                .find(|(p, _)| *p == path)
                .map(|(_, a)| a.clone())
                .unwrap_or(Answer {
                    status: "404 Not Found",
                    headers: vec![],
                    body: b"not found".to_vec(),
                });
            let mut head = format!(
                "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                answer.status,
                answer.body.len()
            );
            for (k, v) in &answer.headers {
                head += &format!("{k}: {v}\r\n");
            }
            head += "\r\n";
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&answer.body);
        }
    });
    base
}

/// A setup as the release builds it: a stub with the game's files appended, `VERSION.txt` among
/// them.
fn fake_setup(version: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let files = [
        Entry { path: VERSION_FILE.into(), bytes: version.as_bytes().to_vec() },
        Entry { path: GAME_EXE.into(), bytes: b"MZ the game".to_vec() },
    ];
    esurv_setup::pack(b"MZ the setup's own code", &files, &mut out).unwrap();
    out
}

/// A release archive holding `setup`, as the release archive script packs it.
fn fake_zip(setup: &[u8]) -> Vec<u8> {
    unzip::test_zip(&[
        ("README.txt", b"Eternal Survivors", true),
        (SETUP_EXE, setup, true),
        ("LICENSE-MIT.txt", b"MIT", false),
    ])
}

/// A fake release `tag` whose archive is `zip` and whose checksum file says `sum`: the feed at
/// `/feed`, each file behind a redirect to `/store/...`. The test feed.
fn release_server(tag: &'static str, zip: Vec<u8>, sum: String) -> Feed {
    let base = serve(move |base| {
        let v = release::version_of_tag(tag).unwrap();
        let (zip_name, sum_name) = (release::archive_name(v), release::checksum_name(v));
        let json = release::test_feed_json(tag, &format!("{base}/download"), &[&zip_name, &sum_name]);
        let redirect = |to: String| Answer {
            status: "302 Found",
            headers: vec![("Location".into(), to)],
            body: vec![],
        };
        vec![
            ("/feed".into(), ok(json)),
            (format!("/download/{zip_name}"), redirect(format!("{base}/store/{zip_name}"))),
            (format!("/download/{sum_name}"), redirect(format!("{base}/store/{sum_name}"))),
            (format!("/store/{zip_name}"), ok(zip.clone())),
            (format!("/store/{sum_name}"), ok(sum.clone())),
        ]
    });
    Feed {
        url: format!("{base}/feed"),
        https_only: false,
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("esurv-update-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_newer_release_is_found_downloaded_checked_and_its_setup_staged() {
    let setup = fake_setup("v9.9.0");
    let zip = fake_zip(&setup);
    let sum = format!("{}  eternal-survivors-v9.9.0.zip\n", checksum::sha256_hex(&zip));
    let feed = release_server("esurv-v9.9.0", zip.clone(), sum);

    let r = fetch::latest(&feed, &fetch::user_agent("v0.4.0")).unwrap();
    assert_eq!(r.version, (9, 9, 0));
    assert!(release::is_newer("v0.4.0", r.version));
    assert!(!release::is_newer("v9.9.0", r.version), "an install of that version is not offered it");

    let game = scratch("staged");
    let progress = Progress::default();
    let staged = fetch_setup(&r, &feed, "test", &game, &progress).unwrap();
    assert_eq!(staged, game.join(DOWNLOAD_FOLDER).join(SETUP_EXE));
    assert_eq!(std::fs::read(&staged).unwrap(), setup);
    assert_eq!(progress.get(), (zip.len() as u64, zip.len() as u64), "the progress reached the end");
    std::fs::remove_dir_all(&game).ok();
}

#[test]
fn an_archive_that_does_not_match_its_checksum_changes_nothing() {
    let zip = fake_zip(&fake_setup("v9.9.0"));
    let sum = format!("{}  eternal-survivors-v9.9.0.zip\n", checksum::sha256_hex(b"another file"));
    let feed = release_server("esurv-v9.9.0", zip, sum);
    let r = fetch::latest(&feed, "test").unwrap();
    let game = scratch("mismatch");
    let why = fetch_setup(&r, &feed, "test", &game, &Progress::default()).unwrap_err();
    assert!(why.contains("does not match its checksum"), "{why}");
    assert!(!game.join(DOWNLOAD_FOLDER).exists(), "nothing was written");
    std::fs::remove_dir_all(&game).ok();
}

#[test]
fn a_setup_that_carries_another_version_is_refused() {
    let zip = fake_zip(&fake_setup("v9.8.0"));
    let sum = format!("{}  eternal-survivors-v9.9.0.zip\n", checksum::sha256_hex(&zip));
    let feed = release_server("esurv-v9.9.0", zip, sum);
    let r = fetch::latest(&feed, "test").unwrap();
    let game = scratch("other-version");
    let why = fetch_setup(&r, &feed, "test", &game, &Progress::default()).unwrap_err();
    assert!(why.contains("not v9.9.0"), "{why}");
    assert!(!game.join(DOWNLOAD_FOLDER).exists());
    std::fs::remove_dir_all(&game).ok();
}

/// Rate-limited, missing or unreachable: the check fails quietly (an `Err` the log keeps).
#[test]
fn a_feed_that_fails_offers_nothing() {
    let base = serve(|_| {
        vec![(
            "/feed".into(),
            Answer {
                status: "403 Forbidden",
                headers: vec![],
                body: br#"{"message": "API rate limit exceeded"}"#.to_vec(),
            },
        )]
    });
    let feed = |path: &str| Feed {
        url: format!("{base}{path}"),
        https_only: false,
    };
    assert!(fetch::latest(&feed("/feed"), "test").unwrap_err().contains("403"));
    assert!(fetch::latest(&feed("/missing"), "test").unwrap_err().contains("404"));
    // a port nothing listens on
    let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let gone = Feed {
        url: format!("http://{closed}/feed"),
        https_only: false,
    };
    assert!(fetch::latest(&gone, "test").is_err());
}

#[test]
fn player_builds_ask_the_real_feed_dev_builds_only_a_test_feed() {
    let real = choose_feed(true, None, Some("v0.4.0".into()), "0.4.0").unwrap();
    assert_eq!(real.0, Feed { url: FEED.into(), https_only: true });
    assert_eq!(real.1, "v0.4.0");
    assert_eq!(choose_feed(true, None, None, "0.4.0"), None, "no VERSION.txt: no check");
    assert_eq!(choose_feed(false, None, Some("v0.4.0".into()), "0.4.0"), None, "a dev build");
    let test = choose_feed(false, Some("http://127.0.0.1:8000/feed".into()), None, "0.4.0").unwrap();
    assert_eq!(test.0, Feed { url: "http://127.0.0.1:8000/feed".into(), https_only: false });
    assert_eq!(test.1, "v0.4.0", "the crate version stands in for VERSION.txt");
    assert_eq!(choose_feed(false, Some(" ".into()), None, "0.4.0"), None, "an empty switch");
}

#[test]
fn the_option_is_on_unless_it_reads_zero() {
    assert!(on_from_value(None));
    assert!(on_from_value(Some("1")));
    assert!(!on_from_value(Some("0")));
    assert!(!on_from_value(Some(" 0 ")));
}

/// The warband screen's notice: the offer and its Update button, the download with its progress
/// (Update dimmed), a failure with Update to try again, and the restart; the option off hides an
/// offer but not an update under way.
#[test]
fn the_notice_offers_the_update_and_follows_it() {
    let v = (0, 5, 0);
    let offer = notice_view(&Phase::Available(v), true).unwrap();
    assert_eq!(offer.line, "v0.5.0 is available");
    assert_eq!(offer.button, Some(true));
    assert!(!offer.progress && !offer.failed);
    assert_eq!(notice_view(&Phase::Available(v), false), None, "the option off");
    assert_eq!(notice_view(&Phase::Quiet, true), None);

    let down = notice_view(&Phase::Downloading(v), false).unwrap();
    assert_eq!(down.line, "Downloading v0.5.0");
    assert_eq!(down.button, Some(false));
    assert!(down.progress);

    let failed = notice_view(&Phase::Failed(v, failure("no connection")), true).unwrap();
    assert_eq!(failed.line, "The update failed: no connection. Nothing was changed.");
    assert!(failed.failed);
    assert_eq!(failed.button, Some(true), "Update tries again");

    let restart = notice_view(&Phase::Restarting(v), true).unwrap();
    assert_eq!(restart.line, "Updating to v0.5.0: the game restarts");
    assert_eq!(restart.button, None);

    assert_eq!(progress_text(512 * 1024, 2 * 1024 * 1024), "25%  (0.5 of 2.0 MB)");
    assert_eq!(progress_text(3 * 1024 * 1024, 0), "3.0 MB");
}

/// A bare app with the updater's systems and `config`, the phase and release given.
fn app_with(config: Option<UpdateConfig>, phase: Phase, release: Option<Release>) -> App {
    let mut app = App::new();
    app.add_message::<MenuPress>()
        .add_message::<AppExit>()
        .init_resource::<Time>();
    register(&mut app, config);
    {
        let updater = app.world().resource::<Updater>();
        *updater.phase.lock().unwrap() = phase;
        *updater.release.lock().unwrap() = release;
    }
    app
}

fn fake_release(base: &str) -> Release {
    let v = (9, 9, 0);
    let asset = |name: String| release::Asset { url: format!("{base}/download/{name}"), name, size: 0 };
    Release { version: v, zip: asset(release::archive_name(v)), sha256: asset(release::checksum_name(v)) }
}

/// Update on a game the setup did not install says to update by hand, and changes nothing.
#[test]
fn update_on_a_game_the_setup_did_not_install_says_to_update_by_hand() {
    let config = UpdateConfig {
        feed: Feed { url: "http://127.0.0.1:9/feed".into(), https_only: false },
        installed: "v0.4.0".into(),
        install: None,
    };
    let mut app = app_with(Some(config), Phase::Available((9, 9, 0)), Some(fake_release("http://127.0.0.1:9")));
    app.world_mut().write_message(MenuPress(MenuAction::UpdateGame));
    app.update();
    let notice = app.world().resource::<UpdateNotice>();
    assert!(
        matches!(&notice.phase, Phase::Failed(_, why) if why.contains("update it by hand")),
        "{:?}",
        notice.phase
    );
}

/// The whole path from the press: the download, the checks, the staged setup, and the hand-off,
/// which fails here (the staged setup is no program) and says so, the game left running.
#[test]
fn update_downloads_stages_and_hands_off_or_says_why_not() {
    let setup = fake_setup("v9.9.0");
    let zip = fake_zip(&setup);
    let sum = format!("{}  eternal-survivors-v9.9.0.zip\n", checksum::sha256_hex(&zip));
    let feed = release_server("esurv-v9.9.0", zip, sum);
    let r = fetch::latest(&feed, "test").unwrap();
    let wow = scratch("handoff");
    let game = wow.join(esurv_setup::GAME_FOLDER);
    std::fs::create_dir_all(&game).unwrap();
    let config = UpdateConfig {
        feed,
        installed: "v0.4.0".into(),
        install: Some(Install { game: game.clone(), wow: wow.clone() }),
    };
    let mut app = app_with(Some(config), Phase::Available(r.version), Some(r));
    let line = app.world_mut().spawn((Text::new(""), UpdateProgressText)).id();
    app.world_mut().write_message(MenuPress(MenuAction::UpdateGame));
    app.update();
    assert_eq!(app.world().resource::<UpdateNotice>().phase, Phase::Downloading((9, 9, 0)));
    let start = std::time::Instant::now();
    loop {
        app.update();
        let phase = app.world().resource::<UpdateNotice>().phase.clone();
        if !matches!(phase, Phase::Downloading(_)) {
            assert!(
                matches!(&phase, Phase::Failed(_, why) if why.contains("the setup did not start")),
                "{phase:?}"
            );
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "the download never ended");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(std::fs::read(game.join(DOWNLOAD_FOLDER).join(SETUP_EXE)).unwrap(), setup);
    assert!(app.world().get::<Text>(line).is_some());
    let exits = app.world().resource::<Messages<AppExit>>();
    assert!(exits.is_empty(), "the game keeps running when the setup does not start");
    std::fs::remove_dir_all(&wow).ok();
}

/// Once the setup runs, the game quits for it; the progress line follows a download.
#[test]
fn the_game_quits_once_the_setup_runs_and_the_progress_line_follows() {
    let mut app = app_with(None, Phase::Downloading((9, 9, 0)), None);
    let line = app.world_mut().spawn((Text::new(""), UpdateProgressText)).id();
    app.world().resource::<Updater>().progress.set(1024 * 1024, 4 * 1024 * 1024);
    app.update();
    assert_eq!(app.world().get::<Text>(line).unwrap().0, "25%  (1.0 of 4.0 MB)");
    assert!(app.world().resource::<Messages<AppExit>>().is_empty());

    *app.world().resource::<Updater>().phase.lock().unwrap() = Phase::Restarting((9, 9, 0));
    app.update();
    assert!(!app.world().resource::<Messages<AppExit>>().is_empty(), "the game quits");
}
