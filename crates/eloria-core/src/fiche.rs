//! Reading notes generated from Zotero, and annotation sync.
//!
//! The annotation block is delimited by two HTML comments. Only this block is
//! regenerated on resync: everything the user writes elsewhere
//! in the note is preserved.

use minijinja::Environment;
use serde::Serialize;

use crate::lang::Lang;
use crate::zotero::{self, Annotation, Item};
use crate::Result;

pub const BLOCK_START: &str = "<!-- zotero:debut -->";
pub const BLOCK_END: &str = "<!-- zotero:fin -->";

/// Default template (French); `render_fiche` also accepts a custom template.
/// Front-matter keys stay the same in English: the Kanban and Thesis panel depend on them.
pub const DEFAULT_TEMPLATE: &str = r#"---
type: fiche
citekey: {{ citekey | y }}
zotero: {{ key | y }}
titre: {{ title | y }}
auteurs: {{ authors | y }}
annee: {{ year | y }}
publication: {{ publication | y }}
doi: {{ doi | y }}
{% if pdf_local %}pdf: {{ pdf_local | y }}
{% endif %}statut: à lire
priorite: moyenne
chapitres: []
tags: {{ fiche_tags | y }}
cree_le: {{ today }}
---
# {{ title }}

{% if authors_short %}**{{ authors_short }}{% if year %} ({{ year }}){% endif %}**{% elif year %}**{{ year }}**{% endif %}{% if publication %} · *{{ publication }}*{% endif %} · [Ouvrir dans Zotero]({{ select_link }}){% if pdf_local %} · [[{{ pdf_local }}|PDF]]{% elif pdf_link %} · [PDF]({{ pdf_link }}){% endif %}

> [!abstract]- Résumé
> {% if abstract %}{{ abstract | replace("\n", "\n> ") }}{% else %}(pas de résumé dans Zotero){% endif %}

## Problématique


## Méthode et données


## Résultats clés


## Limites


## Apport pour ma thèse


## Citations à réutiliser


{{ annotations_block }}
"#;

/// Default English template (same properties as the French one).
pub const DEFAULT_TEMPLATE_EN: &str = r#"---
type: fiche
citekey: {{ citekey | y }}
zotero: {{ key | y }}
titre: {{ title | y }}
auteurs: {{ authors | y }}
annee: {{ year | y }}
publication: {{ publication | y }}
doi: {{ doi | y }}
{% if pdf_local %}pdf: {{ pdf_local | y }}
{% endif %}statut: à lire
priorite: moyenne
chapitres: []
tags: {{ fiche_tags | y }}
cree_le: {{ today }}
---
# {{ title }}

{% if authors_short %}**{{ authors_short }}{% if year %} ({{ year }}){% endif %}**{% elif year %}**{{ year }}**{% endif %}{% if publication %} · *{{ publication }}*{% endif %} · [Open in Zotero]({{ select_link }}){% if pdf_local %} · [[{{ pdf_local }}|PDF]]{% elif pdf_link %} · [PDF]({{ pdf_link }}){% endif %}

> [!abstract]- Abstract
> {% if abstract %}{{ abstract | replace("\n", "\n> ") }}{% else %}(no abstract in Zotero){% endif %}

## Research problem


## Method and data


## Key findings


## Limitations


## Contribution to my thesis


## Quotes to reuse


{{ annotations_block }}
"#;

/// Default template for the chosen language.
pub fn default_template(lang: Lang) -> &'static str {
    match lang {
        Lang::Fr => DEFAULT_TEMPLATE,
        Lang::En => DEFAULT_TEMPLATE_EN,
    }
}

fn color_name(hex: &str) -> &'static str {
    match hex.to_lowercase().as_str() {
        "#ffd400" => "jaune",
        "#ff6666" => "rouge",
        "#5fb236" => "vert",
        "#2ea8e5" => "bleu",
        "#a28ae5" => "violet",
        "#e56eee" => "magenta",
        "#f19837" => "orange",
        "#aaaaaa" => "gris",
        _ => "jaune",
    }
}

fn quote(text: &str) -> String {
    text.trim().lines().map(|l| format!("> {}", l.trim_end())).collect::<Vec<_>>().join("\n")
}

/// Markdown rendering of an annotation (coloured callout).
pub fn render_annotation(a: &Annotation, lang: Lang) -> String {
    let page = if a.page_label.is_empty() { String::new() } else { format!("p. {} · ", a.page_label) };
    let open = lang.pick("ouvrir dans Zotero", "open in Zotero");
    let link = zotero::open_pdf_link(&a.attachment_key, Some(&a.page_label), Some(&a.key));
    let kind = match (a.kind.as_str(), lang) {
        ("note" | "text", _) => "note",
        ("image", _) => "image",
        ("ink", Lang::Fr) => "dessin",
        ("ink", Lang::En) => "ink",
        ("underline", Lang::Fr) => "souligné",
        ("underline", Lang::En) => "underline",
        (_, Lang::Fr) => "citation",
        (_, Lang::En) => "quote",
    };
    let mut s = format!("> [!{kind}|{}] {page}[{open}]({link})", color_name(&a.color));
    if !a.text.trim().is_empty() {
        s.push('\n');
        s.push_str(&quote(&a.text));
    } else if a.kind == "image" {
        s.push_str(lang.pick("\n> *(zone d'image, à consulter dans Zotero)*", "\n> *(image area, see it in Zotero)*"));
    }
    if !a.comment.trim().is_empty() {
        s.push_str(lang.pick("\n>\n> **Commentaire :** ", "\n>\n> **Comment:** "));
        let c = quote(&a.comment);
        s.push_str(c.trim_start_matches("> "));
    }
    if !a.tags.is_empty() {
        s.push_str(&format!(
            "\n> {}",
            a.tags.iter().map(|t| format!("#{}", t.replace(' ', "-"))).collect::<Vec<_>>().join(" ")
        ));
    }
    s
}

/// Whole block (markers included). The markers never change, whatever the language.
pub fn annotations_block(item: &Item, anns: &[Annotation], lang: Lang) -> String {
    let n = anns.len();
    let header = match lang {
        Lang::Fr => format!(
            "## Annotations Zotero\n\n*{n} annotation{s}, synchronisée{s} le {}. Ce bloc est régénéré à chaque synchronisation : écris tes notes au-dessus.*",
            chrono::Local::now().format("%d/%m/%Y %H:%M"),
            s = if n > 1 { "s" } else { "" }
        ),
        Lang::En => format!(
            "## Zotero annotations\n\n*{n} annotation{s}, synced on {}. This block is regenerated at each sync: write your notes above it.*",
            chrono::Local::now().format("%Y-%m-%d %H:%M"),
            s = if n == 1 { "" } else { "s" }
        ),
    };
    let mut s = format!("{BLOCK_START}\n{header}\n");
    let multi_pdf = item.attachments.iter().filter(|a| a.content_type.contains("pdf") || a.content_type.contains("epub")).count() > 1;
    let mut current_att = String::new();
    for a in anns {
        if multi_pdf && a.attachment_key != current_att {
            current_att = a.attachment_key.clone();
            let title = item.attachments.iter().find(|x| x.key == a.attachment_key).map(|x| x.title.as_str()).unwrap_or("Document");
            s.push_str(&format!("\n### {title}\n"));
        }
        s.push('\n');
        s.push_str(&render_annotation(a, lang));
        s.push('\n');
    }
    s.push_str(&format!("\n{BLOCK_END}"));
    s
}

/// Replaces the existing annotation block, or appends it at the end of the note.
pub fn replace_block(content: &str, block: &str) -> String {
    if let (Some(start), Some(end_rel)) = (content.find(BLOCK_START), content.find(BLOCK_END)) {
        if end_rel > start {
            let end = end_rel + BLOCK_END.len();
            return format!("{}{}{}", &content[..start], block, &content[end..]);
        }
    }
    let sep = if content.ends_with("\n\n") {
        ""
    } else if content.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    format!("{content}{sep}{block}\n")
}

#[derive(Serialize)]
struct Ctx<'a> {
    key: &'a str,
    citekey: &'a str,
    title: &'a str,
    authors: Vec<String>,
    authors_short: String,
    year: Option<i32>,
    publication: &'a str,
    doi: &'a str,
    url: &'a str,
    #[serde(rename = "abstract")]
    abstract_note: &'a str,
    item_type: &'a str,
    tags: &'a [String],
    /// Tags of the note: `fiche`, then the item's tags, normalised
    fiche_tags: Vec<String>,
    select_link: String,
    pdf_link: Option<String>,
    /// Copy of the PDF inside the vault (relative path), if any
    pdf_local: Option<&'a str>,
    today: String,
    annotations_block: String,
}

/// Renders a reading note from a template (or the language's default template).
/// `pdf_local`: copy of the PDF inside the vault, which the note links to.
pub fn render_fiche(item: &Item, anns: &[Annotation], template: Option<&str>, lang: Lang, pdf_local: Option<&str>) -> Result<String> {
    let mut env = Environment::new();
    env.set_trim_blocks(false);
    // JSON string (valid YAML), without the HTML escaping of `tojson`.
    env.add_filter("y", |v: minijinja::Value| serde_json::to_string(&v).unwrap_or_else(|_| "null".into()));
    env.add_template("fiche", template.unwrap_or(default_template(lang)))?;
    let pdf = item
        .attachments
        .iter()
        .find(|a| a.content_type.contains("pdf") || a.content_type.contains("epub"))
        .map(|a| zotero::open_pdf_link(&a.key, None, None));
    let c = Ctx {
        key: &item.key,
        citekey: &item.citekey,
        title: &item.title,
        authors: item.creators.iter().map(|c| c.display()).collect(),
        authors_short: zotero::authors_short(&item.creators),
        year: item.year,
        publication: &item.publication,
        doi: &item.doi,
        url: &item.url,
        abstract_note: &item.abstract_note,
        item_type: &item.item_type,
        tags: &item.tags,
        fiche_tags: fiche_tags(&item.tags),
        select_link: zotero::select_link(&item.key),
        pdf_link: pdf,
        pdf_local,
        today: chrono::Local::now().format("%Y-%m-%d").to_string(),
        annotations_block: annotations_block(item, anns, lang),
    };
    let out = env.get_template("fiche")?.render(minijinja::Value::from_serialize(&c))?;
    // If the custom template did not place the block, it is added.
    Ok(if out.contains(BLOCK_START) { out } else { replace_block(&out, &annotations_block(item, anns, lang)) })
}

/// Note tag from a Zotero tag: lower case, spaces to hyphens, characters
/// not allowed in tags removed ("Fraud Detection" and "fraud detection" give `fraud-detection`).
pub fn note_tag(zotero_tag: &str) -> Option<String> {
    let t: String = zotero_tag
        .trim()
        .trim_start_matches('#')
        .to_lowercase()
        .chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '/'))
        .collect();
    // "Computer Science - Machine Learning": a single hyphen between words.
    let mut t = t;
    while t.contains("--") {
        t = t.replace("--", "-");
    }
    let t = t.trim_matches(|c| c == '-' || c == '/').to_string();
    // A tag must start with a letter, a digit or _ and must not be a plain number.
    (!t.is_empty() && !t.chars().all(|c| c.is_ascii_digit()) && !t.starts_with('-')).then_some(t)
}

/// Tags of a reading note: `fiche`, then those of the item (normalised, no duplicates).
pub fn fiche_tags(zotero_tags: &[String]) -> Vec<String> {
    let mut out = vec!["fiche".to_string()];
    for t in zotero_tags.iter().filter_map(|t| note_tag(t)) {
        if !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// Sections of the "Eloria method" outline of a reading note, in order.
pub fn fiche_sections(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Fr => &[
            "Problématique",
            "Résumé tiré de l'article",
            "Mots-clés",
            "Objectifs",
            "Méthodes",
            "Sources de données",
            "Résultats",
            "Discussion des résultats",
            "Contraintes rencontrées par les auteurs",
            "Défis",
            "Perspectives",
            "Meilleures références citées",
            "Commentaire",
        ],
        Lang::En => &[
            "Research problem",
            "Abstract from the article",
            "Keywords",
            "Objectives",
            "Methods",
            "Data sources",
            "Results",
            "Discussion of the results",
            "Constraints faced by the authors",
            "Challenges",
            "Future work",
            "Key references cited",
            "Commentary",
        ],
    }
}

fn yaml_str(v: &str) -> serde_yaml::Value {
    serde_yaml::Value::String(v.to_string())
}

/// **Source** note: the item as it comes from Zotero. All metadata is in
/// the properties (abstract included), then the title and the annotation block: the note stands
/// on its own, even without Zotero. Eloria shows it like the Zotero panel view.
pub fn render_source(item: &Item, anns: &[Annotation], lang: Lang, pdf_local: Option<&str>, collections: &[String]) -> Result<String> {
    use serde_yaml::{Mapping, Value as Y};
    let mut m = Mapping::new();
    let mut put = |k: &str, v: Y| {
        m.insert(yaml_str(k), v);
    };
    put("type", yaml_str("source"));
    put("titre", yaml_str(&item.title));
    put("auteurs", Y::Sequence(item.creators.iter().map(|c| yaml_str(&c.display())).collect()));
    if let Some(y) = item.year {
        put("annee", Y::Number(y.into()));
    }
    for (k, v) in [("type_document", &item.item_type), ("publication", &item.publication), ("doi", &item.doi), ("url", &item.url)] {
        if !v.is_empty() {
            put(k, yaml_str(v));
        }
    }
    put("citekey", yaml_str(&item.citekey));
    if !item.key.is_empty() {
        put("zotero", yaml_str(&item.key));
    }
    if let Some(p) = pdf_local {
        put("pdf", yaml_str(p));
    }
    if !collections.is_empty() {
        put("collections", Y::Sequence(collections.iter().map(|c| yaml_str(c)).collect()));
    }
    if !item.abstract_note.trim().is_empty() {
        put("resume", yaml_str(item.abstract_note.trim()));
    }
    let mut tags = vec!["source".to_string()];
    tags.extend(fiche_tags(&item.tags).into_iter().skip(1));
    put("tags", Y::Sequence(tags.iter().map(|t| yaml_str(t)).collect()));
    put("ajoute_le", yaml_str(&chrono::Local::now().format("%Y-%m-%d").to_string()));
    // A source added without Zotero has no Zotero annotations to sync.
    let body = if item.key.is_empty() { format!("# {}\n", item.title) } else { format!("# {}\n\n{}\n", item.title, annotations_block(item, anns, lang)) };
    crate::markdown::with_frontmatter(&m, &body)
}

/// Item described by the properties of a source note (`type: source`): this is how
/// Eloria's library lives without Zotero (source view, reading notes, bibliography).
pub fn item_from_frontmatter(fm: &serde_json::Map<String, serde_json::Value>) -> Option<Item> {
    use serde_json::Value;
    if fm.get("type").and_then(|v| v.as_str()) != Some("source") {
        return None;
    }
    let s = |k: &str| match fm.get(k) {
        Some(Value::String(v)) => v.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    let citekey = s("citekey");
    if citekey.is_empty() {
        return None;
    }
    let list = |k: &str| -> Vec<String> {
        match fm.get(k) {
            Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
            Some(Value::String(v)) => v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
            _ => vec![],
        }
    };
    let creators = list("auteurs")
        .into_iter()
        .map(|a| {
            let (first, last) = match a.trim().rsplit_once(' ') {
                Some((f, l)) => (f.trim().to_string(), l.trim().to_string()),
                None => (String::new(), a.trim().to_string()),
            };
            zotero::Creator { creator_type: "author".into(), first_name: first, last_name: last }
        })
        .collect();
    let year = s("annee").parse::<i32>().ok();
    Some(Item {
        key: s("zotero"),
        item_type: if s("type_document").is_empty() { "journalArticle".into() } else { s("type_document") },
        title: if s("titre").is_empty() { citekey.clone() } else { s("titre") },
        creators,
        date: year.map(|y| y.to_string()).unwrap_or_default(),
        year,
        publication: s("publication"),
        doi: s("doi"),
        url: s("url"),
        abstract_note: s("resume"),
        citekey,
        tags: list("tags").into_iter().filter(|t| t != "source").collect(),
        ..Default::default()
    })
}

/// File name of a reading note attached to a source: `Fiche @key.md`.
pub fn fiche_page_name(item: &Item, lang: Lang) -> String {
    format!("{} @{}", lang.pick("Fiche", "Note"), crate::vault::sanitize_file_name(&item.citekey))
}

/// **Reading note** attached to a source: the title, the link to the source, then the empty
/// "Eloria method" outline, between the AI block markers (the "Eloria method" button
/// fills it; otherwise the user completes it by hand).
pub fn render_fiche_page(item: &Item, source_stem: &str, lang: Lang) -> Result<String> {
    use serde_yaml::{Mapping, Value as Y};
    let title = format!("{} : {}", lang.pick("Fiche", "Note"), item.title);
    let mut m = Mapping::new();
    let mut put = |k: &str, v: Y| {
        m.insert(yaml_str(k), v);
    };
    put("type", yaml_str("fiche"));
    put("titre", yaml_str(&title));
    put("source", yaml_str(&format!("[[{source_stem}]]")));
    put("citekey", yaml_str(&item.citekey));
    if !item.key.is_empty() {
        put("zotero", yaml_str(&item.key));
    }
    put("statut", yaml_str(lang.pick("à lire", "à lire")));
    put("priorite", yaml_str("moyenne"));
    put("chapitres", Y::Sequence(vec![]));
    put("tags", Y::Sequence(vec![yaml_str("fiche")]));
    put("cree_le", yaml_str(&chrono::Local::now().format("%Y-%m-%d").to_string()));
    let authors = zotero::authors_short(&item.creators);
    let year = item.year.map(|y| format!(" ({y})")).unwrap_or_default();
    let sections: String = fiche_sections(lang).iter().map(|s| format!("## {s}\n\n\n")).collect();
    let body = format!(
        "# {title}\n\n**{} :** [[{source_stem}|{}]]{}\n\n{}\n{sections}{}\n",
        lang.pick("Source", "Source"),
        item.title.replace('|', "-").replace(']', ")"),
        if authors.is_empty() { String::new() } else { format!(" · {authors}{year}") },
        crate::fiche_ia::IA_START,
        crate::fiche_ia::IA_END,
    );
    crate::markdown::with_frontmatter(&m, &body)
}

/// File name of a reading note: `@key.md`.
pub fn fiche_file_name(item: &Item) -> String {
    format!("@{}.md", crate::vault::sanitize_file_name(&item.citekey))
}

/// Rank of a reading status (à lire < en cours < lue < exploitée).
pub fn status_rank(s: &str) -> u8 {
    match s.trim() {
        "en cours" => 1,
        "lue" => 2,
        "exploitée" | "exploitee" => 3,
        _ => 0,
    }
}

/// Status that the content of a note justifies: "en cours" as soon as one section is
/// written, "lue" when three quarters are. `None`: the note is still empty.
pub fn reading_status(content: &str) -> Option<&'static str> {
    let (_, body, _) = crate::markdown::split_frontmatter(content);
    let secs = crate::matrix::sections(body);
    let total = secs.len();
    if total == 0 {
        return None;
    }
    let filled = secs.values().filter(|v| v.chars().filter(|c| c.is_alphanumeric()).count() >= 30).count();
    match filled {
        0 => None,
        n if n * 4 >= total * 3 => Some("lue"),
        _ => Some("en cours"),
    }
}

/// Note imported by the old template (before sources) and left empty: `type: fiche`,
/// annotation block, and nothing above it but the title, the record line, the
/// folded abstract and headings. The annotation block is regenerated from Zotero:
/// its content does not count. A single hand-written line is enough to keep it.
pub fn is_empty_legacy_import(content: &str) -> bool {
    let (fm, body, _) = crate::markdown::split_frontmatter(content);
    if !fm.unwrap_or_default().lines().any(|l| l.trim() == "type: fiche") || !body.contains(BLOCK_START) || body.contains(crate::fiche_ia::IA_START) {
        return false;
    }
    let before = &body[..body.find(BLOCK_START).unwrap_or(0)];
    before.lines().all(|l| {
        let l = l.trim();
        l.is_empty() || l.starts_with('#') || l.starts_with("**") || l.starts_with('>')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::parse_note;
    use crate::zotero::{Attachment, Creator};

    fn item() -> Item {
        Item {
            key: "K1".into(),
            item_type: "journalArticle".into(),
            title: "Titre \"spécial\": test".into(),
            creators: vec![Creator { creator_type: "author".into(), first_name: "A".into(), last_name: "Kaboré".into() }],
            year: Some(2023),
            citekey: "kabore2023titre".into(),
            abstract_note: "Ligne 1\nLigne 2".into(),
            attachments: vec![Attachment { key: "P1".into(), title: "PDF".into(), content_type: "application/pdf".into(), path: String::new() }],
            ..Default::default()
        }
    }

    fn ann(text: &str) -> Annotation {
        Annotation {
            key: "N1".into(),
            item_key: "K1".into(),
            attachment_key: "P1".into(),
            kind: "highlight".into(),
            text: text.into(),
            comment: "mon avis".into(),
            color: "#5fb236".into(),
            page_label: "7".into(),
            ..Default::default()
        }
    }

    #[test]
    fn tag_without_repeated_hyphens() {
        assert_eq!(note_tag("Computer Science - Machine Learning").as_deref(), Some("computer-science-machine-learning"));
    }

    #[test]
    fn valid_reading_note() {
        let out = render_fiche(&item(), &[ann("passage")], None, Lang::Fr, None).unwrap();
        let p = parse_note(&out);
        assert_eq!(p.frontmatter["citekey"], "kabore2023titre");
        assert_eq!(p.frontmatter["titre"], "Titre \"spécial\": test");
        assert_eq!(p.frontmatter["annee"], 2023);
        assert!(out.contains("> [!citation|vert] p. 7"));
        assert!(out.contains("**Commentaire :** mon avis"));
        assert!(out.contains("> Ligne 1\n> Ligne 2"));
        assert_eq!(fiche_file_name(&item()), "@kabore2023titre.md");
    }

    #[test]
    fn resync_preserves_the_rest() {
        let out = render_fiche(&item(), &[ann("v1")], None, Lang::Fr, None).unwrap();
        let edited = out.replace("## Limites\n", "## Limites\nMon texte perso.\n");
        let synced = replace_block(&edited, &annotations_block(&item(), &[ann("v2"), ann("v3")], Lang::Fr));
        assert!(synced.contains("Mon texte perso."));
        assert!(!synced.contains("> v1"));
        assert!(synced.contains("> v2") && synced.contains("> v3"));
        assert_eq!(synced.matches(BLOCK_START).count(), 1);
        // No block: appended at the end
        let added = replace_block("texte", "BLOC");
        assert_eq!(added, "texte\n\nBLOC\n");
    }

    #[test]
    fn reading_note_in_english() {
        let out = render_fiche(&item(), &[ann("passage")], None, Lang::En, Some("PDF/@kabore2023titre.pdf")).unwrap();
        let p = parse_note(&out);
        // Same properties as the French note (Kanban, Thesis panel).
        assert_eq!(p.frontmatter["type"], "fiche");
        assert_eq!(p.frontmatter["statut"], "à lire");
        assert!(out.contains("## Research problem"));
        assert!(out.contains("> [!abstract]- Abstract"));
        assert!(out.contains("> [!quote|vert] p. 7 · [open in Zotero]"));
        assert!(out.contains("**Comment:** mon avis"));
        assert!(out.contains("## Zotero annotations"));
        // Local copy of the PDF: property and link into the vault.
        assert_eq!(p.frontmatter["pdf"], "PDF/@kabore2023titre.pdf");
        assert!(out.contains("[[PDF/@kabore2023titre.pdf|PDF]]"));
        // The block markers do not depend on the language.
        assert!(out.contains(BLOCK_START) && out.contains(BLOCK_END));
    }

    #[test]
    fn item_tags() {
        let tags = vec!["Fraud Detection".to_string(), "fraud detection".into(), "Graph neural networks".into(), "2024".into(), " #XAI ".into()];
        assert_eq!(fiche_tags(&tags), vec!["fiche", "fraud-detection", "graph-neural-networks", "xai"]);
        let out = render_fiche(&Item { tags, ..item() }, &[], None, Lang::Fr, None).unwrap();
        let p = parse_note(&out);
        assert_eq!(p.tags, vec!["fiche", "fraud-detection", "graph-neural-networks", "xai"]);
    }

    #[test]
    fn reading_status_cases() {
        let vide = "---\ntype: fiche\n---\n# F\n\n## A\n\n## B\n\n## C\n\n## D\n";
        assert_eq!(reading_status(vide), None);
        let un = vide.replace("## A\n", "## A\nUne section écrite avec assez de mots pour compter.\n");
        assert_eq!(reading_status(&un), Some("en cours"));
        let trois = un.replace("## B\n", "## B\nUne deuxième section écrite avec assez de mots.\n").replace("## C\n", "## C\nUne troisième section écrite avec assez de mots.\n");
        assert_eq!(reading_status(&trois), Some("lue"));
        assert!(status_rank("exploitée") > status_rank("lue") && status_rank("à lire") == 0);
    }

    #[test]
    fn empty_legacy_import() {
        let vide = "---\ntype: fiche\nzotero: \"AB\"\n---\n# Titre\n\n**Smith (2024)** · *Revue*\n\n> [!abstract]- Résumé\n> Texte.\n\n## Problématique\n\n\n## Limites\n\n<!-- zotero:debut -->\n## Annotations Zotero\n\n> [!citation] p. 3\n> Passage.\n<!-- zotero:fin -->\n";
        assert!(is_empty_legacy_import(vide));
        // One hand-written line: the note is kept.
        assert!(!is_empty_legacy_import(&vide.replace("## Limites\n", "## Limites\nPas de données UEMOA.\n")));
        // A source, or a reading note of the new template, is never affected.
        assert!(!is_empty_legacy_import(&vide.replace("type: fiche", "type: source")));
        assert!(!is_empty_legacy_import(&vide.replace("## Problématique", "<!-- eloria:ia:debut -->\n## Problématique")));
    }

    #[test]
    fn source_and_reading_note() {
        let it = Item { tags: vec!["Fraud Detection".into()], url: "https://x.org".into(), ..item() };
        let s = render_source(&it, &[ann("passage")], Lang::Fr, Some("PDF/@kabore2023titre.pdf"), &["Thèse/Ch2".into()]).unwrap();
        let p = parse_note(&s);
        assert_eq!(p.frontmatter["type"], "source");
        assert_eq!(p.frontmatter["resume"], "Ligne 1\nLigne 2");
        assert_eq!(p.frontmatter["pdf"], "PDF/@kabore2023titre.pdf");
        assert_eq!(p.frontmatter["collections"][0], "Thèse/Ch2");
        assert_eq!(p.tags, vec!["source", "fraud-detection"]);
        assert!(s.contains("# Titre \"spécial\": test") && s.contains(BLOCK_START));

        let f = render_fiche_page(&it, "@kabore2023titre", Lang::Fr).unwrap();
        let p = parse_note(&f);
        assert_eq!(p.frontmatter["type"], "fiche");
        assert_eq!(p.frontmatter["source"], "[[@kabore2023titre]]");
        assert!(f.contains("[[@kabore2023titre|Titre \"spécial\": test]]"));
        // Complete, empty outline between the AI block markers.
        let start = f.find(crate::fiche_ia::IA_START).unwrap();
        for s in fiche_sections(Lang::Fr) {
            assert!(f[start..].contains(&format!("## {s}\n")), "{s}");
        }
        assert!(!f.contains(BLOCK_START), "les annotations restent dans la source");
        assert_eq!(fiche_page_name(&it, Lang::Fr), "Fiche @kabore2023titre");
    }

    #[test]
    fn no_authors_no_empty_bold() {
        let mut it = item();
        it.creators.clear();
        for lang in [Lang::Fr, Lang::En] {
            let out = render_fiche(&it, &[], None, lang, None).unwrap();
            assert!(!out.contains("** ("), "{out}");
            assert!(out.contains("**2023**"), "{out}");
        }
        it.year = None;
        let out = render_fiche(&it, &[], None, Lang::Fr, None).unwrap();
        assert!(!out.contains("****"), "{out}");
    }

    #[test]
    fn custom_template() {
        let out = render_fiche(&item(), &[], Some("# {{ citekey }} / {{ authors_short }}"), Lang::Fr, None).unwrap();
        assert!(out.starts_with("# kabore2023titre / Kaboré"));
        assert!(out.contains(BLOCK_START));
    }
}
