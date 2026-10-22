//! Names as content writes them: our own text, or a pointer to a name the player's 1.12 client
//! holds (a spell's, an area's, a class's or a race's), read from the client when the game loads
//! (the repository holds no client text).
//!
//! In RON a name is either a string, `name: "Our Own Name"`, or a pointer, `name: Spell(12294)`,
//! `Area(12)`, `Class(1)`, `Race(1)`. [`crate::Content::load_with_names`] turns every pointer into
//! the client's name; with no client tables ([`ClientNames::none`]) a pointer reads as a neutral
//! placeholder such as `Spell 12294`.
//!
//! Our prose can name the client's names the same way, a pointer in braces in the text:
//! `"{Spell(772)}: +{v}% damage or healing."` ([`ClientNames::show_text`]).

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// A name as content writes it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NameRef {
    /// A name the client holds, by its row id.
    Client(ClientName),
    /// Our own name.
    Text(String),
}

impl Default for NameRef {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

impl From<&str> for NameRef {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl From<ClientName> for NameRef {
    fn from(pointer: ClientName) -> Self {
        Self::Client(pointer)
    }
}

/// A pointer to a name in one of the client's tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClientName {
    /// A `Spell.dbc` row: a spell's, a talent's or a racial trait's name.
    Spell(u32),
    /// An `AreaTable.dbc` row: a zone's name.
    Area(u32),
    /// A `ChrClasses.dbc` row: a class's name.
    Class(u32),
    /// A `ChrRaces.dbc` row: a race's name.
    Race(u32),
    /// A `TalentTab.dbc` row: a talent tree's name.
    TalentTab(u32),
    /// An Alliance PvP rank's title, rank 1 (Private) to 14 (Grand Marshal): the client's
    /// `GlobalStrings.lua` `PVP_RANK_<rank + 4>_1`.
    AllianceRank(u32),
    /// A Horde PvP rank's title, rank 1 (Scout) to 14 (High Warlord): `PVP_RANK_<rank + 4>_0`.
    HordeRank(u32),
}

impl ClientName {
    /// The pointer's kind as written (`Spell`, `Area` …), with its table.
    const KINDS: [(&'static str, &'static str); 7] = [
        ("Spell", "Spell"),
        ("Area", "AreaTable"),
        ("Class", "ChrClasses"),
        ("Race", "ChrRaces"),
        ("TalentTab", "TalentTab"),
        ("AllianceRank", "GlobalStrings"),
        ("HordeRank", "GlobalStrings"),
    ];

    fn kind_index(self) -> usize {
        match self {
            Self::Spell(_) => 0,
            Self::Area(_) => 1,
            Self::Class(_) => 2,
            Self::Race(_) => 3,
            Self::TalentTab(_) => 4,
            Self::AllianceRank(_) => 5,
            Self::HordeRank(_) => 6,
        }
    }

    /// The pointer of kind `kind` (`Spell`, `Area` …) to row `id`.
    fn of_kind(kind: &str, id: u32) -> Option<Self> {
        Some(match kind {
            "Spell" => Self::Spell(id),
            "Area" => Self::Area(id),
            "Class" => Self::Class(id),
            "Race" => Self::Race(id),
            "TalentTab" => Self::TalentTab(id),
            "AllianceRank" => Self::AllianceRank(id),
            "HordeRank" => Self::HordeRank(id),
            _ => return None,
        })
    }

    /// The table the pointer reads.
    pub fn table(self) -> &'static str {
        Self::KINDS[self.kind_index()].1
    }

    /// The row id.
    pub fn id(self) -> u32 {
        match self {
            Self::Spell(id)
            | Self::Area(id)
            | Self::Class(id)
            | Self::Race(id)
            | Self::TalentTab(id)
            | Self::AllianceRank(id)
            | Self::HordeRank(id) => id,
        }
    }
}

/// How a pointer reads with no client tables: `Spell 12294`, `Area 12`, `Class 1`, `Race 1`,
/// `TalentTab 161`.
impl fmt::Display for ClientName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", Self::KINDS[self.kind_index()].0, self.id())
    }
}

/// The names of the client's tables a pointer can read, by row id; empty when the game runs with
/// no client data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientNames {
    /// By pointer kind, in [`ClientName::KINDS`] order.
    tables: [HashMap<u32, String>; 7],
}

impl ClientNames {
    /// No client tables: every pointer reads as its placeholder.
    pub fn none() -> Self {
        Self::default()
    }

    /// The tables' names by row id.
    pub fn from_tables(
        spells: HashMap<u32, String>,
        areas: HashMap<u32, String>,
        classes: HashMap<u32, String>,
        races: HashMap<u32, String>,
    ) -> Self {
        Self { tables: [spells, areas, classes, races, HashMap::new(), HashMap::new(), HashMap::new()] }
    }

    /// These names with the PvP rank titles by rank (1 to 14), the Alliance's and the Horde's.
    pub fn with_pvp_ranks(mut self, alliance: HashMap<u32, String>, horde: HashMap<u32, String>) -> Self {
        self.tables[5] = alliance;
        self.tables[6] = horde;
        self
    }

    /// These names with the talent trees' (`TalentTab.dbc`) by row id.
    pub fn with_talent_tabs(mut self, talent_tabs: HashMap<u32, String>) -> Self {
        self.tables[4] = talent_tabs;
        self
    }

    /// Whether no table was read.
    pub fn is_empty(&self) -> bool {
        self.tables.iter().all(HashMap::is_empty)
    }

    /// The client's name for `pointer`, if its table has the row.
    pub fn get(&self, pointer: ClientName) -> Option<&str> {
        self.tables[pointer.kind_index()].get(&pointer.id()).map(String::as_str)
    }

    /// `text` with every pointer written in it (`{Spell(772)}`, `{Area(12)}`, `{Class(1)}`,
    /// `{Race(1)}`) shown as [`ClientNames::show`] shows a name; the rest as written.
    pub fn show_text(&self, text: &str, missing: &mut Vec<ClientName>) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(at) = rest.find('{') {
            out.push_str(&rest[..at]);
            rest = &rest[at..];
            match pointer_at(rest) {
                Some((pointer, len)) => {
                    out.push_str(&self.show(&NameRef::Client(pointer), missing));
                    rest = &rest[len..];
                }
                None => {
                    out.push('{');
                    rest = &rest[1..];
                }
            }
        }
        out.push_str(rest);
        out
    }

    /// `name` as the game shows it: our text as written, a pointer as the client's name, or its
    /// placeholder when no table was read. A pointer the read tables lack also shows its
    /// placeholder, and is added to `missing`.
    pub fn show(&self, name: &NameRef, missing: &mut Vec<ClientName>) -> String {
        match name {
            NameRef::Text(text) => text.clone(),
            NameRef::Client(pointer) => match self.get(*pointer) {
                Some(shown) => shown.to_owned(),
                None => {
                    if !self.is_empty() {
                        missing.push(*pointer);
                    }
                    pointer.to_string()
                }
            },
        }
    }
}

/// The pointer `{Kind(id)}` that `text` starts with, and its length in bytes.
fn pointer_at(text: &str) -> Option<(ClientName, usize)> {
    let inner = text.strip_prefix('{')?;
    let open = inner.find('(')?;
    let close = inner.find(")}")?;
    let id: u32 = inner.get(open + 1..close)?.parse().ok()?;
    let pointer = ClientName::of_kind(&inner[..open], id)?;
    Some((pointer, close + 3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Record {
        name: NameRef,
        other: Option<NameRef>,
    }

    #[test]
    fn a_name_reads_as_text_or_as_a_pointer() {
        let r: Record = ron::from_str("(name: Spell(12294), other: Some(\"Our Own Name\"))").unwrap();
        assert_eq!(r.name, NameRef::Client(ClientName::Spell(12294)));
        assert_eq!(r.other, Some(NameRef::from("Our Own Name")));
        for (text, pointer) in [("Area(12)", ClientName::Area(12)), ("Class(1)", ClientName::Class(1)), ("Race(8)", ClientName::Race(8))] {
            let r: Record = ron::from_str(&format!("(name: {text}, other: None)")).unwrap();
            assert_eq!(r.name, NameRef::Client(pointer));
        }
        assert_eq!(ron::to_string(&r).unwrap(), "(name:Spell(12294),other:Some(\"Our Own Name\"))");
    }

    #[test]
    fn a_pointer_shows_the_clients_name_or_its_placeholder() {
        let mut missing = Vec::new();
        let none = ClientNames::none();
        assert_eq!(none.show(&ClientName::Spell(12294).into(), &mut missing), "Spell 12294");
        assert_eq!(none.show(&"Ours".into(), &mut missing), "Ours");
        assert!(missing.is_empty(), "no tables: placeholders are expected, not missing");
        let spells = HashMap::from([(1, "Placeholder Spell".to_owned())]);
        let some = ClientNames::from_tables(spells, HashMap::new(), HashMap::new(), HashMap::new());
        assert_eq!(some.show(&ClientName::Spell(1).into(), &mut missing), "Placeholder Spell");
        assert_eq!(some.show(&ClientName::Area(7).into(), &mut missing), "Area 7");
        assert_eq!(missing, vec![ClientName::Area(7)], "a read table that lacks the row is reported");
    }

    #[test]
    fn a_text_shows_the_pointers_written_in_it_and_keeps_the_rest() {
        let mut missing = Vec::new();
        let spells = HashMap::from([(1, "Placeholder Spell".to_owned())]);
        let some = ClientNames::from_tables(spells, HashMap::new(), HashMap::new(), HashMap::new());
        let text = "{Spell(1)}: +{v}% damage. {Class(3)} only. {Spell(x)} {targets:a|# b}";
        assert_eq!(some.show_text(text, &mut missing), "Placeholder Spell: +{v}% damage. Class 3 only. {Spell(x)} {targets:a|# b}");
        assert_eq!(missing, vec![ClientName::Class(3)]);
        assert_eq!(ClientNames::none().show_text("{Area(12)} and {Race(2)}", &mut missing), "Area 12 and Race 2");
    }
}
