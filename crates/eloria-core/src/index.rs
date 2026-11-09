//! SQLite index of the vault (`.eloria/index.db`). It can be entirely rebuilt
//! from the files: when in doubt, `rebuild()`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::markdown::{parse_note, ParsedNote};
use crate::vault::{stem, Vault};
use crate::Result;

/// 4: the FTS table shares the rowid of `notes`.
const SCHEMA_VERSION: i32 = 4;

pub struct Index {
    pub(crate) conn: Connection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteMeta {
    pub path: String,
    pub title: String,
    pub stem: String,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    pub mtime: i64,
    pub words: i64,
    pub frontmatter: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub path: String,
    pub title: String,
    /// Excerpt; matched terms are wrapped in \u{1} ... \u{2}.
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkRef {
    pub path: String,
    pub title: String,
    pub line: i64,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutLink {
    pub target: String,
    pub resolved: Option<String>,
    pub line: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    /// "note", "tag" or "missing"
    pub kind: String,
    pub degree: usize,
    /// First folder of the path (used to colour groups).
    pub group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    /// "link" or "tag"
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphOptions {
    pub include_tags: bool,
    pub include_orphans: bool,
    pub include_missing: bool,
    /// Keep only notes whose path starts with this prefix.
    pub folder: Option<String>,
    /// Excluded folders (e.g. templates).
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Tags shown only if they hold at least this many notes (a tag
    /// on a single note links nothing).
    #[serde(default = "default_min_tag_notes")]
    pub min_tag_notes: usize,
    /// Tags hidden if they hold more than this share of notes (e.g. `#fiche` on every
    /// reading note: it would link everything to everything). 1.0 = no hiding.
    #[serde(default = "default_max_tag_share")]
    pub max_tag_share: f64,
}

fn default_min_tag_notes() -> usize {
    2
}

fn default_max_tag_share() -> f64 {
    0.5
}

impl Default for GraphOptions {
    fn default() -> Self {
        Self {
            include_tags: false,
            include_orphans: true,
            include_missing: false,
            folder: None,
            exclude: vec![],
            min_tag_notes: default_min_tag_notes(),
            max_tag_share: default_max_tag_share(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncStats {
    pub indexed: usize,
    pub removed: usize,
    pub total: usize,
}

impl Index {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=OFF;")?;
        let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version != SCHEMA_VERSION {
            conn.execute_batch(
                "DROP TABLE IF EXISTS notes; DROP TABLE IF EXISTS links; DROP TABLE IF EXISTS tags;
                 DROP TABLE IF EXISTS citations; DROP TABLE IF EXISTS notes_fts;",
            )?;
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS notes(
                path TEXT PRIMARY KEY, title TEXT NOT NULL, stem TEXT NOT NULL,
                mtime INTEGER NOT NULL, words INTEGER NOT NULL,
                frontmatter TEXT NOT NULL, aliases TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS links(
                src TEXT NOT NULL, target TEXT NOT NULL, heading TEXT, line INTEGER NOT NULL,
                embed INTEGER NOT NULL, context TEXT NOT NULL, resolved TEXT);
             CREATE INDEX IF NOT EXISTS links_src ON links(src);
             CREATE INDEX IF NOT EXISTS links_res ON links(resolved);
             CREATE TABLE IF NOT EXISTS tags(path TEXT NOT NULL, tag TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS tags_tag ON tags(tag);
             CREATE INDEX IF NOT EXISTS tags_path ON tags(path);
             CREATE TABLE IF NOT EXISTS citations(path TEXT NOT NULL, citekey TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS cit_key ON citations(citekey);
             CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
                path UNINDEXED, title, body, tokenize = 'unicode61 remove_diacritics 2');",
        )?;
        conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
        Ok(Self { conn })
    }

    /// In-memory index (tests).
    pub fn in_memory() -> Result<Self> {
        Self::open(Path::new(":memory:"))
    }

    fn delete_note_rows(&self, path: &str) -> Result<()> {
        // The FTS table shares the rowid of `notes`: direct delete, without
        // scanning the full-text index (the `path` column is not indexed there).
        let rowid: Option<i64> =
            self.conn.query_row("SELECT rowid FROM notes WHERE path=?1", [path], |r| r.get(0)).optional()?;
        if let Some(id) = rowid {
            self.conn.execute("DELETE FROM notes_fts WHERE rowid=?1", [id])?;
        }
        for sql in [
            "DELETE FROM notes WHERE path=?1",
            "DELETE FROM links WHERE src=?1",
            "DELETE FROM tags WHERE path=?1",
            "DELETE FROM citations WHERE path=?1",
        ] {
            self.conn.execute(sql, [path])?;
        }
        Ok(())
    }

    /// Indexes (or reindexes) a note. Does not resolve links: call
    /// [`Index::resolve_links`] afterwards.
    pub fn upsert(&self, path: &str, content: &str, mtime: i64) -> Result<ParsedNote> {
        let p = parse_note(content);
        let st = stem(path);
        let title = p.title.clone().unwrap_or_else(|| st.clone());
        self.delete_note_rows(path)?;
        self.conn.execute(
            "INSERT INTO notes(path,title,stem,mtime,words,frontmatter,aliases) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                path,
                title,
                st,
                mtime,
                p.word_count as i64,
                serde_json::to_string(&p.frontmatter)?,
                serde_json::to_string(&p.aliases)?
            ],
        )?;
        let rowid = self.conn.last_insert_rowid();
        let lines: Vec<&str> = content.lines().collect();
        {
            let mut ins = self.conn.prepare_cached(
                "INSERT INTO links(src,target,heading,line,embed,context) VALUES(?1,?2,?3,?4,?5,?6)",
            )?;
            for l in &p.links {
                let ctx = lines.get(l.line).map(|s| s.trim()).unwrap_or("");
                let ctx: String = ctx.chars().take(240).collect();
                ins.execute(params![path, l.target, l.heading, l.line as i64, l.embed as i64, ctx])?;
            }
            let mut ins = self.conn.prepare_cached("INSERT INTO tags(path,tag) VALUES(?1,?2)")?;
            for t in &p.tags {
                ins.execute(params![path, t])?;
            }
            let mut ins = self.conn.prepare_cached("INSERT INTO citations(path,citekey) VALUES(?1,?2)")?;
            for c in &p.citations {
                ins.execute(params![path, c])?;
            }
        }
        self.conn.execute(
            "INSERT INTO notes_fts(rowid,path,title,body) VALUES(?1,?2,?3,?4)",
            params![rowid, path, title, p.body],
        )?;
        Ok(p)
    }

    /// Indexes a note and resolves links as narrowly as possible: only those of the note
    /// if the set of known names did not change (existing note, same aliases),
    /// otherwise every link of the vault (a new note may resolve others).
    pub fn update_note(&self, path: &str, content: &str, mtime: i64) -> Result<ParsedNote> {
        let before: Option<String> =
            self.conn.query_row("SELECT aliases FROM notes WHERE path=?1", [path], |r| r.get(0)).optional()?;
        self.conn.execute_batch("SAVEPOINT maj_note")?;
        let res = self.upsert(path, content, mtime);
        match res {
            Ok(p) => {
                self.conn.execute_batch("RELEASE maj_note")?;
                if before.as_deref() == Some(serde_json::to_string(&p.aliases)?.as_str()) {
                    self.resolve_links_of(path)?;
                } else {
                    self.resolve_links()?;
                }
                Ok(p)
            }
            Err(e) => {
                self.conn.execute_batch("ROLLBACK TO maj_note; RELEASE maj_note")?;
                Err(e)
            }
        }
    }

    pub fn remove(&self, path: &str) -> Result<()> {
        self.delete_note_rows(path)
    }

    /// Removes every note below a folder.
    pub fn remove_prefix(&self, folder: &str) -> Result<()> {
        let prefix = format!("{}/", folder.trim_end_matches('/'));
        let paths: Vec<String> = {
            let mut st = self.conn.prepare("SELECT path FROM notes WHERE substr(path,1,length(?1))=?1")?;
            let rows = st.query_map([&prefix], |r| r.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        for p in paths {
            self.remove(&p)?;
        }
        Ok(())
    }

    /// Syncs the index with the disk (only modified files are read again).
    pub fn sync(&self, vault: &Vault) -> Result<SyncStats> {
        let on_disk = vault.list_notes();
        let known: HashMap<String, i64> = {
            let mut st = self.conn.prepare("SELECT path, mtime FROM notes")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let mut stats = SyncStats { total: on_disk.len(), ..Default::default() };
        self.conn.execute_batch("BEGIN")?;
        let res = (|| -> Result<()> {
            let disk_set: HashSet<&str> = on_disk.iter().map(|(p, _)| p.as_str()).collect();
            for (path, m) in &on_disk {
                if known.get(path) != Some(m) {
                    if let Ok(content) = vault.read(path) {
                        self.upsert(path, &content, *m)?;
                        stats.indexed += 1;
                    }
                }
            }
            for path in known.keys() {
                if !disk_set.contains(path.as_str()) {
                    self.remove(path)?;
                    stats.removed += 1;
                }
            }
            Ok(())
        })();
        match res {
            Ok(()) => self.conn.execute_batch("COMMIT")?,
            Err(e) => {
                self.conn.execute_batch("ROLLBACK")?;
                return Err(e);
            }
        }
        self.resolve_links()?;
        Ok(stats)
    }

    pub fn rebuild(&self, vault: &Vault) -> Result<SyncStats> {
        self.conn.execute_batch(
            "DELETE FROM notes; DELETE FROM links; DELETE FROM tags; DELETE FROM citations; DELETE FROM notes_fts;",
        )?;
        self.sync(vault)
    }

    /// Resolution table: name (lower case) -> path.
    fn resolver(&self) -> Result<Resolver> {
        let mut st = self.conn.prepare("SELECT path, stem, aliases FROM notes")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
        let mut r = Resolver::default();
        for row in rows {
            let (path, st, aliases) = row?;
            r.add(&path, &st, serde_json::from_str::<Vec<String>>(&aliases).unwrap_or_default());
        }
        Ok(r)
    }

    /// Recomputes the target of every link.
    pub fn resolve_links(&self) -> Result<()> {
        let r = self.resolver()?;
        let rows: Vec<(i64, String, String)> = {
            let mut st = self.conn.prepare("SELECT rowid, target, src FROM links")?;
            let it = st.query_map([], |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?)))?;
            it.collect::<std::result::Result<_, _>>()?
        };
        self.conn.execute_batch("BEGIN")?;
        {
            let mut up = self.conn.prepare_cached("UPDATE links SET resolved=?1 WHERE rowid=?2")?;
            for (id, target, src) in rows {
                // Link to a section of the current note: [[#Section]]
                let res = if target.is_empty() { Some(src) } else { r.resolve(&target) };
                up.execute(params![res, id])?;
            }
        }
        self.conn.execute_batch("COMMIT")?;
        Ok(())
    }

    /// Recomputes the target of the links of a single note.
    pub fn resolve_links_of(&self, src: &str) -> Result<()> {
        let r = self.resolver()?;
        let rows: Vec<(i64, String)> = {
            let mut st = self.conn.prepare_cached("SELECT rowid, target FROM links WHERE src=?1")?;
            let it = st.query_map([src], |x| Ok((x.get(0)?, x.get(1)?)))?;
            it.collect::<std::result::Result<_, _>>()?
        };
        let mut up = self.conn.prepare_cached("UPDATE links SET resolved=?1 WHERE rowid=?2")?;
        for (id, target) in rows {
            let res = if target.is_empty() { Some(src.to_string()) } else { r.resolve(&target) };
            up.execute(params![res, id])?;
        }
        Ok(())
    }

    /// Resolves a link target to a vault note.
    pub fn resolve(&self, target: &str) -> Result<Option<String>> {
        Ok(self.resolver()?.resolve(target))
    }

    pub fn all_notes(&self) -> Result<Vec<NoteMeta>> {
        let mut tags: HashMap<String, Vec<String>> = HashMap::new();
        {
            let mut st = self.conn.prepare("SELECT path, tag FROM tags")?;
            for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
                let (p, t) = row?;
                tags.entry(p).or_default().push(t);
            }
        }
        let mut st = self
            .conn
            .prepare("SELECT path,title,stem,aliases,mtime,words,frontmatter FROM notes ORDER BY title COLLATE NOCASE")?;
        let rows = st.query_map([], |r| {
            Ok(NoteMeta {
                path: r.get(0)?,
                title: r.get(1)?,
                stem: r.get(2)?,
                aliases: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                tags: vec![],
                mtime: r.get(4)?,
                words: r.get(5)?,
                frontmatter: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            let mut m = row?;
            m.tags = tags.remove(&m.path).unwrap_or_default();
            out.push(m);
        }
        Ok(out)
    }

    pub fn note_meta(&self, path: &str) -> Result<Option<NoteMeta>> {
        Ok(self.all_notes()?.into_iter().find(|n| n.path == path))
    }

    /// Full-text search. Syntax: words (implicit AND), `"exact phrase"`,
    /// `#tag` (tag filter), `-word` (exclusion).
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let (fts, tag_filters) = build_fts_query(query);
        if fts.is_empty() && tag_filters.is_empty() {
            return Ok(vec![]);
        }
        let mut hits = Vec::new();
        if fts.is_empty() {
            // Tags only
            let mut st = self.conn.prepare("SELECT path, title FROM notes ORDER BY mtime DESC")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for row in rows {
                let (path, title) = row?;
                if self.has_tags(&path, &tag_filters)? {
                    hits.push(SearchHit { path, title, snippet: String::new() });
                    if hits.len() >= limit {
                        break;
                    }
                }
            }
            return Ok(hits);
        }
        let mut st = self.conn.prepare(
            "SELECT path, title, snippet(notes_fts, 2, char(1), char(2), '…', 14)
             FROM notes_fts WHERE notes_fts MATCH ?1
             ORDER BY bm25(notes_fts, 0.0, 8.0, 1.0) LIMIT ?2",
        )?;
        let rows = st.query_map(params![fts, (limit * 4) as i64], |r| {
            Ok(SearchHit { path: r.get(0)?, title: r.get(1)?, snippet: r.get(2)? })
        })?;
        for row in rows {
            let h = match row {
                Ok(h) => h,
                Err(_) => continue,
            };
            if tag_filters.is_empty() || self.has_tags(&h.path, &tag_filters)? {
                hits.push(h);
                if hits.len() >= limit {
                    break;
                }
            }
        }
        Ok(hits)
    }

    /// Notes containing at least one of the words (prefixes), most relevant first.
    /// For the question assistant: ordinary search requires every word.
    pub fn search_any(&self, words: &[String], limit: usize) -> Result<Vec<(String, String)>> {
        let terms: Vec<String> = words
            .iter()
            .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
            .filter(|w| !w.is_empty())
            .map(|w| format!("\"{w}\"*"))
            .collect();
        if terms.is_empty() {
            return Ok(vec![]);
        }
        let mut st = self.conn.prepare(
            "SELECT path, title FROM notes_fts WHERE notes_fts MATCH ?1 ORDER BY bm25(notes_fts, 0.0, 6.0, 1.0) LIMIT ?2",
        )?;
        let rows = st.query_map(params![terms.join(" OR "), limit as i64], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    fn has_tags(&self, path: &str, wanted: &[String]) -> Result<bool> {
        let tags = self.tags_of(path)?;
        Ok(wanted.iter().all(|w| {
            let w = w.to_lowercase();
            tags.iter().any(|t| {
                let t = t.to_lowercase();
                t == w || t.starts_with(&format!("{w}/"))
            })
        }))
    }

    pub fn tags_of(&self, path: &str) -> Result<Vec<String>> {
        let mut st = self.conn.prepare("SELECT tag FROM tags WHERE path=?1")?;
        let rows = st.query_map([path], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Notes that link to `path`.
    pub fn backlinks(&self, path: &str) -> Result<Vec<LinkRef>> {
        let mut st = self.conn.prepare(
            "SELECT l.src, n.title, l.line, l.context FROM links l JOIN notes n ON n.path=l.src
             WHERE l.resolved=?1 AND l.src<>?1 ORDER BY n.title COLLATE NOCASE, l.line",
        )?;
        let rows = st.query_map([path], |r| {
            Ok(LinkRef { path: r.get(0)?, title: r.get(1)?, line: r.get(2)?, context: r.get(3)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Unlinked mentions: notes that contain the title of `path` without a link.
    pub fn unlinked_mentions(&self, path: &str) -> Result<Vec<SearchHit>> {
        let Some(meta) = self.note_meta(path)? else { return Ok(vec![]) };
        let linked: HashSet<String> = self.backlinks(path)?.into_iter().map(|b| b.path).collect();
        let phrase = format!("\"{}\"", meta.stem.replace('"', ""));
        let mut st = self.conn.prepare(
            "SELECT path, title, snippet(notes_fts, 2, char(1), char(2), '…', 14)
             FROM notes_fts WHERE notes_fts MATCH ?1 LIMIT 50",
        )?;
        let rows = st.query_map([format!("body:{phrase}")], |r| {
            Ok(SearchHit { path: r.get(0)?, title: r.get(1)?, snippet: r.get(2)? })
        });
        let mut out = vec![];
        if let Ok(rows) = rows {
            for h in rows.flatten() {
                if h.path != path && !linked.contains(&h.path) {
                    out.push(h);
                }
            }
        }
        Ok(out)
    }

    pub fn outgoing(&self, path: &str) -> Result<Vec<OutLink>> {
        let mut st = self.conn.prepare("SELECT target, resolved, line FROM links WHERE src=?1 ORDER BY line")?;
        let rows = st.query_map([path], |r| Ok(OutLink { target: r.get(0)?, resolved: r.get(1)?, line: r.get(2)? }))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Tags with their number of occurrences (distinct notes).
    pub fn tags(&self) -> Result<Vec<(String, i64)>> {
        let mut st = self.conn.prepare(
            "SELECT tag, COUNT(DISTINCT path) FROM tags GROUP BY lower(tag) ORDER BY lower(tag)",
        )?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Notes that cite a Zotero key.
    pub fn notes_citing(&self, citekey: &str) -> Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare(
            "SELECT DISTINCT c.path, n.title FROM citations c JOIN notes n ON n.path=c.path
             WHERE lower(c.citekey)=lower(?1) ORDER BY n.title",
        )?;
        let rows = st.query_map([citekey], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Every key cited in the vault (for the bibliography export).
    pub fn all_citekeys(&self) -> Result<Vec<String>> {
        let mut st = self.conn.prepare("SELECT DISTINCT citekey FROM citations")?;
        let rows = st.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn citekeys_of(&self, path: &str) -> Result<Vec<String>> {
        let mut st = self.conn.prepare("SELECT DISTINCT citekey FROM citations WHERE path=?1")?;
        let rows = st.query_map([path], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Graphe global.
    pub fn graph(&self, opts: &GraphOptions) -> Result<Graph> {
        let notes = self.all_notes()?;
        let keep = |p: &str| {
            opts.folder.as_ref().map(|f| f.is_empty() || p.starts_with(f.as_str())).unwrap_or(true)
                && !opts.exclude.iter().any(|e| !e.is_empty() && p.starts_with(&format!("{}/", e.trim_end_matches('/'))))
        };
        let mut nodes: HashMap<String, GraphNode> = HashMap::new();
        for n in notes.iter().filter(|n| keep(&n.path)) {
            nodes.insert(
                n.path.clone(),
                GraphNode {
                    id: n.path.clone(),
                    label: n.title.clone(),
                    kind: "note".into(),
                    degree: 0,
                    group: n.path.split('/').next().filter(|_| n.path.contains('/')).unwrap_or("").to_string(),
                },
            );
        }
        let mut edges = Vec::new();
        let mut seen = HashSet::new();
        {
            let mut st = self.conn.prepare("SELECT src, target, resolved FROM links WHERE embed=0 OR resolved IS NOT NULL")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)))?;
            for row in rows {
                let (src, target, resolved) = row?;
                if !nodes.contains_key(&src) || target.is_empty() {
                    continue;
                }
                let tgt = match resolved {
                    Some(r) if nodes.contains_key(&r) => r,
                    Some(_) => continue,
                    None if opts.include_missing && !target.contains('.') => {
                        let id = format!("missing:{}", target.to_lowercase());
                        nodes.entry(id.clone()).or_insert(GraphNode {
                            id: id.clone(),
                            label: target.clone(),
                            kind: "missing".into(),
                            degree: 0,
                            group: String::new(),
                        });
                        id
                    }
                    None => continue,
                };
                if src == tgt || !seen.insert((src.clone(), tgt.clone())) {
                    continue;
                }
                edges.push(GraphEdge { source: src, target: tgt, kind: "link".into() });
            }
        }
        // Citations: a note that cites [@key] is linked to the reading note of that item.
        {
            let fiche_of: HashMap<String, String> = notes
                .iter()
                .filter(|n| nodes.contains_key(&n.path))
                .filter_map(|n| Some((n.frontmatter.get("citekey")?.as_str()?.to_lowercase(), n.path.clone())))
                .collect();
            let mut st = self.conn.prepare("SELECT path, citekey FROM citations")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for row in rows {
                let (src, ck) = row?;
                let Some(fiche) = fiche_of.get(&ck.to_lowercase()) else { continue };
                if &src == fiche || !nodes.contains_key(&src) || !seen.insert((src.clone(), fiche.clone())) {
                    continue;
                }
                edges.push(GraphEdge { source: src, target: fiche.clone(), kind: "citation".into() });
            }
        }
        if opts.include_tags {
            let kept: HashSet<String> = nodes.keys().cloned().collect();
            let kept_notes = notes.iter().filter(|n| kept.contains(&n.path)).count();
            // Number of notes per tag, to drop tags that link nothing or everything.
            let mut per_tag: HashMap<String, usize> = HashMap::new();
            for n in notes.iter().filter(|n| kept.contains(&n.path)) {
                for t in &n.tags {
                    *per_tag.entry(t.to_lowercase()).or_default() += 1;
                }
            }
            let shown = |t: &str| {
                let c = per_tag.get(&t.to_lowercase()).copied().unwrap_or(0);
                let too_common = kept_notes >= 10 && (c as f64) > opts.max_tag_share * kept_notes as f64;
                c >= opts.min_tag_notes.max(1) && !too_common
            };
            for n in notes.iter().filter(|n| kept.contains(&n.path)) {
                for t in n.tags.iter().filter(|t| shown(t)) {
                    let id = format!("tag:{}", t.to_lowercase());
                    nodes.entry(id.clone()).or_insert(GraphNode {
                        id: id.clone(),
                        label: format!("#{t}"),
                        kind: "tag".into(),
                        degree: 0,
                        group: String::new(),
                    });
                    edges.push(GraphEdge { source: n.path.clone(), target: id, kind: "tag".into() });
                }
            }
        }
        for e in &edges {
            if let Some(n) = nodes.get_mut(&e.source) {
                n.degree += 1;
            }
            if let Some(n) = nodes.get_mut(&e.target) {
                n.degree += 1;
            }
        }
        let mut nodes: Vec<GraphNode> = nodes.into_values().collect();
        if !opts.include_orphans {
            nodes.retain(|n| n.degree > 0);
        }
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Graph { nodes, edges })
    }

    /// Local graph: neighbourhood of `path` up to `depth` hops.
    pub fn local_graph(&self, path: &str, depth: usize, include_tags: bool) -> Result<Graph> {
        // Local graph: every tag of the note counts, even rare ones.
        let full = self.graph(&GraphOptions { include_tags, min_tag_notes: 1, max_tag_share: 1.0, ..Default::default() })?;
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for e in &full.edges {
            adj.entry(&e.source).or_default().push(&e.target);
            adj.entry(&e.target).or_default().push(&e.source);
        }
        let mut keep: HashSet<String> = HashSet::new();
        let mut q = VecDeque::from([(path.to_string(), 0usize)]);
        keep.insert(path.to_string());
        while let Some((n, d)) = q.pop_front() {
            if d >= depth {
                continue;
            }
            // Tags do not propagate the exploration (otherwise the whole vault is linked).
            if n.starts_with("tag:") {
                continue;
            }
            for m in adj.get(n.as_str()).cloned().unwrap_or_default() {
                if keep.insert(m.to_string()) {
                    q.push_back((m.to_string(), d + 1));
                }
            }
        }
        Ok(Graph {
            nodes: full.nodes.into_iter().filter(|n| keep.contains(&n.id)).collect(),
            edges: full.edges.into_iter().filter(|e| keep.contains(&e.source) && keep.contains(&e.target)).collect(),
        })
    }

    pub fn note_count(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))?)
    }

    /// Modification time stored in the index (milliseconds).
    pub fn mtime_of(&self, path: &str) -> Result<Option<i64>> {
        Ok(self.conn.query_row("SELECT mtime FROM notes WHERE path=?1", [path], |r| r.get(0)).optional()?)
    }

    pub fn title_of(&self, path: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT title FROM notes WHERE path=?1", [path], |r| r.get(0)).optional()?)
    }
}

#[derive(Default)]
struct Resolver {
    by_path: HashMap<String, String>,
    by_stem: HashMap<String, Vec<String>>,
    by_alias: HashMap<String, String>,
}

impl Resolver {
    fn add(&mut self, path: &str, st: &str, aliases: Vec<String>) {
        let key = path.strip_suffix(".md").unwrap_or(path).to_lowercase();
        self.by_path.insert(key, path.to_string());
        self.by_stem.entry(st.to_lowercase()).or_default().push(path.to_string());
        for a in aliases {
            self.by_alias.entry(a.to_lowercase()).or_insert_with(|| path.to_string());
        }
    }

    fn resolve(&self, target: &str) -> Option<String> {
        let t = target.trim().replace('\\', "/");
        let t = t.trim_start_matches('/');
        let t = t.strip_suffix(".md").unwrap_or(t).to_lowercase();
        if t.is_empty() {
            return None;
        }
        if let Some(p) = self.by_path.get(&t) {
            return Some(p.clone());
        }
        if t.contains('/') {
            // Suffix match: [[sub/Note]] -> a/sub/Note.md
            let suffix = format!("/{t}");
            let mut c: Vec<&String> = self.by_path.iter().filter(|(k, _)| k.ends_with(&suffix)).map(|(_, v)| v).collect();
            c.sort_by_key(|p| p.len());
            if let Some(p) = c.first() {
                return Some((*p).clone());
            }
            let last = t.rsplit('/').next().unwrap_or(&t).to_string();
            return self.by_stem.get(&last).and_then(|v| shortest(v));
        }
        if let Some(v) = self.by_stem.get(&t) {
            return shortest(v);
        }
        self.by_alias.get(&t).cloned()
    }
}

fn shortest(v: &[String]) -> Option<String> {
    v.iter().min_by_key(|p| (p.matches('/').count(), p.len())).cloned()
}

/// Builds a safe FTS5 query from user input.
fn build_fts_query(q: &str) -> (String, Vec<String>) {
    let mut parts = Vec::new();
    let mut excluded: Vec<String> = Vec::new();
    let mut tags = Vec::new();
    let mut chars = q.chars().peekable();
    let mut tokens: Vec<(bool, String)> = Vec::new(); // (exact, texte)
    let mut cur = String::new();
    while let Some(c) = chars.next() {
        if c == '"' {
            if !cur.trim().is_empty() {
                tokens.push((false, std::mem::take(&mut cur)));
            }
            cur.clear();
            let mut phrase = String::new();
            for d in chars.by_ref() {
                if d == '"' {
                    break;
                }
                phrase.push(d);
            }
            if !phrase.trim().is_empty() {
                tokens.push((true, phrase));
            }
        } else if c.is_whitespace() {
            if !cur.is_empty() {
                tokens.push((false, std::mem::take(&mut cur)));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        tokens.push((false, cur));
    }
    let n = tokens.len();
    for (i, (exact, t)) in tokens.into_iter().enumerate() {
        if exact {
            parts.push(format!("\"{}\"", t.replace('"', "")));
            continue;
        }
        if let Some(tag) = t.strip_prefix('#') {
            if !tag.is_empty() {
                tags.push(tag.to_string());
            }
            continue;
        }
        let (neg, word) = match t.strip_prefix('-') {
            Some(w) if !w.is_empty() => (true, w.to_string()),
            _ => (false, t.clone()),
        };
        let clean: String = word.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '\'' || *c == '-').collect();
        let clean = clean.replace('"', "");
        if clean.is_empty() {
            continue;
        }
        // Prefix on the last word for search-as-you-type.
        let term = if i == n - 1 && !neg { format!("\"{clean}\"*") } else { format!("\"{clean}\"") };
        if neg {
            excluded.push(term);
        } else {
            parts.push(term);
        }
    }
    // NOT is binary in FTS5: exclusions come after the searched terms,
    // wherever they were typed. Without a positive term, nothing to exclude.
    if parts.is_empty() {
        return (String::new(), tags);
    }
    let mut q = parts.join(" ");
    for e in excluded {
        q.push_str(" NOT ");
        q.push_str(&e);
    }
    (q, tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Vault, Index) {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        v.write("A.md", "# Alpha\nLien vers [[B]] et [[Sous/C|c]]. #these\nMot fraude mobile.").unwrap();
        v.write("B.md", "---\naliases: [Bêta]\ntags: [these/ch2]\n---\nRetour [[A]]. Alpha est cité sans lien.").unwrap();
        v.write("Sous/C.md", "Voir [[Bêta]] et [[Inexistante]]. [@doe2020]").unwrap();
        let idx = Index::open(&v.eloria_dir().join("index.db")).unwrap();
        idx.sync(&v).unwrap();
        (d, v, idx)
    }

    #[test]
    fn links_and_backlinks() {
        let (_d, _v, idx) = setup();
        let bl: Vec<_> = idx.backlinks("B.md").unwrap().into_iter().map(|b| b.path).collect();
        assert_eq!(bl, vec!["A.md", "Sous/C.md"]);
        assert_eq!(idx.backlinks("Sous/C.md").unwrap().len(), 1);
        let out = idx.outgoing("Sous/C.md").unwrap();
        assert_eq!(out[1].resolved, None);
        assert_eq!(idx.resolve("bêta").unwrap().as_deref(), Some("B.md"));
    }

    #[test]
    fn search_cases() {
        let (_d, _v, idx) = setup();
        let h = idx.search("fraude", 10).unwrap();
        assert_eq!(h[0].path, "A.md");
        assert!(h[0].snippet.contains('\u{1}'));
        // Prefix + accent-insensitive
        assert_eq!(idx.search("frau", 10).unwrap().len(), 1);
        assert_eq!(idx.search("#these", 10).unwrap().len(), 2);
        assert_eq!(idx.search("alpha #these/ch2", 10).unwrap()[0].path, "B.md");
        assert!(idx.search("\"", 10).unwrap().is_empty());
        assert_eq!(idx.unlinked_mentions("A.md").unwrap().len(), 0); // B already links to A
        // Exclusion typed first: A contains "fraude", B does not.
        let ex: Vec<_> = idx.search("-fraude alpha", 10).unwrap().into_iter().map(|h| h.path).collect();
        assert_eq!(ex, vec!["B.md"]);
    }

    #[test]
    fn graph_and_tags() {
        let (_d, _v, idx) = setup();
        let g = idx.graph(&GraphOptions::default()).unwrap();
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.edges.len(), 4);
        let g2 = idx.graph(&GraphOptions { include_tags: true, include_missing: true, min_tag_notes: 1, ..Default::default() }).unwrap();
        assert!(g2.nodes.iter().any(|n| n.kind == "tag"));
        assert!(g2.nodes.iter().any(|n| n.kind == "missing"));
        let local = idx.local_graph("Sous/C.md", 1, false).unwrap();
        assert_eq!(local.nodes.len(), 3);
        let tags = idx.tags().unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(idx.notes_citing("DOE2020").unwrap().len(), 1);
    }

    #[test]
    fn graph_citations_and_useful_tags() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        v.write("F/@a.md", "---\ncitekey: a2020\ntags: [fiche, fraude]\n---\nFiche A").unwrap();
        v.write("F/@b.md", "---\ncitekey: b2021\ntags: [fiche, fraude, rare]\n---\nFiche B").unwrap();
        v.write("Ch2.md", "Comme montré [@a2020, p. 3]. #fiche").unwrap();
        let idx = Index::open(&v.eloria_dir().join("index.db")).unwrap();
        idx.sync(&v).unwrap();
        let g = idx.graph(&GraphOptions { include_tags: true, ..Default::default() }).unwrap();
        // The chapter is linked to the reading note it cites.
        assert!(g.edges.iter().any(|e| e.kind == "citation" && e.source == "Ch2.md" && e.target == "F/@a.md"));
        // #fraude (2 notes) links the reading notes; #rare (1 note) links nothing.
        let tag_ids: Vec<_> = g.nodes.iter().filter(|n| n.kind == "tag").map(|n| n.id.as_str()).collect();
        assert!(tag_ids.contains(&"tag:fraude") && !tag_ids.contains(&"tag:rare"));
        // With few notes, a very common tag stays visible; beyond 10 notes it is hidden.
        assert!(tag_ids.contains(&"tag:fiche"));
        for i in 0..12 {
            v.write(&format!("F/@x{i}.md"), "---\ntags: [fiche]\n---\nx").unwrap();
        }
        idx.sync(&v).unwrap();
        let g = idx.graph(&GraphOptions { include_tags: true, ..Default::default() }).unwrap();
        assert!(!g.nodes.iter().any(|n| n.id == "tag:fiche"));
    }

    #[test]
    fn incremental_sync() {
        let (_d, v, idx) = setup();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        v.write("A.md", "nouveau contenu").unwrap();
        std::fs::remove_file(v.resolve("Sous/C.md").unwrap()).unwrap();
        let s = idx.sync(&v).unwrap();
        assert_eq!(s.indexed, 1);
        assert_eq!(s.removed, 1);
        assert_eq!(idx.note_count().unwrap(), 2);
    }

    #[test]
    fn targeted_update() {
        let (_d, v, idx) = setup();
        // Replaced text disappears from search, the new text appears.
        idx.update_note("A.md", "# Alpha\nTexte zèbre. [[Future]]", 5).unwrap();
        assert!(idx.search("fraude", 10).unwrap().is_empty());
        assert_eq!(idx.search("zebre", 10).unwrap().len(), 1);
        assert_eq!(idx.note_count().unwrap(), 3);
        // A new note resolves the links that were waiting for it elsewhere.
        v.write("Future.md", "x").unwrap();
        idx.update_note("Future.md", "x", 6).unwrap();
        assert_eq!(idx.outgoing("A.md").unwrap()[0].resolved.as_deref(), Some("Future.md"));
        // So does a new alias.
        idx.update_note("Future.md", "---\naliases: [Inexistante]\n---\nx", 7).unwrap();
        let c = idx.outgoing("Sous/C.md").unwrap();
        assert_eq!(c[1].resolved.as_deref(), Some("Future.md"));
    }

    #[test]
    fn fts_query() {
        assert_eq!(build_fts_query("a b").0, "\"a\" \"b\"*");
        assert_eq!(build_fts_query("-x y").0, "\"y\"* NOT \"x\"");
        assert_eq!(build_fts_query("a -b c").0, "\"a\" \"c\"* NOT \"b\"");
        assert_eq!(build_fts_query("-seul").0, "");
        assert_eq!(build_fts_query("\"deep learning\" #t").0, "\"deep learning\"");
        assert_eq!(build_fts_query("\"deep learning\" #t").1, vec!["t"]);
    }
}
