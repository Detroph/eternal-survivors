//! The pick-one-of-three screens: a level-up offers new spells and spell upgrades, a cache
//! offers rings, trinkets, amulets and empowering the worn amulet. While a screen is open the
//! run waits.

use esurv_content::{Ability, Amulet, Delivery, SpellEffect, Stat, TalentEffect, Upgrade};

use super::casting::ProcOrigin;
use super::abilities::duration;
use super::amulets::{amulet_candidates, empower_factor, empowered, pick_amulet};
use super::offers::{
    gold_card, jewelry_cap, jewelry_upgrade_step, new_jewelry_value, rarity_factor, reroll_cost, roll_rarity, soulstone_lives,
    spell_card_effect, Rarity, SpellCardEffect, AMULET_RARITY_BONUS, DOUBLE_CAST_CAP, RARE_CACHE_RARITY_BONUS, OFFERS_PER_SCREEN,
};
use super::run::{Event, Phase, Run};
use super::spells::{cooldown, SpellModifier};
use super::stats::Modifier;
use super::upgrades::{apply as apply_upgrade, kinds_per_card, new_spell_weight, offerable, step, COMMON_CARD_WEIGHT_FACTOR, EMPTY_SCREEN_GOLD, UPGRADE_WEIGHT};

/// Free rerolls a hero starts a run with; paid rerolls also start again at none.
pub const FREE_REROLLS: u32 = 3;
/// Cache offer weights: an upgrade to a worn ring or trinket, a new one, an amulet, empowering
/// the worn amulet.
pub const WORN_UPGRADE_WEIGHT: f32 = 2.0;
pub const NEW_JEWELRY_WEIGHT: f32 = 0.5;
pub const AMULET_WEIGHT: f32 = 2.5;
pub const EMPOWER_WEIGHT: f32 = 1.0;
/// Jewelry stops being offered as an upgrade at this share of its cap.
pub const NEAR_CAP: f32 = 0.9;
/// Most extra lives and extra stacks jewelry can give.
pub const MAX_JEWELRY_LIVES: u32 = 2;
pub const MAX_JEWELRY_DOT_STACKS: f32 = 3.0;
/// A hero wears at most this many rings, and this many trinkets: with as many worn, no new one
/// of that slot is offered (upgrades of the worn ones still are).
pub const JEWELRY_PER_SLOT: usize = 2;

/// Which screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKind {
    LevelUp,
    /// A cache the hero bought.
    Cache,
    /// The cache a slain rare dropped, free to open: a cache's offers, named apart.
    RareCache,
}

/// One card on a screen.
#[derive(Debug, Clone, PartialEq)]
pub enum Offer {
    /// Learn `spell`, made stronger by the card's rarity.
    NewSpell { spell: String, rarity: Rarity },
    /// Improve a known spell in `kinds` (two on a legendary card).
    UpgradeSpell { spell: String, kinds: Vec<Upgrade>, rarity: Rarity },
    /// Wear a new ring or trinket.
    NewJewelry { id: String, value: f32, rarity: Rarity },
    /// Raise a worn ring or trinket by `step`.
    UpgradeJewelry { id: String, step: f32, rarity: Rarity },
    /// Wear this amulet instead of the current one.
    Amulet { amulet: Box<Amulet> },
    /// The worn amulet gains a rank.
    Empower,
    Gold(u32),
}

impl Offer {
    /// The codex entry taking this offer collects: a new ring or trinket, or a new necklace
    /// (ordinary or legendary), by its id.
    pub fn codex_entry(&self) -> Option<&str> {
        match self {
            Offer::NewJewelry { id, .. } => Some(id),
            Offer::Amulet { amulet } => Some(&amulet.id),
            _ => None,
        }
    }
}

/// An open screen.
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceScreen {
    pub hero: usize,
    pub kind: ScreenKind,
    pub offers: Vec<Offer>,
}

/// A worn ring or trinket.
#[derive(Debug, Clone, PartialEq)]
pub struct WornJewelry {
    pub id: String,
    pub value: f32,
    pub rank: u32,
}

/// The worn amulet and how often it has been empowered (rank 1 when first worn).
#[derive(Debug, Clone, PartialEq)]
pub struct WornAmulet {
    pub amulet: Amulet,
    pub rank: u32,
}

/// Whether buff power strengthens spell `a`: one cast on the hero (a self effect) that puts up a
/// shield, a damage or haste buff, or an aura.
fn strengthened_by_buff_power(a: &Ability) -> bool {
    a.delivery == Delivery::SelfEffect
        && matches!(
            a.effect,
            SpellEffect::Shield { .. } | SpellEffect::DamageBuff { .. } | SpellEffect::HasteBuff { .. } | SpellEffect::Thorns { .. }
        )
}

/// Picks up to `n` distinct indices by weight.
fn pick_distinct(rng: &mut crate::rng::Rng, mut weights: Vec<f32>, n: usize) -> Vec<usize> {
    let mut out = Vec::new();
    while out.len() < n {
        let Some(i) = rng.weighted(&weights) else { break };
        weights[i] = 0.0;
        out.push(i);
    }
    out
}

/// Whether a card's rarity counts as power (damage, heals, shields) or as cooldown.
pub fn damaging(spell: &Ability) -> bool {
    spell.damage > 0.0 || matches!(spell.effect, SpellEffect::Heal { .. } | SpellEffect::Shield { .. })
}

/// A new-spell card's head start of `rarity` on the spell it teaches: more power for a spell
/// that deals damage, heals or shields, else a shorter cooldown.
pub fn head_start(u: &mut super::upgrades::SpellUpgrades, spell: &Ability, rarity: Rarity) {
    match spell_card_effect(rarity, damaging(spell)) {
        SpellCardEffect::Power(b) => u.power *= 1.0 + b,
        SpellCardEffect::Cooldown(b) => u.cooldown *= 1.0 - b,
        SpellCardEffect::None => {}
    }
}

/// The extra rarity factor of an offer on a cache screen: none for a ring or trinket from a cache
/// bought with gold, more for an amulet, and more again for anything in a rare cache.
pub fn cache_rarity_bonus(kind: ScreenKind, amulet: bool) -> f32 {
    let rare = if kind == ScreenKind::RareCache { RARE_CACHE_RARITY_BONUS } else { 0.0 };
    rare + if amulet { AMULET_RARITY_BONUS } else { 0.0 }
}

impl Run {
    fn card_rarity(&mut self, hero: usize, bonus: f32) -> Rarity {
        let h = &self.heroes[hero];
        let a = rarity_factor(h.stats.luck, h.progress.level, bonus);
        roll_rarity(&mut self.rng, a)
    }

    /// After a tick: opens the next screen a hero is owed (level-ups first), pausing the run.
    pub(crate) fn open_pending_screen(&mut self) {
        if self.screen.is_some() || self.phase != Phase::Running {
            return;
        }
        let next = (0..self.heroes.len()).find_map(|i| {
            let h = &self.heroes[i];
            if !h.alive {
                None
            } else if h.pending_choices > 0 {
                Some((i, ScreenKind::LevelUp))
            } else if h.pending_rare_caches > 0 {
                Some((i, ScreenKind::RareCache))
            } else if h.pending_caches > 0 {
                Some((i, ScreenKind::Cache))
            } else {
                None
            }
        });
        if let Some((hero, kind)) = next {
            let offers = self.build_offers(hero, kind);
            self.screen = Some(ChoiceScreen { hero, kind, offers: offers.clone() });
            self.phase = Phase::Choosing;
            let id = self.heroes[hero].id;
            self.push(Event::ScreenOpened { hero: id, kind, offers });
        }
    }

    fn build_offers(&mut self, hero: usize, kind: ScreenKind) -> Vec<Offer> {
        match kind {
            ScreenKind::LevelUp => self.level_up_offers(hero),
            ScreenKind::Cache | ScreenKind::RareCache => self.cache_offers(hero, kind),
        }
    }

    /// New spells (while fewer than six are known) and an upgrade for each known spell that can
    /// still improve; three different ones by weight; a 25-gold card when there is nothing.
    fn level_up_offers(&mut self, hero: usize) -> Vec<Offer> {
        let content = self.content.clone();
        let h = &self.heroes[hero];
        let class = &content.classes[h.class];
        let mut cands: Vec<(f32, Option<String>, String)> = Vec::new();
        // a starting option's finisher has a slot of its own, outside the six
        let partners = super::kit::option_of(&content, h).map_or(0, |o| o.spells.len().saturating_sub(1));
        if let Some(w) = new_spell_weight(h.spells.known.len().saturating_sub(partners)) {
            let common = super::kit::option_of(&content, h).map_or(&[][..], |o| o.common_cards.as_slice());
            for id in class.card_pool(Some(&h.start)).iter().filter(|id| !h.spells.knows(id)) {
                let w = if common.contains(id) { w * COMMON_CARD_WEIGHT_FACTOR } else { w };
                cands.push((w, None, id.clone()));
            }
        }
        for slot in &h.spells.known {
            let Some(a) = content.ability(&slot.spell) else { continue };
            let inst = slot.instance();
            let lasting = matches!(a.effect, SpellEffect::DamageBuff { .. } | SpellEffect::HasteBuff { .. } | SpellEffect::Thorns { .. })
                && a.delivery == Delivery::SelfEffect;
            let cooldown_still_matters = !(lasting && duration(a, &inst) >= cooldown(a, &inst, h.stats.cooldown_factor));
            if !offerable(a, &slot.upgrades, cooldown_still_matters, true).is_empty() {
                cands.push((UPGRADE_WEIGHT, Some(slot.spell.clone()), slot.spell.clone()));
            }
        }
        let picks = pick_distinct(&mut self.rng, cands.iter().map(|c| c.0).collect(), OFFERS_PER_SCREEN);
        if picks.is_empty() {
            return vec![Offer::Gold(EMPTY_SCREEN_GOLD)];
        }
        let mut offers = Vec::new();
        for i in picks {
            let rarity = self.card_rarity(hero, 0.0);
            let (upgrade, spell) = (&cands[i].1, cands[i].2.clone());
            if upgrade.is_none() {
                offers.push(Offer::NewSpell { spell, rarity });
                continue;
            }
            let h = &self.heroes[hero];
            let slot = h.spells.known.iter().find(|s| s.spell == spell).unwrap();
            let a = content.ability(&spell).unwrap();
            let inst = slot.instance();
            let lasting = matches!(a.effect, SpellEffect::DamageBuff { .. } | SpellEffect::HasteBuff { .. } | SpellEffect::Thorns { .. })
                && a.delivery == Delivery::SelfEffect;
            let still = !(lasting && duration(a, &inst) >= cooldown(a, &inst, h.stats.cooldown_factor));
            let all = offerable(a, &slot.upgrades, still, true);
            // a kind that adds nothing at the card's rarity (Targets below rare)
            // is left off; a card left with none rolls rare instead
            let adds = |r: Rarity| all.iter().copied().filter(|&k| step(&slot.upgrades, k, r) > 0.0).collect::<Vec<_>>();
            let (rarity, mut kinds) = match adds(rarity) {
                k if k.is_empty() && rarity < Rarity::Rare => (Rarity::Rare, adds(Rarity::Rare)),
                k => (rarity, k),
            };
            // a random choice of the kinds that can still grow: two on a legendary card
            for k in (1..kinds.len()).rev() {
                let j = (self.rng.next_u64() % (k as u64 + 1)) as usize;
                kinds.swap(k, j);
            }
            kinds.truncate(kinds_per_card(rarity));
            offers.push(Offer::UpgradeSpell { spell, kinds, rarity });
        }
        offers
    }

    /// Upgrades to worn rings and trinkets, new ones, an amulet and (with one worn) empowering
    /// it; three different ones by weight, topped up with a gold card. Rarities roll with
    /// `cache_rarity_bonus`.
    fn cache_offers(&mut self, hero: usize, kind: ScreenKind) -> Vec<Offer> {
        enum Cand {
            Upgrade(String),
            New(String),
            Amulet,
            Empower,
        }
        let content = self.content.clone();
        let h = &self.heroes[hero];
        // a new buff-power piece only for a hero with a spell it strengthens (a shield, a damage or
        // haste buff, an aura; its race spell counts), a new DoT-stacks piece only for one with a
        // damage-over-time spell
        let spell = |s: &super::spells::SpellSlot| content.ability(&s.spell);
        let buffs = h.spells.known.iter().chain(h.spells.racial.iter()).filter_map(spell).any(strengthened_by_buff_power);
        let dots = h.spells.known.iter().filter_map(spell).any(|a| a.delivery == Delivery::DamageOverTime);
        let mut cands: Vec<(f32, Cand)> = Vec::new();
        for j in &content.jewelry {
            match h.jewelry.iter().find(|w| w.id == j.id) {
                Some(w) => {
                    let open = match j.stat {
                        Stat::ExtraLives => h.extra_lives < MAX_JEWELRY_LIVES,
                        Stat::DotStacks => w.value < MAX_JEWELRY_DOT_STACKS,
                        Stat::DoubleCast => w.value < NEAR_CAP * DOUBLE_CAST_CAP,
                        s => match jewelry_cap(s).0 {
                            Some(mult) => w.value < NEAR_CAP * mult * j.value,
                            None => true,
                        },
                    };
                    if open {
                        cands.push((WORN_UPGRADE_WEIGHT, Cand::Upgrade(j.id.clone())));
                    }
                }
                None if (j.stat == Stat::BuffPower && !buffs) || (j.stat == Stat::DotStacks && !dots) => {}
                None => {
                    let worn_in_slot = h.jewelry.iter().filter(|w| content.jewelry.iter().any(|k| k.id == w.id && k.slot == j.slot)).count();
                    if worn_in_slot < JEWELRY_PER_SLOT {
                        cands.push((NEW_JEWELRY_WEIGHT, Cand::New(j.id.clone())));
                    }
                }
            }
        }
        cands.push((AMULET_WEIGHT, Cand::Amulet));
        if h.amulet.is_some() {
            cands.push((EMPOWER_WEIGHT, Cand::Empower));
        }
        let picks = pick_distinct(&mut self.rng, cands.iter().map(|c| c.0).collect(), OFFERS_PER_SCREEN);
        let mut offers = Vec::new();
        for i in picks {
            let rarity = match cands[i].1 {
                Cand::Upgrade(_) | Cand::New(_) => self.card_rarity(hero, cache_rarity_bonus(kind, false)),
                Cand::Amulet => self.card_rarity(hero, cache_rarity_bonus(kind, true)),
                // the worn amulet, at its own rarity
                Cand::Empower => Rarity::Common,
            };
            let h = &self.heroes[hero];
            match &cands[i].1 {
                Cand::New(id) => {
                    let j = content.jewelry.iter().find(|j| &j.id == id).unwrap();
                    let value = match j.stat {
                        Stat::ExtraLives => soulstone_lives(rarity, h.extra_lives, MAX_JEWELRY_LIVES) as f32,
                        Stat::DotStacks => 1.0 + f32::from(rarity > Rarity::Uncommon),
                        s => {
                            let (mult, hard) = jewelry_cap(s);
                            new_jewelry_value(j.value, rarity, mult, hard)
                        }
                    };
                    offers.push(Offer::NewJewelry { id: id.clone(), value, rarity });
                }
                Cand::Upgrade(id) => {
                    let j = content.jewelry.iter().find(|j| &j.id == id).unwrap();
                    let w = h.jewelry.iter().find(|w| &w.id == id).unwrap();
                    let step = match j.stat {
                        Stat::ExtraLives => soulstone_lives(rarity, h.extra_lives, MAX_JEWELRY_LIVES) as f32,
                        Stat::DotStacks => (1.0 + f32::from(rarity > Rarity::Uncommon)).min(MAX_JEWELRY_DOT_STACKS - w.value),
                        s => {
                            let (mult, hard) = jewelry_cap(s);
                            jewelry_upgrade_step(j.value, w.value, w.rank, rarity, mult, hard)
                        }
                    };
                    offers.push(Offer::UpgradeJewelry { id: id.clone(), step, rarity });
                }
                Cand::Amulet => {
                    let known: Vec<&Ability> = h.spells.known.iter().filter_map(|s| content.ability(&s.spell)).collect();
                    let worn = h.amulet.as_ref().map(|a| a.amulet.id.clone());
                    let list = amulet_candidates(&content, rarity, worn.as_deref(), &known);
                    if let Some(amulet) = pick_amulet(&mut self.rng, &list) {
                        offers.push(Offer::Amulet { amulet: Box::new(amulet) });
                    }
                }
                Cand::Empower => offers.push(Offer::Empower),
            }
        }
        // the first cache a hero opens with a sure amulet offers it, in the amulet's place (or
        // the last offer's)
        if kind == ScreenKind::Cache {
            if let Some(amulet) = self.heroes[hero].first_cache_amulet.take() {
                let offer = Offer::Amulet { amulet: Box::new(amulet) };
                match offers.iter().position(|o| matches!(o, Offer::Amulet { .. })) {
                    Some(k) => offers[k] = offer,
                    None if offers.len() >= OFFERS_PER_SCREEN => *offers.last_mut().unwrap() = offer,
                    None => offers.push(offer),
                }
            }
        }
        if offers.len() < OFFERS_PER_SCREEN {
            offers.push(Offer::Gold(gold_card(self.field.cache_price)));
        }
        offers
    }

    /// The open screen's hero takes offer `index`.
    pub fn choose(&mut self, hero: u32, index: usize) -> bool {
        let Some(screen) = self.screen.clone() else { return false };
        if self.heroes[screen.hero].id != hero || index >= screen.offers.len() {
            return false;
        }
        let i = screen.hero;
        let content = self.content.clone();
        match &screen.offers[index] {
            Offer::NewSpell { spell, rarity } => {
                if let Some(a) = content.ability(spell) {
                    let mods = self.heroes[i].spell_modifiers.clone();
                    self.heroes[i].spells.learn(a, &mods);
                    self.push(Event::SpellLearned { hero, spell: spell.clone() });
                    if let Some(slot) = self.heroes[i].spells.known.iter_mut().find(|s| &s.spell == spell) {
                        head_start(&mut slot.upgrades, a, *rarity);
                    }
                }
            }
            Offer::UpgradeSpell { spell, kinds, rarity } => {
                if let Some(slot) = self.heroes[i].spells.known.iter_mut().find(|s| &s.spell == spell) {
                    apply_upgrade(&mut slot.upgrades, kinds, *rarity);
                }
            }
            Offer::NewJewelry { id, value, .. } => {
                let stat = content.jewelry.iter().find(|j| &j.id == id).map(|j| j.stat);
                if stat == Some(Stat::ExtraLives) {
                    let h = &mut self.heroes[i];
                    h.extra_lives = (h.extra_lives + *value as u32).min(MAX_JEWELRY_LIVES);
                }
                self.heroes[i].jewelry.push(WornJewelry { id: id.clone(), value: *value, rank: 1 });
                self.rebuild_hero_modifiers(i);
            }
            Offer::UpgradeJewelry { id, step, .. } => {
                let stat = content.jewelry.iter().find(|j| &j.id == id).map(|j| j.stat);
                if stat == Some(Stat::ExtraLives) {
                    let h = &mut self.heroes[i];
                    h.extra_lives = (h.extra_lives + *step as u32).min(MAX_JEWELRY_LIVES);
                }
                if let Some(w) = self.heroes[i].jewelry.iter_mut().find(|w| &w.id == id) {
                    w.value += step;
                    w.rank += 1;
                }
                self.rebuild_hero_modifiers(i);
            }
            Offer::Amulet { amulet } => {
                self.heroes[i].amulet = Some(WornAmulet { amulet: (**amulet).clone(), rank: 1 });
                self.rebuild_hero_modifiers(i);
            }
            Offer::Empower => {
                if let Some(a) = self.heroes[i].amulet.as_mut() {
                    a.rank += 1;
                }
                self.rebuild_hero_modifiers(i);
            }
            Offer::Gold(g) => {
                self.heroes[i].gold += g;
                self.heroes[i].gold_earned += g;
            }
        }
        match screen.kind {
            ScreenKind::LevelUp => self.heroes[i].pending_choices -= 1,
            ScreenKind::Cache => self.heroes[i].pending_caches -= 1,
            ScreenKind::RareCache => self.heroes[i].pending_rare_caches -= 1,
        }
        self.push(Event::OfferTaken { hero, offer: screen.offers[index].clone() });
        self.screen = None;
        self.run_again();
        self.open_pending_screen();
        true
    }

    /// New offers on the open screen: free while free rerolls remain, then 100 × 3ⁿ gold.
    pub fn reroll(&mut self, hero: u32) -> bool {
        let Some(screen) = self.screen.clone() else { return false };
        let i = screen.hero;
        if self.heroes[i].id != hero {
            return false;
        }
        let h = &self.heroes[i];
        let cost = reroll_cost(h.rerolls_free, h.rerolls_paid);
        if h.gold < cost {
            return false;
        }
        let h = &mut self.heroes[i];
        h.gold -= cost;
        if h.rerolls_free > 0 {
            h.rerolls_free -= 1;
        } else {
            h.rerolls_paid += 1;
        }
        let offers = self.build_offers(i, screen.kind);
        self.screen = Some(ChoiceScreen { offers: offers.clone(), ..screen });
        self.push(Event::ScreenOpened { hero, kind: screen.kind, offers });
        true
    }

    /// For scripted test runs: whenever a screen is open, take its first card.
    #[cfg(test)]
    pub(crate) fn take_first_offers(&mut self) {
        while let Some(screen) = &self.screen {
            let hero = self.heroes[screen.hero].id;
            self.choose(hero, 0);
        }
    }

    /// Puts a new ring or trinket (`NewJewelry`) or an amulet (`Amulet`) on hero `hero` as taking
    /// its card does, with no screen: a development aid, for a live look at the run's jewelry.
    /// `false` for another offer or an unknown hero.
    pub fn grant(&mut self, hero: u32, offer: Offer) -> bool {
        let Some(i) = self.heroes.iter().position(|h| h.id == hero) else { return false };
        match &offer {
            Offer::NewJewelry { id, value, .. } => self.heroes[i].jewelry.push(WornJewelry { id: id.clone(), value: *value, rank: 1 }),
            Offer::Amulet { amulet } => self.heroes[i].amulet = Some(WornAmulet { amulet: (**amulet).clone(), rank: 1 }),
            _ => return false,
        }
        self.rebuild_hero_modifiers(i);
        self.push(Event::OfferTaken { hero, offer });
        true
    }

    /// Rebuilds a hero's modifiers from their sources (race, talents and gear from the setup,
    /// worn jewelry, the worn amulet at its rank), then the spells' talent/amulet multipliers
    /// and the stat sheet.
    pub(crate) fn rebuild_hero_modifiers(&mut self, i: usize) {
        let content = self.content.clone();
        let h = &mut self.heroes[i];
        let mut mods = h.base_modifiers.clone();
        let mut spell_mods = h.base_spell_modifiers.clone();
        let mut milestones = h.base_milestones.clone();
        let mut origins = h.base_milestone_origins.clone();
        origins.resize(milestones.len(), ProcOrigin::Other);
        for w in &h.jewelry {
            if let Some(j) = content.jewelry.iter().find(|j| j.id == w.id) {
                if j.stat != Stat::ExtraLives {
                    mods.push(Modifier::new(j.stat, w.value));
                }
                // Hand of Justice's weapon double hit follows its double cast
                let twice = j.double_hit_at(w.value);
                if twice > 0.0 {
                    mods.push(Modifier::new(Stat::WeaponDoubleHit, twice));
                }
            }
        }
        if let Some(a) = &h.amulet {
            let k = empower_factor(a.amulet.growth, a.rank);
            for e in &a.amulet.effects {
                match empowered(e, k) {
                    TalentEffect::Stat { stat, per_rank } => mods.push(Modifier::new(stat, per_rank)),
                    e @ TalentEffect::AbilityMod { .. } => spell_mods.extend(SpellModifier::from_effect(&e, 1)),
                    TalentEffect::Milestone(m) => {
                        milestones.push(m);
                        origins.push(ProcOrigin::of_amulet(&a.amulet.id));
                    }
                }
            }
        }
        h.modifiers = mods;
        h.spell_modifiers = spell_mods.clone();
        h.milestones = milestones;
        h.milestone_origins = origins;
        h.spells.refresh_mods(&spell_mods, |id| content.ability(id));
        self.refresh_stats(i);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::Content;

    use super::*;
    use crate::game::enemy::RunOptions;
    use crate::game::run::{Flat, HeroSetup};

    #[test]
    fn amulets_and_rare_caches_roll_rarer() {
        assert_eq!(cache_rarity_bonus(ScreenKind::Cache, false), 0.0);
        assert_eq!(cache_rarity_bonus(ScreenKind::Cache, true), 0.5);
        assert_eq!(cache_rarity_bonus(ScreenKind::RareCache, false), 0.25);
        assert_eq!(cache_rarity_bonus(ScreenKind::RareCache, true), 0.75);
    }

    #[test]
    fn only_new_rings_trinkets_and_necklaces_join_the_codex() {
        let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let amulet = content.amulets[0].clone();
        let id = amulet.id.clone();
        assert_eq!(Offer::NewJewelry { id: "a_ring".into(), value: 0.1, rarity: Rarity::Common }.codex_entry(), Some("a_ring"));
        assert_eq!(Offer::Amulet { amulet: Box::new(amulet) }.codex_entry(), Some(id.as_str()));
        assert_eq!(Offer::UpgradeJewelry { id: "a_ring".into(), step: 0.1, rarity: Rarity::Rare }.codex_entry(), None, "already collected");
        assert_eq!(Offer::Gold(10).codex_entry(), None);
    }

    fn run(class: &str) -> Run {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let content = Arc::new(Content::load(dir).expect("content loads"));
        let mut r = Run::new(
            content,
            "elwynn",
            RunOptions::default(),
            &[HeroSetup { class: class.into(), race: "orc".into(), ..Default::default() }],
            Box::new(Flat),
            21,
        )
        .unwrap();
        let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
        r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        r.enemies.clear();
        r.pickups.clear();
        r.drain_events();
        r
    }

    #[test]
    fn each_druid_form_offers_its_swipe_twice_as_often_as_its_other_cards() {
        for (form, swipe) in [("cat_form", "cat_swipe"), ("bear_form", "swipe")] {
            let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
            let content = Arc::new(Content::load(dir).expect("content loads"));
            let setup = HeroSetup { class: "druid".into(), race: "night_elf".into(), start: Some(form.into()), ..Default::default() };
            let mut r = Run::new(content.clone(), "elwynn", RunOptions::default(), &[setup], Box::new(Flat), 21).unwrap();
            let pool = content.class("druid").unwrap().card_pool(Some(form)).to_vec();
            assert!(pool.iter().any(|s| s == swipe) && content.starting_option(form).unwrap().common_cards == [swipe]);
            let mut seen = std::collections::HashMap::<String, u32>::new();
            for _ in 0..4000 {
                for o in r.level_up_offers(0) {
                    if let Offer::NewSpell { spell, .. } = o {
                        *seen.entry(spell).or_default() += 1;
                    }
                }
            }
            let others: Vec<u32> = pool.iter().filter(|s| *s != swipe).map(|s| seen.get(s).copied().unwrap_or(0)).collect();
            let mean = others.iter().sum::<u32>() as f32 / others.len() as f32;
            let most = *others.iter().max().unwrap() as f32;
            let s = seen[swipe] as f32;
            assert!(s > 1.6 * mean && s > most, "{form}: {swipe} offered {s} times, the others {mean} on average, at most {most}");
        }
    }

    #[test]
    fn a_level_up_pauses_the_run_until_a_card_is_taken() {
        let mut r = run("mage");
        r.gain_experience(0, 200);
        r.tick(0.05);
        assert_eq!(r.phase, Phase::Choosing);
        let screen = r.screen.clone().unwrap();
        assert_eq!(screen.kind, ScreenKind::LevelUp);
        assert_eq!(screen.offers.len(), 3);
        let before = r.now;
        r.tick(0.05);
        assert_eq!(r.now, before, "the run waits");
        let id = r.heroes[0].id;
        let new_spell = screen.offers.iter().position(|o| matches!(o, Offer::NewSpell { .. }));
        assert!(r.choose(id, new_spell.unwrap_or(0)));
        assert_eq!(r.phase, Phase::Running);
        assert_eq!(r.heroes[0].pending_choices, 0);
        if new_spell.is_some() {
            assert_eq!(r.heroes[0].spells.known.len(), 2);
        }
    }

    #[test]
    fn several_level_ups_open_one_screen_after_another() {
        let mut r = run("warrior");
        r.gain_experience(0, 200 + 265 + 384);
        assert_eq!(r.heroes[0].pending_choices, 3);
        r.tick(0.05);
        let id = r.heroes[0].id;
        for left in (0..3).rev() {
            assert_eq!(r.phase, Phase::Choosing);
            assert!(r.choose(id, 0));
            assert_eq!(r.heroes[0].pending_choices, left);
        }
        assert_eq!(r.phase, Phase::Running);
    }

    #[test]
    fn rerolls_are_free_three_times_then_cost_gold() {
        let mut r = run("mage");
        r.gain_experience(0, 1000);
        r.tick(0.05);
        let id = r.heroes[0].id;
        for _ in 0..3 {
            assert!(r.reroll(id));
        }
        assert!(!r.reroll(id), "no gold for the fourth");
        r.heroes[0].gold = 150;
        assert!(r.reroll(id));
        assert_eq!(r.heroes[0].gold, 50);
    }

    #[test]
    fn a_cache_offers_jewelry_and_amulets_and_wearing_them_changes_stats() {
        let mut r = run("warrior");
        r.heroes[0].pending_caches = 1;
        r.tick(0.05);
        let screen = r.screen.clone().unwrap();
        assert_eq!(screen.kind, ScreenKind::Cache);
        assert_eq!(screen.offers.len(), 3);
        // wear a new damage ring directly and see the stat sheet move
        let damage = r.heroes[0].stats.damage;
        let id = r.heroes[0].id;
        r.screen = Some(ChoiceScreen {
            offers: vec![Offer::NewJewelry { id: "accuria".into(), value: 0.1, rarity: Rarity::Common }],
            ..screen
        });
        assert!(r.choose(id, 0));
        assert!((r.heroes[0].stats.damage - damage - 0.1).abs() < 1e-5);
        assert_eq!(r.heroes[0].pending_caches, 0);
    }

    /// The first cache a hero opens with a sure amulet offers it (once), in the amulet's place.
    #[test]
    fn a_sure_amulet_comes_with_the_first_cache_only() {
        let mut r = run("mage");
        let content = r.content.clone();
        let sure = super::super::amulets::legendary_for(super::super::amulets::legendary_named(&content, "fireball").unwrap());
        r.heroes[0].first_cache_amulet = Some(sure.clone());
        r.heroes[0].pending_caches = 2;
        r.tick(0.05);
        let screen = r.screen.clone().unwrap();
        assert_eq!(screen.kind, ScreenKind::Cache);
        let k = screen.offers.iter().position(|o| matches!(o, Offer::Amulet { amulet } if amulet.id == sure.id)).expect("the sure amulet is offered");
        assert_eq!(screen.offers.len(), 3);
        let id = r.heroes[0].id;
        assert!(r.choose(id, k));
        assert_eq!(r.heroes[0].amulet.as_ref().map(|a| a.amulet.id.as_str()), Some("legendary:fireball"));
        assert!(r.heroes[0].first_cache_amulet.is_none());
        let next = r.screen.clone().unwrap();
        assert!(!next.offers.iter().any(|o| matches!(o, Offer::Amulet { amulet } if amulet.id == sure.id)), "only the first cache");
    }

    /// A new buff-power piece is offered only to a hero with a spell buff power strengthens (its
    /// race spell counts), a new DoT-stacks piece only to one with a damage-over-time spell.
    #[test]
    fn buff_power_and_dot_stacks_come_only_to_a_hero_with_spells_they_strengthen() {
        use super::super::spells::SpellSlot;
        let mut r = run("warrior");
        let content = r.content.clone();
        let with = |stat: Stat| content.jewelry.iter().find(|j| j.stat == stat).unwrap().id.clone();
        let (buff_piece, dot_piece) = (with(Stat::BuffPower), with(Stat::DotStacks));
        let ability = |pred: &dyn Fn(&Ability) -> bool| content.abilities.iter().find(|a| pred(a)).unwrap().clone();
        let plain = ability(&|a| a.delivery == Delivery::Projectile);
        let dot = ability(&|a| a.delivery == Delivery::DamageOverTime);
        let buff = ability(&strengthened_by_buff_power);
        let new_ids = |r: &mut Run| -> Vec<String> {
            (0..400).flat_map(|_| r.cache_offers(0, ScreenKind::Cache)).filter_map(|o| match o { Offer::NewJewelry { id, .. } => Some(id), _ => None }).collect()
        };
        r.heroes[0].spells.known = vec![SpellSlot::new(&plain, &[])];
        r.heroes[0].spells.racial = None;
        let seen = new_ids(&mut r);
        assert!(!seen.contains(&buff_piece) && !seen.contains(&dot_piece), "neither without a spell they strengthen");
        assert!(!seen.is_empty(), "other new pieces still come");

        r.heroes[0].spells.known.push(SpellSlot::new(&dot, &[]));
        let seen = new_ids(&mut r);
        assert!(seen.contains(&dot_piece) && !seen.contains(&buff_piece), "a DoT spell brings DoT stacks");

        r.heroes[0].spells.racial = Some(SpellSlot::racial(&buff, &[]));
        assert!(new_ids(&mut r).contains(&buff_piece), "a buff, even the race spell, brings buff power");
    }

    /// A hero wears two rings and two trinkets at most: with both ring slots full, no new ring is
    /// offered, though the worn ones still offer their upgrades, and new trinkets still come; then
    /// the same with both trinket slots full.
    #[test]
    fn no_new_ring_or_trinket_is_offered_once_both_of_its_slots_are_full() {
        use esurv_content::JewelrySlot;
        let mut r = run("warrior");
        let content = r.content.clone();
        let of = |slot: JewelrySlot| content.jewelry.iter().filter(move |j| j.slot == slot).map(|j| j.id.clone()).collect::<Vec<_>>();
        let (rings, trinkets) = (of(JewelrySlot::Ring), of(JewelrySlot::Trinket));
        let wear = |r: &mut Run, id: &str| r.heroes[0].jewelry.push(WornJewelry { id: id.into(), value: 0.01, rank: 1 });
        wear(&mut r, &rings[0]);
        wear(&mut r, &rings[1]);
        let offered = |r: &mut Run| (0..400).flat_map(|_| r.cache_offers(0, ScreenKind::Cache)).collect::<Vec<_>>();
        let seen = offered(&mut r);
        let new_ids: Vec<&String> = seen.iter().filter_map(|o| match o { Offer::NewJewelry { id, .. } => Some(id), _ => None }).collect();
        assert!(new_ids.iter().all(|id| !rings.contains(id)), "no new ring with two worn");
        assert!(new_ids.iter().any(|id| trinkets.contains(id)), "new trinkets still come");
        assert!(seen.iter().any(|o| matches!(o, Offer::UpgradeJewelry { id, .. } if *id == rings[0])), "a worn ring still upgrades");

        wear(&mut r, &trinkets[0]);
        wear(&mut r, &trinkets[1]);
        let seen = offered(&mut r);
        assert!(!seen.iter().any(|o| matches!(o, Offer::NewJewelry { .. })), "nothing new with every slot full");
        assert!(seen.iter().any(|o| matches!(o, Offer::UpgradeJewelry { id, .. } if *id == trinkets[1])));

        // with one ring worn, new rings still come
        r.heroes[0].jewelry.retain(|w| w.id != rings[1]);
        let seen = offered(&mut r);
        assert!(seen.iter().any(|o| matches!(o, Offer::NewJewelry { id, .. } if rings.contains(id))));
    }

    /// A new piece never starts past its stat's ceiling, and a Soulstone brings only the lives
    /// still missing.
    #[test]
    fn new_jewelry_starts_within_its_ceiling() {
        let mut r = run("warrior");
        let content = r.content.clone();
        r.heroes[0].stats.luck = 4.0; // mostly epic and legendary rolls
        r.heroes[0].extra_lives = 1;
        let mut seen = 0;
        for _ in 0..400 {
            for o in r.cache_offers(0, ScreenKind::RareCache) {
                let Offer::NewJewelry { id, value, rarity } = o else { continue };
                let j = content.jewelry.iter().find(|j| j.id == id).unwrap();
                match j.stat {
                    Stat::ExtraLives => assert_eq!(value, 1.0, "one life missing"),
                    Stat::DotStacks => {}
                    s => {
                        let (mult, hard) = jewelry_cap(s);
                        let cap = mult.map(|m| m * j.value).or(hard).unwrap_or(f32::INFINITY);
                        assert!(value <= cap + 1e-6, "{id} {rarity:?} {value} over {cap}");
                        seen += usize::from(mult == Some(3.0) && rarity >= Rarity::Epic);
                    }
                }
            }
        }
        assert!(seen > 0, "some 3x pieces rolled epic or better");
    }

    #[test]
    fn a_granted_ring_is_worn_as_its_card_would_wear_it() {
        let mut r = run("warrior");
        let id = r.heroes[0].id;
        let damage = r.heroes[0].stats.damage;
        let ring = Offer::NewJewelry { id: "accuria".into(), value: 0.1, rarity: Rarity::Epic };
        assert!(r.grant(id, ring.clone()));
        assert!((r.heroes[0].stats.damage - damage - 0.1).abs() < 1e-5);
        assert!(r.drain_events().contains(&Event::OfferTaken { hero: id, offer: ring }));
        assert!(!r.grant(id, Offer::Gold(5)), "only jewelry and amulets");
        assert!(r.grant(id, Offer::Amulet { amulet: Box::new(r.content.amulets[0].clone()) }));
        assert_eq!(r.heroes[0].amulet.as_ref().map(|a| a.rank), Some(1));
    }

    #[test]
    fn an_empowered_amulet_grows() {
        let mut r = run("warrior");
        let amulet = r.content.amulets.iter().find(|a| a.effects.iter().any(|e| matches!(e, TalentEffect::Stat { .. }))).unwrap().clone();
        r.heroes[0].amulet = Some(WornAmulet { amulet: amulet.clone(), rank: 1 });
        r.rebuild_hero_modifiers(0);
        let first = r.heroes[0].modifiers.clone();
        r.heroes[0].amulet.as_mut().unwrap().rank = 2;
        r.rebuild_hero_modifiers(0);
        let k = empower_factor(amulet.growth, 2);
        let grew = r.heroes[0].modifiers.iter().zip(&first).any(|(b, a)| (b.value - a.value * k).abs() < 1e-5 && b.value != a.value);
        assert!(grew, "its numbers are multiplied by the empower factor");
    }
}
