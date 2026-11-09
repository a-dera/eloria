//! Quality checks, entirely local (no AI): bibliography audit,
//! citation validation of a manuscript, academic style, near-duplicates.

use std::collections::{HashMap, HashSet};

use regex::Regex;
use serde::Serialize;

use crate::lang::Lang;
use crate::zotero::Item;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BibIssue {
    pub citekey: String,
    pub title: String,
    /// "duplicate", "missing fields", "key"
    pub kind: String,
    pub detail: String,
}

fn norm_title(t: &str) -> String {
    crate::assist::fold(t).chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Bibliography audit: duplicates (same DOI or same title), missing fields,
/// problematic citation keys (spaces, accented characters, duplicate key).
pub fn bib_audit(items: &[Item], lang: Lang) -> Vec<BibIssue> {
    let mut out = Vec::new();
    let mut by_doi: HashMap<String, Vec<&Item>> = HashMap::new();
    let mut by_title: HashMap<String, Vec<&Item>> = HashMap::new();
    let mut by_key: HashMap<String, Vec<&Item>> = HashMap::new();
    for it in items {
        if !it.doi.trim().is_empty() {
            by_doi.entry(it.doi.trim().to_lowercase()).or_default().push(it);
        }
        let t = norm_title(&it.title);
        if t.len() > 12 {
            by_title.entry(t).or_default().push(it);
        }
        by_key.entry(it.citekey.to_lowercase()).or_default().push(it);
    }
    let mut seen_pairs: HashSet<String> = HashSet::new();
    for group in by_doi.values().chain(by_title.values()) {
        if group.len() < 2 {
            continue;
        }
        let mut keys: Vec<&str> = group.iter().map(|i| i.citekey.as_str()).collect();
        keys.sort();
        if !seen_pairs.insert(keys.join(",")) {
            continue;
        }
        for it in group {
            out.push(BibIssue {
                citekey: it.citekey.clone(),
                title: it.title.clone(),
                kind: lang.pick("doublon", "duplicate").into(),
                detail: format!("{} {}", lang.pick("même document que", "same document as"), keys.iter().filter(|k| **k != it.citekey).map(|k| format!("@{k}")).collect::<Vec<_>>().join(", ")),
            });
        }
    }
    for (k, group) in &by_key {
        if group.len() > 1 {
            out.push(BibIssue { citekey: k.clone(), title: group[0].title.clone(), kind: lang.pick("clé", "key").into(), detail: lang.pick("clé de citation utilisée par plusieurs documents", "citation key used by several documents").into() });
        }
    }
    for it in items {
        if it.citekey.chars().any(|c| c.is_whitespace() || !c.is_ascii()) {
            out.push(BibIssue { citekey: it.citekey.clone(), title: it.title.clone(), kind: lang.pick("clé", "key").into(), detail: lang.pick("espaces ou accents dans la clé : à éviter pour LaTeX", "spaces or accents in the key: avoid for LaTeX").into() });
        }
        let mut missing = Vec::new();
        if it.creators.is_empty() {
            missing.push(lang.pick("auteurs", "authors"));
        }
        if it.year.is_none() {
            missing.push(lang.pick("année", "year"));
        }
        let article = matches!(it.item_type.as_str(), "journalArticle" | "conferencePaper");
        if article && it.publication.trim().is_empty() {
            missing.push(lang.pick("revue ou conférence", "journal or conference"));
        }
        if article && it.doi.trim().is_empty() {
            missing.push("DOI");
        }
        if it.item_type == "journalArticle" && it.fields.get("pages").map(|p| p.trim().is_empty()).unwrap_or(true) {
            missing.push("pages");
        }
        if !missing.is_empty() {
            out.push(BibIssue { citekey: it.citekey.clone(), title: it.title.clone(), kind: lang.pick("champs manquants", "missing fields").into(), detail: missing.join(", ") });
        }
    }
    out.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.citekey.cmp(&b.citekey)));
    out
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CitationCheck {
    pub key: String,
    /// Lines (1 = first) where the key is cited
    pub lines: Vec<usize>,
    pub known: bool,
}

/// Citations of a manuscript (Markdown `[@key]` or LaTeX `\cite{key}`), known to the library or not.
pub fn validate_citations(text: &str, library: &HashSet<String>) -> Vec<CitationCheck> {
    let md = Regex::new(r"@([A-Za-z0-9_][A-Za-z0-9_:\-.]*[A-Za-z0-9_])").unwrap();
    let mut map: Vec<CitationCheck> = Vec::new();
    let mut add = |key: String, line: usize| {
        if let Some(c) = map.iter_mut().find(|c| c.key == key) {
            if !c.lines.contains(&line) {
                c.lines.push(line);
            }
        } else {
            let known = library.contains(&key.to_lowercase());
            map.push(CitationCheck { key, lines: vec![line], known });
        }
    };
    let mut in_code = false;
    for (i, l) in text.lines().enumerate() {
        if l.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        for k in crate::latex::cited_keys(l) {
            add(k, i + 1);
        }
        // Markdown citations only between brackets: an e-mail address is not one.
        for group in Regex::new(r"\[[^\[\]]*@[^\[\]]*\]").unwrap().find_iter(l) {
            for c in md.captures_iter(group.as_str()) {
                add(c[1].to_string(), i + 1);
            }
        }
    }
    map.sort_by(|a, b| a.known.cmp(&b.known).then(a.key.cmp(&b.key)));
    map
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StyleIssue {
    pub line: usize,
    /// long, personal, vague, repetition
    pub kind: String,
    pub excerpt: String,
    pub message: String,
}

const VAGUE_FR: &[&str] = &["beaucoup", "très", "chose", "choses", "énormément", "assez", "plutôt", "vraiment", "plusieurs", "certains", "divers", "etc", "on", "évidemment", "clairement", "bien sûr", "en quelque sorte"];
const VAGUE_EN: &[&str] = &["very", "really", "thing", "things", "a lot", "lots of", "quite", "rather", "several", "various", "etc", "obviously", "clearly", "of course", "kind of", "sort of"];

/// Academic style checker: sentences too long, first person
/// singular, vague terms, words repeated in two consecutive sentences.
pub fn style_check(text: &str, lang: Lang) -> Vec<StyleIssue> {
    let (_, body, _) = crate::markdown::split_frontmatter(text);
    // Line numbers of the whole file: properties come before the body.
    let first_line = text[..text.len() - body.len()].matches('\n').count();
    let mut out = Vec::new();
    let mut in_code = false;
    let first = match lang {
        Lang::Fr => Regex::new(r"(?i)\b(je|j'|j’|moi|mon|ma|mes)\b").unwrap(),
        Lang::En => Regex::new(r"\b(I|me|my|mine)\b").unwrap(),
    };
    let vague = match lang {
        Lang::Fr => VAGUE_FR,
        Lang::En => VAGUE_EN,
    };
    let mut prev_words: HashSet<String> = HashSet::new();
    for (i, raw) in body.lines().enumerate() {
        let line = first_line + i + 1;
        let t = raw.trim();
        if t.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        // Code, headings, quotes, tables, generated blocks: outside the style to check.
        if in_code || t.is_empty() || t.starts_with('#') || t.starts_with('>') || t.starts_with('|') || t.starts_with("<!--") || t.starts_with("$$") {
            prev_words.clear();
            continue;
        }
        let plain = Regex::new(r"\[\[[^\]]*\]\]|\[[^\]]*@[^\]]*\]|`[^`]*`|\$[^$]*\$|https?://\S+").unwrap().replace_all(t, " ").to_string();
        for sentence in plain.split_inclusive(['.', '!', '?']) {
            let words: Vec<&str> = sentence.split_whitespace().collect();
            let limit = if lang == Lang::Fr { 45 } else { 40 };
            if words.len() > limit {
                out.push(StyleIssue { line, kind: "longue".into(), excerpt: words[..8].join(" ") + " …", message: format!("{} {} {}", lang.pick("phrase de", "sentence of"), words.len(), lang.pick("mots : à couper", "words: split it")) });
            }
            if let Some(m) = first.find(sentence) {
                out.push(StyleIssue { line, kind: "personnelle".into(), excerpt: m.as_str().to_string(), message: lang.pick("première personne du singulier : préférer « nous » ou une tournure impersonnelle", "first person singular: prefer an impersonal phrasing").into() });
            }
            let low = crate::assist::fold(sentence);
            for v in vague {
                let vf = crate::assist::fold(v);
                if Regex::new(&format!(r"\b{}\b", regex::escape(&vf))).unwrap().is_match(&low) {
                    out.push(StyleIssue { line, kind: "vague".into(), excerpt: (*v).to_string(), message: lang.pick("terme vague : préciser (chiffre, exemple, source)", "vague term: be specific (figure, example, source)").into() });
                }
            }
            let ws: HashSet<String> = low.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() >= 7).map(str::to_string).collect();
            for w in ws.intersection(&prev_words) {
                out.push(StyleIssue { line, kind: "répétition".into(), excerpt: w.clone(), message: lang.pick("mot répété dans deux phrases qui se suivent", "word repeated in two consecutive sentences").into() });
            }
            prev_words = ws;
        }
    }
    out
}

/// Set of 5-word sequences of a text (to compare two notes).
fn shingles(text: &str) -> HashSet<u64> {
    use std::hash::{Hash, Hasher};
    let words: Vec<String> = crate::assist::fold(text).split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_string).collect();
    words
        .windows(5)
        .map(|w| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            w.hash(&mut h);
            h.finish()
        })
        .collect()
}

/// Pairs of notes with almost identical content (Jaccard similarity ≥ `threshold`).
pub fn near_duplicates(notes: &[(String, String)], threshold: f32) -> Vec<(String, String, f32)> {
    let sh: Vec<(String, HashSet<u64>)> = notes.iter().map(|(p, t)| (p.clone(), shingles(t))).filter(|(_, s)| s.len() >= 25).collect();
    let mut out = Vec::new();
    for i in 0..sh.len() {
        for j in i + 1..sh.len() {
            let (a, b) = (&sh[i].1, &sh[j].1);
            let small = a.len().min(b.len()) as f32;
            let big = a.len().max(b.len()) as f32;
            if small / big < threshold {
                continue; // sizes too different: the threshold cannot be reached
            }
            let inter = a.intersection(b).count() as f32;
            let jac = inter / (a.len() as f32 + b.len() as f32 - inter);
            if jac >= threshold {
                out.push((sh[i].0.clone(), sh[j].0.clone(), jac));
            }
        }
    }
    out.sort_by(|a, b| b.2.total_cmp(&a.2));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zotero::Creator;

    fn item(ck: &str, title: &str, doi: &str) -> Item {
        Item {
            key: ck.to_uppercase(),
            item_type: "journalArticle".into(),
            title: title.into(),
            creators: vec![Creator { creator_type: "author".into(), first_name: "A".into(), last_name: "B".into() }],
            year: Some(2020),
            publication: "Revue".into(),
            doi: doi.into(),
            citekey: ck.into(),
            fields: [("pages".to_string(), "1-10".to_string())].into(),
            ..Default::default()
        }
    }

    #[test]
    fn bibliography_audit() {
        let mut c = item("c2021", "Autre étude", "");
        c.year = None;
        let items = vec![item("a2020", "Graph Neural Networks for Fraud", "10.1/x"), item("a2020b", "Graph neural networks for fraud!", "10.1/X"), c, item("é 2", "Encore un article long", "10.1/z")];
        let r = bib_audit(&items, Lang::Fr);
        assert_eq!(r.iter().filter(|x| x.kind == "doublon").count(), 2);
        let m = r.iter().find(|x| x.kind == "champs manquants").unwrap();
        assert_eq!((m.citekey.as_str(), m.detail.as_str()), ("c2021", "année, DOI"));
        assert!(r.iter().any(|x| x.kind == "clé" && x.citekey == "é 2"));
    }

    #[test]
    fn validated_citations() {
        let lib: HashSet<String> = ["a2020".to_string()].into();
        let t = "Voir [@a2020, p. 3] et [@inconnu2019].\nÉcrire à moi@exemple.org.\n\\cite{a2020,b2018}\n```\n[@code]\n```";
        let r = validate_citations(t, &lib);
        let keys: Vec<(&str, bool)> = r.iter().map(|c| (c.key.as_str(), c.known)).collect();
        assert_eq!(keys, vec![("b2018", false), ("inconnu2019", false), ("a2020", true)]);
        assert_eq!(r[2].lines, vec![1, 3]);
    }

    #[test]
    fn academic_style() {
        let long = std::iter::repeat_n("mot", 50).collect::<Vec<_>>().join(" ");
        let t = format!("---\ntitre: x\n---\n# Titre\n\nJe pense que c'est très important.\n{long}.\n> Je cite un auteur.\nLa détection automatique progresse. La détection reste difficile.");
        let r = style_check(&t, Lang::Fr);
        let kinds: Vec<(usize, &str)> = r.iter().map(|x| (x.line, x.kind.as_str())).collect();
        assert!(kinds.contains(&(6, "personnelle")) && kinds.contains(&(6, "vague")));
        assert!(kinds.contains(&(7, "longue")));
        assert!(!kinds.iter().any(|(l, _)| *l == 8), "les citations ne sont pas vérifiées");
        assert!(r.iter().any(|x| x.kind == "répétition" && x.excerpt == "detection"));
    }

    #[test]
    fn near_duplicates_cases() {
        let base: String = (0..60).map(|i| format!("mot{i} ")).collect();
        let notes = vec![("a.md".to_string(), base.clone()), ("b.md".to_string(), format!("{base} Une phrase de plus.")), ("c.md".to_string(), "Tout autre chose. ".repeat(40))];
        let d = near_duplicates(&notes, 0.6);
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].0.as_str(), d[0].1.as_str()), ("a.md", "b.md"));
    }
}
