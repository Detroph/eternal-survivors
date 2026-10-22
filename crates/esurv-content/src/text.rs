//! Card texts as templates (design step 9): a spell's `description` and the
//! words in `words.ron` hold `{name}` placeholders that a card fills with the spell's numbers at
//! the rank it offers. The names a spell description may use are [`SPELL_NUMBERS`] (and
//! `{school}`); the run's own rules work the numbers out (`esurv-sim` `game::spell_numbers`).
//!
//! The template forms:
//!
//! * `{damage}`: the number, shown the way its kind reads (`74`, `5.6`, `35%`);
//! * `{targets:the nearest enemy|the # nearest enemies}`: the first words when the number is 1,
//!   else the second, `#` standing for the number;
//! * `{{` and `}}`: a brace.

use std::fmt;

/// How a number reads on a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberKind {
    /// Damage or another amount, whole: `74`.
    Amount,
    /// How many, whole: `3`.
    Count,
    /// Seconds, to a tenth: `5.6`.
    Seconds,
    /// Yards, to a tenth: `12`.
    Yards,
    /// A fraction shown as a percentage, to a tenth: `0.35` reads `35%`.
    Share,
}

impl NumberKind {
    /// `value` as a card shows it.
    pub fn show(self, value: f32) -> String {
        match self {
            NumberKind::Amount | NumberKind::Count => format!("{}", value.round().max(0.0) as i64),
            NumberKind::Seconds | NumberKind::Yards => tenths(value),
            NumberKind::Share => format!("{}%", tenths(value * 100.0)),
        }
    }
}

/// A number to a tenth at most, with no trailing zero: `2`, `5.6`.
pub fn tenths(value: f32) -> String {
    let t = (value * 10.0).round();
    if t % 10.0 == 0.0 {
        format!("{}", t / 10.0)
    } else {
        format!("{:.1}", t / 10.0)
    }
}

/// Every number a spell description may name, and how it reads. What each means is said where
/// the run works it out (`esurv-sim` `game::spell_numbers`); a spell has only the ones that
/// fit what it does.
pub const SPELL_NUMBERS: &[(&str, NumberKind)] = &[
    ("damage", NumberKind::Amount),
    ("tick", NumberKind::Amount),
    ("ticks", NumberKind::Count),
    ("every", NumberKind::Seconds),
    ("pulses", NumberKind::Count),
    ("novas", NumberKind::Count),
    ("low_damage", NumberKind::Amount),
    ("low_health", NumberKind::Share),
    ("burn", NumberKind::Amount),
    ("burn_for", NumberKind::Seconds),
    ("impact", NumberKind::Amount),
    ("targets", NumberKind::Count),
    ("range", NumberKind::Yards),
    ("radius", NumberKind::Yards),
    ("duration", NumberKind::Seconds),
    ("cooldown", NumberKind::Seconds),
    ("falloff", NumberKind::Share),
    ("slow", NumberKind::Share),
    ("slow_for", NumberKind::Seconds),
    ("hold", NumberKind::Seconds),
    ("boss_hold", NumberKind::Seconds),
    ("fear", NumberKind::Seconds),
    ("leech", NumberKind::Share),
    ("heal", NumberKind::Share),
    ("heal_below", NumberKind::Share),
    ("charges", NumberKind::Count),
    ("charge_heal", NumberKind::Share),
    ("shield", NumberKind::Share),
    ("buff", NumberKind::Share),
    ("haste", NumberKind::Share),
    ("swing_speed", NumberKind::Share),
    ("buff_range", NumberKind::Yards),
    ("weaken", NumberKind::Share),
    ("weaken_for", NumberKind::Seconds),
    ("immune_below", NumberKind::Share),
    ("aura", NumberKind::Amount),
    ("totem_range", NumberKind::Yards),
    ("vulnerable", NumberKind::Share),
    ("vulnerable_for", NumberKind::Seconds),
    ("cost", NumberKind::Amount),
    ("combo_points", NumberKind::Count),
    ("finisher_at", NumberKind::Count),
    ("crit_bonus", NumberKind::Share),
    ("block", NumberKind::Share),
    ("stacks", NumberKind::Count),
    ("chance", NumberKind::Share),
    ("slow_chance", NumberKind::Share),
    ("hits", NumberKind::Count),
    ("energy", NumberKind::Amount),
    ("energy_below", NumberKind::Amount),
    ("count", NumberKind::Count),
    ("summon_for", NumberKind::Seconds),
    ("reduction", NumberKind::Share),
    ("reduction_for", NumberKind::Seconds),
    ("delay", NumberKind::Seconds),
    ("speed", NumberKind::Share),
];

/// The placeholder for the spell's school's name ("Fire").
pub const SCHOOL: &str = "school";

/// How the spell number `name` reads, if it is one.
pub fn spell_number_kind(name: &str) -> Option<NumberKind> {
    SPELL_NUMBERS.iter().find(|(n, _)| *n == name).map(|(_, k)| *k)
}

/// One part of a template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part<'a> {
    Words(String),
    /// `{name}`.
    Number(&'a str),
    /// `{name:one|many}`.
    Choice { name: &'a str, one: &'a str, many: &'a str },
}

impl Part<'_> {
    /// The placeholder it names, if any.
    pub fn name(&self) -> Option<&str> {
        match self {
            Part::Words(_) => None,
            Part::Number(n) | Part::Choice { name: n, .. } => Some(n),
        }
    }
}

/// A template that does not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateError(pub String);

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A template's parts, in order.
pub fn parse(template: &str) -> Result<Vec<Part<'_>>, TemplateError> {
    let mut parts = Vec::new();
    let mut words = String::new();
    let mut rest = template;
    while let Some(i) = rest.find(['{', '}']) {
        words.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            words.push_str(&tail[..1]);
            rest = &tail[2..];
            continue;
        }
        if tail.starts_with('}') {
            return Err(TemplateError(format!("a lone }} in {template:?}")));
        }
        let Some(end) = tail.find('}') else {
            return Err(TemplateError(format!("an unclosed {{ in {template:?}")));
        };
        let inner = &tail[1..end];
        if inner.contains('{') {
            return Err(TemplateError(format!("a {{ inside a placeholder in {template:?}")));
        }
        if !words.is_empty() {
            parts.push(Part::Words(std::mem::take(&mut words)));
        }
        let part = match inner.split_once(':') {
            None => Part::Number(inner.trim()),
            Some((name, choice)) => {
                let Some((one, many)) = choice.split_once('|') else {
                    return Err(TemplateError(format!("{{{inner}}} needs one|many in {template:?}")));
                };
                Part::Choice { name: name.trim(), one, many }
            }
        };
        if part.name().is_some_and(str::is_empty) {
            return Err(TemplateError(format!("an empty placeholder in {template:?}")));
        }
        parts.push(part);
        rest = &tail[end + 1..];
    }
    words.push_str(rest);
    if !words.is_empty() {
        parts.push(Part::Words(words));
    }
    Ok(parts)
}

/// One piece of a filled template: words, or a number filled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filled {
    Words(String),
    Value(String),
}

/// A filled number: how it reads, and whether it is exactly one (for `{name:one|many}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub text: String,
    pub is_one: bool,
}

impl Shown {
    pub fn new(text: impl Into<String>, is_one: bool) -> Self {
        Self { text: text.into(), is_one }
    }

    /// A number of `kind`, as a card shows it.
    pub fn number(kind: NumberKind, value: f32) -> Self {
        let text = kind.show(value);
        Self { is_one: text == "1", text }
    }
}

/// `template` filled from `value`; an error names the first placeholder `value` has nothing for.
/// A choice's words are words, with its number (`#`) a value.
pub fn fill(template: &str, value: impl Fn(&str) -> Option<Shown>) -> Result<Vec<Filled>, TemplateError> {
    let mut out: Vec<Filled> = Vec::new();
    let words = |out: &mut Vec<Filled>, w: &str| {
        if w.is_empty() {
            return;
        }
        match out.last_mut() {
            Some(Filled::Words(last)) => last.push_str(w),
            _ => out.push(Filled::Words(w.to_owned())),
        }
    };
    for part in parse(template)? {
        match part {
            Part::Words(w) => words(&mut out, &w),
            Part::Number(name) => {
                let shown = value(name).ok_or_else(|| TemplateError(format!("nothing for {{{name}}} in {template:?}")))?;
                out.push(Filled::Value(shown.text));
            }
            Part::Choice { name, one, many } => {
                let shown = value(name).ok_or_else(|| TemplateError(format!("nothing for {{{name}}} in {template:?}")))?;
                let pick = if shown.is_one { one } else { many };
                let mut pieces = pick.split('#').peekable();
                while let Some(w) = pieces.next() {
                    words(&mut out, w);
                    if pieces.peek().is_some() {
                        out.push(Filled::Value(shown.text.clone()));
                    }
                }
            }
        }
    }
    Ok(out)
}

/// The filled pieces as one plain text.
pub fn plain(filled: &[Filled]) -> String {
    filled.iter().map(|f| match f { Filled::Words(w) | Filled::Value(w) => w.as_str() }).collect()
}

/// Checks a spell description: it reads, and names only spell numbers and `{school}`.
pub fn check_spell_description(template: &str) -> Result<(), TemplateError> {
    for part in parse(template)? {
        if let Some(name) = part.name() {
            if name != SCHOOL && spell_number_kind(name).is_none() {
                return Err(TemplateError(format!("{{{name}}} is no spell number, in {template:?}")));
            }
        }
    }
    Ok(())
}


/// A share as a percentage, to a tenth at most: 0.46 reads "46%", 0.065 "6.5%".
pub fn percent(share: f32) -> String {
    NumberKind::Share.show(share)
}

/// A number to a tenth at most, with no trailing zero.
pub fn number(value: f32) -> String {
    tenths(value)
}

/// A jewelry text with its `{v}`: a share where a `%` follows it, else the number as it is.
pub fn with_value(text: &str, value: f32) -> String {
    if text.contains("{v}%") {
        text.replace("{v}%", &percent(value))
    } else {
        text.replace("{v}", &number(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_values_read_plainly() {
        assert_eq!(percent(0.46), "46%");
        assert_eq!(percent(0.065), "6.5%");
        assert_eq!(number(2.0), "2");
        assert_eq!(with_value("+{v} health per second", 1.5), "+1.5 health per second");
        assert_eq!(with_value("{v}% chance for a spell to cast twice", 0.06), "6% chance for a spell to cast twice");
    }

    fn numbers(name: &str) -> Option<Shown> {
        match name {
            "damage" => Some(Shown::number(NumberKind::Amount, 73.6)),
            "targets" => Some(Shown::number(NumberKind::Count, 1.0)),
            "radius" => Some(Shown::number(NumberKind::Yards, 12.0)),
            "slow" => Some(Shown::number(NumberKind::Share, 0.065)),
            "school" => Some(Shown::new("Holy", false)),
            _ => None,
        }
    }

    #[test]
    fn numbers_read_by_their_kind() {
        assert_eq!(NumberKind::Amount.show(73.6), "74");
        assert_eq!(NumberKind::Seconds.show(5.6), "5.6");
        assert_eq!(NumberKind::Seconds.show(3.0), "3");
        assert_eq!(NumberKind::Share.show(0.35), "35%");
        assert_eq!(NumberKind::Share.show(0.065), "6.5%");
    }

    #[test]
    fn a_template_fills_its_numbers_and_choices() {
        let t = "Hits {targets:the nearest enemy|the # nearest enemies} within {radius} yd for {damage} {school} damage, {{braces}}.";
        let filled = fill(t, numbers).unwrap();
        assert_eq!(plain(&filled), "Hits the nearest enemy within 12 yd for 74 Holy damage, {braces}.");
        assert_eq!(filled[1], Filled::Value("12".into()));
        let three = fill(t, |n| if n == "targets" { Some(Shown::number(NumberKind::Count, 3.0)) } else { numbers(n) }).unwrap();
        assert!(plain(&three).starts_with("Hits the 3 nearest enemies within"));
        assert!(three.contains(&Filled::Value("3".into())));
    }

    #[test]
    fn a_missing_number_or_a_broken_template_is_an_error() {
        assert!(fill("{ticks} ticks", numbers).unwrap_err().0.contains("{ticks}"));
        assert!(parse("{damage").is_err());
        assert!(parse("damage}").is_err());
        assert!(parse("{targets:one}").is_err());
        assert!(check_spell_description("{damage} {school}").is_ok());
        assert!(check_spell_description("{dmg}").is_err());
    }
}
