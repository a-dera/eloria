//! Adding a PDF without Zotero: the DOI is searched in the file (page text, compressed
//! streams, metadata), then the metadata comes from Crossref. The PDF text,
//! extracted by the built-in reader, is kept in `.eloria/pdftext/` for search and
//! Ask Eloria.

use std::io::Read;

use regex::bytes::Regex;

/// First DOI candidate of a PDF.
pub fn find_doi(bytes: &[u8]) -> Option<String> {
    find_dois(bytes).into_iter().next()
}

/// DOI candidates of a PDF, most likely first. A DOI split between two
/// text strings ("10.55670/fpll.futech" then ".5.4.9") gives two candidates:
/// extended and short. Crossref decides at import.
pub fn find_dois(bytes: &[u8]) -> Vec<String> {
    let re = Regex::new(r"(?i)\b(10\.\d{4,9}/[-._;()/:A-Za-z0-9]+)").unwrap();
    let cont = Regex::new(r"^ ([-./][-._;()/:A-Za-z0-9]+)").unwrap();
    let clean = |d: &[u8]| String::from_utf8_lossy(d).trim_end_matches(['.', ',', ';', ')', '(', '/']).to_lowercase();
    let push = |d: String, out: &mut Vec<String>| {
        if d.len() > 8 && !d.contains('\\') && !out.contains(&d) {
            out.push(d);
        }
    };
    let scan = |data: &[u8], out: &mut Vec<String>| {
        for m in re.find_iter(data) {
            let mut end = m.end();
            let mut long = m.as_bytes().to_vec();
            while let Some(c) = cont.captures(&data[end..]) {
                long.extend_from_slice(&c[1]);
                end += c[0].len();
            }
            if long.len() > m.as_bytes().len() {
                push(clean(&long), out);
            }
            push(clean(m.as_bytes()), out);
            if out.len() >= 4 {
                return;
            }
        }
    };
    let mut out: Vec<String> = Vec::new();
    // Compressed streams first ("stream … endstream"): compression leaves bits
    // of text as is, which a raw read would take for a truncated DOI.
    let mut pos = 0;
    let mut checked = 0;
    while let Some(i) = find(bytes, b"stream", pos) {
        let mut start = i + 6;
        if bytes.get(start) == Some(&b'\r') {
            start += 1;
        }
        if bytes.get(start) == Some(&b'\n') {
            start += 1;
        }
        let Some(end) = find(bytes, b"endstream", start) else { break };
        let mut data = Vec::new();
        // A stream may be followed by end-of-line bytes: what was decompressed counts.
        let _ = flate2::read::ZlibDecoder::new(&bytes[start..end]).take(4_000_000).read_to_end(&mut data);
        if !data.is_empty() {
            scan(&strings_of(&data), &mut out);
            if out.is_empty() {
                scan(&data, &mut out);
            }
            if !out.is_empty() {
                return out;
            }
        }
        pos = end + 9;
        checked += 1;
        if checked > 400 {
            break;
        }
    }
    // Then the raw file (XMP metadata, /Info dictionary), streams excluded.
    let mut plain = Vec::with_capacity(bytes.len());
    let mut p = 0;
    while let Some(i) = find(bytes, b"stream", p) {
        plain.extend_from_slice(&bytes[p..i]);
        match find(bytes, b"endstream", i + 6) {
            Some(e) => p = e + 9,
            None => {
                p = bytes.len();
                break;
            }
        }
    }
    plain.extend_from_slice(&bytes[p.min(bytes.len())..]);
    scan(&plain, &mut out);
    out
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

/// Text of the strings of a page stream: pieces of the same `[(…) -6 (…)] TJ` array
/// form one word (joined), other strings are separated by a space.
fn strings_of(content: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut esc = false;
    let mut in_array = false;
    for &b in content {
        if depth > 0 {
            if esc {
                esc = false;
                out.push(b);
                continue;
            }
            match b {
                b'\\' => esc = true,
                b'(' => {
                    depth += 1;
                    out.push(b);
                }
                b')' => {
                    depth -= 1;
                    if depth > 0 {
                        out.push(b);
                    }
                }
                _ => out.push(b),
            }
            continue;
        }
        match b {
            b'[' => {
                in_array = true;
                out.push(b' ');
            }
            b']' => {
                in_array = false;
                out.push(b' ');
            }
            b'(' => {
                depth = 1;
                if !in_array {
                    out.push(b' ');
                }
            }
            _ => {}
        }
    }
    out
}

/// Fallback title from the file name ("Smith_2024_GNN-fraud.pdf" → "Smith 2024 GNN fraud").
pub fn title_from_filename(name: &str) -> String {
    let stem = name.rsplit(['/', '\\']).next().unwrap_or(name).trim_end_matches(".pdf").trim_end_matches(".PDF");
    stem.replace(['_', '-', '.'], " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Abstract and keywords read from an article's text ("Abstract" or "Résumé" section,
/// "Keywords" or "Mots-clés" line), when no online database provides them (SSRN,
/// Zenodo deposits without description). The text comes from the PDF reader, as is.
pub fn abstract_from_text(text: &str) -> (String, Vec<String>) {
    let head: String = text.chars().take(20_000).collect();
    let start = regex::Regex::new(r"(?i)\b(abstract|résumé|summary)\b\s*[:.\-–—]?\s*").unwrap();
    let stop = regex::Regex::new(r"(?i)\b(key\s?words?|index terms|mots[- ]cl[ée]s|jel classification|(?:1|i)\.?\s+introduction|introduction\s*\n)").unwrap();
    let mut abs = String::new();
    if let Some(m) = start.find(&head) {
        let rest = &head[m.end()..];
        let end = stop.find(rest).map(|s| s.start()).unwrap_or(rest.len().min(3000));
        let cut: String = rest[..end].chars().take(3000).collect();
        let cut = cut.split_whitespace().collect::<Vec<_>>().join(" ");
        // Too short: a running title ("Abstract" in a table of contents).
        if cut.chars().count() >= 150 {
            abs = cut.trim_end_matches(['©', ' ']).to_string();
        }
    }
    let kw_re = regex::Regex::new(r"(?is)\b(?:key\s?words?|index terms|mots[- ]cl[ée]s)\s*[:.\-–—]?\s*(.{3,500}?)(?:\n\s*\n|\b(?:1|i)\.?\s+introduction|\bintroduction\b|\bjel\b|\babstract\b|$)").unwrap();
    let keywords = kw_re
        .captures(&head)
        .map(|c| {
            c[1].split([';', ',', '•', '·', '\n'])
                .map(|k| k.split_whitespace().collect::<Vec<_>>().join(" ").trim_end_matches('.').to_string())
                .filter(|k| k.chars().count() > 2 && k.chars().count() < 60)
                .take(10)
                .collect()
        })
        .unwrap_or_default();
    (abs, keywords)
}

/// Excerpt of a PDF's text (kept by the reader) around the searched words, on whole words.
pub fn snippet(text: &str, query: &str, width: usize) -> Option<String> {
    let low = crate::assist::fold(text);
    let q = crate::assist::fold(query.trim());
    if q.is_empty() {
        return None;
    }
    let i = low.find(&q)?;
    let chars: Vec<char> = text.chars().collect();
    let ci = low[..i].chars().count().min(chars.len());
    let a = ci.saturating_sub(width / 2);
    let b = (ci + q.chars().count() + width / 2).min(chars.len());
    let words: Vec<String> = chars[a..b].iter().collect::<String>().split_whitespace().map(str::to_string).collect();
    if words.is_empty() {
        return None;
    }
    // Words cut at both ends are removed.
    let from = usize::from(a > 0 && chars.get(a - 1).map(|c| !c.is_whitespace()).unwrap_or(false) && words.len() > 1);
    let to = words.len() - usize::from(b < chars.len() && !chars[b].is_whitespace() && words.len() > from + 1);
    Some(words[from..to].join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn pdf_with_stream(content: &[u8]) -> Vec<u8> {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(content).unwrap();
        let mut pdf = b"%PDF-1.7\n1 0 obj << /Filter /FlateDecode >>\nstream\n".to_vec();
        pdf.extend_from_slice(&z.finish().unwrap());
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
        pdf
    }

    #[test]
    fn doi_in_pdf() {
        assert_eq!(find_doi(b"%PDF-1.4\n/Subject (doi:10.1145/2939672.2939778)\n").as_deref(), Some("10.1145/2939672.2939778"));
        // DOI split inside a TJ array, followed by a /Lang string that is not part of it.
        let pdf = pdf_with_stream(b"BT /F1 9 Tf [(https://doi.org/10.)-6(1109/ACC)3(ESS.2024.0001)] TJ ET /Span <</Lang (en-US)>>");
        assert_eq!(find_doi(&pdf).as_deref(), Some("10.1109/access.2024.0001"));
        assert_eq!(find_doi(b"%PDF-1.4 rien"), None);
        // DOI split between two strings: extended candidate, then short candidate.
        let pdf2 = pdf_with_stream(b"BT (doi: 10.55670/fpll.futech) Tj (.5.4.9) Tj ET");
        assert_eq!(find_dois(&pdf2), vec!["10.55670/fpll.futech.5.4.9", "10.55670/fpll.futech"]);
        assert_eq!(title_from_filename("C:/x/Smith_2024_GNN-fraud.pdf"), "Smith 2024 GNN fraud");
        assert_eq!(snippet("Le texte parle de comptes mules et de fraude.", "MULES", 20).as_deref(), Some("comptes mules et de"));
    }

    #[test]
    fn abstract_read_from_pdf() {
        let t = "Mobile Money Social Engineering Attacks\nJ. Doe\n\nAbstract: Mobile money has expanded financial inclusion across Africa, but social engineering attacks such as phishing, vishing and smishing now threaten users. This survey reviews attacks and defences in twelve countries.\n\nKeywords: social engineering; mobile money; phishing; Kenya\n\n1. Introduction\nMobile money...";
        let (abs, kw) = abstract_from_text(t);
        assert!(abs.starts_with("Mobile money has expanded") && abs.ends_with("twelve countries."), "{abs}");
        assert_eq!(kw, vec!["social engineering", "mobile money", "phishing", "Kenya"]);
        // Lone "Abstract" (table of contents): nothing.
        assert_eq!(abstract_from_text("Contents\nAbstract 2\nIntroduction 3").0, "");
    }
}
