//! The ui rework's look on the stock frames the game keeps: the camp's talk window and the
//! run's portrait ring, in the menus' colours ([`crate::menus::look::palette`]); the action bar
//! bronze, a dragon of our own at each end for the gryphons ([`DRAGON_CAP`]), and the art the
//! run's bar is recreated in ([`BAR_ART`]). All Lua in the
//! interface VM ([`LUA`], `stock_look.lua`), run once in each VM once the hero is in the world.
//!
//! `ESURV_TALK_DEMO=1`, a development aid: once installed, opens a sample talk window (Chromie's
//! greeting) with no one to talk to, to see its look.

use bevy::prelude::*;

use benilla_app::Objects;
use benilla_ui::script::{GossipMenu, GossipOptionView, UiScript};

use crate::menus::look::palette;
use crate::ui::in_world;

/// The Lua half. Returns true once installed.
pub const LUA: &str = include_str!("stock_look.lua");

/// The action bar's end caps: the bronze
/// dragon, cut from its grey ground, 128 square, facing right: inward from the bar's left end.
const DRAGON_CAP: &[u8] = include_bytes!("../assets/dragon_cap.tga");
/// The path the Lua sets it by, which the archive chain answers from [`DRAGON_CAP`].
const DRAGON_CAP_PATH: &str = "Interface\\ESurv\\DragonCap.tga";
/// The experience bar's fill, now a red and blue night: a band of it turned a quarter and brightened, and its mirror, twice over, 4096
/// by 16; `stock_look.lua` slides a window of it along the fill.
const XP_SWIRL: &[u8] = include_bytes!("../assets/xp_swirl.tga");
const XP_SWIRL_PATH: &str = "Interface\\ESurv\\XpSwirl.tga";
/// The run's action bar, recreated: our
/// own pieces in the dragons' bronze, drawn after a mock-up, by the paths
/// `run_frames/action_bar.lua` sets them by. The frame round the buttons and round the menu
/// buttons (nine-sliced, corners a quarter), each button's well (a faint scale pattern in it, our
/// touch) and rim, the frames' small shadow, the experience bar's frame ticked in tenths, the menu buttons
/// (spellbook a book, talents a target, options a computer), the character button's frame round the
/// hero's portrait, and the menu buttons' glow. And the run's minimap
/// (`run_frames/minimap.lua`): its thin gold ring with
/// the four points, the zone's night-blue ribbon, the zoom marks and the round buttons' ring. And
/// the run's player and target frames (`run_frames/unit_frames.lua`): the portrait's ring, the
/// plate, the name's strip (grey, tinted) and the level's diamond.
const BAR_ART: &[(&str, &[u8])] = &[
    ("Interface\\ESurv\\BarFrame.tga", include_bytes!("../assets/bar_frame.tga")),
    ("Interface\\ESurv\\BarWell.tga", include_bytes!("../assets/bar_well.tga")),
    ("Interface\\ESurv\\BarRim.tga", include_bytes!("../assets/bar_rim.tga")),
    ("Interface\\ESurv\\BarShadow.tga", include_bytes!("../assets/bar_shadow.tga")),
    ("Interface\\ESurv\\XpFrame.tga", include_bytes!("../assets/xp_frame.tga")),
    ("Interface\\ESurv\\MicroBook.tga", include_bytes!("../assets/micro_book.tga")),
    ("Interface\\ESurv\\MicroTarget.tga", include_bytes!("../assets/micro_target.tga")),
    ("Interface\\ESurv\\MicroPc.tga", include_bytes!("../assets/micro_pc.tga")),
    ("Interface\\ESurv\\MicroPortrait.tga", include_bytes!("../assets/micro_portrait.tga")),
    ("Interface\\ESurv\\MicroGlow.tga", include_bytes!("../assets/micro_glow.tga")),
    ("Interface\\ESurv\\MapRing.tga", include_bytes!("../assets/map_ring.tga")),
    ("Interface\\ESurv\\MapRibbon.tga", include_bytes!("../assets/map_ribbon.tga")),
    ("Interface\\ESurv\\MapPlus.tga", include_bytes!("../assets/map_plus.tga")),
    ("Interface\\ESurv\\MapMinus.tga", include_bytes!("../assets/map_minus.tga")),
    ("Interface\\ESurv\\MapButtonRing.tga", include_bytes!("../assets/map_button_ring.tga")),
    ("Interface\\ESurv\\UfRing.tga", include_bytes!("../assets/uf_ring.tga")),
    ("Interface\\ESurv\\UfPlate.tga", include_bytes!("../assets/uf_plate.tga")),
    ("Interface\\ESurv\\UfPlateEdge.tga", include_bytes!("../assets/uf_plate_edge.tga")),
    ("Interface\\ESurv\\UfName.tga", include_bytes!("../assets/uf_name.tga")),
    ("Interface\\ESurv\\UfLevel.tga", include_bytes!("../assets/uf_level.tga")),
];

pub(crate) fn add(app: &mut App) {
    app.add_systems(Update, (provide_the_art, keep_the_look).chain());
}

/// Puts the game's own art in the archive chain, once for each chain (the world's assets open it
/// once the data folder is known).
fn provide_the_art(assets: Option<Res<benilla_assets::WorldAssets>>, mut done: Local<Option<usize>>) {
    use benilla_assets::LockRecover;
    let Some(assets) = assets else {
        return;
    };
    let chain = std::sync::Arc::as_ptr(&assets.chain) as usize;
    if *done == Some(chain) {
        return;
    }
    let mut files = assets.chain.lock_recover();
    files.provide(DRAGON_CAP_PATH, DRAGON_CAP.to_vec());
    files.provide(XP_SWIRL_PATH, XP_SWIRL.to_vec());
    for (path, bytes) in BAR_ART {
        files.provide(path, bytes.to_vec());
    }
    drop(files);
    *done = Some(chain);
}

/// The colour table the Lua reads: the palette's, in 0 to 1.
fn colours_chunk() -> String {
    let c = |name: &str, colour: Color| {
        let s = colour.to_srgba();
        format!("{name} = {{ {:.3}, {:.3}, {:.3}, {:.3} }}", s.red, s.green, s.blue, s.alpha)
    };
    format!(
        "ESURV_LOOK = {{ colours = {{ {}, {}, {}, {}, {}, {}, {} }}, dragon_cap = {:?}, xp_swirl = {:?} }}",
        c("panel", palette::PANEL),
        c("bronze", palette::BRONZE),
        c("bronze_light", palette::LABEL),
        c("heading", palette::HEADING),
        c("value", palette::VALUE),
        c("bar", palette::CARVED_LIGHT),
        c("gold", palette::GOLD_DEEP),
        DRAGON_CAP_PATH,
        XP_SWIRL_PATH,
    )
}

/// The sample talk window of `ESURV_TALK_DEMO`.
fn demo_menu() -> GossipMenu {
    let blue = |words: &str, action: &str| format!("{words} |cff{}<{action}>|r", esurv_protocol::ACTION_BLUE_HEX);
    let option = |label: String, icon: &str| GossipOptionView { label, icon_type: icon.into() };
    GossipMenu {
        greeting: "Oh good, another one! Don't worry, the timeways spit everyone out a little dizzy. \
                   Stand still a moment, you'll feel better."
            .into(),
        quests: Vec::new(),
        options: vec![
            option(blue("I'd like to recruit a new hero.", "Create a new hero"), "gossip"),
            option("Why a gnome?".into(), "gossip"),
            option("Tell me about the portal.".into(), "gossip"),
            option(blue("Can you explain everything again?", "Tour around camp"), "taxi"),
        ],
    }
}

#[derive(Default)]
struct Told {
    session: u64,
    installed: bool,
    demo: bool,
}

fn keep_the_look(script: Option<NonSendMut<UiScript>>, objects: Objects, mut told: Local<Told>, mut warned: Local<bool>) {
    let Some(mut script) = script else {
        return;
    };
    if told.session != script.session() {
        *told = Told { session: script.session(), ..default() };
    }
    if told.installed || !in_world(&objects) {
        return;
    }
    if let Err(e) = script.run(&colours_chunk()) {
        warn!("stock look: the colours: {e}");
        return;
    }
    match script.eval::<bool>(LUA) {
        Ok(true) => {
            info!("stock look: installed in the interface");
            told.installed = true;
        }
        Ok(false) => return,
        Err(e) => {
            if !std::mem::replace(&mut *warned, true) {
                warn!("stock look: the Lua did not load: {e}");
            }
            return;
        }
    }
    if !told.demo && std::env::var("ESURV_TALK_DEMO").is_ok_and(|v| v == "1") {
        told.demo = true;
        script.set_gossip(Some(demo_menu()));
        script.fire_event("GOSSIP_SHOW", Vec::new());
        // no one is talked to: the name the window would show
        if let Err(e) = script.run("GossipFrameNpcNameText:SetText(\"Chromie <Keeper of Time>\")") {
            warn!("stock look: the sample's name: {e}");
        }
        info!("stock look: the sample talk window (ESURV_TALK_DEMO)");
    }
}
