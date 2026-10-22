//! The scripted pause (docs/game/bosses.md, "Building blocks"): for a cutscene of
//! at most 5 seconds the whole fight freezes, the heroes, the enemies, the spawns and every timer
//! alike, while its line shows. It is not a boss's crowd control: everyone stands, and the run's
//! clock stops with them. No Elwynn or Duskwood boss uses it yet.

use super::run::{Event, Run};

/// A scripted pause running: the real seconds it has left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cutscene {
    pub left: f32,
}

impl Run {
    /// The fight freezes for `seconds` (5 at most) while `text` shows.
    pub(crate) fn scripted_pause(&mut self, seconds: f32, text: &str) {
        let seconds = seconds.clamp(0.0, esurv_content::LONGEST_PAUSE);
        if seconds <= 0.0 {
            return;
        }
        self.cutscene = Some(Cutscene { left: seconds });
        self.push(Event::PauseBegan { seconds, text: text.to_string() });
    }

    /// Whether a scripted pause holds the fight now.
    pub fn in_cutscene(&self) -> bool {
        self.cutscene.is_some()
    }

    /// A tick during a scripted pause: only its own time runs; true while it still holds.
    pub(crate) fn cutscene_tick(&mut self, dt: f32) -> bool {
        let Some(c) = &mut self.cutscene else { return false };
        c.left -= dt;
        if c.left <= 0.0 {
            self.cutscene = None;
            self.push(Event::PauseEnded);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scripted_pause_freezes_the_clock_enemies_and_spawns_then_lets_go() {
        let mut r = crate::game::ground_zones::tests::quiet_run();
        r.spawn_boss(0);
        r.enemies.last_mut().unwrap().hold_until = 0.0;
        let p = r.heroes[0].pos;
        let (id, now) = (r.heroes[0].id, r.now);
        let before: Vec<_> = r.enemies.iter().map(|e| e.position(now)).collect();
        let enemies = r.enemies.len();
        r.scripted_pause(9.0, "Stop!");
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::PauseBegan { seconds, .. } if *seconds == 5.0)), "5 s at most");
        // a hero's move is not taken while it holds
        r.hero_moved(id, (p.0 + 5.0, p.1), 0.0, 0.0);
        for _ in 0..40 {
            r.tick(0.1);
        }
        assert_eq!(r.now, now, "the clock stood");
        assert_eq!(r.heroes[0].pos, p, "the hero stood");
        assert_eq!(r.enemies.len(), enemies, "nothing spawned");
        assert_eq!(r.enemies.iter().map(|e| e.position(now)).collect::<Vec<_>>(), before, "the enemies stood");
        for _ in 0..20 {
            r.tick(0.1);
        }
        assert!(!r.in_cutscene());
        assert!(r.drain_events().iter().any(|e| matches!(e, Event::PauseEnded)));
        assert!(r.now > now, "the clock runs again");
    }
}
