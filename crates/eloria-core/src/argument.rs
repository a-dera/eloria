//! Argument builder. An argument is a readable Markdown note
//! (`type: argument`) :
//!
//! ```text
//! ## Affirmation
//! ## Argument : <title>      (text, then lines « - Preuve : … [@key, p. 4] »)
//! ## Objection : <title>     (text, then « > Réponse : … »)
//! ## Conclusion
//! ```
//!
//! The interface map reads and rewrites this structure; conversion produces continuous
//! text (Markdown) or LaTeX, citations included. No reference is ever added.

use serde::{Deserialize, Serialize};

use crate::lang::Lang;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Point {
    pub title: String,
    pub text: String,
    /// Evidence (arguments): one line each, with its citations
    pub evidence: Vec<String>,
    /// Answer (objections)
    pub rebuttal: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Argument {
    pub title: String,
    pub claim: String,
    pub arguments: Vec<Point>,
    pub objections: Vec<Point>,
    pub conclusion: String,
}

fn strip_label<'a>(s: &'a str, labels: &[&str]) -> Option<&'a str> {
    let t = s.trim();
    labels.iter().find_map(|l| {
        let low = t.to_lowercase();
        if low.starts_with(&l.to_lowercase()) {
            let rest = t[l.len()..].trim_start();
            rest.strip_prefix(':').map(|r| r.trim()).or((rest.is_empty()).then_some(""))
        } else {
            None
        }
    })
}

/// Reads an argument note (front matter ignored).
pub fn parse(content: &str) -> Argument {
    let (_, body, _) = crate::markdown::split_frontmatter(content);
    let mut a = Argument::default();
    #[derive(PartialEq)]
    enum Sec {
        None,
        Claim,
        Arg,
        Obj,
        Concl,
    }
    let mut sec = Sec::None;
    let push_text = |dst: &mut String, line: &str| {
        if !dst.is_empty() {
            dst.push('\n');
        }
        dst.push_str(line);
    };
    let mut in_rebuttal = false;
    for line in body.lines() {
        let t = line.trim();
        if let Some(h) = t.strip_prefix("# ") {
            if a.title.is_empty() {
                a.title = h.trim().to_string();
            }
            continue;
        }
        if let Some(h) = t.strip_prefix("## ") {
            in_rebuttal = false;
            if strip_label(h, &["Affirmation", "Claim", "Thèse défendue"]).is_some() {
                sec = Sec::Claim;
            } else if strip_label(h, &["Conclusion"]).is_some() {
                sec = Sec::Concl;
            } else if let Some(title) = strip_label(h, &["Argument"]) {
                a.arguments.push(Point { title: title.to_string(), ..Default::default() });
                sec = Sec::Arg;
            } else if let Some(title) = strip_label(h, &["Objection"]) {
                a.objections.push(Point { title: title.to_string(), ..Default::default() });
                sec = Sec::Obj;
            } else {
                sec = Sec::None;
            }
            continue;
        }
        match sec {
            Sec::Claim if !t.is_empty() => push_text(&mut a.claim, t),
            Sec::Concl if !t.is_empty() => push_text(&mut a.conclusion, t),
            Sec::Arg => {
                let p = a.arguments.last_mut().unwrap();
                if let Some(e) = t.strip_prefix("- ").and_then(|x| strip_label(x, &["Preuve", "Evidence"])) {
                    p.evidence.push(e.to_string());
                } else if !t.is_empty() {
                    push_text(&mut p.text, t);
                }
            }
            Sec::Obj => {
                let p = a.objections.last_mut().unwrap();
                if let Some(r) = t.strip_prefix('>').map(str::trim) {
                    if let Some(first) = strip_label(r, &["Réponse", "Response"]) {
                        in_rebuttal = true;
                        push_text(&mut p.rebuttal, first);
                    } else if in_rebuttal && !r.is_empty() {
                        push_text(&mut p.rebuttal, r);
                    }
                } else if !t.is_empty() {
                    in_rebuttal = false;
                    push_text(&mut p.text, t);
                }
            }
            _ => {}
        }
    }
    a
}

/// Rewrites the note (body only) from the structure.
pub fn render(a: &Argument, lang: Lang) -> String {
    let fr = lang == Lang::Fr;
    let mut md = format!("# {}\n\n## {}\n\n{}\n\n", a.title.trim(), if fr { "Affirmation" } else { "Claim" }, a.claim.trim());
    for p in &a.arguments {
        md.push_str(&format!("## Argument : {}\n\n", p.title.trim()));
        if !p.text.trim().is_empty() {
            md.push_str(&format!("{}\n\n", p.text.trim()));
        }
        for e in p.evidence.iter().filter(|e| !e.trim().is_empty()) {
            md.push_str(&format!("- {} : {}\n", if fr { "Preuve" } else { "Evidence" }, e.trim()));
        }
        if p.evidence.iter().any(|e| !e.trim().is_empty()) {
            md.push('\n');
        }
    }
    for p in &a.objections {
        md.push_str(&format!("## Objection : {}\n\n", p.title.trim()));
        if !p.text.trim().is_empty() {
            md.push_str(&format!("{}\n\n", p.text.trim()));
        }
        if !p.rebuttal.trim().is_empty() {
            let mut lines = p.rebuttal.trim().lines();
            md.push_str(&format!("> {} : {}\n", if fr { "Réponse" } else { "Response" }, lines.next().unwrap_or_default()));
            for l in lines {
                md.push_str(&format!("> {l}\n"));
            }
            md.push('\n');
        }
    }
    md.push_str(&format!("## Conclusion\n\n{}\n", a.conclusion.trim()));
    md
}

fn lower_first(s: &str) -> String {
    let s = s.trim().trim_end_matches(['.', ':']);
    let mut c = s.chars();
    match c.next() {
        // Acronyms ("GNN", "UEMOA") left as they are.
        Some(f) if !s.chars().take(2).all(|x| x.is_uppercase()) => f.to_lowercase().collect::<String>() + c.as_str(),
        Some(f) => f.to_string() + c.as_str(),
        None => String::new(),
    }
}

/// Final punctuation (the full stop goes after a final citation).
fn end(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() || s.ends_with(['.', '?', '!']) {
        s.to_string()
    } else {
        format!("{s}.")
    }
}

/// Sentence: initial capital and final punctuation.
fn sentence(s: &str) -> String {
    let s = end(s);
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Paragraphs of the continuous text (Markdown, `[@key]` citations kept).
pub fn prose(a: &Argument, lang: Lang) -> Vec<String> {
    let fr = lang == Lang::Fr;
    let order: &[&str] = if fr { &["Premièrement", "Deuxièmement", "Troisièmement", "Ensuite", "De plus", "Enfin"] } else { &["First", "Second", "Third", "Next", "Moreover", "Finally"] };
    let mut out = Vec::new();
    if !a.claim.trim().is_empty() {
        out.push(sentence(&a.claim.split_whitespace().collect::<Vec<_>>().join(" ")));
    }
    let n = a.arguments.len();
    for (i, p) in a.arguments.iter().enumerate() {
        let conn = if n > 1 && i == n - 1 { order[order.len() - 1] } else { order[i.min(order.len() - 2)] };
        let mut para = format!("{conn}, {}", end(&lower_first(&p.title)));
        for part in std::iter::once(p.text.as_str()).chain(p.evidence.iter().map(String::as_str)) {
            let part = part.split_whitespace().collect::<Vec<_>>().join(" ");
            if !part.is_empty() {
                para.push(' ');
                para.push_str(&sentence(&part));
            }
        }
        out.push(para);
    }
    for p in &a.objections {
        let mut para = format!("{} {}", if fr { "On pourrait objecter que" } else { "One could object that" }, end(&lower_first(&p.title)));
        for part in [p.text.as_str()] {
            let part = part.split_whitespace().collect::<Vec<_>>().join(" ");
            if !part.is_empty() {
                para.push(' ');
                para.push_str(&sentence(&part));
            }
        }
        let r = p.rebuttal.split_whitespace().collect::<Vec<_>>().join(" ");
        if !r.is_empty() {
            para.push_str(&format!(" {} {}", if fr { "Cependant," } else { "However," }, end(&lower_first(&r))));
        }
        out.push(para);
    }
    if !a.conclusion.trim().is_empty() {
        out.push(format!("{} {}", if fr { "En conclusion," } else { "In conclusion," }, end(&lower_first(&a.conclusion.split_whitespace().collect::<Vec<_>>().join(" ")))));
    }
    out
}

/// Continuous text in Markdown.
pub fn to_markdown(a: &Argument, lang: Lang) -> String {
    format!("# {}\n\n{}\n", a.title.trim(), prose(a, lang).join("\n\n"))
}

/// Continuous text in LaTeX (citations as `\autocite`).
pub fn to_latex(a: &Argument, lang: Lang) -> String {
    let paras: Vec<String> = prose(a, lang).iter().map(|p| crate::manuscript::inline(p)).collect();
    format!("\\section{{{}}}\n\n{}\n", crate::manuscript::inline(a.title.trim()), paras.join("\n\n"))
}

/// Keys cited in the argument.
pub fn citekeys(a: &Argument) -> Vec<String> {
    let all = std::iter::once(a.claim.as_str())
        .chain(a.arguments.iter().flat_map(|p| std::iter::once(p.text.as_str()).chain(p.evidence.iter().map(String::as_str))))
        .chain(a.objections.iter().flat_map(|p| [p.text.as_str(), p.rebuttal.as_str()]))
        .chain(std::iter::once(a.conclusion.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = crate::markdown::parse_note(&all).citations;
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "---\ntype: argument\n---\n# Les GNN détectent la fraude organisée\n\n## Affirmation\n\nLes réseaux de neurones sur graphes repèrent les réseaux de fraude du mobile money.\n\n## Argument : La fraude organisée forme des sous-graphes denses\n\nLes comptes mules échangent vite et beaucoup.\n\n- Preuve : sous-graphes denses observés sur 2,3 M de transactions [@smith2024graph, p. 4]\n- Preuve : même constat en Inde [@kumar2022]\n\n## Argument : GNN apprend la structure\n\n## Objection : Les données sont rares en UEMOA\n\nPeu d'opérateurs partagent leurs données.\n\n> Réponse : l'apprentissage fédéré permet d'apprendre\n> sans partager les données [@abbas2026].\n\n## Conclusion\n\nLes GNN sont pertinents pour la thèse.\n";

    #[test]
    fn read_and_write() {
        let a = parse(NOTE);
        assert_eq!(a.title, "Les GNN détectent la fraude organisée");
        assert!(a.claim.starts_with("Les réseaux de neurones"));
        assert_eq!(a.arguments.len(), 2);
        assert_eq!(a.arguments[0].evidence.len(), 2);
        assert_eq!(a.arguments[0].text, "Les comptes mules échangent vite et beaucoup.");
        assert_eq!(a.objections[0].rebuttal, "l'apprentissage fédéré permet d'apprendre\nsans partager les données [@abbas2026].");
        assert_eq!(a.conclusion, "Les GNN sont pertinents pour la thèse.");
        // Stable round trip.
        let again = parse(&render(&a, Lang::Fr));
        assert_eq!(again, a);
        assert_eq!(citekeys(&a), vec!["smith2024graph", "kumar2022", "abbas2026"]);
    }

    #[test]
    fn continuous_text_and_latex() {
        let a = parse(NOTE);
        let p = prose(&a, Lang::Fr);
        assert_eq!(p.len(), 5);
        assert_eq!(p[1], "Premièrement, la fraude organisée forme des sous-graphes denses. Les comptes mules échangent vite et beaucoup. Sous-graphes denses observés sur 2,3 M de transactions [@smith2024graph, p. 4]. Même constat en Inde [@kumar2022].");
        assert!(p[2].starts_with("Enfin, GNN apprend la structure."));
        assert!(p[3].starts_with("On pourrait objecter que les données sont rares en UEMOA. Peu d'opérateurs") && p[3].contains("Cependant, l'apprentissage fédéré"));
        assert_eq!(p[4], "En conclusion, les GNN sont pertinents pour la thèse.");
        let tex = to_latex(&a, Lang::Fr);
        assert!(tex.starts_with("\\section{Les GNN détectent la fraude organisée}"));
        assert!(tex.contains("\\autocite[p. 4]{smith2024graph}") && tex.contains("\\autocite{abbas2026}"), "{tex}");
        assert!(!to_markdown(&a, Lang::Fr).contains('—'));
    }
}
