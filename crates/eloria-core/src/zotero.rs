//! Local mirror of the Zotero library.
//!
//! Two possible sources:
//! 1. **SQLite** (default): `zotero.sqlite` (and `better-bibtex.sqlite`) is copied
//!    to a temporary folder, then read. Works whether Zotero is open or
//!    not, and provides PDF annotations. Zotero's database is never written to.
//! 2. **Local API** (`http://127.0.0.1:23119/api/`, Zotero 7+, option "Allow other
//!    applications on this computer to communicate with Zotero"): fallback when the database
//!    cannot be found.
//!
//! The result is stored in the `z_*` tables of the vault index, which makes it possible
//! to browse the library even when Zotero is closed.
//!
//! Current limit: only the personal library is read (not groups).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Creator {
    pub creator_type: String,
    pub first_name: String,
    pub last_name: String,
}

impl Creator {
    pub fn display(&self) -> String {
        if self.first_name.is_empty() {
            self.last_name.clone()
        } else {
            format!("{} {}", self.first_name, self.last_name)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Attachment {
    pub key: String,
    pub title: String,
    pub content_type: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Item {
    pub key: String,
    pub item_type: String,
    pub title: String,
    pub creators: Vec<Creator>,
    pub date: String,
    pub year: Option<i32>,
    pub publication: String,
    pub doi: String,
    pub url: String,
    pub abstract_note: String,
    pub citekey: String,
    pub tags: Vec<String>,
    pub collections: Vec<String>,
    pub attachments: Vec<Attachment>,
    /// Every Zotero field (field name -> value).
    pub fields: HashMap<String, String>,
    pub date_added: String,
    pub date_modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Collection {
    pub key: String,
    pub name: String,
    pub parent: Option<String>,
    pub item_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Annotation {
    pub key: String,
    pub item_key: String,
    pub attachment_key: String,
    /// highlight, underline, note, text, image, ink
    pub kind: String,
    pub text: String,
    pub comment: String,
    pub color: String,
    pub page_label: String,
    pub sort_index: String,
    /// Physical page in the file (0 = first), taken from the Zotero position.
    /// The displayed number (`page_label`) may differ: page 12 of the journal = 20th page of the PDF.
    #[serde(default)]
    pub page_index: Option<i64>,
    pub tags: Vec<String>,
    pub date_modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Library {
    pub items: Vec<Item>,
    pub collections: Vec<Collection>,
    pub annotations: Vec<Annotation>,
}

/// Summary of an item for lists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemSummary {
    pub key: String,
    pub item_type: String,
    pub title: String,
    pub authors: String,
    pub year: Option<i32>,
    pub citekey: String,
    pub publication: String,
    pub annotation_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDetail {
    pub item: Item,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MirrorStatus {
    pub items: i64,
    pub annotations: i64,
    pub collections: i64,
    pub last_sync: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Auto,
    Sqlite,
    Api,
}

// ---------------------------------------------------------------------------
// Utilitaires
// ---------------------------------------------------------------------------

/// Default Zotero data folder (`~/Zotero`).
pub fn default_data_dir() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let p = PathBuf::from(home).join("Zotero");
    p.join("zotero.sqlite").exists().then_some(p)
}

pub fn extract_year(date: &str) -> Option<i32> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\b(1[5-9]\d\d|2[01]\d\d)\b").unwrap());
    re.captures(date).and_then(|c| c[1].parse().ok())
}

fn ascii_slug(s: &str) -> String {
    s.nfd().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

/// Fallback citation key, Better BibTeX style: `nameYearWord`.
pub fn generate_citekey(item: &Item) -> String {
    const STOP: &[&str] = &[
        "a", "an", "the", "of", "on", "in", "for", "and", "to", "le", "la", "les", "un", "une", "des", "de", "du",
        "et", "pour", "sur", "dans", "l", "d",
    ];
    let author = item
        .creators
        .iter()
        .find(|c| c.creator_type == "author")
        .or_else(|| item.creators.first())
        .map(|c| ascii_slug(&c.last_name))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "anon".into());
    let year = item.year.map(|y| y.to_string()).unwrap_or_default();
    let word = item
        .title
        .split(|c: char| !c.is_alphanumeric())
        .map(ascii_slug)
        .find(|w| w.len() > 1 && !STOP.contains(&w.as_str()))
        .unwrap_or_default();
    format!("{author}{year}{word}")
}

fn citekey_from_extra(extra: &str) -> Option<String> {
    extra.lines().find_map(|l| {
        let l = l.trim();
        let lower = l.to_lowercase();
        ["citation key:", "bibtex:"].iter().find_map(|p| {
            lower.starts_with(p).then(|| l[p.len()..].trim().to_string()).filter(|s| !s.is_empty())
        })
    })
}

/// Makes keys unique (adds a, b, c... to generated duplicates).
fn ensure_unique_citekeys(items: &mut [Item], explicit: &HashSet<String>) {
    let mut used: HashSet<String> = HashSet::new();
    for it in items.iter_mut() {
        let base = it.citekey.clone();
        if explicit.contains(&it.key) {
            used.insert(base.to_lowercase());
            continue;
        }
        let mut k = base.clone();
        let mut suffix = b'a';
        while used.contains(&k.to_lowercase()) {
            k = format!("{base}{}", suffix as char);
            suffix = suffix.saturating_add(1);
        }
        used.insert(k.to_lowercase());
        it.citekey = k;
    }
}

pub fn authors_short(creators: &[Creator]) -> String {
    let authors: Vec<&Creator> = {
        let a: Vec<&Creator> = creators.iter().filter(|c| c.creator_type == "author").collect();
        if a.is_empty() { creators.iter().collect() } else { a }
    };
    match authors.len() {
        0 => String::new(),
        1 => authors[0].last_name.clone(),
        2 => format!("{} & {}", authors[0].last_name, authors[1].last_name),
        _ => format!("{} et al.", authors[0].last_name),
    }
}

pub fn select_link(item_key: &str) -> String {
    format!("zotero://select/library/items/{item_key}")
}

pub fn open_pdf_link(attachment_key: &str, page: Option<&str>, annotation: Option<&str>) -> String {
    let mut s = format!("zotero://open-pdf/library/items/{attachment_key}");
    let mut q = vec![];
    if let Some(p) = page.filter(|p| !p.is_empty()) {
        q.push(format!("page={}", p.replace(' ', "%20")));
    }
    if let Some(a) = annotation.filter(|a| !a.is_empty()) {
        q.push(format!("annotation={a}"));
    }
    if !q.is_empty() {
        s.push('?');
        s.push_str(&q.join("&"));
    }
    s
}

fn annotation_kind(t: i64) -> &'static str {
    match t {
        1 => "highlight",
        2 => "note",
        3 => "image",
        4 => "ink",
        5 => "underline",
        6 => "text",
        _ => "other",
    }
}

const NON_REGULAR: &[&str] = &["attachment", "note", "annotation"];

/// Physical page (0 = first) in the JSON position of a Zotero annotation.
fn page_index_of(position: &str) -> Option<i64> {
    serde_json::from_str::<Value>(position).ok()?.get("pageIndex")?.as_i64()
}

/// File of an attachment on disk: `storage:name.pdf` is stored in
/// `<Zotero data>/storage/<key>/name.pdf`; a linked file keeps its absolute path.
pub fn attachment_file(data_dir: &Path, att: &Attachment) -> Option<PathBuf> {
    let p = if let Some(name) = att.path.strip_prefix("storage:") {
        data_dir.join("storage").join(&att.key).join(name)
    } else if Path::new(&att.path).is_absolute() {
        PathBuf::from(&att.path)
    } else {
        // `attachments:…` (base folder of linked files): a Zotero-specific setting, unknown here.
        return None;
    };
    p.is_file().then_some(p)
}

/// Readable attachment (PDF or EPUB) with the key of its item: the first one of an
/// item, or the one with the given key (`key` = key of the item or of the attachment).
pub fn find_attachment(conn: &Connection, key: &str) -> Result<Option<(String, Attachment)>> {
    if let Some(it) = item(conn, key)? {
        return Ok(it.attachments.into_iter().find(|a| is_readable(&a.content_type)).map(|a| (it.key, a)));
    }
    for it in all_items(conn)? {
        let k = it.key.clone();
        if let Some(a) = it.attachments.into_iter().find(|a| a.key == key) {
            return Ok(Some((k, a)));
        }
    }
    Ok(None)
}

/// Full text indexed by Zotero for an attachment (`.zotero-ft-cache`), if it exists.
pub fn fulltext(data_dir: &Path, attachment_key: &str) -> Option<String> {
    std::fs::read_to_string(data_dir.join("storage").join(attachment_key).join(".zotero-ft-cache")).ok().filter(|t| !t.trim().is_empty())
}

/// First readable attachment of an item.
pub fn readable_attachment(item: &Item) -> Option<&Attachment> {
    item.attachments.iter().find(|a| is_readable(&a.content_type))
}

/// Attachment readable in Zotero's reader (hence annotatable).
fn is_readable(content_type: &str) -> bool {
    content_type.contains("pdf") || content_type.contains("epub")
}

/// Readable title taken from an attachment path (`storage:My article.pdf`).
fn file_title(path: &str) -> String {
    let name = path.strip_prefix("storage:").unwrap_or(path);
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name).to_string()
}

/// Finalises the derived fields of an item (year, known fields, key).
fn finalize_item(it: &mut Item) {
    let g = |k: &str| it.fields.get(k).cloned().unwrap_or_default();
    it.title = {
        let t = g("title");
        if t.is_empty() { g("caseName").to_string() } else { t }
    };
    it.date = g("date");
    it.year = extract_year(&it.date);
    it.publication = ["publicationTitle", "bookTitle", "proceedingsTitle", "websiteTitle", "university", "publisher"]
        .iter()
        .map(|k| g(k))
        .find(|v| !v.is_empty())
        .unwrap_or_default();
    it.doi = g("DOI");
    it.url = g("url");
    it.abstract_note = g("abstractNote");
}

// ---------------------------------------------------------------------------
// Lecture SQLite
// ---------------------------------------------------------------------------

fn table_exists(c: &Connection, name: &str) -> bool {
    c.query_row("SELECT 1 FROM sqlite_master WHERE type IN ('table','view') AND name=?1", [name], |_| Ok(()))
        .optional()
        .ok()
        .flatten()
        .is_some()
}

fn columns(c: &Connection, table: &str) -> Vec<String> {
    let Ok(mut st) = c.prepare(&format!("PRAGMA table_info({table})")) else { return vec![] };
    st.query_map([], |r| r.get::<_, String>(1)).map(|r| r.flatten().collect()).unwrap_or_default()
}

/// Copies an SQLite database (and its -wal/-shm files) into `dest_dir`, then opens it.
fn open_copy(src: &Path, dest_dir: &Path) -> Result<Connection> {
    std::fs::create_dir_all(dest_dir)?;
    let name = src.file_name().ok_or_else(|| Error::Zotero("nom de fichier invalide".into()))?;
    let dest = dest_dir.join(name);
    std::fs::copy(src, &dest)?;
    for suffix in ["-wal", "-journal"] {
        let side = PathBuf::from(format!("{}{suffix}", src.display()));
        let side_dest = PathBuf::from(format!("{}{suffix}", dest.display()));
        let _ = std::fs::remove_file(&side_dest);
        if side.exists() {
            let _ = std::fs::copy(&side, &side_dest);
        }
    }
    let conn = Connection::open_with_flags(&dest, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    Ok(conn)
}

/// Reads the library from the Zotero data folder.
pub fn read_sqlite(data_dir: &Path, scratch: &Path) -> Result<Library> {
    let db = data_dir.join("zotero.sqlite");
    if !db.exists() {
        return Err(Error::Zotero(format!("zotero.sqlite introuvable dans {}", data_dir.display())));
    }
    let conn = open_copy(&db, scratch)?;
    let bbt = {
        let p = data_dir.join("better-bibtex.sqlite");
        if p.exists() { open_copy(&p, scratch).ok() } else { None }
    };
    let lib = read_connection(&conn, bbt.as_ref());
    drop(conn);
    drop(bbt);
    let _ = std::fs::remove_dir_all(scratch);
    lib
}

/// Reads from an open connection (also used by the tests).
pub fn read_connection(conn: &Connection, bbt: Option<&Connection>) -> Result<Library> {
    let item_types = if table_exists(conn, "itemTypesCombined") { "itemTypesCombined" } else { "itemTypes" };
    let fields_t = if table_exists(conn, "fieldsCombined") { "fieldsCombined" } else { "fields" };
    let lib_id: i64 = conn
        .query_row("SELECT libraryID FROM libraries WHERE type='user' LIMIT 1", [], |r| r.get(0))
        .optional()?
        .unwrap_or(1);
    let deleted: HashSet<i64> = if table_exists(conn, "deletedItems") {
        let mut st = conn.prepare("SELECT itemID FROM deletedItems")?;
        let r = st.query_map([], |r| r.get(0))?;
        r.flatten().collect()
    } else {
        HashSet::new()
    };

    // Every item of the library: id -> (key, type, added, modified)
    let mut all: HashMap<i64, (String, String, String, String)> = HashMap::new();
    {
        let mut st = conn.prepare(&format!(
            "SELECT i.itemID, i.key, t.typeName, i.dateAdded, i.dateModified FROM items i
             JOIN {item_types} t ON t.itemTypeID=i.itemTypeID WHERE i.libraryID=?1"
        ))?;
        let rows = st.query_map([lib_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?))
        })?;
        for row in rows {
            let (id, key, t, a, m) = row?;
            if !deleted.contains(&id) {
                all.insert(id, (key, t, a, m));
            }
        }
    }

    // Champs
    let mut fields: HashMap<i64, HashMap<String, String>> = HashMap::new();
    {
        let mut st = conn.prepare(&format!(
            "SELECT d.itemID, f.fieldName, v.value FROM itemData d
             JOIN {fields_t} f ON f.fieldID=d.fieldID JOIN itemDataValues v ON v.valueID=d.valueID"
        ))?;
        let rows = st.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, rusqlite::types::Value>(2)?))
        })?;
        for row in rows {
            let (id, f, v) = row?;
            if all.contains_key(&id) {
                use rusqlite::types::Value as V;
                let v = match v {
                    V::Text(s) => s,
                    V::Integer(i) => i.to_string(),
                    V::Real(x) => x.to_string(),
                    _ => continue,
                };
                fields.entry(id).or_default().insert(f, v);
            }
        }
    }

    // Auteurs
    let mut creators: HashMap<i64, Vec<Creator>> = HashMap::new();
    {
        let mut st = conn.prepare(
            "SELECT ic.itemID, c.firstName, c.lastName, ct.creatorType FROM itemCreators ic
             JOIN creators c ON c.creatorID=ic.creatorID
             JOIN creatorTypes ct ON ct.creatorTypeID=ic.creatorTypeID
             ORDER BY ic.itemID, ic.orderIndex",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                r.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (id, first, last, t) = row?;
            creators.entry(id).or_default().push(Creator { creator_type: t, first_name: first, last_name: last });
        }
    }

    // Tags
    let mut tags: HashMap<i64, Vec<String>> = HashMap::new();
    {
        let mut st = conn.prepare("SELECT it.itemID, t.name FROM itemTags it JOIN tags t ON t.tagID=it.tagID")?;
        for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, t) = row?;
            tags.entry(id).or_default().push(t);
        }
    }

    // Collections
    let mut collections = Vec::new();
    let mut coll_key: HashMap<i64, String> = HashMap::new();
    {
        let mut st = conn.prepare(
            "SELECT collectionID, key, collectionName, parentCollectionID FROM collections WHERE libraryID=?1",
        )?;
        let rows = st.query_map([lib_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<i64>>(3)?))
        })?;
        let raw: Vec<_> = rows.collect::<std::result::Result<_, _>>()?;
        for (id, key, _, _) in &raw {
            coll_key.insert(*id, key.clone());
        }
        for (_, key, name, parent) in raw {
            collections.push(Collection { key, name, parent: parent.and_then(|p| coll_key.get(&p).cloned()), item_count: 0 });
        }
    }
    let mut item_colls: HashMap<i64, Vec<String>> = HashMap::new();
    {
        let mut st = conn.prepare("SELECT collectionID, itemID FROM collectionItems")?;
        for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
            let (c, i) = row?;
            if let Some(k) = coll_key.get(&c) {
                item_colls.entry(i).or_default().push(k.clone());
            }
        }
    }

    // Attachments
    let mut attachments: HashMap<i64, Vec<Attachment>> = HashMap::new();
    let mut standalone: HashSet<i64> = HashSet::new();
    {
        let mut st = conn.prepare("SELECT itemID, parentItemID, contentType, path FROM itemAttachments")?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            ))
        })?;
        for row in rows {
            let (id, parent, ct, path) = row?;
            let Some((key, ..)) = all.get(&id) else { continue };
            let title = fields.get(&id).and_then(|f| f.get("title").cloned()).unwrap_or_default();
            let att = Attachment { key: key.clone(), title, content_type: ct, path };
            match parent {
                Some(parent) => attachments.entry(parent).or_default().push(att),
                // Standalone PDF or EPUB: it becomes an item of its own,
                // which is its own attachment (its annotations are attached to it).
                None if is_readable(&att.content_type) => {
                    standalone.insert(id);
                    attachments.entry(id).or_default().push(att);
                }
                None => {}
            }
        }
    }

    // Better BibTeX keys
    let mut bbt_keys: HashMap<String, String> = HashMap::new();
    if let Some(b) = bbt {
        let cols = columns(b, "citationkey");
        let key_col = ["itemKey", "itemkey"].into_iter().find(|c| cols.iter().any(|x| x == c));
        let ck_col = ["citationKey", "citekey"].into_iter().find(|c| cols.iter().any(|x| x == c));
        if let (Some(kc), Some(cc)) = (key_col, ck_col) {
            if let Ok(mut st) = b.prepare(&format!("SELECT {kc}, {cc} FROM citationkey")) {
                if let Ok(rows) = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))) {
                    bbt_keys.extend(rows.flatten());
                }
            }
        }
    }

    // Documents
    let mut items = Vec::new();
    let mut explicit = HashSet::new();
    let mut key_of: HashMap<i64, String> = HashMap::new();
    for (id, (key, t, added, modified)) in &all {
        key_of.insert(*id, key.clone());
        if NON_REGULAR.contains(&t.as_str()) && !standalone.contains(id) {
            continue;
        }
        let mut it = Item {
            key: key.clone(),
            item_type: t.clone(),
            creators: creators.remove(id).unwrap_or_default(),
            tags: tags.get(id).cloned().unwrap_or_default(),
            collections: item_colls.remove(id).unwrap_or_default(),
            attachments: attachments.remove(id).unwrap_or_default(),
            fields: fields.remove(id).unwrap_or_default(),
            date_added: added.clone(),
            date_modified: modified.clone(),
            ..Default::default()
        };
        finalize_item(&mut it);
        if it.title.is_empty() {
            it.title = it.attachments.first().map(|a| file_title(&a.path)).unwrap_or_default();
        }
        let explicit_key = bbt_keys
            .get(key)
            .cloned()
            .or_else(|| it.fields.get("citationKey").cloned().filter(|s| !s.is_empty()))
            .or_else(|| it.fields.get("extra").and_then(|e| citekey_from_extra(e)));
        match explicit_key {
            Some(k) => {
                it.citekey = k;
                explicit.insert(it.key.clone());
            }
            None => it.citekey = generate_citekey(&it),
        }
        items.push(it);
    }
    items.sort_by(|a, b| a.date_added.cmp(&b.date_added).then(a.key.cmp(&b.key)));
    ensure_unique_citekeys(&mut items, &explicit);

    // Annotations
    let mut annotations = Vec::new();
    if table_exists(conn, "itemAnnotations") {
        let attachment_parent: HashMap<String, String> = items
            .iter()
            .flat_map(|it| it.attachments.iter().map(move |a| (a.key.clone(), it.key.clone())))
            .collect();
        // The `position` column (JSON with pageIndex) exists in Zotero 6 and 7.
        let position = if columns(conn, "itemAnnotations").iter().any(|c| c == "position") { "position" } else { "NULL" };
        let mut st = conn.prepare(&format!(
            "SELECT itemID, parentItemID, type, text, comment, color, pageLabel, sortIndex, {position} FROM itemAnnotations"
        ))?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                r.get::<_, Option<String>>(7)?.unwrap_or_default(),
                r.get::<_, Option<String>>(8)?,
            ))
        })?;
        for row in rows {
            let (id, parent, t, text, comment, color, page, sort, pos) = row?;
            let (Some((key, _, _, modified)), Some(att_key)) = (all.get(&id), key_of.get(&parent)) else { continue };
            let Some(item_key) = attachment_parent.get(att_key) else { continue };
            annotations.push(Annotation {
                key: key.clone(),
                item_key: item_key.clone(),
                attachment_key: att_key.clone(),
                kind: annotation_kind(t).into(),
                text,
                comment,
                color,
                page_label: page,
                sort_index: sort,
                page_index: pos.as_deref().and_then(page_index_of),
                tags: tags.get(&id).cloned().unwrap_or_default(),
                date_modified: modified.clone(),
            });
        }
    }
    annotations.sort_by(|a, b| a.attachment_key.cmp(&b.attachment_key).then(a.sort_index.cmp(&b.sort_index)));

    Ok(Library { items, collections, annotations })
}

// ---------------------------------------------------------------------------
// HTTP local API
// ---------------------------------------------------------------------------

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .proxy(None)
        .http_status_as_error(true)
        .build()
        .into()
}

fn api_get_all(agent: &ureq::Agent, base: &str, path: &str) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let limit = 100usize;
    loop {
        let sep = if path.contains('?') { '&' } else { '?' };
        let url = format!("{base}{path}{sep}limit={limit}&start={start}&format=json");
        let v: Value = agent
            .get(&url)
            .header("Zotero-API-Version", "3")
            .header("Zotero-Allowed-Request", "1")
            .call()
            .map_err(|e| Error::Zotero(format!("API locale injoignable ({e}). Zotero est-il ouvert, avec l'API locale activée ?")))?
            .body_mut()
            .with_config()
            .limit(256 * 1024 * 1024)
            .read_json()
            .map_err(|e| Error::Zotero(format!("réponse API invalide : {e}")))?;
        let arr = v.as_array().cloned().unwrap_or_default();
        let n = arr.len();
        out.extend(arr);
        if n < limit {
            break;
        }
        start += n;
        if start > 500_000 {
            break;
        }
    }
    Ok(out)
}

/// Checks that the local API answers.
pub fn api_available(port: u16) -> bool {
    agent()
        .get(&format!("http://127.0.0.1:{port}/api/users/0/items?limit=1"))
        .header("Zotero-API-Version", "3")
        .header("Zotero-Allowed-Request", "1")
        .call()
        .is_ok()
}

pub fn read_api(port: u16) -> Result<Library> {
    let a = agent();
    let base = format!("http://127.0.0.1:{port}/api/users/0");
    let raw_items = api_get_all(&a, &base, "/items")?;
    let raw_colls = api_get_all(&a, &base, "/collections")?;
    library_from_api_json(&raw_items, &raw_colls)
}

fn s(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::String(x)) => x.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// Builds the library from the JSON responses of the API (Web API v3 format).
pub fn library_from_api_json(raw_items: &[Value], raw_colls: &[Value]) -> Result<Library> {
    let mut items = Vec::new();
    let mut atts: Vec<(String, Attachment)> = Vec::new();
    let mut anns: Vec<(String, Annotation)> = Vec::new();
    let mut explicit = HashSet::new();
    for raw in raw_items {
        let d = raw.get("data").unwrap_or(raw);
        let t = s(d, "itemType");
        if d.get("deleted").and_then(|x| x.as_bool().or(x.as_i64().map(|n| n != 0))).unwrap_or(false) {
            continue;
        }
        let tags: Vec<String> = d
            .get("tags")
            .and_then(|x| x.as_array())
            .map(|a| a.iter().map(|t| s(t, "tag")).filter(|t| !t.is_empty()).collect())
            .unwrap_or_default();
        match t.as_str() {
            "attachment" => {
                let parent = s(d, "parentItem");
                let att = Attachment { key: s(d, "key"), title: s(d, "title"), content_type: s(d, "contentType"), path: s(d, "path") };
                if !parent.is_empty() {
                    atts.push((parent, att));
                } else if is_readable(&att.content_type) {
                    // Standalone PDF: an item of its own, its own attachment.
                    let mut it = Item {
                        key: att.key.clone(),
                        item_type: t.clone(),
                        title: if att.title.is_empty() { file_title(&att.path) } else { att.title.clone() },
                        tags,
                        date_added: s(d, "dateAdded"),
                        date_modified: s(d, "dateModified"),
                        ..Default::default()
                    };
                    it.citekey = generate_citekey(&it);
                    items.push(it);
                    atts.push((att.key.clone(), att));
                }
            }
            "annotation" => {
                anns.push((
                    s(d, "parentItem"),
                    Annotation {
                        key: s(d, "key"),
                        item_key: String::new(),
                        attachment_key: s(d, "parentItem"),
                        kind: s(d, "annotationType"),
                        text: s(d, "annotationText"),
                        comment: s(d, "annotationComment"),
                        color: s(d, "annotationColor"),
                        page_label: s(d, "annotationPageLabel"),
                        sort_index: s(d, "annotationSortIndex"),
                        page_index: page_index_of(&s(d, "annotationPosition")),
                        tags,
                        date_modified: s(d, "dateModified"),
                    },
                ));
            }
            "note" | "" => {}
            _ => {
                let mut fields = HashMap::new();
                if let Some(obj) = d.as_object() {
                    for (k, v) in obj {
                        if let Value::String(x) = v {
                            if !x.is_empty() && !["key", "itemType", "dateAdded", "dateModified", "parentItem"].contains(&k.as_str()) {
                                fields.insert(k.clone(), x.clone());
                            }
                        }
                    }
                }
                let creators = d
                    .get("creators")
                    .and_then(|x| x.as_array())
                    .map(|a| {
                        a.iter()
                            .map(|c| {
                                let name = s(c, "name");
                                Creator {
                                    creator_type: s(c, "creatorType"),
                                    first_name: if name.is_empty() { s(c, "firstName") } else { String::new() },
                                    last_name: if name.is_empty() { s(c, "lastName") } else { name },
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let mut it = Item {
                    key: s(d, "key"),
                    item_type: t.clone(),
                    creators,
                    tags,
                    collections: d
                        .get("collections")
                        .and_then(|x| x.as_array())
                        .map(|a| a.iter().filter_map(|c| c.as_str().map(str::to_string)).collect())
                        .unwrap_or_default(),
                    fields,
                    date_added: s(d, "dateAdded"),
                    date_modified: s(d, "dateModified"),
                    ..Default::default()
                };
                finalize_item(&mut it);
                let ck = it
                    .fields
                    .get("citationKey")
                    .cloned()
                    .filter(|s| !s.is_empty())
                    .or_else(|| it.fields.get("extra").and_then(|e| citekey_from_extra(e)));
                match ck {
                    Some(k) => {
                        it.citekey = k;
                        explicit.insert(it.key.clone());
                    }
                    None => it.citekey = generate_citekey(&it),
                }
                items.push(it);
            }
        }
    }
    let idx: HashMap<String, usize> = items.iter().enumerate().map(|(i, it)| (it.key.clone(), i)).collect();
    let mut att_parent = HashMap::new();
    for (parent, a) in atts {
        if let Some(&i) = idx.get(&parent) {
            att_parent.insert(a.key.clone(), items[i].key.clone());
            items[i].attachments.push(a);
        }
    }
    let mut annotations = Vec::new();
    for (att, mut a) in anns {
        if let Some(item_key) = att_parent.get(&att) {
            a.item_key = item_key.clone();
            annotations.push(a);
        }
    }
    annotations.sort_by(|a, b| a.attachment_key.cmp(&b.attachment_key).then(a.sort_index.cmp(&b.sort_index)));
    items.sort_by(|a, b| a.date_added.cmp(&b.date_added).then(a.key.cmp(&b.key)));
    ensure_unique_citekeys(&mut items, &explicit);
    let collections = raw_colls
        .iter()
        .map(|c| {
            let d = c.get("data").unwrap_or(c);
            let parent = match d.get("parentCollection") {
                Some(Value::String(p)) if !p.is_empty() => Some(p.clone()),
                _ => None,
            };
            Collection { key: s(d, "key"), name: s(d, "name"), parent, item_count: 0 }
        })
        .collect();
    Ok(Library { items, collections, annotations })
}

// ---------------------------------------------------------------------------
// Mirror (z_* tables of the index)
// ---------------------------------------------------------------------------

pub fn init_mirror(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS z_items(
            key TEXT PRIMARY KEY, item_type TEXT, title TEXT, authors TEXT, year INTEGER,
            citekey TEXT, publication TEXT, date_added TEXT, json TEXT NOT NULL);
         CREATE INDEX IF NOT EXISTS z_items_ck ON z_items(citekey COLLATE NOCASE);
         CREATE TABLE IF NOT EXISTS z_collections(key TEXT PRIMARY KEY, name TEXT, parent TEXT);
         CREATE TABLE IF NOT EXISTS z_item_collections(item_key TEXT, collection_key TEXT);
         CREATE INDEX IF NOT EXISTS z_ic_c ON z_item_collections(collection_key);
         CREATE TABLE IF NOT EXISTS z_annotations(
            key TEXT PRIMARY KEY, item_key TEXT, sort TEXT, text TEXT, comment TEXT, json TEXT NOT NULL);
         CREATE INDEX IF NOT EXISTS z_ann_item ON z_annotations(item_key);
         CREATE TABLE IF NOT EXISTS z_meta(k TEXT PRIMARY KEY, v TEXT);",
    )?;
    Ok(())
}

pub fn store_library(conn: &Connection, lib: &Library, source: &str) -> Result<()> {
    conn.execute_batch(
        "BEGIN; DELETE FROM z_items; DELETE FROM z_collections; DELETE FROM z_item_collections; DELETE FROM z_annotations;",
    )?;
    let res = (|| -> Result<()> {
        let mut ins = conn.prepare(
            "INSERT OR REPLACE INTO z_items(key,item_type,title,authors,year,citekey,publication,date_added,json)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        )?;
        let mut ins_ic = conn.prepare("INSERT INTO z_item_collections(item_key,collection_key) VALUES(?1,?2)")?;
        for it in &lib.items {
            let authors = it.creators.iter().map(|c| c.display()).collect::<Vec<_>>().join("; ");
            ins.execute(params![
                it.key,
                it.item_type,
                it.title,
                authors,
                it.year,
                it.citekey,
                it.publication,
                it.date_added,
                serde_json::to_string(it)?
            ])?;
            for c in &it.collections {
                ins_ic.execute(params![it.key, c])?;
            }
        }
        let mut ins = conn.prepare("INSERT OR REPLACE INTO z_collections(key,name,parent) VALUES(?1,?2,?3)")?;
        for c in &lib.collections {
            ins.execute(params![c.key, c.name, c.parent])?;
        }
        let mut ins =
            conn.prepare("INSERT OR REPLACE INTO z_annotations(key,item_key,sort,text,comment,json) VALUES(?1,?2,?3,?4,?5,?6)")?;
        for a in &lib.annotations {
            ins.execute(params![
                a.key,
                a.item_key,
                format!("{}|{}", a.attachment_key, a.sort_index),
                a.text,
                a.comment,
                serde_json::to_string(a)?
            ])?;
        }
        let now = chrono::Local::now().to_rfc3339();
        conn.execute("INSERT OR REPLACE INTO z_meta(k,v) VALUES('last_sync',?1)", [now])?;
        conn.execute("INSERT OR REPLACE INTO z_meta(k,v) VALUES('source',?1)", [source])?;
        Ok(())
    })();
    match res {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(e) => {
            conn.execute_batch("ROLLBACK")?;
            return Err(e);
        }
    }
    Ok(())
}

pub fn status(conn: &Connection) -> Result<MirrorStatus> {
    let count = |t: &str| -> Result<i64> { Ok(conn.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))?) };
    let meta = |k: &str| -> Result<Option<String>> {
        Ok(conn.query_row("SELECT v FROM z_meta WHERE k=?1", [k], |r| r.get(0)).optional()?)
    };
    Ok(MirrorStatus {
        items: count("z_items")?,
        annotations: count("z_annotations")?,
        collections: count("z_collections")?,
        last_sync: meta("last_sync")?,
        source: meta("source")?,
    })
}

pub fn collections(conn: &Connection) -> Result<Vec<Collection>> {
    let mut st = conn.prepare(
        "SELECT c.key, c.name, c.parent, (SELECT COUNT(*) FROM z_item_collections ic WHERE ic.collection_key=c.key)
         FROM z_collections c ORDER BY c.name COLLATE NOCASE",
    )?;
    let rows = st.query_map([], |r| {
        Ok(Collection { key: r.get(0)?, name: r.get(1)?, parent: r.get(2)?, item_count: r.get(3)? })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// List of items, filtered by collection and/or text (title, authors, key, journal).
pub fn items(conn: &Connection, collection: Option<&str>, query: Option<&str>, limit: usize) -> Result<Vec<ItemSummary>> {
    let mut sql = String::from(
        "SELECT i.key, i.item_type, i.title, i.json, i.year, i.citekey, i.publication,
                (SELECT COUNT(*) FROM z_annotations a WHERE a.item_key=i.key)
         FROM z_items i WHERE 1=1",
    );
    let mut args: Vec<String> = vec![];
    if let Some(c) = collection.filter(|c| !c.is_empty()) {
        args.push(c.to_string());
        sql.push_str(&format!(
            " AND i.key IN (SELECT item_key FROM z_item_collections WHERE collection_key=?{})",
            args.len()
        ));
    }
    if let Some(q) = query.map(str::trim).filter(|q| !q.is_empty()) {
        for word in q.split_whitespace() {
            args.push(format!("%{}%", word.replace('%', "")));
            let n = args.len();
            sql.push_str(&format!(
                " AND (i.title LIKE ?{n} OR i.authors LIKE ?{n} OR i.citekey LIKE ?{n} OR i.publication LIKE ?{n} OR CAST(i.year AS TEXT) LIKE ?{n})"
            ));
        }
    }
    sql.push_str(&format!(" ORDER BY i.date_added DESC LIMIT {limit}"));
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        let json: String = r.get(3)?;
        let item: Item = serde_json::from_str(&json).unwrap_or_default();
        Ok(ItemSummary {
            key: r.get(0)?,
            item_type: r.get(1)?,
            title: r.get(2)?,
            authors: authors_short(&item.creators),
            year: r.get(4)?,
            citekey: r.get(5)?,
            publication: r.get(6)?,
            annotation_count: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn item(conn: &Connection, key: &str) -> Result<Option<Item>> {
    let json: Option<String> = conn.query_row("SELECT json FROM z_items WHERE key=?1", [key], |r| r.get(0)).optional()?;
    Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
}

pub fn item_by_citekey(conn: &Connection, citekey: &str) -> Result<Option<Item>> {
    let json: Option<String> = conn
        .query_row("SELECT json FROM z_items WHERE citekey=?1 COLLATE NOCASE LIMIT 1", [citekey], |r| r.get(0))
        .optional()?;
    Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
}

pub fn annotations(conn: &Connection, item_key: &str) -> Result<Vec<Annotation>> {
    let mut st = conn.prepare("SELECT json FROM z_annotations WHERE item_key=?1 ORDER BY sort")?;
    let rows = st.query_map([item_key], |r| r.get::<_, String>(0))?;
    Ok(rows.flatten().filter_map(|j| serde_json::from_str(&j).ok()).collect())
}

/// Search in the text and comments of annotations.
pub fn search_annotations(conn: &Connection, query: &str, limit: usize) -> Result<Vec<(Annotation, String)>> {
    let q = format!("%{}%", query.trim().replace('%', ""));
    let mut st = conn.prepare(
        "SELECT a.json, i.citekey FROM z_annotations a JOIN z_items i ON i.key=a.item_key
         WHERE a.text LIKE ?1 OR a.comment LIKE ?1 ORDER BY i.citekey, a.sort LIMIT ?2",
    )?;
    let rows = st.query_map(params![q, limit as i64], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows
        .flatten()
        .filter_map(|(j, ck)| serde_json::from_str(&j).ok().map(|a| (a, ck)))
        .collect())
}

pub fn all_items(conn: &Connection) -> Result<Vec<Item>> {
    let mut st = conn.prepare("SELECT json FROM z_items ORDER BY citekey")?;
    let rows = st.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.flatten().filter_map(|j| serde_json::from_str(&j).ok()).collect())
}

/// Syncs the mirror. `scratch` is a temporary folder for the SQLite copy.
pub fn sync(conn: &Connection, source: Source, data_dir: Option<&Path>, port: u16, scratch: &Path) -> Result<MirrorStatus> {
    init_mirror(conn)?;
    let dir = data_dir.map(Path::to_path_buf).filter(|p| p.join("zotero.sqlite").exists()).or_else(default_data_dir);
    let (lib, used) = match source {
        Source::Sqlite => {
            let d = dir.ok_or_else(|| Error::Zotero("dossier de données Zotero introuvable".into()))?;
            (read_sqlite(&d, scratch)?, "sqlite")
        }
        Source::Api => (read_api(port)?, "api"),
        Source::Auto => match dir.map(|d| read_sqlite(&d, scratch)) {
            Some(Ok(l)) => (l, "sqlite"),
            Some(Err(e)) => match read_api(port) {
                Ok(l) => (l, "api"),
                Err(_) => return Err(e),
            },
            None => (read_api(port)?, "api"),
        },
    };
    store_library(conn, &lib, used)?;
    status(conn)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Builds a minimal database with the Zotero schema (the subset used).
    pub fn fake_zotero(path: &Path) {
        let c = Connection::open(path).unwrap();
        c.execute_batch(
            "CREATE TABLE libraries(libraryID INTEGER PRIMARY KEY, type TEXT);
             INSERT INTO libraries VALUES(1,'user');
             CREATE TABLE itemTypes(itemTypeID INTEGER PRIMARY KEY, typeName TEXT);
             INSERT INTO itemTypes VALUES(1,'journalArticle'),(2,'attachment'),(3,'annotation'),(4,'book'),(5,'note');
             CREATE TABLE items(itemID INTEGER PRIMARY KEY, itemTypeID INT, libraryID INT, key TEXT, dateAdded TEXT, dateModified TEXT);
             INSERT INTO items VALUES
               (1,1,1,'ITEMAAAA','2024-01-01 10:00:00','2024-01-02 10:00:00'),
               (2,2,1,'ATTAAAAA','2024-01-01 10:00:00','2024-01-01 10:00:00'),
               (3,3,1,'ANNAAAAA','2024-01-03 10:00:00','2024-01-03 10:00:00'),
               (4,3,1,'ANNBBBBB','2024-01-03 10:00:00','2024-01-03 10:00:00'),
               (5,4,1,'ITEMBBBB','2024-02-01 10:00:00','2024-02-01 10:00:00'),
               (6,1,1,'ITEMDELE','2024-02-01 10:00:00','2024-02-01 10:00:00');
             CREATE TABLE deletedItems(itemID INTEGER PRIMARY KEY);
             INSERT INTO deletedItems VALUES(6);
             CREATE TABLE fields(fieldID INTEGER PRIMARY KEY, fieldName TEXT);
             INSERT INTO fields VALUES(1,'title'),(2,'date'),(3,'publicationTitle'),(4,'DOI'),(5,'extra'),(6,'abstractNote');
             CREATE TABLE itemDataValues(valueID INTEGER PRIMARY KEY, value);
             INSERT INTO itemDataValues VALUES(1,'Graph Neural Networks for Mobile Money Fraud'),(2,'2024-03-00 2024'),
               (3,'IEEE Access'),(4,'10.1/abc'),(5,'Le Développement en Afrique'),(6,'2019'),(7,'Citation Key: kone2019dev'),
               (8,'Article PDF'),(9,'Résumé de test');
             CREATE TABLE itemData(itemID INT, fieldID INT, valueID INT);
             INSERT INTO itemData VALUES(1,1,1),(1,2,2),(1,3,3),(1,4,4),(1,6,9),(5,1,5),(5,2,6),(5,5,7),(2,1,8);
             CREATE TABLE creatorTypes(creatorTypeID INTEGER PRIMARY KEY, creatorType TEXT);
             INSERT INTO creatorTypes VALUES(1,'author'),(2,'editor');
             CREATE TABLE creators(creatorID INTEGER PRIMARY KEY, firstName TEXT, lastName TEXT, fieldMode INT);
             INSERT INTO creators VALUES(1,'Jane','Smith',0),(2,'Ali','Ouédraogo',0),(3,'Awa','Koné',0);
             CREATE TABLE itemCreators(itemID INT, creatorID INT, creatorTypeID INT, orderIndex INT);
             INSERT INTO itemCreators VALUES(1,1,1,0),(1,2,1,1),(5,3,1,0);
             CREATE TABLE tags(tagID INTEGER PRIMARY KEY, name TEXT);
             INSERT INTO tags VALUES(1,'GNN'),(2,'important');
             CREATE TABLE itemTags(itemID INT, tagID INT);
             INSERT INTO itemTags VALUES(1,1),(3,2);
             CREATE TABLE collections(collectionID INTEGER PRIMARY KEY, key TEXT, collectionName TEXT, parentCollectionID INT, libraryID INT);
             INSERT INTO collections VALUES(1,'COLLAAAA','Thèse',NULL,1),(2,'COLLBBBB','Ch2 État de l''art',1,1);
             CREATE TABLE collectionItems(collectionID INT, itemID INT);
             INSERT INTO collectionItems VALUES(2,1),(1,5);
             CREATE TABLE itemAttachments(itemID INTEGER PRIMARY KEY, parentItemID INT, contentType TEXT, path TEXT);
             INSERT INTO itemAttachments VALUES(2,1,'application/pdf','storage:smith.pdf');
             CREATE TABLE itemAnnotations(itemID INTEGER PRIMARY KEY, parentItemID INT, type INT, text TEXT, comment TEXT,
               color TEXT, pageLabel TEXT, sortIndex TEXT, position TEXT);
             INSERT INTO itemAnnotations VALUES
               (3,2,1,'Fraud rings form dense subgraphs','Clé pour le ch. 4','#ffd400','12','00011|000100|00200','{\"pageIndex\":19,\"rects\":[[1,2,3,4]]}'),
               (4,2,2,NULL,'Note libre','#5fb236','3','00002|000010|00100','{\"pageIndex\":2}');",
        )
        .unwrap();
    }

    /// Adds a PDF attachment to an item of the fake database.
    pub fn add_attachment(db: &Path, key: &str, parent_item: i64, path: &str) {
        let c = Connection::open(db).unwrap();
        let id: i64 = c.query_row("SELECT MAX(itemID) + 1 FROM items", [], |r| r.get(0)).unwrap();
        c.execute("INSERT INTO items VALUES(?1,2,1,?2,'2024-02-02 10:00:00','2024-02-02 10:00:00')", params![id, key]).unwrap();
        c.execute("INSERT INTO itemAttachments VALUES(?1,?2,'application/pdf',?3)", params![id, parent_item, path]).unwrap();
    }

    #[test]
    fn sqlite_read() {
        let d = tempfile::tempdir().unwrap();
        fake_zotero(&d.path().join("zotero.sqlite"));
        let lib = read_sqlite(d.path(), &d.path().join("scratch")).unwrap();
        assert_eq!(lib.items.len(), 2);
        let a = lib.items.iter().find(|i| i.key == "ITEMAAAA").unwrap();
        assert_eq!(a.title, "Graph Neural Networks for Mobile Money Fraud");
        assert_eq!(a.year, Some(2024));
        assert_eq!(a.citekey, "smith2024graph");
        assert_eq!(a.creators.len(), 2);
        assert_eq!(a.attachments[0].key, "ATTAAAAA");
        assert_eq!(a.collections, vec!["COLLBBBB"]);
        let b = lib.items.iter().find(|i| i.key == "ITEMBBBB").unwrap();
        assert_eq!(b.citekey, "kone2019dev");
        assert_eq!(lib.annotations.len(), 2);
        assert_eq!(lib.annotations[0].kind, "note"); // sorted by sortIndex
        assert_eq!(lib.annotations[1].tags, vec!["important"]);
        assert_eq!(lib.annotations[1].page_label, "12");
        assert_eq!(lib.annotations[1].page_index, Some(19));
        assert_eq!(lib.collections.iter().find(|c| c.key == "COLLBBBB").unwrap().parent.as_deref(), Some("COLLAAAA"));
    }

    #[test]
    fn standalone_pdf() {
        let d = tempfile::tempdir().unwrap();
        let db = d.path().join("zotero.sqlite");
        fake_zotero(&db);
        Connection::open(&db)
            .unwrap()
            .execute_batch(
                "INSERT INTO items VALUES(7,2,1,'SOLOPDF1','2024-03-01 10:00:00','2024-03-01 10:00:00'),
                                         (8,3,1,'ANNSOLO1','2024-03-02 10:00:00','2024-03-02 10:00:00');
                 INSERT INTO itemAttachments VALUES(7,NULL,'application/pdf','storage:Rapport BCEAO 2023.pdf');
                 INSERT INTO itemAnnotations VALUES(8,7,1,'Taux de pénétration','','#ffd400','2','00001|000001|00010',NULL);",
            )
            .unwrap();
        let lib = read_sqlite(d.path(), &d.path().join("scratch")).unwrap();
        assert_eq!(lib.items.len(), 3);
        let solo = lib.items.iter().find(|i| i.key == "SOLOPDF1").unwrap();
        assert_eq!(solo.title, "Rapport BCEAO 2023");
        assert_eq!(solo.attachments[0].key, "SOLOPDF1");
        let ann = lib.annotations.iter().find(|a| a.key == "ANNSOLO1").unwrap();
        assert_eq!(ann.item_key, "SOLOPDF1");
        assert_eq!(ann.page_index, None);
        // Attachment file: storage/<key>/<name>
        let att = &solo.attachments[0];
        assert_eq!(attachment_file(d.path(), att), None);
        let f = d.path().join("storage").join("SOLOPDF1");
        std::fs::create_dir_all(&f).unwrap();
        std::fs::write(f.join("Rapport BCEAO 2023.pdf"), b"%PDF").unwrap();
        assert_eq!(attachment_file(d.path(), att), Some(f.join("Rapport BCEAO 2023.pdf")));
    }

    #[test]
    fn mirror() {
        let d = tempfile::tempdir().unwrap();
        fake_zotero(&d.path().join("zotero.sqlite"));
        let conn = Connection::open_in_memory().unwrap();
        let st = sync(&conn, Source::Sqlite, Some(d.path()), 1, &d.path().join("s")).unwrap();
        assert_eq!(st.items, 2);
        assert_eq!(st.source.as_deref(), Some("sqlite"));
        assert_eq!(items(&conn, Some("COLLBBBB"), None, 50).unwrap().len(), 1);
        let found = items(&conn, None, Some("ouédraogo 2024"), 50).unwrap();
        assert_eq!(found[0].authors, "Smith & Ouédraogo");
        assert_eq!(found[0].annotation_count, 2);
        assert!(item_by_citekey(&conn, "KONE2019DEV").unwrap().is_some());
        assert_eq!(search_annotations(&conn, "subgraph", 10).unwrap()[0].1, "smith2024graph");
        assert_eq!(collections(&conn).unwrap().len(), 2);
    }

    #[test]
    fn json_api() {
        let items = serde_json::json!([
            {"key":"I1","data":{"key":"I1","itemType":"journalArticle","title":"Deep Learning","date":"2021",
              "creators":[{"creatorType":"author","firstName":"Y","lastName":"LeCun"}],"tags":[{"tag":"dl"}],
              "collections":["C1"],"dateAdded":"2021-01-01T00:00:00Z"}},
            {"key":"A1","data":{"key":"A1","itemType":"attachment","parentItem":"I1","contentType":"application/pdf","title":"PDF"}},
            {"key":"N1","data":{"key":"N1","itemType":"annotation","parentItem":"A1","annotationType":"highlight",
              "annotationText":"texte","annotationComment":"","annotationColor":"#ffd400","annotationPageLabel":"5","annotationSortIndex":"00004"}}
        ]);
        let colls = serde_json::json!([{"key":"C1","data":{"key":"C1","name":"ML","parentCollection":false}}]);
        let lib = library_from_api_json(items.as_array().unwrap(), colls.as_array().unwrap()).unwrap();
        assert_eq!(lib.items[0].citekey, "lecun2021deep");
        assert_eq!(lib.annotations[0].item_key, "I1");
        assert_eq!(lib.collections[0].parent, None);
    }

    #[test]
    fn helpers() {
        assert_eq!(extract_year("March 2019"), Some(2019));
        assert_eq!(citekey_from_extra("foo\nCitation Key: abc2020x"), Some("abc2020x".into()));
        assert_eq!(open_pdf_link("K", Some("12"), Some("A")), "zotero://open-pdf/library/items/K?page=12&annotation=A");
        let mut v = vec![
            Item { key: "1".into(), citekey: "x2020".into(), ..Default::default() },
            Item { key: "2".into(), citekey: "x2020".into(), ..Default::default() },
        ];
        ensure_unique_citekeys(&mut v, &HashSet::new());
        assert_eq!(v[1].citekey, "x2020a");
    }
}
