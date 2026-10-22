//! Dev builds only (feature `watch`): the content folder, subfolders included, is watched, and a
//! change to its `.ron` files loads it again ([`LiveContent::reload`]) once the changes settle.

use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context as _, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{Content, LiveContent};

/// How long the folder must stay still before it is loaded again: an editor's save, or a tool
/// writing several files, makes a burst of changes, and the folder loads once after it.
pub const SETTLE: Duration = Duration::from_millis(500);

/// A content folder being watched; dropping it stops the watch and its thread.
pub struct ContentWatcher {
    // dropped first: its event sender goes with it, and the thread sees the channel close
    _watcher: RecommendedWatcher,
    _thread: JoinHandle<()>,
}

/// Watches `live`'s folder and reloads it after each burst of changes, handing every outcome to
/// `on_reload` (on the watcher's thread): the new content, or the error that kept the old.
pub fn watch(live: Arc<LiveContent>, mut on_reload: impl FnMut(&Result<Arc<Content>>) + Send + 'static) -> Result<ContentWatcher> {
    let dir = live.dir().context("this content has no folder to watch")?.to_path_buf();
    let (events, changes) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = notify::recommended_watcher(events).context("starting the content watcher")?;
    watcher.watch(&dir, RecursiveMode::Recursive).with_context(|| format!("watching {}", dir.display()))?;
    let thread = std::thread::Builder::new().name("content-watch".into()).spawn(move || loop {
        match changes.recv() {
            Ok(Ok(event)) if touches_content(&event) => {}
            Ok(_) => continue,
            Err(_) => return,
        }
        // the burst settles: no change for `SETTLE`
        loop {
            match changes.recv_timeout(SETTLE) {
                Ok(_) => continue,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        on_reload(&live.reload());
    })?;
    Ok(ContentWatcher { _watcher: watcher, _thread: thread })
}

/// Whether `event` changed a content file: a `.ron` file made, changed, renamed or removed (an
/// editor's backup and swap files do not count).
fn touches_content(event: &Event) -> bool {
    !matches!(event.kind, EventKind::Access(_)) && event.paths.iter().any(|p| is_content_file(p))
}

fn is_content_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ron"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{content_folder_with_zones, set_warrior_base_health};
    use std::time::Instant;

    #[test]
    fn only_content_files_count() {
        assert!(is_content_file(Path::new("content/zones/elwynn.ron")));
        assert!(is_content_file(Path::new("classes.RON")));
        assert!(!is_content_file(Path::new("content/classes.ron~")));
        assert!(!is_content_file(Path::new("content/.classes.ron.swp")));
    }

    #[test]
    fn a_saved_file_reloads_the_folder() {
        let from = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content"));
        let dir = content_folder_with_zones(from, "watch", &[]).unwrap();
        let live = Arc::new(LiveContent::load(&dir).unwrap());
        let (told, outcomes) = mpsc::channel();
        let watcher = watch(live.clone(), move |r| {
            let _ = told.send(r.as_ref().map(|c| c.classes[0].base_health).map_err(|e| format!("{e:#}")));
        })
        .unwrap();
        // the watch is up once `watch` returns; a short wait still spares slow file systems
        std::thread::sleep(Duration::from_millis(100));
        set_warrior_base_health(&dir, 777.0).unwrap();
        let start = Instant::now();
        let outcome = outcomes.recv_timeout(Duration::from_secs(10)).expect("a reload within 10 s");
        assert_eq!(outcome, Ok(777.0));
        assert!(start.elapsed() >= SETTLE, "the reload waits for the changes to settle");
        assert_eq!(live.current().classes[0].base_health, 777.0);
        drop(watcher);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
