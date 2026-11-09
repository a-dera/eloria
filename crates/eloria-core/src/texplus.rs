//! Advanced LaTeX: source ↔ PDF sync (SyncTeX), preview of a TikZ snippet or
//! formula compiled separately, the user's formula library (a Markdown
//! note: "## Name" then the formula between `$$`).

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::latex::{CompileResult, Engine};
use crate::Result;

/// Factor from sp to PostScript point (bp).
const SP_PER_BP: f64 = 65781.76;

/// SyncTeX record: page, file (tag), line, position in bp from the top-left
/// corner of the page.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncRecord {
    pub page: u32,
    pub tag: u32,
    pub line: u32,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Default)]
pub struct SyncMap {
    /// (tag, path as written by the engine)
    pub inputs: Vec<(u32, String)>,
    pub records: Vec<SyncRecord>,
}

/// Reads a `.synctex` file (text); paths may not be UTF-8.
pub fn parse_synctex(bytes: &[u8]) -> SyncMap {
    let text = String::from_utf8_lossy(bytes);
    let mut map = SyncMap::default();
    let mut unit = 1.0f64;
    let mut page = 0u32;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Input:") {
            if let Some((tag, path)) = rest.split_once(':') {
                if let Ok(t) = tag.parse() {
                    map.inputs.push((t, path.to_string()));
                }
            }
            continue;
        }
        if let Some(u) = line.strip_prefix("Unit:") {
            unit = u.trim().parse().unwrap_or(1.0);
            continue;
        }
        if let Some(p) = line.strip_prefix('{') {
            page = p.trim().parse().unwrap_or(page);
            continue;
        }
        let Some(first) = line.chars().next() else { continue };
        if !matches!(first, '[' | '(' | 'v' | 'h' | 'x' | 'k' | 'g' | '$') || page == 0 {
            continue;
        }
        // <type><tag>,<line>[,<col>]:<x>,<y>[:…]
        let body = &line[first.len_utf8()..];
        let Some((head, pos)) = body.split_once(':') else { continue };
        let mut h = head.split(',');
        let (Some(tag), Some(ln)) = (h.next().and_then(|v| v.parse::<u32>().ok()), h.next().and_then(|v| v.parse::<u32>().ok())) else { continue };
        let mut p = pos.split(':').next().unwrap_or_default().split(',');
        let (Some(x), Some(y)) = (p.next().and_then(|v| v.parse::<f64>().ok()), p.next().and_then(|v| v.parse::<f64>().ok())) else { continue };
        if ln == 0 {
            continue;
        }
        map.records.push(SyncRecord { page, tag, line: ln, x: x * unit / SP_PER_BP, y: y * unit / SP_PER_BP });
    }
    map
}

/// Reads `<pdf without extension>.synctex.gz` (or `.synctex`).
pub fn load_synctex(pdf: &Path) -> Option<SyncMap> {
    let gz = pdf.with_extension("synctex.gz");
    if let Ok(f) = std::fs::File::open(&gz) {
        let mut buf = Vec::new();
        flate2::read::GzDecoder::new(f).read_to_end(&mut buf).ok()?;
        return Some(parse_synctex(&buf));
    }
    std::fs::read(pdf.with_extension("synctex")).ok().map(|b| parse_synctex(&b))
}

fn same_file(a: &str, b: &Path) -> bool {
    let norm = |s: &str| s.replace('\\', "/").replace("/./", "/").to_lowercase();
    let b = norm(&b.to_string_lossy());
    let a = norm(a);
    // Non-UTF-8 paths ("Th�se"): compare the end, from the file name.
    a == b || (a.ends_with(&b.rsplit('/').next().unwrap_or_default().to_string()) && {
        let an = a.replace('\u{fffd}', "?");
        let bn = b.chars().map(|c| if c.is_ascii() { c } else { '?' }).collect::<String>();
        an.len() >= 4 && (an.ends_with(&bn[bn.len().saturating_sub(an.len().min(bn.len()))..]) || bn.ends_with(an.rsplit('/').next().unwrap_or_default()))
    })
}

/// Source -> PDF: page and height (bp from the top) of line `line` of file `tex`
/// (or of the nearest following line that produces content).
pub fn forward(map: &SyncMap, tex: &Path, line: u32) -> Option<(u32, f64)> {
    let tags: Vec<u32> = map.inputs.iter().filter(|(_, p)| same_file(p, tex)).map(|(t, _)| *t).collect();
    let mut best: Option<&SyncRecord> = None;
    for r in map.records.iter().filter(|r| tags.contains(&r.tag) && r.line >= line) {
        if best.map(|b| r.line < b.line || (r.line == b.line && (r.page, r.y as i64) < (b.page, b.y as i64))).unwrap_or(true) {
            best = Some(r);
        }
    }
    best.map(|r| (r.page, r.y))
}

/// PDF -> source: the line closest to the point (page, y in bp from the top).
/// Returns (path written by the engine, line). Distribution files (.cls,
/// .sty) are ignored.
pub fn inverse(map: &SyncMap, page: u32, y: f64) -> Option<(String, u32)> {
    let ok_tags: Vec<u32> = map.inputs.iter().filter(|(_, p)| p.to_lowercase().ends_with(".tex")).map(|(t, _)| *t).collect();
    let r = map
        .records
        .iter()
        .filter(|r| r.page == page && ok_tags.contains(&r.tag))
        .min_by(|a, b| (a.y - y).abs().partial_cmp(&(b.y - y).abs()).unwrap_or(std::cmp::Ordering::Equal))?;
    let path = map.inputs.iter().find(|(t, _)| *t == r.tag)?.1.clone();
    Some((path, r.line))
}

/// Standalone document for a preview: TikZ (`standalone` + tikz) or formula (`$…$`).
pub fn standalone(snippet: &str) -> String {
    let body = snippet.trim();
    let is_math = !body.contains("\\begin{tikzpicture}") && !body.contains("\\begin{figure}") && !body.contains("\\begin{table}");
    let inner = if is_math && !body.starts_with("\\begin{") && !body.starts_with('$') { format!("$\\displaystyle {body}$") } else { body.replace("\\begin{figure}[ht]", "").replace("\\begin{figure}", "").replace("\\end{figure}", "").replace("\\centering", "") };
    // A figure's caption and label make no sense outside the document.
    let inner = regex::Regex::new(r"\\(caption|label)\{[^}]*\}").unwrap().replace_all(&inner, "").into_owned();
    format!(
        "\\documentclass[border=6pt,varwidth]{{standalone}}\n\\usepackage[T1]{{fontenc}}\n\\usepackage{{amsmath,amssymb,booktabs}}\n\\usepackage{{tikz}}\n\\usetikzlibrary{{arrows.meta,positioning,shapes,calc,fit,backgrounds}}\n\\begin{{document}}\n{inner}\n\\end{{document}}\n"
    )
}

/// Compiles a snippet separately (folder `dir`, erased and recreated); returns the result.
pub fn preview(engine: &Engine, snippet: &str, dir: &Path) -> Result<CompileResult> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir)?;
    let tex: PathBuf = dir.join("apercu.tex");
    std::fs::write(&tex, standalone(snippet))?;
    crate::latex::compile(engine, &tex)
}

/// A formula of the library.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Formula {
    pub name: String,
    pub latex: String,
    /// Category (level-1 heading above, otherwise empty)
    pub group: String,
}

/// Formulas of a "formula library" note: `## Name` then `$$ … $$`.
pub fn parse_formulas(md: &str) -> Vec<Formula> {
    let (_, body, _) = crate::markdown::split_frontmatter(md);
    let mut out = Vec::new();
    let mut group = String::new();
    let mut name = String::new();
    let mut inside = false;
    let mut buf = String::new();
    for line in body.lines() {
        let t = line.trim();
        if !inside {
            if let Some(g) = t.strip_prefix("# ") {
                group = g.trim().to_string();
            } else if let Some(n) = t.strip_prefix("## ") {
                name = n.trim().to_string();
            } else if let Some(rest) = t.strip_prefix("$$") {
                if let Some(one) = rest.strip_suffix("$$") {
                    if !one.trim().is_empty() && !name.is_empty() {
                        out.push(Formula { name: name.clone(), latex: one.trim().to_string(), group: group.clone() });
                    }
                } else {
                    inside = true;
                    buf = rest.to_string();
                }
            }
        } else if let Some(end) = t.strip_suffix("$$") {
            buf.push('\n');
            buf.push_str(end);
            inside = false;
            if !name.is_empty() {
                out.push(Formula { name: name.clone(), latex: buf.trim().to_string(), group: group.clone() });
            }
            buf.clear();
        } else {
            buf.push('\n');
            buf.push_str(line);
        }
    }
    out
}

/// Adds a formula to the note (created with its header if needed).
pub fn append_formula(md: Option<&str>, name: &str, latex: &str, lang: crate::lang::Lang) -> String {
    let base = md.map(str::to_string).unwrap_or_else(|| {
        format!(
            "---\ntype: formules\n---\n# {}\n\n{}\n",
            lang.pick("Mes formules", "My formulas"),
            lang.pick("Bibliothèque de formules d'Eloria : un titre « ## Nom » puis la formule entre $$.", "Eloria formula library: a “## Name” heading then the formula between $$.")
        )
    });
    format!("{}\n\n## {}\n\n$$\n{}\n$$\n", base.trim_end(), name.trim(), latex.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYN: &str = "SyncTeX Version:1\nInput:1:C:\\Users\\me\\Vault\\Th\u{fffd}se\\test.tex\nInput:2:C:\\miktex\\article.cls\nOutput:pdf\nMagnification:1000\nUnit:1\nX Offset:0\nY Offset:0\nContent:\n!1234\n{1\n[1,20:4736286,9000000:26607616,44616926,0\nh1,22:4736286,12000000:0,0,0\nx1,23:4736286,14000000\n(2,99:4736286,15000000:0,0,0\n}1\n{2\nh1,30:4736286,5000000:0,0,0\n}2\n";

    #[test]
    fn synctex_round_trip() {
        let m = parse_synctex(SYN.as_bytes());
        assert_eq!(m.inputs.len(), 2);
        assert_eq!(m.records.len(), 5);
        assert!((m.records[0].x - 72.0).abs() < 0.1, "{}", m.records[0].x);
        let tex = Path::new("C:/Users/me/Vault/Thèse/test.tex");
        // Line 23: page 1; line 25 (no content): the next one that produces some, page 2.
        assert_eq!(forward(&m, tex, 23).map(|(p, y)| (p, y.round() as i64)), Some((1, 213)));
        assert_eq!(forward(&m, tex, 25).map(|(p, _)| p), Some(2));
        assert_eq!(forward(&m, Path::new("autre.tex"), 1), None);
        // Click at 182 bp on page 1: line 22; .cls files are ignored.
        assert_eq!(inverse(&m, 1, 182.0).map(|(_, l)| l), Some(22));
        assert_eq!(inverse(&m, 2, 70.0).map(|(_, l)| l), Some(30));
    }

    #[test]
    fn preview_and_formulas() {
        let s = standalone("\\frac{a}{b}");
        assert!(s.contains("\\documentclass[border=6pt,varwidth]{standalone}") && s.contains("$\\displaystyle \\frac{a}{b}$"));
        let f = standalone("\\begin{figure}[ht]\n\\centering\n\\begin{tikzpicture}\\node{A};\\end{tikzpicture}\n\\caption{X}\\label{fig:x}\n\\end{figure}");
        assert!(f.contains("\\begin{tikzpicture}") && !f.contains("\\caption") && !f.contains("\\begin{figure}"));
        let md = append_formula(None, "F1", "F_1 = \\frac{2PR}{P+R}", crate::lang::Lang::Fr);
        let md = append_formula(Some(&md), "Softmax", "\\sigma(z)_i = \\frac{e^{z_i}}{\\sum_j e^{z_j}}", crate::lang::Lang::Fr);
        let fs = parse_formulas(&md);
        assert_eq!(fs.len(), 2);
        assert_eq!((fs[0].name.as_str(), fs[0].group.as_str()), ("F1", "Mes formules"));
        assert_eq!(fs[1].latex, "\\sigma(z)_i = \\frac{e^{z_i}}{\\sum_j e^{z_j}}");
        assert_eq!(parse_formulas("## Inline\n\n$$x^2$$\n")[0].latex, "x^2");
    }
}
