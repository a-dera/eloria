//! Local API (127.0.0.1 only, protected by a token): web capture from the
//! browser ("Send to Eloria" bookmarklet) and other tools on the machine. This part
//! reads an HTTP request and decides the response; the server itself runs in the application.

use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// Parses a complete HTTP/1.1 request (headers and body).
pub fn parse_request(raw: &[u8]) -> Option<Request> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text.split_once("\r\n\r\n")?;
    let mut lines = head.lines();
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?.to_string();
    let path = first.next()?.split('?').next()?.to_string();
    let headers = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    Some(Request { method, path, headers, body: body.to_string() })
}

/// Announced body length (to know when the request is complete).
pub fn content_length(raw: &[u8]) -> Option<usize> {
    let text = String::from_utf8_lossy(raw);
    let head = text.split("\r\n\r\n").next()?;
    head.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case("content-length").then(|| v.trim().parse().ok())?
    })
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct CaptureBody {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub text: String,
}

impl CaptureBody {
    /// Text of the captured note: title, selection as a quote, address.
    pub fn note_text(&self) -> String {
        let mut s = String::new();
        if !self.title.trim().is_empty() {
            s.push_str(&format!("# {}\n\n", self.title.trim()));
        }
        if !self.text.trim().is_empty() {
            for l in self.text.trim().lines() {
                s.push_str(&format!("> {l}\n"));
            }
            s.push('\n');
        }
        if !self.url.trim().is_empty() {
            s.push_str(&format!("<{}>\n", self.url.trim()));
        }
        s
    }
}

/// What the API asks the application to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Reply right away (status, JSON body)
    Reply(u16, Value),
    /// Capture into the inbox
    Capture(CaptureBody),
}

/// Decides the response. The token is required everywhere except for /ping.
pub fn route(req: &Request, token: &str) -> Action {
    if req.method == "OPTIONS" {
        return Action::Reply(204, Value::Null);
    }
    if req.path == "/ping" {
        return Action::Reply(200, json!({ "app": "eloria", "version": env!("CARGO_PKG_VERSION") }));
    }
    let given = req.header("authorization").and_then(|v| v.strip_prefix("Bearer ")).unwrap_or_default();
    if token.is_empty() || !constant_eq(given, token) {
        return Action::Reply(401, json!({ "error": "jeton absent ou invalide" }));
    }
    match (req.method.as_str(), req.path.as_str()) {
        ("POST", "/capture") => match serde_json::from_str::<CaptureBody>(&req.body) {
            Ok(b) if !(b.url.trim().is_empty() && b.text.trim().is_empty() && b.title.trim().is_empty()) => Action::Capture(b),
            _ => Action::Reply(400, json!({ "error": "corps attendu : {url, title, text}" })),
        },
        _ => Action::Reply(404, json!({ "error": "inconnu" })),
    }
}

fn constant_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Complete HTTP response, with the headers that allow a call from a web page
/// to 127.0.0.1 (CORS and private network access).
pub fn response(status: u16, body: &Value) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        _ => "Not Found",
    };
    let payload = if status == 204 { String::new() } else { body.to_string() };
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Authorization, Content-Type\r\nAccess-Control-Allow-Private-Network: true\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    )
    .into_bytes()
}

/// "Send to Eloria" bookmarklet: the page (title, address) and any selection.
pub fn bookmarklet(port: u16, token: &str) -> String {
    format!(
        "javascript:(()=>{{fetch('http://127.0.0.1:{port}/capture',{{method:'POST',headers:{{'Authorization':'Bearer {token}','Content-Type':'application/json'}},body:JSON.stringify({{url:location.href,title:document.title,text:String(getSelection())}})}}).then(r=>alert(r.ok?'Envoyé à Eloria':'Eloria : '+r.status)).catch(()=>alert('Eloria n\\'est pas ouvert'))}})()"
    )
}

/// Random token (32 hexadecimal characters) drawn from the clock and a memory address.
pub fn new_token() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for _ in 0..2 {
        let mut h = RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_and_routes() {
        let body = r#"{"url":"https://ex.org/a","title":"Article","text":"Un passage.\nSuite."}"#;
        let raw = format!("POST /capture HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer abc\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        assert_eq!(content_length(raw.as_bytes()), Some(body.len()));
        let req = parse_request(raw.as_bytes()).unwrap();
        assert_eq!((req.method.as_str(), req.path.as_str()), ("POST", "/capture"));
        match route(&req, "abc") {
            Action::Capture(b) => assert_eq!(b.note_text(), "# Article\n\n> Un passage.\n> Suite.\n\n<https://ex.org/a>\n"),
            other => panic!("{other:?}"),
        }
        // Wrong token, no token configured: refused.
        assert_eq!(route(&req, "autre"), Action::Reply(401, json!({ "error": "jeton absent ou invalide" })));
        assert!(matches!(route(&req, ""), Action::Reply(401, _)));
        // /ping without a token, OPTIONS (browser preflight) accepted.
        let ping = parse_request(b"GET /ping?x=1 HTTP/1.1\r\n\r\n").unwrap();
        assert!(matches!(route(&ping, "abc"), Action::Reply(200, _)));
        let opt = parse_request(b"OPTIONS /capture HTTP/1.1\r\n\r\n").unwrap();
        assert_eq!(route(&opt, "abc"), Action::Reply(204, Value::Null));
        let r = String::from_utf8(response(204, &Value::Null)).unwrap();
        assert!(r.contains("Access-Control-Allow-Private-Network: true") && r.ends_with("\r\n\r\n"));
        assert!(bookmarklet(27123, "abc").contains("Bearer abc") && new_token().len() == 32);
    }
}
