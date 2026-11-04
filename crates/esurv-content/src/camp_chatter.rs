//! The camp's life between runs (`camp_chatter.ron`; plans/hub-hero-life.md): the temperaments a
//! hero is given, what each does while it waits at its tent, and the lines it says. Flavour only:
//! nothing here changes a run.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::Content;

pub use esurv_protocol::TimePeriod;

/// The whole file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CampChatter {
    pub temperaments: Vec<Temperament>,
    pub lines: Vec<CampLine>,
    /// Two heroes talking, a line each in turn.
    #[serde(default)]
    pub exchanges: Vec<Exchange>,
}

/// A temperament: a hero's voice and how it passes the time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Temperament {
    /// Unique, lowercase; the save keeps a hero's by it, so it never changes once shipped.
    pub id: String,
    pub name: String,
    /// One line on its voice, for the hero sheet.
    pub voice: String,
    /// A texture name from `Interface\Icons` in the WoW client.
    pub icon: String,
    /// What a hero like this settles into for a while, besides standing at ease, at its tent
    /// (never Sit, Sleep, Work or Ready: sitting and dozing only on a seat or a fire log, the hammering only at an
    /// anvil, the fists up only at a target dummy).
    pub poses: Vec<Pose>,
    /// What it does now and then, a second or two.
    pub gestures: Vec<Gesture>,
}

/// A held pose (the 1.12 client's `Emotes.dbc` states).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pose {
    Stand,
    Sit,
    Kneel,
    Sleep,
    /// The hammering loop: sharpening, tinkering, at an anvil only.
    Work,
    /// Fists up, on guard, facing a target dummy only.
    Ready,
    Dance,
}

/// A one-shot gesture (the 1.12 client's `Emotes.dbc` one-shots).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gesture {
    Wave,
    Salute,
    Bow,
    Cheer,
    Applaud,
    Laugh,
    Flex,
    Roar,
    Eat,
    Point,
    Cry,
    Yes,
    No,
    Question,
    Exclamation,
    Kneel,
    /// Mischievous only.
    Chicken,
}

/// When a line is said.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineWhen {
    /// To nobody in particular.
    Idle,
    /// To the player's hero walking near.
    ToPlayer,
    /// After the player's hero was overrun.
    Loss,
    /// After a zone was won.
    Win,
    /// As a new hero arrives.
    Newcomer,
    /// After dark.
    Night,
}

/// One line a hero may say.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampLine {
    /// A temperament's id.
    pub temperament: String,
    pub when: LineWhen,
    /// Said only by a hero of this race (an id of `races.ron`); none: any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub race: Option<String>,
    /// Said only by a hero of this class (an id of `classes.ron`); none: any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// Said only by a hero pulled in from this time period; none: any time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_period: Option<TimePeriod>,
    pub text: String,
}

/// Two heroes talking: the first says the first line, the second the next, and so on in turn (a
/// hook, a reply, a sting; four lines at most). `{hero}` in a line is the name of the hero the
/// player plays; `{zone}`, in a `Win` or `Loss` exchange, the zone its run was in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Exchange {
    /// `Idle`, or after the player's hero won a zone for the first time (`Win`) or was overrun
    /// (`Loss`).
    pub when: LineWhen,
    pub first: Speaker,
    pub second: Speaker,
    pub lines: Vec<String>,
}

/// Who may speak one side of an exchange: a hero with this temperament (leading or second), of
/// this race and of this class (ids of `races.ron` and `classes.ron`) and pulled in
/// from this time period; none: any.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Speaker {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperament: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub race: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_period: Option<TimePeriod>,
}

impl CampChatter {
    pub fn temperament(&self, id: &str) -> Option<&Temperament> {
        self.temperaments.iter().find(|t| t.id == id)
    }
}

impl Content {
    /// `camp_chatter.ron`'s rules: temperament ids once each, each with a name, voice and icon and
    /// the chicken only for the mischievous; every line of a known temperament, race and class.
    pub(crate) fn check_camp_chatter(&self, problems: &mut Vec<String>) {
        let chatter = &self.camp_chatter;
        let mut seen = HashSet::new();
        for t in &chatter.temperaments {
            if !seen.insert(t.id.as_str()) {
                problems.push(format!("temperament {} is listed twice", t.id));
            }
            if t.name.trim().is_empty() || t.voice.trim().is_empty() || t.icon.trim().is_empty() {
                problems.push(format!("temperament {} needs a name, a voice and an icon", t.id));
            }
            if t.gestures.contains(&Gesture::Chicken) && t.id != "mischievous" {
                problems.push(format!("temperament {}: the chicken is the mischievous one's only", t.id));
            }
            if t.poses.iter().any(|p| matches!(p, Pose::Sit | Pose::Sleep | Pose::Work | Pose::Ready)) {
                problems.push(format!("temperament {}: Sit and Sleep are for a seat or a fire log, Work for an anvil and Ready for a target dummy, not a tent", t.id));
            }
        }
        for l in &chatter.lines {
            if chatter.temperament(&l.temperament).is_none() {
                problems.push(format!("camp line \"{}\" names unknown temperament {}", l.text, l.temperament));
            }
            if l.race.as_ref().is_some_and(|r| !self.races.iter().any(|x| &x.id == r)) {
                problems.push(format!("camp line \"{}\" names an unknown race", l.text));
            }
            if l.class.as_ref().is_some_and(|c| self.class(c).is_none()) {
                problems.push(format!("camp line \"{}\" names an unknown class", l.text));
            }
            if l.text.trim().is_empty() {
                problems.push(format!("a camp line of {} has no words", l.temperament));
            }
        }
        for e in &chatter.exchanges {
            let first = e.lines.first().map_or("", String::as_str);
            if !(2..=4).contains(&e.lines.len()) || e.lines.iter().any(|l| l.trim().is_empty()) {
                problems.push(format!("exchange \"{first}\" needs two to four lines, each with words"));
            }
            if !matches!(e.when, LineWhen::Idle | LineWhen::Win | LineWhen::Loss) {
                problems.push(format!("exchange \"{first}\" is Idle, Win or Loss"));
            }
            for s in [&e.first, &e.second] {
                if s.temperament.as_ref().is_some_and(|t| chatter.temperament(t).is_none())
                    || s.race.as_ref().is_some_and(|r| !self.races.iter().any(|x| &x.id == r))
                    || s.class.as_ref().is_some_and(|c| self.class(c).is_none())
                {
                    problems.push(format!("exchange \"{first}\" names an unknown temperament, race or class"));
                }
            }
            for l in &e.lines {
                let named = l.replace("{hero}", "").replace("{zone}", "");
                if named.contains(['{', '}']) || (l.contains("{zone}") && e.when == LineWhen::Idle) {
                    problems.push(format!("exchange line \"{l}\" names only {{hero}}, and {{zone}} after a run"));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).expect("content loads")
    }

    /// Every race of `races.ron` has lines of its own for each time period, so a hero from any
    /// of them speaks of its own time.
    #[test]
    fn every_race_has_lines_of_its_own_in_every_time_period() {
        let content = content();
        let lines = &content.camp_chatter.lines;
        for race in &content.races {
            for period in TimePeriod::ALL {
                let n = lines.iter().filter(|l| l.race.as_ref() == Some(&race.id) && l.time_period == Some(period)).count();
                assert!(n >= 4, "{} has {n} lines of the {} time period", race.id, period.word());
            }
        }
    }

    /// A line names its time period in the file as `time_period: Some(Past)`; one without is
    /// any time's.
    #[test]
    fn a_line_reads_its_time_period() {
        let line: CampLine = ron::from_str(r#"(temperament: "grim", when: Idle, race: Some("human"), time_period: Some(Past), text: "Hm.")"#).unwrap();
        assert_eq!(line.time_period, Some(TimePeriod::Past));
        let line: CampLine = ron::from_str(r#"(temperament: "grim", when: Idle, text: "Hm.")"#).unwrap();
        assert_eq!(line.time_period, None);
    }
}
