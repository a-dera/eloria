//! Source reliability: verifiable signals (retraction, publication type,
//! journal, citations), never an opaque score. Data from OpenAlex, which includes the
//! Retraction Watch database for retractions. Kept in `.eloria/fiabilite.json`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::lang::Lang;

/// What OpenAlex knows about an article and its journal.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct Reliability {
    pub doi: String,
    /// Date of the check (YYYY-MM-DD)
    pub checked: String,
    pub retracted: bool,
    /// OpenAlex type: article, preprint, review, conference-paper, book-chapter…
    pub work_type: String,
    /// Published, accepted or submitted version (publishedVersion, acceptedVersion, submittedVersion)
    pub version: String,
    pub venue: String,
    /// journal, conference, repository, book series, ebook platform
    pub venue_type: String,
    pub publisher: String,
    pub in_doaj: bool,
    /// Source in the CWTS "core" (recognised scientific journals)
    pub core: bool,
    pub venue_h_index: Option<i64>,
    pub venue_mean_citedness: Option<f64>,
    pub cited_by: i64,
    pub year: Option<i32>,
    pub open_access: bool,
    /// The article is known to OpenAlex
    pub known: bool,
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(|x| x.as_str()).unwrap_or_default()
}

/// Reads an OpenAlex work (`work`) and, if known, its journal (`source`).
pub fn parse(doi: &str, work: &Value, source: Option<&Value>, today: &str) -> Reliability {
    let loc = work.get("primary_location").filter(|l| !l.is_null());
    let src = loc.and_then(|l| l.get("source")).filter(|x| !x.is_null());
    let flag = |v: Option<&Value>, k: &str| v.and_then(|x| x.get(k)).and_then(|b| b.as_bool()).unwrap_or(false);
    let stats = source.and_then(|x| x.get("summary_stats"));
    Reliability {
        doi: doi.to_lowercase(),
        checked: today.to_string(),
        retracted: work.get("is_retracted").and_then(|b| b.as_bool()).unwrap_or(false),
        work_type: s(work, "type").to_string(),
        version: loc.map(|l| s(l, "version").to_string()).unwrap_or_default(),
        venue: src.map(|x| s(x, "display_name").to_string()).unwrap_or_default(),
        venue_type: src.map(|x| s(x, "type").to_string()).unwrap_or_default(),
        publisher: src.map(|x| s(x, "host_organization_name").to_string()).filter(|p| !p.is_empty()).or_else(|| source.map(|x| s(x, "host_organization_name").to_string())).unwrap_or_default(),
        in_doaj: flag(src, "is_in_doaj") || flag(source, "is_in_doaj"),
        core: flag(src, "is_core") || flag(source, "is_core"),
        venue_h_index: stats.and_then(|x| x.get("h_index")).and_then(|h| h.as_i64()),
        venue_mean_citedness: stats.and_then(|x| x.get("2yr_mean_citedness")).and_then(|h| h.as_f64()),
        cited_by: work.get("cited_by_count").and_then(|c| c.as_i64()).unwrap_or(0),
        year: work.get("publication_year").and_then(|y| y.as_i64()).map(|y| y as i32),
        open_access: work.get("open_access").and_then(|o| o.get("is_oa")).and_then(|b| b.as_bool()).unwrap_or(false),
        known: true,
    }
}

/// Level of a signal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Ok,
    Info,
    Attention,
    Alerte,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Signal {
    pub level: Level,
    pub text: String,
}

/// Readable signals. None of them judges the article for you: they are facts to check.
pub fn signals(r: &Reliability, current_year: i32, lang: Lang) -> Vec<Signal> {
    let mut out = Vec::new();
    let mut push = |level: Level, fr: String, en: String| out.push(Signal { level, text: if lang == Lang::Fr { fr } else { en } });
    if !r.known {
        push(Level::Attention, "Article inconnu d'OpenAlex : vérifie la revue et le DOI à la main.".into(), "Article unknown to OpenAlex: check the journal and DOI by hand.".into());
        return out;
    }
    if r.retracted {
        push(Level::Alerte, "Article rétracté (base Retraction Watch, via OpenAlex). Ne le cite pas comme preuve.".into(), "Retracted article (Retraction Watch database, via OpenAlex). Do not cite it as evidence.".into());
    }
    let preprint = r.work_type == "preprint" || r.venue_type == "repository" || r.version == "submittedVersion";
    if preprint {
        push(
            Level::Attention,
            format!("Prépublication ou dépôt ({}) : pas d'évaluation par les pairs attestée. Cherche une version publiée.", if r.venue.is_empty() { "entrepôt" } else { &r.venue }),
            format!("Preprint or repository deposit ({}): no peer review on record. Look for a published version.", if r.venue.is_empty() { "repository" } else { &r.venue }),
        );
    }
    if r.venue.is_empty() && !preprint {
        push(Level::Attention, "Revue ou éditeur inconnus d'OpenAlex.".into(), "Journal or publisher unknown to OpenAlex.".into());
    }
    if !r.venue.is_empty() && !preprint {
        let kind = match r.venue_type.as_str() {
            "journal" => ("revue", "journal"),
            "conference" => ("conférence", "conference"),
            "book series" | "ebook platform" => ("collection d'ouvrages", "book series"),
            _ => ("source", "source"),
        };
        match r.venue_h_index {
            Some(h) if h >= 50 => push(Level::Ok, format!("{} établie : {} (h-index {h}).", cap(kind.0), r.venue), format!("Established {}: {} (h-index {h}).", kind.1, r.venue)),
            Some(h) if h < 10 => push(Level::Attention, format!("{} peu citée : {} (h-index {h}). Vérifie son comité et son indexation.", cap(kind.0), r.venue), format!("Little-cited {}: {} (h-index {h}). Check its board and indexing.", kind.1, r.venue)),
            Some(h) => push(Level::Info, format!("{} : {} (h-index {h}).", cap(kind.0), r.venue), format!("{}: {} (h-index {h}).", cap(kind.1), r.venue)),
            None => push(Level::Info, format!("{} : {}.", cap(kind.0), r.venue), format!("{}: {}.", cap(kind.1), r.venue)),
        }
        if r.core {
            push(Level::Ok, "Source du noyau scientifique CWTS (indexation reconnue).".into(), "CWTS core source (recognised indexing).".into());
        }
        if r.in_doaj {
            push(Level::Ok, "Revue en accès ouvert référencée dans le DOAJ.".into(), "Open-access journal listed in the DOAJ.".into());
        } else if r.open_access && r.venue_type == "journal" && !r.core && r.venue_h_index.map(|h| h < 10).unwrap_or(true) {
            push(Level::Attention, "Revue en accès ouvert absente du DOAJ et du noyau CWTS : vérifie qu'elle n'est pas prédatrice.".into(), "Open-access journal missing from DOAJ and the CWTS core: check it is not predatory.".into());
        }
    }
    let age = r.year.map(|y| current_year - y).unwrap_or(0);
    if r.cited_by >= 100 {
        push(Level::Ok, format!("Très cité : {} citations.", r.cited_by), format!("Highly cited: {} citations.", r.cited_by));
    } else if r.cited_by == 0 && age >= 3 {
        push(Level::Attention, format!("Jamais cité en {age} ans."), format!("Never cited in {age} years."));
    } else {
        push(Level::Info, format!("{} citation(s).", r.cited_by), format!("{} citation(s).", r.cited_by));
    }
    out
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Today's date (YYYY-MM-DD), to date a check.
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Signals computed for the current year.
pub fn signals_now(r: &Reliability, lang: Lang) -> Vec<Signal> {
    signals(r, chrono::Datelike::year(&chrono::Local::now()), lang)
}

/// Most severe level of the signals.
pub fn worst(signals: &[Signal]) -> Level {
    signals.iter().map(|s| s.level).max().unwrap_or(Level::Info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reliability_signals() {
        let work = json!({"is_retracted": false, "type": "article", "cited_by_count": 240, "publication_year": 2019,
            "open_access": {"is_oa": true},
            "primary_location": {"version": "publishedVersion", "source": {"display_name": "IEEE Access", "type": "journal", "is_in_doaj": true, "is_core": true, "host_organization_name": "IEEE"}}});
        let src = json!({"summary_stats": {"h_index": 204, "2yr_mean_citedness": 3.4}});
        let r = parse("10.1109/X", &work, Some(&src), "2026-11-08");
        assert_eq!((r.doi.as_str(), r.venue_h_index, r.publisher.as_str()), ("10.1109/x", Some(204), "IEEE"));
        let sg = signals(&r, 2026, Lang::Fr);
        assert_eq!(worst(&sg), Level::Ok, "{sg:?}");
        assert!(sg.iter().any(|x| x.text.contains("Revue établie : IEEE Access (h-index 204)")));
        assert!(sg.iter().any(|x| x.text.contains("DOAJ")));

        // Retracted: alert, no matter what.
        let mut bad = r.clone();
        bad.retracted = true;
        assert_eq!(worst(&signals(&bad, 2026, Lang::Fr)), Level::Alerte);

        // Zenodo preprint, never cited.
        let pre = json!({"type": "preprint", "cited_by_count": 0, "publication_year": 2021,
            "primary_location": {"version": "submittedVersion", "source": {"display_name": "Zenodo", "type": "repository"}}});
        let p = signals(&parse("10.5281/z", &pre, None, "2026-11-08"), 2026, Lang::Fr);
        assert!(p.iter().any(|x| x.level == Level::Attention && x.text.starts_with("Prépublication ou dépôt (Zenodo)")));
        assert!(p.iter().any(|x| x.text == "Jamais cité en 5 ans."));

        // Open-access journal, rarely cited, outside DOAJ and the core: to check.
        let oa = json!({"type": "article", "cited_by_count": 2, "publication_year": 2025, "open_access": {"is_oa": true},
            "primary_location": {"version": "publishedVersion", "source": {"display_name": "Intl J. of Everything", "type": "journal", "is_in_doaj": false}}});
        let o = signals(&parse("10.1/o", &oa, Some(&json!({"summary_stats": {"h_index": 3}})), "2026-11-08"), 2026, Lang::En);
        assert!(o.iter().any(|x| x.text.contains("predatory")));
        assert!(o.iter().any(|x| x.text.contains("Little-cited journal")));

        let unknown = Reliability { doi: "10.1/u".into(), ..Default::default() };
        assert_eq!(worst(&signals(&unknown, 2026, Lang::Fr)), Level::Attention);
    }
}
