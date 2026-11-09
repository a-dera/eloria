//! PDF annotations made in Eloria: highlights (colour, text, comment) and
//! page notes. Independent of Zotero.
//!
//! Storage: `.eloria/annotations/<PDF path>.json` (readable, one entry per
//! annotation). To make them searchable and visible in the source, the PDF's source
//! note gets a regenerated block between `<!-- eloria:annotations:debut -->` and
//! `<!-- eloria:annotations:fin -->`; text written outside the block is never touched.

use serde::{Deserialize, Serialize};

use crate::lang::Lang;
use crate::vault::Vault;
use crate::Result;

pub const START: &str = "<!-- eloria:annotations:debut -->";
pub const END: &str = "<!-- eloria:annotations:fin -->";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PdfAnnotation {
    pub id: String,
    /// Physical page (1 = first)
    pub page: u32,
    /// "highlight" or "note"
    pub kind: String,
    /// jaune, vert, bleu, rouge, violet
    pub color: String,
    /// Highlighted text (empty for a page note)
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub comment: String,
    /// Rectangles of the highlight, as fractions of the page: [x, y, width, height]
    #[serde(default)]
    pub rects: Vec<[f32; 4]>,
    #[serde(default)]
    pub created: String,
}

fn store_rel(pdf_rel: &str) -> std::path::PathBuf {
    std::path::PathBuf::from("annotations").join(format!("{pdf_rel}.json"))
}

pub fn load(vault: &Vault, pdf_rel: &str) -> Result<Vec<PdfAnnotation>> {
    let p = vault.eloria_dir().join(store_rel(pdf_rel));
    match std::fs::read_to_string(&p) {
        Ok(s) => Ok(serde_json::from_str(&s)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e.into()),
    }
}

/// Saves the annotations (sorted by page then position), atomically.
pub fn save(vault: &Vault, pdf_rel: &str, list: &[PdfAnnotation]) -> Result<Vec<PdfAnnotation>> {
    vault.resolve(pdf_rel)?; // the PDF must be inside the vault
    let mut list = list.to_vec();
    list.sort_by(|a, b| a.page.cmp(&b.page).then(a.rects.first().map(|r| r[1]).unwrap_or(0.0).total_cmp(&b.rects.first().map(|r| r[1]).unwrap_or(0.0))));
    let p = vault.eloria_dir().join(store_rel(pdf_rel));
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&list)?)?;
    std::fs::rename(&tmp, &p)?;
    Ok(list)
}

fn color_name(c: &str, lang: Lang) -> &'static str {
    match (c, lang) {
        ("vert", Lang::Fr) => "vert",
        ("bleu", Lang::Fr) => "bleu",
        ("rouge", Lang::Fr) => "rouge",
        ("violet", Lang::Fr) => "violet",
        (_, Lang::Fr) => "jaune",
        ("vert", Lang::En) => "green",
        ("bleu", Lang::En) => "blue",
        ("rouge", Lang::En) => "red",
        ("violet", Lang::En) => "purple",
        (_, Lang::En) => "yellow",
    }
}

/// Markdown block of the annotations, for the source note (`eloria-pdf:` link to the page).
pub fn block(pdf_rel: &str, list: &[PdfAnnotation], lang: Lang) -> String {
    let mut s = format!("{START}\n## {}\n\n", lang.pick("Annotations dans Eloria", "Annotations in Eloria"));
    if list.is_empty() {
        s.push_str(lang.pick("*Aucune annotation pour l'instant : ouvre le PDF dans Eloria et surligne.*\n", "*No annotations yet: open the PDF in Eloria and highlight.*\n"));
    }
    let link = pdf_rel.replace(' ', "%20");
    for a in list {
        let kind = if a.kind == "note" { "note" } else { "citation" };
        s.push_str(&format!("> [!{kind}|{}] p. {} · [{}](eloria-pdf:{link}#page={})\n", color_name(&a.color, lang), a.page, lang.pick("voir dans le PDF", "see in the PDF"), a.page));
        if !a.text.trim().is_empty() {
            s.push_str(&format!("> {}\n", a.text.trim().replace('\n', " ")));
        }
        if !a.comment.trim().is_empty() {
            if !a.text.trim().is_empty() {
                s.push_str(">\n");
            }
            s.push_str(&format!("> **{}** {}\n", lang.pick("Commentaire :", "Comment:"), a.comment.trim().replace('\n', " ")));
        }
        s.push('\n');
    }
    s.push_str(END);
    s
}

/// Replaces (or appends at the end) the annotation block of a note.
pub fn replace_block(content: &str, block: &str) -> String {
    match (content.find(START), content.find(END)) {
        (Some(a), Some(b)) if b > a => format!("{}{}{}", &content[..a], block, &content[b + END.len()..]),
        _ => format!("{}\n\n{block}\n", content.trim_end()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(page: u32, y: f32, text: &str) -> PdfAnnotation {
        PdfAnnotation { id: format!("{page}-{y}"), page, kind: "highlight".into(), color: "vert".into(), text: text.into(), comment: String::new(), rects: vec![[0.1, y, 0.5, 0.02]], created: String::new() }
    }

    #[test]
    fn storage_and_block() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        v.write_bytes("PDF/Thèse/@a.pdf", b"%PDF-1.4").unwrap();
        assert!(load(&v, "PDF/Thèse/@a.pdf").unwrap().is_empty());
        let saved = save(&v, "PDF/Thèse/@a.pdf", &[a(3, 0.5, "B"), a(1, 0.2, "A"), a(3, 0.1, "C")]).unwrap();
        assert_eq!(saved.iter().map(|x| x.text.as_str()).collect::<String>(), "ACB");
        assert_eq!(load(&v, "PDF/Thèse/@a.pdf").unwrap(), saved);
        assert!(save(&v, "../hors.pdf", &[]).is_err());

        let mut n = a(2, 0.0, "");
        n.kind = "note".into();
        n.comment = "Idée pour le ch. 4".into();
        let b = block("PDF/Thèse/@a.pdf", &[saved[0].clone(), n], Lang::Fr);
        assert!(b.starts_with(START) && b.ends_with(END));
        assert!(b.contains("> [!citation|vert] p. 1 · [voir dans le PDF](eloria-pdf:PDF/Thèse/@a.pdf#page=1)\n> A\n"));
        assert!(b.contains("> [!note|vert] p. 2") && b.contains("> **Commentaire :** Idée pour le ch. 4"));
        // Text outside the block is kept; the block is replaced, not duplicated.
        let note = "# Source\n\nMon texte.";
        let once = replace_block(note, &b);
        let twice = replace_block(&once, &block("PDF/Thèse/@a.pdf", &[], Lang::Fr));
        assert!(twice.contains("Mon texte.") && twice.matches(START).count() == 1 && twice.contains("Aucune annotation"));
    }
}
