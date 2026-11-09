//! AI client: Anthropic (native Messages API) and OpenAI-compatible providers
//! (`/chat/completions`: OpenAI, Mistral, OpenRouter, local Ollama…).
//!
//! - Streamed responses (SSE), forwarded chunk by chunk, cancellable.
//! - API key in the system keychain (Windows Credential Manager),
//!   never in the settings nor in the vault.
//! - No request without an explicit user action; each call is logged
//!   in `.eloria/ia-journal.jsonl` (date, provider, model, tokens), without the content.
//!
//! Anthropic: PDF sent as a `document` block, adjustable effort (`output_config.effort`),
//! server-side fallback on refusal (`fallbacks: "default"`), refusals handled before
//! reading any content. No `temperature` (rejected by API version 2026-10-21).

use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::settings::Settings;
use crate::vault::Vault;
use crate::{Error, Result};

pub const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
pub const ANTHROPIC_VERSION: &str = "2026-10-21";
pub const OPENAI_URL: &str = "https://api.openai.com/v1";
/// Default Anthropic model.
pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-opus-5-5";
/// Server-side fallback, chosen by Anthropic according to the refusal category.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// Keychain service for API keys.
const KEYRING_SERVICE: &str = "com.adera.eloria";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Anthropic,
    /// Official OpenAI API
    Openai,
    /// Any OpenAI-compatible service, at the given address (Ollama, Mistral, OpenRouter…)
    Compatible,
}

impl Provider {
    pub fn from_code(s: &str) -> Self {
        match s {
            "openai" => Provider::Openai,
            "compatible" => Provider::Compatible,
            _ => Provider::Anthropic,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::Openai => "openai",
            Provider::Compatible => "compatible",
        }
    }
}

/// AI settings needed for a call.
#[derive(Debug, Clone)]
pub struct AiConfig {
    pub provider: Provider,
    pub model: String,
    /// Base URL for a compatible provider (e.g. `http://localhost:11434/v1`)
    pub base_url: String,
    /// low, medium, high, xhigh or max (Anthropic)
    pub effort: String,
}

impl AiConfig {
    pub fn from_settings(s: &Settings) -> Self {
        let provider = Provider::from_code(&s.ai_provider);
        let model = if s.ai_model.trim().is_empty() && provider == Provider::Anthropic {
            DEFAULT_ANTHROPIC_MODEL.to_string()
        } else {
            s.ai_model.trim().to_string()
        };
        AiConfig { provider, model, base_url: s.ai_base_url.trim().to_string(), effort: s.ai_effort.clone() }
    }
}

/// Part of the message sent to the model.
#[derive(Debug, Clone)]
pub enum Part {
    Text(String),
    /// Whole PDF. Sent as is to Anthropic; other providers receive
    /// the fallback text (`fallback_text`) instead.
    Pdf { data: Vec<u8>, fallback_text: Option<String> },
}

#[derive(Debug, Clone)]
pub struct AiRequest {
    pub system: String,
    pub parts: Vec<Part>,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AiResponse {
    pub text: String,
    /// Model that actually answered (may differ after a fallback)
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub stop_reason: String,
    /// A refusal was recovered by the fallback model
    pub fell_back: bool,
    /// Response cut by the length limit
    pub truncated: bool,
}

// ------------------------------------------------------------------ API keys

fn entry(provider: Provider) -> Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, &format!("ia-{}", provider.code())).map_err(|e| Error::Ai(format!("trousseau : {e}")))
}

/// Saves (or clears, if empty) a provider's API key.
pub fn set_api_key(provider: Provider, key: &str) -> Result<()> {
    let e = entry(provider)?;
    if key.trim().is_empty() {
        return match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(Error::Ai(format!("trousseau : {err}"))),
        };
    }
    e.set_password(key.trim()).map_err(|err| Error::Ai(format!("trousseau : {err}")))
}

pub fn get_api_key(provider: Provider) -> Result<Option<String>> {
    match entry(provider)?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(Error::Ai(format!("trousseau : {err}"))),
    }
}

// ------------------------------------------------------------ requests

/// Does the model accept the server-side fallback `fallbacks: "default"`?
fn supports_fallback(model: &str) -> bool {
    ["claude-opus-5-5", "claude-opus-5", "claude-sonnet-5-5", "claude-fable-5-1", "claude-fable-5"].contains(&model)
}

/// Anthropic request body (streamed).
pub fn anthropic_body(cfg: &AiConfig, req: &AiRequest) -> Value {
    let mut content = Vec::new();
    for p in &req.parts {
        match p {
            Part::Pdf { data, .. } => content.push(json!({
                "type": "document",
                "source": {
                    "type": "base64",
                    "media_type": "application/pdf",
                    "data": base64::engine::general_purpose::STANDARD.encode(data),
                },
            })),
            Part::Text(t) => content.push(json!({ "type": "text", "text": t })),
        }
    }
    let mut body = json!({
        "model": cfg.model,
        "max_tokens": req.max_tokens,
        "stream": true,
        "system": [{ "type": "text", "text": req.system, "cache_control": { "type": "ephemeral" } }],
        "messages": [{ "role": "user", "content": content }],
    });
    if ["low", "medium", "high", "xhigh", "max"].contains(&cfg.effort.as_str()) {
        body["output_config"] = json!({ "effort": cfg.effort });
    }
    if supports_fallback(&cfg.model) {
        body["fallbacks"] = json!("default");
    }
    body
}

/// OpenAI-compatible request body. PDFs are replaced by their text.
pub fn openai_body(cfg: &AiConfig, req: &AiRequest) -> Value {
    let mut user = String::new();
    for p in &req.parts {
        let t = match p {
            Part::Text(t) => t.as_str(),
            Part::Pdf { fallback_text, .. } => fallback_text.as_deref().unwrap_or(""),
        };
        if !t.is_empty() {
            if !user.is_empty() {
                user.push_str("\n\n");
            }
            user.push_str(t);
        }
    }
    json!({
        "model": cfg.model,
        "stream": true,
        "messages": [
            { "role": "system", "content": req.system },
            { "role": "user", "content": user },
        ],
    })
}

fn agent() -> ureq::Agent {
    // A full reading note can take several minutes to generate.
    crate::net::agent(Duration::from_secs(900))
}

/// Readable error message from a failed HTTP response.
fn http_error(provider: Provider, status: u16, body: &str) -> Error {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.chars().take(300).collect());
    let what = match status {
        401 | 403 => "clé API refusée (vérifie-la dans Réglages › Intelligence artificielle)",
        404 => "modèle ou adresse introuvable",
        413 => "document trop volumineux pour une requête",
        429 => "limite de débit atteinte, réessaie dans un moment",
        500..=599 => "service momentanément indisponible",
        _ => "requête refusée",
    };
    Error::Ai(format!("{} : {what} ({status}) {detail}", provider.code()))
}

/// Sends a request and streams the text to `on_delta`.
/// Setting `cancel` to `true` stops reading.
pub fn complete(
    cfg: &AiConfig,
    api_key: Option<&str>,
    req: &AiRequest,
    on_delta: &mut dyn FnMut(&str),
    cancel: &AtomicBool,
) -> Result<AiResponse> {
    if cfg.model.is_empty() {
        return Err(Error::Ai("aucun modèle choisi".into()));
    }
    let a = agent();
    let resp = match cfg.provider {
        Provider::Anthropic => {
            let key = api_key.filter(|k| !k.is_empty()).ok_or_else(|| Error::Ai("aucune clé API Anthropic enregistrée".into()))?;
            let mut r = a
                .post(ANTHROPIC_URL)
                .header("x-api-key", key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("content-type", "application/json");
            if supports_fallback(&cfg.model) {
                r = r.header("anthropic-beta", FALLBACK_BETA);
            }
            r.send_json(anthropic_body(cfg, req))
        }
        Provider::Openai | Provider::Compatible => {
            let base = if cfg.provider == Provider::Openai || cfg.base_url.is_empty() { OPENAI_URL } else { cfg.base_url.as_str() };
            let mut r = a.post(&format!("{}/chat/completions", base.trim_end_matches('/'))).header("content-type", "application/json");
            if let Some(k) = api_key.filter(|k| !k.is_empty()) {
                r = r.header("authorization", &format!("Bearer {k}"));
            } else if cfg.provider == Provider::Openai {
                return Err(Error::Ai("aucune clé API OpenAI enregistrée".into()));
            }
            r.send_json(openai_body(cfg, req))
        }
    }
    .map_err(|e| Error::Ai(format!("{} injoignable : {e}", cfg.provider.code())))?;

    let status = resp.status().as_u16();
    let mut body = resp.into_body();
    if status != 200 {
        let text = body.read_to_string().unwrap_or_default();
        return Err(http_error(cfg.provider, status, &text));
    }
    let reader = body.into_reader();
    match cfg.provider {
        Provider::Anthropic => read_anthropic_sse(reader, on_delta, cancel),
        _ => read_openai_sse(reader, &cfg.model, on_delta, cancel),
    }
}

/// Reads an SSE stream line by line and passes each JSON `data:` to `handle`.
fn for_each_event(reader: impl Read, cancel: &AtomicBool, mut handle: impl FnMut(&str) -> Result<bool>) -> Result<()> {
    for line in BufReader::new(reader).lines() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Ai("annulé".into()));
        }
        let line = line.map_err(|e| Error::Ai(format!("flux interrompu : {e}")))?;
        let Some(data) = line.strip_prefix("data:") else { continue };
        let data = data.trim();
        if data.is_empty() {
            continue;
        }
        if !handle(data)? {
            break;
        }
    }
    Ok(())
}

/// Parses the SSE stream of Anthropic's Messages API.
pub fn read_anthropic_sse(reader: impl Read, on_delta: &mut dyn FnMut(&str), cancel: &AtomicBool) -> Result<AiResponse> {
    let mut r = AiResponse::default();
    let mut refusal: Option<String> = None;
    for_each_event(reader, cancel, |data| {
        let v: Value = serde_json::from_str(data).map_err(|e| Error::Ai(format!("réponse illisible : {e}")))?;
        match v["type"].as_str().unwrap_or("") {
            "message_start" => {
                r.model = v["message"]["model"].as_str().unwrap_or_default().to_string();
                r.input_tokens = v["message"]["usage"]["input_tokens"].as_u64().unwrap_or(0);
            }
            // A "fallback" block marks the switch to the fallback model (no dedicated event).
            "content_block_start" if v["content_block"]["type"] == "fallback" => r.fell_back = true,
            "content_block_delta" if v["delta"]["type"] == "text_delta" => {
                let t = v["delta"]["text"].as_str().unwrap_or("");
                r.text.push_str(t);
                on_delta(t);
            }
            "message_delta" => {
                if let Some(s) = v["delta"]["stop_reason"].as_str() {
                    r.stop_reason = s.to_string();
                }
                if let Some(o) = v["usage"]["output_tokens"].as_u64() {
                    r.output_tokens = o;
                }
                if r.stop_reason == "refusal" {
                    refusal = Some(v["delta"]["stop_details"]["category"].as_str().unwrap_or("").to_string());
                }
            }
            "error" => {
                let msg = v["error"]["message"].as_str().unwrap_or("erreur inconnue");
                return Err(Error::Ai(format!("anthropic : {msg}")));
            }
            "message_stop" => return Ok(false),
            _ => {}
        }
        Ok(true)
    })?;
    // Refusal: any partial text must not be used as a complete answer.
    if let Some(cat) = refusal {
        let cat = if cat.is_empty() { String::new() } else { format!(" (catégorie : {cat})") };
        return Err(Error::Ai(format!("le modèle a refusé de répondre{cat}")));
    }
    r.truncated = r.stop_reason == "max_tokens";
    Ok(r)
}

/// Parses the SSE stream of `/chat/completions` (OpenAI-compatible).
pub fn read_openai_sse(reader: impl Read, model: &str, on_delta: &mut dyn FnMut(&str), cancel: &AtomicBool) -> Result<AiResponse> {
    let mut r = AiResponse { model: model.to_string(), ..Default::default() };
    for_each_event(reader, cancel, |data| {
        if data == "[DONE]" {
            return Ok(false);
        }
        let v: Value = serde_json::from_str(data).map_err(|e| Error::Ai(format!("réponse illisible : {e}")))?;
        if let Some(msg) = v["error"]["message"].as_str() {
            return Err(Error::Ai(msg.to_string()));
        }
        if let Some(m) = v["model"].as_str() {
            r.model = m.to_string();
        }
        let choice = &v["choices"][0];
        if let Some(t) = choice["delta"]["content"].as_str() {
            r.text.push_str(t);
            on_delta(t);
        }
        if let Some(f) = choice["finish_reason"].as_str() {
            r.stop_reason = f.to_string();
        }
        if let Some(u) = v["usage"].as_object() {
            r.input_tokens = u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0);
            r.output_tokens = u.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0);
        }
        Ok(true)
    })?;
    r.truncated = r.stop_reason == "length";
    Ok(r)
}

// ------------------------------------------------------------- journal

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogEntry {
    pub date: String,
    /// Usage: reading note, summary, question…
    pub task: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub ok: bool,
}

/// Current date in the log format (RFC 3339, local time).
pub fn log_date() -> String {
    chrono::Local::now().to_rfc3339()
}

/// Appends a line to the AI request log (without the content of the exchange).
pub fn log(vault: &Vault, entry: &LogEntry) -> Result<()> {
    use std::io::Write;
    let p = vault.eloria_dir().join("ia-journal.jsonl");
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p)?;
    writeln!(f, "{}", serde_json::to_string(entry)?)?;
    Ok(())
}

/// Latest log entries, most recent first.
pub fn read_log(vault: &Vault, limit: usize) -> Result<Vec<LogEntry>> {
    let p = vault.eloria_dir().join("ia-journal.jsonl");
    if !p.exists() {
        return Ok(vec![]);
    }
    let text = std::fs::read_to_string(p)?;
    Ok(text.lines().rev().filter_map(|l| serde_json::from_str(l).ok()).take(limit).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(provider: Provider, model: &str) -> AiConfig {
        AiConfig { provider, model: model.into(), base_url: String::new(), effort: "high".into() }
    }

    fn req() -> AiRequest {
        AiRequest {
            system: "Tu es un assistant de recherche.".into(),
            parts: vec![Part::Pdf { data: b"%PDF-1.7".to_vec(), fallback_text: Some("texte intégral".into()) }, Part::Text("Fiche ?".into())],
            max_tokens: 64000,
        }
    }

    #[test]
    fn anthropic_body_cases() {
        let b = anthropic_body(&cfg(Provider::Anthropic, "claude-opus-5-5"), &req());
        assert_eq!(b["model"], "claude-opus-5-5");
        assert_eq!(b["stream"], true);
        assert_eq!(b["fallbacks"], "default");
        assert_eq!(b["output_config"]["effort"], "high");
        assert!(b.get("temperature").is_none() && b.get("thinking").is_none());
        let c = &b["messages"][0]["content"];
        assert_eq!(c[0]["type"], "document");
        assert_eq!(c[0]["source"]["media_type"], "application/pdf");
        assert_eq!(c[0]["source"]["data"], "JVBERi0xLjc=");
        assert_eq!(c[1]["text"], "Fiche ?");
        // No server fallback for Haiku
        let h = anthropic_body(&cfg(Provider::Anthropic, "claude-haiku-5-5"), &req());
        assert!(h.get("fallbacks").is_none());
    }

    #[test]
    fn openai_body_without_pdf() {
        let b = openai_body(&cfg(Provider::Compatible, "llama3"), &req());
        assert_eq!(b["messages"][0]["role"], "system");
        assert_eq!(b["messages"][1]["content"], "texte intégral\n\nFiche ?");
    }

    #[test]
    fn anthropic_stream() {
        let sse = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"usage\":{\"input_tokens\":1200}}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"## Problé\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"matique\"}}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":42}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        let mut got = String::new();
        let r = read_anthropic_sse(sse.as_bytes(), &mut |t| got.push_str(t), &AtomicBool::new(false)).unwrap();
        assert_eq!(got, "## Problématique");
        assert_eq!(r.text, got);
        assert_eq!((r.input_tokens, r.output_tokens), (1200, 42));
        assert_eq!(r.model, "claude-opus-5-5");
        assert!(!r.fell_back && !r.truncated);
    }

    #[test]
    fn anthropic_stream_fallback_and_refusal() {
        let repli = "data: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-4-8\",\"usage\":{}}}\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"fallback\"}}\n\
data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\
data: {\"type\":\"message_stop\"}\n";
        let r = read_anthropic_sse(repli.as_bytes(), &mut |_| {}, &AtomicBool::new(false)).unwrap();
        assert!(r.fell_back);
        assert_eq!(r.model, "claude-opus-4-8");

        let refus = "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"début\"}}\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"refusal\",\"stop_details\":{\"category\":\"bio\"}},\"usage\":{\"output_tokens\":3}}\n\
data: {\"type\":\"message_stop\"}\n";
        let e = read_anthropic_sse(refus.as_bytes(), &mut |_| {}, &AtomicBool::new(false)).unwrap_err();
        assert!(e.to_string().contains("refusé") && e.to_string().contains("bio"));

        let err = "data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n";
        assert!(read_anthropic_sse(err.as_bytes(), &mut |_| {}, &AtomicBool::new(false)).unwrap_err().to_string().contains("Overloaded"));

        let stop = AtomicBool::new(true);
        assert!(read_anthropic_sse(repli.as_bytes(), &mut |_| {}, &stop).unwrap_err().to_string().contains("annulé"));
    }

    #[test]
    fn openai_stream() {
        let sse = "data: {\"model\":\"llama3\",\"choices\":[{\"delta\":{\"content\":\"Bon\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\"jour\"},\"finish_reason\":\"length\"}]}\n\n\
data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2}}\n\ndata: [DONE]\n\n";
        let mut got = String::new();
        let r = read_openai_sse(sse.as_bytes(), "x", &mut |t| got.push_str(t), &AtomicBool::new(false)).unwrap();
        assert_eq!(got, "Bonjour");
        assert!(r.truncated);
        assert_eq!((r.input_tokens, r.output_tokens), (10, 2));
    }

    #[test]
    fn log_cases() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        for i in 0..3 {
            let e = LogEntry {
                date: format!("2026-11-0{i}"),
                task: "fiche".into(),
                provider: "anthropic".into(),
                model: "claude-opus-5-5".into(),
                input_tokens: 10,
                output_tokens: 5,
                ok: true,
            };
            log(&v, &e).unwrap();
        }
        let l = read_log(&v, 2).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].date, "2026-11-02");
    }

    #[test]
    fn http_errors() {
        let e = http_error(Provider::Anthropic, 401, "{\"error\":{\"message\":\"invalid x-api-key\"}}").to_string();
        assert!(e.contains("clé API refusée") && e.contains("invalid x-api-key"));
    }
}
