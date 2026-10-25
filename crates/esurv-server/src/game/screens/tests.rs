//! The screens' messages against a hosted run: hello, the choice screens and their stand-in, pause,
//! leaving, a victory and Endless, a fresh run and a reset, the meter, the boss bars and the armory.

use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use benilla_protocol::messages::parse_server;
use benilla_protocol::ServerPacket;
use esurv_protocol::{Reader, Source};

use super::*;
use crate::game::{hero_setup, Player};
use crate::save::{FinishedRun, Saves};
use crate::wire::PacketWriter;

/// A run with one mage from the save, its client at the other end of a local socket (unread).
fn game() -> (Game, TcpStream, Arc<Saves>) {
    let content = Arc::new(
        esurv_content::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content"))
            .unwrap(),
    );
    let saves = Arc::new(Saves::in_memory());
    let saved = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
    let (_host, inputs) = std::sync::mpsc::channel();
    let mut game = Game::new(content, None, 0, &saved, saves.clone(), inputs).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (server, _) = listener.accept().unwrap();
    let hero = game.hero_id(0);
    assert!(game.join(Player {
        hero,
        saved,
        writer: Arc::new(Mutex::new(PacketWriter::new(server))),
    }));
    (game, client, saves)
}

/// The protocol messages among `packets`, in order.
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

fn hello(screens: Screens) -> ToServer {
    ToServer::Hello {
        version: esurv_protocol::VERSION,
        screens,
    }
}

/// What the screens are told after the events since the last call.
fn after_tick(game: &mut Game) -> Vec<ToClient> {
    let events = game.run.drain_events();
    told(&game.settle(&events))
}

/// The bar holds seven learned spells on its first seven buttons and the race spell on the eighth;
/// an eighth learned spell has no button.
#[test]
fn the_bar_holds_seven_spells_then_the_race_spell() {
    let (mut game, _client, _) = game();
    let content = game.run.content.clone();
    let racial = game.run.heroes[0].spells.racial.as_ref().unwrap().spell.clone();
    let more: Vec<_> = content
        .abilities
        .iter()
        .filter(|a| a.id != racial && !game.run.heroes[0].spells.knows(&a.id))
        .take(7)
        .collect();
    for a in more {
        game.run.heroes[0].spells.learn(a, &[]);
    }
    assert_eq!(game.run.heroes[0].spells.known.len(), 8);
    let packet = game.action_bar(&game.run.heroes[0]);
    let Ok(ServerPacket::ActionButtons { buttons }) = parse_server(packet.opcode, &packet.body)
    else {
        panic!("the action bar")
    };
    let filled: Vec<(u8, u32)> = buttons
        .iter()
        .filter(|b| b.action != 0)
        .map(|b| (b.slot, b.action))
        .collect();
    let slots: Vec<u8> = filled.iter().map(|(s, _)| *s).collect();
    assert_eq!(slots, [0, 1, 2, 3, 4, 5, 6, crate::game::present::RACE_SPELL_SLOT]);
    let look = content.ability(&racial).unwrap().look_spell;
    assert_eq!(filled[7], (7, look), "the race spell on the eighth button");
}

#[test]
fn hello_brings_the_armory_the_options_the_codex_then_the_hud() {
    let (mut game, _client, saves) = game();
    let hero = game.hero_id(0);
    saves.add_to_codex("accuria");
    assert!(after_tick(&mut game).is_empty(), "nothing before hello");
    let t = told(&game.ask(hero, hello(Screens::default())));
    let [ToClient::Ready(r), ToClient::Progress(p), ToClient::RunOptions { options, bonus }, ToClient::Codex { found }, ToClient::LookStyles(styles), ToClient::SheathHolds(holds)] =
        &t[..]
    else {
        panic!("{t:?}")
    };
    assert_eq!((r.name.as_str(), r.class.as_str()), ("Ana", "Class 8"));
    assert_eq!(r.zone, game.run.content.zones[0].name);
    assert_eq!((p.class.as_str(), p.points, p.zones_open), ("mage", 0, 1));
    assert_eq!((options.density, options.speed, *bonus), (100, 100, 1.0));
    assert_eq!(found, &["accuria"]);
    assert!(!styles.is_empty(), "how the options' looks are drawn");
    assert!(holds.is_empty(), "a mage holds no sheath state");
    let t = after_tick(&mut game);
    let [ToClient::Hud(h), ToClient::SpellTips { .. }, ToClient::Buffs { .. }] = &t[..] else {
        panic!("{t:?}")
    };
    assert_eq!(h.phase, protocol::Phase::Waiting);
    assert_eq!(h.next_boss, Some(180), "the first boss comes at 3 minutes");
    assert!(
        after_tick(&mut game).is_empty(),
        "told once until it changes"
    );
}

#[test]
fn the_buffs_say_what_cast_them_and_their_value_now() {
    use esurv_sim::game::pickups::SPEED_RUNE_VISUAL;
    use esurv_sim::game::run::{Buff, BuffKind};
    let (mut game, _client, _) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    let t = after_tick(&mut game);
    let Some(ToClient::Buffs { buffs, .. }) =
        t.iter().find(|m| matches!(m, ToClient::Buffs { .. }))
    else {
        panic!("{t:?}")
    };
    assert!(buffs.is_empty(), "none yet");
    let content = game.run.content.clone();
    let now = game.run.now;
    let h = &mut game.run.heroes[0];
    let racial = h.spells.racial.as_ref().unwrap().spell.clone();
    let look = content.ability(&racial).unwrap().look_spell;
    h.stats.buff_power = 0.5;
    h.buffs.push(Buff {
        kind: BuffKind::Speed,
        value: 0.6,
        visual: SPEED_RUNE_VISUAL,
        until: now + 10.0,
    });
    h.buffs.push(Buff {
        kind: BuffKind::Damage,
        value: 0.2,
        visual: look,
        until: now + 12.0,
    });
    let tips = buff_tips(&content, h, now);
    assert_eq!(tips.len(), 2, "{tips:?}");
    assert_eq!(
        (tips[0].look, tips[0].kind, tips[0].spell.as_str()),
        (SPEED_RUNE_VISUAL, protocol::BuffKind::Speed, "")
    );
    assert!((tips[0].value - 0.6).abs() < 1e-6, "the rune's as it is");
    assert_eq!(
        (tips[1].kind, tips[1].spell.as_str()),
        (protocol::BuffKind::Damage, racial.as_str()),
        "the race's spell cast it"
    );
    assert!(
        (tips[1].value - 0.3).abs() < 1e-6,
        "grown by buff power: {}",
        tips[1].value
    );
    // told to the hero's client once the beat allows
    std::thread::sleep(SCREENS_EVERY);
    let t = after_tick(&mut game);
    let Some(ToClient::Buffs { buffs, .. }) =
        t.iter().find(|m| matches!(m, ToClient::Buffs { .. }))
    else {
        panic!("{t:?}")
    };
    assert_eq!(buffs.len(), 2);
}

/// The spellbook's tooltips, as the hero's client is told them.
fn spell_tips(t: &[ToClient]) -> Option<&Vec<protocol::SpellTip>> {
    t.iter().find_map(|m| match m {
        ToClient::SpellTips { spells, .. } => Some(spells),
        _ => None,
    })
}

#[test]
fn the_spellbook_tooltips_carry_the_heros_numbers_and_follow_an_upgrade() {
    use esurv_sim::game::upgrades::apply;
    let (mut game, _client, _) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    let t = after_tick(&mut game);
    let tips = spell_tips(&t).expect("told after hello").clone();
    let content = game.run.content.clone();
    let h = &game.run.heroes[0];
    // the race's spell first (no rank), then the spell the run starts with, at rank 1; without the
    // game data the race's passives have no stock spell to show as
    let racial = h.spells.racial.as_ref().unwrap();
    let first = &h.spells.known[0];
    assert_eq!(tips.len(), 2, "{tips:?}");
    assert_eq!(
        (tips[0].kind, tips[0].id.as_str(), tips[0].rank),
        (protocol::TipKind::Racial, racial.spell.as_str(), 0)
    );
    assert_eq!(
        tips[0].look,
        content.ability(&racial.spell).unwrap().look_spell
    );
    assert_eq!(
        (tips[1].kind, tips[1].id.as_str(), tips[1].rank),
        (protocol::TipKind::Learned, first.spell.as_str(), 1)
    );
    // the numbers a card names, as the casting rules work them out for this hero
    let a = content.ability(&first.spell).unwrap();
    let expected: Vec<(String, f32)> = spell_numbers(a, &first.instance(), &h.stats)
        .into_iter()
        .map(|(w, v)| (w.to_owned(), v))
        .collect();
    let got: Vec<(String, f32)> = tips[1]
        .numbers
        .iter()
        .map(|n| (n.what.clone(), n.value))
        .collect();
    assert_eq!(got, expected);
    assert!(got.iter().any(|(w, _)| w == "cooldown"));
    // a fresh spell: no card taken, nothing from the hero but its staff's +10 % damage (the
    // mage's first weapon loadout; the human's passives touch no spell), and its hits crit at the
    // hero's chance and damage
    assert!(tips[1].upgrades.is_empty(), "{tips:?}");
    let bonuses: Vec<(String, f32)> = tips[1]
        .bonuses
        .iter()
        .map(|b| (b.what.clone(), (b.value * 100.0).round()))
        .collect();
    assert_eq!(bonuses, [("power".to_owned(), 10.0)], "{tips:?}");
    let s = &game.run.heroes[0].stats;
    assert_eq!(tips[1].crit, Some((s.crit_chance, s.crit_damage)));
    assert!(
        after_tick(&mut game).is_empty(),
        "told once until it changes"
    );
    // a damage talent on the hero: its bonus, told with the rest
    game.run.heroes[0]
        .modifiers
        .push(esurv_sim::game::stats::Modifier::new(
            esurv_content::Stat::Damage,
            0.2,
        ));
    let h = &mut game.run.heroes[0];
    h.stats = compute(&content.classes[h.class], h.progress.level, &h.modifiers);
    std::thread::sleep(SCREENS_EVERY);
    let t = after_tick(&mut game);
    let tips = spell_tips(&t).expect("told after the talent").clone();
    let bonus = tips[1]
        .bonuses
        .iter()
        .find(|b| b.what == "power")
        .map(|b| b.value)
        .expect("a damage bonus");
    // +0.2 on a level-1 sheet's 1.0, beside the staff's +0.1
    assert!((bonus - 0.3).abs() < 1e-4, "{bonus}");
    assert!(
        after_tick(&mut game).is_empty(),
        "told once until it changes"
    );
    // a power upgrade: rank 2 and more damage, told again once the 0.2 s beat allows
    let damage = |tip: &protocol::SpellTip| {
        tip.numbers
            .iter()
            .find(|n| n.what == "damage")
            .map(|n| n.value)
    };
    let before = damage(&tips[1]).expect("the first spell hits");
    apply(
        &mut game.run.heroes[0].spells.known[0].upgrades,
        &[Upgrade::Power],
        esurv_sim::game::offers::Rarity::Common,
    );
    std::thread::sleep(SCREENS_EVERY);
    let t = after_tick(&mut game);
    let tips = spell_tips(&t).expect("told again");
    assert_eq!(tips[1].rank, 2);
    let after = damage(&tips[1]).unwrap();
    assert!(after > before, "{before} -> {after}");
    // the card's power, in its upgrade totals
    let u = game.run.heroes[0].spells.known[0].upgrades;
    let power = tips[1].upgrades.iter().find(|v| v.what == "power").unwrap();
    assert!((power.value - (u.power - 1.0)).abs() < 1e-4, "{tips:?}");
}

#[test]
fn a_client_with_choice_screens_chooses_for_itself() {
    let (mut game, _client, _) = game();
    let hero = game.hero_id(0);
    let cards = Screens {
        cards: true,
        ..Default::default()
    };
    game.ask(hero, hello(cards));
    game.start();
    game.run.heroes[0].pending_choices = 1;
    game.run.tick(0.05);
    game.stand_in_for_the_card_screen();
    assert_eq!(game.run.phase, Phase::Choosing, "nothing taken for it");
    let t = after_tick(&mut game);
    let Some(ToClient::Choices(c)) = t.iter().find(|m| matches!(m, ToClient::Choices(_))) else {
        panic!("{t:?}")
    };
    assert_eq!(
        (c.kind, c.free_rerolls, c.reroll_cost),
        (protocol::ScreenKind::LevelUp, 3, 0)
    );
    assert!(!c.offers.is_empty());
    // the kit: the spell every run starts with, at rank 1, nothing worn
    let known = &game.run.heroes[0].spells.known;
    assert_eq!(c.kit.spells.len(), known.len());
    assert_eq!(
        (c.kit.spells[0].id.as_str(), c.kit.spells[0].rank),
        (known[0].spell.as_str(), 1)
    );
    assert!(c.kit.jewelry.is_empty() && c.kit.amulet.is_none());
    assert_eq!(
        told(&game.ask(hero, ToServer::Choose(9))),
        [ToClient::Refused("choose".into())]
    );
    assert!(told(&game.ask(hero, ToServer::Reroll)).is_empty());
    let t = after_tick(&mut game);
    assert!(
        t.iter()
            .any(|m| matches!(m, ToClient::Choices(c) if c.free_rerolls == 2)),
        "new offers, a free reroll spent"
    );
    assert!(told(&game.ask(hero, ToServer::Choose(0))).is_empty());
    assert_eq!(game.run.phase, Phase::Running);
    let t = after_tick(&mut game);
    assert!(t.contains(&ToClient::ChoicesClosed {
        hero: game.hero_guid(hero)
    }));
}

#[test]
fn a_rares_cache_is_named_apart_and_the_kit_shows_what_is_worn() {
    use esurv_sim::game::choices::{WornAmulet, WornJewelry};
    let (mut game, _client, _) = game();
    let hero = game.hero_id(0);
    game.ask(
        hero,
        hello(Screens {
            cards: true,
            ..Default::default()
        }),
    );
    game.start();
    let amulet = game.run.content.amulets[0].clone();
    let h = &mut game.run.heroes[0];
    h.jewelry.push(WornJewelry {
        id: "accuria".into(),
        value: 0.4,
        rank: 2,
    });
    h.amulet = Some(WornAmulet {
        amulet: amulet.clone(),
        rank: 1,
    });
    h.pending_rare_caches = 1;
    game.run.tick(0.05);
    let t = after_tick(&mut game);
    let Some(ToClient::Choices(c)) = t.iter().find(|m| matches!(m, ToClient::Choices(_))) else {
        panic!("{t:?}")
    };
    assert_eq!(c.kind, protocol::ScreenKind::RareCache);
    assert_eq!(
        c.kit.jewelry,
        [protocol::Worn {
            id: "accuria".into(),
            name: String::new(),
            rank: 2,
            value: 0.4,
        }]
    );
    let worn = c.kit.amulet.as_ref().unwrap();
    assert_eq!(
        (worn.id.as_str(), worn.name.as_str(), worn.rank, worn.value),
        (amulet.id.as_str(), amulet.name.as_str(), 1, 1.0)
    );
}

#[test]
fn a_new_ring_taken_joins_the_codex_and_every_told_client_hears() {
    use esurv_sim::game::choices::Offer;
    use esurv_sim::game::offers::Rarity;
    let (mut game, _client, saves) = game();
    let hero = game.hero_id(0);
    let ring = |id: &str| Event::OfferTaken {
        hero,
        offer: Offer::NewJewelry {
            id: id.into(),
            value: 0.17,
            rarity: Rarity::Common,
        },
    };
    // not told before hello, but kept all the same
    game.settle(&[ring("accuria")]);
    assert_eq!(saves.codex(), ["accuria"]);
    game.ask(hero, hello(Screens::default()));
    let codex = |t: &[ToClient]| -> Vec<Vec<String>> {
        t.iter()
            .filter_map(|m| match m {
                ToClient::Codex { found } => Some(found.clone()),
                _ => None,
            })
            .collect()
    };
    let t = told(&game.settle(&[ring("quick_strike")]));
    assert_eq!(
        codex(&t),
        [vec!["accuria".to_owned(), "quick_strike".to_owned()]]
    );
    let t = told(&game.settle(&[ring("quick_strike")]));
    assert!(
        codex(&t).is_empty(),
        "a piece already in the codex: no news"
    );
    let gold = Event::OfferTaken {
        hero,
        offer: Offer::Gold(10),
    };
    assert!(codex(&told(&game.settle(&[gold]))).is_empty());
}

#[test]
fn without_choice_screens_the_first_offer_is_taken() {
    let (mut game, _client, _) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    game.start();
    game.run.heroes[0].pending_choices = 1;
    game.run.tick(0.05);
    game.stand_in_for_the_card_screen();
    assert_eq!(game.run.phase, Phase::Running);
    assert_eq!(game.run.heroes[0].pending_choices, 0);
}

#[test]
fn a_pause_stops_the_clock_until_resumed() {
    let (mut game, _client, _) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    assert_eq!(
        told(&game.ask(hero, ToServer::Pause)),
        [ToClient::Refused("pause".into())],
        "not before the run begins"
    );
    game.start();
    game.run.tick(0.05);
    after_tick(&mut game);
    assert!(told(&game.ask(hero, ToServer::Pause)).is_empty());
    let at = game.run.now;
    game.run.tick(0.05);
    assert_eq!(game.run.now, at);
    let t = after_tick(&mut game);
    assert!(t
        .iter()
        .any(|m| matches!(m, ToClient::Hud(h) if h.phase == protocol::Phase::Paused)));
    assert!(told(&game.ask(hero, ToServer::Resume)).is_empty());
    game.run.tick(0.05);
    assert!(game.run.now > at);
}

#[test]
fn leaving_pays_out_and_tells_the_end_at_once() {
    let (mut game, _client, saves) = game();
    let hero = game.hero_id(0);
    let end = Screens {
        run_end: true,
        ..Default::default()
    };
    game.ask(hero, hello(end));
    game.start();
    game.run.now = 120.0;
    let t = told(&game.ask(hero, ToServer::Leave));
    let [ToClient::RunEnd(e)] = &t[..] else {
        panic!("{t:?}")
    };
    assert!(game.run.is_over());
    assert_eq!((e.ending, e.won, e.seconds), (Ending::Left, false, 120.0));
    assert_eq!(e.experience.time, 20, "2 minutes x 10");
    assert_eq!((e.experience_before, e.experience_after), (0, 20));
    let guid = game.saved(hero).unwrap().guid;
    assert_eq!(saves.hero(guid).unwrap().records.runs, 1);
    assert!(
        after_tick(&mut game)
            .iter()
            .all(|m| !matches!(m, ToClient::RunEnd(_))),
        "a hero who left is not told the run's end again"
    );
    assert_eq!(
        told(&game.ask(hero, ToServer::Leave)),
        [ToClient::Refused("leave".into())]
    );
    let records = saves.hero(guid).unwrap().records;
    assert_eq!(records.runs, 1, "counted once");
    assert_eq!(saves.class_progress(&game.run.content, "mage").talent_experience, 20);
}

#[test]
fn a_run_left_before_it_began_pays_nothing() {
    let (mut game, _client, saves) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    let t = told(&game.ask(hero, ToServer::Leave));
    assert!(t.iter().any(|m| matches!(m, ToClient::RunEnd(e)
            if e.ending == Ending::Left && e.experience == protocol::Experience::default())));
    let guid = game.saved(hero).unwrap().guid;
    assert_eq!(saves.hero(guid).unwrap().records.runs, 0);
}

#[test]
fn logging_out_pays_the_run_out_too() {
    let (mut game, _client, saves) = game();
    let hero = game.hero_id(0);
    game.start();
    game.run.now = 300.0;
    let (done, _wait) = std::sync::mpsc::channel();
    game.input(crate::game::Input::Left { hero, done });
    assert!(game.players.is_empty());
    let records = saves.heroes()[0].records;
    assert_eq!((records.runs, records.best_seconds), (1, 300));
    assert_eq!(saves.class_progress(&game.run.content, "mage").talent_experience, 50);
}

#[test]
fn a_victory_pays_and_endless_pays_its_extra() {
    let (mut game, _client, saves) = game();
    let hero = game.hero_id(0);
    let end = Screens {
        run_end: true,
        ..Default::default()
    };
    game.ask(hero, hello(end));
    game.start();
    game.run.now = 990.0;
    game.run.won_at = Some(990.0);
    game.run.phase = Phase::Victory;
    let t = after_tick(&mut game);
    let Some(ToClient::RunEnd(won)) = t.iter().find(|m| matches!(m, ToClient::RunEnd(_))) else {
        panic!("{t:?}")
    };
    assert_eq!((won.ending, won.won), (Ending::Victory, true));
    // 16.5 minutes: 165 for the time, 250 for the victory
    assert_eq!((won.experience.time, won.experience.victory), (165, 250));
    assert_eq!(
        won.zone_opened.as_deref(),
        Some(game.run.content.zones[1].name.as_str())
    );
    assert!(told(&game.ask(hero, ToServer::Endless)).is_empty());
    assert_eq!(game.run.phase, Phase::Running);
    let t = after_tick(&mut game);
    assert!(t.iter().any(
        |m| matches!(m, ToClient::Hud(h) if h.endless && h.phase == protocol::Phase::Running)
    ));
    // a minute of Endless, then a fall: the extra minute is paid, the run counted once
    game.run.now = 1050.0;
    game.run.damage_hero(0, 1e9, None, false);
    game.run.tick(0.05);
    let t = after_tick(&mut game);
    let Some(ToClient::RunEnd(fell)) = t.iter().find(|m| matches!(m, ToClient::RunEnd(_))) else {
        panic!("{t:?}")
    };
    assert_eq!((fell.ending, fell.won), (Ending::Fell, true));
    assert_eq!(fell.experience.time, 10, "the extra minute only");
    assert_eq!(
        (fell.experience.victory, fell.zone_opened.as_deref()),
        (0, None)
    );
    let guid = game.saved(hero).unwrap().guid;
    let records = saves.hero(guid).unwrap().records;
    assert_eq!((records.runs, records.wins), (1, 1));
    assert_eq!(
        saves.class_progress(&game.run.content, "mage").open_zones(&game.run.content),
        2,
        "one zone opened"
    );
}

#[test]
fn won_gear_joins_the_collection_at_once_and_pops_up() {
    let (mut game, _client, saves) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    // a mage's (class 8) shoulders, tier 0
    let entry = esurv_sim::game::rewards::gear_item_entry(8, 1, 0);
    let events = [Event::GearDropped {
        hero,
        slot: 1,
        tier: 0,
        entry,
    }];
    let chat = game.hand_out_loot(&events);
    assert_eq!(chat.len(), 1, "a line in its chat");
    assert!(saves.collection(&game.run.content).contains(&entry));
    let t = told(&game.settle(&events));
    let Some(ToClient::GearDropped(d)) = t.iter().find(|m| matches!(m, ToClient::GearDropped(_)))
    else {
        panic!("{t:?}")
    };
    assert_eq!((d.entry, d.slot, d.tier, d.new), (entry, 1, 0, true));
    assert!(!d.name.is_empty());
    game.hand_out_loot(&events);
    let t = told(&game.settle(&events));
    assert!(t
        .iter()
        .any(|m| matches!(m, ToClient::GearDropped(d) if !d.new)));
}

#[test]
fn bosses_get_their_banners_and_rares_none() {
    use protocol::Banner;
    let (mut game, _client, _) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    game.start();
    game.run.now = 180.0;
    game.run.tick(0.05);
    game.units_now();
    let t = after_tick(&mut game);
    let first = game.run.content.zones[0].bosses[0].name.clone();
    assert!(t.contains(&ToClient::Banner(Banner::BossComes {
        name: first.clone(),
        final_boss: false
    })));
    let boss = game.run.enemies.iter().find(|e| e.is_boss()).unwrap().id;
    let t = told(&game.settle(&[Event::EnemyDied { id: boss, by: None }]));
    assert!(t.contains(&ToClient::Banner(Banner::BossDown { name: first })));
    // a rare falls with no banner
    let rare = [
        Event::RareSpawned {
            id: 999,
            roster: 0,
            at: (0.0, 0.0),
        },
        Event::EnemyDied { id: 999, by: None },
    ];
    let t = told(&game.settle(&rare));
    assert!(!t.iter().any(|m| matches!(m, ToClient::Banner(_))), "{t:?}");
}

#[test]
fn the_meter_counts_by_source_with_area_for_no_enemy() {
    let (mut game, _client, _) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    let hit = |amount, source| Event::EnemyHit {
        id: 1,
        amount,
        crit: false,
        by: Some(hero),
        source,
    };
    let events = [
        hit(10.0, HitSource::Spell("fireball".into())),
        hit(5.5, HitSource::Tick("fireball".into())),
        hit(
            3.0,
            HitSource::Proc {
                spell: 467,
                school: esurv_content::School::Nature,
                origin: ProcOrigin::Other,
            },
        ),
        hit(
            2.0,
            HitSource::Proc {
                spell: 133,
                school: esurv_content::School::Fire,
                origin: ProcOrigin::Talent("ignite".into()),
            },
        ),
        Event::HeroHit {
            hero,
            amount: 9.0,
            lost: 4.0,
            by: None,
            melee: false,
        },
    ];
    let t = told(&game.settle(&events));
    let Some(ToClient::Meter(m)) = t.iter().find(|m| matches!(m, ToClient::Meter(_))) else {
        panic!("{t:?}")
    };
    let done: Vec<(Source, u64)> = m
        .done
        .iter()
        .map(|e| (e.source.clone(), e.amount))
        .collect();
    assert_eq!(
        done,
        [
            (Source::Spell("fireball".into()), 16),
            (
                Source::Proc {
                    spell: 467,
                    origin: protocol::ProcOrigin::Other
                },
                3
            ),
            (
                Source::Proc {
                    spell: 133,
                    origin: protocol::ProcOrigin::Talent("ignite".into())
                },
                2
            )
        ]
    );
    let taken: Vec<(Source, u64)> = m
        .taken
        .iter()
        .map(|e| (e.source.clone(), e.amount))
        .collect();
    assert_eq!(taken, [(Source::Area, 4)], "what the hero's health lost");
}

/// Warsong Gulch's player-looking enemies: an elite's, a rare's and a boss's rank go to every
/// told client once as they appear, and again to a client saying `hello`; a normal one's never.
#[test]
fn player_looking_elites_rares_and_bosses_have_their_ranks_told() {
    use esurv_sim::game::run::{Flat, Run};
    let (mut game, _client, saves) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    let content = game.run.content.clone();
    let saved = game.players[0].saved.clone();
    let setup = hero_setup(&content, &saved, &saves.class_progress(&content, &saved.class));
    game.run = Run::new(content.clone(), "wsg", Default::default(), &[setup], Box::new(Flat), 5).unwrap();
    let zone = content.zones.iter().find(|z| z.id == "wsg").unwrap();
    let elite = zone.enemies.iter().position(|e| e.elite).unwrap();
    let h = game.run.heroes[0].id;
    let p = game.run.heroes[0].pos;
    game.run.hero_moved(h, (p.0 + 1.0, p.1), 0.0, 0.0);
    while game.run.enemies.len() < 4 {
        game.run.tick(0.05);
    }
    game.run.enemies.truncate(4);
    game.run.enemies[0].kind = EnemyKind::Roster(elite);
    game.run.enemies[1].kind = EnemyKind::Rare { roster: elite };
    game.run.enemies[2].kind = EnemyKind::Boss(0);
    let normal = game.run.enemies[3].kind;
    assert!(matches!(normal, EnemyKind::Roster(r) if !zone.enemies[r].elite), "{normal:?}");
    game.units_now();
    let ids: Vec<u32> = game.run.enemies.iter().map(|e| e.id).collect();
    let guid = |i: usize| super::super::player_enemies::player_enemy_guid(ids[i]);
    let want = vec![(guid(0), packets::RANK_ELITE), (guid(1), packets::RANK_RARE), (guid(2), packets::RANK_BOSS)];
    let ranks = |t: &[ToClient]| t.iter().filter_map(|m| if let ToClient::EnemyRanks(r) = m { Some(r.clone()) } else { None }).collect::<Vec<_>>();
    assert_eq!(ranks(&after_tick(&mut game)), vec![want.clone()]);
    assert!(ranks(&after_tick(&mut game)).is_empty(), "told once");
    assert_eq!(ranks(&told(&game.ask(hero, hello(Screens::default())))), vec![want]);
}

#[test]
fn a_boss_has_a_bar_until_it_is_gone() {
    let (mut game, _client, _) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    game.start();
    game.run.now = 180.0;
    game.run.tick(0.05);
    game.units_now();
    let t = after_tick(&mut game);
    let Some(ToClient::BossBar(b)) = t.iter().find(|m| matches!(m, ToClient::BossBar(_))) else {
        panic!("{t:?}")
    };
    let first = &game.run.content.zones[0].bosses[0];
    assert_eq!(b.name, first.name);
    assert!(b.health > 0 && b.health == b.max_health);
    assert_eq!(b.phases, super::boss_phases(first), "with its phase marks");
    game.run.enemies.retain(|e| !e.is_boss());
    let t = after_tick(&mut game);
    assert!(t
        .iter()
        .any(|m| matches!(m, ToClient::BossBar(x) if x.boss == b.boss && x.health == 0)));
}

#[test]
fn the_armory_answers_with_the_class_progress_or_a_refusal() {
    let (mut game, _client, saves) = game();
    let hero = game.hero_id(0);
    let first = game
        .run
        .content
        .talent_trees
        .iter()
        .find(|t| t.class == "mage")
        .and_then(|t| t.talents.iter().find(|x| x.row == 0))
        .unwrap()
        .id
        .clone();
    assert_eq!(
        told(&game.ask(hero, ToServer::LearnTalent(first.clone()))),
        [ToClient::Refused("learn".into())],
        "no points yet"
    );
    let guid = game.saved(hero).unwrap().guid;
    let run = FinishedRun {
        experience: 145,
        ..Default::default()
    };
    saves.finish_run(&game.run.content, guid, &run).unwrap();
    let t = told(&game.ask(hero, ToServer::LearnTalent(first.clone())));
    let [ToClient::Progress(p)] = &t[..] else {
        panic!("{t:?}")
    };
    assert_eq!((p.points, p.spent), (1, 1));
    assert_eq!(p.talents, [(first, 1)]);
    let asked = protocol::RunOptions {
        density: 130,
        health: 100,
        damage: 100,
        speed: 200,
        ..Default::default()
    };
    let t = told(&game.ask(hero, ToServer::RunOptions(asked)));
    let [ToClient::RunOptions { options, bonus }] = &t[..] else {
        panic!("{t:?}")
    };
    assert_eq!(
        (options.density, options.speed),
        (125, 150),
        "on the sliders"
    );
    assert_eq!(*bonus, 2.0, "at most double");
    assert_eq!(saves.run_options().speed, 150);
}

/// The world packets among `packets`, read back.
fn world(packets: &[(u32, Packet)]) -> Vec<ServerPacket> {
    packets
        .iter()
        .filter_map(|(_, p)| parse_server(p.opcode, &p.body).ok())
        .collect()
}

#[test]
fn again_waits_for_the_end_of_the_run() {
    let (mut game, _client, _) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    let refused = [ToClient::Refused("again".into())];
    assert_eq!(told(&game.ask(hero, ToServer::Again)), refused, "waiting");
    game.run.start();
    game.run.tick(0.05);
    assert_eq!(told(&game.ask(hero, ToServer::Again)), refused, "running");
    // a hero who left has left the run for good
    game.ask(hero, ToServer::Leave);
    assert!(game.run.is_over());
    assert_eq!(told(&game.ask(hero, ToServer::Again)), refused, "left");
}

#[test]
fn again_after_a_fall_pays_clears_the_field_and_starts_over() {
    let (mut game, _client, saves) = game();
    let hero = game.hero_id(0);
    let end = Screens {
        run_end: true,
        ..Default::default()
    };
    game.ask(hero, hello(end));
    // started by a first move, not at once
    game.run.start();
    game.run.now = 120.0;
    assert!(game.run.spawn_roster_enemy(0, None));
    game.units_now();
    game.run.heroes[0].gold = 40;
    game.run.damage_hero(0, 1e9, None, false);
    game.run.tick(0.05);
    let t = after_tick(&mut game);
    assert!(t
        .iter()
        .any(|m| matches!(m, ToClient::RunEnd(e) if e.ending == Ending::Fell)));
    let on_the_field: Vec<u64> = game.shown.values().map(|s| s.guid).collect();
    assert!(!on_the_field.is_empty());

    let answer = game.ask(hero, ToServer::Again);
    assert!(answer.iter().all(|(h, _)| *h == hero), "all for its client");
    let packets = world(&answer);
    for guid in on_the_field {
        assert!(
            packets
                .iter()
                .any(|p| matches!(p, ServerPacket::DestroyObject { guid: g } if *g == guid)),
            "{guid:X} goes"
        );
    }
    let me = game.hero_guid(hero);
    let spawn = game.run.content.zones[0].spawn;
    assert!(packets.iter().any(|p| matches!(p,
        ServerPacket::Teleport { guid, position, .. }
            if *guid == me && (position.x, position.y) == (spawn.x, spawn.y))));
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::CooldownCheat { caster } if *caster == me)));
    let Some(ServerPacket::InitialSpells { spell_ids, .. }) = packets
        .iter()
        .find(|p| matches!(p, ServerPacket::InitialSpells { .. }))
    else {
        panic!("the spellbook again")
    };
    assert_eq!(spell_ids.len(), 2, "the race spell and the starting spell");
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::ActionButtons { .. })));
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::UpdateObject { .. })));
    assert!(
        !packets
            .iter()
            .any(|p| matches!(p, ServerPacket::MoveMode { .. })),
        "a fall rooted nothing, so nothing is let go"
    );
    let t = told(&answer);
    let [ToClient::Ready(r), ToClient::Progress(p)] = &t[..] else {
        panic!("{t:?}")
    };
    assert_eq!(r.name, "Ana");
    assert_eq!(p.talent_experience, 20, "the fall paid 2 minutes");

    // the fresh run waits for a first move, its hero whole at the spawn
    assert_eq!(game.run.phase, Phase::Waiting);
    assert_eq!(game.run.now, 0.0);
    let h = &game.run.heroes[0];
    assert_eq!(h.id, hero, "the same hero, the same id");
    assert!(h.alive && h.health == h.stats.max_health);
    assert_eq!((h.progress.level, h.gold, h.kills), (1, 0, 0));
    assert_eq!(h.pos, (spawn.x, spawn.y));
    assert!(game.shown.is_empty() && game.run.enemies.is_empty());
    // a stale move from where it fell starts nothing
    game.run
        .hero_moved(hero, (spawn.x + 30.0, spawn.y), spawn.z, 0.0);
    assert_eq!(game.run.phase, Phase::Waiting);
    let t = after_tick(&mut game);
    assert!(
        t.iter().any(|m| matches!(m,
            ToClient::Hud(h) if h.phase == protocol::Phase::Waiting && h.clock == 0 && h.gold == 0)),
        "{t:?}"
    );

    // the new run is a run of its own: it pays from nothing and counts again
    game.run.start();
    game.run.now = 60.0;
    game.run.damage_hero(0, 1e9, None, false);
    game.run.tick(0.05);
    let t = after_tick(&mut game);
    let Some(ToClient::RunEnd(e)) = t.iter().find(|m| matches!(m, ToClient::RunEnd(_))) else {
        panic!("{t:?}")
    };
    assert_eq!((e.experience.time, e.experience_before), (10, 20));
    let guid = game.saved(hero).unwrap().guid;
    assert_eq!(saves.hero(guid).unwrap().records.runs, 2);
}

#[test]
fn again_after_a_victory_lets_the_hero_go_and_an_at_once_start_starts_at_once() {
    let (mut game, _client, saves) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    game.start();
    game.run.now = 990.0;
    game.run.won_at = Some(990.0);
    game.run.phase = Phase::Victory;
    game.rooted = true;
    after_tick(&mut game);
    assert_eq!(saves.class_progress(&game.run.content, "mage").open_zones(&game.run.content), 2);
    let answer = game.ask(hero, ToServer::Again);
    let packets = world(&answer);
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::MoveMode { apply: false, .. })));
    assert!(!game.rooted);
    assert_eq!(game.run.phase, Phase::Running, "the dev switch starts it");
    assert_eq!(game.run.zone, 0, "the same zone");
    let guid = game.saved(hero).unwrap().guid;
    assert_eq!(saves.hero(guid).unwrap().records.wins, 1);
    // nothing more was paid: the victory had paid it all
    assert_eq!(
        saves.class_progress(&game.run.content, "mage").talent_experience,
        165 + 250,
        "16.5 minutes and the victory"
    );
}

#[test]
fn a_reset_waits_for_the_run_to_begin_and_not_after_leaving() {
    let (mut game, _client, _) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    let refused = [ToClient::Refused("reset".into())];
    assert_eq!(
        told(&game.ask(hero, ToServer::Reset)),
        refused,
        "nothing to reset before the first step"
    );
    game.run.start();
    game.run.tick(0.05);
    game.ask(hero, ToServer::Leave);
    assert_eq!(
        told(&game.ask(hero, ToServer::Reset)),
        refused,
        "a hero who left has left the run for good"
    );
}

#[test]
fn a_reset_mid_run_pays_it_out_as_left_and_starts_a_fresh_one() {
    let (mut game, _client, saves) = game();
    let hero = game.hero_id(0);
    let screens = Screens {
        cards: true,
        run_end: true,
        hud: true,
    };
    game.ask(hero, hello(screens));
    game.run.start();
    game.run.now = 120.0;
    assert!(game.run.spawn_roster_enemy(0, None));
    game.units_now();
    game.run.heroes[0].gold = 40;
    game.run.tick(0.05);
    after_tick(&mut game);
    // from the Escape menu: the run is paused
    assert!(told(&game.ask(hero, ToServer::Pause)).is_empty());
    let on_the_field: Vec<u64> = game.shown.values().map(|s| s.guid).collect();
    assert!(!on_the_field.is_empty());

    let answer = game.ask(hero, ToServer::Reset);
    let t = told(&answer);
    let [ToClient::Ready(r), ToClient::Progress(p)] = &t[..] else {
        panic!("a fresh run at once, no end panel: {t:?}")
    };
    assert_eq!(r.name, "Ana");
    assert_eq!(
        p.talent_experience, 20,
        "paid as a run left: 2 minutes, nothing more"
    );
    let packets = world(&answer);
    for guid in on_the_field {
        assert!(
            packets
                .iter()
                .any(|p| matches!(p, ServerPacket::DestroyObject { guid: g } if *g == guid)),
            "{guid:X} goes"
        );
    }
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::MessageChat(m) if !m.is_addon())),
        "the chat says what the run earned"
    );
    let guid = game.saved(hero).unwrap().guid;
    let records = saves.hero(guid).unwrap().records;
    assert_eq!((records.runs, records.wins), (1, 0));

    // the fresh run waits for its first move, its hero whole at the spawn, still in it
    assert_eq!((game.run.phase, game.run.now), (Phase::Waiting, 0.0));
    let h = &game.run.heroes[0];
    assert_eq!(h.id, hero);
    assert!(h.alive && h.health == h.stats.max_health);
    assert_eq!((h.progress.level, h.gold), (1, 0));
    assert!(!game.left.contains(&hero));
    assert!(game.run.enemies.is_empty() && game.shown.is_empty());
    let t = after_tick(&mut game);
    assert!(
        t.iter().any(|m| matches!(m,
            ToClient::Hud(h) if h.phase == protocol::Phase::Waiting && h.clock == 0)),
        "{t:?}"
    );
    assert!(
        !t.iter().any(|m| matches!(m, ToClient::RunEnd(_))),
        "the old run's end is never told"
    );
    assert_eq!(records.runs, 1, "paid once");
    assert_eq!(saves.class_progress(&game.run.content, "mage").talent_experience, 20);
}

#[test]
fn a_reset_in_a_running_run_started_at_once_starts_the_next_at_once() {
    let (mut game, _client, _) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    game.start();
    game.run.now = 30.0;
    assert!(told(&game.ask(hero, ToServer::Reset)).len() >= 2);
    assert_eq!(game.run.phase, Phase::Running, "the dev switch starts it");
    assert!(game.run.now < 1.0);
}

#[test]
fn a_reset_never_ends_another_heros_run() {
    let (mut game, _client, saves) = game();
    let ana = game.hero_id(0);
    // a second hero joins the same run (roadmap R6): the run is rebuilt with both
    let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
    let content = game.run.content.clone();
    let ana_saved = game.saved(ana).unwrap().clone();
    let setups = [
        hero_setup(
            &content,
            &ana_saved,
            &saves.class_progress(&content, &ana_saved.class),
        ),
        hero_setup(&content, &bo, &saves.class_progress(&content, &bo.class)),
    ];
    let zone = content.zones[0].id.clone();
    let options = saves.run_options().run_options(&content.run_options);
    game.run = esurv_sim::game::run::Run::new(
        content,
        &zone,
        options,
        &setups,
        Box::new(esurv_sim::game::run::Flat),
        1,
    )
    .unwrap();
    assert_eq!(game.hero_id(0), ana);
    let bo_id = game.hero_id(1);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let _bo_client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (server, _) = listener.accept().unwrap();
    assert!(game.join(Player {
        hero: bo_id,
        saved: bo,
        writer: Arc::new(Mutex::new(PacketWriter::new(server))),
    }));
    game.ask(ana, hello(Screens::default()));
    game.ask(bo_id, hello(Screens::default()));
    game.run.start();
    game.run.now = 60.0;
    game.run.pause();

    // Bo still plays: Ana's reset is refused, and the run stays as it was
    assert_eq!(
        told(&game.ask(ana, ToServer::Reset)),
        [ToClient::Refused("reset".into())]
    );
    assert_eq!((game.run.phase, game.run.now), (Phase::Paused, 60.0));
    assert!(game.run.heroes.iter().all(|h| h.alive));
    // Bo fallen and watching is still in it
    game.run.damage_hero(1, 1e9, None, false);
    assert!(!game.run.is_over());
    assert_eq!(
        told(&game.ask(ana, ToServer::Reset)),
        [ToClient::Refused("reset".into())]
    );
    // once Bo has ended its run (End run), Ana may reset hers; Bo stays out of the fresh one
    game.ask(bo_id, ToServer::Leave);
    let answer = game.ask(ana, ToServer::Reset);
    assert!(told(&answer)
        .iter()
        .any(|m| matches!(m, ToClient::Ready(r) if r.name == "Ana")));
    assert_eq!(game.run.phase, Phase::Waiting);
    let fresh_bo = game.run.heroes.iter().find(|h| h.id == bo_id).unwrap();
    assert!(
        !fresh_bo.alive,
        "a hero who left is out of the fresh run too"
    );
    assert!(game.run.heroes.iter().any(|h| h.id == ana && h.alive));
}

/// A boss's counter ("Guards left 5/8") reaches a client with a HUD as a bar that does not count
/// down (negative seconds).
#[test]
fn a_boss_counter_reaches_the_hud_as_a_bar_that_does_not_count() {
    let (mut game, _client, _saves) = game();
    let hero = game.hero_id(0);
    let screens = Screens {
        cards: true,
        run_end: true,
        hud: true,
    };
    game.ask(hero, hello(screens));
    let ev = esurv_sim::game::run::Event::CounterShown { boss: 7, key: 91, label: "Guards left 5/8".into() };
    let t = told(&game.present(&[ev]).1);
    let [ToClient::BossTimer(b)] = &t[..] else { panic!("{t:?}") };
    assert_eq!((b.boss, b.key, b.label.as_str()), (7, 91, "Guards left 5/8"));
    assert!(b.seconds < 0.0);
}

/// An assist's hits count on a row of its own per character, by the character's name; the pets
/// an ally summoned count on its row too.
#[test]
fn the_meter_counts_each_assist_on_its_own_row() {
    let (mut game, _client, _) = game();
    let hero = game.hero_id(0);
    game.ask(hero, hello(Screens::default()));
    let characters: Vec<(String, String)> = game.run.content.player_characters.iter().take(2).map(|c| (c.id.clone(), c.name.clone())).collect();
    let hit = |amount, source| Event::EnemyHit { id: 1, amount, crit: false, by: Some(hero), source };
    let events = [
        hit(10.0, HitSource::Assist(characters[0].0.clone())),
        hit(4.0, HitSource::Assist(characters[1].0.clone())),
        hit(5.0, HitSource::Assist(characters[0].0.clone())),
    ];
    let t = told(&game.settle(&events));
    let Some(ToClient::Meter(m)) = t.iter().find(|m| matches!(m, ToClient::Meter(_))) else {
        panic!("{t:?}")
    };
    let mut done: Vec<(Source, u64)> = m.done.iter().map(|e| (e.source.clone(), e.amount)).collect();
    done.sort_by_key(|(_, a)| std::cmp::Reverse(*a));
    assert_eq!(done, [(Source::Assist(characters[0].1.clone()), 15), (Source::Assist(characters[1].1.clone()), 4)]);
}

/// The phase marks come from the bosses' `Health(..)` lines, labelled by their "soon" lines:
/// Hogger's gnoll pack at half health, Mr. Smite's two weapon swaps, Mother Fang none.
#[test]
fn a_boss_bar_marks_its_health_phases() {
    let content = esurv_content::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
    let elwynn = content.zones.iter().find(|z| z.id == "elwynn").unwrap();
    let boss = |name: &str| elwynn.bosses.iter().find(|b| b.name == name).unwrap();
    let hogger = super::boss_phases(boss("Hogger"));
    assert_eq!(hogger, vec![protocol::BossPhase { at: 50.0, label: "Gnoll pack".into() }]);
    let smite: Vec<f32> = super::boss_phases(boss("Mr. Smite")).iter().map(|p| p.at).collect();
    assert_eq!(smite, vec![66.0, 33.0], "highest first");
    assert!(super::boss_phases(boss("Mother Fang")).is_empty());
    // every boss of every zone: marks inside the bar, highest first
    for z in &content.zones {
        for b in &z.bosses {
            let p = super::boss_phases(b);
            assert!(p.iter().all(|p| p.at > 0.0 && p.at < 100.0), "{}: {p:?}", b.name);
            assert!(p.windows(2).all(|w| w[0].at > w[1].at), "{}: {p:?}", b.name);
        }
    }
}

/// A Challenge Mode run pays only its class gear, as in Normal Mode, and the talent
/// experience the same run in Ahn'Qiraj would, × 1.1: no records, no zone won or opened, nothing
/// for the codex.
#[test]
fn a_challenge_run_pays_only_its_gear_and_its_talent_experience() {
    use esurv_sim::game::challenge::{Challenge, CHALLENGE_ZONE};
    use esurv_sim::game::choices::Offer;
    use esurv_sim::game::offers::Rarity;
    use esurv_sim::game::rewards::{talent_experience_parts, RunTally};
    let (mut game, _client, saves) = game();
    let hero = game.hero_id(0);
    let end = Screens {
        run_end: true,
        ..Default::default()
    };
    game.ask(hero, hello(end));
    let content = game.run.content.clone();
    game.run.options.challenge = Challenge::new(&content, 300);
    game.start();
    // a ring taken: not for the codex
    let ring = Event::OfferTaken {
        hero,
        offer: Offer::NewJewelry {
            id: "accuria".into(),
            value: 0.17,
            rarity: Rarity::Common,
        },
    };
    game.settle(&[ring]);
    assert!(saves.codex().is_empty(), "nothing for the codex");
    // a gear piece dropped: into the collection as in Normal Mode
    let entry = esurv_sim::game::rewards::gear_item_entry(8, 1, 0);
    let drop = [Event::GearDropped {
        hero,
        slot: 1,
        tier: 0,
        entry,
    }];
    game.hand_out_loot(&drop);
    assert!(saves.collection(&content).contains(&entry), "gear drops count");
    // a victory in Elwynn
    game.run.now = 990.0;
    game.run.won_at = Some(990.0);
    game.run.phase = Phase::Victory;
    let t = after_tick(&mut game);
    let Some(ToClient::RunEnd(won)) = t.iter().find(|m| matches!(m, ToClient::RunEnd(_))) else {
        panic!("{t:?}")
    };
    assert_eq!(won.zone_opened, None, "no zone opened");
    let guid = game.saved(hero).unwrap().guid;
    assert_eq!(saves.hero(guid).unwrap().records, crate::save::HeroRecords::default(), "no records");
    let progress = saves.class_progress(&content, "mage");
    assert!(progress.zones_won.is_empty() && progress.open_zones(&content) == 1, "no zone won");
    // talent experience: Ahn'Qiraj's for the same run, × 1.1, never the slider's 3x
    let tally = RunTally { seconds: 990.0, kills: game.run.heroes[0].kills, bosses: game.run.bosses_slain, won: true };
    let aq = content.zone(CHALLENGE_ZONE).unwrap().difficulty;
    let normal = esurv_sim::game::enemy::RunOptions::default();
    let in_aq = talent_experience_parts(&tally, &content.balance.zone_scaling, aq, &normal, &content.run_options).total();
    let paid = progress.talent_experience;
    assert!(paid > 0 && (paid as f32 - in_aq as f32 * 1.1).abs() <= 2.0, "{paid} vs {in_aq} × 1.1");
}
