//! The warband's camp as a place to walk around in: the hub (on unless `ESURV_HUB=0`; the
//! server's `game::camp`, plans/hub-concept.md). With it on, the warband screen
//! gives way to the camp: the picked hero of a faction with a camp goes into the world, into its
//! camp, as soon as the warband screen is up ([`crate::menus`]). In the camp the server says
//! [`ToClient::Camp`] in place of a run's `Ready`; a thing used there opens a window
//! ([`ToClient::CampOpen`]); taking over another hero or setting off on a run leaves the world and
//! comes back in as that hero ([`ToClient::CampLeave`]), the server knowing which is a run.

use benilla_app::{GroundLookModel, GroundLookModels};
use bevy::prelude::*;
use esurv_content::Faction;
use esurv_protocol::{ToClient, ToServer};

use crate::protocol::{Request, ServerEvent};

/// How a camp thing worn by a unit is tipped to its ground, which a unit cannot be: its pitch,
/// roll and sink, and how far its name is raised ([`benilla_app::PartPlacement`]). The Horde weapon rack stands on a slope: at its spot the ground rises 0.11 a yard ahead of it and 0.16 a yard to its left,
/// so its front is tipped up atan 0.11 and its left atan 0.16, and it is sunk by how far its low
/// back right corner (x -0.42, y -1.40) falls below its middle, 0.11 * 0.4 + 0.16 * 1.4. The Horde
/// Lore Tablet (TrollTablet) leans back against the Records' deck: its carved
/// face (+x) tipped up 30 degrees (its name sat inside it at 20), so its back (x
/// -0.31 at its 0.4) meets the deck's edge,
/// 0.71 up, 0.41 behind its foot; it rests on its back foot. Its name is raised 1.5 (0.6 yards at
/// its 0.4) clear of its top. The neutral camp's Bronze portal (COT_portal01)
/// had its name at its foot, inside the arch: raised 32 (6.4 yards at its 0.2), clear of its top,
/// 32.1 over its foot. It is tilted to the slope across
/// it and buried 8%: the ground
/// under its left foot (3.6 yards out) is 3.05 over the anchor spot, under its right 2.47, so its
/// left is rolled up atan(0.58 / 7.1), and it is sunk by how far its right foot's ground falls below
/// its middle's, 0.31 yards (1.55 at its 0.2), and 8% of its height, 35.69.
fn thing_tilt(display: u32) -> (f32, f32, f32, f32) {
    match display {
        esurv_content::camp_looks::CAMP_TAUREN_RACK_MODEL => (-6.3, 9.1, 0.27, 0.0),
        5 => (-30.0, 0.0, 0.0, 1.5),
        4713 => (0.0, 4.7, 0.31 / 0.2 + 0.08 * 35.69, 32.0),
        _ => (0.0, 0.0, 0.0, 0.0),
    }
}

/// The camp things drawn in other colours than the game made them in, by object display: the
/// Caverns portal (`COT_portal01`) with its violet nether, runes and sparks turned gold, the
/// bronze dragonflight's, its grey stone kept; the textures are the player's own, turned in
/// memory at load.
fn thing_recolor(display: u32) -> Option<benilla_app::HueShift> {
    match display {
        4713 => Some(benilla_app::HueShift {
            from: (225.0, 360.0),
            to: (28.0, 52.0),
            feather: 20.0,
            min_saturation: 0.4,
            brighten: 1.25,
        }),
        _ => None,
    }
}

/// The game's own objects (`GameObjectDisplayInfo.dbc`) the neutral camp's Alliance hubs are
/// dressed with, and its things to use the Horde sees as objects: the wagon, the smith's things,
/// the Market's goods, crates, sacks, barrel and rugs, the library's books, scroll, quill and
/// candles, the Muster's chairs, crates, sword, candles, goblet and brazier, the lectern's book,
/// the weapon rack, the crystal ball and the reading table, and the mage banners behind the Orb;
/// then the Horde hubs': the Muster's brazier, drum, skull candles, crates, barrel and sack, the
/// Market's crates, brazier and sack, the Mask's chest, the smithy's forge, anvil and tool rack, the
/// library's table, candle and purple brazier, the Forsaken banner, the weeds and the Lore Tablet.
const NEUTRAL_CAMP_STOCK_OBJECTS: [u32; 51] = [
    5392, 273, 233, 254, 381, 229, 1747, 4071, 6038, 225, 297, 31, 275, 2890, 32, 159, 113, 164, 2630, 107, 43, 100, 39, 6708, 669, 6363, 527, 197,
    130, 4891, 3371, 634,
    655, 2572, 6406, 5271, 5531, 457, 323, 408, 2450, 305, 208, 1487, 234, 4152, 474, 6388, 4111, 4112, 5,
];

/// Whether the camp replaces the warband screen: yes, unless `ESURV_HUB=0` (then the warband
/// screen is as before, and the Caverns of Time as in the runs).
pub fn hub_on() -> bool {
    std::env::var("ESURV_HUB").map_or(true, |v| v != "0")
}

/// Whether a faction has a camp yet (the server's `game::camp_of`): both have.
pub fn has_camp(faction: Faction) -> bool {
    matches!(faction, Faction::Alliance | Faction::Horde)
}

/// The achievement walking the camp with its guide earns (`achievements.ron`), its title "Recruit".
pub const RECRUIT: &str = "recruit";

/// Where the player is with the camp.
#[derive(Resource, Default, Debug)]
pub struct CampVisit {
    /// The hero is in the camp now (not in a run).
    pub in_camp: bool,
    /// The hero walking the camp (its guid), whom its windows are about.
    pub hero: Option<u64>,
    /// The warband as the character screen listed it on the way into the camp: the world keeps no
    /// roster, and the camp's windows read the heroes from it.
    pub roster: Vec<benilla_protocol::Character>,
    /// Back at the character screen, enter the world as this hero (taken over, or off to its
    /// run) rather than the one picked.
    pub enter_next: Option<u64>,
    /// Back at the character screen, the class picker for a new hero rather than the camp
    /// (`CampOpen("recruit")`): made, the new hero walks in; Back, the hero who left.
    pub recruit: bool,
    /// A window a thing in the camp opened, not yet shown (`equipment` ...).
    pub open: Option<String>,
    /// Wren is walking the hero round the camp.
    pub guided: bool,
    /// The warband has the Recruit title, earned walking the camp with its guide: the Orb's
    /// changes are open.
    pub recruited: bool,
    /// The camp fire burning low (its guid): its flames and sparks dimmed till it is fed.
    pub fire_low: Option<u64>,
}

mod caverns_pieces;
pub mod deco_spots;
pub mod hero_glow;
mod object_shadows;

pub(crate) fn register(app: &mut App) {
    hero_glow::register(app);
    object_shadows::register(app);
    deco_spots::register(app);
    caverns_pieces::register();
    // the camp's things to use are units wearing their object's model (`camp_looks`)
    if let Some(mut composed) = app.world_mut().get_resource_mut::<benilla_app::ComposedDisplays>() {
        composed.0.extend(esurv_content::camp_looks::CAMP_THING_MODELS.iter().map(|&(display, path)| {
            let (pitch_degrees, roll_degrees, sink, raise_attachments) = thing_tilt(display);
            benilla_app::ComposedDisplay {
                id: esurv_content::camp_looks::camp_thing_look(display),
                base: esurv_content::looks::PLANTED_ITEM_BASE,
                part: benilla_app::PartModel::Model(path.into()),
                placement: benilla_app::PartPlacement {
                    scale: 1.0,
                    pitch_degrees,
                    roll_degrees,
                    turn_degrees: 0.0,
                    sink,
                    raise_effects: 0.0,
                    raise_attachments,
                    hide_base_body: true,
                },
                recolor: thing_recolor(display),
            }
        }));
    }
    // the Orb's keeper in Dalaran's gear (`camp_looks`)
    app.world_mut().get_resource_or_insert_with(benilla_app::DressedDisplays::default).0.push(benilla_app::DressedDisplay {
        id: esurv_content::camp_looks::CAMP_ORB_KEEPER_LOOK,
        base: esurv_content::camp_looks::CAMP_ORB_KEEPER_BODY,
        equipment: esurv_content::camp_looks::CAMP_ORB_KEEPER_GEAR,
    });
    // the War Table's Marshal in the rank 14 warrior's plate (`camp_looks`)
    app.world_mut().get_resource_or_insert_with(benilla_app::DressedDisplays::default).0.push(benilla_app::DressedDisplay {
        id: esurv_content::camp_looks::CAMP_MARSHAL_LOOK,
        base: esurv_content::camp_looks::CAMP_MARSHAL_BODY,
        equipment: esurv_content::camp_looks::CAMP_MARSHAL_GEAR,
    });
    // the camp's runner, Wren, in her green leathers (`camp_looks`)
    app.world_mut().get_resource_or_insert_with(benilla_app::DressedDisplays::default).0.push(benilla_app::DressedDisplay {
        id: esurv_content::camp_looks::CAMP_WREN_LOOK,
        base: esurv_content::camp_looks::CAMP_WREN_BODY,
        equipment: esurv_content::camp_looks::CAMP_WREN_GEAR,
    });
    // the camp's buildings with no object of their own get a display of ours (`camp_looks`)
    // (inserted if the client has not yet: its own init keeps what is there)
    app.world_mut()
        .get_resource_or_insert_with(benilla_app::AddedGameObjectDisplays::default)
        .0
        .extend(esurv_content::camp_looks::CAMP_OBJECT_DISPLAYS.iter().map(|&(id, path)| (id, path.into())));
    // ... and the Horde's platform its undead hue
    app.world_mut()
        .get_resource_or_insert_with(benilla_app::RecoloredGameObjectDisplays::default)
        .0
        .extend([
            (esurv_content::camp_looks::CAMP_UNDEAD_PLATFORM_DISPLAY, esurv_content::camp_looks::CAMP_UNDEAD_PLATFORM_HUE),
            // ... and the neutral camp's hut the ruin's
            (esurv_content::camp_looks::CAMP_NEUTRAL_ORC_HUT_DISPLAY, esurv_content::camp_looks::CAMP_NEUTRAL_ORC_HUT_HUE),
            (esurv_content::camp_looks::CAMP_NEUTRAL_TROLL_BURROW_DISPLAY, esurv_content::camp_looks::CAMP_NEUTRAL_TROLL_BURROW_HUE),
        ]);
    // The tents and huts stand in the open: set into the ground, their feet are under the terrain,
    // where the client's room ray falls into the Caverns of Time's halls far below and hides them
    app.world_mut()
        .get_resource_or_insert_with(benilla_app::OutdoorGameObjectDisplays::default)
        .0
        .extend([
            esurv_content::camp_looks::CAMP_TENT_DISPLAY,
            esurv_content::camp_looks::CAMP_HORDE_TENT_DISPLAY,
            esurv_content::camp_looks::CAMP_ORC_TENT_DISPLAY,
            esurv_content::camp_looks::CAMP_NEUTRAL_ORC_HUT_DISPLAY,
            esurv_content::camp_looks::CAMP_NEUTRAL_TROLL_BURROW_DISPLAY,
            esurv_content::camp_looks::CAMP_NEUTRAL_STAIRS_DISPLAY,
            esurv_content::camp_looks::CAMP_NEUTRAL_CHROMIE_TABLE_DISPLAY,
            // the neutral camp's Alliance and Horde standards by Chromie, the Horde's foot at the
            // Alliance's height, under the slope where it stands
            5651,
            5652,
            // the neutral camp's War Table, and the Alliance's hubs' pieces of the camp's own
            esurv_content::camp_looks::CAMP_NEUTRAL_WAR_MAP_TABLE_DISPLAY,
            esurv_content::camp_looks::CAMP_GOL_BOLAR_PLATFORM_DISPLAY,
            esurv_content::camp_looks::CAMP_LIBRARY_LARGE_SHELF_DISPLAY,
            esurv_content::camp_looks::CAMP_LIBRARY_TABLE_DISPLAY,
            esurv_content::camp_looks::CAMP_LIBRARY_SMALL_SHELF_DISPLAY,
            esurv_content::camp_looks::CAMP_WAR_HELM_DISPLAY,
            esurv_content::camp_looks::CAMP_GNOME_TABLE_DISPLAY,
            esurv_content::camp_looks::CAMP_NEUTRAL_MAGIC_IMPLEMENT_1_DISPLAY,
            esurv_content::camp_looks::CAMP_NEUTRAL_MAGIC_IMPLEMENT_8_DISPLAY,
            // the Horde's hubs' pieces of the camp's own, and its War Table, rack and Mask as pieces
            esurv_content::camp_looks::CAMP_HORDE_KODO_RUG_DISPLAY,
            esurv_content::camp_looks::CAMP_HORDE_KODO_HIDE_DISPLAY,
            esurv_content::camp_looks::CAMP_HORDE_MARKET_BLANKET_DISPLAY,
            esurv_content::camp_looks::CAMP_HORDE_LARGE_BASKET_DISPLAY,
            esurv_content::camp_looks::CAMP_TROLL_SHOP_DISPLAY,
            esurv_content::camp_looks::CAMP_UNDEAD_PLATFORM_DISPLAY,
            esurv_content::camp_looks::CAMP_KARAZHAN_LARGE_SHELF_DISPLAY,
            esurv_content::camp_looks::CAMP_KARAZHAN_SMALL_SHELF_DISPLAY,
            esurv_content::camp_looks::CAMP_NEUTRAL_HORDE_WAR_MAP_TABLE_DISPLAY,
            esurv_content::camp_looks::CAMP_NEUTRAL_TAUREN_RACK_DISPLAY,
            esurv_content::camp_looks::CAMP_NEUTRAL_TIKI_MASK_DISPLAY,
        ]);
    // ... and, with the hub on, the game's own objects its hubs are dressed with: set on the
    // basins' floors over the Caverns' halls as well. Every object of these looks is outdoor then,
    // in the runs too.
    if hub_on() {
        app.world_mut().resource_mut::<benilla_app::OutdoorGameObjectDisplays>().0.extend(NEUTRAL_CAMP_STOCK_OBJECTS);
    }
    app.init_resource::<CampVisit>()
        .add_systems(Startup, draw_the_camp_looks)
        .add_systems(Update, (take_camp_lines, keep_the_chatter_option, dim_the_fire).chain());
}

/// How often the Camp chatter option is read, in seconds.
const CHATTER_OPTION_EVERY: f32 = 1.0;

/// The Camp chatter option ([`crate::camp_chatter_option`]), read now and then: told to the server
/// on the way into the camp and whenever it changes there (it mutes the heroes at Off); at Bubbles
/// only, the chat frame drops the warband's heroes' lines (`CHAT_MSG_MONSTER_SAY` by their names:
/// the bubbles come from the client's own chat bubbles, not the frame), Wren's and the keepers'
/// kept.
fn keep_the_chatter_option(
    script: Option<NonSend<benilla_ui::script::UiScript>>,
    visit: Res<CampVisit>,
    time: Res<Time>,
    mut requests: MessageWriter<Request>,
    mut next: Local<f32>,
    mut told: Local<Option<u32>>,
    mut hooked: Local<bool>,
) {
    let Some(script) = script else { return };
    if !visit.in_camp {
        if told.take() == Some(1) {
            let _ = script.eval::<()>("ESURV_CAMP_BUBBLES_ONLY = nil");
        }
        return;
    }
    let now = time.elapsed_secs();
    if told.is_some() && now < *next {
        return;
    }
    *next = now + CHATTER_OPTION_EVERY;
    if !*hooked {
        crate::camp_chatter_option::register_cvar(&script);
        let _ = script.eval::<()>(CHAT_FRAME_HOOK);
        *hooked = true;
    }
    let value = script
        .eval::<Option<String>>(&format!("return GetCVar(\"{}\")", crate::camp_chatter_option::CVAR))
        .ok()
        .flatten();
    let level = crate::camp_chatter_option::level(value.as_deref());
    if *told == Some(level) {
        return;
    }
    *told = Some(level);
    requests.write(Request(ToServer::CampChatter(level)));
    let names: Vec<String> = visit
        .roster
        .iter()
        .filter(|c| Some(c.guid) != visit.hero && !c.name.contains(['"', '\\']))
        .map(|c| format!("[\"{}\"] = true", c.name))
        .collect();
    let table = if level == 1 { format!("{{ {} }}", names.join(", ")) } else { "nil".into() };
    let _ = script.eval::<()>(&format!("ESURV_CAMP_BUBBLES_ONLY = {table}"));
}

/// The chat frame's camp filter: while `ESURV_CAMP_BUBBLES_ONLY` names the warband's heroes, their
/// lines and emote lines (`arg2` the speaker) are not printed. Wrapped once.
const CHAT_FRAME_HOOK: &str = r#"
if ChatFrame_OnEvent and not ESURV_CAMP_CHAT_HOOKED then
    ESURV_CAMP_CHAT_HOOKED = true
    local stock_chat_event = ChatFrame_OnEvent
    ChatFrame_OnEvent = function(ev, a2, a3, a4)
        if (ev == "CHAT_MSG_MONSTER_SAY" or ev == "CHAT_MSG_MONSTER_EMOTE") and ESURV_CAMP_BUBBLES_ONLY and arg2 and ESURV_CAMP_BUBBLES_ONLY[arg2] then
            return
        end
        return stock_chat_event(ev, a2, a3, a4)
    end
end
"#;

/// How far the fire's flames and sparks die down while it burns low: births and brightness alike
/// (flavour).
const FIRE_LOW_DIM: f32 = 0.3;

/// The camp fire burning low ([`ToClient::CampFireLow`]): the clouds of its model dimmed ("only dim the particles", the model left as it is), and lit again once it is fed
/// or the camp is left. Its clouds come in with its model, so this holds them each frame.
fn dim_the_fire(
    visit: Res<CampVisit>,
    objects: benilla_app::Objects,
    parents: Query<&ChildOf>,
    mut emitters: Query<&mut benilla_world::particles::ParticleEmitter>,
) {
    let fire = visit.fire_low.filter(|_| visit.in_camp).and_then(|g| objects.entity(g));
    let of_the_fire = |e: Entity| fire.is_some_and(|f| e == f || parents.iter_ancestors(e).any(|a| a == f));
    for mut em in &mut emitters {
        let want = if em.anchor().is_some_and(of_the_fire) { FIRE_LOW_DIM } else { 1.0 };
        if em.dim() != want {
            em.set_dim(want);
        }
    }
}

/// The camp's models that are no game object (the tents) are drawn for their dynamic objects
/// (`esurv_content::camp_looks`), beside the ground looks' own.
fn draw_the_camp_looks(models: Option<ResMut<GroundLookModels>>) {
    let Some(mut models) = models else {
        return;
    };
    for camp in esurv_content::camp_looks::CAMP_LOOKS {
        let (id, look) = (camp.id, camp.model);
        let model = GroundLookModel { path: look.path.into(), scale: look.scale, lift: look.lift, tint: look.tint, once: false, recolor: false, whole: true, lean_back_degrees: camp.lean_back, level_sideways: camp.level_sideways, rest_on: camp.rest_on.to_vec() };
        models.0.insert(id, vec![model]);
    }
}

/// The server's camp lines: in the camp, a window to open, off to another hero or a run.
fn take_camp_lines(
    mut events: MessageReader<ServerEvent>,
    mut visit: ResMut<CampVisit>,
    mut requests: MessageWriter<Request>,
) {
    for ServerEvent(m) in events.read() {
        match m {
            ToClient::Camp { hero } => {
                visit.in_camp = true;
                visit.hero = Some(*hero);
                visit.fire_low = None;
            }
            // the Orb's changes open with the Recruit title (sheet::appearance_shut)
            ToClient::Achievements(a) => visit.recruited = a.earned.iter().any(|(id, _)| id == RECRUIT),
            // a run's own `Ready`: not the camp
            ToClient::Ready(_) => visit.in_camp = false,
            ToClient::CampOpen(window) => visit.open = Some(window.clone()),
            ToClient::CampGuided(on) => visit.guided = *on,
            ToClient::CampFireLow { fire, low } => visit.fire_low = low.then_some(*fire),
            ToClient::CampLeave { enter } => {
                info!("camp: leaving the camp, then in as {enter:#x}");
                visit.enter_next = Some(*enter);
                visit.in_camp = false;
                requests.write(Request(ToServer::Leave));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At Bubbles only the chat frame drops the named heroes' lines and prints Wren's and every
    /// other kind; with no names (the other levels) it prints all.
    #[test]
    fn bubbles_only_keeps_the_heroes_out_of_the_chat_frame() {
        let lua = mlua::Lua::new();
        lua.load("LINES = 0; function ChatFrame_OnEvent(ev) LINES = LINES + 1 end").exec().unwrap();
        lua.load(CHAT_FRAME_HOOK).exec().unwrap();
        lua.load(CHAT_FRAME_HOOK).exec().unwrap();
        let lines = |chunk: &str| lua.load(format!("LINES = 0; {chunk}; return LINES")).eval::<i64>().unwrap();
        let say = "arg2 = 'Brom'; ChatFrame_OnEvent('CHAT_MSG_MONSTER_SAY')";
        assert_eq!(lines(say), 1);
        lua.load("ESURV_CAMP_BUBBLES_ONLY = { [\"Brom\"] = true }").exec().unwrap();
        assert_eq!(lines(say), 0);
        assert_eq!(lines("arg2 = 'Wren'; ChatFrame_OnEvent('CHAT_MSG_MONSTER_SAY')"), 1);
        assert_eq!(lines("arg2 = 'Brom'; ChatFrame_OnEvent('CHAT_MSG_SAY')"), 1);
        assert_eq!(lines("arg2 = 'Brom'; ChatFrame_OnEvent('CHAT_MSG_MONSTER_EMOTE')"), 0);
        lua.load("ESURV_CAMP_BUBBLES_ONLY = nil").exec().unwrap();
        assert_eq!(lines(say), 1, "wrapped once, and open again");
    }
}
