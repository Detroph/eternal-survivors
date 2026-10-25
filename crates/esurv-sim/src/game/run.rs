//! A run: the clock, the heroes, the enemies and the order things happen in each tick.
//!
//! Heroes are a list from the start (roadmap R6): enemies chase the nearest living hero, and
//! every event names the hero it concerns.

use std::sync::Arc;

use esurv_content::{Content, Milestone, Stat, TalentEffect};

use super::director::{roster_weights, Director, DirectorInput};
use super::enemy::{boss_stats, live_cap, spawn_stats, RunOptions, RUN_FACTOR};
use super::enemy_ai::{attack_damage, chase, flee, move_speed, reach, Segment, Vec2, ENRAGE_HEALTH};
use super::level::Progress;
use super::casting::{HitSource, ProcOrigin};
use super::spells::{SpellModifier, Spellbook};
use super::stats::{compute, race_modifiers, Modifier, StatSheet};
use crate::rng::Rng;

/// The longest step one tick may take (the real time between updates is clamped to it).
pub const MAX_DT: f32 = 0.1;
/// A roster enemy first decides where to go up to this many seconds after it appears (at random).
pub const FIRST_THOUGHT: f32 = 0.3;
/// A boss stands this long after it appears before it first decides where to go, and is held
/// this long (it neither moves nor swings).
pub const BOSS_FIRST_THOUGHT: f32 = 1.0;
pub const BOSS_ARRIVAL_HOLD: f32 = 1.5;
/// In Endless, once every boss of the schedule has come, the final boss comes back: each tick in
/// the second before the next swarm, with this chance.
pub const ENDLESS_BOSS_CHANCE: f32 = 0.002;
pub const ENDLESS_BOSS_WINDOW: f32 = 1.0;
/// Tries at finding a spawn spot whose ground is near the hero's height.
pub const SPAWN_TRIES: u32 = 12;
pub const SPAWN_HEIGHT_TOLERANCE: f32 = 25.0;
/// A spawn spot that does not fit (a wall or map edge in the way) is moved this many yards
/// toward the hero at a time, along its direction, before a new direction is drawn.
pub const SPAWN_STEP_IN: f32 = 2.5;

/// The nearest a stepped-in spawn may come to the hero: half the ring's inner edge
/// (15 yd for a 30-40 ring).
pub fn spawn_inner_floor(ring: (f32, f32)) -> f32 {
    ring.0 / 2.0
}
/// A hero may roam this far past the arena radius before being sent back.
pub const EDGE_SLACK: f32 = 10.0;
/// Where a stray hero is put back: this far inside the slack.
pub const EDGE_RETURN_INSET: f32 = 4.0;
/// A hero this far below the ground is put back on it.
pub const FALL_LIMIT: f32 = 15.0;
/// The first move longer than this starts the run.
pub const START_MOVE: f32 = 0.2;
/// An extra life brings the hero back with this share of max health...
pub const EXTRA_LIFE_HEALTH: f32 = 0.5;
/// ...and blasts enemies this close for hero damage × this.
pub const EXTRA_LIFE_BLAST: (f32, f32) = (12.0, 200.0);

/// In a dungeon a spawn spot, drawn in the ring as anywhere, must be walked to from the hero
/// within this many times the ring's far edge: up a ramp to another level counts, a balcony with
/// no way down does not (a walk held to the ring itself would leave the other
/// level nearly empty).
pub const SPAWN_WALK_REACH: f32 = 2.0;
/// Spawn sight runs between points this high over the hero's and the spot's floors, about chest
/// height.
pub const SIGHT_HEIGHT: f32 = 1.5;

/// What the run needs from the game world: ground heights (from the map), whether a line is clear
/// and how fast a WoW spell's missile flies (`Spell.dbc` Speed; 0 = it hits at once). Tests use
/// `Flat`.
pub trait World: Send + Sync {
    fn height(&self, x: f32, y: f32) -> Option<f32>;
    /// The floor at `(x, y)` nearest below `near_z` (plus a step), for maps whose floors stack
    /// (a dungeon's halls over its cellars); terrain has one ground, so by default it is
    /// [`World::height`].
    fn floor(&self, x: f32, y: f32, _near_z: f32) -> Option<f32> {
        self.height(x, y)
    }
    /// Whether nothing blocks sight from `from` to `to`; open terrain never does.
    fn clear_line(&self, _from: [f32; 3], _to: [f32; 3]) -> bool {
        true
    }
    /// How a body at `from` (feet) walks to `to`, searched out to `reach` yards: open terrain is
    /// walked straight, a dungeon over its floors.
    fn walk(&self, _from: [f32; 3], _to: Vec2, _reach: f32) -> Walk {
        Walk::Straight
    }
    /// Where a body at `from` heads next to reach `hero` (feet) when the walk is not straight:
    /// a point along the way round a wall or up a ramp; `None` goes straight at the hero, as on
    /// open terrain always.
    fn toward(&self, _from: Vec2, _hero: [f32; 3]) -> Option<Vec2> {
        None
    }
    fn missile_speed(&self, _spell: u32) -> f32 {
        0.0
    }
    /// The lines nobody walks or sees across from now on (a zone's walls and its shut doors):
    /// walks, sight and the walls' flow fields go round them. Open terrain has none.
    fn set_blocked(&self, _lines: &[(Vec2, Vec2)]) {}
    /// Floor laid over holes, (centre, radius, height) discs: walks, spawns and the hero stand
    /// on them. Open terrain needs none.
    fn set_platforms(&self, _discs: &[(Vec2, f32, f32)]) {}
}

/// Whether a walk from `a` to `b` crosses the line `l` (touching it counts).
pub fn crosses(a: Vec2, b: Vec2, l: (Vec2, Vec2)) -> bool {
    let side = |p: Vec2, q: Vec2, r: Vec2| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2) = (side(l.0, l.1, a), side(l.0, l.1, b));
    let (d3, d4) = (side(a, b, l.0), side(a, b, l.1));
    (d1 != 0.0 || d2 != 0.0) && d1 * d2 <= 0.0 && d3 * d4 <= 0.0
}

/// How a body gets from one spot to another (`World::walk`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Walk {
    /// Open ground: a straight line.
    Straight,
    /// Over a dungeon's floors, `distance` yards on foot to the floor at `z`.
    Reached { distance: f32, z: f32 },
    /// No walk within the reach asked: another level with no way there, or too far round.
    Unreachable,
}

/// Flat ground at height 0, every spell instant.
pub struct Flat;

impl World for Flat {
    fn height(&self, _x: f32, _y: f32) -> Option<f32> {
        Some(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// "Move to begin": nothing happens until a hero moves.
    Waiting,
    Running,
    /// A level-up or cache choice is open; the run is paused.
    Choosing,
    Paused,
    Defeat,
    Victory,
}

/// What one hero brings into a run.
#[derive(Debug, Clone, Default)]
pub struct HeroSetup {
    pub class: String,
    pub race: String,
    /// The starting option picked (`StartingOption::id`); `None` or one the class lacks: its first.
    pub start: Option<String>,
    /// The weapon loadout picked (`Loadout::id`, `loadouts.ron`); `None` or one the class lacks:
    /// its first. Its effects are added as the run begins.
    pub loadout: Option<String>,
    /// Stat modifiers from talents, gear, jewelry and the amulet (race passives are added here).
    pub modifiers: Vec<Modifier>,
    pub spell_modifiers: Vec<SpellModifier>,
    pub milestones: Vec<Milestone>,
    /// What gave the hero each of `milestones` (by index; one missing is `ProcOrigin::Other`).
    pub milestone_origins: Vec<ProcOrigin>,
}

impl HeroSetup {
    /// A hero of `class` and `race` with what its class has earned: the talent ranks bought (by
    /// talent id; a rank past the talent's last counts as its last) and the class gear worn (item
    /// entries; another class's piece or an unknown entry is left off).
    pub fn with_progress<'t>(content: &Content, class: &str, race: &str, talents: impl IntoIterator<Item = (&'t str, u32)>, worn: &[u32]) -> Self {
        let mut setup = HeroSetup { class: class.into(), race: race.into(), ..Default::default() };
        let known: Vec<&esurv_content::Talent> = content.talent_trees.iter().filter(|t| t.class == class).flat_map(|t| &t.talents).collect();
        let bought: Vec<(&esurv_content::Talent, u32)> = talents
            .into_iter()
            .filter(|&(_, rank)| rank > 0)
            .filter_map(|(id, rank)| known.iter().find(|t| t.id == id).map(|t| (*t, rank.min(u32::from(t.max_rank)))))
            .collect();
        for &(t, rank) in &bought {
            setup.add_effects_of(&ProcOrigin::Talent(t.id.clone()), t.effects.iter().map(|e| (e, rank)));
        }
        if let (Some(set), Some(c)) = (content.gear_set(class), content.classes.iter().find(|c| c.id == class)) {
            for &entry in worn {
                let Some((wow_class, slot, zone)) = super::gear::decode_item_entry(entry) else { continue };
                if wow_class != c.wow_class_id {
                    continue;
                }
                if let Some(m) = super::gear::piece_modifiers(content, set, slot, zone) {
                    setup.modifiers.extend(m);
                }
            }
        }
        setup
    }

    /// Adds what the class's prestige ranks give (by prestige stat id),
    /// as its talents' effects are added.
    pub fn add_prestige(&mut self, content: &Content, ranks: &super::prestige::PrestigeRanks) {
        let class = self.class.clone();
        self.add_effects(super::prestige::effects(content, &class, ranks));
    }

    /// Splits talent and amulet effects (with their ranks) into the three kinds of modifier.
    pub fn add_effects<'a>(&mut self, effects: impl IntoIterator<Item = (&'a TalentEffect, u32)>) {
        self.add_effects_of(&ProcOrigin::Other, effects);
    }

    /// As `add_effects`, each milestone given by `origin` (the meter names its procs after it).
    pub fn add_effects_of<'a>(&mut self, origin: &ProcOrigin, effects: impl IntoIterator<Item = (&'a TalentEffect, u32)>) {
        for (e, rank) in effects {
            match e {
                TalentEffect::Stat { stat, per_rank } => self.modifiers.push(Modifier::new(*stat, per_rank * rank as f32)),
                TalentEffect::AbilityMod { .. } => self.spell_modifiers.extend(SpellModifier::from_effect(e, rank)),
                TalentEffect::Milestone(m) => {
                    let mut m = m.clone();
                    if let Some(v) = m.rank_value_mut() {
                        *v *= rank as f32;
                    }
                    // the origins line up with the milestones, whatever was pushed by hand before
                    self.milestone_origins.resize(self.milestones.len(), ProcOrigin::Other);
                    self.milestones.push(m);
                    self.milestone_origins.push(origin.clone());
                }
            }
        }
    }
}

/// How much a blocked melee blow takes off its damage.
pub const BLOCK_REDUCTION: f32 = 0.3;

/// Kinds of timed buff on a hero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffKind {
    /// An absorb shield is up.
    Shield,
    Damage,
    Haste,
    /// Nothing hurts the hero.
    Immunity,
    /// A thorns-like aura.
    Aura,
    /// A heal over time: `value` health per second.
    Regen,
    /// The Speed Rune: + `value` move speed.
    Speed,
    /// The Berserker Rune: + `value` damage.
    Berserk,
    /// Healing charges (`bloodthirst`): each of the hero's next weapon
    /// swings or spell casts that hit heals `value` × max health; `Hero::healing_charges` are left.
    HealingCharges,
    /// The hero's attacks also strike `value` more enemies (`Mechanic::AttacksStrikeMore`).
    ExtraStrikes,
    /// Weapon swings `value` faster (a spell's `SpellKit::swing_speed`, Slice and Dice).
    /// It carries its spell's visual and goes with the spell's other buff.
    SwingSpeed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Buff {
    pub kind: BuffKind,
    pub value: f32,
    /// WoW spell id of its visual.
    pub visual: u32,
    pub until: f32,
}

#[derive(Debug, Clone)]
pub struct Hero {
    pub id: u32,
    pub class: usize,
    pub race: usize,
    /// The starting option it plays (`StartingOption::id`).
    pub start: String,
    /// The weapon loadout it fights with (`Loadout::id`; empty for a class without loadouts).
    pub loadout: String,
    pub pos: Vec2,
    pub height: f32,
    /// Facing in radians (cones and totems use it).
    pub facing: f32,
    /// Everything that modifies the hero now (rebuilt from the sources below and what the run
    /// added: jewelry and the amulet).
    pub modifiers: Vec<Modifier>,
    pub spell_modifiers: Vec<SpellModifier>,
    pub milestones: Vec<Milestone>,
    /// What gave the hero each of `milestones` (by index; one missing is `ProcOrigin::Other`).
    pub milestone_origins: Vec<ProcOrigin>,
    /// What the hero brought into the run: race, talents, gear.
    pub base_modifiers: Vec<Modifier>,
    pub base_spell_modifiers: Vec<SpellModifier>,
    pub base_milestones: Vec<Milestone>,
    pub base_milestone_origins: Vec<ProcOrigin>,
    pub jewelry: Vec<super::choices::WornJewelry>,
    pub amulet: Option<super::choices::WornAmulet>,
    /// The amulet the first cache this hero opens is sure to offer (the balance check's
    /// legendary measure); taken off as that cache opens.
    pub first_cache_amulet: Option<esurv_content::Amulet>,
    pub rerolls_free: u32,
    pub rerolls_paid: u32,
    /// Gear won from bosses this run.
    pub loot: Vec<GearDrop>,
    /// Seconds until the weapon swing (classes with one): the main hand's (or the ranged one's).
    pub swing_timer: f32,
    /// Seconds until the off hand's swing (a loadout of two weapons that both swing).
    pub off_swing_timer: f32,
    /// What each milestone remembers (counters, cooldowns, stacks), by milestone index.
    pub milestone_state: Vec<super::milestones::MilestoneState>,
    pub elites_killed: u32,
    pub rares_killed: u32,
    pub caches_opened: u32,
    /// When the hero fell for good (run seconds).
    pub fell_at: Option<f32>,
    pub buffs: Vec<Buff>,
    /// Healing charges left on the hero's `BuffKind::HealingCharges` buff (none without it).
    pub healing_charges: u32,
    /// The cast (`Run::casts_made` when it was cast) that last used or granted a healing charge:
    /// one cast uses at most one, however many enemies it hits.
    pub charged_cast: u64,
    pub stats: StatSheet,
    pub health: f32,
    pub shield: f32,
    /// Damage the current shield has absorbed so far.
    pub absorbed: f32,
    pub progress: Progress,
    pub gold: u32,
    pub spells: Spellbook,
    /// The starting option's resource, counter and stacks.
    pub kit: super::kit::KitState,
    pub extra_lives: u32,
    /// When cheat death may save the hero again.
    pub cheat_death_ready: f32,
    pub alive: bool,
    pub damage_taken: f64,
    pub damage_dealt: f64,
    pub kills: u32,
    pub gold_earned: u32,
    pub magnets_taken: u32,
    /// Caches paid for whose choice is not yet made.
    pub pending_caches: u32,
    /// Rares' caches taken whose choice is not yet made.
    pub pending_rare_caches: u32,
    /// Level-up choices earned and not yet made.
    pub pending_choices: u32,
    /// A teleport back into the arena not yet confirmed by the client, and the reports left to
    /// ignore until it is.
    pub pending_teleport: Option<(Vec2, u8)>,
    /// Stacking effects bosses left on it (`game::ground_zones`; damage only).
    pub afflictions: Vec<super::ground_zones::HeroAffliction>,
}

/// A class gear piece a boss dropped this run (it joins the hero's collection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GearDrop {
    pub slot: u32,
    /// The tier of the zone it dropped in: its quality colour.
    pub tier: u32,
    /// The item entry the client shows (it names the zone).
    pub entry: u32,
}

/// Which kind of enemy a unit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind {
    /// An entry of the zone roster.
    Roster(usize),
    /// A boss of the zone's schedule.
    Boss(usize),
    /// A rare: a stronger copy of roster elite `roster`.
    Rare { roster: usize },
}

#[derive(Debug, Clone)]
pub struct EnemyUnit {
    pub id: u32,
    pub kind: EnemyKind,
    pub segment: Segment,
    pub next_decision: f32,
    pub aim_angle: f32,
    pub health: f32,
    pub max_health: f32,
    pub damage: f32,
    pub base_speed: f32,
    pub attack_interval: f32,
    pub swing_timer: f32,
    pub collision_radius: f32,
    pub ranged_range: Option<f32>,
    /// A ranged caster's spell (its look and missile speed) and how long it stands casting.
    pub ranged_spell: Option<u32>,
    pub cast_time: f32,
    /// The share of `damage` its cast shot hits for (a caster boss's half;
    /// 1 for any other).
    pub shot_share: f32,
    /// Casting until then; the shot leaves when it ends.
    pub casting_until: Option<f32>,
    /// Rushing in a charge until then (no stopping, no swings).
    pub charging_until: f32,
    /// A boss's or rare's abilities.
    pub abilities: Vec<super::enemy_attacks::AbilitySlot>,
    /// No ability before then (the gap between two ability casts).
    pub next_ability_at: f32,
    pub slow: Option<(f32, f32)>,
    pub weaken: Option<(f32, f32)>,
    pub fear_until: f32,
    /// Silenced until then: no ranged shot, no ability (`AfflictEffect::Silence`).
    pub silenced_until: f32,
    /// A root or stun holds it in place until then.
    pub hold_until: f32,
    /// When the current chain of holds began (a chain lasts at most 3 s).
    pub hold_chain_start: f32,
    /// No new hold takes until then.
    pub hold_immune_until: f32,
    /// Takes this much more damage from everything, until then (milestone Vulnerable).
    pub vulnerable: Option<(f32, f32)>,
    pub dots: Vec<super::casting::Dot>,
    /// What the starting options' rules left on it (marks, freezes).
    pub kit: super::kit::EnemyKit,
    pub elite: bool,
    /// Picked as an elite is (`Enemy::focus_as_elite`), its numbers its own kind's.
    pub focus_as_elite: bool,
    pub enraged: bool,
    pub spawn_cost: f32,
    /// What a boss's script and building blocks keep on it (`game::boss_scripts`).
    pub mech: super::boss_scripts::BossState,
}

impl EnemyUnit {
    pub fn position(&self, now: f32) -> Vec2 {
        self.segment.position(now)
    }

    pub fn is_boss(&self) -> bool {
        matches!(self.kind, EnemyKind::Boss(_))
    }

    pub fn is_rare(&self) -> bool {
        matches!(self.kind, EnemyKind::Rare { .. })
    }

    /// An elite, or one picked as an elite (`Enemy::focus_as_elite`): what targeting ranks as elite.
    pub fn ranks_as_elite(&self) -> bool {
        self.elite || self.focus_as_elite
    }

    pub fn active(effect: Option<(f32, f32)>, now: f32) -> Option<f32> {
        effect.filter(|(_, until)| now < *until).map(|(v, _)| v)
    }
}

/// A unit an event shows, by its simulation id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Hero(u32),
    Enemy(u32),
    Pet(u32),
    Totem(u32),
    /// An ally a battle standard summoned (`game::assists`), by its id in `Event::AllySummoned`.
    Ally(u32),
}

/// Where a spell visual is aimed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Aim {
    Unit(Unit),
    Ground(Vec2),
    /// At the caster itself (novas, cones, buffs, totems).
    Caster,
}

/// Who a centre-screen line names (`%s` in its text).
#[derive(Debug, Clone, PartialEq)]
pub enum Speaker {
    Unit(Unit),
    Named(String),
}

/// A ranged class shot carries an arrow: `auto_shot`, `multi_shot`, `steady_shot` and `aimed_shot`
/// (Aimed Shot's rank 1 and 2: the two spells wear different ranks, so the bar tells them apart).
pub const AMMO_SPELLS: [u32; 4] = [75, 2643, 19434, 20900];
/// The extra life: the hero comes back (`reincarnation`'s look) and the blast (`holy_nova`'s).
pub const EXTRA_LIFE_VISUAL: u32 = 21169;
pub const EXTRA_LIFE_BLAST_VISUAL: u32 = 15237;
/// Cheat death: `stoneform`'s look, or Vanish's when it makes the hero immune.
pub const CHEAT_DEATH_VISUAL: u32 = 20594;
pub const CHEAT_DEATH_IMMUNE_VISUAL: u32 = 1856;

/// What happened during a tick, for the server to show and for tests.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    RunStarted,
    EnemySpawned { id: u32, roster: usize, at: Vec2 },
    BossSpawned { id: u32, boss: usize, at: Vec2 },
    EnemyDied { id: u32, by: Option<u32> },
    /// A hero spell went off (for the client's cast visual).
    SpellCast { hero: u32, spell: String },
    /// A hero in a form left it for `seconds` to cast a caster spell, then shifts back into the
    /// form `look_spell` (a powershift).
    FormLeft { hero: u32, seconds: f32, look_spell: u32 },
    EnemyHit { id: u32, amount: f32, crit: bool, by: Option<u32>, source: super::casting::HitSource },
    /// Hero `hero`'s next weapon-swing hit is its off hand's (dual wield: the client plays the
    /// off-hand swing).
    OffHandSwing { hero: u32 },
    /// `spell`: the `Spell.dbc` id of what healed (the client shows a heal line), or 0.
    HeroHealed { hero: u32, amount: f32, spell: u32 },
    PickupDropped { id: u32, kind: super::loot::Pickup, value: u32, at: Vec2 },
    PickupTaken { id: u32, hero: u32 },
    LevelUp { hero: u32, level: u32 },
    CacheOpened { hero: u32, price: u32 },
    PetSummoned { id: u32, hero: u32, pet: String, at: Vec2 },
    PetGone { id: u32 },
    /// A boss dropped a class gear piece for this hero (the "dropped" pop-up).
    GearDropped { hero: u32, slot: u32, tier: u32, entry: u32 },
    /// A rare came: a copy of roster elite `roster`.
    RareSpawned { id: u32, roster: usize, at: Vec2 },
    ScreenOpened { hero: u32, kind: super::choices::ScreenKind, offers: Vec<super::choices::Offer> },
    OfferTaken { hero: u32, offer: super::choices::Offer },
    /// `amount`: after damage reduction, before any shield; `lost`: what it took off the hero's
    /// health (after any shield; the damage-taken meter counts it). `melee`: an enemy's melee
    /// swing (the client shows the swing).
    HeroHit { hero: u32, amount: f32, lost: f32, by: Option<u32>, melee: bool },
    HeroTeleported { hero: u32, to: Vec2, height: f32, left_arena: bool },
    CheatedDeath { hero: u32 },
    ExtraLifeUsed { hero: u32 },
    HeroDied { hero: u32 },
    /// A message for the HUD (a toast).
    Announce(String),
    /// Door `door` of the zone swings open.
    DoorOpened { door: usize },
    /// Door `door` of the zone swings shut again (a script).
    DoorShut { door: usize },
    /// A raid warning across the middle of the screen.
    RaidWarning(String),
    /// A red notice across the screen for one hero (the client's error frame).
    Notice { hero: u32, text: String },
    /// A system line in one hero's chat.
    Told { hero: u32, text: String },
    /// A sound for everyone (`SoundEntries.dbc` id).
    Sound(u32),
    /// A spell's look goes off: `spell` (a `Spell.dbc` id) from `caster`, striking `hits`,
    /// aimed at `aim`; `ammo` for a ranged shot that flies an arrow; `reach`: how many yards the
    /// look covers when it is an area (its radius, a cone's length), else 0; `triggered` for a
    /// proc's look (`show_proc`), whose missile and impact play while the caster shows no cast,
    /// as a triggered spell in WoW.
    SpellGo { caster: Unit, spell: u32, hits: Vec<Unit>, aim: Aim, ammo: bool, reach: f32, triggered: bool },
    /// A hero spell was cast and waits `seconds` before the next (the action bar's sweep).
    SpellCooldown { hero: u32, spell: String, seconds: f32 },
    /// A card taught the hero a new spell.
    SpellLearned { hero: u32, spell: String },
    /// A hero's mana bar or combo points changed: `kind` (rage, energy, combo points, a
    /// passive's counter) is now `value` of `max`.
    ResourceChanged { hero: u32, kind: esurv_content::ResourceKind, value: f32, max: f32 },
    /// A hero's passive went off (`Passive::id`: a full counter released, a proc).
    PassiveTriggered { hero: u32, passive: String },
    /// A summon of hero `hero` appeared; when it shows a pet's model, `PetSummoned` with the same
    /// id comes just before (and `PetGone` just before `SummonGone`). It leaves at `until` (run
    /// seconds), or never for `None`.
    SummonAppeared { id: u32, hero: u32, kind: esurv_content::SummonKind, at: Vec2, until: Option<f32> },
    SummonGone { id: u32 },
    /// A stacking buff on a hero (or its pets) is now at `stacks` of `max`, lasting to `until`
    /// (run seconds; `None`: until used); 0 stacks: gone.
    StacksChanged { hero: u32, kind: esurv_content::StackKind, stacks: u32, max: u32, until: Option<f32> },
    /// Hero `hero` blocked enemy `by`'s melee blow (it did no damage).
    Blocked { hero: u32, by: u32 },
    /// A ranged enemy starts casting `spell` at a hero for `seconds`.
    EnemyCastStarted { id: u32, spell: u32, seconds: f32, hero: u32 },
    /// A spell's lasting look on the ground (a lasting area, or a boss's marked spot): `spell`
    /// from `caster`, `radius` yards around `at`; it goes at `until` (run seconds), or on
    /// `AreaGone` when that is `None`.
    AreaShown { id: u32, caster: Unit, spell: u32, at: Vec2, radius: f32, until: Option<f32> },
    AreaGone { id: u32 },
    /// A lasting area's look moved to `at` or grew to `radius` (an area that follows its hero, or
    /// grows as it strikes); its spell and time stay.
    AreaChanged { id: u32, at: Vec2, radius: f32 },
    /// A unit shouts a line.
    Yell { id: u32, text: String },
    /// A line across the middle of the screen; `%s` in it stands for the speaker's name.
    Emote { from: Speaker, text: String },
    /// A boss's marked attack is laid: `shape` at `at` facing `facing`,
    /// drawn with `warning_spell` over it and `fill_spell` filling it over `wind_up` seconds.
    TelegraphShown { id: u32, shape: esurv_content::Shape, at: Vec2, facing: f32, warning_spell: u32, fill_spell: u32, wind_up: f32 },
    /// Boss `boss` winds up an ability for `seconds` (a mark round itself or at a hero, a rush, a
    /// ground burst): `label` its warning's bar label (empty for none), `spell` its icon's spell
    /// (and its name when it has no label). Its boss bar shows it as a cast.
    BossCasting { boss: u32, label: String, spell: u32, seconds: f32 },
    /// It landed (its looks go).
    TelegraphGone { id: u32 },
    /// An object is put on the ground for a hero to walk over: its name, model
    /// (`GameObjectDisplayInfo.dbc`), size, the glow under it and how near a hero stands on it.
    ObjectPlaced { id: u32, name: String, display_id: u32, scale: f32, at: Vec2, glow_spell: u32, radius: f32 },
    /// A hero has stood on it for this share of its contact (0: stepped off).
    ObjectContact { id: u32, progress: f32 },
    /// It went: broken or set off by a hero (`taken`), or hatched.
    ObjectGone { id: u32, taken: bool },
    /// A boss kneels (or stands up again).
    BossKneels { id: u32, kneeling: bool },
    /// A boss's weapons or model changed (a phase change): the server reads them again.
    BossChanged { id: u32 },
    /// The scripted pause begins (the client freezes its hero) and ends.
    PauseBegan { seconds: f32, text: String },
    PauseEnded,
    /// A raid warning for a boss mechanic: `text` in `colour` with
    /// `icon_spell`'s icon; `soon` for an early warning.
    Warning { text: String, colour: esurv_content::WarningColour, icon_spell: u32, soon: bool },
    /// A boss's timer bar `key` (an ability or script line) counts down to `until` (run seconds).
    BarStarted { boss: u32, key: u32, label: String, icon_spell: u32, until: f32 },
    /// Every bar of boss `boss` goes (it died, or changed its kit).
    BarsCleared { boss: u32 },
    /// A boss look that shows only on a unit it hits plays on an invisible target unit put at `at`
    /// (a "bunny"), kept `linger` seconds (the full look where a mark
    /// lands, the hero there or not). No one is hurt by it.
    ImpactAtSpot { caster: Unit, spell: u32, at: Vec2, linger: f32 },
    /// A line strikes with runners (`LineHit`'s `runners`, Stampede's beasts): a row of them
    /// sets off behind `from` (the hero) across `width` yards either side of the line and runs to
    /// `to` (its end), as `runners` says. A look only: no one is hurt by it.
    LineRunners { from: Vec2, to: Vec2, width: f32, runners: esurv_content::LineRunners },
    /// A `StrikeOne` with `reflection` (Blink Strike): a reflection of hero `hero` appears at
    /// enemy `target`, strikes it with look `spell` and fades. A look only; the hero stays put.
    ReflectionStrikes { hero: u32, target: u32, spell: u32 },
    /// Spirits set off with lasting area `area` (`LeaveArea`'s `spirits`): they run beside hero
    /// `hero` until `until`. A look only.
    SpiritsAppeared { area: u32, hero: u32, until: f32, spirits: esurv_content::AreaSpirits },
    /// Area `area`'s spirits strike, one enemy each of `targets` (what the area struck).
    SpiritsStrike { area: u32, targets: Vec<u32> },
    /// A boss's counter `key` reads `label` (the adds it waits on: "Guards left 5/8"); a bar
    /// that does not count down.
    CounterShown { boss: u32, key: u32, label: String },
    /// A battle standard summoned an ally for hero `hero` (`game::assists`, docs/game/assists.md):
    /// hand-made player character `character` (its id in `player_characters.ron`) appears at `at`
    /// facing `facing`, drawn `scale` times its size, arriving `how` with `look_spell` played on
    /// it. Its `yell` (if any) follows as a `Yell` with the same id. It is `Unit::Ally(id)` in
    /// every other event.
    AllySummoned { id: u32, hero: u32, character: String, at: Vec2, facing: f32, how: esurv_content::Appear, look_spell: Option<u32>, scale: f32 },
    /// The ally goes from where it stands to `to`, facing `facing`, there at run second `until`
    /// (`Blink` and `Step`: at once); `look_spell` played on it as it goes (a charge's look).
    AllyMoved { id: u32, to: Vec2, facing: f32, how: super::assists::AllyMove, until: f32, look_spell: Option<u32> },
    /// The ally begins casting rotation spell `spell` (a `Spell.dbc` id) for `seconds`, aimed at
    /// `aim`; it goes off as a `SpellGo` from `Unit::Ally(id)` when the cast is done.
    AllyCastStarted { id: u32, spell: u32, seconds: f32, aim: Aim },
    /// The ally strikes enemy `target` in melee (the client plays its swing). Its spell looks
    /// come as `SpellGo` with `caster: Unit::Ally(id)`.
    AllySwing { id: u32, target: u32 },
    /// The ally's taunt holds (`true`: the taunted enemies come at it), or no longer does.
    AllyTaunting { id: u32, taunting: bool },
    /// Enemy `by`, taunted, strikes the ally (it takes no damage).
    AllyStruck { id: u32, by: u32 },
    /// The ally left something that stays after it goes: `unit` is a `Unit::Pet` (just after its
    /// `PetSummoned`) or a `Unit::Totem` (in `Run::totems`). Its lasting ground areas come as
    /// `AreaShown` with `caster: Unit::Ally(id)`.
    AllyDropped { id: u32, unit: Unit },
    /// The ally goes, `how`, with `look_spell` (in place of the way's own look) played on it;
    /// `to`: where it runs off to (`Leave::RunAway`), else `None`. Its `death_yell`, if it has
    /// one, comes just before as a `Yell` with the same id.
    AllyLeft { id: u32, how: esurv_content::Leave, look_spell: Option<u32>, to: Option<Vec2> },
    /// `target` wears `spell`'s aura until run second `until` (an ally's marks on enemies, its
    /// buffs' looks on the hero, its effects over time); `from` put it there.
    AuraShown { target: Unit, spell: u32, until: f32, from: Unit },
    Defeat,
    Victory,
}

pub struct Run {
    pub content: Arc<Content>,
    pub zone: usize,
    pub options: RunOptions,
    pub world: Box<dyn World>,
    pub rng: Rng,
    pub phase: Phase,
    /// Seconds since the run started.
    pub now: f32,
    pub heroes: Vec<Hero>,
    pub enemies: Vec<EnemyUnit>,
    /// Ranged enemies' shots on their way.
    pub enemy_shots: Vec<super::enemy_attacks::EnemyShot>,
    /// Boss and rare hits that land later at a spot, with who made them.
    pub ground_hits: Vec<super::enemy_attacks::PlacedHit>,
    /// When the rares still to come are due (run seconds; `game::rares`).
    pub rares_due: Vec<f32>,
    pub pets: Vec<super::pets::PetUnit>,
    /// How deep milestone procs are nested right now.
    pub proc_depth: u32,
    /// What gave the hero the milestone going off right now (the origin its proc hits carry).
    pub proc_origin: ProcOrigin,
    /// Above 0 while a milestone's free cast (or what it left behind) is doing damage: such
    /// hits and kills set off no milestones.
    pub casting_proc: u32,
    /// While a summon casts for its hero (Call of the Ancestors' double): the hero's id and the
    /// summon. The looks of those casts come from the summon, so it is the one seen casting them
    ///.
    pub cast_by_summon: Option<(u32, Unit)>,
    /// How many hero casts there have been: each cast's number, which its hits carry.
    pub casts_made: u64,
    pub rares_slain: u32,
    /// Hero hits waiting to land (projectiles, chain steps).
    pub hits: Vec<super::casting::PendingHit>,
    pub areas: Vec<super::casting::Area>,
    /// The novas still to come of casts of novas that repeat around their hero.
    pub novas: Vec<super::casting::RepeatNova>,
    pub totems: Vec<super::casting::Totem>,
    pub pickups: Vec<super::pickups::FieldPickup>,
    /// The pick-one-of-three screen that is open (the run waits meanwhile).
    pub screen: Option<super::choices::ChoiceScreen>,
    /// Cache price and the cache, rune and magnet timers.
    pub field: super::pickups::FieldTimers,
    /// What the legendary amulets' milestone kinds left to happen later (`game::proc_effects`).
    pub procs: super::proc_effects::ProcEffects,
    pub bosses_slain: u32,
    pub director: Director,
    /// The next boss of the schedule.
    pub next_boss: usize,
    /// When the final boss died (the victory moment), if it has.
    pub won_at: Option<f32>,
    /// When Endless began, if the player chose to go on.
    pub endless_since: Option<f32>,
    /// When a final boss last died: the first one (the victory) and, in Endless, each one that
    /// comes back. Endless's growth counts from it, so a final boss slain in Endless sets it back.
    pub final_boss_fell_at: Option<f32>,
    /// Which of the zone's doors stand open.
    pub doors_open: Vec<bool>,
    /// The number the run was started with (its draws follow from it); what is picked once for a
    /// run (the characters playing its bosses) is spread from it.
    pub seed: u64,
    /// The faction the party fights as (the one most of its heroes' races share); the enemies
    /// that look like players are of the other.
    pub party_faction: esurv_content::Faction,
    /// A dev aid for shots of the ground: the orbs a passive drops (Holy Orbs) are never taken
    /// and lie until their time is up.
    pub orbs_stay: bool,
    /// Boss marks waiting to land, lasting zones, objects on the ground and those to come back
    /// (the building blocks).
    pub telegraphs: Vec<super::telegraphs::Telegraph>,
    /// Bosses running their rush lines (`game::telegraphs`).
    pub charges: Vec<super::telegraphs::ChargeRun>,
    pub ground_zones: Vec<super::ground_zones::GroundZone>,
    pub ground_objects: Vec<super::interactables::FieldObject>,
    pub objects_due: Vec<super::interactables::ObjectDue>,
    /// The spawns a boss's script stopped or narrowed.
    pub spawn_rules: super::boss_scripts::SpawnRules,
    /// A scripted pause holding the fight.
    pub cutscene: Option<super::scripted_pause::Cutscene>,
    /// Shield looks to put back on their boss when its shield returns: (boss, look, when).
    pub mortal_looks: Vec<(u32, u32, f32)>,
    /// What the later zones' script actions keep (`boss_script_actions`): actions due later,
    /// beams, where bosses fell.
    pub extras: super::boss_script_actions::ScriptExtras,
    /// Beams sweeping round their boss (`game::beams`).
    pub beams: Vec<super::beams::Beam>,
    /// Figures to rise later (`game::boss_phases`): when, which, round where.
    pub cameos_due: Vec<(f32, esurv_content::ScriptAction, Vec2)>,
    /// The attack damage the boss whose death lines are running fell with, and its id.
    /// Battle standards and the allies they summon (`game::assists`).
    pub assists: super::assists::Assists,
    pub(crate) fallen_attack: f32,
    pub(crate) fallen_boss: Option<u32>,
    next_id: u32,
    pub(crate) events: Vec<Event>,
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Run {
    /// A run in `zone` (a zone id: its file name in `content/zones/`), waiting for its first move.
    pub fn new(content: Arc<Content>, zone: &str, options: RunOptions, heroes: &[HeroSetup], ground: Box<dyn World>, seed: u64) -> Option<Self> {
        let zi = content.zones.iter().position(|z| z.id == zone)?;
        // a zone with a roster for each faction is fought with the
        // one the party's faction meets; the run keeps that zone as its own
        let factions: Vec<_> = heroes.iter().filter_map(|h| content.races.iter().find(|r| r.id == h.race)).map(|r| r.faction).collect();
        let party_faction = esurv_content::party_faction(&factions);
        let content = match content.zones[zi].by_faction {
            Some(_) => {
                let mut own = (*content).clone();
                own.zones[zi] = content.zones[zi].for_faction(party_faction);
                Arc::new(own)
            }
            None => content,
        };
        let mut run = Self {
            content: content.clone(),
            zone: zi,
            options,
            world: ground,
            rng: Rng::new(seed),
            seed,
            phase: Phase::Waiting,
            now: 0.0,
            heroes: Vec::new(),
            enemies: Vec::new(),
            enemy_shots: Vec::new(),
            ground_hits: Vec::new(),
            rares_due: Vec::new(),
            pets: Vec::new(),
            proc_depth: 0,
            proc_origin: ProcOrigin::Other,
            casting_proc: 0,
            cast_by_summon: None,
            casts_made: 0,
            rares_slain: 0,
            hits: Vec::new(),
            areas: Vec::new(),
            novas: Vec::new(),
            totems: Vec::new(),
            pickups: Vec::new(),
            screen: None,
            field: Default::default(),
            procs: Default::default(),
            bosses_slain: 0,
            director: Director::default(),
            next_boss: 0,
            won_at: None,
            endless_since: None,
            final_boss_fell_at: None,
            doors_open: vec![false; content.zones[zi].doors.len()],
            party_faction,
            orbs_stay: false,
            telegraphs: Vec::new(),
            charges: Vec::new(),
            ground_zones: Vec::new(),
            ground_objects: Vec::new(),
            objects_due: Vec::new(),
            spawn_rules: Default::default(),
            cutscene: None,
            mortal_looks: Vec::new(),
            extras: Default::default(),
            beams: Vec::new(),
            cameos_due: Vec::new(),
            assists: Default::default(),
            fallen_attack: 0.0,
            fallen_boss: None,
            next_id: 10,
            events: Vec::new(),
        };
        let lines = run.blocking_lines();
        run.world.set_blocked(&lines);
        let discs: Vec<(Vec2, f32, f32)> = content.zones[zi].platforms.iter().map(|p| (p.centre, p.radius, p.z)).collect();
        run.world.set_platforms(&discs);
        let spawn = content.zones[zi].spawn;
        for setup in heroes {
            let class = content.classes.iter().position(|c| c.id == setup.class)?;
            let race = content.races.iter().position(|r| r.id == setup.race)?;
            let c = &content.classes[class];
            let r = &content.races[race];
            let option = c.starting_option(setup.start.as_deref())?;
            let starting = content.ability(option.main_spell())?;
            let start = option.id.clone();
            // the option's other level-1 spells (a finisher) are known from the start too
            let partners: Vec<&esurv_content::Ability> = option.spells.iter().skip(1).filter_map(|s| content.ability(s)).collect();
            let race_spell = content.ability(&r.spell)?;
            // the weapon loadout's effects join what the hero brought (a
            // form gets them too)
            let weapons = content.loadout(&c.id, setup.loadout.as_deref());
            let loadout = weapons.map_or_else(String::new, |l| l.id.clone());
            let mut with_weapons = setup.clone();
            if let Some(l) = weapons {
                with_weapons.add_effects_of(&ProcOrigin::Loadout(l.id.clone()), l.effects.iter().map(|e| (e, 1)));
            }
            let setup = &with_weapons;
            let mut modifiers = race_modifiers(r);
            modifiers.extend(Run::form_modifiers(option));
            modifiers.extend(setup.modifiers.iter().copied());
            let stats = compute(c, 1, &modifiers);
            let extra_lives = modifiers.iter().filter(|m| m.stat == Stat::ExtraLives).map(|m| m.value).sum::<f32>().max(0.0) as u32;
            let id = run.new_id();
            run.heroes.push(Hero {
                id,
                class,
                race,
                pos: (spawn.x, spawn.y),
                height: spawn.z,
                facing: spawn.facing,
                start,
                loadout,
                kit: Default::default(),
                spells: {
                    let mut book = Spellbook::start(starting, race_spell, &setup.spell_modifiers, c.spell_damage_scale);
                    for p in &partners {
                        book.learn(p, &setup.spell_modifiers);
                    }
                    book
                },
                base_modifiers: modifiers.clone(),
                modifiers,
                spell_modifiers: setup.spell_modifiers.clone(),
                milestones: setup.milestones.clone(),
                base_spell_modifiers: setup.spell_modifiers.clone(),
                base_milestones: setup.milestones.clone(),
                milestone_origins: setup.milestone_origins.clone(),
                base_milestone_origins: setup.milestone_origins.clone(),
                jewelry: Vec::new(),
                amulet: None,
                first_cache_amulet: None,
                rerolls_free: super::choices::FREE_REROLLS,
                rerolls_paid: 0,
                loot: Vec::new(),
                swing_timer: 0.0,
                off_swing_timer: 0.0,
                milestone_state: Vec::new(),
                elites_killed: 0,
                rares_killed: 0,
                caches_opened: 0,
                fell_at: None,
                buffs: Vec::new(),
                healing_charges: 0,
                charged_cast: 0,
                health: stats.max_health,
                stats,
                shield: 0.0,
                absorbed: 0.0,
                progress: Progress::default(),
                gold: 0,
                extra_lives,
                cheat_death_ready: 0.0,
                alive: true,
                damage_taken: 0.0,
                damage_dealt: 0.0,
                kills: 0,
                gold_earned: 0,
                magnets_taken: 0,
                pending_caches: 0,
                pending_rare_caches: 0,
                pending_choices: 0,
                pending_teleport: None,
                afflictions: Vec::new(),
            });
        }
        (!run.heroes.is_empty()).then_some(run)
    }

    pub(crate) fn new_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    pub(crate) fn push(&mut self, mut e: Event) {
        // a summon casting for its hero shows the cast's looks itself
        if let (Some((hero, summon)), Event::SpellGo { caster, .. }) = (self.cast_by_summon, &mut e) {
            if *caster == Unit::Hero(hero) {
                *caster = summon;
            }
        }
        self.events.push(e);
    }

    /// Takes the events gathered since the last call.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub(crate) fn zone(&self) -> &esurv_content::Zone {
        &self.content.zones[self.zone]
    }

    pub(crate) fn arena(&self) -> (Vec2, f32) {
        let z = self.zone();
        (z.arena_centre.unwrap_or((z.spawn.x, z.spawn.y)), z.arena_radius)
    }

    /// The lines nobody crosses now: the zone's walls and its doors still shut.
    pub(crate) fn blocking_lines(&self) -> Vec<(Vec2, Vec2)> {
        let z = self.zone();
        let shut = z.doors.iter().zip(&self.doors_open).filter(|(_, open)| !**open).map(|(d, _)| d.doorway);
        z.walls.iter().copied().chain(shut).map(|w| (w.from, w.to)).collect()
    }

    /// Opens every door whose minute has come, with its raid warning.
    fn doors_tick(&mut self) {
        let minute = self.now / 60.0;
        let content = self.content.clone();
        let doors = &content.zones[self.zone].doors;
        let due: Vec<usize> = (0..doors.len()).filter(|&d| !self.doors_open[d] && minute >= doors[d].opens_at_minute).collect();
        if due.is_empty() {
            return;
        }
        for d in due {
            self.set_door(d, true);
        }
    }

    /// Door `d` of the zone swings open (with its raid warning, if it has one) or shut again; its
    /// doorway stops or starts being a wall (a script may open a door,
    /// Stratholme's Slaughterhouse).
    pub(crate) fn set_door(&mut self, d: usize, open: bool) {
        if self.doors_open.get(d).is_none_or(|&now| now == open) {
            return;
        }
        self.doors_open[d] = open;
        self.events.push(if open { Event::DoorOpened { door: d } } else { Event::DoorShut { door: d } });
        let announce = &self.content.zones[self.zone].doors[d].announce;
        if open && !announce.is_empty() {
            self.events.push(Event::RaidWarning(announce.clone()));
        }
        let lines = self.blocking_lines();
        self.world.set_blocked(&lines);
    }

    /// A hero's client reports a new position. Strays past the arena (or through the ground)
    /// are sent back; the first real move starts the run.
    pub fn hero_moved(&mut self, hero: u32, pos: Vec2, height: f32, facing: f32) {
        let Some(i) = self.heroes.iter().position(|h| h.id == hero) else { return };
        // a scripted pause holds every hero where it stands
        if self.cutscene.is_some() {
            return;
        }
        if let Some((target, left)) = self.heroes[i].pending_teleport {
            if dist(pos, target) > 3.0 && left > 0 {
                self.heroes[i].pending_teleport = Some((target, left - 1));
                return;
            }
            self.heroes[i].pending_teleport = None;
        }
        let (centre, radius) = self.arena();
        let limit = radius + EDGE_SLACK;
        let from_centre = dist(pos, centre);
        let spawn_z = self.zone().spawn.z;
        let ground_centre = self.world.floor(centre.0, centre.1, spawn_z).unwrap_or(0.0);
        // with no floor under them a hero fell through it; a dungeon still has floors overhead
        let fell = match self.world.floor(pos.0, pos.1, height).or_else(|| self.world.height(pos.0, pos.1)) {
            Some(g) => height < g - FALL_LIMIT,
            None => height < ground_centre - 120.0,
        };
        let (mut pos, mut height) = (pos, height);
        // a wall or a shut door is not crossed; the hero is put back
        // where they last stood
        let before = self.heroes[i].pos;
        if self.blocking_lines().into_iter().any(|l| crosses(before, pos, l)) {
            pos = before;
            height = self.heroes[i].height;
            self.heroes[i].pending_teleport = Some((pos, 12));
            self.events.push(Event::HeroTeleported { hero, to: pos, height, left_arena: false });
        } else if from_centre > limit || fell {
            let strayed = from_centre > limit;
            if strayed {
                let k = (limit - EDGE_RETURN_INSET) / from_centre;
                pos = (centre.0 + (pos.0 - centre.0) * k, centre.1 + (pos.1 - centre.1) * k);
            }
            let floor = self.world.floor(pos.0, pos.1, height).or_else(|| self.world.floor(pos.0, pos.1, spawn_z));
            height = floor.unwrap_or(ground_centre) + 0.5;
            self.heroes[i].pending_teleport = Some((pos, 12));
            self.events.push(Event::HeroTeleported { hero, to: pos, height, left_arena: strayed });
            if strayed {
                self.events.push(Event::Announce(super::lines::LEFT_ARENA.into()));
            }
        }
        self.heroes[i].facing = facing;
        self.heroes[i].pos = pos;
        self.heroes[i].height = height;
        if dist(before, pos) > START_MOVE {
            self.start();
        }
    }

    /// Starts a waiting run (a hero's first move does this).
    pub fn start(&mut self) {
        if self.phase == Phase::Waiting {
            self.phase = Phase::Running;
            self.events.push(Event::RunStarted);
            let zone = self.zone();
            let line = Event::Emote { from: Speaker::Named(zone.name.clone()), text: super::lines::run_opens(&zone.subtitle) };
            self.events.push(line);
            self.open_run();
        }
    }

    /// The living hero nearest to `at`.
    pub(crate) fn nearest_hero(&self, at: Vec2) -> Option<usize> {
        self.heroes
            .iter()
            .enumerate()
            .filter(|(_, h)| h.alive)
            .min_by(|a, b| dist(a.1.pos, at).total_cmp(&dist(b.1.pos, at)))
            .map(|(i, _)| i)
    }

    /// A living hero to place spawns around (with one hero, always that one).
    pub(crate) fn anchor_hero(&mut self) -> Option<usize> {
        let living: Vec<usize> = (0..self.heroes.len()).filter(|&i| self.heroes[i].alive).collect();
        match living.len() {
            0 => None,
            1 => Some(living[0]),
            n => Some(living[(self.rng.next_u64() % n as u64) as usize]),
        }
    }

    /// A random spot `ring` yards from hero `h`, pulled into the arena, on a floor near the
    /// hero's height, in the hero's sight on open ground or walked to within the ring in a dungeon,
    /// when one can be found (blocked or unreachable spots re-roll as height
    /// misses do).
    /// A spot that does not fit is first stepped in toward the hero along its direction
    /// ([`SPAWN_STEP_IN`] at a time, down to [`spawn_inner_floor`], never nearer), so a hero beside a wall or a map edge still gets
    /// spawns on that side, closer; only when no step fits is a new direction drawn.
    pub(crate) fn spawn_point(&mut self, h: usize, ring: (f32, f32)) -> Vec2 {
        let (centre, radius) = self.arena();
        let hero = &self.heroes[h];
        let (hpos, hheight) = (hero.pos, hero.height);
        let clamp = |spot: Vec2| {
            let from_centre = dist(spot, centre);
            if from_centre > radius {
                let k = radius / from_centre;
                (centre.0 + (spot.0 - centre.0) * k, centre.1 + (spot.1 - centre.1) * k)
            } else {
                spot
            }
        };
        let floor = spawn_inner_floor(ring);
        let mut spot = hpos;
        for _ in 0..SPAWN_TRIES {
            let angle = self.rng.range(0.0, std::f32::consts::TAU);
            let d = self.rng.range(ring.0, ring.1);
            let dir = (angle.cos(), angle.sin());
            spot = clamp((hpos.0 + d * dir.0, hpos.1 + d * dir.1));
            if self.spawn_spot_fits(hpos, hheight, spot, ring) {
                return spot;
            }
            // step in from where the (clamped) spot lies, along the same direction
            // (only a blocked draw pays for these: as many steps as reach the floor, the last
            // one landing on it)
            let start = dist(hpos, spot);
            let steps = ((start - floor).max(0.0) / SPAWN_STEP_IN).ceil() as u32;
            for k in 1..=steps {
                let d = (start - SPAWN_STEP_IN * k as f32).max(floor);
                let step = clamp((hpos.0 + d * dir.0, hpos.1 + d * dir.1));
                if self.spawn_spot_fits(hpos, hheight, step, ring) {
                    return step;
                }
            }
        }
        spot
    }

    /// Whether `spot` can take a spawn for a hero at `hpos` / `hheight`: a floor near the hero's
    /// height in the hero's sight on open ground, or walked to within the ring's reach in a dungeon.
    fn spawn_spot_fits(&self, hpos: Vec2, hheight: f32, spot: Vec2, ring: (f32, f32)) -> bool {
        let near = |g: f32| (g - hheight).abs() < SPAWN_HEIGHT_TOLERANCE;
        match self.world.walk([hpos.0, hpos.1, hheight], spot, ring.1 * SPAWN_WALK_REACH) {
            Walk::Straight => {
                let floor = self.world.floor(spot.0, spot.1, hheight).filter(|&g| near(g));
                let eye = |at: Vec2, z: f32| [at.0, at.1, z + SIGHT_HEIGHT];
                floor.is_some_and(|g| self.world.clear_line(eye(hpos, hheight), eye(spot, g)))
            }
            // in a dungeon the spot (drawn in the ring) is walked to, not seen; the
            // walk's reach is searched, so it is never further than that
            Walk::Reached { z, .. } => near(z),
            Walk::Unreachable => false,
        }
    }

    fn alive_count(&self) -> usize {
        self.enemies.len()
    }

    /// Spawns roster enemy `index` (at `at`, or around a hero); false when the live cap is reached.
    /// A boss script's spawn rules may put another of the roster in its place, or none.
    pub fn spawn_roster_enemy(&mut self, index: usize, at: Option<Vec2>) -> bool {
        self.spawn_roster_enemy_with(index, at, false)
    }

    /// [`Run::spawn_roster_enemy`]; `summoned` (a boss script's wave or hatchlings) goes past the
    /// live cap and the spawn rules.
    pub(crate) fn spawn_roster_enemy_with(&mut self, index: usize, at: Option<Vec2>, summoned: bool) -> bool {
        if !summoned && self.alive_count() >= live_cap(self.options.density_pct, &self.content.balance) {
            return false;
        }
        let index = index.min(self.zone().enemies.len().saturating_sub(1));
        let index = if summoned {
            index
        } else {
            match self.allowed_roster(index) {
                Some(i) => i,
                None => return false,
            }
        };
        let pos = match at {
            Some(p) => p,
            None => match self.anchor_hero() {
                Some(h) => self.spawn_point(h, self.zone().enemy_spawn_ring),
                None => return false,
            },
        };
        let content = self.content.clone();
        let zone = &content.zones[self.zone];
        let e = &zone.enemies[index];
        let stats = spawn_stats(e, zone, &content.balance, self.now / 60.0, self.endless_minutes(), &self.options, RUN_FACTOR);
        let id = self.new_id();
        let aim_angle = self.rng.range(0.0, std::f32::consts::TAU);
        let first_thought = self.rng.range(0.0, FIRST_THOUGHT);
        self.enemies.push(EnemyUnit {
            id,
            kind: EnemyKind::Roster(index),
            segment: Segment::standing(pos, self.now),
            next_decision: self.now + first_thought,
            aim_angle,
            health: stats.health,
            max_health: stats.health,
            damage: stats.damage,
            base_speed: stats.move_speed,
            attack_interval: e.attack_interval,
            swing_timer: stats.first_swing,
            collision_radius: stats.collision_radius,
            ranged_range: e.ranged.map(|r| r.range),
            ranged_spell: e.ranged.map(|r| r.look_spell),
            cast_time: e.ranged.map_or(0.0, |r| r.cast_time),
            shot_share: 1.0,
            casting_until: None,
            charging_until: 0.0,
            abilities: Vec::new(),
            next_ability_at: 0.0,
            slow: None,
            weaken: None,
            fear_until: 0.0,
            silenced_until: 0.0,
            hold_until: 0.0,
            hold_chain_start: 0.0,
            hold_immune_until: 0.0,
            vulnerable: None,
            dots: Vec::new(),
            kit: Default::default(),
            elite: e.elite,
            focus_as_elite: e.focus_as_elite,
            enraged: false,
            spawn_cost: e.spawn_cost,
            mech: Default::default(),
        });
        self.events.push(Event::EnemySpawned { id, roster: index, at: pos });
        true
    }

    /// Brings the zone's boss `index` in (at its own spot, or on the boss ring round a hero), with
    /// its yell and its pull voice line.
    pub fn spawn_boss(&mut self, index: usize) {
        self.spawn_boss_with(index, None, false);
    }

    /// Brings boss `index` back at `spot` without its arrival yell and voice (a revive).
    pub(crate) fn spawn_boss_quietly(&mut self, index: usize, spot: Vec2) {
        self.spawn_boss_with(index, Some(spot), true);
    }

    fn spawn_boss_with(&mut self, index: usize, spot: Option<Vec2>, quiet: bool) {
        let Some(h) = self.anchor_hero() else { return };
        // a boss with a spot of its own comes there
        let pos = match (spot, self.zone().bosses[index].at) {
            (Some(p), _) => p,
            (None, Some(at)) => (at.x, at.y),
            (None, None) => self.spawn_point(h, self.zone().boss_spawn_ring),
        };
        let content = self.content.clone();
        let zone = &content.zones[self.zone];
        let b = &zone.bosses[index];
        let stats = boss_stats(b, zone, &content.balance, self.now / 60.0, self.endless_minutes(), &self.options, RUN_FACTOR);
        let yell = b.yell.clone();
        // a boss with a spell (its character's, a moonkin's, its class's) stands and casts it from
        // a caster's range for a share of its damage
        let caster = content.boss_spell(zone, index, self.party_faction.other(), self.seed);
        // a player-looking boss fights as its class, then with its zone's own
        let abilities = content.boss_abilities(zone, index, self.party_faction.other(), self.seed);
        let ranged_kind = content.enemy_kinds.as_ref().and_then(|k| k.enemies.get(&esurv_content::KindName("Ranged".into()))).and_then(|k| k.ranged);
        let (range, cast_time) = ranged_kind.map_or((18.0, 1.5), |r| (r.range, r.cast_time));
        let id = self.new_id();
        self.enemies.push(EnemyUnit {
            id,
            kind: EnemyKind::Boss(index),
            segment: Segment::standing(pos, self.now),
            next_decision: self.now + BOSS_FIRST_THOUGHT,
            aim_angle: 0.0,
            health: stats.health,
            max_health: stats.health,
            damage: stats.damage,
            base_speed: stats.move_speed,
            attack_interval: super::enemy::BOSS_SWING,
            swing_timer: stats.first_swing,
            collision_radius: stats.collision_radius,
            ranged_range: caster.map(|_| range),
            ranged_spell: caster,
            cast_time: if caster.is_some() { cast_time } else { 0.0 },
            shot_share: if caster.is_some() { content.pvp_looks.caster_boss_damage } else { 1.0 },
            casting_until: None,
            charging_until: 0.0,
            abilities: abilities.iter().map(super::enemy_attacks::AbilitySlot::boss).collect(),
            next_ability_at: 0.0,
            slow: None,
            weaken: None,
            fear_until: 0.0,
            silenced_until: 0.0,
            // it arrives held a moment, as a hold that began as it appeared
            hold_until: self.now + BOSS_ARRIVAL_HOLD,
            hold_chain_start: self.now,
            hold_immune_until: 0.0,
            vulnerable: None,
            dots: Vec::new(),
            kit: Default::default(),
            elite: false,
            focus_as_elite: false,
            enraged: false,
            spawn_cost: 0.0,
            mech: Default::default(),
        });
        let k = self.enemies.len() - 1;
        self.events.push(Event::BossSpawned { id, boss: index, at: pos });
        if !b.yell.is_empty() && !quiet {
            self.events.push(Event::Yell { id, text: yell });
        }
        // the original boss's own pull voice line with it (a hand-made character has none)
        if let (Some(sound), None, false) = (b.aggro_sound, &b.played_by, quiet) {
            self.events.push(Event::Sound(sound));
        }
        if !quiet {
            self.events.push(Event::Emote { from: Speaker::Unit(Unit::Enemy(id)), text: super::lines::BOSS_ARRIVES.into() });
        }
        // its script begins
        self.boss_arrives(k);
    }

    /// Minutes into Endless: since a final boss last died, while Endless is on; 0 otherwise.
    pub(crate) fn endless_minutes(&self) -> f32 {
        self.endless_clock().map_or(0.0, |s| ((self.now - s) / 60.0).max(0.0))
    }

    /// Where Endless's growth counts from: the latest final boss death, while Endless is on.
    pub(crate) fn endless_clock(&self) -> Option<f32> {
        self.endless_since.and(self.final_boss_fell_at)
    }

    /// `None` when no boss is alive; `Some(true)` while the final boss lives.
    fn boss_alive(&self) -> Option<bool> {
        let zone = self.zone();
        let mut any = None;
        for e in &self.enemies {
            if let EnemyKind::Boss(b) = e.kind {
                let fin = zone.bosses[b].is_final;
                any = Some(any.unwrap_or(false) || fin);
            }
        }
        any
    }

    /// Advances the run by `dt` seconds (clamped to `MAX_DT`).
    pub fn tick(&mut self, dt: f32) {
        if self.phase != Phase::Running {
            return;
        }
        // the final boss fell: the run is won at the first tick that finds it running, so a choice
        // the kill brought is made first
        if self.won_at.is_some() && self.endless_since.is_none() {
            self.phase = Phase::Victory;
            self.events.push(Event::Victory);
            // the run is over: novas still to come of a cast go off no more
            self.novas.clear();
            return;
        }
        let dt = dt.clamp(0.0, MAX_DT);
        // a scripted pause freezes everything but its own time
        if self.cutscene_tick(dt) {
            // a cutscene's own timed lines run through it (Whitemane's revive)
            self.later_tick(dt);
            return;
        }
        self.now += dt;
        self.heroes_tick(dt);
        self.kit_tick(dt);
        self.spells_tick(dt);
        self.periodic_milestones_tick();
        self.pets_tick();
        self.assists_tick();
        self.effects_tick();
        self.enemies_tick(dt);
        self.enemy_hits_tick();
        self.ground_zones_tick(dt);
        self.ground_objects_tick(dt);
        self.mortal_looks_tick();
        self.later_tick(dt);
        self.beams_tick(dt);
        self.cameos_tick();
        self.pickups_tick();
        self.field_timers_tick();
        self.doors_tick();
        self.spawning_tick(dt);
        self.open_pending_screen();
        if self.heroes.iter().all(|h| !h.alive) && self.phase == Phase::Running {
            self.phase = Phase::Defeat;
            self.events.push(Event::Defeat);
            self.novas.clear();
        }
    }

    pub(crate) fn refresh_stats(&mut self, i: usize) {
        let h = &mut self.heroes[i];
        let c = &self.content.classes[h.class];
        let mut mods = h.modifiers.clone();
        for b in h.buffs.iter() {
            match b.kind {
                BuffKind::Damage => mods.push(Modifier::new(Stat::Damage, b.value * (1.0 + h.stats.buff_power))),
                BuffKind::Haste => mods.push(Modifier::new(Stat::CooldownRecovery, b.value * (1.0 + h.stats.buff_power))),
                BuffKind::Speed => mods.push(Modifier::new(Stat::MoveSpeed, b.value)),
                BuffKind::Berserk => mods.push(Modifier::new(Stat::Damage, b.value)),
                _ => {}
            }
        }
        let old_max = h.stats.max_health;
        let buffs = self.milestone_buff_modifiers(i);
        let kit = self.kit_modifiers(i);
        let assisted = self.assist_modifiers(i);
        let h = &mut self.heroes[i];
        mods.extend(buffs);
        mods.extend(kit);
        mods.extend(assisted);
        h.stats = compute(c, h.progress.level, &mods);
        if h.stats.max_health > old_max {
            h.health += h.stats.max_health - old_max;
        }
        h.health = h.health.min(h.stats.max_health);
    }

    fn heroes_tick(&mut self, dt: f32) {
        let now = self.now;
        for i in 0..self.heroes.len() {
            if !self.heroes[i].alive {
                continue;
            }
            let expired = self.heroes[i].buffs.iter().any(|b| b.until <= now);
            let ended: Vec<u32> = self.heroes[i].buffs.iter().filter(|b| b.until <= now && b.visual != 0).map(|b| b.visual).collect();
            if expired {
                if self.heroes[i].buffs.iter().any(|b| b.until <= now && b.kind == BuffKind::Shield) {
                    self.heroes[i].shield = 0.0;
                    self.heroes[i].absorbed = 0.0;
                }
                self.heroes[i].buffs.retain(|b| b.until > now);
                // unused healing charges vanish with their buff
                if !self.heroes[i].buffs.iter().any(|b| b.kind == BuffKind::HealingCharges) {
                    self.heroes[i].healing_charges = 0;
                }
                self.refresh_stats(i);
                self.buffs_ended(i, &ended);
            }
            let ended = self.heroes[i].milestone_state.iter_mut().filter(|s| s.stacks > 0 && now >= s.until).fold(false, |_, s| {
                s.stacks = 0;
                true
            });
            if ended {
                self.refresh_stats(i);
            }
            // health per second first, then the heals over time, which (unlike it) feed the
            // healing milestones with what they restored and what spilled over
            let h = &mut self.heroes[i];
            let hot = h.buffs.iter().filter(|b| b.kind == BuffKind::Regen).map(|b| b.value).sum::<f32>() * dt;
            let max = h.stats.max_health;
            if h.health < max {
                h.health = (h.health + h.stats.health_regen * dt).min(max);
            }
            if hot > 0.0 {
                let before = h.health;
                h.health = (h.health + hot).min(max.max(h.health));
                let healed = h.health - before;
                self.healing_milestones(i, hot, healed);
            }
        }
    }

    pub(crate) fn enemies_tick(&mut self, dt: f32) {
        let mut k = 0;
        while k < self.enemies.len() {
            let id = self.enemies[k].id;
            self.enemy_tick(k, dt);
            // abilities may add enemies and thorns may kill this one: carry on after it
            k = self.enemies.iter().position(|e| e.id == id).map_or(k, |p| p + 1);
        }
    }

    fn enemy_tick(&mut self, mut k: usize, dt: f32) {
        let now = self.now;
        let pos = self.enemies[k].position(now);
        let Some(h) = self.nearest_hero(pos) else { return };
        let hero_pos = self.heroes[h].pos;
        {
            let e = &mut self.enemies[k];
            if (e.is_boss() || e.is_rare()) && !e.enraged && e.health <= e.max_health * ENRAGE_HEALTH {
                e.enraged = true;
            }
        }
        // its script's health and timed lines
        if self.enemies[k].is_boss() {
            let id = self.enemies[k].id;
            self.boss_script_tick(k);
            let Some(again) = self.enemies.iter().position(|e| e.id == id) else { return };
            k = again;
        }
        // abilities keep counting down while held; a ready one waits until the hold ends, and
        // two casts are at least `ABILITY_GAP` apart
        if !self.enemies[k].abilities.is_empty() {
            let busy = now < self.enemies[k].mech.busy_until;
            let (held, id) = (now < self.enemies[k].hold_until || busy, self.enemies[k].id);
            self.enemy_abilities(k, h, dt, held);
            // a blow on the hero can set off milestones that kill enemies (a breaking shield's
            // burst …) and shift the list: find this enemy again, and stop if it died
            let Some(again) = self.enemies.iter().position(|e| e.id == id) else { return };
            k = again;
        }
        if now < self.enemies[k].mech.busy_until {
            // a boss running its rush line runs on (busy all the same: no swing, no ability)
            if now >= self.enemies[k].charging_until {
                self.stand_still(k);
            }
            return;
        }
        // an ally's taunt: it goes for the ally instead of the hero (`game::assists`)
        let taunt = self.taunted_by(self.enemies[k].id);
        let e = &mut self.enemies[k];
        if now < e.charging_until {
            return;
        }
        let enraged = e.is_boss() && e.enraged;
        // rage stacked on a boss makes it quicker (`game::boss_phases`)
        let speed = move_speed(e.base_speed, EnemyUnit::active(e.slow, now), enraged) * e.mech.phases.haste(now);
        if now < e.hold_until {
            if e.segment.moving(now) {
                e.segment = Segment::standing(pos, now);
            }
            return;
        }
        // Dominate Mind: a feared enemy turned on the others fights them, not the hero
        if e.kit.turned.is_some_and(|(_, until)| now < until) {
            self.turned_enemy_tick(k, pos, speed, dt);
            return;
        }
        if now < e.fear_until {
            if now >= e.next_decision {
                let (seg, next) = flee(pos, hero_pos, speed, now);
                e.segment = seg;
                e.next_decision = next;
            }
            return;
        }
        if let Some((ally, at)) = taunt {
            // a cast under way is dropped
            e.casting_until = None;
            self.taunted_enemy_tick(k, pos, ally, at, speed, dt);
            return;
        }
        let e = &mut self.enemies[k];
        // a ranged caster stands while casting; the shot leaves when the cast ends
        if let Some(until) = e.casting_until {
            if now >= until {
                e.casting_until = None;
                self.enemy_shoots(k, h);
            }
            return;
        }
        let (chase_beyond, orbit) = reach(e.collision_radius, e.ranged_range);
        let d = dist(pos, hero_pos);
        // one that never moves (a figure, a tentacle) waits for the hero to come into reach
        if d > chase_beyond && e.mech.phases.rooted {
            if e.segment.moving(now) {
                e.segment = Segment::standing(pos, now);
            }
            return;
        }
        if d > chase_beyond {
            if now >= e.next_decision {
                // in a dungeon, round walls and up ramps: the next stretch of the hero's flow field
                let hero_feet = [hero_pos.0, hero_pos.1, self.heroes[h].height];
                let (seg, next) = match self.world.toward(pos, hero_feet) {
                    Some(waypoint) => chase(pos, waypoint, &mut e.aim_angle, 0.0, speed, now),
                    None => chase(pos, hero_pos, &mut e.aim_angle, orbit, speed, now),
                };
                e.segment = seg;
                e.next_decision = next;
            }
            return;
        }
        // in reach: stop and swing
        if e.segment.moving(now) {
            e.segment = Segment::standing(pos, now);
        }
        e.swing_timer -= dt;
        if e.swing_timer > 0.0 {
            return;
        }
        e.swing_timer = e.attack_interval;
        if let Some(spell) = e.ranged_spell {
            // a silenced caster waits (the legendary amulets)
            if now < e.silenced_until {
                return;
            }
            e.casting_until = Some(now + e.cast_time);
            let (id, seconds, hero) = (e.id, e.cast_time, self.heroes[h].id);
            self.events.push(Event::EnemyCastStarted { id, spell, seconds, hero });
            return;
        }
        let swing = if e.is_boss() { e.mech.swing_mult } else { 1.0 };
        // a boss that never swings (a caster keeping to its marks)
        if swing <= 0.0 {
            return;
        }
        let base = attack_damage(e.damage, EnemyUnit::active(e.weaken, now), enraged);
        let (id, kind) = (e.id, e.kind);
        self.damage_hero(h, base * swing, Some(id), true);
        // a boss whose bite leaves a stack (Lupos)
        if let EnemyKind::Boss(b) = kind {
            if let Some(aff) = self.content.zones[self.zone].bosses[b].on_hit {
                if self.heroes[h].alive {
                    self.stack_on_hero(h, aff, base);
                }
            }
        }
    }

    /// Hero `i` takes `amount` (before damage reduction) from enemy `source`, if any; `melee`
    /// when it was a melee swing (auras and thorns strike back then).
    pub fn damage_hero(&mut self, i: usize, amount: f32, source: Option<u32>, melee: bool) {
        let now = self.now;
        if self.phase != Phase::Running || !self.heroes[i].alive {
            return;
        }
        if self.heroes[i].buffs.iter().any(|b| b.kind == BuffKind::Immunity && now < b.until) {
            return;
        }
        // a blocked melee blow does 30 % less damage
        let blocked = match (melee, source) {
            (true, Some(src)) => self.kit_block(i, src),
            _ => false,
        };
        let amount = if blocked { amount * (1.0 - BLOCK_REDUCTION) } else { amount };
        if let (true, Some(src)) = (blocked, source) {
            self.milestone_event(i, super::milestones::CombatEvent::Blocked { by: src });
        }
        let h = &mut self.heroes[i];
        let mut amount = amount * (1.0 - h.stats.damage_reduction);
        // the swing shows what it was worth before any shield took some
        let struck = amount;
        if h.shield > 0.0 {
            let took = h.shield.min(amount);
            h.shield -= took;
            amount -= took;
            h.absorbed += took;
            if h.shield <= 0.0 {
                h.buffs.retain(|b| b.kind != BuffKind::Shield);
                let absorbed = std::mem::take(&mut h.absorbed);
                self.refresh_stats(i);
                if absorbed > 0.0 {
                    self.milestone_event(i, super::milestones::CombatEvent::ShieldBroken { absorbed });
                }
            }
        }
        let h = &mut self.heroes[i];
        h.health -= amount;
        h.damage_taken += amount as f64;
        let hero = h.id;
        self.events.push(Event::HeroHit { hero, amount: struck, lost: amount, by: source, melee });
        self.kit_on_hurt(i, amount);
        // only a melee blow from an enemy strikes back and sets off milestones (being struck,
        // being hurt); spells, missiles and ground effects do neither
        if let (true, Some(src)) = (melee, source) {
            self.strike_back(i, src, amount);
            self.milestone_event(i, super::milestones::CombatEvent::StruckInMelee { by: src, taken: struck });
            self.milestone_event(i, super::milestones::CombatEvent::HeroDamaged { amount });
        }
        if self.heroes[i].health <= 0.0 {
            // who it falls to: the enemy that struck, or the boss whose mark or zone it was
            self.extras.last_blow = source.or(self.extras.blame);
            self.hero_falls(i);
        }
    }

    fn hero_falls(&mut self, i: usize) {
        let now = self.now;
        let hero = self.heroes[i].id;
        let cheat = self.heroes[i].milestones.iter().find_map(|m| match m {
            Milestone::CheatDeath { cooldown, health, immune_seconds } => Some((*cooldown, *health, *immune_seconds)),
            _ => None,
        });
        if let Some((cooldown, share, immune)) = cheat {
            if now >= self.heroes[i].cheat_death_ready {
                let h = &mut self.heroes[i];
                h.cheat_death_ready = now + cooldown;
                h.health = share * h.stats.max_health;
                if immune > 0.0 {
                    h.buffs.push(Buff { kind: BuffKind::Immunity, value: 1.0, visual: CHEAT_DEATH_IMMUNE_VISUAL, until: now + immune });
                }
                let (spell, text) = if immune > 0.0 { (CHEAT_DEATH_IMMUNE_VISUAL, super::lines::CHEAT_DEATH_IMMUNE) } else { (CHEAT_DEATH_VISUAL, super::lines::CHEAT_DEATH) };
                let me = Unit::Hero(hero);
                self.events.push(Event::SpellGo { caster: me, spell, hits: vec![me], aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
                self.events.push(Event::Emote { from: Speaker::Unit(me), text: text.into() });
                self.events.push(Event::CheatedDeath { hero });
                return;
            }
        }
        if self.heroes[i].extra_lives > 0 {
            let h = &mut self.heroes[i];
            h.extra_lives -= 1;
            h.health = h.stats.max_health * EXTRA_LIFE_HEALTH;
            let (at, blast) = (h.pos, h.stats.damage * EXTRA_LIFE_BLAST.1);
            let me = Unit::Hero(hero);
            self.events.push(Event::SpellGo { caster: me, spell: EXTRA_LIFE_VISUAL, hits: vec![me], aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
            self.events.push(Event::Emote { from: Speaker::Unit(me), text: super::lines::EXTRA_LIFE.into() });
            self.events.push(Event::ExtraLifeUsed { hero });
            // a boss a hero lost a life to (Mandokir's level-up)
            self.hero_fell();
            let near: Vec<u32> =
                self.enemies.iter().filter(|e| dist(e.position(now), at) < EXTRA_LIFE_BLAST.0).map(|e| e.id).collect();
            let hits = near.iter().map(|&id| Unit::Enemy(id)).collect();
            self.events.push(Event::SpellGo { caster: me, spell: EXTRA_LIFE_BLAST_VISUAL, hits, aim: Aim::Caster, ammo: false, reach: EXTRA_LIFE_BLAST.0, triggered: false });
            // a silent proc (no combat-log line, as before), on the meter under the Soulstone worn
            let origin = self.heroes[i]
                .jewelry
                .iter()
                .find(|w| self.content.jewelry.iter().any(|j| j.id == w.id && j.stat == Stat::ExtraLives))
                .map_or(ProcOrigin::Other, |w| ProcOrigin::Jewelry(w.id.clone()));
            let source = HitSource::Proc { spell: 0, school: esurv_content::School::Physical, origin };
            for id in near {
                self.hit_enemy(Some(i), id, blast, false, source.clone());
            }
            return;
        }
        let h = &mut self.heroes[i];
        h.health = 0.0;
        h.alive = false;
        h.fell_at = Some(now);
        self.events.push(Event::HeroDied { hero });
        self.hero_fell();
    }

    /// After a victory, the player may go on into Endless; `false` when the run is not won and
    /// waiting.
    pub fn continue_endless(&mut self) -> bool {
        if self.phase != Phase::Victory {
            return false;
        }
        self.endless_since = Some(self.now);
        self.run_again();
        self.events.push(Event::Emote { from: Speaker::Named(super::lines::ENDLESS_SPEAKER.into()), text: super::lines::ENDLESS.into() });
        true
    }

    /// Pauses a running run: the clock stops (phase 3). `false` when it is not running (a choice
    /// open, not yet begun, over).
    pub fn pause(&mut self) -> bool {
        if self.phase != Phase::Running {
            return false;
        }
        self.phase = Phase::Paused;
        true
    }

    /// Runs a paused run again.
    pub fn resume(&mut self) -> bool {
        if self.phase != Phase::Paused {
            return false;
        }
        self.run_again();
        true
    }

    /// Back to running after a pause, a choice or a victory: every enemy decides where to go at once.
    pub(crate) fn run_again(&mut self) {
        self.phase = Phase::Running;
        let now = self.now;
        for e in &mut self.enemies {
            e.next_decision = now;
        }
    }

    /// Hero `hero` leaves the run: it is out from now on, as if it fell (its time ends here), and
    /// its open or owed choices go. The run is over when no hero is left in it (a victory stays a
    /// victory).
    pub fn leave(&mut self, hero: u32) {
        let Some(i) = self.heroes.iter().position(|h| h.id == hero) else { return };
        let now = self.now;
        let h = &mut self.heroes[i];
        if h.alive {
            h.alive = false;
            h.fell_at = Some(now);
        }
        h.pending_choices = 0;
        h.pending_caches = 0;
        h.pending_rare_caches = 0;
        if self.screen.as_ref().is_some_and(|s| s.hero == i) {
            self.screen = None;
            if self.phase == Phase::Choosing {
                self.run_again();
            }
        }
        if self.heroes.iter().all(|h| !h.alive) {
            if matches!(self.phase, Phase::Waiting | Phase::Running | Phase::Choosing | Phase::Paused) {
                self.phase = Phase::Defeat;
                self.events.push(Event::Defeat);
            }
        } else if self.phase == Phase::Running {
            self.open_pending_screen();
        }
    }

    fn spawning_tick(&mut self, dt: f32) {
        let input = DirectorInput {
            elapsed: self.now,
            dt,
            density_pct: self.options.density_pct,
            budget_per_second: super::enemy::time_curve(&self.content.balance.spawn_budget, self.now / 60.0),
            boss_alive: self.boss_alive(),
            endless_since: self.endless_clock(),
        };
        let minute = self.now / 60.0;
        // population floor
        let normals = self
            .enemies
            .iter()
            .filter(|e| matches!(e.kind, EnemyKind::Roster(r) if !self.zone().enemies[r].elite))
            .count() as u32;
        // a boss's script may have stopped the zone's spawns
        if self.spawn_rules.stopped {
            self.spawn_bosses_due(minute);
            return;
        }
        let missing = Director::floor_shortfall(&input, normals);
        for _ in 0..missing {
            let weights = roster_weights(&self.zone().enemies, minute);
            let Some(index) = self.rng.weighted(&weights) else { break };
            if !self.spawn_roster_enemy(index, None) {
                break;
            }
        }
        // the budget director
        let content = self.content.clone();
        let zone = &content.zones[self.zone];
        let mut director = std::mem::take(&mut self.director);
        let mut rng = std::mem::replace(&mut self.rng, Rng::new(0));
        let mut wanted = Vec::new();
        let live = self.alive_count();
        let cap = live_cap(self.options.density_pct, &self.content.balance);
        director.tick(&input, zone, &mut rng, |index| {
            if live + wanted.len() >= cap {
                return false;
            }
            wanted.push(index);
            true
        });
        self.rng = rng;
        for index in wanted {
            self.spawn_roster_enemy(index, None);
        }
        // elites
        let mut rng = std::mem::replace(&mut self.rng, Rng::new(0));
        let elite = director.elite_due(&input, zone, &mut rng);
        let swarm = director.swarm_due(&input, zone, &mut rng);
        self.rng = rng;
        self.director = director;
        if let Some(index) = elite {
            // the elite whose turn a due rare takes comes as that
            // rare, silently (`game::rares`)
            if !self.elite_as_rare(index) && self.spawn_roster_enemy(index, None) {
                let id = self.enemies.last().map_or(0, |e| e.id);
                self.events.push(Event::Emote { from: Speaker::Unit(Unit::Enemy(id)), text: super::lines::ELITE_APPROACHES.into() });
            }
        }
        if let Some(swarm) = swarm {
            if let Some(h) = self.anchor_hero() {
                let at = self.heroes[h].pos;
                for (dx, dy) in &swarm.offsets {
                    if !self.spawn_roster_enemy(swarm.enemy, Some((at.0 + dx, at.1 + dy))) {
                        break;
                    }
                }
                self.events.push(Event::Emote { from: Speaker::Named(zone.enemies[swarm.enemy].name.clone()), text: super::lines::SWARM.into() });
            }
        }
        self.spawn_bosses_due(minute);
    }

    /// The boss schedule; in Endless, after it, the final boss now and then once more (in the
    /// second before a swarm).
    fn spawn_bosses_due(&mut self, minute: f32) {
        let content = self.content.clone();
        let zone = &content.zones[self.zone];
        if self.next_boss < zone.bosses.len() {
            // a boss only a script brings is passed by
            if zone.bosses[self.next_boss].called {
                self.next_boss += 1;
            } else if zone.bosses[self.next_boss].spawn_minute <= minute {
                let b = self.next_boss;
                self.next_boss += 1;
                self.spawn_boss(b);
            }
        } else if self.endless_since.is_some()
            && self.now >= self.director.next_swarm() - ENDLESS_BOSS_WINDOW
            && self.rng.unit() < ENDLESS_BOSS_CHANCE
        {
            // every final boss, as they came together (one only a script brings comes by it)
            let finals: Vec<usize> = (0..zone.bosses.len()).filter(|&b| zone.bosses[b].is_final && !zone.bosses[b].called).collect();
            for b in finals {
                self.spawn_boss(b);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Arc<Content> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        Arc::new(Content::load(dir).expect("content loads"))
    }

    fn hero() -> HeroSetup {
        HeroSetup { class: "warrior".into(), race: "human".into(), ..Default::default() }
    }

    fn elwynn() -> Run {
        Run::new(content(), "elwynn", RunOptions::default(), &[hero()], Box::new(Flat), 7).unwrap()
    }

    fn start(run: &mut Run) {
        let h = run.heroes[0].id;
        let p = run.heroes[0].pos;
        run.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
    }

    #[test]
    fn nothing_happens_until_the_hero_moves() {
        let mut run = elwynn();
        run.tick(0.1);
        assert_eq!((run.phase, run.now), (Phase::Waiting, 0.0));
        let h = run.heroes[0].id;
        let p = run.heroes[0].pos;
        run.hero_moved(h, (p.0 + 0.1, p.1), 0.0, 0.0);
        assert_eq!(run.phase, Phase::Waiting);
        start(&mut run);
        assert_eq!(run.phase, Phase::Running);
        assert!(run.drain_events().contains(&Event::RunStarted));
    }

    #[test]
    fn strays_are_sent_back_inside_the_arena() {
        let mut run = elwynn();
        let ((cx, cy), r) = run.arena();
        let h = run.heroes[0].id;
        run.hero_moved(h, (cx + r + 20.0, cy), 0.0, 0.0);
        let p = run.heroes[0].pos;
        assert!((dist(p, (cx, cy)) - (r + 6.0)).abs() < 1e-3);
        assert!(run.drain_events().iter().any(|e| matches!(e, Event::HeroTeleported { left_arena: true, .. })));
    }

    #[test]
    fn the_hero_gets_the_race_spell_and_passives() {
        let run = Run::new(content(), "elwynn", RunOptions::default(), &[HeroSetup { race: "tauren".into(), ..hero() }], Box::new(Flat), 1).unwrap();
        let h = &run.heroes[0];
        assert_eq!(h.spells.racial.as_ref().unwrap().spell, "war_stomp");
        // the tauren's +15 % health, beside the default two-hander's -5 %
        assert!((h.stats.max_health - 240.0 * (1.15 - 0.05)).abs() < 1e-3);
    }

    #[test]
    fn bosses_arrive_on_schedule_with_their_health() {
        let mut run = elwynn();
        start(&mut run);
        // keep the hero alive: this test only watches the schedule
        let cheat = Milestone::CheatDeath { cooldown: 0.0, health: 1.0, immune_seconds: 0.0 };
        run.heroes[0].base_milestones.push(cheat.clone());
        run.heroes[0].milestones.push(cheat);
        let mut arrivals = Vec::new();
        while run.now < 16.5 * 60.0 && run.phase == Phase::Running {
            run.tick(0.05);
            run.take_first_offers();
            for e in run.drain_events() {
                if let Event::BossSpawned { boss, .. } = e {
                    arrivals.push((boss, run.now));
                }
            }
            // the hero does no damage yet: clear the field now and then so the live cap never blocks
            if run.enemies.len() > 40 {
                let ids: Vec<u32> = run.enemies.iter().filter(|e| !e.is_boss()).map(|e| e.id).collect();
                for id in ids {
                    run.hit_enemy(None, id, f32::MAX, false, HitSource::Other);
                }
            }
        }
        let minutes: Vec<f32> = arrivals.iter().map(|(_, t)| (t / 60.0 * 10.0).floor() / 10.0).collect();
        assert_eq!(minutes, [3.0, 6.0, 9.0, 12.0, 16.0]);
        let zone = &run.content.zones[run.zone];
        let last = run.enemies.iter().find(|e| e.kind == EnemyKind::Boss(4)).unwrap();
        // the final boss kind on the health clock (to boss_clock_power) at its arrival: 240 × final kind × growth × clock(16 + a tick)^p
        let arrival = arrivals[4].1 / 60.0;
        let growth = run.content.balance.zone_scaling.health_factor(esurv_content::EnemyType::FinalBoss, zone.difficulty);
        let b = &run.content.balance;
        let want = 240.0 * b.final_boss.health * growth * crate::game::enemy::time_curve(&b.health_clock, arrival).powf(b.boss_clock_power);
        assert!((last.max_health - want).abs() / want < 1e-3, "{} vs {want}", last.max_health);
    }

    #[test]
    fn enemies_come_and_a_hero_who_never_fights_falls() {
        let mut run = elwynn();
        start(&mut run);
        let mut spawned = 0;
        while run.phase == Phase::Running && run.now < 600.0 {
            run.tick(0.05);
            run.take_first_offers();
            spawned += run.drain_events().iter().filter(|e| matches!(e, Event::EnemySpawned { .. })).count();
        }
        assert!(spawned >= 4);
        assert_eq!(run.phase, Phase::Defeat);
        assert!(!run.heroes[0].alive);
    }

    /// Caster bosses: a Warsong boss with a spell (its character's own,
    /// Starfire in Moonkin Form, else its class's) stands and casts it from a caster's range, each
    /// hit half the boss formula's damage; any other swings, its shots (none) at its whole damage.
    #[test]
    fn caster_bosses_cast_from_range_for_half_damage() {
        let content = content();
        let looks = &content.pvp_looks;
        let mut casters = 0;
        // both factions' characters: a Horde party fights the Alliance's
        for (setup, faction) in [(hero(), esurv_content::Faction::Horde), (HeroSetup { race: "orc".into(), ..hero() }, esurv_content::Faction::Alliance)] {
            for seed in 0..20 {
                let mut run = Run::new(content.clone(), "wsg", RunOptions::default(), std::slice::from_ref(&setup), Box::new(Flat), seed).unwrap();
                start(&mut run);
                let zone = run.content.zone("wsg").unwrap().clone();
                for b in 0..zone.bosses.len() {
                    let who = run.content.boss_character(&zone, b, faction, seed).map_or(String::new(), |c| c.name.clone());
                    let spell = run.content.boss_spell(&zone, b, faction, seed);
                    run.spawn_boss(b);
                    let k = run.enemies.len() - 1;
                    let e = &run.enemies[k];
                    assert_eq!(e.ranged_spell, spell, "seed {seed} boss {b}: {who}");
                    assert_eq!(e.ranged_range.is_some(), spell.is_some());
                    assert_eq!(e.shot_share, if spell.is_some() { looks.caster_boss_damage } else { 1.0 });
                    if spell.is_some() {
                        let full = e.damage;
                        run.enemy_shoots(k, 0);
                        assert_eq!(run.enemy_shots.last().unwrap().damage, full * 0.5, "{who}");
                    }
                    casters += usize::from(spell.is_some());
                }
            }
        }
        assert!(casters > 0, "a caster came up");
        assert_eq!(looks.caster_boss_damage, 0.5);
        for (class, spell) in [("mage", Some(133)), ("priest", Some(585)), ("warlock", Some(686)), ("warrior", None), ("rogue", None), ("hunter", Some(75))] {
            assert_eq!(looks.of(class).unwrap().boss_spell, spell, "{class}");
        }
        // the hand-picked spells: Nerf's Fireball, a hunter's shot, a moonkin's Starfire, a cat's swing
        let wsg = content.zone("wsg").unwrap();
        let named = |id: &str| {
            let mut zone = wsg.clone();
            zone.bosses[0].played_by = Some(esurv_content::CharacterPick::Named(id.into()));
            content.boss_spell(&zone, 0, esurv_content::Faction::Alliance, 0)
        };
        for (id, spell) in [("nerf", Some(133)), ("eggmike", Some(133)), ("tebay", Some(75)), ("regiss", Some(2912)), ("banksey", Some(585)), ("pawsz", None), ("moffel", None)] {
            assert_eq!(named(id), spell, "{id}");
        }
    }

    /// The Scarlet Monastery: the doorway behind the spawn is walled,
    /// the cathedral door opens at minute 10 with its raid warning, and the run is won only when
    /// both last bosses have fallen, each at its own spot: Mograine first, Whitemane, whom his fall
    /// brings (here killed before she raises him).
    #[test]
    fn the_scarlet_monastery_walls_opens_and_wins_on_both_last_bosses() {
        let mut run = Run::new(content(), "scarlet", RunOptions::default(), &[hero()], Box::new(Flat), 7).unwrap();
        start(&mut run);
        let h = run.heroes[0].id;
        // back through the doorway: put back where they stood
        let p = run.heroes[0].pos;
        run.hero_moved(h, (870.0, p.1), 0.0, 0.0);
        assert_eq!(run.heroes[0].pos, p);
        // the cathedral door is a wall until minute 10
        assert_eq!(run.blocking_lines().len(), 2);
        run.drain_events();
        run.now = 600.0;
        run.doors_tick();
        let events = run.drain_events();
        assert!(events.contains(&Event::DoorOpened { door: 0 }));
        assert!(events.iter().any(|e| matches!(e, Event::RaidWarning(t) if t.contains("cathedral"))));
        assert_eq!(run.blocking_lines().len(), 1);
        // the two last bosses, at their spots; the first to fall does not win the run
        let sm = run.content.zone("scarlet").unwrap().clone();
        let finals: Vec<usize> = (0..sm.bosses.len()).filter(|&b| sm.bosses[b].is_final).collect();
        run.next_boss = sm.bosses.len();
        run.spawn_boss(finals[0]);
        let at = sm.bosses[finals[0]].at.unwrap();
        assert_eq!(run.enemies.last().unwrap().segment.position(run.now), (at.x, at.y));
        let mograine = run.enemies.last().unwrap().id;
        run.hit_enemy(Some(0), mograine, f32::MAX, false, HitSource::Other);
        assert_eq!(run.won_at, None, "one of two");
        // his fall brought her, at her own spot
        let at = sm.bosses[finals[1]].at.unwrap();
        let whitemane = run.enemies.iter().find(|e| e.kind == EnemyKind::Boss(finals[1])).unwrap();
        assert_eq!(whitemane.segment.position(run.now), (at.x, at.y));
        let whitemane = whitemane.id;
        run.hit_enemy(Some(0), whitemane, f32::MAX, false, HitSource::Other);
        assert_eq!(run.won_at, Some(run.now));
    }

    /// Of two final bosses only the last to fall rolls the weapons:
    /// over many runs the first never drops one, the last does.
    #[test]
    fn only_the_last_final_boss_to_fall_drops_the_weapons() {
        let mut last_dropped = 0;
        for seed in 0..20 {
            let mut run = Run::new(content(), "scarlet", RunOptions::default(), &[hero()], Box::new(Flat), seed).unwrap();
            start(&mut run);
            let bosses = run.content.zone("scarlet").unwrap().bosses.clone();
            run.next_boss = bosses.len();
            // Mograine first; his fall brings Whitemane
            let first = (0..bosses.len()).find(|&b| bosses[b].is_final).unwrap();
            run.spawn_boss(first);
            let id = run.enemies.last().unwrap().id;
            run.drain_events();
            run.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Other);
            assert!(!run.drain_events().iter().any(|e| matches!(e, Event::GearDropped { .. })), "seed {seed}: the first");
            let id = run.enemies.iter().find(|e| e.is_boss()).unwrap().id;
            run.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Other);
            last_dropped += usize::from(run.drain_events().iter().any(|e| matches!(e, Event::GearDropped { .. })));
        }
        assert!(last_dropped > 0);
    }

    /// A zone with a roster for each faction: the party fights the one
    /// its faction meets, the faction most of its heroes share (the first hero's on a tie).
    #[test]
    fn a_party_fights_the_roster_its_faction_meets() {
        use esurv_content::testing::{content_folder_with_faction_zone, ExtraZone};
        let shipped = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let zone = ExtraZone { id: "test_factions", name: "Test Factions", difficulty: 11.0 };
        let dir = content_folder_with_faction_zone(shipped.as_ref(), "sim-faction-rosters", zone).unwrap();
        let loaded = Content::load(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        let content = Arc::new(loaded.unwrap());
        let of = |race: &str| HeroSetup { race: race.into(), ..hero() };
        let fights = |party: &[HeroSetup]| {
            let run = Run::new(content.clone(), "test_factions", RunOptions::default(), party, Box::new(Flat), 7).unwrap();
            (run.zone().enemies[0].name.clone(), run.zone().bosses[0].name.clone())
        };
        assert_eq!(fights(&[of("human")]), ("Horde Grunt".into(), "Horde Warlord".into()));
        assert_eq!(fights(&[of("orc")]), ("Alliance Footman".into(), "Alliance Marshal".into()));
        assert_eq!(fights(&[of("human"), of("orc"), of("orc")]).0, "Alliance Footman", "two Horde heroes of three");
        assert_eq!(fights(&[of("orc"), of("human")]).0, "Alliance Footman", "a tie: the first hero's");
        // the shared content keeps the zone with both rosters
        assert!(content.zone("test_factions").unwrap().by_faction.is_some());
    }

    #[test]
    fn killing_the_final_boss_wins() {
        let mut run = elwynn();
        start(&mut run);
        run.spawn_boss(4);
        let id = run.enemies.last().unwrap().id;
        run.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Other);
        assert_eq!((run.phase, run.won_at), (Phase::Running, Some(run.now)), "the moment is kept");
        // a choice the kill brought comes first
        run.heroes[0].pending_choices = 1;
        run.open_pending_screen();
        run.tick(0.05);
        assert_eq!(run.phase, Phase::Choosing);
        run.take_first_offers();
        let at = run.now;
        run.tick(0.05);
        assert_eq!((run.phase, run.now), (Phase::Victory, at), "won at the next running tick");
        assert!(run.drain_events().contains(&Event::Victory));
        assert!(run.continue_endless());
        assert_eq!(run.phase, Phase::Running);
        run.tick(0.05);
        assert_eq!(run.phase, Phase::Running, "Endless goes on");
        assert!(!run.continue_endless(), "only from a victory");
    }

    #[test]
    fn enemies_first_think_after_a_short_random_wait_and_bosses_arrive_held() {
        let mut run = elwynn();
        start(&mut run);
        run.enemies.clear();
        let p = run.heroes[0].pos;
        for i in 0..12 {
            assert!(run.spawn_roster_enemy(0, Some((p.0 + 10.0, p.1 + i as f32))));
        }
        let waits: Vec<f32> = run.enemies.iter().map(|e| e.next_decision - run.now).collect();
        assert!(waits.iter().all(|w| (0.0..FIRST_THOUGHT).contains(w)), "{waits:?}");
        assert!(waits.iter().any(|w| *w != waits[0]), "each draws its own wait");
        run.spawn_boss(0);
        let b = run.enemies.last().unwrap();
        assert_eq!(b.next_decision, run.now + BOSS_FIRST_THOUGHT);
        assert_eq!((b.hold_until, b.hold_chain_start), (run.now + BOSS_ARRIVAL_HOLD, run.now));
    }

    /// A voiced boss's pull voice line plays as it arrives, with its yell; an unvoiced one's
    /// arrival plays none.
    #[test]
    fn a_voiced_boss_speaks_its_pull_line_as_it_arrives() {
        let mut run = elwynn();
        start(&mut run);
        run.drain_events();
        run.spawn_boss(4);
        let events = run.drain_events();
        assert!(events.iter().any(|e| matches!(e, Event::Yell { .. })));
        assert!(events.iter().any(|e| matches!(e, Event::Sound(5780))), "VanCleef's own line");
        run.spawn_boss(0);
        assert!(!run.drain_events().iter().any(|e| matches!(e, Event::Sound(_))), "Hogger has no voice");
    }

    /// Kills the final boss at `at` and takes any choice it brings.
    fn kill_final_boss(run: &mut Run, at: f32) {
        run.now = at;
        run.spawn_boss(4);
        let id = run.enemies.last().unwrap().id;
        run.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Other);
    }

    #[test]
    fn endless_grows_from_the_latest_final_boss_death() {
        let mut run = elwynn();
        start(&mut run);
        kill_final_boss(&mut run, 600.0);
        for _ in 0..20 {
            run.tick(0.05);
            run.take_first_offers();
            if run.phase == Phase::Victory {
                break;
            }
        }
        assert_eq!(run.endless_minutes(), 0.0, "not in Endless yet");
        assert!(run.continue_endless());
        run.now = 720.0;
        assert!((run.endless_minutes() - 2.0).abs() < 1e-4);
        kill_final_boss(&mut run, 900.0);
        assert_eq!(run.endless_minutes(), 0.0, "a final boss's death starts it over");
        assert_eq!(run.won_at, Some(600.0), "the victory stays the first");
        run.now = 960.0;
        assert!((run.endless_minutes() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn in_endless_the_final_boss_may_return_in_the_second_before_a_swarm() {
        let mut run = elwynn();
        start(&mut run);
        let finals = |run: &Run| run.enemies.iter().filter(|e| e.kind == EnemyKind::Boss(4)).count();
        run.next_boss = run.zone().bosses.len();
        let swarm = run.director.next_swarm();
        // outside Endless, never
        run.now = swarm - 0.5;
        for _ in 0..2000 {
            run.spawning_tick(0.05);
        }
        assert_eq!(finals(&run), 0);
        run.endless_since = Some(0.0);
        // before the window, never
        run.now = swarm - 1.5;
        for _ in 0..2000 {
            run.spawning_tick(0.05);
        }
        assert_eq!(finals(&run), 0);
        // in the window, about one roll in 500
        run.now = swarm - 0.5;
        for _ in 0..10_000 {
            run.spawning_tick(0.05);
        }
        let n = finals(&run);
        assert!((8..=35).contains(&n), "{n} returns in 10000 rolls");
    }

    #[test]
    fn a_paused_run_stops_its_clock_until_it_runs_again() {
        let mut run = elwynn();
        assert!(!run.pause(), "nothing to pause before the run starts");
        start(&mut run);
        run.tick(0.05);
        let at = run.now;
        assert!(run.pause());
        assert_eq!(run.phase, Phase::Paused);
        run.tick(0.05);
        assert_eq!(run.now, at, "the clock stands");
        assert!(run.resume());
        assert_eq!(run.phase, Phase::Running);
        assert!(run.enemies.iter().all(|e| e.next_decision <= run.now), "every enemy decides at once");
        assert!(!run.resume());
        // a choice screen is not paused
        run.heroes[0].pending_choices = 1;
        run.tick(0.05);
        assert_eq!(run.phase, Phase::Choosing);
        assert!(!run.pause());
    }

    #[test]
    fn the_last_hero_to_leave_ends_the_run() {
        let mut run = elwynn();
        start(&mut run);
        run.now = 75.0;
        run.heroes[0].pending_choices = 1;
        run.tick(0.05);
        assert!(run.screen.is_some());
        let h = run.heroes[0].id;
        run.leave(h);
        assert!(run.screen.is_none(), "its choice goes with it");
        assert_eq!(run.phase, Phase::Defeat);
        assert!(run.is_over());
        assert_eq!(run.heroes[0].fell_at, Some(run.now), "its time ends as it leaves");
        // after a victory the run stays won
        let mut run = elwynn();
        start(&mut run);
        run.spawn_boss(4);
        let id = run.enemies.last().unwrap().id;
        run.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Other);
        run.tick(0.05);
        let h = run.heroes[0].id;
        run.leave(h);
        assert_eq!(run.phase, Phase::Victory);
    }

    #[test]
    fn an_extra_life_brings_the_hero_back_at_half_health() {
        let mut run = Run::new(
            content(),
            "elwynn",
            RunOptions::default(),
            &[HeroSetup { modifiers: vec![Modifier::new(Stat::ExtraLives, 1.0)], ..hero() }],
            Box::new(Flat),
            3,
        )
        .unwrap();
        start(&mut run);
        let max = run.heroes[0].stats.max_health;
        run.damage_hero(0, 1e9, None, false);
        assert!(run.heroes[0].alive);
        assert!((run.heroes[0].health - max * 0.5).abs() < 1e-3);
        run.damage_hero(0, 1e9, None, false);
        assert!(!run.heroes[0].alive);
    }

    /// Design step 8's "done when": a zone added with data alone (a file in `zones/`, no code)
    /// loads in its place and plays: a run starts in it, its roster spawns and its bosses come.
    #[test]
    fn a_zone_added_with_data_alone_plays() {
        use esurv_content::testing::{content_folder_with_zones, ExtraZone};
        let shipped = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let added = ExtraZone { id: "test_zone", name: "Test Zone", difficulty: 2.5 };
        let dir = content_folder_with_zones(shipped.as_ref(), "sim-added-zone", &[added]).unwrap();
        let content = Content::load(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        let content = Arc::new(content.expect("content with the added zone loads"));
        let mut run = Run::new(content.clone(), "test_zone", RunOptions::default(), &[hero()], Box::new(Flat), 7).unwrap();
        assert_eq!(content.zones[run.zone].id, "test_zone");
        assert_eq!(run.zone, 2, "between Area 10 and Warsong by its difficulty");
        start(&mut run);
        for _ in 0..50 {
            run.tick(0.1);
        }
        assert!(!run.enemies.is_empty(), "its roster spawns");
        run.spawn_boss(1);
        assert!(run.enemies.iter().any(|e| e.is_boss()), "its final boss comes");
        for _ in 0..20 {
            // levels come quickly: take the first card on every screen
            while let Some(screen) = &run.screen {
                let id = run.heroes[screen.hero].id;
                run.choose(id, 0);
            }
            run.tick(0.1);
        }
        assert!(matches!(run.phase, Phase::Running | Phase::Choosing), "{:?}", run.phase);
    }

    /// Level ground at one height, with a wall hiding everything north-east of whoever looks.
    struct WalledNorthEast(f32);

    impl World for WalledNorthEast {
        fn height(&self, _x: f32, _y: f32) -> Option<f32> {
            Some(self.0)
        }
        fn clear_line(&self, from: [f32; 3], to: [f32; 3]) -> bool {
            !(to[0] > from[0] && to[1] > from[1])
        }
    }

    #[test]
    fn a_spawn_spot_out_of_the_heros_sight_is_rolled_again() {
        let content = content();
        let z = content.zones.iter().find(|z| z.id == "elwynn").unwrap().spawn.z;
        let world = Box::new(WalledNorthEast(z));
        let mut run = Run::new(content, "elwynn", RunOptions::default(), &[hero()], world, 7).unwrap();
        run.heroes[0].height = z;
        let hero = run.heroes[0].pos;
        for _ in 0..200 {
            let spot = run.spawn_point(0, (30.0, 40.0));
            assert!(!(spot.0 > hero.0 && spot.1 > hero.1), "{spot:?} is behind the wall from {hero:?}");
        }
    }

    /// A dungeon floor at one height: everything north-east of the walker is another level with
    /// no way there, everything south-west is walked to the long way, 1.8 × the crow's line.
    struct LedgeNorthEast(f32);

    impl World for LedgeNorthEast {
        fn height(&self, _x: f32, _y: f32) -> Option<f32> {
            Some(self.0)
        }
        fn clear_line(&self, _from: [f32; 3], _to: [f32; 3]) -> bool {
            false
        }
        fn walk(&self, from: [f32; 3], to: Vec2, reach: f32) -> Walk {
            let distance = 1.8 * dist((from[0], from[1]), to);
            if (to.0 > from[0] && to.1 > from[1]) || distance > reach {
                return Walk::Unreachable;
            }
            Walk::Reached { distance, z: self.0 }
        }
    }

    #[test]
    fn in_a_dungeon_a_spawn_spot_is_walked_to_the_long_way_not_seen() {
        let content = content();
        let z = content.zones.iter().find(|z| z.id == "elwynn").unwrap().spawn.z;
        let world = Box::new(LedgeNorthEast(z));
        let mut run = Run::new(content, "elwynn", RunOptions::default(), &[hero()], world, 7).unwrap();
        run.heroes[0].height = z;
        let hero = run.heroes[0].pos;
        // nothing is in sight and every walk goes the long way round (another level up a ramp),
        // yet spots are found: drawn in the ring, walked to within twice its far edge, never on
        // the ledge
        for _ in 0..200 {
            let spot = run.spawn_point(0, (20.0, 40.0));
            assert!(!(spot.0 > hero.0 && spot.1 > hero.1), "{spot:?} is on the ledge");
            let crow = dist(hero, spot);
            assert!((20.0 - 0.01..=40.0 + 0.01).contains(&crow), "{spot:?} is {crow} yd out");
            assert!(1.8 * crow <= 40.0 * SPAWN_WALK_REACH, "{spot:?} is {} yd on foot", 1.8 * crow);
        }
    }

    /// Level ground at one height with a wall running north-south at `x`: nothing on or past it
    /// is in sight.
    struct WallAtX(f32, f32);

    impl World for WallAtX {
        fn height(&self, _x: f32, _y: f32) -> Option<f32> {
            Some(self.0)
        }
        fn clear_line(&self, _from: [f32; 3], to: [f32; 3]) -> bool {
            to[0] < self.1
        }
    }

    #[test]
    fn beside_a_wall_spawns_step_in_toward_the_hero_but_not_too_near() {
        let content = content();
        let z = content.zones.iter().find(|z| z.id == "elwynn").unwrap().spawn.z;
        let mut run = Run::new(content.clone(), "elwynn", RunOptions::default(), &[hero()], Box::new(Flat), 7).unwrap();
        let hero = run.heroes[0].pos;
        let wall = hero.0 + 25.0;
        run.world = Box::new(WallAtX(z, wall));
        run.heroes[0].height = z;
        let ring = (30.0, 40.0);
        let mut closer_toward_wall = 0;
        for _ in 0..400 {
            let spot = run.spawn_point(0, ring);
            let d = dist(hero, spot);
            assert!(spot.0 < wall, "{spot:?} is in or behind the wall at x {wall}");
            assert!(d >= spawn_inner_floor(ring) - 0.01, "{spot:?} is only {d} yd out");
            if spot.0 > hero.0 + 10.0 && d < ring.0 {
                closer_toward_wall += 1;
            }
        }
        assert!(closer_toward_wall > 0, "no spawn came in on the wall's side");
    }

    #[test]
    fn a_wall_12_yd_away_still_gets_spawns_on_its_side_from_15_yd() {
        let content = content();
        let z = content.zones.iter().find(|z| z.id == "elwynn").unwrap().spawn.z;
        let mut run = Run::new(content.clone(), "elwynn", RunOptions::default(), &[hero()], Box::new(Flat), 7).unwrap();
        let hero = run.heroes[0].pos;
        let wall = hero.0 + 12.0;
        run.world = Box::new(WallAtX(z, wall));
        run.heroes[0].height = z;
        let ring = (30.0, 40.0);
        let mut on_wall_side = 0;
        for _ in 0..400 {
            let spot = run.spawn_point(0, ring);
            let d = dist(hero, spot);
            assert!(spot.0 < wall, "{spot:?} is in or behind the wall at x {wall}");
            assert!(d >= spawn_inner_floor(ring) - 0.01, "{spot:?} is only {d} yd out");
            if spot.0 > hero.0 + 5.0 {
                on_wall_side += 1;
            }
        }
        assert!(on_wall_side > 0, "no spawn came on the wall's side");
    }

    #[test]
    fn on_open_ground_spawns_stay_in_the_full_ring() {
        let content = content();
        let mut run = Run::new(content, "elwynn", RunOptions::default(), &[hero()], Box::new(Flat), 7).unwrap();
        run.heroes[0].height = 0.0;
        let hero = run.heroes[0].pos;
        for _ in 0..400 {
            let d = dist(hero, run.spawn_point(0, (30.0, 40.0)));
            assert!((30.0 - 0.01..=40.0 + 0.01).contains(&d), "{d} yd out");
        }
    }

    /// A dungeon whose walk always bends through one doorway.
    struct Doorway(f32, Vec2);

    impl World for Doorway {
        fn height(&self, _x: f32, _y: f32) -> Option<f32> {
            Some(self.0)
        }
        fn toward(&self, _from: Vec2, _hero: [f32; 3]) -> Option<Vec2> {
            Some(self.1)
        }
    }

    #[test]
    fn in_a_dungeon_an_enemy_heads_for_the_next_stretch_of_the_walk_not_the_hero() {
        let content = content();
        let zone = content.zones.iter().find(|z| z.id == "elwynn").unwrap();
        let (z, at) = (zone.spawn.z, (zone.spawn.x + 20.0, zone.spawn.y));
        let door = (at.0 - 3.0, at.1 + 4.0);
        let mut run = Run::new(content.clone(), "elwynn", RunOptions::default(), &[hero()], Box::new(Doorway(z, door)), 7).unwrap();
        start(&mut run);
        run.enemies.clear();
        assert!(run.spawn_roster_enemy(0, Some(at)));
        for _ in 0..60 {
            if run.enemies[0].segment.to != at {
                break;
            }
            run.tick(0.05);
        }
        let seg = run.enemies[0].segment;
        assert!(dist(seg.to, door) < 0.01, "heads for the doorway {door:?}, not the hero: {:?}", seg.to);
    }
}
