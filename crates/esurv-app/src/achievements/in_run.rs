//! The warband's achievements in a run (screens.md, "Achievements in a run"): the "Achievement
//! earned" banner and the tracked achievements' list.
//!
//! * **The banner.** On `ToClient::AchievementEarned` a gold banner slides up at the top centre,
//!   under the HUD's readouts: the achievement's icon, "Achievement earned", its name and a shield
//!   with its points, with the stock quest-complete sound. It holds [`BANNER_HOLD`] seconds and
//!   fades; several earned at once wait their turn. The chat line is the server's.
//! * **The tracker.** The achievements ticked in the Achievements window (at most three), as a small
//!   list at the right under the minimap, each with its progress ("312 / 1,000"), kept up to date
//!   by every `ToClient::Achievements` of the run.
//!
//! Both wear the menus' look ([`crate::menus::look`]). `ESURV_ACHIEVEMENT_DEMO=<id>[,<id>...]` (a
//! dev aid) shows these achievements' banners a few seconds into the world, one after the other.

use std::collections::VecDeque;

use bevy::prelude::*;

use benilla_app::Objects;
use benilla_ui::script::UiScript;
use esurv_protocol::{Achievements, ToClient};

use crate::menus::look::{palette, Face, MenuLook};
use crate::protocol::ServerEvent;
use crate::ui::{in_world, layer, GameContent};

/// The banner's timing, seconds: sliding up and fading in, holding, fading out.
pub const BANNER_RISE: f32 = 0.35;
pub const BANNER_HOLD: f32 = 4.0;
pub const BANNER_FADE: f32 = 0.8;
/// How far the banner rises as it comes, in units.
const BANNER_SLIDE: f32 = 24.0;
/// Where the banner and the tracker sit, in units: the banner's top under the HUD's readouts and
/// the boss bar; the tracker's top under the minimap (the run's, a fifth larger than the stock one,
/// its south point at 237: `run_frames/minimap.lua`), at the right.
const BANNER_TOP: f32 = 124.0;
const BANNER_WIDTH: f32 = 300.0;
const BANNER_ICON: f32 = 36.0;
const TRACKER_TOP: f32 = 246.0;
const TRACKER_RIGHT: f32 = 16.0;
const TRACKER_WIDTH: f32 = 210.0;
/// The client's quest-complete sound (`igQuestListComplete`, `Sound\Interface\iQuestComplete.wav`).
pub const EARNED_SOUND: &str = "igQuestListComplete";
/// How long into the world the dev aid's first banner comes, seconds.
const DEMO_AFTER: f32 = 6.0;

/// What the run shows of the warband's achievements.
#[derive(Resource, Default)]
pub struct RunAchievements {
    /// The warband's achievements as last told in this run.
    pub told: Option<Achievements>,
    /// The banners waiting, by achievement id.
    pub waiting: VecDeque<String>,
    /// The banner up: its achievement, and when it came (the game's seconds).
    pub showing: Option<(String, f32)>,
}

impl RunAchievements {
    /// The banner's place in its life at `now`: its strength (0 to 1) and how far it still has to
    /// rise (units); `None` once it is over.
    pub fn banner_at(&self, now: f32) -> Option<(f32, f32)> {
        let (_, at) = self.showing.as_ref()?;
        banner_phase(now - at)
    }

    /// The next banner comes up when none is showing (or the one up is over): its id.
    fn next_banner(&mut self, now: f32) -> Option<String> {
        if self.banner_at(now).is_some() {
            return None;
        }
        self.showing = None;
        let id = self.waiting.pop_front()?;
        self.showing = Some((id.clone(), now));
        Some(id)
    }
}

/// A banner `age` seconds old: its strength and how far it still has to rise; `None` once over.
pub fn banner_phase(age: f32) -> Option<(f32, f32)> {
    if age < 0.0 || age > BANNER_RISE + BANNER_HOLD + BANNER_FADE {
        return None;
    }
    if age < BANNER_RISE {
        let t = age / BANNER_RISE;
        return Some((t, BANNER_SLIDE * (1.0 - t)));
    }
    let fading = age - BANNER_RISE - BANNER_HOLD;
    if fading > 0.0 {
        return Some((1.0 - fading / BANNER_FADE, 0.0));
    }
    Some((1.0, 0.0))
}

/// The banner's and the tracker's nodes.
#[derive(Resource)]
struct Parts {
    banner: Entity,
    tracker: Entity,
    /// What each was drawn for.
    banner_for: Option<String>,
    tracker_for: Option<u64>,
}

#[derive(Component)]
struct BannerRoot;

pub(crate) fn register(app: &mut App) {
    app.init_resource::<RunAchievements>().add_systems(
        Update,
        (
            read_the_server,
            queue_the_demo,
            show_the_banner,
            fade_the_banner,
            draw_the_tracker,
        )
            .chain(),
    );
}

/// The server's achievements and earns; a new run's `Ready` drops the banners still waiting.
fn read_the_server(mut events: MessageReader<ServerEvent>, mut run: ResMut<RunAchievements>) {
    for ServerEvent(m) in events.read() {
        match m {
            ToClient::Achievements(a) => run.told = Some(a.clone()),
            ToClient::AchievementEarned { id, .. } => run.waiting.push_back(id.clone()),
            ToClient::Ready(_) => {
                run.waiting.clear();
                run.showing = None;
            }
            _ => {}
        }
    }
}

/// `ESURV_ACHIEVEMENT_DEMO`: the named banners, once, a few seconds into the world.
fn queue_the_demo(
    objects: Objects,
    time: Res<Time>,
    mut since: Local<Option<f32>>,
    mut done: Local<bool>,
    mut run: ResMut<RunAchievements>,
) {
    if *done {
        return;
    }
    let Ok(ids) = std::env::var("ESURV_ACHIEVEMENT_DEMO") else {
        *done = true;
        return;
    };
    if !in_world(&objects) {
        *since = None;
        return;
    }
    let at = *since.get_or_insert(time.elapsed_secs());
    if time.elapsed_secs() - at >= DEMO_AFTER {
        run.waiting.extend(
            ids.split(',')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty()),
        );
        *done = true;
    }
}

/// The banner up: the next one comes as the last ends (its sound with it), and it rises, holds
/// and fades.
#[allow(clippy::too_many_arguments)] // one Bevy system's inputs
fn show_the_banner(
    mut commands: Commands,
    mut run: ResMut<RunAchievements>,
    parts: Option<ResMut<Parts>>,
    content: Option<Res<GameContent>>,
    look: MenuLook,
    time: Res<Time>,
    objects: Objects,
    script: Option<NonSend<UiScript>>,
    mut banners: Query<(&mut Node, &mut Visibility), With<BannerRoot>>,
) {
    let now = time.elapsed_secs();
    let Some(mut parts) = parts else {
        let banner = commands
            .spawn((
                Name::new("achievement banner"),
                BannerRoot,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    top: look.px(BANNER_TOP),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                GlobalZIndex(layer::HUD + 5),
                Pickable::IGNORE,
                Visibility::Hidden,
            ))
            .id();
        let tracker = commands
            .spawn((
                Name::new("achievement tracker"),
                Node {
                    position_type: PositionType::Absolute,
                    top: look.px(TRACKER_TOP),
                    right: look.px(TRACKER_RIGHT),
                    width: look.px(TRACKER_WIDTH),
                    ..default()
                },
                GlobalZIndex(layer::HUD),
                Pickable::IGNORE,
            ))
            .id();
        commands.insert_resource(Parts {
            banner,
            tracker,
            banner_for: None,
            tracker_for: None,
        });
        return;
    };
    if !in_world(&objects) {
        run.showing = None;
    }
    if let Some(_id) = run.next_banner(now) {
        if let Some(script) = &script {
            let _ = script.eval::<()>(&format!("PlaySound(\"{EARNED_SOUND}\")"));
        }
    }
    let Ok((mut node, mut shown)) = banners.get_mut(parts.banner) else {
        return;
    };
    let phase = run.banner_at(now).filter(|_| in_world(&objects));
    let Some((strength, rise)) = phase else {
        shown.set_if_neq(Visibility::Hidden);
        return;
    };
    shown.set_if_neq(Visibility::Inherited);
    node.top = look.px(BANNER_TOP + rise);
    let id = run.showing.as_ref().map(|(id, _)| id.clone());
    if parts.banner_for != id {
        parts.banner_for = id.clone();
        commands.entity(parts.banner).despawn_related::<Children>();
        // an achievement, or a bounty earned again (its banner says so)
        let earned = id.as_deref().and_then(|id| {
            let c = &content.as_ref()?.0;
            match c.achievement(id) {
                Some(a) => Some(("Achievement earned", a.icon.clone(), a.name.clone(), a.points)),
                None => c.bounty(id).map(|b| ("Bounty earned", b.icon.clone(), b.name.clone(), b.points)),
            }
        });
        if let Some((heading, icon, name, points)) = earned {
            commands
                .entity(parts.banner)
                .with_children(|b| banner(b, &look, heading, &icon, &name, points));
        }
    }
    // the whole banner fades as one: its parts' colours carry the strength through the alpha
    commands
        .entity(parts.banner)
        .insert(BannerStrength(strength));
}

/// How strong the banner is now (0 to 1), which its parts' colours follow.
#[derive(Component, Clone, Copy)]
struct BannerStrength(f32);

/// The banner's content: the icon, `heading` ("Achievement earned") over the name, the points'
/// shield.
fn banner(b: &mut ChildSpawnerCommands, look: &MenuLook, heading: &str, icon: &str, name: &str, points: u32) {
    b.spawn((
        Node {
            width: look.px(BANNER_WIDTH),
            align_items: AlignItems::Center,
            column_gap: look.px(10.0),
            padding: UiRect::axes(look.px(10.0), look.px(7.0)),
            border: UiRect::all(look.px(1.5)),
            border_radius: BorderRadius::all(look.px(6.0)),
            ..default()
        },
        BackgroundColor(palette::PANEL),
        BorderColor::all(palette::HEADING),
        BoxShadow::new(
            palette::GLOW.with_alpha(0.55),
            Val::ZERO,
            Val::ZERO,
            look.px(1.0),
            look.px(9.0),
        ),
        Pickable::IGNORE,
    ))
    .with_children(|row| {
        crate::menus::look::plain_icon(row, look, look.icon_art(icon), BANNER_ICON);
        row.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                row_gap: look.px(2.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|t| {
            t.spawn((look.label(heading), Pickable::IGNORE));
            t.spawn((
                look.text(Face::Display, 16.0, palette::HEADING, name.to_owned()),
                Pickable::IGNORE,
            ));
        });
        crate::menus::look::points_shield(row, look, points, true);
    });
}

/// A part of the banner's own colours, as drawn, which its fade scales.
#[derive(Component, Clone, Copy)]
struct FadeBase {
    text: Option<Color>,
    image: Option<Color>,
    back: Option<Color>,
    border: Option<Color>,
}

/// Every part of the banner takes the banner's strength times its own alpha.
#[allow(clippy::type_complexity)] // one Bevy system's queries
fn fade_the_banner(
    mut commands: Commands,
    banners: Query<(&BannerStrength, &Children)>,
    children: Query<&Children>,
    mut parts: Query<(
        Option<&FadeBase>,
        Option<&mut TextColor>,
        Option<&mut ImageNode>,
        Option<&mut BackgroundColor>,
        Option<&mut BorderColor>,
    )>,
) {
    let scaled = |c: Color, a: f32| c.with_alpha(c.alpha() * a);
    for (strength, kids) in &banners {
        let a = strength.0.clamp(0.0, 1.0);
        let mut stack: Vec<Entity> = kids.iter().collect();
        while let Some(e) = stack.pop() {
            if let Ok(more) = children.get(e) {
                stack.extend(more.iter());
            }
            let Ok((base, text, image, back, border)) = parts.get_mut(e) else {
                continue;
            };
            let base = match base {
                Some(b) => *b,
                None => {
                    let b = FadeBase {
                        text: text.as_ref().map(|t| t.0),
                        image: image.as_ref().map(|i| i.color),
                        back: back.as_ref().map(|c| c.0),
                        border: border.as_ref().map(|c| c.top),
                    };
                    commands.entity(e).insert(b);
                    b
                }
            };
            if let (Some(mut t), Some(c)) = (text, base.text) {
                t.0 = scaled(c, a);
            }
            if let (Some(mut i), Some(c)) = (image, base.image) {
                i.color = scaled(c, a);
            }
            if let (Some(mut bg), Some(c)) = (back, base.back) {
                bg.0 = scaled(c, a);
            }
            if let (Some(mut b), Some(c)) = (border, base.border) {
                b.set_all(scaled(c, a));
            }
        }
    }
}

/// The tracker: drawn anew when what it shows changes; hidden out of the world or with nothing
/// tracked.
fn draw_the_tracker(
    mut commands: Commands,
    run: Res<RunAchievements>,
    parts: Option<ResMut<Parts>>,
    content: Option<Res<GameContent>>,
    look: MenuLook,
    objects: Objects,
    mut nodes: Query<&mut Node>,
) {
    let (Some(mut parts), Some(content)) = (parts, content) else {
        return;
    };
    // its place follows the window's scale (the parts are made before the window has its size)
    if let Ok(mut node) = nodes.get_mut(parts.tracker) {
        let (top, right, width) = (
            look.px(TRACKER_TOP),
            look.px(TRACKER_RIGHT),
            look.px(TRACKER_WIDTH),
        );
        if node.top != top || node.right != right || node.width != width {
            (node.top, node.right, node.width) = (top, right, width);
        }
    }
    let rows = match (&run.told, in_world(&objects)) {
        (Some(told), true) => super::tracked_rows(&content.0, told),
        _ => Vec::new(),
    };
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for r in &rows {
            (&r.id, super::progress_words(&r.progress)).hash(&mut h);
        }
        look.scale().unit.to_bits().hash(&mut h);
        h.finish()
    };
    if parts.tracker_for == Some(key) {
        return;
    }
    parts.tracker_for = Some(key);
    commands.entity(parts.tracker).despawn_related::<Children>();
    if rows.is_empty() {
        return;
    }
    commands.entity(parts.tracker).with_children(|t| {
        let mut node = look.panel_node();
        node.width = Val::Percent(100.0);
        node.padding = UiRect::axes(look.px(9.0), look.px(7.0));
        node.row_gap = look.px(5.0);
        t.spawn((
            node,
            BackgroundColor(palette::PANEL.with_alpha(0.8)),
            BorderColor::all(palette::GOLD_DIM),
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((look.label("Achievements"), Pickable::IGNORE));
            for r in &rows {
                p.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: look.px(2.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|line| {
                    line.spawn((
                        look.text(Face::Plain, 10.5, palette::HEADING, r.name.clone()),
                        Pickable::IGNORE,
                    ));
                    match super::progress_words(&r.progress) {
                        Some(words) => {
                            let share = match &r.progress {
                                super::RowProgress::Bar { current, goal } => {
                                    *current as f32 / (*goal).max(1) as f32
                                }
                                super::RowProgress::Checklist(items) => {
                                    items.iter().filter(|(_, d)| *d).count() as f32
                                        / items.len().max(1) as f32
                                }
                                super::RowProgress::None => 0.0,
                            };
                            crate::menus::look::progress_bar(line, &look, share, &words, 12.0);
                        }
                        None => {
                            line.spawn((
                                look.text(Face::Plain, 9.0, palette::VALUE, r.text.clone()),
                                Pickable::IGNORE,
                            ));
                        }
                    }
                });
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_banner_rises_holds_fades_and_the_next_waits_its_turn() {
        assert_eq!(banner_phase(-0.1), None);
        let (s, rise) = banner_phase(0.0).unwrap();
        assert_eq!((s, rise), (0.0, BANNER_SLIDE));
        assert_eq!(banner_phase(BANNER_RISE + 1.0), Some((1.0, 0.0)));
        let (s, _) = banner_phase(BANNER_RISE + BANNER_HOLD + BANNER_FADE / 2.0).unwrap();
        assert!((s - 0.5).abs() < 1e-4);
        assert_eq!(
            banner_phase(BANNER_RISE + BANNER_HOLD + BANNER_FADE + 0.01),
            None
        );

        let mut run = RunAchievements::default();
        run.waiting
            .extend(["first_victory".to_string(), "slayer_1".to_string()]);
        assert_eq!(run.next_banner(10.0).as_deref(), Some("first_victory"));
        assert_eq!(run.next_banner(11.0), None, "the first is still up");
        let over = 10.0 + BANNER_RISE + BANNER_HOLD + BANNER_FADE + 0.1;
        assert_eq!(run.next_banner(over).as_deref(), Some("slayer_1"));
        assert_eq!(run.next_banner(over + 20.0), None, "none left");
        assert!(run.showing.is_none());
    }
}
