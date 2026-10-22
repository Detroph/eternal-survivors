//! The meter's input, [`MeterFeed`], and the rows the window draws from it: sorted largest first,
//! each with its share of the mode's total and its amount per second.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// What the meter lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MeterMode {
    /// Damage dealt, per hero, pet or spell.
    #[default]
    DamageDone,
    /// Damage received, per source (an enemy's name, or "area" for a hit with no source).
    DamageTaken,
}

impl MeterMode {
    /// Every mode, in the order the arrows walk them.
    pub const ALL: [MeterMode; 2] = [MeterMode::DamageDone, MeterMode::DamageTaken];

    /// The window's title in this mode.
    pub fn title(self) -> &'static str {
        match self {
            MeterMode::DamageDone => "Damage Done",
            MeterMode::DamageTaken => "Damage Taken",
        }
    }

    /// The mode after this one, wrapping.
    pub fn next(self) -> Self {
        self.step(1)
    }

    /// The mode before this one, wrapping.
    pub fn previous(self) -> Self {
        self.step(Self::ALL.len() - 1)
    }

    fn step(self, by: usize) -> Self {
        let at = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[(at + by) % Self::ALL.len()]
    }
}

/// One line of the meter as the feed gives it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeterEntry {
    /// Who or what the line counts: a hero, a pet, a spell, an enemy, "area".
    pub name: String,
    /// The total so far.
    pub amount: u64,
    /// Per second, when the sender knows it better than `amount / elapsed` (a hero who joined
    /// late); `None` divides by the feed's elapsed seconds.
    pub per_second: Option<f32>,
    /// The hero class id from `classes.ron` (`"mage"`), for a bar in that class's colour; `None`
    /// gives the bar a colour of its own, kept by name.
    pub class: Option<String>,
}

impl MeterEntry {
    /// An entry with no class and no per-second of its own.
    pub fn new(name: impl Into<String>, amount: u64) -> Self {
        Self {
            name: name.into(),
            amount,
            ..default()
        }
    }
}

/// The meter's whole input: the current run's totals per mode and how long the run has gone.
/// The protocol fills it; the window only reads it, and its reset button clears it.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct MeterFeed {
    /// Seconds the run has gone, the divisor for per-second.
    pub elapsed: f32,
    pub damage_done: Vec<MeterEntry>,
    pub damage_taken: Vec<MeterEntry>,
}

impl MeterFeed {
    /// The entries of one mode, in the order they were given.
    pub fn entries(&self, mode: MeterMode) -> &[MeterEntry] {
        match mode {
            MeterMode::DamageDone => &self.damage_done,
            MeterMode::DamageTaken => &self.damage_taken,
        }
    }

    /// The entries of one mode, to replace or edit.
    pub fn entries_mut(&mut self, mode: MeterMode) -> &mut Vec<MeterEntry> {
        match mode {
            MeterMode::DamageDone => &mut self.damage_done,
            MeterMode::DamageTaken => &mut self.damage_taken,
        }
    }

    /// Adds `amount` to the entry named `name`, making it if it is new.
    pub fn add(&mut self, mode: MeterMode, name: &str, amount: u64) {
        let entries = self.entries_mut(mode);
        match entries.iter_mut().find(|e| e.name == name) {
            Some(entry) => entry.amount = entry.amount.saturating_add(amount),
            None => entries.push(MeterEntry::new(name, amount)),
        }
    }

    /// Empties every mode and restarts the clock.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// One mode's rows as the window draws them: largest first (ties by name), each with its
    /// share of the mode's total and its amount per second.
    pub fn rows(&self, mode: MeterMode) -> Vec<MeterRow> {
        let entries = self.entries(mode);
        let total: u64 = entries.iter().map(|e| e.amount).sum();
        let top = entries.iter().map(|e| e.amount).max().unwrap_or(0);
        let mut sorted: Vec<&MeterEntry> = entries.iter().collect();
        sorted.sort_by(|a, b| b.amount.cmp(&a.amount).then_with(|| a.name.cmp(&b.name)));
        sorted
            .into_iter()
            .enumerate()
            .map(|(i, e)| MeterRow {
                rank: i + 1,
                name: e.name.clone(),
                amount: e.amount,
                per_second: e.per_second.or_else(|| per_second(e.amount, self.elapsed)),
                percent: (ratio(e.amount, total) * 100.0) as f32,
                fill: ratio(e.amount, top) as f32,
                class: e.class.clone(),
            })
            .collect()
    }
}

/// The reset button was pressed: the feed is already cleared; a sender that keeps its own totals
/// reads this to start them over.
#[derive(Message, Clone, Copy, Debug)]
pub struct MeterReset;

/// One drawn line of the meter.
#[derive(Clone, Debug, PartialEq)]
pub struct MeterRow {
    /// 1 for the largest.
    pub rank: usize,
    pub name: String,
    pub amount: u64,
    pub per_second: Option<f32>,
    /// The share of the mode's total, 0 to 100.
    pub percent: f32,
    /// The bar's length against the largest entry's, 0 to 1.
    pub fill: f32,
    pub class: Option<String>,
}

impl MeterRow {
    /// The left text: `3. Name`.
    pub fn label(&self) -> String {
        format!("{}. {}", self.rank, self.name)
    }

    /// The right text: `13288 (19.4, 28%)`, or `13288 (28%)` with no per-second.
    pub fn numbers(&self) -> String {
        let amount = format_amount(self.amount);
        let percent = format!("{:.0}%", self.percent);
        match self.per_second {
            Some(rate) => format!("{amount} ({}, {percent})", format_rate(rate)),
            None => format!("{amount} ({percent})"),
        }
    }
}

/// Amount per second; none before the run's first second, when the division only shows noise.
fn per_second(amount: u64, elapsed: f32) -> Option<f32> {
    (elapsed >= 1.0).then(|| amount as f32 / elapsed)
}

/// `part` over `whole`, 0 to 1; 0 when the whole is 0.
fn ratio(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

/// A total, whole below 100 000, then in thousands and millions so the line stays short.
pub fn format_amount(amount: u64) -> String {
    match amount {
        0..=99_999 => amount.to_string(),
        100_000..=999_999 => format!("{:.1}k", amount as f64 / 1_000.0),
        _ => format!("{:.2}M", amount as f64 / 1_000_000.0),
    }
}

/// A per-second figure: one decimal below 1 000, whole below 100 000, then thousands.
pub fn format_rate(rate: f32) -> String {
    let rate = rate.max(0.0);
    if rate < 1_000.0 {
        format!("{rate:.1}")
    } else if rate < 100_000.0 {
        format!("{rate:.0}")
    } else {
        format!("{:.1}k", rate / 1_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed() -> MeterFeed {
        MeterFeed {
            elapsed: 10.0,
            damage_done: vec![
                MeterEntry::new("Spell 116", 300),
                MeterEntry::new("Spell 10", 600),
                MeterEntry::new("Spell 5143", 100),
            ],
            damage_taken: vec![MeterEntry::new("area", 50)],
        }
    }

    #[test]
    fn rows_run_largest_first_with_ranks() {
        let rows = feed().rows(MeterMode::DamageDone);
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Spell 10", "Spell 116", "Spell 5143"]);
        let ranks: Vec<usize> = rows.iter().map(|r| r.rank).collect();
        assert_eq!(ranks, [1, 2, 3]);
    }

    #[test]
    fn equal_amounts_sort_by_name() {
        let mut feed = MeterFeed::default();
        feed.add(MeterMode::DamageTaken, "Wolf", 10);
        feed.add(MeterMode::DamageTaken, "Bear", 10);
        let rows = feed.rows(MeterMode::DamageTaken);
        assert_eq!(rows[0].name, "Bear");
        assert_eq!(rows[1].name, "Wolf");
    }

    #[test]
    fn percent_is_the_share_of_the_mode_total() {
        let rows = feed().rows(MeterMode::DamageDone);
        let percents: Vec<f32> = rows.iter().map(|r| r.percent).collect();
        assert_eq!(percents, [60.0, 30.0, 10.0]);
        let fills: Vec<f32> = rows.iter().map(|r| r.fill).collect();
        assert_eq!(fills[..2], [1.0, 0.5], "bars measure against the largest");
        assert!((fills[2] - 1.0 / 6.0).abs() < 1e-6);
        let other = feed().rows(MeterMode::DamageTaken);
        assert_eq!(other[0].percent, 100.0, "each mode has its own total");
    }

    #[test]
    fn nothing_dealt_is_zero_percent() {
        let mut feed = MeterFeed::default();
        feed.damage_done.push(MeterEntry::new("Idle", 0));
        let rows = feed.rows(MeterMode::DamageDone);
        assert_eq!(rows[0].percent, 0.0);
        assert_eq!(rows[0].fill, 0.0);
        assert!(feed.rows(MeterMode::DamageTaken).is_empty());
    }

    #[test]
    fn per_second_divides_by_elapsed_unless_given() {
        let mut feed = feed();
        feed.damage_done[0].per_second = Some(42.5);
        let rows = feed.rows(MeterMode::DamageDone);
        assert_eq!(rows[0].per_second, Some(60.0), "600 over 10 s");
        assert_eq!(
            rows[1].per_second,
            Some(42.5),
            "the sender's own figure wins"
        );
        feed.elapsed = 0.5;
        let early = feed.rows(MeterMode::DamageDone);
        assert_eq!(early[0].per_second, None, "no rate in the first second");
    }

    #[test]
    fn add_sums_by_name_and_clear_empties() {
        let mut feed = feed();
        feed.add(MeterMode::DamageDone, "Spell 116", 50);
        feed.add(MeterMode::DamageDone, "Spell 120", 5);
        assert_eq!(feed.damage_done[0].amount, 350);
        assert_eq!(feed.damage_done.len(), 4);
        feed.clear();
        assert_eq!(feed, MeterFeed::default());
    }

    #[test]
    fn row_text_reads_like_the_window() {
        let rows = feed().rows(MeterMode::DamageDone);
        assert_eq!(rows[0].label(), "1. Spell 10");
        assert_eq!(rows[0].numbers(), "600 (60.0, 60%)");
        let mut no_rate = rows[2].clone();
        no_rate.per_second = None;
        assert_eq!(no_rate.numbers(), "100 (10%)");
    }

    #[test]
    fn big_numbers_shorten() {
        assert_eq!(format_amount(13_288), "13288");
        assert_eq!(format_amount(99_999), "99999");
        assert_eq!(format_amount(153_240), "153.2k");
        assert_eq!(format_amount(2_345_678), "2.35M");
        assert_eq!(format_rate(19.44), "19.4");
        assert_eq!(format_rate(4_321.6), "4322");
        assert_eq!(format_rate(123_456.0), "123.5k");
        assert_eq!(format_rate(-3.0), "0.0");
    }

    #[test]
    fn modes_cycle_both_ways() {
        assert_eq!(MeterMode::DamageDone.next(), MeterMode::DamageTaken);
        assert_eq!(MeterMode::DamageTaken.next(), MeterMode::DamageDone);
        assert_eq!(MeterMode::DamageDone.previous(), MeterMode::DamageTaken);
        assert_eq!(MeterMode::DamageTaken.title(), "Damage Taken");
    }
}
