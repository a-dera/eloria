use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Erreur d'entrée/sortie : {0}")]
    Io(#[from] std::io::Error),
    #[error("Erreur de base de données : {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("Erreur JSON : {0}")]
    Json(#[from] serde_json::Error),
    #[error("Erreur YAML : {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Erreur de modèle : {0}")]
    Template(#[from] minijinja::Error),
    #[error("Chemin invalide ou hors du coffre : {0}")]
    InvalidPath(String),
    #[error("Introuvable : {0}")]
    NotFound(String),
    #[error("Existe déjà : {0}")]
    AlreadyExists(String),
    #[error("Aucun coffre ouvert")]
    NoVault,
    #[error("Zotero : {0}")]
    Zotero(String),
    #[error("Export : {0}")]
    Export(String),
    #[error("IA : {0}")]
    Ai(String),
    #[error("Réseau : {0}")]
    Net(String),
    #[error("Extension : {0}")]
    Extension(String),
}

pub type Result<T> = std::result::Result<T, Error>;

// Errors cross the Tauri boundary as a readable string.
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
