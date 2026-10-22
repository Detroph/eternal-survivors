//! How the boss mechanics' building blocks show (docs/game/bosses.md,
//! "Building blocks"): a marked attack is drawn by the clients with a HUD as an orange indicator
//! (`ground_indicators`); for a client without one it is laid as ground looks over every disc covering its
//! shape (the warning look), with the fill look growing inside each disc in steps over the
//! wind-up; an object lying on the ground is a game object of its model with a glow under it that
//! fills as a hero stands on it; a kneeling boss kneels; a phase change shows its new weapons and
//! model; and the mechanics' raid warnings and timer bars go to a client with a HUD of its own
//! (the others get the warnings as the client's raid warnings).

use std::collections::HashMap;

use esurv_protocol::{BossTimer, BossWarning, ToClient};
use esurv_sim::game::enemy_ai::Vec2;
use esurv_sim::game::run::{Event, Unit};

use super::pickups::{gameobject_guid, PICKUP_FACTION};
use super::Game;
use crate::packets::{self, GameObjectInfo};
use crate::update::{field, update_object, Fields, ObjectTypeId, ObjectUpdate, TYPE_GAMEOBJECT, TYPE_OBJECT};
use crate::wire::Packet;

/// The looks of marks and objects are numbered from here, apart from the simulation's areas and
/// the game's short looks.
const MARK_LOOK_IDS: u32 = 0x4000_0000;
/// A mark's fill grows in this many steps over its wind-up (a big one, of many discs, in fewer).
const FILL_STEPS: u32 = 3;
const FILL_STEPS_BIG: u32 = 2;
/// A mark of more discs than this is big.
const BIG_MARK: usize = 8;
/// A counter's seconds as told: negative, a bar that does not count down
/// (`esurv_protocol::BossTimer`).
pub const COUNTER_SECONDS: f32 = -1.0;
/// `UNIT_STAND_STATE_KNEEL`, the first byte of `UNIT_FIELD_BYTES_1`.
const STAND_KNEEL: u8 = 8;
/// `GAMEOBJECT_TYPE_GENERIC`, ready.
const GO_TYPE_GENERIC: u32 = 5;
const GO_READY: u32 = 1;
/// Nobody clicks it: a hero stands on it.
const GO_FLAG_NO_INTERACT: u32 = 0x10;
/// An aura slot's flags: it is on.
const AURA_ON: u32 = 0x02;

/// A mark as the clients were last told: its looks, its discs, and how far its fill has grown.
pub(super) struct ShownMark {
    looks: Vec<u32>,
    patches: Vec<(Vec2, f32)>,
    fill_spell: u32,
    from: f32,
    wind_up: f32,
    steps: u32,
    filled: u32,
}

/// An object as the clients were last told: its guid, the glow under it and the fill on it.
pub(super) struct ShownObject {
    guid: u64,
    glow: u32,
    fills: Vec<u32>,
    told: u32,
}

/// The game-object entry of an object that looks like `display_id`.
pub fn object_entry(display_id: u32) -> u32 {
    crate::entries::GROUND_OBJECTS_FROM + display_id
}

/// What the client's game-object query hears of an object any boss puts down.
pub fn object_info(content: &esurv_content::Content, entry: u32) -> Option<GameObjectInfo> {
    let o = content.zones.iter().flat_map(|z| &z.bosses).flat_map(|b| b.ground_objects()).find(|o| object_entry(o.display_id) == entry)?;
    Some(GameObjectInfo { type_id: GO_TYPE_GENERIC, display_id: o.display_id, name: o.name.clone(), data: [0; 24] })
}

impl Game {
    fn free_look_id(&self) -> u32 {
        (MARK_LOOK_IDS..).find(|id| !self.looks_shown.contains_key(id)).unwrap_or(MARK_LOOK_IDS)
    }

    /// The guid looks of the mechanics are put down by: the first hero's (as a boss's marked spot
    /// is).
    fn look_caster(&self) -> Option<u64> {
        let h = self.run.heroes.first()?.id;
        self.guid_of(Unit::Hero(h))
    }

    /// A building block's event as the clients see it: what goes to every client, and to each
    /// hero's own.
    pub(super) fn present_boss_event(&mut self, ev: &Event, all: &mut Vec<Packet>, own: &mut Vec<(u32, Packet)>) {
        match ev {
            // drawn by the clients as indicators (`ground_indicators`); as spell looks only for a
            // client without a HUD of its own
            Event::TelegraphShown { id, shape, at, facing, warning_spell, fill_spell, wind_up } => {
                if !self.marks_as_looks() {
                    return;
                }
                let Some(caster) = self.look_caster() else { return };
                let patches = shape.patches(*at, *facing);
                let mut looks = Vec::new();
                for (c, r) in &patches {
                    let look = self.free_look_id();
                    all.push(self.ground_look(look, caster, *warning_spell, *c, *r, None));
                    looks.push(look);
                }
                let steps = if patches.len() > BIG_MARK { FILL_STEPS_BIG } else { FILL_STEPS };
                self.marks_shown.insert(*id, ShownMark { looks, patches, fill_spell: *fill_spell, from: self.run.now, wind_up: *wind_up, steps, filled: 0 });
            }
            Event::BossKneels { id, kneeling } => {
                if let Some(guid) = self.guid_of(Unit::Enemy(*id)) {
                    let mut f = Fields::new();
                    f.bytes(field::UNIT_BYTES_1, [if *kneeling { STAND_KNEEL } else { 0 }, 0, 0, 0]);
                    all.push(update_object(&[ObjectUpdate::Values { guid, fields: f }]));
                }
            }
            Event::BossChanged { id } => {
                let Some(i) = self.run.enemies.iter().position(|e| e.id == *id) else { return };
                let Some(guid) = self.guid_of(Unit::Enemy(*id)) else { return };
                let mut f = Fields::new();
                if let Some(display) = self.run.enemies[i].mech.display_id {
                    f.u32(field::UNIT_DISPLAYID, display).u32(field::UNIT_NATIVEDISPLAYID, display);
                }
                if let Some(scale) = self.run.enemies[i].mech.scale {
                    f.f32(field::OBJECT_SCALE_X, scale);
                }
                // every slot told again, an empty one cleared
                let weapons = self.enemy_weapons(i).unwrap_or_default();
                for (slot, w) in weapons.slots().iter().enumerate() {
                    let slot = slot as u16;
                    f.u32(field::UNIT_VIRTUAL_ITEM_SLOT_DISPLAY + slot, w.map_or(0, |w| w.display))
                        .u32(field::UNIT_VIRTUAL_ITEM_INFO + 2 * slot, w.map_or(0, |w| w.info()))
                        .u32(field::UNIT_VIRTUAL_ITEM_INFO + 2 * slot + 1, w.map_or(0, |w| w.sheath & 0xff));
                }
                all.push(update_object(&[ObjectUpdate::Values { guid, fields: f }]));
            }
            Event::PauseBegan { text, .. } if !text.is_empty() => all.push(packets::raid_warning(text)),
            Event::Warning { text, colour, icon_spell, soon } => {
                let message = ToClient::BossWarning(BossWarning { text: text.clone(), colour: colour.rgb(), icon: self.icon_name(*icon_spell), soon: *soon });
                for p in &self.players {
                    if self.told.get(&p.hero).is_some_and(|t| t.screens.hud) {
                        own.extend(self.lines(vec![(p.hero, message.clone())]));
                    } else {
                        own.push((p.hero, packets::raid_warning(text)));
                    }
                }
            }
            Event::BarStarted { boss, key, label, icon_spell, until } => {
                let seconds = (until - self.run.now).max(0.0);
                let message = ToClient::BossTimer(BossTimer { boss: *boss, key: *key, label: label.clone(), icon: self.icon_name(*icon_spell), seconds });
                self.to_huds(message, own);
            }
            Event::BarsCleared { boss } => self.to_huds(ToClient::BossTimersCleared { boss: *boss }, own),
            // a counter is a bar that does not count down (no seconds)
            Event::CounterShown { boss, key, label } => {
                let message = ToClient::BossTimer(BossTimer { boss: *boss, key: *key, label: label.clone(), icon: String::new(), seconds: COUNTER_SECONDS });
                self.to_huds(message, own);
            }
            _ => {}
        }
    }

    /// Spell `spell`'s icon's file name, for a warning or bar (none for 0 or without the game data).
    fn icon_name(&self, spell: u32) -> String {
        self.data.as_ref().and_then(|d| d.spell_icon(spell)).unwrap_or_default().to_string()
    }

    fn to_huds(&self, message: ToClient, own: &mut Vec<(u32, Packet)>) {
        let huds: Vec<u32> = self.players.iter().map(|p| p.hero).filter(|h| self.told.get(h).is_some_and(|t| t.screens.hud)).collect();
        own.extend(self.lines(huds.into_iter().map(|h| (h, message.clone())).collect()));
    }

    /// Marks fill as they wind up and go once they landed; objects appear, fill as a hero stands
    /// on them and go once broken, taken or hatched.
    pub(super) fn mirror_boss_looks(&mut self, out: &mut Vec<Packet>) {
        let now = self.run.now;
        let live: Vec<u32> = self.run.telegraphs.iter().map(|t| t.id).collect();
        let ended: Vec<u32> = self.marks_shown.keys().copied().filter(|id| !live.contains(id)).collect();
        for id in ended {
            if let Some(m) = self.marks_shown.remove(&id) {
                out.extend(m.looks.iter().filter_map(|l| self.ground_look_gone(*l)));
            }
        }
        let caster = self.look_caster();
        let ids: Vec<u32> = self.marks_shown.keys().copied().collect();
        for id in ids {
            let (due, steps, filled) = {
                let m = &self.marks_shown[&id];
                let progress = ((now - m.from) / m.wind_up.max(0.01)).clamp(0.0, 1.0);
                (((progress * m.steps as f32).floor() as u32).min(m.steps), m.steps, m.filled)
            };
            let Some(caster) = caster else { continue };
            for step in filled + 1..=due {
                let (patches, spell) = {
                    let m = &self.marks_shown[&id];
                    (m.patches.clone(), m.fill_spell)
                };
                for (c, r) in patches {
                    let look = self.free_look_id();
                    out.push(self.ground_look(look, caster, spell, c, r * step as f32 / steps as f32, None));
                    if let Some(m) = self.marks_shown.get_mut(&id) {
                        m.looks.push(look);
                    }
                }
            }
            if let Some(m) = self.marks_shown.get_mut(&id) {
                m.filled = m.filled.max(due);
            }
        }
        // objects
        let live: HashMap<u32, usize> = self.run.ground_objects.iter().enumerate().map(|(k, o)| (o.id, k)).collect();
        let gone: Vec<u32> = self.objects_shown.keys().copied().filter(|id| !live.contains_key(id)).collect();
        for id in gone {
            if let Some(s) = self.objects_shown.remove(&id) {
                out.push(packets::destroy_object(s.guid));
                out.extend(std::iter::once(s.glow).chain(s.fills).filter_map(|l| self.ground_look_gone(l)));
            }
        }
        let Some(caster) = caster else { return };
        let mut creates = Vec::new();
        for k in 0..self.run.ground_objects.len() {
            let o = self.run.ground_objects[k].clone();
            if !self.objects_shown.contains_key(&o.id) {
                let (guid, create) = shown_object(&o.spec, o.id, [o.at.0, o.at.1, self.ground_z(o.at)]);
                creates.push(create);
                let glow = self.free_look_id();
                out.push(self.ground_look(glow, caster, o.spec.glow_spell, o.at, o.spec.radius, None));
                self.objects_shown.insert(o.id, ShownObject { guid, glow, fills: Vec::new(), told: 0 });
            }
            let told = self.objects_shown[&o.id].told;
            if o.told == told {
                continue;
            }
            // the glow fills as the hero stands on it, and empties as it steps off
            if o.told < told {
                let fills = std::mem::take(&mut self.objects_shown.get_mut(&o.id).unwrap().fills);
                out.extend(fills.into_iter().filter_map(|l| self.ground_look_gone(l)));
            } else {
                let steps = (1.0 / esurv_sim::game::interactables::PROGRESS_STEP) as u32;
                for step in told + 1..=o.told {
                    let fill = self.free_look_id();
                    out.push(self.ground_look(fill, caster, o.spec.glow_spell, o.at, o.spec.radius * step as f32 / steps as f32, None));
                    self.objects_shown.get_mut(&o.id).unwrap().fills.push(fill);
                }
            }
            self.objects_shown.get_mut(&o.id).unwrap().told = o.told;
        }
        if !creates.is_empty() {
            out.push(update_object(&creates));
        }
    }

    /// Every object on the ground, as creates (for a player who joins).
    pub(super) fn ground_objects_now(&self) -> Vec<ObjectUpdate> {
        self.run
            .ground_objects
            .iter()
            .filter_map(|o| {
                self.objects_shown.get(&o.id)?;
                Some(shown_object(&o.spec, o.id, [o.at.0, o.at.1, self.ground_z(o.at)]).1)
            })
            .collect()
    }

    /// The marks and objects shown are forgotten (the field was cleared).
    pub(super) fn forget_boss_looks(&mut self) -> Vec<Packet> {
        self.marks_shown.clear();
        self.objects_shown.drain().map(|(_, s)| packets::destroy_object(s.guid)).collect()
    }
}

fn object_create(guid: u64, entry: u32, display_id: u32, scale: f32, position: [f32; 3]) -> ObjectUpdate {
    let mut f = Fields::new();
    f.u64(field::OBJECT_GUID, guid)
        .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_GAMEOBJECT)
        .u32(field::OBJECT_ENTRY, entry)
        .f32(field::OBJECT_SCALE_X, scale);
    f.u32(field::GAMEOBJECT_DISPLAYID, display_id)
        .u32(field::GAMEOBJECT_FLAGS, GO_FLAG_NO_INTERACT)
        .f32(field::GAMEOBJECT_ROTATION + 3, 1.0)
        .u32(field::GAMEOBJECT_STATE, GO_READY);
    for (k, v) in position.iter().enumerate() {
        f.f32(field::GAMEOBJECT_POS_X + k as u16, *v);
    }
    f.f32(field::GAMEOBJECT_FACING, 0.0)
        .u32(field::GAMEOBJECT_FACTION, PICKUP_FACTION)
        .u32(field::GAMEOBJECT_TYPE_ID, GO_TYPE_GENERIC);
    ObjectUpdate::CreatePlaced { guid, type_id: ObjectTypeId::GameObject, position, facing: 0.0, fields: f }
}

/// An object on the ground as the clients see it, with its guid: a game object of its model, or,
/// for one shown as a creature model (the troll wards and totems) or a planted item (Morbent's
/// Bane), an unselectable unit, wearing its aura look if it has one.
fn shown_object(spec: &esurv_content::GroundObject, id: u32, position: [f32; 3]) -> (u64, ObjectUpdate) {
    let unit_look = spec.planted_item.as_ref().map(esurv_content::looks::planted_item_look).or(spec.creature_display);
    match unit_look {
        Some(display) => {
            let guid = super::unit_guid(crate::entries::GROUND_UNIT, id);
            let mut create = super::bunnies::still_unit_create(guid, crate::entries::GROUND_UNIT, display, spec.scale, position);
            if let (Some(look), ObjectUpdate::Create { fields, .. }) = (spec.aura_look, &mut create) {
                fields.u32(field::UNIT_AURA, look).u32(field::UNIT_AURAFLAGS, AURA_ON).u32(field::UNIT_AURALEVELS, 1);
            }
            (guid, create)
        }
        None => {
            let entry = object_entry(spec.display_id);
            let guid = gameobject_guid(entry, id);
            (guid, object_create(guid, entry, spec.display_id, spec.scale, position))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Morbent's Bane is the Torch of Holy Flame's model planted in an unseen unit, wearing Divine
    /// Shield's golden bubble; a plain object stays a game object of its model.
    #[test]
    fn a_planted_item_is_a_unit_of_its_composed_look_wearing_its_aura() {
        let content = esurv_content::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let morbent = content.zones.iter().flat_map(|z| &z.bosses).find(|b| b.name == "Morbent Fel").unwrap();
        let bane = morbent.ground_objects().into_iter().find(|o| o.name == "Morbent's Bane").unwrap();
        let (guid, create) = shown_object(bane, 7, [0.0; 3]);
        assert_eq!(guid, super::super::unit_guid(crate::entries::GROUND_UNIT, 7));
        let ObjectUpdate::Create { fields, type_id: ObjectTypeId::Unit, .. } = create else { panic!("a unit create") };
        assert_eq!(fields.get(field::UNIT_DISPLAYID), Some(esurv_content::looks::planted_item_look(bane.planted_item.as_ref().unwrap())));
        assert_eq!(fields.get(field::UNIT_AURA), bane.aura_look);
        assert_eq!(fields.get(field::UNIT_AURAFLAGS), Some(AURA_ON));
        let plain = esurv_content::GroundObject { planted_item: None, aura_look: None, ..bane.clone() };
        assert!(matches!(shown_object(&plain, 7, [0.0; 3]).1, ObjectUpdate::CreatePlaced { type_id: ObjectTypeId::GameObject, .. }));
    }
}
