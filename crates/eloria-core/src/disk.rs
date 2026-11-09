//! Vault disk usage: by folder, by file type, largest
//! files, Eloria's internal space (index, history, trash, PDF texts,
//! previews) and LaTeX auxiliary files that can be removed without loss.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

/// Extensions of files produced by a LaTeX compilation (recreated by the next one).
pub const LATEX_AUX: &[&str] = &["aux", "log", "out", "toc", "lof", "lot", "bbl", "bcf", "blg", "run.xml", "fdb_latexmk", "fls", "synctex.gz", "nav", "snm", "xdv"];

#[derive(Debug, Clone, Serialize, Default)]
pub struct DiskReport {
    pub total: u64,
    pub files: usize,
    /// (category, bytes, files): notes, PDFs, images, LaTeX, LaTeX auxiliaries, others
    pub by_type: Vec<(String, u64, usize)>,
    /// (top-level folder, bytes, files), largest first
    pub by_folder: Vec<(String, u64, usize)>,
    /// (path, bytes), the 15 largest files
    pub largest: Vec<(String, u64)>,
    /// (part of .eloria, bytes)
    pub internal: Vec<(String, u64)>,
    /// LaTeX auxiliary files (next to a .tex of the same name)
    pub aux: Vec<String>,
    pub aux_bytes: u64,
    pub trash_bytes: u64,
}

fn dir_size(p: &Path) -> u64 {
    let mut total = 0;
    let Ok(rd) = std::fs::read_dir(p) else { return 0 };
    for e in rd.flatten() {
        let Ok(m) = e.metadata() else { continue };
        total += if m.is_dir() { dir_size(&e.path()) } else { m.len() };
    }
    total
}

/// Auxiliary file of a compilation: known extension and a .tex of the same name next to it.
pub fn aux_of(name: &str) -> Option<&'static str> {
    let low = name.to_lowercase();
    LATEX_AUX.iter().copied().find(|ext| low.ends_with(&format!(".{ext}")))
}

pub fn scan(root: &Path) -> DiskReport {
    let mut r = DiskReport::default();
    let mut types: BTreeMap<&'static str, (u64, usize)> = BTreeMap::new();
    let mut folders: BTreeMap<String, (u64, usize)> = BTreeMap::new();
    let mut all: Vec<(String, u64)> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                if dir == root && (name == ".eloria" || name == ".git") {
                    continue;
                }
                stack.push(path);
                continue;
            }
            let rel = path.strip_prefix(root).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            let size = m.len();
            r.total += size;
            r.files += 1;
            let low = name.to_lowercase();
            let aux = aux_of(&name).filter(|ext| {
                let stem = &name[..name.len() - ext.len() - 1];
                path.with_file_name(format!("{stem}.tex")).is_file()
            });
            let kind = if aux.is_some() {
                r.aux.push(rel.clone());
                r.aux_bytes += size;
                "auxiliaires LaTeX"
            } else if low.ends_with(".md") {
                "notes"
            } else if low.ends_with(".pdf") {
                "PDF"
            } else if [".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp"].iter().any(|x| low.ends_with(x)) {
                "images"
            } else if low.ends_with(".tex") || low.ends_with(".bib") {
                "LaTeX"
            } else {
                "autres"
            };
            let t = types.entry(kind).or_default();
            t.0 += size;
            t.1 += 1;
            let top = rel.split('/').next().filter(|_| rel.contains('/')).unwrap_or("(racine)").to_string();
            let f = folders.entry(top).or_default();
            f.0 += size;
            f.1 += 1;
            all.push((rel, size));
        }
    }
    r.by_type = types.into_iter().map(|(k, (b, n))| (k.to_string(), b, n)).collect();
    r.by_type.sort_by(|a, b| b.1.cmp(&a.1));
    r.by_folder = folders.into_iter().map(|(k, (b, n))| (k, b, n)).collect();
    r.by_folder.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    all.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    all.truncate(15);
    r.largest = all;
    r.aux.sort();
    let el = root.join(".eloria");
    let mut internal = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&el) {
        for e in rd.flatten() {
            let size = if e.path().is_dir() { dir_size(&e.path()) } else { e.metadata().map(|m| m.len()).unwrap_or(0) };
            internal.push((e.file_name().to_string_lossy().to_string(), size));
        }
    }
    internal.sort_by(|a, b| b.1.cmp(&a.1));
    r.trash_bytes = internal.iter().find(|(n, _)| n == "trash").map(|x| x.1).unwrap_or(0);
    r.internal = internal;
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_analysis() {
        let d = tempfile::tempdir().unwrap();
        let w = |p: &str, n: usize| {
            let f = d.path().join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, vec![b'x'; n]).unwrap();
        };
        w("Notes/a.md", 100);
        w("PDF/x.pdf", 5000);
        w("Thèse/main.tex", 300);
        w("Thèse/main.aux", 40);
        w("Thèse/main.synctex.gz", 60);
        w("Thèse/journal.log", 10); // no journal.tex: not an auxiliary
        w(".eloria/index.db", 700);
        w(".eloria/trash/1/old.md", 20);
        let r = scan(d.path());
        assert_eq!((r.total, r.files), (5510, 6));
        assert_eq!(r.aux, vec!["Thèse/main.aux", "Thèse/main.synctex.gz"]);
        assert_eq!(r.aux_bytes, 100);
        assert_eq!(r.by_folder[0], ("PDF".to_string(), 5000, 1));
        assert_eq!(r.largest[0], ("PDF/x.pdf".to_string(), 5000));
        assert_eq!(r.trash_bytes, 20);
        assert!(r.internal.iter().any(|(n, b)| n == "index.db" && *b == 700));
        assert!(r.by_type.iter().any(|(k, b, _)| k == "auxiliaires LaTeX" && *b == 100));
    }
}
