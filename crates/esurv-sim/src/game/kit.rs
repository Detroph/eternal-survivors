//! The starting options inside a run (AUTHORING.md "Starting
//! options"): rage, energy and combo points, the passives' counters and procs, blocking, the
//! summons, the weapon swing's procs and the hidden level-1 targeting.
//!
//! The hero's option is looked up from its class (`Hero::start`); its first spell is the main
//! spell, whose card slot carries the option's cards (its power also grows what the passive casts
//! and summons).

use esurv_content::{
    Ability, AutoAttack, Content, Counter, Fill, Mechanic, PassiveKind, Release, Resource, ResourceKind, School, SpellEffect, StackKind,
    StartingOption, Stat, SummonKind, SummonSpec, TargetOrder, Upgrade,
};

use super::abilities::{burn_ticks, duration, hit_damage, radius, target_count, ticks, Instance};
use super::casting::{Dot, HitSource, PendingHit};
use super::enemy_ai::{Segment, Vec2};
use super::run::{Aim, Buff, BuffKind, Event, Hero, Run, Unit};
use super::stats::Modifier;

/// A spell waiting for its rage, energy or combo points looks again this often.
pub const RESOURCE_RETRY: f32 = 0.05;
/// A weapon swing of this many seconds or less is fast (procs that follow the swing use their
/// fast chance).
pub const FAST_SWING: f32 = 2.0;
/// While the hero is this level, every starting spell takes the enemies with the least health
/// first (flat health points: the first kills come quickly). Hidden:
/// tooltips state the spell's usual order.
pub const LEVEL_1_RULE_UNTIL: u32 = 1;
/// A proc that strikes more than one enemy takes the others within this many yards of the first.
pub const PROC_SPREAD: f32 = 8.0;
/// How much farther than the swing's own reach a proc looks for the next enemy when the swing
/// killed its target (enemies a body's width past the swing).
pub const SWING_PROC_REACH: f32 = 2.0;
/// How long a hero in a form is out of it to cast a caster spell (`Event::FormLeft`).
pub const POWERSHIFT_SECONDS: f32 = 0.5;
/// The look of the extra hits `Mechanic::AttacksStrikeMore` makes: a small sweeping slash.
pub const EXTRA_STRIKE_LOOK: u32 = 12723;
/// Combo points stop here (points past a finisher's carry over up to it).
pub const MAX_COMBO_POINTS: u32 = 10;
/// Orbs (Holy Orbs, Demonic Souls) show as a ground look of this many yards.
pub const ORB_LOOK_RADIUS: f32 = 0.6;
/// How far inside the arena's edge a dropped orb stays, so it can always be walked to.
pub const ORB_EDGE_MARGIN: f32 = 3.0;
/// The ground looks of the orbs: a ball of holy light (its model the client's, `option_looks`), a
/// small purple light.
pub const HOLY_ORB_LOOK: u32 = esurv_content::option_looks::HOLY_ORB_LOOK;
pub const SOUL_ORB_LOOK: u32 = 688;
/// Summons: a critter takes a random enemy within this many yards of the hero.
pub const CRITTER_REACH: f32 = 20.0;
/// A melee summon strikes within this many yards.
pub const SUMMON_MELEE_RANGE: f32 = 3.5;
/// A plant walking at its enemy stops this many yards short of it.
pub const PLANT_BURST_RANGE: f32 = 2.0;
/// Plants sprout in a ring this many yards round the hero, walk at
/// their enemies at this many yards a second, and burst where they stand this long after they
/// sprouted.
pub const PLANT_APPEAR_DISTANCE: f32 = 3.0;
pub const PLANT_WALK_SPEED: f32 = super::pets::PET_MIN_SPEED;
pub const PLANT_BURST_AFTER: f32 = 2.0;
/// Tentacles rise anywhere between these many yards from the hero.
pub const TENTACLE_APPEAR_RING: (f32, f32) = (4.0, 8.0);
/// Summons run at this many yards per second.
pub const SUMMON_SPEED: f32 = 12.0;
/// A Water Elemental stays this long (its nova, then its two bolts).
pub const WATER_ELEMENTAL_STAY: f32 = 3.0;
/// The Water Elemental's look: its two little bolts.
pub const WATER_ELEMENTAL_BOLT: u32 = 116;
/// A tentacle or a summon that casts looks for a new target this far from where it stands.
pub const SUMMON_REACH: f32 = 30.0;

/// What a hero's starting option keeps during a run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KitState {
    /// Rage or energy.
    pub resource: f32,
    /// Energy: the time toward the next tick, in seconds at the hero's cooldown pace.
    pub energy_tick: f32,
    pub combo: u32,
    /// The passive's counter (a `Counter`'s fill, or `Empower`'s stacks).
    pub counter: f32,
    /// When each swing speed stack runs out (Zeal).
    pub swing_stacks: Vec<f32>,
    /// When each damage reduction stack runs out (Thick Hide).
    pub reduction_stacks: Vec<f32>,
    /// Quick Shots: the swing is faster until then.
    pub swing_haste_until: f32,
    /// Lock and Load: casts of the main spell left that come at once.
    pub free_casts: u32,
    /// Lock and Load: when it last rolled, so one tick of the shot rolls once, however many
    /// areas the shot made.
    pub free_cast_rolled_at: f32,
    /// Holy Shield, while up.
    pub block: Option<BlockBuff>,
    /// Whose turn it is among the finishers (Rip, then Ferocious Bite …).
    pub finisher_turn: u32,
    /// Whose turn it is among the strikes that build combo points (Claw, then the Cat's Swipe).
    pub builder_turn: u32,
    /// Damage and healing toward the next point of a `Fill::Damage` counter.
    pub damage_fill: f32,
    /// The number of the last drain cast, and of the drain that last gave a shard.
    pub drain_cast: u64,
    pub drain_shard_cast: u64,
    /// The orbs lying on the ground: (look id, where, until).
    pub orbs: Vec<(u32, Vec2, f32)>,
    /// Legendary moments' lasting looks the hero wears: (look, until).
    pub worn_looks: Vec<(u32, f32)>,
    /// What the interface was last told, so a change is told once.
    shown: Vec<(ResourceKind, i32, i32)>,
    shown_stacks: Vec<(StackKind, u32, u32)>,
}

/// Holy Shield's buff.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockBuff {
    pub until: f32,
    pub stacks: u32,
    pub max: u32,
    pub chance: f32,
    /// Damage each block deals to the attacker, before crit.
    pub damage: f32,
    pub spell: String,
}

/// What the starting options left on an enemy.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnemyKit {
    /// Lightning Rod's mark.
    pub marked_until: f32,
    /// Frozen by a passive's own summon (its hits fill nothing; Shatter's freeze).
    pub frozen_by_release_until: f32,
    /// Shatter took its first frost hit.
    pub shattered: bool,
    /// `MarkAttackers`: marked by this hero until then, showing this aura (0: none).
    pub attacker_mark: Option<(usize, f32, u32)>,
    /// `FearedTurn`: fights the other enemies for this hero until then.
    pub turned: Option<(usize, f32)>,
    /// `FireTrail`: where it last left a patch of fire (or first burned), and when it may next.
    pub trail_from: Option<super::enemy_ai::Vec2>,
    pub trail_next: f32,
    /// Legendary moments' lasting looks the enemy wears: (look, until).
    pub worn_looks: Vec<(u32, f32)>,
}

/// A summon of a starting option (it is a pet of the run with this attached).
#[derive(Debug, Clone, PartialEq)]
pub struct Summoned {
    pub kind: SummonKind,
    /// What it hits with (`SummonSpec::spells`).
    pub spells: Vec<String>,
    /// Its power: the main spell's.
    pub inst: Instance,
    /// Brought by the passive's counter (its damage fills nothing).
    pub from_release: bool,
    /// Its next act (a strike, a cast, a tick).
    pub next_act: f32,
    /// A second timer (the Infernal's aura).
    pub next_aura: f32,
    /// What it did so far (the Water Elemental's nova, then its bolts).
    pub step: u32,
    /// Frenzy: faster strikes until then.
    pub frenzy_until: f32,
}

/// A finisher: it spends combo points.
fn is_finisher(a: &Ability) -> bool {
    a.kit.finisher_at > 0
}

/// A strike that builds combo points; two known take turns.
fn is_builder(a: &Ability) -> bool {
    a.kit.combo_points > 0 && a.kit.finisher_at == 0
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// The starting option hero `h` plays.
pub fn option_of<'c>(content: &'c Content, h: &Hero) -> Option<&'c StartingOption> {
    content.classes[h.class].starting_option(Some(&h.start))
}

/// The bar a counter's stacks also show in the buff bar.
fn stack_twin(kind: ResourceKind) -> Option<StackKind> {
    Some(match kind {
        ResourceKind::InnerFire => StackKind::InnerFire,
        ResourceKind::ArcaneKnowledge => StackKind::ArcaneKnowledge,
        ResourceKind::HotStreak => StackKind::HotStreak,
        ResourceKind::MaelstromWeapon => StackKind::MaelstromWeapon,
        ResourceKind::BurningEmbers => StackKind::BurningEmbers,
        _ => return None,
    })
}

/// The swing a hero has: its main hand's (or ranged weapon's, [`swings_of`]).
pub fn swing_of(content: &Content, h: &Hero) -> Option<AutoAttack> {
    swings_of(content, h).into_iter().next()
}

/// The swings a hero has, the main hand (or the ranged weapon) first: a form's own (a
/// form keeps its swing whatever the loadout), else its weapon loadout's, each
/// weapon its own swing time and its share of the class's swing damage, else the class's own.
pub fn swings_of(content: &Content, h: &Hero) -> Vec<AutoAttack> {
    if let Some(own) = option_of(content, h).and_then(|o| o.swing) {
        return vec![own];
    }
    let class = &content.classes[h.class];
    let Some(base) = class.auto_attack else { return Vec::new() };
    let weapons = content.loadout(&class.id, Some(&h.loadout)).map(|l| l.swings()).unwrap_or_default();
    if weapons.is_empty() {
        return if base.swing > 0.0 { vec![base] } else { Vec::new() };
    }
    weapons.into_iter().map(|w| AutoAttack { damage: base.damage * w.share, swing: w.seconds, ..base }).collect()
}

impl Run {
    /// The main spell's card slot of hero `i`.
    fn main_slot(&self, i: usize) -> Option<&super::spells::SpellSlot> {
        let o = option_of(&self.content, &self.heroes[i])?;
        self.heroes[i].spells.known.iter().find(|s| s.spell == o.main_spell())
    }

    /// The main spell's power, as what the passive casts and summons hit with.
    pub(crate) fn release_instance(&self, i: usize) -> Instance {
        self.main_slot(i).map_or_else(Instance::default, |s| {
            let m = s.instance();
            Instance { damage_scale: m.damage_scale, power_from_cards: m.power_from_cards, power_from_talents: m.power_from_talents, ..Instance::default() }
        })
    }

    /// The main spell's cards as a finisher or a poison of the option casts with them: its power
    /// and duration, not its targets.
    fn partner_instance(&self, i: usize) -> Instance {
        self.main_slot(i).map_or_else(Instance::default, |s| Instance { extra_targets_from_cards: 0, extra_targets_from_talents: 0, ..s.instance() })
    }

    /// The main spell's Rage %, Energy % or Chance cards together (1: none), with the Energy %
    /// cards of the other spells that take them (the Cat's Swipe).
    fn gain(&self, i: usize) -> f32 {
        let main = self.main_slot(i).map_or(1.0, |s| s.upgrades.gain * s.mods.gain);
        let Some(o) = option_of(&self.content, &self.heroes[i]) else { return main };
        let others: f32 = self.heroes[i]
            .spells
            .known
            .iter()
            .filter(|s| {
                s.spell != o.main_spell()
                    && self.content.ability(&s.spell).is_some_and(|a| a.upgrades.contains(&Upgrade::EnergyGain))
            })
            .map(|s| s.upgrades.gain * s.mods.gain)
            .product();
        main * others
    }

    /// Whether `a` is one of hero `i`'s starting spells.
    fn is_start_spell(&self, i: usize, a: &Ability) -> bool {
        option_of(&self.content, &self.heroes[i]).is_some_and(|o| o.spells.contains(&a.id))
    }

    /// Whether `a` is hero `i`'s main spell.
    pub(crate) fn is_main(&self, i: usize, spell: &str) -> bool {
        option_of(&self.content, &self.heroes[i]).is_some_and(|o| o.main_spell() == spell)
    }

    /// Tells the interface that hero `i`'s `kind` is now `value` of `max`, if it changed.
    pub(crate) fn show_resource(&mut self, i: usize, kind: ResourceKind, value: f32, max: f32) {
        let key = (value.floor() as i32, max.round() as i32);
        let h = &mut self.heroes[i];
        match h.kit.shown.iter_mut().find(|s| s.0 == kind) {
            Some(s) if (s.1, s.2) == key => return,
            Some(s) => (s.1, s.2) = key,
            None => h.kit.shown.push((kind, key.0, key.1)),
        }
        let hero = h.id;
        self.push(Event::ResourceChanged { hero, kind, value, max });
    }

    /// Tells the interface hero `i`'s `kind` stacks, if they changed.
    fn show_stacks(&mut self, i: usize, kind: StackKind, stacks: u32, max: u32, until: Option<f32>) {
        let h = &mut self.heroes[i];
        match h.kit.shown_stacks.iter_mut().find(|s| s.0 == kind) {
            Some(s) if (s.1, s.2) == (stacks, max) && until.is_none() => return,
            Some(s) => (s.1, s.2) = (stacks, max),
            None => h.kit.shown_stacks.push((kind, stacks, max)),
        }
        let hero = h.id;
        self.push(Event::StacksChanged { hero, kind, stacks, max, until });
    }

    /// The resource bar of hero `i`, told again.
    fn show_kit_resource(&mut self, i: usize) {
        let content = self.content.clone();
        let Some(o) = option_of(&content, &self.heroes[i]) else { return };
        if let Some(r) = o.resource {
            let value = self.heroes[i].kit.resource;
            self.show_resource(i, r.kind(), value, r.cap());
            if let Resource::Energy { combo_points, .. } = r {
                let c = self.heroes[i].kit.combo as f32;
                self.show_resource(i, ResourceKind::ComboPoints, c, combo_points as f32);
            }
        }
    }

    /// Adds `amount` rage or energy to hero `i`, up to its cap.
    pub(crate) fn gain_resource(&mut self, i: usize, amount: f32) {
        let content = self.content.clone();
        let Some(r) = option_of(&content, &self.heroes[i]).and_then(|o| o.resource) else { return };
        let k = &mut self.heroes[i].kit;
        k.resource = (k.resource + amount).clamp(0.0, r.cap());
        self.show_kit_resource(i);
    }

    // ---- casting ---------------------------------------------------------------------------

    /// Whether hero `i` may cast `a` now as far as its option goes: it has the rage, energy or
    /// combo points, it is the finisher's turn, and it is not a spell that is always on.
    pub(crate) fn kit_ready(&self, i: usize, a: &Ability) -> bool {
        let k = &self.heroes[i].kit;
        match &a.kit.mechanic {
            Some(Mechanic::SwingProc { .. } | Mechanic::ExtraSwingHits { .. }) => return false,
            Some(Mechanic::GainEnergy { below, .. }) => return k.resource <= *below,
            _ => {}
        }
        if a.kit.cost > 0.0 && k.resource < a.kit.cost {
            return false;
        }
        if a.kit.finisher_at > 0 {
            if k.combo < a.kit.finisher_at {
                return false;
            }
            return self.has_turn(i, a, k.finisher_turn, is_finisher);
        }
        if is_builder(a) {
            return self.has_turn(i, a, k.builder_turn, is_builder);
        }
        true
    }

    /// Whether it is `a`'s turn among hero `i`'s known spells of its kind (`of`): they take turns
    /// in the order they were learned, `turn` counting their casts so far (Rip, then Ferocious
    /// Bite; Claw, then the Cat's Swipe).
    fn has_turn(&self, i: usize, a: &Ability, turn: u32, of: fn(&Ability) -> bool) -> bool {
        let content = self.content.clone();
        let them: Vec<&str> =
            self.heroes[i].spells.known.iter().filter(|s| content.ability(&s.spell).is_some_and(of)).map(|s| s.spell.as_str()).collect();
        let turn = turn as usize % them.len().max(1);
        them.get(turn).is_some_and(|f| *f == a.id)
    }

    /// The instance hero `i` casts `a` with: a finisher or a poison takes its main spell's cards,
    /// and with Arcane Knowledge's stacks full the next other damaging spell deals more.
    pub(crate) fn kit_instance(&self, i: usize, a: &Ability, inst: Instance) -> Instance {
        let mut inst = inst;
        if a.kit.finisher_at > 0 && self.is_start_spell(i, a) && !self.is_main(i, &a.id) {
            // the main spell's cards, but the finisher's own Duration cards (Rip)
            inst = Instance { duration_from_cards: inst.duration_from_cards, ..self.partner_instance(i) };
        }
        if let Some(PassiveKind::Empower { max, bonus, .. }) = option_of(&self.content, &self.heroes[i]).map(|o| &o.passive.kind) {
            if self.heroes[i].kit.counter >= *max as f32 && a.damage > 0.0 && !self.is_main(i, &a.id) {
                inst.power_from_talents *= 1.0 + bonus;
            }
        }
        inst
    }

    /// After hero `i` cast `a`: it pays the cost, gains or spends combo points, and the main
    /// spell feeds the passive. True when the next cast of the slot comes at once (Lock and Load).
    pub(crate) fn kit_after_cast(&mut self, i: usize, a: &Ability) -> bool {
        let content = self.content.clone();
        let Some(o) = option_of(&content, &self.heroes[i]) else { return false };
        let main = o.main_spell() == a.id;
        let k = &mut self.heroes[i].kit;
        k.resource = (k.resource - a.kit.cost).max(0.0);
        k.combo = (k.combo + a.kit.combo_points).min(MAX_COMBO_POINTS);
        if a.kit.finisher_at > 0 {
            k.combo -= a.kit.finisher_at.min(k.combo);
            k.finisher_turn += 1;
        }
        if is_builder(a) {
            k.builder_turn += 1;
        }
        if let Some(Mechanic::GainEnergy { amount, .. }) = a.kit.mechanic {
            // Rage % and Energy % (cards, talents, the amulet) raise what it gives
            let gain = self.heroes[i].spells.known.iter().find(|s| s.spell == a.id).map_or(1.0, |s| s.upgrades.gain * s.mods.gain);
            self.gain_resource(i, amount * gain);
        }
        if let Some(Mechanic::ReductionStacks { seconds, per_card, base, .. }) = a.kit.mechanic {
            let cards = self.heroes[i].spells.known.iter().find(|s| s.spell == a.id).map_or(0, |s| s.upgrades.taken[Upgrade::Stacks as usize]);
            let max = base + per_card * cards;
            if max > 0 {
                let now = self.now;
                let k = &mut self.heroes[i].kit;
                k.reduction_stacks.retain(|t| *t > now);
                if k.reduction_stacks.len() as u32 >= max {
                    k.reduction_stacks.remove(0);
                }
                k.reduction_stacks.push(now + seconds);
                let n = k.reduction_stacks.len() as u32;
                self.refresh_stats(i);
                self.show_stacks(i, StackKind::ThickHide, n, max, Some(now + seconds));
            }
        }
        // Arcane Knowledge: a damaging spell of another kind used the full stacks (not its own burst)
        if let PassiveKind::Empower { max, .. } = o.passive.kind {
            if !main && a.damage > 0.0 && self.heroes[i].kit.counter >= max as f32 && !o.passive_spells().contains(&a.id.as_str()) {
                self.heroes[i].kit.counter = 0.0;
                self.show_counter(i);
            }
        }
        if main {
            self.fill(i, |f| if let Fill::Cast(n) = f { n } else { 0.0 });
            if let PassiveKind::Empower { max, look, burst, .. } = &o.passive.kind {
                let max = *max;
                // full stacks that burst start over with the next cast (cards cast before it still take them)
                let was = if burst.is_some() && self.heroes[i].kit.counter >= max as f32 { 0.0 } else { self.heroes[i].kit.counter };
                self.heroes[i].kit.counter = (was + 1.0).min(max as f32);
                if was < max as f32 && self.heroes[i].kit.counter >= max as f32 {
                    if let Some(look) = look {
                        self.look_on_hero(i, *look);
                    }
                    let hero = self.heroes[i].id;
                    self.push(Event::PassiveTriggered { hero, passive: o.passive.id.clone() });
                    // the full stacks burst on their own
                    if let Some(b) = burst.as_ref().and_then(|id| content.ability(id)) {
                        let inst = self.release_instance(i);
                        self.cast(i, b, &inst);
                    }
                }
                self.show_counter(i);
            }
        }
        self.show_kit_resource(i);
        if main && self.heroes[i].kit.free_casts > 0 {
            self.heroes[i].kit.free_casts -= 1;
            let left = self.heroes[i].kit.free_casts;
            self.show_stacks(i, StackKind::LockAndLoad, left, 2, None);
            return left > 0;
        }
        false
    }

    // ---- targets ---------------------------------------------------------------------------

    /// How hero `i`'s `a` ranks enemies: boss 0, elite or rare 1, the rest 2.
    fn rank(e: &super::run::EnemyUnit) -> u8 {
        if e.is_boss() {
            0
        } else if e.ranks_as_elite() || e.is_rare() {
            1
        } else {
            2
        }
    }

    /// Whether `a` takes its targets in an order of its own (else the delivery's usual).
    pub(crate) fn kit_orders(&self, i: usize, a: &Ability) -> bool {
        a.kit.target_order != TargetOrder::Usual
            || self.level_1_rule(i, a)
            || matches!(a.kit.mechanic, Some(Mechanic::PrefersMarked { .. } | Mechanic::Poisons { .. }))
            || self.marks_first(i, a)
    }

    fn level_1_rule(&self, i: usize, a: &Ability) -> bool {
        self.heroes[i].progress.level <= LEVEL_1_RULE_UNTIL && self.is_start_spell(i, a)
    }

    fn marks_first(&self, i: usize, a: &Ability) -> bool {
        self.is_main(i, &a.id) && matches!(option_of(&self.content, &self.heroes[i]).map(|o| &o.passive.kind), Some(PassiveKind::Marks { .. }))
    }

    /// Living enemies within `r` of `at` sorted by `key` (a rank, then a number), then nearest.
    fn enemies_by(&self, at: Vec2, r: f32, key: &dyn Fn(&super::run::EnemyUnit) -> (u8, f32)) -> Vec<u32> {
        let now = self.now;
        let mut v: Vec<(u8, f32, f32, u32)> = self
            .enemies
            .iter()
            .map(|e| (e, dist(e.position(now), at)))
            .filter(|(_, d)| *d <= r)
            .map(|(e, d)| (key(e).0, key(e).1, d, e.id))
            .collect();
        v.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)).then(x.2.total_cmp(&y.2)));
        v.into_iter().map(|x| x.3).collect()
    }

    /// Living enemies within `r` of `at` in target order `order`.
    pub(crate) fn enemies_in_order(&mut self, order: TargetOrder, at: Vec2, r: f32) -> Vec<u32> {
        match order {
            TargetOrder::Usual => self.enemies_in_target_order(at, r),
            TargetOrder::Healthiest => self.enemies_by(at, r, &|e| (Self::rank(e), -e.health)),
            TargetOrder::MostWounded => self.enemies_by(at, r, &|e| (0, e.health / e.max_health.max(1.0))),
            TargetOrder::LowestHealth => self.enemies_by(at, r, &|e| (0, e.health)),
            TargetOrder::BossesFirst => self.enemies_by(at, r, &|e| (Self::rank(e), 0.0)),
            TargetOrder::Random => {
                let mut v = self.enemies_by(at, r, &|_| (0, 0.0));
                self.shuffle(&mut v);
                v
            }
        }
    }

    /// Living enemies within `r` of `at` in the order hero `i`'s `a` takes them: its own order
    /// (`SpellKit::target_order`), the hidden level-1 rule, then marked or unpoisoned enemies
    /// first where the spell asks.
    pub(crate) fn kit_targets(&mut self, i: usize, a: &Ability, at: Vec2, r: f32) -> Vec<u32> {
        let now = self.now;
        let mut order = if self.level_1_rule(i, a) {
            self.enemies_by(at, r, &|e| (0, e.health))
        } else {
            self.enemies_in_order(a.kit.target_order, at, r)
        };
        let first = |order: &mut Vec<u32>, pick: &dyn Fn(&super::run::EnemyUnit) -> bool| {
            let (mut yes, no): (Vec<u32>, Vec<u32>) = order.iter().partition(|id| self.enemies.iter().find(|e| e.id == **id).is_some_and(pick));
            yes.extend(no);
            *order = yes;
        };
        match &a.kit.mechanic {
            Some(Mechanic::PrefersMarked { .. }) => first(&mut order, &|e| super::run::EnemyUnit::active(e.vulnerable, now).is_some()),
            Some(Mechanic::Poisons { poison }) => first(&mut order, &|e| !e.dots.iter().any(|d| &d.spell == poison && d.ticks_left > 0)),
            _ => {}
        }
        if self.marks_first(i, a) {
            first(&mut order, &|e| e.kit.marked_until > now);
        }
        order
    }

    // ---- what spells do --------------------------------------------------------------------

    /// Casts `a` for hero `i` when its own rule replaces its delivery; `None` when the delivery
    /// does it.
    pub(crate) fn kit_cast(&mut self, i: usize, a: &Ability, inst: &Instance) -> Option<bool> {
        let mechanic = a.kit.mechanic.clone()?;
        let now = self.now;
        let at = self.heroes[i].pos;
        let stats = self.heroes[i].stats;
        Some(match mechanic {
            Mechanic::BlockShield { block_chance, stacks } => {
                if self.enemies_within(at, super::abilities::BUFF_ENEMY_RANGE).is_empty() {
                    return Some(false);
                }
                let max = stacks + inst.extra_targets_from_cards + inst.extra_targets_from_talents;
                let until = now + duration(a, inst);
                let damage = hit_damage(a, inst, stats.damage);
                self.heroes[i].kit.block = Some(BlockBuff { until, stacks: max, max, chance: block_chance, damage, spell: a.id.clone() });
                let me = Unit::Hero(self.heroes[i].id);
                self.push(Event::SpellGo { caster: me, spell: a.look_spell, hits: vec![me], aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
                self.show_stacks(i, StackKind::HolyShield, max, max, Some(until));
                self.show_resource(i, ResourceKind::HolyShieldStacks, max as f32, max as f32);
                true
            }
            Mechanic::GainEnergy { .. } => {
                let me = Unit::Hero(self.heroes[i].id);
                self.push(Event::SpellGo { caster: me, spell: a.look_spell, hits: vec![me], aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
                true
            }
            Mechanic::StrikesAfflicted { spell } => {
                let targets: Vec<u32> = self
                    .enemies_within(at, a.range)
                    .into_iter()
                    .filter(|id| self.enemy(*id).is_some_and(|e| e.dots.iter().any(|d| d.spell == spell && d.ticks_left > 0)))
                    .collect();
                if targets.is_empty() {
                    return Some(false);
                }
                self.show_cast(i, a, &targets, Aim::Caster, a.range);
                let hit = hit_damage(a, inst, stats.damage);
                for t in targets {
                    let (h, crit) = self.roll_crit(i, hit);
                    self.hit_enemy(Some(i), t, h, crit, HitSource::Spell(a.id.clone()));
                }
                true
            }
            Mechanic::PetsStrike => {
                let hit = hit_damage(a, inst, stats.damage);
                let pets: Vec<(u32, Vec2, Option<u32>)> = self
                    .pets
                    .iter()
                    .filter(|p| p.hero == i && p.summon.as_ref().is_some_and(|s| s.kind == SummonKind::HunterPet))
                    .map(|p| (p.id, p.segment.position(now), p.target))
                    .collect();
                let mut struck = false;
                for (id, pos, target) in pets {
                    let target = target.filter(|t| self.enemy(*t).is_some()).or_else(|| self.nearest_to(pos, a.range).first().copied());
                    let Some(t) = target else { continue };
                    let enemy = Unit::Enemy(t);
                    self.push(Event::SpellGo { caster: Unit::Pet(id), spell: a.look_spell, hits: vec![enemy], aim: Aim::Unit(enemy), ammo: false, reach: 0.0, triggered: false });
                    let (h, crit) = self.roll_crit(i, hit);
                    self.hit_enemy(Some(i), t, h, crit, HitSource::Spell(a.id.clone()));
                    struck = true;
                }
                struck
            }
            Mechanic::Returns => self.cast_returning(i, a, inst),
            Mechanic::Summons(spec) if a.delivery == esurv_content::Delivery::SelfEffect => {
                if self.enemies_within(at, CRITTER_REACH).is_empty() {
                    return Some(false);
                }
                self.show_cast(i, a, &[], Aim::Caster, 0.0);
                let summon_inst = Instance { duration_from_cards: inst.duration_from_cards, ..self.release_instance(i) };
                let extra = inst.extra_targets_from_cards + inst.extra_targets_from_talents + self.more_summons(i, a);
                self.summon(i, &spec, summon_inst, false, at, extra, duration(a, inst));
                true
            }
            Mechanic::AttacksStrikeMore => {
                if self.enemies_within(at, super::abilities::BUFF_ENEMY_RANGE).is_empty() {
                    return Some(false);
                }
                let more = a.targets + inst.extra_targets_from_cards + inst.extra_targets_from_talents;
                let until = now + duration(a, inst);
                let h = &mut self.heroes[i];
                h.buffs.retain(|b| b.kind != BuffKind::ExtraStrikes);
                h.buffs.push(Buff { kind: BuffKind::ExtraStrikes, value: more as f32, visual: a.look_spell, until });
                let me = Unit::Hero(self.heroes[i].id);
                self.push(Event::SpellGo { caster: me, spell: a.look_spell, hits: vec![me], aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
                true
            }
            Mechanic::FreshTargetFirst => {
                let targets = self.kit_targets(i, a, at, a.range);
                // an enemy not yet bleeding from it first, in the spell's own order
                let hero = i;
                let bleeding = |e: u32, en: &[super::run::EnemyUnit]| {
                    en.iter().any(|x| x.id == e && x.dots.iter().any(|d| d.hero == hero && d.spell == a.id))
                };
                let Some(t) = targets.iter().copied().find(|&t| !bleeding(t, &self.enemies)).or(targets.first().copied()) else {
                    return Some(false);
                };
                let (ticks_left, per_tick) = ticks(a, inst, hit_damage(a, inst, stats.damage));
                let interval = a.duration.max(0.01) / a.ticks.max(1) as f32;
                let from_proc = self.casting_proc > 0;
                self.show_cast(i, a, &[t], Aim::Unit(Unit::Enemy(t)), 0.0);
                self.add_dot(t, Dot { hero, spell: a.id.clone(), per_tick, ticks_left, interval, next: now + interval, from_proc, leech: 0.0 });
                for x in self.extra_strikes(i, t, &[t]) {
                    self.add_dot(x, Dot { hero, spell: a.id.clone(), per_tick, ticks_left, interval, next: now + interval, from_proc, leech: 0.0 });
                }
                true
            }
            Mechanic::Poisons { poison } => {
                let n = target_count(a, inst, stats.extra_targets) as usize;
                let targets: Vec<u32> = self.kit_targets(i, a, at, a.range).into_iter().take(n).collect();
                if targets.is_empty() {
                    return Some(false);
                }
                self.show_cast(i, a, &targets, Aim::Unit(Unit::Enemy(targets[0])), 0.0);
                let hit = hit_damage(a, inst, stats.damage);
                let content = self.content.clone();
                let p = content.ability(&poison);
                for t in targets {
                    let (h, crit) = self.roll_crit(i, hit);
                    if self.hit_enemy(Some(i), t, h, crit, HitSource::Spell(a.id.clone())).is_some() {
                        if let Some(p) = p {
                            let pinst = Instance { extra_targets_from_cards: 0, extra_targets_from_talents: 0, ..*inst };
                            let (count, per_tick) = ticks(p, &pinst, hit_damage(p, &pinst, stats.damage));
                            let interval = p.duration.max(0.01) / p.ticks.max(1) as f32;
                            for e in &mut self.enemies {
                                if e.id == t {
                                    e.dots.retain(|d| !(d.hero == i && d.spell == p.id));
                                }
                            }
                            let from_proc = self.casting_proc > 0;
                            self.add_dot(t, Dot { hero: i, spell: p.id.clone(), per_tick, ticks_left: count, interval, next: now + interval, from_proc, leech: 0.0 });
                        }
                    }
                }
                true
            }
            _ => return None,
        })
    }

    /// While hero `i`'s `AttacksStrikeMore` buff is up: the enemies its attack on `first` also
    /// strikes, the nearest to `first` within `PROC_SPREAD` yd that are not in `struck`, as many
    /// as the buff gives (none without it). Shows their look.
    pub(crate) fn extra_strikes(&mut self, i: usize, first: u32, struck: &[u32]) -> Vec<u32> {
        let now = self.now;
        let Some(n) = self.heroes[i].buffs.iter().find(|b| b.kind == BuffKind::ExtraStrikes && b.until > now).map(|b| b.value as usize) else {
            return Vec::new();
        };
        let Some(at) = self.enemy_pos(first) else { return Vec::new() };
        let more: Vec<u32> = self.nearest_to(at, PROC_SPREAD).into_iter().filter(|id| !struck.contains(id)).take(n).collect();
        if !more.is_empty() {
            let hits = more.iter().map(|&id| Unit::Enemy(id)).collect();
            let aim = Aim::Unit(Unit::Enemy(more[0]));
            self.push(Event::SpellGo { caster: Unit::Hero(self.heroes[i].id), spell: EXTRA_STRIKE_LOOK, hits, aim, ammo: false, reach: 0.0, triggered: true });
        }
        more
    }

    /// Divine Star: out toward the first target and back, striking each enemy near its path once
    /// each way.
    fn cast_returning(&mut self, i: usize, a: &Ability, inst: &Instance) -> bool {
        let at = self.heroes[i].pos;
        let reach = a.range * inst.radius_from_cards * inst.radius_from_talents;
        let width = radius(a, inst, self.heroes[i].stats.area);
        let Some(&first) = self.kit_targets(i, a, at, reach).first() else { return false };
        let Some(tp) = self.enemy_pos(first) else { return false };
        let d = dist(at, tp).max(0.01);
        let dir = ((tp.0 - at.0) / d, (tp.1 - at.1) / d);
        // a second star a milestone throws flies the other way (`CastAgain` opposite)
        let dir = if self.procs.reverse_return { (-dir.0, -dir.1) } else { dir };
        let end = (at.0 + dir.0 * reach, at.1 + dir.1 * reach);
        let now = self.now;
        let on_path: Vec<(u32, f32)> = self
            .enemies
            .iter()
            .filter_map(|e| {
                let p = e.position(now);
                let along = ((p.0 - at.0) * dir.0 + (p.1 - at.1) * dir.1).clamp(0.0, reach);
                let closest = (at.0 + dir.0 * along, at.1 + dir.1 * along);
                (dist(p, closest) <= width).then_some((e.id, along))
            })
            .collect();
        self.push(Event::SpellGo { caster: Unit::Hero(self.heroes[i].id), spell: a.look_spell, hits: vec![], aim: Aim::Ground(end), ammo: false, reach: width, triggered: false });
        let hit = hit_damage(a, inst, self.heroes[i].stats.damage);
        // it flies out, then back, at its look's missile speed (so its hits land as the star is
        // seen passing), else at `reach` yards a second
        let look_speed = self.world.missile_speed(a.look_spell);
        let speed = if look_speed > 0.0 { look_speed } else { reach.max(0.01) };
        for (id, along) in on_path {
            for at_time in [along / speed, (2.0 * reach - along) / speed] {
                let (damage, crit) = self.roll_crit(i, hit);
                let cast = Some(self.casts_made);
                self.hits.push(PendingHit { hero: i, target: id, at: now + at_time, damage, crit, spell: a.id.clone(), dot: None, pet: None, from_proc: self.casting_proc > 0, cast, burn: None });
            }
        }
        true
    }

    /// A look on hero `i` itself.
    fn look_on_hero(&mut self, i: usize, spell: u32) {
        let me = Unit::Hero(self.heroes[i].id);
        self.push(Event::SpellGo { caster: me, spell, hits: vec![me], aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
    }

    // ---- the passive -----------------------------------------------------------------------

    /// The counter of hero `i`, told again.
    fn show_counter(&mut self, i: usize) {
        let content = self.content.clone();
        let Some(o) = option_of(&content, &self.heroes[i]) else { return };
        let value = self.heroes[i].kit.counter;
        match &o.passive.kind {
            PassiveKind::Counter(c) => {
                self.show_resource(i, c.meter, value, c.max);
                if let Some(s) = stack_twin(c.meter) {
                    self.show_stacks(i, s, value.floor() as u32, c.max as u32, None);
                }
            }
            PassiveKind::Empower { max, .. } => {
                self.show_resource(i, ResourceKind::ArcaneKnowledge, value, *max as f32);
                self.show_stacks(i, StackKind::ArcaneKnowledge, value as u32, *max, None);
            }
            _ => {}
        }
    }

    /// Adds to hero `i`'s counter what `each` says each fill source gives; a full counter is
    /// released and starts over.
    pub(crate) fn fill(&mut self, i: usize, each: impl Fn(Fill) -> f32) {
        let content = self.content.clone();
        let Some(o) = option_of(&content, &self.heroes[i]) else { return };
        let PassiveKind::Counter(c) = &o.passive.kind else { return };
        let add: f32 = c.fill.iter().map(|f| each(*f)).sum();
        if add <= 0.0 {
            return;
        }
        self.heroes[i].kit.counter += add;
        if self.heroes[i].kit.counter + 1e-4 >= c.max {
            self.heroes[i].kit.counter = 0.0;
            self.show_counter(i);
            let hero = self.heroes[i].id;
            self.push(Event::PassiveTriggered { hero, passive: o.passive.id.clone() });
            self.release(i, c);
        } else {
            self.show_counter(i);
        }
    }

    /// A full counter of hero `i` goes off.
    fn release(&mut self, i: usize, c: &Counter) {
        let content = self.content.clone();
        if let Some(look) = c.look {
            self.look_on_hero(i, look);
        }
        let inst = self.release_instance(i);
        match &c.release {
            Release::Spell(id) => {
                if let Some(a) = content.ability(id) {
                    self.cast(i, a, &inst);
                }
            }
            Release::Summon(list) => {
                let at = self.heroes[i].pos;
                for spec in list {
                    self.summon(i, spec, inst, true, at, 0, spec.seconds);
                }
                // Demonic Souls: the orbs on the floor coalesce on the hero
                let orbs = std::mem::take(&mut self.heroes[i].kit.orbs);
                for (id, _, _) in orbs {
                    self.push(Event::AreaGone { id });
                }
            }
        }
    }

    /// A hit of hero `h` landed on enemy `id` for `dealt` (before the kill): it feeds the passive.
    pub(crate) fn kit_on_hit(&mut self, h: usize, id: u32, source: &HitSource, dealt: f32, crit: bool) {
        let content = self.content.clone();
        let Some(o) = option_of(&content, &self.heroes[h]) else { return };
        let (spell, tick) = match source {
            HitSource::Spell(s) => (s.as_str(), false),
            HitSource::Tick(s) => (s.as_str(), true),
            _ => ("", false),
        };
        // what the passive itself casts or summons fills nothing
        let own = !spell.is_empty() && !o.spells.iter().any(|s| s == spell) && o.all_spells().contains(&spell);
        let from_release = matches!(source, HitSource::Pet(p) if self.pets.iter().any(|x| x.id == *p && x.summon.as_ref().is_some_and(|s| s.from_release)));
        // `main_only`: the level-1 spells and what their summons strike with (Friend of the Forest's
        // critters hit as `critter_strike`)
        let summoned_by_level_one = |id: &str| {
            o.spells.iter().filter_map(|x| content.ability(x)).any(|a| matches!(&a.kit.mechanic, Some(Mechanic::Summons(spec)) if spec.spells.iter().any(|s| s == id)))
        };
        let level_one = o.spells.iter().any(|s| s == spell)
            || summoned_by_level_one(spell)
            || matches!(source, HitSource::Pet(p) if self.pets.iter().any(|x| x.id == *p && x.summon.as_ref().is_some_and(|s| !s.from_release)));
        let main_only = matches!(&o.passive.kind, PassiveKind::Counter(c) if c.fill.iter().any(|f| matches!(f, Fill::Damage { main_only: true, .. })));
        if !own && !from_release && (!main_only || level_one) {
            self.fill_damage(h, dealt, 0.0);
        }
        let main = spell == o.main_spell();
        if !main || tick {
            return;
        }
        let now = self.now;
        let Some(e) = self.enemy(id) else { return };
        let (held, by_release, marked) = (now < e.hold_until, e.kit.frozen_by_release_until > now, e.kit.marked_until > now);
        self.fill(h, |f| match f {
            Fill::Crit(n) if crit => n,
            Fill::FrozenHit(n) if held && !by_release => n,
            _ => 0.0,
        });
        match o.passive.kind.clone() {
            PassiveKind::SwingSpeedOnHit { chance, seconds, .. } => {
                if self.rng.unit() < chance {
                    self.heroes[h].kit.swing_haste_until = now + seconds;
                    self.show_stacks(h, StackKind::QuickShots, 1, 1, Some(now + seconds));
                }
            }
            PassiveKind::Marks { chance, seconds, share, radius, spell: rod } => {
                if marked {
                    let at = self.enemy_pos(id).unwrap_or(self.heroes[h].pos);
                    let others: Vec<u32> = self.enemies_within(at, radius).into_iter().filter(|x| *x != id).collect();
                    if let Some(a) = content.ability(&rod) {
                        let me = Unit::Hero(self.heroes[h].id);
                        let hits = others.iter().map(|x| Unit::Enemy(*x)).collect();
                        self.push(Event::SpellGo { caster: me, spell: a.look_spell, hits, aim: Aim::Unit(Unit::Enemy(id)), ammo: false, reach: radius, triggered: false });
                    }
                    for x in others {
                        self.hit_enemy(Some(h), x, dealt * share, false, HitSource::Spell(rod.clone()));
                    }
                }
                if self.rng.unit() < chance {
                    if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                        e.kit.marked_until = now + seconds;
                    }
                }
            }
            _ => {}
        }
    }

    /// Damage or healing toward a `Fill::Damage` counter (healing counts its `heal_factor` times).
    fn fill_damage(&mut self, h: usize, amount: f32, heal: f32) {
        let content = self.content.clone();
        let Some(PassiveKind::Counter(c)) = option_of(&content, &self.heroes[h]).map(|o| &o.passive.kind) else { return };
        let Some((per, factor)) = c.fill.iter().find_map(|f| if let Fill::Damage { per, heal_factor, .. } = f { Some((*per, *heal_factor)) } else { None }) else {
            return;
        };
        let weight = if heal > 0.0 { factor } else { 1.0 };
        let k = &mut self.heroes[h].kit;
        k.damage_fill += amount * weight;
        let points = (k.damage_fill / per).floor();
        if points >= 1.0 {
            k.damage_fill -= points * per;
            self.fill(h, |f| if let Fill::Damage { .. } = f { points } else { 0.0 });
        }
    }

    /// Hero `i` was healed `healed` (no overhealing) by its own spells.
    pub(crate) fn kit_on_heal(&mut self, i: usize, healed: f32) {
        if healed > 0.0 {
            self.fill_damage(i, healed, 1.0);
        }
    }

    /// One tick of hero `h`'s lasting area `spell` went off (Lock and Load): one roll per tick of
    /// the shot, however many areas it made (Targets cards, double casts), and none for the free
    /// shots Lock and Load itself gave (`from_proc`).
    pub(crate) fn kit_on_area_tick(&mut self, h: usize, spell: &str, from_proc: bool) {
        if from_proc || !self.is_main(h, spell) || (self.now - self.heroes[h].kit.free_cast_rolled_at).abs() < 1e-4 {
            return;
        }
        self.heroes[h].kit.free_cast_rolled_at = self.now;
        let content = self.content.clone();
        if let Some(PassiveKind::FreeCasts { chance, charges }) = option_of(&content, &self.heroes[h]).map(|o| o.passive.kind.clone()) {
            if self.rng.unit() < chance {
                self.heroes[h].kit.free_casts = charges;
                if let Some(s) = self.heroes[h].spells.known.iter_mut().find(|s| s.spell == spell) {
                    s.timer = s.timer.min(0.0);
                }
                let hero = self.heroes[h].id;
                self.push(Event::PassiveTriggered { hero, passive: option_of(&content, &self.heroes[h]).unwrap().passive.id.clone() });
                self.show_stacks(h, StackKind::LockAndLoad, charges, charges, None);
            }
        }
    }

    /// Enemy `e` died of hero `h`'s doing: a shard for an enemy under the drain, and the drain
    /// goes on to the next enemy.
    pub(crate) fn kit_on_kill(&mut self, h: usize, e: &super::run::EnemyUnit) {
        let content = self.content.clone();
        let Some(o) = option_of(&content, &self.heroes[h]) else { return };
        let main = o.main_spell();
        let Some(d) = e.dots.iter().find(|d| d.hero == h && d.spell == main && d.ticks_left > 0).cloned() else { return };
        let cast = self.heroes[h].kit.drain_cast;
        if self.heroes[h].kit.drain_shard_cast != cast {
            let before = self.heroes[h].kit.counter;
            self.fill(h, |f| if let Fill::KillUnderSpell(n) = f { n } else { 0.0 });
            if self.heroes[h].kit.counter != before || self.heroes[h].kit.counter == 0.0 {
                self.heroes[h].kit.drain_shard_cast = cast;
            }
        }
        let Some(a) = content.ability(main) else { return };
        if matches!(a.kit.mechanic, Some(Mechanic::Drain)) {
            let at = self.heroes[h].pos;
            let next = self.kit_targets(h, a, at, a.range).into_iter().find(|id| *id != e.id);
            if let Some(t) = next {
                let enemy = Unit::Enemy(t);
                self.push(Event::SpellGo { caster: Unit::Hero(self.heroes[h].id), spell: a.look_spell, hits: vec![enemy], aim: Aim::Unit(enemy), ammo: false, reach: 0.0, triggered: false });
                self.add_dot(t, d);
            }
        }
    }

    /// Hero `i` cast its drain: a new drain for the one-shard rule.
    pub(crate) fn kit_on_drain_cast(&mut self, i: usize, a: &Ability) {
        if matches!(a.kit.mechanic, Some(Mechanic::Drain)) {
            self.heroes[i].kit.drain_cast += 1;
        }
    }

    /// Hero `i` lost `amount` health: rage.
    pub(crate) fn kit_on_hurt(&mut self, i: usize, amount: f32) {
        let content = self.content.clone();
        if let Some(Resource::Rage { per_health_lost, .. }) = option_of(&content, &self.heroes[i]).and_then(|o| o.resource) {
            let max = self.heroes[i].stats.max_health.max(1.0);
            let gain = self.gain(i) * (1.0 + self.heroes[i].stats.rage_when_hurt);
            self.gain_resource(i, amount / max * per_health_lost * gain);
        }
    }

    /// An enemy's melee blow at hero `i` (by `by`) is blocked or not (a blocked blow does
    /// `BLOCK_REDUCTION` less damage); a block of Holy Shield uses a stack and strikes the
    /// attacker. True when blocked.
    pub(crate) fn kit_block(&mut self, i: usize, by: u32) -> bool {
        let now = self.now;
        let shield = self.heroes[i].kit.block.clone().filter(|b| b.until > now && b.stacks > 0);
        let chance = self.heroes[i].stats.block_chance + shield.as_ref().map_or(0.0, |b| b.chance);
        if chance <= 0.0 || self.rng.unit() >= chance {
            return false;
        }
        let hero = self.heroes[i].id;
        self.push(Event::Blocked { hero, by });
        // Holy Shield strikes first (its charge is used on the blow), then Block's Reckoning on
        // an attacker still standing: the passive must not take the shield's kill
        if let Some(b) = shield {
            let left = b.stacks - 1;
            self.heroes[i].kit.block = (left > 0).then(|| BlockBuff { stacks: left, ..b.clone() });
            if left == 0 {
                self.milestone_event(i, super::milestones::CombatEvent::BuffEnded { spell: b.spell.clone() });
            }
            self.show_stacks(i, StackKind::HolyShield, left, b.max, Some(b.until));
            self.show_resource(i, ResourceKind::HolyShieldStacks, left as f32, b.max as f32);
            let (h, crit) = self.roll_crit(i, b.damage);
            self.hit_enemy(Some(i), by, h, crit, HitSource::Spell(b.spell.clone()));
        }
        let content = self.content.clone();
        if let Some(PassiveKind::BlockStrikes { spell }) = option_of(&content, &self.heroes[i]).map(|o| &o.passive.kind) {
            if let (Some(a), true) = (content.ability(spell), self.enemy(by).is_some_and(|e| e.health > 0.0)) {
                let inst = self.release_instance(i);
                let (h, crit) = self.roll_crit(i, hit_damage(a, &inst, self.heroes[i].stats.damage));
                self.hit_enemy(Some(i), by, h, crit, HitSource::Spell(a.id.clone()));
            }
        }
        true
    }

    // ---- the weapon swing ------------------------------------------------------------------

    /// How much harder hero `i`'s swing hits now (1 = as usual): Quick Shots' bonus while it is up.
    pub(crate) fn swing_damage_factor(&self, i: usize) -> f32 {
        let h = &self.heroes[i];
        match option_of(&self.content, h).map(|o| &o.passive.kind) {
            Some(PassiveKind::SwingSpeedOnHit { damage_bonus, .. }) if h.kit.swing_haste_until > self.now => 1.0 + damage_bonus,
            _ => 1.0,
        }
    }

    /// How much faster hero `i` swings now: its stat, Zeal's stacks, Quick Shots.
    pub(crate) fn swing_speed(&self, i: usize) -> f32 {
        let now = self.now;
        let h = &self.heroes[i];
        let mut speed = h.stats.swing_speed;
        speed += h.buffs.iter().filter(|b| b.kind == BuffKind::SwingSpeed && b.until > now).map(|b| b.value).sum::<f32>();
        if let Some(o) = option_of(&self.content, h) {
            match o.passive.kind {
                PassiveKind::SwingSpeedStacks { amount, .. } => speed += amount * h.kit.swing_stacks.iter().filter(|t| **t > now).count() as f32,
                PassiveKind::SwingSpeedOnHit { amount, .. } if h.kit.swing_haste_until > now => speed += amount,
                _ => {}
            }
        }
        speed
    }

    /// Hero `i`'s swing hit enemy `target`: rage, the swing's procs, Maelstrom Weapon.
    pub(crate) fn kit_on_swing(&mut self, i: usize, target: u32, swing: &AutoAttack) {
        let content = self.content.clone();
        let Some(o) = option_of(&content, &self.heroes[i]) else { return };
        if let Some(Resource::Rage { per_swing, .. }) = o.resource {
            // the two-hander loadout's rage per swing
            let gain = self.gain(i) * (1.0 + self.heroes[i].stats.rage_per_swing).max(0.0);
            self.gain_resource(i, per_swing * gain);
        }
        // Zeal: each stack adds a strike of its spell to the swing
        if let PassiveKind::SwingSpeedStacks { spell: Some(sp), .. } = &o.passive.kind {
            let now = self.now;
            let n = self.heroes[i].kit.swing_stacks.iter().filter(|t| **t > now).count();
            if let (true, Some(a)) = (n > 0, content.ability(sp)) {
                let inst = self.release_instance(i);
                let hit = hit_damage(a, &inst, self.heroes[i].stats.damage) * n as f32;
                let (h, crit) = self.roll_crit(i, hit);
                self.hit_enemy(Some(i), target, h, crit, HitSource::Spell(a.id.clone()));
            }
        }
        self.melee_hit_fill(i, swing);
        let known: Vec<(String, Instance, u32)> = self.heroes[i].spells.known.iter().map(|s| (s.spell.clone(), s.instance(), s.upgrades.extra_targets)).collect();
        for (id, inst, extra) in known {
            let Some(a) = content.ability(&id) else { continue };
            let gain = self.heroes[i].spells.known.iter().find(|s| s.spell == id).map_or(1.0, |s| s.upgrades.gain * s.mods.gain);
            match a.kit.mechanic {
                Some(Mechanic::SwingProc { fast_chance, slow_chance, base_swing }) => {
                    let base = if swing.swing <= FAST_SWING { fast_chance } else { slow_chance };
                    let chance = base * gain;
                    let procs = chance.floor() as u32 + u32::from(self.rng.unit() < chance.fract());
                    let hit = hit_damage(a, &inst, self.heroes[i].stats.damage) * swing.swing / base_swing * fast_chance / base;
                    for _ in 0..procs {
                        // the swing's target, or the nearest living enemy in reach if the swing killed it
                        let Some(target) = self.swing_target_or_next(i, target, swing) else { break };
                        let n = target_count(a, &inst, self.heroes[i].stats.extra_targets) as usize;
                        let at = self.enemy_pos(target).unwrap_or(self.heroes[i].pos);
                        let mut targets = vec![target];
                        targets.extend(self.enemies_within(at, PROC_SPREAD).into_iter().filter(|x| *x != target).take(n.saturating_sub(1)));
                        targets.retain(|t| self.enemy(*t).is_some());
                        self.show_cast(i, a, &targets, Aim::Unit(Unit::Enemy(target)), 0.0);
                        for t in targets {
                            let (h, crit) = self.roll_crit(i, hit);
                            self.hit_enemy(Some(i), t, h, crit, HitSource::Spell(a.id.clone()));
                        }
                        self.add_swing_stack(i);
                    }
                }
                Some(Mechanic::ExtraSwingHits { chance, hits, base_swing }) => {
                    let chance = chance * gain;
                    let procs = chance.floor() as u32 + u32::from(self.rng.unit() < chance.fract());
                    let hit = hit_damage(a, &inst, self.heroes[i].stats.damage) * swing.swing / base_swing;
                    for _ in 0..procs {
                        let Some(first) = self.swing_target_or_next(i, target, swing) else { break };
                        self.show_cast(i, a, &[first], Aim::Unit(Unit::Enemy(first)), 0.0);
                        for _ in 0..hits + extra {
                            // each extra hit on the swing's target, or the nearest living enemy in reach once it died
                            let Some(t) = self.swing_target_or_next(i, target, swing) else { break };
                            let (h, crit) = self.roll_crit(i, hit);
                            self.hit_enemy(Some(i), t, h, crit, HitSource::Spell(a.id.clone()));
                            self.melee_hit_fill(i, swing);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// The enemy a swing's proc strikes: the swing's target while it lives, else the living enemy
    /// nearest the hero within the swing's reach (a swing that kills its target keeps its procs).
    fn swing_target_or_next(&self, i: usize, target: u32, swing: &AutoAttack) -> Option<u32> {
        if self.enemy(target).is_some() {
            return Some(target);
        }
        self.enemies_within(self.heroes[i].pos, swing.range + SWING_PROC_REACH).first().copied()
    }

    /// A melee hit's chance at a Maelstrom Weapon stack.
    fn melee_hit_fill(&mut self, i: usize, swing: &AutoAttack) {
        let roll = self.rng.unit();
        let fast = swing.swing <= FAST_SWING;
        self.fill(i, |f| match f {
            Fill::MeleeHit { fast: c, .. } if fast && roll < c => 1.0,
            Fill::MeleeHit { slow: c, .. } if !fast && roll < c => 1.0,
            _ => 0.0,
        });
    }

    /// Zeal: a swing speed stack of its own.
    fn add_swing_stack(&mut self, i: usize) {
        let content = self.content.clone();
        let Some(PassiveKind::SwingSpeedStacks { seconds, max, .. }) = option_of(&content, &self.heroes[i]).map(|o| o.passive.kind.clone()) else { return };
        let now = self.now;
        let k = &mut self.heroes[i].kit;
        k.swing_stacks.retain(|t| *t > now);
        if (k.swing_stacks.len() as u32) < max {
            k.swing_stacks.push(now + seconds);
        }
        let n = k.swing_stacks.len() as u32;
        self.show_stacks(i, StackKind::Zeal, n, max, Some(now + seconds));
        self.show_resource(i, ResourceKind::Zeal, n as f32, max as f32);
    }

    /// The stat modifiers hero `i`'s option gives now: its form's, Thick Hide's stacks.
    pub(crate) fn kit_modifiers(&self, i: usize) -> Vec<Modifier> {
        let now = self.now;
        let h = &self.heroes[i];
        let mut v = Vec::new();
        for s in &h.spells.known {
            if let Some(Mechanic::ReductionStacks { amount, .. }) = self.content.ability(&s.spell).and_then(|a| a.kit.mechanic.clone()) {
                let n = h.kit.reduction_stacks.iter().filter(|t| **t > now).count();
                v.push(Modifier::new(Stat::DamageReduction, amount * n as f32));
            }
        }
        v
    }

    // ---- every tick ------------------------------------------------------------------------

    /// Energy refills, counters that fill with time, stacks and buffs run out, orbs are picked
    /// up, the hunter's pets come and the summons act.
    pub(crate) fn kit_tick(&mut self, dt: f32) {
        let content = self.content.clone();
        let now = self.now;
        for i in 0..self.heroes.len() {
            if !self.heroes[i].alive {
                continue;
            }
            let Some(o) = option_of(&content, &self.heroes[i]) else { continue };
            if let Some(Resource::Energy { per_tick, tick_every, .. }) = o.resource {
                // 1.12's ticks: Energy % cards make each bigger, faster cooldowns bring them sooner
                let pace = self.heroes[i].stats.cooldown_factor.max(0.01);
                let k = &mut self.heroes[i].kit;
                k.energy_tick += dt / pace;
                let mut ticks = 0;
                while k.energy_tick >= tick_every.max(0.01) {
                    k.energy_tick -= tick_every.max(0.01);
                    ticks += 1;
                }
                if ticks > 0 {
                    let gain = self.gain(i);
                    self.gain_resource(i, ticks as f32 * per_tick * gain);
                }
            }
            self.fill(i, |f| if let Fill::Time(n) = f { n * dt } else { 0.0 });
            // Zeal and Thick Hide stacks that run out
            let k = &mut self.heroes[i].kit;
            let (zeal, hide) = (k.swing_stacks.len(), k.reduction_stacks.len());
            k.swing_stacks.retain(|t| *t > now);
            k.reduction_stacks.retain(|t| *t > now);
            let (zeal_now, hide_now) = (k.swing_stacks.len(), k.reduction_stacks.len());
            if zeal != zeal_now {
                if let PassiveKind::SwingSpeedStacks { max, .. } = o.passive.kind {
                    self.show_stacks(i, StackKind::Zeal, zeal_now as u32, max, None);
                    self.show_resource(i, ResourceKind::Zeal, zeal_now as f32, max as f32);
                }
            }
            if hide != hide_now {
                self.refresh_stats(i);
                self.show_stacks(i, StackKind::ThickHide, hide_now as u32, hide as u32, None);
            }
            if self.heroes[i].kit.block.as_ref().is_some_and(|b| b.until <= now) {
                let max = self.heroes[i].kit.block.take().map_or(0, |b| b.max);
                self.show_stacks(i, StackKind::HolyShield, 0, max, None);
                self.show_resource(i, ResourceKind::HolyShieldStacks, 0.0, max as f32);
            }
            if self.heroes[i].kit.swing_haste_until > 0.0 && self.heroes[i].kit.swing_haste_until <= now {
                self.heroes[i].kit.swing_haste_until = 0.0;
                self.show_stacks(i, StackKind::QuickShots, 0, 1, None);
            }
            self.pick_up_orbs(i);
            // the hunter's pets: one, and one more for each Pet card
            if let Some(spec) = &o.pet {
                let extra = self.main_slot(i).map_or(0, |s| s.upgrades.extra_targets + s.mods.targets);
                let have = self.pets.iter().filter(|p| p.hero == i && p.summon.as_ref().is_some_and(|s| s.kind == spec.kind)).count() as u32;
                let want = spec.count + extra;
                if have < want && self.phase == super::run::Phase::Running {
                    let one = SummonSpec { count: 1, pets: spec.pets.iter().skip(have as usize).chain(spec.pets.iter()).take(1).cloned().collect(), ..spec.clone() };
                    let inst = self.release_instance(i);
                    let at = self.heroes[i].pos;
                    self.summon(i, &one, inst, false, at, 0, 0.0);
                }
            }
        }
        // summons keep their power up to date with the main spell's cards
        for k in 0..self.pets.len() {
            if self.pets[k].summon.is_some() {
                let i = self.pets[k].hero;
                let inst = self.release_instance(i);
                if let Some(s) = self.pets[k].summon.as_mut() {
                    if !s.from_release {
                        s.inst = Instance { duration_from_cards: s.inst.duration_from_cards, ..inst };
                    }
                }
            }
        }
    }

    /// Holy Orbs hero `i` walks over (within their `touch`, not its pickup range) are taken: each
    /// casts its spell.
    fn pick_up_orbs(&mut self, i: usize) {
        let content = self.content.clone();
        let Some(PassiveKind::Orbs { spell, touch, .. }) = option_of(&content, &self.heroes[i]).map(|o| o.passive.kind.clone()) else { return };
        let (now, at) = (self.now, self.heroes[i].pos);
        let orbs = std::mem::take(&mut self.heroes[i].kit.orbs);
        let stay = self.orbs_stay;
        let (taken, kept): (Vec<_>, Vec<_>) = orbs.into_iter().filter(|o| o.2 > now).partition(|o| !stay && dist(o.1, at) <= touch);
        self.heroes[i].kit.orbs = kept;
        for (id, _, _) in taken {
            self.push(Event::AreaGone { id });
            let hero = self.heroes[i].id;
            self.push(Event::PassiveTriggered { hero, passive: option_of(&content, &self.heroes[i]).unwrap().passive.id.clone() });
            if let Some(a) = content.ability(&spell) {
                let inst = self.main_slot(i).map_or_else(Instance::default, |s| s.instance());
                self.cast(i, a, &inst);
            }
        }
    }

    /// Hero `i` cast its main spell: Holy Orbs may drop an orb.
    pub(crate) fn kit_on_cast(&mut self, i: usize, a: &Ability) {
        if !self.is_main(i, &a.id) {
            return;
        }
        let content = self.content.clone();
        if let Some(PassiveKind::Orbs { chance, distance, lasts, .. }) = option_of(&content, &self.heroes[i]).map(|o| o.passive.kind.clone()) {
            if self.rng.unit() < chance {
                // spread evenly over the ring's area, then kept a step inside the arena's edge
                let angle = self.rng.range(0.0, std::f32::consts::TAU);
                let d = self.rng.range(distance.0 * distance.0, distance.1 * distance.1).sqrt();
                let p = self.heroes[i].pos;
                let at = self.inside_arena((p.0 + d * angle.cos(), p.1 + d * angle.sin()));
                let id = self.new_id();
                let until = self.now + lasts;
                self.heroes[i].kit.orbs.push((id, at, until));
                let caster = Unit::Hero(self.heroes[i].id);
                self.push(Event::AreaShown { id, caster, spell: HOLY_ORB_LOOK, at, radius: ORB_LOOK_RADIUS, until: Some(until) });
            }
        }
    }

    /// `at`, brought a step inside the arena's edge if it lies beyond it.
    fn inside_arena(&self, at: Vec2) -> Vec2 {
        let (centre, radius) = self.arena();
        let inside = (radius - ORB_EDGE_MARGIN).max(0.0);
        let out = dist(at, centre);
        if out > inside {
            let k = inside / out;
            (centre.0 + (at.0 - centre.0) * k, centre.1 + (at.1 - centre.1) * k)
        } else {
            at
        }
    }

    // ---- summons ---------------------------------------------------------------------------

    /// Brings hero `i`'s summons of `spec` (`extra` more than it says) near `at`, for `seconds`
    /// (0: the whole run).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn summon(&mut self, i: usize, spec: &SummonSpec, inst: Instance, from_release: bool, at: Vec2, extra: u32, seconds: f32) {
        let now = self.now;
        let until = (seconds > 0.0).then_some(now + seconds);
        let hero = self.heroes[i].id;
        // plants each take a different enemy
        let mut free_enemies = if spec.kind == SummonKind::Plant { self.nearest_to(at, SUMMON_REACH) } else { Vec::new() };
        let count = spec.count + extra;
        let mut first_angle = None;
        for n in 0..count {
            let model = (!spec.pets.is_empty()).then(|| spec.pets[n as usize % spec.pets.len()].clone());
            let angle = self.rng.range(0.0, std::f32::consts::TAU);
            let spot = match spec.kind {
                SummonKind::Clone => at,
                // plants sprout evenly spaced in a ring round the hero
                SummonKind::Plant => {
                    let a = *first_angle.get_or_insert(angle) + n as f32 * std::f32::consts::TAU / count.max(1) as f32;
                    (at.0 + PLANT_APPEAR_DISTANCE * a.cos(), at.1 + PLANT_APPEAR_DISTANCE * a.sin())
                }
                // tentacles rise anywhere in a ring round the hero, never on it
                SummonKind::EyeTentacle | SummonKind::ClawTentacle => {
                    let (near, far) = TENTACLE_APPEAR_RING;
                    let d = self.rng.range(near * near, far * far).sqrt();
                    self.inside_arena((at.0 + d * angle.cos(), at.1 + d * angle.sin()))
                }
                _ => (at.0 + 2.5 * angle.cos(), at.1 + 2.5 * angle.sin()),
            };
            let id = self.new_id();
            let target = match spec.kind {
                SummonKind::Plant if !free_enemies.is_empty() => Some(free_enemies.remove(0)),
                _ => None,
            };
            self.pets.push(super::pets::PetUnit {
                id,
                hero: i,
                pet: model.clone().unwrap_or_default(),
                slot_angle: super::pets::PET_FOLLOW_ANGLES[n as usize % super::pets::PET_FOLLOW_ANGLES.len()],
                until,
                segment: Segment::standing(spot, now),
                next_decision: now,
                next_swing: now,
                target,
                assist: None,
                summon: Some(Summoned {
                    kind: spec.kind,
                    spells: spec.spells.clone(),
                    inst,
                    from_release,
                    // a plant's next act is its burst
                    next_act: now + if spec.kind == SummonKind::Plant { PLANT_BURST_AFTER } else { 0.5 },
                    next_aura: now + 1.0,
                    step: 0,
                    frenzy_until: 0.0,
                }),
            });
            if let Some(pet) = model {
                self.push(Event::PetSummoned { id, hero, pet, at: spot });
            }
            self.push(Event::SummonAppeared { id, hero, kind: spec.kind, at: spot, until });
        }
    }

    /// A summon left (its time ran out, or it burst): Demonic Souls' orb, a counter's fill.
    pub(crate) fn summon_left(&mut self, p: &super::pets::PetUnit, expired: bool) {
        let Some(s) = &p.summon else { return };
        let i = p.hero;
        if expired && !s.from_release && i < self.heroes.len() && self.heroes[i].alive {
            let before = self.heroes[i].kit.counter;
            self.fill(i, |f| if let Fill::SummonExpired(n) = f { n } else { 0.0 });
            // Demonic Souls: the soul lies where the imp was until the counter fills
            if self.heroes[i].kit.counter > before {
                let id = self.new_id();
                let at = p.segment.position(self.now);
                self.heroes[i].kit.orbs.push((id, at, f32::MAX));
                let caster = Unit::Hero(self.heroes[i].id);
                self.push(Event::AreaShown { id, caster, spell: SOUL_ORB_LOOK, at, radius: ORB_LOOK_RADIUS, until: None });
            }
        }
        self.push(Event::SummonGone { id: p.id });
    }

    /// What summon `k` does this decision.
    pub(crate) fn summon_tick(&mut self, k: usize) {
        let now = self.now;
        let content = self.content.clone();
        let p = self.pets[k].clone();
        let Some(s) = p.summon.clone() else { return };
        let i = p.hero;
        let pos = p.segment.position(now);
        let spell = |n: usize| s.spells.get(n).and_then(|id| content.ability(id));
        let damage = self.heroes[i].stats.damage;
        match s.kind {
            SummonKind::Clone => {
                if now >= s.next_act && !s.spells.is_empty() {
                    let pick = (self.rng.next_u64() % s.spells.len() as u64) as usize;
                    if let Some(a) = spell(pick) {
                        self.cast_by_summon = Some((self.heroes[i].id, Unit::Pet(p.id)));
                        self.cast(i, a, &s.inst);
                        self.cast_by_summon = None;
                        self.set_next_act(k, now + a.cooldown.max(0.1));
                    }
                }
            }
            SummonKind::EyeTentacle | SummonKind::ClawTentacle => {
                let Some(a) = spell(0) else { return };
                if now < s.next_act {
                    return;
                }
                let order = if s.kind == SummonKind::EyeTentacle { TargetOrder::Healthiest } else { TargetOrder::MostWounded };
                // the eye flays from afar; the claw claws only what stands next to it
                let claw = s.kind == SummonKind::ClawTentacle;
                let reach = if claw { a.range } else { SUMMON_REACH };
                let keep = p.target.filter(|t| self.enemy_pos(*t).is_some_and(|tp| !claw || dist(pos, tp) <= reach));
                let target = keep.or_else(|| {
                    let probe = Ability { kit: esurv_content::SpellKit { target_order: order, ..Default::default() }, ..a.clone() };
                    let mut ranked = self.kit_targets_from(&probe, pos, reach);
                    (!ranked.is_empty()).then(|| ranked.remove(0))
                });
                self.pets[k].target = target;
                self.set_next_act(k, now + a.cooldown.max(0.1));
                if let Some(t) = target {
                    let (h, crit) = self.roll_crit(i, hit_damage(a, &s.inst, damage));
                    if claw {
                        // its own melee attack, as a melee pet swings
                        self.hit_enemy(Some(i), t, h, crit, HitSource::Pet(p.id));
                    } else {
                        let enemy = Unit::Enemy(t);
                        self.push(Event::SpellGo { caster: Unit::Pet(p.id), spell: a.look_spell, hits: vec![enemy], aim: Aim::Unit(enemy), ammo: false, reach: 0.0, triggered: false });
                        self.hit_enemy(Some(i), t, h, crit, HitSource::Spell(a.id.clone()));
                    }
                }
            }
            SummonKind::Imp => {
                let Some(a) = spell(0) else { return };
                if now < s.next_act {
                    return;
                }
                let Some(&t) = self.nearest_to(pos, a.range).first() else { return };
                self.set_next_act(k, now + a.cooldown.max(0.1));
                let enemy = Unit::Enemy(t);
                self.push(Event::SpellGo { caster: Unit::Pet(p.id), spell: a.look_spell, hits: vec![enemy], aim: Aim::Unit(enemy), ammo: false, reach: 0.0, triggered: false });
                let (h, crit) = self.roll_crit(i, hit_damage(a, &s.inst, damage));
                let flight = match (self.world.missile_speed(a.look_spell), self.enemy_pos(t)) {
                    (v, Some(tp)) if v > 0.0 => dist(pos, tp) / v,
                    _ => 0.0,
                };
                let spell = a.id.clone();
                self.hits.push(PendingHit { hero: i, target: t, at: now + flight, damage: h, crit, spell, dot: None, pet: None, from_proc: true, cast: None, burn: None });
            }
            SummonKind::WaterElemental => {
                let Some(a) = spell(0) else { return };
                if s.step == 0 {
                    // Frost Nova at the nearest pack: around the enemy nearest the hero
                    let hero_pos = self.heroes[i].pos;
                    if let Some(&first) = self.nearest_to(hero_pos, SUMMON_REACH).first() {
                        let centre = self.enemy_pos(first).unwrap_or(hero_pos);
                        let r = radius(a, &s.inst, 1.0);
                        let hits = self.enemies_within(centre, r);
                        let units = hits.iter().map(|x| Unit::Enemy(*x)).collect();
                        self.push(Event::SpellGo { caster: Unit::Pet(p.id), spell: a.look_spell, hits: units, aim: Aim::Ground(centre), ammo: false, reach: r, triggered: false });
                        for t in hits {
                            let (h, crit) = self.roll_crit(i, hit_damage(a, &s.inst, damage));
                            if let Some(d) = self.hit_enemy(Some(i), t, h, crit, HitSource::Spell(a.id.clone())) {
                                self.apply_on_hit(i, t, &a.effect, d);
                                if let Some(e) = self.enemies.iter_mut().find(|e| e.id == t) {
                                    e.kit.frozen_by_release_until = e.hold_until;
                                    e.kit.shattered = false;
                                }
                            }
                        }
                    }
                    self.set_step(k, 1, now + 0.8);
                } else if s.step < 3 && now >= s.next_act {
                    // two little bolts, for the look only
                    if let Some(&t) = self.nearest_to(pos, SUMMON_REACH).first() {
                        let enemy = Unit::Enemy(t);
                        self.push(Event::SpellGo { caster: Unit::Pet(p.id), spell: WATER_ELEMENTAL_BOLT, hits: vec![enemy], aim: Aim::Unit(enemy), ammo: false, reach: 0.0, triggered: false });
                    }
                    self.set_step(k, s.step + 1, now + 0.8);
                }
            }
            SummonKind::Infernal | SummonKind::Critter | SummonKind::HunterPet | SummonKind::Plant => self.melee_summon_tick(k, &s, pos),
        }
    }

    fn set_next_act(&mut self, k: usize, at: f32) {
        if let Some(s) = self.pets[k].summon.as_mut() {
            s.next_act = at;
        }
    }

    fn set_step(&mut self, k: usize, step: u32, next: f32) {
        if let Some(s) = self.pets[k].summon.as_mut() {
            s.step = step;
            s.next_act = next;
        }
    }

    /// Living enemies near `at` in `a`'s own order, for a summon (no hero rules).
    fn kit_targets_from(&mut self, a: &Ability, at: Vec2, r: f32) -> Vec<u32> {
        let now = self.now;
        let mut v: Vec<(u8, f32, u32)> = self
            .enemies
            .iter()
            .filter(|e| dist(e.position(now), at) <= r)
            .map(|e| match a.kit.target_order {
                TargetOrder::Healthiest => (Self::rank(e), -e.health, e.id),
                TargetOrder::MostWounded => (0, e.health / e.max_health.max(1.0), e.id),
                _ => (0, dist(e.position(now), at), e.id),
            })
            .collect();
        v.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)));
        v.into_iter().map(|x| x.2).collect()
    }

    /// A summon that runs at enemies: the Infernal (swing and aura), a critter (a random enemy
    /// until it dies), the hunter's pet, a plant (walks at its enemy and bursts where it stands
    /// [`PLANT_BURST_AFTER`] after it sprouted).
    fn melee_summon_tick(&mut self, k: usize, s: &Summoned, pos: Vec2) {
        let now = self.now;
        let content = self.content.clone();
        let p = self.pets[k].clone();
        let i = p.hero;
        let damage = self.heroes[i].stats.damage;
        let hero_pos = self.heroes[i].pos;
        let alive = |run: &Run, t: u32| run.enemy(t).is_some();
        let target = match p.target.filter(|t| alive(self, *t)) {
            Some(t) => Some(t),
            None if s.kind == SummonKind::Plant => {
                // its enemy died on the way: the nearest
                self.nearest_to(pos, SUMMON_REACH).first().copied()
            }
            None if s.kind == SummonKind::Critter => {
                // the hidden level-1 rule: the least health first, then at random
                let mut near = self.nearest_to(hero_pos, CRITTER_REACH);
                if self.heroes[i].progress.level <= LEVEL_1_RULE_UNTIL {
                    near.sort_by(|x, y| self.enemy(*x).map_or(0.0, |e| e.health).total_cmp(&self.enemy(*y).map_or(0.0, |e| e.health)));
                } else {
                    self.shuffle(&mut near);
                }
                near.first().copied()
            }
            None => self.nearest_to(pos, SUMMON_REACH).first().copied(),
        };
        self.pets[k].target = target;
        // the Infernal's aura burns around it every second
        if s.kind == SummonKind::Infernal && now >= s.next_aura {
            if let Some(aura) = s.spells.get(1).and_then(|id| content.ability(id)) {
                let r = radius(aura, &s.inst, 1.0);
                let hits = self.area_targets(aura, pos, r);
                let units = hits.iter().map(|x| Unit::Enemy(*x)).collect();
                self.push(Event::SpellGo { caster: Unit::Pet(p.id), spell: aura.look_spell, hits: units, aim: Aim::Caster, ammo: false, reach: r, triggered: false });
                for t in hits {
                    let (h, crit) = self.roll_crit(i, hit_damage(aura, &s.inst, damage));
                    self.hit_enemy(Some(i), t, h, crit, HitSource::Spell(aura.id.clone()));
                }
            }
            if let Some(x) = self.pets[k].summon.as_mut() {
                x.next_aura = now + 1.0;
            }
        }
        // a plant bursts where it stands once its time comes
        if s.kind == SummonKind::Plant && now >= s.next_act {
            if let Some(a) = s.spells.first().and_then(|id| content.ability(id)) {
                let r = radius(a, &s.inst, 1.0);
                let hits = self.area_targets(a, pos, r);
                let units = hits.iter().map(|x| Unit::Enemy(*x)).collect();
                self.push(Event::SpellGo { caster: Unit::Pet(p.id), spell: a.look_spell, hits: units, aim: Aim::Ground(pos), ammo: false, reach: r, triggered: false });
                for x in hits {
                    let (h, crit) = self.roll_crit(i, hit_damage(a, &s.inst, damage));
                    self.hit_enemy(Some(i), x, h, crit, HitSource::Spell(a.id.clone()));
                }
            }
            // it burst: gone at the next look
            self.pets[k].until = Some(now);
            return;
        }
        let Some(t) = target else {
            if s.kind == SummonKind::Plant {
                // no enemy near: it waits where it stands
                return;
            }
            let a = p.slot_angle + self.heroes[i].facing;
            let place = (hero_pos.0 + a.cos() * super::pets::PET_FOLLOW_DISTANCE, hero_pos.1 + a.sin() * super::pets::PET_FOLLOW_DISTANCE);
            self.move_pet(k, pos, place, SUMMON_SPEED);
            return;
        };
        let tp = self.enemy_pos(t).unwrap_or(pos);
        let d = dist(pos, tp);
        let Some(a) = s.spells.first().and_then(|id| content.ability(id)) else { return };
        if s.kind == SummonKind::Plant {
            // it walks at its enemy, stopping short of it
            if d > PLANT_BURST_RANGE {
                let short = PLANT_BURST_RANGE / d;
                let stop = (tp.0 + (pos.0 - tp.0) * short, tp.1 + (pos.1 - tp.1) * short);
                self.move_pet(k, pos, stop, PLANT_WALK_SPEED);
            }
            return;
        }
        if d <= SUMMON_MELEE_RANGE && now >= s.next_act {
            // a critter's hit: its share of what it deals over its time; others: the spell's hit
            let (hit, interval) = if s.kind == SummonKind::Critter {
                let (n, per) = ticks(a, &s.inst, hit_damage(a, &s.inst, damage));
                let _ = n;
                (per, a.duration.max(0.01) / a.ticks.max(1) as f32)
            } else {
                let frenzy = if s.frenzy_until > now { self.frenzy_amount(i) } else { 0.0 };
                (hit_damage(a, &s.inst, damage), a.cooldown.max(0.1) / (1.0 + frenzy))
            };
            self.set_next_act(k, now + interval);
            let enemy = Unit::Enemy(t);
            if a.look_spell != 0 {
                self.push(Event::SpellGo { caster: Unit::Pet(p.id), spell: a.look_spell, hits: vec![enemy], aim: Aim::Unit(enemy), ammo: false, reach: 0.0, triggered: false });
            }
            let (h, crit) = self.roll_crit(i, hit);
            self.hit_enemy(Some(i), t, h, crit, HitSource::Spell(a.id.clone()));
            if crit && s.kind == SummonKind::HunterPet {
                self.frenzy(i, k);
            }
        }
        if d > SUMMON_MELEE_RANGE * 0.7 {
            let short = 1.5 / d.max(0.01);
            let stop = (tp.0 + (pos.0 - tp.0) * short, tp.1 + (pos.1 - tp.1) * short);
            self.move_pet(k, pos, stop, SUMMON_SPEED);
        }
    }

    fn frenzy_amount(&self, i: usize) -> f32 {
        match option_of(&self.content, &self.heroes[i]).map(|o| &o.passive.kind) {
            Some(PassiveKind::PetFrenzy { amount, .. }) => *amount,
            _ => 0.0,
        }
    }

    /// Frenzy: pet `k`'s crit makes it strike faster for a while.
    fn frenzy(&mut self, i: usize, k: usize) {
        let content = self.content.clone();
        let Some(PassiveKind::PetFrenzy { seconds, .. }) = option_of(&content, &self.heroes[i]).map(|o| o.passive.kind.clone()) else { return };
        let until = self.now + seconds;
        if let Some(s) = self.pets[k].summon.as_mut() {
            s.frenzy_until = until;
        }
        self.show_stacks(i, StackKind::Frenzy, 1, 1, Some(until));
    }

    /// Frost hits on an enemy a passive's summon froze: the first deals `shatter` times as much.
    pub(crate) fn shatter(&mut self, h: usize, k: usize, source: &HitSource) -> f32 {
        let content = self.content.clone();
        let Some(PassiveKind::Counter(c)) = option_of(&content, &self.heroes[h]).map(|o| &o.passive.kind) else { return 1.0 };
        if c.shatter <= 0.0 {
            return 1.0;
        }
        let frost = match source {
            HitSource::Spell(s) => content.ability(s).is_some_and(|a| a.school == School::Frost),
            _ => false,
        };
        let e = &mut self.enemies[k];
        if frost && e.kit.frozen_by_release_until > self.now && !e.kit.shattered {
            e.kit.shattered = true;
            return c.shatter;
        }
        1.0
    }

    /// The form's stat modifiers of hero setup's option.
    pub fn form_modifiers(o: &StartingOption) -> Vec<Modifier> {
        o.form.as_ref().map_or_else(Vec::new, |f| {
            vec![Modifier::new(Stat::FlatMaxHealth, f.bonus_health), Modifier::new(Stat::DamageReduction, f.bonus_damage_reduction)]
        })
    }
}

/// Whether a spell's effect is a Vulnerable debuff (for tests and cards).
pub fn is_vulnerable(effect: &SpellEffect) -> bool {
    matches!(effect, SpellEffect::Vulnerable { .. })
}

/// The burn ticks a spell would leave (kept here for the passives' spells that burn).
pub fn burn_of(a: &Ability, inst: &Instance, hero_damage: f32) -> Option<super::abilities::BurnTicks> {
    burn_ticks(a, inst, hero_damage)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::Content;

    use super::*;
    use crate::game::enemy::RunOptions;
    use crate::game::run::{Flat, HeroSetup};

    fn content() -> Arc<Content> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        Arc::new(Content::load(dir).expect("content loads"))
    }

    /// The weapon loadout these tests give a class whose first one changes the timings or health
    /// they measure: one that leaves cast times, cooldowns and the swing alone (and max health, but
    /// for the Druid's and the Shaman's: each of their three changes it).
    const QUIET_LOADOUTS: [(&str, &str); 5] =
        [("mage", "sword_off_hand"), ("warlock", "sword_off_hand"), ("priest", "mace_off_hand"), ("shaman", "shield"), ("druid", "staff")];

    /// A run of `class` started with option `start`, no enemies, no race spell, the hero past
    /// level 1 (so the hidden level-1 rule is off) unless `level_1`.
    pub(crate) fn run_with(class: &str, start: &str, level_1: bool) -> Run {
        let loadout = QUIET_LOADOUTS.iter().find(|(c, _)| *c == class).map(|(_, l)| *l);
        run_armed(class, start, level_1, loadout)
    }

    /// [`run_with`] with weapon loadout `loadout` (none: the class's first).
    pub(crate) fn run_armed(class: &str, start: &str, level_1: bool, loadout: Option<&str>) -> Run {
        let race = match class {
            "shaman" => "orc",
            "druid" => "night_elf",
            _ => "human",
        };
        let loadout = loadout.map(str::to_owned);
        let setup = HeroSetup { class: class.into(), race: race.into(), start: Some(start.into()), loadout, ..Default::default() };
        let mut r = Run::new(content(), "elwynn", RunOptions::default(), &[setup], Box::new(Flat), 7).unwrap();
        let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
        r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        r.enemies.clear();
        r.pickups.clear();
        r.heroes[0].spells.racial = None;
        r.heroes[0].stats.crit_chance = 0.0;
        if !level_1 {
            r.heroes[0].progress.level = 2;
        }
        r.drain_events();
        r
    }

    /// An enemy that stands still `offset` from the hero, with `health`.
    pub(crate) fn foe(r: &mut Run, offset: Vec2, health: f32) -> u32 {
        let p = r.heroes[0].pos;
        assert!(r.spawn_roster_enemy(0, Some((p.0 + offset.0, p.1 + offset.1))));
        let e = r.enemies.last_mut().unwrap();
        e.health = health;
        e.max_health = health;
        e.base_speed = 0.0;
        e.damage = 0.0;
        e.id
    }

    /// Ticks for `seconds`, taking the first offer of any screen; the events.
    /// Only the enemies there at the start stay: what the run spawns meanwhile is taken away.
    pub(crate) fn play(r: &mut Run, seconds: f32) -> Vec<Event> {
        let mut out = Vec::new();
        let steps = (seconds / 0.05).round() as u32;
        let keep: Vec<u32> = r.enemies.iter().map(|e| e.id).collect();
        for _ in 0..steps {
            r.tick(0.05);
            r.enemies.retain(|e| keep.contains(&e.id));
            r.take_first_offers();
            out.extend(r.drain_events());
        }
        out
    }

    /// Damage each spell dealt in `events`.
    /// Spell `id`'s damage as the content gives it (tests follow balance passes).
    fn base_damage(r: &Run, id: &str) -> f32 {
        r.content.ability(id).unwrap().damage
    }

    /// The starting option `start`'s passive kind as the content gives it.
    fn passive_of(r: &Run, start: &str) -> PassiveKind {
        r.content.starting_option(start).unwrap().passive.kind.clone()
    }

    pub(crate) fn dealt_by(events: &[Event], spell: &str) -> Vec<f32> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::EnemyHit { amount, source: HitSource::Spell(s) | HitSource::Tick(s), .. } if s == spell => Some(*amount),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn every_class_has_three_starting_options_and_the_druid_four() {
        let c = content();
        for class in &c.classes {
            let want = if class.id == "druid" { 4 } else { 3 };
            assert_eq!(class.starting_options.len(), want, "{}", class.id);
        }
    }

    #[test]
    fn a_hero_plays_the_option_it_picked_and_the_first_by_default() {
        let c = content();
        for class in &c.classes {
            for o in &class.starting_options {
                let r = run_with(&class.id, &o.id, false);
                assert_eq!(r.heroes[0].start, o.id);
                assert_eq!(r.heroes[0].spells.known[0].spell, o.main_spell(), "{}", o.id);
            }
            let setup = HeroSetup { class: class.id.clone(), race: "human".into(), ..Default::default() };
            if let Some(r) = Run::new(c.clone(), "elwynn", RunOptions::default(), &[setup], Box::new(Flat), 1) {
                assert_eq!(r.heroes[0].start, class.starting_options[0].id);
            }
        }
    }

    #[test]
    fn rage_comes_with_each_swing_and_slam_spends_sixty_on_a_cone_in_front() {
        let mut r = run_with("warrior", "slam", false);
        let ahead = foe(&mut r, (3.0, 0.0), 1e9);
        let behind = foe(&mut r, (-3.0, 0.0), 1e9);
        let Some(Resource::Rage { per_swing, cap, .. }) = option_of(&r.content, &r.heroes[0]).and_then(|o| o.resource) else { panic!("the slam warrior has rage") };
        let ev = play(&mut r, 1.0);
        // the first swing: its rage, no slam yet
        assert!(ev.iter().any(|e| matches!(e, Event::ResourceChanged { kind: ResourceKind::Rage, value, max, .. } if *value == per_swing * (1.0 + r.heroes[0].stats.rage_per_swing) && *max == cap)));
        assert!(dealt_by(&ev, "slam").is_empty());
        let ev = play(&mut r, 20.0);
        let slams: Vec<&Event> = ev.iter().filter(|e| matches!(e, Event::EnemyHit { source: HitSource::Spell(s), .. } if s == "slam")).collect();
        assert!(!slams.is_empty());
        for e in slams {
            let Event::EnemyHit { id, amount, .. } = e else { unreachable!() };
            assert_eq!(*id, ahead, "only the enemy in front");
            assert!((amount - base_damage(&r, "slam") * r.heroes[0].stats.damage).abs() < 0.01);
        }
        let _ = behind;
        assert!(r.heroes[0].kit.resource < r.content.ability("slam").unwrap().kit.cost + per_swing);
    }

    #[test]
    fn rage_comes_from_health_lost_and_stops_at_a_hundred() {
        let mut r = run_with("warrior", "whirlwind", false);
        let max = r.heroes[0].stats.max_health;
        let reduction = r.heroes[0].stats.damage_reduction;
        let Some(Resource::Rage { per_health_lost, cap, .. }) = option_of(&r.content, &r.heroes[0]).and_then(|o| o.resource) else { panic!("the whirlwind warrior has rage") };
        // 10 % of max health lost: a tenth of the rage for all of it
        r.damage_hero(0, 0.1 * max / (1.0 - reduction), None, false);
        assert!((r.heroes[0].kit.resource - 0.1 * per_health_lost).abs() < 1e-3, "{}", r.heroes[0].kit.resource);
        r.gain_resource(0, 500.0);
        assert_eq!(r.heroes[0].kit.resource, cap);
    }

    #[test]
    fn whirlwind_spends_fifty_five_on_everything_close_and_revenge_fifty_on_five() {
        let mut r = run_with("warrior", "whirlwind", false);
        let near: Vec<u32> = (0..4).map(|k| foe(&mut r, (2.0 + k as f32, 1.0), 1e9)).collect();
        r.heroes[0].kit.resource = 55.0;
        let ev = play(&mut r, 0.6);
        let hits = dealt_by(&ev, "whirlwind");
        assert_eq!(hits.len(), near.len());
        assert!(hits.iter().all(|h| (h - base_damage(&r, "whirlwind") * r.heroes[0].stats.damage).abs() < 0.01));
        let mut r = run_with("warrior", "revenge", false);
        for k in 0..5 {
            foe(&mut r, (2.0 + k as f32 * 0.5, 0.0), 1e9);
        }
        r.heroes[0].kit.resource = 50.0;
        let ev = play(&mut r, 0.6);
        assert_eq!(dealt_by(&ev, "revenge").len(), 5);
    }

    #[test]
    fn at_level_1_a_starting_spell_takes_the_lowest_health_first() {
        let mut r = run_with("warrior", "revenge", true);
        let big = foe(&mut r, (1.5, 0.0), 500.0);
        let small = foe(&mut r, (5.0, 0.0), 40.0);
        let _ = big;
        let mut a = r.content.ability("revenge").unwrap().clone();
        a.targets = 1;
        let inst = r.heroes[0].spells.known[0].instance();
        r.cast(0, &a, &inst);
        let ev = r.drain_events();
        assert!(ev.iter().any(|e| matches!(e, Event::EnemyHit { id, .. } | Event::EnemyDied { id, .. } if *id == small)));
    }

    #[test]
    fn denounce_starts_on_an_elite_and_every_fifth_cast_sets_off_radiance() {
        let mut r = run_with("paladin", "denounce", false);
        let near = foe(&mut r, (3.0, 0.0), 1e9);
        let elite = foe(&mut r, (20.0, 0.0), 1e9);
        r.enemies.last_mut().unwrap().elite = true;
        let ev = play(&mut r, 0.6);
        let first = ev.iter().find_map(|e| if let Event::EnemyHit { id, source: HitSource::Spell(s), .. } = e { (s == "denounce").then_some(*id) } else { None });
        assert_eq!(first, Some(elite));
        let _ = near;
        // four more casts (every 4 s): the fifth fills Holy Power and Radiance goes off
        let ev = play(&mut r, 16.5);
        assert!(ev.iter().any(|e| matches!(e, Event::PassiveTriggered { passive, .. } if passive == "holy_power")));
        let radiance = dealt_by(&ev, "radiance");
        assert_eq!(radiance.len(), 1, "the enemy within 10 yd");
        assert_eq!(r.heroes[0].kit.counter, 0.0);
    }

    #[test]
    fn holy_shield_blocks_and_strikes_back_until_its_stacks_run_out() {
        let mut r = run_with("paladin", "holy_shield", false);
        let e = foe(&mut r, (2.0, 0.0), 1e9);
        play(&mut r, 0.6);
        let b = r.heroes[0].kit.block.clone().expect("the shield is up");
        assert_eq!((b.stacks, b.chance), (5, 0.3));
        // with a sure block, five blows use the five stacks and each strikes back for 80
        r.heroes[0].stats.block_chance = 1.0;
        // no lifesteal: its strikes back would heal some of what the blows took
        r.heroes[0].stats.lifesteal = 0.0;
        let health = r.heroes[0].health;
        for _ in 0..5 {
            r.damage_hero(0, 10.0, Some(e), true);
        }
        let ev = r.drain_events();
        let taken = 5.0 * 10.0 * (1.0 - crate::game::run::BLOCK_REDUCTION) * (1.0 - r.heroes[0].stats.damage_reduction);
        assert!((health - r.heroes[0].health - taken).abs() < 1e-3, "blocked blows do 30 % less damage");
        let back = dealt_by(&ev, "holy_shield");
        assert_eq!(back.len(), 5);
        assert!(back.iter().all(|d| (d - 80.0 * r.heroes[0].stats.damage).abs() < 0.01));
        assert!(r.heroes[0].kit.block.is_none());
        assert_eq!(ev.iter().filter(|x| matches!(x, Event::Blocked { .. })).count(), 5);
    }

    #[test]
    fn seal_of_command_procs_on_swings_and_its_damage_follows_the_swing() {
        let mut r = run_with("paladin", "seal_of_command", false);
        foe(&mut r, (2.0, 0.0), 1e9);
        let ev = play(&mut r, 120.0);
        let swings = ev.iter().filter(|e| matches!(e, Event::EnemyHit { source: HitSource::WeaponSwing, .. })).count() as f32;
        let procs = dealt_by(&ev, "seal_of_command");
        let share = procs.len() as f32 / swings;
        assert!((share - 0.3).abs() < 0.08, "about 30 % of swings: {share}");
        // the 1.8 s swing is fast: 240 a proc (Zeal only speeds the swing)
        assert!(procs.iter().all(|d| (d - 240.0 * r.heroes[0].stats.damage).abs() < 0.01));
        assert!(ev.iter().any(|e| matches!(e, Event::StacksChanged { kind: StackKind::Zeal, .. })));
    }

    #[test]
    fn a_slow_weapon_procs_at_fifty_percent_for_more() {
        let c = content();
        let a = c.ability("seal_of_command").unwrap();
        let Some(Mechanic::SwingProc { fast_chance, slow_chance, base_swing }) = a.kit.mechanic.clone() else { panic!() };
        // damage = spell damage × swing ÷ base swing × fast ÷ the chance used
        let slow = a.damage * 3.6 / base_swing * fast_chance / slow_chance;
        assert!((slow - 288.0).abs() < 0.01);
        let fast = a.damage * 1.6 / base_swing;
        assert!((fast - 213.3).abs() < 0.1);
    }

    #[test]
    fn every_hunter_has_auto_shot_as_a_ranged_swing() {
        let mut r = run_with("hunter", "steady_shot", false);
        foe(&mut r, (30.0, 0.0), 1e9);
        let ev = play(&mut r, 1.0);
        let shots: Vec<f32> = ev.iter().filter_map(|e| if let Event::EnemyHit { amount, source: HitSource::WeaponSwing, .. } = e { Some(*amount) } else { None }).collect();
        assert!(!shots.is_empty());
        // its listed damage at the swing scale
        let listed = swing_of(&r.content, &r.heroes[0]).unwrap().damage;
        assert!((shots[0] - listed * super::super::casting::SWING_SCALE * r.heroes[0].stats.damage).abs() < 0.01);
        assert!(ev.iter().any(|e| matches!(e, Event::SpellGo { spell: 75, ammo: true, .. })));
        assert!(!dealt_by(&ev, "steady_shot").is_empty());
    }

    #[test]
    fn kill_command_brings_a_pet_that_strikes_on_command() {
        let mut r = run_with("hunter", "kill_command", false);
        foe(&mut r, (6.0, 0.0), 1e9);
        let ev = play(&mut r, 6.0);
        assert!(ev.iter().any(|e| matches!(e, Event::SummonAppeared { kind: SummonKind::HunterPet, .. })));
        assert!(ev.iter().any(|e| matches!(e, Event::PetSummoned { pet, .. } if pet == "Cat")));
        assert!(!dealt_by(&ev, "hunter_pet_attack").is_empty(), "the pet's own attacks");
        let commands = dealt_by(&ev, "kill_command");
        assert!(!commands.is_empty());
        assert!(commands.iter().all(|d| (d - base_damage(&r, "kill_command") * r.heroes[0].stats.damage).abs() < 0.01));
    }

    #[test]
    fn explosive_shot_bursts_then_burns_and_lock_and_load_makes_the_next_two_free() {
        let mut r = run_with("hunter", "explosive_shot", false);
        let e = foe(&mut r, (20.0, 0.0), 1e9);
        let near = foe(&mut r, (22.0, 0.0), 1e9);
        let ev = play(&mut r, 2.6);
        let hits = dealt_by(&ev, "explosive_shot");
        // impact and two ticks of 30 on each of the two
        assert_eq!(hits.len(), 6, "{hits:?}");
        let _ = (e, near);
        r.heroes[0].kit.free_casts = 2;
        r.heroes[0].spells.known[0].timer = 0.0;
        let ev = play(&mut r, 0.2);
        assert_eq!(ev.iter().filter(|x| matches!(x, Event::SpellCast { spell, .. } if spell == "explosive_shot")).count(), 2, "two at once");
    }

    #[test]
    fn energy_comes_in_ticks_and_mutilate_poisons_two_then_envenom_strikes_every_poisoned() {
        let mut r = run_with("rogue", "mutilate", false);
        let Some(Resource::Energy { per_tick, tick_every, .. }) = option_of(&r.content, &r.heroes[0]).and_then(|o| o.resource) else {
            panic!("the mutilate rogue has energy")
        };
        let cost = r.content.ability("mutilate").unwrap().kit.cost;
        let foes: Vec<u32> = (0..4).map(|k| foe(&mut r, (2.0, k as f32), 1e9)).collect();
        // energy starts empty and comes a whole tick at a time, nothing in between
        let ev = play(&mut r, tick_every - 0.1);
        assert!(!ev.iter().any(|e| matches!(e, Event::ResourceChanged { kind: ResourceKind::Energy, .. })));
        let ev = play(&mut r, 0.2);
        assert!(ev.iter().any(|e| matches!(e, Event::ResourceChanged { kind: ResourceKind::Energy, value, .. } if *value == per_tick)));
        // the cost takes whole ticks
        let wait = (cost / per_tick).ceil() * tick_every;
        let ev = play(&mut r, wait - tick_every - 0.2);
        assert!(dealt_by(&ev, "mutilate").is_empty());
        let ev = play(&mut r, 0.3);
        assert_eq!(dealt_by(&ev, "mutilate").len(), 2);
        assert_eq!(r.heroes[0].kit.combo, 2);
        // the next Mutilate takes the two not yet poisoned
        play(&mut r, wait + 0.1);
        let poisoned = r.enemies.iter().filter(|e| e.dots.iter().any(|d| d.spell == "deadly_poison")).count();
        assert_eq!(poisoned, 4);
        // the third gives the fifth point: Envenom on all four, and one point carries over
        let ev = play(&mut r, wait + 0.2);
        assert_eq!(dealt_by(&ev, "envenom").len(), foes.len());
        assert_eq!(r.heroes[0].kit.combo, 1);
        assert!(ev.iter().any(|e| matches!(e, Event::ResourceChanged { kind: ResourceKind::ComboPoints, .. })));
    }

    #[test]
    fn sinister_strike_builds_to_eviscerate_and_hemorrhage_marks_for_rupture() {
        let mut r = run_with("rogue", "sinister_strike", false);
        foe(&mut r, (2.0, 0.0), 1e9);
        foe(&mut r, (2.0, 1.0), 1e9);
        r.heroes[0].kit.resource = 100.0;
        // long enough for the energy of five strikes and the finisher, ticks whole, and for five
        // strikes at the cooldown cap
        let Some(Resource::Energy { per_tick, tick_every, .. }) = option_of(&r.content, &r.heroes[0]).and_then(|o| o.resource) else {
            panic!("the sinister strike rogue has energy")
        };
        let cost = |s: &str| r.content.ability(s).unwrap().kit.cost;
        let need = (5.0 * cost("sinister_strike") + cost("eviscerate") - 100.0).max(0.0);
        let ev = play(&mut r, ((need / per_tick).ceil() * tick_every).max(5.0 * crate::game::spells::MIN_COOLDOWN) + 0.5);
        let evis = dealt_by(&ev, "eviscerate");
        assert_eq!(evis.len(), 1, "five strikes, one finisher");
        assert!((evis[0] - base_damage(&r, "eviscerate") * r.heroes[0].stats.damage).abs() < 0.01);
        let mut r = run_with("rogue", "hemorrhage", false);
        let marked = foe(&mut r, (2.0, 0.0), 1e9);
        let plain = foe(&mut r, (6.0, 0.0), 1e9);
        let mut a = r.content.ability("hemorrhage").unwrap().clone();
        a.targets = 1;
        a.kit.cost = 0.0;
        let inst = r.heroes[0].spells.known[0].instance();
        r.cast(0, &a, &inst);
        assert!(r.enemies.iter().find(|e| e.id == marked).unwrap().vulnerable.is_some());
        r.heroes[0].kit.combo = 5;
        let mut rupture = r.content.ability("rupture").unwrap().clone();
        rupture.targets = 1;
        r.cast(0, &rupture, &inst);
        play(&mut r, 0.1);
        assert!(r.enemies.iter().find(|e| e.id == marked).unwrap().dots.iter().any(|d| d.spell == "rupture"));
        assert!(!r.enemies.iter().find(|e| e.id == plain).unwrap().dots.iter().any(|d| d.spell == "rupture"));
    }

    #[test]
    fn every_fourth_smite_burns_around_the_priest_for_five_seconds() {
        let mut r = run_with("priest", "smite", false);
        let cd = r.content.ability("smite").unwrap().cooldown;
        foe(&mut r, (5.0, 0.0), 1e9);
        let ev = play(&mut r, 0.5 + 2.0 * cd + 0.3);
        assert!(dealt_by(&ev, "inner_fire_burn").is_empty());
        // the fourth smite, cd − 0.3 sec on, then its five seconds of burning
        let ev = play(&mut r, cd - 0.3 + 4.6);
        let burns = dealt_by(&ev, "inner_fire_burn");
        assert_eq!(burns.len(), 5, "once a second for 5 sec");
        assert!(ev.iter().any(|e| matches!(e, Event::StacksChanged { kind: StackKind::InnerFire, .. })));
    }

    #[test]
    fn a_holy_orb_is_taken_only_by_walking_over_it() {
        let mut r = run_with("priest", "holy_fire", false);
        r.heroes[0].stats.pickup_range = 40.0;
        let (id, p) = (999_998, r.heroes[0].pos);
        r.heroes[0].kit.orbs.push((id, (p.0 + 6.0, p.1), r.now + 15.0));
        let ev = play(&mut r, 0.2);
        assert!(!ev.iter().any(|e| matches!(e, Event::AreaGone { id: x } if *x == id)), "pickup range does not reach it");
        assert_eq!(r.heroes[0].kit.orbs.len(), 1);
    }

    #[test]
    fn holy_orbs_drop_within_25_yards_inside_the_arena() {
        let mut r = run_with("priest", "holy_fire", false);
        foe(&mut r, (10.0, 0.0), 1e9);
        let (centre, radius) = r.arena();
        let hero = r.heroes[0].id;
        r.hero_moved(hero, (centre.0 + radius - 5.0, centre.1), 0.0, 0.0);
        for _ in 0..400 {
            let a = r.content.ability("holy_fire").unwrap().clone();
            r.kit_on_cast(0, &a);
        }
        let p = r.heroes[0].pos;
        let orbs = &r.heroes[0].kit.orbs;
        assert!(orbs.len() > 40, "about one cast in five drops one: {}", orbs.len());
        assert!(orbs.iter().all(|o| dist(o.1, p) <= 25.01 && dist(o.1, centre) <= radius - ORB_EDGE_MARGIN + 0.01));
        assert!(orbs.iter().any(|o| dist(o.1, p) > 15.0), "they spread over the whole 25 yd");
    }

    #[test]
    fn a_holy_orb_picked_up_echoes_holy_fire_on_two_enemies() {
        let mut r = run_with("priest", "holy_fire", false);
        foe(&mut r, (10.0, 0.0), 1e9);
        foe(&mut r, (12.0, 0.0), 1e9);
        let id = 999_999;
        let at = r.heroes[0].pos;
        r.heroes[0].kit.orbs.push((id, at, r.now + 15.0));
        let ev = play(&mut r, 0.2);
        assert!(ev.iter().any(|e| matches!(e, Event::AreaGone { id: x } if *x == id)));
        let echo = dealt_by(&ev, "holy_echo");
        assert_eq!(echo.len(), 2);
        assert!(echo.iter().all(|d| (d - base_damage(&r, "holy_echo") * r.heroes[0].stats.damage).abs() < 0.01));
    }

    #[test]
    fn the_fifteenth_mind_spike_raises_two_tentacles_for_ten_seconds() {
        let mut r = run_with("priest", "mind_spike", false);
        foe(&mut r, (10.0, 0.0), 1e9);
        r.heroes[0].kit.counter = 98.0;
        let ev = play(&mut r, 0.6);
        let kinds: Vec<SummonKind> = ev.iter().filter_map(|e| if let Event::SummonAppeared { kind, .. } = e { Some(*kind) } else { None }).collect();
        assert_eq!(kinds, [SummonKind::EyeTentacle, SummonKind::ClawTentacle]);
        // the claw claws only an enemy next to it, with its own melee attack
        let claw = r.pets.iter().find(|p| p.summon.as_ref().is_some_and(|s| s.kind == SummonKind::ClawTentacle)).unwrap();
        let (claw_id, claw_at) = (claw.id, claw.segment.position(r.now));
        let hero_at = r.heroes[0].pos;
        foe(&mut r, (claw_at.0 - hero_at.0 + 1.0, claw_at.1 - hero_at.1), 1e9);
        let ev = play(&mut r, 10.5);
        let clawed = ev.iter().any(|e| matches!(e, Event::EnemyHit { source: HitSource::Pet(p), .. } if *p == claw_id));
        assert!(!dealt_by(&ev, "eye_tentacle").is_empty(), "the eye flays");
        assert!(clawed, "the claw claws");
        assert!(dealt_by(&ev, "claw_tentacle").is_empty(), "no mind flay from the claw");
        assert_eq!(ev.iter().filter(|e| matches!(e, Event::SummonGone { .. })).count(), 2);
    }

    #[test]
    fn divine_star_strikes_on_its_way_out_and_back() {
        let mut r = run_with("priest", "smite", false);
        foe(&mut r, (10.0, 0.0), 1e9);
        let a = r.content.ability("divine_star").unwrap().clone();
        r.cast(0, &a, &Instance::default());
        let ev = play(&mut r, 2.5);
        assert_eq!(dealt_by(&ev, "divine_star").len(), 2);
    }

    #[test]
    fn a_bolt_on_a_marked_enemy_discharges_a_fifth_of_it_around() {
        let mut r = run_with("shaman", "lightning_bolt", false);
        r.heroes[0].stats.damage = 1.0;
        let marked = foe(&mut r, (20.0, 0.0), 1e9);
        foe(&mut r, (22.0, 0.0), 1e9);
        foe(&mut r, (21.0, 1.0), 1e9);
        r.enemies.iter_mut().find(|e| e.id == marked).unwrap().kit.marked_until = 100.0;
        let a = r.content.ability("lightning_bolt").unwrap().clone();
        let inst = r.heroes[0].spells.known[0].instance();
        r.cast(0, &a, &inst);
        let ev = play(&mut r, 0.1);
        let hits: Vec<(u32, f32)> = ev.iter().filter_map(|e| if let Event::EnemyHit { id, amount, source: HitSource::Spell(s), .. } = e { (s == "lightning_bolt").then_some((*id, *amount)) } else { None }).collect();
        assert_eq!(hits[0].0, marked, "marked enemies first");
        let rod = dealt_by(&ev, "lightning_rod");
        // the bolt on the marked one discharges its share to each of the two others
        let PassiveKind::Marks { share, .. } = passive_of(&r, "lightning_bolt") else { panic!("Lightning Rod marks") };
        assert!(rod.len() >= 2);
        assert!(rod.iter().all(|d| (d - share * hits[0].1).abs() < 0.01), "{rod:?}");
    }

    #[test]
    fn a_swing_that_kills_its_target_sends_its_procs_to_the_next_enemy() {
        let swing = AutoAttack { damage: 27.0, swing: 1.8, range: 6.0, look_spell: None };
        for (class, start) in [("shaman", "windfury_weapon"), ("paladin", "seal_of_command")] {
            let mut r = run_with(class, start, false);
            let dead = foe(&mut r, (2.0, 0.0), 1e9);
            let next = foe(&mut r, (3.0, 0.0), 1e9);
            // a sure proc, and the swing's target already gone
            for s in &mut r.heroes[0].spells.known {
                if s.spell == start {
                    s.upgrades.gain = 1.0 / 0.3;
                }
            }
            r.enemies.retain(|e| e.id != dead);
            r.kit_on_swing(0, dead, &swing);
            let ev = r.drain_events();
            let on_next = ev.iter().filter(|e| matches!(e, Event::EnemyHit { id, source: HitSource::Spell(s), .. } if *id == next && s == start)).count();
            assert!(on_next >= 1, "{start}: the proc went on to the next enemy");
        }
    }

    #[test]
    fn lock_and_load_rolls_once_a_tick_and_never_for_its_own_free_shots() {
        let mut r = run_with("hunter", "explosive_shot", false);
        let fired = |ev: &[Event]| ev.iter().filter(|e| matches!(e, Event::PassiveTriggered { passive, .. } if passive == "lock_and_load")).count();
        // the free shots it gave never roll
        for k in 0..400 {
            r.now = k as f32 + 0.5;
            r.kit_on_area_tick(0, "explosive_shot", true);
        }
        assert_eq!(fired(&r.drain_events()), 0);
        // many areas of one shot ticking at the same moment roll once between them
        r.now = 1000.0;
        for _ in 0..400 {
            r.kit_on_area_tick(0, "explosive_shot", false);
        }
        assert!(fired(&r.drain_events()) <= 1);
        // a tick at each moment rolls each time: about one in ten
        for k in 0..400 {
            r.now = 2000.0 + k as f32;
            r.heroes[0].kit.free_casts = 0;
            r.kit_on_area_tick(0, "explosive_shot", false);
        }
        let n = fired(&r.drain_events());
        let PassiveKind::FreeCasts { chance, .. } = passive_of(&r, "explosive_shot") else { panic!("Lock and Load") };
        let want = 400.0 * chance;
        assert!((n as f32 - want).abs() < want * 0.5, "about {want} of 400: {n}");
    }

    #[test]
    fn windfury_strikes_twice_more_and_maelstrom_lets_loose_its_lightning_at_five() {
        let mut r = run_with("shaman", "windfury_weapon", false);
        foe(&mut r, (2.0, 0.0), 1e9);
        foe(&mut r, (8.0, 0.0), 1e9);
        let ev = play(&mut r, 120.0);
        let extra = dealt_by(&ev, "windfury_weapon");
        assert!(!extra.is_empty());
        assert_eq!(extra.len() % 2, 0, "two extra hits a proc");
        assert!(extra.iter().all(|d| (d - base_damage(&r, "windfury_weapon") * r.heroes[0].stats.damage).abs() < 0.01));
        assert!(ev.iter().any(|e| matches!(e, Event::PassiveTriggered { passive, .. } if passive == "maelstrom_weapon")));
        assert!(!dealt_by(&ev, "maelstrom_lightning").is_empty());
    }

    #[test]
    fn damage_and_healing_call_a_ghostly_double_that_casts_for_ten_seconds() {
        let mut r = run_with("shaman", "wellspring", false);
        foe(&mut r, (10.0, 0.0), 1e9);
        // 99 points: one more 12 damage fills it
        r.heroes[0].kit.counter = 99.0;
        r.kit_on_heal(0, 6.0);
        let ev = r.drain_events();
        assert!(ev.iter().any(|e| matches!(e, Event::SummonAppeared { kind: SummonKind::Clone, .. })), "healing counts double: 6 healed is a point");
        let ev = play(&mut r, 10.5);
        let ancestors = ["ancestral_lightning", "ancestral_flame"].iter().map(|s| dealt_by(&ev, s).len()).sum::<usize>();
        assert!(ancestors > 0);
        assert!(ev.iter().any(|e| matches!(e, Event::SummonGone { .. })));
        // only Wellspring fills it: not the double's damage, not the swings
        let wellspring: f32 = dealt_by(&ev, "wellspring").iter().sum::<f32>();
        let PassiveKind::Counter(c) = passive_of(&r, "wellspring") else { panic!("a counter") };
        let per = c.fill.iter().find_map(|f| if let Fill::Damage { per, .. } = f { Some(*per) } else { None }).unwrap();
        assert!((r.heroes[0].kit.counter - (wellspring / per).floor()).abs() <= 1.0, "{} vs {}", r.heroes[0].kit.counter, wellspring / per);
    }

    #[test]
    fn four_arcane_blasts_make_the_next_other_spell_deal_forty_percent_more() {
        let mut r = run_with("mage", "arcane_blast", false);
        foe(&mut r, (10.0, 0.0), 1e9);
        play(&mut r, 0.5 + 3.0 * 3.0 + 0.3);
        assert_eq!(r.heroes[0].kit.counter, 4.0);
        let fire_blast = r.content.ability("fire_blast").unwrap().clone();
        r.heroes[0].spells.learn(&fire_blast, &[]);
        let ev = play(&mut r, 0.7);
        let hit = dealt_by(&ev, "fire_blast");
        assert_eq!(hit.len(), 1);
        assert!((hit[0] - fire_blast.damage * 1.4 * r.heroes[0].stats.damage).abs() < 0.01, "{hit:?}");
        assert_eq!(r.heroes[0].kit.counter, 0.0, "used up");
    }

    #[test]
    fn every_fourth_fireball_crit_hurls_hot_streak() {
        let mut r = run_with("mage", "fireball", false);
        foe(&mut r, (10.0, 0.0), 1e9);
        // fireball's own 10 % on top of the hero's
        r.heroes[0].stats.crit_chance = 0.9;
        // four fireballs, and the Hot Streak's flight
        let cd = r.content.ability("fireball").unwrap().cooldown;
        let ev = play(&mut r, 0.5 + 3.0 * cd + 2.5);
        let hot = dealt_by(&ev, "hot_streak");
        let PassiveKind::Counter(c) = passive_of(&r, "fireball") else { panic!("a counter") };
        assert_eq!(hot.len(), (4.0 / c.max) as usize, "four crits in the first four fireballs, one Hot Streak each {}", c.max);
        assert!(hot[0] >= base_damage(&r, "hot_streak") * r.heroes[0].stats.damage - 0.01);
    }

    #[test]
    fn the_water_elemental_freezes_a_pack_and_the_first_frost_hit_shatters() {
        let mut r = run_with("mage", "frostbolt", false);
        let e = foe(&mut r, (10.0, 0.0), 1e9);
        r.heroes[0].kit.counter = 99.5;
        let mut ev = play(&mut r, 0.3);
        assert!(ev.iter().any(|x| matches!(x, Event::SummonAppeared { kind: SummonKind::WaterElemental, .. })));
        ev.extend(play(&mut r, 0.4));
        assert_eq!(dealt_by(&ev, "water_elemental_nova").len(), 1);
        let frozen = r.enemies.iter().find(|x| x.id == e).unwrap();
        assert!(frozen.hold_until > r.now && frozen.kit.frozen_by_release_until > r.now);
        // the next bolt, a cooldown after the first
        let cd = r.content.ability("frostbolt").unwrap().cooldown;
        ev.extend(play(&mut r, cd));
        let bolts = dealt_by(&ev, "frostbolt");
        assert_eq!(bolts.len(), 2);
        assert!((bolts[1] - base_damage(&r, "frostbolt") * r.heroes[0].stats.damage).abs() < 0.01, "only the first shatters");
        assert!(!bolts.is_empty());
        assert!((bolts[0] - 4.0 * base_damage(&r, "frostbolt") * r.heroes[0].stats.damage).abs() < 0.01, "4 times the hit: {bolts:?}");
        // no fill from bolts on an enemy the elemental froze
        assert!(r.heroes[0].kit.counter < 10.0);
    }

    #[test]
    fn drain_soul_takes_the_weakest_jumps_on_a_kill_and_gives_one_shard_per_drain() {
        let mut r = run_with("warlock", "drain_soul", false);
        let weak = foe(&mut r, (10.0, 0.0), 40.0);
        let next = foe(&mut r, (12.0, 0.0), 45.0);
        let strong = foe(&mut r, (8.0, 0.0), 1e9);
        let ev = play(&mut r, 4.0);
        let first = ev.iter().find_map(|e| if let Event::EnemyHit { id, source: HitSource::Tick(s), .. } = e { (s == "drain_soul").then_some(*id) } else { None });
        assert_eq!(first, Some(weak), "the least health first");
        assert!(r.enemies.iter().all(|e| e.id != weak && e.id != next), "it went on to the next and drained it too");
        assert_eq!(r.heroes[0].kit.counter, 1.0, "two kills in one drain: one shard");
        let _ = strong;
        r.heroes[0].kit.counter = 3.0;
        r.heroes[0].kit.drain_shard_cast = 0;
        r.kit_on_kill(0, &{
            let mut e = r.enemies[0].clone();
            e.dots.push(Dot { hero: 0, spell: "drain_soul".into(), per_tick: 1.0, ticks_left: 3, interval: 0.5, next: r.now, from_proc: false, leech: 0.0 });
            e
        });
        let ev = play(&mut r, 2.5);
        assert_eq!(dealt_by(&ev, "seed_of_corruption").len(), 1, "the fourth shard: the seed bursts 2 sec later");
    }

    #[test]
    fn a_drain_goes_on_alone_when_its_enemy_dies_under_an_effect_that_spreads_on_death() {
        // Pendant of the Plague on Drain Soul: copies would merge back into the drain as they die
        // and be copied again, its ticks growing until the damage ran into the billions
        let spread = esurv_content::Milestone::SpreadOnDeath { look_spell: None, count: 3, filter: esurv_content::SpellFilter::Spell("drain_soul".into()) };
        let setup = HeroSetup { class: "warlock".into(), race: "human".into(), start: Some("drain_soul".into()), milestones: vec![spread], ..Default::default() };
        let mut r = Run::new(content(), "elwynn", RunOptions::default(), &[setup], Box::new(Flat), 7).unwrap();
        r.enemies.clear();
        r.pickups.clear();
        let dying = foe(&mut r, (10.0, 0.0), 1e6);
        let near: Vec<u32> = (1..=3).map(|k| foe(&mut r, (10.0 + k as f32, 0.0), 1e6)).collect();
        let drain = Dot { hero: 0, spell: "drain_soul".into(), per_tick: 5.0, ticks_left: 6, interval: 0.5, next: r.now + 0.5, from_proc: false, leech: 0.0 };
        r.enemies.iter_mut().find(|e| e.id == dying).unwrap().dots.push(drain);
        r.hit_enemy(Some(0), dying, f32::MAX, false, HitSource::Spell("shadow_bolt".into()));
        let drains: Vec<(u32, u32)> =
            r.enemies.iter().flat_map(|e| e.dots.iter().filter(|d| d.spell == "drain_soul").map(move |d| (e.id, d.ticks_left))).collect();
        assert_eq!(drains.len(), 1, "one drain, gone on to the next: {drains:?}");
        assert!(near.contains(&drains[0].0));
        assert_eq!(drains[0].1, 6, "with the time it had left");
    }

    #[test]
    fn hand_of_guldan_leaves_imps_whose_souls_call_an_infernal_at_fifteen() {
        let mut r = run_with("warlock", "hand_of_guldan", false);
        foe(&mut r, (10.0, 0.0), 1e9);
        let ev = play(&mut r, 1.0);
        assert_eq!(dealt_by(&ev, "hand_of_guldan").len(), 1);
        assert!(ev.iter().any(|e| matches!(e, Event::SummonAppeared { kind: SummonKind::Imp, .. })));
        let ev = play(&mut r, 12.0);
        assert!(!dealt_by(&ev, "imp_firebolt").is_empty());
        assert!(r.heroes[0].kit.counter >= 1.0, "the first imp's soul");
        assert!(ev.iter().any(|e| matches!(e, Event::AreaShown { spell: SOUL_ORB_LOOK, .. })));
        r.heroes[0].kit.counter = 14.0;
        let ev = play(&mut r, 6.5);
        assert!(ev.iter().any(|e| matches!(e, Event::SummonAppeared { kind: SummonKind::Infernal, .. })));
        let ev = play(&mut r, 3.0);
        assert!(!dealt_by(&ev, "infernal_immolation").is_empty());
        assert!(r.heroes[0].kit.orbs.is_empty(), "the souls gathered");
    }

    #[test]
    fn eight_shadow_bolts_light_four_embers_and_soulfire_strikes_the_healthiest() {
        let mut r = run_with("warlock", "shadow_bolt", false);
        foe(&mut r, (10.0, 0.0), 1e6);
        let big = foe(&mut r, (25.0, 0.0), 1e9);
        // eight bolts, and Soulfire's flight
        let cd = r.content.ability("shadow_bolt").unwrap().cooldown;
        let ev = play(&mut r, 0.5 + 7.0 * cd + 1.5);
        let soulfire: Vec<u32> = ev.iter().filter_map(|e| if let Event::EnemyHit { id, source: HitSource::Spell(s), .. } = e { (s == "soulfire").then_some(*id) } else { None }).collect();
        assert_eq!(soulfire, [big]);
    }

    #[test]
    fn each_druid_form_offers_its_own_card_pool_and_keeps_its_form_stats() {
        let c = content();
        let druid = c.class("druid").unwrap();
        let cat = druid.card_pool(Some("cat_form"));
        assert!(cat.iter().any(|s| s == "ferocious_bite") && !cat.iter().any(|s| s == "wrath"));
        assert!(druid.card_pool(Some("bear_form")).iter().any(|s| s == "lacerate"));
        assert!(!druid.card_pool(Some("cat_form")).iter().any(|s| s == "berserk"), "Berserk comes from the Feral capstone");
        assert_eq!(druid.card_pool(Some("cat_form")).len(), 12);
        assert!(!druid.card_pool(Some("moonkin_form")).iter().any(|s| s == "starfire"));
        let moonkin = run_with("druid", "moonkin_form", false);
        let bear = run_with("druid", "bear_form", false);
        let rest = run_with("druid", "restoration", false);
        // the form's 40 health, under the staff's -5 % max health
        assert!((bear.heroes[0].stats.max_health - rest.heroes[0].stats.max_health - 40.0 * 0.95).abs() < 1e-3);
        assert!((bear.heroes[0].stats.damage_reduction - rest.heroes[0].stats.damage_reduction - 0.1).abs() < 1e-5);
        assert!((moonkin.heroes[0].stats.damage_reduction - rest.heroes[0].stats.damage_reduction - 0.1).abs() < 1e-5);
    }

    #[test]
    fn eight_starfires_fill_lunar_power_and_stars_fall_for_six_seconds() {
        let mut r = run_with("druid", "moonkin_form", false);
        foe(&mut r, (8.0, 0.0), 1e9);
        r.heroes[0].kit.counter = 87.5;
        let ev = play(&mut r, 7.0);
        assert!(ev.iter().any(|e| matches!(e, Event::PassiveTriggered { passive, .. } if passive == "starshards")));
        assert_eq!(dealt_by(&ev, "starshards").len(), 6);
    }

    #[test]
    fn the_cat_rips_for_ten_seconds_and_bite_takes_turns() {
        let mut r = run_with("druid", "cat_form", false);
        let e = foe(&mut r, (2.0, 0.0), 1e9);
        r.heroes[0].kit.combo = 5;
        let ev = play(&mut r, 3.7);
        assert_eq!(ev.iter().filter(|x| matches!(x, Event::SpellCast { spell, .. } if spell == "rip")).count(), 1);
        assert_eq!(dealt_by(&ev, "rip").len(), 3, "15 a second");
        let rip = |r: &Run, e: u32| r.enemies.iter().find(|x| x.id == e).unwrap().dots.iter().filter(|d| d.spell == "rip").map(|d| d.ticks_left).sum::<u32>();
        assert!((5..=8).contains(&rip(&r, e)), "10 s, 3 of them gone: {}", rip(&r, e));
        // with Ferocious Bite learned the finishers take turns: Bite, then Rip, still 10 s
        let bite = r.content.ability("ferocious_bite").unwrap().clone();
        r.heroes[0].spells.learn(&bite, &[]);
        r.enemies.iter_mut().for_each(|x| x.dots.clear());
        r.heroes[0].kit.combo = 5;
        let ev = play(&mut r, 0.7);
        assert_eq!(dealt_by(&ev, "ferocious_bite").len(), 1);
        r.heroes[0].kit.combo = 5;
        let ev = play(&mut r, 0.7);
        assert!(ev.iter().any(|x| matches!(x, Event::SpellCast { spell, .. } if spell == "rip")));
        // a flat 10 s, × the staff loadout's +10 % spell duration
        assert!((10..=11).contains(&rip(&r, e)), "a flat 10 s: {}", rip(&r, e));
    }

    #[test]
    fn the_cats_swipe_is_free_on_its_own_cooldown_and_gives_no_combo_point() {
        let mut r = run_with("druid", "cat_form", false);
        foe(&mut r, (2.0, 0.0), 1e9);
        foe(&mut r, (2.0, 1.0), 1e9);
        let swipe = r.content.ability("cat_swipe").unwrap().clone();
        assert_eq!((swipe.kit.cost, swipe.kit.combo_points, swipe.cooldown), (0.0, 0, 3.0));
        r.heroes[0].spells.learn(&swipe, &[]);
        r.heroes[0].kit.resource = 0.0;
        let ev = play(&mut r, 20.0);
        let swipes = ev.iter().filter(|x| matches!(x, Event::SpellCast { spell, .. } if spell == "cat_swipe")).count();
        // with no energy at all, it still swipes on its own 3 s cooldown (about 6 in 20 s)
        assert!(swipes >= 5, "{swipes} swipes");
        assert_eq!(dealt_by(&ev, "cat_swipe").len(), 2 * swipes, "every enemy near");
        // it costs nothing, gives no combo point, and never waits for Claw's turn
        let mut r = run_with("druid", "cat_form", false);
        foe(&mut r, (2.0, 0.0), 1e9);
        let claw = r.content.ability("claw").unwrap().clone();
        r.heroes[0].spells.learn(&swipe, &[]);
        r.heroes[0].kit.resource = 0.0;
        assert!(r.kit_ready(0, &swipe), "no energy needed");
        r.heroes[0].kit.resource = 70.0;
        assert!(r.kit_ready(0, &claw) && r.kit_ready(0, &swipe), "both ready, no turns");
        r.kit_after_cast(0, &swipe);
        assert!((r.heroes[0].kit.resource - 70.0).abs() < 1e-4, "no energy spent");
        assert_eq!(r.heroes[0].kit.combo, 0, "no combo point");
        assert!(r.kit_ready(0, &claw), "Claw is still ready");
    }

    #[test]
    fn the_bears_swipe_keeps_its_rage_rules() {
        let mut r = run_with("druid", "bear_form", false);
        foe(&mut r, (2.0, 0.0), 1e9);
        let swipe = r.content.ability("swipe").unwrap().clone();
        assert_eq!((swipe.kit.cost, swipe.kit.combo_points), (0.0, 0));
        assert!(!swipe.upgrades.contains(&Upgrade::EnergyGain));
        // two spells of their own: their names say whose; only Blood Frenzy reaches both,
        // each has its own legendary
        let cat = r.content.ability("cat_swipe").unwrap();
        assert!(swipe.name.ends_with(" (Bear)") && cat.name.ends_with(" (Cat)"), "{} / {}", swipe.name, cat.name);
        assert!((cat.damage - 47.0).abs() < 1e-6);
        let reaching = |spell: &str| -> Vec<String> {
            let mut ids = Vec::new();
            for tree in &r.content.talent_trees {
                for t in &tree.talents {
                    if format!("{:?}", t.effects).contains(&format!("\"{spell}\"")) {
                        ids.push(t.id.clone());
                    }
                }
            }
            ids
        };
        assert_eq!(reaching("cat_swipe"), ["blood_frenzy"]);
        assert!(reaching("swipe").contains(&"blood_frenzy".to_string()) && reaching("swipe").len() > 1);
        let legendary = |spell: &str| r.content.legendary_amulets.iter().filter(|l| l.spells.iter().any(|s| s == spell)).count();
        assert_eq!((legendary("swipe"), legendary("cat_swipe")), (1, 1));
        r.heroes[0].kit.resource = 0.0;
        r.heroes[0].spells.learn(&swipe, &[]);
        assert!(r.kit_ready(0, &swipe), "free, no turn to wait for");
        r.kit_after_cast(0, &swipe);
        assert_eq!((r.heroes[0].kit.combo, r.heroes[0].kit.builder_turn), (0, 0));
    }

    #[test]
    fn the_cats_swipe_energy_cards_speed_up_the_energy_as_claws_do() {
        let energy_after = |card_on: Option<&str>| {
            let mut r = run_with("druid", "cat_form", false);
            let swipe = r.content.ability("cat_swipe").unwrap().clone();
            r.heroes[0].spells.learn(&swipe, &[]);
            if let Some(s) = card_on {
                let slot = r.heroes[0].spells.known.iter_mut().find(|x| x.spell == s).unwrap();
                crate::game::upgrades::apply(&mut slot.upgrades, &[Upgrade::EnergyGain], esurv_content::Rarity::Epic);
            }
            r.heroes[0].kit.resource = 0.0;
            play(&mut r, 1.02);
            r.heroes[0].kit.resource
        };
        let (none, claw, swipe) = (energy_after(None), energy_after(Some("claw")), energy_after(Some("cat_swipe")));
        assert!(claw > none, "{claw} > {none}");
        assert!((swipe - claw).abs() < 1e-4, "the same card on cat_swipe: {swipe} vs {claw}");
    }

    #[test]
    fn a_targets_card_makes_claw_strike_two_enemies_for_one_combo_point() {
        let mut r = run_with("druid", "cat_form", false);
        let (a, b) = (foe(&mut r, (2.0, 0.0), 1e9), foe(&mut r, (2.0, 1.0), 1e9));
        let claw = r.content.ability("claw").unwrap().clone();
        assert!(claw.upgrades.contains(&Upgrade::Targets));
        let slot = r.heroes[0].spells.known.iter_mut().find(|x| x.spell == "claw").unwrap();
        crate::game::upgrades::apply(&mut slot.upgrades, &[Upgrade::Targets], esurv_content::Rarity::Rare);
        r.heroes[0].kit.resource = 60.0;
        let ev = play(&mut r, 1.2);
        let hit: Vec<u32> = ev.iter().filter_map(|x| if let Event::EnemyHit { id, source: HitSource::Spell(s), .. } = x { (s == "claw").then_some(*id) } else { None }).collect();
        assert!(hit.contains(&a) && hit.contains(&b) && hit.len() == 2, "{hit:?}");
        assert_eq!(r.heroes[0].kit.combo, 1);
    }

    #[test]
    fn a_faster_pace_keeps_rip_bleeding_on_several_enemies() {
        let mut r = run_with("druid", "cat_form", false);
        let (a, b) = (foe(&mut r, (2.0, 0.0), 1e9), foe(&mut r, (2.0, 1.0), 1e9));
        r.heroes[0].kit.combo = 5;
        play(&mut r, 0.7);
        r.heroes[0].kit.combo = 5;
        // the finisher again once the cooldown cap lets it
        play(&mut r, crate::game::spells::MIN_COOLDOWN + 0.2);
        let bleeding = |r: &Run, e: u32| r.enemies.iter().find(|x| x.id == e).unwrap().dots.iter().any(|d| d.spell == "rip");
        assert!(bleeding(&r, a) && bleeding(&r, b), "the second Rip went on the enemy not yet bleeding");
    }

    #[test]
    fn a_heal_from_a_form_leaves_it_and_shifts_straight_back_into_the_same_form() {
        for (option, look) in [("bear_form", 5487), ("cat_form", 768)] {
            let mut r = run_with("druid", option, false);
            foe(&mut r, (10.0, 0.0), 1e9);
            // hurt, so the heal has something to do
            r.heroes[0].health = r.heroes[0].stats.max_health * 0.5;
            learn_and_cast(&mut r, "regrowth");
            let ev = r.drain_events();
            assert!(ev.iter().any(|e| matches!(e, Event::FormLeft { look_spell, .. } if *look_spell == look)), "{option} shifts back into its own form");
        }
        let mut r = run_with("druid", "moonkin_form", false);
        r.heroes[0].health = r.heroes[0].stats.max_health * 0.5;
        learn_and_cast(&mut r, "regrowth");
        assert!(!r.drain_events().iter().any(|e| matches!(e, Event::FormLeft { .. })), "moonkin heals in form");
    }

    #[test]
    fn powershift_leaves_cat_form_and_shifts_straight_back() {
        let mut r = run_with("druid", "cat_form", false);
        foe(&mut r, (10.0, 0.0), 1e9);
        r.heroes[0].kit.resource = 0.0;
        learn_and_cast(&mut r, "powershift");
        let ev = r.drain_events();
        assert!(ev.iter().any(|e| matches!(e, Event::SpellCast { spell, .. } if spell == "powershift")), "it shifted");
        assert!(ev.iter().any(|e| matches!(e, Event::FormLeft { look_spell: 768, .. })), "out of Cat Form and back");
    }

    #[test]
    fn mauls_stacks_cards_add_no_targets() {
        let c = content();
        let maul = c.ability("maul").unwrap().clone();
        let mut slot = crate::game::spells::SpellSlot::new(&maul, &[]);
        crate::game::upgrades::apply(&mut slot.upgrades, &[esurv_content::Upgrade::Stacks], esurv_content::Rarity::Epic);
        assert_eq!(slot.upgrades.taken[esurv_content::Upgrade::Stacks as usize], 1, "the card counts for thick hide");
        assert_eq!(crate::game::abilities::target_count(&maul, &slot.instance(), 0), maul.targets, "but adds no target");
        crate::game::upgrades::apply(&mut slot.upgrades, &[esurv_content::Upgrade::Targets], esurv_content::Rarity::Rare);
        assert_eq!(crate::game::abilities::target_count(&maul, &slot.instance(), 0), maul.targets + 1, "a Targets card still does");
    }

    #[test]
    fn the_bear_mauls_the_healthiest_with_rage_and_thick_hide_stacks() {
        let mut r = run_with("druid", "bear_form", false);
        let weak = foe(&mut r, (2.0, 0.0), 1e6);
        let big = foe(&mut r, (3.0, 0.0), 1e9);
        let mid = foe(&mut r, (3.0, 1.0), 1e8);
        r.heroes[0].kit.resource = 40.0;
        // a Thick Hide card: 5 stacks
        r.heroes[0].spells.known[0].upgrades.taken[Upgrade::Stacks as usize] = 1;
        let reduction = r.heroes[0].stats.damage_reduction;
        let ev = play(&mut r, 0.7);
        let hit: Vec<u32> = ev.iter().filter_map(|x| if let Event::EnemyHit { id, source: HitSource::Spell(s), .. } = x { (s == "maul").then_some(*id) } else { None }).collect();
        assert_eq!(hit, [big, mid, weak]);
        assert!((r.heroes[0].stats.damage_reduction - reduction - 0.01).abs() < 1e-5, "one Thick Hide stack");
    }

    /// Hero 0 learns `spell` and casts it once now, its hits landing at once.
    fn learn_and_cast(r: &mut Run, spell: &str) -> Instance {
        let a = r.content.ability(spell).unwrap().clone();
        r.heroes[0].spells.learn(&a, &[]);
        let inst = r.heroes[0].spells.known.iter().find(|s| s.spell == spell).unwrap().instance();
        cast_now(r, spell, &inst);
        inst
    }

    fn cast_now(r: &mut Run, spell: &str, inst: &Instance) -> bool {
        let a = r.content.ability(spell).unwrap().clone();
        let cast = r.cast(0, &a, inst);
        r.effects_tick();
        cast
    }

    fn stacks_of(r: &Run, e: u32, spell: &str) -> Vec<Dot> {
        r.enemies.iter().find(|x| x.id == e).unwrap().dots.iter().filter(|d| d.spell == spell).cloned().collect()
    }

    #[test]
    fn lacerate_bleeds_three_enemies_up_to_three_stacks_each() {
        let mut r = run_with("druid", "bear_form", false);
        let weak = foe(&mut r, (2.0, 0.0), 1e6);
        let big = foe(&mut r, (3.0, 0.0), 1e9);
        let mid = foe(&mut r, (3.0, 1.0), 1e8);
        let low = foe(&mut r, (2.0, 1.0), 1e7);
        let inst = learn_and_cast(&mut r, "lacerate");
        for _ in 0..2 {
            assert!(cast_now(&mut r, "lacerate", &inst));
        }
        let per_stack = 150.0 * r.heroes[0].stats.damage;
        for e in [big, mid, low] {
            let s = stacks_of(&r, e, "lacerate");
            assert_eq!(s.len(), 3, "three stacks on each of the three healthiest");
            assert!(s.iter().all(|d| d.ticks_left == 3 && (d.per_tick * 3.0 - per_stack).abs() < 0.01 && (d.interval - 3.0).abs() < 1e-5));
        }
        assert!(stacks_of(&r, weak, "lacerate").is_empty());
        // all three full and none due before the next cast: the next one goes down the line
        assert!(cast_now(&mut r, "lacerate", &inst));
        assert_eq!(stacks_of(&r, weak, "lacerate").len(), 1);
        assert_eq!(stacks_of(&r, big, "lacerate").len(), 3);
    }

    #[test]
    fn a_fourth_lacerate_renews_the_oldest_stack_and_keeps_what_it_had_left() {
        let mut r = run_with("druid", "bear_form", false);
        let e = foe(&mut r, (2.0, 0.0), 1e9);
        let lacerate = |next: f32| Dot { hero: 0, spell: "lacerate".into(), per_tick: 50.0, ticks_left: 3, interval: 3.0, next, from_proc: false, leech: 0.0 };
        for k in 0..4 {
            r.add_dot(e, lacerate(3.0 + k as f32));
        }
        let s = stacks_of(&r, e, "lacerate");
        assert_eq!(s.len(), 3, "stacks up to 3");
        let total: f32 = s.iter().map(|d| d.per_tick * d.ticks_left as f32).sum();
        assert!((total - 4.0 * 150.0).abs() < 0.01, "nothing lost: {total}");
        assert_eq!(s[0].ticks_left, 6, "the oldest took the new one's ticks");
        // an effect over time without the rule renews instead of stacking
        let rake = Dot { spell: "rake".into(), ..lacerate(3.0) };
        r.add_dot(e, rake.clone());
        r.add_dot(e, rake);
        assert_eq!(stacks_of(&r, e, "rake").len(), 1);
    }

    #[test]
    fn berserk_makes_the_cats_attacks_strike_the_two_nearest_others() {
        let mut r = run_with("druid", "cat_form", false);
        let first = foe(&mut r, (2.0, 0.0), 1e9);
        let near = foe(&mut r, (3.0, 0.5), 1e9);
        let next = foe(&mut r, (3.5, -0.5), 1e9);
        let third = foe(&mut r, (5.0, 0.0), 1e9);
        let far = foe(&mut r, (30.0, 0.0), 1e9);
        let claw = r.heroes[0].spells.known.iter().find(|s| s.spell == "claw").unwrap().instance();
        assert!(cast_now(&mut r, "claw", &claw));
        assert_eq!(dealt_by(&r.drain_events(), "claw").len(), 1, "no buff: one target");
        learn_and_cast(&mut r, "berserk");
        let buff = r.heroes[0].buffs.iter().find(|b| b.kind == BuffKind::ExtraStrikes).copied().unwrap();
        assert_eq!((buff.value, buff.until - r.now), (2.0, 12.0));
        r.drain_events();
        assert!(cast_now(&mut r, "claw", &claw));
        let ev = r.drain_events();
        let hits: Vec<(u32, f32)> = ev.iter().filter_map(|x| if let Event::EnemyHit { id, amount, source: HitSource::Spell(s), .. } = x { (s == "claw").then_some((*id, *amount)) } else { None }).collect();
        assert_eq!(hits.len(), 3, "the target and the two nearest it, under Claw's name: {hits:?}");
        assert!(hits.iter().all(|h| (h.1 - hits[0].1).abs() < 0.01), "full damage");
        assert!(!hits.iter().any(|h| h.0 == far || h.0 == third));
        assert!(ev.iter().any(|x| matches!(x, Event::SpellGo { spell, .. } if *spell == EXTRA_STRIKE_LOOK)));
        // a bleed goes on two more as well
        let rip = r.heroes[0].spells.known.iter().find(|s| s.spell == "rip").unwrap().instance();
        assert!(cast_now(&mut r, "rip", &rip));
        let bleeding = [first, near, next, third].iter().filter(|e| !stacks_of(&r, **e, "rip").is_empty()).count();
        assert_eq!(bleeding, 3);
        // the swing too
        let ev = play(&mut r, 1.5);
        let swings = ev.iter().filter(|x| matches!(x, Event::EnemyHit { source: HitSource::WeaponSwing, .. })).count();
        assert!(swings >= 3 && swings % 3 == 0, "{swings}");
        // and gone when the buff runs out
        play(&mut r, 9.0);
        r.heroes[0].buffs.retain(|b| b.kind != BuffKind::ExtraStrikes);
        let ev = play(&mut r, 1.5);
        let swings = ev.iter().filter(|x| matches!(x, Event::EnemyHit { source: HitSource::WeaponSwing, .. })).count();
        assert!((1..3).contains(&swings), "{swings}");
    }

    /// While Slice and Dice is up, weapon swings are 20 % faster, on top
    /// of its damage; the speed goes when the buff does.
    #[test]
    fn slice_and_dice_swings_twenty_percent_faster_while_it_is_up() {
        let mut r = run_with("druid", "cat_form", false);
        foe(&mut r, (2.0, 0.0), 1e9);
        let before = r.swing_speed(0);
        let damage = r.heroes[0].stats.damage;
        learn_and_cast(&mut r, "slice_and_dice");
        assert!((r.swing_speed(0) - before - 0.2).abs() < 1e-5);
        assert!(r.heroes[0].stats.damage > damage, "and its damage still");
        let buff = r.heroes[0].buffs.iter().find(|b| b.kind == BuffKind::SwingSpeed).copied().unwrap();
        assert!((buff.until - r.now - 8.0).abs() < 1e-4, "as long as the buff: 40 % of its 20 sec");
        r.now = buff.until + 0.1;
        assert!((r.swing_speed(0) - before).abs() < 1e-5, "gone with the buff");
        let a = r.content.ability("slice_and_dice").unwrap();
        assert!(a.description.contains("{swing_speed}"), "its tooltip says so");
    }

    #[test]
    fn berserks_targets_card_adds_one_more() {
        let mut r = run_with("druid", "cat_form", false);
        foe(&mut r, (2.0, 0.0), 1e9);
        let a = r.content.ability("berserk").unwrap().clone();
        r.heroes[0].spells.learn(&a, &[]);
        let mut inst = r.heroes[0].spells.known.iter().find(|s| s.spell == "berserk").unwrap().instance();
        inst.extra_targets_from_cards = 1;
        assert!(cast_now(&mut r, "berserk", &inst));
        assert_eq!(r.heroes[0].buffs.iter().find(|b| b.kind == BuffKind::ExtraStrikes).unwrap().value, 3.0);
    }

    #[test]
    fn critters_run_at_enemies_and_plants_burst_against_them() {
        let mut r = run_with("druid", "restoration", false);
        foe(&mut r, (8.0, 0.0), 1e9);
        foe(&mut r, (10.0, 3.0), 1e9);
        let ev = play(&mut r, 7.0);
        assert_eq!(ev.iter().filter(|x| matches!(x, Event::SummonAppeared { kind: SummonKind::Critter, .. })).count(), 6);
        let strikes = dealt_by(&ev, "critter_strike");
        assert!(strikes.len() >= 12, "{}", strikes.len());
        assert!(strikes.iter().all(|d| (d - base_damage(&r, "critter_strike") / 6.0 * r.heroes[0].stats.damage).abs() < 0.01));
        r.heroes[0].kit.counter = 99.9;
        r.kit_on_heal(0, 6.0);
        let ev = play(&mut r, 3.0);
        assert!(!dealt_by(&ev, "emerald_slumber").is_empty());
    }

    #[test]
    fn plants_sprout_round_the_druid_walk_at_the_horde_and_burst_two_seconds_later() {
        let mut r = run_with("druid", "restoration", false);
        foe(&mut r, (12.0, 0.0), 1e9);
        let hero = r.heroes[0].pos;
        r.heroes[0].kit.counter = 99.9;
        r.kit_on_heal(0, 6.0);
        let ev = r.drain_events();
        let spots: Vec<Vec2> = ev.iter().filter_map(|e| if let Event::SummonAppeared { kind: SummonKind::Plant, at, .. } = e { Some(*at) } else { None }).collect();
        assert_eq!(spots.len(), 4);
        assert!(spots.iter().all(|s| (dist(*s, hero) - PLANT_APPEAR_DISTANCE).abs() < 0.01), "{spots:?}");
        // not yet: they walk first
        let ev = play(&mut r, PLANT_BURST_AFTER - 0.3);
        assert!(dealt_by(&ev, "emerald_slumber").is_empty());
        let walked = r.pets.iter().filter(|p| p.summon.as_ref().is_some_and(|s| s.kind == SummonKind::Plant)).all(|p| dist(p.segment.position(r.now), hero) > PLANT_APPEAR_DISTANCE + 1.0);
        assert!(walked, "the plants walked at the enemy");
        let ev = play(&mut r, 0.6);
        // each of the four bursts once, its damage as before
        let bursts = dealt_by(&ev, "emerald_slumber");
        assert_eq!(bursts.len(), 4);
        assert!(r.pets.iter().all(|p| p.summon.as_ref().is_none_or(|s| s.kind != SummonKind::Plant) || p.until.is_some_and(|u| u <= r.now)));
    }

    #[test]
    fn tentacles_rise_in_a_ring_round_the_priest() {
        let mut r = run_with("priest", "mind_spike", false);
        foe(&mut r, (10.0, 0.0), 1e9);
        r.heroes[0].kit.counter = 98.0;
        let hero = r.heroes[0].pos;
        let ev = play(&mut r, 0.6);
        let spots: Vec<Vec2> = ev.iter().filter_map(|e| if let Event::SummonAppeared { at, .. } = e { Some(*at) } else { None }).collect();
        assert_eq!(spots.len(), 2);
        let (near, far) = TENTACLE_APPEAR_RING;
        assert!(spots.iter().all(|s| (near - 0.01..=far + 0.01).contains(&dist(*s, hero))), "{spots:?}");
    }

    #[test]
    fn the_ghostly_double_is_the_one_seen_casting() {
        let mut r = run_with("shaman", "wellspring", false);
        foe(&mut r, (10.0, 0.0), 1e9);
        r.heroes[0].kit.counter = 99.0;
        r.kit_on_heal(0, 6.0);
        let ev = r.drain_events();
        let double = ev.iter().find_map(|e| if let Event::SummonAppeared { id, kind: SummonKind::Clone, .. } = e { Some(*id) } else { None }).unwrap();
        let ev = play(&mut r, 10.5);
        let looks: Vec<u32> = ["ancestral_lightning", "ancestral_flame", "ancestral_heal"].iter().map(|s| r.content.ability(s).unwrap().look_spell).collect();
        let by = |u: Unit| ev.iter().filter(|e| matches!(e, Event::SpellGo { caster, spell, .. } if *caster == u && looks.contains(spell))).count();
        assert!(by(Unit::Pet(double)) > 0);
        assert_eq!(by(Unit::Hero(r.heroes[0].id)), 0, "the hero casts none of them");
        assert!(r.cast_by_summon.is_none());
    }
    // ---- weapon loadouts (roadmap R3) --------------------------------

    /// The weapon swings' hits in `events`.
    fn swing_hits(events: &[Event]) -> Vec<f32> {
        events.iter().filter_map(|e| if let Event::EnemyHit { amount, source: HitSource::WeaponSwing, .. } = e { Some(*amount) } else { None }).collect()
    }

    /// The class's swing damage as `classes.ron` gives it (a one-hand swing's).
    fn class_swing_damage(r: &Run, class: &str) -> f32 {
        r.content.class(class).unwrap().auto_attack.unwrap().damage
    }

    #[test]
    fn the_first_loadout_is_the_default_and_its_effects_join_the_hero() {
        let r = run_armed("warrior", "slam", false, None);
        assert_eq!(r.heroes[0].loadout, "two_hander");
        assert!((r.heroes[0].stats.rage_per_swing - 0.1).abs() < 1e-6);
        assert_eq!(run_armed("warrior", "slam", false, Some("no_such_loadout")).heroes[0].loadout, "two_hander");
        let staff = run_armed("mage", "arcane_blast", false, None);
        assert_eq!(staff.heroes[0].loadout, "staff");
        assert!(swing_of(&staff.content, &staff.heroes[0]).is_none(), "a caster never swings");
        let sword = run_armed("mage", "arcane_blast", false, Some("sword_off_hand"));
        // +5 % damage against none
        assert!((staff.heroes[0].stats.damage - sword.heroes[0].stats.damage - 0.05).abs() < 1e-5);
        // the sword's +10 % damage to elites joins its modifiers, the staff's +10 % area its sheet
        assert!(sword.heroes[0].modifiers.iter().any(|m| m.stat == Stat::DamageVsElites && (m.value - 0.1).abs() < 1e-6));
        assert!((staff.heroes[0].stats.area - sword.heroes[0].stats.area - 0.1).abs() < 1e-5);
    }

    #[test]
    fn a_two_hander_swings_every_three_point_six_seconds_for_double_damage_and_more_rage() {
        let mut r = run_armed("warrior", "slam", false, Some("two_hander"));
        r.heroes[0].spells.known.clear();
        foe(&mut r, (3.0, 0.0), 1e9);
        let ev = play(&mut r, 0.05);
        let hits = swing_hits(&ev);
        assert_eq!(hits.len(), 1);
        let base = class_swing_damage(&r, "warrior");
        assert!((hits[0] - base * 2.0 * super::super::casting::SWING_SCALE * r.heroes[0].stats.damage).abs() < 0.01, "{hits:?}");
        let pace = r.heroes[0].stats.swing_factor.sqrt();
        assert!((r.heroes[0].swing_timer - 3.6 * pace).abs() < 1e-4, "{}", r.heroes[0].swing_timer);
        let Some(Resource::Rage { per_swing, .. }) = option_of(&r.content, &r.heroes[0]).and_then(|o| o.resource) else { panic!("rage") };
        assert!((r.heroes[0].kit.resource - per_swing * 1.1).abs() < 1e-3, "+10 % rage a swing");
    }

    #[test]
    fn dual_wield_swings_both_hands_on_their_own_timers_the_off_hand_for_half() {
        let mut r = run_armed("warrior", "slam", false, Some("dual_wield"));
        r.heroes[0].spells.known.clear();
        foe(&mut r, (3.0, 0.0), 1e9);
        let ev = play(&mut r, 0.05);
        let hits = swing_hits(&ev);
        assert_eq!(hits.len(), 2, "both hands at once: {hits:?}");
        let off_marks = ev.iter().filter(|e| matches!(e, Event::OffHandSwing { .. })).count();
        assert_eq!(off_marks, 1, "the off hand's swing is marked for the client");
        let full = class_swing_damage(&r, "warrior") * super::super::casting::SWING_SCALE * r.heroes[0].stats.damage;
        let off = r.content.loadout("warrior", Some("dual_wield")).unwrap().swings()[1].share;
        assert!((hits[0] - full).abs() < 0.01 && (hits[1] - full * off).abs() < 0.01, "{hits:?}");
        // +5 % swing speed
        assert!((r.heroes[0].stats.swing_speed - 0.05).abs() < 1e-6);
        let pace = r.heroes[0].stats.swing_factor.sqrt() / (1.0 + r.heroes[0].stats.swing_speed);
        assert!((r.heroes[0].swing_timer - 2.6 * pace).abs() < 1e-4);
        assert!((r.heroes[0].off_swing_timer - 2.4 * pace).abs() < 1e-4);
        // rage from each hand
        let Some(Resource::Rage { per_swing, .. }) = option_of(&r.content, &r.heroes[0]).and_then(|o| o.resource) else { panic!("rage") };
        assert!((r.heroes[0].kit.resource - 2.0 * per_swing).abs() < 1e-3);
        // 10.4 swing-seconds more: the main hand 4 more (2.6 each), the off hand 4 more (2.4 each)
        let ev = play(&mut r, 10.4 * pace + 0.3);
        assert_eq!(swing_hits(&ev).len(), 4 + 4);
    }

    #[test]
    fn the_shield_loadouts_block_thirty_percent_of_blows() {
        for (class, start) in [("warrior", "slam"), ("paladin", "denounce"), ("shaman", "lightning_bolt")] {
            let r = run_armed(class, start, false, Some("shield"));
            assert!((r.heroes[0].stats.block_chance - 0.3).abs() < 1e-6, "{class}");
        }
        let two = run_armed("paladin", "denounce", false, Some("two_hander"));
        let off = run_armed("paladin", "denounce", false, Some("off_hand"));
        let mut r = run_armed("paladin", "denounce", false, Some("shield"));
        assert!((off.heroes[0].stats.damage - r.heroes[0].stats.damage - 0.1).abs() < 1e-5, "-10 % damage");
        let e = foe(&mut r, (2.0, 0.0), 1e9);
        let blocked = (0..2000).filter(|_| r.kit_block(0, e)).count();
        assert!((500..700).contains(&blocked), "about 600 of 2000: {blocked}");
        assert_eq!(two.heroes[0].stats.block_chance, 0.0, "no shield, no block");
    }

    #[test]
    fn two_maces_stun_with_their_swings_and_two_swords_strike_again() {
        let held_ticks = |loadout: &str| {
            let mut r = run_armed("rogue", "mutilate", false, Some(loadout));
            r.heroes[0].spells.known.clear();
            let e = foe(&mut r, (2.0, 0.0), 1e9);
            let mut held = 0;
            for _ in 0..1200 {
                r.tick(0.05);
                r.drain_events();
                if r.enemies.iter().any(|x| x.id == e && x.hold_until > r.now) {
                    held += 1;
                }
            }
            held
        };
        assert!(held_ticks("maces") > 0, "a mace swing stuns now and then");
        assert_eq!(held_ticks("daggers"), 0);
        let maces = run_armed("rogue", "mutilate", false, Some("maces"));
        assert!(maces.heroes[0].modifiers.iter().any(|m| m.stat == Stat::DamageVsCrowdControlled && (m.value - 0.1).abs() < 1e-6));
        let swords = run_armed("rogue", "mutilate", false, Some("swords"));
        assert!(swords.heroes[0].milestones.iter().any(|m| matches!(m, esurv_content::Milestone::StrikeAgain { chance, filter: esurv_content::SpellFilter::WeaponSwing, .. } if (*chance - 0.1).abs() < 1e-6)));
        // a swing strikes again: more hits than swings over a minute
        let mut r = swords;
        r.heroes[0].spells.known.clear();
        foe(&mut r, (2.0, 0.0), 1e9);
        let ev = play(&mut r, 60.0);
        let pace = r.heroes[0].stats.swing_factor.sqrt() / (1.0 + r.heroes[0].stats.swing_speed);
        let swings = (60.0 / (2.4 * pace)).ceil() + (60.0 / (1.8 * pace)).ceil();
        assert!(swing_hits(&ev).len() as f32 > swings, "{} hits, {swings} swings", swing_hits(&ev).len());
    }

    #[test]
    fn the_hunters_ranged_weapon_sets_auto_shot() {
        let bow = run_armed("hunter", "steady_shot", false, Some("bow"));
        let crossbow = run_armed("hunter", "steady_shot", false, Some("crossbow"));
        let base = class_swing_damage(&bow, "hunter");
        let b = swing_of(&bow.content, &bow.heroes[0]).unwrap();
        let c = swing_of(&crossbow.content, &crossbow.heroes[0]).unwrap();
        assert_eq!((b.swing, b.look_spell), (2.2, Some(75)));
        assert!((b.damage - base * 0.9).abs() < 1e-4);
        assert_eq!(c.swing, 3.4);
        assert!((c.damage - base * 1.6).abs() < 1e-4);
        assert!((bow.heroes[0].stats.move_speed - crossbow.heroes[0].stats.move_speed - 0.1).abs() < 1e-5);
    }

    #[test]
    fn a_druid_form_keeps_its_own_swing_and_gets_every_loadout_bonus() {
        let maul = run_armed("druid", "bear_form", false, Some("two_hander"));
        let staff = run_armed("druid", "bear_form", false, Some("staff"));
        let own = option_of(&maul.content, &maul.heroes[0]).unwrap().swing.unwrap();
        for r in [&maul, &staff] {
            let swings = swings_of(&r.content, &r.heroes[0]);
            assert_eq!(swings.len(), 1);
            assert_eq!((swings[0].swing, swings[0].damage), (own.swing, own.damage));
        }
        assert!((maul.heroes[0].stats.damage - staff.heroes[0].stats.damage - 0.1).abs() < 1e-5, "+10 % damage in Bear Form");
        let resto = run_armed("druid", "restoration", false, None);
        assert!(swing_of(&resto.content, &resto.heroes[0]).is_none(), "a caster druid never swings");
    }
}
