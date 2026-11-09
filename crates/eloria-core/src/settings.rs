//! Application preferences (JSON file in the OS configuration folder).

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Result;

/// Custom assistant: a name and instructions added to Ask Eloria.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AiAgent {
    pub name: String,
    pub instructions: String,
}

/// User-defined typed property (suitable editor in the Properties panel).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct PropDef {
    pub name: String,
    /// texte, nombre, date, case, choix, liste, lien
    pub kind: String,
    /// Offered values ("choix" type)
    pub options: Vec<String>,
}

/// Saved view of the notes database (filters, sort order, hidden columns).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct SavedView {
    pub name: String,
    pub folder: String,
    pub type_filter: String,
    pub tag: String,
    pub text: String,
    pub sort_key: String,
    pub sort_dir: i32,
    pub hidden: Vec<String>,
}

/// Custom palette command.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct CustomCommand {
    pub name: String,
    /// modele (create a note from a template), texte (insert), vue, note, recherche
    pub kind: String,
    pub arg: String,
    /// Folder of the created note (modele)
    pub folder: String,
}

/// Extension approved for a vault, on a precise version of its code.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct ExtensionGrant {
    /// Vault path
    pub vault: String,
    pub id: String,
    /// SHA-256 fingerprint of the approved code
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Language of the interface and generated notes: "fr" or "en"
    pub language: String,
    /// "light", "dark", "sepia" or "system"
    pub theme: String,
    /// Accent colour (hex)
    pub accent: String,
    pub font_ui: String,
    pub font_editor: String,
    pub font_mono: String,
    /// Editor text size (px)
    pub font_size: f32,
    pub line_height: f32,
    /// Maximum width of the writing column (px), 0 = full width
    pub editor_width: u32,
    /// Heading scale (1.0 = default)
    pub heading_scale: f32,
    /// Live preview (otherwise: raw Markdown)
    pub live_preview: bool,
    pub spellcheck: bool,
    /// Spell-checker language ("fr", "en-US"...)
    pub spellcheck_lang: String,
    pub show_line_numbers: bool,
    pub readable_line_length: bool,
    /// Autosave (ms after the last keystroke)
    pub autosave_ms: u32,
    /// Minimum interval between two versions (seconds)
    pub history_interval: i64,
    /// Zotero data folder (contains zotero.sqlite). Empty = auto-detection.
    pub zotero_data_dir: String,
    /// Port of Zotero's local API
    pub zotero_port: u16,
    /// Sources folder (items imported from Zotero, collections kept)
    pub sources_folder: String,
    /// Reading-notes folder in the vault
    pub fiches_folder: String,
    /// Copy each item's PDF into the vault (reading note or import): the vault
    /// stays complete even if Zotero or its file storage disappears.
    pub copy_pdfs: bool,
    /// Resync the mirror as soon as Zotero changes (watching zotero.sqlite)
    pub zotero_auto_sync: bool,
    /// Folder of the PDFs copied into the vault
    pub pdf_folder: String,
    /// Attachments folder (pasted images)
    pub attachments_folder: String,
    /// Templates folder
    pub templates_folder: String,
    /// Daily journal folder
    pub daily_folder: String,
    /// Path to pandoc (empty = PATH)
    pub pandoc_path: String,
    /// LaTeX engine (tectonic, latexmk or pdflatex); empty = automatic search
    pub latex_path: String,
    /// Goals: reading notes per week, words written per day (0 = no goal)
    pub goal_fiches_week: u32,
    pub goal_words_day: u32,
    /// Length of a focus session, in minutes
    pub focus_minutes: u32,
    /// Local API (127.0.0.1) for web capture: disabled by default
    pub local_api: bool,
    pub local_api_port: u16,
    /// Token required by the local API (generated on activation)
    pub local_api_token: String,
    /// Inbox: captures land here before being filed
    pub inbox_folder: String,
    /// Reading chain: the status of reading notes moves forward on its own (to read, in progress, read, used)
    pub auto_reading_status: bool,
    /// Custom Ask Eloria assistants (name, instructions)
    pub ai_agents: Vec<AiAgent>,
    /// Semantic search: embedding provider (openai, voyage, ollama; empty = disabled)
    pub embed_provider: String,
    pub embed_model: String,
    /// Service address (empty = the provider's)
    pub embed_url: String,
    /// CSL file for citations (empty = Pandoc's default style)
    pub csl_path: String,
    /// AI enabled (disabled until the user chooses it)
    pub ai_enabled: bool,
    /// "anthropic", "openai" or "compatible"
    pub ai_provider: String,
    /// Model (empty = the provider's default model)
    pub ai_model: String,
    /// Address of an OpenAI-compatible service (e.g. http://localhost:11434/v1 for Ollama)
    pub ai_base_url: String,
    /// Thinking effort (Anthropic): low, medium, high, xhigh, max
    pub ai_effort: String,
    /// Research topic, given to the AI to judge how relevant an article is
    pub research_topic: String,
    /// Custom shortcuts: command identifier -> keys ("Mod+Shift+K"), "" = disabled
    pub keys: std::collections::BTreeMap<String, String>,
    /// Folder for automatic backups (outside the vault); empty = disabled
    pub backup_dir: String,
    /// Interval between two backups, in hours
    pub backup_interval_hours: u32,
    /// Number of archives kept (the oldest are deleted)
    pub backup_keep: u32,
    pub recent_vaults: Vec<String>,
    pub last_vault: Option<String>,
    /// Typed properties
    pub property_defs: Vec<PropDef>,
    /// Saved views of the notes database
    pub saved_views: Vec<SavedView>,
    /// Custom palette commands
    pub custom_commands: Vec<CustomCommand>,
    /// Approved extensions (per vault and per code fingerprint)
    pub extension_grants: Vec<ExtensionGrant>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "fr".into(),
            theme: "system".into(),
            accent: "#7c5cff".into(),
            font_ui: "\"Inter Variable\", Inter, \"Segoe UI\", system-ui, sans-serif".into(),
            font_editor: "\"Literata Variable\", Literata, Georgia, serif".into(),
            font_mono: "\"JetBrains Mono Variable\", \"JetBrains Mono\", Consolas, monospace".into(),
            font_size: 17.0,
            line_height: 1.7,
            editor_width: 760,
            heading_scale: 1.0,
            live_preview: true,
            spellcheck: true,
            spellcheck_lang: "fr".into(),
            show_line_numbers: false,
            readable_line_length: true,
            autosave_ms: 800,
            history_interval: 300,
            zotero_data_dir: String::new(),
            zotero_port: 23119,
            sources_folder: "Sources".into(),
            fiches_folder: "Fiches de lecture".into(),
            copy_pdfs: true,
            zotero_auto_sync: true,
            pdf_folder: "PDF".into(),
            attachments_folder: "Pièces jointes".into(),
            templates_folder: "Modèles".into(),
            daily_folder: "Journal".into(),
            pandoc_path: String::new(),
            latex_path: String::new(),
            embed_provider: String::new(),
            auto_reading_status: true,
            inbox_folder: "Boîte de réception".into(),
            goal_fiches_week: 3,
            goal_words_day: 500,
            focus_minutes: 25,
            local_api: false,
            local_api_port: 27123,
            local_api_token: String::new(),
            ai_agents: vec![
                AiAgent { name: "Relecteur de chapitre".into(), instructions: "Relis comme un membre exigeant du jury : cohérence de l'argumentation, affirmations non étayées, sources manquantes, en citant les extraits.".into() },
                AiAgent { name: "Méthodologue".into(), instructions: "Concentre-toi sur les méthodes, les données, les protocoles d'évaluation et leurs limites ; compare-les entre les extraits.".into() },
            ],
            embed_model: String::new(),
            embed_url: String::new(),
            csl_path: String::new(),
            ai_enabled: false,
            ai_provider: "anthropic".into(),
            ai_model: String::new(),
            ai_base_url: String::new(),
            ai_effort: "high".into(),
            research_topic: String::new(),
            keys: Default::default(),
            recent_vaults: vec![],
            last_vault: None,
            backup_dir: String::new(),
            backup_interval_hours: 24,
            backup_keep: 10,
            property_defs: vec![],
            saved_views: vec![],
            custom_commands: vec![],
            extension_grants: vec![],
        }
    }
}

impl Settings {
    /// Language of generated notes.
    pub fn lang(&self) -> crate::lang::Lang {
        crate::lang::Lang::from_code(&self.language)
    }

    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(p) = path.parent() {
            fs::create_dir_all(p)?;
        }
        fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn remember_vault(&mut self, path: &str) {
        self.recent_vaults.retain(|p| p != path);
        self.recent_vaults.insert(0, path.to_string());
        self.recent_vaults.truncate(8);
        self.last_vault = Some(path.to_string());
    }
}

#[cfg(test)]
mod tests_personnalisation {
    use super::*;

    #[test]
    fn customization_kept() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("settings.json");
        // An old file, without these fields, can still be read.
        std::fs::write(&path, r#"{"language":"fr","theme":"dark"}"#).unwrap();
        let mut s = Settings::load(&path);
        assert_eq!(s.theme, "dark");
        s.property_defs.push(PropDef { name: "statut".into(), kind: "choix".into(), options: vec!["à lire".into(), "lue".into()] });
        s.saved_views.push(SavedView { name: "Fiches à lire".into(), type_filter: "fiche".into(), sort_key: "annee".into(), sort_dir: -1, ..Default::default() });
        s.custom_commands.push(CustomCommand { name: "Nouvelle réunion".into(), kind: "modele".into(), arg: "Réunion".into(), folder: "Réunions".into() });
        s.save(&path).unwrap();
        let back = Settings::load(&path);
        assert_eq!(back.property_defs, s.property_defs);
        assert_eq!(back.saved_views[0].sort_dir, -1);
        assert_eq!(back.custom_commands[0].folder, "Réunions");
        // An old file with profiles (removed feature) is read without error.
        std::fs::write(&path, r#"{"theme":"dark","profiles":[{"name":"Écriture"}],"active_profile":"Écriture"}"#).unwrap();
        assert_eq!(Settings::load(&path).theme, "dark");
    }
}
