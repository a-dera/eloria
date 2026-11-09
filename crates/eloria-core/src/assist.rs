//! AI writing and reading assistants: rephrase, fix, summarise, and "Ask Eloria"
//! (questions asked to the vault and the library).
//!
//! Two safeguards, enforced by code and not left to the model:
//! - **Protected citations**: before sending, every `[@key]`, `[[link]]`, formula and
//!   code span is replaced by a token ⟦n⟧. The answer must return each token exactly
//!   once, without inventing any: otherwise it is rejected. The AI can therefore neither
//!   change, remove nor add a citation.
//! - **Checked sources** (Ask Eloria): the model only sees numbered excerpts
//!   [S1]…[Sn] taken from the vault and Zotero. Any reference it cites outside these
//!   excerpts and the library is flagged "to check".

use std::collections::HashSet;
use std::sync::atomic::AtomicBool;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::ai::{self, AiConfig, AiRequest, AiResponse, Part};
use crate::error::Error;
use crate::lang::Lang;
use crate::Result;

// ------------------------------------------------------------------ protection

fn protected_re() -> Regex {
    // Order: embeds and links, citations, formulas, inline code, URLs.
    Regex::new(r"!?\[\[[^\]]*\]\]|\[[^\[\]]*@[A-Za-z0-9_:\-.]+[^\[\]]*\]|\$\$[\s\S]*?\$\$|\$[^$\n]+\$|`[^`\n]+`|https?://[^\s)>\]]+").unwrap()
}

/// Replaces the elements to protect with tokens ⟦1⟧, ⟦2⟧… Returns the masked text and
/// the original elements (token n is element n-1).
pub fn protect(text: &str) -> (String, Vec<String>) {
    let mut kept = Vec::new();
    let masked = protected_re()
        .replace_all(text, |c: &regex::Captures| {
            kept.push(c[0].to_string());
            format!("⟦{}⟧", kept.len())
        })
        .to_string();
    (masked, kept)
}

/// Puts the protected elements back. Fails if a token is missing, repeated or invented, or if
/// the answer contains a citation or link that was not in the text.
pub fn restore(output: &str, kept: &[String]) -> Result<String> {
    let tok = Regex::new(r"⟦(\d+)⟧").unwrap();
    let mut seen = vec![0usize; kept.len()];
    for c in tok.captures_iter(output) {
        let n: usize = c[1].parse().unwrap_or(0);
        if n == 0 || n > kept.len() {
            return Err(Error::Ai("la réponse contient une citation ou un lien inventé : elle est écartée".into()));
        }
        seen[n - 1] += 1;
    }
    if let Some(i) = seen.iter().position(|&s| s != 1) {
        return Err(Error::Ai(format!(
            "la réponse {} « {} » : elle est écartée pour ne pas toucher à tes références",
            if seen[i] == 0 { "a retiré" } else { "a dupliqué" },
            kept[i]
        )));
    }
    // Nothing new may look like a citation or a link.
    let (_, extra) = protect(&tok.replace_all(output, ""));
    if let Some(x) = extra.iter().find(|x| x.starts_with('[') || x.starts_with("![")) {
        return Err(Error::Ai(format!("la réponse ajoute une référence absente du texte ({x}) : elle est écartée")));
    }
    Ok(tok
        .replace_all(output, |c: &regex::Captures| kept[c[1].parse::<usize>().unwrap() - 1].clone())
        .to_string())
}

// ------------------------------------------------------------------ rewriting

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Task {
    Rephrase,
    Fix,
    Expand,
    Shorten,
    Academic,
    TranslateEn,
    TranslateFr,
    Summarize,
}

impl Task {
    pub fn id(self) -> &'static str {
        match self {
            Task::Rephrase => "reformuler",
            Task::Fix => "corriger",
            Task::Expand => "developper",
            Task::Shorten => "raccourcir",
            Task::Academic => "style-academique",
            Task::TranslateEn => "traduire-en",
            Task::TranslateFr => "traduire-fr",
            Task::Summarize => "resume",
        }
    }

    fn instruction(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Task::Rephrase, Lang::Fr) => "Reformule ce passage avec d'autres mots, même sens, même longueur environ.",
            (Task::Fix, Lang::Fr) => "Corrige l'orthographe, la grammaire, la ponctuation et la typographie française. Ne change rien d'autre.",
            (Task::Expand, Lang::Fr) => "Développe ce passage : explicite le raisonnement et les transitions, sans ajouter de faits ni de sources.",
            (Task::Shorten, Lang::Fr) => "Raccourcis ce passage d'environ un tiers en gardant toutes les idées et toutes les références.",
            (Task::Academic, Lang::Fr) => "Réécris ce passage dans un style académique de thèse : précis, impersonnel, nuancé, sans emphase.",
            (Task::TranslateEn, _) => "Translate this passage into academic English.",
            (Task::TranslateFr, _) => "Traduis ce passage en français académique.",
            (Task::Summarize, Lang::Fr) => "Résume ce texte fidèlement, sans rien ajouter : environ un tiers de sa longueur, 6 phrases au plus (une ou deux pour un texte court). Garde les références qui portent les idées principales.",
            (Task::Rephrase, Lang::En) => "Rephrase this passage in other words, same meaning, about the same length.",
            (Task::Fix, Lang::En) => "Fix spelling, grammar, punctuation and typography. Change nothing else.",
            (Task::Expand, Lang::En) => "Expand this passage: make the reasoning and transitions explicit, without adding facts or sources.",
            (Task::Shorten, Lang::En) => "Shorten this passage by about a third, keeping every idea and every reference.",
            (Task::Academic, Lang::En) => "Rewrite this passage in an academic thesis style: precise, impersonal, nuanced, no hype.",
            (Task::Summarize, Lang::En) => "Summarize this text faithfully, adding nothing: about a third of its length, at most 6 sentences (one or two for a short text). Keep the references that carry the main ideas.",
        }
    }
}

/// Result of a rewrite, for the user to accept or reject.
#[derive(Debug, Clone, Serialize)]
pub struct Rewrite {
    pub text: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

fn system_rewrite(lang: Lang, summarize: bool) -> String {
    let base = match lang {
        Lang::Fr => "Tu es l'assistant de rédaction d'Eloria, pour une thèse de doctorat. Tu réponds uniquement par le texte demandé, sans préambule ni commentaire, en Markdown.\n\nLe texte contient des jetons ⟦1⟧, ⟦2⟧… : ce sont des citations, liens, formules ou codes protégés. Recopie chaque jeton exactement une fois, à l'endroit qui convient. N'en invente aucun, n'en supprime aucun, n'écris jamais de citation [@…] ni de lien [[…]] toi-même.",
        Lang::En => "You are Eloria's writing assistant, for a doctoral thesis. Reply only with the requested text, no preamble or comment, in Markdown.\n\nThe text contains tokens ⟦1⟧, ⟦2⟧…: they are protected citations, links, formulas or code. Copy each token exactly once, where it belongs. Never invent or drop one, never write a citation [@…] or link [[…]] yourself.",
    };
    if summarize && lang == Lang::Fr {
        format!("{base}\n\nSi le texte est long, ne garde que les jetons des idées principales : les autres peuvent être omis.")
    } else if summarize {
        format!("{base}\n\nIf the text is long, keep only the tokens of the main ideas: the others may be omitted.")
    } else {
        base.to_string()
    }
}

/// Rewrites `text` according to `task`. A summary may omit references (never add any).
pub fn rewrite(
    cfg: &AiConfig,
    key: Option<&str>,
    task: Task,
    text: &str,
    lang: Lang,
    on_delta: &mut dyn FnMut(&str),
    cancel: &AtomicBool,
) -> Result<Rewrite> {
    let (masked, kept) = protect(text);
    let req = AiRequest {
        system: system_rewrite(lang, task == Task::Summarize),
        parts: vec![Part::Text(format!("{}\n\n---\n{masked}\n---", task.instruction(lang)))],
        max_tokens: 8000,
    };
    let r: AiResponse = ai::complete(cfg, key, &req, on_delta, cancel)?;
    let out = r.text.trim().trim_matches('-').trim().to_string();
    let restored = if task == Task::Summarize { restore_subset(&out, &kept)? } else { restore(&out, &kept)? };
    Ok(Rewrite { text: restored, model: r.model, input_tokens: r.input_tokens, output_tokens: r.output_tokens })
}

/// Like `restore`, but a token may be omitted (summary). Never invented nor repeated.
fn restore_subset(output: &str, kept: &[String]) -> Result<String> {
    let tok = Regex::new(r"⟦(\d+)⟧").unwrap();
    let mut seen = HashSet::new();
    for c in tok.captures_iter(output) {
        let n: usize = c[1].parse().unwrap_or(0);
        if n == 0 || n > kept.len() || !seen.insert(n) {
            return Err(Error::Ai("le résumé contient une référence inventée ou répétée : il est écarté".into()));
        }
    }
    let (_, extra) = protect(&tok.replace_all(output, ""));
    if let Some(x) = extra.iter().find(|x| x.starts_with('[') || x.starts_with("![")) {
        return Err(Error::Ai(format!("le résumé ajoute une référence absente du texte ({x}) : il est écarté")));
    }
    Ok(tok.replace_all(output, |c: &regex::Captures| kept[c[1].parse::<usize>().unwrap() - 1].clone()).to_string())
}

/// `[!résumé]` callout ready to insert.
pub fn summary_callout(summary: &str, lang: Lang) -> String {
    let body: Vec<String> = summary.lines().map(|l| format!("> {l}").trim_end().to_string()).collect();
    format!("> [!résumé] {}\n{}\n", lang.pick("Résumé (IA, relu)", "Summary (AI, reviewed)"), body.join("\n"))
}

// ------------------------------------------------------------------ Ask Eloria

/// An excerpt given to the model, numbered [S1]…
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Passage {
    pub id: usize,
    /// Vault note (empty for a Zotero item not imported)
    pub path: String,
    pub title: String,
    /// Citation key when the excerpt comes from a source or an item
    pub citekey: String,
    /// Zotero identifier, to open an item that was not imported
    pub zotero: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Answer {
    pub text: String,
    pub passages: Vec<Passage>,
    /// References cited by the model that are neither given excerpts nor in the library
    pub unverified: Vec<String>,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

const STOP: &[&str] = &[
    "le", "la", "les", "un", "une", "des", "du", "de", "d", "l", "et", "ou", "en", "dans", "sur", "pour", "par", "avec", "sans", "que", "qui", "quoi", "quels",
    "quelles", "quel", "quelle", "est", "sont", "ce", "cette", "ces", "mes", "mon", "ma", "se", "sa", "son", "ses", "leur", "leurs", "il", "elle", "ils", "elles",
    "au", "aux", "ne", "pas", "plus", "comment", "pourquoi", "disent", "dit", "entre", "compare", "the", "a", "an", "of", "and", "or", "in", "on", "for", "to",
    "is", "are", "what", "which", "how", "why", "my", "do", "does", "about", "with", "lectures", "notes", "articles",
];

/// Lower-case text without accents (to compare stems).
pub fn fold(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfd().filter(|c| !('\u{300}'..='\u{36f}').contains(c)).collect::<String>().to_lowercase()
}

/// Useful words of a question (for full-text search).
pub fn keywords(question: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    question
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .map(|w| w.trim_matches('-').to_lowercase())
        .filter(|w| w.chars().count() > 2 && !STOP.contains(&w.as_str()))
        .filter(|w| seen.insert(w.clone()))
        .take(12)
        .collect()
}

/// Approximate stem of a word (first 5 letters, no accent): "fraude" and
/// "fraud", "transactionnelle" and "transaction" meet, across languages.
pub fn stem(w: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let plain: String = w.nfd().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
    plain.chars().take(5).collect()
}

/// Stems of the useful words of a question.
pub fn stems(question: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    keywords(question).iter().map(|w| stem(w)).filter(|s| s.len() >= 3 && seen.insert(s.clone())).collect()
}

/// Paragraphs of a note richest in words from the question, up to `max` characters.
pub fn excerpt(body: &str, words: &[String], max: usize) -> String {
    // Headings alone (the empty outline of a reading note) bring nothing to the model.
    let paras: Vec<&str> = body
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty() && !p.starts_with("<!--") && !p.lines().all(|l| l.trim().is_empty() || l.trim_start().starts_with('#')))
        .collect();
    let mut scored: Vec<(usize, usize)> = paras
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let low = crate::assist::fold(p);
            (words.iter().filter(|w| low.contains(&stem(w))).count(), i)
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut chosen: Vec<usize> = Vec::new();
    let mut len = 0;
    for (score, i) in scored {
        if score == 0 && !chosen.is_empty() {
            break;
        }
        if len + paras[i].len() > max && !chosen.is_empty() {
            continue;
        }
        len += paras[i].len();
        chosen.push(i);
        if len >= max {
            break;
        }
    }
    chosen.sort();
    let mut out = chosen.iter().map(|&i| paras[i]).collect::<Vec<_>>().join("\n\n");
    if out.len() > max {
        let mut cut = max;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        out.push('…');
    }
    out
}

fn system_ask(lang: Lang, topic: &str) -> String {
    let topic = if topic.trim().is_empty() { String::new() } else { format!("\n\n{} {topic}", lang.pick("Sujet de recherche de l'utilisateur :", "User's research topic:")) };
    match lang {
        Lang::Fr => format!(
            "Tu es Ask Eloria, l'assistant de recherche d'un doctorant. Tu réponds à sa question UNIQUEMENT à partir des extraits numérotés [S1], [S2]… de ses notes et de sa bibliothèque.\n\n\
Règles absolues :\n\
- Chaque affirmation tirée d'un extrait est suivie de sa source entre crochets, par exemple [S2] ou [S1][S4].\n\
- N'invente jamais de référence, d'auteur, d'année ni de citation. N'utilise que les numéros d'extraits fournis.\n\
- Si les extraits ne permettent pas de répondre, dis-le clairement et indique ce qu'il faudrait lire ou chercher.\n\
- Distingue ce que disent les sources de ton analyse (« Analyse : … »).\n\
- Réponds en français, en Markdown, de façon structurée et concise.{topic}"
        ),
        Lang::En => format!(
            "You are Ask Eloria, a doctoral student's research assistant. Answer ONLY from the numbered excerpts [S1], [S2]… of their notes and library.\n\n\
Absolute rules:\n\
- Every claim taken from an excerpt is followed by its source in brackets, e.g. [S2] or [S1][S4].\n\
- Never invent a reference, author, year or quotation. Only use the excerpt numbers provided.\n\
- If the excerpts cannot answer, say so clearly and suggest what to read or search.\n\
- Separate what the sources say from your own analysis (\"Analysis: …\").\n\
- Answer in English, in Markdown, structured and concise.{topic}"
        ),
    }
}

fn user_ask(question: &str, passages: &[Passage], lang: Lang) -> String {
    let mut s = String::new();
    for p in passages {
        let cite = if p.citekey.is_empty() { String::new() } else { format!(" (@{})", p.citekey) };
        s.push_str(&format!("[S{}] {}{cite}\n{}\n\n", p.id, p.title, p.text));
    }
    format!("{}\n\n{s}{} {question}", lang.pick("Extraits :", "Excerpts:"), lang.pick("Question :", "Question:"))
}

/// Checks an answer: excerpt numbers that do not exist and `[@key]` citations outside the
/// library and the excerpts.
pub fn verify_answer(text: &str, passages: &[Passage], library: &HashSet<String>) -> Vec<String> {
    let mut bad = Vec::new();
    for c in Regex::new(r"\[S(\d+)\]").unwrap().captures_iter(text) {
        let n: usize = c[1].parse().unwrap_or(0);
        if !passages.iter().any(|p| p.id == n) && !bad.contains(&c[0].to_string()) {
            bad.push(c[0].to_string());
        }
    }
    let known: HashSet<String> = passages.iter().map(|p| p.citekey.to_lowercase()).filter(|k| !k.is_empty()).collect();
    for c in Regex::new(r"@([A-Za-z0-9_:\-.]+)").unwrap().captures_iter(text) {
        let k = c[1].trim_end_matches('.').to_lowercase();
        if !known.contains(&k) && !library.contains(&k) && !bad.contains(&format!("@{k}")) {
            bad.push(format!("@{k}"));
        }
    }
    bad
}

/// Answer ready to insert into a note: [S2] becomes `[@key]` for a source or an
/// item, `[[note]]` for a vault note.
pub fn answer_to_markdown(text: &str, passages: &[Passage]) -> String {
    Regex::new(r"\[S(\d+)\]")
        .unwrap()
        .replace_all(text, |c: &regex::Captures| {
            let n: usize = c[1].parse().unwrap_or(0);
            match passages.iter().find(|p| p.id == n) {
                Some(p) if !p.citekey.is_empty() => format!("[@{}]", p.citekey),
                Some(p) if !p.path.is_empty() => format!("[[{}]]", crate::vault::stem(&p.path)),
                _ => c[0].to_string(),
            }
        })
        .to_string()
}

/// Draft state of the art from the excerpts of the articles included in a review.
#[allow(clippy::too_many_arguments)]
pub fn draft_review(
    cfg: &AiConfig,
    key: Option<&str>,
    question: &str,
    passages: Vec<Passage>,
    library: &HashSet<String>,
    lang: Lang,
    on_delta: &mut dyn FnMut(&str),
    cancel: &AtomicBool,
) -> Result<Answer> {
    if passages.is_empty() {
        return Err(Error::Ai(lang.pick("aucun article inclus n'a de fiche ni de résumé : inclus des articles et remplis leurs fiches", "no included article has a note or abstract: include articles and fill in their notes").into()));
    }
    let req = AiRequest {
        system: crate::review::draft_system(lang, question),
        parts: vec![Part::Text(user_ask(lang.pick("Rédige le brouillon de la section.", "Write the draft section."), &passages, lang))],
        max_tokens: 12000,
    };
    let r = ai::complete(cfg, key, &req, on_delta, cancel)?;
    let unverified = verify_answer(&r.text, &passages, library);
    Ok(Answer { text: r.text, passages, unverified, model: r.model, input_tokens: r.input_tokens, output_tokens: r.output_tokens })
}

/// Ask Eloria modes: same excerpt search, same reference check,
/// different instructions. `custom:` carries the instructions of a custom assistant.
pub fn mode_instruction(mode: &str, lang: Lang) -> Option<String> {
    let m = match (mode, lang) {
        ("contradictions", Lang::Fr) => "Mode « contradictions » : relève les points sur lesquels les extraits se contredisent ou divergent (résultats, définitions, méthodes). Pour chaque point, mets côte à côte les passages exacts en citation (> …) avec leur source [Sn], puis explique l'origine probable de l'écart (données, contexte, méthode). S'il n'y a pas de contradiction, dis-le.",
        ("contradictions", Lang::En) => "\"Contradictions\" mode: identify points where the excerpts contradict or diverge (results, definitions, methods). For each point, quote the exact passages side by side (> …) with their source [Sn], then explain the likely cause (data, context, method). If there is none, say so.",
        ("debat", Lang::Fr) => "Mode « débat » : la question oppose deux positions (théories, auteurs, documents). Présente la position A puis la position B avec leurs meilleurs arguments sourcés, puis les objections de chacune à l'autre, et termine par « Arbitrage » : ce que les extraits permettent de trancher, et ce qui reste ouvert.",
        ("debat", Lang::En) => "\"Debate\" mode: the question opposes two positions (theories, authors, documents). Present position A then B with their best sourced arguments, then each one's objections to the other, and end with \"Verdict\": what the excerpts allow to settle and what remains open.",
        ("manques", Lang::Fr) => "Mode « manques » : la question décrit un chapitre ou une question de recherche. Dis ce que les extraits couvrent bien (sourcé), puis liste les sous-sujets peu ou pas couverts par les lectures et notes, et pour chacun le type de lecture à chercher (mots-clés de recherche). Ne cite que les extraits pour ce qui est couvert.",
        ("manques", Lang::En) => "\"Gaps\" mode: the question describes a chapter or research question. State what the excerpts cover well (sourced), then list the sub-topics poorly or not covered by the readings and notes, each with the kind of reading to look for (search keywords). Cite excerpts only for what is covered.",
        ("synthese", Lang::Fr) => "Mode « synthèse » : rédige une synthèse structurée des extraits sur la question, en paragraphes, en regroupant les travaux par approche, chaque affirmation sourcée.",
        ("synthese", Lang::En) => "\"Synthesis\" mode: write a structured synthesis of the excerpts on the question, in paragraphs, grouping works by approach, every claim sourced.",
        _ => {
            return mode.strip_prefix("custom:").map(|c| format!("{} {}", lang.pick("Consignes de l'assistant choisi par l'utilisateur :", "Instructions of the assistant chosen by the user:"), c.trim()));
        }
    };
    Some(m.to_string())
}

#[allow(clippy::too_many_arguments)]
pub fn ask(
    cfg: &AiConfig,
    key: Option<&str>,
    question: &str,
    passages: Vec<Passage>,
    library: &HashSet<String>,
    lang: Lang,
    topic: &str,
    mode: &str,
    on_delta: &mut dyn FnMut(&str),
    cancel: &AtomicBool,
) -> Result<Answer> {
    if passages.is_empty() {
        return Err(Error::Ai(lang.pick("aucune note ni aucun document ne correspond à la question : reformule avec d'autres mots", "no note or document matches the question: rephrase with other words").into()));
    }
    let mut system = system_ask(lang, topic);
    if let Some(m) = mode_instruction(mode, lang) {
        system.push_str("\n\n");
        system.push_str(&m);
    }
    let req = AiRequest { system, parts: vec![Part::Text(user_ask(question, &passages, lang))], max_tokens: 8000 };
    let r = ai::complete(cfg, key, &req, on_delta, cancel)?;
    let unverified = verify_answer(&r.text, &passages, library);
    Ok(Answer { text: r.text, passages, unverified, model: r.model, input_tokens: r.input_tokens, output_tokens: r.output_tokens })
}

// ------------------------------------------------------------------ structured suggestions

/// A concept suggested from a note (name and definition taken from the text).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConceptSuggestion {
    pub name: String,
    pub definition: String,
}

/// First JSON array of an answer (models sometimes wrap it in text).
/// First non-empty JSON array of an answer, even wrapped in text, code fences
/// or preceded by brackets that are not one ("[S1]").
pub fn json_array(text: &str) -> Option<serde_json::Value> {
    for (i, _) in text.match_indices('[') {
        let mut it = serde_json::Deserializer::from_str(&text[i..]).into_iter::<serde_json::Value>();
        if let Some(Ok(v)) = it.next() {
            if v.as_array().map(|a| !a.is_empty()).unwrap_or(false) {
                return Some(v);
            }
        }
    }
    None
}

pub fn parse_concepts(text: &str) -> Vec<ConceptSuggestion> {
    json_array(text)
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|o| {
            let name = o.get("name").or_else(|| o.get("nom"))?.as_str()?.trim().to_string();
            let def = o.get("definition").or_else(|| o.get("définition")).and_then(|d| d.as_str()).unwrap_or_default().trim().to_string();
            (!name.is_empty() && name.len() < 80).then_some(ConceptSuggestion { name, definition: def })
        })
        .take(12)
        .collect()
}

/// Suggested tags: the existing taxonomy first, in tag format (lower case, hyphens).
pub fn parse_tags(text: &str, existing: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for v in json_array(text).and_then(|v| v.as_array().cloned()).unwrap_or_default() {
        let Some(t) = v.as_str() else { continue };
        let Some(tag) = crate::fiche::note_tag(t.trim_start_matches('#')) else { continue };
        if tag.is_empty() || out.contains(&tag) {
            continue;
        }
        // An existing tag written differently (case, accents) takes the existing form.
        let known = existing.iter().find(|e| fold(e) == fold(&tag)).cloned();
        out.push(known.unwrap_or(tag));
    }
    out.truncate(10);
    out
}

pub fn suggest_concepts(cfg: &AiConfig, key: Option<&str>, text: &str, lang: Lang, cancel: &AtomicBool) -> Result<Vec<ConceptSuggestion>> {
    let system = match lang {
        Lang::Fr => "Tu extrais les concepts importants d'une note de recherche (notions, méthodes, phénomènes), pour en faire des notes de concept. Réponds UNIQUEMENT par un tableau JSON : [{\"name\": \"…\", \"definition\": \"…\"}]. Le nom est court (1 à 4 mots, sans article). La définition, en une ou deux phrases, s'appuie sur le texte fourni ; si le texte ne définit pas le concept, laisse \"definition\" vide. Au plus 10 concepts.",
        Lang::En => "You extract the important concepts of a research note (notions, methods, phenomena) to make concept notes. Reply ONLY with a JSON array: [{\"name\": \"…\", \"definition\": \"…\"}]. The name is short (1 to 4 words). The definition, one or two sentences, relies on the given text; if the text does not define it, leave \"definition\" empty. At most 10 concepts.",
    };
    let req = AiRequest { system: system.into(), parts: vec![Part::Text(text.chars().take(24000).collect())], max_tokens: 3000 };
    let r = ai::complete(cfg, key, &req, &mut |_| {}, cancel)?;
    Ok(parse_concepts(&r.text))
}

pub fn suggest_tags(cfg: &AiConfig, key: Option<&str>, text: &str, existing: &[String], lang: Lang, cancel: &AtomicBool) -> Result<Vec<String>> {
    let taxo = existing.iter().take(300).cloned().collect::<Vec<_>>().join(", ");
    let system = match lang {
        Lang::Fr => format!("Tu proposes des tags pour une note de recherche. Réutilise en priorité les tags existants de l'utilisateur ; n'en crée de nouveaux que si aucun ne convient. Réponds UNIQUEMENT par un tableau JSON de 3 à 8 tags, en minuscules, mots séparés par des tirets. Tags existants : {taxo}"),
        Lang::En => format!("You suggest tags for a research note. Reuse the user's existing tags first; only create new ones if none fits. Reply ONLY with a JSON array of 3 to 8 tags, lowercase, words separated by hyphens. Existing tags: {taxo}"),
    };
    let req = AiRequest { system, parts: vec![Part::Text(text.chars().take(16000).collect())], max_tokens: 800 };
    let r = ai::complete(cfg, key, &req, &mut |_| {}, cancel)?;
    Ok(parse_tags(&r.text, existing))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_array_in_answer() {
        assert_eq!(parse_tags("Voici [S1] mes tags :
```json
[\"mobile-money\", \"#Fraude\"]
```
Voilà [fin].", &["fraude".into()]), vec!["mobile-money", "fraude"]);
        assert_eq!(parse_tags("[]  puis [\"xai\"]", &[]), vec!["xai"]);
        assert!(parse_tags("aucun tableau", &[]).is_empty());
    }

    #[test]
    fn modes_and_suggestions() {
        assert!(mode_instruction("contradictions", Lang::Fr).unwrap().contains("côte à côte"));
        assert!(mode_instruction("question", Lang::Fr).is_none());
        assert_eq!(mode_instruction("custom:Relis comme un jury.", Lang::Fr).unwrap(), "Consignes de l'assistant choisi par l'utilisateur : Relis comme un jury.");
        let c = parse_concepts("Voici :\n[{\"name\": \"Compte mule\", \"definition\": \"Compte qui fait transiter des fonds.\"}, {\"nom\": \"SHAP\"}, {\"name\": \"\"}]");
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].name.as_str(), c[1].definition.as_str()), ("Compte mule", ""));
        let t = parse_tags("[\"Fraude\", \"machine learning\", \"#GNN\", \"fraude\"]", &["fraude".to_string(), "gnn".to_string()]);
        assert_eq!(t, vec!["fraude", "machine-learning", "gnn"]);
        assert!(parse_concepts("pas de json").is_empty());
    }

    #[test]
    fn protected_citations() {
        let t = "Les fraudes forment des sous-graphes [@smith2024graph, p. 4], voir [[SHAP|attributions]] et $x^2$.";
        let (m, kept) = protect(t);
        assert_eq!(kept.len(), 3);
        assert!(!m.contains("smith") && m.contains("⟦1⟧") && m.contains("⟦3⟧"));
        // Correct answer: everything comes back unchanged.
        let ok = restore("Des sous-graphes denses apparaissent ⟦1⟧ ; cf. ⟦2⟧ et ⟦3⟧.", &kept).unwrap();
        assert!(ok.contains("[@smith2024graph, p. 4]") && ok.contains("[[SHAP|attributions]]") && ok.contains("$x^2$"));
        // Token removed, duplicated, invented, citation added: rejected.
        assert!(restore("Sans la citation ⟦2⟧ ⟦3⟧.", &kept).is_err());
        assert!(restore("⟦1⟧ ⟦1⟧ ⟦2⟧ ⟦3⟧", &kept).is_err());
        assert!(restore("⟦1⟧ ⟦2⟧ ⟦3⟧ ⟦4⟧", &kept).is_err());
        assert!(restore("⟦1⟧ ⟦2⟧ ⟦3⟧ [@doe2020]", &kept).is_err());
        assert!(restore("⟦1⟧ ⟦2⟧ ⟦3⟧ [[Autre]]", &kept).is_err());
        // A summary may omit, never add.
        assert!(restore_subset("Résumé ⟦1⟧.", &kept).is_ok());
        assert!(restore_subset("Résumé ⟦1⟧ [@inventé2021].", &kept).is_err());
    }

    #[test]
    fn words_and_excerpts() {
        assert_eq!(keywords("Que disent mes lectures sur la fraude transactionnelle ?"), vec!["fraude", "transactionnelle"]);
        // Stems: French meets the English of the articles.
        assert_eq!(stems("la fraude transactionnelle"), vec!["fraud", "trans"]);
        assert!(fold("Fraud Detection in Transactions").contains(&stem("transati")));
        let body = "Intro sans rapport.\n\nLa fraude transactionnelle croît.\n\nAutre chose.\n\nFraude et mobile money.";
        let e = excerpt(body, &keywords("fraude transactionnelle"), 200);
        assert!(e.starts_with("La fraude transactionnelle") && e.contains("Fraude et mobile") && !e.contains("Intro"));
    }

    #[test]
    fn checked_answer() {
        let ps = vec![
            Passage { id: 1, path: "Sources/@smith2024graph.md".into(), title: "GNN".into(), citekey: "smith2024graph".into(), zotero: "A".into(), text: "x".into() },
            Passage { id: 2, path: "Concepts/SHAP.md".into(), title: "SHAP".into(), citekey: String::new(), zotero: String::new(), text: "y".into() },
        ];
        let lib: HashSet<String> = ["lundberg2017unified".to_string()].into();
        let bad = verify_answer("Selon [S1] et [S3], voir @lundberg2017unified et @fantome2020.", &ps, &lib);
        assert_eq!(bad, vec!["[S3]", "@fantome2020"]);
        assert_eq!(answer_to_markdown("Dense [S1], défini [S2].", &ps), "Dense [@smith2024graph], défini [[SHAP]].");
        assert_eq!(summary_callout("Ligne 1\nLigne 2", Lang::Fr), "> [!résumé] Résumé (IA, relu)\n> Ligne 1\n> Ligne 2\n");
    }
}
