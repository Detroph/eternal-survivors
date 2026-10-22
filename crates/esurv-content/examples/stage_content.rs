//! Stages the content folder for a release (design step 9): every file copied, each RON file
//! without its comments (they document the files for us and may name what the player's client
//! holds; the game never reads them). Then it checks what it wrote: each stripped RON file reads
//! as the same value as its original, and the staged folder loads as the game loads it. Run by
//! `scripts\release-archive.ps1`.
//!
//! `cargo run --release -p esurv-content --example stage_content -- <content folder> <staged folder>`
//!
//! The exit code: 0 staged and checked, 1 a file that differs or a folder that does not load.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

/// `text` without its comments (`// ...` to the line's end, `/* ... */` nested), never touching a
/// string (`"..."` with its escapes, raw `r#"..."#`) or a char (`'x'`). A line that held only a
/// comment goes; a line's trailing blanks left by a comment go too.
fn strip_comments(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut line_had_comment = false;
    let mut line_start = 0; // where the current line begins in `out`
    while i < b.len() {
        let c = b[i];
        if c == b'"' || (c == b'r' && matches!(b.get(i + 1), Some(b'"' | b'#')) && !prev_is_word(b, i)) {
            // a string, raw or not: copied whole
            let end = string_end(b, i);
            out.push_str(&text[i..end]);
            i = end;
        } else if c == b'\'' && b.get(i + 2) == Some(&b'\'') || (c == b'\'' && b.get(i + 1) == Some(&b'\\')) {
            // a char literal: copied whole
            let end = text[i + 1..].find('\'').map_or(b.len(), |e| i + 1 + e + 1);
            let end = if b.get(i + 1) == Some(&b'\\') { text[i + 3..].find('\'').map_or(b.len(), |e| i + 3 + e + 1) } else { end };
            out.push_str(&text[i..end]);
            i = end;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            line_had_comment = true;
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let mut depth = 0;
            while i < b.len() {
                if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    // keep the line breaks a block comment spans, so line numbers in errors hold
                    if b[i] == b'\n' {
                        out.push('\n');
                        line_start = out.len();
                    }
                    i += 1;
                }
            }
            line_had_comment = true;
        } else if c == b'\n' {
            let line = &out[line_start..];
            let trimmed_len = line.trim_end().len();
            if line_had_comment {
                out.truncate(line_start + trimmed_len);
            }
            let blank = line_had_comment && out[line_start..].trim().is_empty();
            if blank {
                out.truncate(line_start);
            } else {
                out.push('\n');
            }
            line_start = out.len();
            line_had_comment = false;
            i += 1;
        } else {
            let ch = text[i..].chars().next().unwrap_or('\0');
            out.push(ch);
            i += ch.len_utf8().max(1);
        }
    }
    if line_had_comment {
        let keep = out[line_start..].trim_end().len();
        out.truncate(line_start + keep);
    }
    out
}

/// Whether the byte before `i` is part of a word (so `r"` there is no raw string's start).
fn prev_is_word(b: &[u8], i: usize) -> bool {
    i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')
}

/// Where the string starting at `i` (`"..."` or `r#"..."#`) ends, just past its closing quote.
fn string_end(b: &[u8], i: usize) -> usize {
    if b[i] == b'r' {
        let hashes = b[i + 1..].iter().take_while(|&&c| c == b'#').count();
        let mut j = i + 1 + hashes + 1;
        while j < b.len() {
            if b[j] == b'"' && b[j + 1..].iter().take(hashes).filter(|&&c| c == b'#').count() == hashes {
                return j + 1 + hashes;
            }
            j += 1;
        }
        return b.len();
    }
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'"' => return j + 1,
            _ => j += 1,
        }
    }
    b.len()
}

/// Every file under `dir`, recursively.
fn files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for e in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = e?.path();
        if path.is_dir() {
            files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn stage(from: &Path, to: &Path) -> Result<(usize, usize)> {
    if to.exists() {
        std::fs::remove_dir_all(to).with_context(|| format!("clearing {}", to.display()))?;
    }
    let mut all = Vec::new();
    files(from, &mut all)?;
    let (mut ron_files, mut removed) = (0, 0);
    for src in &all {
        let rel = src.strip_prefix(from)?;
        let dst = to.join(rel);
        std::fs::create_dir_all(dst.parent().unwrap_or(to))?;
        if src.extension().is_some_and(|x| x == "ron") {
            let text = std::fs::read_to_string(src).with_context(|| format!("reading {}", src.display()))?;
            let stripped = strip_comments(&text);
            let before: ron::Value = ron::from_str(&text).with_context(|| format!("parsing {}", src.display()))?;
            let after: ron::Value = ron::from_str(&stripped).with_context(|| format!("parsing the stripped {}", rel.display()))?;
            if before != after {
                bail!("{}: stripping its comments changed what it holds", rel.display());
            }
            removed += text.len() - stripped.len();
            ron_files += 1;
            std::fs::write(&dst, stripped)?;
        } else {
            std::fs::copy(src, &dst)?;
        }
    }
    Ok((ron_files, removed))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [from, to] = args.as_slice() else {
        eprintln!("usage: stage_content <content folder> <staged folder>");
        std::process::exit(2);
    };
    let (from, to) = (Path::new(from), Path::new(to));
    let staged = stage(from, to).and_then(|n| {
        let c = esurv_content::Content::load(to).with_context(|| format!("loading the staged {}", to.display()))?;
        Ok((n, c.abilities.len(), c.zones.len()))
    });
    match staged {
        Ok(((files, removed), abilities, zones)) => {
            println!(
                "stage_content: {files} RON files staged without comments ({removed} bytes fewer), each the same value as its original; the staged folder loads ({abilities} spells, {zones} zones)"
            );
        }
        Err(e) => {
            eprintln!("stage_content: FAIL, {e:#}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::strip_comments;

    #[test]
    fn comments_go_and_strings_stay() {
        let text = "// head\n(\n    a: \"x // not a comment\", // tail\n    b: \"say \\\"// no\\\"\",\n    /* block\n */ c: r#\"raw // kept\"#,\n    d: '/',\n)\n";
        let out = strip_comments(text);
        // a block comment keeps its line breaks (and the blanks before it)
        let expected = "(\n    a: \"x // not a comment\",\n    b: \"say \\\"// no\\\"\",\n    \n c: r#\"raw // kept\"#,\n    d: '/',\n)\n";
        assert_eq!(out, expected);
    }
}
