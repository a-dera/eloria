//! Thesis workspace: a chapter's objectives (its "Objectifs" section), chapter order
//! from their dependencies (`depend_de: [ch1]`), research questions (`type: question`).

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

/// An objective: an item of the list under "## Objectifs"; checked box = reached.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Objective {
    pub text: String,
    /// `Some(true)` : `- [x]`, `Some(false)` : `- [ ]`, `None` : simple puce
    pub done: Option<bool>,
}

/// Objectives declared in the "Objectifs" (or "Objectives") section of a note.
pub fn objectives(content: &str) -> Vec<Objective> {
    let Some(sec) = crate::markdown::section(content, "Objectifs").or_else(|| crate::markdown::section(content, "Objectives")) else { return vec![] };
    sec.lines()
        .skip(1)
        .filter_map(|l| {
            let t = l.trim_start();
            let rest = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| {
                let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
                (digits > 0).then(|| t[digits..].strip_prefix(". ")).flatten()
            })?;
            let (done, text) = if let Some(x) = rest.strip_prefix("[x] ").or_else(|| rest.strip_prefix("[X] ")) {
                (Some(true), x)
            } else if let Some(x) = rest.strip_prefix("[ ] ") {
                (Some(false), x)
            } else {
                (None, rest)
            };
            let text = text.trim();
            (!text.is_empty()).then(|| Objective { text: text.to_string(), done })
        })
        .collect()
}

/// A chapter and its progress.
#[derive(Debug, Clone, Serialize)]
pub struct ChapterInfo {
    pub id: String,
    pub path: String,
    pub title: String,
    pub status: String,
    pub words: i64,
    /// Word-count goal (`objectif` property)
    pub target: i64,
    pub objectives: Vec<Objective>,
    pub depends_on: Vec<String>,
    /// Attached research questions (paths)
    pub questions: Vec<String>,
    pub fiches: usize,
    pub linked: usize,
}

/// A research question and what is attached to it.
#[derive(Debug, Clone, Serialize)]
pub struct QuestionInfo {
    pub path: String,
    pub title: String,
    /// open, in progress, answered…
    pub status: String,
    pub chapters: Vec<String>,
    /// Cited or linked documents (citation keys)
    pub sources: Vec<String>,
    /// Notes that link to the question
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThesisOverview {
    pub chapters: Vec<ChapterInfo>,
    /// Writing order: levels of chapter identifiers
    pub layers: Vec<Vec<String>>,
    /// (chapter, unknown dependency)
    pub missing: Vec<(String, String)>,
    pub cycle: Vec<String>,
    pub questions: Vec<QuestionInfo>,
}

/// Chapter levels: a chapter comes after those it depends on. Returns the
/// levels (each naturally sorted), dependencies on an unknown chapter and the
/// chapters caught in a cycle (placed at the last level).
pub fn layers(deps: &BTreeMap<String, Vec<String>>) -> (Vec<Vec<String>>, Vec<(String, String)>, Vec<String>) {
    let ids: HashSet<&String> = deps.keys().collect();
    let mut missing = Vec::new();
    for (id, ds) in deps {
        for d in ds {
            if !ids.contains(d) {
                missing.push((id.clone(), d.clone()));
            }
        }
    }
    let mut level: HashMap<String, usize> = HashMap::new();
    let mut remaining: Vec<String> = deps.keys().cloned().collect();
    let mut changed = true;
    while changed {
        changed = false;
        remaining.retain(|id| {
            let ds: Vec<&String> = deps[id].iter().filter(|d| ids.contains(d)).collect();
            if ds.iter().all(|d| level.contains_key(*d)) {
                let l = ds.iter().map(|d| level[*d] + 1).max().unwrap_or(0);
                level.insert(id.clone(), l);
                changed = true;
                false
            } else {
                true
            }
        });
    }
    let cycle = remaining;
    let top = level.values().copied().max().map(|m| m + 1).unwrap_or(0);
    let mut out: Vec<Vec<String>> = vec![Vec::new(); top + usize::from(!cycle.is_empty())];
    for (id, l) in &level {
        out[*l].push(id.clone());
    }
    if !cycle.is_empty() {
        out[top] = cycle.clone();
    }
    for l in &mut out {
        l.sort_by_key(|s| natural(s));
    }
    (out, missing, cycle)
}

/// Natural sort key: "ch2" before "ch10".
pub fn natural(s: &str) -> (String, u64, String) {
    let prefix: String = s.chars().take_while(|c| !c.is_ascii_digit()).collect();
    let digits: String = s.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    (prefix.to_lowercase(), digits.parse().unwrap_or(0), s.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_objectives() {
        let c = "---\ntype: chapitre\n---\n# Ch2\n\n## Objectifs\n\n- [x] Définir la fraude\n- [ ] Typologie des comptes mules\n* Situer le cas UEMOA\n1. Annoncer le ch3\n\n## Plan\n\n- pas un objectif\n";
        let o = objectives(c);
        assert_eq!(o.len(), 4);
        assert_eq!(o[0], Objective { text: "Définir la fraude".into(), done: Some(true) });
        assert_eq!(o[1].done, Some(false));
        assert_eq!((o[2].text.as_str(), o[2].done), ("Situer le cas UEMOA", None));
        assert_eq!(o[3].text, "Annoncer le ch3");
        assert!(objectives("# Sans objectifs").is_empty());
    }

    #[test]
    fn chapter_order() {
        let mut d = BTreeMap::new();
        d.insert("ch1".to_string(), vec![]);
        d.insert("ch2".to_string(), vec!["ch1".to_string()]);
        d.insert("ch10".to_string(), vec!["ch2".to_string(), "ch3".to_string()]);
        d.insert("ch3".to_string(), vec!["ch1".to_string(), "annexe".to_string()]);
        let (l, missing, cycle) = layers(&d);
        assert_eq!(l, vec![vec!["ch1"], vec!["ch2", "ch3"], vec!["ch10"]]);
        assert_eq!(missing, vec![("ch3".to_string(), "annexe".to_string())]);
        assert!(cycle.is_empty());
        // Cycle: ch4 <-> ch5, at the last level.
        d.insert("ch4".to_string(), vec!["ch5".to_string()]);
        d.insert("ch5".to_string(), vec!["ch4".to_string()]);
        let (l, _, cycle) = layers(&d);
        assert_eq!(cycle.len(), 2);
        assert_eq!(l.last().unwrap(), &vec!["ch4".to_string(), "ch5".to_string()]);
    }
}
