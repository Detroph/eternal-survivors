//! What the server tells the client's screens. Heroes are named by their player guid, so a client
//! in a run with others knows whose a message is.

use crate::fields::{In, Out};
use crate::to_server::RunOptions;
use crate::Wire;

/// Where the run stands (phases 0 to 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// "Move to begin": nothing happens until a hero moves.
    Waiting,
    Running,
    /// A level-up or cache choice is open; the run waits.
    Choosing,
    Paused,
    Defeat,
    /// The final boss fell; the run waits for Endless or the way out.
    Victory,
}

impl Phase {
    fn word(self) -> &'static str {
        match self {
            Phase::Waiting => "waiting",
            Phase::Running => "running",
            Phase::Choosing => "choosing",
            Phase::Paused => "paused",
            Phase::Defeat => "defeat",
            Phase::Victory => "victory",
        }
    }

    fn from_word(w: &str) -> Option<Self> {
        Some(match w {
            "waiting" => Phase::Waiting,
            "running" => Phase::Running,
            "choosing" => Phase::Choosing,
            "paused" => Phase::Paused,
            "defeat" => Phase::Defeat,
            "victory" => Phase::Victory,
            _ => return None,
        })
    }
}

/// The HUD's numbers for one hero (its level and experience are its player fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hud {
    pub hero: u64,
    pub phase: Phase,
    /// The run went on into Endless.
    pub endless: bool,
    /// The run clock, whole seconds.
    pub clock: u32,
    pub kills: u32,
    pub gold: u32,
    /// Gold the next cache costs.
    pub cache_price: u32,
    /// Whole seconds until the next boss arrives; `None` when none is left to come, or it is due.
    pub next_boss: Option<u32>,
}

/// The hero is in the world and its run is set up ("move to begin"): who it is and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ready {
    pub hero: u64,
    /// The hero's name, and its class's and race's names.
    pub name: String,
    pub class: String,
    pub race: String,
    /// The zone's name and subtitle.
    pub zone: String,
    pub subtitle: String,
}

/// A big banner across the HUD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Banner {
    /// A boss arrives; `final_boss` for the one whose death wins the run.
    BossComes {
        name: String,
        final_boss: bool,
    },
    BossDown {
        name: String,
    },
}

/// A boss's health bar; health 0 takes it away. `phases`: where its script changes something by
/// health (the content's `Health(..)` lines), highest first, each with its label ("Gnoll pack";
/// empty for none).
#[derive(Debug, Clone, PartialEq)]
pub struct BossBar {
    pub boss: u64,
    pub name: String,
    pub health: u32,
    pub max_health: u32,
    pub phases: Vec<BossPhase>,
}

/// A phase mark on a boss's health bar: at `at` % health, labelled `label`.
#[derive(Debug, Clone, PartialEq)]
pub struct BossPhase {
    pub at: f32,
    pub label: String,
}

/// Boss `boss` starts casting (winding up) `label` with the icon `icon` (its file name), done in
/// `seconds`; 0 seconds: its cast stops.
#[derive(Debug, Clone, PartialEq)]
pub struct BossCast {
    pub boss: u64,
    pub label: String,
    pub icon: String,
    pub seconds: f32,
}

/// A raid warning for a boss mechanic: big across the screen in
/// `colour` (linear 0-1 RGB) with the icon `icon` (its file name, `Spell_Nature_Web`; empty for
/// none), the raid warning sound with it; `soon` for an early warning.
#[derive(Debug, Clone, PartialEq)]
pub struct BossWarning {
    pub text: String,
    pub colour: [f32; 3],
    pub icon: String,
    pub soon: bool,
}

/// A boss's timer bar: bar `key` of boss `boss` (its run id), labelled
/// `label` with the icon `icon` (as a warning's), runs out in `seconds`; negative `seconds`: a
/// counter that does not count down ("Guards left 5/8"), kept until told again or cleared.
#[derive(Debug, Clone, PartialEq)]
pub struct BossTimer {
    pub boss: u32,
    pub key: u32,
    pub label: String,
    pub icon: String,
    pub seconds: f32,
}

/// The shape a ground indicator covers, laid from its origin along its facing (as the content's
/// marked attacks: `esurv_content::Shape`), in yards and degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndicatorShape {
    Circle { radius: f32 },
    /// Safe in the middle, inside `inner`.
    Ring { inner: f32, outer: f32 },
    /// `angle` degrees wide (the whole width).
    Cone { length: f32, angle: f32 },
    Line { length: f32, width: f32 },
}

impl IndicatorShape {
    fn write(&self, o: &mut Out) {
        let (word, a, b) = match *self {
            IndicatorShape::Circle { radius } => ("circle", radius, 0.0),
            IndicatorShape::Ring { inner, outer } => ("ring", inner, outer),
            IndicatorShape::Cone { length, angle } => ("cone", length, angle),
            IndicatorShape::Line { length, width } => ("line", length, width),
        };
        o.word(word).num(a).num(b);
    }

    fn read(r: &mut In) -> Option<Self> {
        let word = r.word()?;
        let (a, b): (f32, f32) = (r.num()?, r.num()?);
        Some(match word {
            "circle" => IndicatorShape::Circle { radius: a },
            "ring" => IndicatorShape::Ring { inner: a, outer: b },
            "cone" => IndicatorShape::Cone { length: a, angle: b },
            "line" => IndicatorShape::Line { length: a, width: b },
            _ => return None,
        })
    }
}

/// A boss's mark drawn flat on the ground by the client (Final Fantasy
/// XIV's orange indicators, docs/game/bosses.md, "Building blocks"): `shape` laid at `at` (WoW
/// x, y and the ground's z there) facing `facing` (radians). A mark winding up fills from its
/// origin to its edge over `wind_up` seconds and lands `lands_in` seconds after the line came;
/// an `edge_only` one (a lasting zone, a sweeping beam) is its outline. It moves `velocity` yards
/// a second and turns `turn` radians a second (a drifting zone, a beam), with chevrons the way it
/// goes; `still` while the run stands (its time and motion held). Told again whenever it changes;
/// it goes with [`ToClient::GroundIndicatorGone`].
#[derive(Debug, Clone, PartialEq)]
pub struct GroundIndicator {
    pub id: u32,
    pub shape: IndicatorShape,
    pub at: [f32; 3],
    pub facing: f32,
    pub wind_up: f32,
    pub lands_in: f32,
    pub velocity: [f32; 2],
    pub turn: f32,
    pub edge_only: bool,
    pub still: bool,
}

/// Which choice screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKind {
    LevelUp,
    /// A cache the hero bought.
    Cache,
    /// The cache a slain rare dropped, free to open: the same offers as a cache.
    RareCache,
}

impl ScreenKind {
    fn word(self) -> &'static str {
        match self {
            ScreenKind::LevelUp => "levelup",
            ScreenKind::Cache => "cache",
            ScreenKind::RareCache => "rare",
        }
    }

    fn from_word(w: &str) -> Option<Self> {
        Some(match w {
            "levelup" => ScreenKind::LevelUp,
            "cache" => ScreenKind::Cache,
            "rare" => ScreenKind::RareCache,
            _ => return None,
        })
    }
}

/// What an offer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferKind {
    /// Learn the spell `id`.
    NewSpell,
    /// Improve the known spell `id`.
    UpgradeSpell,
    /// Wear the ring or trinket `id`.
    NewJewelry,
    /// Raise the worn ring or trinket `id`.
    UpgradeJewelry,
    /// Wear the amulet `id` (named `name`) instead of the current one.
    Amulet,
    /// The worn amulet `id` gains a rank.
    Empower,
    Gold,
}

impl OfferKind {
    fn word(self) -> &'static str {
        match self {
            OfferKind::NewSpell => "spell",
            OfferKind::UpgradeSpell => "upgrade",
            OfferKind::NewJewelry => "jewelry",
            OfferKind::UpgradeJewelry => "jewelryup",
            OfferKind::Amulet => "amulet",
            OfferKind::Empower => "empower",
            OfferKind::Gold => "gold",
        }
    }

    fn from_word(w: &str) -> Option<Self> {
        Some(match w {
            "spell" => OfferKind::NewSpell,
            "upgrade" => OfferKind::UpgradeSpell,
            "jewelry" => OfferKind::NewJewelry,
            "jewelryup" => OfferKind::UpgradeJewelry,
            "amulet" => OfferKind::Amulet,
            "empower" => OfferKind::Empower,
            "gold" => OfferKind::Gold,
            _ => return None,
        })
    }
}

/// One number an offer brings: `what` names it (`power`, `cooldown`, `targets`, `area`,
/// `duration`, `value`, `step`, `rank`, `factor`, `gold`).
#[derive(Debug, Clone, PartialEq)]
pub struct OfferValue {
    pub what: String,
    pub value: f32,
}

/// One card on a choice screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    pub kind: OfferKind,
    /// The spell, ring, trinket or amulet id from the content (empty for gold).
    pub id: String,
    /// A name the id alone does not give (a remade legendary amulet's), else empty.
    pub name: String,
    /// 0 common to 4 legendary.
    pub rarity: u8,
    pub values: Vec<OfferValue>,
    /// A new spell's numbers at the rank it starts, for the hero taking it (its card's
    /// description fills them in): `damage`, `targets`, `range` ... (`esurv_content::text::
    /// SPELL_NUMBERS`); empty for other offers. On the line they go after the kit, so a line
    /// without them reads with none.
    pub numbers: Vec<OfferValue>,
}

/// A choice screen opening for `hero`.
#[derive(Debug, Clone, PartialEq)]
pub struct Choices {
    pub hero: u64,
    pub kind: ScreenKind,
    /// The hero's level (a level-up screen's header).
    pub level: u32,
    /// Gold the next reroll costs (0 while free ones are left).
    pub reroll_cost: u32,
    pub free_rerolls: u32,
    pub offers: Vec<Offer>,
    /// What the hero has as the screen opens (the slots a card fills, the screen's side panels).
    pub kit: Kit,
}

/// A known spell, by its content id, and its rank (1 when learned).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownSpell {
    pub id: String,
    pub rank: u32,
}

/// What a spell of a hero's spellbook is, for its tooltip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipKind {
    /// A spell learned in the run.
    Learned,
    /// The race's own spell (its slot of its own; cards never upgrade it).
    Racial,
    /// The race's lasting bonus at this place in the race's list (shown as a stock racial spell).
    Passive(u32),
}

/// One spell of a hero's spellbook as its tooltip shows it: the `Spell.dbc` id it shows as on the
/// action bar and in the spellbook, what it is, and, for a spell, its rank and its numbers for
/// this hero now (as a card names them: `damage`, `cooldown` ...).
#[derive(Debug, Clone, PartialEq)]
pub struct SpellTip {
    pub look: u32,
    pub kind: TipKind,
    /// The spell's content id; a passive's race's id.
    pub id: String,
    /// 1 when learned; 0 for the race's spell and passives, which have none.
    pub rank: u32,
    pub numbers: Vec<OfferValue>,
    /// What the upgrade cards taken for it add up to, by kind (`power`, `cooldown`, `targets`,
    /// `area`, `duration`, as a card's upgrade values: shares, a cooldown's as the share it
    /// shrank by, targets as a count); only the kinds that moved it.
    pub upgrades: Vec<OfferValue>,
    /// What the hero's talents, worn gear, jewelry, amulet and race add to it, the same way.
    pub bonuses: Vec<OfferValue>,
    /// The hero's crit chance and crit damage, for a spell whose hits can crit.
    pub crit: Option<(f32, f32)>,
}

/// A tip's totals, after the whole list (a line from an older server has none).
fn write_totals(o: &mut Out, t: &SpellTip) {
    o.list(&t.upgrades, write_value)
        .list(&t.bonuses, write_value);
    match t.crit {
        Some((chance, damage)) => o.flag(true).num(chance).num(damage),
        None => o.flag(false),
    };
}

type Totals = (Vec<OfferValue>, Vec<OfferValue>, Option<(f32, f32)>);

fn read_totals(r: &mut In) -> Option<Totals> {
    let upgrades = r.list(read_value)?;
    let bonuses = r.list(read_value)?;
    let crit = if r.flag()? {
        Some((r.num()?, r.num()?))
    } else {
        None
    };
    Some((upgrades, bonuses, crit))
}

fn write_tip(o: &mut Out, t: &SpellTip) {
    o.num(t.look);
    match t.kind {
        TipKind::Learned => o.word("learned"),
        TipKind::Racial => o.word("racial"),
        TipKind::Passive(i) => o.word("passive").num(i),
    };
    o.text(&t.id).num(t.rank).list(&t.numbers, write_value);
}

fn read_tip(r: &mut In) -> Option<SpellTip> {
    let look = r.num()?;
    let kind = match r.word()? {
        "learned" => TipKind::Learned,
        "racial" => TipKind::Racial,
        "passive" => TipKind::Passive(r.num()?),
        _ => return None,
    };
    Some(SpellTip {
        look,
        kind,
        id: r.text()?,
        rank: r.num()?,
        numbers: r.list(read_value)?,
        upgrades: Vec::new(),
        bonuses: Vec::new(),
        crit: None,
    })
}

/// What one of a hero's buffs does, as the run's rules have it (`esurv-sim`'s buff kinds).
/// `words.ron` names it by its variant (`kind: Speed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BuffKind {
    /// An absorb shield; its value is what it still absorbs.
    Shield,
    /// More damage; its value is the share.
    Damage,
    /// Faster cooldowns; its value is the share.
    Haste,
    /// Nothing hurts the hero.
    Immunity,
    /// Whatever hits the hero is struck back; its value is the damage.
    Aura,
    /// Health a second; its value is how much.
    Regen,
    /// The Speed Rune: more movement speed; its value is the share.
    Speed,
    /// The Berserker Rune: more damage; its value is the share.
    Berserk,
    /// Healing charges (`bloodthirst`): each weapon swing or spell hit heals; its value is the health
    /// one charge restores, its stacks the charges left.
    HealingCharges,
    /// A starting option's stacking buff (Zeal, Quick Shots, Hot Streak ...); its spell is the
    /// option's passive id, its value how many it can stack.
    Passive,
    /// The hero's strikes, finishers, swings and bleeds also strike more enemies; its value is how
    /// many.
    ExtraStrikes,
}

impl BuffKind {
    pub const ALL: [BuffKind; 11] = [
        BuffKind::Shield,
        BuffKind::Damage,
        BuffKind::Haste,
        BuffKind::Immunity,
        BuffKind::Aura,
        BuffKind::Regen,
        BuffKind::Speed,
        BuffKind::Berserk,
        BuffKind::HealingCharges,
        BuffKind::Passive,
        BuffKind::ExtraStrikes,
    ];

    /// Its word on the wire and in the word table (`words.ron`'s `buffs`).
    pub fn word(self) -> &'static str {
        match self {
            BuffKind::Shield => "shield",
            BuffKind::Damage => "damage",
            BuffKind::Haste => "haste",
            BuffKind::Immunity => "immunity",
            BuffKind::Aura => "aura",
            BuffKind::Regen => "regen",
            BuffKind::Speed => "speed",
            BuffKind::Berserk => "berserk",
            BuffKind::HealingCharges => "charges",
            BuffKind::Passive => "passive",
            BuffKind::ExtraStrikes => "strikes",
        }
    }

    pub fn from_word(w: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.word() == w)
    }
}

/// One of a hero's buffs as the buff frame shows it: the `Spell.dbc` id of the aura it wears (its
/// look, the aura slot's spell), what it does, the spell of ours that cast it (its content id; empty
/// for a rune or a buff no spell of the hero's casts), its value now, and how many stack.
#[derive(Debug, Clone, PartialEq)]
pub struct BuffTip {
    pub look: u32,
    pub kind: BuffKind,
    pub spell: String,
    pub value: f32,
    pub stacks: u32,
}

/// A worn ring, trinket or amulet.
#[derive(Debug, Clone, PartialEq)]
pub struct Worn {
    /// Its content id (a remade legendary amulet's is `legendary:<spell id>`).
    pub id: String,
    /// A name the id alone does not give (a remade legendary amulet's), else empty.
    pub name: String,
    /// 1 when first worn.
    pub rank: u32,
    /// A ring's or trinket's value now (its text's `{v}`); an amulet's empower factor now.
    pub value: f32,
}

/// The hero's spells and worn jewelry when a choice screen opens.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Kit {
    /// In the order learned.
    pub spells: Vec<KnownSpell>,
    /// Rings and trinkets, in the order put on.
    pub jewelry: Vec<Worn>,
    pub amulet: Option<Worn>,
}

/// How a hero's run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// Every life spent.
    Fell,
    /// The final boss fell; Endless or the way out is next.
    Victory,
    /// The player left.
    Left,
}

impl Ending {
    fn word(self) -> &'static str {
        match self {
            Ending::Fell => "fell",
            Ending::Victory => "victory",
            Ending::Left => "left",
        }
    }

    fn from_word(w: &str) -> Option<Self> {
        Some(match w {
            "fell" => Ending::Fell,
            "victory" => Ending::Victory,
            "left" => Ending::Left,
            _ => return None,
        })
    }
}

/// Talent experience a run earned, part by part: time, kills, bosses and the victory make the
/// base, then the zone and run-option bonuses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Experience {
    pub time: u32,
    pub kills: u32,
    pub bosses: u32,
    pub victory: u32,
    pub zone: u32,
    pub options: u32,
}

/// A hero's run as it ended (the end panel): the tally and what it paid out.
#[derive(Debug, Clone, PartialEq)]
pub struct RunEnd {
    pub hero: u64,
    pub ending: Ending,
    /// The final boss fell (also for a run that went on into Endless).
    pub won: bool,
    pub seconds: f32,
    pub level: u32,
    pub kills: u32,
    pub elites: u32,
    pub rares: u32,
    pub bosses: u32,
    pub damage_dealt: u64,
    pub damage_taken: u64,
    pub gold: u32,
    pub caches: u32,
    /// The class gear won, by item entry.
    pub loot: Vec<u32>,
    /// The talent experience this end paid out, part by part: what the run earned since it last
    /// paid (all 0 when it paid nothing).
    pub experience: Experience,
    /// The class's talent experience, and its talent points, before and after.
    pub experience_before: u32,
    pub experience_after: u32,
    pub points_before: u32,
    pub points_after: u32,
    /// Talent experience into the next point, and what that point costs.
    pub into_next: u32,
    pub next_cost: u32,
    /// The zone this end opened to the class, by name.
    pub zone_opened: Option<String>,
}

/// A class gear piece a boss dropped for a hero (the "dropped" pop-up); it is in the collection
/// from now on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GearDrop {
    pub hero: u64,
    pub entry: u32,
    pub slot: u32,
    pub tier: u32,
    pub name: String,
    /// The collection did not have it yet.
    pub new: bool,
}

/// Who or what a meter line counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// The hero's spell (content id), its ticks included.
    Spell(String),
    /// The hero's weapon swing.
    Swing,
    /// A pet, by name.
    Pet(String),
    /// A milestone, thorns, an aura striking back or an extra life's blast, by its `Spell.dbc`
    /// id (0: none), and what gave the hero it.
    Proc { spell: u32, origin: ProcOrigin },
    /// Anything else the hero did.
    Other,
    /// An enemy, by name.
    Enemy(String),
    /// Damage taken with no enemy behind it.
    Area,
    /// An assist (a battle standard's ally), by its character's name: its own hits and its
    /// summoned pets'.
    Assist(String),
}

/// What gave the hero a proc, by content id: the meter names the proc's row after it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProcOrigin {
    Talent(String),
    /// A ring or trinket.
    Jewelry(String),
    Amulet(String),
    /// A legendary amulet (`legendary:<main spell>`).
    Legendary(String),
    /// A weapon loadout.
    Loadout(String),
    /// A spell of the hero's own (an aura striking back).
    Spell(String),
    /// Nothing known (thorns; a line from an older server).
    #[default]
    Other,
}

impl ProcOrigin {
    fn write(&self, o: &mut Out) {
        match self {
            ProcOrigin::Talent(id) => o.word("talent").text(id),
            ProcOrigin::Jewelry(id) => o.word("jewelry").text(id),
            ProcOrigin::Amulet(id) => o.word("amulet").text(id),
            ProcOrigin::Legendary(id) => o.word("legendary").text(id),
            ProcOrigin::Loadout(id) => o.word("loadout").text(id),
            ProcOrigin::Spell(id) => o.word("spell").text(id),
            ProcOrigin::Other => o.word("other"),
        };
    }

    /// An origin of a kind this client does not know reads as `Other` (its id skipped).
    fn read(r: &mut In) -> Option<Self> {
        Some(match r.word()? {
            "talent" => ProcOrigin::Talent(r.text()?),
            "jewelry" => ProcOrigin::Jewelry(r.text()?),
            "amulet" => ProcOrigin::Amulet(r.text()?),
            "legendary" => ProcOrigin::Legendary(r.text()?),
            "loadout" => ProcOrigin::Loadout(r.text()?),
            "spell" => ProcOrigin::Spell(r.text()?),
            "other" => ProcOrigin::Other,
            _ => {
                r.text()?;
                ProcOrigin::Other
            }
        })
    }
}

impl Source {
    fn write(&self, o: &mut Out) {
        match self {
            Source::Spell(id) => o.word("spell").text(id),
            Source::Swing => o.word("swing"),
            Source::Pet(name) => o.word("pet").text(name),
            // the origin goes after both lists (`write_meter_origins`): an older client still
            // reads the line
            Source::Proc { spell, .. } => o.word("proc").num(spell),
            Source::Other => o.word("other"),
            Source::Enemy(name) => o.word("enemy").text(name),
            Source::Area => o.word("area"),
            Source::Assist(name) => o.word("assist").text(name),
        };
    }

    fn read(r: &mut In) -> Option<Self> {
        Some(match r.word()? {
            "spell" => Source::Spell(r.text()?),
            "swing" => Source::Swing,
            "pet" => Source::Pet(r.text()?),
            "proc" => Source::Proc { spell: r.num()?, origin: ProcOrigin::Other },
            "other" => Source::Other,
            "enemy" => Source::Enemy(r.text()?),
            "area" => Source::Area,
            "assist" => Source::Assist(r.text()?),
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeterEntry {
    pub source: Source,
    /// Whole points of damage.
    pub amount: u64,
}

/// The damage meter for one hero: what it (with its pets and procs) did, by source, and what it
/// took, by source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meter {
    pub hero: u64,
    pub done: Vec<MeterEntry>,
    pub taken: Vec<MeterEntry>,
}

/// What the character screen tells of a hero before a run ([`crate::PreRunAsk`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroFacts {
    pub hero: u64,
    /// Runs played and won, its best level and its longest run in seconds.
    pub runs: u32,
    pub wins: u32,
    pub best_level: u32,
    pub best_seconds: u32,
    /// The zone its next run is in (its id in the content).
    pub zone: String,
    /// The starting option its next run starts with (its id in the content): its pick on the
    /// warband screen, else its class's first.
    pub start: String,
}

/// One save in the folder saves are exported to and imported from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveFileFacts {
    /// Its file name in the folder.
    pub name: String,
    /// How many heroes it holds; `None` when it is no save this game can take (it does not read,
    /// or its heroes break the save's rules).
    pub heroes: Option<u32>,
}

/// What a hero's class has earned (the armory): talent experience and points, ranks bought, zones
/// open, gear worn, looks chosen and the class's gear won.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// The class id.
    pub class: String,
    /// All the talent experience earned.
    pub talent_experience: u32,
    /// Points earned, and ranks bought with them.
    pub points: u32,
    pub spent: u32,
    pub into_next: u32,
    pub next_cost: u32,
    /// How many zones are open to the class, from the first in the zones' difficulty order.
    pub zones_open: u32,
    /// The zones the whole save has discovered: every zone any of its
    /// classes has opened, by id, the same in every class's progress. A zone neither in it nor
    /// open to this class shows only "???".
    pub zones_discovered: Vec<String>,
    /// Ranks bought, by talent id.
    pub talents: Vec<(String, u32)>,
    /// Gear worn, by item entry.
    pub worn: Vec<u32>,
    /// The look shown in a slot (slot, look: a place in the class set's looks), where it is not
    /// the worn piece's own.
    pub looks: Vec<(u32, u32)>,
    /// The class's own gear won, by item entry.
    pub collection: Vec<u32>,
    /// Its prestige points and ranks.
    pub prestige: PrestigeProgress,
    /// The weapon loadout it fights with (its id in `loadouts.ron`; the class's first when none
    /// was chosen; empty for a class without loadouts).
    pub loadout: String,
    /// Gear unlocked for the class since the player last opened its Collection, and talent (or
    /// prestige) points earned since it last opened its Talents (the
    /// warband screen's buttons to them glow). An older server says neither.
    pub new_gear: bool,
    pub new_points: bool,
    /// The class has cleared Ahn'Qiraj: the zone select shows the Normal Mode and Challenge Mode tabs.
    /// An older server says no.
    pub challenge_open: bool,
    /// The save has never shown the box telling that Challenge Mode is open, and a class has it open:
    /// the warband screen shows it once (`seen <class> challenge`). The same in every class's progress.
    pub challenge_news: bool,
}

/// A class's prestige: the points its talent experience bought once every talent point was
/// earned, the ranks bought with them, and the experience into the next point.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrestigeProgress {
    pub points: u32,
    pub spent: u32,
    pub into_next: u32,
    /// What the next point costs; 0 before every talent point is earned.
    pub next_cost: u32,
    /// Ranks bought, by prestige stat id.
    pub ranks: Vec<(String, u32)>,
}

/// A hero whose weapons stay in one sheath state all run, whatever its swings, casts and clips
/// would do: Moonkin Form keeps them put away, the shaman's two-hander
/// keeps it in hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SheathHold {
    /// The hero's player guid.
    pub hero: u64,
    /// 0 put away, 1 melee in hand, 2 ranged in hand.
    pub state: u8,
}

/// How a spell's looks are drawn on the client, whoever casts it:
/// grown, tinted, laid flat on the floor, or without their camera shakes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookStyle {
    /// The `Spell.dbc` id whose looks it styles.
    pub spell: u32,
    pub scale: f32,
    /// A colour multiplier (linear 0-1 RGB) on its particles, ribbons and beams.
    pub tint: Option<[f32; 3]>,
    /// With a tint: its particles and ribbons are recoloured rather than multiplied, each fragment
    /// keeping its texture's brightness and taking the tint as its hue (orange fire turns green).
    pub recolor: bool,
    pub flat: bool,
    pub no_shake: bool,
    /// How see-through its ground looks are (their alpha times this; 1.0 as authored).
    pub opacity: f32,
}

/// One message from the server.
#[derive(Debug, Clone, PartialEq)]
pub enum ToClient {
    /// `ESURVW`: after `hello`, who the hero is and where its run is.
    Ready(Ready),
    /// `ESURVH`, when it changes, at most every 0.2 s (a new phase at once).
    Hud(Hud),
    /// `ESURVB`, when a boss's health changes (at most every 0.2 s), and once at 0 when it goes.
    BossBar(BossBar),
    /// `ESURVBK`: a boss starts a cast or a wind-up (or stops one).
    BossCast(BossCast),
    /// `ESURVA`: a boss arrives or falls, a rare falls.
    Banner(Banner),
    /// `ESURVT`: a HUD message (a toast), to every hero.
    Toast(String),
    /// `ESURVC`: a choice screen opens, or its offers are rerolled.
    Choices(Choices),
    /// `ESURVX`: `hero`'s choice screen closes (an offer was taken).
    ChoicesClosed { hero: u64 },
    /// `ESURVG`: a boss dropped a class gear piece.
    GearDropped(GearDrop),
    /// `ESURVE`: the run ended for a hero (it fell, won, or left), with what that paid out.
    RunEnd(RunEnd),
    /// `ESURVM`, when it changed, at most every 0.2 s.
    Meter(Meter),
    /// `ESURVP`: after `hello`, a change in the armory and a pay-out.
    Progress(Progress),
    /// `ESURVO`: the hero's run options for its next run and the bonus they give.
    RunOptions { options: RunOptions, bonus: f32 },
    /// `ESURVN`: a request could not be done (its verb).
    Refused(String),
    /// `ESURVK`: after `hello` and whenever it grows, the codex: every ring, trinket and amulet any
    /// hero of the save has taken in play, by id (a remade legendary amulet is
    /// `legendary:<spell id>`), in the save's order.
    Codex { found: Vec<String> },
    /// `ESURVF`: before a run, a hero's records and the zone its next run is in.
    Hero(HeroFacts),
    /// `ESURVV`: before a run, the folder saves are exported to and imported from (its whole path
    /// on the save's machine), and the saves in it, newest first.
    SaveFiles {
        folder: String,
        files: Vec<SaveFileFacts>,
    },
    /// `ESURVU`: the save was written to this file in that folder.
    SaveExported(String),
    /// `ESURVJ`: the save was replaced by this file's, and the save it replaced kept as `backup`
    /// in the same folder.
    SaveImported { file: String, backup: String },
    /// `ESURVL`: to a hero's own client, when one of them changed (a spell learned or upgraded, a
    /// talent, the amulet, the hero's numbers): every spell of its spellbook as its tooltip shows
    /// it, in the spellbook's order.
    SpellTips { hero: u64, spells: Vec<SpellTip> },
    /// `ESURVD`: to a hero's own client, when its buffs change (one starts, ends or its value
    /// moves), at most every 0.2 s: each buff as its aura shows, in the aura slots' order.
    Buffs { hero: u64, buffs: Vec<BuffTip> },
    /// `ESURVY`: to a hero's own client, when its mana bar's resource or counter changes: its
    /// kind (the content's word, `Rage`, `HolyPower`, `ComboPoints` ...), the name the bar shows
    /// (empty for the client's own word for rage and energy), its value and its most.
    Bar { hero: u64, kind: String, name: String, value: f32, max: f32 },
    /// `ESURVZ`: after `hello`, how some spells' looks are drawn ([`LookStyle`]).
    LookStyles(Vec<LookStyle>),
    /// `ESURVI`: after `hello`, the heroes whose weapons keep one sheath state all run ([`SheathHold`]).
    SheathHolds(Vec<SheathHold>),
    /// `ESURVBW`: a raid warning for a boss mechanic ([`BossWarning`]).
    BossWarning(BossWarning),
    /// `ESURVBT`: a boss's timer bar starts or starts again ([`BossTimer`]).
    BossTimer(BossTimer),
    /// `ESURVBC`: every timer bar of boss `boss` goes (it fell, or changed its kit).
    BossTimersCleared { boss: u32 },
    /// `ESURVGI`: a ground indicator appears or changes ([`GroundIndicator`]).
    GroundIndicator(GroundIndicator),
    /// `ESURVGX`: ground indicator `id` goes (its mark landed, its zone ended).
    GroundIndicatorGone { id: u32 },
    /// `ESURVER`: the ranks of units the client cannot ask the creature query about (the
    /// player-looking enemies, whose guids are players'): each unit's guid and its rank in the
    /// creature query's numbering (1 elite, 3 boss, 4 rare). Told as they appear, and every one
    /// alive after `hello`.
    EnemyRanks(Vec<(u64, u32)>),
}

const READY: &str = "ESURVW";
const HERO: &str = "ESURVF";
const HUD: &str = "ESURVH";
const BOSS_BAR: &str = "ESURVB";
const BANNER: &str = "ESURVA";
const TOAST: &str = "ESURVT";
const CHOICES: &str = "ESURVC";
const CHOICES_CLOSED: &str = "ESURVX";
const GEAR_DROPPED: &str = "ESURVG";
const RUN_END: &str = "ESURVE";
const METER: &str = "ESURVM";
const PROGRESS: &str = "ESURVP";
const RUN_OPTIONS: &str = "ESURVO";
const REFUSED: &str = "ESURVN";
const CODEX: &str = "ESURVK";
const SAVE_FILES: &str = "ESURVV";
const SAVE_EXPORTED: &str = "ESURVU";
const SAVE_IMPORTED: &str = "ESURVJ";
const SPELL_TIPS: &str = "ESURVL";
const BUFFS: &str = "ESURVD";
const BAR: &str = "ESURVY";
const LOOK_STYLES: &str = "ESURVZ";
const SHEATH_HOLDS: &str = "ESURVI";
const BOSS_WARNING: &str = "ESURVBW";
const BOSS_TIMER: &str = "ESURVBT";
const BOSS_TIMERS_CLEARED: &str = "ESURVBC";
const BOSS_CAST: &str = "ESURVBK";
const GROUND_INDICATOR: &str = "ESURVGI";
const GROUND_INDICATOR_GONE: &str = "ESURVGX";
const ENEMY_RANKS: &str = "ESURVER";

fn write_offer(o: &mut Out, offer: &Offer) {
    o.word(offer.kind.word())
        .text(&offer.id)
        .text(&offer.name)
        .num(offer.rarity)
        .list(&offer.values, write_value);
}

fn write_worn(o: &mut Out, w: &Worn) {
    o.text(&w.id).text(&w.name).num(w.rank).num(w.value);
}

fn read_worn(r: &mut In) -> Option<Worn> {
    Some(Worn {
        id: r.text()?,
        name: r.text()?,
        rank: r.num()?,
        value: r.num()?,
    })
}

/// The kit goes after the offers; a line without it (an older server's) reads with an empty one.
fn write_kit(o: &mut Out, kit: &Kit) {
    o.list(&kit.spells, |o, s| {
        o.text(&s.id).num(s.rank);
    });
    o.list(&kit.jewelry, write_worn);
    o.flag(kit.amulet.is_some());
    if let Some(a) = &kit.amulet {
        write_worn(o, a);
    }
}

fn read_kit(r: &mut In) -> Option<Kit> {
    Some(Kit {
        spells: r.list(|r| {
            Some(KnownSpell {
                id: r.text()?,
                rank: r.num()?,
            })
        })?,
        jewelry: r.list(read_worn)?,
        amulet: if r.flag()? { Some(read_worn(r)?) } else { None },
    })
}

fn write_value(o: &mut Out, v: &OfferValue) {
    o.text(&v.what).num(v.value);
}

fn read_value(r: &mut In) -> Option<OfferValue> {
    Some(OfferValue {
        what: r.text()?,
        value: r.num()?,
    })
}

fn read_offer(r: &mut In) -> Option<Offer> {
    Some(Offer {
        kind: OfferKind::from_word(r.word()?)?,
        id: r.text()?,
        name: r.text()?,
        rarity: r.num()?,
        values: r.list(read_value)?,
        numbers: Vec::new(),
    })
}

fn write_meter(o: &mut Out, entries: &[MeterEntry]) {
    o.list(entries, |o, e| {
        e.source.write(o);
        o.num(e.amount);
    });
}

fn read_meter(r: &mut In) -> Option<Vec<MeterEntry>> {
    r.list(|r| {
        Some(MeterEntry {
            source: Source::read(r)?,
            amount: r.num()?,
        })
    })
}

/// The origins of the procs among `entries`, in their order, after both lists (a line from an
/// older server has none: its procs read with no origin).
fn write_meter_origins(o: &mut Out, entries: &[MeterEntry]) {
    let origins: Vec<&ProcOrigin> = entries
        .iter()
        .filter_map(|e| match &e.source {
            Source::Proc { origin, .. } => Some(origin),
            _ => None,
        })
        .collect();
    o.list(&origins, |o, origin| origin.write(o));
}

fn read_meter_origins(r: &mut In, entries: &mut [MeterEntry]) {
    let Some(origins) = r.list(ProcOrigin::read) else { return };
    let procs = entries.iter_mut().filter_map(|e| match &mut e.source {
        Source::Proc { origin, .. } => Some(origin),
        _ => None,
    });
    for (slot, origin) in procs.zip(origins) {
        *slot = origin;
    }
}

impl Wire for ToClient {
    fn prefix(&self) -> &'static str {
        match self {
            ToClient::Ready(_) => READY,
            ToClient::Hud(_) => HUD,
            ToClient::BossBar(_) => BOSS_BAR,
            ToClient::BossCast(_) => BOSS_CAST,
            ToClient::Banner(_) => BANNER,
            ToClient::Toast(_) => TOAST,
            ToClient::Choices(_) => CHOICES,
            ToClient::ChoicesClosed { .. } => CHOICES_CLOSED,
            ToClient::GearDropped(_) => GEAR_DROPPED,
            ToClient::RunEnd(_) => RUN_END,
            ToClient::Meter(_) => METER,
            ToClient::Progress(_) => PROGRESS,
            ToClient::RunOptions { .. } => RUN_OPTIONS,
            ToClient::Refused(_) => REFUSED,
            ToClient::Codex { .. } => CODEX,
            ToClient::Hero(_) => HERO,
            ToClient::SaveFiles { .. } => SAVE_FILES,
            ToClient::SaveExported(_) => SAVE_EXPORTED,
            ToClient::SaveImported { .. } => SAVE_IMPORTED,
            ToClient::SpellTips { .. } => SPELL_TIPS,
            ToClient::Buffs { .. } => BUFFS,
            ToClient::Bar { .. } => BAR,
            ToClient::LookStyles(_) => LOOK_STYLES,
            ToClient::SheathHolds(_) => SHEATH_HOLDS,
            ToClient::BossWarning(_) => BOSS_WARNING,
            ToClient::BossTimer(_) => BOSS_TIMER,
            ToClient::BossTimersCleared { .. } => BOSS_TIMERS_CLEARED,
            ToClient::GroundIndicator(_) => GROUND_INDICATOR,
            ToClient::GroundIndicatorGone { .. } => GROUND_INDICATOR_GONE,
            ToClient::EnemyRanks(_) => ENEMY_RANKS,
        }
    }

    fn message(&self) -> String {
        let mut o = Out::new();
        match self {
            ToClient::Ready(r) => {
                o.guid(r.hero)
                    .text(&r.name)
                    .text(&r.class)
                    .text(&r.race)
                    .text(&r.zone)
                    .text(&r.subtitle);
            }
            ToClient::Hud(h) => {
                o.guid(h.hero)
                    .word(h.phase.word())
                    .flag(h.endless)
                    .num(h.clock)
                    .num(h.kills)
                    .num(h.gold)
                    .num(h.cache_price)
                    .opt(h.next_boss);
            }
            ToClient::BossBar(b) => {
                o.guid(b.boss).num(b.health).num(b.max_health).text(&b.name).list(&b.phases, |o, p| {
                    o.num(p.at).text(&p.label);
                });
            }
            ToClient::BossCast(c) => {
                o.guid(c.boss).text(&c.label).text(&c.icon).num(c.seconds);
            }
            ToClient::Banner(b) => {
                match b {
                    Banner::BossComes { name, final_boss } => {
                        o.word("boss").flag(*final_boss).text(name)
                    }
                    Banner::BossDown { name } => o.word("bossdown").text(name),
                };
            }
            ToClient::Toast(text) => {
                o.text(text);
            }
            ToClient::Choices(c) => {
                o.guid(c.hero)
                    .word(c.kind.word())
                    .num(c.level)
                    .num(c.reroll_cost)
                    .num(c.free_rerolls)
                    .list(&c.offers, write_offer);
                write_kit(&mut o, &c.kit);
                o.list(&c.offers, |o, offer| {
                    o.list(&offer.numbers, write_value);
                });
            }
            ToClient::ChoicesClosed { hero } => {
                o.guid(*hero);
            }
            ToClient::GearDropped(d) => {
                o.guid(d.hero)
                    .num(d.entry)
                    .num(d.slot)
                    .num(d.tier)
                    .flag(d.new)
                    .text(&d.name);
            }
            ToClient::RunEnd(e) => {
                let x = &e.experience;
                o.guid(e.hero)
                    .word(e.ending.word())
                    .flag(e.won)
                    .num(e.seconds)
                    .num(e.level)
                    .num(e.kills)
                    .num(e.elites)
                    .num(e.rares)
                    .num(e.bosses)
                    .num(e.damage_dealt)
                    .num(e.damage_taken)
                    .num(e.gold)
                    .num(e.caches);
                o.num(x.time)
                    .num(x.kills)
                    .num(x.bosses)
                    .num(x.victory)
                    .num(x.zone)
                    .num(x.options);
                o.num(e.experience_before)
                    .num(e.experience_after)
                    .num(e.points_before)
                    .num(e.points_after)
                    .num(e.into_next)
                    .num(e.next_cost);
                match &e.zone_opened {
                    Some(zone) => o.flag(true).text(zone),
                    None => o.flag(false),
                };
                o.list(&e.loot, |o, entry| {
                    o.num(entry);
                });
            }
            ToClient::Meter(m) => {
                o.guid(m.hero);
                write_meter(&mut o, &m.done);
                write_meter(&mut o, &m.taken);
                write_meter_origins(&mut o, &m.done);
            }
            ToClient::Progress(p) => {
                o.text(&p.class)
                    .num(p.talent_experience)
                    .num(p.points)
                    .num(p.spent)
                    .num(p.into_next)
                    .num(p.next_cost)
                    .num(p.zones_open);
                o.list(&p.talents, |o, (id, rank)| {
                    o.text(id).num(rank);
                });
                o.list(&p.worn, |o, e| {
                    o.num(e);
                });
                o.list(&p.looks, |o, (slot, tier)| {
                    o.num(slot).num(tier);
                });
                o.list(&p.collection, |o, e| {
                    o.num(e);
                });
                let q = &p.prestige;
                o.num(q.points).num(q.spent).num(q.into_next).num(q.next_cost);
                o.list(&q.ranks, |o, (id, rank)| {
                    o.text(id).num(rank);
                });
                o.list(&p.zones_discovered, |o, id| {
                    o.text(id);
                });
                o.text(&p.loadout).flag(p.new_gear).flag(p.new_points).flag(p.challenge_open).flag(p.challenge_news);
            }
            ToClient::RunOptions { options, bonus } => {
                options.write(&mut o);
                o.num(bonus);
                options.write_mode(&mut o);
            }
            ToClient::Refused(verb) => {
                o.text(verb);
            }
            ToClient::Codex { found } => {
                o.list(found, |o, id| {
                    o.text(id);
                });
            }
            ToClient::Hero(h) => {
                o.guid(h.hero)
                    .num(h.runs)
                    .num(h.wins)
                    .num(h.best_level)
                    .num(h.best_seconds)
                    .text(&h.zone)
                    .text(&h.start);
            }
            ToClient::SaveFiles { folder, files } => {
                o.text(folder).list(files, |o, f| {
                    o.text(&f.name).opt(f.heroes);
                });
            }
            ToClient::SaveExported(file) => {
                o.text(file);
            }
            ToClient::SaveImported { file, backup } => {
                o.text(file).text(backup);
            }
            ToClient::SpellTips { hero, spells } => {
                o.guid(*hero).list(spells, write_tip);
                // each spell's totals, after the list
                o.list(spells, write_totals);
            }
            ToClient::Buffs { hero, buffs } => {
                o.guid(*hero).list(buffs, |o, b| {
                    o.num(b.look)
                        .word(b.kind.word())
                        .text(&b.spell)
                        .num(b.value)
                        .num(b.stacks);
                });
            }
            ToClient::Bar {
                hero,
                kind,
                name,
                value,
                max,
            } => {
                o.guid(*hero).word(kind).text(name).num(value).num(max);
            }
            ToClient::LookStyles(styles) => {
                o.list(styles, |o, s| {
                    let [r, g, b] = s.tint.unwrap_or([1.0; 3]);
                    o.num(s.spell)
                        .num(s.scale)
                        .flag(s.tint.is_some())
                        .num(r)
                        .num(g)
                        .num(b)
                        .flag(s.flat)
                        .flag(s.no_shake)
                        .flag(s.recolor)
                        .num(s.opacity);
                });
            }
            ToClient::SheathHolds(holds) => {
                o.list(holds, |o, h| {
                    o.guid(h.hero).num(h.state);
                });
            }
            ToClient::BossWarning(w) => {
                let [r, g, b] = w.colour;
                o.text(&w.text).num(r).num(g).num(b).text(&w.icon).flag(w.soon);
            }
            ToClient::BossTimer(t) => {
                o.num(t.boss).num(t.key).text(&t.label).text(&t.icon).num(t.seconds);
            }
            ToClient::BossTimersCleared { boss } => {
                o.num(boss);
            }
            ToClient::GroundIndicator(g) => {
                o.num(g.id);
                g.shape.write(&mut o);
                let [x, y, z] = g.at;
                let [vx, vy] = g.velocity;
                o.num(x).num(y).num(z).num(g.facing).num(g.wind_up).num(g.lands_in).num(vx).num(vy).num(g.turn).flag(g.edge_only).flag(g.still);
            }
            ToClient::GroundIndicatorGone { id } => {
                o.num(id);
            }
            ToClient::EnemyRanks(units) => {
                o.list(units, |o, &(unit, rank)| {
                    o.guid(unit).num(rank);
                });
            }
        }
        o.finish()
    }

    fn decode(prefix: &str, message: &str) -> Option<Self> {
        let r = &mut In::new(message);
        Some(match prefix {
            READY => ToClient::Ready(Ready {
                hero: r.guid()?,
                name: r.text()?,
                class: r.text()?,
                race: r.text()?,
                zone: r.text()?,
                subtitle: r.text()?,
            }),
            HUD => ToClient::Hud(Hud {
                hero: r.guid()?,
                phase: Phase::from_word(r.word()?)?,
                endless: r.flag()?,
                clock: r.num()?,
                kills: r.num()?,
                gold: r.num()?,
                cache_price: r.num()?,
                next_boss: r.opt()?,
            }),
            BOSS_BAR => ToClient::BossBar(BossBar {
                boss: r.guid()?,
                health: r.num()?,
                max_health: r.num()?,
                name: r.text()?,
                phases: r.list(|r| Some(BossPhase { at: r.num()?, label: r.text()? }))?,
            }),
            BOSS_CAST => ToClient::BossCast(BossCast { boss: r.guid()?, label: r.text()?, icon: r.text()?, seconds: r.num()? }),
            BANNER => ToClient::Banner(match r.word()? {
                "boss" => Banner::BossComes {
                    final_boss: r.flag()?,
                    name: r.text()?,
                },
                "bossdown" => Banner::BossDown { name: r.text()? },
                _ => return None,
            }),
            TOAST => ToClient::Toast(r.text()?),
            BOSS_WARNING => ToClient::BossWarning(BossWarning { text: r.text()?, colour: [r.num()?, r.num()?, r.num()?], icon: r.text()?, soon: r.flag()? }),
            BOSS_TIMER => ToClient::BossTimer(BossTimer { boss: r.num()?, key: r.num()?, label: r.text()?, icon: r.text()?, seconds: r.num()? }),
            BOSS_TIMERS_CLEARED => ToClient::BossTimersCleared { boss: r.num()? },
            GROUND_INDICATOR => ToClient::GroundIndicator(GroundIndicator {
                id: r.num()?,
                shape: IndicatorShape::read(r)?,
                at: [r.num()?, r.num()?, r.num()?],
                facing: r.num()?,
                wind_up: r.num()?,
                lands_in: r.num()?,
                velocity: [r.num()?, r.num()?],
                turn: r.num()?,
                edge_only: r.flag()?,
                still: r.flag()?,
            }),
            GROUND_INDICATOR_GONE => ToClient::GroundIndicatorGone { id: r.num()? },
            ENEMY_RANKS => ToClient::EnemyRanks(r.list(|r| Some((r.guid()?, r.num()?)))?),
            CHOICES => {
                let mut choices = Choices {
                    hero: r.guid()?,
                    kind: ScreenKind::from_word(r.word()?)?,
                    level: r.num()?,
                    reroll_cost: r.num()?,
                    free_rerolls: r.num()?,
                    offers: r.list(read_offer)?,
                    kit: read_kit(r).unwrap_or_default(),
                };
                // each offer's numbers, after the kit (none from an older server)
                if let Some(numbers) = r.list(|r| r.list(read_value)) {
                    for (offer, n) in choices.offers.iter_mut().zip(numbers) {
                        offer.numbers = n;
                    }
                }
                ToClient::Choices(choices)
            }
            CHOICES_CLOSED => ToClient::ChoicesClosed { hero: r.guid()? },
            GEAR_DROPPED => ToClient::GearDropped(GearDrop {
                hero: r.guid()?,
                entry: r.num()?,
                slot: r.num()?,
                tier: r.num()?,
                new: r.flag()?,
                name: r.text()?,
            }),
            RUN_END => {
                let hero = r.guid()?;
                let ending = Ending::from_word(r.word()?)?;
                let won = r.flag()?;
                let seconds = r.num()?;
                let level = r.num()?;
                let kills = r.num()?;
                let elites = r.num()?;
                let rares = r.num()?;
                let bosses = r.num()?;
                let damage_dealt = r.num()?;
                let damage_taken = r.num()?;
                let gold = r.num()?;
                let caches = r.num()?;
                let experience = Experience {
                    time: r.num()?,
                    kills: r.num()?,
                    bosses: r.num()?,
                    victory: r.num()?,
                    zone: r.num()?,
                    options: r.num()?,
                };
                let experience_before = r.num()?;
                let experience_after = r.num()?;
                let points_before = r.num()?;
                let points_after = r.num()?;
                let into_next = r.num()?;
                let next_cost = r.num()?;
                let zone_opened = if r.flag()? { Some(r.text()?) } else { None };
                let loot = r.list(|r| r.num())?;
                ToClient::RunEnd(RunEnd {
                    hero,
                    ending,
                    won,
                    seconds,
                    level,
                    kills,
                    elites,
                    rares,
                    bosses,
                    damage_dealt,
                    damage_taken,
                    gold,
                    caches,
                    loot,
                    experience,
                    experience_before,
                    experience_after,
                    points_before,
                    points_after,
                    into_next,
                    next_cost,
                    zone_opened,
                })
            }
            METER => {
                let hero = r.guid()?;
                let mut done = read_meter(r)?;
                let taken = read_meter(r)?;
                read_meter_origins(r, &mut done);
                ToClient::Meter(Meter { hero, done, taken })
            }
            PROGRESS => ToClient::Progress(Progress {
                class: r.text()?,
                talent_experience: r.num()?,
                points: r.num()?,
                spent: r.num()?,
                into_next: r.num()?,
                next_cost: r.num()?,
                zones_open: r.num()?,
                talents: r.list(|r| Some((r.text()?, r.num()?)))?,
                worn: r.list(|r| r.num())?,
                looks: r.list(|r| Some((r.num()?, r.num()?)))?,
                collection: r.list(|r| r.num())?,
                prestige: PrestigeProgress {
                    points: r.num()?,
                    spent: r.num()?,
                    into_next: r.num()?,
                    next_cost: r.num()?,
                    ranks: r.list(|r| Some((r.text()?, r.num()?)))?,
                },
                zones_discovered: r.list(|r| r.text())?,
                // an older server says no loadout
                loadout: r.text().unwrap_or_default(),
                new_gear: r.flag().unwrap_or(false),
                new_points: r.flag().unwrap_or(false),
                // an older server opens no Challenge Mode
                challenge_open: r.flag().unwrap_or(false),
                challenge_news: r.flag().unwrap_or(false),
            }),
            RUN_OPTIONS => {
                let mut options = RunOptions::read(r)?;
                let bonus = r.num()?;
                options.read_mode(r);
                ToClient::RunOptions { options, bonus }
            }
            REFUSED => ToClient::Refused(r.text()?),
            CODEX => ToClient::Codex {
                found: r.list(|r| r.text())?,
            },
            HERO => ToClient::Hero(HeroFacts {
                hero: r.guid()?,
                runs: r.num()?,
                wins: r.num()?,
                best_level: r.num()?,
                best_seconds: r.num()?,
                zone: r.text()?,
                start: r.text()?,
            }),
            SAVE_FILES => ToClient::SaveFiles {
                folder: r.text()?,
                files: r.list(|r| {
                    Some(SaveFileFacts {
                        name: r.text()?,
                        heroes: r.opt()?,
                    })
                })?,
            },
            SAVE_EXPORTED => ToClient::SaveExported(r.text()?),
            SAVE_IMPORTED => ToClient::SaveImported {
                file: r.text()?,
                backup: r.text()?,
            },
            SPELL_TIPS => {
                let hero = r.guid()?;
                let mut spells = r.list(read_tip)?;
                // each spell's totals, after the list (none from an older server)
                if let Some(totals) = r.list(read_totals) {
                    for (t, (upgrades, bonuses, crit)) in spells.iter_mut().zip(totals) {
                        t.upgrades = upgrades;
                        t.bonuses = bonuses;
                        t.crit = crit;
                    }
                }
                ToClient::SpellTips { hero, spells }
            }
            BUFFS => ToClient::Buffs {
                hero: r.guid()?,
                buffs: r.list(|r| {
                    Some(BuffTip {
                        look: r.num()?,
                        kind: BuffKind::from_word(r.word()?)?,
                        spell: r.text()?,
                        value: r.num()?,
                        stacks: r.num()?,
                    })
                })?,
            },
            BAR => ToClient::Bar {
                hero: r.guid()?,
                kind: r.word()?.to_owned(),
                name: r.text()?,
                value: r.num()?,
                max: r.num()?,
            },
            SHEATH_HOLDS => ToClient::SheathHolds(r.list(|r| Some(SheathHold { hero: r.guid()?, state: r.num()? }))?),
            LOOK_STYLES => ToClient::LookStyles(r.list(|r| {
                let spell = r.num()?;
                let scale = r.num()?;
                let tinted = r.flag()?;
                let tint = [r.num()?, r.num()?, r.num()?];
                Some(LookStyle {
                    spell,
                    scale,
                    tint: tinted.then_some(tint),
                    flat: r.flag()?,
                    no_shake: r.flag()?,
                    recolor: r.flag()?,
                    opacity: r.num()?,
                })
            })?),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Reader, MAX_LINE};

    /// Every message goes out as lines within the limit and reads back as it was.
    fn round_trip(m: ToClient) {
        let lines = m.lines();
        assert!(lines.iter().all(|l| l.to_string().len() <= MAX_LINE));
        let mut reader = Reader::<ToClient>::default();
        let mut back = None;
        for l in &lines {
            assert!(back.is_none());
            back = reader.read(&l.to_string());
        }
        assert_eq!(back.as_ref(), Some(&m), "{lines:?}");
    }

    const HERO: u64 = 0x0000_0000_0000_0007;

    #[test]
    fn a_boss_warning_and_its_timer_bars_read_back() {
        round_trip(ToClient::BossWarning(BossWarning { text: "Move out!".into(), colour: [1.0, 0.25, 0.5], icon: "Ability_ThunderClap".into(), soon: false }));
        round_trip(ToClient::BossWarning(BossWarning { text: "Lapdogs soon".into(), colour: [0.75, 0.5, 1.0], icon: String::new(), soon: true }));
        round_trip(ToClient::BossTimer(BossTimer { boss: 42, key: 100, label: "Next slam".into(), icon: "Ability_ThunderClap".into(), seconds: 7.5 }));
        round_trip(ToClient::BossTimersCleared { boss: 42 });
        round_trip(ToClient::BossCast(BossCast { boss: 0xF130_1F40_0000_0123, label: "Head Butt".into(), icon: "Ability_Rogue_Sprint".into(), seconds: 1.8 }));
        round_trip(ToClient::BossCast(BossCast { boss: 7, label: String::new(), icon: String::new(), seconds: 0.0 }));
    }

    /// Every shape of ground indicator reads back whole, a moving one's motion with it, and its
    /// going.
    #[test]
    fn ground_indicators_read_back() {
        let shapes = [
            IndicatorShape::Circle { radius: 8.0 },
            IndicatorShape::Ring { inner: 6.5, outer: 18.0 },
            IndicatorShape::Cone { length: 20.0, angle: 60.0 },
            IndicatorShape::Line { length: 45.0, width: 7.5 },
        ];
        for (k, shape) in shapes.into_iter().enumerate() {
            round_trip(ToClient::GroundIndicator(GroundIndicator {
                id: 4_000_000_000 + k as u32,
                shape,
                at: [-8913.123, -105.456_78, 52.75],
                facing: -2.356_194_5,
                wind_up: 2.5,
                lands_in: 2.499,
                velocity: [0.0, 0.0],
                turn: 0.0,
                edge_only: false,
                still: false,
            }));
        }
        round_trip(ToClient::GroundIndicator(GroundIndicator {
            id: 7,
            shape: IndicatorShape::Circle { radius: 5.0 },
            at: [10.0, 20.0, 30.0],
            facing: 0.5,
            wind_up: 0.0,
            lands_in: 0.0,
            velocity: [1.25, -0.75],
            turn: -0.523_598_8,
            edge_only: true,
            still: true,
        }));
        round_trip(ToClient::GroundIndicatorGone { id: 7 });
        assert_eq!(ToClient::GroundIndicatorGone { id: 7 }.lines()[0].to_string(), "ESURVGX\t7");
    }

    #[test]
    fn enemy_ranks_read_back() {
        let ranks = ToClient::EnemyRanks(vec![(0x2000_0007, 1), (0x2000_0009, 4)]);
        round_trip(ranks.clone());
        assert_eq!(ranks.lines()[0].to_string(), "ESURVER\t2 20000007 1 20000009 4");
        round_trip(ToClient::EnemyRanks(Vec::new()));
    }

    #[test]
    fn ready_and_the_hud_read_back() {
        round_trip(ToClient::Ready(Ready {
            hero: HERO,
            name: "Ana".into(),
            class: "Class 8".into(),
            race: "Race 1".into(),
            zone: "Area 12".into(),
            subtitle: "The Defias Brotherhood".into(),
        }));
        let hud = Hud {
            hero: HERO,
            phase: Phase::Running,
            endless: false,
            clock: 185,
            kills: 412,
            gold: 37,
            cache_price: 31,
            next_boss: Some(175),
        };
        round_trip(ToClient::Hud(hud.clone()));
        for phase in [
            Phase::Waiting,
            Phase::Choosing,
            Phase::Paused,
            Phase::Defeat,
            Phase::Victory,
        ] {
            round_trip(ToClient::Hud(Hud {
                phase,
                endless: true,
                next_boss: None,
                ..hud.clone()
            }));
        }
        assert_eq!(
            ToClient::Hud(Hud {
                phase: Phase::Paused,
                clock: 61,
                kills: 3,
                gold: 9,
                cache_price: 20,
                next_boss: None,
                ..hud
            })
            .lines()[0]
                .to_string(),
            "ESURVH\t7 paused 0 61 3 9 20 ~"
        );
    }

    #[test]
    fn a_boss_bar_a_banner_and_a_toast_read_back() {
        round_trip(ToClient::BossBar(BossBar {
            boss: 0xF130_1F40_0000_0123,
            name: "Morgan the Collector".into(),
            health: 12_345,
            max_health: 20_700,
            phases: vec![BossPhase { at: 50.0, label: "Gnoll pack".into() }, BossPhase { at: 33.5, label: String::new() }],
        }));
        round_trip(ToClient::Banner(Banner::BossComes {
            name: "Hogger".into(),
            final_boss: false,
        }));
        round_trip(ToClient::Banner(Banner::BossComes {
            name: "Edwin VanCleef".into(),
            final_boss: true,
        }));
        round_trip(ToClient::Banner(Banner::BossDown {
            name: "Hogger".into(),
        }));
        round_trip(ToClient::Toast(
            "Magnet! All the loot on the field flies to you.".into(),
        ));
    }

    #[test]
    fn a_choice_screen_reads_back_and_closes() {
        let v = |what: &str, value| OfferValue {
            what: what.into(),
            value,
        };
        round_trip(ToClient::Choices(Choices {
            hero: HERO,
            kind: ScreenKind::LevelUp,
            level: 7,
            reroll_cost: 0,
            free_rerolls: 3,
            offers: vec![
                Offer {
                    kind: OfferKind::NewSpell,
                    id: "frost_nova".into(),
                    name: String::new(),
                    rarity: 2,
                    values: vec![v("cooldown", 0.1)],
                    numbers: vec![v("damage", 28.7), v("radius", 10.0), v("hold", 3.0)],
                },
                Offer {
                    kind: OfferKind::UpgradeSpell,
                    id: "fireball".into(),
                    name: String::new(),
                    rarity: 4,
                    values: vec![v("rank", 3.0), v("power", 0.65), v("targets", 2.0)],
                    numbers: vec![],
                },
                Offer {
                    kind: OfferKind::Gold,
                    id: String::new(),
                    name: String::new(),
                    rarity: 0,
                    values: vec![v("gold", 25.0)],
                    numbers: vec![],
                },
            ],
            kit: Kit {
                spells: vec![
                    KnownSpell {
                        id: "fireball".into(),
                        rank: 2,
                    },
                    KnownSpell {
                        id: "arcane_missiles".into(),
                        rank: 1,
                    },
                ],
                jewelry: vec![],
                amulet: None,
            },
        }));
        round_trip(ToClient::Choices(Choices {
            hero: HERO,
            kind: ScreenKind::Cache,
            level: 12,
            reroll_cost: 300,
            free_rerolls: 0,
            offers: vec![
                Offer {
                    kind: OfferKind::NewJewelry,
                    id: "band_of_might".into(),
                    name: String::new(),
                    rarity: 1,
                    values: vec![v("value", 0.357), v("worn", 1.0)],
                    numbers: vec![],
                },
                Offer {
                    kind: OfferKind::UpgradeJewelry,
                    id: "band_of_might".into(),
                    name: String::new(),
                    rarity: 3,
                    values: vec![v("rank", 2.0), v("step", 0.08), v("value", 0.4)],
                    numbers: vec![],
                },
                Offer {
                    kind: OfferKind::Amulet,
                    id: "legendary:fireball".into(),
                    name: "Heart of the Phoenix: Spell 133".into(),
                    rarity: 4,
                    values: vec![],
                    numbers: vec![],
                },
                Offer {
                    kind: OfferKind::Empower,
                    id: "amulet_of_haste".into(),
                    name: String::new(),
                    rarity: 2,
                    values: vec![v("rank", 3.0), v("factor", 1.6875)],
                    numbers: vec![],
                },
            ],
            kit: Kit {
                spells: vec![KnownSpell {
                    id: "fireball".into(),
                    rank: 4,
                }],
                jewelry: vec![Worn {
                    id: "band_of_might".into(),
                    name: String::new(),
                    rank: 1,
                    value: 0.4,
                }],
                amulet: Some(Worn {
                    id: "legendary:fireball".into(),
                    name: "Heart of the Phoenix: Spell 133".into(),
                    rank: 2,
                    value: 1.2,
                }),
            },
        }));
        round_trip(ToClient::Choices(Choices {
            hero: HERO,
            kind: ScreenKind::RareCache,
            level: 9,
            reroll_cost: 0,
            free_rerolls: 1,
            offers: vec![],
            kit: Kit::default(),
        }));
        round_trip(ToClient::ChoicesClosed { hero: HERO });
    }

    #[test]
    fn a_choice_screen_without_the_kit_reads_with_an_empty_one() {
        let line = "ESURVC\t7 cache 4 0 2 0";
        let mut reader = Reader::<ToClient>::default();
        let Some(ToClient::Choices(c)) = reader.read(line) else {
            panic!("{line} reads");
        };
        assert_eq!((c.kind, c.level, c.free_rerolls), (ScreenKind::Cache, 4, 2));
        assert_eq!(c.kit, Kit::default());
    }

    #[test]
    fn a_drop_and_the_run_end_read_back() {
        round_trip(ToClient::GearDropped(GearDrop {
            hero: HERO,
            entry: 108_310,
            slot: 3,
            tier: 1,
            name: "Belt of Valor".into(),
            new: true,
        }));
        let end = RunEnd {
            hero: HERO,
            ending: Ending::Victory,
            won: true,
            seconds: 987.25,
            level: 31,
            kills: 2412,
            elites: 14,
            rares: 3,
            bosses: 5,
            damage_dealt: 4_812_331,
            damage_taken: 20_115,
            gold: 1460,
            caches: 9,
            loot: (0..40).map(|i| 108_000 + i * 10).collect(),
            experience: Experience {
                time: 197,
                kills: 241,
                bosses: 200,
                victory: 200,
                zone: 0,
                options: 84,
            },
            experience_before: 412,
            experience_after: 1334,
            points_before: 4,
            points_after: 9,
            into_next: 37,
            next_cost: 215,
            zone_opened: Some("Area 10".into()),
        };
        assert!(ToClient::RunEnd(end.clone()).lines().len() > 1, "in pieces");
        round_trip(ToClient::RunEnd(end.clone()));
        round_trip(ToClient::RunEnd(RunEnd {
            ending: Ending::Fell,
            won: false,
            zone_opened: None,
            loot: vec![],
            ..end.clone()
        }));
        round_trip(ToClient::RunEnd(RunEnd {
            ending: Ending::Left,
            ..end
        }));
    }

    #[test]
    fn the_meter_reads_back() {
        let e = |source, amount| MeterEntry { source, amount };
        round_trip(ToClient::Meter(Meter {
            hero: HERO,
            done: vec![
                e(Source::Spell("fireball".into()), 120_400),
                e(Source::Swing, 3_100),
                e(Source::Pet("Water Elemental".into()), 22_000),
                e(Source::Proc { spell: 467, origin: ProcOrigin::Other }, 800),
                e(Source::Proc { spell: 133, origin: ProcOrigin::Talent("ignite".into()) }, 640),
                e(Source::Proc { spell: 0, origin: ProcOrigin::Jewelry("soulstone".into()) }, 90),
                e(Source::Proc { spell: 133, origin: ProcOrigin::Amulet("burning_heart".into()) }, 55),
                e(Source::Proc { spell: 2120, origin: ProcOrigin::Legendary("legendary:fireball".into()) }, 4_000),
                e(Source::Proc { spell: 0, origin: ProcOrigin::Loadout("sword_and_board".into()) }, 30),
                e(Source::Proc { spell: 324, origin: ProcOrigin::Spell("lightning_shield".into()) }, 20),
                e(Source::Other, 12),
                e(Source::Assist("Vodu".into()), 66),
            ],
            taken: vec![
                e(Source::Enemy("Riverpaw Gnoll".into()), 412),
                e(Source::Area, 77),
            ],
        }));
        // a line from an older server, without the origins, reads with its procs of no origin
        let mut reader = Reader::<ToClient>::default();
        let older = reader.read(&format!("{METER}\t7 2 proc 467 800 swing 5 1 area 3"));
        let Some(ToClient::Meter(m)) = older else { panic!("{older:?}") };
        assert_eq!(m.done[0].source, Source::Proc { spell: 467, origin: ProcOrigin::Other });
        assert_eq!(m.taken[0].source, Source::Area);
        round_trip(ToClient::Meter(Meter {
            hero: HERO,
            done: vec![],
            taken: vec![],
        }));
    }

    #[test]
    fn the_armory_and_the_options_read_back() {
        round_trip(ToClient::Progress(Progress {
            class: "mage".into(),
            talent_experience: 1234,
            points: 8,
            spent: 6,
            into_next: 54,
            next_cost: 200,
            zones_open: 3,
            zones_discovered: vec!["elwynn".into(), "duskwood".into(), "wsg".into(), "scarlet".into()],
            talents: vec![("arcane_focus".into(), 3), ("improved_fireball".into(), 5)],
            worn: vec![108_000, 108_110],
            looks: vec![(0, 2), (5, 0)],
            collection: (0..60).map(|i| 108_000 + i * 10).collect(),
            prestige: PrestigeProgress {
                points: 30,
                spent: 28,
                into_next: 400,
                next_cost: 2100,
                ranks: (0..24).map(|i| (format!("prestige_stat_{i}"), i + 1)).collect(),
            },
            loadout: "dagger_off_hand".into(),
            new_gear: true,
            new_points: false,
            challenge_open: true,
            challenge_news: true,
        }));
        round_trip(ToClient::RunOptions {
            options: RunOptions {
                density: 150,
                health: 100,
                damage: 120,
                speed: 105,
                challenge: true,
                challenge_pct: 225,
            },
            bonus: 1.245,
        });
        round_trip(ToClient::Refused("reroll".into()));
        round_trip(ToClient::Hero(HeroFacts {
            hero: 0x12,
            runs: 14,
            wins: 3,
            best_level: 27,
            best_seconds: 1204,
            zone: "duskwood".into(),
            start: "slam".into(),
        }));
    }

    #[test]
    fn the_saves_folder_reads_back() {
        round_trip(ToClient::SaveFiles {
            folder: r"C:\Users\Someone\AppData\Roaming\benilla\saves\exports".into(),
            files: vec![
                SaveFileFacts {
                    name: "heroes-2026-10-13-142530.ron".into(),
                    heroes: Some(4),
                },
                SaveFileFacts {
                    name: "not a save.ron".into(),
                    heroes: None,
                },
            ],
        });
        round_trip(ToClient::SaveFiles {
            folder: String::new(),
            files: vec![],
        });
        round_trip(ToClient::SaveExported(
            "heroes-2026-10-13-142530.ron".into(),
        ));
        round_trip(ToClient::SaveImported {
            file: "heroes-2026-10-13-142530.ron".into(),
            backup: "heroes-before-import-2026-10-13-150000.ron".into(),
        });
    }

    #[test]
    fn the_codex_reads_back_in_pieces_when_long() {
        round_trip(ToClient::Codex { found: vec![] });
        let found: Vec<String> = (0..40)
            .map(|i| format!("ring_{i}"))
            .chain([
                "legendary:fireball".to_owned(),
                "mark_of_fordring".to_owned(),
            ])
            .collect();
        let m = ToClient::Codex { found };
        assert!(m.lines().len() > 1, "a long codex goes in pieces");
        assert!(m.lines().iter().all(|l| l.prefix.starts_with("ESURVK")));
        round_trip(m);
        let short = ToClient::Codex {
            found: vec!["legendary:fireball".into(), "accuria".into()],
        };
        assert_eq!(
            short.lines()[0].to_string(),
            "ESURVK\t2 legendary:fireball accuria"
        );
    }

    #[test]
    fn the_spell_tips_read_back_in_pieces() {
        let v = |what: &str, value| OfferValue {
            what: what.into(),
            value,
        };
        let tip = |look, kind, id: &str, rank, numbers| SpellTip {
            look,
            kind,
            id: id.into(),
            rank,
            numbers,
            upgrades: vec![],
            bonuses: vec![],
            crit: None,
        };
        round_trip(ToClient::SpellTips {
            hero: 0x2a,
            spells: vec![],
        });
        let m = ToClient::SpellTips {
            hero: 0x2a,
            spells: vec![
                tip(
                    23234,
                    TipKind::Racial,
                    "blood_fury",
                    0,
                    vec![v("cooldown", 40.0), v("buff", 0.35), v("buff_range", 30.0)],
                ),
                tip(20573, TipKind::Passive(0), "orc", 0, vec![]),
                SpellTip {
                    upgrades: vec![v("power", 0.08), v("area", 0.18), v("duration", 0.15)],
                    bonuses: vec![v("power", 0.31), v("cooldown", 0.04), v("targets", 1.0)],
                    crit: Some((0.07, 2.04)),
                    ..tip(
                        133,
                        TipKind::Learned,
                        "fireball",
                        3,
                        vec![
                            v("damage", 74.25),
                            v("cooldown", 2.6),
                            v("range", 30.0),
                            v("radius", 4.5),
                        ],
                    )
                },
                tip(
                    116,
                    TipKind::Learned,
                    "frostbolt",
                    1,
                    vec![
                        v("damage", 31.0),
                        v("cooldown", 3.1),
                        v("slow", 0.4),
                        v("slow_for", 3.0),
                    ],
                ),
            ],
        };
        assert!(m.lines().len() > 1, "a hero's spells go in pieces");
        assert!(m.lines().iter().all(|l| l.prefix.starts_with("ESURVL")));
        round_trip(m);
        let one = ToClient::SpellTips {
            hero: 0x2a,
            spells: vec![tip(20573, TipKind::Passive(1), "orc", 0, vec![])],
        };
        assert_eq!(
            one.lines()[0].to_string(),
            "ESURVL\t2A 1 20573 passive 1 orc 0 0 1 0 0 0"
        );
        // a line from an older server, without the totals, reads with none
        let mut reader = Reader::<ToClient>::default();
        let older = reader.read("ESURVL\t2A 1 133 learned fireball 2 1 cooldown 2.5");
        let Some(ToClient::SpellTips { spells, .. }) = older else {
            panic!("{older:?}")
        };
        assert_eq!((spells[0].rank, spells[0].numbers.len()), (2, 1));
        assert!(spells[0].upgrades.is_empty() && spells[0].bonuses.is_empty());
        assert_eq!(spells[0].crit, None);
    }

    #[test]
    fn the_buffs_read_back() {
        round_trip(ToClient::Buffs {
            hero: 0x2a,
            buffs: vec![],
        });
        let buff = |look, kind, spell: &str, value, stacks| BuffTip {
            look,
            kind,
            spell: spell.into(),
            value,
            stacks,
        };
        let m = ToClient::Buffs {
            hero: 0x2a,
            buffs: vec![
                buff(23451, BuffKind::Speed, "", 0.6, 1),
                buff(23234, BuffKind::Damage, "blood_fury", 0.35, 1),
                buff(17, BuffKind::Shield, "power_word_shield", 54.5, 2),
            ],
        };
        round_trip(m);
        let one = ToClient::Buffs {
            hero: 0x2a,
            buffs: vec![buff(23451, BuffKind::Speed, "", 0.6, 1)],
        };
        assert_eq!(
            one.lines()[0].to_string(),
            "ESURVD\t2A 1 23451 speed ~ 0.6 1"
        );
        for k in BuffKind::ALL {
            assert_eq!(BuffKind::from_word(k.word()), Some(k));
        }
    }

    #[test]
    fn the_bar_and_the_look_styles_read_back() {
        round_trip(ToClient::Bar {
            hero: 0x2a,
            kind: "HolyPower".into(),
            name: "Holy Power".into(),
            value: 3.0,
            max: 5.0,
        });
        round_trip(ToClient::Bar {
            hero: 0x2a,
            kind: "Rage".into(),
            name: String::new(),
            value: 47.5,
            max: 100.0,
        });
        round_trip(ToClient::LookStyles(vec![
            LookStyle { spell: 12051, scale: 1.0, tint: Some([1.0, 0.25, 0.2]), recolor: false, flat: false, no_shake: false, opacity: 1.0 },
            LookStyle { spell: 604, scale: 1.0, tint: None, recolor: false, flat: true, no_shake: false, opacity: 1.0 },
            LookStyle { spell: 26093, scale: 0.5, tint: None, recolor: false, flat: false, no_shake: true, opacity: 1.0 },
            LookStyle { spell: 13241, scale: 1.0, tint: Some([0.3, 1.0, 0.2]), recolor: true, flat: false, no_shake: false, opacity: 0.5 },
        ]));
        round_trip(ToClient::SheathHolds(vec![SheathHold { hero: 0x2a, state: 0 }, SheathHold { hero: 0x2b, state: 1 }]));
        round_trip(ToClient::SheathHolds(Vec::new()));
    }

    #[test]
    fn lines_of_other_kinds_are_not_messages() {
        let mut reader = Reader::<ToClient>::default();
        assert_eq!(reader.read("ESURV\thealth=230;damage=1.000"), None);
        assert_eq!(reader.read("ESURVS\tF130251C00000007 8187 1.250"), None);
        assert_eq!(reader.read("Quiver\tVERSION:3.1.4"), None);
        assert_eq!(reader.read("ESURVH\tnonsense"), None);
    }
}
