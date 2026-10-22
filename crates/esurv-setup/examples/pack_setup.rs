//! Makes the one-file setup: the setup executable with every file of a staged game folder
//! appended as its payload (`esurv_setup::pack`).
//!
//! `cargo run --release -p esurv-setup --example pack_setup -- <esurv-setup.exe> <staged folder> <out.exe>`

use std::path::{Path, PathBuf};

use esurv_setup::Entry;

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [stub, staged, out] = &args[..] else {
        eprintln!("usage: pack_setup <esurv-setup.exe> <staged folder> <out.exe>");
        std::process::exit(2);
    };
    let mut entries = Vec::new();
    collect(staged, staged, &mut entries);
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let stub_bytes = std::fs::read(stub).expect("read the setup executable");
    let mut file = std::io::BufWriter::new(std::fs::File::create(out).expect("create the setup"));
    esurv_setup::pack(&stub_bytes, &entries, &mut file).expect("write the setup");
    println!("{} files packed into {}", entries.len(), out.display());
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<Entry>) {
    for e in std::fs::read_dir(dir).expect("list the staged folder") {
        let path = e.expect("a staged entry").path();
        if path.is_dir() {
            collect(root, &path, out);
        } else {
            let rel = path.strip_prefix(root).unwrap();
            let rel: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            out.push(Entry {
                path: rel.join("/"),
                bytes: std::fs::read(&path).expect("read a staged file"),
            });
        }
    }
}
