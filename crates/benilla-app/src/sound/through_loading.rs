//! A sound that plays on through a loading screen: a crate's seam (no 1.12 path plays one), for a
//! sound that marks the way into a load, such as a teleport as a run starts. The kit plays 2D on
//! SFX by name, held here rather than in the pump's channels, so the world's leave
//! (`kit::stop_all_channels`) does not cut it, and the loading cover's hold
//! ([`super::SoundConfig::world_hold`]) does not refuse it if the cover is already up.

use bevy::prelude::*;

use benilla_assets::WorldAssets;

use super::kit::{self, SoundKits};
use super::{mixer, SoundConfig, SoundOutput};

/// Play this `SoundEntries` kit (by name) once, on through the world's leave and the loading
/// screen.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundThroughLoading(pub &'static str);

/// The sounds playing on through a load, dropped once stopped.
#[derive(Default)]
struct HeldSounds(Vec<mixer::StaticSoundHandle>);

/// Plays the asked sounds and lets go of those that ended.
fn play_sounds_through_loading(
    mut msgs: MessageReader<SoundThroughLoading>,
    kits: Option<ResMut<SoundKits>>,
    assets: Option<Res<WorldAssets>>,
    mut out: NonSendMut<SoundOutput>,
    config: Res<SoundConfig>,
    mut held: NonSendMut<HeldSounds>,
) {
    held.0
        .retain(|h| h.state() != kira::sound::PlaybackState::Stopped);
    let (Some(mut kits), Some(assets)) = (kits, assets) else {
        msgs.clear();
        return;
    };
    for SoundThroughLoading(name) in msgs.read() {
        match kit::play_kit_held(&mut kits, &assets, &mut out, &config, name) {
            Ok(Some(h)) => {
                info!("sound: {name}, on through the loading screen");
                held.0.push(h);
            }
            Ok(None) => {}
            Err(e) => debug!("sound(through loading): {name} — {e:#}"),
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<SoundThroughLoading>()
        .insert_non_send_resource(HeldSounds::default())
        .add_systems(Update, play_sounds_through_loading);
}
