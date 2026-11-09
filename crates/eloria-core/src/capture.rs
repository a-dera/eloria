//! Capture and import: the inbox (everything that arrives before filing) and import
//! of Markdown folders from other tools (Obsidian, Notion, Logseq, Evernote export).

use std::path::Path;

use regex::Regex;
use serde::Serialize;
use walkdir::WalkDir;

use crate::lang::Lang;
use crate::vault::Vault;
use crate::Result;

/// Title of a capture: first useful line, shortened.
pub fn capture_title(text: &str, lang: Lang) -> String {
    // A link: the site, then the last path segment, in words.
    let t = text.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        let url = t.split_whitespace().next().unwrap_or(t);
        let rest = url.split("://").nth(1).unwrap_or(url);
        let host = rest.split('/').next().unwrap_or(rest).trim_start_matches("www.");
        let last = rest.trim_end_matches('/').rsplit('/').next().filter(|s| *s != host).unwrap_or("");
        let last = last.split(['?', '#']).next().unwrap_or("").trim_end_matches(".html").replace(['-', '_', '+'], " ");
        let words: String = last.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
        return if words.trim().is_empty() { host.to_string() } else { format!("{host} {}", words.trim()) };
    }
    let first = text
        .lines()
        .map(|l| l.trim().trim_start_matches(['#', '>', '-', '*', ' ']))
        .find(|l| !l.is_empty())
        .unwrap_or_default();
    let clean: String = first.chars().filter(|c| !"\\/:*?\"<>|[]#^".contains(*c)).collect();
    let words: Vec<&str> = clean.split_whitespace().take(9).collect();
    let t = words.join(" ");
    if t.is_empty() {
        format!("{} {}", lang.pick("Capture", "Capture"), chrono::Local::now().format("%Y-%m-%d %H-%M"))
    } else {
        t
    }
}

/// Content of a captured note: properties (type, date, source address), then the text.
pub fn capture_note(text: &str, url: Option<&str>) -> String {
    let url = url.map(str::trim).filter(|u| !u.is_empty()).map(String::from).or_else(|| {
        let t = text.trim();
        (t.starts_with("http://") || t.starts_with("https://")).then(|| t.split_whitespace().next().unwrap_or(t).to_string())
    });
    let mut fm = format!("---\ntype: capture\ncapture_le: {}\n", chrono::Local::now().format("%Y-%m-%d %H:%M"));
    if let Some(u) = &url {
        fm.push_str(&format!("source: \"{}\"\n", u.replace('"', "%22")));
    }
    fm.push_str("tags: [a-classer]\n---\n");
    format!("{fm}{}\n", text.trim_end())
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ImportReport {
    pub notes: usize,
    pub files: usize,
    pub links_rewritten: usize,
    pub skipped: usize,
    /// Vault folder where the import was placed
    pub folder: String,
}

/// File name without the identifier Notion adds ("Page 1a2b…32 hex").
pub fn strip_notion_id(name: &str) -> String {
    let re = Regex::new(r"\s+[0-9a-f]{32}(\.[A-Za-z0-9]+)?$").unwrap();
    re.replace(name, "$1").to_string()
}

/// Markdown links to pages (`[Title](Title%20abc….md)`) rewritten as `[[Title]]`.
pub fn rewrite_md_links(text: &str) -> (String, usize) {
    let re = Regex::new(r"\[([^\]]+)\]\(([^)\s]+\.md)\)").unwrap();
    let mut n = 0;
    let out = re
        .replace_all(text, |c: &regex::Captures| {
            if c[2].starts_with("http") {
                return c[0].to_string();
            }
            n += 1;
            let target = urldecode(c[2].rsplit('/').next().unwrap_or(&c[2]));
            let stem = strip_notion_id(target.trim_end_matches(".md"));
            if stem == c[1] {
                format!("[[{stem}]]")
            } else {
                format!("[[{stem}|{}]]", &c[1])
            }
        })
        .to_string();
    (out, n)
}

fn urldecode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Imports a Markdown folder into `target` (vault folder): notes and attachments
/// copied, structure kept, Notion identifiers removed, Markdown links rewritten.
/// Configuration folders (.obsidian, .trash, .git) are ignored; nothing is overwritten.
pub fn import_markdown_dir(vault: &Vault, src: &Path, target: &str) -> Result<ImportReport> {
    let mut r = ImportReport { folder: target.to_string(), ..Default::default() };
    for e in WalkDir::new(src).into_iter().filter_entry(|e| {
        let n = e.file_name().to_string_lossy();
        !(n.starts_with('.') && e.depth() > 0) && n != "node_modules"
    }) {
        let Ok(e) = e else { continue };
        if !e.file_type().is_file() {
            continue;
        }
        let Ok(rel) = e.path().strip_prefix(src) else { continue };
        let parts: Vec<String> = rel.components().map(|c| strip_notion_id(&c.as_os_str().to_string_lossy())).collect();
        let dest = format!("{}/{}", target.trim_matches('/'), parts.join("/"));
        if vault.exists(&dest) {
            r.skipped += 1;
            continue;
        }
        let ext = e.path().extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ext == "md" {
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            let (text, n) = rewrite_md_links(&text);
            r.links_rewritten += n;
            vault.write(&dest, &text)?;
            r.notes += 1;
        } else if crate::vault::ATTACHMENT_EXT.contains(&ext.as_str()) || ["csv", "txt", "bib", "tex"].contains(&ext.as_str()) {
            vault.write_bytes(&dest, &std::fs::read(e.path())?)?;
            r.files += 1;
        } else {
            r.skipped += 1;
        }
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture() {
        assert_eq!(capture_title("\n# Idée : graphes / fraude ?\nsuite", Lang::Fr), "Idée graphes fraude");
        assert!(capture_title("   ", Lang::Fr).starts_with("Capture 20"));
        assert_eq!(capture_title("https://www.exemple.org/blog/article-sur-la-fraude.html?x=1", Lang::Fr), "exemple.org article sur la fraude");
        assert_eq!(capture_title("https://exemple.org/", Lang::Fr), "exemple.org");
        let n = capture_note("https://exemple.org/article intéressant", None);
        assert!(n.contains("type: capture") && n.contains("source: \"https://exemple.org/article\"") && n.contains("a-classer"));
        assert!(!capture_note("Juste du texte", None).contains("source:"));
    }

    #[test]
    fn notion_and_obsidian_import() {
        assert_eq!(strip_notion_id("Ma page 0123456789abcdef0123456789abcdef.md"), "Ma page.md");
        assert_eq!(strip_notion_id("Dossier 0123456789abcdef0123456789abcdef"), "Dossier");
        let (t, n) = rewrite_md_links("Voir [Ma page](Ma%20page%200123456789abcdef0123456789abcdef.md) et [site](https://x.org/a.md) et [autre nom](Sous/Cible.md).");
        assert_eq!(t, "Voir [[Ma page]] et [site](https://x.org/a.md) et [[Cible|autre nom]].");
        assert_eq!(n, 2);

        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("export");
        std::fs::create_dir_all(src.join("Projet 0123456789abcdef0123456789abcdef")).unwrap();
        std::fs::create_dir_all(src.join(".obsidian")).unwrap();
        std::fs::write(src.join(".obsidian/app.json"), "{}").unwrap();
        std::fs::write(src.join("Accueil.md"), "[Projet](Projet%200123456789abcdef0123456789abcdef/Plan%200123456789abcdef0123456789abcdef.md)").unwrap();
        std::fs::write(src.join("Projet 0123456789abcdef0123456789abcdef/Plan 0123456789abcdef0123456789abcdef.md"), "# Plan").unwrap();
        std::fs::write(src.join("Projet 0123456789abcdef0123456789abcdef/schema.png"), b"png").unwrap();
        std::fs::write(src.join("divers.exe"), b"x").unwrap();
        let v = Vault::open(d.path().join("coffre")).unwrap();
        let r = import_markdown_dir(&v, &src, "Importé/export").unwrap();
        assert_eq!((r.notes, r.files, r.links_rewritten, r.skipped), (2, 1, 1, 1));
        assert_eq!(v.read("Importé/export/Accueil.md").unwrap(), "[[Plan|Projet]]");
        assert!(v.exists("Importé/export/Projet/Plan.md") && v.exists("Importé/export/Projet/schema.png"));
        assert!(!v.exists("Importé/export/.obsidian/app.json"));
        // Second import: nothing is overwritten.
        assert_eq!(import_markdown_dir(&v, &src, "Importé/export").unwrap().skipped, 4);
    }
}
