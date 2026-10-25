//! The assists (docs/game/assists.md): what a hand-made player character
//! (`player_characters.ron`) does when a hero walks over a battle standard and summons it. Each
//! assist is a short script made of blocks: how the ally arrives, its steps in order, and how it
//! leaves. A character without a script of its own takes its class's (and form's) default from
//! `class_assists.ron`.
//!
//! The blocks reuse the content's own pieces: the effects put on enemies are [`SpellEffect`]s and
//! go through the same rules as the hero's own (a boss is held for a quarter, fear never works on
//! a boss), the hero's buffs are [`Stat`]s, the summons are pets from `pets.ron`, and every look
//! is a `Spell.dbc` id as everywhere else.

use serde::{Deserialize, Serialize};

use crate::schema::{PlayerForm, SpellEffect, Stat};

/// An assist: how the ally comes, what it does, and how it goes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistScript {
    #[serde(default)]
    pub arrive: Arrival,
    /// What it does, one step after the other: a step starts once the one before it is done.
    pub steps: Vec<AssistStep>,
    #[serde(default)]
    pub leave: Leaving,
    /// How long the ally stays, seconds (its script, then its rotation); missing: `balance.ron`'s
    /// `battle_standard.assist_seconds` (15). Whatever it left behind (a ground area, a buff, a
    /// totem, an effect over time) runs out on its own time after it has gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f32>,
}

/// How the ally appears.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Arrival {
    #[serde(default)]
    pub how: Appear,
    /// Where: beside the enemy these pick (missing: beside the hero).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beside: Option<Targets>,
    /// Played on the ally as it appears (its visual brings its own sound: Vanish's, Arcane
    /// Power's flash).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_spell: Option<u32>,
    /// A sound of its own (`SoundEntries.dbc`) played as it appears.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<u32>,
}

/// The way the ally appears.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appear {
    /// It is simply there.
    #[default]
    Plain,
    /// It blinks in.
    Blink,
    /// It fades in out of stealth.
    Stealth,
    /// A summoning portal opens and it steps out.
    Portal,
}

/// How the ally leaves (its `death_yell`, if it has one, as it goes).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Leaving {
    #[serde(default)]
    pub how: Leave,
    /// Played on the ally as it goes, in place of the way's own look.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_spell: Option<u32>,
}

/// The way the ally leaves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Leave {
    /// It fades away where it stands.
    #[default]
    Fade,
    /// It vanishes (Vanish's look, `Spell(1856)`).
    Vanish,
    /// It hearthstones away (the Hearthstone's cast, `Spell(8690)`).
    Hearth,
    /// It runs off away from the hero and is gone.
    RunAway,
    /// It dies and its body fades.
    Dies,
}

/// The enemies a step picks. "On screen" is within the zone's enemy spawn ring's inner radius of
/// the hero (where the zone's enemies appear: 22 yd as shipped); every pick looks no farther.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Targets {
    /// The `n` enemies nearest the hero.
    Nearest(u32),
    /// Every enemy within `radius` yd of the hero; with `most`, only that many, the nearest first.
    NearHero {
        radius: f32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        most: u32,
    },
    /// Every enemy within `radius` yd of the ally; with `most`, only that many, the nearest first.
    NearAlly {
        radius: f32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        most: u32,
    },
    /// The strongest enemy: a boss, else a rare, else an elite, else the one with the most max
    /// health.
    Strongest,
    /// The strongest enemy that is not a boss (Polymorph's).
    StrongestNotBoss,
    /// The enemy with the most health left.
    Healthiest,
    /// The enemy farthest from the hero.
    Farthest,
    /// Every enemy on screen.
    OnScreen,
    /// The enemies the step before picked (struck, or moved to), those still alive.
    Same,
}

/// How the ally moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Movement {
    /// At a run.
    #[default]
    Run,
    /// A charge: a fast dash in a straight line (Charge's look, `Spell(100)`, unless `look_spell`
    /// says another).
    Charge,
    /// A blink: it is there at once.
    Blink,
}

/// Where a ground area goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AreaAt {
    /// Under the hero, where the hero stood as it was laid.
    Hero,
    /// Under the ally.
    Ally,
    /// On the thickest crowd on screen (as the hero's own ground areas pick it).
    BiggestPack,
}

/// Damage over time a hit leaves on each enemy it strikes: a tick every `every` seconds for
/// `seconds`, each the assist's damage for `every` seconds (`balance.ron`
/// `battle_standard.assist_damage_per_second`), worn as `look_spell`'s aura.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OverTime {
    #[serde(default = "one")]
    pub every: f32,
    pub seconds: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_spell: Option<u32>,
}

/// One step of an assist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssistStep {
    /// The ally goes to the enemies `to` picks (beside one, the middle of several). It takes the
    /// time the way takes.
    Move {
        to: Targets,
        #[serde(default)]
        how: Movement,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
    },
    /// The ally casts or uses something that is only a look on itself (Sprint, Shield Wall, a
    /// potion, a portal it opens).
    Cast { look_spell: u32 },
    /// The ally strikes the enemies `targets` picks: with `damage`, the assist's damage to each
    /// (`balance.ron` `battle_standard.assist_damage_per_second`; without it none), `look_spell`
    /// played on each (a missile flies from the ally), `effects` put on each through the hero's
    /// own rules (Stun, Root, Fear, Slow, Vulnerable, Weaken), each wearing one of `wears` (one
    /// picked at random for each enemy) while its effects last, and `over_time` on each. With
    /// `melee`, the ally steps to the first one as it strikes.
    Hit {
        targets: Targets,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        damage: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        melee: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        effects: Vec<SpellEffect>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        wears: Vec<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        over_time: Option<OverTime>,
    },
    /// A ground area `radius` yd round at `at` for `seconds`, `look_spell`'s look: every `every`
    /// seconds each enemy inside takes the assist's damage (with `damage`) and gets `effects`; with
    /// `most`, only that many
    /// each time, the nearest its middle first.
    Area {
        at: AreaAt,
        radius: f32,
        seconds: f32,
        #[serde(default = "one")]
        every: f32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        damage: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        effects: Vec<SpellEffect>,
        look_spell: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        most: u32,
    },
    /// A buff on the hero for `seconds`, worn as `look_spell`'s aura: `stats` added, `heal` of
    /// max health every `every` seconds, and with `immune` no damage taken. With `totem`, it comes
    /// from a totem of that spell the ally drops beside the hero, which stands while it lasts.
    Buff {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        stats: Vec<(Stat, f32)>,
        seconds: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        heal: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        every: f32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        immune: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        totem: Option<u32>,
    },
    /// The hero is healed `share` of max health at once, `look_spell` played on them.
    Heal {
        share: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
    },
    /// The enemies `targets` picks attack the ally for `seconds` (a boss for a quarter of it);
    /// the ally stays while it holds (up to its time).
    Taunt {
        targets: Targets,
        seconds: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_spell: Option<u32>,
    },
    /// `count` of `pet` (a name in `pets.ron`) appear beside the ally and fight for the hero
    /// for `seconds`. Their attacks are the assist's damage (no hero bonus, no crit, nothing of
    /// the hero's set off), each for the pet's attack interval.
    Summon {
        pet: String,
        #[serde(default = "one_u32")]
        count: u32,
        seconds: f32,
    },
    /// `steps` run `times` times, one every `every` seconds (the first at once); the step takes
    /// `every × times`.
    Repeat { every: f32, times: u32, steps: Vec<AssistStep> },
    /// The ally waits `seconds`.
    Wait { seconds: f32 },
}

impl AssistStep {
    /// The time the step takes as written (a move's way is not known before it is made: 0).
    pub fn seconds(&self) -> f32 {
        match self {
            AssistStep::Repeat { every, times, .. } => every * *times as f32,
            AssistStep::Wait { seconds } => *seconds,
            _ => 0.0,
        }
    }
}

impl AssistScript {
    /// The time its steps take as written (moves counted as nothing, taunts held as nothing).
    pub fn written_seconds(&self) -> f32 {
        self.steps.iter().map(AssistStep::seconds).sum()
    }

    /// The longest the ally stays: its own `seconds`, else `default`.
    pub fn longest(&self, default: f32) -> f32 {
        self.seconds.unwrap_or(default)
    }
}

impl AssistScript {
    /// The script's damage and healing as one "round" the ally's rotation casts deal again: its
    /// hits' damage and damage over time, its ground areas' damage and its heals, in order, with
    /// every effect, look and wear left out (crowd control, buffs, taunts, summons and casts stay
    /// in the script alone). A repeat's steps come once per pass. A step that picks enemies but
    /// deals nothing (a stun, a move, a taunt) stays as a pick, so a `Same` after it means what it
    /// meant in the script. Empty when the script deals and heals nothing.
    pub fn damage_round(&self) -> Vec<AssistStep> {
        let mut round = Vec::new();
        round_of(&self.steps, &mut round);
        if round.iter().all(|s| matches!(s, AssistStep::Hit { damage: false, over_time: None, .. })) {
            round.clear();
        }
        round
    }
}

fn round_of(steps: &[AssistStep], round: &mut Vec<AssistStep>) {
    let hit = |targets: &Targets, damage: bool, over_time: Option<OverTime>| AssistStep::Hit {
        targets: targets.clone(),
        damage,
        look_spell: None,
        melee: false,
        effects: Vec::new(),
        wears: Vec::new(),
        over_time,
    };
    let pick = |targets: &Targets| hit(targets, false, None);
    for step in steps {
        match step {
            AssistStep::Move { to, .. } => round.push(pick(to)),
            AssistStep::Taunt { targets, .. } => round.push(pick(targets)),
            AssistStep::Hit { targets, damage, over_time, .. } => {
                let over_time = over_time.map(|o| OverTime { look_spell: None, ..o });
                round.push(hit(targets, *damage, over_time));
            }
            AssistStep::Area { at, radius, seconds, every, damage: true, look_spell, most, .. } => round.push(AssistStep::Area {
                at: *at,
                radius: *radius,
                seconds: *seconds,
                every: *every,
                damage: true,
                effects: Vec::new(),
                look_spell: *look_spell,
                most: *most,
            }),
            AssistStep::Heal { share, .. } => round.push(AssistStep::Heal { share: *share, look_spell: None }),
            AssistStep::Repeat { times, steps, .. } => {
                for _ in 0..*times {
                    round_of(steps, round);
                }
            }
            _ => {}
        }
    }
}

/// How an ally moves while it loops its rotation (docs/game/assists.md, "Its rotation").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveStyle {
    /// It runs to the enemy nearest it and stays in melee.
    Melee,
    /// As `Melee`, and in melee it strafes left and right (a rogue's A-D-A-D) and jumps now and
    /// then.
    ErraticMelee,
    /// It stands beside the hero and casts, following when the hero has gone too far.
    Caster,
    /// As `Caster`, and after an instant cast with an enemy close it jumps away (a frost mage's
    /// Frost Nova, a jump back, then Blizzard).
    NovaKiteCaster,
    /// It stays close to the hero, its heals on the hero.
    Healer,
    /// It does nothing after its script: no rotation, no move; it leaves as soon as its script is
    /// done and no taunt of its holds (Vodu dies once his taunt is over; Eggmike hearths away
    /// once her portal is cast).
    LeavesAfterScript,
}

/// What an ally does after its assist, until it leaves: its rotation of 1.12 spells (`Spell.dbc`
/// ids, each a row in `assist_spells.ron`), cast in order, one skipped while its cooldown runs,
/// and how it moves. An empty rotation casts nothing: a `Melee` or `ErraticMelee` ally only
/// auto attacks (white swings at the enemy nearest it, each its own hit), a caster or healer only
/// keeps beside the hero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistBehaviour {
    pub style: MoveStyle,
    pub rotation: Vec<u32>,
}

/// Where a rotation spell is aimed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CastOn {
    /// At an enemy: its round's first, else the one nearest the ally.
    #[default]
    Enemy,
    /// At the hero (a heal, a buff).
    Hero,
    /// At itself (a nova, a shout, a seal, a whirlwind).
    Caster,
    /// On the ground: where its round's area goes, else under the enemy nearest the ally.
    Ground,
}

/// A rotation spell (`assist_spells.ron`): its cast time and cooldown as `Spell.dbc` holds them
/// (seconds; the `check_assist_spells` example checks them against the client), where it is
/// aimed, and for a channelled spell (Blizzard) its channel, its ticks' interval and its radius
/// (seconds, seconds, yards; all 0 for a spell not channelled).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AssistSpell {
    pub spell: u32,
    pub cast: f32,
    pub cooldown: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub channel: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tick_every: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub radius: f32,
    #[serde(default)]
    pub on: CastOn,
}

impl AssistSpell {
    /// It is channelled: its effect comes in ticks while the ally channels it.
    pub fn channelled(&self) -> bool {
        self.channel > 0.0
    }

    /// Its channel's ticks: the channel over the ticks' interval, at least one.
    pub fn ticks(&self) -> u32 {
        if self.tick_every > 0.0 {
            ((self.channel / self.tick_every).round() as u32).max(1)
        } else {
            1
        }
    }
}

/// A class's default assist (`class_assists.ron`), for a character of that class (and form) that
/// has no script of its own: `form` `None` for the class as a whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassAssist {
    pub class: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<PlayerForm>,
    /// What it does, described, for a character whose own `assist` text is empty.
    pub text: String,
    pub script: AssistScript,
    /// Its rotation and movement after the script, for a character with no `behaviour` of its own.
    pub behaviour: AssistBehaviour,
}

/// The rules every behaviour follows: a rotation of spells `assist_spells.ron` has (an empty one
/// is auto attacks only, or nothing), none for one that leaves after its script.
pub(crate) fn check_behaviour(what: &str, b: &AssistBehaviour, spells: &[AssistSpell], problems: &mut Vec<String>) {
    if b.style == MoveStyle::LeavesAfterScript && !b.rotation.is_empty() {
        problems.push(format!("{what}: it leaves after its script, so its rotation casts nothing"));
    }
    for id in &b.rotation {
        if !spells.iter().any(|s| s.spell == *id) {
            problems.push(format!("{what}: its rotation casts {id}, which assist_spells.ron lacks"));
        }
    }
}

/// `assist_spells.ron`'s rules: each spell once, times never below 0, a channel with its ticks and
/// its radius.
pub(crate) fn check_assist_spells(spells: &[AssistSpell], problems: &mut Vec<String>) {
    for (k, s) in spells.iter().enumerate() {
        if spells[..k].iter().any(|o| o.spell == s.spell) {
            problems.push(format!("assist_spells.ron: spell {} is written twice", s.spell));
        }
        if s.spell == 0 || !(s.cast >= 0.0) || !(s.cooldown >= 0.0) || !(s.channel >= 0.0) || !(s.tick_every >= 0.0) || !(s.radius >= 0.0) {
            problems.push(format!("assist_spells.ron: {s:?} needs a spell and times of 0 or more"));
        }
        if s.channelled() && (s.tick_every <= 0.0 || s.radius <= 0.0) {
            problems.push(format!("assist_spells.ron: {s:?} is channelled, so it needs its ticks' interval and its radius"));
        }
    }
}

fn one() -> f32 {
    1.0
}

fn one_u32() -> u32 {
    1
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

/// The rules every assist script follows; each problem is pushed as a line naming `what`.
pub(crate) fn check_script(
    what: &str,
    script: &AssistScript,
    longest: f32,
    pet_known: &dyn Fn(&str) -> bool,
    problems: &mut Vec<String>,
) {
    if script.steps.is_empty() {
        problems.push(format!("{what}: its assist has no steps"));
    }
    if let Some(s) = script.seconds {
        if !(s > 0.0) {
            problems.push(format!("{what}: its assist lasts {s} sec"));
        }
    }
    let written = script.written_seconds();
    if written > script.longest(longest) + 1e-3 {
        problems.push(format!("{what}: its steps take {written} sec, longer than its {} sec", script.longest(longest)));
    }
    if let Some(t) = &script.arrive.beside {
        check_targets(what, t, false, problems);
    }
    let mut picked = script.arrive.beside.is_some();
    check_steps(what, &script.steps, &mut picked, pet_known, problems);
}

fn check_targets(what: &str, t: &Targets, picked: bool, problems: &mut Vec<String>) {
    match *t {
        Targets::Nearest(0) => problems.push(format!("{what}: picks the 0 nearest enemies")),
        Targets::NearHero { radius, .. } | Targets::NearAlly { radius, .. } if !(radius > 0.0) => {
            problems.push(format!("{what}: picks enemies within {radius} yd"))
        }
        Targets::Same if !picked => problems.push(format!("{what}: picks the same enemies before any step picked some")),
        _ => {}
    }
}

/// Only effects on enemies, each with a time.
fn check_effects(what: &str, effects: &[SpellEffect], problems: &mut Vec<String>) {
    for e in effects {
        let seconds = match *e {
            SpellEffect::Slow { seconds, .. }
            | SpellEffect::Root { seconds }
            | SpellEffect::Stun { seconds }
            | SpellEffect::Fear { seconds }
            | SpellEffect::Vulnerable { seconds, .. }
            | SpellEffect::Weaken { seconds, .. } => seconds,
            _ => {
                problems.push(format!("{what}: puts {e:?} on enemies (only effects that last on an enemy: slows, roots, stuns, fears, vulnerability, weakening)"));
                continue;
            }
        };
        if !(seconds > 0.0) {
            problems.push(format!("{what}: {e:?} lasts no time"));
        }
    }
}

fn check_steps(what: &str, steps: &[AssistStep], picked: &mut bool, pet_known: &dyn Fn(&str) -> bool, problems: &mut Vec<String>) {
    for step in steps {
        match step {
            AssistStep::Move { to, .. } => {
                check_targets(what, to, *picked, problems);
                *picked = true;
            }
            AssistStep::Cast { look_spell } => {
                if *look_spell == 0 {
                    problems.push(format!("{what}: casts no look"));
                }
            }
            AssistStep::Hit { targets, damage, effects, wears, over_time, look_spell, .. } => {
                check_targets(what, targets, *picked, problems);
                *picked = true;
                check_effects(what, effects, problems);
                if let Some(o) = over_time {
                    if !(o.every > 0.0 && o.seconds >= o.every) {
                        problems.push(format!("{what}: its effect over time ticks every {} sec for {} sec", o.every, o.seconds));
                    }
                }
                if !wears.is_empty() && effects.is_empty() {
                    problems.push(format!("{what}: its hit puts a look to wear but no effect"));
                }
                if !*damage && effects.is_empty() && over_time.is_none() && look_spell.is_none() {
                    problems.push(format!("{what}: its hit does nothing"));
                }
            }
            AssistStep::Area { radius, seconds, every, damage, effects, look_spell, .. } => {
                if !(*radius > 0.0 && *seconds > 0.0 && *every > 0.0) || *look_spell == 0 {
                    problems.push(format!("{what}: its area needs a radius, a time, a tick and a look"));
                }
                if !*damage && effects.is_empty() {
                    problems.push(format!("{what}: its area does nothing"));
                }
                check_effects(what, effects, problems);
            }
            AssistStep::Buff { stats, seconds, heal, every, immune, .. } => {
                if !(*seconds > 0.0) {
                    problems.push(format!("{what}: its buff lasts no time"));
                }
                if let Some((s, v)) = stats.iter().find(|(_, v)| !(*v > 0.0)) {
                    problems.push(format!("{what}: its buff gives {s:?} {v}: an assist never weakens the hero"));
                }
                if *heal < 0.0 || (*heal > 0.0 && !(*every > 0.0)) {
                    problems.push(format!("{what}: its buff heals {heal} every {every} sec"));
                }
                if stats.is_empty() && *heal == 0.0 && !*immune {
                    problems.push(format!("{what}: its buff does nothing"));
                }
            }
            AssistStep::Heal { share, .. } => {
                if !(*share > 0.0 && *share <= 1.0) {
                    problems.push(format!("{what}: heals {share} of max health"));
                }
            }
            AssistStep::Taunt { targets, seconds, .. } => {
                check_targets(what, targets, *picked, problems);
                *picked = true;
                if !(*seconds > 0.0) {
                    problems.push(format!("{what}: taunts for no time"));
                }
            }
            AssistStep::Summon { pet, count, seconds } => {
                if !pet_known(pet) {
                    problems.push(format!("{what}: summons {pet}, which pets.ron lacks"));
                }
                if *count == 0 || !(*seconds > 0.0) {
                    problems.push(format!("{what}: summons {count} {pet} for {seconds} sec"));
                }
            }
            AssistStep::Repeat { every, times, steps } => {
                if !(*every > 0.0) || *times == 0 || steps.is_empty() {
                    problems.push(format!("{what}: repeats {times} times every {every} sec"));
                }
                check_steps(what, steps, picked, pet_known, problems);
            }
            AssistStep::Wait { seconds } => {
                if !(*seconds > 0.0) {
                    problems.push(format!("{what}: waits no time"));
                }
            }
        }
    }
}
