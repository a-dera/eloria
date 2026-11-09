//! Eloria: application core.
//!
//! This crate does not depend on Tauri. All the business logic lives here, which makes
//! it testable with `cargo test -p eloria-core` and reusable (CLI, another UI).
//!
//! Modules :
//! - [`markdown`]: note parsing (front matter, links, tags, citations, headings)
//! - [`vault`]: disk access to the vault (tree, safe read/write, rename)
//! - [`index`]: SQLite index (FTS5, backlinks, tags, graph, database-style queries)
//! - [`history`]: note version snapshots
//! - [`zotero`]: local mirror of the Zotero library (SQLite or local API)
//! - [`fiche`]: reading notes and annotation sync
//! - [`fiche_ia`]: AI-written "Eloria method" reading note, with checked references
//! - [`ai`]: AI client (Anthropic, OpenAI-compatible), keys in the keychain, request log
//! - [`bib`]: BibTeX export
//! - [`export`]: HTML / Pandoc export
//! - [`lang`]: language of generated content (reading notes, daily note)
//! - [`scaffold`]: default layout of a new vault
//! - [`settings`]: application preferences
//! - [`themes`]: custom themes (`.eloria/themes/*.css`)
//! - [`workspace`]: facade grouping vault + index + Zotero mirror

pub mod ai;
pub mod argument;
pub mod assist;
pub mod backup;
pub mod capture;
pub mod bib;
pub mod bibimport;
pub mod entities;
pub mod disk;
pub mod error;
pub mod export;
pub mod extensions;
pub mod fiche;
pub mod fiche_ia;
pub mod flow;
pub mod git;
pub mod history;
pub mod index;
pub mod knowledge;
pub mod lang;
pub mod latex;
pub mod localapi;
pub mod manuscript;
pub mod markdown;
pub mod review;
pub mod matrix;
pub mod methods;
pub mod mcp;
pub mod net;
pub mod pdf_annot;
pub mod pdfimport;
pub mod quality;
pub mod reliability;
pub mod online;
pub mod outline;
pub mod scaffold;
pub mod semantic;
pub mod science;
pub mod settings;
pub mod similarity;
pub mod stats;
pub mod taxonomy;
pub mod texplus;
pub mod thesis;
pub mod topics;
pub mod themes;
pub mod vault;
pub mod workspace;
pub mod zotero;

pub use error::{Error, Result};
pub use workspace::Workspace;

/// Name of Eloria's internal folder, at the root of the vault.
pub const ELORIA_DIR: &str = ".eloria";
/// Eloria version (the core's, identical to the application's).
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
