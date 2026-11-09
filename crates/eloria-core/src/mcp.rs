//! MCP (Model Context Protocol) server: an assistant such as Claude Desktop queries the
//! vault (search, notes, sources, sourced excerpts). Read-only by default;
//! writing (note creation) only exists if the user allows it (`--ecriture`).
//!
//! Transport: JSON-RPC 2.0, one message per line on standard input and output.
//! This part only handles a message; the `eloria-mcp` executable reads and writes.

use serde_json::{json, Value};

use crate::settings::Settings;
use crate::workspace::Workspace;

pub const PROTOCOL: &str = "2025-06-18";

pub struct Server {
    pub ws: Workspace,
    pub settings: Settings,
    pub allow_write: bool,
}

fn tool(name: &str, description: &str, props: Value, required: &[&str]) -> Value {
    json!({ "name": name, "description": description, "inputSchema": { "type": "object", "properties": props, "required": required } })
}

impl Server {
    pub fn tools(&self) -> Vec<Value> {
        let mut v = vec![
            tool(
                "search_notes",
                "Full-text search in the Eloria vault (Markdown notes: reading notes, sources, concepts, thesis chapters). Returns paths, titles and matching snippets.",
                json!({ "query": { "type": "string", "description": "Words to search; \"exact phrase\", #tag, -exclude" }, "limit": { "type": "integer", "default": 20 } }),
                &["query"],
            ),
            tool("read_note", "Read a note of the vault (Markdown with its YAML properties).", json!({ "path": { "type": "string", "description": "Path relative to the vault, e.g. Sources/@smith2024graph.md" } }), &["path"]),
            tool(
                "find_passages",
                "Retrieve the most relevant excerpts from the vault and the Zotero library for a research question, each with its source note and citation key. Use these to answer with precise references; never cite anything outside them.",
                json!({ "question": { "type": "string" }, "limit": { "type": "integer", "default": 10 } }),
                &["question"],
            ),
            tool(
                "list_sources",
                "List bibliographic sources (Zotero library and sources added in Eloria), optionally filtered by words in title, authors, year or citation key.",
                json!({ "query": { "type": "string" }, "limit": { "type": "integer", "default": 30 } }),
                &[],
            ),
            tool("get_source", "Full record of a source by citation key: metadata, abstract, annotations and attached reading notes.", json!({ "citekey": { "type": "string" } }), &["citekey"]),
            tool("get_links", "Incoming and outgoing links of a note.", json!({ "path": { "type": "string" } }), &["path"]),
            tool("list_tags", "All tags of the vault with their number of notes.", json!({}), &[]),
        ];
        if self.allow_write {
            v.push(tool(
                "create_note",
                "Create a new Markdown note in the vault (never overwrites: a number is added if the name exists).",
                json!({ "folder": { "type": "string", "default": "" }, "title": { "type": "string" }, "content": { "type": "string" } }),
                &["title", "content"],
            ));
        }
        v
    }

    /// Handles a JSON-RPC message; `None` for a notification (no response).
    pub fn handle(&self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or_default();
        let id = id?;
        let result = match method {
            "initialize" => {
                let version = msg.pointer("/params/protocolVersion").and_then(|v| v.as_str()).unwrap_or(PROTOCOL);
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "eloria", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "Eloria is a doctoral researcher's vault (notes, reading notes, Zotero sources). Answer from find_passages and cite sources with their citation keys like [@key]; never invent references."
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": self.tools() })),
            "tools/call" => {
                let name = msg.pointer("/params/name").and_then(|v| v.as_str()).unwrap_or_default();
                let args = msg.pointer("/params/arguments").cloned().unwrap_or(json!({}));
                Ok(match self.call(name, &args) {
                    Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
                    Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
                })
            }
            _ => Err(json!({ "code": -32601, "message": format!("méthode inconnue : {method}") })),
        };
        Some(match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
        })
    }

    fn call(&self, name: &str, a: &Value) -> std::result::Result<String, String> {
        let s = |k: &str| a.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let n = |k: &str, d: usize| a.get(k).and_then(|v| v.as_u64()).map(|x| x as usize).unwrap_or(d).clamp(1, 100);
        let e = |x: crate::Error| x.to_string();
        let out = match name {
            "search_notes" => serde_json::to_string_pretty(&self.ws.index.search(&s("query"), n("limit", 20)).map_err(e)?),
            "read_note" => return self.ws.vault.read(&s("path")).map_err(e),
            "find_passages" => serde_json::to_string_pretty(&self.ws.ask_passages(&s("question"), &self.settings, n("limit", 10)).map_err(e)?),
            "list_sources" => {
                let q = crate::assist::fold(&s("query"));
                let items: Vec<Value> = self
                    .ws
                    .library_items()
                    .map_err(e)?
                    .into_iter()
                    .filter(|i| q.is_empty() || crate::assist::fold(&format!("{} {} {} {:?}", i.title, i.citekey, crate::zotero::authors_short(&i.creators), i.year)).contains(&q))
                    .take(n("limit", 30))
                    .map(|i| json!({ "citekey": i.citekey, "title": i.title, "authors": crate::zotero::authors_short(&i.creators), "year": i.year, "doi": i.doi }))
                    .collect();
                serde_json::to_string_pretty(&items)
            }
            "get_source" => {
                let ck = s("citekey").trim_start_matches('@').to_string();
                let path = self.ws.fiches_map().map_err(e)?.get(&ck.to_lowercase()).cloned();
                let (item, anns) = match &path {
                    Some(p) => {
                        let d = self.ws.source_detail(p).map_err(e)?;
                        (d.item, d.annotations)
                    }
                    None => match self.ws.zotero_item_by_citekey(&ck).map_err(e)? {
                        Some(d) => (d.item, d.annotations),
                        None => return Err(format!("source @{ck} introuvable")),
                    },
                };
                let fiches = path.as_deref().map(|p| self.ws.fiches_of_source(p).unwrap_or_default()).unwrap_or_default();
                serde_json::to_string_pretty(&json!({
                    "citekey": item.citekey, "title": item.title, "authors": item.creators.iter().map(|c| c.display()).collect::<Vec<_>>(),
                    "year": item.year, "publication": item.publication, "doi": item.doi, "abstract": item.abstract_note,
                    "source_note": path, "reading_notes": fiches,
                    "annotations": anns.iter().map(|a| json!({ "page": a.page_label, "text": a.text, "comment": a.comment })).collect::<Vec<_>>(),
                }))
            }
            "get_links" => {
                let p = s("path");
                serde_json::to_string_pretty(&json!({ "backlinks": self.ws.index.backlinks(&p).map_err(e)?, "outgoing": self.ws.index.outgoing(&p).map_err(e)? }))
            }
            "list_tags" => serde_json::to_string_pretty(&self.ws.index.tags().map_err(e)?),
            "create_note" if self.allow_write => {
                let rel = self.ws.create_note(&s("folder"), &s("title"), &s("content")).map_err(e)?;
                return Ok(format!("note créée : {rel}"));
            }
            "create_note" => return Err("écriture non autorisée : relance eloria-mcp avec --ecriture".into()),
            _ => return Err(format!("outil inconnu : {name}")),
        };
        out.map_err(|x| x.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(write: bool) -> (tempfile::TempDir, Server) {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::open(d.path()).unwrap();
        ws.create_note("Sources", "@a2020", "---\ntype: source\ntitre: Fraude mobile\nauteurs: [Ana Diallo]\nannee: 2020\ncitekey: a2020\nresume: Détection de la fraude au mobile money.\n---\n# Fraude mobile").unwrap();
        ws.create_note("Concepts", "Mule", "# Mule\n\nUn compte mule fait transiter la fraude. Voir [[@a2020]].").unwrap();
        (d, Server { ws, settings: Settings::default(), allow_write: write })
    }

    fn call(s: &Server, name: &str, args: Value) -> Value {
        s.handle(&json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": name, "arguments": args } })).unwrap()
    }

    #[test]
    fn protocol_and_tools() {
        let (_d, s) = server(false);
        let init = s.handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } })).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(init["result"]["serverInfo"]["name"], "eloria");
        assert!(s.handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());
        let tools = s.handle(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).unwrap();
        let names: Vec<&str> = tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"find_passages") && !names.contains(&"create_note"));
        assert!(call(&s, "search_notes", json!({ "query": "mule" }))["result"]["content"][0]["text"].as_str().unwrap().contains("Concepts/Mule.md"));
        assert!(call(&s, "read_note", json!({ "path": "Concepts/Mule.md" }))["result"]["content"][0]["text"].as_str().unwrap().contains("compte mule"));
        let src = call(&s, "get_source", json!({ "citekey": "@a2020" }));
        assert!(src["result"]["content"][0]["text"].as_str().unwrap().contains("Détection de la fraude"));
        let p = call(&s, "find_passages", json!({ "question": "fraude mobile money" }));
        assert!(p["result"]["content"][0]["text"].as_str().unwrap().contains("a2020"));
        // Read-only: writing is refused.
        let w = call(&s, "create_note", json!({ "title": "X", "content": "y" }));
        assert_eq!(w["result"]["isError"], true);
        // Path outside the vault: refused.
        assert_eq!(call(&s, "read_note", json!({ "path": "../secret.txt" }))["result"]["isError"], true);
        assert_eq!(s.handle(&json!({ "jsonrpc": "2.0", "id": 3, "method": "inconnue" })).unwrap()["error"]["code"], -32601);
    }

    #[test]
    fn writing_allowed() {
        let (_d, s) = server(true);
        let r = call(&s, "create_note", json!({ "folder": "Boîte de réception", "title": "Idée", "content": "# Idée" }));
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("Boîte de réception/Idée.md"));
    }
}
