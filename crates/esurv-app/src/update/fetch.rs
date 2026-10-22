//! The updater's two requests: the release feed, and a release file with its progress. Plain
//! anonymous GETs over HTTPS (ureq on rustls, the certificates checked against the system's own
//! store), with a User-Agent of the game's name and version and nothing else about the player.

use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
use ureq::Agent;

use super::release::{parse_release, Release};

/// How long the feed may take in all.
const FEED_TIMEOUT: Duration = Duration::from_secs(20);
/// How long a connection may take to open, and a download may go without a byte.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const STALL_TIMEOUT: Duration = Duration::from_secs(30);
/// The most the feed's answer may be.
const FEED_LIMIT: u64 = 1 << 20;
/// The most a release file may be.
pub const DOWNLOAD_LIMIT: u64 = 1 << 30;

/// Where the release feed is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feed {
    pub url: String,
    /// Only HTTPS, for the feed and every file it points at (redirects too). A test feed named by
    /// `ESURV_UPDATE_FEED` may be plain HTTP.
    pub https_only: bool,
}

/// A download's progress, read by the menus while a thread downloads.
#[derive(Debug, Default)]
pub struct Progress {
    done: AtomicU64,
    /// The size the server said (0 when it said none).
    total: AtomicU64,
}

impl Progress {
    /// Bytes in, and the size expected (0: not known).
    pub fn get(&self) -> (u64, u64) {
        (self.done.load(Ordering::Relaxed), self.total.load(Ordering::Relaxed))
    }

    pub fn set(&self, done: u64, total: u64) {
        self.done.store(done, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
    }
}

/// The User-Agent the requests carry: the game's name and its version.
pub fn user_agent(version: &str) -> String {
    format!("Eternal Survivors/{}", version.trim())
}

fn agent(https_only: bool, total: Option<Duration>, user_agent: &str) -> Agent {
    let tls = TlsConfig::builder()
        .provider(TlsProvider::Rustls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    Agent::config_builder()
        .https_only(https_only)
        .tls_config(tls)
        .timeout_global(total)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_per_read(Some(STALL_TIMEOUT))
        .user_agent(user_agent)
        .build()
        .new_agent()
}

/// A request's failure in a few words.
fn why(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(code) => format!("the server answered {code}"),
        ureq::Error::Timeout(_) => "the server took too long".to_owned(),
        ureq::Error::HostNotFound => "no connection".to_owned(),
        ureq::Error::Io(e) => format!("the connection failed ({})", e.kind()),
        e => e.to_string(),
    }
}

/// The latest release the feed names.
pub fn latest(feed: &Feed, user_agent: &str) -> Result<Release, String> {
    let agent = agent(feed.https_only, Some(FEED_TIMEOUT), user_agent);
    let mut answer = agent
        .get(&feed.url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(why)?;
    let text = answer
        .body_mut()
        .with_config()
        .limit(FEED_LIMIT)
        .read_to_string()
        .map_err(why)?;
    parse_release(&text)
}

/// A release file's bytes, the progress kept in `progress` as they come.
pub fn download(
    url: &str,
    https_only: bool,
    user_agent: &str,
    progress: &Progress,
) -> Result<Vec<u8>, String> {
    let agent = agent(https_only, None, user_agent);
    let answer = agent.get(url).call().map_err(why)?;
    let total = answer.body().content_length().unwrap_or(0);
    if total > DOWNLOAD_LIMIT {
        return Err("the file is larger than any release".to_owned());
    }
    progress.set(0, total);
    let mut reader = answer.into_body().into_reader();
    let mut bytes = Vec::with_capacity(total as usize);
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("the download broke off ({})", e.kind()))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.len() as u64 > DOWNLOAD_LIMIT {
            return Err("the file is larger than any release".to_owned());
        }
        progress.set(bytes.len() as u64, total);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_agent_names_the_game_and_its_version_only() {
        assert_eq!(user_agent("v0.4.0\r\n"), "Eternal Survivors/v0.4.0");
    }

    /// The feed is HTTPS only: a plain-HTTP address is refused before any connection.
    #[test]
    fn the_real_feed_refuses_plain_http() {
        let feed = Feed {
            url: "http://127.0.0.1:9/feed".into(),
            https_only: true,
        };
        assert!(latest(&feed, "test").is_err());
    }

    /// The real feed, read once (`cargo test -p esurv-app --release real_feed -- --ignored`):
    /// read-only, it needs the internet and is left out of the usual run.
    #[test]
    #[ignore]
    fn the_real_feed_answers() {
        let feed = Feed {
            url: super::super::release::FEED.into(),
            https_only: true,
        };
        match latest(&feed, &user_agent("v0.0.0")) {
            Ok(r) => println!("the real feed offers {:?}", r),
            // a release without its checksum is not offered, and says why
            Err(e) => println!("the real feed offers nothing: {e}"),
        }
    }
}
