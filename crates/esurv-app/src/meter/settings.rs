//! What the meter keeps between sessions: where the window sits, its size, the mode shown,
//! whether it is folded to its title bar and whether the minimap's meter button hid it, in
//! `meter.ron` in benilla's state folder.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::feed::MeterMode;

/// The file's name in [`benilla_app::config_dir`].
const FILE_NAME: &str = "meter.ron";

/// The smallest and largest window, in logical pixels: two rows to forty.
pub const MIN_SIZE: Vec2 = Vec2::new(160.0, 59.0);
pub const MAX_SIZE: Vec2 = Vec2::new(640.0, 743.0);

/// The meter window's kept state.
#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MeterSettings {
    /// The window's top-left corner in logical pixels from the game window's top-left; `None`
    /// until the player moves it, which places it at the bottom right above the action bars.
    pub position: Option<(f32, f32)>,
    /// Width and height in logical pixels.
    pub size: (f32, f32),
    pub mode: MeterMode,
    /// Folded to the title bar.
    pub collapsed: bool,
    /// Hidden by the minimap's meter button.
    pub hidden: bool,
}

impl Default for MeterSettings {
    fn default() -> Self {
        Self {
            position: None,
            // eight rows
            size: (250.0, 167.0),
            mode: MeterMode::DamageDone,
            collapsed: false,
            hidden: false,
        }
    }
}

impl MeterSettings {
    /// The size, held inside [`MIN_SIZE`] and [`MAX_SIZE`].
    pub fn clamped_size(&self) -> Vec2 {
        Vec2::new(self.size.0, self.size.1).clamp(MIN_SIZE, MAX_SIZE)
    }

    /// Reads settings written by [`Self::to_text`]; `None` for text that is not one.
    pub fn from_text(text: &str) -> Option<Self> {
        ron::from_str(text).ok()
    }

    /// The file's text.
    pub fn to_text(&self) -> String {
        let pretty = ron::ser::PrettyConfig::new().depth_limit(1);
        ron::ser::to_string_pretty(self, pretty).unwrap_or_default()
    }

    /// Reads the file at `path`: defaults when it is missing or unreadable.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_text(&text).unwrap_or_else(|| {
                warn!(
                    "meter: {} is not a meter layout; starting over",
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
pub fn settings_path() -> Option<PathBuf> {
    Some(benilla_app::config_dir()?.join(FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moved() -> MeterSettings {
        MeterSettings {
            position: Some((1210.5, 604.0)),
            size: (300.0, 220.0),
            mode: MeterMode::DamageTaken,
            collapsed: true,
            hidden: true,
        }
    }

    #[test]
    fn the_text_reads_back() {
        let settings = moved();
        assert_eq!(
            MeterSettings::from_text(&settings.to_text()),
            Some(settings)
        );
        let fresh = MeterSettings::default();
        assert_eq!(MeterSettings::from_text(&fresh.to_text()), Some(fresh));
    }

    #[test]
    fn the_file_reads_back() {
        let dir =
            std::env::temp_dir().join(format!("esurv-meter-test-{}", std::process::id()));
        let path = dir.join("meter.ron");
        assert_eq!(
            MeterSettings::load(&path),
            MeterSettings::default(),
            "no file yet"
        );
        let settings = moved();
        settings.save(&path).unwrap();
        assert_eq!(MeterSettings::load(&path), settings);
        std::fs::write(&path, "not a layout").unwrap();
        assert_eq!(MeterSettings::load(&path), MeterSettings::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_partial_file_fills_in_defaults() {
        let read = MeterSettings::from_text("(mode: DamageTaken)").unwrap();
        assert_eq!(read.mode, MeterMode::DamageTaken);
        assert_eq!(read.size, MeterSettings::default().size);
        assert_eq!(read.position, None);
        assert!(!read.hidden, "a file from before the minimap button shows the meter");
    }

    #[test]
    fn sizes_stay_in_bounds() {
        let tiny = MeterSettings {
            size: (10.0, 10.0),
            ..default()
        };
        assert_eq!(tiny.clamped_size(), MIN_SIZE);
        let huge = MeterSettings {
            size: (5000.0, 5000.0),
            ..default()
        };
        assert_eq!(huge.clamped_size(), MAX_SIZE);
    }
}
