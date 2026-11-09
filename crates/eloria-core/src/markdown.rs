//! Parses a Markdown note: YAML front matter, `[[...]]` links, `#tag` tags,
//! `[@key]` citations, headings. Code blocks are ignored for links/tags.

use std::ops::Range;
use std::sync::OnceLock;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WikiLink {
    /// Target as written (without `#section` or `|alias`).
    pub target: String,
    pub heading: Option<String>,
    pub alias: Option<String>,
    /// Line (0-based) in the whole file.
    pub line: usize,
    /// `true` for a `![[...]]` embed.
    pub embed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ParsedNote {
    pub frontmatter: Map<String, Value>,
    /// Title: `titre`/`title` from the front matter, otherwise the first H1.
    pub title: Option<String>,
    pub aliases: Vec<String>,
    pub links: Vec<WikiLink>,
    pub tags: Vec<String>,
    pub citations: Vec<String>,
    pub headings: Vec<Heading>,
    pub word_count: usize,
    /// Body without the front matter.
    pub body: String,
}

fn wikilink_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(!?)\[\[([^\]\|#\n]*)(?:#([^\]\|\n]*))?(?:\|([^\]\n]*))?\]\]").unwrap())
}

fn tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|[\s(\[,;])#([\p{L}\p{N}_][\p{L}\p{N}_/\-]*)").unwrap())
}

fn citation_group_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[[^\[\]\n]*@[^\[\]\n]*\]").unwrap())
}

fn citekey_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|[\s\[;-])@([\p{L}\p{N}_][\p{L}\p{N}_:.#$%&+?<>~/\-]*[\p{L}\p{N}_])").unwrap())
}

/// Splits the front matter from the body. Returns `(yaml, body, body_line_offset)`.
pub fn split_frontmatter(content: &str) -> (Option<&str>, &str, usize) {
    let rest = if let Some(r) = content.strip_prefix("---\n") {
        r
    } else if let Some(r) = content.strip_prefix("---\r\n") {
        r
    } else {
        return (None, content, 0);
    };
    let header_len = content.len() - rest.len();
    let mut pos = 0usize;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" || trimmed == "..." {
            let yaml = &rest[..pos];
            let body_start = header_len + pos + line.len();
            let body = &content[body_start.min(content.len())..];
            let offset = content[..body_start.min(content.len())].matches('\n').count();
            return (Some(yaml), body, offset);
        }
        pos += line.len();
    }
    (None, content, 0)
}

/// Converts the front-matter YAML into a JSON object. Invalid YAML gives an empty object.
pub fn parse_frontmatter(yaml: &str) -> Map<String, Value> {
    if yaml.trim().is_empty() {
        return Map::new();
    }
    match serde_yaml::from_str::<serde_yaml::Value>(yaml) {
        Ok(v) => match serde_json::to_value(v) {
            Ok(Value::Object(m)) => m,
            _ => Map::new(),
        },
        Err(_) => Map::new(),
    }
}

/// Byte ranges of code blocks and inline code (to ignore).
fn code_ranges(body: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start: Option<usize> = None;
    for (ev, range) in Parser::new_ext(body, Options::all()).into_offset_iter() {
        match ev {
            Event::Start(Tag::CodeBlock(_)) => start = Some(range.start),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(s) = start.take() {
                    ranges.push(s..range.end);
                }
            }
            Event::Code(_) | Event::InlineMath(_) | Event::DisplayMath(_) => ranges.push(range),
            _ => {}
        }
    }
    ranges
}

fn in_ranges(pos: usize, ranges: &[Range<usize>]) -> bool {
    ranges.iter().any(|r| r.contains(&pos))
}

fn line_of(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].matches('\n').count()
}

fn value_to_string_list(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => s
            .split(',')
            .map(|t| t.trim().trim_start_matches('#').to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        Value::Array(a) => a
            .iter()
            .filter_map(|x| match x {
                Value::String(s) => Some(s.trim().trim_start_matches('#').to_string()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .filter(|t| !t.is_empty())
            .collect(),
        _ => vec![],
    }
}

/// Full parse of a note.
pub fn parse_note(content: &str) -> ParsedNote {
    let (yaml, body, offset) = split_frontmatter(content);
    let frontmatter = yaml.map(parse_frontmatter).unwrap_or_default();
    let codes = code_ranges(body);

    // Liens
    let mut links = Vec::new();
    for cap in wikilink_re().captures_iter(body) {
        let m = cap.get(0).unwrap();
        if in_ranges(m.start(), &codes) {
            continue;
        }
        let target = cap.get(2).map(|x| x.as_str().trim().to_string()).unwrap_or_default();
        let heading = cap.get(3).map(|x| x.as_str().trim().to_string()).filter(|s| !s.is_empty());
        if target.is_empty() && heading.is_none() {
            continue;
        }
        links.push(WikiLink {
            target,
            heading,
            alias: cap.get(4).map(|x| x.as_str().trim().to_string()).filter(|s| !s.is_empty()),
            line: offset + line_of(body, m.start()),
            embed: !cap.get(1).unwrap().as_str().is_empty(),
        });
    }

    // Tags: front matter + body (excluding code and "# " headings)
    let mut tags: Vec<String> = Vec::new();
    for key in ["tags", "tag"] {
        if let Some(v) = frontmatter.get(key) {
            tags.extend(value_to_string_list(v));
        }
    }
    for cap in tag_re().captures_iter(body) {
        let m = cap.get(1).unwrap();
        if in_ranges(m.start(), &codes) {
            continue;
        }
        let t = m.as_str().trim_end_matches(['/', '-']);
        // Excludes plain numbers (#1, #2024), which are rarely tags.
        if t.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        tags.push(t.to_string());
    }
    dedup_ci(&mut tags);

    // Citations [@key] and [see @key, p. 3; @other]
    let mut citations = Vec::new();
    for g in citation_group_re().find_iter(body) {
        if in_ranges(g.start(), &codes) {
            continue;
        }
        for c in citekey_re().captures_iter(g.as_str()) {
            citations.push(c[1].to_string());
        }
    }
    if let Some(Value::String(ck)) = frontmatter.get("citekey") {
        citations.push(ck.clone());
    }
    dedup_ci(&mut citations);

    // Titres
    let mut headings = Vec::new();
    let mut current: Option<(u8, usize, String)> = None;
    for (ev, range) in Parser::new_ext(body, Options::all()).into_offset_iter() {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some((heading_level(level), offset + line_of(body, range.start), String::new()))
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, _, s)) = current.as_mut() {
                    s.push_str(&t);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, line, text)) = current.take() {
                    headings.push(Heading { level, text: text.trim().to_string(), line });
                }
            }
            _ => {}
        }
    }

    let title = ["titre", "title"]
        .iter()
        .find_map(|k| frontmatter.get(*k).and_then(|v| v.as_str()).map(str::to_string))
        .or_else(|| headings.iter().find(|h| h.level == 1).map(|h| h.text.clone()));

    let aliases = frontmatter.get("aliases").map(value_to_string_list).unwrap_or_default();

    ParsedNote {
        word_count: count_words(body),
        frontmatter,
        title,
        aliases,
        links,
        tags,
        citations,
        headings,
        body: body.to_string(),
    }
}

fn heading_level(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn dedup_ci(v: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    v.retain(|s| seen.insert(s.to_lowercase()));
}

/// Word count of the body, excluding common Markdown syntax.
pub fn count_words(body: &str) -> usize {
    body.split_whitespace()
        .filter(|w| w.chars().any(|c| c.is_alphanumeric()))
        .count()
}

/// Replaces (or adds) a front-matter property and returns the new content.
/// `Value::Null` removes the property. The order of other keys is kept.
pub fn set_property(content: &str, key: &str, value: Value) -> Result<String> {
    let (yaml, body, _) = split_frontmatter(content);
    let mut map: serde_yaml::Mapping = match yaml {
        Some(y) if !y.trim().is_empty() => serde_yaml::from_str(y).unwrap_or_default(),
        _ => serde_yaml::Mapping::new(),
    };
    let k = serde_yaml::Value::String(key.to_string());
    if value.is_null() {
        map.remove(&k);
    } else {
        map.insert(k, serde_yaml::to_value(value)?);
    }
    with_frontmatter(&map, body)
}

/// Rebuilds a file from a YAML mapping and a body.
pub fn with_frontmatter(map: &serde_yaml::Mapping, body: &str) -> Result<String> {
    if map.is_empty() {
        return Ok(body.to_string());
    }
    let yaml = serde_yaml::to_string(map)?;
    let body = body.strip_prefix('\n').unwrap_or(body);
    Ok(format!("---\n{}---\n{}", yaml, if body.is_empty() { String::new() } else { format!("\n{body}") }))
}

/// Replaces in `content` the links pointing to `old` (case-insensitive
/// comparison) with `new`, keeping `#section` and `|alias`.
pub fn rewrite_links(content: &str, old: &str, new: &str) -> (String, usize) {
    let (_, body, _) = split_frontmatter(content);
    let body_start = content.len() - body.len();
    let codes = code_ranges(body);
    let old_l = old.to_lowercase();
    let mut count = 0;
    let mut out = String::with_capacity(content.len());
    out.push_str(&content[..body_start]);
    let mut last = 0;
    for cap in wikilink_re().captures_iter(body) {
        let m = cap.get(0).unwrap();
        let t = cap.get(2).unwrap();
        if in_ranges(m.start(), &codes) || t.as_str().trim().to_lowercase() != old_l {
            continue;
        }
        out.push_str(&body[last..t.start()]);
        out.push_str(new);
        last = t.end();
        count += 1;
    }
    out.push_str(&body[last..]);
    (out, count)
}

/// HTML rendering of Markdown text (used by export).
pub fn to_html(markdown: &str) -> String {
    let (_, body, _) = split_frontmatter(markdown);
    // Internal links become text (the alias if any).
    let body = wikilink_re().replace_all(body, |c: &regex::Captures| {
        c.get(4)
            .or(c.get(2))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    });
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, Parser::new_ext(&body, Options::all()));
    html
}

/// Section of a note: from the `heading` (case-insensitive) up to the next heading
/// of the same or a higher level. Heading included.
pub fn section(content: &str, heading: &str) -> Option<String> {
    let (_, body, _) = split_frontmatter(content);
    let want = heading.trim().to_lowercase();
    let mut out = String::new();
    let mut level = 0usize;
    let mut in_code = false;
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
        }
        let hashes = if in_code { 0 } else { t.chars().take_while(|c| *c == '#').count() };
        let is_heading = (1..=6).contains(&hashes) && t[hashes..].starts_with(' ');
        if level > 0 {
            if is_heading && hashes <= level {
                break;
            }
            out.push_str(line);
            out.push('\n');
        } else if is_heading && t[hashes..].trim().to_lowercase() == want {
            level = hashes;
            out.push_str(line);
            out.push('\n');
        }
    }
    (level > 0).then_some(out)
}

/// HTML of an excerpt embedded in another note (`![[Note]]`): the note's raw HTML
/// is shown as text (never executed), links and embeds become text,
/// the Zotero and AI block markers disappear.
pub fn to_safe_html(markdown: &str) -> String {
    use pulldown_cmark::{CowStr, Event};
    let (_, body, _) = split_frontmatter(markdown);
    let body = body.replace("![[", "[[");
    let body = wikilink_re().replace_all(&body, |c: &regex::Captures| c.get(4).or(c.get(2)).map(|m| m.as_str().to_string()).unwrap_or_default());
    let body: String = body.lines().filter(|l| !l.trim_start().starts_with("<!-- zotero:") && !l.trim_start().starts_with("<!-- eloria:")).collect::<Vec<_>>().join("\n");
    let events = Parser::new_ext(&body, Options::all()).map(|e| match e {
        Event::Html(h) | Event::InlineHtml(h) => Event::Text(CowStr::from(h.to_string())),
        e => e,
    });
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events);
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_and_embed() {
        let n = "---\ntitre: T\n---\n# T\nIntro\n## Méthode\nGNN sur graphe.\n```\n# pas un titre\n```\n### Détail\nX\n## Résultats\nY\n";
        let s = section(n, "méthode").unwrap();
        assert!(s.starts_with("## Méthode") && s.contains("# pas un titre") && s.contains("### Détail") && !s.contains("Résultats"));
        assert!(section(n, "absent").is_none());
        let h = to_safe_html("Voir [[A|alias]] et ![[B]].\n\n<script>alert(1)</script>\n<!-- eloria:ia:debut -->\n");
        assert!(h.contains("alias") && h.contains("&lt;script&gt;") && !h.contains("<script>") && !h.contains("eloria:ia"));
    }

    const NOTE: &str = "---\ntitre: Ma fiche\ntags: [these, gnn]\naliases: [Fiche GNN]\ncitekey: smith2024\n---\n# Grand titre\n\nVoir [[Concept A]] et [[dossier/Concept B#Def|B]].\nTag #fraude et #these/ch2. Pas #123.\n\n```\n[[pas un lien]] #pasuntag\n```\n\nComme dit [@doe2020, p. 3; @roe2021].\n![[image.png]]\n";

    #[test]
    fn full_parse() {
        let p = parse_note(NOTE);
        assert_eq!(p.title.as_deref(), Some("Ma fiche"));
        assert_eq!(p.aliases, vec!["Fiche GNN"]);
        let targets: Vec<_> = p.links.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(targets, vec!["Concept A", "dossier/Concept B", "image.png"]);
        assert_eq!(p.links[1].heading.as_deref(), Some("Def"));
        assert_eq!(p.links[1].alias.as_deref(), Some("B"));
        assert!(p.links[2].embed);
        assert_eq!(p.links[0].line, 8);
        assert_eq!(p.tags, vec!["these", "gnn", "fraude", "these/ch2"]);
        assert_eq!(p.citations, vec!["doe2020", "roe2021", "smith2024"]);
        assert_eq!(p.headings[0].text, "Grand titre");
        assert_eq!(p.headings[0].line, 6);
    }

    #[test]
    fn without_front_matter() {
        let p = parse_note("Bonjour [[X]]");
        assert!(p.frontmatter.is_empty());
        assert_eq!(p.links[0].target, "X");
        assert_eq!(p.title, None);
    }

    #[test]
    fn property() {
        let c = set_property("---\na: 1\nstatut: à lire\n---\ncorps\n", "statut", Value::String("lue".into())).unwrap();
        assert!(c.contains("statut: lue"));
        assert!(c.starts_with("---\na: 1\n"));
        assert!(c.ends_with("corps\n"));
        let c2 = set_property("corps", "x", serde_json::json!(["a", "b"])).unwrap();
        assert!(c2.starts_with("---\nx:\n- a\n- b\n---\n"));
        let c3 = set_property(&c, "a", Value::Null).unwrap();
        assert!(!c3.contains("a: 1"));
    }

    #[test]
    fn link_rewriting() {
        let (out, n) = rewrite_links("[[Vieux]] et [[vieux#S|al]] et `[[Vieux]]` et [[Autre]]", "Vieux", "Neuf");
        assert_eq!(n, 2);
        assert_eq!(out, "[[Neuf]] et [[Neuf#S|al]] et `[[Vieux]]` et [[Autre]]");
    }

    #[test]
    fn html() {
        let h = to_html("# T\n[[A|alias]] **gras**");
        assert!(h.contains("<h1>T</h1>"));
        assert!(h.contains("alias"));
        assert!(h.contains("<strong>gras</strong>"));
    }
}
