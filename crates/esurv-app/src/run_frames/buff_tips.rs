//! Our tooltips for the hero's buffs and for its spells' effects on an enemy, in a run
//! (screens.md, "The stock frames in a run"). Each buff wears the look of a stock
//! spell's aura, whose stock tooltip would say what that stock spell does; in a run the stock
//! tooltip frame shows ours instead, in its own look:
//!
//! * a buff on the hero (the buff frame at the top right, `GameTooltip:SetPlayerBuff`; the target
//!   frame's buffs while the hero targets itself, `SetUnitBuff`): its name (the spell of ours that
//!   cast it, else the word table's, a rune's), what it does now (the word table's line for its
//!   kind with its value: "+60% movement speed"), the spell's description with the hero's numbers
//!   now (from the spellbook's tooltips), the race's line for the race's spell, and the time left;
//! * an effect of the hero's on an enemy (the target frame's debuffs, `SetUnitDebuff`), when it is
//!   one of the hero's spells: its name and its description with the hero's numbers.
//!
//! The server says which buff each aura slot is (`ESURVD`, [`ToClient::Buffs`]); this module writes
//! each one's lines and hands them to the run frames' Lua by the aura's `Spell.dbc` id, with which
//! aura each buff position of the player and each debuff of the target is (benilla's
//! `UiScript::player_aura_spell_ids` and `UiScript::aura_spell_ids`: no 1.12 verb names an aura's
//! spell).

use bevy::prelude::*;

use benilla_app::Objects;
use benilla_ui::script::UiScript;
use esurv_content::Content;
use esurv_protocol::{BuffKind, BuffTip, SpellTip, TipKind, ToClient};

use super::spell_tips::{description, lua_slots, lua_tips, RunSpellTips, SpellTipText, TipRow};
use crate::cards::card_text::value_line;
use crate::protocol::ServerEvent;
use crate::ui::{in_world, GameContent, GOLD_TEXT, GRAY_TEXT, GREEN_TEXT, WHITE_TEXT};

/// The hero's buffs as the server last told them.
#[derive(Resource, Default)]
pub(super) struct RunBuffs(pub Option<Vec<BuffTip>>);

pub(super) fn add(app: &mut App) {
    app.init_resource::<RunBuffs>().add_systems(
        Update,
        (keep_buffs, hand_the_buff_tips_to_the_lua)
            .chain()
            .after(super::spell_tips::hand_the_tips_to_the_lua),
    );
}

/// Keeps the newest `Buffs` (each hero's client hears only its own); a hero out of the world
/// forgets them.
fn keep_buffs(
    mut events: MessageReader<ServerEvent>,
    objects: Objects,
    mut was_in: Local<bool>,
    mut buffs: ResMut<RunBuffs>,
) {
    let now_in = in_world(&objects);
    if *was_in && !now_in {
        buffs.0 = None;
    }
    *was_in = now_in;
    for ServerEvent(m) in events.read() {
        if let ToClient::Buffs { buffs: b, .. } = m {
            buffs.0 = Some(b.clone());
        }
    }
}

/// The spellbook tooltip of the spell `id`, for its numbers and rank.
fn spell_tip<'t>(spells: &'t [SpellTip], id: &str) -> Option<&'t SpellTip> {
    spells
        .iter()
        .find(|t| t.id == id && !matches!(t.kind, TipKind::Passive(_)))
}

/// A buff's tooltip (without the time left, which the Lua adds as it shows it): its name (the
/// spell's, else the word table's) and the spell's rank or "Racial"; what it does now, in the word
/// table's line for its kind; the spell's description with the hero's numbers; for the race's
/// spell, the race's line.
pub fn buff_text(content: &Content, spells: &[SpellTip], buff: &BuffTip) -> Option<SpellTipText> {
    if buff.kind == BuffKind::Passive {
        return passive_text(content, buff);
    }
    let words = content.words.buff(buff.kind)?;
    let ability = (!buff.spell.is_empty())
        .then(|| content.ability(&buff.spell))
        .flatten();
    let race = ability.and_then(|a| content.races.iter().find(|r| r.spell == a.id));
    let tip = ability.and_then(|a| spell_tip(spells, &a.id));
    let name = ability.map_or_else(|| words.name.clone(), |a| a.name.clone());
    let sub = match (race, tip) {
        (Some(_), _) => "Racial".to_owned(),
        (None, Some(t)) if t.rank > 0 => format!("Rank {}", t.rank),
        _ => String::new(),
    };
    let mut rows = vec![if sub.is_empty() {
        TipRow::one(&name, WHITE_TEXT)
    } else {
        TipRow::two(&name, WHITE_TEXT, &sub, GRAY_TEXT)
    }];
    rows.push(TipRow::one(
        value_line(&words.line, None, buff.value).text(),
        WHITE_TEXT,
    ));
    if let (Some(a), Some(t)) = (ability, tip) {
        if let Some(d) = description(a, t) {
            rows.push(TipRow::one(d, GOLD_TEXT));
        }
    }
    if let Some(race) = race {
        rows.push(TipRow::one(format!("{} racial", race.name), GREEN_TEXT));
    }
    Some(SpellTipText { name, sub, rows })
}

/// A starting option's stacking buff: the passive's name (or, for a spell's own stacks, the
/// spell's), how many stack of how many, and what the passive does.
fn passive_text(content: &Content, buff: &BuffTip) -> Option<SpellTipText> {
    let options = content.classes.iter().flat_map(|c| &c.starting_options);
    let (name, line) = match content.ability(&buff.spell) {
        Some(a) => (a.name.clone(), a.description.clone()),
        None => {
            let p = options.map(|o| &o.passive).find(|p| p.id == buff.spell)?;
            (p.name.clone(), p.description.clone())
        }
    };
    let mut rows = vec![TipRow::one(&name, WHITE_TEXT)];
    if buff.value > 1.0 {
        rows.push(TipRow::one(format!("{} of {}", buff.stacks, buff.value as u32), WHITE_TEXT));
    }
    if !line.trim().is_empty() && !line.contains('{') {
        rows.push(TipRow::one(line, GOLD_TEXT));
    }
    Some(SpellTipText { name, sub: String::new(), rows })
}

/// An effect of one of the hero's spells on an enemy: the spell's name, and its description with
/// the hero's numbers.
pub fn debuff_text(content: &Content, tip: &SpellTip) -> Option<SpellTipText> {
    if matches!(tip.kind, TipKind::Passive(_)) {
        return None;
    }
    let a = content.ability(&tip.id)?;
    let mut rows = vec![TipRow::two(
        &a.name,
        WHITE_TEXT,
        format!("{:?} spell", a.school),
        WHITE_TEXT,
    )];
    rows.push(TipRow::one(description(a, tip)?, GOLD_TEXT));
    Some(SpellTipText {
        name: a.name.clone(),
        sub: String::new(),
        rows,
    })
}

/// What this VM has been handed.
#[derive(Default)]
pub(super) struct Handed {
    session: u64,
    tips: Option<String>,
    slots: Option<String>,
}

/// Hands the buff and debuff tooltips, and which aura each of the player's buff positions and
/// each of the target's buffs and debuffs is, to the run frames' Lua when they change.
fn hand_the_buff_tips_to_the_lua(
    script: Option<NonSend<UiScript>>,
    buffs: Res<RunBuffs>,
    spells: Res<RunSpellTips>,
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
            .eval::<bool>(
                "return ESURV_RUN_FRAMES ~= nil and ESURV_RUN_FRAMES.set_buff_tips ~= nil",
            )
            .unwrap_or(false)
    {
        return;
    }
    let spells = spells.0.as_deref().unwrap_or_default();
    let buff_tips: Vec<(u32, SpellTipText)> = buffs
        .0
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|b| Some((b.look, buff_text(&content.0, spells, b)?)))
        .collect();
    let debuff_tips: Vec<(u32, SpellTipText)> = spells
        .iter()
        .filter_map(|t| Some((t.look, debuff_text(&content.0, t)?)))
        .collect();
    let tips = format!(
        "ESURV_RUN_FRAMES.set_buff_tips({}, {})",
        lua_tips(&buff_tips),
        lua_tips(&debuff_tips)
    );
    if handed.tips.as_deref() != Some(tips.as_str()) {
        if let Err(e) = script.run(&tips) {
            warn!("run frames: the buff tooltips: {e}");
        }
        handed.tips = Some(tips);
    }
    let numbered = |ids: Vec<u32>, first: u32| -> Vec<(u32, u32)> {
        ids.into_iter()
            .enumerate()
            .map(|(i, id)| (i as u32 + first, id))
            .collect()
    };
    let slots = format!(
        "ESURV_RUN_FRAMES.set_aura_slots({}, {}, {})",
        lua_slots(&numbered(script.player_aura_spell_ids(), 0)),
        lua_slots(&numbered(script.aura_spell_ids("player", true), 1)),
        lua_slots(&numbered(script.aura_spell_ids("target", false), 1)),
    );
    if handed.slots.as_deref() != Some(slots.as_str()) {
        if let Err(e) = script.run(&slots) {
            warn!("run frames: the aura slots: {e}");
        }
        handed.slots = Some(slots);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero_sheet::gear::tests::content;
    use esurv_protocol::OfferValue;

    fn texts(t: &SpellTipText) -> Vec<String> {
        t.rows
            .iter()
            .map(|r| match &r.right {
                Some(right) => format!("{} | {}", r.left.text, right.text),
                None => r.left.text.clone(),
            })
            .collect()
    }

    fn buff(look: u32, kind: BuffKind, spell: &str, value: f32) -> BuffTip {
        BuffTip {
            look,
            kind,
            spell: spell.into(),
            value,
            stacks: 1,
        }
    }

    #[test]
    fn every_buff_kind_has_its_words() {
        let c = content();
        for k in BuffKind::ALL {
            if k == BuffKind::Passive {
                continue; // a passive's own words
            }
            let w = c.words.buff(k).unwrap_or_else(|| panic!("{k:?}"));
            assert!(!w.name.is_empty() && !w.line.is_empty());
        }
    }

    #[test]
    fn a_rune_says_its_name_and_what_it_does_now() {
        let c = content();
        let t = buff_text(c, &[], &buff(23451, BuffKind::Speed, "", 0.6)).unwrap();
        assert_eq!(texts(&t), ["Speed Rune", "+60% movement speed"]);
        let t = buff_text(c, &[], &buff(23505, BuffKind::Berserk, "", 0.5)).unwrap();
        assert_eq!(texts(&t), ["Berserker Rune", "+50% damage"]);
    }

    #[test]
    fn the_race_spells_buff_says_its_live_numbers_and_whose_it_is() {
        let c = content();
        let fury = c.ability("blood_fury").unwrap();
        let v = |what: &str, value| OfferValue {
            what: what.into(),
            value,
        };
        let spells = [SpellTip {
            look: fury.look_spell,
            kind: TipKind::Racial,
            id: fury.id.clone(),
            rank: 0,
            numbers: vec![
                v("cooldown", 40.0),
                v("duration", 12.0),
                v("buff", 0.42),
                v("buff_range", 30.0),
            ],
            upgrades: vec![],
            bonuses: vec![],
            crit: None,
        }];
        let t = buff_text(
            c,
            &spells,
            &buff(fury.look_spell, BuffKind::Damage, "blood_fury", 0.42),
        )
        .unwrap();
        let lines = texts(&t);
        assert_eq!(lines[0], "Spell 20572 | Racial");
        assert_eq!(lines[1], "+42% damage");
        assert!(
            lines[2].contains("42%") && lines[2].contains("12"),
            "{lines:?}"
        );
        assert_eq!(lines[3], "Race 2 racial");
        // its effect on an enemy, were it one: the spell's name and description
        let d = debuff_text(c, &spells[0]).unwrap();
        assert_eq!(texts(&d)[0], "Spell 20572 | Physical spell");
    }
}
