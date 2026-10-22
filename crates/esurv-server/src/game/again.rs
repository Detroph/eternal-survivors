//! Again: a fresh run in place of one that ended, and Reset: the same in the middle of a run
//! (the Escape menu). The run is paid out first, everything it put on the field goes from every
//! client, and a new run in the same zone with the same run options waits for its first move:
//! each hero back at the zone's spawn with its class's talents and worn gear, full health, level
//! 1, no gold, its starting spells and their action bar, free to move.

use esurv_sim::game::run::{Flat, HeroSetup, Phase, Run};

use super::{hero_setup, run_seed, Game, HeroShown};
use crate::packets;
use crate::update::update_object;
use crate::wire::Packet;

/// Reports from a hero's client to ignore until it stands where a fresh run put it (its moves
/// from before the run started over may still be on their way): as for a hero sent back into the
/// arena.
const STALE_MOVES: u8 = 12;

impl Game {
    /// `hero` asks for a fresh run. Only once the run is over (a fall or a victory), and only from
    /// a hero still in it; `None` refuses. The whole run starts over, for every hero in it; one who
    /// left stays out. Each packet is for one hero's client.
    pub(super) fn run_again(&mut self, hero: u32) -> Option<Vec<(u32, Packet)>> {
        if !self.run.is_over() || !self.still_in(hero) {
            return None;
        }
        self.start_over()
    }

    /// `hero` asks to reset the run (the Escape menu's Reset): the run
    /// ends here for it, paid out as a run left, and a fresh one starts as [`Game::run_again`]
    /// starts it. Unlike `again` it may come while the run runs or is paused, not only once it is
    /// over; never before the run has begun (there is nothing to reset), and never from a hero
    /// who left. A fresh run starts over for every hero in it, so while another hero is still in
    /// a run that is not over (playing, or fallen and watching), a reset is refused: it would end
    /// that hero's run too. That hero may still end its own run (`leave`).
    pub(super) fn reset_run(&mut self, hero: u32) -> Option<Vec<(u32, Packet)>> {
        if !self.still_in(hero) || self.run.phase == Phase::Waiting {
            return None;
        }
        if !self.run.is_over() {
            let others_in = self
                .players
                .iter()
                .any(|p| p.hero != hero && !self.left.contains(&p.hero));
            if others_in {
                return None;
            }
            // out of the run as if it fell: its time ends here, as when it leaves
            self.run.leave(hero);
        }
        self.start_over()
    }

    /// Whether `hero` plays in this run and has not left it.
    fn still_in(&self, hero: u32) -> bool {
        self.players.iter().any(|p| p.hero == hero) && !self.left.contains(&hero)
    }

    /// Pays out every hero still in the run, then puts a fresh run in its place for every client.
    fn start_over(&mut self) -> Option<Vec<(u32, Packet)>> {
        let mut told = Vec::new();
        // what the old run earned since it last paid
        let staying: Vec<u32> = self
            .players
            .iter()
            .map(|p| p.hero)
            .filter(|h| !self.left.contains(h))
            .collect();
        for &h in &staying {
            told.extend(self.pay_out(h));
        }
        let fresh = self.fresh_run()?;
        // the old run's field goes from every client
        let mut cleared: Vec<Packet> = self
            .shown
            .drain()
            .map(|(_, s)| packets::destroy_object(s.guid))
            .collect();
        cleared.extend(
            self.totems_shown
                .drain()
                .map(|(_, guid)| packets::destroy_object(guid)),
        );
        cleared.extend(
            self.clones
                .drain()
                .filter(|(_, c)| c.created)
                .map(|(_, c)| packets::destroy_object(c.guid)),
        );
        self.later.clear();
        self.out_of_form.clear();
        // the fresh run tells its bars and stacks anew
        for shown in self.options_shown.values_mut() {
            shown.stacks.clear();
        }
        cleared.extend(self.clear_pickups());
        cleared.extend(self.clear_ground_looks());
        cleared.extend(self.forget_boss_looks());
        self.forget_indicators();
        cleared.extend(self.forget_bunnies());
        self.hero_debuffs_shown.clear();
        self.run = fresh;
        // the doors stand shut again
        let doors: Vec<_> = (0..self.run.doors_open.len()).filter_map(|d| self.door_state(d)).collect();
        if !doors.is_empty() {
            cleared.push(crate::update::update_object(&doors));
        }
        self.moving_shown = false;
        // each hero as it starts: everyone sees its health and level; its own client gets it back
        // at the spawn, let go, its cooldowns over, its spells and action bar
        let mut heroes = Vec::new();
        let mut own = Vec::new();
        let was_rooted = std::mem::replace(&mut self.rooted, false);
        if was_rooted {
            self.next_root = self.next_root.wrapping_add(1);
        }
        for h in &self.run.heroes {
            let Some(p) = self.players.iter().find(|p| p.hero == h.id) else {
                continue;
            };
            let shown = HeroShown::of(h, &self.run.content.balance.experience);
            self.heroes_shown.insert(h.id, shown);
            heroes.push(shown.values(p.guid()));
            if self.left.contains(&h.id) {
                continue;
            }
            if was_rooted {
                own.push((
                    h.id,
                    packets::force_move_root(p.guid(), self.next_root, false),
                ));
            }
            self.next_teleport += 1;
            own.push((
                h.id,
                packets::teleport(
                    p.guid(),
                    self.next_teleport,
                    [h.pos.0, h.pos.1, h.height],
                    h.facing,
                ),
            ));
            own.push((h.id, packets::cooldowns_cleared(p.guid())));
            own.push((h.id, packets::initial_spells(&self.spellbook(h))));
            own.push((h.id, self.action_bar(h)));
        }
        if !heroes.is_empty() {
            cleared.push(update_object(&heroes));
        }
        for p in &self.players {
            told.extend(cleared.iter().map(|pk| (p.hero, pk.clone())));
        }
        told.extend(own);
        told.extend(self.screens_start_over());
        if self.starts_at_once {
            self.run.start();
        }
        Some(told)
    }

    /// A new run like the one that ended: the same zone, the run options as saved, its heroes in
    /// the same order (so each keeps its id) with what their classes have earned by now. A hero
    /// who left is a stand-in of its class and race, out of the run at once.
    fn fresh_run(&mut self) -> Option<Run> {
        let content = self.run.content.clone();
        let setups: Vec<HeroSetup> = self
            .run
            .heroes
            .iter()
            .map(|h| {
                let playing = self
                    .players
                    .iter()
                    .find(|p| p.hero == h.id && !self.left.contains(&h.id));
                match playing {
                    Some(p) => hero_setup(
                        &content,
                        &p.saved,
                        &self.saves.class_progress(&content, &p.saved.class),
                    ),
                    None => HeroSetup {
                        class: content.classes[h.class].id.clone(),
                        race: content.races[h.race].id.clone(),
                        ..Default::default()
                    },
                }
            })
            .collect();
        let ids: Vec<u32> = self.run.heroes.iter().map(|h| h.id).collect();
        let zone = content.zones[self.run.zone].id.clone();
        let options = self.saves.run_options().run_options(&content.run_options);
        let ground = std::mem::replace(&mut self.run.world, Box::new(Flat));
        let mut fresh = Run::new(content, &zone, options, &setups, ground, run_seed())?;
        let fresh_ids: Vec<u32> = fresh.heroes.iter().map(|h| h.id).collect();
        if fresh_ids != ids {
            // never expected: the old run keeps its ground and nothing changes
            self.run.world = std::mem::replace(&mut fresh.world, Box::new(Flat));
            return None;
        }
        for h in &mut fresh.heroes {
            h.pending_teleport = Some((h.pos, STALE_MOVES));
        }
        for &id in &ids {
            if self.left.contains(&id) || !self.players.iter().any(|p| p.hero == id) {
                fresh.leave(id);
            }
        }
        // what leaving put in the new run's log is not news to anyone
        fresh.drain_events();
        Some(fresh)
    }
}
