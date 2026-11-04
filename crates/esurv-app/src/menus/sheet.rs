//! The hero sheet (menus.md, "The flow", steps 2 and 3): a new hero of
//! the class picked, or the hero picked on the warband screen ("Modify selected character"; its
//! race and look may always change). Three views, no tabs: Character,
//! and Collection and Talents ([`collection`], [`talents`]), opened from the warband screen.
//!
//! The Character tab: on the left "Appearance" (the races as portraits under Alliance and Horde,
//! the ones the class may not be dimmed; the race's name; Male and Female), "Racials" (the
//! race's spell and passives as icons) and "Equipment" (the
//! slots, each with what is worn or "Empty", and what worn gear adds); in the middle the hero's
//! name as a big heading (typed in), "sex race class" under it, and the hero on a lit floor,
//! turned by dragging or by its arrows; on the right "Features" (skin colour, face, hair style,
//! hair colour and the race's fifth feature, each with gold arrows, `n / m` and a row of dots)
//! and Randomize. Create makes the hero; Save changes keeps a modified one.

pub mod collection;
pub mod talents;

use benilla_app::CreateChoices;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use esurv_content::{CampLine, Content, Faction, LineWhen, TimePeriod};

use super::art::{cell_rect, race_cell};
use super::look::{palette, Arrow, ButtonStyle, Face, MenuLook};
use super::preview::TurnArea;
use super::{
    race_allowed, races_in_order, Ctx, Draft, MenuAction, MenuPress, Menus, Screen,
    Sheet, SheetMode, SheetTab,
};
use crate::hero_sheet::gear::{stat_line, ClassGear};
use crate::stock::{MenuLookTip, Tip, TipLine};
use crate::ui::{class_colour, rarity_colour};

/// Sizes, in units.
pub(super) const MARGIN: f32 = 16.0;
pub(super) const TOP: f32 = 58.0;
pub(super) const SIDE: f32 = 300.0;
const PORTRAIT: f32 = 40.0;
const SLOT_ICON: f32 = 26.0;
const RACIAL_ICON: f32 = 36.0;
const TEMPERAMENT_ICON: f32 = 28.0;

/// The longest name, as the stock create screen takes it.
pub const NAME_LIMIT: usize = 12;

/// The order the sheet lists the slots in: head, shoulders, chest, hands, wrists, waist, legs,
/// feet, then the weapons (the class's own slot numbers).
pub(super) const SLOT_ORDER: [u32; 8] = [0, 1, 2, 7, 6, 3, 4, 5];

/// The appearance features, in the sheet's order.
pub(super) const FEATURES: usize = 5;

/// How many choices each feature has for the draft's race and sex (1 each without the player's
/// data).
pub(super) fn feature_counts(choices: &CreateChoices, race: u8, sex: u8) -> [u8; FEATURES] {
    choices.ranges(race, sex).map_or([1; FEATURES], |r| {
        [r.skin, r.face, r.hair_style, r.hair_color, r.facial_hair].map(|n| n.max(1))
    })
}

/// A feature's name in our words, by its place and the race's glue tokens; `None` for a fifth
/// feature the race and sex do not have.
pub(super) fn feature_name(
    choices: &CreateChoices,
    race: u8,
    sex: u8,
    i: usize,
) -> Option<&'static str> {
    let horns = choices.hair_token(race) == Some("HORNS");
    Some(match i {
        0 => "Skin colour",
        1 => "Face",
        2 if horns => "Horn style",
        2 => "Hair style",
        3 if horns => "Horn colour",
        3 => "Hair colour",
        _ => match choices.feature_token(race, sex) {
            Some("NONE") => return None,
            Some("NORMAL") => "Facial hair",
            Some("PIERCINGS") => "Piercings",
            Some("EARRINGS") => "Earrings",
            Some("TUSKS") => "Tusks",
            Some("HORNS") => "Horns",
            Some("MARKINGS") => "Markings",
            Some("HAIR") => "Hair",
            _ => "Features",
        },
    })
}

/// A draft's look kept inside its race and sex's choices.
pub(super) fn clamp_look(choices: &CreateChoices, d: &mut Draft) {
    let counts = feature_counts(choices, d.race, d.sex);
    for (v, n) in d.look.iter_mut().zip(counts) {
        if *v >= n {
            *v = n - 1;
        }
    }
}

/// The sex word: "Male" or "Female".
pub(super) fn sex_word(sex: u8) -> &'static str {
    if sex == 1 {
        "Female"
    } else {
        "Male"
    }
}

/// Whether a name may be asked for: two to twelve letters.
pub(super) fn name_ready(name: &str) -> bool {
    let n = name.chars().count();
    (2..=NAME_LIMIT).contains(&n) && name.chars().all(char::is_alphabetic)
}

pub(super) fn draw(
    commands: &mut Commands,
    root: Entity,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    notice: Option<&str>,
    model: Option<Handle<Image>>,
) {
    commands.entity(root).with_children(|r| {
        // a solid backdrop, the panels' night-blue art, nothing of the camp's world through it
        r.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(palette::NIGHT_DEEP),
            Pickable::IGNORE,
            children![crate::splash_art::cut(crate::splash_art::Cut::Night, super::look::NIGHT_TEXTURE)],
        ));
        top_bar(r, look);
        // the Talents tab's trees take the left column's room
        if sheet.tab != SheetTab::Talents {
            left_column(r, look, ctx, sheet);
        }
        match sheet.tab {
            SheetTab::Character => {
                middle(r, look, ctx, sheet, model);
                features(r, look, ctx, sheet);
            }
            SheetTab::Collection => collection::draw(r, look, ctx, sheet),
            SheetTab::Talents => talents::draw(r, look, ctx, sheet),
        }
        if let Some(n) = notice {
            r.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: look.px(84.0),
                    left: look.px(MARGIN * 2.0 + SIDE),
                    right: look.px(MARGIN * 2.0 + SIDE),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|b| {
                b.spawn(look.text(Face::Plain, 12.0, palette::RED, n));
            });
        }
    });
}

/// Back on the left. No tabs; the Collection and the Talents open from
/// the warband screen's buttons, and Back goes back there.
fn top_bar(r: &mut ChildSpawnerCommands, look: &MenuLook) {
    r.spawn(Node {
        position_type: PositionType::Absolute,
        top: look.px(MARGIN),
        left: look.px(MARGIN),
        right: look.px(MARGIN),
        justify_content: JustifyContent::SpaceBetween,
        ..default()
    })
    .with_children(|bar| {
        look.button(bar, "Back", ButtonStyle::Text, true, Some(90.0))
            .insert(MenuAction::Back);
    });
}

/// The left column: on the Character tab Appearance, the race's Racials and Equipment; on the
/// Collection tab Equipment alone (its slots pick their looks).
pub(super) fn left_column(r: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, sheet: &Sheet) {
    let mut node = look.panel_node();
    node.position_type = PositionType::Absolute;
    node.left = look.px(MARGIN);
    node.top = look.px(TOP);
    node.bottom = look.px(MARGIN);
    node.width = look.px(SIDE);
    node.row_gap = look.px(if sheet.tab == SheetTab::Character { 5.0 } else { 7.0 });
    r.spawn(look.panel(node)).with_children(|p| {
        if sheet.tab == SheetTab::Character {
            look.heading(p, "Appearance", 18.0);
            appearance(p, look, ctx, &sheet.draft);
            look.heading(p, "Racials", 16.0);
            racials(p, look, ctx, &sheet.draft);
        }
        look.heading(p, "Equipment", 18.0);
        equipment(p, look, ctx, sheet);
        if sheet.tab == SheetTab::Collection {
            super::companions::sheet_box(p, look, ctx, sheet);
            super::wears::sheet_boxes(p, look, ctx, sheet);
        }
    });
}

/// The race's racials: its spell and its passives as icons in a row,
/// each telling what it is in its tooltip; the names are the client's.
fn racials(p: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, d: &Draft) {
    use crate::hero_sheet::racials::{racials, RacialKind};
    let class = ctx.content.class(&d.class);
    p.spawn(Node {
        column_gap: look.px(10.0),
        justify_content: JustifyContent::Center,
        margin: UiRect::vertical(look.px(2.0)),
        ..default()
    })
    .with_children(|row| {
        for r in racials(ctx.content, ctx.racials, d.race) {
            let mut tip = vec![TipLine::new(r.name.clone(), palette::VALUE)];
            match r.kind {
                RacialKind::Spell => {
                    tip.push(TipLine::new("Racial spell: casts itself", palette::LABEL));
                    let a = r.ability.as_deref().and_then(|id| ctx.content.ability(id));
                    let line = a.zip(class).and_then(|(a, c)| super::warband::level_one_description(c, a));
                    if let Some(line) = line {
                        tip.push(TipLine::new(line, palette::HEADING));
                    }
                }
                RacialKind::Passive => {
                    tip.push(TipLine::new("Racial passive", palette::LABEL));
                    tip.push(TipLine::new(r.text.clone(), palette::GREEN));
                }
            }
            look.icon(row, look.art(&r.icon), None, RACIAL_ICON, false, false, false)
                .insert((Tip(tip), MenuLookTip));
        }
    });
}

/// The races by faction, the race's name, the sexes.
fn appearance(p: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, d: &Draft) {
    let class = ctx
        .content
        .class(&d.class)
        .map_or(0, |c| c.wow_class_id as u8);
    for faction in [Faction::Alliance, Faction::Horde] {
        p.spawn(look.label(match faction {
            Faction::Alliance => "Alliance",
            Faction::Horde => "Horde",
        }));
        p.spawn(Node {
            column_gap: look.px(6.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            for race in races_in_order() {
                let Some(r) = ctx.content.race_by_wow_id(race) else {
                    continue;
                };
                if r.faction != faction {
                    continue;
                }
                let allowed = race_allowed(ctx.content, ctx.choices, race, class) && !ctx.appearance_shut;
                let sheet = ctx.art.race_icons.clone();
                let rect = race_cell(race, d.sex).and_then(|c| cell_rect(ctx.images, &sheet, c));
                let mut e = look.icon(row, sheet, rect, PORTRAIT, race == d.race, false, !allowed);
                e.insert((
                    Tip(vec![TipLine::new(
                        r.name.clone(),
                        if allowed {
                            palette::HEADING
                        } else {
                            palette::LABEL
                        },
                    )]),
                    MenuLookTip,
                ));
                if allowed {
                    e.insert(MenuAction::Race(race));
                }
            }
        });
    }
    let race_name = ctx
        .content
        .race_by_wow_id(d.race)
        .map_or("", |r| r.name.as_str());
    p.spawn(Node {
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|row| {
        look.small_caps(row, race_name, 18.0, palette::VALUE);
        row.spawn(Node {
            column_gap: look.px(6.0),
            ..default()
        })
        .with_children(|sexes| {
            for sex in [0u8, 1] {
                let style = if d.sex == sex {
                    ButtonStyle::Active
                } else {
                    ButtonStyle::Text
                };
                look.button(sexes, sex_word(sex), style, true, Some(66.0))
                    .insert(MenuAction::Sex(sex));
            }
        });
    });
}

/// The slots: what is worn in each, or "Empty"; on the Collection tab a slot is picked by a click
/// (its look choices); at the foot what worn gear adds.
fn equipment(p: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, sheet: &Sheet) {
    let Some(gear) = ClassGear::of(ctx.content, &sheet.draft.class) else {
        return;
    };
    let progress = ctx.facts.progress(&sheet.draft.class);
    let worn = progress.map(|pr| gear.worn(pr)).unwrap_or_default();
    let slots: Vec<u32> = SLOT_ORDER.into_iter().chain(gear.weapon_slots()).collect();
    let pick = sheet.tab == SheetTab::Collection;
    p.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: look.px(3.0),
        ..default()
    })
    .with_children(|list| {
        for slot in slots {
            let zone = worn.get(slot as usize).copied().flatten();
            let style = if pick && sheet.slot == Some(slot) {
                ButtonStyle::RowPicked
            } else {
                ButtonStyle::Row
            };
            let mut row = look.frame_button(list, style, pick, None);
            row.insert(MenuAction::Slot(slot));
            if let Some(z) = zone {
                row.insert((
                    Tip(crate::hero_sheet::gear::piece_tip(
                        &gear,
                        progress.unwrap(),
                        slot,
                        z,
                    )),
                    MenuLookTip,
                ));
            }
            row.with_children(|b| {
                b.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        column_gap: look.px(8.0),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|line| {
                    let path = match zone {
                        Some(z) => ctx.icons.piece_icon(&gear, slot, progress.map_or(gear.own_look(z), |pr| gear.own_zone_look(pr, slot, z))),
                        None => gear.empty_slot_art(slot).to_owned(),
                    };
                    line.spawn((
                        ImageNode::new(look.art(&path)),
                        Node {
                            width: look.px(SLOT_ICON),
                            height: look.px(SLOT_ICON),
                            border_radius: BorderRadius::all(look.px(2.0)),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ));
                    line.spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|t| {
                        t.spawn(look.label(gear.slot_name(slot)));
                        match zone {
                            Some(z) => t.spawn(look.text(
                                Face::Plain,
                                10.5,
                                rarity_colour(gear.quality(z)),
                                progress.map_or_else(|| gear.name(slot, z), |pr| gear.look_name(slot, gear.own_zone_look(pr, slot, z))),
                            )),
                            None => t.spawn(look.text(Face::Plain, 10.5, palette::HINT, "Empty")),
                        };
                    });
                });
            });
        }
    });
    let totals = progress.map(|pr| gear.worn_totals(pr)).unwrap_or_default();
    if totals.is_empty() {
        p.spawn(
            look.hint("Worn gear adds nothing yet. Bosses drop the class's gear, kept for good."),
        );
    } else {
        p.spawn(look.label("Worn gear adds"));
        // side by side where they fit, so the Racials above leave the list room
        p.spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: look.px(12.0),
            ..default()
        })
        .with_children(|lines| {
            for (s, v) in &totals {
                lines.spawn(look.text(Face::Plain, 10.0, palette::GREEN, stat_line(*s, *v)));
            }
        });
    }
}

/// The middle: the name, "sex race class", the hero on its lit floor, the turn arrows and the
/// way out.
fn middle(
    r: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    sheet: &Sheet,
    model: Option<Handle<Image>>,
) {
    let d = &sheet.draft;
    let class = ctx.content.class(&d.class);
    let race = ctx.content.race_by_wow_id(d.race);
    r.spawn(Node {
        position_type: PositionType::Absolute,
        left: look.px(MARGIN * 2.0 + SIDE),
        right: look.px(MARGIN * 2.0 + SIDE),
        top: look.px(TOP),
        bottom: look.px(MARGIN),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: look.px(4.0),
        ..default()
    })
    .with_children(|m| {
        // the name, typed in
        let typed = !d.name.is_empty();
        let shown = if typed {
            d.name.clone()
        } else {
            "Type a name".to_owned()
        };
        m.spawn((
            Node {
                border: UiRect::bottom(look.px(1.0)),
                padding: UiRect::axes(look.px(10.0), look.px(2.0)),
                min_width: look.px(220.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BorderColor::all(palette::GOLD),
        ))
        .with_children(|n| {
            let colour = if typed { palette::VALUE } else { palette::HINT };
            n.spawn(look.text(Face::Display, 34.0, colour, format!("{shown}|")));
        });
        m.spawn(
            look.text(
                Face::Plain,
                12.0,
                class
                    .and_then(|c| class_colour(&c.id))
                    .unwrap_or(palette::VALUE),
                format!(
                    "{} {} {}",
                    sex_word(d.sex),
                    race.map_or("", |r| r.name.as_str()),
                    class.map_or("", |c| c.name.as_str())
                ),
            ),
        );
        // the hero on its lit floor: dragging turns it
        m.spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_height: Val::Px(0.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexEnd,
                overflow: Overflow::clip(),
                ..default()
            },
            TurnArea,
            super::look::solid(),
        ))
        .with_children(|stage| {
            // a soft light falling on the hero from above
            stage.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: look.px(10.0),
                    width: look.px(420.0),
                    height: Val::Percent(96.0),
                    ..default()
                },
                BackgroundGradient::from(RadialGradient {
                    stops: vec![
                        ColorStop::percent(Color::srgba(1.0, 0.85, 0.55, 0.13), 0.0),
                        ColorStop::percent(Color::srgba(1.0, 0.85, 0.55, 0.0), 100.0),
                    ],
                    ..default()
                }),
                Pickable::IGNORE,
            ));
            // the lit floor under its feet
            stage.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: look.px(0.0),
                    width: look.px(360.0),
                    height: look.px(64.0),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundGradient::from(RadialGradient {
                    stops: vec![
                        ColorStop::percent(Color::srgba(1.0, 0.86, 0.52, 0.7), 0.0),
                        ColorStop::percent(Color::srgba(0.9, 0.6, 0.25, 0.3), 45.0),
                        ColorStop::percent(Color::srgba(0.5, 0.3, 0.1, 0.0), 100.0),
                    ],
                    ..default()
                }),
                BoxShadow::new(
                    Color::srgba(1.0, 0.7, 0.3, 0.12),
                    Val::ZERO,
                    Val::ZERO,
                    Val::ZERO,
                    look.px(24.0),
                ),
                Pickable::IGNORE,
            ));
            if let Some(image) = model {
                // the picture fills the stage's height, square, centred
                stage
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            right: Val::Px(0.0),
                            top: Val::Px(0.0),
                            bottom: Val::Px(0.0),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|frame| {
                        frame.spawn((
                            ImageNode::new(image),
                            Node {
                                height: Val::Percent(100.0),
                                aspect_ratio: Some(1.0),
                                ..default()
                            },
                            Pickable::IGNORE,
                        ));
                    });
            }
        });
        m.spawn(Node {
            column_gap: look.px(16.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|turn| {
            look.arrow(turn, Arrow::Left, 26.0, true)
                .insert(MenuAction::Turn(false));
            turn.spawn(look.hint("Drag the hero to turn it."));
            look.arrow(turn, Arrow::Right, 26.0, true)
                .insert(MenuAction::Turn(true));
        });
        let shut = ctx.appearance_shut;
        let ready = name_ready(&d.name) && !shut;
        let label = match sheet.mode {
            SheetMode::Create => "Create",
            SheetMode::Modify(_) => "Save changes",
        };
        look.button(m, label, ButtonStyle::Primary, ready, Some(220.0))
            .insert(MenuAction::ConfirmSheet);
        if shut {
            m.spawn(look.hint(AFTER_THE_TOUR));
        }
    });
}

/// The Transmutation Orb's changes (race and Save changes) are shut in the camp till the warband
/// has the Recruit title, for every hero after.
pub(super) fn appearance_shut(in_camp: bool, recruited: bool, mode: SheetMode) -> bool {
    in_camp && matches!(mode, SheetMode::Modify(_)) && !recruited
}

/// Said under the shut Save changes.
const AFTER_THE_TOUR: &str = "You can change your appearance after the tutorial!";

/// The right column: the features, Randomize.
fn features(r: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, sheet: &Sheet) {
    let d = &sheet.draft;
    let counts = feature_counts(ctx.choices, d.race, d.sex);
    let mut node = look.panel_node();
    node.position_type = PositionType::Absolute;
    node.right = look.px(MARGIN);
    node.top = look.px(TOP);
    node.bottom = look.px(MARGIN);
    node.width = look.px(SIDE);
    node.row_gap = look.px(8.0);
    r.spawn(look.panel(node)).with_children(|p| {
        look.heading(p, "Features", 18.0);
        for (i, &count) in counts.iter().enumerate() {
            let Some(name) = feature_name(ctx.choices, d.race, d.sex, i) else {
                continue;
            };
            let n = count as usize;
            let at = (d.look[i] as usize).min(n.saturating_sub(1));
            p.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: look.px(4.0),
                    padding: UiRect::all(look.px(7.0)),
                    border: UiRect::all(look.px(1.0)),
                    border_radius: BorderRadius::all(look.px(4.0)),
                    ..default()
                },
                BackgroundColor(palette::INSET),
                BorderColor::all(palette::GOLD_DIM),
            ))
            .with_children(|b| {
                b.spawn(look.label(name));
                b.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    look.arrow(row, Arrow::Left, 22.0, n > 1)
                        .insert(MenuAction::Feature {
                            index: i,
                            up: false,
                        });
                    row.spawn(look.text(
                        Face::Plain,
                        12.0,
                        palette::HEADING,
                        format!("{} / {n}", at + 1),
                    ));
                    look.arrow(row, Arrow::Right, 22.0, n > 1)
                        .insert(MenuAction::Feature { index: i, up: true });
                });
                look.dots(b, n, at);
            });
        }
        look.button(p, "Randomize", ButtonStyle::Text, true, None)
            .insert(MenuAction::Randomize);
        temperament(p, look, ctx, sheet);
        time_period(p, look, ctx.content, sheet);
        p.spawn(look.hint("Race and looks can change on any hero before a run."));
    });
}

/// The Quiet choice's icon: the client's Silence.
const QUIET_ICON: &str = "Spell_Holy_Silence";

/// The Temperament block (plans/hub-hero-life.md, section 9): the seven and Quiet as icons (the
/// first click the leading one, the next the second, a picked one let go), the pair named under
/// them with Random, and a sample line in the hero's own voice, the next at a click.
fn temperament(p: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, sheet: &Sheet) {
    let chatter = &ctx.content.camp_chatter;
    let d = &sheet.draft;
    look.heading(p, "Temperament", 16.0);
    p.spawn(Node {
        column_gap: look.px(4.0),
        justify_content: JustifyContent::Center,
        ..default()
    })
    .with_children(|row| {
        for t in &chatter.temperaments {
            let place = d.temperament.iter().position(|x| *x == t.id);
            let mut tip = vec![TipLine::new(t.name.clone(), palette::VALUE)];
            match place {
                Some(0) => tip.push(TipLine::new("Leading", palette::GOLD)),
                Some(_) => tip.push(TipLine::new("Second", palette::LABEL)),
                None => {}
            }
            tip.push(TipLine::new(t.voice.clone(), palette::LABEL));
            if let Some(line) = sample_line(ctx.content, d, std::slice::from_ref(&t.id), 0) {
                tip.push(TipLine::new(format!("\"{line}\""), palette::HEADING));
            }
            look.icon(row, look.icon_art(&t.icon), None, TEMPERAMENT_ICON, place.is_some(), false, false)
                .insert((MenuAction::Temperament(t.id.clone()), Tip(tip), MenuLookTip));
        }
        let tip = vec![
            TipLine::new("Quiet", palette::VALUE),
            TipLine::new("Says nothing at the camp.", palette::LABEL),
        ];
        look.icon(row, look.icon_art(QUIET_ICON), None, TEMPERAMENT_ICON, d.temperament.is_empty(), false, false)
            .insert((MenuAction::Quiet, Tip(tip), MenuLookTip));
    });
    let named = |i: usize| d.temperament.get(i).and_then(|id| chatter.temperament(id)).map(|t| t.name.as_str());
    let words = match (named(0), named(1)) {
        (Some(a), Some(b)) => format!("{a}, and {b}"),
        (Some(a), None) => a.to_owned(),
        _ => "Quiet".to_owned(),
    };
    p.spawn(Node {
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|row| {
        row.spawn(look.text(Face::Plain, 12.0, palette::HEADING, words));
        look.button(row, "Random", ButtonStyle::Text, true, Some(80.0))
            .insert(MenuAction::RandomTemperament);
    });
    if let Some(line) = sample_line(ctx.content, d, &d.temperament, sheet.sample) {
        look.frame_button(p, ButtonStyle::Row, true, None)
            .insert((
                MenuAction::SampleLine,
                Tip(vec![TipLine::new("Click for another line", palette::LABEL)]),
                MenuLookTip,
            ))
            .with_children(|b| {
                b.spawn((look.text(Face::Plain, 10.5, palette::VALUE, format!("\"{line}\"")), Pickable::IGNORE));
            });
    }
}

/// The Time period block: when the hero was pulled in from, as icons like the
/// Temperament's, the picked one named beside them. Flavour only: it picks the camp lines the hero
/// says of home.
fn time_period(p: &mut ChildSpawnerCommands, look: &MenuLook, content: &Content, sheet: &Sheet) {
    let picked = sheet.draft.time_period;
    look.heading(p, "Time period", 16.0);
    p.spawn(Node {
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|row| {
        row.spawn(Node {
            column_gap: look.px(4.0),
            ..default()
        })
        .with_children(|icons| {
            for period in TimePeriod::ALL {
                let (name, icon, about) = time_period_words(period);
                let tip = vec![
                    TipLine::new(name, palette::VALUE),
                    TipLine::new(content.shown(about), palette::LABEL),
                ];
                look.icon(icons, look.icon_art(icon), None, TEMPERAMENT_ICON, period == picked, false, false)
                    .insert((MenuAction::TimePeriod(period), Tip(tip), MenuLookTip));
            }
        });
        row.spawn(look.text(Face::Plain, 12.0, palette::HEADING, time_period_words(picked).0));
    });
}

/// A time period's name, icon (from `Interface\Icons`) and what it means, for the sheet.
pub(super) fn time_period_words(period: TimePeriod) -> (&'static str, &'static str, &'static str) {
    match period {
        TimePeriod::Past => (
            "Past",
            "INV_Misc_Book_11",
            "From before the classic era: the Second War, {Area(2017)} before the plague, Draenor before the Portal.",
        ),
        TimePeriod::Present => ("Present", "INV_Misc_PocketWatch_01", "From the classic era itself."),
        TimePeriod::Future => (
            "Future",
            "Spell_Nature_CrystalBall",
            "From after the classic era: the Lich King's fall, the Cataclysm and on.",
        ),
    }
}

/// A sample line for hero `d` of temperaments `temperaments`, the `k`th: the lines it may say
/// to nobody or to the player, its race's, class's and time period's own first so its voice is
/// heard.
pub(super) fn sample_line<'a>(content: &'a Content, d: &Draft, temperaments: &[String], k: usize) -> Option<&'a str> {
    let race = content.race_by_wow_id(d.race).map(|r| r.id.as_str());
    let mut lines: Vec<&CampLine> = content
        .camp_chatter
        .lines
        .iter()
        .filter(|l| {
            temperaments.contains(&l.temperament)
                && matches!(l.when, LineWhen::Idle | LineWhen::ToPlayer)
                && l.race.as_deref().is_none_or(|r| Some(r) == race)
                && l.class.as_ref().is_none_or(|c| *c == d.class)
                && l.time_period.is_none_or(|p| p == d.time_period)
        })
        .collect();
    lines.sort_by_key(|l| (l.race.is_none() && l.class.is_none() && l.time_period.is_none(), temperaments.iter().position(|t| *t == l.temperament)));
    lines.get(k % lines.len().max(1)).map(|l| l.text.as_str())
}

/// A temperament clicked: let go if picked (the second leads then), else the leading one, else
/// the second (in place of the second picked before).
pub(super) fn pick_temperament(picked: &mut Vec<String>, id: &str) {
    if let Some(i) = picked.iter().position(|t| t == id) {
        picked.remove(i);
    } else if picked.len() < 2 {
        picked.push(id.to_owned());
    } else {
        picked[1] = id.to_owned();
    }
}

/// Two temperaments at random (the plan: every pair possible for everyone).
pub(super) fn random_temperaments(content: &Content, seed: u64) -> Vec<String> {
    let ids: Vec<&String> = content.camp_chatter.temperaments.iter().map(|t| &t.id).collect();
    let n = ids.len() as u64;
    if n < 2 {
        return ids.into_iter().cloned().collect();
    }
    let mut x = seed | 1;
    let a = xorshift(&mut x) % n;
    let b = (a + 1 + xorshift(&mut x) % (n - 1)) % n;
    vec![ids[a as usize].clone(), ids[b as usize].clone()]
}

/// The clock as a seed for the sheet's random picks.
pub(super) fn seed_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(7, |d| d.as_nanos() as u64)
}

/// xorshift: enough for a look.
fn xorshift(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

/// Steps a feature, round its choices.
pub(super) fn step_feature(choices: &CreateChoices, d: &mut Draft, i: usize, up: bool) {
    let n = feature_counts(choices, d.race, d.sex)[i.min(FEATURES - 1)];
    let v = &mut d.look[i.min(FEATURES - 1)];
    *v = if up { (*v + 1) % n } else { (*v + n - 1) % n };
}

/// Every feature picked at random among its choices.
pub(super) fn randomize(choices: &CreateChoices, d: &mut Draft, seed: u64) {
    let counts = feature_counts(choices, d.race, d.sex);
    let mut x = seed | 1;
    for (v, n) in d.look.iter_mut().zip(counts) {
        // xorshift: enough for a look
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *v = (x % u64::from(n)) as u8;
    }
}

/// The name typed on the Character tab: letters only, up to twelve; Enter confirms the sheet.
pub(super) fn type_the_name(
    mut keys: MessageReader<KeyboardInput>,
    mut menus: ResMut<Menus>,
    mut presses: MessageWriter<MenuPress>,
) {
    let Screen::Sheet(sheet) = &mut menus.screen else {
        keys.clear();
        return;
    };
    if sheet.tab != SheetTab::Character {
        keys.clear();
        return;
    }
    let mut name = sheet.draft.name.clone();
    for k in keys.read() {
        if !k.state.is_pressed() {
            continue;
        }
        match &k.logical_key {
            Key::Backspace => {
                name.pop();
            }
            Key::Enter => {
                presses.write(MenuPress(MenuAction::ConfirmSheet));
            }
            Key::Character(s) => {
                for c in s.chars().filter(|c| c.is_alphabetic()) {
                    if name.chars().count() < NAME_LIMIT {
                        name.push(c);
                    }
                }
            }
            _ => {}
        }
    }
    if name != sheet.draft.name {
        sheet.draft.name = name;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In the camp the Orb's changes stay shut till the Recruit title, and open after it for
    /// every hero; a new hero's creation and the screens outside the camp are never shut.
    #[test]
    fn the_orb_opens_with_the_recruit_title() {
        let modify = SheetMode::Modify(7);
        assert!(appearance_shut(true, false, modify), "shut before the title");
        assert!(!appearance_shut(true, true, modify), "open after it");
        assert!(!appearance_shut(true, false, SheetMode::Create));
        assert!(!appearance_shut(false, false, modify));
    }

    #[test]
    fn a_name_is_two_to_twelve_letters() {
        assert!(name_ready("Ana"));
        assert!(!name_ready("A"));
        assert!(!name_ready("Abcdefghijklm"));
        assert!(!name_ready("Ana1"));
    }

    #[test]
    fn features_step_round_and_stay_inside() {
        let choices = CreateChoices::default();
        let mut d = Draft {
            class: "mage".into(),
            race: 1,
            sex: 0,
            look: [0; 5],
            name: String::new(),
            temperament: Vec::new(),
            time_period: TimePeriod::Present,
        };
        step_feature(&choices, &mut d, 2, false);
        assert_eq!(d.look[2], 0, "one choice without the data");
        randomize(&choices, &mut d, 42);
        assert_eq!(d.look, [0; 5]);
        assert_eq!(feature_name(&choices, 1, 0, 4), Some("Features"));
        assert_eq!(feature_name(&choices, 1, 0, 2), Some("Hair style"));
    }

    #[test]
    fn temperaments_pick_the_leading_then_the_second() {
        let mut t = Vec::new();
        pick_temperament(&mut t, "grim");
        pick_temperament(&mut t, "weary");
        assert_eq!(t, ["grim", "weary"]);
        pick_temperament(&mut t, "curious");
        assert_eq!(t, ["grim", "curious"], "a third takes the second's place");
        pick_temperament(&mut t, "grim");
        assert_eq!(t, ["curious"], "the second leads once the leading one is let go");
    }
}
