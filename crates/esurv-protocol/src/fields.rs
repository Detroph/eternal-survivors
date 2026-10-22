//! A payload's tokens: written one after another with a space between, read back in the same
//! order. Text is escaped so it is always one token.

use std::fmt::Display;
use std::str::FromStr;

/// Stands for an empty text, or for nothing where a value may be missing.
const NOTHING: &str = "~";
/// The longest list read back; a longer count is refused (a bad line, not a big message).
const MAX_LIST: usize = 4096;

/// Whether `b` is written as `%XX` in a text token.
fn escaped(b: u8) -> bool {
    matches!(b, b'%' | b'~' | b'|') || b <= b' ' || b == 0x7F
}

/// `t` as one token: `~` when empty, else with `%`, `~`, `|`, space and control bytes as `%XX`.
pub fn escape(t: &str) -> String {
    if t.is_empty() {
        return NOTHING.into();
    }
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        // every escaped byte is ASCII, so a wider character goes through whole
        if c.is_ascii() && escaped(c as u8) {
            out.push_str(&format!("%{:02X}", c as u8));
        } else {
            out.push(c);
        }
    }
    out
}

/// A text token back as the text it was.
pub fn unescape(token: &str) -> Option<String> {
    if token == NOTHING {
        return Some(String::new());
    }
    let raw = token.as_bytes();
    let mut bytes = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            let hex = token.get(i + 1..i + 3)?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            bytes.push(raw[i]);
            i += 1;
        }
    }
    String::from_utf8(bytes).ok()
}

/// Writes a payload's tokens.
#[derive(Default)]
pub struct Out(String);

impl Out {
    pub fn new() -> Self {
        Self::default()
    }

    fn token(&mut self, t: &str) -> &mut Self {
        if !self.0.is_empty() {
            self.0.push(' ');
        }
        self.0.push_str(t);
        self
    }

    /// A keyword of the format (never escaped; it has no space).
    pub fn word(&mut self, w: &str) -> &mut Self {
        self.token(w)
    }

    /// A number, as Rust prints it (a decimal reads back exactly).
    pub fn num(&mut self, v: impl Display) -> &mut Self {
        self.token(&v.to_string())
    }

    pub fn guid(&mut self, g: u64) -> &mut Self {
        self.token(&format!("{g:X}"))
    }

    pub fn flag(&mut self, b: bool) -> &mut Self {
        self.token(if b { "1" } else { "0" })
    }

    pub fn text(&mut self, t: &str) -> &mut Self {
        self.token(&escape(t))
    }

    /// A number that may be missing (`~`).
    pub fn opt(&mut self, v: Option<impl Display>) -> &mut Self {
        match v {
            Some(v) => self.num(v),
            None => self.token(NOTHING),
        }
    }

    /// A list: its length, then each item as `item` writes it.
    pub fn list<T>(&mut self, items: &[T], mut item: impl FnMut(&mut Self, &T)) -> &mut Self {
        self.num(items.len());
        for i in items {
            item(self, i);
        }
        self
    }

    pub fn finish(&mut self) -> String {
        std::mem::take(&mut self.0)
    }
}

/// Reads a payload's tokens back, in the order they were written.
pub struct In<'a>(std::iter::Filter<std::str::Split<'a, char>, fn(&&str) -> bool>);

impl<'a> In<'a> {
    pub fn new(payload: &'a str) -> Self {
        Self(payload.split(' ').filter(|t| !t.is_empty()))
    }

    pub fn word(&mut self) -> Option<&'a str> {
        self.0.next()
    }

    pub fn num<T: FromStr>(&mut self) -> Option<T> {
        self.word()?.parse().ok()
    }

    pub fn guid(&mut self) -> Option<u64> {
        u64::from_str_radix(self.word()?, 16).ok()
    }

    pub fn flag(&mut self) -> Option<bool> {
        match self.word()? {
            "1" => Some(true),
            "0" => Some(false),
            _ => None,
        }
    }

    pub fn text(&mut self) -> Option<String> {
        unescape(self.word()?)
    }

    pub fn opt<T: FromStr>(&mut self) -> Option<Option<T>> {
        match self.word()? {
            NOTHING => Some(None),
            t => t.parse().ok().map(Some),
        }
    }

    pub fn list<T>(&mut self, mut item: impl FnMut(&mut Self) -> Option<T>) -> Option<Vec<T>> {
        let n: usize = self.num()?;
        if n > MAX_LIST {
            return None;
        }
        (0..n).map(|_| item(self)).collect()
    }

    /// Whether every token was read (a line with more is from a newer format and still reads).
    #[cfg(test)]
    pub fn done(&mut self) -> bool {
        self.0.next().is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_always_one_token_and_reads_back() {
        for t in [
            "",
            "~",
            "Hogger",
            "Morgan the Collector",
            "Hero's Rest",
            "100% | ~ tab\there",
            "Señor Größe",
            "%41",
        ] {
            let token = escape(t);
            assert!(!token.contains(' ') && !token.contains('\t') && !token.is_empty());
            assert_eq!(unescape(&token).as_deref(), Some(t), "{token}");
        }
        assert_eq!(escape("a b"), "a%20b");
        assert_eq!(unescape("%4"), None, "a cut escape");
    }

    #[test]
    fn tokens_read_back_in_order() {
        let mut out = Out::new();
        out.word("hud")
            .guid(0xF130_0000_0000_0007)
            .num(0.1f32)
            .flag(true)
            .text("two words")
            .opt(None::<u32>)
            .opt(Some(5u32))
            .list(&[1u32, 2, 3], |o, v| {
                o.num(v);
            });
        let line = out.finish();
        let mut r = In::new(&line);
        assert_eq!(r.word(), Some("hud"));
        assert_eq!(r.guid(), Some(0xF130_0000_0000_0007));
        assert_eq!(r.num::<f32>(), Some(0.1));
        assert_eq!(r.flag(), Some(true));
        assert_eq!(r.text().as_deref(), Some("two words"));
        assert_eq!(r.opt::<u32>(), Some(None));
        assert_eq!(r.opt::<u32>(), Some(Some(5)));
        assert_eq!(r.list(|r| r.num::<u32>()), Some(vec![1, 2, 3]));
        assert!(r.done());
        assert_eq!(In::new("99999999 1").list(|r| r.num::<u32>()), None);
    }
}
