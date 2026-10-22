//! What the client asks of the server: every request goes under `ESURVR`, its verb first.

use crate::fields::{In, Out};
use crate::{Wire, REQUEST_PREFIX};

/// The screens a client has, said with `hello`. For each one it lacks, the server stands in (an
/// offer taken for it, a victory left as it is).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Screens {
    /// It shows the choice screens and answers them (`choose`, `reroll`).
    pub cards: bool,
    /// It shows the end panel and answers the end of a run (`again`, `endless`, `leave`).
    pub run_end: bool,
    /// It shows HUD messages itself ([`crate::ToClient::Toast`] instead of a chat line).
    pub hud: bool,
}

/// The run options, as whole percentages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOptions {
    pub density: u16,
    pub health: u16,
    pub damage: u16,
    pub speed: u16,
}

impl RunOptions {
    pub(crate) fn write(&self, o: &mut Out) {
        o.num(self.density)
            .num(self.health)
            .num(self.damage)
            .num(self.speed);
    }

    pub(crate) fn read(r: &mut In) -> Option<Self> {
        Some(Self {
            density: r.num()?,
            health: r.num()?,
            damage: r.num()?,
            speed: r.num()?,
        })
    }
}

/// One request from the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToServer {
    /// `hello <version> <screen>...`: the client speaks this protocol; it is told its HUD, choices
    /// and the end of its run from now on.
    Hello {
        version: u32,
        screens: Screens,
    },
    /// `choose <n>`: take offer `n` (from 0) of the open screen.
    Choose(u32),
    /// `reroll`: new offers on the open screen.
    Reroll,
    /// `pause` and `resume` the run.
    Pause,
    Resume,
    /// `endless`: after a victory, go on into Endless.
    Endless,
    /// `again`: once the run has ended, a fresh one for the same hero, zone and run options.
    Again,
    /// `reset`: end the run here (paid out as a run left) and start a fresh one for the same
    /// hero, zone and run options, at any time once it has begun.
    Reset,
    /// `leave`: end the run here.
    Leave,
    /// `options <density> <health> <damage> <speed>`: the run options for the next run.
    RunOptions(RunOptions),
    /// `learn <talent>` and `unlearn <talent>`: one rank more or less.
    LearnTalent(String),
    UnlearnTalent(String),
    /// `prestige <stat>` and `unprestige <stat>`: one prestige rank more or less.
    LearnPrestige(String),
    UnlearnPrestige(String),
    /// `resetprestige`: every prestige rank back.
    ResetPrestige,
    /// `wear <item entry>`: put on a class gear piece from the collection.
    Wear(u32),
    /// `takeoff <slot>`: take off what is worn in a slot.
    TakeOff(u32),
    /// `look <slot> <look or ~>`: show a look (a place in the class set's looks) in a slot; `~` shows the worn
    /// piece's own.
    SetLook {
        slot: u32,
        look: Option<u32>,
    },
    /// `loadout <id>`: the weapon loadout the class fights with from its next run
    /// (roadmap R3).
    ChooseLoadout(String),
}

impl ToServer {
    /// The request's verb, as [`crate::ToClient::Refused`] names it.
    pub fn verb(&self) -> &'static str {
        match self {
            ToServer::Hello { .. } => "hello",
            ToServer::Choose(_) => "choose",
            ToServer::Reroll => "reroll",
            ToServer::Pause => "pause",
            ToServer::Resume => "resume",
            ToServer::Endless => "endless",
            ToServer::Again => "again",
            ToServer::Reset => "reset",
            ToServer::Leave => "leave",
            ToServer::RunOptions(_) => "options",
            ToServer::LearnTalent(_) => "learn",
            ToServer::UnlearnTalent(_) => "unlearn",
            ToServer::LearnPrestige(_) => "prestige",
            ToServer::UnlearnPrestige(_) => "unprestige",
            ToServer::ResetPrestige => "resetprestige",
            ToServer::Wear(_) => "wear",
            ToServer::TakeOff(_) => "takeoff",
            ToServer::SetLook { .. } => "look",
            ToServer::ChooseLoadout(_) => "loadout",
        }
    }
}

const SCREEN_WORDS: [&str; 3] = ["cards", "end", "hud"];

impl Wire for ToServer {
    fn prefix(&self) -> &'static str {
        REQUEST_PREFIX
    }

    fn message(&self) -> String {
        let mut o = Out::new();
        o.word(self.verb());
        match self {
            ToServer::Hello { version, screens } => {
                o.num(version);
                for (on, word) in [screens.cards, screens.run_end, screens.hud]
                    .into_iter()
                    .zip(SCREEN_WORDS)
                {
                    if on {
                        o.word(word);
                    }
                }
            }
            ToServer::Choose(n) => {
                o.num(n);
            }
            ToServer::RunOptions(options) => options.write(&mut o),
            ToServer::LearnTalent(id)
            | ToServer::UnlearnTalent(id)
            | ToServer::LearnPrestige(id)
            | ToServer::UnlearnPrestige(id)
            | ToServer::ChooseLoadout(id) => {
                o.text(id);
            }
            ToServer::Wear(entry) => {
                o.num(entry);
            }
            ToServer::TakeOff(slot) => {
                o.num(slot);
            }
            ToServer::SetLook { slot, look } => {
                o.num(slot).opt(*look);
            }
            ToServer::Reroll
            | ToServer::Pause
            | ToServer::Resume
            | ToServer::Endless
            | ToServer::Again
            | ToServer::Reset
            | ToServer::Leave
            | ToServer::ResetPrestige => {}
        }
        o.finish()
    }

    fn decode(prefix: &str, message: &str) -> Option<Self> {
        if prefix != REQUEST_PREFIX {
            return None;
        }
        let r = &mut In::new(message);
        Some(match r.word()? {
            "hello" => {
                let version = r.num()?;
                let mut screens = Screens::default();
                // a screen this server does not know is left out
                while let Some(word) = r.word() {
                    match word {
                        "cards" => screens.cards = true,
                        "end" => screens.run_end = true,
                        "hud" => screens.hud = true,
                        _ => {}
                    }
                }
                ToServer::Hello { version, screens }
            }
            "choose" => ToServer::Choose(r.num()?),
            "reroll" => ToServer::Reroll,
            "pause" => ToServer::Pause,
            "resume" => ToServer::Resume,
            "endless" => ToServer::Endless,
            "again" => ToServer::Again,
            "reset" => ToServer::Reset,
            "leave" => ToServer::Leave,
            "options" => ToServer::RunOptions(RunOptions::read(r)?),
            "learn" => ToServer::LearnTalent(r.text()?),
            "unlearn" => ToServer::UnlearnTalent(r.text()?),
            "prestige" => ToServer::LearnPrestige(r.text()?),
            "unprestige" => ToServer::UnlearnPrestige(r.text()?),
            "resetprestige" => ToServer::ResetPrestige,
            "wear" => ToServer::Wear(r.num()?),
            "takeoff" => ToServer::TakeOff(r.num()?),
            "loadout" => ToServer::ChooseLoadout(r.text()?),
            "look" => ToServer::SetLook {
                slot: r.num()?,
                look: r.opt()?,
            },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Reader, MAX_LINE};

    fn round_trip(m: ToServer) {
        let lines = m.lines();
        assert_eq!(lines.len(), 1, "a request is one line");
        assert!(lines[0].to_string().len() <= MAX_LINE);
        assert_eq!(
            Reader::<ToServer>::default().read(&lines[0].to_string()),
            Some(m)
        );
    }

    #[test]
    fn every_request_reads_back() {
        round_trip(ToServer::Hello {
            version: crate::VERSION,
            screens: Screens::default(),
        });
        round_trip(ToServer::Hello {
            version: 1,
            screens: Screens {
                cards: true,
                run_end: true,
                hud: true,
            },
        });
        round_trip(ToServer::Choose(2));
        round_trip(ToServer::Reroll);
        round_trip(ToServer::Pause);
        round_trip(ToServer::Resume);
        round_trip(ToServer::Endless);
        round_trip(ToServer::Again);
        round_trip(ToServer::Reset);
        round_trip(ToServer::Leave);
        round_trip(ToServer::RunOptions(RunOptions {
            density: 200,
            health: 125,
            damage: 110,
            speed: 105,
        }));
        round_trip(ToServer::LearnTalent("improved_fireball".into()));
        round_trip(ToServer::UnlearnTalent("arcane_focus".into()));
        round_trip(ToServer::LearnPrestige("damage".into()));
        round_trip(ToServer::UnlearnPrestige("max_health".into()));
        round_trip(ToServer::ResetPrestige);
        round_trip(ToServer::Wear(108_310));
        round_trip(ToServer::TakeOff(3));
        round_trip(ToServer::SetLook {
            slot: 0,
            look: Some(4),
        });
        round_trip(ToServer::SetLook {
            slot: 7,
            look: None,
        });
        round_trip(ToServer::ChooseLoadout("dual_wield".into()));
    }

    #[test]
    fn requests_read_as_written() {
        assert_eq!(
            ToServer::Choose(1).lines()[0].to_string(),
            "ESURVR\tchoose 1"
        );
        assert_eq!(
            ToServer::Hello {
                version: 1,
                screens: Screens {
                    cards: true,
                    ..Default::default()
                }
            }
            .lines()[0]
                .to_string(),
            "ESURVR\thello 1 cards"
        );
        assert_eq!(ToServer::Again.lines()[0].to_string(), "ESURVR\tagain");
        let mut r = Reader::<ToServer>::default();
        assert_eq!(
            r.read("ESURVR\thello 2 cards dance"),
            Some(ToServer::Hello {
                version: 2,
                screens: Screens {
                    cards: true,
                    ..Default::default()
                }
            }),
            "a newer client's screens this server does not know are left out"
        );
        assert_eq!(r.read("ESURVR\tchoose"), None);
        assert_eq!(r.read("ESURVR\tdance"), None);
        assert_eq!(r.read("ESURVH\tchoose 1"), None, "not a request prefix");
    }
}
