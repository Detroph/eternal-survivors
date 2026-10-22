//! What the HUD knows of the run, from the server's messages, and the words it shows (our own).

use bevy::prelude::*;
use esurv_protocol::{Banner, BossBar, Hud, Phase, Ready, ToClient};

use super::lines::LineLook;

/// The run as the HUD knows it: who and where the hero is (`Ready`), its numbers (`Hud`), and
/// the bosses up (`BossBar`), oldest first.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct RunHud {
    pub ready: Option<Ready>,
    pub numbers: Option<Hud>,
    pub bosses: Vec<BossBar>,
}

impl RunHud {
    /// Takes in one message from the server; whether it changed what the HUD shows. `Ready` starts
    /// a run afresh; a HUD line for another hero is not ours; a boss bar at 0 health goes.
    pub fn apply(&mut self, message: &ToClient) -> bool {
        match message {
            ToClient::Ready(r) => {
                *self = RunHud {
                    ready: Some(r.clone()),
                    ..Default::default()
                };
                true
            }
            ToClient::Hud(h) => {
                if self.ready.as_ref().is_some_and(|r| r.hero != h.hero) {
                    return false;
                }
                let changed = self.numbers.as_ref() != Some(h);
                self.numbers = Some(h.clone());
                changed
            }
            ToClient::BossBar(b) => {
                let at = self.bosses.iter().position(|x| x.boss == b.boss);
                match (at, b.health) {
                    (Some(i), 0) => {
                        self.bosses.remove(i);
                    }
                    (None, 0) => return false,
                    (Some(i), _) => {
                        if self.bosses[i] == *b {
                            return false;
                        }
                        self.bosses[i] = b.clone();
                    }
                    (None, _) => self.bosses.push(b.clone()),
                }
                true
            }
            _ => false,
        }
    }

    /// The run's phase, once the server has said it.
    pub fn phase(&self) -> Option<Phase> {
        self.numbers.as_ref().map(|h| h.phase)
    }

    /// The boss whose bar shows: the first one up still alive.
    pub fn boss(&self) -> Option<&BossBar> {
        self.bosses.first()
    }

    /// Forgets the run (the hero left the world).
    pub fn clear(&mut self) {
        *self = RunHud::default();
    }
}

/// The run clock as minutes and seconds, `12:05`; the minutes go on past the hour.
pub fn clock_text(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// The line under the clock: when the next boss comes, Endless once it is on, or nothing when no
/// boss is left to come.
pub fn next_boss_text(endless: bool, next_boss: Option<u32>) -> Option<String> {
    if endless {
        return Some("Endless mode".into());
    }
    next_boss.map(|s| format!("Boss arrives in {}", clock_text(s)))
}

/// A boss bar's health, as the stock status bars write it.
pub fn health_text(health: u32, max_health: u32) -> String {
    format!("{health} / {max_health}")
}

/// A boss bar's fill, 0 to 1.
pub fn health_fraction(health: u32, max_health: u32) -> f32 {
    if max_health == 0 {
        return 0.0;
    }
    (health as f32 / max_health as f32).clamp(0.0, 1.0)
}

/// What a banner says, and how it looks.
pub fn banner_line(banner: &Banner) -> (String, LineLook) {
    match banner {
        Banner::BossComes {
            name,
            final_boss: false,
        } => (format!("{name} draws near!"), LineLook::BossComes),
        Banner::BossComes {
            name,
            final_boss: true,
        } => (
            format!("{name} takes the field for the last stand!"),
            LineLook::FinalBossComes,
        ),
        Banner::BossDown { name } => (format!("{name} is vanquished!"), LineLook::BossDown),
    }
}

/// The hint before the first step.
pub const START_HINT: &str = "Take a step to begin the fight.";

/// The Escape menu's words.
pub const MENU_TITLE: &str = "Run Paused";
pub const MENU_HINT: &str = "Press Escape to fight on.";
pub const CONTINUE_BUTTON: &str = "Continue";
pub const RESET_BUTTON: &str = "Reset Run";
pub const OPTIONS_BUTTON: &str = "Options";
pub const END_BUTTON: &str = "End Run";
pub const QUIT_BUTTON: &str = "Quit Game";
/// Over the end panel, End Run is the panel's own way out.
pub const MAIN_MENU_BUTTON: &str = "Return to main menu";
/// The menu opened before the first step and over the end panel: no run is paused.
pub const GAME_MENU_TITLE: &str = "Game Menu";
pub const BEFORE_START_HINT: &str =
    "The fight begins with your first step. Press Escape to return.";
pub const RUN_OVER_HINT: &str = "Press Escape to return to the run report.";
/// Said when the server will not reset the run: another hero still fights in it.
pub const RESET_REFUSED: &str = "The run cannot be reset while another hero is still in it.";

/// The readouts' labels.
pub const KILLS_LABEL: &str = "Foes slain";
pub const GOLD_LABEL: &str = "Gold";

/// The next cache's price, under the gold.
pub fn cache_text(price: u32) -> String {
    format!("Next cache: {price}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hud(hero: u64, phase: Phase) -> Hud {
        Hud {
            hero,
            phase,
            endless: false,
            clock: 0,
            kills: 0,
            gold: 0,
            cache_price: 20,
            next_boss: Some(60),
        }
    }

    fn ready(hero: u64) -> Ready {
        Ready {
            hero,
            name: "Probe".into(),
            class: "Class 8".into(),
            race: "Race 1".into(),
            zone: "Area 12".into(),
            subtitle: "".into(),
        }
    }

    fn bar(boss: u64, health: u32) -> BossBar {
        BossBar {
            boss,
            name: format!("Boss {boss}"),
            health,
            max_health: 1000,
        }
    }

    #[test]
    fn the_clock_reads_minutes_and_seconds() {
        assert_eq!(clock_text(0), "0:00");
        assert_eq!(clock_text(9), "0:09");
        assert_eq!(clock_text(65), "1:05");
        assert_eq!(clock_text(3725), "62:05", "no hours");
    }

    #[test]
    fn the_next_boss_line_has_three_cases() {
        assert_eq!(
            next_boss_text(false, Some(146)).as_deref(),
            Some("Boss arrives in 2:26")
        );
        assert_eq!(next_boss_text(false, None), None, "none left to come");
        assert_eq!(
            next_boss_text(true, Some(30)).as_deref(),
            Some("Endless mode"),
            "Endless wins over a schedule"
        );
    }

    #[test]
    fn the_health_bar_fills_by_health() {
        assert_eq!(health_fraction(250, 1000), 0.25);
        assert_eq!(health_fraction(5, 0), 0.0);
        assert_eq!(health_fraction(2000, 1000), 1.0);
        assert_eq!(health_text(250, 1000), "250 / 1000");
    }

    #[test]
    fn ready_starts_afresh_and_only_our_hud_counts() {
        let mut run = RunHud::default();
        assert!(run.apply(&ToClient::BossBar(bar(9, 500))));
        assert!(run.apply(&ToClient::Ready(ready(7))));
        assert!(run.bosses.is_empty(), "a new run has no boss up");
        assert_eq!(run.phase(), None);
        assert!(
            !run.apply(&ToClient::Hud(hud(8, Phase::Running))),
            "not ours"
        );
        assert!(run.apply(&ToClient::Hud(hud(7, Phase::Waiting))));
        assert_eq!(run.phase(), Some(Phase::Waiting));
        assert!(
            !run.apply(&ToClient::Hud(hud(7, Phase::Waiting))),
            "the same numbers change nothing"
        );
    }

    #[test]
    fn the_first_boss_up_shows_until_it_goes() {
        let mut run = RunHud::default();
        run.apply(&ToClient::BossBar(bar(1, 900)));
        run.apply(&ToClient::BossBar(bar(2, 1000)));
        assert_eq!(run.boss().map(|b| b.boss), Some(1));
        assert!(run.apply(&ToClient::BossBar(bar(1, 400))));
        assert_eq!(run.boss().map(|b| b.health), Some(400), "kept in place");
        assert!(run.apply(&ToClient::BossBar(bar(1, 0))), "it went");
        assert_eq!(run.boss().map(|b| b.boss), Some(2));
        assert!(!run.apply(&ToClient::BossBar(bar(5, 0))), "never up");
    }

    #[test]
    fn banners_name_the_boss() {
        let (text, look) = banner_line(&Banner::BossComes {
            name: "Hogger".into(),
            final_boss: true,
        });
        assert!(text.contains("Hogger"));
        assert_eq!(look, LineLook::FinalBossComes);
    }
}
