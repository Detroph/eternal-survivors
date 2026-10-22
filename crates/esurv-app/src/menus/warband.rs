//! Character select, the warband screen (menus.md, "The flow", step 4),
//! over the camps of [`crate::picker`]:
//!
//! * the camp fills the lower left: its campfire, the picked hero with its nameplate (class icon
//!   and name) over its head, clear of the panels; a round button under the top-left panel that
//!   switches camps (a badge with how many heroes the other camp holds; it follows the panel's
//!   height, so it never covers it); and the buttons New character, Modify character and Delete
//!   character (which asks first);
//! * the bottom-left corner: Options (the game's settings, [`super::options`]) over Quit game;
//! * the top-left panel: the picked hero's name, class and total wins, its starting spell, and
//!   under it its class's weapon loadout (a row of three icons as the starting spell's, each
//!   telling its weapons, strength and drawback in its tooltip; roadmap
//!   R3);
//! * the top middle, centred over the camp: Collection and Talents (each opening the hero sheet
//!   there, glowing while it has news) and two places kept for later, over the armour the hero
//!   wears;
//! * the top right, beside the zone column: the codex, how much of each list is found, and the
//!   way into it ([`super::codex`]);
//! * the right column: the zone selector (the chosen zone as a card, every other zone as a row,
//!   a locked one saying what opens it), Difficulty modifiers and Start run.

use benilla_protocol::Character;
use bevy::prelude::*;
use esurv_content::{Content, Faction, Zone};
use esurv_sim::game::gear::ARMOR_SLOTS;

use super::look::{palette, solid, ButtonStyle, Face, MenuLook, PAD};
use super::scroll::{scrolling, ScrollingList};
use super::art::{cell_rect, class_cell};
use super::{class_of, Ctx, MenuAction, SheetTab};
use crate::hero_sheet::gear::{minute_text, origin_line, ClassGear};
use crate::picker::faction_of;
use crate::stock::{MenuLookTip, Tip, TipLine};
use crate::ui::{class_colour, rarity_colour};
use crate::zone_discovery::{is_discovered, LOCK_ICON, UNDISCOVERED};

/// Sizes, in units.
const MARGIN: f32 = 16.0;
const INFO_WIDTH: f32 = 290.0;
const ZONE_WIDTH: f32 = 350.0;
const SLOT_ICON: f32 = 34.0;
/// A starting option's icon on the info panel.
const START_ICON: f32 = 40.0;
const CAMP_SWITCH: f32 = 46.0;
const CAMP_BUTTON: f32 = 150.0;
const CORNER_BUTTON: f32 = 110.0;
/// The codex panel beside the zone column.
pub(super) const CODEX_WIDTH: f32 = 210.0;

/// The picked hero's nameplate (its class icon, then its name), kept over its figure's head as
/// the camera sees it.
#[derive(Component)]
pub(super) struct NameOverHead {
    /// The hero it names, by roster guid.
    guid: u64,
}

/// Marks a panel the nameplate keeps clear of: under one above the head, beside one at its side.
#[derive(Component)]
pub(super) struct KeepNameClear;

/// The nameplate's class icon, in units.
const PLATE_ICON: f32 = 22.0;
/// How far over the figure's head (its helm's seat) the plate's foot sits, yards: clear of the heads
/// around the fire.
const PLATE_LIFT: f32 = 0.5;
/// Room kept between the plate and a panel or the screen's edge, logical pixels.
const PLATE_ROOM: f32 = 6.0;

/// The heroes of the camp shown, in roster order.
fn camp<'a>(ctx: &Ctx<'a>) -> Vec<&'a Character> {
    ctx.screen
        .characters
        .iter()
        .filter(|c| faction_of(ctx.content, c) == Some(ctx.camp))
        .collect()
}

/// The picked hero, when it stands in the camp shown.
fn picked<'a>(ctx: &Ctx<'a>) -> Option<&'a Character> {
    ctx.screen
        .selected_character()
        .filter(|c| faction_of(ctx.content, c) == Some(ctx.camp))
}

/// The zone (its index) the hero's next run is in, as the save says.
pub(super) fn hero_zone(ctx: &Ctx, hero: &Character) -> usize {
    ctx.facts
        .hero(hero.guid)
        .and_then(|h| ctx.content.zones.iter().position(|z| z.id == h.zone))
        .unwrap_or(0)
}

/// How many zones the hero's class has open (the first always is).
fn zones_open(ctx: &Ctx, hero: &Character) -> usize {
    class_of(ctx.content, hero.class)
        .and_then(|c| ctx.facts.progress(&c.id))
        .map_or(1, |p| p.zones_open.max(1) as usize)
}

/// What opens zone `i` (past the first): a win in the zone before it, against its final boss.
pub(super) fn opener(content: &Content, i: usize) -> String {
    let Some(before) = i.checked_sub(1).and_then(|b| content.zones.get(b)) else {
        return String::new();
    };
    let boss = before
        .bosses
        .iter()
        .find(|b| b.is_final)
        .map_or("its final boss", |b| b.name.as_str());
    format!("Defeat {boss} in {} to open it.", before.name)
}

/// One zone as the selector lists it.
#[derive(Debug)]
pub(super) struct ZoneChoice<'a> {
    pub zone: &'a Zone,
    /// Its index in the zones' difficulty order.
    pub index: usize,
    /// Open to the hero's class: picked by a click.
    pub open: bool,
    /// The zone the hero's next run is in: shown opened as a card.
    pub chosen: bool,
    /// Discovered by the save: until then the row shows only "???" and
    /// a lock.
    pub discovered: bool,
}

/// Every zone of the content, in its difficulty order, for a class with `open` zones open and
/// zone `chosen` picked, the save having discovered the zones `discovered` (ids): a zone added to
/// the content takes its place here with no change.
pub(super) fn zone_choices<'c>(
    content: &'c Content,
    chosen: usize,
    open: usize,
    discovered: &[String],
) -> Vec<ZoneChoice<'c>> {
    let choice = |(index, zone)| ZoneChoice {
        zone,
        index,
        open: index < open.max(1),
        chosen: index == chosen,
        discovered: is_discovered(content, discovered, open.max(1), index),
    };
    content.zones.iter().enumerate().map(choice).collect()
}

pub(super) fn draw(commands: &mut Commands, root: Entity, look: &MenuLook, ctx: &Ctx) {
    let hero = picked(ctx);
    commands.entity(root).with_children(|r| {
        top_left(r, look, ctx, hero);
        centre_column(r, look, ctx, hero);
        zone_column(r, look, ctx, hero);
        codex_panel(r, look, ctx);
        camp_parts(r, look, ctx, hero);
        if let Some(h) = hero {
            name_over_head(r, look, ctx, h);
        }
    });
}

/// The top-left column: the info panel, and under it the round camp switch, which so follows
/// the panel's height and never covers it.
fn top_left(r: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, hero: Option<&Character>) {
    r.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: look.px(MARGIN),
            top: look.px(MARGIN),
            width: look.px(INFO_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            row_gap: look.px(14.0),
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|col| {
        info_panel(col, look, ctx, hero);
        camp_switch(col, look, ctx);
    });
}

/// The top-left panel: who the picked hero is.
fn info_panel(r: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, hero: Option<&Character>) {
    let mut node = look.panel_node();
    node.width = Val::Percent(100.0);
    node.row_gap = look.px(8.0);
    r.spawn((look.panel(node), KeepNameClear)).with_children(|p| {
        let Some(h) = hero else {
            look.heading(p, "Warband", 20.0);
            p.spawn(look.value(match ctx.camp {
                Faction::Horde => "No hero waits by the Horde's campfire yet.",
                Faction::Alliance => "No hero waits by the Alliance's campfire yet.",
            }));
            p.spawn(look.hint("Create a hero of one of its races to see it wait here."));
            return;
        };
        let class = class_of(ctx.content, h.class);
        let race = ctx.content.race_by_wow_id(h.race);
        look.small_caps(p, &h.name, 26.0, palette::VALUE);
        p.spawn(
            look.text(
                Face::Plain,
                12.0,
                class
                    .and_then(|c| class_colour(&c.id))
                    .unwrap_or(palette::VALUE),
                format!(
                    "{} {}",
                    race.map_or("", |r| r.name.as_str()),
                    class.map_or("", |c| c.name.as_str())
                ),
            ),
        );
        let facts = ctx.facts.hero(h.guid);
        p.spawn(Node {
            column_gap: look.px(8.0),
            ..default()
        })
        .with_children(|row| {
            look.value_box(
                row,
                "Total wins",
                &facts.map_or("…".into(), |f| f.wins.to_string()),
                palette::HEADING,
            );
            look.value_box(
                row,
                "Runs",
                &facts.map_or("…".into(), |f| f.runs.to_string()),
                palette::VALUE,
            );
        });
        starting_spell(p, look, ctx, h);
        weapon_loadout(p, look, ctx, h);
    });
}

/// The weapons a loadout holds, as its card names them ("One-handed sword, Shield").
pub(crate) fn loadout_weapons(l: &esurv_content::Loadout) -> String {
    l.weapons.iter().map(|w| w.name.as_str()).collect::<Vec<_>>().join(", ")
}

/// One icon of the Weapon loadout row: the loadout and whether the class fights with it.
#[derive(Debug)]
pub(crate) struct LoadoutIcon<'a> {
    pub loadout: &'a esurv_content::Loadout,
    pub chosen: bool,
}

/// The Weapon loadout row of a class, in its loadouts' order: the one the save names lit, else
/// the class's first (a name the class lacks falls back to it too).
pub(crate) fn loadout_icons<'a>(content: &'a Content, class: &str, told: Option<&str>) -> Vec<LoadoutIcon<'a>> {
    let chosen = content.loadout(class, told.filter(|s| !s.is_empty())).map(|l| l.id.as_str());
    content
        .loadouts_of(class)
        .iter()
        .map(|l| LoadoutIcon { loadout: l, chosen: chosen == Some(l.id.as_str()) })
        .collect()
}

/// A loadout's tooltip, as the Starting spell's: its name, its weapons, its strength (green) and
/// its drawback (red).
pub(crate) fn loadout_tip(l: &esurv_content::Loadout) -> Vec<TipLine> {
    let mut lines = vec![
        TipLine::new(l.name.clone(), palette::VALUE),
        TipLine::new(loadout_weapons(l), palette::HEADING),
    ];
    if !l.strength.trim().is_empty() {
        lines.push(TipLine::new(l.strength.clone(), palette::GREEN));
    }
    if !l.drawback.trim().is_empty() {
        lines.push(TipLine::new(l.drawback.clone(), palette::RED));
    }
    lines
}

/// The info panel's Weapon loadout spot, under the Starting spell and built as it: the class's
/// three loadouts as icons, the one the class fights with lit and named under the row, each
/// telling the rest in its tooltip; a click picks another, which the save keeps for the class's
/// next runs (roadmap R3).
fn weapon_loadout(p: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, hero: &Character) {
    let Some(class) = class_of(ctx.content, hero.class) else {
        return;
    };
    let told = ctx.facts.progress(&class.id).map(|p| p.loadout.as_str());
    let icons = loadout_icons(ctx.content, &class.id, told);
    if icons.is_empty() {
        return;
    }
    p.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(look.px(8.0), look.px(5.0)),
            border: UiRect::all(look.px(1.0)),
            border_radius: BorderRadius::all(look.px(4.0)),
            row_gap: look.px(4.0),
            ..default()
        },
        BorderColor::all(palette::GOLD_DIM),
    ))
    .with_children(|b| {
        b.spawn(look.label("Weapon loadout"));
        b.spawn(Node {
            column_gap: look.px(6.0),
            ..default()
        })
        .with_children(|row| {
            for i in &icons {
                let l = i.loadout;
                look.icon(row, look.icon_art(&l.icon), None, START_ICON, i.chosen, false, false)
                    .insert((Tip(loadout_tip(l)), MenuLookTip, MenuAction::PickLoadout(l.id.clone())));
            }
        });
        if let Some(i) = icons.iter().find(|i| i.chosen) {
            b.spawn(look.value(i.loadout.name.clone()));
        }
    });
}

/// The info panel's Starting spell spot: the class's starting options as icons (three, the
/// Druid's four), the hero's pick lit; a click picks another, which the save keeps for its next
/// runs.
fn starting_spell(p: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, hero: &Character) {
    let Some(class) = class_of(ctx.content, hero.class) else {
        return;
    };
    let picked = ctx.facts.hero(hero.guid).map(|f| f.start.as_str()).filter(|s| !s.is_empty());
    let chosen = class.starting_option(picked);
    p.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(look.px(8.0), look.px(5.0)),
            border: UiRect::all(look.px(1.0)),
            border_radius: BorderRadius::all(look.px(4.0)),
            row_gap: look.px(4.0),
            ..default()
        },
        BorderColor::all(palette::GOLD_DIM),
    ))
    .with_children(|b| {
        b.spawn(look.label("Starting spell"));
        b.spawn(Node {
            column_gap: look.px(6.0),
            ..default()
        })
        .with_children(|row| {
            for o in &class.starting_options {
                let on = chosen.is_some_and(|c| c.id == o.id);
                // the option's own icon, else its main spell's
                let icon = match o.icon.as_str() {
                    "" => ctx.content.ability(o.main_spell()).map_or("", |a| a.icon.as_str()),
                    own => own,
                };
                look.icon(row, look.icon_art(icon), None, START_ICON, on, false, false)
                    .insert((
                        Tip(option_tip(ctx.content, class, o)),
                        MenuLookTip,
                        MenuAction::PickStart(o.id.clone()),
                    ));
            }
        });
        if let Some(o) = chosen {
            b.spawn(look.value(o.name.clone()));
        }
    });
}

/// A spell's description with its numbers for a hero of `class` at level 1, rank 1.
pub(super) fn level_one_description(
    class: &esurv_content::Class,
    a: &esurv_content::Ability,
) -> Option<String> {
    use esurv_protocol::OfferValue;
    use esurv_sim::game::spell_numbers::spell_numbers;
    use esurv_sim::game::spells::SpellSlot;
    let stats = esurv_sim::game::stats::compute(class, 1, &[]);
    let inst = SpellSlot::new(a, &[]).with_damage_scale(class.spell_damage_scale).instance();
    let numbers: Vec<OfferValue> = spell_numbers(a, &inst, &stats)
        .into_iter()
        .map(|(what, value)| OfferValue { what: what.into(), value })
        .collect();
    crate::cards::card_text::description(a, &numbers).map(|l| l.text())
}

/// A starting option's tooltip: its name, each of its level-1 spells with its numbers at level 1,
/// how it plays, and its passive.
pub(super) fn option_tip(
    content: &Content,
    class: &esurv_content::Class,
    option: &esurv_content::StartingOption,
) -> Vec<TipLine> {
    let mut lines = vec![TipLine::new(option.name.clone(), palette::VALUE)];
    let described = |a: &esurv_content::Ability| level_one_description(class, a);
    for a in option.spells.iter().filter_map(|s| content.ability(s)) {
        lines.push(TipLine::new(a.name.clone(), palette::HEADING));
        if let Some(line) = described(a) {
            lines.push(TipLine::new(line, palette::HEADING));
        }
        // what its strikes leave on the enemy (Mutilate's poison), with its own numbers
        for b in strike_spells(a).iter().filter_map(|s| content.ability(s)) {
            if let Some(line) = described(b) {
                lines.push(TipLine::new(format!("{}: {line}", b.name), palette::HEADING));
            }
        }
    }
    if !option.description.trim().is_empty() {
        lines.push(TipLine::new(option.description.clone(), palette::VALUE));
    }
    if !option.passive.name.is_empty() {
        lines.push(TipLine::new(format!("Passive: {}", option.passive.name), palette::GREEN));
    }
    if !option.passive.description.trim().is_empty() {
        lines.push(TipLine::new(option.passive.description.clone(), palette::GREEN));
    }
    // what the passive casts and summons, each with its numbers
    let shown: Vec<&str> = option.spells.iter().map(String::as_str).collect();
    let mut told: Vec<&str> = Vec::new();
    for id in option.all_spells() {
        if shown.contains(&id) || told.contains(&id) {
            continue;
        }
        told.push(id);
        let Some(b) = content.ability(id) else {
            continue;
        };
        if let Some(line) = described(b) {
            lines.push(TipLine::new(format!("{}: {line}", b.name), palette::GREEN));
        }
    }
    lines
}

/// The spells a spell's strikes leave or bring (a poison, the creatures it summons).
fn strike_spells(a: &esurv_content::Ability) -> Vec<String> {
    use esurv_content::Mechanic;
    match &a.kit.mechanic {
        Some(Mechanic::Poisons { poison }) => vec![poison.clone()],
        Some(Mechanic::Summons(spec)) => spec.spells.clone(),
        _ => Vec::new(),
    }
}

/// The top middle: a row of buttons over the armour the hero's class
/// wears, centred over the camp, on the axis of the camp's own buttons (the band between the hero
/// panel and as much room again on the right, which keeps clear of the codex).
fn centre_column(
    r: &mut ChildSpawnerCommands,
    look: &MenuLook,
    ctx: &Ctx,
    hero: Option<&Character>,
) {
    let Some(h) = hero else {
        return;
    };
    let Some(class) = class_of(ctx.content, h.class) else {
        return;
    };
    r.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: look.px(MARGIN),
            left: look.px(MARGIN * 2.0 + INFO_WIDTH),
            right: look.px(MARGIN * 3.0 + ZONE_WIDTH + INFO_WIDTH),
            justify_content: JustifyContent::Center,
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|band| {
        band.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                row_gap: look.px(6.0),
                min_width: look.px(SHEET_BUTTON * 4.0 + 18.0),
                ..default()
            },
            KeepNameClear,
            Pickable::IGNORE,
        ))
        .with_children(|col| {
            sheet_buttons(col, look, ctx, &class.id);
            armour(col, look, ctx, &class.id);
        });
    });
}

/// A button of the row over the armour, in units.
const SHEET_BUTTON: f32 = 96.0;

/// Whether the warband screen's Collection and Talents buttons glow:
/// gear unlocked for the class since its Collection was last opened, talent points earned since
/// its Talents were.
pub(super) fn sheet_news(ctx: &Ctx, class: &str) -> (bool, bool) {
    ctx.facts
        .progress(class)
        .map_or((false, false), |p| (p.new_gear, p.new_points))
}

/// The row over the armour: Collection and Talents (each opening the hero sheet there, glowing
/// with news), and two places kept for what comes later.
fn sheet_buttons(col: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, class: &str) {
    let (gear, points) = sheet_news(ctx, class);
    col.spawn((
        Node {
            column_gap: look.px(6.0),
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|row| {
        let glow = |news: bool| {
            if news {
                ButtonStyle::TextGlow
            } else {
                ButtonStyle::Text
            }
        };
        let buttons = [
            ("Collection", glow(gear), Some(SheetTab::Collection)),
            ("Talents", glow(points), Some(SheetTab::Talents)),
            ("TBD", ButtonStyle::Text, None),
            ("TBD", ButtonStyle::Text, None),
        ];
        for (label, style, tab) in buttons {
            let mut b = look.button(row, label, style, tab.is_some(), None);
            b.insert(Node {
                flex_grow: 1.0,
                flex_basis: Val::ZERO,
                padding: UiRect::axes(look.px(10.0), look.px(6.0)),
                border: UiRect::all(look.px(super::look::BORDER)),
                border_radius: BorderRadius::all(look.px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            });
            if let Some(t) = tab {
                b.insert(MenuAction::OpenHeroTab(t));
            }
        }
    });
}

/// The armour the hero's class wears.
fn armour(col: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, class: &str) {
    let Some(gear) = ClassGear::of(ctx.content, class) else {
        return;
    };
    let worn = ctx
        .facts
        .progress(class)
        .map(|p| gear.worn(p))
        .unwrap_or_default();
    {
        let mut node = look.panel_node();
        node.align_items = AlignItems::Center;
        node.padding = UiRect::axes(look.px(PAD), look.px(8.0));
        col.spawn(look.panel(node)).with_children(|p| {
            p.spawn(look.label("Equipped armour"));
            p.spawn(Node {
                column_gap: look.px(5.0),
                ..default()
            })
            .with_children(|row| {
                for slot in 0..gear.slots() {
                    let zone = worn.get(slot as usize).copied().flatten();
                    let (path, tip) = match zone {
                        Some(z) => (
                            ctx.icons.piece_icon(&gear, slot, gear.own_look(z)),
                            vec![
                                TipLine::new(gear.name(slot, z), rarity_colour(gear.quality(z))),
                                TipLine::new(gear.slot_name(slot), palette::VALUE),
                                TipLine::new(origin_line(&gear, ctx.facts.progress(class), z), palette::VALUE),
                            ],
                        ),
                        None => (
                            gear.empty_slot_art(slot).to_owned(),
                            vec![
                                TipLine::new(gear.slot_name(slot), palette::VALUE),
                                TipLine::new("Nothing worn", palette::LABEL),
                            ],
                        ),
                    };
                    let picked = false;
                    look.icon(row, look.art(&path), None, SLOT_ICON, picked, false, false)
                        .insert((Tip(tip), MenuLookTip));
                    if slot + 1 == ARMOR_SLOTS {
                        row.spawn(Node {
                            width: look.px(6.0),
                            ..default()
                        });
                    }
                }
            });
        });
    }
}

/// The right column: the zones, Difficulty modifiers and Start run.
fn zone_column(r: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, hero: Option<&Character>) {
    let mut node = look.panel_node();
    node.position_type = PositionType::Absolute;
    node.right = look.px(MARGIN);
    node.top = look.px(MARGIN);
    node.bottom = look.px(MARGIN);
    node.width = look.px(ZONE_WIDTH);
    node.row_gap = look.px(6.0);
    r.spawn((look.panel(node), KeepNameClear)).with_children(|p| {
        look.heading(p, "Zone", 22.0);
        let (chosen, open) = match hero {
            Some(h) => (hero_zone(ctx, h), zones_open(ctx, h)),
            None => (0, 1),
        };
        // every zone of the content, in difficulty order; more than the column shows scroll
        p.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: look.px(4.0),
                flex_grow: 1.0,
                min_height: Val::ZERO,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            scrolling(ScrollingList::Zones),
        ))
        .with_children(|list| {
            let discovered = ctx.facts.zones_discovered();
            for z in zone_choices(ctx.content, chosen, open, discovered) {
                if !z.discovered {
                    undiscovered_row(list, look);
                } else if z.chosen {
                    zone_card(list, look, z.zone, z.index);
                } else {
                    zone_row(
                        list,
                        look,
                        ctx.content,
                        z.zone,
                        z.index,
                        z.open,
                        hero.is_some(),
                    );
                }
            }
        });
        // the run's options and the way in
        let options = ctx.facts.run_options();
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: look.px(6.0),
            ..default()
        })
        .with_children(|foot| {
            let bonus = options.map(|(_, b)| b).unwrap_or(1.0);
            let label = if (bonus - 1.0).abs() < 0.005 {
                "Difficulty modifiers".to_owned()
            } else {
                format!("Difficulty modifiers  (rewards ×{bonus:.2})")
            };
            look.button(foot, &label, ButtonStyle::Text, true, None)
                .insert(MenuAction::OpenRunOptions);
            let can_start = hero.is_some_and(|h| hero_zone(ctx, h) < zones_open(ctx, h));
            look.button(foot, "Start run", ButtonStyle::Primary, can_start, None)
                .insert(MenuAction::StartRun);
        });
    });
}

/// The chosen zone, opened as a card: its name, its title, a few lines about it, its enemies and
/// its bosses with their arrival times.
fn zone_card(list: &mut ChildSpawnerCommands, look: &MenuLook, zone: &Zone, i: usize) {
    list.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: look.px(4.0),
            padding: UiRect::all(look.px(10.0)),
            border: UiRect::all(look.px(1.0)),
            border_radius: BorderRadius::all(look.px(5.0)),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(palette::INSET),
        BorderColor::all(palette::GOLD),
        BoxShadow::new(
            palette::GLOW.with_alpha(0.3),
            Val::ZERO,
            Val::ZERO,
            look.px(1.0),
            look.px(6.0),
        ),
    ))
    .with_children(|c| {
        c.spawn(look.text(
            Face::Display,
            19.0,
            palette::HEADING,
            format!("{}. {}", i + 1, zone.name),
        ));
        c.spawn(look.label(&zone.subtitle));
        c.spawn(look.text(Face::Plain, 10.0, palette::VALUE, zone.description.clone()));
        c.spawn(look.label("Enemies"));
        let mut names: Vec<&str> = Vec::new();
        for e in &zone.enemies {
            if !names.contains(&e.name.as_str()) {
                names.push(&e.name);
            }
        }
        c.spawn(look.text(Face::Plain, 10.0, palette::VALUE, names.join(", ")));
        c.spawn(look.label("Bosses"));
        for b in &zone.bosses {
            let line = if b.is_final {
                format!("{}  {} (final)", minute_text(b.spawn_minute), b.name)
            } else {
                format!("{}  {}", minute_text(b.spawn_minute), b.name)
            };
            c.spawn(look.text(
                Face::Plain,
                10.0,
                if b.is_final {
                    palette::HEADING
                } else {
                    palette::VALUE
                },
                line,
            ));
        }
    });
}

/// Another zone, as a row: open ones are picked by a click, locked ones say what opens them.
fn zone_row(
    list: &mut ChildSpawnerCommands,
    look: &MenuLook,
    content: &Content,
    zone: &Zone,
    i: usize,
    open: bool,
    has_hero: bool,
) {
    let mut row = look.frame_button(list, ButtonStyle::Row, open && has_hero, None);
    row.insert(MenuAction::PickZone(zone.id.clone()));
    row.with_children(|b| {
        b.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                row_gap: look.px(1.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|t| {
            t.spawn(look.text(
                Face::Plain,
                11.0,
                if open { palette::VALUE } else { palette::LABEL },
                format!("{}. {}", i + 1, zone.name),
            ));
            if !open {
                t.spawn(look.hint(opener(content, i)));
            }
        });
    });
}

/// A zone no class of the save has opened yet: a lock and "???", and
/// nothing else, not even its place in the order.
fn undiscovered_row(list: &mut ChildSpawnerCommands, look: &MenuLook) {
    let mut row = look.frame_button(list, ButtonStyle::Row, false, None);
    row.with_children(|b| {
        b.spawn((
            Node {
                column_gap: look.px(6.0),
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|t| {
            look.icon(t, look.art(LOCK_ICON), None, 16.0, false, true, false)
                .insert(Pickable::IGNORE);
            t.spawn(look.text(Face::Plain, 11.0, palette::LABEL, UNDISCOVERED));
        });
    });
}

/// The round button under the info panel that switches camps, a badge on it with how many
/// heroes the other camp holds.
fn camp_switch(r: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx) {
    let others = ctx
        .screen
        .characters
        .iter()
        .filter(|c| faction_of(ctx.content, c).is_some_and(|f| f != ctx.camp))
        .count();
    // the round switch, the other camp's emblem in it
    let emblem = match ctx.camp {
        Faction::Horde => r"Interface\TargetingFrame\UI-PVP-Alliance",
        Faction::Alliance => r"Interface\TargetingFrame\UI-PVP-Horde",
    };
    let switch_label = match ctx.camp {
        Faction::Horde => "Show the Alliance's camp",
        Faction::Alliance => "Show the Horde's camp",
    };
    r.spawn((
        Node {
            margin: UiRect::left(look.px(6.0)),
            width: look.px(CAMP_SWITCH),
            height: look.px(CAMP_SWITCH),
            border: UiRect::all(look.px(2.0)),
            border_radius: BorderRadius::MAX,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        super::look::MenuButton {
            style: ButtonStyle::Icon,
            enabled: true,
        },
        MenuAction::SwitchCamp,
        Tip(vec![
            TipLine::new(switch_label, palette::HEADING),
            TipLine::new(
                format!(
                    "{others} {} there.",
                    if others == 1 {
                        "hero waits"
                    } else {
                        "heroes wait"
                    }
                ),
                palette::VALUE,
            ),
        ]),
        MenuLookTip,
    ))
    .with_children(|s| {
        s.spawn((
            {
                // the emblem fills the sheet's top left
                let mut img = ImageNode::new(look.art(emblem));
                img.rect = Some(Rect::new(0.0, 0.0, 40.0, 40.0));
                img
            },
            Node {
                width: look.px(CAMP_SWITCH * 0.9),
                height: look.px(CAMP_SWITCH * 0.9),
                ..default()
            },
            Pickable::IGNORE,
        ));
        // the badge: how many heroes the other camp holds
        s.spawn((
            Node {
                position_type: PositionType::Absolute,
                right: look.px(-6.0),
                top: look.px(-6.0),
                min_width: look.px(18.0),
                height: look.px(18.0),
                padding: UiRect::horizontal(look.px(4.0)),
                border: UiRect::all(look.px(1.0)),
                border_radius: BorderRadius::MAX,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(palette::ACTIVE),
            BorderColor::all(palette::GOLD),
            Pickable::IGNORE,
        ))
        .with_children(|b| {
            b.spawn(look.text(Face::Narrow, 11.0, palette::VALUE, others.to_string()));
        });
    });
}

/// The camp's own parts: its buttons along its foot, and the corner's Options and Quit game.
fn camp_parts(r: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, hero: Option<&Character>) {
    // the buttons along the camp's foot
    r.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: look.px(MARGIN),
            right: look.px(MARGIN * 2.0 + ZONE_WIDTH),
            bottom: look.px(MARGIN),
            justify_content: JustifyContent::Center,
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|band| {
        let mut node = look.panel_node();
        node.flex_direction = FlexDirection::Row;
        node.column_gap = look.px(10.0);
        node.padding = UiRect::all(look.px(8.0));
        band.spawn(look.panel(node)).with_children(|row| {
            // New, Modify, Delete, in this order
            let has = hero.is_some();
            let room = camp(ctx).len() < esurv_protocol::MAX_HEROES_PER_FACTION
                && ctx.screen.characters.len() < esurv_protocol::MAX_HEROES;
            look.button(
                row,
                "New character",
                ButtonStyle::Text,
                room,
                Some(CAMP_BUTTON),
            )
            .insert(MenuAction::CreateHero);
            look.button(
                row,
                "Modify character",
                ButtonStyle::Text,
                has,
                Some(CAMP_BUTTON),
            )
            .insert(MenuAction::ModifyHero);
            look.button(
                row,
                "Delete character",
                ButtonStyle::Text,
                has,
                Some(CAMP_BUTTON),
            )
            .insert(MenuAction::DeleteHero);
        });
    });
    // the game's own ways out, at the very bottom left: Options over Quit game
    r.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: look.px(MARGIN),
            bottom: look.px(MARGIN),
            flex_direction: FlexDirection::Column,
            row_gap: look.px(6.0),
            ..default()
        },
        solid(),
    ))
    .with_children(|corner| {
        look.button(
            corner,
            "Options",
            ButtonStyle::Text,
            true,
            Some(CORNER_BUTTON),
        )
        .insert(MenuAction::OpenGameOptions);
        look.button(
            corner,
            "Quit game",
            ButtonStyle::Text,
            true,
            Some(CORNER_BUTTON),
        )
        .insert(MenuAction::QuitGame);
    });
}

/// The question before a hero is deleted: No (the default) and Yes.
pub(super) fn draw_delete_confirm(
    commands: &mut Commands,
    root: Entity,
    look: &MenuLook,
    name: &str,
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
            GlobalZIndex(super::MENU_LAYER + 10),
        ))
        .with_children(|veil| {
            let mut node = look.panel_node();
            node.width = look.px(340.0);
            node.align_items = AlignItems::Center;
            node.row_gap = look.px(12.0);
            node.padding = UiRect::all(look.px(18.0));
            veil.spawn(look.panel(node)).with_children(|p| {
                look.heading(p, "Delete", 20.0);
                p.spawn((
                    look.value("Are you sure you want to delete this character?"),
                    TextLayout::new_with_justify(Justify::Center),
                ));
                p.spawn(look.text(Face::Display, 18.0, palette::HEADING, name));
                p.spawn(Node {
                    column_gap: look.px(14.0),
                    ..default()
                })
                .with_children(|row| {
                    look.button(row, "No", ButtonStyle::Active, true, Some(100.0))
                        .insert(MenuAction::ConfirmNo);
                    look.button(row, "Yes", ButtonStyle::Text, true, Some(100.0))
                        .insert(MenuAction::ConfirmYes);
                });
            });
        });
    });
}

/// The picked hero's nameplate: its class icon (the class picker's), then its name in gold, kept
/// over its figure's head by [`place_name_over_head`] (menus.md).
fn name_over_head(r: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx, hero: &Character) {
    let sheet = ctx.art.class_icons.clone();
    let rect = class_cell(u32::from(hero.class)).and_then(|c| cell_rect(ctx.images, &sheet, c));
    r.spawn((
        NameOverHead { guid: hero.guid },
        Node {
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: look.px(5.0),
            ..default()
        },
        Pickable::IGNORE,
        Visibility::Hidden,
    ))
    .with_children(|plate| {
        if rect.is_some() {
            let mut img = ImageNode::new(sheet);
            img.rect = rect;
            plate.spawn((
                img,
                Node {
                    width: look.px(PLATE_ICON),
                    height: look.px(PLATE_ICON),
                    ..default()
                },
                Pickable::IGNORE,
            ));
        }
        plate.spawn((
            look.text(Face::Display, 22.0, palette::HEADING, hero.name.clone()),
            Pickable::IGNORE,
        ));
    });
}

/// Where the nameplate's top-left corner goes, logical pixels: centred over `head` (the point
/// over the figure's head, on screen) with its foot there; then under each panel above it that it
/// would cover (the hero panel, the armour, the codex), beside each panel at its side (the zone
/// column), and inside the `screen`, [`PLATE_ROOM`] from each.
fn plate_spot(head: Vec2, size: Vec2, screen: Vec2, panels: &[Rect]) -> Vec2 {
    let mut at = Vec2::new(head.x - size.x / 2.0, head.y - size.y);
    let fits = |at: Vec2, p: &Rect| {
        at.x + size.x + PLATE_ROOM <= p.min.x || at.x >= p.max.x + PLATE_ROOM
    };
    // a panel at the head's side: the plate stays on the head's side of it
    for p in panels.iter().filter(|p| p.max.y > head.y) {
        if fits(at, p) || at.y >= p.max.y + PLATE_ROOM || at.y + size.y + PLATE_ROOM <= p.min.y {
            continue;
        }
        if head.x < p.center().x {
            at.x = p.min.x - PLATE_ROOM - size.x;
        } else {
            at.x = p.max.x + PLATE_ROOM;
        }
    }
    at.x = at.x.clamp(PLATE_ROOM, (screen.x - size.x - PLATE_ROOM).max(PLATE_ROOM));
    // a panel above the head: the plate comes down under it
    for p in panels.iter().filter(|p| p.max.y <= head.y) {
        if !fits(at, p) && at.y < p.max.y + PLATE_ROOM {
            at.y = p.max.y + PLATE_ROOM;
        }
    }
    at.y = at.y.clamp(PLATE_ROOM, (screen.y - size.y - PLATE_ROOM).max(PLATE_ROOM));
    at
}

/// Keeps the picked hero's nameplate over its figure's head as the world camera sees it (the
/// figure's head, [`benilla_app::FigureOverhead`], raised by [`PLATE_LIFT`]), clear of the
/// panels; hidden while the figure is not standing in the camp shown.
#[allow(clippy::type_complexity)] // one Bevy system's inputs
pub(super) fn place_name_over_head(
    state: Res<crate::picker::PickerState>,
    cams: Query<(&Camera, &GlobalTransform), With<benilla_world::view::WorldCamera>>,
    mut plates: Query<(&NameOverHead, &mut Node, &ComputedNode, &mut Visibility)>,
    heads: Query<&benilla_app::FigureOverhead>,
    panels: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<KeepNameClear>>,
    screen: Res<benilla_app::SelectScreen>,
) {
    let Ok((cam, at)) = cams.single() else {
        return;
    };
    let Some(view) = cam.logical_viewport_size() else {
        return;
    };
    let clear: Vec<Rect> = panels
        .iter()
        .filter_map(|(node, at)| {
            let scale = node.inverse_scale_factor();
            let size = node.size() * scale;
            (size.x > 0.0 && size.y > 0.0)
                .then(|| Rect::from_center_size(at.translation * scale, size))
        })
        .collect();
    for (plate, mut node, laid, mut vis) in &mut plates {
        let head = state
            .figure_of(plate.guid)
            .and_then(|e| heads.get(e).ok())
            .and_then(|h| cam.world_to_viewport(at, h.0 + Vec3::Y * PLATE_LIFT).ok());
        let size = laid.size() * laid.inverse_scale_factor();
        let Some(head) = head.filter(|_| screen.scene_ready && size.x > 0.0) else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        let spot = plate_spot(head, size, view, &clear);
        let (left, top) = (Val::Px(spot.x), Val::Px(spot.y));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        vis.set_if_neq(Visibility::Inherited);
    }
}

/// A run option's value as the save keeps it, by its slider's id.
pub(super) fn option_value(o: &esurv_protocol::RunOptions, id: &str) -> Option<u16> {
    Some(match id {
        "density" => o.density,
        "health" => o.health,
        "damage" => o.damage,
        "speed" => o.speed,
        _ => return None,
    })
}

/// The run options with one slider moved a step (kept on it).
pub(super) fn options_stepped(
    content: &Content,
    o: esurv_protocol::RunOptions,
    slider: usize,
    up: bool,
) -> esurv_protocol::RunOptions {
    let mut o = o;
    let Some(s) = content.run_options.get(slider) else {
        return o;
    };
    let field = match s.id.as_str() {
        "density" => &mut o.density,
        "health" => &mut o.health,
        "damage" => &mut o.damage,
        "speed" => &mut o.speed,
        _ => return o,
    };
    *field = if up {
        field.saturating_add(s.step_pct).min(s.max_pct)
    } else {
        field.saturating_sub(s.step_pct).max(s.min_pct)
    };
    o
}

/// Difficulty modifiers: the run options, each on its slider with its arrows, and the reward
/// bonus they give.
pub(super) fn draw_run_options(commands: &mut Commands, root: Entity, look: &MenuLook, ctx: &Ctx) {
    let Some((options, bonus)) = ctx.facts.run_options() else {
        return;
    };
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
            GlobalZIndex(super::MENU_LAYER + 10),
        ))
        .with_children(|veil| {
            let mut node = look.panel_node();
            node.width = look.px(400.0);
            node.row_gap = look.px(10.0);
            node.padding = UiRect::all(look.px(18.0));
            veil.spawn(look.panel(node)).with_children(|p| {
                look.heading(p, "Difficulty modifiers", 20.0);
                for (i, s) in ctx.content.run_options.iter().enumerate() {
                    let Some(v) = option_value(&options, &s.id) else {
                        continue;
                    };
                    p.spawn((
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: look.px(8.0),
                            ..default()
                        },
                        Tip(vec![
                            TipLine::new(s.name.clone(), palette::HEADING),
                            TipLine::new(s.tooltip.clone(), palette::VALUE),
                        ]),
                        MenuLookTip,
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Node {
                                flex_grow: 1.0,
                                ..default()
                            },
                            Pickable::IGNORE,
                        ))
                        .with_children(|n| {
                            n.spawn(look.value(s.name.clone()));
                        });
                        look.arrow(row, super::look::Arrow::Left, 22.0, v > s.min_pct)
                            .insert(MenuAction::OptionStep {
                                slider: i,
                                up: false,
                            });
                        row.spawn((
                            Node {
                                width: look.px(54.0),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            Pickable::IGNORE,
                        ))
                        .with_children(|n| {
                            n.spawn(look.text(
                                Face::Plain,
                                12.0,
                                palette::HEADING,
                                format!("{v}%"),
                            ));
                        });
                        look.arrow(row, super::look::Arrow::Right, 22.0, v < s.max_pct)
                            .insert(MenuAction::OptionStep {
                                slider: i,
                                up: true,
                            });
                    });
                }
                p.spawn(look.divider());
                p.spawn(Node {
                    column_gap: look.px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    look.value_box(
                        row,
                        "Rewards",
                        &format!("×{bonus:.2}"),
                        if bonus > 1.0005 {
                            palette::GREEN
                        } else {
                            palette::VALUE
                        },
                    );
                });
                p.spawn(look.hint(
                    "Harder runs pay more talent experience and drop gear more often. They count \
                     for every hero from the next run.",
                ));
                p.spawn(Node {
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|row| {
                    look.button(row, "Done", ButtonStyle::Primary, true, Some(120.0))
                        .insert(MenuAction::CloseRunOptions);
                });
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    const SCREEN: Vec2 = Vec2::new(1600.0, 900.0);
    const PLATE: Vec2 = Vec2::new(120.0, 26.0);

    /// In the open, the plate stands centred over the head, its foot on the head's point.
    #[test]
    fn the_nameplate_stands_centred_over_the_head() {
        let at = plate_spot(Vec2::new(500.0, 600.0), PLATE, SCREEN, &[]);
        assert_eq!(at, Vec2::new(440.0, 574.0));
    }

    /// A head under a panel above it (the hero panel, the armour, the codex) puts the plate under
    /// that panel's foot, never over the panel.
    #[test]
    fn the_nameplate_comes_down_under_a_panel_above() {
        let armour = Rect::new(400.0, 16.0, 800.0, 590.0);
        let at = plate_spot(Vec2::new(500.0, 600.0), PLATE, SCREEN, &[armour]);
        assert_eq!(at.y, 590.0 + PLATE_ROOM);
        assert_eq!(at.x, 440.0, "it keeps over the head sideways");
        // a panel off to the side leaves it where it stands
        let codex = Rect::new(1000.0, 16.0, 1200.0, 590.0);
        let at = plate_spot(Vec2::new(500.0, 600.0), PLATE, SCREEN, &[codex]);
        assert_eq!(at, Vec2::new(440.0, 574.0));
    }

    /// A head beside a tall panel (the zone column) keeps the plate on the head's side of it.
    #[test]
    fn the_nameplate_stays_beside_a_panel_at_its_side() {
        let zones = Rect::new(1234.0, 16.0, 1584.0, 884.0);
        let at = plate_spot(Vec2::new(1200.0, 600.0), PLATE, SCREEN, &[zones]);
        assert_eq!(at.x + PLATE.x, 1234.0 - PLATE_ROOM);
        assert_eq!(at.y, 574.0);
    }

    /// Near the screen's edge the plate stays whole on the screen.
    #[test]
    fn the_nameplate_stays_on_the_screen() {
        let at = plate_spot(Vec2::new(10.0, 5.0), PLATE, SCREEN, &[]);
        assert_eq!(at, Vec2::new(PLATE_ROOM, PLATE_ROOM));
        let at = plate_spot(Vec2::new(1595.0, 900.0), PLATE, SCREEN, &[]);
        assert_eq!(at.x + PLATE.x, SCREEN.x - PLATE_ROOM);
    }

    /// Every class has its icon on the class sheet, for the plate as for the class picker.
    #[test]
    fn every_class_has_its_plate_icon() {
        for class in &content().classes {
            assert!(class_cell(class.wow_class_id).is_some(), "{}", class.id);
        }
    }

    #[test]
    fn a_run_option_steps_on_its_slider() {
        let content = content();
        let o = esurv_protocol::RunOptions {
            density: 100,
            health: 100,
            damage: 100,
            speed: 100,
        };
        let i = content
            .run_options
            .iter()
            .position(|s| s.id == "density")
            .unwrap();
        let s = &content.run_options[i];
        let up = options_stepped(&content, o, i, true);
        assert_eq!(up.density, 100 + s.step_pct);
        assert_eq!(
            options_stepped(&content, o, i, false).density,
            s.min_pct.max(100 - s.step_pct.min(100)),
            "never under its slider"
        );
        assert_eq!(option_value(&up, "density"), Some(up.density));
        assert_eq!(option_value(&up, "luck"), None);
    }

    /// Every class shows three weapon loadouts under its starting spell, each naming its weapons,
    /// its strength and its drawback (roadmap R3).
    #[test]
    fn every_class_offers_three_loadouts_with_weapons_strength_and_drawback() {
        let c = content();
        for class in &c.classes {
            let all = c.loadouts_of(&class.id);
            assert_eq!(all.len(), 3, "{}", class.id);
            for l in all {
                assert!(!loadout_weapons(l).is_empty() && !l.strength.is_empty() && !l.drawback.is_empty());
            }
        }
        let shield = c.loadout("warrior", Some("shield")).unwrap();
        assert_eq!(loadout_weapons(shield), "One-handed sword, Shield");
    }

    /// The Weapon loadout row, as the Starting spell's: one icon per loadout in the class's
    /// order, exactly one lit: the save's pick, else the class's first (no pick, or one the class
    /// lacks).
    #[test]
    fn the_loadout_row_lights_the_saved_pick_else_the_first() {
        let c = content();
        let lit = |told: Option<&str>| -> Vec<(String, bool)> {
            loadout_icons(&c, "warrior", told).iter().map(|i| (i.loadout.id.clone(), i.chosen)).collect()
        };
        let row = |on: &str| -> Vec<(String, bool)> {
            ["two_hander", "dual_wield", "shield"].iter().map(|id| (id.to_string(), *id == on)).collect()
        };
        assert_eq!(lit(Some("shield")), row("shield"));
        assert_eq!(lit(Some("dual_wield")), row("dual_wield"));
        assert_eq!(lit(None), row("two_hander"));
        assert_eq!(lit(Some("")), row("two_hander"));
        assert_eq!(lit(Some("gun")), row("two_hander"), "another class's loadout");
        assert!(loadout_icons(&c, "no_such_class", None).is_empty());
        for class in &c.classes {
            let icons = loadout_icons(&c, &class.id, None);
            assert_eq!(icons.iter().filter(|i| i.chosen).count(), 1, "{}", class.id);
            assert!(icons.iter().all(|i| !i.loadout.icon.is_empty()), "{}", class.id);
        }
    }

    /// A loadout's tooltip tells what the row's icon no longer shows: its name, its weapons, its
    /// strength in green and its drawback in red.
    #[test]
    fn a_loadout_tip_tells_name_weapons_strength_and_drawback() {
        let c = content();
        let shield = c.loadout("warrior", Some("shield")).unwrap();
        let tip = loadout_tip(shield);
        let lines: Vec<(&str, Color)> = tip.iter().map(|l| (l.text.as_str(), l.colour)).collect();
        assert_eq!(
            lines,
            [
                ("One-hander and shield", palette::VALUE),
                ("One-handed sword, Shield", palette::HEADING),
                (shield.strength.as_str(), palette::GREEN),
                (shield.drawback.as_str(), palette::RED),
            ]
        );
        for class in &c.classes {
            for l in c.loadouts_of(&class.id) {
                assert_eq!(loadout_tip(l).len(), 4, "{}/{}", class.id, l.id);
            }
        }
    }

    /// Every class's starting options have a tooltip: the name first, each level-1 spell with its
    /// numbers filled in (no "?"), then the passive.
    #[test]
    fn every_starting_option_tells_its_spells_and_passive() {
        let c = content();
        for class in &c.classes {
            assert!(!class.starting_options.is_empty(), "{}", class.id);
            for o in &class.starting_options {
                let tip = option_tip(&c, class, o);
                assert_eq!(tip[0].text, o.name);
                assert!(tip.iter().all(|l| !l.text.contains('?')), "{}: {tip:?}", o.id);
                let passive = format!("Passive: {}", o.passive.name);
                assert!(tip.iter().any(|l| l.text == passive), "{}: {tip:?}", o.id);
                let main = c.ability(o.main_spell()).unwrap();
                assert!(tip.iter().any(|l| l.text == main.name), "{}", o.id);
            }
        }
    }

    /// Every zone slot is listed; a zone no class has opened is not
    /// discovered ("???" and a lock), one another class opened is discovered but stays locked,
    /// and one open to the hero's class is both.
    #[test]
    fn the_selector_hides_a_zone_until_the_save_discovers_it() {
        let content = content();
        // the hero's class has two open; another class has opened the third
        let discovered = ["elwynn", "duskwood", "wsg"].map(String::from);
        let rows = zone_choices(&content, 0, 2, &discovered);
        assert_eq!(rows.len(), content.zones.len(), "every slot shows");
        let state = |i: usize| (rows[i].discovered, rows[i].open);
        assert_eq!(state(0), (true, true));
        assert_eq!(state(1), (true, true), "open to the class");
        assert_eq!(state(2), (true, false), "discovered, still locked for this class");
        assert_eq!(state(3), (false, false), "undiscovered");
        assert!(rows[3..].iter().all(|z| !z.discovered && !z.open));
        // nothing told yet: only the first zone, and what the class has open
        let none = zone_choices(&content, 0, 3, &[]);
        assert!(none[2].discovered && !none[3].discovered);
    }

    #[test]
    fn a_locked_zone_names_what_opens_it() {
        let content = content();
        assert_eq!(opener(&content, 0), "");
        let line = opener(&content, 1);
        let final_boss = content.zones[0].bosses.iter().find(|b| b.is_final).unwrap();
        assert!(line.contains(&final_boss.name) && line.contains(&content.zones[0].name));
    }

    /// A zone added as a file shows in the selector in its place by difficulty, locked until the
    /// zone before it is won, and says so; one past the last opens after `aq`.
    #[test]
    fn the_selector_lists_an_added_zone_in_its_place() {
        use esurv_content::testing::{content_folder_with_zones, ExtraZone};
        let shipped = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
        let zones = [
            ExtraZone {
                id: "test_middle",
                name: "Test Middle",
                difficulty: 2.5,
            },
            ExtraZone {
                id: "test_end",
                name: "Test End",
                difficulty: 11.0,
            },
        ];
        let dir = content_folder_with_zones(shipped.as_ref(), "app-added-zones", &zones).unwrap();
        let content = Content::load(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        let content = content.unwrap();
        let all: Vec<String> = content.zones.iter().map(|z| z.id.clone()).collect();
        let shown = |open: usize| zone_choices(&content, 0, open, &all);
        let ids: Vec<&str> = shown(1).iter().map(|z| z.zone.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "elwynn",
                "duskwood",
                "test_middle",
                "wsg",
                "scarlet",
                "sunken_temple",
                "feralas",
                "arena",
                "strat",
                "brm",
                "aq",
                "test_end"
            ]
        );
        // two open (Elwynn won): Test Middle is locked, opened by a win in its zone
        let two = shown(2);
        assert!(two[1].open && !two[2].open);
        assert!(
            opener(&content, 2).contains("Area 10"),
            "{}",
            opener(&content, 2)
        );
        // the third zone won (the server tells three open): Test Middle is open, the next is not
        let three = shown(3);
        assert!(three[2].open && !three[3].open);
        assert!(!shown(11)[11].open && shown(12)[11].open);
        assert!(
            opener(&content, 11).contains("Area 3478"),
            "{}",
            opener(&content, 11)
        );
        assert!(shown(1)[0].chosen && zone_choices(&content, 2, 3, &all)[2].chosen);
    }
}

/// The codex beside the zone column: how much of each list the save has found, and the way in.
fn codex_panel(r: &mut ChildSpawnerCommands, look: &MenuLook, ctx: &Ctx) {
    let mut node = look.panel_node();
    node.position_type = PositionType::Absolute;
    node.right = look.px(MARGIN * 2.0 + ZONE_WIDTH);
    node.top = look.px(MARGIN);
    node.width = look.px(CODEX_WIDTH);
    node.row_gap = look.px(5.0);
    r.spawn((look.panel(node), KeepNameClear)).with_children(|p| {
        look.heading(p, "Codex", 18.0);
        super::codex::counts(p, look, ctx);
        look.button(p, "Open the codex", ButtonStyle::Text, true, None)
            .insert(MenuAction::OpenCodex);
    });
}
