//! Similarity check preparation: passages of a manuscript that repeat word
//! for word (8-word sequences) texts of the library (PDF text, abstracts,
//! annotations). Local comparison; does not replace plagiarism-detection software.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use serde::Serialize;

use crate::assist::fold;

/// Length of the compared word sequences.
pub const N: usize = 8;

/// A library text.
pub struct LibDoc {
    pub citekey: String,
    pub title: String,
    pub path: String,
    pub text: String,
}

/// Words of a text: (folded word, start, end) as byte offsets in the original text.
fn words(text: &str) -> Vec<(String, usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in text.char_indices() {
        if c.is_alphanumeric() {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            out.push((fold(&text[s..i]), s, i));
        }
    }
    if let Some(s) = start {
        out.push((fold(&text[s..]), s, text.len()));
    }
    out
}

fn hash(ws: &[(String, usize, usize)]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for w in ws {
        w.0.hash(&mut h);
    }
    h.finish()
}

/// Index of the library's word sequences: fingerprint -> (document, word position).
pub struct Index {
    docs: Vec<(LibDoc, Vec<(String, usize, usize)>)>,
    map: HashMap<u64, Vec<(u32, u32)>>,
}

impl Index {
    pub fn new(docs: Vec<LibDoc>) -> Self {
        let mut map: HashMap<u64, Vec<(u32, u32)>> = HashMap::new();
        let docs: Vec<(LibDoc, Vec<(String, usize, usize)>)> = docs.into_iter().map(|d| {
            let w = words(&d.text);
            (d, w)
        }).collect();
        for (di, (_, w)) in docs.iter().enumerate() {
            for i in 0..w.len().saturating_sub(N - 1) {
                let e = map.entry(hash(&w[i..i + N])).or_default();
                // A very frequent sequence (stock phrases) is only kept a few times.
                if e.len() < 8 && e.last().map(|x| x.0) != Some(di as u32) {
                    e.push((di as u32, i as u32));
                }
            }
        }
        Index { docs, map }
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }
}

/// A passage of the manuscript close to a library text.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SimilarityHit {
    /// Line of the paragraph (1 = first line of the file)
    pub line: usize,
    /// The paragraph, shortened around the repeated passage
    pub excerpt: String,
    /// The repeated passage as written in the manuscript
    pub matched: String,
    pub matched_words: usize,
    /// Share of the paragraph repeated (0 to 1)
    pub coverage: f32,
    pub citekey: String,
    pub title: String,
    pub path: String,
    /// The same passage in the source
    pub source_excerpt: String,
    /// The paragraph already cites this document (`[@key]` or `\cite{key}`)
    pub cited: bool,
}

/// Paragraphs of the manuscript (text, start line, offset), excluding headings, code blocks,
/// block quotes (`>`), front matter and comments.
fn paragraphs(content: &str) -> Vec<(String, usize)> {
    let (_, body, offset) = crate::markdown::split_frontmatter(content);
    // `offset`: number of lines before the body.
    let first_line = offset + 1;
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut start = 0;
    let mut in_code = false;
    for (i, line) in body.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        let skip = in_code || t.starts_with('#') || t.starts_with('>') || t.starts_with("<!--") || t.starts_with('%') || t.starts_with('\\');
        if t.is_empty() || skip {
            if !cur.trim().is_empty() {
                out.push((std::mem::take(&mut cur), start));
            }
            cur.clear();
            continue;
        }
        if cur.is_empty() {
            start = first_line + i;
        }
        cur.push_str(line);
        cur.push('\n');
    }
    if !cur.trim().is_empty() {
        out.push((cur, start));
    }
    out
}

fn around(text: &str, from: usize, to: usize, pad: usize) -> String {
    let mut a = from.saturating_sub(pad);
    while a > 0 && !text.is_char_boundary(a) {
        a -= 1;
    }
    let mut b = (to + pad).min(text.len());
    while b < text.len() && !text.is_char_boundary(b) {
        b += 1;
    }
    let s = text[a..b].split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{}{}{}", if a > 0 { "…" } else { "" }, s, if b < text.len() { "…" } else { "" })
}

/// Repeated passages: for each paragraph and each document, the longest sequence of
/// identical words (at least `N`). Longest first.
pub fn check(content: &str, index: &Index) -> Vec<SimilarityHit> {
    let mut hits = Vec::new();
    for (para, line) in paragraphs(content) {
        let w = words(&para);
        if w.len() < N {
            continue;
        }
        // Positions of the paragraph that start a found sequence, per document.
        let mut by_doc: HashMap<u32, Vec<(usize, u32)>> = HashMap::new();
        for i in 0..=w.len() - N {
            if let Some(list) = self_hits(index, &w[i..i + N]) {
                for &(d, pos) in list {
                    by_doc.entry(d).or_default().push((i, pos));
                }
            }
        }
        let para_folded = fold(&para);
        for (d, starts) in by_doc {
            // Longest continuous sequence (consecutive positions in both texts).
            let mut best = (0usize, 0usize, 0u32); // (start, length in words, source position)
            let mut run_start = starts[0];
            let mut prev = starts[0];
            let mut covered: HashSet<usize> = HashSet::new();
            for &(i, pos) in &starts {
                for k in i..i + N {
                    covered.insert(k);
                }
                if (i, pos) != run_start && !(i == prev.0 + 1 && pos == prev.1 + 1) {
                    run_start = (i, pos);
                }
                let len = i - run_start.0 + N;
                if len > best.1 {
                    best = (run_start.0, len, run_start.1);
                }
                prev = (i, pos);
            }
            let (doc, dw) = &index.docs[d as usize];
            let (s, l) = (best.0, best.1);
            let matched = para[w[s].1..w[s + l - 1].2].split_whitespace().collect::<Vec<_>>().join(" ");
            let p = best.2 as usize;
            let src_end = (p + l - 1).min(dw.len() - 1);
            let ck = doc.citekey.to_lowercase();
            let cited = !ck.is_empty() && (para_folded.contains(&format!("@{ck}")) || para_folded.contains(&format!("{{{ck}}}")) || para_folded.contains(&format!(",{ck}")) || para_folded.contains(&format!(", {ck}")));
            hits.push(SimilarityHit {
                line,
                excerpt: around(&para, w[s].1, w[s + l - 1].2, 80),
                matched,
                matched_words: l,
                coverage: covered.len() as f32 / w.len() as f32,
                citekey: doc.citekey.clone(),
                title: doc.title.clone(),
                path: doc.path.clone(),
                source_excerpt: around(&doc.text, dw[p].1, dw[src_end].2, 80),
                cited,
            });
        }
    }
    hits.sort_by(|a, b| b.matched_words.cmp(&a.matched_words).then(a.line.cmp(&b.line)));
    hits
}

fn self_hits<'a>(index: &'a Index, ws: &[(String, usize, usize)]) -> Option<&'a Vec<(u32, u32)>> {
    index.map.get(&hash(ws))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib() -> Index {
        Index::new(vec![
            LibDoc {
                citekey: "smith2024".into(),
                title: "GNN fraud".into(),
                path: "Sources/@smith2024.md".into(),
                text: "Introduction. Fraud rings tend to form dense subgraphs with high transaction velocity in mobile money networks. Our dataset covers 2.3M transactions.".into(),
            },
            LibDoc { citekey: "autre".into(), title: "Autre".into(), path: "".into(), text: "Rien à voir avec le reste du texte, vraiment rien du tout ici.".into() },
        ])
    }

    #[test]
    fn repeated_passages() {
        let ms = "---\ntitre: Ch2\n---\n# Chapitre\n\nComme on le sait, fraud rings tend to form dense subgraphs with high transaction velocity, ce qui aide.\n\n> Fraud rings tend to form dense subgraphs with high transaction velocity (citation en retrait, ignorée).\n\nLes réseaux de fraude forment des sous-graphes denses [@smith2024].\n\nAinsi fraud rings tend to form dense subgraphs with high transaction velocity [@smith2024].\n";
        let hits = check(ms, &lib());
        assert_eq!(hits.len(), 2, "{hits:?}");
        let h = &hits[0];
        assert_eq!((h.line, h.citekey.as_str(), h.matched_words, h.cited), (6, "smith2024", 11, false));
        assert_eq!(h.matched, "fraud rings tend to form dense subgraphs with high transaction velocity");
        assert!(h.source_excerpt.contains("Fraud rings tend to form dense subgraphs"));
        assert!(h.coverage > 0.5 && h.coverage < 1.0);
        // The second one, already cited.
        assert_eq!((hits[1].line, hits[1].cited), (12, true));
        // Fewer than 8 identical words: nothing.
        assert!(check("Fraud rings tend to form dense structures.", &lib()).is_empty());
        // Case, accents and punctuation ignored.
        let lib2 = Index::new(vec![LibDoc { citekey: "k".into(), title: "".into(), path: "".into(), text: "L'économie numérique transforme profondément les services financiers des ménages africains.".into() }]);
        assert_eq!(check("l economie NUMERIQUE transforme, profondément, les services financiers des ménages.", &lib2).len(), 1);
    }
}
