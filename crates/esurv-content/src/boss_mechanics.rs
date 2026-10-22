//! The boss mechanics' building blocks as content (docs/game/bosses.md,
//! "Building blocks"): the shapes a marked attack covers, the lasting ground zones, the stacking
//! effects a zone or a bite leaves on a hero, the objects lying on the ground that a hero walks
//! over, and a boss's script (what it does at a health share, on a timer, on arrival or death).
//! Every one of them acts on the heroes only, except the scripts written to act on the
//! horde (Raise Dead). None stuns, fears, roots or slows a hero; the scripted pause is a short
//! cutscene that freezes everyone alike.

use serde::{Deserialize, Serialize};

use crate::kinds::BossAbilityEntry;
use crate::schema::{BossAbility, CreatureType, CreatureWeapons};

/// A spot on the ground (x, y), yards.
pub type Point = (f32, f32);

/// What a marked attack covers, laid from its origin along its facing (towards the hero when it
/// was cast).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    /// Everything within `radius` of the origin.
    Circle { radius: f32 },
    /// Between `inner` and `outer` yards of the origin: safe in the middle.
    Ring { inner: f32, outer: f32 },
    /// Within `length` of the origin and `angle` degrees wide (the whole width), along the facing.
    Cone { length: f32, angle: f32 },
    /// A strip `width` yards wide running `length` yards from the origin along the facing.
    Line { length: f32, width: f32 },
}

/// The longest a cone's or line's look is cut into (discs), so a big shape stays a few objects.
const MOST_PATCHES: usize = 28;

impl Shape {
    /// Whether `p` lies in the shape laid at `origin`, facing `facing` (radians).
    pub fn contains(&self, origin: Point, facing: f32, p: Point) -> bool {
        let (dx, dy) = (p.0 - origin.0, p.1 - origin.1);
        let d = (dx * dx + dy * dy).sqrt();
        match *self {
            Shape::Circle { radius } => d <= radius,
            Shape::Ring { inner, outer } => d >= inner && d <= outer,
            Shape::Cone { length, angle } => {
                if d > length {
                    return false;
                }
                if d < 0.5 {
                    return true;
                }
                let off = (dy.atan2(dx) - facing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                off.abs() <= angle.to_radians() / 2.0
            }
            Shape::Line { length, width } => {
                let (fx, fy) = (facing.cos(), facing.sin());
                let along = dx * fx + dy * fy;
                let across = (-dx * fy + dy * fx).abs();
                (0.0..=length).contains(&along) && across <= width / 2.0
            }
        }
    }

    /// How far from its origin the shape reaches.
    pub fn reach(&self) -> f32 {
        match *self {
            Shape::Circle { radius } => radius,
            Shape::Ring { outer, .. } => outer,
            Shape::Cone { length, .. } | Shape::Line { length, .. } => length,
        }
    }

    /// The discs (centre, radius) that cover the shape laid at `origin` facing `facing`: one for a
    /// circle, a ring of them for a ring, rows for a cone or a line. Its looks on the ground are
    /// put down on these (a ground look is round).
    pub fn patches(&self, origin: Point, facing: f32) -> Vec<(Point, f32)> {
        let at = |d: f32, a: f32| (origin.0 + d * a.cos(), origin.1 + d * a.sin());
        match *self {
            Shape::Circle { radius } => vec![(origin, radius)],
            Shape::Ring { inner, outer } => {
                let (mid, half) = ((inner + outer) / 2.0, ((outer - inner) / 2.0).max(0.5));
                let n = ((std::f32::consts::TAU * mid / (half * 1.4)).ceil() as usize).clamp(6, MOST_PATCHES);
                (0..n).map(|i| (at(mid, facing + i as f32 / n as f32 * std::f32::consts::TAU), half)).collect()
            }
            Shape::Cone { length, angle } => {
                let rho = (length / 6.0).clamp(1.5, 3.0);
                let half_angle = angle.to_radians() / 2.0;
                let mut out = Vec::new();
                let mut d = rho;
                while d <= length - rho * 0.4 && out.len() < MOST_PATCHES {
                    let arc = 2.0 * half_angle * d;
                    let m = ((arc / (rho * 1.5)).ceil() as usize).max(1);
                    for j in 0..m {
                        let a = if m == 1 { facing } else { facing - half_angle + 2.0 * half_angle * (j as f32 + 0.5) / m as f32 };
                        out.push((at(d, a), rho));
                    }
                    d += rho * 1.4;
                }
                out.truncate(MOST_PATCHES);
                out
            }
            Shape::Line { length, width } => {
                let r = (width / 2.0).max(0.5);
                let span = (length - 2.0 * r).max(0.0);
                let n = ((span / (r * 1.4)).ceil() as usize + 1).clamp(1, MOST_PATCHES);
                (0..n).map(|i| (at(r + if n > 1 { span * i as f32 / (n - 1) as f32 } else { 0.0 }, facing), r)).collect()
            }
        }
    }
}

/// Where a marked attack is laid.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MarkAt {
    /// On the hero's spot (a cone or line from there, along the boss's line to it).
    Hero,
    /// Around the boss (a cone or line from the boss, towards the hero).
    Boss,
    /// `count` marks: the first on the hero's spot, the rest up to `spread` yards around it.
    NearHero { count: u32, spread: f32 },
    /// `count` lines (or cones) laid across the ground at random facings, each through a spot:
    /// the first through the hero's, the rest up to `spread` yards round it (an avalanche rolling
    /// across the arena).
    Across { count: u32, spread: f32 },
}

/// What a marked attack brings where it lands: `count` of the zone's roster enemy `enemy`
/// bursting from the spot (a tentacle from the ground), staying where they burst if `rooted`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Brings {
    pub enemy: String,
    pub count: u32,
    #[serde(default)]
    pub rooted: bool,
}

/// A marked line that, once it lands, stays and sweeps round its boss (a beam): it turns `turn`
/// degrees a second (minus: the other way) for `seconds`, hurting a hero in it for
/// `damage_mult` × the boss's attack damage every `every` seconds, drawn with `look_spell` along
/// it. It never touches the horde.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sweep {
    pub turn: f32,
    pub seconds: f32,
    #[serde(default = "half_second")]
    pub every: f32,
    pub damage_mult: f32,
    pub look_spell: u32,
}

fn half_second() -> f32 {
    0.5
}

/// A stacking effect on a hero: each stack hurts `damage_mult` × the boss's attack damage every
/// `every` seconds while it lasts (`seconds`, renewed by every new stack), up to `max_stacks`
/// stacks. Damage only: it never slows or holds the hero. Shown on the hero as `look_spell`'s
/// aura.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Affliction {
    pub look_spell: u32,
    pub damage_mult: f32,
    pub every: f32,
    pub seconds: f32,
    pub max_stacks: u32,
}

/// A lasting ground zone: `look_spell`'s look, `radius` yards round, for `seconds` (0: while its
/// boss lives). Every `every` seconds it hurts each hero inside it for `damage_mult` × the boss's
/// attack damage, and adds a stack of `affliction` if it has one. It may `drift` towards the
/// nearest hero (yards per second), or `follow` its boss (an aura). It never touches the horde.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastingZone {
    pub look_spell: u32,
    pub radius: f32,
    pub seconds: f32,
    pub damage_mult: f32,
    #[serde(default = "every_second")]
    pub every: f32,
    #[serde(default)]
    pub drift: f32,
    #[serde(default)]
    pub follows: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affliction: Option<Affliction>,
}

fn every_second() -> f32 {
    1.0
}

/// An object lying on the ground that a hero walks over to break or set off:
/// standing on it (within `radius` yards) for `contact` seconds in one go does it, so
/// brushing past does not; its `glow_spell` look under it fills as the hero stands there. Hero
/// attacks never strike it. Left alone, it may `hatch` (eggs, cocoons); taken, it runs
/// `on_contact`, and comes back `again_after` seconds later if set (a relic that drops again).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundObject {
    /// Its name as shown (pointers such as `{Spell(..)}` allowed).
    pub name: String,
    /// Its model, a `GameObjectDisplayInfo.dbc` id.
    pub display_id: u32,
    #[serde(default = "one")]
    pub scale: f32,
    pub glow_spell: u32,
    #[serde(default = "standard_contact")]
    pub contact: f32,
    #[serde(default = "standard_touch")]
    pub radius: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hatch: Option<Hatch>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_contact: Vec<ScriptAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub again_after: Option<f32>,
    /// Announced each time it is put down.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warn: Option<Warning>,
    /// While it stands: every `seconds` its actions run from its spot (a totem healing round it,
    /// one laying marks round itself).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pulse: Option<Timed>,
    /// Left alone `seconds`, it goes and its actions run (an altar whose sacrifice heals its boss).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_alone: Option<Timed>,
    /// Taken as the last of its name its boss has down, these run too (the fourth fire put out).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_all_taken: Vec<ScriptAction>,
    /// Shown as a creature model instead (a `CreatureDisplayInfo.dbc` id: the troll wards and
    /// totems, creatures in 1.12), an unselectable unit standing there; `display_id` unused then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creature_display: Option<u32>,
}

/// Actions on a timer: every (or after) `seconds`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timed {
    pub seconds: f32,
    pub run: Vec<ScriptAction>,
}

/// Seconds a hero stands on an object to break or set it off.
pub const STANDARD_CONTACT: f32 = 0.5;
/// How near a hero's feet must be to an object to stand on it, yards.
pub const STANDARD_TOUCH: f32 = 2.0;

fn standard_contact() -> f32 {
    STANDARD_CONTACT
}

fn standard_touch() -> f32 {
    STANDARD_TOUCH
}

fn one() -> f32 {
    1.0
}

/// An object left `after` seconds hatches into `count` of the zone's roster enemy `enemy`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hatch {
    pub after: f32,
    pub enemy: String,
    pub count: u32,
}

/// Where an object is put down: this many yards from the boss (towards the hero) or from the
/// hero (to a side picked at random).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ObjectNear {
    Boss(f32),
    Hero(f32),
}

/// Where a wave of adds comes: on a ring this many yards round the boss, or from the arena's
/// edges (the enemy spawn ring round the hero).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum WaveAt {
    Ring(f32),
    Edges,
    /// In this many packs from far-apart sides of the edge, each pack's lane marked on the ground
    /// (pointing at the hero) a little before it runs in (Loksey's hounds).
    Sides(u32),
    /// On a ring this many yards round the hero.
    NearHero(f32),
    /// On a ring `spread` yards round a spot of the map (Stratholme's undead pouring in through
    /// the square's south gate).
    Spot { x: f32, y: f32, spread: f32 },
}

/// One wave of a boss's run of waves (`ScriptAction::Waves`): what it does as it comes (its adds,
/// a yell), and its announcement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveStep {
    pub run: Vec<ScriptAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warn: Option<Warning>,
}

/// One class's call in a boss's class call (`ScriptAction::ClassCall`): a hero of `class` (a
/// `classes.ron` id) is announced `warn` and, for the call's seconds, deals `weakens` less damage
/// (its class twisted; never crowd control), wearing `look_spell` as a debuff; `run` runs too
/// (adds round it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassCall {
    pub class: String,
    pub warn: Warning,
    #[serde(default)]
    pub weakens: f32,
    #[serde(default)]
    pub look_spell: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub run: Vec<ScriptAction>,
}

/// When a script line runs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ScriptTrigger {
    /// As the boss arrives.
    Arrival,
    /// Once, the first time its health is at or below this percentage.
    Health(f32),
    /// Every this many seconds after it arrives.
    Every(f32),
    /// Once, this many seconds after it arrives.
    After(f32),
    /// As it dies (where it fell).
    Death,
    /// Each time a roster enemy named in the line's `add` dies while it is up (at the add's spot).
    AddDied,
    /// Each time a hero loses a life (or falls) to it.
    HeroFell,
}

/// One line of a boss's script: when, and what it does then, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptLine {
    pub on: ScriptTrigger,
    pub run: Vec<ScriptAction>,
    /// Announced as it runs (a raid warning), and as a timer bar while it is on a timer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warn: Option<Warning>,
    /// For a health-share line: announced `SOON_AHEAD` points before it runs ("Lapdogs soon").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soon: Option<String>,
    /// For an `Every` line: it runs no more once the boss's health is below this percentage (a
    /// phase's own timer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until_health: Option<f32>,
    /// For an `Every` line: it runs only once the boss's health is at or below this percentage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_health: Option<f32>,
    /// For an `AddDied` line: the roster enemy whose deaths it waits for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub add: Option<String>,
}

/// What a script does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScriptAction {
    /// The boss shouts `text`; `sound` (a `SoundEntries.dbc` id, the original boss's voice) plays
    /// with it, and then the text is the line it speaks.
    Yell {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sound: Option<u32>,
    },
    /// A line across the middle of the screen; `%s` stands for the boss's name.
    Emote { text: String },
    /// `count` of the zone's roster enemy `enemy` come, at `at`.
    Wave { enemy: String, count: u32, at: WaveAt },
    /// The zone's door named `name` swings open (`open: true`) or shut again; while shut its
    /// doorway is a wall (Stratholme's Slaughterhouse door, opened as the Baron steps out).
    Door { name: String, open: bool },
    /// The zone's spawns stop (the director, the floor, elites, swarms) until `ResumeSpawns`.
    StopSpawns,
    ResumeSpawns,
    /// From now on, for the rest of the run, only roster enemies of this type spawn.
    Roster { only: CreatureType },
    /// Every living enemy that is not undead within `within` yards of a hero dies and rises at
    /// its spot as roster enemy `rise_as`, each with `look_spell`'s look (a script that acts
    /// on the horde).
    RaiseDead { within: f32, rise_as: String, look_spell: u32 },
    /// The boss is shielded: it takes `damage_taken` of every hit, wearing `look_spell`.
    Shield { damage_taken: f32, look_spell: u32 },
    /// The shield drops for `seconds`, then comes back.
    Mortal { seconds: f32 },
    /// An object is put down (`count` of them, fanned out as `Lay` does).
    Place {
        object: GroundObject,
        near: ObjectNear,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        count: Option<u32>,
    },
    /// A lasting zone is put down round the boss (an aura that `follows` it).
    LayZone { zone: LastingZone },
    /// The boss fights with these abilities from now on (a phase change), in place of its own.
    Kit {
        abilities: Vec<BossAbilityEntry>,
        #[serde(skip)]
        resolved: Vec<BossAbility>,
    },
    /// The boss holds these weapons from now on (`zones.md`, "What enemies hold").
    Weapons { weapons: CreatureWeapons },
    /// The boss takes this model (`CreatureDisplayInfo.dbc` id) from now on, at `scale` if set.
    Look {
        display_id: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scale: Option<f32>,
    },
    /// The boss kneels for `seconds`: it stands still and does nothing (a window to strike).
    Kneel { seconds: f32 },
    /// The boss enrages now (as below 30 %: faster, harder, abilities sooner).
    Enrage,
    /// The scripted pause: for `seconds` (5 at most) the whole fight freezes, the
    /// heroes, the enemies, the spawns and every timer, while `text` shows across the screen.
    Pause {
        seconds: f32,
        #[serde(default)]
        text: String,
    },
    /// These run `seconds` later (counted through a scripted pause too, so a cutscene can be
    /// timed), as the same boss.
    Later { seconds: f32, run: Vec<ScriptAction> },
    /// These run as the zone's boss named `boss`, if it is up (its shield, its yell, its enrage).
    On { boss: String, run: Vec<ScriptAction> },
    /// The zone's boss named `boss` comes now (one only a script brings: `called`), unless it is
    /// up already or this boss was itself brought back by `Revive`.
    Call { boss: String },
    /// The zone's boss named `boss`, fallen, rises where it fell with `health` of its health,
    /// wearing `look_spell`'s look as it rises.
    Revive { boss: String, health: f32, look_spell: u32 },
    /// The shield goes for good.
    Unshield,
    /// A heal of `share` of each healed one's health, `look_spell` playing on it: `from` each
    /// living roster enemy of that name (else from the boss, or the object whose pulse it is), on
    /// `on`. With `beam_spell`, a healer that is a unit channels that beam to it for
    /// `beam_seconds`.
    Heal {
        on: HealOn,
        share: f32,
        look_spell: u32,
        #[serde(default)]
        beam_spell: u32,
        #[serde(default = "beam_time")]
        beam_seconds: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
    },
    /// The boss casts this building block now (a marked attack, a zone), off its kit; from an
    /// object's pulse, laid from the object's spot.
    Cast {
        ability: BossAbilityEntry,
        #[serde(skip)]
        resolved: Option<BossAbility>,
    },
    /// The boss's lasting zones go (a form's aura, as it changes form).
    ClearZones,
    /// The boss wears `look_spell`'s look for `seconds` (Herod spinning while his Whirlwind
    /// lasts).
    Wear { look_spell: u32, seconds: f32 },
    /// The boss moves at this many yards a second from now on (× the speed run option).
    Speed { yards: f32 },
    /// Each living roster enemy named `enemy` (the boss itself when left out) hits `damage` times
    /// as hard and moves `speed` times as fast from now on, wearing `look_spell`'s look.
    Bolster {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        enemy: Option<String>,
        damage: f32,
        speed: f32,
        look_spell: u32,
    },
    /// Blood Siphon (after Hakkar's): the boss drains every hero and
    /// every enemy at once, channelling `beam_spell` to each hero for `seconds`. A hero takes
    /// `damage_mult` × its attack damage and heals it `heal_share` of its health, unless the hero
    /// carries `poison_spell`'s stack: then the beam is `poisoned_beam_spell` and the boss loses
    /// `poisoned_share` of its health instead. Each other enemy loses `enemy_share` of its health
    /// and heals the boss `enemy_heal_share` of its own.
    Siphon {
        damage_mult: f32,
        heal_share: f32,
        poison_spell: u32,
        poisoned_share: f32,
        enemy_share: f32,
        enemy_heal_share: f32,
        beam_spell: u32,
        poisoned_beam_spell: u32,
        seconds: f32,
    },
    /// A run of waves (a gate event, a guard of adds): each wave runs in turn, the next once the
    /// last one's adds are all dead, or `gap` seconds after it came (0: only once they are dead).
    /// Meanwhile the boss takes no damage if `immune` (wearing `look_spell`), and with `aloof`
    /// stands apart doing nothing. A timer bar `bar` counts to the next wave (" 2/7" after it),
    /// and a counter `counter` shows the adds left (" 5/8"). Once the last wave's adds are dead
    /// (or its gap ran out), `then` runs and the boss fights.
    Waves {
        waves: Vec<WaveStep>,
        #[serde(default)]
        gap: f32,
        #[serde(default)]
        immune: bool,
        #[serde(default)]
        aloof: bool,
        #[serde(default)]
        look_spell: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bar: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        counter: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        then: Vec<ScriptAction>,
    },
    /// Every enemy of the horde within `within` yards of the boss is pulled into one clump
    /// `ahead` yards in front of it (towards the hero), with `look_spell` round the boss; never
    /// a hero (Arcane Vacuum: a burst opportunity).
    Gather { within: f32, ahead: f32, look_spell: u32 },
    /// For `seconds` the boss takes no spell damage (weapon swings and pets still hurt it),
    /// wearing `look_spell` (Reflection).
    SpellImmune { seconds: f32, look_spell: u32 },
    /// A stack of rage (Bloodrage, a frenzy): per stack its abilities come `faster` sooner, it
    /// moves `faster` quicker and its marked attacks and zones hit `harder` harder, up to
    /// `max_stacks`; for `seconds` (0: for good), wearing `look_spell`. Its swing stays as it is
    /// (the damage a hero cannot avoid stays).
    Empower {
        look_spell: u32,
        faster: f32,
        harder: f32,
        #[serde(default = "one_stack")]
        max_stacks: u32,
        #[serde(default)]
        seconds: f32,
    },
    /// Each living roster enemy named `by` heals the boss for `share` of its health, with
    /// `look_spell` from it to the boss (Medics, Princess Moira): kill them to stop it.
    Mend { by: String, share: f32, look_spell: u32 },
    /// A class call: each hero's class's call (`ClassCall`) twists it for `seconds` (no crowd
    /// control); a class without a call is spared.
    ClassCall { calls: Vec<ClassCall>, seconds: f32 },
    /// At each spot where an add of the boss's waves fell, `rise_as` rises (`most` at most), with
    /// `look_spell` (Nefarian raising the drakonids).
    RaiseFallen { rise_as: String, look_spell: u32, most: u32 },
    /// A figure rises for a moment (Ragnaros): `delay` seconds later, roster enemy `enemy` rises
    /// at `at` round where the boss is (or fell), playing `rise_spell`; it takes no damage, never
    /// moves or swings, runs `run` (its yell) and fights with `abilities`, then after `seconds`
    /// sinks again (`sink_spell`), giving nothing.
    Cameo {
        enemy: String,
        at: WaveAt,
        #[serde(default)]
        delay: f32,
        seconds: f32,
        #[serde(default)]
        rise_spell: u32,
        #[serde(default)]
        sink_spell: u32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        run: Vec<ScriptAction>,
        abilities: Vec<BossAbilityEntry>,
        #[serde(skip)]
        resolved: Vec<BossAbility>,
    },
}

fn one_stack() -> u32 {
    1
}

fn beam_time() -> f32 {
    2.0
}

/// Whom a `Heal` heals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealOn {
    /// The boss whose script it is.
    Myself,
    /// Every enemy (bosses too) within this many yards of the healer, the healer left out.
    Near(f32),
    /// The zone's boss of this name.
    Boss(String),
}


/// A raid-warning announcement (clear warnings for each mechanic, in our
/// own words): big centre-screen `text` in `colour` with `icon_spell`'s icon (the ability's own
/// look when left out) and the raid warning sound, as the mechanic starts; and, for an ability, a
/// timer bar labelled `bar` (the text when left out) counting down to its next use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Warning {
    pub text: String,
    #[serde(default)]
    pub colour: WarningColour,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_spell: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bar: Option<String>,
}

impl Warning {
    /// Its timer bar's label.
    pub fn bar_label(&self) -> &str {
        self.bar.as_deref().unwrap_or(&self.text)
    }
}

/// A warning's colour.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarningColour {
    /// Move out: an attack about to land.
    #[default]
    Red,
    /// Adds coming.
    Orange,
    /// Something to do: grab, break.
    Yellow,
    /// A phase change.
    Purple,
    /// Poison, plague.
    Green,
    Blue,
}

impl WarningColour {
    /// Linear 0-1 RGB.
    pub fn rgb(self) -> [f32; 3] {
        match self {
            WarningColour::Red => [1.0, 0.18, 0.12],
            WarningColour::Orange => [1.0, 0.55, 0.1],
            WarningColour::Yellow => [1.0, 0.86, 0.2],
            WarningColour::Purple => [0.75, 0.35, 1.0],
            WarningColour::Green => [0.35, 1.0, 0.3],
            WarningColour::Blue => [0.35, 0.65, 1.0],
        }
    }
}

/// A health-share event's early warning comes this many percentage points before it (at 55 % for
/// a 50 % event).
pub const SOON_AHEAD: f32 = 5.0;

/// The longest a scripted pause may last.
pub const LONGEST_PAUSE: f32 = 5.0;

impl ScriptAction {
    /// The roster enemies this action names (to check they exist), its own and its objects'.
    pub fn enemies_named(&self) -> Vec<&str> {
        match self {
            ScriptAction::Wave { enemy, .. } => vec![enemy.as_str()],
            ScriptAction::RaiseDead { rise_as, .. } | ScriptAction::RaiseFallen { rise_as, .. } => vec![rise_as.as_str()],
            ScriptAction::Mend { by, .. } => vec![by.as_str()],
            ScriptAction::Place { object, .. } => object.enemies_named(),
            ScriptAction::Kit { resolved, .. } => resolved.iter().flat_map(|a| a.kind.enemies_named()).collect(),
            ScriptAction::Cast { resolved, .. } => resolved.iter().flat_map(|a| a.kind.enemies_named()).collect(),
            ScriptAction::Heal { from: Some(n), .. } | ScriptAction::Bolster { enemy: Some(n), .. } => vec![n.as_str()],
            ScriptAction::Cameo { enemy, run, resolved, .. } => std::iter::once(enemy.as_str())
                .chain(run.iter().flat_map(|a| a.enemies_named()))
                .chain(resolved.iter().flat_map(|a| a.kind.enemies_named()))
                .collect(),
            other => other.nested().into_iter().flat_map(|a| a.enemies_named()).collect(),
        }
    }

    /// The zone's bosses this action names (to check they exist).
    pub fn bosses_named(&self) -> Vec<&str> {
        match self {
            ScriptAction::On { boss, run } => std::iter::once(boss.as_str()).chain(run.iter().flat_map(|a| a.bosses_named())).collect(),
            ScriptAction::Call { boss } | ScriptAction::Revive { boss, .. } | ScriptAction::Heal { on: HealOn::Boss(boss), .. } => vec![boss.as_str()],
            ScriptAction::Place { object, .. } => object.action_lists().flatten().flat_map(|a| a.bosses_named()).collect(),
            other => other.nested().into_iter().flat_map(|a| a.bosses_named()).collect(),
        }
    }

    /// The zone's doors this action opens or shuts (to check they exist), its own and those it
    /// holds.
    pub fn doors_named(&self) -> Vec<&str> {
        match self {
            ScriptAction::Door { name, .. } => vec![name.as_str()],
            other => other.nested().into_iter().flat_map(|a| a.doors_named()).collect(),
        }
    }

    /// The actions this one holds (a run of waves', a class call's, a later run's), to walk them.
    pub fn nested(&self) -> Vec<&ScriptAction> {
        match self {
            ScriptAction::Waves { waves, then, .. } => waves.iter().flat_map(|w| &w.run).chain(then).collect(),
            ScriptAction::ClassCall { calls, .. } => calls.iter().flat_map(|c| &c.run).collect(),
            ScriptAction::Later { run, .. } | ScriptAction::On { run, .. } => run.iter().collect(),
            _ => Vec::new(),
        }
    }
}

impl GroundObject {
    /// The roster enemies it hatches into or its scripts name.
    pub fn enemies_named(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self.hatch.iter().map(|h| h.enemy.as_str()).collect();
        out.extend(self.action_lists().flatten().flat_map(|a| a.enemies_named()));
        out
    }

    /// Its scripts: what it runs taken, taken as the last, on its pulse and left alone.
    pub fn action_lists(&self) -> impl Iterator<Item = &Vec<ScriptAction>> {
        [Some(&self.on_contact), Some(&self.on_all_taken), self.pulse.as_ref().map(|p| &p.run), self.left_alone.as_ref().map(|p| &p.run)].into_iter().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_shape_holds_what_it_covers() {
        let o = (0.0, 0.0);
        assert!(Shape::Circle { radius: 5.0 }.contains(o, 0.0, (3.0, 4.0)));
        assert!(!Shape::Circle { radius: 5.0 }.contains(o, 0.0, (4.0, 4.0)));
        let ring = Shape::Ring { inner: 4.0, outer: 10.0 };
        assert!(!ring.contains(o, 0.0, (1.0, 0.0)), "safe in the middle");
        assert!(ring.contains(o, 0.0, (0.0, -7.0)));
        assert!(!ring.contains(o, 0.0, (11.0, 0.0)));
        // a cone facing +y, 60 degrees wide
        let cone = Shape::Cone { length: 10.0, angle: 60.0 };
        let up = std::f32::consts::FRAC_PI_2;
        assert!(cone.contains(o, up, (0.0, 8.0)));
        assert!(cone.contains(o, up, (2.0, 8.0)));
        assert!(!cone.contains(o, up, (6.0, 6.0)), "45 degrees off: outside");
        assert!(!cone.contains(o, up, (0.0, -3.0)), "behind");
        let line = Shape::Line { length: 20.0, width: 4.0 };
        assert!(line.contains(o, 0.0, (15.0, 1.9)));
        assert!(!line.contains(o, 0.0, (15.0, 2.1)));
        assert!(!line.contains(o, 0.0, (-1.0, 0.0)));
    }

    #[test]
    fn the_patches_lie_inside_their_shape_and_cover_it() {
        let o = (10.0, -5.0);
        for (shape, facing) in [
            (Shape::Circle { radius: 8.0 }, 0.3),
            (Shape::Ring { inner: 6.0, outer: 12.0 }, 1.0),
            (Shape::Cone { length: 14.0, angle: 70.0 }, -2.0),
            (Shape::Line { length: 25.0, width: 5.0 }, 2.5),
        ] {
            let patches = shape.patches(o, facing);
            assert!(!patches.is_empty() && patches.len() <= MOST_PATCHES, "{shape:?}");
            for (c, _) in &patches {
                assert!(shape.contains(o, facing, *c), "{shape:?}: a patch centre outside");
            }
        }
        assert_eq!(Shape::Circle { radius: 8.0 }.patches(o, 0.0), vec![(o, 8.0)]);
    }
}

impl crate::Boss {
    /// Every object this boss may put down: its abilities' (and its phases' kits'), its
    /// script's, and what those objects' own scripts place.
    pub fn ground_objects(&self) -> Vec<&GroundObject> {
        fn of_kind<'a>(k: &'a crate::BossAbilityKind, out: &mut Vec<&'a GroundObject>) {
            match k {
                crate::BossAbilityKind::Marked { leaves_object: Some(o), .. } | crate::BossAbilityKind::Lay { object: o, .. } => of_object(o, out),
                _ => {}
            }
        }
        fn of_object<'a>(o: &'a GroundObject, out: &mut Vec<&'a GroundObject>) {
            out.push(o);
            for a in o.action_lists().flatten() {
                of_action(a, out);
            }
        }
        fn of_action<'a>(a: &'a ScriptAction, out: &mut Vec<&'a GroundObject>) {
            match a {
                ScriptAction::Place { object, .. } => of_object(object, out),
                ScriptAction::Kit { resolved, .. } | ScriptAction::Cameo { resolved, .. } => resolved.iter().for_each(|b| of_kind(&b.kind, out)),
                ScriptAction::Cast { resolved: Some(b), .. } => of_kind(&b.kind, out),
                other => other.nested().into_iter().for_each(|n| of_action(n, out)),
            }
        }
        let mut out = Vec::new();
        for a in &self.abilities {
            of_kind(&a.kind, &mut out);
        }
        for l in &self.script {
            for a in &l.run {
                of_action(a, &mut out);
            }
        }
        out
    }

    /// Every kit this boss fights with: its own abilities and each phase's (`Kit`, wherever its
    /// script holds one, a run of waves' end included).
    pub fn kits(&self) -> Vec<Vec<BossAbility>> {
        fn walk(a: &ScriptAction, out: &mut Vec<Vec<BossAbility>>) {
            if let ScriptAction::Kit { resolved, .. } = a {
                out.push(resolved.clone());
            }
            for n in a.nested() {
                walk(n, out);
            }
        }
        let mut out = vec![self.abilities.clone()];
        for a in self.script.iter().flat_map(|l| &l.run) {
            walk(a, &mut out);
        }
        out
    }

    /// The kits of the figures its script raises (`Cameo`: Ragnaros's smash).
    pub fn cameo_kits(&self) -> Vec<Vec<BossAbility>> {
        fn walk(a: &ScriptAction, out: &mut Vec<Vec<BossAbility>>) {
            if let ScriptAction::Cameo { resolved, .. } = a {
                out.push(resolved.clone());
            }
            for n in a.nested() {
                walk(n, out);
            }
        }
        let mut out = Vec::new();
        for a in self.script.iter().flat_map(|l| &l.run) {
            walk(a, &mut out);
        }
        out
    }
}
