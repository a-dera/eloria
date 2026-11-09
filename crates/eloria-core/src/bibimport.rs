//! Reads a BibTeX file (export from Mendeley, JabRef, Google Scholar…): each
//! entry becomes a work (`online::Work`) with its original key. Common LaTeX
//! commands in fields (accents, `\&`, protecting braces) are converted.

use crate::online::Work;

/// A BibTeX entry: key and work.
#[derive(Debug, Clone, PartialEq)]
pub struct BibEntry {
    pub key: String,
    pub work: Work,
}

/// Replaces LaTeX accents (`{\'e}`, `\"o`, `\c{c}`…) and removes braces.
pub fn delatex(s: &str) -> String {
    let mut t = s.to_string();
    let acc: &[(&str, &[(&str, &str)])] = &[
        ("'", &[("e", "é"), ("a", "á"), ("i", "í"), ("o", "ó"), ("u", "ú"), ("E", "É"), ("c", "ć"), ("n", "ń"), ("y", "ý")]),
        ("`", &[("e", "è"), ("a", "à"), ("u", "ù"), ("E", "È"), ("A", "À"), ("i", "ì"), ("o", "ò")]),
        ("^", &[("e", "ê"), ("a", "â"), ("i", "î"), ("o", "ô"), ("u", "û"), ("E", "Ê")]),
        ("\"", &[("e", "ë"), ("a", "ä"), ("i", "ï"), ("o", "ö"), ("u", "ü"), ("O", "Ö"), ("U", "Ü"), ("A", "Ä")]),
        ("~", &[("n", "ñ"), ("a", "ã"), ("o", "õ"), ("N", "Ñ")]),
        ("c", &[("c", "ç"), ("C", "Ç")]),
    ];
    for (cmd, pairs) in acc {
        for (letter, out) in *pairs {
            let sep = if cmd.chars().all(|c| c.is_ascii_alphabetic()) { " " } else { "" };
            for pat in [
                format!("{{\\{cmd}{{{letter}}}}}"),
                format!("{{\\{cmd}{sep}{letter}}}"),
                format!("\\{cmd}{{{letter}}}"),
                format!("\\{cmd}{sep}{letter}"),
            ] {
                t = t.replace(&pat, out);
            }
        }
    }
    for (a, b) in [("\\&", "&"), ("\\%", "%"), ("\\_", "_"), ("\\$", "$"), ("\\#", "#"), ("---", "—"), ("--", "–"), ("~", " "), ("\\ss", "ß"), ("\\o", "ø"), ("\\textendash", "–")] {
        t = t.replace(a, b);
    }
    t.replace(['{', '}'], "").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Value of a field from `start` (after the `=`); returns (raw value, end).
fn value_at(s: &str, start: usize) -> (String, usize) {
    let b = s.as_bytes();
    let mut i = start;
    while i < b.len() && (b[i] as char).is_whitespace() {
        i += 1;
    }
    let mut out = String::new();
    loop {
        if i >= b.len() {
            return (out, i);
        }
        match b[i] {
            b'{' => {
                let mut depth = 0;
                let from = i;
                while i < b.len() {
                    match b[i] {
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                out.push_str(&s[from + 1..i.min(s.len())]);
                i += 1;
            }
            b'"' => {
                let from = i + 1;
                i += 1;
                let mut depth = 0;
                while i < b.len() && !(b[i] == b'"' && depth == 0) {
                    match b[i] {
                        b'{' => depth += 1,
                        b'}' => depth -= 1,
                        _ => {}
                    }
                    i += 1;
                }
                out.push_str(&s[from..i.min(s.len())]);
                i += 1;
            }
            _ => {
                let from = i;
                while i < b.len() && !matches!(b[i], b',' | b'}' | b'#') && !(b[i] as char).is_whitespace() {
                    i += 1;
                }
                out.push_str(&s[from..i]);
            }
        }
        while i < b.len() && (b[i] as char).is_whitespace() {
            i += 1;
        }
        // Concatenation "a # b".
        if i < b.len() && b[i] == b'#' {
            i += 1;
            while i < b.len() && (b[i] as char).is_whitespace() {
                i += 1;
            }
            continue;
        }
        return (out, i);
    }
}

fn authors(raw: &str) -> Vec<(String, String)> {
    raw.split(" and ")
        .map(str::trim)
        .filter(|a| !a.is_empty() && *a != "others")
        .map(|a| {
            // "{World Bank}": corporate author, kept whole.
            if a.starts_with('{') && a.ends_with('}') {
                return (String::new(), delatex(a));
            }
            match a.split_once(',') {
                Some((last, first)) => (delatex(first), delatex(last)),
                None => match a.rsplit_once(' ') {
                    Some((first, last)) => (delatex(first), delatex(last)),
                    None => (String::new(), delatex(a)),
                },
            }
        })
        .collect()
}

fn kind(t: &str) -> &'static str {
    match t {
        "article" => "journalArticle",
        "inproceedings" | "conference" | "proceedings" => "conferencePaper",
        "book" => "book",
        "incollection" | "inbook" => "bookSection",
        "phdthesis" | "mastersthesis" | "thesis" => "thesis",
        "techreport" | "report" => "report",
        "online" | "electronic" | "www" => "webpage",
        _ => "document",
    }
}

/// Entries of a BibTeX file (without @comment, @string, @preamble).
pub fn parse(text: &str) -> Vec<BibEntry> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(at) = text[pos..].find('@').map(|i| i + pos) {
        let Some(open) = text[at..].find(['{', '(']).map(|i| i + at) else { break };
        let typ = text[at + 1..open].trim().to_lowercase();
        // End of the entry: the balancing closing brace.
        let mut depth = 0;
        let mut end = open;
        for (i, c) in text[open..].char_indices() {
            match c {
                '{' | '(' => depth += 1,
                '}' | ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        if end == open {
            break;
        }
        pos = end + 1;
        if matches!(typ.as_str(), "comment" | "string" | "preamble") || typ.is_empty() {
            continue;
        }
        let inner = &text[open + 1..end];
        let Some(comma) = inner.find(',') else { continue };
        let key = inner[..comma].trim().to_string();
        let body = &inner[comma + 1..];
        let mut fields: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut i = 0;
        while let Some(eq) = body[i..].find('=').map(|e| e + i) {
            let name = body[i..eq].trim().trim_start_matches(',').trim().to_lowercase();
            let (v, next) = value_at(body, eq + 1);
            if !name.is_empty() {
                fields.insert(name, v);
            }
            i = next;
            if i >= body.len() {
                break;
            }
        }
        let f = |k: &str| fields.get(k).map(|v| delatex(v)).unwrap_or_default();
        let title = f("title");
        if title.is_empty() || key.is_empty() {
            continue;
        }
        let year = f("year").chars().take(4).collect::<String>().parse().ok().or_else(|| f("date").chars().take(4).collect::<String>().parse().ok());
        let venue = ["journal", "journaltitle", "booktitle", "publisher", "howpublished", "institution", "school"].iter().map(|k| f(k)).find(|v| !v.is_empty()).unwrap_or_default();
        let doi = crate::online::normalize_doi(&f("doi")).unwrap_or_default();
        out.push(BibEntry {
            key,
            work: Work {
                title,
                authors: authors(fields.get("author").or_else(|| fields.get("editor")).map(String::as_str).unwrap_or_default()),
                year,
                url: if doi.is_empty() { f("url") } else { format!("https://doi.org/{doi}") },
                doi,
                venue,
                abstract_note: f("abstract"),
                item_type: kind(&typ).into(),
                citations: None,
                origin: "bibtex".into(),
                keywords: f("keywords").split([',', ';']).map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect(),
            },
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bibtex_parsing() {
        let bib = r#"
@comment{exporté par JabRef}
@string{ieee = "IEEE Access"}
@article{Smith2024,
  author = {Smith, Jane and Ou{\'e}draogo, Ali and {Banque mondiale}},
  title = {Graph {Neural} Networks for Fraud \& Laundering},
  journal = ieee,
  year = 2024,
  doi = {https://doi.org/10.1109/ACCESS.2024.0001},
  keywords = {fraud; GNN, mobile money},
  abstract = "We detect {mule} accounts."
}
@inproceedings{kone21,
  author = "Awa Kon{\'e}",
  title = "M{\"u}nster -- revisited",
  booktitle = {Proc. KDD},
  date = {2021-08-14}
}
@misc{sanstitre, author = {X}}
"#;
        let e = parse(bib);
        assert_eq!(e.len(), 2);
        let a = &e[0];
        assert_eq!(a.key, "Smith2024");
        assert_eq!(a.work.title, "Graph Neural Networks for Fraud & Laundering");
        assert_eq!(a.work.authors, vec![("Jane".into(), "Smith".into()), ("Ali".into(), "Ouédraogo".into()), ("".into(), "Banque mondiale".into())]);
        assert_eq!((a.work.year, a.work.doi.as_str(), a.work.item_type.as_str()), (Some(2024), "10.1109/access.2024.0001", "journalArticle"));
        assert_eq!(a.work.keywords, vec!["fraud", "GNN", "mobile money"]);
        assert_eq!(a.work.abstract_note, "We detect mule accounts.");
        let b = &e[1];
        assert_eq!((b.work.title.as_str(), b.work.year, b.work.venue.as_str(), b.work.item_type.as_str()), ("Münster – revisited", Some(2021), "Proc. KDD", "conferencePaper"));
        assert_eq!(b.work.authors, vec![("Awa".into(), "Koné".into())]);
    }
}
