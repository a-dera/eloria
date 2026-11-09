//! Personal taxonomy: a `Taxonomie.md` note as a nested list,
//!
//! ```text
//! - Fraude
//!   - Social engineering (phishing, vishing)
//!   - Mule accounts
//! ```
//!
//! Each term becomes a hierarchical tag (`fraud/social-engineering`). A note is
//! filed under a term if it carries this tag (or a sub-tag); a note that mentions
//! a term or an alias without carrying the tag gets a filing suggestion.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TaxNode {
    pub label: String,
    /// Hierarchical tag ("fraud/social-engineering")
    pub tag: String,
    pub depth: usize,
    /// Forms searched in notes (the label and the aliases in parentheses)
    pub terms: Vec<String>,
}

fn slug(s: &str) -> String {
    let t: String = crate::assist::fold(s.trim())
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    t.split('-').filter(|x| !x.is_empty()).collect::<Vec<_>>().join("-")
}

/// Reads the nested list (`-`, `*` or `+` bullets, indented by 2 or 4 spaces or tabs).
pub fn parse(content: &str) -> Vec<TaxNode> {
    let (_, body, _) = crate::markdown::split_frontmatter(content);
    let mut out: Vec<TaxNode> = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new(); // (indentation, tag)
    let unit = body
        .lines()
        .filter_map(|l| {
            let ind = l.len() - l.trim_start().len();
            (ind > 0 && matches!(l.trim_start().chars().next(), Some('-' | '*' | '+'))).then_some(ind)
        })
        .min()
        .unwrap_or(2);
    for line in body.lines() {
        let expanded = line.replace('\t', &" ".repeat(unit));
        let t = expanded.trim_start();
        let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ ")) else { continue };
        let ind = expanded.len() - t.len();
        let (label, aliases) = match rest.split_once('(') {
            Some((l, a)) => (l.trim().to_string(), a.trim_end().trim_end_matches(')').split([',', ';']).map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect::<Vec<_>>()),
            None => (rest.trim().to_string(), vec![]),
        };
        if label.is_empty() {
            continue;
        }
        while stack.last().map(|(i, _)| *i >= ind).unwrap_or(false) {
            stack.pop();
        }
        let tag = match stack.last() {
            Some((_, parent)) => format!("{parent}/{}", slug(&label)),
            None => slug(&label),
        };
        let mut terms = vec![label.clone()];
        terms.extend(aliases);
        out.push(TaxNode { label, tag: tag.clone(), depth: stack.len(), terms });
        stack.push((ind, tag));
    }
    out
}

/// Tags suggested for a note: the most precise terms it mentions and whose tag it
/// does not have yet (a parent is not suggested if a child is).
pub fn propose(folded_text: &str, note_tags: &[String], nodes: &[TaxNode]) -> Vec<String> {
    let has = |tag: &str| note_tags.iter().any(|t| t.to_lowercase() == tag || t.to_lowercase().starts_with(&format!("{tag}/")));
    let hits: Vec<&TaxNode> = nodes.iter().filter(|n| !has(&n.tag) && n.terms.iter().any(|t| crate::methods::contains_term(folded_text, t))).collect();
    hits.iter().filter(|n| !hits.iter().any(|m| m.tag.starts_with(&format!("{}/", n.tag)))).map(|n| n.tag.clone()).collect()
}

/// Taxonomy skeleton from existing tags (hierarchical ones first).
pub fn skeleton(tags: &[(String, i64)], lang: crate::lang::Lang) -> String {
    let mut md = format!(
        "---\ntype: taxonomie\n---\n# {}\n\n{}\n\n",
        lang.pick("Taxonomie", "Taxonomy"),
        lang.pick(
            "Une ligne par terme, en liste imbriquée ; des alias entre parenthèses aident à retrouver le terme dans les notes. Chaque terme devient un tag hiérarchique.",
            "One line per term, as a nested list; aliases in parentheses help find the term in notes. Each term becomes a hierarchical tag."
        )
    );
    let mut roots: Vec<String> = Vec::new();
    let mut children: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for (t, _) in tags {
        let parts: Vec<&str> = t.split('/').collect();
        if ["source", "fiche", "concept"].contains(&parts[0]) {
            continue;
        }
        if !roots.contains(&parts[0].to_string()) {
            roots.push(parts[0].to_string());
        }
        if parts.len() > 1 {
            children.entry(parts[0].to_string()).or_default().push(parts[1..].join("/"));
        }
    }
    for r in roots.iter().take(20) {
        md.push_str(&format!("- {}\n", r.replace('-', " ")));
        for c in children.get(r).into_iter().flatten() {
            md.push_str(&format!("  - {}\n", c.replace('-', " ")));
        }
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taxonomy_read_and_applied() {
        let md = "---\ntype: taxonomie\n---\n# Taxonomie\n\nIntro.\n\n- Fraude\n  - Ingénierie sociale (phishing, vishing)\n  - Comptes mules (mule accounts)\n- Méthodes\n    - Graphes (GNN)\n";
        let n = parse(md);
        assert_eq!(n.iter().map(|x| x.tag.as_str()).collect::<Vec<_>>(), vec!["fraude", "fraude/ingenierie-sociale", "fraude/comptes-mules", "methodes", "methodes/graphes"]);
        assert_eq!(n[1].terms, vec!["Ingénierie sociale", "phishing", "vishing"]);
        assert_eq!((n[0].depth, n[2].depth, n[4].depth), (0, 1, 1));
        let text = crate::assist::fold("Le phishing vise les usagers ; la fraude par comptes mules suit.");
        // The child is enough: "fraude" is not suggested on top of its sub-terms.
        assert_eq!(propose(&text, &[], &n), vec!["fraude/ingenierie-sociale", "fraude/comptes-mules"]);
        assert_eq!(propose(&text, &["fraude/comptes-mules".into()], &n), vec!["fraude/ingenierie-sociale"]);
        let sk = skeleton(&[("fraude/mules".into(), 2), ("gnn".into(), 3), ("source".into(), 9)], crate::lang::Lang::Fr);
        assert!(sk.contains("- fraude\n  - mules\n- gnn\n") && !sk.contains("- source"), "{sk}");
        assert_eq!(parse(&sk).len(), 3);
    }
}
