//! The card screen's dev aid, off by default: `ESURV_CARDS_SCRIPT=<step>;<step>...` drives
//! the screen inside the game, with no input of the operating system; one step after the other,
//! each once the last has had [`STEP_PAUSE`] seconds to show. A step is:
//!
//! * `screen`: waits for a choice screen from the server, and lets it settle;
//! * `show:<kind>`: opens a made-up screen of the content's pieces, to see the look without
//!   playing up to it: `levelup` (a new spell at common, a new one with a head start, an
//!   upgrade), `cache` (a new ring, trinket and amulet), `rare` (upgrades of the worn trinket
//!   and ring, empowering the amulet), `amulet` (one that replaces the worn amulet, a legendary
//!   one, a trinket upgrade) or `legendary` (three legendary amulets). Its numbers come from the
//!   run's own rules; taking one of its cards asks nothing of the server;
//! * `lit:<n>`: card `n` (from 1) shows as under the pointer; `unlit` ends it;
//! * `key:<1-4|r>`: presses that key of the screen (the screen's own key message);
//! * `click:<n>`: clicks card `n` (from 1), as the pointer let go over it would (the screen's own
//!   click message); `take:upgrade` clicks the first card that upgrades a known spell (rerolling
//!   while a free reroll is left and none does), else the first card;
//! * `close`: closes a made-up screen; until then it stays, over any screen the server opens;
//! * `wait:<seconds>`, `shot:<name>` (the game's own shot of the window, `<name>.png` in
//!   `ESURV_CARDS_SHOTS`, a folder; the window is never raised nor pinned), `quit`.

use bevy::app::AppExit;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use benilla_app::OverlayKeyPressed;
use esurv_content::{
    Ability, Amulet, Content, Jewelry, JewelrySlot, Stat, TalentEffect, Upgrade,
};
use esurv_protocol::{
    Choices, Kit, KnownSpell, Offer, OfferKind, OfferValue, ScreenKind, Worn,
};
use esurv_sim::game::amulets::{empower_factor, legendary_for, legendary_named};
use esurv_sim::game::offers::{
    jewelry_cap, jewelry_upgrade_step, new_jewelry_value, Rarity,
};
use esurv_sim::game::spell_numbers::{head_start_gain, upgrade_gain};
use esurv_sim::game::upgrades::{offerable, SpellUpgrades};

use super::screen::{CardPick, LitByScript};
use super::{CardScreen, OpenCards};
use crate::stock::StockClick;
use crate::ui::GameContent;

/// Seconds a step's result shows before the next step.
const STEP_PAUSE: f32 = 1.5;
/// A screen from the server shows this long before `screen` is done.
const SETTLE: f32 = 1.5;

/// A made-up screen's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MadeUp {
    LevelUp,
    Cache,
    Amulet,
    Legendary,
    Rare,
}

/// One step of a script.
#[derive(Clone, Debug, PartialEq)]
enum Step {
    Screen,
    Show(MadeUp),
    Lit(Option<usize>),
    Key(KeyCode),
    /// Card `n` (from 0) is clicked.
    Click(usize),
    /// The first card that upgrades a known spell is clicked (else the first).
    TakeUpgrade,
    Close,
    Wait(f32),
    Shot(String),
    Quit,
}

/// Reads a script; a step it cannot read is left out, and said.
fn parse(script: &str) -> Vec<Step> {
    script
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| {
            let step = match s.split_once(':') {
                Some(("show", kind)) => Some(Step::Show(match kind.trim() {
                    "levelup" => MadeUp::LevelUp,
                    "cache" => MadeUp::Cache,
                    "amulet" => MadeUp::Amulet,
                    "legendary" => MadeUp::Legendary,
                    "rare" => MadeUp::Rare,
                    _ => return warn_step(s),
                })),
                Some(("lit", n)) => match n.trim().parse::<usize>() {
                    Ok(n) if n >= 1 => Some(Step::Lit(Some(n - 1))),
                    _ => None,
                },
                Some(("key", k)) => match k.trim() {
                    "1" => Some(Step::Key(KeyCode::Digit1)),
                    "2" => Some(Step::Key(KeyCode::Digit2)),
                    "3" => Some(Step::Key(KeyCode::Digit3)),
                    "4" => Some(Step::Key(KeyCode::Digit4)),
                    "r" | "R" => Some(Step::Key(KeyCode::KeyR)),
                    _ => None,
                },
                Some(("click", n)) => match n.trim().parse::<usize>() {
                    Ok(n) if n >= 1 => Some(Step::Click(n - 1)),
                    _ => None,
                },
                Some(("take", "upgrade")) => Some(Step::TakeUpgrade),
                Some(("wait", t)) => t.trim().parse().ok().map(Step::Wait),
                Some(("shot", name)) => Some(Step::Shot(name.trim().into())),
                None if s == "screen" => Some(Step::Screen),
                None if s == "unlit" => Some(Step::Lit(None)),
                None if s == "close" => Some(Step::Close),
                None if s == "quit" => Some(Step::Quit),
                _ => None,
            };
            if step.is_none() {
                return warn_step(s);
            }
            step
        })
        .collect()
}

fn warn_step(s: &str) -> Option<Step> {
    warn!("cards: the script step {s:?} means nothing");
    None
}

#[derive(Resource)]
struct Script {
    steps: Vec<Step>,
    next: usize,
    /// When the current step may run.
    at: f32,
    shots: std::path::PathBuf,
    /// The kit of the last screen the server opened: a made-up screen's hero.
    kit: Option<Kit>,
    level: u32,
    /// The made-up screen shown, kept over a screen the server opens meanwhile.
    showing: Option<OpenCards>,
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<LitByScript>();
    let Ok(script) = std::env::var("ESURV_CARDS_SCRIPT") else {
        return;
    };
    let steps = parse(&script);
    info!(
        "cards: a script of {} steps (ESURV_CARDS_SCRIPT)",
        steps.len()
    );
    let shots = std::env::var("ESURV_CARDS_SHOTS").unwrap_or_else(|_| ".".into());
    app.insert_resource(Script {
        steps,
        next: 0,
        at: 0.0,
        shots: shots.into(),
        kit: None,
        level: 2,
        showing: None,
    })
    .add_systems(Update, run_script.before(super::read_server));
}

#[allow(clippy::too_many_arguments)] // one Bevy system's inputs
fn run_script(
    mut commands: Commands,
    mut script: ResMut<Script>,
    time: Res<Time<Real>>,
    mut screen: ResMut<CardScreen>,
    mut lit: ResMut<LitByScript>,
    content: Option<Res<GameContent>>,
    mut keys: MessageWriter<OverlayKeyPressed>,
    mut clicks: MessageWriter<StockClick>,
    cards: Query<(Entity, &CardPick), With<super::screen::CardFrame>>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    match screen.open.as_ref() {
        Some(open) if open.made_up => script.showing = Some(open.clone()),
        open => {
            if let Some(open) = open {
                script.kit = Some(open.choices.kit.clone());
                script.level = open.choices.level;
            }
            if let Some(shown) = script.showing.clone() {
                screen.open = Some(shown);
            }
        }
    }
    if now < script.at {
        return;
    }
    let Some(step) = script.steps.get(script.next).cloned() else {
        return;
    };
    let mut pause = STEP_PAUSE;
    match step {
        Step::Screen => {
            let settled = screen
                .open
                .as_ref()
                .is_some_and(|o| !o.made_up && now - o.since >= SETTLE);
            if !settled {
                return;
            }
            info!("cards: the server's screen is up");
            pause = 0.0;
        }
        Step::Show(kind) => {
            let Some(content) = content.as_ref() else {
                return;
            };
            let kit = script.kit.clone();
            match made_up(kind, &content.0, kit, script.level) {
                Some(choices) => {
                    info!("cards: a made-up {kind:?} screen");
                    let open = OpenCards {
                        choices,
                        since: now,
                        answered: false,
                        taken: None,
                        made_up: true,
                    };
                    script.showing = Some(open.clone());
                    screen.open = Some(open);
                }
                None => warn!("cards: the content has no pieces for a {kind:?} screen"),
            }
        }
        Step::Lit(n) => lit.0 = n,
        Step::Key(key) => {
            info!("cards: the script presses {key:?}");
            keys.write(OverlayKeyPressed(key));
        }
        Step::Click(n) => match cards.iter().find(|(_, p)| p.0 == n) {
            Some((card, _)) => {
                info!("cards: the script clicks card {}", n + 1);
                clicks.write(StockClick(card));
            }
            None => warn!("cards: no card {} to click", n + 1),
        },
        Step::TakeUpgrade => {
            let Some(open) = screen.open.as_ref().filter(|o| !o.made_up) else {
                return;
            };
            if open.answered {
                // a reroll is out: wait for the new cards
                return;
            }
            let upgrade = open
                .choices
                .offers
                .iter()
                .position(|offer| offer.kind == OfferKind::UpgradeSpell);
            if upgrade.is_none() && open.choices.free_rerolls > 0 {
                info!("cards: no upgrade card; the script rerolls");
                keys.write(OverlayKeyPressed(KeyCode::KeyR));
                script.at = now + STEP_PAUSE;
                return;
            }
            let n = upgrade.unwrap_or(0);
            match cards.iter().find(|(_, p)| p.0 == n) {
                Some((card, _)) => {
                    info!("cards: the script takes card {} (an upgrade if any)", n + 1);
                    clicks.write(StockClick(card));
                }
                None => warn!("cards: no card {} to take", n + 1),
            }
        }
        Step::Close => {
            if script.showing.take().is_some() {
                screen.open = None;
            }
            lit.0 = None;
        }
        Step::Wait(t) => pause = t,
        Step::Shot(name) => {
            let path = script.shots.join(format!("{name}.png"));
            info!("cards: writing {}", path.display());
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
        Step::Quit => {
            info!("cards: the script ends the game");
            exit.write(AppExit::Success);
        }
    }
    script.next += 1;
    script.at = now + pause;
}

fn values(pairs: &[(&str, f32)]) -> Vec<OfferValue> {
    pairs
        .iter()
        .map(|(what, value)| OfferValue {
            what: (*what).into(),
            value: *value,
        })
        .collect()
}

fn offer(kind: OfferKind, id: &str, name: &str, rarity: Rarity, pairs: &[(&str, f32)]) -> Offer {
    Offer {
        kind,
        id: id.into(),
        name: name.into(),
        rarity: rarity as u8,
        values: values(pairs),
        numbers: Vec::new(),
    }
}

/// The spells of the made-up hero: the kit's, else the first class's first three.
fn spells<'c>(content: &'c Content, kit: &Kit) -> Vec<&'c Ability> {
    let known: Vec<&Ability> = kit
        .spells
        .iter()
        .filter_map(|s| content.ability(&s.id))
        .collect();
    if !known.is_empty() {
        return known;
    }
    content
        .classes
        .first()
        .map(|c| {
            c.abilities
                .iter()
                .filter_map(|id| content.ability(id))
                .take(3)
                .collect()
        })
        .unwrap_or_default()
}

/// The legendary amulet that remakes `spell`, as an offer.
fn legendary(content: &Content, spell: &Ability) -> Option<Offer> {
    let made = legendary_for(legendary_named(content, &spell.id)?);
    Some(offer(
        OfferKind::Amulet,
        &made.id,
        &made.name,
        Rarity::Legendary,
        &[],
    ))
}

/// A ring or trinket of the content: of `slot`, giving `stat` if one does, else the first.
fn piece(content: &Content, slot: JewelrySlot, stat: Stat) -> Option<&Jewelry> {
    let mut of_slot = content.jewelry.iter().filter(|j| j.slot == slot);
    of_slot
        .clone()
        .find(|j| j.stat == stat)
        .or_else(|| of_slot.next())
}

/// What a new `j` of `rarity` starts at.
fn new_value(j: &Jewelry, rarity: Rarity) -> f32 {
    let (mult, hard) = jewelry_cap(j.stat);
    new_jewelry_value(j.value, rarity, mult, hard)
}

/// A worn ring or trinket as a first rarity-`rarity` pick left it.
fn worn(j: &Jewelry, rarity: Rarity) -> Worn {
    Worn {
        id: j.id.clone(),
        name: String::new(),
        rank: 1,
        value: new_value(j, rarity),
    }
}

/// An upgrade card of `rarity` for the worn piece `w` of `j`.
fn jewelry_upgrade(j: &Jewelry, w: &Worn, rarity: Rarity) -> Offer {
    offer(
        OfferKind::UpgradeJewelry,
        &j.id,
        "",
        rarity,
        &[
            ("rank", (w.rank + 1) as f32),
            ("step", {
                let (mult, hard) = jewelry_cap(j.stat);
                jewelry_upgrade_step(j.value, w.value, w.rank, rarity, mult, hard)
            }),
            ("value", w.value),
        ],
    )
}

/// A new ring or trinket card of `rarity`.
fn new_jewelry(j: &Jewelry, rarity: Rarity, worn: u32) -> Offer {
    offer(
        OfferKind::NewJewelry,
        &j.id,
        "",
        rarity,
        &[("value", new_value(j, rarity)), ("worn", worn as f32)],
    )
}

fn amulet_offer(a: &Amulet) -> Offer {
    offer(
        OfferKind::Amulet,
        &a.id,
        &a.name,
        a.rarity,
        &[],
    )
}

/// A made-up screen of `kind` for a hero with `kit` (else a hero of the first class), from the
/// content's pieces and the run's own rules for their numbers:
///
/// * `levelup`: a new spell at common (written out), a new one with a head start, an upgrade;
/// * `cache`: a new ring, a new trinket and a new amulet, nothing worn;
/// * `rare`: upgrades of the worn trinket and ring, and empowering the worn amulet;
/// * `amulet`: an amulet that replaces the worn one, a legendary one, the worn trinket's upgrade;
/// * `legendary`: three legendary amulets.
fn made_up(kind: MadeUp, content: &Content, kit: Option<Kit>, level: u32) -> Option<Choices> {
    let mut kit = kit.unwrap_or_default();
    let known = spells(content, &kit);
    if kit.spells.is_empty() {
        kit.spells = known
            .iter()
            .map(|a| KnownSpell {
                id: a.id.clone(),
                rank: 1,
            })
            .collect();
    }
    // worn: a damage ring, a critical strike damage trinket, the first amulet
    let ring = piece(content, JewelrySlot::Ring, Stat::Damage)?;
    let trinket = piece(content, JewelrySlot::Trinket, Stat::CritDamage)?;
    let amulet = content.amulets.first()?;
    let ring_worn = worn(ring, Rarity::Uncommon);
    let trinket_worn = worn(trinket, Rarity::Rare);
    let wear = |kit: &mut Kit| {
        kit.jewelry = vec![ring_worn.clone(), trinket_worn.clone()];
        kit.amulet = Some(Worn {
            id: amulet.id.clone(),
            name: amulet.name.clone(),
            rank: 1,
            value: empower_factor(amulet.growth, 1),
        });
    };
    // an amulet with a rule of its own, to show a sentence and its stat lines
    let other_amulet = content
        .amulets
        .iter()
        .find(|a| {
            let rule = a.effects.iter().any(|e| {
                matches!(
                    e,
                    TalentEffect::Milestone(_)
                        | TalentEffect::Stat {
                            stat: Stat::Thorns,
                            ..
                        }
                )
            });
            let stat = a
                .effects
                .iter()
                .any(|e| matches!(e, TalentEffect::Stat { .. }));
            a.id != amulet.id && rule && stat && a.effects.len() > 2
        })
        .or(content.amulets.get(1))?;
    let (screen, offers) = match kind {
        MadeUp::LevelUp => {
            let class = content
                .classes
                .iter()
                .find(|c| kit.spells.iter().any(|s| c.abilities.contains(&s.id)))?;
            let mut new = class
                .abilities
                .iter()
                .filter_map(|id| content.ability(id))
                .filter(|a| !kit.spells.iter().any(|s| s.id == a.id));
            let plain = new.next()?;
            let ahead = new.next()?;
            let head_start: Vec<(&str, f32)> = head_start_gain(ahead, Rarity::Uncommon)
                .map(|(k, by)| vec![(upgrade_word(k), by)])
                .unwrap_or_default();
            let upgraded = known.first()?;
            let fresh = SpellUpgrades::default();
            let kind = *offerable(upgraded, &fresh, true, true).first()?;
            let up = [
                ("rank", (fresh.rank + 1) as f32),
                (
                    upgrade_word(kind),
                    upgrade_gain(upgraded, &fresh, kind, Rarity::Rare),
                ),
            ];
            (
                ScreenKind::LevelUp,
                vec![
                    offer(OfferKind::NewSpell, &plain.id, "", Rarity::Common, &[]),
                    offer(
                        OfferKind::NewSpell,
                        &ahead.id,
                        "",
                        Rarity::Uncommon,
                        &head_start,
                    ),
                    offer(OfferKind::UpgradeSpell, &upgraded.id, "", Rarity::Rare, &up),
                ],
            )
        }
        MadeUp::Cache => {
            kit.jewelry.clear();
            kit.amulet = None;
            let new_ring = piece(content, JewelrySlot::Ring, Stat::CritChance)?;
            let new_trinket = piece(content, JewelrySlot::Trinket, Stat::MoveSpeed)?;
            (
                ScreenKind::Cache,
                vec![
                    new_jewelry(new_ring, Rarity::Legendary, 0),
                    new_jewelry(new_trinket, Rarity::Common, 0),
                    amulet_offer(other_amulet),
                ],
            )
        }
        MadeUp::Rare => {
            wear(&mut kit);
            let empower = offer(
                OfferKind::Empower,
                &amulet.id,
                &amulet.name,
                amulet.rarity,
                &[("rank", 2.0), ("factor", empower_factor(amulet.growth, 2))],
            );
            (
                ScreenKind::RareCache,
                vec![
                    jewelry_upgrade(trinket, &trinket_worn, Rarity::Epic),
                    jewelry_upgrade(ring, &ring_worn, Rarity::Rare),
                    empower,
                ],
            )
        }
        MadeUp::Amulet => {
            wear(&mut kit);
            let legendary = legendary(content, known.first()?)?;
            (
                ScreenKind::Cache,
                vec![
                    amulet_offer(other_amulet),
                    legendary,
                    jewelry_upgrade(trinket, &trinket_worn, Rarity::Epic),
                ],
            )
        }
        MadeUp::Legendary => {
            wear(&mut kit);
            // one for each of the hero's spells, else the content's, of three delivery kinds
            let mut seen = Vec::new();
            let pool = known.iter().copied().chain(content.abilities.iter());
            let offers: Vec<Offer> = pool
                .filter(|a| {
                    let new = !seen.contains(&a.delivery);
                    seen.push(a.delivery);
                    new
                })
                .filter_map(|a| legendary(content, a))
                .take(3)
                .collect();
            (ScreenKind::Cache, offers)
        }
    };
    Some(Choices {
        hero: 0,
        kind: screen,
        level,
        reroll_cost: 0,
        free_rerolls: 3,
        offers,
        kit,
    })
}

/// The name an offer's value gives an upgrade (`power`, `cooldown` ...), as the server's.
fn upgrade_word(u: Upgrade) -> &'static str {
    u.word()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    #[test]
    fn a_script_reads_its_steps() {
        assert_eq!(
            parse("screen; shot:a ;key:1;click:2;show:rare;lit:2;unlit;close;nonsense;show:x;click:0;quit"),
            vec![
                Step::Screen,
                Step::Shot("a".into()),
                Step::Key(KeyCode::Digit1),
                Step::Click(1),
                Step::Show(MadeUp::Rare),
                Step::Lit(Some(1)),
                Step::Lit(None),
                Step::Close,
                Step::Quit,
            ]
        );
    }

    #[test]
    fn every_made_up_screen_is_built_from_the_content() {
        let c = content();
        for kind in [
            MadeUp::LevelUp,
            MadeUp::Cache,
            MadeUp::Amulet,
            MadeUp::Legendary,
            MadeUp::Rare,
        ] {
            let choices = made_up(kind, &c, None, 5).unwrap_or_else(|| panic!("{kind:?}"));
            assert!(choices.offers.len() >= 2, "{kind:?}");
        }
        let rare = made_up(MadeUp::Rare, &c, None, 5).unwrap();
        assert_eq!(rare.kind, ScreenKind::RareCache);
        let levelup = made_up(MadeUp::LevelUp, &c, None, 5).unwrap();
        assert_eq!(levelup.offers.len(), 3);
        assert!(levelup.offers[0].values.is_empty(), "a common new spell");
        assert!(
            !levelup.offers[1].values.is_empty(),
            "a new spell with a head start"
        );
        assert_eq!(levelup.offers[2].kind, OfferKind::UpgradeSpell);
        let cache = made_up(MadeUp::Cache, &c, None, 5).unwrap();
        assert!(cache.kit.amulet.is_none() && cache.kit.jewelry.is_empty());
        let amulet = made_up(MadeUp::Amulet, &c, None, 5).unwrap();
        assert!(amulet.kit.amulet.is_some(), "an amulet to replace");
        assert!(amulet.offers[1].id.starts_with("legendary:"));
    }
}
