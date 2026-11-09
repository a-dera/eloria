//! Vault watching: a file changed outside Eloria (another editor, Git,
//! cloud sync) is reindexed and the interface is notified.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use eloria_core::Workspace;
use notify::{EventKind, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize)]
pub struct ChangeEvent {
    pub paths: Vec<String>,
    /// `true` if the tree changed (creation, deletion, rename)
    pub structural: bool,
}

fn relevant(p: &Path) -> bool {
    let s = p.to_string_lossy();
    !s.contains(".eloria") && !s.ends_with(".eloria-tmp") && !s.contains("/.git") && !s.contains("\\.git")
}

pub fn start(app: AppHandle, ws: Arc<Mutex<Option<Workspace>>>, root: PathBuf) -> notify::Result<notify::RecommendedWatcher> {
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(ev) = res else { return };
        let paths: Vec<&PathBuf> = ev.paths.iter().filter(|p| relevant(p)).collect();
        if paths.is_empty() {
            return;
        }
        let structural = matches!(ev.kind, EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(notify::event::ModifyKind::Name(_)));
        if !matches!(ev.kind, EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_)) {
            return;
        }
        let mut rels = Vec::new();
        if let Ok(guard) = ws.lock() {
            if let Some(ws) = guard.as_ref() {
                for p in paths {
                    if let Some(rel) = ws.vault.rel_of(p) {
                        let _ = ws.refresh_path(&rel);
                        rels.push(rel);
                    }
                }
            }
        }
        if !rels.is_empty() {
            let _ = app.emit("vault:changed", ChangeEvent { paths: rels, structural });
        }
    })?;
    w.watch(&root, RecursiveMode::Recursive)?;
    Ok(w)
}

/// Watches `zotero.sqlite` (and its WAL journal): when Zotero saves a change
/// (item added, annotation), the interface is notified and resyncs the mirror.
/// A simple polling of timestamps: the Zotero database is never opened here.
pub fn watch_zotero(app: AppHandle) {
    use std::time::{Duration, SystemTime};
    use tauri::Manager;
    std::thread::spawn(move || {
        let stamp = |dir: &Path| -> Option<(SystemTime, Option<SystemTime>)> {
            let db = std::fs::metadata(dir.join("zotero.sqlite")).ok()?.modified().ok()?;
            let wal = std::fs::metadata(dir.join("zotero.sqlite-wal")).ok().and_then(|m| m.modified().ok());
            Some((db, wal))
        };
        let mut last: Option<(SystemTime, Option<SystemTime>)> = None;
        let mut pending: Option<std::time::Instant> = None;
        loop {
            std::thread::sleep(Duration::from_secs(10));
            let state = app.state::<crate::AppState>();
            let settings = match state.settings.lock() {
                Ok(s) => s.clone(),
                Err(_) => continue,
            };
            let open = state.ws.lock().map(|g| g.is_some()).unwrap_or(false);
            let Some(dir) = Workspace::zotero_data_dir(&settings) else { continue };
            let now = stamp(&dir);
            if last.is_some() && now != last {
                // Zotero often writes in several steps: wait for a pause.
                pending = Some(std::time::Instant::now());
            }
            last = now;
            if let Some(t) = pending {
                if t.elapsed() >= Duration::from_secs(8) && open && settings.zotero_auto_sync {
                    let _ = app.emit("zotero:changed", ());
                    pending = None;
                }
            }
        }
    });
}

/// Local API (127.0.0.1): open only when enabled in the settings,
/// closed as soon as it is disabled. A capture lands in the inbox.
pub fn serve_local_api(app: AppHandle) {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;
    use eloria_core::localapi::{self, Action};
    use tauri::Manager;
    std::thread::spawn(move || {
        let mut listener: Option<(TcpListener, u16)> = None;
        loop {
            let state = app.state::<crate::AppState>();
            let settings = match state.settings.lock() {
                Ok(s) => s.clone(),
                Err(_) => continue,
            };
            let wanted = settings.local_api && !settings.local_api_token.is_empty();
            match (&listener, wanted) {
                (Some((_, port)), true) if *port == settings.local_api_port => {}
                (_, true) => {
                    listener = TcpListener::bind(("127.0.0.1", settings.local_api_port)).ok().map(|l| {
                        let _ = l.set_nonblocking(true);
                        (l, settings.local_api_port)
                    });
                }
                (Some(_), false) => listener = None,
                (None, false) => {}
            }
            let Some((l, _)) = &listener else {
                std::thread::sleep(Duration::from_millis(1500));
                continue;
            };
            match l.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 8192];
                    while let Ok(n) = stream.read(&mut chunk) {
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            let need = localapi::content_length(&buf).unwrap_or(0);
                            if buf.len() >= pos + 4 + need || buf.len() > 2_000_000 {
                                break;
                            }
                        }
                    }
                    let reply = match localapi::parse_request(&buf) {
                        None => localapi::response(400, &serde_json::json!({ "error": "requête illisible" })),
                        Some(req) => match localapi::route(&req, &settings.local_api_token) {
                            Action::Reply(code, body) => localapi::response(code, &body),
                            Action::Capture(c) => {
                                let res = state.ws.lock().ok().and_then(|g| g.as_ref().map(|ws| ws.capture(&c.note_text(), Some(c.url.as_str()).filter(|u| !u.is_empty()), &settings)));
                                match res {
                                    Some(Ok(rel)) => {
                                        let _ = app.emit("vault:changed", ChangeEvent { paths: vec![rel.clone()], structural: true });
                                        let _ = app.emit("eloria:captured", rel.clone());
                                        localapi::response(200, &serde_json::json!({ "path": rel }))
                                    }
                                    Some(Err(e)) => localapi::response(400, &serde_json::json!({ "error": e.to_string() })),
                                    None => localapi::response(404, &serde_json::json!({ "error": "aucun coffre ouvert" })),
                                }
                            }
                        },
                    };
                    let _ = stream.write_all(&reply);
                }
                Err(_) => std::thread::sleep(Duration::from_millis(150)),
            }
        }
    });
}
