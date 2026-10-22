//! Our tooltips for the hero's spells in a run (screens.md, "The stock
//! frames in a run"). The action bar and the spellbook hold each spell as the stock spell it looks
//! like (its `Spell.dbc` id), so the stock tooltip would say what that stock spell does. In a run
//! the stock tooltip frame (`GameTooltip`) shows ours instead, in its own look: the spell's name
//! and its rank in this run, its school and range, its cooldown, and its description filled with
//! the hero's numbers now, from the same templates and numbers a card uses (`abilities.ron`,
//! `esurv_content::text`, the server's `spell_numbers`); the race's spell says it is the
//! race's, and a race's passive says the race's line for it.
//!
//! The server sends every spell of the hero's spellbook with its numbers when one of them changes
//! (`ESURVL`, [`ToClient::SpellTips`]); this module writes each one's lines and hands them to the
//! run frames' Lua by the spell's `Spell.dbc` id, with which spell each action slot and spellbook
//! slot holds (benilla's `UiScript::action_spell` and `UiScript::spellbook`: 1.12's Lua has no
//! call that names an action slot's spell). The Lua answers `GameTooltip:SetAction`,
//! `GameTooltip:SetSpell` and `GetSpellName` for those spells while the run is on.

use std::fmt::Write as _;

use bevy::prelude::*;

use benilla_app::Objects;
use benilla_ui::script::UiScript;
use esurv_content::text::{fill, plain, spell_number_kind, Shown};
use esurv_content::{Ability, Content};
use esurv_protocol::{OfferValue, SpellTip, TipKind, ToClient};

use super::lua_string;
use crate::cards::card_text::{percent, spell_shown, upgrade_line, upgrade_named};
use crate::protocol::ServerEvent;
use crate::stock::TipLine;
use crate::ui::{in_world, GameContent, GOLD_TEXT, GRAY_TEXT, GREEN_TEXT, WHITE_TEXT};

/// The highest Lua action id (the stock bar's 120 slots).
const ACTIONS: u32 = 120;

/// The hero's spellbook as the server last told it.
#[derive(Resource, Default)]
pub(super) struct RunSpellTips(pub Option<Vec<SpellTip>>);

pub(super) fn add(app: &mut App) {
    app.init_resource::<RunSpellTips>().add_systems(
        Update,
        (keep_spell_tips, hand_the_tips_to_the_lua)
            .chain()
            .after(super::keep_the_frames),
    );
}

/// Keeps the newest `SpellTips` (each hero's client hears only its own); a hero out of the world
/// forgets them.
fn keep_spell_tips(
    mut events: MessageReader<ServerEvent>,
    objects: Objects,
    mut was_in: Local<bool>,
    mut tips: ResMut<RunSpellTips>,
) {
    let now_in = in_world(&objects);
    if *was_in && !now_in {
        tips.0 = None;
    }
    *was_in = now_in;
    for ServerEvent(m) in events.read() {
        if let ToClient::SpellTips { spells, .. } = m {
            tips.0 = Some(spells.clone());
        }
    }
}

/// One line of a tooltip: its left text, and its right column if any (the stock spell tooltip's
/// rank, range and cooldown columns). A line without a right column wraps.
#[derive(Clone, Debug, PartialEq)]
pub struct TipRow {
    pub left: TipLine,
    pub right: Option<TipLine>,
}

impl TipRow {
    pub(super) fn one(text: impl Into<String>, colour: Color) -> Self {
        Self {
            left: TipLine::new(text, colour),
            right: None,
        }
    }

    pub(super) fn two(
        left: impl Into<String>,
        lc: Color,
        right: impl Into<String>,
        rc: Color,
    ) -> Self {
        Self {
            left: TipLine::new(left, lc),
            right: Some(TipLine::new(right, rc)),
        }
    }
}

/// A spell's tooltip, and what the spellbook writes under its name.
#[derive(Clone, Debug, PartialEq)]
pub struct SpellTipText {
    pub name: String,
    /// "Rank 2", "Racial", "Passive".
    pub sub: String,
    pub rows: Vec<TipRow>,
}

/// The number named `what` among a tip's numbers.
fn number(tip: &SpellTip, what: &str) -> Option<f32> {
    tip.numbers.iter().find(|n| n.what == what).map(|n| n.value)
}

/// A number as a card writes it.
fn shown(what: &str, value: f32) -> String {
    spell_number_kind(what).map_or_else(|| value.to_string(), |k| k.show(value))
}

/// A spell's description filled with the tip's numbers (a number it lacks reads "?").
pub(super) fn description(a: &Ability, tip: &SpellTip) -> Option<String> {
    if a.description.trim().is_empty() {
        return None;
    }
    let filled = fill(&a.description, |name| {
        Some(spell_shown(a, &tip.numbers, name).unwrap_or_else(|| Shown::new("?", false)))
    })
    .ok()?;
    Some(plain(&filled))
}

/// The totals of one kind of a tip (`upgrades` or `bonuses`) as one sentence of the card's own
/// upgrade lines ("+8% damage, +18% area, +1 target"); `None` when there is none.
fn totals_line(content: &Content, a: &Ability, totals: &[OfferValue]) -> Option<String> {
    let parts: Vec<String> = totals
        .iter()
        .filter_map(|v| {
            let kind = upgrade_named(&v.what)?;
            Some(upgrade_line(&content.words, kind, v.value, Some(a)).text())
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// The tooltip of one spell of the hero's spellbook, as the stock spell tooltip lays one out:
/// name and rank; its range or radius, and its school; how it casts and its cooldown; the
/// description in gold. Under it, apart: what the upgrade cards taken add up to (white), what the
/// hero's talents, gear, jewelry, amulet and race add (green), and its crit (white), each only
/// when there is something to say, in the cards' words. The race's spell has no rank and says
/// whose it is; a race's passive shows the race's line for it.
pub fn tip_text(content: &Content, tip: &SpellTip) -> Option<SpellTipText> {
    if let TipKind::Passive(i) = tip.kind {
        let race = content.races.iter().find(|r| r.id == tip.id)?;
        let p = race.passives.get(i as usize)?;
        // a line that names the passive first ("<name>: ...") drops it: the tip shows the name above
        let line = p
            .text
            .strip_prefix(&format!("{}: ", p.name))
            .unwrap_or(&p.text);
        return Some(SpellTipText {
            name: p.name.clone(),
            sub: "Passive".into(),
            rows: vec![
                TipRow::two(&p.name, WHITE_TEXT, "Passive", GRAY_TEXT),
                TipRow::one(format!("{} racial", race.name), GREEN_TEXT),
                TipRow::one(line, GOLD_TEXT),
            ],
        });
    }
    let a = content.ability(&tip.id)?;
    let race = (tip.kind == TipKind::Racial)
        .then(|| content.races.iter().find(|r| r.spell == a.id))
        .flatten();
    let sub = match race {
        Some(_) => "Racial".to_owned(),
        None => format!("Rank {}", tip.rank.max(1)),
    };
    let mut rows = vec![TipRow::two(&a.name, WHITE_TEXT, &sub, GRAY_TEXT)];
    // its reach: the range it casts at, else the radius it covers
    let reach = number(tip, "range")
        .map(|r| format!("{} yd range", shown("range", r)))
        .or_else(|| number(tip, "radius").map(|r| format!("{} yd radius", shown("radius", r))));
    let school = format!("{:?} spell", a.school);
    rows.push(match reach {
        Some(reach) => TipRow::two(reach, WHITE_TEXT, school, WHITE_TEXT),
        None => TipRow::one(school, WHITE_TEXT),
    });
    if let Some(c) = number(tip, "cooldown").filter(|c| *c > 0.0) {
        rows.push(TipRow::two(
            "Casts itself",
            WHITE_TEXT,
            format!("{} sec cooldown", shown("cooldown", c)),
            WHITE_TEXT,
        ));
    }
    if let Some(d) = description(a, tip) {
        rows.push(TipRow::one(d, GOLD_TEXT));
    }
    let upgrades = totals_line(content, a, &tip.upgrades);
    let bonuses = totals_line(content, a, &tip.bonuses);
    let crit = tip.crit.map(|(chance, damage)| {
        format!(
            "{} chance to crit for {} damage",
            percent(chance),
            percent(damage)
        )
    });
    if upgrades.is_some() || bonuses.is_some() || crit.is_some() {
        // set apart from the description, as the stock tooltip sets its lines apart
        rows.push(TipRow::one(" ", WHITE_TEXT));
    }
    if let Some(u) = upgrades {
        rows.push(TipRow::one(format!("Upgrades this run: {u}"), WHITE_TEXT));
    }
    if let Some(b) = bonuses {
        rows.push(TipRow::one(format!("Your bonuses: {b}"), GREEN_TEXT));
    }
    if let Some(c) = crit {
        rows.push(TipRow::one(c, WHITE_TEXT));
    }
    if let Some(race) = race {
        rows.push(TipRow::one(
            format!(
                "{} racial: it casts from a slot of its own, and cards never upgrade it.",
                race.name
            ),
            GREEN_TEXT,
        ));
    }
    Some(SpellTipText {
        name: a.name.clone(),
        sub,
        rows,
    })
}

/// `(r, g, b)` of a colour, as the Lua takes it.
fn rgb(c: Color) -> String {
    let c = c.to_srgba();
    format!("{:.3},{:.3},{:.3}", c.red, c.green, c.blue)
}

/// The tips as the Lua table `ESURV_RUN_FRAMES.set_spell_tips` takes: by `Spell.dbc` id,
/// `{ name, sub, lines = { {left, r, g, b [, right, r, g, b]}, ... } }`.
pub fn lua_spell_tips(content: &Content, tips: &[SpellTip]) -> String {
    let texts: Vec<(u32, SpellTipText)> = tips
        .iter()
        .filter_map(|tip| Some((tip.look, tip_text(content, tip)?)))
        .collect();
    lua_tips(&texts)
}

/// Each spell's own icon (its card's) by `Spell.dbc` id, as the Lua table
/// `ESURV_RUN_FRAMES.set_spell_icons` takes: `{[28883]="Interface\\Icons\\..."}`. A race's
/// passive keeps its stock racial spell's icon.
pub fn lua_spell_icons(content: &Content, tips: &[SpellTip]) -> String {
    let mut out = String::from("{");
    for tip in tips.iter().filter(|t| !matches!(t.kind, TipKind::Passive(_))) {
        if let Some(a) = content.ability(&tip.id) {
            let _ = write!(out, "[{}]={},", tip.look, lua_string(&format!(r"Interface\Icons\{}", a.icon)));
        }
    }
    out.push('}');
    out
}

/// Tooltips by `Spell.dbc` id as a Lua table: `{ [id] = { name, sub, lines = { {left, r, g, b
/// [, right, r, g, b]}, ... } } }`.
pub(super) fn lua_tips(tips: &[(u32, SpellTipText)]) -> String {
    let mut out = String::from("{");
    for (look, t) in tips {
        let _ = write!(
            out,
            "[{}]={{name={},sub={},lines={{",
            look,
            lua_string(&t.name),
            lua_string(&t.sub)
        );
        for row in &t.rows {
            let _ = write!(
                out,
                "{{{},{}",
                lua_string(&row.left.text),
                rgb(row.left.colour)
            );
            if let Some(r) = &row.right {
                let _ = write!(out, ",{},{}", lua_string(&r.text), rgb(r.colour));
            }
            out.push_str("},");
        }
        out.push_str("}},");
    }
    out.push('}');
    out
}

/// A list of spell ids by slot as a Lua table: `{[1]=133,[24]=116}`.
pub(super) fn lua_slots(slots: &[(u32, u32)]) -> String {
    let mut out = String::from("{");
    for (slot, spell) in slots {
        let _ = write!(out, "[{slot}]={spell},");
    }
    out.push('}');
    out
}

/// What this VM has been handed.
#[derive(Default)]
pub(super) struct Handed {
    session: u64,
    tips: Option<String>,
    slots: Option<String>,
}

/// Hands the tips, and which spell each action slot and spellbook slot holds, to the run frames'
/// Lua when they change.
pub(super) fn hand_the_tips_to_the_lua(
    script: Option<NonSend<UiScript>>,
    tips: Res<RunSpellTips>,
    content: Option<Res<GameContent>>,
    objects: Objects,
    mut handed: Local<Handed>,
) {
    let (Some(script), Some(content)) = (script, content) else {
        return;
    };
    if handed.session != script.session() {
        *handed = Handed {
            session: script.session(),
            ..default()
        };
    }
    if !in_world(&objects)
        || !script
            .eval::<bool>("return ESURV_RUN_FRAMES ~= nil and ESURV_RUN_FRAMES.set_spell_tips ~= nil")
            .unwrap_or(false)
    {
        return;
    }
    let (table, icons) = tips.0.as_deref().map_or_else(
        || ("{}".to_owned(), "{}".to_owned()),
        |t| (lua_spell_tips(&content.0, t), lua_spell_icons(&content.0, t)),
    );
    let table = format!("ESURV_RUN_FRAMES.set_spell_tips({table}); ESURV_RUN_FRAMES.set_spell_icons({icons})");
    if handed.tips.as_deref() != Some(table.as_str()) {
        if let Err(e) = script.run(&table) {
            warn!("run frames: the spell tooltips: {e}");
        }
        handed.tips = Some(table);
    }
    let actions: Vec<(u32, u32)> = (1..=ACTIONS)
        .filter_map(|a| script.action_spell(a).map(|s| (a, s)))
        .collect();
    let book: Vec<(u32, u32)> = script
        .spellbook()
        .slots
        .iter()
        .enumerate()
        .map(|(i, s)| (i as u32 + 1, s.spell_id))
        .collect();
    let slots = format!(
        "ESURV_RUN_FRAMES.set_spell_slots({}, {})",
        lua_slots(&actions),
        lua_slots(&book)
    );
    if handed.slots.as_deref() != Some(slots.as_str()) {
        if let Err(e) = script.run(&slots) {
            warn!("run frames: the spell slots: {e}");
        }
        handed.slots = Some(slots);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero_sheet::gear::tests::content;
    use esurv_protocol::OfferValue;

    fn v(what: &str, value: f32) -> OfferValue {
        OfferValue {
            what: what.into(),
            value,
        }
    }

    fn tip(look: u32, kind: TipKind, id: &str, rank: u32, numbers: Vec<OfferValue>) -> SpellTip {
        SpellTip {
            look,
            kind,
            id: id.into(),
            rank,
            numbers,
            upgrades: vec![],
            bonuses: vec![],
            crit: None,
        }
    }

    fn texts(t: &SpellTipText) -> Vec<String> {
        t.rows
            .iter()
            .map(|r| match &r.right {
                Some(right) => format!("{} | {}", r.left.text, right.text),
                None => r.left.text.clone(),
            })
            .collect()
    }

    #[test]
    fn a_learned_spell_says_its_rank_and_the_heros_numbers() {
        let c = content();
        let fireball = c.ability("fireball").unwrap();
        let t = tip_text(
            c,
            &tip(
                fireball.look_spell,
                TipKind::Learned,
                "fireball",
                3,
                vec![
                    v("cooldown", 1.82),
                    v("range", 30.0),
                    v("damage", 70.7),
                    v("targets", 1.0),
                ],
            ),
        )
        .unwrap();
        let lines = texts(&t);
        assert_eq!((t.name.as_str(), t.sub.as_str()), ("Spell 133", "Rank 3"));
        assert_eq!(lines[0], "Spell 133 | Rank 3");
        assert_eq!(lines[1], "30 yd range | Fire spell");
        assert_eq!(lines[2], "Casts itself | 1.8 sec cooldown");
        assert!(lines[3].contains("71"), "the hero's damage: {lines:?}");
        assert!(
            !lines[3].contains('{') && !lines[3].contains('?'),
            "{lines:?}"
        );
        assert_eq!(t.rows[3].left.colour, GOLD_TEXT, "the description is gold");
        assert_eq!(
            lines.len(),
            4,
            "nothing taken, no bonus, no crit: {lines:?}"
        );
    }

    #[test]
    fn the_totals_read_in_the_cards_words_under_the_description() {
        let c = content();
        let area = c.ability("consecration").unwrap();
        let t = tip_text(
            c,
            &SpellTip {
                upgrades: vec![v("power", 0.08), v("area", 0.18), v("duration", 0.15)],
                bonuses: vec![v("power", 0.31), v("cooldown", 0.04), v("area", 0.03)],
                crit: Some((0.07, 2.04)),
                ..tip(
                    area.look_spell,
                    TipKind::Learned,
                    "consecration",
                    3,
                    vec![v("cooldown", 11.0), v("radius", 9.7), v("damage", 213.0)],
                )
            },
        )
        .unwrap();
        let lines = texts(&t);
        assert_eq!(
            lines[1], "9.7 yd radius | Holy spell",
            "no range: the radius"
        );
        assert_eq!(lines[2], "Casts itself | 11 sec cooldown");
        let n = lines.len();
        assert_eq!(
            &lines[n - 4..],
            [
                " ",
                "Upgrades this run: +8% damage, +18% area, +15% duration",
                "Your bonuses: +31% damage, -4% cooldown, +3% area",
                "7% chance to crit for 204% damage",
            ]
        );
        assert_eq!(
            t.rows[n - 2].left.colour,
            GREEN_TEXT,
            "the bonuses are green"
        );
    }

    #[test]
    fn the_race_spell_and_a_passive_say_whose_they_are() {
        let c = content();
        let orc = c.races.iter().find(|r| r.id == "orc").unwrap();
        let fury = c.ability(&orc.spell).unwrap();
        let t = tip_text(
            c,
            &tip(
                fury.look_spell,
                TipKind::Racial,
                &fury.id,
                0,
                vec![
                    v("cooldown", 40.0),
                    v("duration", 12.0),
                    v("buff", 0.35),
                    v("buff_range", 30.0),
                ],
            ),
        )
        .unwrap();
        let lines = texts(&t);
        assert_eq!(lines[0], "Spell 20572 | Racial");
        assert_eq!(lines[2], "Casts itself | 40 sec cooldown");
        assert!(
            lines[3].contains("35%") && lines[3].contains("12"),
            "{lines:?}"
        );
        assert!(lines.last().unwrap().starts_with("Race 2 racial"), "{lines:?}");
        let t = tip_text(c, &tip(20573, TipKind::Passive(0), "orc", 0, vec![])).unwrap();
        assert_eq!(
            texts(&t),
            ["Spell 20573 | Passive", "Race 2 racial", "Maximum health +10%."]
        );
        assert_eq!(t.sub, "Passive");
        assert!(
            tip_text(c, &tip(1, TipKind::Passive(9), "orc", 0, vec![])).is_none(),
            "no such passive"
        );
    }

    #[test]
    fn the_tips_read_as_lua_by_spell_id() {
        let c = content();
        let fireball = c.ability("fireball").unwrap();
        let lua = mlua::Lua::new();
        let table = lua_spell_tips(
            c,
            &[tip(
                fireball.look_spell,
                TipKind::Learned,
                "fireball",
                2,
                vec![v("cooldown", 2.0), v("damage", 50.0)],
            )],
        );
        let (name, sub, first, right, lines): (String, String, String, String, i64) = lua
            .load(format!(
                "local t = ({table})[{}] return t.name, t.sub, t.lines[1][1], t.lines[1][5], table.getn(t.lines)",
                fireball.look_spell
            ))
            .eval()
            .unwrap();
        assert_eq!(
            (name.as_str(), sub.as_str(), first.as_str(), right.as_str()),
            ("Spell 133", "Rank 2", "Spell 133", "Rank 2")
        );
        assert!(lines >= 3);
        assert_eq!(lua_slots(&[(1, 133), (72, 116)]), "{[1]=133,[72]=116,}");
    }
}
