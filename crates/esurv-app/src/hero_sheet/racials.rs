//! A race's racials as the hero sheet shows them (the Modify character
//! screen's Racials): the race spell, then each passive, each with its icon and its tooltip. The
//! names are the client's, read through the content's pointers; a passive's icon is its stock
//! racial spell's, read once from the player's own `Spell.dbc` ([`RacialIcons::load`]); without the
//! data it shows a question mark.

use benilla_formats::SpellCatalog;
use bevy::prelude::*;
use esurv_content::{ClientName, Content, NameRef};

/// The icon shown for a passive whose spell's icon is not known.
pub const UNKNOWN_ICON: &str = r"Interface\Icons\INV_Misc_QuestionMark";

/// The spells' icons, when the player's data could be read.
#[derive(Resource, Default)]
pub struct RacialIcons(Option<SpellCatalog>);

impl RacialIcons {
    /// Reads the spell catalog from the player's data folder, if there is one.
    pub fn load() -> Self {
        let Some(dir) = benilla_formats::wow_data() else {
            return Self(None);
        };
        let catalog = benilla_formats::open_chain(&dir)
            .and_then(|mut chain| benilla_formats::load_spell_catalog(&mut chain));
        match catalog {
            Ok(c) => Self(Some(c)),
            Err(e) => {
                warn!("hero sheet: the racials' icons did not load: {e:#}");
                Self(None)
            }
        }
    }

    /// The interface path of spell `id`'s icon (`Interface\Icons\Spell_Nature_Sleep`).
    pub fn icon(&self, id: u32) -> Option<&str> {
        self.0.as_ref()?.get(id)?.icon.as_deref()
    }
}

/// What a racial is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RacialKind {
    /// The race spell, cast by itself in a run.
    Spell,
    Passive,
}

/// One racial as the sheet shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Racial {
    pub kind: RacialKind,
    /// Its icon's interface path.
    pub icon: String,
    /// The client's name for it.
    pub name: String,
    /// The race spell's id in the content (its tooltip's numbers); `None` for a passive.
    pub ability: Option<String>,
    /// A passive's line, in our words.
    pub text: String,
}

/// The `Spell.dbc` id a name points at.
fn spell_id(name: &NameRef) -> Option<u32> {
    match name {
        NameRef::Client(ClientName::Spell(id)) => Some(*id),
        _ => None,
    }
}

/// Race `wow_race`'s racials (its 1.12 id): its spell, then its passives in the content's order.
pub fn racials(content: &Content, icons: &RacialIcons, wow_race: u8) -> Vec<Racial> {
    let Some(race) = content.race_by_wow_id(wow_race) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(a) = content.ability(&race.spell) {
        out.push(Racial {
            kind: RacialKind::Spell,
            icon: format!(r"Interface\Icons\{}", a.icon),
            name: a.name.clone(),
            ability: Some(a.id.clone()),
            text: String::new(),
        });
    }
    for p in &race.passives {
        // the stock racial spell the spellbook shows it as, else its own name's spell
        let id = p.stock_spell_ref.as_ref().and_then(spell_id).or_else(|| spell_id(&p.name_ref));
        let icon = id.and_then(|id| icons.icon(id)).unwrap_or(UNKNOWN_ICON);
        let text = p.text.strip_prefix(&format!("{}: ", p.name)).unwrap_or(&p.text);
        out.push(Racial {
            kind: RacialKind::Passive,
            icon: icon.to_owned(),
            name: p.name.clone(),
            ability: None,
            text: text.to_owned(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero_sheet::gear::tests::content;

    /// Every race shows its spell and its passives, the spell first; a passive without the
    /// player's data shows the question mark.
    #[test]
    fn a_race_shows_its_spell_then_its_passives() {
        let c = content();
        let icons = RacialIcons::default();
        for race in &c.races {
            let shown = racials(c, &icons, race.wow_race_id);
            assert_eq!(shown.len(), 1 + race.passives.len(), "{}", race.id);
            assert_eq!(shown[0].kind, RacialKind::Spell);
            assert_eq!(shown[0].ability.as_deref(), Some(race.spell.as_str()));
            assert!(shown[1..].iter().all(|r| r.kind == RacialKind::Passive && r.icon == UNKNOWN_ICON && !r.text.is_empty()));
        }
        assert!(racials(c, &icons, 99).is_empty());
    }

    /// With the player's data every racial passive has its stock spell's icon.
    #[test]
    fn every_racial_passive_has_its_icon_in_the_players_data() {
        let _ = benilla_formats::wow_data_or_skip!();
        let c = content();
        let icons = RacialIcons::load();
        for race in &c.races {
            for r in racials(c, &icons, race.wow_race_id) {
                assert_ne!(r.icon, UNKNOWN_ICON, "{} {}", race.id, r.name);
            }
        }
    }
}
