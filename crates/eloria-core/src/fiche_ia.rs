//! "Eloria method" reading note: a full analysis of an article by the AI.
//!
//! The AI receives the PDF (Anthropic) or the full text, the metadata and the user's
//! annotations. It returns a Markdown analysis ending with a JSON block (tags,
//! keywords, cited references). Eloria then checks each reference:
//! - present in the library: `[@key]` link;
//! - found in the text of the article: kept, marked "cited by the article";
//! - not found: never presented as true, filed under "To check".
//!
//! The result is inserted between two markers in the note: the user's text and
//! the Zotero annotation block are never modified.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use crate::lang::Lang;
use crate::zotero::{Annotation, Item};

pub const IA_START: &str = "<!-- eloria:ia:debut -->";
pub const IA_END: &str = "<!-- eloria:ia:fin -->";

/// Reference cited by the article, as reported by the AI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CitedRef {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub authors: String,
    #[serde(default)]
    pub year: Option<i64>,
    #[serde(default)]
    pub doi: String,
    /// Why it matters (one sentence)
    #[serde(default)]
    pub why: String,
}

/// Result of checking a reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RefCheck {
    /// In the library: citation key
    InLibrary { citekey: String },
    /// Found in the text of the article
    InArticle,
    /// Not found: to check
    Unverified,
}

/// Parts extracted from the AI's answer.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Extract {
    pub tags: Vec<String>,
    pub keywords: Vec<String>,
    pub references: Vec<CitedRef>,
}

/// "Eloria method" analysis, ready to be reviewed then saved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiFicheDraft {
    pub item_key: String,
    pub citekey: String,
    /// Whole block to insert into the note (markers included)
    pub block: String,
    pub tags: Vec<String>,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// What the AI read: "pdf", "texte" or "résumé"
    pub source: String,
    pub refs_in_library: usize,
    pub refs_in_article: usize,
    pub refs_unverified: usize,
    /// The fallback model answered after a refusal
    pub fell_back: bool,
    /// Response cut by the length limit
    pub truncated: bool,
}

/// Prepared analysis, ready to be sent to the AI (no reference to the open vault).
pub struct Job {
    pub request: crate::ai::AiRequest,
    pub config: crate::ai::AiConfig,
    pub item: Item,
    /// Library, to check cited references
    pub library: Vec<Item>,
    pub fulltext: Option<String>,
    pub source: String,
    pub lang: Lang,
    /// For the request log
    pub vault: crate::vault::Vault,
}

impl Job {
    /// Calls the AI, checks references and builds the note block.
    pub fn run(self, api_key: Option<&str>, on_delta: &mut dyn FnMut(&str), cancel: &std::sync::atomic::AtomicBool) -> crate::Result<AiFicheDraft> {
        use crate::ai;
        let res = ai::complete(&self.config, api_key, &self.request, on_delta, cancel);
        let _ = ai::log(
            &self.vault,
            &ai::LogEntry {
                date: chrono::Local::now().to_rfc3339(),
                task: "fiche".into(),
                provider: self.config.provider.code().into(),
                model: res.as_ref().map(|r| r.model.clone()).unwrap_or(self.config.model.clone()),
                input_tokens: res.as_ref().map(|r| r.input_tokens).unwrap_or(0),
                output_tokens: res.as_ref().map(|r| r.output_tokens).unwrap_or(0),
                ok: res.is_ok(),
            },
        );
        let res = res?;
        let (analysis, extract) = split_response(&res.text);
        let checked: Vec<_> =
            extract.references.iter().map(|r| (r.clone(), check_ref(r, &self.library, self.fulltext.as_deref()))).collect();
        let count = |f: fn(&RefCheck) -> bool| checked.iter().filter(|(_, c)| f(c)).count();
        let refs = references_section(&checked, self.lang, self.fulltext.is_some());
        let mut tags: Vec<String> = Vec::new();
        for t in extract.tags.iter().filter_map(|t| crate::fiche::note_tag(t)) {
            if !tags.contains(&t) {
                tags.push(t);
            }
        }
        Ok(AiFicheDraft {
            item_key: self.item.key.clone(),
            citekey: self.item.citekey.clone(),
            block: fiche_content(&analysis, &refs, &res.model, self.lang),
            tags,
            model: res.model.clone(),
            input_tokens: res.input_tokens,
            output_tokens: res.output_tokens,
            source: self.source,
            refs_in_library: count(|c| matches!(c, RefCheck::InLibrary { .. })),
            refs_in_article: count(|c| matches!(c, RefCheck::InArticle)),
            refs_unverified: count(|c| matches!(c, RefCheck::Unverified)),
            fell_back: res.fell_back,
            truncated: res.truncated,
        })
    }
}

/// System prompt of the Eloria method. `topic`: the user's research topic
/// (setting), so that the commentary judges how relevant the article is to this work.
pub fn system_prompt(lang: Lang, topic: &str) -> String {
    let topic = topic.trim();
    match lang {
        Lang::Fr => SYSTEM_FR.replace("{CONTEXTE}", &if topic.is_empty() { String::new() } else { format!(", et intérêt pour la recherche du doctorant : {topic}") }),
        Lang::En => SYSTEM_EN.replace("{CONTEXT}", &if topic.is_empty() { String::new() } else { format!(", and relevance for the student's research: {topic}") }),
    }
}

const SYSTEM_FR: &str = r#"Tu es l'assistant de recherche d'Eloria. Tu rédiges, en français, la fiche de lecture « méthode Eloria » d'un article scientifique pour un doctorant.

Règles absolues :
- Tu ne t'appuies que sur l'article fourni (PDF ou texte) et sur ses métadonnées. N'invente rien : ni chiffre, ni jeu de données, ni lien, ni référence. Si une information est absente de l'article, écris-le (« non précisé dans l'article »).
- Les références que tu cites doivent figurer dans la bibliographie de l'article, recopiées exactement (titre, auteurs, année, DOI s'il y figure). Ne complète jamais une référence de mémoire.
- Pas de tiret cadratin.

Structure à suivre exactement (titres de niveau 3, dans cet ordre) :
### Problématique
### Résumé tiré de l'article
(le résumé de l'article recopié mot pour mot, dans un encadré > [!résumé]- Résumé de l'article ; si le résumé est fourni dans les métadonnées, reprends-le tel quel)
### Mots-clés
### Objectifs
### Méthodes
(démarche, modèles, algorithmes, protocole ; un diagramme Mermaid flowchart LR du pipeline si cela l'éclaire)
### Sources de données
(jeux de données, origine, taille, période, accès ; liens seulement s'ils figurent dans l'article)
### Résultats
(chiffres clés ; un tableau Markdown pour les comparaisons de performance)
### Discussion des résultats
### Contraintes rencontrées par les auteurs
### Défis
### Perspectives
### Commentaire
(2 ou 3 paragraphes : apport réel de l'étude, solidité de la méthode, limites{CONTEXTE})

Mise en forme : Markdown soigné, **gras** pour les notions clés, *italique* pour les termes techniques ou étrangers, ==surlignage== pour les résultats ou phrases essentiels, encadrés Obsidian (> [!astuce], > [!attention], > [!definition]) quand ils aident. Ne crée pas de section « Références » : Eloria la construit à partir du bloc JSON.

Termine par un bloc de code ```json contenant exactement :
{"tags": ["5 à 10 tags courts en minuscules avec tirets"], "keywords": ["mots-clés de l'article"], "references": [{"title": "", "authors": "", "year": 2020, "doi": "", "why": "une phrase : pourquoi cette référence compte"}]}
avec au plus 8 références : les plus importantes de la bibliographie de l'article."#;

const SYSTEM_EN: &str = r#"You are Eloria's research assistant. You write, in English, the "Eloria method" reading note of a scientific article for a PhD student.

Absolute rules:
- Rely only on the provided article (PDF or text) and its metadata. Invent nothing: no figure, dataset, link or reference. If something is missing from the article, say so ("not stated in the article").
- The references you cite must appear in the article's bibliography, copied exactly (title, authors, year, DOI if present). Never complete a reference from memory.
- No em dashes.

Follow this structure exactly (level-3 headings, in this order):
### Research problem
### Abstract from the article
(the article's abstract copied word for word, inside a > [!abstract]- Abstract callout; if the abstract is given in the metadata, reuse it as is)
### Keywords
### Objectives
### Methods
(approach, models, algorithms, protocol; a Mermaid flowchart LR of the pipeline when it helps)
### Data sources
(datasets, origin, size, period, access; links only if they appear in the article)
### Results
(key figures; a Markdown table for performance comparisons)
### Discussion of the results
### Constraints faced by the authors
### Challenges
### Future work
### Commentary
(2 or 3 paragraphs: real contribution, soundness of the method, limits{CONTEXT})

Formatting: careful Markdown, **bold** for key notions, *italics* for technical or foreign terms, ==highlights== for essential results or sentences, Obsidian callouts (> [!tip], > [!warning], > [!definition]) when they help. Do not write a "References" section: Eloria builds it from the JSON block.

End with a ```json code block containing exactly:
{"tags": ["5 to 10 short lowercase hyphenated tags"], "keywords": ["the article's keywords"], "references": [{"title": "", "authors": "", "year": 2020, "doi": "", "why": "one sentence: why this reference matters"}]}
with at most 8 references: the most important ones from the article's bibliography."#;

/// User message: metadata, Zotero abstract, the user's annotations.
pub fn user_prompt(item: &Item, anns: &[Annotation], lang: Lang) -> String {
    let authors = item.creators.iter().map(|c| c.display()).collect::<Vec<_>>().join(", ");
    let mut s = String::new();
    s.push_str(lang.pick("Article à analyser.\n\n", "Article to analyse.\n\n"));
    s.push_str(&format!("{} : {}\n", lang.pick("Titre", "Title"), item.title));
    if !authors.is_empty() {
        s.push_str(&format!("{} : {authors}\n", lang.pick("Auteurs", "Authors")));
    }
    if let Some(y) = item.year {
        s.push_str(&format!("{} : {y}\n", lang.pick("Année", "Year")));
    }
    if !item.publication.is_empty() {
        s.push_str(&format!("Publication : {}\n", item.publication));
    }
    if !item.doi.is_empty() {
        s.push_str(&format!("DOI : {}\n", item.doi));
    }
    if !item.abstract_note.is_empty() {
        s.push_str(&format!("\n{} :\n{}\n", lang.pick("Résumé (métadonnées Zotero)", "Abstract (Zotero metadata)"), item.abstract_note));
    }
    let marked: Vec<&Annotation> = anns.iter().filter(|a| !a.text.trim().is_empty() || !a.comment.trim().is_empty()).collect();
    if !marked.is_empty() {
        s.push_str(lang.pick(
            "\nPassages surlignés et commentaires du doctorant (à prendre en compte dans le commentaire) :\n",
            "\nPassages highlighted and comments by the PhD student (take them into account in the commentary):\n",
        ));
        for a in marked.iter().take(60) {
            let page = if a.page_label.is_empty() { String::new() } else { format!(" (p. {})", a.page_label) };
            if !a.text.trim().is_empty() {
                s.push_str(&format!("- « {} »{page}", a.text.trim()));
            } else {
                s.push_str(&format!("- {}{page}", lang.pick("(note)", "(note)")));
            }
            if !a.comment.trim().is_empty() {
                s.push_str(&format!(" : {}", a.comment.trim()));
            }
            s.push('\n');
        }
    }
    s.push_str(lang.pick("\nRédige la fiche « méthode Eloria ».", "\nWrite the \"Eloria method\" reading note."));
    s
}

/// Separates the Markdown analysis from the final JSON block.
pub fn split_response(text: &str) -> (String, Extract) {
    let start = text.rfind("```json");
    let Some(start) = start else { return (text.trim().to_string(), Extract::default()) };
    let after = &text[start + 7..];
    let end = after.find("```").unwrap_or(after.len());
    let json = after[..end].trim();
    let extract = serde_json::from_str::<Value>(json)
        .ok()
        .map(|v| Extract {
            tags: strings(&v["tags"]),
            keywords: strings(&v["keywords"]),
            references: serde_json::from_value(v["references"].clone()).unwrap_or_default(),
        })
        .unwrap_or_default();
    (text[..start].trim().to_string(), extract)
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// Normalised text to compare titles: lower case, no accents or punctuation.
pub fn normalize(s: &str) -> String {
    s.nfd()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Checks a reference against the library, then against the text of the article.
pub fn check_ref(r: &CitedRef, library: &[Item], fulltext: Option<&str>) -> RefCheck {
    let doi = r.doi.trim().trim_start_matches("https://doi.org/").to_lowercase();
    let title = normalize(&r.title);
    for it in library {
        let same_doi = !doi.is_empty() && it.doi.trim().to_lowercase() == doi;
        let same_title = title.len() >= 12 && normalize(&it.title) == title;
        if same_doi || same_title {
            return RefCheck::InLibrary { citekey: it.citekey.clone() };
        }
    }
    if let Some(text) = fulltext {
        let hay = normalize(text);
        // The title (or its first 60 characters, bibliographies sometimes cut it) or the DOI.
        let probe: String = title.chars().take(60).collect();
        if (probe.len() >= 12 && hay.contains(&probe)) || (!doi.is_empty() && text.to_lowercase().contains(&doi)) {
            return RefCheck::InArticle;
        }
    }
    RefCheck::Unverified
}

/// "Key references cited" section, built by Eloria after checking.
pub fn references_section(refs: &[(CitedRef, RefCheck)], lang: Lang, has_fulltext: bool) -> String {
    let mut ok = String::new();
    let mut doubt = String::new();
    for (r, c) in refs {
        let year = r.year.map(|y| format!(" ({y})")).unwrap_or_default();
        let authors = if r.authors.is_empty() { String::new() } else { format!("{}{year}. ", r.authors.trim()) };
        let doi = if r.doi.trim().is_empty() { String::new() } else { format!(" [DOI](https://doi.org/{})", r.doi.trim().trim_start_matches("https://doi.org/")) };
        let why = if r.why.trim().is_empty() { String::new() } else { format!(" : {}", r.why.trim()) };
        match c {
            RefCheck::InLibrary { citekey } => {
                ok.push_str(&format!("- {authors}*{}* [@{citekey}]{why}\n", r.title.trim()));
            }
            RefCheck::InArticle => {
                ok.push_str(&format!("- {authors}*{}*{doi}{why} {}\n", r.title.trim(), lang.pick("(citée par l'article)", "(cited by the article)")));
            }
            RefCheck::Unverified => {
                doubt.push_str(&format!("- {authors}*{}*{doi}\n", r.title.trim()));
            }
        }
    }
    let mut s = format!("### {}\n\n", lang.pick("Meilleures références citées", "Key references cited"));
    if ok.is_empty() {
        s.push_str(lang.pick("*Aucune référence vérifiée.*\n", "*No verified reference.*\n"));
    } else {
        s.push_str(&ok);
    }
    if !doubt.is_empty() {
        let why = if has_fulltext {
            lang.pick("introuvables dans ta bibliothèque et dans le texte de l'article", "found neither in your library nor in the article's text")
        } else {
            lang.pick("texte intégral indisponible pour les vérifier", "full text unavailable to check them")
        };
        s.push_str(&format!(
            "\n> [!attention]- {} ({why})\n{}",
            lang.pick("À vérifier", "To check"),
            doubt.lines().map(|l| format!("> {l}\n")).collect::<String>()
        ));
    }
    s
}

/// Whole block to insert into the note (markers included).
pub fn block(analysis: &str, refs_section: &str, model: &str, lang: Lang) -> String {
    let date = chrono::Local::now().format("%Y-%m-%d %H:%M");
    let head = match lang {
        Lang::Fr => format!("## Analyse « méthode Eloria »\n\n*Générée par l'IA ({model}) le {date}, à relire. Ce bloc est remplacé si tu régénères l'analyse : écris tes propres notes ailleurs dans la fiche.*"),
        Lang::En => format!("## \"Eloria method\" analysis\n\n*Generated by AI ({model}) on {date}, to be reviewed. This block is replaced if you regenerate the analysis: write your own notes elsewhere in the note.*"),
    };
    format!("{IA_START}\n{head}\n\n{}\n\n{}\n{IA_END}", analysis.trim(), refs_section.trim_end())
}

/// Content of the AI-written "Eloria method" note, markers included: section headings
/// become level 2 (as in the note's empty outline), and checked
/// references go before the final commentary.
pub fn fiche_content(analysis: &str, refs_section: &str, model: &str, lang: Lang) -> String {
    let date = chrono::Local::now().format("%Y-%m-%d %H:%M");
    let to_h2 = |s: &str| s.lines().map(|l| l.strip_prefix("### ").map(|r| format!("## {r}")).unwrap_or_else(|| l.to_string())).collect::<Vec<_>>().join("\n");
    let analysis = to_h2(analysis.trim());
    let refs = to_h2(refs_section.trim());
    let comment = format!("\n## {}", lang.pick("Commentaire", "Commentary"));
    let body = match analysis.find(&comment) {
        Some(i) => format!("{}\n\n{refs}\n{}", analysis[..i].trim_end(), &analysis[i..]),
        None => format!("{analysis}\n\n{refs}"),
    };
    let note = match lang {
        Lang::Fr => format!("*Fiche rédigée par l'IA ({model}) le {date}, à relire. Une version précédente est gardée dans l'historique.*"),
        Lang::En => format!("*Note written by AI ({model}) on {date}, to be reviewed. The previous version is kept in the history.*"),
    };
    format!("{IA_START}\n{note}\n\n{}\n{IA_END}", body.trim())
}

/// Inserts (or replaces) the AI block: before the annotation block if any, otherwise at the end.
pub fn insert_block(content: &str, block: &str) -> String {
    if let (Some(s), Some(e)) = (content.find(IA_START), content.find(IA_END)) {
        if e > s {
            return format!("{}{}{}", &content[..s], block, &content[e + IA_END.len()..]);
        }
    }
    if let Some(s) = content.find(crate::fiche::BLOCK_START) {
        return format!("{}{}\n\n{}", &content[..s], block, &content[s..]);
    }
    let sep = if content.ends_with("\n\n") { "" } else if content.ends_with('\n') { "\n" } else { "\n\n" };
    format!("{content}{sep}{block}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib() -> Vec<Item> {
        vec![Item {
            key: "K".into(),
            citekey: "chen2019graph".into(),
            title: "Graph-Based Fraud Detection: A Survey".into(),
            doi: "10.1/xyz".into(),
            ..Default::default()
        }]
    }

    #[test]
    fn answer_and_json() {
        let text = "### Problématique\nTexte.\n\n```json\n{\"tags\": [\"gnn\", \"fraude\"], \"keywords\": [\"GNN\"], \"references\": [{\"title\": \"A\", \"year\": 2020}]}\n```\n";
        let (analysis, ex) = split_response(text);
        assert_eq!(analysis, "### Problématique\nTexte.");
        assert_eq!(ex.tags, vec!["gnn", "fraude"]);
        assert_eq!(ex.references[0].year, Some(2020));
        // No JSON block: everything is analysis.
        assert_eq!(split_response("juste du texte").0, "juste du texte");
    }

    #[test]
    fn reference_check() {
        let r = |title: &str, doi: &str| CitedRef { title: title.into(), doi: doi.into(), ..Default::default() };
        let text = "References\n[12] Smith J. Mobile money fraud in West Africa: evidence from Burkina Faso. 2021.";
        assert_eq!(check_ref(&r("Graph-based fraud detection: a survey", ""), &lib(), None), RefCheck::InLibrary { citekey: "chen2019graph".into() });
        assert_eq!(check_ref(&r("Autre titre", "https://doi.org/10.1/XYZ"), &lib(), None), RefCheck::InLibrary { citekey: "chen2019graph".into() });
        assert_eq!(check_ref(&r("Mobile Money Fraud in West Africa — Evidence from Burkina Faso", ""), &lib(), Some(text)), RefCheck::InArticle);
        assert_eq!(check_ref(&r("A reference the model made up entirely", ""), &lib(), Some(text)), RefCheck::Unverified);

        let refs = vec![
            (CitedRef { title: "Graph survey".into(), authors: "Chen".into(), year: Some(2019), why: "Base".into(), ..Default::default() }, RefCheck::InLibrary { citekey: "chen2019graph".into() }),
            (r("Inventée", ""), RefCheck::Unverified),
        ];
        let s = references_section(&refs, Lang::Fr, true);
        assert!(s.contains("[@chen2019graph]"));
        assert!(s.contains("> [!attention]- À vérifier"));
        assert!(s.contains("> - *Inventée*"));
    }

    #[test]
    fn insertion_without_touching_the_rest() {
        let fiche = format!("---\ntype: fiche\n---\n# T\n\n## Limites\nMon texte.\n\n{}\nannotations\n{}\n", crate::fiche::BLOCK_START, crate::fiche::BLOCK_END);
        let b1 = block("### Problématique\nV1", "### Refs", "claude-opus-5-5", Lang::Fr);
        let once = insert_block(&fiche, &b1);
        assert!(once.contains("Mon texte."));
        assert!(once.find(IA_START).unwrap() < once.find(crate::fiche::BLOCK_START).unwrap());
        // Regeneration: the block is replaced and present only once.
        let twice = insert_block(&once, &block("### Problématique\nV2", "### Refs", "m", Lang::Fr));
        assert!(twice.contains("V2") && !twice.contains("V1"));
        assert_eq!(twice.matches(IA_START).count(), 1);
        assert!(twice.contains("annotations"));
    }

    #[test]
    fn reading_note_content() {
        let a = "### Problématique\nP.\n\n### Perspectives\nX.\n\n### Commentaire\nC.";
        let c = fiche_content(a, "### Meilleures références citées\n\n- R", "claude-opus-5-5", Lang::Fr);
        assert!(c.starts_with(IA_START) && c.ends_with(IA_END));
        assert!(c.contains("## Problématique") && !c.contains("### "));
        // References before the final commentary.
        assert!(c.find("## Meilleures références citées").unwrap() < c.find("## Commentaire").unwrap());
    }

    #[test]
    fn instructions() {
        assert!(system_prompt(Lang::Fr, "").contains("N'invente rien"));
        assert!(!system_prompt(Lang::Fr, "").contains("{CONTEXTE}"));
        assert!(system_prompt(Lang::Fr, "fraude au mobile money").contains("recherche du doctorant : fraude au mobile money"));
        assert!(system_prompt(Lang::En, "").contains("Invent nothing"));
        let it = Item { title: "Titre".into(), abstract_note: "Résumé exact.".into(), year: Some(2024), ..Default::default() };
        let ann = Annotation { text: "passage".into(), comment: "à citer".into(), page_label: "4".into(), ..Default::default() };
        let p = user_prompt(&it, &[ann], Lang::Fr);
        assert!(p.contains("Résumé exact.") && p.contains("« passage » (p. 4) : à citer"));
    }
}
