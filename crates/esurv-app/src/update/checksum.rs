//! The archive's checksum: its `.sha256` file holds the archive's SHA-256 in hex, then its file
//! name, as `sha256sum` writes it (`<hash>  <name>`; `*<name>` and a bare hash read too).

use sha2::{Digest, Sha256};

/// The SHA-256 of `bytes`, in lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The hash a checksum file gives for `file_name`, in lower-case hex: the line that names it, or
/// a line with a hash alone.
pub fn expected_hash(text: &str, file_name: &str) -> Result<String, String> {
    for line in text.lines() {
        let mut parts = line.trim().splitn(2, char::is_whitespace);
        let hash = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default().trim().trim_start_matches('*');
        let is_hash = hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit());
        if is_hash && (name.is_empty() || name == file_name) {
            return Ok(hash.to_ascii_lowercase());
        }
    }
    Err(format!("the checksum file gives no hash for {file_name}"))
}

/// `Ok` when `bytes` hash as the checksum file says `file_name` does.
pub fn verify(bytes: &[u8], checksum_file: &str, file_name: &str) -> Result<(), String> {
    let expected = expected_hash(checksum_file, file_name)?;
    if sha256_hex(bytes) == expected {
        Ok(())
    } else {
        Err(format!("{file_name} does not match its checksum"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The SHA-256 of "abc", from FIPS 180-2.
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn the_hash_is_sha256_in_hex() {
        assert_eq!(sha256_hex(b"abc"), ABC);
    }

    #[test]
    fn a_checksum_file_reads_as_sha256sum_writes_it() {
        let name = "eternal-survivors-v0.5.0.zip";
        assert_eq!(expected_hash(&format!("{ABC}  {name}\n"), name).unwrap(), ABC);
        assert_eq!(expected_hash(&format!("{ABC} *{name}\r\n"), name).unwrap(), ABC);
        assert_eq!(expected_hash(&ABC.to_ascii_uppercase(), name).unwrap(), ABC, "a bare hash");
        assert!(expected_hash(&format!("{ABC}  other.zip"), name).is_err(), "another file's");
        assert!(expected_hash("not a hash  x.zip", name).is_err());
        assert!(expected_hash("", name).is_err());
    }

    #[test]
    fn a_download_that_does_not_match_its_checksum_is_refused() {
        let name = "a.zip";
        let file = format!("{ABC}  {name}\n");
        assert!(verify(b"abc", &file, name).is_ok());
        let why = verify(b"abd", &file, name).unwrap_err();
        assert!(why.contains("does not match"), "{why}");
    }
}
