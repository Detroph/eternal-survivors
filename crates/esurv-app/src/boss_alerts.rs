//! Boss warnings and timer bars (clear raid-warning style announcements
//! for the bosses' mechanics, docs/game/bosses.md "Warnings and timer bars"): a mechanic that
//! starts shows big across the screen in its colour, its icon on each side, with the client's raid
//! warning sound; the current boss's next abilities and timed script lines count down as bars at
//! the left side, and a bar under 5 s moves up to the "soon" bars, bigger, and flashes. The bars
//! count the run's own time (they stand while a choice is open or the run is paused). The
//! server's `BossWarning`, `BossTimer` and `BossTimersCleared` lines feed them. Each part can be
//! turned off in Options (Interface): the bars, the warnings and their sound, all on at first.
//!
//! The look is the stock 1.12 interface's ([`crate::stock`]): its fonts, status bar and icon
//! frames; the words are the zone files'.

use bevy::prelude::*;
use bevy::text::LineBreak;

use benilla_ui::script::UiScript;
use esurv_protocol::{Phase, ToClient};

use crate::boss_bar_places::{BarGroup, BarHandle, BarPlaces};
use crate::hud::RunHud;
use crate::protocol::ServerEvent;
use crate::stock::{Stock, StockFont, StockSystems};
use crate::ui::layer;

/// The Options settings (CVars of our own, registered as an addon's are): the timer bars, the
/// warnings, the warnings' sound, and whether the bars' drag handles always show; "1" on, as
/// they start.
pub const BARS_CVAR: &str = "esurvBossTimerBars";
pub const WARNINGS_CVAR: &str = "esurvBossWarnings";
pub const SOUND_CVAR: &str = "esurvBossWarningSound";
pub const HANDLES_CVAR: &str = "esurvBossBarHandles";
/// The client's raid warning sound (`SoundEntries.dbc` "RaidWarning",
/// `Sound\Interface\RaidWarning.wav`), as the stock raid warning plays it.
pub const RAID_WARNING_SOUND: &str = "RaidWarning";

/// A warning holds this long, then fades over `WARNING_FADE`; at most this many show.
pub const WARNING_HOLD: f32 = 3.0;
pub const WARNING_FADE: f32 = 1.0;
pub const WARNING_SLOTS: usize = 2;
/// A bar with this little left is "soon": it moves up, grows and flashes.
pub const SOON: f32 = 5.0;
/// A bar run out stays this long (an ability waiting its turn), then goes.
pub const BAR_LINGER: f32 = 2.0;
/// A soon bar flashes this many times a second.
const FLASH_RATE: f32 = 3.0;
/// The options are read again this often (seconds).
const OPTIONS_EVERY: f32 = 1.0;

// Where the parts sit, in the stock interface's units: the warnings under the boss bar, above
// the stock centre lines (where the run's own lines show), the bars down the left side under the player frame (clear of the damage meter on the right).
const WARNINGS_TOP: f32 = 128.0;
/// With bosses up the warnings go at least this far under their bars (the stack grows a thin bar
/// for each further boss).
const WARNINGS_BELOW_BOSSES: f32 = 10.0;
const WARNING_FONT: f32 = 20.0;
const WARNING_ICON: f32 = 26.0;
const BARS_LEFT: f32 = 24.0;
const BARS_TOP: f32 = 110.0;
const BAR_WIDTH: f32 = 170.0;
const BAR_HEIGHT: f32 = 16.0;
const SOON_BAR_WIDTH: f32 = 220.0;
const SOON_BAR_HEIGHT: f32 = 22.0;
const BAR_GAP: f32 = 4.0;
const GROUP_GAP: f32 = 14.0;

/// One warning up, on the app's clock.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub text: String,
    pub colour: [f32; 3],
    pub icon: String,
    pub at: f32,
}

/// One timer bar: its boss and key, label, icon, the seconds it ran from and has left.
#[derive(Clone, Debug, PartialEq)]
pub struct TimerBar {
    pub boss: u32,
    pub key: u32,
    pub label: String,
    pub icon: String,
    pub total: f32,
    pub left: f32,
    /// Seconds it has stood run out (it goes after `BAR_LINGER`, unless started again).
    pub idle: f32,
}

/// What the warnings and bars hold now.
#[derive(Resource, Default, Debug)]
pub struct BossAlerts {
    pub warnings: Vec<Shown>,
    pub bars: Vec<TimerBar>,
    /// Changed since the parts were last drawn.
    dirty: bool,
}

impl BossAlerts {
    /// A warning comes, at `now` on the app's clock; the oldest goes past the slots.
    pub fn warn(&mut self, text: &str, colour: [f32; 3], icon: &str, now: f32) {
        self.warnings.push(Shown { text: text.into(), colour, icon: icon.into(), at: now });
        if self.warnings.len() > WARNING_SLOTS {
            self.warnings.remove(0);
        }
        self.dirty = true;
    }

    /// A bar starts, or starts again, with `seconds` left.
    pub fn bar(&mut self, boss: u32, key: u32, label: &str, icon: &str, seconds: f32) {
        self.bars.retain(|b| !(b.boss == boss && b.key == key));
        self.bars.push(TimerBar { boss, key, label: label.into(), icon: icon.into(), total: seconds.max(0.1), left: seconds, idle: 0.0 });
        self.dirty = true;
    }

    /// Boss `boss`'s bars go.
    pub fn clear_boss(&mut self, boss: u32) {
        self.bars.retain(|b| b.boss != boss);
        self.dirty = true;
    }

    pub fn clear(&mut self) {
        self.warnings.clear();
        self.bars.clear();
        self.dirty = true;
    }

    /// `dt` seconds pass: the bars count down while the run runs (a bar run out stays at 0
    /// until it starts again); warnings past their time go.
    pub fn tick(&mut self, dt: f32, running: bool, now: f32) {
        if running {
            // a counter (negative seconds) does not count down and stays
            for b in self.bars.iter_mut().filter(|b| !b.is_counter()) {
                if b.left <= 0.0 {
                    b.idle += dt;
                }
                b.left = (b.left - dt).max(0.0);
            }
            self.bars.retain(|b| b.idle < BAR_LINGER);
            if !self.bars.is_empty() {
                self.dirty = true;
            }
        }
        let before = self.warnings.len();
        self.warnings.retain(|w| now - w.at < WARNING_HOLD + WARNING_FADE);
        self.dirty |= before != self.warnings.len() || !self.warnings.is_empty();
    }

    /// The bars split into the soon ones and the later ones, each soonest first (counters with
    /// the later ones, last).
    pub fn split(&self) -> (Vec<&TimerBar>, Vec<&TimerBar>) {
        let mut all: Vec<&TimerBar> = self.bars.iter().collect();
        let order = |b: &TimerBar| if b.is_counter() { f32::INFINITY } else { b.left };
        all.sort_by(|a, b| order(a).total_cmp(&order(b)));
        all.into_iter().partition(|b| !b.is_counter() && b.left <= SOON)
    }
}

impl TimerBar {
    /// A counter ("Guards left 5/8"): told with negative seconds, it does not count down.
    pub fn is_counter(&self) -> bool {
        self.left < 0.0
    }

    /// How full it draws: a counter whole.
    pub fn fill(&self) -> f32 {
        if self.is_counter() {
            1.0
        } else {
            self.left / self.total
        }
    }
}

/// How strong warning `w` shows at `now`: whole while it holds, then fading.
pub fn warning_strength(w: &Shown, now: f32) -> f32 {
    let age = now - w.at;
    if age <= WARNING_HOLD {
        1.0
    } else {
        (1.0 - (age - WARNING_HOLD) / WARNING_FADE).clamp(0.0, 1.0)
    }
}

/// A bar's time left as it reads: tenths under 10 s, whole seconds above.
pub fn time_text(left: f32) -> String {
    if left < 10.0 {
        format!("{left:.1}")
    } else {
        format!("{}", left.ceil() as u32)
    }
}

/// Which parts the player wants (Options).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlertOptions {
    pub bars: bool,
    pub warnings: bool,
    pub sound: bool,
    /// The bars' drag handles always show (off: only while Shift is held).
    pub handles: bool,
}

impl Default for AlertOptions {
    fn default() -> Self {
        Self { bars: true, warnings: true, sound: true, handles: true }
    }
}

impl AlertOptions {
    /// The options from the CVars' values (`None`: not set, on).
    pub fn from_values(bars: Option<&str>, warnings: Option<&str>, sound: Option<&str>, handles: Option<&str>) -> Self {
        let on = |v: Option<&str>| v.is_none_or(|v| v.trim().parse::<f32>().map_or(true, |n| n != 0.0));
        Self { bars: on(bars), warnings: on(warnings), sound: on(warnings) && on(sound), handles: on(handles) }
    }
}

/// The CVars registered, as an addon's `RegisterCVar` does (a no-op once they are), so the
/// Options panel and the game read and keep them.
pub fn register_cvars(script: &UiScript) {
    for cvar in [BARS_CVAR, WARNINGS_CVAR, SOUND_CVAR, HANDLES_CVAR] {
        let _ = script.eval::<()>(&format!("RegisterCVar(\"{cvar}\", \"1\")"));
    }
    let _ = script.eval::<()>(&format!("RegisterCVar(\"{}\", \"0\")", crate::boss_bar_places::RESET_CVAR));
}

#[derive(Component)]
struct AlertsRoot;

/// The parts' roots, and the scale they were built at.
#[derive(Resource)]
struct AlertParts {
    scale: crate::stock::StockScale,
    warnings: Entity,
    /// The two groups' roots, and where each one's bars go.
    roots: [Entity; 2],
    soon: Entity,
    later: Entity,
}

pub(crate) fn register(app: &mut App) {
    crate::boss_bar_places::register(app);
    app.init_resource::<BossAlerts>()
        .init_resource::<AlertOptions>()
        .add_systems(Update, (build_parts, read_alerts, read_options, count_down, place_groups, place_warnings, draw).chain().before(StockSystems));
}

/// Where the warnings go with `bosses` bosses up (stock units down): under the bosses' bars.
pub fn warnings_top(bosses: usize) -> f32 {
    if bosses == 0 {
        return WARNINGS_TOP;
    }
    WARNINGS_TOP.max(crate::hud::boss_stack::stack_bottom(bosses) + WARNINGS_BELOW_BOSSES)
}

/// The warnings move down as the bosses' bars grow, so nothing overlaps.
fn place_warnings(stock: Stock, run: Res<RunHud>, parts: Option<Res<AlertParts>>, mut nodes: Query<&mut Node>) {
    let Some(parts) = parts else { return };
    let Ok(mut node) = nodes.get_mut(parts.warnings) else { return };
    let top = stock.px(warnings_top(run.bosses.len()));
    if node.top != top {
        node.top = top;
    }
}

/// The later bars start this far (stock units) under the soon bars: room for four of them.
const LATER_BELOW: f32 = 4.0 * (SOON_BAR_HEIGHT + BAR_GAP) + GROUP_GAP;

/// Each group where the player put it, else where it starts.
fn place_groups(stock: Stock, places: Res<BarPlaces>, mut roots: Query<(&BarGroup, &mut Node), With<AlertsRoot>>) {
    for (g, mut node) in &mut roots {
        let (left, top) = match (places.of(*g), g) {
            (Some((x, y)), _) => (Val::Px(x), Val::Px(y)),
            (None, BarGroup::Soon) => (stock.px(BARS_LEFT), stock.px(BARS_TOP)),
            (None, BarGroup::Later) => (stock.px(BARS_LEFT), stock.px(BARS_TOP + LATER_BELOW)),
        };
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
    }
}

fn build_parts(mut commands: Commands, stock: Stock, parts: Option<Res<AlertParts>>, mut alerts: ResMut<BossAlerts>) {
    if parts.as_ref().is_some_and(|p| p.scale == stock.scale()) {
        return;
    }
    if let Some(p) = &parts {
        commands.entity(p.warnings).despawn();
        for root in p.roots {
            commands.entity(root).despawn();
        }
    }
    let u = |v: f32| stock.px(v);
    let warnings = commands
        .spawn((
            AlertsRoot,
            Name::new("boss warnings"),
            Node {
                position_type: PositionType::Absolute,
                top: u(WARNINGS_TOP),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: u(4.0),
                ..default()
            },
            GlobalZIndex(layer::HUD),
            Pickable::IGNORE,
        ))
        .id();
    // the two groups of bars, each with its drag handle (shown while Shift is held) over its bars
    let mut group = |g: BarGroup, title: &str| {
        let mut content = Entity::PLACEHOLDER;
        let root = commands
            .spawn((
                AlertsRoot,
                g,
                Name::new(format!("boss timer bars: {title}")),
                Node { position_type: PositionType::Absolute, flex_direction: FlexDirection::Column, align_items: AlignItems::FlexStart, row_gap: u(2.0), ..default() },
                GlobalZIndex(layer::HUD),
                Pickable::IGNORE,
            ))
            .with_children(|c| {
                c.spawn((
                    BarHandle(g),
                    Button,
                    Interaction::default(),
                    // the world, the camera and the stock interface leave a press on the handle
                    // to it: without this a press started a camera look, which hid and locked
                    // the cursor, so the group could not follow it
                    benilla_app::OverlayWindow,
                    Node { display: Display::None, width: u(BAR_WIDTH), padding: UiRect::axes(u(4.0), u(1.0)), ..default() },
                    BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.85)),
                    BorderColor::all(Color::srgb(1.0, 0.82, 0.0)),
                ))
                .with_children(|h| {
                    h.spawn((stock.text(StockFont::GameFontNormalSmall, format!("{title} (drag)")), Pickable::IGNORE));
                });
                content = c.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexStart, row_gap: u(BAR_GAP), ..default() }, Pickable::IGNORE)).id();
            })
            .id();
        (root, content)
    };
    let (soon_root, soon) = group(BarGroup::Soon, "Boss bars: soon");
    let (later_root, later) = group(BarGroup::Later, "Boss bars");
    commands.insert_resource(AlertParts { scale: stock.scale(), warnings, roots: [soon_root, later_root], soon, later });
    alerts.dirty = true;
}

/// The server's warnings and bars; a new run forgets them all, and so does leaving the world
/// (`count_down`).
fn read_alerts(mut events: MessageReader<ServerEvent>, mut alerts: ResMut<BossAlerts>, options: Res<AlertOptions>, time: Res<Time>, script: Option<NonSend<UiScript>>) {
    let now = time.elapsed_secs();
    for ServerEvent(m) in events.read() {
        match m {
            ToClient::Ready(_) => alerts.clear(),
            ToClient::BossWarning(w) => {
                if options.warnings {
                    alerts.warn(&w.text, w.colour, &w.icon, now);
                    if options.sound {
                        if let Some(script) = &script {
                            let _ = script.eval::<()>(&format!("PlaySound(\"{RAID_WARNING_SOUND}\")"));
                        }
                    }
                }
            }
            ToClient::BossTimer(t) => alerts.bar(t.boss, t.key, &t.label, &t.icon, t.seconds),
            ToClient::BossTimersCleared { boss } => alerts.clear_boss(*boss),
            _ => {}
        }
    }
}

/// The options, read from their CVars now and then.
#[allow(clippy::too_many_arguments)]
fn read_options(
    script: Option<NonSend<UiScript>>,
    mut options: ResMut<AlertOptions>,
    mut places: ResMut<BarPlaces>,
    file: Res<crate::boss_bar_places::BarPlacesFile>,
    time: Res<Time>,
    mut next: Local<f32>,
    mut registered: Local<bool>,
) {
    let Some(script) = script else { return };
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + OPTIONS_EVERY;
    if !*registered {
        register_cvars(&script);
        *registered = true;
    }
    let get = |cvar: &str| script.eval::<Option<String>>(&format!("return GetCVar(\"{cvar}\")")).ok().flatten();
    let read = AlertOptions::from_values(get(BARS_CVAR).as_deref(), get(WARNINGS_CVAR).as_deref(), get(SOUND_CVAR).as_deref(), get(HANDLES_CVAR).as_deref());
    options.set_if_neq(read);
    // "Reset boss bar positions" ticked: both groups back where they start, and the box unticks
    if get(crate::boss_bar_places::RESET_CVAR).is_some_and(|v| v.trim() == "1") {
        crate::boss_bar_places::reset_places(&mut places, &file);
        let _ = script.eval::<()>(&format!("SetCVar(\"{}\", \"0\")", crate::boss_bar_places::RESET_CVAR));
    }
}

fn count_down(mut alerts: ResMut<BossAlerts>, run: Res<RunHud>, time: Res<Time>) {
    // the hero left the world (a run ended mid-fight): its warnings and bars go with it
    if run.ready.is_none() {
        if !alerts.warnings.is_empty() || !alerts.bars.is_empty() {
            alerts.clear();
        }
        return;
    }
    let running = run.phase() == Some(Phase::Running);
    alerts.tick(time.delta_secs(), running, time.elapsed_secs());
}

/// Draws the warnings and bars again when they changed (the bars as they count).
fn draw(mut commands: Commands, stock: Stock, fonts: Res<crate::ui::UiFonts>, parts: Option<Res<AlertParts>>, mut alerts: ResMut<BossAlerts>, options: Res<AlertOptions>, time: Res<Time>, mut drawn: Local<f32>) {
    let Some(parts) = parts else { return };
    let now = time.elapsed_secs();
    // at most 20 times a second
    if !(alerts.dirty || options.is_changed()) || now - *drawn < 0.05 {
        return;
    }
    *drawn = now;
    alerts.dirty = false;
    commands.entity(parts.warnings).despawn_related::<Children>();
    commands.entity(parts.soon).despawn_related::<Children>();
    commands.entity(parts.later).despawn_related::<Children>();
    let u = |v: f32| stock.px(v);
    if options.warnings {
        for w in &alerts.warnings {
            let a = warning_strength(w, now);
            let colour = Color::srgba(w.colour[0], w.colour[1], w.colour[2], a);
            let icon = |c: &mut ChildSpawnerCommands| {
                if !w.icon.is_empty() {
                    c.spawn(stock.icon(&w.icon, WARNING_ICON));
                }
            };
            commands.entity(parts.warnings).with_children(|c| {
                c.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: u(8.0), ..default() }).with_children(|row| {
                    icon(row);
                    let mut text = row.spawn((stock.coloured_text(StockFont::GameFontNormalLarge, colour, w.text.clone()), TextLayout::new(Justify::Center, LineBreak::NoWrap)));
                    text.insert(TextFont { font: fonts.title.clone(), font_size: stock.scale().of(WARNING_FONT), ..default() });
                    icon(row);
                });
            });
        }
    }
    if options.bars {
        let (soon, later) = alerts.split();
        let flash = 0.55 + 0.45 * (now * FLASH_RATE * std::f32::consts::TAU).sin().abs();
        let row = |c: &mut ChildSpawnerCommands, b: &TimerBar, big: bool| {
            let (w, h) = if big { (SOON_BAR_WIDTH, SOON_BAR_HEIGHT) } else { (BAR_WIDTH, BAR_HEIGHT) };
            let colour = if big { Color::srgb(1.0, 0.2 * flash, 0.1) } else { Color::srgb(0.85, 0.55, 0.1) };
            c.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: u(3.0), ..default() }).with_children(|r| {
                if !b.icon.is_empty() {
                    r.spawn(stock.icon(&b.icon, h));
                }
                r.spawn(stock.status_bar(colour, b.fill(), w, h)).with_children(|bar| {
                    bar.spawn(Node {
                        position_type: PositionType::Absolute,
                        left: u(4.0),
                        right: u(4.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|t| {
                        let font = if big { StockFont::GameFontHighlight } else { StockFont::GameFontHighlightSmall };
                        t.spawn((stock.text(font, b.label.clone()), TextLayout::new(Justify::Left, LineBreak::NoWrap)));
                        t.spawn((stock.text(StockFont::NumberFontNormalSmall, if b.is_counter() { String::new() } else { time_text(b.left) }), TextLayout::new(Justify::Right, LineBreak::NoWrap)));
                    });
                });
            });
        };
        commands.entity(parts.soon).with_children(|c| {
            for b in &soon {
                row(c, b, true);
            }
        });
        commands.entity(parts.later).with_children(|c| {
            for b in &later {
                row(c, b, false);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bar_counts_down_only_while_the_run_runs_and_moves_up_under_five_seconds() {
        let mut a = BossAlerts::default();
        a.bar(7, 0, "Next hit", "", 9.0);
        a.bar(7, 1, "Charge", "", 20.0);
        a.tick(3.0, true, 0.0);
        a.tick(10.0, false, 0.0);
        assert!((a.bars[0].left - 6.0).abs() < 1e-5, "it stood while the run did not run");
        assert!(a.split().0.is_empty());
        a.tick(1.5, true, 0.0);
        let (soon, later) = a.split();
        assert_eq!(soon.iter().map(|b| b.label.as_str()).collect::<Vec<_>>(), ["Next hit"], "under 5 s: soon");
        assert_eq!(later.len(), 1);
        // started again: back to the later bars
        a.bar(7, 0, "Next hit", "", 9.0);
        assert!(a.split().0.is_empty());
        assert_eq!(a.bars.len(), 2, "one bar per key");
        // one run out goes a little later
        a.tick(20.0, true, 0.0);
        a.tick(BAR_LINGER, true, 0.0);
        assert!(a.bars.is_empty(), "run out: gone");
        a.bar(7, 0, "Next hit", "", 9.0);
        a.clear_boss(7);
        assert!(a.bars.is_empty());
    }

    /// A counter ("Guards left 5/8", told with negative seconds) stands full, never runs out,
    /// stays with the later bars and is replaced when told again.
    #[test]
    fn a_counter_stands_until_told_again_or_cleared() {
        let mut a = BossAlerts::default();
        a.bar(7, 91, "Guards left 8/8", "", -1.0);
        a.bar(7, 0, "Next hit", "", 3.0);
        a.tick(60.0, true, 0.0);
        a.tick(BAR_LINGER, true, 0.0);
        assert_eq!(a.bars.len(), 1, "the timer ran out and went, the counter stays");
        assert!(a.bars[0].is_counter() && a.bars[0].fill() == 1.0);
        assert!(a.split().0.is_empty());
        a.bar(7, 91, "Guards left 5/8", "", -1.0);
        assert_eq!(a.bars.iter().map(|b| b.label.as_str()).collect::<Vec<_>>(), ["Guards left 5/8"]);
        a.clear_boss(7);
        assert!(a.bars.is_empty());
    }

    #[test]
    fn a_warning_holds_then_fades_and_the_oldest_goes_past_the_slots() {
        let mut a = BossAlerts::default();
        a.warn("one", [1.0; 3], "", 0.0);
        let w = a.warnings[0].clone();
        assert_eq!(warning_strength(&w, 2.0), 1.0);
        assert!((warning_strength(&w, WARNING_HOLD + 0.5) - 0.5).abs() < 1e-5);
        a.warn("two", [1.0; 3], "", 1.0);
        a.warn("three", [1.0; 3], "", 1.0);
        assert_eq!(a.warnings.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), ["two", "three"]);
        a.tick(0.0, true, 10.0);
        assert!(a.warnings.is_empty());
        assert_eq!(time_text(4.25), "4.2");
        assert_eq!(time_text(12.2), "13");
    }

    #[test]
    fn the_options_start_on_and_the_sound_follows_the_warnings() {
        assert_eq!(AlertOptions::from_values(None, None, None, None), AlertOptions::default());
        let off = AlertOptions::from_values(Some("0"), Some("1"), Some("0"), Some("0"));
        assert_eq!(off, AlertOptions { bars: false, warnings: true, sound: false, handles: false });
        let no_warnings = AlertOptions::from_values(Some("1"), Some("0"), Some("1"), None);
        assert!(!no_warnings.warnings && !no_warnings.sound, "no warnings: no sound");
    }

    #[test]
    fn the_warnings_stay_under_the_bosses_bars() {
        assert_eq!(warnings_top(0), WARNINGS_TOP);
        for n in 1..=4 {
            assert!(warnings_top(n) >= crate::hud::boss_stack::stack_bottom(n) + WARNINGS_BELOW_BOSSES, "{n} bosses");
        }
        assert!(warnings_top(2) > warnings_top(1), "a second boss pushes them down");
    }
}
