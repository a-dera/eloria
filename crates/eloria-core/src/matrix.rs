//! Research matrix: a comparative table of articles, filled from the reading notes
//! (one row per note, one column per section of the Eloria method). Exportable as
//! CSV, Markdown and LaTeX.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::lang::Lang;

/// Matrix columns: (identifier, French title, English title, reading-note sections read).
/// Several headings are accepted (old reading-note template, hand-written notes).
pub const COLUMNS: &[(&str, &str, &str, &[&str])] = &[
    ("problematique", "Problématique", "Research problem", &["problématique", "problem statement", "research problem"]),
    ("objectifs", "Objectifs", "Objectives", &["objectifs", "objectives"]),
    ("methodes", "Méthodes", "Methods", &["méthodes", "méthode et données", "methods"]),
    ("donnees", "Données", "Data", &["sources de données", "données", "data sources", "data"]),
    ("resultats", "Résultats", "Results", &["résultats", "résultats clés", "results"]),
    ("limites", "Limites", "Limitations", &["contraintes rencontrées par les auteurs", "limites", "constraints", "limitations"]),
    ("defis", "Défis", "Challenges", &["défis", "challenges"]),
    ("perspectives", "Perspectives", "Future work", &["perspectives", "future work"]),
    ("apport", "Apport", "Contribution", &["commentaire", "apport pour ma thèse", "commentary", "contribution"]),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatrixRow {
    /// Reading note the cells come from
    pub path: String,
    pub citekey: String,
    pub title: String,
    /// "Smith & Ouédraogo (2024)"
    pub author_year: String,
    pub year: Option<i32>,
    pub statut: String,
    /// One cell per column, in `COLUMNS` order (short text, one line)
    pub cells: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Matrix {
    /// (identifier, title) of the columns, in the interface language
    pub columns: Vec<(String, String)>,
    pub rows: Vec<MatrixRow>,
}

/// `## Heading` sections of a note (lower-case heading -> raw content).
pub fn sections(body: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut cur: Option<String> = None;
    let mut buf = String::new();
    let mut in_code = false;
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") {
            in_code = !in_code;
        }
        if !in_code && t.starts_with("## ") {
            if let Some(c) = cur.take() {
                out.insert(c, buf.trim().to_string());
            }
            cur = Some(t[3..].trim().to_lowercase());
            buf.clear();
        } else if !in_code && (t.starts_with("# ") || t.starts_with("<!--")) {
            if let Some(c) = cur.take() {
                out.insert(c, buf.trim().to_string());
            }
            buf.clear();
        } else if cur.is_some() {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    if let Some(c) = cur {
        out.insert(c, buf.trim().to_string());
    }
    out
}

/// Text of a cell: one line, without heavy formatting, cut cleanly.
pub fn cell_text(md: &str, max: usize) -> String {
    let mut s = String::new();
    let mut in_code = false;
    for line in md.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code || t.is_empty() || t.starts_with("> [!") {
            continue;
        }
        let t = t.trim_start_matches(['>', '-', '*', '+', ' ']).trim();
        if !s.is_empty() {
            s.push_str(" ; ");
        }
        s.push_str(t);
    }
    let s = s.replace("**", "").replace("==", "");
    if s.chars().count() > max {
        let cut: String = s.chars().take(max).collect();
        format!("{}…", cut.trim_end())
    } else {
        s
    }
}

pub fn row_cells(body: &str, max: usize) -> Vec<String> {
    let secs = sections(body);
    COLUMNS
        .iter()
        .map(|(_, _, _, names)| names.iter().find_map(|n| secs.get(*n)).map(|c| cell_text(c, max)).unwrap_or_default())
        .collect()
}

pub fn columns(lang: Lang) -> Vec<(String, String)> {
    COLUMNS.iter().map(|(id, fr, en, _)| (id.to_string(), lang.pick(fr, en).to_string())).collect()
}

fn esc_csv(s: &str) -> String {
    if s.contains([',', '"', '\n', ';']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn to_csv(m: &Matrix, lang: Lang) -> String {
    let mut out = String::from("\u{feff}"); // Excel recognises UTF-8
    let head: Vec<String> = [lang.pick("Article", "Article").to_string(), lang.pick("Clé", "Key").to_string()]
        .into_iter()
        .chain(m.columns.iter().map(|(_, t)| t.clone()))
        .map(|h| esc_csv(&h))
        .collect();
    out.push_str(&head.join(","));
    out.push_str("\r\n");
    for r in &m.rows {
        let line: Vec<String> = [r.author_year.clone(), r.citekey.clone()].into_iter().chain(r.cells.iter().cloned()).map(|c| esc_csv(&c)).collect();
        out.push_str(&line.join(","));
        out.push_str("\r\n");
    }
    out
}

pub fn to_markdown(m: &Matrix, lang: Lang) -> String {
    let p = |s: &str| s.replace('|', "\\|");
    let mut out = format!("| {} |", lang.pick("Article", "Article"));
    for (_, t) in &m.columns {
        out.push_str(&format!(" {} |", p(t)));
    }
    out.push_str(&format!("\n|---|{}\n", "---|".repeat(m.columns.len())));
    for r in &m.rows {
        let who = if r.citekey.is_empty() { p(&r.author_year) } else { format!("{} [@{}]", p(&r.author_year), r.citekey) };
        out.push_str(&format!("| {who} |"));
        for c in &r.cells {
            out.push_str(&format!(" {} |", p(c)));
        }
        out.push('\n');
    }
    out
}

pub fn to_latex(m: &Matrix, lang: Lang) -> String {
    let e = crate::bib::escape;
    let n = m.columns.len() + 1;
    let width = format!("{:.3}", 0.95 / n as f64);
    let spec = std::iter::repeat_n(format!("p{{{width}\\textwidth}}"), n).collect::<Vec<_>>().join("|");
    let mut out = format!(
        "% {}\n% \\usepackage{{longtable}}\n\\begin{{longtable}}{{|{spec}|}}\n\\hline\n",
        lang.pick("Matrice de recherche générée par Eloria", "Research matrix generated by Eloria")
    );
    let head: Vec<String> = std::iter::once(format!("\\textbf{{{}}}", lang.pick("Article", "Article"))).chain(m.columns.iter().map(|(_, t)| format!("\\textbf{{{}}}", e(t)))).collect();
    out.push_str(&format!("{} \\\\\n\\hline\n\\endhead\n", head.join(" & ")));
    for r in &m.rows {
        let who = if r.citekey.is_empty() { e(&r.author_year) } else { format!("{} \\cite{{{}}}", e(&r.author_year), r.citekey) };
        let cells: Vec<String> = std::iter::once(who).chain(r.cells.iter().map(|c| e(c))).collect();
        out.push_str(&format!("{} \\\\\n\\hline\n", cells.join(" & ")));
    }
    out.push_str("\\end{longtable}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FICHE: &str = "# Fiche : GNN\n\n<!-- eloria:ia:debut -->\n## Problématique\n\nDétecter les **réseaux** de fraude.\n\n## Méthodes\n\n- GNN sur graphe\n- 2,3 M de transactions\n\n```mermaid\nflowchart LR\n```\n\n## Contraintes rencontrées par les auteurs\n\nDonnées hors UEMOA, \"privées\".\n\n## Commentaire\n\nBase de comparaison.\n<!-- eloria:ia:fin -->\n";

    #[test]
    fn cells_from_a_reading_note() {
        let cells = row_cells(FICHE, 200);
        assert_eq!(cells[0], "Détecter les réseaux de fraude.");
        assert_eq!(cells[2], "GNN sur graphe ; 2,3 M de transactions");
        assert_eq!(cells[5], "Données hors UEMOA, \"privées\".");
        assert_eq!(cells[8], "Base de comparaison.");
        assert_eq!(cells[1], "");
        // Old reading-note template: neighbouring headings are recognised.
        let old = row_cells("## Méthode et données\nGNN.\n## Résultats clés\nGain net.\n## Apport pour ma thèse\nOui.", 50);
        assert_eq!((cells.len(), old[2].as_str(), old[4].as_str(), old[8].as_str()), (9, "GNN.", "Gain net.", "Oui."));
        assert_eq!(cell_text("un texte assez long pour être coupé", 10), "un texte a…");
    }

    #[test]
    fn exports() {
        let m = Matrix {
            columns: columns(Lang::Fr)[..2].to_vec(),
            rows: vec![MatrixRow {
                path: "F.md".into(),
                citekey: "smith2024".into(),
                title: "GNN".into(),
                author_year: "Smith (2024)".into(),
                year: Some(2024),
                statut: "lue".into(),
                cells: vec!["Fraude, réseaux".into(), "A | B & 50%".into()],
            }],
        };
        let csv = to_csv(&m, Lang::Fr);
        assert!(csv.contains("Article,Clé,Problématique,Objectifs\r\n") && csv.contains("Smith (2024),smith2024,\"Fraude, réseaux\",A | B & 50%"));
        let md = to_markdown(&m, Lang::Fr);
        assert!(md.contains("| Smith (2024) [@smith2024] | Fraude, réseaux | A \\| B & 50% |"));
        let tex = to_latex(&m, Lang::Fr);
        assert!(tex.contains("Smith (2024) \\cite{smith2024} & Fraude, réseaux & A | B \\& 50\\% \\\\") && tex.contains("\\begin{longtable}"));
    }
}
