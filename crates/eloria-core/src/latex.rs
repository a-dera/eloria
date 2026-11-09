//! LaTeX in Eloria: **local** compilation (never an online service).
//!
//! Engine, in order of preference: Tectonic (installed by Eloria or present on the
//! machine; it downloads and caches packages on first use), then latexmk
//! or pdflatex from MiKTeX or TeX Live. Log errors are collected with their
//! file and line, to jump there in one click.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::Result;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EngineKind {
    Tectonic,
    Latexmk,
    Pdflatex,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Engine {
    pub kind: EngineKind,
    pub path: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TexIssue {
    /// File relative to the document folder (empty if unknown)
    pub file: String,
    /// Line (1 = first), 0 if unknown
    pub line: u32,
    pub message: String,
    /// "error" or "warning"
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileResult {
    pub ok: bool,
    /// Produced PDF (absolute path)
    pub pdf: Option<String>,
    pub engine: String,
    pub issues: Vec<TexIssue>,
    /// End of the log (the last 6000 characters)
    pub log: String,
    pub millis: u64,
}

/// Folder where Eloria installs Tectonic (in the user's data).
pub fn install_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Eloria").join("tectonic")
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Looks for an executable in the PATH.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(exe(name))).find(|p| p.is_file())
}

fn version_of(path: &Path) -> String {
    crate::export::command(path)
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or_default().trim().to_string())
        .unwrap_or_default()
}

/// Available LaTeX engine: configured path, Tectonic installed by Eloria, then PATH.
pub fn find_engine(configured: &str) -> Option<Engine> {
    let mut cands: Vec<(EngineKind, PathBuf)> = Vec::new();
    if !configured.trim().is_empty() {
        let p = PathBuf::from(configured.trim());
        let name = p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        let kind = if name.contains("latexmk") {
            EngineKind::Latexmk
        } else if name.contains("pdflatex") {
            EngineKind::Pdflatex
        } else {
            EngineKind::Tectonic
        };
        cands.push((kind, p));
    }
    cands.push((EngineKind::Tectonic, install_dir().join(exe("tectonic"))));
    for (k, n) in [(EngineKind::Tectonic, "tectonic"), (EngineKind::Latexmk, "latexmk"), (EngineKind::Pdflatex, "pdflatex")] {
        if let Some(p) = which(n) {
            cands.push((k, p));
        }
    }
    cands.into_iter().find(|(_, p)| p.is_file()).map(|(kind, p)| Engine { kind, version: version_of(&p), path: p.to_string_lossy().to_string() })
}

/// Collects errors and warnings from a LaTeX or Tectonic log.
pub fn parse_log(log: &str) -> Vec<TexIssue> {
    let file_line = Regex::new(r"^(?:error: )?([^\s:][^:]*\.(?:tex|sty|cls|bib)):(\d+): (.+)$").unwrap();
    let bang = Regex::new(r"^! (.+)$").unwrap();
    let lnum = Regex::new(r"^l\.(\d+)").unwrap();
    let warn = Regex::new(r"(?:LaTeX|Package \w+) Warning: (.+?)(?: on input line (\d+))?\.?$").unwrap();
    let tect_warn = Regex::new(r"^warning: (?:([^\s:][^:]*\.tex):(\d+): )?(.+)$").unwrap();
    let mut out: Vec<TexIssue> = Vec::new();
    let lines: Vec<&str> = log.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i].trim_end();
        if let Some(c) = file_line.captures(l) {
            out.push(TexIssue { file: c[1].trim_start_matches("./").to_string(), line: c[2].parse().unwrap_or(0), message: c[3].trim().to_string(), level: "error".into() });
        } else if let Some(c) = bang.captures(l) {
            // Classic format: "! Message" then "l.12 …" a few lines below.
            let line = lines[i + 1..lines.len().min(i + 8)].iter().find_map(|x| lnum.captures(x).and_then(|c| c[1].parse().ok())).unwrap_or(0);
            if !out.iter().any(|o| o.line == line && o.message == c[1].trim()) {
                out.push(TexIssue { file: String::new(), line, message: c[1].trim().to_string(), level: "error".into() });
            }
        } else if let Some(c) = tect_warn.captures(l) {
            out.push(TexIssue {
                file: c.get(1).map(|m| m.as_str().to_string()).unwrap_or_default(),
                line: c.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0),
                message: c[3].trim().to_string(),
                level: "warning".into(),
            });
        } else if let Some(c) = warn.captures(l) {
            out.push(TexIssue { file: String::new(), line: c.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0), message: c[1].trim().to_string(), level: "warning".into() });
        }
        i += 1;
    }
    // Errors first, without duplicates.
    out.sort_by_key(|x| (x.level != "error", x.line));
    out.dedup_by(|a, b| a.message == b.message && a.line == b.line);
    out
}

/// Compiles `tex` (absolute path); the PDF is written next to it.
pub fn compile(engine: &Engine, tex: &Path) -> Result<CompileResult> {
    let dir = tex.parent().ok_or_else(|| Error::InvalidPath(tex.display().to_string()))?;
    let name = tex.file_name().ok_or_else(|| Error::InvalidPath(tex.display().to_string()))?;
    let start = Instant::now();
    let mut cmd = crate::export::command(&engine.path);
    cmd.current_dir(dir);
    match engine.kind {
        EngineKind::Tectonic => {
            cmd.arg(name).args(["--synctex", "--keep-logs", "--chatter", "minimal"]);
        }
        EngineKind::Latexmk => {
            cmd.args(["-pdf", "-interaction=nonstopmode", "-file-line-error", "-synctex=1", "-halt-on-error"]).arg(name);
        }
        EngineKind::Pdflatex => {
            cmd.args(["-interaction=nonstopmode", "-file-line-error", "-synctex=1"]).arg(name);
        }
    }
    let out = cmd.output().map_err(|e| Error::Export(format!("{} : {e}", engine.path)))?;
    let mut log = String::from_utf8_lossy(&out.stdout).to_string();
    log.push_str(&String::from_utf8_lossy(&out.stderr));
    // The full .log file, when present, gives line numbers.
    let log_file = tex.with_extension("log");
    if let Ok(full) = std::fs::read(&log_file) {
        log.push('\n');
        log.push_str(&String::from_utf8_lossy(&full));
    }
    let pdf = tex.with_extension("pdf");
    let issues = parse_log(&log);
    let ok = out.status.success() && pdf.is_file();
    let tail: String = {
        let chars: Vec<char> = log.chars().collect();
        chars[chars.len().saturating_sub(6000)..].iter().collect()
    };
    Ok(CompileResult {
        ok,
        pdf: pdf.is_file().then(|| pdf.to_string_lossy().to_string()),
        engine: format!("{:?}", engine.kind).to_lowercase(),
        issues,
        log: tail,
        millis: start.elapsed().as_millis() as u64,
    })
}

/// Keys cited in a LaTeX source (`\cite{a,b}`, `\textcite[p. 3]{c}`, `\autocite`…).
pub fn cited_keys(tex: &str) -> Vec<String> {
    let re = Regex::new(r"\\[a-zA-Z]*cite[a-zA-Z]*\*?(?:\[[^\]]*\]){0,2}\{([^}]*)\}").unwrap();
    let mut keys: Vec<String> = Vec::new();
    for c in re.captures_iter(tex) {
        for k in c[1].split(',') {
            let k = k.trim().to_string();
            if !k.is_empty() && !keys.contains(&k) {
                keys.push(k);
            }
        }
    }
    keys
}

/// `\label{…}` labels of a source (to complete `\ref{`).
pub fn labels(tex: &str) -> Vec<String> {
    Regex::new(r"\\label\{([^}]+)\}").unwrap().captures_iter(tex).map(|c| c[1].to_string()).collect()
}

/// Declared bibliography file (`\addbibresource{x.bib}` or `\bibliography{x}`).
pub fn bib_file(tex: &str) -> Option<String> {
    let re = Regex::new(r"\\(?:addbibresource|bibliography)\{([^}]+)\}").unwrap();
    re.captures(tex).map(|c| {
        let f = c[1].split(',').next().unwrap_or_default().trim().to_string();
        if f.ends_with(".bib") {
            f
        } else {
            format!("{f}.bib")
        }
    })
}

/// Address of the Tectonic archive for this machine (Windows ARM64 uses the x64 build,
/// emulated by Windows).
pub const TECTONIC_VERSION: &str = "0.15.0";
pub fn tectonic_url() -> String {
    let triple = if cfg!(windows) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        }
    } else {
        "x86_64-unknown-linux-musl"
    };
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    format!("https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%40{v}/tectonic-{v}-{triple}.{ext}", v = TECTONIC_VERSION)
}

/// Downloads Tectonic into `install_dir()` (Windows: .zip archive). Returns the engine.
pub fn install_tectonic() -> Result<Engine> {
    if !cfg!(windows) {
        return Err(Error::Export("installation automatique prévue pour Windows ; ailleurs, installe Tectonic avec le gestionnaire de paquets".into()));
    }
    let url = tectonic_url();
    let agent = crate::net::agent(Duration::from_secs(600));
    let mut resp = agent.get(&url).call().map_err(|e| Error::Net(format!("{url} : {e}")))?;
    if resp.status().as_u16() >= 400 {
        return Err(Error::Net(format!("{url} a répondu {}", resp.status())));
    }
    let bytes = resp.body_mut().with_config().limit(200 * 1024 * 1024).read_to_vec().map_err(|e| Error::Net(e.to_string()))?;
    let dir = install_dir();
    std::fs::create_dir_all(&dir)?;
    let mut za = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| Error::Export(e.to_string()))?;
    let mut found = false;
    for i in 0..za.len() {
        let mut f = za.by_index(i).map_err(|e| Error::Export(e.to_string()))?;
        let Some(name) = f.enclosed_name().and_then(|p| p.file_name().map(|n| n.to_owned())) else { continue };
        if name.to_string_lossy().eq_ignore_ascii_case("tectonic.exe") {
            let mut out = std::fs::File::create(dir.join("tectonic.exe"))?;
            std::io::copy(&mut f, &mut out)?;
            found = true;
        }
    }
    if !found {
        return Err(Error::Export("tectonic.exe absent de l'archive téléchargée".into()));
    }
    find_engine("").ok_or_else(|| Error::Export("Tectonic installé mais introuvable".into()))
}

/// LaTeX document templates (identifier, French name, content).
pub fn templates() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "article",
            "Article",
            "\\documentclass[11pt,a4paper]{article}\n\\usepackage[T1]{fontenc}\n\\usepackage[french]{babel}\n\\usepackage{csquotes}\n\\usepackage{amsmath,amssymb}\n\\usepackage{graphicx}\n\\usepackage[hidelinks]{hyperref}\n\\usepackage[backend=biber,style=authoryear]{biblatex}\n\\addbibresource{references.bib}\n\n\\title{Titre}\n\\author{Amédée DERA}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\n\\begin{abstract}\nRésumé.\n\\end{abstract}\n\n\\section{Introduction}\\label{sec:intro}\nTexte \\autocite{cle}.\n\n\\printbibliography\n\\end{document}\n",
        ),
        (
            "chapitre",
            "Chapitre de thèse",
            "\\documentclass[12pt,a4paper]{report}\n\\usepackage[T1]{fontenc}\n\\usepackage[french]{babel}\n\\usepackage{csquotes}\n\\usepackage{amsmath,amssymb}\n\\usepackage{graphicx}\n\\usepackage{booktabs}\n\\usepackage[hidelinks]{hyperref}\n\\usepackage[backend=biber,style=authoryear]{biblatex}\n\\addbibresource{references.bib}\n\n\\begin{document}\n\\chapter{État de l'art}\\label{ch:etat-art}\n\n\\section{Détection de la fraude}\\label{sec:fraude}\nTexte \\autocite{cle}.\n\n\\printbibliography[heading=subbibliography]\n\\end{document}\n",
        ),
        (
            "beamer",
            "Présentation (Beamer)",
            "\\documentclass{beamer}\n\\usepackage[T1]{fontenc}\n\\usepackage[french]{babel}\n\\usetheme{metropolis}\n\\title{Titre}\n\\author{Amédée DERA}\n\\date{\\today}\n\\begin{document}\n\\maketitle\n\\begin{frame}{Plan}\n\\tableofcontents\n\\end{frame}\n\\section{Contexte}\n\\begin{frame}{Contexte}\n\\begin{itemize}\n\\item Point\n\\end{itemize}\n\\end{frame}\n\\end{document}\n",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_read() {
        let log = "This is pdfTeX\n./chap.tex:12: Undefined control sequence.\nl.12 \\foo\n\n! LaTeX Error: File `x.sty' not found.\n\nType X to quit.\nl.3 \\usepackage\n\nLaTeX Warning: Reference `fig:a' on page 1 undefined on input line 20.\nerror: main.tex:7: Missing $ inserted\nwarning: main.tex:9: Overfull \\hbox (2.0pt too wide)\n";
        let i = parse_log(log);
        let errs: Vec<(&str, u32, &str)> = i.iter().filter(|x| x.level == "error").map(|x| (x.file.as_str(), x.line, x.message.as_str())).collect();
        assert!(errs.contains(&("chap.tex", 12, "Undefined control sequence.")));
        assert!(errs.contains(&("", 3, "LaTeX Error: File `x.sty' not found.")));
        assert!(errs.contains(&("main.tex", 7, "Missing $ inserted")));
        let warns: Vec<(u32, &str)> = i.iter().filter(|x| x.level == "warning").map(|x| (x.line, x.message.as_str())).collect();
        assert!(warns.contains(&(20, "Reference `fig:a' on page 1 undefined")));
        assert!(warns.iter().any(|w| w.0 == 9 && w.1.starts_with("Overfull")));
        // Errors before warnings.
        assert_eq!(i[0].level, "error");
    }

    #[test]
    fn citations_and_labels() {
        let tex = "Voir \\cite{a2019, b2020} et \\textcite[p.~3]{c2021}, \\autocite*{a2019}.\n\\label{sec:x} \\label{fig:y}\n\\addbibresource{refs.bib}";
        assert_eq!(cited_keys(tex), vec!["a2019", "b2020", "c2021"]);
        assert_eq!(labels(tex), vec!["sec:x", "fig:y"]);
        assert_eq!(bib_file(tex).as_deref(), Some("refs.bib"));
        assert_eq!(bib_file("\\bibliography{these}").as_deref(), Some("these.bib"));
        assert!(tectonic_url().contains("tectonic%400.15.0") && templates().len() == 3);
    }

    #[test]
    fn configured_engine_missing() {
        // A configured path that does not exist is ignored (no error).
        let e = find_engine("Z:/nulle/part/tectonic.exe");
        assert!(e.map(|e| e.path != "Z:/nulle/part/tectonic.exe").unwrap_or(true));
    }
}
