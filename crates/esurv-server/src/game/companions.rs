//! Companions on the field (docs/game/achievements.md, "Companions"): each run companion
//! (`esurv_sim::game::companions`) is a unit of its look (`companions.ron`), its entry its own
//! ([`crate::entries::companion`], so the creature query names it), friendly to its hero and
//! neither attackable nor selectable. It walks as pets do (`SMSG_MONSTER_MOVE`); one that glides
//! (`hover`) is always on a flight at its height, bobbing, as a flying pet is. It is mirrored
//! with the pets in [`Game::show`]: kept in `shown`, so it stands with the run and vanishes with
//! it.

use esurv_sim::game::enemy_ai::Segment;
use esurv_sim::game::run::{Event, Unit};

use super::{unit_guid, Game, Shown, HOVER_RESEND_LEAD};
use crate::packets;
use crate::wire::Packet;
use crate::update::{
    field, Fields, Living, ObjectTypeId, ObjectUpdate, Speeds, TYPE_OBJECT, TYPE_UNIT,
};

/// Not attackable, not selectable (`UNIT_FLAG_NON_ATTACKABLE | UNIT_FLAG_NOT_SELECTABLE`): no
/// enemy looks at it and the player clicks through it.
pub const COMPANION_UNIT_FLAGS: u32 = 0x0200_0002;
/// Its health as the client shows it (it is never hurt).
const COMPANION_HEALTH: u32 = 1;
/// Its size for collisions: small, so it never seems to stand in the way.
const COMPANION_RADIUS: f32 = 0.2;

impl Game {
    /// How high companion `i` glides above the ground, when it does (`Companion::hover`).
    pub(super) fn companion_hover(&self, i: usize) -> Option<f32> {
        let k = self.run.companions.get(i)?;
        self.run.content.companion(&k.companion)?.hover
    }

    /// The companions' spell looks (`Companion::kits`): each one just created plays its appearing
    /// kit, each one handing gold over its hand-over kit (`SMSG_PLAY_SPELL_VISUAL` on itself, so
    /// at its own size).
    pub(super) fn companion_spell_looks(&self, appeared: &[u32], events: &[Event]) -> Vec<Packet> {
        let deposits = events.iter().filter_map(|e| match e {
            Event::CompanionDeposit { companion, .. } => Some((*companion, false)),
            _ => None,
        });
        let mut out = Vec::new();
        for (id, appearing) in appeared.iter().map(|&id| (id, true)).chain(deposits) {
            let Some(k) = self.run.companions.iter().find(|k| k.id == id) else {
                continue;
            };
            let Some(kits) = self.run.content.companion(&k.companion).map(|c| c.kits) else {
                continue;
            };
            let kit = if appearing { kits.appear } else { kits.hand_over };
            let (Some(kit), Some(s)) = (kit, self.shown.get(&id)) else {
                continue;
            };
            out.push(packets::play_spell_visual(s.guid, kit));
        }
        out
    }

    /// Companion `i`'s create and guid.
    fn companion_object(&self, i: usize, now: f32) -> Option<(ObjectUpdate, u64)> {
        let content = &self.run.content;
        let k = &self.run.companions[i];
        let place = content
            .companions
            .iter()
            .position(|c| c.id == k.companion)?;
        let look = &content.companions[place];
        let owner = self.run.heroes.get(k.hero)?;
        let entry = crate::entries::companion(place);
        let guid = unit_guid(entry, k.id);
        let made_by = self.guid_of(Unit::Hero(owner.id)).unwrap_or(0);
        let at = k.segment.position(now);
        let display = look.display();
        // its size: the look's own (from the player's data) times the companion's
        let own = self
            .data
            .as_ref()
            .and_then(|d| d.model_scale(display))
            .unwrap_or(1.0);
        let scale = own * look.scale;
        let mut f = Fields::new();
        f.u64(field::OBJECT_GUID, guid)
            .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_UNIT)
            .u32(field::OBJECT_ENTRY, entry)
            .f32(field::OBJECT_SCALE_X, scale);
        f.u64(field::UNIT_CREATEDBY, made_by);
        f.u32(field::UNIT_HEALTH, COMPANION_HEALTH)
            .u32(field::UNIT_MAXHEALTH, COMPANION_HEALTH)
            .u32(field::UNIT_LEVEL, 1)
            .u32(
                field::UNIT_FACTIONTEMPLATE,
                content.races[owner.race].faction_template,
            )
            .u32(field::UNIT_FLAGS, COMPANION_UNIT_FLAGS);
        f.f32(field::UNIT_BOUNDINGRADIUS, COMPANION_RADIUS)
            .f32(field::UNIT_COMBATREACH, COMPANION_RADIUS)
            .u32(field::UNIT_DISPLAYID, display)
            .u32(field::UNIT_NATIVEDISPLAYID, display)
            .f32(field::UNIT_MOD_CAST_SPEED, 1.0);
        let living = Living {
            position: [at.0, at.1, self.ground_z(at) + look.hover.unwrap_or(0.0)],
            facing: owner.facing,
            speeds: Speeds::running_at(k.rank.speed),
        };
        Some((
            ObjectUpdate::Create {
                guid,
                type_id: ObjectTypeId::Unit,
                living,
                fields: f,
                own: false,
            },
            guid,
        ))
    }

    /// The companions as the clients should see them: new ones created, moved ones walking (or
    /// flying, for one that glides: told again as its last flight runs out).
    pub(super) fn mirror_companions(
        &mut self,
        objects: &mut Vec<ObjectUpdate>,
        walks: &mut Vec<(u64, Segment)>,
        flights: &mut Vec<(u32, u64, Segment, f32)>,
        appeared: &mut Vec<u32>,
    ) {
        let now = self.run.now;
        let running =
            self.run.phase == esurv_sim::game::run::Phase::Running && !self.run.in_cutscene();
        for i in 0..self.run.companions.len() {
            let (id, segment) = (self.run.companions[i].id, self.run.companions[i].segment);
            let hover = self.companion_hover(i);
            let flight_due = running
                && self
                    .hover_until
                    .get(&id)
                    .is_none_or(|&t| now >= t - HOVER_RESEND_LEAD);
            match self.shown.get_mut(&id) {
                None => {
                    let name = self.run.companions[i].companion.clone();
                    if let Some((create, guid)) = self.companion_object(i, now) {
                        objects.push(create);
                        appeared.push(id);
                        if let Some(h) = hover {
                            flights.push((id, guid, segment, h));
                        } else if segment.moving(now) {
                            walks.push((guid, segment));
                        }
                        self.shown.insert(
                            id,
                            Shown {
                                guid,
                                segment,
                                health: 0,
                                name,
                                corpse_until: None,
                                debuffs: Vec::new(),
                                level: 1,
                            },
                        );
                    }
                }
                Some(s) => {
                    if let Some(h) = hover {
                        if s.segment != segment || flight_due {
                            s.segment = segment;
                            flights.push((id, s.guid, segment, h));
                        }
                    } else if s.segment != segment {
                        s.segment = segment;
                        walks.push((s.guid, segment));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::Content;
    use esurv_sim::game::companions::CompanionSetup;
    use esurv_sim::game::run::{Flat, HeroSetup, Run};

    use super::*;

    fn content() -> Arc<Content> {
        Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap())
    }

    fn game_with(companion: &str) -> Game {
        let content = content();
        let saved = crate::save::SavedHero {
            guid: 1,
            name: "Ana".into(),
            class: "warrior".into(),
            race: "human".into(),
            gender: 0,
            look: [0; 5],
            records: Default::default(),
            zone: None,
            start: None,
            companion: None,
            wears: Default::default(),
            plot: None,
            temperament: None,
            time_period: Default::default(),
        };
        let (_host, inputs) = std::sync::mpsc::channel();
        let saves = Arc::new(crate::save::Saves::in_memory());
        let mut game = Game::new(content.clone(), None, 0, &saved, saves, inputs).unwrap();
        let setup = HeroSetup {
            class: "warrior".into(),
            race: "human".into(),
            companion: Some(CompanionSetup {
                id: companion.into(),
                rank: 1,
            }),
            ..Default::default()
        };
        game.run = Run::new(
            content,
            "elwynn",
            Default::default(),
            &[setup],
            Box::new(Flat),
            3,
        )
        .unwrap();
        game
    }

    /// A companion appears as a unit of its look and size, its entry its own, friendly to its
    /// hero, never attackable nor selectable.
    #[test]
    fn a_companion_is_a_friendly_unit_no_one_can_touch() {
        let game = game_with("tiny_hogger");
        let (create, _) = game.companion_object(0, game.run.now).expect("a create");
        let ObjectUpdate::Create { fields, .. } = create else {
            panic!("a create")
        };
        let k = game.run.content.companion("tiny_hogger").unwrap();
        let place = game
            .run
            .content
            .companions
            .iter()
            .position(|c| c.id == k.id)
            .unwrap();
        assert_eq!(fields.get(field::UNIT_DISPLAYID), Some(384));
        assert_eq!(
            fields.get(field::OBJECT_ENTRY),
            Some(crate::entries::companion(place))
        );
        assert_eq!(fields.get(field::UNIT_FLAGS), Some(COMPANION_UNIT_FLAGS));
        assert_eq!(
            fields.get(field::OBJECT_SCALE_X).map(f32::from_bits),
            Some(0.4)
        );
        let race = &game.run.content.races[game.run.heroes[0].race];
        assert_eq!(
            fields.get(field::UNIT_FACTIONTEMPLATE),
            Some(race.faction_template)
        );
    }

    /// The Eye glides: it is told as a flight, and again as the flight runs out.
    #[test]
    fn the_eye_glides_on_flights() {
        let mut game = game_with("baby_cthun_eye");
        game.run.phase = esurv_sim::game::run::Phase::Running;
        let (mut objects, mut walks, mut flights, mut appeared) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        game.mirror_companions(&mut objects, &mut walks, &mut flights, &mut appeared);
        assert_eq!((objects.len(), walks.len(), flights.len()), (1, 0, 1));
        assert_eq!(flights[0].3, 1.2);
        let walker = game_with("bombay_cat");
        assert_eq!(walker.companion_hover(0), None);
    }

    /// The Warhorse Foal plays its counterpart's looks: the summoning burst as it
    /// appears, the holy cast as it hands gold over; a cat plays none.
    #[test]
    fn the_foal_plays_the_chargers_looks_as_it_appears_and_hands_gold_over() {
        let mut game = game_with("warhorse_foal");
        let (mut objects, mut walks, mut flights, mut appeared) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        game.mirror_companions(&mut objects, &mut walks, &mut flights, &mut appeared);
        let id = game.run.companions[0].id;
        assert_eq!(appeared, [id]);
        let guid = game.shown[&id].guid;
        assert_eq!(game.companion_spell_looks(&appeared, &[]), [packets::play_spell_visual(guid, 165)]);
        let deposit = Event::CompanionDeposit { companion: id, hero: game.run.heroes[0].id, gold: 3 };
        assert_eq!(game.companion_spell_looks(&[], &[deposit.clone()]), [packets::play_spell_visual(guid, 193)]);
        let mut cat = game_with("bombay_cat");
        let mut appeared = Vec::new();
        cat.mirror_companions(&mut Vec::new(), &mut Vec::new(), &mut Vec::new(), &mut appeared);
        assert!(cat.companion_spell_looks(&appeared, &[]).is_empty());
    }
}
