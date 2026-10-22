//! The menus' dev aids, off by default:
//!
//! * `ESURV_MENUS_DEMO=1`: a sample of the look's pieces in place of the first screen.
//! * `ESURV_MENUS_SCRIPT=<step>;<step>...`: presses the menus' buttons inside the game, as a
//!   click does ([`super::MenuPress`]), with no input of the operating system; one step after the
//!   other, each once the last has had [`STEP_PAUSE`] seconds to show. A step is `wait:<seconds>`,
//!   `ready` (the select screen up, its camp loaded, a screen of the menus shown), `shot:<name>`
//!   (the game's own shot of the window, `<name>.png` in `ESURV_MENUS_SHOTS`, a folder;
//!   the window is never raised nor pinned), `quit` (closes the game), or a press
//!   ([`super::script_action`]). Over a run: `over-run` waits until the options panel is open over
//!   the paused run (`ESURV_MENU_PRESS=options@<seconds>` opens it), and `escape` presses
//!   Escape inside the game (a key event of the game's own, as `ESURV_MENU_PRESS` sends it),
//!   which the stock key binding window or the run's Escape menu answers.

use bevy::app::AppExit;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use benilla_app::SelectScreen;

use super::look::{palette, Arrow, ButtonStyle, Face, MenuLook};
use super::{MenuAction, MenuPress, Menus, Screen};
use bevy::window::PrimaryWindow;

/// Seconds a step's result shows before the next step.
const STEP_PAUSE: f32 = 1.5;

/// Whether the sample of the look is asked for.
pub(super) fn sample_wanted() -> bool {
    std::env::var("ESURV_MENUS_DEMO").is_ok_and(|v| v == "1")
}

/// One step of a script.
#[derive(Clone, Debug, PartialEq)]
enum Step {
    Wait(f32),
    Ready,
    Shot(String),
    Size(f32, f32),
    Quit,
    Press(MenuAction),
    OverRun,
    Escape,
    /// Shows the tooltip of the warband screen's icon that presses this as the pointer would: a
    /// Starting spell option (`tip-start:<id>`) or a Weapon loadout (`tip-loadout:<id>`);
    /// `tip-off` takes it away.
    Tip(Option<MenuAction>),
    /// Two presses of a piece of the collection log in one frame, as a double click
    /// (`double-piece:<slot>:<zone>[:<look>]`).
    Double(MenuAction),
    /// A made-up end panel over the run (`end-panel:victory` or `end-panel:defeat`).
    EndPanel(esurv_protocol::Ending),
}

/// Reads a script; a step it cannot read is left out, and said.
fn parse(script: &str) -> Vec<Step> {
    script
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| {
            let step = match s.split_once(':') {
                Some(("wait", t)) => t.trim().parse().ok().map(Step::Wait),
                Some(("shot", name)) => Some(Step::Shot(name.trim().into())),
                Some(("tip-start", id)) => Some(Step::Tip(Some(MenuAction::PickStart(id.trim().into())))),
                Some(("tip-loadout", id)) => {
                    Some(Step::Tip(Some(MenuAction::PickLoadout(id.trim().into()))))
                }
                Some(("double-piece", p)) => super::script_action(&format!("piece:{p}")).map(Step::Double),
                Some(("end-panel", "victory")) => Some(Step::EndPanel(esurv_protocol::Ending::Victory)),
                Some(("end-panel", "defeat")) => Some(Step::EndPanel(esurv_protocol::Ending::Fell)),
                Some(("size", wh)) => wh.split_once('x').and_then(|(w, h)| {
                    Some(Step::Size(w.trim().parse().ok()?, h.trim().parse().ok()?))
                }),
                None if s == "ready" => Some(Step::Ready),
                None if s == "quit" => Some(Step::Quit),
                None if s == "over-run" => Some(Step::OverRun),
                None if s == "escape" => Some(Step::Escape),
                None if s == "tip-off" => Some(Step::Tip(None)),
                _ => super::script_action(s).map(Step::Press),
            };
            if step.is_none() {
                warn!("menus: the script step {s:?} means nothing");
            }
            step
        })
        .collect()
}

#[derive(Resource)]
struct Script {
    steps: Vec<Step>,
    next: usize,
    /// When the current step may run.
    at: f32,
    shots: std::path::PathBuf,
    /// Escape went down last frame, in this window: it comes up now.
    escape_up: Option<Entity>,
}

pub(super) fn register(app: &mut App) {
    let Ok(script) = std::env::var("ESURV_MENUS_SCRIPT") else {
        return;
    };
    let steps = parse(&script);
    info!(
        "menus: a script of {} steps (ESURV_MENUS_SCRIPT)",
        steps.len()
    );
    let shots = std::env::var("ESURV_MENUS_SHOTS").unwrap_or_else(|_| ".".into());
    app.insert_resource(Script {
        steps,
        next: 0,
        at: 0.0,
        shots: shots.into(),
        escape_up: None,
    })
    .init_resource::<ForcedTip>()
    .add_systems(Update, (run_script.before(super::clicks_to_presses), force_the_tip));
}

fn run_script(
    mut commands: Commands,
    mut script: ResMut<Script>,
    time: Res<Time>,
    screen: Res<SelectScreen>,
    menus: Res<Menus>,
    mut presses: MessageWriter<MenuPress>,
    mut exit: MessageWriter<AppExit>,
    mut window: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    mut keys: MessageWriter<KeyboardInput>,
    mut forced: ResMut<ForcedTip>,
    mut end: Option<ResMut<crate::run_end::RunEndState>>,
) {
    if let Some(w) = script.escape_up.take() {
        keys.write(crate::hud::escape_key(w, ButtonState::Released));
    }
    let now = time.elapsed_secs();
    if now < script.at {
        return;
    }
    let Some(step) = script.steps.get(script.next).cloned() else {
        return;
    };
    let mut pause = STEP_PAUSE;
    match step {
        Step::Wait(t) => pause = t,
        Step::Ready => {
            if !(screen.open && screen.scene_ready && menus.screen != Screen::Unset) {
                return;
            }
            // the camp's figures come in and pose
            pause = 6.0;
        }
        Step::Shot(name) => {
            let path = script.shots.join(format!("{name}.png"));
            info!("menus: writing {}", path.display());
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
        Step::Size(w, h) => {
            // the game's own window, resized from inside: nothing of the desktop is touched
            if let Ok((_, mut win)) = window.single_mut() {
                win.resolution.set(w, h);
            }
        }
        Step::OverRun => {
            if !menus.options_over_the_run() {
                return;
            }
        }
        Step::Escape => {
            if let Ok((w, _)) = window.single() {
                info!("menus: the script presses Escape");
                keys.write(crate::hud::escape_key(w, ButtonState::Pressed));
                script.escape_up = Some(w);
            }
        }
        Step::Quit => {
            info!("menus: the script ends the game");
            exit.write(AppExit::Success);
        }
        Step::Tip(id) => forced.0 = id,
        Step::Double(action) => {
            info!("menus: the script double-clicks {action:?}");
            presses.write(MenuPress(action.clone()));
            presses.write(MenuPress(action));
        }
        Step::EndPanel(ending) => {
            // once the hero is in its run
            if !end.as_mut().is_some_and(|e| e.show_sample(ending)) {
                return;
            }
        }
        Step::Press(action) => {
            info!("menus: the script presses {action:?}");
            presses.write(MenuPress(action));
        }
    }
    script.next += 1;
    script.at = now + pause;
}

/// The icon whose tooltip a script shows, by what it presses.
#[derive(Resource, Default)]
struct ForcedTip(Option<MenuAction>);

/// Keeps the forced tooltip on its icon (the screen is drawn anew as it changes).
fn force_the_tip(
    mut commands: Commands,
    forced: Res<ForcedTip>,
    icons: Query<(Entity, &MenuAction, Has<crate::stock::ShowTipNow>)>,
) {
    for (e, action, now) in &icons {
        let want = forced.0.as_ref() == Some(action);
        if want && !now {
            commands.entity(e).insert(crate::stock::ShowTipNow);
        } else if !want && now {
            commands.entity(e).remove::<crate::stock::ShowTipNow>();
        }
    }
}

/// The sample of the look: a panel with a heading, labels and values, value boxes, icons (one
/// picked, one locked), arrows and dots, and the buttons.
pub(super) fn draw_sample(commands: &mut Commands, root: Entity, look: &MenuLook) {
    let mut node = look.panel_node();
    node.width = look.px(420.0);
    node.margin = UiRect::all(Val::Auto);
    node.row_gap = look.px(10.0);
    commands.entity(root).with_children(|r| {
        r.spawn(look.panel(node)).with_children(|p| {
            look.heading(p, "Class", 22.0);
            p.spawn(look.label("A label"));
            p.spawn(look.value("A value in white"));
            p.spawn(look.text(Face::Display, 26.0, palette::HEADING, "Morpheus heading"));
            p.spawn(Node {
                column_gap: look.px(8.0),
                ..default()
            })
            .with_children(|row| {
                look.value_box(row, "Health", "420", palette::VALUE);
                look.value_box(row, "Starts with", "My New Spell", palette::HEADING);
                look.value_box(row, "Zones open", "1 of 9", palette::VALUE);
            });
            p.spawn(Node {
                column_gap: look.px(8.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                let icon = look.icon_art("Spell_Fire_Fireball");
                look.icon(row, icon.clone(), None, 36.0, true, false, false)
                    .insert(MenuAction::Nothing);
                look.icon(row, icon.clone(), None, 36.0, false, false, false)
                    .insert(MenuAction::Nothing);
                look.icon(row, icon, None, 36.0, false, true, false)
                    .insert(MenuAction::Nothing);
                look.arrow(row, Arrow::Left, 22.0, true)
                    .insert(MenuAction::Nothing);
                row.spawn(look.value("3 / 9"));
                look.arrow(row, Arrow::Right, 22.0, true)
                    .insert(MenuAction::Nothing);
            });
            look.dots(p, 9, 2);
            p.spawn(Node {
                column_gap: look.px(8.0),
                ..default()
            })
            .with_children(|row| {
                look.button(row, "Character", ButtonStyle::Active, true, None)
                    .insert(MenuAction::Nothing);
                look.button(row, "Collection", ButtonStyle::Text, true, None)
                    .insert(MenuAction::Nothing);
                look.button(row, "Talents", ButtonStyle::Text, false, None)
                    .insert(MenuAction::Nothing);
            });
            look.button(p, "Start run", ButtonStyle::Primary, true, None)
                .insert(MenuAction::Nothing);
            p.spawn(look.hint("A hint at a column's foot."));
        });
    });
}

/// A line the menus say (a refusal), over the middle of the window's top.
pub(super) fn draw_notice(commands: &mut Commands, root: Entity, look: &MenuLook, text: &str) {
    commands.entity(root).with_children(|r| {
        r.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: look.px(110.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|band| {
            band.spawn(look.text(Face::Plain, 13.0, palette::RED, text));
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_reads_its_steps() {
        let steps = parse("ready; wait:2.5 ;shot:class-picker; nonsense; quit");
        assert_eq!(
            steps,
            vec![
                Step::Ready,
                Step::Wait(2.5),
                Step::Shot("class-picker".into()),
                Step::Quit
            ]
        );
    }

    /// A tooltip step names the icon by what it presses: a starting option or a loadout.
    #[test]
    fn a_tooltip_step_names_its_icon() {
        assert_eq!(
            parse("tip-start:revenge; tip-loadout:shield; tip-off; loadout:gun"),
            vec![
                Step::Tip(Some(MenuAction::PickStart("revenge".into()))),
                Step::Tip(Some(MenuAction::PickLoadout("shield".into()))),
                Step::Tip(None),
                Step::Press(MenuAction::PickLoadout("gun".into())),
            ]
        );
    }
}
