//! Custom themes: CSS files in the vault's `.eloria/themes/`.
//!
//! At the top of the file, an optional comment gives the name and scheme:
//! `/* name: My theme ; scheme: dark */`. Without a scheme, the theme is light.

use std::fs;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::vault::Vault;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomTheme {
    /// File name without extension (stable identifier)
    pub id: String,
    pub name: String,
    /// "light" or "dark"
    pub scheme: String,
    pub css: String,
}

/// Sample written by "Create a sample theme": every useful variable, commented.
pub const EXAMPLE: &str = r#"/* name: Mon thème ; scheme: dark */
/* Eloria custom theme. Change the colours, save, then "Reload" in
   Settings › Appearance. Every variable is optional: missing ones
   come from the base light or dark theme, according to "scheme" above. */
:root {
  --accent: #e0a542;          /* links, active line, linked items */
  --bg: #1b1a22;              /* editor background */
  --bg-soft: #16151c;         /* side panels */
  --bg-sunk: #111017;         /* zones en creux */
  --bg-raised: #23222c;       /* menus, windows */
  --line: #2e2c38;            /* separators */
  --line-strong: #3d3a48;
  --text: #ece8f4;
  --text-soft: #b4adc2;
  --text-faint: #7d7689;
  --mark: #5e4a1a;            /* surlignage ==texte== */
  --code-bg: #15141b;
  --syn-keyword: #f28fad;     /* code highlighting */
  --syn-string: #a6da95;
  --syn-number: #f5a97f;
  --syn-fn: #8aadf4;
  --syn-type: #eed49f;
  --syn-prop: #c6a0f6;
  --font-editor: Georgia, serif;
}

/* Any other CSS setting is possible, for example: */
.cm-content .cm-h1 {
  letter-spacing: 0.01em;
}
"#;

fn header_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)\A\s*/\*(.*?)\*/").unwrap())
}

fn field(header: &str, key: &str) -> Option<String> {
    header.split([';', '\n']).find_map(|part| {
        let (k, v) = part.split_once(':')?;
        (k.trim().eq_ignore_ascii_case(key)).then(|| v.trim().to_string()).filter(|v| !v.is_empty())
    })
}

/// Reads a theme from its CSS content.
pub fn parse(id: &str, css: &str) -> CustomTheme {
    let header = header_re().captures(css).map(|c| c[1].to_string()).unwrap_or_default();
    let scheme = field(&header, "scheme").map(|s| s.to_lowercase()).filter(|s| s == "dark").unwrap_or_else(|| "light".into());
    CustomTheme { id: id.to_string(), name: field(&header, "name").unwrap_or_else(|| id.to_string()), scheme, css: css.to_string() }
}

/// Themes of the vault, sorted by name.
pub fn list(vault: &Vault) -> Result<Vec<CustomTheme>> {
    let dir = vault.eloria_dir().join("themes");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.extension().map(|x| x.eq_ignore_ascii_case("css")).unwrap_or(false) {
            let id = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            out.push(parse(&id, &fs::read_to_string(&p)?));
        }
    }
    out.sort_by_key(|t| t.name.to_lowercase());
    Ok(out)
}

/// Creates the sample file (without overwriting an existing file) and returns its relative path.
pub fn create_example(vault: &Vault) -> Result<String> {
    let dir = vault.eloria_dir().join("themes");
    fs::create_dir_all(&dir)?;
    let mut name = "Mon thème".to_string();
    let mut i = 1;
    while dir.join(format!("{name}.css")).exists() {
        i += 1;
        name = format!("Mon thème {i}");
    }
    fs::write(dir.join(format!("{name}.css")), EXAMPLE.replacen("name: Mon thème", &format!("name: {name}"), 1))?;
    Ok(format!("{}/themes/{name}.css", crate::ELORIA_DIR))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_parsing() {
        let t = parse("x", "/* name: Nuit ; scheme: dark */\n:root{--bg:#000}");
        assert_eq!((t.name.as_str(), t.scheme.as_str()), ("Nuit", "dark"));
        let t = parse("clair", ":root{--bg:#fff}");
        assert_eq!((t.name.as_str(), t.scheme.as_str()), ("clair", "light"));
        let t = parse("y", "/*\n name: Papier\n scheme: LIGHT\n*/");
        assert_eq!((t.name.as_str(), t.scheme.as_str()), ("Papier", "light"));

        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        assert!(list(&v).unwrap().is_empty());
        let p1 = create_example(&v).unwrap();
        let p2 = create_example(&v).unwrap();
        assert_eq!(p1, ".eloria/themes/Mon thème.css");
        assert_eq!(p2, ".eloria/themes/Mon thème 2.css");
        let l = list(&v).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].scheme, "dark");
        assert_eq!(l[1].name, "Mon thème 2");
    }
}
