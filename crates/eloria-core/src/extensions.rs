//! Vault extensions: `.eloria/extensions/<id>/` with `manifest.json`, `main.js` and,
//! optionally, `styles.css`.
//!
//! An extension only runs if the user approved it for this vault, and
//! only as long as its code has not changed: approval covers the SHA-256
//! fingerprint of the code and stylesheet. A vault received from someone else therefore
//! cannot enable an extension by itself, and a code update requires a new
//! approbation.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::settings::ExtensionGrant;
use crate::{Error, Result};

/// Extensions folder, inside `.eloria/`.
pub const DIR: &str = "extensions";
/// Extension data (settings, state), kept in its folder.
const DATA_FILE: &str = "data.json";
/// Maximum size of an extension's code and data.
const MAX_CODE: u64 = 2 * 1024 * 1024;
const MAX_DATA: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    /// Main JavaScript file (in the extension folder)
    pub main: String,
    /// Minimum Eloria version, e.g. "0.3.0"
    pub min_app_version: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    /// Folder relative to the vault
    pub dir: String,
    /// SHA-256 fingerprint (hexadecimal) of the code and styles
    pub hash: String,
    pub has_styles: bool,
    /// Approved for this vault, code unchanged
    pub enabled: bool,
    /// Approved, but the code changed since: to approve again
    pub changed: bool,
    /// Unreadable manifest, invalid identifier, Eloria version too old…
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExtensionCode {
    pub id: String,
    pub name: String,
    pub version: String,
    pub code: String,
    pub css: Option<String>,
}

/// Identifier: lower-case letters, digits and hyphens, 2 to 48 characters.
pub fn valid_id(id: &str) -> bool {
    let b = id.as_bytes();
    (2..=48).contains(&b.len())
        && b[0].is_ascii_alphanumeric()
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// Plain file name, no folder or parent reference.
fn plain_file(name: &str, ext: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', ':']) && name != ".." && !name.starts_with('.') && name.ends_with(ext)
}

fn version_tuple(v: &str) -> (u32, u32, u32) {
    let mut it = v.trim().trim_start_matches('v').split('.').map(|x| x.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

fn read_limited(p: &Path) -> Result<String> {
    let len = fs::metadata(p)?.len();
    if len > MAX_CODE {
        return Err(Error::Extension(format!("{} : fichier trop gros ({} Ko)", p.display(), len / 1024)));
    }
    Ok(fs::read_to_string(p)?)
}

pub fn hash(code: &str, css: Option<&str>) -> String {
    let mut h = Sha256::new();
    h.update(code.as_bytes());
    h.update([0u8]);
    h.update(css.unwrap_or("").as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn load_manifest(dir: &Path) -> Result<Manifest> {
    let raw = fs::read_to_string(dir.join("manifest.json")).map_err(|_| Error::Extension("manifest.json absent".into()))?;
    let mut m: Manifest = serde_json::from_str(&raw).map_err(|e| Error::Extension(format!("manifest.json illisible : {e}")))?;
    if m.main.is_empty() {
        m.main = "main.js".into();
    }
    if m.name.is_empty() {
        m.name = m.id.clone();
    }
    Ok(m)
}

/// Code and styles of an extension, after checking the manifest.
fn read_parts(dir: &Path, folder: &str, app_version: &str) -> Result<(Manifest, String, Option<String>)> {
    let m = load_manifest(dir)?;
    if !valid_id(&m.id) {
        return Err(Error::Extension(format!("identifiant invalide : « {} » (minuscules, chiffres, tirets)", m.id)));
    }
    if m.id != folder {
        return Err(Error::Extension(format!("l'identifiant « {} » doit être le nom du dossier « {folder} »", m.id)));
    }
    if !plain_file(&m.main, ".js") {
        return Err(Error::Extension(format!("fichier principal invalide : « {} »", m.main)));
    }
    if !m.min_app_version.is_empty() && version_tuple(&m.min_app_version) > version_tuple(app_version) {
        return Err(Error::Extension(format!("demande Eloria {} ou plus récent", m.min_app_version)));
    }
    let code = read_limited(&dir.join(&m.main)).map_err(|_| Error::Extension(format!("{} introuvable", m.main)))?;
    let css_path = dir.join("styles.css");
    let css = if css_path.is_file() { Some(read_limited(&css_path)?) } else { None };
    Ok((m, code, css))
}

fn grant_state(grants: &[ExtensionGrant], vault: &str, id: &str, h: &str) -> (bool, bool) {
    match grants.iter().find(|g| g.vault == vault && g.id == id) {
        Some(g) if g.hash == h => (true, false),
        Some(_) => (false, true),
        None => (false, false),
    }
}

/// Extensions found in `<eloria_dir>/extensions`, sorted by name.
pub fn scan(eloria_dir: &Path, vault: &str, grants: &[ExtensionGrant], app_version: &str) -> Vec<ExtensionInfo> {
    let root = eloria_dir.join(DIR);
    let Ok(rd) = fs::read_dir(&root) else { return vec![] };
    let mut out = vec![];
    for e in rd.flatten() {
        let dir = e.path();
        if !dir.is_dir() {
            continue;
        }
        let folder = e.file_name().to_string_lossy().to_string();
        let rel = format!("{}/{DIR}/{folder}", crate::ELORIA_DIR);
        let mut info = ExtensionInfo { id: folder.clone(), name: folder.clone(), version: String::new(), description: String::new(), author: String::new(), dir: rel, hash: String::new(), has_styles: false, enabled: false, changed: false, error: None };
        if let Ok(m) = load_manifest(&dir) {
            info.name = m.name;
            info.version = m.version;
            info.description = m.description;
            info.author = m.author;
        }
        match read_parts(&dir, &folder, app_version) {
            Ok((_, code, css)) => {
                info.hash = hash(&code, css.as_deref());
                info.has_styles = css.is_some();
                (info.enabled, info.changed) = grant_state(grants, vault, &folder, &info.hash);
            }
            Err(err) => info.error = Some(err.to_string()),
        }
        out.push(info);
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn ext_dir(eloria_dir: &Path, id: &str) -> Result<PathBuf> {
    if !valid_id(id) {
        return Err(Error::Extension(format!("identifiant d'extension invalide : {id}")));
    }
    Ok(eloria_dir.join(DIR).join(id))
}

/// Code of an approved extension; refused if not approved or if the code changed.
pub fn load(eloria_dir: &Path, vault: &str, grants: &[ExtensionGrant], id: &str, app_version: &str) -> Result<ExtensionCode> {
    let dir = ext_dir(eloria_dir, id)?;
    let (m, code, css) = read_parts(&dir, id, app_version)?;
    let h = hash(&code, css.as_deref());
    match grant_state(grants, vault, id, &h) {
        (true, _) => Ok(ExtensionCode { id: m.id, name: m.name, version: m.version, code, css }),
        (false, true) => Err(Error::Extension(format!("le code de l'extension « {} » a changé depuis son activation : à réactiver", m.name))),
        _ => Err(Error::Extension(format!("l'extension « {} » n'est pas activée pour ce coffre", m.name))),
    }
}

/// Approves (or revokes) an extension for this vault, on its current code.
pub fn set_grant(eloria_dir: &Path, vault: &str, grants: &mut Vec<ExtensionGrant>, id: &str, on: bool, app_version: &str) -> Result<()> {
    grants.retain(|g| !(g.vault == vault && g.id == id));
    if on {
        let dir = ext_dir(eloria_dir, id)?;
        let (_, code, css) = read_parts(&dir, id, app_version)?;
        grants.push(ExtensionGrant { vault: vault.into(), id: id.into(), hash: hash(&code, css.as_deref()) });
    }
    Ok(())
}

pub fn data_get(eloria_dir: &Path, id: &str) -> Result<serde_json::Value> {
    let p = ext_dir(eloria_dir, id)?.join(DATA_FILE);
    match fs::read_to_string(&p) {
        Ok(s) => Ok(serde_json::from_str(&s).unwrap_or(serde_json::Value::Null)),
        Err(_) => Ok(serde_json::Value::Null),
    }
}

pub fn data_set(eloria_dir: &Path, id: &str, value: &serde_json::Value) -> Result<()> {
    let dir = ext_dir(eloria_dir, id)?;
    if !dir.is_dir() {
        return Err(Error::Extension(format!("extension inconnue : {id}")));
    }
    let s = serde_json::to_string_pretty(value)?;
    if s.len() > MAX_DATA {
        return Err(Error::Extension("données d'extension trop volumineuses (1 Mo au plus)".into()));
    }
    let tmp = dir.join(format!("{DATA_FILE}.tmp"));
    fs::write(&tmp, s)?;
    fs::rename(tmp, dir.join(DATA_FILE))?;
    Ok(())
}

/// Identifier derived from a name: "Word counter!" -> "word-counter".
pub fn slug(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let mut s = String::new();
    for c in name.nfd().filter(|c| c.is_ascii()) {
        if c.is_ascii_alphanumeric() {
            s.push(c.to_ascii_lowercase());
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    let s: String = s.trim_end_matches('-').chars().take(48).collect();
    s.trim_end_matches('-').to_string()
}

/// Skeleton of a new extension (manifest, commented sample code).
pub fn scaffold(eloria_dir: &Path, name: &str, app_version: &str) -> Result<String> {
    let id = slug(name);
    if !valid_id(&id) {
        return Err(Error::Extension(format!("nom d'extension invalide : « {name} »")));
    }
    let dir = eloria_dir.join(DIR).join(&id);
    if dir.exists() {
        return Err(Error::Extension(format!("l'extension « {id} » existe déjà")));
    }
    fs::create_dir_all(&dir)?;
    let m = Manifest { id: id.clone(), name: name.trim().into(), version: "0.1.0".into(), description: String::new(), author: String::new(), main: "main.js".into(), min_app_version: app_version.into() };
    fs::write(dir.join("manifest.json"), serde_json::to_string_pretty(&m)?)?;
    fs::write(dir.join("main.js"), SCAFFOLD_JS.replace("{{name}}", name.trim()))?;
    Ok(format!("{}/{DIR}/{id}", crate::ELORIA_DIR))
}

const SCAFFOLD_JS: &str = r#"// "{{name}}" extension for Eloria.
// Documentation: docs/extensions.md. The `eloria` object gives access to notes,
// the editor, the palette and the status bar. The developer console
// (palette: "Developer console") shows the messages of `eloria.log`.

eloria.commands.add({
  id: "count-words",
  name: "{{name}}: count the words of the note",
  run: async () => {
    const path = eloria.notes.active();
    if (!path) return eloria.ui.toast("Open a note first.");
    const text = await eloria.notes.read(path);
    const words = text.split(/\s+/).filter(Boolean).length;
    eloria.ui.toast(`${words} words in ${path}`);
    eloria.log("count", path, words);
  },
});

// Status bar item, updated on every opened note.
const item = eloria.ui.statusItem({ text: "", title: "{{name}}" });
eloria.events.on("note-open", (path) => item.set(path ? path.split("/").pop() : ""));

// Called when disabled or reloaded.
eloria.onunload(() => eloria.log("goodbye"));
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn write_ext(root: &Path, folder: &str, manifest: &str, code: &str) {
        let d = root.join(DIR).join(folder);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("manifest.json"), manifest).unwrap();
        fs::write(d.join("main.js"), code).unwrap();
    }

    #[test]
    fn identifiers_and_file_names() {
        assert!(valid_id("compteur-de-mots"));
        assert!(!valid_id("Compteur"));
        assert!(!valid_id("../x"));
        assert!(!valid_id("a"));
        assert!(plain_file("main.js", ".js"));
        assert!(!plain_file("../main.js", ".js"));
        assert!(!plain_file("sub/main.js", ".js"));
        assert!(!plain_file("main.ts", ".js"));
        assert_eq!(slug("Compteur de mots !"), "compteur-de-mots");
        assert_eq!(slug("Élément clé"), "element-cle");
        assert_eq!(version_tuple("v1.10.2"), (1, 10, 2));
        assert!(version_tuple("0.10.0") > version_tuple("0.9.9"));
    }

    #[test]
    fn approval_bound_to_code_and_vault() {
        let d = tempfile::tempdir().unwrap();
        let el = d.path();
        write_ext(el, "bonjour", r#"{"id":"bonjour","name":"Bonjour","version":"1.0.0"}"#, "eloria.log('a')");
        write_ext(el, "mauvais", r#"{"id":"autre","name":"Mauvais"}"#, "");
        write_ext(el, "futur", r#"{"id":"futur","min_app_version":"9.0.0"}"#, "");
        write_ext(el, "evasion", r#"{"id":"evasion","main":"../../secret.js"}"#, "");
        let mut grants = vec![];
        let list = scan(el, "/coffre", &grants, "0.9.0");
        assert_eq!(list.len(), 4);
        let b = list.iter().find(|x| x.id == "bonjour").unwrap();
        assert!(!b.enabled && b.error.is_none() && b.hash.len() == 64);
        assert!(list.iter().find(|x| x.id == "mauvais").unwrap().error.as_deref().unwrap().contains("nom du dossier"));
        assert!(list.iter().find(|x| x.id == "futur").unwrap().error.as_deref().unwrap().contains("9.0.0"));
        assert!(list.iter().find(|x| x.id == "evasion").unwrap().error.as_deref().unwrap().contains("invalide"));

        // Not approved: the code is not served.
        assert!(load(el, "/coffre", &grants, "bonjour", "0.9.0").is_err());
        set_grant(el, "/coffre", &mut grants, "bonjour", true, "0.9.0").unwrap();
        assert_eq!(load(el, "/coffre", &grants, "bonjour", "0.9.0").unwrap().code, "eloria.log('a')");
        // Another vault: no approval.
        assert!(load(el, "/autre", &grants, "bonjour", "0.9.0").is_err());
        assert!(set_grant(el, "/coffre", &mut grants, "mauvais", true, "0.9.0").is_err());

        // Code changed: disabled until approved again.
        fs::write(el.join(DIR).join("bonjour").join("main.js"), "eloria.log('b')").unwrap();
        let err = load(el, "/coffre", &grants, "bonjour", "0.9.0").unwrap_err().to_string();
        assert!(err.contains("a changé"), "{err}");
        let b = scan(el, "/coffre", &grants, "0.9.0").into_iter().find(|x| x.id == "bonjour").unwrap();
        assert!(b.changed && !b.enabled);
        // An added stylesheet also changes the fingerprint.
        set_grant(el, "/coffre", &mut grants, "bonjour", true, "0.9.0").unwrap();
        fs::write(el.join(DIR).join("bonjour").join("styles.css"), ".x{}").unwrap();
        assert!(load(el, "/coffre", &grants, "bonjour", "0.9.0").is_err());
        set_grant(el, "/coffre", &mut grants, "bonjour", true, "0.9.0").unwrap();
        assert_eq!(load(el, "/coffre", &grants, "bonjour", "0.9.0").unwrap().css.as_deref(), Some(".x{}"));
        set_grant(el, "/coffre", &mut grants, "bonjour", false, "0.9.0").unwrap();
        assert!(grants.is_empty());
    }

    #[test]
    fn data_and_skeleton() {
        let d = tempfile::tempdir().unwrap();
        let el = d.path();
        let rel = scaffold(el, "Compteur de mots", "0.9.0").unwrap();
        assert_eq!(rel, ".eloria/extensions/compteur-de-mots");
        assert!(scaffold(el, "Compteur de mots", "0.9.0").is_err());
        let list = scan(el, "/c", &[], "0.9.0");
        assert_eq!(list[0].name, "Compteur de mots");
        assert!(list[0].error.is_none());
        assert_eq!(data_get(el, "compteur-de-mots").unwrap(), serde_json::Value::Null);
        data_set(el, "compteur-de-mots", &serde_json::json!({"total": 3})).unwrap();
        assert_eq!(data_get(el, "compteur-de-mots").unwrap()["total"], 3);
        assert!(data_set(el, "inconnue", &serde_json::json!({})).is_err());
        assert!(data_set(el, "../x", &serde_json::json!({})).is_err());
    }
}
