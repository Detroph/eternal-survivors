//! The guide on the warband screen (menus.md, "The flow", step 4; screens.md, "Guide"): the
//! "Guide" button over the armour opens a panel over the screen that explains the game's
//! mechanics, a tab each for the stats, the enemies, the level-up cards and the loot. It wears the
//! codex's look (a veil, a panel, a tab row of the look's buttons, Close); the tab's sections
//! scroll with the wheel. What it says is built from the content ([`entries`]), so its numbers
//! follow the balance.

mod entries;

use bevy::prelude::*;

use super::look::{palette, solid, ButtonStyle, Face, MenuLook};
use super::scroll::{scrolling, ScrollingList};
use super::{Ctx, MenuAction};
use entries::{GuideEntry, GuideIcon, GuideSection};

/// Sizes, in units.
const VIEW_WIDTH: f32 = 860.0;
const BODY_HEIGHT: f32 = 470.0;
const ICON: f32 = 30.0;

/// The guide's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GuideTab {
    #[default]
    Stats,
    Enemies,
    Upgrades,
    Loot,
}

impl GuideTab {
    pub const ALL: [GuideTab; 4] = [
        GuideTab::Stats,
        GuideTab::Enemies,
        GuideTab::Upgrades,
        GuideTab::Loot,
    ];

    /// Its name on its tab.
    pub fn label(self) -> &'static str {
        match self {
            GuideTab::Stats => "Stats",
            GuideTab::Enemies => "Enemies",
            GuideTab::Upgrades => "Upgrades",
            GuideTab::Loot => "Loot",
        }
    }

    /// The tab a dev script names (`guide-tab:loot`).
    pub fn named(word: &str) -> Option<GuideTab> {
        GuideTab::ALL
            .into_iter()
            .find(|t| t.label().eq_ignore_ascii_case(word))
    }
}

/// The guide open on `tab`.
pub(super) fn draw(commands: &mut Commands, root: Entity, look: &MenuLook, ctx: &Ctx, tab: GuideTab) {
    let sections = entries::sections(ctx.content, tab);
    commands.entity(root).with_children(|r| {
        r.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            solid(),
            GlobalZIndex(super::MENU_LAYER + 10),
        ))
        .with_children(|veil| {
            let mut node = look.panel_node();
            node.width = look.px(VIEW_WIDTH);
            node.row_gap = look.px(10.0);
            node.padding = UiRect::all(look.px(18.0));
            veil.spawn(look.panel(node)).with_children(|p| {
                look.heading(p, "Guide", 24.0);
                p.spawn((
                    look.hint("How the game works: what your stats do, who you fight, what level-ups offer and what you can pick up."),
                    TextLayout::new_with_justify(Justify::Center),
                ));
                p.spawn(Node {
                    justify_content: JustifyContent::Center,
                    column_gap: look.px(6.0),
                    ..default()
                })
                .with_children(|tabs| {
                    for t in GuideTab::ALL {
                        let style = if t == tab {
                            ButtonStyle::Active
                        } else {
                            ButtonStyle::Text
                        };
                        look.button(tabs, t.label(), style, true, Some(140.0))
                            .insert(MenuAction::GuideTab(t));
                    }
                });
                p.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: look.px(8.0),
                        max_height: look.px(BODY_HEIGHT),
                        min_height: Val::ZERO,
                        overflow: Overflow::scroll_y(),
                        padding: UiRect::right(look.px(8.0)),
                        ..default()
                    },
                    scrolling(ScrollingList::Guide(tab)),
                ))
                .with_children(|body| {
                    for s in &sections {
                        section(body, look, ctx, s);
                    }
                });
                p.spawn(Node {
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|row| {
                    look.button(row, "Close", ButtonStyle::Primary, true, Some(140.0))
                        .insert(MenuAction::CloseGuide);
                });
            });
        });
    });
}

/// A section: its heading, its line, its entries in two columns.
fn section(body: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, s: &GuideSection) {
    look.heading(body, s.title, 15.0);
    if let Some(h) = &s.hint {
        body.spawn((look.hint(h.clone()), Node { flex_shrink: 0.0, ..default() }));
    }
    // with any icon in the section, an entry without one keeps its place
    let icons: Vec<Option<Handle<Image>>> = s.entries.iter().map(|e| icon(look, ctx, e)).collect();
    let keep_place = icons.iter().any(Option::is_some);
    body.spawn(Node {
        display: Display::Grid,
        grid_template_columns: vec![GridTrack::flex(1.0), GridTrack::flex(1.0)],
        column_gap: look.px(16.0),
        row_gap: look.px(8.0),
        flex_shrink: 0.0,
        ..default()
    })
    .with_children(|grid| {
        for (e, image) in s.entries.iter().zip(icons) {
            entry(grid, look, e, image, keep_place);
        }
    });
}

/// An entry's icon, when the game has it here (a spell's needs the player's data).
fn icon(look: &MenuLook, ctx: &Ctx, e: &GuideEntry) -> Option<Handle<Image>> {
    match e.icon.as_ref()? {
        GuideIcon::Named(name) => Some(look.icon_art(name)),
        GuideIcon::Spell(id) => ctx.racials.icon(*id).map(|path| look.art(path)),
    }
}

/// One entry: its icon, its name (in its glow's colour) and its line.
fn entry(
    grid: &mut ChildSpawnerCommands,
    look: &MenuLook,
    e: &GuideEntry,
    image: Option<Handle<Image>>,
    keep_place: bool,
) {
    grid.spawn(Node {
        column_gap: look.px(8.0),
        align_items: AlignItems::FlexStart,
        ..default()
    })
    .with_children(|row| {
        match image {
            Some(image) => {
                look.icon(row, image, None, ICON, false, false, false);
            }
            None if keep_place => {
                row.spawn(Node {
                    width: look.px(ICON),
                    flex_shrink: 0.0,
                    ..default()
                });
            }
            None => {}
        }
        row.spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_shrink: 1.0,
            row_gap: look.px(1.0),
            ..default()
        })
        .with_children(|t| {
            let colour = e.glow.map_or(palette::HEADING, |g| g.colour());
            t.spawn(look.text(Face::Plain, 11.0, colour, e.name.clone()));
            t.spawn(look.text(Face::Plain, 9.0, palette::VALUE, e.text.clone()));
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_names_each_tab() {
        for t in GuideTab::ALL {
            assert_eq!(GuideTab::named(&t.label().to_lowercase()), Some(t));
        }
        assert_eq!(GuideTab::named("talents"), None);
    }
}
