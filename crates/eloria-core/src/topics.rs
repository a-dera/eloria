//! Topic clustering: TF-IDF vectors of the notes, k-means on
//! cosine similarity, deterministic initialisation (farthest first). Each
//! group is named after its most characteristic terms. Local, no AI.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use crate::assist::fold;

/// Words too common to characterise a topic (French, English, Markdown).
const STOP: &[&str] = &[
    "alors", "aussi", "autre", "autres", "avant", "avec", "avoir", "cela", "celle", "celui", "cette", "comme", "comment", "dans", "depuis", "deux", "donc",
    "dont", "elle", "elles", "encore", "entre", "etre", "fait", "faire", "leur", "leurs", "mais", "meme", "moins", "notre", "nous", "peut", "plus", "pour",
    "quand", "quel", "quelle", "selon", "sans", "sont", "sous", "tous", "tout", "toute", "toutes", "tres", "votre", "vous", "voir", "about", "after", "also",
    "among", "been", "both", "between", "could", "each", "from", "have", "into", "more", "most", "only", "other", "over", "same", "such", "than", "that",
    "their", "them", "then", "there", "these", "they", "this", "those", "through", "using", "used", "very", "were", "what", "when", "where", "which", "while",
    "with", "would", "your", "type", "tags", "titre", "source", "fiche", "note", "notes", "http", "https", "www", "page", "pages", "zotero", "citekey",
    "annee", "auteurs", "resume", "statut", "chapitre", "chapitres", "debut", "fin", "eloria", "annotations", "voir", "aussi", "exemple",
];

/// A note to cluster.
pub struct TopicDoc {
    pub path: String,
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Topic {
    /// Most characteristic terms (most frequent form, with accents)
    pub terms: Vec<String>,
    /// (path, title, closeness to the centre 0..1), most central first
    pub notes: Vec<(String, String, f32)>,
    /// Suggested tag ("thème/…")
    pub tag: String,
}

/// Sorted sparse vector: the order of additions, hence the result, is reproducible.
type Vector = BTreeMap<u32, f32>;

fn tokens(text: &str) -> Vec<(String, String)> {
    text.split(|c: char| !c.is_alphanumeric() && c != '-')
        .map(|w| w.trim_matches('-'))
        .filter(|w| w.chars().count() >= 4 && !w.chars().all(|c| c.is_ascii_digit()))
        .map(|w| (fold(w), w.to_lowercase()))
        .filter(|(f, _)| !STOP.contains(&f.as_str()))
        .collect()
}

fn cosine(a: &Vector, b: &Vector) -> f32 {
    let (small, big) = if a.len() < b.len() { (a, b) } else { (b, a) };
    small.iter().filter_map(|(k, v)| big.get(k).map(|w| v * w)).sum()
}

fn normalize(v: &mut Vector) {
    let n = v.values().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        for x in v.values_mut() {
            *x /= n;
        }
    }
}

/// Topic groups. `k`: number of groups (0 = chosen from the number of notes).
pub fn cluster(docs: &[TopicDoc], k: usize) -> Vec<Topic> {
    // Vocabulary and displayed forms.
    let mut vocab: HashMap<String, u32> = HashMap::new();
    let mut display: HashMap<String, HashMap<String, usize>> = HashMap::new();
    let mut df: HashMap<u32, usize> = HashMap::new();
    let mut tfs: Vec<HashMap<u32, f32>> = Vec::new();
    for d in docs {
        let mut tf: HashMap<u32, f32> = HashMap::new();
        for (f, orig) in tokens(&format!("{} {} {}", d.title, d.title, d.text)) {
            let n = vocab.len() as u32;
            let id = *vocab.entry(f.clone()).or_insert(n);
            *tf.entry(id).or_default() += 1.0;
            *display.entry(f).or_default().entry(orig).or_default() += 1;
        }
        for id in tf.keys() {
            *df.entry(*id).or_default() += 1;
        }
        tfs.push(tf);
    }
    let n = docs.len().max(1) as f32;
    // A term present everywhere or in a single note separates nothing.
    let vectors: Vec<Vector> = tfs
        .into_iter()
        .map(|tf| {
            let mut v: Vector = tf
                .into_iter()
                .filter(|(id, _)| {
                    let d = df[id];
                    d >= 2 && (d as f32) < n * 0.6
                })
                .map(|(id, c)| (id, (1.0 + c.ln()) * (n / df[&id] as f32).ln()))
                .collect();
            normalize(&mut v);
            v
        })
        .collect();
    let usable: Vec<usize> = (0..docs.len()).filter(|&i| !vectors[i].is_empty()).collect();
    if usable.len() < 2 {
        return Vec::new();
    }
    let k = if k == 0 { ((usable.len() as f32 / 3.0).sqrt().round() as usize).clamp(2, 12) } else { k }.min(usable.len());
    // Initialisation: the richest note, then each time the farthest from the centres.
    let mut centers: Vec<Vector> = Vec::new();
    let first = *usable.iter().max_by_key(|&&i| (vectors[i].len(), std::cmp::Reverse(i))).unwrap();
    centers.push(vectors[first].clone());
    while centers.len() < k {
        let far = usable
            .iter()
            .copied()
            .max_by(|&a, &b| {
                let da = centers.iter().map(|c| cosine(&vectors[a], c)).fold(f32::MIN, f32::max);
                let db = centers.iter().map(|c| cosine(&vectors[b], c)).fold(f32::MIN, f32::max);
                db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal).then(b.cmp(&a))
            })
            .unwrap();
        centers.push(vectors[far].clone());
    }
    let mut assign: Vec<usize> = vec![usize::MAX; docs.len()];
    for _ in 0..25 {
        let mut changed = false;
        for &i in &usable {
            let best = (0..centers.len())
                .max_by(|&a, &b| cosine(&vectors[i], &centers[a]).partial_cmp(&cosine(&vectors[i], &centers[b])).unwrap_or(std::cmp::Ordering::Equal).then(b.cmp(&a)))
                .unwrap();
            if assign[i] != best {
                assign[i] = best;
                changed = true;
            }
        }
        for (c, center) in centers.iter_mut().enumerate() {
            let members: Vec<usize> = usable.iter().copied().filter(|&i| assign[i] == c).collect();
            if members.is_empty() {
                continue;
            }
            let mut sum: Vector = BTreeMap::new();
            for i in &members {
                for (id, v) in &vectors[*i] {
                    *sum.entry(*id).or_default() += v;
                }
            }
            normalize(&mut sum);
            *center = sum;
        }
        if !changed {
            break;
        }
    }
    let rev: HashMap<u32, &String> = vocab.iter().map(|(w, id)| (*id, w)).collect();
    let mut topics: Vec<Topic> = Vec::new();
    for (c, center) in centers.iter().enumerate() {
        let mut notes: Vec<(String, String, f32)> = usable.iter().filter(|&&i| assign[i] == c).map(|&i| (docs[i].path.clone(), docs[i].title.clone(), cosine(&vectors[i], center))).collect();
        if notes.is_empty() {
            continue;
        }
        notes.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
        let mut top: Vec<(&u32, &f32)> = center.iter().collect();
        top.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal).then(rev[a.0].cmp(rev[b.0])));
        let mut seen = HashSet::new();
        let terms: Vec<String> = top
            .iter()
            .filter_map(|(id, _)| {
                let f = rev[*id];
                let shown = display[f].iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(w, _)| w.clone()).unwrap_or_else(|| f.clone());
                seen.insert(crate::assist::stem(f)).then_some(shown)
            })
            .take(3)
            .collect();
        let tag = format!("thème/{}", terms.first().cloned().unwrap_or_default().replace(' ', "-"));
        topics.push(Topic { terms, notes, tag });
    }
    topics.sort_by(|a, b| b.notes.len().cmp(&a.notes.len()).then(a.terms.cmp(&b.terms)));
    topics
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(p: &str, t: &str) -> TopicDoc {
        TopicDoc { path: p.into(), title: p.into(), text: t.into() }
    }

    #[test]
    fn two_separate_topics() {
        let docs = vec![
            d("a", "Les graphes de transactions révèlent la fraude organisée ; les graphes denses signalent des comptes mules."),
            d("b", "Un modèle de graphes sur les transactions détecte la fraude des comptes mules."),
            d("c", "Fraude et graphes : les transactions entre comptes mules forment des communautés."),
            d("d", "Les valeurs de Shapley expliquent les prédictions ; SHAP attribue une importance aux variables."),
            d("e", "SHAP et LIME expliquent localement les prédictions d'un modèle par l'importance des variables."),
            d("f", "Explications des prédictions : importance des variables selon Shapley et SHAP."),
        ];
        let t = cluster(&docs, 2);
        assert_eq!(t.len(), 2);
        let mut groups: Vec<Vec<String>> = t.iter().map(|x| { let mut v: Vec<String> = x.notes.iter().map(|n| n.0.clone()).collect(); v.sort(); v }).collect();
        groups.sort();
        assert_eq!(groups, vec![vec!["a", "b", "c"], vec!["d", "e", "f"]]);
        let all_terms: Vec<String> = t.iter().flat_map(|x| x.terms.clone()).collect();
        assert!(all_terms.iter().any(|w| w == "graphes" || w == "transactions" || w == "mules"), "{all_terms:?}");
        assert!(t.iter().all(|x| x.tag.starts_with("thème/")));
        // Same input, same result.
        assert_eq!(cluster(&docs, 2), t);
        assert!(cluster(&docs[..1], 0).is_empty());
    }
}
