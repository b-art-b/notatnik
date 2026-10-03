//! Inline syntax scanner (file-format.md §4): links, tags, block refs, embeds, highlights.
//!
//! Works on one line at a time. Inline code spans, escaped characters and Markdown link
//! destinations are skipped; fenced code is handled by the caller (those lines are never scanned).

use crate::{
    Diagnostic, Ref, RefKind, Severity, Span,
    ids::is_valid_block_id,
    names::{LinkError, PageName},
};

/// Something found in text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineItem {
    /// A link, tag, block ref or embed.
    Ref(Ref),
    /// `==highlight==`
    Highlight(Span),
}

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '/')
}

fn next_boundary(text: &str, mut i: usize) -> usize {
    i += 1;
    while i < text.len() && !text.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Parses `[[...]]` at `i` (which points at the first `[`). Returns the end offset and parts.
fn wikilink(text: &str, i: usize) -> Option<(usize, Span, Option<Span>)> {
    let inner_start = i + 2;
    let rel = text.get(inner_start..)?.find("]]")?;
    let inner_end = inner_start + rel;
    let inner = text.get(inner_start..inner_end)?;
    if inner.contains("[[") {
        return None;
    }
    let (target, label) = match inner.find('|') {
        Some(p) => (
            Span::new(inner_start, inner_start + p),
            Some(Span::new(inner_start + p + 1, inner_end)),
        ),
        None => (Span::new(inner_start, inner_end), None),
    };
    Some((inner_end + 2, target, label))
}

fn run_len(b: &[u8], i: usize, ch: u8) -> usize {
    b.iter().skip(i).take_while(|&&c| c == ch).count()
}

/// Scans one line. `base` is the absolute offset of `text` in the source, added to all spans.
pub fn scan_line(
    text: &str,
    base: usize,
    items: &mut Vec<InlineItem>,
    diags: &mut Vec<Diagnostic>,
) {
    let b = text.as_bytes();
    let abs = |s: Span| s.shifted(base);
    let mut i = 0;
    while i < b.len() {
        let c = b.get(i).copied().unwrap_or(0);
        match c {
            b'\\' => {
                i += if b.get(i + 1).is_some_and(u8::is_ascii_punctuation) {
                    2
                } else {
                    1
                };
            }
            b'`' => {
                let n = run_len(b, i, b'`');
                let mut j = i + n;
                let mut found = None;
                while j < b.len() {
                    if b.get(j) == Some(&b'`') {
                        let m = run_len(b, j, b'`');
                        if m == n {
                            found = Some(j + m);
                            break;
                        }
                        j += m;
                    } else {
                        j += 1;
                    }
                }
                i = found.unwrap_or(i + n);
            }
            b']' if b.get(i + 1) == Some(&b'(') => {
                // Markdown link destination: skip to the closing parenthesis.
                match text.get(i + 2..).and_then(|r| r.find(')')) {
                    Some(p) => i = i + 2 + p + 1,
                    None => i += 1,
                }
            }
            b'!' if b.get(i + 1) == Some(&b'[') && b.get(i + 2) == Some(&b'[') => {
                match wikilink(text, i + 1) {
                    Some((end, target, label)) => {
                        push_page_ref(
                            text,
                            RefKind::PageEmbed,
                            Span::new(i, end),
                            target,
                            label,
                            base,
                            items,
                            diags,
                        );
                        i = end;
                    }
                    None => i += 1,
                }
            }
            b'!' if b.get(i + 1) == Some(&b'(') && b.get(i + 2) == Some(&b'(') => {
                match block_ref(text, i + 1) {
                    Some((end, id)) => {
                        items.push(InlineItem::Ref(Ref {
                            kind: RefKind::BlockEmbed,
                            target: id.slice(text).to_string(),
                            span: abs(Span::new(i, end)),
                            target_span: abs(id),
                            label_span: None,
                        }));
                        i = end;
                    }
                    None => i += 1,
                }
            }
            b'[' if b.get(i + 1) == Some(&b'[') => match wikilink(text, i) {
                Some((end, target, label)) => {
                    push_page_ref(
                        text,
                        RefKind::Page,
                        Span::new(i, end),
                        target,
                        label,
                        base,
                        items,
                        diags,
                    );
                    i = end;
                }
                None => i += 1,
            },
            b'(' if b.get(i + 1) == Some(&b'(') => match block_ref(text, i) {
                Some((end, id)) => {
                    items.push(InlineItem::Ref(Ref {
                        kind: RefKind::Block,
                        target: id.slice(text).to_string(),
                        span: abs(Span::new(i, end)),
                        target_span: abs(id),
                        label_span: None,
                    }));
                    i = end;
                }
                None => i += 1,
            },
            b'#' => {
                let boundary = match text.get(..i).and_then(|p| p.chars().next_back()) {
                    None => true,
                    Some(p) => p.is_whitespace() || p == '(',
                };
                if boundary {
                    if let Some(end) = tag(text, i, base, items) {
                        i = end;
                        continue;
                    }
                }
                i += 1;
            }
            b'=' if b.get(i + 1) == Some(&b'=') => {
                let inner_start = i + 2;
                let found = text
                    .get(inner_start..)
                    .and_then(|r| r.find("=="))
                    .map(|p| inner_start + p);
                match found {
                    Some(e) if e > inner_start => {
                        let inner = text.get(inner_start..e).unwrap_or("");
                        let ws = |c: Option<char>| c.is_none_or(char::is_whitespace);
                        if ws(inner.chars().next()) || ws(inner.chars().next_back()) {
                            i += 2;
                        } else {
                            items.push(InlineItem::Highlight(abs(Span::new(i, e + 2))));
                            i = e + 2;
                        }
                    }
                    _ => i += 2,
                }
            }
            _ if c < 0x80 => i += 1,
            _ => i = next_boundary(text, i),
        }
    }
}

fn block_ref(text: &str, i: usize) -> Option<(usize, Span)> {
    let id_start = i + 2;
    let id = text.get(id_start..id_start + 6)?;
    if !is_valid_block_id(id) || text.get(id_start + 6..id_start + 8) != Some("))") {
        return None;
    }
    Some((id_start + 8, Span::new(id_start, id_start + 6)))
}

#[allow(clippy::too_many_arguments)]
fn push_page_ref(
    text: &str,
    kind: RefKind,
    span: Span,
    target: Span,
    label: Option<Span>,
    base: usize,
    items: &mut Vec<InlineItem>,
    diags: &mut Vec<Diagnostic>,
) {
    match PageName::parse(target.slice(text)) {
        Ok(name) => items.push(InlineItem::Ref(Ref {
            kind,
            target: name.as_str().to_string(),
            span: span.shifted(base),
            target_span: target.shifted(base),
            label_span: label.map(|l| l.shifted(base)),
        })),
        Err(e) => {
            let code = match e {
                LinkError::Empty => "empty-link",
                LinkError::Relative => "relative-link",
            };
            diags.push(Diagnostic::new(
                span.shifted(base),
                Severity::Warning,
                code,
                e.to_string(),
            ));
        }
    }
}

fn tag(text: &str, i: usize, base: usize, items: &mut Vec<InlineItem>) -> Option<usize> {
    let start = i + 1;
    if text.get(start..start + 2) == Some("[[") {
        let (end, target, _) = wikilink(text, start)?;
        let name = PageName::parse(target.slice(text)).ok()?;
        items.push(InlineItem::Ref(Ref {
            kind: RefKind::Tag,
            target: name.as_str().to_string(),
            span: Span::new(i, end).shifted(base),
            target_span: target.shifted(base),
            label_span: None,
        }));
        return Some(end);
    }
    let rest = text.get(start..)?;
    let run: usize = rest
        .chars()
        .take_while(|&c| is_tag_char(c))
        .map(char::len_utf8)
        .sum();
    let trimmed = rest.get(..run)?.trim_end_matches(['-', '/']);
    if trimmed.is_empty() {
        return None;
    }
    let name = PageName::parse(trimmed).ok()?;
    let end = start + trimmed.len();
    items.push(InlineItem::Ref(Ref {
        kind: RefKind::Tag,
        target: name.as_str().to_string(),
        span: Span::new(i, end).shifted(base),
        target_span: Span::new(start, end).shifted(base),
        label_span: None,
    }));
    Some(end)
}

/// Scans a (possibly multi-line) text, line by line, with offsets relative to `text`.
pub fn scan(text: &str) -> (Vec<InlineItem>, Vec<Diagnostic>) {
    let mut items = Vec::new();
    let mut diags = Vec::new();
    let mut pos = 0;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        scan_line(body, pos, &mut items, &mut diags);
        pos += line.len();
    }
    (items, diags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn refs(text: &str) -> Vec<(RefKind, String)> {
        scan(text)
            .0
            .into_iter()
            .filter_map(|i| {
                if let InlineItem::Ref(r) = i {
                    Some((r.kind, r.target))
                } else {
                    None
                }
            })
            .collect()
    }

    #[rstest]
    #[case("see [[work/alpha]] now", vec![(RefKind::Page, "work/alpha")])]
    #[case("[[a|label]]", vec![(RefKind::Page, "a")])]
    #[case("[[ a / b ]]", vec![(RefKind::Page, "a/b")])]
    #[case("[[2026-10-03]]", vec![(RefKind::Page, "2026-10-03")])]
    #[case("#tag", vec![(RefKind::Tag, "tag")])]
    #[case("x #work/alpha y", vec![(RefKind::Tag, "work/alpha")])]
    #[case("#[[multi word]]", vec![(RefKind::Tag, "multi word")])]
    #[case("#zażółć", vec![(RefKind::Tag, "zażółć")])]
    #[case("#123", vec![(RefKind::Tag, "123")])]
    #[case("#tag_name #tagName #tag-name", vec![(RefKind::Tag, "tag_name"), (RefKind::Tag, "tagName"), (RefKind::Tag, "tag-name")])]
    #[case("((abc123))", vec![(RefKind::Block, "abc123")])]
    #[case("!((abc123))", vec![(RefKind::BlockEmbed, "abc123")])]
    #[case("![[page]]", vec![(RefKind::PageEmbed, "page")])]
    #[case("[[a [[b]]]]", vec![(RefKind::Page, "b")])]
    fn finds(#[case] text: &str, #[case] want: Vec<(RefKind, &str)>) {
        let want: Vec<_> = want.into_iter().map(|(k, t)| (k, t.to_string())).collect();
        assert_eq!(refs(text), want, "{text}");
    }

    #[rstest]
    #[case("`[[x]]`")]
    #[case("``a ` [[x]]``")]
    #[case(r"\[[x]]")]
    #[case(r"\#tag")]
    #[case("https://a.b/#frag")]
    #[case("[text](#anchor)")]
    #[case("[text](https://a.b/#frag)")]
    #[case("a#b")]
    #[case("# Heading")]
    #[case("## Heading")]
    #[case("[[unclosed")]
    #[case("((abc12))")]
    #[case("((ABC123))")]
    #[case("#")]
    #[case("#-")]
    fn ignores(#[case] text: &str) {
        assert_eq!(refs(text), vec![], "{text}");
    }

    #[rstest]
    #[case("#tag.", "tag")]
    #[case("(#tag)", "tag")]
    #[case("#tag,", "tag")]
    #[case("#tag-", "tag")]
    #[case("#tag/", "tag")]
    #[case("#123", "123")]
    #[case("#a/b/", "a/b")]
    fn tag_boundaries(#[case] text: &str, #[case] want: &str) {
        assert_eq!(refs(text), vec![(RefKind::Tag, want.to_string())], "{text}");
    }

    #[test]
    fn spans_slice_exactly() {
        let text = "a [[x|label]] b #tag ((abc123)) !((def456)) ![[p]] #[[m w]]";
        let (items, _) = scan(text);
        let slices: Vec<&str> = items
            .iter()
            .filter_map(|i| {
                if let InlineItem::Ref(r) = i {
                    Some(r.span.slice(text))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            slices,
            [
                "[[x|label]]",
                "#tag",
                "((abc123))",
                "!((def456))",
                "![[p]]",
                "#[[m w]]"
            ]
        );
        if let Some(InlineItem::Ref(r)) = items.first() {
            assert_eq!(r.target_span.slice(text), "x");
            assert_eq!(r.label_span.unwrap().slice(text), "label");
        }
    }

    #[test]
    fn highlights() {
        let text = "a ==hl== b == not == c";
        let (items, _) = scan(text);
        let hl: Vec<&str> = items
            .iter()
            .filter_map(|i| {
                if let InlineItem::Highlight(s) = i {
                    Some(s.slice(text))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(hl, ["==hl=="]);
    }

    #[test]
    fn html_is_plain_text() {
        let (items, diags) = scan("<b>x</b> <script>alert(1)</script> [[p]]");
        assert!(diags.is_empty());
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn bad_links_give_diagnostics() {
        let (items, diags) = scan("[[./x]] [[../y]] [[]]");
        assert!(items.is_empty());
        assert_eq!(
            diags.iter().map(|d| d.code).collect::<Vec<_>>(),
            ["relative-link", "relative-link", "empty-link"]
        );
    }
}
