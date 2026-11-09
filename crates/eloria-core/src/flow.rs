//! "New document" flow: what is computed without AI to prepare a reading note
//! (tags suggested from your own tags). The sequence (import, metadata, PDF
//! text, reliability, links, note) lives in `Workspace` and in the interface.

use std::collections::HashSet;

use crate::assist::{fold, stem};

/// Eloria's structural tags: never suggested.
const STRUCTURAL: &[&str] = &["source", "fiche", "concept", "chapitre", "plan", "modele", "modèle", "revue", "journal", "capture", "idee", "idée"];

/// Vault tags whose words all appear in the item's text (title,
/// abstract, start of the PDF), most used first. Tags already present are excluded.
pub fn propose_tags(text: &str, vault_tags: &[(String, i64)], current: &[String]) -> Vec<String> {
    let words: HashSet<String> = fold(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3)
        .map(stem)
        .collect();
    let have: HashSet<String> = current.iter().map(|t| t.to_lowercase()).collect();
    let mut out: Vec<(i64, String)> = vault_tags
        .iter()
        .filter(|(t, _)| !have.contains(&t.to_lowercase()) && !STRUCTURAL.contains(&t.to_lowercase().as_str()))
        .filter(|(t, _)| {
            // "thèse/méthodo": the last level counts.
            let leaf = t.rsplit('/').next().unwrap_or(t);
            let parts: Vec<String> = fold(leaf).split(['-', '_', ' ']).filter(|p| p.chars().count() >= 3).map(stem).collect();
            !parts.is_empty() && parts.iter().all(|p| words.contains(p))
        })
        .map(|(t, n)| (*n, t.clone()))
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    out.into_iter().map(|(_, t)| t).take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_suggested_from_vault() {
        let tags = vec![
            ("mobile-money".to_string(), 12),
            ("fraude".to_string(), 30),
            ("gnn".to_string(), 4),
            ("thèse/méthodologie".to_string(), 2),
            ("source".to_string(), 140),
            ("blockchain".to_string(), 3),
        ];
        let text = "Graph neural networks (GNN) for fraud detection in mobile money: a methodology for West African transactions.";
        assert_eq!(propose_tags(text, &tags, &["source".into(), "gnn".into()]), vec!["fraude", "mobile-money", "thèse/méthodologie"]);
        assert!(propose_tags("rien à voir", &tags, &[]).is_empty());
    }
}
