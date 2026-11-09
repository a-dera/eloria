//! Build: besides Tauri, writes a version fingerprint into the binary
//! (commit, build date) and the list of commands exposed to the interface.
//! At startup, the interface compares this list with the one it uses: a Rust core
//! older than the interface (failed build, stale binary left in place) is reported
//! instead of producing half-empty screens.

use std::process::Command;

fn main() {
    let src = std::fs::read_to_string("src/commands.rs").unwrap_or_default();
    let mut names = Vec::new();
    let mut next_is_command = false;
    for line in src.lines() {
        let t = line.trim();
        if t.starts_with("#[tauri::command") {
            next_is_command = true;
            continue;
        }
        if next_is_command && (t.starts_with("pub async fn ") || t.starts_with("pub fn ")) {
            let rest = t.trim_start_matches("pub async fn ").trim_start_matches("pub fn ");
            let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            names.push(name);
            next_is_command = false;
        } else if next_is_command && !t.starts_with("///") && !t.starts_with("#[") && !t.is_empty() {
            next_is_command = false;
        }
    }
    println!("cargo:rustc-env=ELORIA_COMMANDS={}", names.join(","));
    println!("cargo:rerun-if-changed=src/commands.rs");

    let git = |args: &[&str]| Command::new("git").args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let commit = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "inconnu".into());
    let dirty = git(&["status", "--porcelain"]).map(|s| !s.is_empty()).unwrap_or(false);
    println!("cargo:rustc-env=ELORIA_COMMIT={commit}{}", if dirty { "+modifs" } else { "" });
    println!("cargo:rustc-env=ELORIA_BUILT={}", chrono_like_now());
    println!("cargo:rerun-if-changed=../.git/HEAD");

    tauri_build::build()
}

/// Local build date and time (without an extra dependency).
fn chrono_like_now() -> String {
    let out = if cfg!(windows) {
        Command::new("powershell").args(["-NoProfile", "-Command", "Get-Date -Format 'yyyy-MM-dd HH:mm:ss'"]).output()
    } else {
        Command::new("date").arg("+%Y-%m-%d %H:%M:%S").output()
    };
    out.ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}
