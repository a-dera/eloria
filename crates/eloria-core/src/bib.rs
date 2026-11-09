//! BibTeX export from the Zotero mirror (for Pandoc, LaTeX, Typst).

use crate::zotero::{Creator, Item};

fn entry_type(t: &str) -> &'static str {
    match t {
        "journalArticle" | "magazineArticle" | "newspaperArticle" => "article",
        "book" => "book",
        "bookSection" => "incollection",
        "conferencePaper" => "inproceedings",
        "thesis" => "phdthesis",
        "report" => "techreport",
        "manuscript" => "unpublished",
        _ => "misc",
    }
}

/// Escapes LaTeX special characters (Unicode accents are kept: biber/Pandoc handle them).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' | '%' | '$' | '#' | '_' => {
                out.push('\\');
                out.push(c);
            }
            '{' | '}' => {}
            '\n' | '\r' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

fn names(cs: &[&Creator]) -> String {
    cs.iter()
        .map(|c| {
            if c.first_name.is_empty() {
                format!("{{{}}}", escape(&c.last_name))
            } else {
                format!("{}, {}", escape(&c.last_name), escape(&c.first_name))
            }
        })
        .collect::<Vec<_>>()
        .join(" and ")
}

pub fn item_to_bibtex(it: &Item) -> String {
    let mut f: Vec<(&str, String)> = Vec::new();
    let authors: Vec<&Creator> = it.creators.iter().filter(|c| c.creator_type == "author" || c.creator_type == "inventor").collect();
    let editors: Vec<&Creator> = it.creators.iter().filter(|c| c.creator_type == "editor").collect();
    if !authors.is_empty() {
        f.push(("author", names(&authors)));
    } else if !it.creators.is_empty() && editors.is_empty() {
        f.push(("author", names(&it.creators.iter().collect::<Vec<_>>())));
    }
    if !editors.is_empty() {
        f.push(("editor", names(&editors)));
    }
    f.push(("title", format!("{{{}}}", escape(&it.title))));
    if let Some(y) = it.year {
        f.push(("year", y.to_string()));
    }
    let g = |k: &str| it.fields.get(k).map(|v| escape(v)).filter(|v| !v.is_empty());
    match it.item_type.as_str() {
        "journalArticle" | "magazineArticle" | "newspaperArticle" => {
            if let Some(v) = g("publicationTitle") {
                f.push(("journal", v));
            }
        }
        "bookSection" | "conferencePaper" => {
            if let Some(v) = g("bookTitle").or_else(|| g("proceedingsTitle")) {
                f.push(("booktitle", v));
            }
        }
        "thesis" => {
            if let Some(v) = g("university") {
                f.push(("school", v));
            }
        }
        "report" => {
            if let Some(v) = g("institution").or_else(|| g("publisher")) {
                f.push(("institution", v));
            }
        }
        _ => {}
    }
    for (bib, z) in [
        ("publisher", "publisher"),
        ("address", "place"),
        ("volume", "volume"),
        ("number", "issue"),
        ("pages", "pages"),
        ("isbn", "ISBN"),
        ("issn", "ISSN"),
        ("doi", "DOI"),
        ("url", "url"),
        ("language", "language"),
    ] {
        if it.item_type == "report" && bib == "publisher" {
            continue;
        }
        if let Some(v) = g(z) {
            f.push((bib, if bib == "pages" { v.replace('-', "--").replace("----", "--") } else { v }));
        }
    }
    let body = f.iter().map(|(k, v)| format!("  {k} = {{{v}}}")).collect::<Vec<_>>().join(",\n");
    format!("@{}{{{},\n{}\n}}\n", entry_type(&it.item_type), it.citekey, body)
}

pub fn to_bibtex(items: &[Item]) -> String {
    items.iter().map(item_to_bibtex).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn bibtex() {
        let it = Item {
            key: "K".into(),
            item_type: "journalArticle".into(),
            title: "Fraud & {Detection}".into(),
            citekey: "a2020f".into(),
            year: Some(2020),
            creators: vec![Creator { creator_type: "author".into(), first_name: "Awa".into(), last_name: "Sawadogo".into() }],
            fields: HashMap::from([("publicationTitle".into(), "J. Test".into()), ("pages".into(), "1-10".into())]),
            ..Default::default()
        };
        let b = item_to_bibtex(&it);
        assert!(b.starts_with("@article{a2020f,"));
        assert!(b.contains("author = {Sawadogo, Awa}"));
        assert!(b.contains("title = {{Fraud \\& Detection}}"));
        assert!(b.contains("journal = {J. Test}"));
        assert!(b.contains("pages = {1--10}"));
    }
}
