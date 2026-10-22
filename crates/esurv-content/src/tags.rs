//! The spell tag coefficients (`balance.ron` `spell_tags`), applied to
//! the spells' numbers when content loads, after the category tuning. All 1.0 changes nothing.
//!
//! A coefficient scales only the part of a spell its tag is about: a damage tag the damage,
//! `Sustain` the healing, `Buffs` the buff amount, `Defensive` the shield, weakening and
//! reflected damage. Cooldowns and durations never change. The class's damage knob
//! (`Class::spell_damage_scale`) multiplies on top during the run.

use anyhow::{bail, Result};

use crate::{Ability, Pet, SpellEffect, SpellTag, SpellTagCoefficients};

/// Multiplies each spell's numbers by the coefficients of its tags, and every pet's damage by
/// the `Pet` coefficient (hero pets are summoned by talents, and fight like the `Pet` spells).
pub fn apply_spell_tags(abilities: &mut [Ability], pets: &mut [Pet], coefficients: &SpellTagCoefficients) -> Result<()> {
    for tag in SpellTag::DAMAGE.into_iter().chain([SpellTag::Sustain, SpellTag::Buffs, SpellTag::Defensive]) {
        let c = coefficients.of(tag);
        if !c.is_finite() || c < 0.0 {
            bail!("balance.ron spell_tags: {tag:?} has a negative or invalid number");
        }
    }
    for a in abilities {
        let tags = &a.tags;
        let damage = coefficients.product(tags, SpellTag::is_damage);
        a.damage *= damage;
        // a hit's burn is part of its damage
        if let Some(burn) = a.burn.as_mut() {
            burn.damage *= damage;
        }
        // so is a ground area's impact
        a.impact *= damage;
        let strength = match a.effect {
            SpellEffect::Heal { .. } | SpellEffect::HealFromDamage { .. } | SpellEffect::HealingCharges { .. } => {
                coefficients.product(tags, |t| t == SpellTag::Sustain)
            }
            SpellEffect::DamageBuff { .. } | SpellEffect::HasteBuff { .. } => coefficients.product(tags, |t| t == SpellTag::Buffs),
            SpellEffect::Shield { .. } | SpellEffect::Weaken { .. } | SpellEffect::Thorns { .. } => {
                coefficients.product(tags, |t| t == SpellTag::Defensive)
            }
            _ => 1.0,
        };
        if strength != 1.0 {
            a.effect = a.effect.scaled(strength, 1.0);
        }
    }
    for p in pets {
        p.damage *= coefficients.pet;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Delivery, School, Upgrade};

    fn spell(damage: f32, tags: &[SpellTag], effect: SpellEffect) -> Ability {
        Ability {
            id: "t".into(),
            name_ref: Default::default(),
            name: String::new(),
            description: String::new(),
            icon: String::new(),
            look_spell: 0,
            school: School::Shadow,
            delivery: Delivery::Projectile,
            categories: vec![],
            tags: tags.to_vec(),
            upgrades: vec![Upgrade::Power],
            free_casts_can_proc: false,
            damage,
            cooldown: 2.0,
            range: 30.0,
            radius: 8.0,
            targets: 1,
            duration: 6.0,
            ticks: 6,
            pulse_spell: None,
            effect,
            heal_below: None,
            double_on_low_health: false,
            burn: None,
            impact: 0.0,
            kit: Default::default(),
        }
    }

    #[test]
    fn each_coefficient_scales_only_its_part() {
        use SpellTag::*;
        let mut v = vec![
            spell(100.0, &[Nuke, Sustain], SpellEffect::HealFromDamage { share: 0.5 }),
            spell(100.0, &[Dot, Defensive], SpellEffect::Weaken { amount: 0.2, seconds: 6.0 }),
            spell(0.0, &[Buffs], SpellEffect::DamageBuff { amount: 0.3, seconds: 12.0 }),
            spell(0.0, &[Defensive], SpellEffect::Shield { share: 0.25, seconds: 10.0 }),
            spell(50.0, &[Cleave], SpellEffect::Slow { amount: 0.5, seconds: 3.0 }),
        ];
        let c = SpellTagCoefficients { nuke: 2.0, sustain: 3.0, dot: 0.5, defensive: 2.0, buffs: 2.0, ..Default::default() };
        apply_spell_tags(&mut v, &mut [], &c).unwrap();
        assert_eq!((v[0].damage, v[0].effect), (200.0, SpellEffect::HealFromDamage { share: 1.5 }));
        assert_eq!((v[1].damage, v[1].effect), (50.0, SpellEffect::Weaken { amount: 0.4, seconds: 6.0 }));
        assert_eq!(v[2].effect, SpellEffect::DamageBuff { amount: 0.6, seconds: 12.0 });
        assert_eq!(v[3].effect, SpellEffect::Shield { share: 0.5, seconds: 10.0 });
        assert_eq!((v[4].damage, v[4].effect), (50.0, SpellEffect::Slow { amount: 0.5, seconds: 3.0 }));
        for a in &v {
            assert_eq!((a.cooldown, a.duration, a.ticks), (2.0, 6.0, 6), "cooldowns and durations never change");
        }
    }

    #[test]
    fn a_damage_coefficient_scales_a_hit_s_burn_with_the_hit() {
        let burn = Some(crate::Burn { damage: 160.0, duration: 5.0, ticks: 5 });
        let mut v = vec![Ability { burn, ..spell(280.0, &[SpellTag::Nuke], SpellEffect::None) }];
        let c = SpellTagCoefficients { nuke: 2.0, sustain: 3.0, ..Default::default() };
        apply_spell_tags(&mut v, &mut [], &c).unwrap();
        assert_eq!(v[0].damage, 560.0);
        assert_eq!(v[0].burn, Some(crate::Burn { damage: 320.0, duration: 5.0, ticks: 5 }), "its length never changes");
    }

    #[test]
    fn a_damage_coefficient_scales_a_ground_area_s_impact_with_its_ticks() {
        let mut v = vec![Ability { delivery: Delivery::GroundArea, impact: 46.0, ..spell(70.0, &[SpellTag::Area], SpellEffect::None) }];
        let c = SpellTagCoefficients { area: 2.0, ..Default::default() };
        apply_spell_tags(&mut v, &mut [], &c).unwrap();
        assert_eq!((v[0].damage, v[0].impact), (140.0, 92.0));
    }

    #[test]
    fn negative_coefficients_are_refused() {
        let c = SpellTagCoefficients { area: -1.0, ..Default::default() };
        assert!(apply_spell_tags(&mut [], &mut [], &c).is_err());
    }
}
