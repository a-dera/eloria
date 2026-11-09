//! Semantic search: find notes by meaning, not only by words.
//!
//! Each note gets a vector ("embedding") computed by the chosen provider:
//! OpenAI, Voyage AI or Ollama on the computer (nothing leaves the machine). Vectors
//! live in the disposable index `.eloria/index.db` and are only recomputed for notes
//! changed since. Without a configured provider, full-text search remains available.

use std::time::Duration;

use rusqlite::{params, Connection};
use serde_json::{json, Value};

use crate::error::Error;
use crate::settings::Settings;
use crate::Result;

const KEYRING_SERVICE: &str = "com.adera.eloria";

#[derive(Debug, Clone, PartialEq)]
pub struct EmbedConfig {
    /// openai, voyage or ollama
    pub provider: String,
    pub model: String,
    pub url: String,
}

impl EmbedConfig {
    /// `None`: semantic search not configured.
    pub fn from_settings(s: &Settings) -> Option<Self> {
        let p = s.embed_provider.trim().to_lowercase();
        let (default_model, default_url) = match p.as_str() {
            "openai" => ("text-embedding-3-small", "https://api.openai.com/v1/embeddings"),
            "voyage" => ("voyage-3.5", "https://api.voyageai.com/v1/embeddings"),
            "ollama" => ("nomic-embed-text", "http://localhost:11434/v1/embeddings"),
            _ => return None,
        };
        let model = if s.embed_model.trim().is_empty() { default_model.to_string() } else { s.embed_model.trim().to_string() };
        let url = if s.embed_url.trim().is_empty() { default_url.to_string() } else { s.embed_url.trim().to_string() };
        Some(EmbedConfig { provider: p, model, url })
    }

    pub fn needs_key(&self) -> bool {
        self.provider != "ollama"
    }

    /// Identifier stored with each vector: changing the model invalidates the old ones.
    pub fn tag(&self) -> String {
        format!("{}:{}", self.provider, self.model)
    }
}

fn entry(provider: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, &format!("vecteurs-{provider}")).map_err(|e| Error::Ai(format!("trousseau : {e}")))
}

/// API key of the embedding provider (Voyage, OpenAI), in the system keychain.
pub fn set_key(provider: &str, key: &str) -> Result<()> {
    let e = entry(provider)?;
    if key.trim().is_empty() {
        return match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(Error::Ai(format!("trousseau : {err}"))),
        };
    }
    e.set_password(key.trim()).map_err(|err| Error::Ai(format!("trousseau : {err}")))
}

pub fn get_key(provider: &str) -> Result<Option<String>> {
    // OpenAI: the AI assistant's key works too.
    match entry(provider)?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) if provider == "openai" => crate::ai::get_api_key(crate::ai::Provider::Openai),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(Error::Ai(format!("trousseau : {err}"))),
    }
}

/// Vectors of several texts (OpenAI-compatible format, followed by Voyage and Ollama).
pub fn embed(cfg: &EmbedConfig, key: Option<&str>, texts: &[String], query: bool) -> Result<Vec<Vec<f32>>> {
    if cfg.needs_key() && key.is_none() {
        return Err(Error::Ai(format!("aucune clé API enregistrée pour les vecteurs ({})", cfg.provider)));
    }
    let mut body = json!({ "model": cfg.model, "input": texts });
    if cfg.provider == "voyage" {
        body["input_type"] = json!(if query { "query" } else { "document" });
    }
    let agent = crate::net::agent(Duration::from_secs(120));
    let mut r = agent.post(&cfg.url).header("content-type", "application/json");
    if let Some(k) = key {
        r = r.header("authorization", &format!("Bearer {k}"));
    }
    let mut resp = r.send_json(&body).map_err(|e| Error::Net(e.to_string()))?;
    let status = resp.status().as_u16();
    let v: Value = resp.body_mut().read_json().map_err(|e| Error::Net(e.to_string()))?;
    if status >= 400 {
        let msg = v.pointer("/error/message").or_else(|| v.get("detail")).and_then(|m| m.as_str()).unwrap_or("erreur").to_string();
        return Err(Error::Ai(format!("vecteurs ({}) : {status} {msg}", cfg.provider)));
    }
    let data = v.get("data").and_then(|d| d.as_array()).ok_or_else(|| Error::Ai("réponse de vecteurs illisible".into()))?;
    let mut out = vec![Vec::new(); texts.len()];
    for (i, d) in data.iter().enumerate() {
        let idx = d.get("index").and_then(|x| x.as_u64()).map(|x| x as usize).unwrap_or(i);
        let vec: Vec<f32> = d.get("embedding").and_then(|e| e.as_array()).map(|a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect()).unwrap_or_default();
        if idx < out.len() {
            out[idx] = vec;
        }
    }
    if out.iter().any(|v| v.is_empty()) {
        return Err(Error::Ai("réponse de vecteurs incomplète".into()));
    }
    Ok(out)
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0f32, 0f32, 0f32);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

pub fn pack(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

pub fn unpack(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

/// Vector table (created on demand; the index stays disposable).
pub fn ensure_table(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS embeddings(path TEXT PRIMARY KEY, mtime INTEGER NOT NULL, model TEXT NOT NULL, vec BLOB NOT NULL);")?;
    Ok(())
}

/// Notes whose vector is missing or stale (note changed, other model).
pub fn stale(conn: &Connection, notes: &[(String, i64)], tag: &str) -> Result<Vec<String>> {
    ensure_table(conn)?;
    let mut st = conn.prepare("SELECT mtime, model FROM embeddings WHERE path = ?1")?;
    let mut out = Vec::new();
    for (p, m) in notes {
        let row: Option<(i64, String)> = st.query_row(params![p], |r| Ok((r.get(0)?, r.get(1)?))).ok();
        if row.map(|(mt, md)| mt != *m || md != tag).unwrap_or(true) {
            out.push(p.clone());
        }
    }
    Ok(out)
}

pub fn store(conn: &Connection, path: &str, mtime: i64, tag: &str, v: &[f32]) -> Result<()> {
    conn.execute("INSERT OR REPLACE INTO embeddings(path, mtime, model, vec) VALUES (?1, ?2, ?3, ?4)", params![path, mtime, tag, pack(v)])?;
    Ok(())
}

/// Removes the vectors of notes that no longer exist.
pub fn prune(conn: &Connection, existing: &[String]) -> Result<usize> {
    ensure_table(conn)?;
    let keep: std::collections::HashSet<&str> = existing.iter().map(|s| s.as_str()).collect();
    let paths: Vec<String> = conn.prepare("SELECT path FROM embeddings")?.query_map([], |r| r.get(0))?.filter_map(|r| r.ok()).collect();
    let mut n = 0;
    for p in paths.iter().filter(|p| !keep.contains(p.as_str())) {
        conn.execute("DELETE FROM embeddings WHERE path = ?1", params![p])?;
        n += 1;
    }
    Ok(n)
}

/// Notes closest to a query vector (path, score between -1 and 1).
pub fn nearest(conn: &Connection, q: &[f32], tag: &str, k: usize) -> Result<Vec<(String, f32)>> {
    ensure_table(conn)?;
    let mut st = conn.prepare("SELECT path, vec FROM embeddings WHERE model = ?1")?;
    let mut scored: Vec<(String, f32)> = st
        .query_map(params![tag], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?)))?
        .filter_map(|r| r.ok())
        .map(|(p, b)| {
            let s = cosine(q, &unpack(&b));
            (p, s)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);
    Ok(scored)
}

/// Text sent for a note: title, useful properties (abstract of a source), body.
pub fn note_text(title: &str, resume: &str, body: &str) -> String {
    let mut s = format!("{title}\n{resume}\n{body}");
    if s.len() > 8000 {
        let mut cut = 8000;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors_and_similarity() {
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0);
        let v = vec![0.5f32, -1.25, 3.0];
        assert_eq!(unpack(&pack(&v)), v);

        let conn = Connection::open_in_memory().unwrap();
        let notes = vec![("a.md".to_string(), 10), ("b.md".to_string(), 20)];
        assert_eq!(stale(&conn, &notes, "voyage:x").unwrap().len(), 2);
        store(&conn, "a.md", 10, "voyage:x", &[1.0, 0.0]).unwrap();
        store(&conn, "b.md", 20, "voyage:x", &[0.0, 1.0]).unwrap();
        assert!(stale(&conn, &notes, "voyage:x").unwrap().is_empty());
        // Note changed, or model changed: to recompute.
        assert_eq!(stale(&conn, &[("a.md".into(), 11)], "voyage:x").unwrap(), vec!["a.md"]);
        assert_eq!(stale(&conn, &notes, "openai:y").unwrap().len(), 2);
        let near = nearest(&conn, &[0.9, 0.1], "voyage:x", 5).unwrap();
        assert_eq!(near[0].0, "a.md");
        assert_eq!(prune(&conn, &["a.md".to_string()]).unwrap(), 1);
        assert_eq!(nearest(&conn, &[0.0, 1.0], "voyage:x", 5).unwrap().len(), 1);
    }

    #[test]
    fn settings_roundtrip() {
        let mut s = Settings::default();
        assert!(EmbedConfig::from_settings(&s).is_none());
        s.embed_provider = "ollama".into();
        let c = EmbedConfig::from_settings(&s).unwrap();
        assert_eq!((c.model.as_str(), c.needs_key()), ("nomic-embed-text", false));
        assert!(c.url.starts_with("http://localhost:11434"));
        s.embed_provider = "voyage".into();
        s.embed_model = "voyage-3-large".into();
        assert_eq!(EmbedConfig::from_settings(&s).unwrap().tag(), "voyage:voyage-3-large");
    }
}
