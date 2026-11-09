//! Tauri commands. Each command is a thin wrapper around `Workspace`.
//! Errors are returned to the interface as readable text.

use std::path::PathBuf;

use base64::Engine;
use eloria_core::export::ExportOptions;
use eloria_core::index::{Graph, GraphOptions, LinkRef, NoteMeta, OutLink, SearchHit, SyncStats};
use eloria_core::settings::Settings;
use eloria_core::vault::TreeNode;
use eloria_core::workspace::{FicheResult, NoteFile, RenameResult, VaultStats};
use eloria_core::zotero::{self, Annotation, Collection, ItemDetail, ItemSummary, MirrorStatus, Source};
use eloria_core::{history, Workspace};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::{watcher, AppState};

type R<T> = Result<T, String>;

fn with_ws<T>(state: &AppState, f: impl FnOnce(&Workspace) -> eloria_core::Result<T>) -> R<T> {
    let guard = state.ws.lock().map_err(|_| "verrou du coffre empoisonné".to_string())?;
    let ws = guard.as_ref().ok_or_else(|| eloria_core::Error::NoVault.to_string())?;
    f(ws).map_err(|e| e.to_string())
}

fn settings(state: &AppState) -> Settings {
    state.settings.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Runs a heavy task off the command thread.
async fn blocking<T: Send + 'static>(
    state: &AppState,
    f: impl FnOnce(&Workspace, &Settings) -> eloria_core::Result<T> + Send + 'static,
) -> R<T> {
    let ws = state.ws.clone();
    let s = settings(state);
    tauri::async_runtime::spawn_blocking(move || {
        let guard = ws.lock().map_err(|_| "verrou du coffre empoisonné".to_string())?;
        let ws = guard.as_ref().ok_or_else(|| eloria_core::Error::NoVault.to_string())?;
        f(ws, &s).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ----------------------------------------------------------------- settings

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> R<Settings> {
    Ok(settings(&state))
}

#[tauri::command]
pub async fn default_settings() -> R<Settings> {
    Ok(Settings::default())
}

#[tauri::command]
pub async fn save_settings(state: State<'_, AppState>, settings: Settings) -> R<()> {
    let mut s = state.settings.lock().map_err(|e| e.to_string())?;
    // Recent vaults are managed on the Rust side.
    let recent = s.recent_vaults.clone();
    let last = s.last_vault.clone();
    // Extension approvals only change through `extension_enable`.
    let grants = s.extension_grants.clone();
    *s = settings;
    s.recent_vaults = recent;
    s.last_vault = last;
    s.extension_grants = grants;
    s.save(&state.settings_path).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------- coffre

#[derive(Serialize)]
pub struct VaultInfo {
    pub root: String,
    pub name: String,
    /// Files and folders created on opening (layout of a new vault)
    #[serde(default)]
    pub created: usize,
}

#[tauri::command]
pub async fn open_vault(app: AppHandle, state: State<'_, AppState>, path: String) -> R<VaultInfo> {
    let p = PathBuf::from(&path);
    let ws = tauri::async_runtime::spawn_blocking(move || Workspace::open(p))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let root = ws.root().to_path_buf();
    // Empty folder (new vault): default layout, templates and welcome note.
    let created = ws.init_if_blank(&settings(&state)).map_err(|e| e.to_string())?;
    let _ = app.asset_protocol_scope().allow_directory(&root, true);
    *state.ws.lock().map_err(|e| e.to_string())? = Some(ws);
    let w = watcher::start(app.clone(), state.ws.clone(), root.clone()).ok();
    *state.watcher.lock().map_err(|e| e.to_string())? = w;
    {
        let mut s = state.settings.lock().map_err(|e| e.to_string())?;
        s.remember_vault(&root.to_string_lossy());
        let _ = s.save(&state.settings_path);
    }
    Ok(VaultInfo {
        name: root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        root: root.to_string_lossy().to_string(),
        created: created.len(),
    })
}

#[tauri::command]
pub async fn close_vault(state: State<'_, AppState>) -> R<()> {
    *state.watcher.lock().map_err(|e| e.to_string())? = None;
    *state.ws.lock().map_err(|e| e.to_string())? = None;
    let mut s = state.settings.lock().map_err(|e| e.to_string())?;
    s.last_vault = None;
    s.save(&state.settings_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn vault_info(state: State<'_, AppState>) -> R<Option<VaultInfo>> {
    let g = state.ws.lock().map_err(|e| e.to_string())?;
    Ok(g.as_ref().map(|ws| VaultInfo {
        root: ws.root().to_string_lossy().to_string(),
        name: ws.root().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        created: 0,
    }))
}

#[tauri::command]
pub async fn vault_tree(state: State<'_, AppState>) -> R<TreeNode> {
    with_ws(&state, |ws| ws.vault.tree())
}

#[tauri::command]
pub async fn list_folders(state: State<'_, AppState>) -> R<Vec<String>> {
    with_ws(&state, |ws| Ok(ws.vault.list_folders()))
}

#[tauri::command]
pub async fn all_notes(state: State<'_, AppState>) -> R<Vec<NoteMeta>> {
    with_ws(&state, |ws| ws.index.all_notes())
}

#[tauri::command]
pub async fn vault_stats(state: State<'_, AppState>) -> R<VaultStats> {
    with_ws(&state, |ws| ws.stats())
}

#[tauri::command]
pub async fn rebuild_index(state: State<'_, AppState>) -> R<SyncStats> {
    blocking(&state, |ws, _| ws.rebuild()).await
}

// -------------------------------------------------------------------- notes

#[tauri::command]
pub async fn read_note(state: State<'_, AppState>, path: String) -> R<NoteFile> {
    with_ws(&state, |ws| ws.read_note(&path))
}

#[tauri::command]
pub async fn save_note(state: State<'_, AppState>, path: String, content: String) -> R<i64> {
    let interval = settings(&state).history_interval;
    with_ws(&state, |ws| ws.save_note(&path, &content, interval))
}

#[tauri::command]
pub async fn create_note(state: State<'_, AppState>, folder: String, name: String, content: String) -> R<String> {
    with_ws(&state, |ws| ws.create_note(&folder, &name, &content))
}

#[tauri::command]
pub async fn create_from_link(state: State<'_, AppState>, target: String, from: String) -> R<String> {
    with_ws(&state, |ws| ws.create_from_link(&target, &from))
}

#[tauri::command]
pub async fn create_folder(state: State<'_, AppState>, path: String) -> R<()> {
    with_ws(&state, |ws| ws.create_folder(&path))
}

#[tauri::command]
pub async fn delete_path(state: State<'_, AppState>, path: String) -> R<()> {
    with_ws(&state, |ws| ws.delete(&path))
}

#[tauri::command]
pub async fn rename_path(state: State<'_, AppState>, from: String, to: String, update_links: bool) -> R<RenameResult> {
    with_ws(&state, |ws| ws.rename(&from, &to, update_links))
}

#[tauri::command]
pub async fn set_property(state: State<'_, AppState>, path: String, key: String, value: Value) -> R<String> {
    let interval = settings(&state).history_interval;
    with_ws(&state, |ws| ws.set_property(&path, &key, value, interval))
}

#[tauri::command]
pub async fn save_attachment(state: State<'_, AppState>, folder: String, name: String, data: String) -> R<String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(data).map_err(|e| e.to_string())?;
    with_ws(&state, |ws| ws.save_attachment(&folder, &name, &bytes))
}

// --------------------------------------------------------- index and graph

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String, limit: Option<usize>) -> R<Vec<SearchHit>> {
    with_ws(&state, |ws| ws.index.search(&query, limit.unwrap_or(50)))
}

#[tauri::command]
pub async fn backlinks(state: State<'_, AppState>, path: String) -> R<Vec<LinkRef>> {
    with_ws(&state, |ws| ws.index.backlinks(&path))
}

#[tauri::command]
pub async fn unlinked_mentions(state: State<'_, AppState>, path: String) -> R<Vec<SearchHit>> {
    with_ws(&state, |ws| ws.index.unlinked_mentions(&path))
}

#[tauri::command]
pub async fn outgoing(state: State<'_, AppState>, path: String) -> R<Vec<OutLink>> {
    with_ws(&state, |ws| ws.index.outgoing(&path))
}

#[tauri::command]
pub async fn tags(state: State<'_, AppState>) -> R<Vec<(String, i64)>> {
    with_ws(&state, |ws| ws.index.tags())
}

/// Resolves a link target: note, then file (image, PDF).
#[tauri::command]
pub async fn resolve_link(state: State<'_, AppState>, target: String, from: Option<String>) -> R<Option<String>> {
    with_ws(&state, |ws| {
        let has_ext = target.rsplit('/').next().map(|n| n.contains('.') && !n.ends_with(".md")).unwrap_or(false);
        if !has_ext {
            if let Some(p) = ws.index.resolve(&target)? {
                return Ok(Some(p));
            }
        }
        Ok(ws.vault.find_file(&target, from.as_deref()))
    })
}

#[tauri::command]
pub async fn graph(state: State<'_, AppState>, options: GraphOptions) -> R<Graph> {
    with_ws(&state, |ws| ws.index.graph(&options))
}

#[tauri::command]
pub async fn local_graph(state: State<'_, AppState>, path: String, depth: usize, include_tags: bool) -> R<Graph> {
    with_ws(&state, |ws| ws.index.local_graph(&path, depth, include_tags))
}

// --------------------------------------------------------------- historique

#[tauri::command]
pub async fn history_list(state: State<'_, AppState>, path: String) -> R<Vec<history::Version>> {
    with_ws(&state, |ws| history::list(&ws.vault, &path))
}

#[tauri::command]
pub async fn history_read(state: State<'_, AppState>, path: String, id: String) -> R<String> {
    with_ws(&state, |ws| history::read(&ws.vault, &path, &id))
}

#[tauri::command]
pub async fn history_snapshot(state: State<'_, AppState>, path: String, content: String) -> R<Option<String>> {
    with_ws(&state, |ws| history::snapshot(&ws.vault, &path, &content, 0, true))
}

// ------------------------------------------------------------------ templates

#[tauri::command]
pub async fn templates_list(state: State<'_, AppState>) -> R<Vec<String>> {
    let s = settings(&state);
    with_ws(&state, |ws| Ok(ws.list_templates(&s.templates_folder)))
}

#[tauri::command]
pub async fn template_render(state: State<'_, AppState>, path: String, title: String) -> R<String> {
    with_ws(&state, |ws| ws.render_template(&path, &title))
}

#[tauri::command]
pub async fn daily_note(state: State<'_, AppState>) -> R<String> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.daily_note(&s.daily_folder, &s.templates_folder, s.lang()))
}

// ------------------------------------------------------------------- Zotero

#[derive(Serialize)]
pub struct ZoteroDetect {
    pub data_dir: Option<String>,
    pub api: bool,
}

#[tauri::command]
pub async fn zotero_detect(state: State<'_, AppState>) -> R<ZoteroDetect> {
    let s = settings(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let dir = if !s.zotero_data_dir.is_empty() && PathBuf::from(&s.zotero_data_dir).join("zotero.sqlite").exists() {
            Some(s.zotero_data_dir.clone())
        } else {
            zotero::default_data_dir().map(|p| p.to_string_lossy().to_string())
        };
        ZoteroDetect { data_dir: dir, api: zotero::api_available(s.zotero_port) }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn zotero_sync(state: State<'_, AppState>, source: Option<Source>) -> R<MirrorStatus> {
    let src = source.unwrap_or(Source::Auto);
    blocking(&state, move |ws, s| ws.zotero_sync(s, src)).await
}

#[tauri::command]
pub async fn zotero_status(state: State<'_, AppState>) -> R<MirrorStatus> {
    with_ws(&state, |ws| ws.zotero_status())
}

#[tauri::command]
pub async fn zotero_collections(state: State<'_, AppState>) -> R<Vec<Collection>> {
    with_ws(&state, |ws| ws.zotero_collections())
}

#[tauri::command]
pub async fn zotero_items(
    state: State<'_, AppState>,
    collection: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
) -> R<Vec<ItemSummary>> {
    with_ws(&state, |ws| ws.zotero_items(collection.as_deref(), query.as_deref(), limit.unwrap_or(500)))
}

#[tauri::command]
pub async fn zotero_item(state: State<'_, AppState>, key: String) -> R<ItemDetail> {
    with_ws(&state, |ws| ws.zotero_item(&key))
}

#[tauri::command]
pub async fn zotero_item_by_citekey(state: State<'_, AppState>, citekey: String) -> R<Option<ItemDetail>> {
    with_ws(&state, |ws| ws.zotero_item_by_citekey(&citekey))
}

#[tauri::command]
pub async fn zotero_search_annotations(state: State<'_, AppState>, query: String) -> R<Vec<(Annotation, String)>> {
    with_ws(&state, |ws| ws.zotero_search_annotations(&query))
}

#[tauri::command]
pub async fn zotero_fiches_map(state: State<'_, AppState>) -> R<std::collections::HashMap<String, String>> {
    with_ws(&state, |ws| ws.fiches_map())
}

#[tauri::command]
pub async fn create_fiche(state: State<'_, AppState>, key: String) -> R<FicheResult> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.create_fiche(&key, &s))
}

/// Creates the source note of an item (if it has none) and returns its path.
#[tauri::command]
pub async fn import_source(state: State<'_, AppState>, key: String) -> R<FicheResult> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.import_source(&key, &s))
}

/// Reading notes attached to an item.
#[tauri::command]
pub async fn fiches_of(state: State<'_, AppState>, key: String) -> R<Vec<String>> {
    with_ws(&state, |ws| ws.fiches_of(&key))
}

#[tauri::command]
pub async fn legacy_imports(state: State<'_, AppState>) -> R<Vec<String>> {
    with_ws(&state, |ws| ws.legacy_imports())
}

#[tauri::command]
pub async fn clean_legacy_imports(state: State<'_, AppState>) -> R<usize> {
    with_ws(&state, |ws| ws.clean_legacy_imports())
}

#[tauri::command]
pub async fn sync_fiche(state: State<'_, AppState>, path: String) -> R<String> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.sync_fiche(&path, s.history_interval, s.lang()))
}

/// How many items an import would add (the others already have their note).
#[tauri::command]
pub async fn zotero_import_preview(state: State<'_, AppState>, collection: Option<String>) -> R<eloria_core::workspace::ImportPreview> {
    with_ws(&state, |ws| ws.import_preview(collection.as_deref()))
}

/// Imports the whole Zotero library as Markdown notes (one per item).
#[tauri::command]
pub async fn zotero_import_all(
    state: State<'_, AppState>,
    by_collection: bool,
    collection: Option<String>,
) -> R<eloria_core::workspace::ImportReport> {
    blocking(&state, move |ws, s| ws.import_collection(s, collection.as_deref(), by_collection)).await
}

/// Path of the PDF (or EPUB) of a Zotero item or attachment, allowed for display.
#[tauri::command]
pub async fn zotero_pdf_path(app: AppHandle, state: State<'_, AppState>, key: String) -> R<Option<String>> {
    let s = settings(&state);
    let p = with_ws(&state, |ws| ws.zotero_pdf(&key, &s))?;
    if let Some(p) = &p {
        // The built-in reader reads the file through the asset protocol: only this file is allowed.
        app.asset_protocol_scope().allow_file(p).map_err(|e| e.to_string())?;
    }
    Ok(p.map(|p| p.to_string_lossy().to_string()))
}

/// Writes a .bib file. `output` is absolute; `only_cited` limits to keys cited in the vault.
#[tauri::command]
pub async fn export_bibtex(state: State<'_, AppState>, output: String, only_cited: bool) -> R<usize> {
    with_ws(&state, |ws| {
        let b = ws.bibtex(only_cited, None)?;
        std::fs::write(&output, &b)?;
        Ok(b.matches("\n@").count() + usize::from(b.starts_with('@')))
    })
}

// ------------------------------------------------------------------- themes

#[tauri::command]
pub async fn themes_custom(state: State<'_, AppState>) -> R<Vec<eloria_core::themes::CustomTheme>> {
    with_ws(&state, |ws| eloria_core::themes::list(&ws.vault))
}

#[tauri::command]
pub async fn theme_create_example(state: State<'_, AppState>) -> R<String> {
    with_ws(&state, |ws| eloria_core::themes::create_example(&ws.vault))
}

// --------------------------------------------------------------------- IA

use eloria_core::ai::{self, AiConfig, AiRequest, AiResponse, Part, Provider};
use eloria_core::fiche_ia::AiFicheDraft;
use std::sync::atomic::Ordering;

/// Is an API key saved for this provider? (the key itself never leaves)
#[tauri::command]
pub async fn ai_key_present(provider: String) -> R<bool> {
    ai::get_api_key(Provider::from_code(&provider)).map(|k| k.is_some()).map_err(|e| e.to_string())
}

/// Saves the key in the system keychain (empty = clear).
#[tauri::command]
pub async fn ai_set_key(provider: String, key: String) -> R<()> {
    ai::set_api_key(Provider::from_code(&provider), &key).map_err(|e| e.to_string())
}

/// Small check request (a few tokens).
#[tauri::command]
pub async fn ai_test(state: State<'_, AppState>) -> R<AiResponse> {
    let s = settings(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = AiConfig::from_settings(&s);
        let key = ai::get_api_key(cfg.provider)?;
        let lang = s.lang();
        let req = AiRequest {
            system: lang.pick("Réponds en une courte phrase.", "Answer in one short sentence.").into(),
            parts: vec![Part::Text(lang.pick("Dis bonjour à l'utilisateur d'Eloria.", "Say hello to the Eloria user.").into())],
            max_tokens: 2000,
        };
        ai::complete(&cfg, key.as_deref(), &req, &mut |_| {}, &std::sync::atomic::AtomicBool::new(false))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ai_cancel(state: State<'_, AppState>) -> R<()> {
    state.ai_cancel.store(true, Ordering::Relaxed);
    Ok(())
}

/// Writes the "Eloria method" analysis of an item. The text streams in
/// through `on_delta`; the vault is only held during preparation, not during the call.
#[tauri::command]
pub async fn ai_fiche_generate(state: State<'_, AppState>, key: String, on_delta: tauri::ipc::Channel<String>) -> R<AiFicheDraft> {
    let s = settings(&state);
    let job = with_ws(&state, |ws| ws.ai_fiche_job(&key, &s))?;
    state.ai_cancel.store(false, Ordering::Relaxed);
    let cancel = state.ai_cancel.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let api_key = ai::get_api_key(job.config.provider)?;
        job.run(api_key.as_deref(), &mut |d| {
            let _ = on_delta.send(d.to_string());
        }, &cancel)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// Saves a reviewed analysis into the reading note (created if needed).
#[tauri::command]
pub async fn ai_fiche_save(state: State<'_, AppState>, draft: AiFicheDraft, path: Option<String>) -> R<String> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.ai_fiche_save(&draft, path.as_deref().unwrap_or(""), &s))
}

/// Latest AI requests (date, model, tokens; never the content).
#[tauri::command]
pub async fn ai_journal(state: State<'_, AppState>) -> R<Vec<ai::LogEntry>> {
    with_ws(&state, |ws| ai::read_log(&ws.vault, 100))
}

// ------------------------------------------------------------------- export

#[tauri::command]
pub async fn export_note(state: State<'_, AppState>, path: String, options: ExportOptions) -> R<String> {
    blocking(&state, move |ws, s| ws.export_note(&path, &options, s)).await
}

#[tauri::command]
pub async fn pandoc_version(state: State<'_, AppState>) -> R<Option<String>> {
    let p = settings(&state).pandoc_path;
    tauri::async_runtime::spawn_blocking(move || eloria_core::export::pandoc_version(&p))
        .await
        .map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ system

/// Opens a URL (https, zotero://, mailto:) with the default application.
#[tauri::command]
pub async fn open_url(app: AppHandle, url: String) -> R<()> {
    let allowed = ["http://", "https://", "zotero://", "mailto:"];
    if !allowed.iter().any(|p| url.starts_with(p)) {
        return Err(format!("schéma non autorisé : {url}"));
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// Shows a vault file in the file explorer, or opens it with the default application.
#[tauri::command]
pub async fn reveal_path(app: AppHandle, state: State<'_, AppState>, path: String, open: Option<bool>) -> R<()> {
    let abs = with_ws(&state, |ws| ws.vault.resolve(&path))?;
    if open.unwrap_or(false) {
        app.opener().open_path(abs.to_string_lossy().to_string(), None::<&str>).map_err(|e| e.to_string())
    } else {
        app.opener().reveal_item_in_dir(abs).map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------- Online library

use eloria_core::online::{self, Work};

async fn blocking_call<T: Send + 'static>(f: impl FnOnce() -> eloria_core::Result<T> + Send + 'static) -> R<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())
}

/// Complete metadata of a DOI (Crossref, OpenAlex, Semantic Scholar, DataCite…).
#[tauri::command]
pub async fn lookup_doi(doi: String) -> R<Option<Work>> {
    blocking_call(move || online::lookup_doi(&doi)).await
}

/// Search for articles by title, authors or words (Crossref). A DOI is recognised.
#[tauri::command]
pub async fn search_works(query: String) -> R<Vec<Work>> {
    blocking_call(move || match online::normalize_doi(&query) {
        Some(doi) => Ok(online::lookup_doi(&doi)?.into_iter().collect()),
        None => online::crossref_search(&query, 12),
    })
    .await
}

/// Source completed online (abstract, keywords, journal, year), without overwriting anything.
#[tauri::command]
pub async fn complete_source(state: State<'_, AppState>, path: String) -> R<bool> {
    let s = settings(&state);
    let doi = with_ws(&state, |ws| ws.source_detail(&path))?.item.doi;
    let Some(doi) = online::normalize_doi(&doi) else {
        return Err(s.lang().pick("cette source n'a pas de DOI", "this source has no DOI").into());
    };
    let Some(w) = blocking_call(move || online::lookup_doi(&doi)).await? else { return Ok(false) };
    with_ws(&state, |ws| ws.complete_source(&path, &w, s.history_interval))
}

/// Every source to complete (DOI without abstract or keywords); progress (done, total).
#[tauri::command]
pub async fn complete_sources(state: State<'_, AppState>, on_progress: tauri::ipc::Channel<(usize, usize)>) -> R<usize> {
    let h = settings(&state).history_interval;
    let list = with_ws(&state, |ws| ws.sources_to_complete())?;
    let total = list.len();
    let mut done = 0;
    for (i, (path, doi)) in list.into_iter().enumerate() {
        let _ = on_progress.send((i, total));
        if let Ok(Some(w)) = blocking_call(move || online::lookup_doi(&doi)).await {
            if with_ws(&state, |ws| ws.complete_source(&path, &w, h)).unwrap_or(false) {
                done += 1;
            }
        }
    }
    let _ = on_progress.send((total, total));
    Ok(done)
}

/// PDF added without Zotero.
#[derive(Serialize)]
pub struct PdfAdded {
    pub path: String,
    pub created: bool,
    /// PDF in the vault
    pub pdf: String,
    /// Recognised DOI (empty: title taken from the file name)
    pub doi: String,
}

/// Adds a PDF from disk: DOI searched in the file, metadata online, copy
/// into the vault. Without a recognised DOI, the file name is used as the title.
#[tauri::command]
pub async fn add_pdf(state: State<'_, AppState>, path: String) -> R<PdfAdded> {
    let p = path.clone();
    let work = blocking_call(move || {
        let bytes = std::fs::read(&p)?;
        for d in eloria_core::pdfimport::find_dois(&bytes) {
            if let Ok(Some(w)) = online::lookup_doi(&d) {
                return Ok(Some(w));
            }
        }
        Ok(None)
    })
    .await?;
    let s = settings(&state);
    let (r, pdf) = with_ws(&state, |ws| ws.add_pdf(std::path::Path::new(&path), work.as_ref(), &s))?;
    Ok(PdfAdded { path: r.path, created: r.created, pdf, doi: work.map(|w| w.doi).unwrap_or_default() })
}

/// PDF dropped on the window: its raw bytes (`x-name` header: encoded name) are
/// put in a temporary folder; returns the path, to pass to `add_pdf`.
#[tauri::command]
pub fn stage_pdf(request: tauri::ipc::Request<'_>) -> R<String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else { return Err("PDF attendu".into()) };
    if !bytes.starts_with(b"%PDF") {
        return Err("ce fichier n'est pas un PDF".into());
    }
    let raw = request.headers().get("x-name").and_then(|v| v.to_str().ok()).unwrap_or("document.pdf");
    let name = percent_decode(raw);
    let mut name = eloria_core::vault::sanitize_file_name(name.trim_end_matches(".pdf").trim_end_matches(".PDF"));
    if name.is_empty() {
        name = "document".into();
    }
    let dir = std::env::temp_dir().join("eloria-pdf");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(format!("{name}.pdf"));
    std::fs::write(&dest, bytes).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().into_owned())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Text of a PDF extracted by the reader; returns the sources whose abstract was read from it.
#[tauri::command]
pub async fn store_pdf_text(state: State<'_, AppState>, path: String, text: String) -> R<Vec<String>> {
    let h = settings(&state).history_interval;
    with_ws(&state, |ws| ws.store_pdf_text(&path, &text, h))
}

#[tauri::command]
pub async fn has_pdf_text(state: State<'_, AppState>, path: String) -> R<bool> {
    with_ws(&state, |ws| Ok(ws.pdf_text(&path).is_some()))
}

#[tauri::command]
pub async fn pdfs_without_text(state: State<'_, AppState>) -> R<Vec<String>> {
    with_ws(&state, |ws| ws.pdfs_without_text())
}

/// Search in PDF text: (source, PDF, excerpt).
#[tauri::command]
pub async fn search_pdf_text(state: State<'_, AppState>, query: String) -> R<Vec<(String, String, String)>> {
    with_ws(&state, |ws| ws.search_pdf_text(&query, 30))
}

/// Similar articles (Semantic Scholar, then OpenAlex).
#[tauri::command]
pub async fn similar_works(doi: Option<String>, title: String) -> R<Vec<Work>> {
    blocking_call(move || online::similar(doi.as_deref().filter(|d| !d.is_empty()), &title, 12)).await
}

#[tauri::command]
pub async fn add_reference(state: State<'_, AppState>, work: Work) -> R<FicheResult> {
    // Result of a title search (Crossref only): abstract and keywords completed.
    let mut work = work;
    if work.abstract_note.trim().len() < 40 || work.keywords.is_empty() {
        work = blocking_call(move || {
            let mut w = work;
            online::enrich(&mut w);
            Ok(w)
        })
        .await?;
    }
    let s = settings(&state);
    with_ws(&state, |ws| ws.add_reference(&work, &s))
}

#[tauri::command]
pub async fn source_detail(state: State<'_, AppState>, path: String) -> R<ItemDetail> {
    with_ws(&state, |ws| ws.source_detail(&path))
}

#[tauri::command]
pub async fn fiches_of_source(state: State<'_, AppState>, path: String) -> R<Vec<String>> {
    with_ws(&state, |ws| ws.fiches_of_source(&path))
}

#[tauri::command]
pub async fn create_fiche_from_source(state: State<'_, AppState>, path: String) -> R<FicheResult> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.create_fiche_from_source(&path, &s))
}

/// Content of a `![[Note]]` or `![[Note#Section]]` embed.
#[tauri::command]
pub async fn embed(state: State<'_, AppState>, target: String) -> R<Option<eloria_core::workspace::Embed>> {
    with_ws(&state, |ws| ws.embed(&target))
}

// ---------------------------------------------------------------- Backup and integrity

use eloria_core::backup::{self, BackupInfo};
use eloria_core::workspace::IntegrityReport;

fn vault_root(state: &AppState) -> R<PathBuf> {
    let g = state.ws.lock().map_err(|e| e.to_string())?;
    g.as_ref().map(|w| w.root().to_path_buf()).ok_or_else(|| "Aucun coffre ouvert".to_string())
}

/// Backs up the vault into the configured folder (or `dir`).
#[tauri::command]
pub async fn backup_now(state: State<'_, AppState>, dir: Option<String>) -> R<BackupInfo> {
    let s = settings(&state);
    let dest = dir.filter(|d| !d.is_empty()).unwrap_or(s.backup_dir.clone());
    if dest.is_empty() {
        return Err("Choisis d'abord un dossier de sauvegarde (Réglages › Fichiers).".into());
    }
    let root = vault_root(&state)?;
    blocking_call(move || backup::create(&root, std::path::Path::new(&dest), s.backup_keep as usize)).await
}

#[tauri::command]
pub async fn backup_list(state: State<'_, AppState>) -> R<Vec<BackupInfo>> {
    let s = settings(&state);
    if s.backup_dir.is_empty() {
        return Ok(vec![]);
    }
    let root = vault_root(&state)?;
    let name = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    backup::list(std::path::Path::new(&s.backup_dir), &name).map_err(|e| e.to_string())
}

/// Restores an archive into an empty folder (the open vault is never touched).
#[tauri::command]
pub async fn backup_restore(archive: String, target: String) -> R<usize> {
    blocking_call(move || backup::restore(std::path::Path::new(&archive), std::path::Path::new(&target))).await
}

#[tauri::command]
pub async fn check_integrity(state: State<'_, AppState>) -> R<IntegrityReport> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.check_integrity(&s))
}

// ---------------------------------------------------------------- Version check

#[derive(Serialize)]
pub struct BuildInfo {
    pub commit: &'static str,
    pub built: &'static str,
    pub commands: Vec<&'static str>,
}

/// Fingerprint of the binary (commit, build date) and exposed commands: the interface
/// checks at startup that it is not running on a core older than itself.
#[tauri::command]
pub fn build_info() -> BuildInfo {
    BuildInfo { commit: env!("ELORIA_COMMIT"), built: env!("ELORIA_BUILT"), commands: env!("ELORIA_COMMANDS").split(',').collect() }
}

// ---------------------------------------------------------------- Assistants IA

use eloria_core::assist::{self, Answer, Passage, Rewrite, Task};

/// AI request log (date, model, tokens; never the content).
fn log_ai(state: &AppState, task: &str, cfg: &AiConfig, r: &Result<(String, u64, u64), String>) {
    if let Ok(g) = state.ws.lock() {
        if let Some(ws) = g.as_ref() {
            let (model, i, o) = r.clone().unwrap_or((cfg.model.clone(), 0, 0));
            let _ = ai::log(&ws.vault, &ai::LogEntry {
                date: ai::log_date(),
                task: task.into(),
                provider: cfg.provider.code().into(),
                model,
                input_tokens: i,
                output_tokens: o,
                ok: r.is_ok(),
            });
        }
    }
}

fn ai_ready(s: &Settings) -> R<AiConfig> {
    if !s.ai_enabled {
        return Err("IA : désactivée. Active-la dans Réglages › Intelligence artificielle.".into());
    }
    Ok(AiConfig::from_settings(s))
}

/// Rewrites a passage (rephrase, fix, summarise…). Citations and links are
/// protected: an answer that changes them is rejected.
#[tauri::command]
pub async fn ai_rewrite(state: State<'_, AppState>, task: Task, text: String, on_delta: tauri::ipc::Channel<String>) -> R<Rewrite> {
    let s = settings(&state);
    let cfg = ai_ready(&s)?;
    state.ai_cancel.store(false, Ordering::Relaxed);
    let cancel = state.ai_cancel.clone();
    let lang = s.lang();
    let c2 = cfg.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let key = ai::get_api_key(c2.provider)?;
        assist::rewrite(&c2, key.as_deref(), task, &text, lang, &mut |d| {
            let _ = on_delta.send(d.to_string());
        }, &cancel)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string());
    log_ai(&state, task.id(), &cfg, &res.as_ref().map(|r| (r.model.clone(), r.input_tokens, r.output_tokens)).map_err(|e| e.clone()));
    res
}

/// "Résumé" callout ready to insert.
#[tauri::command]
pub fn ai_summary_callout(state: State<'_, AppState>, summary: String) -> String {
    assist::summary_callout(&summary, settings(&state).lang())
}

/// Ask Eloria: question asked to the vault and the library, sourced and checked answer.
#[tauri::command]
pub async fn ai_ask(state: State<'_, AppState>, question: String, mode: Option<String>, on_delta: tauri::ipc::Channel<String>) -> R<Answer> {
    let mode = mode.unwrap_or_default();
    let s = settings(&state);
    let cfg = ai_ready(&s)?;
    let (passages, library) = with_ws(&state, |ws| Ok((ws.ask_passages(&question, &s, 10)?, ws.library_citekeys()?)))?;
    state.ai_cancel.store(false, Ordering::Relaxed);
    let cancel = state.ai_cancel.clone();
    let lang = s.lang();
    let topic = s.research_topic.clone();
    let c2 = cfg.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let key = ai::get_api_key(c2.provider)?;
        assist::ask(&c2, key.as_deref(), &question, passages, &library, lang, &topic, &mode, &mut |d| {
            let _ = on_delta.send(d.to_string());
        }, &cancel)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string());
    log_ai(&state, "question", &cfg, &res.as_ref().map(|r| (r.model.clone(), r.input_tokens, r.output_tokens)).map_err(|e| e.clone()));
    res
}

/// Ask Eloria answer converted for a note: [S2] becomes [@key] or [[note]].
#[tauri::command]
pub fn ai_answer_markdown(text: String, passages: Vec<Passage>) -> String {
    assist::answer_to_markdown(&text, &passages)
}

// ---------------------------------------------------------------- Matrice de recherche

#[tauri::command]
pub async fn research_matrix(state: State<'_, AppState>, citekeys: Option<Vec<String>>) -> R<eloria_core::matrix::Matrix> {
    let lang = settings(&state).lang();
    with_ws(&state, |ws| ws.research_matrix(lang, citekeys.as_deref()))
}

/// Exports the matrix (csv, md, tex) into the vault's "Matrices" folder.
#[tauri::command]
pub async fn export_matrix(state: State<'_, AppState>, format: String, citekeys: Option<Vec<String>>) -> R<String> {
    let lang = settings(&state).lang();
    with_ws(&state, |ws| ws.export_matrix(&format, lang, citekeys.as_deref(), "Matrices"))
}

// ---------------------------------------------------------------- Literature review

use eloria_core::review::Review;

#[tauri::command]
pub async fn create_review(state: State<'_, AppState>, title: String, question: String) -> R<String> {
    let lang = settings(&state).lang();
    let folder = lang.pick("Revues", "Reviews");
    with_ws(&state, |ws| ws.create_review(folder, &title, &question, lang))
}

#[tauri::command]
pub async fn review(state: State<'_, AppState>, path: String) -> R<Review> {
    with_ws(&state, |ws| ws.review(&path))
}

#[tauri::command]
pub async fn review_set_article(state: State<'_, AppState>, path: String, citekey: String, statut: String) -> R<Review> {
    let h = settings(&state).history_interval;
    with_ws(&state, |ws| ws.review_set_article(&path, &citekey, &statut, h))
}

/// State-of-the-art draft (AI) from the included articles, cited and checked.
#[tauri::command]
pub async fn ai_review_draft(state: State<'_, AppState>, path: String, on_delta: tauri::ipc::Channel<String>) -> R<Answer> {
    let s = settings(&state);
    let cfg = ai_ready(&s)?;
    let (rv, passages, library) = with_ws(&state, |ws| Ok((ws.review(&path)?, ws.review_passages(&path)?, ws.library_citekeys()?)))?;
    state.ai_cancel.store(false, Ordering::Relaxed);
    let cancel = state.ai_cancel.clone();
    let lang = s.lang();
    let c2 = cfg.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let key = ai::get_api_key(c2.provider)?;
        let q = if rv.question.is_empty() { rv.title.clone() } else { rv.question.clone() };
        assist::draft_review(&c2, key.as_deref(), &q, passages, &library, lang, &mut |d| {
            let _ = on_delta.send(d.to_string());
        }, &cancel)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string());
    log_ai(&state, "etat-de-l-art", &cfg, &res.as_ref().map(|r| (r.model.clone(), r.input_tokens, r.output_tokens)).map_err(|e| e.clone()));
    res
}

// ---------------------------------------------------------------- LaTeX (local compilation)

use eloria_core::latex::{self, CompileResult, Engine as TexEngine};

#[tauri::command]
pub async fn latex_engine(state: State<'_, AppState>) -> R<Option<TexEngine>> {
    let s = settings(&state);
    blocking_call(move || Ok(latex::find_engine(&s.latex_path))).await
}

/// Downloads Tectonic into the user's folder (explicit action, button).
#[tauri::command]
pub async fn latex_install_tectonic() -> R<TexEngine> {
    blocking_call(latex::install_tectonic).await
}

#[tauri::command]
pub async fn latex_templates() -> R<Vec<(String, String)>> {
    Ok(latex::templates().into_iter().map(|(id, name, _)| (id.to_string(), name.to_string())).collect())
}

#[tauri::command]
pub async fn latex_new(state: State<'_, AppState>, folder: String, name: String, template: String) -> R<String> {
    with_ws(&state, |ws| ws.latex_new(&folder, &name, &template))
}

#[tauri::command]
pub async fn latex_compile(state: State<'_, AppState>, path: String) -> R<CompileResult> {
    let s = settings(&state);
    let ws = state.ws.clone();
    blocking_call(move || {
        let g = ws.lock().map_err(|e| eloria_core::Error::Export(e.to_string()))?;
        let w = g.as_ref().ok_or(eloria_core::Error::NoVault)?;
        w.latex_compile(&path, &s)
    })
    .await
}

#[tauri::command]
pub async fn read_text(state: State<'_, AppState>, path: String) -> R<String> {
    with_ws(&state, |ws| ws.read_text(&path))
}

#[tauri::command]
pub async fn write_text(state: State<'_, AppState>, path: String, content: String) -> R<()> {
    let h = settings(&state).history_interval;
    with_ws(&state, |ws| ws.write_text(&path, &content, h))
}

// ---------------------------------------------------------------- Semantic search

use eloria_core::semantic::{self, EmbedConfig};

#[derive(Serialize)]
pub struct SemanticStatus {
    pub configured: bool,
    pub provider: String,
    pub model: String,
    pub key_present: bool,
    /// Notes to (re)embed
    pub stale: usize,
}

#[derive(Serialize)]
pub struct SemanticHit {
    pub path: String,
    pub title: String,
    pub score: f32,
}

#[tauri::command]
pub async fn semantic_status(state: State<'_, AppState>) -> R<SemanticStatus> {
    let s = settings(&state);
    let Some(cfg) = EmbedConfig::from_settings(&s) else {
        return Ok(SemanticStatus { configured: false, provider: String::new(), model: String::new(), key_present: false, stale: 0 });
    };
    let stale = with_ws(&state, |ws| ws.semantic_stale(&cfg.tag(), &s))?.len();
    let key_present = !cfg.needs_key() || semantic::get_key(&cfg.provider).ok().flatten().is_some();
    Ok(SemanticStatus { configured: true, provider: cfg.provider, model: cfg.model, key_present, stale })
}

#[tauri::command]
pub async fn semantic_set_key(provider: String, key: String) -> R<()> {
    semantic::set_key(&provider, &key).map_err(|e| e.to_string())
}

/// Computes the vectors of new or modified notes, in batches; the progress
/// (number of notes processed) is streamed.
#[tauri::command]
pub async fn semantic_index(state: State<'_, AppState>, on_progress: tauri::ipc::Channel<usize>) -> R<usize> {
    let s = settings(&state);
    let cfg = EmbedConfig::from_settings(&s).ok_or("Recherche sémantique non réglée (Réglages › Intelligence artificielle).")?;
    let tag = cfg.tag();
    let stale = with_ws(&state, |ws| ws.semantic_stale(&tag, &s))?;
    let key = semantic::get_key(&cfg.provider).map_err(|e| e.to_string())?;
    let mut done = 0;
    for chunk in stale.chunks(32) {
        let texts: Vec<String> = with_ws(&state, |ws| chunk.iter().map(|(p, _)| ws.semantic_text(p)).collect())?;
        let (c2, k2) = (cfg.clone(), key.clone());
        let vecs = blocking_call(move || semantic::embed(&c2, k2.as_deref(), &texts, false)).await?;
        with_ws(&state, |ws| {
            for ((p, m), v) in chunk.iter().zip(&vecs) {
                ws.semantic_store(p, *m, &tag, v)?;
            }
            Ok(())
        })?;
        done += chunk.len();
        let _ = on_progress.send(done);
    }
    Ok(done)
}

#[tauri::command]
pub async fn semantic_search(state: State<'_, AppState>, query: String, limit: usize) -> R<Vec<SemanticHit>> {
    let s = settings(&state);
    let cfg = EmbedConfig::from_settings(&s).ok_or("Recherche sémantique non réglée (Réglages › Intelligence artificielle).")?;
    let key = semantic::get_key(&cfg.provider).map_err(|e| e.to_string())?;
    let c2 = cfg.clone();
    let q = blocking_call(move || semantic::embed(&c2, key.as_deref(), &[query], true)).await?;
    let hits = with_ws(&state, |ws| ws.semantic_nearest(&q[0], &cfg.tag(), limit))?;
    Ok(hits.into_iter().map(|(path, title, score)| SemanticHit { path, title, score }).collect())
}

// ---------------------------------------------------------------- PDF annotation

use eloria_core::pdf_annot::PdfAnnotation;

#[tauri::command]
pub async fn pdf_annotations(state: State<'_, AppState>, path: String) -> R<Vec<PdfAnnotation>> {
    with_ws(&state, |ws| ws.pdf_annotations(&path))
}

/// Saves the annotations of a vault PDF; returns the updated sources.
#[tauri::command]
pub async fn save_pdf_annotations(state: State<'_, AppState>, path: String, list: Vec<PdfAnnotation>) -> R<Vec<String>> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.save_pdf_annotations(&path, &list, &s))
}

// ---------------------------------------------------------------- Propositions de l'IA

use eloria_core::assist::ConceptSuggestion;

/// Important concepts of a note, suggested as concept notes (to approve).
#[tauri::command]
pub async fn ai_concepts(state: State<'_, AppState>, path: String) -> R<Vec<ConceptSuggestion>> {
    let s = settings(&state);
    let cfg = ai_ready(&s)?;
    let text = with_ws(&state, |ws| ws.note_plain_text(&path))?;
    state.ai_cancel.store(false, Ordering::Relaxed);
    let cancel = state.ai_cancel.clone();
    let lang = s.lang();
    let c2 = cfg.clone();
    let res = blocking_call(move || {
        let key = ai::get_api_key(c2.provider)?;
        assist::suggest_concepts(&c2, key.as_deref(), &text, lang, &cancel)
    })
    .await;
    log_ai(&state, "concepts", &cfg, &res.as_ref().map(|_| (cfg.model.clone(), 0, 0)).map_err(|e| e.clone()));
    res
}

/// Tags suggested for a note, from the existing taxonomy (to approve).
#[tauri::command]
pub async fn ai_tags(state: State<'_, AppState>, path: String) -> R<Vec<String>> {
    let s = settings(&state);
    let cfg = ai_ready(&s)?;
    let (text, existing) = with_ws(&state, |ws| Ok((ws.note_plain_text(&path)?, ws.index.tags()?.into_iter().map(|(t, _)| t).collect::<Vec<_>>())))?;
    state.ai_cancel.store(false, Ordering::Relaxed);
    let cancel = state.ai_cancel.clone();
    let lang = s.lang();
    let c2 = cfg.clone();
    let res = blocking_call(move || {
        let key = ai::get_api_key(c2.provider)?;
        assist::suggest_tags(&c2, key.as_deref(), &text, &existing, lang, &cancel)
    })
    .await;
    log_ai(&state, "tags", &cfg, &res.as_ref().map(|_| (cfg.model.clone(), 0, 0)).map_err(|e| e.clone()));
    res
}

#[tauri::command]
pub async fn create_concept_note(state: State<'_, AppState>, name: String, definition: String, source: Option<String>) -> R<FicheResult> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.create_concept_note(&name, &definition, source.as_deref(), &s))
}

// ---------------------------------------------------------------- Quality, reading chain, dashboard

use eloria_core::quality::{BibIssue, CitationCheck, StyleIssue};
use eloria_core::workspace::Dashboard;

/// After saving a reading note or chapter: reading statuses moved forward.
#[tauri::command]
pub async fn reading_chain(state: State<'_, AppState>, path: String) -> R<Vec<String>> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.reading_chain(&path, &s))
}

#[tauri::command]
pub async fn bib_audit(state: State<'_, AppState>) -> R<Vec<BibIssue>> {
    let lang = settings(&state).lang();
    with_ws(&state, |ws| ws.bib_audit(lang))
}

#[tauri::command]
pub async fn check_citations(state: State<'_, AppState>, path: String) -> R<Vec<CitationCheck>> {
    with_ws(&state, |ws| ws.check_citations(&path))
}

#[tauri::command]
pub async fn style_check(state: State<'_, AppState>, path: String) -> R<Vec<StyleIssue>> {
    let lang = settings(&state).lang();
    with_ws(&state, |ws| ws.style_check(&path, lang))
}

#[tauri::command]
pub async fn orphan_notes(state: State<'_, AppState>) -> R<Vec<String>> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.orphan_notes(&s))
}

#[tauri::command]
pub async fn duplicate_notes(state: State<'_, AppState>) -> R<Vec<(String, String, f32)>> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.duplicate_notes(&s))
}

#[tauri::command]
pub async fn dashboard(state: State<'_, AppState>) -> R<Dashboard> {
    with_ws(&state, |ws| ws.dashboard())
}

// ---------------------------------------------------------------- Capture, import, Git

use eloria_core::capture::ImportReport as MdImportReport;
use eloria_core::git::{self as vgit, GitStatus};

#[tauri::command]
pub async fn capture(state: State<'_, AppState>, text: String, url: Option<String>) -> R<String> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.capture(&text, url.as_deref(), &s))
}

#[tauri::command]
pub async fn inbox(state: State<'_, AppState>) -> R<Vec<(String, String, i64)>> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.inbox(&s))
}

/// Imports a Markdown folder (Obsidian, Notion, Logseq, exported Evernote).
#[tauri::command]
pub async fn import_markdown(state: State<'_, AppState>, dir: String) -> R<MdImportReport> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.import_markdown(std::path::Path::new(&dir), &s))
}

#[tauri::command]
pub async fn git_status(state: State<'_, AppState>) -> R<GitStatus> {
    let root = vault_root(&state)?;
    blocking_call(move || Ok(vgit::status(&root))).await
}

#[tauri::command]
pub async fn git_init(state: State<'_, AppState>) -> R<GitStatus> {
    let root = vault_root(&state)?;
    blocking_call(move || vgit::init(&root)).await
}

#[tauri::command]
pub async fn git_commit(state: State<'_, AppState>, message: String) -> R<GitStatus> {
    let root = vault_root(&state)?;
    blocking_call(move || vgit::commit(&root, &message)).await
}

#[tauri::command]
pub async fn git_sync(state: State<'_, AppState>) -> R<GitStatus> {
    let root = vault_root(&state)?;
    blocking_call(move || vgit::sync(&root)).await
}

// ---------------------------------------------------------------- Citations (OpenAlex)

use eloria_core::online::CitationInfo;
use eloria_core::workspace::CitationNetwork;

/// References and citations of a source; `library`: DOI -> key of the items already in the library.
#[derive(Serialize)]
pub struct SourceCitations {
    pub info: Option<CitationInfo>,
    pub library: std::collections::HashMap<String, String>,
}

#[tauri::command]
pub async fn source_citations(state: State<'_, AppState>, path: String) -> R<SourceCitations> {
    let (item, library) = with_ws(&state, |ws| {
        let it = ws.source_detail(&path)?.item;
        let lib = ws.library_items()?.into_iter().filter(|i| !i.doi.is_empty()).map(|i| (i.doi.to_lowercase(), i.citekey)).collect::<std::collections::HashMap<_, _>>();
        Ok((it, lib))
    })?;
    let (doi, title) = (item.doi.clone(), item.title.clone());
    let info = blocking_call(move || online::citations(Some(doi.as_str()).filter(|d| !d.is_empty()), &title, 30)).await?;
    if let Some(i) = &info {
        if !item.doi.is_empty() {
            with_ws(&state, |ws| ws.store_citations(&item.doi, &i.reference_dois))?;
        }
    }
    Ok(SourceCitations { info, library })
}

#[tauri::command]
pub async fn citation_network(state: State<'_, AppState>) -> R<CitationNetwork> {
    with_ws(&state, |ws| ws.citation_network())
}

/// Fetches the references of the items that have none yet (at most `limit`),
/// with progress; returns the updated network.
#[tauri::command]
pub async fn citation_fetch(state: State<'_, AppState>, limit: usize, on_progress: tauri::ipc::Channel<usize>) -> R<CitationNetwork> {
    let missing = with_ws(&state, |ws| ws.citation_network())?.missing;
    for (i, doi) in missing.into_iter().take(limit).enumerate() {
        let d = doi.clone();
        if let Ok(Some(refs)) = blocking_call(move || online::reference_dois(&d)).await {
            with_ws(&state, |ws| ws.store_citations(&doi, &refs))?;
        } else {
            // Unknown to OpenAlex: recorded without references so as not to ask again.
            with_ws(&state, |ws| ws.store_citations(&doi, &[]))?;
        }
        let _ = on_progress.send(i + 1);
    }
    with_ws(&state, |ws| ws.citation_network())
}

// ---------------------------------------------------------------- Serveur MCP

#[derive(Serialize)]
pub struct McpInfo {
    /// Path of eloria-mcp.exe (next to Eloria)
    pub exe: String,
    pub exists: bool,
    /// Block to paste into claude_desktop_config.json
    pub config: String,
}

/// Where to find the MCP server and how to declare it in Claude Desktop.
#[tauri::command]
pub async fn mcp_info(state: State<'_, AppState>) -> R<McpInfo> {
    let root = vault_root(&state)?;
    let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(if cfg!(windows) { "eloria-mcp.exe" } else { "eloria-mcp" }))).unwrap_or_default();
    let config = serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": { "eloria": { "command": exe.to_string_lossy(), "args": [root.to_string_lossy()] } }
    }))
    .unwrap_or_default();
    Ok(McpInfo { exists: exe.is_file(), exe: exe.to_string_lossy().to_string(), config })
}

/// Enables or disables the local API; a token is created on first activation.
/// Returns the "Send to Eloria" bookmarklet to drag to the browser's bookmarks bar.
#[tauri::command]
pub async fn local_api_enable(state: State<'_, AppState>, enabled: bool) -> R<Option<String>> {
    let mut s = state.settings.lock().map_err(|e| e.to_string())?;
    s.local_api = enabled;
    if enabled && s.local_api_token.is_empty() {
        s.local_api_token = eloria_core::localapi::new_token();
    }
    let _ = s.save(&state.settings_path);
    Ok(enabled.then(|| eloria_core::localapi::bookmarklet(s.local_api_port, &s.local_api_token)))
}

// ---------------------------------------------------------------- Extensions

#[tauri::command]
pub async fn extensions_list(state: State<'_, AppState>) -> R<Vec<eloria_core::extensions::ExtensionInfo>> {
    let grants = settings(&state).extension_grants;
    with_ws(&state, |ws| Ok(ws.extensions(&grants)))
}

/// Approves or revokes an extension for the open vault (on its current code).
#[tauri::command]
pub async fn extension_enable(state: State<'_, AppState>, id: String, on: bool) -> R<Vec<eloria_core::extensions::ExtensionInfo>> {
    let mut grants = settings(&state).extension_grants;
    with_ws(&state, |ws| ws.extension_grant(&mut grants, &id, on))?;
    {
        let mut s = state.settings.lock().map_err(|e| e.to_string())?;
        s.extension_grants = grants.clone();
        s.save(&state.settings_path).map_err(|e| e.to_string())?;
    }
    with_ws(&state, |ws| Ok(ws.extensions(&grants)))
}

#[tauri::command]
pub async fn extension_load(state: State<'_, AppState>, id: String) -> R<eloria_core::extensions::ExtensionCode> {
    let grants = settings(&state).extension_grants;
    with_ws(&state, |ws| ws.extension_load(&grants, &id))
}

#[tauri::command]
pub async fn extension_data_get(state: State<'_, AppState>, id: String) -> R<serde_json::Value> {
    with_ws(&state, |ws| ws.extension_data_get(&id))
}

#[tauri::command]
pub async fn extension_data_set(state: State<'_, AppState>, id: String, value: serde_json::Value) -> R<()> {
    with_ws(&state, |ws| ws.extension_data_set(&id, &value))
}

#[tauri::command]
pub async fn extension_create(state: State<'_, AppState>, name: String) -> R<String> {
    with_ws(&state, |ws| ws.extension_create(&name))
}

// ---------------------------------------------------------------- Routines

#[tauri::command]
pub async fn add_focus(state: State<'_, AppState>, minutes: u64) -> R<u64> {
    with_ws(&state, |ws| ws.add_focus(minutes))
}

#[tauri::command]
pub async fn schedule_review(state: State<'_, AppState>, path: String, days: i64) -> R<Option<String>> {
    let h = settings(&state).history_interval;
    with_ws(&state, |ws| ws.schedule_review(&path, days, h))
}

#[tauri::command]
pub async fn weekly_review(state: State<'_, AppState>) -> R<String> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.weekly_review(&s))
}

// ---------------------------------------------------------------- Manuscrit

#[tauri::command]
pub async fn note_to_latex(state: State<'_, AppState>, path: String) -> R<String> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.note_to_latex(&path, &s))
}

#[tauri::command]
pub async fn assemble_thesis(state: State<'_, AppState>) -> R<String> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.assemble_thesis(&s))
}

#[tauri::command]
pub async fn submission_checklist(state: State<'_, AppState>, path: String) -> R<Vec<eloria_core::manuscript::CheckItem>> {
    let lang = settings(&state).lang();
    with_ws(&state, |ws| ws.submission_checklist(&path, lang))
}

// ---------------------------------------------------------------- Source reliability

use eloria_core::reliability::{today, Level, Signal};
use eloria_core::workspace::ReliabilityRow;

/// Reliability signals of a document and date of the check.
#[derive(Serialize)]
pub struct ReliabilityView {
    pub level: Level,
    pub signals: Vec<Signal>,
    pub checked: String,
}

/// Last stored check of a DOI (nothing if never checked).
#[tauri::command]
pub async fn reliability_of(state: State<'_, AppState>, doi: String) -> R<Option<ReliabilityView>> {
    let lang = settings(&state).lang();
    with_ws(&state, |ws| {
        Ok(ws.reliability_signals(&doi, lang).map(|(r, signals)| ReliabilityView { level: eloria_core::reliability::worst(&signals), signals, checked: r.checked }))
    })
}

/// Checks a DOI online (OpenAlex) and stores the result.
#[tauri::command]
pub async fn check_reliability(state: State<'_, AppState>, doi: String) -> R<ReliabilityView> {
    let lang = settings(&state).lang();
    let d = doi.clone();
    let r = blocking_call(move || online::reliability(&d, &today())).await?;
    with_ws(&state, |ws| ws.store_reliability(&r))?;
    let signals = eloria_core::reliability::signals_now(&r, lang);
    Ok(ReliabilityView { level: eloria_core::reliability::worst(&signals), signals, checked: r.checked })
}

/// Checks the library documents never checked; progress (done, total).
#[tauri::command]
pub async fn check_reliability_all(state: State<'_, AppState>, on_progress: tauri::ipc::Channel<(usize, usize)>) -> R<usize> {
    let missing = with_ws(&state, |ws| ws.reliability_missing())?;
    let total = missing.len();
    let mut done = 0;
    for (i, doi) in missing.into_iter().enumerate() {
        let _ = on_progress.send((i, total));
        if let Ok(r) = blocking_call(move || online::reliability(&doi, &today())).await {
            with_ws(&state, |ws| ws.store_reliability(&r))?;
            done += 1;
        }
    }
    let _ = on_progress.send((total, total));
    Ok(done)
}

#[tauri::command]
pub async fn reliability_report(state: State<'_, AppState>) -> R<(Vec<ReliabilityRow>, usize)> {
    let lang = settings(&state).lang();
    with_ws(&state, |ws| Ok((ws.reliability_report(lang)?, ws.reliability_missing()?.len())))
}

// ---------------------------------------------------------------- "New document" flow

/// Zotero items without a source in the vault, most recent first.
#[tauri::command]
pub async fn new_zotero_documents(state: State<'_, AppState>, limit: usize) -> R<Vec<eloria_core::zotero::ItemSummary>> {
    with_ws(&state, |ws| {
        Ok(ws
            .new_zotero_documents(limit)?
            .into_iter()
            .map(|i| eloria_core::zotero::ItemSummary {
                authors: eloria_core::zotero::authors_short(&i.creators),
                key: i.key,
                item_type: i.item_type,
                title: i.title,
                year: i.year,
                citekey: i.citekey,
                publication: i.publication,
                annotation_count: 0,
            })
            .collect())
    })
}

#[tauri::command]
pub async fn propose_tags(state: State<'_, AppState>, path: String) -> R<Vec<String>> {
    with_ws(&state, |ws| ws.propose_tags(&path))
}

#[tauri::command]
pub async fn related_notes(state: State<'_, AppState>, path: String) -> R<Vec<(String, String)>> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.related_notes(&path, 6, &s))
}

#[tauri::command]
pub async fn add_tags(state: State<'_, AppState>, path: String, tags: Vec<String>) -> R<bool> {
    with_ws(&state, |ws| ws.add_tags(&path, &tags))
}

#[tauri::command]
pub async fn create_fiche_with_links(state: State<'_, AppState>, path: String, links: Vec<String>) -> R<FicheResult> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.create_fiche_with_links(&path, &links, &s))
}

// ---------------------------------------------------------------- Thesis workspace

#[tauri::command]
pub async fn thesis_overview(state: State<'_, AppState>) -> R<eloria_core::thesis::ThesisOverview> {
    with_ws(&state, |ws| ws.thesis_overview())
}

/// Research questions that cite or link to an item: (path, title, status).
#[tauri::command]
pub async fn questions_for(state: State<'_, AppState>, citekey: String) -> R<Vec<(String, String, String)>> {
    with_ws(&state, |ws| ws.questions_for(&citekey))
}

// ---------------------------------------------------------------- Methodologies

#[tauri::command]
pub async fn methods_overview(state: State<'_, AppState>) -> R<Vec<eloria_core::methods::MethodRow>> {
    with_ws(&state, |ws| ws.methods_overview())
}

#[tauri::command]
pub async fn create_method_note(state: State<'_, AppState>, name: String, aliases: Vec<String>) -> R<String> {
    let lang = settings(&state).lang();
    with_ws(&state, |ws| ws.create_method_note(&name, &aliases, lang))
}

// ---------------------------------------------------------------- Outline generator

/// Draft outline ("chapter", "tag" or "folder"); returns the created note.
#[tauri::command]
pub async fn generate_outline(state: State<'_, AppState>, scope: String, value: String) -> R<String> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.generate_outline(&scope, &value, &s))
}

// ---------------------------------------------------------------- Similarity

/// Passages repeated word for word from the library (local comparison).
#[tauri::command]
pub async fn similarity_check(state: State<'_, AppState>, path: String) -> R<Vec<eloria_core::similarity::SimilarityHit>> {
    with_ws(&state, |ws| ws.similarity_check(&path))
}

// ---------------------------------------------------------------- Argumentation

use eloria_core::argument::Argument;

#[tauri::command]
pub async fn argument_new(state: State<'_, AppState>, title: String) -> R<String> {
    let lang = settings(&state).lang();
    with_ws(&state, |ws| ws.argument_new(&title, lang))
}

/// Argument of a note and cited keys missing from the library.
#[tauri::command]
pub async fn argument_load(state: State<'_, AppState>, path: String) -> R<(Argument, Vec<String>)> {
    with_ws(&state, |ws| ws.argument_load(&path))
}

#[tauri::command]
pub async fn argument_save(state: State<'_, AppState>, path: String, argument: Argument) -> R<Vec<String>> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.argument_save(&path, &argument, s.lang(), s.history_interval))
}

/// Continuous text ("md") or LaTeX document ("tex"); returns the created path.
#[tauri::command]
pub async fn argument_export(state: State<'_, AppState>, path: String, format: String) -> R<String> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.argument_export(&path, &format, &s))
}

// ---------------------------------------------------------------- Statistiques de recherche

/// Citations, topic heat map and chapter coverage.
#[tauri::command]
pub async fn research_stats(state: State<'_, AppState>, year: i32, month: u32) -> R<eloria_core::stats::ResearchStats> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.research_stats((year, month), &s))
}

// ---------------------------------------------------------------- Concepts and knowledge

#[tauri::command]
pub async fn concepts_overview(state: State<'_, AppState>) -> R<Vec<eloria_core::knowledge::ConceptRow>> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.concepts_overview(&s))
}

#[tauri::command]
pub async fn concept_detail(state: State<'_, AppState>, path: String) -> R<eloria_core::knowledge::ConceptDetail> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.concept_detail(&path, &s))
}

/// Creates or updates the knowledge card of a concept; returns its path.
#[tauri::command]
pub async fn knowledge_card(state: State<'_, AppState>, path: String) -> R<String> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.knowledge_card(&path, &s))
}

// ---------------------------------------------------------------- Topic clustering

#[tauri::command]
pub async fn topics(state: State<'_, AppState>, k: usize) -> R<Vec<eloria_core::topics::Topic>> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.topics(k, &s))
}

// ---------------------------------------------------------------- Named entities

#[tauri::command]
pub async fn entities(state: State<'_, AppState>) -> R<Vec<eloria_core::entities::EntityRow>> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.entities(&s))
}

// ---------------------------------------------------------------- Taxonomie personnelle

#[tauri::command]
pub async fn taxonomy(state: State<'_, AppState>) -> R<eloria_core::workspace::TaxonomyOverview> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.taxonomy(&s))
}

#[tauri::command]
pub async fn taxonomy_proposals(state: State<'_, AppState>) -> R<Vec<(String, String, Vec<String>)>> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.taxonomy_proposals(&s))
}

#[tauri::command]
pub async fn taxonomy_create(state: State<'_, AppState>) -> R<String> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.taxonomy_create(&s))
}

// ---------------------------------------------------------------- Knowledge map

#[tauri::command]
pub async fn knowledge_map(state: State<'_, AppState>, with_sources: bool) -> R<eloria_core::workspace::KnowledgeMap> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.knowledge_map(with_sources, &s))
}

// ---------------------------------------------------------------- Scientific writing assistant

use eloria_core::science::{SciTask, SupportVerdict};

/// Table, equation or TikZ figure, formula explanation or fix, improvements.
#[tauri::command]
pub async fn ai_science(state: State<'_, AppState>, task: SciTask, text: String, on_delta: tauri::ipc::Channel<String>) -> R<Rewrite> {
    let s = settings(&state);
    let cfg = ai_ready(&s)?;
    state.ai_cancel.store(false, Ordering::Relaxed);
    let cancel = state.ai_cancel.clone();
    let lang = s.lang();
    let c2 = cfg.clone();
    let res = blocking_call(move || {
        let key = ai::get_api_key(c2.provider)?;
        eloria_core::science::run(&c2, key.as_deref(), task, &text, lang, &mut |d| {
            let _ = on_delta.send(d.to_string());
        }, &cancel)
    })
    .await;
    log_ai(&state, "ecriture-scientifique", &cfg, &res.as_ref().map(|r| (r.model.clone(), r.input_tokens, r.output_tokens)).map_err(|e| e.clone()));
    res
}

/// Is each citing sentence supported by the cited document? (at most 40 sentences)
#[tauri::command]
pub async fn ai_check_support(state: State<'_, AppState>, path: String) -> R<Vec<SupportVerdict>> {
    let s = settings(&state);
    let cfg = ai_ready(&s)?;
    let (sentences, evidence) = with_ws(&state, |ws| ws.support_job(&path, 40))?;
    state.ai_cancel.store(false, Ordering::Relaxed);
    let cancel = state.ai_cancel.clone();
    let lang = s.lang();
    let c2 = cfg.clone();
    let res = blocking_call(move || {
        let key = ai::get_api_key(c2.provider)?;
        eloria_core::science::check_support(&c2, key.as_deref(), &sentences, &evidence, lang, &cancel)
    })
    .await;
    log_ai(&state, "coherence-citations", &cfg, &res.as_ref().map(|_| (cfg.model.clone(), 0, 0)).map_err(|e| e.clone()));
    res
}

// ---------------------------------------------------------------- PDF figures

/// Figure captured in a PDF: added to the sources of this PDF; returns the modified notes.
#[tauri::command]
pub async fn attach_figure(state: State<'_, AppState>, pdf: String, image: String, page: u32, caption: String) -> R<Vec<String>> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.attach_figure(&pdf, &image, page, &caption, &s))
}

// ---------------------------------------------------------------- Advanced LaTeX

#[tauri::command]
pub async fn formulas(state: State<'_, AppState>) -> R<Vec<eloria_core::texplus::Formula>> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.formulas(&s))
}

#[tauri::command]
pub async fn add_formula(state: State<'_, AppState>, name: String, latex: String) -> R<String> {
    let s = settings(&state);
    with_ws(&state, |ws| ws.add_formula(&name, &latex, &s))
}

#[tauri::command]
pub async fn latex_labels(state: State<'_, AppState>, path: String) -> R<Vec<String>> {
    with_ws(&state, |ws| ws.latex_labels(&path))
}

#[tauri::command]
pub async fn latex_sync_forward(state: State<'_, AppState>, path: String, line: u32) -> R<Option<(u32, f64)>> {
    with_ws(&state, |ws| ws.latex_sync_forward(&path, line))
}

#[tauri::command]
pub async fn latex_sync_inverse(state: State<'_, AppState>, path: String, page: u32, y: f64) -> R<Option<(String, u32)>> {
    with_ws(&state, |ws| ws.latex_sync_inverse(&path, page, y))
}

/// Preview of a snippet compiled separately (TikZ, formula): returns the compilation result.
#[tauri::command]
pub async fn latex_preview(state: State<'_, AppState>, snippet: String) -> R<eloria_core::latex::CompileResult> {
    let s = settings(&state);
    let (engine_dir, path) = with_ws(&state, |ws| Ok((ws.vault.eloria_dir().join("tmp").join("apercu"), s.latex_path.clone())))?;
    let lang = s.lang();
    blocking_call(move || {
        let engine = eloria_core::latex::find_engine(&path).ok_or_else(|| eloria_core::Error::Export(lang.pick("aucun moteur LaTeX : installe Tectonic depuis l'éditeur LaTeX", "no LaTeX engine: install Tectonic from the LaTeX editor").into()))?;
        eloria_core::texplus::preview(&engine, &snippet, &engine_dir)
    })
    .await
}

// ---------------------------------------------------------------- Import / export, disk usage

/// Imports a BibTeX file (path on disk).
#[tauri::command]
pub async fn import_bibtex(state: State<'_, AppState>, path: String) -> R<eloria_core::workspace::BibImportReport> {
    let s = settings(&state);
    let text = std::fs::read(&path).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| e.to_string())?;
    with_ws(&state, |ws| ws.import_bibtex(&text, &s))
}

#[tauri::command]
pub async fn disk_report(state: State<'_, AppState>) -> R<eloria_core::disk::DiskReport> {
    let root = vault_root(&state)?;
    blocking_call(move || Ok(eloria_core::disk::scan(&root))).await
}

#[tauri::command]
pub async fn clean_latex_aux(state: State<'_, AppState>) -> R<usize> {
    with_ws(&state, |ws| ws.clean_latex_aux())
}

#[tauri::command]
pub async fn empty_trash(state: State<'_, AppState>) -> R<u64> {
    with_ws(&state, |ws| ws.empty_trash())
}

#[tauri::command]
pub async fn clear_temp(state: State<'_, AppState>) -> R<u64> {
    with_ws(&state, |ws| ws.clear_temp())
}
