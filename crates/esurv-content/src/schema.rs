//! The record types. Units: seconds, yards, per-second rates, and fractions where the UI shows
//! percent (0.4 = 40 %).

use serde::{Deserialize, Serialize};

use crate::names::NameRef;
use esurv_protocol::BuffKind;

// ---- zones -----------------------------------------------------------------------------------

/// A playable arena: where a run happens, what attacks, and when the bosses come.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub description: String,
    /// `Map.dbc` id the hero is placed on.
    pub map_id: u32,
    /// `AreaTable.dbc` id reported for the hero's position.
    pub area_id: u32,
    pub spawn: Spawn,
    /// Radius of the arena around its centre (`arena_centre`, else `spawn`), yards. Heroes may
    /// roam 10 yd past it.
    pub arena_radius: f32,
    /// The arena circle's centre when it is not the spawn (a walled arena whose spawn is at one
    /// end).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arena_centre: Option<(f32, f32)>,
    /// Lines nobody crosses: heroes are put back, enemies walk round and spawn on the hero's side
    /// (the doorway behind the Scarlet Monastery's spawn).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub walls: Vec<Wall>,
    /// Doors shut at the start that open on the run clock; shut, a door is a wall.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doors: Vec<Door>,
    /// Map objects of our own shown in the arena, solid to the client where their model is
    /// (a round table plugging the Sunken Temple's pit).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub props: Vec<Prop>,
    /// Floor laid over a hole (a disc at a height): the run walks, spawns and stands on it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<Platform>,
    /// How far from a hero roster enemies appear, (nearest, farthest) yards; a spot past the
    /// arena is pulled back onto its edge.
    #[serde(default = "enemy_spawn_ring")]
    pub enemy_spawn_ring: (f32, f32),
    /// The same for bosses.
    #[serde(default = "boss_spawn_ring")]
    pub boss_spawn_ring: (f32, f32),
    /// Server time of day the run is set to, 0..23.
    pub hour: u32,
    pub weather: Weather,
    /// The zone's difficulty D, its place in the unlock order (1 in Elwynn to 10 in `aq`; a zone
    /// added between two may take a step between theirs): every enemy's health grows
    /// with D, and so do gear, drops and talent experience (`balance.ron` `zone_scaling`).
    pub difficulty: f32,
    /// The roster, in unlock order.
    pub enemies: Vec<Enemy>,
    /// The boss schedule, in spawn order; one is the final boss.
    pub bosses: Vec<Boss>,
    /// A roster for each faction the heroes may be (Warsong Gulch
    /// fights the other faction): a run takes the one its party's faction meets in place of
    /// `enemies` and `bosses` (`Zone::for_faction`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by_faction: Option<FactionRosters>,
}

/// A zone's roster for each faction: `vs_alliance` is what Alliance heroes fight, `vs_horde`
/// what Horde heroes fight.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionRosters {
    pub vs_alliance: Roster,
    pub vs_horde: Roster,
}

/// One roster: its enemies and its boss schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Roster {
    pub enemies: Vec<Enemy>,
    pub bosses: Vec<Boss>,
}

impl Zone {
    /// The zone as heroes of `faction` fight it: its roster for that faction in place of its
    /// own, when it has one for each.
    pub fn for_faction(&self, faction: Faction) -> Zone {
        let mut zone = self.clone();
        if let Some(rosters) = &self.by_faction {
            let roster = match faction {
                Faction::Alliance => &rosters.vs_alliance,
                Faction::Horde => &rosters.vs_horde,
            };
            zone.enemies = roster.enemies.clone();
            zone.bosses = roster.bosses.clone();
        }
        zone
    }

    /// Every roster enemy of the zone, its faction rosters' included.
    pub fn every_enemy(&self) -> impl Iterator<Item = &Enemy> {
        let factions = self.by_faction.iter().flat_map(|r| r.vs_alliance.enemies.iter().chain(&r.vs_horde.enemies));
        self.enemies.iter().chain(factions)
    }

    /// Every boss of the zone, its faction rosters' included.
    pub fn every_boss(&self) -> impl Iterator<Item = &Boss> {
        let factions = self.by_faction.iter().flat_map(|r| r.vs_alliance.bosses.iter().chain(&r.vs_horde.bosses));
        self.bosses.iter().chain(factions)
    }
}

/// The faction a party fights as: the one most of its heroes' races share, the first hero's on a
/// tie; the Alliance for none.
pub fn party_faction(factions: &[Faction]) -> Faction {
    let horde = factions.iter().filter(|f| **f == Faction::Horde).count();
    let alliance = factions.len() - horde;
    match alliance.cmp(&horde) {
        std::cmp::Ordering::Greater => Faction::Alliance,
        std::cmp::Ordering::Less => Faction::Horde,
        std::cmp::Ordering::Equal => factions.first().copied().unwrap_or(Faction::Alliance),
    }
}

/// The one enemy stat formula's numbers (`balance.ron`). An enemy that
/// appears at run minute m in a zone of difficulty D has
/// `health = base_health × kind health × zone health(D) × health clock(m) × toughness × options × endless` and
/// `damage = base_damage × kind damage × zone health(D)^share × damage clock(m) × options × endless`,
/// the zone health from `zone_scaling`: `normal_health` at D, to `elite_health_power` for elites and
/// rares or `boss_health_power` for bosses; the share `damage_share` (normal enemies) or
/// `elite_boss_damage_share`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    /// Every enemy's movement speed (normal enemies, bosses, rares) times this: one knob for
    /// the whole game; 1 when the file leaves it out.
    #[serde(default = "one")]
    pub enemy_speed: f32,
    /// Health and damage every enemy kind starts from, for the whole game.
    pub base_health: f32,
    pub base_damage: f32,
    /// Everything that follows the zone's difficulty D: enemy growth, gear, drops, talent experience.
    pub zone_scaling: ZoneScaling,
    /// What each kind of enemy multiplies the base values by.
    pub normal: TypeMultipliers,
    pub elite: TypeMultipliers,
    pub rare: TypeMultipliers,
    pub boss: TypeMultipliers,
    pub final_boss: TypeMultipliers,
    /// The run clocks: (minute, multiplier) points, straight lines between them, and past the
    /// last point the last step's pace goes on.
    pub health_clock: Vec<(f32, f32)>,
    pub damage_clock: Vec<(f32, f32)>,
    /// Bosses and final bosses read the health clock to this power (1 when the file leaves it
    /// out): below 1 a boss that comes late in the run grows less than the swarm, so a zone's
    /// bosses last about as long as each other.
    #[serde(default = "one")]
    pub boss_clock_power: f32,
    /// Spawn budget per second at 100 % density, by run minute: (minute, budget) points read the
    /// same way as the clocks. Density multiplies it; bosses alive and Endless change it as before.
    pub spawn_budget: Vec<(f32, f32)>,
    /// Most normal enemies alive at once: `live_cap.0` at 100 % density, rising in a straight line
    /// with the density option to `live_cap.1` at its highest setting (500 %).
    pub live_cap: (f32, f32),
    /// What kills drop as experience, what each level costs and how orbs look.
    pub experience: ExperienceRules,
    /// One multiplier per spell tag; all 1 when missing.
    #[serde(default)]
    pub spell_tags: SpellTagCoefficients,
    /// The battle standard pickup, which summons an assist.
    #[serde(default)]
    pub battle_standard: BattleStandardRules,
    /// Challenge Mode (docs/game/zones.md, "Challenge Mode"): every enemy's final health
    /// and damage are × this × the challenge slider, outside the kind and zone powers; a challenge
    /// run's talent experience is × this (never the slider). 1 when the file leaves it out.
    #[serde(default = "one")]
    pub challenge_factor: f32,
}

/// The battle standard (`balance.ron` `battle_standard`; docs/game/assists.md): a pickup lying on
/// the caches' ring until a hero walks over it, which summons a hand-made player character of the
/// hero's faction to assist. A run lays `per_run.0` to `per_run.1` of them (picked at random each
/// run), each at a random time between the first elite and the final boss; in Endless one more
/// comes every `endless_every` seconds. None ever lies in the zones `never_in`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattleStandardRules {
    pub per_run: (u32, u32),
    pub endless_every: f32,
    #[serde(default)]
    pub never_in: Vec<String>,
    /// Its look, the hero's faction's Alterac battle standard planted in the ground
    /// (`CreatureDisplayInfo.dbc`: Alliance 14541, Horde 14542).
    pub display: FactionDisplay,
    /// The look played on it as it is planted: its faction's Battle Standard spell (`Spell.dbc`).
    pub planted_look: FactionDisplay,
    /// The summoned ally is drawn this many times its normal size.
    pub ally_scale: f32,
    /// An ally stays this long (seconds) unless its script gives another `seconds`: its assist,
    /// then its rotation.
    pub assist_seconds: f32,
    /// What every assist deals, per second, as a share of the struck enemy's max health.
    pub assist_damage_per_second: AssistDamageRates,
}

/// An assist's damage rates (`battle_standard.assist_damage_per_second`; docs/game/assists.md,
/// "Its damage"): shares of the struck enemy's max health a second, the same for every assist.
/// Each damage event deals the rate for the enemy's kind × its max health × the seconds the event
/// stands for (a swing's attack time, a tick's interval, a cast's cycle, one global cooldown).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AssistDamageRates {
    /// On a normal enemy.
    pub normal: f32,
    /// On an elite, a rare or a boss.
    pub elite_rare_boss: f32,
}

impl Default for BattleStandardRules {
    /// The shipped rules, with no zone left out (content built in code may lack Warsong).
    fn default() -> Self {
        Self {
            per_run: (1, 2),
            endless_every: 600.0,
            never_in: Vec::new(),
            display: FactionDisplay { alliance: 14541, horde: 14542 },
            planted_look: FactionDisplay { alliance: 23034, horde: 23035 },
            ally_scale: 2.0,
            assist_seconds: 15.0,
            assist_damage_per_second: AssistDamageRates { normal: 0.05, elite_rare_boss: 0.01 },
        }
    }
}

/// A look for each faction: `alliance` for Alliance heroes, `horde` for Horde heroes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactionDisplay {
    pub alliance: u32,
    pub horde: u32,
}

impl FactionDisplay {
    /// The look of `faction`.
    pub fn of(self, faction: Faction) -> u32 {
        match faction {
            Faction::Alliance => self.alliance,
            Faction::Horde => self.horde,
        }
    }
}

/// The run's experience numbers (`balance.ron`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExperienceRules {
    /// A normal kill drops `normal_kill × 100 ÷ density %`, rounded at random so the average is
    /// exact.
    pub normal_kill: f32,
    /// An elite kill drops this, whatever the density.
    pub elite_kill: u32,
    /// A boss kill drops `boss_kill.0 + boss_kill.1 × bosses already slain`.
    pub boss_kill: (u32, u32),
    /// Experience to go from level L to L + 1:
    /// `round(level_cost.constant + level_cost.factor × (L − 1)^level_cost.power)`.
    pub level_cost: LevelCost,
    /// Orb looks by value: below `orb_medium` small, below `orb_large` medium, else large.
    pub orb_medium: u32,
    pub orb_large: u32,
}

/// The level cost curve's numbers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LevelCost {
    pub constant: f32,
    pub factor: f32,
    pub power: f32,
    /// No level costs more than this (none when missing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f32>,
}

impl ExperienceRules {
    /// Experience to go from `level` to `level + 1`.
    pub fn to_next(&self, level: u32) -> u32 {
        let l = level.saturating_sub(1) as f32;
        let c = self.level_cost;
        (c.constant + c.factor * l.powf(c.power)).min(c.max.unwrap_or(f32::INFINITY)).round() as u32
    }
}

impl Default for ExperienceRules {
    /// The game's numbers, for content built in code.
    fn default() -> Self {
        Self {
            normal_kill: 100.0,
            elite_kill: 1000,
            boss_kill: (2400, 800),
            level_cost: LevelCost { constant: 200.0, factor: 65.0, power: 1.5, max: Some(20000.0) },
            orb_medium: 200,
            orb_large: 1000,
        }
    }
}


/// Everything that follows a zone's difficulty D (`balance.ron` `zone_scaling`).
/// D is the zone's place in the unlock order (1 for the first zone), and every number here
/// is built on the hero we expect a player to bring to that zone, never on a hero's actual power.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneScaling {
    /// A normal enemy's health multiplier by zone difficulty: (D, multiplier) points joined by
    /// straight lines; past the last point the last step's pace goes on (as the run clocks).
    pub normal_health: Vec<(f32, f32)>,
    /// Elites and rares take the normal health multiplier to this power: a gentler growth.
    pub elite_health_power: f32,
    /// Bosses and final bosses take it to this power, gentler still, so a boss fight keeps its length.
    pub boss_health_power: f32,
    /// A normal enemy's damage multiplier is its health multiplier to this power.
    pub damage_share: f32,
    /// The same for elites, rares and bosses, of their own health multiplier: their blows grow gentler.
    pub elite_boss_damage_share: f32,
    /// A gear piece's stats are its first-zone values × D^`gear_exponent`, rounded, up to
    /// `gear_steeper_from`; past it × `gear_growth_after` more for every step of D.
    pub gear_exponent: f32,
    pub gear_steeper_from: f32,
    pub gear_growth_after: f32,
    /// Each droppable piece drops from a boss at the chance of the last step whose D is at or below
    /// the zone's (no blending between steps; a D below the first step takes the first), × the
    /// run-option bonus: (D, chance) steps in rising D.
    pub gear_drop_chance: Vec<(f32, f32)>,
    /// A run's talent experience is × D^`talent_exponent` (the zone bonus).
    pub talent_exponent: f32,
    /// The D at which each gear tier after the first begins (a piece's name and look).
    pub gear_tier_starts: Vec<f32>,
    /// A normal enemy's damage multiplier by D, read as `normal_health` is: when set, damage follows
    /// it instead of the health multiplier (so a health table can change and blows stay as they
    /// were); elites and rares take it to `elite_damage_power`, bosses to `boss_damage_power`.
    /// Empty: damage is the health multiplier to `damage_share` / `elite_boss_damage_share`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub normal_damage: Vec<(f32, f32)>,
    #[serde(default = "one")]
    pub elite_damage_power: f32,
    #[serde(default = "one")]
    pub boss_damage_power: f32,
}

/// A (D, multiplier) table read at `d`: straight lines between the points, past the last point the
/// last step's pace.
fn table_at(p: &[(f32, f32)], d: f32) -> f32 {
    let i = p.iter().position(|q| d <= q.0).unwrap_or(p.len() - 1).max(1);
    let (a, b) = (p[i - 1], p[i]);
    a.1 + (d - a.0) * (b.1 - a.1) / (b.0 - a.0)
}

impl ZoneScaling {
    /// The normal enemy health multiplier at difficulty `d`: straight lines between the points, and
    /// past the last point the last step's pace goes on.
    pub fn normal_health_at(&self, d: f32) -> f32 {
        let p = &self.normal_health;
        let i = p.iter().position(|q| d <= q.0).unwrap_or(p.len() - 1).max(1);
        let (a, b) = (p[i - 1], p[i]);
        a.1 + (d - a.0) * (b.1 - a.1) / (b.0 - a.0)
    }

    /// The power of the normal health multiplier an enemy of kind `kind` takes for its health.
    pub fn health_power(&self, kind: EnemyType) -> f32 {
        match kind {
            EnemyType::Normal => 1.0,
            EnemyType::Elite | EnemyType::Rare => self.elite_health_power,
            EnemyType::Boss | EnemyType::FinalBoss => self.boss_health_power,
        }
    }

    /// The power of its own health multiplier an enemy of kind `kind` takes for its damage.
    pub fn damage_share_of(&self, kind: EnemyType) -> f32 {
        match kind {
            EnemyType::Normal => self.damage_share,
            _ => self.elite_boss_damage_share,
        }
    }

    /// What an enemy of kind `kind` has its health multiplied by in a zone of difficulty `d`.
    pub fn health_factor(&self, kind: EnemyType, d: f32) -> f32 {
        self.normal_health_at(d).max(0.0).powf(self.health_power(kind))
    }

    /// What an enemy of kind `kind` has its damage multiplied by in a zone of difficulty `d`.
    pub fn damage_factor(&self, kind: EnemyType, d: f32) -> f32 {
        if self.normal_damage.len() >= 2 {
            let power = match kind {
                EnemyType::Normal => 1.0,
                EnemyType::Elite | EnemyType::Rare => self.elite_damage_power,
                EnemyType::Boss | EnemyType::FinalBoss => self.boss_damage_power,
            };
            return table_at(&self.normal_damage, d).max(0.0).powf(power);
        }
        self.health_factor(kind, d).powf(self.damage_share_of(kind))
    }

    /// How much stronger than the first zone's a gear piece from a zone of difficulty `d` is.
    pub fn gear_factor(&self, d: f32) -> f32 {
        let d = d.max(0.0);
        if d <= self.gear_steeper_from {
            d.powf(self.gear_exponent)
        } else {
            self.gear_steeper_from.powf(self.gear_exponent) * self.gear_growth_after.powf(d - self.gear_steeper_from)
        }
    }

    /// Boss gear drop chance in a zone of difficulty `d`, before the run-option bonus.
    pub fn drop_chance_at(&self, d: f32) -> f32 {
        let step = self.gear_drop_chance.iter().rev().find(|s| s.0 <= d).or(self.gear_drop_chance.first());
        step.map_or(0.0, |s| s.1)
    }

    /// The talent experience multiplier of a zone of difficulty `d`.
    pub fn talent_factor(&self, d: f32) -> f32 {
        d.max(0.0).powf(self.talent_exponent)
    }
}

impl Default for ZoneScaling {
    /// Neutral numbers for content built in code: no growth with D.
    fn default() -> Self {
        Self {
            normal_health: vec![(1.0, 1.0), (2.0, 1.0)],
            elite_health_power: 1.0,
            boss_health_power: 1.0,
            damage_share: 0.6,
            elite_boss_damage_share: 0.6,
            gear_exponent: 0.0,
            gear_steeper_from: 10.0,
            gear_growth_after: 1.0,
            gear_drop_chance: vec![(1.0, 1.0)],
            talent_exponent: 0.0,
            gear_tier_starts: vec![2.0, 4.0, 7.0, 9.0],
            normal_damage: Vec::new(),
            elite_damage_power: 1.0,
            boss_damage_power: 1.0,
        }
    }
}

/// One kind's multipliers on the base health and damage.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TypeMultipliers {
    pub health: f32,
    pub damage: f32,
}

/// The kinds of enemy the stat formula tells apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyType {
    Normal,
    Elite,
    Rare,
    Boss,
    FinalBoss,
}

impl Balance {
    /// The multipliers of enemy kind `kind`.
    pub fn multipliers(&self, kind: EnemyType) -> TypeMultipliers {
        match kind {
            EnemyType::Normal => self.normal,
            EnemyType::Elite => self.elite,
            EnemyType::Rare => self.rare,
            EnemyType::Boss => self.boss,
            EnemyType::FinalBoss => self.final_boss,
        }
    }
}

impl Default for Balance {
    /// Neutral numbers for content built in code: every multiplier 1 and flat clocks.
    fn default() -> Self {
        let one = TypeMultipliers { health: 1.0, damage: 1.0 };
        Self {
            enemy_speed: 1.0,
            base_health: 1.0,
            base_damage: 1.0,
            zone_scaling: ZoneScaling::default(),
            normal: one,
            elite: one,
            rare: one,
            boss: one,
            final_boss: one,
            health_clock: vec![(0.0, 1.0), (1.0, 1.0)],
            boss_clock_power: 1.0,
            damage_clock: vec![(0.0, 1.0), (1.0, 1.0)],
            spawn_budget: vec![(0.0, 1.0), (1.0, 1.0)],
            live_cap: (60.0, 120.0),
            experience: ExperienceRules::default(),
            spell_tags: SpellTagCoefficients::default(),
            battle_standard: BattleStandardRules::default(),
            challenge_factor: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Spawn {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub facing: f32,
}

/// A line across the floor, from one end to the other (x, y).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Wall {
    pub from: (f32, f32),
    pub to: (f32, f32),
}

/// A door of the map's own (a game object), shut until `opens_at_minute` on the run clock; then
/// it swings open and `announce` is shown as a raid warning. A door
/// with no minute stays shut until a boss's script opens it (`ScriptAction::Door`: Stratholme's
/// Slaughterhouse); one with no announcement opens silently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Door {
    /// The game object entry and display it is shown as, and its name (ours).
    pub object: u32,
    pub name: String,
    pub display_id: u32,
    /// Where it stands and faces.
    pub at: Spawn,
    /// The doorway it shuts, a wall while it is shut.
    pub doorway: Wall,
    #[serde(default = "no_clock", skip_serializing_if = "no_clock_set")]
    pub opens_at_minute: f32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub announce: String,
}

/// A door no run clock opens (only a script does).
fn no_clock() -> f32 {
    f32::INFINITY
}

fn no_clock_set(minute: &f32) -> bool {
    minute.is_infinite()
}

/// A map object of our own, shown where it stands: a game object of `display_id`
/// (`GameObjectDisplayInfo.dbc`) at `scale`, named `name` (ours).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prop {
    pub name: String,
    pub display_id: u32,
    pub at: Spawn,
    pub scale: f32,
}

/// Floor of our own: a disc of `radius` yards round `centre` at height `z`, laid over a hole.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Platform {
    pub centre: (f32, f32),
    pub radius: f32,
    pub z: f32,
}

/// The base spawn distances, used by a zone that writes none.
pub const ENEMY_SPAWN_RING: (f32, f32) = (22.0, 34.0);
pub const BOSS_SPAWN_RING: (f32, f32) = (24.0, 28.0);

pub(crate) fn enemy_spawn_ring() -> (f32, f32) {
    ENEMY_SPAWN_RING
}

pub(crate) fn boss_spawn_ring() -> (f32, f32) {
    BOSS_SPAWN_RING
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Weather {
    pub kind: WeatherKind,
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherKind {
    None,
    Rain,
    Snow,
    Sandstorm,
}

/// `CreatureType.dbc` ids (the client's numbering, `dbc_id()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreatureType {
    Beast,
    Dragonkin,
    Demon,
    Elemental,
    Giant,
    Undead,
    Humanoid,
    Critter,
    Mechanical,
    NotSpecified,
}

impl CreatureType {
    pub fn dbc_id(self) -> u32 {
        self as u32 + 1
    }
}

/// One entry of a zone's roster, fully resolved: its zone file names its kind and `enemy_kinds.ron`
/// supplies the kind's numbers (see [`crate::EnemyEntry`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enemy {
    pub name: String,
    /// The creature entry the client is told about, numbered as the content loads (`entries`).
    #[serde(skip)]
    pub entry: u32,
    pub display_id: u32,
    pub scale: f32,
    pub creature_type: CreatureType,
    /// How much sturdier than its zone's average normal enemy this one is: a multiplier on its
    /// health only. Read for normal enemies only; defaults to 1.
    #[serde(default = "one")]
    pub toughness: f32,
    /// Seconds between melee swings.
    pub attack_interval: f32,
    /// Yards per second.
    pub move_speed: f32,
    /// Cost against the spawn director's budget (confirmed): cheap fodder 1, elites 12+.
    /// Also what a kill adds to the director's kill pressure.
    pub spawn_cost: f32,
    /// Run minute from which this enemy may spawn.
    pub appears_at_minute: f32,
    pub elite: bool,
    pub ranged: Option<RangedAttack>,
    /// It looks like a player character of the faction the heroes fight
    /// (Warsong Gulch): `display_id` is not read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub looks_like: Option<PlayerLook>,
    /// A PvP rank's title it carries before a made-up player name (`pvp_looks.ron`
    /// `player_names`), in place of `name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<RankTitles>,
    /// What it holds (a creature; a player-looking one wears its class's looks instead).
    #[serde(default, skip_serializing_if = "CreatureWeapons::is_empty")]
    pub weapons: CreatureWeapons,
    /// It never spawns by itself: only a boss's script or ability brings it (a wave, a hatchling).
    #[serde(default)]
    pub only_summoned: bool,
    /// Heroes and assists pick it as they pick an elite (its kind's numbers stay its own), and it
    /// glows as an elite: one to focus, as the Nightmare Suppressors that heal a boss and the Black
    /// Guard Sentries that guard one. Its `scale` is the elite size (zones.md, "Sizes").
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub focus_as_elite: bool,
}

/// What a player-looking enemy is: a player character of the class
/// `class` (an id in classes.ron) of the faction the heroes fight, of a race of that faction,
/// wearing that class's gear in `pvp_looks.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerLook {
    pub class: String,
    /// Shown in a druid's form instead of its race's model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<PlayerForm>,
}

/// What a boss in Moonkin Form casts from range: Starfire.
pub const MOONKIN_BOSS_SPELL: u32 = 2912;

/// A druid's form a player-looking enemy is shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerForm {
    Cat,
    Bear,
    Moonkin,
}

/// A PvP rank (1 to 14) and its title for each faction, as the client names them
/// (`{AllianceRank(n)}`, `{HordeRank(n)}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankTitles {
    pub rank: u32,
    pub alliance: String,
    pub horde: String,
}

impl RankTitles {
    /// The title as a player of `faction` carries it.
    pub fn of(&self, faction: Faction) -> &str {
        match faction {
            Faction::Alliance => &self.alliance,
            Faction::Horde => &self.horde,
        }
    }
}

/// The looks of the player-looking enemies (`pvp_looks.ron`): the gear
/// each class wears, every slot a few pieces to pick from, and the made-up player names a ranked
/// one carries.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PvpLooks {
    pub classes: Vec<ClassLooks>,
    /// A caster boss's spell hits for this share of the damage a boss's swing does: it cannot be
    /// stepped out of.
    #[serde(default = "half_share")]
    pub caster_boss_damage: f32,
    /// What a normal enemy's name starts with where it writes [`JOKE`]: one picked for each.
    #[serde(default)]
    pub jokes: Vec<String>,
    pub player_names: Vec<String>,
}

/// A hand-made player character (`player_characters.ron`): who it is,
/// how it looks and how it assists. Every one is a Warsong boss of its faction (`played_by`) and
/// an assist a battle standard summons for heroes of its faction; this list is the only place a
/// character lives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerCharacter {
    pub id: String,
    pub name: String,
    /// A race and a class by their ids in races.ron and classes.ron; its faction is its race's.
    pub race: String,
    pub class: String,
    pub gender: Gender,
    /// Its face and hair; left out, one is picked within its race's dials each time it shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub looks: Option<CharacterLooks>,
    /// Shown in a druid's form (its faction's look of it) in place of its race's model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<PlayerForm>,
    pub gear: CharacterGear,
    /// What it yells as it arrives, as a boss or when it assists (empty: a boss keeps its own).
    #[serde(default)]
    pub yell: String,
    /// The spell (`Spell.dbc` id) it casts at the hero from range as a boss, in place of its
    /// class's (Nerf's Fireball).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boss_spell: Option<u32>,
    /// The boss abilities it fights with, in place of its class's (`pvp_looks.ron`
    /// `boss_abilities`): its spec's (a tank's, an enhancement
    /// shaman's).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boss_abilities: Vec<crate::BossAbilityEntry>,
    /// What it yells as it dies, as a boss or ending its assist (empty: a boss's
    /// own death yell).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub death_yell: String,
    /// How it assists a hero, described as a player reads it (`assist_script` is what it does).
    /// The client's names in it are pointers (`{Spell(1856)}`), shown as the content loads.
    #[serde(default)]
    pub assist: String,
    /// What it does when a battle standard summons it beside a hero of its faction
    /// (docs/game/assists.md); missing: its class's (and form's) default in `class_assists.ron`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assist_script: Option<crate::AssistScript>,
    /// Its rotation and movement once its assist is done, until it leaves (docs/game/assists.md);
    /// missing: its class's (and form's) default in `class_assists.ron`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behaviour: Option<crate::AssistBehaviour>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gender {
    Male,
    Female,
}

impl Gender {
    /// The client's gender byte.
    pub fn id(self) -> u8 {
        match self {
            Gender::Male => 0,
            Gender::Female => 1,
        }
    }
}

/// A character's face and hair, as the create screen's dials count them (0 the first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CharacterLooks {
    pub skin: u8,
    pub face: u8,
    pub hair_style: u8,
    pub hair_color: u8,
    pub facial_hair: u8,
}

/// What a character wears: its class's PvP looks picked for it (`Random`, as Warsong's players),
/// or the pieces written for it (`Picked`), its class's PvP looks filling any slot left out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CharacterGear {
    Random,
    Picked(PickedGear),
}

/// Hand-picked pieces, by look (`ItemDisplayInfo.dbc`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PickedGear {
    #[serde(default)]
    pub armor: Vec<PickedArmor>,
    #[serde(default)]
    pub main_hand: Option<WeaponLook>,
    #[serde(default)]
    pub off_hand: Option<WeaponLook>,
    #[serde(default)]
    pub ranged: Option<WeaponLook>,
    /// Armor slots shown empty: no piece, not even the class's PvP look under the picks
    /// (e.g. a character without a helm).
    #[serde(default)]
    pub bare: Vec<ArmorSlot>,
}

/// One hand-picked armor piece; `robe` for a chest drawn as a robe.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PickedArmor {
    pub slot: ArmorSlot,
    pub display: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub robe: bool,
}

/// A spell (`Spell.dbc` id) for each faction the heroes may be: `vs_alliance` what Alliance
/// heroes meet, `vs_horde` what Horde heroes meet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactionSpell {
    pub vs_alliance: u32,
    pub vs_horde: u32,
}

impl FactionSpell {
    /// The spell heroes of `faction` meet.
    pub fn against(self, faction: Faction) -> u32 {
        match faction {
            Faction::Alliance => self.vs_alliance,
            Faction::Horde => self.vs_horde,
        }
    }
}

/// Which hand-made character plays a boss: any of the faction the heroes fight (a different
/// one for each boss of a run while there are enough), or the one named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CharacterPick {
    Any,
    Named(String),
}

fn half_share() -> f32 {
    0.5
}

/// A spread of `id` for pick number `salt`: the picks that follow from a number (an enemy's id,
/// a run's seed), each drawing from its own.
pub fn spread(id: u32, salt: u32) -> u32 {
    let mut x = id.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B).rotate_left(13);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^ (x >> 15)
}

/// Where a player-looking enemy's name takes one of `pvp_looks.ron`'s `jokes` (`"{Joke} {Class(1)}"`).
pub const JOKE: &str = "{Joke}";

impl PvpLooks {
    /// The gear of class `class` (an id in classes.ron).
    pub fn of(&self, class: &str) -> Option<&ClassLooks> {
        self.classes.iter().find(|c| c.class == class)
    }
}

/// One class's gear: per armor slot the pieces' `ItemDisplayInfo.dbc` looks to pick from, the
/// weapon sets to pick from, and the rank 12 rare PvP set its ranked ones wear over the rest, each
/// faction's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassLooks {
    pub class: String,
    /// A boss of this class is a caster: it stands and casts this spell
    /// (`Spell.dbc` id) at the hero from range instead of swinging, for `caster_boss_damage`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boss_spell: Option<u32>,
    /// A boss of this class fights with these: its class's own looks,
    /// none that stuns, fears, roots or slows; the numbers are `enemy_kinds.ron`'s. A boss takes
    /// as many as its `class_abilities`, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boss_abilities: Vec<crate::BossAbilityEntry>,
    /// In place of `boss_abilities` while shown in a druid's form.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub form_boss_abilities: Vec<(PlayerForm, Vec<crate::BossAbilityEntry>)>,
    pub armor: Vec<ArmorLooks>,
    pub weapons: Vec<WeaponSet>,
    #[serde(default)]
    pub ranked_alliance: Vec<ArmorLooks>,
    #[serde(default)]
    pub ranked_horde: Vec<ArmorLooks>,
}

impl ClassLooks {
    /// The rank set a ranked one of `faction` wears, the other faction's when its own has none.
    pub fn ranked(&self, faction: Faction) -> &[ArmorLooks] {
        let (own, other) = match faction {
            Faction::Alliance => (&self.ranked_alliance, &self.ranked_horde),
            Faction::Horde => (&self.ranked_horde, &self.ranked_alliance),
        };
        if own.is_empty() {
            other
        } else {
            own
        }
    }
}

/// The pieces one armor slot picks from; `robes` are the chest pieces among them drawn as robes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArmorLooks {
    pub slot: ArmorSlot,
    pub displays: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub robes: Vec<u32>,
}

/// The weapons one carries together: a main hand, an off hand (none beside a two-hander) and a
/// ranged weapon.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeaponSet {
    #[serde(default)]
    pub main_hand: Option<WeaponLook>,
    #[serde(default)]
    pub off_hand: Option<WeaponLook>,
    #[serde(default)]
    pub ranged: Option<WeaponLook>,
}

/// A weapon's kind and look.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeaponLook {
    pub kind: WeaponKind,
    pub display: u32,
}

/// What a creature enemy or boss holds, as the WoW creature of its name does (the
/// vmangos database's `creature_equip_template` items, see `docs/game/enemy-weapons.md`): sent as
/// the creature's virtual items, so the client shows them and swings by them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct CreatureWeapons {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_hand: Option<CreatureWeapon>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub off_hand: Option<CreatureWeapon>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ranged: Option<CreatureWeapon>,
}

impl CreatureWeapons {
    pub fn is_empty(&self) -> bool {
        self.main_hand.is_none() && self.off_hand.is_none() && self.ranged.is_none()
    }

    /// The three virtual item slots in the client's order: main hand, off hand, ranged.
    pub fn slots(&self) -> [Option<CreatureWeapon>; 3] {
        [self.main_hand, self.off_hand, self.ranged]
    }

    /// It holds a weapon in each hand, and so swings with its off hand too.
    pub fn dual_wields(&self) -> bool {
        self.main_hand.is_some() && self.off_hand.is_some_and(|w| w.kind.item_class() == 2)
    }
}

/// One weapon (or shield) a creature holds: its kind (the item class and subclass the client
/// swings by), its look (`ItemDisplayInfo.dbc`), and `item_template`'s own inventory type, sheath
/// and material for the item, which a monster item does not always share with the kind's player
/// rule (a polearm sheathed as a staff, Mograine's two-handed mace marked one-hand).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CreatureWeapon {
    pub kind: WeaponKind,
    pub display: u32,
    pub inventory_type: u32,
    pub sheath: u32,
    pub material: u32,
}

impl CreatureWeapon {
    /// `UNIT_VIRTUAL_ITEM_INFO`'s first word: class, subclass, material and inventory type, a
    /// byte each.
    pub fn info(&self) -> u32 {
        self.kind.item_class() | self.kind.subclass() << 8 | (self.material & 0xff) << 16 | (self.inventory_type & 0xff) << 24
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RangedAttack {
    /// The WoW spell the enemy casts (`Spell.dbc` id): its look, and its `Speed` sets how fast
    /// the projectile flies.
    pub look_spell: u32,
    /// The enemy stops to cast once the hero is within `range + 4` yards.
    pub range: f32,
    /// Seconds the enemy stands casting before the projectile leaves. It attacks every
    /// `attack_interval` seconds.
    pub cast_time: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Boss {
    pub name: String,
    /// The creature entry the client is told about, numbered as the content loads (`entries`).
    #[serde(skip)]
    pub entry: u32,
    pub display_id: u32,
    pub scale: f32,
    pub creature_type: CreatureType,
    /// Run minute the boss arrives.
    pub spawn_minute: f32,
    pub move_speed: f32,
    /// Said (yelled) on arrival.
    pub yell: String,
    /// The boss's special attacks, each on its own cooldown.
    pub abilities: Vec<BossAbility>,
    /// The boss whose death wins the run. Several final bosses come at the same minute, and the
    /// run is won when the last of them falls.
    pub is_final: bool,
    /// A spot of its own instead of the boss ring round a hero
    /// (Whitemane and Mograine behind the cathedral's altar).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Spawn>,
    /// Its share of the health its kind's formula gives (two final bosses at 0.75 each).
    #[serde(default = "full_share")]
    pub health_share: f32,
    /// An aura it carries, shown on it from its arrival, by the faction it is fought by
    /// (Warsong's bosses carry the heroes' own flag).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carries: Option<FactionSpell>,
    /// A hand-made player character plays it: its race, look, gear
    /// and name (after its rank's title) in place of `looks_like`'s, which stands in while there
    /// is none to pick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub played_by: Option<CharacterPick>,
    /// It looks like a player character of the faction the heroes fight
    /// (Warsong Gulch): `display_id` is not read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub looks_like: Option<PlayerLook>,
    /// A PvP rank's title it carries before a made-up player name (`pvp_looks.ron`
    /// `player_names`), in place of `name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<RankTitles>,
    /// How many of its class's boss abilities it uses before `abilities`
    /// (a player-looking boss fights as its class, see [`crate::Content::boss_abilities`]).
    #[serde(default)]
    pub class_abilities: u32,
    /// The original boss's own pull voice line, a `SoundEntries.dbc` id, played as it arrives
    /// with its yell (whose text is the line it speaks); none for a boss the client gives no voice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggro_sound: Option<u32>,
    /// What it holds (a creature; a player-looking one wears its class's looks instead).
    #[serde(default, skip_serializing_if = "CreatureWeapons::is_empty")]
    pub weapons: CreatureWeapons,
    /// Its script: what it does at a health share, on a timer, as it
    /// arrives or dies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub script: Vec<crate::ScriptLine>,
    /// A stack its melee blows leave on the hero (Lupos's shadow bite).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_hit: Option<crate::Affliction>,
    /// Its melee swing hits this many times as hard (a reworked boss
    /// keeps the damage a hero cannot avoid; docs/game/bosses.md, "Damage a hero cannot avoid").
    #[serde(default = "full_share")]
    pub swing_mult: f32,
    /// Only a script brings it (`crate::ScriptAction::Call`); the schedule passes it by.
    #[serde(default)]
    pub called: bool,
}

fn full_share() -> f32 {
    1.0
}

/// One special attack of a boss (or rare), cast whenever its cooldown runs out, fully
/// resolved: its zone file names the kind and its looks, `enemy_kinds.ron` supplies the numbers (see
/// [`crate::BossAbilityEntry`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BossAbility {
    pub kind: BossAbilityKind,
    /// Seconds between casts (×0.7 once the boss is at or below 30 % health).
    pub cooldown: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BossAbilityKind {
    /// Marks the hero's spot (`telegraph_spell`), and `delay` seconds later bursts there
    /// (`impact_spell`): `damage_mult` × the boss's attack damage if the hero is within `radius`.
    GroundBurst { telegraph_spell: u32, impact_spell: u32, radius: f32, delay: f32, damage_mult: f32 },
    /// `count` missiles of WoW spell `look_spell`: one at the hero, the rest 2–6 yd around
    /// the hero; each hits within 2.5 yd for `damage_mult` × attack damage when it lands.
    Volley {
        look_spell: u32,
        count: u32,
        damage_mult: f32,
        /// Each shot plays on an invisible target unit where it lands (`Marked`'s `impact_on_bunny`).
        #[serde(default)]
        impact_on_bunny: bool,
    },
    /// The boss shouts for help and `count` adds appear on a 5-yard ring around the boss.
    Summon { count: u32 },
    /// Plays `look_spell`; `damage_mult` × attack damage if the hero is within `radius`.
    Stomp { look_spell: u32, radius: f32, damage_mult: f32 },
    /// Plays WoW spell `look_spell` (100, Charge) and rushes to the hero's spot; landing
    /// hits within 4.5 yd for `damage_mult` × attack damage.
    Charge { look_spell: u32, damage_mult: f32 },
    /// A marked attack (every boss area attack is marked on the ground
    /// before it lands): `shape` laid `at`, shown with `warning_spell` filling with `fill_spell`
    /// over `wind_up` seconds, then `impact_spell` plays and a hero still inside takes
    /// `damage_mult` × attack damage. It may leave a lasting zone or an object where it landed;
    /// with `missile`, `impact_spell` flies from the boss to land as the wind-up ends. A boss
    /// whose mark is laid round itself stands still while it winds up.
    Marked {
        shape: crate::Shape,
        at: crate::MarkAt,
        wind_up: f32,
        damage_mult: f32,
        impact_spell: u32,
        warning_spell: u32,
        fill_spell: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        leaves: Option<crate::LastingZone>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        leaves_object: Option<crate::GroundObject>,
        #[serde(default)]
        missile: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        warn: Option<crate::Warning>,
        /// Laid from each living roster enemy of this name instead of the boss (round it, or
        /// from it towards the hero; a missile flies from it); none up, nothing is cast.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
        /// Its impact plays on an invisible target unit (a "bunny") put where it lands, for a
        /// look that shows only on a unit it hits (a smite, a strike): seen whether or not the
        /// hero is there. The damage stays the hero's alone.
        #[serde(default)]
        impact_on_bunny: bool,
        /// Each further mark of a cast lands this many seconds after the one before (a chain).
        #[serde(default)]
        chain: f32,
        /// Adds bursting from where it lands.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        brings: Option<crate::Brings>,
        /// A line that stays and sweeps round its boss once it lands (a beam).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sweep: Option<crate::Sweep>,
    },
    /// A marked rush: a `width`-wide line from the boss through the
    /// hero's spot to 8 yd past it (at most `length`) is marked for `wind_up` seconds (the boss
    /// standing, wearing `fade_spell` if set), then the boss runs all of it (`look_spell`), hitting a
    /// hero on it as it passes, once, for `damage_mult` × attack damage.
    Rush {
        look_spell: u32,
        length: f32,
        width: f32,
        wind_up: f32,
        damage_mult: f32,
        warning_spell: u32,
        fill_spell: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fade_spell: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        warn: Option<crate::Warning>,
    },
    /// `count` objects put down `near` (egg sacs): a hero walks over them to break them.
    Lay {
        object: crate::GroundObject,
        count: u32,
        near: crate::ObjectNear,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        warn: Option<crate::Warning>,
    },
    /// A lasting zone put down `at` (a drifting cloud), hurting heroes only.
    Zone {
        zone: crate::LastingZone,
        at: crate::MarkAt,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        warn: Option<crate::Warning>,
    },
}

impl BossAbilityKind {
    /// The roster enemies the ability names (its objects' hatchlings), to check they exist.
    pub fn enemies_named(&self) -> Vec<&str> {
        match self {
            BossAbilityKind::Marked { leaves_object, from, brings, .. } => {
                leaves_object.iter().flat_map(|o| o.enemies_named()).chain(from.as_deref()).chain(brings.iter().map(|b| b.enemy.as_str())).collect()
            }
            BossAbilityKind::Lay { object: o, .. } => o.enemies_named(),
            _ => Vec::new(),
        }
    }

    /// The warning it announces when it starts (raid warnings).
    pub fn warning(&self) -> Option<&crate::Warning> {
        match self {
            BossAbilityKind::Marked { warn, .. } | BossAbilityKind::Rush { warn, .. } | BossAbilityKind::Lay { warn, .. } | BossAbilityKind::Zone { warn, .. } => warn.as_ref(),
            _ => None,
        }
    }
}

// ---- classes and abilities -------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerType {
    Mana,
    Rage,
    Energy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Class {
    pub id: String,
    /// The name as the content file writes it (`name:`): our text, or a pointer to the client's.
    #[serde(rename = "name")]
    pub name_ref: NameRef,
    /// The name as the game shows it, filled from `name_ref` as the content loads.
    #[serde(skip)]
    pub name: String,
    /// `ChrClasses.dbc` id.
    pub wow_class_id: u32,
    pub power: PowerType,
    /// Class colour as `#RRGGBB`.
    pub color: String,
    pub icon: String,
    pub description: String,
    pub base_health: f32,
    /// Damage reduction before bonuses (plate 0.2, mail and leather 0.15, cloth 0.08).
    pub base_damage_reduction: f32,
    /// Lifesteal before bonuses: a share of every hit's damage heals the hero (rogue 0.03).
    pub base_lifesteal: f32,
    /// Melee auto-attack, for the classes that have one.
    pub auto_attack: Option<AutoAttack>,
    /// The class's spell pool for level-up offers.
    pub abilities: Vec<String>,
    /// The starting options a hero picks from before a run (3; the Druid 4), the default first.
    /// See `StartingOption`.
    pub starting_options: Vec<crate::starts::StartingOption>,
    /// Multiplies the damage of this class's spells and the amounts that go with it (heals,
    /// shields, aura damage); never cooldowns or durations, never the race spell: a
    /// per-class tuning knob, 1 for every class for now.
    #[serde(default = "one")]
    pub spell_damage_scale: f32,
}

impl Class {
    /// The starting option `id`, or the first (the default) when it is not one of the class's.
    pub fn starting_option(&self, id: Option<&str>) -> Option<&crate::starts::StartingOption> {
        id.and_then(|id| self.starting_options.iter().find(|o| o.id == id)).or(self.starting_options.first())
    }

    /// The default starting option's main spell.
    pub fn default_start_spell(&self) -> &str {
        self.starting_options.first().map_or("", |o| o.main_spell())
    }

    /// The cards a level-up offers a hero who started with option `id` (its own pool, else the
    /// class's).
    pub fn card_pool(&self, id: Option<&str>) -> &[String] {
        self.starting_option(id).and_then(|o| o.card_pool.as_deref()).unwrap_or(&self.abilities)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AutoAttack {
    /// A hit's damage: a one-hand swing's (share 1) where the class's weapon loadouts set the
    /// swing (`loadouts.ron`).
    pub damage: f32,
    /// Seconds between swings; missing (0) where the loadouts set it.
    #[serde(default)]
    pub swing: f32,
    pub range: f32,
    /// A ranged swing shows this spell's shot (the Hunter's Auto Shot); missing: a melee swing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_spell: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum School {
    Physical,
    Holy,
    Fire,
    Nature,
    Frost,
    Shadow,
    Arcane,
}

/// What an ability does to what it hits, on top of its damage.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SpellEffect {
    None,
    Slow { amount: f32, seconds: f32 },
    Root { seconds: f32 },
    Stun { seconds: f32 },
    Fear { seconds: f32 },
    /// Heals from damage dealt.
    HealFromDamage { share: f32 },
    /// Heals a share of max health.
    Heal { share: f32 },
    /// Absorb shield, a share of max health.
    Shield { share: f32, seconds: f32 },
    DamageBuff { amount: f32, seconds: f32 },
    HasteBuff { amount: f32, seconds: f32 },
    /// A debuff on the target.
    Weaken { amount: f32, seconds: f32 },
    Immunity { seconds: f32 },
    /// A lasting aura on the hero that strikes back at attackers.
    Thorns { amount: f32 },
    /// On a hit, the hero gains `charges` healing charges for the
    /// spell's `duration`; each later weapon swing or spell cast that hits uses one and heals
    /// `share` of max health (`bloodthirst`).
    HealingCharges { charges: u32, share: f32 },
    /// The enemy takes `amount` more damage from everything for `seconds` (Hemorrhage).
    Vulnerable { amount: f32, seconds: f32 },
    /// The hero moves `amount` faster for `seconds` (Dash).
    SpeedBuff { amount: f32, seconds: f32 },
}

/// A way a level-up card can improve a spell (classes.md, "Upgrade cards").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Upgrade {
    Power,
    Cooldown,
    Targets,
    Area,
    Duration,
    /// More rage gained (in place of Cooldown on a rage spell; half a Cooldown card's strength).
    RageGain,
    /// Faster energy refill (in place of Cooldown on an energy spell; half strength).
    EnergyGain,
    /// A higher proc chance (in place of Cooldown on a proc; half strength); above 100 % it works
    /// as double cast: the part over 100 % is the chance of a second proc.
    Chance,
    /// +1 stack (Holy Shield; in place of Targets).
    Stacks,
    /// +1 extra hit per proc (Windfury; in place of Targets).
    Strikes,
    /// +1 imp per cast (Hand of Gul'dan; in place of Targets).
    Imps,
    /// +1 pet, up to 3 (Kill Command; in place of Targets).
    Pets,
    /// +1 critter per cast (Friend of the Forest; in place of Targets).
    Critters,
}

impl Upgrade {
    /// Its word on the wire and in offers (`power`, `rage_gain` …).
    pub fn word(self) -> &'static str {
        match self {
            Upgrade::Power => "power",
            Upgrade::Cooldown => "cooldown",
            Upgrade::Targets => "targets",
            Upgrade::Area => "area",
            Upgrade::Duration => "duration",
            Upgrade::RageGain => "rage_gain",
            Upgrade::EnergyGain => "energy_gain",
            Upgrade::Chance => "chance",
            Upgrade::Stacks => "stacks",
            Upgrade::Strikes => "strikes",
            Upgrade::Imps => "imps",
            Upgrade::Pets => "pets",
            Upgrade::Critters => "critters",
        }
    }

    /// The kind whose word is `word`.
    pub fn named(word: &str) -> Option<Upgrade> {
        Upgrade::ALL.into_iter().find(|u| u.word() == word)
    }

    /// Every upgrade kind, in declaration order (its index is `kind as usize`).
    pub const ALL: [Upgrade; 13] = [
        Upgrade::Power,
        Upgrade::Cooldown,
        Upgrade::Targets,
        Upgrade::Area,
        Upgrade::Duration,
        Upgrade::RageGain,
        Upgrade::EnergyGain,
        Upgrade::Chance,
        Upgrade::Stacks,
        Upgrade::Strikes,
        Upgrade::Imps,
        Upgrade::Pets,
        Upgrade::Critters,
    ];

    /// A kind that steps like Cooldown at half its strength, as a gain (Rage %, Energy %, Chance).
    pub fn is_gain(self) -> bool {
        matches!(self, Upgrade::RageGain | Upgrade::EnergyGain | Upgrade::Chance)
    }

    /// A kind that adds whole units like Targets (Stacks, Strikes, Imps, Pets, Critters).
    pub fn is_count(self) -> bool {
        matches!(self, Upgrade::Targets | Upgrade::Stacks | Upgrade::Strikes | Upgrade::Imps | Upgrade::Pets | Upgrade::Critters)
    }
}

/// What kind of spell an ability is. Our own grouping: it lets
/// `spell_categories.ron` change every spell of a kind at once, and talents filter on it. A
/// spell can be in several.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpellCategory {
    /// Deals its damage in single hits.
    DirectDamage,
    /// Deals its damage in ticks over time (effects over time, ground areas, attacking totems).
    DamageOverTime,
    /// Hits everything in an area (novas, ground areas, cones, totems with a radius).
    AreaOfEffect,
    /// Strikes in melee reach.
    Melee,
    /// Strikes from range (missiles and chains).
    Ranged,
    /// Slows, roots, stuns or fears.
    CrowdControl,
    /// Heals the hero.
    Sustain,
    /// Puts an absorb shield on the hero.
    Shield,
    /// Lowers the damage the hero takes (shields, immunity, weakening enemies).
    DamageReduction,
    /// Strikes back at attackers (thorns-like auras).
    Reflect,
    /// Raises the hero's damage or speed for a while.
    Buff,
    /// Places a creature (totems).
    Summon,
}

fn one() -> f32 {
    1.0
}

/// What a spell is about, for tuning. A spell that deals damage has
/// exactly one damage tag (`Level1` … `Pet`), set by its shape; `Sustain`, `Buffs` and
/// `Defensive` mark what else it gives and may sit on top of a damage tag. Each tag has a
/// coefficient in `balance.ron` (`SpellTagCoefficients`) that scales only the part of the spell
/// the tag is about, never cooldowns or durations. Separate from `categories`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpellTag {
    /// A class's starting spell.
    Level1,
    /// Strikes one target.
    Nuke,
    /// Strikes two to five targets.
    Cleave,
    /// Deals its damage over time on its targets.
    Dot,
    /// Strikes everything around the hero, or in a cone in front of the hero.
    Nova,
    /// Leaves a lasting area on the ground.
    Area,
    /// A creature fights for the hero (totems).
    Pet,
    /// Heals the hero: heals, heals from damage, healing charges, heals on hit.
    Sustain,
    /// Raises the hero's damage or speed for a while.
    Buffs,
    /// Shields, immunity, reflected damage, and weakening enemies.
    Defensive,
}

impl SpellTag {
    /// The tags that say how a spell deals its damage; a damaging spell has exactly one.
    pub const DAMAGE: [SpellTag; 7] =
        [SpellTag::Level1, SpellTag::Nuke, SpellTag::Cleave, SpellTag::Dot, SpellTag::Nova, SpellTag::Area, SpellTag::Pet];

    /// Whether this tag scales a spell's damage.
    pub fn is_damage(self) -> bool {
        Self::DAMAGE.contains(&self)
    }
}

/// The spell tag coefficients (`balance.ron` `spell_tags`). Damage tags
/// scale the spell's damage, `sustain` its healing, `buffs` its buff amount, `defensive` its
/// shield, weakening and reflected damage. A missing number is 1 (unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpellTagCoefficients {
    #[serde(default = "one")]
    pub level_1: f32,
    #[serde(default = "one")]
    pub nuke: f32,
    #[serde(default = "one")]
    pub cleave: f32,
    #[serde(default = "one")]
    pub dot: f32,
    #[serde(default = "one")]
    pub nova: f32,
    #[serde(default = "one")]
    pub area: f32,
    #[serde(default = "one")]
    pub pet: f32,
    #[serde(default = "one")]
    pub sustain: f32,
    #[serde(default = "one")]
    pub buffs: f32,
    #[serde(default = "one")]
    pub defensive: f32,
}

impl Default for SpellTagCoefficients {
    fn default() -> Self {
        Self { level_1: 1.0, nuke: 1.0, cleave: 1.0, dot: 1.0, nova: 1.0, area: 1.0, pet: 1.0, sustain: 1.0, buffs: 1.0, defensive: 1.0 }
    }
}

impl SpellTagCoefficients {
    /// The coefficient of `tag`.
    pub fn of(&self, tag: SpellTag) -> f32 {
        match tag {
            SpellTag::Level1 => self.level_1,
            SpellTag::Nuke => self.nuke,
            SpellTag::Cleave => self.cleave,
            SpellTag::Dot => self.dot,
            SpellTag::Nova => self.nova,
            SpellTag::Area => self.area,
            SpellTag::Pet => self.pet,
            SpellTag::Sustain => self.sustain,
            SpellTag::Buffs => self.buffs,
            SpellTag::Defensive => self.defensive,
        }
    }

    /// The product of the coefficients of those of `tags` that pass `which`.
    pub fn product(&self, tags: &[SpellTag], which: impl Fn(SpellTag) -> bool) -> f32 {
        tags.iter().filter(|t| which(**t)).map(|t| self.of(*t)).product()
    }
}

/// One line of `spell_categories.ron`: multipliers for every spell in `category`. A missing
/// number is 1.0 (unchanged). A spell in several categories gets every matching line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryTuning {
    pub category: SpellCategory,
    /// Damage per hit, or in total for effects over time.
    #[serde(default = "one")]
    pub damage: f32,
    /// Seconds between casts.
    #[serde(default = "one")]
    pub cooldown: f32,
    #[serde(default = "one")]
    pub range: f32,
    /// Area size (a cone's length).
    #[serde(default = "one")]
    pub radius: f32,
    /// How long the spell itself lasts: areas, effects over time, totems, and effects on the
    /// hero. Ticks keep their size, so a longer effect over time also deals more in total.
    #[serde(default = "one")]
    pub duration: f32,
    /// Strength of the on-hit effect: slow amount, heal or shield share, buff, weaken or aura amount.
    #[serde(default = "one")]
    pub effect: f32,
    /// How long the on-hit effect holds an enemy (slow, root, stun, fear, weaken).
    #[serde(default = "one")]
    pub effect_seconds: f32,
}

impl CategoryTuning {
    pub fn multipliers(&self) -> [f32; 7] {
        [self.damage, self.cooldown, self.range, self.radius, self.duration, self.effect, self.effect_seconds]
    }
}

impl SpellEffect {
    /// The effect with its strength × `strength` and its length × `seconds`. Slows and weakening
    /// stop at 100 %.
    pub fn scaled(&self, strength: f32, seconds: f32) -> SpellEffect {
        let (s, t) = (strength, seconds);
        match *self {
            SpellEffect::None => SpellEffect::None,
            SpellEffect::Slow { amount, seconds } => SpellEffect::Slow { amount: (amount * s).min(1.0), seconds: seconds * t },
            SpellEffect::Root { seconds } => SpellEffect::Root { seconds: seconds * t },
            SpellEffect::Stun { seconds } => SpellEffect::Stun { seconds: seconds * t },
            SpellEffect::Fear { seconds } => SpellEffect::Fear { seconds: seconds * t },
            SpellEffect::HealFromDamage { share } => SpellEffect::HealFromDamage { share: share * s },
            SpellEffect::Heal { share } => SpellEffect::Heal { share: share * s },
            SpellEffect::Shield { share, seconds } => SpellEffect::Shield { share: share * s, seconds: seconds * t },
            SpellEffect::DamageBuff { amount, seconds } => SpellEffect::DamageBuff { amount: amount * s, seconds: seconds * t },
            SpellEffect::HasteBuff { amount, seconds } => SpellEffect::HasteBuff { amount: amount * s, seconds: seconds * t },
            SpellEffect::Weaken { amount, seconds } => SpellEffect::Weaken { amount: (amount * s).min(1.0), seconds: seconds * t },
            SpellEffect::Immunity { seconds } => SpellEffect::Immunity { seconds: seconds * t },
            SpellEffect::Thorns { amount } => SpellEffect::Thorns { amount: amount * s },
            SpellEffect::HealingCharges { charges, share } => SpellEffect::HealingCharges { charges, share: share * s },
            SpellEffect::Vulnerable { amount, seconds } => SpellEffect::Vulnerable { amount: amount * s, seconds: seconds * t },
            SpellEffect::SpeedBuff { amount, seconds } => SpellEffect::SpeedBuff { amount: amount * s, seconds: seconds * t },
        }
    }
}

/// How an ability reaches its targets (spells.md, "Delivery").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Delivery {
    /// A projectile at each of the nearest enemies.
    Projectile,
    /// Instant hits on random targets, one after another.
    Chain,
    /// Everything around the hero.
    Nova,
    /// A lasting area placed on the hero or the densest group.
    GroundArea,
    /// A chain that bounces from enemy to enemy (`chain_lightning`).
    Bounce,
    /// The nearest enemies in reach.
    Melee,
    /// A damage-over-time effect.
    DamageOverTime,
    /// Melee finisher, double damage below 30 % health.
    Finisher,
    /// A cone in front of the hero.
    Cone,
    /// Heal, shield, buff or immunity on the hero.
    SelfEffect,
    /// A totem.
    Totem,
}

impl Delivery {
    /// Every delivery kind.
    pub const ALL: [Delivery; 11] = [
        Delivery::Projectile,
        Delivery::Chain,
        Delivery::Nova,
        Delivery::GroundArea,
        Delivery::Bounce,
        Delivery::Melee,
        Delivery::DamageOverTime,
        Delivery::Finisher,
        Delivery::Cone,
        Delivery::SelfEffect,
        Delivery::Totem,
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ability {
    pub id: String,
    /// The name as the content file writes it (`name:`): our text, or a pointer to the client's.
    #[serde(rename = "name")]
    pub name_ref: NameRef,
    /// The name as the game shows it, filled from `name_ref` as the content loads.
    #[serde(skip)]
    pub name: String,
    /// What the spell does, in our own words, as a card shows it the first time it is offered:
    /// a template whose `{name}` placeholders the card fills with the spell's numbers at the rank
    /// it offers (`text::SPELL_NUMBERS`). Our own data (design step 9).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub icon: String,
    /// `Spell.dbc` id used for the visual.
    pub look_spell: u32,
    pub school: School,
    /// How it reaches its targets: `Projectile`, `Nova`, `GroundArea`, `Melee`, `SelfEffect` ...
    pub delivery: Delivery,
    /// What kind of spell it is (see `SpellCategory`).
    #[serde(default)]
    pub categories: Vec<SpellCategory>,
    /// What the spell is about, for the `balance.ron` spell tag coefficients (see `SpellTag`).
    #[serde(default)]
    pub tags: Vec<SpellTag>,
    /// Which upgrades level-up cards may give it.
    pub upgrades: Vec<Upgrade>,
    /// When a milestone casts this spell for free, its hits may still set off milestones (by
    /// default they do not).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub free_casts_can_proc: bool,
    pub damage: f32,
    pub cooldown: f32,
    pub range: f32,
    pub radius: f32,
    pub targets: u32,
    /// For effects over time: how long, and in how many ticks. A `Nova` with 2 or
    /// more `ticks` goes off that many times per cast, `duration ÷ ticks` seconds apart, the
    /// first at once, each around the hero where it then stands, each for the full `damage`.
    pub duration: f32,
    pub ticks: u32,
    /// A totem's pulse: the `Spell.dbc` id each pulse shows and hits with (a `Totem`); for any
    /// other spell, the id its hits are logged under (their numbers' colour and the log's name)
    /// when its look's would read wrong (Fan of Knives: its look is a ranged basic shot, which
    /// the client shows white). Missing: its look spell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pulse_spell: Option<u32>,
    pub effect: SpellEffect,
    /// A heal on the hero waits until health is at or below this share of max health; missing:
    /// the usual 85 %.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heal_below: Option<f32>,
    /// Its hits deal double to enemies under 30 % health, whatever its delivery (finishers always
    /// do).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub double_on_low_health: bool,
    /// Each enemy its hit lands on also burns for a while (see `Burn`); missing: no burn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burn: Option<Burn>,
    /// A `GroundArea` also strikes every enemy inside it for this much at once as it
    /// lands, before its ticks; it scales as the area's hits do. Missing: 0, no impact.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub impact: f32,
    /// What it does beyond its delivery: a cost, combo points, its target order, its own rule
    /// (`SpellKit`). Missing: a plain spell.
    #[serde(default)]
    pub kit: crate::starts::SpellKit,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

/// An effect over time a spell's hit leaves on each enemy it lands on, starting as the hit lands:
/// `damage` in all, in `ticks` even ticks over `duration` seconds. It scales as the hit does
/// (hero damage, power, the class knob, the spell's damage tag), and a longer duration adds whole
/// ticks of the same size at the same rate. It stacks and renews as the spell's effects over time
/// do, and counts as the spell's damage.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Burn {
    pub damage: f32,
    pub duration: f32,
    pub ticks: u32,
}

// ---- races -----------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Faction {
    Alliance,
    Horde,
}

impl Faction {
    /// The other faction.
    pub fn other(self) -> Faction {
        match self {
            Faction::Alliance => Faction::Horde,
            Faction::Horde => Faction::Alliance,
        }
    }
}

/// A lasting bonus of a race (one of WoW's racial traits).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RacePassive {
    /// The name as the content file writes it (`name:`): our text, or a pointer to the client's.
    #[serde(rename = "name")]
    pub name_ref: NameRef,
    /// The name as the game shows it, filled from `name_ref` as the content loads.
    #[serde(skip)]
    pub name: String,
    pub stat: Stat,
    pub value: f32,
    /// Our tooltip line, without the name (the tooltip shows the name above it).
    pub text: String,
    /// The stock racial spell the spellbook shows it as, where that is not `name`
    /// (a run's spellbook holds the race's passives), as the file writes it
    /// (`stock_spell:`, a `Spell.dbc` pointer).
    #[serde(rename = "stock_spell", default, skip_serializing_if = "Option::is_none")]
    pub stock_spell_ref: Option<NameRef>,
    /// The stock racial spell's `Spell.dbc` name, filled from `stock_spell_ref` as the content
    /// loads.
    #[serde(skip)]
    pub stock_spell: Option<String>,
}

impl RacePassive {
    /// The stock racial spell's name: `stock_spell`, else its own name.
    pub fn stock_spell_name(&self) -> &str {
        self.stock_spell.as_deref().unwrap_or(&self.name)
    }
}

/// A playable race: the hero is a real character of it (model, faction, and its race spell).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Race {
    pub id: String,
    /// The name as the content file writes it (`name:`): our text, or a pointer to the client's.
    #[serde(rename = "name")]
    pub name_ref: NameRef,
    /// The name as the game shows it, filled from `name_ref` as the content loads.
    #[serde(skip)]
    pub name: String,
    /// WoW race id (`ChrRaces.dbc`): 1 human … 8 troll.
    pub wow_race_id: u8,
    pub faction: Faction,
    /// The faction template the hero unit carries.
    pub faction_template: u32,
    /// The race spell: cast by itself in a slot of its own, never upgraded by cards.
    pub spell: String,
    pub passives: Vec<RacePassive>,
    /// Classes (ids in `classes.ron`) the race may be beyond the ones 1.12's create screen offers
    /// it: each plays with the class's and the race's numbers unchanged.
    #[serde(default)]
    pub added_classes: Vec<String>,
    /// A look of its own for its shaman's totems; none keeps the stock
    /// look.
    #[serde(default)]
    pub totem_look: Option<TotemLook>,
}

/// A model planted in a race's totems, standing still in the ground where the totem stands: the
/// client composes it with the stock totem model at run time, from the player's own data. Lengths
/// are yards in the totem's model space at rest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TotemLook {
    /// The planted model.
    pub part: TotemPart,
    /// Its scale.
    pub scale: f32,
    /// Its tip about its own Y axis before the turn, degrees: -90 stands a weapon head up.
    pub pitch_degrees: f32,
    /// Its turn about the vertical axis, degrees, counter-clockwise seen from above.
    pub turn_degrees: f32,
    /// How deep its foot goes into the ground.
    pub sink: f32,
    /// How far the totem's particle effects on its moving bones rise.
    pub raise_effects: f32,
    /// The stock totem's body is hidden; its effects (glows, particles) stay.
    pub hide_totem_body: bool,
}

/// What a totem look plants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TotemPart {
    /// An M2 path in the player's 1.12 data.
    Model(String),
    /// A weapon's `ItemDisplayInfo.dbc` display: its model in its own texture.
    Weapon(u32),
}

// ---- talents ---------------------------------------------------------------------------------

/// The hero stats talents and jewelry modify. Nothing depends on their order: they are read and
/// written by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stat {
    Damage,
    CooldownRecovery,
    MaxHealth,
    CritChance,
    CritDamage,
    DamageReduction,
    HealthPerSecond,
    MoveSpeed,
    Area,
    PickupRange,
    Experience,
    Gold,
    Luck,
    DoubleCast,
    Lifesteal,
    HealPerKill,
    WeaponDamage,
    PetDamage,
    DamageVsCrowdControlled,
    DamageVsElites,
    DamageBelow30Pct,
    DamageVsAfflicted,
    Thorns,
    BuffPower,
    ExtraLives,
    DotStacks,
    /// Flat max health (as class gear gives), as opposed to `MaxHealth` (a percentage).
    FlatMaxHealth,
    /// Extra targets for abilities that hit several (whole number).
    ExtraTargets,
    /// Strength of the healing from the hero's spells (instant heals,
    /// heals over time, healing charges, healing from a spell's damage) and of the shields the hero
    /// casts. Health per second, lifesteal and health per kill are not touched.
    HealAndShieldPower,
    /// The chance that an enemy's melee blow is blocked, doing 30 % less
    /// damage (the Warrior's and Paladin's prestige, and the shield loadouts).
    BlockChance,
    /// Faster weapon swings (0.05 = 5 % faster).
    SwingSpeed,
    /// More rage from each weapon swing that hits (0.2 = 20 % more; the
    /// Warrior's two-hander loadout).
    RagePerSwing,
    /// More rage from damage taken (1.0 = twice as much).
    RageWhenHurt,
    /// The hero's damage reduction also adds this share of itself to
    /// its damage (1.0 = 1:1, 30 % reduction gives +30 % damage).
    DamageFromReduction,
    /// The cooldown recovery of talents, rings and trinkets. Spell
    /// cooldowns recover this much faster (0.05 = 5 %), and weapon swings speed up as if from
    /// twice it.
    SpellCooldownRecovery,
    /// Cooldown recovery for weapon swings only (0.01 = 1 % faster).
    /// The talents that speed every spell's cooldown carry it beside `SpellCooldownRecovery`, so
    /// their cut for spells (0.5 % a rank) leaves the swing pace they gave as it was.
    SwingCooldownRecovery,
    /// The chance that a weapon attack (a melee or ranged swing) hits
    /// twice, the second a full weapon hit of its own (0.05 = 5 %; Hand of Justice).
    WeaponDoubleHit,
}

impl Stat {
    /// How the talent text shows `{v}`: as a percentage, except flat rates.
    pub fn shown_as_percent(self) -> bool {
        !matches!(self, Stat::HealthPerSecond | Stat::HealPerKill)
    }
}

// ---- prestige --------------------------------------------------------------------------------

/// One stat prestige points buy (`prestige.ron`): once a class has
/// every talent point, its talent experience buys prestige points, one point a rank.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrestigeStat {
    pub id: String,
    /// As the hero sheet shows it.
    pub name: String,
    pub icon: String,
    /// What one rank gives (`per_rank` × rank): a hero stat, or a number of every spell.
    pub effect: TalentEffect,
    /// The classes that can buy it, by id; every class when empty.
    #[serde(default)]
    pub classes: Vec<String>,
    /// Most ranks it takes; no cap when `None`.
    #[serde(default)]
    pub max_ranks: Option<u32>,
}

impl PrestigeStat {
    /// Whether class `class` can buy it.
    pub fn for_class(&self, class: &str) -> bool {
        self.classes.is_empty() || self.classes.iter().any(|c| c == class)
    }

    /// What one rank adds.
    pub fn per_rank(&self) -> f32 {
        match &self.effect {
            TalentEffect::Stat { per_rank, .. } | TalentEffect::AbilityMod { per_rank, .. } => *per_rank,
            TalentEffect::Milestone(_) => 0.0,
        }
    }

    /// Whether its number reads as a percentage (a share); flat numbers and whole counts do not.
    pub fn shown_as_percent(&self) -> bool {
        match &self.effect {
            TalentEffect::Stat { stat, .. } => !matches!(
                stat,
                Stat::HealthPerSecond | Stat::HealPerKill | Stat::FlatMaxHealth | Stat::ExtraLives | Stat::DotStacks | Stat::ExtraTargets
            ),
            _ => true,
        }
    }

    /// Its bonus at `rank` as the sheet writes it: "+3%", "+15".
    pub fn bonus_text(&self, rank: u32) -> String {
        let v = self.per_rank() * rank as f32;
        let (v, unit) = if self.shown_as_percent() { (v * 100.0, "%") } else { (v, "") };
        if (v - v.round()).abs() < 1e-3 {
            format!("+{}{unit}", v.round() as i64)
        } else {
            // finer steps (0.25 %, 0.5 %) keep their digits: "+0.25%", "+1.5%"
            let s = format!("{v:.2}");
            format!("+{}{unit}", s.trim_end_matches('0'))
        }
    }

    /// Whether it adds targets, or something that stands in for them (stacks, strikes, imps, pets,
    /// critters): prestige never may.
    pub fn adds_targets(&self) -> bool {
        match &self.effect {
            TalentEffect::Stat { stat, .. } => *stat == Stat::ExtraTargets,
            TalentEffect::AbilityMod { kind, .. } => kind.is_count(),
            TalentEffect::Milestone(_) => false,
        }
    }
}

/// One effect of a talent, scaled by its rank.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TalentEffect {
    /// `per_rank × rank` added to a hero stat.
    Stat { stat: Stat, per_rank: f32 },
    /// `per_rank × rank` added to one number of every spell `filter` lets through: `Power`,
    /// `Area` and `Duration` grow by it, `Cooldown` shrinks by it (all cuts together at most
    /// 60 %), `Targets` adds it (the sum is rounded).
    AbilityMod { filter: SpellFilter, kind: Upgrade, per_rank: f32 },
    /// A rule change (see `Milestone`). Its main number grows with rank.
    Milestone(Milestone),
}

impl TalentEffect {
    /// Every spell id it names.
    pub fn spells(&self) -> Vec<&String> {
        match self {
            TalentEffect::Stat { .. } => vec![],
            TalentEffect::AbilityMod { filter, .. } => filter.spells(),
            TalentEffect::Milestone(m) => m.spells(),
        }
    }
}

/// A line's runners, a look only (Stampede's beasts): as the line strikes, a row of `count`
/// creatures (at least the first number, at most the second, picked each time) appears across
/// the line's width a little behind the hero, each shown as one of `displays` (a
/// `CreatureDisplayInfo.dbc` id and the scale it is drawn at, picked each time; the scales bring
/// every look to one size, so a row reads as one herd), runs the line's length at `speed` yards a
/// second and goes at its end. The damage stays the line's; no one is struck by a runner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineRunners {
    pub displays: Vec<(u32, f32)>,
    pub count: (u32, u32),
    pub speed: f32,
}

/// Spirits running with a lasting area that follows the hero, a look only (Sprint of the
/// Wild's wild spirits): `count` see-through creatures in the druid `form`'s look (the hero's
/// faction's), drawn `scale` times their size, beside the hero (left and right) while the area
/// lasts; as the area strikes, each strikes an enemy it struck, playing `strike_look`. The damage
/// stays the area's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreaSpirits {
    pub count: u32,
    pub form: PlayerForm,
    pub scale: f32,
    pub strike_look: u32,
}

/// What sets a milestone off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Trigger {
    /// A hit lands.
    Hit,
    /// A hit lands as a critical strike.
    Crit,
    /// A damage-over-time effect ticks.
    DotTick,
    HitOrTick,
    /// An enemy dies.
    Kill,
    /// A spell is cast.
    Cast,
    /// An enemy strikes the hero in melee.
    StruckInMelee,
    /// An enemy strikes the hero in melee while the hero is below this share of max health.
    HealthBelow(f32),
    /// A shield on the hero breaks.
    ShieldBreak,
    /// An enemy's melee blow at the hero is blocked.
    Block,
    /// A buff a spell left on the hero ends: its time runs out, or Holy Shield's last charge
    /// goes. Used by the legendary amulets.
    BuffEnds,
}

/// Which spells or sources a milestone listens to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpellFilter {
    Any,
    /// One spell, by id. `"{spell}"` in a legendary recipe stands for the remade spell.
    Spell(String),
    Spells(Vec<String>),
    /// Spells of a school (never pets).
    School(School),
    /// Spells of a delivery kind (see spells.md).
    Delivery(Delivery),
    /// Spells of a category (see spells.md).
    Category(SpellCategory),
    /// The hero's own weapon swing.
    WeaponSwing,
    /// The hero's pets.
    Pets,
    /// Hits shown with this `Spell.dbc` id: a milestone's own effect over time (`DotOnHit`'s
    /// look), a proc or an aura striking back with it, or a spell whose look (or pulse) it is.
    /// Used by the legendary amulets.
    Look(u32),
}

impl SpellFilter {
    /// The spell ids it names.
    pub fn spells(&self) -> Vec<&String> {
        match self {
            SpellFilter::Spell(s) => vec![s],
            SpellFilter::Spells(v) => v.iter().collect(),
            _ => vec![],
        }
    }

    /// Whether `spell` passes (pets and weapon swings are not spells, so those never do).
    pub fn matches(&self, spell: &Ability) -> bool {
        match self {
            SpellFilter::Any => true,
            SpellFilter::Spell(s) => *s == spell.id,
            SpellFilter::Spells(v) => v.contains(&spell.id),
            SpellFilter::School(s) => *s == spell.school,
            SpellFilter::Delivery(d) => *d == spell.delivery,
            SpellFilter::Category(c) => spell.categories.contains(c),
            SpellFilter::Look(n) => spell.look_spell == *n || spell.pulse_spell == Some(*n),
            SpellFilter::WeaponSwing | SpellFilter::Pets => false,
        }
    }
}

/// What `Milestone::Afflict` does to its target (or to the hero, for the heals).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AfflictEffect {
    Slow { amount: f32, seconds: f32 },
    Root { seconds: f32 },
    Stun { seconds: f32 },
    /// Heals the hero for a share of the damage dealt.
    HealFromDamage { share: f32 },
    /// Heals the hero for a share of max health.
    HealMaxHealth { share: f32 },
    /// The target takes `amount` more damage from everything.
    Vulnerable { amount: f32, seconds: f32 },
    /// The target flees (not a boss). Used by the legendary amulets.
    Fear { seconds: f32 },
    /// The target cannot cast: a ranged caster's shot and a boss's or rare's abilities wait.
    /// Used by the legendary amulets.
    Silence { seconds: f32 },
    /// The target is thrown `yards` straight away from the hero (not a boss).
    /// Used by the legendary amulets.
    Knockback { yards: f32 },
}

/// A milestone talent (or amulet) effect. Most react to a `Trigger` from spells that pass a
/// `SpellFilter`. Shares are fractions (0.4 = 40 %); `look_spell` and `requires_aura` are WoW spell ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Milestone {
    /// With `chance`, cast `spell` for free (empty: the spell that set it off again), at
    /// most once every `cooldown` seconds, at `power` of its damage (1 when left out).
    FreeCast {
        chance: f32,
        cooldown: f32,
        spell: String,
        trigger: Trigger,
        filter: SpellFilter,
        #[serde(default = "one", skip_serializing_if = "is_one")]
        power: f32,
    },
    /// Every `count`-th matching event casts `spell` for free.
    EveryNth { count: u32, spell: String, trigger: Trigger, filter: SpellFilter },
    /// With `chance`, summon `count` of `pet` (missing: 1) for `seconds`, at most `max` at once.
    /// At most once every `cooldown` seconds (none when left out).
    /// With `afflicted`, a `Kill` sets it off when the dead enemy
    /// carried a matching effect over time, whatever killed it (Curse of Agony's doomguard).
    PetOnTrigger {
        chance: f32,
        seconds: f32,
        max: u32,
        pet: String,
        trigger: Trigger,
        filter: SpellFilter,
        /// Shown at the spot where each one appears (the legendary looks).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        #[serde(default = "one_u32", skip_serializing_if = "is_one_u32")]
        count: u32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        afflicted: bool,
        #[serde(default, skip_serializing_if = "is_zero")]
        cooldown: f32,
    },
    /// A breaking shield bursts for `share` of what it absorbed within `radius`.
    /// `school` `None` uses the school of the spell a legendary amulet remakes.
    ShieldBurst {
        school: Option<School>,
        share: f32,
        radius: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        trigger: Trigger,
        filter: SpellFilter,
    },
    /// The hit also strikes everything within `radius` of its target for `share` of it;
    /// with `requires_aura` only if the target carries that aura.
    Splash {
        share: f32,
        radius: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        requires_aura: Option<u32>,
        trigger: Trigger,
        filter: SpellFilter,
        /// Also put on every splashed enemy (on top of the struck hit's own effect).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        effects: Vec<AfflictEffect>,
    },
    /// Each hit also strikes `count` more enemies near its target for `share` of it. With
    /// `per_swing_second`, a weapon swing's `share` is per second of the weapon's swing time (a
    /// 2 s weapon sweeps for 2 × `share`), so slow weapons sweep for more.
    ExtraTargets {
        count: u32,
        share: f32,
        trigger: Trigger,
        filter: SpellFilter,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        per_swing_second: bool,
        /// Each further enemy takes this much more than the one before (0.2: 20 % stronger per
        /// jump; the first takes `share`).
        #[serde(default, skip_serializing_if = "is_zero")]
        growth: f32,
        /// How near the struck enemy the others must be; 0: `SWEEP_REACH` (10 yd).
        #[serde(default, skip_serializing_if = "is_zero")]
        reach: f32,
        /// Shown on each enemy it sweeps on to, in place of the struck hit's own look (the legendary looks).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        /// Put on each enemy it sweeps on to.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        effects: Vec<AfflictEffect>,
    },
    /// With `chance`, the hit strikes `times` more at once for `share` of it.
    StrikeAgain {
        chance: f32,
        times: u32,
        share: f32,
        trigger: Trigger,
        filter: SpellFilter,
        /// Shown on the struck enemy as it strikes again (the legendary looks).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
    },
    /// With `chance`, apply `effect`.
    Afflict {
        chance: f32,
        effect: AfflictEffect,
        trigger: Trigger,
        filter: SpellFilter,
        /// Worn by the enemy while the effect lasts; an instant one plays on it (the legendary looks).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
    },
    /// The target takes `share` of the hit again over `seconds`, as an aura of spell `look_spell`.
    DotOnHit { look_spell: u32, share: f32, seconds: f32, trigger: Trigger, filter: SpellFilter },
    /// A killing blow by a matching spell readies it again, at most every `min_interval`.
    ResetOnKill { min_interval: f32, filter: SpellFilter },
    /// Enemies that die (killed by a matching spell, or with `afflicted` while carrying a
    /// matching effect over time) explode for `share` of their max health within `radius`.
    ExplodeOnDeath {
        afflicted: bool,
        share: f32,
        radius: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        filter: SpellFilter,
    },
    /// When an enemy dies carrying a matching effect, it jumps to the `count` nearest.
    SpreadOnDeath {
        count: u32,
        filter: SpellFilter,
        /// Shown on each enemy it jumps to (missing: the effect's own spell).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
    },
    /// `stat` + `amount` per stack for `seconds`, up to `max_stacks`; each trigger
    /// adds a stack and refreshes the time.
    TimedBuff {
        stat: Stat,
        amount: f32,
        max_stacks: u32,
        seconds: f32,
        trigger: Trigger,
        filter: SpellFilter,
        /// Worn by the hero while its stacks last (the legendary looks).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
    },
    /// Heal `share` of max health, at most once every `cooldown` seconds.
    HealOnTrigger {
        share: f32,
        cooldown: f32,
        trigger: Trigger,
        filter: SpellFilter,
        /// Plays on the hero as it heals (the legendary looks).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
    },
    /// Gives `amount` of the hero's rage or energy (its starting option's resource; nothing
    /// without one) on its trigger.
    ResourceOnTrigger { amount: f32, trigger: Trigger, filter: SpellFilter },
    /// On its trigger the hero's next `swings` weapon swings come
    /// `amount` faster (0.3 = 30 %); a new trigger starts the count over (no stacking).
    FasterSwings { amount: f32, swings: u32, trigger: Trigger, filter: SpellFilter },
    /// The hero's effects over time can strike critically (its crit
    /// chance); a critical tick deals `bonus` more (Pandemic).
    DotCrits { bonus: f32 },
    /// Every `count`-th cast of a matching summoning spell calls
    /// `extra` more summons (Nature's Grace).
    MoreSummonsEveryNth { count: u32, extra: u32, filter: SpellFilter },
    /// On its trigger the `spells` the hero knows come `seconds`
    /// sooner (Kindling), at most once every `every` seconds (0: on every trigger).
    CooldownOnTrigger {
        seconds: f32,
        spells: Vec<String>,
        trigger: Trigger,
        filter: SpellFilter,
        #[serde(default)]
        every: f32,
    },
    /// A killing blow leaves the hero at `health` instead, then untouchable for
    /// `immune_seconds`; once every `cooldown` seconds.
    CheatDeath { cooldown: f32, health: f32, immune_seconds: f32 },
    /// Healing beyond max health becomes a shield, up to `max_share` of max health.
    OverhealShield { max_share: f32 },
    /// Healing the hero does also strikes every enemy within `radius` for `share` of it.
    HealingStrikes { share: f32, radius: f32 },
    /// Every pulse of one of the hero's totems heals the hero for `share` of max health.
    TotemHeal { share: f32 },
    /// Chain spells jump without losing damage.
    ChainNoFalloff,
    /// Casts `spell` on its own every `interval` seconds (0: the spell's own cooldown).
    Periodic { interval: f32, spell: String },
    /// Every run starts with `spell` known.
    StartWith { spell: String },
    /// `pet` (from pets.ron) fights beside the hero.
    Pet { pet: String },
    /// Every `every`-th matching event (1: each one) also strikes every
    /// enemy in a line from the hero through the struck enemy, `length` yards from the hero and
    /// `width` yards either side of it, for `share` of the hit. With `behind`, only the enemies
    /// past the struck one; with `hits_target`, the struck enemy takes it too. The look,
    /// `look_spell` (missing: the hit's spell), flies from the hero to the line's end (Thori'dal,
    /// Stampede, Netherstrand Longbow); `runners`, if given, run along the line as it strikes
    /// (Stampede's beasts, [`LineRunners`]).
    LineHit {
        every: u32,
        share: f32,
        length: f32,
        width: f32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        behind: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        hits_target: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runners: Option<LineRunners>,
        trigger: Trigger,
        filter: SpellFilter,
    },
    /// An enemy that strikes the hero in melee is marked for
    /// `seconds` (showing aura `look_spell`, if given); matching spells deal `bonus` more to a
    /// marked enemy, and each of their hits on one heals the hero `heal` of max health (Mark of
    /// the Fallen Champion).
    MarkAttackers {
        seconds: f32,
        bonus: f32,
        heal: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        filter: SpellFilter,
    },
    /// A matching spell's lasting ground area grows `per_hit` of its
    /// first radius each time it damages an enemy, up to `max` more (1.0: twice as wide)
    /// (Sanctified Ground).
    GrowingArea {
        per_hit: f32,
        max: f32,
        filter: SpellFilter,
        /// The growing area's look on the ground in place of the spell's own (the legendary looks).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
    },
    /// A matching spell's lasting ground area lands on the hero (with
    /// an enemy within the spell's range) and moves with the hero while it lasts
    /// (Ranger-General's Barrage).
    AreaFollowsHero { filter: SpellFilter },
    /// Every `count`-th cast of a matching spell first pulls the
    /// enemies within `radius` yards (bosses stay) to the hero's side, showing `look_spell`
    /// from the hero onto them (Magnetic Pull).
    PullEveryNth {
        count: u32,
        radius: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        filter: SpellFilter,
    },
    /// An enemy carrying a matching effect over time leaves a patch of
    /// fire behind it as it moves (one per `radius` yards walked, at most one a second), which
    /// burns every enemy within `radius` of it once a second for `seconds`, each burn `share` of
    /// the effect's damage per second; it shows `look_spell` on the ground (Legion Flame).
    FireTrail {
        share: f32,
        radius: f32,
        seconds: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        filter: SpellFilter,
    },
    /// Each enemy a matching spell's fear lands on has `chance` to turn
    /// on the others for the fear's time: it chases the nearest other enemy and strikes it with
    /// its own blows, never the hero (Dominate Mind).
    FearedTurn {
        chance: f32,
        filter: SpellFilter,
        /// Worn by a turned enemy while it fights for the hero (the legendary looks).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
    },
    // ---- our additions (the legendary amulets, one per spell) ----
    /// Every `count`-th time (a cast of a matching spell, or a hit or tick of one, as `per`
    /// says; with `per_enemy` counted on each enemy apart) the milestones in `then` go off too, on
    /// their own triggers: for a counted cast, on its hits until the spell is cast again; for a
    /// counted hit, on that hit. With `chance` below 1, a counted time only goes off that often;
    /// with `first_only`, a counted cast's first event that sets one of them off is its last.
    Every {
        count: u32,
        per: Counted,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        per_enemy: bool,
        #[serde(default = "one", skip_serializing_if = "is_one")]
        chance: f32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        first_only: bool,
        filter: SpellFilter,
        then: Vec<Milestone>,
    },
    /// With `chance`, the matching spell that set it off is ready again, at most every
    /// `min_interval` (`ResetOnKill` on any trigger: a crit readies it).
    ResetOnTrigger { chance: f32, min_interval: f32, trigger: Trigger, filter: SpellFilter },
    /// Strikes one more enemy within `reach` of the hero (0: the spell's own range), chosen by
    /// `pick` (never the struck one), for `share` of the hit, putting `effects` on it. With
    /// `reflection`, a look only: a see-through reflection of the hero appears at that enemy,
    /// strikes it with `look_spell` and fades (Blink Strike; the hero stays where it is).
    StrikeOne {
        pick: Pick,
        reach: f32,
        share: f32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        effects: Vec<AfflictEffect>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        reflection: bool,
        trigger: Trigger,
        filter: SpellFilter,
    },
    /// A burst at `at` (`count` of them, spread up to `scatter` yd from it), `delay` seconds
    /// after its trigger: `share` of `of` to every enemy within `radius`, putting `effects` on
    /// each. Its damage counts as the spell's.
    Burst {
        share: f32,
        of: BurstBase,
        radius: f32,
        at: Spot,
        #[serde(default, skip_serializing_if = "is_zero")]
        delay: f32,
        #[serde(default = "one_u32", skip_serializing_if = "is_one_u32")]
        count: u32,
        #[serde(default, skip_serializing_if = "is_zero")]
        scatter: f32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        effects: Vec<AfflictEffect>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        /// A delayed burst that follows its enemy: worn by that enemy until it goes off (the legendary looks).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mark_spell: Option<u32>,
        trigger: Trigger,
        filter: SpellFilter,
    },
    /// A matching effect over time on an enemy bursts when it runs out (`on_end`) and when its
    /// carrier dies (`on_death`): `share` of its whole damage (a tick × the spell's ticks) to
    /// every other enemy within `radius`; with `spreads`, each one struck gets the effect too.
    DotBurst {
        share: f32,
        radius: f32,
        on_end: bool,
        on_death: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        spreads: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        filter: SpellFilter,
    },
    /// On its trigger, a matching effect over time on the struck enemy passes to the `count`
    /// nearest within `reach` (0: `SPREAD_REACH`) that lack it, each tick `growth` stronger; with
    /// `moves` it leaves the enemy it was on. At most `max_carriers` enemies carry it (0: no
    /// limit), at most once every `every` seconds (0: every time). On a hit (not a tick) the
    /// spell's own burn is what passes.
    DotSpread {
        count: u32,
        #[serde(default, skip_serializing_if = "is_zero")]
        reach: f32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        max_carriers: u32,
        #[serde(default, skip_serializing_if = "is_zero")]
        every: f32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        moves: bool,
        #[serde(default, skip_serializing_if = "is_zero")]
        growth: f32,
        /// Shown on each enemy it passes to (missing: the effect's own spell).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        trigger: Trigger,
        filter: SpellFilter,
    },
    /// Leaves `count` lasting areas at `at` (spread up to `scatter` yd), each `radius` yd for
    /// `seconds`, striking every enemy inside once a second for `share` of `of` (its damage
    /// counts as the spell's) and slowing them by `slow`; with `follows_hero` it moves with the
    /// hero, with `rides_target` with the struck enemy.
    LeaveArea {
        share: f32,
        of: BurstBase,
        seconds: f32,
        radius: f32,
        at: Spot,
        #[serde(default = "one_u32", skip_serializing_if = "is_one_u32")]
        count: u32,
        #[serde(default, skip_serializing_if = "is_zero")]
        scatter: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        slow: f32,
        /// It moves with the hero while it lasts.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        follows_hero: bool,
        /// It puts the poison of the hero's starting option on every enemy inside, each strike.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        poisons: bool,
        /// It rides along with the struck enemy while that one lives.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        rides_target: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        /// Spirits that run with the area while it lasts, a look only ([`AreaSpirits`]).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spirits: Option<AreaSpirits>,
        trigger: Trigger,
        filter: SpellFilter,
    },
    /// While the buff `spell` left on the hero is up: `stats` added, and the milestones in
    /// `then` go off on their triggers; with `once`, only the first of them that goes off, once
    /// a buff.
    WhileBuff {
        spell: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        stats: Vec<(Stat, f32)>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        then: Vec<Milestone>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        once: bool,
        /// Worn by the hero while the buff is up (the legendary looks).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
    },
    /// Every `every` seconds (from the first enemy in reach), for `seconds`: casts `spell` for
    /// free every `interval` and adds `stats` (Bladestorm).
    Spree {
        every: f32,
        seconds: f32,
        interval: f32,
        spell: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        stats: Vec<(Stat, f32)>,
        /// Worn by the hero while the spree lasts (the legendary looks).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
    },
    /// On its trigger, for `seconds`: every `interval` a random enemy within `reach` of the hero
    /// is struck for `share` of `of` (its damage counts as the spell's).
    Barrage {
        seconds: f32,
        interval: f32,
        reach: f32,
        share: f32,
        of: BurstBase,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        trigger: Trigger,
        filter: SpellFilter,
    },
    /// Each own cast of a matching spell casts it again for free `delay` seconds later; with
    /// `opposite` a returning spell flies the other way, with `elsewhere` a ground area falls on
    /// the best spot its first did not cover.
    CastAgain {
        delay: f32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        opposite: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        elsewhere: bool,
        filter: SpellFilter,
    },
    /// A heal of a matching spell heals `share` of it again over `seconds`.
    HealAfter { share: f32, seconds: f32, filter: SpellFilter },
    /// A cast of a matching heal puts an effect over time on `targets` enemies within `reach` yd
    /// of the hero (the nearest, or in `order` when it is set), each dealing `share` of the
    /// cast's whole healing over `seconds`, as an aura of spell `look_spell`.
    /// Used by Renew's legendary, "Deplete".
    DotFromHeal {
        share: f32,
        seconds: f32,
        targets: u32,
        reach: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        order: Option<crate::TargetOrder>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        filter: SpellFilter,
    },
    /// A matching spell that waits for a condition before it is cast (a heal for low health, an
    /// immunity for lower) is cast whenever it is ready and an enemy is within 30 yd
    /// (Renew's legendary).
    CastOnCooldown { filter: SpellFilter },
    /// On its trigger, the struck enemy gets the poison of the hero's starting option (a spell
    /// with `Mechanic::Poisons`); nothing for an option without one.
    ApplyPoison { trigger: Trigger, filter: SpellFilter },
    /// Gives `points` combo points on its trigger (nothing without a combo resource).
    ComboPoints { points: u32, trigger: Trigger, filter: SpellFilter },
    /// A melee blow taken strikes its attacker back for `share` of what it did (before the
    /// shield took any).
    Retaliate {
        share: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
    },
}

/// What `Milestone::Every` counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Counted {
    /// The spell's own casts (not free ones).
    Casts,
    /// Its hits and ticks.
    Hits,
}

/// Which enemy `Milestone::StrikeOne` takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pick {
    Nearest,
    Healthiest,
    Farthest,
}

/// Where a burst or a lasting area a milestone leaves goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Spot {
    /// Where the struck enemy stands (the hero, for an event with no enemy).
    Target,
    Hero,
}

/// What a burst's share is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BurstBase {
    /// The hit or tick that set it off (the absorbed amount for a breaking shield).
    Hit,
    /// One hit of the spell that set it off (its listed damage, scaled as its cast is).
    SpellHit,
    /// The hero's max health × the hero's damage (as a shield burst's absorbed amount is).
    MaxHealth,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

fn one_u32() -> u32 {
    1
}

fn is_one_u32(v: &u32) -> bool {
    *v == 1
}

impl Milestone {
    /// Whether its main number (`rank_value_mut`) grows when a worn amulet is empowered.
    pub fn grows_when_empowered(&self) -> bool {
        matches!(
            self,
            Milestone::FreeCast { .. }
                | Milestone::Splash { .. }
                | Milestone::ExtraTargets { .. }
                | Milestone::StrikeAgain { .. }
                | Milestone::Afflict { .. }
                | Milestone::DotOnHit { .. }
                | Milestone::TimedBuff { .. }
                | Milestone::HealOnTrigger { .. }
                | Milestone::ResourceOnTrigger { .. }
                | Milestone::FasterSwings { .. }
                | Milestone::DotCrits { .. }
                | Milestone::TotemHeal { .. }
                | Milestone::LineHit { .. }
                | Milestone::MarkAttackers { .. }
                | Milestone::FireTrail { .. }
                | Milestone::FearedTurn { .. }
                | Milestone::ResetOnTrigger { .. }
                | Milestone::StrikeOne { .. }
                | Milestone::Burst { .. }
                | Milestone::DotBurst { .. }
                | Milestone::LeaveArea { .. }
                | Milestone::Barrage { .. }
                | Milestone::HealAfter { .. }
                | Milestone::DotFromHeal { .. }
                | Milestone::Retaliate { .. }
        )
    }

    /// The number a talent rank multiplies (and the tooltip shows), if any.
    pub fn rank_value_mut(&mut self) -> Option<&mut f32> {
        match self {
            Milestone::FreeCast { chance, .. }
            | Milestone::PetOnTrigger { chance, .. }
            | Milestone::StrikeAgain { chance, .. }
            | Milestone::Afflict { chance, .. }
            | Milestone::ResetOnTrigger { chance, .. }
            | Milestone::FearedTurn { chance, .. } => Some(chance),
            Milestone::LineHit { share, .. } | Milestone::FireTrail { share, .. } => Some(share),
            Milestone::MarkAttackers { bonus, .. } => Some(bonus),
            Milestone::GrowingArea { per_hit, .. } => Some(per_hit),
            Milestone::ShieldBurst { share, .. }
            | Milestone::Splash { share, .. }
            | Milestone::ExtraTargets { share, .. }
            | Milestone::DotOnHit { share, .. }
            | Milestone::ExplodeOnDeath { share, .. }
            | Milestone::HealOnTrigger { share, .. }
            | Milestone::TotemHeal { share }
            | Milestone::StrikeOne { share, .. }
            | Milestone::Burst { share, .. }
            | Milestone::DotBurst { share, .. }
            | Milestone::LeaveArea { share, .. }
            | Milestone::Barrage { share, .. }
            | Milestone::HealAfter { share, .. }
            | Milestone::DotFromHeal { share, .. }
            | Milestone::Retaliate { share, .. } => Some(share),
            Milestone::TimedBuff { amount, .. } | Milestone::ResourceOnTrigger { amount, .. } | Milestone::FasterSwings { amount, .. } => {
                Some(amount)
            }
            Milestone::DotCrits { bonus } => Some(bonus),
            Milestone::CooldownOnTrigger { seconds, .. } => Some(seconds),
            _ => None,
        }
    }

    /// Every spell id it names (in spells, filters and its free cast).
    pub fn spells(&self) -> Vec<&String> {
        let mut out = self.filter().map_or_else(Vec::new, SpellFilter::spells);
        match self {
            Milestone::FreeCast { spell, .. }
            | Milestone::EveryNth { spell, .. }
            | Milestone::Periodic { spell, .. }
            | Milestone::StartWith { spell }
            | Milestone::Spree { spell, .. }
                if !spell.is_empty() =>
            {
                out.push(spell)
            }
            Milestone::CooldownOnTrigger { spells, .. } => out.extend(spells),
            Milestone::Every { then, .. } => out.extend(then.iter().flat_map(Milestone::spells)),
            Milestone::WhileBuff { spell, then, .. } => {
                out.push(spell);
                out.extend(then.iter().flat_map(Milestone::spells));
            }
            _ => {}
        }
        out
    }

    pub fn filter(&self) -> Option<&SpellFilter> {
        match self {
            Milestone::FreeCast { filter, .. }
            | Milestone::EveryNth { filter, .. }
            | Milestone::PetOnTrigger { filter, .. }
            | Milestone::ShieldBurst { filter, .. }
            | Milestone::Splash { filter, .. }
            | Milestone::ExtraTargets { filter, .. }
            | Milestone::StrikeAgain { filter, .. }
            | Milestone::Afflict { filter, .. }
            | Milestone::DotOnHit { filter, .. }
            | Milestone::ResetOnKill { filter, .. }
            | Milestone::ExplodeOnDeath { filter, .. }
            | Milestone::SpreadOnDeath { filter, .. }
            | Milestone::TimedBuff { filter, .. }
            | Milestone::HealOnTrigger { filter, .. }
            | Milestone::ResourceOnTrigger { filter, .. }
            | Milestone::FasterSwings { filter, .. }
            | Milestone::CooldownOnTrigger { filter, .. }
            | Milestone::MoreSummonsEveryNth { filter, .. }
            | Milestone::LineHit { filter, .. }
            | Milestone::MarkAttackers { filter, .. }
            | Milestone::GrowingArea { filter, .. }
            | Milestone::AreaFollowsHero { filter }
            | Milestone::PullEveryNth { filter, .. }
            | Milestone::FireTrail { filter, .. }
            | Milestone::Every { filter, .. }
            | Milestone::ResetOnTrigger { filter, .. }
            | Milestone::StrikeOne { filter, .. }
            | Milestone::Burst { filter, .. }
            | Milestone::DotBurst { filter, .. }
            | Milestone::DotSpread { filter, .. }
            | Milestone::LeaveArea { filter, .. }
            | Milestone::Barrage { filter, .. }
            | Milestone::CastAgain { filter, .. }
            | Milestone::HealAfter { filter, .. }
            | Milestone::DotFromHeal { filter, .. }
            | Milestone::CastOnCooldown { filter }
            | Milestone::ApplyPoison { filter, .. }
            | Milestone::ComboPoints { filter, .. }
            | Milestone::FearedTurn { filter, .. } => Some(filter),
            _ => None,
        }
    }

    pub fn filter_mut(&mut self) -> Option<&mut SpellFilter> {
        match self {
            Milestone::FreeCast { filter, .. }
            | Milestone::EveryNth { filter, .. }
            | Milestone::PetOnTrigger { filter, .. }
            | Milestone::ShieldBurst { filter, .. }
            | Milestone::Splash { filter, .. }
            | Milestone::ExtraTargets { filter, .. }
            | Milestone::StrikeAgain { filter, .. }
            | Milestone::Afflict { filter, .. }
            | Milestone::DotOnHit { filter, .. }
            | Milestone::ResetOnKill { filter, .. }
            | Milestone::ExplodeOnDeath { filter, .. }
            | Milestone::SpreadOnDeath { filter, .. }
            | Milestone::TimedBuff { filter, .. }
            | Milestone::HealOnTrigger { filter, .. }
            | Milestone::ResourceOnTrigger { filter, .. }
            | Milestone::FasterSwings { filter, .. }
            | Milestone::CooldownOnTrigger { filter, .. }
            | Milestone::MoreSummonsEveryNth { filter, .. }
            | Milestone::LineHit { filter, .. }
            | Milestone::MarkAttackers { filter, .. }
            | Milestone::GrowingArea { filter, .. }
            | Milestone::AreaFollowsHero { filter }
            | Milestone::PullEveryNth { filter, .. }
            | Milestone::FireTrail { filter, .. }
            | Milestone::Every { filter, .. }
            | Milestone::ResetOnTrigger { filter, .. }
            | Milestone::StrikeOne { filter, .. }
            | Milestone::Burst { filter, .. }
            | Milestone::DotBurst { filter, .. }
            | Milestone::DotSpread { filter, .. }
            | Milestone::LeaveArea { filter, .. }
            | Milestone::Barrage { filter, .. }
            | Milestone::CastAgain { filter, .. }
            | Milestone::HealAfter { filter, .. }
            | Milestone::DotFromHeal { filter, .. }
            | Milestone::CastOnCooldown { filter }
            | Milestone::ApplyPoison { filter, .. }
            | Milestone::ComboPoints { filter, .. }
            | Milestone::FearedTurn { filter, .. } => Some(filter),
            _ => None,
        }
    }

    /// The pet it summons, if any.
    pub fn pet(&self) -> Option<&String> {
        match self {
            Milestone::PetOnTrigger { pet, .. } | Milestone::Pet { pet } => Some(pet),
            _ => None,
        }
    }
}

/// A companion summoned by talents (wolf, imp, treants …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pet {
    pub name: String,
    /// The creature entry the client is told about, numbered as the content loads (`entries`).
    #[serde(skip)]
    pub entry: u32,
    pub display_id: u32,
    pub scale: f32,
    pub damage: f32,
    /// Seconds between attacks.
    pub swing: f32,
    /// The `Spell.dbc` id of its ranged attack (the Imp's); missing: it fights in melee.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ranged_spell: Option<u32>,
    pub school: School,
    /// A beast's family: its size then grows with its hero's level ([`PetFamily::scale_at`]) in
    /// place of `scale`. Missing: it keeps `scale`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<PetFamily>,
    /// A lasting look it wears from the moment it appears: the `Spell.dbc` id of an aura whose
    /// state look plays on its body (the Phoenix burns).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aura: Option<u32>,
    /// A pet that flies: it hovers this many yards above the ground, bobbing gently. Missing: it
    /// walks on the ground.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hover: Option<f32>,
}

impl Pet {
    /// Its size beside a hero of `level`: its family's, else its own `scale`.
    pub fn scale_at(&self, level: u32) -> f32 {
        self.family.map_or(self.scale, |f| f.scale_at(level))
    }
}

/// The beast families a pet can be, each growing with its hero's level as 1.12 grows a hunter's
/// pet with its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PetFamily {
    Wolf,
    Cat,
    Bear,
    Raptor,
}

impl PetFamily {
    /// The family's size at its first level, that level, its size at its last level and that
    /// level, as the player's `CreatureFamily.dbc` gives them (min scale, min level, max scale,
    /// max level).
    pub const fn scale_range(self) -> (f32, u32, f32, u32) {
        match self {
            PetFamily::Wolf => (0.7, 1, 1.0, 60),
            PetFamily::Cat => (0.7, 1, 1.1, 60),
            PetFamily::Bear => (0.6, 1, 1.0, 60),
            PetFamily::Raptor => (0.5, 1, 0.8, 60),
        }
    }

    /// The size at `level`: from the first level's to the last's in even steps, held at each end.
    pub fn scale_at(self, level: u32) -> f32 {
        let (min_scale, min_level, max_scale, max_level) = self.scale_range();
        if level <= min_level || max_level <= min_level {
            return min_scale;
        }
        if level >= max_level {
            return max_scale;
        }
        let t = (level - min_level) as f32 / (max_level - min_level) as f32;
        min_scale + (max_scale - min_scale) * t
    }
}

#[cfg(test)]
mod pet_family_tests {
    use super::PetFamily;

    #[test]
    fn a_family_grows_from_its_first_level_to_its_last() {
        assert_eq!(PetFamily::Cat.scale_at(1), 0.7);
        assert_eq!(PetFamily::Cat.scale_at(60), 1.1);
        assert_eq!(PetFamily::Cat.scale_at(70), 1.1, "held past the last level");
        let mid = PetFamily::Wolf.scale_at(30);
        assert!((mid - (0.7 + 0.3 * 29.0 / 59.0)).abs() < 1e-6, "{mid}");
        assert!(PetFamily::Raptor.scale_at(2) > PetFamily::Raptor.scale_at(1));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Talent {
    pub id: String,
    /// The name as the content file writes it (`name:`): our text, or a pointer to the client's.
    #[serde(rename = "name")]
    pub name_ref: NameRef,
    /// The name as the game shows it, filled from `name_ref` as the content loads.
    #[serde(skip)]
    pub name: String,
    pub icon: String,
    /// Tooltip with `{v}` for the current value.
    pub text: String,
    pub row: u8,
    pub col: u8,
    pub max_rank: u8,
    /// Talent id that must be learned first.
    pub requires: Option<String>,
    pub effects: Vec<TalentEffect>,
}

/// The talent rows that hold the milestones (11, 21 and 31 points into a tree).
pub const MILESTONE_ROWS: [u8; 3] = [2, 4, 6];

impl Talent {
    /// Whether it is a milestone: it changes how spells play (a `Milestone` effect), or it is the
    /// one-rank talent of a milestone row, whatever it does (a
    /// milestone may only make spells stronger).
    pub fn is_milestone(&self) -> bool {
        self.effects.iter().any(|e| matches!(e, TalentEffect::Milestone(_)))
            || (self.max_rank == 1 && MILESTONE_ROWS.contains(&self.row))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TalentTree {
    pub class: String,
    /// The name as the file writes it (`name:`): our text, or a pointer to the client's.
    #[serde(rename = "name")]
    pub name_ref: NameRef,
    /// The name as the game shows it, filled from `name_ref` as the content loads.
    #[serde(skip)]
    pub name: String,
    /// `Interface\TalentFrame\<background>-*.blp`.
    pub background: String,
    pub icon: String,
    pub talents: Vec<Talent>,
}

// ---- items -----------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JewelrySlot {
    Ring,
    Trinket,
}

/// A ring or trinket found in caches; lasts one run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jewelry {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub slot: JewelrySlot,
    /// Tooltip with `{v}`.
    pub text: String,
    /// The stat it adds to.
    pub stat: Stat,
    /// Base value at its first rank; upgrades raise it.
    pub value: f32,
    /// A chance for each weapon attack to hit twice that the piece adds beside its stat, shown as
    /// `{w}` in its text (Hand of Justice).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_hit: Option<JewelryDoubleHit>,
}

/// A weapon double hit that follows a piece's stat value: `base` when the stat is at `from`, plus
/// `per` of whatever the stat has above it (`Jewelry::double_hit_at`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct JewelryDoubleHit {
    pub base: f32,
    pub from: f32,
    pub per: f32,
}

impl Jewelry {
    /// The weapon double hit the piece adds with its stat at `value` (0: none).
    pub fn double_hit_at(&self, value: f32) -> f32 {
        self.double_hit.map_or(0.0, |d| (d.base + (value - d.from) * d.per).max(0.0))
    }

    /// Its text with its stat at `value`: `{v}` that value, `{w}` its weapon double hit.
    pub fn text_at(&self, value: f32) -> String {
        let text = crate::text::with_value(&self.text, value);
        text.replace("{w}%", &crate::text::percent(self.double_hit_at(value))).replace("{w}", &crate::text::number(self.double_hit_at(value)))
    }
}

// ---- run options -----------------------------------------------------------------------------

/// A difficulty slider on the Run Options screen, stored as a whole percentage per hero.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOption {
    pub id: String,
    pub name: String,
    pub tooltip: String,
    pub min_pct: u16,
    pub max_pct: u16,
    pub step_pct: u16,
    /// How much a point of this option adds to the run's reward bonus.
    pub reward_weight: f32,
}

impl RunOption {
    /// One press of the slider: move by a step and clamp to `[min_pct, max_pct]`.
    pub fn stepped(&self, current: u16, up: bool) -> u16 {
        let next = if up {
            current.saturating_add(self.step_pct)
        } else {
            current.saturating_sub(self.step_pct)
        };
        next.clamp(self.min_pct, self.max_pct)
    }
}

// ---- class gear ------------------------------------------------------------------------------

/// Armor type of a class set; picks the slot nouns ("Circlet" vs "Helm") and the cloth robe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArmorType {
    Cloth,
    Leather,
    Mail,
    Plate,
}

/// A stat a gear piece grants, before tier scaling.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GearStat {
    pub stat: Stat,
    pub value: f32,
}

/// An armor slot of the class gear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArmorSlot {
    Head,
    Shoulders,
    Chest,
    Waist,
    Legs,
    Feet,
    Wrists,
    Hands,
}

/// The client's inventory type of a cloth chest: a robe.
pub const ROBE_INVENTORY_TYPE: u32 = 20;

impl ArmorSlot {
    /// Its id (`head`, `shoulders` ...).
    pub fn id(self) -> &'static str {
        match self {
            ArmorSlot::Head => "head",
            ArmorSlot::Shoulders => "shoulders",
            ArmorSlot::Chest => "chest",
            ArmorSlot::Waist => "waist",
            ArmorSlot::Legs => "legs",
            ArmorSlot::Feet => "feet",
            ArmorSlot::Wrists => "wrists",
            ArmorSlot::Hands => "hands",
        }
    }

    /// The client's inventory type for a piece of it (`INVTYPE_HEAD` 1 ... `INVTYPE_HANDS` 10).
    pub fn inventory_type(self) -> u32 {
        match self {
            ArmorSlot::Head => 1,
            ArmorSlot::Shoulders => 3,
            ArmorSlot::Chest => 5,
            ArmorSlot::Waist => 6,
            ArmorSlot::Legs => 7,
            ArmorSlot::Feet => 8,
            ArmorSlot::Wrists => 9,
            ArmorSlot::Hands => 10,
        }
    }

    /// The client's equipment slot a piece of it is drawn in.
    pub fn equip_slot(self) -> u8 {
        match self {
            ArmorSlot::Head => 0,
            ArmorSlot::Shoulders => 2,
            ArmorSlot::Chest => 4,
            ArmorSlot::Waist => 5,
            ArmorSlot::Legs => 6,
            ArmorSlot::Feet => 7,
            ArmorSlot::Wrists => 8,
            ArmorSlot::Hands => 9,
        }
    }
}

/// What kind of weapon (or shield, or off-hand item) a piece is: it sets the client's item class,
/// subclass and inventory type, and where it rests when put away: staves
/// and two-handers on the back, one-handers on the hip, a shield on the back, a ranged weapon and
/// an item held in the off hand nowhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WeaponKind {
    OneHandedAxe,
    TwoHandedAxe,
    Bow,
    Gun,
    OneHandedMace,
    TwoHandedMace,
    Polearm,
    OneHandedSword,
    TwoHandedSword,
    Staff,
    FistWeapon,
    Dagger,
    Thrown,
    Crossbow,
    Wand,
    Shield,
    /// A frill held in the off hand (an orb, a tome, a branch; 1.12's "held in off-hand"): it shows,
    /// but never strikes.
    HeldInOffHand,
}

impl WeaponKind {
    /// The client's item class: 2 a weapon, 4 armor (a shield, an off-hand item).
    pub fn item_class(self) -> u32 {
        if matches!(self, WeaponKind::Shield | WeaponKind::HeldInOffHand) {
            4
        } else {
            2
        }
    }

    /// The client's item subclass.
    pub fn subclass(self) -> u32 {
        match self {
            WeaponKind::OneHandedAxe => 0,
            WeaponKind::TwoHandedAxe => 1,
            WeaponKind::Bow => 2,
            WeaponKind::Gun => 3,
            WeaponKind::OneHandedMace => 4,
            WeaponKind::TwoHandedMace => 5,
            WeaponKind::Polearm => 6,
            WeaponKind::OneHandedSword => 7,
            WeaponKind::TwoHandedSword => 8,
            WeaponKind::Staff => 10,
            WeaponKind::FistWeapon => 13,
            WeaponKind::Dagger => 15,
            WeaponKind::Thrown => 16,
            WeaponKind::Crossbow => 18,
            WeaponKind::Wand => 19,
            WeaponKind::Shield => 6,
            WeaponKind::HeldInOffHand => 0,
        }
    }

    /// It is held in both hands (inventory type two-hand), so nothing is in the off hand.
    pub fn two_handed(self) -> bool {
        self.inventory_type() == 17
    }

    /// The client's inventory type: one-hand 13, two-hand 17, bow 15, gun, crossbow and wand
    /// 26, thrown 25, shield 14, held in the off hand 23.
    pub fn inventory_type(self) -> u32 {
        use WeaponKind::*;
        match self {
            OneHandedAxe | OneHandedMace | OneHandedSword | FistWeapon | Dagger => 13,
            TwoHandedAxe | TwoHandedMace | TwoHandedSword | Polearm | Staff => 17,
            Bow => 15,
            Gun | Crossbow | Wand => 26,
            Thrown => 25,
            Shield => 14,
            HeldInOffHand => 23,
        }
    }

    /// Where it rests when put away, as the client's sheath type: 1 two-handed on the back, 2 a
    /// staff on the back, 3 on the hip, 4 a shield on the back, 0 nowhere (shown only drawn).
    pub fn sheath(self) -> u32 {
        use WeaponKind::*;
        match self {
            TwoHandedAxe | TwoHandedMace | TwoHandedSword | Polearm => 1,
            Staff => 2,
            OneHandedAxe | OneHandedMace | OneHandedSword | FistWeapon | Dagger => 3,
            Shield => 4,
            Bow | Gun | Crossbow | Wand | Thrown | HeldInOffHand => 0,
        }
    }
}

/// The weapon slots a class gear set has room for (roadmap R3): three weapon
/// sets of a main hand and an off hand.
pub const WEAPON_SLOTS: u32 = 6;

/// The hand a weapon is held in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Hand {
    MainHand,
    OffHand,
    Ranged,
}

impl Hand {
    /// The client's equipment slot: 15 main hand, 16 off hand, 17 ranged.
    pub fn equip_slot(self) -> u8 {
        match self {
            Hand::MainHand => 15,
            Hand::OffHand => 16,
            Hand::Ranged => 17,
        }
    }
}

/// One of the eight armor slots. Its stats are the same for every class.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GearSlot {
    pub slot: ArmorSlot,
    /// Item noun per armor type: cloth, leather, mail, plate.
    pub names: Vec<String>,
    pub stats: Vec<GearStat>,
}

/// One zone's look of a class set (every zone its own look): the eight
/// armor pieces' names and `ItemDisplayInfo.dbc` looks, in slot order. A zone may have several
/// rows of looks (Blackrock's and Stratholme's two); a piece of the zone unlocks every row. A look
/// of a zone the content does not have (yet) waits for it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GearLook {
    /// The zone's id (`content/zones/<id>.ron`).
    pub zone: String,
    /// Its row among the zone's looks, 0 first.
    #[serde(default, skip_serializing_if = "is_first_row")]
    pub row: u32,
    /// The row's own name where the collection and the look's tooltip show one
    /// (Stratholme's second row, "Scholomance sets"); none keeps "Second look".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_name: Option<String>,
    /// Each armor piece's name.
    pub names: Vec<String>,
    /// Each armor piece's look; 0 borrows the nearest look of another zone (see `esurv-sim` gear).
    pub displays: Vec<u32>,
}

fn is_first_row(n: &u32) -> bool {
    *n == 0
}

/// A class gear weapon: its stats, and the kind and hand of the piece the item query tells of. It
/// is named and shown as its zone's armaments: the weapon loadout's weapons in the zone's looks
/// (`loadouts.ron`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weapon {
    pub stats: Vec<GearStat>,
    pub kind: WeaponKind,
    pub hand: Hand,
}

impl GearSlot {
    /// The client's inventory type for a piece of it (a cloth chest is a robe: `ROBE_INVENTORY_TYPE`).
    pub fn inventory_type(&self) -> u32 {
        self.slot.inventory_type()
    }

    /// The client's equipment slot a piece of it is drawn in.
    pub fn equip_slot(&self) -> u8 {
        self.slot.equip_slot()
    }
}

impl Weapon {
    pub fn item_class(&self) -> u32 {
        self.kind.item_class()
    }

    pub fn subclass(&self) -> u32 {
        self.kind.subclass()
    }

    pub fn inventory_type(&self) -> u32 {
        self.kind.inventory_type()
    }

    pub fn sheath(&self) -> u32 {
        self.kind.sheath()
    }

    pub fn equip_slot(&self) -> u8 {
        self.hand.equip_slot()
    }
}

/// The gear one class earns: an eight-piece set in a look per zone, and its weapons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GearSet {
    pub class: String,
    pub armor: ArmorType,
    /// The bonus pair (a place in `Gear::bonus_stats`) a piece of each slot adds from the second
    /// zone on: the eight armor slots, then one per weapon.
    pub bonus_pairs: Vec<u32>,
    /// The set's looks, a zone's (and its rows) after another in the zones' difficulty order.
    pub looks: Vec<GearLook>,
    pub weapons: Vec<Weapon>,
}

impl GearSet {
    /// The place in `looks` of zone `zone`'s (an id) look of row `row`.
    pub fn look_of(&self, zone: &str, row: u32) -> Option<usize> {
        self.looks.iter().position(|l| l.zone == zone && l.row == row)
    }
}

/// Everything about class gear: shared slot stats, the zone scaling, and each class's set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gear {
    /// The stats here (the slots', weapons' and bonus pairs') are a piece's
    /// from the first zone; a piece from a zone of difficulty D has them × `balance.ron`
    /// `zone_scaling`'s gear factor of D, rounded.
    pub slots: Vec<GearSlot>,
    /// Eight pairs; a piece from the second zone on adds the one its set's `bonus_pairs` names
    /// for its slot.
    pub bonus_stats: Vec<Vec<GearStat>>,
    pub sets: Vec<GearSet>,
}

/// A stock weapon or shield of a starting outfit, by its item entry: what the item query tells of
/// it beyond the look and slot the player's data holds (`CharStartOutfit.dbc`). Where it rests
/// when not in use is staves on the back, one-handers on the hip.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutfitWeapon {
    /// Its item entry, as `CharStartOutfit.dbc` names it.
    pub item: u32,
    /// Its `ItemDisplayInfo.dbc` look, as `CharStartOutfit.dbc` gives it (for the character
    /// select's figures, which know an outfit piece by its look).
    pub display: u32,
    pub kind: WeaponKind,
}

impl OutfitWeapon {
    pub fn item_class(&self) -> u32 {
        self.kind.item_class()
    }

    pub fn subclass(&self) -> u32 {
        self.kind.subclass()
    }

    pub fn sheath(&self) -> u32 {
        self.kind.sheath()
    }
}

// ---- card words ------------------------------------------------------------------------------

/// The words cards put around their numbers, in `words.ron` (our own; one table so every card
/// reads alike). Spell descriptions live with each spell (`Ability::description`), a ring's or
/// trinket's line with the piece (`Jewelry::text`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Words {
    /// What an upgrade card improves, after its number: "+34% damage".
    pub upgrades: Vec<UpgradeWords>,
    /// What `Power` improves on a spell that heals, or that shields, instead of "damage".
    pub power_of_heals: String,
    pub power_of_shields: String,
    /// After an upgrade's gain an amulet gives every spell: "+1 target for every spell".
    pub every_spell: String,
    /// A hero stat as a line (an amulet's): `{v}` is its value, `{v}%` a share as a percentage.
    pub stats: Vec<StatWords>,
    /// A buff on the hero, as its tooltip says what it does now, by the run's buff kind.
    #[serde(default)]
    pub buffs: Vec<BuffWords>,
}

/// What one kind of buff does, as its tooltip says it: `{v}` its value now, `{v}%` a share as a
/// percentage; and its name where no spell of the hero's names it (a rune's).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuffWords {
    /// The buff kind (`Speed`, `Damage`, `Shield` ...).
    pub kind: BuffKind,
    /// The name as the file writes it (`name:`): our text, or a pointer to the client's.
    #[serde(rename = "name")]
    pub name_ref: NameRef,
    /// The name as the game shows it, filled from `name_ref` as the content loads.
    #[serde(skip)]
    pub name: String,
    pub line: String,
}

/// The words after one upgrade's number.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeWords {
    pub upgrade: Upgrade,
    pub words: String,
    /// The words when the number is exactly 1 ("+1 target"), where they differ.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub one: Option<String>,
}

/// A stat's line, `{v}` its value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatWords {
    pub stat: Stat,
    pub line: String,
}

impl Words {
    /// The words after an upgrade's number `n`.
    pub fn upgrade(&self, upgrade: Upgrade, n: f32) -> Option<&str> {
        let w = self.upgrades.iter().find(|w| w.upgrade == upgrade)?;
        Some(match &w.one {
            Some(one) if n == 1.0 => one,
            _ => &w.words,
        })
    }

    /// A stat's line template.
    pub fn stat(&self, stat: Stat) -> Option<&str> {
        self.stats.iter().find(|s| s.stat == stat).map(|s| s.line.as_str())
    }

    /// A buff kind's words.
    pub fn buff(&self, kind: BuffKind) -> Option<&BuffWords> {
        self.buffs.iter().find(|b| b.kind == kind)
    }
}

// ---- amulets ---------------------------------------------------------------------------------

/// An amulet from a cache: one worn at a time, lost at the end of the run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Amulet {
    pub id: String,
    pub name: String,
    pub icon: String,
    /// Tooltip, our own words.
    pub text: String,
    /// Same effect shapes as talents, at rank 1.
    pub effects: Vec<TalentEffect>,
    /// The rarity it is offered at.
    pub rarity: Rarity,
    /// How fast its numbers grow when it is offered again while worn.
    pub growth: Growth,
}

/// An offer's or an amulet's rarity, lowest first (white, green, blue, purple, orange).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    /// Every rarity, lowest first.
    pub const ALL: [Rarity; 5] = [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic, Rarity::Legendary];

    /// Its place, 0 common .. 4 legendary (the wire's and the item quality's order).
    pub fn index(self) -> u8 {
        self as u8
    }
}

/// How fast a worn amulet's numbers grow each time it is offered again (gear.md, "Amulets").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Growth {
    Fastest,
    Fast,
    Medium,
    Slow,
    Slowest,
}

impl Growth {
    /// Its row of the growth rates, 0 fastest .. 4 slowest.
    pub fn index(self) -> usize {
        self as usize
    }
}

/// A legendary amulet: it remakes one spell (or a level-1 strike and its finisher, as one).
/// One legendary per spell, modelled on a boss, enemy or Warcraft III
/// hero who uses a bigger version of it (docs/game/gear.md, "Legendary amulets").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegendaryAmulet {
    /// The spells it remakes, the main one first; the item's id is `legendary:<main spell>`.
    pub spells: Vec<String>,
    /// Our name for it.
    pub name: String,
    pub icon: String,
    /// What it does, in our own words; the client's names in it are pointers (`{Spell(1464)}`),
    /// shown as the content loads.
    pub text: String,
    /// Same effect shapes as talents and amulets, at rank 1.
    pub effects: Vec<TalentEffect>,
    pub rarity: Rarity,
    pub growth: Growth,
}

impl LegendaryAmulet {
    /// The main spell it remakes.
    pub fn main_spell(&self) -> &str {
        self.spells.first().map_or("", String::as_str)
    }

    /// Its item id, `legendary:<main spell>`.
    pub fn id(&self) -> String {
        format!("legendary:{}", self.main_spell())
    }
}
