//! Every line the run says, in one place: HUD messages, shouts, and centre-screen lines. Our own
//! wording.
//! In a centre-screen line `%s` stands for the speaker's name.

// ---- the hero's own chat ----

/// What one level brought: damage (in percent), maximum health and health per second.
pub fn level_up_gains(damage_percent: f32, max_health: f32, regen: f32) -> String {
    format!(
        "Level up! +{}% damage, +{} maximum health, +{:.2} health per second.",
        trimmed(damage_percent),
        trimmed(max_health),
        regen
    )
}

/// A number with at most one decimal, and none when it is whole.
fn trimmed(v: f32) -> String {
    let tenths = (v * 10.0).round() / 10.0;
    if tenths.fract() == 0.0 { format!("{tenths:.0}") } else { format!("{tenths:.1}") }
}

// ---- the end of a run (the hero's own chat) ----

/// What a run earned a class (`classes`: its plural, "Mages"): talent experience, and the talent
/// points that bought.
pub fn talent_experience_earned(classes: &str, experience: u32, new_points: u32) -> String {
    match new_points {
        0 => format!("|cff71d5ff{classes} gain {experience} talent experience.|r"),
        1 => format!("|cff71d5ff{classes} gain {experience} talent experience and a new talent point to spend in the armory.|r"),
        n => format!("|cff71d5ff{classes} gain {experience} talent experience and {n} new talent points to spend in the armory.|r"),
    }
}

/// A win opened the next zone to a class (`classes`: its plural).
pub fn zone_opened(zone: &str, classes: &str) -> String {
    format!("|cff40ff40{zone} now opens to {classes}.|r")
}

/// A boss dropped a class gear piece for the hero; `new` when the collection did not have it.
pub fn gear_won(piece: &str, new: bool) -> String {
    if new { format!("|cffa335ee{piece}|r joins your collection.") } else { format!("|cffa335ee{piece}|r is already in your collection.") }
}

// ---- HUD messages ----

/// A cache the hero cannot pay for yet.
pub fn cache_price(price: u32) -> String {
    format!("{price} gold opens this cache.")
}

pub const MAGNET_TAKEN: &str = "Magnet! All the loot on the field flies to you.";
pub const MAGNET_APPEARED: &str = "A magnet dropped somewhere. Follow the beam of light.";
/// A battle standard was laid on the field.
pub const STANDARD_RAISED: &str = "A Warband Standard was raised somewhere. Follow the beam of light.";

/// A hero took a battle standard and the character `name` came.
pub fn ally_answers(name: &str) -> String {
    format!("{name} answers your call!")
}
pub const LEFT_ARENA: &str = "You can't leave the fight.";
/// The runes, as a hero takes them.
pub const SPEED_RUNE: &str = "Speed rune: your feet feel light.";
pub const BERSERKER_RUNE: &str = "Berserker rune: your blows land harder.";
pub const RESTORATION_RUNE: &str = "Restoration rune: your wounds close.";

// ---- shouts ----

/// A boss calls its adds.
pub const SUMMON_YELL: &str = "Servants, to my side!";
/// Every boss, as it dies.
pub const BOSS_DEATH_YELL: &str = "No... not like this...";

// ---- centre-screen lines ----

/// The run begins (said by the zone).
pub fn run_opens(subtitle: &str) -> String {
    format!("{subtitle}: hold out as long as you can!")
}

/// A boss has arrived (after its own shout).
pub const BOSS_ARRIVES: &str = "%s joins the fight!";
/// An elite has arrived.
pub const ELITE_APPROACHES: &str = "%s is coming for you!";
/// A swarm spawns around a hero (said by the swarm's enemy).
pub const SWARM: &str = "A swarm closes in!";
/// The hero's extra life.
pub const EXTRA_LIFE: &str = "%s rises again!";
/// Cheat death, and cheat death that makes the hero immune for a while.
pub const CHEAT_DEATH: &str = "%s will not go down!";
pub const CHEAT_DEATH_IMMUNE: &str = "%s slips out of death's grip!";
/// Endless begins.
pub const ENDLESS_SPEAKER: &str = "Endless";
pub const ENDLESS: &str = "The waves keep coming...";

#[cfg(test)]
mod tests {
    use super::*;

    /// A centre-screen line a unit says names it: the client puts the speaker's name for `%s`.
    #[test]
    fn a_units_centre_screen_line_names_it() {
        for line in [BOSS_ARRIVES, ELITE_APPROACHES, EXTRA_LIFE, CHEAT_DEATH, CHEAT_DEATH_IMMUNE] {
            assert_eq!(line.matches("%s").count(), 1, "{line}");
        }
    }
}
