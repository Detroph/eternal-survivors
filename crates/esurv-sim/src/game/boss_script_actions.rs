//! The script actions the later zones' bosses need (docs/game/bosses.md,
//! "Building blocks"): actions some seconds later, acting as another boss, calling a boss in and
//! reviving a fallen one, heals with their beams, casting a building block at once, a form's own
//! speed and the end of its zones, empowering adds, and Blood Siphon; with the triggers of an add's
//! death and a hero's fall. Heals and empowering act on the boss's own side; Blood Siphon is the
//! script that drains the horde as well as the hero.

use esurv_content::{BossAbility, HealOn, ScriptAction, ScriptTrigger, School};

use super::casting::HitSource;
use super::enemy_ai::{attack_damage, Vec2};
use super::run::{Aim, EnemyKind, EnemyUnit, Event, Run, Unit};

/// Script actions waiting to run.
#[derive(Debug, Clone, PartialEq)]
pub struct LaterActions {
    /// Seconds left (counted through a scripted pause too).
    pub left: f32,
    pub source: Option<u32>,
    pub at: Vec2,
    pub run: Vec<ScriptAction>,
}

/// A channelled beam an enemy shows to a unit until `until` (a heal, a drain).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beam {
    pub caster: u32,
    pub target: Unit,
    pub spell: u32,
    pub until: f32,
}

/// What these actions keep on the run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScriptExtras {
    pub later: Vec<LaterActions>,
    pub beams: Vec<Beam>,
    /// Where each fallen boss (by its place in the zone's list) last fell.
    pub fallen: Vec<(usize, Vec2)>,
    /// Bosses brought back by `Revive` (unit ids): they call no one.
    pub revived: Vec<u32>,
    /// The boss whose mark or zone is landing now, so a hero falling to it is put down to it.
    pub blame: Option<u32>,
    /// The boss (or enemy) the last blow on a hero came from.
    pub last_blow: Option<u32>,
    /// Packs on their way in from a side, once their lane's mark has stood: when, which roster
    /// enemy, how many, where.
    pub packs: Vec<(f32, usize, u32, Vec2)>,
    /// The speed each boss that changed its speed came with (unit id, yards a second).
    pub first_speeds: Vec<(u32, f32)>,
}

/// A side's lane is marked this long before its pack runs in, and this long and wide (our edge
/// arrows).
pub const LANE_WARNING: f32 = 2.0;
pub const LANE: (f32, f32) = (14.0, 4.0);
/// Packs come from sides at least this far apart (radians) round the hero, when the ground allows.
const SIDES_APART: f32 = 1.0;

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Run {
    /// The living boss unit named `name` (its index in the enemies).
    pub(crate) fn boss_unit(&self, name: &str) -> Option<usize> {
        let zone = self.zone();
        self.enemies.iter().position(|e| matches!(e.kind, EnemyKind::Boss(b) if zone.bosses[b].name == name))
    }

    /// The zone's boss named `name`, by its place in the list.
    fn boss_index(&self, name: &str) -> Option<usize> {
        self.zone().bosses.iter().position(|b| b.name == name)
    }

    /// Runs one of the later zones' actions for boss `source` at `at` (`here`: at that spot, not
    /// the boss's, as an object's pulse or an add's death does).
    pub(crate) fn run_more_action(&mut self, source: Option<u32>, at: Vec2, here: bool, action: &ScriptAction) {
        let now = self.now;
        let k = source.and_then(|s| self.enemies.iter().position(|e| e.id == s));
        match action {
            ScriptAction::Later { seconds, run } => self.extras.later.push(LaterActions { left: *seconds, source, at, run: run.clone() }),
            ScriptAction::On { boss, run } => {
                if let Some(o) = self.boss_unit(boss) {
                    let (id, p) = (self.enemies[o].id, self.enemies[o].position(now));
                    self.run_actions(Some(id), p, run);
                }
            }
            ScriptAction::Call { boss } => {
                let revived = source.is_some_and(|s| self.extras.revived.contains(&s));
                if let (false, None, Some(b)) = (revived, self.boss_unit(boss), self.boss_index(boss)) {
                    self.spawn_boss(b);
                }
            }
            ScriptAction::Revive { boss, health, look_spell } => self.revive_boss(boss, *health, *look_spell),
            ScriptAction::Unshield => {
                if let Some(k) = k {
                    let e = &mut self.enemies[k];
                    if let Some((_, look)) = e.mech.shield.take() {
                        e.kit.worn_looks.retain(|(l, _)| *l != look);
                        let id = e.id;
                        self.mortal_looks.retain(|(i, l, _)| !(*i == id && *l == look));
                    }
                }
            }
            ScriptAction::Heal { on, share, look_spell, beam_spell, beam_seconds, from } => {
                self.script_heal(k, at, here, on, *share, *look_spell, (*beam_spell, *beam_seconds), from.as_deref())
            }
            ScriptAction::Cast { resolved: Some(a), .. } => self.cast_now(k, at, here, a),
            ScriptAction::ClearZones => {
                if let Some(id) = source {
                    let gone: Vec<u32> = self.ground_zones.iter().filter(|z| z.source == id).map(|z| z.id).collect();
                    self.ground_zones.retain(|z| z.source != id);
                    for id in gone {
                        self.push(Event::AreaGone { id });
                    }
                }
            }
            ScriptAction::Wear { look_spell, seconds } => {
                if let Some(k) = k {
                    self.enemies[k].kit.worn_looks.push((*look_spell, now + seconds));
                }
            }
            ScriptAction::Speed { yards } => {
                if let Some(k) = k {
                    if let EnemyKind::Boss(b) = self.enemies[k].kind {
                        // the speed run option kept: its share of the speed it came with
                        let own = self.content.zones[self.zone].bosses[b].move_speed.max(0.1);
                        let id = self.enemies[k].id;
                        let first = match self.extras.first_speeds.iter().find(|(i, _)| *i == id) {
                            Some(&(_, s)) => s,
                            None => {
                                let s = self.enemies[k].base_speed;
                                self.extras.first_speeds.push((id, s));
                                s
                            }
                        };
                        self.enemies[k].base_speed = yards * first / own;
                    }
                }
            }
            ScriptAction::Bolster { enemy, damage, speed, look_spell } => {
                let zone = self.zone();
                let ids: Vec<u32> = match enemy {
                    Some(name) => self.enemies.iter().filter(|e| matches!(e.kind, EnemyKind::Roster(r) if zone.enemies[r].name == *name)).map(|e| e.id).collect(),
                    None => k.map(|k| self.enemies[k].id).into_iter().collect(),
                };
                for id in ids {
                    if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                        // an add wearing it already is passed by (it does not stack); the boss
                        // itself grows each time (a level up)
                        if enemy.is_some() && *look_spell != 0 && e.kit.worn_looks.iter().any(|(l, _)| l == look_spell) {
                            continue;
                        }
                        e.damage *= damage;
                        e.base_speed *= speed;
                        if *look_spell != 0 {
                            e.kit.worn_looks.retain(|(l, _)| l != look_spell);
                            e.kit.worn_looks.push((*look_spell, f32::INFINITY));
                            let me = Unit::Enemy(id);
                            self.push(Event::SpellGo { caster: me, spell: *look_spell, hits: vec![me], aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
                        }
                    }
                }
            }
            ScriptAction::Siphon { .. } => {
                if let Some(k) = k {
                    self.blood_siphon(k, action);
                }
            }
            _ => {}
        }
    }

    /// Script actions whose time has come run (each tick, a scripted pause's too).
    pub(crate) fn later_tick(&mut self, dt: f32) {
        let now = self.now;
        for l in &mut self.extras.later {
            l.left -= dt;
        }
        let (due, later): (Vec<LaterActions>, Vec<LaterActions>) = std::mem::take(&mut self.extras.later).into_iter().partition(|l| l.left <= 0.0);
        self.extras.later = later;
        for l in due {
            self.run_actions(l.source, l.at, &l.run);
        }
        // packs whose lanes stood long enough run in
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.extras.packs).into_iter().partition(|p| now >= p.0);
        self.extras.packs = later;
        for (_, index, count, at) in due {
            for i in 0..count {
                let a = i as f32 / count.max(1) as f32 * std::f32::consts::TAU;
                self.spawn_summoned(index, (at.0 + a.cos() * 1.5, at.1 + a.sin() * 1.5));
            }
        }
        // beams end with their time, or with their caster or target
        let alive = |r: &Run, u: Unit| match u {
            Unit::Enemy(id) => r.enemies.iter().any(|e| e.id == id),
            Unit::Hero(id) => r.heroes.iter().any(|h| h.id == id && h.alive),
            _ => false,
        };
        let beams = std::mem::take(&mut self.extras.beams);
        self.extras.beams = beams.into_iter().filter(|b| now < b.until && alive(self, Unit::Enemy(b.caster)) && alive(self, b.target)).collect();
    }

    /// `count` of roster enemy `name` in `packs` packs from far-apart sides of the edge, each
    /// side's lane marked towards the hero for `LANE_WARNING` seconds first.
    pub(crate) fn wave_from_sides(&mut self, source: Option<u32>, name: &str, count: u32, packs: u32) {
        let Some(index) = self.roster_index(name) else { return };
        let Some(h) = self.anchor_hero() else { return };
        let hero = self.heroes[h].pos;
        let ring = self.zone().enemy_spawn_ring;
        let packs = packs.max(1);
        let mut sides: Vec<Vec2> = Vec::new();
        for _ in 0..packs {
            let mut spot = self.spawn_point(h, ring);
            for _ in 0..8 {
                let face = super::telegraphs::facing(hero, spot);
                let apart = sides.iter().all(|s| {
                    let off = (super::telegraphs::facing(hero, *s) - face + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                    off.abs() >= SIDES_APART
                });
                if apart {
                    break;
                }
                spot = self.spawn_point(h, ring);
            }
            sides.push(spot);
        }
        let now = self.now;
        let caster = source.unwrap_or(0);
        let (warning_spell, fill_spell) = self.content.enemy_kinds.as_ref().map_or((0, 0), |k| (k.boss_abilities.marked.warning_spell, k.boss_abilities.marked.fill_spell));
        for (p, spot) in sides.into_iter().enumerate() {
            let n = count / packs + u32::from((p as u32) < count % packs);
            // the lane: a harmless mark from the side towards the hero
            let lane = super::telegraphs::Telegraph {
                id: 0,
                source: caster,
                shape: esurv_content::Shape::Line { length: LANE.0, width: LANE.1 },
                at: spot,
                facing: super::telegraphs::facing(spot, hero),
                lands_at: now + LANE_WARNING,
                damage: 0.0,
                impact_spell: 0,
                leaves: None,
                leaves_object: None,
                attack_damage: 0.0,
                missile_at: None,
                rush: false,
                from_boss: false,
                on_bunny: false,
                brings: None,
                sweep: None,
            };
            self.lay_mark(lane, warning_spell, fill_spell, LANE_WARNING);
            self.extras.packs.push((now + LANE_WARNING, index, n, spot));
        }
    }

    /// Enemy `caster` channels `spell` to `target` for `seconds` (one beam a caster).
    fn beam(&mut self, caster: u32, target: Unit, spell: u32, seconds: f32) {
        let until = self.now + seconds;
        self.extras.beams.retain(|b| b.caster != caster);
        self.extras.beams.push(Beam { caster, target, spell, until });
    }

    /// Enemy `id` heals `share` of its health (a look on it).
    fn heal_enemy(&mut self, id: u32, share: f32, look_spell: u32) {
        let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) else { return };
        e.health = (e.health + share * e.max_health).min(e.max_health);
        if look_spell != 0 {
            let me = Unit::Enemy(id);
            self.push(Event::SpellGo { caster: me, spell: look_spell, hits: vec![me], aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn script_heal(&mut self, k: Option<usize>, at: Vec2, here: bool, on: &HealOn, share: f32, look_spell: u32, beam: (u32, f32), from: Option<&str>) {
        let now = self.now;
        let zone = self.zone();
        // the healers: the adds named, the object (its spot) or the boss
        let healers: Vec<(Option<u32>, Vec2)> = match (from, here, k) {
            (Some(name), _, _) => self.enemies.iter().filter(|e| matches!(e.kind, EnemyKind::Roster(r) if zone.enemies[r].name == name)).map(|e| (Some(e.id), e.position(now))).collect(),
            (None, true, _) => vec![(None, at)],
            (None, false, Some(k)) => vec![(Some(self.enemies[k].id), self.enemies[k].position(now))],
            (None, false, None) => Vec::new(),
        };
        let me = k.map(|k| self.enemies[k].id);
        for (healer, spot) in healers {
            let targets: Vec<u32> = match on {
                HealOn::Myself => me.into_iter().collect(),
                HealOn::Near(r) => self.enemies.iter().filter(|e| Some(e.id) != healer && dist(e.position(now), spot) <= *r).map(|e| e.id).collect(),
                HealOn::Boss(name) => self.boss_unit(name).map(|o| self.enemies[o].id).into_iter().collect(),
            };
            for &t in &targets {
                self.heal_enemy(t, share, look_spell);
            }
            // one beam a healer: to the boss it heals, else the first it heals
            if let (Some(c), Some(&t), true) = (healer, targets.first(), beam.0 != 0) {
                if c != t {
                    self.beam(c, Unit::Enemy(t), beam.0, beam.1);
                }
            }
        }
    }

    /// Boss `k` casts building block `a` now: from `at` when `here` (an object's pulse).
    fn cast_now(&mut self, k: Option<usize>, at: Vec2, here: bool, a: &BossAbility) {
        let Some(k) = k else { return };
        let now = self.now;
        let Some(h) = self.nearest_hero(self.enemies[k].position(now)) else { return };
        let e = &self.enemies[k];
        let damage = attack_damage(e.damage, EnemyUnit::active(e.weaken, now), e.is_boss() && e.enraged);
        if super::telegraphs::is_building_block(&a.kind) {
            self.cast_block(k, h, &a.kind, damage, here.then_some(at));
        }
    }

    /// The zone's boss named `name` rises where it last fell, with `health` of its health.
    fn revive_boss(&mut self, name: &str, health: f32, look_spell: u32) {
        let Some(b) = self.boss_index(name) else { return };
        if self.boss_unit(name).is_some() {
            return;
        }
        let Some(&(_, spot)) = self.extras.fallen.iter().rev().find(|(i, _)| *i == b) else { return };
        self.spawn_boss_quietly(b, spot);
        let Some(e) = self.enemies.last_mut() else { return };
        e.health = (e.max_health * health).max(1.0);
        let id = e.id;
        self.extras.revived.push(id);
        if look_spell != 0 {
            let me = Unit::Enemy(id);
            self.push(Event::SpellGo { caster: me, spell: look_spell, hits: vec![me], aim: Aim::Caster, ammo: false, reach: 0.0, triggered: false });
        }
    }

    /// Blood Siphon (`ScriptAction::Siphon`) by boss `k`.
    fn blood_siphon(&mut self, k: usize, action: &ScriptAction) {
        let ScriptAction::Siphon { damage_mult, heal_share, poison_spell, poisoned_share, enemy_share, enemy_heal_share, beam_spell, poisoned_beam_spell, seconds } = *action else { return };
        let now = self.now;
        let e = &self.enemies[k];
        let (id, max) = (e.id, e.max_health);
        let attack = attack_damage(e.damage, EnemyUnit::active(e.weaken, now), e.enraged);
        let mut heal = 0.0;
        let mut hurt = 0.0;
        for h in 0..self.heroes.len() {
            if !self.heroes[h].alive {
                continue;
            }
            let hero = Unit::Hero(self.heroes[h].id);
            let poisoned = self.heroes[h].afflictions.iter().any(|a| a.spec.look_spell == poison_spell);
            if poisoned {
                // a poisoned hero's blood hurts the boss instead (as in Zul'Gurub)
                hurt += poisoned_share * max;
                self.beam(id, hero, poisoned_beam_spell, seconds);
            } else {
                self.damage_hero(h, damage_mult * attack, Some(id), false);
                heal += heal_share * max;
                self.beam(id, hero, beam_spell, seconds);
            }
        }
        // the horde is drained too
        let horde: Vec<(u32, f32)> = self.enemies.iter().filter(|e| !e.is_boss()).map(|e| (e.id, e.max_health)).collect();
        for (enemy, its_max) in horde {
            self.hit_enemy(None, enemy, enemy_share * its_max, false, HitSource::Proc { spell: beam_spell, school: School::Shadow, origin: super::casting::ProcOrigin::Other });
            heal += enemy_heal_share * max;
        }
        if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
            e.health = (e.health + heal).min(e.max_health);
        }
        if hurt > 0.0 {
            self.hit_enemy(None, id, hurt, false, HitSource::Proc { spell: poisoned_beam_spell, school: School::Shadow, origin: super::casting::ProcOrigin::Other });
        }
    }

    /// A roster enemy (`roster`) died at `at` (already off the field): the bosses' lines waiting
    /// for it run there, `LastAddDied` ones only when none of its name is left.
    pub(crate) fn add_died(&mut self, roster: usize, at: Vec2) {
        let content = self.content.clone();
        let zone = &content.zones[self.zone];
        let name = &zone.enemies[roster].name;
        let last = !self.enemies.iter().any(|e| matches!(e.kind, EnemyKind::Roster(r) if zone.enemies[r].name == *name));
        let bosses: Vec<(u32, usize)> = self.enemies.iter().filter_map(|e| if let EnemyKind::Boss(b) = e.kind { Some((e.id, b)) } else { None }).collect();
        for (id, b) in bosses {
            for (i, line) in zone.bosses[b].script.iter().enumerate() {
                let waits = match line.on {
                    ScriptTrigger::AddDied => true,
                    ScriptTrigger::LastAddDied => last,
                    _ => false,
                };
                if waits && line.add.as_ref() == Some(name) {
                    if let Some(w) = &line.warn {
                        self.announce_warning(w, 0);
                    }
                    self.run_actions_at(Some(id), at, &zone.bosses[b].script[i].run, true);
                }
            }
        }
    }

    /// A hero lost a life (or fell): the lines of the boss it fell to run.
    pub(crate) fn hero_fell(&mut self) {
        let Some(by) = self.extras.last_blow else { return };
        let Some(k) = self.enemies.iter().position(|e| e.id == by) else { return };
        let EnemyKind::Boss(b) = self.enemies[k].kind else { return };
        let content = self.content.clone();
        let at = self.enemies[k].position(self.now);
        for line in content.zones[self.zone].bosses[b].script.iter().filter(|l| l.on == ScriptTrigger::HeroFell) {
            if let Some(w) = &line.warn {
                self.announce_warning(w, 0);
            }
            self.run_actions(Some(by), at, &line.run);
        }
    }
}
