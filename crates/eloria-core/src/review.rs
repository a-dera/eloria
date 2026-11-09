//! Literature review: a `type: revue` note holds the question, the inclusion and
//! exclusion criteria, and the list of articles with their status (to read, included,
//! excluded). The matrix and the state-of-the-art draft only cover included articles.
//!
//! Everything lives in the note's properties (readable and editable by hand):
//!
//! ```yaml
//! type: revue
//! question: How can mobile money fraud be detected?
//! inclusion: [Real data, 2018 and later]
//! exclusion: [Bank cards only]
//! articles:
//!   - cle: smith2024graph
//!     statut: inclus
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::lang::Lang;

pub const STATUSES: [&str; 3] = ["à lire", "inclus", "exclu"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewArticle {
    pub citekey: String,
    pub statut: String,
    /// Completed by the workspace: title, "Smith (2024)", reading notes and source
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author_year: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub fiches: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Review {
    pub path: String,
    pub title: String,
    pub question: String,
    pub inclusion: Vec<String>,
    pub exclusion: Vec<String>,
    pub articles: Vec<ReviewArticle>,
}

fn list(fm: &Map<String, Value>, k: &str) -> Vec<String> {
    match fm.get(k) {
        Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        Some(Value::String(s)) if !s.trim().is_empty() => vec![s.clone()],
        _ => vec![],
    }
}

/// Reads a review from the properties of its note.
pub fn from_frontmatter(path: &str, title: &str, fm: &Map<String, Value>) -> Review {
    let articles = match fm.get("articles") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| {
                let (ck, st) = match v {
                    Value::Object(o) => (o.get("cle").and_then(|x| x.as_str())?.to_string(), o.get("statut").and_then(|x| x.as_str()).unwrap_or("à lire").to_string()),
                    // Short form: "- smith2024graph"
                    Value::String(s) => (s.trim_start_matches('@').to_string(), "à lire".to_string()),
                    _ => return None,
                };
                Some(ReviewArticle { citekey: ck, statut: st, title: String::new(), author_year: String::new(), source: String::new(), fiches: vec![] })
            })
            .collect(),
        _ => vec![],
    };
    Review {
        path: path.to_string(),
        title: title.to_string(),
        question: fm.get("question").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        inclusion: list(fm, "inclusion"),
        exclusion: list(fm, "exclusion"),
        articles,
    }
}

/// New list of articles after a status change (`""` removes the article).
pub fn set_status(articles: &[ReviewArticle], citekey: &str, statut: &str) -> Value {
    let mut out: Vec<Value> = Vec::new();
    let mut found = false;
    for a in articles {
        if a.citekey.eq_ignore_ascii_case(citekey) {
            found = true;
            if !statut.is_empty() {
                out.push(json!({ "cle": a.citekey, "statut": statut }));
            }
        } else {
            out.push(json!({ "cle": a.citekey, "statut": a.statut }));
        }
    }
    if !found && !statut.is_empty() {
        out.push(json!({ "cle": citekey, "statut": statut }));
    }
    Value::Array(out)
}

/// Content of a new review note.
pub fn new_note(title: &str, question: &str, lang: Lang) -> String {
    let q = question.replace('"', "'");
    match lang {
        Lang::Fr => format!(
            "---\ntype: revue\nquestion: \"{q}\"\ninclusion: []\nexclusion: []\narticles: []\ntags: [revue]\n---\n# {title}\n\n> [!astuce] Revue de littérature\n> Ouvre la revue (bouton « Ouvrir la revue ») pour ajouter des articles, les inclure ou les exclure, voir leur matrice et rédiger un brouillon d'état de l'art.\n\n## Méthode de recherche\n\nBases interrogées, requêtes, dates.\n\n## Synthèse\n\n"
        ),
        Lang::En => format!(
            "---\ntype: revue\nquestion: \"{q}\"\ninclusion: []\nexclusion: []\narticles: []\ntags: [review]\n---\n# {title}\n\n> [!tip] Literature review\n> Open the review (\"Open the review\" button) to add articles, include or exclude them, see their matrix and draft a state of the art.\n\n## Search method\n\nDatabases, queries, dates.\n\n## Synthesis\n\n"
        ),
    }
}

/// Instructions for the state-of-the-art draft (excerpts are the reading notes and sources of the
/// included articles, numbered [S1]…).
pub fn draft_system(lang: Lang, question: &str) -> String {
    match lang {
        Lang::Fr => format!(
            "Tu rédiges, pour une thèse de doctorat, un brouillon de section « État de l'art » répondant à la question : « {question} ».\n\n\
Règles absolues :\n\
- Appuie-toi UNIQUEMENT sur les extraits numérotés [S1], [S2]… fournis (fiches de lecture et sources des articles inclus).\n\
- Chaque affirmation tirée d'un extrait est suivie de sa source : [S2] ou [S1][S4]. N'invente aucune référence, aucun chiffre, aucun auteur.\n\
- Organise par thèmes (approches, données, résultats, limites), compare et confronte les travaux, termine par les manques de la littérature.\n\
- Style académique français, paragraphes rédigés (pas de liste à puces), titres de niveau 3 (###).\n\
- Si un extrait est trop pauvre, n'en tire rien plutôt que de deviner."
        ),
        Lang::En => format!(
            "You are drafting, for a doctoral thesis, a \"state of the art\" section answering: \"{question}\".\n\n\
Absolute rules:\n\
- Rely ONLY on the numbered excerpts [S1], [S2]… provided (reading notes and sources of the included articles).\n\
- Every claim taken from an excerpt is followed by its source: [S2] or [S1][S4]. Never invent a reference, figure or author.\n\
- Organize by themes (approaches, data, results, limitations), compare and contrast the works, end with the gaps in the literature.\n\
- Academic English, written paragraphs (no bullet lists), level 3 headings (###).\n\
- If an excerpt is too thin, draw nothing from it rather than guess."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_read_and_modified() {
        let fm: Map<String, Value> = serde_json::from_value(json!({
            "type": "revue",
            "question": "Fraude ?",
            "inclusion": ["Données réelles"],
            "exclusion": "Cartes seules",
            "articles": [{"cle": "a2020", "statut": "inclus"}, "@b2021"]
        }))
        .unwrap();
        let r = from_frontmatter("Revues/R.md", "R", &fm);
        assert_eq!(r.question, "Fraude ?");
        assert_eq!((r.inclusion.len(), r.exclusion[0].as_str()), (1, "Cartes seules"));
        assert_eq!(r.articles.iter().map(|a| (a.citekey.as_str(), a.statut.as_str())).collect::<Vec<_>>(), vec![("a2020", "inclus"), ("b2021", "à lire")]);
        let v = set_status(&r.articles, "B2021", "exclu");
        assert_eq!(v, json!([{"cle": "a2020", "statut": "inclus"}, {"cle": "b2021", "statut": "exclu"}]));
        let v = set_status(&r.articles, "c2022", "à lire");
        assert_eq!(v.as_array().unwrap().len(), 3);
        let v = set_status(&r.articles, "a2020", "");
        assert_eq!(v, json!([{"cle": "b2021", "statut": "à lire"}]));
        let note = new_note("Revue fraude", "Comment \"détecter\" ?", Lang::Fr);
        let parsed = crate::markdown::parse_note(&note);
        assert_eq!(parsed.frontmatter["type"], "revue");
        assert_eq!(parsed.frontmatter["question"], "Comment 'détecter' ?");
    }
}
