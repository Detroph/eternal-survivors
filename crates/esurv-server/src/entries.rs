//! The server's own fixed entries, past the content's creatures (`esurv_content::entries`,
//! which end at `CREATURES_END`, 110 000): the creatures every run shows, then the game objects
//! the pickups are. Wire-only, never saved.

use esurv_content::entries::CREATURES_END;

/// The experience orb, a unit.
pub const ORB: u32 = CREATURES_END;
/// Every totem.
pub const TOTEM: u32 = CREATURES_END + 1;
/// The invisible target unit boss looks play on (`game::bunnies`).
pub const BUNNY: u32 = CREATURES_END + 2;
/// A boss's object on the ground shown as a creature model (a troll ward), unselectable.
pub const GROUND_UNIT: u32 = CREATURES_END + 3;

/// The first game-object entry.
const OBJECTS_FROM: u32 = CREATURES_END + 100;
/// A pile of gold.
pub const GOLD: u32 = OBJECTS_FROM;
/// A battleground cache.
pub const BATTLEGROUND_CACHE: u32 = OBJECTS_FROM + 1;
pub const SPEED_RUNE: u32 = OBJECTS_FROM + 2;
pub const BERSERKER_RUNE: u32 = OBJECTS_FROM + 3;
pub const RESTORATION_RUNE: u32 = OBJECTS_FROM + 4;
pub const MAGNET: u32 = OBJECTS_FROM + 5;
/// The two objects of a magnet's beam of light.
pub const MAGNET_BEAM_LIGHT: u32 = OBJECTS_FROM + 6;
pub const MAGNET_BEAM_GLOW: u32 = OBJECTS_FROM + 7;
/// A zone's props: this plus the prop's display id, one entry per look (our numbering).
pub const ZONE_PROPS_FROM: u32 = OBJECTS_FROM + 100;
/// The rare cache (free to open), after the magnet's beam.
pub const RARE_CACHE: u32 = OBJECTS_FROM + 8;

/// The objects bosses put on the ground for a hero to walk over: this
/// plus the object's display id, one entry per look; past the zone props' range.
pub const GROUND_OBJECTS_FROM: u32 = 200_000;

/// Rares: a creature entry of their own per elite they copy (this plus the elite's entry past
/// the content's first creature), so the creature query can give them the rare rank (the silver
/// dragon) while the elite keeps the gold one. Past the zone props' range.
pub const RARES_FROM: u32 = 300_000;

/// The entry a rare copy of the creature `elite` shows under.
pub fn rare(elite: u32) -> u32 {
    RARES_FROM + elite.saturating_sub(esurv_content::entries::CREATURES_FROM)
}

/// The elite entry a rare entry copies, if `entry` is a rare's.
pub fn rare_of(entry: u32) -> Option<u32> {
    let past = entry.checked_sub(RARES_FROM)?;
    (past < CREATURES_END - esurv_content::entries::CREATURES_FROM).then(|| esurv_content::entries::CREATURES_FROM + past)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rare_entries_map_back_to_their_elites_and_stay_clear_of_the_others() {
        let elite = esurv_content::entries::CREATURES_FROM + 1_234;
        assert_eq!(rare_of(rare(elite)), Some(elite));
        assert_eq!(rare_of(elite), None);
        assert_eq!(rare_of(ZONE_PROPS_FROM + 65_535), None);
        assert!(rare(CREATURES_END - 1) < 1 << 24, "fits a unit guid's entry bits");
    }
}
