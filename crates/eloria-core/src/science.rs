//! Scientific writing assistant: produce LaTeX (table, equation, TikZ figure),
//! explain or fix a formula, suggest improvements, and check that every
//! citing sentence is supported by the cited document. References are never
//! changed or added: an answer that cites a key absent from the input is rejected.

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::AtomicBool;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::ai::{self, AiConfig, AiRequest, Part};
use crate::assist::{protect, Rewrite};
use crate::error::Error;
use crate::lang::Lang;
use crate::Result;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SciTask {
    LatexTable,
    LatexEquation,
    Tikz,
    ExplainFormula,
    FixFormula,
    Improve,
}

impl SciTask {
    fn instruction(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (SciTask::LatexTable, Lang::Fr) => "Écris un tableau LaTeX à partir de la description ou des données ci-dessous : environnement table, tabular avec booktabs (\\toprule, \\midrule, \\bottomrule), \\caption et \\label{tab:…}. N'invente aucune valeur : une donnée manquante devient « -- ». Réponds UNIQUEMENT par le code LaTeX.",
            (SciTask::LatexTable, Lang::En) => "Write a LaTeX table from the description or data below: table environment, tabular with booktabs (\\toprule, \\midrule, \\bottomrule), \\caption and \\label{tab:…}. Do not invent any value: missing data becomes “--”. Reply ONLY with the LaTeX code.",
            (SciTask::LatexEquation, Lang::Fr) => "Écris l'équation LaTeX décrite ci-dessous, dans un environnement equation (ou align si plusieurs lignes) avec \\label{eq:…}, puis une ligne « où … » qui définit chaque symbole. Réponds UNIQUEMENT par le code LaTeX.",
            (SciTask::LatexEquation, Lang::En) => "Write the LaTeX equation described below, in an equation environment (or align for several lines) with \\label{eq:…}, then a “where …” line defining each symbol. Reply ONLY with the LaTeX code.",
            (SciTask::Tikz, Lang::Fr) => "Dessine la figure décrite ci-dessous en TikZ : environnement figure, tikzpicture lisible (styles nommés, nœuds alignés), \\caption et \\label{fig:…}. Réponds UNIQUEMENT par le code LaTeX.",
            (SciTask::Tikz, Lang::En) => "Draw the figure described below in TikZ: figure environment, a readable tikzpicture (named styles, aligned nodes), \\caption and \\label{fig:…}. Reply ONLY with the LaTeX code.",
            (SciTask::ExplainFormula, Lang::Fr) => "Explique cette formule à un doctorant : ce qu'elle calcule, le rôle de chaque symbole, l'intuition, les hypothèses implicites. Réponds en Markdown, sans réécrire la formule autrement qu'en $…$.",
            (SciTask::ExplainFormula, Lang::En) => "Explain this formula to a PhD student: what it computes, the role of each symbol, the intuition, implicit assumptions. Reply in Markdown, writing formulas only as $…$.",
            (SciTask::FixFormula, Lang::Fr) => "Corrige cette formule LaTeX (syntaxe, notation, cohérence des indices, parenthèses). Réponds en Markdown : d'abord le code corrigé dans un bloc ```latex, puis la liste des corrections. Si elle est déjà correcte, dis-le.",
            (SciTask::FixFormula, Lang::En) => "Fix this LaTeX formula (syntax, notation, index consistency, brackets). Reply in Markdown: first the corrected code in a ```latex block, then the list of fixes. If it is already correct, say so.",
            (SciTask::Improve, Lang::Fr) => "Propose des améliorations pour ce passage scientifique (clarté, structure, précision, transitions, termes vagues), en liste Markdown : pour chacune, la phrase concernée et la proposition. Ne modifie, n'ajoute ni ne retire aucune référence (jetons ⟦n⟧).",
            (SciTask::Improve, Lang::En) => "Suggest improvements for this scientific passage (clarity, structure, precision, transitions, vague terms), as a Markdown list: for each, the sentence concerned and the suggestion. Do not modify, add or remove any reference (⟦n⟧ tokens).",
        }
    }
}

/// Cited keys (Markdown `[@key]` and LaTeX `\cite{key}`).
pub fn cited(text: &str) -> HashSet<String> {
    let mut s: HashSet<String> = crate::markdown::parse_note(text).citations.into_iter().map(|k| k.to_lowercase()).collect();
    s.extend(crate::latex::cited_keys(text).into_iter().map(|k| k.to_lowercase()));
    s
}

/// Assistant answer. References of the input are protected; an answer that
/// cites another one is rejected.
pub fn run(cfg: &AiConfig, key: Option<&str>, task: SciTask, input: &str, lang: Lang, on_delta: &mut dyn FnMut(&str), cancel: &AtomicBool) -> Result<Rewrite> {
    let (masked, kept) = protect(input);
    let system = match lang {
        Lang::Fr => "Tu es l'assistant d'écriture scientifique d'un doctorant. Tu es exact et sobre. Les jetons ⟦n⟧ remplacent des références ou des liens : recopie-les tels quels là où ils servent, n'en invente jamais. N'ajoute aucune référence bibliographique.",
        Lang::En => "You are a PhD student's scientific writing assistant. You are precise and plain. ⟦n⟧ tokens stand for references or links: copy them unchanged where they belong, never invent one. Never add a bibliographic reference.",
    };
    let req = AiRequest { system: system.into(), parts: vec![Part::Text(format!("{}\n\n---\n{masked}\n---", task.instruction(lang)))], max_tokens: 6000 };
    let r = ai::complete(cfg, key, &req, on_delta, cancel)?;
    let mut out = r.text.trim().to_string();
    // Code only expected: a ```latex … ``` block is unwrapped.
    if matches!(task, SciTask::LatexTable | SciTask::LatexEquation | SciTask::Tikz) {
        if let Some(inner) = out.strip_prefix("```latex").or_else(|| out.strip_prefix("```tex")).or_else(|| out.strip_prefix("```")) {
            out = inner.trim_end().trim_end_matches("```").trim().to_string();
        }
    }
    let restored = restore_some(&out, &kept)?;
    let allowed = cited(input);
    let extra: Vec<String> = cited(&restored).into_iter().filter(|k| !allowed.contains(k)).collect();
    if !extra.is_empty() {
        return Err(Error::Ai(format!("{} : {}", lang.pick("la réponse cite une référence absente du texte, elle est écartée", "the answer cites a reference not in the text, it is discarded"), extra.join(", "))));
    }
    Ok(Rewrite { text: restored, model: r.model, input_tokens: r.input_tokens, output_tokens: r.output_tokens })
}

/// Puts the protected references back; an unknown (invented) token makes it fail.
fn restore_some(out: &str, kept: &[String]) -> Result<String> {
    let tok = Regex::new(r"⟦(\d+)⟧").unwrap();
    for c in tok.captures_iter(out) {
        let n: usize = c[1].parse().unwrap_or(0);
        if n == 0 || n > kept.len() {
            return Err(Error::Ai("la réponse contient une référence inventée : elle est écartée".into()));
        }
    }
    Ok(tok.replace_all(out, |c: &regex::Captures| kept[c[1].parse::<usize>().unwrap() - 1].clone()).into_owned())
}

/// A citing sentence, and the cited document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CitedSentence {
    pub line: usize,
    pub sentence: String,
    pub citekey: String,
}

/// Sentences of a text (note or LaTeX) that cite, one entry per (sentence, key). At most `max`.
pub fn cited_sentences(text: &str, max: usize) -> Vec<CitedSentence> {
    let (_, body, offset) = crate::markdown::split_frontmatter(text);
    let mut out = Vec::new();
    for (i, line) in body.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with('%') || t.starts_with("<!--") {
            continue;
        }
        // Splits into sentences without cutting "p. 4" or "et al.".
        let protected = t.replace("p. ", "p.\u{a0}").replace("al. ", "al.\u{a0}").replace("e.g. ", "e.g.\u{a0}").replace("i.e. ", "i.e.\u{a0}");
        for s in sentences_of(&protected) {
            let s = s.replace('\u{a0}', " ");
            // A link to a note ("[[@key|title]]") is not a citation: nothing to check.
            let keys = cited(&wikilink_re().replace_all(&s, ""));
            let mut keys: Vec<String> = keys.into_iter().collect();
            keys.sort();
            for k in keys {
                out.push(CitedSentence { line: offset + i + 1, sentence: s.trim().to_string(), citekey: k });
                if out.len() >= max {
                    return out;
                }
            }
        }
    }
    out
}

fn wikilink_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"!?\[\[[^\]]*\]\]").unwrap())
}

/// Sentences: split after . ? ! followed by a regular space (not a non-breaking one).
fn sentences_of(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut it = t.chars().peekable();
    while let Some(c) = it.next() {
        cur.push(c);
        if matches!(c, '.' | '?' | '!') && it.peek().map(|n| *n == ' ').unwrap_or(true) {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// Consistency verdict for a sentence and its source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SupportVerdict {
    pub line: usize,
    pub sentence: String,
    pub citekey: String,
    /// soutenue, partielle, non_soutenue, inconnue (no material on the source)
    pub verdict: String,
    pub reason: String,
}

/// Checks, with the AI, that each sentence is supported by what is known of the cited document
/// (`evidence`: key -> abstract, reading note, PDF excerpts). Without material: "inconnue".
pub fn check_support(cfg: &AiConfig, key: Option<&str>, sentences: &[CitedSentence], evidence: &BTreeMap<String, String>, lang: Lang, cancel: &AtomicBool) -> Result<Vec<SupportVerdict>> {
    let mut known: Vec<(usize, &CitedSentence)> = Vec::new();
    let mut out: Vec<SupportVerdict> = Vec::new();
    for (i, s) in sentences.iter().enumerate() {
        if evidence.get(&s.citekey).map(|e| e.trim().len() > 40).unwrap_or(false) {
            known.push((i, s));
        } else {
            out.push(SupportVerdict { line: s.line, sentence: s.sentence.clone(), citekey: s.citekey.clone(), verdict: "inconnue".into(), reason: lang.pick("rien sur ce document dans le coffre (ni résumé, ni fiche, ni texte du PDF)", "nothing on this document in the vault (no abstract, reading note or PDF text)").into() });
        }
    }
    if !known.is_empty() {
        let keys: BTreeMap<&str, &str> = known.iter().map(|(_, s)| (s.citekey.as_str(), evidence[&s.citekey].as_str())).collect();
        let mut prompt = String::from(lang.pick("Documents :\n", "Documents:\n"));
        for (k, e) in &keys {
            prompt.push_str(&format!("\n### {k}\n{}\n", e.chars().take(5000).collect::<String>()));
        }
        prompt.push_str(lang.pick("\nPhrases à vérifier :\n", "\nSentences to check:\n"));
        for (n, (_, s)) in known.iter().enumerate() {
            prompt.push_str(&format!("{}. [{}] {}\n", n + 1, s.citekey, s.sentence));
        }
        let system = match lang {
            Lang::Fr => "Tu vérifies qu'une phrase d'un manuscrit est bien soutenue par le document qu'elle cite, d'après le seul matériau fourni sur ce document. Pour chaque phrase numérotée, réponds par un objet JSON {\"n\": numéro, \"verdict\": \"soutenue\" | \"partielle\" | \"non_soutenue\", \"raison\": \"une phrase courte\"}. « partielle » : le document va dans ce sens mais la phrase dit plus, ou autre chose. Si le matériau ne permet pas de trancher, choisis « partielle » et dis-le. Réponds UNIQUEMENT par un tableau JSON.",
            Lang::En => "You check that a manuscript sentence is supported by the document it cites, using only the material given about that document. For each numbered sentence, reply with a JSON object {\"n\": number, \"verdict\": \"soutenue\" | \"partielle\" | \"non_soutenue\", \"raison\": \"one short sentence\"} (soutenue = supported, partielle = partly, non_soutenue = not supported). If the material cannot settle it, choose \"partielle\" and say so. Reply ONLY with a JSON array.",
        };
        let req = AiRequest { system: system.into(), parts: vec![Part::Text(prompt)], max_tokens: 4000 };
        let r = ai::complete(cfg, key, &req, &mut |_| {}, cancel)?;
        let arr = crate::assist::json_array(&r.text).and_then(|v| v.as_array().cloned()).unwrap_or_default();
        for (n, (_, s)) in known.iter().enumerate() {
            let found = arr.iter().find(|o| o.get("n").and_then(|x| x.as_u64()) == Some(n as u64 + 1));
            let verdict = found.and_then(|o| o.get("verdict")).and_then(|v| v.as_str()).filter(|v| ["soutenue", "partielle", "non_soutenue"].contains(v)).unwrap_or("inconnue");
            let reason = found.and_then(|o| o.get("raison").or_else(|| o.get("reason"))).and_then(|v| v.as_str()).unwrap_or_default();
            out.push(SupportVerdict { line: s.line, sentence: s.sentence.clone(), citekey: s.citekey.clone(), verdict: verdict.into(), reason: reason.into() });
        }
    }
    let rank = |v: &str| match v {
        "non_soutenue" => 0,
        "partielle" => 1,
        "inconnue" => 2,
        _ => 3,
    };
    out.sort_by(|a, b| rank(&a.verdict).cmp(&rank(&b.verdict)).then(a.line.cmp(&b.line)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn citing_sentences() {
        let t = "---\ntitre: x\n---\n# Ch2\n\nLes GNN repèrent les réseaux [@smith2024, p. 4]. Sans citation ici. Smith et al. le montrent aussi [@kone2021; @smith2024].\n% commentaire \\cite{x}\nEn LaTeX, voir \\autocite{abbas2026}.\n";
        let s = cited_sentences(t, 50);
        let pairs: Vec<(usize, &str)> = s.iter().map(|c| (c.line, c.citekey.as_str())).collect();
        assert_eq!(pairs, vec![(6, "smith2024"), (6, "kone2021"), (6, "smith2024"), (8, "abbas2026")]);
        assert!(s[0].sentence.ends_with("[@smith2024, p. 4]."), "{:?}", s[0]);
        assert!(s[1].sentence.starts_with("Smith et al. le montrent"));
        assert_eq!(cited_sentences(t, 2).len(), 2);
        // Link to a reading note's source: not a citation to check.
        assert!(cited_sentences("**Source :** [[@abbas2026|Titre]] · Abbas (2026)\n", 10).is_empty());
    }

    #[test]
    fn references_never_added() {
        let (masked, kept) = protect("Voir [@smith2024].");
        assert_eq!(restore_some(&masked, &kept).unwrap(), "Voir [@smith2024].");
        assert!(restore_some("Voir ⟦7⟧.", &kept).is_err());
        assert_eq!(cited("\\autocite[p.~4]{Smith2024} et [@kone2021]"), HashSet::from(["smith2024".to_string(), "kone2021".to_string()]));
    }
}
