//! What the server and the client must agree on to show the starting options' resources and
//! stacks (docs/game/screens.md, "The power bar"): the colour each
//! mana-bar counter paints the player frame's mana bar, the word that names it on the wire, and
//! the aura (a `Spell.dbc` id the client shows in the buff bar) and icon each stacking buff wears.
//! Assets are pointers into the player's 1.12 client.

use crate::{ResourceKind, StackKind};

/// Every kind a hero's mana bar (or its combo point pips) can show.
pub const RESOURCE_KINDS: [ResourceKind; 18] = [
    ResourceKind::Rage,
    ResourceKind::Energy,
    ResourceKind::ComboPoints,
    ResourceKind::HolyPower,
    ResourceKind::HolyShieldStacks,
    ResourceKind::Zeal,
    ResourceKind::InnerFire,
    ResourceKind::Insanity,
    ResourceKind::MaelstromWeapon,
    ResourceKind::CallOfTheAncestors,
    ResourceKind::ArcaneKnowledge,
    ResourceKind::HotStreak,
    ResourceKind::WaterElemental,
    ResourceKind::SoulShards,
    ResourceKind::DemonicSouls,
    ResourceKind::BurningEmbers,
    ResourceKind::LunarPower,
    ResourceKind::EmeraldSlumber,
];

/// Every stacking buff.
pub const STACK_KINDS: [StackKind; 11] = [
    StackKind::Zeal,
    StackKind::QuickShots,
    StackKind::Frenzy,
    StackKind::LockAndLoad,
    StackKind::HolyShield,
    StackKind::InnerFire,
    StackKind::ArcaneKnowledge,
    StackKind::HotStreak,
    StackKind::MaelstromWeapon,
    StackKind::BurningEmbers,
    StackKind::ThickHide,
];

/// The kind's word on the wire (its name in the content files).
pub fn resource_word(kind: ResourceKind) -> String {
    format!("{kind:?}")
}

/// The kind a wire word names.
pub fn resource_of_word(word: &str) -> Option<ResourceKind> {
    RESOURCE_KINDS.into_iter().find(|k| resource_word(*k) == word)
}

/// How the player frame's mana bar shows a kind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BarColour {
    /// The client's own bar for the power type (rage, energy), its own colour.
    Stock,
    /// The mana bar painted this colour (linear 0-1 RGB).
    Painted([f32; 3]),
}

/// The colour of `kind`'s bar ("Power bar colours"). Combo points are
/// pips on the player frame, never the bar.
pub fn bar_colour(kind: ResourceKind) -> BarColour {
    use ResourceKind::*;
    BarColour::Painted(match kind {
        Rage | Energy | ComboPoints => return BarColour::Stock,
        // gold
        HolyPower | HolyShieldStacks | Zeal => [0.95, 0.78, 0.20],
        // fiery orange
        InnerFire => [1.0, 0.50, 0.08],
        // deep purple
        Insanity => [0.45, 0.10, 0.68],
        // teal
        MaelstromWeapon => [0.0, 0.72, 0.68],
        // pale spirit blue-green
        CallOfTheAncestors => [0.55, 0.88, 0.80],
        // arcane pink-violet
        ArcaneKnowledge => [0.86, 0.42, 0.95],
        // flame orange-red
        HotStreak => [1.0, 0.30, 0.05],
        // icy light blue
        WaterElemental => [0.55, 0.85, 1.0],
        // soul violet
        SoulShards => [0.58, 0.25, 0.85],
        // fel green
        DemonicSouls => [0.35, 0.85, 0.10],
        // dark coal red
        BurningEmbers => [0.55, 0.08, 0.04],
        // silvery moonlight blue
        LunarPower => [0.72, 0.80, 1.0],
        // emerald green
        EmeraldSlumber => [0.08, 0.72, 0.36],
    })
}

/// The name a bar shows when it is not its passive's: Moonkin Form's
/// passive is Starshards while its bar is Lunar Power. `None`: the passive's name (or, for rage
/// and energy, the client's own word for the power).
pub fn bar_name(kind: ResourceKind) -> Option<&'static str> {
    match kind {
        ResourceKind::LunarPower => Some("Lunar Power"),
        _ => None,
    }
}

/// The aura a stacking buff wears with `stacks` stacks: the `Spell.dbc` id whose aura the buff bar
/// shows (and whose lasting look plays on the hero), and the icon shown in place of that spell's
/// own when it differs (an icon name in `Interface\Icons`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackLook {
    pub aura: u32,
    pub icon: Option<&'static str>,
}

/// Every aura is a spell the client does not take for a passive one (those it keeps out of the buff
/// bar).
/// The look of `kind` at `stacks` stacks (1 or more). Hot Streak's flames grow with its stacks
///: small flames, bigger, then the whole body ablaze.
pub fn stack_look(kind: StackKind, stacks: u32) -> StackLook {
    use StackKind::*;
    let (aura, icon) = match kind {
        // Ability_Hunter_RunningShot
        QuickShots => (3_049, None),
        // INV_Misc_MonsterClaw_03
        Frenzy => (19_615, None),
        // Spell_Fire_SelfDestruct
        LockAndLoad => (2_124, None),
        // Holy Shield's own, its shield on the hero
        HolyShield => (20_925, None),
        // Spell_Holy_InnerFire, twice: one aura per kind
        Zeal => (1_252, None),
        InnerFire => (1_253, None),
        ArcaneKnowledge => (1_467, Some("INV_Enchant_EssenceMysticalSmall")),
        HotStreak => (hot_streak_aura(stacks), Some("Spell_Fire_Fireball02")),
        MaelstromWeapon => (maelstrom_aura(stacks), Some("INV_Hammer_05")),
        BurningEmbers => (1_571, Some("Spell_Fire_Fire")),
        ThickHide => (1_432, Some("INV_Misc_Pelt_Bear_03")),
    };
    StackLook { aura, icon }
}

/// Hot Streak's three flame looks, smallest first: Ignite's flames at the feet, Flame Buffet's
/// (the same, grown by the server's look style), then Flamethrower's whole body ablaze.
pub const HOT_STREAK_AURAS: [u32; 3] = [12_654, 23_341, 25_029];

fn hot_streak_aura(stacks: u32) -> u32 {
    HOT_STREAK_AURAS[(stacks.clamp(1, 3) - 1) as usize]
}

/// Maelstrom Weapon's five looks, one per stack: a little tornado at the hero's feet (the
/// Windfury look's cyclone), grown by the server's look style with each stack.
pub const MAELSTROM_AURAS: [u32; 5] = [8_233, 8_236, 10_484, 16_361, 10_092];

fn maelstrom_aura(stacks: u32) -> u32 {
    MAELSTROM_AURAS[(stacks.clamp(1, 5) - 1) as usize]
}

/// The Holy Orb on the ground: a dynamic object of this `Spell.dbc` id, which the run uses for
/// nothing else, drawn by the client as [`HOLY_ORB_MODELS`].
pub const HOLY_ORB_LOOK: u32 = 24_865;

/// The Holy Orb's models, drawn together: a round golden glow from the
/// player's client (a weapon enchantment's, picked among glowing orbs, screens/level1
/// `holy-orb-candidate-*`) with a holy bolt's ball inside it, at half its own size and dimmed.
pub const HOLY_ORB_MODELS: [LookModel; 2] = [
    LookModel { path: "Spells\\Enchantments\\YellowGlow_High.mdx", scale: 1.0, lift: 0.6, tint: None },
    LookModel { path: "Spells\\Holy_Missile_Low.mdx", scale: 0.5, lift: 0.6, tint: Some([0.55; 3]) },
];

/// Slam's bursts of earth over its whole cone on the floor: dynamic objects of this `Spell.dbc` id
/// (Quake), which the run uses for nothing else, put down [`SLAM_CONE_SPACING`] yards apart across
/// the cone for [`SLAM_BURST_SECONDS`] and each drawn once by the client as the first of
/// [`SLAM_BURSTS`].
pub const SLAM_BURST_LOOK: u32 = 26_093;
pub const SLAM_BURST_SECONDS: f32 = 2.5;
pub const SLAM_CONE_SPACING: f32 = 2.5;

/// The looks Slam's burst can take, by name: the first is the one drawn, the others are kept for
/// comparing (screens/level1 `warrior-slam-alt-*`, `ESURV_SLAM_BURST` on the client).
pub const SLAM_BURSTS: [(&str, LookModel); 4] = [
    // Quake's earth burst at 0.3, its rocks and dust with it (the one drawn)
    ("quake", LookModel { path: "Spells\\BurrowEarth_Brown_Missile.mdx", scale: 0.3, lift: 0.0, tint: None }),
    // Dust Field's ring of fine dust at half size: a small puff at the enemy's feet
    ("dust_field", LookModel { path: "Spells\\DustNova_Cast_Base.mdx", scale: 0.5, lift: 0.0, tint: None }),
    // the dust a charge kicks up as it lands, at twice its size
    ("charge_dust", LookModel { path: "Spells\\DustCloud_Land.mdx", scale: 2.0, lift: 0.0, tint: None }),
    // War Stomp's shock ring on the ground at 0.35
    ("war_stomp", LookModel { path: "Spells\\ThunderClap_Cast_Base.mdx", scale: 0.35, lift: 0.0, tint: None }),
];

/// A boss mark's looks (`enemy_kinds.ron` `marked`): Consecration's ranks 5 and 4 as dynamic
/// objects (the paladin's own, 26573, keeps its look), drawn by the client as Kel'Thuzad's
/// Shadow Fissure in its own purple. The fissure alone is a handful of faint
/// sparks that trace no shape, so each disc that covers a shape gets a ground glow in the
/// fissure's purple under it (Consecration's flat ground, recoloured) with the fissure's sparks on
/// the warning; the fill grows inside in a brighter violet.
pub const MARK_WARNING_LOOK: u32 = 20_924;
pub const MARK_FILL_LOOK: u32 = 20_923;

/// The Shadow Fissure's effect model (`SpellVisualEffectName` 3035, kit 6624; Shadow Fissure,
/// 27810, summons an invisible creature it shows on).
pub const SHADOW_FISSURE_MODEL: &str = "Spells\\Shadow_Fissure_Base.mdx";
/// Consecration's flat ground (`SpellVisualEffectName` 2542), the marks' glow.
pub const MARK_GROUND_MODEL: &str = "Spells\\Consecration_Impact_Base.mdx";

/// The fissure on a mark: the server scales a mark's look by the disc's radius over
/// Consecration's 8 yards; the fissure's core is about a yard round and its two ray emitters reach
/// about 28 yards, so at 1 its core is a small spark at each disc's heart and its rays stay short
/// (a disc of 2.5 yards: rays of about 9), keeping the shape readable.
pub const MARK_FISSURE_SCALE: f32 = 1.0;
/// The glow's size: Consecration's ground is drawn about 11.5 yards round, so 0.49 lays it at 70 %
/// of each disc (11.5 × 0.49 / 8): the discs (1.4 radii apart) a row of purple spots tracing the
/// shape.
pub const MARK_GROUND_SCALE: f32 = 0.49;
/// The Shadow Fissure's own purple (its sparks' colour) for the warning's glow, and a brighter
/// violet for the fill (linear RGB hues).
pub const MARK_PURPLE: [f32; 3] = [0.56, 0.03, 0.43];
pub const MARK_FILL_VIOLET: [f32; 3] = [0.8, 0.25, 1.0];

/// One model a mark is drawn with; `recolor`: its colours take `look.tint` as their hue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkLook {
    pub look: LookModel,
    pub recolor: bool,
}

/// The marks' models: the warning's purple glow and fissure sparks, the fill's violet glow.
pub const MARK_MODELS: [(u32, &[MarkLook]); 2] = [
    (
        MARK_WARNING_LOOK,
        &[
            MarkLook { look: LookModel { path: MARK_GROUND_MODEL, scale: MARK_GROUND_SCALE, lift: 0.0, tint: Some(MARK_PURPLE) }, recolor: true },
            MarkLook { look: LookModel { path: SHADOW_FISSURE_MODEL, scale: MARK_FISSURE_SCALE, lift: 0.0, tint: None }, recolor: false },
        ],
    ),
    (
        MARK_FILL_LOOK,
        &[MarkLook { look: LookModel { path: MARK_GROUND_MODEL, scale: MARK_GROUND_SCALE, lift: 0.0, tint: Some(MARK_FILL_VIOLET) }, recolor: true }],
    ),
];

/// A model the client draws for a look on the ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookModel {
    /// The M2 path in the player's client.
    pub path: &'static str,
    /// Its size; its particles are grown by it too.
    pub scale: f32,
    /// How far above the ground it floats, yards.
    pub lift: f32,
    /// A colour multiplier (linear RGB) on its particles.
    pub tint: Option<[f32; 3]>,
}

/// Buffs of card spells whose aura (the spell's `look_spell`) wears an icon not its spell's own
///: Berserk's sweeping aura wears the Idol of Ferocity.
pub const BUFF_ICONS: [(u32, &str); 1] = [(12_292, "INV_Relics_IdolofFerocity")];

/// Debuffs on enemies whose aura (the spell's `look_spell`) wears an icon not its spell's own:
/// Deadly Poison's green cloud (Poison State's) wears Deadly Poison's icon.
pub const DEBUFF_ICONS: [(u32, &str); 1] = [(12_881, "Ability_Rogue_DualWeild")];

/// Every aura a stacking buff, a card's buff or a debuff on an enemy can wear whose icon is not
/// its spell's own, with the icon.
pub fn icon_overrides() -> Vec<(u32, &'static str)> {
    let mut out = BUFF_ICONS.to_vec();
    out.extend(DEBUFF_ICONS);
    for kind in STACK_KINDS {
        for stacks in 1..=5 {
            let look = stack_look(kind, stacks);
            if let Some(icon) = look.icon {
                if !out.contains(&(look.aura, icon)) {
                    out.push((look.aura, icon));
                }
            }
        }
    }
    out
}

/// The stacking buff an aura is, if it is one.
pub fn stack_of_aura(aura: u32) -> Option<StackKind> {
    STACK_KINDS.into_iter().find(|k| (1..=5).any(|s| stack_look(*k, s).aura == aura))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_reads_back_from_its_word() {
        for k in RESOURCE_KINDS {
            assert_eq!(resource_of_word(&resource_word(k)), Some(k));
        }
        assert_eq!(resource_of_word("Mana"), None);
    }

    #[test]
    fn each_stack_buff_wears_its_own_aura() {
        let mut auras = Vec::new();
        for k in STACK_KINDS {
            for s in 1..=5 {
                let a = stack_look(k, s).aura;
                assert_eq!(stack_of_aura(a), Some(k), "{k:?}");
                if !auras.contains(&a) {
                    auras.push(a);
                }
            }
        }
        assert_eq!(auras.len(), STACK_KINDS.len() + 6, "Hot Streak has three, Maelstrom Weapon five");
        assert_eq!(stack_look(StackKind::HotStreak, 9).aura, HOT_STREAK_AURAS[2]);
        assert!(icon_overrides().contains(&(MAELSTROM_AURAS[4], "INV_Hammer_05")));
        assert!(icon_overrides().contains(&(12_292, "INV_Relics_IdolofFerocity")), "Berserk's buff");
    }

    #[test]
    fn rage_and_energy_keep_the_stock_bar() {
        assert_eq!(bar_colour(ResourceKind::Rage), BarColour::Stock);
        assert_eq!(bar_colour(ResourceKind::Energy), BarColour::Stock);
        assert!(matches!(bar_colour(ResourceKind::BurningEmbers), BarColour::Painted(_)));
    }
}
