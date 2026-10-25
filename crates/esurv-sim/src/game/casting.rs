//! Hero spells inside a run: casting by delivery kind, hits that land later, effects over time,
//! lasting areas, totems, what a hit does to an enemy, and kills and their drops.

use esurv_content::{Ability, AutoAttack, Delivery, Milestone, SpellEffect, Stat};

use super::abilities::{
    burn_ticks, duration, finisher_damage, heal_amount, hit_damage, impact_damage, nova_repeats, radius, self_effect_ready,
    shield_amount, target_count, ticks, BurnTicks, Instance, AREA_FIRST_TICK, BUFF_ENEMY_RANGE, CHAIN_STEP, MAX_DOT_STACKS,
    SELF_EFFECT_MIN_SECONDS, TOTEM_ENEMY_RANGE,
};
use super::damage::lifesteal_heal;
use super::enemy_ai::Vec2;
use super::loot::{boss_drops, kill_experience, kill_gold, Pickup, ELITE_GOLD};
use super::rewards::{boss_drop_slots, gear_drop_chance, gear_item_entry, zone_tier};
use super::run::{Aim, Buff, BuffKind, EnemyKind, Event, Run, Unit, AMMO_SPELLS};
use super::spells::{RACIAL_RETRY, RETRY};
use super::stats::Modifier;

/// Bouncing chains: each jump lands this much later...
pub const BOUNCE_STEP: f32 = 0.08;
/// ...for this share of the jump before it (none lost with the chain milestone).
pub const BOUNCE_FALLOFF: f32 = 0.92;
/// Cones reach 60° either side of the hero's facing.
pub const CONE_COS: f32 = 0.5;
/// Lasting areas pick the best of this many enemy spots.
pub const AREA_SPOTS: usize = 24;
/// Totems stand this far in front of the hero and first pulse this long after being placed.
pub const TOTEM_OFFSET: f32 = 1.5;
pub const TOTEM_FIRST_PULSE: f32 = 0.5;
/// Then they pulse every this many seconds for as long as they last, so a longer duration
/// means more pulses.
pub const TOTEM_PULSE: f32 = 2.0;
/// A frost pet's shot slows what it hits by this share, for this many seconds.
pub const FROST_PET_SLOW: (f32, f32) = (0.3, 2.0);
/// A lasting area's look stays this long after its time is up.
pub const AREA_LOOK_AFTER: f32 = 0.3;
/// A totem cast by a milestone stands this far round from the hero's facing (to its left).
pub const TOTEM_PROC_TURN: f32 = std::f32::consts::FRAC_PI_2;
/// Single-target spells take the ordinary enemies within this many yards of the nearest one
/// first, the most wounded first: the lowest share of their health left (see
/// `enemies_in_target_order`).
pub const BUNCH_RADIUS: f32 = 6.0;
/// Holds (roots, stuns): bosses are held for this share of the time...
pub const HOLD_BOSS_SHARE: f32 = 0.25;
/// ...a chain of holds ends at most this long after it began...
pub const HOLD_CHAIN: f32 = 3.0;
/// ...and once a hold ends, no new one takes for this long.
pub const HOLD_IMMUNITY: f32 = 2.0;

/// What gave the hero the milestone (or other effect) a proc hit came from: the meter names the
/// hit's row after it. Presentation only: no rule of the run asks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProcOrigin {
    /// A talent, by its id.
    Talent(String),
    /// A ring or trinket, by its jewelry id (the Soulstone's extra life blast).
    Jewelry(String),
    /// An amulet, by its id.
    Amulet(String),
    /// A legendary amulet, by its id (`legendary:<main spell>`).
    Legendary(String),
    /// A weapon loadout, by its id.
    Loadout(String),
    /// A spell of the hero's own (an aura buff striking back), by its content id.
    Spell(String),
    /// Nothing known (a milestone put on the hero by hand, thorns).
    #[default]
    Other,
}

impl ProcOrigin {
    /// The origin of a worn amulet's effects: a legendary one's id starts with `legendary:`.
    pub fn of_amulet(id: &str) -> Self {
        if id.starts_with("legendary:") {
            ProcOrigin::Legendary(id.into())
        } else {
            ProcOrigin::Amulet(id.into())
        }
    }
}

/// Where a hit on an enemy came from (milestone filters ask).
#[derive(Clone, PartialEq)]
pub enum HitSource {
    Spell(String),
    /// A damage-over-time tick of this spell.
    Tick(String),
    /// The hero's own weapon swing.
    WeaponSwing,
    /// A pet's attack (the pet's id).
    Pet(u32),
    /// Damage with a look of its own in the combat log (`spell`, a `Spell.dbc` id; 0 shows
    /// nothing): milestone procs, thorns, auras striking back, an extra life's blast. `origin`:
    /// what gave the hero the proc (the meter's row; presentation only).
    Proc { spell: u32, school: esurv_content::School, origin: ProcOrigin },
    /// A blow of an enemy turned on the others (`Milestone::FearedTurn`): the striker's id.
    Enemy(u32),
    /// An ally a battle standard summoned (`game::assists`), by its character's id in
    /// `player_characters.ron`: the meter's row "Assist: <name>". The hits of the pets it
    /// summons stay `Pet` (their `PetUnit::assist` names the character).
    Assist(String),
    /// Anything else.
    Other,
}

/// Written as `#[derive(Debug)]` would, but a proc without its origin: the origin is only
/// presentation, so the golden run's event log (one Debug line an event) is unchanged by it.
impl std::fmt::Debug for HitSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HitSource::Spell(s) => f.debug_tuple("Spell").field(s).finish(),
            HitSource::Tick(s) => f.debug_tuple("Tick").field(s).finish(),
            HitSource::WeaponSwing => f.write_str("WeaponSwing"),
            HitSource::Pet(p) => f.debug_tuple("Pet").field(p).finish(),
            HitSource::Proc { spell, school, .. } => f.debug_struct("Proc").field("spell", spell).field("school", school).finish(),
            HitSource::Enemy(e) => f.debug_tuple("Enemy").field(e).finish(),
            HitSource::Assist(c) => f.debug_tuple("Assist").field(c).finish(),
            HitSource::Other => f.write_str("Other"),
        }
    }
}

/// The weapon swing hits for its damage × this × hero damage.
pub const SWING_SCALE: f32 = 1.5;
/// With no enemy in reach the swing tries again after this many seconds.
pub const SWING_RETRY: f32 = 0.15;

/// The swing timer of weapon `hand` of hero `h` (0 the main or ranged weapon, else the off hand).
fn hand_timer(h: &mut super::run::Hero, hand: usize) -> &mut f32 {
    if hand == 0 {
        &mut h.swing_timer
    } else {
        &mut h.off_swing_timer
    }
}

/// A damage-over-time effect on an enemy.
#[derive(Debug, Clone, PartialEq)]
pub struct Dot {
    pub hero: usize,
    pub spell: String,
    pub per_tick: f32,
    pub ticks_left: u32,
    pub interval: f32,
    pub next: f32,
    /// Made by a spell a milestone cast: its hits set off no milestones.
    pub from_proc: bool,
    /// The share of each tick's damage that heals the hero, for a spell whose on-hit effect heals
    /// from damage (`drain_life`, `devouring_plague`): it heals as each tick lands, the tick's damage
    /// × the share, never when the effect is applied. 0 for the others.
    pub leech: f32,
}

/// A hero hit on its way (projectile flight, chain steps, damage over time being applied).
#[derive(Debug, Clone, PartialEq)]
pub struct PendingHit {
    pub hero: usize,
    pub target: u32,
    pub at: f32,
    pub damage: f32,
    pub crit: bool,
    pub spell: String,
    /// Set for damage over time: (ticks, seconds between ticks).
    pub dot: Option<(u32, f32)>,
    /// A pet's shot (no spell, no on-hit effect): the pet's id.
    pub pet: Option<u32>,
    /// From a spell a milestone cast: sets off no milestones.
    pub from_proc: bool,
    /// The hero cast it belongs to (`Run::casts_made`), for healing charges; `None` for a totem's
    /// pulse or a pet's shot, which use none.
    pub cast: Option<u64>,
    /// The burn the hit leaves on the enemy as it lands (`Ability::burn`), before crit.
    pub burn: Option<BurnTicks>,
}

/// A lasting ground area.
#[derive(Debug, Clone, PartialEq)]
pub struct Area {
    /// Its lasting look on the ground.
    pub id: u32,
    pub hero: usize,
    pub spell: String,
    pub at: Vec2,
    pub radius: f32,
    pub per_tick: f32,
    pub ticks_left: u32,
    pub interval: f32,
    pub next: f32,
    pub from_proc: bool,
    /// Its radius as it landed (`GrowingArea` grows it from there).
    pub base_radius: f32,
    /// `GrowingArea`: yards it grows each time it damages an enemy, and its widest radius.
    pub growth: Option<(f32, f32)>,
    /// `AreaFollowsHero`: it moves with its hero.
    pub follows: bool,
    /// A milestone's area (a `FireTrail` patch): its hits show as this spell, in this school,
    /// in place of the spell's.
    pub proc_hit: Option<(u32, esurv_content::School, ProcOrigin)>,
    /// Where and how wide its look was last shown (`Event::AreaChanged` tells a change).
    pub shown_at: Vec2,
    pub shown_radius: f32,
    /// A lasting area a `LeaveArea` milestone left: what it does beside its damage.
    pub left: Option<LeftArea>,
}

/// What a lasting area a milestone left does beside its damage (`Milestone::LeaveArea`).
#[derive(Debug, Clone, PartialEq)]
pub struct LeftArea {
    /// It slows every enemy inside by this much at each strike, for `slow_seconds`.
    pub slow: f32,
    pub slow_seconds: f32,
    /// It puts the poison of the hero's starting option on every enemy inside at each strike.
    pub poisons: bool,
    /// It rides along with this enemy while that one lives.
    pub rides: Option<u32>,
    /// Spirits run with it, a look only (`Event::SpiritsAppeared`, `Event::SpiritsStrike`).
    pub spirits: Option<esurv_content::AreaSpirits>,
}

/// The novas still to come of one cast of a nova that repeats around its hero (`nova_repeats`).
#[derive(Debug, Clone, PartialEq)]
pub struct RepeatNova {
    pub hero: usize,
    pub spell: String,
    /// One nova's hit on each enemy before crit, as worked out when the spell was cast.
    pub hit: f32,
    pub radius: f32,
    pub burn: Option<BurnTicks>,
    pub left: u32,
    pub interval: f32,
    pub next: f32,
    pub from_proc: bool,
}

/// A totem a hero placed.
#[derive(Debug, Clone, PartialEq)]
pub struct Totem {
    pub id: u32,
    pub hero: usize,
    pub spell: String,
    pub at: Vec2,
    pub until: f32,
    pub next_pulse: f32,
    pub interval: f32,
    pub inst: Instance,
    pub from_proc: bool,
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn stat_sum(mods: &[Modifier], stat: Stat) -> f32 {
    mods.iter().filter(|m| m.stat == stat).map(|m| m.value).sum()
}

impl Run {
    /// Living enemies within `r` of `at`, nearest first (spawn order among equals): the
    /// weapon swing and a bouncing chain take the
    /// first, a lasting area tests the first 24, a totem without a radius the first n.
    /// The enemies an area of `a` strikes around `at`: those within `r`, nearest first, at most
    /// the spell's `max_hits`.
    pub(crate) fn area_targets(&self, a: &Ability, at: Vec2, r: f32) -> Vec<u32> {
        let mut found = self.enemies_within(at, r);
        if a.kit.max_hits > 0 {
            found.truncate(a.kit.max_hits as usize);
        }
        found
    }

    pub(crate) fn enemies_within(&self, at: Vec2, r: f32) -> Vec<u32> {
        let mut found: Vec<(f32, u32)> = self
            .enemies
            .iter()
            .filter_map(|e| {
                let d = dist(e.position(self.now), at);
                (d <= r).then_some((d, e.id))
            })
            .collect();
        // a stable sort: equal distances keep spawn order
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        found.into_iter().map(|(_, id)| id).collect()
    }

    /// Living enemies within `r` of `at` in the order single-target spells take them: bosses,
    /// then elites, each nearest first; then the
    /// ordinary enemies within [`BUNCH_RADIUS`] of the nearest ordinary one (it included), the
    /// lowest health first; then the other ordinary enemies, nearest first.
    pub(crate) fn enemies_in_target_order(&self, at: Vec2, r: f32) -> Vec<u32> {
        // (distance, rank: 0 boss 1 elite 2 other, position, share of health left, id)
        // one that takes no damage now (a boss behind its waves, a figure) comes last
        let immune: Vec<u32> = self.enemies.iter().filter(|e| e.mech.damage_share(self.now) <= 0.0 && dist(e.position(self.now), at) <= r).map(|e| e.id).collect();
        let mut v: Vec<(f32, u8, Vec2, f32, u32)> = self
            .enemies
            .iter()
            .filter(|e| !immune.contains(&e.id))
            .filter_map(|e| {
                let p = e.position(self.now);
                let d = dist(p, at);
                let rank = if e.is_boss() { 0 } else if e.ranks_as_elite() { 1 } else { 2 };
                let left = if e.max_health > 0.0 { e.health / e.max_health } else { 1.0 };
                (d <= r).then_some((d, rank, p, left, e.id))
            })
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        // stable: each rank stays nearest first
        v.sort_by_key(|x| x.1);
        if let Some(first) = v.iter().position(|x| x.1 == 2) {
            let nearest = v[first].2;
            let ordinary = v.split_off(first);
            let (mut bunch, rest): (Vec<_>, Vec<_>) = ordinary.into_iter().partition(|x| dist(x.2, nearest) <= BUNCH_RADIUS);
            bunch.sort_by(|a, b| a.3.total_cmp(&b.3));
            v.extend(bunch);
            v.extend(rest);
        }
        v.into_iter().map(|x| x.4).chain(immune).collect()
    }

    pub(crate) fn enemy(&self, id: u32) -> Option<&super::run::EnemyUnit> {
        self.enemies.iter().find(|e| e.id == id)
    }

    pub(crate) fn enemy_pos(&self, id: u32) -> Option<Vec2> {
        self.enemy(id).map(|e| e.position(self.now))
    }

    pub(crate) fn roll_crit(&mut self, hero: usize, hit: f32) -> (f32, bool) {
        self.roll_crit_with(hero, hit, 0.0)
    }

    /// As `roll_crit`, with `bonus` more crit chance (a spell's own, `SpellKit::crit_bonus`).
    pub(crate) fn roll_crit_with(&mut self, hero: usize, hit: f32, bonus: f32) -> (f32, bool) {
        let s = self.heroes[hero].stats;
        if self.rng.unit() < s.crit_chance + bonus {
            (hit * s.crit_damage, true)
        } else {
            (hit, false)
        }
    }

    /// Heals hero `i` by `amount`; `spell` is the `Spell.dbc` id of the hero's heal spell that
    /// healed (0: none to show). Only such a spell's heal (and a heal over time, see
    /// `buffs_tick`) feeds the healing milestones: lifesteal, heals per kill, runes, level-ups and
    /// milestone heals do not.
    /// Heals hero `i` by `amount` of a spell's healing (instant, over time, a healing charge, from
    /// its damage), strengthened by the hero's heal and shield power; `spell` as for `heal_hero`.
    pub(crate) fn spell_heal(&mut self, i: usize, amount: f32, spell: u32) {
        let power = 1.0 + self.heroes[i].stats.heal_and_shield_power;
        self.heal_hero(i, amount * power, spell);
    }

    pub(crate) fn heal_hero(&mut self, i: usize, amount: f32, spell: u32) {
        let h = &mut self.heroes[i];
        if !h.alive || amount <= 0.0 {
            return;
        }
        let before = h.health;
        h.health = (h.health + amount).min(h.stats.max_health);
        let hero = h.id;
        let healed = h.health - before;
        self.push(Event::HeroHealed { hero, amount: healed, spell });
        if spell != 0 {
            self.healing_milestones(i, amount, healed);
            self.kit_on_heal(i, healed);
        }
    }

    /// Hero `i`'s spell slot `k` waits on the effect it put on the hero
    /// (`SpellKit::cooldown_after_effect`): true while that effect holds, its cooldown not
    /// counting; once it has ended (run out or taken away) the cooldown starts, the bar sweeping
    /// it from then.
    fn effect_holds_cooldown(&mut self, i: usize, k: usize) -> bool {
        let slot = &self.heroes[i].spells.known[k];
        if !slot.waits_for_effect {
            return false;
        }
        let Some(a) = self.content.ability(&slot.spell) else { return false };
        let (look, id, now) = (a.look_spell, a.id.clone(), self.now);
        if self.heroes[i].buffs.iter().any(|b| b.visual == look && b.until > now) {
            return true;
        }
        let slot = &mut self.heroes[i].spells.known[k];
        slot.waits_for_effect = false;
        let (hero, seconds) = (self.heroes[i].id, self.heroes[i].spells.known[k].timer);
        self.push(Event::SpellCooldown { hero, spell: id, seconds });
        false
    }

    /// Every hero spell slot counts down and casts when ready.
    pub(crate) fn spells_tick(&mut self, dt: f32) {
        let content = self.content.clone();
        for i in 0..self.heroes.len() {
            if !self.heroes[i].alive {
                continue;
            }
            // the weapon swing, then the known spells, then the race's spell
            self.weapon_swing(i, dt);
            for k in 0..self.heroes[i].spells.known.len() {
                if self.effect_holds_cooldown(i, k) {
                    continue;
                }
                if !self.heroes[i].spells.known[k].ready(dt) {
                    continue;
                }
                let Some(a) = content.ability(&self.heroes[i].spells.known[k].spell) else { continue };
                // a spell that waits for rage, energy or combo points looks again shortly
                if !self.kit_ready(i, a) {
                    self.heroes[i].spells.known[k].timer = super::kit::RESOURCE_RETRY;
                    continue;
                }
                let inst = self.kit_instance(i, a, self.heroes[i].spells.known[k].instance());
                self.kit_on_drain_cast(i, a);
                // a shot Lock and Load gave is a free cast: like a milestone's, it sets off no procs
                // (nor Lock and Load again)
                let free = self.heroes[i].kit.free_casts > 0 && self.is_main(i, &a.id);
                self.casting_proc += u32::from(free);
                let counted = self.count_own_cast(i, a);
                let cast = self.cast(i, a, &inst);
                self.casting_proc -= u32::from(free);
                if !cast {
                    self.uncount_cast(i, counted);
                }
                let mut at_once = false;
                if cast {
                    at_once = self.kit_after_cast(i, a);
                    self.kit_on_cast(i, a);
                    self.milestone_event(i, super::milestones::CombatEvent::Cast { spell: a.id.clone() });
                    // double-cast: every whole 100 % is one more cast, the rest the chance of one more
                    // (250 %: two extra casts and a 50 % chance of a third)
                    let d = self.heroes[i].stats.double_cast.max(0.0);
                    let extra = d.floor() as u32 + u32::from(self.rng.unit() < d.fract());
                    for _ in 0..extra {
                        self.cast(i, a, &inst);
                    }
                }
                let factor = self.heroes[i].stats.cooldown_factor;
                self.heroes[i].spells.known[k].after_cast(a, cast, factor, RETRY);
                if at_once {
                    self.heroes[i].spells.known[k].timer = 0.0;
                }
                if cast {
                    let (hero, mut seconds) = (self.heroes[i].id, self.heroes[i].spells.known[k].timer);
                    if a.kit.cooldown_after_effect && !at_once {
                        // unavailable while its effect holds (the bar sweeps that), its cooldown after
                        let now = self.now;
                        let holds = self.heroes[i].buffs.iter().filter(|b| b.visual == a.look_spell && b.until > now).map(|b| b.until - now).fold(0.0, f32::max);
                        if holds > 0.0 {
                            self.heroes[i].spells.known[k].waits_for_effect = true;
                            seconds = holds;
                        }
                    }
                    self.push(Event::SpellCooldown { hero, spell: a.id.clone(), seconds });
                }
            }
            let ready = self.heroes[i].spells.racial.as_mut().is_some_and(|s| s.ready(dt));
            if ready {
                let slot = self.heroes[i].spells.racial.clone().unwrap();
                if let Some(a) = content.ability(&slot.spell) {
                    let cast = self.cast(i, a, &slot.instance());
                    let factor = self.heroes[i].stats.cooldown_factor;
                    if let Some(s) = self.heroes[i].spells.racial.as_mut() {
                        s.after_cast(a, cast, factor, RACIAL_RETRY);
                    }
                    if cast {
                        let hero = self.heroes[i].id;
                        let seconds = self.heroes[i].spells.racial.as_ref().map_or(0.0, |s| s.timer);
                        self.push(Event::SpellCooldown { hero, spell: a.id.clone(), seconds });
                    }
                }
            }
        }
    }

    /// Casts `a` for hero `i`; false when it found nothing to do.
    pub fn cast(&mut self, i: usize, a: &Ability, inst: &Instance) -> bool {
        // Magnetic Pull: every few casts first draw the enemies near the hero in
        self.pull_before_cast(i, a);
        self.casts_made += 1;
        let serial = self.casts_made;
        let stats = self.heroes[i].stats;
        let at = self.heroes[i].pos;
        let hit = hit_damage(a, inst, stats.damage);
        let burn = burn_ticks(a, inst, stats.damage);
        let n = target_count(a, inst, stats.extra_targets) as usize;
        let kind = a.delivery;
        let ordered = self.kit_orders(i, a);
        let own = self.kit_cast(i, a, inst);
        let cast = match kind {
            _ if own.is_some() => own.unwrap_or(false),
            Delivery::Projectile | Delivery::Chain => {
                let found = if ordered { self.kit_targets(i, a, at, a.range) } else { self.enemies_in_target_order(at, a.range) };
                if found.is_empty() {
                    return false;
                }
                let list: Vec<u32> = if kind == Delivery::Chain {
                    let mut shuffled = found;
                    self.shuffle(&mut shuffled);
                    (0..n).map(|k| shuffled[k % shuffled.len()]).collect()
                } else {
                    found.into_iter().take(n).collect()
                };
                for (k, target) in list.into_iter().enumerate() {
                    let delay = if kind == Delivery::Chain { CHAIN_STEP * k as f32 } else { 0.0 };
                    self.fire(i, a, target, hit, delay, None, burn);
                }
                true
            }
            Delivery::Nova => {
                let r = radius(a, inst, stats.area);
                let targets = self.area_targets(a, at, r);
                let heal = match a.effect {
                    SpellEffect::Heal { share } => Some(share),
                    _ => None,
                };
                if targets.is_empty() && heal.is_none() {
                    return false;
                }
                // only this first nova of the cast may use a healing charge
                if self.nova_burst(i, a, &targets, r, hit, burn) {
                    self.spell_hit_landed(i, a, inst, serial);
                }
                let (count, interval) = nova_repeats(a, inst);
                if count > 1 {
                    let (spell, next, from_proc) = (a.id.clone(), self.now + interval, self.casting_proc > 0);
                    self.novas.push(RepeatNova { hero: i, spell, hit, radius: r, burn, left: count - 1, interval, next, from_proc });
                }
                if let Some(f) = heal {
                    let max = self.heroes[i].stats.max_health;
                    self.spell_heal(i, f * max, a.look_spell);
                }
                true
            }
            Delivery::GroundArea => self.place_area(i, a, inst, hit),
            Delivery::Bounce => self.bounce(i, a, inst, hit, n),
            Delivery::Melee | Delivery::Finisher => {
                let mut targets: Vec<u32> = if ordered { self.kit_targets(i, a, at, a.range) } else { self.enemies_in_target_order(at, a.range) }.into_iter().take(n).collect();
                if targets.is_empty() {
                    return false;
                }
                let more = self.extra_strikes(i, targets[0], &targets);
                targets.extend(more);
                self.show_cast(i, a, &targets, Aim::Unit(Unit::Enemy(targets[0])), 0.0);
                for t in targets {
                    let mut h = hit;
                    if kind == Delivery::Finisher || a.double_on_low_health {
                        if let Some(e) = self.enemy(t) {
                            h = finisher_damage(hit, e.health, e.max_health);
                        }
                    }
                    if a.kit.double_above > 0.0 && self.enemy(t).is_some_and(|e| e.health > e.max_health * a.kit.double_above) {
                        h *= 2.0;
                    }
                    let (h, crit) = self.roll_crit(i, h);
                    if let Some(d) = self.hit_enemy(Some(i), t, h, crit, HitSource::Spell(a.id.clone())) {
                        self.apply_on_hit(i, t, &a.effect, d);
                        self.add_burn(i, t, &a.id, burn);
                        self.spell_hit_landed(i, a, inst, serial);
                    }
                }
                true
            }
            Delivery::DamageOverTime => self.apply_dots(i, a, inst, hit, n),
            Delivery::Cone => {
                let len = radius(a, inst, stats.area);
                let dir = (self.heroes[i].facing.cos(), self.heroes[i].facing.sin());
                let now = self.now;
                let cone_cos = if a.kit.cone_degrees > 0.0 { (a.kit.cone_degrees.to_radians() / 2.0).cos() } else { CONE_COS };
                let mut inside: Vec<(f32, u32)> = self
                    .enemies
                    .iter()
                    .filter_map(|e| {
                        let p = e.position(now);
                        let d = dist(p, at);
                        (d > 0.01 && d <= len && ((p.0 - at.0) * dir.0 + (p.1 - at.1) * dir.1) / d > cone_cos).then_some((d, e.id))
                    })
                    .collect();
                // the spell's target cap takes the nearest
                if a.kit.max_hits > 0 {
                    inside.sort_by(|x, y| x.0.total_cmp(&y.0));
                    inside.truncate(a.kit.max_hits as usize);
                }
                let targets: Vec<u32> = inside.into_iter().map(|(_, id)| id).collect();
                if targets.is_empty() {
                    return false;
                }
                self.show_cast(i, a, &targets, Aim::Caster, len);
                for t in targets {
                    let (h, crit) = self.roll_crit(i, hit);
                    if let Some(d) = self.hit_enemy(Some(i), t, h, crit, HitSource::Spell(a.id.clone())) {
                        self.apply_on_hit(i, t, &a.effect, d);
                        self.add_burn(i, t, &a.id, burn);
                        self.spell_hit_landed(i, a, inst, serial);
                    }
                }
                true
            }
            Delivery::SelfEffect => self.self_effect(i, a, inst),
            Delivery::Totem => self.place_totem(i, a, inst),
        };
        if cast {
            let hero = self.heroes[i].id;
            self.push(Event::SpellCast { hero, spell: a.id.clone() });
            // a caster spell from a form: out of it for the cast, then straight back; so is a spell
            // that shifts into the hero's own form (Powershift)
            let form = super::kit::option_of(&self.content, &self.heroes[i]).and_then(|o| o.form.clone());
            if let Some(f) = form.filter(|f| f.caster_spells.iter().any(|s| *s == a.id) || f.look_spell == a.look_spell) {
                self.push(Event::FormLeft { hero, seconds: super::kit::POWERSHIFT_SECONDS, look_spell: f.look_spell });
            }
        }
        cast
    }

    /// The hero's weapon swings (classes with one): each weapon of its loadout that swings has
    /// its own timer (dual wield: two), a form its own swing.
    fn weapon_swing(&mut self, i: usize, dt: f32) {
        let content = self.content.clone();
        let swings = super::kit::swings_of(&content, &self.heroes[i]);
        for (hand, swing) in swings.iter().enumerate().take(2) {
            self.swing_hand(i, hand, swing, dt);
        }
    }

    /// One weapon's swing (`hand` 0 the main or ranged weapon, 1 the off hand): the nearest enemy
    /// within reach takes weapon damage (the hand's share) × 1.5 × hero damage × (1 + weapon
    /// damage bonuses), crit rolled.
    fn swing_hand(&mut self, i: usize, hand: usize, swing: &AutoAttack, dt: f32) {
        *hand_timer(&mut self.heroes[i], hand) -= dt;
        if *hand_timer(&mut self.heroes[i], hand) > 0.0 {
            return;
        }
        let h = &self.heroes[i];
        let Some(&target) = self.enemies_within(h.pos, swing.range).first() else {
            *hand_timer(&mut self.heroes[i], hand) = SWING_RETRY;
            return;
        };
        let weapon = 1.0 + stat_sum(&h.modifiers, Stat::WeaponDamage);
        let hit = weapon * swing.damage * SWING_SCALE * h.stats.damage * self.swing_damage_factor(i);
        let factor = self.heroes[i].stats.swing_factor;
        let speed = self.swing_speed(i);
        let more = self.extra_strikes(i, target, &[target]);
        self.land_swing(i, hand, swing, target, hit);
        // Berserk: the swing also strikes the enemies nearest its target, for its full damage
        let unrolled = weapon * swing.damage * SWING_SCALE * self.heroes[i].stats.damage;
        for x in more {
            let (h, crit) = self.roll_crit(i, unrolled);
            self.hit_enemy(Some(i), x, h, crit, HitSource::WeaponSwing);
        }
        // Hand of Justice: a chance for the swing to hit twice, the second a full weapon hit of its
        // own on the same target, else the nearest in reach
        let twice = self.heroes[i].stats.double_hit;
        if twice > 0.0 && self.rng.unit() < twice {
            let alive = self.enemies.iter().any(|e| e.id == target && e.health > 0.0);
            let pos = self.heroes[i].pos;
            let again = if alive { Some(target) } else { self.enemies_within(pos, swing.range).first().copied() };
            if let Some(again) = again {
                self.land_swing(i, hand, swing, again, hit);
            }
        }
        // read after the hit: a swing that sets off faster swings (Flurry) speeds up the next one
        let speed = speed + self.take_faster_swing(i);
        // the next swing after √(swing factor) × the swing time (the
        // listed time is the real one)
        *hand_timer(&mut self.heroes[i], hand) = factor.sqrt() * swing.swing / (1.0 + speed).max(0.1);
    }

    /// One weapon hit of hero `i`'s `hand` on `target` for `hit` before its crit roll: its shot or
    /// off-hand swing for the client, the hit, and what a swing that hits sets off.
    fn land_swing(&mut self, i: usize, hand: usize, swing: &AutoAttack, target: u32, hit: f32) {
        let (hit, crit) = self.roll_crit(i, hit);
        // a ranged swing shows its shot
        if let Some(look) = swing.look_spell {
            let enemy = Unit::Enemy(target);
            self.push(Event::SpellGo { caster: Unit::Hero(self.heroes[i].id), spell: look, hits: vec![enemy], aim: Aim::Unit(enemy), ammo: AMMO_SPELLS.contains(&look), reach: 0.0, triggered: false });
        }
        if hand == 1 && swing.look_spell.is_none() {
            self.push(Event::OffHandSwing { hero: self.heroes[i].id });
        }
        if self.hit_enemy(Some(i), target, hit, crit, HitSource::WeaponSwing).is_some() {
            self.use_healing_charge(i, None);
            self.kit_on_swing(i, target, swing);
        }
    }

    /// A hit of hero `i`'s cast number `cast` of `a` (cast as `inst`) landed: a spell that grants
    /// healing charges grants them (its own hit uses none), any other uses one charge per cast.
    pub(crate) fn spell_hit_landed(&mut self, i: usize, a: &Ability, inst: &Instance, cast: u64) {
        match a.effect {
            SpellEffect::HealingCharges { charges, share } => self.grant_healing_charges(i, a, inst, charges, share, cast),
            _ => self.use_healing_charge(i, Some(cast)),
        }
    }

    /// `bloodthirst`: `charges` healing charges for the spell's duration,
    /// each worth `share` of max health scaled as heals are (`heal_amount`). Granted again, they
    /// start over: full charges, full time.
    fn grant_healing_charges(&mut self, i: usize, a: &Ability, inst: &Instance, charges: u32, share: f32, cast: u64) {
        let until = self.now + duration(a, inst).max(SELF_EFFECT_MIN_SECONDS);
        let per_charge = heal_amount(share, inst, 1.0);
        let h = &mut self.heroes[i];
        h.buffs.retain(|b| b.kind != BuffKind::HealingCharges);
        h.buffs.push(Buff { kind: BuffKind::HealingCharges, value: per_charge, visual: a.look_spell, until });
        h.healing_charges = charges;
        h.charged_cast = cast;
    }

    /// A weapon swing (`cast` is `None`) or a hit of hero cast `cast`
    /// uses one of hero `i`'s healing charges, if any are left, and heals its share of max health.
    /// A cast uses at most one however many enemies it hits; the last charge ends the buff.
    fn use_healing_charge(&mut self, i: usize, cast: Option<u64>) {
        let now = self.now;
        let h = &mut self.heroes[i];
        if h.healing_charges == 0 || cast.is_some_and(|c| c == h.charged_cast) {
            return;
        }
        let active = h.buffs.iter().find(|b| b.kind == BuffKind::HealingCharges && b.until > now);
        let Some(share) = active.map(|b| b.value) else {
            h.healing_charges = 0;
            return;
        };
        if let Some(c) = cast {
            h.charged_cast = c;
        }
        h.healing_charges -= 1;
        if h.healing_charges == 0 {
            h.buffs.retain(|b| b.kind != BuffKind::HealingCharges);
        }
        let amount = share * h.stats.max_health;
        self.spell_heal(i, amount, 0);
    }

    /// The instance hero `i` casts spell `id` with now (a default one for a spell it does not know).
    pub(crate) fn known_instance(&self, i: usize, id: &str) -> Instance {
        let spells = &self.heroes[i].spells;
        spells.known.iter().chain(&spells.racial).find(|s| s.spell == id).map_or_else(Instance::default, |s| s.instance())
    }

    pub(crate) fn shuffle(&mut self, v: &mut [u32]) {
        for k in (1..v.len()).rev() {
            let j = (self.rng.next_u64() % (k as u64 + 1)) as usize;
            v.swap(k, j);
        }
    }

    /// Queues one hit on `target`: crit rolled now, landing after `delay` plus the missile's flight.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn fire(&mut self, i: usize, a: &Ability, target: u32, hit: f32, delay: f32, dot: Option<(u32, f32)>, burn: Option<BurnTicks>) {
        let (damage, crit) = self.roll_crit_with(i, hit, a.kit.crit_bonus);
        let speed = self.world.missile_speed(a.look_spell);
        let flight = match self.enemy_pos(target) {
            Some(p) if speed > 0.0 => dist(self.heroes[i].pos, p) / speed,
            _ => 0.0,
        };
        let from_proc = self.casting_proc > 0;
        // one look per target; an effect over time shows one too when it is applied
        let (hero, enemy) = (Unit::Hero(self.heroes[i].id), Unit::Enemy(target));
        self.push(Event::SpellGo { caster: hero, spell: a.look_spell, hits: vec![enemy], aim: Aim::Unit(enemy), ammo: AMMO_SPELLS.contains(&a.look_spell), reach: 0.0, triggered: false });
        let (at, spell, cast) = (self.now + delay + flight, a.id.clone(), Some(self.casts_made));
        self.hits.push(PendingHit { hero: i, target, at, damage, crit, spell, dot, pet: None, from_proc, cast, burn });
    }

    pub(crate) fn place_area(&mut self, i: usize, a: &Ability, inst: &Instance, hit: f32) -> bool {
        let stats = self.heroes[i].stats;
        let r = radius(a, inst, stats.area);
        let follows = self.area_follows(i, a);
        let at = if follows {
            // it falls around the hero, with an enemy within the spell's reach
            if self.enemies_within(self.heroes[i].pos, a.range.max(r)).is_empty() {
                return false;
            }
            self.heroes[i].pos
        } else if a.range <= 0.0 {
            // on the hero, and only with an enemy inside it
            if self.enemies_within(self.heroes[i].pos, r).is_empty() {
                return false;
            }
            self.heroes[i].pos
        } else if self.kit_orders(i, a) {
            // on the first enemy of its own order
            let Some(&t) = self.kit_targets(i, a, self.heroes[i].pos, a.range).first() else { return false };
            self.enemy_pos(t).unwrap_or(self.heroes[i].pos)
        } else {
            let mut spots: Vec<Vec2> = self
                .enemies_within(self.heroes[i].pos, a.range)
                .into_iter()
                .take(AREA_SPOTS)
                .filter_map(|id| self.enemy_pos(id))
                .collect();
            if spots.iter().any(|p| self.spot_free(*p)) {
                spots.retain(|p| self.spot_free(*p));
            }
            let Some(best) = spots.iter().copied().max_by_key(|p| self.enemies_within(*p, r).len()) else {
                return false;
            };
            // the first of the best spots
            spots.iter().copied().find(|p| self.enemies_within(*p, r).len() == self.enemies_within(best, r).len()).unwrap_or(best)
        };
        let (count, per_tick) = ticks(a, inst, hit);
        let interval = a.duration.max(0.01) / a.ticks.max(1) as f32;
        let from_proc = self.casting_proc > 0;
        self.show_cast(i, a, &[], Aim::Ground(at), r);
        // its lasting look stays for the area's whole time and a moment more
        let id = self.new_id();
        let until = self.now + duration(a, inst) + AREA_LOOK_AFTER;
        // a growing area may wear a legendary's look of its own
        let look = self.area_look(i, a).unwrap_or(a.look_spell);
        self.push(Event::AreaShown { id, caster: Unit::Hero(self.heroes[i].id), spell: look, at, radius: r, until: Some(until) });
        let first_tick = match a.kit.mechanic {
            Some(esurv_content::Mechanic::Delayed { seconds }) => seconds,
            _ => AREA_FIRST_TICK,
        };
        let growth = self.area_growth(i, a, r);
        self.areas.push(Area {
            id,
            hero: i,
            spell: a.id.clone(),
            at,
            radius: r,
            per_tick,
            ticks_left: count,
            interval,
            next: self.now + first_tick,
            from_proc,
            base_radius: r,
            growth,
            follows,
            proc_hit: None,
            shown_at: at,
            shown_radius: r,
            left: None,
        });
        self.area_impact(i, a, inst, at, r);
        if let Some(esurv_content::Mechanic::Summons(spec)) = &a.kit.mechanic {
            let summon_inst = self.release_instance(i);
            let extra = inst.extra_targets_from_cards + inst.extra_targets_from_talents;
            self.summon(i, spec, summon_inst, false, at, extra, spec.seconds);
        }
        true
    }

    /// A ground area with an impact (`Ability::impact`) strikes every
    /// enemy inside it at once as it lands, one crit roll each, as its ticks do: the spell's
    /// damage, with its on-hit effect and no healing charge.
    fn area_impact(&mut self, i: usize, a: &Ability, inst: &Instance, at: Vec2, r: f32) {
        if a.impact <= 0.0 {
            return;
        }
        let amount = impact_damage(a, inst, self.heroes[i].stats.damage);
        for t in self.area_targets(a, at, r) {
            let (h, crit) = self.roll_crit(i, amount);
            if let Some(d) = self.hit_enemy(Some(i), t, h, crit, HitSource::Spell(a.id.clone())) {
                self.apply_on_hit(i, t, &a.effect, d);
            }
        }
    }

    /// One nova of hero `i`'s `a`, `r` yards around the hero: its look, then each of `targets`
    /// takes `hit`, one crit roll each, with the spell's on-hit effect and burn. True when it
    /// landed on any.
    pub(crate) fn nova_burst(&mut self, i: usize, a: &Ability, targets: &[u32], r: f32, hit: f32, burn: Option<BurnTicks>) -> bool {
        self.show_cast(i, a, targets, Aim::Caster, r);
        let mut landed = false;
        for &t in targets {
            let (h, crit) = self.roll_crit(i, hit);
            if let Some(d) = self.hit_enemy(Some(i), t, h, crit, HitSource::Spell(a.id.clone())) {
                self.apply_on_hit(i, t, &a.effect, d);
                self.add_burn(i, t, &a.id, burn);
                landed = true;
            }
        }
        landed
    }

    fn bounce(&mut self, i: usize, a: &Ability, inst: &Instance, hit: f32, n: usize) -> bool {
        let stats = self.heroes[i].stats;
        let at = self.heroes[i].pos;
        let first = if self.kit_orders(i, a) { self.kit_targets(i, a, at, a.range).first().copied() } else { self.enemies_within(at, a.range).first().copied() };
        let Some(first) = first else { return false };
        let jump = radius(a, inst, stats.area);
        let burn = burn_ticks(a, inst, stats.damage);
        let mut chain = vec![first];
        // each jump: the nearest enemy to the last one struck that the chain has not struck yet
        while chain.len() < n {
            let Some(from) = self.enemy_pos(*chain.last().unwrap()) else { break };
            let Some(next) = self.enemies_within(from, jump).into_iter().find(|id| !chain.contains(id)) else { break };
            chain.push(next);
        }
        self.show_cast(i, a, &chain, Aim::Unit(Unit::Enemy(first)), 0.0);
        let no_falloff = self.heroes[i].milestones.iter().any(|m| matches!(m, Milestone::ChainNoFalloff));
        let falloff = if no_falloff { 1.0 } else { BOUNCE_FALLOFF };
        for (k, target) in chain.into_iter().enumerate() {
            let (damage, crit) = self.roll_crit(i, hit * falloff.powi(k as i32));
            self.hits.push(PendingHit {
                hero: i,
                target,
                at: self.now + BOUNCE_STEP * k as f32,
                damage,
                crit,
                spell: a.id.clone(),
                dot: None,
                pet: None,
                from_proc: self.casting_proc > 0,
                cast: Some(self.casts_made),
                burn,
            });
        }
        true
    }

    #[cfg(test)]
    fn dot_stacks(&self, id: u32, spell: &str) -> usize {
        self.enemy(id).map_or(0, |e| e.dots.iter().filter(|d| d.spell == spell && d.ticks_left > 0).count())
    }

    /// Seconds left on each stack of `spell` on enemy `id`.
    pub(crate) fn dot_times_left(&self, id: u32, spell: &str) -> Vec<f32> {
        let now = self.now;
        self.enemy(id).map_or_else(Vec::new, |e| {
            e.dots
                .iter()
                .filter(|d| d.spell == spell && d.ticks_left > 0)
                .map(|d| (d.ticks_left - 1) as f32 * d.interval + (d.next - now).max(0.0))
                .collect()
        })
    }

    /// Seconds until the stack of `spell` on enemy `id` that runs out first ends (0 if none).
    #[cfg(test)]
    fn dot_time_left(&self, id: u32, spell: &str) -> f32 {
        self.dot_times_left(id, spell).into_iter().min_by(f32::total_cmp).unwrap_or(0.0)
    }

    /// How many stacks of one effect over time an enemy can carry: 1 + the hero's extra stacks
    /// (at most 3 extra).
    fn dot_limit(&self, hero: usize) -> usize {
        1 + self.heroes[hero].stats.dot_stacks.min(MAX_DOT_STACKS) as usize
    }

    /// How many stacks of hero `hero`'s `spell` an enemy carries at most: the hero's limit, or
    /// the spell's own `StacksUpTo` if more (Lacerate).
    fn dot_limit_of(&self, hero: usize, spell: &str) -> usize {
        let own = match self.content.ability(spell).and_then(|a| a.kit.mechanic.as_ref()) {
            Some(esurv_content::Mechanic::StacksUpTo { max }) => *max as usize,
            _ => 0,
        };
        self.dot_limit(hero).max(own)
    }

    /// Effects over time go to bosses first, then elites, then normal enemies from the
    /// healthiest down; a cast puts at most one stack on any enemy, so over several casts the
    /// first in line fills up before the next one gets any (a cast with more targets sends the
    /// rest down the line). A full enemy is only taken again for a stack that would run out
    /// before the spell's next cast (it is renewed); otherwise the next enemy in line gets it.
    fn apply_dots(&mut self, i: usize, a: &Ability, inst: &Instance, hit: f32, n: usize) -> bool {
        let next_cast = super::spells::cooldown(a, inst, self.heroes[i].stats.cooldown_factor);
        let at = self.heroes[i].pos;
        let own_order = if self.kit_orders(i, a) { Some(self.kit_targets(i, a, at, a.range)) } else { None };
        let mut order: Vec<(u8, f32, u32)> = self
            .enemies_within(at, a.range)
            .into_iter()
            // one that takes no damage now comes last
            .filter_map(|id| self.enemy(id).map(|e| (if e.mech.damage_share(self.now) <= 0.0 { 3 } else if e.is_boss() { 0 } else if e.ranks_as_elite() { 1 } else { 2 }, e.health, id)))
            .collect();
        order.sort_by(|x, y| x.0.cmp(&y.0).then(y.1.total_cmp(&x.1)));
        if let Some(own) = own_order {
            order.sort_by_key(|x| own.iter().position(|id| *id == x.2).unwrap_or(usize::MAX));
        }
        let mut targets: Vec<u32> = Vec::new();
        let limit = self.dot_limit_of(i, &a.id);
        for (_, _, id) in order {
            if targets.len() >= n {
                break;
            }
            let times = self.dot_times_left(id, &a.id);
            let free = limit.saturating_sub(times.len());
            let renew = times.iter().filter(|&&t| t < next_cast).count();
            // at most one stack per enemy per cast
            if free + renew > 0 {
                targets.push(id);
            }
        }
        if targets.is_empty() {
            return false;
        }
        let more = self.extra_strikes(i, targets[0], &targets);
        targets.extend(more);
        let (count, per_tick) = ticks(a, inst, hit);
        let interval = a.duration.max(0.01) / a.ticks.max(1) as f32;
        for target in targets {
            self.fire(i, a, target, per_tick, 0.0, Some((count, interval)), None);
        }
        true
    }

    fn self_effect(&mut self, i: usize, a: &Ability, inst: &Instance) -> bool {
        let now = self.now;
        let h = &self.heroes[i];
        let enemy_near = !self.enemies_within(h.pos, BUFF_ENEMY_RANGE).is_empty();
        // a `CastOnCooldown` milestone casts it whenever an enemy is near, whatever it waits for
        let on_cooldown = enemy_near && h.milestones.iter().any(|m| matches!(m, Milestone::CastOnCooldown { filter } if filter.matches(a)));
        if !on_cooldown && !self_effect_ready(a, h.health, h.stats.max_health, h.shield > 0.0, enemy_near) {
            return false;
        }
        let max = h.stats.max_health;
        let buff_power = h.stats.buff_power;
        let heal_power = 1.0 + h.stats.heal_and_shield_power;
        // what a spell on the hero lasts: never under a tenth of a second
        let length = duration(a, inst).max(SELF_EFFECT_MIN_SECONDS);
        // a heal over time, a shield or a buff cast again replaces the one the spell left; an
        // immunity (and an instant heal, which leaves none) does not
        let replaces = match a.effect {
            SpellEffect::Immunity { .. } => false,
            SpellEffect::Heal { .. } => a.duration > 0.0,
            _ => true,
        };
        if replaces {
            self.heroes[i].buffs.retain(|b| b.visual != a.look_spell);
        }
        match a.effect {
            SpellEffect::Heal { share } => {
                let amount = heal_amount(share, inst, max) * heal_power;
                // over time when the spell has a duration of its own, else at once
                if a.duration > 0.0 {
                    // a heal over time: spread evenly, one share per second
                    let per_second = amount / length;
                    self.heroes[i].buffs.push(Buff { kind: BuffKind::Regen, value: per_second, visual: a.look_spell, until: now + length });
                } else {
                    self.heal_hero(i, amount, a.look_spell);
                }
                self.heal_after(i, a, amount);
            }
            SpellEffect::Shield { share, .. } => {
                let amount = shield_amount(share, inst, max, buff_power) * heal_power;
                let h = &mut self.heroes[i];
                h.shield = amount;
                h.absorbed = 0.0;
                h.buffs.push(Buff { kind: BuffKind::Shield, value: amount, visual: a.look_spell, until: now + length });
            }
            SpellEffect::Immunity { .. } => {
                self.heroes[i].buffs.push(Buff { kind: BuffKind::Immunity, value: 1.0, visual: a.look_spell, until: now + length });
            }
            SpellEffect::DamageBuff { amount, .. } => {
                self.heroes[i].buffs.push(Buff { kind: BuffKind::Damage, value: amount, visual: a.look_spell, until: now + length });
                self.refresh_stats(i);
            }
            SpellEffect::HasteBuff { amount, .. } => {
                self.heroes[i].buffs.push(Buff { kind: BuffKind::Haste, value: amount, visual: a.look_spell, until: now + length });
                self.refresh_stats(i);
            }
            SpellEffect::SpeedBuff { amount, .. } => {
                self.heroes[i].buffs.push(Buff { kind: BuffKind::Speed, value: amount, visual: a.look_spell, until: now + length });
                self.refresh_stats(i);
            }
            SpellEffect::Thorns { amount } => {
                let value = inst.damage_scale * inst.power_from_cards * inst.power_from_talents * amount;
                self.heroes[i].buffs.push(Buff { kind: BuffKind::Aura, value, visual: a.look_spell, until: now + length });
            }
            _ => return false,
        }
        // weapon swings faster while the spell's buff is up (Slice and Dice)
        if a.kit.swing_speed > 0.0 {
            self.heroes[i].buffs.push(Buff { kind: BuffKind::SwingSpeed, value: a.kit.swing_speed, visual: a.look_spell, until: now + length });
        }
        // a `WhileBuff` milestone's stats count from the moment the buff starts
        self.refresh_stats(i);
        let me = Unit::Hero(self.heroes[i].id);
        self.push(Event::SpellGo { caster: me, spell: a.look_spell, hits: vec![me], aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
        true
    }

    /// A cast's look: the spell from hero `i`, striking `hits`, aimed at `aim`.
    /// Hero `i` casts `a`: its look, listing `hits`, aimed at `aim`, covering `reach` yards when
    /// it is an area.
    pub(crate) fn show_cast(&mut self, i: usize, a: &Ability, hits: &[u32], aim: Aim, reach: f32) {
        let hits = hits.iter().map(|&id| Unit::Enemy(id)).collect();
        self.push(Event::SpellGo { caster: Unit::Hero(self.heroes[i].id), spell: a.look_spell, hits, aim, ammo: false, reach, triggered: false });
    }

    fn place_totem(&mut self, i: usize, a: &Ability, inst: &Instance) -> bool {
        let h = &self.heroes[i];
        if self.enemies_within(h.pos, TOTEM_ENEMY_RANGE).is_empty() {
            return false;
        }
        // a totem a milestone casts stands to the hero's side, not in front
        let facing = h.facing + if self.proc_depth > 0 { TOTEM_PROC_TURN } else { 0.0 };
        let at = (h.pos.0 + facing.cos() * TOTEM_OFFSET, h.pos.1 + facing.sin() * TOTEM_OFFSET);
        let length = duration(a, inst);
        self.show_cast(i, a, &[], Aim::Caster, 0.0);
        let id = self.new_id();
        self.totems.push(Totem {
            id,
            hero: i,
            spell: a.id.clone(),
            at,
            until: self.now + length,
            next_pulse: self.now + TOTEM_FIRST_PULSE,
            interval: TOTEM_PULSE,
            inst: *inst,
            from_proc: self.casting_proc > 0,
        });
        true
    }

    /// Hits waiting to land, effects over time, lasting areas, repeating novas, totems and hold timers.
    pub(crate) fn effects_tick(&mut self) {
        let now = self.now;
        let content = self.content.clone();
        // hits that land now, in the order they were queued
        let (due, later): (Vec<PendingHit>, Vec<PendingHit>) = std::mem::take(&mut self.hits).into_iter().partition(|h| h.at <= now);
        self.hits = later;
        for h in due {
            if self.enemy(h.target).is_none() {
                continue;
            }
            self.casting_proc += u32::from(h.from_proc);
            match h.dot {
                Some((count, interval)) => {
                    // the spell's on-hit effect: healing from damage rides on every tick; any other
                    // lands once, as the effect is applied
                    let effect = content.ability(&h.spell).map(|a| a.effect);
                    let leech = match effect {
                        Some(SpellEffect::HealFromDamage { share }) => share,
                        _ => 0.0,
                    };
                    self.add_dot(h.target, Dot {
                        hero: h.hero,
                        spell: h.spell.clone(),
                        per_tick: h.damage,
                        ticks_left: count,
                        interval,
                        next: now + interval,
                        from_proc: h.from_proc,
                        leech,
                    });
                    if let Some(effect) = effect.filter(|e| !matches!(e, SpellEffect::HealFromDamage { .. })) {
                        self.apply_on_hit(h.hero, h.target, &effect, h.damage);
                    }
                    // the effect landing is the cast's hit (its ticks use no healing charge)
                    if let (Some(a), Some(cast)) = (content.ability(&h.spell), h.cast) {
                        let inst = self.known_instance(h.hero, &a.id);
                        self.spell_hit_landed(h.hero, a, &inst, cast);
                    }
                }
                None if h.pet.is_some() => {
                    let pet = h.pet.unwrap_or(0);
                    self.hit_enemy(Some(h.hero), h.target, h.damage, h.crit, HitSource::Pet(pet));
                    let frost = self.pets.iter().find(|p| p.id == pet).and_then(|p| content.pets.iter().find(|k| k.name == p.pet)).is_some_and(|k| k.school == esurv_content::School::Frost);
                    if frost {
                        if let Some(e) = self.enemies.iter_mut().find(|e| e.id == h.target) {
                            e.slow = Some((FROST_PET_SLOW.0, now + FROST_PET_SLOW.1));
                        }
                    }
                }
                None => {
                    // a spell that doubles on low health checks the enemy as the hit lands
                    let mut damage = h.damage;
                    if let (Some(a), Some(e)) = (content.ability(&h.spell), self.enemy(h.target)) {
                        if a.double_on_low_health {
                            damage = finisher_damage(damage, e.health, e.max_health);
                        }
                    }
                    if let Some(d) = self.hit_enemy(Some(h.hero), h.target, damage, h.crit, HitSource::Spell(h.spell.clone())) {
                        if let Some(a) = content.ability(&h.spell) {
                            self.apply_on_hit(h.hero, h.target, &a.effect, d);
                            self.add_burn(h.hero, h.target, &a.id, h.burn);
                            if let Some(cast) = h.cast {
                                let inst = self.known_instance(h.hero, &a.id);
                                self.spell_hit_landed(h.hero, a, &inst, cast);
                            }
                        }
                    }
                }
            }
            self.casting_proc -= u32::from(h.from_proc);
        }
        // effects over time
        let mut ticks_due = Vec::new();
        let mut ended = Vec::new();
        for e in &mut self.enemies {
            for d in &mut e.dots {
                while d.ticks_left > 0 && d.next <= now {
                    d.ticks_left -= 1;
                    d.next += d.interval;
                    ticks_due.push((e.id, d.hero, d.per_tick, d.spell.clone(), d.from_proc, d.leech));
                    if d.ticks_left == 0 && !d.from_proc {
                        ended.push((e.id, d.clone()));
                    }
                }
            }
            e.dots.retain(|d| d.ticks_left > 0);
        }
        for (id, hero, amount, spell, from_proc, leech) in ticks_due {
            self.casting_proc += u32::from(from_proc);
            // Pandemic: a tick can crit, for its bonus
            let (amount, crit) = match self.dot_crit_bonus(hero) {
                Some(bonus) if self.rng.unit() < self.heroes[hero].stats.crit_chance => (amount * (1.0 + bonus), true),
                _ => (amount, false),
            };
            self.hit_enemy(Some(hero), id, amount, crit, HitSource::Tick(spell));
            // healing from damage: the tick's damage × the share, as each tick lands
            if leech > 0.0 {
                self.spell_heal(hero, amount * leech, 0);
            }
            self.casting_proc -= u32::from(from_proc);
        }
        // an effect that ran out on an enemy still standing (a dead one's go with its death)
        for (id, dot) in ended {
            if let Some(at) = self.enemy_pos(id) {
                if self.casting_proc == 0 {
                    self.milestone_event(dot.hero, super::milestones::CombatEvent::DotEnded { target: id, at, dot });
                }
            }
        }
        // lasting areas
        let mut area_hits = Vec::new();
        // an area that rides along with an enemy stands where that enemy now is
        let riders: Vec<(usize, Vec2)> = self
            .areas
            .iter()
            .enumerate()
            .filter_map(|(k, a)| a.left.as_ref().and_then(|l| l.rides).and_then(|t| self.enemy_pos(t)).map(|p| (k, p)))
            .collect();
        for (k, p) in riders {
            self.areas[k].at = p;
        }
        let heroes = &self.heroes;
        for ar in &mut self.areas {
            // an area that follows its hero stands where the hero now is
            if ar.follows && heroes[ar.hero].alive {
                ar.at = heroes[ar.hero].pos;
            }
            while ar.ticks_left > 0 && ar.next <= now {
                ar.ticks_left -= 1;
                ar.next += ar.interval;
                area_hits.push((ar.id, ar.hero, ar.at, ar.radius, ar.per_tick, ar.spell.clone(), ar.from_proc, ar.proc_hit.clone(), ar.left.clone()));
            }
        }
        self.areas.retain(|a| a.ticks_left > 0);
        for (id, hero, at, r, amount, spell, from_proc, proc_hit, left) in area_hits {
            self.casting_proc += u32::from(from_proc);
            self.kit_on_area_tick(hero, &spell, from_proc);
            let struck = match content.ability(&spell) {
                Some(a) => self.area_targets(a, at, r),
                None => self.enemies_within(at, r),
            };
            let source = match &proc_hit {
                Some((spell, school, origin)) => HitSource::Proc { spell: *spell, school: *school, origin: origin.clone() },
                None => HitSource::Spell(spell.clone()),
            };
            // the spirits running with it strike what it strikes, one enemy each (a look only)
            if let Some(n) = left.as_ref().and_then(|l| l.spirits.as_ref()).map(|s| s.count as usize) {
                if !struck.is_empty() {
                    self.push(Event::SpiritsStrike { area: id, targets: struck.iter().copied().take(n).collect() });
                }
            }
            let mut landed = 0;
            for t in struck {
                // a milestone's area slows and poisons what is inside, and may deal no damage
                if let Some(l) = &left {
                    if l.slow > 0.0 {
                        self.apply_on_hit(hero, t, &SpellEffect::Slow { amount: l.slow, seconds: l.slow_seconds }, 0.0);
                    }
                    if l.poisons {
                        self.put_option_poison(hero, t);
                    }
                    if amount <= 0.0 {
                        continue;
                    }
                }
                let (h, crit) = self.roll_crit(hero, amount);
                if let Some(d) = self.hit_enemy(Some(hero), t, h, crit, source.clone()) {
                    landed += 1;
                    if let (Some(a), None) = (content.ability(&spell), &proc_hit) {
                        self.apply_on_hit(hero, t, &a.effect, d);
                    }
                }
            }
            self.casting_proc -= u32::from(from_proc);
            self.grow_area(id, landed);
        }
        self.reshow_changed_areas();
        // novas that repeat around their hero, wherever the hero now stands; a fallen hero's stop
        let mut bursts = Vec::new();
        let heroes = &self.heroes;
        for n in &mut self.novas {
            if !heroes[n.hero].alive {
                n.left = 0;
            }
            while n.left > 0 && n.next <= now {
                n.left -= 1;
                n.next += n.interval;
                bursts.push(n.clone());
            }
        }
        self.novas.retain(|n| n.left > 0);
        for n in bursts {
            let Some(a) = content.ability(&n.spell) else { continue };
            self.casting_proc += u32::from(n.from_proc);
            let targets = self.area_targets(a, self.heroes[n.hero].pos, n.radius);
            self.nova_burst(n.hero, a, &targets, n.radius, n.hit, n.burn);
            self.casting_proc -= u32::from(n.from_proc);
        }
        // totems
        let mut pulses = Vec::new();
        for t in &mut self.totems {
            while t.next_pulse <= now.min(t.until) {
                t.next_pulse += t.interval.max(0.01);
                pulses.push(t.clone());
            }
        }
        self.totems.retain(|t| t.until > now);
        for t in pulses {
            let Some(a) = content.ability(&t.spell) else { continue };
            self.casting_proc += u32::from(t.from_proc);
            let stats = self.heroes[t.hero].stats;
            // a spell the hero still knows pulses with its current upgrades
            let inst = self.heroes[t.hero].spells.known.iter().find(|s| s.spell == t.spell).map_or(t.inst, |s| s.instance());
            let hit = hit_damage(a, &inst, stats.damage);
            let pulse = a.pulse_spell.unwrap_or(a.look_spell);
            let totem = Unit::Totem(t.id);
            if a.radius > 0.0 {
                // around the totem: one look listing all it reaches, every pulse; no on-hit
                let reach = radius(a, &inst, stats.area);
                let targets = self.enemies_within(t.at, reach);
                let hits = targets.iter().map(|&id| Unit::Enemy(id)).collect();
                self.push(Event::SpellGo { caster: totem, spell: pulse, hits, aim: Aim::Caster, ammo: false, reach, triggered: false });
                if a.damage > 0.0 {
                    for target in targets {
                        let (h, crit) = self.roll_crit(t.hero, hit);
                        self.hit_enemy(Some(t.hero), target, h, crit, HitSource::Spell(a.id.clone()));
                    }
                }
            } else {
                // a shot at each of the first enemies in range, flying at the pulse spell's speed
                let n = target_count(a, &inst, stats.extra_targets) as usize;
                let targets: Vec<u32> = self.enemies_within(t.at, a.range).into_iter().take(n).collect();
                let speed = self.world.missile_speed(pulse);
                for target in targets {
                    let enemy = Unit::Enemy(target);
                    self.push(Event::SpellGo { caster: totem, spell: pulse, hits: vec![enemy], aim: Aim::Unit(enemy), ammo: false, reach: 0.0, triggered: false });
                    if a.damage > 0.0 {
                        let (damage, crit) = self.roll_crit(t.hero, hit);
                        let flight = match self.enemy_pos(target) {
                            Some(p) if speed > 0.0 => dist(t.at, p) / speed,
                            _ => 0.0,
                        };
                        self.hits.push(PendingHit { hero: t.hero, target, at: now + flight, damage, crit, spell: a.id.clone(), dot: None, pet: None, from_proc: t.from_proc, cast: None, burn: None });
                    }
                }
            }
            let heal: f32 = self.heroes[t.hero]
                .milestones
                .iter()
                .map(|m| if let Milestone::TotemHeal { share } = m { *share } else { 0.0 })
                .sum();
            if heal > 0.0 {
                let max = self.heroes[t.hero].stats.max_health;
                self.heal_hero(t.hero, heal * max, 0);
            }
            self.casting_proc -= u32::from(t.from_proc);
        }
        // holds that run out leave the enemy hold-immune for a while
        for e in &mut self.enemies {
            if e.hold_until > 0.0 && now >= e.hold_until {
                e.hold_until = 0.0;
                e.hold_immune_until = now + HOLD_IMMUNITY;
            }
        }
    }

    /// A hit of hero `hero`'s spell `spell` that landed on enemy `id`
    /// leaves its burn there, if it has one: an effect over time of that spell, first tick one
    /// interval after the hit, crit rolled once as an effect over time's is, stacking and renewing
    /// as the spell's effects over time do (`add_dot`).
    fn add_burn(&mut self, hero: usize, id: u32, spell: &str, burn: Option<BurnTicks>) {
        let Some(b) = burn else { return };
        let (per_tick, _) = self.roll_crit(hero, b.per_tick);
        let (from_proc, next) = (self.casting_proc > 0, self.now + b.interval);
        let spell = spell.to_string();
        self.add_dot(id, Dot { hero, spell, per_tick, ticks_left: b.ticks, interval: b.interval, next, from_proc, leech: 0.0 });
    }

    pub(crate) fn add_dot(&mut self, id: u32, dot: Dot) {
        let max = self.dot_limit_of(dot.hero, &dot.spell);
        let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) else { return };
        let same: Vec<usize> = (0..e.dots.len()).filter(|&k| e.dots[k].spell == dot.spell).collect();
        if same.len() >= max {
            // full: the stack that runs out first is renewed
            let left = |d: &Dot| d.ticks_left as f32 * d.interval + d.next;
            let first_out = same.into_iter().min_by(|&x, &y| left(&e.dots[x]).total_cmp(&left(&e.dots[y]))).unwrap();
            // the renewed stack keeps the ticks it had left, so none is lost;
            // its ticks share out the damage both had left, so a crit on one never lands on the other's
            let old = &e.dots[first_out];
            let mut dot = dot;
            let total = old.per_tick * old.ticks_left as f32 + dot.per_tick * dot.ticks_left as f32;
            dot.ticks_left = dot.ticks_left.saturating_add(old.ticks_left);
            dot.per_tick = total / dot.ticks_left.max(1) as f32;
            dot.next = dot.next.min(old.next);
            e.dots[first_out] = dot;
        } else {
            e.dots.push(dot);
        }
    }

    /// The gear roll when boss `b` dies: each piece of this zone the boss can drop (its armor slots,
    /// or the class's weapons for the final boss) drops with the zone's chance × the run-option
    /// bonus. Every hero rolls for their own class; won pieces join their run loot at once.
    fn roll_boss_gear(&mut self, b: usize) {
        let content = self.content.clone();
        let finals: Vec<Vec<bool>> = content.zones.iter().map(|z| z.bosses.iter().map(|x| x.is_final).collect()).collect();
        let tier = zone_tier(&content, self.zone);
        let chance = gear_drop_chance(&content.balance.zone_scaling, content.zones[self.zone].difficulty, &self.options, &content.run_options);
        for h in 0..self.heroes.len() {
            let class = &content.classes[self.heroes[h].class];
            let weapons = content.gear_set(&class.id).map_or(0, |s| s.weapons.len() as u32);
            for slot in boss_drop_slots(&finals, self.zone, b, weapons) {
                if self.rng.unit() < chance {
                    let entry = gear_item_entry(class.wow_class_id, slot, self.zone as u32);
                    self.heroes[h].loot.push(super::run::GearDrop { slot, tier, entry });
                    let hero = self.heroes[h].id;
                    self.push(Event::GearDropped { hero, slot, tier, entry });
                }
            }
        }
    }

    /// A hit of `hit` on enemy `id` (by hero `hero`, if any): the per-target bonuses, at least
    /// 1 damage, lifesteal, and the kill. Returns the damage dealt, `None` if the enemy is gone.
    pub fn hit_enemy(&mut self, hero: Option<usize>, id: u32, hit: f32, crit: bool, source: HitSource) -> Option<f32> {
        let now = self.now;
        let k = self.enemies.iter().position(|e| e.id == id)?;
        let e = &self.enemies[k];
        let mut mult = 1.0;
        if let Some(h) = hero {
            let mods = &self.heroes[h].modifiers;
            let held = now < e.hold_until || super::run::EnemyUnit::active(e.slow, now).is_some() || now < e.fear_until;
            if held {
                mult += stat_sum(mods, Stat::DamageVsCrowdControlled);
            }
            if e.is_boss() || e.elite {
                mult += stat_sum(mods, Stat::DamageVsElites);
            }
            if e.health <= e.max_health * 0.3 {
                mult += stat_sum(mods, Stat::DamageBelow30Pct);
            }
            if !e.dots.is_empty() {
                mult += stat_sum(mods, Stat::DamageVsAfflicted);
            }
        }
        if let Some(v) = super::run::EnemyUnit::active(e.vulnerable, now) {
            mult += v;
        }
        // a shielded boss takes only its shield's share; an immune one
        // (behind its waves, a figure) or one reflecting spells (a spell's hit) takes nothing
        let spell = matches!(source, HitSource::Spell(_) | HitSource::Tick(_)) || matches!(source, HitSource::Proc { school, .. } if school != esurv_content::School::Physical);
        let shield = if spell && now < e.mech.phases.spell_immune_until { 0.0 } else { e.mech.damage_share(now) };
        if shield <= 0.0 {
            return Some(0.0);
        }
        // Mark of the Fallen Champion: more on an enemy marked for striking the hero, and a heal
        let (marked_bonus, marked_heal) = hero.map_or((0.0, 0.0), |h| self.attacker_mark_rule(h, k, &source));
        mult += marked_bonus;
        if let Some(h) = hero {
            mult *= self.shatter(h, k, &source);
            // a class call twisted it: it deals less (`game::boss_phases`)
            mult *= 1.0 - self.hero_weakened(h);
        }
        let dealt = (mult * hit * shield).max(1.0);
        self.enemies[k].health -= dealt;
        let by = hero.map(|h| self.heroes[h].id);
        self.push(Event::EnemyHit { id, amount: dealt, crit, by, source: source.clone() });
        if let Some(h) = hero {
            self.heroes[h].damage_dealt += dealt as f64;
            let heal = lifesteal_heal(dealt, self.heroes[h].stats.lifesteal);
            if heal > 0.0 {
                self.heal_hero(h, heal, 0);
            }
            if marked_heal > 0.0 {
                let max = self.heroes[h].stats.max_health;
                self.heal_hero(h, marked_heal * max, 0);
            }
        }
        if let Some(h) = hero {
            self.kit_on_hit(h, id, &source, dealt, crit);
        }
        let Some(k) = self.enemies.iter().position(|e| e.id == id) else { return Some(dealt) };
        let at = self.enemies[k].position(now);
        if self.enemies[k].health <= 0.0 {
            self.kill(k, hero, source.clone());
        }
        if let (Some(h), 0) = (hero, self.casting_proc) {
            let e = match source {
                HitSource::Tick(spell) => super::milestones::CombatEvent::Tick { target: id, at, spell, dealt },
                source => super::milestones::CombatEvent::Hit { target: id, at, source, crit, dealt },
            };
            self.milestone_event(h, e);
        }
        Some(dealt)
    }

    /// What a spell's on-hit effect does to enemy `id` (or for the hero) after a hit of `dealt`.
    pub(crate) fn apply_on_hit(&mut self, hero: usize, id: u32, effect: &SpellEffect, dealt: f32) {
        let now = self.now;
        match *effect {
            SpellEffect::HealFromDamage { share } => return self.spell_heal(hero, dealt * share, 0),
            SpellEffect::Heal { share } => {
                let max = self.heroes[hero].stats.max_health;
                return self.spell_heal(hero, share * max, 0);
            }
            SpellEffect::Shield { share, .. } => {
                let h = &mut self.heroes[hero];
                h.shield = h.shield.max(share * h.stats.max_health * (1.0 + h.stats.heal_and_shield_power));
                return;
            }
            _ => {}
        }
        let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) else { return };
        match *effect {
            SpellEffect::Slow { amount, seconds } => e.slow = Some((amount, now + seconds)),
            SpellEffect::Root { seconds } | SpellEffect::Stun { seconds } => {
                if e.hold_until <= now {
                    if now < e.hold_immune_until {
                        return;
                    }
                    e.hold_chain_start = now;
                }
                let share = if e.is_boss() { HOLD_BOSS_SHARE } else { 1.0 };
                let until = (now + seconds * share).min(e.hold_chain_start + HOLD_CHAIN);
                if until > e.hold_until {
                    e.hold_until = until;
                    let p = e.position(now);
                    e.segment = super::enemy_ai::Segment::standing(p, now);
                }
            }
            SpellEffect::Fear { seconds } => {
                if !e.is_boss() {
                    e.fear_until = now + seconds;
                    e.next_decision = now;
                }
            }
            SpellEffect::Weaken { amount, seconds } => e.weaken = Some((amount, now + seconds)),
            // the stronger vulnerability stays while it lasts
            SpellEffect::Vulnerable { amount, seconds } => {
                if super::run::EnemyUnit::active(e.vulnerable, now).is_none_or(|v| v <= amount) {
                    e.vulnerable = Some((amount, now + seconds));
                }
            }
            _ => {}
        }
    }

    /// Enemy at index `k` died (by hero `hero`, if any): rewards, drops, and the final boss.
    pub(crate) fn kill(&mut self, k: usize, hero: Option<usize>, source: HitSource) {
        // an ally's kill is the hero's for its rewards, but sets off nothing of the hero's
        let sets_off = !matches!(source, HitSource::Assist(_));
        self.kill_setting_off(k, hero, source, sets_off);
    }

    /// As `kill`; with `sets_off` false the kill sets off nothing of the hero's (an assist's, its
    /// pets' too), though its rewards are the hero's.
    pub(crate) fn kill_setting_off(&mut self, k: usize, hero: Option<usize>, source: HitSource, sets_off: bool) {
        let e = self.enemies.remove(k);
        let at = e.position(self.now);
        if let (Some(h), true) = (hero, sets_off) {
            self.kit_on_kill(h, &e);
        }
        if let (Some(h), 0, true) = (hero, self.casting_proc, sets_off) {
            let event = super::milestones::CombatEvent::Kill { at, source, max_health: e.max_health, dots: e.dots.clone() };
            self.milestone_event(h, event);
        }
        let by = hero.map(|h| self.heroes[h].id);
        self.push(Event::EnemyDied { id: e.id, by });
        if let Some(h) = hero {
            self.heroes[h].kills += 1;
            let per_kill = self.heroes[h].stats.heal_per_kill;
            if per_kill > 0.0 && sets_off {
                self.heal_hero(h, per_kill, 0);
            }
        }
        let content = self.content.clone();
        let zone = &content.zones[self.zone];
        match e.kind {
            EnemyKind::Roster(r) => {
                let def = &zone.enemies[r];
                if let (true, Some(h)) = (def.elite, hero) {
                    self.heroes[h].elites_killed += 1;
                }
                let xp = kill_experience(def, &content.balance.experience, &self.options, &mut self.rng);
                self.drop_orb(at, xp);
                if def.elite {
                    self.drop(Pickup::Gold, ELITE_GOLD, at);
                    let off = super::pickups::ELITE_CACHE_OFFSET;
                    self.drop(Pickup::BattlegroundCache, 0, (at.0 + off.0, at.1 + off.1));
                } else {
                    // only a normal kill feeds the kill-rate catch-up
                    self.director.on_kill(def.spawn_cost);
                    if let Some(g) = kill_gold(def, &self.options, &mut self.rng) {
                        self.drop(Pickup::Gold, g, at);
                    }
                }
                // a boss's lines waiting for this add's death (Hakkar's sons' blood)
                self.add_died(r, at);
            }
            EnemyKind::Rare { .. } => {
                if let Some(h) = hero {
                    self.heroes[h].rares_killed += 1;
                }
                self.rare_slain(at, hero)
            }
            EnemyKind::Boss(b) => {
                // where it fell (a revive brings it back there), then its timer bars go and its
                // death lines run
                self.extras.fallen.push((b, at));
                let attack = super::enemy_ai::attack_damage(e.damage, super::run::EnemyUnit::active(e.weaken, self.now), e.enraged);
                self.boss_dies(e.id, b, at, attack);
                // one played by a hand-made character with a death yell says its own line;
                // one whose script yells as it dies says only that
                let played = content.boss_character(&content.zones[self.zone], b, self.party_faction.other(), self.seed);
                let text = match played.filter(|c| !c.death_yell.is_empty()) {
                    Some(c) => Some(c.death_yell.clone()),
                    None => (!self.dies_yelling(b)).then(|| super::lines::BOSS_DEATH_YELL.into()),
                };
                if let Some(text) = text {
                    self.push(Event::Yell { id: e.id, text });
                }
                self.bosses_slain += 1;
                let (xp, gold) = boss_drops(self.bosses_slain, &content.balance.experience);
                self.drop_orb(at, xp);
                self.drop(Pickup::Gold, gold, at);
                self.drop_due_magnet(at);
                // several final bosses: the last of them to fall drops
                // the weapons and wins the run
                let finals_left = (self.next_boss..zone.bosses.len()).any(|x| x != b && zone.bosses[x].is_final)
                    || self.enemies.iter().any(|o| matches!(o.kind, EnemyKind::Boss(x) if zone.bosses[x].is_final));
                if !(zone.bosses[b].is_final && finals_left) {
                    self.roll_boss_gear(b);
                }
                // the moment is kept; the victory itself comes with the next tick that finds the
                // run running (`Run::tick`)
                if zone.bosses[b].is_final && !finals_left {
                    if self.won_at.is_none() && self.endless_since.is_none() {
                        self.won_at = Some(self.now);
                    }
                    // Endless's growth counts from a final boss's latest death
                    self.final_boss_fell_at = Some(self.now);
                }
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::Content;

    use super::*;
    use crate::game::enemy::RunOptions;
    use crate::game::run::{Flat, HeroSetup, Phase};
    use crate::game::abilities::{HEAL_BELOW, IMMUNITY_BELOW};

    fn content() -> Arc<Content> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        Arc::new(Content::load(dir).expect("content loads"))
    }

    fn run(class: &str) -> Run {
        // the mage plays its Fireball option here
        let start = (class == "mage").then(|| "fireball".to_string());
        let mut r = Run::new(
            content(),
            "elwynn",
            RunOptions::default(),
            &[HeroSetup { class: class.into(), race: "human".into(), start, ..Default::default() }],
            Box::new(Flat),
            11,
        )
        .unwrap();
        let (h, p) = (r.heroes[0].id, r.heroes[0].pos);
        r.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
        // start from an empty field: these tests place their own enemies
        r.enemies.clear();
        r.pickups.clear();
        r
    }

    /// A Warsong boss played by a character with a death yell (Tebay,
    /// Vodu) says it as it falls in place of a boss's own death yell; the others keep that.
    #[test]
    fn a_boss_that_yells_as_it_dies_says_it_as_it_falls() {
        let content = content();
        let horde = HeroSetup { class: "warrior".into(), race: "orc".into(), ..Default::default() };
        let mut death_yells = 0;
        for seed in 0..20 {
            let mut r = Run::new(content.clone(), "wsg", RunOptions::default(), std::slice::from_ref(&horde), Box::new(Flat), seed).unwrap();
            let zone = r.content.zone("wsg").unwrap().clone();
            for b in 0..zone.bosses.len() {
                let c = content.boss_character(&zone, b, esurv_content::Faction::Alliance, seed).unwrap();
                r.spawn_boss(b);
                r.drain_events();
                let k = r.enemies.len() - 1;
                let id = r.enemies[k].id;
                r.kill(k, None, HitSource::WeaponSwing);
                let said = r.drain_events().into_iter().find_map(|e| match e {
                    Event::Yell { id: y, text } if y == id => Some(text),
                    _ => None,
                });
                let dies_yelling = !c.death_yell.is_empty();
                let line = if dies_yelling { c.death_yell.clone() } else { super::super::lines::BOSS_DEATH_YELL.to_string() };
                assert_eq!(said, Some(line), "seed {seed} boss {b}: {}", c.name);
                death_yells += usize::from(dies_yelling);
            }
        }
        assert!(death_yells > 0, "Tebay came up");
    }

    /// Puts roster enemy 0 at `offset` from the hero.
    fn enemy_at(r: &mut Run, offset: Vec2) -> u32 {
        let p = r.heroes[0].pos;
        assert!(r.spawn_roster_enemy(0, Some((p.0 + offset.0, p.1 + offset.1))));
        r.enemies.last().unwrap().id
    }

    /// Single-target spells take bosses first, then elites, then the ordinary enemies bunched
    /// within 6 yd of the nearest ordinary one (the most wounded first), then the rest, nearest
    /// first; beyond the spell's range, none.
    #[test]
    fn single_target_spells_take_bosses_then_elites_then_the_bunch_by_health_then_the_nearest() {
        use super::super::enemy_ai::Segment;
        let mut r = run("mage");
        let near = enemy_at(&mut r, (3.0, 0.0)); // the nearest ordinary enemy
        let wounded = enemy_at(&mut r, (5.0, 0.0)); // 2 yd from it: in its bunch
        let across = enemy_at(&mut r, (-4.0, 0.0)); // 7 yd from it: not in the bunch
        let farther = enemy_at(&mut r, (0.0, 12.0));
        let elite = enemy_at(&mut r, (15.0, 0.0));
        let out_of_range = enemy_at(&mut r, (40.0, 0.0));
        r.spawn_boss(0);
        let boss = r.enemies.last().unwrap().id;
        let p = r.heroes[0].pos;
        let now = r.now;
        for e in r.enemies.iter_mut() {
            if e.id == elite {
                e.elite = true;
            }
            if e.id == wounded {
                e.health = 1.0;
            }
            if e.id == boss {
                e.segment = Segment::standing((p.0, p.1 + 25.0), now);
            }
        }
        let order = r.enemies_in_target_order(p, 30.0);
        assert_eq!(order, [boss, elite, wounded, near, across, farther]);
        assert!(!order.contains(&out_of_range));
        // with no boss or elite, the bunch comes first all the same
        r.enemies.retain(|e| e.id != boss && e.id != elite);
        assert_eq!(r.enemies_in_target_order(p, 30.0), [wounded, near, across, farther]);
    }

    /// One marked `focus_as_elite` (the Nightmare Suppressors) is picked as an elite is, before
    /// nearer ordinary enemies, its own kind's numbers kept.
    #[test]
    fn one_to_focus_is_picked_as_an_elite() {
        let mut r = run("mage");
        let near = enemy_at(&mut r, (3.0, 0.0));
        let focus = enemy_at(&mut r, (15.0, 0.0));
        let health = r.enemy(focus).unwrap().max_health;
        for e in r.enemies.iter_mut().filter(|e| e.id == focus) {
            e.focus_as_elite = true;
        }
        let p = r.heroes[0].pos;
        assert_eq!(r.enemies_in_target_order(p, 30.0), [focus, near]);
        let e = r.enemy(focus).unwrap();
        assert!(e.ranks_as_elite() && !e.elite);
        assert_eq!(e.max_health, health);
    }

    /// Stratholme's Black Guard Sentry, as a boss brings it, is picked as an elite before a
    /// nearer ordinary enemy.
    #[test]
    fn a_black_guard_sentry_is_picked_as_an_elite() {
        let content = content();
        let hero = HeroSetup { class: "mage".into(), race: "human".into(), start: Some("fireball".into()), ..Default::default() };
        let mut r = Run::new(content.clone(), "strat", RunOptions::default(), &[hero], Box::new(Flat), 11).unwrap();
        r.enemies.clear();
        let zone = content.zone("strat").unwrap();
        let sentry = zone.enemies.iter().position(|e| e.name == "Black Guard Sentry").unwrap();
        let ghoul = zone.enemies.iter().position(|e| e.name == "Plague Ghoul").unwrap();
        let p = r.heroes[0].pos;
        assert!(r.spawn_roster_enemy_with(ghoul, Some((p.0 + 3.0, p.1)), true));
        let near = r.enemies.last().unwrap().id;
        assert!(r.spawn_roster_enemy_with(sentry, Some((p.0 + 15.0, p.1)), true));
        let focus = r.enemies.last().unwrap().id;
        assert_eq!(r.enemies_in_target_order(p, 30.0), [focus, near]);
        let e = r.enemy(focus).unwrap();
        assert!(e.ranks_as_elite() && !e.elite);
    }

    /// Design step 4's test: every class and race spell, cast with enemies around, shows its own
    /// look (effects over time too).
    #[test]
    fn every_spell_shows_its_look_when_cast() {
        let c = content();
        let mut silent = Vec::new();
        let spells: Vec<&Ability> = c.abilities.iter().filter(|a| c.classes.iter().any(|k| k.abilities.contains(&a.id) || k.starting_options.iter().any(|o| o.spells.contains(&a.id))) || c.races.iter().any(|r| r.spell == a.id)).collect();
        assert!(spells.len() > 50, "{}", spells.len());
        // spells that are always on, or need a poisoned enemy or a pet, show theirs in game::kit's tests
        let own = |a: &Ability| matches!(a.kit.mechanic, Some(esurv_content::Mechanic::SwingProc { .. } | esurv_content::Mechanic::ExtraSwingHits { .. } | esurv_content::Mechanic::StrikesAfflicted { .. } | esurv_content::Mechanic::PetsStrike));
        for a in spells.into_iter().filter(|a| !own(a)) {
            let mut r = run("mage");
            for k in 0..6 {
                let angle = k as f32;
                enemy_at(&mut r, (4.0 * angle.cos(), 4.0 * angle.sin()));
            }
            r.heroes[0].health = r.heroes[0].stats.max_health * 0.3;
            r.drain_events();
            let inst = super::super::spells::SpellSlot::new(a, &[]).instance();
            r.cast(0, a, &inst);
            // its own cast, never a triggered one: the hero animates casting it
            let shown = r.drain_events().iter().any(|e| matches!(e, Event::SpellGo { spell, triggered: false, .. } if *spell == a.look_spell));
            if !shown {
                silent.push(a.id.clone());
            }
        }
        assert!(silent.is_empty(), "no look: {silent:?}");
    }

    #[test]
    fn a_mage_fireball_kills_and_drops_experience() {
        let mut r = run("mage");
        let id = enemy_at(&mut r, (10.0, 0.0));
        for _ in 0..200 {
            r.tick(0.05);
            if r.enemy(id).is_none() {
                break;
            }
        }
        assert!(r.enemy(id).is_none(), "the fireball should kill a kobold");
        let ev = r.drain_events();
        assert!(ev.iter().any(|e| matches!(e, Event::SpellCast { spell, .. } if spell == "fireball")));
        assert!(ev.iter().any(|e| matches!(e, Event::EnemyDied { .. })));
        assert_eq!(r.heroes[0].kills, 1);
    }

    /// `hammer_of_wrath` doubles on an enemy under 30 % health, checked as the hammer lands; a spell
    /// without the switch does not.
    #[test]
    fn a_missile_that_doubles_on_low_health_checks_the_enemy_as_it_lands() {
        let mut r = run("paladin");
        let mut lost = Vec::new();
        for (spell, share) in [("hammer_of_wrath", 0.5), ("hammer_of_wrath", 0.2), ("judgement", 0.2)] {
            r.enemies.clear();
            let id = enemy_at(&mut r, (40.0, 0.0));
            let e = r.enemies.iter_mut().find(|e| e.id == id).unwrap();
            e.max_health = 10_000.0;
            e.health = 10_000.0 * share;
            let before = e.health;
            let at = r.now;
            let (dot, pet, cast) = (None, None, None);
            let (damage, crit, from_proc) = (10.0, false, false);
            r.hits.push(PendingHit { hero: 0, target: id, at, damage, crit, spell: spell.into(), dot, pet, from_proc, cast, burn: None });
            r.tick(0.01);
            lost.push(before - r.enemy(id).unwrap().health);
        }
        assert!((lost[1] / lost[0] - 2.0).abs() < 1e-3, "{lost:?}");
        assert!((lost[2] - lost[0]).abs() < 1e-3, "{lost:?}");
    }

    #[test]
    fn orbs_in_pickup_range_fly_in_and_level_the_hero() {
        let mut r = run("warrior");
        let p = r.heroes[0].pos;
        r.drop_orb((p.0 + 5.0, p.1), 200);
        r.drop_orb((p.0 + 50.0, p.1), 200);
        for _ in 0..20 {
            r.tick(0.05);
        }
        assert_eq!(r.heroes[0].progress.level, 2);
        assert_eq!(r.heroes[0].pending_choices, 1);
        assert_eq!(r.pickups.iter().filter(|x| x.kind == Pickup::Experience).count(), 1, "the far orb stays");
    }

    #[test]
    fn only_normal_kills_feed_the_kill_pressure() {
        let mut r = run("warrior");
        let roster = &r.content.zones[r.zone].enemies;
        let normal = roster.iter().position(|e| !e.elite).unwrap();
        let elite = roster.iter().position(|e| e.elite).unwrap();
        let cost = roster[normal].spawn_cost;
        let p = r.heroes[0].pos;
        assert!(r.spawn_roster_enemy(normal, Some((p.0 + 5.0, p.1))));
        let id = r.enemies.last().unwrap().id;
        r.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Other);
        assert!((r.director.pressure() - cost).abs() < 1e-5);
        assert!(r.spawn_roster_enemy(elite, Some((p.0 + 5.0, p.1))));
        let id = r.enemies.last().unwrap().id;
        r.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Other);
        r.spawn_boss(0);
        let id = r.enemies.last().unwrap().id;
        r.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Other);
        assert!((r.director.pressure() - cost).abs() < 1e-5, "elites and bosses add nothing");
    }

    #[test]
    fn holds_shrink_on_bosses_chain_at_most_three_seconds_and_leave_immunity() {
        let mut r = run("warrior");
        let id = enemy_at(&mut r, (5.0, 0.0));
        r.apply_on_hit(0, id, &SpellEffect::Stun { seconds: 2.0 }, 1.0);
        assert!((r.enemy(id).unwrap().hold_until - (r.now + 2.0)).abs() < 1e-5);
        r.apply_on_hit(0, id, &SpellEffect::Stun { seconds: 5.0 }, 1.0);
        assert!((r.enemy(id).unwrap().hold_until - (r.now + 3.0)).abs() < 1e-5, "a chain stops at 3 s");
        r.now += 3.0;
        r.effects_tick();
        r.apply_on_hit(0, id, &SpellEffect::Root { seconds: 2.0 }, 1.0);
        assert_eq!(r.enemy(id).unwrap().hold_until, 0.0, "hold-immune for 2 s");
        r.spawn_boss(0);
        let boss = r.enemies.last().unwrap().id;
        // a boss arrives held; once that hold and its immunity pass, holds shrink to a quarter
        r.now += crate::game::run::BOSS_ARRIVAL_HOLD;
        r.effects_tick();
        r.now += HOLD_IMMUNITY;
        r.apply_on_hit(0, boss, &SpellEffect::Stun { seconds: 2.0 }, 1.0);
        assert!((r.enemy(boss).unwrap().hold_until - (r.now + 0.5)).abs() < 1e-5);
    }

    #[test]
    fn per_target_bonuses_and_the_minimum_hit() {
        let mut r = run("warrior");
        let id = enemy_at(&mut r, (5.0, 0.0));
        r.heroes[0].modifiers.push(Modifier::new(Stat::DamageVsCrowdControlled, 0.5));
        assert_eq!(r.hit_enemy(Some(0), id, 0.2, false, HitSource::Other), Some(1.0));
        let hp = r.enemy(id).unwrap().health;
        r.apply_on_hit(0, id, &SpellEffect::Slow { amount: 0.5, seconds: 5.0 }, 0.0);
        let d = r.hit_enemy(Some(0), id, 2.0, false, HitSource::Other).unwrap();
        assert_eq!(d, 3.0);
        assert_eq!(r.enemy(id).unwrap().health, hp - 3.0);
    }

    #[test]
    fn a_bouncing_chain_jumps_to_the_nearest_enemy_it_has_not_struck() {
        let mut r = run("shaman");
        let content = r.content.clone();
        let chain = content.ability("chain_lightning").unwrap();
        // spawned far first: the first strike still goes to the nearest
        let far = enemy_at(&mut r, (10.0, 0.0));
        let near = enemy_at(&mut r, (5.0, 0.0));
        let side = enemy_at(&mut r, (5.0, 3.0));
        let now = r.now;
        r.enemies.iter_mut().for_each(|e| {
            e.health = 1e9;
            let at = e.position(now);
            e.segment = crate::game::enemy_ai::Segment::standing(at, now);
        });
        assert!(r.cast(0, chain, &Instance::default()));
        let targets: Vec<u32> = r.hits.iter().map(|h| h.target).collect();
        // nearest to the hero, then the nearest to the last one struck it has not struck; with
        // nobody new in reach the chain ends early
        assert_eq!(targets, [near, side, far]);
        let d: Vec<f32> = r.hits.iter().map(|h| h.damage).collect();
        if !r.hits[0].crit && !r.hits[1].crit {
            assert!((d[1] / d[0] - BOUNCE_FALLOFF).abs() < 1e-4);
        }
        assert!((r.hits[1].at - r.hits[0].at - BOUNCE_STEP).abs() < 1e-5);
        // alone, one enemy takes one strike
        r.hits.clear();
        r.enemies.retain(|e| e.id == near);
        assert!(r.cast(0, chain, &Instance::default()));
        assert_eq!(r.hits.len(), 1);
    }

    #[test]
    fn the_swing_takes_the_nearest_enemy_in_reach() {
        let mut r = run("warrior");
        r.heroes[0].spells.known.clear();
        r.heroes[0].spells.racial = None;
        // spawned first but farther: passed over
        let farther = enemy_at(&mut r, (4.0, 0.0));
        let nearer = enemy_at(&mut r, (2.0, 0.0));
        let now = r.now;
        r.enemies.iter_mut().for_each(|e| {
            e.health = 1e9;
            let at = e.position(now);
            e.segment = crate::game::enemy_ai::Segment::standing(at, now);
        });
        r.tick(0.05);
        let struck: Vec<u32> = r.drain_events().iter().filter_map(|e| if let Event::EnemyHit { id, source: HitSource::WeaponSwing, .. } = e { Some(*id) } else { None }).collect();
        assert_eq!(struck, [nearer], "not {farther}");
    }

    #[test]
    fn a_lasting_area_on_the_hero_needs_an_enemy_inside_it() {
        let mut r = run("paladin");
        let content = r.content.clone();
        let consecration = content.ability("consecration").unwrap();
        assert_eq!(consecration.range, 0.0, "on the hero");
        let inst = Instance::default();
        let reach = radius(consecration, &inst, r.heroes[0].stats.area);
        enemy_at(&mut r, (reach + 5.0, 0.0));
        let now = r.now;
        r.enemies.iter_mut().for_each(|e| {
            let at = e.position(now);
            e.segment = crate::game::enemy_ai::Segment::standing(at, now);
        });
        assert!(!r.cast(0, consecration, &inst), "nobody inside: no area");
        enemy_at(&mut r, (reach - 1.0, 0.0));
        let now = r.now;
        r.enemies.iter_mut().for_each(|e| {
            let at = e.position(now);
            e.segment = crate::game::enemy_ai::Segment::standing(at, now);
        });
        assert!(r.cast(0, consecration, &inst));
    }

    #[test]
    fn a_warrior_swings_at_the_first_enemy_in_reach() {
        let mut r = run("warrior");
        r.heroes[0].spells.known.clear();
        r.heroes[0].spells.racial = None;
        let id = enemy_at(&mut r, (3.0, 0.0));
        r.enemies[0].health = 1e9;
        r.heroes[0].stats.crit_chance = 0.0;
        r.tick(0.05);
        let swing = super::super::kit::swing_of(&r.content, &r.heroes[0]).unwrap();
        let want = swing.damage * 1.5 * r.heroes[0].stats.damage;
        let hits: Vec<f32> = r.drain_events().iter().filter_map(|e| if let Event::EnemyHit { id: x, amount, .. } = e { (*x == id).then_some(*amount) } else { None }).collect();
        assert_eq!(hits.len(), 1);
        assert!((hits[0] - want).abs() < 1e-3);
        assert!((r.heroes[0].swing_timer - r.heroes[0].stats.swing_factor.sqrt() * swing.swing).abs() < 1e-5);
    }

    /// The cooldown recovery of talents, rings and trinkets counts half
    /// for spell cooldowns and in full for the swing pace, which stays what it was.
    #[test]
    fn talent_ring_and_trinket_cooldown_recovery_counts_half_for_spells_and_keeps_the_swing_pace() {
        use crate::game::stats::{compute, Modifier};
        let mut r = run("warrior");
        let (cooldown, swing) = (r.heroes[0].stats.cooldown_factor, r.heroes[0].stats.swing_factor);
        assert_eq!(cooldown, swing, "nothing yet but full-strength sources");
        let ring = r.content.jewelry.iter().find(|j| j.id == "quick_strike").unwrap().clone();
        assert_eq!((ring.stat, ring.value), (Stat::SpellCooldownRecovery, 0.05), "half the 10 % it gave");
        r.heroes[0].jewelry.push(crate::game::choices::WornJewelry { id: ring.id.clone(), value: ring.value, rank: 1 });
        r.rebuild_hero_modifiers(0);
        let s = r.heroes[0].stats;
        assert!((1.0 / s.cooldown_factor - 1.0 / cooldown - 0.05).abs() < 1e-5, "spells: 5 %");
        assert!((1.0 / s.swing_factor - 1.0 / swing - 0.10).abs() < 1e-5, "swings: the full 10 % as before");
        // a talent: five ranks of 0.5 % for spells, the swing still as from the 10 %
        // it gave before (its hidden swing-only part keeps the pace)
        let content = r.content.clone();
        let talent = content
            .talent_trees
            .iter()
            .filter(|t| t.class == "warrior")
            .flat_map(|t| &t.talents)
            .find(|t| t.effects.iter().any(|e| matches!(e, esurv_content::TalentEffect::Stat { stat: Stat::SpellCooldownRecovery, .. })))
            .unwrap();
        let setup = HeroSetup::with_progress(&content, "warrior", "human", [(talent.id.as_str(), 5)], &[]);
        let class = content.classes.iter().find(|c| c.id == "warrior").unwrap();
        let s = compute(class, 1, &setup.modifiers);
        assert!((s.cooldown_factor - 1.0 / 1.025).abs() < 1e-5);
        assert!((s.swing_factor - 1.0 / 1.10).abs() < 1e-5);
        // other sources (spell cards, amulets, prestige, loadouts, buffs) stay full for both
        let s = compute(class, 1, &[Modifier::new(Stat::CooldownRecovery, 0.1)]);
        assert!((s.cooldown_factor - 1.0 / 1.1).abs() < 1e-5 && (s.swing_factor - 1.0 / 1.1).abs() < 1e-5);
        // and no talent, ring or trinket gives cooldown recovery at full strength any more
        for t in content.talent_trees.iter().flat_map(|t| &t.talents) {
            for e in &t.effects {
                let full = match e {
                    esurv_content::TalentEffect::Stat { stat, .. } => *stat == Stat::CooldownRecovery,
                    esurv_content::TalentEffect::Milestone(esurv_content::Milestone::TimedBuff { stat, .. }) => *stat == Stat::CooldownRecovery,
                    _ => false,
                };
                assert!(!full, "{}", t.id);
            }
        }
        assert!(content.jewelry.iter().all(|j| j.stat != Stat::CooldownRecovery));
    }

    /// Hand of Justice's chance for a weapon swing to hit twice; the
    /// second is a full weapon hit of its own (own crit roll, what a swing sets off).
    #[test]
    fn a_swing_hits_twice_at_the_double_hit_chance() {
        let mut r = run("warrior");
        r.heroes[0].spells.known.clear();
        r.heroes[0].spells.racial = None;
        let id = enemy_at(&mut r, (3.0, 0.0));
        r.enemies[0].health = 1e9;
        r.heroes[0].stats.crit_chance = 0.0;
        let content = r.content.clone();
        let swing = super::super::kit::swing_of(&content, &r.heroes[0]).unwrap();
        let swings = |r: &mut Run, n: usize| -> Vec<f32> {
            let mut hits = Vec::new();
            for _ in 0..n {
                r.heroes[0].swing_timer = 0.0;
                r.swing_hand(0, 0, &swing, 0.0);
                hits.extend(r.drain_events().iter().filter_map(|e| match e {
                    Event::EnemyHit { id: x, amount, source: HitSource::WeaponSwing, .. } if *x == id => Some(*amount),
                    _ => None,
                }));
            }
            hits
        };
        r.heroes[0].stats.double_hit = 1.0;
        let two = swings(&mut r, 1);
        assert_eq!(two.len(), 2, "always twice");
        assert!((two[0] - two[1]).abs() < 1e-3, "the second hit is a full one: {two:?}");
        r.heroes[0].stats.double_hit = 0.0;
        assert_eq!(swings(&mut r, 1).len(), 1);
        r.heroes[0].stats.double_hit = 0.3;
        let extra = swings(&mut r, 4000).len() - 4000;
        assert!((extra as f32 / 4000.0 - 0.3).abs() < 0.03, "{extra} second hits in 4000 swings");
    }

    #[test]
    fn double_cast_casts_again() {
        let mut r = run("mage");
        enemy_at(&mut r, (10.0, 0.0));
        r.enemies[0].health = 1e9;
        r.heroes[0].spells.racial = None;
        let mut casts_with = |d: f32| {
            r.heroes[0].stats.double_cast = d;
            r.heroes[0].spells.known[0].timer = 0.0;
            r.drain_events();
            r.tick(0.01);
            r.drain_events().iter().filter(|e| matches!(e, Event::SpellCast { .. })).count()
        };
        assert_eq!(casts_with(1.0), 2, "100 %: always a double");
        assert_eq!(casts_with(2.0), 3, "200 %: always a triple");
        assert_eq!(casts_with(4.0), 5, "every 100 % is one more cast");
    }

    #[test]
    fn a_boss_rolls_its_gear_when_it_dies() {
        let mut r = run("warrior");
        let mut won = 0;
        let mut rolls = 0;
        for _ in 0..200 {
            r.spawn_boss(0);
            let id = r.enemies.last().unwrap().id;
            r.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Other);
            rolls += 2; // Elwynn's first boss owns armor slots 0 and 4
            for e in r.drain_events() {
                if let Event::GearDropped { slot, tier, entry, .. } = e {
                    assert!(slot == 0 || slot == 4);
                    assert_eq!((tier, entry), (0, gear_item_entry(1, slot, 0)));
                    won += 1;
                }
            }
        }
        let rate = won as f32 / rolls as f32;
        assert_eq!(rate, 1.0, "Elwynn drops every time");
        assert_eq!(r.heroes[0].loot.len(), won);
    }

    #[test]
    fn the_final_boss_drops_weapons() {
        let mut r = run("warrior");
        r.options.speed_pct = 150; // bonus 2: every roll wins
        r.spawn_boss(4);
        let id = r.enemies.last().unwrap().id;
        r.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Other);
        let slots: Vec<u32> = r.heroes[0].loot.iter().map(|d| d.slot).collect();
        assert!(!slots.is_empty() && slots.iter().all(|&s| s >= 8), "weapon slots only: {slots:?}");
    }

    #[test]
    fn totems_pulse_after_half_a_second_then_every_two_and_longer_ones_pulse_more() {
        let pulses = |duration_mult: f32| {
            let mut r = run("shaman");
            enemy_at(&mut r, (10.0, 0.0));
            r.enemies.iter_mut().for_each(|e| e.health = 1e9);
            let content = r.content.clone();
            let totem = content.ability("magma_totem").unwrap();
            let inst = Instance { duration_from_cards: duration_mult, ..Instance::default() };
            r.heroes[0].spells.known.clear();
            r.heroes[0].spells.racial = None;
            assert!(r.cast(0, totem, &inst));
            let placed = r.now;
            let mut times = Vec::new();
            for _ in 0..800 {
                r.enemies.iter_mut().for_each(|e| e.health = 1e9);
                r.tick(0.05);
                if r.drain_events().iter().any(|e| matches!(e, Event::EnemyHit { by: Some(_), source: HitSource::Spell(s), .. } if s == "magma_totem")) {
                    times.push(r.now - placed);
                }
                r.heroes[0].health = r.heroes[0].stats.max_health;
            }
            times
        };
        let short = pulses(1.0);
        assert!((short[0] - 0.5).abs() < 0.051, "first pulse after 0.5 s ({})", short[0]);
        assert!((short[1] - short[0] - 2.0).abs() < 0.051, "then every 2 s");
        // 8 s: pulses at 0.5, 2.5, 4.5, 6.5
        assert_eq!(short.len(), 4);
        assert_eq!(pulses(2.0).len(), 8, "twice the duration, twice the pulses");
    }

    /// The first pulse of a totem `spell`, with enemies at `offsets`: what it showed.
    fn first_pulse(spell: &str, offsets: &[Vec2]) -> (Run, Vec<Event>) {
        let mut r = run("shaman");
        for &o in offsets {
            enemy_at(&mut r, o);
        }
        r.enemies.iter_mut().for_each(|e| e.health = 1e9);
        let content = r.content.clone();
        r.heroes[0].spells.known.clear();
        r.heroes[0].spells.racial = None;
        assert!(r.cast(0, content.ability(spell).unwrap(), &Instance::default()));
        r.drain_events();
        for _ in 0..12 {
            r.tick(0.05);
            let events = r.drain_events();
            if events.iter().any(|e| matches!(e, Event::SpellGo { caster: Unit::Totem(_), .. })) {
                return (r, events);
            }
        }
        panic!("{spell} never pulsed");
    }

    #[test]
    fn a_magma_totem_shows_each_pulse_around_itself() {
        let (r, events) = first_pulse("magma_totem", &[(4.0, 0.0), (40.0, 0.0)]);
        let near = r.enemies[0].id;
        let totem = r.totems[0].id;
        let pulses: Vec<&Event> = events.iter().filter(|e| matches!(e, Event::SpellGo { .. })).collect();
        assert_eq!(pulses.len(), 1, "one look a pulse");
        assert!(matches!(pulses[0], Event::SpellGo { caster: Unit::Totem(t), spell: 8187, hits, aim: Aim::Caster, .. } if *t == totem && *hits == vec![Unit::Enemy(near)]));
        // the fire wave covers the totem's true radius
        let magma = r.content.ability("magma_totem").unwrap();
        let true_radius = radius(magma, &Instance::default(), r.heroes[0].stats.area);
        assert!(matches!(pulses[0], Event::SpellGo { reach, .. } if (*reach - true_radius).abs() < 1e-4 && *reach > 0.0));
        // a pulse with no one in reach still shows
        let (_, events) = first_pulse("magma_totem", &[(18.0, 0.0)]);
        assert!(events.iter().any(|e| matches!(e, Event::SpellGo { caster: Unit::Totem(_), spell: 8187, hits, .. } if hits.is_empty())));
    }

    #[test]
    fn a_searing_totem_shoots_one_enemy_in_range() {
        let (r, events) = first_pulse("searing_totem", &[(6.0, 0.0), (8.0, 0.0)]);
        let shots: Vec<Unit> = events
            .iter()
            .filter_map(|e| match e {
                Event::SpellGo { caster: Unit::Totem(_), spell: 3606, hits, aim: Aim::Unit(t), .. } if *hits == vec![*t] => Some(*t),
                _ => None,
            })
            .collect();
        assert_eq!(shots.len(), 1);
        assert!(r.enemies.iter().any(|e| Unit::Enemy(e.id) == shots[0]));
    }

    #[test]
    fn a_totem_a_milestone_casts_stands_to_the_side() {
        let place = |depth: u32| {
            let mut r = run("shaman");
            enemy_at(&mut r, (4.0, 0.0));
            let content = r.content.clone();
            let p = r.heroes[0].pos;
            r.heroes[0].facing = 0.0;
            r.proc_depth = depth;
            assert!(r.cast(0, content.ability("magma_totem").unwrap(), &Instance::default()));
            let at = r.totems[0].at;
            (at.0 - p.0, at.1 - p.1)
        };
        let ahead = place(0);
        assert!((ahead.0 - TOTEM_OFFSET).abs() < 1e-4 && ahead.1.abs() < 1e-4, "{ahead:?}");
        let side = place(1);
        assert!(side.0.abs() < 1e-4 && (side.1 - TOTEM_OFFSET).abs() < 1e-4, "{side:?}");
    }

    #[test]
    fn a_lasting_area_shows_on_the_ground_until_just_after_its_time_is_up() {
        let mut r = run("mage");
        enemy_at(&mut r, (6.0, 0.0));
        r.enemies.iter_mut().for_each(|e| e.health = 1e9);
        let content = r.content.clone();
        let blizzard = content.ability("blizzard").unwrap();
        r.drain_events();
        let start = r.now;
        assert!(r.cast(0, blizzard, &Instance::default()));
        let events = r.drain_events();
        let (id, until) = events
            .iter()
            .find_map(|e| match e {
                Event::AreaShown { id, caster: Unit::Hero(_), spell, until: Some(u), .. } if *spell == blizzard.look_spell => Some((*id, *u)),
                _ => None,
            })
            .expect("its look on the ground");
        let radius_now = radius(blizzard, &Instance::default(), r.heroes[0].stats.area);
        assert!(events.iter().any(|e| matches!(e, Event::SpellGo { aim: Aim::Ground(_), reach, .. } if (*reach - radius_now).abs() < 1e-4)), "the cast covers the area");
        assert!((until - start - duration(blizzard, &Instance::default()) - AREA_LOOK_AFTER).abs() < 1e-4);
        assert!(r.areas.iter().any(|a| a.id == id), "the area keeps its look's id");
    }

    #[test]
    fn effects_over_time_renew_a_full_target_only_when_it_runs_out_before_the_next_cast() {
        let mut r = run("warlock");
        r.spawn_boss(0);
        let boss = r.enemies.last().unwrap().id;
        let normal = enemy_at(&mut r, (6.0, 0.0));
        let p = r.heroes[0].pos;
        let now = r.now;
        r.enemies.iter_mut().for_each(|e| {
            e.health = 1e9;
            e.segment = crate::game::enemy_ai::Segment::standing((p.0 + 6.0, p.1), now);
        });
        let content = r.content.clone();
        let corruption = content.ability("corruption").unwrap();
        let inst = Instance::default();
        let hit = hit_damage(corruption, &inst, 1.0);
        // the boss comes first; with no extra stacks one stack fills it
        assert!(r.apply_dots(0, corruption, &inst, hit, 1));
        r.effects_tick();
        assert_eq!(r.dot_stacks(boss, "corruption"), 1);
        // full and lasting longer than the next cast: the next enemy in line gets it
        assert!(r.apply_dots(0, corruption, &inst, hit, 1));
        r.effects_tick();
        assert_eq!(r.dot_stacks(normal, "corruption"), 1);
        // the boss's stack runs low: it is renewed, the normal enemy (full, lasting) is passed over
        let next_cast = crate::game::spells::cooldown(corruption, &inst, r.heroes[0].stats.cooldown_factor);
        r.enemies.iter_mut().filter(|e| e.id == boss).for_each(|e| e.dots[0].ticks_left = 1);
        assert!(r.dot_time_left(boss, "corruption") < next_cast);
        assert!(r.apply_dots(0, corruption, &inst, hit, 1));
        r.effects_tick();
        assert!(r.dot_time_left(boss, "corruption") > next_cast, "the boss's effect was renewed");
        assert_eq!(r.dot_stacks(boss, "corruption"), 1);
        // everyone full and lasting: nothing to do
        assert!(!r.apply_dots(0, corruption, &inst, hit, 1));
    }

    /// A renewed stack keeps the ticks it had left, on top of the new ones, and its next tick
    /// stays when it was due.
    #[test]
    fn a_renewed_effect_over_time_keeps_its_ticks_left() {
        let mut r = run("warlock");
        let id = enemy_at(&mut r, (6.0, 0.0));
        r.enemies.iter_mut().for_each(|e| e.health = 1e9);
        let dot = |ticks_left: u32, next: f32| Dot {
            hero: 0,
            spell: "corruption".into(),
            per_tick: 10.0,
            ticks_left,
            interval: 1.0,
            next,
            from_proc: false,
            leech: 0.0,
        };
        let now = r.now;
        r.add_dot(id, dot(1, now + 0.2));
        r.add_dot(id, dot(8, now + 1.0));
        let e = r.enemy(id).unwrap();
        assert_eq!(e.dots.len(), 1);
        assert_eq!(e.dots[0].ticks_left, 9);
        assert!((e.dots[0].next - (now + 0.2)).abs() < 1e-6);
        // a renewal with bigger ticks (a crit) keeps the damage both had left, no more:
        // 9 ticks of 10 left plus 8 new ticks of 20 = 250 over 17 ticks
        let mut big = dot(8, now + 1.0);
        big.per_tick = 20.0;
        r.add_dot(id, big);
        let d = &r.enemy(id).unwrap().dots[0];
        assert_eq!(d.ticks_left, 17);
        assert!((d.per_tick * 17.0 - 250.0).abs() < 1e-3, "{}", d.per_tick);
    }

    #[test]
    fn an_effect_over_time_heals_from_each_tick_and_lands_its_other_effect_when_applied() {
        let mut r = run("warlock");
        let target = enemy_at(&mut r, (6.0, 0.0));
        let p = r.heroes[0].pos;
        let now = r.now;
        r.enemies.iter_mut().for_each(|e| {
            e.health = 1e9;
            e.segment = crate::game::enemy_ai::Segment::standing((p.0 + 6.0, p.1), now);
        });
        let content = r.content.clone();
        let inst = Instance::default();
        // healing from damage: nothing when applied, the tick's damage × the share on each tick
        let drain = content.ability("drain_life").unwrap();
        let SpellEffect::HealFromDamage { share } = drain.effect else { panic!("drain life heals from damage") };
        let hit = hit_damage(drain, &inst, 1.0);
        let (_, per_tick) = ticks(drain, &inst, hit);
        // no lifesteal of the class's own, so only the tick's leech heals
        r.heroes[0].stats.lifesteal = 0.0;
        r.heroes[0].health = 1.0;
        assert!(r.apply_dots(0, drain, &inst, hit, 1));
        r.effects_tick();
        assert_eq!(r.dot_stacks(target, "drain_life"), 1);
        assert_eq!(r.heroes[0].health, 1.0, "no heal as it is applied");
        let interval = drain.duration / drain.ticks as f32;
        r.now += interval;
        r.effects_tick();
        assert!((r.heroes[0].health - (1.0 + per_tick * share)).abs() < 1e-3, "one tick's heal: {}", r.heroes[0].health);
        // any other effect lands once, as the effect is applied: `insect_swarm` weakens
        let swarm = content.ability("insect_swarm").unwrap();
        let SpellEffect::Weaken { amount, seconds } = swarm.effect else { panic!("insect swarm weakens") };
        let hit = hit_damage(swarm, &inst, 1.0);
        assert!(r.apply_dots(0, swarm, &inst, hit, 1));
        r.effects_tick();
        let weaken = r.enemy(target).and_then(|e| e.weaken);
        assert_eq!(weaken, Some((amount, r.now + seconds)));
    }

    #[test]
    fn bosses_come_first_one_stack_per_cast() {
        let mut r = run("warlock");
        r.spawn_boss(0);
        let boss = r.enemies.last().unwrap().id;
        let normal = enemy_at(&mut r, (6.0, 0.0));
        let p = r.heroes[0].pos;
        let now = r.now;
        r.enemies.iter_mut().for_each(|e| {
            e.health = 1e9;
            e.segment = crate::game::enemy_ai::Segment::standing((p.0 + 6.0, p.1), now);
        });
        // three extra stacks: everyone holds 4
        r.heroes[0].modifiers.push(Modifier::new(Stat::DotStacks, 3.0));
        r.refresh_stats(0);
        let content = r.content.clone();
        let corruption = content.ability("corruption").unwrap();
        let inst = Instance::default();
        let hit = hit_damage(corruption, &inst, 1.0);
        let both = |r: &Run| (r.dot_stacks(boss, "corruption"), r.dot_stacks(normal, "corruption"));
        // one target per cast: the boss fills up first, one stack a cast
        for want in 1..=4 {
            assert!(r.apply_dots(0, corruption, &inst, hit, 1));
            r.effects_tick();
            assert_eq!(both(&r), (want, 0));
        }
        assert!(r.apply_dots(0, corruption, &inst, hit, 1));
        r.effects_tick();
        assert_eq!(both(&r), (4, 1), "full: the next in line");
        // three targets per cast: still one on each, the rest go down the line
        r.enemies.iter_mut().for_each(|e| e.dots.clear());
        assert!(r.apply_dots(0, corruption, &inst, hit, 3));
        r.effects_tick();
        assert_eq!(both(&r), (1, 1));
    }

    #[test]
    fn normal_enemies_get_one_per_cast_healthiest_first_and_still_stack() {
        let mut r = run("warlock");
        let weak = enemy_at(&mut r, (6.0, 0.0));
        let strong = enemy_at(&mut r, (7.0, 0.0));
        let middle = enemy_at(&mut r, (8.0, 0.0));
        let now = r.now;
        let p = r.heroes[0].pos;
        for (id, hp) in [(weak, 100.0), (strong, 300.0), (middle, 200.0)] {
            let e = r.enemies.iter_mut().find(|e| e.id == id).unwrap();
            e.health = hp;
            e.max_health = 1e9;
            e.segment = crate::game::enemy_ai::Segment::standing((p.0 + 6.0, p.1), now);
        }
        // one extra stack: everyone holds 2
        r.heroes[0].modifiers.push(Modifier::new(Stat::DotStacks, 1.0));
        r.refresh_stats(0);
        let content = r.content.clone();
        let corruption = content.ability("corruption").unwrap();
        let inst = Instance::default();
        let stacks = |r: &Run| [weak, strong, middle].map(|id| r.dot_stacks(id, "corruption"));
        let cast = |r: &mut Run| {
            // tiny hits so nobody dies
            assert!(r.apply_dots(0, corruption, &inst, 0.001, 2));
            r.effects_tick();
        };
        cast(&mut r);
        assert_eq!(stacks(&r), [0, 1, 1], "the two healthiest, one each");
        cast(&mut r);
        assert_eq!(stacks(&r), [0, 2, 2], "they keep stacking");
        cast(&mut r);
        assert_eq!(stacks(&r), [1, 2, 2], "full and lasting: the next healthiest");
        // the strongest one's stacks run low: renewed, not added to
        r.enemies.iter_mut().filter(|e| e.id == strong).for_each(|e| e.dots.iter_mut().for_each(|d| d.ticks_left = 1));
        cast(&mut r);
        assert_eq!(stacks(&r), [2, 2, 2]);
        let next_cast = crate::game::spells::cooldown(corruption, &inst, r.heroes[0].stats.cooldown_factor);
        assert!(r.dot_times_left(strong, "corruption").iter().any(|&t| t > next_cast), "one renewed");
    }

    #[test]
    fn a_hero_who_keeps_moving_fights_through_the_first_minutes() {
        let mut r = run("mage");
        let id = r.heroes[0].id;
        let (cx, cy) = r.heroes[0].pos;
        let mut kills = 0;
        let mut angle = 0.0f32;
        while r.phase == Phase::Running && r.now < 120.0 {
            // walk a circle of 20 yd at run speed, facing the way it walks
            let speed = r.heroes[0].stats.run_speed();
            angle += speed * 0.05 / 20.0;
            let pos = (cx + 20.0 * angle.cos(), cy + 20.0 * angle.sin());
            r.hero_moved(id, pos, 0.0, angle + std::f32::consts::FRAC_PI_2);
            r.tick(0.05);
            r.take_first_offers();
            kills += r.drain_events().iter().filter(|e| matches!(e, Event::EnemyDied { .. })).count();
        }
        assert!(kills > 20, "a moving mage kills plenty in two minutes ({kills}, {:?} at {:.0}s)", r.phase, r.now);
        assert!(r.heroes[0].progress.level > 1, "and picks up experience on the way");
    }

    /// The race and class pairs `races.ron` adds each play three minutes
    /// of a run with the class's and the race's numbers unchanged: the class's spells cast and kill,
    /// its pet comes (a human hunter's wolf, an orc mage's water elemental from its Frostbolt
    /// starter, a troll warlock's imp), a dwarf shaman's totems go down, an undead paladin's
    /// blessing casts itself.
    #[test]
    fn every_added_race_and_class_pair_plays_a_run() {
        let c = content();
        let pairs = c.added_pairs();
        assert_eq!(pairs.len(), 6);
        for pair in pairs {
            let race = c.race_by_wow_id(pair.race).unwrap();
            let class = c.classes.iter().find(|k| k.wow_class_id == u32::from(pair.class)).unwrap();
            // the class's pet or blessing talent, bought as the armory would
            let talent = match class.id.as_str() {
                "hunter" => Some("tame_beast"),
                "warlock" => Some("summon_imp"),
                "paladin" => Some("sanctuary"),
                _ => None,
            };
            // the mage's water elemental is its Frostbolt starter's passive
            let start = (class.id == "mage").then_some("frostbolt");
            let start_spell = start.unwrap_or(class.default_start_spell());
            let mut setup = HeroSetup::with_progress(&c, &class.id, &race.id, talent.map(|t| (t, 1)), &[]);
            setup.start = start.map(str::to_string);
            let mut r = Run::new(c.clone(), "elwynn", RunOptions::default(), &[setup], Box::new(Flat), 23).unwrap();
            // the race's own spell, the class's own numbers
            assert_eq!(r.heroes[0].spells.racial.as_ref().map(|s| s.spell.as_str()), Some(race.spell.as_str()));
            assert_eq!(r.heroes[0].spells.known[0].spell, start_spell);
            if class.id == "shaman" {
                // a totem from the class's pool, as a level-up card would teach it
                let totem = c.ability("searing_totem").unwrap();
                r.heroes[0].spells.known.push(super::super::spells::SpellSlot::new(totem, &[]));
            }
            let id = r.heroes[0].id;
            let (cx, cy) = r.heroes[0].pos;
            r.hero_moved(id, (cx + 1.0, cy), 0.0, 0.0);
            let (mut kills, mut casts, mut pets, mut totems) = (0, Vec::new(), Vec::new(), 0);
            let mut angle = 0.0f32;
            while r.phase == Phase::Running && r.now < 180.0 {
                let speed = r.heroes[0].stats.run_speed();
                angle += speed * 0.05 / 20.0;
                let pos = (cx + 20.0 * angle.cos(), cy + 20.0 * angle.sin());
                r.hero_moved(id, pos, 0.0, angle + std::f32::consts::FRAC_PI_2);
                r.tick(0.05);
                r.take_first_offers();
                totems = totems.max(r.totems.len());
                for e in r.drain_events() {
                    match e {
                        Event::EnemyDied { .. } => kills += 1,
                        Event::SpellCast { spell, .. } => casts.push(spell),
                        Event::PetSummoned { pet, .. } => pets.push(pet),
                        _ => {}
                    }
                }
            }
            let who = format!("{} {}", race.name, class.name);
            assert!(kills > 5, "{who} kills ({kills}, {:?} at {:.0}s)", r.phase, r.now);
            assert!(casts.iter().any(|c| c == start_spell), "{who} casts {start_spell}");
            assert!(r.now > 60.0, "{who} lasts past the first minute ({:.0}s)", r.now);
            match class.id.as_str() {
                "hunter" => assert!(pets.iter().any(|p| p == "Wolf"), "{who}'s wolf: {pets:?}"),
                "mage" => assert!(pets.iter().any(|p| p == "Summoned Water Elemental"), "{who}'s elemental: {pets:?}"),
                "warlock" => assert!(pets.iter().any(|p| p == "Imp"), "{who}'s imp: {pets:?}"),
                "shaman" => assert!(totems > 0, "{who} puts a totem down"),
                _ => {}
            }
        }
    }

    /// A spell on the hero cast again replaces what it left (a buff, a heal over time), while an
    /// immunity stacks; a heal waits for health at or below its share, an immunity likewise.
    #[test]
    fn a_recast_spell_on_the_hero_replaces_its_buff_and_heals_wait_for_their_health_share() {
        let content = content();
        let mut r = run("warrior");
        enemy_at(&mut r, (5.0, 0.0));
        let inst = Instance::default();
        let count = |r: &Run, visual: u32| r.heroes[0].buffs.iter().filter(|b| b.visual == visual).count();
        let shout = content.ability("battle_shout").unwrap();
        assert!(r.cast(0, shout, &inst));
        assert!(r.cast(0, shout, &inst));
        assert_eq!(count(&r, shout.look_spell), 1);
        let max = r.heroes[0].stats.max_health;
        let shield = content.ability("divine_shield").unwrap();
        r.heroes[0].health = max * 0.5;
        assert!(!r.cast(0, shield, &inst));
        r.heroes[0].health = max * IMMUNITY_BELOW;
        assert!(r.cast(0, shield, &inst));
        assert!(r.cast(0, shield, &inst));
        assert_eq!(count(&r, shield.look_spell), 2);
        let renew = content.ability("renew").unwrap();
        r.heroes[0].health = max;
        assert!(!r.cast(0, renew, &inst));
        r.heroes[0].health = max * HEAL_BELOW;
        assert!(r.cast(0, renew, &inst));
        assert!(r.cast(0, renew, &inst));
        assert_eq!(count(&r, renew.look_spell), 1);
    }

    /// A warrior with no spells of its own, two enemies standing in reach that never die, no
    /// crits, no lifesteal, and almost no health: what healing charges heal shows plainly.
    fn charges_run() -> Run {
        let mut r = run("warrior");
        r.heroes[0].spells.known.clear();
        r.heroes[0].spells.racial = None;
        enemy_at(&mut r, (3.0, 0.0));
        enemy_at(&mut r, (0.0, 3.0));
        let (p, now) = (r.heroes[0].pos, r.now);
        for (e, off) in r.enemies.iter_mut().zip([(3.0, 0.0), (0.0, 3.0)]) {
            e.health = 1e9;
            e.max_health = 1e9;
            e.segment = crate::game::enemy_ai::Segment::standing((p.0 + off.0, p.1 + off.1), now);
        }
        let h = &mut r.heroes[0];
        h.stats.crit_chance = 0.0;
        h.stats.lifesteal = 0.0;
        h.health = 1.0;
        r
    }

    fn charges_buff(r: &Run) -> Option<Buff> {
        r.heroes[0].buffs.iter().copied().find(|b| b.kind == BuffKind::HealingCharges)
    }

    fn cast_spell(r: &mut Run, id: &str) {
        let content = r.content.clone();
        assert!(r.cast(0, content.ability(id).unwrap(), &Instance::default()), "{id} casts");
    }

    #[test]
    fn bloodthirst_grants_five_healing_charges_for_eight_seconds_and_a_recast_refreshes_them() {
        let mut r = charges_run();
        let bloodthirst = r.content.ability("bloodthirst").unwrap().clone();
        assert_eq!(bloodthirst.effect, SpellEffect::HealingCharges { charges: 5, share: 0.04 });
        cast_spell(&mut r, "bloodthirst");
        assert_eq!(r.heroes[0].healing_charges, 5);
        assert_eq!(r.heroes[0].health, 1.0, "its own hit uses no charge");
        let b = charges_buff(&r).expect("the buff is up");
        assert_eq!((b.visual, b.until), (bloodthirst.look_spell, r.now + 8.0));
        cast_spell(&mut r, "whirlwind");
        cast_spell(&mut r, "whirlwind");
        assert_eq!(r.heroes[0].healing_charges, 3);
        r.now += 3.0;
        cast_spell(&mut r, "bloodthirst");
        assert_eq!(r.heroes[0].healing_charges, 5, "cast again: full charges");
        assert_eq!(charges_buff(&r).unwrap().until, r.now + 8.0, "and full time");
        // a longer duration (an upgrade) keeps them longer
        let longer = Instance { duration_from_cards: 1.5, ..Default::default() };
        assert!(r.cast(0, &bloodthirst, &longer));
        assert_eq!(charges_buff(&r).unwrap().until, r.now + 12.0);
    }

    #[test]
    fn each_swing_or_cast_that_hits_uses_one_charge_and_heals_four_percent_of_max_health() {
        let mut r = charges_run();
        let max = r.heroes[0].stats.max_health;
        cast_spell(&mut r, "bloodthirst");
        // a nova hitting both enemies: one charge
        cast_spell(&mut r, "whirlwind");
        assert_eq!(r.heroes[0].healing_charges, 4);
        assert!((r.heroes[0].health - (1.0 + 0.04 * max)).abs() < 1e-3);
        // a weapon swing: one charge
        r.heroes[0].swing_timer = 0.0;
        r.weapon_swing(0, 0.01);
        assert_eq!(r.heroes[0].healing_charges, 3);
        assert!((r.heroes[0].health - (1.0 + 0.08 * max)).abs() < 1e-3);
        // a melee spell striking several enemies: one charge
        cast_spell(&mut r, "cleave");
        assert_eq!(r.heroes[0].healing_charges, 2);
        // a projectile's charge goes when it lands
        cast_spell(&mut r, "fireball");
        assert_eq!(r.heroes[0].healing_charges, 2, "still flying");
        r.now += 5.0;
        r.effects_tick();
        assert_eq!(r.heroes[0].healing_charges, 1);
        cast_spell(&mut r, "thunder_clap");
        assert_eq!(r.heroes[0].healing_charges, 0);
        assert!(charges_buff(&r).is_none(), "the last charge ends the buff");
        assert!((r.heroes[0].health - (1.0 + 0.2 * max)).abs() < 1e-2, "five charges: 20 %");
        let health = r.heroes[0].health;
        cast_spell(&mut r, "whirlwind");
        assert_eq!(r.heroes[0].health, health, "none left");
    }

    #[test]
    fn unused_healing_charges_vanish_after_eight_seconds() {
        let mut r = charges_run();
        cast_spell(&mut r, "bloodthirst");
        r.now += 8.0;
        let health = r.heroes[0].health;
        cast_spell(&mut r, "whirlwind");
        assert_eq!(r.heroes[0].health, health, "the time is up");
        r.tick(0.01);
        assert_eq!(r.heroes[0].healing_charges, 0);
        assert!(charges_buff(&r).is_none());
    }

    #[test]
    fn ticks_of_effects_over_time_and_lasting_areas_use_no_charges() {
        let mut r = charges_run();
        cast_spell(&mut r, "bloodthirst");
        // the effect landing is the cast's hit: one charge; its ticks none
        cast_spell(&mut r, "rend");
        r.effects_tick();
        assert_eq!(r.heroes[0].healing_charges, 4);
        // a lasting area: placing it hits nothing, its ticks use none
        cast_spell(&mut r, "consecration");
        for _ in 0..12 {
            r.now += 0.5;
            r.effects_tick();
        }
        assert!(r.enemies.iter().any(|e| e.health < 1e9), "the ticks struck");
        assert_eq!(r.heroes[0].healing_charges, 4);
    }

    #[test]
    fn healing_charges_belong_to_the_hero_who_cast_them() {
        let content = content();
        let setup = HeroSetup { class: "warrior".into(), race: "human".into(), ..Default::default() };
        let mut r = Run::new(content.clone(), "elwynn", RunOptions::default(), &[setup.clone(), setup], Box::new(Flat), 11).unwrap();
        r.enemies.clear();
        enemy_at(&mut r, (3.0, 0.0));
        r.enemies[0].health = 1e9;
        r.heroes[1].pos = r.heroes[0].pos;
        assert!(r.cast(0, content.ability("bloodthirst").unwrap(), &Instance::default()));
        assert!(r.cast(1, content.ability("cleave").unwrap(), &Instance::default()));
        assert_eq!((r.heroes[0].healing_charges, r.heroes[1].healing_charges), (5, 0));
        assert!(r.heroes[1].buffs.iter().all(|b| b.kind != BuffKind::HealingCharges));
    }

    /// A priest with one enemy standing 6 yd away that never dies, no crits, and no per-target
    /// bonuses: what a hit and its burn deal shows plainly.
    fn burn_run() -> (Run, u32) {
        let mut r = run("priest");
        let id = enemy_at(&mut r, (6.0, 0.0));
        let (p, now) = (r.heroes[0].pos, r.now);
        let e = r.enemies.iter_mut().find(|e| e.id == id).unwrap();
        e.health = 1e9;
        e.max_health = 1e9;
        e.segment = crate::game::enemy_ai::Segment::standing((p.0 + 6.0, p.1), now);
        let h = &mut r.heroes[0];
        h.stats.crit_chance = 0.0;
        h.stats.lifesteal = 0.0;
        h.modifiers.clear();
        (r, id)
    }

    /// Casts `holy_fire` as `inst` and plays 8 s: each hit on `id` and when it landed, the hit's
    /// own and the burn's ticks apart.
    fn holy_fire_hits(r: &mut Run, id: u32, inst: &Instance) -> (Vec<(f32, f32)>, Vec<(f32, f32)>) {
        let content = r.content.clone();
        r.drain_events();
        assert!(r.cast(0, content.ability("holy_fire").unwrap(), inst));
        let (mut hits, mut ticks) = (Vec::new(), Vec::new());
        for _ in 0..800 {
            r.now += 0.01;
            r.effects_tick();
            for e in r.drain_events() {
                match e {
                    Event::EnemyHit { id: x, amount, source: HitSource::Spell(s), .. } if x == id && s == "holy_fire" => {
                        hits.push((r.now, amount))
                    }
                    Event::EnemyHit { id: x, amount, source: HitSource::Tick(s), .. } if x == id && s == "holy_fire" => {
                        ticks.push((r.now, amount))
                    }
                    _ => {}
                }
            }
        }
        (hits, ticks)
    }

    #[test]
    fn holy_fire_hits_then_burns_its_target_five_times_over_five_seconds() {
        let (mut r, id) = burn_run();
        let d = r.heroes[0].stats.damage;
        let (hits, ticks) = holy_fire_hits(&mut r, id, &Instance::default());
        assert_eq!(hits.len(), 1);
        let base = r.content.ability("holy_fire").unwrap().damage;
        assert!((hits[0].1 - base * d).abs() < 1e-2, "{hits:?}");
        assert_eq!(ticks.len(), 5, "{ticks:?}");
        for (k, (at, amount)) in ticks.iter().enumerate() {
            assert!((amount - 21.0 * d).abs() < 1e-2, "{ticks:?}");
            // the first tick a second after the hit lands, then one a second
            assert!((at - hits[0].0 - (k + 1) as f32).abs() < 0.02, "{ticks:?} after {hits:?}");
        }
    }

    #[test]
    fn power_scales_the_burn_with_the_hit_and_a_longer_duration_adds_whole_ticks() {
        let (mut r, id) = burn_run();
        let d = r.heroes[0].stats.damage;
        let inst = Instance { power_from_cards: 1.5, duration_from_cards: 1.3, ..Default::default() };
        let (hits, ticks) = holy_fire_hits(&mut r, id, &inst);
        let base = r.content.ability("holy_fire").unwrap().damage;
        assert!((hits[0].1 - base * 1.5 * d).abs() < 1e-2, "{hits:?}");
        // 30 % longer: 6 whole ticks of the same size, still a second apart
        assert_eq!(ticks.len(), 6, "{ticks:?}");
        assert!(ticks.iter().all(|(_, amount)| (amount - 31.5 * d).abs() < 1e-2), "{ticks:?}");
        assert!((ticks[5].0 - hits[0].0 - 6.0).abs() < 0.02, "{ticks:?}");
    }

    #[test]
    fn a_second_holy_fire_renews_the_burn_on_its_target() {
        let (mut r, id) = burn_run();
        let content = r.content.clone();
        let holy_fire = content.ability("holy_fire").unwrap();
        for _ in 0..2 {
            assert!(r.cast(0, holy_fire, &Instance::default()));
            for _ in 0..150 {
                r.now += 0.01;
                r.effects_tick();
            }
        }
        assert_eq!(r.dot_stacks(id, "holy_fire"), 1, "one stack with no extra stacks");
    }

    /// `pw_shield` is cast again only once its shield is gone, and its cooldown runs from
    /// the cast: a shield broken early waits out the cooldown, a shield that outlasts it is
    /// replaced as soon as it breaks.
    #[test]
    fn power_word_shield_waits_for_its_shield_to_break_and_its_cooldown_runs_from_the_cast() {
        let mut r = run("priest");
        enemy_at(&mut r, (6.0, 0.0));
        let content = r.content.clone();
        let a = content.ability("pw_shield").unwrap();
        let h = &mut r.heroes[0];
        h.spells.racial = None;
        h.spells.known = vec![crate::game::spells::SpellSlot { timer: 0.0, ..crate::game::spells::SpellSlot::new(a, &[]) }];
        let cd = crate::game::spells::cooldown(a, &h.spells.known[0].instance(), h.stats.cooldown_factor);
        let casts = |r: &mut Run| {
            r.drain_events().iter().filter(|e| matches!(e, Event::SpellCast { spell, .. } if spell == "pw_shield")).count()
        };
        r.spells_tick(0.01);
        assert_eq!(casts(&mut r), 1);
        assert!(r.heroes[0].shield > 0.0);
        // broken a third of the way in: no new shield until the cooldown from the cast is over
        r.spells_tick(cd / 3.0);
        r.heroes[0].shield = 0.0;
        r.spells_tick(cd / 3.0);
        r.spells_tick(cd / 3.0 - 0.05);
        assert_eq!((casts(&mut r), r.heroes[0].shield), (0, 0.0));
        r.spells_tick(0.1);
        assert_eq!(casts(&mut r), 1, "cast again once the cooldown from the first cast ran out");
        // the shield holds past the cooldown: no cast while it holds, a new one as soon as it breaks
        for _ in 0..3 {
            r.spells_tick(cd / 2.0);
        }
        assert_eq!(casts(&mut r), 0);
        r.heroes[0].shield = 0.0;
        r.spells_tick(RETRY + 0.01);
        assert_eq!(casts(&mut r), 1);
    }

    /// Divine Shield's cooldown starts only once its shield has ended
    /// (`cooldown_after_effect`): the bar sweeps the shield's time, then the cooldown; however short
    /// the cooldown, the shield is never up all the time.
    #[test]
    fn divine_shields_cooldown_starts_when_its_shield_ends() {
        let mut r = run("paladin");
        enemy_at(&mut r, (3.0, 0.0));
        let content = r.content.clone();
        let a = content.ability("divine_shield").unwrap();
        assert!(a.kit.cooldown_after_effect);
        let h = &mut r.heroes[0];
        h.spells.racial = None;
        h.spells.known = vec![crate::game::spells::SpellSlot { timer: 0.0, ..crate::game::spells::SpellSlot::new(a, &[]) }];
        // even with the cooldown cut far below the shield's time
        h.spells.known[0].upgrades.cooldown = 0.05;
        let cd = crate::game::spells::cooldown(a, &h.spells.known[0].instance(), h.stats.cooldown_factor);
        assert!(cd < a.duration, "a cooldown shorter than the shield");
        let step = |r: &mut Run, dt: f32| {
            r.now += dt;
            let max = r.heroes[0].stats.max_health;
            r.heroes[0].health = max * IMMUNITY_BELOW * 0.5;
            r.spells_tick(dt);
            r.drain_events()
        };
        let sweeps = |ev: &[Event]| ev.iter().filter_map(|e| if let Event::SpellCooldown { spell, seconds, .. } = e { (spell == "divine_shield").then_some(*seconds) } else { None }).collect::<Vec<_>>();
        let casts = |ev: &[Event]| ev.iter().filter(|e| matches!(e, Event::SpellCast { spell, .. } if spell == "divine_shield")).count();
        let ev = step(&mut r, 0.01);
        assert_eq!(casts(&ev), 1);
        // the bar sweeps the shield's 6 s
        assert_eq!(sweeps(&ev).len(), 1);
        assert!((sweeps(&ev)[0] - a.duration).abs() < 0.05, "{:?}", sweeps(&ev));
        // not cast again while the shield holds
        let mut n = 0;
        let mut t = 0.0;
        while t < a.duration - 0.1 {
            n += casts(&step(&mut r, 0.05));
            t += 0.05;
        }
        assert_eq!(n, 0, "never again while the shield holds");
        // the shield ends: its cooldown starts, swept from then, and it is cast again after it
        let mut ev = Vec::new();
        for _ in 0..4 {
            ev.extend(step(&mut r, 0.05));
        }
        assert_eq!(casts(&ev), 0);
        assert!(sweeps(&ev).iter().any(|s| (s - cd).abs() < 1e-3), "the cooldown's sweep: {:?}", sweeps(&ev));
        let mut ev = Vec::new();
        for _ in 0..((cd / 0.05) as usize + 3) {
            ev.extend(step(&mut r, 0.05));
        }
        assert_eq!(casts(&ev), 1, "cast again once its cooldown after the shield ran out");
    }

    /// Heal and shield power strengthens the heals of the hero's spells
    /// and the shields it casts, and nothing else that heals.
    #[test]
    fn heal_and_shield_power_strengthens_spell_heals_and_shields_only() {
        let content = content();
        let inst = Instance::default();
        // what one cast heals (or shields), at a heal and shield power of `power`
        let healed = |class: &str, spell: &str, power: f32| {
            let mut r = run(class);
            enemy_at(&mut r, (5.0, 0.0));
            r.heroes[0].stats.heal_and_shield_power = power;
            let max = r.heroes[0].stats.max_health;
            r.heroes[0].health = max * 0.05;
            r.drain_events();
            assert!(r.cast(0, content.ability(spell).unwrap(), &inst), "{spell}");
            let events = r.drain_events();
            let heals: f32 = events.iter().filter_map(|e| if let Event::HeroHealed { amount, .. } = e { Some(*amount) } else { None }).sum();
            heals + r.heroes[0].shield
        };
        for (class, spell) in [("paladin", "holy_light"), ("priest", "pw_shield")] {
            let (plain, strong) = (healed(class, spell, 0.0), healed(class, spell, 0.5));
            assert!(plain > 0.0 && (strong / plain - 1.5).abs() < 1e-3, "{spell}: {plain} -> {strong}");
        }
        // a spell's healing from its damage grows too; health per kill and lifesteal do not
        let mut r = run("warlock");
        r.heroes[0].stats.heal_and_shield_power = 1.0;
        r.heroes[0].stats.lifesteal = 0.0;
        r.heroes[0].health = 1.0;
        r.apply_on_hit(0, 0, &SpellEffect::HealFromDamage { share: 0.5 }, 10.0);
        assert!((r.heroes[0].health - 11.0).abs() < 1e-4, "10 × 0.5 × 2: {}", r.heroes[0].health);
        r.heroes[0].stats.heal_per_kill = 3.0;
        r.heroes[0].health = 1.0;
        let id = enemy_at(&mut r, (5.0, 0.0));
        r.hit_enemy(Some(0), id, f32::MAX, false, HitSource::Other);
        assert!((r.heroes[0].health - 4.0).abs() < 1e-4, "3 per kill, as it is: {}", r.heroes[0].health);
    }

    /// A mage with enemies that never die or move standing at `offsets`, no crits, and no
    /// per-target bonuses: what a ground area deals shows plainly.
    fn standing_enemies_run(offsets: &[Vec2]) -> (Run, Vec<u32>) {
        let mut r = run("mage");
        let (p, now) = (r.heroes[0].pos, r.now);
        let ids: Vec<u32> = offsets.iter().map(|&o| enemy_at(&mut r, o)).collect();
        for (&id, o) in ids.iter().zip(offsets) {
            let e = r.enemies.iter_mut().find(|e| e.id == id).unwrap();
            e.health = 1e9;
            e.max_health = 1e9;
            e.segment = crate::game::enemy_ai::Segment::standing((p.0 + o.0, p.1 + o.1), now);
        }
        let h = &mut r.heroes[0];
        h.stats.crit_chance = 0.0;
        h.stats.lifesteal = 0.0;
        h.modifiers.clear();
        (r, ids)
    }

    /// Casts `a` as `inst` and plays 10 s: each of its hits on each of `ids`, with how long
    /// after the cast it landed.
    fn area_hits(r: &mut Run, a: &Ability, inst: &Instance, ids: &[u32]) -> Vec<Vec<(f32, f32)>> {
        r.drain_events();
        let start = r.now;
        assert!(r.cast(0, a, inst));
        let mut hits = vec![Vec::new(); ids.len()];
        for step in 0..=1000 {
            if step > 0 {
                r.now += 0.01;
                r.effects_tick();
            }
            for e in r.drain_events() {
                if let Event::EnemyHit { id, amount, source: HitSource::Spell(s), .. } = e {
                    if let Some(k) = ids.iter().position(|&x| x == id).filter(|_| s == a.id) {
                        hits[k].push((r.now - start, amount));
                    }
                }
            }
        }
        hits
    }

    #[test]
    fn flamestrike_strikes_everything_inside_at_once_then_burns_it_four_times() {
        let (mut r, ids) = standing_enemies_run(&[(6.0, 0.0), (7.0, 0.0), (6.0, 25.0)]);
        let d = r.heroes[0].stats.damage;
        let content = r.content.clone();
        let hits = area_hits(&mut r, content.ability("flamestrike").unwrap(), &Instance::default(), &ids);
        for inside in &hits[..2] {
            assert_eq!(inside.len(), 5, "the impact and 4 ticks: {inside:?}");
            assert!(inside[0].0 < 1e-4 && (inside[0].1 - 46.0 * d).abs() < 1e-2, "the impact as it lands: {inside:?}");
            assert!(inside[1..].iter().all(|(at, amount)| *at > 0.0 && (amount - 17.5 * d).abs() < 1e-2), "{inside:?}");
        }
        assert!(hits[2].is_empty(), "an enemy outside takes nothing: {:?}", hits[2]);
    }

    #[test]
    fn power_and_a_tag_coefficient_scale_flamestrike_s_impact_with_its_ticks() {
        let (mut r, ids) = standing_enemies_run(&[(6.0, 0.0)]);
        let d = r.heroes[0].stats.damage;
        let content = r.content.clone();
        let flamestrike = content.ability("flamestrike").unwrap();
        let hits = area_hits(&mut r, flamestrike, &Instance { power_from_cards: 1.5, ..Default::default() }, &ids);
        assert!((hits[0][0].1 - 69.0 * d).abs() < 1e-2 && (hits[0][1].1 - 26.25 * d).abs() < 1e-2, "{hits:?}");
        // the area tag at 2 doubles both
        let mut retagged = vec![flamestrike.clone()];
        let k = esurv_content::SpellTagCoefficients { area: 2.0, ..Default::default() };
        esurv_content::apply_spell_tags(&mut retagged, &mut [], &k).unwrap();
        let (mut r, ids) = standing_enemies_run(&[(6.0, 0.0)]);
        let hits = area_hits(&mut r, &retagged[0], &Instance::default(), &ids);
        assert!((hits[0][0].1 - 92.0 * d).abs() < 1e-2 && (hits[0][1].1 - 35.0 * d).abs() < 1e-2, "{hits:?}");
    }

    #[test]
    fn hellfire_goes_off_three_times_a_second_apart_each_around_where_the_hero_then_stands() {
        let (mut r, ids) = standing_enemies_run(&[(3.0, 0.0), (23.0, 0.0), (43.0, 0.0)]);
        let (d, p) = (r.heroes[0].stats.damage, r.heroes[0].pos);
        let hellfire = r.content.clone().ability("hellfire").unwrap().clone();
        r.drain_events();
        let start = r.now;
        assert!(r.cast(0, &hellfire, &Instance::default()));
        let (mut hits, mut looks, mut casts) = (vec![Vec::new(); ids.len()], 0, 0);
        for step in 0..=600 {
            // the hero walks 20 yd on between the novas
            if step == 50 || step == 150 {
                r.heroes[0].pos.0 += 20.0;
            }
            if step > 0 {
                r.now += 0.01;
                r.effects_tick();
            }
            for e in r.drain_events() {
                match e {
                    Event::EnemyHit { id, amount, source: HitSource::Spell(s), .. } if s == hellfire.id => {
                        hits[ids.iter().position(|&x| x == id).unwrap()].push((r.now - start, amount));
                    }
                    Event::SpellGo { spell, .. } if spell == hellfire.look_spell => looks += 1,
                    Event::SpellCast { .. } => casts += 1,
                    _ => {}
                }
            }
        }
        assert_eq!((looks, casts), (3, 1), "three novas shown, one cast");
        for (k, at) in [0.0, 1.0, 2.0].into_iter().enumerate() {
            assert_eq!(hits[k].len(), 1, "enemy {k} is hit once, by the nova around the hero then: {hits:?}");
            assert!((hits[k][0].0 - at).abs() < 0.02 && (hits[k][0].1 - 13.5 * d).abs() < 1e-2, "{hits:?}");
        }
        assert_eq!(r.heroes[0].pos, (p.0 + 40.0, p.1));
        assert!(r.novas.is_empty());
    }

    #[test]
    fn a_longer_hellfire_adds_whole_novas_and_power_scales_each() {
        let (mut r, ids) = standing_enemies_run(&[(3.0, 0.0)]);
        let d = r.heroes[0].stats.damage;
        let hellfire = r.content.clone().ability("hellfire").unwrap().clone();
        let hits = area_hits(&mut r, &hellfire, &Instance { duration_from_cards: 1.5, power_from_cards: 1.5, ..Default::default() }, &ids);
        assert_eq!(hits[0].len(), 4, "4.5 s holds 4 whole novas: {hits:?}");
        for (k, (at, amount)) in hits[0].iter().enumerate() {
            assert!((at - k as f32).abs() < 0.02 && (amount - 20.25 * d).abs() < 1e-2, "{hits:?}");
        }
        let hits = area_hits(&mut r, &hellfire, &Instance { duration_from_cards: 1.3, ..Default::default() }, &ids);
        assert_eq!(hits[0].len(), 3, "3.9 s holds 3: {hits:?}");
    }

    #[test]
    fn only_the_first_nova_of_a_hellfire_uses_a_healing_charge() {
        let (mut r, ids) = standing_enemies_run(&[(3.0, 0.0)]);
        let content = r.content.clone();
        assert!(r.cast(0, content.ability("bloodthirst").unwrap(), &Instance::default()));
        assert_eq!(r.heroes[0].healing_charges, 5);
        let hellfire = content.ability("hellfire").unwrap().clone();
        let hits = area_hits(&mut r, &hellfire, &Instance::default(), &ids);
        assert_eq!((hits[0].len(), r.heroes[0].healing_charges), (3, 4));
    }

    #[test]
    fn a_fallen_hero_s_hellfire_goes_off_no_more() {
        let (mut r, ids) = standing_enemies_run(&[(3.0, 0.0)]);
        let hellfire = r.content.clone().ability("hellfire").unwrap().clone();
        assert!(r.cast(0, &hellfire, &Instance::default()));
        r.heroes[0].alive = false;
        r.drain_events();
        r.now += 3.0;
        r.effects_tick();
        let hit = r.drain_events().into_iter().any(|e| matches!(e, Event::EnemyHit { id, .. } if id == ids[0]));
        assert!(!hit && r.novas.is_empty());
    }
}
