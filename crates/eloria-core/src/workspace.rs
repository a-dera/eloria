//! Facade: an open vault = files + index + Zotero mirror.
//! Every operation that changes files keeps the index up to date.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::export::{self, ExportOptions};
use crate::fiche;
use crate::fiche_ia;
use crate::history;
use crate::index::{Index, SyncStats};
use crate::lang::Lang;
use crate::markdown::{self, set_property};
use crate::online;
use crate::assist;
use crate::matrix;
use crate::review;
use crate::latex;
use crate::semantic;
use crate::pdf_annot::{self, PdfAnnotation};
use crate::settings::Settings;
use crate::vault::{self, Vault};
use crate::zotero::{self, ItemDetail, MirrorStatus, Source};
use crate::{bib, Error, Result};

pub struct Workspace {
    pub vault: Vault,
    pub index: Index,
}

/// Result of a BibTeX import.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BibImportReport {
    pub created: usize,
    pub existing: usize,
    pub failed: Vec<String>,
}

/// Node of the knowledge map.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MapNode {
    pub id: String,
    pub title: String,
    /// concept, methode, question, chapitre, argument, source, fiche
    pub kind: String,
    pub degree: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct KnowledgeMap {
    pub nodes: Vec<MapNode>,
    /// (a, b, "lien" or "mention")
    pub edges: Vec<(String, String, String)>,
}

/// A taxonomy term and the notes filed under it (or its sub-terms).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TaxNodeInfo {
    #[serde(flatten)]
    pub node: crate::taxonomy::TaxNode,
    pub notes: Vec<(String, String)>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaxonomyOverview {
    pub path: String,
    pub exists: bool,
    pub nodes: Vec<TaxNodeInfo>,
}

/// A row of the library reliability report.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReliabilityRow {
    pub citekey: String,
    /// Zotero key (empty: Eloria's own source)
    pub key: String,
    pub title: String,
    /// Vault source note (empty: Zotero item not imported yet)
    pub path: String,
    pub doi: String,
    pub level: crate::reliability::Level,
    pub signals: Vec<crate::reliability::Signal>,
    /// Cited in at least one vault note
    pub cited: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NetNode {
    pub citekey: String,
    pub title: String,
    pub year: Option<i32>,
    pub label: String,
    /// References already fetched for this document
    pub fetched: bool,
}

/// Citation network of the library.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CitationNetwork {
    pub nodes: Vec<NetNode>,
    /// (cites, is cited)
    pub edges: Vec<(String, String)>,
    /// DOIs whose references are still to fetch
    pub missing: Vec<String>,
}

/// Natural sort key: "ch2" before "ch10".
fn natural_key(s: &str) -> (String, u64, String) {
    let digits: String = s.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    let prefix: String = s.chars().take_while(|c| !c.is_ascii_digit()).collect();
    (prefix.to_lowercase(), digits.parse().unwrap_or(0), s.to_lowercase())
}

/// Dashboard: where the thesis stands, what is left to read, what was written.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Dashboard {
    pub notes: usize,
    pub words: i64,
    pub sources: usize,
    /// Reading notes by status (to read, in progress, read, used)
    pub fiches: BTreeMap<String, usize>,
    /// Words written over the last 14 days (date, words), oldest to newest
    pub written: Vec<(String, u64)>,
    pub written_today: u64,
    /// Reading notes moved to "lue" or "exploitée" this week (by modification date)
    pub fiches_read_week: usize,
    /// Focus minutes today and over 7 days
    pub focus_today: u64,
    pub focus_week: u64,
    /// Notes to review today or overdue (path, title, due date)
    pub reviews_due: Vec<(String, String, String)>,
    /// Chapters: (path, title, words, goal)
    pub chapters: Vec<(String, String, i64, i64)>,
    /// Reading notes to read (path, title), highest priority first
    pub to_read: Vec<(String, String)>,
    /// Recently modified notes (path, title, date in ms)
    pub recent: Vec<(String, String, i64)>,
    /// AI tokens used this month (read, written)
    pub ai_tokens: (u64, u64),
}

/// A problem found by the integrity check.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct Issue {
    pub path: String,
    /// Line concerned (1 = first), 0 if not applicable
    pub line: i64,
    /// What is missing or wrong (link target, file, error message…)
    pub detail: String,
}

/// Integrity check report of the vault.
#[derive(Debug, Clone, Default, serde::Serialize, PartialEq)]
pub struct IntegrityReport {
    pub notes: usize,
    /// `[[…]]` links to a note that does not exist (excluding templates)
    pub broken_links: Vec<Issue>,
    /// Missing images, PDFs and attachments (`![[…]]`, `pdf` property)
    pub missing_files: Vec<Issue>,
    /// Unreadable properties (YAML front matter)
    pub bad_frontmatter: Vec<Issue>,
    /// Sources whose item is no longer in Zotero (the vault keeps them)
    pub orphan_sources: Vec<Issue>,
    /// Reading notes whose source no longer exists
    pub fiches_without_source: Vec<Issue>,
}

impl IntegrityReport {
    pub fn total(&self) -> usize {
        self.broken_links.len() + self.missing_files.len() + self.bad_frontmatter.len() + self.orphan_sources.len() + self.fiches_without_source.len()
    }
}

/// Embedding of a note in another (read-only preview).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Embed {
    pub path: String,
    pub title: String,
    pub html: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteFile {
    pub path: String,
    pub content: String,
    pub mtime: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameResult {
    pub path: String,
    /// Notes whose links were updated
    pub updated: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FicheResult {
    pub path: String,
    pub created: bool,
}

/// Summary of an import of the Zotero library into notes.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ImportReport {
    /// Notes created
    pub created: usize,
    /// Items that already had their note (left untouched)
    pub skipped: usize,
    /// PDFs copied into the vault
    pub pdfs: usize,
    /// Existing notes completed (item tags added)
    #[serde(default)]
    pub updated: usize,
    /// Failed items, with the reason
    pub failed: Vec<String>,
}

/// Import preview: how many items are new.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ImportPreview {
    pub total: usize,
    pub existing: usize,
    pub new: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultStats {
    pub notes: i64,
    pub words: i64,
    pub fiches: i64,
    pub tags: i64,
}

impl Workspace {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let vault = Vault::open(root)?;
        let index = Index::open(&vault.eloria_dir().join("index.db"))?;
        zotero::init_mirror(&index.conn)?;
        let ws = Self { vault, index };
        ws.index.sync(&ws.vault)?;
        Ok(ws)
    }

    pub fn root(&self) -> &Path {
        self.vault.root()
    }

    /// Empty folder: installs the default layout (folders, templates, welcome note)
    /// and indexes it. Returns the created paths; nothing for a vault that already has content.
    pub fn init_if_blank(&self, settings: &Settings) -> Result<Vec<String>> {
        if !crate::scaffold::is_blank(&self.vault)? {
            return Ok(vec![]);
        }
        let created = crate::scaffold::scaffold(&self.vault, settings)?;
        self.index.sync(&self.vault)?;
        Ok(created)
    }

    pub fn sync(&self) -> Result<SyncStats> {
        self.index.sync(&self.vault)
    }

    pub fn rebuild(&self) -> Result<SyncStats> {
        self.index.rebuild(&self.vault)
    }

    // ------------------------------------------------------------------ notes

    pub fn read_note(&self, rel: &str) -> Result<NoteFile> {
        Ok(NoteFile { path: rel.to_string(), content: self.vault.read(rel)?, mtime: self.vault.mtime(rel)? })
    }

    /// Saves a note, updates the index and takes a history snapshot if needed.
    pub fn save_note(&self, rel: &str, content: &str, history_interval: i64) -> Result<i64> {
        let before = self.index.note_meta(rel)?.map(|n| n.words).unwrap_or(0);
        self.vault.write(rel, content)?;
        let m = self.vault.mtime(rel)?;
        self.index.update_note(rel, content, m)?;
        // Writing statistics: words added today (deletions do not count).
        let after = self.index.note_meta(rel)?.map(|n| n.words).unwrap_or(0);
        if after > before {
            let _ = self.add_written((after - before) as u64);
        }
        let _ = history::snapshot(&self.vault, rel, content, history_interval, false);
        Ok(m)
    }

    fn focus_path(&self) -> std::path::PathBuf {
        self.vault.eloria_dir().join("focus.json")
    }

    /// Focus minutes per day.
    pub fn focus_stats(&self) -> BTreeMap<String, u64> {
        std::fs::read_to_string(self.focus_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    /// Records a finished focus session (minutes).
    pub fn add_focus(&self, minutes: u64) -> Result<u64> {
        let mut f = self.focus_stats();
        let day = chrono::Local::now().format("%Y-%m-%d").to_string();
        let v = f.entry(day).or_insert(0);
        *v += minutes;
        let total = *v;
        std::fs::write(self.focus_path(), serde_json::to_string(&f)?)?;
        Ok(total)
    }

    /// Schedules a review of a note in `days` days (`revoir_le` property);
    /// 0 removes the review.
    pub fn schedule_review(&self, rel: &str, days: i64, history_interval: i64) -> Result<Option<String>> {
        if days <= 0 {
            let c = self.vault.read(rel)?;
            let mut note = markdown::parse_note(&c).frontmatter;
            if note.remove("revoir_le").is_some() {
                self.set_property(rel, "revoir_le", Value::Null, history_interval)?;
            }
            return Ok(None);
        }
        let date = (chrono::Local::now().date_naive() + chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
        self.set_property(rel, "revoir_le", Value::String(date.clone()), history_interval)?;
        Ok(Some(date))
    }

    /// Weekly review: a journal note with the week's figures (words,
    /// reading notes read, notes created, inbox) and questions for what comes next.
    pub fn weekly_review(&self, settings: &Settings) -> Result<String> {
        let lang = settings.lang();
        let now = chrono::Local::now();
        let iso = chrono::Datelike::iso_week(&now);
        let name = format!("{} {}-S{:02}", lang.pick("Revue de la semaine", "Weekly review"), iso.year(), iso.week());
        let folder = settings.daily_folder.trim_matches('/');
        let rel = if folder.is_empty() { format!("{name}.md") } else { format!("{folder}/{name}.md") };
        if self.vault.exists(&rel) {
            return Ok(rel);
        }
        let d = self.dashboard()?;
        let week_ago = now.timestamp_millis() - 7 * 86_400_000;
        let notes = self.index.all_notes()?;
        let changed: Vec<&crate::index::NoteMeta> = notes.iter().filter(|n| n.mtime >= week_ago).collect();
        let words: u64 = d.written.iter().rev().take(7).map(|w| w.1).sum();
        let inbox = self.inbox(settings)?.len();
        let list = |v: &[(String, String)]| v.iter().take(8).map(|(p, t)| format!("- [[{}|{}]]", vault::stem(p), t.replace('|', "-"))).collect::<Vec<_>>().join("\n");
        let changed_list: Vec<(String, String)> = changed.iter().take(12).map(|n| (n.path.clone(), n.title.clone())).collect();
        let content = match lang {
            Lang::Fr => format!(
                "---\ntype: revue-hebdo\nsemaine: {}-S{:02}\ntags: [revue-hebdo]\n---\n# {name}\n\n## En chiffres\n\n- Mots écrits : **{words}** (objectif : {} par jour)\n- Fiches lues ou exploitées : **{}** (objectif : {})\n- Concentration : **{} min**\n- Notes modifiées : **{}**\n- À classer dans la boîte de réception : **{inbox}**\n\n## Notes travaillées\n\n{}\n\n## À lire ensuite\n\n{}\n\n## Ce qui a avancé\n\n\n## Ce qui a bloqué\n\n\n## Objectifs de la semaine prochaine\n\n- [ ] \n",
                iso.year(), iso.week(), settings.goal_words_day, d.fiches_read_week, settings.goal_fiches_week, d.focus_week, changed.len(), list(&changed_list), list(&d.to_read),
            ),
            Lang::En => format!(
                "---\ntype: revue-hebdo\nsemaine: {}-W{:02}\ntags: [weekly-review]\n---\n# {name}\n\n## In numbers\n\n- Words written: **{words}** (goal: {} per day)\n- Notes read or used: **{}** (goal: {})\n- Focus: **{} min**\n- Notes modified: **{}**\n- To file in the inbox: **{inbox}**\n\n## Notes worked on\n\n{}\n\n## Read next\n\n{}\n\n## What moved forward\n\n\n## What got stuck\n\n\n## Goals for next week\n\n- [ ] \n",
                iso.year(), iso.week(), settings.goal_words_day, d.fiches_read_week, settings.goal_fiches_week, d.focus_week, changed.len(), list(&changed_list), list(&d.to_read),
            ),
        };
        self.vault.create_note(&rel, &content)?;
        self.index.upsert(&rel, &content, self.vault.mtime(&rel)?)?;
        self.index.resolve_links()?;
        Ok(rel)
    }

    fn stats_path(&self) -> std::path::PathBuf {
        self.vault.eloria_dir().join("stats.json")
    }

    /// Capture (clipboard, web page, idea): a note in the inbox.
    pub fn capture(&self, text: &str, url: Option<&str>, settings: &Settings) -> Result<String> {
        let folder = settings.inbox_folder.trim_matches('/');
        let title = crate::capture::capture_title(text, settings.lang());
        let content = crate::capture::capture_note(text, url);
        let rel = self.vault.unique_path(folder, &title, "md");
        self.vault.create_note(&rel, &content)?;
        self.index.upsert(&rel, &content, self.vault.mtime(&rel)?)?;
        Ok(rel)
    }

    /// Inbox notes, most recent first.
    pub fn inbox(&self, settings: &Settings) -> Result<Vec<(String, String, i64)>> {
        let prefix = format!("{}/", settings.inbox_folder.trim_matches('/'));
        let mut v: Vec<_> = self.index.all_notes()?.into_iter().filter(|n| n.path.starts_with(&prefix)).map(|n| (n.path, n.title, n.mtime)).collect();
        v.sort_by(|a, b| b.2.cmp(&a.2));
        Ok(v)
    }

    /// Imports a Markdown folder (Obsidian, Notion, Logseq…) into "Importé/<name>".
    pub fn import_markdown(&self, dir: &std::path::Path, settings: &Settings) -> Result<crate::capture::ImportReport> {
        let name = dir.file_name().map(|n| crate::capture::strip_notion_id(&n.to_string_lossy())).unwrap_or_else(|| "import".into());
        let base = settings.lang().pick("Importé", "Imported");
        let r = crate::capture::import_markdown_dir(&self.vault, dir, &format!("{base}/{name}"))?;
        self.index.sync(&self.vault)?;
        Ok(r)
    }

    /// Words written per day (YYYY-MM-DD -> words added).
    pub fn writing_stats(&self) -> BTreeMap<String, u64> {
        std::fs::read_to_string(self.stats_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    fn add_written(&self, n: u64) -> Result<()> {
        let mut st = self.writing_stats();
        *st.entry(chrono::Local::now().format("%Y-%m-%d").to_string()).or_insert(0) += n;
        std::fs::write(self.stats_path(), serde_json::to_string(&st)?)?;
        Ok(())
    }

    /// Reading chain, after a note is saved: a reading note moves forward according to its
    /// content (in progress, read); a chapter that cites an item moves its read reading
    /// notes to "exploitée". Never backwards. Returns the modified notes.
    pub fn reading_chain(&self, rel: &str, settings: &Settings) -> Result<Vec<String>> {
        if !settings.auto_reading_status {
            return Ok(vec![]);
        }
        let content = self.vault.read(rel)?;
        let fm = markdown::parse_note(&content).frontmatter;
        let kind = fm.get("type").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let mut updated = Vec::new();
        let mut bump = |path: &str, text: &str, target: &str| -> Result<()> {
            let cur = markdown::parse_note(text).frontmatter.get("statut").and_then(|v| v.as_str()).unwrap_or("à lire").to_string();
            if fiche::status_rank(target) > fiche::status_rank(&cur) {
                self.set_property(path, "statut", Value::String(target.into()), settings.history_interval)?;
                updated.push(path.to_string());
            }
            Ok(())
        };
        if kind == "fiche" {
            if let Some(st) = fiche::reading_status(&content) {
                bump(rel, &content, st)?;
            }
        } else if kind == "chapitre" {
            let cited: HashSet<String> = self.index.citekeys_of(rel)?.into_iter().map(|k| k.to_lowercase()).collect();
            for n in self.index.all_notes()? {
                let ck = n.frontmatter.get("citekey").and_then(|v| v.as_str()).map(str::to_lowercase).unwrap_or_default();
                if n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("fiche") && cited.contains(&ck) && n.frontmatter.get("statut").and_then(|v| v.as_str()) == Some("lue") {
                    let t = self.vault.read(&n.path)?;
                    bump(&n.path, &t, "exploitée")?;
                }
            }
        }
        Ok(updated)
    }

    pub fn dashboard(&self) -> Result<Dashboard> {
        let notes = self.index.all_notes()?;
        let mut d = Dashboard { notes: notes.len(), ..Default::default() };
        for st in ["à lire", "en cours", "lue", "exploitée"] {
            d.fiches.insert(st.into(), 0);
        }
        let mut to_read: Vec<(u8, String, String)> = Vec::new();
        for n in &notes {
            d.words += n.words;
            let s = |k: &str| n.frontmatter.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
            match s("type").as_str() {
                "source" => d.sources += 1,
                "fiche" => {
                    let st = if s("statut").is_empty() { "à lire".to_string() } else { s("statut") };
                    *d.fiches.entry(st.clone()).or_insert(0) += 1;
                    if st == "à lire" {
                        let prio = match s("priorite").as_str() {
                            "haute" => 0,
                            "moyenne" => 1,
                            _ => 2,
                        };
                        to_read.push((prio, n.path.clone(), n.title.clone()));
                    }
                }
                "chapitre" => {
                    let target = n.frontmatter.get("objectif").and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|x| x.parse().ok()))).unwrap_or(0);
                    d.chapters.push((n.path.clone(), n.title.clone(), n.words, target));
                }
                _ => {}
            }
        }
        to_read.sort();
        d.to_read = to_read.into_iter().take(8).map(|(_, p, t)| (p, t)).collect();
        d.chapters.sort_by(|a, b| a.1.cmp(&b.1));
        let mut recent: Vec<_> = notes.iter().map(|n| (n.path.clone(), n.title.clone(), n.mtime)).collect();
        recent.sort_by(|a, b| b.2.cmp(&a.2));
        d.recent = recent.into_iter().take(8).collect();
        let stats = self.writing_stats();
        let today = chrono::Local::now().date_naive();
        for i in (0..14).rev() {
            let day = (today - chrono::Duration::days(i)).format("%Y-%m-%d").to_string();
            d.written.push((day.clone(), stats.get(&day).copied().unwrap_or(0)));
        }
        d.written_today = d.written.last().map(|x| x.1).unwrap_or(0);
        let week_ago = chrono::Local::now().timestamp_millis() - 7 * 86_400_000;
        let today_s = today.format("%Y-%m-%d").to_string();
        for n in &notes {
            let st = n.frontmatter.get("statut").and_then(|v| v.as_str()).unwrap_or_default();
            if n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("fiche") && matches!(st, "lue" | "exploitée") && n.mtime >= week_ago {
                d.fiches_read_week += 1;
            }
            if let Some(due) = n.frontmatter.get("revoir_le").and_then(|v| v.as_str()) {
                if due <= today_s.as_str() {
                    d.reviews_due.push((n.path.clone(), n.title.clone(), due.to_string()));
                }
            }
        }
        d.reviews_due.sort_by(|a, b| a.2.cmp(&b.2));
        let focus = self.focus_stats();
        d.focus_today = focus.get(&today_s).copied().unwrap_or(0);
        d.focus_week = d.written.iter().map(|(day, _)| focus.get(day).copied().unwrap_or(0)).rev().take(7).sum();
        let month = chrono::Local::now().format("%Y-%m").to_string();
        for e in crate::ai::read_log(&self.vault, 10_000).unwrap_or_default() {
            if e.date.starts_with(&month) {
                d.ai_tokens.0 += e.input_tokens;
                d.ai_tokens.1 += e.output_tokens;
            }
        }
        Ok(d)
    }

    /// Audit of the whole bibliography (Zotero and Eloria's own sources).
    pub fn bib_audit(&self, lang: Lang) -> Result<Vec<crate::quality::BibIssue>> {
        Ok(crate::quality::bib_audit(&self.library_items()?, lang))
    }

    /// Citations of a note or a LaTeX document, known to the library or not.
    pub fn check_citations(&self, rel: &str) -> Result<Vec<crate::quality::CitationCheck>> {
        Ok(crate::quality::validate_citations(&self.vault.read(rel)?, &self.library_citekeys()?))
    }

    pub fn style_check(&self, rel: &str, lang: Lang) -> Result<Vec<crate::quality::StyleIssue>> {
        Ok(crate::quality::style_check(&self.vault.read(rel)?, lang))
    }

    /// Notes without any link (neither incoming nor outgoing), excluding sources, templates and journal.
    pub fn orphan_notes(&self, settings: &Settings) -> Result<Vec<String>> {
        let skip = [settings.templates_folder.trim_matches('/'), settings.daily_folder.trim_matches('/')].map(|f| format!("{f}/"));
        let linked: HashSet<String> = {
            // Links in either direction, or citations: the note is connected.
            let mut st = self.index.conn.prepare("SELECT src FROM links UNION SELECT resolved FROM links WHERE resolved IS NOT NULL UNION SELECT path FROM citations")?;
            let rows: Vec<String> = st.query_map([], |r| r.get(0))?.filter_map(|r| r.ok()).collect();
            rows.into_iter().collect()
        };
        Ok(self
            .index
            .all_notes()?
            .into_iter()
            .filter(|n| !skip.iter().any(|s| n.path.starts_with(s.as_str())))
            .filter(|n| !matches!(n.frontmatter.get("type").and_then(|v| v.as_str()), Some("source")))
            .filter(|n| !linked.contains(&n.path) && n.frontmatter.get("citekey").is_none())
            .map(|n| n.path)
            .collect())
    }

    /// Pairs of notes with almost identical content.
    pub fn duplicate_notes(&self, settings: &Settings) -> Result<Vec<(String, String, f32)>> {
        let tpl = format!("{}/", settings.templates_folder.trim_matches('/'));
        let mut texts = Vec::new();
        for n in self.index.all_notes()? {
            if n.path.starts_with(&tpl) || n.words < 60 || fiche::is_empty_legacy_import(&self.vault.read(&n.path).unwrap_or_default()) {
                continue;
            }
            texts.push((n.path.clone(), self.note_plain_text(&n.path)?));
        }
        Ok(crate::quality::near_duplicates(&texts, 0.6))
    }

    /// Reindexes a file changed outside the application.
    pub fn refresh_path(&self, rel: &str) -> Result<()> {
        if rel.ends_with(".md") && self.vault.exists(rel) {
            let c = self.vault.read(rel)?;
            let m = self.vault.mtime(rel)?;
            // Our own write (already indexed): nothing to redo.
            if self.index.mtime_of(rel)? == Some(m) {
                return Ok(());
            }
            self.index.update_note(rel, &c, m)?;
            return Ok(());
        } else {
            self.index.remove(rel)?;
            self.index.remove_prefix(rel)?;
        }
        self.index.resolve_links()
    }

    pub fn create_note(&self, folder: &str, name: &str, content: &str) -> Result<String> {
        let rel = self.vault.unique_path(folder, name, "md");
        self.vault.create_note(&rel, content)?;
        self.index.upsert(&rel, content, self.vault.mtime(&rel)?)?;
        self.index.resolve_links()?;
        Ok(rel)
    }

    /// Creates a note from an unresolved `[[Name]]` link (or `[[folder/Name]]`).
    pub fn create_from_link(&self, target: &str, from: &str) -> Result<String> {
        if let Some(p) = self.index.resolve(target)? {
            return Ok(p);
        }
        let target = target.trim().trim_end_matches(".md");
        let (folder, name) = match target.rsplit_once('/') {
            Some((f, n)) => (f.to_string(), n.to_string()),
            // Same folder as the originating note
            None => (from.rsplit_once('/').map(|(f, _)| f.to_string()).unwrap_or_default(), target.to_string()),
        };
        self.create_note(&folder, &name, &format!("# {name}\n\n"))
    }

    pub fn create_folder(&self, rel: &str) -> Result<()> {
        self.vault.create_folder(rel)
    }

    pub fn delete(&self, rel: &str) -> Result<()> {
        self.vault.trash(rel)?;
        self.index.remove(rel)?;
        self.index.remove_prefix(rel)?;
        self.index.resolve_links()
    }

    /// Renames/moves a note or folder while updating incoming links.
    pub fn rename(&self, from: &str, to: &str, update_links: bool) -> Result<RenameResult> {
        let is_dir = self.vault.resolve(from)?.is_dir();
        // Old path -> new path mapping for each affected note.
        let mapping: HashMap<String, String> = if is_dir {
            let prefix = format!("{}/", from.trim_end_matches('/'));
            self.index
                .all_notes()?
                .into_iter()
                .filter(|n| n.path.starts_with(&prefix))
                .map(|n| {
                    let new = format!("{}/{}", to.trim_end_matches('/'), &n.path[prefix.len()..]);
                    (n.path, new)
                })
                .collect()
        } else {
            HashMap::from([(from.to_string(), to.to_string())])
        };

        // Links to rewrite: (source note) -> [(raw target, new target)]
        let mut rewrites: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        if update_links {
            for (old, new) in &mapping {
                let old_stem = vault::stem(old);
                let new_stem = vault::stem(new);
                let backlinks = self.index.backlinks(old)?;
                let mut sources: Vec<String> = backlinks.into_iter().map(|b| b.path).collect();
                sources.push(old.clone()); // the note's internal links to itself
                sources.sort();
                sources.dedup();
                for src in sources {
                    for l in self.index.outgoing(&src)? {
                        if l.resolved.as_deref() != Some(old.as_str()) || l.target.is_empty() {
                            continue;
                        }
                        let new_target = if l.target.contains('/') {
                            new.trim_end_matches(".md").to_string()
                        } else if l.target.to_lowercase() == old_stem.to_lowercase() {
                            new_stem.clone()
                        } else {
                            continue; // alias: unchanged
                        };
                        if new_target != l.target {
                            let src_after = mapping.get(&src).cloned().unwrap_or(src.clone());
                            rewrites.entry(src_after).or_default().push((l.target.clone(), new_target));
                        }
                    }
                }
            }
        }

        self.vault.rename(from, to)?;
        for (old, new) in &mapping {
            let _ = history::rename(&self.vault, old, new);
        }

        let mut updated = Vec::new();
        for (src, pairs) in rewrites {
            let mut content = self.vault.read(&src)?;
            let mut n = 0;
            let mut seen = std::collections::HashSet::new();
            for (old_t, new_t) in pairs {
                if seen.insert(old_t.to_lowercase()) {
                    let (c, k) = markdown::rewrite_links(&content, &old_t, &new_t);
                    content = c;
                    n += k;
                }
            }
            if n > 0 {
                self.vault.write(&src, &content)?;
                updated.push(src);
            }
        }
        self.index.sync(&self.vault)?;
        // Explicit reindexing (the timestamp may not have changed).
        for src in &updated {
            let c = self.vault.read(src)?;
            self.index.upsert(src, &c, self.vault.mtime(src)?)?;
        }
        self.index.resolve_links()?;
        Ok(RenameResult { path: to.to_string(), updated })
    }

    pub fn set_property(&self, rel: &str, key: &str, value: Value, history_interval: i64) -> Result<String> {
        let content = set_property(&self.vault.read(rel)?, key, value)?;
        self.save_note(rel, &content, history_interval)?;
        Ok(content)
    }

    /// Saves an attachment (pasted image) and returns its relative path.
    pub fn save_attachment(&self, folder: &str, name: &str, bytes: &[u8]) -> Result<String> {
        let (stem, ext) = name.rsplit_once('.').unwrap_or((name, "png"));
        let rel = self.vault.unique_path(folder, stem, &ext.to_lowercase());
        self.vault.write_bytes(&rel, bytes)?;
        Ok(rel)
    }

    // ------------------------------------------------------------- templates

    pub fn list_templates(&self, folder: &str) -> Vec<String> {
        let prefix = format!("{}/", folder.trim_matches('/'));
        let mut v: Vec<String> =
            self.vault.list_notes().into_iter().map(|(p, _)| p).filter(|p| p.starts_with(&prefix)).collect();
        v.sort();
        v
    }

    /// Variables: `{{title}}`, `{{date}}`, `{{time}}`, `{{datetime}}`.
    pub fn render_template(&self, template_rel: &str, title: &str) -> Result<String> {
        let t = self.vault.read(template_rel)?;
        let now = chrono::Local::now();
        Ok(t.replace("{{title}}", title)
            .replace("{{titre}}", title)
            .replace("{{date}}", &now.format("%Y-%m-%d").to_string())
            .replace("{{time}}", &now.format("%H:%M").to_string())
            .replace("{{heure}}", &now.format("%H:%M").to_string())
            .replace("{{datetime}}", &now.format("%Y-%m-%d %H:%M").to_string()))
    }

    /// Daily note (created if missing). Uses `<templates>/Journal.md` if it exists;
    /// otherwise the title is the long date in the chosen language.
    pub fn daily_note(&self, folder: &str, templates_folder: &str, lang: Lang) -> Result<String> {
        let name = chrono::Local::now().format("%Y-%m-%d").to_string();
        let rel = if folder.is_empty() { format!("{name}.md") } else { format!("{}/{name}.md", folder.trim_matches('/')) };
        if self.vault.exists(&rel) {
            return Ok(rel);
        }
        let tpl = format!("{}/Journal.md", templates_folder.trim_matches('/'));
        let content = if self.vault.exists(&tpl) {
            self.render_template(&tpl, &name)?
        } else {
            format!("# {}\n\n", lang.long_date(chrono::Local::now().date_naive()))
        };
        self.create_note(folder, &name, &content)
    }

    pub fn stats(&self) -> Result<VaultStats> {
        let notes = self.index.all_notes()?;
        Ok(VaultStats {
            notes: notes.len() as i64,
            words: notes.iter().map(|n| n.words).sum(),
            fiches: notes.iter().filter(|n| n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("fiche")).count() as i64,
            tags: self.index.tags()?.len() as i64,
        })
    }

    // -------------------------------------------------------------- Zotero

    pub fn zotero_sync(&self, settings: &Settings, source: Source) -> Result<MirrorStatus> {
        let dir = (!settings.zotero_data_dir.is_empty()).then(|| PathBuf::from(&settings.zotero_data_dir));
        let scratch = self.vault.eloria_dir().join("tmp").join("zotero");
        zotero::sync(&self.index.conn, source, dir.as_deref(), settings.zotero_port, &scratch)
    }

    pub fn zotero_status(&self) -> Result<MirrorStatus> {
        zotero::status(&self.index.conn)
    }

    pub fn zotero_collections(&self) -> Result<Vec<zotero::Collection>> {
        zotero::collections(&self.index.conn)
    }

    pub fn zotero_items(&self, collection: Option<&str>, query: Option<&str>, limit: usize) -> Result<Vec<zotero::ItemSummary>> {
        zotero::items(&self.index.conn, collection, query, limit)
    }

    pub fn zotero_item(&self, key: &str) -> Result<ItemDetail> {
        let item = zotero::item(&self.index.conn, key)?.ok_or_else(|| Error::NotFound(format!("document Zotero {key}")))?;
        let annotations = zotero::annotations(&self.index.conn, key)?;
        Ok(ItemDetail { item, annotations })
    }

    pub fn zotero_item_by_citekey(&self, citekey: &str) -> Result<Option<ItemDetail>> {
        match zotero::item_by_citekey(&self.index.conn, citekey)? {
            Some(item) => {
                let annotations = zotero::annotations(&self.index.conn, &item.key)?;
                Ok(Some(ItemDetail { item, annotations }))
            }
            None => Ok(None),
        }
    }

    pub fn zotero_search_annotations(&self, query: &str) -> Result<Vec<(zotero::Annotation, String)>> {
        zotero::search_annotations(&self.index.conn, query, 200)
    }

    /// Zotero data folder: the configured one if it contains zotero.sqlite, otherwise ~/Zotero.
    pub fn zotero_data_dir(settings: &Settings) -> Option<PathBuf> {
        let d = PathBuf::from(&settings.zotero_data_dir);
        if !settings.zotero_data_dir.is_empty() && d.join("zotero.sqlite").exists() {
            Some(d)
        } else {
            zotero::default_data_dir()
        }
    }

    /// Absolute path of the PDF (or EPUB) of an item or attachment. The copy inside
    /// the vault comes first: it stays reachable even without Zotero.
    pub fn zotero_pdf(&self, key: &str, settings: &Settings) -> Result<Option<PathBuf>> {
        let Some((item_key, att)) = zotero::find_attachment(&self.index.conn, key)? else { return Ok(None) };
        let local = self.index.all_notes()?.into_iter().find_map(|n| {
            let fm = &n.frontmatter;
            (fm.get("zotero").and_then(|v| v.as_str()) == Some(item_key.as_str())).then(|| fm.get("pdf")?.as_str().map(str::to_string))?
        });
        if let Some(rel) = local {
            let p = self.vault.resolve(&rel)?;
            if p.is_file() {
                return Ok(Some(p));
            }
        }
        Ok(Self::zotero_data_dir(settings).and_then(|dir| zotero::attachment_file(&dir, &att)))
    }

    /// Copies an item's PDF into the vault (`<PDF folder>/<sub-folder>/@key.pdf`)
    /// and returns its relative path. An existing copy is reused.
    fn copy_pdf(&self, item: &zotero::Item, settings: &Settings, subfolder: &str) -> Option<String> {
        if !settings.copy_pdfs {
            return None;
        }
        let att = zotero::readable_attachment(item)?;
        let src = zotero::attachment_file(&Self::zotero_data_dir(settings)?, att)?;
        let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "pdf".into());
        let folder = [settings.pdf_folder.trim_matches('/'), subfolder.trim_matches('/')]
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join("/");
        let name = format!("@{}.{ext}", vault::sanitize_file_name(&item.citekey));
        let rel = if folder.is_empty() { name } else { format!("{folder}/{name}") };
        let dest = self.vault.resolve(&rel).ok()?;
        if !dest.exists() {
            std::fs::create_dir_all(dest.parent()?).ok()?;
            std::fs::copy(&src, &dest).ok()?;
        }
        Some(rel)
    }

    /// Citation key -> note of the item: the source if it exists (otherwise a reading note,
    /// for vaults created before the source / reading-note split).
    pub fn fiches_map(&self) -> Result<HashMap<String, String>> {
        let mut map = HashMap::new();
        let notes = self.index.all_notes()?;
        for want_source in [false, true] {
            for n in &notes {
                let is_source = n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("source");
                if is_source != want_source {
                    continue;
                }
                if let Some(ck) = n.frontmatter.get("citekey").and_then(|v| v.as_str()) {
                    map.insert(ck.to_lowercase(), n.path.clone());
                }
            }
        }
        Ok(map)
    }

    /// Source note of an item, found by its Zotero identifier (stable) or citation
    /// key (which can change, for example when Better BibTeX is installed).
    pub fn existing_source(&self, item: &zotero::Item) -> Result<Option<String>> {
        let ck = item.citekey.to_lowercase();
        Ok(self.index.all_notes()?.into_iter().find_map(|n| {
            let fm = &n.frontmatter;
            if fm.get("type").and_then(|v| v.as_str()) != Some("source") {
                return None;
            }
            let by_key = fm.get("zotero").and_then(|v| v.as_str()) == Some(item.key.as_str());
            let by_ck = fm.get("citekey").and_then(|v| v.as_str()).map(|c| c.to_lowercase() == ck).unwrap_or(false);
            (by_key || by_ck).then_some(n.path)
        }))
    }

    /// Notes of the old import left empty whose item now has its source:
    /// pure duplicates. A note the user wrote in never appears here.
    pub fn legacy_imports(&self) -> Result<Vec<String>> {
        let notes = self.index.all_notes()?;
        let sourced: HashSet<String> = notes
            .iter()
            .filter(|n| n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("source"))
            .filter_map(|n| n.frontmatter.get("zotero").and_then(|v| v.as_str()).map(str::to_string))
            .collect();
        let mut v = Vec::new();
        for n in notes {
            let key = n.frontmatter.get("zotero").and_then(|v| v.as_str()).unwrap_or_default();
            if n.frontmatter.get("type").and_then(|v| v.as_str()) != Some("fiche") || !sourced.contains(key) {
                continue;
            }
            if fiche::is_empty_legacy_import(&self.vault.read(&n.path)?) {
                v.push(n.path);
            }
        }
        v.sort();
        Ok(v)
    }

    /// Moves these duplicates to the vault trash (recoverable). Returns their number.
    pub fn clean_legacy_imports(&self) -> Result<usize> {
        let list = self.legacy_imports()?;
        for p in &list {
            self.vault.trash(p)?;
            self.index.remove(p)?;
        }
        if !list.is_empty() {
            self.index.resolve_links()?;
        }
        Ok(list.len())
    }

    /// Reading notes attached to an item (`fiche` type, same Zotero identifier).
    pub fn fiches_of(&self, item_key: &str) -> Result<Vec<String>> {
        let mut v: Vec<String> = self
            .index
            .all_notes()?
            .into_iter()
            .filter(|n| {
                n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("fiche")
                    && n.frontmatter.get("zotero").and_then(|v| v.as_str()) == Some(item_key)
            })
            .filter(|n| !self.vault.read(&n.path).map(|c| fiche::is_empty_legacy_import(&c)).unwrap_or(false))
            .map(|n| n.path)
            .collect();
        v.sort();
        Ok(v)
    }

    /// Adds to a note the tags it lacks, without removing its own. Returns
    /// `true` if the note changed.
    fn merge_tags(&self, rel: &str, wanted: &[String]) -> Result<bool> {
        let content = self.vault.read(rel)?;
        let current = markdown::parse_note(&content).frontmatter.get("tags").cloned();
        let mut tags: Vec<String> = match &current {
            Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
            Some(Value::String(s)) => s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect(),
            _ => vec![],
        };
        let before = tags.len();
        for t in wanted {
            if !tags.iter().any(|x| x.eq_ignore_ascii_case(t)) {
                tags.push(t.clone());
            }
        }
        if tags.len() == before {
            return Ok(false);
        }
        let new = set_property(&content, "tags", Value::from(tags))?;
        self.vault.write(rel, &new)?;
        self.index.upsert(rel, &new, self.vault.mtime(rel)?)?;
        Ok(true)
    }

    /// Items of a collection and its sub-collections (the whole library if `None`).
    fn items_in(&self, collection: Option<&str>) -> Result<Vec<zotero::Item>> {
        let items = zotero::all_items(&self.index.conn)?;
        let Some(root) = collection.filter(|c| !c.is_empty()) else { return Ok(items) };
        let colls = zotero::collections(&self.index.conn)?;
        let mut keep = std::collections::HashSet::from([root.to_string()]);
        // Descendants: extend as long as a collection has its parent in the set.
        loop {
            let before = keep.len();
            for c in &colls {
                if c.parent.as_ref().map(|p| keep.contains(p)).unwrap_or(false) {
                    keep.insert(c.key.clone());
                }
            }
            if keep.len() == before {
                break;
            }
        }
        Ok(items.into_iter().filter(|i| i.collections.iter().any(|c| keep.contains(c))).collect())
    }

    /// Paths of Zotero collections ("Thèse/Ch2 État de l'art"), by key.
    fn collection_paths(&self) -> Result<HashMap<String, String>> {
        let colls: HashMap<String, zotero::Collection> =
            zotero::collections(&self.index.conn)?.into_iter().map(|c| (c.key.clone(), c)).collect();
        Ok(colls
            .keys()
            .map(|key| {
                let mut parts = Vec::new();
                let mut cur = colls.get(key);
                let mut guard = 0;
                while let Some(c) = cur {
                    parts.push(vault::sanitize_file_name(&c.name));
                    cur = c.parent.as_deref().and_then(|p| colls.get(p));
                    guard += 1;
                    if guard > 32 {
                        break;
                    }
                }
                parts.reverse();
                (key.clone(), parts.join("/"))
            })
            .collect())
    }

    /// What an import would add: items (of a collection, or of the whole library)
    /// and those that already have their source.
    pub fn import_preview(&self, collection: Option<&str>) -> Result<ImportPreview> {
        let items = self.items_in(collection)?;
        let (keys, cks) = self.imported_sets()?;
        let existing = items.iter().filter(|i| keys.contains(&i.key) || cks.contains(&i.citekey.to_lowercase())).count();
        Ok(ImportPreview { total: items.len(), existing, new: items.len() - existing })
    }

    /// Zotero identifiers and citation keys of the vault's sources.
    fn imported_sets(&self) -> Result<(std::collections::HashSet<String>, std::collections::HashSet<String>)> {
        let mut keys = std::collections::HashSet::new();
        let mut cks = std::collections::HashSet::new();
        for n in self.index.all_notes()? {
            if n.frontmatter.get("type").and_then(|v| v.as_str()) != Some("source") {
                continue;
            }
            if let Some(k) = n.frontmatter.get("zotero").and_then(|v| v.as_str()) {
                keys.insert(k.to_string());
            }
            if let Some(c) = n.frontmatter.get("citekey").and_then(|v| v.as_str()) {
                cks.insert(c.to_lowercase());
            }
        }
        Ok((keys, cks))
    }

    /// Creates the source note of an item (filed according to its collection) if it does not exist.
    /// Returns its path and whether it was just created.
    pub fn import_source(&self, item_key: &str, settings: &Settings) -> Result<FicheResult> {
        let d = self.zotero_item(item_key)?;
        if let Some(p) = self.existing_source(&d.item)? {
            return Ok(FicheResult { path: p, created: false });
        }
        let paths = self.collection_paths()?;
        let (rel, _) = self.write_source(&d.item, &d.annotations, settings, &paths, true)?;
        self.index.resolve_links()?;
        Ok(FicheResult { path: rel, created: true })
    }

    /// Writes the source note of an item (and copies its PDF). Does not check for duplicates.
    fn write_source(
        &self,
        item: &zotero::Item,
        anns: &[zotero::Annotation],
        settings: &Settings,
        coll_paths: &HashMap<String, String>,
        by_collection: bool,
    ) -> Result<(String, bool)> {
        let colls: Vec<String> = item.collections.iter().filter_map(|c| coll_paths.get(c).cloned()).collect();
        let sub = if by_collection { colls.first().cloned().unwrap_or_default() } else { String::new() };
        let base = settings.sources_folder.trim_matches('/');
        let folder = [base, sub.as_str()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join("/");
        let name = fiche::fiche_file_name(item);
        let rel = if folder.is_empty() { name } else { format!("{folder}/{name}") };
        if self.vault.exists(&rel) {
            return Err(Error::AlreadyExists(rel));
        }
        // The PDF follows the source into the vault, filed in the same collection.
        let pdf = self.copy_pdf(item, settings, &sub);
        let content = fiche::render_source(item, anns, settings.lang(), pdf.as_deref(), &colls)?;
        self.vault.create_note(&rel, &content)?;
        self.index.upsert(&rel, &content, self.vault.mtime(&rel)?)?;
        Ok((rel, pdf.is_some()))
    }

    /// Creates a new reading note attached to an item's source (the source is created if
    /// needed). A source can have several reading notes: each call creates one.
    pub fn create_fiche(&self, item_key: &str, settings: &Settings) -> Result<FicheResult> {
        let source = self.import_source(item_key, settings)?.path;
        let d = self.zotero_item(item_key)?;
        let lang = settings.lang();
        let content = fiche::render_fiche_page(&d.item, &vault::stem(&source), lang)?;
        let rel = self.vault.unique_path(settings.fiches_folder.trim_matches('/'), &fiche::fiche_page_name(&d.item, lang), "md");
        self.vault.create_note(&rel, &content)?;
        self.index.upsert(&rel, &content, self.vault.mtime(&rel)?)?;
        self.index.resolve_links()?;
        Ok(FicheResult { path: rel, created: true })
    }

    /// Annotations made in Eloria on a vault PDF.
    pub fn pdf_annotations(&self, pdf_rel: &str) -> Result<Vec<PdfAnnotation>> {
        pdf_annot::load(&self.vault, pdf_rel)
    }

    /// Saves the annotations of a PDF, then updates the annotation block of the
    /// sources pointing to this PDF (`pdf` property). Returns the updated sources.
    pub fn save_pdf_annotations(&self, pdf_rel: &str, list: &[PdfAnnotation], settings: &Settings) -> Result<Vec<String>> {
        let saved = pdf_annot::save(&self.vault, pdf_rel, list)?;
        let block = pdf_annot::block(pdf_rel, &saved, settings.lang());
        let mut updated = Vec::new();
        for n in self.index.all_notes()? {
            if n.frontmatter.get("pdf").and_then(|v| v.as_str()) != Some(pdf_rel) {
                continue;
            }
            let content = self.vault.read(&n.path)?;
            let next = pdf_annot::replace_block(&content, &block);
            if next != content {
                self.save_note(&n.path, &next, settings.history_interval)?;
                updated.push(n.path);
            }
        }
        Ok(updated)
    }

    /// Notes (path, date) whose vector is missing or stale for this model.
    pub fn semantic_stale(&self, tag: &str, settings: &Settings) -> Result<Vec<(String, i64)>> {
        let tpl = format!("{}/", settings.templates_folder.trim_matches('/'));
        let notes: Vec<(String, i64)> = self.index.all_notes()?.into_iter().filter(|n| !n.path.starts_with(&tpl)).map(|n| (n.path, n.mtime)).collect();
        let existing: Vec<String> = notes.iter().map(|(p, _)| p.clone()).collect();
        semantic::prune(&self.index.conn, &existing)?;
        let stale: HashSet<String> = semantic::stale(&self.index.conn, &notes, tag)?.into_iter().collect();
        Ok(notes.into_iter().filter(|(p, _)| stale.contains(p)).collect())
    }

    /// Text to embed for a note (title, abstract of a source, body without properties).
    pub fn semantic_text(&self, rel: &str) -> Result<String> {
        let content = self.vault.read(rel)?;
        let note = markdown::parse_note(&content);
        let (_, body, _) = markdown::split_frontmatter(&content);
        let resume = note.frontmatter.get("resume").and_then(|v| v.as_str()).unwrap_or_default();
        Ok(semantic::note_text(&note.title.unwrap_or_else(|| vault::stem(rel)), resume, body))
    }

    pub fn semantic_store(&self, rel: &str, mtime: i64, tag: &str, v: &[f32]) -> Result<()> {
        semantic::store(&self.index.conn, rel, mtime, tag, v)
    }

    /// Notes close to a query vector, with title and score.
    pub fn semantic_nearest(&self, q: &[f32], tag: &str, k: usize) -> Result<Vec<(String, String, f32)>> {
        let near = semantic::nearest(&self.index.conn, q, tag, k)?;
        Ok(near.into_iter().map(|(p, s)| (self.index.title_of(&p).ok().flatten().unwrap_or_else(|| vault::stem(&p)), p, s)).map(|(t, p, s)| (p, t, s)).collect())
    }

    /// Bibliography (file marked by Eloria) of the keys cited in LaTeX sources.
    fn write_bib_for(&self, bib_rel: &str, tex_sources: &[String]) -> Result<Vec<String>> {
        const MARK: &str = "% Généré par Eloria : ne pas modifier, réécrit à chaque compilation.";
        if let Ok(existing) = self.vault.read(bib_rel) {
            if !existing.starts_with(MARK) {
                return Ok(vec![]);
            }
        }
        let mut keys: Vec<String> = Vec::new();
        for t in tex_sources {
            for k in latex::cited_keys(t) {
                if !keys.contains(&k) {
                    keys.push(k);
                }
            }
        }
        let items = self.library_items()?;
        let mut chosen = Vec::new();
        let mut missing = Vec::new();
        for k in &keys {
            match items.iter().find(|i| i.citekey.eq_ignore_ascii_case(k)) {
                Some(i) => chosen.push(i.clone()),
                None => missing.push(k.clone()),
            }
        }
        self.vault.write(bib_rel, &format!("{MARK}\n\n{}", crate::bib::to_bibtex(&chosen)))?;
        Ok(missing)
    }

    /// Converts a note into a full LaTeX document ("LaTeX" folder next to the note),
    /// with its bibliography. Returns the path of the .tex.
    pub fn note_to_latex(&self, rel: &str, settings: &Settings) -> Result<String> {
        let md = self.vault.read(rel)?;
        // Title: property, otherwise the first "#" heading, otherwise the file name.
        let (_, body0, _) = markdown::split_frontmatter(&md);
        let h1 = body0.lines().find_map(|l| l.strip_prefix("# ").map(|t| t.trim().to_string()));
        let title = markdown::parse_note(&md).frontmatter.get("titre").and_then(|v| v.as_str()).map(str::to_string).or(h1).unwrap_or_else(|| vault::stem(rel));
        // The "#" heading becomes the document title; ## becomes \section.
        let body = crate::manuscript::md_to_latex(&md, 0).replacen(&format!("\\chapter{{{}}}", crate::manuscript::inline(&title)), "", 1);
        let body = body.replace("\\chapter{", "\\section*{");
        let body = body.lines().filter(|l| !l.starts_with("\\label{sec:") || !l.is_empty()).collect::<Vec<_>>().join("\n");
        let babel = settings.lang().pick("french", "english");
        let doc = format!(
            "\\documentclass[11pt,a4paper]{{article}}\n\\usepackage[T1]{{fontenc}}\n\\usepackage[{babel}]{{babel}}\n\\usepackage{{csquotes}}\n\\usepackage{{amsmath,amssymb}}\n\\usepackage{{graphicx}}\n\\usepackage{{booktabs}}\n\\usepackage[hidelinks]{{hyperref}}\n\\usepackage[backend=biber,style=authoryear]{{biblatex}}\n\\addbibresource{{references.bib}}\n\n\\title{{{}}}\n\\date{{\\today}}\n\n\\begin{{document}}\n\\maketitle\n\n{}\n\n\\printbibliography\n\\end{{document}}\n",
            crate::manuscript::inline(&title),
            body.trim().replace("\\section{", "\\section{")
        );
        let dir = rel.rsplit_once('/').map(|(d, _)| format!("{d}/LaTeX")).unwrap_or_else(|| "LaTeX".into());
        let tex = format!("{dir}/{}.tex", crate::vault::sanitize_file_name(&vault::stem(rel)));
        self.vault.write(&tex, &doc)?;
        self.write_bib_for(&format!("{dir}/references.bib"), std::slice::from_ref(&doc))?;
        Ok(tex)
    }

    /// Assembles the thesis: each chapter note (`type: chapitre`, sorted by identifier)
    /// becomes a LaTeX file, plus `main.tex` and `references.bib`. Returns `main.tex`.
    pub fn assemble_thesis(&self, settings: &Settings) -> Result<String> {
        let lang = settings.lang();
        let notes = self.index.all_notes()?;
        let mut chapters: Vec<(String, String)> = notes
            .iter()
            .filter(|n| n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("chapitre"))
            .map(|n| (n.frontmatter.get("chapitre").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_else(|| n.title.clone()), n.path.clone()))
            .collect();
        if chapters.is_empty() {
            return Err(Error::Export(lang.pick("aucun chapitre : crée-en un depuis le panneau Thèse", "no chapter: create one from the Thesis panel").into()));
        }
        chapters.sort_by(|a, b| natural_key(&a.0).cmp(&natural_key(&b.0)));
        let plan = notes.iter().find(|n| n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("plan"));
        let title = plan.and_then(|p| p.frontmatter.get("titre_these").and_then(|v| v.as_str()).map(str::to_string)).unwrap_or_else(|| lang.pick("Thèse de doctorat", "Doctoral thesis").into());
        let author = plan.and_then(|p| p.frontmatter.get("auteur").and_then(|v| v.as_str()).map(str::to_string)).unwrap_or_default();
        let dir = format!("{}/{}", lang.pick("Thèse", "Thesis"), lang.pick("Manuscrit", "Manuscript"));
        let mut names = Vec::new();
        let mut sources = Vec::new();
        for (i, (_, path)) in chapters.iter().enumerate() {
            let tex = crate::manuscript::md_to_latex(&self.vault.read(path)?, 0);
            let name = format!("ch{}", i + 1);
            self.vault.write(&format!("{dir}/{name}.tex"), &format!("% Généré par Eloria depuis {path} : modifie la note, pas ce fichier.\n{tex}"))?;
            names.push(name);
            sources.push(tex);
        }
        let main = format!("{dir}/main.tex");
        self.vault.write(&main, &crate::manuscript::thesis_main(&title, &author, &names, lang))?;
        self.write_bib_for(&format!("{dir}/references.bib"), &sources)?;
        Ok(main)
    }

    /// Pre-submission checklist (.tex document or note).
    pub fn submission_checklist(&self, rel: &str, lang: Lang) -> Result<Vec<crate::manuscript::CheckItem>> {
        let mut text = self.vault.read(rel)?;
        let is_tex = rel.to_lowercase().ends_with(".tex");
        // main.tex: included chapters count too.
        if is_tex {
            let dir = rel.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
            for inc in regex::Regex::new(r"\\(?:include|input)\{([^}]+)\}").unwrap().captures_iter(&text.clone()) {
                let p = if dir.is_empty() { format!("{}.tex", &inc[1]) } else { format!("{dir}/{}.tex", &inc[1]) };
                if let Ok(c) = self.vault.read(&p) {
                    text.push('\n');
                    text.push_str(&c);
                }
            }
        }
        Ok(crate::manuscript::submission_checklist(&text, is_tex, &self.library_citekeys()?, lang))
    }

    // ------------------------------------------------------------- import / export, disk

    /// Imports a BibTeX file: one source per entry (original key kept), without
    /// duplicates (same DOI, same key, or item already in Zotero).
    pub fn import_bibtex(&self, text: &str, settings: &Settings) -> Result<BibImportReport> {
        let mut r = BibImportReport::default();
        for e in crate::bibimport::parse(text) {
            match self.add_reference_keyed(&e.work, Some(&e.key), settings) {
                Ok(f) if f.created => r.created += 1,
                Ok(_) => r.existing += 1,
                Err(err) => r.failed.push(format!("{} : {err}", e.key)),
            }
        }
        Ok(r)
    }

    /// Disk usage of the vault.
    pub fn disk_report(&self) -> crate::disk::DiskReport {
        crate::disk::scan(self.vault.root())
    }

    /// LaTeX auxiliary files moved to the vault trash (recreated by the next
    /// compilation). Returns the number of files.
    pub fn clean_latex_aux(&self) -> Result<usize> {
        let aux = crate::disk::scan(self.vault.root()).aux;
        for f in &aux {
            self.vault.trash(f)?;
        }
        Ok(aux.len())
    }

    /// Empties the vault trash (permanently); returns the bytes freed.
    pub fn empty_trash(&self) -> Result<u64> {
        let dir = self.vault.eloria_dir().join("trash");
        let before = crate::disk::scan(self.vault.root()).trash_bytes;
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(before)
    }

    /// Clears Eloria's temporary files (LaTeX previews); returns the bytes freed.
    pub fn clear_temp(&self) -> Result<u64> {
        let dir = self.vault.eloria_dir().join("tmp");
        let size = crate::disk::scan(self.vault.root()).internal.iter().find(|(n, _)| n == "tmp").map(|x| x.1).unwrap_or(0);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(size)
    }

    // ------------------------------------------------------------- extensions

    /// Vault key for extension approvals.
    fn vault_key(&self) -> String {
        self.vault.root().to_string_lossy().replace('\\', "/")
    }

    pub fn extensions(&self, grants: &[crate::settings::ExtensionGrant]) -> Vec<crate::extensions::ExtensionInfo> {
        crate::extensions::scan(&self.vault.eloria_dir(), &self.vault_key(), grants, crate::APP_VERSION)
    }

    /// Code of an extension approved for this vault (refused otherwise).
    pub fn extension_load(&self, grants: &[crate::settings::ExtensionGrant], id: &str) -> Result<crate::extensions::ExtensionCode> {
        crate::extensions::load(&self.vault.eloria_dir(), &self.vault_key(), grants, id, crate::APP_VERSION)
    }

    pub fn extension_grant(&self, grants: &mut Vec<crate::settings::ExtensionGrant>, id: &str, on: bool) -> Result<()> {
        crate::extensions::set_grant(&self.vault.eloria_dir(), &self.vault_key(), grants, id, on, crate::APP_VERSION)
    }

    pub fn extension_data_get(&self, id: &str) -> Result<serde_json::Value> {
        crate::extensions::data_get(&self.vault.eloria_dir(), id)
    }

    pub fn extension_data_set(&self, id: &str, value: &serde_json::Value) -> Result<()> {
        crate::extensions::data_set(&self.vault.eloria_dir(), id, value)
    }

    /// New extension (commented skeleton); returns its folder relative to the vault.
    pub fn extension_create(&self, name: &str) -> Result<String> {
        crate::extensions::scaffold(&self.vault.eloria_dir(), name, crate::APP_VERSION)
    }

    // ------------------------------------------------------------- advanced LaTeX

    fn formulas_path(lang: Lang) -> String {
        format!("{}.md", lang.pick("Formules", "Formulas"))
    }

    /// The user's formulas ("Formules" note).
    pub fn formulas(&self, settings: &Settings) -> Result<Vec<crate::texplus::Formula>> {
        Ok(self.vault.read(&Self::formulas_path(settings.lang())).map(|c| crate::texplus::parse_formulas(&c)).unwrap_or_default())
    }

    /// Adds a formula to the "Formules" note (created if needed); returns its path.
    pub fn add_formula(&self, name: &str, latex: &str, settings: &Settings) -> Result<String> {
        let rel = Self::formulas_path(settings.lang());
        let old = self.vault.read(&rel).ok();
        let content = crate::texplus::append_formula(old.as_deref(), name, latex, settings.lang());
        if old.is_some() {
            self.save_note(&rel, &content, settings.history_interval)?;
        } else {
            self.vault.create_note(&rel, &content)?;
            self.index.upsert(&rel, &content, self.vault.mtime(&rel)?)?;
        }
        Ok(rel)
    }

    /// Labels of a LaTeX document and of the files it includes (`\input`, `\include`).
    pub fn latex_labels(&self, rel: &str) -> Result<Vec<String>> {
        let text = self.vault.read(rel)?;
        let dir = rel.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
        let mut all = crate::latex::labels(&text);
        for inc in regex::Regex::new(r"\\(?:include|input)\{([^}]+)\}").unwrap().captures_iter(&text) {
            let name = inc[1].trim_end_matches(".tex");
            let p = if dir.is_empty() { format!("{name}.tex") } else { format!("{dir}/{name}.tex") };
            if let Ok(c) = self.vault.read(&p) {
                all.extend(crate::latex::labels(&c));
            }
        }
        all.dedup();
        Ok(all)
    }

    /// Source -> PDF: page and height (bp) of a line of a compiled .tex.
    pub fn latex_sync_forward(&self, rel: &str, line: u32) -> Result<Option<(u32, f64)>> {
        let tex = self.vault.resolve(rel)?;
        Ok(crate::texplus::load_synctex(&tex.with_extension("pdf")).and_then(|m| crate::texplus::forward(&m, &tex, line)))
    }

    /// PDF -> source: (vault file, line) at the clicked point of the page.
    pub fn latex_sync_inverse(&self, rel: &str, page: u32, y: f64) -> Result<Option<(String, u32)>> {
        let tex = self.vault.resolve(rel)?;
        let Some(m) = crate::texplus::load_synctex(&tex.with_extension("pdf")) else { return Ok(None) };
        let Some((path, line)) = crate::texplus::inverse(&m, page, y) else { return Ok(None) };
        // Engine path -> vault path (otherwise the document itself).
        let root = self.vault.root().to_string_lossy().replace('\\', "/").to_lowercase();
        let p = path.replace('\\', "/");
        let low = p.to_lowercase();
        let file = if let Some(rest) = low.strip_prefix(&format!("{root}/")) {
            p[p.len() - rest.len()..].to_string()
        } else {
            let name = p.rsplit('/').next().unwrap_or_default().to_string();
            let dir = rel.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
            let cand = if dir.is_empty() { name } else { format!("{dir}/{name}") };
            if self.vault.exists(&cand) { cand } else { rel.to_string() }
        };
        // TeX attaches a paragraph to the line where it ends (often empty): move up
        // to the last line that has text.
        let mut line = line;
        if let Ok(src) = self.vault.read(&file) {
            let lines: Vec<&str> = src.lines().collect();
            while line > 1 && lines.get(line as usize - 1).map(|l| l.trim().is_empty()).unwrap_or(false) {
                line -= 1;
            }
        }
        Ok(Some((file, line)))
    }

    /// Preview of a snippet (TikZ, formula, table) compiled separately, in `.eloria/tmp/apercu`.
    pub fn latex_preview(&self, snippet: &str, settings: &Settings) -> Result<crate::latex::CompileResult> {
        let engine = crate::latex::find_engine(&settings.latex_path).ok_or_else(|| Error::Export(settings.lang().pick("aucun moteur LaTeX : installe Tectonic depuis l'éditeur LaTeX", "no LaTeX engine: install Tectonic from the LaTeX editor").into()))?;
        crate::texplus::preview(&engine, snippet, &self.vault.eloria_dir().join("tmp").join("apercu"))
    }

    /// Reads a text file of the vault (LaTeX, bibliography).
    pub fn read_text(&self, rel: &str) -> Result<String> {
        Self::check_text_ext(rel)?;
        self.vault.read(rel)
    }

    /// Writes a text file of the vault (atomic), with version history.
    pub fn write_text(&self, rel: &str, content: &str, history_interval: i64) -> Result<()> {
        Self::check_text_ext(rel)?;
        if let Ok(old) = self.vault.read(rel) {
            let _ = history::snapshot(&self.vault, rel, &old, history_interval, false);
        }
        self.vault.write(rel, content)
    }

    fn check_text_ext(rel: &str) -> Result<()> {
        let ext = rel.rsplit('.').next().unwrap_or_default().to_lowercase();
        if vault::TEXT_EXT.contains(&ext.as_str()) {
            Ok(())
        } else {
            Err(Error::InvalidPath(format!("{rel} : seuls les fichiers .tex, .bib, .sty et .cls s'ouvrent ici")))
        }
    }

    /// New LaTeX document from a template (`article`, `chapitre`, `beamer`).
    pub fn latex_new(&self, folder: &str, name: &str, template: &str) -> Result<String> {
        let content = latex::templates().into_iter().find(|(id, _, _)| *id == template).map(|(_, _, c)| c).unwrap_or(latex::templates()[0].2);
        let rel = self.vault.unique_path(folder.trim_matches('/'), name, "tex");
        self.vault.write(&rel, content)?;
        Ok(rel)
    }

    /// Bibliography of a LaTeX document: the cited references (`\cite…`), taken from
    /// Zotero and Eloria's own sources, written to the file it declares
    /// (`\addbibresource{references.bib}`). A hand-written .bib file is never
    /// replaced: only a file created by Eloria (marker on the first line) is updated.
    pub fn latex_write_bib(&self, rel: &str) -> Result<Option<(String, usize, Vec<String>)>> {
        let tex = self.vault.read(rel)?;
        let Some(bib) = latex::bib_file(&tex) else { return Ok(None) };
        let dir = rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let bib_rel = if dir.is_empty() { bib.clone() } else { format!("{dir}/{bib}") };
        const MARK: &str = "% Généré par Eloria : ne pas modifier, réécrit à chaque compilation.";
        if let Ok(existing) = self.vault.read(&bib_rel) {
            if !existing.starts_with(MARK) {
                return Ok(None);
            }
        }
        let keys = latex::cited_keys(&tex);
        let items = self.library_items()?;
        let mut chosen = Vec::new();
        let mut missing = Vec::new();
        for k in &keys {
            match items.iter().find(|i| i.citekey.eq_ignore_ascii_case(k)) {
                Some(i) => chosen.push(i.clone()),
                None => missing.push(k.clone()),
            }
        }
        let content = format!("{MARK}\n\n{}", crate::bib::to_bibtex(&chosen));
        self.vault.write(&bib_rel, &content)?;
        Ok(Some((bib_rel, chosen.len(), missing)))
    }

    /// Compiles a vault LaTeX document, locally. The bibliography is written first.
    pub fn latex_compile(&self, rel: &str, settings: &Settings) -> Result<latex::CompileResult> {
        let engine = latex::find_engine(&settings.latex_path)
            .ok_or_else(|| Error::Export("aucun moteur LaTeX : installe Tectonic depuis Réglages › Export (ou MiKTeX, TeX Live)".into()))?;
        let bib = self.latex_write_bib(rel)?;
        let mut r = latex::compile(&engine, &self.vault.resolve(rel)?)?;
        if let Some((_, _, missing)) = bib {
            for k in missing {
                r.issues.insert(0, latex::TexIssue { file: String::new(), line: 0, message: format!("référence @{k} absente de ta bibliothèque"), level: "warning".into() });
            }
        }
        Ok(r)
    }

    /// Creates a literature review (`type: revue` note) in `folder`.
    pub fn create_review(&self, folder: &str, title: &str, question: &str, lang: Lang) -> Result<String> {
        let rel = self.vault.unique_path(folder.trim_matches('/'), title, "md");
        let content = review::new_note(title, question, lang);
        self.vault.create_note(&rel, &content)?;
        self.index.upsert(&rel, &content, self.vault.mtime(&rel)?)?;
        Ok(rel)
    }

    /// Review and its articles, completed (title, authors, source, reading notes).
    pub fn review(&self, rel: &str) -> Result<review::Review> {
        let note = markdown::parse_note(&self.vault.read(rel)?);
        let title = note.title.clone().unwrap_or_else(|| vault::stem(rel));
        let mut r = review::from_frontmatter(rel, &title, &note.frontmatter);
        let library: std::collections::HashMap<String, zotero::Item> = self.library_items()?.into_iter().map(|i| (i.citekey.to_lowercase(), i)).collect();
        let notes = self.index.all_notes()?;
        for a in &mut r.articles {
            let ck = a.citekey.to_lowercase();
            if let Some(i) = library.get(&ck) {
                a.title = i.title.clone();
                a.author_year = format!("{}{}", zotero::authors_short(&i.creators), i.year.map(|y| format!(" ({y})")).unwrap_or_default());
            }
            for n in &notes {
                if n.frontmatter.get("citekey").and_then(|v| v.as_str()).map(|c| c.to_lowercase() == ck) != Some(true) {
                    continue;
                }
                match n.frontmatter.get("type").and_then(|v| v.as_str()) {
                    Some("source") => a.source = n.path.clone(),
                    Some("fiche") => a.fiches.push(n.path.clone()),
                    _ => {}
                }
            }
            if a.title.is_empty() {
                a.title = format!("@{}", a.citekey);
            }
        }
        Ok(r)
    }

    /// Adds, changes the status of, or removes (empty `statut`) an article of a review.
    pub fn review_set_article(&self, rel: &str, citekey: &str, statut: &str, history_interval: i64) -> Result<review::Review> {
        if !statut.is_empty() && !review::STATUSES.contains(&statut) {
            return Err(Error::InvalidPath(format!("statut inconnu : {statut}")));
        }
        let r = review::from_frontmatter(rel, "", &markdown::parse_note(&self.vault.read(rel)?).frontmatter);
        self.set_property(rel, "articles", review::set_status(&r.articles, citekey, statut), history_interval)?;
        self.review(rel)
    }

    /// Excerpts of the included articles (reading notes, otherwise source and abstract) for the draft.
    pub fn review_passages(&self, rel: &str) -> Result<Vec<assist::Passage>> {
        let r = self.review(rel)?;
        let library: std::collections::HashMap<String, zotero::Item> = self.library_items()?.into_iter().map(|i| (i.citekey.to_lowercase(), i)).collect();
        let mut out = Vec::new();
        for a in r.articles.iter().filter(|a| a.statut == "inclus") {
            let mut text = String::new();
            if let Some(i) = library.get(&a.citekey.to_lowercase()) {
                text.push_str(&format!("{} ({}). {}\n", zotero::authors_short(&i.creators), i.year.map(|y| y.to_string()).unwrap_or_default(), i.publication));
                if !i.abstract_note.is_empty() {
                    text.push_str(&assist::excerpt(&i.abstract_note, &[], 1500));
                    text.push('\n');
                }
            }
            for f in &a.fiches {
                if let Ok(c) = self.vault.read(f) {
                    if fiche::is_empty_legacy_import(&c) {
                        continue;
                    }
                    let (_, body, _) = markdown::split_frontmatter(&c);
                    text.push_str(&assist::excerpt(body, &[], 3500));
                    text.push('\n');
                }
            }
            if text.chars().filter(|c| c.is_alphanumeric()).count() < 30 {
                continue;
            }
            out.push(assist::Passage {
                id: out.len() + 1,
                path: a.fiches.first().cloned().unwrap_or(a.source.clone()),
                title: a.title.clone(),
                citekey: a.citekey.clone(),
                zotero: library.get(&a.citekey.to_lowercase()).map(|i| i.key.clone()).unwrap_or_default(),
                text,
            });
        }
        Ok(out)
    }

    /// Research matrix: one row per reading note (cells taken from its sections). With
    /// `citekeys`, only these documents (literature review).
    pub fn research_matrix(&self, lang: Lang, citekeys: Option<&[String]>) -> Result<matrix::Matrix> {
        let wanted: Option<HashSet<String>> = citekeys.map(|c| c.iter().map(|k| k.to_lowercase()).collect());
        let library: std::collections::HashMap<String, zotero::Item> = self.library_items()?.into_iter().map(|i| (i.citekey.to_lowercase(), i)).collect();
        let mut rows = Vec::new();
        for n in self.index.all_notes()? {
            if n.frontmatter.get("type").and_then(|v| v.as_str()) != Some("fiche") {
                continue;
            }
            let citekey = n.frontmatter.get("citekey").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            if let Some(w) = &wanted {
                if !w.contains(&citekey.to_lowercase()) {
                    continue;
                }
            }
            let Ok(content) = self.vault.read(&n.path) else { continue };
            // Empty notes of the old import are not reading notes.
            if fiche::is_empty_legacy_import(&content) {
                continue;
            }
            let (_, body, _) = markdown::split_frontmatter(&content);
            let item = library.get(&citekey.to_lowercase());
            let year = item.and_then(|i| i.year);
            let author_year = match item {
                Some(i) => format!("{}{}", zotero::authors_short(&i.creators), i.year.map(|y| format!(" ({y})")).unwrap_or_default()),
                None => n.title.clone(),
            };
            rows.push(matrix::MatrixRow {
                path: n.path.clone(),
                citekey,
                title: item.map(|i| i.title.clone()).unwrap_or(n.title.clone()),
                author_year,
                year,
                statut: n.frontmatter.get("statut").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                cells: matrix::row_cells(body, 400),
            });
        }
        rows.sort_by(|a, b| a.author_year.to_lowercase().cmp(&b.author_year.to_lowercase()));
        Ok(matrix::Matrix { columns: matrix::columns(lang), rows })
    }

    /// Exports the matrix (csv, md, tex) into the vault and returns the created path.
    pub fn export_matrix(&self, format: &str, lang: Lang, citekeys: Option<&[String]>, folder: &str) -> Result<String> {
        let m = self.research_matrix(lang, citekeys)?;
        let (content, ext) = match format {
            "csv" => (matrix::to_csv(&m, lang), "csv"),
            "tex" => (matrix::to_latex(&m, lang), "tex"),
            _ => (format!("# {}\n\n{}", lang.pick("Matrice de recherche", "Research matrix"), matrix::to_markdown(&m, lang)), "md"),
        };
        let name = format!("{} {}", lang.pick("Matrice de recherche", "Research matrix"), chrono::Local::now().format("%Y-%m-%d"));
        let rel = self.vault.unique_path(folder.trim_matches('/'), &name, ext);
        self.vault.write(&rel, &content)?;
        if ext == "md" {
            self.index.upsert(&rel, &content, self.vault.mtime(&rel)?)?;
        }
        Ok(rel)
    }

    /// Concept note (concepts folder) with its definition and source; if it
    /// already exists, it is only returned (never overwritten).
    pub fn create_concept_note(&self, name: &str, definition: &str, source: Option<&str>, settings: &Settings) -> Result<FicheResult> {
        let name = name.trim();
        if let Some(p) = self.index.resolve(name)? {
            return Ok(FicheResult { path: p, created: false });
        }
        let lang = settings.lang();
        let folder = lang.pick("Concepts", "Concepts");
        let def = if definition.trim().is_empty() { String::new() } else { definition.trim().replace('\n', " ") };
        let src = source.map(|s| format!("- [[{}]]\n", vault::stem(s))).unwrap_or_default();
        let content = format!(
            "---\ntype: concept\ntags: [concept]\n---\n# {name}\n\n> [!definition] {}\n> {def}\n\n## {}\n\n\n## {}\n{src}",
            lang.pick("Définition", "Definition"),
            lang.pick("Explication", "Explanation"),
            lang.pick("Sources", "Sources"),
        );
        let rel = self.vault.unique_path(folder, name, "md");
        self.vault.create_note(&rel, &content)?;
        self.index.upsert(&rel, &content, self.vault.mtime(&rel)?)?;
        self.index.resolve_links()?;
        Ok(FicheResult { path: rel, created: true })
    }

    /// Text of a note for AI suggestions (without properties or generated blocks).
    pub fn note_plain_text(&self, rel: &str) -> Result<String> {
        let c = self.vault.read(rel)?;
        let (_, body, _) = markdown::split_frontmatter(&c);
        Ok(body.lines().filter(|l| !l.trim_start().starts_with("<!--")).collect::<Vec<_>>().join("\n"))
    }

    /// Excerpts for "Ask Eloria": vault notes found by full-text search
    /// (sources, reading notes, concepts, chapters), then Zotero items not imported yet whose
    /// title or abstract answers the question. Numbered [S1]…
    pub fn ask_passages(&self, question: &str, settings: &Settings, max_notes: usize) -> Result<Vec<assist::Passage>> {
        let words = assist::keywords(question);
        // Stems: "fraude" finds "fraud", "transati" finds "transaction".
        let roots = assist::stems(question);
        let tpl = format!("{}/", settings.templates_folder.trim_matches('/'));
        let mut out: Vec<assist::Passage> = Vec::new();
        let mut cited: HashSet<String> = HashSet::new();
        // Full text, then sources whose properties (title, abstract, tags) match:
        // a source's abstract is in its properties, outside the full-text index.
        let mut candidates = self.index.search_any(&roots, max_notes * 2)?;
        let mut by_props: Vec<(usize, String, String)> = self
            .index
            .all_notes()?
            .into_iter()
            .filter(|n| n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("source"))
            .map(|n| {
                let hay = assist::fold(&format!(
                    "{} {} {}",
                    n.title,
                    n.frontmatter.get("resume").and_then(|v| v.as_str()).unwrap_or_default(),
                    n.tags.join(" ")
                ));
                let mut score = roots.iter().filter(|w| hay.contains(w.as_str())).count() * 2;
                // Full PDF text: counts less than the title or the abstract.
                if let Some(full) = n.frontmatter.get("pdf").and_then(|v| v.as_str()).and_then(|p| self.pdf_text(p)) {
                    let full = assist::fold(&full);
                    score += roots.iter().filter(|w| full.contains(w.as_str())).count();
                }
                (score, n.path, n.title)
            })
            .filter(|(score, _, _)| *score > 0)
            .collect();
        by_props.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, p, t) in by_props.into_iter().take(max_notes) {
            if !candidates.iter().any(|(q, _)| q == &p) {
                candidates.push((p, t));
            }
        }
        for (path, title) in candidates {
            if path.starts_with(&tpl) || out.len() >= max_notes {
                continue;
            }
            let Ok(content) = self.vault.read(&path) else { continue };
            let note = markdown::parse_note(&content);
            let fm = &note.frontmatter;
            let s = |k: &str| fm.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let citekey = s("citekey");
            let (_, body, _) = markdown::split_frontmatter(&content);
            let mut text = String::new();
            if fm.get("type").and_then(|v| v.as_str()) == Some("source") {
                let year = fm.get("annee").map(|v| v.to_string()).unwrap_or_default();
                let authors = fm.get("auteurs").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                text.push_str(&format!("{authors} ({year}). {}\n", s("publication")));
                if !s("resume").is_empty() {
                    text.push_str(&format!("{}\n", assist::excerpt(&s("resume"), &words, 1500)));
                }
                // Full PDF text (extracted by the reader): the most relevant passage.
                if let Some(full) = fm.get("pdf").and_then(|v| v.as_str()).and_then(|p| self.pdf_text(p)) {
                    text.push_str(&format!("{}\n", assist::excerpt(&full, &words, 2000)));
                }
            }
            text.push_str(&assist::excerpt(body, &words, 1800));
            // Note without useful content (still empty reading note) or item already given: skipped.
            // Its key is kept anyway: the item will not come back through Zotero.
            let known = !citekey.is_empty() && !cited.insert(citekey.to_lowercase());
            if known || text.chars().filter(|c| c.is_alphanumeric()).count() < 30 {
                continue;
            }
            out.push(assist::Passage { id: out.len() + 1, path, title, citekey, zotero: s("zotero"), text });
        }
        // Zotero items not imported: title and abstract.
        let mut docs: Vec<(usize, zotero::Item)> = zotero::all_items(&self.index.conn)?
            .into_iter()
            .filter(|i| !cited.contains(&i.citekey.to_lowercase()))
            .map(|i| {
                let hay = assist::fold(&format!("{} {} {}", i.title, i.abstract_note, i.tags.join(" ")));
                (roots.iter().filter(|w| hay.contains(w.as_str())).count(), i)
            })
            // One common word is enough; closest first.
            .filter(|(score, i)| *score >= 1 && i.abstract_note.len() > 40)
            .collect();
        docs.sort_by(|a, b| b.0.cmp(&a.0));
        let room = (max_notes + 4).saturating_sub(out.len()).max(4);
        for (_, i) in docs.into_iter().take(room) {
            let text = format!("{} ({}). {}\n{}", zotero::authors_short(&i.creators), i.year.map(|y| y.to_string()).unwrap_or_default(), i.publication, i.abstract_note);
            out.push(assist::Passage { id: out.len() + 1, path: String::new(), title: i.title.clone(), citekey: i.citekey.clone(), zotero: i.key.clone(), text });
        }
        Ok(out)
    }

    /// Citation keys of the whole library (Zotero and Eloria's own sources), in lower case.
    pub fn library_citekeys(&self) -> Result<HashSet<String>> {
        Ok(self.library_items()?.into_iter().map(|i| i.citekey.to_lowercase()).collect())
    }

    /// Checks the vault: broken links, missing files, unreadable properties,
    /// sources orphaned from Zotero, reading notes without a source. Changes nothing.
    pub fn check_integrity(&self, settings: &Settings) -> Result<IntegrityReport> {
        let mut r = IntegrityReport::default();
        let notes = self.index.all_notes()?;
        r.notes = notes.len();
        let tpl = format!("{}/", settings.templates_folder.trim_matches('/'));
        let file_exists = |from: &str, target: &str| self.vault.find_file(target, Some(from)).is_some();
        // Links and embeds
        let mut st = self.index.conn.prepare("SELECT src, target, line, embed FROM links WHERE resolved IS NULL AND target <> '' ORDER BY src, line")?;
        let rows: Vec<(String, String, i64, i64)> = st.query_map([], |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?, x.get(3)?)))?.collect::<std::result::Result<_, _>>()?;
        for (src, target, line, embed) in rows {
            if src.starts_with(&tpl) || target.contains("{{") {
                continue;
            }
            let has_ext = target.rsplit('/').next().map(|n| n.contains('.') && !n.to_lowercase().ends_with(".md")).unwrap_or(false);
            if has_ext {
                if !file_exists(&src, &target) {
                    r.missing_files.push(Issue { path: src, line: line + 1, detail: target });
                }
            } else if embed == 0 || !has_ext {
                r.broken_links.push(Issue { path: src, line: line + 1, detail: target });
            }
        }
        let mirror: HashSet<String> = zotero::all_items(&self.index.conn)?.into_iter().map(|i| i.key).collect();
        for n in &notes {
            // Unreadable properties: read again from the file (the index ignored them).
            if let Ok(content) = self.vault.read(&n.path) {
                if let (Some(yaml), _, _) = markdown::split_frontmatter(&content) {
                    if let Err(e) = serde_yaml::from_str::<serde_yaml::Value>(yaml) {
                        r.bad_frontmatter.push(Issue { path: n.path.clone(), line: e.location().map(|l| l.line() as i64 + 1).unwrap_or(1), detail: e.to_string() });
                    }
                }
            }
            let fm = &n.frontmatter;
            let kind = fm.get("type").and_then(|v| v.as_str()).unwrap_or_default();
            if let Some(pdf) = fm.get("pdf").and_then(|v| v.as_str()).filter(|p| !p.is_empty()) {
                if !self.vault.exists(pdf) {
                    r.missing_files.push(Issue { path: n.path.clone(), line: 0, detail: pdf.to_string() });
                }
            }
            if kind == "source" && !mirror.is_empty() {
                if let Some(k) = fm.get("zotero").and_then(|v| v.as_str()).filter(|k| !k.is_empty()) {
                    if !mirror.contains(k) {
                        r.orphan_sources.push(Issue { path: n.path.clone(), line: 0, detail: k.to_string() });
                    }
                }
            }
            if kind == "fiche" {
                if let Some(s) = fm.get("source").and_then(|v| v.as_str()) {
                    let target = s.trim_matches(['[', ']', '"', '\'']).split('|').next().unwrap_or_default().trim().to_string();
                    if !target.is_empty() && self.index.resolve(&target)?.is_none() {
                        r.fiches_without_source.push(Issue { path: n.path.clone(), line: 0, detail: target });
                    }
                }
            }
        }
        Ok(r)
    }

    /// Content of a `![[Note]]` or `![[Note#Section]]` embed, rendered as safe HTML.
    /// `None` if the note (or section) does not exist.
    pub fn embed(&self, target: &str) -> Result<Option<Embed>> {
        let (name, heading) = match target.split_once('#') {
            Some((n, h)) => (n.trim(), Some(h.trim())),
            None => (target.trim(), None),
        };
        let Some(path) = self.index.resolve(name)? else { return Ok(None) };
        let content = self.vault.read(&path)?;
        let md = match heading {
            Some(h) if !h.is_empty() => match markdown::section(&content, h) {
                Some(s) => s,
                None => return Ok(None),
            },
            _ => content.clone(),
        };
        let title = markdown::parse_note(&content).title.unwrap_or_else(|| vault::stem(&path));
        Ok(Some(Embed { path, title, html: markdown::to_safe_html(&md) }))
    }

    /// Item of a source note: the one from the Zotero mirror when the source comes from it (with
    /// its annotations), otherwise the one described by its properties (Eloria's library).
    pub fn source_detail(&self, rel: &str) -> Result<ItemDetail> {
        let note = markdown::parse_note(&self.vault.read(rel)?);
        let fm = note.frontmatter;
        if let Some(key) = fm.get("zotero").and_then(|v| v.as_str()).filter(|k| !k.is_empty()) {
            if let Ok(mut d) = self.zotero_item(key) {
                // Abstract missing from Zotero but completed online in the note: shown.
                if d.item.abstract_note.trim().is_empty() {
                    d.item.abstract_note = fm.get("resume").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                }
                return Ok(d);
            }
        }
        let item = fiche::item_from_frontmatter(&fm).ok_or_else(|| Error::NotFound(format!("{rel} n'est pas une source")))?;
        Ok(ItemDetail { item, annotations: vec![] })
    }

    // ------------------------------------------------------------- concepts and knowledge

    /// Bodies of the notes (excluding templates and knowledge cards) to look for mentions:
    /// (meta, raw text, folded text).
    fn note_bodies(&self, settings: &Settings) -> Result<Vec<(crate::index::NoteMeta, String, String)>> {
        let tpl = format!("{}/", settings.templates_folder.trim_matches('/'));
        let mut out = Vec::new();
        for n in self.index.all_notes()? {
            if n.path.starts_with(&tpl) || n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("fiche-connaissance") {
                continue;
            }
            let c = self.vault.read(&n.path).unwrap_or_default();
            let (_, body, _) = markdown::split_frontmatter(&c);
            let mut text = body.to_string();
            if let Some(r) = n.frontmatter.get("resume").and_then(|v| v.as_str()) {
                text.push('\n');
                text.push_str(r);
            }
            let folded = assist::fold(&text);
            out.push((n, text, folded));
        }
        Ok(out)
    }

    fn concept_terms(n: &crate::index::NoteMeta) -> Vec<String> {
        let mut t = vec![n.title.clone(), vault::stem(&n.path)];
        t.extend(n.aliases.iter().cloned());
        t.retain(|x| x.chars().count() >= 3);
        t.dedup();
        t
    }

    /// Mentions of a concept: notes that link to it or cite its name (or an alias).
    fn concept_mentions(&self, concept: &crate::index::NoteMeta, bodies: &[(crate::index::NoteMeta, String, String)]) -> Result<Vec<crate::knowledge::Mention>> {
        let terms = Self::concept_terms(concept);
        let linked: HashSet<String> = self.index.backlinks(&concept.path)?.into_iter().map(|b| b.path).collect();
        let mut out = Vec::new();
        for (n, text, folded) in bodies {
            if n.path == concept.path {
                continue;
            }
            let is_linked = linked.contains(&n.path);
            if !is_linked && !terms.iter().any(|t| crate::methods::contains_term(folded, t)) {
                continue;
            }
            let excerpt = crate::knowledge::mention(text, &terms, 220).unwrap_or_default();
            out.push(crate::knowledge::Mention {
                date: crate::knowledge::note_date(&n.frontmatter, n.mtime),
                path: n.path.clone(),
                title: n.title.clone(),
                kind: n.frontmatter.get("type").and_then(|v| v.as_str()).unwrap_or("note").to_string(),
                citekey: n.frontmatter.get("citekey").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                excerpt,
                linked: is_linked,
            });
        }
        out.sort_by(|a, b| a.date.cmp(&b.date).then(a.title.cmp(&b.title)));
        Ok(out)
    }

    /// Concept base: every `type: concept` note, its definition and what talks about it.
    pub fn concepts_overview(&self, settings: &Settings) -> Result<Vec<crate::knowledge::ConceptRow>> {
        let bodies = self.note_bodies(settings)?;
        let mut rows = Vec::new();
        for (n, _, _) in bodies.iter().filter(|(n, _, _)| n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("concept")) {
            let m = self.concept_mentions(n, &bodies)?;
            rows.push(crate::knowledge::ConceptRow {
                path: n.path.clone(),
                title: n.title.clone(),
                aliases: n.aliases.clone(),
                definition: crate::knowledge::definition(&self.vault.read(&n.path).unwrap_or_default()),
                sources: m.iter().filter(|x| matches!(x.kind.as_str(), "source" | "fiche")).count(),
                notes: m.len(),
                updated: crate::knowledge::note_date(&serde_json::Map::new(), n.mtime),
            });
        }
        rows.sort_by(|a, b| b.notes.cmp(&a.notes).then(a.title.to_lowercase().cmp(&b.title.to_lowercase())));
        Ok(rows)
    }

    /// A concept in detail: definition (and its versions), notes that talk about it in
    /// chronological order, related concepts, research questions.
    pub fn concept_detail(&self, rel: &str, settings: &Settings) -> Result<crate::knowledge::ConceptDetail> {
        let bodies = self.note_bodies(settings)?;
        let concept = self.index.all_notes()?.into_iter().find(|n| n.path == rel).ok_or_else(|| Error::NotFound(rel.to_string()))?;
        let timeline = self.concept_mentions(&concept, &bodies)?;
        let is_concept = |p: &str| bodies.iter().any(|(n, _, _)| n.path == p && n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("concept"));
        let mut related: Vec<(String, String)> = Vec::new();
        for o in self.index.outgoing(rel)? {
            if let Some(p) = o.resolved.filter(|p| is_concept(p) && p != rel) {
                if !related.iter().any(|(x, _)| x == &p) {
                    let title = bodies.iter().find(|(n, _, _)| n.path == p).map(|(n, _, _)| n.title.clone()).unwrap_or_default();
                    related.push((p, title));
                }
            }
        }
        for m in timeline.iter().filter(|m| m.kind == "concept") {
            if !related.iter().any(|(x, _)| x == &m.path) {
                related.push((m.path.clone(), m.title.clone()));
            }
        }
        let questions = timeline.iter().filter(|m| m.kind == "question").map(|m| (m.path.clone(), m.title.clone())).collect();
        // Successive definitions, from the oldest to the current one.
        let mut definitions: Vec<crate::knowledge::DefinitionVersion> = Vec::new();
        let mut versions = crate::history::list(&self.vault, rel)?;
        versions.reverse();
        for v in versions {
            let Ok(c) = crate::history::read(&self.vault, rel, &v.id) else { continue };
            let d = crate::knowledge::definition(&c);
            if !d.is_empty() && definitions.last().map(|x| x.definition != d).unwrap_or(true) {
                let date = chrono::DateTime::from_timestamp(v.timestamp, 0).map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string()).unwrap_or_default();
                definitions.push(crate::knowledge::DefinitionVersion { date, definition: d });
            }
        }
        let current = crate::knowledge::definition(&self.vault.read(rel)?);
        if !current.is_empty() && definitions.last().map(|x| x.definition != current).unwrap_or(true) {
            definitions.push(crate::knowledge::DefinitionVersion { date: crate::knowledge::note_date(&concept.frontmatter, concept.mtime), definition: current.clone() });
        }
        let card = self.card_path(&concept.title, settings.lang());
        Ok(crate::knowledge::ConceptDetail {
            path: rel.to_string(),
            title: concept.title.clone(),
            aliases: concept.aliases.clone(),
            definition: current,
            timeline,
            related,
            questions,
            definitions,
            card: if self.vault.exists(&card) { card } else { String::new() },
        })
    }

    fn card_path(&self, title: &str, lang: Lang) -> String {
        format!("{}/{}.md", lang.pick("Fiches de connaissance", "Knowledge cards"), vault::sanitize_file_name(title))
    }

    /// Knowledge card of a concept: definition, key points taken from the sources (with
    /// their citation), related concepts, questions, dated evolution. Only the generated block is
    /// rewritten; text added around it is kept. Returns its path.
    pub fn knowledge_card(&self, rel: &str, settings: &Settings) -> Result<String> {
        let lang = settings.lang();
        let d = self.concept_detail(rel, settings)?;
        let mut b = format!("## {}\n\n{}\n\n", lang.pick("Définition", "Definition"), if d.definition.is_empty() { lang.pick("(à écrire dans la note du concept)", "(to write in the concept note)").to_string() } else { d.definition.clone() });
        let key: Vec<&crate::knowledge::Mention> = d.timeline.iter().filter(|m| matches!(m.kind.as_str(), "source" | "fiche") && !m.excerpt.is_empty()).collect();
        b.push_str(&format!("## {}\n\n", lang.pick("Points clés des sources", "Key points from sources")));
        if key.is_empty() {
            b.push_str(lang.pick("Aucune source n'en parle encore.\n", "No source mentions it yet.\n"));
        }
        let mut seen = HashSet::new();
        for m in key {
            if !m.citekey.is_empty() && !seen.insert(m.citekey.to_lowercase()) {
                continue;
            }
            let cite = if m.citekey.is_empty() { String::new() } else { format!(" [@{}]", m.citekey) };
            b.push_str(&format!("- {}{cite} ([[{}]])\n", m.excerpt, vault::stem(&m.path)));
        }
        b.push_str(&format!("\n## {}\n\n", lang.pick("Concepts liés", "Related concepts")));
        b.push_str(&if d.related.is_empty() { lang.pick("Aucun pour l'instant.\n", "None yet.\n").to_string() } else { d.related.iter().map(|(p, _)| format!("- [[{}]]\n", vault::stem(p))).collect() });
        if !d.questions.is_empty() {
            b.push_str(&format!("\n## {}\n\n", lang.pick("Questions de recherche", "Research questions")));
            for (p, _) in &d.questions {
                b.push_str(&format!("- [[{}]]\n", vault::stem(p)));
            }
        }
        b.push_str(&format!("\n## {}\n\n", lang.pick("Évolution", "Evolution")));
        for v in &d.definitions {
            b.push_str(&format!("- {} : {}\n", v.date, v.definition));
        }
        for m in &d.timeline {
            b.push_str(&format!("- {} : [[{}]]\n", m.date, vault::stem(&m.path)));
        }
        let card = self.card_path(&d.title, lang);
        let old = self.vault.read(&card).unwrap_or_else(|_| {
            format!("---\ntype: fiche-connaissance\nconcept: \"[[{}]]\"\ntags: [{}]\n---\n# {}\n", vault::stem(rel), lang.pick("connaissance", "knowledge"), d.title)
        });
        let content = crate::knowledge::replace_card(&old, &b);
        if self.vault.exists(&card) {
            self.save_note(&card, &content, settings.history_interval)?;
        } else {
            self.vault.create_note(&card, &content)?;
            self.index.upsert(&card, &content, self.vault.mtime(&card)?)?;
            self.index.resolve_links()?;
        }
        Ok(card)
    }

    // ------------------------------------------------------------- knowledge map

    /// Knowledge map: concepts, methods, questions, chapters and arguments
    /// (and, on request, sources and reading notes), connected by their links and by mentions of the
    /// name of a concept or a method.
    pub fn knowledge_map(&self, with_sources: bool, settings: &Settings) -> Result<KnowledgeMap> {
        let kinds: &[&str] = if with_sources { &["concept", "methode", "question", "chapitre", "argument", "source", "fiche"] } else { &["concept", "methode", "question", "chapitre", "argument"] };
        let bodies = self.note_bodies(settings)?;
        let kind_of = |n: &crate::index::NoteMeta| {
            let k = n.frontmatter.get("type").and_then(|v| v.as_str()).unwrap_or_default();
            if k == "méthode" { "methode".to_string() } else { k.to_string() }
        };
        let keep: Vec<&(crate::index::NoteMeta, String, String)> = bodies.iter().filter(|(n, _, _)| kinds.contains(&kind_of(n).as_str())).collect();
        let ids: HashSet<&str> = keep.iter().map(|(n, _, _)| n.path.as_str()).collect();
        let mut edges: Vec<(String, String, String)> = Vec::new();
        let mut seen: HashSet<(String, String)> = HashSet::new();
        let mut add = |a: &str, b: &str, k: &str, edges: &mut Vec<(String, String, String)>| {
            if a == b {
                return;
            }
            let key = if a < b { (a.to_string(), b.to_string()) } else { (b.to_string(), a.to_string()) };
            if seen.insert(key) {
                edges.push((a.to_string(), b.to_string(), k.to_string()));
            }
        };
        for (n, _, _) in &keep {
            for o in self.index.outgoing(&n.path)? {
                if let Some(t) = o.resolved.filter(|t| ids.contains(t.as_str())) {
                    add(&n.path, &t, "lien", &mut edges);
                }
            }
        }
        // Mentions of the name (or an alias) of a concept or a method.
        for (c, _, _) in keep.iter().filter(|(n, _, _)| matches!(kind_of(n).as_str(), "concept" | "methode")) {
            let terms = Self::concept_terms(c);
            for (n, _, folded) in &keep {
                if n.path != c.path && terms.iter().any(|t| crate::methods::contains_term(folded, t)) {
                    add(&n.path, &c.path, "mention", &mut edges);
                }
            }
        }
        let mut degree: HashMap<&str, usize> = HashMap::new();
        for (a, b, _) in &edges {
            *degree.entry(a.as_str()).or_default() += 1;
            *degree.entry(b.as_str()).or_default() += 1;
        }
        let nodes = keep
            .iter()
            .map(|(n, _, _)| MapNode { id: n.path.clone(), title: n.title.clone(), kind: kind_of(n), degree: degree.get(n.path.as_str()).copied().unwrap_or(0) })
            .collect();
        Ok(KnowledgeMap { nodes, edges })
    }

    // ------------------------------------------------------------- taxonomie

    fn taxonomy_path(lang: Lang) -> String {
        format!("{}.md", lang.pick("Taxonomie", "Taxonomy"))
    }

    /// Personal taxonomy: terms (from `Taxonomie.md`) and the notes filed under each.
    pub fn taxonomy(&self, settings: &Settings) -> Result<TaxonomyOverview> {
        let path = Self::taxonomy_path(settings.lang());
        if !self.vault.exists(&path) {
            return Ok(TaxonomyOverview { path, exists: false, nodes: vec![] });
        }
        let nodes = crate::taxonomy::parse(&self.vault.read(&path)?);
        let notes = self.index.all_notes()?;
        let nodes = nodes
            .into_iter()
            .map(|n| {
                let classed: Vec<(String, String)> = notes
                    .iter()
                    .filter(|m| m.tags.iter().any(|t| {
                        let t = t.to_lowercase();
                        t == n.tag || t.starts_with(&format!("{}/", n.tag))
                    }))
                    .map(|m| (m.path.clone(), m.title.clone()))
                    .collect();
                TaxNodeInfo { node: n, notes: classed }
            })
            .collect();
        Ok(TaxonomyOverview { path, exists: true, nodes })
    }

    /// Filing suggestions: (note, title, suggested tags) for notes that
    /// mention a taxonomy term without carrying its tag.
    pub fn taxonomy_proposals(&self, settings: &Settings) -> Result<Vec<(String, String, Vec<String>)>> {
        let path = Self::taxonomy_path(settings.lang());
        if !self.vault.exists(&path) {
            return Ok(vec![]);
        }
        let nodes = crate::taxonomy::parse(&self.vault.read(&path)?);
        let mut out = Vec::new();
        for (n, _, folded) in self.note_bodies(settings)? {
            if n.path == path {
                continue;
            }
            let title = assist::fold(&n.title);
            let tags = crate::taxonomy::propose(&format!("{title}\n{folded}"), &n.tags, &nodes);
            if !tags.is_empty() {
                out.push((n.path, n.title, tags));
            }
        }
        Ok(out)
    }

    /// Creates `Taxonomie.md` from existing tags (if it does not exist); returns its path.
    pub fn taxonomy_create(&self, settings: &Settings) -> Result<String> {
        let path = Self::taxonomy_path(settings.lang());
        if !self.vault.exists(&path) {
            let content = crate::taxonomy::skeleton(&self.index.tags()?, settings.lang());
            self.vault.create_note(&path, &content)?;
            self.index.upsert(&path, &content, self.vault.mtime(&path)?)?;
        }
        Ok(path)
    }

    // ------------------------------------------------------------- entities

    /// Named entities of the notes (people = authors of the library, organisations,
    /// methods, technologies, places), rule-based, with the notes that mention them.
    pub fn entities(&self, settings: &Settings) -> Result<Vec<crate::entities::EntityRow>> {
        let bodies = self.note_bodies(settings)?;
        let authors: Vec<(String, String)> = self
            .library_items()?
            .into_iter()
            .flat_map(|i| i.creators.into_iter().map(|c| (c.first_name, c.last_name)))
            .collect();
        let texts: Vec<&str> = bodies.iter().map(|(_, t, _)| t.as_str()).collect();
        let acronyms = crate::entities::recurring_acronyms(&texts, 3);
        let cat = crate::entities::catalogue(&authors, &acronyms);
        let mut hits = Vec::new();
        for (n, text, _) in &bodies {
            // A source does not "mention" its own authors.
            let own: HashSet<String> = n.frontmatter.get("auteurs").and_then(|v| v.as_array()).into_iter().flatten().filter_map(|a| a.as_str().map(assist::fold)).collect();
            for i in crate::entities::find(&format!("{}\n{text}", n.title), &cat) {
                if cat[i].kind == crate::entities::Kind::Personne && own.contains(&assist::fold(&cat[i].name)) {
                    continue;
                }
                hits.push((i, n.path.clone(), n.title.clone()));
            }
        }
        Ok(crate::entities::rows(&cat, &hits))
    }

    // ------------------------------------------------------------- topic clustering

    /// Topic groups of the notes (TF-IDF and k-means, no AI). `k` = 0: automatic.
    pub fn topics(&self, k: usize, settings: &Settings) -> Result<Vec<crate::topics::Topic>> {
        let docs: Vec<crate::topics::TopicDoc> = self
            .note_bodies(settings)?
            .into_iter()
            .filter(|(n, text, _)| n.words >= 20 || text.len() > 150)
            .filter(|(n, _, _)| !matches!(n.frontmatter.get("type").and_then(|v| v.as_str()), Some("brouillon-plan")))
            .map(|(n, text, _)| crate::topics::TopicDoc { path: n.path, title: n.title, text })
            .collect();
        Ok(crate::topics::cluster(&docs, k))
    }

    // ------------------------------------------------------------- statistiques de recherche

    /// Citations (documents, authors, journals, years), topic heat map and
    /// topic coverage per chapter. `now`: (year, month) of the last month shown.
    pub fn research_stats(&self, now: (i32, u32), settings: &Settings) -> Result<crate::stats::ResearchStats> {
        use crate::stats::{gaps, matrix, months_back, top, ResearchStats};
        let notes = self.index.all_notes()?;
        let tpl = format!("{}/", settings.templates_folder.trim_matches('/'));
        let kind = |n: &crate::index::NoteMeta| n.frontmatter.get("type").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let structural: HashSet<&str> = ["source", "fiche", "concept", "chapitre", "question", "methode", "argument", "plan", "brouillon-plan", "brouillon"].into_iter().collect();
        let reading: HashSet<String> = notes.iter().filter(|n| matches!(kind(n).as_str(), "source" | "fiche") || n.path.starts_with(&tpl)).map(|n| n.path.clone()).collect();
        let items: HashMap<String, zotero::Item> = self.library_items()?.into_iter().map(|i| (i.citekey.to_lowercase(), i)).collect();
        // Citations of working notes.
        let mut st = self.index.conn.prepare("SELECT path, citekey FROM citations")?;
        let pairs: Vec<(String, String)> = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .filter_map(|r| r.ok())
            .filter(|(p, _)| !reading.contains(p))
            .map(|(p, k)| (p, k.to_lowercase()))
            .collect();
        let mut per_doc: HashMap<String, usize> = HashMap::new();
        for (_, k) in &pairs {
            *per_doc.entry(k.clone()).or_default() += 1;
        }
        let mut authors: HashMap<String, usize> = HashMap::new();
        let mut venues: HashMap<String, usize> = HashMap::new();
        let mut years: BTreeMap<i32, usize> = BTreeMap::new();
        for (k, n) in &per_doc {
            if let Some(it) = items.get(k) {
                for c in it.creators.iter().filter(|c| c.creator_type == "author" || c.creator_type.is_empty()) {
                    *authors.entry(c.last_name.clone()).or_default() += n;
                }
                *venues.entry(it.publication.clone()).or_default() += n;
                if let Some(y) = it.year {
                    *years.entry(y).or_default() += 1;
                }
            }
        }
        let mut docs: Vec<(String, String, usize)> = per_doc.iter().map(|(k, n)| (k.clone(), items.get(k).map(|i| i.title.clone()).unwrap_or_default(), *n)).collect();
        docs.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
        docs.truncate(10);
        // Heat map: most used tags × the last 12 months.
        let mut tag_count: HashMap<String, usize> = HashMap::new();
        for n in &notes {
            for t in &n.tags {
                if !structural.contains(t.to_lowercase().as_str()) {
                    *tag_count.entry(t.to_lowercase()).or_default() += 1;
                }
            }
        }
        let heat_tags: Vec<String> = top(tag_count, 12).into_iter().map(|(t, _)| t).collect();
        let heat_months = months_back(now, 12);
        let mut heat_pairs = Vec::new();
        for n in &notes {
            let Some(d) = chrono::DateTime::from_timestamp_millis(n.mtime) else { continue };
            let month = d.with_timezone(&chrono::Local).format("%Y-%m").to_string();
            for t in &n.tags {
                heat_pairs.push((t.to_lowercase(), month.clone()));
            }
        }
        let heat = matrix(&heat_tags, &heat_months, &heat_pairs);
        // Coverage: topics of the documents cited by each chapter (and its attached notes).
        let list = |v: Option<&Value>| -> Vec<String> {
            match v {
                Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(|s| s.trim().to_lowercase())).collect(),
                Some(Value::String(s)) => s.split(',').map(|x| x.trim().to_lowercase()).collect(),
                _ => vec![],
            }
        };
        let mut source_tags: BTreeMap<String, HashSet<String>> = BTreeMap::new();
        let mut tags_of_doc: HashMap<String, Vec<String>> = HashMap::new();
        for n in notes.iter().filter(|n| kind(n) == "source") {
            let Some(ck) = n.frontmatter.get("citekey").and_then(|v| v.as_str()).map(|c| c.to_lowercase()) else { continue };
            let tags: Vec<String> = n.tags.iter().map(|t| t.to_lowercase()).filter(|t| !structural.contains(t.as_str())).collect();
            for t in &tags {
                source_tags.entry(t.clone()).or_default().insert(ck.clone());
            }
            tags_of_doc.insert(ck, tags);
        }
        let mut chapters: Vec<(String, String)> = notes
            .iter()
            .filter(|n| kind(n) == "chapitre")
            .filter_map(|n| n.frontmatter.get("chapitre").and_then(|v| v.as_str()).map(|id| (id.to_lowercase(), n.path.clone())))
            .collect();
        chapters.sort_by(|a, b| crate::thesis::natural(&a.0).cmp(&crate::thesis::natural(&b.0)));
        let mut cover_pairs = Vec::new();
        let mut covered: HashSet<String> = HashSet::new();
        for (id, path) in &chapters {
            let mut cited: HashSet<String> = self.index.citekeys_of(path)?.into_iter().map(|k| k.to_lowercase()).collect();
            for n in notes.iter().filter(|n| list(n.frontmatter.get("chapitres")).contains(id)) {
                cited.extend(self.index.citekeys_of(&n.path)?.into_iter().map(|k| k.to_lowercase()));
            }
            for k in cited {
                for t in tags_of_doc.get(&k).into_iter().flatten() {
                    cover_pairs.push((t.clone(), id.clone()));
                    covered.insert(t.clone());
                }
            }
        }
        let cover_themes: Vec<String> = top(source_tags.iter().map(|(t, d)| (t.clone(), d.len())).collect(), 12).into_iter().map(|(t, _)| t).collect();
        let cover_chapters: Vec<String> = chapters.iter().map(|(id, _)| id.clone()).collect();
        let cover = matrix(&cover_themes, &cover_chapters, &cover_pairs);
        Ok(ResearchStats {
            citations_total: pairs.len(),
            documents_cited: per_doc.len(),
            documents_total: items.len(),
            top_documents: docs,
            top_authors: top(authors, 10),
            top_venues: top(venues, 10),
            years: years.into_iter().collect(),
            heat_tags,
            heat_months,
            heat,
            cover_themes,
            cover_chapters,
            cover,
            gaps: gaps(&source_tags, &covered, 3),
        })
    }

    // ------------------------------------------------------------- citation consistency

    /// Citing sentences of a note or a .tex, and what the vault knows about each
    /// cited document (abstract, reading notes, the PDF passage closest to the sentences).
    pub fn support_job(&self, rel: &str, max: usize) -> Result<(Vec<crate::science::CitedSentence>, BTreeMap<String, String>)> {
        let text = self.vault.read(rel)?;
        let sentences = crate::science::cited_sentences(&text, max);
        let items: HashMap<String, zotero::Item> = self.library_items()?.into_iter().map(|i| (i.citekey.to_lowercase(), i)).collect();
        let notes = self.index.all_notes()?;
        let mut evidence = BTreeMap::new();
        let keys: HashSet<String> = sentences.iter().map(|s| s.citekey.clone()).collect();
        for k in keys {
            let words: Vec<String> = sentences.iter().filter(|s| s.citekey == k).flat_map(|s| assist::keywords(&s.sentence)).collect();
            let mut e = String::new();
            if let Some(i) = items.get(&k) {
                e.push_str(&format!("{} ({}). {}\n", i.title, i.year.map(|y| y.to_string()).unwrap_or_default(), zotero::authors_short(&i.creators)));
                if !i.abstract_note.trim().is_empty() {
                    e.push_str(&format!("Résumé : {}\n", i.abstract_note.trim()));
                }
            }
            for n in notes.iter().filter(|n| n.frontmatter.get("citekey").and_then(|v| v.as_str()).map(|c| c.to_lowercase()) == Some(k.clone())) {
                let kind = n.frontmatter.get("type").and_then(|v| v.as_str()).unwrap_or_default();
                if kind == "source" && e.is_empty() {
                    if let Some(r) = n.frontmatter.get("resume").and_then(|v| v.as_str()) {
                        e.push_str(&format!("Résumé : {r}\n"));
                    }
                }
                if kind == "fiche" {
                    let c = self.vault.read(&n.path).unwrap_or_default();
                    if !fiche::is_empty_legacy_import(&c) {
                        let (_, body, _) = markdown::split_frontmatter(&c);
                        e.push_str(&format!("Fiche : {}\n", assist::excerpt(body, &words, 2500)));
                    }
                }
                if let Some(full) = n.frontmatter.get("pdf").and_then(|v| v.as_str()).and_then(|p| self.pdf_text(p)) {
                    e.push_str(&format!("Extrait du PDF : {}\n", assist::excerpt(&full.replace("\n\n", "\n"), &words, 2000)));
                }
            }
            evidence.insert(k, e);
        }
        Ok((sentences, evidence))
    }

    // ------------------------------------------------------------- argumentation

    /// New argument ("Thèse/Arguments"); returns its path.
    pub fn argument_new(&self, title: &str, lang: Lang) -> Result<String> {
        let a = crate::argument::Argument { title: title.trim().to_string(), arguments: vec![Default::default()], objections: vec![Default::default()], ..Default::default() };
        let content = format!("---\ntype: argument\nstatut: brouillon\ntags: [{}]\n---\n{}", lang.pick("thèse/argument", "thesis/argument"), crate::argument::render(&a, lang));
        let folder = format!("{}/{}", lang.pick("Thèse", "Thesis"), lang.pick("Arguments", "Arguments"));
        let rel = self.vault.unique_path(&folder, &vault::sanitize_file_name(title.trim()), "md");
        self.vault.create_note(&rel, &content)?;
        self.index.upsert(&rel, &content, self.vault.mtime(&rel)?)?;
        Ok(rel)
    }

    /// Argument of a note, and cited keys missing from the library.
    pub fn argument_load(&self, rel: &str) -> Result<(crate::argument::Argument, Vec<String>)> {
        let a = crate::argument::parse(&self.vault.read(rel)?);
        let lib = self.library_citekeys()?;
        let unknown = crate::argument::citekeys(&a).into_iter().filter(|k| !lib.contains(&k.to_lowercase())).collect();
        Ok((a, unknown))
    }

    /// Rewrites the note from the map (the front matter is kept).
    pub fn argument_save(&self, rel: &str, a: &crate::argument::Argument, lang: Lang, history_interval: i64) -> Result<Vec<String>> {
        let old = self.vault.read(rel)?;
        let (yaml, _, _) = markdown::split_frontmatter(&old);
        let fm = yaml.map(|y| format!("---\n{}{}---\n", y, if y.ends_with('\n') || y.is_empty() { "" } else { "\n" })).unwrap_or_else(|| "---\ntype: argument\n---\n".into());
        let content = format!("{fm}{}", crate::argument::render(a, lang));
        if content != old {
            self.save_note(rel, &content, history_interval)?;
        }
        Ok(self.argument_load(rel)?.1)
    }

    /// Continuous text of the argument: a Markdown note next to it ("… (texte)"), or a full
    /// LaTeX document with its bibliography. Returns the created path.
    pub fn argument_export(&self, rel: &str, format: &str, settings: &Settings) -> Result<String> {
        let lang = settings.lang();
        let a = crate::argument::parse(&self.vault.read(rel)?);
        let dir = rel.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
        let name = format!("{} ({})", vault::stem(rel), lang.pick("texte", "text"));
        let md = format!("---\ntype: brouillon\nargument: \"[[{}]]\"\n---\n{}", vault::stem(rel), crate::argument::to_markdown(&a, lang));
        let text_rel = self.vault.unique_path(&dir, &vault::sanitize_file_name(&name), "md");
        self.vault.create_note(&text_rel, &md)?;
        self.index.upsert(&text_rel, &md, self.vault.mtime(&text_rel)?)?;
        self.index.resolve_links()?;
        if format == "tex" {
            return self.note_to_latex(&text_rel, settings);
        }
        Ok(text_rel)
    }

    // ------------------------------------------------------------- similarity

    /// Passages of a note or a .tex repeated word for word from the library: PDF
    /// text, abstracts, source annotations, abstracts of Zotero items not imported.
    pub fn similarity_check(&self, rel: &str) -> Result<Vec<crate::similarity::SimilarityHit>> {
        use crate::similarity::{check, Index, LibDoc};
        let content = self.vault.read(rel)?;
        let mut docs: Vec<LibDoc> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for n in self.index.all_notes()? {
            if n.path == rel || n.frontmatter.get("type").and_then(|v| v.as_str()) != Some("source") {
                continue;
            }
            let fm = &n.frontmatter;
            let ck = fm.get("citekey").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let c = self.vault.read(&n.path).unwrap_or_default();
            let (_, body, _) = markdown::split_frontmatter(&c);
            let mut text = format!("{}\n{}\n", fm.get("resume").and_then(|v| v.as_str()).unwrap_or_default(), body);
            if let Some(full) = fm.get("pdf").and_then(|v| v.as_str()).and_then(|p| self.pdf_text(p)) {
                text.push_str(&full);
            }
            seen.insert(ck.to_lowercase());
            docs.push(LibDoc { citekey: ck, title: fm.get("titre").and_then(|v| v.as_str()).map(str::to_string).unwrap_or(n.title.clone()), path: n.path.clone(), text });
        }
        for i in zotero::all_items(&self.index.conn)? {
            if !seen.contains(&i.citekey.to_lowercase()) && i.abstract_note.len() > 40 {
                docs.push(LibDoc { citekey: i.citekey, title: i.title, path: String::new(), text: i.abstract_note });
            }
        }
        let index = Index::new(docs);
        let mut hits = check(&content, &index);
        hits.truncate(200);
        Ok(hits)
    }

    // ------------------------------------------------------------- outline generator

    /// Draft outline from the notes of a scope: `scope` = "chapter" (notes
    /// attached through `chapitres: [id]`), "tag" (the tag and its sub-tags) or "folder".
    /// Written to "Thèse/Plans"; returns the path of the note.
    pub fn generate_outline(&self, scope: &str, value: &str, settings: &Settings) -> Result<String> {
        use crate::outline::{first_sentence, render, OutlineDoc};
        let lang = settings.lang();
        let want = value.trim().trim_start_matches('#').to_lowercase();
        if want.is_empty() {
            return Err(Error::NotFound(lang.pick("périmètre vide", "empty scope").into()));
        }
        let tpl = format!("{}/", settings.templates_folder.trim_matches('/'));
        let plans = format!("{}/{}/", lang.pick("Thèse", "Thesis"), lang.pick("Plans", "Outlines"));
        let list = |v: Option<&Value>| -> Vec<String> {
            match v {
                Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(|s| s.trim().to_lowercase())).collect(),
                Some(Value::String(s)) => s.split(',').map(|x| x.trim().to_lowercase()).collect(),
                _ => vec![],
            }
        };
        let mut docs = Vec::new();
        let mut chapter_title = String::new();
        for n in self.index.all_notes()? {
            if n.path.starts_with(&tpl) || n.path.starts_with(&plans) {
                continue;
            }
            let kind = n.frontmatter.get("type").and_then(|v| v.as_str()).unwrap_or("note").to_string();
            if scope == "chapter" && kind == "chapitre" && n.frontmatter.get("chapitre").and_then(|v| v.as_str()).map(|c| c.to_lowercase()) == Some(want.clone()) {
                chapter_title = n.title.clone();
                continue;
            }
            let inside = match scope {
                "chapter" => list(n.frontmatter.get("chapitres")).contains(&want),
                "tag" => n.tags.iter().any(|t| {
                    let t = t.to_lowercase();
                    t == want || t.starts_with(&format!("{want}/"))
                }),
                "folder" => n.path.to_lowercase().starts_with(&format!("{}/", want.trim_matches('/'))),
                _ => false,
            };
            if !inside || kind == "chapitre" {
                continue;
            }
            let content = self.vault.read(&n.path).unwrap_or_default();
            let fm = &n.frontmatter;
            let summary = match kind.as_str() {
                "fiche" => ["Résultats", "Résumé tiré de l'article", "Results", "Abstract from the article", "Problématique"]
                    .iter()
                    .find_map(|h| markdown::section(&content, h).map(|s| s.lines().skip(1).collect::<Vec<_>>().join(" ")).filter(|s| s.trim().len() > 10))
                    .unwrap_or_default(),
                "source" => fm.get("resume").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                _ => {
                    let (_, body, _) = markdown::split_frontmatter(&content);
                    body.split("\n\n").map(str::trim).find(|p| !p.is_empty() && !p.starts_with('#') && !p.starts_with("<!--") && !p.starts_with('>')).unwrap_or_default().to_string()
                }
            };
            docs.push(OutlineDoc {
                stem: vault::stem(&n.path),
                title: n.title.clone(),
                kind,
                tags: n.tags.clone(),
                citekey: fm.get("citekey").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                year: fm.get("annee").and_then(|v| v.as_i64()).map(|y| y as i32),
                summary: first_sentence(&summary, 180),
            });
        }
        let label = match scope {
            "chapter" if !chapter_title.is_empty() => chapter_title,
            "tag" => format!("#{want}"),
            _ => value.trim().to_string(),
        };
        let title = format!("{} : {label}", lang.pick("Plan", "Outline"));
        // The scope's own tag does not make a section on its own.
        let exclude = vec![want.clone(), format!("thèse/{want}")];
        let body = render(&title, &docs, &exclude, lang);
        let content = format!("---\ntype: brouillon-plan\nperimetre: \"{scope}:{}\"\ncree_le: {}\ntags: [{}]\n---\n{body}", want.replace('"', "'"), chrono::Local::now().format("%Y-%m-%d"), lang.pick("thèse/plan", "thesis/outline"));
        let rel = self.vault.unique_path(plans.trim_end_matches('/'), &vault::sanitize_file_name(&title), "md");
        self.vault.create_note(&rel, &content)?;
        self.index.upsert(&rel, &content, self.vault.mtime(&rel)?)?;
        self.index.resolve_links()?;
        Ok(rel)
    }

    // ------------------------------------------------------------- methodologies

    /// Methods found and the documents that use them: built-in catalogue and method
    /// notes (`type: methode`), searched in the "Méthodes" section of reading notes, the title,
    /// the abstract and the tags of sources; a link from a reading note to a method note counts.
    pub fn methods_overview(&self) -> Result<Vec<crate::methods::MethodRow>> {
        use crate::methods::{Method, MethodRow, MethodUse};
        let notes = self.index.all_notes()?;
        let kind = |n: &crate::index::NoteMeta| n.frontmatter.get("type").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let mut methods = crate::methods::builtin();
        for n in notes.iter().filter(|n| matches!(kind(n).as_str(), "methode" | "méthode" | "method")) {
            let same = methods.iter().position(|m| {
                let t = assist::fold(&n.title);
                assist::fold(&m.name) == t || m.aliases.iter().any(|a| assist::fold(a) == t) || n.aliases.iter().any(|a| assist::fold(a) == assist::fold(&m.name))
            });
            match same {
                Some(i) => {
                    methods[i].path = Some(n.path.clone());
                    methods[i].aliases.extend(n.aliases.iter().cloned());
                }
                None => methods.push(Method { name: n.title.clone(), aliases: n.aliases.clone(), path: Some(n.path.clone()) }),
            }
        }
        // Documents: by citation key, the reading note wins over the source for the link.
        struct Doc {
            title: String,
            path: String,
            year: Option<i32>,
            text: String,
            is_fiche: bool,
        }
        let mut docs: BTreeMap<String, Doc> = BTreeMap::new();
        for n in &notes {
            let k = kind(n);
            if k != "source" && k != "fiche" {
                continue;
            }
            let Some(ck) = n.frontmatter.get("citekey").and_then(|v| v.as_str()).map(|c| c.to_lowercase()) else { continue };
            let mut text = String::new();
            if k == "fiche" {
                let c = self.vault.read(&n.path).unwrap_or_default();
                for h in ["Méthodes", "Methods", "Méthodologie", "Methodology"] {
                    if let Some(sec) = markdown::section(&c, h) {
                        text.push_str(&sec);
                        text.push('\n');
                    }
                }
            } else {
                text = format!(
                    "{}\n{}\n{}",
                    n.frontmatter.get("titre").and_then(|v| v.as_str()).unwrap_or(&n.title),
                    n.frontmatter.get("resume").and_then(|v| v.as_str()).unwrap_or_default(),
                    n.tags.join(" ")
                );
            }
            let year = n.frontmatter.get("annee").and_then(|v| v.as_i64()).map(|y| y as i32);
            let title = n.frontmatter.get("titre").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_else(|| n.title.clone());
            let e = docs.entry(ck).or_insert(Doc { title: title.clone(), path: n.path.clone(), year, text: String::new(), is_fiche: false });
            e.text.push_str(&text);
            e.text.push('\n');
            if k == "fiche" && !e.is_fiche {
                e.path = n.path.clone();
                e.is_fiche = true;
            }
            if k == "source" {
                e.title = title;
                e.year = e.year.or(year);
            }
        }
        let by_path: HashMap<String, String> = notes
            .iter()
            .filter_map(|n| n.frontmatter.get("citekey").and_then(|v| v.as_str()).map(|c| (n.path.clone(), c.to_lowercase())))
            .collect();
        let mut uses: Vec<Vec<String>> = vec![Vec::new(); methods.len()];
        for (ck, d) in &docs {
            for i in crate::methods::find(&d.text, &methods) {
                uses[i].push(ck.clone());
            }
        }
        for (i, m) in methods.iter().enumerate() {
            if let Some(p) = &m.path {
                for b in self.index.backlinks(p)? {
                    if let Some(ck) = by_path.get(&b.path) {
                        if !uses[i].contains(ck) {
                            uses[i].push(ck.clone());
                        }
                    }
                }
            }
        }
        let mut rows: Vec<MethodRow> = methods
            .into_iter()
            .zip(uses)
            .filter(|(m, u)| !u.is_empty() || m.path.is_some())
            .map(|(m, u)| {
                let mut documents: Vec<MethodUse> = u
                    .iter()
                    .filter_map(|ck| docs.get(ck).map(|d| MethodUse { citekey: ck.clone(), title: d.title.clone(), path: d.path.clone(), year: d.year }))
                    .collect();
                documents.sort_by(|a, b| b.year.cmp(&a.year).then(a.citekey.cmp(&b.citekey)));
                MethodRow { name: m.name, aliases: m.aliases, path: m.path.unwrap_or_default(), documents }
            })
            .collect();
        rows.sort_by(|a, b| b.documents.len().cmp(&a.documents.len()).then(a.name.cmp(&b.name)));
        Ok(rows)
    }

    /// Method note ("Méthodes" folder): principle, strengths, limits. Its aliases help
    /// find it in reading notes.
    pub fn create_method_note(&self, name: &str, aliases: &[String], lang: Lang) -> Result<String> {
        let folder = lang.pick("Méthodes", "Methods");
        let alias_yaml = aliases.iter().map(|a| format!("\"{}\"", a.replace('"', "'"))).collect::<Vec<_>>().join(", ");
        let content = format!(
            "---\ntype: methode\naliases: [{alias_yaml}]\ntags: [{}]\n---\n# {name}\n\n## {}\n\n\n## {}\n\n\n## {}\n\n\n## {}\n\n",
            lang.pick("méthode", "method"),
            lang.pick("Principe", "Principle"),
            lang.pick("Forces", "Strengths"),
            lang.pick("Limites", "Limitations"),
            lang.pick("Usage dans ma thèse", "Use in my thesis"),
        );
        let rel = self.vault.unique_path(folder, &vault::sanitize_file_name(name), "md");
        self.vault.create_note(&rel, &content)?;
        self.index.upsert(&rel, &content, self.vault.mtime(&rel)?)?;
        self.index.resolve_links()?;
        Ok(rel)
    }

    // ------------------------------------------------------------- thesis workspace

    /// Thesis overview: chapters (progress, objectives, dependencies),
    /// writing order and research questions with their sources and notes.
    pub fn thesis_overview(&self) -> Result<crate::thesis::ThesisOverview> {
        use crate::thesis::{ChapterInfo, QuestionInfo, ThesisOverview};
        let notes = self.index.all_notes()?;
        let list = |v: Option<&Value>| -> Vec<String> {
            match v {
                Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(|s| s.trim().to_lowercase())).filter(|s| !s.is_empty()).collect(),
                Some(Value::String(s)) => s.split(',').map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect(),
                _ => vec![],
            }
        };
        let kind = |n: &crate::index::NoteMeta| n.frontmatter.get("type").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let str_of = |n: &crate::index::NoteMeta, k: &str| match n.frontmatter.get(k) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(x)) => x.to_string(),
            _ => String::new(),
        };
        // Citation key of sources and reading notes, by path (for [[@key]] links).
        let ck_of: HashMap<String, String> = notes
            .iter()
            .filter(|n| matches!(kind(n).as_str(), "source" | "fiche"))
            .filter_map(|n| n.frontmatter.get("citekey").and_then(|v| v.as_str()).map(|c| (n.path.clone(), c.to_string())))
            .collect();
        let mut questions: Vec<QuestionInfo> = Vec::new();
        for n in notes.iter().filter(|n| kind(n) == "question") {
            let mut sources: Vec<String> = self.index.citekeys_of(&n.path)?;
            for o in self.index.outgoing(&n.path)? {
                if let Some(ck) = o.resolved.as_ref().and_then(|p| ck_of.get(p)) {
                    if !sources.iter().any(|x| x.eq_ignore_ascii_case(ck)) {
                        sources.push(ck.clone());
                    }
                }
            }
            sources.sort();
            let mut linked: Vec<String> = self.index.backlinks(&n.path)?.into_iter().map(|b| b.path).collect();
            linked.dedup();
            let status = str_of(n, "statut");
            questions.push(QuestionInfo {
                path: n.path.clone(),
                title: n.title.clone(),
                status: if status.is_empty() { "ouverte".into() } else { status },
                chapters: list(n.frontmatter.get("chapitres")),
                sources,
                notes: linked,
            });
        }
        questions.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        let mut chapters: Vec<ChapterInfo> = Vec::new();
        for n in notes.iter().filter(|n| kind(n) == "chapitre") {
            let id = str_of(n, "chapitre").to_lowercase();
            let id = if id.is_empty() { vault::stem(&n.path).to_lowercase() } else { id };
            let attached: Vec<&crate::index::NoteMeta> = notes.iter().filter(|m| m.path != n.path && list(m.frontmatter.get("chapitres")).contains(&id)).collect();
            chapters.push(ChapterInfo {
                objectives: crate::thesis::objectives(&self.vault.read(&n.path).unwrap_or_default()),
                depends_on: list(n.frontmatter.get("depend_de")),
                questions: questions.iter().filter(|q| q.chapters.contains(&id)).map(|q| q.path.clone()).collect(),
                fiches: attached.iter().filter(|m| kind(m) == "fiche").count(),
                linked: attached.len(),
                status: str_of(n, "statut"),
                target: n.frontmatter.get("objectif").and_then(|v| v.as_i64().or_else(|| v.as_str()?.trim().parse().ok())).unwrap_or(0),
                words: n.words,
                title: n.title.clone(),
                path: n.path.clone(),
                id,
            });
        }
        chapters.sort_by(|a, b| crate::thesis::natural(&a.id).cmp(&crate::thesis::natural(&b.id)));
        let deps: BTreeMap<String, Vec<String>> = chapters.iter().map(|c| (c.id.clone(), c.depends_on.clone())).collect();
        let (layers, missing, cycle) = crate::thesis::layers(&deps);
        Ok(ThesisOverview { chapters, layers, missing, cycle, questions })
    }

    /// Research questions that cite or link to an item: (path, title, status).
    pub fn questions_for(&self, citekey: &str) -> Result<Vec<(String, String, String)>> {
        Ok(self
            .thesis_overview()?
            .questions
            .into_iter()
            .filter(|q| q.sources.iter().any(|s| s.eq_ignore_ascii_case(citekey)))
            .map(|q| (q.path, q.title, q.status))
            .collect())
    }

    // ------------------------------------------------------------- "new document" flow

    /// Zotero items without a source in the vault, most recently added first.
    pub fn new_zotero_documents(&self, limit: usize) -> Result<Vec<zotero::Item>> {
        let (keys, cks) = self.imported_sets()?;
        let mut items: Vec<zotero::Item> =
            zotero::all_items(&self.index.conn)?.into_iter().filter(|i| !keys.contains(&i.key) && !cks.contains(&i.citekey.to_lowercase())).collect();
        items.sort_by(|a, b| b.date_added.cmp(&a.date_added).then(a.key.cmp(&b.key)));
        items.truncate(limit);
        Ok(items)
    }

    /// Text of an item for suggestions: title, abstract, tags, body, start of the PDF.
    fn document_text(&self, rel: &str) -> Result<(markdown::ParsedNote, String)> {
        let content = self.vault.read(rel)?;
        let note = markdown::parse_note(&content);
        let fm = &note.frontmatter;
        let mut text = format!(
            "{}\n{}\n{}\n",
            note.title.clone().unwrap_or_default(),
            fm.get("titre").and_then(|v| v.as_str()).unwrap_or_default(),
            fm.get("resume").and_then(|v| v.as_str()).unwrap_or_default()
        );
        if let Some(full) = fm.get("pdf").and_then(|v| v.as_str()).and_then(|p| self.pdf_text(p)) {
            text.extend(full.chars().take(20_000));
        }
        Ok((note, text))
    }

    /// Vault tags that suit this item (no AI).
    pub fn propose_tags(&self, rel: &str) -> Result<Vec<String>> {
        let (note, text) = self.document_text(rel)?;
        Ok(crate::flow::propose_tags(&text, &self.index.tags()?, &note.tags))
    }

    /// Vault notes close to this item (full text on its keywords), excluding the
    /// item itself, its reading notes and templates: links to suggest in the reading note.
    pub fn related_notes(&self, rel: &str, limit: usize, settings: &Settings) -> Result<Vec<(String, String)>> {
        let (note, _) = self.document_text(rel)?;
        let fm = &note.frontmatter;
        let query = format!(
            "{} {} {}",
            fm.get("titre").and_then(|v| v.as_str()).map(str::to_string).or(note.title.clone()).unwrap_or_default(),
            fm.get("resume").and_then(|v| v.as_str()).unwrap_or_default().chars().take(400).collect::<String>(),
            note.tags.iter().filter(|t| t.as_str() != "source").cloned().collect::<Vec<_>>().join(" ")
        );
        let roots = assist::stems(&query);
        let mut skip: HashSet<String> = self.fiches_of_source(rel)?.into_iter().collect();
        skip.insert(rel.to_string());
        let tpl = format!("{}/", settings.templates_folder.trim_matches('/'));
        Ok(self
            .index
            .search_any(&roots, limit * 4)?
            .into_iter()
            .filter(|(p, _)| !skip.contains(p) && !p.starts_with(&tpl))
            .take(limit)
            .collect())
    }

    /// Adds tags to a note (normalised, without duplicates). True if the note changed.
    pub fn add_tags(&self, rel: &str, tags: &[String]) -> Result<bool> {
        let wanted: Vec<String> = tags.iter().filter_map(|t| fiche::note_tag(t)).collect();
        self.merge_tags(rel, &wanted)
    }

    /// Sources to complete online, (path, DOI): a DOI but no abstract, or a
    /// source added without Zotero that has no keyword yet.
    pub fn sources_to_complete(&self) -> Result<Vec<(String, String)>> {
        Ok(self
            .index
            .all_notes()?
            .into_iter()
            .filter(|n| n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("source"))
            .filter_map(|n| {
                let fm = &n.frontmatter;
                let doi = fm.get("doi").and_then(|v| v.as_str()).and_then(online::normalize_doi)?;
                let no_abstract = fm.get("resume").and_then(|v| v.as_str()).map(|s| s.trim().is_empty()).unwrap_or(true);
                let own = fm.get("zotero").and_then(|v| v.as_str()).map(|k| k.is_empty()).unwrap_or(true);
                (no_abstract || (own && n.tags.iter().all(|t| t == "source"))).then_some((n.path, doi))
            })
            .collect())
    }

    /// Completes a source with the article found online, without overwriting anything: abstract,
    /// journal and year if missing; keywords added to the tags. True if the note changed.
    pub fn complete_source(&self, rel: &str, work: &online::Work, history_interval: i64) -> Result<bool> {
        let original = self.vault.read(rel)?;
        let fm = markdown::parse_note(&original).frontmatter;
        let empty = |k: &str| match fm.get(k) {
            None | Some(Value::Null) => true,
            Some(Value::String(s)) => s.trim().is_empty(),
            _ => false,
        };
        let mut c = original.clone();
        if empty("resume") && !work.abstract_note.trim().is_empty() {
            c = set_property(&c, "resume", Value::String(work.abstract_note.trim().to_string()))?;
        }
        if empty("publication") && !work.venue.is_empty() {
            c = set_property(&c, "publication", Value::String(work.venue.clone()))?;
        }
        if empty("annee") {
            if let Some(y) = work.year {
                c = set_property(&c, "annee", Value::from(y))?;
            }
        }
        let mut tags: Vec<String> = match fm.get("tags") {
            Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
            _ => vec!["source".into()],
        };
        let before = tags.len();
        for t in fiche::fiche_tags(&work.keywords).into_iter().skip(1) {
            if !tags.contains(&t) {
                tags.push(t);
            }
        }
        if tags.len() != before {
            c = set_property(&c, "tags", Value::Array(tags.into_iter().map(Value::String).collect()))?;
        }
        if c == original {
            return Ok(false);
        }
        self.save_note(rel, &c, history_interval)?;
        Ok(true)
    }

    /// Adds a PDF from disk to the library, without Zotero: copied into the vault's PDF
    /// folder, then a source described by `work` (found online from the file's
    /// DOI) or, failing that, titled after the file name. An existing source
    /// (same DOI) gets the PDF if it had none.
    /// Returns the source and the path of the PDF inside the vault.
    pub fn add_pdf(&self, src: &std::path::Path, work: Option<&online::Work>, settings: &Settings) -> Result<(FicheResult, String)> {
        let fallback;
        let work = match work {
            Some(w) => w,
            None => {
                fallback = online::Work { title: crate::pdfimport::title_from_filename(&src.to_string_lossy()), item_type: "journalArticle".into(), ..Default::default() };
                &fallback
            }
        };
        let r = self.add_reference(work, settings)?;
        let fm = markdown::parse_note(&self.vault.read(&r.path)?).frontmatter;
        if let Some(p) = fm.get("pdf").and_then(|v| v.as_str()).filter(|p| !p.is_empty() && self.vault.exists(p)) {
            return Ok((r, p.to_string()));
        }
        let ck = fm.get("citekey").and_then(|v| v.as_str()).unwrap_or("document").to_string();
        let folder = settings.pdf_folder.trim_matches('/');
        let rel = self.vault.unique_path(folder, &format!("@{}", vault::sanitize_file_name(&ck)), "pdf");
        let dest = self.vault.resolve(&rel)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, &dest)?;
        self.set_property(&r.path, "pdf", Value::String(rel.clone()), settings.history_interval)?;
        Ok((r, rel))
    }

    /// Figure captured in a PDF: added to the "Figures" section of the sources of this PDF
    /// (created if needed, before the annotation block), with its page. Returns the modified notes.
    pub fn attach_figure(&self, pdf_rel: &str, image_rel: &str, page: u32, caption: &str, settings: &Settings) -> Result<Vec<String>> {
        let lang = settings.lang();
        let heading = format!("## {}", lang.pick("Figures", "Figures"));
        let label = if caption.trim().is_empty() { format!("{} {page}", lang.pick("Figure, p.", "Figure, p.")) } else { format!("{} ({} {page})", caption.trim(), lang.pick("p.", "p.")) };
        let entry = format!("![[{image_rel}]]\n*{label}*\n");
        let mut done = Vec::new();
        for n in self.index.all_notes()? {
            if n.frontmatter.get("pdf").and_then(|v| v.as_str()) != Some(pdf_rel) || n.frontmatter.get("type").and_then(|v| v.as_str()) != Some("source") {
                continue;
            }
            let c = self.vault.read(&n.path)?;
            let lines: Vec<&str> = c.lines().collect();
            let new = if let Some(h) = lines.iter().position(|l| l.trim() == heading) {
                // End of the section: next level-1 or level-2 heading, or generated block.
                let end = lines.iter().enumerate().skip(h + 1).find(|(_, l)| l.starts_with("# ") || l.starts_with("## ") || l.trim_start().starts_with("<!--")).map(|(i, _)| i).unwrap_or(lines.len());
                let mut out: Vec<String> = lines[..end].iter().map(|x| x.to_string()).collect();
                while out.last().map(|x| x.trim().is_empty()).unwrap_or(false) {
                    out.pop();
                }
                out.push(String::new());
                out.push(entry.trim_end().to_string());
                out.push(String::new());
                out.extend(lines[end..].iter().map(|x| x.to_string()));
                out.join("\n") + "\n"
            } else {
                let block = format!("{heading}\n\n{entry}\n");
                match c.find("<!-- zotero:debut -->").or_else(|| c.find("<!-- eloria:annotations")) {
                    Some(i) => {
                        let start = c[..i].rfind("\n## ").map(|j| j + 1).filter(|j| c[*j..i].lines().count() <= 3).unwrap_or(i);
                        format!("{}{block}\n{}", &c[..start], &c[start..])
                    }
                    None => format!("{}\n\n{block}", c.trim_end()),
                }
            };
            self.save_note(&n.path, &new, settings.history_interval)?;
            done.push(n.path);
        }
        Ok(done)
    }

    /// Text of a vault PDF, extracted by the reader: kept in `.eloria/pdftext/`
    /// for search and Ask Eloria (disposable, like the index).
    /// A source of this PDF without an abstract gets the one read from the text (with its keywords):
    /// returns the completed sources.
    pub fn store_pdf_text(&self, pdf_rel: &str, text: &str, history_interval: i64) -> Result<Vec<String>> {
        self.vault.resolve(pdf_rel)?;
        let p = self.vault.eloria_dir().join("pdftext").join(format!("{pdf_rel}.txt"));
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(p, text)?;
        let mut done = Vec::new();
        let (abs, keywords) = crate::pdfimport::abstract_from_text(text);
        if abs.is_empty() && keywords.is_empty() {
            return Ok(done);
        }
        for n in self.index.all_notes()? {
            let fm = &n.frontmatter;
            let mine = fm.get("type").and_then(|v| v.as_str()) == Some("source") && fm.get("pdf").and_then(|v| v.as_str()) == Some(pdf_rel);
            let no_abstract = fm.get("resume").and_then(|v| v.as_str()).map(|s| s.trim().is_empty()).unwrap_or(true);
            if mine && no_abstract && !abs.is_empty() {
                let w = online::Work { abstract_note: abs.clone(), keywords: keywords.clone(), ..Default::default() };
                if self.complete_source(&n.path, &w, history_interval)? {
                    done.push(n.path.clone());
                }
            }
        }
        Ok(done)
    }

    pub fn pdf_text(&self, pdf_rel: &str) -> Option<String> {
        std::fs::read_to_string(self.vault.eloria_dir().join("pdftext").join(format!("{pdf_rel}.txt"))).ok()
    }

    /// PDFs whose text is not extracted yet (for background extraction).
    pub fn pdfs_without_text(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for n in self.index.all_notes()? {
            if let Some(p) = n.frontmatter.get("pdf").and_then(|v| v.as_str()).filter(|p| p.to_lowercase().ends_with(".pdf")) {
                if self.vault.exists(p) && self.pdf_text(p).is_none() && !out.iter().any(|x: &String| x == p) {
                    out.push(p.to_string());
                }
            }
        }
        Ok(out)
    }

    /// Search in PDF text: (source, PDF, excerpt), the sources whose PDF
    /// contains the query.
    pub fn search_pdf_text(&self, query: &str, limit: usize) -> Result<Vec<(String, String, String)>> {
        let mut out = Vec::new();
        for n in self.index.all_notes()? {
            let Some(p) = n.frontmatter.get("pdf").and_then(|v| v.as_str()) else { continue };
            let Some(text) = self.pdf_text(p) else { continue };
            if let Some(s) = crate::pdfimport::snippet(&text, query, 160) {
                out.push((n.path.clone(), p.to_string(), s));
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }

    /// Reading notes attached to a source: same Zotero identifier, or a `source` property that
    /// links to it (`[[@key]]`).
    pub fn fiches_of_source(&self, rel: &str) -> Result<Vec<String>> {
        let stem = vault::stem(rel).to_lowercase();
        let zkey = markdown::parse_note(&self.vault.read(rel)?).frontmatter.get("zotero").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let mut v: Vec<String> = self
            .index
            .all_notes()?
            .into_iter()
            .filter(|n| n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("fiche"))
            .filter(|n| {
                let by_key = !zkey.is_empty() && n.frontmatter.get("zotero").and_then(|v| v.as_str()) == Some(zkey.as_str());
                let by_link = n
                    .frontmatter
                    .get("source")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim_matches(['[', ']', '"', '\'']).split('|').next().unwrap_or_default().to_lowercase() == stem)
                    .unwrap_or(false);
                by_key || by_link
            })
            // Empty notes of the old import are not reading notes (duplicates to clean up).
            .filter(|n| !self.vault.read(&n.path).map(|c| fiche::is_empty_legacy_import(&c)).unwrap_or(false))
            .map(|n| n.path)
            .collect();
        v.sort();
        Ok(v)
    }

    /// "Create reading note" from any source (Zotero or not).
    pub fn create_fiche_from_source(&self, source: &str, settings: &Settings) -> Result<FicheResult> {
        self.create_fiche_with_links(source, &[], settings)
    }

    /// Reading note of a source, with a "See also" line to the chosen notes ("new
    /// document" flow), placed before the sections of the Eloria method.
    pub fn create_fiche_with_links(&self, source: &str, links: &[String], settings: &Settings) -> Result<FicheResult> {
        let d = self.source_detail(source)?;
        let lang = settings.lang();
        let mut content = fiche::render_fiche_page(&d.item, &vault::stem(source), lang)?;
        if !links.is_empty() {
            let line = format!(
                "**{} :** {}

",
                lang.pick("Voir aussi", "See also"),
                links.iter().map(|l| format!("[[{}]]", vault::stem(l))).collect::<Vec<_>>().join(" · ")
            );
            if let Some(i) = content.find(crate::fiche_ia::IA_START) {
                content.insert_str(i, &line);
            }
        }
        let rel = self.vault.unique_path(settings.fiches_folder.trim_matches('/'), &fiche::fiche_page_name(&d.item, lang), "md");
        self.vault.create_note(&rel, &content)?;
        self.index.upsert(&rel, &content, self.vault.mtime(&rel)?)?;
        self.index.resolve_links()?;
        Ok(FicheResult { path: rel, created: true })
    }

    /// Vault sources that do not (or no longer) come from the Zotero mirror.
    pub fn own_items(&self) -> Result<Vec<zotero::Item>> {
        let mirror: HashSet<String> = zotero::all_items(&self.index.conn)?.into_iter().map(|i| i.key).collect();
        Ok(self
            .index
            .all_notes()?
            .into_iter()
            .filter_map(|n| fiche::item_from_frontmatter(&n.frontmatter))
            .filter(|i| i.key.is_empty() || !mirror.contains(&i.key))
            .collect())
    }

    /// The whole library: Zotero, then Eloria's own sources (no duplicate key).
    pub fn library_items(&self) -> Result<Vec<zotero::Item>> {
        let mut all = zotero::all_items(&self.index.conn)?;
        let mut seen: HashSet<String> = all.iter().map(|i| i.citekey.to_lowercase()).collect();
        for it in self.own_items()? {
            if seen.insert(it.citekey.to_lowercase()) {
                all.push(it);
            }
        }
        Ok(all)
    }

    // ------------------------------------------------------------- reliability

    fn reliability_path(&self) -> std::path::PathBuf {
        self.vault.eloria_dir().join("fiabilite.json")
    }

    /// Stored reliability checks (DOI -> OpenAlex record).
    pub fn reliability_cache(&self) -> BTreeMap<String, crate::reliability::Reliability> {
        std::fs::read_to_string(self.reliability_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn store_reliability(&self, r: &crate::reliability::Reliability) -> Result<()> {
        let mut c = self.reliability_cache();
        c.insert(r.doi.to_lowercase(), r.clone());
        std::fs::write(self.reliability_path(), serde_json::to_string_pretty(&c)?)?;
        Ok(())
    }

    /// Reliability signals of an already checked DOI.
    pub fn reliability_signals(&self, doi: &str, lang: Lang) -> Option<(crate::reliability::Reliability, Vec<crate::reliability::Signal>)> {
        let r = self.reliability_cache().get(&online::normalize_doi(doi)?)?.clone();
        let sg = crate::reliability::signals_now(&r, lang);
        Some((r, sg))
    }

    /// Library DOIs never checked.
    pub fn reliability_missing(&self) -> Result<Vec<String>> {
        let cache = self.reliability_cache();
        let mut out: Vec<String> = Vec::new();
        for i in self.library_items()? {
            if let Some(d) = online::normalize_doi(&i.doi) {
                if !cache.contains_key(&d) && !out.contains(&d) {
                    out.push(d);
                }
            }
        }
        Ok(out)
    }

    /// Reliability report: checked documents, most worrying first (and, at
    /// equal level, those you cite).
    pub fn reliability_report(&self, lang: Lang) -> Result<Vec<ReliabilityRow>> {
        let cache = self.reliability_cache();
        let year = chrono::Datelike::year(&chrono::Local::now());
        let sources = self.fiches_map()?;
        // Cited by a working note (neither source nor reading note: those carry the item's key).
        let reading: HashSet<String> = self
            .index
            .all_notes()?
            .into_iter()
            .filter(|n| matches!(n.frontmatter.get("type").and_then(|v| v.as_str()), Some("source") | Some("fiche")))
            .map(|n| n.path)
            .collect();
        let mut st = self.index.conn.prepare("SELECT DISTINCT citekey, path FROM citations")?;
        let cited: HashSet<String> = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .filter_map(|r| r.ok())
            .filter(|(_, p)| !reading.contains(p))
            .map(|(k, _)| k.to_lowercase())
            .collect();
        let mut rows: Vec<ReliabilityRow> = Vec::new();
        for i in self.library_items()? {
            let Some(d) = online::normalize_doi(&i.doi) else { continue };
            let Some(r) = cache.get(&d) else { continue };
            if rows.iter().any(|x| x.doi == d) {
                continue;
            }
            let signals = crate::reliability::signals(r, year, lang);
            rows.push(ReliabilityRow {
                level: crate::reliability::worst(&signals),
                signals,
                path: sources.get(&i.citekey.to_lowercase()).cloned().unwrap_or_default(),
                cited: cited.contains(&i.citekey.to_lowercase()),
                citekey: i.citekey,
                key: i.key,
                title: i.title,
                doi: d,
            });
        }
        rows.sort_by(|a, b| b.level.cmp(&a.level).then(b.cited.cmp(&a.cited)).then(a.citekey.cmp(&b.citekey)));
        Ok(rows)
    }

    /// Citation cache file (DOI -> DOIs of references), for the citation network.
    fn citation_cache_path(&self) -> std::path::PathBuf {
        self.vault.eloria_dir().join("citations.json")
    }

    pub fn citation_cache(&self) -> BTreeMap<String, Vec<String>> {
        std::fs::read_to_string(self.citation_cache_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn store_citations(&self, doi: &str, refs: &[String]) -> Result<()> {
        let mut c = self.citation_cache();
        c.insert(doi.to_lowercase(), refs.iter().map(|r| r.to_lowercase()).collect());
        std::fs::write(self.citation_cache_path(), serde_json::to_string(&c)?)?;
        Ok(())
    }

    /// Citation network between the documents of the library (from the cache):
    /// nodes = documents with a DOI, links = "A cites B" when both are in the library.
    pub fn citation_network(&self) -> Result<CitationNetwork> {
        let items = self.library_items()?;
        let cache = self.citation_cache();
        let by_doi: HashMap<String, &zotero::Item> = items.iter().filter(|i| !i.doi.is_empty()).map(|i| (i.doi.to_lowercase(), i)).collect();
        let mut edges = Vec::new();
        for (doi, refs) in &cache {
            let Some(a) = by_doi.get(doi) else { continue };
            for r in refs {
                if let Some(b) = by_doi.get(r) {
                    edges.push((a.citekey.clone(), b.citekey.clone()));
                }
            }
        }
        let nodes = by_doi
            .values()
            .map(|i| NetNode { citekey: i.citekey.clone(), title: i.title.clone(), year: i.year, label: format!("{}{}", zotero::authors_short(&i.creators), i.year.map(|y| format!(" {y}")).unwrap_or_default()), fetched: cache.contains_key(&i.doi.to_lowercase()) })
            .collect();
        Ok(CitationNetwork { nodes, edges, missing: by_doi.keys().filter(|d| !cache.contains_key(*d)).cloned().collect() })
    }

    /// Adds an article to Eloria's library: a source in the vault, without
    /// Zotero. No duplicate: same DOI already a source, or item of the Zotero mirror
    /// (its Zotero source is then created instead).
    pub fn add_reference(&self, work: &online::Work, settings: &Settings) -> Result<FicheResult> {
        self.add_reference_keyed(work, None, settings)
    }

    /// Like `add_reference`, keeping the given citation key when possible (BibTeX import).
    pub fn add_reference_keyed(&self, work: &online::Work, key: Option<&str>, settings: &Settings) -> Result<FicheResult> {
        let doi = work.doi.to_lowercase();
        if !doi.is_empty() {
            for n in self.index.all_notes()? {
                let same = n.frontmatter.get("type").and_then(|v| v.as_str()) == Some("source")
                    && n.frontmatter.get("doi").and_then(|v| v.as_str()).map(|d| d.to_lowercase() == doi).unwrap_or(false);
                if same {
                    return Ok(FicheResult { path: n.path, created: false });
                }
            }
            if let Some(z) = zotero::all_items(&self.index.conn)?.into_iter().find(|i| i.doi.to_lowercase() == doi) {
                return self.import_source(&z.key, settings);
            }
        }
        let taken: HashSet<String> = self.library_items()?.into_iter().map(|i| i.citekey.to_lowercase()).collect();
        // Same key already in the library (repeated import): nothing to create.
        if let Some(k) = key.map(str::trim).filter(|k| !k.is_empty()) {
            if taken.contains(&k.to_lowercase()) {
                if let Some(n) = self.index.all_notes()?.into_iter().find(|n| n.frontmatter.get("citekey").and_then(|v| v.as_str()).map(|c| c.eq_ignore_ascii_case(k)).unwrap_or(false)) {
                    return Ok(FicheResult { path: n.path, created: false });
                }
            }
        }
        let base = key.map(str::trim).filter(|k| !k.is_empty() && !taken.contains(&k.to_lowercase())).map(str::to_string).unwrap_or_else(|| zotero::generate_citekey(&work.to_item("")));
        let mut ck = base.clone();
        for c in 'a'..='z' {
            if !taken.contains(&ck) {
                break;
            }
            ck = format!("{base}{c}");
        }
        let item = work.to_item(&ck);
        let content = fiche::render_source(&item, &[], settings.lang(), None, &[])?;
        let rel = self.vault.unique_path(settings.sources_folder.trim_matches('/'), &format!("@{ck}"), "md");
        self.vault.create_note(&rel, &content)?;
        self.index.upsert(&rel, &content, self.vault.mtime(&rel)?)?;
        self.index.resolve_links()?;
        Ok(FicheResult { path: rel, created: true })
    }

    /// Imports the whole Zotero library as source notes (one per item).
    pub fn import_library(&self, settings: &Settings, by_collection: bool) -> Result<ImportReport> {
        self.import_collection(settings, None, by_collection)
    }

    /// Imports a Zotero collection (with its sub-collections) as source notes, filed
    /// as in Zotero; `None` = the whole library. Never a duplicate: an existing
    /// source only gets the item tags it lacks. PDFs copied.
    pub fn import_collection(&self, settings: &Settings, collection: Option<&str>, by_collection: bool) -> Result<ImportReport> {
        let items = self.items_in(collection)?;
        let (imported_keys, imported_cks) = self.imported_sets()?;
        let paths = self.collection_paths()?;
        let mut report = ImportReport::default();
        self.index.conn.execute_batch("BEGIN")?;
        for item in &items {
            if imported_keys.contains(&item.key) || imported_cks.contains(&item.citekey.to_lowercase()) {
                report.skipped += 1;
                if let Some(path) = self.existing_source(item)? {
                    let mut wanted = vec!["source".to_string()];
                    wanted.extend(fiche::fiche_tags(&item.tags).into_iter().skip(1));
                    if self.merge_tags(&path, &wanted)? {
                        report.updated += 1;
                    }
                }
                continue;
            }
            let made = zotero::annotations(&self.index.conn, &item.key)
                .map_err(Error::from)
                .and_then(|anns| self.write_source(item, &anns, settings, &paths, by_collection));
            match made {
                Ok((_, pdf)) => {
                    report.created += 1;
                    report.pdfs += usize::from(pdf);
                }
                Err(Error::AlreadyExists(_)) => report.skipped += 1,
                Err(e) => report.failed.push(format!("@{} : {e}", item.citekey)),
            }
        }
        self.index.conn.execute_batch("COMMIT")?;
        self.index.resolve_links()?;
        Ok(report)
    }

    /// Prepares the "Eloria method" analysis of an item: everything that needs the vault
    /// and the index (item, PDF, full text, library). The AI call then happens
    /// with [`fiche_ia::Job::run`], without holding the vault: saves
    /// keep working during generation.
    pub fn ai_fiche_job(&self, item_key: &str, settings: &Settings) -> Result<fiche_ia::Job> {
        use crate::ai::{AiConfig, AiRequest, Part, Provider};
        if !settings.ai_enabled {
            return Err(Error::Ai("l'IA est désactivée (Réglages › Intelligence artificielle)".into()));
        }
        let lang = settings.lang();
        let d = self.zotero_item(item_key)?;
        let cfg = AiConfig::from_settings(settings);
        let data_dir = Self::zotero_data_dir(settings);
        let fulltext = zotero::readable_attachment(&d.item).zip(data_dir.as_ref()).and_then(|(a, dir)| zotero::fulltext(dir, &a.key));
        // PDF: the vault copy first, otherwise Zotero storage. Sent as is to Anthropic
        // (32 MB at most per request); other providers receive the full text.
        let pdf = self.zotero_pdf(item_key, settings)?.filter(|p| p.extension().map(|e| e.eq_ignore_ascii_case("pdf")).unwrap_or(false));
        let pdf_bytes = match (&pdf, cfg.provider) {
            (Some(p), Provider::Anthropic) => std::fs::read(p).ok().filter(|b| b.len() <= 30 * 1024 * 1024),
            _ => None,
        };
        let mut parts = Vec::new();
        let source = if let Some(data) = pdf_bytes {
            parts.push(Part::Pdf { data, fallback_text: fulltext.clone() });
            "pdf"
        } else if let Some(t) = &fulltext {
            parts.push(Part::Text(format!("{}\n\n{t}", lang.pick("Texte intégral de l'article :", "Full text of the article:"))));
            "texte"
        } else {
            "résumé"
        };
        parts.push(Part::Text(fiche_ia::user_prompt(&d.item, &d.annotations, lang)));
        Ok(fiche_ia::Job {
            request: AiRequest { system: fiche_ia::system_prompt(lang, &settings.research_topic), parts, max_tokens: 64000 },
            config: cfg,
            item: d.item,
            library: zotero::all_items(&self.index.conn)?,
            fulltext,
            source: source.into(),
            lang,
            vault: self.vault.clone(),
        })
    }

    /// Saves a reviewed "Eloria method" note into the reading note `path` (a new
    /// reading note attached to the source if `path` is empty): the outline between the markers is
    /// replaced, a version is kept in the history, suggested tags are added.
    pub fn ai_fiche_save(&self, draft: &fiche_ia::AiFicheDraft, path: &str, settings: &Settings) -> Result<String> {
        let path = if path.is_empty() { self.create_fiche(&draft.item_key, settings)?.path } else { path.to_string() };
        let content = self.vault.read(&path)?;
        let _ = history::snapshot(&self.vault, &path, &content, 0, true);
        let mut new = fiche_ia::insert_block(&content, &draft.block);
        new = set_property(&new, "ia", Value::from(draft.model.clone()))?;
        self.save_note(&path, &new, settings.history_interval)?;
        self.merge_tags(&path, &draft.tags)?;
        Ok(path)
    }

    /// Regenerates the annotation block of a reading note (found by `zotero` or `citekey` in the front matter).
    pub fn sync_fiche(&self, rel: &str, history_interval: i64, lang: Lang) -> Result<String> {
        let content = self.vault.read(rel)?;
        let p = markdown::parse_note(&content);
        let key = p.frontmatter.get("zotero").and_then(|v| v.as_str()).map(str::to_string);
        let d = match key {
            Some(k) => self.zotero_item(&k).ok(),
            None => None,
        };
        let d = match d {
            Some(d) => d,
            None => {
                let ck = p
                    .frontmatter
                    .get("citekey")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Zotero("cette note n'a ni `zotero` ni `citekey` dans ses propriétés".into()))?;
                self.zotero_item_by_citekey(ck)?.ok_or_else(|| Error::Zotero(format!("@{ck} absent du miroir Zotero")))?
            }
        };
        let new = fiche::replace_block(&content, &fiche::annotations_block(&d.item, &d.annotations, lang));
        let _ = history::snapshot(&self.vault, rel, &content, 0, true);
        self.save_note(rel, &new, history_interval)?;
        Ok(new)
    }

    /// BibTeX bibliography (every reference, or only the cited ones).
    pub fn bibtex(&self, only_cited: bool, paths: Option<&[String]>) -> Result<String> {
        let all = self.library_items()?;
        if !only_cited {
            return Ok(bib::to_bibtex(&all));
        }
        let keys: std::collections::HashSet<String> = match paths {
            Some(ps) => {
                let mut s = std::collections::HashSet::new();
                for p in ps {
                    s.extend(self.index.citekeys_of(p)?.into_iter().map(|k| k.to_lowercase()));
                }
                s
            }
            None => self.index.all_citekeys()?.into_iter().map(|k| k.to_lowercase()).collect(),
        };
        let items: Vec<_> = all.into_iter().filter(|i| keys.contains(&i.citekey.to_lowercase())).collect();
        Ok(bib::to_bibtex(&items))
    }

    /// Exports a note (Pandoc for docx/pdf/odt/tex, native HTML if Pandoc is missing).
    pub fn export_note(&self, rel: &str, opts: &ExportOptions, settings: &Settings) -> Result<String> {
        let content = self.vault.read(rel)?;
        let work = self.vault.eloria_dir().join("tmp").join("export");
        if opts.format == "html" && export::pandoc_version(&settings.pandoc_path).is_none() {
            let title = self.index.title_of(rel)?.unwrap_or_else(|| vault::stem(rel));
            std::fs::write(&opts.output, export::standalone_html(&content, &title))?;
            return Ok(opts.output.clone());
        }
        let bib_path = if opts.citations {
            let b = self.bibtex(true, Some(&[rel.to_string()]))?;
            std::fs::create_dir_all(&work)?;
            let p = work.join("refs.bib");
            std::fs::write(&p, b)?;
            Some(p)
        } else {
            None
        };
        let note_dir = self.vault.resolve(rel)?.parent().map(Path::to_path_buf).unwrap_or_else(|| self.root().to_path_buf());
        let csl = (!settings.csl_path.is_empty()).then(|| PathBuf::from(&settings.csl_path));
        let out = export::pandoc_export(
            &settings.pandoc_path,
            &content,
            &[note_dir, self.root().to_path_buf()],
            &work,
            opts,
            bib_path.as_deref(),
            csl.as_deref(),
        )?;
        Ok(out.to_string_lossy().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_updates_links() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        ws.create_note("", "Cible", "# Cible").unwrap();
        ws.create_note("Dossier", "Source", "Voir [[Cible]], [[cible#Sec|alias]] et [[Dossier/Source]].").unwrap();
        let r = ws.rename("Cible.md", "Archives/Nouveau nom.md", true).unwrap();
        assert_eq!(r.updated, vec!["Dossier/Source.md"]);
        let c = ws.vault.read("Dossier/Source.md").unwrap();
        assert_eq!(c, "Voir [[Nouveau nom]], [[Nouveau nom#Sec|alias]] et [[Dossier/Source]].");
        assert_eq!(ws.index.backlinks("Archives/Nouveau nom.md").unwrap().len(), 2);

        // Folder rename: links with a path are rewritten
        let r = ws.rename("Dossier", "Projet", true).unwrap();
        assert_eq!(r.updated, vec!["Projet/Source.md"]);
        assert!(ws.vault.read("Projet/Source.md").unwrap().contains("[[Projet/Source]]"));
    }

    #[test]
    fn zotero_end_to_end() {
        let d = tempfile::tempdir().unwrap();
        let zdir = d.path().join("zotero");
        std::fs::create_dir_all(&zdir).unwrap();
        crate::zotero::tests::fake_zotero(&zdir.join("zotero.sqlite"));
        let ws = Workspace::open(d.path().join("coffre")).unwrap();
        let settings = Settings { zotero_data_dir: zdir.to_string_lossy().to_string(), ..Default::default() };
        let st = ws.zotero_sync(&settings, Source::Sqlite).unwrap();
        assert_eq!(st.items, 2);

        // "Create reading note": the item's source is created (filed according to its collection),
        // then an attached reading note, with the empty "Eloria method" outline.
        let f = ws.create_fiche("ITEMAAAA", &settings).unwrap();
        assert!(f.created);
        assert_eq!(f.path, "Fiches de lecture/Fiche @smith2024graph.md");
        let src = ws.existing_source(&ws.zotero_item("ITEMAAAA").unwrap().item).unwrap().unwrap();
        assert_eq!(src, "Sources/Thèse/Ch2 État de l'art/@smith2024graph.md");
        let fiche = ws.vault.read(&f.path).unwrap();
        assert!(fiche.contains("source: '[[@smith2024graph]]'") || fiche.contains("source: \"[[@smith2024graph]]\""));
        assert!(fiche.contains("## Problématique") && fiche.contains("## Commentaire"));
        // The reading note links to its source: incoming link in the index.
        assert_eq!(ws.index.backlinks(&src).unwrap().len(), 1);
        // Several reading notes per source.
        let f2 = ws.create_fiche("ITEMAAAA", &settings).unwrap();
        assert_eq!(f2.path, "Fiches de lecture/Fiche @smith2024graph 1.md");
        assert_eq!(ws.fiches_of("ITEMAAAA").unwrap().len(), 2);
        assert!(!ws.import_source("ITEMAAAA", &settings).unwrap().created);

        // The user writes in the source, then resyncs the annotations.
        let c = ws.vault.read(&src).unwrap().replace("# Graph", "Note perso.\n\n# Graph");
        ws.save_note(&src, &c, 300).unwrap();
        let synced = ws.sync_fiche(&src, 300, Lang::Fr).unwrap();
        assert!(synced.contains("Note perso."));
        assert!(synced.contains("Fraud rings form dense subgraphs"));

        // Citation in another note -> targeted bibliography
        ws.create_note("", "Chapitre 2", "Comme montré [@smith2024graph, p. 12].").unwrap();
        let b = ws.bibtex(true, Some(&["Chapitre 2.md".to_string()])).unwrap();
        assert!(b.contains("@article{smith2024graph"));
        assert!(!b.contains("kone2019dev"));
        // Cited by the chapter, the source (citekey) and both reading notes (citekey).
        assert_eq!(ws.index.notes_citing("smith2024graph").unwrap().len(), 4);
        assert_eq!(ws.stats().unwrap().fiches, 2);
    }

    /// Word export with bibliography. Skipped if Pandoc is not installed.
    #[test]
    fn pandoc_export_if_available() {
        if crate::export::pandoc_version("").is_none() {
            eprintln!("pandoc absent : test ignoré");
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let zdir = d.path().join("zotero");
        std::fs::create_dir_all(&zdir).unwrap();
        crate::zotero::tests::fake_zotero(&zdir.join("zotero.sqlite"));
        let ws = Workspace::open(d.path().join("coffre")).unwrap();
        let settings = Settings { zotero_data_dir: zdir.to_string_lossy().to_string(), ..Default::default() };
        ws.zotero_sync(&settings, Source::Sqlite).unwrap();
        ws.create_note("", "Chapitre", "---\ntitre: Chapitre 2\n---\n# Intro\n\nVoir [[Autre]] et [@smith2024graph, p. 4].\n").unwrap();
        let out = d.path().join("chapitre.docx");
        let opts = ExportOptions {
            format: "docx".into(),
            output: out.to_string_lossy().to_string(),
            citations: true,
            strip_annotations: true,
            toc: false,
        };
        ws.export_note("Chapitre.md", &opts, &settings).unwrap();
        assert!(std::fs::metadata(&out).unwrap().len() > 2000);
        // HTML: the citation is resolved by citeproc
        let html = d.path().join("chapitre.html");
        let opts = ExportOptions { format: "html".into(), output: html.to_string_lossy().to_string(), ..opts };
        ws.export_note("Chapitre.md", &opts, &settings).unwrap();
        let h = std::fs::read_to_string(&html).unwrap();
        assert!(h.contains("Smith"), "bibliographie absente");
        assert!(!h.contains("[[Autre]]"));
    }

    #[test]
    fn library_import() {
        let d = tempfile::tempdir().unwrap();
        let zdir = d.path().join("zotero");
        std::fs::create_dir_all(&zdir).unwrap();
        crate::zotero::tests::fake_zotero(&zdir.join("zotero.sqlite"));
        let ws = Workspace::open(d.path().join("coffre")).unwrap();
        let settings = Settings { zotero_data_dir: zdir.to_string_lossy().to_string(), ..Default::default() };
        ws.zotero_sync(&settings, Source::Sqlite).unwrap();
        // A source already exists (imported before Zotero tags were carried over).
        let smith = ws.import_source("ITEMAAAA", &settings).unwrap().path;
        ws.set_property(&smith, "tags", serde_json::json!(["source"]), 300).unwrap();
        let before = ws.vault.read(&smith).unwrap();

        // PDF of the second item in Zotero storage: it must be copied into the vault.
        let store = zdir.join("storage").join("ATTBBBBB");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join("kone.pdf"), b"%PDF-1.7 kone").unwrap();
        crate::zotero::tests::add_attachment(&zdir.join("zotero.sqlite"), "ATTBBBBB", 5, "storage:kone.pdf");
        ws.zotero_sync(&settings, Source::Sqlite).unwrap();

        let r = ws.import_library(&settings, true).unwrap();
        assert_eq!(r, ImportReport { created: 1, skipped: 1, pdfs: 1, updated: 1, failed: vec![] });
        // The existing source got the Zotero tag (GNN); #important comes from an annotation.
        let tags = crate::markdown::parse_note(&ws.vault.read(&smith).unwrap()).tags;
        assert_eq!(tags, vec!["source", "gnn", "important"]);
        // Only the tags changed: the body is identical.
        let body = |c: &str| crate::markdown::split_frontmatter(c).1.to_string();
        assert_eq!(body(&ws.vault.read(&smith).unwrap()), body(&before));
        // New source filed as in Zotero, PDF copied and referenced.
        let kone = "Sources/Thèse/@kone2019dev.md";
        assert!(ws.vault.exists(kone));
        assert_eq!(std::fs::read(ws.vault.resolve("PDF/Thèse/@kone2019dev.pdf").unwrap()).unwrap(), b"%PDF-1.7 kone");
        let k = crate::markdown::parse_note(&ws.vault.read(kone).unwrap());
        assert_eq!(k.frontmatter["pdf"], "PDF/Thèse/@kone2019dev.pdf");
        assert_eq!(k.frontmatter["type"], "source");
        // Zotero deleted: the reader still finds the PDF, inside the vault.
        std::fs::remove_dir_all(zdir.join("storage")).unwrap();
        let p = ws.zotero_pdf("ITEMBBBB", &settings).unwrap().unwrap();
        assert!(p.ends_with("PDF/Thèse/@kone2019dev.pdf") || p.ends_with("PDF\\Thèse\\@kone2019dev.pdf"));
        assert_eq!(ws.fiches_map().unwrap().len(), 2);
        // A second import creates nothing.
        assert_eq!(ws.import_library(&settings, false).unwrap().created, 0);
        assert_eq!(ws.import_preview(None).unwrap(), ImportPreview { total: 2, existing: 2, new: 0 });
        // The citation key changes (Better BibTeX installed): no duplicate thanks to the Zotero identifier.
        let c = ws.vault.read(&smith).unwrap().replace("citekey: smith2024graph", "citekey: smithGNN2024");
        ws.save_note(&smith, &c, 300).unwrap();
        assert_eq!(ws.import_preview(None).unwrap().new, 0);
        assert_eq!(ws.import_library(&settings, false).unwrap().created, 0);
        // Reading notes do not count as sources: a reading note does not block the import.
        assert_eq!(ws.fiches_of("ITEMAAAA").unwrap().len(), 0);
        // Old import (before sources): the empty note is a duplicate, the one with writing stays.
        let legacy = "---
type: fiche
zotero: ITEMAAAA
---
# Graph

## Problématique

<!-- zotero:debut -->
<!-- zotero:fin -->
";
        ws.save_note("Fiches de lecture/@smith2024graph.md", legacy, 300).unwrap();
        ws.save_note("Fiches de lecture/@kone2019dev.md", &legacy.replace("ITEMAAAA", "ITEMBBBB").replace("## Problématique
", "## Problématique
À garder.
"), 300).unwrap();
        assert_eq!(ws.legacy_imports().unwrap(), vec!["Fiches de lecture/@smith2024graph.md"]);
        assert_eq!(ws.clean_legacy_imports().unwrap(), 1);
        assert!(!ws.vault.exists("Fiches de lecture/@smith2024graph.md") && ws.vault.exists("Fiches de lecture/@kone2019dev.md"));
        assert!(ws.legacy_imports().unwrap().is_empty());
    }

    #[test]
    fn pdf_annotations_in_source() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        ws.vault.write_bytes("PDF/@a.pdf", b"%PDF-1.4").unwrap();
        ws.create_note("Sources", "@a", "---\ntype: source\ncitekey: a\npdf: PDF/@a.pdf\n---\n# A\n\nMes notes.").unwrap();
        let ann = PdfAnnotation { id: "1".into(), page: 4, kind: "highlight".into(), color: "jaune".into(), text: "Les mules transfèrent vite.".into(), comment: "Clé pour le ch. 2".into(), rects: vec![[0.1, 0.3, 0.4, 0.02]], created: String::new() };
        assert_eq!(ws.save_pdf_annotations("PDF/@a.pdf", &[ann], &s).unwrap(), vec!["Sources/@a.md"]);
        let src = ws.vault.read("Sources/@a.md").unwrap();
        assert!(src.contains("Mes notes.") && src.contains("Les mules transfèrent vite.") && src.contains("eloria-pdf:PDF/@a.pdf#page=4"));
        // Searchable: the highlighted text is in the full-text index.
        assert_eq!(ws.index.search("mules transfèrent", 5).unwrap()[0].path, "Sources/@a.md");
        assert_eq!(ws.pdf_annotations("PDF/@a.pdf").unwrap().len(), 1);
        // Same annotations: the source is not rewritten.
        let again = ws.pdf_annotations("PDF/@a.pdf").unwrap();
        assert!(ws.save_pdf_annotations("PDF/@a.pdf", &again, &s).unwrap().is_empty());
    }

    #[test]
    fn vault_vectors() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        ws.create_note("", "Fraude", "# Fraude\n\nComptes mules.").unwrap();
        ws.create_note("Modèles", "M", "{{title}}").unwrap();
        ws.create_note("Sources", "@k", "---\ntype: source\nresume: Résumé sur le mobile money.\n---\n# K").unwrap();
        let stale = ws.semantic_stale("t:m", &s).unwrap();
        assert_eq!(stale.len(), 2);
        assert!(ws.semantic_text("Sources/@k.md").unwrap().contains("Résumé sur le mobile money"));
        for (p, m) in &stale {
            ws.semantic_store(p, *m, "t:m", if p.contains("Fraude") { &[1.0, 0.0] } else { &[0.0, 1.0] }).unwrap();
        }
        assert!(ws.semantic_stale("t:m", &s).unwrap().is_empty());
        let near = ws.semantic_nearest(&[1.0, 0.1], "t:m", 1).unwrap();
        assert_eq!((near[0].0.as_str(), near[0].1.as_str()), ("Fraude.md", "Fraude"));
        // Note changed: to embed again.
        ws.save_note("Fraude.md", "# Fraude\n\nAutre texte.", 0).unwrap();
        assert_eq!(ws.semantic_stale("t:m", &s).unwrap().len(), 1);
    }

    #[test]
    fn latex_bibliography() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        ws.create_note("Sources", "@a2019", "---\ntype: source\ntitre: Article A\nauteurs: [Ana Diallo]\nannee: 2019\ncitekey: a2019\n---\n# A").unwrap();
        let tex = ws.latex_new("Thèse", "Chapitre 2", "chapitre").unwrap();
        assert_eq!(tex, "Thèse/Chapitre 2.tex");
        assert!(ws.vault.tree().unwrap().children.iter().any(|c| c.children.iter().any(|f| f.kind == "tex")));
        let src = ws.read_text(&tex).unwrap().replace("\\autocite{cle}", "\\autocite{a2019} et \\cite{inconnu}");
        ws.write_text(&tex, &src, 0).unwrap();
        let (bib, n, missing) = ws.latex_write_bib(&tex).unwrap().unwrap();
        assert_eq!((bib.as_str(), n, missing), ("Thèse/references.bib", 1, vec!["inconnu".to_string()]));
        assert!(ws.vault.read(&bib).unwrap().contains("@article{a2019,"));
        // A hand-written .bib is never replaced.
        ws.vault.write(&bib, "@book{perso, title={Mien}}").unwrap();
        assert!(ws.latex_write_bib(&tex).unwrap().is_none());
        assert_eq!(ws.vault.read(&bib).unwrap(), "@book{perso, title={Mien}}");
        assert!(ws.read_text("Notes.md").is_err());
    }

    #[test]
    fn literature_review() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        ws.create_note("Sources", "@a2019", "---\ntype: source\ntitre: Article A\nauteurs: [Ana Diallo]\nannee: 2019\ncitekey: a2019\nresume: Résumé de A sur la fraude mobile.\n---\n# Article A").unwrap();
        ws.create_note("Fiches de lecture", "Fiche @a2019", "---\ntype: fiche\ncitekey: a2019\n---\n# Fiche A\n\n## Problématique\n\nDétecter la fraude par graphe.").unwrap();
        let r = ws.create_review("Revues", "Revue fraude", "Comment détecter la fraude ?", Lang::Fr).unwrap();
        assert_eq!(r, "Revues/Revue fraude.md");
        ws.review_set_article(&r, "a2019", "à lire", 0).unwrap();
        ws.review_set_article(&r, "b2020", "exclu", 0).unwrap();
        let rv = ws.review_set_article(&r, "a2019", "inclus", 0).unwrap();
        assert_eq!(rv.question, "Comment détecter la fraude ?");
        let a = &rv.articles[0];
        assert_eq!((a.statut.as_str(), a.author_year.as_str(), a.source.as_str()), ("inclus", "Diallo (2019)", "Sources/@a2019.md"));
        assert_eq!(a.fiches, vec!["Fiches de lecture/Fiche @a2019.md"]);
        assert_eq!(rv.articles[1].title, "@b2020");
        // The draft only sees included articles.
        let ps = ws.review_passages(&r).unwrap();
        assert_eq!(ps.len(), 1);
        assert!(ps[0].text.contains("Résumé de A") && ps[0].text.contains("Détecter la fraude par graphe"));
        assert!(ws.review_set_article(&r, "a2019", "peut-être", 0).is_err());
        assert_eq!(ws.review_set_article(&r, "b2020", "", 0).unwrap().articles.len(), 1);
    }

    #[test]
    fn reading_chain_and_statistics() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let f = ws.create_note("Fiches de lecture", "Fiche @a", "---\ntype: fiche\ncitekey: a2020\nstatut: à lire\n---\n# F\n\n## A\n\n## B\n").unwrap();
        assert!(ws.reading_chain(&f, &s).unwrap().is_empty());
        let c = ws.vault.read(&f).unwrap().replace("## A\n", "## A\nUne section écrite avec assez de mots pour compter.\n");
        ws.save_note(&f, &c, 0).unwrap();
        assert_eq!(ws.reading_chain(&f, &s).unwrap(), vec![f.clone()]);
        assert!(ws.vault.read(&f).unwrap().contains("statut: en cours"));
        let c = ws.vault.read(&f).unwrap().replace("## B\n", "## B\nLa deuxième section est maintenant écrite aussi.\n");
        ws.save_note(&f, &c, 0).unwrap();
        ws.reading_chain(&f, &s).unwrap();
        assert!(ws.vault.read(&f).unwrap().contains("statut: lue"));
        // A chapter that cites the item: the read note becomes "exploitée".
        let ch = ws.create_note("Thèse", "Ch2", "---\ntype: chapitre\n---\n# Ch2\n\nComme le montre [@a2020].").unwrap();
        assert_eq!(ws.reading_chain(&ch, &s).unwrap(), vec![f.clone()]);
        assert!(ws.vault.read(&f).unwrap().contains("statut: exploitée"));
        // Never backwards, and the setting is respected.
        let off = Settings { auto_reading_status: false, ..Settings::default() };
        assert!(ws.reading_chain(&f, &off).unwrap().is_empty());
        // Words written today.
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        assert!(ws.writing_stats().get(&today).copied().unwrap_or(0) >= 15);
        let db = ws.dashboard().unwrap();
        assert_eq!(db.fiches["exploitée"], 1);
        assert_eq!(db.written.len(), 14);
        assert!(db.written_today >= 15 && db.chapters.len() == 1);
        // Checks: orphans (the reading note cites, the chapter cites: connected).
        ws.create_note("", "Isolée", "# Isolée\n\nTexte sans aucun lien.").unwrap();
        assert_eq!(ws.orphan_notes(&s).unwrap(), vec!["Isolée.md"]);
        assert_eq!(ws.check_citations(&ch).unwrap()[0].known, false);
    }

    #[test]
    fn inbox_and_import() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path().join("coffre")).unwrap();
        let s = Settings::default();
        let a = ws.capture("Idée : comparer GNN et XGBoost\nsur les données BCEAO", None, &s).unwrap();
        assert_eq!(a, "Boîte de réception/Idée comparer GNN et XGBoost.md");
        ws.capture("https://exemple.org/page", None, &s).unwrap();
        assert_eq!(ws.inbox(&s).unwrap().len(), 2);
        let src = d.path().join("Mon export");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("A.md"), "Lien vers [B](B.md)").unwrap();
        std::fs::write(src.join("B.md"), "# B").unwrap();
        let r = ws.import_markdown(&src, &s).unwrap();
        assert_eq!((r.notes, r.links_rewritten, r.folder.as_str()), (2, 1, "Importé/Mon export"));
        // Indexed: the rewritten link is resolved.
        assert_eq!(ws.index.backlinks("Importé/Mon export/B.md").unwrap().len(), 1);
    }

    #[test]
    fn citation_network_cases() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        for (ck, doi) in [("a2020", "10.1/A"), ("b2019", "10.1/b"), ("c2018", "10.1/c")] {
            ws.create_note("Sources", &format!("@{ck}"), &format!("---\ntype: source\ntitre: {ck}\nauteurs: [Ana Diallo]\nannee: 2020\ncitekey: {ck}\ndoi: {doi}\n---\n# {ck}")).unwrap();
        }
        ws.store_citations("10.1/a", &["10.1/B".into(), "10.9/hors".into()]).unwrap();
        let n = ws.citation_network().unwrap();
        assert_eq!(n.nodes.len(), 3);
        assert_eq!(n.edges, vec![("a2020".to_string(), "b2019".to_string())]);
        assert_eq!(n.missing.len(), 2);
        assert!(n.nodes.iter().find(|x| x.citekey == "a2020").unwrap().fetched);
    }

    #[test]
    fn routines() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        ws.create_note("Fiches de lecture", "Fiche @a", "---\ntype: fiche\nstatut: lue\n---\n# Fiche A").unwrap();
        let n = ws.create_note("", "Idée", "# Idée").unwrap();
        // Scheduled review: shown on the dashboard on the day.
        assert!(ws.schedule_review(&n, 3, 0).unwrap().is_some());
        assert!(ws.dashboard().unwrap().reviews_due.is_empty());
        ws.set_property(&n, "revoir_le", Value::String("2020-01-01".into()), 0).unwrap();
        assert_eq!(ws.dashboard().unwrap().reviews_due[0].0, "Idée.md");
        ws.schedule_review(&n, 0, 0).unwrap();
        assert!(ws.dashboard().unwrap().reviews_due.is_empty());
        // Concentration.
        assert_eq!(ws.add_focus(25).unwrap(), 25);
        assert_eq!(ws.add_focus(25).unwrap(), 50);
        let db = ws.dashboard().unwrap();
        assert_eq!((db.focus_today, db.fiches_read_week), (50, 1));
        // Weekly review: created once, with the figures.
        let r = ws.weekly_review(&s).unwrap();
        assert!(r.starts_with("Journal/Revue de la semaine "));
        let c = ws.vault.read(&r).unwrap();
        assert!(c.contains("Fiches lues ou exploitées : **1**") && c.contains("Concentration : **50 min**") && c.contains("[[Idée|Idée]]"));
        assert_eq!(ws.weekly_review(&s).unwrap(), r);
    }

    #[test]
    fn knowledge_map_cases() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        ws.create_concept_note("Comptes mules", "Transit de la fraude.", None, &s).unwrap();
        ws.create_note("Concepts", "Graphes", "---\ntype: concept\n---\n# Graphes\n\nVoir [[Comptes mules]].").unwrap();
        ws.create_note("Thèse", "Ch2", "---\ntype: chapitre\nchapitre: ch2\n---\n# Ch2\n\nLes comptes mules et les graphes.").unwrap();
        ws.create_note("Thèse/Questions", "Q1", "---\ntype: question\n---\n# Q1\n\n[[Graphes]] ?").unwrap();
        ws.create_note("", "Courses", "Les comptes mules ne sont pas ici.").unwrap();
        let m = ws.knowledge_map(false, &s).unwrap();
        assert_eq!(m.nodes.len(), 4, "{:?}", m.nodes);
        let has = |a: &str, b: &str, k: &str| m.edges.iter().any(|(x, y, z)| ((x == a && y == b) || (x == b && y == a)) && z == k);
        assert!(has("Concepts/Graphes.md", "Concepts/Comptes mules.md", "lien"));
        assert!(has("Thèse/Ch2.md", "Concepts/Comptes mules.md", "mention"));
        assert!(has("Thèse/Ch2.md", "Concepts/Graphes.md", "mention"));
        assert!(has("Thèse/Questions/Q1.md", "Concepts/Graphes.md", "lien"));
        // No duplicate: the Graphes -> Comptes mules link is not also a mention.
        assert_eq!(m.edges.len(), 4, "{:?}", m.edges);
        assert_eq!(m.nodes.iter().find(|n| n.id == "Concepts/Graphes.md").unwrap().degree, 3);
    }

    #[test]
    fn personal_taxonomy() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        ws.create_note("", "A", "---\ntags: [fraude/mules]\n---\n# A\n").unwrap();
        ws.create_note("", "B", "# B\n\nLe phishing vise les usagers du mobile money.").unwrap();
        assert!(!ws.taxonomy(&s).unwrap().exists);
        let p = ws.taxonomy_create(&s).unwrap();
        assert_eq!(p, "Taxonomie.md");
        assert!(ws.vault.read(&p).unwrap().contains("- fraude\n  - mules\n"));
        ws.save_note(&p, "---\ntype: taxonomie\n---\n# Taxonomie\n\n- Fraude\n  - Mules (comptes mules)\n  - Ingénierie sociale (phishing)\n", 0).unwrap();
        let t = ws.taxonomy(&s).unwrap();
        let get = |tag: &str| t.nodes.iter().find(|n| n.node.tag == tag).unwrap();
        assert_eq!(get("fraude").notes.len(), 1);
        assert_eq!(get("fraude/mules").notes, vec![("A.md".to_string(), "A".to_string())]);
        let pr = ws.taxonomy_proposals(&s).unwrap();
        assert_eq!(pr, vec![("B.md".to_string(), "B".to_string(), vec!["fraude/ingenierie-sociale".to_string()])]);
        ws.add_tags("B.md", &pr[0].2).unwrap();
        assert!(ws.taxonomy_proposals(&s).unwrap().is_empty());
        assert_eq!(ws.taxonomy(&s).unwrap().nodes[0].notes.len(), 2);
    }

    #[test]
    fn vault_entities() {
        use crate::entities::Kind;
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let w = online::Work { title: "GNN fraud".into(), year: Some(2024), authors: vec![("Jane".into(), "Smith".into())], ..Default::default() };
        ws.add_reference(&w, &s).unwrap();
        ws.create_note("", "Ch2", "Smith et al. montrent que la BCEAO surveille Orange Money ; code en Python.").unwrap();
        ws.create_note("", "Ch3", "Au Burkina Faso, la BCEAO publie des données.").unwrap();
        let rows = ws.entities(&s).unwrap();
        let get = |k: Kind, n: &str| rows.iter().find(|r| r.kind == k && r.name == n).map(|r| r.notes.len());
        assert_eq!(get(Kind::Organisation, "BCEAO"), Some(2));
        assert_eq!(get(Kind::Personne, "Jane Smith"), Some(1));
        assert_eq!(get(Kind::Technologie, "Python"), Some(1));
        assert_eq!(get(Kind::Lieu, "Burkina Faso"), Some(1));
        assert_eq!(rows[0].name, "BCEAO");
    }

    #[test]
    fn vault_themes() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let g = "Les graphes de transactions révèlent la fraude organisée des comptes mules dans les réseaux de paiement mobile, avec des communautés denses et rapides.";
        let x = "Les valeurs de Shapley expliquent les prédictions du modèle ; SHAP et LIME attribuent une importance locale aux variables explicatives du score.";
        for i in 0..3 {
            ws.create_note("Graphes", &format!("G{i}"), &format!("# G{i}\n\n{g} Variante {i}.")).unwrap();
            ws.create_note("XAI", &format!("X{i}"), &format!("# X{i}\n\n{x} Variante {i}.")).unwrap();
        }
        ws.create_note("", "Court", "Trop court.").unwrap();
        let t = ws.topics(2, &s).unwrap();
        assert_eq!(t.len(), 2);
        for topic in &t {
            let folders: HashSet<&str> = topic.notes.iter().map(|n| n.0.split('/').next().unwrap()).collect();
            assert_eq!(folders.len(), 1, "{topic:?}");
        }
        assert!(t.iter().all(|x| x.notes.iter().all(|n| n.0 != "Court.md")));
    }

    #[test]
    fn concept_base_and_evolution() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let c = ws.create_concept_note("Comptes mules", "Comptes de transit de la fraude.", None, &s).unwrap().path;
        ws.create_note("Concepts", "Blanchiment", "---\ntype: concept\n---\n# Blanchiment\n\nLié aux [[Comptes mules]].").unwrap();
        let w = online::Work { title: "Mules".into(), abstract_note: "We find that comptes mules relay stolen funds in minutes.".into(), year: Some(2024), authors: vec![("".into(), "Diallo".into())], ..Default::default() };
        let src = ws.add_reference(&w, &s).unwrap().path;
        ws.create_note("Thèse/Questions", "Q1", "---\ntype: question\ncree_le: 2026-01-10\n---\n# Peut-on repérer les comptes mules ?\n").unwrap();
        ws.create_note("", "Courses", "Pain, lait.").unwrap();
        let rows = ws.concepts_overview(&s).unwrap();
        let r = rows.iter().find(|r| r.path == c).unwrap();
        assert_eq!((r.definition.as_str(), r.sources, r.notes), ("Comptes de transit de la fraude.", 1, 3));
        // New definition: the evolution keeps it (history is dated to the second).
        let first = ws.vault.read(&c).unwrap();
        ws.save_note(&c, &first, 0).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let content = ws.vault.read(&c).unwrap().replace("Comptes de transit de la fraude.", "Comptes qui relaient l'argent volé en quelques minutes.");
        ws.save_note(&c, &content, 0).unwrap();
        let dd = ws.concept_detail(&c, &s).unwrap();
        assert_eq!(dd.definitions.len(), 2, "{:?}", dd.definitions);
        assert_eq!(dd.definitions[1].definition, "Comptes qui relaient l'argent volé en quelques minutes.");
        assert_eq!(dd.related, vec![("Concepts/Blanchiment.md".to_string(), "Blanchiment".to_string())]);
        assert_eq!(dd.questions.len(), 1);
        assert!(dd.timeline.iter().any(|m| m.path == src && m.excerpt.contains("comptes mules relay")));
        assert!(dd.timeline.iter().all(|m| m.path != "Courses.md"));
        // Knowledge card: block regenerated, personal text kept.
        let card = ws.knowledge_card(&c, &s).unwrap();
        assert_eq!(card, "Fiches de connaissance/Comptes mules.md");
        let k = ws.vault.read(&card).unwrap();
        assert!(k.contains("## Points clés des sources") && k.contains("[@") && k.contains("- [[Blanchiment]]") && k.contains("## Évolution"), "{k}");
        ws.save_note(&card, &k.replace("# Comptes mules\n", "# Comptes mules\n\nMa synthèse.\n"), 0).unwrap();
        ws.knowledge_card(&c, &s).unwrap();
        let k2 = ws.vault.read(&card).unwrap();
        assert!(k2.contains("Ma synthèse.") && k2.matches(crate::knowledge::CARD_START).count() == 1);
        assert_eq!(ws.concept_detail(&c, &s).unwrap().card, card);
        // The knowledge card does not count as a mention.
        assert_eq!(ws.concepts_overview(&s).unwrap().iter().find(|r| r.path == c).unwrap().notes, 3);
    }

    #[test]
    fn research_statistics() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let mk = |t: &str, a: &str, y: i32, venue: &str, kw: &[&str]| {
            let w = online::Work { title: t.into(), year: Some(y), venue: venue.into(), authors: vec![("".into(), a.into())], keywords: kw.iter().map(|k| k.to_string()).collect(), ..Default::default() };
            vault::stem(&ws.add_reference(&w, &s).unwrap().path).trim_start_matches('@').to_string()
        };
        let a = mk("Rings", "Smith", 2024, "IEEE Access", &["gnn"]);
        let b = mk("Mules", "Kone", 2021, "IEEE Access", &["gnn", "mules"]);
        let _c = mk("Chain", "Abbas", 2020, "Zenodo", &["blockchain"]);
        let _e = mk("Chain2", "Ba", 2022, "Zenodo", &["blockchain"]);
        let _f = mk("Chain3", "Diop", 2023, "Zenodo", &["blockchain"]);
        ws.create_note("Thèse", "Ch2", &format!("---\ntype: chapitre\nchapitre: ch2\ntags: [gnn]\n---\n# Ch2\n\n[@{a}] et [@{b}]. Encore [@{a}].")).unwrap();
        ws.create_note("", "Brouillon", &format!("Voir [@{a}].")).unwrap();
        let st = ws.research_stats((2026, 11), &s).unwrap();
        assert_eq!((st.documents_cited, st.documents_total), (2, 5));
        assert_eq!(st.top_documents[0].2, 2);
        assert_eq!(st.top_documents[0].0, a);
        assert_eq!(st.top_venues[0], ("IEEE Access".to_string(), st.citations_total));
        assert_eq!(st.top_authors.len(), 2);
        assert_eq!(st.years, vec![(2021, 1), (2024, 1)]);
        assert_eq!(st.heat_months.len(), 12);
        assert_eq!(st.cover_chapters, vec!["ch2".to_string()]);
        let gi = st.cover_themes.iter().position(|t| t == "gnn").unwrap();
        assert_eq!(st.cover[gi][0], 2);
        assert_eq!(st.gaps, vec![("blockchain".to_string(), 3)]);
    }

    #[test]
    fn material_for_citation_consistency() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let w = online::Work { title: "Rings".into(), abstract_note: "Fraud rings form dense subgraphs in mobile money transaction graphs.".into(), year: Some(2024), authors: vec![("".into(), "Smith".into())], ..Default::default() };
        let src = ws.add_reference(&w, &s).unwrap().path;
        let ck = vault::stem(&src).trim_start_matches('@').to_string();
        ws.create_note("Thèse", "Ch2", &format!("# Ch2\n\nLes réseaux de fraude forment des sous-graphes denses [@{ck}]. Une phrase sans citation. Inconnu [@absent2020].\n")).unwrap();
        let (sentences, ev) = ws.support_job("Thèse/Ch2.md", 30).unwrap();
        assert_eq!(sentences.len(), 2);
        assert!(ev[&ck].contains("dense subgraphs"), "{ev:?}");
        assert!(ev["absent2020"].is_empty());
    }

    #[test]
    fn argument_builder() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let w = online::Work { title: "Rings".into(), year: Some(2024), authors: vec![("".into(), "Smith".into())], ..Default::default() };
        let src = ws.add_reference(&w, &s).unwrap().path;
        let ck = vault::stem(&src).trim_start_matches('@').to_string();
        let p = ws.argument_new("Les GNN détectent la fraude", Lang::Fr).unwrap();
        assert_eq!(p, "Thèse/Arguments/Les GNN détectent la fraude.md");
        let (mut a, unknown) = ws.argument_load(&p).unwrap();
        assert!(unknown.is_empty());
        assert_eq!((a.arguments.len(), a.objections.len()), (1, 1));
        a.claim = "Les GNN repèrent les réseaux de fraude.".into();
        a.arguments[0] = crate::argument::Point { title: "Sous-graphes denses".into(), evidence: vec![format!("observés sur 2,3 M de transactions [@{ck}, p. 4]"), "même constat ailleurs [@inconnu2020]".into()], ..Default::default() };
        a.objections[0] = crate::argument::Point { title: "Données rares".into(), rebuttal: "l'apprentissage fédéré aide".into(), ..Default::default() };
        a.conclusion = "À retenir pour le ch4.".into();
        let unknown = ws.argument_save(&p, &a, Lang::Fr, 0).unwrap();
        assert_eq!(unknown, vec!["inconnu2020".to_string()]);
        let c = ws.vault.read(&p).unwrap();
        assert!(c.starts_with("---\ntype: argument\nstatut: brouillon\n") && c.contains("- Preuve : observés sur 2,3 M"), "{c}");
        assert_eq!(ws.argument_load(&p).unwrap().0, { let mut b = a.clone(); b.title = "Les GNN détectent la fraude".into(); b });
        let t = ws.argument_export(&p, "md", &s).unwrap();
        assert_eq!(t, "Thèse/Arguments/Les GNN détectent la fraude (texte).md");
        let tc = ws.vault.read(&t).unwrap();
        assert!(tc.contains("Premièrement, sous-graphes denses. Observés sur 2,3 M de transactions") && tc.contains("En conclusion, à retenir pour le ch4."), "{tc}");
        let tex = ws.argument_export(&p, "tex", &s).unwrap();
        assert!(tex.ends_with(".tex"));
        let x = ws.vault.read(&tex).unwrap();
        assert!(x.contains(&format!("\\autocite[p. 4]{{{ck}}}")) && x.contains("\\documentclass"), "{x}");
    }

    #[test]
    fn similarity_with_library() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let w = online::Work { title: "Mules".into(), abstract_note: "Mule accounts launder the proceeds of mobile money fraud through chains of rapid transfers.".into(), year: Some(2024), authors: vec![("".into(), "Diallo".into())], ..Default::default() };
        let src = ws.add_reference(&w, &s).unwrap().path;
        let ck = vault::stem(&src).trim_start_matches('@').to_string();
        ws.create_note("Thèse", "Ch3", "# Ch3\n\nEn Afrique de l'Ouest, mule accounts launder the proceeds of mobile money fraud through chains of transfers.\n").unwrap();
        let hits = ws.similarity_check("Thèse/Ch3.md").unwrap();
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!((hits[0].citekey.as_str(), hits[0].line, hits[0].cited, hits[0].path.as_str()), (ck.as_str(), 3, false, src.as_str()));
        assert!(hits[0].matched_words >= 10);
        // The source itself is not compared with itself.
        assert!(ws.similarity_check(&src).unwrap().is_empty());
    }

    #[test]
    fn outline_generator() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        ws.create_note("Thèse", "Ch2", "---\ntype: chapitre\nchapitre: ch2\n---\n# État de l'art\n").unwrap();
        ws.create_note("Concepts", "Graphes", "---\ntype: concept\nchapitres: [ch2]\ntags: [gnn]\n---\n# Graphes\n\nUn graphe relie des comptes. Suite.").unwrap();
        ws.create_note("Fiches", "Fiche A", "---\ntype: fiche\ncitekey: a2024\nannee: 2024\nchapitres: [ch2]\ntags: [fiche, gnn]\n---\n# Fiche A\n\n## Résultats\n\nLe GNN gagne 12 points de rappel. Détail.\n").unwrap();
        ws.create_note("Fiches", "Fiche B", "---\ntype: fiche\ncitekey: b2017\nchapitres: [ch2]\ntags: [fiche, xai]\n---\n# Fiche B\n").unwrap();
        ws.create_note("", "Hors périmètre", "---\ntags: [gnn]\n---\n# Hors\n").unwrap();
        let p = ws.generate_outline("chapter", "CH2", &s).unwrap();
        assert_eq!(p, "Thèse/Plans/Plan - État de l’art.md");
        let c = ws.vault.read(&p).unwrap();
        assert!(c.contains("type: brouillon-plan") && c.contains("# Plan : État de l"), "{c}");
        assert!(c.contains("- [[Graphes]] : Un graphe relie des comptes.") && c.contains("- [[Fiche A]] : Le GNN gagne 12 points de rappel. [@a2024]"), "{c}");
        assert!(c.contains("[[Fiche B]] [@b2017]") && !c.contains("Hors"), "{c}");
        // The outline itself does not enter the next outline; by tag.
        let p2 = ws.generate_outline("tag", "#gnn", &s).unwrap();
        let c2 = ws.vault.read(&p2).unwrap();
        assert!(c2.contains("[[Hors périmètre]]") && c2.contains("[[Graphes]]") && !c2.contains("Plan - "), "{c2}");
    }

    #[test]
    fn methodology_tracking() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let a = online::Work { title: "Fraud rings with GNNs".into(), abstract_note: "We train a graph neural network and compare with XGBoost.".into(), year: Some(2024), authors: vec![("".into(), "Smith".into())], ..Default::default() };
        let b = online::Work { title: "A survey of mobile money users".into(), year: Some(2021), authors: vec![("".into(), "Kone".into())], ..Default::default() };
        let sa = ws.add_reference(&a, &s).unwrap().path;
        let sb = ws.add_reference(&b, &s).unwrap().path;
        // B's reading note describes its method in its "Méthodes" section.
        let fb = ws.create_fiche_from_source(&sb, &s).unwrap().path;
        let c = ws.vault.read(&fb).unwrap().replace("## Méthodes\n", "## Méthodes\n\nQuestionnaire auprès de 400 usagers, puis une régression logistique.\n");
        ws.save_note(&fb, &c, 0).unwrap();
        let rows = ws.methods_overview().unwrap();
        let get = |n: &str| rows.iter().find(|r| r.name == n).unwrap_or_else(|| panic!("{n} absent : {:?}", rows.iter().map(|r| &r.name).collect::<Vec<_>>()));
        assert_eq!(get("Réseaux de neurones sur graphes (GNN)").documents.len(), 1);
        assert_eq!(get("Réseaux de neurones sur graphes (GNN)").documents[0].path, sa);
        // For B, the link leads to its reading note.
        assert_eq!(get("Régression logistique").documents[0].path, fb);
        assert_eq!(get("Enquête par questionnaire").documents.len(), 1);
        assert!(rows.iter().all(|r| r.name != "LIME"));
        // Personal method note: its aliases and the reading notes' links count.
        let m = ws.create_method_note("Graphe de transactions", &["transaction graph".into()], Lang::Fr).unwrap();
        assert_eq!(m, "Méthodes/Graphe de transactions.md");
        let c2 = ws.vault.read(&fb).unwrap() + "\nVoir [[Graphe de transactions]].\n";
        ws.save_note(&fb, &c2, 0).unwrap();
        let rows = ws.methods_overview().unwrap();
        let g = rows.iter().find(|r| r.name == "Graphe de transactions").unwrap();
        assert_eq!((g.path.as_str(), g.documents.len()), (m.as_str(), 1));
        // A note named after a catalogue method attaches to it.
        ws.create_method_note("SHAP", &[], Lang::Fr).unwrap();
        assert_eq!(ws.methods_overview().unwrap().iter().filter(|r| r.name == "SHAP").count(), 1);
    }

    #[test]
    fn thesis_workspace() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let w = online::Work { title: "Mules".into(), year: Some(2024), authors: vec![("".into(), "Diallo".into())], ..Default::default() };
        let src = ws.add_reference(&w, &s).unwrap().path;
        let ck = vault::stem(&src).trim_start_matches('@').to_string();
        ws.create_note("Thèse", "Ch1", "---\ntype: chapitre\nchapitre: ch1\nobjectif: 8000\nstatut: rédaction\n---\n# Introduction\n\n## Objectifs\n\n- [x] Poser le problème\n- [ ] Annoncer le plan\n").unwrap();
        ws.create_note("Thèse", "Ch2", "---\ntype: chapitre\nchapitre: ch2\ndepend_de: [ch1]\n---\n# État de l'art\n").unwrap();
        ws.create_note("Thèse/Questions", "Q1", &format!("---\ntype: question\nstatut: en cours\nchapitres: [ch2]\n---\n# Les comptes mules sont-ils détectables par graphe ?\n\nVoir [[@{ck}]].")).unwrap();
        ws.create_note("Concepts", "Graphes", "Lié à [[Q1]].").unwrap();
        let o = ws.thesis_overview().unwrap();
        assert_eq!(o.chapters.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), vec!["ch1", "ch2"]);
        let c1 = &o.chapters[0];
        assert_eq!((c1.target, c1.status.as_str(), c1.objectives.len()), (8000, "rédaction", 2));
        assert_eq!(o.layers, vec![vec!["ch1".to_string()], vec!["ch2".to_string()]]);
        assert_eq!(o.chapters[1].questions, vec!["Thèse/Questions/Q1.md".to_string()]);
        let q = &o.questions[0];
        assert_eq!((q.status.as_str(), q.chapters.clone(), q.sources.clone(), q.notes.clone()), ("en cours", vec!["ch2".to_string()], vec![ck.clone()], vec!["Concepts/Graphes.md".to_string()]));
        assert_eq!(ws.questions_for(&ck.to_uppercase()).unwrap().len(), 1);
    }

    #[test]
    fn new_document_flow() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        ws.create_note("Concepts", "Comptes mules", "---
tags: [mobile-money, fraude]
---
# Comptes mules

Les comptes mules blanchissent la fraude au mobile money.").unwrap();
        ws.create_note("", "Courses", "# Courses

Pain, lait.").unwrap();
        let w = online::Work { title: "Mule accounts and fraud in mobile money".into(), doi: "10.1/m".into(), year: Some(2024), abstract_note: "We detect mule accounts laundering fraud proceeds in mobile money networks.".into(), authors: vec![("Awa".into(), "Diallo".into())], ..Default::default() };
        let src = ws.add_reference(&w, &s).unwrap().path;
        // Suggested tags: yours, present in the item.
        assert_eq!(ws.propose_tags(&src).unwrap(), vec!["fraude", "mobile-money"]);
        assert!(ws.add_tags(&src, &["mobile-money".into(), "Fraude".into()]).unwrap());
        assert!(ws.propose_tags(&src).unwrap().is_empty());
        // Suggested links: the concept note, not the shopping list nor the source itself.
        let rel = ws.related_notes(&src, 5, &s).unwrap();
        assert_eq!(rel.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), vec!["Concepts/Comptes mules.md"]);
        // Reading note ready, with its links.
        let f = ws.create_fiche_with_links(&src, &["Concepts/Comptes mules.md".into()], &s).unwrap();
        let c = ws.vault.read(&f.path).unwrap();
        assert!(c.contains("**Voir aussi :** [[Comptes mules]]") && c.find("Voir aussi").unwrap() < c.find("## Problématique").unwrap(), "{c}");
        assert_eq!(ws.fiches_of_source(&src).unwrap(), vec![f.path.clone()]);
        assert!(ws.related_notes(&src, 5, &s).unwrap().iter().all(|(p, _)| p != &f.path));
    }

    #[test]
    fn reliability_report_cases() {
        use crate::reliability::{Level, Reliability};
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        for (ck, doi) in [("a2020", "10.1/a"), ("b2021", "10.1/b"), ("c2022", "10.1/c")] {
            let w = online::Work { title: ck.into(), doi: doi.into(), year: Some(2020), authors: vec![("".into(), ck.into())], ..Default::default() };
            let r = ws.add_reference(&w, &s).unwrap();
            ws.set_property(&r.path, "citekey", Value::String(ck.into()), 0).unwrap();
        }
        ws.create_note("", "Chapitre", "Comme le montre [@b2021].").unwrap();
        assert_eq!(ws.reliability_missing().unwrap().len(), 3);
        let ok = Reliability { doi: "10.1/a".into(), known: true, cited_by: 300, venue: "IEEE Access".into(), venue_type: "journal".into(), venue_h_index: Some(200), ..Default::default() };
        let bad = Reliability { doi: "10.1/b".into(), known: true, retracted: true, ..ok.clone() };
        ws.store_reliability(&ok).unwrap();
        ws.store_reliability(&bad).unwrap();
        assert_eq!(ws.reliability_missing().unwrap(), vec!["10.1/c".to_string()]);
        let rep = ws.reliability_report(Lang::Fr).unwrap();
        assert_eq!(rep.len(), 2);
        assert_eq!((rep[0].citekey.as_str(), rep[0].level, rep[0].cited), ("b2021", Level::Alerte, true));
        assert!(rep[0].signals[0].text.starts_with("Article rétracté"));
        assert_eq!((rep[1].level, rep[1].cited), (Level::Ok, false));
        assert!(rep[1].path.ends_with(".md"));
        assert!(ws.reliability_signals("https://doi.org/10.1/A", Lang::Fr).is_some());
    }

    #[test]
    fn captured_figure_in_source() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let pdf = d.path().join("..").join(format!("fig-{}.pdf", std::process::id()));
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        let (r, p) = ws.add_pdf(&pdf, None, &s).unwrap();
        let out = ws.attach_figure(&p, "Pièces jointes/Figures/a p4.png", 4, "", &s).unwrap();
        assert_eq!(out, vec![r.path.clone()]);
        ws.attach_figure(&p, "Pièces jointes/Figures/a p7.png", 7, "Architecture du modèle", &s).unwrap();
        let c = ws.vault.read(&r.path).unwrap();
        assert_eq!(c.matches("## Figures").count(), 1, "{c}");
        assert!(c.contains("![[Pièces jointes/Figures/a p4.png]]\n*Figure, p. 4*") && c.contains("*Architecture du modèle (p. 7)*"), "{c}");
        assert!(c.find("p4.png").unwrap() < c.find("p7.png").unwrap());
        std::fs::remove_file(&pdf).ok();
    }

    #[test]
    fn pdf_added_and_source_completed() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let pdf = d.path().join("..").join(format!("Kone_2021_mobile-money-{}.pdf", std::process::id()));
        std::fs::write(&pdf, b"%PDF-1.4 rien").unwrap();
        // Without a DOI: title taken from the file name, PDF copied into the vault.
        let (r, p) = ws.add_pdf(&pdf, None, &s).unwrap();
        let fm = markdown::parse_note(&ws.vault.read(&r.path).unwrap()).frontmatter;
        assert!(fm["titre"].as_str().unwrap().starts_with("Kone 2021 mobile money"));
        assert_eq!(fm["pdf"].as_str().unwrap(), p);
        assert!(p.starts_with(&format!("{}/@", s.pdf_folder.trim_matches('/'))) && ws.vault.exists(&p), "{p}");
        // PDF text: searchable and read by Ask Eloria.
        // The abstract read from the PDF completes the source that had none.
        let txt = "Kone 2021\n\nRésumé : Les comptes mules servent à blanchir la fraude au mobile money ; cette étude décrit leurs réseaux dans trois pays d'Afrique de l'Ouest et propose une détection par graphes.\n\nMots-clés : comptes mules, mobile money\n\n1. Introduction\n";
        assert_eq!(ws.store_pdf_text(&p, txt, 0).unwrap(), vec![r.path.clone()]);
        let fm1 = markdown::parse_note(&ws.vault.read(&r.path).unwrap()).frontmatter;
        assert!(fm1["resume"].as_str().unwrap().starts_with("Les comptes mules"));
        assert_eq!(fm1["tags"], serde_json::json!(["source", "comptes-mules", "mobile-money"]));
        assert!(ws.pdfs_without_text().unwrap().is_empty());
        let hits = ws.search_pdf_text("comptes mules", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, r.path);
        let passages = ws.ask_passages("comptes mules blanchir", &s, 5).unwrap();
        assert!(passages.iter().any(|x| x.text.contains("blanchir")), "{passages:?}");
        // Adding the same PDF a second time with a DOI: new source, no duplicate PDF.
        let w = online::Work { title: "Mobile money fraud".into(), doi: "10.1/mm".into(), year: Some(2021), authors: vec![("Awa".into(), "Koné".into())], ..Default::default() };
        let (r2, p2) = ws.add_pdf(&pdf, Some(&w), &s).unwrap();
        assert_ne!(p2, p);
        assert_eq!(ws.add_pdf(&pdf, Some(&w), &s).unwrap(), (FicheResult { path: r2.path.clone(), created: false }, p2.clone()));
        // Complete: abstract and keywords added, nothing overwritten.
        assert_eq!(ws.sources_to_complete().unwrap(), vec![(r2.path.clone(), "10.1/mm".to_string())]);
        let found = online::Work { abstract_note: "We study fraud in mobile money services across West Africa.".into(), keywords: vec!["Mobile money".into(), "Fraud detection".into()], venue: "IEEE Access".into(), ..Default::default() };
        assert!(ws.complete_source(&r2.path, &found, 0).unwrap());
        let fm2 = markdown::parse_note(&ws.vault.read(&r2.path).unwrap()).frontmatter;
        assert_eq!(fm2["resume"], "We study fraud in mobile money services across West Africa.");
        assert_eq!(fm2["annee"], 2021);
        assert_eq!(fm2["tags"], serde_json::json!(["source", "mobile-money", "fraud-detection"]));
        assert!(!ws.complete_source(&r2.path, &found, 0).unwrap());
        assert!(ws.sources_to_complete().unwrap().is_empty());
        std::fs::remove_file(&pdf).ok();
    }

    #[test]
    fn bibtex_import_and_disk_usage() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let bib = "@article{Smith2024, author={Smith, Jane}, title={Graph fraud}, journal={IEEE Access}, year=2024, doi={10.1/x}}\n@book{kone21, author={Kon{\\'e}, Awa}, title={Mobile money}, year={2021}}\n";
        let r = ws.import_bibtex(bib, &s).unwrap();
        assert_eq!((r.created, r.existing, r.failed.len()), (2, 0, 0));
        assert!(ws.vault.exists("Sources/@Smith2024.md"));
        assert!(ws.vault.read("Sources/@kone21.md").unwrap().contains("Koné"));
        // Repeated import: nothing new.
        let r2 = ws.import_bibtex(bib, &s).unwrap();
        assert_eq!((r2.created, r2.existing), (0, 2));
        // Disk usage, LaTeX auxiliaries, trash.
        ws.vault.write("Thèse/main.tex", "x").unwrap();
        ws.vault.write("Thèse/main.aux", "aux").unwrap();
        let rep = ws.disk_report();
        assert_eq!(rep.aux, vec!["Thèse/main.aux"]);
        assert_eq!(ws.clean_latex_aux().unwrap(), 1);
        assert!(!ws.vault.exists("Thèse/main.aux") && ws.vault.exists("Thèse/main.tex"));
        assert!(ws.disk_report().trash_bytes > 0);
        assert!(ws.empty_trash().unwrap() > 0);
        assert_eq!(ws.disk_report().trash_bytes, 0);
    }

    #[test]
    fn advanced_latex_formulas_and_references() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        assert!(ws.formulas(&s).unwrap().is_empty());
        assert_eq!(ws.add_formula("F1", "F_1 = \\frac{2PR}{P+R}", &s).unwrap(), "Formules.md");
        ws.add_formula("Rappel", "R = \\frac{TP}{TP+FN}", &s).unwrap();
        let f = ws.formulas(&s).unwrap();
        assert_eq!(f.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), vec!["F1", "Rappel"]);
        ws.vault.write("Thèse/main.tex", "\\section{A}\\label{sec:a}\n\\include{ch1}\n").unwrap();
        ws.vault.write("Thèse/ch1.tex", "\\begin{figure}\\label{fig:archi}\\end{figure}\n\\begin{equation}\\label{eq:f1}\\end{equation}\n").unwrap();
        assert_eq!(ws.latex_labels("Thèse/main.tex").unwrap(), vec!["sec:a", "fig:archi", "eq:f1"]);
        // Without compilation, no SyncTeX.
        assert_eq!(ws.latex_sync_forward("Thèse/main.tex", 1).unwrap(), None);
    }

    #[test]
    fn from_vault_to_manuscript() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        ws.create_note("Sources", "@a2020", "---\ntype: source\ntitre: A\nauteurs: [Ana Diallo]\nannee: 2020\ncitekey: a2020\n---\n# A").unwrap();
        ws.create_note("Thèse", "Plan", "---\ntype: plan\ntitre_these: Fraude et mobile money\nauteur: Amédée DERA\n---\n# Plan").unwrap();
        ws.create_note("Thèse", "Ch10", "---\ntype: chapitre\nchapitre: ch10\n---\n# Conclusion\n\nFin.").unwrap();
        ws.create_note("Thèse", "Ch2", "---\ntype: chapitre\nchapitre: ch2\n---\n# État de l'art\n\nComme le montre [@a2020, p. 3].").unwrap();
        let main = ws.assemble_thesis(&s).unwrap();
        assert_eq!(main, "Thèse/Manuscrit/main.tex");
        let m = ws.vault.read(&main).unwrap();
        assert!(m.contains("\\title{Fraude et mobile money}") && m.contains("\\author{Amédée DERA}") && m.contains("\\include{ch1}\n\\include{ch2}"));
        // Natural sort: ch2 before ch10.
        assert!(ws.vault.read("Thèse/Manuscrit/ch1.tex").unwrap().contains("\\chapter{État de l'art}"));
        assert!(ws.vault.read("Thèse/Manuscrit/ch1.tex").unwrap().contains("\\autocite[p. 3]{a2020}"));
        assert!(ws.vault.read("Thèse/Manuscrit/references.bib").unwrap().contains("@article{a2020,"));
        let check = ws.submission_checklist(&main, Lang::Fr).unwrap();
        assert!(check[0].ok, "{check:?}");
        // A single note becomes a full article.
        let tex = ws.note_to_latex("Thèse/Ch2.md", &s).unwrap();
        assert_eq!(tex, "Thèse/LaTeX/Ch2.tex");
        let t = ws.vault.read(&tex).unwrap();
        assert!(t.contains("\\documentclass[11pt,a4paper]{article}") && t.contains("\\title{État de l'art}") && t.contains("\\printbibliography"));
        assert!(!t.contains("\\chapter{"));
    }

    #[test]
    fn concept_note() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        let r = ws.create_concept_note("Compte mule", "Compte qui fait transiter\ndes fonds.", Some("Sources/@a.md"), &s).unwrap();
        assert_eq!((r.path.as_str(), r.created), ("Concepts/Compte mule.md", true));
        let c = ws.vault.read(&r.path).unwrap();
        assert!(c.contains("> Compte qui fait transiter des fonds.") && c.contains("- [[@a]]") && c.contains("type: concept"));
        // Already there: returned as is.
        assert!(!ws.create_concept_note("compte mule", "", None, &s).unwrap().created);
        assert_eq!(s.ai_agents.len(), 2);
    }

    #[test]
    fn research_matrix_cases() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        ws.create_note("Fiches de lecture", "Fiche @b", "---\ntype: fiche\ncitekey: b2020\nstatut: lue\n---\n# Fiche B\n\n## Problématique\n\nQuestion B.\n\n## Méthodes\n\nSVM.").unwrap();
        ws.create_note("Fiches de lecture", "Fiche @a", "---\ntype: fiche\ncitekey: a2019\n---\n# Fiche A\n\n## Problématique\n\nQuestion A.").unwrap();
        ws.create_note("", "Autre", "# Pas une fiche\n\n## Problématique\n\nX").unwrap();
        let m = ws.research_matrix(Lang::Fr, None).unwrap();
        assert_eq!(m.rows.len(), 2);
        assert_eq!(m.rows[0].cells[0], "Question A.");
        assert_eq!((m.rows[1].cells[2].as_str(), m.rows[1].statut.as_str()), ("SVM.", "lue"));
        let only = ws.research_matrix(Lang::Fr, Some(&["B2020".to_string()])).unwrap();
        assert_eq!(only.rows.len(), 1);
        let p = ws.export_matrix("csv", Lang::Fr, None, "Matrices").unwrap();
        assert!(p.starts_with("Matrices/Matrice de recherche ") && p.ends_with(".csv"));
        assert!(ws.vault.read(&p).unwrap().contains("Question B."));
    }

    #[test]
    fn excerpts_for_questions() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        ws.create_note("Concepts", "Fraude", "# Fraude\n\nIntro.\n\nLa fraude transactionnelle exploite les comptes mules.").unwrap();
        ws.create_note("Sources", "@k2022", "---\ntype: source\ntitre: Mobile money\nauteurs: [Awa Konaté]\nannee: 2022\ncitekey: k2022\nresume: Fraude au mobile money.\n---\n# Mobile money").unwrap();
        ws.create_note("Modèles", "M", "fraude {{title}}").unwrap();
        ws.create_note("", "Cuisine", "# Recette").unwrap();
        let ps = ws.ask_passages("Que disent mes lectures sur la fraude transactionnelle ?", &s, 8).unwrap();
        let paths: Vec<&str> = ps.iter().map(|p| p.path.as_str()).collect();
        assert!(paths.contains(&"Concepts/Fraude.md") && paths.contains(&"Sources/@k2022.md"));
        assert!(!paths.iter().any(|p| p.starts_with("Modèles/") || p.contains("Cuisine")));
        let src = ps.iter().find(|p| p.path == "Sources/@k2022.md").unwrap();
        assert_eq!(src.citekey, "k2022");
        assert!(src.text.contains("Awa Konaté (2022)") && src.text.contains("Fraude au mobile money"));
        assert_eq!(ps.iter().map(|p| p.id).collect::<Vec<_>>(), (1..=ps.len()).collect::<Vec<_>>());
    }

    #[test]
    fn integrity_check() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let s = Settings::default();
        ws.create_note("", "Cible", "# Cible").unwrap();
        ws.create_note("", "A", "Voir [[Cible]] et [[Absente]].\n![[image.png]]\n![[Cible]]").unwrap();
        ws.create_note("Modèles", "Modèle", "[[{{title}}]] [[Rien]]").unwrap();
        ws.create_note("", "Cassée", "---\ntitre: [oups\n---\nTexte").unwrap();
        ws.create_note("Fiches de lecture", "Fiche @x", "---\ntype: fiche\nsource: \"[[@x]]\"\n---\n# F").unwrap();
        ws.create_note("", "B", "---\npdf: PDF/manquant.pdf\n---\n# B").unwrap();
        let r = ws.check_integrity(&s).unwrap();
        assert_eq!(r.broken_links, vec![Issue { path: "A.md".into(), line: 1, detail: "Absente".into() }]);
        let missing: Vec<&str> = r.missing_files.iter().map(|i| i.detail.as_str()).collect();
        assert!(missing.contains(&"image.png") && missing.contains(&"PDF/manquant.pdf"));
        assert_eq!(r.bad_frontmatter.len(), 1);
        assert_eq!(r.bad_frontmatter[0].path, "Cassée.md");
        assert_eq!(r.fiches_without_source[0].detail, "@x");
        // Once the image is added, the problem disappears.
        std::fs::write(ws.vault.resolve("image.png").unwrap(), b"png").unwrap();
        assert!(ws.check_integrity(&s).unwrap().missing_files.iter().all(|i| i.detail != "image.png"));
    }

    #[test]
    fn library_without_zotero() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let settings = Settings::default();
        let w = crate::online::Work {
            title: "Mobile Money Fraud".into(),
            authors: vec![("Awa".into(), "Konaté".into())],
            year: Some(2022),
            doi: "10.1000/mm.1".into(),
            venue: "Journal of Fintech".into(),
            item_type: "journalArticle".into(),
            ..Default::default()
        };
        let r = ws.add_reference(&w, &settings).unwrap();
        assert!(r.created);
        assert_eq!(r.path, "Sources/@konate2022mobile.md");
        let src = ws.vault.read(&r.path).unwrap();
        assert!(!src.contains("zotero:") && src.contains("doi: 10.1000/mm.1"));
        // Same DOI: no duplicate. Same key, other article: suffix.
        assert!(!ws.add_reference(&w, &settings).unwrap().created);
        let w2 = crate::online::Work { doi: "10.1000/mm.2".into(), ..w.clone() };
        assert_eq!(ws.add_reference(&w2, &settings).unwrap().path, "Sources/@konate2022mobilea.md");
        // The source reads like an item, gets reading notes and enters the bibliography.
        let det = ws.source_detail(&r.path).unwrap();
        assert_eq!((det.item.title.as_str(), det.item.year, det.item.creators[0].last_name.as_str()), ("Mobile Money Fraud", Some(2022), "Konaté"));
        let f = ws.create_fiche_from_source(&r.path, &settings).unwrap();
        assert_eq!(f.path, "Fiches de lecture/Fiche @konate2022mobile.md");
        assert_eq!(ws.fiches_of_source(&r.path).unwrap(), vec![f.path.clone()]);
        assert!(ws.fiches_of_source("Sources/@konate2022mobilea.md").unwrap().is_empty());
        let bib = ws.bibtex(false, None).unwrap();
        assert!(bib.contains("@article{konate2022mobile,") && bib.contains("@article{konate2022mobilea,"));
    }

    #[test]
    fn collection_import() {
        let d = tempfile::tempdir().unwrap();
        let zdir = d.path().join("zotero");
        std::fs::create_dir_all(&zdir).unwrap();
        crate::zotero::tests::fake_zotero(&zdir.join("zotero.sqlite"));
        let ws = Workspace::open(d.path().join("coffre")).unwrap();
        let settings = Settings { zotero_data_dir: zdir.to_string_lossy().to_string(), ..Default::default() };
        ws.zotero_sync(&settings, Source::Sqlite).unwrap();
        // "Ch2 État de l'art" (sub-collection) only holds the first item.
        assert_eq!(ws.import_preview(Some("COLLBBBB")).unwrap(), ImportPreview { total: 1, existing: 0, new: 1 });
        // "Thèse" includes its sub-collection: both items.
        assert_eq!(ws.import_preview(Some("COLLAAAA")).unwrap().total, 2);
        let r = ws.import_collection(&settings, Some("COLLBBBB"), true).unwrap();
        assert_eq!(r.created, 1);
        assert!(ws.vault.exists("Sources/Thèse/Ch2 État de l'art/@smith2024graph.md"));
        // Then the parent collection: only the missing item is added.
        assert_eq!(ws.import_collection(&settings, Some("COLLAAAA"), true).unwrap().created, 1);
        assert_eq!(ws.import_preview(None).unwrap().new, 0);
    }

    #[test]
    fn empty_vault_layout() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        let made = ws.init_if_blank(&Settings::default()).unwrap();
        assert!(made.contains(&"Bienvenue.md".to_string()));
        assert!(ws.index.note_count().unwrap() >= 5);
        assert!(ws.init_if_blank(&Settings::default()).unwrap().is_empty());
    }

    #[test]
    fn templates_and_daily_note() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        ws.create_note("Modèles", "Réunion", "# {{title}}\nLe {{date}}").unwrap();
        assert_eq!(ws.list_templates("Modèles"), vec!["Modèles/Réunion.md"]);
        let r = ws.render_template("Modèles/Réunion.md", "Point").unwrap();
        assert!(r.starts_with("# Point\nLe 20"));
        let j = ws.daily_note("Journal", "Modèles", Lang::Fr).unwrap();
        assert!(j.starts_with("Journal/20"));
        assert_eq!(ws.daily_note("Journal", "Modèles", Lang::Fr).unwrap(), j);
        // Without a Journal.md template: title as a long date, in the chosen language.
        let d = tempfile::tempdir().unwrap();
        let ws2 = Workspace::open(d.path()).unwrap();
        let j2 = ws2.daily_note("Journal", "Modèles", Lang::Fr).unwrap();
        let today = Lang::Fr.long_date(chrono::Local::now().date_naive());
        assert_eq!(ws2.vault.read(&j2).unwrap(), format!("# {today}\n\n"));
        let l = ws.create_from_link("Projet/Nouvelle", "x.md").unwrap();
        assert_eq!(l, "Projet/Nouvelle.md");
    }
}
