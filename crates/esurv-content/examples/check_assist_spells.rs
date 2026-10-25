//! Checks the assists' rotation spells (`content/assist_spells.ron`) against the player's 1.12.1
//! client: each row's cast time and cooldown must be `Spell.dbc`'s (`SpellCastTimes.dbc` for the
//! cast, the longer of the spell's and its category's cooldown; a channelled spell's channel, ticks and
//! radius from `SpellDuration.dbc`, its periodic amplitude and `SpellRadius.dbc`). It prints every row that differs,
//! written as it should be, and every rotation spell the table lacks; it never writes.
//!
//! `cargo run -p esurv-content --features client --example check_assist_spells [-- <Data folder>] [<spell id>...]`
//! (default: `$WOW_DATA`, else where the game finds the client). Spell ids after the folder are
//! printed as rows, for writing new ones.

use std::path::{Path, PathBuf};

use esurv_content::{read_spell_times, AssistSpell, ClientTables, Content, SpellTimes};

fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let data = match args.first().filter(|a| a.parse::<u32>().is_err()) {
        Some(_) => PathBuf::from(args.remove(0)),
        None => benilla_formats::wow_data().ok_or_else(|| anyhow::anyhow!("no client data found; pass its Data folder"))?,
    };
    let times = read_spell_times(&data)?;
    let of = |id: u32| times.iter().find(|t| t.id == id);
    let tables = ClientTables::read(&data)?;
    let name = |id: u32| tables.spells.iter().find(|s| s.id == id).map_or_else(String::new, |s| format!("{}{}", s.name, s.rank.as_ref().map_or_else(String::new, |r| format!(" ({r})"))));
    for id in args.iter().filter_map(|a| a.parse::<u32>().ok()) {
        match of(id) {
            Some(t) => println!("    {}, // {}", row_text(t), name(id)),
            None => println!("    // {id}: Spell.dbc has no such row"),
        }
    }
    let content = Content::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content"))?;
    let mut wrong = 0;
    for row in &content.assist_spells {
        match of(row.spell) {
            Some(t) if same(t, row) => {}
            Some(t) => {
                wrong += 1;
                println!("differs: {},", row_text(t));
            }
            None => {
                wrong += 1;
                println!("differs: {}: Spell.dbc has no such row", row.spell);
            }
        }
    }
    println!("{} rows, {wrong} differ from Spell.dbc", content.assist_spells.len());
    Ok(())
}

/// A table row as `Spell.dbc` would write it (a channel's fields only for a channelled spell).
fn row_text(t: &SpellTimes) -> String {
    let channel = if t.channel > 0.0 { format!(", channel: {:?}, tick_every: {:?}, radius: {:?}", t.channel, t.tick_every, t.radius) } else { String::new() };
    format!("(spell: {}, cast: {:?}, cooldown: {:?}{channel})", t.id, t.cast, t.cooldown)
}

/// Row `row` holds `Spell.dbc`'s times.
fn same(t: &SpellTimes, row: &AssistSpell) -> bool {
    let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
    near(t.cast, row.cast) && near(t.cooldown, row.cooldown) && near(t.channel, row.channel) && near(t.tick_every, row.tick_every) && near(t.radius, row.radius)
}
