//! Outline generator: from the notes of a scope (chapter, tag, folder), a
//! draft outline in Markdown. Sections = most shared tags (greedy choice),
//! then, in each section, concepts first, then reading notes and sources (with their key
//! sentence and citation). No AI: nothing is made up, everything points to a note.

use std::collections::{BTreeMap, HashSet};

use crate::lang::Lang;

/// A note in the scope.
#[derive(Debug, Clone, Default)]
pub struct OutlineDoc {
    pub stem: String,
    pub title: String,
    /// concept, fiche, source, question, note…
    pub kind: String,
    pub tags: Vec<String>,
    pub citekey: String,
    pub year: Option<i32>,
    /// Key sentence (abstract, results or first paragraph)
    pub summary: String,
}

const IGNORED: &[&str] = &["source", "fiche", "concept", "question", "methode", "méthode", "chapitre", "plan", "capture", "journal"];

/// "thèse/mobile-money" -> "Mobile money".
pub fn tag_label(tag: &str) -> String {
    let leaf = tag.rsplit('/').next().unwrap_or(tag).replace(['-', '_'], " ");
    let mut c = leaf.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Groups (section title, note indices): at each round, the tag covering the most
/// notes not yet placed (at least two); the rest goes to "Other".
pub fn group(docs: &[OutlineDoc], exclude_tags: &[String]) -> Vec<(String, Vec<usize>)> {
    let skip: HashSet<String> = exclude_tags.iter().map(|t| t.to_lowercase()).chain(IGNORED.iter().map(|t| t.to_string())).collect();
    let mut left: Vec<usize> = (0..docs.len()).collect();
    let mut out = Vec::new();
    loop {
        let mut count: BTreeMap<String, usize> = BTreeMap::new();
        for &i in &left {
            for t in &docs[i].tags {
                let t = t.to_lowercase();
                if !skip.contains(&t) && !skip.iter().any(|s| t.starts_with(&format!("{s}/"))) {
                    *count.entry(t).or_default() += 1;
                }
            }
        }
        let Some((best, n)) = count.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))) else { break };
        if n < 2 {
            break;
        }
        let (inside, rest): (Vec<usize>, Vec<usize>) = left.iter().partition(|&&i| docs[i].tags.iter().any(|t| t.to_lowercase() == best));
        out.push((tag_label(&best), inside));
        left = rest;
    }
    if !left.is_empty() {
        out.push((String::new(), left));
    }
    out
}

/// Outline in Markdown (without front matter).
pub fn render(title: &str, docs: &[OutlineDoc], exclude_tags: &[String], lang: Lang) -> String {
    let mut md = format!("# {title}\n\n");
    md.push_str(lang.pick(
        "> [!note] Brouillon de plan généré à partir de tes notes : réordonne, fusionne, renomme. Chaque point renvoie à sa note.\n\n",
        "> [!note] Draft outline generated from your notes: reorder, merge, rename. Each point links to its note.\n\n",
    ));
    let groups = group(docs, exclude_tags);
    for (n, (label, idx)) in groups.iter().enumerate() {
        let label = if label.is_empty() { lang.pick("Autres", "Other").to_string() } else { label.clone() };
        md.push_str(&format!("## {}. {label}\n\n", n + 1));
        let mut items: Vec<&OutlineDoc> = idx.iter().map(|&i| &docs[i]).collect();
        let rank = |d: &OutlineDoc| match d.kind.as_str() {
            "question" => 0,
            "concept" => 1,
            "fiche" | "source" => 2,
            _ => 3,
        };
        items.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.year.cmp(&b.year)).then(a.title.to_lowercase().cmp(&b.title.to_lowercase())));
        // A reading note and its source: the reading note is enough.
        let fiche_keys: HashSet<String> = items.iter().filter(|d| d.kind == "fiche" && !d.citekey.is_empty()).map(|d| d.citekey.to_lowercase()).collect();
        for d in items {
            if d.kind == "source" && fiche_keys.contains(&d.citekey.to_lowercase()) {
                continue;
            }
            let mut line = format!("- [[{}]]", d.stem);
            if !d.summary.is_empty() {
                line.push_str(&format!(" : {}", d.summary));
            }
            if !d.citekey.is_empty() {
                line.push_str(&format!(" [@{}]", d.citekey));
            }
            md.push_str(&line);
            md.push('\n');
        }
        md.push('\n');
    }
    if docs.is_empty() {
        md.push_str(lang.pick("Aucune note dans ce périmètre.\n", "No note in this scope.\n"));
    }
    md
}

/// First sentence of a text (at most `max` characters, on whole words).
pub fn first_sentence(text: &str, max: usize) -> String {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let t = t.trim_start_matches(['#', '>', '-', '*', ' ']);
    let end = t.char_indices().find(|(i, c)| matches!(c, '.' | '?' | '!') && t[*i + 1..].starts_with(' ')).map(|(i, _)| i + 1).unwrap_or(t.len());
    let s = &t[..end];
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for w in s.split(' ') {
        if out.chars().count() + w.chars().count() + 1 > max {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    format!("{out}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(stem: &str, kind: &str, tags: &[&str], ck: &str, year: Option<i32>, summary: &str) -> OutlineDoc {
        OutlineDoc { stem: stem.into(), title: stem.into(), kind: kind.into(), tags: tags.iter().map(|t| t.to_string()).collect(), citekey: ck.into(), year, summary: summary.into() }
    }

    #[test]
    fn outline_by_shared_tags() {
        let docs = vec![
            d("Fiche @a", "fiche", &["fiche", "gnn", "fraude"], "a", Some(2024), "Les GNN repèrent les réseaux."),
            d("@a", "source", &["source", "gnn"], "a", Some(2024), ""),
            d("Fiche @b", "fiche", &["fiche", "gnn"], "b", Some(2020), ""),
            d("Graphes", "concept", &["concept", "gnn"], "", None, "Un graphe relie des comptes."),
            d("Fiche @c", "fiche", &["fiche", "xai", "thèse/ch2"], "c", Some(2017), ""),
            d("SHAP", "concept", &["xai"], "", None, ""),
            d("Divers", "note", &[], "", None, ""),
        ];
        let g = group(&docs, &["thèse/ch2".into()]);
        assert_eq!(g.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(), vec!["Gnn", "Xai", ""]);
        let md = render("Plan du chapitre 2", &docs, &["thèse/ch2".into()], Lang::Fr);
        assert!(md.contains("## 1. Gnn\n\n- [[Graphes]] : Un graphe relie des comptes.\n- [[Fiche @b]] [@b]\n- [[Fiche @a]] : Les GNN repèrent les réseaux. [@a]\n"), "{md}");
        // The source "@a" is covered by its reading note.
        assert!(!md.contains("[[@a]]"));
        assert!(md.contains("## 3. Autres\n\n- [[Divers]]"));
        assert!(!md.contains('—'));
    }

    #[test]
    fn key_sentence() {
        assert_eq!(first_sentence("Les mules blanchissent. Puis autre chose.", 80), "Les mules blanchissent.");
        assert_eq!(first_sentence("un deux trois quatre cinq six", 14), "un deux trois…");
        assert_eq!(tag_label("thèse/mobile-money"), "Mobile money");
    }
}
