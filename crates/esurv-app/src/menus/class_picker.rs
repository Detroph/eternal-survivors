//! The class picker (menus.md, "The flow", step 1): one panel over the
//! whole window. A "Class" heading over a 3 × 3 grid of the nine classes (the stock create
//! screen's class icons, the name under each; the picked one glows and its name takes the class
//! colour); under the grid the class's name, a line on how it plays, three value boxes (health,
//! the spell it starts with, zones open), and its spell pool as a row of spell icons, each with
//! its tooltip. Choosing the class opens the hero sheet on a new hero of it.

use bevy::prelude::*;
use esurv_content::{Ability, Class, School};

use super::art::{cell_rect, class_cell};
use super::look::{palette, ButtonStyle, Face, MenuLook};
use super::{Ctx, MenuAction};
use crate::stock::{MenuLookTip, Tip, TipLine};
use crate::ui::class_colour;

/// Sizes, in units.
const PANEL_WIDTH: f32 = 560.0;
const CLASS_ICON: f32 = 56.0;
const CELL_WIDTH: f32 = 150.0;
const SPELL_ICON: f32 = 30.0;

/// A school's name as a spell's tooltip says it.
fn school_word(s: School) -> &'static str {
    match s {
        School::Physical => "Physical",
        School::Holy => "Holy",
        School::Fire => "Fire",
        School::Nature => "Nature",
        School::Frost => "Frost",
        School::Shadow => "Shadow",
        School::Arcane => "Arcane",
    }
}

/// A seconds count as a tooltip says it: `8 s`, `1.5 s`.
fn seconds(s: f32) -> String {
    if (s - s.round()).abs() < 0.05 {
        format!("{} s", s.round() as i64)
    } else {
        format!("{s:.1} s")
    }
}

/// A pool spell's tooltip: its name, school, cooldown, and whether the class starts with it.
fn spell_tip(a: &Ability, starts_with: bool) -> Vec<TipLine> {
    let mut lines = vec![
        TipLine::new(a.name.clone(), palette::HEADING),
        TipLine::new(format!("{} spell", school_word(a.school)), palette::VALUE),
    ];
    if a.cooldown > 0.0 {
        lines.push(TipLine::new(
            format!("Casts itself every {}", seconds(a.cooldown)),
            palette::VALUE,
        ));
    }
    lines.push(if starts_with {
        TipLine::new("Every run starts with it.", palette::GREEN)
    } else {
        TipLine::new("Offered when a level is gained.", palette::LABEL)
    });
    lines
}

/// How many zones the class has open, of all, as the save says (the first always is).
fn zones_line(ctx: &Ctx, class: &Class) -> String {
    let all = ctx.content.zones.len();
    let open = ctx
        .facts
        .progress(&class.id)
        .map_or(1, |p| (p.zones_open.max(1) as usize).min(all));
    format!("{open} of {all}")
}

pub(super) fn draw(
    commands: &mut Commands,
    root: Entity,
    look: &MenuLook,
    ctx: &Ctx,
    picked: &str,
    notice: Option<&str>,
) {
    let Some(class) = ctx
        .content
        .class(picked)
        .or_else(|| ctx.content.classes.first())
    else {
        return;
    };
    let colour = class_colour(&class.id).unwrap_or(palette::VALUE);
    commands.entity(root).with_children(|r| {
        // the camp's world dims behind the picker
        r.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(look.px(20.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            Pickable::IGNORE,
        ))
        .with_children(|veil| {
            let mut node = look.panel_node();
            node.width = look.px(PANEL_WIDTH);
            node.max_height = Val::Percent(100.0);
            node.padding = UiRect::axes(look.px(24.0), look.px(18.0));
            node.row_gap = look.px(10.0);
            node.align_items = AlignItems::Stretch;
            veil.spawn(look.panel(node)).with_children(|p| {
                look.heading(p, "Class", 26.0);
                grid(p, look, ctx, &class.id);
                p.spawn(look.divider());
                about(p, look, ctx, class, colour);
                buttons(p, look, ctx, class);
                if let Some(n) = notice {
                    p.spawn(look.text(Face::Plain, 10.0, palette::RED, n));
                }
                p.spawn(look.hint(
                    "Every hero of a class shares its talents, its gear and the zones it has opened.",
                ));
            });
        });
    });
}

/// The 3 × 3 grid of classes.
fn grid(p: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, picked: &str) {
    p.spawn(Node {
        display: Display::Grid,
        grid_template_columns: vec![RepeatedGridTrack::px(3, look.of(CELL_WIDTH))],
        row_gap: look.px(10.0),
        justify_content: JustifyContent::Center,
        ..default()
    })
    .with_children(|g| {
        for class in &ctx.content.classes {
            let on = class.id == picked;
            g.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: look.px(4.0),
                ..default()
            })
            .with_children(|cell| {
                let sheet = ctx.art.class_icons.clone();
                let rect =
                    class_cell(class.wow_class_id).and_then(|c| cell_rect(ctx.images, &sheet, c));
                look.icon(cell, sheet, rect, CLASS_ICON, on, false, false)
                    .insert(MenuAction::PickClass(class.id.clone()));
                let name_colour = if on {
                    class_colour(&class.id).unwrap_or(palette::VALUE)
                } else {
                    palette::LABEL
                };
                cell.spawn(look.text(Face::Plain, 11.0, name_colour, class.name.clone()));
            });
        }
    });
}

/// The picked class: its name, how it plays, the value boxes and its spell pool.
fn about(p: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, class: &Class, colour: Color) {
    p.spawn(Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: look.px(4.0),
        ..default()
    })
    .with_children(|c| {
        look.small_caps(c, &class.name, 24.0, colour);
        c.spawn((
            look.text(Face::Plain, 11.0, palette::VALUE, class.description.clone()),
            TextLayout::new_with_justify(Justify::Center),
        ));
    });
    let start = ctx.content.ability(class.default_start_spell());
    p.spawn(Node {
        column_gap: look.px(8.0),
        ..default()
    })
    .with_children(|row| {
        look.value_box(
            row,
            "Health",
            &format!("{}", class.base_health.round() as i64),
            palette::VALUE,
        );
        look.value_box(
            row,
            "Starts with",
            start.map_or(class.default_start_spell(), |a| a.name.as_str()),
            palette::HEADING,
        );
        look.value_box(row, "Zones open", &zones_line(ctx, class), palette::VALUE);
    });
    p.spawn(Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: look.px(5.0),
        ..default()
    })
    .with_children(|pool| {
        pool.spawn(look.label("Spell pool"));
        pool.spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            column_gap: look.px(5.0),
            row_gap: look.px(5.0),
            ..default()
        })
        .with_children(|icons| {
            let mut spells: Vec<&str> = vec![class.default_start_spell()];
            spells.extend(
                class
                    .abilities
                    .iter()
                    .map(String::as_str)
                    .filter(|a| *a != class.default_start_spell()),
            );
            for id in spells {
                let Some(a) = ctx.content.ability(id) else {
                    continue;
                };
                let starts = id == class.default_start_spell();
                look.icon(
                    icons,
                    look.icon_art(&a.icon),
                    None,
                    SPELL_ICON,
                    starts,
                    false,
                    false,
                )
                .insert((Tip(spell_tip(a, starts)), MenuLookTip));
            }
        });
    });
}

/// Back (when the save has heroes to go back to) and Choose.
fn buttons(p: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, class: &Class) {
    p.spawn(Node {
        justify_content: JustifyContent::Center,
        column_gap: look.px(12.0),
        margin: UiRect::top(look.px(4.0)),
        ..default()
    })
    .with_children(|row| {
        if !ctx.screen.characters.is_empty() {
            look.button(row, "Back", ButtonStyle::Text, true, Some(110.0))
                .insert(MenuAction::Back);
        }
        look.button(
            row,
            &format!("Choose {}", class.name),
            ButtonStyle::Primary,
            true,
            Some(200.0),
        )
        .insert(MenuAction::ChooseClass);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spell_tip_says_its_school_and_how_often() {
        assert_eq!(seconds(8.0), "8 s");
        assert_eq!(seconds(1.5), "1.5 s");
        assert_eq!(school_word(School::Shadow), "Shadow");
    }
}
