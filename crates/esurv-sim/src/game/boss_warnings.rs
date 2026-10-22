//! Raid-warning announcements for boss mechanics (docs/game/bosses.md,
//! "Warnings and timer bars"): each ability or script line may carry a warning, written in our own
//! words in its zone file. As it starts the client shows it big across the screen in its colour,
//! with its icon and the raid warning sound; an ability's next use and a timed script line count
//! down as timer bars; a health-share line is announced a little before it runs ("soon"). Which of
//! them show is the player's choice (Options).

use esurv_content::Warning;

use super::run::{Event, Run};

impl Run {
    /// Announces `w` now, with `look`'s icon unless it names its own.
    pub(crate) fn announce_warning(&mut self, w: &Warning, look: u32) {
        self.push(Event::Warning { text: w.text.clone(), colour: w.colour, icon_spell: w.icon_spell.unwrap_or(look), soon: false });
    }
}
