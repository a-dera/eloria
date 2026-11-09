//! Concept base, knowledge cards and how a concept evolves: definition of a
//! concept note, the sentence of a note where the concept appears, the date of a note, and the
//! regenerated block of a knowledge card (text written around it is kept).

use serde::Serialize;

use crate::assist::fold;

pub const CARD_START: &str = "<!-- eloria:carte:debut -->";
pub const CARD_END: &str = "<!-- eloria:carte:fin -->";

/// Definition of a concept note: the `> [!definition]` callout, otherwise the first
/// paragraphe de texte.
pub fn definition(content: &str) -> String {
    let (_, body, _) = crate::markdown::split_frontmatter(content);
    let lines: Vec<&str> = body.lines().collect();
    if let Some(i) = lines.iter().position(|l| fold(l.trim_start()).starts_with("> [!definition]") || fold(l.trim_start()).starts_with("> [!definition")) {
        let first = lines[i].split_once(']').map(|x| x.1).unwrap_or_default().trim();
        let mut parts: Vec<String> = Vec::new();
        // The callout title ("Définition") is not the definition.
        if !first.is_empty() && !matches!(fold(first).as_str(), "definition" | "définition") {
            parts.push(first.to_string());
        }
        for l in &lines[i + 1..] {
            let Some(rest) = l.trim_start().strip_prefix('>') else { break };
            if !rest.trim().is_empty() {
                parts.push(rest.trim().to_string());
            }
        }
        let d = parts.join(" ");
        if !d.is_empty() {
            return d;
        }
    }
    body.split("\n\n")
        .map(str::trim)
        .find(|p| !p.is_empty() && !p.starts_with('#') && !p.starts_with('>') && !p.starts_with("<!--") && !p.starts_with("- "))
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

/// Markdown line reduced to its readable text: without heading, quote,
/// callout, list, bold or code marks; `[[target|alias]]` becomes the alias.
pub fn plain_line(line: &str) -> String {
    use std::sync::OnceLock;
    static LINK: OnceLock<regex::Regex> = OnceLock::new();
    static CALLOUT: OnceLock<regex::Regex> = OnceLock::new();
    let link = LINK.get_or_init(|| regex::Regex::new(r"!?\[\[([^\]|#]*)(?:#[^\]|]*)?(?:\|([^\]]*))?\]\]").unwrap());
    let callout = CALLOUT.get_or_init(|| regex::Regex::new(r"^\[![\w-]+\][+-]?\s*").unwrap());
    let mut t = line.trim().trim_start_matches(['>', ' ']).to_string();
    t = callout.replace(&t, "").to_string();
    let t = t.trim_start_matches(['-', '*', '+', ' ']).trim_start_matches("[ ] ").trim_start_matches("[x] ");
    let t = link.replace_all(t, |c: &regex::Captures| c.get(2).or(c.get(1)).map(|m| m.as_str().trim().to_string()).unwrap_or_default());
    t.replace("**", "").replace("==", "").replace('`', "").trim().to_string()
}

/// The sentence of a text that contains one of the terms (whole words), shortened.
/// Headings, comments, code blocks and tables are ignored.
pub fn mention(text: &str, terms: &[String], max: usize) -> Option<String> {
    let mut in_code = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code || l.is_empty() || l.starts_with('#') || l.starts_with("<!--") || l.starts_with('|') || l.starts_with("---") {
            continue;
        }
        let plain = plain_line(l);
        for s in plain.split_inclusive(['.', '?', '!']) {
            let f = fold(s);
            if terms.iter().any(|t| crate::methods::contains_term(&f, t)) {
                let s = s.trim();
                let short: String = s.chars().take(max).collect();
                return Some(if s.chars().count() > max { format!("{}…", short.trim_end()) } else { short });
            }
        }
    }
    None
}

/// Date of a note for the timeline: `date`, `cree_le`, `ajoute_le`, otherwise the
/// modification (millisecondes).
pub fn note_date(fm: &serde_json::Map<String, serde_json::Value>, mtime_ms: i64) -> String {
    for k in ["date", "cree_le", "ajoute_le", "created"] {
        if let Some(s) = fm.get(k).and_then(|v| v.as_str()) {
            let d: String = s.chars().take(10).collect();
            if d.len() == 10 && d.as_bytes()[4] == b'-' {
                return d;
            }
        }
    }
    chrono::DateTime::from_timestamp_millis(mtime_ms).map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string()).unwrap_or_default()
}

/// Replaces (or appends) the generated block of a knowledge card.
pub fn replace_card(content: &str, block: &str) -> String {
    let full = format!("{CARD_START}\n{}\n{CARD_END}", block.trim_end());
    match (content.find(CARD_START), content.find(CARD_END)) {
        (Some(a), Some(b)) if b > a => format!("{}{}{}", &content[..a], full, &content[b + CARD_END.len()..]),
        _ => format!("{}\n\n{full}\n", content.trim_end()),
    }
}

/// A dated note that talks about the concept.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Mention {
    pub date: String,
    pub path: String,
    pub title: String,
    pub kind: String,
    pub citekey: String,
    pub excerpt: String,
    /// Explicit link to the concept note (otherwise a plain mention of the name)
    pub linked: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConceptRow {
    pub path: String,
    pub title: String,
    pub aliases: Vec<String>,
    pub definition: String,
    /// Sources and reading notes that talk about it
    pub sources: usize,
    /// Every note that talks about it
    pub notes: usize,
    pub updated: String,
}

/// Definition at a date (versions of the concept note).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DefinitionVersion {
    pub date: String,
    pub definition: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConceptDetail {
    pub path: String,
    pub title: String,
    pub aliases: Vec<String>,
    pub definition: String,
    /// Notes that talk about it, oldest to newest
    pub timeline: Vec<Mention>,
    /// Related concepts (links in either direction)
    pub related: Vec<(String, String)>,
    /// Research questions that talk about it
    pub questions: Vec<(String, String)>,
    /// Successive definitions (only when it changed)
    pub definitions: Vec<DefinitionVersion>,
    /// Existing knowledge card
    pub card: String,
}

#[cfg(test)]
mod tests {
    #[test]
    fn readable_mention() {
        let terms = vec![fold("Mobile money")];
        let text = "# Mobile money en titre\n<!-- zotero:debut -->\n> [!astuce] Le **mobile money** domine. Autre phrase.\n";
        assert_eq!(mention(text, &terms, 200).as_deref(), Some("Le mobile money domine."));
        let text = "- Voir [[Mobile money|l'argent mobile]] et [[Mobile money]] ici.";
        assert_eq!(mention(text, &[fold("argent mobile")], 200).as_deref(), Some("Voir l'argent mobile et Mobile money ici."));
        assert_eq!(mention("```\nmobile money\n```\n", &terms, 200), None);
    }

    use super::*;

    #[test]
    fn knowledge_building_blocks() {
        let c = "---\ntype: concept\n---\n# Comptes mules\n\n> [!definition] Définition\n> Comptes qui font transiter\n> l'argent d'une fraude.\n\n## Explication\n\nTexte.";
        assert_eq!(definition(c), "Comptes qui font transiter l'argent d'une fraude.");
        assert_eq!(definition("# X\n\nPremier paragraphe\nsur deux lignes.\n\nSuite."), "Premier paragraphe sur deux lignes.");
        assert_eq!(mention("Intro. Les comptes mules blanchissent. Fin.", &["comptes mules".into()], 80).as_deref(), Some("Les comptes mules blanchissent."));
        assert_eq!(mention("Rien ici.", &["mules".into()], 80), None);
        let mut fm = serde_json::Map::new();
        fm.insert("cree_le".into(), "2025-03-02".into());
        assert_eq!(note_date(&fm, 0), "2025-03-02");
        let card = replace_card("# Carte\n\nMon texte.", "## Définition\n\nA.");
        assert!(card.starts_with("# Carte\n\nMon texte.\n\n<!-- eloria:carte:debut -->\n## Définition"));
        let again = replace_card(&card.replace("Mon texte.", "Mon texte modifié."), "## Définition\n\nB.");
        assert!(again.contains("Mon texte modifié.") && again.contains("\n\nB.\n<!-- eloria:carte:fin -->") && !again.contains("\nA.\n"));
    }
}
