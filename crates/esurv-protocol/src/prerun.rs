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
    /// `achseen`: the player opened the Achievements window, so what was earned is seen (the
    /// warband screen's Achievements button glows until then); answered with `Achievements`.
    AchievementsSeen,
    /// `track <id> <on>`: an achievement not yet earned tracked in runs, or no longer (at most
    /// [`MAX_TRACKED`] at once); answered with `Achievements`.
    Track { id: String, on: bool },
    /// `buy <item>`: the shop sells the warband item `item` (`companion:<id>`,
    /// `companion_rank:<n>`) for points to spend, never the score; answered with
    /// `Achievements`, or refused (unknown, not for sale, owned, too dear, a rank out of turn).
    Buy { item: String },
    /// `companion <guid> <id>`: the companion the hero brings, one the warband owns; `~` for
    /// none. Answered with `Hero`.
    Companion { hero: u64, companion: String },
    /// `temper <guid> <ids>`: the hero's temperaments (ids of `camp_chatter.ron`), its leading
    /// one first, at most two; none for Quiet. Guid 0 is the next hero made (the sheet's Create
    /// sends it first). Answered with `Hero`, or nothing for the next hero.
    Temperament { hero: u64, temperament: Vec<String> },
    /// `period <guid> <past|present|future>`: the time period the hero was pulled into the camp
    /// from (flavour only: which of the camp's lines it says). Guid 0 is the next hero made (the
    /// sheet's Create sends it first). Answered with `Hero`, or nothing for the next hero.
    TimePeriod { hero: u64, time_period: TimePeriod },
    /// `dress <guid> <kind> <id>`: the title (an achievement's id), tabard or weapon glow the
    /// hero wears, one the warband owns (a tabard its faction may wear); empty for none.
    /// Answered with `Hero`.
    Dress { hero: u64, kind: WearKind, id: String },
    /// `decorate <camp> <spot> <piece>`: camp `horde` or `alliance` shows decoration `piece` (one
    /// the warband owns, fitting the spot) on spot `spot` (an open one), moving it from another
    /// spot of that camp; empty for none. Answered with `Achievements`.
    Decorate { camp: String, spot: String, piece: String },
}

/// What a hero wears besides its gear and companion (step 3 of the achievements).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WearKind {
    Title,
    Tabard,
    Glow,
}

impl WearKind {
    pub const ALL: [WearKind; 3] = [WearKind::Title, WearKind::Tabard, WearKind::Glow];

    /// Its word on the line.
    pub fn word(self) -> &'static str {
        match self {
            WearKind::Title => "title",
            WearKind::Tabard => "tabard",
            WearKind::Glow => "glow",
        }
    }

    pub fn from_word(word: &str) -> Option<Self> {
        WearKind::ALL.into_iter().find(|k| k.word() == word)
    }
}

/// The time a hero was pulled into the camp at the Caverns of Time from: before the classic era,
/// classic's own time, or after it. Flavour only: it picks which of the camp's lines the hero says
/// (`camp_chatter.ron`) and changes nothing in a run. A hero of an older save: Present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TimePeriod {
    /// Before the classic era: the Second War, Stratholme before the plague, Draenor before the
    /// Portal.
    Past,
    /// Classic's own time.
    #[default]
    Present,
    /// After the classic era: the Lich King's fall, the Cataclysm and on.
    Future,
}

impl TimePeriod {
    pub const ALL: [TimePeriod; 3] = [TimePeriod::Past, TimePeriod::Present, TimePeriod::Future];

    /// Its word on the line.
    pub fn word(self) -> &'static str {
        match self {
            TimePeriod::Past => "past",
            TimePeriod::Present => "present",
            TimePeriod::Future => "future",
        }
    }

    pub fn from_word(word: &str) -> Option<Self> {
        TimePeriod::ALL.into_iter().find(|p| p.word() == word)
    }
}

/// How many achievements may be tracked in runs at once.
pub const MAX_TRACKED: usize = 3;

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
            PreRunAsk::AchievementsSeen => "achseen",
            PreRunAsk::Track { .. } => "track",
            PreRunAsk::Buy { .. } => "buy",
            PreRunAsk::Companion { .. } => "companion",
            PreRunAsk::Temperament { .. } => "temper",
            PreRunAsk::TimePeriod { .. } => "period",
            PreRunAsk::Dress { .. } => "dress",
            PreRunAsk::Decorate { .. } => "decorate",
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
            PreRunAsk::AchievementsSeen => {}
            PreRunAsk::Track { id, on } => {
                o.text(id).flag(*on);
            }
            PreRunAsk::Buy { item } => {
                o.text(item);
            }
            PreRunAsk::Companion { hero, companion } => {
                o.guid(*hero).text(companion);
            }
            PreRunAsk::Temperament { hero, temperament } => {
                o.guid(*hero).list(temperament, |o, t| {
                    o.text(t);
                });
            }
            PreRunAsk::TimePeriod { hero, time_period } => {
                o.guid(*hero).word(time_period.word());
            }
            PreRunAsk::Dress { hero, kind, id } => {
                o.guid(*hero).text(kind.word()).text(id);
            }
            PreRunAsk::Decorate { camp, spot, piece } => {
                o.text(camp).text(spot).text(piece);
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
            "achseen" => PreRunAsk::AchievementsSeen,
            "track" => PreRunAsk::Track {
                id: r.text()?,
                on: r.flag()?,
            },
            "buy" => PreRunAsk::Buy { item: r.text()? },
            "companion" => PreRunAsk::Companion {
                hero: r.guid()?,
                companion: r.text()?,
            },
            "temper" => PreRunAsk::Temperament {
                hero: r.guid()?,
                temperament: r.list(|r| r.text())?,
            },
            "period" => PreRunAsk::TimePeriod {
                hero: r.guid()?,
                time_period: TimePeriod::from_word(r.word()?)?,
            },
            "dress" => PreRunAsk::Dress {
                hero: r.guid()?,
                kind: WearKind::from_word(&r.text()?)?,
                id: r.text()?,
            },
            "decorate" => PreRunAsk::Decorate {
                camp: r.text()?,
                spot: r.text()?,
                piece: r.text()?,
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
        round_trip(PreRunAsk::AchievementsSeen);
        for kind in WearKind::ALL {
            for id in ["the_eternal_road", ""] {
                round_trip(PreRunAsk::Dress { hero: 4, kind, id: id.into() });
            }
        }
        for piece in ["throne", ""] {
            round_trip(PreRunAsk::Decorate { camp: "horde".into(), spot: "trophy_1".into(), piece: piece.into() });
        }
        for on in [true, false] {
            round_trip(PreRunAsk::Track {
                id: "spin_to_win".into(),
                on,
            });
        }
        round_trip(PreRunAsk::Buy {
            item: "companion:bombay_cat".into(),
        });
        for companion in ["tiny_hogger", ""] {
            round_trip(PreRunAsk::Companion {
                hero: 3,
                companion: companion.into(),
            });
        }
        for temperament in [vec!["grim".into(), "weary".into()], vec![]] {
            round_trip(PreRunAsk::Temperament { hero: 0, temperament });
        }
        for time_period in TimePeriod::ALL {
            round_trip(PreRunAsk::TimePeriod { hero: 5, time_period });
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
