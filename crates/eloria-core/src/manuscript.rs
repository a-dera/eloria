//! From research to manuscript: Markdown → LaTeX conversion (`[@key]` citations to
//! `\autocite`, links, callouts, tables, formulas), assembly of the thesis into a LaTeX
//! project, and a pre-submission checklist.

use std::collections::HashSet;

use regex::Regex;
use serde::Serialize;

use crate::lang::Lang;

/// Escapes LaTeX special characters outside formulas.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' | '%' | '#' | '_' => {
                out.push('\\');
                out.push(c);
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '\\' => out.push_str("\\textbackslash{}"),
            _ => out.push(c),
        }
    }
    out
}

/// Inline text: formulas and code preserved, citations, links, bold, italics.
pub fn inline(s: &str) -> String {
    // Protected pieces: $…$, `…`, citations, links, handled separately, the rest escaped.
    let re = Regex::new(r"\$[^$\n]+\$|`[^`\n]+`|\[[^\[\]]*@[^\[\]]*\]|!?\[\[[^\]]+\]\]|\[[^\]]+\]\([^)\s]+\)|\*\*[^*]+\*\*|==[^=]+==|(?:^|[^*\w])\*[^*\n]+\*").unwrap();
    let cite = Regex::new(r"@([A-Za-z0-9_:\-.]*[A-Za-z0-9_])").unwrap();
    let mut out = String::new();
    let mut last = 0;
    for m in re.find_iter(s) {
        out.push_str(&esc(&s[last..m.start()]));
        let t = m.as_str();
        if t.starts_with('$') {
            out.push_str(t);
        } else if t.starts_with('`') {
            out.push_str(&format!("\\texttt{{{}}}", esc(&t[1..t.len() - 1])));
        } else if t.starts_with('[') && t.contains('@') && !t.starts_with("[[") {
            let inner = &t[1..t.len() - 1];
            let keys: Vec<String> = cite.captures_iter(inner).map(|c| c[1].to_string()).collect();
            // Locator ("p. 3"): after the last key.
            let loc = cite.split(inner).last().unwrap_or_default().trim_start_matches([',', ';', ' ']).trim().to_string();
            if loc.is_empty() {
                out.push_str(&format!("\\autocite{{{}}}", keys.join(",")));
            } else {
                out.push_str(&format!("\\autocite[{}]{{{}}}", esc(&loc), keys.join(",")));
            }
        } else if t.starts_with("[[") || t.starts_with("![[") {
            let inner = t.trim_start_matches('!').trim_start_matches("[[").trim_end_matches("]]");
            let label = inner.split('|').nth(1).unwrap_or_else(|| inner.split('#').next().unwrap_or(inner));
            out.push_str(&esc(label));
        } else if t.starts_with('[') {
            let (txt, url) = t[1..t.len() - 1].split_once("](").unwrap_or((t, ""));
            out.push_str(&format!("\\href{{{}}}{{{}}}", url.replace('%', "\\%").replace('#', "\\#"), inline(txt)));
        } else if t.starts_with("**") {
            out.push_str(&format!("\\textbf{{{}}}", inline(&t[2..t.len() - 2])));
        } else if t.starts_with("==") {
            out.push_str(&format!("\\emph{{{}}}", inline(&t[2..t.len() - 2])));
        } else {
            // *italics* (with the preceding character, kept)
            let star = t.find('*').unwrap_or(0);
            out.push_str(&esc(&t[..star]));
            out.push_str(&format!("\\emph{{{}}}", inline(&t[star + 1..t.len() - 1])));
        }
        last = m.end();
    }
    out.push_str(&esc(&s[last..]));
    out
}

/// Converts a Markdown note to LaTeX. `top`: LaTeX level of the `#` heading (0 = \chapter,
/// 1 = \section). Properties and generated blocks (Zotero, AI) are removed.
pub fn md_to_latex(md: &str, top: usize) -> String {
    const LEVELS: [&str; 5] = ["chapter", "section", "subsection", "subsubsection", "paragraph"];
    let (_, body, _) = crate::markdown::split_frontmatter(md);
    let mut out: Vec<String> = Vec::new();
    let mut list: Option<&str> = None;
    let mut in_code = false;
    let mut in_math = false;
    let mut in_quote = false;
    let mut table: Vec<Vec<String>> = Vec::new();
    let mut skip = false;
    let close_list = |out: &mut Vec<String>, list: &mut Option<&str>| {
        if let Some(l) = list.take() {
            out.push(format!("\\end{{{l}}}"));
        }
    };
    let flush_table = |out: &mut Vec<String>, table: &mut Vec<Vec<String>>| {
        if table.is_empty() {
            return;
        }
        let cols = table.iter().map(|r| r.len()).max().unwrap_or(1);
        out.push("\\begin{table}[htbp]\n\\centering".into());
        out.push(format!("\\begin{{tabular}}{{{}}}\n\\toprule", "l".repeat(cols)));
        for (i, r) in table.iter().enumerate() {
            out.push(format!("{} \\\\", r.iter().map(|c| inline(c)).collect::<Vec<_>>().join(" & ")));
            if i == 0 {
                out.push("\\midrule".into());
            }
        }
        out.push("\\bottomrule\n\\end{tabular}\n\\end{table}".into());
        table.clear();
    };
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("<!-- zotero:debut") || t.starts_with("<!-- eloria:annotations:debut") {
            skip = true;
            continue;
        }
        if t.starts_with("<!-- zotero:fin") || t.starts_with("<!-- eloria:annotations:fin") {
            skip = false;
            continue;
        }
        if skip || t.starts_with("<!--") {
            continue;
        }
        if t.starts_with("```") {
            if in_code {
                out.push("\\end{verbatim}".into());
            } else {
                close_list(&mut out, &mut list);
                out.push("\\begin{verbatim}".into());
            }
            in_code = !in_code;
            continue;
        }
        if in_code {
            out.push(line.to_string());
            continue;
        }
        if t == "$$" || (t.starts_with("$$") && t.ends_with("$$") && t.len() > 4) {
            if t.len() > 4 {
                out.push(format!("\\[\n{}\n\\]", &t[2..t.len() - 2]));
            } else {
                out.push(if in_math { "\\]".into() } else { "\\[".into() });
                in_math = !in_math;
            }
            continue;
        }
        if in_math {
            out.push(line.to_string());
            continue;
        }
        if t.starts_with('|') {
            if Regex::new(r"^\|?\s*:?-{2,}").unwrap().is_match(t) {
                continue;
            }
            table.push(t.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect());
            continue;
        }
        flush_table(&mut out, &mut table);
        if t.starts_with('>') {
            let q = t.trim_start_matches('>').trim();
            if !in_quote {
                close_list(&mut out, &mut list);
                out.push("\\begin{quote}".into());
                in_quote = true;
            }
            // [!type] title callout: the title in bold.
            if let Some(c) = Regex::new(r"^\[!([^\]|]+)(?:\|[^\]]*)?\][-+]?\s*(.*)$").unwrap().captures(q) {
                let title = if c[2].trim().is_empty() { c[1].to_string() } else { c[2].to_string() };
                out.push(format!("\\textbf{{{}}}\\\\", inline(&title)));
            } else if !q.is_empty() {
                out.push(inline(q));
            }
            continue;
        } else if in_quote {
            out.push("\\end{quote}".into());
            in_quote = false;
        }
        let hashes = t.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
            close_list(&mut out, &mut list);
            let lvl = (top + hashes - 1).min(LEVELS.len() - 1);
            let title = t[hashes..].trim();
            let label: String = crate::assist::fold(title).chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect::<String>().split('-').filter(|x| !x.is_empty()).collect::<Vec<_>>().join("-");
            out.push(format!("\\{}{{{}}}\\label{{sec:{}}}", LEVELS[lvl], inline(title), label));
            continue;
        }
        if let Some(img) = Regex::new(r"^!\[([^\]]*)\]\(([^)\s]+)\)$").unwrap().captures(t) {
            close_list(&mut out, &mut list);
            out.push(format!("\\begin{{figure}}[htbp]\n\\centering\n\\includegraphics[width=0.8\\linewidth]{{{}}}\n\\caption{{{}}}\n\\end{{figure}}", &img[2], inline(&img[1])));
            continue;
        }
        let bullet = Regex::new(r"^[-*+]\s+(?:\[[ xX]\]\s+)?(.*)$").unwrap();
        let num = Regex::new(r"^\d+[.)]\s+(.*)$").unwrap();
        if let Some(c) = bullet.captures(t) {
            if list != Some("itemize") {
                close_list(&mut out, &mut list);
                out.push("\\begin{itemize}".into());
                list = Some("itemize");
            }
            out.push(format!("  \\item {}", inline(&c[1])));
            continue;
        }
        if let Some(c) = num.captures(t) {
            if list != Some("enumerate") {
                close_list(&mut out, &mut list);
                out.push("\\begin{enumerate}".into());
                list = Some("enumerate");
            }
            out.push(format!("  \\item {}", inline(&c[1])));
            continue;
        }
        if t.is_empty() {
            close_list(&mut out, &mut list);
            out.push(String::new());
            continue;
        }
        if t == "---" || t == "***" {
            out.push("\\bigskip".into());
            continue;
        }
        out.push(inline(t));
    }
    flush_table(&mut out, &mut table);
    close_list(&mut out, &mut list);
    if in_quote {
        out.push("\\end{quote}".into());
    }
    let mut s = out.join("\n");
    while s.contains("\n\n\n") {
        s = s.replace("\n\n\n", "\n\n");
    }
    s.trim().to_string() + "\n"
}

/// Manuscript preamble (report class, biblatex, French or English).
pub fn thesis_main(title: &str, author: &str, chapters: &[String], lang: Lang) -> String {
    let babel = lang.pick("french", "english");
    let includes: String = chapters.iter().map(|c| format!("\\include{{{c}}}\n")).collect();
    format!(
        "% Manuscrit assemblé par Eloria à partir des chapitres du coffre.\n% Les fichiers chapitres sont régénérés à chaque assemblage : modifie les notes, pas ces fichiers.\n\\documentclass[12pt,a4paper]{{report}}\n\\usepackage[T1]{{fontenc}}\n\\usepackage[{babel}]{{babel}}\n\\usepackage{{csquotes}}\n\\usepackage{{amsmath,amssymb}}\n\\usepackage{{graphicx}}\n\\usepackage{{booktabs}}\n\\usepackage[hidelinks]{{hyperref}}\n\\usepackage[backend=biber,style=authoryear]{{biblatex}}\n\\addbibresource{{references.bib}}\n\n\\title{{{}}}\n\\author{{{}}}\n\\date{{\\today}}\n\n\\begin{{document}}\n\\maketitle\n\\tableofcontents\n\n{includes}\n\\printbibliography\n\\end{{document}}\n",
        esc(title),
        esc(author)
    )
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CheckItem {
    pub label: String,
    pub ok: bool,
    pub detail: String,
}

/// Pre-submission checklist for a LaTeX (or Markdown) document.
pub fn submission_checklist(text: &str, is_tex: bool, library: &HashSet<String>, lang: Lang) -> Vec<CheckItem> {
    let mut v = Vec::new();
    let cites = crate::quality::validate_citations(text, library);
    let unknown: Vec<String> = cites.iter().filter(|c| !c.known).map(|c| format!("@{}", c.key)).collect();
    v.push(CheckItem { label: lang.pick("Citations présentes dans la bibliothèque", "Citations present in the library").into(), ok: unknown.is_empty(), detail: if unknown.is_empty() { format!("{}", cites.len()) } else { unknown.join(", ") } });
    let todo = Regex::new(r"(?i)\b(TODO|FIXME|XXX|À FAIRE|A COMPLETER|À COMPLÉTER)\b").unwrap();
    let todos: Vec<String> = text.lines().enumerate().filter(|(_, l)| todo.is_match(l)).map(|(i, _)| format!("l. {}", i + 1)).collect();
    v.push(CheckItem { label: lang.pick("Aucune marque « TODO » restante", "No leftover \"TODO\" marks").into(), ok: todos.is_empty(), detail: todos.join(", ") });
    if is_tex {
        let labels: HashSet<String> = crate::latex::labels(text).into_iter().collect();
        let refs: Vec<String> = Regex::new(r"\\(?:eq|auto|page|c|C)?ref\{([^}]+)\}").unwrap().captures_iter(text).map(|c| c[1].to_string()).filter(|r| !labels.contains(r)).collect();
        v.push(CheckItem { label: lang.pick("Renvois \\ref vers des étiquettes existantes", "\\ref cross-references to existing labels").into(), ok: refs.is_empty(), detail: refs.join(", ") });
        let figs: Vec<&str> = Regex::new(r"(?s)\\begin\{(figure|table)\}(.*?)\\end\{(?:figure|table)\}").unwrap().captures_iter(text).filter(|c| !c[2].contains("\\caption")).map(|_| "").collect();
        v.push(CheckItem { label: lang.pick("Chaque figure et tableau a une légende", "Every figure and table has a caption").into(), ok: figs.is_empty(), detail: if figs.is_empty() { String::new() } else { format!("{}", figs.len()) } });
        let bib = text.contains("\\printbibliography") || text.contains("\\bibliography{");
        v.push(CheckItem { label: lang.pick("Bibliographie imprimée", "Bibliography printed").into(), ok: bib, detail: String::new() });
        let empty: Vec<String> = Regex::new(r"\\(?:chapter|section|subsection)\{([^}]*)\}[^\n]*\n\s*\\(?:chapter|section|subsection)\{").unwrap().captures_iter(text).map(|c| c[1].to_string()).collect();
        v.push(CheckItem { label: lang.pick("Pas de section vide", "No empty section").into(), ok: empty.is_empty(), detail: empty.join(", ") });
    } else {
        let empty: Vec<String> = crate::matrix::sections(text).into_iter().filter(|(_, c)| c.trim().is_empty()).map(|(k, _)| k).collect();
        v.push(CheckItem { label: lang.pick("Pas de section vide", "No empty section").into(), ok: empty.is_empty(), detail: empty.join(", ") });
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_conversion() {
        assert_eq!(inline("Voir [@smith2024graph, p. 4] et [@a; @b]."), "Voir \\autocite[p. 4]{smith2024graph} et \\autocite{a,b}.");
        assert_eq!(inline("**Gras** et *italique*, 50 % & co, $x_i^2$, `a_b`"), "\\textbf{Gras} et \\emph{italique}, 50 \\% \\& co, $x_i^2$, \\texttt{a\\_b}");
        assert_eq!(inline("Lien [[SHAP|les valeurs]] et [[Note#Titre]] et [site](https://x.org/a#b)"), "Lien les valeurs et Note et \\href{https://x.org/a\\#b}{site}");
    }

    #[test]
    fn note_conversion() {
        let md = "---\ntitre: x\n---\n# Chapitre\n\nTexte.\n\n## Méthode\n\n- un\n- deux\n\n1. a\n2. b\n\n> [!astuce] Retenir\n> Les mules.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n$$\nx=1\n$$\n\n```\ncode_brut\n```\n\n<!-- zotero:debut -->\n## Annotations\n<!-- zotero:fin -->\n![Schéma](img/a.png)\n";
        let tex = md_to_latex(md, 0);
        assert!(tex.starts_with("\\chapter{Chapitre}\\label{sec:chapitre}"));
        assert!(tex.contains("\\section{Méthode}\\label{sec:methode}"));
        assert!(tex.contains("\\begin{itemize}\n  \\item un\n  \\item deux\n\\end{itemize}"));
        assert!(tex.contains("\\begin{enumerate}"));
        assert!(tex.contains("\\begin{quote}\n\\textbf{Retenir}\\\\\nLes mules.\n\\end{quote}"));
        assert!(tex.contains("1 & 2 \\\\") && tex.contains("\\toprule"));
        assert!(tex.contains("\\[\nx=1\n\\]") && tex.contains("\\begin{verbatim}\ncode_brut\n\\end{verbatim}"));
        assert!(!tex.contains("Annotations") && tex.contains("\\includegraphics[width=0.8\\linewidth]{img/a.png}"));
        let main = thesis_main("Ma thèse", "Amédée DERA", &["ch1".into(), "ch2".into()], Lang::Fr);
        assert!(main.contains("\\include{ch1}\n\\include{ch2}") && main.contains("[french]{babel}"));
    }

    #[test]
    fn pre_submission_checklist() {
        let lib: HashSet<String> = ["a2020".to_string()].into();
        let tex = "\\section{Intro}\nVoir \\autocite{a2020} et \\cite{b2019}. TODO relire.\n\\ref{fig:x}\n\\begin{figure}\\includegraphics{x}\\end{figure}\n\\section{Vide}\n\\section{Fin}\nTexte.";
        let c = submission_checklist(tex, true, &lib, Lang::Fr);
        let failed: Vec<&str> = c.iter().filter(|x| !x.ok).map(|x| x.label.as_str()).collect();
        assert_eq!(failed.len(), 6, "{failed:?}");
        assert!(c[0].detail.contains("@b2019"));
        let ok = submission_checklist("\\section{A}\nTexte \\autocite{a2020}.\n\\label{s}\\ref{s}\n\\printbibliography", true, &lib, Lang::Fr);
        assert!(ok.iter().all(|x| x.ok), "{ok:?}");
    }
}
