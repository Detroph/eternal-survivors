//! Amulets: one worn per run. A cache offer rolls a rarity and picks an amulet of that rarity;
//! at legendary rarity the legendary amulets of the spells the hero knows join them (one per
//! spell). Offered again while worn, an amulet is empowered and its
//! numbers grow toward three times their own.

use esurv_content::{Ability, Amulet, Content, Growth, LegendaryAmulet, Milestone, Rarity, TalentEffect, Upgrade};

use crate::rng::Rng;

/// How fast each `Growth` grows, fastest first.
pub const EMPOWER_RATE: [f32; 5] = [0.6, 0.5, 0.35, 0.25, 0.2];
/// Empowered numbers approach this multiple of their own.
pub const EMPOWER_LIMIT: f32 = 3.0;
/// An empowered cooldown cut never passes this.
pub const MAX_COOLDOWN_CUT: f32 = 0.5;

/// The multiplier on a worn amulet's numbers at `rank` (1 when first taken).
pub fn empower_factor(growth: Growth, rank: u32) -> f32 {
    let j = EMPOWER_RATE[growth.index()];
    EMPOWER_LIMIT - (EMPOWER_LIMIT - 1.0) * (1.0 - j * 0.5).powi(rank.max(1) as i32 - 1)
}

/// A milestone with the empower multiplier on its main number, and on those of the milestones
/// it holds (`Every`, `WhileBuff`).
fn empowered_milestone(m: &Milestone, k: f32) -> Milestone {
    let mut m = m.clone();
    if m.grows_when_empowered() {
        if let Some(v) = m.rank_value_mut() {
            *v *= k;
        }
    }
    if let Milestone::Every { then, .. } | Milestone::WhileBuff { then, .. } = &mut m {
        for inner in then.iter_mut() {
            *inner = empowered_milestone(inner, k);
        }
    }
    m
}

/// One effect with the empower multiplier applied.
pub fn empowered(effect: &TalentEffect, k: f32) -> TalentEffect {
    match effect {
        TalentEffect::Stat { stat, per_rank } => TalentEffect::Stat { stat: *stat, per_rank: per_rank * k },
        TalentEffect::AbilityMod { filter, kind, per_rank } => {
            let v = per_rank * k;
            let v = match kind {
                Upgrade::Cooldown => v.min(MAX_COOLDOWN_CUT),
                k if k.is_count() => (v + 0.0001).floor(),
                _ => v,
            };
            TalentEffect::AbilityMod { filter: filter.clone(), kind: *kind, per_rank: v }
        }
        TalentEffect::CooldownSeconds { filter, per_rank } => TalentEffect::CooldownSeconds { filter: filter.clone(), per_rank: per_rank * k },
        TalentEffect::Milestone(m) => TalentEffect::Milestone(empowered_milestone(m, k)),
    }
}

/// The legendary amulet `l` as an amulet to offer and wear (id `legendary:<main spell>`).
pub fn legendary_for(l: &LegendaryAmulet) -> Amulet {
    Amulet { id: l.id(), name: l.name.clone(), icon: l.icon.clone(), text: l.text.clone(), effects: l.effects.clone(), rarity: l.rarity, growth: l.growth }
}

/// The legendary amulet with item id `id` (`legendary:<spell>`), or that remakes spell `id`.
pub fn legendary_named<'a>(content: &'a Content, id: &str) -> Option<&'a LegendaryAmulet> {
    let spell = id.strip_prefix("legendary:").unwrap_or(id);
    content
        .legendary_amulets
        .iter()
        .find(|l| l.main_spell() == spell)
        .or_else(|| content.legendary_amulets.iter().find(|l| l.spells.iter().any(|s| s == spell)))
}

/// The legendary amulets of the spells in `known`, once each, in the content's order (a strike
/// and its finisher share one). The pool follows the class, the
/// level-1 pick and the cards taken.
pub fn legendaries_of<'a>(content: &'a Content, known: &[&Ability]) -> Vec<&'a LegendaryAmulet> {
    content.legendary_amulets.iter().filter(|l| l.spells.iter().any(|s| known.iter().any(|a| a.id == *s))).collect()
}

/// The amulets a cache offer of `rarity` can show: the table's amulets of that rarity, and at
/// legendary rarity the legendary amulet of every spell the hero knows. The worn amulet is left
/// out; if nothing is left, the first amulet of the table stands in.
pub fn amulet_candidates(content: &Content, rarity: Rarity, worn: Option<&str>, known: &[&Ability]) -> Vec<Amulet> {
    let mut out: Vec<Amulet> = content.amulets.iter().filter(|a| a.rarity == rarity).cloned().collect();
    if rarity == Rarity::Legendary {
        out.extend(legendaries_of(content, known).into_iter().map(legendary_for));
    }
    out.retain(|a| Some(a.id.as_str()) != worn);
    if out.is_empty() {
        out.extend(content.amulets.first().cloned());
    }
    out
}

/// Picks one amulet for a cache offer, uniformly among the candidates.
pub fn pick_amulet(rng: &mut Rng, candidates: &[Amulet]) -> Option<Amulet> {
    if candidates.is_empty() {
        return None;
    }
    Some(candidates[(rng.next_u64() % candidates.len() as u64) as usize].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_content::SpellFilter;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    #[test]
    fn empower_grows_toward_three() {
        assert_eq!(empower_factor(Growth::Fastest, 1), 1.0);
        assert_eq!(empower_factor(Growth::Fastest, 0), 1.0);
        // row 0: 3 - 2 x 0.7 = 1.6, then 3 - 2 x 0.49 = 2.02
        assert!((empower_factor(Growth::Fastest, 2) - 1.6).abs() < 1e-5);
        assert!((empower_factor(Growth::Fastest, 3) - 2.02).abs() < 1e-5);
        // row 4 (legendary): 3 - 2 x 0.9 = 1.2
        assert!((empower_factor(Growth::Slowest, 2) - 1.2).abs() < 1e-5);
        assert!(empower_factor(Growth::Fastest, 60) <= 3.0 && empower_factor(Growth::Fastest, 60) > 2.999);
    }

    #[test]
    fn empowered_mods_round_targets_and_cap_cooldowns() {
        let t = TalentEffect::AbilityMod { filter: SpellFilter::Any, kind: Upgrade::Targets, per_rank: 1.0 };
        assert_eq!(empowered(&t, 1.6), TalentEffect::AbilityMod { filter: SpellFilter::Any, kind: Upgrade::Targets, per_rank: 1.0 });
        let c = TalentEffect::AbilityMod { filter: SpellFilter::Any, kind: Upgrade::Cooldown, per_rank: 0.35 };
        assert_eq!(empowered(&c, 2.0), TalentEffect::AbilityMod { filter: SpellFilter::Any, kind: Upgrade::Cooldown, per_rank: 0.5 });
    }

    #[test]
    fn a_legendary_remakes_its_spells_by_name() {
        let c = content();
        let l = legendary_named(&c, "legendary:fireball").unwrap();
        let a = legendary_for(l);
        assert_eq!(a.id, "legendary:fireball");
        assert_eq!((a.rarity, a.growth), (Rarity::Legendary, Growth::Slowest));
        // a finisher finds the legendary it shares with its strike
        assert_eq!(legendary_named(&c, "envenom").unwrap().id(), "legendary:mutilate");
        assert!(legendary_named(&c, "no_such_spell").is_none());
    }

    #[test]
    fn the_legendary_pool_follows_the_known_spells() {
        let c = content();
        let ids = |known: &[&str]| -> Vec<String> {
            let known: Vec<&Ability> = known.iter().map(|s| c.ability(s).unwrap()).collect();
            amulet_candidates(&c, Rarity::Legendary, None, &known).into_iter().map(|a| a.id).filter(|id| id.starts_with("legendary:")).collect()
        };
        // a level-1 pick brings its own; its strike and finisher bring one between them
        assert_eq!(ids(&["slam"]), ["legendary:slam"]);
        assert_eq!(ids(&["mutilate", "envenom"]), ["legendary:mutilate"]);
        // a card taken adds its own
        assert_eq!(ids(&["slam", "cleave"]), ["legendary:slam", "legendary:cleave"]);
        // a form's card pool brings its own (the bear's Lacerate)
        assert_eq!(ids(&["maul", "lacerate"]), ["legendary:maul", "legendary:lacerate"]);
        assert!(ids(&[]).is_empty());
        // only at legendary rarity
        let fireball = [c.ability("fireball").unwrap()];
        assert!(amulet_candidates(&c, Rarity::Epic, None, &fireball).iter().all(|a| !a.id.starts_with("legendary:")));
    }

    #[test]
    fn empowering_reaches_the_milestones_a_legendary_holds() {
        use esurv_content::{Counted, SpellFilter, Trigger};
        let inner = Milestone::Splash { share: 0.5, radius: 5.0, look_spell: None, requires_aura: None, trigger: Trigger::Hit, filter: SpellFilter::Any, effects: vec![] };
        let every = Milestone::Every { count: 4, per: Counted::Casts, per_enemy: false, chance: 1.0, first_only: false, filter: SpellFilter::Any, then: vec![inner] };
        let TalentEffect::Milestone(Milestone::Every { then, .. }) = empowered(&TalentEffect::Milestone(every), 2.0) else { panic!("an Every") };
        assert!(matches!(then[0], Milestone::Splash { share, .. } if (share - 1.0).abs() < 1e-6));
    }

    #[test]
    fn candidates_skip_the_worn_one_and_fall_back() {
        let c = content();
        let common: Vec<_> = c.amulets.iter().filter(|a| a.rarity == Rarity::Common).collect();
        let worn = common[0].id.clone();
        let got = amulet_candidates(&c, Rarity::Common, Some(&worn), &[]);
        assert!(got.iter().all(|a| a.id != worn));
        assert_eq!(got.len(), common.len() - 1);
        let known = [c.ability("fireball").unwrap()];
        let legendary = amulet_candidates(&c, Rarity::Legendary, None, &known);
        assert!(legendary.iter().any(|a| a.id == "legendary:fireball"));
        let worn = amulet_candidates(&c, Rarity::Legendary, Some("legendary:fireball"), &known);
        assert!(worn.iter().all(|a| a.id != "legendary:fireball"), "the worn legendary is not offered again");
    }
}
