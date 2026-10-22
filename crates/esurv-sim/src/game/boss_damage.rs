//! The damage a hero cannot avoid (docs/game/bosses.md, "Damage a hero
//! cannot avoid"): what a hero standing at a boss takes from it whatever it does, a second, in
//! the boss's attack damages: its swings once caught, its unmarked abilities (a stomp, a charge,
//! a volley's shots at the hero's spot) and the stacks its blows leave. Marked attacks, lasting
//! zones and objects are avoidable (step out, kite, walk over the egg in time) and count apart.
//! A reworked boss keeps this the same as it was; its avoidable parts come on top.

use esurv_content::{Affliction, BossAbility, BossAbilityKind};

use super::boss_abilities::{VOLLEY_RADIUS, VOLLEY_SPREAD};
use super::enemy::BOSS_SWING;

/// The damage a hero standing at a boss takes a second whatever it does, in attack damages:
/// the boss's swing (×`swing_mult` every 2 s), its unmarked abilities over their cooldowns and
/// its blows' stacks once they have built up.
pub fn unavoidable_per_second(kit: &[BossAbility], swing_mult: f32, on_hit: Option<Affliction>) -> f32 {
    let mut per_second = swing_mult / BOSS_SWING;
    for a in kit {
        let per_cast = match a.kind {
            BossAbilityKind::Stomp { damage_mult, .. } | BossAbilityKind::Charge { damage_mult, .. } => damage_mult,
            // the first shot lands on the hero's spot; each other one lands within its circle of
            // the hero as often as its spread allows
            BossAbilityKind::Volley { count, damage_mult, .. } => {
                let near = ((VOLLEY_RADIUS - VOLLEY_SPREAD.0) / (VOLLEY_SPREAD.1 - VOLLEY_SPREAD.0)).clamp(0.0, 1.0);
                damage_mult * (1.0 + count.saturating_sub(1) as f32 * near)
            }
            _ => 0.0,
        };
        per_second += per_cast / a.cooldown.max(0.1);
    }
    if let Some(s) = on_hit {
        let stacks = (s.seconds / BOSS_SWING).min(s.max_stacks as f32);
        per_second += stacks * s.damage_mult / s.every.max(0.1);
    }
    per_second
}

/// What a hero who avoids nothing takes on top a second, in attack damages: each marked attack
/// and rush landing on it, as often as its cooldown allows (its zones and objects not counted).
pub fn avoidable_per_second(kit: &[BossAbility]) -> f32 {
    kit.iter()
        .map(|a| match a.kind {
            BossAbilityKind::Marked { damage_mult, .. } | BossAbilityKind::Rush { damage_mult, .. } | BossAbilityKind::GroundBurst { damage_mult, .. } => damage_mult / a.cooldown.max(0.1),
            _ => 0.0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use esurv_content::{BossAbilityEntry, Content};

    use super::*;

    /// Each Elwynn and Duskwood boss's kit before the rework (release v0.2.2), as its zone file
    /// wrote it.
    const BEFORE: [(&str, &str); 10] = [
        ("Hogger", "[Charge(look_spell: 100), Summon()]"),
        ("Mother Fang", "[Stomp(look_spell: 339), Summon()]"),
        ("Rhahk'Zor", "[Stomp(look_spell: 20549), Charge(look_spell: 100), Summon()]"),
        ("Mr. Smite", "[Charge(look_spell: 100), Stomp(look_spell: 6343), Summon()]"),
        ("Edwin VanCleef", "[Charge(look_spell: 100), Stomp(look_spell: 1680), GroundBurst(telegraph_spell: 5740, impact_spell: 13812), Summon()]"),
        ("Naraxis", "[Stomp(look_spell: 339), Summon()]"),
        ("Mor'Ladim", "[Charge(look_spell: 100), Stomp(look_spell: 1680), Summon()]"),
        ("Stitches", "[Stomp(look_spell: 6343), Charge(look_spell: 100), Summon()]"),
        ("Lupos", "[Charge(look_spell: 100), Stomp(look_spell: 8122), Summon()]"),
        ("Morbent Fel", "[Volley(look_spell: 686), GroundBurst(telegraph_spell: 10, impact_spell: 122), Stomp(look_spell: 1949), Summon()]"),
    ];

    /// Each Alterac Valley, Blackrock Mountain and Ahn'Qiraj boss's kit before the rework
    /// (release v0.2.2), as its zone file wrote it. Stratholme's bosses took Alterac Valley's
    /// slots and keep their kits' unavoidable damage: Barthilas
    /// Korrak's, Anastari Ivus's, Nerub'enkan Lokholar's, Maleki Glacius's, and Ramstein and the
    /// Baron, who share the final slot, each Azuregos's (the Baron's without its Summon, which
    /// counts nothing here and has no standard add count past a zone's fifth boss).
    const BEFORE_ALTERAC_BLACKROCK_AQ: [(&str, &str); 16] = [
        ("Magistrate Barthilas", "[Charge(look_spell: 100), Summon()]"),
        ("Baroness Anastari", "[GroundBurst(telegraph_spell: 16914, impact_spell: 339), Volley(look_spell: 5176), Summon()]"),
        ("Nerub'enkan", "[GroundBurst(telegraph_spell: 10, impact_spell: 122), Stomp(look_spell: 122), Summon()]"),
        ("Maleki the Pallid", "[Stomp(look_spell: 20549), GroundBurst(telegraph_spell: 10, impact_spell: 120), Summon()]"),
        ("Ramstein the Gorger", "[Volley(look_spell: 116), GroundBurst(telegraph_spell: 10, impact_spell: 120), Stomp(look_spell: 122), Summon()]"),
        ("Baron Rivendare", "[Volley(look_spell: 116), GroundBurst(telegraph_spell: 10, impact_spell: 120), Stomp(look_spell: 122)]"),
        ("General Angerforge", "[Charge(look_spell: 100), Stomp(look_spell: 1680), Summon()]"),
        ("Warchief Rend Blackhand", "[Charge(look_spell: 100), Stomp(look_spell: 1680), GroundBurst(telegraph_spell: 5740, impact_spell: 2120), Summon()]"),
        ("Emperor Dagran Thaurissan", "[Volley(look_spell: 133), GroundBurst(telegraph_spell: 5740, impact_spell: 2120), Stomp(look_spell: 11113), Summon()]"),
        ("Majordomo Executus", "[Volley(look_spell: 133), Stomp(look_spell: 1535), GroundBurst(telegraph_spell: 5740, impact_spell: 2120), Summon()]"),
        ("Nefarian", "[Volley(look_spell: 686), GroundBurst(telegraph_spell: 5740, impact_spell: 2120), Stomp(look_spell: 8122), Summon()]"),
        ("Kurinnaxx", "[GroundBurst(telegraph_spell: 16914, impact_spell: 20549), Charge(look_spell: 100), Summon()]"),
        ("General Rajaxx", "[Stomp(look_spell: 6343), Charge(look_spell: 100), Summon()]"),
        ("Princess Huhuran", "[Volley(look_spell: 5176), Stomp(look_spell: 6343), Summon()]"),
        ("Ossirian the Unscarred", "[GroundBurst(telegraph_spell: 16914, impact_spell: 20549), Stomp(look_spell: 20549), Charge(look_spell: 100), Summon()]"),
        ("C'Thun", "[Volley(look_spell: 686), GroundBurst(telegraph_spell: 16914, impact_spell: 8122), Stomp(look_spell: 8122), Summon()]"),
    ];

    /// Every reworked boss takes from a hero what
    /// it cannot avoid as it did before, within 3 %, in every phase of its kit; what a hero who
    /// avoids nothing takes on top is more, but within twice the unavoidable.
    #[test]
    fn a_reworked_boss_keeps_the_damage_a_hero_cannot_avoid() {
        let content = Arc::new(Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap());
        let kinds = content.enemy_kinds.as_ref().unwrap();
        for &(name, before) in BEFORE.iter().chain(&BEFORE_ALTERAC_BLACKROCK_AQ) {
            let (zone, b) = content.zones.iter().find_map(|z| z.bosses.iter().position(|b| b.name == name).map(|b| (z, b))).unwrap();
            let entries: Vec<BossAbilityEntry> = ron::from_str(before).unwrap();
            let mut problems = Vec::new();
            let old: Vec<BossAbility> = entries.iter().map(|e| esurv_content::resolve_ability_for_tests(&zone.id, name, b, e, kinds, &mut problems)).collect();
            assert!(problems.is_empty(), "{problems:?}");
            let then = unavoidable_per_second(&old, 1.0, None);
            let boss = &zone.bosses[b];
            // every phase's kit, wherever its script holds one (a run of waves' end too)
            for kit in boss.kits() {
                let now = unavoidable_per_second(&kit, boss.swing_mult, boss.on_hit);
                assert!((now - then).abs() <= then * 0.03, "{name}: unavoidable {now:.3} a second, was {then:.3}");
                let on_top = avoidable_per_second(&kit);
                assert!(on_top > 0.0 && on_top <= 2.0 * then, "{name}: {on_top:.3} on top of {then:.3}");
            }
        }
    }

    #[test]
    fn the_unavoidable_counts_swings_unmarked_abilities_and_stacks() {
        let stomp = BossAbility { kind: BossAbilityKind::Stomp { look_spell: 1, radius: 10.0, damage_mult: 2.0 }, cooldown: 8.0 };
        assert!((unavoidable_per_second(&[stomp], 1.0, None) - 0.75).abs() < 1e-6);
        let bite = Affliction { look_spell: 589, damage_mult: 0.05, every: 1.0, seconds: 10.0, max_stacks: 5 };
        assert!((unavoidable_per_second(&[], 1.5, Some(bite)) - 1.0).abs() < 1e-6);
    }
}
