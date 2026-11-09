//! Research statistics: citations (documents, authors, journals, years), topic
//! heat map (tags by month of activity) and coverage map (topics per
//! chapter, from the cited documents). Everything is computed from the vault.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

/// (label, count)
pub type Count = (String, usize);

#[derive(Debug, Clone, Serialize, Default)]
pub struct ResearchStats {
    /// Citations in working notes (excluding sources and reading notes)
    pub citations_total: usize,
    pub documents_cited: usize,
    pub documents_total: usize,
    pub top_documents: Vec<(String, String, usize)>,
    pub top_authors: Vec<Count>,
    pub top_venues: Vec<Count>,
    /// Years of the cited documents
    pub years: Vec<(i32, usize)>,
    /// Heat map: tags (rows) × "YYYY-MM" months (columns)
    pub heat_tags: Vec<String>,
    pub heat_months: Vec<String>,
    pub heat: Vec<Vec<usize>>,
    /// Coverage: topics (rows) × chapters (columns) = cited documents of the topic
    pub cover_themes: Vec<String>,
    pub cover_chapters: Vec<String>,
    pub cover: Vec<Vec<usize>>,
    /// Topics rich in sources (at least 3) that no chapter cites yet
    pub gaps: Vec<Count>,
}

/// The `n` largest, ties in alphabetical order.
pub fn top(map: HashMap<String, usize>, n: usize) -> Vec<Count> {
    let mut v: Vec<Count> = map.into_iter().filter(|(k, _)| !k.trim().is_empty()).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.to_lowercase().cmp(&b.0.to_lowercase())));
    v.truncate(n);
    v
}

/// "YYYY-MM" months of the last `n` months up to `last` included, oldest to newest.
pub fn months_back(last: (i32, u32), n: usize) -> Vec<String> {
    let (mut y, mut m) = last;
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(format!("{y:04}-{m:02}"));
        if m == 1 {
            m = 12;
            y -= 1;
        } else {
            m -= 1;
        }
    }
    out.reverse();
    out
}

/// Rows × columns matrix from (row, column) pairs.
pub fn matrix(rows: &[String], cols: &[String], pairs: &[(String, String)]) -> Vec<Vec<usize>> {
    let ri: HashMap<&str, usize> = rows.iter().enumerate().map(|(i, r)| (r.as_str(), i)).collect();
    let ci: HashMap<&str, usize> = cols.iter().enumerate().map(|(i, c)| (c.as_str(), i)).collect();
    let mut m = vec![vec![0; cols.len()]; rows.len()];
    for (r, c) in pairs {
        if let (Some(&i), Some(&j)) = (ri.get(r.as_str()), ci.get(c.as_str())) {
            m[i][j] += 1;
        }
    }
    m
}

/// Topics (tags) with enough sources but absent from every chapter.
pub fn gaps(source_tags: &BTreeMap<String, HashSet<String>>, covered: &HashSet<String>, min: usize) -> Vec<Count> {
    let mut v: Vec<Count> = source_tags.iter().filter(|(t, docs)| docs.len() >= min && !covered.contains(*t)).map(|(t, d)| (t.clone(), d.len())).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_building_blocks() {
        let mut m = HashMap::new();
        m.insert("Smith".to_string(), 3);
        m.insert("Kone".to_string(), 5);
        m.insert("Abbas".to_string(), 3);
        m.insert("".to_string(), 9);
        assert_eq!(top(m, 2), vec![("Kone".to_string(), 5), ("Abbas".to_string(), 3)]);
        assert_eq!(months_back((2026, 2), 3), vec!["2025-12", "2026-01", "2026-02"]);
        let rows = vec!["gnn".to_string(), "xai".to_string()];
        let cols = vec!["ch2".to_string(), "ch4".to_string()];
        let pairs = vec![("gnn".into(), "ch2".into()), ("gnn".into(), "ch2".into()), ("xai".into(), "ch4".into()), ("autre".into(), "ch4".into())];
        assert_eq!(matrix(&rows, &cols, &pairs), vec![vec![2, 0], vec![0, 1]]);
        let mut st = BTreeMap::new();
        st.insert("blockchain".to_string(), HashSet::from(["a".to_string(), "b".to_string(), "c".to_string()]));
        st.insert("gnn".to_string(), HashSet::from(["a".to_string(), "b".to_string(), "d".to_string()]));
        st.insert("rare".to_string(), HashSet::from(["a".to_string()]));
        assert_eq!(gaps(&st, &HashSet::from(["gnn".to_string()]), 3), vec![("blockchain".to_string(), 3)]);
    }
}
