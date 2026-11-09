//! Methodology tracking: which methods your documents use. Built-in catalogue
//! (machine learning, fraud detection, survey methods) completed by your
//! method notes (`type: methode`, with their aliases). Search without AI, on whole words.

use serde::Serialize;

use crate::assist::fold;

/// Common methods: (name, aliases). Aliases are matched as whole words, ignoring case
/// ni accents.
pub const KNOWN: &[(&str, &[&str])] = &[
    ("Réseaux de neurones sur graphes (GNN)", &["gnn", "gnns", "graph neural network", "graph neural networks", "graph convolutional", "gcn", "graphsage", "graph attention network", "gat"]),
    ("Forêt aléatoire", &["random forest", "random forests", "forêt aléatoire", "forêts aléatoires"]),
    ("Gradient boosting (XGBoost, LightGBM)", &["xgboost", "lightgbm", "catboost", "gradient boosting", "gradient boosted"]),
    ("Régression logistique", &["logistic regression", "régression logistique"]),
    ("Machines à vecteurs de support (SVM)", &["svm", "support vector machine", "support vector machines"]),
    ("Arbre de décision", &["decision tree", "decision trees", "arbre de décision", "arbres de décision"]),
    ("k plus proches voisins (k-NN)", &["knn", "k-nn", "k-nearest neighbors", "k nearest neighbours"]),
    ("Réseaux récurrents (LSTM, GRU)", &["lstm", "gru", "recurrent neural network", "rnn"]),
    ("Réseaux convolutifs (CNN)", &["cnn", "convolutional neural network", "convolutional neural networks"]),
    ("Transformeurs et attention", &["transformer", "transformers", "self-attention", "bert"]),
    ("Autoencodeurs", &["autoencoder", "autoencoders", "auto-encoder", "variational autoencoder", "vae"]),
    ("Réseaux antagonistes génératifs (GAN)", &["gan", "gans", "generative adversarial network", "generative adversarial networks"]),
    ("Isolation forest", &["isolation forest"]),
    ("Clustering (k-means, DBSCAN)", &["k-means", "kmeans", "dbscan", "clustering", "hierarchical clustering"]),
    ("Apprentissage fédéré", &["federated learning", "apprentissage fédéré"]),
    ("Apprentissage par renforcement", &["reinforcement learning", "apprentissage par renforcement"]),
    ("Apprentissage semi-supervisé", &["semi-supervised", "semi supervised"]),
    ("Rééchantillonnage (SMOTE)", &["smote", "oversampling", "undersampling", "class imbalance", "imbalanced data"]),
    ("SHAP", &["shap", "shapley additive explanations", "shapley values"]),
    ("LIME", &["lime", "local interpretable model-agnostic explanations"]),
    ("Détection d'anomalies", &["anomaly detection", "outlier detection", "détection d'anomalies"]),
    ("Analyse de réseaux sociaux", &["social network analysis", "network analysis", "analyse de réseaux"]),
    ("Règles et systèmes experts", &["rule-based", "expert system", "expert systems"]),
    ("Revue systématique de la littérature", &["systematic review", "systematic literature review", "prisma", "revue systématique"]),
    ("Méta-analyse", &["meta-analysis", "méta-analyse"]),
    ("Enquête par questionnaire", &["survey", "questionnaire", "questionnaires", "enquête"]),
    ("Entretiens", &["interviews", "semi-structured interviews", "entretiens", "focus group", "focus groups"]),
    ("Étude de cas", &["case study", "case studies", "étude de cas"]),
    ("Modèles d'équations structurelles (SEM, PLS)", &["structural equation modeling", "structural equation modelling", "sem", "pls-sem"]),
    ("Économétrie (régression, panel)", &["panel data", "econometric", "regression analysis", "difference-in-differences", "instrumental variables"]),
    ("Expérimentation contrôlée", &["randomized controlled trial", "rct", "field experiment", "a/b test"]),
    ("Simulation", &["simulation", "agent-based model", "agent-based simulation", "monte carlo"]),
    ("Blockchain", &["blockchain", "smart contract", "smart contracts"]),
];

/// A method and its aliases (catalogue or method note).
#[derive(Debug, Clone)]
pub struct Method {
    pub name: String,
    pub aliases: Vec<String>,
    /// Vault method note, if any
    pub path: Option<String>,
}

/// Built-in catalogue, as methods.
pub fn builtin() -> Vec<Method> {
    KNOWN.iter().map(|(n, a)| Method { name: n.to_string(), aliases: a.iter().map(|x| x.to_string()).collect(), path: None }).collect()
}

/// Does the text contain this term as whole words (ignoring case and accents)?
pub fn contains_term(folded_text: &str, term: &str) -> bool {
    let t = fold(term.trim());
    if t.is_empty() {
        return false;
    }
    let bytes = folded_text.as_bytes();
    let mut from = 0;
    while let Some(i) = folded_text[from..].find(&t) {
        let start = from + i;
        let end = start + t.len();
        let before = start == 0 || !is_word(folded_text[..start].chars().next_back().unwrap());
        let after = end >= bytes.len() || !is_word(folded_text[end..].chars().next().unwrap());
        if before && after {
            return true;
        }
        from = start + t.len().max(1);
        if from >= folded_text.len() {
            break;
        }
    }
    false
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

/// Methods used in a text (indices into `methods`).
pub fn find(text: &str, methods: &[Method]) -> Vec<usize> {
    let f = fold(text);
    methods.iter().enumerate().filter(|(_, m)| contains_term(&f, &m.name) || m.aliases.iter().any(|a| contains_term(&f, a))).map(|(i, _)| i).collect()
}

/// A document that uses a method.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MethodUse {
    pub citekey: String,
    pub title: String,
    /// Reading note if any, otherwise the source
    pub path: String,
    pub year: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MethodRow {
    pub name: String,
    pub aliases: Vec<String>,
    /// Vault method note (empty: catalogue only)
    pub path: String,
    pub documents: Vec<MethodUse>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methods_found_as_whole_words() {
        let m = builtin();
        let name = |i: usize| m[i].name.clone();
        let found: Vec<String> = find("We train a GNN and compare it with XGBoost; explanations use SHAP values. Data: an online survey.", &m).into_iter().map(name).collect();
        assert!(found.contains(&"Réseaux de neurones sur graphes (GNN)".to_string()));
        assert!(found.contains(&"Gradient boosting (XGBoost, LightGBM)".to_string()));
        assert!(found.contains(&"SHAP".to_string()));
        assert!(found.contains(&"Enquête par questionnaire".to_string()));
        // "semantic" does not trigger SEM, nor "slime" LIME.
        assert!(find("A semantic layer covered in slime.", &m).is_empty());
        // Accents and case ignored.
        assert!(contains_term(&fold("Une Revue Systématique PRISMA"), "revue systématique"));
        let mine = vec![Method { name: "Graphe de transactions".into(), aliases: vec!["transaction graph".into()], path: Some("Méthodes/Graphe.md".into()) }];
        assert_eq!(find("We build a transaction graph.", &mine), vec![0]);
    }
}
