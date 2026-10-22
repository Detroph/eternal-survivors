//! The messages between Eternal Survivors's server and its client app (design step 6): what the server
//! tells the game's own screens ([`ToClient`]) and what they ask of it ([`ToServer`]). 1.12 has no
//! packet of our own, so they travel as addon chat lines (`CHAT_MSG_ADDON`, language
//! `LANG_ADDON`), which the client hands to its interface and never prints.
//!
//! A line is `prefix` TAB `message`. Every prefix starts with `ESURV`. Two are older lines with
//! layouts of their own, kept as they are: [`NUMBERS_PREFIX`] (the character pane's numbers) and
//! [`LOOK_SIZE_PREFIX`] (an area look's size). Each [`ToClient`] message has a prefix of its own
//! (one more letter); every [`ToServer`] request goes under [`REQUEST_PREFIX`], a verb first, and
//! every ask before a run ([`PreRunAsk`], the character screen's) under [`PRE_RUN_PREFIX`].
//!
//! A message is tokens with one space between them: whole numbers, decimals (as Rust prints them,
//! which read back exactly), guids in hex, keywords, and text, where `%`, `~`, `|`, the space and
//! control characters are written `%XX` and an empty text is `~`; `~` is also a missing number. A
//! list is its length, then its items.
//!
//! A line stays within [`MAX_LINE`] bytes, well under the 255 a 1.12 chat line carries. A longer
//! message goes in pieces, one after another: every piece but the last under its prefix with `+`
//! added, the last under the prefix itself; the pieces' messages joined in order are the whole
//! message ([`frame`], [`Joiner`]).

mod fields;
mod prerun;
mod to_client;
mod to_server;

pub use prerun::{PreRunAsk, SeenView, PRE_RUN_PREFIX};
pub use to_client::{
    Banner, BossBar, BossTimer, BossWarning, BuffKind, BuffTip, Choices, Ending, Experience, GearDrop, GroundIndicator, HeroFacts, Hud, IndicatorShape, Kit, LookStyle, SheathHold,
    KnownSpell, Meter, MeterEntry, Offer, OfferKind, OfferValue, Phase, PrestigeProgress, Progress, Ready, RunEnd,
    ProcOrigin, SaveFileFacts, ScreenKind, Source, SpellTip, TipKind, ToClient, Worn,
};
pub use to_server::{RunOptions, Screens, ToServer};

/// The protocol's version, sent with `hello`.
pub const VERSION: u32 = 1;
/// The run's numbers for the character pane: `key=value;` pairs (esurv-server's
/// `present::stats_line`).
pub const NUMBERS_PREFIX: &str = "ESURV";
/// An area look's size, just before the cast it sizes: `<caster guid, hex> <spell> <scale>`.
pub const LOOK_SIZE_PREFIX: &str = "ESURVS";
/// Every request from the client.
pub const REQUEST_PREFIX: &str = "ESURVR";
/// The longest line: prefix, TAB and message, in bytes.
pub const MAX_LINE: usize = 200;
/// The longest whole message a [`Joiner`] gathers; a longer one is dropped.
pub const MAX_MESSAGE: usize = 64 * 1024;

/// The most heroes a save holds, and of either faction (a camp of the
/// hero picker holds nine): the server refuses a hero past them, the picker offers Create only
/// below them.
pub const MAX_HEROES: usize = 18;
pub const MAX_HEROES_PER_FACTION: usize = 9;
/// Added to a prefix on every piece of a message but the last.
const MORE: char = '+';

/// One addon chat line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub prefix: String,
    pub message: String,
}

impl std::fmt::Display for Line {
    /// The line as chat carries it: `prefix` TAB `message`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\t{}", self.prefix, self.message)
    }
}

/// The lines `message` goes out as under `prefix`: one, or pieces of at most [`MAX_LINE`] bytes.
pub fn frame(prefix: &str, message: &str) -> Vec<Line> {
    // what is left for the message in a line: the prefix, the `+` and the TAB taken off
    let room = MAX_LINE.saturating_sub(prefix.len() + 2).max(4);
    let mut out = Vec::new();
    let mut rest = message;
    while rest.len() > room {
        let mut cut = room;
        while !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        out.push(Line {
            prefix: format!("{prefix}{MORE}"),
            message: rest[..cut].into(),
        });
        rest = &rest[cut..];
    }
    out.push(Line {
        prefix: prefix.into(),
        message: rest.into(),
    });
    out
}

/// Gathers the pieces of messages back into whole ones, prefix by prefix.
#[derive(Debug, Default)]
pub struct Joiner {
    /// What came so far of each prefix's message; `None` once it grew past [`MAX_MESSAGE`].
    pending: Vec<(String, Option<String>)>,
}

impl Joiner {
    /// Takes one chat line (`prefix` TAB `message`): the whole message when this was its last
    /// piece (or its only line), else nothing yet.
    pub fn take(&mut self, line: &str) -> Option<Line> {
        let (prefix, message) = line.split_once('\t')?;
        if let Some(base) = prefix.strip_suffix(MORE) {
            let i = match self.pending.iter().position(|(p, _)| p == base) {
                Some(i) => i,
                None => {
                    self.pending.push((base.into(), Some(String::new())));
                    self.pending.len() - 1
                }
            };
            let slot = &mut self.pending[i].1;
            if let Some(so_far) = slot {
                if so_far.len() + message.len() > MAX_MESSAGE {
                    *slot = None;
                } else {
                    so_far.push_str(message);
                }
            }
            return None;
        }
        let before = match self.pending.iter().position(|(p, _)| p == prefix) {
            Some(i) => self.pending.swap_remove(i).1?,
            None => String::new(),
        };
        Some(Line {
            prefix: prefix.into(),
            message: before + message,
        })
    }

    /// Forgets every message half come (a new session).
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

/// A message of this protocol: its prefix and its message text, and back.
pub trait Wire: Sized {
    fn prefix(&self) -> &'static str;
    /// The whole message, before it is cut into lines.
    fn message(&self) -> String;
    /// The message under `prefix` with `message`; `None` if it is not one (or not this version's).
    fn decode(prefix: &str, message: &str) -> Option<Self>;

    /// The lines it goes out as.
    fn lines(&self) -> Vec<Line> {
        frame(self.prefix(), &self.message())
    }
}

/// Reads one kind of message from chat lines, joining pieces.
#[derive(Debug)]
pub struct Reader<M> {
    joiner: Joiner,
    _kind: std::marker::PhantomData<M>,
}

impl<M> Default for Reader<M> {
    fn default() -> Self {
        Self {
            joiner: Joiner::default(),
            _kind: std::marker::PhantomData,
        }
    }
}

impl<M: Wire> Reader<M> {
    /// Takes one chat line (`prefix` TAB `message`); a message once it is whole. Lines of other
    /// prefixes (other addons, the older `ESURV` lines) give nothing.
    pub fn read(&mut self, line: &str) -> Option<M> {
        if !line.starts_with(NUMBERS_PREFIX) {
            return None;
        }
        let whole = self.joiner.take(line)?;
        M::decode(&whole.prefix, &whole.message)
    }

    /// Forgets every message half come (a new session).
    pub fn clear(&mut self) {
        self.joiner.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_message_goes_in_pieces_that_join_back() {
        let message: String = (0..300).map(|i| format!("{i} ")).collect::<String>() + "é end";
        let lines = frame("ESURVE", &message);
        assert!(lines.len() > 1);
        for l in &lines {
            assert!(
                l.to_string().len() <= MAX_LINE,
                "{} bytes",
                l.to_string().len()
            );
        }
        assert!(lines[..lines.len() - 1]
            .iter()
            .all(|l| l.prefix == "ESURVE+"));
        assert_eq!(lines.last().unwrap().prefix, "ESURVE");
        let mut j = Joiner::default();
        let mut whole = None;
        for l in &lines {
            assert!(whole.is_none(), "whole only at the last piece");
            whole = j.take(&l.to_string());
        }
        assert_eq!(
            whole,
            Some(Line {
                prefix: "ESURVE".into(),
                message
            })
        );
    }

    #[test]
    fn pieces_of_two_prefixes_do_not_mix() {
        let a = frame("ESURVC", &"a".repeat(500));
        let b = frame("ESURVM", &"b".repeat(500));
        let mut j = Joiner::default();
        let mut got = Vec::new();
        for l in a
            .iter()
            .zip(&b)
            .flat_map(|(x, y)| [x, y])
            .chain(&a[a.len().min(b.len())..])
        {
            got.extend(j.take(&l.to_string()));
        }
        assert_eq!(got.len(), 2);
        assert!(got
            .iter()
            .any(|l| l.prefix == "ESURVC" && l.message == "a".repeat(500)));
        assert!(got
            .iter()
            .any(|l| l.prefix == "ESURVM" && l.message == "b".repeat(500)));
    }

    #[test]
    fn a_message_past_the_limit_is_dropped() {
        let mut j = Joiner::default();
        let piece = format!("ESURVE+\t{}", "x".repeat(1000));
        for _ in 0..(MAX_MESSAGE / 1000 + 2) {
            assert!(j.take(&piece).is_none());
        }
        assert!(j.take("ESURVE\tend").is_none());
        assert_eq!(
            j.take("ESURVE\tnext").map(|l| l.message),
            Some("next".into()),
            "the next message reads"
        );
    }
}
