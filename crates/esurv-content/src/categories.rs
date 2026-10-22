//! Uniform changes per spell category (`content/spell_categories.ron`), applied to the spells'
//! numbers when content loads. All 1.0 leaves the base numbers unchanged.

use anyhow::{bail, Result};

use crate::{Ability, CategoryTuning, Delivery};

/// Deliveries whose damage comes in ticks over `duration`. A `Nova` with 2 or more ticks repeats
/// around the hero over `duration`; a `SelfEffect`'s effect lasts as long as the spell; a
/// `Cone`'s `range` is its size.
const TICKING: [Delivery; 3] = [Delivery::GroundArea, Delivery::DamageOverTime, Delivery::Totem];

/// Multiplies each spell's numbers by every line of `tuning` whose category it is in.
pub fn apply_category_tuning(abilities: &mut [Ability], tuning: &[CategoryTuning]) -> Result<()> {
    for (i, t) in tuning.iter().enumerate() {
        if tuning[..i].iter().any(|u| u.category == t.category) {
            bail!("spell_categories.ron lists {:?} twice", t.category);
        }
        if t.multipliers().iter().any(|m| !m.is_finite() || *m < 0.0) {
            bail!("spell_categories.ron: {:?} has a negative or invalid number", t.category);
        }
    }
    for a in abilities {
        let mut m = [1.0f32; 7];
        for t in tuning.iter().filter(|t| a.categories.contains(&t.category)) {
            for (x, y) in m.iter_mut().zip(t.multipliers()) {
                *x *= y;
            }
        }
        let [damage, cooldown, range, radius, duration, effect, effect_seconds] = m;
        a.damage *= damage;
        if let Some(burn) = a.burn.as_mut() {
            burn.damage *= damage;
        }
        a.impact *= damage;
        a.cooldown *= cooldown;
        a.range *= range;
        if a.delivery == Delivery::Cone {
            a.range *= radius;
        }
        a.radius *= radius;
        if duration != 1.0 {
            a.duration *= duration;
            if TICKING.contains(&a.delivery) && a.ticks > 0 {
                let ticks = ((a.ticks as f32 * duration).round() as u32).max(1);
                a.damage *= ticks as f32 / a.ticks as f32;
                a.ticks = ticks;
            } else if a.delivery == Delivery::Nova && a.ticks > 1 {
                // a repeating nova keeps its spacing and the damage of each nova
                a.ticks = ((a.ticks as f32 * duration).round() as u32).max(1);
            }
        }
        let seconds = if a.delivery == Delivery::SelfEffect { duration } else { effect_seconds };
        a.effect = a.effect.scaled(effect, seconds);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{School, SpellCategory, SpellEffect, Upgrade};

    fn spell(delivery: Delivery, categories: &[SpellCategory], effect: SpellEffect) -> Ability {
        Ability {
            id: "t".into(),
            name_ref: Default::default(),
            name: String::new(),
            description: String::new(),
            icon: String::new(),
            look_spell: 0,
            school: School::Shadow,
            delivery,
            categories: categories.to_vec(),
            tags: vec![],
            upgrades: vec![Upgrade::Power],
            free_casts_can_proc: false,
            damage: 60.0,
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

    fn line(category: SpellCategory) -> CategoryTuning {
        CategoryTuning { category, damage: 1.0, cooldown: 1.0, range: 1.0, radius: 1.0, duration: 1.0, effect: 1.0, effect_seconds: 1.0 }
    }

    #[test]
    fn a_line_changes_only_its_category() {
        let mut v = vec![
            spell(Delivery::DamageOverTime, &[SpellCategory::DamageOverTime], SpellEffect::None),
            spell(Delivery::Projectile, &[SpellCategory::DirectDamage], SpellEffect::None),
        ];
        let t = CategoryTuning { damage: 2.0, cooldown: 0.5, ..line(SpellCategory::DamageOverTime) };
        apply_category_tuning(&mut v, &[t]).unwrap();
        assert_eq!((v[0].damage, v[0].cooldown), (120.0, 1.0));
        assert_eq!((v[1].damage, v[1].cooldown), (60.0, 2.0));
    }

    #[test]
    fn damage_scales_a_ground_area_s_impact_and_duration_does_not() {
        let mut v = vec![Ability { impact: 46.0, ..spell(Delivery::GroundArea, &[SpellCategory::DamageOverTime], SpellEffect::None) }];
        let t = CategoryTuning { damage: 2.0, duration: 1.5, ..line(SpellCategory::DamageOverTime) };
        apply_category_tuning(&mut v, &[t]).unwrap();
        assert_eq!(v[0].impact, 92.0);
    }

    #[test]
    fn longer_effects_over_time_keep_their_tick_size() {
        let mut v = vec![spell(Delivery::DamageOverTime, &[SpellCategory::DamageOverTime], SpellEffect::None)];
        apply_category_tuning(&mut v, &[CategoryTuning { duration: 1.5, ..line(SpellCategory::DamageOverTime) }]).unwrap();
        // 6 ticks of 10 over 6 s become 9 ticks of 10 over 9 s
        assert_eq!((v[0].duration, v[0].ticks, v[0].damage), (9.0, 9, 90.0));
    }

    #[test]
    fn a_longer_repeating_nova_adds_novas_of_the_same_size() {
        let mut v = vec![Ability { duration: 3.0, ticks: 3, ..spell(Delivery::Nova, &[SpellCategory::AreaOfEffect], SpellEffect::None) }];
        apply_category_tuning(&mut v, &[CategoryTuning { duration: 2.0, ..line(SpellCategory::AreaOfEffect) }]).unwrap();
        // 3 novas of 60 a second apart become 6 novas of 60 a second apart
        assert_eq!((v[0].duration, v[0].ticks, v[0].damage), (6.0, 6, 60.0));
    }

    #[test]
    fn several_categories_multiply() {
        let mut v = vec![spell(Delivery::Nova, &[SpellCategory::DirectDamage, SpellCategory::AreaOfEffect], SpellEffect::None)];
        let t = [
            CategoryTuning { damage: 1.5, ..line(SpellCategory::DirectDamage) },
            CategoryTuning { damage: 0.5, radius: 2.0, ..line(SpellCategory::AreaOfEffect) },
        ];
        apply_category_tuning(&mut v, &t).unwrap();
        assert_eq!((v[0].damage, v[0].radius), (45.0, 16.0));
    }

    #[test]
    fn effects_scale_and_self_effects_follow_duration() {
        let mut v = vec![
            spell(Delivery::Projectile, &[SpellCategory::CrowdControl], SpellEffect::Slow { amount: 0.6, seconds: 4.0 }),
            spell(Delivery::SelfEffect, &[SpellCategory::Buff], SpellEffect::DamageBuff { amount: 0.3, seconds: 8.0 }),
        ];
        let t = [
            CategoryTuning { effect: 2.0, effect_seconds: 0.5, ..line(SpellCategory::CrowdControl) },
            CategoryTuning { duration: 1.5, effect_seconds: 3.0, ..line(SpellCategory::Buff) },
        ];
        apply_category_tuning(&mut v, &t).unwrap();
        assert_eq!(v[0].effect, SpellEffect::Slow { amount: 1.0, seconds: 2.0 }); // slows stop at 100 %
        assert_eq!(v[1].effect, SpellEffect::DamageBuff { amount: 0.3, seconds: 12.0 });
    }

    #[test]
    fn bad_lines_are_refused() {
        let mut v = vec![];
        let twice = [line(SpellCategory::Melee), line(SpellCategory::Melee)];
        assert!(apply_category_tuning(&mut v, &twice).is_err());
        assert!(apply_category_tuning(&mut v, &[CategoryTuning { damage: -1.0, ..line(SpellCategory::Melee) }]).is_err());
    }
}
