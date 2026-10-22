//! Lists that can hold more than their panel shows (the zones, the collection log's rows): they
//! scroll with the mouse wheel over them, and keep their place when the menus are drawn anew.

use std::collections::HashMap;

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

/// How far one wheel line scrolls, in logical pixels.
const LINE: f32 = 30.0;

/// Which list a scrolling node is, so its place is kept across redraws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum ScrollingList {
    Zones,
    CollectionLog,
    /// The talent trees with the prestige section under them.
    Talents,
}

/// A node whose children scroll up and down with the wheel when they are more than it shows.
#[derive(Component, Clone, Copy)]
pub(super) struct ScrollsWithWheel(pub ScrollingList);

/// Where each list was scrolled to, in logical pixels.
#[derive(Resource, Default)]
pub(super) struct ListScrolls(HashMap<ScrollingList, f32>);

/// The parts a scrolling list's node carries: what it is, the wheel's target, and where it was.
pub(super) fn scrolling(list: ScrollingList) -> impl Bundle {
    (
        ScrollsWithWheel(list),
        RelativeCursorPosition::default(),
        ScrollPosition::default(),
    )
}

/// A list drawn anew starts where it was (the layout keeps it within its rows).
fn keep_the_place(
    scrolls: Res<ListScrolls>,
    mut added: Query<(&ScrollsWithWheel, &mut ScrollPosition), Added<ScrollsWithWheel>>,
) {
    for (list, mut at) in &mut added {
        at.y = scrolls.0.get(&list.0).copied().unwrap_or(0.0);
    }
}

/// The wheel over a list scrolls it.
fn scroll_with_wheel(
    wheel: Res<AccumulatedMouseScroll>,
    mut scrolls: ResMut<ListScrolls>,
    mut lists: Query<(
        &ScrollsWithWheel,
        &RelativeCursorPosition,
        &InheritedVisibility,
        &mut ScrollPosition,
    )>,
) {
    if wheel.delta.y == 0.0 {
        return;
    }
    let pixels = match wheel.unit {
        MouseScrollUnit::Line => wheel.delta.y * LINE,
        MouseScrollUnit::Pixel => wheel.delta.y,
    };
    for (list, cursor, shown, mut at) in &mut lists {
        if shown.get() && cursor.cursor_over() {
            at.y = (at.y - pixels).max(0.0);
            scrolls.0.insert(list.0, at.y);
        }
    }
}

/// The layout may have held a list short of where it was asked to go: remember where it is.
fn remember_the_place(
    mut scrolls: ResMut<ListScrolls>,
    lists: Query<(&ScrollsWithWheel, &ScrollPosition), Changed<ScrollPosition>>,
) {
    for (list, at) in &lists {
        if scrolls.0.get(&list.0) != Some(&at.y) {
            scrolls.0.insert(list.0, at.y);
        }
    }
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<ListScrolls>().add_systems(
        Update,
        (keep_the_place, scroll_with_wheel, remember_the_place).chain(),
    );
}
