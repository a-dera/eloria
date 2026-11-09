//! Rule-based entity recognition (no AI): people (authors of the library),
//! organisations (domain catalogue and recurring acronyms), methods (methodology
//! catalogue), technologies, places. Whole-word search, ignoring case and accents.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use crate::assist::fold;
use crate::methods::contains_term;

pub const ORGANISATIONS: &[(&str, &[&str])] = &[
    ("BCEAO", &["bceao", "banque centrale des etats de l'afrique de l'ouest"]),
    ("UEMOA", &["uemoa", "union economique et monetaire ouest-africaine", "waemu"]),
    ("CEDEAO", &["cedeao", "ecowas"]),
    ("Banque mondiale", &["banque mondiale", "world bank"]),
    ("FMI", &["fmi", "imf", "fonds monetaire international", "international monetary fund"]),
    ("Banque africaine de développement", &["banque africaine de developpement", "african development bank", "afdb"]),
    ("GSMA", &["gsma"]),
    ("Orange Money", &["orange money", "orange"]),
    ("MTN", &["mtn", "mtn mobile money", "momo"]),
    ("Wave", &["wave"]),
    ("Moov Africa", &["moov", "moov money", "moov africa"]),
    ("Airtel", &["airtel", "airtel money"]),
    ("M-Pesa", &["m-pesa", "mpesa"]),
    ("Safaricom", &["safaricom"]),
    ("Visa", &["visa"]),
    ("Mastercard", &["mastercard"]),
    ("GAFI", &["gafi", "fatf", "groupe d'action financiere"]),
    ("CENTIF", &["centif"]),
    ("ARCEP", &["arcep"]),
    ("Interpol", &["interpol"]),
    ("BRI", &["bri", "bis", "banque des reglements internationaux"]),
    ("CGAP", &["cgap"]),
];

pub const TECHNOLOGIES: &[(&str, &[&str])] = &[
    ("Python", &["python"]),
    ("R", &["langage r", "rstudio"]),
    ("PyTorch", &["pytorch"]),
    ("TensorFlow", &["tensorflow", "keras"]),
    ("scikit-learn", &["scikit-learn", "sklearn"]),
    ("PyTorch Geometric", &["pytorch geometric", "pyg"]),
    ("DGL", &["dgl", "deep graph library"]),
    ("NetworkX", &["networkx"]),
    ("Neo4j", &["neo4j"]),
    ("Apache Spark", &["spark", "pyspark"]),
    ("Kafka", &["kafka"]),
    ("SQL", &["sql", "postgresql", "mysql"]),
    ("pandas", &["pandas"]),
    ("Jupyter", &["jupyter"]),
    ("Docker", &["docker"]),
    ("USSD", &["ussd"]),
    ("SMS", &["sms"]),
    ("API", &["api", "apis"]),
    ("QR code", &["qr code", "code qr"]),
    ("NFC", &["nfc"]),
    ("Android", &["android"]),
    ("Stata", &["stata"]),
    ("SPSS", &["spss"]),
    ("Gephi", &["gephi"]),
    ("Cloud", &["cloud computing", "infonuagique"]),
];

pub const LIEUX: &[(&str, &[&str])] = &[
    ("Burkina Faso", &["burkina faso", "burkina"]),
    ("Côte d'Ivoire", &["cote d'ivoire", "ivory coast"]),
    ("Sénégal", &["senegal"]),
    ("Mali", &["mali"]),
    ("Niger", &["niger"]),
    ("Togo", &["togo"]),
    ("Bénin", &["benin"]),
    ("Guinée-Bissau", &["guinee-bissau", "guinea-bissau"]),
    ("Ghana", &["ghana"]),
    ("Nigeria", &["nigeria"]),
    ("Kenya", &["kenya"]),
    ("Ouganda", &["ouganda", "uganda"]),
    ("Tanzanie", &["tanzanie", "tanzania"]),
    ("Rwanda", &["rwanda"]),
    ("Cameroun", &["cameroun", "cameroon"]),
    ("Inde", &["inde", "india"]),
    ("Chine", &["chine", "china"]),
    ("Afrique de l'Ouest", &["afrique de l'ouest", "west africa"]),
    ("Afrique subsaharienne", &["afrique subsaharienne", "sub-saharan africa"]),
    ("Ouagadougou", &["ouagadougou"]),
];

/// Acronyms never taken for organisations (methods, units, common words).
const NOT_ORG: &[&str] = &["SHAP", "LIME", "GNN", "GNNS", "LSTM", "CNN", "RNN", "GAN", "SVM", "AUC", "ROC", "PR", "PDF", "DOI", "URL", "API", "SMS", "USSD", "NFC", "SQL", "IA", "AI", "ML", "DL", "XAI", "KYC", "AML", "TODO", "OK", "NB", "HTML", "CSV", "JSON", "UTF", "OTP", "PIN", "SIM", "GCN", "GAT", "SMOTE", "PRISMA", "SEM", "PLS", "RCT", "VAE", "BERT", "MCP", "TF", "IDF", "TF-IDF", "UE", "EU", "USA", "UK", "PIB", "GDP", "CFA", "FCFA", "XOF", "USD", "EUR"];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Personne,
    Organisation,
    Methode,
    Technologie,
    Lieu,
}

/// An entity and its recognised forms.
pub struct Entity {
    pub kind: Kind,
    pub name: String,
    pub terms: Vec<String>,
}

/// Catalogue: authors ("First Last" and "Last et al."), organisations, methods,
/// technologies, places, plus recurring acronyms (`acronyms`).
pub fn catalogue(authors: &[(String, String)], acronyms: &[String]) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (first, last) in authors {
        let last = last.trim();
        if last.chars().count() < 3 || !seen.insert(fold(last)) {
            continue;
        }
        let name = if first.trim().is_empty() { last.to_string() } else { format!("{} {last}", first.trim()) };
        let mut terms = vec![format!("{last} et al"), format!("{last} and")];
        if !first.trim().is_empty() {
            terms.push(name.clone());
            if let Some(i) = first.trim().chars().next() {
                terms.push(format!("{i}. {last}"));
            }
        }
        out.push(Entity { kind: Kind::Personne, name, terms });
    }
    let list = |kind: Kind, l: &[(&str, &[&str])], out: &mut Vec<Entity>| {
        for (n, a) in l {
            out.push(Entity { kind, name: n.to_string(), terms: a.iter().map(|x| x.to_string()).collect() });
        }
    };
    list(Kind::Organisation, ORGANISATIONS, &mut out);
    for m in crate::methods::builtin() {
        out.push(Entity { kind: Kind::Methode, name: m.name, terms: m.aliases });
    }
    list(Kind::Technologie, TECHNOLOGIES, &mut out);
    list(Kind::Lieu, LIEUX, &mut out);
    let known: HashSet<String> = out.iter().flat_map(|e| e.terms.iter().map(|t| fold(t))).collect();
    for a in acronyms {
        if !known.contains(&fold(a)) {
            out.push(Entity { kind: Kind::Organisation, name: a.clone(), terms: vec![a.clone()] });
        }
    }
    out
}

/// Acronyms (2 to 6 capitals) found in at least `min_notes` notes, except acronyms known
/// not to be organisations.
pub fn recurring_acronyms(texts: &[&str], min_notes: usize) -> Vec<String> {
    let re = regex::Regex::new(r"\b[A-Z][A-Z0-9]{1,5}\b").unwrap();
    // A word also written in lower case elsewhere ("fraud") is not an acronym, only
    // a title in capitals ("FRAUD DETECTION").
    let lower: HashSet<String> = texts
        .iter()
        .flat_map(|t| t.split(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.chars().next().map(|c| c.is_lowercase()).unwrap_or(false))
        .map(|w| w.to_lowercase())
        .collect();
    let mut count: HashMap<String, usize> = HashMap::new();
    for t in texts {
        let found: HashSet<&str> = re.find_iter(t).map(|m| m.as_str()).collect();
        for f in found {
            if !NOT_ORG.contains(&f) && f.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 2 && !lower.contains(&f.to_lowercase()) {
                *count.entry(f.to_string()).or_default() += 1;
            }
        }
    }
    let mut v: Vec<String> = count.into_iter().filter(|(_, n)| *n >= min_notes).map(|(a, _)| a).collect();
    v.sort();
    v
}

/// Whole word, case-sensitive.
fn has_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(k) = text[from..].find(word) {
        let a = from + k;
        let b = a + word.len();
        let before = text[..a].chars().next_back().map(|c| !c.is_alphanumeric()).unwrap_or(true);
        let after = text[b..].chars().next().map(|c| !c.is_alphanumeric()).unwrap_or(true);
        if before && after {
            return true;
        }
        from = a + word.len();
    }
    false
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Entities mentioned in a text (catalogue indices). Short forms (acronyms,
/// one-word names) are matched with their case: "wave" (a wave) is not Wave.
pub fn find(text: &str, cat: &[Entity]) -> Vec<usize> {
    let f = fold(text);
    // Fast pre-filter: every word of a term must appear in the note (hundreds of
    // authors × hundreds of notes, otherwise one search per term and per note).
    let words: HashSet<&str> = f.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let present = |term: &str| fold(term).split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).all(|w| words.contains(w));
    cat.iter()
        .enumerate()
        .filter(|(_, e)| {
            e.terms.iter().any(|t| {
                if !present(t) {
                    return false;
                }
                if !t.contains(' ') && matches!(e.kind, Kind::Organisation | Kind::Lieu | Kind::Technologie) {
                    has_word(text, &t.to_uppercase()) || (t.chars().count() > 3 && has_word(text, &capitalized(t))) || has_word(text, &e.name)
                } else {
                    contains_term(&f, t)
                }
            })
        })
        .map(|(i, _)| i)
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityRow {
    pub kind: Kind,
    pub name: String,
    /// Notes that mention it (path, title)
    pub notes: Vec<(String, String)>,
}

/// Groups mentions: (entity index, path, title) -> rows, most cited first.
pub fn rows(cat: &[Entity], hits: &[(usize, String, String)]) -> Vec<EntityRow> {
    let mut by: BTreeMap<usize, Vec<(String, String)>> = BTreeMap::new();
    for (i, p, t) in hits {
        let v = by.entry(*i).or_default();
        if !v.iter().any(|(x, _)| x == p) {
            v.push((p.clone(), t.clone()));
        }
    }
    let mut out: Vec<EntityRow> = by.into_iter().map(|(i, notes)| EntityRow { kind: cat[i].kind, name: cat[i].name.clone(), notes }).collect();
    out.sort_by(|a, b| b.notes.len().cmp(&a.notes.len()).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognised_entities() {
        let cat = catalogue(&[("Jane".into(), "Smith".into()), ("".into(), "BCEAO".into())], &["CENTIF".into(), "ONG".into()]);
        let text = "Une enquête par survey. Selon Smith et al. (2024), la BCEAO et l'UEMOA suivent Orange Money au Burkina Faso ; le modèle GNN est codé en Python avec PyTorch. L'ONG partenaire aussi.";
        let names: Vec<(Kind, String)> = find(text, &cat).into_iter().map(|i| (cat[i].kind, cat[i].name.clone())).collect();
        for want in [
            (Kind::Personne, "Jane Smith"),
            (Kind::Organisation, "BCEAO"),
            (Kind::Organisation, "UEMOA"),
            (Kind::Organisation, "Orange Money"),
            (Kind::Lieu, "Burkina Faso"),
            (Kind::Methode, "Réseaux de neurones sur graphes (GNN)"),
            (Kind::Technologie, "Python"),
            (Kind::Technologie, "PyTorch"),
            (Kind::Organisation, "ONG"),
            (Kind::Methode, "Enquête par questionnaire"),
        ] {
            assert!(names.contains(&(want.0, want.1.to_string())), "{want:?} absent de {names:?}");
        }
        // "wave" in lower case (a wave) is not the operator Wave; nor is "niger" inside a word.
        let t2 = "A new wave of fraud. Nigeria.";
        let n2: Vec<String> = find(t2, &cat).into_iter().map(|i| cat[i].name.clone()).collect();
        assert!(!n2.contains(&"Wave".to_string()) && !n2.contains(&"Niger".to_string()) && n2.contains(&"Nigeria".to_string()), "{n2:?}");
        assert_eq!(recurring_acronyms(&["La CENTIF et le SHAP.", "CENTIF encore.", "Une seule fois : ABC."], 2), vec!["CENTIF".to_string()]);
        // "FRAUD" in capitals is not an acronym when "fraud" exists in lower case.
        assert_eq!(recurring_acronyms(&["FRAUD DETECTION by IEEE.", "FRAUD and IEEE.", "A fraud case."], 2), vec!["IEEE".to_string()]);
    }
}
