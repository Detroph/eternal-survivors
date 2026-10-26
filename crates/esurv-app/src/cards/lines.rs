//! The lines a card shows, in pieces of their own colour (screens.md,
//! "What a card says"): a number a card brings reads green, its words white, as "+34% damage"
//! does. Built from the content's templates (a spell's description, a ring's line, the words
//! table) and the effects of amulets, whose sentences are templates of their own here.

use esurv_content::text::{fill, Filled, NumberKind, Shown};
use esurv_content::{
    Ability, AfflictEffect, Milestone, SpellEffect, SpellFilter, TalentEffect, Trigger, Upgrade, Words,
};
use esurv_sim::game::spell_numbers::power_gain;

/// How a piece of a line reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    /// Words (white).
    Plain,
    /// A number the card brings (green, as an item's `Equip:` lines).
    Gain,
    /// How things stand now (grey).
    Now,
}

/// One line of a card, in pieces.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Line(pub Vec<(String, Tone)>);

impl Line {
    /// A line all in one tone.
    pub fn one(text: impl Into<String>, tone: Tone) -> Self {
        Self(vec![(text.into(), tone)])
    }

    /// The line as one text.
    pub fn text(&self) -> String {
        self.0.iter().map(|(t, _)| t.as_str()).collect()
    }

    /// The tone a place with one colour per line (a tooltip) shows it in: green where it brings
    /// a number, else its first piece's.
    pub fn tone(&self) -> Tone {
        if self.0.iter().any(|(_, t)| *t == Tone::Gain) {
            Tone::Gain
        } else {
            self.0.first().map_or(Tone::Plain, |(_, t)| *t)
        }
    }

    /// Whether it opens with its number ("+12% movement speed"), as a stat line does, rather
    /// than reading as a sentence.
    pub fn opens_with_number(&self) -> bool {
        self.0.first().is_some_and(|(_, t)| *t == Tone::Gain)
    }

    /// Adds `text` in `tone`, joined to the last piece when that has the same tone.
    fn push(&mut self, text: &str, tone: Tone) {
        if text.is_empty() {
            return;
        }
        match self.0.last_mut() {
            Some((last, t)) if *t == tone => last.push_str(text),
            _ => self.0.push((text.to_owned(), tone)),
        }
    }
}

/// A filled template as a line: its words plain, its numbers in `numbers`.
pub(crate) fn filled_line(filled: &[Filled], numbers: Tone) -> Line {
    let mut line = Line::default();
    for f in filled {
        match f {
            Filled::Words(w) => line.push(w, Tone::Plain),
            Filled::Value(v) => line.push(v, numbers),
        }
    }
    line
}

/// A share as a percentage, to a tenth at most: 0.46 reads "46%", 0.065 "6.5%".
pub(crate) fn percent(share: f32) -> String {
    NumberKind::Share.show(share)
}

/// A number to a tenth at most, with no trailing zero.
pub(crate) fn number(value: f32) -> String {
    esurv_content::text::tenths(value)
}

/// `{v}` of a ring's, trinket's or stat's line as it reads: a share where a `%` follows it, else
/// the number as it is.
fn shown_value(value: f32, share: bool) -> String {
    if share {
        percent(value)
    } else {
        number(value)
    }
}

/// A line with one `{v}` (a ring's or trinket's `text`, a stat's line) and its value `to`; with
/// `from`, the value it had, as "+70% » +114%". The sign before `{v}` and the `%` after it read
/// green with the number; the rest is words.
pub(crate) fn value_line(text: &str, from: Option<f32>, to: f32) -> Line {
    let Some((before, after)) = text.split_once("{v}") else {
        return Line::one(text, Tone::Plain);
    };
    let share = after.starts_with('%');
    let after = if share { &after[1..] } else { after };
    let (words_before, sign) = match before.chars().last() {
        Some(c @ ('+' | '-')) => (&before[..before.len() - 1], c.to_string()),
        _ => (before, String::new()),
    };
    let mut line = Line::default();
    line.push(words_before, Tone::Plain);
    if let Some(from) = from {
        line.push(&format!("{sign}{}", shown_value(from, share)), Tone::Gain);
        line.push(" \u{bb} ", Tone::Plain);
    }
    line.push(&format!("{sign}{}", shown_value(to, share)), Tone::Gain);
    line.push(after, Tone::Plain);
    line
}


/// A ring's or trinket's line at value `to` (with `from`, the value it had): its `{v}`, and its
/// weapon double hit as `{w}` where it has one (Hand of Justice).
pub(crate) fn jewelry_line(j: &esurv_content::Jewelry, from: Option<f32>, to: f32) -> Line {
    let Some((first, second)) = j.text.split_once("{w}") else {
        return value_line(&j.text, from, to);
    };
    let mut line = value_line(first, from, to);
    let rest = value_line(&format!("{{v}}{second}"), from.map(|f| j.double_hit_at(f)), j.double_hit_at(to));
    for (text, tone) in &rest.0 {
        line.push(text, *tone);
    }
    line
}

/// What `Power` raises on `spell`: its damage, else what it heals or shields for.
fn power_words<'w>(words: &'w Words, spell: Option<&Ability>) -> &'w str {
    let on_self = spell.is_some_and(|a| a.delivery == esurv_content::Delivery::SelfEffect);
    match spell.map(|a| a.effect) {
        Some(SpellEffect::Heal { .. }) if on_self => &words.power_of_heals,
        Some(SpellEffect::Shield { .. }) if on_self => &words.power_of_shields,
        _ => words.upgrade(Upgrade::Power, 2.0).unwrap_or("damage"),
    }
}

/// An upgrade's gain as a line: "+34% damage", "-7% cooldown", "+1 target"; `by` is the gain
/// itself (a share; a count of targets).
pub(crate) fn upgrade_line(words: &Words, kind: Upgrade, by: f32, spell: Option<&Ability>) -> Line {
    let value = match kind {
        Upgrade::Cooldown => format!("-{}", percent(by)),
        k if k.is_count() => format!("+{}", NumberKind::Count.show(by)),
        _ => format!("+{}", percent(by)),
    };
    let what = match kind {
        Upgrade::Power => power_words(words, spell),
        _ => words.upgrade(kind, by.round()).unwrap_or_default(),
    };
    Line(vec![(value, Tone::Gain), (format!(" {what}"), Tone::Plain)])
}

/// The upgrade an offer's value names (`power`, `cooldown` ...), if it is one.
pub(crate) fn upgrade_named(what: &str) -> Option<Upgrade> {
    Upgrade::named(what)
}

/// A sentence of an amulet's effect: `template` filled from `numbers`, its numbers green.
fn sentence(template: &str, numbers: &[(&str, NumberKind, f32)]) -> Option<Line> {
    let filled = fill(template, |name| {
        numbers
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, kind, v)| Shown::number(*kind, *v))
    })
    .ok()?;
    Some(filled_line(&filled, Tone::Gain))
}

use NumberKind::{Count, Seconds, Share, Yards};

/// `n` as an ordinal: 2nd, 3rd, 4th, 11th, 21st.
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// What sets an effect off, as a sentence opens.
fn when(trigger: &Trigger) -> &'static str {
    match trigger {
        Trigger::Crit => "Every critical strike",
        Trigger::DotTick => "Every tick",
        Trigger::HitOrTick => "Every hit or tick",
        Trigger::Kill => "Every kill",
        Trigger::Cast => "Every cast",
        _ => "Every hit",
    }
}

/// Whose doing an effect follows: the remade spell's (one spell), else the hero's.
fn whose(filter: Option<&SpellFilter>) -> &'static str {
    match filter {
        Some(SpellFilter::Spell(_)) => "it",
        _ => "you",
    }
}

/// One effect of an amulet (a remade legendary's too) at its strength, as lines: a stat's line,
/// an upgrade's gain on `spell` (the spell a legendary remakes) or on every spell, a rule as a
/// sentence. `None` for an effect no amulet has.
pub(crate) fn effect_line(
    words: &Words,
    effect: &TalentEffect,
    spell: Option<&Ability>,
) -> Option<Line> {
    match effect {
        TalentEffect::Stat { stat, per_rank } => {
            Some(value_line(words.stat(*stat)?, None, *per_rank))
        }
        TalentEffect::AbilityMod {
            kind,
            per_rank,
            filter,
        } => {
            let by = match kind {
                Upgrade::Power => spell.map_or(*per_rank, |a| power_gain(a, *per_rank)),
                _ => *per_rank,
            };
            let mut line = upgrade_line(words, *kind, by, spell);
            if *filter == SpellFilter::Any {
                line.push(&format!(" {}", words.every_spell), Tone::Plain);
            }
            Some(line)
        }
        TalentEffect::Milestone(m) => milestone_line(m),
        // a talent's only, never an amulet's
        TalentEffect::CooldownSeconds { .. } => None,
    }
}

/// A rule an effect brings, as a sentence in our words with its numbers green.
fn milestone_line(m: &Milestone) -> Option<Line> {
    match m {
        Milestone::ExtraTargets {
            count,
            share,
            trigger,
            ..
        } => sentence(
            &format!(
                "{} also strikes {{count:# more enemy|# more enemies}} near the target for {{share}} of its damage",
                when(trigger)
            ),
            &[("count", Count, *count as f32), ("share", Share, *share)],
        ),
        Milestone::StrikeAgain {
            chance,
            times,
            share,
            ..
        } => {
            let strength = if *share >= 1.0 {
                "at full strength"
            } else {
                "for {share} of its damage"
            };
            sentence(
                &format!("{{chance}} chance for a hit to strike {{times:once|# times}} more {strength}"),
                &[
                    ("chance", Share, *chance),
                    ("times", Count, *times as f32),
                    ("share", Share, *share),
                ],
            )
        }
        Milestone::Afflict {
            chance,
            effect,
            trigger,
            ..
        } => {
            let (what, numbers): (&str, Vec<(&str, NumberKind, f32)>) = match effect {
                AfflictEffect::Stun { seconds } => ("stunned for {seconds} sec", vec![("seconds", Seconds, *seconds)]),
                AfflictEffect::Root { seconds } => ("rooted for {seconds} sec", vec![("seconds", Seconds, *seconds)]),
                AfflictEffect::Fear { seconds } => ("feared for {seconds} sec", vec![("seconds", Seconds, *seconds)]),
                AfflictEffect::Silence { seconds } => ("silenced for {seconds} sec", vec![("seconds", Seconds, *seconds)]),
                AfflictEffect::Knockback { yards } => ("knocked back {yards} yd", vec![("yards", Yards, *yards)]),
                AfflictEffect::Slow { amount, seconds } => (
                    "slowed by {amount} for {seconds} sec",
                    vec![("amount", Share, *amount), ("seconds", Seconds, *seconds)],
                ),
                AfflictEffect::Vulnerable { amount, seconds } => (
                    "left taking {amount} more damage for {seconds} sec",
                    vec![("amount", Share, *amount), ("seconds", Seconds, *seconds)],
                ),
                AfflictEffect::HealFromDamage { share } => {
                    let n = [("share", Share, *share), ("chance", Share, *chance)];
                    let template = if *chance >= 1.0 {
                        "Its hits heal you for {share} of their damage"
                    } else {
                        "Its hits have a {chance} chance to heal you for {share} of their damage"
                    };
                    return sentence(template, &n);
                }
                AfflictEffect::HealMaxHealth { share } => {
                    let template = if *chance >= 1.0 {
                        "Its hits heal you for {share} of your max health"
                    } else {
                        "Its hits have a {chance} chance to heal you for {share} of your max health"
                    };
                    return sentence(template, &[("share", Share, *share), ("chance", Share, *chance)]);
                }
            };
            let template = match (trigger, *chance >= 1.0) {
                (Trigger::StruckInMelee, _) => format!("Whatever strikes you in melee has a {{chance}} chance to be {what}"),
                (_, true) => format!("Whatever it hits is {what}"),
                (_, false) => format!("Whatever it hits has a {{chance}} chance to be {what}"),
            };
            let mut all = numbers;
            all.push(("chance", Share, *chance));
            sentence(&template, &all)
        }
        Milestone::DotOnHit {
            share,
            seconds,
            trigger,
            ..
        } => sentence(
            &format!(
                "{} also burns the target for {{share}} of its damage over {{seconds}} sec",
                when(trigger)
            ),
            &[("share", Share, *share), ("seconds", Seconds, *seconds)],
        ),
        Milestone::ExplodeOnDeath {
            share,
            radius,
            filter,
            ..
        } => sentence(
            &format!(
                "Enemies {} kill burst for {{share}} of their max health on every enemy within {{radius}} yd",
                whose(Some(filter))
            ),
            &[("share", Share, *share), ("radius", Yards, *radius)],
        ),
        Milestone::ChainNoFalloff => Some(Line::one(
            "Its jumps keep their full strength",
            Tone::Plain,
        )),
        Milestone::SpreadOnDeath { count, .. } => sentence(
            "When an enemy carrying it dies, it passes to {count:the nearest enemy|the # nearest enemies}",
            &[("count", Count, *count as f32)],
        ),
        Milestone::ResetOnKill { min_interval, .. } => sentence(
            "A killing blow readies it at once, at most every {seconds} sec",
            &[("seconds", Seconds, *min_interval)],
        ),
        Milestone::Splash {
            share,
            radius,
            trigger,
            ..
        } => sentence(
            &format!(
                "{} also splashes {{share}} of its damage on every enemy within {{radius}} yd of the target",
                when(trigger)
            ),
            &[("share", Share, *share), ("radius", Yards, *radius)],
        ),
        Milestone::ShieldBurst { share, radius, .. } => sentence(
            "A shield of yours that breaks bursts for {share} of what it absorbed on every enemy within {radius} yd",
            &[("share", Share, *share), ("radius", Yards, *radius)],
        ),
        Milestone::TotemHeal { share } => sentence(
            "Every pulse of your totems heals you for {share} of your max health",
            &[("share", Share, *share)],
        ),
        Milestone::CheatDeath {
            cooldown,
            health,
            immune_seconds,
        } => {
            let then = if *immune_seconds > 0.0 {
                " and nothing harms you for {immune} sec"
            } else {
                ""
            };
            sentence(
                &format!("Survive a killing blow with {{health}} of your health left{then}, no more than once every {{cooldown}} sec"),
                &[
                    ("health", Share, *health),
                    ("cooldown", Seconds, *cooldown),
                    ("immune", Seconds, *immune_seconds),
                ],
            )
        }
        Milestone::LineHit {
            every,
            share,
            length,
            behind,
            ..
        } => {
            let which = if *every > 1 { format!("Every {} hit", ordinal(*every)) } else { "Every hit".to_string() };
            let line = if *behind { "behind the target" } else { "through the target" };
            sentence(
                &format!("{which} also pierces every enemy in a line {line}, {{length}} yd long, for {{share}} of its damage"),
                &[
                    ("every", Count, *every as f32),
                    ("share", Share, *share),
                    ("length", Yards, *length),
                ],
            )
        }
        Milestone::MarkAttackers {
            seconds,
            bonus,
            heal,
            ..
        } => sentence(
            "Enemies that strike you are marked for {seconds} sec: it deals {bonus} more to them, and each of its hits on them heals you for {heal} of your max health",
            &[
                ("seconds", Seconds, *seconds),
                ("bonus", Share, *bonus),
                ("heal", Share, *heal),
            ],
        ),
        Milestone::GrowingArea { per_hit, max, .. } => sentence(
            "Its area grows {per_hit} each time it damages an enemy, up to {max} wider",
            &[("per_hit", Share, *per_hit), ("max", Share, *max)],
        ),
        Milestone::AreaFollowsHero { .. } => Some(Line::one(
            "It falls around you and moves with you",
            Tone::Plain,
        )),
        Milestone::PullEveryNth { count, radius, .. } => sentence(
            &format!("Every {} cast first pulls the enemies within {{radius}} yd to you", ordinal(*count)),
            &[("count", Count, *count as f32), ("radius", Yards, *radius)],
        ),
        Milestone::FireTrail { share, seconds, .. } => sentence(
            "Enemies burning from it leave a trail of fire that burns the others for {share} of the burn a second, for {seconds} sec",
            &[("share", Share, *share), ("seconds", Seconds, *seconds)],
        ),
        Milestone::FearedTurn { chance, .. } => sentence(
            "{chance} of the enemies it fears turn on the others while the fear lasts",
            &[("chance", Share, *chance)],
        ),
        Milestone::PetOnTrigger {
            chance,
            seconds,
            max,
            pet,
            trigger,
            count,
            ..
        } => {
            let odds = if *chance >= 1.0 { "" } else { " has a {chance} chance to" };
            let calls = if *chance >= 1.0 { "calls" } else { "call" };
            sentence(
                &format!("{}{odds} {calls} {{count:a {pet}|# {pet}s}} that fight for {{seconds}} sec, up to {{max}} at once", when(trigger)),
                &[
                    ("chance", Share, *chance),
                    ("count", Count, *count as f32),
                    ("seconds", Seconds, *seconds),
                    ("max", Count, *max as f32),
                ],
            )
        }
        Milestone::DotFromHeal { share, seconds, targets, order, .. } => {
            let whom = match order {
                Some(_) => "{targets:an enemy|# enemies}, bosses and elites first,",
                None => "{targets:the nearest enemy|the # nearest enemies}",
            };
            sentence(
                &format!("Each cast also burns {whom} for {{share}} of its healing over {{seconds}} sec"),
                &[("targets", Count, *targets as f32), ("share", Share, *share), ("seconds", Seconds, *seconds)],
            )
        }
        Milestone::CastOnCooldown { .. } => Some(Line::one("It is cast whenever it is ready and an enemy is near", Tone::Plain)),
        _ => None,
    }
}

/// An amulet's effects as its card lists them: its rules as sentences first, then its stat lines
///.
pub(crate) fn effect_lines(
    words: &Words,
    effects: &[TalentEffect],
    spell: Option<&Ability>,
) -> Vec<Line> {
    let lines: Vec<Line> = effects
        .iter()
        .filter_map(|e| effect_line(words, e, spell))
        .collect();
    let (stats, sentences): (Vec<Line>, Vec<Line>) =
        lines.into_iter().partition(Line::opens_with_number);
    sentences.into_iter().chain(stats).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words() -> Words {
        esurv_content::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content"))
            .unwrap()
            .words
    }

    /// Hand of Justice's line shows its double cast and its weapon
    /// double hit, both as an upgrade moves them.
    #[test]
    fn hand_of_justices_line_shows_both_its_chances() {
        let content = esurv_content::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let hoj = content.jewelry.iter().find(|j| j.id == "hoj").unwrap();
        assert_eq!(
            jewelry_line(hoj, None, 0.075).text(),
            "7.5% chance for a spell to cast twice, 2% chance for a weapon attack to hit twice"
        );
        assert_eq!(
            jewelry_line(hoj, Some(0.075), 0.175).text(),
            "7.5% » 17.5% chance for a spell to cast twice, 2% » 4% chance for a weapon attack to hit twice"
        );
    }

    #[test]
    fn a_value_line_reads_its_number_green_and_its_words_plain() {
        let line = value_line("+{v}% critical strike damage", None, 0.7);
        assert_eq!(
            line.0,
            vec![
                ("+70%".to_owned(), Tone::Gain),
                (" critical strike damage".to_owned(), Tone::Plain)
            ]
        );
        let up = value_line("+{v}% critical strike damage", Some(0.7), 1.14);
        assert_eq!(up.text(), "+70% \u{bb} +114% critical strike damage");
        assert_eq!(up.0[1], (" \u{bb} ".to_owned(), Tone::Plain));
        let stacks = value_line(
            "Your damage over time spells stack {v} more times on a target",
            Some(1.0),
            2.0,
        );
        assert_eq!(
            stacks.text(),
            "Your damage over time spells stack 1 \u{bb} 2 more times on a target"
        );
        assert!(!stacks.opens_with_number());
    }

    #[test]
    fn upgrade_lines_name_what_they_raise() {
        let w = words();
        assert_eq!(
            upgrade_line(&w, Upgrade::Power, 0.34, None).text(),
            "+34% damage"
        );
        assert_eq!(
            upgrade_line(&w, Upgrade::Cooldown, 0.07, None).text(),
            "-7% cooldown"
        );
        assert_eq!(
            upgrade_line(&w, Upgrade::Targets, 1.0, None).text(),
            "+1 target"
        );
        assert_eq!(
            upgrade_line(&w, Upgrade::Targets, 2.0, None).text(),
            "+2 targets"
        );
        assert_eq!(
            upgrade_line(&w, Upgrade::Chance, 0.033, None).text(),
            "+3.3% proc chance"
        );
        assert_eq!(
            upgrade_line(&w, Upgrade::Stacks, 1.0, None).text(),
            "+1 stack"
        );
        let c = esurv_content::Content::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../content"
        ))
        .unwrap();
        let heal = c.ability("holy_light");
        assert_eq!(
            upgrade_line(&w, Upgrade::Power, 0.2, heal).text(),
            "+20% healing"
        );
    }

    #[test]
    fn an_amulet_lists_its_rules_before_its_stats() {
        let w = words();
        let c = esurv_content::Content::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../content"
        ))
        .unwrap();
        let dragon = c.amulets.iter().find(|a| a.id == "fallen_dragon").unwrap();
        let lines = effect_lines(&w, &dragon.effects, None);
        assert_eq!(lines.len(), 3);
        assert!(!lines[0].opens_with_number(), "the rule first");
        assert!(lines[0]
            .0
            .iter()
            .any(|(t, tone)| t == "50%" && *tone == Tone::Gain));
        assert!(lines[1].opens_with_number() && lines[2].opens_with_number());
        // every effect of every amulet reads
        for a in &c.amulets {
            for e in &a.effects {
                assert!(effect_line(&w, e, None).is_some(), "{}: {e:?}", a.id);
            }
        }
    }
}
