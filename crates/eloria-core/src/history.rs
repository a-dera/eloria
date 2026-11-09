//! Version history: snapshots in `.eloria/history/<path>/<timestamp>.md`.

use std::fs;
use std::path::PathBuf;

use chrono::{Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};

use crate::vault::Vault;
use crate::Result;

const STAMP: &str = "%Y%m%d-%H%M%S";
/// Maximum number of snapshots kept per note.
pub const MAX_VERSIONS: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: String,
    /// Unix timestamp (seconds).
    pub timestamp: i64,
    pub size: u64,
}

fn dir_for(vault: &Vault, rel: &str) -> Result<PathBuf> {
    // `resolve` validates the path; it is applied again under history/.
    vault.resolve(rel)?;
    Ok(vault.eloria_dir().join("history").join(rel))
}

pub fn list(vault: &Vault, rel: &str) -> Result<Vec<Version>> {
    let dir = dir_for(vault, rel)?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for e in fs::read_dir(dir)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().to_string();
        let Some(id) = name.strip_suffix(".md") else { continue };
        let Ok(dt) = NaiveDateTime::parse_from_str(id, STAMP) else { continue };
        let ts = Local.from_local_datetime(&dt).single().map(|d| d.timestamp()).unwrap_or(0);
        out.push(Version { id: id.to_string(), timestamp: ts, size: e.metadata()?.len() });
    }
    out.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(out)
}

pub fn read(vault: &Vault, rel: &str, id: &str) -> Result<String> {
    if id.contains(['/', '\\', '.']) {
        return Err(crate::Error::InvalidPath(id.to_string()));
    }
    Ok(fs::read_to_string(dir_for(vault, rel)?.join(format!("{id}.md")))?)
}

/// Creates a snapshot if the last one is older than `min_interval` seconds and
/// the content changed. `force` ignores the interval. Returns the created id.
pub fn snapshot(vault: &Vault, rel: &str, content: &str, min_interval: i64, force: bool) -> Result<Option<String>> {
    let dir = dir_for(vault, rel)?;
    let versions = list(vault, rel)?;
    let now = Local::now();
    if let Some(last) = versions.first() {
        if !force && now.timestamp() - last.timestamp < min_interval {
            return Ok(None);
        }
        if read(vault, rel, &last.id).map(|c| c == content).unwrap_or(false) {
            return Ok(None);
        }
    }
    fs::create_dir_all(&dir)?;
    let mut id = now.format(STAMP).to_string();
    if versions.first().map(|v| v.id == id).unwrap_or(false) {
        // Same second: overwrite.
        id = versions[0].id.clone();
    }
    fs::write(dir.join(format!("{id}.md")), content)?;
    for old in versions.iter().skip(MAX_VERSIONS - 1) {
        let _ = fs::remove_file(dir.join(format!("{}.md", old.id)));
    }
    Ok(Some(id))
}

/// Moves the history when a note is renamed.
pub fn rename(vault: &Vault, from: &str, to: &str) -> Result<()> {
    let a = dir_for(vault, from)?;
    if a.exists() {
        let b = dir_for(vault, to)?;
        if let Some(p) = b.parent() {
            fs::create_dir_all(p)?;
        }
        let _ = fs::rename(a, b);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        assert!(snapshot(&v, "a/N.md", "v1", 300, false).unwrap().is_some());
        // Too recent
        assert!(snapshot(&v, "a/N.md", "v2", 300, false).unwrap().is_none());
        // Identical content, even when forced
        assert!(snapshot(&v, "a/N.md", "v1", 0, true).unwrap().is_none());
        let l = list(&v, "a/N.md").unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(read(&v, "a/N.md", &l[0].id).unwrap(), "v1");
        assert!(read(&v, "a/N.md", "../x").is_err());
        rename(&v, "a/N.md", "b/M.md").unwrap();
        assert_eq!(list(&v, "b/M.md").unwrap().len(), 1);
    }
}
