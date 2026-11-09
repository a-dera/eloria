//! Online literature search: Crossref (metadata by DOI or by title),
//! Semantic Scholar and OpenAlex (similar articles).
//!
//! Everything returned comes from these databases: no reference is made up. No
//! personal data is sent (no e-mail address in the headers).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Error;
use crate::zotero::{Creator, Item};
use crate::Result;

/// An article as described by a bibliographic database.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct Work {
    pub title: String,
    /// (first name, last name); the first name may be empty (corporate author).
    pub authors: Vec<(String, String)>,
    pub year: Option<i32>,
    pub doi: String,
    pub url: String,
    pub venue: String,
    pub abstract_note: String,
    /// Zotero type (journalArticle, conferencePaper, book…).
    pub item_type: String,
    pub citations: Option<i64>,
    /// Originating database: crossref, semanticscholar, openalex.
    pub origin: String,
    /// Keywords and subjects (OpenAlex, Crossref, Semantic Scholar): become the source's tags.
    pub keywords: Vec<String>,
}

impl Work {
    pub fn authors_short(&self) -> String {
        let last: Vec<&str> = self.authors.iter().map(|(_, l)| l.as_str()).collect();
        match last.len() {
            0 => String::new(),
            1 => last[0].to_string(),
            2 => format!("{} & {}", last[0], last[1]),
            _ => format!("{} et al.", last[0]),
        }
    }

    /// The item in Zotero format (for reading notes, the bibliography and the source view).
    pub fn to_item(&self, citekey: &str) -> Item {
        Item {
            key: String::new(),
            item_type: if self.item_type.is_empty() { "journalArticle".into() } else { self.item_type.clone() },
            title: self.title.clone(),
            creators: self
                .authors
                .iter()
                .map(|(f, l)| Creator { creator_type: "author".into(), first_name: f.clone(), last_name: l.clone() })
                .collect(),
            date: self.year.map(|y| y.to_string()).unwrap_or_default(),
            year: self.year,
            publication: self.venue.clone(),
            doi: self.doi.clone(),
            url: self.url.clone(),
            abstract_note: self.abstract_note.clone(),
            citekey: citekey.to_string(),
            tags: self.keywords.clone(),
            ..Default::default()
        }
    }
}

/// Bare, lower-case DOI, from a DOI, a doi.org link or a text containing one.
pub fn normalize_doi(s: &str) -> Option<String> {
    let s = s.trim();
    let i = s.find("10.")?;
    let rest = &s[i..];
    let slash = rest.find('/')?;
    if slash < 4 || !rest[3..slash].chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let end = rest.find(|c: char| c.is_whitespace() || c == '"' || c == '<' || c == '>').unwrap_or(rest.len());
    let doi = rest[..end].trim_end_matches(['.', ',', ';', ')']).to_lowercase();
    (doi.len() > slash + 1).then_some(doi)
}

/// Google Scholar search (no public API: a link, opened in the browser).
pub fn scholar_url(title: &str) -> String {
    format!("https://scholar.google.com/scholar?q={}", encode(title))
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn split_name(full: &str) -> (String, String) {
    let full = full.trim();
    match full.rsplit_once(' ') {
        Some((f, l)) => (f.trim().to_string(), l.trim().to_string()),
        None => (String::new(), full.to_string()),
    }
}

fn str_at<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(|x| x.as_str()).unwrap_or_default()
}

fn crossref_type(t: &str) -> &'static str {
    match t {
        "journal-article" => "journalArticle",
        "proceedings-article" => "conferencePaper",
        "book-chapter" | "book-section" | "book-part" => "bookSection",
        "book" | "monograph" | "edited-book" | "reference-book" => "book",
        "dissertation" => "thesis",
        "report" | "report-component" => "report",
        "posted-content" => "preprint",
        _ => "journalArticle",
    }
}

/// A Crossref record (`message` of /works/{doi}, or an item of /works).
pub fn parse_crossref(v: &Value) -> Option<Work> {
    let title = v.get("title")?.as_array()?.first()?.as_str()?.trim().to_string();
    if title.is_empty() {
        return None;
    }
    let authors = v
        .get("author")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|p| {
                    let fam = str_at(p, "family");
                    if fam.is_empty() {
                        (String::new(), str_at(p, "name").to_string())
                    } else {
                        (str_at(p, "given").to_string(), fam.to_string())
                    }
                })
                .filter(|(_, l)| !l.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let year = ["issued", "published-print", "published-online", "created"].iter().find_map(|k| {
        v.get(*k)?.get("date-parts")?.as_array()?.first()?.as_array()?.first()?.as_i64().map(|y| y as i32)
    });
    let venue = v.get("container-title").and_then(|c| c.as_array()).and_then(|c| c.first()).and_then(|c| c.as_str()).unwrap_or_default();
    Some(Work {
        title: strip_tags(&title),
        authors,
        year,
        doi: str_at(v, "DOI").to_lowercase(),
        url: str_at(v, "URL").to_string(),
        venue: venue.to_string(),
        abstract_note: strip_tags(str_at(v, "abstract")).trim_start_matches("Abstract").trim().to_string(),
        item_type: crossref_type(str_at(v, "type")).to_string(),
        citations: v.get("is-referenced-by-count").and_then(|c| c.as_i64()),
        origin: "crossref".into(),
        keywords: v.get("subject").and_then(|s| s.as_array()).into_iter().flatten().filter_map(|s| s.as_str()).map(str::to_string).collect(),
    })
}

/// A Semantic Scholar paper (fields title, year, authors, externalIds, url, venue, abstract, citationCount).
pub fn parse_s2(v: &Value) -> Option<Work> {
    let title = str_at(v, "title").trim().to_string();
    if title.is_empty() {
        return None;
    }
    let authors = v
        .get("authors")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().map(|p| split_name(str_at(p, "name"))).filter(|(_, l)| !l.is_empty()).collect())
        .unwrap_or_default();
    let doi = v.get("externalIds").map(|e| str_at(e, "DOI").to_lowercase()).unwrap_or_default();
    Some(Work {
        title,
        authors,
        year: v.get("year").and_then(|y| y.as_i64()).map(|y| y as i32),
        url: if doi.is_empty() { str_at(v, "url").to_string() } else { format!("https://doi.org/{doi}") },
        doi,
        venue: str_at(v, "venue").to_string(),
        abstract_note: str_at(v, "abstract").to_string(),
        item_type: "journalArticle".into(),
        citations: v.get("citationCount").and_then(|c| c.as_i64()),
        origin: "semanticscholar".into(),
        // Fields ("Computer Science", "Business"): broad, useful for lack of anything better.
        keywords: dedup(
            v.get("s2FieldsOfStudy")
                .and_then(|a| a.as_array())
                .into_iter()
                .flatten()
                .map(|f| str_at(f, "category").to_string())
                .chain(v.get("fieldsOfStudy").and_then(|a| a.as_array()).into_iter().flatten().filter_map(|f| f.as_str().map(str::to_string)))
                .collect(),
        ),
    })
}

/// An OpenAlex work (the abstract is an inverted index word -> positions).
pub fn parse_openalex(v: &Value) -> Option<Work> {
    let title = v.get("display_name").or_else(|| v.get("title")).and_then(|t| t.as_str())?.trim().to_string();
    if title.is_empty() {
        return None;
    }
    let authors = v
        .get("authorships")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().filter_map(|x| x.get("author")).map(|p| split_name(str_at(p, "display_name"))).filter(|(_, l)| !l.is_empty()).collect())
        .unwrap_or_default();
    let doi = str_at(v, "doi").trim_start_matches("https://doi.org/").to_lowercase();
    let venue = v.get("primary_location").and_then(|l| l.get("source")).map(|s| str_at(s, "display_name")).unwrap_or_default();
    let mut words: Vec<(i64, &str)> = Vec::new();
    if let Some(Value::Object(m)) = v.get("abstract_inverted_index") {
        for (w, pos) in m {
            for p in pos.as_array().into_iter().flatten().filter_map(|p| p.as_i64()) {
                words.push((p, w.as_str()));
            }
        }
    }
    words.sort();
    let kind = match str_at(v, "type") {
        "book" => "book",
        "book-chapter" => "bookSection",
        "dissertation" => "thesis",
        "report" => "report",
        "preprint" => "preprint",
        _ => "journalArticle",
    };
    Some(Work {
        title,
        authors,
        year: v.get("publication_year").and_then(|y| y.as_i64()).map(|y| y as i32),
        url: if doi.is_empty() { str_at(v, "id").to_string() } else { format!("https://doi.org/{doi}") },
        doi,
        venue: venue.to_string(),
        abstract_note: words.iter().map(|(_, w)| *w).collect::<Vec<_>>().join(" "),
        item_type: kind.into(),
        citations: v.get("cited_by_count").and_then(|c| c.as_i64()),
        origin: "openalex".into(),
        keywords: openalex_keywords(v),
    })
}

/// OpenAlex keywords: the article's keywords, then its primary topic and the
/// most confident concepts (excluding level-0 general disciplines).
fn openalex_keywords(v: &Value) -> Vec<String> {
    let arr = |k: &str| v.get(k).and_then(|a| a.as_array()).cloned().unwrap_or_default();
    let score = |x: &Value| x.get("score").and_then(|s| s.as_f64()).unwrap_or(1.0);
    let mut out: Vec<String> = arr("keywords").iter().filter(|k| score(k) >= 0.3).map(|k| str_at(k, "display_name").to_string()).collect();
    if let Some(t) = v.get("primary_topic").filter(|t| !t.is_null()) {
        out.push(str_at(t, "display_name").to_string());
        if let Some(sub) = t.get("subfield") {
            out.push(str_at(sub, "display_name").to_string());
        }
    }
    if out.len() < 4 {
        out.extend(
            arr("concepts")
                .iter()
                .filter(|c| c.get("level").and_then(|l| l.as_i64()).unwrap_or(0) >= 1 && score(c) >= 0.4)
                .map(|c| str_at(c, "display_name").to_string()),
        );
    }
    dedup(out)
}

fn dedup(list: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for k in list.into_iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        if !out.iter().any(|x| x.eq_ignore_ascii_case(&k)) {
            out.push(k);
        }
    }
    out.truncate(10);
    out
}

/// Completes an article (abstract, keywords, journal, year) with OpenAlex then Semantic
/// Scholar: Crossref rarely gives the abstract and almost never keywords. Best effort:
/// a database that does not answer is ignored.
pub fn enrich(w: &mut Work) {
    let Some(d) = normalize_doi(&w.doi) else { return };
    let need = |w: &Work| w.abstract_note.trim().len() < 40 || w.keywords.len() < 3;
    if need(w) {
        if let Ok(Some(o)) = get_json(&format!("https://api.openalex.org/works/doi:{d}"), &[]).map(|v| v.as_ref().and_then(parse_openalex)) {
            merge(w, o);
        }
    }
    if need(w) {
        if let Ok(Some(s)) = get_json(&format!("https://api.semanticscholar.org/graph/v1/paper/DOI:{d}"), &[("fields", S2_FIELDS)]).map(|v| v.as_ref().and_then(parse_s2)) {
            merge(w, s);
        }
    }
    let no_abstract = |w: &Work| w.abstract_note.trim().len() < 40;
    // DataCite: DOIs from Zenodo, figshare, arXiv, data repositories.
    if no_abstract(w) {
        if let Ok(Some(x)) = datacite(&d) {
            merge(w, x);
        }
    }
    // Europe PMC: health, biology, and part of the social sciences.
    if no_abstract(w) {
        let q = format!("DOI:\"{d}\"");
        if let Ok(Some(v)) = get_json("https://www.ebi.ac.uk/europepmc/webservices/rest/search", &[("query", &q), ("resultType", "core"), ("format", "json")]) {
            if let Some(r) = v.get("resultList").and_then(|r| r.get("result")).and_then(|r| r.as_array()).and_then(|r| r.first()) {
                let kw = r.get("keywordList").and_then(|k| k.get("keyword")).and_then(|k| k.as_array()).into_iter().flatten().filter_map(|k| k.as_str().map(str::to_string)).collect();
                merge(w, Work { abstract_note: strip_tags(str_at(r, "abstractText")), keywords: kw, ..Default::default() });
            }
        }
    }
    // Publisher page: citation_abstract, dc.description tags (Highwire, Dublin Core).
    if no_abstract(w) {
        if let Ok(html) = get_text(&format!("https://doi.org/{d}")) {
            let (abs, kw) = landing_meta(&html);
            merge(w, Work { abstract_note: abs, keywords: kw, ..Default::default() });
        }
    }
}

/// A DataCite DOI (Zenodo, figshare, arXiv…).
pub fn datacite(doi: &str) -> Result<Option<Work>> {
    let Some(v) = get_json(&format!("https://api.datacite.org/dois/{doi}"), &[])? else { return Ok(None) };
    Ok(v.get("data").and_then(|d| d.get("attributes")).and_then(parse_datacite))
}

pub fn parse_datacite(a: &Value) -> Option<Work> {
    let arr = |k: &str| a.get(k).and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let title = arr("titles").first().map(|t| str_at(t, "title").trim().to_string()).unwrap_or_default();
    if title.is_empty() {
        return None;
    }
    let authors = arr("creators")
        .iter()
        .map(|c| match str_at(c, "familyName") {
            "" => (String::new(), str_at(c, "name").to_string()),
            fam => (str_at(c, "givenName").to_string(), fam.to_string()),
        })
        .filter(|(_, l)| !l.is_empty())
        .collect();
    let descs = arr("descriptions");
    let abs = descs.iter().find(|d| str_at(d, "descriptionType") == "Abstract").or_else(|| descs.first()).map(|d| strip_tags(str_at(d, "description"))).unwrap_or_default();
    let kind = match a.get("types").map(|t| str_at(t, "resourceTypeGeneral")).unwrap_or_default() {
        "Preprint" => "preprint",
        "Report" => "report",
        "Book" => "book",
        "BookChapter" => "bookSection",
        "Dissertation" => "thesis",
        "ConferencePaper" => "conferencePaper",
        "Dataset" => "dataset",
        _ => "journalArticle",
    };
    let doi = str_at(a, "doi").to_lowercase();
    Some(Work {
        title,
        authors,
        year: a.get("publicationYear").and_then(|y| y.as_i64().or_else(|| y.as_str()?.parse().ok())).map(|y| y as i32),
        url: if doi.is_empty() { str_at(a, "url").to_string() } else { format!("https://doi.org/{doi}") },
        doi,
        venue: a.get("publisher").map(|p| p.as_str().map(str::to_string).unwrap_or_else(|| str_at(p, "name").to_string())).unwrap_or_default(),
        abstract_note: abs,
        item_type: kind.into(),
        citations: None,
        origin: "datacite".into(),
        keywords: dedup(arr("subjects").iter().map(|s| str_at(s, "subject").to_string()).filter(|s| !s.contains("FOS:")).collect()),
    })
}

/// Abstract and keywords declared in the `<meta>` tags of an article page. The
/// generic (short) description of a site is not taken for an abstract.
pub fn landing_meta(html: &str) -> (String, Vec<String>) {
    let tag = regex::Regex::new(r"(?is)<meta\s[^>]*>").unwrap();
    let attr = regex::Regex::new(r#"(?is)\b(name|property|content)\s*=\s*("([^"]*)"|'([^']*)')"#).unwrap();
    let mut metas: Vec<(String, String)> = Vec::new();
    for t in tag.find_iter(html) {
        let (mut name, mut content) = (String::new(), String::new());
        for c in attr.captures_iter(t.as_str()) {
            let v = c.get(3).or(c.get(4)).map(|m| m.as_str()).unwrap_or_default();
            if c[1].eq_ignore_ascii_case("content") {
                content = html_unescape(v);
            } else {
                name = v.to_lowercase();
            }
        }
        if !name.is_empty() && !content.trim().is_empty() {
            metas.push((name, content.trim().to_string()));
        }
    }
    let get = |names: &[&str], min: usize| names.iter().find_map(|n| metas.iter().find(|(k, v)| k == n && v.chars().count() >= min).map(|(_, v)| strip_tags(v)));
    let abs = get(&["citation_abstract", "dc.description", "dcterms.abstract", "dcterms.description", "eprints.abstract"], 80)
        .or_else(|| get(&["og:description", "description", "twitter:description"], 200))
        .unwrap_or_default();
    let abs = abs.trim_start_matches("Abstract").trim_start_matches([':', '.', ' ']).to_string();
    let kw = metas
        .iter()
        .filter(|(k, _)| k == "citation_keywords" || k == "keywords" || k == "dc.subject")
        .flat_map(|(_, v)| v.split([';', ',']).map(|s| s.trim().to_string()).collect::<Vec<_>>())
        .filter(|s| s.chars().count() > 2 && s.chars().count() < 60)
        .collect();
    (abs, dedup(kw))
}

fn html_unescape(s: &str) -> String {
    let named = s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&apos;", "'").replace("&nbsp;", " ");
    let num = regex::Regex::new(r"&#(x?)([0-9a-fA-F]+);").unwrap();
    num.replace_all(&named, |c: &regex::Captures| {
        let n = if c[1].is_empty() { c[2].parse().ok() } else { u32::from_str_radix(&c[2], 16).ok() };
        n.and_then(char::from_u32).map(String::from).unwrap_or_default()
    })
    .replace("&amp;", "&")
}

/// HTML page (at most 3 MB), following the DOI resolver's redirects.
fn get_text(url: &str) -> Result<String> {
    let agent = crate::net::agent(Duration::from_secs(20));
    let mut resp = agent
        .get(url)
        .header("accept", "text/html,application/xhtml+xml")
        .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Eloria/0.9")
        .call()
        .map_err(|e| Error::Net(e.to_string()))?;
    if resp.status().as_u16() >= 400 {
        return Err(Error::Net(format!("{url} a répondu {}", resp.status())));
    }
    resp.body_mut().with_config().limit(3_000_000).read_to_string().map_err(|e| Error::Net(e.to_string()))
}

/// Adds to `w` what it lacks, taken from `o` (the same article seen by another database).
pub fn merge(w: &mut Work, o: Work) {
    if w.abstract_note.trim().len() < 40 && o.abstract_note.trim().len() > w.abstract_note.trim().len() {
        w.abstract_note = o.abstract_note;
    }
    let mut k = std::mem::take(&mut w.keywords);
    k.extend(o.keywords);
    w.keywords = dedup(k);
    if w.title.is_empty() {
        w.title = o.title;
    }
    if w.authors.is_empty() {
        w.authors = o.authors;
    }
    if w.year.is_none() {
        w.year = o.year;
    }
    if w.venue.is_empty() {
        w.venue = o.venue;
    }
    if w.url.is_empty() {
        w.url = o.url;
    }
    if w.item_type.is_empty() {
        w.item_type = o.item_type;
    }
    w.citations = w.citations.max(o.citations);
}

fn get_json(url: &str, query: &[(&str, &str)]) -> Result<Option<Value>> {
    let agent = crate::net::agent(Duration::from_secs(30));
    let mut r = agent.get(url).header("accept", "application/json");
    for (k, v) in query {
        r = r.query(*k, *v);
    }
    let mut resp = r.call().map_err(|e| Error::Net(e.to_string()))?;
    let status = resp.status().as_u16();
    if status == 404 {
        return Ok(None);
    }
    if status == 429 {
        return Err(Error::Net("trop de requêtes, réessaie dans une minute".into()));
    }
    if status >= 400 {
        return Err(Error::Net(format!("{url} a répondu {status}")));
    }
    let v: Value = resp.body_mut().read_json().map_err(|e| Error::Net(e.to_string()))?;
    Ok(Some(v))
}

/// Metadata of a DOI (Crossref only).
pub fn crossref_doi(doi: &str) -> Result<Option<Work>> {
    let doi = normalize_doi(doi).ok_or_else(|| Error::Net(format!("DOI invalide : {doi}")))?;
    Ok(get_json(&format!("https://api.crossref.org/works/{doi}"), &[])?.and_then(|v| v.get("message").and_then(parse_crossref)))
}

/// Complete metadata of a DOI: Crossref (otherwise OpenAlex, which also knows DataCite
/// and Zenodo DOIs…), completed with abstract and keywords by [`enrich`].
pub fn lookup_doi(doi: &str) -> Result<Option<Work>> {
    let d = normalize_doi(doi).ok_or_else(|| Error::Net(format!("DOI invalide : {doi}")))?;
    let mut w = match crossref_doi(&d) {
        Ok(Some(w)) => w,
        other => match get_json(&format!("https://api.openalex.org/works/doi:{d}"), &[]).map(|v| v.as_ref().and_then(parse_openalex)) {
            Ok(Some(w)) => w,
            _ => match datacite(&d) {
                Ok(Some(w)) => w,
                // No database: Crossref's (network) error wins over "unknown".
                _ => return other,
            },
        },
    };
    enrich(&mut w);
    Ok(Some(w))
}

/// Free bibliographic search (title, authors, year) in Crossref.
pub fn crossref_search(q: &str, rows: usize) -> Result<Vec<Work>> {
    let rows = rows.clamp(1, 20).to_string();
    let v = get_json("https://api.crossref.org/works", &[("query.bibliographic", q), ("rows", &rows)])?;
    Ok(v.and_then(|v| v.get("message")?.get("items")?.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(parse_crossref)
        .collect())
}

/// References and citations of an article (OpenAlex), for tracking and the citation network.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CitationInfo {
    /// Identifiant OpenAlex (W…)
    pub openalex: String,
    pub cited_by_count: i64,
    /// What the article cites
    pub references: Vec<Work>,
    /// Who cites the article (most cited first)
    pub cited_by: Vec<Work>,
    /// DOIs of the references (all of them, even those not detailed)
    pub reference_dois: Vec<String>,
}

fn openalex_id(v: &Value) -> String {
    str_at(v, "id").rsplit('/').next().unwrap_or_default().to_string()
}

/// References and citations of a document, by DOI (otherwise by title).
pub fn citations(doi: Option<&str>, title: &str, limit: usize) -> Result<Option<CitationInfo>> {
    let w = match doi.and_then(normalize_doi) {
        Some(d) => get_json(&format!("https://api.openalex.org/works/doi:{d}"), &[])?,
        None => get_json("https://api.openalex.org/works", &[("search", title), ("per-page", "1")])?.and_then(|v| v.get("results")?.as_array()?.first().cloned()),
    };
    let Some(w) = w else { return Ok(None) };
    let id = openalex_id(&w);
    let refs: Vec<String> = w
        .get("referenced_works")
        .and_then(|r| r.as_array())
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(|s| s.rsplit('/').next().unwrap_or(s).to_string()))
        .collect();
    let lim = limit.clamp(1, 50).to_string();
    let mut info = CitationInfo { openalex: id.clone(), cited_by_count: w.get("cited_by_count").and_then(|c| c.as_i64()).unwrap_or(0), ..Default::default() };
    // Detailed references, in batches of 50 identifiers.
    for chunk in refs.chunks(50) {
        let filter = format!("openalex_id:{}", chunk.join("|"));
        if let Some(v) = get_json("https://api.openalex.org/works", &[("filter", &filter), ("per-page", "50")])? {
            info.references.extend(v.get("results").and_then(|r| r.as_array()).into_iter().flatten().filter_map(parse_openalex));
        }
    }
    info.reference_dois = info.references.iter().filter(|r| !r.doi.is_empty()).map(|r| r.doi.clone()).collect();
    info.references.sort_by(|a, b| b.citations.unwrap_or(0).cmp(&a.citations.unwrap_or(0)));
    if !id.is_empty() {
        let filter = format!("cites:{id}");
        if let Some(v) = get_json("https://api.openalex.org/works", &[("filter", &filter), ("per-page", &lim), ("sort", "cited_by_count:desc")])? {
            info.cited_by = v.get("results").and_then(|r| r.as_array()).into_iter().flatten().filter_map(parse_openalex).collect();
        }
    }
    Ok(Some(info))
}

/// Reliability of an article (OpenAlex: retraction, type, journal and its statistics).
/// An unknown DOI gives a record with `known: false`.
pub fn reliability(doi: &str, today: &str) -> Result<crate::reliability::Reliability> {
    let d = normalize_doi(doi).ok_or_else(|| Error::Net(format!("DOI invalide : {doi}")))?;
    let Some(w) = get_json(&format!("https://api.openalex.org/works/doi:{d}"), &[("select", "is_retracted,type,primary_location,cited_by_count,publication_year,open_access")])? else {
        return Ok(crate::reliability::Reliability { doi: d, checked: today.into(), ..Default::default() });
    };
    let src_id = w.get("primary_location").and_then(|l| l.get("source")).map(|x| str_at(x, "id").rsplit('/').next().unwrap_or_default().to_string()).unwrap_or_default();
    let source = if src_id.is_empty() { None } else { get_json(&format!("https://api.openalex.org/sources/{src_id}"), &[("select", "summary_stats,is_in_doaj,is_core,type,host_organization_name")]).ok().flatten() };
    Ok(crate::reliability::parse(&d, &w, source.as_ref(), today))
}

/// DOIs of an article's references (for the library citation network).
pub fn reference_dois(doi: &str) -> Result<Option<Vec<String>>> {
    let Some(d) = normalize_doi(doi) else { return Ok(None) };
    let Some(w) = get_json(&format!("https://api.openalex.org/works/doi:{d}"), &[])? else { return Ok(None) };
    let refs: Vec<String> = w
        .get("referenced_works")
        .and_then(|r| r.as_array())
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(|s| s.rsplit('/').next().unwrap_or(s).to_string()))
        .collect();
    let mut dois = Vec::new();
    for chunk in refs.chunks(50) {
        let filter = format!("openalex_id:{}", chunk.join("|"));
        if let Some(v) = get_json("https://api.openalex.org/works", &[("filter", &filter), ("per-page", "50"), ("select", "doi")])? {
            dois.extend(v.get("results").and_then(|r| r.as_array()).into_iter().flatten().filter_map(|x| {
                let d = str_at(x, "doi").trim_start_matches("https://doi.org/").to_lowercase();
                (!d.is_empty()).then_some(d)
            }));
        }
    }
    Ok(Some(dois))
}

const S2_FIELDS: &str = "title,year,authors,externalIds,url,venue,abstract,citationCount,fieldsOfStudy,s2FieldsOfStudy";

/// Articles close to a document: Semantic Scholar recommendations, then OpenAlex
/// related works if the first database does not answer. The document itself is excluded.
pub fn similar(doi: Option<&str>, title: &str, limit: usize) -> Result<Vec<Work>> {
    let lim = limit.clamp(1, 30).to_string();
    let doi = doi.and_then(normalize_doi);
    let mut errors = Vec::new();
    // 1. Semantic Scholar: by DOI, otherwise through the identifier found by title.
    let s2_id = match &doi {
        Some(d) => Some(format!("DOI:{d}")),
        None => match get_json("https://api.semanticscholar.org/graph/v1/paper/search", &[("query", title), ("limit", "1"), ("fields", "paperId")]) {
            Ok(v) => v.and_then(|v| v.get("data")?.as_array()?.first()?.get("paperId")?.as_str().map(str::to_string)),
            Err(e) => {
                errors.push(e.to_string());
                None
            }
        },
    };
    if let Some(id) = s2_id {
        match get_json(&format!("https://api.semanticscholar.org/recommendations/v1/papers/forpaper/{id}"), &[("fields", S2_FIELDS), ("limit", &lim)]) {
            Ok(Some(v)) => {
                let list: Vec<Work> = v.get("recommendedPapers").and_then(|a| a.as_array()).into_iter().flatten().filter_map(parse_s2).collect();
                if !list.is_empty() {
                    return Ok(exclude_self(list, doi.as_deref(), title));
                }
            }
            Ok(None) => {}
            Err(e) => errors.push(e.to_string()),
        }
    }
    // 2. OpenAlex: related works of the document.
    let w = match &doi {
        Some(d) => get_json(&format!("https://api.openalex.org/works/doi:{d}"), &[]),
        None => get_json("https://api.openalex.org/works", &[("search", title), ("per-page", "1")])
            .map(|v| v.and_then(|v| v.get("results")?.as_array()?.first().cloned())),
    };
    match w {
        Ok(Some(w)) => {
            let ids: Vec<String> = w
                .get("related_works")
                .and_then(|r| r.as_array())
                .into_iter()
                .flatten()
                .filter_map(|x| x.as_str().map(|s| s.rsplit('/').next().unwrap_or(s).to_string()))
                .take(limit.clamp(1, 30))
                .collect();
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            let filter = format!("openalex_id:{}", ids.join("|"));
            let v = get_json("https://api.openalex.org/works", &[("filter", &filter), ("per-page", &lim)])?;
            let list = v.and_then(|v| v.get("results")?.as_array().cloned()).unwrap_or_default().iter().filter_map(parse_openalex).collect();
            Ok(exclude_self(list, doi.as_deref(), title))
        }
        Ok(None) if errors.is_empty() => Ok(Vec::new()),
        Ok(None) => Err(Error::Net(errors.join(" ; "))),
        Err(e) => {
            errors.push(e.to_string());
            Err(Error::Net(errors.join(" ; ")))
        }
    }
}

fn exclude_self(list: Vec<Work>, doi: Option<&str>, title: &str) -> Vec<Work> {
    let t = title.trim().to_lowercase();
    list.into_iter().filter(|w| doi.map(|d| d != w.doi).unwrap_or(true) && w.title.trim().to_lowercase() != t).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalised_doi() {
        assert_eq!(normalize_doi("https://doi.org/10.1109/ACCESS.2024.0001").as_deref(), Some("10.1109/access.2024.0001"));
        assert_eq!(normalize_doi("doi: 10.1145/2939672.2939778.").as_deref(), Some("10.1145/2939672.2939778"));
        assert_eq!(normalize_doi("voir (10.1000/xyz123), p. 4").as_deref(), Some("10.1000/xyz123"));
        assert_eq!(normalize_doi("pas de doi ici"), None);
        assert_eq!(normalize_doi("10.12/"), None);
    }

    #[test]
    fn crossref_parsed() {
        let v = json!({
            "title": ["Graph <i>Neural</i> Networks for Fraud"],
            "author": [{"given": "Jane", "family": "Smith"}, {"name": "BCEAO"}],
            "issued": {"date-parts": [[2024, 3]]},
            "DOI": "10.1109/ACCESS.2024.0001", "URL": "https://doi.org/10.1109/access.2024.0001",
            "container-title": ["IEEE Access"], "type": "journal-article",
            "abstract": "<jats:p>Abstract We propose a GNN.</jats:p>", "is-referenced-by-count": 12
        });
        let w = parse_crossref(&v).unwrap();
        assert_eq!(w.title, "Graph Neural Networks for Fraud");
        assert_eq!(w.authors, vec![("Jane".into(), "Smith".into()), ("".into(), "BCEAO".into())]);
        assert_eq!((w.year, w.venue.as_str(), w.item_type.as_str()), (Some(2024), "IEEE Access", "journalArticle"));
        assert_eq!(w.abstract_note, "We propose a GNN.");
        assert_eq!(w.citations, Some(12));
        let it = w.to_item("smith2024graph");
        assert_eq!(it.creators[0].last_name, "Smith");
        assert_eq!(crate::zotero::generate_citekey(&it), "smith2024graph");
    }

    #[test]
    fn s2_and_openalex_parsed() {
        let s = json!({"title": "Why Should I Trust You?", "year": 2016, "authors": [{"name": "Marco Tulio Ribeiro"}],
            "externalIds": {"DOI": "10.1145/2939672.2939778"}, "venue": "KDD", "citationCount": 9000});
        let w = parse_s2(&s).unwrap();
        assert_eq!(w.authors[0], ("Marco Tulio".into(), "Ribeiro".into()));
        assert_eq!(w.url, "https://doi.org/10.1145/2939672.2939778");
        let o = json!({"id": "https://openalex.org/W1", "display_name": "SHAP", "publication_year": 2017,
            "doi": "https://doi.org/10.5555/3295222", "authorships": [{"author": {"display_name": "Scott Lundberg"}}],
            "primary_location": {"source": {"display_name": "NeurIPS"}}, "type": "article",
            "abstract_inverted_index": {"unified": [1], "A": [0], "approach": [2]}, "cited_by_count": 5});
        let w = parse_openalex(&o).unwrap();
        assert_eq!(w.abstract_note, "A unified approach");
        assert_eq!((w.doi.as_str(), w.venue.as_str()), ("10.5555/3295222", "NeurIPS"));
        assert_eq!(exclude_self(vec![w.clone()], Some("10.5555/3295222"), "x").len(), 0);
        assert_eq!(scholar_url("Graph & fraud"), "https://scholar.google.com/scholar?q=Graph+%26+fraud");
    }

    #[test]
    fn abstract_and_keywords_completed() {
        let o = json!({"display_name": "GNN fraud", "doi": "https://doi.org/10.1/x",
            "keywords": [{"display_name": "Mobile money", "score": 0.8}, {"display_name": "bruit", "score": 0.1}],
            "primary_topic": {"display_name": "Fraud Detection", "subfield": {"display_name": "Artificial Intelligence"}},
            "concepts": [{"display_name": "Computer science", "level": 0, "score": 0.9}, {"display_name": "Graph", "level": 2, "score": 0.5}],
            "abstract_inverted_index": {"We": [0], "detect": [1], "fraud": [2], "in": [3], "mobile": [4], "money": [5], "with": [6], "graph": [7], "networks.": [8]}});
        let oa = parse_openalex(&o).unwrap();
        assert_eq!(oa.keywords, vec!["Mobile money", "Fraud Detection", "Artificial Intelligence", "Graph"]);
        // Crossref without abstract or keywords: completed, without overwriting what it knew.
        let mut w = Work { title: "Graph Neural Networks for Fraud".into(), doi: "10.1/x".into(), year: Some(2024), keywords: vec!["mobile money".into()], ..Default::default() };
        merge(&mut w, oa);
        assert_eq!(w.title, "Graph Neural Networks for Fraud");
        assert_eq!(w.abstract_note, "We detect fraud in mobile money with graph networks.");
        assert_eq!(w.keywords, vec!["mobile money", "Fraud Detection", "Artificial Intelligence", "Graph"]);
        assert_eq!(w.to_item("k").tags.len(), 4);
        // DataCite (Zenodo) and the tags of a publisher page.
        let dc = parse_datacite(&json!({"doi": "10.5281/ZENODO.1", "titles": [{"title": "AML review"}], "publicationYear": 2026,
            "creators": [{"givenName": "Ana", "familyName": "Diallo"}], "publisher": "Zenodo",
            "descriptions": [{"descriptionType": "Other", "description": "x"}, {"descriptionType": "Abstract", "description": "<p>Federated learning for AML.</p>"}],
            "subjects": [{"subject": "FOS: Computer and information sciences"}, {"subject": "money laundering"}], "types": {"resourceTypeGeneral": "Preprint"}})).unwrap();
        assert_eq!((dc.abstract_note.as_str(), dc.item_type.as_str(), dc.year), ("Federated learning for AML.", "preprint", Some(2026)));
        assert_eq!(dc.keywords, vec!["money laundering"]);
        let html = r#"<head><meta name="description" content="Publisher site.">
            <meta name="citation_abstract" content="Abstract: Mobile money fraud is rising in &#233;conomies with weak KYC; we study 3 &amp; more countries.">
            <meta content="fraud; mobile money; KYC" name="citation_keywords"></head>"#;
        let (abs, kw) = landing_meta(html);
        assert_eq!(abs, "Mobile money fraud is rising in économies with weak KYC; we study 3 & more countries.");
        assert_eq!(kw, vec!["fraud", "mobile money", "KYC"]);
        assert_eq!(landing_meta(r#"<meta property="og:description" content="Read the latest articles.">"#).0, "");
        // A record sent by the interface without "keywords" is still readable.
        let back: Work = serde_json::from_value(json!({"title": "T", "authors": [], "doi": ""})).unwrap();
        assert!(back.keywords.is_empty());
    }
}
