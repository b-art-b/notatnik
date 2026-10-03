//! Page names, link normalisation and the page name ⇔ file path mapping
//! (file-format.md §2; requirements NS-1, NS-2, LN-1).

use std::fmt;

use chrono::NaiveDate;
use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

/// A normalised page name: NFC, `/`-separated, no empty or relative segments, segments trimmed.
///
/// Matching is case-insensitive via [`PageName::lookup_key`]; the original case is kept for display.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct PageName(String);

/// Why a link target could not be normalised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkError {
    /// Nothing left after trimming.
    Empty,
    /// `.` or `..` segment.
    Relative,
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::Empty => f.write_str("empty page name"),
            LinkError::Relative => {
                f.write_str("relative links are not supported; use the full page name")
            }
        }
    }
}

impl PageName {
    /// Normalises a raw link target (see P1.13): trims the name and every segment, collapses `//`,
    /// strips leading/trailing `/`, NFC-normalises. `.`/`..` segments are rejected.
    pub fn parse(raw: &str) -> Result<PageName, LinkError> {
        let mut segments: Vec<String> = Vec::new();
        for seg in raw.split('/') {
            let seg = seg.trim();
            if seg == "." || seg == ".." {
                return Err(LinkError::Relative);
            }
            if !seg.is_empty() {
                segments.push(seg.nfc().collect());
            }
        }
        if segments.is_empty() {
            return Err(LinkError::Empty);
        }
        Ok(PageName(segments.join("/")))
    }

    /// The name as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Case-folded key used for lookups; NFC- and NFD-spelled names give the same key.
    pub fn lookup_key(&self) -> String {
        self.0.nfc().collect::<String>().to_lowercase()
    }

    /// The parent namespace (`work/projects` for `work/projects/alpha`), if any.
    pub fn parent(&self) -> Option<PageName> {
        self.0
            .rfind('/')
            .map(|i| PageName(self.0.get(..i).unwrap_or("").to_string()))
    }

    /// All ancestors, nearest first (`work/projects`, `work`).
    pub fn ancestors(&self) -> Vec<PageName> {
        let mut out = Vec::new();
        let mut cur = self.parent();
        while let Some(p) = cur {
            cur = p.parent();
            out.push(p);
        }
        out
    }

    /// The last segment (`alpha` for `work/projects/alpha`).
    pub fn leaf(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or("")
    }

    /// True when the name is a journal date (`2026-10-03`).
    pub fn journal_date(&self) -> Option<NaiveDate> {
        parse_journal_date(&self.0)
    }
}

impl fmt::Display for PageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Convenience wrapper around [`PageName::parse`].
pub fn normalize_link(raw: &str) -> Result<PageName, LinkError> {
    PageName::parse(raw)
}

fn parse_journal_date(s: &str) -> Option<NaiveDate> {
    let b = s.as_bytes();
    if b.len() != 10 || b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') {
        return None;
    }
    if !b
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn is_unsafe_char(c: char) -> bool {
    matches!(c, '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '%') || c.is_control()
}

fn push_encoded(out: &mut String, c: char) {
    let mut buf = [0u8; 4];
    for b in c.encode_utf8(&mut buf).bytes() {
        out.push_str(&format!("%{b:02X}"));
    }
}

fn encode_segment(seg: &str) -> String {
    let chars: Vec<char> = seg.chars().collect();
    let last = chars.len().saturating_sub(1);
    let mut out = String::with_capacity(seg.len());
    for (i, &c) in chars.iter().enumerate() {
        let edge = (i == 0 && c == '.') || (i == last && (c == '.' || c == ' '));
        if is_unsafe_char(c) || edge {
            push_encoded(&mut out, c);
        } else {
            out.push(c);
        }
    }
    out
}

fn decode_segment(seg: &str) -> Option<String> {
    let b = seg.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let c = *b.get(i)?;
        if c == b'%' {
            let hi = b.get(i + 1).and_then(|&x| (x as char).to_digit(16));
            let lo = b.get(i + 2).and_then(|&x| (x as char).to_digit(16));
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    String::from_utf8(out).ok()
}

/// Relative path (always `/`-separated, inside the graph) of the file that holds `name`.
///
/// Journal dates map to `journals/YYYY/YYYY-MM-DD.md`, everything else to `pages/<name>.md`.
pub fn page_to_path(name: &PageName) -> String {
    if let Some(d) = name.journal_date() {
        return format!("journals/{}/{}.md", d.format("%Y"), name.as_str());
    }
    let segs: Vec<String> = name.as_str().split('/').map(encode_segment).collect();
    format!("pages/{}.md", segs.join("/"))
}

/// The page that lives in the file at `path` (relative to the graph root), if it is a page file.
///
/// Returns `None` for anything outside `pages/` and `journals/`, non-`.md` files, journal files with
/// a wrong name, and files in `pages/` named like a journal date (journal names are reserved).
pub fn path_to_page(path: &str) -> Option<PageName> {
    let path = path.replace('\\', "/");
    if let Some(rest) = path.strip_prefix("journals/") {
        let (year, file) = rest.split_once('/')?;
        let stem = file.strip_suffix(".md")?;
        let date = parse_journal_date(stem)?;
        if date.format("%Y").to_string() != year {
            return None;
        }
        return PageName::parse(stem).ok();
    }
    let rest = path.strip_prefix("pages/")?;
    let stem = rest.strip_suffix(".md")?;
    let mut decoded = Vec::new();
    for seg in stem.split('/') {
        decoded.push(decode_segment(seg)?);
    }
    let name = PageName::parse(&decoded.join("/")).ok()?;
    if name.journal_date().is_some() {
        return None;
    }
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[rstest]
    #[case("work/projects/alpha", "pages/work/projects/alpha.md")]
    #[case("a: b", "pages/a%3A b.md")]
    #[case("100%", "pages/100%25.md")]
    #[case("x.", "pages/x%2E.md")]
    #[case("2026-10-03", "journals/2026/2026-10-03.md")]
    #[case("zażółć", "pages/zażółć.md")]
    #[case("what?", "pages/what%3F.md")]
    #[case(".hidden", "pages/%2Ehidden.md")]
    fn name_and_path_round_trip(#[case] name: &str, #[case] path: &str) {
        let n = PageName::parse(name).unwrap();
        assert_eq!(page_to_path(&n), path);
        assert_eq!(path_to_page(path), Some(n));
    }

    #[test]
    fn lookup_keys() {
        let a = PageName::parse("Work/Alpha").unwrap();
        let b = PageName::parse("work/alpha").unwrap();
        assert_eq!(a.lookup_key(), b.lookup_key());
        let nfc = PageName::parse("za\u{17c}\u{f3}\u{142}\u{107}").unwrap();
        let nfd = PageName::parse("zaz\u{307}o\u{301}\u{142}c\u{301}").unwrap();
        assert_eq!(nfc.lookup_key(), nfd.lookup_key());
        assert_eq!(nfc, nfd);
    }

    #[rstest]
    #[case("assets/x.md")]
    #[case(".notatnik/config.toml")]
    #[case("pages/x.txt")]
    #[case("journals/x.md")]
    #[case("journals/2025/2026-10-03.md")]
    #[case("pages/2026-10-03.md")]
    #[case("notes.md")]
    fn unmapped_paths(#[case] path: &str) {
        assert_eq!(path_to_page(path), None);
    }

    #[rstest]
    #[case("[[ work / projects/alpha ]]", "work/projects/alpha")]
    #[case(" work / projects/alpha ", "work/projects/alpha")]
    #[case("/work/alpha/", "work/alpha")]
    #[case("work//alpha", "work/alpha")]
    fn normalizes(#[case] raw: &str, #[case] want: &str) {
        let raw = raw.trim_start_matches("[[").trim_end_matches("]]");
        assert_eq!(PageName::parse(raw).unwrap().as_str(), want);
    }

    #[rstest]
    #[case("./x", LinkError::Relative)]
    #[case("../x", LinkError::Relative)]
    #[case("", LinkError::Empty)]
    #[case("/", LinkError::Empty)]
    #[case("  ", LinkError::Empty)]
    fn rejects(#[case] raw: &str, #[case] err: LinkError) {
        assert_eq!(PageName::parse(raw), Err(err));
    }

    #[test]
    fn ancestors_and_leaf() {
        let n = PageName::parse("work/projects/alpha").unwrap();
        let anc: Vec<_> = n.ancestors().iter().map(|p| p.to_string()).collect();
        assert_eq!(anc, ["work/projects", "work"]);
        assert_eq!(n.leaf(), "alpha");
    }

    proptest! {
        #[test]
        fn any_valid_name_round_trips(raw in "[^\u{0}]{1,40}") {
            if let Ok(n) = PageName::parse(&raw) {
                let path = page_to_path(&n);
                prop_assert_eq!(path_to_page(&path), Some(n));
            }
        }
    }
}
