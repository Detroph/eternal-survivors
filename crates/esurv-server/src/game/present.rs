//! What happened in the run, as the clients are told it: spell looks (`SMSG_SPELL_GO`),
//! damage and heal numbers, melee swings, enemy casts, cooldowns on the action bar, shouted
//! lines and centre-screen lines, and the run's numbers for the character pane.

use esurv_content::School;
use esurv_sim::game::casting::HitSource;
use esurv_sim::game::run::{Aim, BuffKind, Event, Hero, Speaker, Unit};

use super::Game;
use crate::packets::{self, SpellTarget};
use crate::wire::Packet;

/// A dead enemy lies this long before it is removed.
pub const CORPSE_TIME: f32 = 1.5;
/// The race spell's place on the action bar: the first bar's last button.
pub const RACE_SPELL_SLOT: u8 = 11;
/// The prefix of the run's numbers for the character pane.
pub const STATS_PREFIX: &str = esurv_protocol::NUMBERS_PREFIX;
/// The prefix of a spell look's size (esurv-app reads it): `<caster guid, hex> <spell>
/// <scale>`, sent just before the cast it sizes. An area look shows its
/// true size.
pub const LOOK_SIZE_PREFIX: &str = esurv_protocol::LOOK_SIZE_PREFIX;
/// How long a legendary moment's look put down at a spot stays (`Game::spot_look`).
pub const SPOT_LOOK_STAYS: f32 = 2.5;
/// A look within this share of its own size is left as it is.
const LOOK_SIZE_SLACK: f32 = 0.01;

/// The buff slots a hero's buffs take (1.12 has 32), and the 16 debuff slots after them.
pub const BUFF_SLOTS: usize = 32;
pub const DEBUFF_SLOTS: usize = 16;
/// An aura slot's flags: effect 0 is on (the client shows the icon).
const AURA_FLAGS: u32 = 0x02;

/// A shown aura: its look, how many stack, and when it ends (run seconds).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aura {
    pub spell: u32,
    pub stacks: u32,
    pub until: f32,
}

/// The hero's buffs as auras: one slot per look, stacks counted, the latest end kept. Healing
/// charges stack as the charges left.
pub fn hero_auras(hero: &Hero, now: f32) -> Vec<Aura> {
    let mut out: Vec<Aura> = Vec::new();
    for b in hero.buffs.iter().filter(|b| b.visual != 0 && b.until > now) {
        let stacks = if b.kind == BuffKind::HealingCharges {
            hero.healing_charges.max(1)
        } else {
            1
        };
        match out.iter_mut().find(|a| a.spell == b.visual) {
            Some(a) => {
                a.stacks += stacks;
                a.until = a.until.max(b.until);
            }
            None => out.push(Aura {
                spell: b.visual,
                stacks,
                until: b.until,
            }),
        }
    }
    out.truncate(BUFF_SLOTS);
    out
}

/// An enemy's effects over time as debuffs: one slot per spell, its stacks counted
/// (they stay visible on the enemy).
pub fn enemy_debuffs(
    content: &esurv_content::Content,
    enemy: &esurv_sim::game::run::EnemyUnit,
) -> Vec<Aura> {
    let mut out: Vec<Aura> = Vec::new();
    for d in enemy.dots.iter().filter(|d| d.ticks_left > 0) {
        let Some(spell) = content.ability(&d.spell).map(|a| a.look_spell) else {
            continue;
        };
        match out.iter_mut().find(|a| a.spell == spell) {
            Some(a) => a.stacks += 1,
            None => out.push(Aura {
                spell,
                stacks: 1,
                until: 0.0,
            }),
        }
    }
    out.truncate(DEBUFF_SLOTS);
    out
}

/// The legendary moments' lasting looks a unit wears now, as auras.
pub fn worn_looks(worn: &[(u32, f32)], now: f32) -> Vec<Aura> {
    worn.iter()
        .filter(|(spell, until)| *spell != 0 && *until > now)
        .map(|&(spell, until)| Aura { spell, stacks: 1, until })
        .collect()
}

/// The aura fields for `auras` in the slots from `first` on (0 for buffs, [`BUFF_SLOTS`] for
/// debuffs), clearing the next of the `clear` slots that are now empty.
pub fn aura_fields(auras: &[Aura], clear: usize, first: usize) -> crate::update::Fields {
    use crate::update::field;
    let mut f = crate::update::Fields::new();
    let room = if first == 0 { BUFF_SLOTS } else { DEBUFF_SLOTS };
    let used = auras.len().max(clear).min(room);
    if used == 0 {
        return f;
    }
    for k in 0..used {
        f.u32(
            field::UNIT_AURA + (first + k) as u16,
            auras.get(k).map_or(0, |a| a.spell),
        );
    }
    // the flags (8 slots a word) and the level and stack bytes (4 slots a word) of the words the
    // used slots fall in; the slot ranges start on a word, so no other slot shares them
    let slot_of = |k: usize| auras.get(k);
    for word in first / 8..(first + used).div_ceil(8) {
        let mut v = 0u32;
        for k in 0..8 {
            let slot = word * 8 + k;
            if slot >= first && slot_of(slot - first).is_some() {
                v |= AURA_FLAGS << (k * 4);
            }
        }
        f.u32(field::UNIT_AURAFLAGS + word as u16, v);
    }
    for word in first / 4..(first + used).div_ceil(4) {
        let (mut levels, mut stacks) = (0u32, 0u32);
        for k in 0..4 {
            let slot = word * 4 + k;
            if let Some(a) = (slot >= first).then(|| slot_of(slot - first)).flatten() {
                levels |= 1 << (k * 8);
                stacks |= (a.stacks.saturating_sub(1).min(254)) << (k * 8);
            }
        }
        f.u32(field::UNIT_AURALEVELS + word as u16, levels);
        f.u32(field::UNIT_AURAAPPLICATIONS + word as u16, stacks);
    }
    f
}

/// 1.12's school number.
pub fn school_id(s: School) -> u8 {
    match s {
        School::Physical => 0,
        School::Holy => 1,
        School::Fire => 2,
        School::Nature => 3,
        School::Frost => 4,
        School::Shadow => 5,
        School::Arcane => 6,
    }
}

/// Whole points of damage or healing, as the client is told them.
fn points(v: f32) -> u32 {
    v.max(0.0) as u32
}

/// What the class gear a hero wears adds, stat by stat, as the numbers line's `gear_` keys carry it:
/// flat health, then the shares of damage, cooldown recovery, critical strike chance and damage,
/// damage reduction, health a second, area, extra targets, double cast, movement speed, pickup
/// range and gold; then luck, health per kill and heal and shield power.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GearAdds {
    pub health: f32,
    pub damage: f32,
    pub cooldown: f32,
    pub crit: f32,
    pub crit_damage: f32,
    pub reduction: f32,
    pub regen: f32,
    pub area: f32,
    pub targets: f32,
    pub double_cast: f32,
    pub speed: f32,
    pub pickup: f32,
    pub gold: f32,
    pub luck: f32,
    pub heal_per_kill: f32,
    pub heal_and_shield_power: f32,
}

/// What the pieces `worn` (item entries) add to a hero of `class`: another class's piece or an
/// unknown entry adds nothing, as a run leaves it off.
pub fn gear_adds(content: &esurv_content::Content, class: &str, worn: &[u32]) -> GearAdds {
    use esurv_content::Stat;
    use esurv_sim::game::gear::{decode_item_entry, piece_modifiers};
    let mut adds = GearAdds::default();
    let (Some(set), Some(c)) = (content.gear_set(class), content.class(class)) else {
        return adds;
    };
    for &entry in worn {
        let Some((wow_class, slot, zone)) = decode_item_entry(entry) else {
            continue;
        };
        if wow_class != c.wow_class_id {
            continue;
        }
        for m in piece_modifiers(content, set, slot, zone).unwrap_or_default() {
            let into = match m.stat {
                Stat::FlatMaxHealth => &mut adds.health,
                Stat::Damage => &mut adds.damage,
                Stat::CooldownRecovery => &mut adds.cooldown,
                Stat::CritChance => &mut adds.crit,
                Stat::CritDamage => &mut adds.crit_damage,
                Stat::DamageReduction => &mut adds.reduction,
                Stat::HealthPerSecond => &mut adds.regen,
                Stat::Area => &mut adds.area,
                Stat::ExtraTargets => &mut adds.targets,
                Stat::DoubleCast => &mut adds.double_cast,
                Stat::MoveSpeed => &mut adds.speed,
                Stat::PickupRange => &mut adds.pickup,
                Stat::Gold => &mut adds.gold,
                Stat::Luck => &mut adds.luck,
                Stat::HealPerKill => &mut adds.heal_per_kill,
                Stat::HealAndShieldPower => &mut adds.heal_and_shield_power,
                _ => continue,
            };
            *into += m.value;
        }
    }
    adds
}

/// The run's numbers line for the character pane: `key=value;` pairs named after the stat, each
/// with its own number of decimals; the `gear_` keys are what worn gear adds (`gear`).
pub fn stats_line(hero: &Hero, gear: &GearAdds) -> String {
    let s = &hero.stats;
    format!(
        "health={};damage={:.3};cooldown={:.3};crit={:.3};crit_damage={:.2};reduction={:.3};regen={:.2};area={:.3};\
         targets={};double_cast={:.3};speed={:.3};pickup={:.1};experience={:.2};luck={:.2};gold={:.2};\
         heal_per_kill={:.1};heal_shield={:.3};gear_health={};gear_damage={:.3};gear_cooldown={:.3};gear_crit={:.3};\
         gear_crit_damage={:.2};gear_reduction={:.3};gear_regen={:.2};gear_area={:.3};gear_targets={};\
         gear_double_cast={:.3};gear_speed={:.3};gear_pickup={:.3};gear_gold={:.3};gear_luck={:.3};\
         gear_heal_per_kill={};gear_heal_shield={:.3}",
        s.max_health,
        s.damage,
        s.cooldown_factor,
        s.crit_chance,
        s.crit_damage,
        s.damage_reduction,
        s.health_regen,
        s.area,
        s.extra_targets,
        s.double_cast,
        s.move_speed,
        s.pickup_range,
        s.experience,
        s.luck,
        s.gold,
        s.heal_per_kill,
        s.heal_and_shield_power,
        gear.health.round(),
        gear.damage,
        gear.cooldown,
        gear.crit,
        gear.crit_damage,
        gear.reduction,
        gear.regen,
        gear.area,
        gear.targets.max(0.0) as u32,
        gear.double_cast,
        gear.speed,
        gear.pickup,
        gear.gold,
        gear.luck,
        gear.heal_per_kill.round(),
        gear.heal_and_shield_power,
    )
}

/// A cooldown shorter than this shows no sweep: a spell that casts again at once, or one that
/// found nothing to do and tries again shortly.
const SHORTEST_SWEEP: f32 = 0.25;

/// The packets that set the sweep of the bar button holding look `look` to `seconds`: the
/// client's records for the look are cleared first (a cast's `SMSG_SPELL_GO` starts the look's
/// own `Spell.dbc` cooldown, which is not the spell's: Divine Star looks like Holy Shock, 30 s
/// against its 8), then the run's cooldown is set. A zero `SMSG_SPELL_COOLDOWN` would mean "the
/// look's own `Spell.dbc` cooldown", so none is sent for a spell that is ready.
pub(super) fn bar_sweep(caster: u64, look: u32, seconds: f32) -> Vec<Packet> {
    let mut out = vec![packets::clear_cooldown(caster, look)];
    if seconds >= SHORTEST_SWEEP {
        out.push(packets::spell_cooldown(
            caster,
            &[(look, (seconds * 1000.0) as u32)],
        ));
    }
    out
}

impl Game {
    /// After every `SMSG_SPELL_GO` a hero casts in `sent` (its spells', a starting option's extra
    /// looks, a card's or milestone's casts), that hero's client gets its bar sweep for the look
    /// set back to the run's cooldown ([`bar_sweep`]): the time left on the hero's spell that
    /// wears the look, or none for a look no spell of the hero's wears.
    pub(super) fn bar_sweeps_after_casts(&self, sent: &[Packet]) -> Vec<(u32, Packet)> {
        use benilla_protocol::messages::{opcode, parse_server_with_tail, ServerPacket};
        let mut seen: Vec<(u32, u32)> = Vec::new();
        for p in sent.iter().filter(|p| p.opcode == opcode::SMSG_SPELL_GO) {
            let Ok((ServerPacket::SpellGo(go), _)) = parse_server_with_tail(p.opcode, &p.body)
            else {
                continue;
            };
            let Some(player) = self.players.iter().find(|pl| pl.guid() == go.caster) else {
                continue;
            };
            if !seen.contains(&(player.hero, go.spell_id)) {
                seen.push((player.hero, go.spell_id));
            }
        }
        let mut out = Vec::new();
        for (hero, look) in seen {
            let Some(h) = self.run.heroes.iter().find(|h| h.id == hero) else {
                continue;
            };
            let Some(guid) = self.guid_of(Unit::Hero(hero)) else {
                continue;
            };
            let left = h
                .spells
                .known
                .iter()
                .chain(h.spells.racial.iter())
                .filter(|s| self.ability_look(&s.spell).is_some_and(|(v, _)| v == look))
                .map(|s| s.timer)
                .fold(0.0_f32, f32::max);
            out.extend(bar_sweep(guid, look, left).into_iter().map(|p| (hero, p)));
        }
        out
    }

    /// The guid a unit has on the wire, if the clients know it.
    pub(super) fn guid_of(&self, u: Unit) -> Option<u64> {
        match u {
            Unit::Hero(id) => self.players.iter().find(|p| p.hero == id).map(|p| p.guid()),
            Unit::Enemy(id) => self.shown.get(&id).map(|s| s.guid),
            // a ghostly copy of a hero is a summon the clients know as a player
            Unit::Pet(id) => self
                .shown
                .get(&id)
                .map(|s| s.guid)
                .or_else(|| self.clones.get(&id).filter(|c| c.created).map(|c| c.guid)),
            Unit::Totem(id) => self.totems_shown.get(&id).copied(),
        }
    }

    /// A unit's name, for the lines it says or is named in.
    fn name_of(&self, u: Unit) -> String {
        match u {
            Unit::Hero(id) => self
                .players
                .iter()
                .find(|p| p.hero == id)
                .map(|p| p.saved.name.clone()),
            Unit::Enemy(id) | Unit::Pet(id) => self.shown.get(&id).map(|s| s.name.clone()),
            Unit::Totem(_) => Some("Totem".into()),
        }
        .unwrap_or_default()
    }

    /// An ability's look (`Spell.dbc` id) and school.
    fn ability_look(&self, id: &str) -> Option<(u32, u8)> {
        self.run
            .content
            .ability(id)
            .map(|a| (a.look_spell, school_id(a.school)))
    }

    /// The look and school an ability's hits are logged under: its pulse spell's when it has one
    /// (a totem's pulse; Fan of Knives' hits, whose look is a ranged basic shot the client would
    /// show white), else its look's.
    fn hit_look(&self, id: &str) -> Option<(u32, u8)> {
        let a = self.run.content.ability(id)?;
        let spell = a.pulse_spell.unwrap_or(a.look_spell);
        Some((spell, school_id(a.school)))
    }

    /// A pet's ranged spell (0 for a melee pet) and school, while the clients know the pet.
    fn pet_attack(&self, id: u32) -> Option<(u32, School)> {
        let name = match self.run.pets.iter().find(|p| p.id == id) {
            Some(p) => p.pet.clone(),
            None => self.shown.get(&id)?.name.clone(),
        };
        let kind = self.run.content.pets.iter().find(|k| k.name == name)?;
        Some((kind.ranged_spell.unwrap_or(0), kind.school))
    }

    /// The hero's spellbook (a run's spellbook holds only the race's own
    /// spells and what the hero learned in the run): the race spell, the race's passives under
    /// their stock racial spells (found by name in the player's `Spell.dbc`; none without the
    /// game data), then the known spells, as `Spell.dbc` ids, each once.
    pub(super) fn spellbook(&self, hero: &Hero) -> Vec<u32> {
        let race_spell = hero
            .spells
            .racial
            .iter()
            .filter_map(|s| self.ability_look(&s.spell))
            .map(|(v, _)| v);
        let passives = self
            .run
            .content
            .races
            .get(hero.race)
            .into_iter()
            .flat_map(|r| &r.passives)
            .filter_map(|p| self.data.as_ref()?.racial_spell(p.stock_spell_name()));
        let known = hero
            .spells
            .known
            .iter()
            .filter_map(|s| self.ability_look(&s.spell))
            .map(|(v, _)| v);
        let mut book: Vec<u32> = Vec::new();
        for id in race_spell.chain(passives).chain(known) {
            if !book.contains(&id) {
                book.push(id);
            }
        }
        book
    }

    /// The hero's action bar: known spell `i` on button `i`, the race spell on the last button.
    pub(super) fn action_bar(&self, hero: &Hero) -> Packet {
        let mut buttons: Vec<(u8, u32)> = hero
            .spells
            .known
            .iter()
            .enumerate()
            .filter_map(|(i, s)| Some((i as u8, self.ability_look(&s.spell)?.0)))
            .collect();
        if let Some((v, _)) = hero
            .spells
            .racial
            .as_ref()
            .and_then(|s| self.ability_look(&s.spell))
        {
            buttons.push((RACE_SPELL_SLOT, v));
        }
        packets::action_buttons(&buttons)
    }

    /// The packets for this tick's events: those every client gets, and those for one hero's
    /// client only.
    pub(super) fn present(&mut self, events: &[Event]) -> (Vec<Packet>, Vec<(u32, Packet)>) {
        let mut all = Vec::new();
        // the starting options' own looks and sounds
        all.extend(self.option_extras(events));
        let mut own = Vec::new();
        // heroes whose next weapon-swing hit is their off hand's
        let mut off_hand = Vec::new();
        for ev in events {
            match ev {
                Event::OffHandSwing { hero } => off_hand.push(*hero),
                Event::SpellGo {
                    caster,
                    spell,
                    hits,
                    aim,
                    ammo,
                    reach,
                    triggered,
                } => {
                    // a lasher's burst plays from the ground where it stood (`option_extras`)
                    if matches!(caster, Unit::Pet(_)) && *spell == super::starting_looks::extras::LASHER_BLAST {
                        continue;
                    }
                    let Some(from) = self.guid_of(*caster) else {
                        continue;
                    };
                    // a legendary moment's look at a spot plays there, from a look of its own put
                    // down on it
                    if let (Unit::Hero(_), Aim::Ground(at), true) = (caster, aim, hits.is_empty()) {
                        if self.run.content.is_legendary_look(*spell) && self.run.world.missile_speed(*spell) <= 0.0 {
                            all.extend(self.spot_look(from, *spell, *at, *reach, SPOT_LOOK_STAYS));
                            continue;
                        }
                    }
                    let hits: Vec<u64> = hits.iter().filter_map(|&u| self.guid_of(u)).collect();
                    let Some(target) = self.spell_target(*aim) else {
                        continue;
                    };
                    // an area's look at its true size: its size first, then the cast
                    let scale = self.look_scale(*spell, *reach);
                    if (scale - 1.0).abs() > LOOK_SIZE_SLACK {
                        if let Some(me) = self.players.first().map(|p| p.guid()) {
                            all.push(packets::addon_message(
                                me,
                                LOOK_SIZE_PREFIX,
                                &format!("{from:X} {spell} {scale:.3}"),
                            ));
                        }
                    }
                    // a proc's look is a triggered spell's: the caster plays no cast for it
                    let go = if *triggered { packets::triggered_spell_go } else { packets::spell_go };
                    all.push(go(from, *spell, &hits, target, ammo.then_some(packets::ARROW)));
                    // a shot plays the shooting of the ranged weapon the hero holds (not for a
                    // proc: the hero shoots nothing)
                    if let (Unit::Hero(h), false) = (caster, *triggered) {
                        if let Some(look) = self.shot_look(*h, *spell) {
                            all.push(packets::spell_go(from, look, &[], target, None));
                        }
                    }
                    // and so does a player-looking enemy's, by the ranged weapon it shows
                    if let Unit::Enemy(id) = caster {
                        if let Some(look) = self.enemy_shot_look(*id, *spell) {
                            all.push(packets::spell_go(from, look, &[], target, None));
                        }
                    }
                }
                Event::EnemyHit {
                    id,
                    amount,
                    crit,
                    by,
                    source,
                } => {
                    let Some(target) = self.guid_of(Unit::Enemy(*id)) else {
                        continue;
                    };
                    let hero = by.and_then(|h| self.guid_of(Unit::Hero(h)));
                    match (source, hero) {
                        (HitSource::Spell(a), Some(h)) => {
                            if let Some((spell, school)) = self.hit_look(a) {
                                all.push(packets::spell_damage_log(
                                    target,
                                    h,
                                    spell,
                                    points(*amount),
                                    school,
                                    *crit,
                                ));
                            }
                        }
                        (HitSource::Tick(a), Some(h)) => {
                            if let Some((spell, school)) = self.ability_look(a) {
                                all.push(packets::periodic_damage_log(
                                    target,
                                    h,
                                    spell,
                                    points(*amount),
                                    school,
                                ));
                            }
                        }
                        // a ranged swing (Auto Shot) logs as its shot, a melee one as a swing
                        (HitSource::WeaponSwing, Some(h)) => match by.and_then(|b| self.ranged_swing_look(b)) {
                            Some(shot) => all.push(packets::spell_damage_log(
                                target,
                                h,
                                shot,
                                points(*amount),
                                school_id(School::Physical),
                                *crit,
                            )),
                            None => {
                                let hand = by.and_then(|b| off_hand.iter().position(|&x| x == b));
                                let left = hand.map(|k| off_hand.swap_remove(k)).is_some();
                                all.push(packets::attacker_state_hand(h, target, points(*amount), *crit, left));
                            }
                        },
                        // thorns, auras striking back and milestone procs: logged as the hero's,
                        // under their own spell
                        (HitSource::Proc { spell, school, .. }, Some(h)) if *spell != 0 => {
                            all.push(packets::spell_damage_log(
                                target,
                                h,
                                *spell,
                                points(*amount),
                                school_id(*school),
                                *crit,
                            ));
                        }
                        // a ranged pet's shot is logged as the hero's; a melee pet swings
                        (HitSource::Pet(p), _) => match self.pet_attack(*p) {
                            Some((spell, school)) if spell != 0 => {
                                if let Some(h) = hero {
                                    all.push(packets::spell_damage_log(
                                        target,
                                        h,
                                        spell,
                                        points(*amount),
                                        school_id(school),
                                        *crit,
                                    ));
                                }
                            }
                            _ => {
                                if let Some(pet) = self.guid_of(Unit::Pet(*p)) {
                                    all.push(packets::attacker_state(
                                        pet,
                                        target,
                                        points(*amount),
                                        *crit,
                                    ));
                                }
                            }
                        },
                        // an enemy turned on the others swings at its target
                        (HitSource::Enemy(striker), _) => {
                            if let Some(e) = self.guid_of(Unit::Enemy(*striker)) {
                                let left = self.enemy_swings_left(*striker);
                                all.push(packets::attacker_state_hand(
                                    e,
                                    target,
                                    points(*amount),
                                    *crit,
                                    left,
                                ));
                            }
                        }
                        // an extra life's blast shows no number
                        _ => {}
                    }
                }
                Event::HeroHealed {
                    hero,
                    amount,
                    spell,
                } if *spell != 0 && *amount >= 1.0 => {
                    if let Some(h) = self.guid_of(Unit::Hero(*hero)) {
                        all.push(packets::spell_heal_log(
                            h,
                            h,
                            *spell,
                            points(*amount),
                            false,
                        ));
                    }
                }
                Event::HeroHit {
                    hero,
                    amount,
                    by: Some(enemy),
                    melee: true,
                    ..
                } => {
                    if let (Some(h), Some(e)) = (
                        self.guid_of(Unit::Hero(*hero)),
                        self.guid_of(Unit::Enemy(*enemy)),
                    ) {
                        // at least 1, whatever a shield took; a dual wielder's hands in turn
                        let left = self.enemy_swings_left(*enemy);
                        all.push(packets::attacker_state_hand(
                            e,
                            h,
                            points(amount.max(1.0)),
                            false,
                            left,
                        ));
                    }
                }
                Event::EnemyCastStarted {
                    id,
                    spell,
                    seconds,
                    hero,
                } => {
                    if let (Some(e), Some(h)) = (
                        self.guid_of(Unit::Enemy(*id)),
                        self.guid_of(Unit::Hero(*hero)),
                    ) {
                        all.push(packets::spell_start(
                            e,
                            *spell,
                            (seconds * 1000.0) as u32,
                            SpellTarget::Unit(h),
                        ));
                    }
                }
                Event::AreaShown {
                    id,
                    caster,
                    spell,
                    at,
                    radius,
                    until,
                } => {
                    if let Some(c) = self.guid_of(*caster) {
                        all.push(self.ground_look(*id, c, *spell, *at, *radius, *until));
                    }
                }
                Event::AreaGone { id } => all.extend(self.ground_look_gone(*id)),
                Event::AreaChanged { id, at, radius } => {
                    all.extend(self.ground_look_changed(*id, *at, *radius))
                }
                Event::SpellCooldown {
                    hero,
                    spell,
                    seconds,
                } => {
                    if let (Some(h), Some((v, _))) =
                        (self.guid_of(Unit::Hero(*hero)), self.ability_look(spell))
                    {
                        own.extend(bar_sweep(h, v, *seconds).into_iter().map(|p| (*hero, p)));
                    }
                }
                Event::SpellLearned { hero, spell } => {
                    if let Some((v, _)) = self.ability_look(spell) {
                        own.push((*hero, packets::learned_spell(v)));
                    }
                    if let Some(h) = self.run.heroes.iter().find(|h| h.id == *hero) {
                        own.push((*hero, self.action_bar(h)));
                    }
                }
                Event::RunStarted => {
                    for h in &self.run.heroes {
                        own.push((h.id, self.action_bar(h)));
                    }
                }
                Event::Yell { id, text } => {
                    if let Some(e) = self.guid_of(Unit::Enemy(*id)) {
                        // a boss played by a hand-made character yells its own line
                        let own = self.run.enemies.iter().position(|x| x.id == *id).and_then(|i| self.player_shape(i)?.yell);
                        all.push(packets::monster_yell(
                            e,
                            &self.name_of(Unit::Enemy(*id)),
                            own.as_deref().unwrap_or(text),
                        ));
                    }
                }
                Event::Emote { from, text } => {
                    let name = match from {
                        Speaker::Unit(u) => self.name_of(*u),
                        Speaker::Named(n) => n.clone(),
                    };
                    all.push(packets::raid_boss_emote(&name, text));
                }
                // Shown on the HUD: a client with a HUD of its own gets them as
                // its toasts, the others in the chat frame.
                Event::Announce(text) => own.extend(self.announce(text)),
                Event::RaidWarning(text) => all.push(packets::raid_warning(text)),
                Event::DoorOpened { door } | Event::DoorShut { door } => {
                    if let Some(open) = self.door_state(*door) {
                        all.push(crate::update::update_object(&[open]));
                    }
                }
                Event::Sound(sound) => all.push(packets::play_sound(*sound)),
                Event::Notice { hero, text } => own.push((*hero, packets::notification(text))),
                Event::Told { hero, text } => own.push((*hero, packets::system_message(text))),
                Event::HeroTeleported {
                    hero, to, height, ..
                } => {
                    if let Some(p) = self.players.iter().find(|p| p.hero == *hero) {
                        self.next_teleport += 1;
                        let facing = self
                            .run
                            .heroes
                            .iter()
                            .find(|h| h.id == *hero)
                            .map_or(0.0, |h| h.facing);
                        own.push((
                            *hero,
                            packets::teleport(
                                p.guid(),
                                self.next_teleport,
                                [to.0, to.1, *height],
                                facing,
                            ),
                        ));
                    }
                }
                // the bosses' building blocks
                ev @ (Event::TelegraphShown { .. }
                | Event::BossKneels { .. }
                | Event::BossChanged { .. }
                | Event::PauseBegan { .. }
                | Event::Warning { .. }
                | Event::BarStarted { .. }
                | Event::BarsCleared { .. }
                | Event::CounterShown { .. }) => self.present_boss_event(ev, &mut all, &mut own),
                Event::ImpactAtSpot { caster, spell, at, linger } => {
                    self.impact_at_spot(*caster, *spell, *at, *linger, &mut all)
                }
                _ => {}
            }
        }
        // the run's numbers, when they changed
        for h in &self.run.heroes {
            // what the class wears (worn gear changes only before a run, so this is what the run
            // brought in)
            let class = &self.run.content.classes[h.class].id;
            let worn = self.saves.class_progress(&self.run.content, class).worn;
            let line = stats_line(h, &gear_adds(&self.run.content, class, &worn));
            if self.stats_sent.get(&h.id) != Some(&line) {
                if let Some(guid) = self.guid_of(Unit::Hero(h.id)) {
                    own.push((h.id, packets::addon_message(guid, STATS_PREFIX, &line)));
                }
                self.stats_sent.insert(h.id, line);
            }
        }
        (all, own)
    }

    /// The heroes' buffs that changed this tick: their aura fields (for every client) and the
    /// timers of new or renewed ones (for the hero's own client); with `retell`, every buff's
    /// timer.
    pub(super) fn auras(
        &mut self,
        objects: &mut Vec<crate::update::ObjectUpdate>,
        own: &mut Vec<(u32, Packet)>,
        retell: bool,
    ) {
        let now = self.run.now;
        for h in &self.run.heroes {
            let mut auras = hero_auras(h, now);
            auras.extend(self.stack_auras(h.id));
            auras.extend(worn_looks(&h.kit.worn_looks, now));
            auras.truncate(BUFF_SLOTS);
            let before = self.auras_shown.get(&h.id).cloned().unwrap_or_default();
            if auras == before && !retell {
                continue;
            }
            let Some(guid) = self.guid_of(Unit::Hero(h.id)) else {
                continue;
            };
            if auras != before {
                objects.push(crate::update::ObjectUpdate::Values {
                    guid,
                    fields: aura_fields(&auras, before.len(), 0),
                });
            }
            for (slot, a) in auras.iter().enumerate() {
                if retell || before.get(slot) != Some(a) {
                    own.push((
                        h.id,
                        packets::aura_duration(
                            slot as u8,
                            ((a.until - now).max(0.0) * 1000.0) as u32,
                        ),
                    ));
                }
            }
            self.auras_shown.insert(h.id, auras);
        }
        // the stacks a boss left on a hero, as its debuffs
        for h in &self.run.heroes {
            let debuffs: Vec<Aura> = h
                .afflictions
                .iter()
                .take(DEBUFF_SLOTS)
                .map(|a| Aura { spell: a.spec.look_spell, stacks: a.stacks, until: a.until })
                .collect();
            let before = self.hero_debuffs_shown.get(&h.id).cloned().unwrap_or_default();
            if debuffs == before {
                continue;
            }
            let Some(guid) = self.guid_of(Unit::Hero(h.id)) else {
                continue;
            };
            objects.push(crate::update::ObjectUpdate::Values {
                guid,
                fields: aura_fields(&debuffs, before.len(), BUFF_SLOTS),
            });
            for (slot, a) in debuffs.iter().enumerate() {
                if before.get(slot) != Some(a) {
                    own.push((
                        h.id,
                        packets::aura_duration(
                            (BUFF_SLOTS + slot) as u8,
                            ((a.until - now).max(0.0) * 1000.0) as u32,
                        ),
                    ));
                }
            }
            self.hero_debuffs_shown.insert(h.id, debuffs);
        }
        // the enemies' effects over time
        for e in &self.run.enemies {
            let mut debuffs = enemy_debuffs(&self.run.content, e);
            // Lightning Rod's mark crackles on the enemy while it lasts
            if e.kit.marked_until > now && debuffs.len() < DEBUFF_SLOTS {
                debuffs.push(Aura {
                    spell: super::starting_looks::extras::LIGHTNING_ROD_MARK,
                    stacks: 1,
                    until: 0.0,
                });
            }
            // a legendary's mark on an enemy that struck the hero, while it lasts
            if let Some((_, until, look)) = e.kit.attacker_mark {
                if look != 0 && until > now && debuffs.len() < DEBUFF_SLOTS {
                    debuffs.push(Aura {
                        spell: look,
                        stacks: 1,
                        until: 0.0,
                    });
                }
            }
            // a legendary moment's look the enemy wears while it lasts
            for a in worn_looks(&e.kit.worn_looks, now) {
                if debuffs.len() < DEBUFF_SLOTS && !debuffs.iter().any(|d| d.spell == a.spell) {
                    debuffs.push(Aura { until: 0.0, ..a });
                }
            }
            // Starshards' stars fall on the enemy while its novas go on striking it
            if self.starshards_until.get(&e.id).is_some_and(|t| *t > now) && debuffs.len() < DEBUFF_SLOTS {
                debuffs.push(Aura {
                    spell: super::starting_looks::extras::STARSHARDS,
                    stacks: 1,
                    until: 0.0,
                });
            }
            let Some(s) = self.shown.get_mut(&e.id) else {
                continue;
            };
            if s.debuffs != debuffs {
                let fields = aura_fields(&debuffs, s.debuffs.len(), BUFF_SLOTS);
                objects.push(crate::update::ObjectUpdate::Values {
                    guid: s.guid,
                    fields,
                });
                s.debuffs = debuffs;
            }
        }
    }

    fn spell_target(&self, aim: Aim) -> Option<SpellTarget> {
        Some(match aim {
            Aim::Unit(u) => SpellTarget::Unit(self.guid_of(u)?),
            Aim::Ground(at) => SpellTarget::Ground([at.0, at.1, self.ground_z(at)]),
            Aim::Caster => SpellTarget::Caster,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use benilla_protocol::messages::parse_server;
    use benilla_protocol::{decode, SessionEvent};
    use esurv_sim::game::run::{Buff, BuffKind, Flat, HeroSetup, Run};

    use super::*;
    use crate::update::{update_object, ObjectUpdate};

    fn hero_with(buffs: Vec<Buff>) -> Hero {
        let content = Arc::new(
            esurv_content::Content::load(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../content"
            ))
            .unwrap(),
        );
        let setup = HeroSetup {
            class: "mage".into(),
            race: "human".into(),
            ..Default::default()
        };
        let mut run = Run::new(
            content,
            "elwynn",
            Default::default(),
            &[setup],
            Box::new(Flat),
            1,
        )
        .unwrap();
        run.heroes[0].buffs = buffs;
        run.heroes.remove(0)
    }

    #[test]
    fn healing_charges_show_their_charges_left_as_stacks() {
        let mut hero = hero_with(vec![Buff {
            kind: BuffKind::HealingCharges,
            value: 0.04,
            visual: 23881,
            until: 9.0,
        }]);
        hero.healing_charges = 3;
        let auras = hero_auras(&hero, 1.0);
        assert_eq!(
            auras,
            [Aura {
                spell: 23881,
                stacks: 3,
                until: 9.0
            }]
        );
    }

    #[test]
    fn buffs_with_one_look_stack_in_one_slot_and_read_back() {
        let b = |visual, until| Buff {
            kind: BuffKind::Damage,
            value: 0.1,
            visual,
            until,
        };
        let hero = hero_with(vec![
            b(23768, 5.0),
            b(1856, 3.0),
            b(23768, 8.0),
            b(0, 9.0),
            b(17, 0.5),
        ]);
        let auras = hero_auras(&hero, 1.0);
        assert_eq!(
            auras,
            [
                Aura {
                    spell: 23768,
                    stacks: 2,
                    until: 8.0
                },
                Aura {
                    spell: 1856,
                    stacks: 1,
                    until: 3.0
                }
            ],
            "no look and ended buffs are left out"
        );

        let p = update_object(&[ObjectUpdate::Values {
            guid: 7,
            fields: aura_fields(&auras, 3, 0),
        }]);
        let Some(SessionEvent::ObjectValues { fields, .. }) =
            decode(parse_server(p.opcode, &p.body).unwrap())
                .into_iter()
                .next()
        else {
            panic!()
        };
        let slots: Vec<(u8, u32, u8)> = fields
            .unit_auras()
            .map(|a| (a.slot, a.spell_id, a.stacks))
            .collect();
        assert_eq!(slots, [(0, 23768, 2), (1, 1856, 1)]);
        assert!(matches!(
            parse_server(
                packets::aura_duration(1, 2000).opcode,
                &packets::aura_duration(1, 2000).body
            )
            .unwrap(),
            benilla_protocol::ServerPacket::UpdateAuraDuration {
                slot: 1,
                remaining_ms: 2000
            }
        ));
    }

    #[test]
    fn effects_over_time_show_as_debuffs_after_the_buff_slots() {
        let debuffs = [
            Aura {
                spell: 172,
                stacks: 3,
                until: 0.0,
            },
            Aura {
                spell: 589,
                stacks: 1,
                until: 0.0,
            },
        ];
        let p = update_object(&[ObjectUpdate::Values {
            guid: 9,
            fields: aura_fields(&debuffs, 0, BUFF_SLOTS),
        }]);
        let Some(SessionEvent::ObjectValues { fields, .. }) =
            decode(parse_server(p.opcode, &p.body).unwrap())
                .into_iter()
                .next()
        else {
            panic!()
        };
        let slots: Vec<(u8, u32, u8, bool)> = fields
            .unit_auras()
            .map(|a| (a.slot, a.spell_id, a.stacks, a.is_helpful()))
            .collect();
        assert_eq!(slots, [(32, 172, 3, false), (33, 589, 1, false)]);
        // and gone again
        let p = update_object(&[ObjectUpdate::Values {
            guid: 9,
            fields: aura_fields(&[], 2, BUFF_SLOTS),
        }]);
        let Some(SessionEvent::ObjectValues {
            fields: cleared, ..
        }) = decode(parse_server(p.opcode, &p.body).unwrap())
            .into_iter()
            .next()
        else {
            panic!()
        };
        let mut all = fields;
        all.merge(cleared);
        assert_eq!(all.unit_auras().count(), 0);
    }

    #[test]
    fn the_numbers_line_has_every_key_with_its_decimals() {
        let line = stats_line(&hero_with(vec![]), &GearAdds::default());
        let keys: Vec<&str> = line
            .split(';')
            .map(|kv| kv.split('=').next().unwrap())
            .collect();
        assert_eq!(
            keys,
            [
                "health", "damage", "cooldown", "crit", "crit_damage", "reduction", "regen", "area", "targets",
                "double_cast", "speed", "pickup", "experience", "luck", "gold", "heal_per_kill", "heal_shield",
                "gear_health", "gear_damage", "gear_cooldown", "gear_crit", "gear_crit_damage", "gear_reduction",
                "gear_regen", "gear_area", "gear_targets", "gear_double_cast", "gear_speed", "gear_pickup",
                "gear_gold", "gear_luck", "gear_heal_per_kill", "gear_heal_shield"
            ]
        );
        let value = |k: &str| {
            line.split(';')
                .find_map(|kv| kv.strip_prefix(&format!("{k}=")))
                .unwrap()
                .to_string()
        };
        assert_eq!(value("damage").split('.').nth(1).map(str::len), Some(3));
        assert_eq!(value("crit_damage").split('.').nth(1).map(str::len), Some(2));
        assert_eq!(value("pickup").split('.').nth(1).map(str::len), Some(1));
        assert!(!value("targets").contains('.'));
    }

    #[test]
    fn worn_gear_adds_its_pieces_stats_to_the_g_keys() {
        use esurv_sim::game::gear::piece_modifiers;
        use esurv_sim::game::rewards::gear_item_entry;
        let content = esurv_content::Content::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../content"
        ))
        .unwrap();
        let mage = content.class("mage").unwrap().wow_class_id;
        let warrior = content.class("warrior").unwrap().wow_class_id;
        let set = content.gear_set("mage").unwrap();
        // every armour piece from the last zone, and a warrior's helm the mage cannot wear
        let last = content.zones.len() as u32 - 1;
        let worn: Vec<u32> = (0..8).map(|slot| gear_item_entry(mage, slot, last)).collect();
        let adds = gear_adds(&content, "mage", &worn);
        let mut damage = 0.0;
        let mut health = 0.0;
        for slot in 0..8 {
            for m in piece_modifiers(&content, set, slot, last).unwrap() {
                match m.stat {
                    esurv_content::Stat::Damage => damage += m.value,
                    esurv_content::Stat::FlatMaxHealth => health += m.value,
                    _ => {}
                }
            }
        }
        assert!(
            damage > 0.0 && health > 0.0,
            "the last zone's armour adds damage and health"
        );
        assert!((adds.damage - damage).abs() < 1e-5);
        assert!((adds.health - health).abs() < 1e-3);
        let with_other = [worn.clone(), vec![gear_item_entry(warrior, 0, last), 7]].concat();
        assert_eq!(gear_adds(&content, "mage", &with_other), adds);
        assert_eq!(gear_adds(&content, "mage", &[]), GearAdds::default());
        // the line carries it
        let line = stats_line(&hero_with(vec![]), &adds);
        assert!(line.contains(&format!("gear_health={};", health.round())), "{line}");
        assert!(line.contains(&format!("gear_damage={damage:.3};")), "{line}");
    }
}
