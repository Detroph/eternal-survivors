//! Every milestone talent's tooltip, before and after a change of the talent numbers, as the hero
//! sheet shows it (`hero_sheet::talents::tip`, the client's names filled in from the install):
//! one JSON entry per milestone talent (rows 2, 4 and 6 of every class's trees).
//!
//! `cargo run --release -p esurv-app --features client-names --example milestone_tooltips -- --old <content dir>
//! --new <content dir> [--gains <tsv>] --out <file.json>`
//!
//! `--gains`: tab-separated lines `talent<TAB>gain old<TAB>gain new` (percent, as measured by the
//! balance check's `--milestones`), copied into each entry with its row's budget.

use std::collections::HashMap;
use std::fs;

use esurv_app::hero_sheet::talents::{tip, trees};
use esurv_content::{ClientNames, Content, Talent, TalentEffect};
use esurv_sim::game::talents::Ranks;

/// The budget of a milestone row: the hero's DPS gain with its talent (AUTHORING.md).
fn target(row: u8) -> f64 {
    match row {
        2 => 5.0,
        4 => 10.0,
        _ => 25.0,
    }
}

/// The tooltip of talent `id` of `class` with one rank learned, its lines joined.
fn tooltip(content: &Content, class: &str, tree: usize, t: &Talent) -> String {
    let mut ranks = Ranks::new();
    ranks.insert(t.id.clone(), 1);
    let lines = tip(content, class, &ranks, 0, tree, t);
    lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
}

fn find<'c>(content: &'c Content, class: &str, id: &str) -> Option<(usize, &'c Talent)> {
    trees(content, class).iter().enumerate().find_map(|(i, tr)| tr.talents.iter().find(|t| t.id == id).map(|t| (i, t)))
}

fn effects_text(t: &Talent) -> Vec<String> {
    t.effects.iter().map(|e: &TalentEffect| format!("{e:?}")).collect()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut old_dir, mut new_dir, mut gains_file, mut out) = (None, None, None, None);
    while let Some(a) = args.next() {
        let v = args.next();
        match a.as_str() {
            "--old" => old_dir = v,
            "--new" => new_dir = v,
            "--gains" => gains_file = v,
            "--out" => out = v,
            other => panic!("unknown option {other}"),
        }
    }
    let names = ClientNames::from_install().ok().flatten().unwrap_or_else(ClientNames::none);
    let old = Content::load_with_names(old_dir.expect("--old"), &names).expect("the old content loads");
    let new = Content::load_with_names(new_dir.expect("--new"), &names).expect("the new content loads");
    let mut gains: HashMap<String, (String, String)> = HashMap::new();
    if let Some(f) = gains_file {
        for line in fs::read_to_string(f).expect("the gains read").lines() {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() >= 3 {
                gains.insert(c[0].to_string(), (c[1].to_string(), c[2].to_string()));
            }
        }
    }
    let mut entries = Vec::new();
    for class in &new.classes {
        for (ti, tree) in trees(&new, &class.id).iter().enumerate() {
            let mut talents: Vec<&Talent> = tree.talents.iter().filter(|t| t.is_milestone()).collect();
            talents.sort_by_key(|t| (t.row, t.col));
            for t in talents {
                let new_tip = tooltip(&new, &class.id, ti, t);
                let (old_tip, old_effects) = match find(&old, &class.id, &t.id) {
                    Some((oi, ot)) => (tooltip(&old, &class.id, oi, ot), effects_text(ot)),
                    None => (String::new(), Vec::new()),
                };
                let new_effects = effects_text(t);
                // the same effects with other numbers: retuned; other effects: redesigned
                let shape = |e: &[String]| e.iter().map(|s| s.chars().filter(|c| !c.is_ascii_digit() && *c != '.').collect::<String>()).collect::<Vec<_>>();
                let flag = if old_effects == new_effects {
                    "unchanged"
                } else if shape(&old_effects) == shape(&new_effects) {
                    "retuned"
                } else {
                    "redesigned"
                };
                let (gain_old, gain_new) = gains.get(&t.id).cloned().unwrap_or_default();
                entries.push(serde_json::json!({
                    "class": class.id,
                    "tree": tree.name,
                    "row": t.row,
                    "points": u32::from(t.row) * 5 + 1,
                    "talent": t.name,
                    "id": t.id,
                    "icon": t.icon,
                    "old_tooltip": old_tip,
                    "new_tooltip": new_tip,
                    "gain_old_pct": gain_old,
                    "gain_new_pct": gain_new,
                    "target_pct": target(t.row),
                    "change": flag,
                }));
            }
        }
    }
    let out = out.expect("--out");
    fs::write(&out, serde_json::to_string_pretty(&entries).expect("json")).expect("the json writes");
    println!("{} milestone talents written to {out}", entries.len());
}
