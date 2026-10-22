//! The hero on the hero sheet: shown in benilla's glue booth ([`benilla_app::SelectModel`]) with
//! the sheet's race, sex and look and the gear its class wears (each piece in the look chosen for
//! its slot; the race and class's starting outfit in the slots the class wears nothing in), turned
//! by dragging it or by its arrows. The picture is drawn on a lit floor of our own
//! ([`super::sheet`]).

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

use benilla_app::{FigureHero, SelectModel, SelectModelView};
use benilla_protocol::{CharEnumItem, Character};
use esurv_content::Content;
use esurv_protocol::Progress;
use esurv_sim::game::gear::{
    armor_display, armor_inventory_type, decode_item_entry, loadout_armaments, shown_look,
    weapon_look, ARMOR_SLOTS,
};

use super::facts::PreRunFacts;
use super::{Draft, Menus, Screen};
use crate::ui::GameContent;

/// The model's facing, radians: kept apart from the screen so turning it draws nothing anew.
#[derive(Resource, Default)]
pub struct ModelYaw(pub f32);

/// The part of the sheet a drag turns the hero in.
#[derive(Component)]
pub(super) struct TurnArea;

/// Radians a pixel of drag turns the hero (the stock screens' 0.6 degrees a pixel).
const TURN_PER_PIXEL: f32 = 0.6 * std::f32::consts::PI / 180.0;
/// Radians an arrow turns the hero.
pub(super) const TURN_STEP: f32 = 0.5;

//// The 19 equipment slots the class wears in `progress`, each piece in its look: the look chosen
/// for its slot, else its own (as the character list dresses a hero); the weapons its weapon
/// loadout's, in the armaments of the class gear weapon worn (roadmap
/// R3, as the server arms a hero).
pub(super) fn worn_equipment(
    content: &Content,
    class: &str,
    progress: &Progress,
) -> [CharEnumItem; 19] {
    let mut out = [CharEnumItem::default(); 19];
    let (Some(k), Some(set)) = (content.class(class), content.gear_set(class)) else {
        return out;
    };
    let gear = &content.gear;
    let chosen = |slot: u32| progress.looks.iter().find(|(s, _)| *s == slot).map(|(_, l)| *l);
    for &entry in &progress.worn {
        let Some((owner, slot, zone)) = decode_item_entry(entry) else {
            continue;
        };
        if owner != k.wow_class_id || slot >= ARMOR_SLOTS {
            continue;
        }
        let look = shown_look(content, set, chosen(slot), zone);
        let Some(s) = gear.slots.get(slot as usize) else {
            continue;
        };
        let display_id = armor_display(set, slot, look);
        if display_id != 0 {
            if let Some(o) = out.get_mut(s.equip_slot() as usize) {
                *o = CharEnumItem {
                    display_id,
                    inventory_type: armor_inventory_type(gear, set, slot).min(255) as u8,
                };
            }
        }
    }
    let look = weapon_look(content, set, k.wow_class_id, &progress.worn, chosen);
    let loadout = Some(progress.loadout.as_str()).filter(|l| !l.is_empty());
    for (hand, kind, display_id) in loadout_armaments(content, class, loadout, look) {
        if let Some(o) = out.get_mut(hand.equip_slot() as usize) {
            *o = CharEnumItem {
                display_id,
                inventory_type: kind.inventory_type().min(255) as u8,
            };
        }
    }
    out
}

// The hero the sheet shows, as a roster row.
pub(super) fn draft_character(
    content: &Content,
    draft: &Draft,
    progress: Option<&Progress>,
    guid: u64,
) -> Character {
    let class = content
        .class(&draft.class)
        .map_or(0, |c| c.wow_class_id as u8);
    Character {
        guid,
        name: draft.name.clone(),
        race: draft.race,
        class,
        gender: draft.sex,
        skin: draft.look[0],
        face: draft.look[1],
        hair_style: draft.look[2],
        hair_color: draft.look[3],
        facial_hair: draft.look[4],
        level: 1,
        zone: 0,
        map: 0,
        position: benilla_protocol::wire::Vector3d {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        flags: 0,
        equipment: progress
            .map(|p| worn_equipment(content, &draft.class, p))
            .unwrap_or_default(),
        pet_display_id: 0,
        pet_level: 0,
        pet_family: 0,
    }
}

/// Shows the sheet's hero in the glue booth while the sheet is open, and nothing else.
pub(super) fn show_the_hero(
    menus: Res<Menus>,
    content: Option<Res<GameContent>>,
    facts: Res<PreRunFacts>,
    yaw: Res<ModelYaw>,
    mut model: ResMut<SelectModel>,
    mut shown: Local<Option<(String, u32, u32)>>,
) {
    let want = match (&menus.screen, &content) {
        (Screen::Sheet(s), Some(content)) => {
            let progress = facts.progress(&s.draft.class);
            let c = draft_character(&content.0, &s.draft, progress, 0);
            let held = [15, 16, 17]
                .map(|slot| crate::picker::weapon_sheath(&content.0, c.equipment[slot].display_id));
            Some(SelectModelView {
                hero: FigureHero {
                    character: c,
                    stand_state: 0,
                    held_sheath: held,
                    start_outfit_under: true,
                },
                yaw: yaw.0,
            })
        }
        _ => None,
    };
    // compared by what shows: the look, the gear and the facing
    let key = want.as_ref().map(|v| {
        let c = &v.hero.character;
        (
            format!(
                "{} {} {} {:?} {:?}",
                c.race,
                c.gender,
                c.class,
                [c.skin, c.face, c.hair_style, c.hair_color, c.facial_hair],
                c.equipment.map(|e| e.display_id)
            ),
            facts.version(),
            v.yaw.to_bits(),
        )
    });
    if *shown == key {
        return;
    }
    *shown = key;
    model.0 = want;
}

/// Turns the hero while the mouse drags over it.
pub(super) fn drag_to_turn(
    areas: Query<&Interaction, With<TurnArea>>,
    motion: Res<AccumulatedMouseMotion>,
    mut yaw: ResMut<ModelYaw>,
) {
    if motion.delta.x == 0.0 {
        return;
    }
    if areas.iter().any(|i| *i == Interaction::Pressed) {
        yaw.0 += motion.delta.x * TURN_PER_PIXEL;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_sim::game::rewards::gear_item_entry;

    #[test]
    fn the_sheets_hero_wears_its_class_gear_in_the_look_chosen() {
        let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let w = content.class("warrior").unwrap().wow_class_id;
        let set = content.gear_set("warrior").unwrap();
        let progress = Progress {
            class: "warrior".into(),
            talent_experience: 0,
            points: 0,
            spent: 0,
            into_next: 0,
            next_cost: 0,
            zones_open: 1,
            zones_discovered: Vec::new(),
            talents: vec![],
            worn: vec![gear_item_entry(w, 0, 1)],
            looks: vec![(0, 6)],
            collection: vec![],
            prestige: Default::default(),
            loadout: String::new(),
            new_gear: false,
            new_points: false,
        };
        let e = worn_equipment(&content, "warrior", &progress);
        let head = content.gear.slots[0].equip_slot() as usize;
        assert_eq!(
            e[head].display_id,
            armor_display(set, 0, 6),
            "the look chosen: Gurubashi's"
        );
        // the weapon loadout's two-hander, in Elwynn's armaments with no class gear weapon worn
        let sword = &content.loadout("warrior", None).unwrap().weapons[0];
        assert_eq!(e[15].display_id, sword.zone_display("elwynn", 0));
        assert_eq!(
            e.iter().filter(|i| i.display_id != 0).count(),
            2,
            "nothing else worn"
        );
    }
}
