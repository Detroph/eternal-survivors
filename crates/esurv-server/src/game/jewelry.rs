//! The run's rings, trinkets and amulet as items of the hero's own, for its character pane: the
//! amulet in the neck slot, the first two rings in the finger slots and the first two trinkets in
//! the trinket slots (in the order put on), the rest in the backpack. Made when taken; an upgrade
//! or an empowering is another entry (`looks::jewelry`), so the old item goes and a new one takes
//! its place; a fresh run takes them all away. Only the hero's own client is told: the body shows
//! no jewelry, as in 1.12.

use std::collections::BTreeMap;

use esurv_content::JewelrySlot;
use esurv_sim::game::choices::Offer;
use esurv_sim::game::loot::Pickup;
use esurv_sim::game::offers::RARITIES;
use esurv_sim::game::run::{Event, Hero};

use super::{own_item, Game, HIGHGUID_ITEM};
use crate::looks::jewelry::{amulet_entry, ring_or_trinket_entry};
use crate::packets;
use crate::update::{field, update_object, Fields, ObjectUpdate};
use crate::wire::Packet;

/// The equipment slots the jewelry is held in: the neck, the two fingers, the two trinkets.
const NECK_SLOT: u16 = 1;
const FINGER_SLOTS: [u16; 2] = [10, 11];
const TRINKET_SLOTS: [u16; 2] = [12, 13];
/// The backpack's slots, for the rings and trinkets past those.
const PACK_SLOTS: u16 = 16;
/// A jewelry item's guid: this bit apart from the worn gear's, the hero, a number.
const JEWELRY_GUID_TAG: u64 = 1 << 40;

/// A jewelry item a hero's client holds: its guid and entry, by the player field holding it.
pub(super) type Held = BTreeMap<u16, (u64, u32)>;

/// The card a granted piece is worn as (`Game::grant_jewelry`): `id` a ring or trinket of
/// `jewelry.ron` at its base value (`id@rarity`, 0 common to 4 legendary; common without), an
/// amulet of `amulets.ron`, or `legendary:<spell>` for that spell's remade legendary amulet.
fn granted_offer(content: &esurv_content::Content, granted: &str) -> Option<Offer> {
    let (id, rarity) = match granted.split_once('@') {
        Some((id, r)) => (id, r.parse::<usize>().ok()?),
        None => (granted, 0),
    };
    if let Some(j) = content.jewelry.iter().find(|j| j.id == id) {
        return Some(Offer::NewJewelry {
            id: j.id.clone(),
            value: j.value,
            rarity: *RARITIES.get(rarity)?,
        });
    }
    let amulet = match id.strip_prefix("legendary:") {
        Some(spell) => {
            let l = esurv_sim::game::amulets::legendary_named(content, spell)?;
            esurv_sim::game::amulets::legendary_for(l)
        }
        None => content.amulets.iter().find(|a| a.id == id)?.clone(),
    };
    Some(Offer::Amulet {
        amulet: Box::new(amulet),
    })
}

/// A granted rune (`Game::grant`): its kind and how many yards ahead of the hero it lies.
fn granted_rune(granted: &str) -> Option<(Pickup, f32)> {
    let (name, ahead) = match granted.split_once(':') {
        Some((n, yards)) => (n, yards.trim().parse::<f32>().ok()?),
        None => (granted, 0.0),
    };
    let kind = match name.trim() {
        "speed_rune" => Pickup::SpeedRune,
        "berserker_rune" => Pickup::BerserkerRune,
        "restoration_rune" => Pickup::RestorationRune,
        _ => return None,
    };
    Some((kind, ahead))
}

/// The player field (an equipment or backpack slot's) of equipment slot `slot`.
fn equipment_field(slot: u16) -> u16 {
    field::PLAYER_INV_SLOT_HEAD + 2 * slot
}

/// The player field of backpack slot `slot`.
fn pack_field(slot: u16) -> u16 {
    field::PLAYER_PACK_SLOT_1 + 2 * slot
}

impl Game {
    /// Gives `hero` `granted` from the start of its run (a dev switch, `ESURV_GRANT`, for a
    /// live look): rings, trinkets and amulets put on as taking their cards does, and runes laid
    /// on the ground (`speed_rune`, `berserker_rune`, `restoration_rune`, `:<yards>` ahead of the
    /// hero, where it faces; at its feet without). A name that is none of these is skipped.
    pub fn grant(&mut self, hero: u32, granted: &[String]) {
        let content = self.run.content.clone();
        for g in granted {
            if let Some((kind, ahead)) = granted_rune(g) {
                if let Some(h) = self.run.heroes.iter().find(|h| h.id == hero) {
                    let at = (
                        h.pos.0 + h.facing.cos() * ahead,
                        h.pos.1 + h.facing.sin() * ahead,
                    );
                    self.run.lay_pickup(kind, at);
                }
            } else if let Some(offer) = granted_offer(&content, g) {
                self.run.grant(hero, offer);
            }
        }
    }

    /// What `hero` holds of its jewelry: by player field, the entry there.
    pub(super) fn jewelry_wanted(&self, hero: &Hero) -> BTreeMap<u16, u32> {
        let content = &self.run.content;
        let mut out = BTreeMap::new();
        if let Some(a) = &hero.amulet {
            if let Some(entry) = amulet_entry(content, &a.amulet.id, a.rank) {
                out.insert(equipment_field(NECK_SLOT), entry);
            }
        }
        let (mut fingers, mut trinkets) = (FINGER_SLOTS.iter(), TRINKET_SLOTS.iter());
        let mut pack = 0..PACK_SLOTS;
        for w in &hero.jewelry {
            let Some(j) = content.jewelry.iter().find(|j| j.id == w.id) else {
                continue;
            };
            let rarity = self
                .jewelry_rarity
                .get(&(hero.id, w.id.clone()))
                .copied()
                .unwrap_or(0);
            let Some(entry) = ring_or_trinket_entry(content, &w.id, rarity, w.value) else {
                continue;
            };
            let slot = match j.slot {
                JewelrySlot::Ring => fingers.next(),
                JewelrySlot::Trinket => trinkets.next(),
            };
            let at = match slot {
                Some(&s) => equipment_field(s),
                None => match pack.next() {
                    Some(p) => pack_field(p),
                    None => continue,
                },
            };
            out.insert(at, entry);
        }
        out
    }

    /// Brings each player's jewelry items in line with its hero's jewelry: an item made for
    /// each piece put on, an upgraded or empowered piece's item replaced, the items of pieces gone
    /// (a fresh run) taken away. Each packet is for one hero's client. `events` tell the rarity a
    /// ring or trinket was put on at.
    pub(super) fn mirror_jewelry(&mut self, events: &[Event]) -> Vec<(u32, Packet)> {
        for e in events {
            if let Event::OfferTaken {
                hero,
                offer: Offer::NewJewelry { id, rarity, .. },
            } = e
            {
                let r = RARITIES.iter().position(|x| x == rarity).unwrap_or(0) as u8;
                self.jewelry_rarity.insert((*hero, id.clone()), r);
            }
        }
        let everyone: Vec<(u32, u64, BTreeMap<u16, u32>)> = self
            .players
            .iter()
            .filter(|p| !self.left.contains(&p.hero))
            .filter_map(|p| {
                let hero = self.run.heroes.iter().find(|h| h.id == p.hero)?;
                Some((p.hero, p.guid(), self.jewelry_wanted(hero)))
            })
            .collect();
        let mut out = Vec::new();
        for (hero, owner, wanted) in everyone {
            let held = self.jewelry_held.entry(hero).or_default();
            let (mut made, mut gone) = (Vec::new(), Vec::new());
            let mut slots = Fields::new();
            for (&at, &entry) in &wanted {
                if held.get(&at).map(|h| h.1) == Some(entry) {
                    continue;
                }
                if let Some((old, _)) = held.remove(&at) {
                    gone.push(old);
                }
                self.next_jewel = self.next_jewel.wrapping_add(1);
                let guid = HIGHGUID_ITEM
                    | JEWELRY_GUID_TAG
                    | (owner & 0xFF_FFFF) << 16
                    | u64::from(self.next_jewel);
                made.push(own_item(guid, entry, owner));
                slots.u64(at, guid);
                held.insert(at, (guid, entry));
            }
            let stale: Vec<u16> = held
                .keys()
                .filter(|at| !wanted.contains_key(at))
                .copied()
                .collect();
            for at in stale {
                if let Some((old, _)) = held.remove(&at) {
                    gone.push(old);
                    slots.u64(at, 0);
                }
            }
            if !slots.is_empty() {
                made.push(ObjectUpdate::Values {
                    guid: owner,
                    fields: slots,
                });
                out.push((hero, update_object(&made)));
            }
            out.extend(gone.into_iter().map(|g| (hero, packets::destroy_object(g))));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_granted_name_is_a_ring_or_trinket_at_its_rarity_or_an_amulet() {
        let c = esurv_content::Content::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../content"
        ))
        .unwrap();
        let base = c.jewelry.iter().find(|j| j.id == "accuria").unwrap().value;
        assert_eq!(
            granted_offer(&c, "accuria@3"),
            Some(Offer::NewJewelry {
                id: "accuria".into(),
                value: base,
                rarity: RARITIES[3],
            })
        );
        assert!(matches!(
            granted_offer(&c, &c.amulets[0].id),
            Some(Offer::Amulet { amulet }) if amulet.id == c.amulets[0].id
        ));
        let l = &c.legendary_amulets[0];
        assert!(matches!(
            granted_offer(&c, &l.id()),
            Some(Offer::Amulet { amulet }) if amulet.id == l.id()
        ));
        assert_eq!(granted_offer(&c, "no_such_thing"), None);
        assert_eq!(granted_offer(&c, "accuria@9"), None);
        assert_eq!(
            granted_rune("speed_rune:15"),
            Some((Pickup::SpeedRune, 15.0))
        );
        assert_eq!(
            granted_rune("restoration_rune"),
            Some((Pickup::RestorationRune, 0.0))
        );
        assert_eq!(granted_rune("speed_rune:far"), None);
        assert_eq!(granted_rune("accuria"), None);
    }
}
