//! Vault backups: a dated .zip archive in a chosen folder, with rotation,
//! and guided restore into an empty folder (the open vault is never overwritten).
//!
//! The archive holds the whole vault (notes, PDFs, images, templates, custom themes,
//! version history) except what is rebuilt or discarded: the index, the trash
//! and temporary files.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;

use crate::error::Error;
use crate::Result;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BackupInfo {
    /// Full path of the archive
    pub path: String,
    pub name: String,
    pub bytes: u64,
    /// Creation date (YYYY-MM-DD HH:MM:SS, local time)
    pub created: String,
    pub files: usize,
}

/// Archive prefix for a vault: `Eloria-<vault name>-`.
fn prefix(vault_name: &str) -> String {
    let clean: String = vault_name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    format!("Eloria-{clean}-")
}

/// What does not go into the archive (path relative to the vault, `/` separator).
fn excluded(rel: &str) -> bool {
    let r = rel.trim_start_matches("./");
    r.starts_with(".eloria/index.db")
        || r.starts_with(".eloria/trash/")
        || r.starts_with(".eloria/tmp/")
        || r.starts_with(".git/")
        || r.ends_with(".eloria-tmp")
}

/// Creates an archive of the vault in `dest`, then keeps only the `keep` most recent of this
/// coffre (0 = toutes).
pub fn create(root: &Path, dest: &Path, keep: usize) -> Result<BackupInfo> {
    if dest.starts_with(root) {
        return Err(Error::InvalidPath("le dossier de sauvegarde doit être hors du coffre".into()));
    }
    fs::create_dir_all(dest)?;
    let name = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "coffre".into());
    let stamp = chrono::Local::now();
    let file_name = format!("{}{}.zip", prefix(&name), stamp.format("%Y%m%d-%H%M%S"));
    let final_path = dest.join(&file_name);
    let tmp_path = dest.join(format!("{file_name}.partiel"));
    let mut files = 0usize;
    {
        let mut zw = zip::ZipWriter::new(File::create(&tmp_path)?);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
        for e in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            let rel = match e.path().strip_prefix(root) {
                Ok(r) if !r.as_os_str().is_empty() => r.to_string_lossy().replace('\\', "/"),
                _ => continue,
            };
            if excluded(&rel) || excluded(&format!("{rel}/")) {
                continue;
            }
            if e.file_type().is_dir() {
                zw.add_directory(format!("{rel}/"), opts).map_err(zip_err)?;
            } else if e.file_type().is_file() {
                zw.start_file(&rel, opts).map_err(zip_err)?;
                io::copy(&mut File::open(e.path())?, &mut zw)?;
                files += 1;
            }
        }
        zw.finish().map_err(zip_err)?.flush()?;
    }
    // Atomic write: an interrupted archive never replaces a good one.
    fs::rename(&tmp_path, &final_path)?;
    if keep > 0 {
        for old in list(dest, &name)?.into_iter().skip(keep) {
            let _ = fs::remove_file(&old.path);
        }
    }
    Ok(BackupInfo {
        path: final_path.to_string_lossy().to_string(),
        name: file_name,
        bytes: fs::metadata(&final_path)?.len(),
        created: stamp.format("%Y-%m-%d %H:%M:%S").to_string(),
        files,
    })
}

/// Archives of a vault in `dest`, most recent first.
pub fn list(dest: &Path, vault_name: &str) -> Result<Vec<BackupInfo>> {
    let pre = prefix(vault_name);
    let mut v = Vec::new();
    let Ok(rd) = fs::read_dir(dest) else { return Ok(v) };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with(&pre) || !name.ends_with(".zip") {
            continue;
        }
        let stamp = &name[pre.len()..name.len() - 4];
        let created = chrono::NaiveDateTime::parse_from_str(stamp, "%Y%m%d-%H%M%S")
            .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();
        v.push(BackupInfo { path: e.path().to_string_lossy().to_string(), name, bytes: e.metadata().map(|m| m.len()).unwrap_or(0), created, files: 0 });
    }
    v.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(v)
}

/// Restores an archive into `target`, which must be empty or missing. Returns the number of
/// restored files. Paths outside the target folder are refused (malicious archive).
pub fn restore(archive: &Path, target: &Path) -> Result<usize> {
    if target.exists() && fs::read_dir(target)?.next().is_some() {
        return Err(Error::AlreadyExists(format!("{} n'est pas vide : choisis un dossier vide", target.display())));
    }
    fs::create_dir_all(target)?;
    let mut za = zip::ZipArchive::new(File::open(archive)?).map_err(zip_err)?;
    let mut n = 0;
    for i in 0..za.len() {
        let mut f = za.by_index(i).map_err(zip_err)?;
        let Some(rel) = f.enclosed_name() else { return Err(Error::InvalidPath(f.name().to_string())) };
        let out: PathBuf = target.join(rel);
        if f.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(p) = out.parent() {
            fs::create_dir_all(p)?;
        }
        io::copy(&mut f, &mut File::create(&out)?)?;
        n += 1;
    }
    Ok(n)
}

fn zip_err(e: zip::result::ZipError) -> Error {
    Error::Io(io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_rotation_restore() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("Thèse");
        fs::create_dir_all(root.join("Notes")).unwrap();
        fs::create_dir_all(root.join(".eloria/history")).unwrap();
        fs::create_dir_all(root.join(".eloria/trash")).unwrap();
        fs::write(root.join("Notes/a.md"), "# A").unwrap();
        fs::write(root.join(".eloria/index.db"), "jetable").unwrap();
        fs::write(root.join(".eloria/history/v1.md"), "ancienne").unwrap();
        fs::write(root.join(".eloria/trash/x.md"), "jeté").unwrap();
        let dest = d.path().join("Sauvegardes");
        // The backup folder cannot be inside the vault.
        assert!(create(&root, &root.join("bk"), 3).is_err());
        let b = create(&root, &dest, 2).unwrap();
        assert_eq!(b.files, 2);
        assert!(b.name.starts_with("Eloria-Thèse-") && b.name.ends_with(".zip"));
        // Rotation: three archives (distinct dated names), two kept.
        for s in ["20200101-000000", "20210101-000000"] {
            fs::copy(&b.path, dest.join(format!("Eloria-Thèse-{s}.zip"))).unwrap();
        }
        create(&root, &dest, 2).ok();
        let l = list(&dest, "Thèse").unwrap();
        assert_eq!(l.len(), 2);
        assert!(l.iter().all(|x| !x.name.contains("2020")));
        // Restore into an empty folder only.
        assert!(restore(Path::new(&l[0].path), &root).is_err());
        let back = d.path().join("Restauré");
        assert_eq!(restore(Path::new(&l[0].path), &back).unwrap(), 2);
        assert_eq!(fs::read_to_string(back.join("Notes/a.md")).unwrap(), "# A");
        assert!(back.join(".eloria/history/v1.md").exists());
        assert!(!back.join(".eloria/index.db").exists() && !back.join(".eloria/trash").exists());
    }
}
