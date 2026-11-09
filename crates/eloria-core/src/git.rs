//! Git history of the vault (git installed on the machine): saved versions and,
//! if a private remote is configured, push and pull. The index and the
//! trash are never versioned.

use std::path::Path;

use serde::Serialize;

use crate::error::Error;
use crate::Result;

const IGNORE: &str = "# Eloria : fichiers reconstruits ou temporaires\n.eloria/index.db*\n.eloria/trash/\n.eloria/tmp/\n.eloria/stats.json\n";

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct GitStatus {
    /// git is installed
    pub available: bool,
    /// the vault is a Git repository
    pub repo: bool,
    pub branch: String,
    /// files changed since the last version
    pub changes: usize,
    pub remote: String,
    /// latest versions: (short hash, date, message)
    pub log: Vec<(String, String, String)>,
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let out = crate::export::command("git").current_dir(root).args(args).output().map_err(|e| Error::Export(format!("git : {e}")))?;
    if !out.status.success() {
        return Err(Error::Export(format!("git {} : {}", args.first().unwrap_or(&""), String::from_utf8_lossy(&out.stderr).trim())));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn status(root: &Path) -> GitStatus {
    let mut s = GitStatus { available: crate::export::command("git").arg("--version").output().map(|o| o.status.success()).unwrap_or(false), ..Default::default() };
    if !s.available || !root.join(".git").exists() {
        return s;
    }
    s.repo = true;
    s.branch = git(root, &["branch", "--show-current"]).unwrap_or_default().trim().to_string();
    s.changes = git(root, &["status", "--porcelain"]).map(|o| o.lines().count()).unwrap_or(0);
    s.remote = git(root, &["remote", "get-url", "origin"]).unwrap_or_default().trim().to_string();
    s.log = git(root, &["log", "-n", "10", "--date=format:%Y-%m-%d %H:%M", "--pretty=%h\t%ad\t%s"])
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut p = l.splitn(3, '\t');
            Some((p.next()?.to_string(), p.next()?.to_string(), p.next()?.to_string()))
        })
        .collect();
    s
}

/// Turns the vault into a Git repository (with a suitable .gitignore).
pub fn init(root: &Path) -> Result<GitStatus> {
    if !root.join(".git").exists() {
        git(root, &["init", "-q"])?;
    }
    let gi = root.join(".gitignore");
    let cur = std::fs::read_to_string(&gi).unwrap_or_default();
    if !cur.contains(".eloria/index.db") {
        std::fs::write(&gi, format!("{cur}{}{IGNORE}", if cur.is_empty() || cur.ends_with('\n') { "" } else { "\n" }))?;
    }
    Ok(status(root))
}

/// Saves a version of the whole vault. Without a configured Git identity, the version
/// is signed "Eloria" (nothing personal is made up).
pub fn commit(root: &Path, message: &str) -> Result<GitStatus> {
    git(root, &["add", "-A"])?;
    if git(root, &["status", "--porcelain"])?.trim().is_empty() {
        return Ok(status(root));
    }
    let has_identity = git(root, &["config", "user.email"]).map(|e| !e.trim().is_empty()).unwrap_or(false);
    let msg = if message.trim().is_empty() { format!("Eloria : version du {}", chrono::Local::now().format("%Y-%m-%d %H:%M")) } else { message.trim().to_string() };
    if has_identity {
        git(root, &["commit", "-q", "-m", &msg])?;
    } else {
        git(root, &["-c", "user.name=Eloria", "-c", "user.email=eloria@localhost", "commit", "-q", "-m", &msg])?;
    }
    Ok(status(root))
}

/// Pushes to, then pulls from, the remote repository (`origin`).
pub fn sync(root: &Path) -> Result<GitStatus> {
    let branch = git(root, &["branch", "--show-current"])?.trim().to_string();
    git(root, &["pull", "--rebase", "-q", "origin", &branch]).ok();
    git(root, &["push", "-q", "-u", "origin", &branch])?;
    Ok(status(root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_versions() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let s = status(root);
        if !s.available {
            return; // git missing on the test machine
        }
        assert!(!s.repo);
        std::fs::create_dir_all(root.join(".eloria/trash")).unwrap();
        std::fs::write(root.join(".eloria/index.db"), "x").unwrap();
        std::fs::write(root.join(".eloria/trash/x.md"), "x").unwrap();
        std::fs::write(root.join("Note.md"), "# Note").unwrap();
        let s = init(root).unwrap();
        assert!(s.repo && s.changes >= 2);
        let s = commit(root, "Première version").unwrap();
        assert_eq!(s.changes, 0);
        assert_eq!(s.log[0].2, "Première version");
        let tracked = git(root, &["ls-files"]).unwrap();
        assert!(tracked.contains("Note.md") && !tracked.contains("index.db") && !tracked.contains("trash"));
        // Nothing new: no empty version.
        assert_eq!(commit(root, "").unwrap().log.len(), 1);
        std::fs::write(root.join("Note.md"), "# Note modifiée").unwrap();
        assert_eq!(status(root).changes, 1);
        assert!(commit(root, "").unwrap().log[0].2.starts_with("Eloria : version du"));
    }
}
