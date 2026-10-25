//! What the guide says, tab by tab: short player-facing lines about what the game does. Every
//! number in them is read from the content (`balance.ron`, `prestige.ron`, `jewelry.ron`, ...)
//! or quoted from the rule's own constant in `esurv-sim`, never copied, so the guide follows the
//! balance as it changes.

use esurv_content::{Content, JewelrySlot, Stat, TalentEffect, Upgrade};
use esurv_sim::game::{
    amulets, choices, director, enemy_ai, level, loot, offers, pickups, rares, run, stats, upgrades,
};

use super::GuideTab;
use crate::enemy_glow::Glow;

/// An entry's icon: one of the stock icons by name (`INV_Misc_Coin_01`), or a spell's own icon
/// from the player's `Spell.dbc` (a rune's buff).
#[derive(Clone, Debug, PartialEq)]
pub enum GuideIcon {
    Named(String),
    Spell(u32),
}

/// One entry: its icon, its name (in its glow's colour for an enemy that glows) and its line.
#[derive(Clone, Debug, PartialEq)]
pub struct GuideEntry {
    pub icon: Option<GuideIcon>,
    pub name: String,
    pub glow: Option<Glow>,
    pub text: String,
}

/// A heading over some entries, with a line under it.
#[derive(Clone, Debug, PartialEq)]
pub struct GuideSection {
    pub title: &'static str,
    pub hint: Option<String>,
    pub entries: Vec<GuideEntry>,
}

/// A number as the guide writes it: at most two decimals, none when whole.
fn num(v: f32) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r:.2}").trim_end_matches('0').to_owned()
    }
}

/// A share as a percentage: `0.6` reads `60%`.
fn pct(v: f32) -> String {
    format!("{}%", num(v * 100.0))
}

fn entry(icon: Option<GuideIcon>, name: impl Into<String>, text: impl Into<String>) -> GuideEntry {
    GuideEntry {
        icon,
        name: name.into(),
        glow: None,
        text: text.into(),
    }
}

fn named(icon: &str) -> Option<GuideIcon> {
    Some(GuideIcon::Named(icon.to_owned()))
}

/// The guide's sections on `tab`.
pub fn sections(content: &Content, tab: GuideTab) -> Vec<GuideSection> {
    match tab {
        GuideTab::Stats => stat_sections(content),
        GuideTab::Enemies => enemy_sections(content),
        GuideTab::Upgrades => upgrade_sections(),
        GuideTab::Loot => loot_sections(content),
    }
}

// ---- stats -----------------------------------------------------------------------------------

/// The Stats tab's groups, each stat in exactly one (a test holds every stat the game has to it).
pub const BASIC_STATS: [Stat; 11] = [
    Stat::Damage,
    Stat::CritChance,
    Stat::CritDamage,
    Stat::CooldownRecovery,
    Stat::Area,
    Stat::ExtraTargets,
    Stat::MoveSpeed,
    Stat::PickupRange,
    Stat::Experience,
    Stat::Gold,
    Stat::Luck,
];
pub const DEFENCE_STATS: [Stat; 10] = [
    Stat::MaxHealth,
    Stat::FlatMaxHealth,
    Stat::DamageReduction,
    Stat::HealthPerSecond,
    Stat::Lifesteal,
    Stat::HealPerKill,
    Stat::BlockChance,
    Stat::Thorns,
    Stat::HealAndShieldPower,
    Stat::ExtraLives,
];
pub const SPELL_STATS: [Stat; 9] = [
    Stat::DoubleCast,
    Stat::SpellCooldownRecovery,
    Stat::BuffPower,
    Stat::DotStacks,
    Stat::PetDamage,
    Stat::DamageVsElites,
    Stat::DamageVsCrowdControlled,
    Stat::DamageBelow30Pct,
    Stat::DamageVsAfflicted,
];
pub const WEAPON_STATS: [Stat; 7] = [
    Stat::WeaponDamage,
    Stat::SwingSpeed,
    Stat::SwingCooldownRecovery,
    Stat::WeaponDoubleHit,
    Stat::RagePerSwing,
    Stat::RageWhenHurt,
    Stat::DamageFromReduction,
];

/// A stat's name and what it does.
pub fn stat_text(stat: Stat) -> (&'static str, String) {
    let s = |name, text: &str| (name, text.to_owned());
    match stat {
        Stat::Damage => (
            "Damage",
            format!(
                "Raises all the damage you deal. Each level adds {}.",
                pct(stats::DAMAGE_PER_LEVEL)
            ),
        ),
        Stat::CritChance => (
            "Critical strike chance",
            format!(
                "The chance for a hit to strike critically. You start at {}; past 100% the rest \
                 becomes critical strike damage.",
                pct(stats::BASE_CRIT_CHANCE)
            ),
        ),
        Stat::CritDamage => (
            "Critical strike damage",
            format!(
                "How hard a critical strike hits: you start at {} of a normal hit.",
                pct(stats::BASE_CRIT_DAMAGE)
            ),
        ),
        Stat::CooldownRecovery => s(
            "Cooldown recovery",
            "Your spells come back faster, and your weapon swings come faster too.",
        ),
        Stat::Area => s(
            "Area of effect",
            "Makes your spells' areas larger: novas, ground areas, cones and bouncing chains.",
        ),
        Stat::ExtraTargets => s(
            "Extra targets",
            "Spells that hit several enemies hit this many more.",
        ),
        Stat::MoveSpeed => (
            "Movement speed",
            format!(
                "How fast you run, at most {} of your normal speed.",
                pct(stats::MAX_MOVE_SPEED)
            ),
        ),
        Stat::PickupRange => (
            "Pickup range",
            format!(
                "How near experience and gold must be to fly to you. You start at {} yards.",
                num(stats::BASE_PICKUP_RANGE)
            ),
        ),
        Stat::Experience => s("Experience", "More experience from every orb."),
        Stat::Gold => s("Gold", "More gold from every coin."),
        Stat::Luck => s(
            "Luck",
            "Better odds of rarer cards, on level-ups and in caches.",
        ),
        Stat::MaxHealth => s("Max health", "Raises your maximum health by a share."),
        Stat::FlatMaxHealth => (
            "Base health",
            format!(
                "Health added before the shares of max health multiply it. Each level adds {}.",
                num(stats::HEALTH_PER_LEVEL)
            ),
        ),
        Stat::DamageReduction => (
            "Damage reduction",
            format!(
                "Cuts the damage you take, at most {}. Every class starts with some.",
                pct(stats::MAX_DAMAGE_REDUCTION)
            ),
        ),
        Stat::HealthPerSecond => (
            "Health per second",
            format!(
                "Health you get back every second. You start at {} and gain {} a level.",
                num(stats::BASE_REGEN),
                num(stats::REGEN_PER_LEVEL)
            ),
        ),
        Stat::Lifesteal => s("Lifesteal", "A share of the damage you deal heals you."),
        Stat::HealPerKill => s("Health per kill", "Every kill heals you this much."),
        Stat::BlockChance => (
            "Block chance",
            format!(
                "The chance to block an enemy's melee blow, taking {} less from it.",
                pct(run::BLOCK_REDUCTION)
            ),
        ),
        Stat::Thorns => s(
            "Thorns",
            "Whatever strikes you in melee takes this share of the blow back.",
        ),
        Stat::HealAndShieldPower => s(
            "Heal and shield power",
            "Makes your spells' heals and shields stronger. Health per second, lifesteal and \
             health per kill stay as they are.",
        ),
        Stat::ExtraLives => (
            "Extra life",
            format!(
                "When you would die, you rise again at {} health and blast the enemies near you.",
                pct(run::EXTRA_LIFE_HEALTH)
            ),
        ),
        Stat::DoubleCast => (
            "Double cast chance",
            format!(
                "The chance for a spell to cast twice; past 100% a third time. It stops at {}.",
                pct(stats::DOUBLE_CAST_MAX)
            ),
        ),
        Stat::SpellCooldownRecovery => s(
            "Spell cooldown recovery",
            "Your spells come back faster, and your weapon swings speed up as if from twice it.",
        ),
        Stat::BuffPower => s(
            "Buff strength",
            "Makes your buffs stronger: shields, damage and haste buffs, and auras.",
        ),
        Stat::DotStacks => (
            "Effect-over-time stacks",
            format!(
                "Your effects over time stack more times on one enemy, at most {} more.",
                esurv_sim::game::abilities::MAX_DOT_STACKS
            ),
        ),
        Stat::PetDamage => s("Pet damage", "More damage from your pets."),
        Stat::DamageVsElites => s(
            "Damage to elites",
            "More damage to elites, rares and bosses.",
        ),
        Stat::DamageVsCrowdControlled => s(
            "Damage to held enemies",
            "More damage to enemies that are slowed, held or fleeing.",
        ),
        Stat::DamageBelow30Pct => s(
            "Damage to wounded enemies",
            "More damage to enemies below 30% health.",
        ),
        Stat::DamageVsAfflicted => s(
            "Damage to afflicted enemies",
            "More damage to enemies under an effect over time.",
        ),
        Stat::WeaponDamage => s("Weapon damage", "More damage from your weapon attacks."),
        Stat::SwingSpeed => s("Swing speed", "Your weapon swings come faster."),
        Stat::SwingCooldownRecovery => s(
            "Swing recovery",
            "Only your weapon swings come faster; spells are left as they are.",
        ),
        Stat::WeaponDoubleHit => s(
            "Weapon double hit",
            "The chance for a weapon attack to hit twice, the second a full hit of its own.",
        ),
        Stat::RagePerSwing => s("Rage per swing", "More rage from each weapon swing that hits."),
        Stat::RageWhenHurt => s("Rage when hurt", "More rage from the damage you take."),
        Stat::DamageFromReduction => s(
            "Damage from reduction",
            "Your damage reduction also raises your damage by a share of itself.",
        ),
    }
}

/// Hunter's Mark (rank 1, `Spell.dbc` 1130): its icon is the extra targets'.
const HUNTERS_MARK: u32 = 1130;
/// Thorns (rank 1, `Spell.dbc` 467): its icon is the Thorns stat's.
const THORNS: u32 = 467;
/// Fitting 1.12 icons for pick-ups the game shows no icon of its own for:
/// a tome for experience, the inward swirl of Astral Recall for the magnet's pull, a healing hand
/// for the Restoration Rune (its spell's own icon is crossed swords).
const EXPERIENCE_ICON: &str = "INV_Misc_Book_11";
const MAGNET_ICON: &str = "Spell_Nature_AstralRecal";
const RESTORATION_ICON: &str = "Spell_Holy_Heal";
/// The caches' boxes.
const BATTLEGROUND_CACHE_ICON: &str = "INV_Box_01";
const RARE_CACHE_ICON: &str = "INV_Box_04";

/// The icon for `stat`: Hunter's Mark's for extra targets and the Thorns spell's for thorns (from
/// the player's `Spell.dbc`); else its prestige stat's, else a ring's or trinket's that gives it.
fn stat_icon(content: &Content, stat: Stat) -> Option<GuideIcon> {
    match stat {
        Stat::ExtraTargets => return Some(GuideIcon::Spell(HUNTERS_MARK)),
        Stat::Thorns => return Some(GuideIcon::Spell(THORNS)),
        _ => {}
    }
    content
        .prestige
        .iter()
        .find(|p| matches!(p.effect, TalentEffect::Stat { stat: s, .. } if s == stat))
        .map(|p| p.icon.clone())
        .or_else(|| content.jewelry.iter().find(|j| j.stat == stat).map(|j| j.icon.clone()))
        .map(GuideIcon::Named)
}

fn stat_entries(content: &Content, group: &[Stat]) -> Vec<GuideEntry> {
    group
        .iter()
        .map(|&stat| {
            let (name, text) = stat_text(stat);
            entry(stat_icon(content, stat), name, text)
        })
        .collect()
}

/// What a prestige stat that raises a number of every spell does.
fn spell_number_text(kind: Upgrade) -> String {
    match kind {
        Upgrade::Duration => "Every spell lasts longer.".into(),
        Upgrade::RageGain | Upgrade::EnergyGain | Upgrade::Chance => {
            "More rage gained, faster energy refill and a higher proc chance, for every spell."
                .into()
        }
        k => format!("Raises every spell's {}.", upgrade_text(k).0.to_lowercase()),
    }
}

fn stat_sections(content: &Content) -> Vec<GuideSection> {
    let mut basic = stat_entries(content, &BASIC_STATS);
    // the prestige stats that raise a number of every spell, not a hero stat
    basic.extend(content.prestige.iter().filter_map(|p| match p.effect {
        TalentEffect::AbilityMod { kind, .. } => Some(entry(
            named(&p.icon),
            p.name.clone(),
            spell_number_text(kind),
        )),
        _ => None,
    }));
    vec![
        GuideSection {
            title: "Basic stats",
            hint: None,
            entries: basic,
        },
        GuideSection {
            title: "Defences",
            hint: None,
            entries: stat_entries(content, &DEFENCE_STATS),
        },
        GuideSection {
            title: "Spell stats",
            hint: None,
            entries: stat_entries(content, &SPELL_STATS),
        },
        GuideSection {
            title: "Weapon stats",
            hint: None,
            entries: stat_entries(content, &WEAPON_STATS),
        },
    ]
}

// ---- enemies ---------------------------------------------------------------------------------

fn glowing(glow: Glow, name: &str, text: String) -> GuideEntry {
    GuideEntry {
        icon: None,
        name: name.to_owned(),
        glow: Some(glow),
        text,
    }
}

fn enemy_sections(content: &Content) -> Vec<GuideSection> {
    let b = &content.balance;
    let normal = entry(
        None,
        "Normal enemies",
        "They chase the nearest hero; some are fast, some tough, some cast from range. They \
         drop experience, and gold now and then. No glow.",
    );
    let elite = glowing(
        Glow::Elite,
        "Elites",
        format!(
            "Come on their own timer from minute {}. A purple glow. Each drops {} experience, \
             {} gold and a cache.",
            num(director::FIRST_ELITE / 60.0),
            b.experience.elite_kill,
            loot::ELITE_GOLD,
        ),
    );
    let rare = glowing(
        Glow::Rare,
        "Rares",
        format!(
            "{} to {} a run: bigger copies of the zone's elites. A silver glow. Each drops gold, \
             an instant level, a free Rare Cache, and sometimes a magnet.",
            rares::RARES_PER_RUN.start(),
            rares::RARES_PER_RUN.end(),
        ),
    );
    let boss = glowing(
        Glow::Boss,
        "Bosses",
        "Arrive at set minutes. Many mark their big attacks on the ground before they land. An \
         orange glow. Each drops experience, gold, a magnet when one is due, and a chance at gear \
         for your class."
            .into(),
    );
    let last = glowing(
        Glow::Boss,
        "Final boss",
        "The zone's last boss. Slaying it wins the run and can drop your class's weapons; then \
         you may go on into Endless."
            .into(),
    );
    let enrage = entry(
        None,
        "Enrage",
        format!(
            "Rares and bosses enrage below {} health: they move {} faster, hit {} harder and \
             use their abilities more often.",
            pct(enemy_ai::ENRAGE_HEALTH),
            pct(enemy_ai::ENRAGE_SPEED - 1.0),
            pct(enemy_ai::ENRAGE_DAMAGE - 1.0),
        ),
    );
    vec![GuideSection {
        title: "Enemies",
        hint: Some("The glows can be turned off in the options.".into()),
        entries: vec![normal, elite, rare, boss, last, enrage],
    }]
}

// ---- upgrades --------------------------------------------------------------------------------

/// An upgrade kind's name and what a card of it does to a spell.
pub fn upgrade_text(kind: Upgrade) -> (&'static str, String) {
    let gain = pct(upgrades::MAX_GAIN - 1.0);
    match kind {
        Upgrade::Power => (
            "Power",
            "More damage; on a heal more healing, on a shield more absorb. Each Power card \
             adds a little less than the one before."
                .into(),
        ),
        Upgrade::Cooldown => (
            "Cooldown",
            format!(
                "A shorter cooldown, down to {} of the spell's own.",
                pct(upgrades::COOLDOWN_FLOOR)
            ),
        ),
        Upgrade::Targets => (
            "Targets",
            format!(
                "Hits more enemies at once, up to {} more.",
                upgrades::MAX_EXTRA_TARGETS
            ),
        ),
        Upgrade::Area => (
            "Area",
            format!("A larger area, up to {}\u{d7} its own.", num(upgrades::MAX_AREA)),
        ),
        Upgrade::Duration => (
            "Duration",
            format!("Lasts longer, up to {}\u{d7} its own.", num(upgrades::MAX_DURATION)),
        ),
        Upgrade::RageGain => (
            "Rage gain",
            format!("More rage from a rage spell, up to +{gain}."),
        ),
        Upgrade::EnergyGain => (
            "Energy refill",
            format!("A faster energy refill from an energy spell, up to +{gain}."),
        ),
        Upgrade::Chance => (
            "Proc chance",
            format!(
                "A higher chance to proc, up to +{gain}; past 100% the rest is the chance of a \
                 second proc."
            ),
        ),
        Upgrade::Stacks => ("Stacks", "One more stack, on a spell that stacks.".into()),
        Upgrade::Strikes => (
            "Extra hits",
            "One more extra hit each time the spell procs.".into(),
        ),
        Upgrade::Imps => ("Imps", "One more imp each cast.".into()),
        Upgrade::Pets => (
            "Pets",
            format!("One more pet, up to {} in all.", upgrades::MAX_EXTRA_PETS + 1),
        ),
        Upgrade::Critters => ("Critters", "One more critter each cast.".into()),
    }
}

fn upgrade_sections() -> Vec<GuideSection> {
    let cards = vec![
        entry(
            None,
            "Level up",
            format!(
                "Every level offers {} cards to pick one from and heals you {} of your health. \
                 The run waits while you choose.",
                offers::OFFERS_PER_SCREEN,
                pct(level::LEVEL_UP_HEAL)
            ),
        ),
        entry(
            None,
            "New spell",
            format!(
                "Learns a spell, up to {} spells. A rarer card starts it stronger: more power, \
                 or a shorter cooldown for a spell that neither hurts, heals nor shields.",
                upgrades::MAX_SPELLS
            ),
        ),
        entry(
            None,
            "Spell upgrade",
            format!(
                "Improves one thing about a spell you know, {} on a legendary card. Every \
                 upgrade also raises the spell's rank, adding a little power.",
                upgrades::LEGENDARY_KINDS
            ),
        ),
        entry(
            None,
            "Rarity",
            "Cards come common, uncommon, rare, epic or legendary; a rarer card gives more. \
             Luck and your level raise the odds.",
        ),
        entry(
            None,
            "Reroll",
            format!(
                "Draws new cards. The first {} of a run are free, then a reroll costs {} gold, \
                 {} times more each time.",
                choices::FREE_REROLLS,
                offers::reroll_cost(0, 0),
                offers::reroll_cost(0, 1) / offers::reroll_cost(0, 0).max(1)
            ),
        ),
        entry(
            named(crate::cards::card_text::GOLD_ICON),
            "Gold card",
            format!(
                "When nothing is left to offer, a level-up gives {} gold instead.",
                upgrades::EMPTY_SCREEN_GOLD
            ),
        ),
    ];
    let kinds = Upgrade::ALL
        .into_iter()
        .map(|k| {
            let (name, text) = upgrade_text(k);
            entry(None, name, text)
        })
        .collect();
    vec![
        GuideSection {
            title: "Level-up cards",
            hint: None,
            entries: cards,
        },
        GuideSection {
            title: "Upgrade kinds",
            hint: Some(
                "What a spell upgrade can improve. A spell is only offered the kinds it can use."
                    .into(),
            ),
            entries: kinds,
        },
    ]
}

// ---- loot ------------------------------------------------------------------------------------

fn loot_sections(content: &Content) -> Vec<GuideSection> {
    let standard = &content.balance.battle_standard;
    let (speed, speed_secs) = loot::SPEED_RUNE;
    let (berserk, berserk_secs) = loot::BERSERKER_RUNE;
    let pick_ups = vec![
        entry(
            named(EXPERIENCE_ICON),
            "Experience",
            format!(
                "Orbs from kills. They fly to you within your pickup range, and after {} \
                 seconds from anywhere. Bigger orbs hold more.",
                num(pickups::ORB_SELF_COLLECT_AGE)
            ),
        ),
        entry(
            named(crate::cards::card_text::GOLD_ICON),
            "Gold",
            "Coins from kills, taken within your pickup range. Gold opens caches and buys \
             rerolls.",
        ),
        entry(
            named(MAGNET_ICON),
            "Magnet",
            format!(
                "{} to {} a run, under a beam of light: each appears on its own unless a boss, \
                 or sometimes a rare, drops it first. It pulls every orb and coin on the field \
                 to you.",
                loot::MAGNETS.start(),
                loot::MAGNETS.end()
            ),
        ),
        entry(
            named(RESTORATION_ICON),
            "Restoration Rune",
            format!("Heals {} of your maximum health.", pct(loot::RESTORATION_HEAL)),
        ),
        entry(
            Some(GuideIcon::Spell(pickups::BERSERKER_RUNE_VISUAL)),
            "Berserker Rune",
            format!("+{} damage for {} seconds.", pct(berserk), num(berserk_secs)),
        ),
        entry(
            Some(GuideIcon::Spell(pickups::SPEED_RUNE_VISUAL)),
            "Speed Rune",
            format!("+{} movement speed for {} seconds.", pct(speed), num(speed_secs)),
        ),
        entry(
            Some(GuideIcon::Spell(standard.planted_look.alliance)),
            "Warband Standard",
            format!(
                "{} to {} a run, under a beam of light. Walk over it and a player character of \
                 your faction comes to fight beside you for about {} seconds.",
                standard.per_run.0,
                standard.per_run.1,
                num(standard.assist_seconds)
            ),
        ),
    ];
    let ring = content.jewelry.iter().find(|j| j.slot == JewelrySlot::Ring);
    let trinket = content.jewelry.iter().find(|j| j.slot == JewelrySlot::Trinket);
    let per_slot = choices::JEWELRY_PER_SLOT;
    let caches = vec![
        entry(
            named(BATTLEGROUND_CACHE_ICON),
            "Battleground Cache",
            format!(
                "{} lie on the field as a run starts, more come over time, and every elite \
                 drops one. Opening one costs gold, from {} and more with each one opened. It \
                 offers {} cards: rings, trinkets, amulets, or empowering your amulet.",
                loot::CACHES_AT_START,
                loot::CACHE_PRICE_START,
                offers::OFFERS_PER_SCREEN
            ),
        ),
        GuideEntry {
            icon: named(RARE_CACHE_ICON),
            ..glowing(
                Glow::Rare,
                "Rare Cache (Free)",
                "Dropped by a rare, with its silver glow. It is free to open, leaves the cache \
                 price as it is, and its cards tend to be rarer."
                    .into(),
            )
        },
        entry(
            ring.map(|j| GuideIcon::Named(j.icon.clone())),
            "Rings",
            format!(
                "You wear up to {per_slot} for the run. Once taken a ring is locked in: a cache \
                 can raise it, never take it off."
            ),
        ),
        entry(
            trinket.map(|j| GuideIcon::Named(j.icon.clone())),
            "Trinkets",
            format!(
                "As rings: up to {per_slot} for the run, locked in once taken, raised by later \
                 caches."
            ),
        ),
        entry(
            content.amulets.first().map(|a| GuideIcon::Named(a.icon.clone())),
            "Amulets",
            format!(
                "You wear one. A new amulet replaces the one you wear; an Empower card makes the \
                 worn one stronger, toward {}\u{d7} its numbers.",
                num(amulets::EMPOWER_LIMIT)
            ),
        ),
        entry(
            content
                .legendary_amulets
                .first()
                .map(|a| GuideIcon::Named(a.icon.clone())),
            "Legendary amulets",
            "A legendary amulet card can bring one made for a spell you know: a far bigger \
             version of that spell.",
        ),
    ];
    vec![
        GuideSection {
            title: "Pick-ups",
            hint: Some(format!(
                "Walk over them. A rune appears every {} seconds while fewer than {} lie on the \
                 field; taking a rune whose buff is up starts it over.",
                num(loot::RUNE_EVERY),
                loot::RUNE_MAX_PRESENT
            )),
            entries: pick_ups,
        },
        GuideSection {
            title: "Loot from caches",
            hint: Some(
                "Every ring, trinket and amulet you take is kept in the codex for good.".into(),
            ),
            entries: caches,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::{Deserializer, Error as _, Visitor};

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    /// Every variant name of an enum that derives `Deserialize`, as serde knows them.
    fn variant_names<T: for<'de> serde::Deserialize<'de>>() -> &'static [&'static str] {
        struct Grab<'a>(&'a mut Option<&'static [&'static str]>);
        impl<'de> Deserializer<'de> for Grab<'_> {
            type Error = serde::de::value::Error;
            fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Self::Error> {
                Err(Self::Error::custom("not an enum"))
            }
            fn deserialize_enum<V: Visitor<'de>>(
                self,
                _: &'static str,
                variants: &'static [&'static str],
                _: V,
            ) -> Result<V::Value, Self::Error> {
                *self.0 = Some(variants);
                Err(Self::Error::custom("read"))
            }
            serde::forward_to_deserialize_any! {
                bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes
                byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct map struct
                identifier ignored_any
            }
        }
        let mut names = None;
        let _ = T::deserialize(Grab(&mut names));
        names.expect("an enum")
    }

    /// Every hero stat the game has, read from the `Stat` type itself.
    fn every_stat() -> Vec<Stat> {
        variant_names::<Stat>()
            .iter()
            .map(|n| ron::from_str::<Stat>(n).unwrap_or_else(|e| panic!("{n}: {e}")))
            .collect()
    }

    #[test]
    fn every_stat_has_one_stats_entry() {
        let all = every_stat();
        assert!(all.len() > 30, "{}", all.len());
        let groups: Vec<Stat> = [&BASIC_STATS[..], &DEFENCE_STATS, &SPELL_STATS, &WEAPON_STATS]
            .concat();
        for stat in &all {
            let n = groups.iter().filter(|s| *s == stat).count();
            assert_eq!(n, 1, "{stat:?} is in {n} groups of the Stats tab: it needs one");
            let (name, text) = stat_text(*stat);
            assert!(!name.is_empty() && !text.is_empty(), "{stat:?}");
        }
        assert_eq!(groups.len(), all.len());
        // and the tab shows them all, with the prestige stats that are not hero stats
        let c = content();
        let shown: Vec<String> = sections(&c, GuideTab::Stats)
            .into_iter()
            .flat_map(|s| s.entries)
            .map(|e| e.name)
            .collect();
        for stat in &all {
            assert!(shown.iter().any(|n| n == stat_text(*stat).0), "{stat:?}");
        }
        for p in &c.prestige {
            let named = match p.effect {
                TalentEffect::Stat { stat, .. } => stat_text(stat).0.to_owned(),
                _ => p.name.clone(),
            };
            assert!(shown.contains(&named), "prestige {}", p.id);
        }
    }

    #[test]
    fn every_upgrade_kind_has_its_entry() {
        let shown: Vec<GuideEntry> = sections(&content(), GuideTab::Upgrades)
            .into_iter()
            .flat_map(|s| s.entries)
            .collect();
        for k in Upgrade::ALL {
            let (name, text) = upgrade_text(k);
            assert!(shown.iter().any(|e| e.name == name && e.text == text), "{k:?}");
        }
    }

    #[test]
    fn every_tab_has_entries_with_words() {
        let c = content();
        for tab in GuideTab::ALL {
            let s = sections(&c, tab);
            assert!(!s.is_empty(), "{tab:?}");
            for e in s.iter().flat_map(|s| &s.entries) {
                assert!(!e.name.is_empty() && !e.text.is_empty(), "{tab:?} {}", e.name);
                assert!(!e.text.contains('{'), "{tab:?} {}: {}", e.name, e.text);
            }
        }
    }

    fn text_of(c: &Content, tab: GuideTab, name: &str) -> String {
        sections(c, tab)
            .into_iter()
            .flat_map(|s| s.entries)
            .find(|e| e.name == name)
            .unwrap_or_else(|| panic!("{name}"))
            .text
    }

    #[test]
    fn the_numbers_come_from_the_content() {
        let mut c = content();
        let elite = text_of(&c, GuideTab::Enemies, "Elites");
        assert!(elite.contains(&format!("{} experience", c.balance.experience.elite_kill)), "{elite}");
        // the guide says nothing of how enemy health or damage scales: no multipliers
        for s in sections(&c, GuideTab::Enemies) {
            assert!(!s.hint.unwrap_or_default().contains("grow"));
            for e in s.entries {
                for words in ["the health", "the damage", "\u{d7}", "difficulty", "grow"] {
                    assert!(!e.text.contains(words), "{}: {}", e.name, e.text);
                }
            }
        }
        // change the balance: the guide follows
        c.balance.experience.elite_kill = 1234;
        c.balance.battle_standard.per_run = (3, 4);
        c.balance.battle_standard.assist_seconds = 21.0;
        let elite = text_of(&c, GuideTab::Enemies, "Elites");
        assert!(elite.contains("1234 experience"), "{elite}");
        let standard = text_of(&c, GuideTab::Loot, "Warband Standard");
        assert!(standard.starts_with("3 to 4 a run"), "{standard}");
        assert!(standard.contains("about 21 seconds"), "{standard}");
        // the rules' own constants, quoted
        let speed = text_of(&c, GuideTab::Loot, "Speed Rune");
        assert_eq!(
            speed,
            format!(
                "+{} movement speed for {} seconds.",
                pct(loot::SPEED_RUNE.0),
                num(loot::SPEED_RUNE.1)
            )
        );
        assert!(text_of(&c, GuideTab::Loot, "Restoration Rune").contains(&pct(loot::RESTORATION_HEAL)));
    }

    #[test]
    fn numbers_read_plainly() {
        assert_eq!(num(10.0), "10");
        assert_eq!(num(1.35), "1.35");
        assert_eq!(num(7.5), "7.5");
        assert_eq!(pct(0.6), "60%");
        assert_eq!(pct(0.005), "0.5%");
        assert_eq!(pct(2.2), "220%");
    }
}
