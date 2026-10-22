//! Stamps the commit this binary was built from, as the `benilla` launcher does, and on Windows
//! gives the game exe its icon (`assets/icon.ico`, made by `scripts/make-game-icon.ps1`).
//! The setup exe keeps the default icon.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    benilla_buildstamp::emit();
    embed_icon();
}

/// Compiles a one-line resource script naming the icon with the Windows SDK's `rc.exe` and links
/// the result into the game exe, so Explorer, the taskbar and the window show it. Without the
/// SDK the build goes on with the default icon and says so.
fn embed_icon() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon = manifest.join("assets").join("icon.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }
    let Some(rc) = resource_compiler() else {
        println!("cargo:warning=no rc.exe found (Windows SDK): the game exe keeps the default icon");
        return;
    };
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let script = out.join("esurv.rc");
    let res = out.join("esurv.res");
    // resource 1 is the icon Windows shows for the exe; the path is quoted with forward slashes
    let line = format!("1 ICON \"{}\"\n", icon.display().to_string().replace('\\', "/"));
    std::fs::write(&script, line).unwrap();
    let ok = Command::new(&rc).arg("/nologo").arg("/fo").arg(&res).arg(&script).status().is_ok_and(|s| s.success());
    if ok {
        println!("cargo:rustc-link-arg-bins={}", res.display());
    } else {
        println!("cargo:warning=rc.exe failed: the game exe keeps the default icon");
    }
}

/// `RC` from the environment, else the newest x64 `rc.exe` of the Windows 10/11 SDK.
fn resource_compiler() -> Option<PathBuf> {
    if let Ok(rc) = std::env::var("RC") {
        return Some(PathBuf::from(rc));
    }
    let kits = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(kits).ok()?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    versions.sort();
    versions.into_iter().rev().map(|v| v.join("x64").join("rc.exe")).find(|rc| rc.is_file())
}
