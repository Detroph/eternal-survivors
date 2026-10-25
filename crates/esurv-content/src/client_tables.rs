//! The client's name tables, read from the player's own 1.12.1 data (feature `client`): every
//! name a content pointer can read (`Spell.dbc`, `AreaTable.dbc`, `ChrClasses.dbc`,
//! `ChrRaces.dbc`, `TalentTab.dbc`). Each name is a block of eight locale columns; the first one
//! filled is the client's own language.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use benilla_formats::Chain;

use crate::names::ClientNames;

/// One `Spell.dbc` row's name and rank text ("Rank 2", "Racial", or none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellNameRow {
    pub id: u32,
    pub name: String,
    pub rank: Option<String>,
}

impl SpellNameRow {
    /// The rank's number ("Rank 3" is 3); `None` for a rank that is no number, or no rank.
    pub fn rank_number(&self) -> Option<u32> {
        self.rank.as_deref()?.strip_prefix("Rank ")?.trim().parse().ok()
    }
}

/// The tables' names as the client holds them, every row.
#[derive(Debug, Clone, Default)]
pub struct ClientTables {
    pub spells: Vec<SpellNameRow>,
    /// `AreaTable.dbc`: (id, name, parent zone id; 0 for a zone of its own).
    pub areas: Vec<(u32, String, u32)>,
    pub classes: Vec<(u32, String)>,
    pub races: Vec<(u32, String)>,
    /// `TalentTab.dbc`: (id, name, the classes it belongs to as a mask, bit `id - 1` per class).
    pub talent_tabs: Vec<(u32, String, u32)>,
    /// The PvP rank titles from `GlobalStrings.lua`: (faction side, 1 Alliance or 0 Horde, rank
    /// 1 to 14, title).
    pub pvp_ranks: Vec<(u32, u32, String)>,
}

/// The client's interface strings, where the PvP rank titles are.
const GLOBAL_STRINGS: &str = r"Interface\FrameXML\GlobalStrings.lua";

/// The PvP rank titles in `GlobalStrings.lua`'s text: each `PVP_RANK_<n>_<side> = "<title>";`
/// line (not the `_FEMALE` ones) as (side, rank `n - 4`, title), ranks 1 to 14.
fn pvp_ranks(text: &str) -> Vec<(u32, u32, String)> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            let mut parts = key.trim().strip_prefix("PVP_RANK_")?.split('_');
            let (n, side) = (parts.next()?.parse::<u32>().ok()?, parts.next()?.parse::<u32>().ok()?);
            if parts.next().is_some() || !(5..=18).contains(&n) {
                return None;
            }
            let title = value.trim().strip_prefix('"')?.split('"').next()?;
            Some((side, n - 4, title.to_owned()))
        })
        .collect()
}

/// A table's layout: its file, its column count, and where its name's locale block starts.
struct Layout {
    file: &'static str,
    columns: usize,
    name: usize,
}

const SPELL: Layout = Layout { file: "DBFilesClient\\Spell.dbc", columns: 173, name: 120 };
/// `Spell.dbc`'s rank text block.
const SPELL_RANK: usize = 129;
const AREA_TABLE: Layout = Layout { file: "DBFilesClient\\AreaTable.dbc", columns: 25, name: 11 };
/// `AreaTable.dbc`'s parent zone column.
const AREA_PARENT: usize = 2;
const CHR_CLASSES: Layout = Layout { file: "DBFilesClient\\ChrClasses.dbc", columns: 17, name: 5 };
const CHR_RACES: Layout = Layout { file: "DBFilesClient\\ChrRaces.dbc", columns: 29, name: 17 };
const TALENT_TAB: Layout = Layout { file: "DBFilesClient\\TalentTab.dbc", columns: 15, name: 1 };
/// `TalentTab.dbc`'s class mask column.
const TALENT_TAB_CLASSES: usize = 12;

/// A read `WDBC` file: its records as rows of 32-bit columns, and its string block.
struct Dbc {
    columns: usize,
    records: Vec<u8>,
    strings: Vec<u8>,
}

impl Dbc {
    fn read(chain: &mut Chain, layout: &Layout) -> Result<Self> {
        let bytes = chain.read_file(layout.file).with_context(|| format!("reading {}", layout.file))?;
        let word = |i: usize| bytes.get(i * 4..i * 4 + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
        let (Some(rows), Some(columns), Some(size), Some(strings)) = (word(1), word(2), word(3), word(4)) else {
            bail!("{} is too short for a header", layout.file);
        };
        if !bytes.starts_with(b"WDBC") || columns != layout.columns || size != columns * 4 {
            bail!("{} is not the 1.12.1 table ({columns} columns of {size} bytes)", layout.file);
        }
        let start = 20 + rows * size;
        if bytes.len() < start + strings {
            bail!("{} is cut short", layout.file);
        }
        Ok(Self { columns, records: bytes[20..start].to_vec(), strings: bytes[start..start + strings].to_vec() })
    }

    fn rows(&self) -> impl Iterator<Item = &[u8]> {
        self.records.chunks_exact(self.columns * 4)
    }

    fn number(row: &[u8], column: usize) -> u32 {
        u32::from_le_bytes([row[column * 4], row[column * 4 + 1], row[column * 4 + 2], row[column * 4 + 3]])
    }

    /// The first filled text of the eight locale columns from `column`.
    fn text(&self, row: &[u8], column: usize) -> Option<String> {
        (column..column + 8).find_map(|c| {
            let start = Self::number(row, c) as usize;
            let tail = self.strings.get(start..)?;
            let text = &tail[..tail.iter().position(|&b| b == 0).unwrap_or(tail.len())];
            (!text.is_empty()).then(|| String::from_utf8_lossy(text).into_owned())
        })
    }

    /// Every row's id and name.
    fn names(&self, layout: &Layout) -> Vec<(u32, String)> {
        self.rows().filter_map(|r| Some((Self::number(r, 0), self.text(r, layout.name)?))).collect()
    }
}

/// `Spell.dbc`'s casting time index (a row of `SpellCastTimes.dbc`), its own cooldown and its
/// category's cooldown (milliseconds).
const SPELL_CASTING_TIME: usize = 18;
const SPELL_RECOVERY: usize = 19;
const SPELL_CATEGORY_RECOVERY: usize = 20;
const SPELL_CAST_TIMES: Layout = Layout { file: "DBFilesClient\\SpellCastTimes.dbc", columns: 4, name: 0 };
/// `Spell.dbc`'s `AttributesEx`: a channelled spell has either channel bit (`0x4`, `0x40`).
const SPELL_ATTRIBUTES_EX: usize = 7;
const ATTRIBUTES_EX_CHANNELLED: u32 = 0x44;
/// `Spell.dbc`'s duration index (a row of `SpellDuration.dbc`), its three effects' radius indexes
/// (rows of `SpellRadius.dbc`) and their periodic amplitudes (milliseconds).
const SPELL_DURATION_INDEX: usize = 30;
const SPELL_EFFECT_RADIUS_INDEX: usize = 88;
const SPELL_EFFECT_AMPLITUDE: usize = 94;
const SPELL_DURATION: Layout = Layout { file: "DBFilesClient\\SpellDuration.dbc", columns: 4, name: 0 };
const SPELL_RADIUS: Layout = Layout { file: "DBFilesClient\\SpellRadius.dbc", columns: 4, name: 0 };

/// One spell's times as `Spell.dbc` holds them, seconds: the base cast time of its
/// `SpellCastTimes.dbc` row (0 for an instant or a channel), the longer of its own and its
/// category's cooldown, and for a channelled spell its channel (its `SpellDuration.dbc` base
/// time), its ticks' interval (its first periodic effect's amplitude) and its radius (yards, its
/// first effect's `SpellRadius.dbc` row with one); all three 0 for a spell not channelled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpellTimes {
    pub id: u32,
    pub cast: f32,
    pub cooldown: f32,
    pub channel: f32,
    pub tick_every: f32,
    pub radius: f32,
}

/// Every spell's times from the 1.12.1 `Data` folder `data_dir`.
pub fn read_spell_times(data_dir: &Path) -> Result<Vec<SpellTimes>> {
    let mut chain = Chain::open(data_dir).with_context(|| format!("opening the game data in {}", data_dir.display()))?;
    let spell = Dbc::read(&mut chain, &SPELL)?;
    let casts = Dbc::read(&mut chain, &SPELL_CAST_TIMES)?;
    let cast_ms: HashMap<u32, i32> = casts.rows().map(|r| (Dbc::number(r, 0), Dbc::number(r, 1) as i32)).collect();
    let durations = Dbc::read(&mut chain, &SPELL_DURATION)?;
    let duration_ms: HashMap<u32, i32> = durations.rows().map(|r| (Dbc::number(r, 0), Dbc::number(r, 1) as i32)).collect();
    let radii = Dbc::read(&mut chain, &SPELL_RADIUS)?;
    let radius_yd: HashMap<u32, f32> = radii.rows().map(|r| (Dbc::number(r, 0), f32::from_bits(Dbc::number(r, 1)))).collect();
    Ok(spell
        .rows()
        .map(|r| {
            let cast = cast_ms.get(&Dbc::number(r, SPELL_CASTING_TIME)).copied().unwrap_or(0).max(0);
            let cooldown = Dbc::number(r, SPELL_RECOVERY).max(Dbc::number(r, SPELL_CATEGORY_RECOVERY));
            let (mut channel, mut tick_every, mut radius) = (0.0, 0.0, 0.0);
            if Dbc::number(r, SPELL_ATTRIBUTES_EX) & ATTRIBUTES_EX_CHANNELLED != 0 {
                channel = duration_ms.get(&Dbc::number(r, SPELL_DURATION_INDEX)).copied().unwrap_or(0).max(0) as f32 / 1000.0;
                tick_every = (0..3).map(|i| Dbc::number(r, SPELL_EFFECT_AMPLITUDE + i)).find(|&a| a > 0).unwrap_or(0) as f32 / 1000.0;
                radius = (0..3).filter_map(|i| radius_yd.get(&Dbc::number(r, SPELL_EFFECT_RADIUS_INDEX + i)).copied()).find(|&y| y > 0.0).unwrap_or(0.0);
            }
            SpellTimes { id: Dbc::number(r, 0), cast: cast as f32 / 1000.0, cooldown: cooldown as f32 / 1000.0, channel, tick_every, radius }
        })
        .collect())
}

impl ClientTables {
    /// Reads the tables from the 1.12.1 `Data` folder `data_dir`.
    pub fn read(data_dir: &Path) -> Result<Self> {
        let mut chain = Chain::open(data_dir).with_context(|| format!("opening the game data in {}", data_dir.display()))?;
        let spell = Dbc::read(&mut chain, &SPELL)?;
        let spells = spell
            .rows()
            .filter_map(|r| {
                let name = spell.text(r, SPELL.name)?;
                Some(SpellNameRow { id: Dbc::number(r, 0), name, rank: spell.text(r, SPELL_RANK) })
            })
            .collect();
        let area = Dbc::read(&mut chain, &AREA_TABLE)?;
        let areas = area
            .rows()
            .filter_map(|r| Some((Dbc::number(r, 0), area.text(r, AREA_TABLE.name)?, Dbc::number(r, AREA_PARENT))))
            .collect();
        let classes = Dbc::read(&mut chain, &CHR_CLASSES)?.names(&CHR_CLASSES);
        let races = Dbc::read(&mut chain, &CHR_RACES)?.names(&CHR_RACES);
        let tab = Dbc::read(&mut chain, &TALENT_TAB)?;
        let talent_tabs = tab
            .rows()
            .filter_map(|r| Some((Dbc::number(r, 0), tab.text(r, TALENT_TAB.name)?, Dbc::number(r, TALENT_TAB_CLASSES))))
            .collect();
        let strings = chain.read_file(GLOBAL_STRINGS).with_context(|| format!("reading {GLOBAL_STRINGS}"))?;
        let pvp_ranks = pvp_ranks(&String::from_utf8_lossy(&strings));
        Ok(Self { spells, areas, classes, races, talent_tabs, pvp_ranks })
    }

    /// The names by row id, for content's pointers.
    pub fn names(&self) -> ClientNames {
        let by_id = |rows: &[(u32, String)]| rows.iter().cloned().collect::<HashMap<_, _>>();
        ClientNames::from_tables(
            self.spells.iter().map(|s| (s.id, s.name.clone())).collect(),
            self.areas.iter().map(|(id, name, _)| (*id, name.clone())).collect(),
            by_id(&self.classes),
            by_id(&self.races),
        )
        .with_talent_tabs(self.talent_tabs.iter().map(|(id, name, _)| (*id, name.clone())).collect())
        .with_pvp_ranks(self.ranks_of(1), self.ranks_of(0))
    }

    /// The rank titles of faction side `side` (1 Alliance, 0 Horde) by rank.
    fn ranks_of(&self, side: u32) -> HashMap<u32, String> {
        self.pvp_ranks.iter().filter(|(s, _, _)| *s == side).map(|(_, rank, title)| (*rank, title.clone())).collect()
    }
}

impl ClientNames {
    /// The names of the client in the 1.12.1 `Data` folder `data_dir`.
    pub fn read(data_dir: &Path) -> Result<Self> {
        Ok(ClientTables::read(data_dir)?.names())
    }

    /// The names of the player's client where the game finds one (`benilla_formats::wow_data`):
    /// `Ok(None)` with no client data, an error when it is there but does not read.
    pub fn from_install() -> Result<Option<Self>> {
        benilla_formats::wow_data().map(|dir| Self::read(&dir)).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rank titles read from GlobalStrings.lua's lines, the female ones and others left out.
    #[test]
    fn the_pvp_rank_titles_read_from_the_interface_strings() {
        let text = "PVP_RANK_5_1 = \"Private\";\nPVP_RANK_5_1_FEMALE = \"Private\";\nPVP_RANK_18_0 = \"High Warlord\";\nPVP_RANK_19_0 = \"Leader\"; -- PvP Leader NPC\nPVP_RANK_12_1 = \"Knight-Captain\";";
        assert_eq!(
            pvp_ranks(text),
            [(1, 1, "Private".to_owned()), (0, 14, "High Warlord".to_owned()), (1, 8, "Knight-Captain".to_owned())]
        );
    }
}
