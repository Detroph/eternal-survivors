//! Where the player moved the end panel, kept between sessions in `end_panel.ron` in benilla's
//! state folder, as the meter keeps its own window (`meter.ron`).

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The file's name in [`benilla_app::config_dir`].
const FILE_NAME: &str = "end_panel.ron";

/// The end panel's kept place.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelPlace {
    /// How far the player moved the panel from its default place (centred, near the top), in
    /// logical pixels; (0, 0) until it is moved.
    pub moved: (f32, f32),
}

impl PanelPlace {
    pub fn moved(&self) -> Vec2 {
        Vec2::new(self.moved.0, self.moved.1)
    }

    /// Reads text written by [`Self::to_text`]; `None` for text that is not one.
    pub fn from_text(text: &str) -> Option<Self> {
        ron::from_str::<Self>(text)
            .ok()
            .filter(|p| p.moved.0.is_finite() && p.moved.1.is_finite())
    }

    pub fn to_text(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::new()).unwrap_or_default()
    }

    /// Reads the file at `path`: the default place when it is missing or unreadable.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_text(&text).unwrap_or_else(|| {
                warn!(
                    "run end: {} is not a panel place; starting over",
                    path.display()
                );
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Writes the file at `path` whole: to a side file first, then renamed over the old one.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let side = path.with_extension("ron.new");
        std::fs::write(&side, self.to_text())?;
        std::fs::rename(&side, path)
    }
}

/// Where the file lives; `None` when benilla keeps no state this run.
pub fn place_path() -> Option<PathBuf> {
    Some(benilla_app::config_dir()?.join(FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_place_reads_back_from_its_file() {
        let dir =
            std::env::temp_dir().join(format!("esurv-end-panel-test-{}", std::process::id()));
        let path = dir.join(FILE_NAME);
        assert_eq!(
            PanelPlace::load(&path),
            PanelPlace::default(),
            "no file yet"
        );
        let place = PanelPlace {
            moved: (-312.5, 140.0),
        };
        place.save(&path).unwrap();
        assert_eq!(PanelPlace::load(&path), place);
        assert_eq!(PanelPlace::load(&path).moved(), Vec2::new(-312.5, 140.0));
        std::fs::write(&path, "somewhere").unwrap();
        assert_eq!(PanelPlace::load(&path), PanelPlace::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
