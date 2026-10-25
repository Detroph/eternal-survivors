//! What a card says for each kind of offer, from the server's numbers, the hero's kit and the
//! game's content: its icon, name and rarity, its badge (NEW, RING, TRINKET, AMULET, or the rank
//! it reaches), the line under its name, its lines, and the slot it fills as its footer
//! (screens.md, "What a card says"):
//!
//! * a spell the hero does not know is written out in full, its description filled with its
//!   numbers at the rank offered; a card above common adds its head start under it;
//! * an upgrade says only what it improves ("+34% damage"), its ranks under its name;
//! * a new ring or trinket says its line ("+19% critical strike"), an upgrade its value before
//!   and after ("+70% » +114% critical strike damage");
//! * an amulet lists its rules as sentences, then its stat lines; the one it would replace shows
//!   quietly at the card's foot.
//!
//! Also what the side panels say of the hero's spells and worn jewelry.

use esurv_content::text::{fill, spell_number_kind, Shown, SCHOOL};
use esurv_content::{Ability, Content, JewelrySlot, Words};
use esurv_protocol::{Kit, Offer, OfferKind, OfferValue, ScreenKind, Worn};
use esurv_sim::game::amulets::{empowered, legendary_named};
use esurv_sim::game::offers::RARITIES;
use esurv_sim::game::spell_numbers::new_spell_numbers;
use esurv_sim::game::stats::compute;
use esurv_sim::game::upgrades::MAX_SPELLS;

use super::lines::{effect_lines, filled_line};
pub(crate) use super::lines::{
    number, percent, upgrade_line, upgrade_named, value_line, jewelry_line, Line, Tone,
};

/// A ring or trinket slot holds at most this many.
pub(crate) const JEWELRY_SLOTS: u32 = 2;

/// The icon a card without one of its own shows, and the gold card's.
const UNKNOWN_ICON: &str = "INV_Misc_QuestionMark";
pub(crate) const GOLD_ICON: &str = "INV_Misc_Coin_01";

/// The item quality names, common to legendary.
pub(crate) const RARITY_NAMES: [&str; 5] = ["Common", "Uncommon", "Rare", "Epic", "Legendary"];

/// The small framed label at a card's top right: what is new about it, or the rank it reaches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Badge {
    /// "NEW", "RING", "TRINKET", "AMULET", "RANK 3".
    pub text: String,
    /// Something taken again (a rank): outlined in the rarity's colour, not filled with it.
    pub again: bool,
}

fn badge(text: impl Into<String>, again: bool) -> Option<Badge> {
    Some(Badge {
        text: text.into(),
        again,
    })
}

/// A worn piece or a known spell as the screen shows it: in a side panel's row, and as the
/// amulet a card would replace.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PieceText {
    pub icon: String,
    pub name: String,
    /// Its rarity, where it has one (an amulet).
    pub rarity: Option<u8>,
    pub rank: u32,
    /// Its tooltip under its name.
    pub lines: Vec<Line>,
}

/// Everything a card shows.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CardText {
    /// The icon's name in the content (`Spell_Fire_Fireball`).
    pub icon: String,
    pub name: String,
    /// 0 common to 4 legendary.
    pub rarity: u8,
    pub badge: Option<Badge>,
    /// Under its name: "New spell", "Ring", "Amulet", "Rank 2 » 3".
    pub subtitle: String,
    /// What it brings: the card's middle.
    pub lines: Vec<Line>,
    /// What taking it does to the hero's slots, quietly at its foot: "Takes a spell slot · 4/6",
    /// "Improves a spell you know"; for an amulet that would replace the worn one, "Replaces"
    /// and that amulet (`replaces`).
    pub footer: String,
    /// The worn amulet an amulet card would replace.
    pub replaces: Option<PieceText>,
    /// The card's tooltip under its name (a spell's school and how often it casts; an
    /// empowered amulet's effects), if it has one.
    pub tip: Vec<Line>,
}

impl CardText {
    /// "Rare", the rarity's name.
    pub fn rarity_name(&self) -> &'static str {
        RARITY_NAMES[usize::from(self.rarity.min(4))]
    }
}

/// The screen's title and the line under it.
pub(crate) fn header(kind: ScreenKind, level: u32) -> (String, &'static str) {
    match kind {
        ScreenKind::LevelUp => (
            format!("Level {level}"),
            "Choose one. The rarer the card, the stronger it is.",
        ),
        ScreenKind::Cache => (
            "Treasure Cache".to_owned(),
            "Choose a ring, a trinket or an amulet.",
        ),
        ScreenKind::RareCache => (
            "Rare Cache".to_owned(),
            "Free to open: choose a ring, a trinket or an amulet.",
        ),
    }
}

/// The reroll button's label: the free rerolls left, else what the next one costs.
pub(crate) fn reroll_label(free: u32, cost: u32) -> String {
    if free > 0 {
        format!("Reroll ({free} free)")
    } else {
        format!("Reroll ({cost} gold)")
    }
}

/// The line under the cards: the keys that work.
pub(crate) fn keys_hint(cards: usize) -> String {
    match cards {
        0 => "R: reroll".to_owned(),
        1 => "1: choose    R: reroll".to_owned(),
        n => format!("1 to {n}: choose    R: reroll"),
    }
}

// A jewelry text with its `{v}` (shared with the server's item answers).
pub(crate) use esurv_content::text::with_value;

/// One of the offer's numbers by its name.
fn value(offer: &Offer, what: &str) -> Option<f32> {
    offer
        .values
        .iter()
        .find(|v| v.what == what)
        .map(|v| v.value)
}

/// A spell's name: the content's, else its id as words.
fn spell_name(content: Option<&Content>, id: &str) -> String {
    content
        .and_then(|c| c.ability(id))
        .map(|a| a.name.clone())
        .unwrap_or_else(|| crate::meter::server::spell_name(id))
}

fn spell_icon(content: Option<&Content>, id: &str) -> String {
    content
        .and_then(|c| c.ability(id))
        .map_or(UNKNOWN_ICON, |a| a.icon.as_str())
        .to_owned()
}

/// A spell's tooltip lines under its name: its school, and how often it casts itself.
fn spell_tip(content: Option<&Content>, id: &str) -> Vec<Line> {
    let Some(a) = content.and_then(|c| c.ability(id)) else {
        return Vec::new();
    };
    let mut lines = vec![Line::one(format!("{:?} spell", a.school), Tone::Plain)];
    if a.cooldown > 0.0 {
        lines.push(Line::one(
            format!("Casts itself every {} sec", number(a.cooldown)),
            Tone::Now,
        ));
    }
    lines
}

/// The ranks an offer steps: from the rank the kit says (else one under) to the rank it reaches.
fn ranks(now: Option<u32>, reached: f32) -> (u32, u32) {
    let to = reached.round().max(1.0) as u32;
    (now.unwrap_or(to.saturating_sub(1)), to)
}

/// "Rank 2 » 3".
fn rank_step((from, to): (u32, u32)) -> String {
    format!("Rank {from} \u{bb} {to}")
}

/// "RANK 3": the badge of something taken again.
fn rank_badge((_, to): (u32, u32)) -> Option<Badge> {
    badge(format!("RANK {to}"), true)
}

/// The numbers a new spell's card fills its description with: the server's, for the hero
/// taking it; without them (a made-up screen, an older server), the run's own rules for a hero
/// of the spell's class at `level` with nothing else.
pub(crate) fn offered_numbers(
    offer: &Offer,
    spell: &Ability,
    content: &Content,
    level: u32,
) -> Vec<OfferValue> {
    if !offer.numbers.is_empty() {
        return offer.numbers.clone();
    }
    let Some(class) = content
        .classes
        .iter()
        .find(|c| c.abilities.contains(&spell.id) || c.starting_options.iter().any(|o| o.spells.contains(&spell.id)))
        .or(content.classes.first())
    else {
        return Vec::new();
    };
    let rarity = RARITIES[usize::from(offer.rarity.min(4))];
    new_spell_numbers(spell, rarity, &[], class.spell_damage_scale, &compute(class, level.max(1), &[]))
        .into_iter()
        .map(|(what, value)| OfferValue {
            what: what.into(),
            value,
        })
        .collect()
}

/// A spell's description filled with `numbers` (a number it lacks reads "?"), as one line in
/// plain words.
pub(crate) fn description(spell: &Ability, numbers: &[OfferValue]) -> Option<Line> {
    if spell.description.trim().is_empty() {
        return None;
    }
    let filled = fill(&spell.description, |name| {
        Some(spell_shown(spell, numbers, name).unwrap_or_else(|| Shown::new("?", false)))
    })
    .ok()?;
    Some(filled_line(&filled, Tone::Plain))
}

/// One placeholder of a spell description as it reads: `{school}` its school's name, else the
/// number by that name in the way its kind reads.
pub(crate) fn spell_shown(spell: &Ability, numbers: &[OfferValue], name: &str) -> Option<Shown> {
    if name == SCHOOL {
        return Some(Shown::new(format!("{:?}", spell.school), false));
    }
    let kind = spell_number_kind(name)?;
    let v = numbers.iter().find(|n| n.what == name)?.value;
    Some(Shown::number(kind, v))
}

/// What a card says for `offer`, for a hero with `kit` on a screen of `level`; `content` gives
/// names, icons, texts and words (without it, ids).
pub(crate) fn card_text(
    offer: &Offer,
    kit: &Kit,
    content: Option<&Content>,
    level: u32,
) -> CardText {
    let rarity = offer.rarity.min(4);
    let words = content.map(|c| &c.words);
    let ability = content.and_then(|c| c.ability(&offer.id));
    let gains = |lines: &mut Vec<Line>, words: &Words| {
        for v in &offer.values {
            if let Some(kind) = upgrade_named(&v.what) {
                lines.push(upgrade_line(words, kind, v.value, ability));
            }
        }
    };
    let mut lines = Vec::new();
    let mut replaces = None;
    let mut tip = Vec::new();
    let (icon, name, badge_, subtitle, footer) = match offer.kind {
        OfferKind::NewSpell => {
            // the spell written out, then its head start
            if let (Some(a), Some(c)) = (ability, content) {
                lines.extend(description(a, &offered_numbers(offer, a, c, level)));
            }
            if let Some(w) = words {
                gains(&mut lines, w);
            }
            tip = spell_tip(content, &offer.id);
            (
                spell_icon(content, &offer.id),
                spell_name(content, &offer.id),
                badge("NEW", false),
                "New spell".to_owned(),
                format!(
                    "Takes a spell slot \u{b7} {}/{MAX_SPELLS}",
                    kit.spells.len()
                ),
            )
        }
        OfferKind::UpgradeSpell => {
            // only what it improves
            if let Some(w) = words {
                gains(&mut lines, w);
            }
            tip = spell_tip(content, &offer.id);
            let now = kit.spells.iter().find(|s| s.id == offer.id).map(|s| s.rank);
            let step = ranks(now, value(offer, "rank").unwrap_or(0.0));
            (
                spell_icon(content, &offer.id),
                spell_name(content, &offer.id),
                rank_badge(step),
                rank_step(step),
                "Improves a spell you know".to_owned(),
            )
        }
        OfferKind::NewJewelry | OfferKind::UpgradeJewelry => {
            let item = content.and_then(|c| c.jewelry.iter().find(|j| j.id == offer.id));
            let slot_word = match item.map(|j| j.slot) {
                Some(JewelrySlot::Ring) => "ring",
                Some(JewelrySlot::Trinket) => "trinket",
                None => "jewelry",
            };
            let text = item;
            let worth = value(offer, "value").unwrap_or(0.0);
            let (badge_, subtitle, footer) = if offer.kind == OfferKind::NewJewelry {
                let worn = value(offer, "worn").unwrap_or(0.0).round() as u32;
                lines.extend(text.map(|j| jewelry_line(j, None, worth)));
                (
                    badge(slot_word.to_uppercase(), false),
                    capitalised(slot_word),
                    format!("Takes a {slot_word} slot \u{b7} {worn}/{JEWELRY_SLOTS}"),
                )
            } else {
                let step = value(offer, "step").unwrap_or(0.0);
                lines.extend(text.map(|j| jewelry_line(j, Some(worth), worth + step)));
                let now = kit
                    .jewelry
                    .iter()
                    .find(|w| w.id == offer.id)
                    .map(|w| w.rank);
                let step = ranks(now, value(offer, "rank").unwrap_or(0.0));
                (
                    rank_badge(step),
                    rank_step(step),
                    format!("Upgrades a {slot_word} you wear"),
                )
            };
            let (icon, name) = item.map_or_else(
                || {
                    (
                        UNKNOWN_ICON.to_owned(),
                        crate::meter::server::spell_name(&offer.id),
                    )
                },
                |j| (j.icon.clone(), j.name.clone()),
            );
            (icon, name, badge_, subtitle, footer)
        }
        OfferKind::Amulet => {
            let a = amulet(content, &offer.id, &offer.name, 1.0);
            lines = a.lines;
            let footer = match &kit.amulet {
                Some(worn) => {
                    let worn = worn_amulet(content, worn);
                    let footer = format!("Replaces {}", worn.name);
                    replaces = Some(worn);
                    footer
                }
                None => "Takes the amulet slot".to_owned(),
            };
            (
                a.icon,
                a.name,
                badge("AMULET", false),
                "Amulet".to_owned(),
                footer,
            )
        }
        OfferKind::Empower => {
            let a = amulet(content, &offer.id, &offer.name, 1.0);
            if let Some(factor) = value(offer, "factor") {
                lines.push(Line(vec![
                    ("Its effects grow to ".to_owned(), Tone::Plain),
                    (percent(factor), Tone::Gain),
                ]));
            }
            let worn = kit.amulet.as_ref().filter(|w| w.id == offer.id);
            if let Some(w) = worn {
                lines.push(Line::one(format!("Now: {}", percent(w.value)), Tone::Now));
            }
            tip = a.lines;
            let step = ranks(worn.map(|w| w.rank), value(offer, "rank").unwrap_or(0.0));
            (
                a.icon,
                a.name,
                rank_badge(step),
                rank_step(step),
                "Empowers your amulet".to_owned(),
            )
        }
        OfferKind::Gold => {
            let gold = value(offer, "gold").unwrap_or(0.0).round() as u32;
            lines.push(Line(vec![
                (format!("+{gold}"), Tone::Gain),
                (" gold".to_owned(), Tone::Plain),
            ]));
            (
                GOLD_ICON.to_owned(),
                format!("{gold} Gold"),
                None,
                "Gold".to_owned(),
                "Nothing left to learn".to_owned(),
            )
        }
    };
    CardText {
        icon,
        name,
        rarity,
        badge: badge_,
        subtitle,
        lines,
        footer,
        replaces,
        tip,
    }
}

/// "Ring" for "ring".
fn capitalised(word: &str) -> String {
    let mut c = word.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// An amulet's icon, name and lines by its id, its effects at `factor` (1 when first worn); a
/// remade legendary (`legendary:<spell>`) is built from its recipe, as the run builds it, and
/// says what it does to the spell.
fn amulet(content: Option<&Content>, id: &str, name: &str, factor: f32) -> PieceText {
    let mut piece = PieceText {
        icon: UNKNOWN_ICON.to_owned(),
        name: if name.is_empty() {
            crate::meter::server::spell_name(id)
        } else {
            name.to_owned()
        },
        rarity: None,
        rank: 1,
        lines: Vec::new(),
    };
    if let Some(spell) = id.strip_prefix("legendary:") {
        let ability = content.and_then(|c| c.ability(spell));
        let spell_name = spell_name(content, spell);
        if name.is_empty() {
            piece.name = format!("Legendary {spell_name}");
        }
        piece.rarity = Some(4);
        piece.lines.push(Line::one(
            format!("Remakes your {spell_name}:"),
            Tone::Plain,
        ));
        // its own words say what it does (its rules are too many kinds for lines of numbers)
        let _ = ability;
        if let Some(l) = content.and_then(|c| legendary_named(c, spell)) {
            piece.icon = l.icon.clone();
            piece.rarity = Some(l.rarity.index());
            if name.is_empty() {
                piece.name = l.name.clone();
            }
            piece.lines.push(Line::one(l.text.clone(), Tone::Gain));
            if factor > 1.0 {
                piece.lines.push(Line::one(format!("Its effects at {:.0}%", factor * 100.0), Tone::Gain));
            }
        }
        return piece;
    }
    let Some(content) = content else {
        return piece;
    };
    if let Some(a) = content.amulets.iter().find(|a| a.id == id) {
        piece.icon = a.icon.clone();
        piece.name = a.name.clone();
        piece.rarity = Some(a.rarity.index());
        let effects: Vec<_> = a.effects.iter().map(|e| empowered(e, factor)).collect();
        piece.lines = effect_lines(&content.words, &effects, None);
    }
    piece
}

/// The worn amulet as a side panel and a card it would be replaced on show it, with its tooltip.
fn worn_amulet(content: Option<&Content>, worn: &Worn) -> PieceText {
    let mut piece = amulet(content, &worn.id, &worn.name, worn.value);
    piece.rank = worn.rank;
    if worn.rank > 1 {
        piece.lines.push(Line::one(
            format!("Rank {}: its effects at {}", worn.rank, percent(worn.value)),
            Tone::Now,
        ));
    }
    piece
}

/// The side panels: the hero's spells, rings, trinkets and amulet.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct KitText {
    pub spells: Vec<PieceText>,
    pub rings: Vec<PieceText>,
    pub trinkets: Vec<PieceText>,
    pub amulet: Option<PieceText>,
}

/// What the side panels say of `kit`.
pub(crate) fn kit_text(kit: &Kit, content: Option<&Content>) -> KitText {
    let mut out = KitText {
        spells: kit
            .spells
            .iter()
            .map(|s| PieceText {
                icon: spell_icon(content, &s.id),
                name: spell_name(content, &s.id),
                rarity: None,
                rank: s.rank,
                lines: spell_tip(content, &s.id),
            })
            .collect(),
        amulet: kit.amulet.as_ref().map(|a| worn_amulet(content, a)),
        ..KitText::default()
    };
    for w in &kit.jewelry {
        let item = content.and_then(|c| c.jewelry.iter().find(|j| j.id == w.id));
        let piece = PieceText {
            icon: item.map_or(UNKNOWN_ICON, |j| j.icon.as_str()).to_owned(),
            name: item.map_or_else(
                || crate::meter::server::spell_name(&w.id),
                |j| j.name.clone(),
            ),
            rarity: None,
            rank: w.rank,
            lines: item
                .map(|j| vec![jewelry_line(j, None, w.value)])
                .unwrap_or_default(),
        };
        match item.map(|j| j.slot) {
            Some(JewelrySlot::Trinket) => out.trinkets.push(piece),
            _ => out.rings.push(piece),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_content::text::{plain, SPELL_NUMBERS};
    use esurv_protocol::KnownSpell;
    use esurv_sim::game::offers::Rarity;
    use esurv_sim::game::spell_numbers::spell_numbers;
    use esurv_sim::game::spells::SpellSlot;
    use esurv_sim::game::upgrades::{apply, offerable};

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    fn offer(kind: OfferKind, id: &str, rarity: u8, values: &[(&str, f32)]) -> Offer {
        Offer {
            kind,
            id: id.into(),
            name: String::new(),
            rarity,
            values: values
                .iter()
                .map(|(what, value)| OfferValue {
                    what: (*what).into(),
                    value: *value,
                })
                .collect(),
            numbers: Vec::new(),
        }
    }

    fn texts(card: &CardText) -> Vec<String> {
        card.lines.iter().map(Line::text).collect()
    }

    fn kit(spells: &[(&str, u32)]) -> Kit {
        Kit {
            spells: spells
                .iter()
                .map(|(id, rank)| KnownSpell {
                    id: (*id).into(),
                    rank: *rank,
                })
                .collect(),
            ..Kit::default()
        }
    }

    /// Every spell's description fills from its own numbers, for every rank up to the tenth (a
    /// card for each kind it can take, in turn) and for a hero with every bonus that reaches a
    /// cast: a placeholder a spell has no number for fails here.
    #[test]
    fn every_spell_description_fills_at_every_rank() {
        let c = content();
        let racial: Vec<&str> = c.races.iter().map(|r| r.spell.as_str()).collect();
        let class = &c.classes[0];
        let plain_hero = compute(class, 1, &[]);
        let mut strong_hero = compute(class, 30, &[]);
        strong_hero.extra_targets = 3;
        strong_hero.area = 1.5;
        strong_hero.buff_power = 0.4;
        for a in &c.abilities {
            let mut slot = if racial.contains(&a.id.as_str()) {
                SpellSlot::racial(a, &[])
            } else {
                SpellSlot::new(a, &[])
            };
            for rank in 1..=10u32 {
                for stats in [&plain_hero, &strong_hero] {
                    let numbers: Vec<OfferValue> = spell_numbers(a, &slot.instance(), stats)
                        .into_iter()
                        .map(|(what, value)| OfferValue {
                            what: what.into(),
                            value,
                        })
                        .collect();
                    for n in &numbers {
                        assert!(
                            SPELL_NUMBERS.iter().any(|(name, _)| *name == n.what),
                            "{}: the run gives a number no template can name: {}",
                            a.id,
                            n.what
                        );
                    }
                    let filled = fill(&a.description, |name| spell_shown(a, &numbers, name))
                        .unwrap_or_else(|e| panic!("{} at rank {rank}: {e}", a.id));
                    let text = plain(&filled);
                    assert!(
                        !text.contains('{') && !text.contains('}'),
                        "{}: {text}",
                        a.id
                    );
                }
                let kinds = offerable(a, &slot.upgrades, true, true);
                let Some(&kind) = kinds.get(rank as usize % kinds.len().max(1)) else {
                    break;
                };
                apply(&mut slot.upgrades, &[kind], Rarity::Rare);
            }
        }
    }

    #[test]
    fn a_new_spell_is_written_out_with_its_head_start_under_it() {
        let c = content();
        let card = card_text(
            &offer(OfferKind::NewSpell, "fireball", 3, &[("power", 0.34)]),
            &kit(&[("frostbolt", 1), ("arcane_missiles", 2)]),
            Some(&c),
            1,
        );
        assert_eq!(card.name, "Spell 133");
        assert_eq!(card.icon, c.ability("fireball").unwrap().icon);
        assert_eq!(
            (card.rarity_name(), card.subtitle.as_str()),
            ("Epic", "New spell")
        );
        assert_eq!(
            card.badge,
            Some(Badge {
                text: "NEW".into(),
                again: false
            })
        );
        assert_eq!(card.footer, "Takes a spell slot \u{b7} 2/6");
        assert_eq!(card.lines.len(), 2, "{:?}", texts(&card));
        // the description in plain words, its numbers those of an epic card's fireball
        let fireball = c.ability("fireball").unwrap();
        let hit = (fireball.damage * 1.35).round();
        assert!(card.lines[0].text().contains(&format!("{hit} Fire damage")));
        assert!(card.lines[0].0.iter().all(|(_, t)| *t == Tone::Plain));
        assert_eq!(card.lines[1].text(), "+34% damage");
        assert_eq!(card.lines[1].0[0], ("+34%".to_owned(), Tone::Gain));
        assert_eq!(card.tip[0].text(), "Fire spell");
        // a common card: the description alone; the server's numbers win over the rules'
        let mut common = offer(OfferKind::NewSpell, "fireball", 0, &[]);
        common.numbers = vec![OfferValue {
            what: "damage".into(),
            value: 999.0,
        }];
        let card = card_text(&common, &Kit::default(), Some(&c), 1);
        assert_eq!(card.lines.len(), 1);
        assert!(card.lines[0].text().contains("999 Fire damage"));
    }

    #[test]
    fn an_upgrade_says_only_what_it_improves() {
        let card = card_text(
            &offer(
                OfferKind::UpgradeSpell,
                "fireball",
                4,
                &[("rank", 5.0), ("power", 0.65), ("targets", 2.0)],
            ),
            &kit(&[("fireball", 3)]),
            Some(&content()),
            1,
        );
        assert_eq!(
            (card.rarity_name(), card.subtitle.as_str()),
            ("Legendary", "Rank 3 \u{bb} 5"),
            "a legendary card steps two kinds, two ranks"
        );
        assert_eq!(
            card.badge,
            Some(Badge {
                text: "RANK 5".into(),
                again: true
            })
        );
        assert_eq!(card.footer, "Improves a spell you know");
        assert_eq!(texts(&card), ["+65% damage", "+2 targets"]);
        let cooldown = card_text(
            &offer(
                OfferKind::UpgradeSpell,
                "fireball",
                1,
                &[("rank", 2.0), ("cooldown", 0.1)],
            ),
            &Kit::default(),
            Some(&content()),
            1,
        );
        assert_eq!(cooldown.subtitle, "Rank 1 \u{bb} 2");
        assert_eq!(texts(&cooldown), ["-10% cooldown"]);
    }

    #[test]
    fn jewelry_shows_its_line_or_its_value_before_and_after() {
        let c = content();
        let new = card_text(
            &offer(
                OfferKind::NewJewelry,
                "accuria",
                1,
                &[("value", 0.357), ("worn", 1.0)],
            ),
            &Kit::default(),
            Some(&c),
            1,
        );
        assert_eq!(new.name, "Band of Accuria");
        assert_eq!(
            (new.rarity_name(), new.subtitle.as_str()),
            ("Uncommon", "Ring")
        );
        assert_eq!(
            new.badge,
            Some(Badge {
                text: "RING".into(),
                again: false
            })
        );
        assert_eq!(new.footer, "Takes a ring slot \u{b7} 1/2");
        assert_eq!(texts(&new), ["+35.7% damage"]);
        assert_eq!(new.lines[0].0[0], ("+35.7%".to_owned(), Tone::Gain));
        let worn = Kit {
            jewelry: vec![Worn {
                id: "accuria".into(),
                name: String::new(),
                rank: 1,
                value: 0.4,
            }],
            ..Kit::default()
        };
        let up = card_text(
            &offer(
                OfferKind::UpgradeJewelry,
                "accuria",
                3,
                &[("rank", 2.0), ("step", 0.08), ("value", 0.4)],
            ),
            &worn,
            Some(&c),
            1,
        );
        assert_eq!(up.subtitle, "Rank 1 \u{bb} 2");
        assert_eq!(
            up.badge,
            Some(Badge {
                text: "RANK 2".into(),
                again: true
            })
        );
        assert_eq!(up.footer, "Upgrades a ring you wear");
        assert_eq!(texts(&up), ["+40% \u{bb} +48% damage"]);
    }

    #[test]
    fn an_amulet_shows_its_effects_and_the_one_it_replaces() {
        let c = content();
        let plain = card_text(
            &offer(OfferKind::Amulet, "star_of_mystaria", 0, &[]),
            &Kit::default(),
            Some(&c),
            1,
        );
        assert_eq!(plain.name, "Star of Mystaria");
        assert_eq!(plain.footer, "Takes the amulet slot");
        assert_eq!(
            plain.badge,
            Some(Badge {
                text: "AMULET".into(),
                again: false
            })
        );
        assert_eq!(texts(&plain), ["+25% max health", "+4% damage reduction"]);
        assert!(plain.replaces.is_none());
        let wearing = Kit {
            amulet: Some(Worn {
                id: "mark_of_fordring".into(),
                name: "Mark of Fordring".into(),
                rank: 2,
                value: 1.3,
            }),
            ..Kit::default()
        };
        let card = card_text(
            &offer(OfferKind::Amulet, "steadfast_might", 2, &[]),
            &wearing,
            Some(&c),
            1,
        );
        assert_eq!(card.footer, "Replaces Mark of Fordring");
        // the rule first, its number green; then the stat lines
        let lines = texts(&card);
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("100%"), "{lines:?}");
        assert!(card.lines[0].0.contains(&("100%".to_owned(), Tone::Gain)));
        assert_eq!(&lines[1..], ["+10% damage reduction", "+15% max health"]);
        let worn = card.replaces.unwrap();
        assert_eq!((worn.name.as_str(), worn.rank), ("Mark of Fordring", 2));
        assert_eq!(worn.rarity, Some(0));
        // the worn one's effects at its strength now
        assert_eq!(worn.lines[0].text(), "+19.5% damage");
        assert_eq!(worn.lines[1].text(), "Rank 2: its effects at 130%");
    }

    #[test]
    fn a_legendary_amulet_says_what_it_does_to_the_spell() {
        let c = content();
        let mut legendary = offer(OfferKind::Amulet, "legendary:fireball", 4, &[]);
        legendary.name = "Phoenix".into();
        let card = card_text(&legendary, &Kit::default(), Some(&c), 1);
        assert_eq!(card.name, "Phoenix");
        assert_eq!(
            (card.rarity_name(), card.subtitle.as_str()),
            ("Legendary", "Amulet")
        );
        let l = legendary_named(&c, "fireball").unwrap();
        assert_eq!(card.icon, l.icon);
        assert_eq!(
            card.lines[0],
            Line::one("Remakes your Spell 133:", Tone::Plain)
        );
        // then its own words
        assert_eq!(card.lines[1], Line::one(l.text.clone(), Tone::Gain));
        assert_eq!(card.lines.len(), 2);
    }

    #[test]
    fn an_empowerment_names_its_ranks_and_the_new_strength() {
        let c = content();
        let wearing = Kit {
            amulet: Some(Worn {
                id: "mark_of_fordring".into(),
                name: "Mark of Fordring".into(),
                rank: 1,
                value: 1.0,
            }),
            ..Kit::default()
        };
        let empower = card_text(
            &offer(
                OfferKind::Empower,
                "mark_of_fordring",
                0,
                &[("rank", 2.0), ("factor", 1.3)],
            ),
            &wearing,
            Some(&c),
            1,
        );
        assert_eq!(empower.subtitle, "Rank 1 \u{bb} 2");
        assert_eq!(
            empower.badge,
            Some(Badge {
                text: "RANK 2".into(),
                again: true
            })
        );
        assert_eq!(empower.footer, "Empowers your amulet");
        assert_eq!(texts(&empower), ["Its effects grow to 130%", "Now: 100%"]);
    }

    #[test]
    fn the_gold_card_says_there_was_nothing_left() {
        let card = card_text(
            &offer(OfferKind::Gold, "", 0, &[("gold", 25.0)]),
            &Kit::default(),
            None,
            1,
        );
        assert_eq!(card.name, "25 Gold");
        assert_eq!(card.icon, GOLD_ICON);
        assert_eq!(card.footer, "Nothing left to learn");
        assert_eq!(card.badge, None);
    }

    #[test]
    fn without_content_ids_read_as_words() {
        let card = card_text(
            &offer(OfferKind::NewSpell, "my_new_spell", 0, &[]),
            &Kit::default(),
            None,
            1,
        );
        assert_eq!(card.name, "My New Spell");
        assert_eq!(card.icon, UNKNOWN_ICON);
        assert_eq!(card.rarity_name(), "Common");
        assert!(card.lines.is_empty());
    }

    #[test]
    fn the_side_panels_sort_the_worn_pieces() {
        let c = content();
        let trinket = c
            .jewelry
            .iter()
            .find(|j| j.slot == JewelrySlot::Trinket)
            .unwrap();
        let k = Kit {
            spells: vec![KnownSpell {
                id: "fireball".into(),
                rank: 3,
            }],
            jewelry: vec![
                Worn {
                    id: "accuria".into(),
                    name: String::new(),
                    rank: 2,
                    value: 0.4,
                },
                Worn {
                    id: trinket.id.clone(),
                    name: String::new(),
                    rank: 1,
                    value: trinket.value,
                },
            ],
            amulet: None,
        };
        let t = kit_text(&k, Some(&c));
        assert_eq!(
            (t.spells[0].name.as_str(), t.spells[0].rank),
            ("Spell 133", 3)
        );
        assert_eq!(t.rings[0].lines[0].text(), "+40% damage");
        assert_eq!(t.trinkets[0].name, trinket.name);
        assert!(t.amulet.is_none());
    }

    #[test]
    fn numbers_and_values_read_plainly() {
        assert_eq!(percent(0.46), "46%");
        assert_eq!(percent(0.065), "6.5%");
        assert_eq!(number(2.0), "2");
        assert_eq!(
            with_value("+{v} health per second", 1.5),
            "+1.5 health per second"
        );
        assert_eq!(
            with_value("{v}% chance for a spell to cast twice", 0.06),
            "6% chance for a spell to cast twice"
        );
        assert_eq!(reroll_label(3, 0), "Reroll (3 free)");
        assert_eq!(reroll_label(0, 300), "Reroll (300 gold)");
        assert_eq!(keys_hint(3), "1 to 3: choose    R: reroll");
        assert_eq!(header(ScreenKind::LevelUp, 7).0, "Level 7");
        assert_eq!(header(ScreenKind::RareCache, 7).0, "Rare Cache");
    }
}

/// The last text check of every spell's tooltip and every upgrade card's line: what a card and a
/// tooltip show, filled from the run's own numbers, reads whole and in the right number.
#[cfg(test)]
mod text_audit {
    use super::*;
    use esurv_content::{Fill, PassiveKind, Resource};
    use esurv_content::text::{percent, tenths, Filled};
    use esurv_content::Upgrade;
    use esurv_sim::game::offers::Rarity;
    use esurv_sim::game::spell_numbers::{spell_numbers, upgrade_gain};
    use esurv_sim::game::spells::SpellSlot;
    use esurv_sim::game::upgrades::{apply, offerable};

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    /// Words a count of one must not be followed by.
    const PLURALS: &[&str] = &[
        "targets", "enemies", "times", "stacks", "imps", "pets", "critters", "charges", "strikes", "hits", "missiles",
        "bolts", "points", "creatures", "plants", "souls", "shards", "embers",
    ];

    /// Every problem of one filled description: a number that reads empty, zero or not at all,
    /// and a count of one with a plural after it.
    fn problems(filled: &[Filled]) -> Vec<String> {
        let mut out = Vec::new();
        for (i, f) in filled.iter().enumerate() {
            let Filled::Value(v) = f else { continue };
            if v.is_empty() || v == "0" || v == "0%" || v.contains("NaN") || v.contains("inf") || v == "?" {
                out.push(format!("the number {v:?}"));
            }
            if v == "1" {
                if let Some(Filled::Words(next)) = filled.get(i + 1) {
                    // the noun may come a word later ("1 little imps")
                    let word = next.split_whitespace().take(2).map(|w| w.trim_matches(|c: char| !c.is_alphabetic())).find(|w| PLURALS.contains(w));
                    if let Some(word) = word {
                        out.push(format!("\"1 {word}\""));
                    }
                }
            }
        }
        out
    }

    /// Every spell's description, at rank 1 and after each of ten cards (each kind it can take in
    /// turn, at three rarities), for a hero of its class with nothing and with extra targets:
    /// every placeholder fills, no number reads 0 or empty, no "1 targets"; each upgrade line
    /// names its gain as more than nothing, in the singular or the plural it needs.
    #[test]
    fn every_tooltip_and_upgrade_line_reads_whole() {
        let c = content();
        let racial: Vec<&str> = c.races.iter().map(|r| r.spell.as_str()).collect();
        let mut found = Vec::new();
        for a in &c.abilities {
            let class = c
                .classes
                .iter()
                .find(|k| k.abilities.contains(&a.id) || k.starting_options.iter().any(|o| o.spells.contains(&a.id)))
                .unwrap_or(&c.classes[0]);
            let plain_hero = compute(class, 1, &[]);
            let mut strong_hero = compute(class, 30, &[]);
            strong_hero.extra_targets = 3;
            for rarity in [Rarity::Common, Rarity::Epic, Rarity::Legendary] {
                let slot = if racial.contains(&a.id.as_str()) { SpellSlot::racial(a, &[]) } else { SpellSlot::new(a, &[]) };
                let mut slot = slot.with_damage_scale(class.spell_damage_scale);
                for card in 0..=10usize {
                    for stats in [&plain_hero, &strong_hero] {
                        let numbers: Vec<OfferValue> = spell_numbers(a, &slot.instance(), stats)
                            .into_iter()
                            .map(|(what, value)| OfferValue { what: what.into(), value })
                            .collect();
                        match fill(&a.description, |n| spell_shown(a, &numbers, n)) {
                            Err(e) => found.push(format!("{}: {e}", a.id)),
                            Ok(filled) => {
                                for p in problems(&filled) {
                                    found.push(format!("{} after {card} cards: {p}", a.id));
                                }
                            }
                        }
                    }
                    for &kind in &a.upgrades {
                        let by = upgrade_gain(a, &slot.upgrades, kind, rarity);
                        if by <= 0.0 {
                            continue;
                        }
                        let line = upgrade_line(&c.words, kind, by, Some(a)).text();
                        let words = line.split_once(' ').map_or("", |(_, w)| w);
                        let last = words.rsplit(' ').next().unwrap_or("");
                        if words.is_empty() || line.starts_with("+0") || line.starts_with("-0") {
                            found.push(format!("{} {kind:?}: {line:?}", a.id));
                        }
                        if kind.is_count() && (line.starts_with("+1 ") == PLURALS.contains(&last)) {
                            found.push(format!("{} {kind:?}: {line:?}", a.id));
                        }
                    }
                    let kinds = offerable(a, &slot.upgrades, true, true);
                    let Some(&kind) = kinds.get(card % kinds.len().max(1)) else { break };
                    apply(&mut slot.upgrades, &[kind], rarity);
                }
            }
        }
        found.dedup();
        assert!(found.is_empty(), "{} problems:\n{}", found.len(), found.join("\n"));
    }

    /// Each spell lists only the upgrade kinds that change one of its numbers: a cooldown to cut,
    /// an area to widen, targets to add, a duration to stretch, damage or a heal to raise.
    #[test]
    fn every_listed_upgrade_has_something_to_change() {
        let c = content();
        let mut found = Vec::new();
        for a in &c.abilities {
            let n = spell_numbers(a, &SpellSlot::new(a, &[]).instance(), &compute(&c.classes[0], 1, &[]));
            let has = |name: &str| n.iter().any(|(k, _)| *k == name);
            for &kind in &a.upgrades {
                let ok = match kind {
                    Upgrade::Cooldown => a.cooldown > 0.0,
                    Upgrade::Area => has("radius"),
                    Upgrade::Targets => has("targets"),
                    Upgrade::Duration => has("duration"),
                    Upgrade::Power => ["damage", "heal", "shield", "aura"].iter().any(|k| has(k)),
                    Upgrade::Stacks => has("stacks"),
                    Upgrade::Strikes => has("hits"),
                    Upgrade::Chance => has("chance"),
                    Upgrade::Imps | Upgrade::Critters => has("count"),
                    _ => true,
                };
                if !ok {
                    found.push(format!("{} lists {kind:?}", a.id));
                }
            }
        }
        // Mutilate's Duration cards lengthen its poison, which no number of its own shows
        found.retain(|f| f != "mutilate lists Duration");
        assert!(found.is_empty(), "{}", found.join("\n"));
    }

    /// A starting option's passive says the numbers its rules play: the energy tick, the rage a
    /// swing and lost health give, a counter's fill per cast and its size, and a proc's chance.
    #[test]
    fn every_passive_names_its_own_numbers() {
        let c = content();
        let mut found = Vec::new();
        for class in &c.classes {
            for o in &class.starting_options {
                let text = &o.passive.description;
                let mut want: Vec<String> = Vec::new();
                match o.resource {
                    Some(Resource::Energy { per_tick, tick_every, cap, .. }) => {
                        want.push(format!("{} every {} sec, up to {}", tenths(per_tick), tenths(tick_every), tenths(cap)));
                    }
                    Some(Resource::Rage { per_swing, per_health_lost, .. }) => {
                        want.push(format!("gives {} rage", tenths(per_swing)));
                        want.push(format!("each {}% of your health lost gives 1", tenths(100.0 / per_health_lost)));
                    }
                    None => {}
                }
                match &o.passive.kind {
                    PassiveKind::Counter(k) => {
                        for f in &k.fill {
                            if let Fill::Cast(n) = f {
                                if *n > 1.0 {
                                    want.push(format!("{} ", tenths(*n)));
                                    want.push(tenths(k.max));
                                }
                            }
                        }
                    }
                    PassiveKind::FreeCasts { chance, .. } | PassiveKind::Orbs { chance, .. } | PassiveKind::SwingSpeedOnHit { chance, .. } => {
                        want.push(format!("{} chance", percent(*chance)));
                    }
                    PassiveKind::Marks { chance, share, .. } => {
                        want.push(format!("{} chance", percent(*chance)));
                        want.push(format!("{} of its damage", percent(*share)));
                    }
                    _ => {}
                }
                for w in want {
                    if !text.contains(&w) {
                        found.push(format!("{} / {}: {w:?} not in {text:?}", class.id, o.id));
                    }
                }
            }
        }
        assert!(found.is_empty(), "{}", found.join("\n"));
    }
}
