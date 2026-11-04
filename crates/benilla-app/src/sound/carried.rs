//! Sounds a crate on top of benilla carries itself, not in the client's data: a crate's seam (no
//! 1.12 path plays one).
//!
//! - [`LoadingSound`]: played from its start once for every loading screen with art, for its
//!   whole stretch whenever the screen goes, then faded out; a screen right after another does not
//!   cut the last one's short. The plain black cover of the world's leave does not play it.
//! - [`AmbientSprinkle`]: in the world, now and then, a stretch of a sound under the zone's
//!   ambience, faded out at its end; never while the loading screen is up.
//!
//! Each is decoded once, the first frame the mixer is up, and played 2D at its own volume times
//! its category's slider.

use bevy::prelude::*;

use crate::char_select::ClientState;

use super::kit::SoundCategory;
use super::{mixer, SoundConfig, SoundOutput};

/// A sound file's bytes and the volume it is played at, 0..1, before its slider.
#[derive(Clone, Copy, Debug)]
pub struct CarriedSound {
    pub bytes: &'static [u8],
    pub volume: f32,
}

/// Played on SFX as the loading screen rises: `play_secs` from its start, then faded out over
/// `fade_secs`.
#[derive(Resource, Clone, Copy, Debug)]
pub struct LoadingSound {
    pub sound: CarriedSound,
    pub play_secs: f32,
    pub fade_secs: f32,
}

/// Played on Ambience in the world: every `every_secs` (a uniform draw in the range), a stretch
/// `play_secs` long (the same), from its start, faded out over `fade_secs`.
#[derive(Resource, Clone, Copy, Debug)]
pub struct AmbientSprinkle {
    pub sound: CarriedSound,
    pub every_secs: (f32, f32),
    pub play_secs: (f32, f32),
    pub fade_secs: f32,
}

/// A carried sound decoded, and its playing stretches with when each is faded out.
#[derive(Default)]
struct Voice {
    data: Option<mixer::StaticSoundData>,
    /// Decoding failed: not tried again.
    failed: bool,
    playing: Vec<(mixer::StaticSoundHandle, f64)>,
}

impl Voice {
    /// The decoded sound, decoding it the first time.
    fn data(&mut self, sound: &CarriedSound, what: &str) -> Option<mixer::StaticSoundData> {
        if self.data.is_none() && !self.failed {
            match mixer::sfx_from_bytes(sound.bytes.to_vec()) {
                Ok(d) => self.data = Some(d),
                Err(e) => {
                    warn!("sound({what}): {e:#}");
                    self.failed = true;
                }
            }
        }
        self.data.clone()
    }

    /// Starts it from its start at `amp`, to be faded out at `until`, beside any still playing.
    fn start(&mut self, out: &mut SoundOutput, data: mixer::StaticSoundData, amp: f32, until: f64) {
        let Some(m) = out.mixer.as_mut() else {
            return;
        };
        match m.play_2d(data.volume(mixer::amp_to_db(amp))) {
            Ok(h) => self.playing.push((h, until)),
            Err(e) => debug!("sound(carried): {e:#}"),
        }
    }

    /// Fades each stretch out once its time is up (only the clock: never the screen), and lets go
    /// of those stopped.
    fn tend(&mut self, now: f64, fade: u64) {
        self.playing
            .retain(|(h, _)| h.state() != kira::sound::PlaybackState::Stopped);
        for (h, until) in &mut self.playing {
            if now >= *until {
                h.stop(mixer::fade(fade));
                *until = f64::INFINITY;
            }
        }
    }
}

/// The loading screens the loading sound has played for.
#[derive(Default)]
struct LoadingTicks {
    heard: u32,
}

impl LoadingTicks {
    /// With `shown` screens risen so far, at `now`: when a screen has not had its sound yet, the
    /// time its stretch of `play_secs` ends. It is had once the mixer is up to play it (until then
    /// it waits); several risen since the last look play once.
    fn take(&mut self, shown: u32, mixer_up: bool, now: f64, play_secs: f32) -> Option<f64> {
        if shown == self.heard || !mixer_up {
            return None;
        }
        self.heard = shown;
        Some(now + f64::from(play_secs))
    }
}

fn millis(secs: f32) -> u64 {
    (secs.max(0.0) * 1000.0) as u64
}

/// The loading sound, once for each loading screen with art, for its stretch whenever the screen
/// goes.
fn play_the_loading_sound(
    sound: Option<Res<LoadingSound>>,
    screen: Res<crate::loading_screen::LoadingScreen>,
    config: Res<SoundConfig>,
    time: Res<Time<bevy::time::Real>>,
    mut out: NonSendMut<SoundOutput>,
    mut voice: Local<Voice>,
    mut ticks: Local<LoadingTicks>,
) {
    let Some(sound) = sound else {
        return;
    };
    let now = time.elapsed_secs_f64();
    let fade = millis(sound.fade_secs);
    voice.tend(now, fade);
    let Some(until) = ticks.take(screen.shown_screens(), out.mixer.is_some(), now, sound.play_secs)
    else {
        return;
    };
    let Some(data) = voice.data(&sound.sound, "loading") else {
        return;
    };
    let amp = sound.sound.volume * config.category_amp(SoundCategory::Sfx);
    voice.start(&mut out, data, amp, until);
    info!("sound: the loading sound, {} s", sound.play_secs);
}

/// When the next sprinkle plays, drawn from its range.
#[derive(Default)]
struct Sprinkling {
    next_at: Option<f64>,
    rng: u32,
}

impl Sprinkling {
    /// A uniform draw in `(lo, hi)`.
    fn draw(&mut self, (lo, hi): (f32, f32)) -> f64 {
        // xorshift32, seeded once from the clock
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        let unit = f64::from(x) / f64::from(u32::MAX);
        f64::from(lo) + unit * f64::from((hi - lo).max(0.0))
    }
}

/// The ambient sprinkle, in the world with the loading screen down.
fn play_the_ambient_sprinkle(
    sound: Option<Res<AmbientSprinkle>>,
    state: Res<State<ClientState>>,
    config: Res<SoundConfig>,
    time: Res<Time<bevy::time::Real>>,
    mut out: NonSendMut<SoundOutput>,
    mut voice: Local<Voice>,
    mut sprinkling: Local<Sprinkling>,
) {
    let Some(sound) = sound else {
        return;
    };
    let now = time.elapsed_secs_f64();
    let fade = millis(sound.fade_secs);
    voice.tend(now, fade);
    if *state.get() != ClientState::InWorld || config.world_hold || out.mixer.is_none() {
        return;
    }
    if sprinkling.rng == 0 {
        sprinkling.rng = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.subsec_nanos()))
            | 1;
    }
    let next_at = match sprinkling.next_at {
        Some(t) => t,
        None => {
            let t = now + sprinkling.draw(sound.every_secs);
            sprinkling.next_at = Some(t);
            t
        }
    };
    if now < next_at {
        return;
    }
    sprinkling.next_at = None;
    let Some(data) = voice.data(&sound.sound, "ambient") else {
        return;
    };
    let secs = sprinkling.draw(sound.play_secs);
    let amp = sound.sound.volume * config.category_amp(SoundCategory::Ambience);
    voice.start(&mut out, data, amp, now + secs);
    info!("sound: the ambient sprinkle, {secs:.1} s");
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (play_the_loading_sound, play_the_ambient_sprinkle));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loading_screen::LoadingScreen;

    /// One loading screen gives one tick and a second one more, each its full stretch from its
    /// start; a raise onto a screen already up (a portal's onto the world entry's) is the same
    /// screen, the world's black leave is none, and a screen risen before the mixer is up keeps its
    /// tick until it is.
    #[test]
    fn every_loading_screen_ticks_once_for_its_stretch() {
        const SECS: f32 = 4.0;
        let mut screen = LoadingScreen::default();
        let mut ticks = LoadingTicks::default();
        let mut heard = Vec::new();
        let mut look = |screen: &LoadingScreen, mixer_up: bool, now: f64| {
            heard.extend(ticks.take(screen.shown_screens(), mixer_up, now, SECS));
            heard.clone()
        };
        assert!(look(&screen, true, 0.0).is_empty(), "no screen, no tick");
        screen.test_raise(Some(1));
        assert!(look(&screen, false, 1.0).is_empty(), "the mixer is not up yet");
        assert_eq!(look(&screen, true, 2.0), [6.0], "one screen, one tick, 4 s from its start");
        assert_eq!(look(&screen, true, 3.0), [6.0], "never twice");
        screen.test_raise(Some(389));
        assert_eq!(look(&screen, true, 3.5), [6.0], "a raise onto it is the same screen");
        screen.test_dismiss();
        assert_eq!(look(&screen, true, 4.0), [6.0], "the screen going does not end the tick");
        screen.test_blackout();
        assert_eq!(look(&screen, true, 5.0), [6.0], "the black leave is not a loading screen");
        screen.test_raise(Some(1));
        assert_eq!(look(&screen, true, 5.5), [6.0, 9.5], "a second screen, one more");
        screen.test_dismiss();
        screen.test_raise(Some(1));
        screen.test_dismiss();
        screen.test_raise(Some(1));
        assert_eq!(look(&screen, true, 20.0), [6.0, 9.5, 24.0], "two risen between looks play once");
    }
}
