//! One file read out of a release archive: the archive's central directory names it, its local
//! header leads to its bytes, stored or deflated, and its CRC-32 and size are checked. Enough for
//! the archives `scripts/release-archive.ps1` writes (no ZIP64, no encryption).

use std::io::Read;

/// The end-of-central-directory record's signature and least size.
const END_SIG: u32 = 0x0605_4b50;
const END_LEN: usize = 22;
/// A central directory entry's signature and fixed size.
const ENTRY_SIG: u32 = 0x0201_4b50;
const ENTRY_LEN: usize = 46;
/// A local header's signature and fixed size.
const LOCAL_SIG: u32 = 0x0403_4b50;
const LOCAL_LEN: usize = 30;

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The bytes of the file `name` (at the archive's top, not in a folder) in the zip `zip`.
pub fn read_file(zip: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let broken = || "the archive is damaged".to_owned();
    // the end record: the last signature with room for its record (a comment may follow it)
    let end = (0..=zip.len().checked_sub(END_LEN).ok_or_else(broken)?)
        .rev()
        .take(END_LEN + usize::from(u16::MAX))
        .find(|&at| u32_at(zip, at) == Some(END_SIG))
        .ok_or_else(broken)?;
    let count = u16_at(zip, end + 10).ok_or_else(broken)?;
    let mut at = u32_at(zip, end + 16).ok_or_else(broken)? as usize;
    for _ in 0..count {
        if u32_at(zip, at) != Some(ENTRY_SIG) {
            return Err(broken());
        }
        let field = |off: usize| u16_at(zip, at + off).map(usize::from).ok_or_else(broken);
        let (flags, method) = (field(8)?, field(10)?);
        let crc = u32_at(zip, at + 16).ok_or_else(broken)?;
        let packed = u32_at(zip, at + 20).ok_or_else(broken)?;
        let size = u32_at(zip, at + 24).ok_or_else(broken)?;
        let (name_len, extra_len, comment_len) = (field(28)?, field(30)?, field(32)?);
        let local = u32_at(zip, at + 42).ok_or_else(broken)? as usize;
        let entry_name = zip
            .get(at + ENTRY_LEN..at + ENTRY_LEN + name_len)
            .ok_or_else(broken)?;
        at += ENTRY_LEN + name_len + extra_len + comment_len;
        if entry_name != name.as_bytes() {
            continue;
        }
        if flags & 1 != 0 {
            return Err(format!("{name} is encrypted in the archive"));
        }
        if packed == u32::MAX || size == u32::MAX {
            return Err(format!("{name} is too large to read from the archive"));
        }
        if u32_at(zip, local) != Some(LOCAL_SIG) {
            return Err(broken());
        }
        let local_name = u16_at(zip, local + 26).ok_or_else(broken)? as usize;
        let local_extra = u16_at(zip, local + 28).ok_or_else(broken)? as usize;
        let start = local + LOCAL_LEN + local_name + local_extra;
        let data = zip.get(start..start + packed as usize).ok_or_else(broken)?;
        let bytes = match method {
            0 => data.to_vec(),
            8 => {
                let mut out = Vec::with_capacity(size as usize);
                flate2::read::DeflateDecoder::new(data)
                    .take(u64::from(size) + 1)
                    .read_to_end(&mut out)
                    .map_err(|_| broken())?;
                out
            }
            m => return Err(format!("{name} is packed in a way the game does not read ({m})")),
        };
        if bytes.len() != size as usize || crc32fast::hash(&bytes) != crc {
            return Err(broken());
        }
        return Ok(bytes);
    }
    Err(format!("the archive holds no {name}"))
}

/// A zip of `files` (name, bytes), each deflated or stored, as a test's archive.
#[cfg(test)]
pub(crate) fn test_zip(files: &[(&str, &[u8], bool)]) -> Vec<u8> {
    use std::io::Write;
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, bytes, deflate) in files {
        let data = if *deflate {
            let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
            e.write_all(bytes).unwrap();
            e.finish().unwrap()
        } else {
            bytes.to_vec()
        };
        let method: u16 = if *deflate { 8 } else { 0 };
        let crc = crc32fast::hash(bytes);
        let local = out.len() as u32;
        out.extend(LOCAL_SIG.to_le_bytes());
        out.extend([20u16, 0, method, 0, 0].iter().flat_map(|v| v.to_le_bytes()));
        out.extend(crc.to_le_bytes());
        out.extend((data.len() as u32).to_le_bytes());
        out.extend((bytes.len() as u32).to_le_bytes());
        out.extend((name.len() as u16).to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(name.as_bytes());
        out.extend(&data);
        central.extend(ENTRY_SIG.to_le_bytes());
        central.extend([20u16, 20, 0, method, 0, 0].iter().flat_map(|v| v.to_le_bytes()));
        central.extend(crc.to_le_bytes());
        central.extend((data.len() as u32).to_le_bytes());
        central.extend((bytes.len() as u32).to_le_bytes());
        central.extend([name.len() as u16, 0, 0, 0, 0].iter().flat_map(|v| v.to_le_bytes()));
        central.extend(0u32.to_le_bytes());
        central.extend(local.to_le_bytes());
        central.extend(name.as_bytes());
    }
    let at = out.len() as u32;
    out.extend(&central);
    out.extend(END_SIG.to_le_bytes());
    let n = files.len() as u16;
    out.extend([0u16, 0, n, n].iter().flat_map(|v| v.to_le_bytes()));
    out.extend((central.len() as u32).to_le_bytes());
    out.extend(at.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_reads_back_stored_or_deflated() {
        let big = b"Eternal Survivors ".repeat(500);
        let zip = test_zip(&[
            ("README.txt", b"read me", false),
            ("Eternal Survivors Setup.exe", &big, true),
        ]);
        assert_eq!(read_file(&zip, "README.txt").unwrap(), b"read me");
        assert_eq!(read_file(&zip, "Eternal Survivors Setup.exe").unwrap(), big);
        assert!(read_file(&zip, "missing.exe").unwrap_err().contains("holds no"));
    }

    #[test]
    fn a_damaged_archive_is_refused() {
        let mut zip = test_zip(&[("a.exe", b"the setup's bytes", false)]);
        // a changed byte of the file: its CRC no longer matches
        let at = zip.windows(5).position(|w| w == b"setup").unwrap();
        zip[at] ^= 0xff;
        assert!(read_file(&zip, "a.exe").unwrap_err().contains("damaged"));
        assert!(read_file(b"not a zip at all, but long enough to search", "a.exe").is_err());
        assert!(read_file(b"", "a.exe").is_err());
        let full = test_zip(&[("a.exe", b"x", true)]);
        assert!(read_file(&full[..full.len() - 4], "a.exe").is_err(), "a cut archive");
    }
}
