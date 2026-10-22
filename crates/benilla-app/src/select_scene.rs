//! A character select drawn over the game's own world, and drawn by the embedder: hooks for a
//! crate built on benilla ([`crate::run_with`]). Stock benilla never sets them, and then nothing
//! here changes a thing.
//!
//! * [`SelectScene`]: while the select screen is up and no character is entering the world, the
//!   world streams around the scene's map and point, the world camera looks from `eye` at
//!   `look_at` once the place has loaded, the clock reads `minute_of_day`, and the select screen
//!   drops its painted backdrop and its model (the glue booth), so the world shows behind its
//!   frames. Create, the login screen and world entry tear it down as the world always is.
//! * [`OutfitWeaponSheaths`]: where a hero figure's starting-outfit weapon rests when put away.
//! * [`SceneFigure`]: a character from its roster row, or a game object, standing in that world
//!   with no wire object behind it, standing on the terrain under it once that has loaded (a
//!   height within a few yards of the terrain's means the ground). benilla's own unit pipeline
//!   draws, dresses, lights and poses it: a hero's stand state is its `UNIT_FIELD_BYTES_1`
//!   stand-state byte, so going from kneeling (8) to standing (0) plays KneelEnd, then Stand. A
//!   hero figure carries its head's point in its pose ([`FigureOverhead`]), for a name over it.
//! * [`SelectScreen`] (read) and [`SelectCharacter`] (write): the roster, the selected row and the
//!   realm, and a way to select a row from the embedder's own screen.
//! * [`SelectScreenReplaced`]: the embedder's own select screen in place of the stock one, with
//!   its requests ([`SelectEnterWorld`], [`SelectCreate`], [`SelectDelete`], [`SelectRefresh`]) and
//!   their answers ([`SelectActionDone`]); [`SelectModel`] shows a character in the glue booth
//!   ([`SelectModelImage`]), and [`CreateChoices`] says what the create screen would offer.
//!
//! The figures are the embedder's: it spawns them and despawns them when the screen goes
//! ([`SelectScreen::open`]).

use benilla_assets::coords::wow_to_bevy;
use benilla_protocol::messages::ObjectFields;
use benilla_protocol::{CharEnumItem, Character, EntityKind};
use bevy::prelude::*;

use benilla_world::terrain_stream::WorldLoadProgress;
use benilla_world::view::WorldCamera;
use benilla_world::world_map::CurrentMap;

use crate::char_select::{ClientState, Roster};
use crate::entities::{equip_slot, CharCreate, FigureItems};
use crate::items::HeldTemplate;
use crate::net::{Guid, NetEntity, ObjectStore};

/// Where the select screen's world is and how it is seen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneView {
    /// `Map.dbc` id (0 Eastern Kingdoms, 1 Kalimdor).
    pub map: u32,
    /// The camera, in WoW world coordinates.
    pub eye: [f32; 3],
    /// What it looks at, in WoW world coordinates; the world streams around this point.
    pub look_at: [f32; 3],
    /// The game clock the scene is lit by, minutes after midnight (`0..1440`).
    pub minute_of_day: u32,
}

/// The embedder's scene for the select screen; `None` (the default) keeps the stock screen.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct SelectScene(pub Option<SceneView>);

/// The scene in effect this frame: [`SelectScene`] while the select screen is up and no pick is
/// on its way into the world, and whether its place has loaded (the camera waits for that).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ShownScene {
    pub(crate) view: Option<SceneView>,
    pub(crate) ready: bool,
}

/// The select screen as an embedder sees it, rewritten when it changes.
#[derive(Resource, Clone, Debug, Default)]
pub struct SelectScreen {
    /// The select screen is up.
    pub open: bool,
    /// The roster, in list order.
    pub characters: Vec<Character>,
    /// The selected row's guid.
    pub selected: Option<u64>,
    /// The realm the roster is on.
    pub realm: Option<String>,
    /// The scene's place has loaded and the world shows behind the screen.
    pub scene_ready: bool,
    /// The hero figure under the mouse, by its guid.
    pub hovered: Option<u64>,
}

impl SelectScreen {
    /// The selected roster row.
    pub fn selected_character(&self) -> Option<&Character> {
        let guid = self.selected?;
        self.characters.iter().find(|c| c.guid == guid)
    }

    /// What an embedder reads off it, for the change check (a [`Character`] has no `PartialEq`).
    fn same_as(&self, other: &Self) -> bool {
        let row = |c: &Character| {
            (
                c.guid,
                c.name.clone(),
                c.race,
                c.class,
                c.gender,
                [c.skin, c.face, c.hair_style, c.hair_color, c.facial_hair],
                c.zone,
                c.flags,
                c.equipment,
            )
        };
        self.open == other.open
            && self.selected == other.selected
            && self.realm == other.realm
            && self.scene_ready == other.scene_ready
            && self.hovered == other.hovered
            && self.characters.len() == other.characters.len()
            && self
                .characters
                .iter()
                .zip(&other.characters)
                .all(|(a, b)| row(a) == row(b))
    }
}

/// The roster rows the select screen lists, by guid; `None` (the default) lists every row. Rows
/// keep their roster order; a guid not on the roster is skipped. The rest of the screen is
/// unchanged: Enter World and Delete act on the selected row, wherever it is listed.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct SelectListFilter(pub Option<Vec<u64>>);

/// Whether the select screen offers Create New Character; `None` (the default) keeps the stock rule,
/// offered below ten characters (`MAX_CHARACTERS_PER_REALM`). An embedder with a rule of its own
/// sets it; the server still refuses a character past its own limit.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct SelectCreateAllowed(pub Option<bool>);

/// The roster indices the list shows, in order.
pub(crate) fn listed_rows(chars: &[Character], filter: &SelectListFilter) -> Vec<usize> {
    match &filter.0 {
        None => (0..chars.len()).collect(),
        Some(guids) => (0..chars.len())
            .filter(|&i| guids.contains(&chars[i].guid))
            .collect(),
    }
}

/// Select the roster row of this guid, as a click on it does; ignored when the screen is not up
/// or no row has it.
#[derive(Message, Clone, Copy, Debug)]
pub struct SelectCharacter(pub u64);

/// A hero standing in the scene, from its roster row.
#[derive(Clone, Debug)]
pub struct FigureHero {
    pub character: Character,
    /// `UNIT_FIELD_BYTES_1` byte 0: 0 standing, 1 sitting, 8 kneeling ...
    pub stand_state: u8,
    /// The sheath type (`Item.dbc` `sheath`) of what is held in the main hand, the off hand and
    /// the ranged slot; the roster row carries only the look and the inventory type. The figure
    /// holds its weapons in hand, as the stock select screen shows a character.
    pub held_sheath: [u8; 3],
    /// A slot the roster row leaves empty shows the race and class's starting outfit piece
    /// (`CharStartOutfit`, what the stock create screen dresses a new character in).
    pub start_outfit_under: bool,
}

/// Where each starting-outfit weapon a hero figure holds rests when the figure puts it away, by
/// its look (`ItemDisplayInfo.dbc` id): the sheath type its stock item carries, which the player's
/// data does not hold. Empty in stock benilla, where such a weapon shows only while drawn.
#[derive(Resource, Clone, Debug, Default)]
pub struct OutfitWeaponSheaths(pub std::collections::HashMap<u32, u8>);

/// Something standing in the select screen's world with no wire object. Put it on an entity with
/// a `Transform` ([`SceneFigure::place`] makes one from WoW coordinates).
#[derive(Component, Clone, Debug)]
pub enum SceneFigure {
    Hero(FigureHero),
    /// A game object by its `GameObjectDisplayInfo.dbc` id (a campfire).
    Prop {
        display_id: u32,
    },
}

impl SceneFigure {
    /// The `Transform` of a figure standing at `wow` (WoW world coordinates) facing `facing`
    /// (radians about WoW +Z, from north toward west, as the wire's orientation).
    pub fn place(wow: [f32; 3], facing: f32) -> Transform {
        Transform {
            translation: wow_to_bevy(wow),
            rotation: Quat::from_rotation_y(facing),
            ..default()
        }
    }
}

/// The high bits of a figure's guid: a player guid no server hands out (their low guids fit in
/// 32 bits), so a figure never shares a name or a cache entry with a real object.
const FIGURE_GUID_TAG: u64 = 0x0000_5CE0_0000_0000;
/// A prop's guid: a game object guid in the same unused range.
const PROP_GUID_TAG: u64 = 0xF110_5CE0_0000_0000;
/// The sheath state a figure holds: 1, melee weapons in the hands.
const FIGURE_SHEATH_STATE: u32 = 1;

/// Works out [`ShownScene`] and seeds [`CurrentMap`] with the scene's map, so the world's map
/// lighting is the scene's before any server names a map.
pub(crate) fn publish_shown_scene(
    mut commands: Commands,
    state: Res<State<ClientState>>,
    roster: Res<Roster>,
    scene: Option<Res<SelectScene>>,
    progress: Option<Res<WorldLoadProgress>>,
    current_map: Option<Res<CurrentMap>>,
    mut shown: ResMut<ShownScene>,
) {
    let view = scene
        .and_then(|s| s.0)
        .filter(|_| *state.get() == ClientState::CharSelect && roster.pending_row().is_none());
    let ready = view.is_some_and(|v| {
        let (tx, ty) = benilla_formats::world_to_tile(v.look_at[0], v.look_at[1]);
        progress.as_deref().is_some_and(|p| {
            p.focus_tile == Some((tx as i32, ty as i32)) && p.focus_resident && p.presentable()
        })
    });
    if let Some(v) = view {
        if current_map.as_deref().map(|m| m.0) != Some(v.map) {
            commands.insert_resource(CurrentMap(v.map));
        }
    }
    let now = ShownScene { view, ready };
    if *shown != now {
        if now.ready && !shown.ready {
            info!(
                "select scene: map {} has loaded, the world shows",
                view.map_or(0, |v| v.map)
            );
        }
        *shown = now;
    }
}

/// Seats the world camera on the scene, after input (which never runs at select) and before the
/// stream stage reads the camera.
pub(crate) fn seat_scene_camera(
    shown: Res<ShownScene>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
) {
    let Some(v) = shown.view else {
        return;
    };
    let eye = wow_to_bevy(v.eye);
    let at = wow_to_bevy(v.look_at);
    let want = Transform::from_translation(eye).looking_at(at, Vec3::Y);
    for mut t in &mut cams {
        if *t != want {
            *t = want;
        }
    }
}

/// The figure's descriptor: the fields benilla's unit pipeline reads to draw, dress and pose a
/// player body.
fn hero_fields(c: &Character, display_id: u32, stand_state: u8) -> ObjectFields {
    ObjectFields::from_pairs(&[
        (22, 1),                         // UNIT_FIELD_HEALTH: alive
        (28, 1),                         // UNIT_FIELD_MAXHEALTH
        (34, u32::from(c.level.max(1))), // UNIT_FIELD_LEVEL
        (
            36, // UNIT_FIELD_BYTES_0: race, class, gender
            u32::from(c.race) | u32::from(c.class) << 8 | u32::from(c.gender.min(1)) << 16,
        ),
        (131, display_id),             // UNIT_FIELD_DISPLAYID
        (132, display_id),             // UNIT_FIELD_NATIVEDISPLAYID
        (138, u32::from(stand_state)), // UNIT_FIELD_BYTES_1: stand state
        (164, FIGURE_SHEATH_STATE),    // UNIT_FIELD_BYTES_2: sheath state
        (
            193, // PLAYER_BYTES: skin, face, hair style, hair colour
            u32::from(c.skin)
                | u32::from(c.face) << 8
                | u32::from(c.hair_style) << 16
                | u32::from(c.hair_color) << 24,
        ),
        (194, u32::from(c.facial_hair)), // PLAYER_BYTES_2: facial hair
    ])
}

/// Whether a starting-outfit piece fills equipment slot `slot` of row `c`: the slot is empty and,
/// for a weapon slot (15 main hand, 16 off hand, 17 ranged), the row holds no weapon at all; a row
/// that arms its hero says all its weapons (a two-hander leaves the off hand empty, not the
/// outfit's shield).
fn outfit_fills(c: &Character, slot: usize) -> bool {
    let weapon = |s: usize| (15..=17).contains(&s);
    c.equipment[slot].display_id == 0
        && !(weapon(slot) && (15..=17).any(|s| c.equipment[s].display_id != 0))
}

/// What a hero figure wears, by equipment slot: its roster row's pieces, the starting outfit
/// under them where asked, the held slots with their sheath types (a starting-outfit weapon's
/// from `sheaths`).
fn hero_items(
    hero: &FigureHero,
    outfit: &[(u32, u8)],
    sheaths: &OutfitWeaponSheaths,
) -> FigureItems {
    let mut items = FigureItems::default();
    let mut put = |slot: usize, item: CharEnumItem, from_outfit: bool| {
        if item.display_id == 0 || slot >= 19 {
            return;
        }
        let sheath = match slot {
            15..=17 if from_outfit => sheaths.0.get(&item.display_id).copied().unwrap_or(0),
            15..=17 => hero.held_sheath[slot - 15],
            _ => 0,
        };
        items.0[slot] = Some(HeldTemplate {
            display_info_id: item.display_id,
            inventory_type: u32::from(item.inventory_type),
            sheath: u32::from(sheath),
            class: 0,
            subclass: 0,
            material: 0,
        });
    };
    if hero.start_outfit_under {
        for &(display_id, inv_type) in outfit {
            if let Some(slot) = equip_slot(inv_type) {
                if outfit_fills(&hero.character, slot) {
                    put(
                        slot,
                        CharEnumItem {
                            display_id,
                            inventory_type: inv_type,
                        },
                        true,
                    );
                }
            }
        }
    }
    for (slot, item) in hero.character.equipment.iter().enumerate() {
        put(slot, *item, false);
    }
    items
}

/// Gives each new or changed [`SceneFigure`] the components benilla's unit pipeline draws from. A
/// built hero that changes gets a new descriptor and items only, never a new visual, so a change of
/// stand state plays its transition on the body already standing there.
pub(crate) fn build_scene_figures(
    mut commands: Commands,
    figures: Query<(
        Entity,
        Ref<SceneFigure>,
        Option<&NetEntity>,
        Has<Visibility>,
    )>,
    char_create: Option<Res<CharCreate>>,
    mut names: Option<ResMut<crate::names::NameCache>>,
    sheaths: Option<Res<OutfitWeaponSheaths>>,
) {
    let no_sheaths = OutfitWeaponSheaths::default();
    let sheaths = sheaths.as_deref().unwrap_or(&no_sheaths);
    for (entity, figure, net, has_visibility) in &figures {
        let built = net.is_some();
        if built && !figure.is_changed() {
            continue;
        }
        if let (Some(net), SceneFigure::Hero(hero)) = (net, &*figure) {
            if let (Some(display_id), Some(cc)) = (net.display_id, char_create.as_deref()) {
                let c = &hero.character;
                let outfit: Vec<(u32, u8)> =
                    cc.0.start_outfit(c.race, c.class, c.gender.min(1))
                        .iter()
                        .map(|i| (i.display_id, i.inv_type))
                        .collect();
                commands.entity(entity).insert((
                    ObjectStore(hero_fields(c, display_id, hero.stand_state)),
                    hero_items(hero, &outfit, sheaths),
                ));
            }
            continue;
        }
        if built {
            continue;
        }
        let mut e = commands.entity(entity);
        if !has_visibility {
            e.insert(Visibility::default());
        }
        match &*figure {
            SceneFigure::Hero(hero) => {
                // The body and the outfit wait for the character catalog.
                let Some(cc) = char_create.as_deref() else {
                    continue;
                };
                let c = &hero.character;
                let Some(display_id) = cc.0.body_display(c.race, c.gender.min(1)) else {
                    warn!(
                        "select scene: no body for race {} — {} not shown",
                        c.race, c.name
                    );
                    e.insert(NetEntity {
                        kind: EntityKind::Player,
                        display_id: None,
                        scale: 1.0,
                    });
                    continue;
                };
                let outfit: Vec<(u32, u8)> =
                    cc.0.start_outfit(c.race, c.class, c.gender.min(1))
                        .iter()
                        .map(|i| (i.display_id, i.inv_type))
                        .collect();
                let guid = FIGURE_GUID_TAG | (c.guid & 0xFFFF_FFFF);
                // Its name is known here, so nothing asks a server for it.
                if let Some(names) = names.as_deref_mut() {
                    names.insert_player(guid, c.name.clone(), None);
                }
                e.insert((
                    Guid(guid),
                    NetEntity {
                        kind: EntityKind::Player,
                        display_id: Some(display_id),
                        scale: 1.0,
                    },
                    ObjectStore(hero_fields(c, display_id, hero.stand_state)),
                    hero_items(hero, &outfit, sheaths),
                ));
            }
            SceneFigure::Prop { display_id } => {
                e.insert((
                    Guid(PROP_GUID_TAG | u64::from(entity.index_u32())),
                    NetEntity {
                        kind: EntityKind::GameObject,
                        display_id: Some(*display_id),
                        scale: 1.0,
                    },
                    ObjectStore(ObjectFields::default()),
                ));
            }
        }
    }
}

/// How far a figure's given height may be from the terrain under it and still be set on it, yards:
/// nearer, it stands on the ground; further (a building's floor over the terrain), it stays put.
const GROUND_SNAP: f32 = 3.0;

/// A hero [`SceneFigure`]'s head, in Bevy world space: the model's Head attachment (where a helm
/// sits) in its current pose, so it comes down as the figure sits or kneels; else where the game
/// hangs a unit's name (the PlayerName attachment, else the unit's height from its box). Kept each
/// frame once the transforms have propagated, so an embedder can draw a name over the figure.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct FigureOverhead(pub Vec3);

/// The Head attachment's id in a character model (the helm's seat).
const ATTACH_HEAD: u16 = 11;

/// Keeps each hero figure's [`FigureOverhead`], the nameplate's anchor.
#[allow(clippy::type_complexity)] // one Bevy system's inputs
pub(crate) fn publish_figure_overheads(
    mut commands: Commands,
    mut figures: Query<(Entity, &SceneFigure, &Transform, Option<&mut FigureOverhead>)>,
    anchors: Query<&crate::entities::BoneAttach>,
    poses: Query<&benilla_world::rig_anim::RigPose>,
    fallbacks: Query<&crate::entities::OverheadFallback>,
    globals: Query<&GlobalTransform>,
    mounts: Query<(), With<crate::entities::mount::MountChild>>,
) {
    for (entity, figure, tf, current) in &mut figures {
        if !matches!(figure, SceneFigure::Hero(_)) {
            continue;
        }
        let head = (|| {
            let &(bone, offset) = anchors.get(entity).ok()?.points.get(&ATTACH_HEAD)?;
            let pose = poses.get(entity).ok()?;
            let own;
            let root = if pose.joints_root == entity {
                own = GlobalTransform::from(*tf);
                &own
            } else {
                globals.get(pose.joints_root).ok()?
            };
            pose.posed_point(root, bone, offset)
        })();
        let at = head.unwrap_or_else(|| {
            crate::entities::overhead_anchor(
                entity, tf, &anchors, &poses, &fallbacks, &globals, &mounts,
            )
        });
        match current {
            Some(mut c) => {
                if c.0 != at {
                    c.0 = at;
                }
            }
            None => {
                commands.entity(entity).insert(FigureOverhead(at));
            }
        }
    }
}

/// Sets each figure on the terrain under it once its tile has loaded, so a place needs no exact
/// height. Measured from the height it was placed at, kept in [`PlacedHeight`].
pub(crate) fn ground_scene_figures(
    mut commands: Commands,
    streamer: Option<Res<benilla_world::terrain_stream::TerrainStreamer>>,
    adt_tiles: Option<Res<Assets<benilla_assets::AdtTile>>>,
    mut figures: Query<(Entity, &mut Transform, Option<&PlacedHeight>), With<SceneFigure>>,
) {
    let (Some(streamer), Some(adt_tiles)) = (streamer, adt_tiles) else {
        return;
    };
    for (entity, mut t, placed) in &mut figures {
        let placed = match placed {
            Some(p) => p.0,
            None => {
                commands
                    .entity(entity)
                    .insert(PlacedHeight(t.translation.y));
                t.translation.y
            }
        };
        let probe = Vec3::new(t.translation.x, placed, t.translation.z);
        let Some(ground) =
            benilla_world::terrain_stream::terrain_height_under(&streamer, &adt_tiles, probe)
        else {
            continue;
        };
        if (ground - placed).abs() <= GROUND_SNAP && t.translation.y != ground {
            debug!("select scene: figure {entity} set on the ground at {ground:.2} (placed at {placed:.2})");
            t.translation.y = ground;
        } else if (ground - placed).abs() > GROUND_SNAP {
            debug!(
                "select scene: figure {entity} left at {placed:.2}, the terrain is at {ground:.2}"
            );
        }
    }
}

/// The height a figure was placed at, before [`ground_scene_figures`] set it on the ground.
#[derive(Component, Clone, Copy)]
pub(crate) struct PlacedHeight(f32);

/// How far from a hero figure's axis a click still takes it, yards.
const FIGURE_PICK_RADIUS: f32 = 0.8;

/// How tall a hero figure's body is in its pose, yards: standing, seated, kneeling (stand states
/// 0, 1 and 8; any other counts as standing), a tauren's a quarter more.
fn figure_height(stand_state: u8, race: u8) -> f32 {
    let height = match stand_state {
        1 => 1.1,
        8 => 1.4,
        _ => 2.2,
    };
    if race == 6 {
        height * 1.25
    } else {
        height
    }
}

/// The hero figure a ray hits first, by its roster guid: each figure is an upright cylinder of
/// [`FIGURE_PICK_RADIUS`] around its feet, as tall as its body in its pose (Bevy space, +Y up),
/// so a click over a kneeling hero's head goes on to whoever stands behind it.
fn figure_under_ray(ray: Ray3d, figures: impl Iterator<Item = (u64, Vec3, f32)>) -> Option<u64> {
    let mut best: Option<(f32, u64)> = None;
    for (guid, feet, height) in figures {
        // Where the ray is within the cylinder's circle, in the ground plane: t0..t1 along it.
        let o = Vec2::new(ray.origin.x - feet.x, ray.origin.z - feet.z);
        let d = Vec2::new(ray.direction.x, ray.direction.z);
        let (a, b, c) = (
            d.length_squared(),
            2.0 * o.dot(d),
            o.length_squared() - FIGURE_PICK_RADIUS * FIGURE_PICK_RADIUS,
        );
        if a < 1e-6 {
            continue; // straight down: no click looks that way at a select scene
        }
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 {
            continue;
        }
        let (t0, t1) = (
            (-b - disc.sqrt()) / (2.0 * a),
            (-b + disc.sqrt()) / (2.0 * a),
        );
        if t1 < 0.0 {
            continue;
        }
        let t0 = t0.max(0.0);
        // The heights the ray passes through over that stretch, against the body's.
        let (y0, y1) = (
            ray.origin.y + ray.direction.y * t0,
            ray.origin.y + ray.direction.y * t1,
        );
        let (lo, hi) = (feet.y, feet.y + height);
        if y0.max(y1) < lo || y0.min(y1) > hi {
            continue;
        }
        // Where it enters the body: the circle's edge, or the top or the foot when later.
        let dy = ray.direction.y;
        let enter = if (lo..=hi).contains(&y0) || dy.abs() < 1e-6 {
            t0
        } else if y0 > hi {
            (hi - ray.origin.y) / dy
        } else {
            (lo - ray.origin.y) / dy
        };
        if best.is_none_or(|(t, _)| enter < t) {
            best = Some((enter, guid));
        }
    }
    best.map(|(_, guid)| guid)
}

/// The select scene's pointer, its requests and its mirror ([`point_at_scene_figures`] to
/// [`publish_select_screen`]): the scene's writers of the roster, ordered after the net drain and
/// before the UI feeds, the world stages and the loading cover that read it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SelectSceneRoster;

/// The hero figure under the mouse, by its roster guid; `None` over the screen's buttons, off
/// every figure, or while the scene does not show.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SceneHover(pub(crate) Option<u64>);

/// Tracks the hero figure under the mouse each frame ([`SceneHover`]), and a click on one selects
/// its row, as a click on the row does; the screen's buttons keep the mouse to themselves.
#[allow(clippy::too_many_arguments)] // one Bevy system's inputs
pub(crate) fn point_at_scene_figures(
    shown: Res<ShownScene>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut moves: MessageReader<bevy::window::CursorMoved>,
    mut lefts: MessageReader<bevy::window::CursorLeft>,
    mut cursor: Local<Option<Vec2>>,
    cams: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    figures: Query<(&SceneFigure, &GlobalTransform)>,
    buttons: Query<&Interaction>,
    mut hover: ResMut<SceneHover>,
    mut roster: ResMut<Roster>,
) {
    // The pointer as the window's cursor events place it (bevy_ui's picking reads the same
    // events), so an in-process pointer moves it as the mouse does.
    for m in moves.read() {
        *cursor = Some(m.position);
    }
    if lefts.read().last().is_some() {
        *cursor = None;
    }
    let under = (|| {
        if !shown.ready || buttons.iter().any(|i| *i != Interaction::None) {
            return None;
        }
        let (cam, at) = cams.single().ok()?;
        let ray = cam.viewport_to_world(at, (*cursor)?).ok()?;
        let heroes = figures.iter().filter_map(|(f, t)| match f {
            SceneFigure::Hero(h) => Some((
                h.character.guid,
                t.translation(),
                figure_height(h.stand_state, h.character.race),
            )),
            SceneFigure::Prop { .. } => None,
        });
        figure_under_ray(ray, heroes)
    })();
    if hover.0 != under {
        hover.0 = under;
    }
    if mouse.just_pressed(MouseButton::Left) && under.is_none() && shown.ready {
        let on_button = buttons.iter().any(|i| *i != Interaction::None);
        debug!(
            "select scene: a click at {:?} takes no figure (over a button: {on_button})",
            *cursor
        );
    }
    if let Some(guid) = under.filter(|_| mouse.just_pressed(MouseButton::Left)) {
        if roster.pending_row().is_none() {
            info!("select scene: a click on the figure of {guid:#x} selects it");
            roster.click_guid(guid);
        }
    }
}

/// Plays an emote animation (an `AnimationData.dbc` id, a wave 67 or a salute 113) once on a hero
/// figure, through the shared one-shot emote player: an id already playing is not started again.
#[derive(Message, Clone, Copy, Debug)]
pub struct FigureEmote {
    pub figure: Entity,
    pub anim_id: u16,
}

/// Hands each [`FigureEmote`] to the unit animation driver.
pub(crate) fn play_figure_emotes(
    mut asks: MessageReader<FigureEmote>,
    figures: Query<(), With<SceneFigure>>,
    mut seq: ResMut<crate::creature_anim::PlaySeq>,
    mut anims: MessageWriter<crate::creature_anim::EmoteAnim>,
) {
    for ask in asks.read() {
        if figures.get(ask.figure).is_err() {
            continue;
        }
        anims.write(crate::creature_anim::EmoteAnim {
            entity: ask.figure,
            anim_id: ask.anim_id,
            seq: seq.next(),
            via_player: true,
        });
    }
}

/// Mirrors the select screen into [`SelectScreen`] when it changed.
pub(crate) fn publish_select_screen(
    state: Res<State<ClientState>>,
    roster: Res<Roster>,
    shown: Res<ShownScene>,
    hover: Res<SceneHover>,
    mut screen: ResMut<SelectScreen>,
) {
    let open = *state.get() == ClientState::CharSelect;
    let now = SelectScreen {
        open,
        characters: if open {
            roster.characters().to_vec()
        } else {
            Vec::new()
        },
        selected: if open { roster.selected_guid() } else { None },
        realm: roster.realm_name().map(str::to_string),
        scene_ready: shown.ready,
        hovered: if open { hover.0 } else { None },
    };
    if !screen.same_as(&now) {
        *screen = now;
    }
}

/// Selects the row each [`SelectCharacter`] names, while the select screen is up.
pub(crate) fn take_select_requests(
    state: Res<State<ClientState>>,
    mut asks: MessageReader<SelectCharacter>,
    mut roster: ResMut<Roster>,
) {
    for SelectCharacter(guid) in asks.read() {
        if *state.get() != ClientState::CharSelect || roster.pending_row().is_some() {
            continue;
        }
        info!("select scene: the embedder selects {guid:#x}");
        roster.click_guid(*guid);
    }
}

// ── An embedder's own select screen ─────────────────────────────────────────────────────────────

/// The embedder draws a select screen of its own in place of the stock one; `false` (the default)
/// keeps the stock screen. While it is `true` the stock screen's tree is not built and its keys
/// (Enter, Escape, the arrows) do nothing, its delete dialog never opens, and the session stays
/// parked at character select for the embedder's own requests: [`SelectCharacter`],
/// [`SelectEnterWorld`], [`SelectCreate`], [`SelectDelete`] and [`SelectRefresh`], answered by
/// [`SelectActionDone`] and the roster in [`SelectScreen`]. The scene, its figures and
/// [`SelectModel`] work as with the stock screen.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectScreenReplaced(pub bool);

/// Enter the world as the roster row of this guid, as Enter World does; ignored when the select
/// screen is not up, a pick is already on its way, or no row has it.
#[derive(Message, Clone, Copy, Debug)]
pub struct SelectEnterWorld(pub u64);

/// Create a character (`CMSG_CHAR_CREATE`) and stay at select: on success the fresh roster comes
/// first, with the new row selected, then a [`SelectActionDone`].
#[derive(Message, Clone, Debug)]
pub struct SelectCreate(pub benilla_protocol::CharCreateReq);

/// Delete the character of this guid (`CMSG_CHAR_DELETE`) and stay at select: on success the
/// fresh roster comes first, then a [`SelectActionDone`]. Asking first is the embedder's.
#[derive(Message, Clone, Copy, Debug)]
pub struct SelectDelete(pub u64);

/// Ask the server for the roster again (`CMSG_CHAR_ENUM`), keeping the selected row: an embedder
/// changed its characters another way (a rename, another look) and the list should show it.
#[derive(Message, Clone, Copy, Debug, Default)]
pub struct SelectRefresh;

/// The server's answer to a [`SelectCreate`] or [`SelectDelete`] (or a stock screen's create or
/// delete): its result byte (`CHAR_CREATE_SUCCESS`, `CHAR_NAME_*`, `CHAR_DELETE_SUCCESS` ...).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectActionDone {
    pub action: benilla_protocol::CharAction,
    pub code: u8,
}

/// The name of the character an embedder asked to create, to select its row once made.
#[derive(Resource, Default)]
pub(crate) struct EmbedderCreate(Option<String>);

/// Hands the embedder's requests to the IO thread parked at select.
#[allow(clippy::too_many_arguments)] // one Bevy system's inputs
pub(crate) fn take_embedder_requests(
    state: Res<State<ClientState>>,
    mut roster: ResMut<Roster>,
    pick: Res<crate::net::CharPick>,
    mut created: ResMut<EmbedderCreate>,
    mut enters: MessageReader<SelectEnterWorld>,
    mut creates: MessageReader<SelectCreate>,
    mut deletes: MessageReader<SelectDelete>,
    mut refreshes: MessageReader<SelectRefresh>,
) {
    let parked = *state.get() == ClientState::CharSelect && roster.pending_row().is_none();
    for SelectCreate(req) in creates.read() {
        if parked {
            info!("select scene: the embedder creates {:?}", req.name);
            created.0 = Some(req.name.clone());
            let _ = pick.0.send(crate::net::CharRequest::Create(req.clone()));
        }
    }
    for SelectDelete(guid) in deletes.read() {
        if parked {
            info!("select scene: the embedder deletes {guid:#x}");
            let _ = pick.0.send(crate::net::CharRequest::Delete(*guid));
        }
    }
    if refreshes.read().count() > 0 && parked {
        roster.keep_selected = roster.selected_guid();
        let _ = pick.0.send(crate::net::CharRequest::Refresh);
    }
    for SelectEnterWorld(guid) in enters.read() {
        if parked && roster.pending_row().is_none() && roster.chars.iter().any(|c| c.guid == *guid)
        {
            info!("select scene: the embedder enters the world as {guid:#x}");
            crate::char_select::send_pick(&mut roster, &pick, *guid);
        }
    }
}

/// Passes each create and delete result on as a [`SelectActionDone`], and selects a character
/// the embedder made.
pub(crate) fn relay_action_results(
    mut results: MessageReader<crate::net::CharActionResultMessage>,
    mut roster: ResMut<Roster>,
    mut created: ResMut<EmbedderCreate>,
    mut done: MessageWriter<SelectActionDone>,
) {
    for r in results.read() {
        if r.action == benilla_protocol::CharAction::Create {
            if let Some(name) = created.0.take() {
                if r.code == benilla_protocol::messages::CHAR_CREATE_SUCCESS {
                    roster.note_created(name);
                }
            }
        }
        done.write(SelectActionDone {
            action: r.action,
            code: r.code,
        });
    }
}

/// A character shown in the glue booth while the select screen is up, in place of the stock
/// screen's model: the booth's own body, lighting and camera, no race backdrop (the picture's
/// background is clear), turned by `yaw` (radians). `None` (the default) keeps the stock rule.
#[derive(Resource, Clone, Debug, Default)]
pub struct SelectModel(pub Option<SelectModelView>);

/// What [`SelectModel`] shows.
#[derive(Clone, Debug)]
pub struct SelectModelView {
    /// The character, its roster row's look and equipment (any guid; it needs no row of the
    /// roster), dressed in the starting outfit under it when asked.
    pub hero: FigureHero,
    pub yaw: f32,
}

/// The glue booth's picture, for an embedder to draw [`SelectModel`] with (an `ImageNode`).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct SelectModelImage(pub Option<Handle<Image>>);

/// The character [`SelectModel`] shows: its row, with the starting outfit in each slot it leaves
/// empty when asked.
pub(crate) fn model_character(hero: &FigureHero, outfit: &[(u32, u8)]) -> Character {
    let mut c = hero.character.clone();
    if hero.start_outfit_under {
        for &(display_id, inventory_type) in outfit {
            if let Some(slot) = equip_slot(inventory_type) {
                if outfit_fills(&hero.character, slot) {
                    c.equipment[slot] = CharEnumItem {
                        display_id,
                        inventory_type,
                    };
                }
            }
        }
    }
    c
}

/// Publishes the glue booth's picture.
pub(crate) fn publish_model_image(
    portraits: Res<crate::portrait::PortraitImages>,
    mut image: ResMut<SelectModelImage>,
) {
    let now = SelectModelImage(match portraits.0.get(crate::portrait::GLUE_SLOT) {
        Some(crate::portrait::PortraitSource::Live(h)) => Some(h.clone()),
        _ => None,
    });
    if *image != now {
        *image = now;
    }
}

/// What the create screen offers, for an embedder's own: which race may be which class (the
/// stock `CharBaseInfo` pairs and the ones [`crate::AddedRaceClassPairs`] adds) and how many
/// choices each appearance feature has per race and sex. Empty until the player's data has loaded.
#[derive(Resource, Clone, Debug, Default)]
pub struct CreateChoices {
    pairs: std::collections::HashSet<(u8, u8)>,
    ranges: std::collections::HashMap<(u8, u8), benilla_formats::DialRanges>,
    /// (race, sex) to the fifth feature's glue token (`"NORMAL"`, `"PIERCINGS"`, `"TUSKS"` ...).
    features: std::collections::HashMap<(u8, u8), String>,
    /// race to its hair's glue token (`"NORMAL"`, `"HORNS"`).
    hair: std::collections::HashMap<u8, String>,
}

impl CreateChoices {
    /// Whether race `race` may be class `class` (1.12 ids).
    pub fn allows(&self, race: u8, class: u8) -> bool {
        self.pairs.contains(&(race, class))
    }

    /// How many skins, faces, hair styles, hair colours and facial features (piercings, tusks,
    /// ...) race `race` of sex `sex` (0 male, 1 female) has; each feature picks `0..count`.
    pub fn ranges(&self, race: u8, sex: u8) -> Option<benilla_formats::DialRanges> {
        self.ranges.get(&(race, sex)).copied()
    }

    /// What the fifth feature of race `race` of sex `sex` is, as the create screen's glue token
    /// names it (`"NORMAL"` for facial hair, `"PIERCINGS"`, `"EARRINGS"`, `"TUSKS"`, `"HORNS"`,
    /// `"MARKINGS"`, `"FEATURES"`, `"HAIR"`; `"NONE"` when it has none).
    pub fn feature_token(&self, race: u8, sex: u8) -> Option<&str> {
        self.features.get(&(race, sex)).map(String::as_str)
    }

    /// What race `race`'s hair is, as the glue token names it (`"NORMAL"`, `"HORNS"`).
    pub fn hair_token(&self, race: u8) -> Option<&str> {
        self.hair.get(&race).map(String::as_str)
    }

    /// Whether the player's data has loaded.
    pub fn loaded(&self) -> bool {
        !self.pairs.is_empty()
    }
}

/// Fills [`CreateChoices`] from the create catalog when it comes, and again when it changes.
pub(crate) fn publish_create_choices(
    catalog: Option<Res<CharCreate>>,
    mut choices: ResMut<CreateChoices>,
) {
    let Some(catalog) = catalog.filter(|c| c.is_changed()) else {
        return;
    };
    let mut now = CreateChoices::default();
    for race in 1..=8u8 {
        for class in 1..=11u8 {
            if catalog.0.allows(race, class) {
                now.pairs.insert((race, class));
            }
        }
        if let Some(t) = catalog.0.hair_customization(race) {
            now.hair.insert(race, t.to_owned());
        }
        for sex in 0..=1u8 {
            if let Some(r) = catalog.0.ranges(race, sex) {
                now.ranges.insert((race, sex), r);
            }
            if let Some(t) = catalog.0.facial_hair_customization(race, sex) {
                now.features.insert((race, sex), t.to_owned());
            }
        }
    }
    info!(
        "select scene: the create choices are in ({} race and class pairs)",
        now.pairs.len()
    );
    *choices = now;
}

// ── A figure drawn alone: the embedder's mask of it ─────────────────────────────────────────────

/// Put on a [`SceneFigure`]: its body, gear and weapons are also drawn alone, through a camera that
/// follows the world camera, into [`FigureMaskImage`], so an embedder can draw round the figure's
/// silhouette (an outline, a glow). The picture is clear but where the figure is; it covers the
/// window, pixel for pixel. Take the component off and the figure draws only in the world again.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SceneFigureMask;

/// The picture of the [`SceneFigureMask`] figures, window-sized; `None` until one is asked for.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct FigureMaskImage(pub Option<Handle<Image>>);

/// The mask's own layer, on top of the booth ladder.
pub(crate) const FIGURE_MASK_LAYER: usize =
    crate::portrait::UI_MODEL_CAM_LAYER_BASE + crate::portrait::UI_MODEL_CAM_LAYERS;

/// The mask camera, once made.
#[derive(Resource)]
pub(crate) struct FigureMaskCamera;

/// Puts the model batches of every masked figure on the mask's layer as well as the world's, and
/// takes them off again; only the model batches (body, gear, weapons), never their particles,
/// whose simulation takes the first camera on their layers.
#[allow(clippy::type_complexity)]
pub(crate) fn mark_figure_masks(
    mut commands: Commands,
    figures: Query<(Entity, Has<SceneFigureMask>), With<SceneFigure>>,
    children: Query<&Children>,
    batches: Query<
        Option<&bevy::camera::visibility::RenderLayers>,
        With<MeshMaterial3d<benilla_assets::materials::WowModelMaterial>>,
    >,
) {
    use bevy::camera::visibility::RenderLayers;
    for (figure, masked) in &figures {
        for part in children.iter_descendants(figure) {
            let Ok(layers) = batches.get(part) else {
                continue;
            };
            let on = layers.is_some_and(|l| l.intersects(&RenderLayers::layer(FIGURE_MASK_LAYER)));
            if masked && !on {
                commands
                    .entity(part)
                    .insert(RenderLayers::from_layers(&[0, FIGURE_MASK_LAYER]));
            } else if !masked && on {
                commands.entity(part).remove::<RenderLayers>();
            }
        }
    }
}

/// Keeps the mask camera on the world camera's eye and optic, its picture the window's size, and
/// renders only while a figure is masked.
#[allow(clippy::too_many_arguments)] // one Bevy system's inputs
pub(crate) fn drive_figure_mask_camera(
    mut commands: Commands,
    masked: Query<(), With<SceneFigureMask>>,
    world_cam: Query<(&Transform, &Projection), (With<WorldCamera>, Without<FigureMaskCameraTag>)>,
    mut mask_cam: Query<(&mut Camera, &mut Transform, &mut Projection), With<FigureMaskCameraTag>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut image: ResMut<FigureMaskImage>,
    made: Option<Res<FigureMaskCamera>>,
) {
    let want = !masked.is_empty();
    if made.is_none() {
        if !want {
            return;
        }
        let handle = images.add(crate::portrait::new_target_image_sized(1, 1));
        commands.spawn((
            FigureMaskCameraTag,
            crate::portrait::booth_view_shape(),
            Camera {
                order: -100 + FIGURE_MASK_LAYER as isize,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                is_active: false,
                ..default()
            },
            bevy::camera::RenderTarget::Image(handle.clone().into()),
            bevy::camera::visibility::RenderLayers::layer(FIGURE_MASK_LAYER),
            Transform::default(),
            Projection::from(bevy::camera::PerspectiveProjection::default()),
        ));

        commands.insert_resource(FigureMaskCamera);
        image.0 = Some(handle);
        return;
    }
    let Ok((mut cam, mut t, mut p)) = mask_cam.single_mut() else {
        return;
    };
    let source = world_cam.single().ok();
    let active = want && source.is_some();
    if cam.is_active != active {
        cam.is_active = active;
    }
    let Some((wt, wp)) = source.filter(|_| active) else {
        return;
    };
    if *t != *wt {
        *t = *wt;
    }
    let same = match (&*p, wp) {
        (Projection::Perspective(a), Projection::Perspective(b)) => {
            a.fov == b.fov && a.aspect_ratio == b.aspect_ratio && a.near == b.near && a.far == b.far
        }
        _ => false,
    };
    if !same {
        *p = wp.clone();
    }
    // the picture follows the window, pixel for pixel
    let (Ok(window), Some(handle)) = (windows.single(), image.0.as_ref()) else {
        return;
    };
    let size = bevy::render::render_resource::Extent3d {
        width: window.physical_width().max(1),
        height: window.physical_height().max(1),
        depth_or_array_layers: 1,
    };
    if let Some(img) = images.get(handle) {
        if img.texture_descriptor.size != size {
            if let Some(img) = images.get_mut(handle) {
                img.resize(size);
            }
        }
    }
}

/// Marks the mask camera.
#[derive(Component)]
pub(crate) struct FigureMaskCameraTag;

#[cfg(test)]
mod tests {
    use super::*;

    fn hero(equipment: [CharEnumItem; 19]) -> FigureHero {
        FigureHero {
            character: Character {
                guid: 7,
                name: "Ana".into(),
                race: 2,
                class: 1,
                gender: 0,
                skin: 1,
                face: 2,
                hair_style: 3,
                hair_color: 4,
                facial_hair: 5,
                level: 1,
                zone: 12,
                map: 1,
                position: benilla_protocol::wire::Vector3d {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                flags: 0,
                equipment,
                pet_display_id: 0,
                pet_level: 0,
                pet_family: 0,
            },
            stand_state: 8,
            held_sheath: [1, 0, 0],
            start_outfit_under: true,
        }
    }

    #[test]
    fn a_booth_model_wears_its_row_over_the_starting_outfit() {
        let mut equipment = [CharEnumItem::default(); 19];
        equipment[4] = CharEnumItem {
            display_id: 900,
            inventory_type: 5,
        };
        let outfit = [(10, 5), (11, 7)];
        let c = model_character(&hero(equipment), &outfit);
        assert_eq!(c.equipment[4].display_id, 900, "the worn chest stays");
        assert_eq!(
            (c.equipment[6].display_id, c.equipment[6].inventory_type),
            (11, 7),
            "the outfit's legs fill the empty slot"
        );
        let mut bare = hero(equipment);
        bare.start_outfit_under = false;
        assert_eq!(model_character(&bare, &outfit).equipment[6].display_id, 0);
    }

    #[test]
    fn a_hero_figure_wears_its_row_over_the_starting_outfit() {
        let mut equipment = [CharEnumItem::default(); 19];
        equipment[4] = CharEnumItem {
            display_id: 900,
            inventory_type: 5,
        };
        equipment[15] = CharEnumItem {
            display_id: 901,
            inventory_type: 17,
        };
        // The outfit's chest gives way to the worn one; its legs fill the empty slot.
        let outfit = [(10, 5), (11, 7), (12, 13)];
        let items = hero_items(&hero(equipment), &outfit, &OutfitWeaponSheaths::default());
        let shows = |slot: usize| items.0[slot].map(|t| t.display_info_id);
        assert_eq!(shows(4), Some(900));
        assert_eq!(shows(6), Some(11), "the outfit's legs fill the empty slot");
        assert_eq!(shows(15), Some(901), "the worn weapon, not the outfit's");
        // a row that holds a weapon says all its weapons: the outfit's shield stays away
        let with_shield = [(10, 5), (12, 13), (13, 14)];
        let armed = hero_items(&hero(equipment), &with_shield, &OutfitWeaponSheaths::default());
        assert_eq!(armed.0[16].map(|t| t.display_info_id), None, "no outfit shield beside a two-hander");
        let model = model_character(&hero(equipment), &with_shield);
        assert_eq!(model.equipment[16].display_id, 0);
        assert_eq!(items.0[15].map(|t| t.sheath), Some(1));
        assert_eq!(shows(0), None);

        let mut bare = hero(equipment);
        bare.start_outfit_under = false;
        let items = hero_items(&bare, &outfit, &OutfitWeaponSheaths::default());
        assert_eq!(items.0[6].map(|t| t.display_info_id), None);

        // With no worn weapon the outfit's one-hander is held, resting where the embedder says.
        let mut unarmed = hero(equipment);
        unarmed.character.equipment[15] = CharEnumItem::default();
        let sheaths = OutfitWeaponSheaths([(12, 3)].into());
        let items = hero_items(&unarmed, &outfit, &sheaths);
        assert_eq!(
            items.0[15].map(|t| (t.display_info_id, t.sheath)),
            Some((12, 3))
        );
        let items = hero_items(&unarmed, &outfit, &OutfitWeaponSheaths::default());
        assert_eq!(items.0[15].map(|t| t.sheath), Some(0), "stock: none");
    }

    #[test]
    fn a_filter_lists_its_rows_in_roster_order() {
        let rows: Vec<Character> = [7, 3, 9, 4]
            .iter()
            .map(|&g| {
                let mut c = hero([CharEnumItem::default(); 19]).character;
                c.guid = g;
                c
            })
            .collect();
        assert_eq!(
            listed_rows(&rows, &SelectListFilter(None)),
            vec![0, 1, 2, 3]
        );
        // roster order wins over the filter's, and a guid off the roster is skipped
        let filter = SelectListFilter(Some(vec![4, 7, 99]));
        assert_eq!(listed_rows(&rows, &filter), vec![0, 3]);
    }

    /// A camera 3 yards up and 9 back, as a select scene's, takes a click on a body's middle in
    /// each pose (its projected bounds: a kneeling, seated or standing body's height), and a click
    /// just off its side or over its head takes nothing.
    #[test]
    fn a_click_takes_a_figure_kneeling_seated_or_standing() {
        let eye = Vec3::new(0.0, 3.0, 9.0);
        let feet = Vec3::new(0.6, 0.0, -2.6);
        for (pose, state) in [("kneeling", 8u8), ("seated", 1), ("standing", 0)] {
            let height = figure_height(state, 2);
            let fig = [(7, feet, height)];
            let body = feet + Vec3::Y * (height / 2.0);
            let ray = Ray3d::new(eye, Dir3::new(body - eye).unwrap());
            assert_eq!(figure_under_ray(ray, fig.into_iter()), Some(7), "{pose}");
            let over = feet + Vec3::Y * (height + 0.4);
            let ray = Ray3d::new(eye, Dir3::new(over - eye).unwrap());
            assert_eq!(
                figure_under_ray(ray, fig.into_iter()),
                None,
                "over a {pose} head"
            );
            let beside = feet + Vec3::new(1.2, height / 2.0, 0.0);
            let ray = Ray3d::new(eye, Dir3::new(beside - eye).unwrap());
            assert_eq!(
                figure_under_ray(ray, fig.into_iter()),
                None,
                "beside a {pose} body"
            );
        }
        assert!(
            figure_height(0, 6) > figure_height(0, 2),
            "a tauren stands taller"
        );
    }

    /// A click over a kneeling hero's head takes the hero standing behind it; on the kneeling
    /// body itself it takes the kneeling one.
    #[test]
    fn a_click_over_a_kneeling_hero_reaches_the_one_standing_behind() {
        let eye = Vec3::new(0.0, 3.0, 9.0);
        let front = (1, Vec3::new(0.0, 0.0, -2.6), figure_height(8, 2));
        let back = (2, Vec3::new(0.0, 0.0, -5.2), figure_height(0, 2));
        let chest_behind = Vec3::new(0.0, 1.5, -5.2);
        let ray = Ray3d::new(eye, Dir3::new(chest_behind - eye).unwrap());
        assert_eq!(figure_under_ray(ray, [front, back].into_iter()), Some(2));
        let kneeling_body = Vec3::new(0.0, 0.6, -2.6);
        let ray = Ray3d::new(eye, Dir3::new(kneeling_body - eye).unwrap());
        assert_eq!(figure_under_ray(ray, [front, back].into_iter()), Some(1));
    }

    #[test]
    fn a_click_takes_the_nearest_figure_on_the_ray() {
        let ray = Ray3d::new(Vec3::new(0.0, 1.0, 10.0), Dir3::NEG_Z);
        let figures = [
            (1, Vec3::new(0.3, 0.0, 0.0), 2.2),
            (2, Vec3::new(0.0, 0.0, -3.0), 2.2), // behind the first
            (3, Vec3::new(3.0, 0.0, 5.0), 2.2),  // off to the side
        ];
        assert_eq!(figure_under_ray(ray, figures.into_iter()), Some(1));
        let high = Ray3d::new(Vec3::new(0.0, 5.0, 10.0), Dir3::NEG_Z);
        assert_eq!(
            figure_under_ray(high, figures.into_iter()),
            None,
            "over their heads"
        );
    }

    #[test]
    fn a_hero_figure_reads_back_its_look_and_pose() {
        let h = hero([CharEnumItem::default(); 19]);
        let f = hero_fields(&h.character, 51, 8);
        assert_eq!(f.unit_stand_state(), 8);
        assert_eq!((f.unit_race(), f.unit_class()), (Some(2), Some(1)));
        assert_eq!(
            (
                f.player_skin(),
                f.player_face(),
                f.player_hair_style(),
                f.player_hair_color(),
                f.player_facial_hair()
            ),
            (Some(1), Some(2), Some(3), Some(4), Some(5))
        );
        assert!(!f.unit_reads_dead(), "a figure stands alive");
        assert_eq!(f.unit_sheath_state(), Some(1));
    }
}
