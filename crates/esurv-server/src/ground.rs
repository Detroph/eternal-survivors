//! The ground under the arenas, how fast spells fly and how far their looks reach, read from the
//! player's own 1.12.1 data (`Data\*.MPQ`): terrain heights from the map's ADT tiles, missile
//! speeds and radii from `Spell.dbc` (and `SpellRadius.dbc`), the race and class pairs the
//! stock create screen offers (`CharBaseInfo.dbc`), the outfit it dresses each in
//! (`CharStartOutfit.dbc`) and each race's player models (`ChrRaces.dbc`).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use benilla_formats::{
    load_item_display_catalog, load_map_catalog, load_spell_catalog, load_spell_radii,
    load_tile_mesh, terrain_height_at, AddedPair, Chain, CharCreateCatalog, ChunkMesh, MapCatalog,
    MapTiles, StartOutfitItem,
};
use esurv_content::Content;
use esurv_sim::game::run::{Walk, World};

use crate::walk_graph::Walking;
use crate::wmo_floor::{Tri, WmoFloors, FLOOR_STEP};

/// The game data the server reads, opened once.
pub struct GameData {
    chain: Mutex<Chain>,
    maps: MapCatalog,
    /// Spell id to missile speed (yards per second), for the spells that fly.
    missile_speeds: HashMap<u32, f32>,
    /// Spell id to its own radius (yards, its first effect's that has one), for the spells with
    /// an area.
    radii: HashMap<u32, f32>,
    /// Spell id to its icon's file name (`Spell_Nature_Thorns`), for the boss warnings' icons.
    spell_icons: HashMap<u32, String>,
    /// Spell id to its name, for a boss's cast with no label of its own.
    spell_names: HashMap<u32, String>,
    /// The (race, class) pairs, 1.12 ids, the stock create screen offers; empty if unread.
    stock_pairs: HashSet<(u8, u8)>,
    /// The races' own spells by name (the stock racials: `Spell.dbc` rows whose rank says
    /// "Racial"), for the spellbook: a race's passives show under their stock spell.
    racial_spells: HashMap<String, u32>,
    /// The create screen's catalog, the added pairs dressed in their borrowed outfits; `None` if
    /// unread.
    outfits: Option<CharCreateCatalog>,
    /// Every starting-outfit item, by item entry, for the item queries.
    outfit_items: HashMap<u32, StartOutfitItem>,
    /// An item look by its icon (the icon's file name, lower case), for the run's jewelry, whose
    /// content names an icon: the lowest look of that icon with no model, else the lowest.
    icon_displays: HashMap<String, u32>,
    /// Each race's player models, (male, female) display ids, by 1.12 race id; empty if unread.
    race_displays: HashMap<u8, (u32, u32)>,
    /// The ranged-slot spells (`Spell.dbc` `Attributes & 0x2`): the client fills their look's
    /// empty stages from the held ranged weapon's own shooting look.
    ranged_slot_spells: HashSet<u32>,
    /// An item look's own shooting look (`ItemDisplayInfo` column 10, a `SpellVisual` id), for
    /// the looks that have one.
    display_shot_visuals: HashMap<u32, u32>,
}

const CHR_RACES: &str = "DBFilesClient\\ChrRaces.dbc";
/// `ChrRaces.dbc`'s columns: the race id, then its male and its female player model.
const CHR_RACES_COLUMNS: usize = 29;
const CHR_RACES_MALE_DISPLAY: usize = 4;
const CHR_RACES_FEMALE_DISPLAY: usize = 5;

/// Each race's (male, female) player model display ids by race id, from `ChrRaces.dbc`'s bytes;
/// empty when they are not the 1.12.1 table.
pub fn race_displays(bytes: &[u8]) -> HashMap<u8, (u32, u32)> {
    let word = |i: usize| {
        bytes
            .get(i..i + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let (Some(rows), Some(columns), Some(size)) = (word(4), word(8), word(12)) else {
        return HashMap::new();
    };
    let (rows, columns, size) = (rows as usize, columns as usize, size as usize);
    if !bytes.starts_with(b"WDBC") || columns != CHR_RACES_COLUMNS || size != columns * 4 {
        return HashMap::new();
    }
    (0..rows)
        .filter_map(|r| {
            let at = |c: usize| word(20 + r * size + c * 4);
            let race = u8::try_from(at(0)?).ok()?;
            Some((
                race,
                (at(CHR_RACES_MALE_DISPLAY)?, at(CHR_RACES_FEMALE_DISPLAY)?),
            ))
        })
        .collect()
}

/// Item looks by icon, from `ItemDisplayInfo.dbc`'s (id, icon path, has a model) rows: by the
/// icon's file name in lower case, the lowest look with no model (an icon-only item, as a ring or
/// trinket is), else the lowest.
pub fn icon_displays<'a>(
    rows: impl Iterator<Item = (u32, Option<&'a str>, bool)>,
) -> HashMap<String, u32> {
    let mut best: HashMap<String, (bool, u32)> = HashMap::new();
    for (id, icon, has_model) in rows {
        let Some(icon) = icon else {
            continue;
        };
        let name = icon
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(icon)
            .to_ascii_lowercase();
        // false sorts first: a look with no model wins
        let key = (has_model, id);
        best.entry(name)
            .and_modify(|known| *known = (*known).min(key))
            .or_insert(key);
    }
    best.into_iter().map(|(name, (_, id))| (name, id)).collect()
}

/// The stock racial spells by name, from `Spell.dbc`'s (id, name, rank) rows: a row whose rank
/// says "Racial". Where a name has several, a "Racial Passive" row (a race's trait) before a
/// plain "Racial" one (`nature_resistance` has both), then the lowest id (the `tauren`'s and the
/// `night_elf`'s `nature_resistance` read the same).
pub fn racial_spells<'a>(
    rows: impl Iterator<Item = (u32, &'a str, Option<&'a str>)>,
) -> HashMap<String, u32> {
    let mut best: HashMap<String, (bool, u32)> = HashMap::new();
    for (id, name, rank) in rows {
        let Some(rank) = rank.filter(|r| r.contains("Racial")) else {
            continue;
        };
        // false sorts first: a passive row wins
        let key = (rank != "Racial Passive", id);
        best.entry(name.to_owned())
            .and_modify(|known| *known = (*known).min(key))
            .or_insert(key);
    }
    best.into_iter().map(|(name, (_, id))| (name, id)).collect()
}

impl GameData {
    /// Opens the MPQ archives of a 1.12.1 `Data` folder; `content`'s added race and class pairs
    /// wear the outfits they borrow, as on the create screen.
    pub fn open(data_dir: &Path, content: &Content) -> Result<Arc<Self>> {
        let mut chain = Chain::open(data_dir)
            .with_context(|| format!("opening the game data in {}", data_dir.display()))?;
        let maps = load_map_catalog(&mut chain).context("reading Map.dbc")?;
        let spells = load_spell_catalog(&mut chain).context("reading Spell.dbc")?;
        let missile_speeds = spells
            .iter()
            .filter(|(_, s)| s.speed > 0.0)
            .map(|(id, s)| (id, s.speed))
            .collect();
        let spell_icons = spells
            .iter()
            .filter_map(|(id, s)| {
                let path = s.icon.as_deref()?;
                Some((id, path.rsplit(['\\', '/']).next().unwrap_or(path).to_string()))
            })
            .collect();
        let spell_names = spells.iter().filter(|(_, s)| !s.name.is_empty()).map(|(id, s)| (id, s.name.clone())).collect();
        let racial_spells = racial_spells(
            spells
                .iter()
                .map(|(id, s)| (id, s.name.as_str(), s.rank.as_deref())),
        );
        let radius_rows = load_spell_radii(&mut chain).context("reading SpellRadius.dbc")?;
        let radii = spells
            .iter()
            .filter_map(|(id, s)| {
                let index = s.effect_radius_index.into_iter().find(|&i| i != 0)?;
                let radius = radius_rows.get(index)?.radius;
                (radius > 0.0).then_some((id, radius))
            })
            .collect();
        let mut outfits = CharCreateCatalog::load(&mut chain).ok();
        // the stock pairs before the added ones join the catalog
        let stock_pairs = match &outfits {
            Some(c) => (1..=8u8)
                .flat_map(|race| {
                    c.classes_for_race(race)
                        .into_iter()
                        .map(move |class| (race, class))
                })
                .collect(),
            None => HashSet::new(),
        };
        let mut outfit_items = HashMap::new();
        if let Some(c) = &mut outfits {
            let added: Vec<AddedPair> = content
                .added_pairs()
                .into_iter()
                .map(|p| AddedPair {
                    race: p.race,
                    class: p.class,
                    outfit_races: p.outfit_races,
                })
                .collect();
            c.add_pairs(&added);
            for race in 1..=8u8 {
                for class in 1..=11u8 {
                    for sex in [0, 1] {
                        for item in c.start_outfit(race, class, sex) {
                            if item.item_id != 0 {
                                outfit_items.entry(item.item_id).or_insert(*item);
                            }
                        }
                    }
                }
            }
        }
        let ranged_slot_spells = spells
            .iter()
            .filter(|(_, s)| s.ranged_slot())
            .map(|(id, _)| id)
            .collect();
        let displays = load_item_display_catalog(&mut chain).ok();
        let display_shot_visuals = displays
            .iter()
            .flat_map(|c| c.iter())
            .filter(|(_, d)| d.spell_visual != 0)
            .map(|(id, d)| (id, d.spell_visual))
            .collect();
        let icon_displays = displays
            .map(|c| {
                icon_displays(
                    c.iter()
                        .map(|(id, d)| (id, d.icon.as_deref(), d.model[0].is_some())),
                )
            })
            .unwrap_or_default();
        let race_displays = chain
            .read_file(CHR_RACES)
            .map(|b| race_displays(&b))
            .unwrap_or_default();
        Ok(Arc::new(Self {
            chain: Mutex::new(chain),
            maps,
            missile_speeds,
            radii,
            spell_icons,
            spell_names,
            stock_pairs,
            racial_spells,
            outfits,
            outfit_items,
            icon_displays,
            race_displays,
            ranged_slot_spells,
            display_shot_visuals,
        }))
    }

    /// The starting outfit the create screen dresses 1.12 race `race` as class `class` in (sex 0
    /// male, 1 female); empty when unread.
    pub fn start_outfit(&self, race: u8, class: u8, sex: u8) -> &[StartOutfitItem] {
        self.outfits
            .as_ref()
            .map_or(&[], |c| c.start_outfit(race, class, sex))
    }

    /// The item look that shows icon `icon` (`INV_Jewelry_Ring_15`, any case), if one does.
    pub fn icon_display(&self, icon: &str) -> Option<u32> {
        self.icon_displays.get(&icon.to_ascii_lowercase()).copied()
    }

    /// The starting-outfit item with entry `entry`, if any outfit has it.
    pub fn outfit_item(&self, entry: u32) -> Option<StartOutfitItem> {
        self.outfit_items.get(&entry).copied()
    }

    /// Whether the stock create screen offers race `race` as class `class` (1.12 ids); `None`
    /// when the pairs could not be read.
    pub fn stock_pair(&self, race: u8, class: u8) -> Option<bool> {
        (!self.stock_pairs.is_empty()).then(|| self.stock_pairs.contains(&(race, class)))
    }

    /// The player model of 1.12 race `race` (gender 0 male, 1 female), as `ChrRaces.dbc` has it.
    /// How many skins, faces, hair styles, hair colours and facial hairs the client offers race
    /// `race` of gender `gender` (`CharSections.dbc`, as the create screen counts them).
    pub fn look_ranges(&self, race: u8, gender: u8) -> Option<benilla_formats::DialRanges> {
        self.outfits.as_ref()?.ranges(race, gender)
    }

    /// Whether our game lets race `race` be class `class` (the stock pairs and ours, races.ron
    /// `added_classes`); `None` without the create screen's data.
    pub fn allows(&self, race: u8, class: u8) -> Option<bool> {
        Some(self.outfits.as_ref()?.allows(race, class))
    }

    pub fn player_display(&self, race: u8, gender: u8) -> Option<u32> {
        let (male, female) = *self.race_displays.get(&race)?;
        Some(if gender == 1 { female } else { male })
    }

    /// The stock racial spell named `name` (`the_human_spirit`, `hardiness` ...), if `Spell.dbc`
    /// has one.
    pub fn racial_spell(&self, name: &str) -> Option<u32> {
        self.racial_spells.get(name).copied()
    }

    /// Whether spell `spell` is a ranged-slot one (`Attributes & 0x2`), whose look the client
    /// fills from the held ranged weapon's shooting look.
    pub fn ranged_slot_spell(&self, spell: u32) -> bool {
        self.ranged_slot_spells.contains(&spell)
    }

    /// Item look `display`'s own shooting look (a `SpellVisual` id), if it has one.
    pub fn display_shot_visual(&self, display: u32) -> Option<u32> {
        self.display_shot_visuals.get(&display).copied()
    }

    /// Spell `spell`'s own radius in yards, if it has an area.
    pub fn spell_radius(&self, spell: u32) -> Option<f32> {
        self.radii.get(&spell).copied()
    }

    /// Spell `spell`'s name, if it has one.
    pub fn spell_name(&self, spell: u32) -> Option<&str> {
        self.spell_names.get(&spell).map(String::as_str)
    }

    /// Spell `spell`'s icon's file name, if it has one.
    pub fn spell_icon(&self, spell: u32) -> Option<&str> {
        self.spell_icons.get(&spell).map(String::as_str)
    }

    /// The ground of map `map_id` (a `Map.dbc` id).
    pub fn ground(self: &Arc<Self>, map_id: u32) -> Result<MapGround> {
        let dir = self
            .maps
            .directory(map_id)
            .with_context(|| format!("map {map_id} is not in Map.dbc"))?
            .to_string();
        let mut chain = self.chain.lock().unwrap();
        let tiles = MapTiles::load(&mut chain, &dir).with_context(|| format!("reading {dir}.wdt"))?;
        let building = match tiles.global_wmo() {
            Some(wmo) => Some(
                WmoFloors::load(&mut chain, wmo)
                    .with_context(|| format!("reading {dir}'s building"))?,
            ),
            // a party dungeon whose buildings stand on its tiles (the Scarlet
            // Monastery's wings) walks on their floors, as a WMO-only map walks on its one, and
            // on its terrain beside them where it has any (Stratholme's streets and squares)
            None if self.maps.is_party_dungeon(map_id) => {
                let (wmos, terrain) = tile_ground(&mut chain, &tiles);
                (!wmos.is_empty())
                    .then(|| WmoFloors::load_all_on(&mut chain, &wmos, terrain))
                    .transpose()
                    .with_context(|| format!("reading {dir}'s buildings"))?
            }
            None => None,
        };
        drop(chain);
        Ok(MapGround {
            data: self.clone(),
            map: dir,
            tiles,
            building,
            walking: Walking::default(),
            loaded: Mutex::default(),
        })
    }
}

/// Every building placed on the map's tiles, each once, placed as a WDT places its one, and the
/// tiles' terrain as world-space triangles. A tilted building is left out (Stratholme's
/// cave tunnels, far from its arena), as the floors are read upright.
fn tile_ground(chain: &mut Chain, tiles: &MapTiles) -> (Vec<benilla_formats::GlobalWmo>, Vec<Tri>) {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut terrain = Vec::new();
    for (x, y) in tiles.existing_in_radius(0.0, 0.0, 64) {
        let Ok(mesh) = load_tile_mesh(chain, tiles.map(), x, y) else {
            continue;
        };
        for c in &mesh.chunks {
            for t in c.indices.chunks_exact(3) {
                let v = |k: usize| c.positions.get(t[k] as usize).copied();
                if let (Some(a), Some(b), Some(d)) = (v(0), v(1), v(2)) {
                    terrain.push([a, b, d]);
                }
            }
        }
        for w in mesh.wmos {
            if w.rotation[0] != 0.0 || w.rotation[2] != 0.0 {
                continue;
            }
            if seen.insert(w.unique_id) {
                out.push(benilla_formats::GlobalWmo {
                    model: w.model,
                    position: w.position,
                    rotation: w.rotation,
                    doodad_set: w.doodad_set,
                    name_set: w.name_set,
                });
            }
        }
    }
    (out, terrain)
}

/// A tile's terrain chunks, or `None` where the map has no tile.
type Tile = Option<Arc<Vec<ChunkMesh>>>;

/// One map's ground: its terrain, its tiles read as they are first needed, or, on a WMO-only map,
/// its building's floors.
pub struct MapGround {
    data: Arc<GameData>,
    map: String,
    tiles: MapTiles,
    /// A WMO-only map's building (Dire Maul): its floors stand in for the terrain it lacks.
    building: Option<WmoFloors>,
    /// The walk graph over the building's floors, built as searches reach it.
    walking: Walking,
    loaded: Mutex<HashMap<(u32, u32), Tile>>,
}

impl MapGround {
    fn chunks(&self, tile: (u32, u32)) -> Tile {
        if let Some(t) = self.loaded.lock().unwrap().get(&tile) {
            return t.clone();
        }
        let mesh = load_tile_mesh(
            &mut self.data.chain.lock().unwrap(),
            &self.map,
            tile.0,
            tile.1,
        )
        .ok();
        let chunks = mesh.map(|m| Arc::new(m.chunks));
        self.loaded.lock().unwrap().insert(tile, chunks.clone());
        chunks
    }
}

impl World for MapGround {
    fn height(&self, x: f32, y: f32) -> Option<f32> {
        if let Some(b) = &self.building {
            return b.top(x, y);
        }
        let chunks = self.chunks(self.tiles.tile_at(x, y))?;
        terrain_height_at(&chunks, [x, y, 0.0])
    }

    fn floor(&self, x: f32, y: f32, near_z: f32) -> Option<f32> {
        let below = match &self.building {
            Some(b) => b.floor(x, y, near_z),
            None => self.height(x, y),
        };
        // floor laid over a hole, when it is the nearest below
        let laid = self.walking.platform_heights(x, y).into_iter().filter(|&z| z <= near_z + FLOOR_STEP).max_by(f32::total_cmp);
        match (below, laid) {
            (Some(b), Some(l)) => Some(b.max(l)),
            (b, l) => b.or(l),
        }
    }

    fn clear_line(&self, from: [f32; 3], to: [f32; 3]) -> bool {
        !self.walking.crosses_blocked((from[0], from[1]), (to[0], to[1])) && self.building.as_ref().is_none_or(|b| b.clear_line(from, to))
    }

    fn walk(&self, from: [f32; 3], to: (f32, f32), reach: f32) -> Walk {
        let Some(b) = &self.building else {
            return Walk::Straight;
        };
        match self.walking.walk(b, from, to, reach) {
            Some((distance, z)) => Walk::Reached { distance, z },
            None => Walk::Unreachable,
        }
    }

    fn toward(&self, from: (f32, f32), hero: [f32; 3]) -> Option<(f32, f32)> {
        self.walking.toward(self.building.as_ref()?, from, hero)
    }

    fn missile_speed(&self, spell: u32) -> f32 {
        self.data.missile_speeds.get(&spell).copied().unwrap_or(0.0)
    }

    fn set_blocked(&self, lines: &[((f32, f32), (f32, f32))]) {
        self.walking.set_blocked(lines);
    }

    fn set_platforms(&self, discs: &[((f32, f32), f32, f32)]) {
        self.walking.set_platforms(discs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs a 1.12.1 install (found as benilla finds it); skipped without one.
    #[test]
    fn every_arena_has_ground_at_its_spawn_and_fireballs_fly() {
        let dir = benilla_formats::wow_data_or_skip!();
        let names = esurv_content::ClientNames::read(&dir).unwrap();
        let content_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = esurv_content::Content::load_with_names(content_dir, &names).unwrap();
        let data = GameData::open(&dir, &content).unwrap();
        // every race has its player models, the human's and the gnome's as the client has them
        for race in &content.races {
            assert!(data.player_display(race.wow_race_id, 1).is_some(), "{}", race.id);
        }
        assert_eq!((data.player_display(1, 0), data.player_display(1, 1)), (Some(49), Some(50)));
        assert_eq!(data.player_display(7, 1), Some(1564));
        for zone in &content.zones {
            let ground = data.ground(zone.map_id).unwrap();
            // the floor nearest the spawn's height, on terrain or in a WMO-only map's building
            let h = ground
                .floor(zone.spawn.x, zone.spawn.y, zone.spawn.z)
                .unwrap_or_else(|| panic!("{}: no ground at the spawn", zone.id));
            assert!(
                (h - zone.spawn.z).abs() < 15.0,
                "{}: ground {h}, spawn height {}",
                zone.id,
                zone.spawn.z
            );
        }
        // a WMO-only map has ground too: Dire Maul's floor at a `.gps` spot inside it, and a
        // wall in the way of some sight lines
        let dire_maul = data.ground(429).unwrap();
        let (x, y, z) = (806.627_26, 482.043, 37.317_01);
        let floor = dire_maul.floor(x, y, z).expect("Dire Maul's floor");
        assert!((floor - z).abs() < 0.1, "{floor}");
        assert!(dire_maul.clear_line([x, y, z + 1.5], [x + 5.0, y, z + 1.5]));
        assert!(
            (0..72).any(|i| {
                let a = (i as f32 * 5.0).to_radians();
                !dire_maul.clear_line([x, y, z + 1.5], [x + 60.0 * a.cos(), y + 60.0 * a.sin(), z + 1.5])
            }),
            "Dire Maul's walls block sight"
        );
        assert!(
            data.ground(0).unwrap().clear_line([0.0; 3], [1000.0, 0.0, 0.0]),
            "terrain never blocks sight"
        );
        let ground = data.ground(0).unwrap();
        assert!(ground.missile_speed(133) > 0.0, "Spell 133 flies");
        assert_eq!(ground.missile_speed(122), 0.0, "Spell 122 does not");
        // the sizes the scaled looks are given are the spells' own radii
        for (spell, radius) in [
            (10, 8.0),
            (26573, 8.0),
            (122, 10.0),
            (1449, 10.0),
            (2120, 5.0),
        ] {
            assert_eq!(data.spell_radius(spell), Some(radius), "{spell}");
        }
        assert!(
            data.spell_radius(8187).is_some(),
            "Spell 8187's pulse has an area"
        );
        assert_eq!(data.spell_radius(133), None, "Spell 133 has none");
        // the stock create screen's pairs: no dwarf shaman, and the dead dwarf mage row stays dead
        assert_eq!(data.stock_pair(2, 7), Some(true), "orc shaman");
        assert_eq!(data.stock_pair(3, 7), Some(false), "dwarf shaman");
        assert_eq!(data.stock_pair(3, 8), Some(false), "dwarf mage");
        // the starting outfits, each piece by its item entry: the human warrior's sword, and a
        // dwarf shaman (an added pair) in a borrowed outfit
        let sword = data
            .start_outfit(1, 1, 0)
            .iter()
            .find(|i| i.inv_type == 21)
            .copied()
            .expect("the human warrior's sword");
        assert_ne!(sword.item_id, 0);
        assert_eq!(data.outfit_item(sword.item_id), Some(sword));
        assert!(!data.start_outfit(3, 7, 0).is_empty(), "dwarf shaman");
        let answer = crate::looks::item_info(&content, Some(&data), sword.item_id).unwrap();
        assert_eq!(
            (answer.display_id, answer.inventory_type),
            (sword.display_id, 21)
        );
        // every starting-outfit weapon and shield is in the content, with the look the data gives
        for race in 1..=8u8 {
            for class in 1..=11u8 {
                for sex in [0, 1] {
                    for item in data.start_outfit(race, class, sex) {
                        if matches!(item.inv_type, 13..=15 | 17 | 21..=23 | 25 | 26) {
                            let w = content.outfit_weapon(item.item_id).unwrap_or_else(|| {
                                panic!("outfit_weapons.ron lacks item {}", item.item_id)
                            });
                            assert_eq!(w.display, item.display_id, "item {}", item.item_id);
                        }
                    }
                }
            }
        }
        // every ring, trinket and amulet icon has a look that shows it; a legendary amulet's spell
        // icon may not, and then an amulet's stands in
        let icons = content
            .jewelry
            .iter()
            .map(|j| &j.icon)
            .chain(content.amulets.iter().map(|a| &a.icon));
        for icon in icons {
            assert!(data.icon_display(icon).is_some(), "no look shows {icon}");
        }
        assert!(data.icon_display(crate::looks::LEGENDARY_ITEM_ICON).is_some(), "the legendary item look");
        // every race's passives have a stock racial to show in the spellbook
        for race in &content.races {
            for p in &race.passives {
                assert!(
                    data.racial_spell(p.stock_spell_name()).is_some(),
                    "{}: {} has no stock racial",
                    race.id,
                    p.name
                );
            }
        }
    }

    #[test]
    fn a_racial_is_a_row_whose_rank_says_racial_a_passive_one_first() {
        let rows = [
            (20599, "Spell 20599", Some("Racial Passive")),
            (99, "Spell 20599", Some("Racial Passive")),
            (7, "Spell 20573", Some("Rank 1")),
            (20577, "Spell 20577", Some("Racial")),
            (5, "Nothing", None),
            (4081, "Spell 848", Some("Racial")),
            (20583, "Spell 848", Some("Racial Passive")),
            (20551, "Spell 848", Some("Racial Passive")),
        ];
        let found = racial_spells(rows.into_iter());
        assert_eq!(found.get("Spell 20599"), Some(&99));
        assert_eq!(found.get("Spell 20577"), Some(&20577));
        assert_eq!(found.get("Spell 20573"), None, "not a racial rank");
        assert_eq!(
            found.get("Spell 848"),
            Some(&20551),
            "a passive row first"
        );
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn an_icon_shows_on_its_lowest_look_with_no_model() {
        let rows = [
            (9, Some("Interface\\Icons\\INV_Jewelry_Ring_15"), true),
            (12, Some("Interface\\Icons\\INV_Jewelry_Ring_15"), false),
            (30, Some("Interface\\Icons\\INV_Jewelry_Ring_15"), false),
            (4, Some("Interface\\Icons\\INV_Sword_04"), true),
            (5, None, false),
        ];
        let found = icon_displays(rows.into_iter());
        assert_eq!(found.get("inv_jewelry_ring_15"), Some(&12));
        assert_eq!(
            found.get("inv_sword_04"),
            Some(&4),
            "a model's look where none is bare"
        );
        assert_eq!(found.len(), 2);
    }

    /// The Scarlet Monastery on the real map: from the spawn, the hall
    /// behind the walled doorway and the altar past the shut cathedral door are out of reach; the
    /// door opened, the altar is walked to.
    #[test]
    fn the_scarlet_monastery_keeps_to_its_courtyard_until_the_cathedral_opens() {
        let dir = benilla_formats::wow_data_or_skip!();
        let names = esurv_content::ClientNames::read(&dir).unwrap();
        let content_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = esurv_content::Content::load_with_names(content_dir, &names).unwrap();
        let sm = content.zone("scarlet").unwrap();
        let data = GameData::open(&dir, &content).unwrap();
        let ground = data.ground(sm.map_id).unwrap();
        let spawn = [sm.spawn.x, sm.spawn.y, sm.spawn.z];
        let line = |w: &esurv_content::Wall| (w.from, w.to);
        let (wall, door) = (line(&sm.walls[0]), line(&sm.doors[0].doorway));
        let altar = (1154.0, 1398.45);
        // floors behind the doorway, on a grid
        let behind: Vec<(f32, f32)> = (0..20).flat_map(|i| (0..20).map(move |j| (840.0 + 2.0 * i as f32, 1380.0 + 2.0 * j as f32))).collect();
        let reached = |w: Walk| matches!(w, Walk::Reached { .. });
        // the courtyard's floor at the spawn's .gps
        let floor = ground.floor(spawn[0], spawn[1], spawn[2]).expect("a floor at the spawn");
        assert!((floor - spawn[2]).abs() < 0.5, "{floor}");
        // with nothing blocked, the hall behind the doorway is walked to
        assert!(behind.iter().any(|&b| reached(ground.walk(spawn, b, 120.0))), "the hall behind is reachable unwalled");
        ground.set_blocked(&[wall, door]);
        assert!(!behind.iter().any(|&b| reached(ground.walk(spawn, b, 120.0))), "the doorway is walled");
        assert!(!reached(ground.walk(spawn, altar, 400.0)), "the cathedral is shut");
        ground.set_blocked(&[wall]);
        assert!(reached(ground.walk(spawn, altar, 400.0)), "the door opened, the altar is walked to");
    }

    /// Our game's race and class pairs: the stock ones and ours (races.ron `added_classes`: a
    /// human hunter, an orc mage, a dwarf shaman, an undead paladin, a gnome priest, a troll
    /// warlock), and no other.
    #[test]
    fn our_race_and_class_pairs_are_the_stock_ones_and_ours() {
        let dir = benilla_formats::wow_data_or_skip!();
        let names = esurv_content::ClientNames::read(&dir).unwrap();
        let content_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = esurv_content::Content::load_with_names(content_dir, &names).unwrap();
        let data = GameData::open(&dir, &content).unwrap();
        let ids = |race: &str, class: &str| {
            (content.races.iter().find(|r| r.id == race).unwrap().wow_race_id, content.class(class).unwrap().wow_class_id as u8)
        };
        for (race, class) in [("human", "hunter"), ("orc", "mage"), ("dwarf", "shaman"), ("undead", "paladin"), ("gnome", "priest"), ("troll", "warlock"), ("tauren", "druid"), ("night_elf", "druid")] {
            let (r, c) = ids(race, class);
            assert_eq!(data.allows(r, c), Some(true), "{race} {class}");
        }
        for (race, class) in [("tauren", "paladin"), ("gnome", "druid"), ("orc", "druid"), ("night_elf", "shaman")] {
            let (r, c) = ids(race, class);
            assert_eq!(data.allows(r, c), Some(false), "{race} {class}");
        }
        // every hand-made character is a pair our game allows, its face within its race's dials
        for ch in &content.player_characters {
            let (r, c) = ids(&ch.race, &ch.class);
            assert_eq!(data.allows(r, c), Some(true), "{}: {} {}", ch.id, ch.race, ch.class);
            let dials = data.look_ranges(r, ch.gender.id()).unwrap();
            let Some(l) = ch.looks else { continue };
            assert!(
                l.skin < dials.skin && l.face < dials.face && l.hair_style < dials.hair_style && l.hair_color < dials.hair_color,
                "{}: {l:?} past {dials:?}",
                ch.id
            );
            assert!(l.facial_hair < dials.facial_hair.max(1), "{}: {l:?} past {dials:?}", ch.id);
        }
    }

    /// The Sunken Temple's pit: plugged with a round table whose top
    /// sits flush with the floor round it, the disc beneath it floor the run stands and walks on,
    /// and every spot of the pit under the table's top.
    #[test]
    fn the_sunken_temple_s_pit_is_plugged_flush_with_its_floor() {
        let dir = benilla_formats::wow_data_or_skip!();
        let names = esurv_content::ClientNames::read(&dir).unwrap();
        let content_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = esurv_content::Content::load_with_names(content_dir, &names).unwrap();
        let st = content.zone("sunken_temple").unwrap();
        let data = GameData::open(&dir, &content).unwrap();
        let ground = data.ground(st.map_id).unwrap();
        let pit = st.platforms[0];
        let (c, z) = (pit.centre, pit.z);
        let spawn = [st.spawn.x, st.spawn.y, st.spawn.z];
        // open, the pit drops far below its rim
        assert!(ground.floor(c.0, c.1, z + 1.0).is_some_and(|b| b < z - 50.0));
        let discs: Vec<_> = st.platforms.iter().map(|p| (p.centre, p.radius, p.z)).collect();
        ground.set_platforms(&discs);
        assert_eq!(ground.floor(c.0, c.1, z + 1.0), Some(z), "flush with the floor round it");
        let across = (c.0 - 4.0, c.1);
        match ground.walk(spawn, across, 60.0) {
            Walk::Reached { distance, z: at } => {
                assert!((at - z).abs() < 0.1 && distance < 16.0, "{distance} {at}")
            }
            w => panic!("the pit is walked across: {w:?}"),
        }
        // every spot of the pit lies under the table's top: its hull, scaled and placed
        let mut chain = data.chain.lock().unwrap();
        let hull = benilla_formats::load_m2_collision_hull(
            &mut chain,
            r"World\Generic\Human\Passive Doodads\Tables\InnTableSmall.mdx",
        )
        .unwrap();
        assert!(st.props.iter().all(|p| p.display_id == 6508 && p.at.facing == 0.0));
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &hull.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        for p in &st.props {
            let top = p.at.z + hi[2] * p.scale;
            // a hair under the floor, so the model does not clip through it
            assert!(top <= z && top > z - 0.25, "the table's top at {top}");
        }
        let covered = |x: f32, y: f32| {
            st.props.iter().any(|p| {
                let (px, py, s) = (p.at.x, p.at.y, p.scale);
                (px + lo[0] * s..=px + hi[0] * s).contains(&x) && (py + lo[1] * s..=py + hi[1] * s).contains(&y)
            })
        };
        for k in 0..72 {
            let a = (k as f32 * 5.0).to_radians();
            for r in [0.0, 2.5, 5.0] {
                let (x, y) = (c.0 + r * a.cos(), c.1 + r * a.sin());
                assert!(covered(x, y), "({x}, {y}) is open");
            }
        }
    }
}
