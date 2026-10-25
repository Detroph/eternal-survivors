//! Enemies that look like player characters of the faction the heroes fight
//! (Warsong Gulch; zones.md "Player-looking enemies"). A roster enemy or boss whose content
//! says `looks_like` is shown as a player: a race of the other faction, a gender and a face and
//! hair within the client's own ranges, its class's gear from `pvp_looks.ron` (a piece per slot
//! and a weapon set, each picked from the lists), a ranked one in its faction's rank 12 rare set
//! with its rank's title before a made-up player name. Every pick follows from the enemy's id, so
//! a player who arrives later sees it the same. Its name reaches the clients before it does
//! (`SMSG_NAME_QUERY_RESPONSE`, which the client keeps), since a player is named by guid.

use esurv_content::{
    spread, ArmorSlot, CharacterGear, Faction, Hand, PlayerCharacter, PlayerForm, WeaponKind, WeaponLook, ROBE_INVENTORY_TYPE,
};
use esurv_sim::game::run::EnemyKind;

use super::{facing_of, whole, Game, ENEMY_FACTION, PLAYER_COMBAT_REACH, REST_STATE_NORMAL};
use crate::packets::{self, ItemInfo};
use crate::update::{field, Fields, Living, ObjectTypeId, ObjectUpdate, Speeds, TYPE_OBJECT, TYPE_PLAYER, TYPE_UNIT};
use crate::wire::Packet;

/// A player-looking enemy's guid: a player guid with this bit and the enemy's id.
const PLAYER_ENEMY_BIT: u64 = 0x2000_0000;

/// The guid player-looking enemy `id` is shown with.
pub fn player_enemy_guid(id: u32) -> u64 {
    PLAYER_ENEMY_BIT | u64::from(id & 0x1FFF_FFFF)
}

/// Whether `guid` is a player-looking enemy's (a player guid with the enemy bit).
pub fn is_player_enemy_guid(guid: u64) -> bool {
    guid >> 32 == 0 && guid & PLAYER_ENEMY_BIT != 0
}

/// The druid forms' looks a player-looking enemy is shown in, by its faction: the night elf's
/// and the tauren's (`CreatureDisplayInfo.dbc`), whatever its race.
pub(super) fn form_display(form: PlayerForm, faction: Faction) -> u32 {
    match (form, faction) {
        (PlayerForm::Cat, Faction::Alliance) => 892,
        (PlayerForm::Cat, Faction::Horde) => 8571,
        (PlayerForm::Bear, Faction::Alliance) => 2281,
        (PlayerForm::Bear, Faction::Horde) => 2289,
        (PlayerForm::Moonkin, Faction::Alliance) => 15374,
        (PlayerForm::Moonkin, Faction::Horde) => 15375,
    }
}

/// 1.12's power types by a class's power: mana, rage, energy.
fn power_of(class: &esurv_content::Class) -> u8 {
    match class.power {
        esurv_content::PowerType::Mana => 0,
        esurv_content::PowerType::Rage => 1,
        esurv_content::PowerType::Energy => 3,
    }
}

/// One of `list` for pick `salt` of enemy `id`.
fn pick<T>(list: &[T], id: u32, salt: u32) -> Option<&T> {
    (!list.is_empty()).then(|| &list[spread(id, salt) as usize % list.len()])
}

/// What a player-looking enemy is shown as.
pub(super) struct PlayerShape {
    pub name: String,
    /// A hand-made character's own yell, said in place of its boss's.
    pub yell: Option<String>,
    pub race: u8,
    pub class: u8,
    pub gender: u8,
    power: u8,
    /// skin, face, hair style, hair colour, facial hair.
    looks: [u8; 5],
    display: u32,
    native: u32,
    /// (equipment slot, item entry) of every piece it shows.
    pub(crate) items: Vec<(u8, u32)>,
}

impl PlayerShape {
    /// The fields that draw a player as this shape: its race, class, gender and power, its model
    /// (a form's over its race's), every piece it shows, and its face and hair.
    pub(super) fn dress(&self, f: &mut Fields) {
        f.bytes(field::UNIT_BYTES_0, [self.race, self.class, self.gender, self.power]);
        f.u32(field::UNIT_DISPLAYID, self.display).u32(field::UNIT_NATIVEDISPLAYID, self.native);
        for &(slot, entry) in &self.items {
            f.u32(field::PLAYER_VISIBLE_ITEM_1_0 + u16::from(slot) * field::PLAYER_VISIBLE_ITEM_STRIDE, entry);
        }
        let l = self.looks;
        f.bytes(field::PLAYER_BYTES, [l[0], l[1], l[2], l[3]])
            .bytes(field::PLAYER_BYTES_2, [l[4], 0, 0, REST_STATE_NORMAL])
            .u32(field::PLAYER_BYTES_3, self.gender.into());
    }
}

impl Game {
    /// The shooting look player-looking enemy `id` shows with its shot of look `spell` (a
    /// hunter boss's Auto Shot): its ranged weapon's, as a hero's
    /// (`ranged_shots`): none where the client draws it from the weapon look itself; `None` too for
    /// any other spell, or one holding no bow, gun or crossbow.
    pub(super) fn enemy_shot_look(&self, id: u32, spell: u32) -> Option<u32> {
        if !esurv_sim::game::run::AMMO_SPELLS.contains(&spell) {
            return None;
        }
        let i = self.run.enemies.iter().position(|e| e.id == id)?;
        let shape = self.player_shape(i)?;
        let (_, entry) = shape.items.iter().find(|(slot, _)| *slot == Hand::Ranged.equip_slot())?;
        let item = look_item_info(*entry)?;
        super::ranged_shots::shot_look_of(self.data.as_deref(), spell, item.class, item.subclass, item.display_id)
    }

    /// The rank the creature query gives enemy `i`'s kind (`packets::RANK_*`): an elite of the
    /// roster, a rare, a boss; `None` for a normal enemy. A player-looking enemy's clients are told
    /// it (`ToClient::EnemyRanks`), since a player guid has no creature query.
    pub(super) fn enemy_rank(&self, i: usize) -> Option<u32> {
        let zone = &self.run.content.zones[self.run.zone];
        match self.run.enemies.get(i)?.kind {
            EnemyKind::Roster(r) => zone.enemies.get(r).map(|e| e.elite || e.focus_as_elite)?.then_some(packets::RANK_ELITE),
            EnemyKind::Rare { .. } => Some(packets::RANK_RARE),
            EnemyKind::Boss(_) => Some(packets::RANK_BOSS),
        }
    }

    /// The faction the heroes fight: the other one than the party's.
    fn enemy_faction(&self) -> Faction {
        match self.run.party_faction {
            Faction::Alliance => Faction::Horde,
            Faction::Horde => Faction::Alliance,
        }
    }

    /// What enemy `i` is shown as, when it looks like a player: a boss played by a hand-made
    /// character as that character, else as `looks_like` says.
    pub(super) fn player_shape(&self, i: usize) -> Option<PlayerShape> {
        let content = &self.run.content;
        let zone = &content.zones[self.run.zone];
        let e = &self.run.enemies[i];
        let (look, rank, name, played) = match e.kind {
            EnemyKind::Roster(r) | EnemyKind::Rare { roster: r, .. } => {
                let x = &zone.enemies[r];
                (x.looks_like.as_ref(), x.rank.as_ref(), &x.name, None)
            }
            EnemyKind::Boss(b) => {
                let x = &zone.bosses[b];
                (x.looks_like.as_ref(), x.rank.as_ref(), &x.name, x.played_by.as_ref().map(|p| (p, b)))
            }
        };
        let faction = self.enemy_faction();
        let id = e.id;
        if let Some(c) = played.and_then(|(_, b)| self.character_for(b, faction)) {
            return self.character_shape(c, rank, id);
        }
        let look = look?;
        let class = content.class(&look.class)?;
        // a race of that faction our game lets be the class (the stock pairs and ours)
        let races: Vec<_> = content
            .races
            .iter()
            .filter(|r| r.faction == faction && self.data.as_ref().and_then(|d| d.allows(r.wow_race_id, class.wow_class_id as u8)).unwrap_or(true))
            .collect();
        let race = *pick(&races, id, 1)?;
        let gender = (spread(id, 2) & 1) as u8;
        let looks = self.random_looks(race.wow_race_id, gender, id);
        let items = random_items(content.pvp_looks.of(&look.class)?, rank.is_some(), faction, id);
        let name = match rank {
            Some(titles) => {
                let player = pick(&content.pvp_looks.player_names, id, 40).cloned().unwrap_or_default();
                format!("{} {player}", titles.of(faction))
            }
            // one with no name of its own (a boss's stand-in) takes a made-up player name
            None if name.is_empty() => pick(&content.pvp_looks.player_names, id, 40).cloned().unwrap_or_default(),
            // a joke picked for each ("Noob", "Clicker" ... before its class)
            None => {
                let joke = pick(&content.pvp_looks.jokes, id, 50).map_or("", String::as_str);
                name.replace(esurv_content::JOKE, joke)
            }
        };
        let native = self.data.as_ref().and_then(|d| d.player_display(race.wow_race_id, gender)).unwrap_or(0);
        let display = look.form.map_or(native, |f| form_display(f, faction));
        Some(PlayerShape {
            name,
            yell: None,
            race: race.wow_race_id,
            class: class.wow_class_id as u8,
            gender,
            power: power_of(class),
            looks,
            display,
            native,
            items,
        })
    }

    /// A face and hair for enemy `id` of race `race` and gender `gender`, each dial picked within
    /// the client's ranges (all 0 without its data).
    fn random_looks(&self, race: u8, gender: u8, id: u32) -> [u8; 5] {
        let dial = |count: u8, salt: u32| if count == 0 { 0 } else { (spread(id, salt) % u32::from(count)) as u8 };
        self.data.as_ref().and_then(|d| d.look_ranges(race, gender)).map_or([0; 5], |r| {
            [dial(r.skin, 3), dial(r.face, 4), dial(r.hair_style, 5), dial(r.hair_color, 6), dial(r.facial_hair, 7)]
        })
    }

    /// The hand-made character playing boss number `b`: the one named,
    /// or one of `faction`'s dealt at random, the pool shuffled once for the run (by its seed) and
    /// dealt to its bosses in turn, so none plays two while there are enough; `None` while the
    /// pool has none of `faction`, and the boss shows its stand-in.
    fn character_for(&self, b: usize, faction: Faction) -> Option<&PlayerCharacter> {
        // the content's pick, the one the run made its boss by (a caster or not)
        let content = &self.run.content;
        content.boss_character(&content.zones[self.run.zone], b, faction, self.run.seed)
    }

    /// Hand-made character `c` as a boss shown with id `id`: its race, looks, its own name (a boss
    /// carries no title) and gear, its class's PvP looks (with the rank
    /// set when ranked) under any piece written for it.
    pub(super) fn character_shape(&self, c: &PlayerCharacter, rank: Option<&esurv_content::RankTitles>, id: u32) -> Option<PlayerShape> {
        let content = &self.run.content;
        let race = content.races.iter().find(|r| r.id == c.race)?;
        let class = content.class(&c.class)?;
        let mut items = random_items(content.pvp_looks.of(&c.class)?, rank.is_some(), race.faction, id);
        if let CharacterGear::Picked(gear) = &c.gear {
            let mut wear = |slot: u8, entry: u32| {
                items.retain(|(s, _)| *s != slot);
                items.push((slot, entry));
            };
            for a in &gear.armor {
                wear(a.slot.equip_slot(), armor_entry(a.slot, a.display, a.robe));
            }
            for (hand, w) in [(Hand::MainHand, gear.main_hand), (Hand::OffHand, gear.off_hand), (Hand::Ranged, gear.ranged)] {
                if let Some(w) = w {
                    wear(hand.equip_slot(), weapon_entry(w));
                }
            }
            items.retain(|(s, _)| !gear.bare.iter().any(|b| b.equip_slot() == *s));
            // a two-hander in the main hand leaves the off hand empty (no class shield under it)
            if gear.main_hand.is_some_and(|w| w.kind.two_handed()) && gear.off_hand.is_none() {
                items.retain(|(s, _)| *s != Hand::OffHand.equip_slot());
            }
        }
        let name = c.name.clone();
        let gender = c.gender.id();
        let looks = match c.looks {
            Some(l) => [l.skin, l.face, l.hair_style, l.hair_color, l.facial_hair],
            None => self.random_looks(race.wow_race_id, gender, id),
        };
        let native = self.data.as_ref().and_then(|d| d.player_display(race.wow_race_id, gender)).unwrap_or(0);
        Some(PlayerShape {
            name,
            yell: (!c.yell.is_empty()).then(|| c.yell.clone()),
            race: race.wow_race_id,
            class: class.wow_class_id as u8,
            gender,
            power: power_of(class),
            looks,
            display: c.form.map_or(native, |f| form_display(f, race.faction)),
            native,
            items,
        })
    }

    /// The name of player-looking enemy `i` for every client, before it shows.
    pub(super) fn player_enemy_name(&self, i: usize, shape: &PlayerShape) -> Packet {
        let guid = player_enemy_guid(self.run.enemies[i].id);
        packets::name_query_response(guid, &shape.name, shape.race, shape.gender, shape.class)
    }

    /// The names of every player-looking enemy, for a player who arrives.
    pub(super) fn player_enemy_names_now(&self) -> Vec<Packet> {
        (0..self.run.enemies.len()).filter_map(|i| Some(self.player_enemy_name(i, &self.player_shape(i)?))).collect()
    }

    /// The create of player-looking enemy `i`, shown as `shape`, and its guid.
    pub(super) fn player_enemy_object(&self, i: usize, now: f32, shape: &PlayerShape, scale: f32) -> (ObjectUpdate, u64) {
        let e = &self.run.enemies[i];
        let guid = player_enemy_guid(e.id);
        let at = e.position(now);
        let mut f = Fields::new();
        f.u64(field::OBJECT_GUID, guid)
            .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_UNIT | TYPE_PLAYER)
            .f32(field::OBJECT_SCALE_X, scale);
        f.u32(field::UNIT_HEALTH, whole(e.health)).u32(field::UNIT_MAXHEALTH, whole(e.max_health));
        f.u32(field::UNIT_LEVEL, self.enemy_level()).u32(field::UNIT_FACTIONTEMPLATE, ENEMY_FACTION);
        f.f32(field::UNIT_BOUNDINGRADIUS, e.collision_radius).f32(field::UNIT_COMBATREACH, PLAYER_COMBAT_REACH);
        f.u32(field::UNIT_BASEATTACKTIME, (e.attack_interval * 1000.0) as u32).f32(field::UNIT_MOD_CAST_SPEED, 1.0);
        if let Some(carried) = self.carried_aura(i) {
            f.extend(&carried);
        }
        shape.dress(&mut f);
        let living = Living {
            position: [at.0, at.1, self.ground_z(at)],
            facing: facing_of(&e.segment),
            speeds: Speeds::running_at(e.base_speed),
        };
        (ObjectUpdate::Create { guid, type_id: ObjectTypeId::Player, living, fields: f, own: false }, guid)
    }
}

/// A class's PvP looks picked for enemy `id`: a piece per armor slot and a weapon set, and when
/// `ranked` its `faction`'s rank set over them; as (equipment slot, item entry).
fn random_items(gear: &esurv_content::ClassLooks, ranked: bool, faction: Faction, id: u32) -> Vec<(u8, u32)> {
    let mut items: Vec<(u8, u32)> = Vec::new();
    for (k, slot) in gear.armor.iter().enumerate() {
        if let Some(&d) = pick(&slot.displays, id, 10 + k as u32) {
            items.push((slot.slot.equip_slot(), armor_entry(slot.slot, d, slot.robes.contains(&d))));
        }
    }
    // a ranked one wears its faction's rank set over the rest
    if ranked {
        for slot in gear.ranked(faction) {
            if let Some(&d) = slot.displays.first() {
                let piece = (slot.slot.equip_slot(), armor_entry(slot.slot, d, slot.robes.contains(&d)));
                items.retain(|(s, _)| *s != piece.0);
                items.push(piece);
            }
        }
    }
    if let Some(set) = pick(&gear.weapons, id, 30) {
        for (hand, weapon) in [(Hand::MainHand, set.main_hand), (Hand::OffHand, set.off_hand), (Hand::Ranged, set.ranged)] {
            if let Some(w) = weapon {
                items.push((hand.equip_slot(), weapon_entry(w)));
            }
        }
    }
    items
}

/// Where the looks' item entries start (our numbering, wire-only): a piece is
/// `LOOK_ITEMS_FROM + kind × KIND_SPAN + inventory type × INVENTORY_SPAN + display`, kind 0 for
/// armour and 1 + the weapon kind's place in [`WEAPON_KINDS`] for a weapon, so its entry holds all
/// the item query answers.
const LOOK_ITEMS_FROM: u32 = 1_600_000_000;
const KIND_SPAN: u32 = 3_000_000;
const INVENTORY_SPAN: u32 = 100_000;
/// The weapon kinds in their place for the entries.
const WEAPON_KINDS: [WeaponKind; 17] = [
    WeaponKind::OneHandedAxe,
    WeaponKind::TwoHandedAxe,
    WeaponKind::Bow,
    WeaponKind::Gun,
    WeaponKind::OneHandedMace,
    WeaponKind::TwoHandedMace,
    WeaponKind::Polearm,
    WeaponKind::OneHandedSword,
    WeaponKind::TwoHandedSword,
    WeaponKind::Staff,
    WeaponKind::FistWeapon,
    WeaponKind::Dagger,
    WeaponKind::Thrown,
    WeaponKind::Crossbow,
    WeaponKind::Wand,
    WeaponKind::Shield,
    // a weapon loadout's off-hand frill (last, so the others keep their numbers)
    WeaponKind::HeldInOffHand,
];
/// 1.12's armour item class, and the quality the looks show as (rare).
const ITEM_CLASS_ARMOR: u32 = 4;
const LOOK_QUALITY: u32 = 3;

/// The entry of an armour piece of look `display` in `slot` (a chest drawn as a robe when `robe`).
fn armor_entry(slot: ArmorSlot, display: u32, robe: bool) -> u32 {
    let inventory = if robe { ROBE_INVENTORY_TYPE } else { slot.inventory_type() };
    LOOK_ITEMS_FROM + inventory * INVENTORY_SPAN + display % INVENTORY_SPAN
}

/// The entry of weapon `w` (a player enemy's, a weapon loadout's).
pub fn weapon_entry(w: WeaponLook) -> u32 {
    let kind = WEAPON_KINDS.iter().position(|k| *k == w.kind).unwrap_or(0) as u32 + 1;
    LOOK_ITEMS_FROM + kind * KIND_SPAN + w.kind.inventory_type() * INVENTORY_SPAN + w.display % INVENTORY_SPAN
}

/// What the item query answers for a look's entry: its class, kind, look and slot; `None` for
/// any other entry.
pub fn look_item_info(entry: u32) -> Option<ItemInfo> {
    let n = entry.checked_sub(LOOK_ITEMS_FROM)?;
    let (kind, rest) = (n / KIND_SPAN, n % KIND_SPAN);
    let (inventory_type, display_id) = (rest / INVENTORY_SPAN, rest % INVENTORY_SPAN);
    if inventory_type == 0 || display_id == 0 {
        return None;
    }
    let (class, subclass, sheath) = match kind {
        0 => (ITEM_CLASS_ARMOR, 0, 0),
        k => {
            let w = *WEAPON_KINDS.get(k as usize - 1)?;
            (w.item_class(), w.subclass(), w.sheath())
        }
    };
    Some(ItemInfo {
        class,
        subclass,
        name: String::new(),
        display_id,
        quality: LOOK_QUALITY,
        inventory_type,
        sheath,
        description: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A look's entry answers the item query with what it was made of.
    #[test]
    fn a_look_s_entry_reads_back_its_piece() {
        let helm = look_item_info(armor_entry(ArmorSlot::Head, 29965, false)).unwrap();
        assert_eq!((helm.class, helm.display_id, helm.inventory_type), (ITEM_CLASS_ARMOR, 29965, 1));
        let robe = look_item_info(armor_entry(ArmorSlot::Chest, 31102, true)).unwrap();
        assert_eq!(robe.inventory_type, ROBE_INVENTORY_TYPE);
        let axe = look_item_info(weapon_entry(WeaponLook { kind: WeaponKind::TwoHandedAxe, display: 23904 })).unwrap();
        let w = WeaponKind::TwoHandedAxe;
        assert_eq!((axe.class, axe.subclass, axe.display_id, axe.inventory_type, axe.sheath), (w.item_class(), w.subclass(), 23904, 17, w.sheath()));
        let shield = look_item_info(weapon_entry(WeaponLook { kind: WeaponKind::Shield, display: 18490 })).unwrap();
        assert_eq!(shield.inventory_type, 14);
        // an item held in the off hand, as 1.12 has Sapphiron's Left Eye: armour, miscellaneous, type 23, never sheathed
        let eye = look_item_info(weapon_entry(WeaponLook { kind: WeaponKind::HeldInOffHand, display: 35792 })).unwrap();
        assert_eq!((eye.class, eye.subclass, eye.display_id, eye.inventory_type, eye.sheath), (4, 0, 35792, 23, 0));
        assert!(look_item_info(5).is_none());
        // every pick spreads
        let races: Vec<u32> = (0..64).map(|id| spread(id, 1) % 4).collect();
        assert!((0..4).all(|r| races.contains(&r)));
    }

    /// A roster enemy that looks like a player shows as a player of the
    /// faction the party fights: a race of it, its class's gear with the rank set over it, its
    /// rank's title before a player name, a player's guid, its name sent ahead.
    #[test]
    fn a_player_looking_enemy_shows_as_a_player_of_the_other_faction() {
        use std::sync::Arc;
        use esurv_content::testing::{content_folder_with_faction_zone, ExtraZone};
        use esurv_content::Content;
        use esurv_sim::game::run::{Flat, Run};

        let shipped = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let zone = ExtraZone { id: "test_factions", name: "Test Factions", difficulty: 11.0 };
        let dir = content_folder_with_faction_zone(shipped.as_ref(), "server-player-enemies", zone).unwrap();
        let loaded = Content::load(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        let content = Arc::new(loaded.unwrap());
        let saves = Arc::new(crate::save::Saves::in_memory());
        let ana = saves.create("Ana", "warrior", "human", 1, [0; 5]).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(content.clone(), None, 0, &ana, saves.clone(), inputs).unwrap();
        let setup = super::super::hero_setup(&content, &ana, &saves.class_progress(&content, "warrior"));
        game.run = Run::new(content.clone(), "test_factions", Default::default(), &[setup], Box::new(Flat), 3).unwrap();
        let h = game.run.heroes[0].id;
        let p = game.run.heroes[0].pos;
        game.run.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        for _ in 0..200 {
            if !game.run.enemies.is_empty() {
                break;
            }
            game.run.tick(0.05);
        }
        assert!(!game.run.enemies.is_empty(), "an enemy came");
        let shape = game.player_shape(0).expect("it looks like a player");
        // the party is Alliance: a Horde race, the rank's Horde title
        let horde: Vec<u8> = content.races.iter().filter(|r| r.faction == Faction::Horde).map(|r| r.wow_race_id).collect();
        assert!(horde.contains(&shape.race), "{}", shape.race);
        assert!(shape.name.starts_with("HordeRank 3 "), "{}", shape.name);
        assert_eq!(shape.class, content.class("warrior").unwrap().wow_class_id as u8);
        // the Horde's rank set on its head, a weapon in its main hand
        let gear = content.pvp_looks.of("warrior").unwrap();
        let head = gear.ranked(Faction::Horde).iter().find(|s| s.slot == ArmorSlot::Head).unwrap().displays[0];
        let worn = |slot: u8| shape.items.iter().find(|(s, _)| *s == slot).and_then(|(_, e)| look_item_info(*e));
        assert_eq!(worn(ArmorSlot::Head.equip_slot()).map(|i| i.display_id), Some(head));
        assert!(worn(Hand::MainHand.equip_slot()).is_some_and(|i| i.class == 2));
        let (create, guid) = game.enemy_object(0, game.run.now);
        assert_eq!(guid, player_enemy_guid(game.run.enemies[0].id));
        assert!(matches!(create, ObjectUpdate::Create { type_id: ObjectTypeId::Player, .. }));
        assert_eq!(game.enemy_name(0), shape.name);
        assert_eq!(game.player_enemy_names_now().len(), game.run.enemies.len());
    }

    /// Warsong Gulch's normal enemies: a joke picked for each before
    /// its class, never an epic piece (the item query answers rare at most), and over many the
    /// jokes and the pieces vary.
    #[test]
    fn warsong_s_players_vary_their_jokes_and_gear() {
        warsong_s_players_vary(None);
    }

    /// With the player's data, every Warsong player is a race and class pair our game allows (the
    /// stock pairs and ours: an undead paladin, an orc mage ...), and has a face of its race.
    #[test]
    fn warsong_s_players_are_pairs_our_game_allows() {
        let Some(dir) = benilla_formats::wow_data() else { return };
        let content = esurv_content::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let data = crate::ground::GameData::open(&dir, &content).unwrap();
        warsong_s_players_vary(Some(data));
    }

    /// Warsong Gulch's normal enemies over a minute of a run: a joke
    /// picked for each before its class, the jokes and the pieces varied, every race of the other
    /// faction, and with the player's data each a race and class pair our game allows.
    fn warsong_s_players_vary(data: Option<std::sync::Arc<crate::ground::GameData>>) {
        use std::sync::Arc;
        use esurv_content::Content;
        use esurv_sim::game::run::{Flat, Run};

        let content = Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
        let saves = Arc::new(crate::save::Saves::in_memory());
        let ana = saves.create("Ana", "warrior", "orc", 0, [0; 5]).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(content.clone(), data.as_ref(), 0, &ana, saves.clone(), inputs).unwrap();
        let setup = super::super::hero_setup(&content, &ana, &saves.class_progress(&content, "warrior"));
        game.run = Run::new(content.clone(), "wsg", Default::default(), &[setup], Box::new(Flat), 5).unwrap();
        let h = game.run.heroes[0].id;
        let p = game.run.heroes[0].pos;
        game.run.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        let (mut jokes, mut heads) = (std::collections::HashSet::new(), std::collections::HashSet::new());
        for _ in 0..1200 {
            game.run.tick(0.05);
            for i in 0..game.run.enemies.len() {
                let Some(shape) = game.player_shape(i) else { continue };
                assert!(!shape.name.contains(esurv_content::JOKE), "{}", shape.name);
                // the party is Horde: Alliance races
                assert!([1, 3, 4, 7].contains(&shape.race), "{}", shape.race);
                if let Some(d) = &data {
                    assert_eq!(d.allows(shape.race, shape.class), Some(true), "race {} class {}", shape.race, shape.class);
                }
                jokes.insert(shape.name.rsplit_once(' ').map_or(String::new(), |(j, _)| j.to_owned()));
                for &(slot, entry) in &shape.items {
                    let info = look_item_info(entry).unwrap();
                    assert!(info.quality <= 3);
                    if slot == ArmorSlot::Head.equip_slot() {
                        heads.insert(info.display_id);
                    }
                }
            }
        }
        assert!(jokes.len() >= 5, "{jokes:?}");
        assert!(heads.len() >= 5, "{heads:?}");
    }

    /// Warsong's bosses carry the heroes' own flag from their arrival:
    /// against an Alliance party the Silverwing Flag in their first buff slot; normal enemies none.
    #[test]
    fn warsong_s_bosses_carry_the_heroes_flag() {
        use std::sync::Arc;
        use esurv_content::Content;
        use esurv_sim::game::run::{Flat, Run};

        let content = Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
        let saves = Arc::new(crate::save::Saves::in_memory());
        let ana = saves.create("Ana", "warrior", "human", 1, [0; 5]).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(content.clone(), None, 0, &ana, saves.clone(), inputs).unwrap();
        let setup = super::super::hero_setup(&content, &ana, &saves.class_progress(&content, "warrior"));
        game.run = Run::new(content.clone(), "wsg", Default::default(), &[setup], Box::new(Flat), 4).unwrap();
        let h = game.run.heroes[0].id;
        let p = game.run.heroes[0].pos;
        game.run.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        let boss = |g: &Game| g.run.enemies.iter().position(|e| matches!(e.kind, EnemyKind::Boss(_)));
        let mut ticks = 0;
        while boss(&game).is_none() && ticks < 4000 {
            game.run.tick(0.05);
            // level-up and cache offers pause the run: take the first
            game.run.choose(h, 0);
            game.run.heroes[0].health = game.run.heroes[0].stats.max_health;
            ticks += 1;
        }
        let b = boss(&game).expect("the first boss came");
        let carried = game.carried_aura(b).expect("it carries the flag");
        let flag = |f: &Fields| format!("{f:?}").contains(&format!("{}: 23335", field::UNIT_AURA));
        assert!(flag(&carried), "{carried:?}");
        let (create, _) = game.enemy_object(b, game.run.now);
        let ObjectUpdate::Create { fields, .. } = create else { panic!("a create") };
        assert!(flag(&fields), "the create carries it");
        let normal = game.run.enemies.iter().position(|e| !matches!(e.kind, EnemyKind::Boss(_))).unwrap();
        assert!(game.carried_aura(normal).is_none());
    }

    /// Bosses played by hand-made characters: each boss of a run a
    /// different character of the faction the party fights, named after its rank's title, in
    /// its race and its hand-picked pieces over its class's PvP looks.
    #[test]
    fn bosses_take_hand_made_characters_of_the_other_faction() {
        use std::sync::Arc;
        use esurv_content::Content;
        use esurv_sim::game::run::{Flat, Run};

        let shipped = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let dir = esurv_content::testing::content_folder_with_zones(shipped.as_ref(), "server-characters", &[]).unwrap();
        let character = |id: &str, race: &str, class: &str, gear: &str| {
            format!("(id: \"{id}\", name: \"{id}\", race: \"{race}\", class: \"{class}\", gender: Female, looks: Some((skin: 1, face: 2, hair_style: 3, hair_color: 4, facial_hair: 0)), gear: {gear}, yell: \"Zap!\", form: None),")
        };
        let list = [
            character("Grubgrub", "orc", "warrior", "Random"),
            character("Zapzap", "troll", "mage", "Picked((armor: [(slot: Head, display: 31286)], main_hand: Some((kind: Staff, display: 20257)), bare: [Shoulders]))"),
            character("Lightbringer", "human", "paladin", "Random"),
        ];
        std::fs::write(dir.join("player_characters.ron"), format!("([{}])", list.join("
"))).unwrap();
        let loaded = Content::load(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        let content = Arc::new(loaded.unwrap());
        let saves = Arc::new(crate::save::Saves::in_memory());
        let ana = saves.create("Ana", "warrior", "human", 1, [0; 5]).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(content.clone(), None, 0, &ana, saves.clone(), inputs).unwrap();
        let setup = super::super::hero_setup(&content, &ana, &saves.class_progress(&content, "warrior"));
        game.run = Run::new(content.clone(), "wsg", Default::default(), &[setup], Box::new(Flat), 9).unwrap();
        // the party is Alliance: the Horde's two dealt in turn, a different one for each of two
        // bosses, the third boss the first again; no Alliance character fights an Alliance party
        let picks: Vec<&str> = (0..3).map(|b| game.character_for(b, Faction::Horde).unwrap().id.as_str()).collect();
        assert_ne!(picks[0], picks[1]);
        assert_eq!(picks[0], picks[2]);
        assert!(picks.iter().all(|p| ["Grubgrub", "Zapzap"].contains(p)), "{picks:?}");
        let zap = content.player_character("Zapzap").unwrap();
        // a ranked boss would wear its rank set, its hand-picked pieces over it; still no title
        let rank = esurv_content::RankTitles { rank: 14, alliance: "Grand Marshal".into(), horde: "High Warlord".into() };
        let shape = game.character_shape(zap, Some(&rank), 77).unwrap();
        assert_eq!(shape.name, "Zapzap", "a boss carries no title");
        let troll = content.races.iter().find(|r| r.id == "troll").unwrap().wow_race_id;
        assert_eq!((shape.race, shape.gender, shape.looks), (troll, 1, [1, 2, 3, 4, 0]));
        let worn = |slot: u8| shape.items.iter().find(|(s, _)| *s == slot).and_then(|(_, e)| look_item_info(*e)).map(|i| i.display_id);
        assert_eq!(worn(ArmorSlot::Head.equip_slot()), Some(31286), "the hand-picked helm over the rank set's");
        assert_eq!(worn(Hand::MainHand.equip_slot()), Some(20257));
        assert!(worn(ArmorSlot::Legs.equip_slot()).is_some(), "a slot left out takes its class's looks");
        assert_eq!(worn(ArmorSlot::Shoulders.equip_slot()), None, "a bare slot shows nothing");
        assert_eq!(shape.yell.as_deref(), Some("Zap!"));
    }
}

#[cfg(test)]
mod real_data {
    use std::collections::HashSet;

    use benilla_formats::{load_item_display_catalog, load_spell_catalog, open_chain, SpellCatalog};
    use esurv_content::{BossAbilityKind, CharacterGear, CharacterPick, Content, Faction};

    /// `Spell.dbc`'s auras that stun, fear, root, slow, charm or confuse (5 confuse, 6 charm,
    /// 7 fear, 12 stun, 26 root, 33 movement slowed).
    const IMPAIRING_AURAS: [u32; 6] = [5, 6, 7, 12, 26, 33];
    /// `SpellMechanic.dbc`'s that do the same (1 charm, 2 disorient, 5 fear, 7 root, 10 sleep,
    /// 11 snare, 12 stun, 13 freeze, 14 knock out, 17 polymorph).
    const IMPAIRING_MECHANICS: [u32; 10] = [1, 2, 5, 7, 10, 11, 12, 13, 14, 17];
    /// Class spells whose `Spell.dbc` row names no class family (a talent's): the class they are.
    const FAMILYLESS_CLASS_SPELLS: [(u32, &str); 1] = [(17364, "shaman")];

    fn content(dir: &std::path::Path) -> Content {
        let names = esurv_content::ClientNames::read(dir).unwrap();
        Content::load_with_names(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content"), &names).unwrap()
    }

    /// The `Spell.dbc` family (`SpellFamilyName`) of 1.12 class id `class`.
    fn family(class: u32) -> u32 {
        match class {
            1 => 4,
            2 => 10,
            3 => 9,
            4 => 8,
            5 => 6,
            7 => 11,
            8 => 3,
            9 => 5,
            11 => 7,
            _ => panic!("class {class}"),
        }
    }

    /// Whether spell `id` is one of class `class`'s own (its family, or a familyless talent of it).
    fn fits(spells: &SpellCatalog, content: &Content, class: &str, id: u32) -> bool {
        let Some(s) = spells.get(id) else { return false };
        s.spell_family == family(content.class(class).unwrap().wow_class_id) || FAMILYLESS_CLASS_SPELLS.contains(&(id, class))
    }

    /// Whether spell `id` stuns, fears, roots, slows, charms or confuses, by its auras or mechanics.
    fn impairs(spells: &SpellCatalog, id: u32) -> bool {
        let s = spells.get(id).unwrap();
        s.effect_apply_aura.iter().any(|a| IMPAIRING_AURAS.contains(a))
            || IMPAIRING_MECHANICS.contains(&s.mechanic)
            || s.effect_mechanic.iter().any(|m| IMPAIRING_MECHANICS.contains(m))
    }

    /// Every Warsong boss, each hand-made character of either faction
    /// playing it and its stand-in, fights as its class: its boss abilities and its ranged spell
    /// are its class's own spells (its spec's where it has one), none that stuns, fears, roots or
    /// slows; and every player-looking roster enemy casts its own class's spell.
    #[test]
    fn warsong_bosses_and_players_fight_with_their_class_s_spells() {
        let dir = benilla_formats::wow_data_or_skip!();
        let content = content(&dir);
        let spells = load_spell_catalog(&mut open_chain(&dir).unwrap()).unwrap();
        let wsg = content.zone("wsg").unwrap();
        let mut checked = 0;
        for faction in [Faction::Alliance, Faction::Horde] {
            for (b, boss) in wsg.bosses.iter().enumerate() {
                assert!(boss.class_abilities > 0, "Warsong boss {b} fights as its class");
                let mut picks: Vec<Option<String>> = content.player_characters_of(faction).map(|c| Some(c.id.clone())).collect();
                picks.push(None);
                for pick in picks {
                    let mut zone = wsg.clone();
                    zone.bosses[b].played_by = pick.clone().map(CharacterPick::Named);
                    let class = content.boss_class(&zone, b, faction, 0).unwrap().to_string();
                    let who = format!("boss {b} played by {pick:?} ({class})");
                    let abilities = content.boss_abilities(&zone, b, faction, 0);
                    assert_eq!(abilities.len(), boss.class_abilities as usize + boss.abilities.len(), "{who}");
                    for a in &abilities[..boss.class_abilities as usize] {
                        let looks = match a.kind {
                            BossAbilityKind::GroundBurst { telegraph_spell, impact_spell, .. } => vec![telegraph_spell, impact_spell],
                            BossAbilityKind::Volley { look_spell, .. }
                            | BossAbilityKind::Stomp { look_spell, .. }
                            | BossAbilityKind::Charge { look_spell, .. } => vec![look_spell],
                            BossAbilityKind::Summon { .. } => panic!("{who}: a class ability is a summon"),
                            ref k => vec![esurv_sim::game::telegraphs::kind_look(k)],
                        };
                        for id in looks {
                            assert!(fits(&spells, &content, &class, id), "{who}: spell {id} is not its class's");
                            assert!(!impairs(&spells, id), "{who}: spell {id} impairs");
                        }
                        checked += 1;
                    }
                    if let Some(id) = content.boss_spell(&zone, b, faction, 0) {
                        assert!(fits(&spells, &content, &class, id), "{who}: its ranged spell {id} is not its class's");
                        assert!(!impairs(&spells, id), "{who}: its ranged spell {id} impairs");
                    }
                }
            }
        }
        assert!(checked > 300, "{checked}");
        // the hand-made two: the tank warrior charges and strikes, the paladin consecrates; both swing
        let mut zone = wsg.clone();
        zone.bosses[1].played_by = Some(CharacterPick::Named("quintys".into()));
        let kinds: Vec<_> = content.boss_abilities(&zone, 1, Faction::Alliance, 0).into_iter().map(|a| a.kind).collect();
        assert!(matches!(kinds[0], BossAbilityKind::Charge { look_spell: 100, .. }), "{kinds:?}");
        assert_eq!(content.boss_spell(&zone, 1, Faction::Alliance, 0), None);
        zone.bosses[3].played_by = Some(CharacterPick::Named("vodu".into()));
        let kinds: Vec<_> = content.boss_abilities(&zone, 3, Faction::Alliance, 0).into_iter().map(|a| a.kind).collect();
        assert!(matches!(kinds[0], BossAbilityKind::Stomp { look_spell: 26573, .. }), "{kinds:?}");
        assert_eq!(content.boss_spell(&zone, 3, Faction::Alliance, 0), None);
        // the roster's casters cast their own class's spell (its elites swing)
        for e in &wsg.enemies {
            let (Some(look), Some(r)) = (&e.looks_like, &e.ranged) else { continue };
            assert!(fits(&spells, &content, &look.class, r.look_spell), "{} casts {}", look.class, r.look_spell);
        }
    }

    /// Every look the hand-made characters and the player-looking enemies wear is one the 1.12
    /// client draws (`ItemDisplayInfo.dbc`), the hand-made sets at their 1.12 looks.
    #[test]
    fn the_player_looks_are_the_client_s() {
        let dir = benilla_formats::wow_data_or_skip!();
        let content = content(&dir);
        let catalog = load_item_display_catalog(&mut open_chain(&dir).unwrap()).unwrap();
        let mut looks: Vec<(String, u32)> = Vec::new();
        for c in &content.player_characters {
            if let CharacterGear::Picked(g) = &c.gear {
                looks.extend(g.armor.iter().map(|a| (c.id.clone(), a.display)));
                looks.extend([g.main_hand, g.off_hand, g.ranged].into_iter().flatten().map(|w| (c.id.clone(), w.display)));
            }
        }
        for c in &content.pvp_looks.classes {
            for a in c.armor.iter().chain(&c.ranked_alliance).chain(&c.ranked_horde) {
                looks.extend(a.displays.iter().chain(&a.robes).map(|&d| (c.class.clone(), d)));
            }
            for w in &c.weapons {
                looks.extend([w.main_hand, w.off_hand, w.ranged].into_iter().flatten().map(|w| (c.class.clone(), w.display)));
            }
        }
        let missing: Vec<_> = looks.iter().filter(|(_, d)| catalog.get(*d).is_none()).collect();
        assert!(missing.is_empty(), "{missing:?}");
        assert!(looks.len() > 1000, "{}", looks.len());
        // the rank 14 weapons at their 1.12 looks (items 18874, 16345, 18826), not their first ones
        let picked = |id: &str| match &content.player_character(id).unwrap().gear {
            CharacterGear::Picked(g) => g.clone(),
            CharacterGear::Random => panic!("{id}"),
        };
        assert_eq!(picked("xaryu").main_hand.unwrap().display, 31765);
        assert_eq!(picked("mir").main_hand.unwrap().display, 31997);
        assert_eq!(picked("cdew").off_hand.unwrap().display, 31746);
        let first_looks: HashSet<u32> = [31332, 27089, 23825, 31303, 31310, 31312, 31319, 31321, 31330, 31334].into();
        assert!(!looks.iter().any(|(_, d)| first_looks.contains(d)), "a first look is back");
    }

    /// Every weapon a creature enemy or boss holds (vmangos's, docs/game/enemy-weapons.md) is a
    /// look the 1.12 client draws (`ItemDisplayInfo.dbc`).
    #[test]
    fn the_creature_weapons_are_the_client_s() {
        let dir = benilla_formats::wow_data_or_skip!();
        let content = content(&dir);
        let catalog = load_item_display_catalog(&mut open_chain(&dir).unwrap()).unwrap();
        let mut looks: Vec<(String, u32)> = Vec::new();
        for z in &content.zones {
            let held = z.enemies.iter().map(|e| (&e.name, e.weapons)).chain(z.bosses.iter().map(|b| (&b.name, b.weapons)));
            for (name, weapons) in held {
                looks.extend(weapons.slots().into_iter().flatten().map(|w| (name.clone(), w.display)));
            }
        }
        let missing: Vec<_> = looks.iter().filter(|(_, d)| catalog.get(*d).is_none()).collect();
        assert!(missing.is_empty(), "{missing:?}");
        assert!(looks.len() >= 38, "{}", looks.len());
    }
}
