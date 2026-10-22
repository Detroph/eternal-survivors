//! The bosses' marks as the client draws them (docs/game/bosses.md,
//! "Building blocks"): Final Fantasy XIV's orange indicators laid flat on the ground, told to every
//! client with a HUD of its own over the game's lines (`ESURVGI`, `ESURVGX`). A mark winding up is
//! its shape, filling to its edge as it lands, and goes as it lands; a lasting zone (fire,
//! poison, a blizzard) and a sweeping beam keep their spell looks and get an orange outline too,
//! with chevrons the way a drifting zone or a beam moves. A client without a HUD (a stock 1.12
//! client) sees the marks as spell looks instead (`boss_looks`).

use esurv_content::Shape;
use esurv_protocol::{GroundIndicator, IndicatorShape, ToClient};
use esurv_sim::game::enemy_ai::Vec2;

use super::Game;
use crate::wire::Packet;

/// A moving indicator is told again once it strays this far (yards) from where the client
/// reckons it is, turns this far (radians) from its reckoned facing, or its speed changes this
/// much (yards a second).
const STRAY: f32 = 0.4;
const STRAY_TURN: f32 = 0.05;
const STRAY_SPEED: f32 = 0.3;

/// What an indicator stands for in the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum IndicatorOf {
    /// A mark winding up, by its id.
    Mark(u32),
    /// A lasting zone, by its id.
    Zone(u32),
    /// A beam sweeping, by its boss and when it ends (its bits).
    Beam(u32, u32),
}

/// An indicator as the clients were last told it.
pub(super) struct ShownIndicator {
    told: GroundIndicator,
    /// When it was told (the run's clock).
    told_at: f32,
    /// Where it stood on the last tick, and when, for its speed.
    last: (Vec2, f32),
}

/// The shape as the client draws it.
pub fn indicator_shape(shape: Shape) -> IndicatorShape {
    match shape {
        Shape::Circle { radius } => IndicatorShape::Circle { radius },
        Shape::Ring { inner, outer } => IndicatorShape::Ring { inner, outer },
        Shape::Cone { length, angle } => IndicatorShape::Cone { length, angle },
        Shape::Line { length, width } => IndicatorShape::Line { length, width },
    }
}

/// An indicator as it stands this tick, before it is numbered and placed on the ground.
struct Wanted {
    of: IndicatorOf,
    shape: IndicatorShape,
    at: Vec2,
    facing: f32,
    lands_at: Option<f32>,
    turn: f32,
    edge_only: bool,
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Game {
    /// Whether some player's client has no HUD of its own, so cannot draw indicators: the marks
    /// are laid as spell looks for it.
    pub(super) fn marks_as_looks(&self) -> bool {
        self.players.iter().any(|p| !self.told.get(&p.hero).is_some_and(|t| t.screens.hud))
    }

    /// The indicators as they stand this tick.
    fn wanted_indicators(&self) -> Vec<Wanted> {
        let mut out = Vec::new();
        for t in &self.run.telegraphs {
            out.push(Wanted { of: IndicatorOf::Mark(t.id), shape: indicator_shape(t.shape), at: t.at, facing: t.facing, lands_at: Some(t.lands_at), turn: 0.0, edge_only: false });
        }
        for z in &self.run.ground_zones {
            out.push(Wanted { of: IndicatorOf::Zone(z.id), shape: IndicatorShape::Circle { radius: z.spec.radius }, at: z.at, facing: 0.0, lands_at: None, turn: 0.0, edge_only: true });
        }
        let now = self.run.now;
        for b in &self.run.beams {
            let Some(origin) = self.run.enemies.iter().find(|e| e.id == b.source).map(|e| e.position(now)) else { continue };
            out.push(Wanted { of: IndicatorOf::Beam(b.source, b.until.to_bits()), shape: indicator_shape(b.shape), at: origin, facing: b.facing, lands_at: None, turn: b.turn, edge_only: true });
        }
        out
    }

    /// The indicators told to the clients with a HUD: new ones, ones that changed or strayed
    /// from where the clients reckon them, and the ones gone (a mark landed, a zone ended); all
    /// of them to a client that has not heard them yet, and again when the run stops or goes on.
    pub(super) fn mirror_indicators(&mut self) -> Vec<(u32, Packet)> {
        let now = self.run.now;
        let still = !(self.run.phase == esurv_sim::game::run::Phase::Running && !self.run.in_cutscene());
        let all_again = still != self.indicators_still;
        self.indicators_still = still;
        let mut messages: Vec<ToClient> = std::mem::take(&mut self.indicators_dropped).into_iter().map(|id| ToClient::GroundIndicatorGone { id }).collect();
        let wanted = self.wanted_indicators();
        let live: Vec<IndicatorOf> = wanted.iter().map(|w| w.of).collect();
        let gone: Vec<IndicatorOf> = self.indicators_shown.keys().copied().filter(|k| !live.contains(k)).collect();
        for k in gone {
            if let Some(s) = self.indicators_shown.remove(&k) {
                messages.push(ToClient::GroundIndicatorGone { id: s.told.id });
            }
        }
        for w in wanted {
            let lands_in = w.lands_at.map_or(0.0, |l| (l - now).max(0.0));
            match self.indicators_shown.get(&w.of) {
                None => {
                    let id = self.next_indicator;
                    self.next_indicator = self.next_indicator.wrapping_add(1);
                    let told = GroundIndicator {
                        id,
                        shape: w.shape,
                        at: [w.at.0, w.at.1, self.ground_z(w.at)],
                        facing: w.facing,
                        // a mark first seen the tick it was laid: its whole wind-up is ahead
                        wind_up: lands_in,
                        lands_in,
                        velocity: [0.0, 0.0],
                        turn: w.turn,
                        edge_only: w.edge_only,
                        still,
                    };
                    messages.push(ToClient::GroundIndicator(told.clone()));
                    self.indicators_shown.insert(w.of, ShownIndicator { told, told_at: now, last: (w.at, now) });
                }
                Some(s) => {
                    let (last, last_at) = s.last;
                    let dt = now - last_at;
                    let velocity = if dt > 1e-4 { [(w.at.0 - last.0) / dt, (w.at.1 - last.1) / dt] } else { s.told.velocity };
                    let since = if s.told.still { 0.0 } else { now - s.told_at };
                    let reckoned = (s.told.at[0] + s.told.velocity[0] * since, s.told.at[1] + s.told.velocity[1] * since);
                    let reckoned_facing = s.told.facing + s.told.turn * since;
                    let speed_change = ((velocity[0] - s.told.velocity[0]).powi(2) + (velocity[1] - s.told.velocity[1]).powi(2)).sqrt();
                    let again = all_again
                        || s.told.shape != w.shape
                        || dist(reckoned, w.at) > STRAY
                        || (reckoned_facing - w.facing).abs() > STRAY_TURN
                        || speed_change > STRAY_SPEED;
                    let wind_up = s.told.wind_up;
                    let id = s.told.id;
                    if let Some(s) = self.indicators_shown.get_mut(&w.of) {
                        s.last = (w.at, now);
                    }
                    if again {
                        let told = GroundIndicator {
                            id,
                            shape: w.shape,
                            at: [w.at.0, w.at.1, self.ground_z(w.at)],
                            facing: w.facing,
                            wind_up,
                            lands_in,
                            velocity: if still { [0.0, 0.0] } else { velocity },
                            turn: w.turn,
                            edge_only: w.edge_only,
                            still,
                        };
                        messages.push(ToClient::GroundIndicator(told.clone()));
                        if let Some(s) = self.indicators_shown.get_mut(&w.of) {
                            s.told = told;
                            s.told_at = now;
                        }
                    }
                }
            }
        }
        // a client with a HUD that has not heard the indicators yet hears all that stand
        let mut out = Vec::new();
        let huds: Vec<u32> = self.players.iter().map(|p| p.hero).filter(|h| self.told.get(h).is_some_and(|t| t.screens.hud)).collect();
        let fresh: Vec<u32> = huds.iter().copied().filter(|h| self.told.get(h).is_some_and(|t| !t.indicators)).collect();
        // as they stand now: what the others reckon since they were told
        let standing: Vec<ToClient> = self
            .indicators_shown
            .values()
            .map(|s| {
                let since = if s.told.still { 0.0 } else { now - s.told_at };
                let mut g = s.told.clone();
                g.at[0] += g.velocity[0] * since;
                g.at[1] += g.velocity[1] * since;
                g.facing += g.turn * since;
                g.lands_in = (g.lands_in - since).max(0.0);
                ToClient::GroundIndicator(g)
            })
            .collect();
        for h in &huds {
            let list = if fresh.contains(h) { standing.clone() } else { messages.clone() };
            out.extend(self.lines(list.into_iter().map(|m| (*h, m)).collect()));
        }
        for h in fresh {
            if let Some(t) = self.told.get_mut(&h) {
                t.indicators = true;
            }
        }
        out
    }

    /// The indicators shown are dropped (the field was cleared): the clients are told they went.
    pub(super) fn forget_indicators(&mut self) {
        let dropped: Vec<u32> = self.indicators_shown.drain().map(|(_, s)| s.told.id).collect();
        self.indicators_dropped.extend(dropped);
    }

    /// The indicators as last told, by what they stand for (for the tests).
    #[cfg(test)]
    pub(super) fn indicators_told(&self) -> std::collections::HashMap<IndicatorOf, GroundIndicator> {
        self.indicators_shown.iter().map(|(k, s)| (*k, s.told.clone())).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    use benilla_protocol::messages::parse_server;
    use benilla_protocol::ServerPacket;
    use esurv_content::LastingZone;
    use esurv_protocol::{Reader, Screens, ToServer};
    use esurv_sim::game::ground_zones::GroundZone;
    use esurv_sim::game::run::{Event, Phase};
    use esurv_sim::game::telegraphs::Telegraph;

    use super::*;
    use crate::game::Player;
    use crate::save::Saves;
    use crate::wire::PacketWriter;

    /// A run with one mage, its client (with a HUD of its own when `hud`) at the other end of a
    /// local socket, nothing else on the field.
    fn game(hud: bool) -> (Game, TcpStream) {
        let content = Arc::new(esurv_content::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
        let saves = Arc::new(Saves::in_memory());
        let saved = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let mut game = Game::new(content, None, 0, &saved, saves, inputs).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        let hero = game.hero_id(0);
        assert!(game.join(Player { hero, saved, writer: Arc::new(Mutex::new(PacketWriter::new(server))) }));
        game.ask(hero, ToServer::Hello { version: esurv_protocol::VERSION, screens: Screens { hud, ..Default::default() } });
        game.run.enemies.clear();
        game.run.spawn_rules.stopped = true;
        game.run.cutscene = None;
        game.run.phase = Phase::Running;
        game.run.drain_events();
        (game, client)
    }

    fn told(packets: &[(u32, Packet)]) -> Vec<ToClient> {
        let mut reader = Reader::<ToClient>::default();
        packets
            .iter()
            .filter_map(|(_, p)| match parse_server(p.opcode, &p.body) {
                Ok(ServerPacket::MessageChat(m)) if m.is_addon() => reader.read(&m.text),
                _ => None,
            })
            .collect()
    }

    fn mark(game: &Game, shape: Shape, wind_up: f32) -> Telegraph {
        let p = game.run.heroes[0].pos;
        Telegraph {
            id: 9001,
            source: 0,
            shape,
            at: (p.0 + 30.0, p.1),
            facing: 1.0,
            lands_at: game.run.now + wind_up,
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
        }
    }

    /// A mark winding up is told to the client with a HUD as an indicator of its shape, place,
    /// facing and wind-up, and told gone the tick it lands; no spell look is laid for it.
    #[test]
    fn a_mark_is_an_indicator_until_it_lands() {
        let (mut game, _client) = game(true);
        let t = mark(&game, Shape::Cone { length: 20.0, angle: 60.0 }, 0.45);
        game.run.telegraphs.push(t.clone());
        let mut looks = (Vec::new(), Vec::new());
        game.present_boss_event(&Event::TelegraphShown { id: t.id, shape: t.shape, at: t.at, facing: t.facing, warning_spell: 1, fill_spell: 2, wind_up: 0.45 }, &mut looks.0, &mut looks.1);
        assert!(looks.0.is_empty() && game.marks_shown.is_empty(), "no spell look for a client that draws indicators");
        let first = told(&game.mirror_indicators());
        let [ToClient::GroundIndicator(g)] = &first[..] else { panic!("{first:?}") };
        assert_eq!(g.shape, IndicatorShape::Cone { length: 20.0, angle: 60.0 });
        assert_eq!((g.at[0], g.at[1], g.facing), (t.at.0, t.at.1, 1.0));
        assert!((g.wind_up - 0.45).abs() < 1e-4 && (g.lands_in - 0.45).abs() < 1e-4 && !g.edge_only && !g.still);
        let id = g.id;
        // nothing new while it winds up in place
        game.run.tick(0.1);
        game.run.drain_events();
        assert!(told(&game.mirror_indicators()).is_empty());
        let mut gone = Vec::new();
        for _ in 0..5 {
            game.run.tick(0.1);
            game.run.drain_events();
            gone.extend(told(&game.mirror_indicators()));
        }
        assert!(game.run.telegraphs.is_empty(), "landed");
        assert_eq!(gone, vec![ToClient::GroundIndicatorGone { id }]);
        assert!(game.indicators_told().is_empty());
    }

    /// A client without a HUD gets the mark as spell looks, and no indicator lines.
    #[test]
    fn a_client_without_a_hud_sees_the_mark_as_spell_looks() {
        let (mut game, _client) = game(false);
        let t = mark(&game, Shape::Circle { radius: 8.0 }, 2.0);
        game.run.telegraphs.push(t.clone());
        let mut looks = (Vec::new(), Vec::new());
        game.present_boss_event(&Event::TelegraphShown { id: t.id, shape: t.shape, at: t.at, facing: t.facing, warning_spell: 1, fill_spell: 2, wind_up: 2.0 }, &mut looks.0, &mut looks.1);
        assert!(!looks.0.is_empty() && game.marks_shown.contains_key(&t.id));
        assert!(told(&game.mirror_indicators()).is_empty());
    }

    /// A lasting zone is an outline; drifting after the hero, it is told with its speed (its
    /// chevrons' way), and told again when the run stops (held) and goes on.
    #[test]
    fn a_drifting_zone_is_an_outline_told_with_its_speed() {
        let (mut game, _client) = game(true);
        let p = game.run.heroes[0].pos;
        let spec = LastingZone { look_spell: 0, radius: 5.0, seconds: 30.0, damage_mult: 0.0, every: 1.0, drift: 2.0, follows: false, affliction: None };
        game.run.ground_zones.push(GroundZone { id: 77, source: 0, spec, at: (p.0 + 20.0, p.1), shown_at: (p.0 + 20.0, p.1), until: Some(game.run.now + 30.0), next_tick: game.run.now + 1.0, damage: 0.0, attack_damage: 0.0 });
        let first = told(&game.mirror_indicators());
        let [ToClient::GroundIndicator(g)] = &first[..] else { panic!("{first:?}") };
        assert!(g.edge_only && g.shape == IndicatorShape::Circle { radius: 5.0 } && g.velocity == [0.0, 0.0]);
        game.run.tick(0.1);
        game.run.drain_events();
        let moving = told(&game.mirror_indicators());
        let [ToClient::GroundIndicator(m)] = &moving[..] else { panic!("{moving:?}") };
        assert_eq!(m.id, g.id);
        assert!((m.velocity[0] + 2.0).abs() < 0.05 && m.velocity[1].abs() < 0.05, "towards the hero: {:?}", m.velocity);
        // steady on its way: nothing more to tell
        game.run.tick(0.1);
        game.run.drain_events();
        assert!(told(&game.mirror_indicators()).is_empty());
        game.run.phase = Phase::Paused;
        let held = told(&game.mirror_indicators());
        let [ToClient::GroundIndicator(h)] = &held[..] else { panic!("{held:?}") };
        assert!(h.still && h.velocity == [0.0, 0.0]);
    }
}
