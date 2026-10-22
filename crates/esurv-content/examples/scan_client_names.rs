//! Scans the repository for names the player's 1.12 client holds, written as plain text (design
//! step 9: the client's names are pointers, read when the game loads). It reads the client's name
//! tables (`Spell.dbc`, `AreaTable.dbc`, `ChrClasses.dbc`, `ChrRaces.dbc`, `TalentTab.dbc`) and
//! searches `content/`, the content templates (`docs/game/templates/`) and the game crates'
//! sources, tests and examples (`crates/esurv*`) for any of them, whole words, case and all,
//! in quoted text and comments (code is identifiers, our schema's words). It only reads the client
//! and prints what it finds; it never writes.
//!
//! `cargo run -p esurv-content --features client --example scan_client_names [-- <Data folder>]`
//! (default: `$WOW_DATA`, else the project folder's `WoW/Data`). What it finds is grouped:
//!
//! * **our text by decision**: enemy, boss and pet names (creatures), boss yells, gear, weapon
//!   and jewelry names, by file and line;
//! * **our own names**: a `name:` in content that is no client name but holds one, or a client
//!   name inside one of our own names of two words or more written in a text (`Emerald Slumber`),
//!   our text by the rule;
//! * **comments**: comments in code, tests, content and templates, which document them and are
//!   never shown in the game;
//! * **reviewed**: the hits `scan_client_names.allow` (beside this file) keeps as text, a line
//!   each with its reason (test messages and fixtures, our own words that equal a client name);
//! * **elsewhere**: every other hit, the ones to fix: each line says the pointer to write (the one
//!   the content already uses for that name, else the table's first row of it);
//! * **ordinary words**: one-word names that are also ordinary words or our schema's, counted, not
//!   listed: a name no content pointer shows whose lower-case form the scanned files also use, or
//!   that their code uses as an identifier (a school, a stat, a tag).
//!
//! Allow-list lines that match nothing any more are listed last, to be removed.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use esurv_content::{ClientName, ClientTables, Content, NameRef};

/// The shortest name looked for: shorter ones (`Hi`, `Go`) are noise.
const SHORTEST: usize = 3;

/// The project folder: two levels up from this crate.
fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The client's `Data` folder: the first argument, `$WOW_DATA`, else the project's `WoW/Data`.
fn data_dir() -> PathBuf {
    std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("WOW_DATA").filter(|d| !d.is_empty()).map(PathBuf::from))
        .unwrap_or_else(|| project().join("WoW/Data"))
}

/// Every file under `dir` (recursively) with one of `extensions`, `target` folders left out.
fn files(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "target") {
                files(&path, extensions, out);
            }
        } else if path.extension().and_then(|x| x.to_str()).is_some_and(|x| extensions.contains(&x)) {
            out.push(path);
        }
    }
}

/// Whether `c` is part of a word: a letter, a digit or `_` (so `fire_nova` and `Ability_Parry`
/// are one word each, never a name).
fn word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The words of `text` with their byte offsets; an apostrophe between letters joins (`o'clock`).
fn words(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    let bytes = text.as_bytes();
    for (i, c) in text.char_indices() {
        let joins = c == '\'' && start.is_some() && bytes.get(i + 1).is_some_and(u8::is_ascii_alphabetic);
        match (word_char(c) || joins, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, &text[s..i]));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, &text[s..]));
    }
    out
}

/// The client's names, by first word, for finding them in a line.
struct Matcher {
    by_first: HashMap<String, Vec<String>>,
}

impl Matcher {
    fn new<'a>(names: impl Iterator<Item = &'a String>) -> Self {
        let mut by_first: HashMap<String, Vec<String>> = HashMap::new();
        for name in names {
            if let Some((0, first)) = words(name).first().copied() {
                by_first.entry(first.to_owned()).or_default().push(name.clone());
            }
        }
        Self { by_first }
    }
    /// The names in `line`, whole words (a possessive's too), the longest at each place, none
    /// overlapping.
    fn find<'a>(&'a self, line: &str) -> Vec<(usize, &'a str)> {
        let mut found: Vec<(usize, &str)> = Vec::new();
        let mut covered = 0;
        for (at, first) in words(line) {
            if at < covered {
                continue;
            }
            let tail = &line[at..];
            let longest = self
                .by_first
                .get(first.strip_suffix("'s").unwrap_or(first))
                .into_iter()
                .flatten()
                .filter(|n| tail.starts_with(n.as_str()) && !tail[n.len()..].starts_with(word_char))
                .max_by_key(|n| n.len());
            if let Some(name) = longest {
                found.push((at, name));
                covered = at + name.len();
            }
        }
        found
    }
}

/// The text of `line`, where a name could be written as text: its quoted strings, then its comment
/// (after `//` or `--`, marked true); never its code, whose words are identifiers, our schema's
/// words and not the client's.
fn text_parts(line: &str) -> Vec<(bool, &str)> {
    let (code, comment) = match [line.find("//"), line.find("--")].into_iter().flatten().min() {
        Some(at) => (&line[..at], Some(&line[at..])),
        None => (line, None),
    };
    let strings = code.split('"').skip(1).step_by(2).map(|s| (false, s));
    strings.chain(comment.map(|c| (true, c))).collect()
}

/// A content line's RON field (`name`, `yell` …) and how deep it sits.
fn field_of(line: &str) -> Option<(&str, usize)> {
    let trimmed = line.trim_start();
    let depth = line.len() - trimmed.len();
    let trimmed = trimmed.trim_start_matches('(');
    let key = trimmed.split(':').next()?;
    let is_key = !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    (is_key && trimmed.len() > key.len()).then_some((key, depth))
}

/// Names kept as our text wherever they appear: the talent pets (creatures,
/// like the enemies), our own buff name "Retribution" and the verb "Poisons".
const KEPT_NAMES: [&str; 4] = ["Water Elemental", "Spirit Wolf", "Retribution", "Poisons"];
/// Our own schema's words that a client name happens to equal: an on-hit kind (`Shield`), a spell
/// tag (`Cleave`), a boss ability kind (`Volley`) and a creature type (`Undead`).
const SCHEMA_WORDS: [&str; 4] = ["Shield", "Cleave", "Volley", "Undead"];

/// Whether a hit on `line` of `file` is our text by decision: an enemy's or a boss's
/// name or yell (a zone file's deeper `name:`, any `yell:`), or an item's name (gear, jewelry,
/// amulets, outfit weapons).
fn text_by_decision(file: &str, line: &str) -> bool {
    let code = line.split("//").next().unwrap_or(line);
    if code.trim().is_empty() {
        return false;
    }
    if file.ends_with("content/gear.ron") || file.ends_with("content/outfit_weapons.ron") || file.ends_with("gear_set.ron") {
        return true;
    }
    let Some((key, depth)) = field_of(line) else { return false };
    let zone = file.contains("/zones/") || file.ends_with("templates/zone.ron");
    let items = ["jewelry.ron", "amulets.ron"];
    key == "yell"
        || (zone && key == "name" && depth > 4)
        || (file.ends_with("templates/boss.ron") && key == "name")
        || (items.iter().any(|f| file.ends_with(f)) && key == "name")
}

/// What the content itself tells the scan, read with the client's names.
#[derive(Default)]
struct Known {
    /// The names the content's pointers show (never ordinary words: a talent tree's aside, which
    /// are the schools' and roles' words).
    pointed: HashSet<String>,
    /// Our own names of two words or more that are no client name (`Emerald Slumber`, `Seed of
    /// Corruption`): a client name inside one is part of our name.
    ours: Vec<String>,
    /// The pets' names: creatures, our text like the enemies.
    pets: HashSet<String>,
}

fn known(names: &esurv_content::ClientNames, client: &HashMap<String, &str>) -> Known {
    let dir = project().join("content");
    let Ok(c) = Content::load_with_names(&dir, names) else { return Known::default() };
    let mut refs: Vec<(&NameRef, &String)> = Vec::new();
    refs.extend(c.classes.iter().map(|x| (&x.name_ref, &x.name)));
    refs.extend(c.classes.iter().flat_map(|x| &x.starting_options).map(|o| (&o.name_ref, &o.name)));
    refs.extend(c.classes.iter().flat_map(|x| &x.starting_options).map(|o| (&o.passive.name_ref, &o.passive.name)));
    refs.extend(c.abilities.iter().map(|x| (&x.name_ref, &x.name)));
    refs.extend(c.races.iter().map(|x| (&x.name_ref, &x.name)));
    refs.extend(c.races.iter().flat_map(|r| &r.passives).map(|x| (&x.name_ref, &x.name)));
    refs.extend(c.talent_trees.iter().flat_map(|t| &t.talents).map(|x| (&x.name_ref, &x.name)));
    refs.extend(c.words.buffs.iter().map(|x| (&x.name_ref, &x.name)));
    let mut pointed: HashSet<String> = c.zones.iter().map(|z| z.name.clone()).collect();
    pointed.extend(refs.iter().filter(|(r, _)| matches!(r, NameRef::Client(_))).map(|(_, n)| (*n).clone()));
    let mut ours: Vec<String> = refs.iter().filter(|(r, _)| matches!(r, NameRef::Text(_))).map(|(_, n)| (*n).clone()).collect();
    ours.extend(c.legendary_amulets.iter().map(|l| l.name.clone()));
    ours.retain(|n| words(n).len() > 1 && !client.contains_key(n.as_str()));
    ours.sort();
    ours.dedup();
    Known { pointed, ours, pets: c.pets.iter().map(|p| p.name.clone()).collect() }
}

/// The pointer to write for each client name: the one the content already uses most for it
/// (`name: Spell(12294)`, `{Spell(772)}`), else the table's first row of that name (a spell's
/// first ranked row, as player spells have ranks).
fn pointers(tables: &ClientTables, content_texts: &[&str]) -> HashMap<String, String> {
    let names = tables.names();
    let mut used: HashMap<(String, String), usize> = HashMap::new();
    for text in content_texts {
        for kind in ["Spell", "Area", "Class", "Race", "TalentTab"] {
            let opening = format!("{kind}(");
            for (at, _) in text.match_indices(opening.as_str()) {
                let rest = &text[at + opening.len()..];
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                let Ok(id) = digits.parse::<u32>() else { continue };
                let pointer = match kind {
                    "Spell" => ClientName::Spell(id),
                    "Area" => ClientName::Area(id),
                    "Class" => ClientName::Class(id),
                    "Race" => ClientName::Race(id),
                    _ => ClientName::TalentTab(id),
                };
                if let Some(name) = names.get(pointer) {
                    *used.entry((name.to_owned(), format!("{kind}({id})"))).or_default() += 1;
                }
            }
        }
    }
    let mut out: HashMap<String, (usize, String)> = HashMap::new();
    for ((name, pointer), n) in used {
        let e = out.entry(name).or_insert((0, String::new()));
        if n > e.0 || (n == e.0 && pointer < e.1) {
            *e = (n, pointer);
        }
    }
    let mut out: HashMap<String, String> = out.into_iter().map(|(k, (_, p))| (k, p)).collect();
    let mut spells: Vec<_> = tables.spells.iter().collect();
    spells.sort_by_key(|s| (s.rank_number().is_none(), s.id));
    let rows = spells.iter().map(|s| (s.name.clone(), format!("Spell({})", s.id)));
    let rows = rows.chain(tables.areas.iter().map(|(id, n, _)| (n.clone(), format!("Area({id})"))));
    let rows = rows.chain(tables.classes.iter().map(|(id, n)| (n.clone(), format!("Class({id})"))));
    let rows = rows.chain(tables.races.iter().map(|(id, n)| (n.clone(), format!("Race({id})"))));
    let rows = rows.chain(tables.talent_tabs.iter().map(|(id, n, _)| (n.clone(), format!("TalentTab({id})"))));
    for (name, pointer) in rows {
        out.entry(name).or_insert(pointer);
    }
    out
}

/// One reviewed line of `scan_client_names.allow`: a client name in a file that stays text, and
/// why.
struct Allowed {
    file: String,
    name: String,
    reason: String,
    used: std::cell::Cell<usize>,
}

/// The reviewed allow-list beside this example: `file | name | reason` a line, `#` comments.
fn allow_list() -> Vec<Allowed> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/scan_client_names.allow");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut parts = l.splitn(3, '|').map(str::trim);
            let (file, name, reason) = (parts.next()?, parts.next()?, parts.next()?);
            (!reason.is_empty()).then(|| Allowed { file: file.into(), name: name.into(), reason: reason.into(), used: 0.into() })
        })
        .collect()
}

fn main() {
    let data = data_dir();
    let tables = match ClientTables::read(&data) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("scan_client_names: the client's tables did not read from {}: {e:#}", data.display());
            std::process::exit(2);
        }
    };
    // every name, with the table that holds it
    let mut names: HashMap<String, &'static str> = HashMap::new();
    let rows = tables.spells.iter().map(|s| (s.name.as_str(), "Spell"));
    let rows = rows.chain(tables.areas.iter().map(|(_, n, _)| (n.as_str(), "AreaTable")));
    let rows = rows.chain(tables.classes.iter().map(|(_, n)| (n.as_str(), "ChrClasses")));
    let rows = rows.chain(tables.races.iter().map(|(_, n)| (n.as_str(), "ChrRaces")));
    let rows = rows.chain(tables.talent_tabs.iter().map(|(_, n, _)| (n.as_str(), "TalentTab")));
    for (name, table) in rows {
        // a name must start and end on a word, and be long enough
        let starts = name.starts_with(word_char) && name.ends_with(word_char);
        if starts && name.chars().count() >= SHORTEST && name.chars().any(char::is_alphabetic) {
            names.entry(name.to_owned()).or_insert(table);
        }
    }
    let matcher = Matcher::new(names.keys());
    let known = known(&tables.names(), &names);
    let pointed = &known.pointed;
    let root = project();
    let mut scanned = Vec::new();
    files(&root.join("content"), &["ron"], &mut scanned);
    files(&root.join("docs/game/templates"), &["ron"], &mut scanned);
    if let Ok(crates) = std::fs::read_dir(root.join("crates")) {
        for c in crates.flatten().filter(|c| c.file_name().to_string_lossy().starts_with("esurv")) {
            files(&c.path(), &["rs", "ron", "toml", "lua", "xml", "toc"], &mut scanned);
        }
    }
    scanned.sort();
    let texts: Vec<String> = scanned.iter().map(|p| std::fs::read_to_string(p).unwrap_or_default()).collect();
    let content_texts: Vec<&str> =
        scanned.iter().zip(&texts).filter(|(p, _)| p.starts_with(root.join("content"))).map(|(_, t)| t.as_str()).collect();
    let pointer_of = pointers(&tables, &content_texts);
    let allowed = allow_list();
    // the words the scanned files use in lower case, and as identifiers in code (our schema's
    // kinds: a school, a stat, a tag), to tell ordinary words apart
    let lower: HashSet<&str> = texts
        .iter()
        .flat_map(|t| words(t))
        .map(|(_, w)| w)
        .filter(|w| w.chars().next().is_some_and(char::is_lowercase))
        .collect();
    let schema: HashSet<&str> = texts.iter().flat_map(|t| t.lines().flat_map(code_words)).collect();
    let ordinary = |name: &str| {
        let one_word = words(name).len() == 1 && !name.contains('-') && !pointed.contains(name);
        SCHEMA_WORDS.contains(&name) || one_word && (lower.contains(name.to_lowercase().as_str()) || schema.contains(name))
    };
    // the groups, in the order printed
    let mut groups: [(&str, Vec<String>); 5] = [
        ("our text by decision (enemies, bosses, pets, yells, items)", Vec::new()),
        ("our own names (no exact client name) that contain one", Vec::new()),
        ("comments (code, tests, content, templates): documentation, never shown in the game", Vec::new()),
        ("reviewed in scan_client_names.allow", Vec::new()),
        ("elsewhere", Vec::new()),
    ];
    let mut ordinary_hits: BTreeMap<&str, usize> = BTreeMap::new();
    for (path, text) in scanned.iter().zip(&texts) {
        let file = path.strip_prefix(&root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        for (n, line) in text.lines().enumerate() {
            for (comment, part) in text_parts(line) {
                // where our own names lie in this text: a client name inside one is part of it
                let ours: Vec<(usize, usize)> = known
                    .ours
                    .iter()
                    .flat_map(|o| part.match_indices(o.as_str()).map(|(at, o)| (at, at + o.len())))
                    .collect();
                for (at, name) in matcher.find(part) {
                    if ordinary(name) {
                        *ordinary_hits.entry(name).or_default() += 1;
                        continue;
                    }
                    let pointer = pointer_of.get(name).map_or("?", String::as_str);
                    let hit = format!("{file}:{}: {name} ({}, write {{{pointer}}}): {}", n + 1, names[name], line.trim());
                    let kept = KEPT_NAMES.iter().any(|k| k.contains(name) && line.contains(k));
                    let in_ours = ours.iter().any(|&(from, to)| from <= at && at + name.len() <= to);
                    let allow = allowed.iter().find(|a| a.file == file && a.name == name);
                    // a loadout weapon's `name:` is a gear item's, our text by decision
                    let weapon = file.ends_with("content/loadouts.ron") && line.contains(&format!("name: \"{part}\""));
                    let group = if text_by_decision(&file, line) || kept || weapon || known.pets.contains(part) {
                        0
                    } else if in_ours || our_own_name(&file, line, &names) {
                        1
                    } else if comment {
                        2
                    } else if let Some(a) = allow {
                        a.used.set(a.used.get() + 1);
                        3
                    } else {
                        4
                    };
                    groups[group].1.push(hit);
                }
            }
        }
    }
    println!("scanned {} files for {} client names", scanned.len(), names.len());
    for (title, hits) in &groups {
        println!("\n== {title}: {}", hits.len());
        for h in hits {
            println!("  {h}");
        }
    }
    let total: usize = ordinary_hits.values().sum();
    println!("\n== ordinary words: {} names, {total} hits", ordinary_hits.len());
    let list: Vec<String> = ordinary_hits.iter().map(|(n, c)| format!("{n} ({c})")).collect();
    println!("  {}", list.join(", "));
    let stale: Vec<&Allowed> = allowed.iter().filter(|a| a.used.get() == 0).collect();
    println!("\n== allow-list lines that matched nothing (stale, remove them): {}", stale.len());
    for a in stale {
        println!("  {} | {} | {}", a.file, a.name, a.reason);
    }
}

/// Whether `line` of a content file or template is a `name:` that is ours: its text is no client
/// name, though a client name is part of it (a talent that improves a spell, named after it).
fn our_own_name(file: &str, line: &str, names: &HashMap<String, &str>) -> bool {
    let content = file.starts_with("content/") || file.starts_with("docs/");
    let value = line.split('"').nth(1);
    content && field_of(line).is_some_and(|(key, _)| key == "name") && value.is_some_and(|v| !names.contains_key(v))
}

/// The words of `line`'s code: what is left without its quoted strings and its comment.
fn code_words(line: &str) -> Vec<&str> {
    let code = [line.find("//"), line.find("--")].into_iter().flatten().min().map_or(line, |at| &line[..at]);
    code.split('"').step_by(2).flat_map(words).map(|(_, w)| w).collect()
}
