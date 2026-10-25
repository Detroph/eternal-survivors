//! The bosses' health bars at the top centre: the first boss up on a wide bar (its skull, name,
//! health and share, a tick at each health phase of its script with the phase's name over it),
//! its cast or wind-up on a casting bar under it, a clear gap apart; every other living boss on a
//! thinner bar of the same look below. A bar goes as its boss dies. The server's `BossBar` and
//! `BossCast` feed it.
//!
//! The look is the stock 1.12 interface's: the skill bar's border (`UI-Character-Skills-BarBorder`)
//! round the health bars, the casting bar's border and colour round the cast, the target frame's
//! boss skull, the stock fonts.

use bevy::prelude::*;
use bevy::text::LineBreak;
use esurv_protocol::{BossBar, Phase, ToClient};

use crate::protocol::ServerEvent;
use crate::stock::{load_icon, load_stock_art, Stock, StockFont};
use crate::ui::{GOLD_TEXT, WHITE_TEXT};

use super::state::{health_fraction, health_text, RunHud};
use super::view::HudParts;

// The stack's layout, in stock units down from the top of the screen.
/// Where the stack starts: the phase names' line, then the first boss's bar.
pub const STACK_TOP: f32 = 62.0;
const LABEL_HEIGHT: f32 = 11.0;
const MAIN_WIDTH: f32 = 370.0;
const MAIN_HEIGHT: f32 = 18.0;
/// The gap between the first boss's bar and its cast.
const CAST_GAP: f32 = 16.0;
const CAST_WIDTH: f32 = 200.0;
const CAST_HEIGHT: f32 = 11.0;
/// The other bosses' bars: under the cast's room, each this tall, this far apart.
const OTHERS_GAP: f32 = 12.0;
const OTHER_WIDTH: f32 = 310.0;
const OTHER_HEIGHT: f32 = 13.0;
const OTHER_ROW_GAP: f32 = 6.0;
/// A phase tick's width.
const TICK_WIDTH: f32 = 1.5;
/// A phase name's room across, centred on its tick.
const LABEL_WIDTH: f32 = 120.0;

/// The skill bar's border: 256 by 32, its bar inside from 5 to 251 across and 8 to 24 down.
const SKILL_BORDER: &str = r"Interface\PaperDollInfoFrame\UI-Character-Skills-BarBorder";
const SKILL_BORDER_SIZE: (f32, f32) = (256.0, 32.0);
const SKILL_BORDER_INSIDE: (f32, f32, f32, f32) = (5.0, 8.0, 246.0, 16.0);
/// The casting bar's border: 256 by 64, its 195 by 13 bar 28 down.
const CAST_BORDER: &str = r"Interface\CastingBar\UI-CastingBar-Border";
const CAST_BORDER_SIZE: (f32, f32) = (256.0, 64.0);
const CAST_BORDER_INSIDE: (f32, f32, f32, f32) = (30.5, 28.0, 195.0, 13.0);
/// The target frame's boss skull (a level too high to read).
const BOSS_SKULL: &str = r"Interface\TargetingFrame\UI-TargetingFrame-Skull";

/// The bosses' health: a dark red, as a hostile unit's bar reads against the green of the
/// player's own.
const BOSS_RED: Color = Color::srgb(0.8, 0.08, 0.08);
/// The stock casting bar's colour (`CastingBarFrame`, 1.0, 0.7, 0.0).
const CAST_YELLOW: Color = Color::srgb(1.0, 0.7, 0.0);
const TICK_GOLD: Color = Color::srgb(1.0, 0.86, 0.35);

/// How far down the stack reaches with `bosses` bosses up (stock units): the boss warnings go
/// under it. The cast's room is kept whether the first boss casts or not, so nothing jumps.
pub fn stack_bottom(bosses: usize) -> f32 {
    if bosses == 0 {
        return STACK_TOP;
    }
    let first = STACK_TOP + LABEL_HEIGHT + MAIN_HEIGHT + CAST_GAP + CAST_HEIGHT;
    let others = bosses - 1;
    if others == 0 {
        return first;
    }
    first + OTHERS_GAP + others as f32 * OTHER_HEIGHT + (others - 1) as f32 * OTHER_ROW_GAP
}

/// A boss's health as a share, `58%` (rounded down, so a living boss never reads 0 % or a hurt
/// one 100 %).
pub fn percent_text(health: u32, max_health: u32) -> String {
    if max_health == 0 {
        return "0%".into();
    }
    let pct = (health as u64 * 100 / max_health as u64) as u32;
    let pct = if health > 0 { pct.max(1) } else { 0 };
    format!("{pct}%")
}

/// One boss's cast as the bar shows it: what, its icon, how long in all and how long is left.
#[derive(Clone, Debug, PartialEq)]
pub struct Cast {
    pub boss: u64,
    pub label: String,
    pub icon: String,
    pub total: f32,
    pub left: f32,
}

impl Cast {
    /// How far along it is, 0 to 1.
    pub fn done(&self) -> f32 {
        if self.total <= 0.0 {
            return 1.0;
        }
        (1.0 - self.left / self.total).clamp(0.0, 1.0)
    }
}

/// The bosses' casts under way.
#[derive(Resource, Default, Debug)]
pub struct BossCasts {
    pub casts: Vec<Cast>,
    dirty: bool,
}

impl BossCasts {
    /// Takes in a server message: a cast starts (or stops, at 0 s); a boss gone takes its cast
    /// with it; a new run forgets them all.
    pub fn apply(&mut self, message: &ToClient) {
        match message {
            ToClient::Ready(_) => self.clear(),
            ToClient::BossCast(c) => {
                self.casts.retain(|x| x.boss != c.boss);
                if c.seconds > 0.0 {
                    self.casts.push(Cast { boss: c.boss, label: c.label.clone(), icon: c.icon.clone(), total: c.seconds, left: c.seconds });
                }
                self.dirty = true;
            }
            ToClient::BossBar(BossBar { boss, health: 0, .. }) => {
                let before = self.casts.len();
                self.casts.retain(|x| x.boss != *boss);
                self.dirty |= self.casts.len() != before;
            }
            _ => {}
        }
    }

    /// `dt` seconds of the run pass (the casts stand while it is paused or a choice is open); a
    /// cast done goes.
    pub fn tick(&mut self, dt: f32, running: bool) {
        if !running || self.casts.is_empty() {
            return;
        }
        for c in &mut self.casts {
            c.left -= dt;
        }
        self.casts.retain(|c| c.left > 0.0);
        self.dirty = true;
    }

    /// Boss `boss`'s cast, if it is casting.
    pub fn of(&self, boss: u64) -> Option<&Cast> {
        self.casts.iter().find(|c| c.boss == boss)
    }

    pub fn clear(&mut self) {
        self.casts.clear();
        self.dirty = true;
    }
}

/// The casts from the server's messages.
pub(super) fn read_casts(mut events: MessageReader<ServerEvent>, mut casts: ResMut<BossCasts>) {
    for ServerEvent(m) in events.read() {
        casts.apply(m);
    }
}

/// The casts count down with the run.
pub(super) fn count_casts(mut casts: ResMut<BossCasts>, run: Res<RunHud>, time: Res<Time>) {
    let running = run.phase() == Some(Phase::Running);
    casts.tick(time.delta_secs(), running);
}

/// Draws the stack again when the bosses or their casts changed (at most 20 times a second).
#[allow(clippy::too_many_arguments)] // one Bevy system's inputs
pub(super) fn draw_stack(
    mut commands: Commands,
    stock: Stock,
    assets: Res<AssetServer>,
    run: Res<RunHud>,
    parts: Option<Res<HudParts>>,
    mut casts: ResMut<BossCasts>,
    time: Res<Time>,
    mut drawn: Local<f32>,
) {
    let Some(parts) = parts else { return };
    let now = time.elapsed_secs();
    let changed = run.is_changed() || parts.is_changed();
    if !(changed || (casts.dirty && now - *drawn >= 0.05)) {
        return;
    }
    *drawn = now;
    casts.dirty = false;
    commands.entity(parts.boss).despawn_related::<Children>();
    if run.bosses.is_empty() {
        return;
    }
    let u = |v: f32| stock.px(v);
    let one_line = |justify| TextLayout::new(justify, LineBreak::NoWrap);
    let skull = load_stock_art(&assets, BOSS_SKULL);
    let skill_border = load_stock_art(&assets, SKILL_BORDER);
    let cast_border = load_stock_art(&assets, CAST_BORDER);
    // a border image round a bar `w` by `h`: the texture's inside laid over the bar
    let border = |image: Handle<Image>, size: (f32, f32), inside: (f32, f32, f32, f32), w: f32, h: f32| {
        let (sx, sy) = (w / inside.2, h / inside.3);
        (
            Node {
                position_type: PositionType::Absolute,
                left: u(-inside.0 * sx),
                top: u(-inside.1 * sy),
                width: u(size.0 * sx),
                height: u(size.1 * sy),
                ..default()
            },
            ImageNode { image, image_mode: NodeImageMode::Stretch, ..default() },
            Pickable::IGNORE,
        )
    };
    // a health bar: skull, name, health in the middle, share at the right, the phase ticks
    let health_bar = |c: &mut ChildSpawnerCommands, b: &BossBar, w: f32, h: f32, name_font: StockFont| {
        c.spawn(stock.status_bar(BOSS_RED, health_fraction(b.health, b.max_health), w, h)).with_children(|bar| {
            for p in &b.phases {
                bar.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(p.at.clamp(0.0, 100.0)),
                        top: u(-2.0),
                        bottom: u(-2.0),
                        width: u(TICK_WIDTH),
                        margin: UiRect::left(u(-TICK_WIDTH / 2.0)),
                        ..default()
                    },
                    BackgroundColor(TICK_GOLD),
                ));
            }
            bar.spawn(border(skill_border.clone(), SKILL_BORDER_SIZE, SKILL_BORDER_INSIDE, w, h));
            bar.spawn((
                Node { position_type: PositionType::Absolute, left: u(1.5), top: u(1.0), width: u(h - 2.0), height: u(h - 2.0), ..default() },
                ImageNode { image: skull.clone(), image_mode: NodeImageMode::Stretch, ..default() },
            ));
            bar.spawn(Node {
                position_type: PositionType::Absolute,
                left: u(h + 2.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                ..default()
            })
                .with_children(|t| {
                    t.spawn((stock.coloured_text(name_font, WHITE_TEXT, b.name.clone()), one_line(Justify::Left)));
                });
            // health and share together at the right, clear of the half-health tick in the middle
            bar.spawn(Node {
                position_type: PositionType::Absolute,
                right: u(5.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                column_gap: u(8.0),
                ..default()
            })
            .with_children(|t| {
                t.spawn((stock.text(StockFont::NumberFontNormalSmall, health_text(b.health, b.max_health)), one_line(Justify::Right)));
                t.spawn((stock.coloured_text(name_font, WHITE_TEXT, percent_text(b.health, b.max_health)), one_line(Justify::Right)));
            });
        });
    };

    let first = &run.bosses[0];
    commands.entity(parts.boss).with_children(|stack| {
        // the phase names over their ticks, then the first boss's bar
        stack
            .spawn(Node { width: u(MAIN_WIDTH), height: u(LABEL_HEIGHT), ..default() })
            .with_children(|labels| {
                for p in first.phases.iter().filter(|p| !p.label.is_empty()) {
                    labels
                        .spawn(Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(p.at.clamp(0.0, 100.0)),
                            margin: UiRect::left(u(-LABEL_WIDTH / 2.0)),
                            width: u(LABEL_WIDTH),
                            bottom: u(1.0),
                            justify_content: JustifyContent::Center,
                            ..default()
                        })
                        .with_children(|t| {
                            t.spawn((stock.coloured_text(StockFont::GameFontNormalSmall, GOLD_TEXT, p.label.clone()), one_line(Justify::Center)));
                        });
                }
            });
        health_bar(stack, first, MAIN_WIDTH, MAIN_HEIGHT, StockFont::GameFontHighlight);
        // its cast, a clear gap under it; the room is kept while it does not cast
        let cast = casts.of(first.boss);
        stack
            .spawn((
                Node { margin: UiRect::top(u(CAST_GAP)), width: u(CAST_WIDTH), height: u(CAST_HEIGHT), ..default() },
                if cast.is_some() { Visibility::Inherited } else { Visibility::Hidden },
            ))
            .with_children(|row| {
                let Some(cast) = cast else { return };
                row.spawn(stock.status_bar(CAST_YELLOW, cast.done(), CAST_WIDTH, CAST_HEIGHT)).with_children(|bar| {
                    bar.spawn(border(cast_border.clone(), CAST_BORDER_SIZE, CAST_BORDER_INSIDE, CAST_WIDTH, CAST_HEIGHT));
                    if !cast.icon.is_empty() {
                        bar.spawn((
                            Node { position_type: PositionType::Absolute, left: u(-CAST_HEIGHT - 6.0), top: u(-1.0), width: u(CAST_HEIGHT + 2.0), height: u(CAST_HEIGHT + 2.0), ..default() },
                            ImageNode { image: load_icon(&assets, &cast.icon), image_mode: NodeImageMode::Stretch, ..default() },
                        ));
                    }
                    bar.spawn(Node { position_type: PositionType::Absolute, left: u(4.0), top: Val::Px(0.0), bottom: Val::Px(0.0), align_items: AlignItems::Center, ..default() })
                        .with_children(|t| {
                            t.spawn((stock.coloured_text(StockFont::GameFontHighlightSmall, WHITE_TEXT, cast.label.clone()), one_line(Justify::Left)));
                        });
                    bar.spawn(Node { position_type: PositionType::Absolute, right: u(4.0), top: Val::Px(0.0), bottom: Val::Px(0.0), align_items: AlignItems::Center, ..default() })
                        .with_children(|t| {
                            let left = crate::boss_alerts::time_text(cast.left.max(0.0));
                            t.spawn((stock.coloured_text(StockFont::GameFontHighlightSmall, WHITE_TEXT, left), one_line(Justify::Right)));
                        });
                });
            });
        // every other boss up, thinner, below
        for (i, b) in run.bosses.iter().enumerate().skip(1) {
            let gap = if i == 1 { OTHERS_GAP } else { OTHER_ROW_GAP };
            stack.spawn(Node { margin: UiRect::top(u(gap)), ..default() }).with_children(|row| {
                health_bar(row, b, OTHER_WIDTH, OTHER_HEIGHT, StockFont::GameFontHighlightSmall);
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_protocol::BossCast;

    fn cast(boss: u64, seconds: f32) -> ToClient {
        ToClient::BossCast(BossCast { boss, label: "Head Butt".into(), icon: "Ability_Hunter_Pet_Hyena".into(), seconds })
    }

    #[test]
    fn a_cast_counts_down_with_the_run_and_goes_when_done() {
        let mut c = BossCasts::default();
        c.apply(&cast(7, 2.0));
        c.tick(0.5, true);
        let now = c.of(7).unwrap();
        assert!((now.left - 1.5).abs() < 1e-6 && (now.done() - 0.25).abs() < 1e-6);
        c.tick(5.0, false);
        assert!(c.of(7).is_some(), "it stands while the run is paused");
        c.tick(1.6, true);
        assert!(c.of(7).is_none(), "done, it goes");
    }

    #[test]
    fn a_new_cast_replaces_the_old_and_a_stop_or_death_ends_it() {
        let mut c = BossCasts::default();
        c.apply(&cast(7, 2.0));
        c.apply(&cast(8, 3.0));
        c.apply(&cast(7, 1.0));
        assert_eq!(c.casts.len(), 2);
        assert_eq!(c.of(7).unwrap().total, 1.0);
        c.apply(&cast(7, 0.0));
        assert!(c.of(7).is_none(), "0 s stops it");
        c.apply(&ToClient::BossBar(BossBar { boss: 8, name: "Mother Fang".into(), health: 0, max_health: 10, phases: vec![] }));
        assert!(c.casts.is_empty(), "its boss died");
    }

    #[test]
    fn the_share_never_reads_0_while_alive_nor_100_once_hurt() {
        assert_eq!(percent_text(3743, 6409), "58%");
        assert_eq!(percent_text(1, 100_000), "1%");
        assert_eq!(percent_text(99_999, 100_000), "99%");
        assert_eq!(percent_text(100, 100), "100%");
        assert_eq!(percent_text(0, 100), "0%");
        assert_eq!(percent_text(5, 0), "0%");
    }

    #[test]
    fn the_stack_grows_by_a_thin_bar_per_further_boss() {
        assert_eq!(stack_bottom(0), STACK_TOP);
        let one = stack_bottom(1);
        assert!(one > STACK_TOP + MAIN_HEIGHT + CAST_GAP, "room for the cast under the first");
        assert_eq!(stack_bottom(2), one + OTHERS_GAP + OTHER_HEIGHT);
        assert_eq!(stack_bottom(3), stack_bottom(2) + OTHER_ROW_GAP + OTHER_HEIGHT);
    }
}
