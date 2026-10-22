//! The class gear's icons. A piece names an item display (its look, from the content); the
//! display's inventory icon is in the player's own `ItemDisplayInfo.dbc`, read once by
//! [`ItemIcons::load`]. Without the data every piece shows its slot's empty picture instead.

use benilla_formats::ItemDisplayCatalog;
use bevy::prelude::*;

use super::gear::ClassGear;

/// The item displays' icons, when the player's data could be read.
#[derive(Resource, Default)]
pub struct ItemIcons(Option<ItemDisplayCatalog>);

impl ItemIcons {
    /// Reads the catalog from the player's data folder, if there is one.
    pub fn load() -> Self {
        let Some(dir) = benilla_formats::wow_data() else {
            return Self(None);
        };
        let catalog = benilla_formats::open_chain(&dir)
            .and_then(|mut chain| benilla_formats::load_item_display_catalog(&mut chain));
        match catalog {
            Ok(c) => Self(Some(c)),
            Err(e) => {
                warn!("hero sheet: the gear's icons did not load: {e:#}");
                Self(None)
            }
        }
    }

    /// The interface path of display `display`'s icon (`Interface\Icons\INV_Helmet_01`).
    pub fn icon(&self, display: u32) -> Option<&str> {
        self.0.as_ref()?.get(display)?.icon.as_deref()
    }

    /// A class piece's icon path in look `look`: its display's icon, else its slot's empty picture.
    pub fn piece_icon(&self, gear: &ClassGear, slot: u32, look: u32) -> String {
        self.icon(gear.display(slot, look))
            .map_or_else(|| gear.empty_slot_art(slot).to_owned(), str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero_sheet::gear::tests::content;

    #[test]
    fn without_the_data_a_piece_shows_its_empty_slot() {
        let g = ClassGear::of(content(), "warrior").unwrap();
        let icons = ItemIcons::default();
        assert_eq!(icons.icon(g.display(0, 1)), None);
        assert_eq!(icons.piece_icon(&g, 0, 1), g.empty_slot_art(0));
    }

    /// Every zone's look of every class (Naxxramas's waiting ones too)
    /// and every loadout weapon's look in every zone's armaments is an item display of the
    /// player's data, with an icon, and a weapon's with a model to hold.
    #[test]
    fn every_gear_and_armament_look_is_in_the_players_data() {
        let dir = benilla_formats::wow_data_or_skip!();
        let catalog = benilla_formats::open_chain(&dir)
            .and_then(|mut chain| benilla_formats::load_item_display_catalog(&mut chain))
            .expect("the item displays read");
        let c = content();
        let mut missing = Vec::new();
        for set in &c.gear.sets {
            for look in &set.looks {
                for (slot, &d) in look.displays.iter().enumerate() {
                    if d != 0 && catalog.get(d).and_then(|i| i.icon.as_ref()).is_none() {
                        missing.push(format!("{} {} row {} slot {slot}: {d}", set.class, look.zone, look.row));
                    }
                }
            }
            for l in c.loadouts_of(&set.class) {
                for w in &l.weapons {
                    for a in &w.looks {
                        let shown = catalog.get(a.display);
                        if shown.and_then(|i| i.icon.as_ref()).is_none() || shown.is_some_and(|i| i.model[0].is_none()) {
                            missing.push(format!("{}/{} {} {}: {} ({})", set.class, l.id, a.zone, a.row, a.name, a.display));
                        }
                    }
                }
            }
        }
        assert!(missing.is_empty(), "looks the player's data lacks: {missing:#?}");
    }
}
