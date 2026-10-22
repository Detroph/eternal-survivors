//! The content the game hands out, which can be loaded again while the game runs.
//!
//! Every reader takes the content as it is now ([`LiveContent::current`]) and keeps that `Arc` for
//! as long as it needs one consistent set: a run takes it once as it starts and plays to its end
//! with it, whatever is loaded meanwhile; a screen before a run takes it as it draws. A reload
//! ([`LiveContent::reload`]) swaps the content only when the folder loads in full, so a broken file
//! leaves the game on what it had. Dev builds reload as the folder changes (`watch`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use anyhow::{anyhow, Result};

use crate::{ClientNames, Content};

/// The content now, and how its last reload went.
#[derive(Debug)]
pub struct LiveContent {
    /// The folder a reload reads; `None` for content given whole (tests), which never reloads.
    dir: Option<PathBuf>,
    /// The client's names a load shows pointers as; a reload shows them the same way.
    names: Arc<ClientNames>,
    current: RwLock<Arc<Content>>,
    /// How many reloads swapped the content in.
    version: AtomicU64,
    /// The last reload tried, success or not.
    last: Mutex<Option<ReloadOutcome>>,
}

/// How a reload went: its number among the reloads tried (from 1), and the error if the folder did
/// not load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReloadOutcome {
    pub attempt: u64,
    pub error: Option<String>,
}

impl LiveContent {
    /// `content`, as loaded from `dir`, which reloads read again.
    pub fn new(content: Content, dir: impl Into<PathBuf>) -> Self {
        Self::with_dir(Arc::new(content), Some(dir.into()), Arc::new(ClientNames::none()))
    }

    /// Loads the content folder `dir` with no client tables (pointers read as placeholders).
    pub fn load(dir: impl Into<PathBuf>) -> Result<Self> {
        Self::load_with_names(dir, Arc::new(ClientNames::none()))
    }

    /// Loads the content folder `dir`, its pointers shown as `names` has them, now and on every
    /// reload.
    pub fn load_with_names(dir: impl Into<PathBuf>, names: Arc<ClientNames>) -> Result<Self> {
        let dir = dir.into();
        let content = Content::load_with_names(&dir, &names)?;
        Ok(Self::with_dir(Arc::new(content), Some(dir), names))
    }

    /// `content` given whole, with no folder: it never reloads.
    pub fn fixed(content: Arc<Content>) -> Self {
        Self::with_dir(content, None, Arc::new(ClientNames::none()))
    }

    fn with_dir(content: Arc<Content>, dir: Option<PathBuf>, names: Arc<ClientNames>) -> Self {
        Self { dir, names, current: RwLock::new(content), version: AtomicU64::new(0), last: Mutex::new(None) }
    }

    /// The content as it is now; keep the `Arc` for as long as one consistent set is needed.
    pub fn current(&self) -> Arc<Content> {
        self.current.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// How many reloads swapped the content in (0 before the first).
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    /// The folder reloads read.
    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// How the last reload went, `None` before any.
    pub fn last_outcome(&self) -> Option<ReloadOutcome> {
        self.last.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Loads the folder again: on success the new content replaces the old for every reader that
    /// asks from now on, and is returned; on failure the old content stays and the error is
    /// returned. Either way the outcome is kept for [`LiveContent::last_outcome`].
    pub fn reload(&self) -> Result<Arc<Content>> {
        let loaded = match &self.dir {
            Some(dir) => Content::load_with_names(dir, &self.names).map(Arc::new),
            None => Err(anyhow!("this content has no folder to load again")),
        };
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        let attempt = last.as_ref().map_or(1, |o| o.attempt + 1);
        match loaded {
            Ok(content) => {
                *self.current.write().unwrap_or_else(|e| e.into_inner()) = content.clone();
                self.version.fetch_add(1, Ordering::AcqRel);
                *last = Some(ReloadOutcome { attempt, error: None });
                Ok(content)
            }
            Err(e) => {
                *last = Some(ReloadOutcome { attempt, error: Some(format!("{e:#}")) });
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::content_folder_with_zones;

    fn content_copy(test: &str) -> PathBuf {
        let from = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content"));
        content_folder_with_zones(from, test, &[]).unwrap()
    }

    fn warrior_health(c: &Content) -> f32 {
        c.classes.iter().find(|c| c.id == "warrior").unwrap().base_health
    }

    fn set_warrior_health(dir: &Path, health: f32) {
        crate::testing::set_warrior_base_health(dir, health).unwrap();
    }

    #[test]
    fn a_reload_swaps_in_the_new_content() {
        let dir = content_copy("live-swap");
        let live = LiveContent::load(&dir).unwrap();
        let before = live.current();
        set_warrior_health(&dir, 999.0);
        let after = live.reload().unwrap();
        assert_eq!(warrior_health(&after), 999.0);
        assert_eq!(warrior_health(&live.current()), 999.0, "readers from now on get the new content");
        assert_ne!(warrior_health(&before), 999.0, "a reader keeping the old content keeps its numbers");
        assert_eq!(live.version(), 1);
        assert_eq!(live.last_outcome(), Some(ReloadOutcome { attempt: 1, error: None }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_reload_keeps_the_old_content() {
        let dir = content_copy("live-fail");
        let live = LiveContent::load(&dir).unwrap();
        let before = live.current();
        let classes = dir.join("classes.ron");
        let good = std::fs::read_to_string(&classes).unwrap();
        std::fs::write(&classes, "([ (id: \"warrior\", broken").unwrap();
        assert!(live.reload().is_err());
        assert!(Arc::ptr_eq(&before, &live.current()), "the old content stays");
        assert_eq!(live.version(), 0);
        let outcome = live.last_outcome().unwrap();
        assert_eq!(outcome.attempt, 1);
        assert!(outcome.error.unwrap().contains("classes.ron"), "the error names the file");
        // mended, the next reload goes through
        std::fs::write(&classes, good).unwrap();
        set_warrior_health(&dir, 321.0);
        assert_eq!(warrior_health(&live.reload().unwrap()), 321.0);
        assert_eq!(live.last_outcome(), Some(ReloadOutcome { attempt: 2, error: None }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_reload_shows_the_clients_names_the_same_way() {
        let dir = content_copy("live-names");
        let names = Arc::new(crate::testing::stand_in_client_names(&dir).unwrap());
        let live = LiveContent::load_with_names(&dir, names).unwrap();
        let shown = |c: &Content| {
            let warrior = c.classes.iter().find(|c| c.id == "warrior").unwrap();
            (warrior.name.clone(), c.zone("elwynn").unwrap().name.clone())
        };
        let before = shown(&live.current());
        assert_eq!(before, ("Client Class 1".to_owned(), "Client Area 12".to_owned()));
        set_warrior_health(&dir, 999.0);
        let after = live.reload().unwrap();
        assert_eq!(warrior_health(&after), 999.0);
        assert_eq!(shown(&after), before, "a reload reads the names from the same tables");
        // with no tables, the pointers show as placeholders
        assert_eq!(shown(&Content::load(&dir).unwrap()), ("Class 1".to_owned(), "Area 12".to_owned()));
        // tables that lack a row the content points at do not load
        let spells = std::collections::HashMap::from([(1, "Only One".to_owned())]);
        let partial = ClientNames::from_tables(spells, Default::default(), Default::default(), Default::default());
        let error = format!("{:#}", Content::load_with_names(&dir, &partial).unwrap_err());
        assert!(error.contains("the client has no AreaTable row 12"), "{error}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn content_given_whole_never_reloads() {
        let dir = content_copy("live-fixed");
        let content = Arc::new(Content::load(&dir).unwrap());
        let live = LiveContent::fixed(content.clone());
        assert!(live.reload().is_err());
        assert!(Arc::ptr_eq(&content, &live.current()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
