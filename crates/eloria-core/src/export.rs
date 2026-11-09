//! Export: standalone HTML (no dependency) and Pandoc (Word, PDF, ODT, LaTeX).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::markdown::{split_frontmatter, to_html};
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    /// docx, pdf, odt, tex, html
    pub format: String,
    /// Absolute path of the output file
    pub output: String,
    /// Include the bibliography (`[@key]` citations resolved through the Zotero mirror)
    pub citations: bool,
    /// Remove the Zotero annotation block
    pub strip_annotations: bool,
    /// Table of contents
    pub toc: bool,
}

fn callout_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^(>\s*)\[!([\w\-]+)(?:\|[\w\-]+)?\][+-]?\s*(.*)$").unwrap())
}

fn wikilink_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"!?\[\[([^\]\|#\n]*)(?:#([^\]\|\n]*))?(?:\|([^\]\n]*))?\]\]").unwrap())
}

/// Prepares Eloria's Markdown for Pandoc: internal links -> text, callouts -> quotes
/// in bold, Zotero block removed if requested. Front matter is kept (Pandoc reads `title`).
pub fn prepare_for_pandoc(content: &str, strip_annotations: bool) -> String {
    let (yaml, body, _) = split_frontmatter(content);
    let mut body = body.to_string();
    if strip_annotations {
        if let (Some(s), Some(e)) = (body.find(crate::fiche::BLOCK_START), body.find(crate::fiche::BLOCK_END)) {
            if e > s {
                body.replace_range(s..e + crate::fiche::BLOCK_END.len(), "");
            }
        }
    }
    let body = wikilink_re().replace_all(&body, |c: &regex::Captures| {
        c.get(3)
            .or(c.get(1).filter(|m| !m.as_str().is_empty()))
            .or(c.get(2))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    });
    let body = callout_re().replace_all(&body, |c: &regex::Captures| {
        let label = if c[3].trim().is_empty() { capitalize(&c[2]) } else { c[3].trim().to_string() };
        format!("{}**{}**", &c[1], label)
    });
    // Title: Pandoc expects `title`, Eloria often uses `titre`.
    let header = match yaml {
        Some(y) => {
            let mut map: serde_yaml::Mapping = serde_yaml::from_str(y).unwrap_or_default();
            let t = serde_yaml::Value::String("title".into());
            if !map.contains_key(&t) {
                if let Some(v) = map.get(serde_yaml::Value::String("titre".into())).cloned() {
                    map.insert(t, v);
                }
            }
            // Eloria-specific keys Pandoc does not need
            for k in ["tags", "aliases", "statut", "chapitres", "priorite", "zotero", "type", "cree_le"] {
                map.remove(serde_yaml::Value::String(k.into()));
            }
            if map.is_empty() { String::new() } else { format!("---\n{}---\n", serde_yaml::to_string(&map).unwrap_or_default()) }
        }
        None => String::new(),
    };
    format!("{header}{body}")
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// External process without a console window (otherwise a console flashes on
/// Windows, since the application is a GUI application).
pub(crate) fn command(bin: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(bin);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// Pandoc version, if available.
pub fn pandoc_version(pandoc: &str) -> Option<String> {
    let bin = if pandoc.is_empty() { "pandoc" } else { pandoc };
    let out = command(bin).arg("--version").output().ok()?;
    String::from_utf8_lossy(&out.stdout).lines().next().map(str::to_string)
}

fn has_program(name: &str) -> bool {
    command(name).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Exports through Pandoc. `bib` is an already written .bib file (or None).
pub fn pandoc_export(
    pandoc: &str,
    content: &str,
    resource_dirs: &[PathBuf],
    work_dir: &Path,
    opts: &ExportOptions,
    bib: Option<&Path>,
    csl: Option<&Path>,
) -> Result<PathBuf> {
    let bin = if pandoc.is_empty() { "pandoc" } else { pandoc };
    std::fs::create_dir_all(work_dir)?;
    let input = work_dir.join("export.md");
    std::fs::write(&input, prepare_for_pandoc(content, opts.strip_annotations))?;
    let output = PathBuf::from(&opts.output);
    let mut cmd = command(bin);
    cmd.arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--from=markdown")
        .arg("--standalone");
    let sep = if cfg!(windows) { ";" } else { ":" };
    let res = resource_dirs.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join(sep);
    if !res.is_empty() {
        cmd.arg(format!("--resource-path={res}"));
    }
    if opts.toc {
        cmd.arg("--toc");
    }
    if opts.citations {
        if let Some(b) = bib {
            cmd.arg("--citeproc").arg(format!("--bibliography={}", b.display()));
            if let Some(c) = csl {
                cmd.arg(format!("--csl={}", c.display()));
            }
        }
    }
    if opts.format == "pdf" {
        // Typst is light and enough; otherwise Pandoc uses LaTeX if installed.
        if has_program("typst") {
            cmd.arg("--pdf-engine=typst");
        } else if !has_program("pdflatex") && !has_program("xelatex") {
            return Err(Error::Export(
                "l'export PDF nécessite Typst (recommandé : `winget install Typst.Typst`) ou une distribution LaTeX".into(),
            ));
        }
    }
    if opts.format == "html" {
        cmd.arg("--embed-resources").arg("--mathml");
    }
    let out = cmd.output().map_err(|e| Error::Export(format!("Pandoc introuvable ({e}). Installe-le : `winget install JohnMacFarlane.Pandoc`")))?;
    if !out.status.success() {
        return Err(Error::Export(String::from_utf8_lossy(&out.stderr).to_string()));
    }
    Ok(output)
}

/// Standalone HTML without Pandoc (styles included).
pub fn standalone_html(content: &str, title: &str) -> String {
    let body = to_html(&prepare_for_pandoc(content, false));
    format!(
        r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{}</title><style>
body{{max-width:760px;margin:3rem auto;padding:0 1rem;font:17px/1.7 Georgia,serif;color:#222}}
h1,h2,h3{{font-family:system-ui,sans-serif;line-height:1.25}} code,pre{{font-family:Consolas,monospace;background:#f4f4f5}}
pre{{padding:1rem;overflow:auto;border-radius:6px}} blockquote{{border-left:3px solid #7c5cff;margin:0;padding:.2rem 1rem;color:#444;background:#f7f5ff}}
table{{border-collapse:collapse}} td,th{{border:1px solid #ddd;padding:.3rem .6rem}} img{{max-width:100%}}
</style></head><body>{}</body></html>"#,
        html_escape(title),
        body
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparation() {
        let src = "---\ntitre: Ma note\ntags: [a]\n---\nVoir [[Note|alias]] et [[Autre]].\n> [!citation|vert] p. 7 · lien\n> texte\n<!-- zotero:debut -->\nX\n<!-- zotero:fin -->\nFin";
        let out = prepare_for_pandoc(src, true);
        assert!(out.contains("title: Ma note"));
        assert!(!out.contains("tags"));
        assert!(out.contains("Voir alias et Autre."));
        assert!(out.contains("> **p. 7 · lien**"));
        assert!(!out.contains("zotero:debut"));
        assert!(standalone_html(src, "T").contains("<blockquote>"));
    }
}
