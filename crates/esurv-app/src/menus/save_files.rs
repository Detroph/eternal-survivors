//! The Options panel's Saves tab (design step 6, "options (save export and import)").
//! The game has no file dialog, so the save
//! goes to and comes from one fixed folder beside it (`saves/exports` in benilla's state folder),
//! named on the page:
//!
//! * **Export save** writes the whole save as it stands to a new file there, named after the date
//!   and time (`heroes-2026-10-13-142530.ron`, UTC);
//! * the folder's saves are listed newest first, each with how many heroes it holds, or greyed
//!   when it is no save this game can take (the server reads and checks each one);
//! * **Import save** takes the save picked in the list, after a question in the menus' look that
//!   says what it replaces; the server checks it again, keeps the save it replaces in the same
//!   folder (`heroes-before-import-<date and time>.ron`, which the list then shows, so an import
//!   can be undone), and only then replaces the save. The character list is read anew.
//!
//! It all goes through the app's way to the save before a run ([`crate::prerun::PreRunLink`]):
//! only the game the save is kept on has one that answers it; any other is refused.
//!
//! Over a paused run (the run's Escape menu, Options) the page exports only: an import would
//! replace the save the run is paid out into, so it waits for the warband screen, and a line on
//! the page says so ([`SavesPage::in_run`]).

use bevy::prelude::*;
use esurv_protocol::{PreRunAsk, SaveFileFacts, ToClient};

use super::look::{palette, solid, ButtonStyle, Face, MenuLook};
use super::MenuAction;
use crate::prerun::PreRunLink;

/// How many saves the page lists (the newest).
pub const SHOWN: usize = 8;

/// The Saves tab as it shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SavesPage {
    /// The folder and its saves as the save last told them; `None` until told (or with no way to
    /// the save).
    pub folder: Option<String>,
    pub files: Vec<SaveFileFacts>,
    /// The save picked in the list, by its name.
    pub picked: Option<String>,
    /// The import question is up.
    pub confirm: bool,
    /// What the page says after an export or an import.
    pub said: Option<String>,
    /// The page shows over a paused run: export only. An import would replace the save the run
    /// is paid out into, so it waits for the warband screen.
    pub in_run: bool,
}

impl SavesPage {
    /// The picked save, when it is one this game can take; none over a run, where nothing is
    /// imported.
    pub fn importable(&self) -> Option<&SaveFileFacts> {
        if self.in_run {
            return None;
        }
        let name = self.picked.as_deref()?;
        self.files
            .iter()
            .find(|f| f.name == name && f.heroes.is_some())
    }

    /// Takes what the save told; whether the save was replaced (an import went through).
    pub fn take(&mut self, told: Vec<ToClient>) -> bool {
        let mut imported = false;
        for m in told {
            match m {
                ToClient::SaveFiles { folder, files } => {
                    self.folder = Some(folder);
                    self.files = files;
                    if self
                        .picked
                        .as_ref()
                        .is_some_and(|p| !self.files.iter().any(|f| &f.name == p))
                    {
                        self.picked = None;
                    }
                }
                ToClient::SaveExported(file) => {
                    self.said = Some(format!("The save was written to {file}."));
                    self.picked = Some(file);
                }
                ToClient::SaveImported { file, backup } => {
                    self.said = Some(format!(
                        "The save now comes from {file}. The save it replaced is kept as {backup}."
                    ));
                    self.picked = None;
                    imported = true;
                }
                ToClient::Refused(verb) => {
                    self.said = Some(
                        match verb.as_str() {
                            "export" => "The save could not be written.",
                            "import" => "That file could not be imported: nothing was changed.",
                            _ => "Saves can be exported and imported only on the machine the save is kept on.",
                        }
                        .to_owned(),
                    );
                }
                _ => {}
            }
        }
        imported
    }

    /// Picks a save by its name, or by its place in the list (`0` the newest), as a dev script
    /// names it.
    pub fn pick(&mut self, name_or_place: &str) {
        let by_place = name_or_place
            .parse::<usize>()
            .ok()
            .and_then(|i| self.files.get(i));
        let found = by_place.or_else(|| self.files.iter().find(|f| f.name == name_or_place));
        if let Some(f) = found {
            self.picked = Some(f.name.clone());
        }
    }
}

/// Asks the save `ask` and takes what it told; whether the save was replaced. Without a way to the
/// save the page says so.
pub fn ask(page: &mut SavesPage, link: Option<&PreRunLink>, ask: &PreRunAsk) -> bool {
    match link {
        Some(link) => page.take(link.ask(ask)),
        None => page.take(vec![ToClient::Refused(ask.verb().into())]),
    }
}

/// How many heroes a save holds, in words.
pub fn heroes_in(f: &SaveFileFacts) -> String {
    match f.heroes {
        Some(1) => "1 hero".into(),
        Some(n) => format!("{n} heroes"),
        None => "Not a save this game can take".into(),
    }
}

/// The page, in the Options panel under its tabs.
pub(super) fn draw_page(p: &mut ChildSpawnerCommands, look: &MenuLook, page: &SavesPage) {
    p.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: look.px(6.0),
        padding: UiRect::axes(look.px(12.0), look.px(4.0)),
        ..default()
    })
    .with_children(|c| {
        c.spawn(look.value(
            "One save holds every hero, what each class has earned, the gear collection, the codex \
             and the run options. Export writes a copy of it; import replaces it with a copy.",
        ));
        c.spawn(look.label("Folder"));
        let folder = match &page.folder {
            Some(f) if !f.is_empty() => f.clone(),
            Some(_) => "This save is kept in memory only: it has no folder.".into(),
            None => "Only the game the save is kept on can export and import it.".into(),
        };
        c.spawn(look.text(Face::Plain, 10.0, palette::HEADING, folder));
        c.spawn(look.hint(
            "Exports are written there, named with the date and time (UTC). To import a save from \
             elsewhere, put its file there first.",
        ));
        c.spawn(Node {
            column_gap: look.px(12.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            look.button(
                row,
                "Export save",
                ButtonStyle::Primary,
                page.folder.as_deref().is_some_and(|f| !f.is_empty()),
                Some(160.0),
            )
            .insert(MenuAction::ExportSave);
            look.button(
                row,
                "Import save",
                ButtonStyle::Text,
                page.importable().is_some(),
                Some(160.0),
            )
            .insert(MenuAction::ImportSave);
            row.spawn(look.hint(if page.in_run {
                "Import works from the warband screen, before a run."
            } else {
                "Pick a save in the list to import it."
            }));
        });
        c.spawn(look.label("Saves in the folder"));
        if page.files.is_empty() {
            c.spawn(look.hint("None yet."));
        }
        for f in page.files.iter().take(SHOWN) {
            let picked = page.picked.as_deref() == Some(f.name.as_str());
            let style = if picked {
                ButtonStyle::RowPicked
            } else {
                ButtonStyle::Row
            };
            let readable = f.heroes.is_some();
            look.frame_button(c, style, readable, None)
                .insert(MenuAction::PickSaveFile(f.name.clone()))
                .with_children(|b| {
                    b.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: look.px(12.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|l| {
                        let colour = if readable {
                            palette::VALUE
                        } else {
                            palette::HINT
                        };
                        l.spawn(look.text(Face::Plain, 10.5, colour, f.name.clone()));
                        l.spawn(look.text(Face::Plain, 10.0, palette::HINT, heroes_in(f)));
                    });
                });
        }
        if page.files.len() > SHOWN {
            c.spawn(look.hint(format!("And {} older ones.", page.files.len() - SHOWN)));
        }
        if let Some(said) = &page.said {
            c.spawn(look.text(Face::Plain, 11.0, palette::HEADING, said.clone()));
        }
    });
}

/// The import question, over the Options panel: what is replaced and what is kept; No is the
/// default.
pub(super) fn draw_confirm(
    commands: &mut Commands,
    root: Entity,
    look: &MenuLook,
    file: &SaveFileFacts,
) {
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            solid(),
            GlobalZIndex(super::MENU_LAYER + 20),
        ))
        .with_children(|veil| {
            let mut node = look.panel_node();
            node.width = look.px(420.0);
            node.align_items = AlignItems::Center;
            node.row_gap = look.px(12.0);
            node.padding = UiRect::all(look.px(18.0));
            veil.spawn(look.panel(node)).with_children(|p| {
                look.heading(p, "Import save", 20.0);
                p.spawn((
                    look.value("Replace the save with this one?"),
                    TextLayout::new_with_justify(Justify::Center),
                ));
                p.spawn(look.text(Face::Display, 15.0, palette::HEADING, file.name.clone()));
                p.spawn(look.text(Face::Plain, 10.5, palette::VALUE, heroes_in(file)));
                p.spawn((
                    look.hint(
                        "Every hero, what each class has earned, the collection, the codex and the \
                         run options are replaced by the file's. The save you have now is kept in \
                         the same folder first, so it can be imported back.",
                    ),
                    TextLayout::new_with_justify(Justify::Center),
                ));
                p.spawn(Node {
                    column_gap: look.px(14.0),
                    ..default()
                })
                .with_children(|row| {
                    look.button(row, "No", ButtonStyle::Active, true, Some(100.0))
                        .insert(MenuAction::ImportAnswer(false));
                    look.button(row, "Yes", ButtonStyle::Text, true, Some(100.0))
                        .insert(MenuAction::ImportAnswer(true));
                });
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed() -> Vec<ToClient> {
        vec![ToClient::SaveFiles {
            folder: r"C:\state\saves\exports".into(),
            files: vec![
                SaveFileFacts {
                    name: "heroes-2026-10-13-150000.ron".into(),
                    heroes: Some(3),
                },
                SaveFileFacts {
                    name: "broken.ron".into(),
                    heroes: None,
                },
            ],
        }]
    }

    #[test]
    fn only_a_save_the_game_can_take_is_importable() {
        let mut page = SavesPage::default();
        assert!(!page.take(listed()));
        assert_eq!(page.folder.as_deref(), Some(r"C:\state\saves\exports"));
        page.pick("1");
        assert_eq!(page.picked.as_deref(), Some("broken.ron"));
        assert!(page.importable().is_none(), "no save this game can take");
        page.pick("heroes-2026-10-13-150000.ron");
        assert_eq!(page.importable().map(|f| f.heroes), Some(Some(3)));
        page.pick("not listed");
        assert_eq!(page.picked.as_deref(), Some("heroes-2026-10-13-150000.ron"));
    }

    #[test]
    fn over_a_run_nothing_is_importable() {
        let mut page = SavesPage {
            in_run: true,
            ..default()
        };
        page.take(listed());
        page.pick("heroes-2026-10-13-150000.ron");
        assert!(
            page.importable().is_none(),
            "the save waits for the warband screen"
        );
        page.in_run = false;
        assert!(page.importable().is_some());
    }

    #[test]
    fn an_export_is_picked_and_an_import_says_what_it_kept() {
        let mut page = SavesPage::default();
        page.take(listed());
        assert!(!page.take(vec![ToClient::SaveExported("heroes-x.ron".into())]));
        assert_eq!(page.picked.as_deref(), Some("heroes-x.ron"));
        assert!(page.said.as_deref().unwrap().contains("heroes-x.ron"));
        let imported = page.take(vec![ToClient::SaveImported {
            file: "heroes-x.ron".into(),
            backup: "heroes-before-import-y.ron".into(),
        }]);
        assert!(imported);
        assert!(page
            .said
            .as_deref()
            .unwrap()
            .contains("heroes-before-import-y.ron"));
        assert!(!page.take(vec![ToClient::Refused("import".into())]));
        assert!(page
            .said
            .as_deref()
            .unwrap()
            .starts_with("That file could not"));
    }

    #[test]
    fn without_a_way_to_the_save_the_page_says_so() {
        let mut page = SavesPage::default();
        assert!(!ask(&mut page, None, &PreRunAsk::ExportSave));
        assert_eq!(page.said.as_deref(), Some("The save could not be written."));
        ask(&mut page, None, &PreRunAsk::SaveFiles);
        assert!(page
            .said
            .as_deref()
            .unwrap()
            .starts_with("Saves can be exported"));
    }
}
