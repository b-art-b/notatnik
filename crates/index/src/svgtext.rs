//! The words in a drawing (DR-14): the text of the `<text>` elements of an `assets/drawings/draw-<8 hex>.svg`, so a
//! search for what was written with the text tool finds the block that shows the drawing. Handwriting is strokes and
//! is not searched. No XML parser: js-draw writes `<text ...>` with `<tspan>` children, and the entities are decoded
//! by hand; a drawing this does not understand just has no words.

use std::collections::HashMap;
use std::path::Path;

/// Longest drawing file read for its words (the same limit as the drawing commands, DR-7).
const MAX_FILE: u64 = 25 * 1024 * 1024;
/// The words of one drawing that are kept; more than this is not a note, it is a text dump.
const MAX_WORDS: usize = 20_000;

/// The `<text>` content of an SVG, one line per text element, entities decoded.
pub(crate) fn svg_text(svg: &str) -> String {
    let mut out = String::new();
    let mut rest = svg;
    while let Some(i) = rest.find("<text") {
        rest = &rest[i + 5..];
        // `<text>` or `<text ...>`, not `<textPath>` and the like
        if !rest.starts_with([' ', '>', '\n', '\t', '\r', '/']) {
            continue;
        }
        let Some(open_end) = rest.find('>') else {
            break;
        };
        if rest[..open_end].ends_with('/') {
            continue;
        }
        let body = &rest[open_end + 1..];
        let Some(close) = body.find("</text>") else {
            break;
        };
        let mut line = String::new();
        let mut in_tag = false;
        for c in body[..close].chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => line.push(c),
                _ => {}
            }
        }
        let line = decode(line.trim());
        if !line.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&line);
        }
        rest = &body[close + 7..];
        if out.len() > MAX_WORDS {
            break;
        }
    }
    out.chars().take(MAX_WORDS).collect()
}

fn decode(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// The graph-relative paths of the drawings a page's text links, each once (`assets/drawings/draw-<8 hex>.svg`, however
/// the link spells the way up to it).
pub(crate) fn drawing_paths(text: &str) -> Vec<String> {
    const DIR: &str = "assets/drawings/draw-";
    let mut found: Vec<String> = Vec::new();
    let mut from = 0;
    while let Some(i) = text[from..].find(DIR) {
        let start = from + i;
        let tail = &text[start + DIR.len()..];
        from = start + DIR.len();
        let hex = tail.get(..8);
        if hex.is_some_and(|h| {
            h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) && tail.get(8..12) == Some(".svg")
        {
            let p = text[start..start + DIR.len() + 12].to_string();
            if !found.contains(&p) {
                found.push(p);
            }
        }
    }
    found
}

/// The words of every drawing a page links, by path. A missing or unreadable file has none.
pub(crate) fn drawings_of(root: &Path, page_text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for rel in drawing_paths(page_text) {
        let abs = root.join(&rel);
        if std::fs::metadata(&abs).map_or(true, |m| m.len() > MAX_FILE) {
            continue;
        }
        if let Ok(svg) = std::fs::read_to_string(&abs) {
            let words = svg_text(&svg);
            if !words.is_empty() {
                out.insert(rel, words);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_text_elements_and_tspans() {
        let svg = r#"<svg><path d="M0 0"/><text x="1" style="font:12px">Router &amp; switch<tspan>, rack 2</tspan></text>
            <g><text>second</text></g><textPath>no</textPath><text/></svg>"#;
        assert_eq!(svg_text(svg), "Router & switch, rack 2\nsecond");
    }

    #[test]
    fn a_drawing_without_text_has_no_words() {
        assert_eq!(svg_text("<svg><path d=\"M0 0 L1 1\"/></svg>"), "");
    }

    #[test]
    fn finds_the_drawings_a_page_links_however_the_link_climbs() {
        let t = "- ![a](../assets/drawings/draw-0123abcd.svg)\n- [b](assets/drawings/draw-0123abcd.svg) ![c](../../assets/drawings/draw-ffffffff.svg) ![x](assets/drawings/draw-zzzzzzzz.svg) assets/drawings/draw-1234.svg";
        assert_eq!(
            drawing_paths(t),
            vec![
                "assets/drawings/draw-0123abcd.svg".to_string(),
                "assets/drawings/draw-ffffffff.svg".to_string()
            ]
        );
    }
}
