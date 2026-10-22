//! The starting options on the clients (docs/game/screens.md, "The
//! power bar", and docs/game/server.md): the hero's mana bar showing its option's resource or
//! counter (the client's own rage and energy, else the mana bar painted by our interface), its
//! combo points, its stacking buffs as auras, the ghostly copy a passive summons, a form's model,
//! and how some spells' looks are drawn ([`look_styles`]).

use esurv_content::option_looks::{
    bar_name, resource_word, stack_look, HOT_STREAK_AURAS, MAELSTROM_AURAS, SLAM_BURST_LOOK, SLAM_BURST_SECONDS,
    SLAM_CONE_SPACING,
};
use esurv_content::{Content, Faction, ResourceKind, StackKind, StartingOption, SummonKind};
use esurv_protocol::{self as protocol, LookStyle, SheathHold, ToClient};
use esurv_sim::game::enemy_ai::Vec2;
use esurv_sim::game::run::{Aim, Event, Hero, Unit};

use super::present::Aura;
use super::Game;
use crate::packets;
use crate::update::{field, Fields, Living, ObjectTypeId, ObjectUpdate, Speeds, TYPE_OBJECT, TYPE_PLAYER, TYPE_UNIT};
use crate::wire::Packet;

/// 1.12's power types: what the player frame's power bar is.
const POWER_MANA: u8 = 0;
const POWER_RAGE: u8 = 1;
const POWER_ENERGY: u8 = 3;
/// The client counts rage in tenths.
const RAGE_TENTHS: f32 = 10.0;

/// The aura that makes a unit a see-through, blue-tinted ghost (`Spell.dbc`: Ghost Visual).
pub const GHOST_LOOK: u32 = 9_617;
/// A clone's guid: a player guid with this bit, its summon id and its hero's save guid.
pub const CLONE_GUID_BIT: u64 = 0x4000_0000;

/// The guid of clone `summon` of the hero whose save guid is `hero`.
pub fn clone_guid(hero: u32, summon: u32) -> u64 {
    CLONE_GUID_BIT | (u64::from(summon) & 0x3FFF) << 16 | (u64::from(hero) & 0xFFFF)
}

/// The save guid of the hero a clone's guid copies, if it is a clone's.
pub fn clone_of(guid: u64) -> Option<u32> {
    (guid & CLONE_GUID_BIT != 0 && guid >> 32 == 0).then_some((guid & 0xFFFF) as u32)
}

/// What one hero's client was last shown of its starting option.
#[derive(Debug, Clone, Default)]
pub(super) struct OptionShown {
    /// The mana bar's kind, value and most, as the run last said.
    pub bar: Option<(ResourceKind, f32, f32)>,
    /// The power fields last sent: type, value and most as the client counts them.
    power_sent: Option<(u8, u32, u32)>,
    /// The stacking buffs: kind, stacks, most, and when they end (`None`: until used).
    pub stacks: Vec<(StackKind, u32, u32, Option<f32>)>,
}

/// A ghostly copy of a hero on the field.
#[derive(Debug, Clone, Copy)]
pub(super) struct CloneShown {
    pub hero: u32,
    pub at: Vec2,
    pub guid: u64,
    /// Told to the clients.
    pub created: bool,
}

/// How the run's looks of some spells are drawn on every client: as
/// docs/game/screens.md "Looks" lists them.
pub fn look_styles() -> Vec<LookStyle> {
    let style = |spell, scale, tint, flat, no_shake| LookStyle {
        spell,
        scale,
        tint,
        recolor: false,
        flat,
        no_shake,
        opacity: 1.0,
    };
    // A look recoloured: its own colours' brightness, in `tint`'s hue
    let recolored = |spell, scale, tint| LookStyle { recolor: true, ..style(spell, scale, Some(tint), false, false) };
    let mut out = vec![
        // Inner Fire's burn on the Priest: the channelled whirl tinted red and white
        style(EVOCATION, 1.0, Some([1.0, 0.45, 0.40]), false, false),
        // Maelstrom Lightning: Chain Lightning's look, teal
        style(MAELSTROM_LIGHTNING, 1.0, Some([0.25, 1.0, 0.85]), false, false),
        // Arcane Knowledge's rune laid on the floor
        style(DAMPEN_MAGIC, 1.0, None, true, false),
        // Wellspring: Massive Geyser at two thirds
        style(MASSIVE_GEYSER, 0.66, None, false, false),
        // Denounce: Holy Wrath's bolts, chain and bursts at half size, so the screen stays clear
        style(HOLY_WRATH, 0.5, None, false, false),
        // Berserk's extra hits: a small sweeping slash on each
        style(EXTRA_STRIKE_LOOK, 0.4, None, false, false),
        // Fan of Knives' thrown knives at twice their size
        style(extras::THROW, 2.0, None, false, false),
        // Divine Star: a holy bolt with its trail, at twice its size
        style(extras::DIVINE_STAR, 2.0, None, false, false),
        // Emerald Slumber's lasher bursting: the Goblin Sapper Charge's blast recoloured poison green,
        // at half size
        recolored(extras::LASHER_BLAST, 0.5, LASHER_POISON_GREEN),
        // Sanctified Ground: Consecration grows as Death and Decay's ground, recoloured holy gold
        recolored(SANCTIFIED_GROUND, 1.0, HOLY_GOLD),
        // the bosses' marks (docs/game/bosses.md "Building blocks"): drawn by the client as
        // Kel'Thuzad's Shadow Fissure in its own purple (esurv_content::option_looks::MARK_MODELS),
        // half see-through
        LookStyle { opacity: MARK_OPACITY, ..style(MARK_WARNING, 1.0, None, false, false) },
        LookStyle { opacity: MARK_OPACITY, ..style(MARK_FILL, 1.0, None, false, false) },
    ];
    // Hot Streak's flames grow with its stacks
    for (aura, scale) in HOT_STREAK_AURAS.into_iter().zip([0.6, 1.0, 1.25]) {
        out.push(style(aura, scale, None, false, false));
    }
    // Maelstrom Weapon's tornado round the hero grows with each stack
    for (aura, scale) in MAELSTROM_AURAS.into_iter().zip(MAELSTROM_TORNADO_SCALES) {
        out.push(style(aura, scale, None, false, false));
    }
    out
}

/// The poison green a lasher's burst is recoloured to (linear 0-1 RGB).
pub const LASHER_POISON_GREEN: [f32; 3] = [0.4, 1.0, 0.15];
/// The legendary looks that are recoloured (`legendary_looks.ron`): Sanctified Ground's growing
/// Consecration (Death and Decay, 11433).
pub const SANCTIFIED_GROUND: u32 = 11_433;
/// Its colour (linear 0-1 RGB).
pub const HOLY_GOLD: [f32; 3] = [1.0, 0.82, 0.35];
/// A boss mark's looks (`enemy_kinds.ron` `marked`): Consecration's ranks 5 and 4, so the
/// paladin's own (26573) keeps its gold; the client draws them as Shadow Fissures
/// (`esurv_content::option_looks::MARK_MODELS`).
pub const MARK_WARNING: u32 = esurv_content::option_looks::MARK_WARNING_LOOK;
pub const MARK_FILL: u32 = esurv_content::option_looks::MARK_FILL_LOOK;
/// How see-through the marks are: half.
pub const MARK_OPACITY: f32 = 0.5;

/// Maelstrom Weapon's tornado round the hero, one size per stack, smallest first.
pub const MAELSTROM_TORNADO_SCALES: [f32; 5] = [0.3, 0.42, 0.54, 0.66, 0.8];

/// `Spell.dbc` ids whose looks the styles name.
pub const EVOCATION: u32 = 12_051;
/// A chain lightning of its own, so the card's Chain Lightning (421) keeps its colour.
pub const MAELSTROM_LIGHTNING: u32 = 930;
pub const DAMPEN_MAGIC: u32 = 604;
/// Wellspring's geyser bursting up under each enemy it hits.
pub const MASSIVE_GEYSER: u32 = 22_421;
/// Denounce's chaining holy bolt.
pub const HOLY_WRATH: u32 = 28_883;
/// The slash on each enemy Berserk's attacks also strike (the run's own look id).
pub const EXTRA_STRIKE_LOOK: u32 = esurv_sim::game::kit::EXTRA_STRIKE_LOOK;

/// The power type a hero's player frame shows: its option's rage or energy, else mana (the bar our
/// interface paints for a counter).
pub fn hero_power_type(content: &Content, hero: &Hero) -> u8 {
    match option_of(content, hero).and_then(|o| o.resource).map(|r| r.kind()) {
        Some(ResourceKind::Rage) => POWER_RAGE,
        Some(ResourceKind::Energy) => POWER_ENERGY,
        _ => POWER_MANA,
    }
}

/// The hero's starting option.
fn option_of<'a>(content: &'a Content, hero: &Hero) -> Option<&'a StartingOption> {
    content.classes.get(hero.class)?.starting_option(Some(&hero.start))
}

/// The model a hero in a form wears (`CreatureDisplayInfo.dbc`), by its race: the night elf's and
/// the tauren's own, else its faction's.
pub fn form_display(content: &Content, hero: &Hero) -> Option<u32> {
    let form = option_of(content, hero)?.form.as_ref()?;
    let race = content.races.get(hero.race)?;
    // (night elf, tauren)
    let (alliance, horde) = match form.look_spell {
        // Cat Form
        768 => (892, 8571),
        // Bear Form, Dire Bear Form
        5487 | 9634 => (2281, 2289),
        MOONKIN_FORM => (15374, 15375),
        _ => return None,
    };
    Some(match (race.wow_race_id, race.faction) {
        (4, _) => alliance,
        (6, _) => horde,
        (_, Faction::Alliance) => alliance,
        (_, Faction::Horde) => horde,
    })
}

/// Moonkin Form's look (`Spell.dbc`).
const MOONKIN_FORM: u32 = 24858;

/// How a hero's weapons show in its form (roadmap R3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormWeapons {
    /// No form: as its class carries them.
    AsTheClass,
    /// Moonkin Form: put away.
    PutAway,
    /// Cat Form, Bear Form (and any other animal form): not shown.
    Hidden,
}

/// How hero `hero`'s weapons show in the form it plays in ([`FormWeapons`]).
pub fn form_weapons(content: &Content, hero: &Hero) -> FormWeapons {
    match option_of(content, hero).and_then(|o| o.form.as_ref()) {
        None => FormWeapons::AsTheClass,
        Some(f) if f.look_spell == MOONKIN_FORM => FormWeapons::PutAway,
        Some(_) => FormWeapons::Hidden,
    }
}

/// The sheath state hero `hero`'s weapons keep all run, whatever its swings, casts and clips would
/// do, or `None` to follow the stock client: Moonkin Form keeps them put
/// away (0), the shaman's two-hander keeps it in hand (1).
pub fn sheath_hold(content: &Content, hero: &Hero) -> Option<u8> {
    if form_weapons(content, hero) == FormWeapons::PutAway {
        return Some(0);
    }
    let class = &content.classes[hero.class];
    (class.id == "shaman" && hero.loadout == "two_hander").then_some(1)
}

/// The name a hero's bar shows for `kind`: empty for rage and energy (the client's own word), the
/// bar's own names where they differ from the passive's, Holy Shield's own for its stacks, else the
/// option's passive's.
pub fn bar_label(content: &Content, hero: &Hero, kind: ResourceKind) -> String {
    if matches!(kind, ResourceKind::Rage | ResourceKind::Energy | ResourceKind::ComboPoints) {
        return String::new();
    }
    if let Some(name) = bar_name(kind) {
        return name.into();
    }
    let Some(option) = option_of(content, hero) else {
        return String::new();
    };
    if kind == ResourceKind::HolyShieldStacks {
        if let Some(a) = content.ability(option.main_spell()) {
            return a.name.clone();
        }
    }
    option.passive.name.clone()
}

impl Game {
    /// Keeps what the run says of the heroes' options: their bars, stacks and clones.
    pub(super) fn note_option_events(&mut self, events: &[Event]) {
        for e in events {
            match e {
                Event::ResourceChanged { hero, kind, value, max } if *kind != ResourceKind::ComboPoints => {
                    self.options_shown.entry(*hero).or_default().bar = Some((*kind, *value, *max));
                }
                Event::StacksChanged { hero, kind, stacks, max, until } => {
                    let shown = &mut self.options_shown.entry(*hero).or_default().stacks;
                    shown.retain(|s| s.0 != *kind);
                    if *stacks > 0 {
                        shown.push((*kind, *stacks, *max, *until));
                    }
                }
                Event::SummonAppeared { id, hero, kind: SummonKind::Clone, at, .. } => {
                    let save = self.saved_of(*hero).map_or(0, |s| s.guid);
                    self.clones.insert(*id, CloneShown { hero: *hero, at: *at, guid: clone_guid(save, *id), created: false });
                }
                Event::SummonGone { id } => {
                    if let Some(c) = self.clones.get_mut(id) {
                        // gone at the next mirror
                        c.hero = u32::MAX;
                    }
                }
                _ => {}
            }
        }
    }

    /// The saved hero of run hero `hero`.
    fn saved_of(&self, hero: u32) -> Option<&crate::save::SavedHero> {
        self.players.iter().find(|p| p.hero == hero).map(|p| &p.saved)
    }

    /// Hero `hero`'s stacking buffs as auras, after its other buffs.
    pub(super) fn stack_auras(&self, hero: u32) -> Vec<Aura> {
        self.options_shown
            .get(&hero)
            .map(|s| {
                s.stacks
                    .iter()
                    .map(|&(kind, stacks, _, until)| Aura {
                        spell: stack_look(kind, stacks).aura,
                        stacks: if kind == StackKind::HotStreak || kind == StackKind::MaelstromWeapon { stacks } else { stacks.max(1) },
                        until: until.unwrap_or(0.0),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Hero `hero`'s stacking buffs as its buff frame's tooltips tell them.
    pub(super) fn stack_tips(&self, hero: &Hero) -> Vec<protocol::BuffTip> {
        let passive = option_of(&self.run.content, hero).map(|o| o.passive.id.clone()).unwrap_or_default();
        self.options_shown
            .get(&hero.id)
            .map(|s| {
                s.stacks
                    .iter()
                    .map(|&(kind, stacks, max, _)| protocol::BuffTip {
                        look: stack_look(kind, stacks).aura,
                        kind: protocol::BuffKind::Passive,
                        spell: if kind == StackKind::HolyShield {
                            option_of(&self.run.content, hero).map(|o| o.main_spell().to_owned()).unwrap_or_default()
                        } else {
                            passive.clone()
                        },
                        value: max as f32,
                        stacks,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The heroes' power fields that changed: the bar's type, value and most, for every client.
    pub(super) fn power_fields(&mut self, objects: &mut Vec<ObjectUpdate>) {
        for h in &self.run.heroes {
            let Some(shown) = self.options_shown.get(&h.id) else {
                continue;
            };
            let Some((kind, value, max)) = shown.bar else {
                continue;
            };
            let power = match kind {
                ResourceKind::Rage => POWER_RAGE,
                ResourceKind::Energy => POWER_ENERGY,
                _ => POWER_MANA,
            };
            let tenths = if power == POWER_RAGE { RAGE_TENTHS } else { 1.0 };
            let sent = (power, (value * tenths).round().max(0.0) as u32, (max * tenths).round().max(1.0) as u32);
            if shown.power_sent == Some(sent) {
                continue;
            }
            let Some(guid) = self.guid_of(Unit::Hero(h.id)) else {
                continue;
            };
            let content = &self.run.content;
            let (class, race) = (&content.classes[h.class], &content.races[h.race]);
            let gender = self.saved_of(h.id).map_or(0, |s| s.gender);
            let mut f = Fields::new();
            f.bytes(field::UNIT_BYTES_0, [race.wow_race_id, class.wow_class_id as u8, gender, power]);
            f.u32(field::UNIT_POWER1 + u16::from(power), sent.1).u32(field::UNIT_MAXPOWER1 + u16::from(power), sent.2);
            objects.push(ObjectUpdate::Values { guid, fields: f });
            if let Some(s) = self.options_shown.get_mut(&h.id) {
                s.power_sent = Some(sent);
            }
        }
    }

    /// The bar message for hero `hero`'s own client.
    pub(super) fn bar_message(&self, hero: u32, kind: ResourceKind, value: f32, max: f32) -> Option<ToClient> {
        let h = self.run.heroes.iter().find(|h| h.id == hero)?;
        Some(ToClient::Bar {
            hero: self.guid_of(Unit::Hero(hero))?,
            kind: resource_word(kind),
            name: bar_label(&self.run.content, h, kind),
            value,
            max,
        })
    }

    /// What a joining client is told of its hero's option: the looks' styles and its bar now.
    pub(super) fn option_hello(&self, hero: u32) -> Vec<ToClient> {
        let mut told = vec![ToClient::LookStyles(look_styles())];
        // every hero whose weapons keep one sheath state, for the client to hold it
        let holds: Vec<SheathHold> = self
            .run
            .heroes
            .iter()
            .filter_map(|h| Some(SheathHold { hero: self.guid_of(Unit::Hero(h.id))?, state: sheath_hold(&self.run.content, h)? }))
            .collect();
        told.push(ToClient::SheathHolds(holds));
        if let Some((kind, value, max)) = self.options_shown.get(&hero).and_then(|s| s.bar) {
            told.extend(self.bar_message(hero, kind, value, max));
        }
        told
    }

    /// The create of a clone of its hero: the hero's player look, see-through, where it stands.
    fn clone_object(&self, c: &CloneShown) -> Option<ObjectUpdate> {
        let hero = self.run.heroes.iter().find(|h| h.id == c.hero)?;
        let saved = self.saved_of(c.hero)?;
        let ObjectUpdate::Create { mut fields, living, .. } = self.hero_object(hero, saved, false) else {
            return None;
        };
        fields.u64(field::OBJECT_GUID, c.guid).u32(field::OBJECT_TYPE, TYPE_OBJECT | TYPE_UNIT | TYPE_PLAYER);
        // the ghost's look, and nothing to click or fight
        fields.u32(field::UNIT_AURA, GHOST_LOOK).u32(field::UNIT_AURAFLAGS, 0x2).u32(field::UNIT_AURALEVELS, 1);
        fields.u32(field::UNIT_FLAGS, CLONE_UNIT_FLAGS);
        let living = Living { position: [c.at.0, c.at.1, self.ground_z(c.at)], facing: living.facing, speeds: Speeds::DEFAULT };
        Some(ObjectUpdate::Create { guid: c.guid, type_id: ObjectTypeId::Player, living, fields, own: false })
    }

    /// Every clone the clients already know of, as creates (for a player who joins).
    pub(super) fn clones_now(&self) -> Vec<ObjectUpdate> {
        self.clones.values().filter(|c| c.created).filter_map(|c| self.clone_object(c)).collect()
    }

    /// New clones appear (their creates go to `creates`); those whose time is up go.
    pub(super) fn mirror_clones(&mut self, creates: &mut Vec<ObjectUpdate>, gone: &mut Vec<Packet>) {
        let ended: Vec<u32> = self.clones.iter().filter(|(_, c)| c.hero == u32::MAX).map(|(id, _)| *id).collect();
        for id in ended {
            if let Some(c) = self.clones.remove(&id) {
                if c.created {
                    gone.push(packets::destroy_object(c.guid));
                }
            }
        }
        let new: Vec<u32> = self.clones.iter().filter(|(_, c)| !c.created).map(|(id, _)| *id).collect();
        for id in new {
            let Some(create) = self.clones.get(&id).and_then(|c| self.clone_object(c)) else {
                continue;
            };
            creates.push(create);
            if let Some(c) = self.clones.get_mut(&id) {
                c.created = true;
            }
        }
    }
}

/// How far apart two points on the ground are, yards.
fn distance(a: Vec2, b: Vec2) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// A clone cannot be attacked or picked (`UNIT_FLAG_NON_ATTACKABLE | UNIT_FLAG_NOT_SELECTABLE`).
const CLONE_UNIT_FLAGS: u32 = 0x0200_0002;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clone_guid_names_its_hero() {
        let g = clone_guid(7, 300);
        assert_eq!(clone_of(g), Some(7));
        assert_eq!(clone_of(7), None, "a hero's own guid is no clone's");
        assert_ne!(clone_guid(7, 301), g);
    }

    #[test]
    fn the_styles_name_each_spell_once() {
        let styles = look_styles();
        let mut ids: Vec<u32> = styles.iter().map(|s| s.spell).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), styles.len());
        let rune = styles.iter().find(|s| s.spell == DAMPEN_MAGIC).unwrap();
        assert!(rune.flat);
        let knives = styles.iter().find(|s| s.spell == extras::THROW).unwrap();
        assert_eq!(knives.scale, 2.0, "Fan of Knives' knives at twice their size");
        let lasher = styles.iter().find(|s| s.spell == extras::LASHER_BLAST).unwrap();
        assert!(lasher.recolor, "the lasher's blast is recoloured, not multiplied");
        assert_eq!(lasher.tint, Some(LASHER_POISON_GREEN));
        assert_eq!(lasher.scale, 0.5, "the lasher's blast at half size");
        // the recoloured ones: the lasher's blast and Sanctified Ground (the boss marks are Shadow
        // Fissures in their own purple)
        let recoloured = [extras::LASHER_BLAST, SANCTIFIED_GROUND];
        assert!(styles.iter().filter(|s| !recoloured.contains(&s.spell)).all(|s| !s.recolor));
        let gold = styles.iter().find(|s| s.spell == SANCTIFIED_GROUND).unwrap();
        assert_eq!((gold.recolor, gold.tint), (true, Some(HOLY_GOLD)), "Sanctified Ground in holy gold");
        // the boss marks are half see-through; every other look is as drawn
        let see_through: Vec<(u32, f32)> = styles.iter().filter(|s| s.opacity != 1.0).map(|s| (s.spell, s.opacity)).collect();
        assert_eq!(see_through, [(MARK_WARNING, 0.5), (MARK_FILL, 0.5)]);
        // in their own purple: no tint, no recolour
        assert!(styles.iter().filter(|s| s.spell == MARK_WARNING || s.spell == MARK_FILL).all(|s| s.tint.is_none() && !s.recolor));
    }

    /// Roadmap R3: Cat and Bear Form show no weapons, Moonkin Form shows
    /// them put away, a hero in no form as its class carries them.
    #[test]
    fn a_form_hides_or_puts_away_the_loadouts_weapons() {
        use std::sync::Arc;
        use esurv_sim::game::run::{Flat, HeroSetup, Run};
        let content = Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
        let weapons = |class: &str, start: &str| {
            let setup = HeroSetup { class: class.into(), race: "night_elf".into(), start: Some(start.into()), ..Default::default() };
            let run = Run::new(content.clone(), "elwynn", Default::default(), &[setup], Box::new(Flat), 1).unwrap();
            form_weapons(&content, &run.heroes[0])
        };
        assert_eq!(weapons("druid", "cat_form"), FormWeapons::Hidden);
        assert_eq!(weapons("druid", "bear_form"), FormWeapons::Hidden);
        assert_eq!(weapons("druid", "moonkin_form"), FormWeapons::PutAway);
        assert_eq!(weapons("warrior", "slam"), FormWeapons::AsTheClass);
    }

    /// Moonkin keeps its weapons put away all run, the shaman's
    /// two-hander stays in hand; every other hero follows the stock client.
    #[test]
    fn moonkin_holds_its_weapons_away_and_the_shamans_two_hander_holds_it_drawn() {
        use std::sync::Arc;
        use esurv_sim::game::run::{Flat, HeroSetup, Run};
        let content = Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
        let hold = |class: &str, start: Option<&str>, loadout: Option<&str>| {
            let setup = HeroSetup {
                class: class.into(),
                race: "tauren".into(),
                start: start.map(Into::into),
                loadout: loadout.map(Into::into),
                ..Default::default()
            };
            let run = Run::new(content.clone(), "elwynn", Default::default(), &[setup], Box::new(Flat), 1).unwrap();
            sheath_hold(&content, &run.heroes[0])
        };
        assert_eq!(hold("druid", Some("moonkin_form"), None), Some(0));
        assert_eq!(hold("druid", Some("cat_form"), None), None);
        assert_eq!(hold("shaman", None, Some("two_hander")), Some(1));
        assert_eq!(hold("shaman", None, Some("staff")), None);
        assert_eq!(hold("shaman", None, None), None, "its first loadout");
        assert_eq!(hold("warrior", None, Some("two_hander")), None, "only the shaman's");
    }

    #[test]
    fn slams_earth_covers_its_cone() {
        let spots = cone_spots((0.0, 0.0), 0.0, 15.0, 60.0, 2.5);
        // six rows out to the cone's end, wider as they go
        assert_eq!(spots.len(), 2 + 3 + 4 + 5 + 6 + 7, "{}", spots.len());
        for s in &spots {
            let d = (s.0 * s.0 + s.1 * s.1).sqrt();
            assert!((2.49..=15.01).contains(&d));
            assert!(s.1.atan2(s.0).abs() <= 30f32.to_radians() + 1e-4, "inside the cone");
        }
        assert!(spots.iter().any(|s| (s.0 - 15.0).abs() < 0.01), "it reaches the end");
        assert!(cone_spots((0.0, 0.0), 0.0, 0.0, 60.0, 2.5).is_empty());
    }
}

/// The extra looks and sounds of the options' spells (docs/game/
/// screens.md, "Looks"), as `Spell.dbc` and `SoundEntries.dbc` ids in the player's client.
pub mod extras {
    /// Slam's look; bursts of earth cover its cone on the floor (`option_looks::SLAM_BURSTS`).
    pub const SLAM: u32 = 1_464;
    /// Whirlwind: Fan of Knives' spin before the knives fly.
    pub const SPIN: u32 = 1_680;
    /// Throw: a missile that flies the thrown weapon it is given.
    pub const THROW: u32 = 2_764;
    /// Holy Shield's strike back on a block.
    pub const HOLY_SHIELD_STRIKE: u32 = 9_800;
    /// Lightning Shield's crackling swirl on an enemy Lightning Rod marked.
    pub const LIGHTNING_ROD_MARK: u32 = 324;
    /// A throwing knife's item look (`ItemDisplayInfo.dbc`) and its inventory type (thrown).
    pub const KNIFE: (u32, u32) = (3_274, 25);
    /// Sounds.
    pub const DYNAMITE: u32 = 4_100;
    pub const DAGGER_IN_FLESH: u32 = 1_151;
    pub const DAGGER_IN_FLESH_CRITICAL: u32 = 1_152;
    pub const THROW_ITEM: u32 = 7_140;
    pub const POISON_CLOUD: u32 = 5_335;
    pub const LAY_ON_HANDS: u32 = 1_431;
    pub const OIL_OF_IMMOLATION: u32 = 4_474;
    pub const UNDER_WATER: u32 = 4_123;
    /// Shred's sound (its cast's), on each enemy Lacerate reaches.
    pub const CAT_ATTACK: u32 = 4_121;
    /// Lacerate's look (Rend's), which its bleed shows as it lands on each enemy.
    pub const REND: u32 = 772;
    /// Fan of Knives: how many knives fly out, how far, and when after the spin.
    pub const KNIVES: u32 = 14;
    pub const KNIFE_REACH: f32 = 8.0;
    pub const KNIVES_AFTER: f32 = 0.35;
    /// Explosive Shot's second blast sound, after the first.
    pub const SECOND_BLAST_AFTER: f32 = 0.5;
    /// The dynamite blowing up at the feet of each enemy Explosive Shot's blast hits (Ez-Thro
    /// Dynamite Backfire's look: its blast plays at its caster's feet, so each enemy casts it).
    pub const DYNAMITE_BLAST: u32 = 8_332;
    /// Divine Star's look (a holy bolt that flies to a point): it flies out to its range, and a
    /// second one flies back from there to the priest, cast by a look of its own left on the
    /// ground where the first lands (that look stays `DIVINE_STAR_TURN_STAYS` after the return
    /// starts).
    pub const DIVINE_STAR: u32 = 10_319;
    /// Emerald Slumber's lasher bursting: a Goblin Sapper Charge's
    /// blast (its own look), recoloured poison green by its look style ([`super::look_styles`]),
    /// with the poison cloud sound, cast by a look left on the ground where it stood, so the blast
    /// outlives the lasher.
    pub const LASHER_BLAST: u32 = 13_241;
    pub const LASHER_BLAST_STAYS: f32 = 2.0;
    pub const DIVINE_STAR_TURN_STAYS: f32 = 2.0;
    /// Starshards' falling stars (its own look's lasting part) on each enemy its novas strike,
    /// while they go on striking it: a debuff that lasts this long after each strike.
    pub const STARSHARDS: u32 = 10_797;
    pub const STARSHARDS_MARK_SECONDS: f32 = 1.25;
}

/// Where Slam's bursts of earth lie to cover its cone: rows every
/// `spacing` yards out from `at` along `facing` up to `length`, each spread across the cone's
/// `degrees` with a burst every `spacing` yards of its arc (at least one, in its middle).
pub fn cone_spots(at: Vec2, facing: f32, length: f32, degrees: f32, spacing: f32) -> Vec<Vec2> {
    let mut out = Vec::new();
    if spacing <= 0.0 || length <= 0.0 {
        return out;
    }
    let half = degrees.to_radians() / 2.0;
    let rows = (length / spacing).round().max(1.0) as u32;
    for row in 1..=rows {
        let d = length * row as f32 / rows as f32;
        let n = ((2.0 * half * d) / spacing).round().max(0.0) as u32 + 1;
        for k in 0..n {
            let a = if n == 1 { facing } else { facing - half + 2.0 * half * k as f32 / (n - 1) as f32 };
            out.push((at.0 + d * a.cos(), at.1 + d * a.sin()));
        }
    }
    out
}

impl Game {
    /// The extra looks and sounds this tick's events bring, for every client, with the ones whose
    /// time came.
    pub(super) fn option_extras(&mut self, events: &[Event]) -> Vec<Packet> {
        use extras::*;
        use esurv_sim::game::casting::HitSource;
        let now = self.run.now;
        let mut out = Vec::new();
        for (k, e) in events.iter().enumerate() {
            match e {
                Event::SpellCast { hero, spell } if spell == "lacerate" => {
                    // Shred's sound on each enemy the cast reached: the bleed's looks just before it
                    let reached = events[..k]
                        .iter()
                        .rev()
                        .take_while(|x| !matches!(x, Event::SpellCast { .. }))
                        .filter_map(|x| match x {
                            Event::SpellGo { spell: REND, caster: Unit::Hero(h), hits, .. } if h == hero => Some(hits),
                            _ => None,
                        })
                        .flatten();
                    for u in reached {
                        out.extend(self.guid_of(*u).map(|g| packets::play_object_sound(CAT_ATTACK, g)));
                    }
                }
                Event::SpellGo { spell, caster: caster @ Unit::Hero(hero), reach, .. } if *spell == SLAM => {
                    // bursts of earth over the whole cone on the floor, the client's model for them
                    let Some(from) = self.guid_of(*caster) else {
                        continue;
                    };
                    let Some((at, facing)) = self.run.heroes.iter().find(|h| h.id == *hero).map(|h| (h.pos, h.facing)) else {
                        continue;
                    };
                    let degrees = self.run.content.ability("slam").map_or(60.0, |a| a.kit.cone_degrees);
                    for spot in cone_spots(at, facing, *reach, degrees, SLAM_CONE_SPACING) {
                        out.push(self.burst_look(from, SLAM_BURST_LOOK, spot, SLAM_BURST_SECONDS));
                    }
                }
                Event::SpellGo { spell, caster: caster @ Unit::Hero(hero), aim: Aim::Ground(end), .. } if *spell == DIVINE_STAR => {
                    // the star turns where it lands and flies back to the priest
                    let (Some(from), Some(at)) = (self.guid_of(*caster), self.run.heroes.iter().find(|h| h.id == *hero).map(|h| h.pos)) else {
                        continue;
                    };
                    let speed = self.run.world.missile_speed(DIVINE_STAR);
                    let out_flight = if speed > 0.0 { distance(at, *end) / speed } else { 1.0 };
                    let (turn, create) = self.burst_look_placed(from, DIVINE_STAR, *end, out_flight + DIVINE_STAR_TURN_STAYS);
                    out.push(create);
                    let back = packets::spell_go(turn, DIVINE_STAR, &[from], packets::SpellTarget::Unit(from), None);
                    self.later.push((now + out_flight, back));
                }
                Event::SpellGo { spell, caster: Unit::Pet(_), aim: Aim::Ground(at), .. } if *spell == LASHER_BLAST => {
                    let Some(me) = self.players.first().map(|p| p.guid()) else {
                        continue;
                    };
                    let (spot, create) = self.burst_look_placed(me, LASHER_BLAST, *at, LASHER_BLAST_STAYS);
                    out.push(create);
                    out.push(packets::spell_go(spot, LASHER_BLAST, &[], packets::SpellTarget::Caster, None));
                    out.push(packets::play_object_sound(POISON_CLOUD, spot));
                }
                Event::EnemyHit { id, source: HitSource::Spell(s), .. } if s == "starshards" => {
                    self.starshards_until.insert(*id, now + STARSHARDS_MARK_SECONDS);
                }
                Event::SpellGo { spell, hits, caster, .. } if *spell == MASSIVE_GEYSER => {
                    // the geyser bursts up from the ground under each enemy
                    let Some(from) = self.guid_of(*caster) else {
                        continue;
                    };
                    for u in hits {
                        let Unit::Enemy(id) = u else {
                            continue;
                        };
                        let Some(at) = self.run.enemies.iter().find(|e| e.id == *id).map(|e| e.segment.position(now)) else {
                            continue;
                        };
                        let dest = packets::SpellTarget::Ground([at.0, at.1, self.ground_z(at)]);
                        out.push(packets::spell_go(from, MASSIVE_GEYSER, &[], dest, None));
                    }
                }
                Event::SpellGo { spell, caster, .. } if *spell == EVOCATION => {
                    out.extend(self.guid_of(*caster).map(|g| packets::play_object_sound(OIL_OF_IMMOLATION, g)));
                }
                Event::SpellCast { hero, spell } => {
                    let Some(me) = self.guid_of(Unit::Hero(*hero)) else {
                        continue;
                    };
                    match spell.as_str() {
                        "explosive_shot" => {
                            out.push(packets::play_object_sound(DYNAMITE, me));
                            self.later.push((now + SECOND_BLAST_AFTER, packets::play_object_sound(DYNAMITE, me)));
                            // dynamite blows up at the feet of each enemy the blast hit
                            let blasted: Vec<u32> = events[..k]
                                .iter()
                                .rev()
                                .take_while(|x| !matches!(x, Event::SpellCast { .. }))
                                .filter_map(|x| match x {
                                    Event::EnemyHit { id, by: Some(h), source: HitSource::Spell(s), .. } if h == hero && s == "explosive_shot" => Some(*id),
                                    _ => None,
                                })
                                .collect();
                            for id in blasted {
                                if let Some(them) = self.guid_of(Unit::Enemy(id)) {
                                    out.push(packets::spell_go(them, DYNAMITE_BLAST, &[], packets::SpellTarget::Caster, None));
                                }
                            }
                        }
                        "fan_of_knives" => {
                            out.push(packets::spell_go(me, SPIN, &[], packets::SpellTarget::Caster, None));
                            let Some(at) = self.run.heroes.iter().find(|h| h.id == *hero).map(|h| h.pos) else {
                                continue;
                            };
                            for k in 0..KNIVES {
                                let a = k as f32 * std::f32::consts::TAU / KNIVES as f32;
                                let to = (at.0 + KNIFE_REACH * a.cos(), at.1 + KNIFE_REACH * a.sin());
                                let dest = packets::SpellTarget::Ground([to.0, to.1, self.ground_z(to) + 1.0]);
                                self.later.push((now + KNIVES_AFTER, packets::spell_go(me, THROW, &[], dest, Some(KNIFE))));
                            }
                            self.later.push((now + KNIVES_AFTER, packets::play_object_sound(THROW_ITEM, me)));
                            self.later.push((now + KNIVES_AFTER + 0.15, packets::play_object_sound(DAGGER_IN_FLESH_CRITICAL, me)));
                        }
                        "envenom" => out.push(packets::play_object_sound(POISON_CLOUD, me)),
                        "radiance" => out.push(packets::play_object_sound(LAY_ON_HANDS, me)),
                        _ => {}
                    }
                }
                Event::EnemyHit { id, source: HitSource::Spell(s), .. } if s == "mutilate" => {
                    out.extend(self.guid_of(Unit::Enemy(*id)).map(|g| packets::play_object_sound(DAGGER_IN_FLESH, g)));
                }
                Event::Blocked { hero, by } => {
                    if let (Some(me), Some(them)) = (self.guid_of(Unit::Hero(*hero)), self.guid_of(Unit::Enemy(*by))) {
                        out.push(packets::spell_go(me, HOLY_SHIELD_STRIKE, &[them], packets::SpellTarget::Unit(them), None));
                    }
                }
                Event::SummonAppeared { id, kind: SummonKind::WaterElemental, .. } => {
                    out.extend(self.guid_of(Unit::Pet(*id)).map(|g| packets::play_object_sound(UNDER_WATER, g)));
                }
                _ => {}
            }
        }
        self.starshards_until.retain(|_, until| *until > now);
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.later).into_iter().partition(|(at, _)| *at <= now);
        self.later = later;
        out.extend(due.into_iter().map(|(_, p)| p));
        out
    }
}
