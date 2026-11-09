//! Disk access to the vault. Every path exchanged with the interface is
//! relative to the root, separated by `/`, and checked never to leave the vault.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{Error, Result, ELORIA_DIR};

/// Extensions shown in the file tree (besides `.md` notes).
pub const ATTACHMENT_EXT: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "webp", "pdf"];
/// Text files editable in Eloria besides notes (LaTeX, bibliography, styles).
pub const TEXT_EXT: &[&str] = &["tex", "bib", "sty", "cls"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    /// "note", "image", "pdf" or "dir"
    pub kind: String,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
}

/// Folders ignored everywhere.
fn is_ignored_dir(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules"
}

/// Cleans a file name so it is valid on Windows, macOS and Linux.
pub fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    if cleaned.is_empty() {
        "Sans titre".to_string()
    } else {
        cleaned
    }
}

/// Note name without extension, from a relative path.
pub fn stem(rel: &str) -> String {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.strip_suffix(".md").unwrap_or(name).to_string()
}

impl Vault {
    /// Opens (and creates if needed) a vault.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        if !root.exists() {
            fs::create_dir_all(root)?;
        }
        let root = dunce(root.canonicalize()?);
        fs::create_dir_all(root.join(ELORIA_DIR))?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn eloria_dir(&self) -> PathBuf {
        self.root.join(ELORIA_DIR)
    }

    /// Absolute path from a relative path, with a safety check.
    pub fn resolve(&self, rel: &str) -> Result<PathBuf> {
        let rel = rel.replace('\\', "/");
        let rel = rel.trim_start_matches('/');
        let p = Path::new(rel);
        if p.is_absolute() {
            return Err(Error::InvalidPath(rel.to_string()));
        }
        for c in p.components() {
            match c {
                Component::Normal(_) | Component::CurDir => {}
                _ => return Err(Error::InvalidPath(rel.to_string())),
            }
        }
        Ok(self.root.join(p))
    }

    /// Relative path (with `/`) from an absolute path.
    pub fn rel_of(&self, abs: &Path) -> Option<String> {
        let abs = dunce(abs.to_path_buf());
        abs.strip_prefix(&self.root)
            .ok()
            .map(|p| p.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"))
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.resolve(rel).map(|p| p.exists()).unwrap_or(false)
    }

    /// Full tree of the vault.
    pub fn tree(&self) -> Result<TreeNode> {
        fn walk(v: &Vault, dir: &Path) -> Result<Vec<TreeNode>> {
            let mut out = Vec::new();
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                let path = entry.path();
                let ft = entry.file_type()?;
                if ft.is_dir() {
                    if is_ignored_dir(&name) {
                        continue;
                    }
                    out.push(TreeNode {
                        path: v.rel_of(&path).unwrap_or_default(),
                        name,
                        is_dir: true,
                        kind: "dir".into(),
                        children: walk(v, &path)?,
                    });
                } else if ft.is_file() {
                    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                    let kind = if ext == "md" {
                        "note"
                    } else if ext == "pdf" {
                        "pdf"
                    } else if ATTACHMENT_EXT.contains(&ext.as_str()) {
                        "image"
                    } else if TEXT_EXT.contains(&ext.as_str()) {
                        // LaTeX sources and bibliographies: opened in the LaTeX editor.
                        "tex"
                    } else {
                        continue;
                    };
                    out.push(TreeNode {
                        path: v.rel_of(&path).unwrap_or_default(),
                        name,
                        is_dir: false,
                        kind: kind.into(),
                        children: vec![],
                    });
                }
            }
            out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
            Ok(out)
        }
        Ok(TreeNode {
            name: self.root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            path: String::new(),
            is_dir: true,
            kind: "dir".into(),
            children: walk(self, &self.root)?,
        })
    }

    /// Every `.md` note with its modification time (Unix milliseconds).
    pub fn list_notes(&self) -> Vec<(String, i64)> {
        WalkDir::new(&self.root)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !(e.file_type().is_dir() && is_ignored_dir(&e.file_name().to_string_lossy())))
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file() && e.path().extension().map(|x| x == "md").unwrap_or(false))
            .filter_map(|e| {
                let rel = self.rel_of(e.path())?;
                Some((rel, mtime(e.path())))
            })
            .collect()
    }

    /// Every folder (for pickers).
    pub fn list_folders(&self) -> Vec<String> {
        let mut v: Vec<String> = WalkDir::new(&self.root)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| !(e.file_type().is_dir() && is_ignored_dir(&e.file_name().to_string_lossy())))
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_dir())
            .filter_map(|e| self.rel_of(e.path()))
            .collect();
        v.sort_by_key(|s| s.to_lowercase());
        v
    }

    /// Finds a file by name (for `![[image.png]]`): first relative to `near`, then anywhere.
    pub fn find_file(&self, name: &str, near: Option<&str>) -> Option<String> {
        let name = name.trim().replace('\\', "/");
        if self.exists(&name) && !name.is_empty() {
            return Some(name);
        }
        if let Some(dir) = near.and_then(|n| n.rsplit_once('/')).map(|(d, _)| d) {
            let cand = format!("{dir}/{name}");
            if self.exists(&cand) {
                return Some(cand);
            }
        }
        let file = name.rsplit('/').next()?.to_lowercase();
        WalkDir::new(&self.root)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !(e.file_type().is_dir() && is_ignored_dir(&e.file_name().to_string_lossy())))
            .filter_map(|e| e.ok())
            .find(|e| e.file_type().is_file() && e.file_name().to_string_lossy().to_lowercase() == file)
            .and_then(|e| self.rel_of(e.path()))
    }

    pub fn read(&self, rel: &str) -> Result<String> {
        let p = self.resolve(rel)?;
        if !p.exists() {
            return Err(Error::NotFound(rel.to_string()));
        }
        Ok(fs::read_to_string(p)?)
    }

    pub fn read_bytes(&self, rel: &str) -> Result<Vec<u8>> {
        Ok(fs::read(self.resolve(rel)?)?)
    }

    pub fn mtime(&self, rel: &str) -> Result<i64> {
        Ok(mtime(&self.resolve(rel)?))
    }

    /// Atomic write (temporary file, then rename).
    pub fn write(&self, rel: &str, content: &str) -> Result<()> {
        self.write_bytes(rel, content.as_bytes())
    }

    pub fn write_bytes(&self, rel: &str, bytes: &[u8]) -> Result<()> {
        let p = self.resolve(rel)?;
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = p.with_extension(format!(
            "{}.eloria-tmp",
            p.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default()
        ));
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, &p)?;
        Ok(())
    }

    /// Creates a note; error if it already exists.
    pub fn create_note(&self, rel: &str, content: &str) -> Result<()> {
        if self.exists(rel) {
            return Err(Error::AlreadyExists(rel.to_string()));
        }
        self.write(rel, content)
    }

    pub fn create_folder(&self, rel: &str) -> Result<()> {
        let p = self.resolve(rel)?;
        if p.exists() {
            return Err(Error::AlreadyExists(rel.to_string()));
        }
        fs::create_dir_all(p)?;
        Ok(())
    }

    /// First free path: `folder/Name.md`, `folder/Name 1.md`, ...
    pub fn unique_path(&self, folder: &str, name: &str, ext: &str) -> String {
        let folder = folder.trim_matches('/');
        let base = sanitize_file_name(name);
        let join = |n: &str| if folder.is_empty() { format!("{n}.{ext}") } else { format!("{folder}/{n}.{ext}") };
        let mut candidate = join(&base);
        let mut i = 1;
        while self.exists(&candidate) {
            candidate = join(&format!("{base} {i}"));
            i += 1;
        }
        candidate
    }

    /// Soft delete: moved to `.eloria/trash/<timestamp>/`.
    pub fn trash(&self, rel: &str) -> Result<String> {
        let src = self.resolve(rel)?;
        if !src.exists() {
            return Err(Error::NotFound(rel.to_string()));
        }
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let dest = self.eloria_dir().join("trash").join(stamp).join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&src, &dest)?;
        Ok(dest.to_string_lossy().to_string())
    }

    /// Renames or moves a file or folder.
    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        let a = self.resolve(from)?;
        let b = self.resolve(to)?;
        if !a.exists() {
            return Err(Error::NotFound(from.to_string()));
        }
        // Allows a case-only change (Windows/macOS are case-insensitive).
        if b.exists() && from.to_lowercase() != to.to_lowercase() {
            return Err(Error::AlreadyExists(to.to_string()));
        }
        if let Some(parent) = b.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(a, b)?;
        Ok(())
    }
}

fn mtime(p: &Path) -> i64 {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Removes the `\\?\` prefix that `canonicalize` adds on Windows.
fn dunce(p: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let s = p.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            if !rest.starts_with("UNC\\") {
                return PathBuf::from(rest.to_string());
            }
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_safety() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        assert!(v.resolve("../x.md").is_err());
        assert!(v.resolve("a/../../x.md").is_err());
        assert!(v.resolve("a/b.md").is_ok());
    }

    #[test]
    fn lifecycle() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        v.create_note("Dossier/Note.md", "x").unwrap();
        assert!(v.create_note("Dossier/Note.md", "y").is_err());
        assert_eq!(v.unique_path("Dossier", "Note", "md"), "Dossier/Note 1.md");
        v.rename("Dossier/Note.md", "Autre/N.md").unwrap();
        assert_eq!(v.read("Autre/N.md").unwrap(), "x");
        let t = v.tree().unwrap();
        assert_eq!(t.children.len(), 2);
        assert_eq!(v.list_notes().len(), 1);
        v.trash("Autre/N.md").unwrap();
        assert_eq!(v.list_notes().len(), 0);
        assert_eq!(sanitize_file_name("a:b/c?"), "a-b-c-");
        assert_eq!(stem("x/y/Note.md"), "Note");
    }
}
