//! The save's lock: the save file on disk is its text sealed with XChaCha20-Poly1305, so it reads
//! as noise in a text editor and any change to it breaks the seal.
//!
//! A locked file is [`MAGIC`], a fresh 24-byte nonce, then the sealed text. The key is built in:
//! the release build sets `ESURV_SAVE_KEY` (64 hex digits, kept off the repository by
//! `scripts\release-build.ps1`); any other build uses [`DEV_KEY`], so a dev build cannot read a
//! release save nor the other way round.
//!
//! The key lives in the game, so this stops editing by hand, not a reader of the executable; the
//! save's sanity checks (`rein_in`) catch what gets past it.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;

/// What a locked save file starts with.
pub const MAGIC: &[u8] = b"ESURVSAVE1\n";
/// The nonce's length.
const NONCE: usize = 24;
/// The key of builds without `ESURV_SAVE_KEY`: public, like the code.
const DEV_KEY: [u8; 32] = *b"eternal-survivors-dev-save-key!!";

/// What a save file on disk holds.
#[derive(Debug, PartialEq, Eq)]
pub enum Unlocked {
    /// A save from before the lock: plain text.
    Plain(String),
    /// A locked save whose seal holds.
    Locked(String),
    /// A locked save whose seal is broken (changed outside the game), or made with another key.
    Broken,
}

/// The build's key: `ESURV_SAVE_KEY` when it was set at build time, else [`DEV_KEY`].
fn key() -> [u8; 32] {
    option_env!("ESURV_SAVE_KEY").and_then(parse_key).unwrap_or(DEV_KEY)
}

/// 64 hex digits as a key.
fn parse_key(hex: &str) -> Option<[u8; 32]> {
    let hex = hex.trim().as_bytes();
    if hex.len() != 64 {
        return None;
    }
    let mut key = [0u8; 32];
    for (i, pair) in hex.chunks(2).enumerate() {
        key[i] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(key)
}

/// Whether this build reads release saves (it was built with `ESURV_SAVE_KEY`).
pub fn release_key() -> bool {
    option_env!("ESURV_SAVE_KEY").and_then(parse_key).is_some()
}

/// The save text locked, ready to write.
pub fn lock(text: &str) -> Vec<u8> {
    lock_with(&key(), text)
}

fn lock_with(key: &[u8; 32], text: &str) -> Vec<u8> {
    let mut nonce = [0u8; NONCE];
    rand::thread_rng().fill_bytes(&mut nonce);
    let sealed = XChaCha20Poly1305::new(key.into())
        .encrypt(XNonce::from_slice(&nonce), text.as_bytes())
        .expect("sealing never fails for a save's size");
    [MAGIC, &nonce, &sealed].concat()
}

/// What a save file's bytes hold.
pub fn unlock(bytes: &[u8]) -> Unlocked {
    unlock_with(&key(), bytes)
}

fn unlock_with(key: &[u8; 32], bytes: &[u8]) -> Unlocked {
    let Some(rest) = bytes.strip_prefix(MAGIC) else {
        return match std::str::from_utf8(bytes) {
            Ok(text) => Unlocked::Plain(text.to_owned()),
            Err(_) => Unlocked::Broken,
        };
    };
    if rest.len() < NONCE {
        return Unlocked::Broken;
    }
    let (nonce, sealed) = rest.split_at(NONCE);
    XChaCha20Poly1305::new(key.into())
        .decrypt(XNonce::from_slice(nonce), sealed)
        .ok()
        .and_then(|text| String::from_utf8(text).ok())
        .map_or(Unlocked::Broken, Unlocked::Locked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_locked_save_reads_back_and_shows_no_text() {
        let text = "(heroes: [], next_guid: 7, classes: {\"mage\": (talent_experience: 900)})";
        let locked = lock(text);
        assert!(locked.starts_with(MAGIC));
        assert!(!locked.windows(17).any(|w| w == b"talent_experience"), "no text shows through");
        assert_eq!(unlock(&locked), Unlocked::Locked(text.into()));
        assert_ne!(lock(text), locked, "a fresh nonce every write");
    }

    #[test]
    fn a_changed_byte_or_another_key_breaks_the_seal_and_plain_text_is_an_older_save() {
        let mut locked = lock("(heroes: [], next_guid: 1)");
        let last = locked.len() - 1;
        locked[last] ^= 1;
        assert_eq!(unlock(&locked), Unlocked::Broken);
        let other = lock_with(&[7; 32], "(heroes: [], next_guid: 1)");
        assert_eq!(unlock(&other), Unlocked::Broken);
        assert_eq!(unlock(&MAGIC[..5]), Unlocked::Plain("ESURV".into()));
        assert_eq!(unlock(b"(heroes: [])"), Unlocked::Plain("(heroes: [])".into()));
        assert_eq!(unlock(&[0xff, 0xfe, 0x00]), Unlocked::Broken);
    }

    #[test]
    fn a_key_is_64_hex_digits() {
        assert_eq!(parse_key(&"ab".repeat(32)), Some([0xab; 32]));
        assert_eq!(parse_key("abc"), None);
        assert_eq!(parse_key(&"zz".repeat(32)), None);
    }
}
