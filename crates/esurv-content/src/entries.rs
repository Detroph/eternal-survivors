//! The creature entries the client is told about, numbered as the content loads and never saved
//! (our model): a zone's roster enemy is `100_000 + zone × 100 + its place in the roster`, a boss
//! `100_000 + zone × 100 + 90 + its place among the bosses`, a pet `100_000 + 99 × 100 + its place
//! in pets.ron`; zones count from 0 in difficulty order. The server's own fixed entries (the
//! experience orb, totems, pickups) start at [`CREATURES_END`].

/// The first creature entry of ours.
pub const CREATURES_FROM: u32 = 100_000;
/// Entries each zone has: its roster's, then its bosses' from [`BOSSES_FROM`].
const PER_ZONE: u32 = 100;
/// A zone's bosses start at this place among its entries.
const BOSSES_FROM: u32 = 90;
/// How many zones the numbering has room for; the pets take the place after the last.
pub const MAX_ZONES: usize = 99;
/// How many roster enemies a zone has room for.
pub const MAX_ROSTER: usize = BOSSES_FROM as usize;
/// How many bosses a zone has room for.
pub const MAX_BOSSES: usize = (PER_ZONE - BOSSES_FROM) as usize;
/// How many pets the numbering has room for.
pub const MAX_PETS: usize = PER_ZONE as usize;
/// The first entry past the content's creatures.
pub const CREATURES_END: u32 = CREATURES_FROM + (MAX_ZONES as u32 + 1) * PER_ZONE;

/// The entry of the roster enemy at place `roster` of zone `zone` (difficulty order).
pub fn enemy_entry(zone: usize, roster: usize) -> u32 {
    CREATURES_FROM + zone as u32 * PER_ZONE + roster as u32
}

/// The entry of the boss at place `boss` of zone `zone` (difficulty order).
pub fn boss_entry(zone: usize, boss: usize) -> u32 {
    CREATURES_FROM + zone as u32 * PER_ZONE + BOSSES_FROM + boss as u32
}

/// Zone places from here on number faction rosters (a zone that fights
/// by the heroes' faction): zone `zone`'s roster fought by `side` (0 the Alliance, 1 the Horde)
/// numbers as the zone at place `FACTION_ROSTERS_FROM + zone × 2 + side`, so no two rosters share
/// an entry. Zones themselves stay below it.
pub const FACTION_ROSTERS_FROM: usize = 50;

/// The zone place whose entries zone `zone`'s roster fought by `side` takes (0 the Alliance's
/// heroes, 1 the Horde's).
pub fn faction_roster_place(zone: usize, side: usize) -> usize {
    FACTION_ROSTERS_FROM + zone * 2 + side
}

/// The entry of the pet at place `pet` in pets.ron.
pub fn pet_entry(pet: usize) -> u32 {
    CREATURES_FROM + MAX_ZONES as u32 * PER_ZONE + pet as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_never_meet() {
        assert_eq!(enemy_entry(0, 0), 100_000);
        assert_eq!(enemy_entry(MAX_ZONES - 1, MAX_ROSTER - 1), 109_889);
        assert_eq!(boss_entry(2, 1), 100_291);
        assert_eq!(boss_entry(MAX_ZONES - 1, MAX_BOSSES - 1), 109_899);
        assert_eq!((pet_entry(0), pet_entry(MAX_PETS - 1)), (109_900, 109_999));
        assert_eq!(CREATURES_END, 110_000);
    }
}
