//! Lines that show for a while and fade out, newest on top, as the stock 1.12 message frames
//! (`UIErrorsFrame`, `RaidWarningFrame`) show theirs: the HUD's banners and its toasts. A line
//! holds at full strength, then fades; when the frame is full the oldest goes. A line that comes
//! again while it is still up starts over on top instead of showing twice (ours: the server can
//! repeat a message, a cache it cannot pay for, every few seconds).

/// How a line looks: a toast, or one of the big banners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineLook {
    /// A small HUD message.
    Toast,
    /// A boss arrives.
    BossComes,
    /// The boss whose death wins the run arrives.
    FinalBossComes,
    /// A boss falls.
    BossDown,
}

/// One line and when it came, in seconds of the HUD's clock.
#[derive(Clone, Debug, PartialEq)]
pub struct TimedLine {
    pub text: String,
    pub look: LineLook,
    pub since: f32,
}

/// A stack of lines that fade, newest first.
#[derive(Clone, Debug)]
pub struct TimedLines {
    lines: Vec<TimedLine>,
    /// At most this many lines show.
    limit: usize,
    /// Seconds a line holds at full strength, then seconds it takes to fade away.
    hold: f32,
    fade: f32,
}

impl TimedLines {
    pub fn new(limit: usize, hold: f32, fade: f32) -> Self {
        Self {
            lines: Vec::new(),
            limit: limit.max(1),
            hold: hold.max(0.0),
            fade: fade.max(0.0),
        }
    }

    /// A new line on top at `now`; the same text already up moves to the top and starts over.
    pub fn push(&mut self, text: impl Into<String>, look: LineLook, now: f32) {
        let text = text.into();
        self.lines.retain(|l| l.text != text);
        self.lines.insert(
            0,
            TimedLine {
                text,
                look,
                since: now,
            },
        );
        self.lines.truncate(self.limit);
    }

    /// Drops the lines that have faded away by `now`.
    pub fn prune(&mut self, now: f32) {
        let (hold, fade) = (self.hold, self.fade);
        self.lines
            .retain(|l| strength(now - l.since, hold, fade) > 0.0);
    }

    /// The lines up at `now`, newest first, each with its strength (1 full, 0 gone).
    pub fn shown(&self, now: f32) -> Vec<(&TimedLine, f32)> {
        self.lines
            .iter()
            .map(|l| (l, strength(now - l.since, self.hold, self.fade)))
            .filter(|(_, a)| *a > 0.0)
            .collect()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// A line's strength `age` seconds after it came: full while it holds, then down to nothing over
/// the fade. A line from the future (a clock that went back) is full.
pub fn strength(age: f32, hold: f32, fade: f32) -> f32 {
    if age <= hold {
        return 1.0;
    }
    if fade <= 0.0 {
        return 0.0;
    }
    (1.0 - (age - hold) / fade).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_holds_then_fades() {
        assert_eq!(strength(0.0, 2.0, 1.0), 1.0);
        assert_eq!(strength(2.0, 2.0, 1.0), 1.0);
        assert!((strength(2.5, 2.0, 1.0) - 0.5).abs() < 1e-6);
        assert_eq!(strength(3.0, 2.0, 1.0), 0.0);
        assert_eq!(strength(9.0, 2.0, 0.0), 0.0, "no fade: gone at once");
        assert_eq!(strength(-1.0, 2.0, 1.0), 1.0);
    }

    #[test]
    fn newest_on_top_and_the_oldest_goes_when_full() {
        let mut lines = TimedLines::new(2, 3.0, 1.0);
        lines.push("one", LineLook::Toast, 0.0);
        lines.push("two", LineLook::Toast, 0.5);
        lines.push("three", LineLook::Toast, 1.0);
        let shown: Vec<&str> = lines
            .shown(1.0)
            .iter()
            .map(|(l, _)| l.text.as_str())
            .collect();
        assert_eq!(shown, ["three", "two"]);
    }

    #[test]
    fn a_repeated_line_starts_over_on_top() {
        let mut lines = TimedLines::new(3, 3.0, 1.0);
        lines.push("cache", LineLook::Toast, 0.0);
        lines.push("magnet", LineLook::Toast, 1.0);
        lines.push("cache", LineLook::Toast, 3.5);
        let shown = lines.shown(3.5);
        assert_eq!(shown.len(), 2, "shown once");
        assert_eq!(shown[0].0.text, "cache");
        assert_eq!(shown[0].1, 1.0, "at full strength again");
    }

    #[test]
    fn faded_lines_are_dropped() {
        let mut lines = TimedLines::new(4, 1.0, 1.0);
        lines.push("old", LineLook::BossDown, 0.0);
        lines.push("new", LineLook::BossComes, 1.5);
        lines.prune(2.0);
        let shown = lines.shown(2.0);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].0.look, LineLook::BossComes);
        lines.prune(5.0);
        assert!(lines.is_empty());
    }
}
