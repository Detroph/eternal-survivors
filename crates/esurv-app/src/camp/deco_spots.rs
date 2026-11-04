//! The camp's decoration spots (plans/hub-decoration-spots.md): the server tells
//! where they are and what stands on each ([`ToClient::DecorationSpots`]); each is a small gold
//! ring of light on the ground, whole from any distance, and while the
//! hero stands on one a prompt shows, "Decorate (F)". F opens the Decorate list over it: "Empty"
//! and every piece the server says may stand there, the one standing there marked; a click
//! sets it out ([`ToServer::CampDecorate`]). The list closes on Close, Escape, F again or walking
//! off the spot. The key is F, not E: E is the stock step-right. While the
//! prompt shows, F runs no key binding, nor Escape while the list is open ([`OverlayKeys`]).
//!
//! The spots by the fire take the camp pieces the warband owns (the ground pieces of
//! `content/decorations.ron`); a tent's take its hero's things (the tent pieces owned, and its
//! class's own).

use benilla_app::{Objects, OverlayKeyPressed, OverlayKeys, OverlayWindow};
use benilla_assets::coords::wow_to_bevy;
use benilla_world::particles::buffer::{begin_effect_frame, EffectVertex, WorldEffectDraw};
use benilla_world::sky_order::Rung;
use benilla_world::view::WorldCamera;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use esurv_content::{Content, Decoration};
use esurv_protocol::{ToClient, ToServer};

use super::CampVisit;
use crate::protocol::{Request, ServerEvent};
use crate::menus::look::{palette, plain_icon, ButtonStyle, Face, MenuLook, BORDER};
use crate::menus::scroll::{scrolling, ScrollingList};
use crate::ui::{layer, GameContent};

/// The ring's radius and height, yards.
const RING_RADIUS: f32 = 0.8;
const RING_HEIGHT: f32 = 0.35;
const RING_STEPS: usize = 24;
/// The camp's gold (the edge's, `crate::arena_edge`).
const GOLD: [f32; 3] = [1.0, 0.70, 0.15];
/// How near a spot the hero stands on it, yards.
const ON_SPOT: f32 = 1.5;
/// The prompt's words, its place above the action bar (units from the bottom) and its font size.
const PROMPT: &str = "Decorate (F)";
const PROMPT_BOTTOM: f32 = 110.0;
const PROMPT_FONT: f32 = 16.0;
/// The list's width, and its place at the side of the window, clear of the hero: in from the right edge and down from the top, each a share of the window (percent).
const LIST_WIDTH: f32 = 260.0;
const LIST_RIGHT: f32 = 20.0;
const LIST_TOP: f32 = 22.0;
/// A piece's icon in its row, units.
const PIECE_ICON: f32 = 22.0;
/// How tall the rows' box may grow, units (five rows); more scroll.
const LIST_ROWS_HEIGHT: f32 = 180.0;
/// The scroll track's width, units.
const TRACK_WIDTH: f32 = 4.0;
/// The holder of F (and Escape while the list is open) ([`OverlayKeys`]).
const KEYS_OWNER: &str = "camp decorate";
/// The ring's picture, texels: a glow fading up from the ground.
const PICTURE: u32 = 32;

pub(super) fn register(app: &mut App) {
    app.init_resource::<DecorationSpots>()
        .add_systems(Update, (take_spots, stand_on_spots, take_keys, press_choices, show_list, drag_the_bar, place_the_bar).chain())
        .add_systems(PostUpdate, draw_rings.after(begin_effect_frame));
}

/// A spot as told: its id, world point, the piece on it and the pieces it may take.
type ToldSpot = (String, [f32; 3], Option<String>, Vec<String>);

/// The spots told, the one the hero stands on, the prompt, the list (open, and drawn) and the
/// rings' picture.
#[derive(Resource, Default)]
struct DecorationSpots {
    spots: Vec<ToldSpot>,
    on: Option<usize>,
    prompt: Option<Entity>,
    open: bool,
    list: Option<Entity>,
    /// The list is drawn again (opened, or what stands where changed).
    redraw: bool,
    picture: Option<Handle<Image>>,
}

fn take_spots(mut events: MessageReader<ServerEvent>, mut spots: ResMut<DecorationSpots>, mut images: ResMut<Assets<Image>>) {
    for ServerEvent(m) in events.read() {
        match m {
            ToClient::DecorationSpots(told) => {
                spots.spots = told.clone();
                spots.redraw = true;
                if spots.picture.is_none() {
                    spots.picture = Some(images.add(ring_picture()));
                }
            }
            // a fresh greeting tells its own spots again (a run has none)
            ToClient::Ready(_) | ToClient::Camp { .. } => {
                spots.spots.clear();
                spots.open = false;
            }
            _ => {}
        }
    }
}

/// The hero's feet (Bevy space), when in the world.
fn hero_feet(objects: &Objects, places: &Query<&GlobalTransform, Without<WorldCamera>>) -> Option<Vec3> {
    objects.player().and_then(|e| places.get(e).ok()).map(|t| t.translation())
}

/// How far, along the ground, the hero is from spot `at`.
fn ground_distance(hero: Vec3, at: [f32; 3]) -> f32 {
    (wow_to_bevy(at) - hero).with_y(0.0).length()
}

/// The spot the hero stands on: the nearest within [`ON_SPOT`].
fn spot_stood_on(hero: Vec3, spots: &[ToldSpot]) -> Option<usize> {
    spots
        .iter()
        .enumerate()
        .map(|(i, (_, at, _, _))| (i, ground_distance(hero, *at)))
        .filter(|&(_, d)| d <= ON_SPOT)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The pieces of `ids` by name.
fn choices<'c>(content: &'c Content, ids: &[String]) -> Vec<&'c Decoration> {
    let mut list: Vec<&Decoration> = ids.iter().filter_map(|id| content.decoration(id)).collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

/// Shows the prompt while the hero stands on a spot in the camp; walking off closes the list.
fn stand_on_spots(
    mut commands: Commands,
    mut spots: ResMut<DecorationSpots>,
    visit: Res<CampVisit>,
    objects: Objects,
    places: Query<&GlobalTransform, Without<WorldCamera>>,
    look: MenuLook,
) {
    let on = hero_feet(&objects, &places).filter(|_| visit.in_camp).and_then(|h| spot_stood_on(h, &spots.spots));
    if on == spots.on && (on.is_none() || spots.prompt.is_some()) {
        return;
    }
    if on != spots.on {
        spots.open = false;
    }
    spots.on = on;
    if let Some(prompt) = spots.prompt.take() {
        commands.entity(prompt).despawn();
    }
    if on.is_none() {
        return;
    }
    // the menus' own panel and gold heading words
    let prompt = commands
        .spawn((Name::new("camp decorate prompt"), centred_above(&look, PROMPT_BOTTOM), GlobalZIndex(layer::HUD), Pickable::IGNORE))
        .with_children(|c| {
            let mut node = look.panel_node();
            node.padding = UiRect::axes(look.px(14.0), look.px(6.0));
            c.spawn((look.panel(node), Pickable::IGNORE)).with_children(|p| {
                p.spawn((look.text(Face::Display, PROMPT_FONT, palette::HEADING, PROMPT), Pickable::IGNORE));
            });
        })
        .id();
    spots.prompt = Some(prompt);
}

/// A row across the window, its content centred, `bottom` units up from the window's foot.
fn centred_above(look: &MenuLook, bottom: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        bottom: look.px(bottom),
        left: Val::Px(0.0),
        right: Val::Px(0.0),
        justify_content: JustifyContent::Center,
        ..default()
    }
}

/// The keys the spots take: F on a spot (opening and closing the list), Escape while it is open.
fn take_keys(mut presses: MessageReader<OverlayKeyPressed>, mut spots: ResMut<DecorationSpots>, mut keys: ResMut<OverlayKeys>) {
    for OverlayKeyPressed(key) in presses.read() {
        match *key {
            KeyCode::KeyF if spots.on.is_some() => spots.open = !spots.open,
            KeyCode::Escape => spots.open = false,
            _ => {}
        }
    }
    let wanted: &[KeyCode] = match (spots.on.is_some(), spots.open) {
        (true, true) => &[KeyCode::KeyF, KeyCode::Escape],
        (true, false) => &[KeyCode::KeyF],
        _ => &[],
    };
    if keys.of(KEYS_OWNER) != wanted {
        keys.set(KEYS_OWNER, wanted);
    }
}

/// A row of the list: set this piece out (`None`: clear the spot), or close the list.
#[derive(Component, Clone, Debug, PartialEq)]
enum ListButton {
    Set(Option<String>),
    Close,
}

fn press_choices(
    presses: Query<(&Interaction, &ListButton), Changed<Interaction>>,
    mut spots: ResMut<DecorationSpots>,
    mut requests: MessageWriter<Request>,
) {
    for (interaction, button) in &presses {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            ListButton::Close => spots.open = false,
            ListButton::Set(piece) => {
                if let Some((id, _, _, _)) = spots.on.and_then(|i| spots.spots.get(i)) {
                    info!("camp: decorate {id} with {piece:?}");
                    requests.write(Request(ToServer::CampDecorate { spot: id.clone(), piece: piece.clone() }));
                }
            }
        }
    }
}

/// The list's rows' box, the track beside it and the bar in the track.
#[derive(Component)]
struct ListRows;
#[derive(Component)]
struct ListTrack;
#[derive(Component)]
struct ListBar;

/// The bar dragged with the mouse scrolls the rows as far as it moves along its track: held from the press on it until the button is let go, wherever the pointer goes.
fn drag_the_bar(
    bars: Query<&Interaction, With<ListBar>>,
    mut rows: Query<(&ComputedNode, &mut ScrollPosition), With<ListRows>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut grab: Local<Option<(f32, f32)>>,
) {
    let (Ok((laid, mut at)), Some(cursor)) = (rows.single_mut(), windows.single().ok().and_then(|w| w.cursor_position())) else {
        *grab = None;
        return;
    };
    if !mouse.pressed(MouseButton::Left) {
        *grab = None;
        return;
    }
    if grab.is_none() && bars.iter().any(|i| *i == Interaction::Pressed) {
        *grab = Some((cursor.y, at.y));
    }
    let Some((from, scrolled)) = *grab else {
        return;
    };
    let scale = laid.inverse_scale_factor();
    let (shown, whole) = (laid.size().y * scale, laid.content_size().y * scale);
    if shown <= 0.0 || whole <= shown {
        return;
    }
    // the track is as tall as the box: a pixel along it is whole / shown pixels of rows
    let y = (scrolled + (cursor.y - from) * whole / shown).clamp(0.0, whole - shown);
    if at.y != y {
        at.y = y;
    }
}

/// The bar shows the share of the rows the box shows, as far down the track as the box is
/// scrolled; the track shows only while the rows do not all fit.
fn place_the_bar(
    rows: Query<(&ComputedNode, &ScrollPosition), With<ListRows>>,
    mut tracks: Query<&mut Node, (With<ListTrack>, Without<ListBar>)>,
    mut bars: Query<&mut Node, (With<ListBar>, Without<ListTrack>)>,
) {
    let (Ok((laid, at)), Ok(mut track), Ok(mut bar)) = (rows.single(), tracks.single_mut(), bars.single_mut()) else {
        return;
    };
    let scale = laid.inverse_scale_factor();
    let (shown, whole) = (laid.size().y * scale, laid.content_size().y * scale);
    let display = if whole > shown + 1.0 { Display::Flex } else { Display::None };
    if track.display != display {
        track.display = display;
    }
    if display == Display::None || whole <= 0.0 {
        return;
    }
    let (top, height) = (Val::Percent(100.0 * at.y / whole), Val::Percent(100.0 * shown / whole));
    if bar.top != top || bar.height != height {
        bar.top = top;
        bar.height = height;
    }
}

/// Draws the list while it is open (again when what stands where changed), and takes it away
/// once closed.
fn show_list(mut commands: Commands, mut spots: ResMut<DecorationSpots>, content: Option<Res<GameContent>>, look: MenuLook) {
    let spot = spots.on.filter(|_| spots.open).and_then(|i| spots.spots.get(i)).cloned();
    if spot.is_some() && spots.list.is_some() && !spots.redraw {
        return;
    }
    spots.redraw = false;
    if let Some(list) = spots.list.take() {
        commands.entity(list).despawn();
    }
    let (Some((id, _, standing, allowed)), Some(content)) = (spot, content) else {
        return;
    };
    let rows = choices(&content.0, &allowed);
    let where_words = if id.starts_with("fire_") { "A spot by the fire" } else { "A spot at a tent" };
    // a row as the Decorate camp screen's: framed, the one standing here picked
    let row_node = |look: &MenuLook| Node {
        width: Val::Percent(100.0),
        flex_shrink: 0.0,
        column_gap: look.px(6.0),
        align_items: AlignItems::Center,
        padding: UiRect::all(look.px(4.0)),
        border: UiRect::all(look.px(BORDER)),
        border_radius: BorderRadius::all(look.px(4.0)),
        ..default()
    };
    let style = |here: bool| if here { ButtonStyle::RowPicked } else { ButtonStyle::Row };
    let list = commands
        .spawn((
            Name::new("camp decorate list"),
            Node { position_type: PositionType::Absolute, right: Val::Percent(LIST_RIGHT), top: Val::Percent(LIST_TOP), ..default() },
            GlobalZIndex(layer::HUD),
            Pickable::IGNORE,
        ))
        .with_children(|c| {
            let mut node = look.panel_node();
            node.width = look.px(LIST_WIDTH);
            c.spawn((OverlayWindow, look.panel(node))).with_children(|w| {
                look.heading(w, "Decorate", 16.0);
                w.spawn((look.hint(where_words), Pickable::IGNORE));
                // the rows in a box of their own that never grows past LIST_ROWS_HEIGHT: more
                // scroll with the wheel
                let rows_box = Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    row_gap: look.px(4.0),
                    max_height: look.px(LIST_ROWS_HEIGHT),
                    overflow: Overflow::scroll_y(),
                    ..default()
                };
                // beside the rows, a thin track whose bar shows where in them the box is; unseen while they all fit
                let beside = Node { flex_direction: FlexDirection::Row, column_gap: look.px(4.0), ..default() };
                w.spawn((beside, Pickable::IGNORE)).with_children(|r| {
                    r.spawn((ListRows, rows_box, scrolling(ScrollingList::CampDecorate))).with_children(|l| {
                        let none_here = standing.is_none();
                        look.button(l, "Empty", style(none_here), !none_here, None).insert((ListButton::Set(None), row_node(&look)));
                        for d in &rows {
                            let here = standing.as_deref() == Some(d.id.as_str());
                            look.frame_button(l, style(here), !here, None).insert((ListButton::Set(Some(d.id.clone())), row_node(&look))).with_children(|b| {
                                plain_icon(b, &look, look.icon_art(&d.icon), PIECE_ICON);
                                b.spawn((look.text(Face::Plain, 11.0, palette::VALUE, d.name.clone()), Pickable::IGNORE));
                            });
                        }
                    });
                    let track = Node { width: look.px(TRACK_WIDTH), display: Display::None, border_radius: BorderRadius::all(look.px(2.0)), ..default() };
                    r.spawn((ListTrack, track, BackgroundColor(palette::INSET), Pickable::IGNORE)).with_children(|t| {
                        let bar = Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), border_radius: BorderRadius::all(look.px(2.0)), ..default() };
                        t.spawn((ListBar, bar, BackgroundColor(palette::GOLD), Button, Interaction::default()));
                    });
                });
                if rows.is_empty() {
                    let words = if id.starts_with("fire_") { "Nothing you own fits here yet: the Merchant sells camp pieces." } else { "Nothing you own fits here yet: the Merchant sells things for a tent." };
                    w.spawn((look.hint(words), Pickable::IGNORE));
                }
                look.button(w, "Close", ButtonStyle::Text, true, None).insert(ListButton::Close);
            });
        })
        .id();
    spots.list = Some(list);
}

/// The rings' picture: white, its alpha 1 at the ground (v = 1) fading to nothing at the top.
fn ring_picture() -> Image {
    let rgba: Vec<u8> = (0..PICTURE)
        .flat_map(|j| {
            let ground = (j as f32 + 0.5) / PICTURE as f32;
            [255, 255, 255, (ground.powf(1.8) * 255.0) as u8]
        })
        .collect();
    let mut image = Image::new(
        Extent3d { width: 1, height: PICTURE, depth_or_array_layers: 1 },
        TextureDimension::D2,
        rgba,
        // gamma bytes, as the effect lane's art
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..Default::default()
    });
    image
}

fn draw_rings(
    spots: Res<DecorationSpots>,
    visit: Res<CampVisit>,
    objects: Objects,
    places: Query<&GlobalTransform, Without<WorldCamera>>,
    cam: Query<Entity, With<WorldCamera>>,
    mut draw: WorldEffectDraw,
) {
    let (Some(picture), Some(_), Ok(cam)) = (&spots.picture, hero_feet(&objects, &places), cam.single()) else {
        return;
    };
    if !visit.in_camp {
        return;
    }
    let up = Vec3::Y * RING_HEIGHT;
    for (_, at, _, _) in &spots.spots {
        let [r, g, b] = GOLD;
        let colour = [r, g, b, 1.0];
        let middle = wow_to_bevy(*at);
        let foot = |i: usize| {
            let a = i as f32 / RING_STEPS as f32 * std::f32::consts::TAU;
            middle + Vec3::new(RING_RADIUS * a.cos(), 0.0, RING_RADIUS * a.sin())
        };
        let vert = |pos: Vec3, v: f32| EffectVertex { pos: pos.to_array(), uv: [0.5, v], color: colour };
        // drawn with the ground marks, as the edge is
        let mut batch = draw.batch(cam, picture.id()).additive().anchored(middle).rung(Rung::RETICLE, 0);
        batch.extend((0..RING_STEPS).flat_map(|i| {
            let (a, b) = (foot(i), foot(i + 1));
            [vert(a, 1.0), vert(b, 1.0), vert(b + up, 0.0), vert(a, 1.0), vert(b + up, 0.0), vert(a + up, 0.0)]
        }));
        batch.tris();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hero stands on the nearest spot within reach, and on none farther.
    #[test]
    fn the_nearest_spot_within_reach_is_stood_on() {
        let spots = vec![("a".to_string(), [0.0, 0.0, 0.0], None, vec![]), ("b".to_string(), [1.0, 0.0, 0.0], None, vec![])];
        let hero = wow_to_bevy([0.8, 0.0, 0.0]);
        assert_eq!(spot_stood_on(hero, &spots), Some(1));
        assert_eq!(spot_stood_on(wow_to_bevy([5.0, 0.0, 0.0]), &spots), None);
    }
}
