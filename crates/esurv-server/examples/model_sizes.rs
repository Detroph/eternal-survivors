//! How big models are, to plan the camp with their true sizes: each M2's width (x), depth (y) and
//! height (z) in yards at size 1, from its vertices, how far its lowest point sits below its
//! origin, and where along x and y it reaches from its origin. Reads the player's own data.
//!
//! `cargo run -p esurv-server --example model_sizes -- 'World\Generic\Buildings\HumanTentMedium\HumanTentMedium.mdx' ...`
//! (`.mdx` and `.mdl` are read as the `.m2` the archives hold); `go:<display id>` measures a game
//! object by its display record, `find:<word>` lists the game objects whose model path holds it,
//! `list:<word>` every model of the archives' file lists whose path holds it.

use std::io::Cursor;

/// The archives a model is looked for in, newest first.
const ARCHIVES: [&str; 7] = ["patch-2.MPQ", "patch.MPQ", "model.MPQ", "misc.MPQ", "dbc.MPQ", "wmo.MPQ", "terrain.MPQ"];

fn main() -> anyhow::Result<()> {
    let dir = benilla_formats::wow_data().ok_or_else(|| anyhow::anyhow!("no WoW install found"))?;
    let archives: Vec<benilla_mpq::Archive> =
        ARCHIVES.iter().filter_map(|a| benilla_mpq::Archive::open(dir.join(a)).ok()).collect();
    println!("model\twidth_x\tdepth_y\theight_z\tbelow_origin\tx_from_to\ty_from_to");
    let displays = archives.iter().find_map(|a| a.read_file("DBFilesClient\\GameObjectDisplayInfo.dbc").ok());
    for name in std::env::args().skip(1) {
        // `find:<word>`: every game-object display whose model path holds the word
        if let Some(word) = name.strip_prefix("find:") {
            for (id, path) in displays.as_deref().map(all_displays).unwrap_or_default() {
                if path.to_lowercase().contains(&word.to_lowercase()) {
                    println!("go:{id}\t{path}");
                }
            }
            continue;
        }
        // `blp:<model path>`: the textures a model names
        if let Some(model) = name.strip_prefix("blp:") {
            let Some(bytes) = archives.iter().find_map(|a| a.read_file(&model.replace(".mdx", ".m2")).ok()) else {
                continue;
            };
            for text in bytes.split(|&b| b == 0).map(String::from_utf8_lossy) {
                if text.to_lowercase().ends_with(".blp") {
                    println!("{text}");
                }
            }
            continue;
        }
        // `wears:<item look>`: the creature looks dressed in that item look, with their race, sex
        // and every piece they wear (head, shoulder, shirt, chest, belt, legs, feet, wrists, hands,
        // tabard)
        if let Some(item) = name.strip_prefix("wears:") {
            let item: u32 = item.parse().unwrap_or(0);
            let read = |file: &str| archives.iter().find_map(|a| a.read_file(&format!("DBFilesClient\\{file}")).ok());
            let (Some(info), Some(extra)) = (read("CreatureDisplayInfo.dbc"), read("CreatureDisplayInfoExtra.dbc")) else {
                continue;
            };
            let extras: std::collections::HashMap<u32, Vec<u32>> = dbc_rows(&extra).map(|r| (r[0], r)).collect();
            for r in dbc_rows(&info) {
                if let Some(x) = extras.get(&r[3]).filter(|x| x[8..18].contains(&item)) {
                    println!("display {}\trace {} sex {}\twears {:?}", r[0], x[1], x[2], &x[8..18]);
                }
            }
            continue;
        }
        // `people:<race>:<sex>`: the creature looks of that race and sex with a body of their own
        // (`CreatureDisplayInfoExtra`): skin, face, hair style and colour, facial hair, and what
        // they wear; then the hair styles that show the scalp (bald)
        if let Some((race, sex)) = name.strip_prefix("people:").and_then(|w| w.split_once(':')) {
            let (race, sex): (u32, u32) = (race.parse().unwrap_or(0), sex.parse().unwrap_or(0));
            let read = |file: &str| archives.iter().find_map(|a| a.read_file(&format!("DBFilesClient\\{file}")).ok());
            let (Some(info), Some(extra)) = (read("CreatureDisplayInfo.dbc"), read("CreatureDisplayInfoExtra.dbc")) else {
                continue;
            };
            let extras: std::collections::HashMap<u32, Vec<u32>> = dbc_rows(&extra).map(|r| (r[0], r)).collect();
            for r in dbc_rows(&info) {
                if let Some(x) = extras.get(&r[3]).filter(|x| x[1] == race && x[2] == sex) {
                    println!("display {}\tskin {} face {} hair {} colour {} facial {}\twears {:?}", r[0], x[3], x[4], x[5], x[6], x[7], &x[8..18]);
                }
            }
            if let Some(hair) = read("CharHairGeosets.dbc") {
                for h in dbc_rows(&hair).filter(|h| h[1] == race && h[2] == sex) {
                    println!("hair style {}\tgeoset {} bald {}", h[3], h[4], h[5]);
                }
            }
            if let Some(facial) = read("CharacterFacialHairStyles.dbc") {
                for f in dbc_rows(&facial).filter(|f| f[0] == race && f[1] == sex) {
                    println!("facial hair {}\tgeosets {:?}", f[2], &f[3..]);
                }
            }
            continue;
        }
        // `items:<word>`: every item look (`ItemDisplayInfo.dbc`) whose model, texture or icon name
        // holds the word: its display id and those names
        if let Some(word) = name.strip_prefix("items:") {
            let Some(dbc) = archives.iter().find_map(|a| a.read_file("DBFilesClient\\ItemDisplayInfo.dbc").ok()) else {
                continue;
            };
            let word = word.to_lowercase();
            let strings = 20 + dbc_rows(&dbc).count() * u32::from_le_bytes([dbc[12], dbc[13], dbc[14], dbc[15]]) as usize;
            let text = |at: u32| -> Option<String> {
                let s = dbc.get(strings + at as usize..)?;
                (at > 0 && dbc.get(strings + at as usize - 1) == Some(&0)).then_some(())?;
                let end = s.iter().position(|&b| b == 0)?;
                (end > 0).then(|| String::from_utf8_lossy(&s[..end]).into_owned())
            };
            for row in dbc_rows(&dbc) {
                let names: Vec<String> = row[1..].iter().filter_map(|&v| text(v)).collect();
                if names.iter().any(|n| n.to_lowercase().contains(&word)) {
                    println!("item look {}\t{}", row[0], names.join(" "));
                }
            }
            continue;
        }
        // `npc:<race>:<sex>`: the creature looks dressed as a person of that race and sex (1.12 ids:
        // 1 human, 3 dwarf ...; 0 male, 1 female), with how many pieces they wear, their hair style, beard and head piece (0: bareheaded)
        if let Some(who) = name.strip_prefix("npc:") {
            let (race, sex) = who.split_once(':').unwrap_or((who, "0"));
            let (race, sex): (u32, u32) = (race.parse().unwrap_or(1), sex.parse().unwrap_or(0));
            let read = |file: &str| archives.iter().find_map(|a| a.read_file(&format!("DBFilesClient\\{file}")).ok());
            let (Some(info), Some(extra)) = (read("CreatureDisplayInfo.dbc"), read("CreatureDisplayInfoExtra.dbc")) else {
                continue;
            };
            let extras: std::collections::HashMap<u32, (u32, u32, u32, u32, u32, u32)> = dbc_rows(&extra)
                .map(|r| (r[0], (r[1], r[2], r[8..19].iter().filter(|&&d| d != 0).count() as u32, r[5], r[7], r[8])))
                .collect();
            for r in dbc_rows(&info) {
                if let Some(&(rc, sx, worn, hair, beard, head)) = extras.get(&r[3]) {
                    if rc == race && sx == sex && worn >= 4 {
                        println!("display {}\twears {worn}\thair style {hair}\tbeard {beard}\thead {head}", r[0]);
                    }
                }
            }
            continue;
        }
        // `list:<word>`: every model in the archives' own file lists whose path holds the word
        if let Some(word) = name.strip_prefix("list:") {
            let mut found = std::collections::BTreeSet::new();
            for a in &archives {
                let Ok(list) = a.read_file("(listfile)") else { continue };
                for path in String::from_utf8_lossy(&list).lines() {
                    let lower = path.to_lowercase();
                    if (lower.ends_with(".m2") || lower.ends_with(".wmo")) && lower.contains(&word.to_lowercase()) {
                        found.insert(path.trim().to_string());
                    }
                }
            }
            for path in found {
                println!("{path}");
            }
            continue;
        }
        // `slice:<x from>..<x to>:<model>`: the model's highest point in each half-yard along y, of
        // its part between those x (a counter's plank, a shelf), with how far along x it reaches
        let (slice, name) = match name.strip_prefix("slice:").and_then(|r| r.split_once(':')) {
            Some((range, model)) => (range.split_once("..").and_then(|(a, b)| Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?))), model.to_string()),
            None => (None, name),
        };
        // `go:<display id>`: the game object's model, from its display record
        let name = match name.strip_prefix("go:").and_then(|id| id.parse::<u32>().ok()) {
            Some(id) => match displays.as_deref().and_then(|d| display_model(d, id)) {
                Some(path) => format!("{path} (go:{id})"),
                None => {
                    println!("{name}\tno display record");
                    continue;
                }
            },
            None => name,
        };
        let path = name.split(" (go:").next().unwrap_or(&name);
        if path.to_lowercase().ends_with(".wmo") {
            wmo_sizes(&archives, path);
            continue;
        }
        let m2 = path.rsplit_once('.').map_or(path.to_string(), |(stem, _)| format!("{stem}.m2"));
        let Some(bytes) = archives.iter().find_map(|a| a.read_file(&m2).ok()) else {
            println!("{name}\tnot found");
            continue;
        };
        let parsed = benilla_m2::parse_m2(&mut Cursor::new(bytes.as_slice()))?;
        let model = parsed.model();
        if let Some((x0, x1)) = slice {
            let mut cells: std::collections::BTreeMap<i32, (f32, f32, f32)> = Default::default();
            for v in &model.vertices {
                let p = [v.position.x, v.position.y, v.position.z];
                if p[0] >= x0 && p[0] <= x1 {
                    let e = cells.entry((p[1] * 2.0).floor() as i32).or_insert((f32::MIN, f32::MAX, f32::MIN));
                    *e = (e.0.max(p[2]), e.1.min(p[0]), e.2.max(p[0]));
                }
            }
            println!("{name}: x {x0}..{x1}");
            for (cell, (z, xa, xb)) in cells {
                println!("\ty {:.1}..{:.1}: up to {z:.2}, x {xa:.2}..{xb:.2}", cell as f32 / 2.0, cell as f32 / 2.0 + 0.5);
            }
            continue;
        }
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &model.vertices {
            for (k, c) in [v.position.x, v.position.y, v.position.z].into_iter().enumerate() {
                lo[k] = lo[k].min(c);
                hi[k] = hi[k].max(c);
            }
        }
        println!(
            "{name}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}..{:.2}\t{:.2}..{:.2}",
            hi[0] - lo[0],
            hi[1] - lo[1],
            hi[2] - lo[2],
            -lo[2],
            lo[0],
            hi[0],
            lo[1],
            hi[1]
        );
        // where its collision hull (what the client makes solid of an object) reaches
        let hull: Vec<[f32; 3]> = model
            .raw_data
            .bounding_vertices
            .chunks_exact(12)
            .map(|c| [0, 4, 8].map(|k| f32::from_le_bytes([c[k], c[k + 1], c[k + 2], c[k + 3]])))
            .collect();
        if !hull.is_empty() {
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in &hull {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            println!(
                "\thull: {} points, x {:.2}..{:.2}, y {:.2}..{:.2}, z {:.2}..{:.2}",
                hull.len(),
                lo[0],
                hi[0],
                lo[1],
                hi[1],
                lo[2],
                hi[2]
            );
        }
        // the sitting animations it has (a person's: on the ground, and on a low, medium and high chair)
        let sits: Vec<&str> = [(97, "ground"), (102, "low chair"), (103, "medium chair"), (104, "high chair")]
            .into_iter()
            .filter(|(id, _)| model.owns_animation(*id))
            .map(|(_, sit)| sit)
            .collect();
        if !sits.is_empty() {
            println!("\tsits: {}", sits.join(", "));
        }
        // its feet: the vertices within a tenth of a yard of its lowest point, grouped in half-yard
        // cells (a barrel's bottom, a pole's foot)
        let mut feet: std::collections::BTreeMap<(i32, i32), (f32, f32, f32, u32)> = Default::default();
        for v in &model.vertices {
            let p = [v.position.x, v.position.y, v.position.z];
            if p[2] <= lo[2] + 0.1 {
                let e = feet.entry(((p[0] * 2.0).floor() as i32, (p[1] * 2.0).floor() as i32)).or_default();
                *e = (e.0 + p[0], e.1 + p[1], e.2.min(p[2]), e.3 + 1);
            }
        }
        for (x, y, z, n) in feet.values() {
            println!("\tfoot: ({:.2}, {:.2}) at {z:.2}", x / *n as f32, y / *n as f32);
        }
        // its rim on each side (the vertices within a tenth of a yard of its outermost on that side):
        // how high it runs, to hang a thing from an eave
        for (side, k, outer) in [("front +x", 0, hi[0]), ("back -x", 0, lo[0]), ("left +y", 1, hi[1]), ("right -y", 1, lo[1])] {
            let zs: Vec<f32> = model
                .vertices
                .iter()
                .map(|v| [v.position.x, v.position.y, v.position.z])
                .filter(|p| (p[k] - outer).abs() <= 0.1)
                .map(|p| p[2])
                .collect();
            if let (Some(lo_z), Some(hi_z)) = (zs.iter().copied().reduce(f32::min), zs.iter().copied().reduce(f32::max)) {
                println!("\trim {side}: {lo_z:.2}..{hi_z:.2} up");
            }
        }
        // its top (the vertices in the top fifth of its height): where it is, and how it slopes,
        // to set a thing on it
        let top: Vec<[f32; 3]> = model
            .vertices
            .iter()
            .map(|v| [v.position.x, v.position.y, v.position.z])
            .filter(|p| p[2] >= hi[2] - (hi[2] - lo[2]) * 0.2)
            .collect();
        if let Some(((cx, cy, cz), (sx, sy))) = top_plane(&top) {
            println!(
                "\ttop: middle ({cx:.2}, {cy:.2}, {cz:.2}), rising {sx:.2} a yard along x, {sy:.2} along y ({:.0} and {:.0} degrees)",
                sx.atan().to_degrees(),
                sy.atan().to_degrees()
            );
        }
    }
    Ok(())
}

/// A building's (`.wmo`) size, as a model's, and its top's height in each one-yard cell, to see
/// its shape (where a ramp runs).
fn wmo_sizes(archives: &[benilla_mpq::Archive], path: &str) {
    let read = |p: &str| archives.iter().find_map(|a| a.read_file(p).ok());
    let Some(Ok(benilla_wmo::ParsedWmo::Root(root))) =
        read(path).map(|b| benilla_wmo::parse_wmo(&mut Cursor::new(b.as_slice())))
    else {
        println!("{path}\tnot found");
        return;
    };
    let stem = path.rsplit_once('.').map_or(path, |(s, _)| s);
    let mut points = Vec::new();
    for g in 0..root.n_groups {
        let Some(bytes) = read(&format!("{stem}_{g:03}.wmo")) else { continue };
        let Ok(benilla_wmo::ParsedWmo::Group(group)) = benilla_wmo::parse_wmo(&mut Cursor::new(bytes.as_slice())) else {
            continue;
        };
        // points across each triangle, so a long flat face still marks every cell it covers
        for t in group.vertex_indices.chunks(3).filter(|t| t.len() == 3) {
            let v = [0, 1, 2].map(|k| &group.vertex_positions[t[k] as usize]).map(|p| [p.x, p.y, p.z]);
            for a in 0..=6 {
                for b in 0..=(6 - a) {
                    let (a, b) = (a as f32 / 6.0, b as f32 / 6.0);
                    points.push([0, 1, 2].map(|c| v[0][c] + (v[1][c] - v[0][c]) * a + (v[2][c] - v[0][c]) * b));
                }
            }
        }
    }
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &points {
        for c in 0..3 {
            lo[c] = lo[c].min(p[c]);
            hi[c] = hi[c].max(p[c]);
        }
    }
    println!(
        "{path}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}..{:.2}\t{:.2}..{:.2}",
        hi[0] - lo[0],
        hi[1] - lo[1],
        hi[2] - lo[2],
        -lo[2],
        lo[0],
        hi[0],
        lo[1],
        hi[1]
    );
    // seen from above, its front (+x) up the page and its left (+y) on the left
    let (x0, y0) = (lo[0].floor() as i32, lo[1].floor() as i32);
    let (nx, ny) = ((hi[0].ceil() as i32 - x0).max(1) as usize, (hi[1].ceil() as i32 - y0).max(1) as usize);
    let mut top = vec![vec![f32::MIN; ny]; nx];
    for p in &points {
        let (i, j) = ((p[0].floor() as i32 - x0) as usize, (p[1].floor() as i32 - y0) as usize);
        if i < nx && j < ny {
            top[i][j] = top[i][j].max(p[2]);
        }
    }
    println!("\ttop heights, x down the side, y (from {}) along the top:", y0 + ny as i32 - 1);
    for i in (0..nx).rev() {
        let row: String = (0..ny)
            .rev()
            .map(|j| if top[i][j] == f32::MIN { "    .".to_string() } else { format!("{:5.1}", top[i][j]) })
            .collect();
        println!("\t{:4} {row}", x0 + i as i32);
    }
}

/// The middle of `points` and the least-squares plane's rise per yard along x and y.
fn top_plane(points: &[[f32; 3]]) -> Option<((f32, f32, f32), (f32, f32))> {
    let n = points.len() as f32;
    if n < 3.0 {
        return None;
    }
    let m = points.iter().fold([0.0; 3], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n, s[2] + p[2] / n]);
    let (mut xx, mut xy, mut yy, mut xz, mut yz) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for p in points {
        let (dx, dy, dz) = (p[0] - m[0], p[1] - m[1], p[2] - m[2]);
        xx += dx * dx;
        xy += dx * dy;
        yy += dy * dy;
        xz += dx * dz;
        yz += dy * dz;
    }
    let det = xx * yy - xy * xy;
    (det.abs() > f32::EPSILON).then(|| ((m[0], m[1], m[2]), ((xz * yy - yz * xy) / det, (yz * xx - xz * xy) / det)))
}

/// The model path of game-object display `id` in `GameObjectDisplayInfo.dbc` (its first field the
/// id, its second the path's offset in the string block).
fn display_model(dbc: &[u8], id: u32) -> Option<String> {
    all_displays(dbc).into_iter().find_map(|(k, path)| (k == id).then_some(path))
}

/// The rows of a `.dbc`, each as its 4-byte fields.
fn dbc_rows(dbc: &[u8]) -> impl Iterator<Item = Vec<u32>> + '_ {
    let word = |at: usize| dbc.get(at..at + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let (count, fields) = (word(4) as usize, word(8) as usize);
    let size = word(12) as usize;
    (0..count).map(move |k| (0..fields).map(|f| word(20 + k * size + f * 4)).collect())
}

/// Every game-object display in `GameObjectDisplayInfo.dbc`: its id and its model path.
fn all_displays(dbc: &[u8]) -> Vec<(u32, String)> {
    let word = |at: usize| dbc.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let (Some(count), Some(size)) = (word(4), word(12)) else {
        return Vec::new();
    };
    let strings = 20 + count * size;
    (0..count)
        .filter_map(|k| {
            let row = 20 + k * size;
            let text = dbc.get(strings + word(row + 4)?..)?;
            let end = text.iter().position(|&b| b == 0)?;
            Some((word(row)? as u32, String::from_utf8_lossy(&text[..end]).into_owned()))
        })
        .collect()
}
