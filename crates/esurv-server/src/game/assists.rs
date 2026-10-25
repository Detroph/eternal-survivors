//! Assists on the field (docs/game/assists.md): the ally a battle standard summons, shown as its
//! hand-made player character as the Warsong bosses show theirs (`player_enemies`'s shape: race,
//! gender, looks, form, class PvP gear under its picked gear), drawn at `ally_scale` (twice its
//! size), friendly to the heroes and not to be targeted. It arrives with its arrival's look, moves
//! and charges, swings, casts its steps' looks, and leaves its own way. Its hits are logged under
//! its own name (a swing as its swing, the rest under the look it last cast), the enemies it
//! marks wear their looks, and the totems it plants take its race's totem look.

use std::collections::HashMap;

use benilla_protocol::messages::opcode;
use esurv_content::{Appear, Leave};
use esurv_sim::game::assists::AllyMove;
use esurv_sim::game::enemy_ai::{Segment, Vec2};
use esurv_sim::game::run::{Event, Unit};

use super::present::{self, Aura};
use super::{dist, sheath_state, Game, PLAYER_BOUNDING_RADIUS, PLAYER_COMBAT_REACH};
use crate::packets::{self, SpellTarget};
use crate::update::{field, update_object, Fields, Living, ObjectTypeId, ObjectUpdate, Speeds, TYPE_OBJECT, TYPE_PLAYER, TYPE_UNIT};
use crate::wire::Packet;

/// An ally's guid: a player guid with this bit and the ally's id (apart from the player-looking
/// enemies' and the hero copies' bits).
const ALLY_BIT: u64 = 0x1000_0000;
/// It shows full health (it is never hurt).
const ALLY_HEALTH: u32 = 100;
/// It cannot be attacked or selected (`UNIT_FLAG_NON_ATTACKABLE | UNIT_FLAG_NOT_SELECTABLE`).
pub const ALLY_UNIT_FLAGS: u32 = 0x0200_0002;
/// It runs at a hero's base run speed (yards a second), and runs off as fast.
const ALLY_RUN_SPEED: f32 = 7.0;

/// The looks of the ways it comes and goes (`Spell.dbc` ids): Blink's, Stealth's (worn as it
/// fades in), Vanish's, the Hearthstone's cast.
pub const BLINK_LOOK: u32 = 1953;
pub const STEALTH_LOOK: u32 = 1784;
pub const VANISH_LOOK: u32 = 1856;
pub const HEARTH_LOOK: u32 = 8690;
/// Out of stealth: it wears Stealth's look this long as it appears, then fades in (the client's
/// aura fade, 1 s).
const STEALTH_FADE_IN: f32 = 0.4;
/// Fading where it stands: it wears Stealth's look (its model fades to see-through over the
/// client's 1 s) and goes when the fade is done.
const FADE_OUT: f32 = 1.0;
/// Vanish's puff plays, then it is gone.
const VANISH_GONE: f32 = 0.2;
/// Its Hearthstone casts this long (a short cast bar and the cast's glow), then it is gone.
const HEARTH_CAST: f32 = 1.5;
/// Its body lies this long once it died.
const BODY_LIES: f32 = present::CORPSE_TIME;

/// The guid ally `id` is shown with.
pub fn ally_guid(id: u32) -> u64 {
    ALLY_BIT | u64::from(id & 0x0FFF_FFFF)
}

/// An ally as the clients were last told.
pub(super) struct ShownAlly {
    pub guid: u64,
    pub name: String,
    /// Its character's id.
    character: String,
    segment: Segment,
    /// Its race's place in `races.ron` (its totems' look).
    race: Option<usize>,
    /// Gone from the run: it is removed from the clients at this run time.
    gone_at: Option<f32>,
    /// A strafe or a jump the clients were told of as a player's relayed move, until it ends.
    relay: Option<RelayedMove>,
}

/// A strafe or a jump an ally is making, as the clients were told: how, when it ends and where,
/// and how long it lasts.
#[derive(Clone, Copy)]
struct RelayedMove {
    how: AllyMove,
    ends: f32,
    to: Vec2,
    seconds: f32,
}

/// What the clients are told of the run's allies.
#[derive(Default)]
pub(super) struct AlliesShown {
    allies: HashMap<u32, ShownAlly>,
    /// The looks the allies put on units: (unit, look, until).
    marks: Vec<(Unit, u32, f32)>,
    /// Allies' swings of this tick: (ally, enemy).
    swings: Vec<(u32, u32)>,
    /// The look each character last cast at enemies, laid or marked them with: its hits that are
    /// no swing show under it.
    last_look: HashMap<String, u32>,
    /// The totems allies planted, by totem id: the character's id.
    totems: HashMap<u32, String>,
    /// How many of `ESURV_STANDARDS`'s battle standards this run has laid.
    dev_laid: usize,
    /// `ESURV_ASSISTS`'s order was handed to this run.
    dev_order_set: bool,
}

/// `ESURV_ASSISTS=<id>[,<id>...]`, a dev aid: the characters (`player_characters.ron` ids) the
/// run's battle standards summon, in this order, before the random picks.
fn assists_order() -> Vec<String> {
    std::env::var("ESURV_ASSISTS").unwrap_or_default().split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

/// `ESURV_STANDARDS=<s>[,<s>...]`, a dev aid for a live look at assists: at each of these run
/// clocks (seconds) a battle standard is laid at the first hero's feet, so it is taken at once and
/// its ally comes then. Each fresh run lays them again.
fn standards_at() -> &'static [f32] {
    static AT: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    AT.get_or_init(|| {
        let mut at: Vec<f32> = std::env::var("ESURV_STANDARDS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        at.sort_by(f32::total_cmp);
        at
    })
}

impl Game {
    /// Lays the battle standards `ESURV_STANDARDS` has due by now, while the run runs, and hands
    /// the run `ESURV_ASSISTS`'s order once.
    pub(super) fn lay_standards_due(&mut self) {
        if !self.allies.dev_order_set {
            self.allies.dev_order_set = true;
            let order = assists_order();
            if !order.is_empty() {
                self.run.assists.forced = order;
            }
        }
        let at = standards_at();
        if self.run.phase != esurv_sim::game::run::Phase::Running {
            return;
        }
        while self.allies.dev_laid < at.len() && at[self.allies.dev_laid] <= self.run.now {
            self.allies.dev_laid += 1;
            if let Some(h) = self.run.heroes.first() {
                let (pos, faction) = (h.pos, self.run.party_faction);
                self.run.lay_pickup(esurv_sim::game::loot::Pickup::BattleStandard(faction), pos);
            }
        }
    }

    /// The ally of `character` on the field (the last summoned), if any.
    fn ally_of(&self, character: &str) -> Option<&ShownAlly> {
        self.allies.allies.values().filter(|a| a.character == character).min_by(|a, b| a.gone_at.is_some().cmp(&b.gone_at.is_some()))
    }

    /// The guid of ally `id`, while the clients know it.
    pub(super) fn ally_guid_shown(&self, id: u32) -> Option<u64> {
        self.allies.allies.get(&id).map(|a| a.guid)
    }

    /// The name of ally `id`.
    pub(super) fn ally_name(&self, id: u32) -> Option<String> {
        self.allies.allies.get(&id).map(|a| a.name.clone())
    }

    /// The name of character `id` (`player_characters.ron`), for the meter's "Assist: <name>".
    pub(super) fn character_name(&self, id: &str) -> String {
        self.run.content.player_characters.iter().find(|c| c.id == id).map_or_else(|| id.to_string(), |c| c.name.clone())
    }

    /// The race (its place in `races.ron`) of the character that planted totem `totem`, when an
    /// ally planted it.
    pub(super) fn totem_race(&self, totem: u32) -> Option<usize> {
        let character = self.allies.totems.get(&totem)?;
        let c = self.run.content.player_characters.iter().find(|c| &c.id == character)?;
        self.run.content.races.iter().position(|r| r.id == c.race)
    }

    /// The looks allies put on unit `u` that still hold, as auras.
    pub(super) fn ally_marks_on(&self, u: Unit) -> Vec<Aura> {
        let now = self.run.now;
        let mut out: Vec<Aura> = Vec::new();
        for &(_, spell, until) in self.allies.marks.iter().filter(|(on, spell, until)| *on == u && *spell != 0 && *until > now) {
            match out.iter_mut().find(|a| a.spell == spell) {
                Some(a) => a.until = a.until.max(until),
                None => out.push(Aura { spell, stacks: 1, until }),
            }
        }
        out
    }

    /// The create of the ally summoned by `ev` (an `AllySummoned`), its name first; and its
    /// arrival's looks, to follow the create.
    fn ally_appears(&mut self, ev: &Event) -> Option<(Packet, ObjectUpdate, Vec<Packet>)> {
        let Event::AllySummoned { id, hero, character, at, facing, how, look_spell, scale } = ev else {
            return None;
        };
        let content = self.run.content.clone();
        let c = content.player_characters.iter().find(|c| &c.id == character)?;
        let shape = self.character_shape(c, None, *id)?;
        let race = content.races.iter().position(|r| r.id == c.race);
        let faction_template = race.map_or(super::pickups::PICKUP_FACTION, |r| content.races[r].faction_template);
        let level = self.run.heroes.iter().find(|h| h.id == *hero).map_or(1, |h| h.progress.level);
        let guid = ally_guid(*id);
        let name = packets::name_query_response(guid, &shape.name, shape.race, shape.gender, shape.class);
        let mut f = Fields::new();
        f.u64(field::OBJECT_GUID, guid)
            .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_UNIT | TYPE_PLAYER)
            .f32(field::OBJECT_SCALE_X, *scale);
        f.u32(field::UNIT_HEALTH, ALLY_HEALTH).u32(field::UNIT_MAXHEALTH, ALLY_HEALTH);
        f.u32(field::UNIT_LEVEL, level).u32(field::UNIT_FACTIONTEMPLATE, faction_template);
        f.u32(field::UNIT_FLAGS, ALLY_UNIT_FLAGS);
        f.f32(field::UNIT_BOUNDINGRADIUS, PLAYER_BOUNDING_RADIUS).f32(field::UNIT_COMBATREACH, PLAYER_COMBAT_REACH);
        f.f32(field::UNIT_MOD_CAST_SPEED, 1.0);
        // its weapons drawn as the hero of its class holds them; a form's put away
        let sheath = if c.form.is_some() { 0 } else { sheath_state(&c.class) };
        f.bytes(field::UNIT_BYTES_2, [sheath, 0, 0, 0]);
        shape.dress(&mut f);
        // out of stealth: it comes wearing Stealth (see-through), then fades in
        if *how == Appear::Stealth {
            f.extend(&present::aura_fields(&[Aura { spell: STEALTH_LOOK, stacks: 1, until: 0.0 }], 0, 0));
            let mut clear = present::aura_fields(&[], 1, 0);
            clear.u32(field::UNIT_HEALTH, ALLY_HEALTH);
            self.later.push((self.run.now + STEALTH_FADE_IN, update_object(&[ObjectUpdate::Values { guid, fields: clear }])));
        }
        let living = Living { position: [at.0, at.1, self.ground_z(*at)], facing: *facing, speeds: Speeds::running_at(ALLY_RUN_SPEED) };
        let create = ObjectUpdate::Create { guid, type_id: ObjectTypeId::Player, living, fields: f, own: false };
        let mut looks = Vec::new();
        if *how == Appear::Blink {
            looks.push(packets::spell_go(guid, BLINK_LOOK, &[], SpellTarget::Caster, None));
        }
        if let Some(look) = look_spell.filter(|&l| l != 0) {
            looks.push(packets::spell_go(guid, look, &[guid], SpellTarget::Unit(guid), None));
        }
        eprintln!("esurv: assist {} ({}) comes at {:.1} s", shape.name, character, self.run.now);
        self.allies.allies.insert(
            *id,
            ShownAlly { guid, name: shape.name.clone(), character: character.clone(), segment: Segment::standing(*at, self.run.now), race, gone_at: None, relay: None },
        );
        Some((name, create, looks))
    }

    /// The allies before the rest of the tick shows: those summoned appear (their names first,
    /// their creates among `objects`, their arrival's looks in `after`), those that moved walk,
    /// charge or blink there, and what the tick's events say of them is noted (their swings, the
    /// looks they cast and put on units, the totems they planted).
    pub(super) fn mirror_allies(&mut self, events: &[Event], names: &mut Vec<Packet>, objects: &mut Vec<ObjectUpdate>, after: &mut Vec<Packet>) {
        self.allies.swings.clear();
        for ev in events {
            match ev {
                Event::AllySummoned { .. } => {
                    if let Some((name, create, looks)) = self.ally_appears(ev) {
                        names.push(name);
                        objects.push(create);
                        after.extend(looks);
                    }
                }
                Event::AllySwing { id, target } => self.allies.swings.push((*id, *target)),
                Event::AllyDropped { id, unit: Unit::Totem(t) } => {
                    if let Some(a) = self.allies.allies.get(id) {
                        self.allies.totems.insert(*t, a.character.clone());
                    }
                }
                Event::SpellGo { caster: Unit::Ally(id), spell, hits, .. } if hits.iter().any(|u| matches!(u, Unit::Enemy(_))) => {
                    self.note_ally_look(*id, *spell);
                }
                Event::AreaShown { caster: Unit::Ally(id), spell, .. } => self.note_ally_look(*id, *spell),
                Event::AuraShown { target, spell, until, from } => {
                    if let (Unit::Ally(id), Unit::Enemy(_)) = (from, target) {
                        self.note_ally_look(*id, *spell);
                    }
                    self.allies.marks.push((*target, *spell, *until));
                }
                _ => {}
            }
        }
        let now = self.run.now;
        self.allies.marks.retain(|(_, _, until)| *until > now);
        // a strafe or a jump whose time is up ends where it went
        let ended: Vec<(u32, u64, RelayedMove)> =
            self.allies.allies.iter().filter_map(|(id, s)| s.relay.filter(|r| now >= r.ends).map(|r| (*id, s.guid, r))).collect();
        for (id, guid, r) in ended {
            let facing = self.run.assists.allies.iter().find(|a| a.id == id).map_or(0.0, |a| a.facing);
            after.push(self.relay_ends(guid, r, r.to, facing));
            if let Some(s) = self.allies.allies.get_mut(&id) {
                s.relay = None;
            }
        }
        // where each ally on the field is going
        let moved: Vec<(u32, Segment, AllyMove, f32)> = self
            .run
            .assists
            .allies
            .iter()
            .filter_map(|a| {
                let s = self.allies.allies.get(&a.id)?;
                (s.segment != a.segment).then_some((a.id, a.segment, a.moving, a.facing))
            })
            .collect();
        for (id, segment, how, facing) in moved {
            let Some(s) = self.allies.allies.get_mut(&id) else { continue };
            let (guid, was, relay) = (s.guid, s.segment, s.relay.take());
            s.segment = segment;
            let from = segment.position(now);
            // a strafe or a jump cut short ends where it is; a run still going stops
            if let Some(r) = relay {
                after.push(self.relay_ends(guid, r, from, facing));
            } else if was.moving(now) && matches!(how, AllyMove::StrafeLeft | AllyMove::StrafeRight | AllyMove::Jump) {
                after.push(self.walk(guid, &Segment::standing(from, now)));
            }
            match how {
                AllyMove::StrafeLeft | AllyMove::StrafeRight | AllyMove::Jump => {
                    after.push(self.relay_begins(guid, how, &segment, facing));
                    let r = RelayedMove { how, ends: segment.start + segment.duration, to: segment.to, seconds: segment.duration };
                    if let Some(s) = self.allies.allies.get_mut(&id) {
                        s.relay = Some(r);
                    }
                }
                _ => after.push(self.walk(guid, &segment)),
            }
        }
    }

    /// The relayed move that starts ally `guid`'s strafe or jump along `segment`, facing
    /// `facing`: a strafe's start (the client moves it sideways at its run speed until the stop),
    /// or a jump's take-off with its flight (the client flies its arc).
    fn relay_begins(&mut self, guid: u64, how: AllyMove, segment: &Segment, facing: f32) -> Packet {
        let from = segment.from;
        let at = [from.0, from.1, self.ground_z(from)];
        let time = (self.run.now * 1000.0) as u32;
        match how {
            AllyMove::StrafeLeft => packets::relayed_move(opcode::MSG_MOVE_START_STRAFE_LEFT, guid, packets::MOVEFLAG_STRAFE_LEFT, time, at, facing, 0, None),
            AllyMove::StrafeRight => packets::relayed_move(opcode::MSG_MOVE_START_STRAFE_RIGHT, guid, packets::MOVEFLAG_STRAFE_RIGHT, time, at, facing, 0, None),
            _ => {
                let length = dist(from, segment.to);
                let angle = (segment.to.1 - from.1).atan2(segment.to.0 - from.0);
                let ground_speed = if segment.duration > 0.0 { length / segment.duration } else { 0.0 };
                let jump = packets::JumpTail { angle, ground_speed };
                packets::relayed_move(opcode::MSG_MOVE_JUMP, guid, packets::MOVEFLAG_JUMPING, time, at, facing, 0, Some(jump))
            }
        }
    }

    /// The relayed move that ends ally `guid`'s strafe or jump `r` at `at`, facing `facing`: the
    /// strafe's stop, or the jump's landing.
    fn relay_ends(&mut self, guid: u64, r: RelayedMove, at: Vec2, facing: f32) -> Packet {
        let spot = [at.0, at.1, self.ground_z(at)];
        let time = (self.run.now * 1000.0) as u32;
        match r.how {
            AllyMove::Jump => packets::relayed_move(opcode::MSG_MOVE_FALL_LAND, guid, 0, time, spot, facing, (r.seconds * 1000.0) as u32, None),
            _ => packets::relayed_move(opcode::MSG_MOVE_STOP_STRAFE, guid, 0, time, spot, facing, 0, None),
        }
    }

    fn note_ally_look(&mut self, id: u32, spell: u32) {
        if let Some(a) = self.allies.allies.get(&id) {
            if spell != 0 {
                self.allies.last_look.insert(a.character.clone(), spell);
            }
        }
    }


    /// What an event of an ally shows (in its turn among the tick's events).
    pub(super) fn present_ally_event(&mut self, ev: &Event, all: &mut Vec<Packet>) {
        match ev {
            // a charge or a blink plays its look as it goes (the move itself is its segment's)
            Event::AllyMoved { id, look_spell, how, .. } => {
                let Some(guid) = self.ally_guid_shown(*id) else { return };
                let look = match how {
                    esurv_sim::game::assists::AllyMove::Blink => Some(look_spell.unwrap_or(BLINK_LOOK)),
                    _ => *look_spell,
                };
                if let Some(look) = look.filter(|&l| l != 0) {
                    all.push(packets::spell_go(guid, look, &[], SpellTarget::Caster, None));
                }
            }
            // a rotation spell with a cast time: its cast bar and the caster's cast; it goes off
            // as a spell go
            Event::AllyCastStarted { id, spell, seconds, aim } => {
                let Some(guid) = self.ally_guid_shown(*id) else { return };
                let target = self.spell_target(aim.clone()).unwrap_or(SpellTarget::Caster);
                all.push(packets::spell_start(guid, *spell, (seconds * 1000.0) as u32, target));
            }
            // an enemy taunted by the ally strikes it, and it takes nothing
            Event::AllyStruck { id, by } => {
                if let (Some(ally), Some(enemy)) = (self.ally_guid_shown(*id), self.guid_of(Unit::Enemy(*by))) {
                    all.push(packets::attacker_state_absorbed(enemy, ally));
                }
            }
            Event::AllyLeft { id, how, look_spell, to } => self.ally_leaves(*id, *how, *look_spell, *to, all),
            _ => {}
        }
    }

    /// Ally `id` leaves its way: fading where it stands, vanishing, hearthing, running off to
    /// `to`, or dying; it goes from the clients once that is done.
    fn ally_leaves(&mut self, id: u32, how: Leave, look_spell: Option<u32>, to: Option<Vec2>, all: &mut Vec<Packet>) {
        let now = self.run.now;
        let Some(s) = self.allies.allies.get(&id) else { return };
        let (guid, segment) = (s.guid, s.segment);
        let here = segment.position(now);
        let look = look_spell.filter(|&l| l != 0);
        eprintln!("esurv: assist {} leaves ({how:?}) at {:.1} s", s.name, now);
        let gone_in = match how {
            Leave::Fade => {
                let fields = present::aura_fields(&[Aura { spell: STEALTH_LOOK, stacks: 1, until: 0.0 }], 0, 0);
                all.push(update_object(&[ObjectUpdate::Values { guid, fields }]));
                if let Some(l) = look {
                    all.push(packets::spell_go(guid, l, &[guid], SpellTarget::Unit(guid), None));
                }
                FADE_OUT
            }
            Leave::Vanish => {
                all.push(packets::spell_go(guid, look.unwrap_or(VANISH_LOOK), &[], SpellTarget::Caster, None));
                VANISH_GONE
            }
            Leave::Hearth => {
                let spell = look.unwrap_or(HEARTH_LOOK);
                all.push(packets::spell_start(guid, spell, (HEARTH_CAST * 1000.0) as u32, SpellTarget::Caster));
                self.later.push((now + HEARTH_CAST, packets::spell_go(guid, spell, &[], SpellTarget::Caster, None)));
                HEARTH_CAST + VANISH_GONE
            }
            Leave::RunAway => {
                if let Some(l) = look {
                    all.push(packets::spell_go(guid, l, &[], SpellTarget::Caster, None));
                }
                let to = to.unwrap_or(here);
                let seconds = dist(here, to) / ALLY_RUN_SPEED;
                let run = Segment { from: here, to, start: now, duration: seconds };
                all.push(self.walk(guid, &run));
                seconds
            }
            Leave::Dies => {
                if let Some(l) = look {
                    all.push(packets::spell_go(guid, l, &[guid], SpellTarget::Unit(guid), None));
                }
                if segment.moving(now) {
                    all.push(self.walk(guid, &Segment::standing(here, now)));
                }
                let mut f = Fields::new();
                f.u32(field::UNIT_HEALTH, 0);
                all.push(update_object(&[ObjectUpdate::Values { guid, fields: f }]));
                BODY_LIES
            }
        };
        if let Some(s) = self.allies.allies.get_mut(&id) {
            s.gone_at = Some(now + gone_in);
        }
    }

    /// The allies whose leaving is done go from the clients.
    pub(super) fn allies_gone(&mut self) -> Vec<Packet> {
        let now = self.run.now;
        let live: Vec<u32> = self.run.assists.allies.iter().map(|a| a.id).collect();
        let done: Vec<u32> = self
            .allies
            .allies
            .iter()
            // one the run let go of without a leave (a fresh run) goes at once
            .filter(|(id, a)| a.gone_at.map_or(!live.contains(id), |t| now >= t))
            .map(|(id, _)| *id)
            .collect();
        done.into_iter().filter_map(|id| self.allies.allies.remove(&id)).map(|a| packets::destroy_object(a.guid)).collect()
    }

    /// An ally's hit on enemy `enemy` (guid `target`) for `amount`: its swing when it swung at
    /// it this tick, else under the look its character last cast (from the ally, or from the hero
    /// once the ally has gone: a ground area or a bleed it left).
    pub(super) fn assist_hit(&mut self, character: &str, enemy: u32, target: u64, amount: u32, hero: Option<u64>) -> Option<Packet> {
        let ally = self.ally_of(character).map(|a| (a.guid, self.allies.allies.iter().find(|(_, s)| s.guid == a.guid).map(|(id, _)| *id)));
        if let Some((guid, Some(id))) = ally {
            if let Some(k) = self.allies.swings.iter().position(|&(a, e)| a == id && e == enemy) {
                self.allies.swings.swap_remove(k);
                return Some(packets::attacker_state(guid, target, amount, false));
            }
        }
        let look = *self.allies.last_look.get(character)?;
        let caster = ally.map(|(g, _)| g).or(hero)?;
        Some(packets::spell_damage_log(target, caster, look, amount, present::school_id(esurv_content::School::Physical), false))
    }

    /// Every ally the clients know of, its name and create (for a player who arrives).
    pub(super) fn allies_now(&self) -> (Vec<Packet>, Vec<ObjectUpdate>) {
        let (mut names, mut creates) = (Vec::new(), Vec::new());
        let content = &self.run.content;
        for a in &self.run.assists.allies {
            let (Some(s), Some(c)) = (self.allies.allies.get(&a.id), content.player_characters.iter().find(|c| c.id == a.character)) else {
                continue;
            };
            let Some(shape) = self.character_shape(c, None, a.id) else { continue };
            names.push(packets::name_query_response(s.guid, &shape.name, shape.race, shape.gender, shape.class));
            let at = a.segment.position(self.run.now);
            let race = s.race.map(|r| content.races[r].faction_template).unwrap_or(super::pickups::PICKUP_FACTION);
            let mut f = Fields::new();
            f.u64(field::OBJECT_GUID, s.guid)
                .u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_UNIT | TYPE_PLAYER)
                .f32(field::OBJECT_SCALE_X, content.balance.battle_standard.ally_scale);
            f.u32(field::UNIT_HEALTH, ALLY_HEALTH).u32(field::UNIT_MAXHEALTH, ALLY_HEALTH).u32(field::UNIT_LEVEL, 1);
            f.u32(field::UNIT_FACTIONTEMPLATE, race).u32(field::UNIT_FLAGS, ALLY_UNIT_FLAGS);
            f.f32(field::UNIT_BOUNDINGRADIUS, PLAYER_BOUNDING_RADIUS).f32(field::UNIT_COMBATREACH, PLAYER_COMBAT_REACH);
            shape.dress(&mut f);
            let living = Living { position: [at.0, at.1, self.ground_z(at)], facing: a.facing, speeds: Speeds::running_at(ALLY_RUN_SPEED) };
            creates.push(ObjectUpdate::Create { guid: s.guid, type_id: ObjectTypeId::Player, living, fields: f, own: false });
        }
        (names, creates)
    }

    /// Every ally goes from the clients (a fresh run).
    pub(super) fn forget_allies(&mut self) -> Vec<Packet> {
        let gone = self.allies.allies.drain().map(|(_, a)| packets::destroy_object(a.guid)).collect();
        self.allies = AlliesShown::default();
        gone
    }
}


#[cfg(test)]
mod tests;
