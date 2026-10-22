//! `ESURV_TOOLTIP_SHOT=<where>:<n>@<when>[,...]`, a development aid: shows a spell's tooltip
//! inside the game with nobody at the mouse, and leaves it up, for a look at our spell tooltips
//! ([`super::spell_tips`], [`super::buff_tips`]). `<where>:<n>` is `action:<n>` (button `n` of
//! the main action bar), `book:<n>` (the spellbook's `n`-th spell, which opens first) or
//! `buff:<n>` (the buff frame's `n`-th buff); the Lua calls the stock hover code for that button
//! (`ActionButton_SetTooltip`, `SpellButton_OnEnter`, the buff button's `SetPlayerBuff`), so the
//! tooltip goes the whole stock way. `<when>` is real seconds after the hero entered the world
//! (`@15`), or
//! `rank<r>` (`@rank2`): once the spell that button holds has reached rank `r`, as after a level-up
//! card upgraded it. The items go in turn.
//!
//! `ESURV_TOOLTIP_SHOTS=<png>` adds the game's own shot 1 s after each, `<stem>-<i>-<where><n>.png`
//! (`<i>` counting from 1); the window is never raised or pinned.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use benilla_app::Objects;
use benilla_ui::script::UiScript;

use super::spell_tips::RunSpellTips;
use crate::ui::in_world;

const SWITCH: &str = "ESURV_TOOLTIP_SHOT";
const SHOTS: &str = "ESURV_TOOLTIP_SHOTS";
/// Real seconds from a tooltip shown to its shot.
const SHOT_AFTER: f32 = 1.0;

/// Which button's tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Button {
    Action(u32),
    Book(u32),
    /// The buff frame's `n`-th buff (the hero's own).
    Buff(u32),
}

impl Button {
    fn name(self) -> String {
        match self {
            Button::Action(n) => format!("action{n}"),
            Button::Book(n) => format!("book{n}"),
            Button::Buff(n) => format!("buff{n}"),
        }
    }

    fn lua(self) -> String {
        match self {
            Button::Action(n) => format!("ESURV_RUN_FRAMES.show_tip_for('action', {n})"),
            Button::Book(n) => format!("ESURV_RUN_FRAMES.show_tip_for('book', {n})"),
            Button::Buff(n) => format!("ESURV_RUN_FRAMES.show_tip_for('buff', {n})"),
        }
    }

    /// The spell the button holds.
    fn spell(self, script: &UiScript) -> Option<u32> {
        match self {
            // the main bar's button n is action n on its first page
            Button::Action(n) => script.action_spell(n),
            Button::Book(n) => script
                .spellbook()
                .slots
                .get((n as usize).checked_sub(1)?)
                .map(|s| s.spell_id),
            // a buff has no rank to wait for
            Button::Buff(_) => None,
        }
    }
}

/// When a tooltip shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum When {
    /// Real seconds after the hero entered the world.
    At(f32),
    /// Once the button's spell has reached this rank.
    Rank(u32),
}

/// Reads the switch: `<where>:<n>@<when>` items, comma-separated, in order; anything else is left
/// out.
pub(super) fn parse_shots(value: &str) -> Vec<(Button, When)> {
    value
        .split(',')
        .filter_map(|item| {
            let (button, when) = item.trim().split_once('@')?;
            let (kind, n) = button.trim().split_once(':')?;
            let n: u32 = n.trim().parse().ok().filter(|n| *n >= 1)?;
            let button = match kind.trim() {
                "action" => Button::Action(n),
                "book" => Button::Book(n),
                "buff" => Button::Buff(n),
                _ => return None,
            };
            let when = when.trim();
            let when = match when.strip_prefix("rank") {
                Some(r) => When::Rank(r.trim().parse().ok()?),
                None => When::At(
                    when.parse()
                        .ok()
                        .filter(|t: &f32| t.is_finite() && *t >= 0.0)?,
                ),
            };
            Some((button, when))
        })
        .collect()
}

/// `<stem>-<i>-<what>.png` from the shots' path.
fn shot_path(out: &str, i: usize, what: &str) -> String {
    match out.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}-{i}-{what}.{ext}"),
        None => format!("{out}-{i}-{what}.png"),
    }
}

#[derive(Resource)]
struct TooltipShots {
    due: Vec<(Button, When)>,
    shots: Option<String>,
    /// When the hero entered the world (real seconds).
    entered: Option<f32>,
    /// The shot to take: (when, path).
    shot_due: Option<(f32, String)>,
    shown: usize,
}

pub(super) fn add(app: &mut App) {
    let Ok(value) = std::env::var(SWITCH) else {
        return;
    };
    let mut due = parse_shots(&value);
    if due.is_empty() {
        return;
    }
    due.reverse();
    info!("run frames: spell tooltips will be shown: {value} ({SWITCH})");
    app.insert_resource(TooltipShots {
        due,
        shots: std::env::var(SHOTS).ok().filter(|v| !v.is_empty()),
        entered: None,
        shot_due: None,
        shown: 0,
    })
    .add_systems(
        Update,
        show_tooltips.after(super::spell_tips::hand_the_tips_to_the_lua),
    );
}

fn show_tooltips(
    mut shots: ResMut<TooltipShots>,
    script: Option<NonSend<UiScript>>,
    tips: Res<RunSpellTips>,
    objects: Objects,
    time: Res<Time<Real>>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs();
    if let Some((at, _)) = &shots.shot_due {
        if now >= *at {
            let (_, path) = shots.shot_due.take().unwrap();
            info!("run frames: shot {path} ({SHOTS})");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
        return;
    }
    let Some(script) = script else {
        return;
    };
    if !in_world(&objects) {
        shots.entered = None;
        return;
    }
    let entered = *shots.entered.get_or_insert(now);
    let Some(&(button, when)) = shots.due.last() else {
        return;
    };
    let ready = match when {
        When::At(t) => now - entered >= t,
        When::Rank(r) => button.spell(&script).is_some_and(|id| {
            tips.0
                .as_ref()
                .and_then(|t| t.iter().find(|t| t.look == id))
                .is_some_and(|t| t.rank >= r)
        }),
    };
    if !ready {
        return;
    }
    shots.due.pop();
    shots.shown += 1;
    info!(
        "run frames: the tooltip of {} is shown ({SWITCH})",
        button.name()
    );
    if let Err(e) = script.run(&button.lua()) {
        warn!("run frames: the tooltip of {}: {e}", button.name());
    }
    if let Some(out) = shots.shots.clone() {
        let path = shot_path(&out, shots.shown, &button.name());
        shots.shot_due = Some((now + SHOT_AFTER, path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shots_read_in_their_order() {
        assert_eq!(
            parse_shots(
                "action:1@15, book:2@rank2,buff:1@6,action:0@3,shelf:1@2,action:2@x,action:3@rank"
            ),
            vec![
                (Button::Action(1), When::At(15.0)),
                (Button::Book(2), When::Rank(2)),
                (Button::Buff(1), When::At(6.0)),
            ]
        );
        assert_eq!(
            shot_path("C:/s/tip.png", 2, "book3"),
            "C:/s/tip-2-book3.png"
        );
        assert_eq!(Button::Action(4).name(), "action4");
    }
}
