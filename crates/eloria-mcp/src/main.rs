//! eloria-mcp: MCP server (stdio) for Claude Desktop or any MCP client.
//!
//! Usage: `eloria-mcp <vault path> [--ecriture]`
//! Without `--ecriture`, the server is read-only.
//!
//! Claude Desktop configuration (claude_desktop_config.json):
//! { "mcpServers": { "eloria": { "command": "C:\...\eloria-mcp.exe", "args": ["C:\Users\me\Thesis"] } } }

use std::io::{BufRead, Write};

use eloria_core::mcp::Server;
use eloria_core::settings::Settings;
use eloria_core::Workspace;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let allow_write = args.iter().any(|a| a == "--ecriture" || a == "--write");
    let Some(root) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("usage : eloria-mcp <chemin du coffre> [--ecriture]");
        std::process::exit(2);
    };
    let ws = match Workspace::open(root) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("eloria-mcp : impossible d'ouvrir le coffre {root} : {e}");
            std::process::exit(1);
        }
    };
    // Application settings (language, folders) if they exist.
    let settings = std::env::var_os("APPDATA")
        .map(|d| std::path::PathBuf::from(d).join("com.adera.eloria").join("settings.json"))
        .map(|p| Settings::load(&p))
        .unwrap_or_default();
    let server = Server { ws, settings, allow_write };
    eprintln!("eloria-mcp : coffre {root} ({})", if allow_write { "lecture et écriture" } else { "lecture seule" });

    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        // Some Windows clients (PowerShell 5.1) prefix the input with a byte order mark.
        let line = line.trim_start_matches('\u{feff}');
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<serde_json::Value>(line) {
            Ok(msg) => server.handle(&msg),
            Err(e) => Some(serde_json::json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } })),
        };
        if let Some(r) = reply {
            let _ = writeln!(out, "{r}");
            let _ = out.flush();
        }
    }
}
