//! The talent experience a run paid, revealed step by step: its parts appear one after another,
//! the counter climbs through each as it appears, and the class's bar fills from where it stood
//! before the run toward where it stands now; each talent point crossed on the way is a step of
//! its own for the panel to show. The steps' timing is ours.

use esurv_protocol::Experience;
use esurv_sim::game::rewards::{talent_point_cost, talent_points, MAX_TALENT_POINTS};

/// The first part appears this long after the panel opens.
pub const FIRST_PART_AT: f32 = 0.8;
/// Then one part every this many seconds.
pub const PART_EVERY: f32 = 0.6;
/// A part's amount is counted up over this long.
pub const COUNT_TIME: f32 = 0.45;

/// One part of the talent experience a run pays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperiencePart {
    Time,
    Kills,
    Bosses,
    Victory,
    Zone,
    Options,
}

impl ExperiencePart {
    pub fn label(self) -> &'static str {
        match self {
            ExperiencePart::Time => "Time endured",
            ExperiencePart::Kills => "Enemies",
            ExperiencePart::Bosses => "Bosses",
            ExperiencePart::Victory => "Victory",
            ExperiencePart::Zone => "Zone difficulty",
            ExperiencePart::Options => "Challenge settings",
        }
    }
}

/// The parts paid, in the order they are shown; a part that paid nothing is left out.
pub fn paid_parts(x: &Experience) -> Vec<(ExperiencePart, u32)> {
    [
        (ExperiencePart::Time, x.time),
        (ExperiencePart::Kills, x.kills),
        (ExperiencePart::Bosses, x.bosses),
        (ExperiencePart::Victory, x.victory),
        (ExperiencePart::Zone, x.zone),
        (ExperiencePart::Options, x.options),
    ]
    .into_iter()
    .filter(|&(_, amount)| amount > 0)
    .collect()
}

/// Where a class stands with `total` talent experience: its points, the experience into the next
/// point and that point's cost (0 once it has them all).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    pub points: u32,
    pub into_next: u32,
    pub next_cost: u32,
}

impl Standing {
    pub fn of(total: u32) -> Self {
        let (points, into_next) = talent_points(total);
        let next_cost = if points < MAX_TALENT_POINTS {
            talent_point_cost(points + 1)
        } else {
            0
        };
        Self {
            points,
            into_next,
            next_cost,
        }
    }

    /// How full the bar toward the next point is, 0 to 1 (full once every point is earned).
    pub fn fill(&self) -> f32 {
        if self.next_cost == 0 {
            1.0
        } else {
            (self.into_next as f32 / self.next_cost as f32).clamp(0.0, 1.0)
        }
    }
}

/// The reveal of one payment.
#[derive(Debug, Clone, PartialEq)]
pub struct Reveal {
    /// The class's talent experience before the payment.
    pub before: u32,
    pub parts: Vec<(ExperiencePart, u32)>,
}

/// What the reveal shows at one moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RevealFrame {
    /// How many parts are shown.
    pub parts_shown: usize,
    /// The counter: the experience counted so far.
    pub counted: u32,
    /// Where the class stands with what is counted.
    pub standing: Standing,
    /// Talent points gained so far.
    pub points_gained: u32,
    /// Everything is shown.
    pub done: bool,
}

impl Reveal {
    pub fn new(before: u32, x: &Experience) -> Self {
        Self {
            before,
            parts: paid_parts(x),
        }
    }

    /// Everything the payment brought.
    pub fn total(&self) -> u32 {
        self.parts.iter().map(|&(_, a)| a).sum()
    }

    /// When the last part is counted.
    pub fn length(&self) -> f32 {
        if self.parts.is_empty() {
            0.0
        } else {
            FIRST_PART_AT + (self.parts.len() - 1) as f32 * PART_EVERY + COUNT_TIME
        }
    }

    /// When part `i` appears.
    pub fn part_at(i: usize) -> f32 {
        FIRST_PART_AT + i as f32 * PART_EVERY
    }

    /// The reveal `t` seconds after the panel opened.
    pub fn at(&self, t: f32) -> RevealFrame {
        let mut parts_shown = 0;
        let mut counted = 0.0f64;
        for (i, &(_, amount)) in self.parts.iter().enumerate() {
            let since = t - Self::part_at(i);
            if since < 0.0 {
                break;
            }
            parts_shown += 1;
            let share = (since / COUNT_TIME).min(1.0);
            counted += f64::from(amount) * f64::from(share);
        }
        let done = t >= self.length();
        let counted = if done {
            self.total()
        } else {
            (counted.floor() as u32).min(self.total())
        };
        let start = Standing::of(self.before);
        let standing = Standing::of(self.before.saturating_add(counted));
        RevealFrame {
            parts_shown,
            counted,
            standing,
            points_gained: standing.points - start.points,
            done,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paid() -> Experience {
        Experience {
            time: 198,
            kills: 241,
            bosses: 200,
            victory: 200,
            zone: 0,
            options: 84,
        }
    }

    #[test]
    fn only_the_parts_that_paid_show_in_order() {
        let parts = paid_parts(&paid());
        let shown: Vec<ExperiencePart> = parts.iter().map(|&(p, _)| p).collect();
        use ExperiencePart::*;
        assert_eq!(shown, [Time, Kills, Bosses, Victory, Options]);
        assert!(paid_parts(&Experience::default()).is_empty());
    }

    #[test]
    fn a_standing_counts_points_and_the_way_to_the_next() {
        // points cost 110, 113, 115, ...
        assert_eq!(
            Standing::of(0),
            Standing {
                points: 0,
                into_next: 0,
                next_cost: 110
            }
        );
        let s = Standing::of(110 + 113 + 20);
        assert_eq!((s.points, s.into_next, s.next_cost), (2, 20, 115));
        assert!((s.fill() - 20.0 / 115.0).abs() < 1e-6);
        let all = Standing::of(u32::MAX);
        assert_eq!((all.points, all.next_cost, all.fill()), (51, 0, 1.0));
    }

    #[test]
    fn the_parts_appear_one_by_one_and_the_counter_climbs_through_them() {
        let r = Reveal::new(412, &paid());
        assert_eq!(r.total(), 923);
        let start = r.at(0.0);
        assert_eq!(
            (start.parts_shown, start.counted, start.points_gained),
            (0, 0, 0)
        );
        assert_eq!(start.standing, Standing::of(412));
        assert!(!start.done);
        // the first part, half counted
        let half = r.at(FIRST_PART_AT + COUNT_TIME / 2.0 + 0.001);
        assert_eq!((half.parts_shown, half.counted), (1, 99));
        // the first part counted, the second not yet there
        let one = r.at(FIRST_PART_AT + COUNT_TIME + 0.01);
        assert_eq!((one.parts_shown, one.counted), (1, 198));
        let two = r.at(Reveal::part_at(1));
        assert_eq!((two.parts_shown, two.counted), (2, 198));
        // all of it, and the points it crossed
        let end = r.at(r.length());
        assert!(end.done);
        assert_eq!((end.parts_shown, end.counted), (5, 923));
        assert_eq!(end.standing, Standing::of(412 + 923));
        assert_eq!(
            end.points_gained,
            Standing::of(1335).points - Standing::of(412).points
        );
        assert_eq!(r.at(r.length() + 30.0), end, "it stays done");
    }

    #[test]
    fn each_point_crossed_is_its_own_step() {
        // 70 before: the first point (110) comes 40 into the payment, the second (113) 153 into it
        let r = Reveal::new(
            70,
            &Experience {
                time: 250,
                ..Default::default()
            },
        );
        let mut seen = Vec::new();
        let mut t = 0.0;
        while t <= r.length() + 0.05 {
            let gained = r.at(t).points_gained;
            if seen.last() != Some(&gained) {
                seen.push(gained);
            }
            t += 0.01;
        }
        assert_eq!(seen, [0, 1, 2]);
    }

    #[test]
    fn a_payment_of_nothing_is_done_at_once() {
        let r = Reveal::new(500, &Experience::default());
        assert_eq!(r.length(), 0.0);
        let f = r.at(0.0);
        assert!(f.done);
        assert_eq!((f.parts_shown, f.counted, f.points_gained), (0, 0, 0));
    }
}
