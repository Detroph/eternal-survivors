//! The game's own client side, added to the benilla client through `benilla_app::run_with`: area
//! looks at their true size, the messages with the server ([`protocol`]), and the game's own
//! screens (design step 6), drawn with `bevy_ui` over the game: so far the level-up and cache
//! cards ([`cards`]), the run's HUD with its Escape menu ([`hud`]), the end of a run
//! ([`run_end`]), the damage meter ([`meter`]), the hero picker at the character select
//! ([`picker`]), what the hero sheet before a run will show ([`hero_sheet`]: the class's progress
//! and the codex, drawn by the menu rework), the stock frames a run keeps and blocks
//! ([`run_frames`]), and the stock 1.12 look kit every screen but the meter is built from
//! ([`stock`]).

pub mod boss_alerts;
mod boss_bar_drag_check;
pub mod boss_bar_places;
pub mod buff_frame;
pub mod cards;
pub mod embers;
pub mod enemy_glow;
mod ground_indicators;
mod ground_models;
mod window_icon;
pub mod content_reload;
pub mod glow_option;
pub mod hero_sheet;
mod hero_shots;
pub mod hud;
pub mod menus;
pub mod meter;
pub mod minimap_buttons;
pub mod picker;
pub mod prerun;
pub mod protocol;
mod races;
pub mod run_end;
pub mod run_frames;
pub mod smart_numbers;
pub mod stock;
pub mod ui;
pub mod update;
pub mod zone_discovery;

use std::sync::Arc;

use benilla_app::{NetHandlerApp, SheathHolds, SpellLookSizes, SpellLookStyle, SpellLookStyles};
use benilla_protocol::{SessionEvent, SessionEventKind};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use esurv_content::LiveContent;
use esurv_protocol::LOOK_SIZE_PREFIX;

/// The looks benilla keeps level on their flights ([`benilla_app::LevelFlight`]): every pet that
/// flies (`hover` in `pets.ron`). The server keeps such a pet on short flights that bob it at its
/// height, and the stock flight attitude would pitch it with each bob and bank it over at each
/// flight's end; it hovers upright instead.
pub fn level_flight(content: &esurv_content::Content) -> benilla_app::LevelFlight {
    benilla_app::LevelFlight(content.pets.iter().filter(|p| p.hover.is_some()).map(|p| p.display_id).collect())
}

/// Adds the game's systems to the client app; `content` is the game's content, for the names,
/// icons and texts the screens show (`None` leaves them to ids), which they follow as it is loaded
/// again ([`content_reload`]); `link` is the way to the server before a run, for the character
/// screen (a local game's own save; `None` without one).
pub fn extend(app: &mut App, content: Option<Arc<LiveContent>>, link: Option<prerun::PreRunLink>) {
    app.add_systems(Startup, name_the_window);
    window_icon::register(app);
    app.net_handler(SessionEventKind::Chat, take_look_size);
    app.add_systems(Update, (take_look_styles, take_sheath_holds));
    if let Some(live) = &content {
        // the create screen's added pairs and their looks are read once, at launch
        races::register(app, &live.current());
        // pets that fly hover upright, read once at launch
        app.insert_resource(level_flight(&live.current()));
    }
    ui::register(app, content.as_ref().map(|live| live.current()));
    if let Some(live) = content {
        content_reload::register(app, live);
    }
    protocol::register(app);
    hero_shots::register(app);
    ground_models::register(app);
    ground_indicators::register(app);
    buff_frame::register(app);
    embers::register(app);
    boss_alerts::register(app);
    boss_bar_drag_check::register(app);
    smart_numbers::register(app);
    enemy_glow::register(app);
    minimap_buttons::register(app);
    app.add_plugins((
        stock::StockPlugin,
        cards::CardsPlugin,
        hud::HudPlugin,
        meter::MeterPlugin,
        run_end::RunEndPlugin,
        picker::PickerPlugin,
        menus::MenusPlugin,
        run_frames::RunFramesPlugin,
    ));
    if let Some(link) = link {
        app.insert_resource(link);
    }
}

/// The game's name, as the player sees it: the window's title.
pub const GAME_TITLE: &str = "Eternal Survivors";

/// Titles the game's window with [`GAME_TITLE`] (benilla's own is `benilla`).
fn name_the_window(mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    for mut window in &mut windows {
        window.title = GAME_TITLE.into();
    }
}

/// A spell look's size from the server, for the cast that follows it: an area's look shows its
/// true size.
fn take_look_size(In(ev): In<SessionEvent>, mut sizes: ResMut<SpellLookSizes>) {
    let SessionEvent::Chat(line) = ev else {
        return;
    };
    if !line.is_addon() {
        return;
    }
    if let Some((caster, spell, scale)) = look_size(&line.text) {
        sizes.set(caster, spell, scale);
    }
}

/// How some spells' looks are drawn, from the server (`ESURVZ`).
fn take_look_styles(mut events: MessageReader<protocol::ServerEvent>, styles: Option<ResMut<SpellLookStyles>>) {
    let Some(mut styles) = styles else {
        return;
    };
    for protocol::ServerEvent(m) in events.read() {
        if let esurv_protocol::ToClient::LookStyles(list) = m {
            for s in list {
                styles.set(
                    s.spell,
                    SpellLookStyle {
                        scale: s.scale,
                        tint: s.tint,
                        recolor: s.recolor,
                        flat: s.flat,
                        no_shake: s.no_shake,
                        opacity: s.opacity,
                    },
                );
            }
        }
    }
}

/// The heroes whose weapons keep one sheath state all run, from the server (`ESURVI`)
/// (Moonkin Form put away, a shaman's two-hander in hand). Each list replaces the last.
fn take_sheath_holds(mut events: MessageReader<protocol::ServerEvent>, holds: Option<ResMut<SheathHolds>>) {
    let Some(mut holds) = holds else {
        return;
    };
    for protocol::ServerEvent(m) in events.read() {
        if let esurv_protocol::ToClient::SheathHolds(list) = m {
            holds.clear();
            for h in list {
                holds.hold(h.hero, h.state);
            }
        }
    }
}

/// Reads `ESURVS<TAB><caster guid, hex> <spell> <scale>`.
fn look_size(text: &str) -> Option<(u64, u32, f32)> {
    let rest = text.strip_prefix(LOOK_SIZE_PREFIX)?.strip_prefix('\t')?;
    let mut parts = rest.split(' ');
    let caster = u64::from_str_radix(parts.next()?, 16).ok()?;
    let spell = parts.next()?.parse().ok()?;
    let scale: f32 = parts.next()?.parse().ok()?;
    (scale.is_finite() && scale > 0.0).then_some((caster, spell, scale))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pets that fly (the assist's Gargoyle) are kept level on their flights; walking pets are
    /// not listed.
    #[test]
    fn the_pets_that_fly_hover_upright() {
        let content = esurv_content::Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap();
        let level = level_flight(&content);
        let gargoyle = content.pets.iter().find(|p| p.name == "Gargoyle").unwrap();
        assert!(gargoyle.hover.is_some() && level.0.contains(&gargoyle.display_id));
        let wolf = content.pets.iter().find(|p| p.name == "Wolf").unwrap();
        assert!(!level.0.contains(&wolf.display_id));
    }

    #[test]
    fn a_look_size_reads_back() {
        assert_eq!(
            look_size("ESURVS\tF130251C00000007 8187 1.250"),
            Some((0xF130_251C_0000_0007, 8187, 1.25))
        );
        assert_eq!(
            look_size("ESURV\thealth=1;"),
            None,
            "the numbers line is not a size"
        );
        assert_eq!(
            look_size("ESURVS\t7 122 0"),
            None,
            "no look shrinks to nothing"
        );
        assert_eq!(look_size("ESURVS\t7 122"), None);
    }

    /// A look is drawn whole unless its style says otherwise; a see-through style (the boss marks)
    /// reaches the client's styles with its opacity.
    #[test]
    fn a_look_style_is_whole_by_default_and_carries_its_opacity() {
        assert_eq!(SpellLookStyle::default().opacity, 1.0);
        let mut app = App::new();
        app.add_message::<protocol::ServerEvent>().insert_resource(SpellLookStyles::default()).add_systems(Update, take_look_styles);
        let mark = esurv_protocol::LookStyle { spell: 20924, scale: 1.0, tint: Some([1.0, 0.02, 0.0]), recolor: true, flat: false, no_shake: false, opacity: 0.5 };
        app.world_mut().write_message(protocol::ServerEvent(esurv_protocol::ToClient::LookStyles(vec![mark])));
        app.update();
        assert_eq!(app.world().resource::<SpellLookStyles>().get(20924).map(|s| s.opacity), Some(0.5));
    }
}
