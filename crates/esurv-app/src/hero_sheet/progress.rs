//! The class's progress as the hero sheet will show it (its Character, Collection and Talents
//! tabs; menus.md), read from the server's messages: the class's progress (`Progress`: its
//! talent experience and points, the ranks bought, the gear won, worn and shown), who the hero is
//! (`Ready`) and the run options' reward bonus (for the drop chances). What the player picked (the
//! tab, a piece of the collection, the zone of the drop tables), the change awaiting the server's
//! answer, and the server's last refusal, said in our words.

use bevy::prelude::*;
use esurv_protocol::{Progress, Ready, ToClient, ToServer};

/// The sheet's tabs, left to right; the drop tables become the Collection's "still to find" list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HeroSheetTab {
    #[default]
    Character,
    Collection,
    DropTables,
    Talents,
}

impl HeroSheetTab {
    pub const ALL: [HeroSheetTab; 4] = [
        HeroSheetTab::Character,
        HeroSheetTab::Collection,
        HeroSheetTab::DropTables,
        HeroSheetTab::Talents,
    ];

    pub fn label(self) -> &'static str {
        match self {
            HeroSheetTab::Character => "Character",
            HeroSheetTab::Collection => "Collection",
            HeroSheetTab::DropTables => "Drop Tables",
            HeroSheetTab::Talents => "Talents",
        }
    }
}

/// A change the player asks of the class's progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgressChange {
    /// One rank more, or one less, of a talent.
    Learn(String),
    Unlearn(String),
    /// Put on a piece of the collection (by item entry), or take off what a slot wears.
    Wear(u32),
    TakeOff(u32),
    /// Show look `look` (a place in the class set's looks) in `slot`, or with `None` the worn piece's own.
    ShowLook {
        slot: u32,
        look: Option<u32>,
    },
}

impl ProgressChange {
    pub fn request(&self) -> ToServer {
        match self {
            ProgressChange::Learn(id) => ToServer::LearnTalent(id.clone()),
            ProgressChange::Unlearn(id) => ToServer::UnlearnTalent(id.clone()),
            ProgressChange::Wear(entry) => ToServer::Wear(*entry),
            ProgressChange::TakeOff(slot) => ToServer::TakeOff(*slot),
            ProgressChange::ShowLook { slot, look } => ToServer::SetLook {
                slot: *slot,
                look: *look,
            },
        }
    }
}

/// What the sheet says when the server refuses a change, by its verb.
pub fn refusal_text(verb: &str) -> Option<&'static str> {
    Some(match verb {
        "learn" => "That talent cannot be learned yet.",
        "unlearn" => "That rank cannot be taken back: other talents rest on it.",
        "prestige" => "No prestige point to spend there.",
        "unprestige" => "No prestige rank to take back there.",
        "resetprestige" => "No prestige point is spent.",
        "wear" => "That piece cannot be worn.",
        "takeoff" => "Nothing is worn there.",
        "look" => "That look cannot be shown.",
        _ => return None,
    })
}

/// What the sheet knows and shows of the class's progress.
#[derive(Resource, Debug)]
pub struct HeroSheetState {
    progress: Option<Progress>,
    hero: Option<Ready>,
    /// The run options' reward bonus: the drop chances grow with it.
    bonus: f32,
    tab: HeroSheetTab,
    /// The piece picked in the collection: (slot, tier).
    picked: Option<(u32, u32)>,
    /// The zone the drop tables show; `None` until one is picked (the hero's own zone).
    zone: Option<usize>,
    /// A request sent and not yet answered: nothing more goes meanwhile.
    pending: Option<ToServer>,
    /// The last refusal, until the next change.
    notice: Option<&'static str>,
    /// Changes whenever what the sheet draws changes.
    version: u32,
}

impl Default for HeroSheetState {
    fn default() -> Self {
        Self {
            progress: None,
            hero: None,
            bonus: 1.0,
            tab: HeroSheetTab::default(),
            picked: None,
            zone: None,
            pending: None,
            notice: None,
            version: 0,
        }
    }
}

impl HeroSheetState {
    pub fn progress(&self) -> Option<&Progress> {
        self.progress.as_ref()
    }

    pub fn hero(&self) -> Option<&Ready> {
        self.hero.as_ref()
    }

    pub fn bonus(&self) -> f32 {
        self.bonus
    }

    pub fn tab(&self) -> HeroSheetTab {
        self.tab
    }

    pub fn picked(&self) -> Option<(u32, u32)> {
        self.picked
    }

    pub fn zone(&self) -> Option<usize> {
        self.zone
    }

    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn notice(&self) -> Option<&'static str> {
        self.notice
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    /// Takes in a server message; `true` when what it shows changed.
    pub fn take(&mut self, m: &ToClient) -> bool {
        match m {
            ToClient::Ready(r) => {
                self.hero = Some(r.clone());
                self.pending = None;
                self.changed();
                true
            }
            ToClient::Progress(p) => {
                if self
                    .progress
                    .as_ref()
                    .is_some_and(|old| old.class != p.class)
                {
                    self.picked = None;
                }
                self.progress = Some(p.clone());
                self.pending = None;
                self.notice = None;
                self.changed();
                true
            }
            ToClient::RunOptions { bonus, .. } => {
                self.bonus = *bonus;
                self.changed();
                true
            }
            ToClient::Refused(verb) => {
                let Some(text) = refusal_text(verb) else {
                    return false;
                };
                if self.pending.as_ref().is_some_and(|r| r.verb() == verb) {
                    self.pending = None;
                }
                self.notice = Some(text);
                self.changed();
                true
            }
            _ => false,
        }
    }

    pub fn choose_tab(&mut self, tab: HeroSheetTab) {
        if self.tab != tab {
            self.tab = tab;
            self.notice = None;
            self.changed();
        }
    }

    /// Picks a piece of the collection (slot, tier).
    pub fn pick(&mut self, slot: u32, tier: u32) {
        if self.picked != Some((slot, tier)) {
            self.picked = Some((slot, tier));
            self.changed();
        }
    }

    /// Shows zone `zone` in the drop tables.
    pub fn show_zone(&mut self, zone: usize) {
        if self.zone != Some(zone) {
            self.zone = Some(zone);
            self.changed();
        }
    }

    /// An action: what to ask the server, unless an answer is awaited or nothing is known yet.
    pub fn ask(&mut self, action: &ProgressChange) -> Option<ToServer> {
        if self.pending.is_some() || self.progress.is_none() {
            return None;
        }
        let request = action.request();
        self.pending = Some(request.clone());
        self.notice = None;
        self.changed();
        Some(request)
    }

    /// Forgets what the server said (the hero left the world): nothing is kept but the sheet's own choices.
    pub fn forget(&mut self) {
        *self = Self {
            tab: self.tab,
            version: self.version.wrapping_add(1),
            ..Default::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(class: &str) -> Progress {
        Progress {
            class: class.into(),
            talent_experience: 300,
            points: 3,
            spent: 1,
            into_next: 15,
            next_cost: 110,
            zones_open: 1,
            zones_discovered: Vec::new(),
            talents: vec![("imp_heroic".into(), 1)],
            worn: vec![],
            looks: vec![],
            collection: vec![101_000],
            prestige: Default::default(),
            loadout: String::new(),
            new_gear: false,
            new_points: false,
            challenge_open: false,
            challenge_news: false,
        }
    }

    #[test]
    fn one_request_at_a_time_and_the_answer_frees_the_next() {
        let mut s = HeroSheetState::default();
        assert_eq!(
            s.ask(&ProgressChange::Wear(101_000)),
            None,
            "nothing known yet"
        );
        s.take(&ToClient::Progress(progress("warrior")));
        assert_eq!(
            s.ask(&ProgressChange::Wear(101_000)),
            Some(ToServer::Wear(101_000))
        );
        assert!(s.pending());
        assert_eq!(
            s.ask(&ProgressChange::TakeOff(0)),
            None,
            "awaiting the answer"
        );
        s.take(&ToClient::Progress(progress("warrior")));
        assert!(!s.pending());
        assert_eq!(
            s.ask(&ProgressChange::Learn("deflection".into())),
            Some(ToServer::LearnTalent("deflection".into()))
        );
    }

    #[test]
    fn a_refusal_is_said_and_frees_the_sheet() {
        let mut s = HeroSheetState::default();
        s.take(&ToClient::Progress(progress("warrior")));
        s.ask(&ProgressChange::Unlearn("imp_heroic".into()));
        assert!(
            !s.take(&ToClient::Refused("reroll".into())),
            "not a progress change"
        );
        assert!(s.pending());
        assert!(s.take(&ToClient::Refused("unlearn".into())));
        assert!(!s.pending());
        assert_eq!(s.notice(), refusal_text("unlearn"));
        s.take(&ToClient::Progress(progress("warrior")));
        assert_eq!(s.notice(), None, "gone with the next change");
    }

    #[test]
    fn every_progress_verb_has_its_refusal() {
        for action in [
            ProgressChange::Learn(String::new()),
            ProgressChange::Unlearn(String::new()),
            ProgressChange::Wear(0),
            ProgressChange::TakeOff(0),
            ProgressChange::ShowLook {
                slot: 0,
                look: None,
            },
        ] {
            assert!(
                refusal_text(action.request().verb()).is_some(),
                "{action:?}"
            );
        }
    }

    #[test]
    fn changes_bump_the_version_and_leaving_keeps_the_tab_only() {
        let mut s = HeroSheetState::default();
        let v = s.version();
        s.choose_tab(HeroSheetTab::Talents);
        s.pick(2, 3);
        s.show_zone(4);
        assert!(s.version() != v);
        assert_eq!((s.picked(), s.zone()), (Some((2, 3)), Some(4)));
        s.take(&ToClient::RunOptions {
            options: esurv_protocol::RunOptions {
                density: 100,
                health: 100,
                damage: 100,
                speed: 110,
                ..Default::default()
            },
            bonus: 1.2,
        });
        assert_eq!(s.bonus(), 1.2);
        s.forget();
        assert_eq!(s.tab(), HeroSheetTab::Talents);
        assert_eq!((s.picked(), s.zone(), s.bonus()), (None, None, 1.0));
        assert!(s.progress().is_none());
    }
}
