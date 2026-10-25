//! What a client asks before a run, on the character screen (the menus before a run):
//! the facts the hero sheet and the warband screen show, and the changes made
//! there. Every ask goes under [`PRE_RUN_PREFIX`], a verb first; the answers are
//! [`crate::ToClient`] messages (`Progress`, `RunOptions`, `Codex`, `Hero`, `Refused`).
//!
//! A local game hands them straight to its embedded server's save; the lines are there for a
//! carrier to a remote host. A hero is named by its player guid, a class and a zone by their ids
//! in the content, a race by its 1.12 id.

use crate::fields::{In, Out};
use crate::{RunOptions, ToServer, Wire};

/// Every ask before a run.
pub const PRE_RUN_PREFIX: &str = "ESURVQ";

/// One ask before a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreRunAsk {
    /// `facts <class>`: what the class has earned (`Progress`), the run options (`RunOptions`)
    /// and the codex (`Codex`).
    Facts { class: String },
    /// `hero <guid>`: the hero's records and the zone its next run is in (`Hero`).
    Hero(u64),
    /// `class <class> <request>`: a change of what the class has earned, one of the armory's
    /// requests (`learn`, `unlearn`, `prestige`, `unprestige`, `resetprestige`, `wear`,
    /// `takeoff`, `look`); answered with `Progress`.
    Armory { class: String, change: ToServer },
    /// `options <density> <health> <damage> <speed> <challenge> <challenge %>`: the run options and
    /// the mode (Challenge Mode or not, and its slider) for the next run.
    RunOptions(RunOptions),
    /// `rename <guid> <name>`: the stock name rules; answered with `Hero`.
    Rename { hero: u64, name: String },
    /// `reshape <guid> <race> <gender> <skin> <face> <hair style> <hair colour> <facial hair>`:
    /// another race (1.12 id), sex or look, a race the class may be; answered with `Hero`.
    Reshape {
        hero: u64,
        race: u8,
        gender: u8,
        look: [u8; 5],
    },
    /// `zone <guid> <zone>`: the zone the hero's next run is in, one its class has opened;
    /// answered with `Hero`.
    Zone { hero: u64, zone: String },
    /// `start <guid> <option>`: the starting option the hero's next runs start with, one of its
    /// class's; answered with `Hero`.
    Start { hero: u64, option: String },
    /// `savefiles`: the folder saves are exported to and imported from, and the saves in it
    /// (`SaveFiles`). Only the game on the save's own machine is answered; a remote host refuses.
    SaveFiles,
    /// `export`: a copy of the whole save, written to a new dated file in that folder
    /// (`SaveExported`, then `SaveFiles`). Refused by a remote host.
    ExportSave,
    /// `import <file>`: the save replaced by the file of that name in that folder once it is
    /// checked, the save it replaces kept there first (`SaveImported`, then `SaveFiles`). Refused
    /// by a remote host.
    ImportSave { file: String },
    /// `seen <class> <collection|talents|challenge>`: the player opened the class's Collection or Talents
    /// before a run, so what it shows is seen (the warband screen's
    /// button to it glows until then), or closed the box telling that Challenge Mode is open (once
    /// per save); answered with `Progress`.
    Seen { class: String, view: SeenView },
}

/// A view of the hero sheet whose news the warband screen's buttons show:
/// gear unlocked since the Collection was last opened, talent points earned since the
/// Talents were; and the box telling that Challenge Mode is open, once per save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SeenView {
    Collection,
    Talents,
    Challenge,
}

impl SeenView {
    /// The view's word on the line.
    pub fn word(self) -> &'static str {
        match self {
            SeenView::Collection => "collection",
            SeenView::Talents => "talents",
            SeenView::Challenge => "challenge",
        }
    }

    /// The view a word names.
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "collection" => Some(SeenView::Collection),
            "talents" => Some(SeenView::Talents),
            "challenge" => Some(SeenView::Challenge),
            _ => None,
        }
    }
}

impl PreRunAsk {
    /// An armory request for `class`; `None` for a request that is no armory change.
    pub fn armory(class: &str, change: &ToServer) -> Option<Self> {
        matches!(
            change,
            ToServer::LearnTalent(_)
                | ToServer::UnlearnTalent(_)
                | ToServer::LearnPrestige(_)
                | ToServer::UnlearnPrestige(_)
                | ToServer::ResetPrestige
                | ToServer::Wear(_)
                | ToServer::TakeOff(_)
                | ToServer::SetLook { .. }
                | ToServer::ChooseLoadout(_)
        )
        .then(|| PreRunAsk::Armory {
            class: class.into(),
            change: change.clone(),
        })
    }

    /// The ask's verb.
    pub fn verb(&self) -> &'static str {
        match self {
            PreRunAsk::Facts { .. } => "facts",
            PreRunAsk::Hero(_) => "hero",
            PreRunAsk::Armory { .. } => "class",
            PreRunAsk::RunOptions(_) => "options",
            PreRunAsk::Rename { .. } => "rename",
            PreRunAsk::Reshape { .. } => "reshape",
            PreRunAsk::Zone { .. } => "zone",
            PreRunAsk::Start { .. } => "start",
            PreRunAsk::SaveFiles => "savefiles",
            PreRunAsk::ExportSave => "export",
            PreRunAsk::ImportSave { .. } => "import",
            PreRunAsk::Seen { .. } => "seen",
        }
    }

    /// Whether the ask reads or writes the save's own file, which only the game on the save's
    /// machine may (export and import).
    pub fn touches_the_save_file(&self) -> bool {
        matches!(
            self,
            PreRunAsk::SaveFiles | PreRunAsk::ExportSave | PreRunAsk::ImportSave { .. }
        )
    }

    /// The verb a refusal names: an armory change's own (`learn` ...), else the ask's.
    pub fn refused_verb(&self) -> &'static str {
        match self {
            PreRunAsk::Armory { change, .. } => change.verb(),
            other => other.verb(),
        }
    }
}

impl Wire for PreRunAsk {
    fn prefix(&self) -> &'static str {
        PRE_RUN_PREFIX
    }

    fn message(&self) -> String {
        let mut o = Out::new();
        o.word(self.verb());
        match self {
            PreRunAsk::Facts { class } => {
                o.text(class);
            }
            PreRunAsk::Hero(hero) => {
                o.guid(*hero);
            }
            PreRunAsk::Armory { class, change } => {
                o.text(class);
                // the request as it reads on its own, verb first
                return format!("{} {}", o.finish(), change.message());
            }
            PreRunAsk::RunOptions(options) => {
                options.write(&mut o);
                options.write_mode(&mut o);
            }
            PreRunAsk::Rename { hero, name } => {
                o.guid(*hero).text(name);
            }
            PreRunAsk::Reshape {
                hero,
                race,
                gender,
                look,
            } => {
                o.guid(*hero).num(race).num(gender);
                for l in look {
                    o.num(l);
                }
            }
            PreRunAsk::Zone { hero, zone } => {
                o.guid(*hero).text(zone);
            }
            PreRunAsk::Start { hero, option } => {
                o.guid(*hero).text(option);
            }
            PreRunAsk::SaveFiles | PreRunAsk::ExportSave => {}
            PreRunAsk::ImportSave { file } => {
                o.text(file);
            }
            PreRunAsk::Seen { class, view } => {
                o.text(class).word(view.word());
            }
        }
        o.finish()
    }

    fn decode(prefix: &str, message: &str) -> Option<Self> {
        if prefix != PRE_RUN_PREFIX {
            return None;
        }
        // an armory change carries a whole request after its class
        if let Some(rest) = message.strip_prefix("class ") {
            let (class, request) = rest.split_once(' ')?;
            let class = In::new(class).text()?;
            let change = ToServer::decode(crate::REQUEST_PREFIX, request)?;
            return PreRunAsk::armory(&class, &change);
        }
        let r = &mut In::new(message);
        Some(match r.word()? {
            "facts" => PreRunAsk::Facts { class: r.text()? },
            "hero" => PreRunAsk::Hero(r.guid()?),
            "options" => {
                let mut options = RunOptions::read(r)?;
                options.read_mode(r);
                PreRunAsk::RunOptions(options)
            }
            "rename" => PreRunAsk::Rename {
                hero: r.guid()?,
                name: r.text()?,
            },
            "reshape" => PreRunAsk::Reshape {
                hero: r.guid()?,
                race: r.num()?,
                gender: r.num()?,
                look: [r.num()?, r.num()?, r.num()?, r.num()?, r.num()?],
            },
            "zone" => PreRunAsk::Zone {
                hero: r.guid()?,
                zone: r.text()?,
            },
            "start" => PreRunAsk::Start {
                hero: r.guid()?,
                option: r.text()?,
            },
            "savefiles" => PreRunAsk::SaveFiles,
            "export" => PreRunAsk::ExportSave,
            "import" => PreRunAsk::ImportSave { file: r.text()? },
            "seen" => PreRunAsk::Seen {
                class: r.text()?,
                view: SeenView::from_word(r.word()?)?,
            },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Reader, MAX_LINE};

    fn round_trip(m: PreRunAsk) {
        let lines = m.lines();
        assert_eq!(lines.len(), 1, "an ask is one line");
        assert!(lines[0].to_string().len() <= MAX_LINE);
        assert_eq!(
            Reader::<PreRunAsk>::default().read(&lines[0].to_string()),
            Some(m)
        );
    }

    #[test]
    fn every_ask_reads_back() {
        round_trip(PreRunAsk::Facts {
            class: "mage".into(),
        });
        round_trip(PreRunAsk::Hero(7));
        for change in [
            ToServer::LearnTalent("improved_fireball".into()),
            ToServer::UnlearnTalent("arcane_focus".into()),
            ToServer::LearnPrestige("damage".into()),
            ToServer::UnlearnPrestige("damage".into()),
            ToServer::ResetPrestige,
            ToServer::Wear(108_310),
            ToServer::TakeOff(3),
            ToServer::SetLook {
                slot: 0,
                look: Some(4),
            },
            ToServer::SetLook {
                slot: 7,
                look: None,
            },
        ] {
            round_trip(PreRunAsk::armory("warrior", &change).unwrap());
        }
        round_trip(PreRunAsk::RunOptions(RunOptions {
            density: 200,
            health: 125,
            damage: 110,
            speed: 105,
            challenge: true,
            challenge_pct: 175,
        }));
        round_trip(PreRunAsk::Rename {
            hero: 3,
            name: "Anya".into(),
        });
        round_trip(PreRunAsk::Reshape {
            hero: 3,
            race: 7,
            gender: 1,
            look: [1, 2, 3, 4, 5],
        });
        round_trip(PreRunAsk::Zone {
            hero: 3,
            zone: "duskwood".into(),
        });
        round_trip(PreRunAsk::Start {
            hero: 3,
            option: "whirlwind".into(),
        });
        round_trip(PreRunAsk::SaveFiles);
        round_trip(PreRunAsk::ExportSave);
        round_trip(PreRunAsk::ImportSave {
            file: "heroes 2026-10-13 142530.ron".into(),
        });
        for view in [SeenView::Collection, SeenView::Talents, SeenView::Challenge] {
            round_trip(PreRunAsk::Seen {
                class: "shaman".into(),
                view,
            });
        }
        assert!(PreRunAsk::ExportSave.touches_the_save_file());
        assert!(!PreRunAsk::Hero(3).touches_the_save_file());
    }

    #[test]
    fn only_armory_changes_go_to_a_class() {
        assert_eq!(PreRunAsk::armory("mage", &ToServer::Leave), None);
        let ask = PreRunAsk::armory("mage", &ToServer::Wear(1)).unwrap();
        assert_eq!(ask.message(), "class mage wear 1");
        assert_eq!(ask.refused_verb(), "wear");
        assert_eq!(
            PreRunAsk::decode(PRE_RUN_PREFIX, "class mage leave"),
            None,
            "a run's request is no armory change"
        );
    }
}
