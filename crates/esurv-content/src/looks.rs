//! What the server and the client must agree on to show the added race and class pairs: the pairs
//! themselves with the outfit each borrows, and the composed display of a race's own totem look.

use crate::{Content, Race};

/// Every totem's stock look (`CreatureDisplayInfo.dbc`).
pub const TOTEM_DISPLAY: u32 = 4587;
/// `magma_totem`'s own stock look.
pub const MAGMA_TOTEM_DISPLAY: u32 = 4589;
/// The stock totem looks a race's own totem look is composed with.
pub const TOTEM_DISPLAYS: [u32; 2] = [TOTEM_DISPLAY, MAGMA_TOTEM_DISPLAY];

/// Our composed displays are numbered from here, far past the 1.12 display table (which ends
/// below 20 000): `9 000 000 + race × 100 000 + the stock look`.
const COMPOSED_DISPLAYS_FROM: u32 = 9_000_000;

/// A race and class pair the game adds, in 1.12 ids, and the races whose stock outfit for the
/// class it borrows, in the order they are tried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedPair {
    pub race: u8,
    pub class: u8,
    pub outfit_races: Vec<u8>,
}

impl Content {
    /// The pairs `races.ron` adds (`added_classes`), with the outfit rule: a pair wears
    /// its class's stock outfit from a race of its own faction, else of the other, lowest race id
    /// first among those that have one.
    pub fn added_pairs(&self) -> Vec<AddedPair> {
        let mut out = Vec::new();
        for race in &self.races {
            for class in race.added_classes.iter().filter_map(|c| self.class(c)) {
                let mut others: Vec<&Race> = self.races.iter().filter(|r| r.id != race.id).collect();
                // own faction first, then by race id
                others.sort_by_key(|r| (r.faction != race.faction, r.wow_race_id));
                out.push(AddedPair {
                    race: race.wow_race_id,
                    class: class.wow_class_id as u8,
                    outfit_races: others.iter().map(|r| r.wow_race_id).collect(),
                });
            }
        }
        out
    }

    /// Whether 1.12 race `race` and class `class` is one of the added pairs.
    pub fn is_added_pair(&self, race: u8, class: u8) -> bool {
        self.race_by_wow_id(race)
            .is_some_and(|r| r.added_classes.iter().filter_map(|c| self.class(c)).any(|c| c.wow_class_id == u32::from(class)))
    }

    /// Every composed totem look: (composed display id, the stock look under it, the race's look).
    pub fn composed_totems(&self) -> Vec<(u32, u32, &crate::TotemLook)> {
        self.races
            .iter()
            .filter_map(|r| Some((r, r.totem_look.as_ref()?)))
            .flat_map(|(r, look)| TOTEM_DISPLAYS.iter().map(move |&base| (composed_display(r, base), base, look)))
            .collect()
    }
}

/// The look of a totem of stock look `base` placed by a hero of `race`: the composed one when
/// the race has a totem look of its own, else `base`.
pub fn totem_look(race: &Race, base: u32) -> u32 {
    if race.totem_look.is_some() {
        composed_display(race, base)
    } else {
        base
    }
}

fn composed_display(race: &Race, base: u32) -> u32 {
    COMPOSED_DISPLAYS_FROM + u32::from(race.wow_race_id) * 100_000 + base
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    #[test]
    fn the_six_added_pairs_borrow_their_own_factions_outfit_first() {
        let c = content();
        let pairs = c.added_pairs();
        let ids: Vec<(u8, u8)> = pairs.iter().map(|p| (p.race, p.class)).collect();
        // as race and class ids: dwarf shaman, human hunter, gnome priest, orc mage, troll warlock, undead paladin
        for want in [(3, 7), (1, 3), (7, 5), (2, 8), (8, 9), (5, 2)] {
            assert!(ids.contains(&want), "{want:?} in {ids:?}");
        }
        assert_eq!(ids.len(), 6, "{ids:?}");
        let dwarf_shaman = pairs.iter().find(|p| (p.race, p.class) == (3, 7)).unwrap();
        // the Alliance first by id (human, night elf, gnome), then the Horde (orc ...)
        assert_eq!(dwarf_shaman.outfit_races, vec![1, 4, 7, 2, 5, 6, 8]);
        assert!(c.is_added_pair(3, 7));
        assert!(!c.is_added_pair(3, 8), "the dead dwarf mage row stays dead");
    }

    #[test]
    fn only_the_dwarf_has_its_own_totem_look() {
        let c = content();
        let composed = c.composed_totems();
        assert_eq!(composed.iter().map(|t| (t.0, t.1)).collect::<Vec<_>>(), vec![(9_304_587, 4587), (9_304_589, 4589)]);
        let dwarf = c.race("dwarf").unwrap();
        let orc = c.race("orc").unwrap();
        assert_eq!(totem_look(dwarf, MAGMA_TOTEM_DISPLAY), 9_304_589);
        assert_eq!(totem_look(orc, TOTEM_DISPLAY), TOTEM_DISPLAY, "other races keep the stock look");
        // the mace at scale 1.6, its grip (0.30 from the pommel) at the
        // ground, the glow lowered to just over the head's top (0.72 over the grip)
        let look = composed[0].2;
        assert_eq!(look.scale, 1.6);
        assert!((look.sink - 0.30 * look.scale).abs() < 0.01);
        let top = 0.72 * look.scale;
        assert!((STOCK_TOTEM_GLOW + look.raise_effects - top - 0.05).abs() < 0.01);
        assert!(look.hide_totem_body);
    }

    #[test]
    fn berserks_buff_wears_the_idol_and_lacerate_rends() {
        let c = content();
        let berserk = c.ability("berserk").unwrap();
        assert_eq!(berserk.icon, "INV_Relics_IdolofFerocity");
        assert!(crate::option_looks::BUFF_ICONS.contains(&(berserk.look_spell, "INV_Relics_IdolofFerocity")));
        let lacerate = c.ability("lacerate").unwrap();
        assert_eq!((lacerate.icon.as_str(), lacerate.look_spell), ("Ability_Gouge", 772));
    }

    #[test]
    fn deadly_poison_shows_the_green_cloud_with_its_own_icon() {
        let c = content();
        let poison = c.ability("deadly_poison").unwrap();
        // Poison State: the green poison cloud on the afflicted, and its sound
        assert_eq!(poison.look_spell, 12_881);
        assert!(crate::option_looks::DEBUFF_ICONS.contains(&(poison.look_spell, poison.icon.as_str())));
        assert!(crate::option_looks::icon_overrides().contains(&(12_881, "Ability_Rogue_DualWeild")));
    }

    /// Where the stock totem's glow starts over the ground as it stands (yards).
    const STOCK_TOTEM_GLOW: f32 = 1.61;
}
