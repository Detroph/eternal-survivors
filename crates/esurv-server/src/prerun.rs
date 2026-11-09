//! What the character screen asks before a run ([`PreRunAsk`]: the menus before a run),
//! answered from the save with the same rules as in a run: what a class has
//! earned and its armory changes, the run options, the codex, and a hero's records, name, race,
//! look and zone. The run's own screens answer their armory requests through the same functions
//! ([`progress`], [`armory_change`]).
//!
//! A local game asks straight through [`answer`]; a carrier to a remote host can bring the same
//! asks as lines (`ESURVQ`).

use esurv_content::Content;
use esurv_protocol::{self as protocol, BuildAction, HeroFacts, PreRunAsk, ToClient, ToServer};
use esurv_sim::game::gear::decode_item_entry;
use esurv_sim::game::rewards::{
    option_bonus, prestige_point_cost, prestige_points, talent_point_cost, talent_points,
    MAX_TALENT_POINTS,
};

use crate::ground::GameData;
use crate::save::{SavedHero, SavedRunOptions, Saves, MAX_HEROES_PER_FACTION};

/// The talent experience the next point costs; 0 once every point is earned.
pub fn next_point_cost(points: u32) -> u32 {
    if points < MAX_TALENT_POINTS {
        talent_point_cost(points + 1)
    } else {
        0
    }
}

/// `class`'s `ChrClasses.dbc` id.
pub fn wow_class(content: &Content, class: &str) -> Option<u32> {
    content.class(class).map(|c| c.wow_class_id)
}

/// What `class` has earned, as the screens are told it.
pub fn progress(content: &Content, saves: &Saves, class: &str) -> ToClient {
    let p = saves.class_progress(content, class);
    let (new_gear, new_points) = p.unseen();
    let (points, into_next) = talent_points(p.talent_experience);
    let wow_class = wow_class(content, class);
    ToClient::Progress(protocol::Progress {
        class: class.into(),
        talent_experience: p.talent_experience,
        points,
        spent: p.talents.values().sum(),
        into_next,
        next_cost: next_point_cost(points),
        zones_open: p.open_zones(content) as u32,
        zones_discovered: saves.zones_discovered(content),
        talents: p.talents.into_iter().collect(),
        worn: p.worn,
        looks: p.appearance.into_iter().collect(),
        collection: saves
            .collection(content)
            .into_iter()
            .filter(|&e| decode_item_entry(e).map(|(c, _, _)| c) == wow_class)
            .collect(),
        prestige: prestige_progress(points, p.talent_experience, p.prestige),
        loadout: content
            .loadout(class, p.loadout.as_deref())
            .map(|l| l.id.clone())
            .unwrap_or_default(),
        new_gear,
        new_points,
        challenge_open: saves.challenge_open(content, class),
        challenge_news: saves.challenge_news(content),
        builds: p
            .builds
            .into_iter()
            .map(|(slot, b)| protocol::SavedBuild {
                slot,
                name: b.name,
                start: b.start.unwrap_or_default(),
                loadout: b.loadout.unwrap_or_default(),
                talents: b.talents.into_iter().collect(),
                prestige: b.prestige.into_iter().collect(),
            })
            .collect(),
        active_build: p.active_build,
    })
}

/// A class's prestige as the screens are told it: nothing to earn before every talent point is.
fn prestige_progress(
    talent_points: u32,
    talent_experience: u32,
    ranks: std::collections::BTreeMap<String, u32>,
) -> protocol::PrestigeProgress {
    let (points, into_next) = prestige_points(talent_experience);
    protocol::PrestigeProgress {
        points,
        spent: ranks.values().sum(),
        into_next,
        next_cost: if talent_points >= MAX_TALENT_POINTS {
            prestige_point_cost(points + 1)
        } else {
            0
        },
        ranks: ranks.into_iter().collect(),
    }
}

/// The run options as the screens are told them, with the reward bonus they give.
pub fn run_options(content: &Content, o: SavedRunOptions) -> ToClient {
    ToClient::RunOptions {
        options: o.into(),
        bonus: option_bonus(&o.run_options(&content.run_options), &content.run_options),
    }
}

/// The codex as the save keeps it.
pub fn codex(saves: &Saves) -> ToClient {
    ToClient::Codex {
        found: saves.codex(),
    }
}

/// The warband's achievements as the save keeps them.
pub fn achievements(content: &Content, saves: &Saves) -> ToClient {
    ToClient::Achievements(saves.achievements_told(content))
}

/// A hero's records and the zone its next run is in.
pub fn hero_facts(content: &Content, saves: &Saves, h: &SavedHero) -> ToClient {
    let [title, tabard, glow] = saves.wears_told(content, h);
    let zone = h
        .zone
        .as_deref()
        .filter(|id| content.zones.iter().any(|z| z.id == *id))
        .or_else(|| content.zones.first().map(|z| z.id.as_str()))
        .unwrap_or_default();
    ToClient::Hero(HeroFacts {
        hero: u64::from(h.guid),
        runs: h.records.runs,
        wins: h.records.wins,
        best_level: h.records.best_level,
        best_seconds: h.records.best_seconds,
        zone: zone.into(),
        start: content
            .class(&h.class)
            .and_then(|c| c.starting_option(h.start.as_deref()))
            .map(|o| o.id.clone())
            .unwrap_or_default(),
        companion: h
            .companion
            .clone()
            .filter(|id| content.companion(id).is_some())
            .unwrap_or_default(),
        title,
        tabard,
        glow,
        temperament: h.temperament.clone().unwrap_or_default(),
        time_period: h.time_period,
    })
}

/// An armory change for `class` (`learn`, `unlearn`, `prestige`, `unprestige`, `resetprestige`,
/// `wear`, `takeoff`, `look`, `loadout`), by the save's rules: whether it was done, or `None` for a request
/// that is no armory change.
pub fn armory_change(
    content: &Content,
    saves: &Saves,
    class: &str,
    change: &ToServer,
) -> Option<bool> {
    Some(match change {
        ToServer::LearnTalent(id) => saves.learn_talent(content, class, id),
        ToServer::UnlearnTalent(id) => saves.unlearn_talent(content, class, id),
        ToServer::LearnPrestige(id) => saves.learn_prestige(content, class, id),
        ToServer::UnlearnPrestige(id) => saves.unlearn_prestige(class, id),
        ToServer::ResetPrestige => saves.reset_prestige(class),
        ToServer::Wear(entry) => {
            saves.wear(content, class, *entry)
        }
        ToServer::TakeOff(slot) => saves.take_off(class, *slot),
        ToServer::SetLook { slot, look } => {
            saves.set_look(content, class, *slot, *look)
        }
        ToServer::ChooseLoadout(id) => saves.choose_loadout(content, class, id),
        _ => return None,
    })
}

/// Whether race `race` may be class `class` (1.12 ids): a stock pair (from the player's
/// `CharBaseInfo.dbc`; every pair passes without the data) or one `races.ron` adds.
pub fn pair_offered(content: &Content, data: Option<&GameData>, race: u8, class: u8) -> bool {
    content.is_added_pair(race, class)
        || data.and_then(|d| d.stock_pair(race, class)).unwrap_or(true)
}

/// The folder saves go to and come from, and the saves in it, as the screens are told them.
pub fn save_files(content: &Content, saves: &Saves) -> ToClient {
    ToClient::SaveFiles {
        folder: saves
            .exports_folder()
            .map(|f| f.display().to_string())
            .unwrap_or_default(),
        files: saves
            .save_files(content)
            .into_iter()
            .map(|(name, heroes)| protocol::SaveFileFacts { name, heroes })
            .collect(),
    }
}

/// Answers `ask` for the game on the save's own machine (a local game's launcher): everything
/// [`answer`] answers, and the save's export and import ([`crate::save::files`]).
pub fn answer_local(
    content: &Content,
    saves: &Saves,
    data: Option<&GameData>,
    ask: &PreRunAsk,
) -> Vec<ToClient> {
    let refused = |why: &dyn std::fmt::Display| {
        eprintln!("esurv: {} refused: {why}", ask.verb());
        vec![ToClient::Refused(ask.refused_verb().into())]
    };
    match ask {
        PreRunAsk::SaveFiles => vec![save_files(content, saves)],
        PreRunAsk::ExportSave => match saves.export() {
            Ok(file) => vec![ToClient::SaveExported(file), save_files(content, saves)],
            Err(e) => refused(&e),
        },
        PreRunAsk::ImportSave { file } => match saves.import(content, file) {
            Ok(backup) => vec![
                ToClient::SaveImported {
                    file: file.clone(),
                    backup,
                },
                save_files(content, saves),
            ],
            Err(e) => refused(&e),
        },
        other => answer(content, saves, data, other),
    }
}

/// Answers `ask` from the save: what the screens are told, a `Refused` naming its verb when it
/// could not be done. This is the answer for any client, a remote host's too, so the save's own
/// file is never read or written through it: export and import are refused here and answered only
/// by [`answer_local`].
pub fn answer(
    content: &Content,
    saves: &Saves,
    data: Option<&GameData>,
    ask: &PreRunAsk,
) -> Vec<ToClient> {
    let refused = || vec![ToClient::Refused(ask.refused_verb().into())];
    if ask.touches_the_save_file() {
        return refused();
    }
    let hero = |guid: u64| u32::try_from(guid).ok().and_then(|g| saves.hero(g));
    match ask {
        PreRunAsk::Facts { class } => {
            if content.class(class).is_none() {
                return refused();
            }
            vec![
                progress(content, saves, class),
                run_options(content, saves.run_options()),
                codex(saves),
                achievements(content, saves),
            ]
        }
        PreRunAsk::Hero(guid) => {
            // a hero not yet given its temperaments is given them now, so the sheet shows them
            let ids: Vec<String> = content.camp_chatter.temperaments.iter().map(|t| t.id.clone()).collect();
            saves.give_temperaments(&ids);
            match hero(*guid) {
                Some(h) => vec![hero_facts(content, saves, &h)],
                None => refused(),
            }
        }
        PreRunAsk::Armory { class, change } => {
            if content.class(class).is_none() {
                return refused();
            }
            match armory_change(content, saves, class, change) {
                Some(true) => vec![progress(content, saves, class)],
                _ => refused(),
            }
        }
        PreRunAsk::RunOptions(o) => {
            let asked = SavedRunOptions::from(*o);
            vec![run_options(content, saves.set_run_options(content, asked))]
        }
        PreRunAsk::Rename { hero: guid, name } => match hero(*guid) {
            Some(h) if saves.rename(h.guid, name).is_ok() => saves
                .hero(h.guid)
                .map(|h| vec![hero_facts(content, saves, &h)])
                .unwrap_or_else(refused),
            _ => refused(),
        },
        PreRunAsk::Reshape {
            hero: guid,
            race,
            gender,
            look,
        } => {
            let Some(h) = hero(*guid) else {
                return refused();
            };
            let (Some(new_race), Some(class)) =
                (content.race_by_wow_id(*race), content.class(&h.class))
            else {
                return refused();
            };
            if *gender > 1 || !pair_offered(content, data, *race, class.wow_class_id as u8) {
                return refused();
            }
            // a hero going over to the other faction needs room in its camp
            let old_faction = content.race(&h.race).map(|r| r.faction);
            if old_faction != Some(new_race.faction)
                && saves.heroes_of_faction(content, new_race.faction) >= MAX_HEROES_PER_FACTION
            {
                return refused();
            }
            if !saves.reshape(h.guid, &new_race.id, *gender, *look) {
                return refused();
            }
            saves
                .hero(h.guid)
                .map(|h| vec![hero_facts(content, saves, &h)])
                .unwrap_or_else(refused)
        }
        PreRunAsk::Zone { hero: guid, zone } => {
            let Some(h) = hero(*guid) else {
                return refused();
            };
            let Some(index) = content.zones.iter().position(|z| z.id == *zone) else {
                return refused();
            };
            if !saves.zone_open_for_next_run(content, &h.class, index) || !saves.choose_zone(h.guid, zone) {
                return refused();
            }
            saves
                .hero(h.guid)
                .map(|h| vec![hero_facts(content, saves, &h)])
                .unwrap_or_else(refused)
        }
        PreRunAsk::Start { hero: guid, option } => {
            let Some(h) = hero(*guid) else {
                return refused();
            };
            // only one of the hero's own class's options
            let ours = content
                .class(&h.class)
                .is_some_and(|c| c.starting_options.iter().any(|o| o.id == *option));
            if !ours || !saves.choose_start(h.guid, option) {
                return refused();
            }
            saves
                .hero(h.guid)
                .map(|h| vec![hero_facts(content, saves, &h)])
                .unwrap_or_else(refused)
        }
        PreRunAsk::Seen { class, view } => {
            if content.class(class).is_none() {
                return refused();
            }
            saves.see(class, *view);
            vec![progress(content, saves, class)]
        }
        PreRunAsk::AchievementsSeen => {
            saves.see_achievements(content);
            vec![achievements(content, saves)]
        }
        PreRunAsk::Track { id, on } => {
            if !saves.track_achievement(content, id, *on) {
                return refused();
            }
            vec![achievements(content, saves)]
        }
        PreRunAsk::Buy { item } => {
            if !saves.buy(content, item) {
                return refused();
            }
            vec![achievements(content, saves)]
        }
        PreRunAsk::Companion {
            hero: guid,
            companion,
        } => {
            let Some(h) = hero(*guid) else {
                return refused();
            };
            let pick = Some(companion.as_str()).filter(|c| !c.is_empty());
            if !saves.choose_companion(content, h.guid, pick) {
                return refused();
            }
            saves
                .hero(h.guid)
                .map(|h| vec![hero_facts(content, saves, &h)])
                .unwrap_or_else(refused)
        }
        PreRunAsk::Temperament { hero: guid, temperament } => {
            let Ok(g) = u32::try_from(*guid) else {
                return refused();
            };
            if !saves.temper(content, g, temperament) {
                return refused();
            }
            // the next hero made: told with the rest once it is
            saves
                .hero(g)
                .map(|h| vec![hero_facts(content, saves, &h)])
                .unwrap_or_default()
        }
        PreRunAsk::TimePeriod { hero: guid, time_period } => {
            let Ok(g) = u32::try_from(*guid) else {
                return refused();
            };
            if !saves.set_time_period(g, *time_period) {
                return refused();
            }
            // the next hero made: told with the rest once it is
            saves
                .hero(g)
                .map(|h| vec![hero_facts(content, saves, &h)])
                .unwrap_or_default()
        }
        PreRunAsk::Dress { hero: guid, kind, id } => {
            let Some(h) = hero(*guid) else {
                return refused();
            };
            let pick = Some(id.as_str()).filter(|c| !c.is_empty());
            if !saves.dress(content, h.guid, *kind, pick) {
                return refused();
            }
            saves
                .hero(h.guid)
                .map(|h| vec![hero_facts(content, saves, &h)])
                .unwrap_or_else(refused)
        }
        PreRunAsk::Decorate { camp, spot, piece } => {
            let piece = Some(piece.as_str()).filter(|p| !p.is_empty());
            if !saves.decorate(content, camp, spot, piece) {
                return refused();
            }
            vec![achievements(content, saves)]
        }
        PreRunAsk::Build { hero: guid, class, slot, action } => {
            // no hero (guid 0), or one of the build's own class
            let h = match *guid {
                0 => None,
                g => match hero(g) {
                    Some(h) if h.class == *class => Some(h),
                    _ => return refused(),
                },
            };
            if content.class(class).is_none() || *slot >= protocol::BUILD_SLOTS {
                return refused();
            }
            let done = match action {
                BuildAction::Save => {
                    let start = h.as_ref().and_then(|h| {
                        content.class(class).and_then(|c| c.starting_option(h.start.as_deref())).map(|o| o.id.clone())
                    });
                    saves.save_build(content, class, *slot, start.as_deref())
                }
                BuildAction::Use => {
                    let (lit, start) = saves.use_build(content, class, *slot);
                    if let (Some(h), Some(start)) = (&h, start) {
                        saves.choose_start(h.guid, &start);
                    }
                    lit
                }
                BuildAction::Clear => saves.clear_build(class, *slot),
                BuildAction::Name(name) => saves.name_build(class, *slot, name),
            };
            if !done {
                return refused();
            }
            let mut told = vec![progress(content, saves, class)];
            told.extend(h.and_then(|h| saves.hero(h.guid)).map(|h| hero_facts(content, saves, &h)));
            told
        }
        PreRunAsk::SaveFiles | PreRunAsk::ExportSave | PreRunAsk::ImportSave { .. } => refused(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::FinishedRun;
    use esurv_sim::game::talents;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    fn told_hero(told: &[ToClient]) -> Option<&HeroFacts> {
        told.iter().find_map(|m| match m {
            ToClient::Hero(h) => Some(h),
            _ => None,
        })
    }

    fn refused(told: &[ToClient]) -> Option<&str> {
        told.iter().find_map(|m| match m {
            ToClient::Refused(v) => Some(v.as_str()),
            _ => None,
        })
    }

    #[test]
    fn a_class_is_told_what_it_has_earned_and_changes_it_by_the_rules() {
        let content = content();
        let saves = Saves::in_memory();
        let told = answer(
            &content,
            &saves,
            None,
            &PreRunAsk::Facts {
                class: "warrior".into(),
            },
        );
        assert!(matches!(&told[0], ToClient::Progress(p) if p.class == "warrior" && p.points == 0));
        assert!(matches!(told[1], ToClient::RunOptions { .. }));
        assert!(matches!(told[2], ToClient::Codex { .. }));
        assert!(matches!(&told[3], ToClient::Achievements(a) if a.score == 0 && a.earned.is_empty()));
        let first = content
            .talent_trees
            .iter()
            .find(|t| t.class == "warrior")
            .unwrap()
            .talents
            .iter()
            .find(|t| t.row == 0)
            .unwrap()
            .id
            .clone();
        let learn = PreRunAsk::armory("warrior", &ToServer::LearnTalent(first.clone())).unwrap();
        assert_eq!(
            refused(&answer(&content, &saves, None, &learn)),
            Some("learn"),
            "no points yet"
        );
        let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
        let run = FinishedRun {
            experience: 145,
            ..Default::default()
        };
        saves.finish_run(&content, bo.guid, &run).unwrap();
        let told = answer(&content, &saves, None, &learn);
        assert!(
            matches!(&told[0], ToClient::Progress(p) if p.spent == 1),
            "{told:?}"
        );
        assert_eq!(
            refused(&answer(
                &content,
                &saves,
                None,
                &PreRunAsk::Facts {
                    class: "bard".into()
                }
            )),
            Some("facts")
        );
    }

    #[test]
    fn a_hero_is_renamed_reshaped_and_sent_to_an_open_zone() {
        let content = content();
        let saves = Saves::in_memory();
        let ana = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        let guid = u64::from(ana.guid);
        let h = answer(&content, &saves, None, &PreRunAsk::Hero(guid));
        assert_eq!(told_hero(&h).map(|h| h.zone.as_str()), Some("elwynn"));

        let rename = PreRunAsk::Rename {
            hero: guid,
            name: "anya".into(),
        };
        assert!(told_hero(&answer(&content, &saves, None, &rename)).is_some());
        assert_eq!(saves.hero(ana.guid).unwrap().name, "Anya");

        // a gnome may be a mage; a tauren may not (stock pairs are open without the data, so the
        // check here is the content's: no tauren mage pair is added, and with no data every
        // stock pair passes)
        let gnome = content.race("gnome").unwrap().wow_race_id;
        let reshape = PreRunAsk::Reshape {
            hero: guid,
            race: gnome,
            gender: 0,
            look: [1, 2, 3, 4, 5],
        };
        assert!(told_hero(&answer(&content, &saves, None, &reshape)).is_some());
        let h = saves.hero(ana.guid).unwrap();
        assert_eq!(
            (h.race.as_str(), h.gender, h.look),
            ("gnome", 0, [1, 2, 3, 4, 5])
        );
        let bad = PreRunAsk::Reshape {
            hero: guid,
            race: gnome,
            gender: 2,
            look: [0; 5],
        };
        assert_eq!(
            refused(&answer(&content, &saves, None, &bad)),
            Some("reshape")
        );

        let duskwood = PreRunAsk::Zone {
            hero: guid,
            zone: content.zones[1].id.clone(),
        };
        assert_eq!(
            refused(&answer(&content, &saves, None, &duskwood)),
            Some("zone"),
            "not opened yet"
        );
        let won = FinishedRun {
            won: true,
            first: true,
            opens_zone: true,
            ..Default::default()
        };
        saves.finish_run(&content, ana.guid, &won).unwrap();
        let told = answer(&content, &saves, None, &duskwood);
        assert_eq!(
            told_hero(&told).map(|h| (h.zone.as_str(), h.wins)),
            Some((content.zones[1].id.as_str(), 1))
        );
    }

    /// A class's progress names the weapon loadout it fights
    /// with (its first until one is chosen); `loadout <id>` chooses one of its own, nothing else.
    #[test]
    fn a_class_chooses_its_weapon_loadout() {
        let content = content();
        let saves = Saves::in_memory();
        let loadout_of = |told: &[ToClient]| match &told[0] {
            ToClient::Progress(p) => p.loadout.clone(),
            other => panic!("{other:?}"),
        };
        let facts = PreRunAsk::Facts { class: "paladin".into() };
        assert_eq!(loadout_of(&answer(&content, &saves, None, &facts)), "off_hand");
        let choose = |id: &str| PreRunAsk::armory("paladin", &ToServer::ChooseLoadout(id.into())).unwrap();
        assert_eq!(loadout_of(&answer(&content, &saves, None, &choose("two_hander"))), "two_hander");
        assert_eq!(refused(&answer(&content, &saves, None, &choose("daggers"))), Some("loadout"));
        assert_eq!(loadout_of(&answer(&content, &saves, None, &facts)), "two_hander", "kept");
    }

    /// A saved build puts back the talents, prestige, weapon loadout and starting option it was
    /// saved with, from whatever the class and hero hold since; an empty slot is only lit; a slot
    /// out of range or a hero of another class is refused.
    #[test]
    fn a_saved_build_comes_back_exactly() {
        let content = content();
        let saves = Saves::in_memory();
        let ana = saves.create("Ana", "paladin", "human", 1, [0; 5]).unwrap();
        let guid = u64::from(ana.guid);
        let run = FinishedRun { experience: 100_000, ..Default::default() };
        saves.finish_run(&content, ana.guid, &run).unwrap();
        let row_0: Vec<String> = content
            .trees_of("paladin")
            .flat_map(|t| t.talents.iter().filter(|t| t.row == 0).map(|t| t.id.clone()))
            .collect();
        assert!(row_0.len() >= 2);
        let ask = |action| PreRunAsk::Build { hero: guid, class: "paladin".into(), slot: 2, action };
        let progress_of = |told: &[ToClient]| match &told[0] {
            ToClient::Progress(p) => p.clone(),
            other => panic!("{other:?}"),
        };
        assert!(saves.learn_talent(&content, "paladin", &row_0[0]));
        assert!(saves.learn_talent(&content, "paladin", &row_0[0]));
        assert!(saves.choose_loadout(&content, "paladin", "two_hander"));
        let options = &content.class("paladin").unwrap().starting_options;
        let last = options.last().unwrap().id.clone();
        assert!(saves.choose_start(ana.guid, &last));
        let saved = progress_of(&answer(&content, &saves, None, &ask(BuildAction::Save)));
        assert_eq!(saved.active_build, 2);
        let build = saved.builds.iter().find(|b| b.slot == 2).unwrap().clone();
        assert_eq!(build.talents, vec![(row_0[0].clone(), 2)]);
        assert_eq!((build.loadout.as_str(), build.start.as_str()), ("two_hander", last.as_str()));

        // something else since: other talents, loadout and starting option, another slot lit
        assert!(saves.unlearn_talent(&content, "paladin", &row_0[0]));
        assert!(saves.unlearn_talent(&content, "paladin", &row_0[0]));
        assert!(saves.learn_talent(&content, "paladin", &row_0[1]));
        assert!(saves.choose_loadout(&content, "paladin", "off_hand"));
        assert!(saves.choose_start(ana.guid, &options[0].id));
        let lit = PreRunAsk::Build { hero: guid, class: "paladin".into(), slot: 0, action: BuildAction::Use };
        let told = answer(&content, &saves, None, &lit);
        assert_eq!(progress_of(&told).active_build, 0);
        assert_eq!(progress_of(&told).talents, vec![(row_0[1].clone(), 1)], "an empty slot changes nothing");

        let told = answer(&content, &saves, None, &ask(BuildAction::Use));
        let used = progress_of(&told);
        assert_eq!((used.active_build, used.talents.clone()), (2, build.talents.clone()));
        assert_eq!(used.loadout, "two_hander");
        assert_eq!(told_hero(&told).map(|h| h.start.as_str()), Some(last.as_str()));

        let named = progress_of(&answer(&content, &saves, None, &ask(BuildAction::Name("  Holy light  ".into()))));
        assert_eq!(named.builds.iter().find(|b| b.slot == 2).map(|b| b.name.as_str()), Some("Holy light"));
        let cleared = progress_of(&answer(&content, &saves, None, &ask(BuildAction::Clear)));
        assert!(cleared.builds.is_empty());
        assert_eq!(refused(&answer(&content, &saves, None, &ask(BuildAction::Clear))), Some("build"), "already empty");

        let far = PreRunAsk::Build { hero: guid, class: "paladin".into(), slot: protocol::BUILD_SLOTS, action: BuildAction::Save };
        assert_eq!(refused(&answer(&content, &saves, None, &far)), Some("build"));
        let other = PreRunAsk::Build { hero: guid, class: "mage".into(), slot: 0, action: BuildAction::Save };
        assert_eq!(refused(&answer(&content, &saves, None, &other)), Some("build"));
    }

    /// A build puts back only what the rules and the points earned allow: a talent the content no
    /// longer has is left out, the rest learned again.
    #[test]
    fn a_build_keeps_the_rules() {
        let content = content();
        let saves = Saves::in_memory();
        let bo = saves.create("Bo", "warrior", "orc", 0, [0; 5]).unwrap();
        let run = FinishedRun { experience: 145, ..Default::default() };
        saves.finish_run(&content, bo.guid, &run).unwrap();
        let first = content.trees_of("warrior").flat_map(|t| &t.talents).find(|t| t.row == 0).unwrap().id.clone();
        assert!(saves.learn_talent(&content, "warrior", &first));
        assert!(saves.save_build(&content, "warrior", 1, None));
        saves.edit_build_for_a_test("warrior", 1, |b| {
            b.talents.insert("no_such_talent".into(), 3);
            *b.talents.get_mut(&first).unwrap() = 5;
        });
        assert!(saves.unlearn_talent(&content, "warrior", &first));
        let (lit, start) = saves.use_build(&content, "warrior", 1);
        assert!(lit);
        assert_eq!(start, None);
        let p = saves.class_progress(&content, "warrior");
        assert_eq!(p.talents.values().sum::<u32>(), talents::points_earned(p.talent_experience).min(5));
        assert!(!p.talents.contains_key("no_such_talent"));
    }

    /// A hero starts with its class's first option until it picks another of its class's; the pick
    /// is kept and told, and another class's option is refused.
    #[test]
    fn a_hero_picks_its_starting_option() {
        let content = content();
        let saves = Saves::in_memory();
        let ana = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        let guid = u64::from(ana.guid);
        let mage = content.class("mage").unwrap();
        let first = mage.starting_options[0].id.clone();
        let told = answer(&content, &saves, None, &PreRunAsk::Hero(guid));
        assert_eq!(told_hero(&told).map(|h| h.start.as_str()), Some(first.as_str()));
        let last = mage.starting_options.last().unwrap().id.clone();
        let pick = PreRunAsk::Start { hero: guid, option: last.clone() };
        let told = answer(&content, &saves, None, &pick);
        assert_eq!(told_hero(&told).map(|h| h.start.as_str()), Some(last.as_str()));
        assert_eq!(saves.hero(ana.guid).unwrap().start.as_deref(), Some(last.as_str()));
        let warriors = content.class("warrior").unwrap().starting_options[0].id.clone();
        let wrong = PreRunAsk::Start { hero: guid, option: warriors };
        assert_eq!(refused(&answer(&content, &saves, None, &wrong)), Some("start"));
        assert_eq!(saves.hero(ana.guid).unwrap().start.as_deref(), Some(last.as_str()), "kept");
    }

    #[test]
    fn a_hero_takes_its_temperaments_on_the_sheet() {
        let content = content();
        let saves = Saves::in_memory();
        let temper = |hero: u64, t: &[&str]| PreRunAsk::Temperament {
            hero,
            temperament: t.iter().map(|s| s.to_string()).collect(),
        };
        // the sheet's Create tells the next hero's first
        assert!(answer(&content, &saves, None, &temper(0, &["grim", "weary"])).is_empty());
        let ana = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        assert_eq!(ana.temperament, Some(vec!["grim".into(), "weary".into()]));
        let bo = saves.create("Bo", "mage", "human", 0, [0; 5]).unwrap();
        assert_eq!(bo.temperament, None, "only the hero it was told for");
        // a hero not yet given any is given two when its facts are asked
        let told = answer(&content, &saves, None, &PreRunAsk::Hero(u64::from(bo.guid)));
        assert_eq!(told_hero(&told).map(|h| h.temperament.len()), Some(2));
        let guid = u64::from(ana.guid);
        let told = answer(&content, &saves, None, &temper(guid, &["curious"]));
        assert_eq!(told_hero(&told).map(|h| h.temperament.clone()), Some(vec!["curious".to_string()]));
        let told = answer(&content, &saves, None, &temper(guid, &[]));
        assert_eq!(told_hero(&told).map(|h| h.temperament.is_empty()), Some(true), "Quiet");
        for wrong in [&["grim", "grim"][..], &["grim", "weary", "curious"], &["sulky"]] {
            assert_eq!(refused(&answer(&content, &saves, None, &temper(guid, wrong))), Some("temper"));
        }
        assert_eq!(saves.hero(ana.guid).unwrap().temperament, Some(vec![]), "kept");
    }

    #[test]
    fn a_hero_takes_its_time_period_on_the_sheet() {
        use esurv_content::TimePeriod;
        let content = content();
        let saves = Saves::in_memory();
        let period = |hero: u64, time_period| PreRunAsk::TimePeriod { hero, time_period };
        // the sheet's Create tells the next hero's first
        assert!(answer(&content, &saves, None, &period(0, TimePeriod::Future)).is_empty());
        let ana = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        assert_eq!(ana.time_period, TimePeriod::Future);
        let told = answer(&content, &saves, None, &PreRunAsk::Hero(u64::from(ana.guid)));
        assert_eq!(told_hero(&told).map(|h| h.time_period), Some(TimePeriod::Future));
        let told = answer(&content, &saves, None, &period(u64::from(ana.guid), TimePeriod::Past));
        assert_eq!(told_hero(&told).map(|h| h.time_period), Some(TimePeriod::Past));
        assert_eq!(saves.hero(ana.guid).unwrap().time_period, TimePeriod::Past, "kept");
        assert_eq!(refused(&answer(&content, &saves, None, &period(99, TimePeriod::Past))), Some("period"));
    }

    /// A zone added with data alone is a zone like the others: told open in its place by
    /// difficulty, accepted as a hero's zone once the zone before it is won, and played.
    #[test]
    fn a_zone_added_as_a_file_is_offered_chosen_and_played() {
        use esurv_content::testing::{content_folder_with_zones, ExtraZone};
        let shipped = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let zones = [
            ExtraZone { id: "test_middle", name: "Test Middle", difficulty: 2.5 },
            ExtraZone { id: "test_end", name: "Test End", difficulty: 11.0 },
        ];
        let dir = content_folder_with_zones(shipped.as_ref(), "prerun-added-zones", &zones).unwrap();
        let content = Content::load(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        let content = std::sync::Arc::new(content.unwrap());
        let index = |id: &str| content.zones.iter().position(|z| z.id == id).unwrap();
        let ids: Vec<&str> = content.zones.iter().map(|z| z.id.as_str()).collect();
        assert_eq!(ids[1..4], ["duskwood", "test_middle", "wsg"]);
        assert_eq!(ids.last(), Some(&"test_end"));
        let saves = std::sync::Arc::new(Saves::in_memory());
        let ana = saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        let guid = u64::from(ana.guid);
        let choose = |zone: &str| PreRunAsk::Zone { hero: guid, zone: zone.into() };
        assert_eq!(refused(&answer(&content, &saves, None, &choose("test_middle"))), Some("zone"));
        let won_in = |id: &str| FinishedRun {
            won: true,
            first: true,
            opens_zone: true,
            zone_index: index(id),
            ..Default::default()
        };
        saves.finish_run(&content, ana.guid, &won_in("duskwood")).unwrap();
        let told = answer(&content, &saves, None, &choose("test_middle"));
        assert_eq!(told_hero(&told).map(|h| h.zone.as_str()), Some("test_middle"));
        let facts = PreRunAsk::Facts { class: "mage".into() };
        let told = answer(&content, &saves, None, &facts);
        assert!(matches!(&told[0], ToClient::Progress(p) if p.zones_open == 3), "{told:?}");
        // the end zone opens after an `aq` win
        assert_eq!(refused(&answer(&content, &saves, None, &choose("test_end"))), Some("zone"));
        saves.finish_run(&content, ana.guid, &won_in("aq")).unwrap();
        let told = answer(&content, &saves, None, &choose("test_end"));
        assert_eq!(told_hero(&told).map(|h| h.zone.as_str()), Some("test_end"));
        // and a run starts there
        let saved = saves.hero(ana.guid).unwrap();
        let (_host, inputs) = std::sync::mpsc::channel();
        let zone = index("test_end");
        let game = crate::game::Game::new(content.clone(), None, zone, &saved, saves.clone(), inputs);
        assert!(game.is_some(), "a run starts in the added zone");
    }

    #[test]
    fn a_hero_goes_over_to_a_faction_only_with_room_in_its_camp() {
        let content = content();
        let saves = Saves::in_memory();
        for name in ["Aa", "Bb", "Cc", "Dd", "Ee", "Ff", "Gg", "Hh", "Ii"] {
            saves.create(name, "warrior", "orc", 0, [0; 5]).unwrap();
        }
        let hu = saves.create("Hu", "warrior", "human", 0, [0; 5]).unwrap();
        let orc = content.race("orc").unwrap().wow_race_id;
        let over = PreRunAsk::Reshape {
            hero: u64::from(hu.guid),
            race: orc,
            gender: 0,
            look: [0; 5],
        };
        assert_eq!(
            refused(&answer(&content, &saves, None, &over)),
            Some("reshape")
        );
        let dwarf = content.race("dwarf").unwrap().wow_race_id;
        let same_side = PreRunAsk::Reshape {
            hero: u64::from(hu.guid),
            race: dwarf,
            gender: 1,
            look: [0; 5],
        };
        assert!(told_hero(&answer(&content, &saves, None, &same_side)).is_some());
    }

    #[test]
    fn only_the_saves_own_machine_exports_and_imports() {
        let content = content();
        let dir =
            std::env::temp_dir().join(format!("esurv-prerun-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let saves = std::sync::Arc::new(Saves::open(&dir.join("heroes.ron")).unwrap());
        saves.create("Ana", "mage", "human", 1, [0; 5]).unwrap();
        // a remote host refuses every ask on the save's file
        for (ask, verb) in [
            (PreRunAsk::SaveFiles, "savefiles"),
            (PreRunAsk::ExportSave, "export"),
            (
                PreRunAsk::ImportSave {
                    file: "heroes.ron".into(),
                },
                "import",
            ),
        ] {
            assert_eq!(refused(&answer(&content, &saves, None, &ask)), Some(verb));
        }
        // the local game exports, lists and imports
        let told = answer_local(&content, &saves, None, &PreRunAsk::ExportSave);
        let ToClient::SaveExported(file) = &told[0] else {
            panic!("{told:?}");
        };
        let ToClient::SaveFiles { folder, files } = &told[1] else {
            panic!("{told:?}");
        };
        assert!(folder.ends_with("exports"), "{folder}");
        assert_eq!(files[0].name, *file);
        assert_eq!(files[0].heroes, Some(1));
        // never under a run: a hero in the world holds the save
        let in_a_run = saves.enter_world();
        let told = answer_local(
            &content,
            &saves,
            None,
            &PreRunAsk::ImportSave { file: file.clone() },
        );
        assert_eq!(
            refused(&told),
            Some("import"),
            "not while a hero is in a run"
        );
        drop(in_a_run);
        let told = answer_local(
            &content,
            &saves,
            None,
            &PreRunAsk::ImportSave { file: file.clone() },
        );
        assert!(matches!(&told[0], ToClient::SaveImported { file: f, .. } if f == file));
        let told = answer_local(
            &content,
            &saves,
            None,
            &PreRunAsk::ImportSave {
                file: "nothing.ron".into(),
            },
        );
        assert_eq!(refused(&told), Some("import"));
        // the rest is answered as for anyone
        assert!(told_hero(&answer_local(
            &content,
            &saves,
            None,
            &PreRunAsk::Hero(u64::from(saves.heroes()[0].guid))
        ))
        .is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
