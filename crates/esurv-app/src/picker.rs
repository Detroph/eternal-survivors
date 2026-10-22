//! The camps of the warband screen: the character select is a camp
//! scene, one per faction. The Horde's heroes wait by a campfire before the gate of `orgrimmar` at
//! night, the Alliance's by a campfire beside the gate of Stormwind ([`scenes`]); every hero waits
//! by the campfire in its class's worn gear, and the picked one stands up. The menus before a run
//! ([`crate::menus`]) draw the warband screen over the camp shown and switch the camps.
//!
//! The scene is the world itself, through benilla's select-scene hooks
//! ([`benilla_app::SelectScene`], [`benilla_app::SceneFigure`], [`benilla_app::SelectScreen`]).

mod pointer;
pub mod scenes;

use benilla_app::{
    FigureEmote, FigureHero, SceneFigure, SelectCharacter, SelectCreateAllowed, SelectListFilter,
    SelectScene, SelectScreen,
};
use benilla_protocol::Character;
use bevy::prelude::*;
use esurv_content::{Content, Faction};
use esurv_protocol::{MAX_HEROES, MAX_HEROES_PER_FACTION};

use crate::ui::GameContent;
use scenes::{greeting, FactionScene, Place, PICKED_POSE};

/// The camp shown and who stands in it.
#[derive(Resource)]
pub(crate) struct PickerState {
    pub(crate) faction: Faction,
    /// The selection last seen, to follow a pick into the other camp once.
    last_selected: Option<(u64, u8)>,
    /// The heroes standing, by roster guid, in their places' order, and the campfire.
    heroes: Vec<(u64, Entity, Place)>,
    /// What the heroes standing were made from (camp, race, sex, look, gear), so a hero changed
    /// on the hero sheet stands anew.
    stood_for: u64,
    campfire: Option<(Faction, Entity)>,
    /// The back hero that last greeted on being picked, so it greets once per pick.
    greeted: Option<u64>,
    /// When the heroes last stood anew: a greeting waits for their models.
    stood_at: f32,
}

impl PickerState {
    /// The figure standing for the hero of `guid` in the camp shown, if it stands there.
    pub(crate) fn figure_of(&self, guid: u64) -> Option<Entity> {
        self.heroes
            .iter()
            .find(|(g, _, _)| *g == guid)
            .map(|(_, e, _)| *e)
    }
}

/// `ESURV_PICKER_FACTION=horde|alliance`: the camp shown first (a dev aid; else the picked
/// hero's).
fn faction_from(value: Option<&str>) -> Option<Faction> {
    match value?.trim().to_ascii_lowercase().as_str() {
        "horde" => Some(Faction::Horde),
        "alliance" => Some(Faction::Alliance),
        _ => None,
    }
}

/// A roster row's faction, by its race in the content.
pub(crate) fn faction_of(content: &Content, c: &Character) -> Option<Faction> {
    content.race_by_wow_id(c.race).map(|r| r.faction)
}

/// The other camp.
pub(crate) fn other(f: Faction) -> Faction {
    match f {
        Faction::Horde => Faction::Alliance,
        Faction::Alliance => Faction::Horde,
    }
}

/// The camp's place, with the dev switches' corrections.
pub(crate) fn faction_scene(f: Faction) -> FactionScene {
    scenes::scene(f).with_overrides(
        std::env::var("ESURV_PICKER_CAMPFIRE").ok().as_deref(),
        std::env::var("ESURV_PICKER_BEARING").ok().as_deref(),
        std::env::var("ESURV_PICKER_FRAMING").ok().as_deref(),
    )
}

/// The warband screen's aim, with the dev switch's correction (`ESURV_WARBAND_AIM=<aside>,
/// <lift>`, yards).
fn warband_aim() -> (f32, f32) {
    std::env::var("ESURV_WARBAND_AIM")
        .ok()
        .and_then(|v| {
            let (a, l) = v.split_once(',')?;
            Some((a.trim().parse().ok()?, l.trim().parse().ok()?))
        })
        .unwrap_or(scenes::WARBAND_AIM)
}

/// The sheath type of a weapon loadout's weapon by its look (in any zone's armaments, with its
/// own grip there), for the figures' held items; 0 for any other.
pub(crate) fn weapon_sheath(content: &Content, display_id: u32) -> u8 {
    content
        .loadouts
        .iter()
        .flat_map(|c| &c.loadouts)
        .flat_map(|l| &l.weapons)
        .find_map(|w| w.looks.iter().find(|a| a.display == display_id).map(|a| w.kind_of(a)))
        .map_or(0, |k| k.sheath().min(255) as u8)
}

pub struct PickerPlugin;

impl Plugin for PickerPlugin {
    fn build(&self, app: &mut App) {
        let first = faction_from(std::env::var("ESURV_PICKER_FACTION").ok().as_deref());
        app.insert_resource(PickerState {
            faction: first.unwrap_or(Faction::Horde),
            last_selected: None,
            stood_for: 0,
            heroes: Vec::new(),
            campfire: None,
            greeted: None,
            stood_at: 0.0,
        })
        .add_systems(
            Update,
            (follow_selection, show_scene, list_the_camp, stand_heroes).chain(),
        );
        shot::register(app);
        pointer::register(app);
    }
}

/// A hero picked in the other camp (in the list, or the first roster), or one gone over to it, brings
/// its camp up.
fn follow_selection(
    screen: Res<SelectScreen>,
    content: Option<Res<GameContent>>,
    mut state: ResMut<PickerState>,
) {
    if !screen.open {
        state.last_selected = None;
        return;
    }
    // the pick, and its race: a hero whose race changes may go over to the other camp
    let now = screen.selected_character().map(|c| (c.guid, c.race));
    if now == state.last_selected {
        return;
    }
    state.last_selected = now;
    let (Some(content), Some(c)) = (content, screen.selected_character()) else {
        return;
    };
    if let Some(f) = faction_of(&content.0, c) {
        if f != state.faction {
            state.faction = f;
        }
    }
}

/// The camp's place behind the select screen.
fn show_scene(
    state: Res<PickerState>,
    content: Option<Res<GameContent>>,
    mut scene: ResMut<SelectScene>,
    menus: Res<benilla_app::SelectScreenReplaced>,
) {
    // Without the content no hero has a faction: the stock screen stays. Under the menus'
    // warband screen the camp sits in the lower left.
    let place = faction_scene(state.faction);
    let view = if menus.0 {
        place.warband_view(warband_aim())
    } else {
        place.view()
    };
    let want = SelectScene(content.map(|_| view));
    if *scene != want {
        *scene = want;
    }
}

/// The camp's heroes, by guid in roster order, as many as the camp holds.
pub(crate) fn camp_heroes(screen: &SelectScreen, content: &Content, faction: Faction) -> Vec<u64> {
    screen
        .characters
        .iter()
        .filter(|c| faction_of(content, c) == Some(faction))
        .take(FactionScene::room())
        .map(|c| c.guid)
        .collect()
}

/// The stock list shows only the camp's heroes: switching camps
/// swaps the list, and the pick follows the camp ([`switch_camp`], [`follow_selection`]).
fn list_the_camp(
    screen: Res<SelectScreen>,
    content: Option<Res<GameContent>>,
    state: Res<PickerState>,
    mut filter: ResMut<SelectListFilter>,
    mut create: ResMut<SelectCreateAllowed>,
) {
    let want = SelectListFilter(
        content
            .as_ref()
            .filter(|_| screen.open)
            .map(|c| camp_heroes(&screen, &c.0, state.faction)),
    );
    if *filter != want {
        *filter = want;
    }
    // Create while the camp shown has room and the save too
    let allowed = SelectCreateAllowed(content.as_ref().filter(|_| screen.open).map(|c| {
        let camp = screen
            .characters
            .iter()
            .filter(|h| faction_of(&c.0, h) == Some(state.faction))
            .count();
        may_create(camp, screen.characters.len())
    }));
    if *create != allowed {
        *create = allowed;
    }
}

/// Whether a hero may be made: the camp shown has fewer than nine and the save fewer than 18.
fn may_create(camp: usize, total: usize) -> bool {
    camp < MAX_HEROES_PER_FACTION && total < MAX_HEROES
}

/// The camp's heroes by the campfire: each at its place in the pose of its place; a front hero
/// stands while hovered or picked, a back hero greets once when picked; gone when the select
/// screen goes.
fn stand_heroes(
    mut commands: Commands,
    screen: Res<SelectScreen>,
    held: Option<Res<shot::HeldPick>>,
    content: Option<Res<GameContent>>,
    mut state: ResMut<PickerState>,
    mut figures: Query<&mut SceneFigure>,
    mut greetings: MessageWriter<FigureEmote>,
    time: Res<Time>,
) {
    let content = content.map(|c| c.0.clone());
    let wanted: Vec<&Character> = match (&content, screen.open) {
        (Some(content), true) => screen
            .characters
            .iter()
            .filter(|c| faction_of(content, c) == Some(state.faction))
            .take(FactionScene::room())
            .collect(),
        _ => Vec::new(),
    };
    let place = faction_scene(state.faction);
    // who stands: the picked hero, or the one a dev switch holds picked
    let standing = held
        .as_deref()
        .and_then(|h| {
            screen
                .characters
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(&h.0))
        })
        .map(|c| c.guid)
        .or(screen.selected);
    // a held pick (a dev switch) makes a fixed picture: the mouse stands nobody up
    let hovered = if held.is_some() { None } else { screen.hovered };

    // the campfire burns while the screen is up
    let campfire_wanted = screen.open && content.is_some();
    match state.campfire {
        Some((f, e)) if !campfire_wanted || f != state.faction => {
            commands.entity(e).despawn();
            state.campfire = None;
        }
        _ => {}
    }
    if campfire_wanted && state.campfire.is_none() {
        let e = commands
            .spawn((
                Name::new("picker campfire"),
                SceneFigure::Prop {
                    display_id: place.campfire_display,
                },
                SceneFigure::place(place.campfire, 0.0),
            ))
            .id();
        state.campfire = Some((state.faction, e));
    }

    // a different set of heroes (a camp switched, a hero made or deleted) stands anew
    let made_from = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (state.faction == Faction::Horde).hash(&mut h);
        for c in &wanted {
            let look = [c.skin, c.face, c.hair_style, c.hair_color, c.facial_hair];
            (
                c.guid,
                c.race,
                c.gender,
                look,
                c.equipment.map(|e| e.display_id),
            )
                .hash(&mut h);
        }
        h.finish()
    };
    let same = made_from == state.stood_for
        && wanted.len() == state.heroes.len()
        && wanted
            .iter()
            .zip(&state.heroes)
            .all(|(c, (g, _, _))| c.guid == *g);
    if !same {
        state.stood_for = made_from;
        for (_, e, _) in state.heroes.drain(..) {
            commands.entity(e).despawn();
        }
        let Some(content) = &content else {
            return;
        };
        for (i, c) in wanted.iter().enumerate() {
            let Some((at, facing, spot)) = place.slot(i, wanted.len()) else {
                break;
            };
            let held = [15, 16, 17].map(|s| weapon_sheath(content, c.equipment[s].display_id));
            let up = spot == Place::Front && (standing == Some(c.guid) || hovered == Some(c.guid));
            let pose = if up { PICKED_POSE } else { place.pose(spot) };
            let e = commands
                .spawn((
                    Name::new(format!("picker hero {}", c.name)),
                    SceneFigure::Hero(FigureHero {
                        character: (*c).clone(),
                        stand_state: pose.stand_state(),
                        held_sheath: held,
                        start_outfit_under: true,
                    }),
                    SceneFigure::place(at, facing),
                ))
                .id();
            state.heroes.push((c.guid, e, spot));
        }
        state.stood_at = time.elapsed_secs();
        state.greeted = None;
        return;
    }

    // the same heroes: a front one stands while hovered or picked, the rest wait in the pose of
    // their place
    for &(guid, e, spot) in &state.heroes {
        let up = spot == Place::Front && (standing == Some(guid) || hovered == Some(guid));
        let want = if up { PICKED_POSE } else { place.pose(spot) }.stand_state();
        if let Ok(mut f) = figures.get_mut(e) {
            let differs = matches!(&*f, SceneFigure::Hero(h) if h.stand_state != want);
            if differs {
                if let SceneFigure::Hero(h) = &mut *f {
                    h.stand_state = want;
                }
            }
        }
    }

    // a back hero greets once when picked, after the models have had time to come in
    let picked_back = state
        .heroes
        .iter()
        .find(|(g, _, p)| standing == Some(*g) && *p == Place::Back)
        .map(|(g, e, _)| (*g, *e));
    match picked_back {
        Some((guid, e)) if state.greeted != Some(guid) => {
            if time.elapsed_secs() - state.stood_at < GREET_AFTER {
                return;
            }
            let class = content.as_ref().and_then(|content| {
                let c = screen.characters.iter().find(|c| c.guid == guid)?;
                content
                    .classes
                    .iter()
                    .find(|k| k.wow_class_id == u32::from(c.class))
                    .map(|k| k.id.clone())
            });
            if let Some(class) = class {
                info!(
                    "picker: {class} hero {guid:#x} greets ({:?})",
                    greeting(&class)
                );
                greetings.write(FigureEmote {
                    figure: e,
                    anim_id: greeting(&class) as u16,
                });
            }
            state.greeted = Some(guid);
        }
        Some(_) => {}
        None => state.greeted = None,
    }
}

/// Seconds after the heroes stand anew before a picked back hero greets, so its model is in.
const GREET_AFTER: f32 = 2.0;

/// The first hero of a camp, to pick when the camp is switched to.
pub(crate) fn first_of(screen: &SelectScreen, content: &Content, f: Faction) -> Option<u64> {
    screen
        .characters
        .iter()
        .find(|c| faction_of(content, c) == Some(f))
        .map(|c| c.guid)
}

/// Switches the camp, picking its first hero when it has one.
pub(crate) fn switch_camp(
    state: &mut PickerState,
    screen: &SelectScreen,
    content: Option<&Content>,
    picks: &mut MessageWriter<SelectCharacter>,
) {
    state.faction = other(state.faction);
    if let Some(guid) = content.and_then(|c| first_of(screen, c, state.faction)) {
        state.last_selected = screen
            .characters
            .iter()
            .find(|c| c.guid == guid)
            .map(|c| (c.guid, c.race));
        picks.write(SelectCharacter(guid));
    }
}

/// The live check's picture (`ESURV_PICKER_SHOT=<png>`): once the camp has loaded and its
/// heroes have had time to come in, one shot of the window, then the game closes. A dev aid.
mod shot {
    use bevy::app::AppExit;
    use bevy::prelude::*;
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};

    use benilla_app::SelectScreen;

    /// Seconds after the camp shows before the shot, so the models are in and posed.
    const SETTLE: f32 = 8.0;

    #[derive(Resource)]
    struct Shot {
        path: String,
        ready_at: Option<f32>,
        shot_at: Option<f32>,
    }

    pub(super) fn register(app: &mut App) {
        if let Ok(path) = std::env::var("ESURV_PICKER_SHOT") {
            info!("picker: a shot of the camp goes to {path} (ESURV_PICKER_SHOT)");
            app.insert_resource(Shot {
                path,
                ready_at: None,
                shot_at: None,
            })
            .add_systems(Update, take_shot);
        }
        if let Ok(name) = std::env::var("ESURV_PICKER_PICK") {
            info!("picker: {name} stays picked (ESURV_PICKER_PICK)");
            app.insert_resource(HeldPick(name))
                .add_systems(Update, hold_pick);
        }
    }

    /// `ESURV_PICKER_PICK=<name>`: the hero kept picked whatever is clicked, for a shot no
    /// stray click changes.
    #[derive(Resource)]
    pub(super) struct HeldPick(pub(super) String);

    fn hold_pick(
        held: Res<HeldPick>,
        screen: Res<SelectScreen>,
        mut picks: MessageWriter<benilla_app::SelectCharacter>,
    ) {
        if !screen.open {
            return;
        }
        let Some(c) = screen
            .characters
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(&held.0))
        else {
            return;
        };
        if screen.selected != Some(c.guid) {
            picks.write(benilla_app::SelectCharacter(c.guid));
        }
    }

    fn take_shot(
        mut commands: Commands,
        time: Res<Time>,
        screen: Res<SelectScreen>,
        mut shot: ResMut<Shot>,
        mut exit: MessageWriter<AppExit>,
    ) {
        let now = time.elapsed_secs();
        if let Some(t) = shot.shot_at {
            if now > t + 2.0 {
                exit.write(AppExit::Success);
            }
            return;
        }
        if !screen.scene_ready {
            shot.ready_at = None;
            return;
        }
        let ready_at = *shot.ready_at.get_or_insert(now);
        let settle = std::env::var("ESURV_PICKER_SHOT_AT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(SETTLE);
        if now - ready_at < settle {
            return;
        }
        info!("picker: writing {}", shot.path);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(shot.path.clone()));
        shot.shot_at = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_camp_is_read_from_its_switch() {
        assert_eq!(faction_from(Some("Alliance")), Some(Faction::Alliance));
        assert_eq!(faction_from(Some("horde ")), Some(Faction::Horde));
        assert_eq!(faction_from(Some("scourge")), None);
        assert_eq!(other(Faction::Horde), Faction::Alliance);
    }

    fn hero(guid: u64, race: u8) -> Character {
        Character {
            guid,
            name: format!("H{guid}"),
            race,
            class: 1,
            gender: 0,
            skin: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            facial_hair: 0,
            level: 1,
            zone: 0,
            map: 0,
            position: benilla_protocol::wire::Vector3d {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            flags: 0,
            equipment: [Default::default(); 19],
            pet_display_id: 0,
            pet_level: 0,
            pet_family: 0,
        }
    }

    #[test]
    fn create_is_offered_while_the_camp_and_the_save_have_room() {
        assert!(may_create(8, 17));
        assert!(!may_create(9, 9), "the camp is full");
        assert!(!may_create(3, 18), "the save is full");
        assert!(may_create(0, 9), "the other camp still has room");
    }

    #[test]
    fn the_list_shows_the_camp_and_the_pick_follows_it() {
        let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        // orcs (race 2) and humans (race 1), mixed in the roster, ten orcs in all
        let mut characters: Vec<Character> = (1..=10).map(|g| hero(g, 2)).collect();
        characters.insert(3, hero(50, 1));
        characters.push(hero(51, 1));
        let screen = SelectScreen {
            open: true,
            characters,
            selected: Some(1),
            realm: None,
            scene_ready: true,
            hovered: None,
        };
        let horde = camp_heroes(&screen, &content, Faction::Horde);
        assert_eq!(
            horde,
            (1..=9).collect::<Vec<u64>>(),
            "the first nine orcs, in roster order"
        );
        assert_eq!(
            camp_heroes(&screen, &content, Faction::Alliance),
            vec![50, 51]
        );
        assert_eq!(first_of(&screen, &content, Faction::Alliance), Some(50));
    }

    #[test]
    fn a_class_weapon_is_held_by_its_own_sheath() {
        let content = Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let w = &content.loadout("warrior", None).unwrap().weapons[0];
        assert_eq!(weapon_sheath(&content, w.looks[0].display), w.kind.sheath() as u8);
        // an axe in a line of two-handed swords rests as a two-hander too
        let axe = w.looks.iter().find(|a| a.kind.is_some()).unwrap();
        assert_eq!(weapon_sheath(&content, axe.display), w.kind_of(axe).sheath() as u8);
        assert_eq!(weapon_sheath(&content, 0), 0);
    }
}
