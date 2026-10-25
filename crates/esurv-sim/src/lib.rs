//! The game rules, with no networking and no rendering (design step 2).
//!
//! One module per system under `game/` (enemy, abilities, rares, upgrades, talents, ...).

pub mod rng;

pub mod game {
    pub mod abilities;
    pub mod amulets;
    pub mod assist_rotation;
    pub mod assists;
    pub mod beams;
    pub mod boss_abilities;
    pub mod boss_damage;
    pub mod boss_phases;
    #[cfg(test)]
    mod boss_rules_tests;
    #[cfg(test)]
    mod boss_rules_zones_3_to_6_tests;
    #[cfg(test)]
    mod boss_rules_stratholme_blackrock_aq_tests;
    #[cfg(test)]
    mod assists_tests;
    pub mod boss_script_actions;
    pub mod boss_scripts;
    pub mod boss_warnings;
    pub mod casting;
    pub mod challenge;
    pub mod choices;
    pub mod damage;
    pub mod director;
    pub mod enemy;
    pub mod enemy_ai;
    pub mod enemy_attacks;
    pub mod field_milestones;
    pub mod gear;
    pub mod ground_zones;
    pub mod kit;
    pub mod rares;
    pub mod interactables;
    pub mod level;
    pub mod lines;
    pub mod loot;
    pub mod milestones;
    pub mod offers;
    pub mod pets;
    pub mod pickups;
    pub mod prestige;
    pub mod proc_effects;
    pub mod rewards;
    pub mod run;
    pub mod scripted_pause;
    pub mod spell_numbers;
    pub mod spells;
    pub mod stats;
    pub mod talents;
    pub mod tally;
    pub mod telegraphs;
    pub mod upgrades;
}
