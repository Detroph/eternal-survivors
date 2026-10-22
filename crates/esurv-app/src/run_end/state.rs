//! What the end panel knows, read from the server's messages: who the hero is and where (the last
//! `Ready`), the gear it won this run (each `GearDropped`), and the run as it ended (`RunEnd`). The
//! panel opens on a fall or a victory and closes on a fresh run (`Ready`), when the HUD shows the
//! run going on again (Endless), and when the hero leaves; a run ended from the Escape menu shows
//! no panel (its client is on its way out of the world, or a fresh run is up at once).

use bevy::prelude::*;
use esurv_protocol::{Ending, GearDrop, Phase, Ready, RunEnd, ToClient, ToServer};

/// One of the panel's buttons.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndButton {
    /// A fresh run for the same hero, zone and run options (`again`).
    Again,
    /// After a victory, on into Endless (`endless`).
    KeepFighting,
    /// Out of the run, back to the main menu, the warband screen (`leave`;
    /// "Return to main menu" in place of Change Hero, after a fall as after a victory).
    MainMenu,
}

impl EndButton {
    /// What the server is asked.
    pub fn request(self) -> ToServer {
        match self {
            EndButton::Again => ToServer::Again,
            EndButton::KeepFighting => ToServer::Endless,
            EndButton::MainMenu => ToServer::Leave,
        }
    }

    /// Its label.
    pub fn label(self) -> &'static str {
        match self {
            EndButton::Again => "Run Again",
            EndButton::KeepFighting => "Go Endless",
            EndButton::MainMenu => "Return to main menu",
        }
    }
}

/// The buttons an ending shows, left to right: a fresh run, Endless after a victory only, and the
/// way out. A run that was left shows none (no panel).
pub fn buttons_for(ending: Ending) -> &'static [EndButton] {
    match ending {
        Ending::Fell => &[EndButton::Again, EndButton::MainMenu],
        Ending::Victory => &[
            EndButton::Again,
            EndButton::KeepFighting,
            EndButton::MainMenu,
        ],
        Ending::Left => &[],
    }
}

/// A piece of gear the run won, as the panel lists it: its item entry, and what the server said
/// of it when it dropped (its name, and whether the collection lacked it), when it said so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WonPiece {
    pub entry: u32,
    pub name: Option<String>,
    pub new: bool,
}

/// The run as the panel shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct EndOfRun {
    pub end: RunEnd,
    /// Who the hero is and where its run was.
    pub who: Option<Ready>,
    pub loot: Vec<WonPiece>,
}

impl EndOfRun {
    pub fn buttons(&self) -> &'static [EndButton] {
        buttons_for(self.end.ending)
    }
}

/// The end panel's state.
#[derive(Resource, Debug, Default)]
pub struct RunEndState {
    /// Who the hero is and where its run is: the last `Ready`.
    ready: Option<Ready>,
    /// The gear won this run, as it dropped.
    drops: Vec<GearDrop>,
    /// The panel, while it shows.
    shown: Option<EndOfRun>,
    /// A pressed button whose answer has not come yet: the buttons wait meanwhile.
    pending: Option<EndButton>,
    /// Counts the panels opened, so a new one is drawn afresh.
    opened: u32,
    /// The panel shown is a made-up one (a dev script's `end-panel:`): it stays while the run goes
    /// on, until a button or a fresh run closes it.
    sample: bool,
}

impl RunEndState {
    /// The panel, while it shows.
    pub fn shown(&self) -> Option<&EndOfRun> {
        self.shown.as_ref()
    }

    /// Which panel this is (a new number for each one opened).
    pub fn opened(&self) -> u32 {
        self.opened
    }

    /// The button pressed, while its answer is awaited.
    pub fn pending(&self) -> Option<EndButton> {
        self.pending
    }

    /// Shows a made-up end of the run of the hero in the world (a dev script's `end-panel:`, to
    /// look at the panel unattended): a tally and a payout of sample numbers, no gear. `false`
    /// while no hero is in a run.
    pub fn show_sample(&mut self, ending: Ending) -> bool {
        let Some(who) = self.ready.clone() else {
            return false;
        };
        let won = ending == Ending::Victory;
        self.shown = Some(EndOfRun {
            end: RunEnd {
                hero: who.hero,
                ending,
                won,
                seconds: if won { 960.0 } else { 412.0 },
                level: if won { 31 } else { 17 },
                kills: if won { 2_481 } else { 903 },
                elites: 12,
                rares: 3,
                bosses: if won { 5 } else { 1 },
                damage_dealt: 1_843_207,
                damage_taken: 48_311,
                gold: 1_260,
                caches: 4,
                loot: Vec::new(),
                experience: esurv_protocol::Experience {
                    time: 96,
                    kills: 120,
                    bosses: 50,
                    victory: if won { 100 } else { 0 },
                    ..Default::default()
                },
                experience_before: 40,
                experience_after: if won { 406 } else { 306 },
                points_before: 0,
                points_after: if won { 4 } else { 3 },
                into_next: if won { 74 } else { 60 },
                next_cost: 88,
                zone_opened: None,
            },
            who: Some(who),
            loot: Vec::new(),
        });
        self.pending = None;
        self.sample = true;
        self.opened = self.opened.wrapping_add(1);
        true
    }

    /// Whether a message names this client's hero (any hero while it is not yet known).
    fn mine(&self, hero: u64) -> bool {
        self.ready.as_ref().is_none_or(|r| r.hero == hero)
    }

    fn close(&mut self) {
        self.shown = None;
        self.pending = None;
        self.sample = false;
    }

    /// Takes in a server message; `true` when the panel changed.
    pub fn take(&mut self, m: &ToClient) -> bool {
        match m {
            // a run (a fresh one too) begins: nothing of the last is shown or kept
            ToClient::Ready(r) => {
                self.ready = Some(r.clone());
                self.drops.clear();
                let was = self.shown.is_some();
                self.close();
                was
            }
            ToClient::GearDropped(d) if self.mine(d.hero) => {
                self.drops.push(d.clone());
                false
            }
            ToClient::RunEnd(e) if self.mine(e.hero) => {
                if e.ending == Ending::Left {
                    let was = self.shown.is_some();
                    self.close();
                    return was;
                }
                self.shown = Some(EndOfRun {
                    end: e.clone(),
                    who: self.ready.clone(),
                    loot: self.won(&e.loot),
                });
                self.pending = None;
                self.opened = self.opened.wrapping_add(1);
                true
            }
            // the run goes on (into Endless): the panel has done its work
            ToClient::Hud(h)
                if self.mine(h.hero)
                    && self.shown.is_some()
                    && !self.sample
                    && !matches!(h.phase, Phase::Defeat | Phase::Victory) =>
            {
                self.close();
                true
            }
            ToClient::Refused(verb)
                if self
                    .pending
                    .is_some_and(|b| b.request().verb() == verb.as_str()) =>
            {
                self.pending = None;
                true
            }
            _ => false,
        }
    }

    /// The gear won, each piece matched with what its drop said (a piece won twice, twice).
    fn won(&self, loot: &[u32]) -> Vec<WonPiece> {
        let mut used = vec![false; self.drops.len()];
        loot.iter()
            .map(|&entry| {
                let drop = self
                    .drops
                    .iter()
                    .enumerate()
                    .find(|(i, d)| !used[*i] && d.entry == entry);
                match drop {
                    Some((i, d)) => {
                        used[i] = true;
                        WonPiece {
                            entry,
                            name: (!d.name.is_empty()).then(|| d.name.clone()),
                            new: d.new,
                        }
                    }
                    None => WonPiece {
                        entry,
                        name: None,
                        new: false,
                    },
                }
            })
            .collect()
    }

    /// A button pressed: what to ask the server, unless the panel is not showing it or another
    /// answer is awaited.
    pub fn press(&mut self, button: EndButton) -> Option<ToServer> {
        let shown = self.shown.as_ref()?;
        if self.pending.is_some() || !shown.buttons().contains(&button) {
            return None;
        }
        self.pending = Some(button);
        Some(button.request())
    }

    /// The hero is out of the world: nothing is kept.
    pub fn forget(&mut self) {
        *self = Self {
            opened: self.opened,
            ..Default::default()
        };
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use esurv_protocol::{Experience, Hud};

    use super::*;

    const HERO: u64 = 7;

    fn ready() -> ToClient {
        ToClient::Ready(Ready {
            hero: HERO,
            name: "Probe".into(),
            class: "Class 9".into(),
            race: "Race 1".into(),
            zone: "Area 12".into(),
            subtitle: "Where it begins".into(),
        })
    }

    pub(crate) fn run_end(ending: Ending) -> RunEnd {
        RunEnd {
            hero: HERO,
            ending,
            won: ending == Ending::Victory,
            seconds: 75.4,
            level: 6,
            kills: 151,
            elites: 1,
            rares: 0,
            bosses: 0,
            damage_dealt: 12_345,
            damage_taken: 678,
            gold: 40,
            caches: 1,
            loot: vec![],
            experience: Experience {
                time: 15,
                kills: 15,
                ..Default::default()
            },
            experience_before: 70,
            experience_after: 100,
            points_before: 0,
            points_after: 1,
            into_next: 20,
            next_cost: 95,
            zone_opened: None,
        }
    }

    fn hud(phase: Phase) -> ToClient {
        ToClient::Hud(Hud {
            hero: HERO,
            phase,
            endless: phase == Phase::Running,
            clock: 990,
            kills: 10,
            gold: 0,
            cache_price: 20,
            next_boss: None,
        })
    }

    #[test]
    fn each_ending_has_its_buttons() {
        use EndButton::*;
        assert_eq!(buttons_for(Ending::Fell), [Again, MainMenu]);
        assert_eq!(
            buttons_for(Ending::Victory),
            [Again, KeepFighting, MainMenu]
        );
        assert!(buttons_for(Ending::Left).is_empty());
        assert_eq!(Again.request(), ToServer::Again);
        assert_eq!(KeepFighting.request(), ToServer::Endless);
        assert_eq!(MainMenu.request(), ToServer::Leave);
    }

    #[test]
    fn a_fall_opens_the_panel_and_a_fresh_run_closes_it() {
        let mut s = RunEndState::default();
        s.take(&ready());
        assert!(s.shown().is_none());
        assert!(s.take(&ToClient::RunEnd(run_end(Ending::Fell))));
        let shown = s.shown().unwrap();
        assert_eq!(shown.who.as_ref().unwrap().name, "Probe");
        assert_eq!(s.opened(), 1);
        assert_eq!(s.press(EndButton::KeepFighting), None, "not after a fall");
        assert_eq!(s.press(EndButton::Again), Some(ToServer::Again));
        assert_eq!(s.press(EndButton::MainMenu), None, "one at a time");
        assert!(!s.take(&hud(Phase::Defeat)), "the HUD still shows the fall");
        assert!(s.take(&ready()), "the fresh run closes it");
        assert!(s.shown().is_none() && s.pending().is_none());
    }

    #[test]
    fn a_refusal_lets_the_buttons_work_again() {
        let mut s = RunEndState::default();
        s.take(&ready());
        s.take(&ToClient::RunEnd(run_end(Ending::Fell)));
        s.press(EndButton::Again);
        assert!(!s.take(&ToClient::Refused("reroll".into())));
        assert_eq!(s.pending(), Some(EndButton::Again));
        assert!(s.take(&ToClient::Refused("again".into())));
        assert_eq!(s.pending(), None);
        assert!(s.shown().is_some());
    }

    #[test]
    fn a_victory_waits_and_endless_closes_it_when_the_run_goes_on() {
        let mut s = RunEndState::default();
        s.take(&ready());
        s.take(&ToClient::RunEnd(run_end(Ending::Victory)));
        assert_eq!(s.press(EndButton::KeepFighting), Some(ToServer::Endless));
        assert!(!s.take(&hud(Phase::Victory)));
        assert!(s.take(&hud(Phase::Running)));
        assert!(s.shown().is_none());
        // the Endless run falls later: a panel of its own
        assert!(s.take(&ToClient::RunEnd(run_end(Ending::Fell))));
        assert_eq!(s.opened(), 2);
    }

    #[test]
    fn a_run_left_shows_no_panel_and_another_heros_end_is_not_ours() {
        let mut s = RunEndState::default();
        s.take(&ready());
        assert!(!s.take(&ToClient::RunEnd(run_end(Ending::Left))));
        assert!(s.shown().is_none());
        let other = RunEnd {
            hero: HERO + 1,
            ..run_end(Ending::Fell)
        };
        assert!(!s.take(&ToClient::RunEnd(other)));
        assert!(s.shown().is_none());
        s.take(&ToClient::RunEnd(run_end(Ending::Victory)));
        assert!(
            s.take(&ToClient::RunEnd(run_end(Ending::Left))),
            "left from the panel"
        );
        assert!(s.shown().is_none());
    }

    #[test]
    fn won_gear_is_matched_with_its_drops() {
        let mut s = RunEndState::default();
        s.take(&ready());
        let drop = |entry, name: &str, new| {
            ToClient::GearDropped(GearDrop {
                hero: HERO,
                entry,
                slot: 0,
                tier: 0,
                name: name.into(),
                new,
            })
        };
        s.take(&drop(108_000, "Recruit's Circlet", true));
        s.take(&drop(108_000, "Recruit's Circlet", false));
        s.take(&drop(108_500, "Recruit's Boots", false));
        s.take(&ToClient::RunEnd(RunEnd {
            loot: vec![108_000, 108_500, 108_000, 108_700],
            ..run_end(Ending::Fell)
        }));
        let loot = &s.shown().unwrap().loot;
        let seen: Vec<(u32, Option<&str>, bool)> = loot
            .iter()
            .map(|p| (p.entry, p.name.as_deref(), p.new))
            .collect();
        assert_eq!(
            seen,
            [
                (108_000, Some("Recruit's Circlet"), true),
                (108_500, Some("Recruit's Boots"), false),
                (108_000, Some("Recruit's Circlet"), false),
                (108_700, None, false),
            ]
        );
        // a fresh run forgets the drops
        s.take(&ready());
        s.take(&ToClient::RunEnd(RunEnd {
            loot: vec![108_500],
            ..run_end(Ending::Fell)
        }));
        assert_eq!(s.shown().unwrap().loot[0].name, None);
    }
}
